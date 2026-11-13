# Architecture

Pigeon is a Rust workspace with two crates. The split keeps everything testable without a window.

```text
pigeon-app  ──depends on──▶  pigeon-core
(window, egui)               (no user interface)
```

## pigeon-core

| Module | Responsibility |
| --- | --- |
| `model` | Collections, folders, requests, environments, tree and URL helpers, sample data |
| `workspace` | Reads and writes the workspace folder. See [Workspace format](workspace-format.md). |
| `settings` | Personal settings and history, kept outside the workspace |
| `http` | Variables, preparing a request, the network call with retries, checks, snippets, cURL |
| `scripting` | Runs a whole send: scripts, the request, more scripts, then checks |
| `formats` | Collection v2.1 import and export, OpenAPI 3 import |
| `git` | Runs the `git` command and interprets its results |
| `suggest` | Decides which variables to suggest and how accepting one changes the text |

## pigeon-app

| Module | Responsibility |
| --- | --- |
| `main`, `cli`, `crash` | Start-up, command line options, crash log |
| `ui` | Building blocks with no app state: theme, icons, syntax colouring, tables, `VarEdit` |
| `app` | State (`mod.rs`), and one file per area: `actions`, `sending`, `sync`, `workspace_state`, `sidebar`, `tab_strip`, `request_view`, `pages`, `editors`, `dialogs`, `console`, `capture` |

## How the app runs

The interface is immediate mode (egui): `App::ui` redraws everything each frame from the current state.

- **Actions.** Drawing code doesn't change collections or tabs directly while walking them. It pushes an `Action`,
  applied between frames in `actions.rs`. This avoids changing a list during its own drawing.
- **The open tab.** While drawn, the active tab is taken out of `App::tabs` and put back afterwards, so its view can
  have `&mut self` and `&mut tab` at once. Anything that changes the tab list must go through an `Action`.
- **Background work.** Network calls, scripts and git never run on the drawing thread. Each job gets a thread and
  sends its result through a channel, which `poll_git` and `poll_responses` check each frame.

## A send, step by step

`scripting::run` handles everything for one request:

1. Pre-request scripts run from the collection inwards. They can change a copy of the request and the variables.
2. Variables and authorization are applied by `http::prepare`.
3. `http::send_blocking` makes the call, retrying up to 3 times on dropped connections.
4. Post-response scripts run, unless the call failed.
5. Point-and-click checks run, and results join those from `pm.test`.
6. The `Outcome` returns logs, extra calls and which variables changed. The app shows them, and stores changes
   locally in `Settings::script_vars`.

The collection runner calls the same function in a loop on one thread, carrying the variables along.

## Scripting

`prelude.js` implements the whole `pm` API in JavaScript, and the Rust side passes JSON in and out. The engine
only needs two host functions: `__http`, for `pm.sendRequest`, and `__random`, for `crypto.getRandomValues`. This keeps
the API easy to extend. `crypto-js.js` is the unmodified crypto-js library, loaded only into scripts that name it.

Every script gets a fresh QuickJS runtime with a memory limit and a deadline enforced by an interrupt handler. Timers
are driven from Rust. The sandbox exposes no file or process access.

## Key decisions

| Decision | Why |
| --- | --- |
| One small file per request | Teammates working on different requests never conflict |
| Item order stored inside each file | Adding items doesn't touch a shared list |
| Calling the `git` program, not a library | Uses the login people already have, including SSH keys and credential helpers |
| Saving pauses when a file can't be read | Saving rewrites the tree, which would otherwise delete a conflicted file |
| Script-set values stay local | A token set by a login script must never reach the team repository |
| `--locked` builds and a pinned toolchain | The same source gives the same result on every builder |
| `unsafe` code forbidden | Set for the whole workspace in `Cargo.toml` |

## Adding things

- **A `pm` feature:** add it to `prelude.js`, add a case to `tests/scripting.rs`, and update the
  [scripting reference](scripting-reference.md).
- **A field on a request:** add it to `model` with a serde default, so older files still load. Then cover
  `formats/collection.rs` if the format has an equivalent, and add it to the [workspace format](workspace-format.md).
- **A new kind of tab:** add a `TabKind`, draw it from `tab_strip.rs`, and open it with an `Action`.
