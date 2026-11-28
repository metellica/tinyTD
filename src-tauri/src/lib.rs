//! tinyTD · 基于 todo.txt 的极简桌面待办
//!
//! 数据格式（纯文本，每行一个任务，可直接用任何编辑器修改、可同步、可 grep）：
//!
//! - 待办：`<创建时间> <任务文本> [doing:<本段开始时间>] [spent:<累计分钟>] [note:<笔记编号>] [mod:<最后修改时间>]`
//! - 已完成：`x <完成时间> <创建时间> <任务文本> [spent:<累计分钟>] [note:<笔记编号>] [mod:<最后修改时间>]`
//! - 状态：有 `doing:` 为进行中；只有 `spent:` 为已暂停；都没有为普通待办
//! - 时间格式：`YYYY-MM-DD` 或 `YYYY-MM-DDTHH:MM`（本地时间），行序即任务顺序
//! - 笔记：`notes/<编号>.md`（Markdown），其中的图片存放在 `notes/<编号>/`

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_HOTKEY: &str = "Ctrl+K";
const TODO_FILE: &str = "todo.txt";
const DONE_FILE: &str = "done.txt";
const SETTINGS_FILE: &str = "settings.json";
const NOTES_DIR: &str = "notes";
/// 笔记 URI 协议：前端通过 `convertFileSrc("<编号>/<文件>", "tdnote")` 显示笔记里的图片
const NOTE_SCHEME: &str = "tdnote";
const NOTE_IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
const NOTE_IMAGE_MAX: usize = 20 * 1024 * 1024;

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
    /// 暂停（及隔天继续）时自动打开任务笔记
    pub auto_note: bool,
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
            auto_note: true,
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

fn window_theme(s: &Settings) -> Option<Theme> {
    match s.theme.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

/// 把设置中需要原生层配合的部分（窗口标题栏主题、托盘文字）应用出去
fn apply_native(app: &AppHandle, s: &Settings) {
    for w in app.webview_windows().values() {
        let _ = w.set_theme(window_theme(s));
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
    /// 笔记编号；第一次写笔记时才分配
    pub note: Option<String>,
    /// 笔记正文非空（主界面据此显示小标记）
    pub has_note: bool,
    /// 正文里的 `+项目` 标记（不含 `+`）
    pub projects: Vec<String>,
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
    pub note: Option<String>,
    pub has_note: bool,
    pub projects: Vec<String>,
}

/// todo.txt 的项目标记：空白分隔、以 `+` 开头、紧跟一个字母（含中文）的词，如 `+工作`、`+japan2026`
/// `+1`、`+5%`、`C++` 这类不算；重复的只取一次，保持出现顺序
fn projects_of(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let Some(p) = word.strip_prefix('+') else { continue };
        if p.chars().next().is_some_and(char::is_alphabetic) && !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    }
    out
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
    /// 笔记编号
    note: Option<String>,
}

/// 笔记编号：4~16 位小写字母或数字（也用作文件名，必须严格校验）
fn valid_note_key(s: &str) -> bool {
    (4..=16).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// 解析待办行
fn parse_active_line(line: &str) -> ActiveLine {
    let mut rest = line.trim_end();
    // 行尾的 doing: / spent: / note: / mod: 标记，顺序不限（手工编辑可能产生多个，各取最后一个写入的）
    let (mut modified, mut doing, mut spent, mut note) = (None, None, None, None);
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
        } else if let Some(v) = tail.strip_prefix("note:").filter(|v| valid_note_key(v)) {
            note.get_or_insert_with(|| v.to_string());
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
    ActiveLine { text, created, modified, doing, spent, note }
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
        if let Some(n) = &self.note {
            line.push_str(" note:");
            line.push_str(n);
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
                projects: projects_of(&l.text),
                text: l.text,
                created: l.created.unwrap_or_default(),
                modified: l.modified,
                doing: l.doing,
                spent: l.spent,
                has_note: has_note(dir, &l.note),
                note: l.note,
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
                projects: projects_of(&l.text),
                text: l.text,
                created: l.created.unwrap_or_default(),
                completed: completed.unwrap_or_default(),
                modified: l.modified,
                spent: l.spent,
                has_note: has_note(dir, &l.note),
                note: l.note,
            }
        })
        .collect()
}

// ---------- 笔记 ----------

fn notes_dir(dir: &Path) -> PathBuf {
    dir.join(NOTES_DIR)
}

fn has_note(dir: &Path, key: &Option<String>) -> bool {
    key.as_deref()
        .is_some_and(|k| notes_dir(dir).join(format!("{k}.md")).is_file())
}

/// 生成新的笔记编号：当前毫秒数的 36 进制，已被占用时顺延
fn new_note_key(dir: &Path) -> String {
    let mut n = chrono::Local::now().timestamp_millis().max(0) as u64;
    loop {
        let mut key = String::new();
        let mut v = n;
        loop {
            let d = (v % 36) as u32;
            key.insert(0, char::from_digit(d, 36).unwrap_or('0'));
            v /= 36;
            if v == 0 {
                break;
            }
        }
        while key.len() < 4 {
            key.insert(0, '0');
        }
        let nd = notes_dir(dir);
        if !nd.join(format!("{key}.md")).exists() && !nd.join(&key).exists() {
            return key;
        }
        n += 1;
    }
}

fn check_note_key(key: &str) -> R<()> {
    if valid_note_key(key) {
        Ok(())
    } else {
        Err("笔记编号无效".into())
    }
}

/// 删除笔记正文与图片目录（不存在时忽略）
fn remove_note(dir: &Path, key: &str) {
    if !valid_note_key(key) {
        return;
    }
    let nd = notes_dir(dir);
    let _ = fs::remove_file(nd.join(format!("{key}.md")));
    let _ = fs::remove_dir_all(nd.join(key));
}

/// 笔记是否仍归某个待办 / 已完成任务所有（任务删掉后不再接受写入，免得留下孤儿文件）
fn note_referenced(dir: &Path, key: &str) -> bool {
    let has = |file: &str, done: bool| {
        read_lines(&dir.join(file))
            .iter()
            .any(|l| note_of_line(l, done).as_deref() == Some(key))
    };
    has(TODO_FILE, false) || has(DONE_FILE, true)
}

fn note_window_label(key: &str) -> String {
    format!("note-{key}")
}

/// 笔记随任务被删除：通知它的独立窗口关闭
fn notify_note_deleted(app: &AppHandle, key: &str) {
    let label = note_window_label(key);
    if app.get_webview_window(&label).is_some() {
        let _ = app.emit_to(label.as_str(), "note-deleted", key);
    }
}

/// 从一行待办（`done = false`）或已完成（`done = true`）里取出笔记编号
fn note_of_line(line: &str, done: bool) -> Option<String> {
    if done {
        parse_active_line(&split_done_line(line).1).note
    } else {
        parse_active_line(line).note
    }
}

/// 删掉笔记图片目录里正文不再引用的图片
fn prune_note_images(dir: &Path, key: &str, content: &str) {
    let img_dir = notes_dir(dir).join(key);
    let Ok(entries) = fs::read_dir(&img_dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !content.contains(&format!("{key}/{name}")) {
            let _ = fs::remove_file(e.path());
        }
    }
    let _ = fs::remove_dir(&img_dir); // 只在已空时成功
}

/// 图片文件名只允许 `[0-9a-z-]+.<扩展名>`，避免路径穿越
fn valid_image_name(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && stem.len() <= 40
                && stem.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && NOTE_IMAGE_EXTS.contains(&ext)
        }
        None => false,
    }
}

fn percent_decode(s: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `tdnote://localhost/<编号>/<文件>` -> 笔记图片的绝对路径
fn note_image_path(dir: &Path, uri_path: &str) -> Option<PathBuf> {
    let p = percent_decode(uri_path.trim_start_matches('/'));
    let (key, name) = p.split_once('/')?;
    (valid_note_key(key) && valid_image_name(name)).then(|| notes_dir(dir).join(key).join(name))
}

fn image_mime(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, e)| e) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
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
        projects: projects_of(&text),
        text,
        created,
        modified: None,
        doing: None,
        spent: None,
        note: None,
        has_note: false,
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
fn remove_task(app: AppHandle, state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(TODO_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let line = lines.remove(id);
    write_lines(&path, &lines)?;
    if let Some(key) = note_of_line(&line, false) {
        remove_note(&state.dir, &key);
        notify_note_deleted(&app, &key);
    }
    Ok(())
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
    // 恢复后回到普通待办：清掉进行中 / 累计用时，笔记保留
    let restored = ActiveLine {
        text: l.text,
        created: l.created,
        modified: Some(now_ts()),
        note: l.note,
        ..ActiveLine::default()
    };
    append_line(&state.dir.join(TODO_FILE), &restored.to_line())
}

#[tauri::command]
fn delete_done(app: AppHandle, state: State<AppData>, id: usize) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(DONE_FILE);
    let mut lines = read_lines(&path);
    if id >= lines.len() {
        return Err("任务不存在，可能刚被修改，请刷新后重试".into());
    }
    let line = lines.remove(id);
    write_lines(&path, &lines)?;
    if let Some(key) = note_of_line(&line, true) {
        remove_note(&state.dir, &key);
        notify_note_deleted(&app, &key);
    }
    Ok(())
}

/// 清空已完成；给了 `project` 时只清掉带该项目标记的
#[tauri::command]
fn clear_done(app: AppHandle, state: State<AppData>, project: Option<String>) -> R<()> {
    let _g = guard(&state)?;
    let path = state.dir.join(DONE_FILE);
    let (removed, kept) = split_by_project(read_lines(&path), project.as_deref());
    write_lines(&path, &kept)?;
    for key in removed.iter().filter_map(|l| note_of_line(l, true)) {
        remove_note(&state.dir, &key);
        notify_note_deleted(&app, &key);
    }
    Ok(())
}

/// 把已完成行分成 (要清掉的, 保留的)；`project` 为空时全部清掉
fn split_by_project(lines: Vec<String>, project: Option<&str>) -> (Vec<String>, Vec<String>) {
    match project {
        None => (lines, Vec::new()),
        Some(p) => lines.into_iter().partition(|line| {
            let text = parse_active_line(&split_done_line(line).1).text;
            projects_of(&text).iter().any(|x| x == p)
        }),
    }
}

/// 在行列表里定位任务：先看 `id` 这一行，对不上（行序已变）再按创建时间 + 文本查找
fn locate_line(lines: &[String], done: bool, id: usize, created: &str, text: &str) -> Option<usize> {
    let matches = |line: &String| {
        let l = if done { parse_active_line(&split_done_line(line).1) } else { parse_active_line(line) };
        l.text == text && l.created.as_deref().unwrap_or("") == created
    };
    if lines.get(id).is_some_and(matches) {
        return Some(id);
    }
    lines.iter().position(matches)
}

/// 取得任务的笔记编号；没有时分配一个并写回该行（不算修改，不更新 mod:）
/// `scope` 为 `todo` 或 `done`；`created` + `text` 用来确认是同一个任务
#[tauri::command]
fn note_key(
    state: State<AppData>,
    scope: String,
    id: usize,
    created: String,
    text: String,
) -> R<String> {
    let _g = guard(&state)?;
    let done = match scope.as_str() {
        "todo" => false,
        "done" => true,
        _ => return Err("参数无效".into()),
    };
    let path = state.dir.join(if done { DONE_FILE } else { TODO_FILE });
    let mut lines = read_lines(&path);
    let id = locate_line(&lines, done, id, &created, &text)
        .ok_or("任务不存在，可能刚被修改，请刷新后重试")?;
    if let Some(key) = note_of_line(&lines[id], done) {
        return Ok(key);
    }
    let key = new_note_key(&state.dir);
    lines[id] = if done {
        let (completed, orig) = split_done_line(&lines[id]);
        let mut l = parse_active_line(&orig);
        l.note = Some(key.clone());
        match completed {
            Some(c) => format!("x {c} {}", l.to_line()),
            None => format!("x {}", l.to_line()),
        }
    } else {
        let mut l = parse_active_line(&lines[id]);
        l.note = Some(key.clone());
        l.to_line()
    };
    write_lines(&path, &lines)?;
    Ok(key)
}

#[tauri::command]
fn read_note(state: State<AppData>, key: String) -> R<String> {
    check_note_key(&key)?;
    let _g = guard(&state)?;
    Ok(fs::read_to_string(notes_dir(&state.dir).join(format!("{key}.md"))).unwrap_or_default())
}

/// 保存笔记正文；内容为空则删除文件。`prune` 时顺带清理不再引用的图片（关闭笔记时）。
/// 独立笔记窗口保存后若「有没有笔记」变了，通知主窗口刷新列表上的标记
#[tauri::command]
fn write_note(
    window: WebviewWindow,
    state: State<AppData>,
    key: String,
    content: String,
    prune: bool,
) -> R<()> {
    check_note_key(&key)?;
    let _g = guard(&state)?;
    if !note_referenced(&state.dir, &key) {
        return Err("笔记所属的任务已被删除".into());
    }
    let nd = notes_dir(&state.dir);
    let file = nd.join(format!("{key}.md"));
    let existed = file.is_file();
    if content.trim().is_empty() {
        let _ = fs::remove_file(&file);
    } else {
        fs::create_dir_all(&nd).map_err(|e| format!("写入失败：{e}"))?;
        let mut body = content.trim_end().to_string();
        body.push('\n');
        fs::write(&file, body).map_err(|e| format!("写入失败：{e}"))?;
    }
    if prune {
        prune_note_images(&state.dir, &key, &content);
    }
    if window.label() != "main" && existed != file.is_file() {
        let _ = window.emit_to("main", "note-saved", &key);
    }
    Ok(())
}

/// 保存粘贴/拖入的图片。请求体为图片原始字节，
/// 请求头 `x-note-key` 为笔记编号、`x-ext` 为扩展名；返回 `<编号>/<文件名>`
#[tauri::command]
fn save_note_image(state: State<AppData>, request: tauri::ipc::Request) -> R<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("参数无效".into());
    };
    let header = |k: &str| {
        request
            .headers()
            .get(k)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase()
    };
    let key = header("x-note-key");
    check_note_key(&key)?;
    let ext = match header("x-ext").as_str() {
        "jpeg" => "jpg".to_string(),
        e if NOTE_IMAGE_EXTS.contains(&e) => e.to_string(),
        _ => return Err("不支持的图片格式".into()),
    };
    if bytes.is_empty() || bytes.len() > NOTE_IMAGE_MAX {
        return Err("图片过大（上限 20 MB）".into());
    }
    let _g = guard(&state)?;
    if !note_referenced(&state.dir, &key) {
        return Err("笔记所属的任务已被删除".into());
    }
    let img_dir = notes_dir(&state.dir).join(&key);
    fs::create_dir_all(&img_dir).map_err(|e| format!("写入失败：{e}"))?;
    let stem = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut name = format!("{stem}.{ext}");
    let mut n = 2;
    while img_dir.join(&name).exists() {
        name = format!("{stem}-{n}.{ext}");
        n += 1;
    }
    fs::write(img_dir.join(&name), bytes).map_err(|e| format!("写入失败：{e}"))?;
    Ok(format!("{key}/{name}"))
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
    // 独立笔记窗口跟着换主题 / 字体 / 语言
    let _ = app.emit("settings-changed", &s);
    Ok(s)
}

#[tauri::command]
fn data_dir(state: State<AppData>) -> R<String> {
    Ok(state.dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_data_dir(state: State<AppData>) -> R<()> {
    spawn_opener(&state.dir.to_string_lossy());
    Ok(())
}

/// 用系统默认浏览器打开笔记里的链接（仅 http/https/mailto）
#[tauri::command]
fn open_link(url: String) -> R<()> {
    let lower = url.to_ascii_lowercase();
    let ok = ["http://", "https://", "mailto:"].iter().any(|p| lower.starts_with(p))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"');
    if !ok {
        return Err("参数无效".into());
    }
    spawn_opener(&url);
    Ok(())
}

/// 用系统默认看图程序打开笔记图片（`<编号>/<文件名>`）
#[tauri::command]
fn open_note_image(state: State<AppData>, rel: String) -> R<()> {
    let path = note_image_path(&state.dir, &rel).ok_or("参数无效")?;
    spawn_opener(&path.to_string_lossy());
    Ok(())
}

/// 在独立窗口打开笔记（每篇笔记一个窗口，已打开则切过去）。
/// 创建窗口须在异步命令里进行，否则 Windows 上会卡死
#[tauri::command]
async fn open_note_window(
    app: AppHandle,
    state: State<'_, AppData>,
    key: String,
    title: String,
    width: f64,
    height: f64,
) -> R<()> {
    check_note_key(&key)?;
    if focus_note_window(app.clone(), key.clone(), false) {
        return Ok(());
    }
    let theme = state
        .settings
        .lock()
        .map(|s| window_theme(&s))
        .map_err(|_| "内部锁异常".to_string())?;
    let title: String = title.chars().filter(|c| !c.is_control()).take(80).collect();
    let init = format!(
        "window.__TINYTD_NOTE__ = {};",
        serde_json::json!({ "key": key, "title": title })
    );
    let clamp = |v: f64, min: f64, max: f64| if v.is_finite() { v.clamp(min, max) } else { min };
    WebviewWindowBuilder::new(&app, note_window_label(&key), WebviewUrl::App("note.html".into()))
        .title(format!("{title} · tinyTD"))
        .inner_size(clamp(width, 420.0, 4000.0), clamp(height, 360.0, 3000.0))
        .min_inner_size(360.0, 300.0)
        .theme(theme)
        .disable_drag_drop_handler()
        .initialization_script(&init)
        .build()
        .map_err(|e| format!("打开窗口失败：{e}"))?;
    Ok(())
}

/// 笔记已在独立窗口打开时切过去并返回 true；`stamp` 时让它在顶部插入暂停分隔
#[tauri::command]
fn focus_note_window(app: AppHandle, key: String, stamp: bool) -> bool {
    let label = note_window_label(&key);
    let Some(w) = app.get_webview_window(&label) else {
        return false;
    };
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
    if stamp {
        let _ = app.emit_to(label.as_str(), "note-stamp", &key);
    }
    true
}

fn spawn_opener(target: &str) {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(target).spawn();
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
        .register_uri_scheme_protocol(NOTE_SCHEME, |ctx, req| {
            use tauri::http::{header::CONTENT_TYPE, Response, StatusCode};
            let not_found = || {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap()
            };
            let Some(state) = ctx.app_handle().try_state::<AppData>() else {
                return not_found();
            };
            let Some(path) = note_image_path(&state.dir, req.uri().path()) else {
                return not_found();
            };
            match fs::read(&path) {
                Ok(bytes) => Response::builder()
                    .header(CONTENT_TYPE, image_mime(&path.to_string_lossy()))
                    .body(bytes)
                    .unwrap_or_else(|_| not_found()),
                Err(_) => not_found(),
            }
        })
        .on_window_event(|w, e| {
            // 主窗口关闭按钮的行为由设置决定；`exit` 时连同独立笔记窗口一起退出
            if let WindowEvent::CloseRequested { api, .. } = e {
                if w.label() != "main" {
                    return;
                }
                let app = w.app_handle();
                api.prevent_close();
                if close_action(app) == "exit" {
                    app.exit(0);
                } else {
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
            note_key,
            read_note,
            write_note,
            save_note_image,
            get_settings,
            set_settings,
            data_dir,
            open_data_dir,
            open_link,
            open_note_image,
            open_note_window,
            focus_note_window
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
    fn project_markers() {
        assert_eq!(projects_of("订酒店 +日本旅行 +work +日本旅行"), ["日本旅行", "work"]);
        assert_eq!(projects_of("+生活 买牛奶"), ["生活"]);
        // 数字、符号、词中间的 + 都不算
        assert!(projects_of("学 C++ 加班 +1 涨幅 +5% a+b + ++x").is_empty());
        // 项目标记是正文的一部分，行尾标记照常解析
        let l = parse_active_line("2026-11-25T16:09 订机票 +旅行 spent:5");
        assert_eq!((l.text.as_str(), l.spent), ("订机票 +旅行", Some(5)));
    }

    #[test]
    fn clear_done_by_project() {
        let lines = vec![
            "x 2026-11-26T10:00 2026-11-25T16:09 订机票 +旅行".to_string(),
            "x 2026-11-26T11:00 2026-11-25T16:10 写周报 +工作 note:abcd1234".to_string(),
            "x 2026-11-26T12:00 2026-11-25T16:11 +旅行2 换汇".to_string(),
        ];
        let (removed, kept) = split_by_project(lines.clone(), Some("旅行"));
        assert_eq!(removed, [lines[0].clone()]);
        assert_eq!(kept, [lines[1].clone(), lines[2].clone()]);
        let (removed, kept) = split_by_project(lines.clone(), None);
        assert_eq!((removed.len(), kept.len()), (3, 0));
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
            auto_note: false,
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
        assert!(d.auto_note);
    }

    #[test]
    fn note_marker_roundtrip() {
        let line = "2026-11-25T16:09 对接支付回调 spent:130 note:mi2x8k0a mod:2026-11-26T09:00";
        let l = parse_active_line(line);
        assert_eq!(l.text, "对接支付回调");
        assert_eq!(l.note.as_deref(), Some("mi2x8k0a"));
        assert_eq!(l.spent, Some(130));
        assert_eq!(l.to_line(), line);
        // 编号不合法时不算标记，留在正文里
        let l = parse_active_line("2026-11-25 看 note:../x 和 note:AB");
        assert_eq!(l.text, "看 note:../x 和 note:AB");
        assert_eq!(l.note, None);
        // 已完成行
        let done = format!("x 2026-11-27T10:00 {line}");
        assert_eq!(note_of_line(&done, true).as_deref(), Some("mi2x8k0a"));
        assert_eq!(note_of_line(line, false).as_deref(), Some("mi2x8k0a"));
    }

    #[test]
    fn note_image_paths_are_confined() {
        let dir = Path::new("data");
        let ok = note_image_path(dir, "/mi2x8k0a%2F20261127-184012.png").unwrap();
        assert_eq!(ok, dir.join("notes").join("mi2x8k0a").join("20261127-184012.png"));
        assert!(note_image_path(dir, "/mi2x8k0a/20261127-184012-2.jpg").is_some());
        for bad in [
            "/mi2x8k0a%2F..%2F..%2Fsettings.json",
            "/..%2Ftodo.txt",
            "/mi2x8k0a/a.exe",
            "/mi2x8k0a/sub/a.png",
            "/MI2X8K0A/a.png",
            "/mi2x8k0a",
            "/",
        ] {
            assert!(note_image_path(dir, bad).is_none(), "{bad}");
        }
        assert_eq!(percent_decode("a%2Fb%zz%2"), "a/b%zz%2");
    }

    #[test]
    fn note_keys_are_unique_and_valid() {
        let dir = std::env::temp_dir().join(format!("tinytd-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(notes_dir(&dir)).unwrap();
        let a = new_note_key(&dir);
        assert!(valid_note_key(&a), "{a}");
        fs::write(notes_dir(&dir).join(format!("{a}.md")), "x").unwrap();
        let b = new_note_key(&dir);
        assert!(valid_note_key(&b) && a != b);

        // 清理不再引用的图片，引用中的保留
        let img = notes_dir(&dir).join(&a);
        fs::create_dir_all(&img).unwrap();
        fs::write(img.join("1.png"), "").unwrap();
        fs::write(img.join("2.png"), "").unwrap();
        prune_note_images(&dir, &a, &format!("![]({a}/1.png)"));
        assert!(img.join("1.png").exists() && !img.join("2.png").exists());
        prune_note_images(&dir, &a, "");
        assert!(!img.exists());
        remove_note(&dir, &a);
        assert!(!notes_dir(&dir).join(format!("{a}.md")).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn note_reference_follows_task_lines() {
        let dir = std::env::temp_dir().join(format!("tinytd-ref-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(TODO_FILE), "2026-11-25T10:00 写周报 note:abcd\n").unwrap();
        fs::write(dir.join(DONE_FILE), "x 2026-11-27T10:00 2026-11-25T09:00 对接 note:ef01\n").unwrap();
        assert!(note_referenced(&dir, "abcd"));
        assert!(note_referenced(&dir, "ef01"));
        // 任务删掉后，笔记不再算有主
        fs::write(dir.join(TODO_FILE), "").unwrap();
        assert!(!note_referenced(&dir, "abcd"));
        assert!(!note_referenced(&dir, "zzzz"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn locate_line_by_identity() {
        let lines: Vec<String> = vec![
            "2026-11-25T10:00 写周报".into(),
            "2026-11-25T11:00 对接支付回调 spent:5".into(),
        ];
        assert_eq!(locate_line(&lines, false, 1, "2026-11-25T11:00", "对接支付回调"), Some(1));
        // 行序变了：按创建时间 + 文本找回
        assert_eq!(locate_line(&lines, false, 0, "2026-11-25T11:00", "对接支付回调"), Some(1));
        assert_eq!(locate_line(&lines, false, 0, "2026-11-25T11:00", "别的任务"), None);
        let done: Vec<String> = vec!["x 2026-11-27T10:00 2026-11-25T10:00 写周报 spent:3".into()];
        assert_eq!(locate_line(&done, true, 5, "2026-11-25T10:00", "写周报"), Some(0));
    }

    #[test]
    fn not_done_marker() {
        let (completed, orig) = split_done_line("x-box 买牛奶");
        assert_eq!(completed, None);
        assert_eq!(orig, "x-box 买牛奶");
    }
}
