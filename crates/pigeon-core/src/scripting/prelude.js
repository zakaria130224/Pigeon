// The scripting API available to pre-request and post-response scripts.
// Runs once per script in a fresh sandbox: no file system, only what is defined here.
(function (g) {
  "use strict";

  const logs = [];
  const tests = [];
  const timers = [];
  let timerSeq = 1;
  let clock = 0;
  const out = { skip: false, next: undefined, error: null };

  // ------------------------------------------------------------------ console

  function show(v) {
    if (typeof v === "string") return v;
    if (v === undefined) return "undefined";
    if (v instanceof Error) return v.name + ": " + v.message;
    if (typeof v === "function") return "[Function " + (v.name || "anonymous") + "]";
    if (v instanceof HeaderList) return JSON.stringify(v.toObject(), null, 2);
    if (v instanceof Url) return v.toString();
    try {
      const s = JSON.stringify(v, null, 2);
      return s === undefined ? String(v) : s;
    } catch (e) {
      return String(v);
    }
  }
  const logger = (level) => (...args) => logs.push({ level, text: args.map(show).join(" ") });
  g.console = {
    log: logger("log"), info: logger("info"), debug: logger("log"),
    warn: logger("warn"), error: logger("error"), trace: logger("log"),
    clear() {},
  };

  // ---------------------------------------------------------------- variables

  const asText = (v) => (typeof v === "string" ? v : v !== null && typeof v === "object" ? JSON.stringify(v) : String(v));
  const uuid = () =>
    "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      return (c === "x" ? r : (r & 3) | 8).toString(16);
    });
  const dynamic = {
    $guid: uuid,
    $randomUUID: uuid,
    $timestamp: () => String(Math.floor(Date.now() / 1000)),
    $isoTimestamp: () => new Date().toISOString(),
    $randomInt: () => String(Math.floor(Math.random() * 1001)),
    $randomBoolean: () => String(Math.random() < 0.5),
  };

  class Scope {
    constructor(values, usable, name) {
      this.values = Object.assign({}, values);
      this.usable = usable !== false;
      this.name = name || "";
    }
    has(key) { return Object.prototype.hasOwnProperty.call(this.values, key); }
    get(key) { return this.has(key) ? this.values[key] : undefined; }
    set(key, value) {
      if (!this.usable) {
        console.warn('No environment is selected, so "' + key + '" was not saved.');
        return;
      }
      if (key !== undefined && key !== null) this.values[String(key)] = asText(value);
    }
    unset(key) { delete this.values[key]; }
    clear() { this.values = {}; }
    toObject() { return Object.assign({}, this.values); }
    replaceIn(text) { return replaceVars(text); }
  }

  let scopes = {};
  function lookup(key) {
    for (const s of [scopes.local, scopes.environment, scopes.collection, scopes.globals]) {
      if (s.has(key)) return s.get(key);
    }
    return undefined;
  }
  function replaceVars(text) {
    if (typeof text !== "string") return text;
    let result = text;
    for (let depth = 0; depth < 8 && result.includes("{{"); depth++) {
      const next = result.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (whole, name) => {
        const v = lookup(name);
        if (v !== undefined) return v;
        return dynamic[name] ? dynamic[name]() : whole;
      });
      if (next === result) break;
      result = next;
    }
    return result;
  }

  // ------------------------------------------------------- headers and query

  class HeaderList {
    constructor(items) {
      this.items = (items || []).map((i) => ({ key: i.key, value: i.value, disabled: i.disabled === true }));
    }
    find_(key) {
      const k = String(key).toLowerCase();
      return this.items.find((i) => !i.disabled && String(i.key).toLowerCase() === k);
    }
    get(key) { const i = this.find_(key); return i ? i.value : undefined; }
    one(key) { return this.find_(key); }
    has(key) { return this.find_(key) !== undefined; }
    add(item, value) {
      if (typeof item === "string") {
        const at = item.indexOf(":");
        item = value !== undefined ? { key: item, value } : { key: at < 0 ? item : item.slice(0, at).trim(), value: at < 0 ? "" : item.slice(at + 1).trim() };
      }
      this.items.push({ key: item.key, value: asText(item.value), disabled: item.disabled === true });
    }
    upsert(item, value) {
      if (typeof item === "string") item = { key: item, value };
      const found = this.find_(item.key);
      if (found) found.value = asText(item.value);
      else this.add(item);
    }
    remove(key) {
      const k = String(typeof key === "object" && key ? key.key : key).toLowerCase();
      this.items = this.items.filter((i) => String(i.key).toLowerCase() !== k);
    }
    clear() { this.items = []; }
    all() { return this.items.filter((i) => !i.disabled); }
    count() { return this.all().length; }
    each(fn) { this.all().forEach(fn); }
    map(fn) { return this.all().map(fn); }
    filter(fn) { return this.all().filter(fn); }
    find(fn) { return this.all().find(fn); }
    toObject() {
      const o = {};
      this.each((i) => { o[i.key] = i.value; });
      return o;
    }
    toString() { return this.all().map((i) => i.key + ": " + i.value).join("\n"); }
  }

  class Url {
    constructor(text) {
      let s = String(text);
      const hash = s.indexOf("#");
      this.hash = hash < 0 ? "" : s.slice(hash);
      if (hash >= 0) s = s.slice(0, hash);
      const q = s.indexOf("?");
      this.base = q < 0 ? s : s.slice(0, q);
      const pairs = q < 0 || q === s.length - 1 ? [] : s.slice(q + 1).split("&");
      this.query = new HeaderList(pairs.map((p) => {
        const at = p.indexOf("=");
        return at < 0 ? { key: p, value: null } : { key: p.slice(0, at), value: p.slice(at + 1) };
      }));
    }
    get protocol() { const m = /^([a-z][a-z0-9+.-]*):\/\//i.exec(this.base); return m ? m[1] : undefined; }
    getHost() { const m = /^(?:[a-z][a-z0-9+.-]*:\/\/)?([^/:]*)/i.exec(this.base); return m ? m[1] : ""; }
    get host() { return this.getHost().split("."); }
    get port() { const m = /^(?:[a-z][a-z0-9+.-]*:\/\/)?[^/:]*:(\d+)/i.exec(this.base); return m ? m[1] : undefined; }
    getPath() { const m = /^(?:[a-z][a-z0-9+.-]*:\/\/)?[^/]*(\/.*)?$/i.exec(this.base); return (m && m[1]) || "/"; }
    get path() { return this.getPath().split("/").filter((p) => p !== ""); }
    getQueryString() { return this.query.all().map((p) => (p.value === null || p.value === undefined ? p.key : p.key + "=" + p.value)).join("&"); }
    toString() {
      const q = this.getQueryString();
      return this.base + (q ? "?" + q : "") + this.hash;
    }
  }

  // ------------------------------------------------------------------ request

  function makeRequest(r) {
    let url = new Url(r.url);
    const headers = new HeaderList(r.headers);
    const body = {
      mode: r.body.mode,
      raw_: r.body.raw,
      urlencoded: new HeaderList(r.body.urlencoded),
      formdata: new HeaderList(r.body.formdata),
      get raw() { return this.raw_; },
      set raw(v) { this.raw_ = asText(v); if (this.mode === "none") this.mode = "raw"; },
      update(v) { this.raw = v; },
      toString() { return this.raw_; },
    };
    const request = {
      id: r.id,
      name: r.name,
      method: r.method,
      headers,
      get url() { return url; },
      set url(v) { url = v instanceof Url ? v : new Url(v); },
      get body() { return body; },
      set body(v) {
        if (typeof v === "string") { body.raw = v; return; }
        if (!v) { body.mode = "none"; return; }
        if (v.mode) body.mode = v.mode;
        if (v.raw !== undefined) body.raw = v.raw;
        if (v.urlencoded) body.urlencoded = new HeaderList(v.urlencoded);
        if (v.formdata) body.formdata = new HeaderList(v.formdata);
      },
      addHeader(h) { headers.add(h); },
      removeHeader(k) { headers.remove(k); },
      upsertHeader(h) { headers.upsert(h); },
      addQueryParams(p) {
        (typeof p === "string" ? new Url("?" + p).query.items : p).forEach((i) => url.query.add(i));
      },
      removeQueryParams(p) { [].concat(p).forEach((k) => url.query.remove(k)); },
      toJSON() { return exportRequest(request); },
    };
    return request;
  }

  function exportRequest(r) {
    const list = (l) => l.items.map((i) => ({ key: i.key, value: i.value === null ? "" : i.value, disabled: i.disabled }));
    return {
      method: String(r.method).toUpperCase(),
      url: r.url.toString(),
      headers: list(r.headers),
      body: { mode: r.body.mode, raw: r.body.raw, urlencoded: list(r.body.urlencoded), formdata: list(r.body.formdata) },
    };
  }

  // ----------------------------------------------------------------- response

  class Response {
    constructor(r) {
      this.code = r.code;
      this.status = r.status;
      this.responseTime = r.time;
      this.responseSize = r.size;
      this.headers = new HeaderList(r.headers);
      this.body_ = r.body;
    }
    text() { return this.body_; }
    json() {
      try {
        return JSON.parse(this.body_);
      } catch (e) {
        throw new SyntaxError("The response body is not valid JSON: " + e.message);
      }
    }
    size() { return { body: this.responseSize, header: 0, total: this.responseSize }; }
    get to() { return new Assertion(this); }
    toJSON() { return { code: this.code, status: this.status, responseTime: this.responseTime, headers: this.headers.items, body: this.body_ }; }
  }

  function makeCookies(response) {
    const jar = {};
    if (response) {
      response.headers.items
        .filter((h) => String(h.key).toLowerCase() === "set-cookie")
        .forEach((h) => {
          const pair = String(h.value).split(";")[0];
          const at = pair.indexOf("=");
          if (at > 0) jar[pair.slice(0, at).trim()] = pair.slice(at + 1).trim();
        });
    }
    return {
      get: (k) => jar[k],
      has: (k) => k in jar,
      toObject: () => Object.assign({}, jar),
      all: () => Object.keys(jar).map((name) => ({ name, value: jar[name] })),
    };
  }

  // --------------------------------------------------------------- assertions

  class AssertionError extends Error {
    constructor(message) {
      super(message);
      this.name = "AssertionError";
    }
  }

  function same(a, b) {
    if (a === b || (a !== a && b !== b)) return true;
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object") return false;
    if (a instanceof Date && b instanceof Date) return a.getTime() === b.getTime();
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    return ka.length === kb.length && ka.every((k) => Object.prototype.hasOwnProperty.call(b, k) && same(a[k], b[k]));
  }
  const brief = (v) => {
    const s = typeof v === "string" ? "'" + v + "'" : show(v).replace(/\s*\n\s*/g, " ");
    return s.length > 120 ? s.slice(0, 120) + "…" : s;
  };
  function typeName(v) {
    if (v === null) return "null";
    if (Array.isArray(v)) return "array";
    if (v instanceof Date) return "date";
    if (v instanceof RegExp) return "regexp";
    return typeof v;
  }
  function schemaProblem(schema, v, at) {
    if (schema.enum && !schema.enum.some((e) => same(e, v))) return at + " is not one of the allowed values";
    if (schema.type) {
      const kinds = [].concat(schema.type);
      const kind = typeName(v);
      const fits = kinds.some((t) => t === kind || (t === "integer" && Number.isInteger(v)));
      if (!fits) return at + " should be " + kinds.join(" or ") + " but is " + kind;
    }
    if (typeName(v) === "object") {
      for (const k of schema.required || []) {
        if (!(k in v)) return at + " is missing required property " + k;
      }
      for (const k of Object.keys(schema.properties || {})) {
        if (k in v) {
          const p = schemaProblem(schema.properties[k], v[k], at + "." + k);
          if (p) return p;
        }
      }
    }
    if (Array.isArray(v) && schema.items) {
      for (let i = 0; i < v.length; i++) {
        const p = schemaProblem(schema.items, v[i], at + "[" + i + "]");
        if (p) return p;
      }
    }
    return null;
  }

  class Assertion {
    constructor(subject, note) {
      this.v = subject;
      this.note = note ? note + ": " : "";
      this.negate = false;
      this.deepFlag = false;
      this.partial = false;
      this.anyFlag = false;
    }
    check(ok, says, saysNot) {
      if (this.negate ? ok : !ok) throw new AssertionError(this.note + (this.negate ? saysNot : says));
      return this;
    }
    is_(ok, what) {
      return this.check(ok, "expected " + brief(this.v) + " to " + what, "expected " + brief(this.v) + " not to " + what);
    }
    get not() { this.negate = !this.negate; return this; }
    get deep() { this.deepFlag = true; return this; }
    get any() { this.anyFlag = true; return this; }
    get all() { this.anyFlag = false; return this; }
    get own() { return this; }
    get ordered() { return this; }
    get nested() { return this; }

    // Plain values
    get true() { return this.is_(this.v === true, "be true"); }
    get false() { return this.is_(this.v === false, "be false"); }
    get null() { return this.is_(this.v === null, "be null"); }
    get undefined() { return this.is_(this.v === undefined, "be undefined"); }
    get NaN() { return this.is_(this.v !== this.v, "be NaN"); }
    get exist() { return this.is_(this.v !== null && this.v !== undefined, "exist"); }
    get exists() { return this.exist; }
    get finite() { return this.is_(Number.isFinite(this.v), "be a finite number"); }
    get empty() {
      const v = this.v;
      const size = typeof v === "string" || Array.isArray(v) ? v.length : v && typeof v === "object" ? Object.keys(v).length : -1;
      return this.is_(size === 0, "be empty");
    }
    equal(x) {
      const ok = this.deepFlag ? same(this.v, x) : this.v === x;
      return this.is_(ok, "equal " + brief(x));
    }
    equals(x) { return this.equal(x); }
    eq(x) { return this.equal(x); }
    eql(x) { return this.is_(same(this.v, x), "deeply equal " + brief(x)); }
    eqls(x) { return this.eql(x); }
    a(kind) { return this.is_(typeName(this.v) === String(kind).toLowerCase(), "be a " + kind); }
    an(kind) { return this.a(kind); }
    instanceof(c) { return this.is_(this.v instanceof c, "be an instance of " + (c.name || "that type")); }
    instanceOf(c) { return this.instanceof(c); }
    above(n) { return this.is_(this.v > n, "be above " + n); }
    gt(n) { return this.above(n); }
    greaterThan(n) { return this.above(n); }
    least(n) { return this.is_(this.v >= n, "be at least " + n); }
    gte(n) { return this.least(n); }
    below(n) { return this.is_(this.v < n, "be below " + n); }
    lt(n) { return this.below(n); }
    lessThan(n) { return this.below(n); }
    most(n) { return this.is_(this.v <= n, "be at most " + n); }
    lte(n) { return this.most(n); }
    within(a, b) { return this.is_(this.v >= a && this.v <= b, "be within " + a + ".." + b); }
    closeTo(n, delta) { return this.is_(Math.abs(this.v - n) <= delta, "be close to " + n); }
    oneOf(list) { return this.is_(list.some((x) => (this.deepFlag ? same(x, this.v) : x === this.v)), "be one of " + brief(list)); }
    match(re) { return this.is_(re.test(this.v), "match " + re); }
    matches(re) { return this.match(re); }
    string(part) { return this.is_(String(this.v).includes(part), "contain " + brief(part)); }
    satisfy(fn) { return this.is_(!!fn(this.v), "satisfy the given check"); }
    satisfies(fn) { return this.satisfy(fn); }
    lengthOf(n) {
      const size = this.v === null || this.v === undefined ? undefined : this.v.length;
      return this.check(size === n, "expected " + brief(this.v) + " to have a length of " + n + " but got " + size, "expected " + brief(this.v) + " not to have a length of " + n);
    }
    length(n) { return this.lengthOf(n); }
    contains_(x) {
      const v = this.v;
      if (typeof v === "string") return v.includes(x);
      if (Array.isArray(v)) return v.some((i) => (this.deepFlag ? same(i, x) : i === x));
      if (v && typeof v === "object") return Object.keys(x).every((k) => same(v[k], x[k]));
      return false;
    }
    get include() {
      const f = (x) => this.is_(this.contains_(x), "include " + brief(x));
      f.keys = (...k) => { this.partial = true; return this.keys(...k); };
      f.members = (l) => { this.partial = true; return this.members(l); };
      return f;
    }
    get includes() { return this.include; }
    get contain() { return this.include; }
    get contains() { return this.include; }
    keys(...list) {
      const wanted = list.length === 1 && typeof list[0] === "object" ? (Array.isArray(list[0]) ? list[0] : Object.keys(list[0])) : list;
      const have = Object.keys(this.v || {});
      const ok = this.anyFlag
        ? wanted.some((k) => have.includes(k))
        : wanted.every((k) => have.includes(k)) && (this.partial || have.length === wanted.length);
      return this.is_(ok, "have keys " + brief(wanted));
    }
    key(k) { return this.keys(k); }
    members(list) {
      const have = this.v || [];
      const inHave = (x) => have.some((i) => same(i, x));
      const ok = list.every(inHave) && (this.partial || have.length === list.length);
      return this.is_(ok, "have members " + brief(list));
    }
    property(name, ...value) {
      const v = this.v;
      const has = v !== null && v !== undefined && Object(v) === v ? name in v : v !== null && v !== undefined && v[name] !== undefined;
      if (value.length === 0) {
        this.is_(has, "have property " + brief(name));
      } else {
        this.is_(has && (this.deepFlag ? same(v[name], value[0]) : v[name] === value[0]), "have property " + brief(name) + " of " + brief(value[0]));
      }
      if (!this.negate && has) this.v = v[name];
      return this;
    }
    ownProperty(name, ...value) { return this.property(name, ...value); }
    throw(kind) {
      let thrown = null;
      try { this.v(); } catch (e) { thrown = e; }
      const ok = thrown !== null && (typeof kind !== "function" || thrown instanceof kind) && (typeof kind !== "string" || String(thrown.message).includes(kind));
      return this.check(ok, "expected the function to throw", "expected the function not to throw");
    }
    throws(kind) { return this.throw(kind); }

    // Responses
    get response_() {
      if (!(this.v instanceof Response)) throw new TypeError("This check only works on a response");
      return this.v;
    }
    status(want) {
      const r = this.response_;
      const ok = typeof want === "number" ? r.code === want : r.status === want;
      const got = typeof want === "number" ? r.code : r.status;
      return this.check(ok, "expected response to have status " + brief(want) + " but got " + brief(got), "expected response not to have status " + brief(want));
    }
    header(name, ...value) {
      const r = this.response_;
      if (value.length === 0) return this.check(r.headers.has(name), "expected response to have header " + name, "expected response not to have header " + name);
      const got = r.headers.get(name);
      return this.check(got === value[0], "expected header " + name + " to be " + brief(value[0]) + " but got " + brief(got), "expected header " + name + " not to be " + brief(value[0]));
    }
    body(want) {
      const text = this.response_.text();
      if (want === undefined) return this.check(text.length > 0, "expected response to have a body", "expected response to have no body");
      const ok = want instanceof RegExp ? want.test(text) : text === want;
      return this.check(ok, "expected response body to match " + brief(want), "expected response body not to match " + brief(want));
    }
    jsonBody(path, ...value) {
      const r = this.response_;
      let data;
      let ok = true;
      try { data = r.json(); } catch (e) { ok = false; }
      if (path === undefined || !ok) return this.check(ok, "expected response body to be JSON", "expected response body not to be JSON");
      if (typeof path === "object") return this.check(same(data, path), "expected response JSON to equal " + brief(path), "expected response JSON not to equal " + brief(path));
      const got = String(path).split(".").reduce((o, k) => (o === null || o === undefined ? undefined : o[k]), data);
      const fits = value.length === 0 ? got !== undefined : same(got, value[0]);
      return this.check(fits, "expected response JSON to have " + path, "expected response JSON not to have " + path);
    }
    jsonSchema(schema) {
      const problem = schemaProblem(schema, this.response_.json(), "body");
      return this.check(problem === null, "response does not match the schema: " + problem, "expected response not to match the schema");
    }
    range_(from, to, what) {
      const c = this.response_.code;
      return this.check(c >= from && c < to, "expected response to be " + what + " but got " + c, "expected response not to be " + what + " but got " + c);
    }
    get ok() {
      if (this.v instanceof Response) return this.range_(200, 201, "ok (200)");
      return this.is_(!!this.v, "be truthy");
    }
    get success() { return this.range_(200, 300, "successful (2xx)"); }
    get redirection() { return this.range_(300, 400, "a redirect (3xx)"); }
    get clientError() { return this.range_(400, 500, "a client error (4xx)"); }
    get serverError() { return this.range_(500, 600, "a server error (5xx)"); }
    get error() { return this.range_(400, 600, "an error (4xx or 5xx)"); }
    get notFound() { return this.range_(404, 405, "not found (404)"); }
    get unauthorized() { return this.range_(401, 402, "unauthorized (401)"); }
    get forbidden() { return this.range_(403, 404, "forbidden (403)"); }
    get badRequest() { return this.range_(400, 401, "a bad request (400)"); }
    get json() {
      let ok = true;
      try { this.response_.json(); } catch (e) { ok = false; }
      return this.check(ok, "expected response body to be JSON", "expected response body not to be JSON");
    }
  }
  for (const word of ["to", "be", "been", "is", "that", "which", "and", "has", "have", "with", "at", "of", "same", "but", "does", "still", "also"]) {
    Object.defineProperty(Assertion.prototype, word, { get() { return this; } });
  }

  // -------------------------------------------------------------------- tests

  function describe(e) {
    if (e instanceof Error) {
      const where = /:(\d+)(?::\d+)?\)?\s*(?:\n|$)/.exec(String(e.stack || ""));
      const line = where && !(e instanceof AssertionError) ? " (line " + Math.max(1, Number(where[1]) - 2) + ")" : "";
      return e.name + ": " + e.message + line;
    }
    return show(e);
  }
  function pmTest(name, fn) {
    const record = { name: String(name), passed: true, error: "" };
    tests.push(record);
    const fail = (e) => { record.passed = false; record.error = e instanceof AssertionError ? e.message : describe(e); };
    if (typeof fn !== "function") return pmTest;
    try {
      const done = (e) => { if (e) fail(e); };
      const result = fn.length > 0 ? fn(done) : fn();
      if (result && typeof result.then === "function") result.then(null, fail);
    } catch (e) {
      fail(e);
    }
    return pmTest;
  }
  pmTest.skip = (name) => { logs.push({ level: "info", text: "Skipped test: " + name }); return pmTest; };

  // ------------------------------------------------------------------- timers

  g.setTimeout = (fn, ms, ...args) => {
    const id = timerSeq++;
    timers.push({ id, due: clock + (Number(ms) || 0), fn: () => fn(...args) });
    timers.sort((a, b) => a.due - b.due || a.id - b.id);
    return id;
  };
  g.clearTimeout = (id) => {
    const at = timers.findIndex((t) => t.id === id);
    if (at >= 0) timers.splice(at, 1);
  };
  g.setImmediate = (fn, ...args) => g.setTimeout(fn, 0, ...args);
  g.clearImmediate = g.clearTimeout;
  g.setInterval = (fn, ms, ...args) => {
    const id = timerSeq++;
    const tick = () => {
      fn(...args);
      timers.push({ id, due: clock + Math.max(1, Number(ms) || 0), fn: tick });
      timers.sort((a, b) => a.due - b.due || a.id - b.id);
    };
    timers.push({ id, due: clock + Math.max(1, Number(ms) || 0), fn: tick });
    timers.sort((a, b) => a.due - b.due || a.id - b.id);
    return id;
  };
  g.clearInterval = g.clearTimeout;
  g.__nextTimer = () => (timers.length ? Math.max(0, timers[0].due - clock) : -1);
  g.__fireTimer = () => {
    const t = timers.shift();
    if (!t) return;
    clock = Math.max(clock, t.due);
    try {
      t.fn();
    } catch (e) {
      if (out.error === null) out.error = describe(e);
    }
  };

  // -------------------------------------------------------------- sendRequest

  function sendRequest(req, callback) {
    const spec = typeof req === "string" ? { url: req } : req;
    const headers = [];
    const h = spec.header || spec.headers;
    if (Array.isArray(h)) h.forEach((i) => headers.push([String(i.key), asText(i.value)]));
    else if (h instanceof HeaderList) h.each((i) => headers.push([String(i.key), asText(i.value)]));
    else if (typeof h === "string") { const at = h.indexOf(":"); headers.push([h.slice(0, at).trim(), h.slice(at + 1).trim()]); }
    else if (h) Object.keys(h).forEach((k) => headers.push([k, asText(h[k])]));
    let body = null;
    const b = spec.body;
    if (typeof b === "string") body = b;
    else if (b && b.mode === "raw") body = asText(b.raw);
    else if (b && b.mode === "urlencoded") {
      const pairs = Array.isArray(b.urlencoded) ? b.urlencoded : Object.keys(b.urlencoded || {}).map((key) => ({ key, value: b.urlencoded[key] }));
      body = pairs.map((p) => encodeURIComponent(p.key) + "=" + encodeURIComponent(p.value)).join("&");
      if (!headers.some((x) => x[0].toLowerCase() === "content-type")) headers.push(["Content-Type", "application/x-www-form-urlencoded"]);
    }
    const job = JSON.stringify({ method: String(spec.method || "GET").toUpperCase(), url: spec.url instanceof Url ? spec.url.toString() : String(spec.url), headers, body });
    const run = () => {
      const r = JSON.parse(g.__http(job));
      if (r.error) throw new Error(r.error);
      return new Response(r);
    };
    if (typeof callback === "function") {
      g.setTimeout(() => {
        let res;
        try {
          res = run();
        } catch (e) {
          callback(e, null);
          return;
        }
        callback(null, res);
      }, 0);
      return undefined;
    }
    return new Promise((resolve, reject) => {
      g.setTimeout(() => {
        try {
          resolve(run());
        } catch (e) {
          reject(e);
        }
      }, 0);
    });
  }

  // ---------------------------------------------------------------- utilities

  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  g.btoa = (s) => {
    let o = "";
    const t = String(s);
    for (let i = 0; i < t.length; i += 3) {
      const a = t.charCodeAt(i), b = t.charCodeAt(i + 1), c = t.charCodeAt(i + 2);
      if (a > 255 || b > 255 || c > 255) throw new Error("btoa only accepts Latin-1 text");
      const n = (a << 16) | ((b || 0) << 8) | (c || 0);
      o += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + (i + 1 < t.length ? B64[(n >> 6) & 63] : "=") + (i + 2 < t.length ? B64[n & 63] : "=");
    }
    return o;
  };
  g.atob = (s) => {
    const t = String(s).replace(/=+$/, "");
    let o = "", bits = 0, n = 0;
    for (const ch of t) {
      const v = B64.indexOf(ch);
      if (v < 0) throw new Error("atob received invalid text");
      n = (n << 6) | v;
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        o += String.fromCharCode((n >> bits) & 255);
      }
    }
    return o;
  };
  g.require = (name) => {
    throw new Error('The library "' + name + '" is not available in scripts here.');
  };

  // ------------------------------------------------------------------- set-up

  g.__init = (json) => {
    const input = JSON.parse(json);
    scopes = {
      local: new Scope(input.local),
      environment: new Scope(input.environment, input.hasEnvironment, input.environmentName),
      collection: new Scope(input.collection),
      globals: new Scope(input.globals),
    };
    const request = makeRequest(input.request);
    const response = input.response ? new Response(input.response) : undefined;
    const pm = {
      info: Object.assign({ eventName: input.event }, input.info),
      environment: scopes.environment,
      collectionVariables: scopes.collection,
      globals: scopes.globals,
      iterationData: new Scope({}),
      variables: {
        has: (k) => lookup(k) !== undefined,
        get: lookup,
        set: (k, v) => scopes.local.set(k, v),
        unset: (k) => scopes.local.unset(k),
        replaceIn: replaceVars,
        toObject: () => Object.assign({}, scopes.globals.values, scopes.collection.values, scopes.environment.values, scopes.local.values),
      },
      request,
      response,
      cookies: makeCookies(response),
      test: pmTest,
      expect: (v, note) => new Assertion(v, note),
      sendRequest,
      execution: {
        skipRequest: () => { out.skip = true; },
        setNextRequest: (name) => { out.next = name === null ? null : String(name); },
      },
      visualizer: { set: () => console.warn("Visualizations aren't supported here.") },
      require: g.require,
    };
    pm.expect.fail = (m) => { throw new AssertionError(m || "expected to fail"); };
    pm.setNextRequest = pm.execution.setNextRequest;
    g.pm = pm;

    // Older style of script.
    g.postman = {
      setEnvironmentVariable: (k, v) => scopes.environment.set(k, v),
      getEnvironmentVariable: (k) => scopes.environment.get(k),
      clearEnvironmentVariable: (k) => scopes.environment.unset(k),
      setGlobalVariable: (k, v) => scopes.globals.set(k, v),
      getGlobalVariable: (k) => scopes.globals.get(k),
      clearGlobalVariable: (k) => scopes.globals.unset(k),
      setNextRequest: pm.execution.setNextRequest,
      getResponseHeader: (k) => (response ? response.headers.get(k) : undefined),
    };
    g.tests = {};
    g.environment = scopes.environment.toObject();
    g.globals = scopes.globals.toObject();
    g.request = { url: request.url.toString(), method: request.method, headers: request.headers.toObject(), name: request.name };
    if (response) {
      g.responseBody = response.text();
      g.responseCode = { code: response.code, name: response.status };
      g.responseTime = response.responseTime;
      g.responseHeaders = response.headers.toObject();
    }
  };

  g.__run = (code) => {
    const AsyncFunction = (async function () {}).constructor;
    let script;
    try {
      script = new AsyncFunction(code);
    } catch (e) {
      out.error = describe(e);
      return;
    }
    script().then(null, (e) => {
      if (out.error === null) out.error = describe(e);
    });
  };

  g.__finish = () => {
    for (const name of Object.keys(g.tests || {})) {
      tests.push({ name, passed: !!g.tests[name], error: "" });
    }
    return JSON.stringify({
      logs,
      tests,
      skip: out.skip,
      next: out.next,
      error: out.error,
      pendingTimers: timers.length,
      request: exportRequest(g.pm.request),
      local: scopes.local.toObject(),
      environment: scopes.environment.toObject(),
      collection: scopes.collection.toObject(),
      globals: scopes.globals.toObject(),
    });
  };
})(globalThis);
