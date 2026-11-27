// tinyTD 前端 · 无框架原生 JS
const invoke = window.__TAURI__?.core?.invoke;

const state = {
  tab: 'todo', // 'todo' | 'done'
  tasks: [],
  done: [],
  confirmKey: '',
  confirmTimer: 0,
};

let dragFrom = -1;
let dragGroup = ''; // 拖动任务所在的分组：只允许在同组内排序
let flipNext = false; // 下一次渲染播放位移动画（仅用于切换状态，任务 id 不变）
const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

// 'doing' 进行中 | 'paused' 已暂停 | 'todo' 普通待办；也是列表自上而下的分组顺序
const GROUPS = ['doing', 'paused', 'todo'];
const statusOf = (t) => (t.doing ? 'doing' : t.spent ? 'paused' : 'todo');

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
  setAutoNote: $('set-auto-note'),
  note: $('note'),
  noteClose: $('btn-note-close'),
  noteTitle: $('note-title'),
  noteMeta: $('note-meta'),
  noteEditor: $('note-editor'),
};

/* ---------- 工具 ---------- */

const ICONS = {
  grip: '<circle cx="9" cy="6" r="1.6"/><circle cx="15" cy="6" r="1.6"/><circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/><circle cx="9" cy="18" r="1.6"/><circle cx="15" cy="18" r="1.6"/>',
  x: '<path d="M6 6l12 12M18 6L6 18"/>',
  undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  notes: '<path d="M6 7h12M6 12h12M6 17h7"/>',
  expand: '<path d="M14 4h6v6M10 20H4v-6M20 4l-6.5 6.5M4 20l6.5-6.5"/>',
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

function parseTs(ts) {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(ts || '');
  return m ? new Date(+m[1], m[2] - 1, +m[3], +m[4], +m[5]) : null;
}

function durText(ms) {
  const mins = Math.floor(ms / 60000);
  if (!(mins >= 1)) return '';
  return tr('dur', Math.floor(mins / 1440), Math.floor((mins % 1440) / 60), mins % 60);
}

function doingLabel(started, spent) {
  const s = parseTs(started);
  if (!s) return tr('doingNow');
  const total = (spent || 0) * 60000 + (Date.now() - s);
  return `${tr('doingNow')} · ${durText(total) || tr('doingJust')}`;
}

function metaLine(t, withCompleted) {
  const parts = [];
  if (withCompleted && t.completed) parts.push(`${tr('completedAt')} ${fmt(t.completed)}`);
  if (withCompleted && t.spent) parts.push(`${tr('took')} ${durText(t.spent * 60000)}`);
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
  reconcileCard();
  render();
}

async function tryRun(fn, okMsg) {
  try {
    await noteQueue; // 先让笔记保存 / 改名落盘，避免行号错位
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
  const keep = captureFocus();
  renderList();
  mountEditor(keep);
}

function renderList() {
  const before = flipNext ? rowTops() : null;
  flipNext = false;
  const groups = Object.fromEntries(GROUPS.map((g) => [g, []]));
  for (const t of state.tasks) groups[statusOf(t)].push(t);
  els.countTodo.textContent = state.tasks.length;
  els.countDone.textContent = state.done.length;
  els.clear.hidden = state.tab !== 'done';
  els.stats.textContent = tr('stats', state.tasks.length, state.done.length, groups.doing.length, groups.paused.length);
  els.list.replaceChildren();
  const items = state.tab === 'todo' ? state.tasks : state.done;
  if (items.length === 0) {
    els.list.appendChild(state.tab === 'todo'
      ? emptyEl('sun', tr('emptyTodo'))
      : emptyEl('inbox', tr('emptyDone')));
    return;
  }
  if (state.tab === 'done') {
    for (const t of items) els.list.appendChild(renderDone(t));
    return;
  }
  // 按 进行中 / 已暂停 / 待办 分组显示；文件里的行序不变，回到待办时自然回到原位
  let first = true;
  for (const g of GROUPS) {
    if (!groups[g].length) continue;
    if (!first) {
      const sep = document.createElement('div');
      sep.className = 'group-sep';
      els.list.appendChild(sep);
    }
    first = false;
    for (const t of groups[g]) els.list.appendChild(renderTask(t));
  }
  if (before) playFlip(before);
}

function rowTops() {
  const tops = new Map();
  for (const r of els.list.querySelectorAll('.row')) tops.set(r.dataset.id, r.getBoundingClientRect().top);
  return tops;
}

function playFlip(before) {
  if (reduceMotion.matches) return;
  for (const r of els.list.querySelectorAll('.row')) {
    const prev = before.get(r.dataset.id);
    if (prev === undefined) continue;
    const dy = prev - r.getBoundingClientRect().top;
    if (Math.abs(dy) < 1) continue;
    r.animate([{ transform: `translateY(${dy}px)` }, { transform: 'none' }], {
      duration: 320,
      easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
    });
  }
}

function actionBtn(title, iconName) {
  const btn = document.createElement('button');
  btn.className = 'icon-btn act';
  btn.title = title;
  btn.appendChild(icon(iconName));
  return btn;
}

function deleteBtn(scope, t) {
  const btn = actionBtn(tr('del'), 'x');
  btn.classList.add('danger');
  const key = `del:${scope}:${t.id}`;
  btn.addEventListener('click', () => {
    if (state.confirmKey === key) {
      clearTimeout(state.confirmTimer);
      state.confirmKey = '';
      closeCardOf(scope, t);
      const run = scope === 'todo'
        ? () => invoke('remove_task', { id: t.id })
        : () => invoke('delete_done', { id: t.id });
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
  const line = metaLine(t, withCompleted);
  const status = withCompleted ? 'todo' : statusOf(t);
  if (status === 'todo') {
    meta.textContent = line;
    return meta;
  }
  const tag = document.createElement('span');
  if (status === 'doing') {
    tag.className = 'doing-tag';
    tag.dataset.started = t.doing;
    tag.dataset.spent = t.spent || 0;
    tag.textContent = doingLabel(t.doing, t.spent);
  } else {
    tag.className = 'paused-tag';
    tag.textContent = `${tr('pausedNow')} · ${tr('spentTotal')} ${durText(t.spent * 60000)}`;
  }
  meta.append(tag, line ? ` · ${line}` : '');
  return meta;
}

// 有笔记的任务：标题右侧一个淡灰小标记（不显示内容）
function noteMark() {
  const mark = document.createElement('span');
  mark.className = 'note-mark';
  mark.title = tr('hasNote');
  mark.appendChild(icon('notes'));
  return mark;
}

// 卡片底部：笔记编辑区（编辑器节点在渲染后挂进来）+ 全屏按钮
function cardParts(row, body) {
  const host = document.createElement('div');
  host.className = 'card-note';
  // 点到笔记区空白处：光标放到末尾
  host.addEventListener('mousedown', (e) => {
    if (e.target !== host) return;
    e.preventDefault();
    els.noteEditor.focus();
    placeCaret(els.noteEditor, true);
  });
  body.appendChild(host);
  const full = actionBtn(tr('fullscreen'), 'expand');
  full.addEventListener('click', () => setCardFull(true));
  row.appendChild(full);
}

// 单击条目（圆圈、把手、行尾按钮以外）展开卡片；点在标题上则直接编辑标题
function onRowClick(row, scope, t) {
  row.addEventListener('click', (e) => {
    if (row.classList.contains('expanded')) return;
    if (e.composedPath().some((n) => n.matches?.('.check, .handle, .act'))) return;
    const onText = e.composedPath().some((n) => n.matches?.('.text'));
    openCard(scope, t, { focus: onText && scope === 'todo' ? 'title' : 'note' });
  });
}

const CIRCLE_TITLE = { todo: 'circleTodo', doing: 'circleDoing', paused: 'circlePaused' };

function renderTask(t) {
  const status = statusOf(t);
  const open = isCardOf('todo', t);
  const row = document.createElement('div');
  row.className = ['row', status === 'todo' ? '' : status, open ? 'expanded' : ''].filter(Boolean).join(' ');
  row.dataset.id = t.id;

  if (status === 'doing') {
    const bar = document.createElement('span');
    bar.className = 'doing-bar';
    row.appendChild(bar);
  }

  const handle = document.createElement('span');
  handle.className = 'handle';
  handle.appendChild(icon('grip'));
  handle.title = tr('dragTitle');
  handle.addEventListener('mousedown', () => (row.draggable = !open));
  handle.addEventListener('mouseup', () => (row.draggable = false));
  row.appendChild(handle);

  // 圆圈：单击 开始 → 暂停 → 继续 …；Shift+单击 完成
  const circle = document.createElement('button');
  circle.className = 'check';
  circle.title = tr(CIRCLE_TITLE[status]);
  circle.addEventListener('click', (e) => {
    if (row.classList.contains('completing')) return;
    if (e.shiftKey) {
      closeCardOf('todo', t);
      circle.classList.add('checked');
      row.classList.add('completing');
      setTimeout(() => tryRun(() => invoke('complete_task', { id: t.id })), 160);
      return;
    }
    flipNext = true;
    const next = status === 'doing' ? 'paused' : 'doing';
    tryRun(() => invoke('set_status', { id: t.id, status: next })).then(() => {
      flipNext = false;
      autoOpenCard(t, status, next);
    });
  });
  row.appendChild(circle);

  const body = document.createElement('div');
  body.className = 'body';
  if (open) {
    body.appendChild(titleInput(t));
  } else {
    const text = document.createElement('div');
    text.className = 'text';
    text.textContent = t.text;
    text.title = tr('clickEdit');
    body.appendChild(text);
  }
  body.appendChild(metaEl(t, false));
  row.appendChild(body);

  if (open) cardParts(row, body);
  else if (t.has_note) row.appendChild(noteMark());
  row.appendChild(deleteBtn('todo', t));
  onRowClick(row, 'todo', t);

  row.addEventListener('dragstart', (e) => {
    dragFrom = t.id;
    dragGroup = status;
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
  const sameGroup = () => dragFrom >= 0 && dragGroup === status;
  row.addEventListener('dragover', (e) => {
    if (!sameGroup()) return clearMarks();
    e.preventDefault();
    const r = row.getBoundingClientRect();
    markDrop(row, e.clientY < r.top + r.height / 2);
  });
  row.addEventListener('drop', (e) => {
    if (!sameGroup()) return;
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
  const open = isCardOf('done', t);
  const row = document.createElement('div');
  row.className = open ? 'row expanded' : 'row';
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
  if (!open) text.title = tr('clickNote');
  body.appendChild(text);
  body.appendChild(metaEl(t, true));
  row.appendChild(body);

  if (open) cardParts(row, body);
  else if (t.has_note) row.appendChild(noteMark());
  const restore = actionBtn(tr('restoreTitle'), 'undo');
  restore.addEventListener('click', () => {
    closeCardOf('done', t);
    tryRun(() => invoke('restore_task', { id: t.id }), tr('restored'));
  });
  row.appendChild(restore);
  row.appendChild(deleteBtn('done', t));
  onRowClick(row, 'done', t);
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

// 拖到列表空白处 = 移到末尾（仅普通待办；行与分隔线上的事件由行自己处理）
const onBlank = (e) => dragFrom >= 0 && dragGroup === 'todo' && !e.target.closest('.row, .group-sep');
els.list.addEventListener('dragover', (e) => {
  if (onBlank(e)) e.preventDefault();
});
els.list.addEventListener('drop', (e) => {
  if (!onBlank(e)) return;
  e.preventDefault();
  clearMarks();
  move(dragFrom, state.tasks.length);
});

// 按住 Shift 时，悬停的圆圈预览「完成」
const setShift = (on) => document.documentElement.classList.toggle('shift', on);
window.addEventListener('keydown', (e) => e.key === 'Shift' && setShift(true));
window.addEventListener('keyup', (e) => e.key === 'Shift' && setShift(false));
window.addEventListener('blur', () => setShift(false));
els.list.addEventListener('mousemove', (e) => setShift(e.shiftKey));

/* ---------- 顶部与页脚 ---------- */

function setTab(tab) {
  if (state.tab !== tab) closeCard();
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

/* ---------- 任务卡片与笔记 ---------- */

// 单击条目就地展开成卡片：标题可直接修改，下面是笔记（富文本，可贴图）。
// 同一时间只展开一张；笔记编辑器只有一个 DOM 节点，在卡片与全屏面板之间移动。

const noteSrc = (rel) => window.__TAURI__?.core?.convertFileSrc?.(rel, 'tdnote') ?? rel;
const IMG_EXT = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/gif': 'gif', 'image/webp': 'webp' };

let card = null; // 当前展开的卡片
let cardOpening = false;
let noteQueue = Promise.resolve(); // 笔记保存、改名等按顺序执行
let noteCloseTimer = 0;
const noteFullHost = els.noteEditor.parentElement;

const sameTask = (a, b) => a.created === b.created && a.text === b.text;
const isCardOf = (scope, t) => !!card && card.scope === scope && card.id === t.id;

// 按 id 找任务；行序变了（排序、删除）就按创建时间 + 文本找回
function findTask(scope, ref) {
  const list = scope === 'todo' ? state.tasks : state.done;
  const t = list[ref.id];
  if (t && sameTask(t, ref)) return t;
  return list.find((x) => sameTask(x, ref)) || (ref.key && list.find((x) => x.note === ref.key)) || null;
}

function localTs(d = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function placeCaret(node, atEnd = false) {
  const r = document.createRange();
  r.selectNodeContents(node);
  r.collapse(!atEnd);
  const sel = getSelection();
  sel.removeAllRanges();
  sel.addRange(r);
}

function placeholderLine() {
  const line = document.createElement('div');
  line.className = 'ph';
  line.setAttribute('placeholder', tr('notePlaceholder'));
  line.appendChild(document.createElement('br'));
  return line;
}

/* 卡片标题：直接修改；Enter 跳到笔记，Esc 放弃修改并收起 */
function titleInput(t) {
  const input = document.createElement('input');
  input.className = 'edit title';
  input.maxLength = 300;
  input.value = card.titleDraft ?? t.text;
  input.spellcheck = false;
  input.addEventListener('input', () => card && (card.titleDraft = input.value));
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      commitTitle();
      els.noteEditor.focus();
      placeCaret(els.noteEditor.firstChild || els.noteEditor);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      if (card) card.titleDraft = null;
      closeCard();
    }
  });
  input.addEventListener('blur', () => commitTitle());
  return input;
}

function commitTitle() {
  const c = card;
  if (!c || c.scope !== 'todo' || c.titleDraft == null) return;
  const value = c.titleDraft.trim();
  c.titleDraft = null;
  if (!value || value === c.text) return;
  const old = c.text;
  c.text = value; // 之后申请笔记编号时按新标题核对
  noteQueue = noteQueue
    .then(() => invoke('update_task', { id: c.id, text: value }))
    .then(refresh)
    .catch((e) => {
      c.text = old;
      toast(tErr(e), true);
      refresh().catch(() => {});
    });
}

/* 编辑器在重新渲染前后保持焦点与光标 */
function captureFocus() {
  const ed = els.noteEditor;
  const a = document.activeElement;
  if (a && a.classList?.contains('title')) return { title: a.selectionStart };
  if (!ed.contains(a) || !getSelection().rangeCount) return null;
  const r = getSelection().getRangeAt(0);
  return { range: [r.startContainer, r.startOffset, r.endContainer, r.endOffset] };
}

function mountEditor(keep) {
  const ed = els.noteEditor;
  const host = !card ? null : card.full ? noteFullHost : els.list.querySelector('.row.expanded .card-note');
  if (!host) {
    if (ed.parentElement !== noteFullHost) noteFullHost.appendChild(ed);
    return;
  }
  if (ed.parentElement !== host) host.appendChild(ed);
  if (!keep) return;
  if (keep.title !== undefined) {
    const input = els.list.querySelector('.row.expanded .title');
    if (input && !card.full) {
      input.focus();
      input.setSelectionRange(keep.title, keep.title);
    }
    return;
  }
  ed.focus();
  try {
    getSelection().setBaseAndExtent(...keep.range);
  } catch {
    placeCaret(ed, true);
  }
}

/** 展开卡片。focus：'title' | 'note' | 'none'；stamp：在笔记顶部插入「⏸ 时间」并把光标放在其下 */
async function openCard(scope, t, { focus = 'note', stamp = false } = {}) {
  if (isCardOf(scope, t)) {
    if (stamp) insertStamp(card);
    focusCard(focus, stamp);
    return;
  }
  if (cardOpening) return;
  cardOpening = true;
  try {
    closeCard();
    await noteQueue; // 等上一张卡片保存完、列表刷新完
    t = findTask(scope, t);
    if (!t) return;
    let md = '';
    if (t.note && t.has_note) {
      try {
        md = await invoke('read_note', { key: t.note });
      } catch (e) {
        toast(tErr(e), true);
        return;
      }
    }
    const ed = els.noteEditor;
    if (md.trim()) ed.innerHTML = MD.toHtml(md, noteSrc);
    else ed.replaceChildren(placeholderLine());
    card = {
      scope,
      id: t.id,
      created: t.created,
      text: t.text,
      key: t.note || null,
      dirty: false,
      touched: false,
      stamp: null,
      orig: null,
      timer: 0,
      full: false,
      titleDraft: null,
    };
    if (stamp) insertStamp(card);
    render();
    els.list.querySelector('.row.expanded')?.scrollIntoView({ block: 'nearest' });
    focusCard(focus, stamp);
  } finally {
    cardOpening = false;
  }
}

function focusCard(focus, stamp) {
  const ed = els.noteEditor;
  if (focus === 'title') {
    const input = els.list.querySelector('.row.expanded .title');
    if (input) {
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    }
  } else if (focus === 'note') {
    ed.focus();
    placeCaret(stamp && card.stamp ? card.stamp.nextSibling : ed.firstChild || ed);
  }
}

// 暂停时在顶部插入「⏸ 时间」分隔和一个空行
function insertStamp(c) {
  dropEmptyStamp(c);
  const ed = els.noteEditor;
  if (ed.childNodes.length === 1 && ed.firstChild.classList?.contains('ph')) ed.replaceChildren();
  c.orig = new Set(ed.childNodes);
  const h = document.createElement('h3');
  h.className = 'stamp';
  h.textContent = `⏸ ${localTs()}`;
  ed.prepend(h, placeholderLine());
  c.stamp = h;
}

// 分隔下面什么都没写，就把分隔撤掉
function dropEmptyStamp(c) {
  const h = c.stamp;
  c.stamp = null;
  if (!h || !h.isConnected) return;
  const added = [];
  for (let n = h.nextSibling; n && !c.orig.has(n); n = n.nextSibling) added.push(n);
  const hasContent = (n) => n.textContent.trim() || n.nodeName === 'IMG' || n.querySelector?.('img');
  if (added.some(hasContent)) return;
  [h, ...added].forEach((n) => n.remove());
  if (c.touched) c.dirty = true;
}

function scheduleNoteSave(c) {
  clearTimeout(c.timer);
  c.timer = setTimeout(() => saveNote(c, false), 800);
}

/** 保存笔记；第一次有内容时才申请笔记编号。prune：清理正文不再引用的图片 */
function saveNote(c, prune) {
  clearTimeout(c.timer);
  if (!c.dirty && !(prune && c.touched && c.key)) return noteQueue;
  const md = MD.fromHtml(els.noteEditor);
  c.dirty = false;
  noteQueue = noteQueue.then(async () => {
    try {
      if (!c.key) {
        if (!md.trim()) return;
        c.key = await invoke('note_key', { scope: c.scope, id: c.id, created: c.created, text: c.text });
      }
      await invoke('write_note', { key: c.key, content: md, prune });
    } catch (e) {
      c.dirty = true;
      toast(`${tr('noteSaveFailed')}：${tErr(e)}`, true);
    }
  });
  return noteQueue;
}

/** 收起卡片：保存笔记，只把这一行就地换回普通行（不打断正在进行的点击 / 拖动） */
function closeCard() {
  const c = card;
  if (!c) return;
  commitTitle();
  dropEmptyStamp(c);
  saveNote(c, true);
  card = null;
  if (c.full) hideFull();
  els.noteEditor.replaceChildren();
  noteFullHost.appendChild(els.noteEditor);
  const row = els.list.querySelector('.row.expanded');
  const t = findTask(c.scope, c);
  if (row && t) row.replaceWith(c.scope === 'todo' ? renderTask(t) : renderDone(t));
  else if (row) render();
  // 刷新以更新「有笔记」小标记
  if (c.touched) noteQueue = noteQueue.then(refresh).catch(() => {});
}

function closeCardOf(scope, t) {
  if (isCardOf(scope, t)) closeCard();
}

/* 全屏：长笔记、看图时用；返回后仍是展开的卡片 */
function setCardFull(on) {
  if (!card) return;
  const keep = captureFocus();
  card.full = on;
  if (on) {
    clearTimeout(noteCloseTimer);
    const t = findTask(card.scope, card);
    els.noteTitle.textContent = card.text;
    els.noteTitle.title = card.text;
    els.noteMeta.textContent = t ? metaEl(t, card.scope === 'done').textContent : '';
    els.note.classList.remove('closing');
    els.note.hidden = false;
    mountEditor(null);
    els.noteEditor.focus();
    if (keep?.range) getSelection().setBaseAndExtent(...keep.range);
  } else {
    hideFull();
    mountEditor(keep?.range ? keep : null);
  }
}

function hideFull() {
  els.note.classList.add('closing');
  noteCloseTimer = setTimeout(() => {
    els.note.hidden = true;
    els.note.classList.remove('closing');
  }, 160);
}

// 暂停：展开卡片记一笔（不写就点别处，什么都不留下）；继续有笔记的任务：展开给你看
function autoOpenCard(t, from, to) {
  if (!settings.auto_note) return;
  const fresh = findTask('todo', t);
  if (!fresh || statusOf(fresh) !== to) return; // 失败或 1 分钟内的误触
  if (to === 'paused') openCard('todo', fresh, { stamp: true });
  else if (from === 'paused' && fresh.has_note) openCard('todo', fresh, { focus: 'none' });
}

// 列表刷新后核对卡片对应的任务（行号可能已变）；找不到就收起
function reconcileCard() {
  if (!card) return;
  const t = findTask(card.scope, card);
  if (!t) {
    const c = card;
    saveNote(c, true);
    card = null;
    if (c.full) hideFull();
    return;
  }
  card.id = t.id;
  card.key = card.key || t.note || null;
}

/* 图片：粘贴 / 拖入后存到 notes/<编号>/，正文里只记相对路径 */
async function imageBytes(file) {
  const ext = IMG_EXT[file.type];
  if (ext) return { ext, bytes: new Uint8Array(await file.arrayBuffer()) };
  // 其他格式（如 bmp / tiff）转成 png
  const bmp = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = bmp.width;
  canvas.height = bmp.height;
  canvas.getContext('2d').drawImage(bmp, 0, 0);
  const blob = await new Promise((ok) => canvas.toBlob(ok, 'image/png'));
  return { ext: 'png', bytes: new Uint8Array(await blob.arrayBuffer()) };
}

async function insertImages(files) {
  const c = card;
  if (!c) return;
  const sel = getSelection();
  const range = sel.rangeCount && els.noteEditor.contains(sel.anchorNode) ? sel.getRangeAt(0).cloneRange() : null;
  try {
    if (!c.key) {
      await noteQueue;
      c.key = c.key || (await invoke('note_key', { scope: c.scope, id: c.id, created: c.created, text: c.text }));
    }
    let html = '';
    for (const f of files) {
      const { ext, bytes } = await imageBytes(f);
      const rel = await invoke('save_note_image', bytes, { headers: { 'x-note-key': c.key, 'x-ext': ext } });
      html += `<img src="${noteSrc(rel)}" data-src="${rel}" alt="">`;
    }
    if (card !== c) return;
    els.noteEditor.focus();
    if (range) {
      sel.removeAllRanges();
      sel.addRange(range);
    } else {
      placeCaret(els.noteEditor, true);
    }
    document.execCommand('insertHTML', false, html);
  } catch (e) {
    toast(tErr(e), true);
  }
}

const imageFiles = (list) => [...list].filter((f) => f && f.type.startsWith('image/'));

// 行首输入 - / 1. / # 再按空格：转成列表或标题
function autoFormat(e) {
  const ed = els.noteEditor;
  const sel = getSelection();
  if (!sel.rangeCount || !sel.isCollapsed) return;
  const r = sel.getRangeAt(0);
  let blk = r.startContainer;
  while (blk && blk !== ed && !(blk.parentNode === ed && /^(DIV|P)$/.test(blk.nodeName))) blk = blk.parentNode;
  if (!blk || blk === ed) return;
  const head = document.createRange();
  head.setStart(blk, 0);
  head.setEnd(r.startContainer, r.startOffset);
  const s = head.toString();
  let cmd;
  let val;
  if (/^[-*+]$/.test(s)) cmd = 'insertUnorderedList';
  else if (/^1[.)]$/.test(s)) cmd = 'insertOrderedList';
  else if (/^#{1,3}$/.test(s)) [cmd, val] = ['formatBlock', `h${s.length}`];
  else return;
  e.preventDefault();
  sel.removeAllRanges();
  sel.addRange(head);
  document.execCommand('delete');
  document.execCommand(cmd, false, val);
}

(() => {
  const ed = els.noteEditor;
  document.execCommand('defaultParagraphSeparator', false, 'div');
  ed.addEventListener('input', () => {
    const c = card;
    if (!c) return;
    ed.querySelectorAll('.ph').forEach((n) => n.classList.remove('ph'));
    c.dirty = c.touched = true;
    scheduleNoteSave(c);
  });
  ed.addEventListener('keydown', (e) => {
    if (e.key === ' ' && !e.ctrlKey && !e.metaKey && !e.altKey) autoFormat(e);
  });
  ed.addEventListener('paste', (e) => {
    const cd = e.clipboardData;
    if (!cd) return;
    e.preventDefault();
    const files = imageFiles([...cd.items].filter((i) => i.kind === 'file').map((i) => i.getAsFile()));
    if (files.length) return void insertImages(files);
    const html = cd.getData('text/html');
    if (!html) return void document.execCommand('insertText', false, cd.getData('text/plain'));
    // 网页 / 文档里复制来的内容：只保留笔记支持的格式
    const doc = new DOMParser().parseFromString(html, 'text/html');
    let out = MD.toHtml(MD.fromHtml(doc.body), noteSrc);
    const single = /^<div>([\s\S]*)<\/div>$/.exec(out);
    if (single && !single[1].includes('<div>')) out = single[1]; // 单行内容就地插入，不另起一行
    document.execCommand('insertHTML', false, out);
  });
  ed.addEventListener('dragover', (e) => {
    if (e.dataTransfer.types.includes('Files')) e.preventDefault();
  });
  ed.addEventListener('drop', (e) => {
    const files = imageFiles(e.dataTransfer.files);
    if (!files.length) return;
    e.preventDefault();
    e.stopPropagation();
    const r = document.caretRangeFromPoint?.(e.clientX, e.clientY);
    if (r) {
      const sel = getSelection();
      sel.removeAllRanges();
      sel.addRange(r);
    }
    insertImages(files);
  });
  ed.addEventListener('click', (e) => {
    const a = e.target.closest?.('a');
    if (!a) return;
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) invoke('open_link', { url: a.getAttribute('href') }).catch((err) => toast(tErr(err), true));
  });
  ed.addEventListener('mouseover', (e) => {
    const a = e.target.closest?.('a');
    if (a) a.title = a.getAttribute('href');
  });
  ed.addEventListener('dblclick', (e) => {
    const src = e.target.nodeName === 'IMG' && e.target.dataset.src;
    if (src) invoke('open_note_image', { rel: src }).catch((err) => toast(tErr(err), true));
  });
  els.noteClose.addEventListener('click', () => setCardFull(false));
  // 点卡片以外的地方收起（捕获阶段先收起，再让这次点击照常生效）
  document.addEventListener(
    'click',
    (e) => {
      if (!card || card.full) return;
      const row = els.list.querySelector('.row.expanded');
      if (e.composedPath().some((n) => n === row || n === els.toast)) return;
      closeCard();
    },
    true,
  );
  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || !card || recording) return;
    e.preventDefault();
    if (card.full) setCardFull(false);
    else closeCard();
  });
  // 窗口被收起 / 切走时立即保存
  window.addEventListener('blur', () => card && saveNote(card, false));
  // 拖进窗口的文件不处理时，阻止 WebView 直接打开它而离开应用
  window.addEventListener('dragover', (e) => e.dataTransfer.types.includes('Files') && e.preventDefault());
  window.addEventListener('drop', (e) => e.dataTransfer.types.includes('Files') && e.preventDefault());
})();

/* ---------- 设置 ---------- */

const DEFAULT_SETTINGS = { close_action: 'minimize', theme: 'auto', lang: 'zh', font: '', font_size: 13, hotkey: 'Ctrl+K', auto_note: true };
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
  setRadio(els.setAutoNote, settings.auto_note ? 'on' : 'off');
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
els.setAutoNote.addEventListener('change', () => saveSettings({ auto_note: radioValue(els.setAutoNote) === 'on' }));
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
  // 进行中时长每 30 秒刷新一次，只改文字，不打断编辑/拖拽
  setInterval(() => {
    for (const tag of els.list.querySelectorAll('.doing-tag')) {
      tag.textContent = doingLabel(tag.dataset.started, Number(tag.dataset.spent));
    }
  }, 30000);
  els.add.focus();
  // 通过托盘/热键唤起窗口时，直接聚焦输入框
  window.addEventListener('focus', () => {
    if (document.activeElement === document.body) (card ? els.noteEditor : els.add).focus();
  });
}
