[简体中文](README.zh-CN.md) | **English**

# tinyTD

> A tiny, tidy todo.txt desktop app · Rust + Tauri 2 + vanilla JS (no frontend framework) · Windows / macOS / Linux

A minimal task notepad for fast-paced AI workflows: open it, type, press Enter. No reminders or scheduling, just quick capture, quick cleanup and easy review.

## Download

Grab the installer for your platform from the [Releases](https://github.com/metellica/tinyTD/releases) page: Windows `.exe`, macOS `.dmg`, Linux `.deb` / `.AppImage`.

> The installers are not code-signed: Windows shows a SmartScreen prompt (click "More info → Run anyway"), and on macOS right-click the app and choose "Open" the first time.

## Features

| Action | How |
| --- | --- |
| Add a task | Type in the top input box, press `Enter` |
| Projects | Optional: put `+project` anywhere in the text (e.g. `Book hotel +JapanTrip`). Once any task has one, a filter bar (All / +Work / +JapanTrip …) appears under the input box; the selected filter is remembered. While filtering, new tasks without a `+project` are added to the current one. Click a task's `+project` to filter by it. `+1`, `+5%`, `C++` and the like are not projects |
| Start / pause | Click the circle at the start of the row: ○ to do → ◉ in progress (highlighted, pinned to the top, live timer) → ⏸ paused (time accumulates) → click again to resume; pausing within 1 minute counts as a misclick and returns it to a plain to-do |
| Complete | `Shift` + click the circle; the task moves to "Done" with the actual time spent. Hold `Shift` while hovering the circle to preview ✓ |
| Edit / notes | Click a task (anywhere except the circle and row buttons) to expand it in place into a card: edit the title directly (`Enter` jumps to the notes, the modified time is recorded) and write notes below it: rich text, paste or drop screenshots, auto-saved. `⤢` switches to full screen. Click `⌃`, click outside or press `Esc` to collapse. Only one card is open at a time; tasks with notes show a faint `≡` mark |
| Pause / resume notes | Pausing expands the card with a `⏸ time` divider so you can jot down where you stopped (just click elsewhere to skip, nothing is kept); resuming a task with notes expands them so you can pick up the context (can be turned off in Settings) |
| Delete | Click `✕` at the end of the row, then click again to confirm |
| Reorder | Drag the `⠿` handle; drop on empty space to move to the end |
| Restore | Click `↩` in the Done list |
| Clear done | "Clear" at the top right (asks for confirmation); while filtering by a project, only that project's completed tasks are cleared |
| Show / hide | Global hotkey (default `Ctrl+K`, configurable in Settings) or a click on the tray icon; launching the app again just brings up the running window (single instance) |
| Settings | ⚙ at the top right: global hotkey, close-button behavior (minimize to taskbar / hide to tray / quit; default is minimize), expand notes on pause / resume, light / dark / follow system, 中文 / English, font and font size |
| Quit | Depends on the close-button setting; right-click the tray icon → "Quit" always works |

- 🕒 **Timestamps**: every task records its created time and last-modified time, plus a completed time when done, shown in small text under the task.
- 🌗 **Light / dark**: follows the system theme by default, or pick one in Settings.
- 📄 **Plain-text storage**: your data is two todo.txt-style text files. Edit them in Notepad, sync them via cloud storage, track them in git, or search them with grep.

## Data format

Files live in the app data directory (use the "Data files" button at the bottom right to open it; hover to see the full path):

- Windows: `%APPDATA%\com.tinytd.app`
- macOS: `~/Library/Application Support/com.tinytd.app`
- Linux: `~/.config/com.tinytd.app`

It contains:

- `todo.txt`: active tasks, **line order is task order**
- `done.txt`: completed tasks (appended in completion order)
- `settings.json`: UI settings (maintained by the settings panel)
- `notes/<id>.md`: task notes in Markdown (the line carries `note:<id>`); images live in `notes/<id>/`. Deleting a task or clearing Done removes its notes too

> For a portable setup or syncing (cloud drive / git), set the `TINYTD_DATA_DIR` environment variable to point the data directory at any folder, for example:
> `$env:TINYTD_DATA_DIR = 'D:\sync\tinytd'`

```text
todo.txt:
2026-11-25T16:09 Reply to the client's email
2026-11-25T16:12 Fix login page bug mod:2026-11-25T18:02
2026-11-25T16:20 Refactor login module doing:2026-11-27T10:30 spent:45
2026-11-25T16:30 Tidy up meeting notes spent:20
2026-11-25T16:40 Hook up payment callback spent:130 note:mi2x8k0a
2026-11-26T09:10 Book hotel +JapanTrip

done.txt:
x 2026-11-25T20:15 2026-11-25T16:09 Write weekly report spent:75 mod:2026-11-25T17:40

notes/mi2x8k0a.md:
### ⏸ 2026-11-27 18:40
Waiting on Zhang for the /refund API doc, branch **feat/pay-cb**
![](mi2x8k0a/20261127-184012.png)
```

- Each line starts with the created time (`YYYY-MM-DDTHH:MM`, local time; classic date-only `YYYY-MM-DD` todo.txt lines are also accepted)
- `mod:` marks the last-modified time (only present after the text was actually changed)
- `doing:` means in progress and records when the current session started; `spent:` is the minutes accumulated before it. `doing:` present = in progress; only `spent:` = paused
- On completion the running session is added to `spent:`, so the time shown in Done excludes paused time; restoring a task clears both
- `note:` links the task to its notes file; the note supports a Markdown subset (`#` headings, `-` / `1.` lists, bold, italic, strikethrough, code, links, images), other formatting is dropped
- Completed lines start with `x <completed time>`, following the todo.txt convention
- `+project` is the todo.txt project marker: a whitespace-separated word starting with `+` followed by a letter (CJK included). It stays in the text; the list shows it in small text under the title
- One gotcha: avoid ending a task's text with something shaped exactly like `mod:<timestamp>`, `doing:<timestamp>`, `spent:<number>` or `note:<id>`, since it would be parsed as a marker

## Build from source

Requirements: Rust stable, plus the WebView prerequisites for your platform:

- **Windows**: MSVC toolchain + WebView2 runtime (usually preinstalled on Win10/11)
- **macOS**: Xcode Command Line Tools
- **Linux**: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (Debian/Ubuntu package names)

```powershell
cd src-tauri
cargo build --release
# Output: src-tauri/target/release/tinytd (tinytd.exe on Windows)
```

During development run `cargo test` (unit tests for todo.txt parsing and settings). After installing tauri-cli, `cargo tauri build` produces installers: NSIS on Windows, app / dmg on macOS, deb / AppImage on Linux (see `src-tauri/tauri.*.conf.json`).

- `.github/workflows/ci.yml`: tests and builds on all three platforms for every push
- `.github/workflows/release.yml`: pushing a `v*` tag (e.g. `git tag v0.1.1 && git push origin v0.1.1`) builds the installers and publishes them to Releases; bump the version in `tauri.conf.json` and `Cargo.toml` first
- Building the macOS dmg locally needs `icons/icon.icns`; generate it with `cargo tauri icon src-tauri/icons/icon.png` (CI does this automatically)
- Global hotkeys may not work under Linux Wayland

```text
tinyTD/
├─ src/                    Frontend (no framework, embedded in the binary)
│  ├─ index.html
│  ├─ style.css
│  ├─ i18n.js              Chinese / English UI strings
│  ├─ md.js                Markdown subset ⇄ note editor HTML
│  └─ app.js
├─ src-tauri/
│  ├─ tauri.conf.json      Tauri 2 config (window / assets / icons)
│  ├─ tauri.{windows,macos,linux}.conf.json   Per-platform bundle targets
│  ├─ capabilities/        Permission declarations
│  ├─ icons/               App icons
│  └─ src/
│     ├─ lib.rs            todo.txt parsing + invoke commands + unit tests
│     └─ main.rs           Entry point
├─ .github/workflows/      CI and release
├─ scripts/make_icons.py   Generates icons with Pillow
└─ README.md / README.zh-CN.md
```

## Troubleshooting

### The exe crashes on launch (`Failed to setup app: Access is denied. (os error 5)`)

**Cause**: some enterprise antivirus products (observed with Athena EPP, with Windows Defender disabled) block **unsigned exes** from creating folders or writing files under `%LOCALAPPDATA%` / `%APPDATA%` / `%TEMP%`. On first launch Tauri 2 must create the WebView2 data folder `%LOCALAPPDATA%\com.tinytd.app`; when that is blocked, the app panics and exits.

**Why other locally built exes seem fine**: they are not whitelisted, they just never hit the trigger:
- Older Tauri apps already created their data folders on first launch and skip creation afterwards; their web data is written by the **Microsoft-signed** `msedgewebview2.exe`, which is not blocked.
- CLI tools that don't write to AppData are unaffected.
- Signed or reputation-whitelisted programs (such as PowerShell) are unaffected.

Observed on the same machine: an unsigned exe can write to `D:\` but every write to AppData fails with `Access denied`.

**Fixes** (pick one):

1. **Recommended**: add `tinytd.exe` to the trust list / whitelist of your antivirus.
2. Pre-create the folders once (PowerShell):

   ```powershell
   New-Item -ItemType Directory "$env:LOCALAPPDATA\com.tinytd.app"  # WebView2 data
   New-Item -ItemType Directory "$env:APPDATA\com.tinytd.app"       # app data
   ```

   Launching and saving data work normally afterwards.
3. To keep the data directory out of AppData entirely (portable / synced), redirect it with the environment variable, but the WebView2 folder from option 2 still has to be pre-created:

   ```powershell
   $env:TINYTD_DATA_DIR = 'D:\sync\tinytd'
   ```

## Design choices

- **Not included**: reminders / notifications, priorities, due dates, or a project manager (no project settings, colors or nesting). In fast-paced work you need a place to jot something down in two seconds, not yet another planner.
- **Included**: instant input, drag-to-reorder, edit timestamps, a reviewable and restorable done list, optional todo.txt `+project` markers to separate work / life / a trip without adding a step to input, and plain text you can move anywhere.

License: MIT
