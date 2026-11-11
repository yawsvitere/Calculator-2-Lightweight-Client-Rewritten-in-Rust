use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke,
};
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
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn symbol(self) -> &'static str {
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
const SHUTDOWN_DELAY: u32 = 10;

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

const FORMULAS: [&str; 34] = [
    "int sin(x²) dx = ?",
    "sum 1/n² = π²/6",
    "lim x->0 sin(x)/x = 1",
    "e^(iπ) + 1 = 0",
    "d/dx x³ = 3x²",
    "det(A - λI) = 0",
    "x = (-b ± sqrt(D)) / 2a",
    "Γ(n) = (n-1)!",
    "sin²x + cos²x = 1",
    "ln(xy) = ln x + ln y",
    "a² + b² = c²",
    "inf - inf = ???",
    "0.999... = 1",
    "i² = -1",
    "F = ma  =>  a = F/m",
    "sum 2^(-n) = 1",
    "sqrt(2) = 1.41421356...",
    "π = 3.14159265...",
    "x² - 1 = (x-1)(x+1)",
    "1 + 2 + 3 + ... = -1/12",
    "A·B = |A||B| cos θ",
    "P(A|B) = P(B|A)P(A)/P(B)",
    "2 + 2 = 5 (почти)",
    "переношу единицу...",
    "считаю в столбик...",
    "сверяюсь с таблицей Брадиса",
    "пересчитываю заново...",
    "складываю в уме...",
    "делю на ноль... шучу",
    "int 1/x dx = ln|x| + C",
    "sum n = n(n+1)/2",
    "log2(8) = 3 (наверное)",
    "тут была теорема Ферма...",
    "e = 2.71828182...",
];

/// Псевдослучайное число из шага анимации (splitmix64)
fn hash(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

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

struct Busy {
    start: Instant,
    action: Action,
    result: f64,
}

struct LyricLine {
    at: std::time::Duration,
    text: String,
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

struct Karaoke {
    start: Instant,
    lines: Vec<LyricLine>,
    action: Action,
    result: f64,
}

// ---------- Состояние калькулятора ----------

struct Calculator {
    current: String,
    acc: Option<f64>,
    op: Option<Op>,
    expr: String,
    fresh: bool,
    rng: Rng,
    calculations: u64,
    doom: Option<Instant>, // когда запущено выключение
    busy: Option<Busy>,    // идёт «вычисление»
    karaoke: Option<Karaoke>,
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

    fn tick(&mut self, ctx: &egui::Context) {
        if self.karaoke.is_some() {
            if self
                .audio
                .as_ref()
                .map(|(_, sink)| sink.empty())
                .unwrap_or(true)
            {
                self.finish_karaoke();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            return;
        }
        let done = self
            .busy
            .as_ref()
            .map(|b| b.start.elapsed() >= std::time::Duration::from_secs(CALC_SECONDS));
        match done {
            Some(true) => self.finish_busy(),
            Some(false) => ctx.request_repaint_after(std::time::Duration::from_millis(50)),
            None => {}
        }
    }

    fn press(&mut self, label: &str) {
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

// ---------- Интерфейс в стиле macOS ----------

// (подпись, ширина в колонках). Кнопка «0» занимает две колонки, как в macOS.
const ROWS: [&[(&str, f32)]; 5] = [
    &[("C", 1.0), ("±", 1.0), ("%", 1.0), ("÷", 1.0)],
    &[("7", 1.0), ("8", 1.0), ("9", 1.0), ("×", 1.0)],
    &[("4", 1.0), ("5", 1.0), ("6", 1.0), ("-", 1.0)],
    &[("1", 1.0), ("2", 1.0), ("3", 1.0), ("+", 1.0)],
    &[("0", 2.0), (".", 1.0), ("=", 1.0)],
];

const ORANGE: Color32 = Color32::from_rgb(255, 159, 10);

/// Возвращает (заливка, цвет текста). Кнопки полностью непрозрачные.
fn button_colors(label: &str, active: bool) -> (Color32, Color32) {
    match label {
        "÷" | "×" | "-" | "+" => {
            if active {
                (Color32::WHITE, ORANGE) // выбранная операция подсвечивается
            } else {
                (ORANGE, Color32::WHITE)
            }
        }
        "=" => (ORANGE, Color32::WHITE),
        "C" | "±" | "%" => (Color32::from_rgb(104, 104, 108), Color32::WHITE),
        _ => (Color32::from_rgb(78, 78, 82), Color32::WHITE),
    }
}

fn char_to_label(c: char) -> Option<&'static str> {
    Some(match c {
        '0' => "0",
        '1' => "1",
        '2' => "2",
        '3' => "3",
        '4' => "4",
        '5' => "5",
        '6' => "6",
        '7' => "7",
        '8' => "8",
        '9' => "9",
        '.' | ',' => ".",
        '+' => "+",
        '-' => "-",
        '*' | 'x' | 'X' => "×",
        '/' => "÷",
        '%' => "%",
        '=' => "=",
        _ => return None,
    })
}

impl eframe::App for Calculator {
    // Само окно прозрачное: прозрачность задаём только фону панели ниже
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);

        // Ввод с клавиатуры
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    for c in t.chars() {
                        if let Some(label) = char_to_label(c) {
                            self.press(label);
                        }
                    }
                }
                egui::Event::Key {
                    key, pressed: true, ..
                } => match key {
                    egui::Key::Enter => self.press("="),
                    egui::Key::Backspace => self.press("Del"),
                    egui::Key::Escape => self.press("C"),
                    _ => {}
                },
                _ => {}
            }
        }

        // Пока идёт отсчёт, перерисовываем окно, чтобы секунды менялись
        if self.doom.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Единственная полупрозрачная часть: фон окна
        let background = egui::Frame::default()
            .fill(Color32::from_rgba_unmultiplied(28, 28, 30, 170))
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(40)))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(Margin::same(14));

        egui::CentralPanel::default()
            .frame(background)
            .show(ctx, |ui| {
                // «Светофор» macOS: закрыть, свернуть, (зелёная — просто для вида)
                let (bar, bar_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 24.0),
                    Sense::click_and_drag(),
                );
                if bar_resp.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                let colors = [
                    Color32::from_rgb(255, 95, 87),
                    Color32::from_rgb(254, 188, 46),
                    Color32::from_rgb(40, 200, 64),
                ];
                let glyphs = ["x", "-", "+"];
                let mut hovered_any = false;
                let mut responses = Vec::new();
                for i in 0..3 {
                    let center = egui::pos2(bar.min.x + 6.0 + i as f32 * 20.0, bar.center().y);
                    let rect = egui::Rect::from_center_size(center, egui::vec2(16.0, 16.0));
                    let resp = ui.interact(rect, ui.id().with(("traffic", i)), Sense::click());
                    hovered_any |= resp.hovered();
                    responses.push((center, resp));
                }
                for (i, (center, _)) in responses.iter().enumerate() {
                    ui.painter().circle_filled(*center, 6.0, colors[i]);
                    if hovered_any {
                        ui.painter().text(
                            *center,
                            Align2::CENTER_CENTER,
                            glyphs[i],
                            FontId::proportional(11.0),
                            Color32::from_black_alpha(170),
                        );
                    }
                }
                if responses[0].1.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if responses[1].1.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                // Дисплей: просто крупные цифры справа, без рамки
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    let doom_left = self
                        .doom
                        .map(|t| (SHUTDOWN_DELAY as f32 - t.elapsed().as_secs_f32()).max(0.0));
                    let busy_ms = self
                        .busy
                        .as_ref()
                        .map(|b| b.start.elapsed().as_millis() as u64);
                    let karaoke_active = self.karaoke.is_some();
                    let expr = if doom_left.is_some() {
                        "Делить на ноль нельзя! C — отмена".to_string()
                    } else if karaoke_active {
                        " ".to_string()
                    } else if self.expr.is_empty() {
                        " ".to_string()
                    } else {
                        self.expr.clone()
                    };
                    ui.label(
                        RichText::new(expr)
                            .size(15.0)
                            .color(Color32::from_white_alpha(140)),
                    );

                    let text = match doom_left {
                        Some(s) => format!("{}", s.ceil() as u32),
                        None => match busy_ms {
                            Some(ms) => FORMULAS[(hash(ms / 260) % FORMULAS.len() as u64) as usize]
                                .to_string(),
                            None => match self.karaoke.as_ref() {
                                Some(karaoke) => karaoke
                                    .lines
                                    .iter()
                                    .rev()
                                    .find(|line| line.at <= karaoke.start.elapsed())
                                    .map(|line| line.text.clone())
                                    .unwrap_or_default(),
                                None => self.current.clone(),
                            },
                        },
                    };
                    let size = match text.chars().count() {
                        0..=7 => 58.0,
                        8..=16 => 38.0,
                        17..=24 => 26.0,
                        25..=36 => 18.0,
                        _ => 14.0,
                    };
                    ui.label(RichText::new(text).size(size).color(Color32::WHITE));
                });

                ui.add_space(8.0);

                // Круглые непрозрачные кнопки
                let spacing = 6.0;
                ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                let w = (ui.available_width() - spacing * 3.0) / 4.0;
                let h = ((ui.available_height() - spacing * 4.0) / 5.0).clamp(36.0, 80.0);
                let radius = (h / 2.0) as u8;
                let mut pressed: Option<&'static str> = None;

                for row in ROWS {
                    ui.horizontal(|ui| {
                        for &(label, span) in row {
                            let bw = w * span + spacing * (span - 1.0);
                            let active = self.fresh && self.op.map(|o| o.symbol()) == Some(label);
                            let (fill, text_color) = button_colors(label, active);
                            let btn = egui::Button::new(
                                RichText::new(label).size(24.0).color(text_color),
                            )
                            .fill(fill)
                            .stroke(Stroke::NONE)
                            .corner_radius(CornerRadius::same(radius));
                            if ui.add_sized([bw, h], btn).clicked() {
                                pressed = Some(label);
                            }
                        }
                    });
                }

                if let Some(label) = pressed {
                    self.press(label);
                }
            });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 460.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Calculator",
        options,
        Box::new(|_cc| Ok(Box::new(Calculator::default()))),
    )
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
