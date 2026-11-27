**简体中文** | [English](README.md)

# tinyTD

> 小而美的 todo.txt 桌面待办 · Rust + Tauri 2 + 原生 JS（无前端框架）· Windows / macOS / Linux

为快节奏的 AI 工作流设计的极简任务速记本：打开即输入，回车即记录。不搞复杂的时间提醒，只为「快记、快清、可回看」。

## 下载

到 [Releases](https://github.com/metellica/tinyTD/releases) 页面下载对应平台的安装包：Windows `.exe`、macOS `.dmg`、Linux `.deb` / `.AppImage`。

> 安装包未做代码签名：Windows 会弹出 SmartScreen 提示（点「更多信息 → 仍要运行」），macOS 首次打开需右键选择「打开」。

## 功能一览

| 操作 | 方式 |
| --- | --- |
| 记任务 | 顶部输入框，`Enter` 添加 |
| 开始 / 暂停 | 单击行首圆圈：○ 待办 → ◉ 进行中（高亮置顶、实时计时）→ ⏸ 已暂停（累计用时）→ 再单击继续；1 分钟内暂停视为误触，直接回到待办 |
| 完成 | `Shift` + 单击圆圈，任务移入「已完成」并显示实际用时；按住 `Shift` 悬停圆圈会预览 ✓ |
| 编辑 | 点任务文字就地修改（自动记录修改时间） |
| 删除 | 行尾 `✕`，再点一次确认 |
| 调整顺序 | 拖动行首 `⠿` 把手，拖到列表空白处即放末尾 |
| 恢复 | 已完成列表里点 `↩` |
| 清空已完成 | 右上角「清空」（二次确认） |
| 唤起 / 收起 | 全局热键（默认 `Ctrl+K`，可在设置中修改），或单击托盘图标；重复启动程序也会唤起已运行的窗口（单实例） |
| 设置 | 右上角 ⚙：全局热键、关闭按钮行为（最小化到任务栏 / 隐藏到托盘 / 直接退出，默认最小化到任务栏）、浅色/深色/跟随系统、中文/English、字体与字号 |
| 退出 | 取决于「关闭按钮行为」设置；右键托盘图标 →「退出」随时可真正退出 |

- 🕒 **时间留痕**：每条任务记录「创建时间」与「最后修改时间」，完成时记录「完成时间」，都以小字显示在任务下方。
- 🌗 **明暗自适应**：默认跟随系统深浅色模式，也可在设置中手动指定。
- 📄 **纯文本存储**：数据就是两个 todo.txt 风格的文本文件，可以用记事本直接改、放进网盘同步、用 git 管理、用 grep 检索。

## 数据格式

文件保存在应用数据目录（界面右下角「数据文件」按钮可一键打开；悬停可看到完整路径）：

- Windows：`%APPDATA%\com.tinytd.app`
- macOS：`~/Library/Application Support/com.tinytd.app`
- Linux：`~/.config/com.tinytd.app`

目录里有：

- `todo.txt` —— 进行中任务，**行序即任务顺序**
- `done.txt` —— 已完成任务（按完成先后追加）
- `settings.json` —— 界面设置（设置面板自动维护）

> 需要便携或同步（网盘 / git）时，设置环境变量 `TINYTD_DATA_DIR` 即可把数据目录指到任意文件夹，例如：
> `$env:TINYTD_DATA_DIR = 'D:\sync\tinytd'`

```text
todo.txt:
2026-11-25T16:09 给客户回邮件
2026-11-25T16:12 修复登录页显示 bug mod:2026-11-25T18:02
2026-11-25T16:20 重构登录模块 doing:2026-11-27T10:30 spent:45
2026-11-25T16:30 整理周会纪要 spent:20

done.txt:
x 2026-11-25T20:15 2026-11-25T16:09 写周报 spent:75 mod:2026-11-25T17:40
```

- 每行开头是创建时间（`YYYY-MM-DDTHH:MM`，本地时间；也兼容纯日期 `YYYY-MM-DD` 的老式 todo.txt 行）
- `mod:` 标记最后修改时间（只在内容真正改过之后出现）
- `doing:` 表示进行中，记录本段开始时间；`spent:` 是之前累计投入的分钟数。有 `doing:` 为进行中，只有 `spent:` 为已暂停
- 完成时把进行中的这一段计入 `spent:`，「已完成」显示的用时不含暂停时间；恢复为待办时两者都清除
- 完成行以 `x <完成时间>` 开头，符合 todo.txt 的完成约定
- 唯一注意：任务文本结尾请避免恰好是 `mod:时间`、`doing:时间` 或 `spent:数字` 形式的字样（会被当作标记解析掉）

## 从源码构建

依赖：Rust stable，以及各平台的 WebView 运行环境：

- **Windows**：MSVC 工具链 + WebView2 运行时（Win10/11 一般自带）
- **macOS**：Xcode Command Line Tools
- **Linux**：`libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`（Debian/Ubuntu 包名）

```powershell
cd src-tauri
cargo build --release
# 产物：src-tauri/target/release/tinytd（Windows 为 tinytd.exe）
```

日常开发：`cargo test`（todo.txt 解析与设置逻辑的单元测试）。安装 tauri-cli 后运行 `cargo tauri build` 可生成安装包：Windows 为 NSIS，macOS 为 app / dmg，Linux 为 deb / AppImage（见 `src-tauri/tauri.*.conf.json`）。

- `.github/workflows/ci.yml`：每次推送在三个平台上跑测试与构建
- `.github/workflows/release.yml`：推送 `v*` tag（如 `git tag v0.1.1 && git push origin v0.1.1`）自动打包并发布到 Releases；发版前先同步修改 `tauri.conf.json` 与 `Cargo.toml` 的版本号
- 本地打 macOS dmg 需要 `icons/icon.icns`，可用 `cargo tauri icon src-tauri/icons/icon.png` 生成（CI 里已自动处理）
- Linux Wayland 下全局热键可能不可用

```text
tinyTD/
├─ src/                    前端（无框架，随二进制内嵌）
│  ├─ index.html
│  ├─ style.css
│  ├─ i18n.js              中 / 英界面文案
│  └─ app.js
├─ src-tauri/
│  ├─ tauri.conf.json      Tauri 2 配置（窗口/资源/图标）
│  ├─ tauri.{windows,macos,linux}.conf.json   各平台安装包类型
│  ├─ capabilities/        权限声明
│  ├─ icons/               应用图标
│  └─ src/
│     ├─ lib.rs            todo.txt 解析 + invoke 命令 + 单元测试
│     └─ main.rs           入口
├─ .github/workflows/      CI 与发布
├─ scripts/make_icons.py   用 Pillow 生成图标
└─ README.md / README.zh-CN.md
```

## 常见问题

### 双击 exe 闪退（`Failed to setup app: Access is denied. (os error 5)`）

**原因**：本机企业杀软（Athena EPP，Windows Defender 已被禁用）会拦截**未签名 exe** 在 `%LOCALAPPDATA%` / `%APPDATA%` / `%TEMP%` 下新建目录或写文件。Tauri 2 首次启动必须创建 WebView2 数据目录 `%LOCALAPPDATA%\com.tinytd.app`，这一步被拦截后应用直接 panic 退出。

**为什么其他本地编译的 exe 没这个问题**：不是它们被放行，而是没踩到触发条件——
- 旧 Tauri 应用（如 ShuttleOne 等）的数据目录早在首启时就已建好，之后每次启动都跳过创建，且网页数据实际由**微软签名**的 `msedgewebview2.exe` 写入，不受影响；
- 不写 AppData 的 CLI 工具自然无感；
- 已签名/在信誉白名单里的程序（如 PowerShell）也不受影响。

实测对照：同一台机器上，未签名 exe 写 `D:\` 正常，写 AppData 全部 `Access denied`。

**解决**（任选其一）：

1. **推荐**：把 `tinytd.exe` 加入 Athena EPP 的信任/白名单；
2. 预建一次目录（PowerShell）：

   ```powershell
   New-Item -ItemType Directory "$env:LOCALAPPDATA\com.tinytd.app"  # WebView2 数据
   New-Item -ItemType Directory "$env:APPDATA\com.tinytd.app"       # 应用数据
   ```

   之后启动与数据写入均正常；
3. 数据目录想彻底避开 AppData（便携 / 网盘同步）时，用环境变量重定向，但 WebView2 数据目录仍需按上一条预建：

   ```powershell
   $env:TINYTD_DATA_DIR = 'D:\sync\tinytd'
   ```

## 设计取舍

- **不做**：提醒/通知、优先级、标签、截止日期——在快节奏工作里，你需要的是一个「2 秒记完一件事」的地方，而不是又一个日程管理软件。
- **做了**：一键快速输入、拖拽排序、修改留痕、完成列表可回看/可恢复、纯文本随时可迁移。

License: MIT
