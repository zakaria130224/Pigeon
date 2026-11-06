//! Running git in the background and showing what came back.

use super::*;

impl App {
    pub(super) fn start_git(&mut self, op: GitOp) {
        if self.no_disk {
            return;
        }
        if self.git_rx.is_some() {
            if !matches!(op, GitOp::Refresh) {
                self.git_msg = Some((false, "Git is busy, try again in a moment.".into()));
            }
            return;
        }
        self.flush_save();
        if !matches!(op, GitOp::Refresh) {
            self.git_busy = Some(op.label());
            self.git_msg = None;
        }
        self.git_rx = Some(git::spawn(self.ws.clone(), op));
    }

    pub(super) fn poll_git(&mut self, now: f64) {
        let Some(rx) = &self.git_rx else {
            if now - self.last_git_refresh > 12.0 {
                self.last_git_refresh = now;
                self.start_git(GitOp::Refresh);
            }
            return;
        };
        let Ok(res) = rx.try_recv() else { return };
        self.git_rx = None;
        self.git_busy = None;
        self.last_git_refresh = now;
        let user_op = !matches!(res.op, GitOp::Refresh);
        if let GitOp::Clone { dest, .. } = &res.op
            && res.ok
        {
            let d = dest.clone();
            self.switch_workspace(d);
            self.dialog = None;
        }
        if res.op.changes_files() {
            self.load_workspace();
        }
        self.gstat = res.status;
        if !self.git_identity_loaded && self.gstat.is_repo {
            self.id_name = self.gstat.user_name.clone();
            self.id_email = self.gstat.user_email.clone();
            self.remote_input = self.gstat.remote_url.clone();
            self.git_identity_loaded = true;
        }
        if matches!(res.op, GitOp::SetRemote(_) | GitOp::Init { .. }) {
            self.remote_input = self.gstat.remote_url.clone();
        }
        if let Some(d) = res.diff {
            if let GitOp::Diff(p) = &res.op {
                self.dialog = Some(Dialog::Diff { path: p.clone(), text: d });
            }
            return;
        }
        if user_op {
            if !res.output.trim().is_empty() {
                self.git_output = res.output.clone();
            }
            if res.ok && matches!(res.op, GitOp::Commit(_) | GitOp::Push(_) | GitOp::Sync(_)) {
                self.commit_msg.clear();
            }
            if !res.message.is_empty() {
                self.toast(res.message.clone(), res.ok, now);
                let level = if res.ok { console::LogLevel::Info } else { console::LogLevel::Error };
                self.console.log(level, format!("Git {}: {}", res.op.label(), res.message));
                self.git_msg = Some((res.ok, res.message));
            }
            if !self.gstat.conflicts.is_empty() || !res.ok {
                self.side = SideView::Git;
            }
        }
    }
}
