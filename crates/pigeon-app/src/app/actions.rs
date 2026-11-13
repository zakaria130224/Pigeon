//! Everything a click can ask for, applied between frames.

use super::*;

impl App {
    pub(super) fn new_uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    pub(super) fn push_tab(&mut self, kind: TabKind) {
        let uid = self.new_uid();
        let at = if self.tabs.is_empty() { 0 } else { (self.active + 1).min(self.tabs.len()) };
        self.tabs.insert(at, Tab { uid, kind });
        self.active = at;
    }

    pub(super) fn apply_actions(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let actions = std::mem::take(&mut self.actions);
        for a in actions {
            self.apply(a, now);
        }
    }

    pub(super) fn focus_tab(&mut self, pred: impl Fn(&TabKind) -> bool) -> bool {
        if let Some(i) = self.tabs.iter().position(|t| pred(&t.kind)) {
            self.active = i;
            true
        } else {
            false
        }
    }

    pub(super) fn apply(&mut self, a: Action, now: f64) {
        match a {
            Action::OpenRequest { coll, id } => {
                if self.focus_tab(|k| matches!(k, TabKind::Request(r) if r.req.id == id)) {
                    return;
                }
                if let Some(r) = self.coll(&coll).and_then(|c| find_request(&c.items, &id)).cloned() {
                    // Replace an untouched empty tab, like a preview.
                    let replace = matches!(self.tabs.get(self.active).map(|t| &t.kind), Some(TabKind::Request(rt)) if rt.origin.is_none() && !rt.dirty() && rt.resp.is_none() && rt.rx.is_none());
                    if replace {
                        self.tabs[self.active].kind = TabKind::Request(Box::new(ReqTab::new(r, Some(coll))));
                    } else {
                        self.push_tab(TabKind::Request(Box::new(ReqTab::new(r, Some(coll)))));
                    }
                }
            }
            Action::OpenNewRequest(r) => {
                let mut r = *r;
                r.id = new_id();
                self.push_tab(TabKind::Request(Box::new(ReqTab::new(r, None))));
            }
            Action::NewTab => {
                self.push_tab(TabKind::Request(Box::new(ReqTab::new(
                    Request { name: "Untitled Request".into(), ..Default::default() },
                    None,
                ))));
            }
            Action::AddRequest { coll, parent } => {
                let r = Request { name: "New Request".into(), ..Default::default() };
                let rid = r.id.clone();
                if let Some(c) = self.coll_mut(&coll) {
                    let list = match &parent {
                        Some(pid) => find_folder_mut(&mut c.items, pid).map(|f| &mut f.items),
                        None => Some(&mut c.items),
                    };
                    if let Some(list) = list {
                        list.push(Item::Request(r));
                    }
                }
                self.expanded.insert(coll.clone());
                if let Some(p) = &parent {
                    self.expanded.insert(p.clone());
                }
                self.mark_dirty(now);
                self.apply(Action::OpenRequest { coll: coll.clone(), id: rid.clone() }, now);
                self.dialog = Some(Dialog::Rename { coll, item: Some(rid), name: "New Request".into() });
            }
            Action::AddFolder { coll, parent } => {
                let f = Folder {
                    id: new_id(),
                    name: "New Folder".into(),
                    description: String::new(),
                    auth: Auth::default(),
                    scripts: Scripts::default(),
                    items: vec![],
                };
                let fid = f.id.clone();
                if let Some(c) = self.coll_mut(&coll) {
                    let list = match &parent {
                        Some(pid) => find_folder_mut(&mut c.items, pid).map(|f| &mut f.items),
                        None => Some(&mut c.items),
                    };
                    if let Some(list) = list {
                        list.push(Item::Folder(f));
                    }
                }
                self.expanded.insert(coll.clone());
                self.expanded.insert(fid.clone());
                if let Some(p) = &parent {
                    self.expanded.insert(p.clone());
                }
                self.mark_dirty(now);
                self.dialog = Some(Dialog::Rename { coll, item: Some(fid), name: "New Folder".into() });
            }
            Action::NewCollection => {
                let c = Collection::new("New Collection");
                let id = c.id.clone();
                self.collections.push(c);
                self.expanded.insert(id.clone());
                self.mark_dirty(now);
                self.side = SideView::Collections;
                self.dialog = Some(Dialog::Rename { coll: id, item: None, name: "New Collection".into() });
            }
            Action::Rename { coll, item } => {
                let name = match &item {
                    None => self.coll(&coll).map(|c| c.name.clone()),
                    Some(id) => self.coll(&coll).and_then(|c| find_item_ref(&c.items, id)).map(|i| i.name().to_string()),
                };
                if let Some(name) = name {
                    self.dialog = Some(Dialog::Rename { coll, item, name });
                }
            }
            Action::Duplicate { coll, item } => {
                match item {
                    None => {
                        if let Some(c) = self.coll(&coll).cloned() {
                            let mut n = c.clone();
                            n.id = new_id();
                            n.name = format!("{} Copy", c.name);
                            n.items = c.items.iter().map(reid).collect();
                            self.collections.push(n);
                        }
                    }
                    Some(id) => {
                        if let Some(c) = self.coll_mut(&coll)
                            && let Some((list, i)) = find_parent_list_mut(&mut c.items, &id)
                        {
                            let mut copy = reid(&list[i]);
                            match &mut copy {
                                Item::Request(r) => r.name = format!("{} Copy", r.name),
                                Item::Folder(f) => f.name = format!("{} Copy", f.name),
                            }
                            list.insert(i + 1, copy);
                        }
                    }
                }
                self.mark_dirty(now);
            }
            Action::Delete { coll, item } => {
                let what = match &item {
                    None => self.coll(&coll).map(|c| format!("the collection \"{}\" and everything in it", c.name)),
                    Some(id) => self.coll(&coll).and_then(|c| find_item_ref(&c.items, id)).map(|i| format!("\"{}\"", i.name())),
                };
                if let Some(what) = what {
                    self.dialog =
                        Some(Dialog::Confirm { text: format!("Delete {}?", what), action: Action::DeleteConfirmed { coll, item } });
                }
            }
            Action::DeleteConfirmed { coll, item } => {
                match item {
                    None => self.collections.retain(|c| c.id != coll),
                    Some(id) => {
                        if let Some(c) = self.coll_mut(&coll)
                            && let Some((list, i)) = find_parent_list_mut(&mut c.items, &id)
                        {
                            list.remove(i);
                        }
                    }
                }
                let cols = &self.collections;
                for t in &mut self.tabs {
                    if let TabKind::Request(rt) = &mut t.kind
                        && let Some(o) = &rt.origin
                        && !cols.iter().any(|c| &c.id == o && find_request(&c.items, &rt.req.id).is_some())
                    {
                        rt.origin = None;
                        rt.saved = None;
                    }
                }
                self.tabs.retain(|t| match &t.kind {
                    TabKind::Collection { id, .. } => cols.iter().any(|c| &c.id == id),
                    TabKind::Folder { coll, id } => cols.iter().any(|c| &c.id == coll && find_folder_ref(&c.items, id).is_some()),
                    TabKind::Runner(r) => cols.iter().any(|c| c.id == r.coll),
                    _ => true,
                });
                self.active = self.active.min(self.tabs.len().saturating_sub(1));
                if self.tabs.is_empty() {
                    self.apply(Action::NewTab, now);
                }
                self.mark_dirty(now);
            }
            Action::Move { coll, item, up } => {
                if let Some(c) = self.coll_mut(&coll)
                    && let Some((list, i)) = find_parent_list_mut(&mut c.items, &item)
                {
                    if up && i > 0 {
                        list.swap(i, i - 1);
                    } else if !up && i + 1 < list.len() {
                        list.swap(i, i + 1);
                    }
                }
                self.mark_dirty(now);
            }
            Action::Export(coll) => {
                if let Some(c) = self.coll(&coll) {
                    let text = formats::export_collection(c);
                    let name = format!("{}.collection.json", c.name);
                    if let Some(path) = rfd::FileDialog::new().set_file_name(&name).add_filter("JSON", &["json"]).save_file() {
                        match std::fs::write(&path, text) {
                            Ok(_) => self.toast(format!("Exported to {}", path.display()), true, now),
                            Err(e) => self.toast(format!("Export failed: {}", e), false, now),
                        }
                    }
                }
            }
            Action::OpenCollection(id) => {
                if !self.focus_tab(|k| matches!(k, TabKind::Collection { id: x, .. } if x == &id)) {
                    self.push_tab(TabKind::Collection { id, sub: 0, bulk_headers: None });
                }
            }
            Action::OpenFolder { coll, id } => {
                if !self.focus_tab(|k| matches!(k, TabKind::Folder { id: x, .. } if x == &id)) {
                    self.push_tab(TabKind::Folder { coll, id });
                }
            }
            Action::RunCollection(coll) => {
                if !self.focus_tab(|k| matches!(k, TabKind::Runner(r) if r.coll == coll)) {
                    self.push_tab(TabKind::Runner(Box::new(RunnerTab {
                        coll,
                        iterations: 1,
                        delay_ms: 0,
                        results: vec![],
                        rx: None,
                        started: false,
                    })));
                }
            }
            Action::OpenEnv(id) => {
                if !self.focus_tab(|k| matches!(k, TabKind::Env(x) if x == &id)) {
                    self.push_tab(TabKind::Env(id));
                }
            }
            Action::NewEnv => {
                let e = Environment { id: new_id(), name: "New Environment".into(), values: vec![] };
                let id = e.id.clone();
                self.envs.push(e);
                self.mark_dirty(now);
                self.side = SideView::Environments;
                self.apply(Action::OpenEnv(id), now);
            }
            Action::DeleteEnv(id) => {
                let name = self.envs.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
                self.dialog =
                    Some(Dialog::Confirm { text: format!("Delete the environment \"{}\"?", name), action: Action::DeleteEnvConfirmed(id) });
            }
            Action::DeleteEnvConfirmed(id) => {
                self.envs.retain(|e| e.id != id);
                self.tabs.retain(|t| !matches!(&t.kind, TabKind::Env(x) if x == &id));
                if self.settings.active_env.as_deref() == Some(id.as_str()) {
                    self.settings.active_env = None;
                    self.save_settings();
                }
                self.active = self.active.min(self.tabs.len().saturating_sub(1));
                if self.tabs.is_empty() {
                    self.apply(Action::NewTab, now);
                }
                self.mark_dirty(now);
            }
            Action::DuplicateEnv(id) => {
                if let Some(e) = self.envs.iter().find(|e| e.id == id).cloned() {
                    self.envs.push(Environment { id: new_id(), name: format!("{} Copy", e.name), values: e.values });
                    self.mark_dirty(now);
                }
            }
            Action::ExportEnv(id) => {
                if let Some(e) = self.envs.iter().find(|e| e.id == id) {
                    let text = formats::export_environment(e);
                    if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}.environment.json", e.name)).save_file() {
                        match std::fs::write(&path, text) {
                            Ok(_) => self.toast("Environment exported", true, now),
                            Err(e) => self.toast(format!("Export failed: {}", e), false, now),
                        }
                    }
                }
            }
            Action::SetActiveEnv(v) => {
                self.settings.active_env = v;
                self.save_settings();
            }
            Action::OpenGlobals => {
                if !self.focus_tab(|k| matches!(k, TabKind::Globals)) {
                    self.push_tab(TabKind::Globals);
                }
            }
            Action::SaveTab(i) => self.save_tab(i, now),
            Action::CloseTab(i) => {
                let dirty = matches!(self.tabs.get(i).map(|t| &t.kind), Some(TabKind::Request(r)) if r.dirty() && r.origin.is_some());
                if dirty {
                    self.dialog = Some(Dialog::CloseUnsaved { tab: i });
                } else {
                    self.close_tab(i);
                }
            }
            Action::Git(op) => self.start_git(op),
            Action::Import => self.dialog = Some(Dialog::Import { text: String::new(), error: String::new() }),
            Action::ChooseWorkspace => {
                if let Some(p) =
                    rfd::FileDialog::new().set_title("Choose a project folder, such as your team's git repository").pick_folder()
                {
                    self.actions.push(Action::OpenProject(p));
                }
            }
            Action::OpenProject(path) => {
                let unsaved =
                    self.tabs.iter().filter(|t| matches!(&t.kind, TabKind::Request(r) if r.dirty() && r.origin.is_some())).count();
                if path == self.ws {
                    // Already open.
                } else if self.git_busy.is_some() {
                    self.toast("Git is busy, try again in a moment.", false, now);
                } else if !path.is_dir() {
                    let name = settings::project_name(&path);
                    self.toast(format!("Can't find the folder for {}. If it moved, open it again from its new place.", name), false, now);
                } else if unsaved > 0 {
                    self.dialog = Some(Dialog::SwitchProject { path, unsaved });
                } else {
                    self.switch_workspace(path);
                }
            }
            Action::SwitchProject(path) => self.switch_workspace(path),
            Action::ForgetProject(path) => {
                self.settings.forget_project(&path);
                self.save_settings();
            }
            Action::ReloadProject => self.load_workspace(),
        }
    }

    pub(super) fn close_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.tabs.remove(i);
            if self.active > i || (self.active == i && self.active > 0) {
                self.active -= 1;
            }
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        if self.tabs.is_empty() {
            self.actions.push(Action::NewTab);
        }
    }

    pub(super) fn save_tab(&mut self, i: usize, now: f64) {
        let Some(Tab { kind: TabKind::Request(rt), .. }) = self.tabs.get(i) else { return };
        let req = clean_request(&rt.req);
        match rt.origin.clone() {
            Some(cid) => {
                let mut ok = false;
                if let Some(c) = self.coll_mut(&cid)
                    && let Some(r) = find_request_mut(&mut c.items, &req.id)
                {
                    *r = req.clone();
                    ok = true;
                }
                if ok {
                    if let Some(Tab { kind: TabKind::Request(rt), .. }) = self.tabs.get_mut(i) {
                        rt.saved = Some(req.clone());
                        rt.req = req;
                    }
                    self.mark_dirty(now);
                    self.flush_save();
                    if self.load_errors.is_empty() {
                        self.toast("Saved", true, now);
                    } else {
                        self.toast("Saving is paused until the git conflict is fixed", false, now);
                    }
                    self.start_git(GitOp::Refresh);
                } else {
                    self.open_save_as(i);
                }
            }
            None => self.open_save_as(i),
        }
    }

    pub(super) fn open_save_as(&mut self, i: usize) {
        let Some(Tab { kind: TabKind::Request(rt), .. }) = self.tabs.get(i) else { return };
        let name = if rt.req.name.is_empty() || rt.req.name == "Untitled Request" {
            rt.req.url.split('?').next().unwrap_or("").to_string()
        } else {
            rt.req.name.clone()
        };
        self.dialog = Some(Dialog::SaveAs {
            tab: i,
            name: if name.is_empty() { "New Request".into() } else { name },
            coll: self.collections.first().map(|c| c.id.clone()),
            folder: None,
            new_coll: String::new(),
        });
    }
}
