(() => {
  if (window.__cornercase) return;
  const send = (kind, data) => {
    try {
      window.cornercaseBridge(JSON.stringify(Object.assign({ kind }, data || {})));
    } catch (_) {}
  };
  const typed = 'input:not([type=button]):not([type=submit]):not([type=reset]):not([type=checkbox]):not([type=radio]):not([type=range]):not([type=color]):not([type=file]), textarea';
  const hasText = (el) => Array.from(el.childNodes).some((n) => n.nodeType === 3 && n.textContent.trim() !== '');
  const cursorOf = (el) => {
    if (!(el instanceof Element)) return 'default';
    const css = getComputedStyle(el).cursor;
    if (css && css !== 'auto') return css;
    if (el.isContentEditable || el.matches(typed)) return 'text';
    return hasText(el) ? 'text' : 'default';
  };
  let cursor = '';
  addEventListener(
    'mousemove',
    (e) => {
      const next = cursorOf(e.target);
      if (next !== cursor) {
        cursor = next;
        send('cursor', { cursor: next });
      }
    },
    { capture: true, passive: true },
  );
  window.__cornercase = { send, typed };
  const clip = (value, most) => {
    const text = String(value == null ? '' : value).replace(/\s+/g, ' ').trim();
    return text.length > most ? text.slice(0, most - 1) + '…' : text;
  };
  const quoted = (value) => '"' + value.replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"';
  const escaped = (value) => (window.CSS && CSS.escape ? CSS.escape(value) : value.replace(/[^\w-]/g, '\\$&'));
  const one = (root, selector) => {
    try {
      return root.querySelectorAll(selector).length === 1;
    } catch (_) {
      return false;
    }
  };
  const testIds = ['data-testid', 'data-test', 'data-cy', 'data-qa'];
  const plainClass = (name) =>
    name.length <= 30 && !/[:[\]/@!]/.test(name) && !/^(css|sc|jsx|emotion|svelte|chakra)-/.test(name) && !/\d{3,}/.test(name);
  const anchorOf = (el, root) => {
    for (const name of testIds) {
      const value = el.getAttribute(name);
      const selector = value && `[${name}=${quoted(value)}]`;
      if (selector && one(root, selector)) return selector;
    }
    const id = el.id && '#' + escaped(el.id);
    return id && !/\d{4,}/.test(el.id) && one(root, id) ? id : null;
  };
  const stepOf = (el) => {
    const twins = el.parentElement ? Array.from(el.parentElement.children).filter((s) => s.localName === el.localName) : [el];
    const classes = Array.from(el.classList).filter(plainClass).slice(0, 2).map((c) => '.' + escaped(c)).join('');
    return el.localName + classes + (twins.length > 1 ? `:nth-of-type(${twins.indexOf(el) + 1})` : '');
  };
  const selectorOf = (el) => {
    const root = el.getRootNode();
    const parts = [];
    for (let node = el; node && node.nodeType === 1; node = node.parentElement) {
      const anchor = anchorOf(node, root);
      parts.unshift(anchor || stepOf(node));
      const selector = parts.join(' > ');
      if (anchor || one(root, selector)) return selector;
    }
    return parts.join(' > ');
  };
  const shortFile = (file) =>
    String(file || '')
      .replace(/^[a-z]+:\/\/[^/]+\//i, '')
      .replace(/[?#].*$/, '');
  const fromStack = (stack) => {
    const lines = String(stack || '').split('\n').slice(1);
    const frame = lines.find((l) => !/node_modules|react-dom|react\.|jsx-dev-runtime|chunk-|\.vite\/deps/.test(l) && /:\d+:\d+/.test(l));
    const found = frame && frame.match(/(?:\(|at |@)([^()\s]+):(\d+):(\d+)\)?\s*$/);
    return found ? { file: shortFile(found[1]), line: Number(found[2]), approximate: true } : null;
  };
  const typeName = (type) => {
    if (typeof type === 'function') return type.displayName || type.name || '';
    if (!type || typeof type !== 'object') return '';
    const inner = type.render || type.type || {};
    return type.displayName || inner.displayName || inner.name || '';
  };
  const reactOf = (el) => {
    const key = Object.keys(el).find((k) => k.startsWith('__reactFiber$') || k.startsWith('__reactInternalInstance$'));
    if (!key) return null;
    const owners = [];
    const host = el[key];
    let source = host._debugSource ? { file: shortFile(host._debugSource.fileName), line: host._debugSource.lineNumber } : null;
    source = source || fromStack(host._debugStack && host._debugStack.stack);
    for (let fiber = host.return; fiber && owners.length < 5; fiber = fiber.return) {
      const name = typeName(fiber.type);
      if (name && /^[A-Z]/.test(name)) owners.push(name);
    }
    return { framework: 'React', name: owners[0] || '', file: source && source.file, line: source && source.line, approximate: !!(source && source.approximate), owners: owners.reverse() };
  };
  const vueName = (instance) => {
    const type = instance.type || instance.$options || {};
    const file = type.__file || '';
    return type.name || type.__name || (file.split('/').pop() || '').replace(/\.vue$/, '');
  };
  const vueOf = (el) => {
    const instance = el.__vueParentComponent || (el.__vue__ && { type: el.__vue__.$options, parent: el.__vue__.$parent && { type: el.__vue__.$parent.$options } });
    if (!instance) return null;
    const owners = [];
    for (let node = instance; node && owners.length < 5; node = node.parent) {
      const name = vueName(node);
      if (name) owners.push(name);
    }
    const type = instance.type || {};
    return { framework: 'Vue', name: vueName(instance), file: shortFile(type.__file), line: null, approximate: false, owners: owners.reverse() };
  };
  const svelteOf = (el) => {
    const meta = el.__svelte_meta;
    const loc = meta && meta.loc;
    if (!loc) return null;
    const [file, line] = Array.isArray(loc) ? loc : [loc.file, loc.line];
    const name = (String(file || '').split('/').pop() || '').replace(/\.svelte$/, '');
    return { framework: 'Svelte', name, file: shortFile(file), line: typeof line === 'number' ? line + 1 : null, approximate: false, owners: name ? [name] : [] };
  };
  const componentOf = (el) => {
    for (let node = el, depth = 0; node && node.nodeType === 1 && depth < 12; node = node.parentElement, depth++) {
      const found = reactOf(node) || vueOf(node) || svelteOf(node);
      if (found) return found;
    }
    return null;
  };
  const keyAttributes = ['href', 'src', 'alt', 'title', 'aria-label', 'role', 'name', 'type', 'placeholder', 'for', 'action', ...testIds];
  const styleNames = ['color', 'background-color', 'font', 'display', 'position', 'margin', 'padding'];
  const topOf = (win) => {
    let x = 0;
    let y = 0;
    try {
      for (let w = win; w !== w.top && w.frameElement; w = w.parent) {
        const box = w.frameElement.getBoundingClientRect();
        x += box.left + w.frameElement.clientLeft;
        y += box.top + w.frameElement.clientTop;
      }
      return { x, y, top: win.top, url: win.top.location.href };
    } catch (_) {
      return { x, y, top: win, url: win.location.href };
    }
  };
  const describe = (el) => {
    const win = el.ownerDocument.defaultView || window;
    const box = el.getBoundingClientRect();
    const place = topOf(win);
    const view = place.top.visualViewport || { pageLeft: place.top.scrollX, pageTop: place.top.scrollY, width: place.top.innerWidth, height: place.top.innerHeight };
    const css = getComputedStyle(el);
    return {
      url: place.url,
      tag: el.localName,
      selector: selectorOf(el),
      id: el.id || '',
      classes: Array.from(el.classList).slice(0, 12),
      text: clip(el.innerText || el.textContent, 200),
      attributes: keyAttributes.filter((a) => el.hasAttribute(a)).map((a) => [a, clip(el.getAttribute(a), 200)]),
      html: clip(el.outerHTML, 1500),
      rect: { x: box.left + place.x, y: box.top + place.y, width: box.width, height: box.height },
      page: { x: view.pageLeft, y: view.pageTop, width: view.width, height: view.height },
      styles: styleNames.map((n) => [n, clip(css.getPropertyValue(n), 120)]).filter(([, v]) => v !== ''),
      component: componentOf(el),
    };
  };
  window.__cornercase.describe = describe;
})();
(() => {
  const cc = window.__cornercase;
  if (!cc || cc.choose) return;
  const send = cc.send;
  const deepActive = () => {
    let el = document.activeElement;
    while (el && el.shadowRoot && el.shadowRoot.activeElement) el = el.shadowRoot.activeElement;
    return el;
  };
  const inner = (el) => {
    try {
      return el && el.tagName === 'IFRAME' ? el.contentWindow.__cornercase : null;
    } catch (_) {
      return null;
    }
  };
  const isField = (el) => !!el && el.matches(cc.typed) && typeof el.selectionStart === 'number';
  const selected = () => {
    const el = deepActive();
    const frame = inner(el);
    if (frame) return frame.selected();
    if (isField(el)) return el.type === 'password' ? '' : el.value.slice(el.selectionStart, el.selectionEnd);
    return String(getSelection());
  };
  const cut = () => {
    const el = deepActive();
    const frame = inner(el);
    if (frame) return frame.cut();
    const text = selected();
    if (text && (isField(el) || (el && el.isContentEditable))) document.execCommand('delete');
    return text;
  };
  const offset = () => {
    let x = 0;
    let y = 0;
    try {
      for (let w = window; w.frameElement; w = w.parent) {
        const r = w.frameElement.getBoundingClientRect();
        x += r.left + w.frameElement.clientLeft;
        y += r.top + w.frameElement.clientTop;
      }
    } catch (_) {}
    return [x, y];
  };
  const placed = (x, y, w, h) => {
    const [ox, oy] = offset();
    return { x: x + ox, y: y + oy, w, h };
  };
  const measure = document.createElement('canvas').getContext('2d');
  const fieldCaret = (el) => {
    const css = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const line = parseFloat(css.lineHeight) || parseFloat(css.fontSize) * 1.2;
    const before = el.value.slice(0, el.selectionEnd || 0);
    const lines = el.tagName === 'TEXTAREA' ? before.split('\n') : [before];
    measure.font = css.font;
    const left = r.left + el.clientLeft + parseFloat(css.paddingLeft);
    const x = left + measure.measureText(lines[lines.length - 1]).width - el.scrollLeft;
    const top = r.top + el.clientTop + parseFloat(css.paddingTop);
    const y = el.tagName === 'TEXTAREA' ? top + (lines.length - 1) * line - el.scrollTop : r.top + (r.height - line) / 2;
    return placed(Math.min(x, r.right), y, 1, line);
  };
  const rangeCaret = () => {
    const sel = getSelection();
    if (!sel || !sel.rangeCount) return null;
    const range = sel.getRangeAt(0).cloneRange();
    range.collapse(false);
    const rects = range.getClientRects();
    const r = rects.length ? rects[rects.length - 1] : sel.focusNode && sel.focusNode.parentElement && sel.focusNode.parentElement.getBoundingClientRect();
    return r ? placed(r.left, r.top, 1, r.height || 16) : null;
  };
  const caretOf = (el) => {
    if (!el || inner(el) || el.tagName === 'IFRAME') return undefined;
    if (isField(el)) return fieldCaret(el);
    return el.isContentEditable ? rangeCaret() : null;
  };
  let last = '';
  let queued = false;
  const report = () => {
    queued = false;
    const caret = caretOf(deepActive());
    if (caret === undefined) return;
    const next = JSON.stringify(caret || {});
    if (next !== last) {
      last = next;
      send('caret', caret || {});
    }
  };
  const later = () => {
    if (!queued) {
      queued = true;
      requestAnimationFrame(report);
    }
  };
  for (const type of ['selectionchange', 'focusin', 'focusout', 'input', 'scroll']) {
    addEventListener(type, later, { capture: true, passive: true });
  }
  let picking = null;
  const single = (el) => el instanceof HTMLSelectElement && !el.multiple && el.size <= 1 && !el.disabled;
  const option = (o) => {
    const group = o.parentElement && o.parentElement.tagName === 'OPTGROUP' ? o.parentElement : null;
    return { label: o.label || o.text, disabled: o.disabled || !!(group && group.disabled), group: group ? group.label : '' };
  };
  const pick = (el) => {
    picking = el;
    const r = el.getBoundingClientRect();
    const at = placed(r.left, r.top, r.width, r.height);
    send('select', Object.assign(at, { options: Array.from(el.options).map(option), selected: el.selectedIndex }));
  };
  addEventListener(
    'mousedown',
    (e) => {
      const el = e.composedPath().find((n) => n instanceof HTMLSelectElement);
      if (e.button !== 0 || !single(el)) return;
      e.preventDefault();
      el.focus();
      pick(el);
    },
    true,
  );
  addEventListener(
    'keydown',
    (e) => {
      const el = deepActive();
      if (!single(el) || e.altKey || e.metaKey || e.ctrlKey || ![' ', 'ArrowDown', 'ArrowUp'].includes(e.key)) return;
      e.preventDefault();
      pick(el);
    },
    true,
  );
  const choose = (index) => {
    const el = picking;
    picking = null;
    if (!el || index < 0 || index === el.selectedIndex) return;
    el.selectedIndex = index;
    el.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
  Object.assign(cc, { selected, cut, choose });
})();
