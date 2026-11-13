//! The request editor and the response below it.

use super::*;
use eframe::egui::{Align, Color32, FontId, Id, Layout, PointerButton, Pos2, Rect, RichText, Sense, Vec2};
use pigeon_core::suggest;

/// Height inside the URL bar's border, which makes the whole bar as tall as the Send button.
const URL_ROW_H: f32 = 30.0;

impl App {
    pub(super) fn request_view(&mut self, ui: &mut Ui, rt: &mut ReqTab, idx: usize) {
        let p = self.pal();
        let dark = self.dark();
        let vars = self.vars_for(rt.origin.as_deref());
        // Breadcrumb + save
        ui.horizontal(|ui| {
            match rt.origin.as_ref().and_then(|cid| self.coll(cid)) {
                Some(c) => {
                    let mut path = vec![];
                    path_to(&c.items, &rt.req.id, &mut path);
                    ui.label(RichText::new(&c.name).color(p.weak));
                    for f in path {
                        ui.label(RichText::new("/").color(p.weak));
                        ui.label(RichText::new(f).color(p.weak));
                    }
                    ui.label(RichText::new("/").color(p.weak));
                }
                None => {
                    ui.label(RichText::new("HTTP").small().color(p.weak));
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut rt.req.name)
                    .frame(egui::Frame::NONE)
                    .align(egui::Align2::LEFT_CENTER)
                    .min_size(Vec2::new(0.0, ui.spacing().interact_size.y))
                    .desired_width(280.0)
                    .font(FontId::proportional(14.0)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new("</>").monospace()).selected(rt.show_code))
                    .on_hover_text("Code snippet")
                    .clicked()
                {
                    rt.show_code = !rt.show_code;
                }
                let save_txt = if rt.dirty() { "Save ●" } else { "Save" };
                if ui.button(save_txt).on_hover_text(format!("Save ({})", shortcut("S", false))).clicked() {
                    self.actions.push(Action::SaveTab(idx));
                }
            });
        });
        ui.add_space(6.0);

        // URL bar
        let mut send_now = false;
        let sending = rt.rx.is_some();
        ui.horizontal(|ui| {
            let send_w = 100.0;
            let bar_w = ui.available_width() - send_w - 8.0;
            egui::Frame::new()
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(4)
                .inner_margin(Margin::symmetric(2, 1))
                .fill(p.input)
                .show(ui, |ui| {
                    ui.set_width(bar_w - 6.0);
                    let row = Vec2::new(ui.available_width(), URL_ROW_H);
                    ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
                        let m = rt.req.method.clone();
                        egui::ComboBox::from_id_salt("method")
                            .selected_text(RichText::new(&m).strong().color(method_color(&m, dark)))
                            .width(92.0)
                            .show_ui(ui, |ui| {
                                for mm in METHODS {
                                    if ui.selectable_label(m == mm, RichText::new(mm).strong().color(method_color(mm, dark))).clicked() {
                                        rt.req.method = mm.to_string();
                                    }
                                }
                            });
                        let sep = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [Pos2::new(sep.left() - 3.0, sep.top() + 4.0), Pos2::new(sep.left() - 3.0, sep.bottom() - 4.0)],
                            Stroke::new(1.0, p.border),
                        );
                        let mut layouter = var_layouter(&vars, dark, false);
                        let field = VarEdit {
                            id: Id::unique(("url", rt.req.id.clone())),
                            vars: &vars,
                            dark,
                            trigger: Trigger::Template,
                            multiline: false,
                        };
                        let r = field.show(ui, &mut rt.req.url, &mut layouter, |t| {
                            // A custom frame replaces the text field's own margin, so the padding has to be on the frame.
                            t.hint_text("Enter URL or paste a cURL command")
                                .frame(egui::Frame::NONE.inner_margin(Margin::symmetric(6, 0)))
                                .align(egui::Align2::LEFT_CENTER)
                                .min_size(Vec2::new(0.0, URL_ROW_H))
                                .desired_width(f32::INFINITY)
                        });
                        if r.changed() {
                            if rt.req.url.trim_start().starts_with("curl ") {
                                if let Some(mut c) = http::parse_curl(&rt.req.url) {
                                    c.id = rt.req.id.clone();
                                    c.name = rt.req.name.clone();
                                    rt.req = c;
                                }
                            } else {
                                sync_params_from_url(&rt.req.url, &mut rt.req.params);
                            }
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            send_now = true;
                        }
                    });
                });
            let (label, fill) = if sending { ("Cancel", p.weak) } else { ("Send", p.send) };
            let b = ui
                .add_sized(
                    [send_w, 34.0],
                    egui::Button::new(RichText::new(label).color(Color32::WHITE).strong()).fill(fill).corner_radius(4),
                )
                .on_hover_text(format!("Send ({})", shortcut("Enter", false)));
            if b.clicked() {
                if sending {
                    rt.rx = None;
                } else {
                    send_now = true;
                }
            }
        });
        let all_text = format!(
            "{} {} {}",
            rt.req.url,
            rt.req.headers.iter().filter(|h| h.enabled).map(|h| h.value.as_str()).collect::<Vec<_>>().join(" "),
            if rt.req.body.mode == BodyMode::Raw { rt.req.body.raw.as_str() } else { "" }
        );
        let missing = vars.missing(&all_text);
        if !missing.is_empty() {
            let names: Vec<String> = missing
                .iter()
                .map(|name| match suggest::closest(name, &vars) {
                    Some(other) => format!("{} (did you mean {}?)", name, other),
                    None => name.clone(),
                })
                .collect();
            ui.label(RichText::new(format!("Unknown variable: {}. Pick an environment or add it.", names.join(", "))).small().color(p.err));
        }
        if (send_now || ui.ctx().input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))) && !sending {
            self.send_tab(rt);
        }

        // Code panel on the right
        if rt.show_code {
            egui::Panel::right("code_panel").default_size(360.0).min_size(240.0).resizable(true).show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Code snippet").strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if x_button(ui, Vec2::splat(22.0), dark).clicked() {
                            rt.show_code = false;
                        }
                    });
                });
                let langs = ["cURL", "JavaScript - Fetch", "Python - Requests"];
                egui::ComboBox::from_id_salt("codelang").selected_text(langs[rt.code_lang]).show_ui(ui, |ui| {
                    for (i, l) in langs.iter().enumerate() {
                        ui.selectable_value(&mut rt.code_lang, i, *l);
                    }
                });
                let chain = self.auth_chain_for(rt.origin.as_deref(), &rt.req.id);
                let mut sent = rt.req.clone();
                http::inherit_headers(&mut sent, &self.collection_headers(rt.origin.as_deref()), &vars);
                let pr = http::prepare(&sent, &http::effective_auth(&sent, &chain), &vars);
                let (code, colours): (String, CodeColouring) = match rt.code_lang {
                    1 => (http::fetch_snippet(&pr), js_job),
                    2 => (http::python_snippet(&pr), python_job),
                    _ => (http::curl_snippet(&pr), shell_job),
                };
                if ui.small_button("Copy").clicked() {
                    ui.ctx().copy_text(code.clone());
                }
                code_view(ui, "codescroll", &code, dark, colours);
            });
        }

        // Response at the bottom
        let avail_h = ui.available_height();
        egui::Panel::bottom("resp_panel")
            .resizable(true)
            .default_size((avail_h * 0.52).max(200.0))
            .min_size(120.0)
            .frame(egui::Frame::new().inner_margin(Margin { left: 0, right: 0, top: 8, bottom: 4 }))
            .show(ui, |ui| self.response_view(ui, rt));

        // Request sub tabs
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let np = rt.req.params.iter().filter(|k| k.enabled && !k.key.is_empty()).count();
            let nh = rt.req.headers.iter().filter(|k| k.enabled && !k.key.is_empty()).count();
            let nt = rt.req.checks.len();
            let has_auth = !matches!(rt.req.auth.kind, AuthKind::Inherit | AuthKind::None);
            let has_scripts = !rt.req.scripts.is_empty();
            let tabs: [(ReqSub, &str, Option<String>); 7] = [
                (ReqSub::Params, "Params", (np > 0).then(|| "●".to_string())),
                (ReqSub::Auth, "Authorization", has_auth.then(|| "●".to_string())),
                (ReqSub::Headers, "Headers", (nh > 0).then(|| format!("({})", nh))),
                (ReqSub::Body, "Body", (rt.req.body.mode != BodyMode::None).then(|| "●".to_string())),
                (ReqSub::Scripts, "Scripts", has_scripts.then(|| "●".to_string())),
                (ReqSub::Tests, "Tests", (nt > 0).then(|| format!("({})", nt))),
                (ReqSub::Docs, "Docs", None),
            ];
            for (s, label, badge) in tabs {
                if tab_label(ui, rt.sub == s, label, badge, dark).clicked() {
                    rt.sub = s;
                }
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt("reqscroll").auto_shrink([false, false]).show(ui, |ui| match rt.sub {
            ReqSub::Params => {
                if kv_section(ui, "params", "Query Params", &mut rt.req.params, &mut rt.bulk_params, &vars, dark) {
                    rt.req.url = url_with_params(&rt.req.url, &rt.req.params);
                }
            }
            ReqSub::Headers => {
                kv_section(ui, "headers", "Headers", &mut rt.req.headers, &mut rt.bulk_headers, &vars, dark);
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Content-Type and Authorization headers are added automatically from the Body and Authorization tabs. Collection headers are sent too, unless one here has the same name.")
                        .small()
                        .color(p.weak),
                );
            }
            ReqSub::Auth => {
                auth_editor(ui, &mut rt.req.auth, true, &vars, dark);
                if rt.req.auth.kind == AuthKind::Inherit {
                    let chain = self.auth_chain_for(rt.origin.as_deref(), &rt.req.id);
                    let eff = http::effective_auth(&rt.req, &chain);
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("This request will use: {}", eff.kind.label())).color(p.weak));
                }
            }
            ReqSub::Body => body_editor(ui, &mut rt.req.body, &vars, dark),
            ReqSub::Scripts => {
                scripts_editor(ui, &mut rt.req.scripts, &vars, dark);
            }
            ReqSub::Tests => tests_editor(ui, &mut rt.req.checks, dark),
            ReqSub::Docs => {
                ui.label(RichText::new("Description").color(p.weak));
                ui.add(
                    egui::TextEdit::multiline(&mut rt.req.description)
                        .hint_text("Describe what this request does…")
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
            }
        });
    }

    pub(super) fn response_view(&mut self, ui: &mut Ui, rt: &mut ReqTab) {
        let p = self.pal();
        let dark = self.dark();
        let r = ui.max_rect();
        ui.painter().line_segment(
            [Pos2::new(r.left() - 16.0, r.top() - 8.0), Pos2::new(r.right() + 16.0, r.top() - 8.0)],
            Stroke::new(1.0, p.border),
        );
        if rt.rx.is_some() {
            ui.label(RichText::new("Response").strong());
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.label(RichText::new("Sending request…").color(p.weak));
            });
            ui.ctx().request_repaint();
            return;
        }
        let Some(resp) = &rt.resp else {
            ui.label(RichText::new("Response").strong());
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                let (rr, _) = ui.allocate_exact_size(Vec2::new(140.0, 80.0), Sense::hover());
                let pt = ui.painter();
                let card = Rect::from_center_size(rr.center() + Vec2::new(-10.0, 6.0), Vec2::new(78.0, 52.0));
                pt.rect_filled(card, 6.0, p.hover);
                pt.rect_stroke(card, 6.0, Stroke::new(1.5, p.border), egui::StrokeKind::Middle);
                pt.circle_filled(rr.center() + Vec2::new(36.0, -22.0), 13.0, p.accent.gamma_multiply(0.85));
                for (k, len) in [(0.0, 50.0), (11.0, 38.0), (22.0, 26.0)] {
                    let y = card.top() + 13.0 + k;
                    pt.line_segment([Pos2::new(card.left() + 12.0, y), Pos2::new(card.left() + 12.0 + len, y)], Stroke::new(3.0, p.border));
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Enter the URL and click Send to get a response").color(p.weak));
            });
            return;
        };
        if let Some(e) = &resp.error {
            ui.label(RichText::new("Response").strong());
            ui.add_space(10.0);
            egui::Frame::new().fill(p.err.gamma_multiply(0.08)).corner_radius(6).inner_margin(14).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Could not send request").strong().color(p.err));
                ui.label(e);
                ui.label(RichText::new(&resp.final_url).small().monospace().color(p.weak));
            });
            return;
        }
        let passed = resp.tests.iter().filter(|t| t.1).count();
        if let Some(e) = &resp.script_error {
            egui::Frame::new().fill(p.err.gamma_multiply(0.08)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("A post-response script failed: {}", e)).color(p.err));
            });
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if tab_label(ui, rt.resp_view == RespView::Body, "Body", None, dark).clicked() {
                rt.resp_view = RespView::Body;
            }
            if tab_label(ui, rt.resp_view == RespView::Headers, "Headers", Some(format!("({})", resp.headers.len())), dark).clicked() {
                rt.resp_view = RespView::Headers;
            }
            let tb = if resp.tests.is_empty() { None } else { Some(format!("({}/{})", passed, resp.tests.len())) };
            if tab_label(ui, rt.resp_view == RespView::Tests, "Test Results", tb, dark).clicked() {
                rt.resp_view = RespView::Tests;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(human_size(resp.size)).color(p.ok));
                ui.label(RichText::new("Size:").color(p.weak));
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{} ms", resp.time_ms)).color(p.ok));
                ui.label(RichText::new("Time:").color(p.weak));
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{} {}", resp.status, resp.status_text)).strong().color(status_color(resp.status, dark)));
                ui.label(RichText::new("Status:").color(p.weak));
            });
        });
        ui.add_space(4.0);
        match rt.resp_view {
            RespView::Body => {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Pretty").selected(rt.pretty)).clicked() {
                        rt.pretty = true;
                    }
                    if ui.add(egui::Button::new("Raw").selected(!rt.pretty)).clicked() {
                        rt.pretty = false;
                    }
                    let ct = resp.header("content-type").unwrap_or("");
                    let kind = if resp.is_json() {
                        "JSON"
                    } else if ct.contains("html") {
                        "HTML"
                    } else if ct.contains("xml") {
                        "XML"
                    } else {
                        "Text"
                    };
                    ui.label(RichText::new(kind).small().color(p.send));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("Copy").clicked() {
                            ui.ctx().copy_text(resp.pretty.clone().unwrap_or_else(|| resp.body.clone()));
                        }
                    });
                });
                let text: &str = if rt.pretty { resp.pretty.as_deref().unwrap_or(&resp.body) } else { &resp.body };
                let is_json = rt.pretty && resp.is_json() && text.len() < 2_000_000;
                let mut picked = None;
                egui::Frame::new().fill(p.header).corner_radius(4).inner_margin(8).show(ui, |ui| {
                    egui::ScrollArea::both().id_salt("respbody").auto_shrink([false, false]).show(ui, |ui| {
                        let id = Id::unique(("respbody", rt.req.id.as_str(), rt.pretty));
                        // Pressing the right button moves the cursor, so remember the selection from before.
                        let focused = ui.memory(|m| m.has_focus(id));
                        let before = egui::text_edit::TextEditState::load(ui.ctx(), id).and_then(|s| s.cursor.char_range());
                        let right = ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary) || i.pointer.secondary_down());
                        let mut t = text;
                        let mut lay = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                            ui.fonts_mut(|f| f.layout_job(json_job(buf.as_str(), dark, wrap)))
                        };
                        let edit = egui::TextEdit::multiline(&mut t).id(id).frame(egui::Frame::NONE).desired_width(f32::INFINITY);
                        let edit = if is_json { edit.layouter(&mut lay) } else { edit.font(egui::TextStyle::Monospace) };
                        let out = edit.show(ui);
                        let resp = out.response.response;
                        if right && resp.hovered() {
                            if let Some(range) = before.filter(|_| focused) {
                                let mut state = out.state;
                                state.cursor.set_char_range(Some(range));
                                state.store(ui.ctx(), id);
                            }
                            if ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
                                rt.resp_pick = before.filter(|_| focused).map(|r| r.slice_str(text).to_string()).unwrap_or_default();
                                rt.new_var.clear();
                            }
                        }
                        // Clicks inside stay open, so a new variable's name can be typed.
                        egui::Popup::context_menu(&resp)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| picked = self.set_variable_menu(ui, rt.origin.as_deref(), &rt.resp_pick, &mut rt.new_var));
                    });
                });
                if let Some((scope, key)) = picked {
                    let now = ui.ctx().input(|i| i.time);
                    self.set_variable(rt.origin.as_deref(), scope, &key, rt.resp_pick.clone(), now);
                }
            }
            RespView::Headers => {
                egui::ScrollArea::vertical().id_salt("respheaders").auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("rh").striped(true).num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                        ui.label(RichText::new("Key").color(p.weak));
                        ui.label(RichText::new("Value").color(p.weak));
                        ui.end_row();
                        for (k, v) in &resp.headers {
                            ui.label(RichText::new(k).strong());
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                });
            }
            RespView::Tests => {
                if resp.tests.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new("No tests for this request. Add checks in the Tests tab above.").color(p.weak));
                } else {
                    ui.label(RichText::new(format!("{} of {} passed", passed, resp.tests.len())).strong());
                    egui::ScrollArea::vertical().id_salt("resptests").show(ui, |ui| {
                        for (name, ok, detail) in &resp.tests {
                            ui.horizontal(|ui| {
                                let (bg, txt) = if *ok { (p.ok, "PASS") } else { (p.err, "FAIL") };
                                egui::Frame::new().fill(bg).corner_radius(3).inner_margin(Margin::symmetric(6, 1)).show(ui, |ui| {
                                    ui.label(RichText::new(txt).small().strong().color(Color32::WHITE));
                                });
                                ui.label(name);
                                if !ok && !detail.is_empty() {
                                    ui.label(RichText::new(detail).small().color(p.weak));
                                }
                            });
                        }
                    });
                }
            }
        }
    }

    /// The right-click menu on selected response text: copy it, or keep it in a variable.
    fn set_variable_menu(&self, ui: &mut Ui, coll: Option<&str>, picked: &str, new_var: &mut String) -> Option<(VarScope, String)> {
        let p = self.pal();
        if picked.is_empty() {
            ui.label(RichText::new("Select some text first, then right-click it.").color(p.weak));
            return None;
        }
        ui.add(egui::Label::new(RichText::new(preview(picked)).monospace().small().color(p.weak)).truncate());
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(picked.to_string());
            ui.close();
        }
        ui.separator();
        let scopes = self.scopes_for(coll);
        let mut out = None;
        match self.active_env() {
            Some(e) => {
                ui.menu_button(format!("Set in environment “{}”", e.name), |ui| {
                    out = variable_list(ui, &scopes.environment, new_var).map(|k| (VarScope::Environment, k));
                });
            }
            None => {
                ui.add_enabled(false, egui::Button::new("Set in environment"))
                    .on_disabled_hover_text("No environment is active. Choose one at the top right first.");
            }
        }
        if let Some(c) = coll.and_then(|c| self.coll(c)) {
            ui.menu_button(format!("Set in collection “{}”", c.name), |ui| {
                out = variable_list(ui, &scopes.collection, new_var).map(|k| (VarScope::Collection, k));
            });
        }
        ui.menu_button("Set in globals", |ui| {
            out = variable_list(ui, &scopes.globals, new_var).map(|k| (VarScope::Globals, k));
        });
        if out.is_some() {
            ui.close();
        }
        out
    }

    /// Keep a value picked from a response. Like values set by scripts, environment and collection
    /// values stay on this computer so a token copied from a response never reaches the team.
    fn set_variable(&mut self, coll: Option<&str>, scope: VarScope, key: &str, value: String, now: f64) {
        let entry = vec![(key.to_string(), Some(value))];
        let (changes, place) = match scope {
            VarScope::Environment => {
                let name = self.active_env().map(|e| e.name.clone()).unwrap_or_default();
                (Changes { environment: entry, ..Default::default() }, format!("environment “{}”", name))
            }
            VarScope::Collection => {
                let name = coll.and_then(|c| self.coll(c)).map(|c| c.name.clone()).unwrap_or_default();
                (Changes { collection: entry, ..Default::default() }, format!("collection “{}”", name))
            }
            VarScope::Globals => (Changes { globals: entry, ..Default::default() }, "globals".to_string()),
        };
        self.apply_changes(coll, &changes);
        self.console.log(console::LogLevel::Info, format!("Set {{{{{}}}}} in {} from the response", key, place));
        self.toast(format!("Set {{{{{}}}}} in {}", key, place), true, now);
    }
}

#[derive(Clone, Copy)]
enum VarScope {
    Environment,
    Collection,
    Globals,
}

/// The variables of one scope to choose from, and a field for a new one. Returns the chosen name.
fn variable_list(ui: &mut Ui, values: &BTreeMap<String, String>, new_var: &mut String) -> Option<String> {
    let mut out = None;
    if values.is_empty() {
        ui.label(RichText::new("No variables yet").small().weak());
    }
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        for (key, value) in values {
            if ui.button(RichText::new(key).monospace()).on_hover_text(format!("Now: {}", preview(value))).clicked() {
                out = Some(key.clone());
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(new_var).hint_text("New variable name").desired_width(150.0));
        let name = new_var.trim().to_string();
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.add_enabled(!name.is_empty(), egui::Button::new("Set")).clicked() || enter) && !name.is_empty() {
            out = Some(name);
        }
    });
    out
}

/// A short, one-line version of a value for menus.
fn preview(s: &str) -> String {
    let line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 60 { format!("{}…", line.chars().take(60).collect::<String>()) } else { line }
}
