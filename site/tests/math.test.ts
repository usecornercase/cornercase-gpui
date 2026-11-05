import assert from 'node:assert/strict';
import { test } from 'node:test';
import { backOut, beat, clamp, damp, easeExpo, pulse, span } from '../src/lib/math';

test('a span runs from nothing to whole between its ends', () => {
  assert.equal(span(5, 10, 20), 0);
  assert.equal(span(15, 10, 20), 0.5);
  assert.equal(span(25, 10, 20), 1);
});

test('an empty span is a step at its end', () => {
  assert.equal(span(9, 10, 10), 0);
  assert.equal(span(10, 10, 10), 1);
});

test('clamp keeps a value inside its range', () => {
  assert.equal(clamp(-1), 0);
  assert.equal(clamp(2), 1);
  assert.equal(clamp(5, 0, 10), 5);
});

test('every ease starts at nothing and ends whole', () => {
  for (const ease of [easeExpo, backOut]) {
    assert.ok(Math.abs(ease(0)) < 1e-9);
    assert.ok(Math.abs(ease(1) - 1) < 1e-9);
  }
});

test('backOut overshoots before it settles', () => {
  assert.ok(backOut(0.7) > 1);
});

test('a beat holds its end after it finishes', () => {
  assert.equal(beat(0.1, 0.2, 0.4), 0);
  assert.equal(beat(0.9, 0.2, 0.4), 1);
});

test('a pulse comes and goes', () => {
  assert.equal(pulse(0, 0.1, 0.2, 0.7, 0.8), 0);
  assert.equal(pulse(0.5, 0.1, 0.2, 0.7, 0.8), 1);
  assert.equal(pulse(0.9, 0.1, 0.2, 0.7, 0.8), 0);
});

test('damping moves the same way whatever the frame rate', () => {
  const once = damp(0, 1, 8, 1 / 30);
  const twice = damp(damp(0, 1, 8, 1 / 60), 1, 8, 1 / 60);
  assert.ok(once > 0 && once < 1);
  assert.ok(Math.abs(once - twice) < 1e-9);
});
