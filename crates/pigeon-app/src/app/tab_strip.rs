//! The row of open tabs, and showing the active one.

use super::*;
use eframe::egui::{Align, Align2, FontId, Layout, Pos2, Rect, Sense, Vec2};

impl App {
    pub(super) fn tab_strip(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        let full = ui.available_rect_before_wrap();
        let strip = Rect::from_min_size(full.min, Vec2::new(full.width(), 38.0));
        ui.painter().rect_filled(strip, 0.0, p.header);
        ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, p.border));
        let env_w = 210.0;
        let tabs_rect = Rect::from_min_max(strip.min, Pos2::new(strip.right() - env_w - 12.0, strip.bottom()));
        let env_rect = Rect::from_min_max(Pos2::new(strip.right() - env_w - 6.0, strip.top()), strip.max);
        let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs_rect).layout(Layout::left_to_right(Align::Center)));
        egui::ScrollArea::horizontal()
            .id_salt("tabscroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(&mut tabs_ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let mut close: Option<usize> = None;
                for (i, t) in self.tabs.iter().enumerate() {
                    let (method, name, dirty) = match &t.kind {
                        TabKind::Request(rt) => (Some(rt.req.method.clone()), rt.req.name.clone(), rt.dirty() && rt.origin.is_some()),
                        TabKind::Env(id) => {
                            (None, self.envs.iter().find(|e| &e.id == id).map(|e| e.name.clone()).unwrap_or_default(), false)
                        }
                        TabKind::Globals => (None, "Globals".into(), false),
                        TabKind::Collection { id, .. } => (None, self.coll(id).map(|c| c.name.clone()).unwrap_or_default(), false),
                        TabKind::Folder { coll, id } => (
                            None,
                            self.coll(coll).and_then(|c| find_folder_ref(&c.items, id)).map(|f| f.name.clone()).unwrap_or_default(),
                            false,
                        ),
                        TabKind::Runner(r) => {
                            (None, format!("Runner · {}", self.coll(&r.coll).map(|c| c.name.as_str()).unwrap_or("")), false)
                        }
                    };
                    let sel = i == self.active;
                    let mut label: String = name.chars().take(24).collect();
                    if name.chars().count() > 24 {
                        label.push('…');
                    }
                    let font = FontId::proportional(12.5);
                    let mlabel = method.as_deref().map(method_short).unwrap_or_default();
                    let mfont = FontId::proportional(10.5);
                    let mw = if mlabel.is_empty() {
                        18.0
                    } else {
                        ui.fonts_mut(|f| f.layout_no_wrap(mlabel.clone(), mfont.clone(), p.text).size().x) + 6.0
                    };
                    let tw = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), p.text).size().x);
                    let w = (mw + tw + 46.0).clamp(100.0, 250.0);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 38.0), Sense::click());
                    let painter = ui.painter();
                    if sel {
                        painter.rect_filled(rect, 0.0, p.bg);
                        painter.line_segment(
                            [rect.left_top() + Vec2::new(0.0, 1.0), rect.right_top() + Vec2::new(0.0, 1.0)],
                            Stroke::new(2.0, p.accent),
                        );
                    } else if resp.hovered() {
                        painter.rect_filled(rect, 0.0, p.hover);
                    }
                    painter.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, p.border));
                    let mut x = rect.left() + 12.0;
                    if !mlabel.is_empty() {
                        let m = method.as_deref().unwrap_or("");
                        painter.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, &mlabel, mfont, method_color(m, dark));
                        x += mw;
                    } else {
                        let ic = Rect::from_center_size(Pos2::new(x + 6.0, rect.center().y), Vec2::splat(13.0));
                        let kind = match &t.kind {
                            TabKind::Env(_) | TabKind::Globals => "environments",
                            TabKind::Folder { .. } => "folder",
                            TabKind::Runner(_) => "history",
                            _ => "collections",
                        };
                        paint_icon(ui, ic, kind, p.weak);
                        x += mw;
                    }
                    painter.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, &label, font, if sel { p.text } else { p.weak });
                    let cx = Rect::from_center_size(Pos2::new(rect.right() - 15.0, rect.center().y), Vec2::splat(16.0));
                    let hovered_close = ui.rect_contains_pointer(cx);
                    if dirty && !resp.hovered() {
                        painter.circle_filled(cx.center(), 3.5, p.accent);
                    } else if resp.hovered() || sel {
                        if hovered_close {
                            painter.rect_filled(cx, 3.0, p.selected);
                        }
                        let c = cx.center();
                        let s = Stroke::new(1.2, p.weak);
                        painter.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], s);
                        painter.line_segment([c + Vec2::new(3.5, -3.5), c + Vec2::new(-3.5, 3.5)], s);
                    }
                    if resp.clicked() {
                        if hovered_close {
                            close = Some(i);
                        } else {
                            self.active = i;
                        }
                    }
                    if resp.middle_clicked() {
                        close = Some(i);
                    }
                    resp.on_hover_text(&name);
                }
                ui.add_space(4.0);
                if icon_button(ui, "plus", "", dark, &format!("New request tab ({})", shortcut("T", false))).clicked() {
                    self.actions.push(Action::NewTab);
                }
                if let Some(i) = close {
                    self.actions.push(Action::CloseTab(i));
                }
            });
        let mut env_ui = ui.new_child(egui::UiBuilder::new().max_rect(env_rect).layout(Layout::left_to_right(Align::Center)));
        let cur = self.active_env().map(|e| e.name.clone()).unwrap_or_else(|| "No environment".into());
        let mut pick: Option<Option<String>> = None;
        egui::ComboBox::from_id_salt("envpick").selected_text(cur).width(env_w - 12.0).show_ui(&mut env_ui, |ui| {
            if ui.selectable_label(self.settings.active_env.is_none(), "No environment").clicked() {
                pick = Some(None);
            }
            for e in &self.envs {
                if ui.selectable_label(self.settings.active_env.as_deref() == Some(e.id.as_str()), &e.name).clicked() {
                    pick = Some(Some(e.id.clone()));
                }
            }
        });
        if let Some(v) = pick {
            self.actions.push(Action::SetActiveEnv(v));
        }
        ui.advance_cursor_after_rect(strip);
    }

    pub(super) fn main_area(&mut self, ui: &mut Ui) {
        self.tab_strip(ui);
        if self.tabs.is_empty() {
            return;
        }
        let i = self.active.min(self.tabs.len() - 1);
        self.active = i;
        let mut tab = std::mem::replace(&mut self.tabs[i], Tab { uid: 0, kind: TabKind::Globals });
        let now = ui.ctx().input(|inp| inp.time);
        egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 10, bottom: 0 }).show(ui, |ui| {
            ui.push_id(tab.uid, |ui| match &mut tab.kind {
                TabKind::Request(rt) => self.request_view(ui, rt, i),
                TabKind::Env(id) => {
                    let id = id.clone();
                    self.env_view(ui, Some(id), now);
                }
                TabKind::Globals => self.env_view(ui, None, now),
                TabKind::Collection { id, sub, bulk_headers } => {
                    let id = id.clone();
                    self.collection_view(ui, &id, sub, bulk_headers, now);
                }
                TabKind::Folder { coll, id } => {
                    let (c, f) = (coll.clone(), id.clone());
                    self.folder_view(ui, &c, &f, now);
                }
                TabKind::Runner(run) => self.runner_view(ui, run),
            });
        });
        if i < self.tabs.len() && self.tabs[i].uid == 0 {
            self.tabs[i] = tab;
        }
    }
}
