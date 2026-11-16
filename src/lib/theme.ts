export const THEME_COLOR_KEYS = ['background', 'panel', 'surface', 'surfaceHover', 'border', 'text', 'secondary', 'muted', 'accent', 'accentText', 'sidebar', 'player', 'selection', 'success', 'error'] as const;
export type ThemeColorKey = typeof THEME_COLOR_KEYS[number];
export type ThemeBase = 'light' | 'dark' | 'gold';
export interface ThemeDefinition {
  version: 1;
  id: string;
  name: string;
  base: ThemeBase;
  colors: Record<ThemeColorKey, string>;
  fontFamily: 'system' | 'sans' | 'serif' | 'mono';
  fontScale: number;
  radius: number;
  spacing: number;
  shadows: number;
}
export interface AppearanceSettings { activeThemeId: string; customThemes: ThemeDefinition[] }

export const BUILTIN_THEMES: readonly ThemeDefinition[] = [
  { version: 1, id: 'light', name: 'Light', base: 'light', colors: {
    background: '#F5F6F8', panel: '#FFFFFF', surface: '#EDF0F3', surfaceHover: '#E2E7EC', border: '#D5DBE2',
    text: '#202731', secondary: '#52606E', muted: '#687482', accent: '#187562', accentText: '#FFFFFF',
    sidebar: '#EBEEF2', player: '#FFFFFF', selection: '#D7EBE5', success: '#24734B', error: '#B13740',
  }, fontFamily: 'sans', fontScale: 1, radius: 8, spacing: 1, shadows: 0.45 },
  { version: 1, id: 'dark', name: 'Dark', base: 'dark', colors: {
    background: '#141716', panel: '#191D1B', surface: '#232A26', surfaceHover: '#303C34', border: '#35463B',
    text: '#EEEEE8', secondary: '#B1C2B6', muted: '#82998B', accent: '#8DE2CE', accentText: '#16392E',
    sidebar: '#111613', player: '#17231B', selection: '#30483B', success: '#9DD2AB', error: '#EBAEA7',
  }, fontFamily: 'sans', fontScale: 1, radius: 8, spacing: 1, shadows: 0.6 },
  { version: 1, id: 'gold', name: 'Gold', base: 'gold', colors: {
    // Gold only reads as gold when it is bright; a dark gold is brown. So every dark surface stays a
    // neutral, slightly cool black and the warmth is left to the accent alone.
    background: '#0E0E10', panel: '#151518', surface: '#1C1C20', surfaceHover: '#27272C', border: '#2D2D33',
    text: '#F5F2EA', secondary: '#B7B5AF', muted: '#8C8A85', accent: '#DDB65B', accentText: '#17120A',
    sidebar: '#0A0A0C', player: '#121214', selection: '#232226', success: '#8FCBA4', error: '#EE9B94',
  }, fontFamily: 'sans', fontScale: 1, radius: 8, spacing: 1.04, shadows: 0.85 },
];
export const DEFAULT_APPEARANCE: AppearanceSettings = { activeThemeId: 'dark', customThemes: [] };
const bases = new Set(['light', 'dark', 'gold']);
const fonts = new Set(['system', 'sans', 'serif', 'mono']);
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const validId = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9_-]{1,64}$/.test(value);
function range(value: unknown, min: number, max: number, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max) throw new Error(`Invalid theme ${label}`);
  return value;
}

/** Validate and copy only supported values; imported files can never inject CSS. */
export function validateTheme(value: unknown): ThemeDefinition {
  if (!record(value) || value.version !== 1 || !validId(value.id) || typeof value.name !== 'string'
    || !value.name.trim() || value.name.trim().length > 80 || /[\u0000-\u001f]/.test(value.name)
    || !bases.has(String(value.base)) || !fonts.has(String(value.fontFamily)) || !record(value.colors)) throw new Error('Invalid theme format');
  const colors = {} as Record<ThemeColorKey, string>;
  for (const key of THEME_COLOR_KEYS) {
    const color = value.colors[key];
    if (typeof color !== 'string' || !/^#[0-9a-f]{6}$/i.test(color)) throw new Error(`Invalid theme color: ${key}`);
    colors[key] = color.toUpperCase();
  }
  return {
    version: 1, id: value.id, name: value.name.trim(), base: value.base as ThemeBase, colors,
    fontFamily: value.fontFamily as ThemeDefinition['fontFamily'], fontScale: range(value.fontScale, .85, 1.3, 'font size'),
    radius: range(value.radius, 0, 20, 'corner radius'), spacing: range(value.spacing, .8, 1.3, 'spacing'),
    shadows: range(value.shadows, 0, 1, 'shadow strength'),
  };
}

export function validateAppearance(value: unknown): AppearanceSettings {
  if (!record(value) || !validId(value.activeThemeId) || !Array.isArray(value.customThemes) || value.customThemes.length > 32) throw new Error('Invalid appearance settings');
  const customThemes = value.customThemes.map(validateTheme);
  const ids = new Set(BUILTIN_THEMES.map(theme => theme.id));
  for (const theme of customThemes) {
    if (ids.has(theme.id)) throw new Error('Duplicate or reserved theme ID');
    ids.add(theme.id);
  }
  if (!ids.has(value.activeThemeId)) throw new Error('Active theme is missing');
  return { activeThemeId: value.activeThemeId, customThemes };
}

export function resolveTheme(value: AppearanceSettings): ThemeDefinition {
  return BUILTIN_THEMES.find(theme => theme.id === value.activeThemeId)
    ?? value.customThemes.find(theme => theme.id === value.activeThemeId) ?? BUILTIN_THEMES[1];
}

export function createCustomTheme(theme: ThemeDefinition, name: string, existing: readonly ThemeDefinition[] = []): ThemeDefinition {
  if (existing.length >= 32) throw new Error('Theme limit reached');
  let id: string;
  do { id = `custom-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`}`; }
  while (existing.some(item => item.id === id));
  return validateTheme({ ...theme, id, name, colors: { ...theme.colors } });
}

export function parseThemeFile(contents: string): ThemeDefinition {
  if (contents.length > 100_000) throw new Error('Theme file is too large');
  const value: unknown = JSON.parse(contents);
  if (!record(value) || value.format !== 'song-player-theme' || value.version !== 1) throw new Error('Unsupported theme file');
  return validateTheme(value.theme);
}
export function serializeThemeFile(theme: ThemeDefinition): string {
  return `${JSON.stringify({ format: 'song-player-theme', version: 1, theme: validateTheme(theme) }, null, 2)}\n`;
}

export function buildThemeVariables(themeValue: ThemeDefinition): Record<string, string> {
  const theme = validateTheme(themeValue);
  const variables: Record<string, string> = {};
  for (const key of THEME_COLOR_KEYS) {
    const cssKey = key === 'background' ? 'bg' : key.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`);
    variables[`--${cssKey}`] = theme.colors[key];
  }
  const families = {
    system: "'Segoe UI', system-ui, sans-serif", sans: "'DM Sans', 'Segoe UI', sans-serif",
    serif: "Georgia, 'Times New Roman', serif", mono: "'Cascadia Code', Consolas, monospace",
  };
  variables['--font-family'] = families[theme.fontFamily];
  variables['--heading-font'] = theme.fontFamily === 'sans' ? "'Manrope', 'Segoe UI', sans-serif" : families[theme.fontFamily];
  variables['--font-scale'] = String(theme.fontScale);
  variables['--radius'] = `${theme.radius}px`;
  variables['--spacing'] = String(theme.spacing);
  variables['--shadow-strength'] = String(theme.shadows);
  return variables;
}

export function applyAppearance(value: AppearanceSettings): void {
  if (typeof document === 'undefined') return;
  const theme = resolveTheme(value);
  const root = document.documentElement;
  for (const [property, cssValue] of Object.entries(buildThemeVariables(theme))) root.style.setProperty(property, cssValue);
  root.dataset.theme = theme.id;
  root.dataset.themeBase = theme.base;
  root.style.colorScheme = theme.base === 'light' ? 'light' : 'dark';
}
