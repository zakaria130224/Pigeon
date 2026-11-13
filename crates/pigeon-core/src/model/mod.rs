//! Data model: collections, folders, requests, environments, history.

mod bulk;
mod samples;
mod tree;
mod url;

pub use bulk::*;
pub use samples::*;
pub use tree::*;
pub use url::*;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A reasonably unique id (time + process counter + random-ish bits).
pub fn new_id() -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mix = (t as u64) ^ (std::process::id() as u64).rotate_left(32) ^ c.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{:012x}{:04x}", (t as u64) & 0xffff_ffff_ffff, mix & 0xffff)
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Kv {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Kv {
    pub fn new(k: &str, v: &str) -> Self {
        Kv { key: k.into(), value: v.into(), enabled: true, description: String::new() }
    }
    pub fn is_blank(&self) -> bool {
        self.key.is_empty() && self.value.is_empty() && self.description.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    #[default]
    Inherit,
    None,
    Bearer,
    Basic,
    ApiKey,
}

impl AuthKind {
    pub const ALL: [AuthKind; 5] = [AuthKind::Inherit, AuthKind::None, AuthKind::Bearer, AuthKind::Basic, AuthKind::ApiKey];
    pub fn label(self) -> &'static str {
        match self {
            AuthKind::Inherit => "Inherit auth from parent",
            AuthKind::None => "No Auth",
            AuthKind::Bearer => "Bearer Token",
            AuthKind::Basic => "Basic Auth",
            AuthKind::ApiKey => "API Key",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Auth {
    #[serde(default)]
    pub kind: AuthKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// For API key: true = query param, false = header.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_query: bool,
}

impl Auth {
    pub fn none() -> Self {
        Auth { kind: AuthKind::None, ..Default::default() }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BodyMode {
    #[default]
    None,
    Raw,
    UrlEncoded,
    FormData,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawLang {
    #[default]
    Json,
    Text,
    Xml,
    Html,
    JavaScript,
}

impl RawLang {
    pub const ALL: [RawLang; 5] = [RawLang::Text, RawLang::JavaScript, RawLang::Json, RawLang::Html, RawLang::Xml];
    pub fn label(self) -> &'static str {
        match self {
            RawLang::Json => "JSON",
            RawLang::Text => "Text",
            RawLang::Xml => "XML",
            RawLang::Html => "HTML",
            RawLang::JavaScript => "JavaScript",
        }
    }
    pub fn content_type(self) -> &'static str {
        match self {
            RawLang::Json => "application/json",
            RawLang::Text => "text/plain",
            RawLang::Xml => "application/xml",
            RawLang::Html => "text/html",
            RawLang::JavaScript => "application/javascript",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Body {
    #[serde(default)]
    pub mode: BodyMode,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub raw: String,
    #[serde(default)]
    pub raw_lang: RawLang,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urlencoded: Vec<Kv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formdata: Vec<Kv>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    #[default]
    StatusEquals,
    BodyContains,
    TimeBelow,
    HeaderExists,
    JsonEquals,
}

impl CheckKind {
    pub const ALL: [CheckKind; 5] =
        [CheckKind::StatusEquals, CheckKind::BodyContains, CheckKind::TimeBelow, CheckKind::HeaderExists, CheckKind::JsonEquals];
    pub fn label(self) -> &'static str {
        match self {
            CheckKind::StatusEquals => "Status code is",
            CheckKind::BodyContains => "Body contains",
            CheckKind::TimeBelow => "Response time below (ms)",
            CheckKind::HeaderExists => "Header is present",
            CheckKind::JsonEquals => "JSON value equals",
        }
    }
}

/// A simple, script-free test assertion.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Check {
    #[serde(default)]
    pub kind: CheckKind,
    /// JSON path for JsonEquals (e.g. `data.items[0].id`), header name for HeaderExists.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default)]
    pub expected: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// JavaScript that runs before the request is sent and after the response arrives.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Scripts {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pre: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub post: String,
}

impl Scripts {
    pub fn is_empty(&self) -> bool {
        self.pre.trim().is_empty() && self.post.trim().is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Request {
    pub id: String,
    pub name: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Kv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Kv>,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub body: Body,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

fn default_method() -> String {
    "GET".into()
}

impl Default for Request {
    fn default() -> Self {
        Request {
            id: new_id(),
            name: "New Request".into(),
            method: "GET".into(),
            url: String::new(),
            params: vec![],
            headers: vec![],
            auth: Auth::default(),
            body: Body::default(),
            checks: vec![],
            scripts: Scripts::default(),
            description: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
    #[serde(skip)]
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Folder(Folder),
    Request(Request),
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Folder(f) => &f.id,
            Item::Request(r) => &r.id,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Item::Folder(f) => &f.name,
            Item::Request(r) => &r.name,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Collection {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub auth: Auth,
    /// Sent with every request in the collection, unless the request sets a header with the same name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Kv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Kv>,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
    #[serde(skip)]
    pub items: Vec<Item>,
}

impl Collection {
    pub fn new(name: &str) -> Self {
        Collection {
            id: new_id(),
            name: name.into(),
            description: String::new(),
            auth: Auth::none(),
            headers: vec![],
            variables: vec![],
            scripts: Scripts::default(),
            items: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub values: Vec<Kv>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub at_ms: u64,
    pub request: Request,
    #[serde(default)]
    pub status: u16,
    #[serde(default)]
    pub time_ms: u64,
}
