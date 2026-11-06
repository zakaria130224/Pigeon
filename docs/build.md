# Build guide

## What you need

| System | Install |
| --- | --- |
| All | [rustup](https://rustup.rs) and git. The exact Rust version comes from `rust-toolchain.toml` automatically. |
| macOS | Xcode Command Line Tools: `xcode-select --install` |
| Windows | Visual Studio Build Tools with "Desktop development with C++" (rustup offers these) |
| Debian or Ubuntu | `sudo apt install build-essential libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev` |

A C compiler is required because the JavaScript engine (QuickJS) and the TLS library (ring) include C code.

## Everyday commands

```sh
cargo run -p pigeon-app                 # run the app
cargo build --release -p pigeon-app     # optimised build, in target/release
cargo test --workspace                  # all tests
cargo fmt --all                         # format the code
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Continuous integration fails on any formatting difference or lint warning, so run the last two before pushing.

## Tests

`crates/pigeon-core/tests` holds the tests, grouped by area:

| File | Covers |
| --- | --- |
| `workspace.rs` | Saving and loading, file names, carrying over old settings |
| `team_sync.rs` | Two users pushing, pulling and resolving a conflict through a real git repository |
| `requests.rs` | Variables, cURL, retrying dropped connections |
| `formats.rs` | Collection and OpenAPI import and export |
| `scripting.rs` | Scripts against a local test server, including failures and runaway scripts |
| `suggestions.rs` | Which variables to suggest and what accepting one writes |

Some tests are skipped by default:

```sh
cargo test --workspace -- --ignored                     # needs internet access
OPENAPI_FILE=api.json cargo test openapi_real_document -- --ignored --nocapture
```

## Checking the real window

`--screenshot` opens the app with sample data, leaving your workspace alone, then saves a picture and exits. It's
the quickest check that a build draws correctly:

```sh
target/release/pigeon --screenshot now.png
```

The `qa` feature adds scripted clicks and typing, for testing what unit tests can't reach, such as keyboard focus.
It's never part of a released build. Each line of `PIGEON_QA_EVENTS` is a frame number and an action
(`click`, `down`, `up`, `move`, `text` or `key`). Frames run up to 120, and positions are in window points:

```sh
PIGEON_QA_EVENTS='20 click 900 153
40 text /{{
60 text b' cargo run -p pigeon-app --features qa -- --screenshot suggest.png
```

## Building packages

| Package | Command | Runs on |
| --- | --- | --- |
| macOS app and disk image | `./packaging/macos/package.sh` | A Mac |
| Windows installer and zip, Linux `.deb` and `.tar.gz` | `./packaging/build-windows-linux.sh` | Any computer with Docker |

Results go to `dist/`. Signing and publishing are in the [distribution runbook](distribution-runbook.md).

The macOS script builds for the Mac it runs on. Windows and Linux packages are 64-bit Intel/AMD (`x86_64`).
Docker keeps its compiler cache in the volumes `pigeon-target` and `pigeon-cargo-registry`, which you can delete
to reclaim space.

## Changing the icon

Edit `assets/icon/pigeon.svg`, then regenerate `pigeon-256.png`, `pigeon-512.png`, `pigeon.ico` (16 to 256 pixels)
and `pigeon.icns` from it. On a Mac, `rsvg-convert` renders the sizes, `magick` makes the `.ico` and `iconutil`
makes the `.icns`.
