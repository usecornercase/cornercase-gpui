export type Span = [string, string];
export type Line = Span[];

const t = (text: string, kind = ''): Span => [kind, text];

export const AGENT: Line[] = [
  [t('> ', 'dim'), t('Dark mode for the checkout. Follow the system,')],
  [t('  '), t('and keep the brand colour apart from both grounds.')],
  [],
  [t('● ', 'green'), t('Read', 'bold'), t('(src/theme.ts)')],
  [t('  ⎿  ', 'dim'), t('Read 27 lines', 'dim')],
  [],
  [t('● ', 'green'), t('Read', 'bold'), t('(src/checkout.tsx)')],
  [t('  ⎿  ', 'dim'), t('Read 19 lines', 'dim')],
  [],
  [t('● ', 'text'), t('The checkout hardcodes '), t('#ffffff', 'blue'), t(' in three places, so')],
  [t('  dark mode never reaches it. Its colours come from')],
  [t('  the theme now.')],
  [],
  [t('● ', 'green'), t('Update', 'bold'), t('(src/checkout.tsx)')],
  [t('  ⎿  ', 'dim'), t('Updated with 7 additions and 5 removals', 'dim')],
  [],
  [t('● ', 'green'), t('Update', 'bold'), t('(src/theme.ts)')],
  [t('  ⎿  ', 'dim'), t('Updated with 10 additions and 2 removals', 'dim')],
  [],
  [t('● ', 'green'), t('Bash', 'bold'), t('(npm test)')],
  [t('  ⎿  ', 'dim'), t('tests 6 · pass 6 · fail 0', 'dim')],
  [],
  [t('● ', 'text'), t('Done. '), t('themeFor', 'blue'), t(' is in '), t('src/theme.ts:25', 'w-link js-path-code'), t(', and')],
  [t('  the page is on '), t('http://localhost:5173/', 'w-link js-path-url'), t('. Here is')],
  [t('  how it looks: '), t('checkout.png', 'w-link js-path-image')],
];

export const ORCHESTRATOR: Line[] = [
  [t('> ', 'dim'), t('Fix #482: the returns page crashes on an empty')],
  [t('  address. Run the plan with the team.')],
  [],
  [t('● ', 'green'), t('Sending #482 to '), t('coder', 'blue')],
  [t('● ', 'text'), t('coder', 'blue'), t(': first_line returns an Option now')],
  [t('● ', 'green'), t('Sending the change to '), t('reviewer', 'magenta'), t(' and '), t('security', 'cyan')],
  [t('● ', 'orange'), t('reviewer', 'magenta'), t(': one finding, back to '), t('coder', 'blue'), t(' (1/3)')],
  [t('● ', 'text'), t('coder', 'blue'), t(': the 422 now names the field')],
  [t('● ', 'green'), t('reviewer', 'magenta'), t(' and '), t('security', 'cyan'), t(' approve')],
  [t('● ', 'green'), t('Sending the branch to '), t('tester', 'green')],
  [t('● ', 'text'), t('tester', 'green'), t(': every test passes')],
  [t('● ', 'green'), t('Opening the pull request')],
];

export const THEME_TS: Line[] = [
  [t('export type', 'magenta'), t(' Theme = {')],
  [t('  background: '), t('string', 'cyan'), t(';')],
  [t('  surface: '), t('string', 'cyan'), t(';')],
  [t('  text: '), t('string', 'cyan'), t(';')],
  [t('  muted: '), t('string', 'cyan'), t(';')],
  [t('  brand: '), t('string', 'cyan'), t(';')],
  [t('};')],
  [],
  [t('export const', 'magenta'), t(' light: '), t('Theme', 'cyan'), t(' = {')],
  [t('  background: '), t('"#ffffff"', 'green'), t(',')],
  [t('  surface: '), t('"#f4f5f7"', 'green'), t(',')],
  [t('  text: '), t('"#1a1b26"', 'green'), t(',')],
  [t('  muted: '), t('"#6b7080"', 'green'), t(',')],
  [t('  brand: '), t('"#875fff"', 'green'), t(',')],
  [t('};')],
  [],
  [t('export const', 'magenta'), t(' dark: '), t('Theme', 'cyan'), t(' = {')],
  [t('  background: '), t('"#1a1b26"', 'green'), t(',')],
  [t('  surface: '), t('"#24283b"', 'green'), t(',')],
  [t('  text: '), t('"#c0caf5"', 'green'), t(',')],
  [t('  muted: '), t('"#7a82a6"', 'green'), t(',')],
  [t('  brand: '), t('"#9d7cff"', 'green'), t(',')],
  [t('};')],
  [],
  [t('export function', 'magenta'), t(' '), t('themeFor', 'blue'), t('(prefersDark: '), t('boolean', 'cyan'), t('): '), t('Theme', 'cyan'), t(' {')],
  [t('  return', 'magenta'), t(' prefersDark ? dark : light;')],
  [t('}')],
];

export const TESTS: Line[] = [
  [t('> web-shop@0.4.0 test', 'soft')],
  [t('> node --test', 'soft')],
  [],
  [t('✔ an empty cart costs nothing ', 'pass'), t('(0.50ms)', 'faint')],
  [t('✔ the total adds every line ', 'pass'), t('(0.05ms)', 'faint')],
  [t('✔ the total is rounded to cents ', 'pass'), t('(0.03ms)', 'faint')],
  [t('✔ the light theme stays the default ', 'pass'), t('(0.51ms)', 'faint')],
  [t('✔ dark mode follows the system ', 'pass'), t('(0.03ms)', 'faint')],
  [t('✔ the brand colour differs from both ', 'pass'), t('(0.04ms)', 'faint')],
  [t('ℹ tests 6', 'blue')],
  [t('ℹ pass 6', 'blue')],
  [t('ℹ fail 0', 'blue')],
  [t('ℹ duration_ms 86.89', 'blue')],
  [t('feat-dark-mode', 'soft'), t(' feat/dark-mode ', 'green'), t('❯', 'green')],
];

export interface DiffLine {
  old?: number;
  now?: number;
  kind: 'same' | 'add' | 'del';
  text: Line;
}

export const DIFF: DiffLine[] = [
  { old: 1, now: 1, kind: 'same', text: [t('import type', 'magenta'), t(' { Cart } '), t('from', 'magenta'), t(' '), t('"./cart.ts"', 'green'), t(';')] },
  { old: 2, now: 2, kind: 'same', text: [t('import', 'magenta'), t(' { total } '), t('from', 'magenta'), t(' '), t('"./cart.ts"', 'green'), t(';')] },
  { now: 3, kind: 'add', text: [t('import', 'magenta'), t(' { themeFor } '), t('from', 'magenta'), t(' '), t('"./theme.ts"', 'green'), t(';')] },
  { old: 3, now: 4, kind: 'same', text: [] },
  { old: 4, kind: 'del', text: [t('export function', 'magenta'), t(' '), t('Checkout', 'blue'), t('({ cart }) {')] },
  { now: 5, kind: 'add', text: [t('export function', 'magenta'), t(' '), t('Checkout', 'blue'), t('({ cart, '), t('prefersDark', 'hot'), t(' }) {')] },
  { now: 6, kind: 'add', text: [t('  const', 'magenta'), t(' theme = '), t('themeFor', 'blue'), t('(prefersDark);')] },
  { old: 5, now: 7, kind: 'same', text: [t('  return', 'magenta'), t(' (')] },
  { old: 6, kind: 'del', text: [t('    <main style={{ background: '), t('"#ffffff"', 'hotdel'), t(' }}>')] },
  { now: 8, kind: 'add', text: [t('    <main style={{ background: '), t('theme.background', 'hot'), t(' }}>')] },
  { old: 7, now: 9, kind: 'same', text: [t('      <h1>Checkout</h1>')] },
  { old: 8, kind: 'del', text: [t('      <ul style={{ background: '), t('"#ffffff"', 'hotdel'), t(' }}>')] },
  { now: 10, kind: 'add', text: [t('      <ul style={{ background: '), t('theme.surface', 'hot'), t(' }}>')] },
  { old: 9, now: 11, kind: 'same', text: [t('        {cart.items.map((item) => (')] },
  { old: 10, now: 12, kind: 'same', text: [t('          <li key={item.sku}>')] },
  { old: 11, kind: 'del', text: [t('            {item.name} × {item.quantity}')] },
  { now: 13, kind: 'add', text: [t('            {item.name} '), t('<span style={{', 'hot')] },
  { now: 14, kind: 'add', text: [t('              color: theme.muted }}>', 'hot'), t('× {item.quantity}')] },
];

export const PREVIEW_CODE: { at: number; text: Line; here?: boolean }[] = [
  { at: 23, text: [t('};')] },
  { at: 24, text: [] },
  { at: 25, here: true, text: [t('export function', 'magenta'), t(' '), t('themeFor', 'blue'), t('(prefersDark: '), t('boolean', 'cyan'), t('): '), t('Theme', 'cyan'), t(' {')] },
  { at: 26, text: [t('  return', 'magenta'), t(' prefersDark ? dark : light;')] },
  { at: 27, text: [t('}')] },
];

export const ROLES = {
  orchestrator: { name: 'orchestrator', agent: 'claude', colour: '#005fff' },
  coder: { name: 'coder', agent: 'claude', colour: '#7aa2f7' },
  reviewer: { name: 'reviewer', agent: 'codex', colour: '#bb9af7' },
  security: { name: 'security', agent: 'claude', colour: '#7dcfff' },
  tester: { name: 'tester', agent: 'claude', colour: '#9ece6a' },
};

export const SHEETS: { name: string; file: string; keys: string[]; line: string }[] = [
  { name: 'Search', file: 'search', keys: ['cmd', 'F'], line: 'Names, settings, and what agents and shells said.' },
  { name: 'Issues', file: 'issues', keys: ['cmd', 'I'], line: 'Your trackers, and an agent on any issue.' },
  { name: 'Queue', file: 'queue', keys: ['shift', 'cmd', 'S'], line: 'Agents that start later, or repeat as routines.' },
  { name: 'Flows', file: 'flows', keys: ['shift', 'cmd', 'G'], line: 'Teams of agents, their plans and runs.' },
  { name: 'Usage', file: 'usage', keys: ['cmd', 'U'], line: 'Plan limits and the numbers of your agents, on each machine.' },
  { name: 'Accounts', file: 'accounts', keys: ['shift', 'cmd', 'A'], line: 'Agent logins, and the account each pane runs on.' },
  { name: 'Machines', file: 'machines', keys: ['shift', 'cmd', 'N'], line: 'Other computers your agents can run on, and each link.' },
  { name: 'Processes', file: 'processes', keys: ['shift', 'cmd', 'M'], line: 'What every pane runs, its CPU and memory.' },
  { name: 'Ports', file: 'ports', keys: ['shift', 'cmd', 'O'], line: 'What listens, to open it or stop it.' },
  { name: 'Worktrees', file: 'worktrees', keys: ['shift', 'cmd', 'T'], line: 'Every worktree on disk, and what is safe to remove.' },
  { name: 'Plugins', file: 'plugins', keys: ['shift', 'cmd', 'X'], line: 'Install them, link your own, run them from the menus and the rail.' },
  { name: 'Settings', file: 'settings', keys: ['cmd', ','], line: 'All of cornercase, in one form.' },
];
