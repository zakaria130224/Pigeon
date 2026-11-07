//! Pages for environments, collections, folders and the runner.

use super::*;
use eframe::egui::{Align, FontId, Layout, RichText, Sense, Vec2};

impl App {
    pub(super) fn env_view(&mut self, ui: &mut Ui, id: Option<String>, now: f64) {
        let p = self.pal();
        let dark = self.dark();
        let vars = self.vars_for(None);
        let mut changed = false;
        let mut set_active: Option<Option<String>> = None;
        let mut clear_local = false;
        match &id {
            Some(id) => {
                let is_active = self.settings.active_env.as_deref() == Some(id.as_str());
                let Some(e) = self.envs.iter_mut().find(|e| &e.id == id) else { return };
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    paint_icon(ui, r, "environments", p.weak);
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut e.name)
                                .frame(egui::Frame::NONE)
                                .font(FontId::proportional(17.0))
                                .desired_width(300.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if is_active {
                            if ui.button("Deactivate").clicked() {
                                set_active = Some(None);
                            }
                            ui.label(RichText::new("● Active").color(p.ok));
                        } else if primary_button(ui, "Set as active", p.send, true).clicked() {
                            set_active = Some(Some(id.clone()));
                        }
                    });
                });
                ui.label(
                    RichText::new("Shared with your team through git. Use variables as {{name}} in URLs, headers and bodies.")
                        .small()
                        .color(p.weak),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if kv_table(ui, "envvars", &mut e.values, &vars, dark, "Variable", false) {
                        changed = true;
                    }
                    script_values(ui, &mut self.settings.script_vars, id, &mut clear_local, dark);
                });
            }
            None => {
                ui.label(RichText::new("Globals").heading());
                ui.label(
                    RichText::new("Global variables work in every request. They stay on this computer and are not shared.")
                        .small()
                        .color(p.weak),
                );
                ui.add_space(8.0);
                let mut ch = false;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ch = kv_table(ui, "globals", &mut self.settings.globals, &vars, dark, "Variable", false);
                });
                if ch {
                    self.save_settings();
                }
            }
        }
        if changed {
            self.mark_dirty(now);
        }
        if clear_local {
            self.save_settings();
        }
        if let Some(v) = set_active {
            self.actions.push(Action::SetActiveEnv(v));
        }
    }

    pub(super) fn collection_view(&mut self, ui: &mut Ui, id: &str, sub: &mut usize, now: f64) {
        let p = self.pal();
        let dark = self.dark();
        let vars = self.vars_for(Some(id));
        let mut changed = false;
        let mut run = false;
        let mut clear_local = false;
        let Some(c) = self.collections.iter_mut().find(|c| c.id == id) else { return };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            paint_icon(ui, r, "collections", p.weak);
            if ui
                .add(egui::TextEdit::singleline(&mut c.name).frame(egui::Frame::NONE).font(FontId::proportional(17.0)).desired_width(360.0))
                .changed()
            {
                changed = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, "▶ Run", p.send, true).clicked() {
                    run = true;
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let has_auth = !matches!(c.auth.kind, AuthKind::Inherit | AuthKind::None);
            let has_vars = c.variables.iter().any(|v| v.enabled && !v.key.is_empty());
            let tabs = [("Overview", false), ("Authorization", has_auth), ("Scripts", !c.scripts.is_empty()), ("Variables", has_vars)];
            for (i, (label, in_use)) in tabs.into_iter().enumerate() {
                if tab_label(ui, *sub == i, label, in_use.then(|| "●".to_string()), dark).clicked() {
                    *sub = i;
                }
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match *sub {
            0 => {
                ui.label(RichText::new(format!("{} requests", count_requests(&c.items))).color(p.weak));
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut c.description)
                            .hint_text("Add a description for your team…")
                            .desired_rows(8)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    changed = true;
                }
            }
            1 => {
                ui.label(RichText::new("Used by every request in this collection that inherits auth.").small().color(p.weak));
                ui.add_space(4.0);
                if auth_editor(ui, &mut c.auth, false, &vars, dark) {
                    changed = true;
                }
            }
            2 => {
                ui.label(
                    RichText::new("These run for every request in this collection, before the request's own scripts.")
                        .small()
                        .color(p.weak),
                );
                ui.add_space(4.0);
                if scripts_editor(ui, &mut c.scripts, &vars, dark) {
                    changed = true;
                }
            }
            _ => {
                ui.label(RichText::new("Collection variables are available to every request in this collection.").small().color(p.weak));
                ui.add_space(4.0);
                if kv_table(ui, "collvars", &mut c.variables, &vars, dark, "Variable", false) {
                    changed = true;
                }
                script_values(ui, &mut self.settings.script_vars, id, &mut clear_local, dark);
            }
        });
        if changed {
            self.mark_dirty(now);
        }
        if clear_local {
            self.save_settings();
        }
        if run {
            self.actions.push(Action::RunCollection(id.to_string()));
        }
    }

    pub(super) fn folder_view(&mut self, ui: &mut Ui, coll: &str, id: &str, now: f64) {
        let p = self.pal();
        let dark = self.dark();
        let vars = self.vars_for(Some(coll));
        let mut changed = false;
        let Some(c) = self.collections.iter_mut().find(|c| c.id == coll) else { return };
        let Some(f) = find_folder_mut(&mut c.items, id) else { return };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            paint_icon(ui, r, "folder", p.weak);
            if ui
                .add(egui::TextEdit::singleline(&mut f.name).frame(egui::Frame::NONE).font(FontId::proportional(17.0)).desired_width(360.0))
                .changed()
            {
                changed = true;
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("Description").color(p.weak));
            if ui.add(egui::TextEdit::multiline(&mut f.description).desired_rows(4).desired_width(f32::INFINITY)).changed() {
                changed = true;
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Authorization").color(p.weak));
            if auth_editor(ui, &mut f.auth, true, &vars, dark) {
                changed = true;
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Scripts").color(p.weak));
            ui.label(RichText::new("These run for every request in this folder, before the request's own scripts.").small().color(p.weak));
            ui.add_space(4.0);
            if scripts_editor(ui, &mut f.scripts, &vars, dark) {
                changed = true;
            }
        });
        if changed {
            self.mark_dirty(now);
        }
    }

    pub(super) fn runner_view(&mut self, ui: &mut Ui, run: &mut RunnerTab) {
        let p = self.pal();
        let dark = self.dark();
        let name = self.coll(&run.coll).map(|c| c.name.clone()).unwrap_or_default();
        ui.label(RichText::new(format!("Run \"{}\"", name)).heading());
        ui.add_space(6.0);
        let running = run.rx.is_some();
        ui.horizontal(|ui| {
            ui.label("Iterations");
            ui.add(egui::DragValue::new(&mut run.iterations).range(1..=100));
            ui.label("Delay (ms)");
            ui.add(egui::DragValue::new(&mut run.delay_ms).range(0..=10000));
            let label = if running { "Running…" } else { "▶ Run collection" };
            if primary_button(ui, label, p.send, !running).clicked() {
                self.start_run(run);
            }
            if running && ui.button("Stop").clicked() {
                run.rx = None;
            }
        });
        if running {
            ui.ctx().request_repaint();
        }
        ui.add_space(8.0);
        if run.started {
            let total_tests: usize = run.results.iter().map(|r| r.tests.len()).sum();
            let passed: usize = run.results.iter().map(|r| r.tests.iter().filter(|t| t.1).count()).sum();
            let failed_req = run.results.iter().filter(|r| r.error.is_some() || r.status >= 400).count();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} requests", run.results.len())).strong());
                ui.label(RichText::new(format!("Passed: {}", passed)).color(p.ok));
                ui.label(RichText::new(format!("Failed: {}", total_tests - passed)).color(if total_tests > passed {
                    p.err
                } else {
                    p.weak
                }));
                if failed_req > 0 {
                    ui.label(RichText::new(format!("{} request(s) with errors", failed_req)).color(p.err));
                }
            });
            ui.separator();
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (i, r) in run.results.iter().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(method_short(&r.method)).small().strong().color(method_color(&r.method, dark)));
                        ui.label(RichText::new(&r.name).strong());
                        ui.label(RichText::new(&r.url).small().color(p.weak));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("{} ms", r.time_ms)).small().color(p.weak));
                            match &r.error {
                                Some(_) => ui.label(RichText::new("Error").color(p.err)),
                                None => ui.label(RichText::new(r.status.to_string()).color(status_color(r.status, dark))),
                            };
                        });
                    });
                    if let Some(e) = &r.error {
                        ui.label(RichText::new(e).small().color(p.err));
                    }
                    for (n, ok, d) in &r.tests {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            ui.label(RichText::new(if *ok { "PASS" } else { "FAIL" }).small().strong().color(if *ok {
                                p.ok
                            } else {
                                p.err
                            }));
                            ui.label(RichText::new(n).small());
                            if !ok {
                                ui.label(RichText::new(d).small().color(p.weak));
                            }
                        });
                    }
                    ui.separator();
                });
            }
            if !run.started {
                ui.label(RichText::new("Sends every request in the collection in order and runs their tests.").color(p.weak));
            }
        });
    }
}
