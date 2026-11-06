# Contributing

1. Read the [architecture](docs/architecture.md), and set up with the [build guide](docs/build.md).
2. Create a branch from `main`.
3. Make your change with tests. Logic belongs in `pigeon-core`, where it can be tested without a window.
4. Before pushing, run:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

5. Update the [user manual](docs/user-manual.md) for anything people will notice, and add a line to the
   [changelog](CHANGELOG.md).
6. Open a pull request. CI must pass on macOS, Windows and Linux.

## Standards

- No `unsafe` code. The workspace forbids it.
- Never block the drawing thread: network, script and git work goes to a background thread.
- Keep old workspaces loading: new fields need a serde default.
- Messages shown to users are plain words that say what to do next.
