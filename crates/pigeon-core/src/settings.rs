//! Personal settings and history: these stay on this computer and are never shared.

use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    #[serde(default)]
    pub workspace: Option<PathBuf>,
    #[serde(default)]
    pub active_env: Option<String>,
    #[serde(default)]
    pub dark: bool,
    #[serde(default = "yes")]
    pub pull_on_start: bool,
    #[serde(default)]
    pub globals: Vec<Kv>,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
    #[serde(default = "yes")]
    pub follow_redirects: bool,
    /// Values scripts wrote to an environment or collection, by its id. These stay on this
    /// computer so a token set by a script is never pushed to the team. `None` hides a shared value.
    #[serde(default)]
    pub script_vars: BTreeMap<String, BTreeMap<String, Option<String>>>,
    /// Project folders opened on this computer, most recent first.
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
}

/// How many folders the recent projects list keeps.
pub const MAX_RECENT_PROJECTS: usize = 10;

fn yes() -> bool {
    true
}
fn default_timeout() -> u64 {
    30
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            workspace: None,
            active_env: None,
            dark: false,
            pull_on_start: true,
            globals: vec![],
            timeout_s: 30,
            follow_redirects: true,
            script_vars: BTreeMap::new(),
            recent_projects: vec![],
        }
    }
}

impl Settings {
    /// Puts a project at the top of the recent list.
    pub fn remember_project(&mut self, path: &Path) {
        self.forget_project(path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
    }

    /// Takes a project off the recent list. Its folder is left alone.
    pub fn forget_project(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
    }
}

/// The name a project is shown under: its folder's name.
pub fn project_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string())
}

/// Where a project is, short enough for a list.
pub fn project_location(path: &Path) -> String {
    shorten_path(path, dirs::home_dir().as_deref(), 42)
}

/// Writes the home folder as `~` and cuts off the start of a long path, since the end says the most.
pub fn shorten_path(path: &Path, home: Option<&Path>, most: usize) -> String {
    let under_home = home.and_then(|h| path.strip_prefix(h).ok()).map(|rest| Path::new("~").join(rest));
    let text = under_home.as_deref().unwrap_or(path).display().to_string();
    let count = text.chars().count();
    if count <= most { text } else { format!("…{}", text.chars().skip(count + 1 - most).collect::<String>()) }
}

pub fn app_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(crate::APP_NAME)
}

pub fn default_workspace() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir).join("Pigeon Workspace")
}

pub fn load_settings() -> Settings {
    fs::read_to_string(app_dir().join("settings.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    let d = app_dir();
    let _ = fs::create_dir_all(&d);
    let _ = fs::write(d.join("settings.json"), serde_json::to_string_pretty(s).unwrap_or_default());
}

pub fn load_history() -> Vec<HistoryEntry> {
    fs::read_to_string(app_dir().join("history.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_history(h: &[HistoryEntry]) {
    let d = app_dir();
    let _ = fs::create_dir_all(&d);
    let _ = fs::write(d.join("history.json"), serde_json::to_string(h).unwrap_or_default());
}

/// Copies settings and history from the app's previous name, the first time Pigeon starts.
/// Nothing is removed, and a workspace keeps working from wherever it already is.
pub fn adopt_previous_data() {
    if let Some(data) = dirs::data_dir() {
        adopt_from(&data.join("Dispatch"), &app_dir());
    }
}

/// See [`adopt_previous_data`]. Does nothing once the new folder exists.
pub fn adopt_from(old: &Path, new: &Path) {
    if new.exists() || !old.exists() || fs::create_dir_all(new).is_err() {
        return;
    }
    for name in ["settings.json", "history.json"] {
        let _ = fs::copy(old.join(name), new.join(name));
    }
}
