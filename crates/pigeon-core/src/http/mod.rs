//! Sending requests: variables, preparation, the network call, checks and code snippets.

mod checks;
mod client;
mod curl;
mod request;
mod snippets;
mod vars;

pub use checks::*;
pub use client::*;
pub use curl::*;
pub use request::*;
pub use snippets::*;
pub use vars::*;

#[derive(Clone, Debug, Default)]
pub struct ResponseData {
    pub status: u16,
    pub status_text: String,
    pub time_ms: u64,
    pub size: usize,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub pretty: Option<String>,
    pub error: Option<String>,
    pub final_url: String,
    pub tests: Vec<(String, bool, String)>,
    /// What actually went on the wire (for the console).
    pub sent: Option<Prepared>,
    /// How many tries it took (connection drops are retried).
    pub attempts: usize,
    /// A post-response script failed (the response itself is still fine).
    pub script_error: Option<String>,
}

impl ResponseData {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn is_json(&self) -> bool {
        self.pretty.is_some()
    }
}

/// A fully resolved request, ready to go on the wire.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}
