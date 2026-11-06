//! Reading and writing a workspace folder.
//!
//! The layout is designed to merge well in git when several people work on it:
//!
//! ```text
//! <workspace>/collections/<Collection>/collection.json
//! <workspace>/collections/<Collection>/<Folder>/folder.json
//! <workspace>/collections/<Collection>/<Folder>/<Request>.request.json
//! <workspace>/environments/<Environment>.env.json
//! ```
//!
//! Each request lives in its own small file, so two people changing different
//! requests never touch the same file.

use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const COLLECTION_FILE: &str = "collection.json";
pub const FOLDER_FILE: &str = "folder.json";
pub const REQUEST_EXT: &str = ".request.json";
pub const ENV_EXT: &str = ".env.json";

#[derive(Default)]
pub struct Loaded {
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub errors: Vec<String>,
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let s = s.trim().trim_matches('.').trim().to_string();
    if s.is_empty() {
        return "Untitled".into();
    }
    // File names are limited to 255 bytes, and non-English letters take several bytes each.
    let mut out = String::new();
    for c in s.chars().take(80) {
        if out.len() + c.len_utf8() > 200 {
            break;
        }
        out.push(c);
    }
    let out = out.trim_end_matches(['.', ' ']).to_string();
    // Windows refuses these names, whatever comes after them.
    let stem = out.split('.').next().unwrap_or("").trim_end().to_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.ends_with(|c: char| c.is_ascii_digit()));
    if reserved { format!("{}_", out) } else { out }
}

fn unique_name(name: &str, id: &str, used: &mut HashSet<String>) -> String {
    let base = sanitize(name);
    let mut cand = base.clone();
    if used.contains(&cand.to_lowercase()) {
        cand = format!("{} ({})", base, &id[id.len().saturating_sub(4)..]);
        let mut n = 2;
        while used.contains(&cand.to_lowercase()) {
            cand = format!("{} ({})", base, n);
            n += 1;
        }
    }
    used.insert(cand.to_lowercase());
    cand
}

fn to_json<T: Serialize>(v: &T, order: Option<usize>) -> String {
    let mut val = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    if let (Some(o), Some(map)) = (order, val.as_object_mut()) {
        map.insert("order".into(), serde_json::Value::from(o));
    }
    let mut s = serde_json::to_string_pretty(&val).unwrap_or_default();
    s.push('\n');
    s
}

fn plan_items(dir: &Path, items: &[Item], out: &mut BTreeMap<PathBuf, String>) {
    let mut used = HashSet::new();
    used.insert(FOLDER_FILE.trim_end_matches(".json").to_lowercase());
    used.insert(COLLECTION_FILE.trim_end_matches(".json").to_lowercase());
    for (i, it) in items.iter().enumerate() {
        match it {
            Item::Request(r) => {
                let n = unique_name(&r.name, &r.id, &mut used);
                out.insert(dir.join(format!("{}{}", n, REQUEST_EXT)), to_json(r, Some(i)));
            }
            Item::Folder(f) => {
                let n = unique_name(&f.name, &f.id, &mut used);
                let sub = dir.join(n);
                out.insert(sub.join(FOLDER_FILE), to_json(f, Some(i)));
                plan_items(&sub, &f.items, out);
            }
        }
    }
}

/// All files the workspace should contain, with their contents.
pub fn plan(root: &Path, collections: &[Collection], envs: &[Environment]) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let cdir = root.join("collections");
    let mut used = HashSet::new();
    for c in collections {
        let n = unique_name(&c.name, &c.id, &mut used);
        let d = cdir.join(n);
        out.insert(d.join(COLLECTION_FILE), to_json(c, None));
        plan_items(&d, &c.items, &mut out);
    }
    let edir = root.join("environments");
    let mut used = HashSet::new();
    for e in envs {
        let n = unique_name(&e.name, &e.id, &mut used);
        out.insert(edir.join(format!("{}{}", n, ENV_EXT)), to_json(e, None));
    }
    out
}

fn is_managed(p: &Path) -> bool {
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name == COLLECTION_FILE || name == FOLDER_FILE || name.ends_with(REQUEST_EXT) || name.ends_with(ENV_EXT)
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn remove_empty_dirs(dir: &Path, keep_root: bool) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            remove_empty_dirs(&p, false);
        }
    }
    if !keep_root {
        let _ = fs::remove_dir(dir); // only succeeds when empty
    }
}

/// Write the workspace to disk. Only changed files are touched and files for
/// deleted/renamed items are removed. Returns number of files written+removed.
pub fn save(root: &Path, collections: &[Collection], envs: &[Environment]) -> Result<usize, String> {
    let planned = plan(root, collections, envs);
    let mut changes = 0;
    for (path, content) in &planned {
        let same = fs::read_to_string(path).map(|c| c.replace("\r\n", "\n") == *content).unwrap_or(false);
        if !same {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
            }
            fs::write(path, content).map_err(|e| format!("{}: {}", path.display(), e))?;
            changes += 1;
        }
    }
    for sub in ["collections", "environments"] {
        let d = root.join(sub);
        let mut files = vec![];
        walk_files(&d, &mut files);
        for f in files {
            if is_managed(&f) && !planned.contains_key(&f) {
                let _ = fs::remove_file(&f);
                changes += 1;
            }
        }
        remove_empty_dirs(&d, true);
    }
    Ok(changes)
}

#[derive(Deserialize)]
struct Ordered {
    #[serde(default)]
    order: usize,
}

fn read_json<T: for<'de> Deserialize<'de>>(p: &Path, errors: &mut Vec<String>) -> Option<(T, usize)> {
    match fs::read_to_string(p) {
        Ok(s) => {
            if s.contains("<<<<<<<") && s.contains(">>>>>>>") {
                errors.push(format!("{} has a git conflict", p.display()));
                return None;
            }
            match serde_json::from_str::<T>(&s) {
                Ok(v) => {
                    let o = serde_json::from_str::<Ordered>(&s).map(|o| o.order).unwrap_or(usize::MAX);
                    Some((v, o))
                }
                Err(e) => {
                    errors.push(format!("{}: {}", p.display(), e));
                    None
                }
            }
        }
        Err(e) => {
            errors.push(format!("{}: {}", p.display(), e));
            None
        }
    }
}

fn load_items(dir: &Path, errors: &mut Vec<String>) -> Vec<Item> {
    let mut items: Vec<(usize, String, Item)> = vec![];
    let Ok(rd) = fs::read_dir(dir) else { return vec![] };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            let ff = p.join(FOLDER_FILE);
            if ff.exists()
                && let Some((mut f, o)) = read_json::<Folder>(&ff, errors)
            {
                f.items = load_items(&p, errors);
                items.push((o, f.name.to_lowercase(), Item::Folder(f)));
            }
        } else if p.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.ends_with(REQUEST_EXT))
            && let Some((r, o)) = read_json::<Request>(&p, errors)
        {
            items.push((o, r.name.to_lowercase(), Item::Request(r)));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    // Make ids unique in case two people copied the same file.
    let mut seen = HashSet::new();
    items
        .into_iter()
        .map(|(_, _, mut it)| {
            if !seen.insert(it.id().to_string()) {
                match &mut it {
                    Item::Request(r) => r.id = new_id(),
                    Item::Folder(f) => f.id = new_id(),
                }
            }
            it
        })
        .collect()
}

pub fn load(root: &Path) -> Loaded {
    let mut out = Loaded::default();
    let cdir = root.join("collections");
    if let Ok(rd) = fs::read_dir(&cdir) {
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            let cf = d.join(COLLECTION_FILE);
            if !cf.exists() {
                continue;
            }
            if let Some((mut c, _)) = read_json::<Collection>(&cf, &mut out.errors) {
                c.items = load_items(&d, &mut out.errors);
                out.collections.push(c);
            }
        }
    }
    let edir = root.join("environments");
    if let Ok(rd) = fs::read_dir(&edir) {
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            if f.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.ends_with(ENV_EXT))
                && let Some((e, _)) = read_json::<Environment>(&f, &mut out.errors)
            {
                out.environments.push(e);
            }
        }
    }
    out
}
