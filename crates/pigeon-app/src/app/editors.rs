//! Editors shared by several pages: scripts, authorization, body and tests.

use super::*;
use eframe::egui::{RichText, Vec2};

const PRE_SNIPPETS: [(&str, &str); 5] = [
    ("Set an environment variable", "pm.environment.set(\"name\", \"value\");"),
    ("Set a variable for this run", "pm.variables.set(\"name\", \"value\");"),
    ("Add a header", "pm.request.headers.upsert({ key: \"X-Example\", value: \"value\" });"),
    ("Add a timestamp variable", "pm.variables.set(\"timestamp\", Date.now());"),
    ("Send another request first", "const res = await pm.sendRequest(\"https://example.com\");\nconsole.log(res.code);"),
];

const POST_SNIPPETS: [(&str, &str); 6] = [
    ("Status code is 200", "pm.test(\"Status code is 200\", () => {\n  pm.response.to.have.status(200);\n});"),
    (
        "Response time is below 500 ms",
        "pm.test(\"Response time is below 500 ms\", () => {\n  pm.expect(pm.response.responseTime).to.be.below(500);\n});",
    ),
    ("Body contains text", "pm.test(\"Body contains text\", () => {\n  pm.expect(pm.response.text()).to.include(\"text\");\n});"),
    (
        "Check a JSON value",
        "pm.test(\"JSON value is right\", () => {\n  const data = pm.response.json();\n  pm.expect(data.id).to.eql(1);\n});",
    ),
    ("Save a value from the response", "const data = pm.response.json();\npm.environment.set(\"token\", data.token);"),
    ("Log the response", "console.log(pm.response.json());"),
];

/// Editor with Pre-request and Post-response panes. Returns true if a script changed.
pub(super) fn scripts_editor(ui: &mut Ui, scripts: &mut Scripts, vars: &Vars, dark: bool) -> bool {
    let p = palette(dark);
    let pane_id = ui.scope_id().with("script_pane");
    let only_post = scripts.pre.trim().is_empty() && !scripts.post.trim().is_empty();
    let mut pane = ui.data(|d| d.get_temp::<usize>(pane_id)).unwrap_or(usize::from(only_post));
    let mut changed = false;
    let dot = |code: &str| (!code.trim().is_empty()).then(|| "●".to_string());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if tab_label(ui, pane == 0, "Pre-request", dot(&scripts.pre), dark).clicked() {
            pane = 0;
        }
        if tab_label(ui, pane == 1, "Post-response", dot(&scripts.post), dark).clicked() {
            pane = 1;
        }
    });
    ui.data_mut(|d| d.insert_temp(pane_id, pane));
    ui.add_space(4.0);
    let (code, hint, snippets): (&mut String, &str, &[(&str, &str)]) = if pane == 0 {
        (&mut scripts.pre, "// JavaScript that runs before the request is sent", &PRE_SNIPPETS)
    } else {
        (&mut scripts.post, "// JavaScript that runs after the response arrives", &POST_SNIPPETS)
    };
    let side = 210.0;
    let width = (ui.available_width() - side - 12.0).max(200.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(width);
            egui::Frame::new().stroke(Stroke::new(1.0, p.border)).corner_radius(4).inner_margin(6).show(ui, |ui| {
                let mut lay =
                    |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.fonts_mut(|f| f.layout_job(js_job(buf.as_str(), dark, wrap)));
                let id = ui.scope_id().with(if pane == 0 { "pre-request script" } else { "post-response script" });
                let field = VarEdit { id, vars, dark, trigger: Trigger::Script, multiline: true };
                changed |= field
                    .show(ui, code, &mut lay, |t| {
                        t.code_editor().hint_text(hint).frame(egui::Frame::NONE).desired_rows(14).desired_width(f32::INFINITY)
                    })
                    .changed();
            });
        });
        ui.vertical(|ui| {
            ui.set_width(side);
            ui.label(RichText::new("Snippets").small().strong().color(p.weak));
            for (label, snippet) in snippets {
                if ui.link(RichText::new(*label).small().color(p.send)).clicked() {
                    if !code.is_empty() && !code.ends_with('\n') {
                        code.push('\n');
                    }
                    code.push_str(snippet);
                    code.push('\n');
                    changed = true;
                }
            }
        });
    });
    changed
}

/// Variables scripts have set for an environment or collection on this computer.
pub(super) fn script_values(
    ui: &mut Ui,
    all: &mut BTreeMap<String, BTreeMap<String, Option<String>>>,
    id: &str,
    cleared: &mut bool,
    dark: bool,
) {
    let p = palette(dark);
    let Some(values) = all.get(id).filter(|v| !v.is_empty()) else { return };
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Set on this computer").strong());
        ui.label(
            RichText::new("By scripts or from a response. These are used instead of the values above and are never pushed to your team.")
                .small()
                .color(p.weak),
        );
    });
    ui.add_space(4.0);
    egui::Grid::new("script_values").striped(true).num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
        for (key, value) in values {
            ui.label(RichText::new(key).monospace());
            match value {
                Some(v) => ui.label(RichText::new(v).monospace()),
                None => ui.label(RichText::new("(removed)").small().color(p.weak)),
            };
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    if ui.small_button("Clear these").clicked() {
        all.remove(id);
        *cleared = true;
    }
}

pub(super) fn auth_editor(ui: &mut Ui, auth: &mut Auth, allow_inherit: bool, vars: &Vars, dark: bool) -> bool {
    let p = palette(dark);
    let mut changed = false;
    // A collection has no parent, and one saved without auth means the same as No Auth.
    if !allow_inherit && auth.kind == AuthKind::Inherit {
        auth.kind = AuthKind::None;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Auth Type").color(p.weak));
        egui::ComboBox::from_id_salt("authkind").selected_text(auth.kind.label()).width(200.0).show_ui(ui, |ui| {
            for k in AuthKind::ALL {
                if k == AuthKind::Inherit && !allow_inherit {
                    continue;
                }
                if ui.selectable_value(&mut auth.kind, k, k.label()).changed() {
                    changed = true;
                }
            }
        });
    });
    ui.add_space(6.0);
    let mut field = |ui: &mut Ui, label: &str, v: &mut String, password: bool| {
        ui.horizontal(|ui| {
            ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new(label).color(p.weak)));
            let hide = password && !v.contains("{{");
            let mut layouter = var_layouter(vars, dark, false);
            let id = ui.scope_id().with(label);
            let field = VarEdit { id, vars, dark, trigger: Trigger::Template, multiline: false };
            if field.show(ui, v, &mut layouter, |t| t.desired_width(360.0).password(hide)).changed() {
                changed = true;
            }
        });
    };
    match auth.kind {
        AuthKind::Inherit => {
            ui.label(RichText::new("Uses the authorization of the parent folder or collection.").color(p.weak));
        }
        AuthKind::None => {
            ui.label(RichText::new("No authorization is sent.").color(p.weak));
        }
        AuthKind::Bearer => field(ui, "Token", &mut auth.token, false),
        AuthKind::Basic => {
            field(ui, "Username", &mut auth.username, false);
            field(ui, "Password", &mut auth.password, true);
        }
        AuthKind::ApiKey => {
            field(ui, "Key", &mut auth.key, false);
            field(ui, "Value", &mut auth.value, false);
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 24.0], egui::Label::new(RichText::new("Add to").color(p.weak)));
                if ui.radio_value(&mut auth.in_query, false, "Header").changed() {
                    changed = true;
                }
                if ui.radio_value(&mut auth.in_query, true, "Query Params").changed() {
                    changed = true;
                }
            });
        }
    }
    changed
}

pub(super) fn body_editor(ui: &mut Ui, body: &mut Body, vars: &Vars, dark: bool) {
    let p = palette(dark);
    ui.horizontal(|ui| {
        ui.radio_value(&mut body.mode, BodyMode::None, "none");
        ui.radio_value(&mut body.mode, BodyMode::FormData, "form-data");
        ui.radio_value(&mut body.mode, BodyMode::UrlEncoded, "x-www-form-urlencoded");
        ui.radio_value(&mut body.mode, BodyMode::Raw, "raw");
        if body.mode == BodyMode::Raw {
            egui::ComboBox::from_id_salt("rawlang").selected_text(RichText::new(body.raw_lang.label()).color(p.send)).width(100.0).show_ui(
                ui,
                |ui| {
                    for l in RawLang::ALL {
                        ui.selectable_value(&mut body.raw_lang, l, l.label());
                    }
                },
            );
            if body.raw_lang == RawLang::Json
                && ui.small_button("Beautify").clicked()
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&body.raw)
                && let Ok(s) = serde_json::to_string_pretty(&v)
            {
                body.raw = s;
            }
        }
    });
    ui.add_space(6.0);
    match body.mode {
        BodyMode::None => {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("This request does not have a body").color(p.weak)));
        }
        BodyMode::FormData => {
            kv_table(ui, "formdata", &mut body.formdata, vars, dark, "Key", true);
        }
        BodyMode::UrlEncoded => {
            kv_table(ui, "urlenc", &mut body.urlencoded, vars, dark, "Key", true);
        }
        BodyMode::Raw => {
            let json = body.raw_lang == RawLang::Json;
            let plain_json = json && !body.raw.contains("{{");
            egui::Frame::new().stroke(Stroke::new(1.0, p.border)).corner_radius(4).inner_margin(6).show(ui, |ui| {
                let mut json_colours =
                    |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.fonts_mut(|f| f.layout_job(json_job(buf.as_str(), dark, wrap)));
                let mut variable_colours = var_layouter(vars, dark, true);
                let layouter: &mut Layouter<'_> = if plain_json { &mut json_colours } else { &mut variable_colours };
                let field = VarEdit { id: ui.scope_id().with("raw body"), vars, dark, trigger: Trigger::Template, multiline: true };
                field.show(ui, &mut body.raw, layouter, |t| {
                    t.code_editor().frame(egui::Frame::NONE).desired_rows(10).desired_width(f32::INFINITY)
                });
            });
            if plain_json
                && !body.raw.trim().is_empty()
                && let Err(e) = serde_json::from_str::<serde_json::Value>(&body.raw)
            {
                ui.label(RichText::new(format!("JSON problem: {}", e)).small().color(p.warn));
            }
        }
    }
}

pub(super) fn tests_editor(ui: &mut Ui, checks: &mut Vec<Check>, dark: bool) {
    let p = palette(dark);
    ui.label(RichText::new("Checks run after every response. Results show under Test Results.").small().color(p.weak));
    ui.add_space(4.0);
    let mut remove = None;
    for (i, c) in checks.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut c.enabled, "");
                egui::ComboBox::from_id_salt("checkkind").selected_text(c.kind.label()).width(190.0).show_ui(ui, |ui| {
                    for k in CheckKind::ALL {
                        ui.selectable_value(&mut c.kind, k, k.label());
                    }
                });
                match c.kind {
                    CheckKind::JsonEquals => {
                        ui.add(egui::TextEdit::singleline(&mut c.target).hint_text("path e.g. data[0].id").desired_width(160.0));
                        ui.add(egui::TextEdit::singleline(&mut c.expected).hint_text("expected").desired_width(140.0));
                    }
                    CheckKind::HeaderExists => {
                        ui.add(egui::TextEdit::singleline(&mut c.target).hint_text("Header name").desired_width(200.0));
                    }
                    _ => {
                        ui.add(egui::TextEdit::singleline(&mut c.expected).hint_text("expected").desired_width(200.0));
                    }
                }
                if x_button(ui, Vec2::splat(24.0), dark).on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        checks.remove(i);
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("+ Status is 200").clicked() {
            checks.push(Check { kind: CheckKind::StatusEquals, expected: "200".into(), enabled: true, target: String::new() });
        }
        if ui.button("+ Time below 1000 ms").clicked() {
            checks.push(Check { kind: CheckKind::TimeBelow, expected: "1000".into(), enabled: true, target: String::new() });
        }
        if ui.button("+ Add check").clicked() {
            checks.push(Check { kind: CheckKind::BodyContains, enabled: true, ..Default::default() });
        }
    });
}
