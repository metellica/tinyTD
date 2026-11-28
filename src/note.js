// tinyTD 独立笔记窗口：一个任务的笔记，可自由调整大小，适合看长代码、截图
const invoke = window.__TAURI__?.core?.invoke;
const NOTE = window.__TINYTD_NOTE__ || {}; // 由后端在创建窗口时注入：{ key, title }
const NOTE_WIN_KEY = 'tinytd.noteWin'; // 与主窗口共用：下次打开沿用这个尺寸
const $ = (id) => document.getElementById(id);
const ed = $('note-editor');
const win = window.__TAURI__?.window?.getCurrentWindow?.();
const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

const sess = { key: NOTE.key, dirty: false, touched: false, stamp: null, orig: null };
let saveTimer = 0;
let queue = Promise.resolve();
let failed = false;
let gone = false; // 任务已被删除：不再保存
let look = null;

let toastTimer = 0;
function toast(msg, isErr = false) {
  const el = $('toast');
  el.textContent = msg;
  el.classList.toggle('err', isErr);
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), 2400);
}

function applyLook(s) {
  look = s;
  setLang(s.lang);
  const dark = s.theme === 'dark' || (s.theme === 'auto' && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  const root = document.documentElement.style;
  if (s.font) root.setProperty('--font-user', `${s.font}, `);
  else root.removeProperty('--font-user');
  root.setProperty('--fs', `${s.font_size}px`);
}
darkQuery.addEventListener('change', () => look && applyLook(look));

/** 保存笔记；prune：顺带清理正文不再引用的图片（关窗时） */
function save(prune) {
  clearTimeout(saveTimer);
  if (gone || (!sess.dirty && !(prune && sess.touched))) return queue;
  const content = MD.fromHtml(ed);
  sess.dirty = false;
  queue = queue.then(async () => {
    try {
      await invoke('write_note', { key: sess.key, content, prune });
      failed = false;
    } catch (e) {
      sess.dirty = true;
      failed = true;
      toast(`${tr('noteSaveFailed')}：${tErr(e)}`, true);
    }
  });
  return queue;
}

NoteEditor.attach(ed, {
  session: () => (gone ? null : sess),
  ensureKey: async () => sess.key,
  changed: () => {
    sess.dirty = sess.touched = true;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => save(false), 800);
  },
  toast,
});

// 点正文下方空白处：光标放到末尾
$('note-scroll').addEventListener('mousedown', (e) => {
  if (e.target.closest('#note-editor, .note-doc-title')) return;
  e.preventDefault();
  ed.focus();
  NoteEditor.placeCaret(ed, true);
});

// 关窗前先保存；保存失败时留住窗口一次，再关就不拦了
let closeBlocked = false;
win?.onCloseRequested(async (e) => {
  NoteEditor.dropEmptyStamp(sess);
  await save(true);
  if (failed && !gone && !closeBlocked) {
    closeBlocked = true;
    e.preventDefault();
  }
});
window.addEventListener('blur', () => save(false));

let sizeTimer = 0;
window.addEventListener('resize', () => {
  clearTimeout(sizeTimer);
  sizeTimer = setTimeout(() => {
    if (innerWidth >= screen.availWidth - 16) return; // 最大化时不记
    try {
      localStorage.setItem(NOTE_WIN_KEY, JSON.stringify({ w: innerWidth, h: innerHeight }));
    } catch {}
  }, 400);
});

// 拖进窗口的文件不处理时，阻止 WebView 直接打开它
window.addEventListener('dragover', (e) => e.dataTransfer.types.includes('Files') && e.preventDefault());
window.addEventListener('drop', (e) => e.dataTransfer.types.includes('Files') && e.preventDefault());

async function start() {
  if (!invoke || !sess.key) return;
  try {
    applyLook(await invoke('get_settings'));
  } catch {}
  $('note-title').textContent = NOTE.title || '';
  let md = '';
  try {
    md = await invoke('read_note', { key: sess.key });
  } catch (e) {
    toast(tErr(e), true);
  }
  if (md.trim()) ed.innerHTML = MD.toHtml(md, NoteEditor.noteSrc);
  else ed.replaceChildren(NoteEditor.placeholderLine());
  ed.focus();
  NoteEditor.placeCaret(ed.firstChild || ed);

  const listen = window.__TAURI__.event.listen;
  listen('settings-changed', (e) => applyLook(e.payload));
  // 主窗口里暂停了这个任务：顶部插入「⏸ 时间」接着记
  listen('note-stamp', (e) => {
    if (e.payload !== sess.key) return;
    NoteEditor.insertStamp(ed, sess);
    ed.focus();
    NoteEditor.placeCaret(sess.stamp.nextSibling);
    $('note-scroll').scrollTop = 0;
  });
  // 任务在主窗口被删除：笔记已随之删除，直接关窗
  listen('note-deleted', (e) => {
    if (e.payload !== sess.key) return;
    gone = true;
    win?.destroy();
  });
}

setLang('zh');
start();
