//! The workspace in memory: loading, saving, variables and what scripts changed.

use super::*;

impl App {
    pub(super) fn load_workspace(&mut self) {
        let l = workspace::load(&self.ws);
        self.collections = l.collections;
        self.envs = l.environments;
        self.load_errors = l.errors;
        let colls = &self.collections;
        for t in &mut self.tabs {
            if let TabKind::Request(rt) = &mut t.kind
                && let Some(cid) = rt.origin.clone()
            {
                let found = colls.iter().find(|c| c.id == cid).and_then(|c| find_request(&c.items, &rt.req.id)).cloned();
                match found {
                    Some(r) => {
                        if !rt.dirty() {
                            rt.req = r.clone();
                        }
                        rt.saved = Some(r);
                    }
                    None => {
                        rt.origin = None;
                        rt.saved = None;
                    }
                }
            }
        }
        let envs = &self.envs;
        let cols = &self.collections;
        self.tabs.retain(|t| match &t.kind {
            TabKind::Env(id) => envs.iter().any(|e| &e.id == id),
            TabKind::Collection { id, .. } => cols.iter().any(|c| &c.id == id),
            TabKind::Folder { coll, id } => cols.iter().any(|c| &c.id == coll && find_folder_ref(&c.items, id).is_some()),
            TabKind::Runner(r) => cols.iter().any(|c| c.id == r.coll),
            _ => true,
        });
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        if self.tabs.is_empty() {
            self.actions.push(Action::NewTab);
        }
    }

    pub(super) fn mark_dirty(&mut self, now: f64) {
        if !self.ws_dirty {
            self.dirty_since = now;
        }
        self.ws_dirty = true;
    }

    pub(super) fn flush_save(&mut self) {
        if !self.ws_dirty || self.no_disk {
            self.ws_dirty = false;
            return;
        }
        if !self.load_errors.is_empty() {
            // Never overwrite files we could not read (e.g. git conflicts).
            return;
        }
        if let Err(e) = workspace::save(&self.ws, &self.collections, &self.envs) {
            self.toasts.push((format!("Could not save: {}", e), false, f64::MAX));
        }
        self.ws_dirty = false;
    }

    pub(super) fn switch_workspace(&mut self, path: PathBuf) {
        self.flush_save();
        // Nothing from the old project may carry over: an edit held back by a conflict,
        // a git answer still on its way, or the words of its last commit.
        self.ws_dirty = false;
        self.git_rx = None;
        self.git_msg = None;
        self.git_output.clear();
        self.commit_msg.clear();
        self.ws = path;
        self.settings.workspace = Some(self.ws.clone());
        self.settings.remember_project(&self.ws.clone());
        self.save_settings();
        self.tabs.clear();
        self.active = 0;
        self.load_workspace();
        self.git_identity_loaded = false;
        for c in &self.collections {
            self.expanded.insert(c.id.clone());
        }
        self.gstat = git::status(&self.ws);
        self.remote_input = self.gstat.remote_url.clone();
        self.id_name = self.gstat.user_name.clone();
        self.id_email = self.gstat.user_email.clone();
    }

    pub(super) fn coll(&self, id: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }

    pub(super) fn coll_mut(&mut self, id: &str) -> Option<&mut Collection> {
        self.collections.iter_mut().find(|c| c.id == id)
    }

    pub(super) fn active_env(&self) -> Option<&Environment> {
        self.settings.active_env.as_ref().and_then(|id| self.envs.iter().find(|e| &e.id == id))
    }

    pub(super) fn vars_for(&self, coll: Option<&str>) -> Vars {
        self.scopes_for(coll).vars()
    }

    /// The variables a request can see: shared values plus what scripts set on this computer.
    pub(super) fn scopes_for(&self, coll: Option<&str>) -> Scopes {
        let values = |list: &[Kv]| -> BTreeMap<String, String> {
            list.iter().filter(|k| k.enabled && !k.key.is_empty()).map(|k| (k.key.clone(), k.value.clone())).collect()
        };
        let local_values = |id: &str, into: &mut BTreeMap<String, String>| {
            for (key, value) in self.settings.script_vars.get(id).into_iter().flatten() {
                match value {
                    Some(v) => into.insert(key.clone(), v.clone()),
                    None => into.remove(key),
                };
            }
        };
        let mut scopes = Scopes { globals: values(&self.settings.globals), ..Default::default() };
        if let Some(e) = self.active_env() {
            scopes.environment = values(&e.values);
            local_values(&e.id, &mut scopes.environment);
            scopes.has_environment = true;
            scopes.environment_name = e.name.clone();
        }
        if let Some(c) = coll.and_then(|c| self.coll(c)) {
            scopes.collection = values(&c.variables);
            local_values(&c.id, &mut scopes.collection);
        }
        scopes
    }

    pub(super) fn job_for(&self, req: &Request, coll: Option<&str>, iteration_count: usize) -> Job {
        let mut scripts = vec![];
        if let Some(c) = coll.and_then(|c| self.coll(c)) {
            let mut folders = vec![];
            script_chain(&c.items, &req.id, &mut folders);
            scripts.push(c.scripts.clone());
            scripts.extend(folders.into_iter().rev());
        }
        scripts.push(req.scripts.clone());
        Job {
            req: req.clone(),
            auth_chain: self.auth_chain_for(coll, &req.id),
            headers: self.collection_headers(coll),
            scripts,
            timeout_s: self.settings.timeout_s,
            follow_redirects: self.settings.follow_redirects,
            iteration: 0,
            iteration_count,
            script_limit: std::time::Duration::from_secs(30),
        }
    }

    /// Keep what scripts changed. Environment and collection values stay on this computer.
    pub(super) fn apply_changes(&mut self, coll: Option<&str>, changes: &Changes) {
        let mut touched = false;
        let env_id = self.settings.active_env.clone().filter(|_| self.active_env().is_some());
        for (id, list) in [(env_id.as_deref(), &changes.environment), (coll, &changes.collection)] {
            if let Some(id) = id.filter(|_| !list.is_empty()) {
                let local = self.settings.script_vars.entry(id.to_string()).or_default();
                local.extend(list.iter().cloned());
                touched = true;
            }
        }
        for (key, value) in &changes.globals {
            let globals = &mut self.settings.globals;
            match (globals.iter_mut().find(|k| &k.key == key), value) {
                (Some(row), Some(v)) => row.value = v.clone(),
                (None, Some(v)) => globals.push(Kv::new(key, v)),
                (_, None) => globals.retain(|k| &k.key != key),
            }
            touched = true;
        }
        if touched {
            self.save_settings();
        }
    }

    /// Put everything that happened during a send into the console, and keep variable changes.
    pub(super) fn record_outcome(&mut self, coll: Option<&str>, outcome: &Outcome, source: &str) {
        for line in &outcome.before.logs {
            self.console.log_script(&line.level, &line.text);
        }
        for call in &outcome.before.calls {
            self.console.log_response(call, "pm.sendRequest");
        }
        self.console.log_response(&outcome.resp, source);
        for line in &outcome.after.logs {
            self.console.log_script(&line.level, &line.text);
        }
        for call in &outcome.after.calls {
            self.console.log_response(call, "pm.sendRequest");
        }
        self.apply_changes(coll, &outcome.changes);
    }

    pub(super) fn collection_headers(&self, coll: Option<&str>) -> Vec<Kv> {
        coll.and_then(|c| self.coll(c)).map(|c| c.headers.clone()).unwrap_or_default()
    }

    pub(super) fn auth_chain_for(&self, coll: Option<&str>, req_id: &str) -> Vec<Auth> {
        let mut chain = vec![];
        if let Some(c) = coll.and_then(|c| self.coll(c)) {
            auth_chain(&c.items, req_id, &mut chain);
            chain.push(c.auth.clone());
        }
        chain
    }
}
