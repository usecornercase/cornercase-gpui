import assert from 'node:assert/strict';
import { test } from 'node:test';
import { downloadLabel, keysFor, megabytes, osOf } from '../src/lib/os';

test('each system is told from its browser', () => {
  assert.equal(osOf('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)'), 'mac');
  assert.equal(osOf('Mozilla/5.0 (Windows NT 10.0; Win64; x64)'), 'windows');
  assert.equal(osOf('Mozilla/5.0 (X11; Linux x86_64)'), 'linux');
  assert.equal(osOf('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)'), 'phone');
  assert.equal(osOf('Mozilla/5.0 (Linux; Android 15; Pixel 8)'), 'phone');
});

test('an iPad that says it is a Mac is told apart by its touch screen', () => {
  const desktopSafari = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15';
  assert.equal(osOf(desktopSafari, 'MacIntel', 5), 'phone');
  assert.equal(osOf(desktopSafari, 'MacIntel', 0), 'mac');
});

test('the download names the system it is for', () => {
  assert.equal(downloadLabel('windows'), 'Download for Windows');
  assert.equal(downloadLabel('phone'), 'Download for your computer');
});

test('a Mac shows its own key symbols', () => {
  assert.deepEqual(keysFor(['shift', 'cmd', 'C'], 'mac'), ['⇧', '⌘', 'C']);
});

test('elsewhere command is Ctrl+Shift', () => {
  assert.deepEqual(keysFor(['cmd', 'F'], 'linux'), ['Ctrl', 'Shift', 'F']);
});

test('elsewhere shift and command are Ctrl+Alt+Shift', () => {
  assert.deepEqual(keysFor(['shift', 'cmd', 'G'], 'windows'), ['Ctrl', 'Alt', 'Shift', 'G']);
});

test('Usage takes Ctrl+Alt+Shift, since IBus keeps Ctrl+Shift+U', () => {
  assert.deepEqual(keysFor(['cmd', 'U'], 'linux', true), ['Ctrl', 'Alt', 'Shift', 'U']);
});

test('download sizes read in megabytes', () => {
  assert.equal(megabytes(24_724_649), '25 MB');
});
