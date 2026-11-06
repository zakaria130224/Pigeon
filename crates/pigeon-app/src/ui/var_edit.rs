//! A text field with variable suggestions, and details when pointing at a variable.

use super::palette;
use eframe::egui::{
    self, Align2, Color32, EventFilter, FontId, Id, Key, Modifiers, Pos2, Rect, Response, RichText, Sense, Stroke, TextEdit, Ui, Vec2,
    text::CCursor, text::CCursorRange,
};
use pigeon_core::http::{VarHint, VarKind, Vars};
pub use pigeon_core::suggest::Trigger;
use pigeon_core::suggest::{candidates, closest, find_partial, insert, var_at};
use std::sync::Arc;

const MAX_ROWS: usize = 8;

pub type Layouter<'a> = dyn FnMut(&Ui, &dyn egui::TextBuffer, f32) -> Arc<egui::Galley> + 'a;

#[derive(Clone, Default)]
struct State {
    /// Something was typed, so a list may show. Moving around existing text doesn't open it.
    armed: bool,
    open: bool,
    selected: usize,
    query: String,
    names: Vec<String>,
    list_rect: Option<Rect>,
}

/// A text field with variable suggestions and hover details.
pub struct VarEdit<'a> {
    pub id: Id,
    pub vars: &'a Vars,
    pub dark: bool,
    pub trigger: Trigger,
    pub multiline: bool,
}

impl VarEdit<'_> {
    fn cursor(&self, ui: &Ui) -> Option<usize> {
        let state = TextEdit::load_state(ui.ctx(), self.id)?;
        Some(state.cursor.char_range()?.primary.index.0)
    }

    fn place_cursor(&self, ui: &Ui, at: usize) {
        let mut state = TextEdit::load_state(ui.ctx(), self.id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(at))));
        TextEdit::store_state(ui.ctx(), self.id, state);
    }

    fn accept(&self, ui: &Ui, text: &mut String, name: &str) -> bool {
        let Some(partial) = self.cursor(ui).and_then(|c| find_partial(text, c, self.trigger)) else { return false };
        let at = insert(text, &partial, name);
        self.place_cursor(ui, at);
        true
    }

    pub fn show(
        self,
        ui: &mut Ui,
        text: &mut String,
        layouter: &mut Layouter<'_>,
        style: impl for<'e> FnOnce(TextEdit<'e>) -> TextEdit<'e>,
    ) -> Response {
        let key = self.id.with("suggestions");
        let mut state: State = ui.data(|d| d.get_temp(key)).unwrap_or_default();
        let mut inserted = false;

        // The list takes these keys before the text field can act on them.
        if state.open && ui.memory(|m| m.has_focus(self.id)) {
            let press = |ui: &mut Ui, k: Key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, k));
            let count = state.names.len().max(1);
            if press(ui, Key::ArrowDown) {
                state.selected = (state.selected + 1) % count;
            }
            if press(ui, Key::ArrowUp) {
                state.selected = (state.selected + count - 1) % count;
            }
            if press(ui, Key::Escape) {
                state.armed = false;
            }
            let enter = press(ui, Key::Enter);
            if enter || press(ui, Key::Tab) {
                if let Some(name) = state.names.get(state.selected).cloned() {
                    inserted = self.accept(ui, text, &name);
                }
                state.armed = false;
            }
        }

        let base = if self.multiline { TextEdit::multiline(&mut *text) } else { TextEdit::singleline(&mut *text) };
        let mut edit = style(base).id(self.id).layouter(&mut *layouter);
        if state.open {
            edit = edit.event_filter(EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true });
        }
        let output = edit.show(ui);
        let mut response = output.response.response.clone();
        if inserted {
            response.mark_changed();
        }

        let pointer = ui.ctx().pointer_latest_pos();
        let over_list = matches!((state.list_rect, pointer), (Some(r), Some(p)) if r.contains(p));
        let focused = response.has_focus();
        if response.changed() && !inserted {
            state.armed = true;
        }
        if !focused && !over_list {
            state.armed = false;
        }

        let cursor = output.state.cursor.char_range().map(|r| r.primary.index.0);
        let partial = cursor.filter(|_| state.armed).and_then(|c| find_partial(text, c, self.trigger));
        if partial.is_none() {
            state.armed = false;
        }
        let items = partial.as_ref().map(|p| candidates(self.vars, p)).unwrap_or_default();
        let complete = partial.as_ref().is_some_and(|p| p.closed > 0 && items.len() == 1 && items[0].name == p.query);
        state.open = state.armed && !items.is_empty() && !complete;
        state.list_rect = None;

        if state.open {
            let query = partial.as_ref().map(|p| p.query.clone()).unwrap_or_default();
            if query != state.query {
                state.query = query;
                state.selected = 0;
            }
            let shown = &items[..items.len().min(MAX_ROWS)];
            state.names = shown.iter().map(|h| h.name.clone()).collect();
            state.selected = state.selected.min(shown.len() - 1);
            let caret = output.galley.pos_from_cursor(CCursor::new(cursor.unwrap_or(0)));
            let at = output.galley_pos + caret.left_bottom().to_vec2() + Vec2::new(0.0, 4.0);
            let (clicked, rect) = suggestion_list(ui.ctx(), at, shown, state.selected, self.dark);
            state.list_rect = Some(rect);
            if let Some(i) = clicked {
                let name = state.names[i].clone();
                if self.accept(ui, text, &name) {
                    response.mark_changed();
                }
                response.request_focus();
                state.armed = false;
                state.open = false;
            }
            ui.ctx().request_repaint();
        } else if response.hovered()
            && let Some(p) = pointer
        {
            let index = output.galley.cursor_from_pos(p - output.galley_pos).index.0;
            if let Some(name) = var_at(text, index) {
                response = response.on_hover_ui_at_pointer(|ui| variable_details(ui, &name, self.vars, self.dark));
            }
        }
        ui.data_mut(|d| d.insert_temp(key, state));
        response
    }
}

fn badge(kind: VarKind, dark: bool) -> (&'static str, Color32) {
    let p = palette(dark);
    match kind {
        VarKind::Environment => ("E", p.ok),
        VarKind::Collection => ("C", p.send),
        VarKind::Global => ("G", p.accent),
        VarKind::Dynamic => ("$", Color32::from_rgb(0x8e, 0x5c, 0xc4)),
    }
}

fn one_line(value: &str, limit: usize) -> String {
    let flat = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > limit { flat.chars().take(limit).collect::<String>() + "…" } else { flat }
}

fn suggestion_list(ctx: &egui::Context, at: Pos2, items: &[&VarHint], selected: usize, dark: bool) -> (Option<usize>, Rect) {
    let p = palette(dark);
    let mut clicked = None;
    let shown =
        egui::Area::new(Id::unique("variable suggestions")).order(egui::Order::Foreground).fixed_pos(at).constrain(true).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(p.bg).inner_margin(4).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let width = 380.0;
                ui.set_width(width);
                for (i, hint) in items.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
                    if i == selected {
                        ui.painter().rect_filled(rect, 4.0, p.selected);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 4.0, p.hover);
                    }
                    let (letter, color) = badge(hint.kind, dark);
                    let dot = Rect::from_center_size(Pos2::new(rect.left() + 15.0, rect.center().y), Vec2::splat(17.0));
                    ui.painter().rect_filled(dot, 4.0, color);
                    ui.painter().text(dot.center(), Align2::CENTER_CENTER, letter, FontId::proportional(10.5), Color32::WHITE);
                    let name = ui.painter().text(
                        Pos2::new(rect.left() + 32.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        one_line(&hint.name, 34),
                        FontId::monospace(12.5),
                        p.text,
                    );
                    let room = ((rect.right() - name.right() - 16.0) / 6.5).max(4.0) as usize;
                    ui.painter().text(
                        Pos2::new(rect.right() - 8.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        one_line(&hint.value, room),
                        FontId::proportional(11.5),
                        p.weak,
                    );
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        clicked = Some(i);
                    }
                }
                ui.add_space(4.0);
                let line = ui.available_rect_before_wrap().left_top();
                ui.painter().line_segment([line, line + Vec2::new(width, 0.0)], Stroke::new(1.0, p.border));
                ui.add_space(3.0);
                ui.label(RichText::new("↑↓ choose   Enter or Tab insert   Esc close").small().color(p.weak));
            });
        });
    (clicked, shown.response.rect)
}

fn variable_details(ui: &mut Ui, name: &str, vars: &Vars, dark: bool) {
    let p = palette(dark);
    ui.set_max_width(360.0);
    match vars.hints.iter().find(|h| h.name == name) {
        Some(hint) => {
            ui.label(RichText::new(name).monospace().strong());
            ui.label(RichText::new(one_line(&hint.value, 200)).monospace());
            ui.label(RichText::new(&hint.place).small().color(p.weak));
        }
        None => {
            ui.label(RichText::new(name).monospace().strong().color(p.err));
            ui.label("This variable isn't defined.");
            match closest(name, vars) {
                Some(other) => ui.label(RichText::new(format!("Did you mean {}?", other)).small().color(p.weak)),
                None => ui.label(RichText::new("Add it to an environment, the collection's variables or Globals.").small().color(p.weak)),
            };
        }
    }
}
