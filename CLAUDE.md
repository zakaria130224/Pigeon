# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Pigeon is a desktop API client (Rust, egui) whose collections are plain JSON files shared through a git repository.
There is no account or cloud service. The Rust toolchain is pinned in `rust-toolchain.toml` and installs itself.

## Commands

```sh
cargo run -p pigeon-app                                  # run the app
cargo test --workspace                                   # all tests
cargo test -p pigeon-core --test scripting               # one test file (see crates/pigeon-core/tests)
cargo test -p pigeon-core --test scripting <name>        # one test, by name filter
cargo test --workspace -- --ignored                      # tests that need internet access
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

CI runs `cargo fmt --all --check`, clippy as above with `--locked`, and `cargo test --workspace --locked` on macOS,
Windows and Linux. Any formatting difference or lint warning fails it. `team_sync` tests need a git identity set
globally (`user.name` and `user.email`).

Checking the real window, for what tests can't reach (the binary is named `pigeon`, not `pigeon-app`):

```sh
cargo run -p pigeon-app -- --screenshot now.png          # sample data, draws ~120 frames, saves, exits
PIGEON_QA_EVENTS='20 click 900 153
40 text /{{' cargo run -p pigeon-app --features qa -- --screenshot out.png
```

`--screenshot` leaves your workspace alone. Each `PIGEON_QA_EVENTS` line is a frame number (up to 120) and an action:
`click`, `down`, `up`, `move`, `text` or `key`, with positions in window points. With `qa`, `PIGEON_QA_PROJECTS` lists
recent folders, one per line, and `PIGEON_QA_SYNC=1` (or `conflict`) fills Team Sync. `qa` is never part of a release build.

Packages go to `dist/`: `./packaging/macos/package.sh` on a Mac, `./packaging/build-windows-linux.sh` anywhere with
Docker. The version lives only in `Cargo.toml` under `[workspace.package]`, and a release tag `vX.Y.Z` must match it.

## Architecture

Two crates: `pigeon-app` (window, egui) depends on `pigeon-core` (no UI). Logic belongs in `pigeon-core`, where almost
all tests live, as integration tests in `crates/pigeon-core/tests` grouped by area. See `docs/architecture.md` for the
module tables.

### The app's frame loop

The UI is immediate mode: `App::ui` redraws everything each frame from current state. Three rules follow from that:

- **Actions.** Drawing code doesn't change collections or tabs while walking them. It pushes an `Action`, applied
  between frames in `app/actions.rs`.
- **The open tab.** While drawn, the active tab is taken out of `App::tabs` and put back afterwards, so its view can
  hold `&mut self` and `&mut tab` together. Anything changing the tab list must go through an `Action`.
- **Background work.** Network, script and git work never runs on the drawing thread. Each job gets a thread and
  sends its result through a channel, checked each frame by `poll_git` and `poll_responses`.

### A send

`scripting::run` owns one whole request: pre-request scripts (collection inwards, on a copy of the request), then
`http::prepare` (variables and auth), `http::send_blocking` (retries dropped connections), post-response scripts
(skipped if the call failed), then point-and-click checks merged with `pm.test` results. It returns an `Outcome`. The
collection runner calls the same function in a loop on one thread, carrying variables along.

### Scripting

`scripting/prelude.js` implements the entire `pm` API in JavaScript, and Rust passes JSON in and out. The only host
function is `__http`, for `pm.sendRequest`. Each script gets a fresh QuickJS runtime with a memory limit and an
interrupt-enforced deadline, and timers are driven from Rust. The sandbox has no file or process access.

### Workspace and sync

A workspace is one small JSON file per request, with order stored inside each file, so teammates rarely conflict
(`docs/workspace-format.md`). Sync shells out to the `git` program rather than using a library, to reuse people's
existing logins.

- Saving rewrites the tree and would delete a conflicted file, so saving pauses while any file can't be read.
- Values set by scripts stay local in `Settings::script_vars` and must never be written to the workspace, because a
  login token would reach the team repository.

## Conventions

- `unsafe` is forbidden across the workspace.
- Old workspaces must keep loading: every new field needs a serde default.
- Messages shown to users are plain words that say what to do next.
- Update `docs/user-manual.md` for anything users will notice, and add a line to `CHANGELOG.md`.

Checklists for changes that span several files:

- **A `pm` feature:** `prelude.js`, a case in `tests/scripting.rs`, and `docs/scripting-reference.md`.
- **A request field:** `model` with a serde default, `formats/collection.rs` if the format has an equivalent, and
  `docs/workspace-format.md`.
- **A new kind of tab:** a `TabKind`, drawn from `tab_strip.rs`, opened with an `Action`.
