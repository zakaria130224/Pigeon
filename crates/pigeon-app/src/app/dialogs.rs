//! Modal dialogs, toasts, keyboard shortcuts and the screenshot mode.

use super::*;
use eframe::egui::{Align2, Color32, Id, RichText, Vec2};

impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.dialog.take() else { return };
        let p = self.pal();
        let now = ctx.input(|i| i.time);
        let mut keep = true;
        let modal = egui::Modal::new(Id::unique("dialog")).show(ctx, |ui| {
            ui.set_min_width(400.0);
            match &mut d {
                Dialog::Rename { coll, item, name } => {
                    ui.label(RichText::new("Rename").heading());
                    ui.add_space(4.0);
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    if !r.has_focus() && !ui.ctx().memory(|m| m.focused().is_some()) {
                        r.request_focus();
                    }
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if primary_button(ui, "Save", p.send, !name.trim().is_empty()).clicked() || (enter && !name.trim().is_empty()) {
                            let n = name.trim().to_string();
                            let (coll, item) = (coll.clone(), item.clone());
                            self.rename(&coll, item.as_deref(), &n);
                            self.mark_dirty(now);
                            keep = false;
                        }
                    });
                }
                Dialog::SaveAs { tab, name, coll, folder, new_coll } => {
                    ui.label(RichText::new("Save request").heading());
                    ui.add_space(4.0);
                    ui.label("Request name");
                    ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    ui.add_space(6.0);
                    ui.label("Save to");
                    let cur = coll.as_ref().and_then(|c| self.coll(c)).map(|c| c.name.clone()).unwrap_or_else(|| "New collection…".into());
                    egui::ComboBox::from_id_salt("saveas_coll").selected_text(cur).width(360.0).show_ui(ui, |ui| {
                        for c in &self.collections {
                            if ui.selectable_label(coll.as_deref() == Some(c.id.as_str()), &c.name).clicked() {
                                *coll = Some(c.id.clone());
                                *folder = None;
                            }
                        }
                        if ui.selectable_label(coll.is_none(), "New collection…").clicked() {
                            *coll = None;
                        }
                    });
                    match coll.clone() {
                        Some(cid) => {
                            let mut folders = vec![];
                            if let Some(c) = self.coll(&cid) {
                                list_folders(&c.items, "", &mut folders);
                            }
                            if !folders.is_empty() {
                                let fcur = folder
                                    .as_ref()
                                    .and_then(|f| folders.iter().find(|x| &x.0 == f))
                                    .map(|x| x.1.clone())
                                    .unwrap_or_else(|| "(top level)".into());
                                egui::ComboBox::from_id_salt("saveas_folder").selected_text(fcur).width(360.0).show_ui(ui, |ui| {
                                    if ui.selectable_label(folder.is_none(), "(top level)").clicked() {
                                        *folder = None;
                                    }
                                    for (fid, fname) in &folders {
                                        if ui.selectable_label(folder.as_deref() == Some(fid.as_str()), fname).clicked() {
                                            *folder = Some(fid.clone());
                                        }
                                    }
                                });
                            }
                        }
                        None => {
                            ui.add(egui::TextEdit::singleline(new_coll).hint_text("New collection name").desired_width(f32::INFINITY));
                        }
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if primary_button(ui, "Save", p.send, !name.trim().is_empty()).clicked() {
                            let target = match coll.clone() {
                                Some(c) => c,
                                None => {
                                    let c = Collection::new(if new_coll.trim().is_empty() { "New Collection" } else { new_coll.trim() });
                                    let id = c.id.clone();
                                    self.collections.push(c);
                                    id
                                }
                            };
                            self.save_as(*tab, name.trim(), &target, folder.clone());
                            self.mark_dirty(now);
                            self.flush_save();
                            self.start_git(GitOp::Refresh);
                            keep = false;
                        }
                    });
                }
                Dialog::Confirm { text, action } => {
                    ui.label(RichText::new(text.as_str()).strong());
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("If the project is shared, this is removed for your team after you push.").small().color(p.weak),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if primary_button(ui, "Delete", p.err, true).clicked() {
                            self.actions.push(action.clone());
                            keep = false;
                        }
                    });
                }
                Dialog::SwitchProject { path, unsaved } => {
                    let tabs = if *unsaved == 1 { "1 tab has".to_string() } else { format!("{} tabs have", unsaved) };
                    ui.label(RichText::new(format!("{} unsaved changes", tabs)).heading());
                    ui.add_space(4.0);
                    ui.label(format!(
                        "Opening {} closes every tab and loses those changes. Cancel and save them to keep them.",
                        settings::project_name(path)
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if primary_button(ui, "Open and lose changes", p.err, true).clicked() {
                            self.actions.push(Action::SwitchProject(path.clone()));
                            keep = false;
                        }
                    });
                }
                Dialog::CloseUnsaved { tab } => {
                    ui.label(RichText::new("This tab has unsaved changes").heading());
                    ui.label("Do you want to save them before closing?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button("Don't save").clicked() {
                            let t = *tab;
                            self.close_tab(t);
                            keep = false;
                        }
                        if primary_button(ui, "Save changes", p.send, true).clicked() {
                            let t = *tab;
                            self.save_tab(t, now);
                            self.close_tab(t);
                            keep = false;
                        }
                    });
                }
                Dialog::Import { text, error } => {
                    ui.label(RichText::new("Import").heading());
                    ui.label(
                        RichText::new(
                            "Bring in OpenAPI 3 JSON files, collection or environment exports (v2.1 JSON), or paste a cURL command.",
                        )
                        .small()
                        .color(p.weak),
                    );
                    ui.add_space(6.0);
                    if ui.button("Choose files…").clicked()
                        && let Some(files) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_files()
                    {
                        let mut ok = 0;
                        let mut errs = vec![];
                        for f in files {
                            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| formats::import_json(&t)) {
                                Ok(imp) => {
                                    self.add_import(imp);
                                    ok += 1;
                                }
                                Err(e) => errs.push(format!("{}: {}", f.display(), e)),
                            }
                        }
                        if ok > 0 {
                            self.mark_dirty(now);
                            self.toast(format!("Imported {} file(s)", ok), true, now);
                        }
                        if errs.is_empty() {
                            keep = false;
                        } else {
                            *error = errs.join("\n");
                        }
                    }
                    ui.add_space(6.0);
                    ui.label("Or paste raw text");
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(8)
                                .desired_width(f32::INFINITY)
                                .hint_text("curl https://…  or  { \"openapi\": \"3.0.1\", … }"),
                        );
                    });
                    if !error.is_empty() {
                        ui.colored_label(p.err, error.as_str());
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if primary_button(ui, "Import", p.send, !text.trim().is_empty()).clicked() {
                            let t = text.trim().to_string();
                            if t.starts_with("curl") {
                                match http::parse_curl(&t) {
                                    Some(r) => {
                                        self.actions.push(Action::OpenNewRequest(Box::new(r)));
                                        keep = false;
                                    }
                                    None => *error = "Could not read that cURL command.".into(),
                                }
                            } else {
                                match formats::import_json(&t) {
                                    Ok(imp) => {
                                        self.add_import(imp);
                                        self.mark_dirty(now);
                                        self.toast("Imported", true, now);
                                        keep = false;
                                    }
                                    Err(e) => *error = e,
                                }
                            }
                        }
                    });
                }
                Dialog::Settings => {
                    ui.label(RichText::new("Settings").heading());
                    ui.add_space(6.0);
                    let mut ch = false;
                    ch |= ui.checkbox(&mut self.settings.dark, "Dark theme").changed();
                    ch |= ui.checkbox(&mut self.settings.follow_redirects, "Follow redirects").changed();
                    ch |= ui.checkbox(&mut self.settings.pull_on_start, "Pull team changes when the app starts").changed();
                    ui.horizontal(|ui| {
                        ui.label("Request timeout (seconds)");
                        ch |= ui.add(egui::DragValue::new(&mut self.settings.timeout_s).range(1..=600)).changed();
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("Project folder").small().strong());
                    ui.label(RichText::new(self.ws.display().to_string()).small().monospace());
                    if ui.button("Change folder…").clicked() {
                        self.actions.push(Action::ChooseWorkspace);
                    }
                    if ch {
                        self.save_settings();
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("{} {}", pigeon_core::APP_NAME, pigeon_core::VERSION)).small().color(p.weak));
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        keep = false;
                    }
                }
                Dialog::Clone { url, dest } => {
                    ui.label(RichText::new("Clone a team repository").heading());
                    ui.label(
                        RichText::new("Everyone on the team clones the same repository. Pull and push keep it in sync.")
                            .small()
                            .color(p.weak),
                    );
                    ui.add_space(6.0);
                    ui.label("Repository URL");
                    ui.add(
                        egui::TextEdit::singleline(url)
                            .hint_text("https://github.com/team/api-collections.git")
                            .desired_width(f32::INFINITY),
                    );
                    let repo_name =
                        url.trim().trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or("").trim_end_matches(".git").to_string();
                    let base = settings::default_workspace().parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    let suggested = if repo_name.is_empty() { String::new() } else { base.join(&repo_name).display().to_string() };
                    ui.label("Save to folder");
                    ui.horizontal(|ui| {
                        let mut shown = if dest.is_empty() { suggested.clone() } else { dest.clone() };
                        if ui.add(egui::TextEdit::singleline(&mut shown).desired_width(300.0)).changed() {
                            *dest = shown;
                        }
                        if ui.button("Browse…").clicked()
                            && let Some(pf) = rfd::FileDialog::new().pick_folder()
                        {
                            *dest = pf.join(if repo_name.is_empty() { "team-workspace" } else { &repo_name }).display().to_string();
                        }
                    });
                    let target = if dest.is_empty() { suggested } else { dest.clone() };
                    let busy = self.git_busy.is_some();
                    if busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Cloning…");
                        });
                    }
                    if let Some((false, m)) = &self.git_msg {
                        ui.colored_label(p.err, m);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        let can = !url.trim().is_empty() && !target.trim().is_empty() && !busy && self.git_rx.is_none();
                        if primary_button(ui, "Clone", p.send, can).clicked() {
                            let destp = PathBuf::from(target.trim());
                            let non_empty = destp.exists() && std::fs::read_dir(&destp).map(|mut r| r.next().is_some()).unwrap_or(false);
                            if non_empty {
                                self.git_msg = Some((false, "That folder is not empty. Pick a new folder.".into()));
                            } else {
                                self.flush_save();
                                self.git_busy = Some("Clone");
                                self.git_msg = None;
                                self.git_rx = Some(git::spawn(self.ws.clone(), GitOp::Clone { url: url.trim().to_string(), dest: destp }));
                            }
                        }
                    });
                }
                Dialog::Diff { path, text } => {
                    ui.set_min_width(640.0);
                    ui.label(RichText::new(pretty_path(path)).strong());
                    ui.add_space(4.0);
                    egui::ScrollArea::both().max_height(460.0).show(ui, |ui| {
                        for line in text.lines() {
                            let col = if line.starts_with('+') && !line.starts_with("+++") {
                                p.ok
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                p.err
                            } else if line.starts_with("@@") {
                                p.send
                            } else {
                                p.weak
                            };
                            ui.label(RichText::new(line).monospace().color(col));
                        }
                        if text.trim().is_empty() {
                            ui.label("No differences.");
                        }
                    });
                    ui.add_space(6.0);
                    if ui.button("Close").clicked() {
                        keep = false;
                    }
                }
            }
        });
        if modal.should_close() && !matches!(d, Dialog::Clone { .. }) {
            keep = false;
        }
        if keep && self.dialog.is_none() {
            self.dialog = Some(d);
        }
    }

    fn rename(&mut self, coll: &str, item: Option<&str>, n: &str) {
        match item {
            None => {
                if let Some(c) = self.coll_mut(coll) {
                    c.name = n.to_string();
                }
            }
            Some(id) => {
                if let Some(c) = self.coll_mut(coll)
                    && let Some((list, i)) = find_parent_list_mut(&mut c.items, id)
                {
                    match &mut list[i] {
                        Item::Request(r) => r.name = n.to_string(),
                        Item::Folder(f) => f.name = n.to_string(),
                    }
                }
                for t in &mut self.tabs {
                    if let TabKind::Request(rt) = &mut t.kind
                        && rt.req.id == id
                    {
                        rt.req.name = n.to_string();
                        if let Some(s) = &mut rt.saved {
                            s.name = n.to_string();
                        }
                    }
                }
            }
        }
    }

    fn save_as(&mut self, tab: usize, name: &str, target: &str, folder: Option<String>) {
        let Some(Tab { kind: TabKind::Request(rt), .. }) = self.tabs.get_mut(tab) else { return };
        rt.req.name = name.to_string();
        if rt.origin.is_some() {
            // Saving a copy of an existing request.
            rt.req.id = new_id();
        }
        let req = clean_request(&rt.req);
        rt.req = req.clone();
        rt.saved = Some(req.clone());
        rt.origin = Some(target.to_string());
        if let Some(c) = self.collections.iter_mut().find(|c| c.id == target) {
            let list = match &folder {
                Some(f) => find_folder_mut(&mut c.items, f).map(|f| &mut f.items),
                None => Some(&mut c.items),
            };
            if let Some(l) = list {
                l.push(Item::Request(req));
            }
        }
        self.expanded.insert(target.to_string());
        if let Some(f) = folder {
            self.expanded.insert(f);
        }
    }

    fn add_import(&mut self, imp: formats::Imported) {
        match imp {
            formats::Imported::Collection(c) => {
                self.expanded.insert(c.id.clone());
                self.collections.push(c);
                self.side = SideView::Collections;
            }
            formats::Imported::Environment(e) => {
                self.envs.push(e);
                self.side = SideView::Environments;
            }
        }
    }

    pub(super) fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        for t in &mut self.toasts {
            if t.2 > now + 1e6 {
                t.2 = now + 8.0;
            }
        }
        self.toasts.retain(|t| t.2 > now);
        if self.toasts.is_empty() {
            return;
        }
        let p = self.pal();
        egui::Area::new(Id::unique("toasts")).anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -40.0)).show(ctx, |ui| {
            for (text, ok, _) in &self.toasts {
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0, if *ok { p.ok } else { p.err }))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(12, 8))
                    .shadow(egui::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(40) })
                    .show(ui, |ui| {
                        ui.set_max_width(360.0);
                        ui.label(RichText::new(text).color(if *ok { p.text } else { p.err }));
                    });
                ui.add_space(6.0);
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        let console = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::C));
        if console {
            self.console.open = !self.console.open;
        }
        let (save, newtab, close) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::W),
            )
        });
        if save {
            self.actions.push(Action::SaveTab(self.active));
        }
        if newtab {
            self.actions.push(Action::NewTab);
        }
        if close && !self.tabs.is_empty() {
            self.actions.push(Action::CloseTab(self.active));
        }
    }
}
