//! Small line icons, drawn rather than taken from a font.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Ui, Vec2};

/// Paint a small line icon. Shapes are drawn rather than relying on fonts.
pub fn paint_icon(ui: &Ui, rect: Rect, kind: &str, color: Color32) {
    let pt = ui.painter();
    let s = Stroke::new(1.4, color);
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    match kind {
        "collections" => {
            // stacked trash-can-like box (collection)
            let b = Rect::from_center_size(c + Vec2::new(0.0, 1.5), Vec2::new(r * 1.5, r * 1.3));
            pt.rect_stroke(b, 2.0, s, egui::StrokeKind::Middle);
            pt.line_segment([Pos2::new(b.left() - 1.5, b.top() - 3.0), Pos2::new(b.right() + 1.5, b.top() - 3.0)], s);
            pt.line_segment([Pos2::new(c.x - 3.0, b.top() + 4.0), Pos2::new(c.x + 3.0, b.top() + 4.0)], s);
        }
        "environments" => {
            pt.rect_stroke(Rect::from_center_size(c, Vec2::splat(r * 1.5)), 2.0, s, egui::StrokeKind::Middle);
            pt.line_segment([c + Vec2::new(-r * 0.75, -r * 0.25), c + Vec2::new(r * 0.75, -r * 0.25)], s);
            pt.circle_stroke(c + Vec2::new(-r * 0.3, r * 0.3), 1.5, s);
            pt.circle_stroke(c + Vec2::new(r * 0.3, r * 0.3), 1.5, s);
        }
        "history" => {
            pt.circle_stroke(c, r * 0.78, s);
            pt.line_segment([c, c + Vec2::new(0.0, -r * 0.5)], s);
            pt.line_segment([c, c + Vec2::new(r * 0.35, r * 0.2)], s);
        }
        "git" => {
            let a = c + Vec2::new(-r * 0.4, -r * 0.6);
            let b = c + Vec2::new(-r * 0.4, r * 0.6);
            let d = c + Vec2::new(r * 0.5, -r * 0.2);
            pt.circle_stroke(a, 2.2, s);
            pt.circle_stroke(b, 2.2, s);
            pt.circle_stroke(d, 2.2, s);
            pt.line_segment([a + Vec2::new(0.0, 2.2), b - Vec2::new(0.0, 2.2)], s);
            pt.line_segment([d + Vec2::new(-1.2, 2.0), b + Vec2::new(1.5, -1.5)], s);
        }
        "folder" => {
            let b = Rect::from_center_size(c + Vec2::new(0.0, 1.0), Vec2::new(r * 1.7, r * 1.25));
            pt.rect_stroke(b, 1.5, s, egui::StrokeKind::Middle);
            pt.line_segment([b.left_top() + Vec2::new(0.0, 0.0), b.left_top() + Vec2::new(r * 0.6, 0.0)], Stroke::new(2.2, color));
        }
        "pull" => {
            pt.line_segment([c + Vec2::new(0.0, -r * 0.8), c + Vec2::new(0.0, r * 0.6)], s);
            pt.line_segment([c + Vec2::new(-r * 0.5, r * 0.1), c + Vec2::new(0.0, r * 0.6)], s);
            pt.line_segment([c + Vec2::new(r * 0.5, r * 0.1), c + Vec2::new(0.0, r * 0.6)], s);
            pt.line_segment([c + Vec2::new(-r * 0.7, r * 0.9), c + Vec2::new(r * 0.7, r * 0.9)], s);
        }
        "push" => {
            pt.line_segment([c + Vec2::new(0.0, r * 0.6), c + Vec2::new(0.0, -r * 0.8)], s);
            pt.line_segment([c + Vec2::new(-r * 0.5, -r * 0.3), c + Vec2::new(0.0, -r * 0.8)], s);
            pt.line_segment([c + Vec2::new(r * 0.5, -r * 0.3), c + Vec2::new(0.0, -r * 0.8)], s);
            pt.line_segment([c + Vec2::new(-r * 0.7, r * 0.9), c + Vec2::new(r * 0.7, r * 0.9)], s);
        }
        "gear" => {
            pt.circle_stroke(c, r * 0.35, s);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                pt.line_segment([c + d * r * 0.55, c + d * r * 0.85], Stroke::new(2.0, color));
            }
            pt.circle_stroke(c, r * 0.62, s);
        }
        "console" => {
            let b = Rect::from_center_size(c, Vec2::new(r * 1.7, r * 1.4));
            pt.rect_stroke(b, 2.0, s, egui::StrokeKind::Middle);
            let l = b.left() + r * 0.35;
            pt.line_segment([Pos2::new(l, b.top() + r * 0.4), Pos2::new(l + r * 0.35, c.y)], s);
            pt.line_segment([Pos2::new(l + r * 0.35, c.y), Pos2::new(l, b.bottom() - r * 0.4)], s);
            pt.line_segment([Pos2::new(c.x, b.bottom() - r * 0.4), Pos2::new(b.right() - r * 0.35, b.bottom() - r * 0.4)], s);
        }
        "copy" => {
            let a = Rect::from_min_size(c + Vec2::new(-r * 0.75, -r * 0.75), Vec2::splat(r * 1.05));
            let b = Rect::from_min_size(c + Vec2::new(-r * 0.3, -r * 0.3), Vec2::splat(r * 1.05));
            pt.rect_stroke(a, 2.0, s, egui::StrokeKind::Middle);
            pt.rect_filled(b, 2.0, ui.visuals().panel_fill);
            pt.rect_stroke(b, 2.0, s, egui::StrokeKind::Middle);
        }
        "search" => {
            pt.circle_stroke(c + Vec2::new(-1.5, -1.5), r * 0.55, s);
            pt.line_segment([c + Vec2::new(r * 0.25, r * 0.25), c + Vec2::new(r * 0.8, r * 0.8)], s);
        }
        "plus" => {
            pt.line_segment([c + Vec2::new(-r * 0.6, 0.0), c + Vec2::new(r * 0.6, 0.0)], s);
            pt.line_segment([c + Vec2::new(0.0, -r * 0.6), c + Vec2::new(0.0, r * 0.6)], s);
        }
        "sun" => {
            pt.circle_stroke(c, r * 0.4, s);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                pt.line_segment([c + d * r * 0.6, c + d * r * 0.85], s);
            }
        }
        "moon" => {
            // crescent outline
            let mut pts = vec![];
            for k in 0..=16 {
                let a = std::f32::consts::FRAC_PI_4 + k as f32 / 16.0 * 1.5 * std::f32::consts::PI;
                pts.push(c + Vec2::new(a.cos(), a.sin()) * r * 0.7);
            }
            let inner = c + Vec2::new(r * 0.28, -r * 0.28);
            for k in (0..=16).rev() {
                let a = std::f32::consts::FRAC_PI_4 + k as f32 / 16.0 * 1.5 * std::f32::consts::PI;
                pts.push(inner + Vec2::new(a.cos(), a.sin()) * r * 0.5);
            }
            pts.push(pts[0]);
            pt.add(egui::Shape::line(pts, s));
        }
        "more" => {
            for dx in [-r * 0.6, 0.0, r * 0.6] {
                pt.circle_filled(c + Vec2::new(dx, 0.0), 1.5, color);
            }
        }
        _ => {
            pt.circle_stroke(c, r * 0.6, s);
        }
    }
}
