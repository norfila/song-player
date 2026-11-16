import assert from 'node:assert/strict';
import { test } from 'node:test';
import { consumeVolumeWheel } from './volume-wheel.ts';

const wheel = (deltaY, extra = {}) => ({ deltaX: 0, deltaY, deltaMode: 0, ctrlKey: false, metaKey: false, ...extra });

test('usual pixel, line and page wheel events adjust two points in the expected direction', () => {
  assert.equal(consumeVolumeWheel(50, 0, wheel(-100)).volume, 52);
  assert.equal(consumeVolumeWheel(50, 0, wheel(100)).volume, 48);
  assert.equal(consumeVolumeWheel(50, 0, wheel(-3, { deltaMode: 1 })).volume, 52);
  assert.equal(consumeVolumeWheel(50, 0, wheel(1, { deltaMode: 2 })).volume, 48);
});

test('small touchpad deltas accumulate rather than jumping per event', () => {
  let result = { volume: 50, remainder: 0 };
  for (let i = 0; i < 4; i++) {
    result = consumeVolumeWheel(result.volume, result.remainder, wheel(-10));
    assert.equal(result.volume, 50);
  }
  result = consumeVolumeWheel(result.volume, result.remainder, wheel(-10));
  assert.equal(result.volume, 51);
  assert.equal(result.remainder, 0);
});

test('volume is clamped and boundary fractions do not delay scrolling back', () => {
  assert.deepEqual(consumeVolumeWheel(99, .8, wheel(-100)), { handled: true, volume: 100, remainder: 0 });
  assert.deepEqual(consumeVolumeWheel(1, -.8, wheel(100)), { handled: true, volume: 0, remainder: 0 });
  assert.equal(consumeVolumeWheel(100, 0, wheel(100)).volume, 98);
  assert.equal(consumeVolumeWheel(50, .9, wheel(50)).volume, 49);
});

test('zoom, horizontal and invalid wheel events remain available to the browser', () => {
  for (const event of [wheel(-100, { ctrlKey: true }), wheel(-100, { metaKey: true }), wheel(10, { deltaX: 20 }), wheel(0), wheel(NaN)]) {
    assert.deepEqual(consumeVolumeWheel(50, .5, event), { handled: false, volume: 50, remainder: .5 });
  }
});
