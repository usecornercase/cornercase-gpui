import { COBALT, Field } from './field/field';
import { Life } from './field/life';
import { Hero } from './hero';
import { preparePage } from './page';
import { all, one, paint, text } from './paint';
import { Scroll } from './scroll';
import { Stage } from './stage';
import { Close, Finale, Remote, Sheets } from './story';
import { beat, clamp, easeOut, mix, span } from '../lib/math';

const os = preparePage();
(window as unknown as { cornercaseAlive: boolean }).cornercaseAlive = true;

function bootstrap(field: Field): void {
  const scroll = new Scroll();
  const life = new Life(field);
  const hero = new Hero();
  const stage = new Stage(scroll, life);
  const sheets = new Sheets(scroll, life, os);
  const close = new Close(scroll, life);
  const remote = new Remote(scroll, life);
  const finale = new Finale(scroll, field);
  const caps = all('.ch').map((section) => ({
    id: section.dataset.section ?? '',
    pin: one('.ch__pin', section),
    cap: one('[data-cap]', section),
    title: one('[data-title]', section),
    side: section.classList.contains('ch--right') ? 1 : -1,
  }));
  const closeTitle = one('[data-title]', one('.close'));
  let chapterWas = '';
  const fps = one('[data-hud-fps]');
  const canvas = one<HTMLCanvasElement>('[data-field]');
  const root = document.documentElement;

  let lastWidth = 0;
  let lastHeight = 0;
  let quality = 0;
  let scale = Math.min(window.devicePixelRatio || 1, 2);
  const size = () => {
    const width = root.clientWidth;
    const height = canvas.clientHeight || window.innerHeight;
    if (width !== lastWidth || Math.abs(height - lastHeight) > 2) {
      field.resize(width, height, scale);
      lastWidth = width;
      lastHeight = height;
    }
    scroll.measure();
    stage.measure();
    sheets.measure();
    hero.measure();
  };

  const reduceTo = (level: number) => {
    quality = level;
    if (level === 1) field.iterations = 8;
    if (level >= 2) {
      field.iterations = 4;
      scale = 1;
      root.classList.add('lite');
      lastWidth = 0;
      size();
    }
  };

  const mood = () => {
    settleMood();
    if (scroll.width < 980) life.mood.gain *= 0.72;
  };

  const settleMood = () => {
    const m = life.mood;
    m.hue = COBALT;
    m.gain = 1;
    m.glow = 0.62;
    m.emit = 1;
    m.low = 0;
    const heroP = scroll.progress('hero');
    if (heroP < 1) {
      m.low = 1 - heroP;
      m.emit = 1.25;
      return;
    }
    const layersP = scroll.progress('layers');
    if (layersP > 0 && layersP < 1) {
      m.gain = 0.9;
      m.emit = 1;
      return;
    }
    const statusP = scroll.progress('status');
    if (statusP > 0 && statusP < 1) {
      m.hue = stage.statusHue(statusP);
      m.gain = 0.92;
      m.emit = 1.25;
      return;
    }
    const closeP = scroll.progress('close');
    if (closeP > 0 && closeP < 1) {
      const open = beat(closeP, 0.28, 0.36) * (1 - beat(closeP, 0.7, 0.78));
      m.emit = mix(0.7, 1.3, open);
      m.gain = mix(0.55, 0.86, open);
      return;
    }
    if (scroll.progress('sheets') > 0 && scroll.progress('sheets') < 1) {
      m.gain = 0.6;
      m.emit = 0.8;
      return;
    }
    if (scroll.progress('remote') > 0 && scroll.progress('remote') < 1) {
      m.gain = 0.62;
      m.emit = 0.7;
      return;
    }
    if (scroll.progress('remote') >= 1) {
      m.gain = 0.76;
      m.emit = 0.9;
      return;
    }
    m.gain = 0.6;
    m.emit = 0.72;
  };

  const captions = (time: number) => {
    for (const { id, pin, cap, title, side } of caps) {
      const p = scroll.progress(id);
      const on = p > 0 && p < 1;
      paint(cap, 'visibility', on ? 'visible' : 'hidden');
      if (!on) continue;
      const enter = easeOut(span(p, 0.03, 0.14));
      const leave = span(p, 0.9, 0.99);
      const float = Math.sin(time * 0.8) * 3;
      const opacity = enter * (1 - leave);
      const y = (1 - enter) * 44 - leave * 34 + float * opacity;
      const x = (1 - enter) * 26 * side;
      const wide = scroll.width >= 980;
      paint(cap, 'opacity', opacity.toFixed(3));
      paint(pin, '--scrim', opacity.toFixed(3));
      paint(title, 'font-stretch', `${mix(72, 118, easeOut(span(p, 0.02, 0.17))).toFixed(1)}%`);
      if (p > 0.05 && p < 0.1 && chapterWas !== id) {
        chapterWas = id;
        life.flood(side > 0 ? 0.2 : 0.8, 0.5, COBALT, 620);
      }
      paint(cap, 'transform', wide ? `translate3d(${x.toFixed(1)}px,calc(-50% + ${y.toFixed(1)}px),0)` : `translate3d(0,${y.toFixed(1)}px,0)`);
    }
  };

  size();
  addEventListener('resize', size);
  const fontsReady = document.fonts?.ready ?? Promise.resolve();
  void fontsReady.then(() => {
    field.refreshGlyphs();
    size();
  });

  addEventListener(
    'pointermove',
    (event) => {
      life.pointer(event.clientX, event.clientY, root.clientWidth, canvas.clientHeight || innerHeight);
      hero.pointer(event.clientX, event.clientY);
    },
    { passive: true },
  );

  const intro = scroll.now < 60;
  const started = performance.now();
  let burst = !intro;
  let heroShown = intro ? Number.POSITIVE_INFINITY : -10;
  if (intro) {
    const side = Math.min(innerHeight * 0.42, root.clientWidth * 0.62);
    field.markAt = [0.5, 0.54];
    field.markSize = side;
    field.markFray = 0.06;
    field.markOn = 0;
  } else {
    hero.show(0);
    life.awake = true;
  }

  let last = started;
  let simDebt = 0;
  let frames = 0;
  let framesAt = started;
  let slowSpell = 0;

  const tick = (now: number) => {
    const dt = Math.min(0.05, Math.max(0.001, (now - last) / 1000));
    last = now;
    const time = (now - started) / 1000;

    scroll.tick(dt);
    const heroP = scroll.progress('hero');

    if (!burst) {
      field.markOn = clamp(time / 0.55);
      const cursorOn = time > 0.62 && Math.floor((time - 0.62) * 6) % 2 === 1;
      field.markFray = cursorOn ? 0.16 : 0.05;
      if (time > 1.08) {
        burst = true;
        field.stamp();
        field.markOn = 0;
        field.burst(0.5, 0.54, Math.max(1000, field.markSize * 3.4), [0.0, 0.14, 0.4]);
        life.awake = true;
        hero.show(time);
        heroShown = time;
      }
    }

    mood();
    life.tick(dt, time, scroll.velocity, Math.max(1, scroll.height));
    const finaleP = burst ? finale.update() : 0;
    if (finaleP > 0) life.mood.gain = mix(life.mood.gain, 0.66, finaleP);

    hero.update(time, dt, heroP, scroll.now);
    stage.entrance = clamp((time - heroShown) / 1.15);
    stage.update(time, dt);
    captions(time);
    sheets.update(time);
    close.update();
    paint(closeTitle, 'font-stretch', `${mix(72, 122, easeOut(span(scroll.progress('close'), 0, 0.09))).toFixed(1)}%`);
    if (scroll.seen('remote')) remote.update(time);

    simDebt += dt;
    if (simDebt >= 1 / 72 || quality >= 3) {
      field.frame(simDebt, time);
      simDebt = 0;
    }

    frames += 1;
    if (now - framesAt > 500) {
      const rate = (frames * 1000) / (now - framesAt);
      text(fps, String(Math.round(rate)));
      slowSpell = rate < 38 && !document.hidden ? slowSpell + 1 : 0;
      if (slowSpell >= 3 && quality < 2) {
        reduceTo(quality + 1);
        slowSpell = 0;
      }
      frames = 0;
      framesAt = now;
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function fallback(): void {
  document.documentElement.classList.remove('motion', 'intro');
}

if (document.documentElement.classList.contains('motion')) {
  const canvas = one<HTMLCanvasElement>('[data-field]');
  const field = Field.start(canvas);
  if (field) bootstrap(field);
  else fallback();
}
