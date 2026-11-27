// tinyTD 前端 · 无框架原生 JS
const invoke = window.__TAURI__?.core?.invoke;

const state = {
  tab: 'todo', // 'todo' | 'done'
  tasks: [],
  done: [],
  editingId: -1,
  confirmKey: '',
  confirmTimer: 0,
};

let dragFrom = -1;

const $ = (id) => document.getElementById(id);
const els = {
  tabTodo: $('tab-todo'),
  tabDone: $('tab-done'),
  countTodo: $('count-todo'),
  countDone: $('count-done'),
  clear: $('btn-clear'),
  add: $('input-add'),
  list: $('list'),
  stats: $('stats'),
  dir: $('btn-dir'),
  toast: $('toast'),
  settingsBtn: $('btn-settings'),
  settings: $('settings'),
  settingsClose: $('btn-settings-close'),
  setClose: $('set-close'),
  setTheme: $('set-theme'),
  setLang: $('set-lang'),
  setFont: $('set-font'),
  fieldFontCustom: $('field-font-custom'),
  setFontCustom: $('set-font-custom'),
  setFontSize: $('set-font-size'),
  fontSizeVal: $('font-size-val'),
  setHotkey: $('set-hotkey'),
  hotkeyReset: $('btn-hotkey-reset'),
};

/* ---------- 工具 ---------- */

const ICONS = {
  grip: '<circle cx="9" cy="6" r="1.6"/><circle cx="15" cy="6" r="1.6"/><circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/><circle cx="9" cy="18" r="1.6"/><circle cx="15" cy="18" r="1.6"/>',
  x: '<path d="M6 6l12 12M18 6L6 18"/>',
  undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  inbox: '<path d="M3 13h5l1.5 3h5L16 13h5"/><path d="M5.5 5h13L21 13v5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18v-5z"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>',
};

function icon(name) {
  const filled = name === 'grip';
  const attrs = filled
    ? 'fill="currentColor" stroke="none"'
    : 'fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"';
  const tpl = document.createElement('template');
  tpl.innerHTML = `<svg class="i" viewBox="0 0 24 24" ${attrs}>${ICONS[name]}</svg>`;
  return tpl.content.firstChild;
}

function emptyEl(iconName, msg) {
  const empty = document.createElement('div');
  empty.className = 'empty';
  const span = document.createElement('span');
  span.textContent = msg;
  empty.append(icon(iconName), span);
  return empty;
}

function fmt(ts) {
  if (!ts) return '';
  const [d, t] = ts.split('T');
  const [y, m, day] = d.split('-').map(Number);
  if (!y || !m || !day) return ts;
  const now = new Date();
  const sameDate = (a) => a.getFullYear() === y && a.getMonth() + 1 === m && a.getDate() === day;
  let label;
  if (sameDate(now)) {
    label = tr('today');
  } else {
    const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
    label = sameDate(yesterday)
      ? tr('yesterday')
      : y === now.getFullYear()
        ? `${m}-${String(day).padStart(2, '0')}`
        : `${y}-${String(m).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  }
  return t ? `${label} ${t}` : label;
}

function metaLine(t, withCompleted) {
  const parts = [];
  if (withCompleted && t.completed) parts.push(`${tr('completedAt')} ${fmt(t.completed)}`);
  if (t.created) parts.push(`${tr('createdAt')} ${fmt(t.created)}`);
  if (t.modified) parts.push(`${tr('modifiedAt')} ${fmt(t.modified)}`);
  return parts.join(' · ');
}

let toastTimer = 0;
function toast(msg, isErr = false) {
  els.toast.textContent = msg;
  els.toast.classList.toggle('err', isErr);
  els.toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (els.toast.hidden = true), 2400);
}

async function refresh() {
  const [tasks, done] = await Promise.all([invoke('list_tasks'), invoke('list_done')]);
  state.tasks = tasks;
  state.done = done;
  render();
}

async function tryRun(fn, okMsg) {
  try {
    const r = await fn();
    await refresh();
    if (okMsg) toast(okMsg);
    return r;
  } catch (e) {
    toast(tErr(e), true);
    return null;
  }
}

/* ---------- 渲染 ---------- */

function render() {
  els.countTodo.textContent = state.tasks.length;
  els.countDone.textContent = state.done.length;
  els.clear.hidden = state.tab !== 'done';
  els.stats.textContent = tr('stats', state.tasks.length, state.done.length);
  els.list.replaceChildren();
  const items = state.tab === 'todo' ? state.tasks : state.done;
  if (items.length === 0) {
    els.list.appendChild(state.tab === 'todo'
      ? emptyEl('sun', tr('emptyTodo'))
      : emptyEl('inbox', tr('emptyDone')));
    return;
  }
  for (const t of items) {
    els.list.appendChild(state.tab === 'todo' ? renderTask(t) : renderDone(t));
  }
}

function actionBtn(title, iconName) {
  const btn = document.createElement('button');
  btn.className = 'icon-btn act';
  btn.title = title;
  btn.appendChild(icon(iconName));
  return btn;
}

function deleteBtn(scope, id) {
  const btn = actionBtn(tr('del'), 'x');
  btn.classList.add('danger');
  const key = `del:${scope}:${id}`;
  btn.addEventListener('click', () => {
    if (state.confirmKey === key) {
      clearTimeout(state.confirmTimer);
      state.confirmKey = '';
      const run = scope === 'todo'
        ? () => invoke('remove_task', { id })
        : () => invoke('delete_done', { id });
      tryRun(run, tr('deleted'));
    } else {
      state.confirmKey = key;
      clearTimeout(state.confirmTimer);
      btn.textContent = tr('confirm');
      btn.classList.add('confirm');
      state.confirmTimer = setTimeout(() => {
        state.confirmKey = '';
        btn.replaceChildren(icon('x'));
        btn.classList.remove('confirm');
      }, 2200);
    }
  });
  return btn;
}

function metaEl(t, withCompleted) {
  const meta = document.createElement('div');
  meta.className = 'meta';
  meta.textContent = metaLine(t, withCompleted);
  return meta;
}

function renderTask(t) {
  const row = document.createElement('div');
  row.className = 'row';
  row.dataset.id = t.id;

  const handle = document.createElement('span');
  handle.className = 'handle';
  handle.appendChild(icon('grip'));
  handle.title = tr('dragTitle');
  handle.addEventListener('mousedown', () => (row.draggable = true));
  handle.addEventListener('mouseup', () => (row.draggable = false));
  row.appendChild(handle);

  const cb = document.createElement('input');
  cb.type = 'checkbox';
  cb.className = 'check';
  cb.title = tr('markDone');
  cb.addEventListener('change', () => {
    row.classList.add('completing');
    setTimeout(() => tryRun(() => invoke('complete_task', { id: t.id })), 160);
  });
  row.appendChild(cb);

  const body = document.createElement('div');
  body.className = 'body';
  const text = document.createElement('div');
  text.className = 'text';
  text.textContent = t.text;
  text.title = tr('clickEdit');
  text.addEventListener('click', () => startEdit(row, t));
  body.appendChild(text);
  body.appendChild(metaEl(t, false));
  row.appendChild(body);

  row.appendChild(deleteBtn('todo', t.id));

  row.addEventListener('dragstart', (e) => {
    dragFrom = t.id;
    row.classList.add('dragging');
    e.dataTransfer.effectAllowed = 'move';
    try { e.dataTransfer.setData('text/plain', String(t.id)); } catch {}
  });
  row.addEventListener('dragend', () => {
    row.draggable = false;
    row.classList.remove('dragging');
    clearMarks();
    dragFrom = -1;
  });
  row.addEventListener('dragover', (e) => {
    if (dragFrom < 0) return;
    e.preventDefault();
    const r = row.getBoundingClientRect();
    markDrop(row, e.clientY < r.top + r.height / 2);
  });
  row.addEventListener('drop', (e) => {
    if (dragFrom < 0) return;
    e.preventDefault();
    e.stopPropagation();
    const r = row.getBoundingClientRect();
    const before = e.clientY < r.top + r.height / 2;
    clearMarks();
    move(dragFrom, before ? t.id : t.id + 1);
  });
  return row;
}

function renderDone(t) {
  const row = document.createElement('div');
  row.className = 'row';
  row.dataset.id = t.id;

  const mark = document.createElement('span');
  mark.className = 'done-mark';
  mark.appendChild(icon('check'));
  row.appendChild(mark);

  const body = document.createElement('div');
  body.className = 'body';
  const text = document.createElement('div');
  text.className = 'text done';
  text.textContent = t.text;
  body.appendChild(text);
  body.appendChild(metaEl(t, true));
  row.appendChild(body);

  const restore = actionBtn(tr('restoreTitle'), 'undo');
  restore.addEventListener('click', () =>
    tryRun(() => invoke('restore_task', { id: t.id }), tr('restored')));
  row.appendChild(restore);
  row.appendChild(deleteBtn('done', t.id));
  return row;
}

/* ---------- 拖拽排序 ---------- */

function markDrop(row, before) {
  clearMarks();
  row.classList.add(before ? 'drop-before' : 'drop-after');
}

function clearMarks() {
  els.list
    .querySelectorAll('.drop-before,.drop-after')
    .forEach((n) => n.classList.remove('drop-before', 'drop-after'));
}

function move(from, to) {
  dragFrom = -1;
  if (from === to || from + 1 === to) return; // 原地不动
  tryRun(() => invoke('move_task', { from, to }));
}

// 拖到列表空白处 = 移到末尾
els.list.addEventListener('dragover', (e) => {
  if (dragFrom >= 0) e.preventDefault();
});
els.list.addEventListener('drop', (e) => {
  if (dragFrom < 0) return;
  e.preventDefault();
  clearMarks();
  move(dragFrom, state.tasks.length);
});

/* ---------- 行内编辑 ---------- */

function startEdit(row, t) {
  if (state.editingId === t.id) return;
  state.editingId = t.id;
  row.draggable = false;
  const textEl = row.querySelector('.text');
  const input = document.createElement('input');
  input.className = 'edit';
  input.maxLength = 300;
  input.value = t.text;
  textEl.replaceWith(input);
  input.focus();
  const end = t.text.length;
  input.setSelectionRange(end, end);

  let finished = false;
  const finish = async (save) => {
    if (finished) return;
    finished = true;
    const value = input.value.trim();
    state.editingId = -1;
    if (save && value && value !== t.text) {
      await tryRun(() => invoke('update_task', { id: t.id, text: value }));
    } else {
      render(); // 取消/未改动，还原
    }
  };
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      finish(true);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      finish(false);
    }
  });
  input.addEventListener('blur', () => finish(true));
}

/* ---------- 顶部与页脚 ---------- */

function setTab(tab) {
  state.tab = tab;
  els.tabTodo.classList.toggle('active', tab === 'todo');
  els.tabDone.classList.toggle('active', tab === 'done');
  render();
}

els.tabTodo.addEventListener('click', () => setTab('todo'));
els.tabDone.addEventListener('click', () => setTab('done'));

els.add.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    const value = els.add.value.trim();
    if (!value) return;
    tryRun(() => invoke('add_task', { text: value })).then((r) => {
      if (r) els.add.value = '';
    });
  } else if (e.key === 'Escape') {
    els.add.value = '';
  }
});

let clearArmed = false;
let clearTimer = 0;
function resetClear() {
  clearArmed = false;
  clearTimeout(clearTimer);
  els.clear.textContent = tr('clear');
  els.clear.classList.remove('confirm');
}
els.clear.addEventListener('click', () => {
  if (state.done.length === 0) return;
  if (!clearArmed) {
    clearArmed = true;
    els.clear.textContent = tr('clearConfirm');
    els.clear.classList.add('confirm');
    clearTimer = setTimeout(resetClear, 2600);
  } else {
    resetClear();
    tryRun(() => invoke('clear_done'), tr('clearedDone'));
  }
});

els.dir.addEventListener('click', () => {
  invoke('open_data_dir').catch((e) => toast(tErr(e), true));
});

/* ---------- 设置 ---------- */

const DEFAULT_SETTINGS = { close_action: 'minimize', theme: 'auto', lang: 'zh', font: '', font_size: 13, hotkey: 'Ctrl+K' };
const FONT_PRESETS = [
  ['微软雅黑', '"Microsoft YaHei UI", "Microsoft YaHei"'],
  ['Segoe UI', '"Segoe UI"'],
  ['宋体 / SimSun', '"SimSun", "宋体"'],
  ['楷体 / KaiTi', '"KaiTi", "楷体"'],
  ['Consolas', 'Consolas, "Cascadia Mono", monospace'],
  ['Georgia', 'Georgia, serif'],
];
const CUSTOM = '__custom__';

let settings = { ...DEFAULT_SETTINGS };
const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

function applyTheme() {
  const dark = settings.theme === 'dark' || (settings.theme === 'auto' && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}
darkQuery.addEventListener('change', applyTheme);

function applyFont() {
  const root = document.documentElement.style;
  if (settings.font) root.setProperty('--font-user', `${settings.font}, `);
  else root.removeProperty('--font-user');
  root.setProperty('--fs', `${settings.font_size}px`);
}

function rebuildFontOptions() {
  els.setFont.replaceChildren();
  const add = (value, label) => {
    const o = document.createElement('option');
    o.value = value;
    o.textContent = label;
    els.setFont.appendChild(o);
  };
  add('', tr('fontDefault'));
  for (const [label, value] of FONT_PRESETS) add(value, label);
  add(CUSTOM, tr('fontCustom'));
}

function radioValue(group) {
  return group.querySelector('input:checked')?.value;
}
function setRadio(group, value) {
  for (const r of group.querySelectorAll('input[type="radio"]')) r.checked = r.value === value;
}

function syncFontSize(size) {
  const min = Number(els.setFontSize.min);
  const max = Number(els.setFontSize.max);
  els.setFontSize.value = size;
  els.setFontSize.style.setProperty('--p', `${((size - min) / (max - min)) * 100}%`);
  els.fontSizeVal.textContent = `${size}px`;
}

function syncSettingsForm() {
  setRadio(els.setClose, settings.close_action);
  setRadio(els.setTheme, settings.theme);
  setRadio(els.setLang, settings.lang);
  const isPreset = settings.font === '' || FONT_PRESETS.some(([, v]) => v === settings.font);
  els.setFont.value = isPreset ? settings.font : CUSTOM;
  els.fieldFontCustom.hidden = isPreset;
  els.setFontCustom.value = isPreset ? '' : settings.font.replace(/^"(.*)"$/, '$1');
  syncFontSize(settings.font_size);
  stopRecording();
  els.setHotkey.textContent = settings.hotkey;
  els.hotkeyReset.hidden = settings.hotkey === DEFAULT_SETTINGS.hotkey;
}

function applySettings() {
  setLang(settings.lang);
  rebuildFontOptions();
  applyTheme();
  applyFont();
  syncSettingsForm();
  resetClear();
  render();
}

async function saveSettings(patch) {
  const prev = settings;
  settings = { ...settings, ...patch };
  applyTheme();
  applyFont();
  if (patch.lang !== undefined) applySettings();
  try {
    settings = await invoke('set_settings', { settings });
  } catch (e) {
    settings = prev;
    applySettings();
    toast(tErr(e), true);
  }
  syncSettingsForm();
}

// 自定义字体名：单个名字自动加引号；含逗号/引号则视为完整 font-family 原样使用
function customFontValue() {
  const v = els.setFontCustom.value.trim();
  return /[",]/.test(v) || v === '' ? v : `"${v}"`;
}

els.setClose.addEventListener('change', () => saveSettings({ close_action: radioValue(els.setClose) }));
els.setTheme.addEventListener('change', () => saveSettings({ theme: radioValue(els.setTheme) }));
els.setLang.addEventListener('change', () => saveSettings({ lang: radioValue(els.setLang) }));
els.setFont.addEventListener('change', () => {
  if (els.setFont.value === CUSTOM) {
    els.fieldFontCustom.hidden = false;
    els.setFontCustom.focus();
    if (els.setFontCustom.value.trim()) saveSettings({ font: customFontValue() });
  } else {
    saveSettings({ font: els.setFont.value });
  }
});
els.setFontCustom.addEventListener('change', () => saveSettings({ font: customFontValue() }));
els.setFontCustom.addEventListener('keydown', (e) => e.stopPropagation());
els.setFontSize.addEventListener('input', () => {
  settings.font_size = Number(els.setFontSize.value);
  syncFontSize(settings.font_size);
  applyFont();
});
els.setFontSize.addEventListener('change', () => saveSettings({ font_size: Number(els.setFontSize.value) }));

/* 热键录制：点按钮后按下组合键即保存，Esc 或失焦取消 */
const HOTKEY_CODE = /^(Key[A-Z]|Digit\d|F\d{1,2}|Space|Enter|Tab|Arrow(Up|Down|Left|Right)|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Backquote|Minus|Equal|Bracket(Left|Right)|Backslash|Semicolon|Quote|Comma|Period|Slash)$/;
let recording = false;

function stopRecording() {
  recording = false;
  els.setHotkey.classList.remove('recording');
  els.setHotkey.textContent = settings.hotkey;
}

function startRecording() {
  recording = true;
  els.setHotkey.classList.add('recording');
  els.setHotkey.textContent = tr('hotkeyPress');
}

function comboFromEvent(e) {
  const mods = [];
  if (e.ctrlKey) mods.push('Ctrl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');
  if (e.metaKey) mods.push(/Mac/i.test(navigator.platform) ? 'Cmd' : 'Super');
  const key = e.code.replace(/^Key|^Digit/, '');
  return { mods, key };
}

els.setHotkey.addEventListener('click', () => (recording ? stopRecording() : startRecording()));
els.setHotkey.addEventListener('blur', () => recording && stopRecording());
els.hotkeyReset.addEventListener('click', () => saveSettings({ hotkey: DEFAULT_SETTINGS.hotkey }));
window.addEventListener(
  'keydown',
  (e) => {
    if (!recording) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) return stopRecording();
    if (!HOTKEY_CODE.test(e.code)) return; // 仅按下修饰键时继续等待
    const { mods, key } = comboFromEvent(e);
    if (mods.length === 0) return toast(tr('hotkeyNeedMod'), true);
    stopRecording();
    saveSettings({ hotkey: [...mods, key].join('+') });
  },
  true,
);

let closeTimer = 0;
function openSettings() {
  clearTimeout(closeTimer);
  syncSettingsForm();
  els.settings.classList.remove('closing');
  els.settings.hidden = false;
}
function closeSettings() {
  if (els.settings.hidden || els.settings.classList.contains('closing')) return;
  els.settings.classList.add('closing');
  closeTimer = setTimeout(() => {
    els.settings.hidden = true;
    els.settings.classList.remove('closing');
    els.add.focus();
  }, 160);
}
els.settingsBtn.addEventListener('click', openSettings);
els.settingsClose.addEventListener('click', closeSettings);
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && !els.settings.hidden) closeSettings();
});

/* ---------- 启动 ---------- */

applySettings();
if (!invoke) {
  els.list.replaceChildren(emptyEl('inbox', tr('openInApp')));
} else {
  invoke('get_settings')
    .then((s) => {
      settings = { ...DEFAULT_SETTINGS, ...s };
      applySettings();
    })
    .catch(() => {});
  invoke('data_dir')
    .then((d) => (els.dir.title = d))
    .catch(() => {});
  const getVersion = window.__TAURI__?.app?.getVersion;
  if (getVersion) {
    getVersion()
      .then((v) => ($('about').textContent = `tinyTD v${v}`))
      .catch(() => {});
  }
  refresh().catch((e) => toast(tErr(e), true));
  els.add.focus();
  // 通过托盘/热键唤起窗口时，直接聚焦输入框
  window.addEventListener('focus', () => {
    if (document.activeElement === document.body) els.add.focus();
  });
}
