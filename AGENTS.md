# AGENTS.md

Instructions for any AI agent working in this repository. Agent-agnostic; see
`CLAUDE.md` for Claude Code specifics.

## What this project is

A desktop widget + tray indicator showing Claude Code usage: how much of the
subscription rate limit is spent, and what the token traffic would cost at API
rates. Tauri v2 (Rust backend, HTML/CSS/JS frontend). Targets Windows and macOS.

## Non-negotiables

1. **Never read `~/.claude/.credentials.json`.** It holds an OAuth access token
   with full account scope. Rate-limit percentages come from spawning
   `claude -p "/usage"`, never from calling internal endpoints with that token.
   See `docs/decisions/0003-no-credential-access.md`.
2. **Never transmit usage data anywhere.** Everything stays on the machine. The
   app makes no outbound network requests. If a task seems to need one, stop and
   ask.
3. **Deduplicate transcript records.** One API response writes several JSONL
   lines carrying the same `usage` block. Key on `message.id` + `requestId`.
   Measured on real data: 48.9% of usage blocks are duplicates. Skipping this
   roughly doubles every reported number.
4. **Price cache tiers separately.** `cache_read` is ~99% of input volume and is
   billed at a fraction of the input rate. Summing raw input tokens is wrong by
   orders of magnitude.

## Language

- **Code, comments, identifiers, commit messages, log strings: English.**
- **`README.md` and everything in `docs/`: Vietnamese.**
- `AGENTS.md` and `CLAUDE.md` stay English — they are read by tools.

## Comments

Comments record constraints the code cannot state itself: an upstream format
quirk, a platform limitation, a unit. Never narrate history ("fixed review
finding", "refactored"). That is reviewer conversation and becomes noise on
merge.

## Line endings

LF everywhere, enforced by `.gitattributes` (`* text=auto eol=lf`). The repo
used to be mixed, and it cost a whole-file diff: a tool that reads a CRLF file
and writes it back as LF rewrites every line, and the real change disappears
into 2000 rewritten lines. Editing scripts must write bytes back in the encoding
they read — check `git diff --stat` before believing a diff.

## Evidence discipline

Do not report a task green without running it. `cargo check`, `cargo clippy`,
and `cargo test` are the gates. When reporting, separate "ran it and observed X"
from "inferred X from reading the code". If a test fails, say so and paste the
output.

## Layout

```
src/                  frontend: HTML/CSS/JS, no build step, no framework
  widget.*            the floating window
  settings.*          the settings window
src-tauri/src/
  lib.rs              commands, the two polling loops, app setup
  snapshot.rs         the single struct the frontend renders
  collector.rs        runs both sources, assembles the snapshot
  pricing.rs          model price table + cost math
  transcript.rs       JSONL scanning, dedup, incremental reads
  aggregate.rs        per-day / model / project / session totals
  usage_cli.rs        spawns `claude -p "/usage"`, parses it, holds the result
  forecast.rs         projects the burn rate to the end of a window
  alerts.rs           decides when a limit is worth interrupting for
  settings.rs         persisted user settings
  window.rs           widget placement, position memory, taskbar indicator
  tray.rs             tray icon, its menu, the number it shows
  tray_icon.rs        bitmap font and badge rasterizer
tools/                one-off generators run by hand, not part of the build
docs/                 Vietnamese; backlog.md is the work queue
docs/decisions/       ADRs; add one when a choice constrains later work
```

Two rules the layout encodes. The frontend computes nothing: it renders
`Snapshot` and nothing else, so a figure that looks wrong is a backend bug.
And `pricing.rs` knows nothing about files, so the price table can change
without touching how transcripts are read.

`src-tauri/icons/` is generated from `src-tauri/icon-source.png` by
`node tools/make-icon.mjs && npx tauri icon src-tauri/icon-source.png`. Edit the
generator, never the generated files.

## Working the backlog

`docs/backlog.md` is the queue. Each item has an ID (`CUW-nnn`), acceptance
criteria, and dependencies. Take items in dependency order. When an item is
done, set its status to `done` in that file in the same change that implements
it — a backlog that disagrees with the code is worse than no backlog.

Do not start an item whose dependencies are not `done`.

## Platform facts already verified

Do not re-research these; they were checked against the live API docs and the
machine.

- `TrayIcon::set_title` works on macOS and Linux, **not Windows**. Showing a
  number in the Windows tray means rendering it into the 32x32 icon.
- Doing both — rasterizing the number into the icon *and* calling `set_title` —
  shows it twice on macOS: once in the level colour, once in the menu bar's own.
  The bitmap is there for Windows only, so `tray.rs` skips it on macOS.
- `src-tauri/icons/32x32.png` is a filled rounded square: 74% of its pixels are
  fully opaque, 16% transparent. `icon_as_template(true)` renders by alpha, so
  it would draw as a solid black tile. A monochrome menu bar icon needs its own
  asset, not this one.
- `TrayIcon::set_tooltip` works on Windows and macOS, not Linux.
- `WebviewWindow::set_progress_bar` drives the Windows taskbar button and the
  macOS Dock; on Linux it needs libunity.
- `set_badge_count` is unsupported on Windows — use `set_overlay_icon`.
- `set_skip_taskbar` is unsupported on macOS.
- `claude -p "/usage" --output-format json` returns the limit text in `.result`,
  costs no tokens (`total_cost_usd: 0`, `num_turns: 0`), and takes ~2.9s.
- A bundled macOS app is launched with `/` as its working directory, and a child
  process inherits it. The CLI takes its working directory to be the project it
  is looking at, so from `/` it reads into every protected folder in the home
  directory, and macOS asks the user to allow Desktop, Music, Documents and the
  rest **in the app's name**. `usage_cli.rs` therefore runs the command in an
  empty directory of its own. Measured on this machine: five polls, every
  session file carrying `"cwd":"/"`.
- Every `/usage` poll leaves a session transcript in
  `~/.claude/projects/<cwd slug>/`, ~2.4 KB each, one file per poll. They carry
  no `usage` block, so they do not affect the figures — but they accumulate
  inside the directory the transcript scanner walks.
