//! Small reusable controls.

use super::{paint_icon, palette};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, TextFormat, Ui, Vec2, text::LayoutJob,
};

/// Underlined tab label like in the request editor ("Params", "Body", ...).
pub fn tab_label(ui: &mut Ui, selected: bool, text: &str, badge: Option<String>, dark: bool) -> Response {
    let p = palette(dark);
    let mut job = LayoutJob::default();
    let col = if selected { p.text } else { p.weak };
    job.append(text, 0.0, TextFormat::simple(FontId::proportional(13.0), col));
    if let Some(b) = badge {
        job.append(&format!(" {}", b), 0.0, TextFormat::simple(FontId::proportional(12.0), p.ok));
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let size = galley.size() + Vec2::new(12.0, 14.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() && !selected {
        ui.painter().rect_filled(rect.shrink2(Vec2::new(0.0, 3.0)), 4.0, p.hover);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, col);
    if selected {
        let y = rect.bottom() - 1.0;
        ui.painter().line_segment([Pos2::new(rect.left() + 4.0, y), Pos2::new(rect.right() - 4.0, y)], Stroke::new(2.0, p.accent));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A colored primary button.
pub fn primary_button(ui: &mut Ui, text: &str, fill: Color32, enabled: bool) -> Response {
    let b = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(0.0, 30.0));
    ui.add_enabled(enabled, b)
}

/// A small painted "x" button (the font has no reliable cross glyph).
pub fn x_button(ui: &mut Ui, size: Vec2, dark: bool) -> Response {
    let p = palette(dark);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let c = rect.center();
    if resp.hovered() {
        ui.painter().rect_filled(Rect::from_center_size(c, Vec2::splat(18.0)), 3.0, p.hover);
    }
    let col = if resp.hovered() { p.text } else { p.weak };
    let s = Stroke::new(1.3, col);
    ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], s);
    ui.painter().line_segment([c + Vec2::new(4.0, -4.0), c + Vec2::new(-4.0, 4.0)], s);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Small icon button (painted icon), optional text after it.
pub fn icon_button(ui: &mut Ui, icon: &str, text: &str, dark: bool, tooltip: &str) -> Response {
    let p = palette(dark);
    let font = FontId::proportional(13.0);
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, p.text));
    let w = 26.0 + if text.is_empty() { 0.0 } else { galley.size().x + 8.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, p.hover);
    }
    let icon_rect = Rect::from_min_size(rect.min + Vec2::new(5.0, 6.0), Vec2::splat(16.0));
    paint_icon(ui, icon_rect, icon, if resp.hovered() { p.text } else { p.weak });
    if !text.is_empty() {
        ui.painter().galley(Pos2::new(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, p.text);
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}
