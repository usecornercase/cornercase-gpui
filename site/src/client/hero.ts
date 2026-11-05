import { all, one, paint } from './paint';
import { backOut, clamp, damp, easeExpo, mix, span } from '../lib/math';

const REST = 112;
const NEAR = 125;
const ENTERED = 1.6;

interface Letter {
  el: HTMLElement;
  x: number;
  y: number;
  bump: number;
}

export class Hero {
  private readonly letters: Letter[];
  private readonly title: HTMLElement;
  private readonly rises: HTMLElement[];
  private readonly hint: HTMLElement;
  private readonly glow: HTMLElement;
  private shownAt = -1;
  private pointerX = -9999;
  private pointerY = -9999;

  constructor() {
    this.title = one('[data-hero-title]');
    this.rises = all('[data-hero-rise]');
    this.hint = one('[data-hero-scroll]');
    this.glow = one('[data-hero-glow]');
    this.letters = all('[data-ch]').map((el) => ({ el, x: 0, y: 0, bump: 0 }));
  }

  measure(): void {
    for (const letter of this.letters) {
      const box = letter.el.getBoundingClientRect();
      letter.x = box.left + box.width / 2;
      letter.y = box.top + box.height / 2;
    }
  }

  pointer(x: number, y: number): void {
    this.pointerX = x;
    this.pointerY = y;
  }

  show(time: number): void {
    this.shownAt = time;
    document.documentElement.classList.remove('intro');
    this.measure();
  }

  update(time: number, dt: number, progress: number): void {
    paint(this.title, 'visibility', progress >= 1 ? 'hidden' : 'visible');
    if (this.shownAt < 0) return;
    this.fadeRest(progress, time - this.shownAt);
    if (progress >= 1) return;
    const since = time - this.shownAt;
    const count = this.letters.length;
    const squeeze = span(progress, 0.02, 0.62);
    this.letters.forEach((letter, i) => this.letter(letter, i, count, since, squeeze, time, dt));
  }

  private letter(letter: Letter, i: number, count: number, since: number, squeeze: number, time: number, dt: number): void {
    const local = clamp((since - i * 0.024) / 1.05);
    const rise = easeExpo(local);
    const dx = letter.x - this.pointerX;
    const dy = letter.y - this.pointerY;
    const close = Math.exp(-(dx * dx + dy * dy) / (2 * 150 * 150));
    letter.bump = damp(letter.bump, close, 10, dt);
    const wave = Math.sin(time * 0.9 - i * 0.34) * 2.6;
    const stretch = mix(62, REST + wave + (NEAR - REST) * letter.bump, backOut(local));
    const squeezed = mix(stretch, 62, squeeze);
    const weight = 800 + 100 * letter.bump;
    const y = (1 - rise) * 62 - squeeze * 80 * (1 + (i % 3) * 0.14);
    const spread = (i - count / 2) * squeeze * 6;
    const opacity = clamp(local * 2.4) * (1 - span(squeeze, 0.45, 0.98));
    paint(letter.el, 'font-stretch', `${clamp(squeezed, 62, 125).toFixed(1)}%`);
    paint(letter.el, 'font-weight', weight.toFixed(0));
    paint(letter.el, 'transform', `translate3d(${spread.toFixed(1)}px,${y.toFixed(1)}%,0)`);
    paint(letter.el, 'opacity', opacity.toFixed(3));
  }

  private fadeRest(progress: number, since: number): void {
    const out = span(progress, 0, 0.3);
    paint(this.glow, 'translate', `0 ${(-progress * 40).toFixed(1)}vh`);
    paint(this.glow, 'scale', (1 + progress * 0.4).toFixed(3));
    for (const rise of this.rises) {
      if (since > ENTERED) paint(rise, 'transition', 'none');
      paint(rise, 'opacity', (1 - out).toFixed(3));
      paint(rise, 'translate', `0 ${(-out * 46).toFixed(1)}px`);
    }
    paint(this.hint, 'opacity', progress > 0.02 ? '0' : '1');
  }
}
