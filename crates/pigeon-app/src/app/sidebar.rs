//! Header bar, icon rail, sidebar panels and status bar.

use super::*;
use eframe::egui::{Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Vec2};
use std::path::Path;

impl App {
    pub(super) fn header_bar(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::from_texture(egui::load::SizedTexture::from_handle(logo)).fit_to_exact_size(Vec2::splat(24.0)));
            }
            ui.add_space(4.0);
            if ui.add(egui::Button::new("Home").frame(false)).clicked() {
                self.side = SideView::Collections;
            }
            ui.menu_button("Projects ▾", |ui| self.project_menu(ui));
            if ui.add(egui::Button::new("Import").frame(false)).clicked() {
                self.actions.push(Action::Import);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(6.0);
                if icon_button(ui, "gear", "", dark, "Settings").clicked() {
                    self.dialog = Some(Dialog::Settings);
                }
                if icon_button(ui, if dark { "sun" } else { "moon" }, "", dark, "Switch light / dark").clicked() {
                    self.settings.dark = !self.settings.dark;
                    self.save_settings();
                }
                ui.separator();
                let busy = self.git_busy.is_some();
                let repo = self.gstat.is_repo;
                let pending = self.gstat.ahead as usize + usize::from(!self.gstat.changes.is_empty());
                let push_label = if pending > 0 { format!("Push ({})", pending) } else { "Push".to_string() };
                let pull_label = if self.gstat.behind > 0 { format!("Pull ({})", self.gstat.behind) } else { "Pull".to_string() };
                ui.add_enabled_ui(!busy, |ui| {
                    if icon_button(ui, "push", &push_label, dark, "Commit your changes and push them to the team repository").clicked() {
                        if repo {
                            self.actions.push(Action::Git(GitOp::Push(self.commit_msg.clone())));
                        } else {
                            self.side = SideView::Git;
                        }
                    }
                    if icon_button(ui, "pull", &pull_label, dark, "Get the latest collections from the team repository").clicked() {
                        if repo {
                            self.actions.push(Action::Git(GitOp::Pull(String::new())));
                        } else {
                            self.side = SideView::Git;
                        }
                    }
                });
                if let Some(op) = self.git_busy {
                    ui.spinner();
                    ui.label(RichText::new(format!("{}…", op)).color(p.weak));
                }
                if repo {
                    let b = if self.gstat.branch.is_empty() { "no branch".to_string() } else { self.gstat.branch.clone() };
                    let resp = ui.add(egui::Button::new(RichText::new(format!("⎇ {}", b)).color(p.weak)).frame(false));
                    if resp.on_hover_text("Open team sync").clicked() {
                        self.side = SideView::Git;
                    }
                } else if ui.add(egui::Button::new(RichText::new("Set up team sync").color(p.accent)).frame(false)).clicked() {
                    self.side = SideView::Git;
                }

                // search in the middle
                let w = (ui.available_width() - 60.0).clamp(120.0, 420.0);
                let pad = ((ui.available_width() - w) / 2.0 - 40.0).max(0.0);
                ui.add_space(pad);
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter).hint_text("Search collections").desired_width(w).background_color(p.input),
                );
                let r = ui.min_rect();
                let _ = r;
            });
        });
    }

    pub(super) fn rail(&mut self, ui: &mut Ui) {
        let p = self.pal();
        ui.add_space(6.0);
        for (view, icon, label) in [
            (SideView::Collections, "collections", "Collections"),
            (SideView::Environments, "environments", "Environments"),
            (SideView::History, "history", "History"),
            (SideView::Git, "git", "Team Sync"),
        ] {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 54.0), Sense::click());
            let sel = self.side == view;
            if sel {
                ui.painter().rect_filled(rect.shrink2(Vec2::new(5.0, 2.0)), 6.0, p.selected);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect.shrink2(Vec2::new(5.0, 2.0)), 6.0, p.hover);
            }
            let ic = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 18.0), Vec2::splat(18.0));
            paint_icon(ui, ic, icon, if sel { p.text } else { p.weak });
            ui.painter().text(
                Pos2::new(rect.center().x, rect.top() + 39.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                if sel { p.text } else { p.weak },
            );
            if view == SideView::Git && (!self.gstat.changes.is_empty() || self.gstat.behind > 0) {
                let c = if self.gstat.conflicts.is_empty() { p.accent } else { p.err };
                ui.painter().circle_filled(ic.right_top() + Vec2::new(2.0, 0.0), 3.5, c);
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.side = view;
            }
        }
    }

    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            // New and Import sit to the right, so the name gets what they leave.
            let room = ((ui.available_width() - 140.0) / 7.2).max(8.0) as usize;
            let title = format!("{} ▾", elide_end(&settings::project_name(&self.ws), room));
            let button = egui::Button::new(RichText::new(title).strong()).frame(false);
            let (resp, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| self.project_menu(ui));
            resp.on_hover_text(format!("{}\nClick to switch projects", self.ws.display()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Import").clicked() {
                    self.actions.push(Action::Import);
                }
                ui.menu_button("New", |ui| {
                    if ui.button("HTTP Request").clicked() {
                        self.actions.push(Action::NewTab);
                        ui.close();
                    }
                    if ui.button("Collection").clicked() {
                        self.actions.push(Action::NewCollection);
                        ui.close();
                    }
                    if ui.button("Environment").clicked() {
                        self.actions.push(Action::NewEnv);
                        ui.close();
                    }
                });
            });
        });
        ui.add_space(4.0);
        let r = ui.available_rect_before_wrap();
        ui.painter().line_segment([Pos2::new(r.left() - 8.0, r.top()), Pos2::new(r.right() + 8.0, r.top())], Stroke::new(1.0, p.border));
        ui.add_space(6.0);
        if !self.load_errors.is_empty() {
            egui::Frame::new().fill(p.err.gamma_multiply(0.12)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                ui.label(
                    RichText::new("Some files could not be read (often a git conflict). Saving is paused until they are fixed.")
                        .color(p.err)
                        .small(),
                );
                for e in self.load_errors.iter().take(3) {
                    ui.label(RichText::new(e).small().color(p.weak));
                }
                if ui.small_button("Open Team Sync").clicked() {
                    self.side = SideView::Git;
                }
            });
        }
        match self.side {
            SideView::Collections => self.side_collections(ui, dark),
            SideView::Environments => self.side_envs(ui, dark),
            SideView::History => self.side_history(ui, dark),
            SideView::Git => self.side_git(ui, dark),
        }
    }

    fn side_collections(&mut self, ui: &mut Ui, dark: bool) {
        let p = palette(dark);
        ui.horizontal(|ui| {
            if icon_button(ui, "plus", "", dark, "New collection").clicked() {
                self.actions.push(Action::NewCollection);
            }
            ui.label(RichText::new("Collections").color(p.weak).small());
        });
        let active_req = match self.tabs.get(self.active).map(|t| &t.kind) {
            Some(TabKind::Request(rt)) => Some(rt.req.id.clone()),
            _ => None,
        };
        let filter = self.filter.to_lowercase();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.collections.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Create a collection for your requests").color(p.weak));
                    ui.add_space(6.0);
                    if ui.button("Create Collection").clicked() {
                        self.actions.push(Action::NewCollection);
                    }
                });
            }
            let mut actions = vec![];
            for c in &self.collections {
                if !filter.is_empty() && !c.name.to_lowercase().contains(&filter) && !items_match(&c.items, &filter) {
                    continue;
                }
                let open = self.expanded.contains(&c.id) || !filter.is_empty();
                let resp = tree_row(ui, 0, false, dark, |ui| {
                    caret(ui, open, p.weak);
                    ui.label(RichText::new(&c.name).strong());
                });
                if resp.clicked() {
                    if open {
                        self.expanded.remove(&c.id);
                    } else {
                        self.expanded.insert(c.id.clone());
                    }
                }
                if resp.double_clicked() {
                    actions.push(Action::OpenCollection(c.id.clone()));
                }
                resp.context_menu(|ui| collection_menu(ui, &c.id, &mut actions));
                if open {
                    tree_items(ui, &c.items, &c.id, 1, &mut self.expanded, active_req.as_deref(), &filter, dark, &mut actions);
                    if c.items.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(34.0);
                            if ui.link(RichText::new("Add a request").small()).clicked() {
                                actions.push(Action::AddRequest { coll: c.id.clone(), parent: None });
                            }
                        });
                    }
                }
            }
            self.actions.extend(actions);
        });
    }

    fn side_envs(&mut self, ui: &mut Ui, dark: bool) {
        let p = palette(dark);
        ui.horizontal(|ui| {
            if icon_button(ui, "plus", "", dark, "New environment").clicked() {
                self.actions.push(Action::NewEnv);
            }
            ui.label(RichText::new("Environments").color(p.weak).small());
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let r = tree_row(ui, 0, false, dark, |ui| {
                let (rr, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                paint_icon(ui, rr, "environments", p.weak);
                ui.label("Globals");
            });
            if r.clicked() {
                self.actions.push(Action::OpenGlobals);
            }
            ui.add_space(4.0);
            let active = self.settings.active_env.clone();
            for e in &self.envs {
                let is_active = active.as_deref() == Some(e.id.as_str());
                let r = tree_row(ui, 0, false, dark, |ui| {
                    let (rr, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    if is_active {
                        ui.painter().circle_filled(rr.center(), 4.5, p.ok);
                    } else {
                        ui.painter().circle_stroke(rr.center(), 4.5, Stroke::new(1.0, p.weak));
                    }
                    ui.label(&e.name);
                    ui.label(RichText::new(format!("{} vars", e.values.len())).small().color(p.weak));
                });
                if r.clicked() {
                    self.actions.push(Action::OpenEnv(e.id.clone()));
                }
                let id = e.id.clone();
                r.context_menu(|ui| {
                    if ui.button(if is_active { "Deactivate" } else { "Set active" }).clicked() {
                        self.actions.push(Action::SetActiveEnv(if is_active { None } else { Some(id.clone()) }));
                        ui.close();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.actions.push(Action::DuplicateEnv(id.clone()));
                        ui.close();
                    }
                    if ui.button("Export…").clicked() {
                        self.actions.push(Action::ExportEnv(id.clone()));
                        ui.close();
                    }
                    if ui.button(RichText::new("Delete").color(p.err)).clicked() {
                        self.actions.push(Action::DeleteEnv(id.clone()));
                        ui.close();
                    }
                });
            }
            if self.envs.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Environments let you switch between sets of variables, like dev and production.").color(p.weak),
                    );
                    if ui.button("Create Environment").clicked() {
                        self.actions.push(Action::NewEnv);
                    }
                });
            }
        });
    }

    fn side_history(&mut self, ui: &mut Ui, dark: bool) {
        let p = palette(dark);
        ui.horizontal(|ui| {
            ui.label(RichText::new("History").color(p.weak).small());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.history.is_empty() && ui.small_button("Clear all").clicked() {
                    self.history.clear();
                    if !self.no_disk {
                        settings::save_history(&self.history);
                    }
                }
            });
        });
        let filter = self.filter.to_lowercase();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.history.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("Requests you send will appear here.").color(p.weak)));
            }
            let now = now_ms();
            let mut last_group = String::new();
            for (i, h) in self.history.iter().enumerate() {
                if !filter.is_empty() && !h.request.url.to_lowercase().contains(&filter) && !h.request.name.to_lowercase().contains(&filter)
                {
                    continue;
                }
                let group = match now.saturating_sub(h.at_ms) / 86_400_000 {
                    0 => "Today".to_string(),
                    1 => "Yesterday".to_string(),
                    n => format!("{} days ago", n),
                };
                if group != last_group {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&group).small().strong().color(p.weak));
                    last_group = group;
                }
                let r = ui.push_id(i, |ui| {
                    tree_row(ui, 0, false, dark, |ui| {
                        let (mr, _) = ui.allocate_exact_size(Vec2::new(34.0, 16.0), Sense::hover());
                        ui.painter().text(
                            mr.right_center(),
                            Align2::RIGHT_CENTER,
                            method_short(&h.request.method),
                            FontId::proportional(10.0),
                            method_color(&h.request.method, dark),
                        );
                        let mut u = h.request.url.clone();
                        if u.chars().count() > 42 {
                            u = u.chars().take(42).collect::<String>() + "…";
                        }
                        ui.label(u);
                    })
                });
                if r.inner.clicked() {
                    self.actions.push(Action::OpenNewRequest(Box::new(h.request.clone())));
                }
            }
        });
    }

    /// The switcher behind both "Projects" in the top bar and the project's name in the sidebar.
    fn project_menu(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        ui.set_width(300.0);
        ui.label(RichText::new("Recent projects").small().color(p.weak));
        for path in self.settings.recent_projects.clone() {
            let current = path == self.ws;
            let resp = project_row(ui, &path, current, dark);
            let mut forget = false;
            if !current && ui.rect_contains_pointer(resp.rect) {
                let spot = Rect::from_center_size(Pos2::new(resp.rect.right() - 16.0, resp.rect.center().y), Vec2::splat(22.0));
                let x = ui.scope_builder(egui::UiBuilder::new().max_rect(spot), |ui| x_button(ui, Vec2::splat(22.0), dark)).inner;
                forget = x.on_hover_text("Remove from this list. The folder stays where it is.").clicked();
            }
            if forget {
                self.actions.push(Action::ForgetProject(path));
                ui.close();
            } else if resp.clicked() {
                self.actions.push(Action::OpenProject(path));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Open project folder…").clicked() {
            self.actions.push(Action::ChooseWorkspace);
            ui.close();
        }
        if ui.button("Clone a team repository…").clicked() {
            self.dialog = Some(Dialog::Clone { url: String::new(), dest: String::new() });
            ui.close();
        }
        if ui.button("Reload from disk").on_hover_text("Read this project's files again").clicked() {
            self.actions.push(Action::ReloadProject);
            ui.close();
        }
        if ui.button("Team Sync").clicked() {
            self.side = SideView::Git;
            ui.close();
        }
    }

    fn side_git(&mut self, ui: &mut Ui, dark: bool) {
        let p = palette(dark);
        let busy = self.git_busy.is_some();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Team Sync").color(p.weak).small()).on_hover_text(
                "Collections and environments are saved as files in a git repository. Pull to get your team's changes, push to share yours.",
            );
            if self.gstat.is_repo {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let more = icon_button(ui, "more", "", dark, "More git actions");
                    egui::Popup::menu(&more).show(|ui| {
                        let can_commit = !busy && !self.gstat.changes.is_empty();
                        let commit = ui.add_enabled(can_commit, egui::Button::new("Commit only"));
                        if commit.on_hover_text("Save a commit on this computer without pushing").clicked() {
                            self.actions.push(Action::Git(GitOp::Commit(self.commit_msg.clone())));
                            ui.close();
                        }
                        if ui.button("Refresh").clicked() {
                            self.actions.push(Action::Git(GitOp::Refresh));
                            ui.close();
                        }
                    });
                });
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !self.gstat.git_found && !self.no_disk {
                ui.colored_label(p.err, "Git is not installed on this computer. Install git to share collections with your team.");
                return;
            }
            // Successes show as a toast. Only a problem stays on screen.
            if let Some((false, m)) = &self.git_msg {
                egui::Frame::new().fill(p.err.gamma_multiply(0.12)).corner_radius(4).inner_margin(6).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(m).small().color(p.err));
                });
                ui.add_space(4.0);
            }
            if !self.gstat.is_repo {
                ui.label(RichText::new("This folder isn't shared with git yet.").strong());
                ui.add(
                    egui::TextEdit::singleline(&mut self.init_remote)
                        .hint_text("Team repository URL (optional)")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(2.0);
                if primary_button(ui, "Set up git in this folder", p.send, !busy).clicked() {
                    self.actions.push(Action::Git(GitOp::Init { remote: self.init_remote.clone() }));
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Or, if your team already has a repository:").small().color(p.weak));
                if ui.button("Clone a team repository…").clicked() {
                    self.dialog = Some(Dialog::Clone { url: String::new(), dest: String::new() });
                }
                return;
            }

            ui.horizontal(|ui| {
                let mut counts = vec![];
                if self.gstat.ahead > 0 {
                    counts.push(format!("↑{}", self.gstat.ahead));
                }
                if self.gstat.behind > 0 {
                    counts.push(format!("↓{}", self.gstat.behind));
                }
                let counts = counts.join(" ");
                let room = if counts.is_empty() { 0.0 } else { counts.chars().count() as f32 * 8.0 + 8.0 };
                let width = (ui.available_width() - room - 8.0).max(100.0);
                let cur = self.gstat.branch.clone();
                egui::ComboBox::from_id_salt("branch").selected_text(format!("⎇ {}", cur)).width(width).show_ui(ui, |ui| {
                    for b in self.gstat.branches.clone() {
                        if ui.selectable_label(b == cur, &b).clicked() && b != cur {
                            self.actions.push(Action::Git(GitOp::SwitchBranch(b.clone())));
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_branch).hint_text("new-branch").desired_width(100.0));
                        if ui.small_button("Create").clicked() && !self.new_branch.trim().is_empty() {
                            self.actions.push(Action::Git(GitOp::NewBranch(self.new_branch.trim().to_string())));
                            self.new_branch.clear();
                        }
                    });
                });
                if !counts.is_empty() {
                    ui.label(RichText::new(counts).small().color(p.weak))
                        .on_hover_text(format!("{} commit(s) to push, {} to pull", self.gstat.ahead, self.gstat.behind));
                }
            });
            if self.gstat.remote_url.is_empty() {
                ui.label(RichText::new("Add a remote URL under Repository to share with your team.").small().color(p.weak));
            } else if self.gstat.upstream.is_none() {
                ui.label(RichText::new("This branch isn't pushed yet.").small().color(p.weak));
            }
            ui.add_space(4.0);

            if !self.gstat.conflicts.is_empty() || self.gstat.merging {
                egui::Frame::new().fill(p.err.gamma_multiply(0.10)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Merge in progress").strong().color(p.err));
                    if self.gstat.conflicts.is_empty() {
                        ui.label(RichText::new("All conflicts are resolved. Finish the merge, then push.").small());
                    } else {
                        ui.label(RichText::new("You and a teammate changed the same items. Pick which version to keep:").small());
                    }
                    for (i, (code, path)) in self.gstat.conflicts.clone().into_iter().enumerate() {
                        ui.push_id(i, |ui| {
                            ui.add(egui::Label::new(RichText::new(pretty_path(&path)).monospace().size(11.0)).truncate());
                            ui.horizontal(|ui| {
                                if ui.small_button("Keep mine").clicked() {
                                    self.actions.push(Action::Git(GitOp::KeepMine(vec![(code.clone(), path.clone())])));
                                }
                                if ui.small_button("Keep theirs").clicked() {
                                    self.actions.push(Action::Git(GitOp::KeepTheirs(vec![(code.clone(), path.clone())])));
                                }
                                if ui.small_button("View").clicked() {
                                    self.actions.push(Action::Git(GitOp::Diff(path.clone())));
                                }
                            });
                        });
                    }
                    if !self.gstat.conflicts.is_empty() {
                        ui.horizontal(|ui| {
                            if ui.small_button("Keep all mine").clicked() {
                                self.actions.push(Action::Git(GitOp::KeepMine(self.gstat.conflicts.clone())));
                            }
                            if ui.small_button("Keep all theirs").clicked() {
                                self.actions.push(Action::Git(GitOp::KeepTheirs(self.gstat.conflicts.clone())));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        if primary_button(ui, "Finish merge", p.send, self.gstat.conflicts.is_empty() && !busy).clicked() {
                            self.actions.push(Action::Git(GitOp::FinishMerge));
                        }
                        if ui.button("Abort merge").clicked() {
                            self.actions.push(Action::Git(GitOp::AbortMerge));
                        }
                    });
                });
                ui.add_space(6.0);
            }

            if !self.gstat.changes.is_empty() {
                ui.add(
                    egui::TextEdit::multiline(&mut self.commit_msg)
                        .hint_text("Describe your changes (optional)")
                        .desired_rows(1)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(2.0);
            }
            ui.horizontal(|ui| {
                let w = ((ui.available_width() - 16.0) / 3.0).max(60.0);
                ui.add_enabled_ui(!busy, |ui| {
                    if ui.add_sized([w, 26.0], egui::Button::new("Pull")).on_hover_text("Get your team's latest changes").clicked() {
                        self.actions.push(Action::Git(GitOp::Pull(self.commit_msg.clone())));
                    }
                    if ui
                        .add_sized([w, 26.0], egui::Button::new(RichText::new("Push").color(Color32::WHITE)).fill(p.send))
                        .on_hover_text("Commit your changes and send them to the team")
                        .clicked()
                    {
                        self.actions.push(Action::Git(GitOp::Push(self.commit_msg.clone())));
                    }
                    if ui.add_sized([w, 26.0], egui::Button::new("Sync")).on_hover_text("Pull, then push").clicked() {
                        self.actions.push(Action::Git(GitOp::Sync(self.commit_msg.clone())));
                    }
                });
            });
            ui.add_space(6.0);

            if self.gstat.changes.is_empty() {
                ui.label(RichText::new("Everything is committed.").small().color(p.weak));
            } else {
                ui.label(RichText::new(format!("Changes ({})", self.gstat.changes.len())).small().strong());
            }
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, (code, path)) in self.gstat.changes.clone().into_iter().enumerate() {
                let r = ui.push_id(("chg", i), |ui| {
                    sized_row(ui, 22.0, 0, false, dark, |ui| {
                        let kind = git::describe_code(&code);
                        let col = match kind {
                            "new" | "added" => p.ok,
                            "deleted" | "conflict" => p.err,
                            _ => p.warn,
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new(kind).small().color(col));
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(pretty_path(&path)).small()).truncate().show_tooltip_when_elided(false),
                                );
                            });
                        });
                    })
                });
                if r.inner.on_hover_text(format!("{}\nClick to see the change", pretty_path(&path))).clicked() {
                    self.actions.push(Action::Git(GitOp::Diff(path.clone())));
                }
            }
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(6.0);

            let setup_needed = self.gstat.user_name.is_empty() || self.gstat.remote_url.is_empty();
            egui::CollapsingHeader::new(RichText::new("Repository").small().strong()).default_open(setup_needed).show(ui, |ui| {
                repo_field(ui, "Remote", "Repository URL", &mut self.remote_input);
                repo_field(ui, "Name", "Shown on your commits", &mut self.id_name);
                repo_field(ui, "Email", "Shown on your commits", &mut self.id_email);
                ui.horizontal(|ui| {
                    if self.remote_input.trim() != self.gstat.remote_url && ui.small_button("Save remote").clicked() {
                        self.actions.push(Action::Git(GitOp::SetRemote(self.remote_input.clone())));
                    }
                    let unchanged = self.id_name == self.gstat.user_name && self.id_email == self.gstat.user_email;
                    if !unchanged && ui.small_button("Save identity").clicked() {
                        self.actions.push(Action::Git(GitOp::SetIdentity { name: self.id_name.clone(), email: self.id_email.clone() }));
                    }
                });
            });
            egui::CollapsingHeader::new(RichText::new(format!("Recent commits ({})", self.gstat.log.len())).small().strong())
                .id_salt("git_commits")
                .show(ui, |ui| {
                    if self.gstat.log.is_empty() {
                        ui.label(RichText::new("No commits yet.").small().color(p.weak));
                    }
                    for c in &self.gstat.log {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&c.hash).monospace().size(11.0).color(p.send));
                            ui.add(egui::Label::new(RichText::new(&c.subject).small()).truncate().show_tooltip_when_elided(false))
                                .on_hover_text(format!("{}\n{} · {}", c.subject, c.author, c.when));
                        });
                    }
                });
            if !self.git_output.is_empty() {
                egui::CollapsingHeader::new(RichText::new("Git output").small().strong()).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.git_output.as_str())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
        });
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let p = self.pal();
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.5, p.ok);
            ui.label(RichText::new("Online").small().color(p.weak));
            ui.separator();
            // Console toggle (like the bottom bar of most API clients)
            {
                let open = self.console.open;
                let errors = self.console.entries.iter().filter(|e| e.level == console::LogLevel::Error).count();
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(if errors > 0 { 92.0 } else { 74.0 }, 22.0), Sense::click());
                if open || resp.hovered() {
                    ui.painter().rect_filled(rect, 4.0, if open { p.selected } else { p.hover });
                }
                let ic = Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), Vec2::splat(14.0));
                paint_icon(ui, ic, "console", if open { p.text } else { p.weak });
                ui.painter().text(
                    Pos2::new(rect.left() + 22.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "Console",
                    FontId::proportional(11.0),
                    if open { p.text } else { p.weak },
                );
                if errors > 0 {
                    let c = Pos2::new(rect.right() - 10.0, rect.center().y);
                    ui.painter().circle_filled(c, 7.0, p.err);
                    ui.painter().text(c, Align2::CENTER_CENTER, errors.min(99).to_string(), FontId::proportional(9.0), Color32::WHITE);
                }
                if resp
                    .on_hover_text(format!("Show all requests and responses ({})", shortcut("C", true)))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.console.open = !self.console.open;
                }
            }
            ui.separator();
            if self.gstat.is_repo {
                let mut parts = vec![format!("⎇ {}", self.gstat.branch)];
                if !self.gstat.changes.is_empty() {
                    parts.push(format!("{} changed", self.gstat.changes.len()));
                }
                if self.gstat.ahead > 0 {
                    parts.push(format!("↑{}", self.gstat.ahead));
                }
                if self.gstat.behind > 0 {
                    parts.push(format!("↓{}", self.gstat.behind));
                }
                if ui.add(egui::Button::new(RichText::new(parts.join("   ")).small().color(p.weak)).frame(false)).clicked() {
                    self.side = SideView::Git;
                }
            } else {
                ui.label(RichText::new("Not synced with git").small().color(p.weak));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let env = match self.active_env() {
                    Some(e) => format!("Environment: {}", e.name),
                    None => "No environment".into(),
                };
                ui.label(RichText::new(env).small().color(p.weak));
            });
        });
    }
}

/// One line in the recent projects list: its name, and where it is underneath.
fn project_row(ui: &mut Ui, path: &Path, current: bool, dark: bool) -> egui::Response {
    let p = palette(dark);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 4.0, p.selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, p.hover);
    }
    let icon = Rect::from_center_size(Pos2::new(rect.left() + 15.0, rect.center().y), Vec2::splat(16.0));
    paint_icon(ui, icon, "folder", if current { p.accent } else { p.weak });
    let left = rect.left() + 30.0;
    let name = elide_end(&settings::project_name(path), 32);
    ui.painter().text(Pos2::new(left, rect.top() + 12.0), Align2::LEFT_CENTER, name, FontId::proportional(13.0), p.text);
    ui.painter().text(
        Pos2::new(left, rect.top() + 27.0),
        Align2::LEFT_CENTER,
        settings::project_location(path),
        FontId::proportional(10.5),
        p.weak,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(path.display().to_string())
}

/// A labelled text box filling the panel's width.
fn repo_field(ui: &mut Ui, label: &str, hint: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(42.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(42.0);
            ui.label(RichText::new(label).small());
        });
        ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(ui.available_width()));
    });
}

fn elide_end(text: &str, most: usize) -> String {
    if text.chars().count() <= most { text.to_string() } else { format!("{}…", text.chars().take(most - 1).collect::<String>()) }
}

pub(super) fn caret(ui: &mut Ui, open: bool, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
    let c = r.center();
    let pts = if open {
        vec![c + Vec2::new(-3.5, -1.8), c + Vec2::new(3.5, -1.8), c + Vec2::new(0.0, 2.5)]
    } else {
        vec![c + Vec2::new(-1.8, -3.5), c + Vec2::new(-1.8, 3.5), c + Vec2::new(2.5, 0.0)]
    };
    ui.painter().add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

pub(super) fn tree_row(ui: &mut Ui, depth: usize, selected: bool, dark: bool, content: impl FnOnce(&mut Ui)) -> egui::Response {
    sized_row(ui, 28.0, depth, selected, dark, content)
}

fn sized_row(ui: &mut Ui, height: f32, depth: usize, selected: bool, dark: bool, content: impl FnOnce(&mut Ui)) -> egui::Response {
    let p = palette(dark);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 4.0, p.selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, p.hover);
    }
    let inner = Rect::from_min_max(rect.min + Vec2::new(4.0 + depth as f32 * 14.0, 0.0), rect.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = 6.0;
    child.style_mut().interaction.selectable_labels = false;
    content(&mut child);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn collection_menu(ui: &mut Ui, cid: &str, actions: &mut Vec<Action>) {
    let c = cid.to_string();
    if ui.button("Open overview").clicked() {
        actions.push(Action::OpenCollection(c.clone()));
        ui.close();
    }
    if ui.button("Run collection").clicked() {
        actions.push(Action::RunCollection(c.clone()));
        ui.close();
    }
    ui.separator();
    if ui.button("Add request").clicked() {
        actions.push(Action::AddRequest { coll: c.clone(), parent: None });
        ui.close();
    }
    if ui.button("Add folder").clicked() {
        actions.push(Action::AddFolder { coll: c.clone(), parent: None });
        ui.close();
    }
    ui.separator();
    if ui.button("Rename").clicked() {
        actions.push(Action::Rename { coll: c.clone(), item: None });
        ui.close();
    }
    if ui.button("Duplicate").clicked() {
        actions.push(Action::Duplicate { coll: c.clone(), item: None });
        ui.close();
    }
    if ui.button("Export…").clicked() {
        actions.push(Action::Export(c.clone()));
        ui.close();
    }
    if ui.button(RichText::new("Delete").color(Color32::from_rgb(0xd1, 0x2a, 0x1f))).clicked() {
        actions.push(Action::Delete { coll: c, item: None });
        ui.close();
    }
}

#[allow(clippy::too_many_arguments)]
fn tree_items(
    ui: &mut Ui,
    items: &[Item],
    cid: &str,
    depth: usize,
    expanded: &mut HashSet<String>,
    active: Option<&str>,
    filter: &str,
    dark: bool,
    actions: &mut Vec<Action>,
) {
    let p = palette(dark);
    for it in items {
        match it {
            Item::Folder(f) => {
                if !filter.is_empty() && !f.name.to_lowercase().contains(filter) && !items_match(&f.items, filter) {
                    continue;
                }
                let open = expanded.contains(&f.id) || !filter.is_empty();
                let resp = ui
                    .push_id(&f.id, |ui| {
                        tree_row(ui, depth, false, dark, |ui| {
                            caret(ui, open, p.weak);
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            paint_icon(ui, r, "folder", p.weak);
                            ui.label(&f.name);
                        })
                    })
                    .inner;
                if resp.clicked() {
                    if open {
                        expanded.remove(&f.id);
                    } else {
                        expanded.insert(f.id.clone());
                    }
                }
                let (c, id) = (cid.to_string(), f.id.clone());
                resp.context_menu(|ui| {
                    if ui.button("Edit folder").clicked() {
                        actions.push(Action::OpenFolder { coll: c.clone(), id: id.clone() });
                        ui.close();
                    }
                    if ui.button("Add request").clicked() {
                        actions.push(Action::AddRequest { coll: c.clone(), parent: Some(id.clone()) });
                        ui.close();
                    }
                    if ui.button("Add folder").clicked() {
                        actions.push(Action::AddFolder { coll: c.clone(), parent: Some(id.clone()) });
                        ui.close();
                    }
                    item_menu(ui, &c, &id, actions);
                });
                if open {
                    tree_items(ui, &f.items, cid, depth + 1, expanded, active, filter, dark, actions);
                }
            }
            Item::Request(r) => {
                if !filter.is_empty() && !r.name.to_lowercase().contains(filter) && !r.url.to_lowercase().contains(filter) {
                    continue;
                }
                let sel = active == Some(r.id.as_str());
                let resp = ui
                    .push_id(&r.id, |ui| {
                        tree_row(ui, depth, sel, dark, |ui| {
                            let m = method_short(&r.method);
                            let (mr, _) = ui.allocate_exact_size(Vec2::new(40.0, 16.0), Sense::hover());
                            ui.painter().text(
                                mr.right_center(),
                                Align2::RIGHT_CENTER,
                                m,
                                FontId::proportional(10.0),
                                method_color(&r.method, dark),
                            );
                            ui.add(egui::Label::new(&r.name).truncate());
                        })
                    })
                    .inner;
                if resp.clicked() {
                    actions.push(Action::OpenRequest { coll: cid.to_string(), id: r.id.clone() });
                }
                let (c, id) = (cid.to_string(), r.id.clone());
                resp.context_menu(|ui| {
                    if ui.button("Open in tab").clicked() {
                        actions.push(Action::OpenRequest { coll: c.clone(), id: id.clone() });
                        ui.close();
                    }
                    item_menu(ui, &c, &id, actions);
                });
            }
        }
    }
}

fn item_menu(ui: &mut Ui, c: &str, id: &str, actions: &mut Vec<Action>) {
    ui.separator();
    let (c, id) = (c.to_string(), id.to_string());
    if ui.button("Rename").clicked() {
        actions.push(Action::Rename { coll: c.clone(), item: Some(id.clone()) });
        ui.close();
    }
    if ui.button("Duplicate").clicked() {
        actions.push(Action::Duplicate { coll: c.clone(), item: Some(id.clone()) });
        ui.close();
    }
    if ui.button("Move up").clicked() {
        actions.push(Action::Move { coll: c.clone(), item: id.clone(), up: true });
        ui.close();
    }
    if ui.button("Move down").clicked() {
        actions.push(Action::Move { coll: c.clone(), item: id.clone(), up: false });
        ui.close();
    }
    if ui.button(RichText::new("Delete").color(Color32::from_rgb(0xd1, 0x2a, 0x1f))).clicked() {
        actions.push(Action::Delete { coll: c, item: Some(id) });
        ui.close();
    }
}
