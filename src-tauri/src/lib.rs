//! tinyTD · 基于 todo.txt 的极简桌面待办
//!
//! 数据格式（纯文本，每行一个任务，可直接用任何编辑器修改、可同步、可 grep）：
//!
//! - 进行中：`<创建时间> <任务文本> [mod:<最后修改时间>]`
//! - 已完成：`x <完成时间> <创建时间> <任务文本> [mod:<最后修改时间>]`
//! - 时间格式：`YYYY-MM-DD` 或 `YYYY-MM-DDTHH:MM`（本地时间），行序即任务顺序

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, Theme, Wry, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_HOTKEY: &str = "Ctrl+K";
const TODO_FILE: &str = "todo.txt";
const DONE_FILE: &str = "done.txt";
const SETTINGS_FILE: &str = "settings.json";

/// 运行时状态：数据目录 + 文件读写锁 + 用户设置
pub struct AppData {
    dir: PathBuf,
    lock: Mutex<()>,
    settings: Mutex<Settings>,
}

/// 托盘菜单项句柄，切换语言时用来更新文字
struct TrayItems {
    show: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

/// 用户设置，保存在数据目录的 settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 关闭按钮行为：`minimize` 最小化到任务栏 | `tray` 隐藏到托盘 | `exit` 退出
    pub close_action: String,
    /// `auto` | `light` | `dark`
    pub theme: String,
    /// `zh` | `en`
    pub lang: String,
    /// 自定义字体族，空 = 默认
    pub font: String,
    pub font_size: u32,
    /// 全局唤起热键，如 `Ctrl+K`、`Ctrl+Alt+T`、`Cmd+Shift+Space`
    pub hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            close_action: "minimize".into(),
            theme: "auto".into(),
            lang: "zh".into(),
            font: String::new(),
            font_size: 13,
            hotkey: DEFAULT_HOTKEY.into(),
        }
    }
}

/// 解析热键字符串；全局热键必须带修饰键，否则会吞掉普通按键
fn parse_hotkey(s: &str) -> Result<Shortcut, String> {
    let sc = Shortcut::from_str(s.trim()).map_err(|e| format!("热键无效：{e}"))?;
    if sc.mods.is_empty() {
        return Err("热键必须包含 Ctrl / Alt / Shift / Cmd 中的至少一个".into());
    }
    Ok(sc)
}

impl Settings {
    /// 把来自前端/手工编辑的值规整到合法范围
    fn sanitized(mut self) -> Self {
        let pick = |v: &str, allowed: &[&str], default: &str| -> String {
            if allowed.contains(&v) { v.to_string() } else { default.to_string() }
        };
        self.close_action = pick(&self.close_action, &["minimize", "tray", "exit"], "minimize");
        self.theme = pick(&self.theme, &["auto", "light", "dark"], "auto");
        self.lang = pick(&self.lang, &["zh", "en"], "zh");
        self.font = self
            .font
            .chars()
            .filter(|c| !matches!(c, ';' | '{' | '}' | '<' | '>' | '\\' | '\n' | '\r'))
            .take(120)
            .collect::<String>()
            .trim()
            .to_string();
        self.font_size = self.font_size.clamp(11, 20);
        if parse_hotkey(&self.hotkey).is_err() {
            self.hotkey = DEFAULT_HOTKEY.into();
        }
        self.hotkey = self.hotkey.trim().to_string();
        self
    }
}

fn load_settings(dir: &Path) -> Settings {
    fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default()
        .sanitized()
}

fn tray_labels(lang: &str, hotkey: &str) -> (String, &'static str) {
    if lang == "en" {
        (format!("Show tinyTD  ({hotkey})"), "Quit")
    } else {
        (format!("显示 tinyTD  ({hotkey})"), "退出")
    }
}

/// 把设置中需要原生层配合的部分（窗口标题栏主题、托盘文字）应用出去
fn apply_native(app: &AppHandle, s: &Settings) {
    if let Some(w) = app.get_webview_window("main") {
        let theme = match s.theme.as_str() {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        };
        let _ = w.set_theme(theme);
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let (show, quit) = tray_labels(&s.lang, &s.hotkey);
        let _ = items.show.set_text(show);
        let _ = items.quit.set_text(quit);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: usize,
    pub text: String,
    pub created: String,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoneTask {
    pub id: usize,
    pub text: String,
    pub created: String,
    pub completed: String,
    pub modified: Option<String>,
}

type R<T> = Result<T, String>;

// ---------- 时间 ----------

fn now_ts() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string()
}

/// 判断是否形如 `YYYY-MM-DD` / `YYYY-MM-DDTHH:MM` / `YYYY-MM-DDTHH:MM:SS`
fn looks_like_ts(s: &str) -> bool {
    fn digits(b: &[u8], r: std::ops::Range<usize>) -> bool {
        b.get(r)
            .map(|x| x.iter().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
    }
    let b = s.as_bytes();
    let date_ok = b.len() >= 10
        && digits(b, 0..4)
        && b[4] == b'-'
        && digits(b, 5..7)
        && b[7] == b'-'
        && digits(b, 8..10);
    if !date_ok {
        return false;
    }
    match b.len() {
        10 => true,
        16 => b[10] == b'T' && digits(b, 11..13) && b[13] == b':' && digits(b, 14..16),
        19 => {
            b[10] == b'T'
                && digits(b, 11..13)
                && b[13] == b':'
                && digits(b, 14..16)
                && b[16] == b':'
                && digits(b, 17..19)
        }
        _ => false,
    }
}

// ---------- 行解析 ----------

/// 解析“进行中”行 -> (文本, 创建时间, 修改时间)
fn parse_active_line(line: &str) -> (String, Option<String>, Option<String>) {
    let mut rest = line.trim_end();
    // 行尾的 mod: 标记（手工编辑可能产生多个，取最后一个写入的）
    let mut modified = None;
    while let Some((head, tail)) = rest.rsplit_once(' ') {
        if let Some(v) = tail.strip_prefix("mod:") {
            if looks_like_ts(v) {
                if modified.is_none() {
                    modified = Some(v.to_string());
                }
                rest = head.trim_end();
                continue;
            }
        }
        break;
    }
    // 行首的创建时间
    let (text, created) = match rest.split_once(' ') {
        Some((first, other)) if looks_like_ts(first) => {
            (other.to_string(), Some(first.to_string()))
        }
        _ => (rest.to_string(), None),
    };
    (text, created, modified)
}

/// 拼装“进行中”行
fn build_active_line(text: &str, created: &str, modified: Option<&str>) -> String {
    let mut line = format!("{created} {text}");
    if let Some(m) = modified {
        line.push_str(" mod:");
        line.push_str(m);
    }
    line
}

/// 拆开“已完成”行 -> (完成时间, 原始的进行中行)
fn split_done_line(line: &str) -> (Option<String>, String) {
    let rest = match line.strip_prefix("x ") {
        Some(r) => r.trim_start(),
        None => line,
    };
    match rest.split_once(' ') {
        Some((first, other)) if looks_like_ts(first) => {
            (Some(first.to_string()), other.to_string())
        }
        _ => (None, rest.to_string()),
    }
}

// ---------- 文件读写 ----------

fn read_lines(path: &Path) -> Vec<String> {
    match fs::read_to_string(path) {
        Ok(s) => s
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn write_lines(path: &Path, lines: &[String]) -> R<()> {
    let mut out = String::new();
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    fs::write(path, out).map_err(|e| format!("写入失败：{e}"))
}

fn append_line(path: &Path, line: &str) -> R<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("打开文件失败：{e}"))?;
    f.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| format!("写入失败：{e}"))
}

fn load_tasks(dir: &Path) -> Vec<Task> {
    read_lines(&dir.join(TODO_FILE))
        .iter()
        .enumerate()
        .map(|(id, line)| {
            let (text, created, modified) = parse_active_line(line);
            Task {
                id,
                text,
                created: created.unwrap_or_default(),
                modified,
            }
        })
        .collect()
}

fn load_done(dir: &Path) -> Vec<DoneTask> {
    read_lines(&dir.join(DONE_FILE))
        .iter()
        .enumerate()
        .map(|(id, line)| {
            let (completed, orig) = split_done_line(line);
            let (text, created, modified) = parse_active_line(&orig);
            DoneTask {
                id,
                text,
                created: created.unwrap_or_default(),
                completed: completed.unwrap_or_default(),
                modified,
            }
        })
        .collect()
}

fn guard<'a, 'r>(state: &'a State<'r, AppData>) -> R<MutexGuard<'a, ()>> {
    state.lock.lock().map_err(|_| "内部锁异常".to_string())
}

// ---------- 命令 ----------

#[tauri::command]
fn list_tasks(state: State<AppData>) -> R<Vec<Task>> {
    let _g = guard(&state)?;
    Ok(load_tasks(&state.dir))
}

#[tauri::command]
fn list_done(state: State<AppData>) -> R<Vec<DoneTask>> {
    let _g = guard(&state)?;
    Ok(load_done(&state.dir))
}

#[tauri::command]
fn add_task(state: State<AppData>, text: String) -> R<Task> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("内容不能为空".into());
    }
    let _g = guard(&state)?;
    let created = now_ts();
    append_line(
        &state.dir.join(TODO_FILE),
        &build_active_line(&text, &created, None),
    )?;
    let id = read_lines(&state.dir.join(TODO_FILE)).len().saturating_sub(1);
    Ok(Task {
        id,
        text,
        created,
        modified: None,
    })
}

#[tauri::command]
fn update_task(state: State<AppData>, id: usize, text: String) -> R<()> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("内容不能为空".into());
    }
    let _g = guard(&state)?;
    let path = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let (_, created, _) = parse_active_line(&lines[id]);
    let created = created.unwrap_or_else(now_ts);
    lines[id] = build_active_line(&text, &created, Some(&now_ts()));
    write_lines(&path, &lines)
}

#[tauri::command]
fn remove_task(state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    lines.remove(id);
    write_lines(&path, &lines)
}

/// 把第 `from` 行移动到「原数组的第 `to` 个插槽」（0..=len，指向移动前的下标空间）
#[tauri::command]
fn move_task(state: State<AppData>, from: usize, to: usize) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&path);
    if from >= lines.len() || to > lines.len() {
        return Err("位置无效".into());
    }
    if from != to && to != from + 1 {
        let item = lines.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        lines.insert(insert_at.min(lines.len()), item);
    }
    write_lines(&path, &lines)
}

#[tauri::command]
fn complete_task(state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let todo = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&todo);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let line = lines.remove(id);
    write_lines(&todo, &lines)?;
    append_line(
        &state.dir.join(DONE_FILE),
        &format!("x {} {}", now_ts(), line),
    )
}

#[tauri::command]
fn restore_task(state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let done = state.dir.join(DONE_FILE);
    let mut lines = read_lines(&done);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let line = lines.remove(id);
    write_lines(&done, &lines)?;
    let (_, orig) = split_done_line(&line);
    let (text, created, _) = parse_active_line(&orig);
    let created = created.unwrap_or_else(now_ts);
    append_line(
        &state.dir.join(TODO_FILE),
        &build_active_line(&text, &created, Some(&now_ts())),
    )
}

#[tauri::command]
fn delete_done(state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(DONE_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    lines.remove(id);
    write_lines(&path, &lines)
}

#[tauri::command]
fn clear_done(state: State<AppData>) -> R<()> {
    let _g = guard(&state)?;
    write_lines(&state.dir.join(DONE_FILE), &[])
}

#[tauri::command]
fn get_settings(state: State<AppData>) -> R<Settings> {
    Ok(state.settings.lock().map_err(|_| "内部锁异常".to_string())?.clone())
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppData>, settings: Settings) -> R<Settings> {
    let new_hotkey = parse_hotkey(&settings.hotkey)?;
    let s = settings.sanitized();
    let old_hotkey = {
        let cur = state.settings.lock().map_err(|_| "内部锁异常".to_string())?;
        parse_hotkey(&cur.hotkey).ok()
    };
    if old_hotkey != Some(new_hotkey) {
        // 先注册新热键：被占用时报错并保留旧热键，设置不落盘
        app.global_shortcut()
            .register(new_hotkey)
            .map_err(|e| format!("热键注册失败（可能已被其他程序占用）：{e}"))?;
        if let Some(old) = old_hotkey {
            let _ = app.global_shortcut().unregister(old);
        }
    }
    let json = serde_json::to_string_pretty(&s).map_err(|e| format!("写入失败：{e}"))?;
    fs::write(state.dir.join(SETTINGS_FILE), json).map_err(|e| format!("写入失败：{e}"))?;
    *state.settings.lock().map_err(|_| "内部锁异常".to_string())? = s.clone();
    apply_native(&app, &s);
    Ok(s)
}

#[tauri::command]
fn data_dir(state: State<AppData>) -> R<String> {
    Ok(state.dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_data_dir(state: State<AppData>) -> R<()> {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(&state.dir).spawn();
    Ok(())
}

// ---------- 入口 ----------

/// WebView2 数据目录：默认的 %LOCALAPPDATA% 在部分受限环境（如终端防护软件）下无法创建，
/// 导致窗口空白且进程挂起。优先放到 TINYTD_DATA_DIR，否则放到 exe 旁边；
/// 用户已设置 WEBVIEW2_USER_DATA_FOLDER 时不覆盖。
#[cfg(windows)]
fn init_webview_dir() {
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some() {
        return;
    }
    let base = match std::env::var_os("TINYTD_DATA_DIR") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf)),
    };
    if let Some(base) = base {
        let dir = base.join(".webview");
        if fs::create_dir_all(&dir).is_ok() {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir);
        }
    }
}

fn dir_writable(dir: &Path) -> bool {
    fs::create_dir_all(dir).is_ok() && {
        let probe = dir.join(".write-test");
        let ok = fs::write(&probe, b"").is_ok();
        let _ = fs::remove_file(&probe);
        ok
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn close_action(app: &AppHandle) -> String {
    app.try_state::<AppData>()
        .and_then(|s| s.settings.lock().ok().map(|g| g.close_action.clone()))
        .unwrap_or_else(|| "minimize".into())
}

/// 收起窗口：`tray` 模式隐藏到托盘，其余模式最小化（保留在任务栏）
fn dismiss_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if close_action(app) == "tray" {
            let _ = w.hide();
        } else {
            let _ = w.minimize();
        }
    }
}

/// 在前台则收起，否则唤起
fn toggle_main(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(w) if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) => {
            dismiss_main(app)
        }
        _ => show_main(app),
    }
}

pub fn run() {
    #[cfg(windows)]
    init_webview_dir();
    tauri::Builder::default()
        // 必须最先注册：重复启动时把已运行的实例唤起，新进程随即退出
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                // 只会注册当前设置里的那一个热键，按下即切换窗口
                .with_handler(|app, _sc, ev| {
                    if ev.state() == ShortcutState::Pressed {
                        toggle_main(app);
                    }
                })
                .build(),
        )
        .on_window_event(|w, e| {
            // 关闭按钮的行为由设置决定；`exit` 时放行关闭，应用随之退出
            if let WindowEvent::CloseRequested { api, .. } = e {
                let app = w.app_handle();
                if close_action(app) != "exit" {
                    api.prevent_close();
                    dismiss_main(app);
                }
            }
        })
        .setup(|app| {
            // 数据目录：默认 AppData；设置 TINYTD_DATA_DIR 可重定向（便携 / 网盘同步场景）
            let dir = match std::env::var_os("TINYTD_DATA_DIR") {
                Some(p) if !p.is_empty() => p.into(),
                _ => {
                    let cfg = app.path().app_config_dir()?;
                    // 用户目录在部分受限环境下不可写，此时回退到 exe 旁边的 data 目录
                    if dir_writable(&cfg) {
                        cfg
                    } else {
                        std::env::current_exe()
                            .ok()
                            .and_then(|e| e.parent().map(|p| p.join("data")))
                            .unwrap_or(cfg)
                    }
                }
            };
            fs::create_dir_all(&dir)?;
            let settings = load_settings(&dir);

            // 热键被其他程序占用时只是不可用，不影响启动
            if let Ok(hotkey) = parse_hotkey(&settings.hotkey) {
                let _ = app.global_shortcut().register(hotkey);
            }

            let (show_label, quit_label) = tray_labels(&settings.lang, &settings.hotkey);
            let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let tray = TrayIconBuilder::new()
                .tooltip("tinyTD")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        show_main(tray.app_handle());
                    }
                });
            let tray = tray.icon(tauri::include_image!("icons/32x32.png")).build(app)?;
            // 持有托盘句柄，避免被提前 drop 导致图标消失
            app.manage(tray);
            app.manage(TrayItems { show, quit });

            apply_native(app.handle(), &settings);
            app.manage(AppData {
                dir,
                lock: Mutex::new(()),
                settings: Mutex::new(settings),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            list_done,
            add_task,
            update_task,
            remove_task,
            move_task,
            complete_task,
            restore_task,
            delete_done,
            clear_done,
            get_settings,
            set_settings,
            data_dir,
            open_data_dir
        ])
        .run(tauri::generate_context!())
        .expect("tinyTD 启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_shapes() {
        assert!(looks_like_ts("2026-11-25"));
        assert!(looks_like_ts("2026-11-25T16:09"));
        assert!(looks_like_ts("2026-11-25T16:09:33"));
        assert!(!looks_like_ts("hello"));
        assert!(!looks_like_ts("2026-11-2"));
        assert!(!looks_like_ts("2026-11-25T16"));
        assert!(!looks_like_ts("2026-11-25T16:09:3"));
        assert!(!looks_like_ts(""));
    }

    #[test]
    fn active_roundtrip() {
        let line = "2026-11-25T16:09 写周报 mod:2026-11-25T18:02";
        let (text, created, modified) = parse_active_line(line);
        assert_eq!(text, "写周报");
        assert_eq!(created.as_deref(), Some("2026-11-25T16:09"));
        assert_eq!(modified.as_deref(), Some("2026-11-25T18:02"));
        assert_eq!(
            build_active_line(&text, created.as_deref().unwrap(), modified.as_deref()),
            line
        );
    }

    #[test]
    fn plain_and_legacy() {
        let (text, created, modified) = parse_active_line("买牛奶");
        assert_eq!((text.as_str(), created, modified), ("买牛奶", None, None));
        // 兼容只有日期的老式 todo.txt 行
        let (text, created, _) = parse_active_line("2026-11-25 买牛奶");
        assert_eq!(text, "买牛奶");
        assert_eq!(created.as_deref(), Some("2026-11-25"));
    }

    #[test]
    fn text_with_mod_colon_inside() {
        let (text, _, modified) = parse_active_line("2026-11-25 修复 mod:xxx 的解析 bug");
        assert_eq!(text, "修复 mod:xxx 的解析 bug");
        assert_eq!(modified, None);
    }

    #[test]
    fn done_roundtrip() {
        let active = build_active_line("写周报", "2026-11-25T16:09", None);
        let done = format!("x 2026-11-25T20:15 {active}");
        let (completed, orig) = split_done_line(&done);
        assert_eq!(completed.as_deref(), Some("2026-11-25T20:15"));
        let (text, created, _) = parse_active_line(&orig);
        assert_eq!(text, "写周报");
        assert_eq!(created.as_deref(), Some("2026-11-25T16:09"));
    }

    #[test]
    fn hotkey_parsing() {
        assert!(parse_hotkey(DEFAULT_HOTKEY).is_ok());
        assert!(parse_hotkey("Ctrl+Alt+T").is_ok());
        assert!(parse_hotkey("Cmd+Shift+Space").is_ok());
        assert!(parse_hotkey("Ctrl+F5").is_ok());
        assert!(parse_hotkey("K").is_err());
        assert!(parse_hotkey("Ctrl+").is_err());
        assert!(parse_hotkey("").is_err());
        let bad = Settings { hotkey: "nope".into(), ..Settings::default() }.sanitized();
        assert_eq!(bad.hotkey, DEFAULT_HOTKEY);
    }

    #[test]
    fn done_without_timestamp() {
        let (completed, orig) = split_done_line("x 写周报");
        assert_eq!(completed, None);
        assert_eq!(orig, "写周报");
    }

    #[test]
    fn settings_sanitized() {
        let s = Settings {
            close_action: "bogus".into(),
            theme: "dark".into(),
            lang: "fr".into(),
            font: " Foo; } <b> ".into(),
            font_size: 99,
            hotkey: " Ctrl+Alt+T ".into(),
        }
        .sanitized();
        assert_eq!(s.close_action, "minimize");
        assert_eq!(s.theme, "dark");
        assert_eq!(s.lang, "zh");
        assert_eq!(s.font, "Foo  b");
        assert_eq!(s.font_size, 20);
        assert_eq!(s.hotkey, "Ctrl+Alt+T");
        // 旧版/缺字段的 settings.json 仍可解析，热键回落到默认
        let d: Settings = serde_json::from_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(d.theme, "light");
        assert_eq!(d.close_action, "minimize");
        assert_eq!(d.hotkey, DEFAULT_HOTKEY);
    }

    #[test]
    fn not_done_marker() {
        let (completed, orig) = split_done_line("x-box 买牛奶");
        assert_eq!(completed, None);
        assert_eq!(orig, "x-box 买牛奶");
    }
}
