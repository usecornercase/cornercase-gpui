import { COBALT } from '../field/field';
import type { Colour } from '../field/field';
import type { Life } from '../field/life';
import { backOut, beat, easeOut, mix, smooth, span } from '../../lib/math';
import { mixPanes, placeKeys, poseAt, toPixels, transformOf } from '../../lib/track';
import type { PaneRects, PlacedKey, Rect } from '../../lib/track';
import { all, badge, flag, offsetWithin, one, paint, text } from '../paint';
import type { Scroll } from '../scroll';
import { cameraKeys } from './camera';

const PANES_WIDTH = 948;
const PANES_HEIGHT = 728;
const DOCK_RIGHT = 1272;
const NARROW = 420;
const WIDE = 640;

const YELLOW: Colour = [0.96, 0.78, 0.28];
const ORANGE: Colour = [1.0, 0.45, 0.0];
const RED: Colour = [1.0, 0.22, 0.38];
const GREEN: Colour = [0.2, 0.92, 0.55];

type Point = [number, number];
type Status = 'working' | 'waiting' | 'failed' | 'done' | 'idle';

const START: PaneRects = {
  a: [0, 0, 0.56, 1],
  b: [0.56, 0, 0.44, 0.6],
  c: [0.56, 0.6, 0.44, 0.4],
  s: [1, 0, 0, 0.4],
};

const STACKED: PaneRects = {
  a: [0, 0, 1, 0.66],
  b: [1, 0, 0, 0.66],
  c: [0, 0.66, 1, 0.34],
  s: [1, 0.66, 0, 0.34],
};

function pathAt(p: number, points: [number, number, number][]): Point {
  const first = points[0] as [number, number, number];
  if (p <= first[0]) return [first[1], first[2]];
  for (let i = 1; i < points.length; i += 1) {
    const a = points[i - 1] as [number, number, number];
    const b = points[i] as [number, number, number];
    if (p <= b[0]) {
      const t = smooth((p - a[0]) / (b[0] - a[0]));
      return [mix(a[1], b[1], t), mix(a[2], b[2], t)];
    }
  }
  const last = points[points.length - 1] as [number, number, number];
  return [last[1], last[2]];
}

function crossed(before: number, now: number, mark: number): boolean {
  return before < mark && now >= mark;
}

export class Stage {
  private readonly scroll: Scroll;
  private readonly life: Life;
  private keys: PlacedKey[] = [];

  private readonly scene = one('[data-scene]');
  private readonly rig = one('[data-rig]');
  private readonly win = one('[data-win]');
  private readonly dock = one('[data-dock]');
  private readonly layerList = all('[data-layers] li');
  private readonly statusList = all('[data-statuses] li');
  private readonly beatsList = all('[data-preview-beats] li');
  private readonly lifted: Record<string, HTMLElement> = {};

  private readonly panes = {
    a: one('.js-pane-agent'),
    b: one('.js-pane-code'),
    c: one('.js-pane-tests'),
    s: one('.js-pane-shell'),
  };
  private readonly gap = one('.js-gap');
  private readonly drop = one('.js-drop');

  private readonly agentTerm = one('.js-agent-lines');
  private readonly agentLines = all('.js-agent-lines .w-line');
  private readonly orchTerm = one('.js-orch-lines');
  private readonly orchLines = all('.js-orch-lines .w-line');
  private readonly agentFoot = one('.w-agentfoot');
  private readonly spin = one('.js-spin');
  private readonly spinTime = one('.js-spin-time');
  private readonly handed = one('.js-handed');
  private streamed = 2;
  private streamClock = 0;

  private readonly ptr = one('.js-ptr');
  private readonly tap = one('.js-ptr-tap');
  private readonly flyer = one('.js-flyer');

  private readonly toast = one('.js-toast');
  private readonly darkBadge = one('.js-dark-badge');
  private readonly giftBadge = one('.js-gift-badge');
  private readonly emptyBadge = one('.js-empty-badge');
  private readonly prDark = one('.js-pr-dark');
  private readonly prGift = one('.js-pr-gift');
  private readonly tabDark = one('.js-tab-dark');
  private readonly tabEmpty = one('.js-tab-empty');
  private readonly statusPath = one('.js-status-path');

  private readonly pathCode = one('.js-path-code');
  private readonly pathUrl = one('.js-path-url');
  private readonly pathImage = one('.js-path-image');
  private readonly previewCode = one('.js-preview-code');
  private readonly previewUrl = one('.js-preview-url');
  private readonly previewImage = one('.js-preview-image');

  private readonly pages = {
    changes: one('.js-page-changes'),
    files: one('.js-page-files'),
    todo: one('.js-page-todo'),
    browser: one('.js-page-browser'),
    flow: one('.js-page-flow'),
  };
  private readonly rail = {
    changes: one('.js-rail-git-compare'),
    files: one('.js-rail-folder-tree'),
    todo: one('.js-rail-list-checks'),
    browser: one('.js-rail-globe'),
    flow: one('.js-rail-waypoints'),
  };
  private readonly diffLines = all('.w-dl');
  private readonly checks = all('.js-check-1, .js-check-2, .js-check-3');
  private readonly viewedCount = one('.js-viewed-count');
  private readonly stageBtn = one('.js-stage');
  private readonly askBtn = one('.js-ask');
  private readonly fileTheme = one('.js-file-theme');
  private readonly todoSend = one('.js-todo-send');
  private readonly todoGo = one('.js-todo-go');

  private readonly site = one('.js-site');
  private readonly reloaded = one('.js-reloaded');
  private readonly device = one('.js-device');
  private readonly toolDevice = one('.js-tool-device');
  private readonly toolPick = one('.js-tool-pick');
  private readonly banner = one('.js-pickbanner');
  private readonly pickbox = one('.js-pickbox');
  private readonly pay = one('.js-pay');
  private readonly picked = one('.js-picked');
  private readonly pickedText = one('.js-picked-text');
  private readonly pickedSend = one('.js-picked-send');

  private readonly wires = {
    b: one<SVGPathElement>('.js-wire-b'),
    c: one<SVGPathElement>('.js-wire-c'),
    d: one<SVGPathElement>('.js-wire-d'),
    e: one<SVGPathElement>('.js-wire-e'),
    f: one<SVGPathElement>('.js-wire-f'),
  };
  private readonly loop = one<SVGPathElement>('.js-loop');
  private readonly spark = one<SVGCircleElement>('.js-spark');
  private readonly roles = {
    orchestrator: one('.js-role-orchestrator'),
    coder: one('.js-role-coder'),
    reviewer: one('.js-role-reviewer'),
    security: one('.js-role-security'),
    tester: one('.js-role-tester'),
  };
  private readonly end = one('.js-end');
  private readonly railFill = one('.js-rail-fill');
  private readonly stages = [one('.js-stage-1'), one('.js-stage-2'), one('.js-stage-3')];
  private readonly stageWord = one('.js-stage-word');
  private readonly flowCount = one('.js-flow-count');
  private readonly flowPill = one('.js-flow-pill');
  private readonly flowState = one('.js-flow-state');
  private readonly loopLabel = one('.js-loop-label');
  private readonly loopCount = one('.js-loop-count');

  entrance = 1;
  private dockLift = 0;
  private previous = { status: 0, layers: 0, browser: 0, flow: 0 };
  private spots = { code: { x: 0, y: 0 }, url: { x: 0, y: 0 }, image: { x: 0, y: 0 } };

  constructor(scroll: Scroll, life: Life) {
    this.scroll = scroll;
    this.life = life;
    for (const el of all('[data-layer]', this.win)) this.lifted[el.dataset.layer ?? ''] = el;
  }

  measure(): void {
    this.keys = placeKeys(cameraKeys(this.scroll), this.scroll.width, this.scroll.height);
    const hidden = this.scene.style.display === 'none';
    if (hidden) this.scene.style.display = 'block';
    this.settleSpots();
    if (hidden) this.scene.style.display = 'none';
  }

  private settleSpots(): void {
    const spot = (el: HTMLElement) => {
      const box = offsetWithin(el, this.win);
      return { x: box.x, y: box.y };
    };
    const shown = this.agentLines.map((line) => line.classList.contains('is-off'));
    this.agentLines.forEach((line) => line.classList.remove('is-off'));
    this.spots = { code: spot(this.pathCode), url: spot(this.pathUrl), image: spot(this.pathImage) };
    this.agentLines.forEach((line, i) => line.classList.toggle('is-off', shown[i] ?? false));
  }

  update(time: number, dt: number): void {
    const s = this.scroll;
    const seen = s.now < s.at('sheets', 0.12) || (s.now > s.at('sheets', 0.95) && s.now < s.at('close', 1));
    paint(this.scene, 'display', seen ? 'block' : 'none');
    if (!seen) return;
    const hero = s.progress('hero');
    const layers = s.progress('layers');
    const status = s.progress('status');
    const panes = s.progress('panes');
    const preview = s.progress('preview');
    const review = s.progress('review');
    const browser = s.progress('browser');
    const flow = s.progress('flow');
    const close = s.progress('close');

    this.camera(s.now);
    this.explode(layers);
    this.dockState(layers, review, browser, flow, close);
    const rect = this.paneRects(panes, review, close);
    this.layoutPanes(rect, panes);
    this.agent(time, dt, hero, status, browser, flow, close);
    this.statuses(status, browser, flow, close);
    this.preview(preview);
    this.reviewing(review);
    this.browsing(browser);
    this.flowing(flow);
    this.pointer(panes, preview, review, browser, close, rect);
    this.previous = { status, layers, browser, flow };
  }

  private camera(at: number): void {
    const p = poseAt(this.keys, at);
    if (this.entrance < 1) p.y += (1 - easeOut(this.entrance)) * this.scroll.height * 0.48;
    paint(this.rig, 'transform', transformOf(p));
    paint(this.rig, 'opacity', p.fade.toFixed(3));
  }

  private explode(p: number): void {
    const on = p > 0.03 && p < 0.985;
    flag(this.win, 'is-layered', on);
    const rail = backOut(span(p, 0.2, 0.3));
    const side = backOut(span(p, 0.3, 0.4));
    const panes = backOut(span(p, 0.4, 0.5));
    const dock = backOut(span(p, 0.5, 0.6));
    const landing = 1 - beat(p, 0.8, 0.95);
    const gap = this.scroll.width < 980 ? 120 : 92;
    const lift = (name: string, amount: number, level: number) => {
      const el = this.lifted[name];
      paint(el ?? null, 'transform', `translate3d(0,0,${(amount * landing * gap * level).toFixed(1)}px)`);
    };
    lift('rail', rail, 1);
    lift('side', side, 1.8);
    lift('panes', panes, 2.6);
    lift('title', rail * 0.3, 1);
    lift('status', rail * 0.3, 1);
    this.dockLift = dock * landing * gap * 3.5;
    const named = [p > 0.1 && p < 0.2, p >= 0.2 && p < 0.3, p >= 0.3 && p < 0.4, p >= 0.4 && p < 0.5, p >= 0.5 && p < 0.9];
    this.layerList.forEach((li, i) => flag(li, 'is-on', named[i] ?? false));
    if (crossed(this.previous.layers, p, 0.94)) this.life.flood(0.5, 0.5, COBALT, 1100);
  }

  private dockState(layers: number, review: number, browser: number, flow: number, close: number): void {
    const inLayers = beat(layers, 0.5, 0.56) * (1 - beat(layers, 0.84, 0.94));
    const open = beat(review, 0.02, 0.12) * (1 - beat(close, 0.0, 0.03));
    const wide = beat(browser, 0.0, 0.07);
    const width = mix(NARROW, WIDE, wide);
    const shown = Math.max(inLayers, open);
    paint(this.dock, 'width', `${width.toFixed(1)}px`);
    paint(this.dock, 'opacity', shown.toFixed(3));
    const slide = (1 - open) * (1 - inLayers) * 40;
    paint(this.dock, 'transform', `translate3d(${slide.toFixed(1)}px,0,${this.dockLift.toFixed(1)}px)`);
    paint(this.win, '--dock', `${((width + 8) * open).toFixed(1)}px`);
    paint(this.dock, 'visibility', shown > 0.002 ? 'visible' : 'hidden');
    const page = flow > 0.001 ? 'flow' : browser > 0.001 ? 'browser' : review > 0.76 ? 'todo' : review > 0.52 ? 'files' : 'changes';
    for (const name of ['changes', 'files', 'todo', 'browser', 'flow'] as const) {
      flag(this.pages[name], 'is-on', name === page);
      flag(this.rail[name], 'is-on', name === page && open > 0.5);
    }
  }

  private paneRects(p: number, review: number, close: number): PaneRects {
    const drag = beat(p, 0.1, 0.2) * (1 - beat(p, 0.22, 0.32));
    const settle = beat(p, 0.22, 0.32);
    const g = mix(0.56, 0.44, drag) + settle * 0.04;
    const r = 1 - g;
    const swap = beat(p, 0.52, 0.6);
    const split = beat(p, 0.76, 0.86);
    const rects: PaneRects = {
      a: [0, 0, g, 1],
      b: [g, mix(0, 0.4, swap), r, 0.6],
      c: [g, mix(0.6, 0, swap), r * (1 - split * 0.5), 0.4],
      s: [g + r * (1 - split * 0.5), 0, r * split * 0.5, 0.4],
    };
    const stacked = mixPanes(rects, STACKED, beat(review, 0.02, 0.12));
    return mixPanes(stacked, START, beat(close, 0, 0.03));
  }

  private layoutPanes(rect: PaneRects, p: number): void {
    const held = p >= 0.36 && p < 0.53;
    const hold = pathAt(p, [
      [0.36, 0, 0],
      [0.42, 0, 8],
      [0.5, -6, 300],
    ]);
    const entries: [keyof PaneRects, HTMLElement][] = [
      ['a', this.panes.a],
      ['b', this.panes.b],
      ['c', this.panes.c],
      ['s', this.panes.s],
    ];
    const dockWidth = Number.parseFloat(this.win.style.getPropertyValue('--dock')) || 0;
    const width = PANES_WIDTH - dockWidth;
    for (const [name, el] of entries) {
      const [x, y, w, h] = toPixels(rect[name] as Rect, width, PANES_HEIGHT, 8);
      const moved = name === 'b' && held ? hold : [0, 0];
      paint(el, 'left', `${x.toFixed(1)}px`);
      paint(el, 'top', `${y.toFixed(1)}px`);
      paint(el, 'width', `${w.toFixed(1)}px`);
      paint(el, 'height', `${h.toFixed(1)}px`);
      paint(el, 'visibility', w < 14 || h < 14 ? 'hidden' : 'visible');
      paint(el, 'transform', moved[0] === 0 && moved[1] === 0 ? 'none' : `translate3d(${moved[0]}px,${moved[1]}px,0) scale(1.015)`);
    }
    flag(this.panes.b, 'is-held', held);
    flag(this.panes.b, 'show-handle', p >= 0.32 && p < 0.53);
    paint(this.panes.b, 'z-index', held ? '3' : '1');
    flag(this.panes.c, 'is-focused', p >= 0.64 && p < 0.9);
    flag(this.panes.a, 'is-focused', p < 0.64 || p >= 0.9);
    const [gx, , , gh] = toPixels(rect.a, width, PANES_HEIGHT, 8);
    const gapOn = p >= 0.08 && p < 0.33;
    paint(this.gap, 'left', `${(gx + rect.a[2] * width - 1).toFixed(1)}px`);
    paint(this.gap, 'top', '0');
    paint(this.gap, 'width', '3px');
    paint(this.gap, 'height', `${gh.toFixed(0)}px`);
    paint(this.gap, 'opacity', gapOn ? '1' : '0');
    const dropOn = p >= 0.42 && p < 0.53;
    const [cx, cy, cw, ch] = toPixels(rect.c, width, PANES_HEIGHT, 8);
    paint(this.drop, 'left', `${cx.toFixed(1)}px`);
    paint(this.drop, 'top', `${cy.toFixed(1)}px`);
    paint(this.drop, 'width', `${cw.toFixed(1)}px`);
    paint(this.drop, 'height', `${ch.toFixed(1)}px`);
    paint(this.drop, 'z-index', '2');
    paint(this.drop, 'opacity', dropOn ? '1' : '0');
  }

  private agent(time: number, dt: number, hero: number, status: number, browser: number, flow: number, close: number): void {
    this.streamClock += dt;
    if (this.streamClock > 0.34 && this.streamed < this.agentLines.length) {
      this.streamClock = 0;
      this.streamed += 1;
    }
    if (hero > 0.98) this.streamed = this.agentLines.length;
    const count = this.streamed;
    this.agentLines.forEach((line, i) => flag(line, 'is-off', i >= count));
    const ownsFlow = flow > 0.001 && close <= 0.001;
    paint(this.agentTerm, 'visibility', ownsFlow ? 'hidden' : 'visible');
    paint(this.orchTerm, 'visibility', ownsFlow ? 'visible' : 'hidden');
    const handedOn = browser >= 0.8 && !ownsFlow && close <= 0.001;
    flag(this.handed, 'is-on', handedOn);
    const working = this.darkWorking(status, browser);
    paint(this.spin, 'display', working && !ownsFlow ? 'block' : 'none');
    text(this.spinTime, String(19 + Math.floor(time) % 240));
    const paneHeight = this.panes.a.offsetHeight;
    const lines = ownsFlow ? this.orchVisible(flow) : count;
    const footHeight = (working ? 19 : 0) + (handedOn ? 42 : 0) + 56;
    const content = lines * 17;
    const shift = Math.min(0, paneHeight - 12 - content - footHeight - 8);
    paint(this.agentTerm, 'transform', `translate3d(0,${shift.toFixed(0)}px,0)`);
    paint(this.orchTerm, 'transform', `translate3d(0,${shift.toFixed(0)}px,0)`);
    paint(this.agentFoot, 'transform', `translate3d(0,${(12 + content + shift + 8).toFixed(0)}px,0)`);
  }

  private darkWorking(status: number, browser: number): boolean {
    return status < 0.74 || (browser > 0 && browser < 0.11) || (browser >= 0.8 && browser < 0.91);
  }

  private orchVisible(flow: number): number {
    const beats = [0, 0.02, 0.02, 0.08, 0.22, 0.3, 0.42, 0.5, 0.6, 0.7, 0.82, 0.88];
    let count = 0;
    this.orchLines.forEach((line, i) => {
      const on = flow >= (beats[i] ?? 1);
      flag(line, 'is-off', !on);
      if (on) count += 1;
    });
    return count;
  }

  private statuses(p: number, browser: number, flow: number, close: number): void {
    const waiting = p >= 0.27;
    const failed = p >= 0.5;
    const done = p >= 0.74;
    const gift: Status = close > 0.62 ? 'done' : failed && close <= 0.001 ? 'failed' : 'working';
    badge(this.darkBadge, this.darkWorking(p, browser) ? 'working' : 'done');
    badge(this.giftBadge, gift);
    badge(this.emptyBadge, flow > 0.88 ? 'done' : flow > 0.001 ? 'working' : waiting ? 'waiting' : 'working');
    flag(this.toast, 'is-on', p > 0.3 && p < 0.47);
    flag(this.prDark, 'w-pr--done', done);
    flag(this.prDark, 'w-pr--pending', !done);
    text(this.prDark, done ? '#483✓' : '#483•');
    text(this.prGift, failed ? '#485✕' : '#485•');
    flag(this.prGift, 'w-pr--failed', failed);
    flag(this.prGift, 'w-pr--pending', !failed);
    const onFlow = flow > 0.001 && close <= 0.001;
    flag(this.tabDark, 'is-lit', !onFlow);
    flag(this.tabEmpty, 'is-lit', onFlow);
    text(this.statusPath, onFlow ? 'orchestrator · ~/.cornercase/worktrees/web-shop/fix-empty-address' : 'claude · ~/.cornercase/worktrees/web-shop/feat-dark-mode');
    const beats = [p > 0 && p < 0.27, p >= 0.27 && p < 0.5, p >= 0.5 && p < 0.74, p >= 0.74];
    this.statusList.forEach((li, i) => flag(li, 'is-on', beats[i] ?? false));
    const before = this.previous.status;
    if (crossed(before, p, 0.27)) this.life.flood(0.26, 0.42, ORANGE);
    if (crossed(before, p, 0.5)) this.life.flood(0.26, 0.5, RED);
    if (crossed(before, p, 0.74)) this.life.flood(0.26, 0.56, GREEN);
  }

  statusHue(p: number): Colour {
    if (p >= 0.74) return GREEN;
    if (p >= 0.5) return RED;
    if (p >= 0.27) return ORANGE;
    return YELLOW;
  }

  private preview(p: number): void {
    const code = p > 0.16 && p < 0.42;
    const url = p >= 0.46 && p < 0.68;
    const image = p >= 0.72 && p < 0.97;
    flag(this.pathCode, 'is-hover', code);
    flag(this.pathUrl, 'is-hover', url);
    flag(this.pathImage, 'is-hover', image);
    flag(this.previewCode, 'is-on', code);
    flag(this.previewUrl, 'is-on', url);
    flag(this.previewImage, 'is-on', image);
    const place = (el: HTMLElement, x: number, y: number, h: number) => {
      paint(el, 'left', `${x.toFixed(0)}px`);
      paint(el, 'top', `${(y - h - 10).toFixed(0)}px`);
    };
    place(this.previewCode, this.spots.code.x - 60, this.spots.code.y, 134);
    place(this.previewUrl, this.spots.url.x - 40, this.spots.url.y, 312);
    place(this.previewImage, this.spots.image.x - 30, this.spots.image.y, 280);
    const beats = [code, url, image];
    this.beatsList.forEach((li, i) => flag(li, 'is-on', beats[i] ?? false));
  }

  private reviewing(p: number): void {
    this.diffLines.forEach((line, i) => {
      const local = easeOut(span(p, 0.1 + i * 0.008, 0.16 + i * 0.008));
      paint(line, 'opacity', local.toFixed(3));
      paint(line, 'transform', `translate3d(${((1 - local) * 12).toFixed(1)}px,0,0)`);
    });
    const onFirst = p >= 0.33;
    const onSecond = p >= 0.41;
    flag(this.checks[0] ?? null, 'is-on', onFirst);
    flag(this.checks[1] ?? null, 'is-on', onSecond);
    flag(this.stageBtn, 'is-pressed', p > 0.325 && p < 0.34);
    flag(this.askBtn, 'is-on', p > 0.44 && p < 0.5);
    text(this.viewedCount, onSecond ? '2 of 3 viewed' : onFirst ? '1 of 3 viewed' : '0 of 3 viewed');
    flag(this.fileTheme, 'is-lit', p > 0.6);
    const hover = p > 0.82;
    flag(this.todoSend, 'is-hover', hover && p < 0.92);
    flag(this.todoGo, 'is-on', p > 0.88 && p < 0.92);
    flag(this.todoSend, 'is-sent', p >= 0.92);
  }

  private browsing(p: number): void {
    flag(this.site, 'is-light', p < 0.11);
    flag(this.reloaded, 'is-on', (p >= 0.11 && p < 0.2) || (p >= 0.91 && p < 0.99));
    const phone = p >= 0.25 && p < 0.38;
    flag(this.toolDevice, 'is-on', phone);
    flag(this.device, 'is-on', phone);
    paint(this.site, '--page', phone ? '296px' : '100%');
    const picking = p >= 0.42 && p < 0.55;
    flag(this.toolPick, 'is-on', picking);
    flag(this.banner, 'is-on', picking);
    const hovering = p >= 0.49;
    const pickOn = hovering && p < 0.84;
    flag(this.pickbox, 'is-on', pickOn);
    if (pickOn) {
      const box = offsetWithin(this.pay, this.site);
      paint(this.pickbox, 'left', `${box.x}px`);
      paint(this.pickbox, 'top', `${box.y}px`);
      paint(this.pickbox, 'width', `${box.w}px`);
      paint(this.pickbox, 'height', `${box.h}px`);
    }
    flag(this.picked, 'is-on', p >= 0.55 && p < 0.8);
    const words = 'Use the brand colour here';
    const typed = Math.round(span(p, 0.58, 0.72) * words.length);
    text(this.pickedText, words.slice(0, typed));
    flag(this.pickedSend, 'is-pressed', p > 0.775 && p < 0.795);
    const flight = span(p, 0.79, 0.87);
    const on = flight > 0 && flight < 1;
    paint(this.flyer, 'opacity', on ? '1' : '0');
    if (on) {
      const t = smooth(flight);
      const x = mix(1130, 372, t);
      const y = mix(690, 440, t) - Math.sin(t * Math.PI) * 120;
      const scale = 1 + Math.sin(t * Math.PI) * 0.5;
      paint(this.flyer, 'transform', `translate3d(${x.toFixed(1)}px,${y.toFixed(1)}px,0) scale(${scale.toFixed(3)})`);
    }
    flag(this.pay, 'is-brand', p >= 0.91);
    const before = this.previous.browser;
    if (crossed(before, p, 0.87)) this.life.flood(0.28, 0.5, COBALT, 800);
  }

  private flowing(p: number): void {
    const moments = { coder: 0.08, review: 0.22, back: 0.36, again: 0.48, test: 0.64, end: 0.8 };
    const draw = (el: SVGPathElement, from: number, to: number) => paint(el, 'stroke-dashoffset', (1 - span(p, from, to)).toFixed(3));
    draw(this.wires.b, moments.review - 0.06, moments.review);
    draw(this.wires.c, moments.review - 0.06, moments.review);
    draw(this.wires.d, moments.test - 0.06, moments.test);
    draw(this.wires.e, moments.test - 0.06, moments.test);
    draw(this.wires.f, moments.end - 0.05, moments.end);
    paint(this.loop, 'stroke-dashoffset', (1 - span(p, moments.back - 0.05, moments.back)).toFixed(3));
    const onCoder = (p >= moments.coder && p < moments.review) || (p >= moments.back && p < moments.again);
    const onReview = (p >= moments.review && p < moments.back) || (p >= moments.again && p < moments.test);
    const onTester = p >= moments.test && p < moments.end;
    flag(this.roles.orchestrator, 'is-on', p > 0.02 && p < moments.end + 0.1);
    flag(this.roles.coder, 'is-on', onCoder);
    flag(this.roles.coder, 'is-past', p >= moments.review);
    flag(this.roles.reviewer, 'is-on', onReview);
    flag(this.roles.reviewer, 'is-past', p >= moments.test);
    flag(this.roles.security, 'is-on', p >= moments.review && p < moments.test && !(p >= moments.back && p < moments.again));
    flag(this.roles.security, 'is-past', p >= moments.test);
    flag(this.roles.tester, 'is-on', onTester);
    flag(this.roles.tester, 'is-past', p >= moments.end);
    flag(this.end, 'is-on', p >= moments.end);
    const stage = p >= moments.test ? 3 : p >= moments.review ? 2 : p >= moments.coder ? 1 : 0;
    this.stages.forEach((bar, i) => flag(bar, 'is-on', i < stage));
    text(this.stageWord, p >= moments.end ? 'finished' : stage > 0 ? `stage ${stage} of 3` : 'not started');
    paint(this.railFill, 'transform', `scaleY(${(p >= moments.end ? 1 : stage === 3 ? 0.71 : stage === 2 ? 0.36 : stage === 1 ? 0.1 : 0).toFixed(2)})`);
    flag(this.loopLabel, 'is-on', p >= moments.back);
    text(this.loopCount, p >= moments.back ? '1' : '0');
    const messages = [moments.coder, moments.review - 0.03, moments.review, moments.back, moments.again - 0.03, moments.again, moments.test - 0.03, moments.test, moments.end - 0.03].filter((at) => p >= at).length;
    text(this.flowCount, String(messages));
    flag(this.flowPill, 'is-done', p >= moments.end);
    text(this.flowState, p >= moments.end ? 'finished' : 'running');
    this.sparkle(p, moments);
    const before = this.previous.flow;
    if (crossed(before, p, moments.end)) this.life.flood(0.72, 0.3, COBALT, 1000);
    if (crossed(before, p, moments.back)) this.life.flood(0.72, 0.6, ORANGE, 500);
  }

  private sparkle(p: number, moments: Record<string, number>): void {
    const windows: [SVGPathElement, number, number][] = [
      [this.wires.b, (moments.review ?? 0) - 0.06, moments.review ?? 0],
      [this.loop, (moments.back ?? 0) - 0.05, moments.back ?? 0],
      [this.wires.b, (moments.again ?? 0) - 0.05, moments.again ?? 0],
      [this.wires.d, (moments.test ?? 0) - 0.06, moments.test ?? 0],
      [this.wires.f, (moments.end ?? 0) - 0.05, moments.end ?? 0],
    ];
    const active = windows.find(([, from, to]) => p >= from && p < to);
    paint(this.spark, 'opacity', active ? '1' : '0');
    if (!active) return;
    const [path, from, to] = active;
    const length = path.getTotalLength();
    const point = path.getPointAtLength(length * span(p, from, to));
    this.spark.setAttribute('cx', point.x.toFixed(1));
    this.spark.setAttribute('cy', point.y.toFixed(1));
  }

  private pointer(panes: number, preview: number, review: number, browser: number, close: number, rect: PaneRects): void {
    const place = this.pointerAt(panes, preview, review, browser, close, rect);
    paint(this.ptr, 'opacity', place ? '1' : '0');
    if (!place) return;
    paint(this.ptr, 'transform', `translate3d(${place[0].toFixed(1)}px,${place[1].toFixed(1)}px,0)`);
    flag(this.tap, 'is-on', place[2]);
  }

  private pointerAt(panes: number, preview: number, review: number, browser: number, close: number, rect: PaneRects): [number, number, boolean] | null {
    const ox = 324;
    const oy = 44;
    const dockLeft = DOCK_RIGHT - WIDE;
    if (panes > 0.03 && panes < 0.92) {
      const gapX = ox + rect.a[2] * PANES_WIDTH - 2;
      const bX = ox + (rect.a[2] + rect.b[2] / 2) * PANES_WIDTH;
      const point = pathAt(panes, [
        [0.03, 600, 500],
        [0.09, gapX, 420],
        [0.33, gapX, 420],
        [0.38, bX, oy + 8],
        [0.42, bX, oy + 16],
        [0.5, bX - 6, oy + 330],
        [0.56, bX - 6, oy + 330],
        [0.74, 980, 300],
        [0.86, 980, 300],
      ]);
      const x = panes >= 0.09 && panes < 0.33 ? gapX : point[0];
      return [x, point[1], false];
    }
    if (preview > 0.04 && preview < 0.97) {
      const point = pathAt(preview, [
        [0.04, 900, 640],
        [0.14, this.spots.code.x + 30, this.spots.code.y + 8],
        [0.42, this.spots.code.x + 30, this.spots.code.y + 8],
        [0.46, this.spots.url.x + 60, this.spots.url.y + 8],
        [0.68, this.spots.url.x + 60, this.spots.url.y + 8],
        [0.72, this.spots.image.x + 30, this.spots.image.y + 8],
        [0.97, this.spots.image.x + 30, this.spots.image.y + 8],
      ]);
      return [point[0], point[1], false];
    }
    if (review > 0.2 && review < 0.97) {
      const point = pathAt(review, [
        [0.2, 700, 400],
        [0.31, 1252, 170],
        [0.34, 1252, 170],
        [0.4, 1252, 196],
        [0.44, 1050, 334],
        [0.5, 1050, 334],
        [0.53, 30, 107],
        [0.6, 30, 107],
        [0.68, 900, 216],
        [0.74, 900, 216],
        [0.77, 30, 147],
        [0.8, 30, 147],
        [0.86, 1240, 174],
        [0.97, 1240, 174],
      ]);
      const tap = (review > 0.33 && review < 0.345) || (review > 0.405 && review < 0.42) || (review > 0.52 && review < 0.535) || (review > 0.765 && review < 0.78) || (review > 0.885 && review < 0.9);
      return [point[0], point[1], tap];
    }
    if (browser > 0.2 && browser < 0.82) {
      const point = pathAt(browser, [
        [0.2, 900, 500],
        [0.24, dockLeft + 464, 110],
        [0.38, dockLeft + 464, 110],
        [0.41, dockLeft + 494, 110],
        [0.45, dockLeft + 494, 110],
        [0.49, dockLeft + 320, 430],
        [0.56, dockLeft + 320, 430],
        [0.74, dockLeft + 500, 700],
        [0.82, dockLeft + 560, 722],
      ]);
      const tap = (browser > 0.245 && browser < 0.26) || (browser > 0.415 && browser < 0.43) || (browser > 0.545 && browser < 0.56) || (browser > 0.775 && browser < 0.79);
      return [point[0], point[1], tap];
    }
    if (close > 0.1 && close < 0.3) {
      const point = pathAt(close, [
        [0.1, 700, 500],
        [0.22, 22, 22],
        [0.3, 22, 22],
      ]);
      return [point[0], point[1], close > 0.235 && close < 0.26];
    }
    return null;
  }

}
