//! The application window: state and logic. UI pieces live in submodules.

mod actions;
mod capture;
mod console;
mod dialogs;
mod editors;
mod helpers;
mod pages;
mod request_view;
mod sending;
mod sidebar;
mod sync;
mod tab_strip;
mod workspace_state;

use crate::ui::*;
use editors::*;
use eframe::egui::{self, Margin, Stroke, Ui};
use helpers::*;
use pigeon_core::formats;
use pigeon_core::git::{self, GitOp, GitResult, GitStatus};
use pigeon_core::http::{self, ResponseData, Vars};
use pigeon_core::model::*;
use pigeon_core::scripting::{self, Changes, Job, Outcome, Scopes};
use pigeon_core::settings::{self, Settings};
use pigeon_core::workspace;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SideView {
    Collections,
    Environments,
    History,
    Git,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReqSub {
    Params,
    Auth,
    Headers,
    Body,
    Scripts,
    Tests,
    Docs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RespView {
    Body,
    Headers,
    Tests,
}

struct ReqTab {
    req: Request,
    saved: Option<Request>,
    origin: Option<String>,
    sub: ReqSub,
    resp: Option<ResponseData>,
    rx: Option<Receiver<Outcome>>,
    resp_view: RespView,
    pretty: bool,
    show_code: bool,
    code_lang: usize,
    bulk_params: Option<BulkEdit>,
    bulk_headers: Option<BulkEdit>,
}

impl ReqTab {
    fn new(req: Request, origin: Option<String>) -> Self {
        let saved = origin.as_ref().map(|_| req.clone());
        ReqTab {
            req,
            saved,
            origin,
            sub: ReqSub::Params,
            resp: None,
            rx: None,
            resp_view: RespView::Body,
            pretty: true,
            show_code: false,
            code_lang: 0,
            bulk_params: None,
            bulk_headers: None,
        }
    }
    fn dirty(&self) -> bool {
        match &self.saved {
            Some(s) => s != &self.req,
            None => self.req != Request { id: self.req.id.clone(), name: "Untitled Request".into(), ..Default::default() },
        }
    }
}

struct RunResult {
    name: String,
    method: String,
    url: String,
    status: u16,
    time_ms: u64,
    error: Option<String>,
    tests: Vec<(String, bool, String)>,
}

enum RunMsg {
    Result(RunResult, Box<Outcome>),
    Done,
}

struct RunnerTab {
    coll: String,
    iterations: u32,
    delay_ms: u64,
    results: Vec<RunResult>,
    rx: Option<Receiver<RunMsg>>,
    started: bool,
}

enum TabKind {
    Request(Box<ReqTab>),
    Env(String),
    Globals,
    Collection { id: String, sub: usize },
    Folder { coll: String, id: String },
    Runner(Box<RunnerTab>),
}

struct Tab {
    uid: u64,
    kind: TabKind,
}

#[derive(Clone)]
enum Action {
    OpenRequest {
        coll: String,
        id: String,
    },
    OpenNewRequest(Box<Request>),
    NewTab,
    AddRequest {
        coll: String,
        parent: Option<String>,
    },
    AddFolder {
        coll: String,
        parent: Option<String>,
    },
    NewCollection,
    Rename {
        coll: String,
        item: Option<String>,
    },
    Duplicate {
        coll: String,
        item: Option<String>,
    },
    Delete {
        coll: String,
        item: Option<String>,
    },
    DeleteConfirmed {
        coll: String,
        item: Option<String>,
    },
    Move {
        coll: String,
        item: String,
        up: bool,
    },
    Export(String),
    OpenCollection(String),
    OpenFolder {
        coll: String,
        id: String,
    },
    RunCollection(String),
    OpenEnv(String),
    NewEnv,
    DeleteEnv(String),
    DeleteEnvConfirmed(String),
    DuplicateEnv(String),
    ExportEnv(String),
    SetActiveEnv(Option<String>),
    OpenGlobals,
    SaveTab(usize),
    CloseTab(usize),
    Git(GitOp),
    Import,
    ChooseWorkspace,
    /// Open a project, asking first if that would lose anything.
    OpenProject(PathBuf),
    /// Open a project now, closing every tab.
    SwitchProject(PathBuf),
    ForgetProject(PathBuf),
    ReloadProject,
}

enum Dialog {
    Rename { coll: String, item: Option<String>, name: String },
    SaveAs { tab: usize, name: String, coll: Option<String>, folder: Option<String>, new_coll: String },
    Confirm { text: String, action: Action },
    Import { text: String, error: String },
    Settings,
    Clone { url: String, dest: String },
    Diff { path: String, text: String },
    CloseUnsaved { tab: usize },
    SwitchProject { path: PathBuf, unsaved: usize },
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
    done: Arc<AtomicBool>,
}

pub struct App {
    settings: Settings,
    ws: PathBuf,
    collections: Vec<Collection>,
    envs: Vec<Environment>,
    history: Vec<HistoryEntry>,
    load_errors: Vec<String>,
    ws_dirty: bool,
    dirty_since: f64,
    tabs: Vec<Tab>,
    active: usize,
    next_uid: u64,
    side: SideView,
    filter: String,
    expanded: HashSet<String>,
    actions: Vec<Action>,
    dialog: Option<Dialog>,
    toasts: Vec<(String, bool, f64)>,
    // git
    gstat: GitStatus,
    git_rx: Option<Receiver<GitResult>>,
    git_busy: Option<&'static str>,
    git_msg: Option<(bool, String)>,
    git_output: String,
    commit_msg: String,
    remote_input: String,
    init_remote: String,
    id_name: String,
    id_email: String,
    new_branch: String,
    last_git_refresh: f64,
    git_identity_loaded: bool,
    shot: Option<Shot>,
    no_disk: bool,
    theme_applied: Option<bool>,
    logo: Option<egui::TextureHandle>,
    console: console::Console,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        let no_disk = screenshot.is_some();
        let settings = if no_disk { Settings::default() } else { settings::load_settings() };
        apply_theme(&cc.egui_ctx, settings.dark);
        let ws = settings.workspace.clone().unwrap_or_else(settings::default_workspace);
        let mut app = App {
            settings,
            ws,
            collections: vec![],
            envs: vec![],
            history: if no_disk { vec![] } else { settings::load_history() },
            load_errors: vec![],
            ws_dirty: false,
            dirty_since: 0.0,
            tabs: vec![],
            active: 0,
            next_uid: 1,
            side: SideView::Collections,
            filter: String::new(),
            expanded: HashSet::new(),
            actions: vec![],
            dialog: None,
            toasts: vec![],
            gstat: GitStatus::default(),
            git_rx: None,
            git_busy: None,
            git_msg: None,
            git_output: String::new(),
            commit_msg: String::new(),
            remote_input: String::new(),
            init_remote: String::new(),
            id_name: String::new(),
            id_email: String::new(),
            new_branch: String::new(),
            last_git_refresh: -100.0,
            git_identity_loaded: false,
            shot: screenshot.map(|path| Shot { path, frames: 0, requested: false, done: Arc::new(AtomicBool::new(false)) }),
            no_disk,
            theme_applied: None,
            logo: load_logo(&cc.egui_ctx),
            console: console::Console::new(),
        };
        let opened = app.ws.clone();
        app.settings.remember_project(&opened);
        if no_disk {
            let c = sample_collection();
            let e = sample_environment();
            app.settings.active_env = Some(e.id.clone());
            app.envs.push(e);
            app.collections.push(c);
            app.gstat = GitStatus { git_found: true, is_repo: true, branch: "main".into(), ..Default::default() };
            #[cfg(feature = "qa")]
            app.qa_sample();
        } else {
            app.load_workspace();
            if app.collections.is_empty() && app.envs.is_empty() && app.load_errors.is_empty() && !app.ws.join("collections").exists() {
                app.collections.push(sample_collection());
                let e = sample_environment();
                app.settings.active_env = Some(e.id.clone());
                app.envs.push(e);
                app.ws_dirty = true;
                app.flush_save();
            }
            if app.settings.workspace.is_none() {
                app.settings.workspace = Some(app.ws.clone());
            }
            settings::save_settings(&app.settings);
            let st = git::status(&app.ws);
            let pull = app.settings.pull_on_start && st.is_repo && !st.remote_url.is_empty() && st.conflicts.is_empty();
            app.gstat = st;
            app.start_git(if pull { GitOp::Pull(String::new()) } else { GitOp::Refresh });
        }
        for c in &app.collections {
            app.expanded.insert(c.id.clone());
        }
        // Open the first request so the editor isn't empty.
        let first = app.collections.first().and_then(|c| first_request(&c.items).map(|r| (c.id.clone(), r.id.clone())));
        if let Some(c) = app.collections.first()
            && let Some(Item::Folder(f)) = c.items.first()
        {
            app.expanded.insert(f.id.clone());
        }
        match first {
            Some((cid, rid)) => app.actions.push(Action::OpenRequest { coll: cid, id: rid }),
            None => app.actions.push(Action::NewTab),
        }
        app
    }

    fn dark(&self) -> bool {
        self.settings.dark
    }

    fn pal(&self) -> Palette {
        palette(self.settings.dark)
    }

    fn toast(&mut self, text: impl Into<String>, ok: bool, now: f64) {
        self.toasts.push((text.into(), ok, now + 4.5));
    }

    fn save_settings(&self) {
        if !self.no_disk {
            settings::save_settings(&self.settings);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.theme_applied != Some(self.settings.dark) {
            apply_theme(&ctx, self.settings.dark);
            self.theme_applied = Some(self.settings.dark);
        }
        let now = ctx.input(|i| i.time);
        self.apply_actions(&ctx);
        self.poll_git(now);
        self.poll_responses();
        self.shortcuts(&ctx);
        let waiting = self.git_rx.is_some()
            || self.tabs.iter().any(|t| match &t.kind {
                TabKind::Request(r) => r.rx.is_some(),
                TabKind::Runner(r) => r.rx.is_some(),
                _ => false,
            });
        if waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
        let p = self.pal();

        egui::Panel::top("header")
            .exact_size(46.0)
            .frame(egui::Frame::new().fill(p.header).stroke(Stroke::new(1.0, p.border)).inner_margin(Margin::symmetric(4, 0)))
            .show(ui, |ui| self.header_bar(ui));
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .frame(egui::Frame::new().fill(p.header).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| self.status_bar(ui));
        if self.console.open {
            egui::Panel::bottom("console")
                .resizable(true)
                .default_size(280.0)
                .min_size(120.0)
                .frame(egui::Frame::new().fill(p.bg).stroke(Stroke::new(1.0, p.border)).inner_margin(Margin::symmetric(6, 2)))
                .show(ui, |ui| self.console_panel(ui));
        }
        egui::Panel::left("rail")
            .exact_size(76.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| self.rail(ui));
        egui::Panel::left("sidebar")
            .default_size(300.0)
            .min_size(220.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.border)).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| self.main_area(ui));

        self.dialogs(&ctx);
        self.draw_toasts(&ctx);
        self.apply_actions(&ctx);

        if self.ws_dirty && now - self.dirty_since > 0.6 {
            self.flush_save();
            self.start_git(GitOp::Refresh);
        } else if self.ws_dirty {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        self.screenshot_tick(&ctx);
    }

    #[cfg(feature = "qa")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        self.scripted_input(input);
    }

    fn on_exit(&mut self) {
        self.flush_save();
    }
}
