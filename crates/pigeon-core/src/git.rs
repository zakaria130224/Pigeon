//! Git sync: runs the `git` command line tool in the background.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};

#[derive(Clone, Debug)]
pub enum GitOp {
    Refresh,
    Init { remote: String },
    Clone { url: String, dest: PathBuf },
    SetRemote(String),
    SetIdentity { name: String, email: String },
    Commit(String),
    Pull(String),
    Push(String),
    Sync(String),
    KeepMine(Vec<(String, String)>),
    KeepTheirs(Vec<(String, String)>),
    FinishMerge,
    AbortMerge,
    Diff(String),
    SwitchBranch(String),
    NewBranch(String),
}

impl GitOp {
    pub fn label(&self) -> &'static str {
        match self {
            GitOp::Refresh => "Refresh",
            GitOp::Init { .. } => "Set up git",
            GitOp::Clone { .. } => "Clone",
            GitOp::SetRemote(_) => "Set remote",
            GitOp::SetIdentity { .. } => "Save identity",
            GitOp::Commit(_) => "Commit",
            GitOp::Pull(_) => "Pull",
            GitOp::Push(_) => "Push",
            GitOp::Sync(_) => "Sync",
            GitOp::KeepMine(_) => "Keep mine",
            GitOp::KeepTheirs(_) => "Keep theirs",
            GitOp::FinishMerge => "Finish merge",
            GitOp::AbortMerge => "Abort merge",
            GitOp::Diff(_) => "Diff",
            GitOp::SwitchBranch(_) => "Switch branch",
            GitOp::NewBranch(_) => "New branch",
        }
    }
    /// Whether the workspace on disk may have changed and must be reloaded.
    pub fn changes_files(&self) -> bool {
        matches!(
            self,
            GitOp::Clone { .. }
                | GitOp::Pull(_)
                | GitOp::Sync(_)
                | GitOp::KeepMine(_)
                | GitOp::KeepTheirs(_)
                | GitOp::AbortMerge
                | GitOp::FinishMerge
                | GitOp::SwitchBranch(_)
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Commit {
    pub hash: String,
    pub author: String,
    pub when: String,
    pub subject: String,
}

#[derive(Clone, Debug, Default)]
pub struct GitStatus {
    pub git_found: bool,
    pub is_repo: bool,
    pub branch: String,
    pub branches: Vec<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub remote_url: String,
    /// (two-letter code, path)
    pub changes: Vec<(String, String)>,
    pub conflicts: Vec<(String, String)>,
    pub merging: bool,
    pub log: Vec<Commit>,
    pub user_name: String,
    pub user_email: String,
}

#[derive(Clone, Debug)]
pub struct GitResult {
    pub op: GitOp,
    pub ok: bool,
    pub message: String,
    pub output: String,
    pub status: GitStatus,
    pub diff: Option<String>,
}

/// A `git` command that never waits for typing and, on Windows, never flashes a console window.
fn git_command() -> Command {
    let mut cmd = Command::new("git");
    // Keep non-English file names readable, and allow deep folders on Windows.
    cmd.args(["-c", "core.quotepath=false", "-c", "core.longpaths=true"]);
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("GIT_MERGE_AUTOEDIT", "no").env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = git_command().arg("-C").arg(dir).args(args).output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            let e = String::from_utf8_lossy(&o.stderr);
            if !e.trim().is_empty() {
                if !s.is_empty() && !s.ends_with('\n') {
                    s.push('\n');
                }
                s.push_str(&e);
            }
            (o.status.success(), s)
        }
        Err(e) => (false, format!("Could not run git: {}", e)),
    }
}

fn git_quiet(dir: &Path, args: &[&str]) -> String {
    let (ok, s) = git(dir, args);
    if ok { s.trim().to_string() } else { String::new() }
}

pub fn status(dir: &Path) -> GitStatus {
    let git_found = git_command().arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let mut st = GitStatus { git_found, ..Default::default() };
    if !st.git_found || !dir.exists() {
        return st;
    }
    let (ok, top) = git(dir, &["rev-parse", "--show-toplevel"]);
    st.is_repo = ok && !top.trim().is_empty();
    if !st.is_repo {
        return st;
    }
    let (_, s) = git(dir, &["status", "--porcelain=v1", "-b", "--untracked-files=all", "--", "."]);
    for (i, line) in s.lines().enumerate() {
        if i == 0 && line.starts_with("## ") {
            let head = &line[3..];
            let (names, extra) = match head.find(" [") {
                Some(p) => (&head[..p], &head[p..]),
                None => (head, ""),
            };
            if let Some(rest) = names.strip_prefix("No commits yet on ") {
                st.branch = rest.to_string();
            } else if let Some((b, u)) = names.split_once("...") {
                st.branch = b.to_string();
                st.upstream = Some(u.to_string());
            } else {
                st.branch = names.to_string();
            }
            for part in extra.trim_matches(|c| c == ' ' || c == '[' || c == ']').split(", ") {
                if let Some(n) = part.strip_prefix("ahead ") {
                    st.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    st.behind = n.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if line.len() < 4 {
            continue;
        }
        let code = line[..2].to_string();
        let mut path = line[3..].to_string();
        if let Some((_, to)) = path.split_once(" -> ") {
            path = to.to_string();
        }
        let path = path.trim_matches('"').to_string();
        if matches!(code.as_str(), "UU" | "AA" | "DD" | "AU" | "UA" | "DU" | "UD") {
            st.conflicts.push((code.clone(), path.clone()));
        }
        st.changes.push((code, path));
    }
    st.merging = git(dir, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).0;
    st.remote_url = git_quiet(dir, &["remote", "get-url", "origin"]);
    st.user_name = git_quiet(dir, &["config", "user.name"]);
    st.user_email = git_quiet(dir, &["config", "user.email"]);
    st.branches = git_quiet(dir, &["branch", "--format=%(refname:short)"]).lines().map(|s| s.to_string()).collect();
    let (_, log) = git(dir, &["log", "-n", "30", "--format=%h%x1f%an%x1f%ar%x1f%s"]);
    for l in log.lines() {
        let p: Vec<&str> = l.split('\u{1f}').collect();
        if p.len() == 4 {
            st.log.push(Commit { hash: p[0].into(), author: p[1].into(), when: p[2].into(), subject: p[3].into() });
        }
    }
    st
}

/// Paths in the workspace relative to repo root are fine; we always stage `.`.
fn commit_all(dir: &Path, msg: &str, log: &mut String) -> Result<bool, String> {
    let (ok, o) = git(dir, &["add", "-A", "--", "."]);
    log.push_str(&o);
    if !ok {
        return Err("Could not stage changes.".into());
    }
    let (nothing, _) = git(dir, &["diff", "--cached", "--quiet"]);
    if nothing {
        return Ok(false);
    }
    let msg = if msg.trim().is_empty() { "Update collections" } else { msg.trim() };
    let (ok, o) = git(dir, &["commit", "-m", msg]);
    log.push_str(&o);
    if !ok {
        if o.contains("Please tell me who you are") || o.contains("user.email") {
            return Err("Git needs your name and email before it can save a commit. Fill them in under 'Your identity'.".into());
        }
        return Err("Commit failed.".into());
    }
    Ok(true)
}

fn pull(dir: &Path, st: &GitStatus, log: &mut String) -> Result<String, String> {
    if st.remote_url.is_empty() {
        return Err("No remote repository is set. Add the repository URL first.".into());
    }
    let (ok, o) = if st.upstream.is_some() {
        git(dir, &["pull", "--no-rebase", "--no-edit"])
    } else {
        let (fok, fo) = git(dir, &["fetch", "origin"]);
        log.push_str(&fo);
        if !fok {
            return Err(auth_hint(&fo, "Could not reach the remote repository."));
        }
        let remote_ref = format!("origin/{}", st.branch);
        if !git(dir, &["rev-parse", "-q", "--verify", &remote_ref]).0 {
            return Ok("The remote has no commits on this branch yet. Push to share yours.".into());
        }
        let r = git(dir, &["merge", "--no-edit", "--allow-unrelated-histories", &remote_ref]);
        let _ = git(dir, &["branch", "--set-upstream-to", &remote_ref]);
        r
    };
    log.push_str(&o);
    if ok {
        if o.contains("Already up to date") {
            Ok("Already up to date.".into())
        } else {
            Ok("Pulled the latest changes from your team.".into())
        }
    } else if o.contains("CONFLICT") || o.contains("Automatic merge failed") {
        Err("Some files were changed by you and by someone else. Choose which version to keep below.".into())
    } else {
        Err(auth_hint(&o, "Pull failed."))
    }
}

fn push(dir: &Path, st: &GitStatus, log: &mut String) -> Result<String, String> {
    if st.remote_url.is_empty() {
        return Err("No remote repository is set. Add the repository URL first.".into());
    }
    let (ok, o) = if st.upstream.is_some() { git(dir, &["push"]) } else { git(dir, &["push", "-u", "origin", "HEAD"]) };
    log.push_str(&o);
    if ok {
        if o.contains("Everything up-to-date") {
            Ok("Nothing new to push.".into())
        } else {
            Ok("Pushed your changes to the team repository.".into())
        }
    } else if o.contains("rejected") || o.contains("fetch first") || o.contains("non-fast-forward") {
        Err("Someone else pushed first. Pull their changes, then push again.".into())
    } else {
        Err(auth_hint(&o, "Push failed."))
    }
}

fn auth_hint(out: &str, base: &str) -> String {
    let l = out.to_lowercase();
    if l.contains("authentication") || l.contains("could not read username") || l.contains("permission denied") || l.contains("403") {
        format!(
            "{} The repository refused your login. Set up access with an SSH key or a saved token (credential helper), then try again.",
            base
        )
    } else if l.contains("could not resolve host") || l.contains("unable to access") {
        format!("{} Could not reach the server. Check the URL and your connection.", base)
    } else if l.contains("not found") || l.contains("does not appear to be a git repository") {
        format!("{} The repository was not found. Check the URL.", base)
    } else {
        base.to_string()
    }
}

fn resolve(dir: &Path, files: &[(String, String)], mine: bool, log: &mut String) -> Result<String, String> {
    let root = git_quiet(dir, &["rev-parse", "--show-toplevel"]);
    let root = if root.is_empty() { dir.to_path_buf() } else { PathBuf::from(root) };
    for (code, path) in files {
        // Paths from `git status --porcelain` are relative to the repository root.
        let deleted_on_kept_side = matches!((code.as_str(), mine), ("DD", _) | ("DU", true) | ("UD", false));
        let (ok, o) = if deleted_on_kept_side {
            git(&root, &["rm", "-q", "--", path])
        } else {
            let side = if mine { "--ours" } else { "--theirs" };
            let r = git(&root, &["checkout", side, "--", path]);
            if r.0 { git(&root, &["add", "--", path]) } else { r }
        };
        log.push_str(&o);
        if !ok {
            return Err(format!("Could not resolve {}", path));
        }
    }
    Ok(format!(
        "Kept {} for {} file(s). Press Finish merge when all conflicts are resolved.",
        if mine { "your version" } else { "their version" },
        files.len()
    ))
}

fn run_op(dir: &Path, op: &GitOp) -> (bool, String, String, Option<String>) {
    let mut log = String::new();
    let st = status(dir);
    let res: Result<String, String> = match op {
        GitOp::Refresh => Ok(String::new()),
        GitOp::Diff(path) => {
            let root = git_quiet(dir, &["rev-parse", "--show-toplevel"]);
            let root = if root.is_empty() { dir.to_path_buf() } else { PathBuf::from(root) };
            let (_, mut o) = git(&root, &["diff", "HEAD", "--", path]);
            if o.trim().is_empty() {
                let (_, o2) = git(&root, &["diff", "--no-index", "--", "/dev/null", path]);
                o = o2;
            }
            return (true, String::new(), String::new(), Some(o));
        }
        GitOp::Init { remote } => {
            let _ = std::fs::create_dir_all(dir);
            let (ok, o) = git(dir, &["init", "-b", "main"]);
            log.push_str(&o);
            if !ok {
                Err("Could not set up git in this folder.".into())
            } else {
                if !remote.trim().is_empty() {
                    let (_, o) = git(dir, &["remote", "add", "origin", remote.trim()]);
                    log.push_str(&o);
                }
                match commit_all(dir, "Start shared workspace", &mut log) {
                    Ok(_) => Ok("This folder is now a git repository.".into()),
                    Err(e) => Err(e),
                }
            }
        }
        GitOp::Clone { url, dest } => {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let out = git_command().arg("clone").arg(url.trim()).arg(dest).output();
            match out {
                Ok(o) => {
                    log.push_str(&String::from_utf8_lossy(&o.stdout));
                    log.push_str(&String::from_utf8_lossy(&o.stderr));
                    if o.status.success() { Ok("Cloned the team repository.".into()) } else { Err(auth_hint(&log, "Clone failed.")) }
                }
                Err(e) => Err(format!("Could not run git: {}", e)),
            }
        }
        GitOp::SetRemote(url) => {
            let url = url.trim();
            let (ok, o) = if st.remote_url.is_empty() {
                git(dir, &["remote", "add", "origin", url])
            } else if url.is_empty() {
                git(dir, &["remote", "remove", "origin"])
            } else {
                git(dir, &["remote", "set-url", "origin", url])
            };
            log.push_str(&o);
            if ok { Ok("Remote repository saved.".into()) } else { Err("Could not save the remote.".into()) }
        }
        GitOp::SetIdentity { name, email } => {
            let a = git(dir, &["config", "user.name", name.trim()]);
            let b = git(dir, &["config", "user.email", email.trim()]);
            if a.0 && b.0 { Ok("Identity saved for this repository.".into()) } else { Err("Could not save identity.".into()) }
        }
        GitOp::Commit(msg) => match commit_all(dir, msg, &mut log) {
            Ok(true) => Ok("Committed your changes.".into()),
            Ok(false) => Ok("Nothing to commit.".into()),
            Err(e) => Err(e),
        },
        GitOp::Pull(msg) => {
            if st.merging {
                Err("Finish or abort the current merge first.".into())
            } else {
                match commit_all(dir, if msg.trim().is_empty() { "Local changes" } else { msg }, &mut log) {
                    Err(e) => Err(e),
                    Ok(_) => pull(dir, &status(dir), &mut log),
                }
            }
        }
        GitOp::Push(msg) => {
            if st.merging {
                Err("Finish or abort the current merge first.".into())
            } else {
                match commit_all(dir, msg, &mut log) {
                    Err(e) => Err(e),
                    Ok(_) => push(dir, &status(dir), &mut log),
                }
            }
        }
        GitOp::Sync(msg) => {
            if st.merging {
                Err("Finish or abort the current merge first.".into())
            } else {
                match commit_all(dir, msg, &mut log) {
                    Err(e) => Err(e),
                    Ok(_) => match pull(dir, &status(dir), &mut log) {
                        Err(e) => Err(e),
                        Ok(_) => push(dir, &status(dir), &mut log).map(|_| "Synced: pulled and pushed.".into()),
                    },
                }
            }
        }
        GitOp::KeepMine(files) => resolve(dir, files, true, &mut log),
        GitOp::KeepTheirs(files) => resolve(dir, files, false, &mut log),
        GitOp::FinishMerge => {
            let s = status(dir);
            if !s.conflicts.is_empty() {
                Err("Some files still have conflicts.".into())
            } else {
                let _ = git(dir, &["add", "-A", "--", "."]);
                let (ok, o) = git(dir, &["commit", "--no-edit"]);
                log.push_str(&o);
                if ok { Ok("Merge finished. Push to share it.".into()) } else { Err("Could not finish the merge.".into()) }
            }
        }
        GitOp::AbortMerge => {
            let (ok, o) = git(dir, &["merge", "--abort"]);
            log.push_str(&o);
            if ok { Ok("Merge aborted; your files are back as they were.".into()) } else { Err("Could not abort the merge.".into()) }
        }
        GitOp::SwitchBranch(b) => match commit_all(dir, "Local changes", &mut log) {
            Err(e) => Err(e),
            Ok(_) => {
                let (ok, o) = git(dir, &["checkout", b]);
                log.push_str(&o);
                if ok { Ok(format!("Switched to {}.", b)) } else { Err("Could not switch branch.".into()) }
            }
        },
        GitOp::NewBranch(b) => {
            let (ok, o) = git(dir, &["checkout", "-b", b.trim()]);
            log.push_str(&o);
            if ok { Ok(format!("Created branch {}.", b.trim())) } else { Err("Could not create branch.".into()) }
        }
    };
    match res {
        Ok(m) => (true, m, log, None),
        Err(m) => (false, m, log, None),
    }
}

pub fn spawn(dir: PathBuf, op: GitOp) -> Receiver<GitResult> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (ok, message, output, diff) = run_op(&dir, &op);
        let status_dir = match &op {
            GitOp::Clone { dest, .. } if ok => dest.clone(),
            _ => dir.clone(),
        };
        let status = status(&status_dir);
        let _ = tx.send(GitResult { op, ok, message, output, status, diff });
    });
    rx
}

pub fn describe_code(code: &str) -> &'static str {
    match code {
        "??" => "new",
        "UU" | "AA" | "DD" | "AU" | "UA" | "DU" | "UD" => "conflict",
        c if c.contains('D') => "deleted",
        c if c.contains('A') => "added",
        c if c.contains('R') => "renamed",
        _ => "changed",
    }
}
