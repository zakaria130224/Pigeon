# Scripting reference

Scripts are JavaScript (ES2023) and run in a sandbox with no access to your files. `await` works at the top level.
Each script starts fresh: variables are the only thing that carries from one script to the next.

Scripts stop after 30 seconds, not counting time spent waiting for `pm.sendRequest`, and can use up to 256 MB.

## Variables

`pm.environment`, `pm.collectionVariables` and `pm.globals` each offer:

| Call | Does |
| --- | --- |
| `.get(name)` | Reads a value |
| `.set(name, value)` | Sets a value. Objects are stored as JSON text. |
| `.has(name)` | Checks whether it exists |
| `.unset(name)` | Removes it |
| `.clear()` | Removes all of them |
| `.toObject()` | Returns them all |
| `.replaceIn(text)` | Fills in `{{variables}}` |

`pm.environment.name` is the active environment's name. With no active environment, `pm.environment.set` logs a
warning and keeps nothing.

`pm.variables` looks in every scope. `get` searches this run's variables, then the environment, collection and
globals. `set` and `unset` affect only this run, and `has`, `replaceIn` and `toObject` work as above.

Values set in environments and collections stay on this computer. See the
[user manual](user-manual.md#scripts).

## The request

| Call | Does |
| --- | --- |
| `pm.request.method` | Read or change the method |
| `pm.request.url` | Read or assign the URL. `.toString()`, `.getHost()`, `.getPath()`, `.protocol`, `.port`, `.query` |
| `pm.request.headers` | A list, described below |
| `pm.request.url.query` | A list of query parameters |
| `pm.request.body` | `.mode`, `.raw`, `.urlencoded`, `.formdata`, `.update(text)`. Assign a string or `{ mode, raw }`. |
| `pm.request.addHeader(h)`, `.upsertHeader(h)`, `.removeHeader(name)` | Shortcuts for headers |
| `pm.request.addQueryParams(p)`, `.removeQueryParams(names)` | Shortcuts for the query |

Changes made in a pre-request script affect this send only and never change the saved request. In pre-request
scripts the URL still contains its `{{variables}}`.

**Lists** offer `get(key)`, `has(key)`, `add({ key, value })`, `upsert({ key, value })`, `remove(key)`, `clear()`,
`all()`, `count()`, `each(fn)`, `map(fn)`, `filter(fn)`, `find(fn)` and `toObject()`.

## The response

Available in post-response scripts. If the request failed to connect, post-response scripts don't run.

| Call | Does |
| --- | --- |
| `pm.response.code` | Status code, such as `200` |
| `pm.response.status` | Status text, such as `"OK"` |
| `pm.response.responseTime` | Time in milliseconds |
| `pm.response.responseSize` | Body size in bytes |
| `pm.response.headers` | A list |
| `pm.response.text()` | Body as text |
| `pm.response.json()` | Body parsed as JSON |
| `pm.cookies.get(name)`, `.has(name)`, `.toObject()`, `.all()` | Cookies this response set |

## Tests

```js
pm.test("name", () => { /* a failed expectation fails the test */ });
pm.test("async", async () => { const r = await pm.sendRequest(url); });
pm.test("callback", (done) => { setTimeout(done, 10); });
pm.test.skip("not ready", () => {});
```

Use `await pm.test(...)` with an async test if later lines depend on it.

### pm.expect

```js
pm.expect(value).to.equal(1);
pm.expect(data).to.have.property("id", 7);
pm.expect(list).to.not.be.empty;
```

Connecting words: `to be been is that which and has have with at of same but does still also`.
Modifiers: `not deep any all`.

| Group | Checks |
| --- | --- |
| Equality | `equal eq equals eql eqls` |
| Type | `a an instanceof instanceOf` |
| Values | `true false null undefined NaN ok exist exists empty finite` |
| Numbers | `above gt greaterThan least gte below lt lessThan most lte within closeTo` |
| Text and lists | `include includes contain contains match matches string lengthOf length oneOf members` |
| Objects | `property ownProperty keys key` |
| Functions | `throw throws satisfy satisfies` |

`pm.expect.fail(message)` fails on purpose.

### Response checks

Use these on `pm.response.to` or `pm.expect(pm.response).to`:

```js
pm.response.to.have.status(200);            // or a status text
pm.response.to.have.header("Content-Type", "application/json");
pm.response.to.have.body("exact text");     // or a /pattern/, or no argument
pm.response.to.have.jsonBody("user.name", "Ada");
pm.response.to.have.jsonSchema({ type: "object", required: ["id"] });
pm.response.to.be.ok;                       // exactly 200
pm.response.to.be.success;                  // any 2xx
```

Also available: `redirection clientError serverError error notFound unauthorized forbidden badRequest json`.

`jsonSchema` understands `type`, `required`, `properties`, `items` and `enum`.

## Sending other requests

```js
const res = await pm.sendRequest("https://example.com");

pm.sendRequest({
  url: "https://example.com/login",
  method: "POST",
  header: { "Content-Type": "application/json" },
  body: { mode: "raw", raw: JSON.stringify({ user: "ada" }) },
}, (err, res) => {
  if (!err) pm.environment.set("token", res.json().token);
});
```

`body` can also be a string or `{ mode: "urlencoded", urlencoded: [...] }`. `{{variables}}` aren't filled in here,
so use `pm.variables.replaceIn`. These calls appear in the Console.

## Controlling the run

| Call | Does |
| --- | --- |
| `pm.execution.skipRequest()` | In a pre-request script: don't send this request |
| `pm.execution.setNextRequest(name)` | In the runner: go to the request with that name |
| `pm.execution.setNextRequest(null)` | In the runner: stop |

An unknown name stops the run. `pm.setNextRequest` does the same.

## Information

`pm.info` holds `requestName`, `requestId`, `eventName` (`prerequest` or `test`), `iteration` (from 0) and
`iterationCount`.

## Everyday JavaScript

`console.log`, `.info`, `.debug`, `.warn` and `.error` write to the Console. `setTimeout`, `setInterval`,
`setImmediate`, their `clear` versions, `btoa` and `atob` are available, along with standard JavaScript such as
`JSON`, `Date`, `Math`, `Promise` and `RegExp`. `crypto.getRandomValues` fills a typed array with secure random
bytes.

## CryptoJS

The crypto-js library (version 4.2.0) is available as `CryptoJS`, or through `require("crypto-js")`, as in Postman.
It covers hashes, HMAC, encodings and ciphers:

```js
const token = CryptoJS.MD5(pm.environment.get("channel") + ":" + date).toString();
const signature = CryptoJS.HmacSHA256(body, secret).toString(CryptoJS.enc.Base64);
```

## Older scripts

Scripts written in the older style also work: `tests["name"] = true`, `responseBody`, `responseCode.code`,
`responseTime`, `responseHeaders`, `postman.setEnvironmentVariable`, `postman.getEnvironmentVariable`,
`postman.setGlobalVariable`, `postman.getResponseHeader` and `postman.setNextRequest`.

## Not available

`require` for anything other than crypto-js (it stops the script with a clear error), so libraries such as lodash and
moment can't be used.
Also `pm.visualizer`, `pm.iterationData` from data files, and browser or Node features such as `fetch` and `fs`.
