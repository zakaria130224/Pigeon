//! Runs JavaScript before and after requests.
//!
//! Order for one send, like the original: collection, then folders from the outside in, then the
//! request's own script. Every script gets a fresh sandbox; variables are the only thing that
//! carries over. All of this happens on a background thread.

use crate::http::{self, DYNAMIC_VARS, Prepared, ResponseData, VarHint, VarKind, Vars};
use crate::model::*;
use rquickjs::{Context, Ctx, Error as JsError, Function, Runtime};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

const PRELUDE: &str = include_str!("prelude.js");
const MEMORY_LIMIT: usize = 256 * 1024 * 1024;
const MAX_LOGS: usize = 1000;

type Values = BTreeMap<String, String>;

/// Every variable a script can see or change.
#[derive(Clone, Debug, Default)]
pub struct Scopes {
    pub environment: Values,
    pub has_environment: bool,
    pub environment_name: String,
    pub collection: Values,
    pub globals: Values,
    pub local: Values,
}

impl Scopes {
    pub fn vars(&self) -> Vars {
        let mut vars = Vars::default();
        for scope in [&self.globals, &self.collection, &self.environment, &self.local] {
            vars.map.extend(scope.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        // In the order they win, so a name shows up with the value it really has.
        let environment = format!("Environment: {}", self.environment_name);
        for (scope, kind, place) in [
            (&self.environment, VarKind::Environment, environment.as_str()),
            (&self.collection, VarKind::Collection, "Collection variable"),
            (&self.globals, VarKind::Global, "Global variable"),
        ] {
            for (name, value) in scope {
                if !self.local.contains_key(name) && !vars.hints.iter().any(|h| &h.name == name) {
                    vars.hints.push(VarHint { name: name.clone(), value: value.clone(), kind, place: place.to_string() });
                }
            }
        }
        vars.hints.extend(DYNAMIC_VARS.iter().map(|(name, about)| VarHint {
            name: name.to_string(),
            value: about.to_string(),
            kind: VarKind::Dynamic,
            place: "Filled in fresh for each send".to_string(),
        }));
        vars
    }
}

#[derive(Clone)]
pub struct Job {
    pub req: Request,
    pub auth_chain: Vec<Auth>,
    /// Outermost first, ending with the request's own scripts.
    pub scripts: Vec<Scripts>,
    pub timeout_s: u64,
    pub follow_redirects: bool,
    pub iteration: usize,
    pub iteration_count: usize,
    pub script_limit: Duration,
}

pub struct LogLine {
    pub level: String,
    pub text: String,
}

/// `None` as a value means the variable was removed.
#[derive(Default)]
pub struct Changes {
    pub environment: Vec<(String, Option<String>)>,
    pub collection: Vec<(String, Option<String>)>,
    pub globals: Vec<(String, Option<String>)>,
}

#[derive(Default)]
pub struct Phase {
    pub logs: Vec<LogLine>,
    pub calls: Vec<ResponseData>,
}

pub struct Outcome {
    pub resp: ResponseData,
    pub before: Phase,
    pub after: Phase,
    pub changes: Changes,
    /// `Some(None)` stops a collection run, `Some(Some(name))` jumps to that request.
    pub next: Option<Option<String>>,
}

pub fn spawn(job: Job, mut scopes: Scopes) -> Receiver<Outcome> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(&job, &mut scopes));
    });
    rx
}

fn diff(before: &Values, after: &Values) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> =
        after.iter().filter(|(k, v)| before.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), Some(v.clone()))).collect();
    out.extend(before.keys().filter(|k| !after.contains_key(*k)).map(|k| (k.clone(), None)));
    out
}

/// Runs the whole send: pre-request scripts, the request, post-response scripts and checks.
pub fn run(job: &Job, scopes: &mut Scopes) -> Outcome {
    let start = scopes.clone();
    let mut req = job.req.clone();
    let mut tests: Vec<(String, bool, String)> = vec![];
    let mut before = Phase::default();
    let mut after = Phase::default();
    let mut next = None;

    let pre = run_scripts(job, "prerequest", &mut req, scopes, None, &mut before, &mut tests, &mut next);
    let vars = scopes.vars();
    let auth = http::effective_auth(&req, &job.auth_chain);
    let prepared = http::prepare(&req, &auth, &vars);
    let mut resp = match pre {
        Stage::Failed(error) => not_sent(&prepared, format!("A pre-request script failed: {}", error)),
        Stage::Skip => not_sent(&prepared, "Not sent: a pre-request script skipped this request.".to_string()),
        Stage::Continue => http::send_blocking(&prepared, job.timeout_s, job.follow_redirects),
    };

    if resp.error.is_none() {
        if let Stage::Failed(error) = run_scripts(job, "test", &mut req, scopes, Some(&resp), &mut after, &mut tests, &mut next) {
            resp.script_error = Some(error);
        }
        let mut all = http::run_checks(&req.checks, &resp, &scopes.vars());
        all.append(&mut tests);
        resp.tests = all;
    } else {
        resp.tests = tests;
    }
    let changes = Changes {
        environment: diff(&start.environment, &scopes.environment),
        collection: diff(&start.collection, &scopes.collection),
        globals: diff(&start.globals, &scopes.globals),
    };
    Outcome { resp, before, after, changes, next }
}

fn not_sent(prepared: &Prepared, error: String) -> ResponseData {
    ResponseData { final_url: prepared.url.clone(), sent: Some(prepared.clone()), error: Some(error), ..Default::default() }
}

enum Stage {
    Continue,
    Skip,
    Failed(String),
}

#[allow(clippy::too_many_arguments)]
fn run_scripts(
    job: &Job,
    event: &str,
    req: &mut Request,
    scopes: &mut Scopes,
    response: Option<&ResponseData>,
    phase: &mut Phase,
    tests: &mut Vec<(String, bool, String)>,
    next: &mut Option<Option<String>>,
) -> Stage {
    let is_pre = event == "prerequest";
    let mut skip = false;
    for scripts in &job.scripts {
        let code = if is_pre { &scripts.pre } else { &scripts.post };
        if code.trim().is_empty() {
            continue;
        }
        let input = script_input(job, event, req, scopes, response);
        let calls = Rc::new(RefCell::new(vec![]));
        let result = execute(code, &input, job, calls.clone());
        phase.calls.append(&mut calls.borrow_mut());
        let output = match result {
            Ok(v) => v,
            Err(e) => {
                phase.logs.push(LogLine { level: "error".into(), text: e.clone() });
                return Stage::Failed(e);
            }
        };
        for line in output.get("logs").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().rev().take(MAX_LOGS).rev() {
            phase.logs.push(LogLine { level: text(line, "level"), text: text(line, "text") });
        }
        for t in output.get("tests").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            tests.push((text(t, "name"), t.get("passed").and_then(Value::as_bool).unwrap_or(false), text(t, "error")));
        }
        read_values(&output, "local", &mut scopes.local);
        read_values(&output, "collection", &mut scopes.collection);
        read_values(&output, "globals", &mut scopes.globals);
        if scopes.has_environment {
            read_values(&output, "environment", &mut scopes.environment);
        }
        match output.get("next") {
            Some(Value::Null) => *next = Some(None),
            Some(Value::String(name)) => *next = Some(Some(name.clone())),
            _ => {}
        }
        if is_pre {
            apply_request(output.get("request").unwrap_or(&Value::Null), req);
            skip |= output.get("skip").and_then(Value::as_bool).unwrap_or(false);
        }
        if let Some(error) = output.get("error").and_then(Value::as_str) {
            phase.logs.push(LogLine { level: "error".into(), text: error.to_string() });
            return Stage::Failed(error.to_string());
        }
    }
    if skip { Stage::Skip } else { Stage::Continue }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn read_values(output: &Value, key: &str, into: &mut Values) {
    if let Some(map) = output.get(key).and_then(Value::as_object) {
        *into = map.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect();
    }
}

fn kv_json(list: &[Kv]) -> Value {
    Value::Array(list.iter().filter(|k| !k.is_blank()).map(|k| json!({"key": k.key, "value": k.value, "disabled": !k.enabled})).collect())
}

fn kv_read(v: Option<&Value>) -> Vec<Kv> {
    v.and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|i| Kv {
                    enabled: i.get("disabled").and_then(Value::as_bool) != Some(true),
                    ..Kv::new(&text(i, "key"), &text(i, "value"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn script_input(job: &Job, event: &str, req: &Request, scopes: &Scopes, response: Option<&ResponseData>) -> String {
    let mode = match req.body.mode {
        BodyMode::None => "none",
        BodyMode::Raw => "raw",
        BodyMode::UrlEncoded => "urlencoded",
        BodyMode::FormData => "formdata",
    };
    let input = json!({
        "event": event,
        "request": {
            "id": req.id,
            "name": req.name,
            "method": req.method,
            "url": req.url,
            "headers": kv_json(&req.headers),
            "body": { "mode": mode, "raw": req.body.raw, "urlencoded": kv_json(&req.body.urlencoded), "formdata": kv_json(&req.body.formdata) },
        },
        "response": response.map(response_json),
        "local": scopes.local,
        "environment": scopes.environment,
        "hasEnvironment": scopes.has_environment,
        "environmentName": scopes.environment_name,
        "collection": scopes.collection,
        "globals": scopes.globals,
        "info": {
            "requestName": req.name,
            "requestId": req.id,
            "iteration": job.iteration,
            "iterationCount": job.iteration_count,
        },
    });
    input.to_string()
}

fn response_json(r: &ResponseData) -> Value {
    let headers: Vec<Value> = r.headers.iter().map(|(k, v)| json!({"key": k, "value": v})).collect();
    json!({"code": r.status, "status": r.status_text, "time": r.time_ms, "size": r.size, "headers": headers, "body": r.body})
}

fn apply_request(out: &Value, req: &mut Request) {
    if !out.is_object() {
        return;
    }
    req.method = text(out, "method");
    req.url = text(out, "url");
    req.headers = kv_read(out.get("headers"));
    let Some(body) = out.get("body") else { return };
    req.body.mode = match text(body, "mode").as_str() {
        "raw" => BodyMode::Raw,
        "urlencoded" => BodyMode::UrlEncoded,
        "formdata" => BodyMode::FormData,
        _ => BodyMode::None,
    };
    req.body.raw = text(body, "raw");
    req.body.urlencoded = kv_read(body.get("urlencoded"));
    req.body.formdata = kv_read(body.get("formdata"));
}

/// Carries out a `pm.sendRequest` call.
fn host_http(job_text: &str, job: &Job, calls: &Rc<RefCell<Vec<ResponseData>>>) -> String {
    let spec: Value = serde_json::from_str(job_text).unwrap_or(Value::Null);
    let headers = spec
        .get("headers")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(|h| Some((h.get(0)?.as_str()?.to_string(), h.get(1)?.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let prepared = Prepared {
        method: text(&spec, "method"),
        url: encode_url_loose(&text(&spec, "url")),
        headers,
        body: spec.get("body").and_then(Value::as_str).map(|b| b.as_bytes().to_vec()),
    };
    let resp = http::send_blocking(&prepared, job.timeout_s, job.follow_redirects);
    let out = match &resp.error {
        Some(e) => json!({"error": e}),
        None => response_json(&resp),
    };
    calls.borrow_mut().push(resp);
    out.to_string()
}

fn js_error(ctx: &Ctx<'_>, e: JsError) -> String {
    if !e.is_exception() {
        return e.to_string();
    }
    let thrown = ctx.catch();
    match thrown.as_exception() {
        Some(ex) => ex.message().unwrap_or_else(|| "The script failed".to_string()),
        None => "The script failed".to_string(),
    }
}

/// Runs one script in a fresh sandbox and returns what it produced.
fn execute(code: &str, input: &str, job: &Job, calls: Rc<RefCell<Vec<ResponseData>>>) -> Result<Value, String> {
    let started = Instant::now();
    let limit_ms = job.script_limit.as_millis() as u64;
    // Milliseconds after `started` at which the script is stopped. Time spent waiting
    // for a pm.sendRequest answer doesn't count against the script.
    let deadline = Arc::new(AtomicU64::new(limit_ms));
    let past_deadline = {
        let deadline = deadline.clone();
        move || started.elapsed().as_millis() as u64 > deadline.load(Ordering::Relaxed)
    };
    let too_long = || format!("The script ran for more than {} seconds and was stopped.", job.script_limit.as_secs().max(1));
    // Being stopped surfaces as an error from inside the engine, so explain it properly.
    let explain = |e: String| if past_deadline() { too_long() } else { e };

    let rt = Runtime::new().map_err(|e| e.to_string())?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_interrupt_handler(Some(Box::new(past_deadline.clone())));
    let context = Context::full(&rt).map_err(|e| e.to_string())?;

    let job_copy = job.clone();
    let http_deadline = deadline.clone();
    context
        .with(|ctx| {
            let setup = || -> rquickjs::Result<()> {
                let globals = ctx.globals();
                let host = move |spec: String| -> String {
                    let call_started = Instant::now();
                    let answer = host_http(&spec, &job_copy, &calls);
                    http_deadline.fetch_add(call_started.elapsed().as_millis() as u64, Ordering::Relaxed);
                    answer
                };
                globals.set("__http", Function::new(ctx.clone(), host)?)?;
                ctx.eval::<(), _>(PRELUDE)?;
                globals.get::<_, Function>("__init")?.call::<_, ()>((input.to_string(),))?;
                globals.get::<_, Function>("__run")?.call::<_, ()>((code.to_string(),))
            };
            setup().map_err(|e| js_error(&ctx, e))
        })
        .map_err(explain)?;

    // Promises first, then timers, until nothing is left to do.
    let mut stopped = false;
    loop {
        while rt.is_job_pending() {
            if past_deadline() {
                stopped = true;
                break;
            }
            if let Err(failed) = rt.execute_pending_job() {
                let message = failed.0.with(|ctx| js_error(&ctx, JsError::Exception));
                return Err(explain(message));
            }
        }
        if stopped || past_deadline() {
            stopped = true;
            break;
        }
        let wait = context
            .with(|ctx| {
                let next: rquickjs::Result<f64> = ctx.globals().get::<_, Function>("__nextTimer").and_then(|f| f.call(()));
                next.map_err(|e| js_error(&ctx, e))
            })
            .map_err(explain)?;
        if wait < 0.0 {
            break;
        }
        let remaining = deadline.load(Ordering::Relaxed).saturating_sub(started.elapsed().as_millis() as u64);
        std::thread::sleep(Duration::from_millis((wait as u64).min(remaining)));
        if past_deadline() {
            stopped = true;
            break;
        }
        context
            .with(|ctx| {
                let fired: rquickjs::Result<()> = ctx.globals().get::<_, Function>("__fireTimer").and_then(|f| f.call(()));
                fired.map_err(|e| js_error(&ctx, e))
            })
            .map_err(explain)?;
    }
    if stopped || past_deadline() {
        return Err(too_long());
    }
    let output = context.with(|ctx| {
        let text: rquickjs::Result<String> = ctx.globals().get::<_, Function>("__finish").and_then(|f| f.call(()));
        text.map_err(|e| js_error(&ctx, e))
    })?;
    serde_json::from_str(&output).map_err(|e| e.to_string())
}
