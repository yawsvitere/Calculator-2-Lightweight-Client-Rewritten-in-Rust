use crate::calculator::{Calculator, SHUTDOWN_DELAY};
use crate::config::Theme;
use crate::fx::{self, cr, rgb};
use crate::settings_window::{self, State};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Margin, RichText, Sense, Stroke,
    StrokeKind,
};
use std::time::Duration;

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

// (подпись, ширина в колонках). Кнопка «0» занимает две колонки, как в macOS.
const ROWS: [&[(&str, f32)]; 5] = [
    &[("C", 1.0), ("±", 1.0), ("%", 1.0), ("÷", 1.0)],
    &[("7", 1.0), ("8", 1.0), ("9", 1.0), ("×", 1.0)],
    &[("4", 1.0), ("5", 1.0), ("6", 1.0), ("-", 1.0)],
    &[("1", 1.0), ("2", 1.0), ("3", 1.0), ("+", 1.0)],
    &[("0", 2.0), (".", 1.0), ("=", 1.0)],
];

/// Возвращает (заливка, цвет текста) в зависимости от темы.
fn button_colors(label: &str, active: bool, th: &Theme, accent: Color32, on_acc: Color32) -> (Color32, Color32) {
    match label {
        "÷" | "×" | "-" | "+" => {
            if active {
                (rgb(th.text), accent) // выбранная операция подсвечивается
            } else {
                (accent, on_acc)
            }
        }
        "=" => (accent, on_acc),
        "C" | "±" | "%" => (rgb(th.func), rgb(th.text)),
        _ => (rgb(th.num), rgb(th.text)),
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

/// «1234567.5» -> «1 234 567.5». Всё, что не похоже на число, возвращается как есть.
fn group_digits(s: &str) -> String {
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return s.to_string();
    }
    let (int, frac) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let n = int.len();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// Кнопка калькулятора: наведение, нажатие, свечение и волна.
#[allow(clippy::too_many_arguments)]
fn calc_button(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    label: &str,
    fill: Color32,
    fg: Color32,
    radius: f32,
    glow: bool,
    ripple_age: Option<f32>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let ctx = ui.ctx().clone();
    let hover = ctx.animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let down = ctx.animate_bool_with_time(resp.id.with("d"), resp.is_pointer_button_down_on(), 0.06);

    let p = ui.painter();
    let r = rect.shrink(2.5 * down);
    let radius = radius.min(r.height() / 2.0);

    if glow && hover > 0.01 {
        for i in 1..=4 {
            let e = i as f32 * 2.5 * hover;
            p.rect_filled(
                r.expand(e),
                cr(radius + e),
                Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), (16.0 * hover) as u8),
            );
        }
    }

    let base = fill
        .lerp_to_gamma(Color32::WHITE, 0.12 * hover)
        .lerp_to_gamma(Color32::BLACK, 0.15 * down);
    p.rect_filled(r, cr(radius), base);
    p.rect_stroke(r, cr(radius), Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);

    if let Some(age) = ripple_age {
        if age < 0.5 {
            let t = age / 0.5;
            let ease = 1.0 - (1.0 - t) * (1.0 - t);
            p.circle_filled(
                r.center(),
                r.height() / 2.0 * (0.25 + 0.75 * ease),
                Color32::from_white_alpha((80.0 * (1.0 - t)) as u8),
            );
        }
    }

    p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(24.0), fg);
    resp
}

pub(crate) struct App {
    calc: Calculator,
    st: State,
    ripple: Option<(&'static str, f64)>,
    applied_top: Option<bool>,
}

impl App {
    fn new() -> Self {
        Self {
            calc: Calculator::default(),
            st: State::new(),
            ripple: None,
            applied_top: None,
        }
    }

    fn press(&mut self, label: &'static str, now: f64) {
        self.ripple = Some((label, now));
        self.calc.press(label);
    }
}

impl eframe::App for App {
    // Само окно прозрачное: фон рисуем сами
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.st.ensure_avatar(ctx);

        if self.calc.tick() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        // «Поверх всех окон»
        if self.applied_top != Some(self.st.cfg.always_on_top) {
            self.applied_top = Some(self.st.cfg.always_on_top);
            let level = if self.st.cfg.always_on_top {
                egui::viewport::WindowLevel::AlwaysOnTop
            } else {
                egui::viewport::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        }

        // Ввод с клавиатуры
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    for c in t.chars() {
                        if let Some(label) = char_to_label(c) {
                            self.press(label, now);
                        }
                    }
                }
                egui::Event::Key {
                    key, pressed: true, ..
                } => match key {
                    egui::Key::Enter => self.press("=", now),
                    egui::Key::Backspace => self.press("Del", now),
                    egui::Key::Escape => self.press("C", now),
                    _ => {}
                },
                _ => {}
            }
        }

        // Пока идёт отсчёт, перерисовываем окно, чтобы секунды менялись
        if self.calc.doom.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.st.cfg.animated_bg {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if let Some((_, t0)) = self.ripple {
            if now - t0 < 0.55 {
                ctx.request_repaint();
            } else {
                self.ripple = None;
            }
        }

        // Тема и цвета
        let th = self.st.cfg.theme();
        let accent = self.st.cfg.accent_color();
        let on_acc = self.st.cfg.on_accent();
        let text = rgb(th.text);
        let dim = text.gamma_multiply(0.55);

        let frame = egui::Frame::default().inner_margin(Margin::same(14));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            let full = ui.max_rect().expand(14.0);
            fx::window_bg(
                &ctx.layer_painter(egui::LayerId::background()),
                full,
                20.0,
                &th,
                self.st.cfg.glass,
                self.st.cfg.animated_bg,
                now,
            );

            // ---------- Верхняя полоса: «светофор» слева, профиль справа ----------
            let (bar, bar_resp) = ui.allocate_exact_size(
                vec2(ui.available_width(), 32.0),
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
                let center = pos2(bar.min.x + 6.0 + i as f32 * 20.0, bar.center().y);
                let rect = egui::Rect::from_center_size(center, vec2(16.0, 16.0));
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

            // Профиль в правом верхнем углу
            let pc = pos2(bar.max.x - 16.0, bar.center().y);
            let pr = ui.interact(
                egui::Rect::from_center_size(pc, vec2(36.0, 36.0)),
                ui.id().with("profile"),
                Sense::click(),
            );
            let hv = ctx.animate_bool_with_time(pr.id.with("hv"), pr.hovered(), 0.12);
            let name = {
                let n = self.st.cfg.name.trim();
                if n.chars().count() > 12 {
                    format!("{}..", n.chars().take(11).collect::<String>())
                } else {
                    n.to_string()
                }
            };
            ui.painter().text(
                pos2(pc.x - 26.0, pc.y),
                Align2::RIGHT_CENTER,
                &name,
                FontId::proportional(13.0),
                text.gamma_multiply(0.5 + 0.4 * hv),
            );
            fx::avatar(
                ui.painter(),
                pc,
                13.0 + 1.5 * hv,
                self.st.avatar.as_ref(),
                &self.st.cfg.name,
                accent,
                on_acc,
            );
            if self.st.vpn_on {
                let dot = pc + vec2(10.0, 10.0);
                ui.painter().circle_filled(dot, 5.5, Color32::from_black_alpha(160));
                ui.painter().circle_filled(dot, 4.0, Color32::from_rgb(52, 211, 105));
            }
            if pr.clicked() {
                self.st.open = true;
            }
            let _ = pr.on_hover_text("Профиль и настройки");

            // ---------- Дисплей ----------
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                let doom_left = self
                    .calc
                    .doom
                    .map(|t| (SHUTDOWN_DELAY as f32 - t.elapsed().as_secs_f32()).max(0.0));
                let busy_ms = self
                    .calc
                    .busy
                    .as_ref()
                    .map(|b| b.start.elapsed().as_millis() as u64);
                let karaoke_active = self.calc.karaoke.is_some();
                let expr = if doom_left.is_some() {
                    "Делить на ноль нельзя! C — отмена".to_string()
                } else if karaoke_active || self.calc.expr.is_empty() || !self.st.cfg.show_expr {
                    " ".to_string()
                } else {
                    self.calc.expr.clone()
                };
                ui.label(RichText::new(expr).size(15.0).color(dim));

                let text_str = match doom_left {
                    Some(s) => format!("{}", s.ceil() as u32),
                    None => match busy_ms {
                        Some(ms) => FORMULAS[(hash(ms / 260) % FORMULAS.len() as u64) as usize]
                            .to_string(),
                        None => match self.calc.karaoke.as_ref() {
                            Some(karaoke) => karaoke
                                .lines
                                .iter()
                                .rev()
                                .find(|line| line.at <= karaoke.start.elapsed())
                                .map(|line| line.text.clone())
                                .unwrap_or_default(),
                            None => {
                                if self.st.cfg.group_digits {
                                    group_digits(&self.calc.current)
                                } else {
                                    self.calc.current.clone()
                                }
                            }
                        },
                    },
                };
                let size = match text_str.chars().count() {
                    0..=7 => 58.0,
                    8..=16 => 38.0,
                    17..=24 => 26.0,
                    25..=36 => 18.0,
                    _ => 14.0,
                };
                ui.label(RichText::new(text_str).size(size).color(text));
            });

            ui.add_space(8.0);

            // ---------- Кнопки ----------
            let spacing = 6.0;
            ui.spacing_mut().item_spacing = vec2(spacing, spacing);
            let w = (ui.available_width() - spacing * 3.0) / 4.0;
            let h = ((ui.available_height() - spacing * 4.0) / 5.0).clamp(36.0, 80.0);
            let radius = match self.st.cfg.shape {
                0 => h / 2.0,
                1 => h * 0.3,
                _ => 8.0,
            };
            let glow = self.st.cfg.glow;
            let ripple_on = self.st.cfg.ripple;
            let mut pressed: Option<&'static str> = None;

            for row in ROWS {
                ui.horizontal(|ui| {
                    for &(label, span) in row {
                        let bw = w * span + spacing * (span - 1.0);
                        let active = self.calc.fresh
                            && self.calc.op.map(|o| o.symbol()) == Some(label);
                        let (fill, fg) = button_colors(label, active, &th, accent, on_acc);
                        let age = match self.ripple {
                            Some((l, t)) if ripple_on && l == label => Some((now - t) as f32),
                            _ => None,
                        };
                        if calc_button(ui, vec2(bw, h), label, fill, fg, radius, glow, age).clicked() {
                            pressed = Some(label);
                        }
                    }
                });
            }

            if let Some(label) = pressed {
                self.press(label, now);
            }
        });

        // Окно профиля и настроек
        if self.st.open {
            settings_window::show(ctx, &mut self.st);
        }
        self.st.persist(ctx);
    }
}

pub(crate) fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 540.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Calculator",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}