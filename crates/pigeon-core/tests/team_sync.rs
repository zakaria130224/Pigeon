mod common;

use common::*;
use pigeon_core::git::GitOp;
use pigeon_core::model::*;
use pigeon_core::workspace;
use std::process::Command;

#[test]
fn team_push_pull_and_conflict() {
    let root = tmp("team");
    let bare = root.join("remote.git");
    assert!(Command::new("git").args(["init", "--bare", "-b", "main"]).arg(&bare).output().unwrap().status.success());
    let remote = bare.display().to_string();

    // Alice sets up the shared workspace and pushes.
    let alice = root.join("alice");
    std::fs::create_dir_all(&alice).unwrap();
    let mut coll = sample_collection();
    let env = sample_environment();
    workspace::save(&alice, std::slice::from_ref(&coll), std::slice::from_ref(&env)).unwrap();
    let r = run(&alice, GitOp::Init { remote: String::new() });
    // Identity may be missing on CI machines: set it and commit again.
    identity(&alice, "Alice");
    let _ = r;
    let r = run(&alice, GitOp::SetRemote(remote.clone()));
    assert!(r.ok, "{}", r.message);
    let r = run(&alice, GitOp::Push("first".into()));
    assert!(r.ok, "push failed: {} {}", r.message, r.output);

    // Bob clones it.
    let bob = root.join("bob");
    let r = run(&root, GitOp::Clone { url: remote.clone(), dest: bob.clone() });
    assert!(r.ok, "clone failed: {} {}", r.message, r.output);
    identity(&bob, "Bob");
    let lb = workspace::load(&bob);
    assert_eq!(lb.collections.len(), 1);
    assert_eq!(lb.collections[0].items, coll.items);

    // Bob adds a request and pushes; Alice pulls and sees it.
    let mut bcoll = lb.collections[0].clone();
    bcoll.items.push(Item::Request(Request { name: "Bob's request".into(), url: "https://example.com".into(), ..Default::default() }));
    workspace::save(&bob, std::slice::from_ref(&bcoll), &lb.environments).unwrap();
    let r = run(&bob, GitOp::Push("Add request".into()));
    assert!(r.ok, "{} {}", r.message, r.output);
    let r = run(&alice, GitOp::Pull(String::new()));
    assert!(r.ok, "{} {}", r.message, r.output);
    let la = workspace::load(&alice);
    assert_eq!(count_requests(&la.collections[0].items), count_requests(&coll.items) + 1);
    coll = la.collections[0].clone();

    // Both edit the same request differently -> conflict on pull.
    let edit = |c: &mut Collection, url: &str| {
        if let Item::Folder(f) = &mut c.items[0]
            && let Item::Request(r) = &mut f.items[0]
        {
            r.url = url.into();
        }
    };
    let mut a = coll.clone();
    edit(&mut a, "https://alice.example");
    workspace::save(&alice, std::slice::from_ref(&a), &la.environments).unwrap();
    let mut b = workspace::load(&bob).collections[0].clone();
    let benv = workspace::load(&bob).environments;
    let r = run(&bob, GitOp::Pull(String::new()));
    assert!(r.ok, "{} {}", r.message, r.output);
    b = workspace::load(&bob).collections.into_iter().next().unwrap_or(b);
    edit(&mut b, "https://bob.example");
    workspace::save(&bob, std::slice::from_ref(&b), &benv).unwrap();
    assert!(run(&bob, GitOp::Push("bob edit".into())).ok);

    let r = run(&alice, GitOp::Pull("alice edit".into()));
    assert!(!r.ok, "expected a conflict");
    assert!(!r.status.conflicts.is_empty());
    assert!(!workspace::load(&alice).errors.is_empty(), "conflicted file should not parse");

    // Alice keeps her version, finishes the merge and pushes.
    let r = run(&alice, GitOp::KeepMine(r.status.conflicts.clone()));
    assert!(r.ok, "{} {}", r.message, r.output);
    let r = run(&alice, GitOp::FinishMerge);
    assert!(r.ok, "{} {}", r.message, r.output);
    let l = workspace::load(&alice);
    assert!(l.errors.is_empty(), "{:?}", l.errors);
    let url = match &l.collections[0].items[0] {
        Item::Folder(f) => match &f.items[0] {
            Item::Request(r) => r.url.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    };
    assert_eq!(url, "https://alice.example");
    assert!(run(&alice, GitOp::Push(String::new())).ok);
    assert!(run(&bob, GitOp::Pull(String::new())).ok);
    let _ = std::fs::remove_dir_all(&root);
}
