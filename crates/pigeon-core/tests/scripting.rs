mod common;

use common::*;
use pigeon_core::model::*;
use pigeon_core::scripting::{self, Scopes};
use pigeon_core::workspace;
use std::time::Duration;

#[test]
fn pre_request_script_changes_what_is_sent() {
    let base = echo_server();
    let mut scopes = env_scopes();
    let pre = format!(
        r#"pm.environment.set("host", "{base}");
           pm.collectionVariables.set("kept", "yes");
           pm.globals.set("g", 5);
           pm.variables.set("segment", "users");
           pm.request.headers.add({{ key: "X-One", value: "1" }});
           pm.request.headers.upsert({{ key: "X-Two", value: "2" }});
           pm.request.headers.remove("X-Gone");
           pm.request.method = "POST";
           pm.request.url.query.add({{ key: "added", value: "yes" }});
           pm.request.body = {{ mode: "raw", raw: JSON.stringify({{ n: 1 }}) }};
           console.log("host is", pm.environment.get("host"));"#
    );
    let req = Request {
        url: "{{host}}/{{segment}}".into(),
        headers: vec![Kv::new("X-Gone", "bye")],
        scripts: Scripts { pre, post: String::new() },
        ..Default::default()
    };
    let o = scripting::run(&job(req, vec![]), &mut scopes);
    assert!(o.resp.error.is_none(), "{:?}", o.resp.error);
    let sent: serde_json::Value = serde_json::from_str(&o.resp.body).unwrap();
    assert_eq!(sent["line"], "POST /users?added=yes HTTP/1.1");
    assert_eq!(sent["headers"]["x-one"], "1");
    assert_eq!(sent["headers"]["x-two"], "2");
    assert!(sent["headers"].get("x-gone").is_none());
    assert_eq!(sent["body"], "{\"n\":1}");
    assert_eq!(logged(&o), vec![format!("host is {}", base)]);
    assert_eq!(o.changes.environment, vec![("host".to_string(), Some(base))]);
    assert_eq!(o.changes.collection, vec![("kept".to_string(), Some("yes".to_string()))]);
    assert_eq!(o.changes.globals, vec![("g".to_string(), Some("5".to_string()))]);
}

#[test]
fn post_response_script_reads_the_response() {
    let base = echo_server();
    let mut scopes = env_scopes();
    let post = r#"
        pm.test("status", () => pm.response.to.have.status(200));
        pm.test("ok", () => { pm.response.to.be.ok; pm.response.to.be.success; pm.response.to.not.be.error; });
        pm.test("header", () => pm.response.to.have.header("Content-Type", "application/json"));
        pm.test("json", () => {
            const data = pm.response.json();
            pm.expect(data.items).to.have.lengthOf(2);
            pm.expect(data.items[1].id).to.eql(2);
            pm.environment.set("token", data.token);
        });
        pm.test("request visible", () => pm.expect(pm.request.url.toString()).to.include("/x"));
        pm.test("cookie", () => pm.expect(pm.cookies.get("sid")).to.equal("abc"));
        pm.test("wrong", () => pm.expect(pm.response.code).to.equal(201));
        pm.test("wrong status", () => pm.response.to.have.status(404));
    "#;
    let o = run_scripts(&format!("{}/x", base), "", post, &mut scopes);
    assert_eq!(o.resp.tests.len(), 8);
    assert_eq!(failed(&o), vec!["wrong: expected 200 to equal 201", "wrong status: expected response to have status 404 but got 200"]);
    assert_eq!(scopes.environment["token"], "tok-123");
    assert!(o.resp.script_error.is_none());
}

#[test]
fn scripts_run_outside_in_and_share_variables() {
    let base = echo_server();
    let mut scopes = Scopes::default();
    let layer = |name: &str| Scripts {
        pre: format!("pm.variables.set('order', (pm.variables.get('order') || '') + '{name}');"),
        post: format!("console.log('{name}', pm.variables.get('order'));"),
    };
    let req = Request { url: format!("{}/{{{{order}}}}", base), scripts: layer("r"), ..Default::default() };
    let o = scripting::run(&job(req, vec![layer("c"), layer("f")]), &mut scopes);
    assert_eq!(logged(&o), vec!["c cfr", "f cfr", "r cfr"]);
    assert!(o.resp.body.contains("GET /cfr "));
}

#[test]
fn checks_and_scripts_both_report() {
    let base = echo_server();
    let mut scopes = Scopes::default();
    let req = Request {
        url: base,
        checks: vec![Check { kind: CheckKind::StatusEquals, expected: "200".into(), enabled: true, target: String::new() }],
        scripts: Scripts { pre: String::new(), post: "pm.test('from script', () => {});".into() },
        ..Default::default()
    };
    let o = scripting::run(&job(req, vec![]), &mut scopes);
    assert_eq!(o.resp.tests.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["Status code is 200", "from script"]);
}

#[test]
fn script_problems_are_reported() {
    let base = echo_server();
    let url = format!("{}/x", base);
    // A failing pre-request script stops the request, for both kinds of mistake.
    for pre in ["this is not javascript(", "nothing.here();", "throw new Error('boom');"] {
        let o = run_scripts(&url, pre, "", &mut Scopes::default());
        let error = o.resp.error.clone().unwrap();
        assert!(error.starts_with("A pre-request script failed"), "{}", error);
        assert_eq!(o.resp.status, 0, "request must not be sent");
        assert!(o.before.logs.iter().any(|l| l.level == "error"));
    }
    // A failing post-response script keeps the response.
    let o = run_scripts(&url, "", "console.log('before');\nconst a = null;\na.b();", &mut Scopes::default());
    assert_eq!(o.resp.status, 200);
    let error = o.resp.script_error.clone().unwrap();
    println!("runtime error text: {error}");
    assert!(error.contains("TypeError"), "{}", error);
    assert_eq!(logged(&o)[0], "before");
    // Tests that ran before the failure still count.
    let o = run_scripts(&url, "", "pm.test('one', () => {});\nthrow new Error('late');", &mut Scopes::default());
    assert_eq!(o.resp.tests.len(), 1);
    assert!(o.resp.script_error.unwrap().contains("late"));
    // Without an environment the value is not kept, and the script is told why.
    let o = run_scripts(&url, "pm.environment.set('a', 1);", "", &mut Scopes::default());
    assert!(logged(&o)[0].contains("No environment"));
    assert!(o.changes.environment.is_empty());
    // Libraries that aren't there say so.
    let o = run_scripts(&url, "", "require('lodash');", &mut Scopes::default());
    assert!(o.resp.script_error.unwrap().contains("lodash"));
}

#[test]
fn endless_scripts_are_stopped() {
    let base = echo_server();
    for pre in [
        "while (true) {}",
        "for (;;) { try { while (true) {} } catch (e) {} }",
        "setInterval(() => {}, 1);",
        "await new Promise(() => setTimeout(() => {}, 99999));",
        "const f = async () => { await null; return f(); }; await f();",
    ] {
        let mut scopes = Scopes::default();
        let req = Request { url: base.clone(), scripts: Scripts { pre: pre.into(), post: String::new() }, ..Default::default() };
        let mut j = job(req, vec![]);
        j.script_limit = Duration::from_millis(300);
        let started = std::time::Instant::now();
        let o = scripting::run(&j, &mut scopes);
        assert!(started.elapsed() < Duration::from_secs(5), "{} took too long", pre);
        let error = o.resp.error.unwrap_or_default();
        assert!(error.contains("was stopped"), "{}: {}", pre, error);
    }
}

#[test]
fn async_scripts_and_send_request() {
    let base = echo_server();
    let mut scopes = env_scopes();
    let pre = format!(
        r#"const login = await pm.sendRequest({{ url: "{base}/login", method: "POST", header: {{ "X-A": "1" }}, body: {{ mode: "raw", raw: "hi" }} }});
           pm.expect(login.code).to.equal(200);
           const seen = login.json();
           pm.environment.set("token", seen.token);
           pm.environment.set("line", seen.line);
           pm.environment.set("body", seen.body);
           await new Promise((done) => setTimeout(done, 20));
           pm.sendRequest("{base}/later", (err, res) => {{
             pm.environment.set("callback", err ? "failed" : String(res.code));
             pm.test("inside callback", () => pm.expect(res.json().line).to.include("/later"));
           }});
           pm.environment.set("order", "first");"#
    );
    let post = r#"
        await pm.test("async pass", async () => { await null; pm.expect(1).to.equal(1); });
        await pm.test("async fail", async () => { await null; pm.expect(1).to.equal(2); });
        pm.test("done style", (done) => { setTimeout(() => done(new Error("late fail")), 5); });
    "#;
    let o = run_scripts(&format!("{}/{{{{token}}}}", base), &pre, post, &mut scopes);
    assert!(o.resp.error.is_none(), "{:?}", o.resp.error);
    assert!(o.resp.body.contains("GET /tok-123 "), "{}", o.resp.body);
    assert_eq!(scopes.environment["line"], "POST /login HTTP/1.1");
    assert_eq!(scopes.environment["body"], "hi");
    assert_eq!(scopes.environment["callback"], "200");
    assert_eq!(o.before.calls.len(), 2, "both extra calls are kept for the console");
    assert_eq!(failed(&o), vec!["async fail: expected 1 to equal 2", "done style: Error: late fail (line 4)"]);
    assert_eq!(o.resp.tests.iter().filter(|t| t.1).count(), 2);
    // A call that can't connect reaches the callback as an error.
    let bad = "pm.sendRequest('http://127.0.0.1:1/', (err) => pm.environment.set('err', err ? 'yes' : 'no'));";
    run_scripts(&base, bad, "", &mut scopes);
    assert_eq!(scopes.environment["err"], "yes");
}

#[test]
fn skipping_and_choosing_the_next_request() {
    let base = echo_server();
    let o = run_scripts(&base, "pm.execution.skipRequest();", "pm.environment.set('ran', 1);", &mut env_scopes());
    assert_eq!(o.resp.status, 0);
    assert!(o.resp.error.unwrap().contains("skipped"));
    let o = run_scripts(&base, "", "pm.execution.setNextRequest('Other');", &mut Scopes::default());
    assert_eq!(o.next, Some(Some("Other".to_string())));
    let o = run_scripts(&base, "", "postman.setNextRequest(null);", &mut Scopes::default());
    assert_eq!(o.next, Some(None));
    let o = run_scripts(&base, "", "", &mut Scopes::default());
    assert_eq!(o.next, None);
}

#[test]
fn older_style_scripts_work() {
    let base = echo_server();
    let mut scopes = env_scopes();
    let post = r#"
        tests["status"] = responseCode.code === 200;
        tests["bad"] = false;
        var data = JSON.parse(responseBody);
        postman.setEnvironmentVariable("token", data.token);
        tests["header"] = postman.getResponseHeader("Content-Type") === "application/json";
    "#;
    let o = run_scripts(&base, "", post, &mut scopes);
    assert_eq!(scopes.environment["token"], "tok-123");
    assert_eq!(o.resp.tests.len(), 3);
    assert_eq!(failed(&o), vec!["bad: "]);
}

#[test]
fn assertions_behave_like_chai() {
    let base = echo_server();
    let pass = r#"
        const e = pm.expect;
        e(1).to.equal(1); e("a").to.be.a("string"); e([]).to.be.an("array"); e({}).to.be.an("object"); e(null).to.be.null; e(undefined).to.be.undefined;
        e(true).to.be.true; e(false).to.be.false; e(1).to.exist; e(1).to.be.ok; e("").to.be.empty; e([]).to.be.empty; e({}).to.be.empty;
        e({a: {b: [1]}}).to.eql({a: {b: [1]}}); e({a: 1}).to.deep.equal({a: 1}); e([1, 2]).to.deep.include(2); e(1).to.not.equal(2);
        e(5).to.be.above(1); e(5).to.be.below(9); e(5).to.be.at.least(5); e(5).to.be.at.most(5); e(5).to.be.within(1, 9); e(1.01).to.be.closeTo(1, 0.1);
        e("hello").to.include("ell"); e("hello").to.contain("ell"); e([1, 2]).to.include(1); e({a: 1, b: 2}).to.include({a: 1});
        e("hello").to.match(/h.l/); e("a").to.be.oneOf(["a", "b"]); e([1, 2]).to.have.lengthOf(2); e("abc").to.have.length(3);
        e({a: 1}).to.have.property("a"); e({a: 1}).to.have.property("a", 1); e({a: {b: 2}}).to.have.property("a").that.has.property("b", 2);
        e({a: 1, b: 2}).to.have.keys("a", "b"); e({a: 1, b: 2}).to.have.all.keys("a", "b"); e({a: 1, b: 2}).to.include.keys("a"); e({a: 1}).to.have.any.keys("a", "z");
        e([1, 2]).to.have.members([2, 1]); e([1, 2, 3]).to.include.members([2]); e(() => { throw new Error("x"); }).to.throw();
        e(2).to.satisfy((n) => n % 2 === 0); e(new Date()).to.be.a("date");
        e(pm.response).to.have.status(200); e(pm.response).to.have.jsonBody("token", "tok-123"); pm.response.to.have.jsonBody(); pm.response.to.have.body(); pm.response.to.be.json;
        pm.response.to.have.jsonSchema({ type: "object", required: ["token"], properties: { token: { type: "string" }, items: { type: "array", items: { type: "object", required: ["id"] } } } });
        console.log(btoa("hi"), atob("aGk="), pm.variables.replaceIn("{{$guid}}").length, pm.info.requestName, pm.info.eventName);
    "#;
    let o = run_scripts(&base, "", pass, &mut Scopes::default());
    assert!(o.resp.script_error.is_none(), "{:?}", o.resp.script_error);
    assert_eq!(logged(&o), vec!["aGk= hi 36 New Request test"]);
    for fail in [
        "pm.expect(1).to.equal(2)",
        "pm.expect(1).to.not.equal(1)",
        "pm.expect('a').to.be.a('number')",
        "pm.expect(1).to.be.null",
        "pm.expect({a: 1}).to.equal({a: 1})",
        "pm.expect({a: 1}).to.have.property('b')",
        "pm.expect([1]).to.have.lengthOf(2)",
        "pm.expect({a: 1, b: 2}).to.have.keys('a')",
        "pm.expect(1).to.be.above(2)",
        "pm.expect(() => {}).to.throw()",
        "pm.response.to.have.jsonSchema({ type: 'array' })",
        "pm.response.to.be.clientError",
        "pm.expect.fail('nope')",
    ] {
        let o = run_scripts(&base, "", &format!("pm.test('t', () => {{ {} }});", fail), &mut Scopes::default());
        assert_eq!(o.resp.tests.iter().filter(|t| !t.1).count(), 1, "should fail: {}", fail);
    }
}

#[test]
fn scripts_are_saved_shared_and_exported() {
    let mut c = sample_collection();
    c.scripts = Scripts { pre: "console.log('collection');".into(), post: String::new() };
    if let Item::Folder(f) = &mut c.items[0] {
        f.scripts.post = "console.log('folder');\nconsole.log('two');".into();
        if let Item::Request(r) = &mut f.items[0] {
            r.scripts = Scripts { pre: "console.log('pre');".into(), post: "console.log('post');".into() };
        }
    }
    let d = tmp("scripts");
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    assert_eq!(workspace::load(&d).collections[0].scripts, c.scripts);
    assert_eq!(workspace::load(&d).collections[0].items, c.items);
    match pigeon_core::formats::import_json(&pigeon_core::formats::export_collection(&c)).unwrap() {
        pigeon_core::formats::Imported::Collection(back) => {
            assert_eq!(back.scripts, c.scripts);
            let (Item::Folder(a), Item::Folder(b)) = (&back.items[0], &c.items[0]) else { panic!() };
            assert_eq!(a.scripts, b.scripts);
            let (Item::Request(a), Item::Request(b)) = (&a.items[0], &b.items[0]) else { panic!() };
            assert_eq!(a.scripts, b.scripts);
        }
        _ => panic!("expected a collection"),
    }
}

#[test]
fn runaway_scripts_fail_safely() {
    let base = echo_server();
    for pre in [
        "function f() { return f() + 1; } f();",
        "const a = []; while (true) a.push(new Array(1000000).fill(0));",
        "let s = 'x'; while (true) s += s;",
    ] {
        let req = Request { url: base.clone(), scripts: Scripts { pre: pre.into(), post: String::new() }, ..Default::default() };
        let mut j = job(req, vec![]);
        j.script_limit = Duration::from_secs(5);
        let o = scripting::run(&j, &mut Scopes::default());
        assert!(o.resp.error.is_some(), "{}", pre);
        assert_eq!(o.resp.status, 0);
    }
}

#[test]
#[ignore = "needs network"]
fn sample_script_live() {
    let c = sample_collection();
    let req = request_named(&c.items, "List users").clone();
    let mut scopes = env_scopes();
    scopes.collection.insert("baseUrl".into(), "https://jsonplaceholder.typicode.com".into());
    let o = scripting::run(&job(req, vec![]), &mut scopes);
    assert!(o.resp.error.is_none(), "{:?}", o.resp.error);
    assert_eq!(failed(&o), Vec::<String>::new());
    assert_eq!(o.resp.tests.len(), 2);
    assert_eq!(logged(&o), vec!["First user: Leanne Graham"]);
    assert_eq!(scopes.environment["userId"], "1");
}
