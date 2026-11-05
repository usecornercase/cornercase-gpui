const styles = new WeakMap<HTMLElement | SVGElement, Map<string, string>>();
const classes = new WeakMap<Element, Map<string, boolean>>();

export function paint(el: HTMLElement | SVGElement | null, prop: string, value: string): void {
  if (!el) return;
  let seen = styles.get(el);
  if (!seen) {
    seen = new Map();
    styles.set(el, seen);
  }
  if (seen.get(prop) === value) return;
  seen.set(prop, value);
  el.style.setProperty(prop, value);
}

export function flag(el: Element | null, name: string, on: boolean): void {
  if (!el) return;
  let seen = classes.get(el);
  if (!seen) {
    seen = new Map();
    classes.set(el, seen);
  }
  if (seen.get(name) === on) return;
  seen.set(name, on);
  el.classList.toggle(name, on);
}

export function text(el: Element | null, value: string): void {
  if (el && el.textContent !== value) el.textContent = value;
}

export function one<T extends Element = HTMLElement>(selector: string, root: ParentNode = document): T {
  const found = root.querySelector<T>(selector);
  if (!found) throw new Error(`nothing matches ${selector}`);
  return found;
}

export function all<T extends Element = HTMLElement>(selector: string, root: ParentNode = document): T[] {
  return [...root.querySelectorAll<T>(selector)];
}

export function offsetWithin(el: HTMLElement, root: HTMLElement): { x: number; y: number; w: number; h: number } {
  let x = 0;
  let y = 0;
  let at: HTMLElement | null = el;
  while (at && at !== root) {
    x += at.offsetLeft;
    y += at.offsetTop;
    at = at.offsetParent as HTMLElement | null;
  }
  return { x, y, w: el.offsetWidth, h: el.offsetHeight };
}

export function badge(el: Element | null, status: 'working' | 'waiting' | 'failed' | 'done' | 'idle'): void {
  if (!el) return;
  for (const name of ['working', 'waiting', 'failed', 'done', 'idle']) flag(el, `is-${name}`, name === status);
}
