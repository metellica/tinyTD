// 笔记编辑器：主窗口（卡片 / 全屏）与独立笔记窗口共用
// 负责粘贴、拖入图片、行首快捷格式、代码块、图片查看器；何时保存由调用方决定。
const NoteEditor = (() => {
  const call = (cmd, args, opts) => window.__TAURI__.core.invoke(cmd, args, opts);
  const noteSrc = (rel) => window.__TAURI__?.core?.convertFileSrc?.(rel, 'tdnote') ?? rel;
  const IMG_EXT = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/gif': 'gif', 'image/webp': 'webp' };
  const LANG = /^[\w+#.-]{0,20}$/;
  const FOLD_LINES = 10; // 超过这么多行的代码块在小窗里默认折叠
  const MONO = /font-family:[^;"']*(monospace|consolas|menlo|monaco|courier|cascadia|jetbrains|fira code|source code)/i;
  const svg = (d) =>
    `<svg class="i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
  const ICON = {
    copy: '<rect x="8.5" y="8.5" width="11" height="11" rx="2"/><path d="M15.5 8.5V6a1.5 1.5 0 0 0-1.5-1.5H6A1.5 1.5 0 0 0 4.5 6v8A1.5 1.5 0 0 0 6 15.5h2.5"/>',
    fold: '<path d="M7 9l5-5 5 5M7 15l5 5 5-5"/>',
    open: '<path d="M14 4h6v6"/><path d="M20 4l-8 8"/><path d="M18 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10"/>',
    x: '<path d="M6 6l12 12M18 6L6 18"/>',
    prev: '<path d="M15 5l-7 7 7 7"/>',
    next: '<path d="M9 5l7 7-7 7"/>',
  };

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

  function emptyLine() {
    const line = document.createElement('div');
    line.appendChild(document.createElement('br'));
    return line;
  }

  function placeholderLine() {
    const line = emptyLine();
    line.className = 'ph';
    line.setAttribute('placeholder', tr('notePlaceholder'));
    return line;
  }

  /* ---------- 暂停分隔「⏸ 时间」 ---------- */

  // 在顶部插入分隔和一个空行；c 记录本次插入，便于没写内容时撤掉
  function insertStamp(ed, c) {
    dropEmptyStamp(c);
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

  /* ---------- 图片：粘贴 / 拖入后存到 notes/<编号>/，正文里只记相对路径 ---------- */

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

  const imageFiles = (list) => [...list].filter((f) => f && f.type.startsWith('image/'));

  /* ---------- 代码块 ---------- */

  function caretPre(ed) {
    const sel = getSelection();
    if (!sel.rangeCount) return null;
    let n = sel.anchorNode;
    if (n && n.nodeType !== 1) n = n.parentNode;
    const pre = n?.closest?.('pre');
    return pre && ed.contains(pre) ? pre : null;
  }

  const lineCount = (pre) => pre.textContent.replace(/\n$/, '').split('\n').length;

  function setFold(pre, on) {
    pre.classList.toggle('fold', on);
    if (on) pre.dataset.more = tr('codeMore', lineCount(pre));
    else delete pre.dataset.more;
  }

  /** 载入内容后调用：小窗里把长代码块折叠起来 */
  function decorate(ed, fold) {
    if (!fold) return;
    for (const pre of ed.querySelectorAll('pre')) if (lineCount(pre) > FOLD_LINES) setFold(pre, true);
  }

  // 代码块后面没有可落笔的行时补一个空行，免得光标出不去
  function ensureLineAfter(pre) {
    if (pre.nextElementSibling) return;
    pre.after(emptyLine());
  }

  // 回车：换行并沿用上一行的缩进（代码块里的换行可能是 \n 也可能是 <br>）
  function newlineInPre(pre) {
    const r = getSelection().getRangeAt(0);
    const head = document.createRange();
    head.selectNodeContents(pre);
    head.setEnd(r.startContainer, r.startOffset);
    let text = '';
    const walk = document.createTreeWalker(head.cloneContents(), NodeFilter.SHOW_ALL);
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      if (n.nodeType === 3) text += n.nodeValue;
      else if (n.nodeName === 'BR') text += '\n';
    }
    const indent = /^[ \t]*/.exec(text.split('\n').pop())[0];
    document.execCommand('insertLineBreak');
    if (indent) document.execCommand('insertText', false, indent);
  }

  // Ctrl+Enter：跳到代码块下一行
  function exitPre(pre) {
    ensureLineAfter(pre);
    placeCaret(pre.nextElementSibling);
  }

  // 粘贴的代码：在光标所在行处插入一个代码块（空行直接替换；行中间则把这一行拆开）
  function insertCode(ed, text, lang) {
    const sel = getSelection();
    const pre = document.createElement('pre');
    if (lang) pre.dataset.lang = lang;
    pre.textContent = text;
    let blk = sel.rangeCount && ed.contains(sel.anchorNode) ? sel.anchorNode : null;
    while (blk && blk.parentNode !== ed) blk = blk.parentNode;
    const empty = (n) => !n.textContent && !n.querySelector?.('img');
    if (!blk) {
      ed.appendChild(pre);
    } else if (empty(blk)) {
      blk.replaceWith(pre);
    } else if (!/^(DIV|P)$/.test(blk.nodeName)) {
      blk.after(pre); // 列表、标题、文字节点等：放在它后面，不拆
    } else {
      const r = sel.getRangeAt(0);
      r.deleteContents();
      const tail = document.createRange();
      tail.setStart(r.endContainer, r.endOffset);
      tail.setEndAfter(blk.lastChild || blk);
      const rest = tail.extractContents();
      if (empty(blk)) blk.replaceWith(pre);
      else blk.after(pre);
      if (!empty(rest)) {
        const line = document.createElement('div');
        line.append(rest);
        pre.after(line);
      }
    }
    ensureLineAfter(pre);
    placeCaret(pre.nextElementSibling);
    ed.dispatchEvent(new Event('input')); // 直接改了 DOM，没有 input 事件，手动通知保存
  }

  // 粘贴内容像代码时返回语言（可能为空串），否则返回 null
  function codeLang(cd, html, text) {
    if (!text.includes('\n')) return null;
    const vs = cd.getData('vscode-editor-data');
    if (vs) {
      try {
        const mode = JSON.parse(vs).mode || '';
        return LANG.test(mode) && mode !== 'plaintext' ? mode : '';
      } catch {
        return '';
      }
    }
    if (html) return MONO.test(html) && !/<(p|li|h[1-6]|table|pre)\b/i.test(html) ? '' : null;
    return text.split('\n').filter((l) => /^(\t| {2,})\S/.test(l)).length >= 2 ? '' : null;
  }

  async function copyText(text) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.style.cssText = 'position:fixed;opacity:0;pointer-events:none';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      ta.remove();
    }
  }

  // 鼠标移到代码块上时右上角浮出「复制 / 折叠」
  function codeTools(ed, { toast, fold }) {
    const bar = document.createElement('div');
    bar.className = 'code-tools keep-card';
    bar.hidden = true;
    const btn = (icon) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.innerHTML = `${svg(ICON[icon])}<span></span>`;
      bar.appendChild(b);
      return b;
    };
    const copyBtn = btn('copy');
    const foldBtn = btn('fold');
    document.body.appendChild(bar);
    let cur = null;

    const hide = () => {
      bar.hidden = true;
      cur = null;
    };
    const place = () => {
      const r = cur.getBoundingClientRect();
      const box = (ed.parentElement || ed).getBoundingClientRect();
      const top = Math.max(r.top, box.top) + 4;
      if (top + 24 > Math.min(r.bottom, box.bottom)) return void (bar.hidden = true);
      bar.hidden = false;
      bar.style.top = `${top}px`;
      bar.style.left = `${Math.min(r.right, box.right) - bar.offsetWidth - 4}px`;
    };
    const update = () => {
      copyBtn.lastChild.textContent = tr('copy');
      foldBtn.hidden = !fold || lineCount(cur) <= FOLD_LINES;
      foldBtn.lastChild.textContent = cur.classList.contains('fold') ? tr('codeUnfold') : tr('codeFold');
    };

    document.addEventListener('mouseover', (e) => {
      if (bar.contains(e.target)) return;
      const pre = e.target.closest?.('pre');
      if (!pre || !ed.contains(pre)) return hide();
      cur = pre;
      update();
      place();
    });
    document.addEventListener('scroll', hide, true);
    ed.addEventListener('input', hide);
    bar.addEventListener('mousedown', (e) => e.preventDefault());
    copyBtn.addEventListener('click', () => {
      if (!cur) return;
      copyText(cur.innerText.replace(/\n$/, '')).then(() => toast(tr('copied')));
    });
    foldBtn.addEventListener('click', () => {
      if (!cur) return;
      setFold(cur, !cur.classList.contains('fold'));
      update();
      place();
    });
  }

  /* ---------- 图片查看器：滚轮缩放、拖动、←/→ 切换、Esc 关闭 ---------- */

  const Viewer = (() => {
    let root, img, count, openBtn, prevBtn, nextBtn;
    let list = [];
    let idx = 0;
    let s = 1, tx = 0, ty = 0, fit = 1;
    let drag = null;

    const apply = () => (img.style.transform = `translate(${tx}px, ${ty}px) scale(${s})`);

    function reset() {
      const nw = img.naturalWidth;
      const nh = img.naturalHeight;
      if (!nw || !nh) return;
      const vw = root.clientWidth;
      const vh = root.clientHeight;
      fit = Math.min(1, (vw - 48) / nw, (vh - 96) / nh);
      s = fit;
      tx = (vw - nw * s) / 2;
      ty = (vh - nh * s) / 2;
      apply();
    }

    function zoomAt(px, py, next) {
      const ns = Math.min(8, Math.max(fit * 0.5, next));
      tx = px - ((px - tx) * ns) / s;
      ty = py - ((py - ty) * ns) / s;
      s = ns;
      apply();
    }

    function show(i) {
      idx = (i + list.length) % list.length;
      const it = list[idx];
      img.onload = reset;
      img.src = it.src;
      if (img.complete && img.naturalWidth) reset();
      count.textContent = list.length > 1 ? `${idx + 1} / ${list.length}` : '';
      openBtn.hidden = !it.rel;
      prevBtn.hidden = nextBtn.hidden = list.length < 2;
    }

    function close() {
      root.hidden = true;
      img.removeAttribute('src');
    }

    function build() {
      root = document.createElement('div');
      root.className = 'viewer keep-card';
      root.hidden = true;
      root.innerHTML =
        `<img class="viewer-img" alt="" draggable="false">` +
        `<button class="viewer-nav prev" data-act="prev">${svg(ICON.prev)}</button>` +
        `<button class="viewer-nav next" data-act="next">${svg(ICON.next)}</button>` +
        `<div class="viewer-bar"><span class="viewer-count"></span>` +
        `<button class="viewer-btn" data-act="open">${svg(ICON.open)}</button>` +
        `<button class="viewer-btn" data-act="close">${svg(ICON.x)}</button></div>`;
      document.body.appendChild(root);
      img = root.querySelector('img');
      count = root.querySelector('.viewer-count');
      openBtn = root.querySelector('[data-act="open"]');
      prevBtn = root.querySelector('[data-act="prev"]');
      nextBtn = root.querySelector('[data-act="next"]');

      root.addEventListener('click', (e) => {
        const act = e.target.closest?.('button')?.dataset.act;
        if (act === 'close') close();
        else if (act === 'prev') show(idx - 1);
        else if (act === 'next') show(idx + 1);
        else if (act === 'open') call('open_note_image', { rel: list[idx].rel }).catch(() => {});
      });
      root.addEventListener('wheel', (e) => {
        e.preventDefault();
        zoomAt(e.clientX, e.clientY, s * Math.exp(-e.deltaY * 0.0015));
      }, { passive: false });
      root.addEventListener('pointerdown', (e) => {
        if (e.button !== 0 || e.target.closest('button')) return;
        drag = { x: e.clientX, y: e.clientY, tx, ty, moved: false, onImg: e.target === img };
        root.setPointerCapture(e.pointerId);
      });
      root.addEventListener('pointermove', (e) => {
        if (!drag) return;
        const dx = e.clientX - drag.x;
        const dy = e.clientY - drag.y;
        if (Math.abs(dx) + Math.abs(dy) > 3) {
          drag.moved = true;
          root.classList.add('dragging');
        }
        tx = drag.tx + dx;
        ty = drag.ty + dy;
        apply();
      });
      const end = () => {
        const d = drag;
        drag = null;
        root.classList.remove('dragging');
        if (d && !d.moved && !d.onImg) close(); // 点图片外的空白处关闭
      };
      root.addEventListener('pointerup', end);
      root.addEventListener('pointercancel', () => {
        drag = null;
        root.classList.remove('dragging');
      });
      // 双击：适应窗口 ⇄ 原始大小
      img.addEventListener('dblclick', (e) => {
        if (Math.abs(s - fit) > 1e-3) reset();
        else zoomAt(e.clientX, e.clientY, fit < 1 ? 1 : fit * 2);
      });
      window.addEventListener(
        'keydown',
        (e) => {
          if (root.hidden) return;
          e.preventDefault();
          e.stopImmediatePropagation();
          if (e.key === 'Escape') close();
          else if (e.key === 'ArrowLeft') show(idx - 1);
          else if (e.key === 'ArrowRight') show(idx + 1);
          else if (e.key === '0') reset();
        },
        true,
      );
      window.addEventListener('resize', () => !root.hidden && reset());
    }

    function open(imgs, i) {
      if (!root) build();
      list = imgs.map((n) => ({ src: n.src, rel: n.dataset.src || '' }));
      openBtn.title = tr('viewerOpen');
      prevBtn.title = tr('viewerPrev');
      nextBtn.title = tr('viewerNext');
      root.querySelector('[data-act="close"]').title = tr('close');
      root.hidden = false;
      show(i);
    }

    return { open };
  })();

  /* ---------- 行首快捷格式 ---------- */

  // 行首输入 - / 1. / # 再按空格：列表或标题；``` 再按空格或回车：代码块
  function autoFormat(e, ed) {
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
    const code = /^```([\w+#.-]{0,20})$/.exec(s);
    let cmd;
    let val;
    if (code) [cmd, val] = ['formatBlock', 'pre'];
    else if (e.key !== ' ') return;
    else if (/^[-*+]$/.test(s)) cmd = 'insertUnorderedList';
    else if (/^1[.)]$/.test(s)) cmd = 'insertOrderedList';
    else if (/^#{1,3}$/.test(s)) [cmd, val] = ['formatBlock', `h${s.length}`];
    else return;
    e.preventDefault();
    sel.removeAllRanges();
    sel.addRange(head);
    document.execCommand('delete');
    document.execCommand(cmd, false, val);
    if (!code) return;
    const pre = caretPre(ed);
    if (!pre) return;
    if (code[1]) pre.dataset.lang = code[1];
    ensureLineAfter(pre);
  }

  /**
   * 给编辑器挂上交互。hooks：
   * - session()：当前编辑会话（无则返回 null，编辑器不响应）
   * - ensureKey(session)：确保有笔记编号并返回它（插图时才需要）
   * - changed()：内容变了（调用方负责保存）
   * - toast(msg, isErr)
   * - fold：长代码块是否折叠（小窗用）
   */
  function attach(ed, { session, ensureKey, changed, toast, fold = false }) {
    document.execCommand('defaultParagraphSeparator', false, 'div');

    async function insertImages(files) {
      const s = session();
      if (!s) return;
      const sel = getSelection();
      const range = sel.rangeCount && ed.contains(sel.anchorNode) ? sel.getRangeAt(0).cloneRange() : null;
      try {
        const key = await ensureKey(s);
        let html = '';
        for (const f of files) {
          const { ext, bytes } = await imageBytes(f);
          const rel = await call('save_note_image', bytes, { headers: { 'x-note-key': key, 'x-ext': ext } });
          html += `<img src="${noteSrc(rel)}" data-src="${rel}" alt="">`;
        }
        if (session() !== s) return;
        ed.focus();
        if (range) {
          sel.removeAllRanges();
          sel.addRange(range);
        } else {
          placeCaret(ed, true);
        }
        document.execCommand('insertHTML', false, html);
      } catch (e) {
        toast(tErr(e), true);
      }
    }

    ed.addEventListener('input', () => {
      if (!session()) return;
      ed.querySelectorAll('.ph').forEach((n) => n.classList.remove('ph'));
      changed();
    });

    ed.addEventListener('keydown', (e) => {
      if (e.isComposing) return;
      const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
      const pre = caretPre(ed);
      if (pre) {
        if (pre.classList.contains('fold')) setFold(pre, false);
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          exitPre(pre);
        } else if (e.key === 'Enter' && plain && !e.shiftKey) {
          e.preventDefault();
          newlineInPre(pre);
        } else if (e.key === 'Tab' && plain) {
          e.preventDefault();
          if (!e.shiftKey) document.execCommand('insertText', false, '    ');
        }
        return;
      }
      if ((e.key === ' ' || e.key === 'Enter') && plain && !e.shiftKey) autoFormat(e, ed);
    });

    ed.addEventListener('paste', (e) => {
      const cd = e.clipboardData;
      if (!cd) return;
      e.preventDefault();
      const files = imageFiles([...cd.items].filter((i) => i.kind === 'file').map((i) => i.getAsFile()));
      if (files.length) return void insertImages(files);
      const text = cd.getData('text/plain').replace(/\r\n?/g, '\n');
      // 代码块里：原样粘贴纯文本，保留缩进
      if (caretPre(ed)) return void document.execCommand('insertText', false, text);
      const html = cd.getData('text/html');
      const lang = codeLang(cd, html, text.replace(/\n+$/, ''));
      if (lang !== null) return void insertCode(ed, text.replace(/\n+$/, ''), lang);
      if (!html) return void document.execCommand('insertText', false, text);
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
      if (e.target.nodeName === 'IMG') {
        const imgs = [...ed.querySelectorAll('img')];
        return void Viewer.open(imgs, imgs.indexOf(e.target));
      }
      const a = e.target.closest?.('a');
      if (!a) return;
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) call('open_link', { url: a.getAttribute('href') }).catch((err) => toast(tErr(err), true));
    });
    ed.addEventListener('mouseover', (e) => {
      const a = e.target.closest?.('a');
      if (a) a.title = a.getAttribute('href');
    });

    // 点进折叠的代码块，或光标在里面开始按键：展开（程序放置光标时不展开）
    ed.addEventListener('mousedown', (e) => {
      const pre = e.target.closest?.('pre.fold');
      if (pre) setFold(pre, false);
    });

    codeTools(ed, { toast, fold });
  }

  return { attach, decorate, noteSrc, placeCaret, placeholderLine, insertStamp, dropEmptyStamp };
})();
