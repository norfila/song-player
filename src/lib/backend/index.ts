import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { AppSnapshot, AudioDevice, DefaultAppStatus, ImportMode, Playback, PlayerCommand } from './types';
import { DEFAULT_SHORTCUTS } from '../shortcuts';
import { DEFAULT_APPEARANCE } from '../theme';
import type { SettingsDocument, TransferSection } from '../settings-transfer';

import type { AnalysisOptions, AudioExportOptions, AudioSelection, AudioJob, ExternalLaunch, GlobalSearchSettings, PlaylistIcon, Statistics } from './types';

export type { DefaultAppStatus } from './types';

export const isDesktop = isTauri();
export const assetUrl = (path: string): string => isDesktop ? convertFileSrc(path) : '';

const filters = [{ name: 'Audio', extensions: ['mp3', 'flac', 'wav', 'aiff', 'aif', 'm4a', 'm4b', 'aac', 'ogg', 'opus', 'wma', 'wv', 'ape', 'mpc', 'tta', 'ac3', 'dts', 'mka', 'webm', 'caf', 'dsf', 'dff'] }];

function desktopOnly(): never { throw { code: 'desktop_only' }; }

function previewSnapshot(): AppSnapshot {
  return {
    revision: 0,
    tracks: [], libraryTrackIds: [], libraryFolders: [], excludedFolders: [], playlists: [],
    sessions: [{ id: 'preview', name: '', entries: [], currentEntryId: null, position: 0, repeat: 'off', orderMode: 'manual' }],
    activeSessionId: 'preview',
    settings: { language: 'de', volume: 75, muted: false, outputDevice: 'auto', replayGain: false, loudnessNormalization: false, closeToTray: false, defaultAppReminder: true, defaultAppHintSeen: false, appearance: structuredClone(DEFAULT_APPEARANCE), openExternalNowPlaying: true, suggestExternalFolders: true, seekbarStyle: 'line', statistics: { enabled: true, percent: 50, seconds: 240 }, equalizer: { enabled: false, bands: [0, 0, 0, 0, 0], preamp: 0 }, gainGrouping: 'track', globalSearch: { enabled: false, binding: 'Ctrl+Alt+Space' }, shortcuts: { ...DEFAULT_SHORTCUTS }, panelWidths: { sidebar: 220, queue: 310 } },
    playback: { sequence: 0, status: 'stopped', position: 0, duration: 0, volume: 75, muted: false, currentEntryId: null, trackId: null, sessionId: 'preview' },
    engine: { available: false, version: '', error: 'desktop_only' }, warnings: [], settingsUndoAvailable: false, favoriteTrackIds: [], sleepTimer: null, loudnessValues: [],
  };
}

export const api = {
  /** The window starts hidden so the first frame the user sees is the finished view. */
  async showWindow(): Promise<void> {
    if (!isDesktop) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const window = getCurrentWindow();
    await window.show();
    await window.setFocus();
  },
  async getStatisticsYears(): Promise<number[]> {
    return isDesktop ? invoke('get_statistics_years') : [];
  },
  async getStatistics(from: number | null, to: number | null): Promise<Statistics> {
    return isDesktop ? invoke('get_statistics', { from, to }) : { plays: 0, listenedSeconds: 0, tracks: [], artists: [] };
  },
  async clearStatistics(): Promise<void> { await invoke('clear_statistics'); },
  async exportHistory(): Promise<unknown> { return invoke('export_history'); },
  async exportHistoryFile(): Promise<boolean> {
    if (!isDesktop) desktopOnly();
    const path = await save({ defaultPath: 'Song-Player-listening-history.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!path) return false;
    await invoke('export_history_file', { path });
    return true;
  },
  async takeExternalLaunch(): Promise<ExternalLaunch | null> { return isDesktop ? invoke('take_external_launch') : null; },
  async subscribeExternalLaunch(listener: (value: ExternalLaunch) => void): Promise<() => void> { return isDesktop ? listen<ExternalLaunch>('player://external-open', event => listener(event.payload)) : () => {}; },
  async subscribeQuickSearch(listener: () => void): Promise<() => void> { return isDesktop ? listen('player://quick-search', listener) : () => {}; },
  async configureGlobalSearch(settings: GlobalSearchSettings): Promise<void> { if (!isDesktop) desktopOnly(); await invoke('configure_global_search', { ...settings }); },
  async importPlaylistIcon(): Promise<PlaylistIcon | null> {
    if (!isDesktop) desktopOnly();
    const path = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    return path ? invoke('import_playlist_icon', { path }) : null;
  },
  async startLoudnessAnalysis(options: AnalysisOptions): Promise<AudioJob> { return invoke('start_loudness_analysis', options as unknown as Record<string, unknown>); },
  async startAudioExport(options: AudioExportOptions): Promise<AudioJob> { return invoke('start_audio_export', options as unknown as Record<string, unknown>); },
  async getAudioWaveform(trackId: string): Promise<number[]> { return invoke('get_audio_waveform', { trackId }); },
  async getAudioPreview(options: AudioSelection): Promise<string> { return invoke('get_audio_preview', options as unknown as Record<string, unknown>); },
  async getAudioJob(): Promise<AudioJob | null> { return isDesktop ? invoke('get_audio_job') : null; },
  async cancelAudioJob(): Promise<void> { await invoke('cancel_audio_job'); },
  async subscribeAudioJob(listener: (job: AudioJob) => void): Promise<() => void> { return isDesktop ? listen<AudioJob>('player://audio-job', event => listener(event.payload)) : () => {}; },
  async saveAudioExportPath(defaultName: string, format: 'flac' | 'wav'): Promise<string | null> {
    if (!isDesktop) desktopOnly();
    return save({ defaultPath: defaultName, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
  },
  async getSnapshot(): Promise<AppSnapshot> {
    return isDesktop ? invoke('get_snapshot') : previewSnapshot();
  },
  async dispatch(command: PlayerCommand): Promise<AppSnapshot> {
    if (!isDesktop) desktopOnly();
    return invoke('dispatch', { command });
  },
  async importPaths(paths: string[], mode: ImportMode = 'play', recursive = true): Promise<AppSnapshot> {
    if (!isDesktop) desktopOnly();
    return invoke('import_paths', { paths, mode, recursive });
  },
  async openFiles(mode: ImportMode = 'play'): Promise<AppSnapshot | null> {
    if (!isDesktop) desktopOnly();
    const result = await open({ multiple: true, filters });
    return result ? api.importPaths(typeof result === 'string' ? [result] : result, mode) : null;
  },
  async openFolder(mode: ImportMode = 'library'): Promise<AppSnapshot | null> {
    if (!isDesktop) desktopOnly();
    const result = await open({ directory: true, multiple: false });
    return result ? api.importPaths([result], mode) : null;
  },
  async importPlaylist(): Promise<AppSnapshot | null> {
    if (!isDesktop) desktopOnly();
    const path = await open({ multiple: false, filters: [{ name: 'M3U', extensions: ['m3u8', 'm3u'] }] });
    return path ? invoke('import_playlist', { path }) : null;
  },
  async exportPlaylist(playlistId?: string): Promise<void> {
    if (!isDesktop) desktopOnly();
    const path = await save({ defaultPath: 'Song-Player.m3u8', filters: [{ name: 'M3U8', extensions: ['m3u8'] }] });
    if (path) await invoke('export_playlist', { path, playlistId: playlistId ?? null });
  },
  async openConfigurationFile(): Promise<unknown | null> {
    if (!isDesktop) desktopOnly();
    const path = await open({ multiple: false, filters: [{ name: 'Song Player JSON', extensions: ['json'] }] });
    return path ? invoke('read_config_file', { path }) : null;
  },
  async saveConfigurationFile(document: unknown, defaultPath: string): Promise<boolean> {
    if (!isDesktop) desktopOnly();
    const path = await save({ defaultPath, filters: [{ name: 'Song Player JSON', extensions: ['json'] }] });
    if (!path) return false;
    await invoke('write_config_file', { path, document });
    return true;
  },
  async exportSettingsDocument(sections: TransferSection[]): Promise<SettingsDocument> {
    if (!isDesktop) desktopOnly();
    return invoke('export_settings_document', { sections });
  },
  async getAudioDevices(): Promise<AudioDevice[]> {
    return isDesktop ? invoke('get_audio_devices') : [];
  },
  async getDefaultAppStatus(): Promise<DefaultAppStatus> {
    return isDesktop ? invoke('get_default_app_status') : { supported: false, missingExtensions: [] };
  },
  async openDefaultAppsSettings(): Promise<void> {
    if (!isDesktop) desktopOnly();
    await invoke('open_default_apps_settings');
  },
  async quit(): Promise<void> {
    if (!isDesktop) desktopOnly();
    await invoke('quit_app');
  },
  async subscribe(onState: (snapshot: AppSnapshot) => void, onPlayback: (playback: Playback) => void, onError?: (error: unknown) => void): Promise<() => void> {
    if (!isDesktop) return () => undefined;
    const cleanups = await Promise.all([
      listen<AppSnapshot>('player://state', event => onState(event.payload)),
      listen<Playback>('player://playback', event => onPlayback(event.payload)),
      listen('player://error', event => onError?.(event.payload)),
    ]);
    return () => cleanups.forEach(cleanup => cleanup());
  },
};

