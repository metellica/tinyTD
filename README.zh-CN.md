**简体中文** | [English](README.md)

# tinyTD

> 小而美的 todo.txt 桌面待办 · Rust + Tauri 2 + 原生 JS（无前端框架）· Windows / macOS / Linux

为快节奏的 AI 工作流设计的极简任务速记本：打开即输入，回车即记录。不搞复杂的时间提醒，只为「快记、快清、可回看」。

## 适用场景

- ✅ **适合**：在一台主力电脑上边工作边随手记下要做的事，做完就清，偶尔回看。任务存在时间短，通常是当天或本周的事。
- ✅ **适合**：喜欢纯文本、希望数据完全在自己手里，可以用任何编辑器或脚本处理数据的人。
- ❌ **不适合**：需要手机端、需要多台电脑同时使用并实时同步、需要团队协作或分派任务。
- ❌ **不适合**：需要提醒、截止日期、日程安排的长期计划管理。

tinyTD 不内置同步，也没有账号和服务器。如果确实要在几台电脑之间轮流使用，请先看「[多台电脑使用](#多台电脑使用)」。

## 下载

到 [Releases](https://github.com/metellica/tinyTD/releases) 页面下载对应平台的安装包：Windows `.exe`、macOS `.dmg`、Linux `.deb` / `.AppImage`。

> 安装包未做代码签名：Windows 会弹出 SmartScreen 提示（点「更多信息 → 仍要运行」），macOS 首次打开需右键选择「打开」。

## 功能一览

| 操作 | 方式 |
| --- | --- |
| 记任务 | 顶部输入框，`Enter` 添加 |
| 分项目 | 可选：在任务文字里任意位置写 `+项目`（如 `订酒店 +日本旅行`）。有项目后，输入框下方出现筛选条（全部 / +工作 / +日本旅行 …），选中的筛选会被记住；筛选时新加的任务没写 `+项目` 就自动归到当前项目；点任务下方的 `+项目` 也可直接筛选。`+1`、`+5%`、`C++` 这类不算项目 |
| 开始 / 暂停 | 单击行首圆圈：○ 待办 → ◉ 进行中（高亮置顶、实时计时）→ ⏸ 已暂停（累计用时）→ 再单击继续；1 分钟内暂停视为误触，直接回到待办 |
| 完成 | `Shift` + 单击圆圈，任务移入「已完成」并显示实际用时；按住 `Shift` 悬停圆圈会预览 ✓ |
| 编辑 / 笔记 | 单击任务（圆圈和行尾按钮以外的位置）就地展开成卡片：标题可直接修改（`Enter` 跳到笔记，自动记录修改时间），下面写笔记：富文本，可直接粘贴 / 拖入截图，自动保存。`⤢` 切到全屏；点 `⌃`、点卡片外或按 `Esc` 收起。同一时间只展开一张；写过笔记的任务右侧有一个淡灰 `≡` 标记 |
| 长笔记 / 代码 / 截图 | 卡片或全屏页上点 `⧉` 在独立窗口打开笔记：可自由调整大小、与 IDE 并排，下次沿用上次的尺寸；每篇笔记一个窗口，再点该任务会切到已打开的窗口，暂停时分隔也记在那里。行首输入 ` ``` `（可带语言，如 ` ```js `）再按空格或回车变成代码块：不自动换行、回车沿用缩进、`Tab` 缩进、`Ctrl+Enter` 跳出；从 VS Code 等编辑器粘贴的多行代码自动成为代码块。鼠标移到代码块上可「复制」；小窗里超过 10 行的代码默认折叠，点击展开。单击图片在应用内查看大图：滚轮缩放、拖动、`←` / `→` 切换、双击在适应窗口与原始大小间切换、`Esc` 关闭 |
| 暂停 / 继续 | 暂停时自动展开卡片并插入「⏸ 时间」分隔，顺手记下停在哪（不想写直接点别处，不留痕迹）；继续有笔记的任务时自动展开，接上当时的上下文（可在设置中关闭） |
| 删除 | 行尾 `✕`，再点一次确认 |
| 调整顺序 | 拖动行首 `⠿` 把手，拖到列表空白处即放末尾 |
| 恢复 | 已完成列表里点 `↩` |
| 清空已完成 | 右上角「清空」（二次确认）；筛选某个项目时只清空该项目的已完成任务 |
| 唤起 / 收起 | 全局热键（默认 `Ctrl+K`，可在设置中修改），或单击托盘图标；重复启动程序也会唤起已运行的窗口（单实例） |
| 设置 | 右上角 ⚙：全局热键、关闭按钮行为（最小化到任务栏 / 隐藏到托盘 / 直接退出，默认最小化到任务栏）、暂停 / 继续时展开笔记、浅色/深色/跟随系统、中文/English、字体与字号 |
| 退出 | 取决于「关闭按钮行为」设置；右键托盘图标 →「退出」随时可真正退出 |

- 🕒 **时间留痕**：每条任务记录「创建时间」与「最后修改时间」，完成时记录「完成时间」，都以小字显示在任务下方。
- 🌗 **明暗自适应**：默认跟随系统深浅色模式，也可在设置中手动指定。
- 📄 **纯文本存储**：数据就是两个 todo.txt 风格的文本文件，可以用记事本直接改、用 git 管理历史、用 grep 检索，备份或迁移只需复制整个文件夹。

## 数据格式

文件保存在应用数据目录（界面右下角「数据文件」按钮可一键打开；悬停可看到完整路径）：

- Windows：`%APPDATA%\com.tinytd.app`
- macOS：`~/Library/Application Support/com.tinytd.app`
- Linux：`~/.config/com.tinytd.app`

目录里有：

- `todo.txt` —— 进行中任务，**行序即任务顺序**
- `done.txt` —— 已完成任务（按完成先后追加）
- `settings.json` —— 界面设置（设置面板自动维护）
- `notes/<编号>.md` —— 任务笔记（Markdown，任务行上带 `note:<编号>`），图片存放在 `notes/<编号>/`；删除任务或清空已完成时一并删除

> 需要便携，或想把数据放到其他位置（如网盘、git 仓库）时，设置环境变量 `TINYTD_DATA_DIR` 即可把数据目录指到任意文件夹，例如：
> `$env:TINYTD_DATA_DIR = 'D:\sync\tinytd'`
>
> 在多台电脑之间使用前，请先看「[多台电脑使用](#多台电脑使用)」。

```text
todo.txt:
2026-11-25T16:09 给客户回邮件
2026-11-25T16:12 修复登录页显示 bug mod:2026-11-25T18:02
2026-11-25T16:20 重构登录模块 doing:2026-11-27T10:30 spent:45
2026-11-25T16:30 整理周会纪要 spent:20
2026-11-25T16:40 对接支付回调 spent:130 note:mi2x8k0a
2026-11-26T09:10 订酒店 +日本旅行

done.txt:
x 2026-11-25T20:15 2026-11-25T16:09 写周报 spent:75 mod:2026-11-25T17:40

notes/mi2x8k0a.md:
### ⏸ 2026-11-27 18:40
等张三给 /refund 接口文档，分支 **feat/pay-cb**
![](mi2x8k0a/20261127-184012.png)
```

- 每行开头是创建时间（`YYYY-MM-DDTHH:MM`，本地时间；也兼容纯日期 `YYYY-MM-DD` 的老式 todo.txt 行）
- `mod:` 标记最后修改时间（只在内容真正改过之后出现）
- `doing:` 表示进行中，记录本段开始时间；`spent:` 是之前累计投入的分钟数。有 `doing:` 为进行中，只有 `spent:` 为已暂停
- 完成时把进行中的这一段计入 `spent:`，「已完成」显示的用时不含暂停时间；恢复为待办时两者都清除
- `note:` 关联任务笔记；笔记支持 Markdown 子集（`#` 标题、`-` / `1.` 列表、粗体、斜体、删除线、代码、链接、图片），其他格式粘贴时会被去掉
- 完成行以 `x <完成时间>` 开头，符合 todo.txt 的完成约定
- `+项目` 即 todo.txt 的项目标记：空白分隔、以 `+` 开头且紧跟字母或汉字的词；它就留在任务文字里，列表中改为在标题下方以小字显示
- 唯一注意：任务文本结尾请避免恰好是 `mod:时间`、`doing:时间`、`spent:数字` 或 `note:编号` 形式的字样（会被当作标记解析掉）

### 多台电脑使用

tinyTD 按「一台电脑使用」设计，不内置同步。把数据目录放进网盘或 git 仓库，可以在电脑之间搬运数据，但应用本身不感知同步，请注意：

- **同一时间只在一台电脑上运行 tinyTD**。换电脑前，先在原电脑上退出（托盘图标右键 →「退出」），并等网盘同步完成或 git 推送完成。
- 应用运行期间**不会自动读取外部修改**（网盘同步下来的变化、用记事本做的改动），需要重启应用才能看到。界面显示的还是旧内容时继续操作，可能会改到别的任务上。
- 两台电脑同时改动时，网盘可能生成冲突副本或直接覆盖，tinyTD 不会帮你合并。
- 使用网盘时，请把数据文件夹设为「始终保留在此设备上」。如果文件只是按需下载的占位文件或被网盘锁住，应用可能读取失败，严重时会丢失数据。
- 使用 git 时，todo.txt 一行一个任务，按行合并比较友好，还能保留历史；但 `notes/` 里的图片会让仓库逐渐变大。

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
│  ├─ index.html           主窗口
│  ├─ note.html            独立笔记窗口（note.js）
│  ├─ style.css
│  ├─ i18n.js              中 / 英界面文案
│  ├─ md.js                笔记：Markdown 子集 ⇄ 编辑器 HTML
│  ├─ editor.js            两个窗口共用的笔记编辑器（粘贴、图片、代码块、看图）
│  ├─ note.js
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
3. 数据目录想彻底避开 AppData（便携 / 放到网盘）时，用环境变量重定向，但 WebView2 数据目录仍需按上一条预建：

   ```powershell
   $env:TINYTD_DATA_DIR = 'D:\sync\tinytd'
   ```

## 设计取舍

- **不做**：提醒/通知、优先级、截止日期，也不做「项目管理」（没有项目设置、颜色、嵌套）——在快节奏工作里，你需要的是一个「2 秒记完一件事」的地方，而不是又一个日程管理软件。
- **不做同步**：没有账号、服务器和手机端。它是一台电脑上的工作速记本，数据就是本地文本文件；需要跨电脑时交给网盘或 git，见「[多台电脑使用](#多台电脑使用)」。- **做了**：一键快速输入、拖拽排序、修改留痕、完成列表可回看/可恢复、可选的 todo.txt `+项目` 标记（区分工作 / 生活 / 某次旅行，记任务时不多一步）、纯文本随时可迁移。

License: MIT
