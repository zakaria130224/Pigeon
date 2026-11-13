//! The key and value tables used for parameters, headers and variables.

use super::{Trigger, VarEdit, palette, var_layouter, x_button};
use eframe::egui::{self, Align, Align2, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use pigeon_core::http::Vars;
use pigeon_core::model::{BulkEdit, Kv};

const CHECK_W: f32 = 30.0;
const DELETE_W: f32 = 30.0;
const MIN_COLUMN_W: f32 = 80.0;
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 30.0;
/// Space between a column's left edge and its text, shared by the header and the cells.
const CELL_PAD: i8 = 8;
const CORNER: u8 = 4;
const OFF_ROW_OPACITY: f32 = 0.45;

/// Key / value table editor with a trailing empty row for adding entries.
/// Returns true if anything changed.
pub fn kv_table(ui: &mut Ui, id: &str, rows: &mut Vec<Kv>, vars: &Vars, dark: bool, key_hint: &str, show_desc: bool) -> bool {
    let p = palette(dark);
    let mut changed = false;
    let mut remove: Option<usize> = None;
    let mut titles = vec![key_hint, "Value"];
    if show_desc {
        titles.push("Description");
    }
    let cols = titles.len();
    let col_w = ((ui.available_width() - CHECK_W - DELETE_W) / cols as f32).max(MIN_COLUMN_W);

    // One block for the whole table, so every cell can be placed exactly and the columns can't drift.
    let n = rows.len();
    let size = Vec2::new(CHECK_W + DELETE_W + col_w * cols as f32, HEADER_H + ROW_H * (n + 1) as f32);
    let (table, _) = ui.allocate_exact_size(size, Sense::hover());
    let edges: Vec<f32> = (0..=cols).map(|c| (table.left() + CHECK_W + col_w * c as f32).round()).collect();

    ui.painter().rect_filled(table, CORNER, p.input);
    let header = Rect::from_min_size(table.min, Vec2::new(table.width(), HEADER_H));
    ui.painter().rect_filled(header, CornerRadius { nw: CORNER, ne: CORNER, sw: 0, se: 0 }, p.header);
    for (title, x) in titles.iter().zip(&edges) {
        let at = Pos2::new(x + f32::from(CELL_PAD), header.center().y);
        ui.painter().text(at, Align2::LEFT_CENTER, *title, FontId::proportional(12.0), p.weak);
    }

    for i in 0..=n {
        let mut blank = Kv { enabled: true, ..Default::default() };
        let is_new = i == n;
        let row: &mut Kv = if is_new { &mut blank } else { &mut rows[i] };
        let top = header.bottom() + ROW_H * i as f32;
        let band = Rect::from_min_max(Pos2::new(table.left(), top), Pos2::new(table.right(), top + ROW_H));
        if ui.rect_contains_pointer(band) {
            let corners = if is_new { CornerRadius { nw: 0, ne: 0, sw: CORNER, se: CORNER } } else { CornerRadius::ZERO };
            ui.painter().rect_filled(band, corners, p.hover);
        }
        let in_band = |left: f32, right: f32| Rect::from_min_max(Pos2::new(left, band.top()), Pos2::new(right, band.bottom()));

        if !is_new {
            // A child UI, not `ui.put`, which would move the cursor into the table and let later widgets overlap it.
            let rect = in_band(table.left(), edges[0]);
            let mut child =
                ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::centered_and_justified(egui::Direction::LeftToRight)));
            let check = child.add(egui::Checkbox::without_text(&mut row.enabled));
            changed |= check.on_hover_text("Include this row").changed();
        }
        let faded = !is_new && !row.enabled;
        let mut cell = |text: &mut String, hint: &str, column: usize| -> bool {
            let rect = in_band(edges[column], edges[column + 1]);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
            if faded {
                child.multiply_opacity(OFF_ROW_OPACITY);
            }
            // Rows have a fixed height, so a long value stays on one line and scrolls sideways.
            let mut colours = var_layouter(vars, dark, false);
            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| colours(ui, buf, f32::INFINITY);
            let field = VarEdit { id: egui::Id::unique((id, i, column)), vars, dark, trigger: Trigger::Template, multiline: false };
            let hint = hint.to_string();
            let response = field.show(&mut child, text, &mut layouter, |t| {
                // A custom frame replaces the text field's own margin, so the padding has to be on the frame.
                t.hint_text(hint)
                    .frame(Frame::NONE.inner_margin(Margin::symmetric(CELL_PAD, 0)))
                    .align(Align2::LEFT_CENTER)
                    .min_size(rect.size())
                    .desired_width(f32::INFINITY)
            });
            if response.has_focus() {
                ui.painter().rect_stroke(rect.shrink(1.0), 2.0, Stroke::new(1.0, p.accent), egui::StrokeKind::Inside);
            }
            response.changed()
        };
        let mut c = false;
        c |= cell(&mut row.key, if is_new { key_hint } else { "" }, 0);
        c |= cell(&mut row.value, if is_new { "Value" } else { "" }, 1);
        if show_desc {
            c |= cell(&mut row.description, if is_new { "Description" } else { "" }, 2);
        }
        if !is_new {
            let rect = in_band(edges[cols], table.right());
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            if x_button(&mut child, rect.size(), dark).on_hover_text("Remove").clicked() {
                remove = Some(i);
            }
        }
        if c {
            changed = true;
            if is_new && !blank.is_blank() {
                rows.push(blank);
            }
        }
    }

    // Lines go on last, so a hovered row's tint can't cover them.
    let painter = ui.painter();
    let line = Stroke::new(1.0, p.border);
    for r in 0..=n {
        painter.hline(table.x_range(), painter.round_to_pixel_center(header.bottom() + ROW_H * r as f32), line);
    }
    for x in &edges {
        painter.vline(painter.round_to_pixel_center(*x), table.y_range(), line);
    }
    painter.rect_stroke(table, CORNER, line, egui::StrokeKind::Inside);

    if let Some(i) = remove {
        rows.remove(i);
        changed = true;
    }
    changed
}

/// A titled table that can switch to being edited as text. Returns true if anything changed.
pub fn kv_section(ui: &mut Ui, id: &str, title: &str, rows: &mut Vec<Kv>, bulk: &mut Option<BulkEdit>, vars: &Vars, dark: bool) -> bool {
    let p = palette(dark);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).color(p.weak));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (label, tip) = if bulk.is_some() {
                ("Key-Value Edit", "Go back to the table")
            } else {
                ("Bulk Edit", "Edit these rows as text, one key:value per line")
            };
            let button = egui::Button::new(RichText::new(label).color(p.send)).frame(false);
            if ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tip).clicked() {
                *bulk = if bulk.is_some() { None } else { Some(BulkEdit::new(rows)) };
            }
        });
    });
    match bulk {
        Some(edit) => bulk_editor(ui, id, edit, rows, vars, dark),
        None => kv_table(ui, id, rows, vars, dark, "Key", true),
    }
}

fn bulk_editor(ui: &mut Ui, id: &str, edit: &mut BulkEdit, rows: &mut Vec<Kv>, vars: &Vars, dark: bool) -> bool {
    let p = palette(dark);
    edit.refresh(rows);
    let frame = Frame::new().stroke(Stroke::new(1.0, p.border)).fill(p.input).corner_radius(CORNER).inner_margin(8);
    let changed = frame
        .show(ui, |ui| {
            let mut layouter = var_layouter(vars, dark, true);
            let field = VarEdit { id: ui.scope_id().with((id, "bulk")), vars, dark, trigger: Trigger::Template, multiline: true };
            field
                .show(ui, &mut edit.text, &mut layouter, |t| {
                    t.code_editor().hint_text("key:value").frame(Frame::NONE).desired_rows(10).desired_width(f32::INFINITY)
                })
                .changed()
        })
        .inner;
    if changed {
        edit.apply(rows);
    }
    ui.add_space(4.0);
    ui.label(RichText::new("One key:value per line. Start a line with // to leave it out.").small().color(p.weak));
    changed
}
