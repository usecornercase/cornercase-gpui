import { all, one } from './paint';
import { downloadLabel, keysFor, megabytes, osOf } from '../lib/os';
import type { Os } from '../lib/os';
import { DOWNLOAD, FILES } from '../data/links';

function renameKeys(os: Os): void {
  if (os === 'mac' || os === 'phone') return;
  for (const keys of all('[data-keys]')) {
    const names = keysFor(keys.dataset.keys?.split(' ') ?? [], os, keys.dataset.usage !== undefined);
    keys.replaceChildren(
      ...names.map((name) => {
        const kbd = document.createElement('kbd');
        kbd.textContent = name;
        return kbd;
      }),
    );
  }
}

function markDownloads(os: Os): void {
  const label = one('[data-download-label]');
  const link = one<HTMLAnchorElement>('[data-download]');
  label.textContent = downloadLabel(os);
  if (os !== 'phone') link.href = `${DOWNLOAD}/${FILES[os]}`;
  for (const card of all('[data-platform]')) card.classList.toggle('is-yours', card.dataset.platform === os);
}

function copyButton(line: HTMLElement, status: HTMLElement): void {
  const copy = one<HTMLButtonElement>('[data-copy]', line);
  copy.addEventListener('click', async () => {
    status.textContent = '';
    try {
      await navigator.clipboard.writeText(copy.dataset.copy ?? '');
      copy.classList.add('is-done');
      setTimeout(() => copy.classList.remove('is-done'), 1600);
    } catch {
      window.getSelection()?.selectAllChildren(one('code', line));
      status.textContent = 'Your browser did not let the page copy it. The command is selected: copy it from there.';
    }
  });
}

function installBox(): void {
  const box = one('[data-install]');
  const status = one('[data-copy-status]', box);
  const ways = all<HTMLButtonElement>('[data-way]', box);
  const lines = all('[data-line]', box);
  ways.forEach((way, i) => {
    way.addEventListener('click', () => {
      ways.forEach((other) => other.setAttribute('aria-pressed', String(other === way)));
      lines.forEach((line, j) => line.classList.toggle('is-on', j === i));
      status.textContent = '';
    });
  });
  for (const line of lines) copyButton(line, status);
}

interface Release {
  tag_name?: string;
  assets?: { name: string; size: number }[];
}

async function latestRelease(): Promise<void> {
  try {
    const answer = await fetch('https://api.github.com/repos/usecornercase/cornercase-gpui/releases/latest');
    if (!answer.ok) return;
    const release = (await answer.json()) as Release;
    for (const size of all('[data-size]')) {
      const asset = release.assets?.find((one_) => one_.name === size.dataset.size);
      if (asset) size.textContent = `· ${megabytes(asset.size)}`;
    }
    if (release.tag_name) one('[data-version]').textContent = ` · Latest: ${release.tag_name}`;
  } catch {
    one('[data-version]').textContent = '';
  }
}

function spotlight(): void {
  for (const card of all('.platform')) {
    card.addEventListener('pointermove', (event) => {
      const box = card.getBoundingClientRect();
      card.style.setProperty('--px', `${event.clientX - box.left}px`);
      card.style.setProperty('--py', `${event.clientY - box.top}px`);
    });
  }
}

function magnets(): void {
  if (!matchMedia('(pointer: fine)').matches) return;
  for (const el of all('[data-magnet]')) {
    el.addEventListener('pointermove', (event) => {
      const box = el.getBoundingClientRect();
      const x = (event.clientX - box.left - box.width / 2) * 0.16;
      const y = (event.clientY - box.top - box.height / 2) * 0.24;
      el.style.transform = `translate(${x.toFixed(1)}px,${y.toFixed(1)}px)`;
    });
    el.addEventListener('pointerleave', () => {
      el.style.transform = '';
    });
  }
}

export function preparePage(): Os {
  const os = osOf(navigator.userAgent, navigator.platform, navigator.maxTouchPoints);
  renameKeys(os);
  markDownloads(os);
  installBox();
  spotlight();
  magnets();
  void latestRelease();
  return os;
}
