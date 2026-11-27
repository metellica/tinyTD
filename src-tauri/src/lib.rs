//! tinyTD · 基于 todo.txt 的极简桌面待办
//!
//! 数据格式（纯文本，每行一个任务，可直接用任何编辑器修改、可同步、可 grep）：
//!
//! - 待办：`<创建时间> <任务文本> [doing:<本段开始时间>] [spent:<累计分钟>] [mod:<最后修改时间>]`
//! - 已完成：`x <完成时间> <创建时间> <任务文本> [spent:<累计分钟>] [mod:<最后修改时间>]`
//! - 状态：有 `doing:` 为进行中；只有 `spent:` 为已暂停；都没有为普通待办
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
    /// 本段进行中的开始时间；有值即「进行中」
    pub doing: Option<String>,
    /// 之前各段累计的分钟数；没有 doing 但有 spent 即「已暂停」
    pub spent: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoneTask {
    pub id: usize,
    pub text: String,
    pub created: String,
    pub completed: String,
    pub modified: Option<String>,
    /// 实际投入的总分钟数（不含暂停时间）
    pub spent: Option<u32>,
}

type R<T> = Result<T, String>;

// ---------- 时间 ----------

fn now_ts() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string()
}

fn parse_ts(s: &str) -> Option<chrono::NaiveDateTime> {
    use chrono::{NaiveDate, NaiveDateTime};
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
        .ok()
        .or_else(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?.and_hms_opt(0, 0, 0))
}

/// 两个时间之间的整分钟数；无法解析或倒序时为 0
fn minutes_between(from: &str, to: &str) -> u32 {
    match (parse_ts(from), parse_ts(to)) {
        (Some(a), Some(b)) => u32::try_from((b - a).num_minutes()).unwrap_or(0),
        _ => 0,
    }
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

/// 一条待办行的各个字段
///
/// 状态由标记推出：有 `doing:` 为进行中；只有 `spent:` 为已暂停；都没有为普通待办
#[derive(Debug, Default, PartialEq)]
struct ActiveLine {
    text: String,
    created: Option<String>,
    modified: Option<String>,
    /// 本段进行中的开始时间
    doing: Option<String>,
    /// 之前各段累计投入的分钟数
    spent: Option<u32>,
}

/// 解析待办行
fn parse_active_line(line: &str) -> ActiveLine {
    let mut rest = line.trim_end();
    // 行尾的 doing: / spent: / mod: 标记，顺序不限（手工编辑可能产生多个，各取最后一个写入的）
    let (mut modified, mut doing, mut spent) = (None, None, None);
    while let Some((head, tail)) = rest.rsplit_once(' ') {
        let ok = if let Some(v) = tail.strip_prefix("mod:").filter(|v| looks_like_ts(v)) {
            modified.get_or_insert_with(|| v.to_string());
            true
        } else if let Some(v) = tail.strip_prefix("doing:").filter(|v| looks_like_ts(v)) {
            doing.get_or_insert_with(|| v.to_string());
            true
        } else if let Some(n) = tail.strip_prefix("spent:").and_then(|v| v.parse::<u32>().ok()) {
            spent.get_or_insert(n);
            true
        } else {
            false
        };
        if !ok {
            break;
        }
        rest = head.trim_end();
    }
    // 行首的创建时间
    let (text, created) = match rest.split_once(' ') {
        Some((first, other)) if looks_like_ts(first) => {
            (other.to_string(), Some(first.to_string()))
        }
        _ => (rest.to_string(), None),
    };
    ActiveLine { text, created, modified, doing, spent }
}

impl ActiveLine {
    /// 拼装待办行；没有创建时间时补上当前时间
    fn to_line(&self) -> String {
        let created = self.created.clone().unwrap_or_else(now_ts);
        let mut line = format!("{created} {}", self.text);
        if let Some(d) = &self.doing {
            line.push_str(" doing:");
            line.push_str(d);
        }
        if let Some(s) = self.spent {
            line.push_str(&format!(" spent:{s}"));
        }
        if let Some(m) = &self.modified {
            line.push_str(" mod:");
            line.push_str(m);
        }
        line
    }

    /// 结束当前进行段，计入累计时长
    fn fold_doing(&mut self, now: &str) {
        if let Some(start) = self.doing.take() {
            let total = self.spent.unwrap_or(0) + minutes_between(&start, now);
            self.spent = (total > 0).then_some(total);
        }
    }

    /// 切换到 doing / paused；暂停时累计不足 1 分钟视为误触，直接回到普通待办
    fn set_status(&mut self, status: &str, now: &str) -> R<()> {
        match status {
            "doing" => {
                self.doing.get_or_insert_with(|| now.to_string());
            }
            "paused" => self.fold_doing(now),
            _ => return Err("状态无效".into()),
        }
        Ok(())
    }
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
            let l = parse_active_line(line);
            Task {
                id,
                text: l.text,
                created: l.created.unwrap_or_default(),
                modified: l.modified,
                doing: l.doing,
                spent: l.spent,
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
            let mut l = parse_active_line(&orig);
            // 兼容完成时仍带 doing: 的旧记录：把这一段计到完成时间为止
            if let Some(c) = &completed {
                l.fold_doing(c);
            }
            DoneTask {
                id,
                text: l.text,
                created: l.created.unwrap_or_default(),
                completed: completed.unwrap_or_default(),
                modified: l.modified,
                spent: l.spent,
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
    let line = ActiveLine {
        text: text.clone(),
        created: Some(created.clone()),
        ..ActiveLine::default()
    };
    append_line(&state.dir.join(TODO_FILE), &line.to_line())?;
    let id = read_lines(&state.dir.join(TODO_FILE)).len().saturating_sub(1);
    Ok(Task {
        id,
        text,
        created,
        modified: None,
        doing: None,
        spent: None,
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
    let mut l = parse_active_line(&lines[id]);
    l.text = text;
    l.modified = Some(now_ts());
    lines[id] = l.to_line();
    write_lines(&path, &lines)
}

/// 切换进行中 / 已暂停（status 为 "doing" 或 "paused"）；只是状态变化，不更新 mod:
#[tauri::command]
fn set_status(state: State<AppData>, id: usize, status: String) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let mut l = parse_active_line(&lines[id]);
    l.set_status(&status, &now_ts())?;
    lines[id] = l.to_line();
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
    let mut line = lines.remove(id);
    write_lines(&todo, &lines)?;
    let now = now_ts();
    // 进行中的任务：把当前这一段计入累计用时
    let mut l = parse_active_line(&line);
    if l.doing.is_some() {
        l.fold_doing(&now);
        line = l.to_line();
    }
    append_line(&state.dir.join(DONE_FILE), &format!("x {now} {line}"))
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
    let l = parse_active_line(&orig);
    // 恢复后回到普通待办：清掉进行中 / 累计用时
    let restored = ActiveLine {
        text: l.text,
        created: l.created,
        modified: Some(now_ts()),
        ..ActiveLine::default()
    };
    append_line(&state.dir.join(TODO_FILE), &restored.to_line())
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
            set_status,
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
        let l = parse_active_line(line);
        assert_eq!(l.text, "写周报");
        assert_eq!(l.created.as_deref(), Some("2026-11-25T16:09"));
        assert_eq!(l.modified.as_deref(), Some("2026-11-25T18:02"));
        assert_eq!(l.doing, None);
        assert_eq!(l.to_line(), line);
    }

    #[test]
    fn status_markers_roundtrip() {
        let line = "2026-11-25T16:09 写周报 doing:2026-11-27T10:30 spent:45 mod:2026-11-25T18:02";
        let l = parse_active_line(line);
        assert_eq!(l.text, "写周报");
        assert_eq!(l.doing.as_deref(), Some("2026-11-27T10:30"));
        assert_eq!(l.spent, Some(45));
        assert_eq!(l.modified.as_deref(), Some("2026-11-25T18:02"));
        assert_eq!(l.to_line(), line);
        // 手工编辑后标记顺序颠倒也能识别
        let l = parse_active_line("2026-11-25 写周报 mod:2026-11-26 spent:3 doing:2026-11-27T10:30");
        assert_eq!(l.text, "写周报");
        assert_eq!(l.doing.as_deref(), Some("2026-11-27T10:30"));
        assert_eq!(l.spent, Some(3));
        assert_eq!(l.modified.as_deref(), Some("2026-11-26"));
        // 正文中的、或值不合法的标记不算
        let l = parse_active_line("2026-11-25 记录 doing:xxx 的含义 spent:abc");
        assert_eq!(l.text, "记录 doing:xxx 的含义 spent:abc");
        assert_eq!((l.doing, l.spent), (None, None));
    }

    #[test]
    fn status_transitions() {
        let mut l = parse_active_line("2026-11-25T16:09 写周报");
        // 开始
        l.set_status("doing", "2026-11-27T10:00").unwrap();
        assert_eq!(l.doing.as_deref(), Some("2026-11-27T10:00"));
        // 重复开始不会重置开始时间
        l.set_status("doing", "2026-11-27T10:05").unwrap();
        assert_eq!(l.doing.as_deref(), Some("2026-11-27T10:00"));
        // 暂停：计入累计
        l.set_status("paused", "2026-11-27T10:45").unwrap();
        assert_eq!((l.doing.as_deref(), l.spent), (None, Some(45)));
        // 继续 -> 再暂停（跨天）：继续累加
        l.set_status("doing", "2026-11-27T23:50").unwrap();
        l.set_status("paused", "2026-11-28T00:20").unwrap();
        assert_eq!(l.spent, Some(75));
        // 已暂停时再暂停无变化
        l.set_status("paused", "2026-11-28T09:00").unwrap();
        assert_eq!(l.spent, Some(75));
        assert!(l.set_status("bogus", "2026-11-28T09:00").is_err());
    }

    #[test]
    fn quick_pause_reverts_to_todo() {
        let mut l = parse_active_line("2026-11-25T16:09 写周报");
        l.set_status("doing", "2026-11-27T10:00").unwrap();
        l.set_status("paused", "2026-11-27T10:00").unwrap();
        assert_eq!((l.doing.as_deref(), l.spent), (None, None));
        assert_eq!(l.to_line(), "2026-11-25T16:09 写周报");
    }

    #[test]
    fn minutes_between_shapes() {
        assert_eq!(minutes_between("2026-11-27T10:00", "2026-11-27T11:30"), 90);
        assert_eq!(minutes_between("2026-11-27", "2026-11-27T01:00"), 60);
        assert_eq!(minutes_between("2026-11-27T11:00", "2026-11-27T10:00"), 0);
        assert_eq!(minutes_between("garbage", "2026-11-27T10:00"), 0);
    }

    #[test]
    fn plain_and_legacy() {
        let l = parse_active_line("买牛奶");
        assert_eq!(l, ActiveLine { text: "买牛奶".into(), ..ActiveLine::default() });
        // 兼容只有日期的老式 todo.txt 行
        let l = parse_active_line("2026-11-25 买牛奶");
        assert_eq!(l.text, "买牛奶");
        assert_eq!(l.created.as_deref(), Some("2026-11-25"));
    }

    #[test]
    fn text_with_mod_colon_inside() {
        let l = parse_active_line("2026-11-25 修复 mod:xxx 的解析 bug");
        assert_eq!(l.text, "修复 mod:xxx 的解析 bug");
        assert_eq!(l.modified, None);
    }

    #[test]
    fn done_roundtrip() {
        let active = ActiveLine {
            text: "写周报".into(),
            created: Some("2026-11-25T16:09".into()),
            spent: Some(70),
            ..ActiveLine::default()
        };
        let done = format!("x 2026-11-25T20:15 {}", active.to_line());
        let (completed, orig) = split_done_line(&done);
        assert_eq!(completed.as_deref(), Some("2026-11-25T20:15"));
        let l = parse_active_line(&orig);
        assert_eq!(l.text, "写周报");
        assert_eq!(l.created.as_deref(), Some("2026-11-25T16:09"));
        assert_eq!(l.spent, Some(70));
    }

    #[test]
    fn legacy_done_with_doing_counts_until_completion() {
        let (completed, orig) =
            split_done_line("x 2026-11-25T20:15 2026-11-25T16:09 写周报 doing:2026-11-25T19:00");
        let mut l = parse_active_line(&orig);
        l.fold_doing(completed.as_deref().unwrap());
        assert_eq!((l.doing, l.spent), (None, Some(75)));
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
