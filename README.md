# Pigeon

Pigeon is a desktop API client for macOS, Windows and Linux. Build and send HTTP requests, organise them
into collections, test them with JavaScript, and share everything with your team through a git repository
using Pull and Push. There is no account and no cloud service: your work is plain files in a folder you own.

## Features

- Requests with parameters, headers, authorization and JSON, text, form or URL-encoded bodies
- Collections, folders, a collection runner and request history
- Environments and `{{variables}}`, with suggestions as you type
- JavaScript pre-request and post-response scripts using the `pm` API
- Point-and-click response checks
- Console showing every call in full
- Import from OpenAPI 3 JSON, v2.1 collection JSON and cURL
- Team sync through git, with help for resolving conflicts

## Documents

| Document | For |
| --- | --- |
| [User manual](docs/user-manual.md) | Everyone using Pigeon |
| [Scripting reference](docs/scripting-reference.md) | People writing scripts |
| [Troubleshooting](docs/troubleshooting.md) | When something goes wrong |
| [Build guide](docs/build.md) | Developers compiling and testing Pigeon |
| [Distribution runbook](docs/distribution-runbook.md) | Whoever packages, signs and publishes releases |
| [Architecture](docs/architecture.md) | Developers changing the code |
| [Workspace format](docs/workspace-format.md) | Anyone reading or generating workspace files |
| [Contributing](CONTRIBUTING.md), [Security](SECURITY.md), [Changelog](CHANGELOG.md) | Project information |

## Quick start for developers

```sh
cargo run -p pigeon-app      # run the app
cargo test --workspace       # run the tests
```

The Rust version is pinned in `rust-toolchain.toml` and installs itself. See the [build guide](docs/build.md).

## Project layout

```text
crates/pigeon-core   Everything that doesn't need a window: requests, scripts, git, files
crates/pigeon-app    The desktop app
assets/icon          The icon, in every format the systems need
packaging            Recipes for the macOS, Windows and Linux packages
docs                 The documents above
.github/workflows    Automatic checks and release builds
```
