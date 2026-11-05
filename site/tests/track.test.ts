import assert from 'node:assert/strict';
import { test } from 'node:test';
import { freeArea, mixPanes, placeKeys, pose, poseAt, toPixels, transformOf, WIDE_SCREEN, WINDOW } from '../src/lib/track';
import type { PaneRects, Rect, Shot } from '../src/lib/track';

const front: Shot = { focus: WINDOW, free: 'all' };

test('a framed rectangle fits inside the free area', () => {
  const shot: Shot = { focus: [100, 100, 400, 300], free: 'right' };
  const area = freeArea('right', 1440, 900);
  const p = pose(shot, 1440, 900);
  const left = p.x + 100 * p.scale;
  const right = p.x + 500 * p.scale;
  const top = p.y + 100 * p.scale;
  const bottom = p.y + 400 * p.scale;
  assert.ok(left >= area[0] - 0.01);
  assert.ok(right <= area[0] + area[2] + 0.01);
  assert.ok(top >= area[1] - 0.01);
  assert.ok(bottom <= area[1] + area[3] + 0.01);
});

test('the caption takes the side the camera leaves free', () => {
  const right = freeArea('right', 1440, 900);
  const left = freeArea('left', 1440, 900);
  assert.ok(right[0] > 300);
  assert.ok(left[0] + left[2] < 1140);
});

test('a phone frames above the caption whatever the side', () => {
  const area = freeArea('right', 390, 844);
  assert.deepEqual(area, freeArea('left', 390, 844));
  assert.ok(area[1] + area[3] < 844 - 200);
  assert.ok(390 < WIDE_SCREEN);
});

test('the camera never turns the window inside out on a very short screen', () => {
  for (const [width, height] of [[900, 120], [1440, 60], [390, 90]] as const) {
    assert.ok(pose(front, width, height).scale > 0, `${width}x${height}`);
  }
});

test('the track holds its first and last poses beyond its ends', () => {
  const keys = placeKeys([{ at: 100, shot: front }, { at: 200, shot: { focus: WINDOW, free: 'all', zoom: 0.5 } }], 1440, 900);
  assert.deepEqual(poseAt(keys, 0), keys[0]?.pose);
  assert.deepEqual(poseAt(keys, 999), keys[1]?.pose);
});

test('the track passes through every key', () => {
  const second: Shot = { focus: WINDOW, free: 'right', rx: 30 };
  const keys = placeKeys([{ at: 0, shot: front }, { at: 100, shot: second }, { at: 200, shot: front }], 1440, 900);
  assert.deepEqual(poseAt(keys, 100).rx, 30);
});

test('keys in any order give the same track', () => {
  const second: Shot = { focus: WINDOW, free: 'all', rx: 30 };
  const sorted = placeKeys([{ at: 0, shot: front }, { at: 100, shot: second }], 1440, 900);
  const shuffled = placeKeys([{ at: 100, shot: second }, { at: 0, shot: front }], 1440, 900);
  assert.deepEqual(poseAt(sorted, 50), poseAt(shuffled, 50));
});

test('a pose that does not turn writes no rotation', () => {
  assert.ok(!transformOf(pose(front, 1440, 900)).includes('rotate'));
  assert.ok(transformOf(pose({ ...front, rx: 10 }, 1440, 900)).includes('rotateX(10.00deg)'));
});

test('panes keep a gap between them and none at the edges', () => {
  const left: Rect = [0, 0, 0.5, 1];
  const right: Rect = [0.5, 0, 0.5, 1];
  const a = toPixels(left, 1000, 800, 8);
  const b = toPixels(right, 1000, 800, 8);
  assert.equal(a[0], 0);
  assert.equal(b[0] + b[2], 1000);
  assert.equal(b[0] - (a[0] + a[2]), 8);
});

test('panes blend from one layout to another', () => {
  const from: PaneRects = { a: [0, 0, 1, 1], b: [1, 0, 0, 1], c: [1, 0, 0, 1], s: [1, 0, 0, 1] };
  const to: PaneRects = { a: [0, 0, 0.5, 1], b: [0.5, 0, 0.5, 1], c: [1, 0, 0, 1], s: [1, 0, 0, 1] };
  assert.equal(mixPanes(from, to, 0.5).a[2], 0.75);
});
