# Claude Usage Widget

**English** · [Tiếng Việt](README.vi.md)

A floating widget and system tray icon that show your Claude Code usage: how
much of your plan limit is left, and what your token traffic would cost at API
prices.

Runs on Windows and macOS. Makes no network calls — every number is read from
your own machine.

## What it shows

```
┌──────────────────────────────────┐
│  Session          41%            │
│  ████████░░░░░░░░  1h52m left    │
│  at this pace → 68% at reset     │
├──────────────────────────────────┤
│  Week (all)       23%  ███░░░░   │
│  Week (Fable)     22%  ███░░░░   │
├──────────────────────────────────┤
│  Today        $85.31             │
│  1.4× the 7-day average          │
│  ▁▂▅▃█▂▁▄▆▃▂▅█▃                  │
└──────────────────────────────────┘
```

The most useful line is the third one: it tells you where you are heading, not
where you are. If your current pace would use up the limit before it resets,
it states the time you will run out.

## What the dollar amount means

It is **what that traffic would cost if it were billed at API prices**, not
money taken from your account. On a subscription plan you spend limit, not
money, so the two numbers differ. The app shows both and labels which is which.

## Where the data comes from

Two sources, both already on your machine:

| Source | Provides | Read every |
|---|---|---|
| `~/.claude/projects/**/*.jsonl` | tokens, API-equivalent cost, split by project and model | 3 seconds |
| `claude -p "/usage"` | session and weekly limit percentages | 3 minutes |

The `/usage` command does not call a model, so it costs no tokens — measured as
`total_cost_usd: 0` and `num_turns: 0` in the JSON it returns.

The app does **not** read `~/.claude/.credentials.json` and does **not** send
anything anywhere. Rationale in
[ADR 0003](docs/decisions/0003-khong-doc-credentials.md).

The app locates the `claude` binary itself instead of relying on the inherited
`PATH` — an app launched from Finder on macOS does not get your terminal's
`PATH`. If it still reports that `claude` cannot be found, run `which claude`
and paste the path into the **Path to the `claude` command** field in the
settings window. Rationale and the four lookup steps:
[ADR 0004](docs/decisions/0004-tim-lenh-claude.md).

## Development setup

Install the Rust toolchain:

```bash
winget install Rustlang.Rustup                                      # Windows
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh      # macOS
```

Windows 11 ships with WebView2, so nothing else is needed. macOS uses the
system WKWebView, also nothing to install, but the linker needs the Xcode
Command Line Tools:

```bash
xcode-select --install     # skip if already installed
```

Then:

```bash
npm install
npm run tauri dev
```

Packaging: `npm run tauri build`. On macOS it produces a `.dmg`, on Windows an
NSIS installer. Cross-compiling is not possible, and any target that does not
belong to the current platform is skipped **without an error**.

## Building when a proxy blocks GitHub

`npm run tauri build` downloads the NSIS toolset to package the installer.
Behind a proxy that blocks `github.com` this step fails with
`CONNECT proxy failed: 407`, even though `cargo` and `npm` themselves work.

Workaround: fetch the two files some other way and put them where Tauri looks;
it then skips the download.

| File | Source | SHA1 |
|---|---|---|
| `nsis-3.11.zip` | `github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/` | `EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D` |
| `nsis_tauri_utils.dll` | `github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/` | `75197FEE3C6A814FE035788D1C34EAD39349B860` |

```bash
D="$LOCALAPPDATA/tauri/NSIS"
mkdir -p "$D" && unzip -q nsis-3.11.zip -d /tmp/nx && cp -r /tmp/nx/nsis-3.11/. "$D/"
mkdir -p "$D/Plugins/x86-unicode" && cp nsis_tauri_utils.dll "$D/Plugins/x86-unicode/"
```

Tauri checks exactly the paths below; **if one is missing it deletes the whole
directory and downloads everything again**, so all of them must be in place
before you run the build:

```
makensis.exe                    Include/MUI2.nsh
Bin/makensis.exe                Include/FileFunc.nsh
Stubs/lzma-x86-unicode          Include/x64.nsh
Stubs/lzma_solid-x86-unicode    Include/nsDialogs.nsh
Plugins/x86-unicode/            Include/WinMessages.nsh
  nsis_tauri_utils.dll          Include/Win/COM.nsh
                                Include/Win/Propkey.nsh
                                Include/Win/RestartManager.nsh
```

Versions must match: the current Tauri CLI requires NSIS 3.11 and plugin
v0.5.3; any other version is rejected by its SHA1.

## Checks before committing

```bash
cargo fmt   --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test  --manifest-path src-tauri/Cargo.toml
```

## Documentation

The design docs are written in Vietnamese.

| File | Contents |
|---|---|
| [docs/backlog.md](docs/backlog.md) | work queue, 35 items, each with acceptance criteria |
| [docs/data-sources.md](docs/data-sources.md) | the two data sources, with real measurements |
| [docs/architecture.md](docs/architecture.md) | architecture and data flow |
| [docs/ui-spec.md](docs/ui-spec.md) | layout and settings |
| [docs/decisions/](docs/decisions/) | why Tauri, why two sources, why tokens are never touched |
| [AGENTS.md](AGENTS.md) | rules for AI agents working in this repo |

## Measurements

Measured on release builds:

| | Windows 11 | macOS 15 ARM |
|---|---|---|
| Installer | 2.0 MB (NSIS) | 2.7 MB (`.dmg`) |
| Binary size | 8.8 MB | 6.2 MB (Mach-O arm64) |
| Clean rebuild | — | 2 min 7 s |
| Idle RAM | 181 MB private (app 8.5 MB + 6 WebView2 processes) | not measured |
| Scan of 98 MB of transcripts | 207 ms | not measured |
| One `claude -p /usage` run | ~2.9 s, 0 tokens | ~2.9 s, 0 tokens |

The RAM figure is far higher than first expected and weakens the case for
choosing Tauri — see [ADR 0001](docs/decisions/0001-tauri-thay-vi-electron.md),
the addendum at the end of the file.

## Status

33 of 35 items done. The macOS installer builds; nothing is blocking.

Both remaining items are waiting for someone to open the app on macOS and
check it by eye, not for code:

- **CUW-052** menu bar text. The number showing twice is fixed; what remains is
  settling the monochrome icon — the current app icon cannot be used as a
  template image (its alpha channel is a solid square, so it would render as a
  black box).
- **CUW-082** macOS installer. Still to check: the badge on the Dock icon, that
  the "Hide from the taskbar" option does not appear in settings, and that settings
  are saved to `~/Library/Application Support/claude-usage-widget/`.

Three bugs that only showed up when actually running on macOS, all fixed: the
`claude` command could not be found (CUW-023), the number showed twice in the
menu bar (CUW-052), and the app asked for `Desktop`/`Music` access because a
child process inherited `/` as its working directory (CUW-024).

Details in [docs/backlog.md](docs/backlog.md).

## License

[MIT](LICENSE)
