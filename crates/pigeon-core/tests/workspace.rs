mod common;

use common::*;
use pigeon_core::model::*;
use pigeon_core::settings::{MAX_RECENT_PROJECTS, Settings, project_name, shorten_path};
use pigeon_core::workspace;
use std::path::{Path, PathBuf};

#[test]
fn save_and_load_round_trip() {
    let d = tmp("roundtrip");
    let c = sample_collection();
    let e = sample_environment();
    workspace::save(&d, std::slice::from_ref(&c), std::slice::from_ref(&e)).unwrap();
    let l = workspace::load(&d);
    assert!(l.errors.is_empty(), "{:?}", l.errors);
    assert_eq!(l.collections.len(), 1);
    assert_eq!(l.collections[0].items, c.items);
    assert_eq!(l.environments[0], e);
    // Renaming a request removes the old file.
    let mut c2 = c.clone();
    if let Item::Folder(f) = &mut c2.items[0]
        && let Item::Request(r) = &mut f.items[0]
    {
        r.name = "All users".into();
    }
    workspace::save(&d, std::slice::from_ref(&c2), std::slice::from_ref(&e)).unwrap();
    assert!(!d.join("collections/Sample API/Users/List users.request.json").exists());
    assert!(d.join("collections/Sample API/Users/All users.request.json").exists());
    assert_eq!(workspace::load(&d).collections[0].items, c2.items);
}

#[test]
fn long_names_still_save() {
    let d = tmp("longnames");
    let mut c = Collection::new("Long");
    for _ in 0..2 {
        c.items.push(Item::Request(Request { name: "নাম".repeat(60), ..Default::default() }));
    }
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    assert_eq!(count_requests(&workspace::load(&d).collections[0].items), 2);
}

#[test]
fn file_names_work_on_every_system() {
    let d = tmp("names");
    let mut c = Collection::new("AUX");
    for name in ["CON", "nul", "com1", "LPT9", "con.fig", "dots...", "a/b:c*d?", "Trailing "] {
        c.items.push(Item::Request(Request { name: name.into(), ..Default::default() }));
    }
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    let mut files = vec![];
    for entry in std::fs::read_dir(d.join("collections/AUX_")).unwrap() {
        files.push(entry.unwrap().file_name().to_string_lossy().to_string());
    }
    files.sort();
    println!("{files:?}");
    for bad in ["CON.request.json", "nul.request.json", "com1.request.json", "LPT9.request.json", "con.fig.request.json"] {
        assert!(!files.iter().any(|f| f == bad), "{}", bad);
    }
    assert!(!files.iter().any(|f| f.contains("..") || f.contains(':') || f.contains('?')));
    assert_eq!(count_requests(&workspace::load(&d).collections[0].items), 8);
    // A file git turned into Windows line endings is not rewritten.
    let path = d.join("collections/AUX_/collection.json");
    let crlf = std::fs::read_to_string(&path).unwrap().replace('\n', "\r\n");
    std::fs::write(&path, &crlf).unwrap();
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), crlf);
}

#[test]
fn settings_carry_over_from_the_previous_name_once() {
    let d = tmp("adopt");
    let (old, new) = (d.join("old"), d.join("new"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("settings.json"), "from old").unwrap();
    std::fs::write(old.join("history.json"), "[]").unwrap();
    pigeon_core::settings::adopt_from(&old, &new);
    assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "from old");
    assert!(old.join("settings.json").exists(), "the old copy stays");
    // Later settings are never replaced.
    std::fs::write(new.join("settings.json"), "newer").unwrap();
    pigeon_core::settings::adopt_from(&old, &new);
    assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "newer");
}

#[test]
fn recent_projects_keep_the_latest_first_without_repeats() {
    let mut s = Settings::default();
    s.remember_project(Path::new("/work/one"));
    s.remember_project(Path::new("/work/two"));
    s.remember_project(Path::new("/work/one/"));
    assert_eq!(s.recent_projects, [PathBuf::from("/work/one"), PathBuf::from("/work/two")]);
    s.forget_project(Path::new("/work/one"));
    assert_eq!(s.recent_projects, [PathBuf::from("/work/two")]);
}

#[test]
fn recent_projects_drop_the_oldest_past_the_limit() {
    let mut s = Settings::default();
    for i in 0..=MAX_RECENT_PROJECTS {
        s.remember_project(&PathBuf::from(format!("/work/{i}")));
    }
    assert_eq!(s.recent_projects.len(), MAX_RECENT_PROJECTS);
    assert_eq!(s.recent_projects[0], PathBuf::from(format!("/work/{MAX_RECENT_PROJECTS}")));
    assert!(!s.recent_projects.contains(&PathBuf::from("/work/0")));
}

#[test]
fn settings_saved_before_recent_projects_still_load() {
    let s: Settings = serde_json::from_str(r#"{"workspace":"/work/one"}"#).unwrap();
    assert!(s.recent_projects.is_empty());
    assert_eq!(s.workspace, Some(PathBuf::from("/work/one")));
}

#[test]
fn a_project_is_named_after_its_folder() {
    assert_eq!(project_name(Path::new("/work/payments-api")), "payments-api");
    assert_eq!(project_name(Path::new("/work/payments-api/")), "payments-api");
    assert_eq!(project_name(Path::new("/")), "/");
}

#[test]
fn long_project_paths_keep_their_end() {
    let home = Some(Path::new("/Users/sam"));
    assert_eq!(shorten_path(Path::new("/Users/sam/work/api"), home, 40), Path::new("~/work/api").display().to_string());
    assert_eq!(shorten_path(Path::new("/srv/api"), home, 40), Path::new("/srv/api").display().to_string());
    let short = shorten_path(Path::new("/srv/teams/payments/api"), None, 10);
    assert_eq!(short.chars().count(), 10);
    assert!(short.starts_with('…') && short.ends_with("api"), "{short}");
}
