export type GainGrouping = 'track' | 'album' | 'folder';
export interface StatisticsSettings { enabled: boolean; percent: number; seconds: number }
export interface EqualizerSettings { enabled: boolean; bands: number[]; preamp: number }
export interface GlobalSearchSettings { enabled: boolean; binding: string }
export interface PlaylistIcon { kind: 'builtin' | 'image'; value: string }
export interface SleepTimer { mode: 'minutes' | 'track' | 'queue'; endsAt: number | null }
export interface StatisticsTrack { trackId: string; title: string; artist: string; album: string; path: string; plays: number; listenedSeconds: number }
export interface Statistics { plays: number; listenedSeconds: number; tracks: StatisticsTrack[]; artists: { artist: string; plays: number; listenedSeconds: number }[] }
export interface ExternalLaunch { id: string; paths: string[]; folders: string[] }
export interface AudioJob { id: string; kind: 'analysis' | 'export'; status: 'running' | 'completed' | 'cancelled' | 'failed'; progress: number; message: string | null; outputPath: string | null }
export interface AnalysisOptions { trackIds: string[]; grouping: GainGrouping; writeTags: boolean }
export interface AudioSelection { trackId: string; start: number; end: number; remove: boolean }
export interface AudioExportOptions extends AudioSelection { outputPath: string }
