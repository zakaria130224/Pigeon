mod common;

use common::*;
use pigeon_core::http;
use pigeon_core::suggest::{self, Trigger};

fn suggestion_vars() -> http::Vars {
    let mut s = env_scopes();
    s.environment.insert("token".into(), "dev-123".into());
    s.environment.insert("baseUrl".into(), "https://dev.test".into());
    s.collection.insert("baseUrl".into(), "https://shared.test".into());
    s.collection.insert("pageSize".into(), "20".into());
    s.globals.insert("appKey".into(), "k".into());
    s.vars()
}

/// Text with `|` marking the cursor.
fn at(marked: &str, trigger: Trigger) -> (String, Option<suggest::Partial>) {
    let cursor = marked.chars().position(|c| c == '|').unwrap();
    let text = marked.replacen('|', "", 1);
    let partial = suggest::find_partial(&text, cursor, trigger);
    (text, partial)
}

fn names(marked: &str, trigger: Trigger) -> Vec<String> {
    let vars = suggestion_vars();
    let (_, partial) = at(marked, trigger);
    suggest::candidates(&vars, &partial.unwrap()).iter().map(|h| h.name.clone()).collect()
}

fn accept(marked: &str, trigger: Trigger, name: &str) -> String {
    let (mut text, partial) = at(marked, trigger);
    let cursor = suggest::insert(&mut text, &partial.unwrap(), name);
    let mut out: Vec<char> = text.chars().collect();
    out.insert(cursor, '|');
    out.into_iter().collect()
}

#[test]
fn suggestions_appear_in_the_right_places() {
    let t = Trigger::Template;
    assert_eq!(at("{{|", t).1.unwrap().query, "");
    assert_eq!(at("https://x/{{ba|", t).1.unwrap().query, "ba");
    assert_eq!(at("{{ ba|", t).1.unwrap().query, "ba");
    assert_eq!(at("a {{b}} {{c|", t).1.unwrap().query, "c");
    assert_eq!(at("{{নাম|", t).1.unwrap().query, "নাম");
    for none in ["|", "plain|", "{|", "{{a}}|", "{{a}} b|", "{{{|", "{{a/b|", "{\"a\": 1|"] {
        assert!(at(none, t).1.is_none(), "{}", none);
    }
    let s = Trigger::Script;
    assert_eq!(at("pm.environment.get(\"to|", s).1.unwrap().query, "to");
    assert_eq!(at("const a = pm.variables.get('|", s).1.unwrap().query, "");
    assert_eq!(at("pm.globals.set(`a|", s).1.unwrap().query, "a");
    assert_eq!(at("pm.variables.replaceIn(\"{{to|", s).1.unwrap().query, "to");
    for none in ["console.log(\"to|", "x.get(\"to|", "pm.environment.get(|", "pm.response.get(\"a|"] {
        assert!(at(none, s).1.is_none(), "{}", none);
    }
    assert!(at("pm.environment.get(\"to|", t).1.is_none(), "scripts only");
}

#[test]
fn suggestions_rank_and_filter() {
    let t = Trigger::Template;
    // Everything is offered, with fresh values after real variables.
    let all = names("{{|", t);
    assert_eq!(&all[..4], ["baseUrl", "token", "pageSize", "appKey"]);
    assert_eq!(all.len(), 10);
    // The start of a name comes first, then the middle, then scattered letters. Case doesn't matter.
    assert_eq!(names("{{PA|", t)[0], "pageSize");
    assert_eq!(names("{{key|", t), ["appKey"]);
    assert_eq!(names("{{bsu|", t), ["baseUrl"]);
    assert_eq!(names("{{$g|", t), ["$guid"]);
    assert!(names("{{nothing|", t).is_empty());
    // A script only offers what its own scope holds, and never the fresh values.
    let s = Trigger::Script;
    assert_eq!(names("pm.environment.get(\"|", s), ["baseUrl", "token"]);
    assert_eq!(names("pm.collectionVariables.get(\"|", s), ["pageSize"]);
    assert_eq!(names("pm.globals.get(\"|", s), ["appKey"]);
    assert_eq!(names("pm.variables.get(\"|", s), ["baseUrl", "token", "pageSize", "appKey"]);
    // The value shown is the one that wins: the environment beats the collection.
    let vars = suggestion_vars();
    let base = vars.hints.iter().find(|h| h.name == "baseUrl").unwrap();
    assert_eq!(base.value, "https://dev.test");
    assert_eq!(base.place, "Environment: Dev");
    assert_eq!(vars.hints.iter().filter(|h| h.name == "baseUrl").count(), 1);
}

#[test]
fn accepting_a_suggestion_writes_the_right_text() {
    let t = Trigger::Template;
    assert_eq!(accept("{{|", t, "token"), "{{token}}|");
    assert_eq!(accept("https://{{ba|/users", t, "baseUrl"), "https://{{baseUrl}}|/users");
    assert_eq!(accept("{{ ba|", t, "baseUrl"), "{{baseUrl}}|");
    // Inside an existing variable the whole name is replaced, without doubling the braces.
    assert_eq!(accept("{{to|ken}}/x", t, "pageSize"), "{{pageSize}}|/x");
    assert_eq!(accept("{{|}}", t, "token"), "{{token}}|");
    // A word after the cursor that isn't part of a variable stays.
    assert_eq!(accept("{{|word", t, "token"), "{{token}}|word");
    assert_eq!(accept("নাম {{to|", t, "token"), "নাম {{token}}|");
    let s = Trigger::Script;
    assert_eq!(accept("pm.environment.get(\"to|", s, "token"), "pm.environment.get(\"token\"|");
    assert_eq!(accept("pm.environment.get('to|')", s, "token"), "pm.environment.get('token'|)");
    assert_eq!(accept("pm.environment.get(\"to|ken\");", s, "baseUrl"), "pm.environment.get(\"baseUrl\"|);");
}

#[test]
fn pointing_at_a_variable_and_catching_typos() {
    let text = "https://{{baseUrl}}/a/{{ id }}";
    assert_eq!(suggest::var_at(text, 12).as_deref(), Some("baseUrl"));
    assert_eq!(suggest::var_at(text, 8).as_deref(), Some("baseUrl"));
    assert_eq!(suggest::var_at(text, 26).as_deref(), Some("id"));
    assert_eq!(suggest::var_at(text, 3), None);
    assert_eq!(suggest::var_at(text, 20), None);
    assert_eq!(suggest::var_at("{{open", 3), None);
    let vars = suggestion_vars();
    assert_eq!(suggest::closest("baseURL", &vars), Some("baseUrl"));
    assert_eq!(suggest::closest("tokne", &vars), Some("token"));
    assert_eq!(suggest::closest("pagesize", &vars), Some("pageSize"));
    assert_eq!(suggest::closest("somethingelse", &vars), None);
    assert_eq!(suggest::closest("x", &vars), None);
}
