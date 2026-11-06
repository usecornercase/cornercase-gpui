import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import { FILES, INSTALLS } from '../src/data/links';
import { SHEETS } from '../src/data/session';

const KEYMAP = readFileSync('../src/keymap.rs', 'utf8');
const PANELS = readFileSync('../src/panels/mod.rs', 'utf8');
const DOWNLOAD_SECTION = readFileSync('src/components/Download.astro', 'utf8');
const LOCATE = readFileSync('../src/cli/locate.rs', 'utf8');

const PANEL_KEYS: Record<string, string> = {
  Search: 'Search',
  Issues: 'Issues',
  Queue: 'Queue',
  Flows: 'Flows',
  Usage: 'Usage',
  Accounts: 'Accounts',
  Machines: 'Machines',
  Processes: 'Processes',
  Ports: 'Ports',
  Worktrees: 'Worktrees',
  Plugins: 'Plugins',
  Settings: 'Settings',
};

test('every sheet has a picture', () => {
  const pictures = readdirSync('src/assets/sheets');
  for (const sheet of SHEETS) assert.ok(pictures.includes(`${sheet.file}.webp`), sheet.name);
});

test('every sheet is a panel of the app', () => {
  for (const sheet of SHEETS) assert.ok(PANELS.includes(`"${sheet.name}"`), sheet.name);
});

test('every sheet shows the key the app gives it', () => {
  for (const sheet of SHEETS) {
    const last = (sheet.keys[sheet.keys.length - 1] ?? '').toLowerCase();
    const shift = sheet.keys.includes('shift');
    assert.ok(sheet.keys.includes('cmd'), `${sheet.name}: its key needs the command modifier`);
    const expected = `key(&["${last}"], ${shift}, Command::Panel(PanelKind::${PANEL_KEYS[sheet.name]}))`;
    assert.ok(KEYMAP.includes(expected), `${sheet.name}: ${expected}`);
  }
});

test('the version of cornercase the page asks for is the one the app asks for', () => {
  const minimum = LOCATE.slice(LOCATE.indexOf('const MINIMUM'), LOCATE.indexOf('impl Version'));
  const version = minimum.match(/major: (\d+), minor: (\d+), patch: (\d+)/);
  assert.ok(version, 'the app states its minimum');
  assert.ok(DOWNLOAD_SECTION.includes(`${version[1]}.${version[2]}.${version[3]}`), version[0]);
});

test('the downloads are the files a release publishes', () => {
  const release = readFileSync('../README.md', 'utf8');
  for (const file of Object.values(FILES)) assert.ok(release.includes(file), file);
});

test('the install lines are cornercase own', () => {
  assert.equal(INSTALLS.length, 3);
  for (const [, command] of INSTALLS) assert.ok(command.includes('cornercase'), command);
});
