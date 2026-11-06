mod common;

use common::*;
use pigeon_core::http;
use pigeon_core::model::*;
use pigeon_core::workspace;

#[test]
fn import_export_round_trip() {
    let c = sample_collection();
    let text = pigeon_core::formats::export_collection(&c);
    match pigeon_core::formats::import_json(&text).unwrap() {
        pigeon_core::formats::Imported::Collection(c2) => {
            assert_eq!(c2.name, c.name);
            assert_eq!(count_requests(&c2.items), count_requests(&c.items));
        }
        _ => panic!("expected a collection"),
    }
}

const SPRING_SPEC: &str = r##"{
  "openapi": "3.0.1",
  "info": { "title": "Shop API", "description": "Orders and users", "version": "v1" },
  "servers": [ { "url": "{scheme}://api.shop.test:{port}/v1/", "variables": { "scheme": { "default": "https" }, "port": { "default": "8443" } } } ],
  "security": [ { "bearerAuth": [] } ],
  "tags": [ { "name": "user-controller", "description": "Users" } ],
  "paths": {
    "/users/{id}": {
      "parameters": [ { "name": "id", "in": "path", "required": true, "schema": { "type": "integer", "format": "int64", "example": 7 } } ],
      "get": {
        "tags": [ "user-controller" ], "operationId": "getUser",
        "parameters": [
          { "name": "verbose", "in": "query", "schema": { "type": "boolean" } },
          { "name": "X-Trace", "in": "header", "required": true, "schema": { "type": "string", "example": "abc" } },
          { "name": "Accept", "in": "header", "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/User" } } } } }
      },
      "put": {
        "tags": [ "user-controller" ], "summary": "Update a user\nsecond line",
        "requestBody": { "$ref": "#/components/requestBodies/UserBody" },
        "responses": { "200": { "description": "ok", "content": { "*/*": {} } } }
      }
    },
    "/users": {
      "get": {
        "tags": [ "user-controller" ], "operationId": "listUsers",
        "parameters": [ { "$ref": "#/components/parameters/Paging" } ],
        "responses": { "200": { "description": "ok" } }
      }
    },
    "/health": {
      "get": { "operationId": "health", "security": [], "responses": { "200": { "description": "ok" } } }
    },
    "/login": {
      "post": {
        "operationId": "login", "security": [ { "keyAuth": [] } ],
        "requestBody": { "content": { "application/x-www-form-urlencoded": { "schema": { "type": "object", "properties": { "user": { "type": "string" }, "age": { "type": "integer" } } } } } },
        "responses": { "204": { "description": "ok" } }
      }
    },
    "/files": {
      "post": {
        "operationId": "upload",
        "requestBody": { "content": { "multipart/form-data": { "schema": { "type": "object", "properties": { "note": { "type": "string" }, "file": { "type": "string", "format": "binary" } } } } } },
        "responses": { "201": { "description": "ok" } }
      }
    },
    "/events": {
      "post": {
        "operationId": "sendEvent",
        "requestBody": { "content": { "application/vnd.shop+json": { "examples": { "a": { "$ref": "#/components/examples/Event" } } } } },
        "responses": { "202": { "description": "ok" } }
      }
    }
  },
  "components": {
    "parameters": { "Paging": { "name": "pageable", "in": "query", "required": true, "schema": { "type": "object", "properties": { "page": { "type": "integer", "minimum": 0 }, "size": { "type": "integer", "example": 20 } } } } },
    "requestBodies": { "UserBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/User" } } } } },
    "examples": { "Event": { "value": { "kind": "ping" } } },
    "securitySchemes": {
      "bearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" },
      "keyAuth": { "type": "apiKey", "in": "header", "name": "X-Key" }
    },
    "schemas": {
      "Base": { "type": "object", "properties": { "id": { "type": "integer", "readOnly": true }, "created": { "type": "string", "format": "date-time" } } },
      "User": {
        "allOf": [ { "$ref": "#/components/schemas/Base" }, { "type": "object", "properties": {
          "name": { "type": "string", "example": "Ada" },
          "role": { "type": "string", "enum": [ "ADMIN", "USER" ] },
          "tags": { "type": "array", "items": { "type": "string" } },
          "manager": { "$ref": "#/components/schemas/User" }
        } } ]
      }
    }
  }
}"##;

#[test]
fn openapi_import() {
    let c = imported_openapi(SPRING_SPEC);
    assert_eq!(c.name, "Shop API");
    assert_eq!(count_requests(&c.items), 7);
    assert_eq!(c.auth.kind, AuthKind::Bearer);
    let var = |n: &str| c.variables.iter().find(|k| k.key == n).map(|k| k.value.clone());
    assert_eq!(var("baseUrl").unwrap(), "https://api.shop.test:8443/v1");
    assert_eq!(var("id").unwrap(), "7");
    let folder = c.items.iter().find_map(|i| match i {
        Item::Folder(f) if f.name == "user-controller" => Some(f),
        _ => None,
    });
    assert_eq!(folder.unwrap().items.len(), 3);
    assert_eq!(folder.unwrap().description, "Users");

    // Parameters: optional ones are listed but off, the shared path one becomes a variable.
    let get = request_named(&c.items, "getUser");
    assert_eq!(get.url, "{{baseUrl}}/users/{{id}}");
    assert!(!get.params[0].enabled);
    assert_eq!(get.auth.kind, AuthKind::Inherit);
    let trace = get.headers.iter().find(|h| h.key == "X-Trace").unwrap();
    assert!(trace.enabled && trace.value == "abc");
    assert_eq!(get.headers.iter().filter(|h| h.key == "Accept").map(|h| h.value.as_str()).collect::<Vec<_>>(), ["application/json"]);

    // Object query parameters are spread out and appear in the URL.
    let list = request_named(&c.items, "listUsers");
    assert_eq!(list.url, "{{baseUrl}}/users?page=0&size=20");

    // JSON body from a schema: allOf merged, read-only left out, recursion stopped.
    let put = request_named(&c.items, "Update a user");
    assert_eq!(put.body.mode, BodyMode::Raw);
    let body: serde_json::Value = serde_json::from_str(&put.body.raw).unwrap();
    assert_eq!(body["name"], "Ada");
    assert_eq!(body["role"], "ADMIN");
    assert_eq!(body["tags"][0], "string");
    assert!(body.get("id").is_none());
    assert!(body.get("manager").is_none());
    assert!(!put.headers.iter().any(|h| h.key == "Accept"));

    // Operation security overrides the collection's.
    assert_eq!(request_named(&c.items, "health").auth.kind, AuthKind::None);
    let login = request_named(&c.items, "login");
    assert_eq!(login.auth.kind, AuthKind::ApiKey);
    assert_eq!(login.auth.key, "X-Key");
    assert_eq!(login.body.mode, BodyMode::UrlEncoded);
    assert_eq!(login.body.urlencoded.len(), 2);

    let upload = request_named(&c.items, "upload");
    assert_eq!(upload.body.mode, BodyMode::FormData);
    assert!(upload.body.formdata.iter().any(|k| k.key == "note" && k.enabled));
    assert!(upload.body.formdata.iter().any(|k| k.key == "file" && !k.enabled));

    let event = request_named(&c.items, "sendEvent");
    assert!(event.body.raw.contains("ping"));
    assert!(event.headers.iter().any(|h| h.key == "Content-Type" && h.value == "application/vnd.shop+json"));

    // The result sends correctly, and survives saving to the workspace.
    let vars = http::Vars::build(None, Some(&c), &[]);
    let chain = [c.auth.clone()];
    let sent = http::prepare(get, &http::effective_auth(get, &chain), &vars);
    assert_eq!(sent.url, "https://api.shop.test:8443/v1/users/7");
    let d = tmp("openapi");
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    let l = workspace::load(&d);
    assert!(l.errors.is_empty(), "{:?}", l.errors);
    assert_eq!(count_requests(&l.collections[0].items), 7);
    assert_eq!(l.collections[0].variables, c.variables);
}

#[test]
fn openapi_rejects_what_it_cannot_read() {
    use pigeon_core::formats::import_json;
    assert!(import_json("openapi: 3.0.1\ninfo:\n  title: x").err().unwrap().contains("YAML"));
    assert!(import_json(r#"{"swagger":"2.0"}"#).err().unwrap().contains("Swagger 2"));
    assert!(import_json(r#"{"openapi":"2.5"}"#).is_err());
    // A byte-order mark and a missing server are both fine.
    let c = imported_openapi("\u{feff}{\"openapi\":\"3.0.1\",\"paths\":{\"/\":{\"get\":{}}}}");
    assert_eq!(c.variables[0].value, "http://localhost");
    assert_eq!(count_requests(&c.items), 1);
}

#[test]
#[ignore = "needs a document path in OPENAPI_FILE"]
fn openapi_real_document() {
    let text = std::fs::read_to_string(std::env::var("OPENAPI_FILE").unwrap()).unwrap();
    let c = imported_openapi(&text);
    println!("{}: {} requests, variables {:?}", c.name, count_requests(&c.items), c.variables);
    fn show(items: &[Item], depth: usize) {
        for i in items {
            match i {
                Item::Folder(f) => {
                    println!("{}[{}]", "  ".repeat(depth), f.name);
                    show(&f.items, depth + 1);
                }
                Item::Request(r) => {
                    println!("{}{} {} | {} | auth {:?} | body {:?}", "  ".repeat(depth), r.method, r.url, r.name, r.auth.kind, r.body.mode)
                }
            }
        }
    }
    show(&c.items, 0);
    let d = tmp("real");
    workspace::save(&d, std::slice::from_ref(&c), &[]).unwrap();
    assert_eq!(count_requests(&workspace::load(&d).collections[0].items), count_requests(&c.items));
}
