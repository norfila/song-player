import { validateAppearance } from './theme.ts';
import type { AppearanceSettings } from './theme.ts';
import { resolveShortcuts, SHORTCUT_ACTIONS, validateShortcut, validateShortcuts } from './shortcuts.ts';
import type { EqualizerSettings, GainGrouping, GlobalSearchSettings, StatisticsSettings } from './backend/expansion-types';

export const TRANSFER_SECTIONS = ['general', 'appearance', 'audio', 'shortcuts', 'layout', 'device', 'library'] as const;
export type TransferSection = typeof TRANSFER_SECTIONS[number];
export const PORTABLE_SECTIONS: TransferSection[] = ['general', 'appearance', 'audio', 'shortcuts', 'layout'];

export interface SettingsDocument {
  format: 'song-player-settings';
  version: 1;
  exportedAt: number;
  sections: {
    general?: { language: string; closeToTray: boolean; openExternalNowPlaying?: boolean; suggestExternalFolders?: boolean; statistics?: StatisticsSettings };
    appearance?: AppearanceSettings;
    audio?: { volume: number; muted: boolean; replayGain: boolean; loudnessNormalization: boolean; equalizer?: EqualizerSettings; gainGrouping?: GainGrouping };
    shortcuts?: Record<string, string>;
    layout?: { panelWidths: { sidebar: number; queue: number }; seekbarStyle?: 'line' | 'waveform' };
    device?: { outputDevice: string; globalSearch?: GlobalSearchSettings };
    library?: { folders: string[] };
  };
}

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const bounded = (value: unknown, min: number, max: number) => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const boolean = (value: unknown) => typeof value === 'boolean';
const keysOnly = (value: Record<string, unknown>, keys: string[]) => Object.keys(value).every(key => keys.includes(key));
const bytes = (value: string) => new TextEncoder().encode(value).length;
const invalid = (): never => { throw new Error('invalid_settings_file'); };

/** Validate before displaying an import review; the native layer validates again on apply. */
export function parseSettingsDocument(value: unknown): SettingsDocument {
  if (!object(value) || value.format !== 'song-player-settings' || value.version !== 1 || !object(value.sections)
    || !keysOnly(value, ['format', 'version', 'exportedAt', 'sections']) || bytes(JSON.stringify(value)) > 2 * 1024 * 1024) return invalid();
  const sections = { ...value.sections };
  if (!Object.keys(sections).length || Object.keys(sections).some(key => !(TRANSFER_SECTIONS as readonly string[]).includes(key))) return invalid();
  if (!Number.isSafeInteger(value.exportedAt) || !bounded(value.exportedAt, 0, Number.MAX_SAFE_INTEGER)) return invalid();
  if (sections.general !== undefined) {
    const item = sections.general;
    if (!object(item) || !keysOnly(item, ['language', 'closeToTray', 'openExternalNowPlaying', 'suggestExternalFolders', 'statistics']) || typeof item.language !== 'string' || !['de', 'en'].includes(item.language) || !boolean(item.closeToTray)) return invalid();
    if (item.openExternalNowPlaying !== undefined && !boolean(item.openExternalNowPlaying) || item.suggestExternalFolders !== undefined && !boolean(item.suggestExternalFolders)) return invalid();
    if (item.statistics !== undefined && (!object(item.statistics) || !keysOnly(item.statistics, ['enabled', 'percent', 'seconds']) || !boolean(item.statistics.enabled) || !bounded(item.statistics.percent, 1, 100) || !bounded(item.statistics.seconds, 0, 3600))) return invalid();
  }
  if (sections.appearance !== undefined) sections.appearance = validateAppearance(sections.appearance);
  if (sections.audio !== undefined) {
    const item = sections.audio;
    if (!object(item) || !keysOnly(item, ['volume', 'muted', 'replayGain', 'loudnessNormalization', 'equalizer', 'gainGrouping']) || !bounded(item.volume, 0, 100) || !boolean(item.muted) || !boolean(item.replayGain) || !boolean(item.loudnessNormalization)) return invalid();
    if (item.gainGrouping !== undefined && !['track', 'album', 'folder'].includes(item.gainGrouping as string)) return invalid();
    if (item.equalizer !== undefined && (!object(item.equalizer) || !keysOnly(item.equalizer, ['enabled', 'bands', 'preamp']) || !boolean(item.equalizer.enabled) || !Array.isArray(item.equalizer.bands) || item.equalizer.bands.length !== 5 || !item.equalizer.bands.every(gain => bounded(gain, -12, 12)) || !bounded(item.equalizer.preamp, -24, 0))) return invalid();
  }
  if (sections.shortcuts !== undefined) {
    const item = sections.shortcuts;
    if (!object(item) || !keysOnly(item, [...SHORTCUT_ACTIONS, 'fullscreen']) || Object.values(item).some(binding => typeof binding !== 'string')) return invalid();
    const shortcuts = resolveShortcuts(item as Record<string, string>);
    if (validateShortcuts(shortcuts).length) return invalid();
    sections.shortcuts = shortcuts;
  }
  if (sections.layout !== undefined) {
    const item = sections.layout;
    if (!object(item) || !keysOnly(item, ['panelWidths', 'seekbarStyle']) || (item.seekbarStyle !== undefined && item.seekbarStyle !== 'line' && item.seekbarStyle !== 'waveform') || !object(item.panelWidths) || !keysOnly(item.panelWidths, ['sidebar', 'queue']) || !bounded(item.panelWidths.sidebar, 160, 360) || !bounded(item.panelWidths.queue, 240, 560)) return invalid();
  }
  if (sections.device !== undefined) {
    const item = sections.device;
    if (!object(item) || !keysOnly(item, ['outputDevice', 'globalSearch']) || typeof item.outputDevice !== 'string' || !item.outputDevice.trim() || bytes(item.outputDevice) > 1024 || /[\u0000-\u001f\u007f-\u009f]/.test(item.outputDevice)) return invalid();
    if (item.globalSearch !== undefined && (!object(item.globalSearch) || !keysOnly(item.globalSearch, ['enabled', 'binding']) || !boolean(item.globalSearch.enabled) || typeof item.globalSearch.binding !== 'string' || validateShortcut(item.globalSearch.binding) || !/^(Ctrl\+|Alt\+)/.test(item.globalSearch.binding) || item.globalSearch.binding === 'Ctrl+KeyF')) return invalid();
  }
  if (sections.library !== undefined) {
    const item = sections.library;
    if (!object(item) || !keysOnly(item, ['folders']) || !Array.isArray(item.folders) || item.folders.length > 256 || item.folders.some(path => typeof path !== 'string' || !path.trim() || bytes(path) > 32768 || /[\u0000-\u001f\u007f-\u009f]/.test(path))) return invalid();
  }
  return { ...value, sections } as SettingsDocument;
}

export function availableSections(document: SettingsDocument): TransferSection[] {
  return TRANSFER_SECTIONS.filter(section => document.sections[section] !== undefined);
}

export function defaultImportSections(document: SettingsDocument): TransferSection[] {
  return availableSections(document).filter(section => PORTABLE_SECTIONS.includes(section));
}
