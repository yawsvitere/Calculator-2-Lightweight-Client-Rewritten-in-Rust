//! Помощники для рисования: градиентный фон с «живыми» бликами, аватар в круге,
//! и свои виджеты (переключатель, слайдеры, сегмент-контрол, кнопка-таблетка).

use crate::config::Theme;
use eframe::egui::{
    self, ecolor::Hsva, epaint::Vertex, pos2, vec2, Align2, Color32, CornerRadius, FontId, Id,
    Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui,
};

pub(crate) fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub(crate) fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.clamp(0.0, 255.0) as u8)
}

fn rgba(c: [u8; 3], a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

// ---------- Фон окна ----------

/// Точки контура скруглённого прямоугольника (по часовой стрелке).
pub(crate) fn rounded_points(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.5);
    let n = 10;
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -90.0_f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    let mut pts = Vec::with_capacity(4 * (n + 1));
    for (c, start) in corners {
        for i in 0..=n {
            let a = (start + 90.0 * i as f32 / n as f32).to_radians();
            pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    pts
}

/// Скруглённый прямоугольник с вертикальным градиентом.
pub(crate) fn gradient_rounded(painter: &Painter, rect: Rect, r: f32, top: Color32, bottom: Color32) {
    let pts = rounded_points(rect, r);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), top.lerp_to_gamma(bottom, 0.5));
    for p in &pts {
        let t = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        mesh.colored_vertex(*p, top.lerp_to_gamma(bottom, t));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

/// Мягкое светящееся пятно (набор колец с малой прозрачностью).
fn blob(painter: &Painter, center: Pos2, radius: f32, color: [u8; 3], alpha: u8) {
    let steps = 12;
    let a = (alpha as f32 / steps as f32).max(1.0) as u8;
    for k in 0..steps {
        let t = k as f32 / steps as f32;
        painter.circle_filled(center, radius * (1.0 - t * 0.92), rgba(color, a));
    }
}

/// Фон окна: градиент + три медленно плавающих цветных пятна + тонкая рамка.
pub(crate) fn window_bg(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    th: &Theme,
    glass: f32,
    animated: bool,
    time: f64,
) {
    let a = (glass.clamp(0.0, 1.0) * 255.0) as u8;
    gradient_rounded(painter, rect, radius, rgba(th.bg_top, a), rgba(th.bg_bottom, a));

    let clip = rect.shrink(radius * 0.3 + 1.0);
    let bp = painter.with_clip_rect(clip);
    let t = if animated { time as f32 } else { 0.0 };
    let (w, h) = (rect.width(), rect.height());
    let at = |fx: f32, fy: f32| pos2(rect.left() + w * fx, rect.top() + h * fy);
    let strength = 0.55 + 0.45 * glass;

    let c1 = at(0.22 + 0.14 * (t * 0.35).sin(), 0.16 + 0.08 * (t * 0.27).cos());
    let c2 = at(0.80 + 0.10 * (t * 0.31).cos(), 0.70 + 0.12 * (t * 0.23).sin());
    let c3 = at(0.45 + 0.20 * (t * 0.19).sin(), 0.95 + 0.05 * (t * 0.41).cos());
    blob(&bp, c1, w * 0.8, th.blob_a, (120.0 * strength) as u8);
    blob(&bp, c2, w * 0.85, th.blob_b, (110.0 * strength) as u8);
    blob(&bp, c3, w * 0.6, th.blob_a, (70.0 * strength) as u8);

    let border = if th.light {
        Color32::from_black_alpha(36)
    } else {
        Color32::from_white_alpha(44)
    };
    painter.add(Shape::closed_line(rounded_points(rect, radius), Stroke::new(1.0, border)));
}

/// «Стеклянная» карточка.
pub(crate) fn card(p: &Painter, rect: Rect, r: f32, text: Color32) {
    let c = |a| Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), a);
    p.rect_filled(rect, cr(r), c(16));
    p.rect_stroke(rect, cr(r), Stroke::new(1.0, c(28)), StrokeKind::Inside);
}

// ---------- Аватар ----------

fn circle_image(painter: &Painter, tex: egui::TextureId, center: Pos2, radius: f32) {
    let mut mesh = Mesh::with_texture(tex);
    mesh.vertices.push(Vertex {
        pos: center,
        uv: pos2(0.5, 0.5),
        color: Color32::WHITE,
    });
    let n = 64u32;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        mesh.vertices.push(Vertex {
            pos: center + vec2(c, s) * radius,
            uv: pos2(0.5 + 0.5 * c, 0.5 + 0.5 * s),
            color: Color32::WHITE,
        });
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 2 + i);
    }
    painter.add(Shape::mesh(mesh));
}

/// Круглый аватар. Если картинки нет, рисуется цветной круг с первой буквой имени.
pub(crate) fn avatar(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    tex: Option<&egui::TextureHandle>,
    name: &str,
    accent: Color32,
    on_accent: Color32,
) {
    match tex {
        Some(t) => circle_image(painter, t.id(), center, radius),
        None => {
            painter.circle_filled(center, radius, accent);
            let initial = name
                .trim()
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".to_string());
            painter.text(
                center,
                Align2::CENTER_CENTER,
                initial,
                FontId::proportional(radius * 1.05),
                on_accent,
            );
        }
    }
    painter.circle_stroke(center, radius + 2.0, Stroke::new(1.5, accent));
}

// ---------- Свои виджеты ----------

/// Переключатель в стиле iOS.
pub(crate) fn switch(ui: &mut Ui, rect: Rect, id: Id, on: &mut bool, accent: Color32, off: Color32) {
    let resp = ui.interact(rect, id, Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool_with_time(id.with("a"), *on, 0.16);
    let p = ui.painter();
    p.rect_filled(rect, cr(rect.height() / 2.0), off.lerp_to_gamma(accent, t));
    let r = rect.height() / 2.0 - 3.0;
    let x = egui::lerp((rect.left() + r + 3.0)..=(rect.right() - r - 3.0), t);
    p.circle_filled(pos2(x + 0.5, rect.center().y + 1.0), r, Color32::from_black_alpha(40));
    p.circle_filled(pos2(x, rect.center().y), r, Color32::WHITE);
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// Слайдер со скруглённой дорожкой.
pub(crate) fn slider(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    value: &mut f32,
    lo: f32,
    hi: f32,
    accent: Color32,
    track: Color32,
) {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let (x0, x1, cy) = (rect.left() + 10.0, rect.right() - 10.0, rect.center().y);
    if resp.dragged() || resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let t = ((pp.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            *value = lo + t * (hi - lo);
        }
    }
    let t = ((*value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let xk = x0 + (x1 - x0) * t;
    let grow = ui
        .ctx()
        .animate_bool_with_time(id.with("g"), resp.hovered() || resp.dragged(), 0.12);
    let p = ui.painter();
    p.rect_filled(
        Rect::from_min_max(pos2(x0, cy - 3.0), pos2(x1, cy + 3.0)),
        cr(3.0),
        track,
    );
    p.rect_filled(
        Rect::from_min_max(pos2(x0, cy - 3.0), pos2(xk, cy + 3.0)),
        cr(3.0),
        accent,
    );
    p.circle_filled(pos2(xk, cy), 9.0 + 2.0 * grow, Color32::WHITE);
    p.circle_stroke(pos2(xk, cy), 9.0 + 2.0 * grow, Stroke::new(2.5, accent));
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// Слайдер оттенка с радужной дорожкой.
pub(crate) fn hue_slider(ui: &mut Ui, rect: Rect, id: Id, hue: &mut f32) {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let (x0, x1, cy) = (rect.left() + 10.0, rect.right() - 10.0, rect.center().y);
    if resp.dragged() || resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            *hue = ((pp.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        }
    }
    let n = 36u32;
    let mut mesh = Mesh::default();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = x0 + (x1 - x0) * t;
        let c: Color32 = Hsva::new(t, 0.72, 1.0, 1.0).into();
        mesh.colored_vertex(pos2(x, cy - 4.0), c);
        mesh.colored_vertex(pos2(x, cy + 4.0), c);
    }
    for i in 0..n {
        let a = 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    let p = ui.painter();
    let end: Color32 = Hsva::new(0.0, 0.72, 1.0, 1.0).into();
    p.circle_filled(pos2(x0, cy), 4.0, end);
    p.circle_filled(pos2(x1, cy), 4.0, end);
    p.add(Shape::mesh(mesh));
    let xk = x0 + (x1 - x0) * *hue;
    let cur: Color32 = Hsva::new(*hue, 0.72, 1.0, 1.0).into();
    p.circle_filled(pos2(xk, cy), 10.0, Color32::WHITE);
    p.circle_filled(pos2(xk, cy), 6.5, cur);
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// Сегмент-контрол с плавно переезжающей «таблеткой». Возвращает true при смене.
pub(crate) fn segmented(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    value: &mut u8,
    labels: &[&str],
    accent: Color32,
    on_accent: Color32,
    text: Color32,
) -> bool {
    let h = rect.height();
    let w = rect.width() / labels.len() as f32;
    let mut changed = false;

    ui.painter().rect_filled(
        rect,
        cr(h / 2.0),
        Color32::from_rgba_unmultiplied(text.r(), text.g(), text.b(), 18),
    );
    let pos = ui
        .ctx()
        .animate_value_with_time(id.with("pill"), *value as f32, 0.18);
    let pill = Rect::from_min_size(
        pos2(rect.left() + pos * w + 2.0, rect.top() + 2.0),
        vec2(w - 4.0, h - 4.0),
    );
    ui.painter().rect_filled(pill, cr((h - 4.0) / 2.0), accent);

    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w, h));
        let resp = ui.interact(r, id.with(i), Sense::click());
        if resp.clicked() && *value as usize != i {
            *value = i as u8;
            changed = true;
        }
        let active = *value as usize == i;
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(13.0),
            if active { on_accent } else { text },
        );
        let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    changed
}

/// Кнопка-таблетка. Возвращает true при клике.
pub(crate) fn pill_button(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    label: &str,
    fill: Color32,
    fg: Color32,
) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered(), 0.12);
    let r = if resp.is_pointer_button_down_on() {
        rect.shrink(1.5)
    } else {
        rect
    };
    let p = ui.painter();
    p.rect_filled(r, cr(r.height() / 2.0), fill.lerp_to_gamma(Color32::WHITE, 0.16 * hover));
    p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(14.0), fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}
