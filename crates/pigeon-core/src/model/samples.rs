//! The example collection a new workspace starts with.

use super::*;

pub fn sample_collection() -> Collection {
    let mut c = Collection::new("Sample API");
    c.description = "Example requests against a public test API. Edit, send and share them through git.".into();
    c.variables = vec![Kv::new("baseUrl", "https://jsonplaceholder.typicode.com")];
    let mut users = Folder {
        id: new_id(),
        name: "Users".into(),
        description: String::new(),
        auth: Auth::default(),
        scripts: Scripts::default(),
        items: vec![],
    };
    users.items.push(Item::Request(Request {
        name: "List users".into(),
        url: "{{baseUrl}}/users?_limit=5".into(),
        params: vec![Kv::new("_limit", "5")],
        checks: vec![Check { kind: CheckKind::StatusEquals, target: String::new(), expected: "200".into(), enabled: true }],
        scripts: Scripts {
            pre: String::new(),
            post: "pm.test(\"Returns a list of users\", () => {\n  const users = pm.response.json();\n  pm.expect(users).to.be.an(\"array\");\n  pm.environment.set(\"userId\", users[0].id);\n});\n\nconsole.log(\"First user:\", pm.response.json()[0].name);\n".into(),
        },
        ..Default::default()
    }));
    users.items.push(Item::Request(Request { name: "Get user".into(), url: "{{baseUrl}}/users/1".into(), ..Default::default() }));
    let mut posts = Folder {
        id: new_id(),
        name: "Posts".into(),
        description: String::new(),
        auth: Auth::default(),
        scripts: Scripts::default(),
        items: vec![],
    };
    posts.items.push(Item::Request(Request {
        name: "Create post".into(),
        method: "POST".into(),
        url: "{{baseUrl}}/posts".into(),
        headers: vec![Kv::new("Accept", "application/json")],
        body: Body {
            mode: BodyMode::Raw,
            raw: "{\n  \"title\": \"Hello\",\n  \"body\": \"First post\",\n  \"userId\": 1\n}".into(),
            raw_lang: RawLang::Json,
            ..Default::default()
        },
        ..Default::default()
    }));
    posts.items.push(Item::Request(Request {
        name: "Update post".into(),
        method: "PUT".into(),
        url: "{{baseUrl}}/posts/1".into(),
        body: Body {
            mode: BodyMode::Raw,
            raw: "{\n  \"id\": 1,\n  \"title\": \"Updated\"\n}".into(),
            raw_lang: RawLang::Json,
            ..Default::default()
        },
        ..Default::default()
    }));
    posts.items.push(Item::Request(Request {
        name: "Delete post".into(),
        method: "DELETE".into(),
        url: "{{baseUrl}}/posts/1".into(),
        ..Default::default()
    }));
    c.items.push(Item::Folder(users));
    c.items.push(Item::Folder(posts));
    c.items.push(Item::Request(Request {
        name: "Echo headers".into(),
        url: "https://httpbin.org/headers".into(),
        headers: vec![Kv::new("X-Demo", "{{token}}")],
        ..Default::default()
    }));
    c
}

pub fn sample_environment() -> Environment {
    Environment { id: new_id(), name: "Development".into(), values: vec![Kv::new("token", "dev-123"), Kv::new("userId", "1")] }
}
