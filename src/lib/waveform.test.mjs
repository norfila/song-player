import assert from 'node:assert/strict';
import test from 'node:test';
import { waveformBars } from './waveform.ts';

test('waveform bars keep bucket peaks and normalise to the loudest bar', () => {
  assert.deepEqual(waveformBars([0.1, 0.5, 0.25, 1], 2), [Math.sqrt(0.5), 1]);
  assert.equal(waveformBars(Array.from({ length: 1000 }, (_, i) => i / 1000), 120).length, 120);
});

test('waveform bars handle silence, invalid values and more bars than peaks', () => {
  assert.deepEqual(waveformBars([], 3), [0, 0, 0]);
  assert.deepEqual(waveformBars([0, 0], 2), [0, 0]);
  assert.deepEqual(waveformBars([Number.NaN, -1], 2), [0, 1]);
  assert.equal(waveformBars([0.5], 4).length, 4);
});
