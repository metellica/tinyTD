// 任务笔记：受限 Markdown 子集 ⇄ 编辑器 HTML（无第三方依赖）
//
// 支持：# / ## / ### 标题、- 无序列表、1. 有序列表、``` 代码块（可带语言）、
//       **粗体**、*斜体*、~~删除线~~、`代码`、[文字](链接)、![](图片)、裸链接
// 编辑器里每一行对应文件里的一行；其他格式（颜色、字号、表格样式等）一律丢弃。
const MD = (() => {
  const ESC_HTML = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ESC_HTML[c]);
  // 笔记自己的图片：<编号>/<文件名>，与后端校验一致
  const NOTE_IMG = /^[0-9a-z]{4,16}\/[0-9a-z-]+\.(png|jpe?g|gif|webp)$/;
  const SAFE_URL = /^(https?:\/\/|mailto:)/i;
  const PUNCT = /[!-/:-@[-`{-~]/;
  const BARE_URL = /^https?:\/\/[^\s<>"'`]*[^\s<>"'`.,;:!?)\]}，。；：！？、）】」』]/;
  const STAMP = /^⏸/;
  const LANG = /^[\w+#.-]{0,20}$/; // 代码块语言标记，如 ```js

  // 找到 from 之后第一个未被反斜杠转义的 token
  function findClose(s, token, from) {
    for (let i = from; i < s.length; i++) {
      if (s[i] === '\\') { i++; continue; }
      if (s.startsWith(token, i)) return i;
    }
    return -1;
  }

  function inline(s, imgSrc) {
    let out = '';
    let i = 0;
    while (i < s.length) {
      const c = s[i];
      if (c === '\\' && PUNCT.test(s[i + 1] || '')) {
        out += esc(s[i + 1]);
        i += 2;
        continue;
      }
      if (c === '`') {
        const j = s.indexOf('`', i + 1);
        if (j > i + 1) {
          out += `<code>${esc(s.slice(i + 1, j))}</code>`;
          i = j + 1;
          continue;
        }
      }
      const pair = s.startsWith('**', i) ? ['**', 'b'] : s.startsWith('~~', i) ? ['~~', 's'] : c === '*' ? ['*', 'i'] : null;
      if (pair) {
        const [tok, tag] = pair;
        const j = findClose(s, tok, i + tok.length);
        if (j > i + tok.length) {
          out += `<${tag}>${inline(s.slice(i + tok.length, j), imgSrc)}</${tag}>`;
          i = j + tok.length;
          continue;
        }
      }
      if (c === '!' || c === '[') {
        const m = /^(!?)\[((?:\\.|[^\]\\])*)\]\(([^)\s]+)\)/.exec(s.slice(i));
        if (m) {
          const [all, bang, label, url] = m;
          if (bang && NOTE_IMG.test(url)) out += `<img src="${esc(imgSrc(url))}" data-src="${esc(url)}" alt="">`;
          else if (SAFE_URL.test(url)) out += `<a href="${esc(url)}">${inline(label, imgSrc) || esc(url)}</a>`;
          else out += inline(label, imgSrc);
          i += all.length;
          continue;
        }
      }
      if (c === 'h' && (i === 0 || /[\s(（]/.test(s[i - 1]))) {
        const m = BARE_URL.exec(s.slice(i));
        if (m) {
          out += `<a href="${esc(m[0])}">${esc(m[0])}</a>`;
          i += m[0].length;
          continue;
        }
      }
      out += esc(c);
      i++;
    }
    return out;
  }

  /** Markdown -> 编辑器 HTML；imgSrc 把 `<编号>/<文件名>` 映射成可显示的 URL */
  function toHtml(md, imgSrc) {
    const lines = md.replace(/\r\n?/g, '\n').replace(/\n+$/, '').split('\n');
    let html = '';
    let list = '';
    const closeList = () => {
      if (list) html += `</${list}>`;
      list = '';
    };
    const openList = (tag) => {
      if (list !== tag) {
        closeList();
        html += `<${tag}>`;
        list = tag;
      }
    };
    for (let k = 0; k < lines.length; k++) {
      const line = lines[k];
      let m;
      if ((m = /^```\s*(\S*)/.exec(line))) {
        closeList();
        const buf = [];
        for (k++; k < lines.length && !/^```\s*$/.test(lines[k]); k++) buf.push(lines[k]);
        const lang = LANG.test(m[1]) ? m[1] : '';
        html += `<pre${lang ? ` data-lang="${esc(lang)}"` : ''}>${esc(buf.join('\n')) || '<br>'}</pre>`;
      } else if ((m = /^(#{1,3})\s+(.*)$/.exec(line))) {
        closeList();
        const n = m[1].length;
        const cls = STAMP.test(m[2]) ? ' class="stamp"' : '';
        html += `<h${n}${cls}>${inline(m[2], imgSrc) || '<br>'}</h${n}>`;
      } else if ((m = /^\s*[-*+]\s+(.*)$/.exec(line))) {
        openList('ul');
        html += `<li>${inline(m[1], imgSrc) || '<br>'}</li>`;
      } else if ((m = /^\s*\d+[.)]\s+(.*)$/.exec(line))) {
        openList('ol');
        html += `<li>${inline(m[1], imgSrc) || '<br>'}</li>`;
      } else {
        closeList();
        html += `<div>${inline(line, imgSrc) || '<br>'}</div>`;
      }
    }
    closeList();
    return html;
  }

  /* ---------- HTML -> Markdown ---------- */

  const SKIP = new Set(['SCRIPT', 'STYLE', 'TITLE', 'META', 'LINK', 'HEAD', 'TEMPLATE', 'NOSCRIPT', 'IFRAME', 'OBJECT', 'SVG']);
  const BLOCK = new Set([
    'DIV', 'P', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'UL', 'OL', 'LI', 'PRE', 'BLOCKQUOTE', 'SECTION', 'ARTICLE',
    'HEADER', 'FOOTER', 'MAIN', 'NAV', 'ASIDE', 'FIGURE', 'FIGCAPTION', 'TABLE', 'THEAD', 'TBODY', 'TFOOT', 'TR',
    'TD', 'TH', 'HR', 'DL', 'DT', 'DD', 'ADDRESS', 'DETAILS', 'SUMMARY', 'BODY',
  ]);

  const escText = (s) => s.replace(/[\\`*~[\]]/g, '\\$&');

  // 行首看起来像块级语法的普通文字要转义，避免下次打开变成标题/列表
  function escLineStart(line) {
    if (/^\s*(#{1,6}\s|[-+]\s)/.test(line)) return line.replace(/^(\s*)([#+-])/, '$1\\$2');
    if (/^\s*\d+[.)]\s/.test(line)) return line.replace(/^(\s*\d+)([.)])/, '$1\\$2');
    return line;
  }

  function wrap(tok, s) {
    if (!s.trim()) return s;
    const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(s);
    return `${m[1]}${tok}${m[2]}${tok}${m[3]}`;
  }

  function inl(n) {
    if (n.nodeType === 3) return escText(n.nodeValue.replace(/\u00a0/g, ' ').replace(/[\r\n]+/g, ' '));
    if (n.nodeType !== 1 || SKIP.has(n.nodeName)) return '';
    const kids = () => [...n.childNodes].map(inl).join('');
    switch (n.nodeName) {
      case 'BR':
        return '\n';
      case 'B':
      case 'STRONG':
        return wrap('**', kids());
      case 'I':
      case 'EM':
        return wrap('*', kids());
      case 'S':
      case 'DEL':
      case 'STRIKE':
        return wrap('~~', kids());
      case 'CODE':
      case 'KBD':
      case 'SAMP': {
        const t = n.textContent.replace(/`/g, '').replace(/\s+/g, ' ');
        return t.trim() ? `\`${t}\`` : t;
      }
      case 'A': {
        const href = (n.getAttribute('href') || '').trim();
        const t = kids();
        if (!SAFE_URL.test(href) || /[\s()]/.test(href)) return t;
        return t === escText(href) || !t.trim() ? href : `[${t.replace(/\n/g, ' ')}](${href})`;
      }
      case 'IMG': {
        const src = n.dataset.src || '';
        if (NOTE_IMG.test(src)) return `![](${src})`;
        const remote = (n.getAttribute('src') || '').trim();
        return SAFE_URL.test(remote) && !/[\s()]/.test(remote) ? `![](${remote})` : '';
      }
      default:
        return kids();
    }
  }

  /** 编辑器 / 粘贴内容的 DOM -> Markdown */
  function fromHtml(root) {
    const lines = [];
    let cur = null; // 当前行内容（行内节点累积）
    const flush = () => {
      if (cur === null) return;
      // contenteditable 里块末尾的 <br> 只是占位，不算新行
      for (const l of cur.replace(/\n$/, '').split('\n')) lines.push(escLineStart(l));
      cur = null;
    };
    const oneLine = (n) => [...n.childNodes].map(inl).join('').replace(/\n+/g, ' ').trim();

    function list(n) {
      let idx = 0;
      for (const li of n.children) {
        if (li.nodeName !== 'LI') continue;
        idx++;
        const nested = [];
        const own = [...li.childNodes]
          .filter((c) => (/^(UL|OL)$/.test(c.nodeName) ? (nested.push(c), false) : true))
          .map(inl)
          .join('')
          .replace(/\n+/g, ' ')
          .trim();
        lines.push(`${n.nodeName === 'OL' ? `${idx}.` : '-'} ${own}`.trimEnd());
        nested.forEach(list);
      }
    }

    function block(node) {
      for (const n of node.childNodes) {
        if (n.nodeType === 3 || (n.nodeType === 1 && !BLOCK.has(n.nodeName) && !SKIP.has(n.nodeName))) {
          cur = (cur ?? '') + inl(n);
          continue;
        }
        if (n.nodeType !== 1 || SKIP.has(n.nodeName)) continue;
        flush();
        const tag = n.nodeName;
        if (/^H[1-6]$/.test(tag)) {
          lines.push(`${'#'.repeat(Math.min(+tag[1], 3))} ${oneLine(n)}`.trimEnd());
        } else if (tag === 'UL' || tag === 'OL') {
          list(n);
        } else if (tag === 'PRE') {
          const text = (n.innerText ?? n.textContent).replace(/\u00a0/g, ' ').replace(/\n$/, '');
          const lang = LANG.test(n.dataset?.lang || '') ? n.dataset.lang || '' : '';
          lines.push(`\`\`\`${lang}`, ...text.split('\n').map((l) => (/^```/.test(l) ? ` ${l}` : l)), '```');
        } else if (tag === 'HR') {
          lines.push('');
        } else {
          block(n);
          flush();
        }
      }
    }

    block(root);
    flush();
    while (lines.length && !lines[0].trim()) lines.shift();
    while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
    return lines.join('\n');
  }

  return { toHtml, fromHtml, NOTE_IMG };
})();
