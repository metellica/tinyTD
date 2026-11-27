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
| Complete | Click the circle at the start of the row; the task moves to "Done" |
| Edit | Click the task text to edit in place (the modified time is recorded) |
| Delete | Click `✕` at the end of the row, then click again to confirm |
| Reorder | Drag the `⠿` handle; drop on empty space to move to the end |
| Restore | Click `↩` in the Done list |
| Clear done | "Clear" at the top right (asks for confirmation) |
| Show / hide | Global hotkey (default `Ctrl+K`, configurable in Settings) or a click on the tray icon; launching the app again just brings up the running window (single instance) |
| Settings | ⚙ at the top right: global hotkey, close-button behavior (minimize to taskbar / hide to tray / quit; default is minimize), light / dark / follow system, 中文 / English, font and font size |
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

> For a portable setup or syncing (cloud drive / git), set the `TINYTD_DATA_DIR` environment variable to point the data directory at any folder, for example:
> `$env:TINYTD_DATA_DIR = 'D:\sync\tinytd'`

```text
todo.txt:
2026-11-25T16:09 Reply to the client's email
2026-11-25T16:12 Fix login page bug mod:2026-11-25T18:02

done.txt:
x 2026-11-25T20:15 2026-11-25T16:09 Write weekly report mod:2026-11-25T17:40
```

- Each line starts with the created time (`YYYY-MM-DDTHH:MM`, local time; classic date-only `YYYY-MM-DD` todo.txt lines are also accepted)
- `mod:` marks the last-modified time (only present after the text was actually changed)
- Completed lines start with `x <completed time>`, following the todo.txt convention
- One gotcha: avoid ending a task's text with something shaped exactly like `mod:<timestamp>`, since it would be parsed as the modified time

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

- **Not included**: reminders / notifications, priorities, tags, due dates. In fast-paced work you need a place to jot something down in two seconds, not yet another planner.
- **Included**: instant input, drag-to-reorder, edit timestamps, a reviewable and restorable done list, and plain text you can move anywhere.

License: MIT
