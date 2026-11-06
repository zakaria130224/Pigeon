//! Import of OpenAPI 3.x JSON documents into a collection.
//!
//! - The first server becomes the `baseUrl` collection variable.
//! - Operations are grouped into folders by their first tag (or first path segment).
//! - `{id}` in a path becomes `{{id}}`, with a sample value in the collection variables.
//! - Optional query/header parameters are imported switched off.
//! - Request bodies come from the document's examples, or are generated from its schemas.

use crate::model::*;
use serde_json::{Map, Value};

const HTTP_METHODS: [&str; 8] = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];
static NULL: Value = Value::Null;

/// Find a local `#/...` reference.
fn pointer<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    root.pointer(reference.strip_prefix('#')?)
}

/// Follow `$ref`s until a real object is reached.
fn deref<'a>(root: &'a Value, v: &'a Value) -> &'a Value {
    let mut cur = v;
    for _ in 0..32 {
        match cur.get("$ref").and_then(Value::as_str) {
            Some(r) => match pointer(root, r) {
                Some(next) => cur = next,
                None => return &NULL,
            },
            None => return cur,
        }
    }
    &NULL
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Builds example values from schemas. Recursive schemas stop at the point they repeat.
struct Sampler<'a> {
    root: &'a Value,
    stack: Vec<String>,
    budget: usize,
}

impl<'a> Sampler<'a> {
    fn new(root: &'a Value) -> Self {
        Sampler { root, stack: vec![], budget: 5000 }
    }

    /// `None` means "stop here": a repeating reference, or the document is too large to expand.
    fn sample(&mut self, schema: &Value) -> Option<Value> {
        if self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
            if self.stack.iter().any(|s| s == r) {
                return None;
            }
            let target = pointer(self.root, r)?;
            self.stack.push(r.to_string());
            let out = self.sample(target);
            self.stack.pop();
            return out;
        }
        for key in ["example", "default", "const"] {
            if let Some(v) = schema.get(key).filter(|v| !v.is_null()) {
                return Some(v.clone());
            }
        }
        for key in ["examples", "enum"] {
            if let Some(v) = schema.get(key).and_then(Value::as_array).and_then(|a| a.first()) {
                return Some(v.clone());
            }
        }
        if let Some(all) = schema.get("allOf").and_then(Value::as_array) {
            let mut merged = Map::new();
            let mut other = None;
            for part in all {
                match self.sample(part) {
                    Some(Value::Object(m)) => merged.extend(m),
                    Some(v) => other = Some(v),
                    None => {}
                }
            }
            merged.extend(self.properties(schema));
            return Some(match other {
                Some(v) if merged.is_empty() => v,
                _ => Value::Object(merged),
            });
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(list) = schema.get(key).and_then(Value::as_array)
                && let Some(v) = list.iter().find_map(|s| self.sample(s))
            {
                return Some(v);
            }
        }
        let kind = match schema.get("type") {
            Some(Value::String(s)) => s.as_str(),
            // OpenAPI 3.1 allows ["string", "null"].
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).find(|t| *t != "null").unwrap_or(""),
            _ if schema.get("properties").is_some() => "object",
            _ if schema.get("items").is_some() => "array",
            _ => "",
        };
        Some(match kind {
            "array" => {
                let item = schema.get("items").and_then(|i| self.sample(i));
                Value::Array(item.into_iter().collect())
            }
            "string" => Value::String(sample_string(schema).to_string()),
            "integer" | "number" => schema.get("minimum").filter(|m| m.is_number()).cloned().unwrap_or_else(|| Value::from(1)),
            "boolean" => Value::Bool(false),
            _ => {
                let mut map = self.properties(schema);
                if let Some(extra) = schema.get("additionalProperties").filter(|a| a.is_object())
                    && let Some(v) = self.sample(extra)
                {
                    map.insert("key".into(), v);
                }
                Value::Object(map)
            }
        })
    }

    fn properties(&mut self, schema: &Value) -> Map<String, Value> {
        let mut out = Map::new();
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            for (name, prop) in props {
                // Read-only fields are only ever sent by the server.
                if deref(self.root, prop).get("readOnly").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if let Some(v) = self.sample(prop) {
                    out.insert(name.clone(), v);
                }
            }
        }
        out
    }
}

fn sample_string(schema: &Value) -> &'static str {
    match schema.get("format").and_then(Value::as_str).unwrap_or("") {
        "date-time" => "2024-01-01T00:00:00Z",
        "date" => "2024-01-01",
        "time" => "00:00:00Z",
        "uuid" => "00000000-0000-0000-0000-000000000000",
        "email" => "user@example.com",
        "uri" | "url" => "https://example.com",
        "ipv4" => "127.0.0.1",
        "binary" | "byte" => "",
        _ => "string",
    }
}

/// An example from a media type or parameter object, or one built from its schema.
fn example_of(root: &Value, holder: &Value) -> Value {
    if let Some(v) = holder.get("example").filter(|v| !v.is_null()) {
        return v.clone();
    }
    if let Some(list) = holder.get("examples").and_then(Value::as_object)
        && let Some(v) = list.values().find_map(|e| deref(root, e).get("value"))
    {
        return v.clone();
    }
    holder.get("schema").and_then(|s| Sampler::new(root).sample(s)).unwrap_or(Value::Null)
}

fn add_var(vars: &mut Vec<Kv>, name: &str, value: &str, description: &str) {
    if !vars.iter().any(|k| k.key == name) {
        vars.push(Kv { key: name.into(), value: value.into(), enabled: true, description: description.into() });
    }
}

/// The URL of a `servers` list's first entry, with its `{variables}` filled in.
fn server_url(servers: Option<&Value>) -> Option<String> {
    let server = servers?.as_array()?.first()?;
    let mut url = server.get("url")?.as_str()?.trim().to_string();
    if let Some(vars) = server.get("variables").and_then(Value::as_object) {
        for (name, var) in vars {
            url = url.replace(&format!("{{{}}}", name), &var.get("default").map(scalar).unwrap_or_default());
        }
    }
    Some(url.trim_end_matches('/').to_string())
}

fn scheme_auth(root: &Value, name: &str, vars: &mut Vec<Kv>) -> Option<Auth> {
    let schemes = root.get("components")?.get("securitySchemes")?;
    let scheme = deref(root, schemes.get(name)?);
    let variable = |vars: &mut Vec<Kv>, key: &str| {
        add_var(vars, key, "", "Fill this in to sign in");
        format!("{{{{{}}}}}", key)
    };
    match text(scheme, "type").as_str() {
        "http" => match text(scheme, "scheme").to_lowercase().as_str() {
            "bearer" => Some(Auth { kind: AuthKind::Bearer, token: variable(vars, "bearerToken"), ..Default::default() }),
            "basic" => Some(Auth {
                kind: AuthKind::Basic,
                username: variable(vars, "basicUsername"),
                password: variable(vars, "basicPassword"),
                ..Default::default()
            }),
            _ => None,
        },
        "apiKey" => {
            let key = text(scheme, "name");
            let value = variable(vars, "apiKey");
            Some(match text(scheme, "in").as_str() {
                "cookie" => {
                    Auth { kind: AuthKind::ApiKey, key: "Cookie".into(), value: format!("{}={}", key, value), ..Default::default() }
                }
                place => Auth { kind: AuthKind::ApiKey, key, value, in_query: place == "query", ..Default::default() },
            })
        }
        // There is no sign-in flow here: paste a token you got elsewhere.
        "oauth2" | "openIdConnect" => Some(Auth { kind: AuthKind::Bearer, token: variable(vars, "accessToken"), ..Default::default() }),
        _ => None,
    }
}

fn security_auth(root: &Value, security: &Value, vars: &mut Vec<Kv>) -> Auth {
    let requirements = security.as_array().map(Vec::as_slice).unwrap_or(&[]);
    requirements
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|req| req.keys())
        .find_map(|name| scheme_auth(root, name, vars))
        .unwrap_or_else(Auth::none)
}

/// Parameters of an operation, including those shared by its path. The operation's win.
fn parameters<'a>(root: &'a Value, path_item: &'a Value, op: &'a Value) -> Vec<&'a Value> {
    let mut out: Vec<&Value> = vec![];
    for holder in [path_item, op] {
        for p in holder.get("parameters").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let p = deref(root, p);
            let same = |q: &&Value| text(q, "name") == text(p, "name") && text(q, "in") == text(p, "in");
            match out.iter().position(same) {
                Some(i) => out[i] = p,
                None => out.push(p),
            }
        }
    }
    out
}

fn parameter_value(root: &Value, p: &Value) -> Value {
    if let Some((_, media)) = p.get("content").and_then(Value::as_object).and_then(|c| c.iter().next()) {
        return example_of(root, media);
    }
    example_of(root, p)
}

fn apply_parameters(root: &Value, path_item: &Value, op: &Value, req: &mut Request, vars: &mut Vec<Kv>) {
    let mut cookies: Vec<String> = vec![];
    let mut cookie_required = false;
    for p in parameters(root, path_item, op) {
        let name = text(p, "name");
        let description = text(p, "description");
        let required = p.get("required").and_then(Value::as_bool) == Some(true);
        let value = parameter_value(root, p);
        let row = |key: &str, value: &Value| Kv {
            key: key.into(),
            value: query_text(value),
            enabled: required,
            description: description.clone(),
        };
        match text(p, "in").as_str() {
            "path" => add_var(vars, &name, &query_text(&value), &description),
            "query" => match &value {
                // Objects in a query are spread into one parameter per field, unless told otherwise.
                Value::Object(fields) if p.get("explode").and_then(Value::as_bool) != Some(false) => {
                    req.params.extend(fields.iter().map(|(k, v)| row(k, v)));
                }
                _ => req.params.push(row(&name, &value)),
            },
            "header" => {
                // OpenAPI says these three are described elsewhere and must be ignored here.
                let lower = name.to_lowercase();
                if !matches!(lower.as_str(), "accept" | "content-type" | "authorization") {
                    req.headers.push(Kv { key: name, value: scalar_or_json(&value), enabled: required, description });
                }
            }
            "cookie" => {
                cookies.push(format!("{}={}", name, scalar_or_json(&value)));
                cookie_required |= required;
            }
            _ => {}
        }
    }
    if !cookies.is_empty() {
        req.headers.push(Kv { key: "Cookie".into(), value: cookies.join("; "), enabled: cookie_required, description: String::new() });
    }
}

fn scalar_or_json(v: &Value) -> String {
    match v {
        Value::Array(_) | Value::Object(_) => v.to_string(),
        other => scalar(other),
    }
}

/// Text for a query value: lists use their first item, and URL separators are escaped.
fn query_text(v: &Value) -> String {
    let v = match v {
        Value::Array(a) => a.first().unwrap_or(&NULL),
        other => other,
    };
    scalar_or_json(v)
        .replace('%', "%25")
        .replace('&', "%26")
        .replace('#', "%23")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('/', "%2F")
}

fn media_rank(media: &str) -> u8 {
    let m = media.to_lowercase();
    if m.contains("json") {
        0
    } else if m.contains("x-www-form-urlencoded") {
        1
    } else if m.contains("multipart/form-data") {
        2
    } else if m.contains("xml") {
        3
    } else if m.starts_with("text/") {
        4
    } else {
        5
    }
}

fn best_media(content: &Map<String, Value>) -> Option<(&String, &Value)> {
    content.iter().min_by_key(|(k, _)| media_rank(k))
}

fn form_fields(root: &Value, media: &Value) -> Vec<Kv> {
    let schema = media.get("schema").map(|s| deref(root, s)).unwrap_or(&NULL);
    let props = schema.get("properties").and_then(Value::as_object);
    let Value::Object(fields) = example_of(root, media) else { return vec![] };
    fields
        .iter()
        .map(|(key, value)| {
            let prop = props.and_then(|p| p.get(key)).map(|p| deref(root, p)).unwrap_or(&NULL);
            let is_file = prop.get("format").and_then(Value::as_str) == Some("binary");
            Kv {
                key: key.clone(),
                value: scalar_or_json(value),
                enabled: !is_file,
                description: if is_file { "File field: uploading files isn't supported yet".into() } else { text(prop, "description") },
            }
        })
        .collect()
}

fn apply_body(root: &Value, op: &Value, req: &mut Request) {
    let Some(body) = op.get("requestBody").map(|b| deref(root, b)) else { return };
    let Some((media_type, media)) = body.get("content").and_then(Value::as_object).and_then(best_media) else { return };
    let plain = media_type.split(';').next().unwrap_or("").trim().to_lowercase();
    match media_rank(&plain) {
        1 => req.body = Body { mode: BodyMode::UrlEncoded, urlencoded: form_fields(root, media), ..Default::default() },
        2 => req.body = Body { mode: BodyMode::FormData, formdata: form_fields(root, media), ..Default::default() },
        rank => {
            let lang = match rank {
                0 => RawLang::Json,
                3 => RawLang::Xml,
                _ if plain == "text/html" => RawLang::Html,
                _ => RawLang::Text,
            };
            let raw = match example_of(root, media) {
                Value::String(s) => s,
                Value::Null => String::new(),
                // A generated object is only meaningful as JSON.
                v if lang == RawLang::Json => serde_json::to_string_pretty(&v).unwrap_or_default(),
                _ => String::new(),
            };
            if plain != lang.content_type() && !plain.is_empty() {
                req.headers.push(Kv::new("Content-Type", &plain));
            }
            req.body = Body { mode: BodyMode::Raw, raw, raw_lang: lang, ..Default::default() };
        }
    }
}

fn apply_accept(root: &Value, op: &Value, req: &mut Request) {
    let Some(responses) = op.get("responses").and_then(Value::as_object) else { return };
    let chosen = responses.iter().find(|(code, _)| code.starts_with('2')).or_else(|| responses.iter().find(|(code, _)| *code == "default"));
    let Some((_, response)) = chosen else { return };
    let content = deref(root, response).get("content").and_then(Value::as_object);
    if let Some((media, _)) = content.and_then(best_media)
        && media != "*/*"
    {
        req.headers.push(Kv::new("Accept", media));
    }
}

fn request_name(op: &Value, method: &str, path: &str) -> String {
    let summary = text(op, "summary");
    let first_line = summary.lines().next().unwrap_or("").trim();
    if !first_line.is_empty() {
        return first_line.to_string();
    }
    let id = text(op, "operationId");
    if !id.is_empty() {
        return id;
    }
    format!("{} {}", method.to_uppercase(), path)
}

/// `/users/{id}` -> `/users/{{id}}`
fn path_with_variables(path: &str) -> String {
    path.replace('{', "{{").replace('}', "}}")
}

/// The folder an operation belongs in: its first tag, or the start of its path.
fn folder_name(op: &Value, path: &str) -> Option<String> {
    let tag = op.get("tags").and_then(Value::as_array).and_then(|t| t.first()).and_then(Value::as_str).map(str::trim);
    if let Some(tag) = tag.filter(|t| !t.is_empty()) {
        return Some(tag.to_string());
    }
    path.split('/').find(|s| !s.is_empty()).filter(|s| !s.contains('{')).map(str::to_string)
}

pub fn import(root: &Value) -> Result<Collection, String> {
    let version = root.get("openapi").and_then(Value::as_str).unwrap_or("");
    if !version.starts_with("3.") {
        return Err(format!(
            "This OpenAPI version ({}) isn't supported. Use OpenAPI 3.",
            if version.is_empty() { "unknown" } else { version }
        ));
    }
    let info = root.get("info").unwrap_or(&NULL);
    let title = text(info, "title");
    let mut c = Collection::new(if title.is_empty() { "Imported API" } else { &title });
    c.description = text(info, "description");

    let mut vars = vec![];
    let base = server_url(root.get("servers")).unwrap_or_default();
    // A relative or missing server means "wherever this document is hosted", which an import can't know.
    let base = if base.contains("://") { base } else { format!("http://localhost{}", base) };
    add_var(&mut vars, "baseUrl", &base, "Server address from the OpenAPI document");
    if let Some(security) = root.get("security") {
        c.auth = security_auth(root, security, &mut vars);
    }

    let mut folders: Vec<Folder> = vec![];
    let mut loose: Vec<Item> = vec![];
    for (path, path_item) in root.get("paths").and_then(Value::as_object).into_iter().flatten() {
        let path_item = deref(root, path_item);
        for method in HTTP_METHODS {
            let Some(op) = path_item.get(method) else { continue };
            let servers = op.get("servers").or_else(|| path_item.get("servers"));
            let prefix = server_url(servers).filter(|u| !u.is_empty()).unwrap_or_else(|| "{{baseUrl}}".to_string());
            let mut req = Request {
                name: request_name(op, method, path),
                method: method.to_uppercase(),
                url: format!("{}{}", prefix, path_with_variables(path)),
                description: text(op, "description"),
                ..Default::default()
            };
            apply_parameters(root, path_item, op, &mut req, &mut vars);
            apply_body(root, op, &mut req);
            if !req.headers.iter().any(|h| h.key.eq_ignore_ascii_case("accept")) {
                apply_accept(root, op, &mut req);
            }
            if let Some(security) = op.get("security") {
                let auth = security_auth(root, security, &mut vars);
                if auth != c.auth {
                    req.auth = auth;
                }
            }
            req.url = url_with_params(&req.url, &req.params);
            match folder_name(op, path) {
                Some(name) => {
                    let i = match folders.iter().position(|f| f.name == name) {
                        Some(i) => i,
                        None => {
                            let tags = root.get("tags").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                            let description =
                                tags.iter().find(|t| text(t, "name") == name).map(|t| text(t, "description")).unwrap_or_default();
                            folders.push(Folder {
                                id: new_id(),
                                name,
                                description,
                                auth: Auth::default(),
                                scripts: Scripts::default(),
                                items: vec![],
                            });
                            folders.len() - 1
                        }
                    };
                    folders[i].items.push(Item::Request(req));
                }
                None => loose.push(Item::Request(req)),
            }
        }
    }
    c.variables = vars;
    c.items = folders.into_iter().map(Item::Folder).chain(loose).collect();
    Ok(c)
}
