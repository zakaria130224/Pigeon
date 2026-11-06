# Workspace format

A workspace is a folder of JSON files, designed to merge well in git.

```text
<workspace>/
  collections/
    <Collection>/
      collection.json
      <Request>.request.json
      <Folder>/
        folder.json
        <Request>.request.json
  environments/
    <Environment>.env.json
```

## Rules

- **File names** come from item names. Characters some systems refuse are replaced with `_`, and names Windows
  reserves, such as `CON`, get a trailing `_`. Clashes get a short suffix. The name inside the file is the real one.
- **Order** is the `order` number in each request and folder file. Equal numbers sort by name.
- **Ids** identify items, and the file name doesn't. A duplicated id is replaced when loading.
- **Unknown files** are left alone. Pigeon only manages `collection.json`, `folder.json`, `*.request.json` and
  `*.env.json`, and removes those when their item is deleted or renamed.
- **Line endings:** files are written with LF, and CRLF from git on Windows is treated as unchanged.

## Files

Empty optional fields are left out.

**`collection.json`**: `id`, `name`, `description`, `auth`, `variables`, `scripts`

**`folder.json`**: `id`, `name`, `description`, `auth`, `scripts`, `order`

**`*.env.json`**: `id`, `name`, `values`

**`*.request.json`**:

```json
{
  "id": "0193a1b2c3d4e5f6",
  "name": "Create post",
  "method": "POST",
  "url": "{{baseUrl}}/posts",
  "params": [],
  "headers": [{ "key": "Accept", "value": "application/json", "enabled": true }],
  "auth": { "kind": "inherit" },
  "body": { "mode": "raw", "raw": "{ \"title\": \"Hello\" }", "raw_lang": "json" },
  "checks": [{ "kind": "status_equals", "expected": "201", "enabled": true }],
  "scripts": { "pre": "", "post": "pm.test(\"ok\", () => {});" },
  "description": "",
  "order": 0
}
```

| Field | Values |
| --- | --- |
| `auth.kind` | `inherit`, `none`, `bearer`, `basic`, `apikey`, with `token`, `username`, `password`, `key`, `value`, `in_query` |
| `body.mode` | `none`, `raw`, `urlencoded`, `formdata`, with `raw`, `raw_lang`, `urlencoded`, `formdata` |
| `body.raw_lang` | `json`, `text`, `xml`, `html`, `javascript` |
| `checks[].kind` | `status_equals`, `body_contains`, `time_below`, `header_exists`, `json_equals`, with `target` and `expected` |

## Secrets

Everything in these files reaches everyone with access to the repository, including tokens and passwords typed
into a request or environment. Use variables set by a script, or globals, which stay on one computer.
