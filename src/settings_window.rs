//! Второе окно: профиль, аватар, темы, VPN (демо) и прочие настройки.

use crate::config::{avatar_path, config_dir, Settings, Theme, PLANS, SERVERS, THEMES};
use crate::fx::{self, cr, rgb};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Id, LayerId, Margin, Order, Pos2, Rect, RichText,
    Sense, Shape, Stroke, StrokeKind, Ui,
};
use std::path::Path;
use std::time::{Duration, Instant};

/// Всё, что относится к профилю и настройкам. Живёт в приложении, пока оно запущено.
pub(crate) struct State {
    pub cfg: Settings,
    saved: Settings,
    pub open: bool,
    tab: usize,
    pub avatar: Option<egui::TextureHandle>,
    avatar_checked: bool,
    pub vpn_on: bool,
    vpn_since: Option<Instant>,
    note: Option<(String, f64)>,
}

impl State {
    pub fn new() -> Self {
        let cfg = Settings::load();
        Self {
            saved: cfg.clone(),
            cfg,
            open: false,
            tab: 0,
            avatar: None,
            avatar_checked: false,
            vpn_on: false,
            vpn_since: None,
            note: None,
        }
    }

    /// Один раз подгружает сохранённый аватар.
    pub fn ensure_avatar(&mut self, ctx: &egui::Context) {
        if self.avatar_checked {
            return;
        }
        self.avatar_checked = true;
        let p = avatar_path();
        if p.exists() {
            self.avatar = load_texture(ctx, &p);
        }
    }

    /// Сохраняет настройки, если они изменились (не во время перетаскивания слайдера).
    pub fn persist(&mut self, ctx: &egui::Context) {
        if self.cfg != self.saved && !ctx.input(|i| i.pointer.any_down()) {
            self.cfg.save();
            self.saved = self.cfg.clone();
        }
    }

    fn toast(&mut self, msg: &str, now: f64) {
        self.note = Some((msg.to_string(), now));
    }

    fn pick_avatar(&mut self, ctx: &egui::Context, now: f64) {
        let file = rfd::FileDialog::new()
            .set_title("Выберите картинку для аватара")
            .add_filter("Изображения", &["png", "jpg", "jpeg"])
            .pick_file();
        if let Some(path) = file {
            self.set_avatar(ctx, &path, now);
        }
    }

    fn set_avatar(&mut self, ctx: &egui::Context, src: &Path, now: f64) {
        match process_avatar(src) {
            Ok(()) => {
                self.avatar = load_texture(ctx, &avatar_path());
                self.toast("Аватар обновлён", now);
            }
            Err(e) => self.toast(&e, now),
        }
    }

    fn remove_avatar(&mut self, now: f64) {
        let _ = std::fs::remove_file(avatar_path());
        self.avatar = None;
        self.toast("Аватар удалён", now);
    }
}

fn process_avatar(src: &Path) -> Result<(), String> {
    let img = image::open(src).map_err(|e| format!("Не удалось открыть картинку: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    if side == 0 {
        return Err("Пустая картинка".to_string());
    }
    // квадратная обрезка по центру и уменьшение до 256x256
    let sq = img
        .crop_imm((w - side) / 2, (h - side) / 2, side, side)
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    sq.save(avatar_path()).map_err(|e| e.to_string())
}

fn load_texture(ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("avatar", ci, egui::TextureOptions::LINEAR))
}

// ---------- Окно ----------

pub(crate) fn show(ctx: &egui::Context, st: &mut State) {
    let id = egui::ViewportId::from_hash_of("settings_window");
    let builder = egui::ViewportBuilder::default()
        .with_title("Профиль и настройки")
        .with_inner_size([480.0, 680.0])
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false);
    ctx.show_viewport_immediate(id, builder, |ctx, _class| {
        draw(ctx, st);
        if ctx.input(|i| i.viewport().close_requested()) {
            st.open = false;
        }
    });
}

#[derive(Clone, Copy)]
struct Pal {
    th: Theme,
    accent: Color32,
    on_acc: Color32,
    text: Color32,
    dim: Color32,
    now: f64,
}

fn draw(ctx: &egui::Context, st: &mut State) {
    let th = st.cfg.theme();
    let text = rgb(th.text);
    let pal = Pal {
        th,
        accent: st.cfg.accent_color(),
        on_acc: st.cfg.on_accent(),
        text,
        dim: text.gamma_multiply(0.6),
        now: ctx.input(|i| i.time),
    };
    let now = pal.now;

    // Перетаскивание картинки в окно = новый аватар
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    for f in dropped {
        if let Some(path) = f.path {
            st.set_avatar(ctx, &path, now);
        }
    }

    if st.cfg.animated_bg {
        ctx.request_repaint_after(Duration::from_millis(33));
    } else if st.vpn_on || st.note.is_some() {
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    let frame = egui::Frame::default().inner_margin(Margin::same(18));
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        let full = ui.max_rect().expand(18.0);
        let bg = ctx.layer_painter(LayerId::background());
        fx::window_bg(&bg, full, 20.0, &th, st.cfg.glass.max(0.85), st.cfg.animated_bg, now);

        ui.visuals_mut().override_text_color = Some(text);
        ui.visuals_mut().extreme_bg_color =
            Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 22);
        ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
        let w = ui.available_width();

        // --- Заголовок окна ---
        let (bar, bar_resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click_and_drag());
        if bar_resp.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let close = Rect::from_center_size(pos2(bar.left() + 8.0, bar.center().y), vec2(16.0, 16.0));
        let close_resp = ui.interact(close, Id::new("settings_close"), Sense::click());
        ui.painter()
            .circle_filled(close.center(), 6.0, Color32::from_rgb(255, 95, 87));
        if close_resp.hovered() {
            ui.painter().text(
                close.center(),
                Align2::CENTER_CENTER,
                "x",
                FontId::proportional(11.0),
                Color32::from_black_alpha(170),
            );
        }
        if close_resp.clicked() {
            st.open = false;
        }
        ui.painter().text(
            bar.center(),
            Align2::CENTER_CENTER,
            "Профиль и настройки",
            FontId::proportional(14.0),
            pal.dim,
        );

        // --- Шапка профиля ---
        let (hdr, _) = ui.allocate_exact_size(vec2(w, 96.0), Sense::hover());
        fx::card(ui.painter(), hdr, 18.0, text);
        let ac = pos2(hdr.left() + 52.0, hdr.center().y);
        let aresp = ui.interact(
            Rect::from_center_size(ac, vec2(76.0, 76.0)),
            Id::new("hdr_avatar"),
            Sense::click(),
        );
        let hv = ctx.animate_bool_with_time(Id::new("hdr_avatar_h"), aresp.hovered(), 0.12);
        fx::avatar(
            ui.painter(),
            ac,
            34.0 + 2.0 * hv,
            st.avatar.as_ref(),
            &st.cfg.name,
            pal.accent,
            pal.on_acc,
        );
        if hv > 0.01 {
            ui.painter()
                .circle_filled(ac, 34.0, Color32::from_black_alpha((140.0 * hv) as u8));
            ui.painter().text(
                ac,
                Align2::CENTER_CENTER,
                "Изменить",
                FontId::proportional(12.0),
                Color32::from_white_alpha((255.0 * hv) as u8),
            );
        }
        if aresp.clicked() {
            st.pick_avatar(ctx, now);
        }

        let name = if st.cfg.name.trim().is_empty() {
            "Гость".to_string()
        } else {
            st.cfg.name.trim().to_string()
        };
        ui.painter().text(
            pos2(hdr.left() + 104.0, hdr.center().y - 14.0),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(22.0),
            text,
        );
        let plan_label = if st.cfg.plan == 0 {
            "FREE".to_string()
        } else {
            format!("PRO · {}", PLANS[st.cfg.plan as usize - 1].0)
        };
        let badge = Rect::from_min_size(
            pos2(hdr.left() + 104.0, hdr.center().y + 4.0),
            vec2(24.0 + plan_label.chars().count() as f32 * 7.5, 22.0),
        );
        let pro = st.cfg.plan > 0;
        ui.painter().rect_filled(
            badge,
            cr(11.0),
            if pro {
                pal.accent
            } else {
                Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 30)
            },
        );
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            plan_label,
            FontId::proportional(12.0),
            if pro { pal.on_acc } else { pal.dim },
        );

        // --- Вкладки ---
        let (tabs, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
        let mut tab = st.tab as u8;
        fx::segmented(
            ui,
            tabs,
            Id::new("settings_tabs"),
            &mut tab,
            &["Профиль", "Оформление", "VPN", "Ещё"],
            pal.accent,
            pal.on_acc,
            text,
        );
        st.tab = tab as usize;

        // --- Содержимое вкладки ---
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                match st.tab {
                    0 => tab_profile(ui, ctx, st, &pal),
                    1 => tab_look(ui, st, &pal),
                    2 => tab_vpn(ui, ctx, st, &pal),
                    _ => tab_more(ui, st, &pal),
                }
                ui.add_space(12.0);
            });

        // --- Всплывающее уведомление ---
        if let Some((msg, t0)) = st.note.clone() {
            if now - t0 < 2.6 {
                let tp = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("toast")));
                let r = Rect::from_center_size(
                    pos2(full.center().x, full.bottom() - 36.0),
                    vec2(msg.chars().count() as f32 * 7.6 + 40.0, 34.0),
                );
                tp.rect_filled(r, cr(17.0), Color32::from_rgba_unmultiplied(20, 20, 24, 235));
                tp.rect_stroke(
                    r,
                    cr(17.0),
                    Stroke::new(1.0, Color32::from_white_alpha(40)),
                    StrokeKind::Inside,
                );
                tp.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    msg,
                    FontId::proportional(13.0),
                    Color32::WHITE,
                );
            } else {
                st.note = None;
            }
        }
    });
}

// ---------- Общие куски ----------

fn section(ui: &mut Ui, p: &Pal, title: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title.to_uppercase()).size(11.5).color(p.dim));
}

/// Строка настройки: заголовок слева, подпись под ним. Элемент управления рисуется поверх справа.
fn row(ui: &mut Ui, p: &Pal, title: &str, sub: &str) -> Rect {
    let h = if sub.is_empty() { 44.0 } else { 54.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter();
    let ty = if sub.is_empty() {
        rect.center().y
    } else {
        rect.center().y - 9.0
    };
    painter.text(
        pos2(rect.left() + 4.0, ty),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        p.text,
    );
    if !sub.is_empty() {
        painter.text(
            pos2(rect.left() + 4.0, rect.center().y + 10.0),
            Align2::LEFT_CENTER,
            sub,
            FontId::proportional(12.0),
            p.dim,
        );
    }
    painter.line_segment(
        [pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())],
        Stroke::new(1.0, p.text.gamma_multiply(0.1)),
    );
    rect
}

fn toggle_row(ui: &mut Ui, p: &Pal, title: &str, sub: &str, val: &mut bool) {
    let r = row(ui, p, title, sub);
    let sw = Rect::from_center_size(pos2(r.right() - 26.0, r.center().y), vec2(46.0, 27.0));
    fx::switch(ui, sw, Id::new(("sw", title)), val, p.accent, p.text.gamma_multiply(0.22));
}

fn neutral(p: &Pal) -> Color32 {
    Color32::from_rgba_unmultiplied(p.text.r(), p.text.g(), p.text.b(), 34)
}

// ---------- Вкладка «Профиль» ----------

fn tab_profile(ui: &mut Ui, ctx: &egui::Context, st: &mut State, p: &Pal) {
    section(ui, p, "Аватар");
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let b1 = Rect::from_min_size(r.min + vec2(0.0, 2.0), vec2(170.0, 34.0));
    let b2 = Rect::from_min_size(r.min + vec2(180.0, 2.0), vec2(110.0, 34.0));
    if fx::pill_button(ui, b1, Id::new("av_upload"), "Загрузить фото", p.accent, p.on_acc) {
        st.pick_avatar(ctx, p.now);
    }
    if fx::pill_button(ui, b2, Id::new("av_remove"), "Удалить", neutral(p), p.text) {
        st.remove_avatar(p.now);
    }
    ui.label(
        RichText::new("Или перетащите PNG / JPG прямо в это окно. Фото обрезается по центру в квадрат.")
            .size(12.0)
            .color(p.dim),
    );

    section(ui, p, "Имя");
    let (nr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.put(
        nr,
        egui::TextEdit::singleline(&mut st.cfg.name)
            .char_limit(18)
            .hint_text("Ваше имя")
            .desired_width(nr.width()),
    );

    section(ui, p, "Подписка");
    let r = row(ui, p, "Тариф", "");
    let label = if st.cfg.plan == 0 {
        "Бесплатный".to_string()
    } else {
        format!("PRO · {}", PLANS[st.cfg.plan as usize - 1].0)
    };
    ui.painter().text(
        pos2(r.right() - 130.0, r.center().y),
        Align2::RIGHT_CENTER,
        label,
        FontId::proportional(14.0),
        p.dim,
    );
    let btn = Rect::from_center_size(pos2(r.right() - 55.0, r.center().y), vec2(110.0, 30.0));
    if fx::pill_button(ui, btn, Id::new("goto_vpn"), "Управлять", neutral(p), p.text) {
        st.tab = 2;
    }
    let r = row(ui, p, "VPN", "");
    ui.painter().text(
        pos2(r.right() - 4.0, r.center().y),
        Align2::RIGHT_CENTER,
        if st.vpn_on { "Подключён" } else { "Отключён" },
        FontId::proportional(14.0),
        if st.vpn_on { p.accent } else { p.dim },
    );
}

// ---------- Вкладка «Оформление» ----------

fn tab_look(ui: &mut Ui, st: &mut State, p: &Pal) {
    section(ui, p, "Тема");
    let cw = ui.available_width();
    let gap = 8.0;
    let tw = (cw - gap * 2.0) / 3.0;
    let thh = 74.0;
    let rows = THEMES.len().div_ceil(3);
    let (grid, _) = ui.allocate_exact_size(
        vec2(cw, rows as f32 * thh + (rows as f32 - 1.0) * gap),
        Sense::hover(),
    );
    for (i, t) in THEMES.iter().enumerate() {
        let rect = Rect::from_min_size(
            grid.min + vec2((i % 3) as f32 * (tw + gap), (i / 3) as f32 * (thh + gap)),
            vec2(tw, thh),
        );
        let resp = ui.interact(rect, Id::new(("theme", i)), Sense::click());
        let hv = ui
            .ctx()
            .animate_bool_with_time(Id::new(("theme_h", i)), resp.hovered(), 0.12);
        let pt = ui.painter();
        fx::gradient_rounded(pt, rect, 14.0, rgb(t.bg_top), rgb(t.bg_bottom));
        let [a, b] = [t.blob_a, t.blob_b];
        pt.circle_filled(
            rect.left_top() + vec2(tw * 0.78, 20.0),
            17.0,
            Color32::from_rgba_unmultiplied(a[0], a[1], a[2], 150),
        );
        pt.circle_filled(
            rect.left_top() + vec2(tw * 0.34, 34.0),
            14.0,
            Color32::from_rgba_unmultiplied(b[0], b[1], b[2], 130),
        );
        pt.circle_filled(pos2(rect.left() + 14.0, rect.bottom() - 15.0), 5.0, rgb(t.accent));
        pt.text(
            pos2(rect.left() + 26.0, rect.bottom() - 15.0),
            Align2::LEFT_CENTER,
            t.name,
            FontId::proportional(12.5),
            rgb(t.text),
        );
        let selected = st.cfg.theme == i;
        if selected {
            pt.rect_stroke(rect, cr(14.0), Stroke::new(2.5, p.accent), StrokeKind::Outside);
        } else if hv > 0.01 {
            pt.rect_stroke(
                rect,
                cr(14.0),
                Stroke::new(1.5, Color32::from_white_alpha((120.0 * hv) as u8)),
                StrokeKind::Outside,
            );
        }
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            st.cfg.theme = i;
        }
    }

    section(ui, p, "Акцентный цвет");
    toggle_row(ui, p, "Свой цвет", "Перекрашивает кнопки и подсветку", &mut st.cfg.custom_accent);
    if st.cfg.custom_accent {
        let r = row(ui, p, "Оттенок", "");
        let s = Rect::from_min_size(pos2(r.right() - 250.0, r.center().y - 14.0), vec2(250.0, 28.0));
        fx::hue_slider(ui, s, Id::new("hue"), &mut st.cfg.hue);
    }

    section(ui, p, "Стекло и форма");
    let r = row(ui, p, "Прозрачность окна", "");
    ui.painter().text(
        pos2(r.right() - 205.0, r.center().y),
        Align2::RIGHT_CENTER,
        format!("{}%", (st.cfg.glass * 100.0).round() as u32),
        FontId::proportional(13.0),
        p.dim,
    );
    let s = Rect::from_min_size(pos2(r.right() - 195.0, r.center().y - 12.0), vec2(195.0, 24.0));
    fx::slider(ui, s, Id::new("glass"), &mut st.cfg.glass, 0.35, 1.0, p.accent, p.text.gamma_multiply(0.2));

    let r = row(ui, p, "Форма кнопок", "");
    let s = Rect::from_min_size(pos2(r.right() - 250.0, r.center().y - 16.0), vec2(250.0, 32.0));
    fx::segmented(
        ui,
        s,
        Id::new("shape"),
        &mut st.cfg.shape,
        &["Круглые", "Мягкие", "Острые"],
        p.accent,
        p.on_acc,
        p.text,
    );
}

// ---------- Вкладка «VPN» ----------

fn fmt_clock(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn tab_vpn(ui: &mut Ui, ctx: &egui::Context, st: &mut State, p: &Pal) {
    let cw = ui.available_width();
    let subscribed = st.cfg.plan > 0;

    // Карточка статуса с кнопкой питания
    let (card, _) = ui.allocate_exact_size(vec2(cw, 150.0), Sense::hover());
    fx::card(ui.painter(), card, 18.0, p.text);
    let c = pos2(card.left() + 70.0, card.center().y);
    let resp = ui.interact(
        Rect::from_center_size(c, vec2(84.0, 84.0)),
        Id::new("vpn_power"),
        Sense::click(),
    );
    let t = ctx.animate_bool_with_time(Id::new("vpn_on_anim"), st.vpn_on, 0.3);
    let pt = ui.painter();
    if st.vpn_on {
        let ph = (p.now * 0.8).fract() as f32;
        pt.circle_stroke(
            c,
            38.0 + ph * 22.0,
            Stroke::new(2.0, p.accent.gamma_multiply(1.0 - ph)),
        );
    }
    pt.circle_filled(
        c,
        36.0,
        Color32::from_rgba_unmultiplied(p.text.r(), p.text.g(), p.text.b(), 30)
            .lerp_to_gamma(p.accent, t),
    );
    let icon = p.text.lerp_to_gamma(p.on_acc, t);
    let arc: Vec<Pos2> = (0..=30)
        .map(|i| {
            let a = (-90.0_f32 + 40.0 + 280.0 * i as f32 / 30.0).to_radians();
            c + vec2(a.cos(), a.sin()) * 14.0
        })
        .collect();
    pt.add(Shape::line(arc, Stroke::new(3.0, icon)));
    pt.line_segment([c + vec2(0.0, -18.0), c + vec2(0.0, -4.0)], Stroke::new(3.0, icon));

    let (title, sub) = if st.vpn_on {
        let secs = st.vpn_since.map(|s| s.elapsed().as_secs()).unwrap_or(0);
        (
            "Защищено".to_string(),
            format!("{} · {}", SERVERS[st.cfg.vpn_server].1, fmt_clock(secs)),
        )
    } else if subscribed {
        ("Отключено".to_string(), "Нажмите на кнопку, чтобы подключиться".to_string())
    } else {
        ("Нужна подписка".to_string(), "Выберите тариф ниже".to_string())
    };
    pt.text(
        pos2(card.left() + 136.0, card.center().y - 12.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(22.0),
        if st.vpn_on { p.accent } else { p.text },
    );
    pt.text(
        pos2(card.left() + 136.0, card.center().y + 14.0),
        Align2::LEFT_CENTER,
        sub,
        FontId::proportional(13.0),
        p.dim,
    );
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        if subscribed {
            st.vpn_on = !st.vpn_on;
            st.vpn_since = if st.vpn_on { Some(Instant::now()) } else { None };
        } else {
            st.toast("Сначала оформите подписку", p.now);
        }
    }

    // Тарифы
    section(ui, p, "Тарифы");
    for (i, (name, price, note)) in PLANS.iter().enumerate() {
        let (r, _) = ui.allocate_exact_size(vec2(cw, 66.0), Sense::hover());
        let active = st.cfg.plan as usize == i + 1;
        fx::card(ui.painter(), r, 16.0, p.text);
        if active {
            ui.painter()
                .rect_stroke(r, cr(16.0), Stroke::new(2.0, p.accent), StrokeKind::Inside);
        }
        let pt = ui.painter();
        pt.text(
            pos2(r.left() + 16.0, r.center().y - 10.0),
            Align2::LEFT_CENTER,
            *name,
            FontId::proportional(17.0),
            p.text,
        );
        pt.text(
            pos2(r.left() + 16.0, r.center().y + 11.0),
            Align2::LEFT_CENTER,
            *note,
            FontId::proportional(12.0),
            if i == 1 { p.accent } else { p.dim },
        );
        let btn = Rect::from_center_size(pos2(r.right() - 62.0, r.center().y), vec2(104.0, 32.0));
        pt.text(
            pos2(btn.left() - 12.0, r.center().y),
            Align2::RIGHT_CENTER,
            *price,
            FontId::proportional(14.0),
            p.text,
        );
        let (label, fill, fg) = if active {
            ("Активна", neutral(p), p.dim)
        } else {
            ("Оформить", p.accent, p.on_acc)
        };
        if fx::pill_button(ui, btn, Id::new(("plan", i)), label, fill, fg) && !active {
            st.cfg.plan = i as u8 + 1;
            st.toast(&format!("Тариф «{name}» подключён (демо)"), p.now);
        }
    }
    if subscribed {
        let (r, _) = ui.allocate_exact_size(vec2(cw, 34.0), Sense::hover());
        let btn = Rect::from_min_size(r.min, vec2(190.0, 32.0));
        if fx::pill_button(ui, btn, Id::new("plan_cancel"), "Отменить подписку", neutral(p), p.text) {
            st.cfg.plan = 0;
            st.vpn_on = false;
            st.vpn_since = None;
            st.toast("Подписка отменена", p.now);
        }
    }

    // Серверы
    section(ui, p, "Сервер");
    for (i, (code, city, ping)) in SERVERS.iter().enumerate() {
        let (r, _) = ui.allocate_exact_size(vec2(cw, 46.0), Sense::hover());
        let resp = ui.interact(r, Id::new(("srv", i)), Sense::click());
        let hv = ctx.animate_bool_with_time(Id::new(("srv_h", i)), resp.hovered(), 0.12);
        let dim = if subscribed { 1.0 } else { 0.45 };
        let selected = st.cfg.vpn_server == i;
        let pt = ui.painter();
        pt.rect_filled(
            r,
            cr(14.0),
            Color32::from_rgba_unmultiplied(p.text.r(), p.text.g(), p.text.b(), (12.0 + 14.0 * hv) as u8),
        );
        if selected {
            pt.rect_stroke(r, cr(14.0), Stroke::new(1.5, p.accent.gamma_multiply(dim)), StrokeKind::Inside);
        }
        let chip = Rect::from_center_size(pos2(r.left() + 30.0, r.center().y), vec2(38.0, 24.0));
        pt.rect_filled(chip, cr(8.0), p.accent.gamma_multiply(0.25 * dim));
        pt.text(chip.center(), Align2::CENTER_CENTER, *code, FontId::proportional(12.0), p.text.gamma_multiply(dim));
        pt.text(
            pos2(r.left() + 62.0, r.center().y),
            Align2::LEFT_CENTER,
            *city,
            FontId::proportional(15.0),
            p.text.gamma_multiply(dim),
        );
        pt.text(
            pos2(r.right() - 14.0, r.center().y),
            Align2::RIGHT_CENTER,
            format!("{ping} мс"),
            FontId::proportional(13.0),
            p.dim.gamma_multiply(dim),
        );
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            if subscribed {
                st.cfg.vpn_server = i;
                if st.vpn_on {
                    st.vpn_since = Some(Instant::now());
                }
            } else {
                st.toast("Серверы доступны по подписке", p.now);
            }
        }
    }

}

// ---------- Вкладка «Ещё» ----------

fn tab_more(ui: &mut Ui, st: &mut State, p: &Pal) {
    section(ui, p, "Эффекты");
    toggle_row(ui, p, "Живой фон", "Плавающие цветные блики", &mut st.cfg.animated_bg);
    toggle_row(ui, p, "Свечение кнопок", "Подсветка при наведении", &mut st.cfg.glow);
    toggle_row(ui, p, "Волна при нажатии", "Круг расходится от кнопки", &mut st.cfg.ripple);

    section(ui, p, "Калькулятор");
    toggle_row(ui, p, "Показывать выражение", "Строка над результатом", &mut st.cfg.show_expr);
    toggle_row(ui, p, "Группировать разряды", "1 234 567 вместо 1234567", &mut st.cfg.group_digits);

    section(ui, p, "Окно");
    toggle_row(ui, p, "Поверх всех окон", "Калькулятор не прячется", &mut st.cfg.always_on_top);

    section(ui, p, "Сброс");
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let btn = Rect::from_min_size(r.min + vec2(0.0, 2.0), vec2(220.0, 34.0));
    if fx::pill_button(ui, btn, Id::new("reset_look"), "Сбросить оформление", neutral(p), p.text) {
        st.cfg.reset_look();
        st.toast("Оформление сброшено", p.now);
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("Калькулятор · считает как умеет")
            .size(12.0)
            .color(p.dim),
    );
}
