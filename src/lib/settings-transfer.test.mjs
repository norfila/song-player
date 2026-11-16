import assert from 'node:assert/strict';
import { test } from 'node:test';
import { availableSections, defaultImportSections, parseSettingsDocument } from './settings-transfer.ts';
import { BUILTIN_THEMES } from './theme.ts';
import { DEFAULT_SHORTCUTS } from './shortcuts.ts';

const document = sections => ({ format: 'song-player-settings', version: 1, exportedAt: 1_700_000_000_000, sections });

test('portable categories are selected while machine-specific values require an explicit choice', () => {
  const parsed = parseSettingsDocument(document({ general: { language: 'de', closeToTray: false }, device: { outputDevice: 'auto' }, library: { folders: ['C:/Music'] } }));
  assert.deepEqual(availableSections(parsed), ['general', 'device', 'library']);
  assert.deepEqual(defaultImportSections(parsed), ['general']);
});

test('backup review rejects wrong formats, future versions, unknown categories and malformed values', () => {
  for (const invalid of [
    { ...document({}), format: 'song-player-theme' },
    { ...document({ general: { language: 'en', closeToTray: false } }), version: 2 },
    document({ unknown: {} }), document({}), document({ general: { language: ['de'], closeToTray: true } }),
    document({ audio: { volume: 101, muted: false, replayGain: false, loudnessNormalization: true } }),
    document({ layout: { panelWidths: { sidebar: NaN, queue: 310 } } }),
    document({ device: { outputDevice: ' \t' } }), document({ library: { folders: ['C:/Music\u0001'] } }),
    { ...document({ shortcuts: DEFAULT_SHORTCUTS }), exportedAt: 1.5 },
    { ...document({ shortcuts: DEFAULT_SHORTCUTS }), unrecognized: true },
  ]) assert.throws(() => parseSettingsDocument(invalid));
});

test('shortcut validation rejects unknown actions and conflicts, and migrates missing actions', () => {
  assert.throws(() => parseSettingsDocument(document({ shortcuts: { ...DEFAULT_SHORTCUTS, unknown: 'KeyX' } })));
  assert.throws(() => parseSettingsDocument(document({ shortcuts: { ...DEFAULT_SHORTCUTS, next: 'Space' } })));
  const parsed = parseSettingsDocument(document({ shortcuts: { playPause: '' } }));
  assert.equal(parsed.sections.shortcuts.playPause, '');
  assert.equal(parsed.sections.shortcuts.nowPlaying, DEFAULT_SHORTCUTS.nowPlaying);
});

test('theme values in backups are canonicalized without mutating the imported document', () => {
  const theme = { ...structuredClone(BUILTIN_THEMES[0]), id: 'custom-theme', name: '  My theme  ', extra: 'ignored' };
  theme.colors.accent = '#abcdef';
  const input = document({ appearance: { activeThemeId: theme.id, customThemes: [theme] } });
  const parsed = parseSettingsDocument(input);
  assert.equal(parsed.sections.appearance.customThemes[0].name, 'My theme');
  assert.equal(parsed.sections.appearance.customThemes[0].colors.accent, '#ABCDEF');
  assert.equal('extra' in parsed.sections.appearance.customThemes[0], false);
  assert.equal(input.sections.appearance.customThemes[0].name, '  My theme  ');
});

test('a settings file cannot smuggle audio data or arbitrary fields into a selected category', () => {
  assert.throws(() => parseSettingsDocument(document({ general: { language: 'de', closeToTray: false, tracks: ['file'] } })));
  assert.throws(() => parseSettingsDocument(document({ library: { folders: ['C:/Music'], playlists: [] } })));
});

test('expanded portable preferences roundtrip while the global shortcut stays device-specific', () => {
  const sections = {
    general: { language: 'en', closeToTray: true, openExternalNowPlaying: false, suggestExternalFolders: true, statistics: { enabled: true, percent: 75, seconds: 0 } },
    audio: { volume: 55, muted: false, replayGain: true, loudnessNormalization: false, gainGrouping: 'folder', equalizer: { enabled: true, bands: [3, 0, -2, 1, 0], preamp: -4 } },
    device: { outputDevice: 'auto', globalSearch: { enabled: true, binding: 'Ctrl+Alt+Space' } },
  };
  const parsed = parseSettingsDocument(document(sections));
  assert.deepEqual(parsed.sections, sections);
  assert.deepEqual(defaultImportSections(parsed), ['general', 'audio']);
  const legacy = parseSettingsDocument(document({ general: { language: 'de', closeToTray: false }, audio: { volume: 65, muted: false, replayGain: false, loudnessNormalization: false }, device: { outputDevice: 'auto' } }));
  assert.equal(legacy.sections.general.statistics, undefined);
  assert.equal(legacy.sections.audio.equalizer, undefined);
  assert.equal(legacy.sections.device.globalSearch, undefined);
});

test('old fullscreen assignments disappear without resetting other customized shortcuts', () => {
  const parsed = parseSettingsDocument(document({ shortcuts: { fullscreen: 'Space', nowPlaying: 'Alt+KeyN', search: 'Ctrl+KeyK' } }));
  assert.equal('fullscreen' in parsed.sections.shortcuts, false);
  assert.equal(parsed.sections.shortcuts.nowPlaying, 'Alt+KeyN');
  assert.equal(parsed.sections.shortcuts.search, 'Ctrl+KeyK');
});

test('expanded preference bounds reject invalid analysis, equalizer and global shortcut settings', () => {
  const general = statistics => document({ general: { language: 'de', closeToTray: false, statistics } });
  for (const statistics of [
    { enabled: true, percent: 0, seconds: 240 }, { enabled: true, percent: 101, seconds: 0 },
    { enabled: true, percent: 50, seconds: -1 }, { enabled: true, percent: 50, seconds: 3601 },
    { enabled: true, percent: Infinity, seconds: 240 },
  ]) assert.throws(() => parseSettingsDocument(general(statistics)));
  const audio = changes => document({ audio: { volume: 65, muted: false, replayGain: true, loudnessNormalization: false, ...changes } });
  for (const equalizer of [
    { enabled: true, bands: [0, 0, 0, 0], preamp: 0 }, { enabled: true, bands: [0, 0, 13, 0, 0], preamp: 0 },
    { enabled: true, bands: [0, 0, 0, 0, 0], preamp: 1 }, { enabled: true, bands: [0, 0, 0, 0, 0], preamp: -25 },
  ]) assert.throws(() => parseSettingsDocument(audio({ equalizer })));
  assert.throws(() => parseSettingsDocument(audio({ gainGrouping: 'unknown' })));
  for (const binding of ['F8', 'Space', 'Ctrl+KeyF', 'Alt+F4', '']) {
    assert.throws(() => parseSettingsDocument(document({ device: { outputDevice: 'auto', globalSearch: { enabled: true, binding } } })));
  }
});
