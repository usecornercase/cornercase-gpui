export type Os = 'mac' | 'windows' | 'linux' | 'phone';

export function osOf(agent: string, platform = '', touchPoints = 0): Os {
  const text = `${agent} ${platform}`;
  if (/iPhone|iPad|iPod|Android/i.test(text)) return 'phone';
  if (/Mac/i.test(text)) return touchPoints > 1 ? 'phone' : 'mac';
  if (/Win/i.test(text)) return 'windows';
  if (/Linux|X11|CrOS/i.test(text)) return 'linux';
  return 'mac';
}

const NAMES: Record<Exclude<Os, 'phone'>, string> = { mac: 'macOS', windows: 'Windows', linux: 'Linux' };

export function downloadLabel(os: Os): string {
  return os === 'phone' ? 'Download for your computer' : `Download for ${NAMES[os]}`;
}

export function keysFor(keys: string[], os: Os, usage = false): string[] {
  if (os === 'mac' || os === 'phone') return keys.map((key) => (key === 'cmd' ? '⌘' : key === 'shift' ? '⇧' : key));
  const last = keys[keys.length - 1] ?? '';
  const shifted = keys.includes('shift');
  if (usage || shifted) return ['Ctrl', 'Alt', 'Shift', last];
  return ['Ctrl', 'Shift', last];
}

export function megabytes(bytes: number): string {
  return `${Math.round(bytes / 1_000_000)} MB`;
}
