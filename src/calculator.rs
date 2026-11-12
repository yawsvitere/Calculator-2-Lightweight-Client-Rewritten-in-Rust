use rodio::{buffer::SamplesBuffer, Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ---------- Генератор случайных чисел (без внешних крейтов) ----------

struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);
        Rng(n ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
}

// ---------- Операции (с неправильной математикой) ----------

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "×",
            Op::Div => "÷",
        }
    }

    /// Считает неправильно, причём каждая операция по-своему.
    fn apply(self, a: f64, b: f64, rng: &mut Rng) -> f64 {
        match self {
            Op::Add => a + b + rng.range(1, 3) as f64, // лишние 1..3
            Op::Sub => b - a,                          // аргументы наоборот
            Op::Mul => a * (b + 1.0),                  // множитель на единицу больше
            Op::Div => {
                if b == 0.0 {
                    42.0 // на ноль делить можно
                } else {
                    (a / b).floor() + 0.5 // округляем вниз и добавляем половинку
                }
            }
        }
    }

    fn apply_correctly(self, a: f64, b: f64) -> f64 {
        match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => a / b,
        }
    }
}

/// Форматирование числа: без хвостовых нулей, научная запись для очень больших/малых.
fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "∞".to_string();
    }
    if v != 0.0 && (v.abs() >= 1e15 || v.abs() < 1e-9) {
        return format!("{:e}", v);
    }
    let mut s = format!("{:.10}", v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

// ---------- Выключение компьютера при делении на ноль ----------

/// Сколько секунд даётся на отмену (клавиша C или Esc)
pub(crate) const SHUTDOWN_DELAY: u32 = 10;

#[cfg(windows)]
fn run_shutdown(args: &[&str]) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("shutdown")
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW: без мигающей консоли
        .spawn();
}

#[cfg(not(windows))]
fn run_shutdown(_args: &[&str]) {}

// ---------- «Тяжёлые вычисления»: звук заводящейся машины и бесконечные формулы ----------

/// Звук вшит в .exe, отдельный файл рядом не нужен (лежит в src/car.mp3)
static CAR_SOUND: &[u8] = include_bytes!("car.mp3");
static SUPERMARKET_MIDI: &[u8] = include_bytes!("supermarket.mid");
const MOLLY_LYRICS: &str = include_str!("molly.lrc");
const MIDI_SAMPLE_RATE: u32 = 44_100;
const MIDI_BPM: u32 = 75;

/// Сколько секунд калькулятор «думает»
const CALC_SECONDS: u64 = 9;

fn parse_lyrics(lrc: &str) -> Vec<LyricLine> {
    lrc.lines()
        .filter_map(|line| {
            let line = line.trim_start_matches('\u{feff}');
            let (timestamp, text) = line.strip_prefix('[')?.split_once(']')?;
            let (minutes, seconds) = timestamp.split_once(':')?;
            let at = minutes.parse::<u64>().ok()? as f64 + seconds.parse::<f64>().ok()? / 60.0;
            Some(LyricLine {
                at: std::time::Duration::from_secs_f64(at * 60.0),
                text: text.to_string(),
            })
        })
        .collect()
}

fn read_vlq(bytes: &[u8], pos: &mut usize) -> Option<u32> {
    let mut value = 0u32;
    for _ in 0..4 {
        let byte = *bytes.get(*pos)?;
        *pos += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn parse_midi(bytes: &[u8]) -> Result<(u16, Vec<MidiNote>), &'static str> {
    if bytes.get(..4) != Some(b"MThd") {
        return Err("missing MIDI header");
    }
    let header_len = u32::from_be_bytes(
        bytes
            .get(4..8)
            .ok_or("truncated MIDI header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let header_end = 8usize
        .checked_add(header_len)
        .ok_or("invalid MIDI header length")?;
    let header = bytes.get(8..header_end).ok_or("truncated MIDI header")?;
    if header.len() < 6 {
        return Err("short MIDI header");
    }
    let track_count = u16::from_be_bytes([header[2], header[3]]) as usize;
    let division = u16::from_be_bytes([header[4], header[5]]);
    if division & 0x8000 != 0 || division == 0 {
        return Err("unsupported MIDI time division");
    }

    let mut events = Vec::new();
    let mut offset = header_end;

    for _ in 0..track_count {
        if bytes.get(offset..offset + 4) != Some(b"MTrk") {
            return Err("missing MIDI track");
        }
        let len = u32::from_be_bytes(
            bytes
                .get(offset + 4..offset + 8)
                .ok_or("truncated MIDI track header")?
                .try_into()
                .unwrap(),
        ) as usize;
        let start = offset + 8;
        let end = start.checked_add(len).ok_or("invalid MIDI track length")?;
        let track = bytes.get(start..end).ok_or("truncated MIDI track")?;
        offset = end;

        let mut pos = 0;
        let mut tick = 0u64;
        let mut running_status = None;
        while pos < track.len() {
            tick += u64::from(read_vlq(track, &mut pos).ok_or("invalid MIDI delta time")?);
            let first = *track.get(pos).ok_or("truncated MIDI event")?;
            let status = if first & 0x80 != 0 {
                pos += 1;
                if first < 0xf0 {
                    running_status = Some(first);
                }
                first
            } else {
                running_status.ok_or("invalid MIDI running status")?
            };

            match status {
                0xff => {
                    let meta_type = *track.get(pos).ok_or("truncated MIDI meta event")?;
                    pos += 1;
                    let meta_len =
                        read_vlq(track, &mut pos).ok_or("invalid MIDI meta event length")? as usize;
                    track
                        .get(pos..pos + meta_len)
                        .ok_or("truncated MIDI meta event data")?;
                    pos += meta_len;
                    if meta_type == 0x2f {
                        break;
                    }
                }
                0xf0 | 0xf7 => {
                    let sysex_len =
                        read_vlq(track, &mut pos).ok_or("invalid MIDI SysEx length")? as usize;
                    pos = pos
                        .checked_add(sysex_len)
                        .filter(|&end| end <= track.len())
                        .ok_or("truncated MIDI SysEx event")?;
                    running_status = None;
                }
                0x80..=0xef => {
                    let kind = status & 0xf0;
                    let note = *track.get(pos).ok_or("truncated MIDI channel event")?;
                    pos += 1;
                    if note >= 0x80 {
                        return Err("invalid MIDI channel data");
                    }
                    let one_byte = matches!(kind, 0xc0 | 0xd0);
                    let value = if one_byte {
                        0
                    } else {
                        let value = *track.get(pos).ok_or("truncated MIDI channel event")?;
                        pos += 1;
                        value
                    };
                    if value >= 0x80 {
                        return Err("invalid MIDI channel data");
                    }
                    if matches!(kind, 0x80 | 0x90) {
                        events.push(MidiEvent {
                            tick,
                            channel: status & 0x0f,
                            note,
                            velocity: if kind == 0x90 && value > 0 { value } else { 0 },
                        });
                    }
                }
                _ => return Err("unsupported MIDI event"),
            }
        }
    }

    let mut active: Vec<Vec<Vec<(u64, u8)>>> = (0..16)
        .map(|_| (0..128).map(|_| Vec::new()).collect())
        .collect();
    let mut notes = Vec::new();
    events.sort_by_key(|event| event.tick);
    for event in events {
        let sounding = &mut active[event.channel as usize][event.note as usize];
        if event.velocity > 0 {
            sounding.push((event.tick, event.velocity));
        } else if let Some((start_tick, velocity)) = sounding.pop() {
            notes.push(MidiNote {
                start_tick,
                end_tick: event.tick,
                note: event.note,
                velocity,
            });
        }
    }

    Ok((division, notes))
}

fn synthesize_midi(bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
    let (ticks_per_quarter, notes) = parse_midi(bytes)?;
    if notes.is_empty() {
        return Err("MIDI contains no complete notes");
    }

    let to_seconds =
        |tick: u64| tick as f64 * 60.0 / (f64::from(ticks_per_quarter) * f64::from(MIDI_BPM));

    const RELEASE_SECONDS: f64 = 0.12;
    let end_seconds = notes
        .iter()
        .map(|note| to_seconds(note.end_tick))
        .fold(0.0_f64, f64::max)
        + RELEASE_SECONDS;
    if !end_seconds.is_finite() || end_seconds > 600.0 {
        return Err("MIDI duration is invalid or longer than 10 minutes");
    }

    let mut samples = vec![0.0_f32; (end_seconds * f64::from(MIDI_SAMPLE_RATE)).ceil() as usize];
    let fifth_ratio = 2.0_f64.powf(7.0 / 12.0);
    for note in notes {
        let start = (to_seconds(note.start_tick) * f64::from(MIDI_SAMPLE_RATE)) as usize;
        let end = (to_seconds(note.end_tick) * f64::from(MIDI_SAMPLE_RATE)) as usize;
        let release_frames = (RELEASE_SECONDS * f64::from(MIDI_SAMPLE_RATE)) as usize;
        let voice_end = (end + release_frames).min(samples.len());
        let frequency = 440.0 * 2.0_f64.powf((f64::from(note.note) - 69.0) / 12.0);
        let amplitude = f64::from(note.velocity) / 127.0 * 0.10;

        for (frame, sample) in samples.iter_mut().enumerate().take(voice_end).skip(start) {
            let elapsed = frame - start;
            let release_left = voice_end.saturating_sub(frame);
            let attack = (elapsed as f64 / (0.008 * f64::from(MIDI_SAMPLE_RATE))).min(1.0);
            let release = if frame >= end {
                (release_left as f64 / release_frames as f64).min(1.0)
            } else {
                1.0
            };
            let phase = ((elapsed as f64 * frequency / f64::from(MIDI_SAMPLE_RATE)) % 1.0) as f32;
            let fifth_phase = ((elapsed as f64 * frequency * fifth_ratio
                / f64::from(MIDI_SAMPLE_RATE))
                % 1.0) as f32;
            let root_square = if phase < 0.5 { 1.0 } else { -1.0 };
            let fifth_square = if fifth_phase < 0.5 { 1.0 } else { -1.0 };
            let saw = phase * 2.0 - 1.0;
            let waveform = 0.55 * root_square + 0.30 * fifth_square + 0.15 * saw;
            *sample += (f64::from(waveform) * amplitude * attack * release) as f32;
        }
    }
    for sample in &mut samples {
        *sample = sample.clamp(-0.95, 0.95);
    }
    Ok(samples)
}

fn start_midi_audio() -> Result<(OutputStream, Sink), String> {
    let samples = synthesize_midi(SUPERMARKET_MIDI).map_err(str::to_string)?;
    let (stream, handle) = OutputStream::try_default().map_err(|error| error.to_string())?;
    let sink = Sink::try_new(&handle).map_err(|error| error.to_string())?;
    sink.append(SamplesBuffer::new(1, MIDI_SAMPLE_RATE, samples));
    Ok((stream, sink))
}

/// Запускает воспроизведение. Поток нужно держать живым, пока играет звук.
fn start_sound() -> Option<(OutputStream, Sink)> {
    start_audio(CAR_SOUND)
}

fn start_audio(data: &'static [u8]) -> Option<(OutputStream, Sink)> {
    let (stream, handle) = OutputStream::try_default().ok()?;
    let sink = Sink::try_new(&handle).ok()?;
    let source = Decoder::new(Cursor::new(data)).ok()?;
    sink.append(source);
    Some((stream, sink))
}

/// Что делать, когда «вычисление» закончится
#[derive(Clone, Copy)]
enum Action {
    Equals { a: f64, op: Op, b: f64 },
    Chain { new: Op },
}

pub(crate) struct Busy {
    pub(crate) start: Instant,
    action: Action,
    result: f64,
}

pub(crate) struct LyricLine {
    pub(crate) at: std::time::Duration,
    pub(crate) text: String,
}

struct MidiNote {
    start_tick: u64,
    end_tick: u64,
    note: u8,
    velocity: u8,
}

struct MidiEvent {
    tick: u64,
    channel: u8,
    note: u8,
    velocity: u8,
}

pub(crate) struct Karaoke {
    pub(crate) start: Instant,
    pub(crate) lines: Vec<LyricLine>,
    action: Action,
    result: f64,
}

// ---------- Состояние калькулятора ----------

pub(crate) struct Calculator {
    pub(crate) current: String,
    acc: Option<f64>,
    pub(crate) op: Option<Op>,
    pub(crate) expr: String,
    pub(crate) fresh: bool,
    rng: Rng,
    calculations: u64,
    pub(crate) doom: Option<Instant>, // когда запущено выключение
    pub(crate) busy: Option<Busy>,    // идёт «вычисление»
    pub(crate) karaoke: Option<Karaoke>,
    audio: Option<(OutputStream, Sink)>,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            current: "0".to_string(),
            acc: None,
            op: None,
            expr: String::new(),
            fresh: true,
            rng: Rng::new(),
            calculations: 0,
            doom: None,
            busy: None,
            karaoke: None,
            audio: None,
        }
    }
}

impl Calculator {
    fn value(&self) -> f64 {
        self.current.parse().unwrap_or(0.0)
    }

    fn reset(&mut self) {
        let calculations = self.calculations;
        *self = Self::default();
        self.calculations = calculations;
    }

    /// Вычисление с перехватом деления на ноль: запускает выключение ПК.
    fn calc(&mut self, op: Op, a: f64, b: f64) -> f64 {
        if op == Op::Div && b == 0.0 {
            if self.doom.is_none() {
                self.doom = Some(Instant::now());
                run_shutdown(&[
                    "/s",
                    "/t",
                    &SHUTDOWN_DELAY.to_string(),
                    "/c",
                    "Деление на ноль. Компьютер выключается.",
                ]);
            }
            return 0.0;
        }
        self.calculations += 1;
        if self.calculations <= 3 {
            op.apply_correctly(a, b)
        } else {
            op.apply(a, b, &mut self.rng)
        }
    }

    fn needs_busy_mode(a: f64, b: f64) -> bool {
        [a, b]
            .iter()
            .any(|value| fmt(*value).chars().filter(char::is_ascii_digit).count() > 1)
    }

    fn is_molly_sum(op: Op, a: f64, b: f64) -> bool {
        op == Op::Add && (a + b - 2017.0).abs() < f64::EPSILON * 2017.0
    }

    fn digit(&mut self, d: &str) {
        if self.fresh {
            if self.op.is_none() {
                self.expr.clear();
            }
            self.current = d.to_string();
            self.fresh = false;
        } else if self.current == "0" {
            self.current = d.to_string();
        } else if self.current == "-0" {
            self.current = format!("-{}", d);
        } else if self.current.chars().filter(|c| c.is_ascii_digit()).count() < 15 {
            self.current.push_str(d);
        }
    }

    fn dot(&mut self) {
        if self.fresh {
            if self.op.is_none() {
                self.expr.clear();
            }
            self.current = "0.".to_string();
            self.fresh = false;
        } else if !self.current.contains('.') {
            self.current.push('.');
        }
    }

    fn negate(&mut self) {
        if self.current == "0" {
            return;
        }
        if let Some(rest) = self.current.strip_prefix('-') {
            self.current = rest.to_string();
        } else {
            self.current.insert(0, '-');
        }
    }

    fn backspace(&mut self) {
        if self.fresh {
            return;
        }
        self.current.pop();
        if self.current.is_empty() || self.current == "-" {
            self.current = "0".to_string();
        }
    }

    fn percent(&mut self) {
        // «Процент» тоже врёт: делит на 10 вместо 100
        let r = self.value() / 10.0;
        self.current = fmt(r);
        self.fresh = false;
    }

    fn set_op(&mut self, new: Op) {
        let v = self.value();
        if let (Some(a), Some(cur_op)) = (self.acc, self.op) {
            if self.fresh {
                // оператор просто заменили, не вводя второе число
                self.op = Some(new);
                self.expr = format!("{} {}", fmt(a), new.symbol());
                return;
            }
            let r = self.calc(cur_op, a, v);
            if self.doom.is_none() {
                let action = Action::Chain { new };
                self.expr = format!("{} {} {} =", fmt(a), cur_op.symbol(), fmt(v));
                if Self::is_molly_sum(cur_op, a, v) {
                    self.start_karaoke(action, r);
                } else if Self::needs_busy_mode(a, v) {
                    self.start_busy(action, r);
                } else {
                    self.finish_action(action, r);
                }
            }
            return;
        } else {
            self.acc = Some(v);
        }
        self.op = Some(new);
        self.fresh = true;
        self.expr = format!("{} {}", fmt(self.acc.unwrap_or(0.0)), new.symbol());
    }

    fn equals(&mut self) {
        let (Some(a), Some(op)) = (self.acc, self.op) else {
            return;
        };
        let b = self.value();
        let r = self.calc(op, a, b);
        if self.doom.is_none() {
            let action = Action::Equals { a, op, b };
            self.expr = format!("{} {} {} =", fmt(a), op.symbol(), fmt(b));
            if Self::is_molly_sum(op, a, b) {
                self.start_karaoke(action, r);
            } else if Self::needs_busy_mode(a, b) {
                self.start_busy(action, r);
            } else {
                self.finish_action(action, r);
            }
        }
    }

    fn start_busy(&mut self, action: Action, result: f64) {
        self.busy = Some(Busy {
            start: Instant::now(),
            action,
            result,
        });
        self.audio = start_sound();
    }

    fn start_karaoke(&mut self, action: Action, result: f64) {
        let audio = match start_midi_audio() {
            Ok(audio) => audio,
            Err(error) => {
                eprintln!("Не удалось воспроизвести supermarket.mid: {error}");
                self.finish_action(action, result);
                return;
            }
        };
        self.audio = Some(audio);
        self.karaoke = Some(Karaoke {
            start: Instant::now(),
            lines: parse_lyrics(MOLLY_LYRICS),
            action,
            result,
        });
    }

    fn finish_busy(&mut self) {
        let Some(job) = self.busy.take() else {
            return;
        };
        self.audio = None; // звук обрывается
        self.finish_action(job.action, job.result);
    }

    fn finish_karaoke(&mut self) {
        let Some(karaoke) = self.karaoke.take() else {
            return;
        };
        self.audio = None;
        self.finish_action(karaoke.action, karaoke.result);
    }

    fn finish_action(&mut self, action: Action, result: f64) {
        match action {
            Action::Equals { a, op, b } => {
                self.expr = format!("{} {} {} =", fmt(a), op.symbol(), fmt(b));
                self.current = fmt(result);
                self.acc = None;
                self.op = None;
                self.fresh = true;
            }
            Action::Chain { new } => {
                self.acc = Some(result);
                self.current = fmt(result);
                self.op = Some(new);
                self.fresh = true;
                self.expr = format!("{} {}", fmt(result), new.symbol());
            }
        }
    }

    pub(crate) fn tick(&mut self) -> bool {
        if self.karaoke.is_some() {
            if self
                .audio
                .as_ref()
                .map(|(_, sink)| sink.empty())
                .unwrap_or(true)
            {
                self.finish_karaoke();
            } else {
                return true;
            }
            return false;
        }
        let done = self
            .busy
            .as_ref()
            .map(|b| b.start.elapsed() >= std::time::Duration::from_secs(CALC_SECONDS));
        match done {
            Some(true) => self.finish_busy(),
            Some(false) => return true,
            None => {}
        }
        false
    }
    pub(crate) fn press(&mut self, label: &str) {
        // Пока «считает», работает только отмена (C или Esc)
        if self.busy.is_some() || self.karaoke.is_some() {
            if label == "C" {
                self.reset(); // старый audio при этом уничтожается и звук замолкает
            }
            return;
        }
        // Пока идёт отсчёт, работает только отмена (C или Esc)
        if self.doom.is_some() {
            if label == "C" {
                run_shutdown(&["/a"]);
                self.reset();
            }
            return;
        }
        match label {
            "C" => self.reset(),
            "Del" => self.backspace(),
            "%" => self.percent(),
            "±" => self.negate(),
            "." => self.dot(),
            "+" => self.set_op(Op::Add),
            "-" => self.set_op(Op::Sub),
            "×" => self.set_op(Op::Mul),
            "÷" => self.set_op(Op::Div),
            "=" => self.equals(),
            d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() => self.digit(d),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_three_calculations_are_correct_then_use_the_existing_wrong_math() {
        let mut calculator = Calculator::default();

        assert_eq!(calculator.calc(Op::Add, 5.0, 5.0), 10.0);
        assert_eq!(calculator.calc(Op::Sub, 9.0, 2.0), 7.0);
        assert_eq!(calculator.calc(Op::Mul, 9.0, 2.0), 18.0);

        let fourth_result = calculator.calc(Op::Add, 5.0, 5.0);
        assert!((11.0..=13.0).contains(&fourth_result));
    }

    #[test]
    fn busy_mode_is_only_used_for_multi_digit_numbers() {
        assert!(!Calculator::needs_busy_mode(5.0, 5.0));
        assert!(!Calculator::needs_busy_mode(9.0, 2.0));
        assert!(Calculator::needs_busy_mode(10.0, 10.0));
        assert!(Calculator::needs_busy_mode(100.0, 1.0));
    }

    #[test]
    fn molly_mode_only_triggers_for_addition_resulting_in_2017() {
        assert!(Calculator::is_molly_sum(Op::Add, 2000.0, 17.0));
        assert!(Calculator::is_molly_sum(Op::Add, 2017.0, 0.0));
        assert!(!Calculator::is_molly_sum(Op::Sub, 2018.0, 1.0));
        assert!(!Calculator::is_molly_sum(Op::Add, 2000.0, 18.0));
    }

    #[test]
    fn lrc_timestamps_are_parsed_into_timed_lines() {
        let lyrics = parse_lyrics("[00:36.25] line one\n[00:37.53] line two");

        assert_eq!(lyrics.len(), 2);
        assert_eq!(lyrics[0].at, std::time::Duration::from_millis(36_250));
        assert_eq!(lyrics[1].text, " line two");
    }

    #[test]
    fn supermarket_midi_is_synthesized_into_audio() {
        let samples = synthesize_midi(SUPERMARKET_MIDI).expect("valid supermarket MIDI");

        assert!(
            samples.len() > MIDI_SAMPLE_RATE as usize * 60,
            "MIDI duration was only {:.1} seconds",
            samples.len() as f64 / f64::from(MIDI_SAMPLE_RATE)
        );
        assert!(samples.iter().any(|sample| *sample != 0.0));
    }
}
