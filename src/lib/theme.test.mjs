import assert from 'node:assert/strict';
import { test } from 'node:test';
import { BUILTIN_THEMES, DEFAULT_APPEARANCE, buildThemeVariables, createCustomTheme, parseThemeFile, resolveTheme, serializeThemeFile, validateAppearance, validateTheme } from './theme.ts';

test('all bundled themes round-trip with complete, valid colors', () => {
  for (const theme of BUILTIN_THEMES) assert.deepEqual(parseThemeFile(serializeThemeFile(theme)), theme);
  assert.equal(resolveTheme(DEFAULT_APPEARANCE).id, 'dark');
});
test('theme import rejects unsupported versions, oversized files and CSS injection', () => {
  assert.throws(() => parseThemeFile(JSON.stringify({ format: 'sonora-theme', version: 2, theme: BUILTIN_THEMES[0] })));
  assert.throws(() => parseThemeFile(' '.repeat(100_001)));
  assert.throws(() => validateTheme({ ...BUILTIN_THEMES[0], colors: { ...BUILTIN_THEMES[0].colors, accent: 'red; background: url(x)' } }));
  assert.throws(() => validateTheme({ ...BUILTIN_THEMES[0], fontFamily: 'url(font)' }));
});
test('numeric theme controls reject nonfinite and out-of-range input', () => {
  for (const [key, bad] of [['fontScale', .1], ['radius', 21], ['spacing', 9], ['shadows', -1], ['radius', NaN]]) {
    assert.throws(() => validateTheme({ ...BUILTIN_THEMES[0], [key]: bad }));
  }
});
test('custom themes have independent color values and keep builtins unchanged', () => {
  const before = JSON.stringify(BUILTIN_THEMES);
  const custom = createCustomTheme(BUILTIN_THEMES[2], '  My gold  ');
  assert.equal(custom.name, 'My gold');
  assert.match(custom.id, /^custom-/);
  custom.colors.accent = '#123456';
  assert.equal(JSON.stringify(BUILTIN_THEMES), before);
  assert.equal(validateAppearance({ activeThemeId: custom.id, customThemes: [custom] }).activeThemeId, custom.id);
});
test('appearance import requires unique custom IDs and an existing active theme', () => {
  const custom = createCustomTheme(BUILTIN_THEMES[1], 'Custom');
  assert.throws(() => validateAppearance({ activeThemeId: custom.id, customThemes: [custom, custom] }));
  assert.throws(() => validateAppearance({ activeThemeId: 'light', customThemes: [BUILTIN_THEMES[0]] }));
  assert.throws(() => validateAppearance({ activeThemeId: 'missing', customThemes: [] }));
  assert.throws(() => validateAppearance({ activeThemeId: 'dark', customThemes: Array(33).fill(custom) }));
});
test('theme variables cover all interface areas and expose controlled typography and density', () => {
  const vars = buildThemeVariables(BUILTIN_THEMES[0]);
  assert.equal(vars['--bg'], '#F5F6F8');
  assert.equal(vars['--surface-hover'], '#E2E7EC');
  assert.equal(vars['--accent-text'], '#FFFFFF');
  for (const key of ['--sidebar', '--player', '--selection', '--error', '--font-family', '--spacing', '--radius']) assert.ok(vars[key]);
});
