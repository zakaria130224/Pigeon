//! Import and export of the widely used v2.1 collection / environment JSON format.

use crate::model::*;
use serde_json::{Value, json};

fn s(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn desc(v: &Value) -> String {
    match v.get("description") {
        Some(Value::String(s)) => s.clone(),
        Some(o) => o.get("content").map(s).unwrap_or_default(),
        None => String::new(),
    }
}

fn kvs(v: Option<&Value>) -> Vec<Kv> {
    v.and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| Kv {
                    key: x.get("key").map(s).unwrap_or_default(),
                    value: x.get("value").map(s).unwrap_or_default(),
                    enabled: !x.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false)
                        && x.get("enabled").and_then(|d| d.as_bool()).unwrap_or(true),
                    description: desc(x),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn auth_from(v: Option<&Value>) -> Auth {
    let Some(a) = v else { return Auth::default() };
    let typ = a.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let field = |name: &str, key: &str| -> String {
        a.get(name)
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.iter().find(|x| x.get("key").and_then(|k| k.as_str()) == Some(key)))
            .and_then(|x| x.get("value"))
            .map(s)
            .unwrap_or_default()
    };
    match typ {
        "noauth" => Auth::none(),
        "bearer" => Auth { kind: AuthKind::Bearer, token: field("bearer", "token"), ..Default::default() },
        "basic" => {
            Auth { kind: AuthKind::Basic, username: field("basic", "username"), password: field("basic", "password"), ..Default::default() }
        }
        "apikey" => Auth {
            kind: AuthKind::ApiKey,
            key: field("apikey", "key"),
            value: field("apikey", "value"),
            in_query: field("apikey", "in") == "query",
            ..Default::default()
        },
        _ => Auth::default(),
    }
}

fn auth_to(a: &Auth) -> Value {
    let kv = |k: &str, v: &str| json!({"key": k, "value": v, "type": "string"});
    match a.kind {
        AuthKind::Inherit => Value::Null,
        AuthKind::None => json!({"type": "noauth"}),
        AuthKind::Bearer => json!({"type": "bearer", "bearer": [kv("token", &a.token)]}),
        AuthKind::Basic => json!({"type": "basic", "basic": [kv("username", &a.username), kv("password", &a.password)]}),
        AuthKind::ApiKey => {
            json!({"type": "apikey", "apikey": [kv("key", &a.key), kv("value", &a.value), kv("in", if a.in_query { "query" } else { "header" })]})
        }
    }
}

/// Scripts live in an `event` list next to the request, folder or collection.
fn scripts_from(holder: &Value) -> Scripts {
    let mut out = Scripts::default();
    for e in holder.get("event").and_then(|e| e.as_array()).map(Vec::as_slice).unwrap_or(&[]) {
        let code = match e.get("script").and_then(|s| s.get("exec")) {
            Some(Value::Array(lines)) => lines.iter().map(s).collect::<Vec<_>>().join("\n"),
            Some(Value::String(text)) => text.clone(),
            _ => continue,
        };
        match e.get("listen").and_then(|l| l.as_str()) {
            Some("prerequest") => out.pre = code,
            Some("test") => out.post = code,
            _ => {}
        }
    }
    out
}

fn scripts_to(scripts: &Scripts) -> Value {
    let event = |listen: &str, code: &str| json!({"listen": listen, "script": {"type": "text/javascript", "exec": code.lines().collect::<Vec<_>>()}});
    let mut list = vec![];
    if !scripts.pre.trim().is_empty() {
        list.push(event("prerequest", &scripts.pre));
    }
    if !scripts.post.trim().is_empty() {
        list.push(event("test", &scripts.post));
    }
    Value::Array(list)
}

fn request_from(name: &str, v: &Value, item: &Value) -> Request {
    let mut r = Request { name: name.to_string(), scripts: scripts_from(item), ..Default::default() };
    if let Some(u) = v.as_str() {
        r.url = u.to_string();
        r.params = parse_query(&r.url);
        return r;
    }
    r.method = v.get("method").and_then(|m| m.as_str()).unwrap_or("GET").to_uppercase();
    r.url = match v.get("url") {
        Some(Value::String(u)) => u.clone(),
        Some(u) => u.get("raw").map(s).unwrap_or_default(),
        None => String::new(),
    };
    r.params = match v.get("url").and_then(|u| u.get("query")) {
        Some(q) => kvs(Some(q)),
        None => parse_query(&r.url),
    };
    r.headers = kvs(v.get("header"));
    r.auth = auth_from(v.get("auth"));
    r.description = desc(v);
    if let Some(b) = v.get("body") {
        match b.get("mode").and_then(|m| m.as_str()) {
            Some("raw") => {
                let lang =
                    b.get("options").and_then(|o| o.get("raw")).and_then(|o| o.get("language")).and_then(|l| l.as_str()).unwrap_or("text");
                r.body = Body {
                    mode: BodyMode::Raw,
                    raw: b.get("raw").map(s).unwrap_or_default(),
                    raw_lang: match lang {
                        "json" => RawLang::Json,
                        "xml" => RawLang::Xml,
                        "html" => RawLang::Html,
                        "javascript" => RawLang::JavaScript,
                        _ => RawLang::Text,
                    },
                    ..Default::default()
                };
            }
            Some("urlencoded") => r.body = Body { mode: BodyMode::UrlEncoded, urlencoded: kvs(b.get("urlencoded")), ..Default::default() },
            Some("formdata") => r.body = Body { mode: BodyMode::FormData, formdata: kvs(b.get("formdata")), ..Default::default() },
            _ => {}
        }
    }
    r
}

fn items_from(arr: Option<&Value>) -> Vec<Item> {
    let Some(arr) = arr.and_then(|a| a.as_array()) else { return vec![] };
    arr.iter()
        .map(|it| {
            let name = it.get("name").map(s).unwrap_or_else(|| "Untitled".into());
            if let Some(children) = it.get("item") {
                Item::Folder(Folder {
                    id: new_id(),
                    name,
                    description: desc(it),
                    auth: auth_from(it.get("auth")),
                    scripts: scripts_from(it),
                    items: items_from(Some(children)),
                })
            } else {
                let req = it.get("request").cloned().unwrap_or(Value::Null);
                Item::Request(request_from(&name, &req, it))
            }
        })
        .collect()
}

// Lives only for the moment an import is handed over, so its size doesn't matter.
#[allow(clippy::large_enum_variant)]
pub enum Imported {
    Collection(Collection),
    Environment(Environment),
}

pub fn import_json(text: &str) -> Result<Imported, String> {
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.starts_with("openapi:") || text.starts_with("swagger:") {
        return Err("This looks like YAML. Only OpenAPI files in JSON format can be imported for now.".into());
    }
    let v: Value = serde_json::from_str(text).map_err(|e| format!("Not valid JSON: {}", e))?;
    if v.get("openapi").is_some() {
        return super::openapi::import(&v).map(Imported::Collection);
    }
    if v.get("swagger").is_some() {
        return Err("This is an older Swagger 2 file. Only OpenAPI 3 can be imported.".into());
    }
    if let (Some(info), Some(items)) = (v.get("info"), v.get("item")) {
        let mut c = Collection::new(&info.get("name").map(s).unwrap_or_else(|| "Imported".into()));
        c.description = desc(info);
        c.auth = match v.get("auth") {
            Some(a) => auth_from(Some(a)),
            None => Auth::none(),
        };
        c.variables = kvs(v.get("variable"));
        c.scripts = scripts_from(&v);
        c.items = items_from(Some(items));
        return Ok(Imported::Collection(c));
    }
    if let Some(values) = v.get("values") {
        return Ok(Imported::Environment(Environment {
            id: new_id(),
            name: v.get("name").map(s).unwrap_or_else(|| "Imported".into()),
            values: kvs(Some(values)),
        }));
    }
    Err("This file is not a collection, environment or OpenAPI file.".into())
}

fn kv_out(k: &[Kv]) -> Value {
    Value::Array(
        k.iter()
            .filter(|x| !x.is_blank())
            .map(|x| {
                let mut o = json!({"key": x.key, "value": x.value});
                if !x.enabled {
                    o["disabled"] = json!(true);
                }
                if !x.description.is_empty() {
                    o["description"] = json!(x.description);
                }
                o
            })
            .collect(),
    )
}

fn items_to(items: &[Item]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|it| match it {
                Item::Folder(f) => {
                    let mut o = json!({"name": f.name, "item": items_to(&f.items)});
                    if !f.description.is_empty() {
                        o["description"] = json!(f.description);
                    }
                    let a = auth_to(&f.auth);
                    if !a.is_null() {
                        o["auth"] = a;
                    }
                    if !f.scripts.is_empty() {
                        o["event"] = scripts_to(&f.scripts);
                    }
                    o
                }
                Item::Request(r) => {
                    let mut req = json!({
                        "method": r.method,
                        "header": kv_out(&r.headers),
                        "url": {"raw": r.url, "query": kv_out(&r.params)},
                    });
                    let a = auth_to(&r.auth);
                    if !a.is_null() {
                        req["auth"] = a;
                    }
                    if !r.description.is_empty() {
                        req["description"] = json!(r.description);
                    }
                    match r.body.mode {
                        BodyMode::None => {}
                        BodyMode::Raw => {
                            req["body"] = json!({"mode": "raw", "raw": r.body.raw, "options": {"raw": {"language": r.body.raw_lang.label().to_lowercase()}}})
                        }
                        BodyMode::UrlEncoded => req["body"] = json!({"mode": "urlencoded", "urlencoded": kv_out(&r.body.urlencoded)}),
                        BodyMode::FormData => req["body"] = json!({"mode": "formdata", "formdata": kv_out(&r.body.formdata)}),
                    }
                    let mut o = json!({"name": r.name, "request": req});
                    if !r.scripts.is_empty() {
                        o["event"] = scripts_to(&r.scripts);
                    }
                    o
                }
            })
            .collect(),
    )
}

pub fn export_collection(c: &Collection) -> String {
    let mut v = json!({
        "info": {
            "name": c.name,
            "description": c.description,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": items_to(&c.items),
        "variable": kv_out(&c.variables),
    });
    let a = auth_to(&c.auth);
    if !a.is_null() {
        v["auth"] = a;
    }
    if !c.scripts.is_empty() {
        v["event"] = scripts_to(&c.scripts);
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

pub fn export_environment(e: &Environment) -> String {
    serde_json::to_string_pretty(&json!({"name": e.name, "values": kv_out(&e.values)})).unwrap_or_default()
}
