# CLAUDE.md

Read `AGENTS.md` first — it holds the project rules, the layout, and the
non-negotiables. This file adds only what is specific to Claude Code.

## Commands

```bash
npm run tauri dev       # run the app with hot reload
npm run tauri build     # produce installers
cargo check   --manifest-path src-tauri/Cargo.toml
cargo clippy  --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test    --manifest-path src-tauri/Cargo.toml
cargo fmt     --manifest-path src-tauri/Cargo.toml
```

The verification gate before marking any backlog item done: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`. All three, actually
executed.

`--all-targets` is not decoration: without it clippy skips test code, and a
test-only mistake (a missing trait import) passed the gate once and only turned
up on the next run.

## The installer does not follow a debug build

`cargo test`, `cargo run` and `cargo build` all produce `target/debug/`. The
release binary and the installer only change when `npm run tauri build` runs.

This has already been got wrong once: two features were verified in a debug
build and reported as done while the installer on disk still predated them.
After changing anything a person installs rather than runs from the repo, run
the release build too, and check the timestamps rather than assuming:

```bash
# macOS — BSD ls has no --time-style, so use stat
stat -f '%Sm  %z bytes  %N' -t '%H:%M:%S' \
  src-tauri/target/release/bundle/dmg/*.dmg
# Windows / Linux
ls -la --time-style=+%H:%M:%S src-tauri/target/release/bundle/nsis/*.exe
```

What each platform actually produces:

- **macOS** builds only the `.dmg`, whatever `tauri.conf.json` lists. The `nsis`
  target is skipped silently, with no error. `bundle/macos/` is left **empty**:
  Tauri copies the `.app` into the `.dmg` and then deletes it (`Cleaning …` in
  the log), so the `.dmg` is the only artifact whose timestamp can be checked.
- **Windows** builds the NSIS installer. Cross-compiling either one is not
  possible, and `cargo check --target x86_64-pc-windows-msvc` does not work from
  macOS either: `tauri-winres` needs `llvm-rc`. To type-check a
  `#[cfg(windows)]` block from macOS, drop the gate, run clippy, put it back.

## Reading the user's own transcripts

This app parses `~/.claude/projects/**/*.jsonl` — the same files that hold this
user's conversations. When developing, prefer the fixtures in
`src-tauri/tests/fixtures/` over the live directory. If a task genuinely needs
live data, read only the `usage`, `model`, `timestamp`, `requestId`, `cwd`, and
`sessionId` fields; never read or quote `message.content`.

## Do not run the app to "check usage"

`claude -p "/usage"` is the app's data source, not a debugging shortcut. Each
call spawns a full CLI (~2.9s). Use the fixture in
`src-tauri/tests/fixtures/usage-output.txt` when working on the parser.

## Scope

Anything touching authentication, credentials, or outbound network calls is out
of scope by default — see `AGENTS.md` non-negotiables 1 and 2. Raise it with the
user rather than implementing it.
