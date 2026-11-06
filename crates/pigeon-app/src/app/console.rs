//! Console: a bottom panel that logs every request, response, error and git action.

use super::*;
use eframe::egui::{Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LogLevel {
    Request,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LogFilter {
    All,
    Requests,
    Info,
    Warnings,
    Errors,
}

impl LogFilter {
    const ALL: [LogFilter; 5] = [LogFilter::All, LogFilter::Requests, LogFilter::Info, LogFilter::Warnings, LogFilter::Errors];
    fn label(self) -> &'static str {
        match self {
            LogFilter::All => "All Logs",
            LogFilter::Requests => "Network",
            LogFilter::Info => "Info",
            LogFilter::Warnings => "Warnings",
            LogFilter::Errors => "Errors",
        }
    }
    fn accepts(self, e: &LogEntry) -> bool {
        match self {
            LogFilter::All => true,
            LogFilter::Requests => e.level == LogLevel::Request || e.net.is_some(),
            LogFilter::Info => e.level == LogLevel::Info,
            LogFilter::Warnings => e.level == LogLevel::Warn,
            LogFilter::Errors => e.level == LogLevel::Error,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ConsoleTab {
    Console,
    Issues,
}

/// Details of one HTTP call.
pub(super) struct NetLog {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub time_ms: u64,
    pub size: usize,
    pub attempts: usize,
    pub req_headers: Vec<(String, String)>,
    pub req_body: String,
    pub resp_headers: Vec<(String, String)>,
    pub resp_body: String,
    pub error: Option<String>,
    pub tests: Vec<(String, bool, String)>,
    pub source: String,
}

pub(super) struct LogEntry {
    pub id: u64,
    pub at: String,
    pub level: LogLevel,
    pub text: String,
    pub net: Option<NetLog>,
}

pub(super) struct Console {
    pub open: bool,
    pub tab: ConsoleTab,
    pub entries: Vec<LogEntry>,
    pub filter: LogFilter,
    pub search: String,
    pub expanded: HashSet<u64>,
    pub timestamps: bool,
    pub next: u64,
    pub tz_offset_s: i64,
}

const MAX_ENTRIES: usize = 1000;
const MAX_BODY: usize = 20_000;

fn clip(s: &str) -> String {
    if s.len() <= MAX_BODY {
        return s.to_string();
    }
    let mut end = MAX_BODY;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… ({} more bytes not shown)", &s[..end], s.len() - end)
}

impl Console {
    pub fn new() -> Self {
        Console {
            open: false,
            tab: ConsoleTab::Console,
            entries: vec![],
            filter: LogFilter::All,
            search: String::new(),
            expanded: HashSet::new(),
            timestamps: true,
            next: 1,
            tz_offset_s: chrono::Local::now().offset().local_minus_utc() as i64,
        }
    }

    fn clock(&self) -> String {
        let ms = now_ms() as i64 + self.tz_offset_s * 1000;
        let secs = ms.div_euclid(1000).rem_euclid(86_400);
        format!("{:02}:{:02}:{:02}.{:03}", secs / 3600, (secs % 3600) / 60, secs % 60, ms.rem_euclid(1000))
    }

    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        let e = LogEntry { id: self.next, at: self.clock(), level, text: text.into(), net: None };
        self.push(e);
    }

    /// A `console.log` from a script, or a script failure.
    pub fn log_script(&mut self, level: &str, text: &str) {
        let level = match level {
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        };
        self.log(level, clip(text));
    }

    pub fn log_response(&mut self, r: &ResponseData, source: &str) {
        let sent = r.sent.clone();
        let (method, url, req_headers, req_body) = match sent {
            Some(p) => (p.method, p.url, p.headers, p.body.map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default()),
            None => (String::new(), r.final_url.clone(), vec![], String::new()),
        };
        let level = if r.error.is_some() {
            LogLevel::Error
        } else if r.status >= 400 {
            LogLevel::Warn
        } else {
            LogLevel::Request
        };
        let text = format!("{} {}", method, url);
        let net = NetLog {
            method,
            url,
            status: r.status,
            status_text: r.status_text.clone(),
            time_ms: r.time_ms,
            size: r.size,
            attempts: r.attempts,
            req_headers,
            req_body: clip(&req_body),
            resp_headers: r.headers.clone(),
            resp_body: clip(r.pretty.as_deref().unwrap_or(&r.body)),
            error: r.error.clone(),
            tests: r.tests.clone(),
            source: source.to_string(),
        };
        let e = LogEntry { id: self.next, at: self.clock(), level, text, net: Some(net) };
        self.push(e);
    }

    fn push(&mut self, e: LogEntry) {
        self.next += 1;
        self.entries.push(e);
        if self.entries.len() > MAX_ENTRIES {
            let drop = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..drop);
        }
    }

    fn visible(&self) -> Vec<&LogEntry> {
        let q = self.search.to_lowercase();
        self.entries
            .iter()
            .filter(|e| self.filter.accepts(e))
            .filter(|e| {
                q.is_empty()
                    || e.text.to_lowercase().contains(&q)
                    || e.net.as_ref().is_some_and(|n| n.resp_body.to_lowercase().contains(&q))
            })
            .collect()
    }

    fn as_text(&self) -> String {
        let mut s = String::new();
        for e in self.visible() {
            s.push_str(&format!("[{}] {}", e.at, e.text));
            if let Some(n) = &e.net {
                match &n.error {
                    Some(err) => s.push_str(&format!("  ERROR: {}", err)),
                    None => s.push_str(&format!("  {} {}  {} ms  {}", n.status, n.status_text, n.time_ms, human_size(n.size))),
                }
                s.push('\n');
                s.push_str("  Request Headers\n");
                for (k, v) in &n.req_headers {
                    s.push_str(&format!("    {}: {}\n", k, v));
                }
                if !n.req_body.is_empty() {
                    s.push_str(&format!("  Request Body\n    {}\n", n.req_body.replace('\n', "\n    ")));
                }
                if n.error.is_none() {
                    s.push_str("  Response Headers\n");
                    for (k, v) in &n.resp_headers {
                        s.push_str(&format!("    {}: {}\n", k, v));
                    }
                    s.push_str(&format!("  Response Body\n    {}\n", n.resp_body.replace('\n', "\n    ")));
                }
            } else {
                s.push('\n');
            }
        }
        s
    }
}

impl App {
    pub(super) fn console_panel(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        // Toolbar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(4.0);
            let issues = self.issue_count();
            for (t, label) in [
                (ConsoleTab::Console, "Console".to_string()),
                (ConsoleTab::Issues, if issues > 0 { format!("Issues ({})", issues) } else { "Issues".into() }),
            ] {
                let sel = self.console.tab == t;
                let b = egui::Button::new(RichText::new(label).color(if sel { p.text } else { p.weak }))
                    .fill(if sel { p.selected } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE);
                if ui.add(b).clicked() {
                    self.console.tab = t;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add_space(4.0);
                if x_button(ui, Vec2::splat(24.0), dark).on_hover_text("Close console").clicked() {
                    self.console.open = false;
                }
                ui.menu_button("•••", |ui| {
                    ui.checkbox(&mut self.console.timestamps, "Show timestamps");
                    if ui.button("Expand all").clicked() {
                        let ids: Vec<u64> = self.console.entries.iter().filter(|e| e.net.is_some()).map(|e| e.id).collect();
                        self.console.expanded.extend(ids);
                        ui.close();
                    }
                    if ui.button("Collapse all").clicked() {
                        self.console.expanded.clear();
                        ui.close();
                    }
                });
                let copy = icon_button(ui, "copy", "", dark, "Copy logs");
                if copy.clicked() {
                    ui.ctx().copy_text(self.console.as_text());
                    let now = ui.ctx().input(|i| i.time);
                    self.toast("Console logs copied", true, now);
                }
                let has = !self.console.entries.is_empty();
                if ui.add_enabled(has, egui::Button::new("Clear").min_size(Vec2::new(70.0, 26.0))).clicked() {
                    self.console.entries.clear();
                    self.console.expanded.clear();
                }
                egui::ComboBox::from_id_salt("logfilter").selected_text(self.console.filter.label()).width(110.0).show_ui(ui, |ui| {
                    for f in LogFilter::ALL {
                        ui.selectable_value(&mut self.console.filter, f, f.label());
                    }
                });
                ui.add(egui::TextEdit::singleline(&mut self.console.search).hint_text("Search messages").desired_width(180.0));
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().line_segment([Pos2::new(r.left(), r.top() + 2.0), Pos2::new(r.right(), r.top() + 2.0)], Stroke::new(1.0, p.border));
        ui.add_space(4.0);

        match self.console.tab {
            ConsoleTab::Console => self.console_logs(ui),
            ConsoleTab::Issues => self.console_issues(ui),
        }
    }

    fn issue_count(&self) -> usize {
        self.load_errors.len() + self.gstat.conflicts.len()
    }

    fn console_issues(&mut self, ui: &mut Ui) {
        let p = self.pal();
        egui::ScrollArea::vertical().id_salt("issues").auto_shrink([false, false]).show(ui, |ui| {
            if self.issue_count() == 0 {
                empty_state(ui, "No issues", "Problems reading the project or git conflicts show up here.", p);
                return;
            }
            for e in &self.load_errors {
                ui.label(RichText::new(format!("⚠ {}", e)).monospace().color(p.err));
            }
            for (_, path) in &self.gstat.conflicts {
                ui.label(RichText::new(format!("⚠ Git conflict: {}", pretty_path(path))).monospace().color(p.err));
            }
            if !self.gstat.conflicts.is_empty() && ui.button("Resolve in Team Sync").clicked() {
                self.side = SideView::Git;
            }
        });
    }

    fn console_logs(&mut self, ui: &mut Ui) {
        let p = self.pal();
        let dark = self.dark();
        let vis: Vec<u64> = self.console.visible().iter().map(|e| e.id).collect();
        if vis.is_empty() {
            if self.console.entries.is_empty() {
                empty_state(ui, "No logs yet", "Send a request to view its details in the console.", p);
            } else {
                empty_state(ui, "Nothing matches", "Change the filter or search to see more logs.", p);
            }
            return;
        }
        let mut toggle: Option<u64> = None;
        let mut copy: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("consolelogs").auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            for e in self.console.entries.iter().filter(|e| vis.contains(&e.id)) {
                let open = self.console.expanded.contains(&e.id);
                let w = ui.available_width();
                let opens = e.net.is_some() || e.text.contains('\n');
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), if opens { Sense::click() } else { Sense::hover() });
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, p.hover);
                }
                let mut child = ui.new_child(
                    egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(6.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
                );
                child.spacing_mut().item_spacing.x = 8.0;
                child.style_mut().interaction.selectable_labels = false;
                // caret
                let (cr, _) = child.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                if opens {
                    let c = cr.center();
                    let pts = if open {
                        vec![c + Vec2::new(-3.5, -1.8), c + Vec2::new(3.5, -1.8), c + Vec2::new(0.0, 2.5)]
                    } else {
                        vec![c + Vec2::new(-1.8, -3.5), c + Vec2::new(-1.8, 3.5), c + Vec2::new(2.5, 0.0)]
                    };
                    child.painter().add(egui::Shape::convex_polygon(pts, p.weak, Stroke::NONE));
                }
                let level_col = match e.level {
                    LogLevel::Error => p.err,
                    LogLevel::Warn => p.warn,
                    _ => p.text,
                };
                match &e.net {
                    Some(n) => {
                        child.label(RichText::new(&n.method).monospace().strong().color(method_color(&n.method, dark)));
                        let mut url = n.url.clone();
                        if url.chars().count() > 140 {
                            url = url.chars().take(140).collect::<String>() + "…";
                        }
                        child.label(RichText::new(url).monospace().color(level_col));
                        if !n.source.is_empty() {
                            child.label(RichText::new(&n.source).small().color(p.weak));
                        }
                    }
                    None => {
                        let first = e.text.lines().next().unwrap_or("");
                        child.add(egui::Label::new(RichText::new(first).monospace().color(level_col)).truncate());
                    }
                }
                // right side: status, time, timestamp
                child.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.console.timestamps {
                        ui.label(RichText::new(&e.at).small().monospace().color(p.weak));
                    }
                    if let Some(n) = &e.net {
                        match &n.error {
                            Some(_) => {
                                ui.label(RichText::new("Error").strong().color(p.err));
                            }
                            None => {
                                ui.label(RichText::new(format!("{} ms", n.time_ms)).monospace().color(p.weak));
                                ui.label(RichText::new(format!("{}", n.status)).monospace().strong().color(status_color(n.status, dark)));
                            }
                        }
                    }
                });
                if resp.clicked() {
                    toggle = Some(e.id);
                }
                if let Some(n) = &e.net {
                    resp.context_menu(|ui| {
                        if ui.button("Copy URL").clicked() {
                            copy = Some(n.url.clone());
                            ui.close();
                        }
                        if ui.button("Copy response body").clicked() {
                            copy = Some(n.resp_body.clone());
                            ui.close();
                        }
                    });
                }
                if open {
                    match &e.net {
                        Some(n) => {
                            ui.push_id(e.id, |ui| net_details(ui, n, p, dark));
                        }
                        None => {
                            ui.push_id(e.id, |ui| {
                                egui::Frame::new()
                                    .fill(p.header)
                                    .corner_radius(4)
                                    .inner_margin(Margin { left: 26, right: 10, top: 6, bottom: 8 })
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        body_text(ui, &e.text, dark);
                                    });
                            });
                        }
                    }
                }
                // thin separator
                let y = ui.cursor().top();
                ui.painter()
                    .line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(1.0, p.border.gamma_multiply(0.6)));
            }
        });
        if let Some(id) = toggle
            && !self.console.expanded.remove(&id)
        {
            self.console.expanded.insert(id);
        }
        if let Some(t) = copy {
            ui.ctx().copy_text(t);
        }
    }
}

fn empty_state(ui: &mut Ui, title: &str, sub: &str, p: Palette) {
    let r = ui.available_rect_before_wrap();
    let c = r.center();
    ui.painter().text(c - Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, title, FontId::proportional(15.0), p.text);
    ui.painter().text(c + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, sub, FontId::proportional(13.0), p.weak);
    ui.allocate_rect(Rect::from_min_size(r.min, Vec2::new(r.width(), r.height().max(40.0))), Sense::hover());
}

fn net_details(ui: &mut Ui, n: &NetLog, p: Palette, dark: bool) {
    egui::Frame::new().fill(p.header).corner_radius(4).inner_margin(Margin { left: 26, right: 10, top: 6, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().interact_size.y = 18.0;
        if let Some(err) = &n.error {
            ui.label(RichText::new(err).monospace().color(p.err));
            ui.add_space(4.0);
        }
        let mut meta = vec![];
        if n.error.is_none() {
            meta.push(format!("Status: {} {}", n.status, n.status_text));
            meta.push(format!("Size: {}", human_size(n.size)));
        }
        meta.push(format!("Time: {} ms", n.time_ms));
        if n.attempts > 1 {
            meta.push(format!("Tries: {} (connection was dropped and retried)", n.attempts));
        }
        ui.label(RichText::new(meta.join("   ·   ")).small().color(p.weak));
        ui.add_space(2.0);
        section(ui, "Request Headers", true, |ui| kv_lines(ui, &n.req_headers, p));
        if !n.req_body.is_empty() {
            section(ui, "Request Body", false, |ui| body_text(ui, &n.req_body, dark));
        }
        if n.error.is_none() {
            section(ui, "Response Headers", false, |ui| kv_lines(ui, &n.resp_headers, p));
            section(ui, "Response Body", false, |ui| body_text(ui, &n.resp_body, dark));
        }
        if !n.tests.is_empty() {
            section(ui, "Tests", true, |ui| {
                for (name, ok, d) in &n.tests {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(if *ok { "PASS" } else { "FAIL" }).small().strong().color(if *ok { p.ok } else { p.err }));
                        ui.label(RichText::new(name).small());
                        if !ok {
                            ui.label(RichText::new(d).small().color(p.weak));
                        }
                    });
                }
            });
        }
    });
}

fn section(ui: &mut Ui, title: &str, open: bool, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong().small()).default_open(open).show(ui, add);
}

fn kv_lines(ui: &mut Ui, kv: &[(String, String)], p: Palette) {
    if kv.is_empty() {
        ui.label(RichText::new("(none)").small().color(p.weak));
    }
    for (k, v) in kv {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(format!("{}:", k)).monospace().color(p.weak));
            ui.label(RichText::new(v).monospace());
        });
    }
}

fn body_text(ui: &mut Ui, body: &str, dark: bool) {
    let mut t = body;
    let looks_json = body.trim_start().starts_with('{') || body.trim_start().starts_with('[');
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        if looks_json {
            let mut lay =
                |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.fonts_mut(|f| f.layout_job(json_job(buf.as_str(), dark, wrap)));
            ui.add(egui::TextEdit::multiline(&mut t).frame(egui::Frame::NONE).desired_width(f32::INFINITY).layouter(&mut lay));
        } else {
            ui.add(
                egui::TextEdit::multiline(&mut t).frame(egui::Frame::NONE).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY),
            );
        }
    });
}
