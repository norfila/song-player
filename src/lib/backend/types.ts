import type { AppearanceSettings } from '../theme';
import type { SettingsDocument, TransferSection } from '../settings-transfer';
import type { EqualizerSettings, GainGrouping, GlobalSearchSettings, PlaylistIcon, SleepTimer, StatisticsSettings } from './expansion-types';
export type * from './expansion-types';

export interface Track {
  id: string; path: string; title: string; artist: string; album: string;
  albumArtist: string; genre: string; year: number | null;
  trackNumber: number | null; discNumber: number | null; duration: number;
  sampleRate: number | null; channels: number | null; bitrate: number | null;
  format: string; coverPath: string | null; addedAt: number; contentFingerprint?: string | null;
}
export type RepeatMode = 'off' | 'one' | 'all';
export type PlayOrder = 'manual' | 'shuffle_tracks' | 'shuffle_albums' | 'shuffle_folders' | 'title' | 'artist' | 'album' | 'filename' | 'year' | 'duration' | 'added';
export interface QueueEntry { id: string; track: Track }
export interface Session {
  id: string; name: string; entries: QueueEntry[]; currentEntryId: string | null;
  position: number; repeat: RepeatMode; orderMode: PlayOrder;
}
export interface Playlist { id: string; name: string; trackIds: string[]; createdAt: number; pinned: boolean; icon: PlaylistIcon }
export interface Settings {
  language: string; volume: number; muted: boolean; outputDevice: string; replayGain: boolean; loudnessNormalization: boolean;
  closeToTray: boolean; defaultAppReminder: boolean; shortcuts: Record<string, string>; panelWidths: { sidebar: number; queue: number };
  defaultAppHintSeen: boolean; appearance: AppearanceSettings;
  openExternalNowPlaying: boolean; suggestExternalFolders: boolean;
  statistics: StatisticsSettings; equalizer: EqualizerSettings; gainGrouping: GainGrouping; globalSearch: GlobalSearchSettings;
  seekbarStyle?: SeekbarStyle;
}
export type SeekbarStyle = 'line' | 'waveform';
export interface DefaultAppStatus { supported: boolean; missingExtensions: string[] }
export interface EngineInfo { available: boolean; version: string; error: string | null }
export interface Playback {
  sequence: number;
  status: 'stopped' | 'playing' | 'paused'; position: number; duration: number;
  volume: number; muted: boolean; currentEntryId: string | null; trackId: string | null; sessionId: string | null;
}
export interface AppSnapshot {
  revision: number;
  tracks: Track[]; libraryTrackIds: string[]; libraryFolders: string[]; excludedFolders?: string[];
  playlists: Playlist[]; sessions: Session[]; activeSessionId: string;
  settings: Settings; playback: Playback; engine: EngineInfo; warnings: string[];
  settingsUndoAvailable: boolean;
  favoriteTrackIds: string[]; sleepTimer: SleepTimer | null; loudnessValues: { trackId: string; gain: number; peak: number; grouping: GainGrouping; groupKey: string | null }[];
}
export interface AudioDevice { id: string; name: string }
export interface ApiError { code: string; details?: string }
export type ImportMode = 'play' | 'append' | 'next' | 'library';
export type PlayerCommand =
  | { type: 'play_pause' | 'play' | 'pause' | 'stop' | 'next' | 'previous' | 'clear_queue' }
  | { type: 'seek'; position: number }
  | { type: 'set_volume'; volume: number }
  | { type: 'set_muted'; muted: boolean }
  | { type: 'set_repeat'; mode: RepeatMode }
  | { type: 'set_shuffle'; enabled: boolean }
  | { type: 'set_appearance'; appearance: AppearanceSettings }
  | { type: 'acknowledge_default_app_hint' | 'undo_settings_import' }
  | { type: 'import_settings'; document: SettingsDocument; sections: TransferSection[] }
  | { type: 'set_order'; mode: PlayOrder }
  | { type: 'play_entry' | 'remove_entry'; entryId: string }
  | { type: 'move_entry'; entryId: string; beforeEntryId: string | null }
  | { type: 'play_tracks'; trackIds: string[]; orderMode?: PlayOrder }
  | { type: 'append_tracks' | 'enqueue_next_tracks'; trackIds: string[] }
  | { type: 'new_session'; name: string }
  | { type: 'activate_session' | 'delete_session'; sessionId: string }
  | { type: 'delete_sessions'; sessionIds: string[] }
  | { type: 'rename_session'; sessionId: string; name: string }
  | { type: 'save_playlist'; name: string; playlistId?: string }
  | { type: 'delete_playlist' | 'play_playlist'; playlistId: string }
  | { type: 'rename_playlist'; playlistId: string; name: string }
  | { type: 'add_to_playlist'; playlistId: string; trackIds: string[] }
  | { type: 'remove_from_playlist'; playlistId: string; index: number }
  | { type: 'set_language'; language: string }
  | { type: 'remove_library_folder'; path: string }
  | { type: 'set_audio_device'; id: string }
  | { type: 'set_replay_gain' | 'set_loudness_normalization' | 'set_close_to_tray' | 'set_default_app_reminder'; enabled: boolean }
  | { type: 'set_shortcuts'; shortcuts: Record<string, string> }
  | { type: 'set_panel_widths'; sidebar: number; queue: number }
  | { type: 'set_seekbar_style'; style: SeekbarStyle }
  | { type: 'create_playlist'; name: string }
  | { type: 'set_playlist_pinned'; playlistId: string; pinned: boolean }
  | { type: 'set_playlist_icon'; playlistId: string; icon: PlaylistIcon }
  | { type: 'set_playlist_membership'; trackIds: string[]; playlistIds: string[]; preservePlaylistIds?: string[] }
  | { type: 'set_favorite'; trackIds: string[]; favorite: boolean }
  | { type: 'set_library_options'; openExternalNowPlaying: boolean; suggestExternalFolders: boolean }
  | { type: 'set_statistics_settings'; settings: StatisticsSettings }
  | { type: 'set_equalizer'; equalizer: EqualizerSettings }
  | { type: 'set_gain_grouping'; grouping: GainGrouping }
  | { type: 'set_global_search'; enabled: boolean; binding: string }
  | { type: 'set_sleep_timer'; mode: 'minutes' | 'track' | 'queue'; minutes: number }
  | { type: 'cancel_sleep_timer' };

