import { easeInOut, mix } from './math';

export type Rect = [number, number, number, number];
export type Free = 'all' | 'left' | 'right';

export const WINDOW: Rect = [0, 0, 1280, 800];
export const WIDE_SCREEN = 980;

export interface Shot {
  focus: Rect;
  free: Free;
  rx?: number;
  ry?: number;
  rz?: number;
  zoom?: number;
  dx?: number;
  dy?: number;
  fade?: number;
}

export interface Key {
  at: number;
  shot: Shot;
}

export interface Pose {
  x: number;
  y: number;
  scale: number;
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  rz: number;
  fade: number;
}

export function freeArea(free: Free, width: number, height: number): Rect {
  const top = 72;
  if (width < WIDE_SCREEN) {
    const sheet = Math.min(height * 0.46, 330);
    return [8, top - 8, width - 16, height - top - sheet];
  }
  const margin = 24;
  const inner: Rect = [margin, top, width - margin * 2, height - top - margin];
  if (free === 'all') return inner;
  const caption = Math.max(380, Math.min(width * 0.36, 620));
  return free === 'right' ? [caption, top, width - caption - margin, inner[3]] : [margin, top, width - caption - margin, inner[3]];
}

export function pose(shot: Shot, width: number, height: number): Pose {
  const [x, y, w, h] = shot.focus;
  const [ax, ay, aw, ah] = freeArea(shot.free, width, height);
  const zoom = shot.zoom ?? 1;
  const scale = Math.min(aw / w, ah / h, 2.1) * zoom;
  const cx = x + w / 2;
  const cy = y + h / 2;
  return {
    x: ax + aw / 2 - cx * scale + (shot.dx ?? 0) * width,
    y: ay + ah / 2 - cy * scale + (shot.dy ?? 0) * height,
    scale,
    cx,
    cy,
    rx: shot.rx ?? 0,
    ry: shot.ry ?? 0,
    rz: shot.rz ?? 0,
    fade: shot.fade ?? 1,
  };
}

export function blend(a: Pose, b: Pose, amount: number): Pose {
  return {
    x: mix(a.x, b.x, amount),
    y: mix(a.y, b.y, amount),
    scale: a.scale * (b.scale / a.scale) ** amount,
    cx: mix(a.cx, b.cx, amount),
    cy: mix(a.cy, b.cy, amount),
    rx: mix(a.rx, b.rx, amount),
    ry: mix(a.ry, b.ry, amount),
    rz: mix(a.rz, b.rz, amount),
    fade: mix(a.fade, b.fade, amount),
  };
}

export interface PlacedKey {
  at: number;
  pose: Pose;
}

export function placeKeys(keys: Key[], width: number, height: number): PlacedKey[] {
  return [...keys].sort((a, b) => a.at - b.at).map((key) => ({ at: key.at, pose: pose(key.shot, width, height) }));
}

export function poseAt(keys: PlacedKey[], at: number): Pose {
  const first = keys[0];
  const last = keys[keys.length - 1];
  if (!first || !last) throw new Error('a track needs keys');
  if (at <= first.at) return { ...first.pose };
  if (at >= last.at) return { ...last.pose };
  let low = 0;
  let high = keys.length - 1;
  while (high - low > 1) {
    const middle = (low + high) >> 1;
    if ((keys[middle] as PlacedKey).at <= at) low = middle;
    else high = middle;
  }
  const a = keys[low] as PlacedKey;
  const b = keys[high] as PlacedKey;
  return blend(a.pose, b.pose, easeInOut((at - a.at) / (b.at - a.at)));
}

export function transformOf(p: Pose): string {
  const rotate = p.rx !== 0 || p.ry !== 0 || p.rz !== 0;
  const turn = rotate
    ? ` translate(${p.cx.toFixed(1)}px,${p.cy.toFixed(1)}px) rotateX(${p.rx.toFixed(2)}deg) rotateY(${p.ry.toFixed(2)}deg) rotateZ(${p.rz.toFixed(2)}deg) translate(${(-p.cx).toFixed(1)}px,${(-p.cy).toFixed(1)}px)`
    : '';
  return `translate3d(${p.x.toFixed(2)}px,${p.y.toFixed(2)}px,0) scale(${p.scale.toFixed(4)})${turn}`;
}

export interface PaneRects {
  a: Rect;
  b: Rect;
  c: Rect;
  s: Rect;
}

export function mixRect(from: Rect, to: Rect, amount: number): Rect {
  return [mix(from[0], to[0], amount), mix(from[1], to[1], amount), mix(from[2], to[2], amount), mix(from[3], to[3], amount)];
}

export function mixPanes(from: PaneRects, to: PaneRects, amount: number): PaneRects {
  return {
    a: mixRect(from.a, to.a, amount),
    b: mixRect(from.b, to.b, amount),
    c: mixRect(from.c, to.c, amount),
    s: mixRect(from.s, to.s, amount),
  };
}

export function toPixels(rect: Rect, width: number, height: number, gap: number): Rect {
  const x = rect[0] * width + (rect[0] > 0 ? gap / 2 : 0);
  const y = rect[1] * height + (rect[1] > 0 ? gap / 2 : 0);
  const right = (rect[0] + rect[2]) * width - (rect[0] + rect[2] < 1 ? gap / 2 : 0);
  const bottom = (rect[1] + rect[3]) * height - (rect[1] + rect[3] < 1 ? gap / 2 : 0);
  return [x, y, Math.max(0, right - x), Math.max(0, bottom - y)];
}
