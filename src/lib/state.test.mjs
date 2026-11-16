import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mergePlayback, mergeSnapshot } from './state.ts';

function snapshot(revision, sequence, name = 'Session') {
  return {
    revision,
    tracks: [], libraryTrackIds: [], libraryFolders: [], playlists: [],
    sessions: [{ id: 'session', name, entries: [], currentEntryId: null, position: 0, repeat: 'off', orderMode: 'manual' }],
    activeSessionId: 'session',
    settings: { language: 'de', volume: 75, muted: false, outputDevice: 'auto', replayGain: false, loudnessNormalization: false, closeToTray: false, defaultAppReminder: true, shortcuts: {}, panelWidths: { sidebar: 220, queue: 310 } },
    playback: { sequence, status: 'paused', position: sequence, duration: 200, volume: 75, muted: false, currentEntryId: null, trackId: null, sessionId: 'session' },
    engine: { available: true, version: '', error: null }, warnings: [],
  };
}

test('initial snapshot is accepted without synthesizing counters', () => {
  const incoming = snapshot(4, 9);
  assert.strictEqual(mergeSnapshot(null, incoming), incoming);
});

test('late structural response cannot replace a newer revision', () => {
  const current = snapshot(8, 12, 'Renamed');
  const oldResponse = snapshot(7, 13, 'Old name');
  assert.strictEqual(mergeSnapshot(current, oldResponse), current);
});

test('newer structure keeps playback received from a later tick', () => {
  const current = snapshot(8, 22);
  const newerStructure = snapshot(9, 18, 'Renamed');
  const result = mergeSnapshot(current, newerStructure);
  assert.equal(result.revision, 9);
  assert.equal(result.sessions[0].name, 'Renamed');
  assert.strictEqual(result.playback, current.playback);
  assert.equal(newerStructure.playback.sequence, 18, 'incoming snapshot remains unmodified');
});

test('equal structural revision is accepted without regressing playback', () => {
  const current = snapshot(8, 22);
  const result = mergeSnapshot(current, snapshot(8, 20, 'Same revision'));
  assert.equal(result.sessions[0].name, 'Same revision');
  assert.strictEqual(result.playback, current.playback);
});

test('snapshot with newer structure and playback is accepted intact', () => {
  const incoming = snapshot(9, 25);
  assert.strictEqual(mergeSnapshot(snapshot(8, 22), incoming), incoming);
});

test('new playback sequence does not advance structural revision', () => {
  const current = snapshot(8, 22);
  const tick = { ...current.playback, sequence: 23, position: 25 };
  const result = mergePlayback(current, tick);
  assert.equal(result.revision, 8);
  assert.strictEqual(result.sessions, current.sessions);
  assert.strictEqual(result.playback, tick);
});

test('old playback events are ignored; equal sequences remain admissible', () => {
  const current = snapshot(8, 22);
  assert.strictEqual(mergePlayback(current, { ...current.playback, sequence: 21 }), current);
  const sameSequence = { ...current.playback, position: 30 };
  assert.strictEqual(mergePlayback(current, sameSequence).playback, sameSequence);
});

test('late initial snapshot and dispatch response cannot undo already delivered events', () => {
  let state = mergeSnapshot(null, snapshot(3, 9, 'Latest name'));
  state = mergePlayback(state, { ...state.playback, sequence: 11, position: 44 });
  state = mergeSnapshot(state, snapshot(0, 0, 'Startup'));
  state = mergeSnapshot(state, snapshot(2, 8, 'Late command'));
  assert.equal(state.revision, 3);
  assert.equal(state.sessions[0].name, 'Latest name');
  assert.equal(state.playback.sequence, 11);
  assert.equal(state.playback.position, 44);
});

test('playback event before structural initialization cannot create partial application state', () => {
  assert.equal(mergePlayback(null, snapshot(0, 1).playback), null);
});
