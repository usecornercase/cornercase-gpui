import { COBALT } from './field/field';
import type { Colour, Field } from './field/field';
import type { Life } from './field/life';
import { all, badge, flag, one, paint, text } from './paint';
import type { Scroll } from './scroll';
import { beat, clamp, easeInOut, easeOut, mix, smooth, span } from '../lib/math';
import { keysFor } from '../lib/os';
import type { Os } from '../lib/os';

const GREEN: Colour = [0.2, 0.92, 0.55];
const ORANGE: Colour = [1.0, 0.45, 0.0];

const VIEW_DISTANCE = 1100;

function veil(p: number, from: number, to: number, leave: number, gone: number): number {
  return beat(p, from, to) * (1 - beat(p, leave, gone));
}

export class Sheets {
  private readonly scroll: Scroll;
  private readonly life: Life;
  private readonly head = one('[data-sheets-head]');
  private readonly ring = one('[data-ring]');
  private readonly cards = all('[data-sheet]');
  private readonly now = one('.sheets__now');
  private readonly name = one('[data-sheet-name]');
  private readonly line = one('[data-sheet-line]');
  private readonly caps = one('[data-sheet-caps]');
  private current = -1;
  private radius = 1000;
  private cardWidth = 800;
  private pressedAt = 0;
  private readonly keysets: string[][];

  constructor(scroll: Scroll, life: Life, os: Os) {
    this.scroll = scroll;
    this.life = life;
    for (const picture of all<HTMLImageElement>('img', this.ring)) {
      picture.loading = 'eager';
      void picture.decode().catch(() => undefined);
    }
    this.keysets = this.cards.map((card) => {
      const shown = card.querySelector<HTMLElement>('[data-keys]');
      const keys = shown?.dataset.keys?.split(' ') ?? [];
      return keysFor(keys, os, shown?.dataset.usage !== undefined);
    });
  }

  measure(): void {
    const { width, height } = this.scroll;
    this.cardWidth = Math.min(width * (width < 980 ? 0.86 : 0.56), height * 0.92, 1000);
    const cardHeight = this.cardWidth * (688 / 1050);
    const count = this.cards.length;
    const step = (Math.PI * 2) / count;
    this.radius = ((this.cardWidth * 1.08) / 2) / Math.tan(step / 2);
    for (const card of this.cards) {
      paint(card, '--w', `${this.cardWidth.toFixed(0)}px`);
      paint(card, '--h', `${cardHeight.toFixed(0)}px`);
    }
  }

  update(time: number): void {
    const p = this.scroll.progress('sheets');
    const seen = p > 0 && p < 1;
    paint(this.ring, 'display', seen ? 'block' : 'none');
    paint(this.head, 'visibility', seen ? 'visible' : 'hidden');
    paint(this.now, 'visibility', seen ? 'visible' : 'hidden');
    if (!seen) return;
    const shown = veil(p, 0.02, 0.1, 0.92, 0.99);
    paint(this.head, 'opacity', veil(p, 0.03, 0.1, 0.9, 0.97).toFixed(3));
    paint(this.head, 'transform', `translate3d(0,${((1 - easeOut(span(p, 0.03, 0.12))) * 30).toFixed(1)}px,0)`);
    paint(this.ring, 'opacity', shown.toFixed(3));
    paint(this.now, 'opacity', veil(p, 0.08, 0.14, 0.9, 0.97).toFixed(3));
    const count = this.cards.length;
    const travel = span(p, 0.1, 0.92) * (count - 1);
    const whole = Math.floor(travel);
    const part = travel - whole;
    const place = whole + easeInOut(clamp((part - 0.48) / 0.52));
    const step = 360 / count;
    const enter = (1 - easeOut(span(p, 0.02, 0.13))) * 1400;
    const exit = easeInOut(span(p, 0.92, 1)) * 900;
    const wobble = Math.sin(time * 0.6) * 0.5;
    const depth = this.radius + this.cardWidth * 0.02 - enter + exit;
    this.placeCards(place, step, depth, wobble);
    this.announce(Math.min(count - 1, Math.round(place)), time);
  }

  private placeCards(place: number, step: number, depth: number, wobble: number): void {
    const near = VIEW_DISTANCE * 0.82;
    this.cards.forEach((card, i) => {
      const turn = (i - place) * step + wobble;
      const nearness = depth - this.radius * Math.cos((turn * Math.PI) / 180);
      const shown = nearness < near && Math.abs(turn) < 100;
      paint(card, 'visibility', shown ? 'visible' : 'hidden');
      if (!shown) return;
      paint(card, 'transform', `perspective(${VIEW_DISTANCE}px) translateZ(${depth.toFixed(1)}px) rotateX(${(wobble * 0.6).toFixed(2)}deg) rotateY(${turn.toFixed(3)}deg) translateZ(${(-this.radius).toFixed(1)}px)`);
    });
  }

  private announce(index: number, time: number): void {
    const hold = time - this.pressedAt;
    if (index !== this.current) {
      this.current = index;
      this.pressedAt = time;
      const card = this.cards[index];
      text(this.name, card?.dataset.name ?? '');
      text(this.line, card?.dataset.line ?? '');
      this.caps.replaceChildren(
        ...(this.keysets[index] ?? []).map((key) => {
          const cap = document.createElement('span');
          cap.className = key.length > 1 ? 'cap-key cap-key--wide' : 'cap-key';
          cap.textContent = key;
          return cap;
        }),
      );
      this.life.flood(0.5, 0.52, COBALT, 300);
      return;
    }
    const down = hold > 0.05 && hold < 0.3;
    for (const cap of this.caps.children) flag(cap, 'is-down', down);
  }
}

export class Close {
  private readonly scroll: Scroll;
  private readonly life: Life;
  private readonly head = one('[data-close-head]');
  private readonly agents = one('[data-agents]');
  private readonly pin = one('.close__pin');
  private readonly clock = one('[data-close-clock]');
  private readonly rows: { bar: HTMLElement; badge: HTMLElement; from: number; to: number; end: 'done' | 'waiting' }[];
  private before = 0;

  constructor(scroll: Scroll, life: Life) {
    this.scroll = scroll;
    this.life = life;
    const row = (key: string, from: number, to: number, end: 'done' | 'waiting') => ({
      bar: one(`.js-agent-bar-${key}`),
      badge: one(`.js-agent-badge-${key}`),
      from,
      to,
      end,
    });
    this.rows = [row('dark', 0.3, 0.62, 'done'), row('gift', 0.3, 0.52, 'done'), row('empty', 0.3, 0.58, 'waiting'), row('pages', 0.3, 0.68, 'done')];
  }

  update(): void {
    const p = this.scroll.progress('close');
    const seen = p > 0 && p < 1;
    paint(this.head, 'visibility', seen ? 'visible' : 'hidden');
    paint(this.agents, 'visibility', seen ? 'visible' : 'hidden');
    if (!seen) {
      this.before = p;
      return;
    }
    const headOn = veil(p, 0.0, 0.05, 0.92, 0.985);
    paint(this.head, 'opacity', headOn.toFixed(3));
    paint(this.pin, '--scrim', headOn.toFixed(3));
    const wide = this.scroll.width >= 980 ? '-50%' : '0px';
    paint(this.head, 'transform', `translate3d(0,calc(${wide} + ${((1 - easeOut(span(p, 0, 0.07))) * 40).toFixed(1)}px),0)`);
    const shown = veil(p, 0.3, 0.37, 0.71, 0.77);
    paint(this.agents, 'opacity', shown.toFixed(3));
    paint(this.agents, 'translate', `0 ${((1 - easeOut(span(p, 0.3, 0.4))) * 40).toFixed(1)}px`);
    for (const row of this.rows) {
      const local = span(p, row.from, row.to);
      paint(row.bar, 'transform', `scaleX(${smooth(local).toFixed(3)})`);
      badge(row.badge, local >= 1 ? row.end : 'working');
    }
    const seconds = Math.round(span(p, 0.34, 0.7) * 258);
    text(this.clock, `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`);
    if (this.before < 0.31 && p >= 0.31) this.life.flood(0.5, 0.42, COBALT, 1400);
    if (this.before < 0.62 && p >= 0.62) this.life.flood(0.5, 0.42, GREEN, 900);
    this.before = p;
  }
}

export class Remote {
  private readonly scroll: Scroll;
  private readonly life: Life;
  private readonly head = one('[data-remote-head]');
  private readonly letters = all('[data-cmd-ch]');
  private readonly caret = one('.remote__caret');
  private readonly link = one('[data-link]');
  private readonly fill = one('[data-link-fill]');
  private readonly cut = one('[data-link-cut]');
  private readonly state = one('[data-link-state]');
  private readonly packets = [one('[data-link-a]'), one('[data-link-b]'), one('[data-link-c]')];
  private before = 0;

  constructor(scroll: Scroll, life: Life) {
    this.scroll = scroll;
    this.life = life;
  }

  update(time: number): void {
    const p = this.scroll.progress('remote');
    paint(this.head, 'opacity', veil(p, 0.02, 0.1, 0.94, 1).toFixed(3));
    paint(this.head, 'transform', `translate3d(0,${((1 - easeOut(span(p, 0.02, 0.12))) * 34).toFixed(1)}px,0)`);
    const typed = span(p, 0.1, 0.36) * this.letters.length;
    this.letters.forEach((letter, i) => paint(letter, 'opacity', i < typed ? '1' : '0'));
    const blink = Math.sin(time * 6) > -0.2;
    paint(this.caret, 'opacity', p > 0.08 && p < 0.97 && (p < 0.36 || blink) ? '1' : '0');
    paint(this.link, 'opacity', veil(p, 0.34, 0.42, 0.96, 1).toFixed(3));
    const growing = easeInOut(span(p, 0.38, 0.5));
    const lost = p >= 0.66 && p < 0.82;
    paint(this.fill, 'transform', `scaleX(${(lost ? 0.5 : growing).toFixed(3)})`);
    paint(this.fill, 'opacity', lost ? '0.35' : '1');
    paint(this.cut, 'opacity', lost ? '1' : '0');
    const live = p >= 0.5 && !lost;
    this.packets.forEach((packet, i) => {
      const run = (time * 0.42 + i / 3) % 1;
      paint(packet, 'left', `${(run * 100).toFixed(2)}%`);
      paint(packet, 'opacity', live ? Math.sin(run * Math.PI).toFixed(3) : '0');
    });
    text(this.state, lost ? 'the link dropped · reconnecting…' : p >= 0.5 ? 'connected to devbox' : 'connecting…');
    flag(this.state, 'is-lost', lost);
    flag(this.state, 'is-live', live);
    if (this.before < 0.66 && p >= 0.66) this.life.flood(0.5, 0.36, ORANGE, 520);
    if (this.before < 0.82 && p >= 0.82) this.life.flood(0.5, 0.36, GREEN, 620);
    this.before = p;
  }
}

export class Finale {
  private readonly scroll: Scroll;
  private readonly field: Field;
  private readonly mark: HTMLElement;
  private shown = false;

  constructor(scroll: Scroll, field: Field) {
    this.scroll = scroll;
    this.field = field;
    this.mark = one('.download__mark');
  }

  update(): number {
    const start = this.scroll.at('download', 0) - this.scroll.height;
    const p = span(this.scroll.now, start + this.scroll.height * 0.45, start + this.scroll.height * 1.05);
    if (p <= 0) {
      if (this.shown) this.field.markOn = 0;
      this.shown = false;
      return 0;
    }
    this.shown = true;
    const box = this.mark.getBoundingClientRect();
    const lvh = this.field.lvh;
    this.field.markAt = [(box.left + box.width / 2) / this.scroll.width, 1 - (box.top + box.height / 2) / lvh];
    this.field.markSize = box.width;
    this.field.markOn = easeOut(p);
    this.field.markFray = mix(0.5, 0.06, p);
    return p;
  }
}
