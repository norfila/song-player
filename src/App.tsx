import { SettingsPanel } from './features/settings/SettingsPanel';
import type { SettingsDestination } from './features/settings/SettingsPanel';
import { AppearanceEditor } from './features/settings/AppearanceEditor';
import { StatisticsPage } from './features/statistics/StatisticsPage';
import { StatisticsPreferences } from './features/statistics/StatisticsPreferences';
import { AudioEditor, EqualizerPanel, GlobalSearchOptions, LibraryOptionsPanel, LoudnessPanel, SleepTimerDialog } from './features/tools';
import { QuickSearch } from './components/QuickSearch';
import { PlaylistMembershipDialog, PlaylistIconDialog } from './components/PlaylistDialogs';
import { PlaylistIcon } from './components/PlaylistIcon';
import { ExternalFolderOffer } from './components/ExternalFolderOffer';
import { Dialog } from './components/Dialog';
import { resetSearchUsage } from './lib/power-search';
import type { SearchDocument } from './lib/power-search';
import type { ExternalLaunch, Playlist } from './lib/backend/types';
import { DataSettingsPanel } from './features/settings/DataSettingsPanel';
import { PlayerBar } from './features/player/PlayerBar';
import { NowPlaying } from './features/player/NowPlaying';
import { QueuePanel } from './features/queue/QueuePanel';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BarChart3, Heart, Star, Image, AlertCircle, ArrowDownToLine, ArrowLeft, AudioLines, Check, CheckCircle2, ChevronDown, ChevronRight, Disc3, FileAudio2, Folder, FolderMinus, FolderOpen, Headphones, Info, Library, ListEnd, ListMusic, ListPlus, LoaderCircle, Music2, Play, Plus, Search, Settings2, Shuffle, Trash2, X } from 'lucide-react';
import { api, isDesktop } from './lib/backend';
import { SessionCleanupMenu, onlyEmptySession } from './features/queue/SessionCleanupMenu';
import type { DefaultAppStatus } from './lib/backend';
import { findShortcutAction, formatShortcut, resolveShortcuts } from './lib/shortcuts';
import type { AppSnapshot, PlayerCommand, PlayOrder, Track } from './lib/backend/types';
import { basename, parentFolder, time } from './lib/format';
import { isLibraryFolderCovered } from './lib/library-paths';
import { Cover } from './components/Cover';
import { TrackList } from './components/TrackList';
import { TitleBar } from './components/TitleBar';
import { SortControl } from './components/SortControl';
import { DefaultAppNotice } from './components/DefaultAppNotice';
import { PanelResizeHandle } from './components/PanelResizeHandle';
import { mergePlayback, mergeSnapshot } from './lib/state';
import { applyAppearance, DEFAULT_APPEARANCE, parseThemeFile, serializeThemeFile } from './lib/theme';
import type { AppearanceSettings } from './lib/theme';

type Page = 'library' | 'albums' | 'folders' | 'playlists' | 'sessions' | 'settings' | 'favorites' | 'statistics';
type NameDialog = { kind: 'playlist' | 'queue_playlist' | 'session' | 'rename_playlist' | 'rename_session'; id?: string; value: string };
const immediatePlaybackCommands = new Set<PlayerCommand['type']>([
  'play_pause', 'play', 'pause', 'stop', 'next', 'previous', 'seek', 'play_entry',
  'set_volume', 'set_muted', 'set_repeat', 'set_shuffle', 'set_order',
  'set_replay_gain', 'set_loudness_normalization', 'set_panel_widths',
  'set_appearance', 'set_default_app_reminder', 'acknowledge_default_app_hint',
]);

export default function App() {
  const { t, i18n } = useTranslation();
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [bootError, setBootError] = useState(false);
  const [page, setPage] = useState<Page>('library');
  const [settingsDestination, setSettingsDestination] = useState<SettingsDestination | null>(null);
  const handleSettingsDestination = useCallback(() => setSettingsDestination(null), []);
  const [playlistId, setPlaylistId] = useState<string | null>(null);
  const [group, setGroup] = useState<{ name: string; tracks: Track[]; path?: string } | null>(null);
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<PlayOrder>('title');
  const [descending, setDescending] = useState(false);
  const [selection, setSelection] = useState<Set<number>>(new Set());
  const lastSelected = useRef(0);
  const [quickSearch, setQuickSearch] = useState(false);
  const [membershipTracks, setMembershipTracks] = useState<Track[] | null>(null);
  const [iconPlaylist, setIconPlaylist] = useState<Playlist | null>(null);
  const [editorTrack, setEditorTrack] = useState<Track | null>(null);
  const [equalizerOpen, setEqualizerOpen] = useState(false);
  const [sleepOpen, setSleepOpen] = useState(false);
  const [externalFolders, setExternalFolders] = useState<string[]>([]);
  const pendingExternalFolders = useMemo(() => externalFolders.filter(folder => !isLibraryFolderCovered(folder, snapshot?.libraryFolders ?? [], snapshot?.excludedFolders ?? [])), [externalFolders, snapshot?.libraryFolders, snapshot?.excludedFolders]);
  const handledLaunches = useRef(new Set<string>());
  const snapshotRef = useRef(snapshot); snapshotRef.current = snapshot;
  const [queueVisible, setQueueVisible] = useState(true);
  const [nowPlaying, setNowPlaying] = useState(false);
  const [launchChecked, setLaunchChecked] = useState(false);
  const windowShown = useRef(false);
  const [importMenu, setImportMenu] = useState(false);
  const [nameDialog, setNameDialog] = useState<NameDialog | null>(null);
  const [details, setDetails] = useState<Track | null>(null);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [busy, setBusy] = useState(0);
  const [busyVisible, setBusyVisible] = useState(false);
  const isBusy = busy > 0;
  const [dropActive, setDropActive] = useState(false);
  const [defaultAppStatus, setDefaultAppStatus] = useState<DefaultAppStatus | null>(null);
  const [defaultHintVisible, setDefaultHintVisible] = useState(false);
  const defaultHintHandled = useRef(false);
  const [appearancePreview, setAppearancePreview] = useState<AppearanceSettings | null>(null);
  const [panelWidths, setPanelWidths] = useState({ sidebar: 220, queue: 310 });
  const [viewportWidth, setViewportWidth] = useState(window.innerWidth);
  const searchInput = useRef<HTMLInputElement>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const notify = useCallback((text: string, error = false) => {
    clearTimeout(noticeTimer.current);
    setNotice({ text, error });
    noticeTimer.current = setTimeout(() => setNotice(null), error ? 8000 : 4000);
  }, []);

  const reportError = useCallback((error: unknown) => {
    let value = error as { code?: string; message?: string; details?: string };
    if (typeof error === 'string') {
      try { value = JSON.parse(error); } catch { value = { code: error }; }
    }
    const key = `errors.${value?.code || 'unknown'}`;
    notify(i18n.t(i18n.exists(key) ? key : 'errors.unknown'), true);
  }, [i18n, notify]);
  const reportErrorRef = useRef(reportError);
  reportErrorRef.current = reportError;

  const accept = useCallback((state: AppSnapshot | null) => {
    if (state) setSnapshot(previous => mergeSnapshot(previous, state));
  }, []);
  const run = useCallback(async (operation: () => Promise<AppSnapshot | null | void>, showProgress = true) => {
    if (showProgress) setBusy(n => n + 1);
    try { const state = await operation(); if (state) accept(state); }
    catch (error) { reportError(error); }
    finally { if (showProgress) setBusy(n => n - 1); }
  }, [accept, reportError]);
  const command = useCallback((value: PlayerCommand) => run(() => api.dispatch(value), !immediatePlaybackCommands.has(value.type)), [run]);
  const featureCommand = useCallback(async (value: PlayerCommand) => { accept(await api.dispatch(value)); }, [accept]);
  const shortcuts = useMemo(() => resolveShortcuts(snapshot?.settings.shortcuts), [snapshot?.settings.shortcuts]);

  useEffect(() => {
    if (!isBusy) { setBusyVisible(false); return; }
    const timer = setTimeout(() => setBusyVisible(true), 300);
    return () => clearTimeout(timer);
  }, [isBusy]);

  useEffect(() => {
    if (!isDesktop) return;
    let alive = true;
    const refresh = () => { void api.getDefaultAppStatus().then(status => { if (alive) setDefaultAppStatus(status); }).catch(() => { /* A reminder must not prevent playback. */ }); };
    refresh();
    window.addEventListener('focus', refresh);
    return () => { alive = false; window.removeEventListener('focus', refresh); };
  }, []);
  useEffect(() => { if (snapshot?.settings.panelWidths) setPanelWidths(snapshot.settings.panelWidths); }, [snapshot?.settings.panelWidths.sidebar, snapshot?.settings.panelWidths.queue]);
  useEffect(() => {
    if (!isDesktop || !snapshot || !defaultAppStatus?.supported || !snapshot.settings.defaultAppReminder || defaultHintHandled.current) return;
    defaultHintHandled.current = true;
    setDefaultHintVisible(defaultAppStatus.missingExtensions.length > 0);
  }, [snapshot?.settings.defaultAppReminder, defaultAppStatus]);
  useEffect(() => {
    try { applyAppearance(appearancePreview || snapshot?.settings.appearance || DEFAULT_APPEARANCE); }
    catch { applyAppearance(DEFAULT_APPEARANCE); }
  }, [appearancePreview, snapshot?.settings.appearance]);
  useEffect(() => {
    const resize = () => setViewportWidth(window.innerWidth);
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, []);

  useEffect(() => {
    let alive = true;
    let unsubscribe: (() => void) | undefined;
    void api.getSnapshot().then(state => { if (alive) {
      // Open straight into Now Playing when a restored queue has a current track.
      if (state.playback.trackId && state.tracks.some(track => track.id === state.playback.trackId)) setNowPlaying(true);
      accept(state); setBootError(false);
    } }).catch(() => { if (alive) setBootError(true); });
    void api.subscribe(state => { if (alive) accept(state); }, playback => { if (alive) setSnapshot(previous => mergePlayback(previous, playback)); }, error => { if (alive) reportErrorRef.current(error); }).then(off => { if (alive) unsubscribe = off; else off(); }).catch(error => reportErrorRef.current(error));
    return () => { alive = false; unsubscribe?.(); clearTimeout(noticeTimer.current); };
  }, []);

  useEffect(() => { if (snapshot?.settings.language && snapshot.settings.language !== i18n.language) void i18n.changeLanguage(snapshot.settings.language); }, [snapshot?.settings.language, i18n]);
  useEffect(() => { document.documentElement.lang = i18n.language; }, [i18n.language]);
  useEffect(() => { if (nameDialog) setTimeout(() => { nameInput.current?.focus(); nameInput.current?.select(); }, 0); }, [nameDialog?.kind, nameDialog?.id]);
  useEffect(() => {
    if (!nameDialog && !details) return;
    const previous = document.activeElement as HTMLElement | null;
    const frame = requestAnimationFrame(() => {
      const modal = document.querySelector<HTMLElement>('[role="dialog"]');
      (nameDialog ? nameInput.current : modal?.querySelector<HTMLElement>('button'))?.focus();
    });
    const trapFocus = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') return;
      const controls = Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"] button:not(:disabled), [role="dialog"] input:not(:disabled), [role="dialog"] select:not(:disabled)'));
      const first = controls[0]; const last = controls.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    };
    window.addEventListener('keydown', trapFocus);
    return () => { cancelAnimationFrame(frame); window.removeEventListener('keydown', trapFocus); if (previous?.isConnected) previous.focus(); };
  }, [!!nameDialog, !!details]);
  useEffect(() => {
    if (!isDesktop) return;
    let alive = true;
    let off: (() => void) | undefined;
    void import('@tauri-apps/api/webview').then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type === 'over' || event.payload.type === 'enter') setDropActive(true);
      else if (event.payload.type === 'leave') setDropActive(false);
      else if (event.payload.type === 'drop') {
        setDropActive(false);
        const paths = event.payload.paths;
        void run(() => api.importPaths(paths, 'append', true));
      }
    })).then(unlisten => { if (alive) off = unlisten; else unlisten(); }).catch(reportError);
    return () => { alive = false; off?.(); };
  }, [reportError, run]);

  const loaded = !!snapshot;
  useEffect(() => {
    if (!isDesktop || !loaded) return;
    let alive = true; const cleanups: (() => void)[] = [];
    const external = (launch: ExternalLaunch) => {
      if (!alive || handledLaunches.current.has(launch.id)) return;
      handledLaunches.current.add(launch.id);
      if (handledLaunches.current.size > 128) handledLaunches.current.delete(handledLaunches.current.values().next().value!);
      const settings = snapshotRef.current?.settings;
      if (settings?.openExternalNowPlaying) setNowPlaying(true);
      if (settings?.suggestExternalFolders && launch.folders.length) setExternalFolders(launch.folders);
    };
    void api.subscribeExternalLaunch(external).then(async off => { if (!alive) { off(); return; } cleanups.push(off); const pending = await api.takeExternalLaunch(); if (pending) external(pending); }).catch(reportError).finally(() => setLaunchChecked(true));
    void api.subscribeQuickSearch(() => { if (alive) setQuickSearch(true); }).then(off => { if (alive) cleanups.push(off); else off(); }).catch(reportError);
    return () => { alive = false; cleanups.forEach(off => off()); };
  }, [loaded, reportError]);

  // The window starts hidden (tauri.conf.json). Show it once the first real view
  // is rendered with the saved language and appearance, so startup has no flicker.
  const viewReady = bootError || (loaded && launchChecked && i18n.language === snapshot?.settings.language);
  useEffect(() => {
    if (!viewReady || windowShown.current) return;
    const timer = setTimeout(() => { windowShown.current = true; void api.showWindow().catch(reportError); }, 16);
    return () => clearTimeout(timer);
  }, [viewReady, reportError]);

  const playback = snapshot?.playback;
  const activePlaylist = snapshot?.playlists.find(list => list.id === playlistId);
  const libraryTracks = useMemo(() => {
    const ids = new Set(snapshot?.libraryTrackIds || []);
    return snapshot?.tracks.filter(track => ids.has(track.id)) || [];
  }, [snapshot?.tracks, snapshot?.libraryTrackIds]);
  const playlistTracks = useMemo(() => {
    const map = new Map(snapshot?.tracks.map(track => [track.id, track]));
    return activePlaylist?.trackIds.map(id => map.get(id)).filter((track): track is Track => !!track) || [];
  }, [activePlaylist, snapshot?.tracks]);
  const displayedTracks = useMemo(() => {
    const source = group?.tracks || (activePlaylist ? playlistTracks : page === 'favorites' ? (snapshot?.tracks.filter(track => snapshot.favoriteTrackIds?.includes(track.id)) || []) : libraryTracks);
    const query = search.trim().toLocaleLowerCase(i18n.language);
    const tracks = query ? source.filter(track => `${track.title} ${track.artist} ${track.album} ${track.path}`.toLocaleLowerCase(i18n.language).includes(query)) : [...source];
    // A playlist retains its explicitly saved order until the user selects another sort.
    if (activePlaylist && sort === 'manual') return descending ? tracks.reverse() : tracks;
    const compare = new Intl.Collator(i18n.language, { numeric: true, sensitivity: 'base' });
    tracks.sort((a, b) => {
      let result = 0;
      if (sort === 'duration') result = a.duration - b.duration;
      else if (sort === 'year') result = (a.year || 0) - (b.year || 0);
      else if (sort === 'added') result = b.addedAt - a.addedAt;
      else if (sort === 'album') result = compare.compare(a.album, b.album) || (a.discNumber || 0) - (b.discNumber || 0) || (a.trackNumber || 0) - (b.trackNumber || 0);
      else if (sort === 'artist') result = compare.compare(a.artist, b.artist) || compare.compare(a.title, b.title);
      else result = compare.compare(sort === 'filename' ? basename(a.path) : a.title, sort === 'filename' ? basename(b.path) : b.title);
      return descending ? -result : result;
    });
    return tracks;
  }, [group, activePlaylist, playlistTracks, libraryTracks, page, snapshot?.favoriteTrackIds, snapshot?.tracks, search, sort, descending, i18n.language]);

  const groups = useMemo(() => {
    const grouped = new Map<string, { name: string; subtitle: string; tracks: Track[] }>();
    for (const track of libraryTracks) {
      const key = page === 'folders' ? parentFolder(track.path) : `${track.albumArtist || track.artist}\u0000${track.album || ''}`;
      if (!grouped.has(key)) grouped.set(key, { name: page === 'folders' ? basename(parentFolder(track.path)) : track.album || t('library.unknownAlbum'), subtitle: page === 'folders' ? parentFolder(track.path) : track.albumArtist || track.artist || t('library.unknownArtist'), tracks: [] });
      grouped.get(key)!.tracks.push(track);
    }
    return [...grouped.entries()].filter(([, entry]) => `${entry.name} ${entry.subtitle}`.toLocaleLowerCase(i18n.language).includes(search.toLocaleLowerCase(i18n.language))).sort((a, b) => a[1].name.localeCompare(b[1].name, i18n.language, { numeric: true }));
  }, [libraryTracks, page, search, i18n.language, t]);

  const selectionIdentity = useMemo(() => displayedTracks.map(track => track.id).join('\u0000'), [displayedTracks]);
  useEffect(() => { setSelection(new Set()); lastSelected.current = 0; }, [page, playlistId, group, search, sort, descending, selectionIdentity]);

  const navigate = (nextPage: Page, id: string | null = null) => {
    setPage(nextPage); setPlaylistId(id); setGroup(null); setSearch(''); setSort(id ? 'manual' : 'title'); setDescending(false);
  };
  const openStatisticsSettings = () => {
    setSettingsDestination({ category: 'statistics', id: 'statistics-preferences' });
    setNowPlaying(false);
    navigate('settings');
  };
  const playTracks = (tracks: Track[]) => { if (tracks.length) void command({ type: 'play_tracks', trackIds: tracks.map(track => track.id) }); };
  const appendTracks = (tracks: Track[]) => { if (tracks.length) void command({ type: 'append_tracks', trackIds: tracks.map(track => track.id) }); };
  const playFromIndex = (index: number) => playTracks(displayedTracks.slice(index));
  const closeNowPlaying = () => {
    setNowPlaying(false);
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('.playerbar-expand')?.focus());
  };
  const toggleQueue = () => {
    setNowPlaying(false);
    setQueueVisible(visible => !visible);
  };

  useEffect(() => {
    const handle = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const element = e.target as HTMLElement;
      const input = element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement || element.isContentEditable;
      if (e.key === 'Escape') { setNameDialog(null); setDetails(null); setImportMenu(false); return; }
      if (nameDialog || details || document.querySelector('[aria-modal="true"]') || element.closest('[data-shortcut-recorder], [role="menu"], [role="menubar"], [role="combobox"], [role="listbox"]')) return;
      const action = findShortcutAction(e, shortcuts);
      const enteringText = input && e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey;
      if (action === 'search' && !enteringText) { e.preventDefault(); if (!e.repeat) setQuickSearch(true); return; }
      if (input) return;
      if (element.closest('button') && ['Space', 'Enter'].includes(e.code) && !e.ctrlKey && !e.altKey && !e.metaKey) return;
      if (!action) return;
      e.preventDefault();
      if (e.repeat && action !== 'seekBack' && action !== 'seekForward') return;
      switch (action) {
        case 'openFiles': void run(() => api.openFiles('play')); break;
        case 'addFolder': void run(() => api.openFolder('library')); break;
        case 'search': setQuickSearch(true); break;
        case 'nowPlaying': if (nowPlaying) closeNowPlaying(); else setNowPlaying(true); break;
        case 'toggleQueue': toggleQueue(); break;
        case 'playPause': void command({ type: 'play_pause' }); break;
        case 'previous': void command({ type: 'previous' }); break;
        case 'next': void command({ type: 'next' }); break;
        case 'seekBack': case 'seekForward': void command({ type: 'seek', position: Math.max(0, Math.min(playback?.duration || 0, (playback?.position || 0) + (action === 'seekForward' ? 5 : -5))) }); break;
        case 'quit': if (isDesktop) void run(() => api.quit()); break;
      }
    };
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [command, run, playback?.position, playback?.duration, nameDialog, details, shortcuts, nowPlaying, page]);

  const submitName = (e: React.FormEvent) => {
    e.preventDefault(); if (!nameDialog) return;
    const name = nameDialog.value.trim(); if (!name) return;
    const value: PlayerCommand = nameDialog.kind === 'playlist' ? { type: 'create_playlist', name } : nameDialog.kind === 'queue_playlist' ? { type: 'save_playlist', name } : nameDialog.kind === 'session' ? { type: 'new_session', name } : nameDialog.kind === 'rename_playlist' ? { type: 'rename_playlist', playlistId: nameDialog.id!, name } : { type: 'rename_session', sessionId: nameDialog.id!, name };
    void run(async () => {
      const state = await api.dispatch(value);
      if (nameDialog.kind === 'playlist') {
        const existing = new Set(snapshot?.playlists.map(list => list.id));
        const created = state.playlists.find(list => !existing.has(list.id) && list.name === name);
        if (created) navigate('playlists', created.id);
      }
      setNameDialog(null); return state;
    });
  };


  const trackSearchItems = useMemo<SearchDocument[]>(() => (snapshot?.tracks || []).map(track => ({ id: `track:${track.id}`, kind: 'track', label: track.title, detail: [track.artist, track.album].filter(Boolean).join(' · '), keywords: basename(track.path) })), [snapshot?.tracks]);
  const titlebar = <TitleBar snapshot={snapshot} onCommand={command} onRun={run} onError={reportError} onNowPlaying={() => setNowPlaying(true)} onSettings={() => { setNowPlaying(false); navigate('settings'); }} onToggleQueue={toggleQueue} queueVisible={queueVisible} />;

  if (!snapshot) return <>{titlebar}<div className="boot-screen"><div className="brand-mark"><AudioLines /></div><h1>{t('app.productName')}</h1>{bootError ? <><p>{t('errors.unknown')}</p><button className="button primary" onClick={() => void run(async () => { const state = await api.getSnapshot(); setBootError(false); return state; })}>{t('app.retry')}</button></> : <p><LoaderCircle className="spin" size={16} />{t('app.loading')}</p>}</div></>;

  const playing = playback?.status === 'playing';
  const pickedTracks = displayedTracks.filter((_, index) => selection.has(index));
  const tablePage = (page === 'library' || page === 'favorites') || !!activePlaylist || !!group;
  const pageTitle = group?.name || activePlaylist?.name || t(page === 'favorites' ? 'workspace.favorites' : page === 'statistics' ? 'workspace.statistics' : page === 'library' ? 'library.title' : page === 'albums' ? 'library.albumTitle' : page === 'folders' ? 'library.folderTitle' : page === 'playlists' ? 'playlist.title' : page === 'sessions' ? 'session.title' : 'settings.title');
  const pageSubtitle = activePlaylist || group || page === 'library' ? t('library.tracks', { count: displayedTracks.length }) : page === 'sessions' ? t('session.subtitle') : '';
  const showQueue = queueVisible;
  const inlineQueue = showQueue && viewportWidth > 1000;
  const sidebarMax = Math.max(160, Math.min(360, viewportWidth - 400 - (inlineQueue ? 240 : 0)));
  const sidebarWidth = Math.max(160, Math.min(sidebarMax, panelWidths.sidebar));
  const queueMax = Math.max(240, Math.min(560, viewportWidth - sidebarWidth - (inlineQueue ? 400 : 120)));
  const queueWidth = Math.max(240, Math.min(queueMax, panelWidths.queue));
  const resizePanel = (panel: 'sidebar' | 'queue', width: number, persist = false) => {
    const widths = { ...panelWidths, [panel]: width };
    setPanelWidths(widths);
    if (persist && isDesktop) void command({ type: 'set_panel_widths', ...widths });
  };

  const currentTrack = snapshot.tracks.find(track => track.id === playback?.trackId);
  const canAddContainingFolder = (track: Track) => !isLibraryFolderCovered(parentFolder(track.path), snapshot.libraryFolders, snapshot.excludedFolders);
  const removeFolder = (path: string) => { void run(async () => { const state = await api.dispatch({ type: 'remove_library_folder', path }); setGroup(current => current?.path === path ? null : current); notify(t('library.folderRemoved', { name: basename(path) })); return state; }); };
  const addContainingFolder = (track: Track) => { void run(() => api.importPaths([parentFolder(track.path)], 'library', true)); };
  const actions: { id: string; label: string; detail: string; keywords: string; run: () => void; disabled?: boolean }[] = [
    { id: 'next', label: t('player.next'), detail: t('native.playback'), keywords: 'next weiter nächsten', run: () => void command({ type: 'next' }), disabled: !playback?.trackId },
    { id: 'previous', label: t('player.previous'), detail: t('native.playback'), keywords: 'previous zurück vorherigen', run: () => void command({ type: 'previous' }), disabled: !playback?.trackId },
    { id: 'playPause', label: t(playing ? 'player.pause' : 'player.play'), detail: t('native.playback'), keywords: 'play pause abspielen pausieren', run: () => void command({ type: 'play_pause' }) },
    { id: 'shuffle', label: t('player.shuffle'), detail: t('native.playback'), keywords: 'shuffle random mischen zufällig', run: () => void command({ type: 'set_shuffle', enabled: !snapshot.sessions.find(session => session.id === snapshot.activeSessionId)?.orderMode.startsWith('shuffle') }) },
    ...(['off','all','one'] as const).map(mode => ({ id: `repeat-${mode}`, label: t(mode === 'off' ? 'player.repeatOff' : mode === 'all' ? 'player.repeatAll' : 'player.repeatOne'), detail: t('player.repeat'), keywords: mode === 'off' ? 'repeat loop wiederholen nächsten next sequential' : 'repeat loop wiederholen', run: () => void command({ type: 'set_repeat', mode }) })),
    { id: 'settings', label: t('settings.title'), detail: t('window.view'), keywords: 'settings einstellungen', run: () => { setNowPlaying(false); navigate('settings'); } },
    { id: 'statistics', label: t('workspace.statistics'), detail: t('window.view'), keywords: 'stats history statistik gehört', run: () => { setNowPlaying(false); navigate('statistics'); } },
    { id: 'favorites', label: t('workspace.favorites'), detail: t('window.view'), keywords: 'favorites favoriten herz', run: () => { setNowPlaying(false); navigate('favorites'); } },
    { id: 'nowPlaying', label: t('player.expand'), detail: t('window.view'), keywords: 'now playing musik aktuelle wiedergabe', run: () => setNowPlaying(true) },
    { id: 'sleep', label: t('features.sleep.title'), detail: t('queue.title'), keywords: 'sleep timer einschlafen schlaftimer', run: () => setSleepOpen(true) },
    { id: 'favorite-current', label: t(snapshot.favoriteTrackIds.includes(currentTrack?.id || '') ? 'workspace.unfavorite' : 'workspace.favorite'), detail: t('workspace.favorites'), keywords: 'favorite favorisieren herz', run: () => { if (currentTrack) void command({ type: 'set_favorite', trackIds: [currentTrack.id], favorite: !snapshot.favoriteTrackIds.includes(currentTrack.id) }); }, disabled: !currentTrack },
    { id: 'equalizer', label: t('workspace.equalizer'), detail: t('settings.audio'), keywords: 'equalizer eq bass höhen klang', run: () => setEqualizerOpen(true) },
    { id: 'edit', label: t('workspace.editAudio'), detail: t('native.file'), keywords: 'edit cut trim schneiden kürzen', run: () => { if (currentTrack) setEditorTrack(currentTrack); }, disabled: !currentTrack },
    { id: 'membership', label: t('workspace.membership'), detail: t('nav.playlists'), keywords: 'playlist hinzufügen entfernen wiedergabeliste', run: () => { if (currentTrack) setMembershipTracks([currentTrack]); }, disabled: !currentTrack },
    { id: 'open', label: t('native.openFiles'), detail: t('native.file'), keywords: 'open öffnen datei', run: () => void run(() => api.openFiles()) },
    { id: 'folder', label: t('native.openFolder'), detail: t('native.file'), keywords: 'folder ordner bibliothek', run: () => void run(() => api.openFolder()) },
  ];
  const searchItems: SearchDocument[] = actions.map(action => ({ ...action, kind: 'action' as const }));

  const folderOffer = !!pendingExternalFolders.length && snapshot.settings.suggestExternalFolders && <ExternalFolderOffer folders={pendingExternalFolders} onClose={() => setExternalFolders(current => current === externalFolders ? [] : current)} onAdd={async () => { accept(await api.importPaths(pendingExternalFolders, 'library', true)); }} />;

  return <><div className="application-frame">{titlebar}<div className="application-view">{nowPlaying ? <NowPlaying snapshot={snapshot} onTimerCommand={featureCommand} onCommand={command} onClose={closeNowPlaying} onDetails={setDetails} onPlaylist={track => setMembershipTracks([track])} onFavorite={track => void command({ type: 'set_favorite', trackIds: [track.id], favorite: !snapshot.favoriteTrackIds?.includes(track.id) })} onEdit={setEditorTrack} onEqualizer={() => setEqualizerOpen(true)} onAddFolder={addContainingFolder} canAddFolder={!!currentTrack && canAddContainingFolder(currentTrack)} /> : <div className={`app-shell ${showQueue ? 'with-queue' : ''}`} style={{ '--sidebar-width': `${sidebarWidth}px`, '--queue-width': `${queueWidth}px` } as React.CSSProperties} onDragOver={e => { if (e.dataTransfer.types.includes('Files')) { e.preventDefault(); setDropActive(true); } }} onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setDropActive(false); }} onDrop={e => { if (!isDesktop && e.dataTransfer.files.length) { e.preventDefault(); setDropActive(false); notify(t('errors.desktop_only'), true); } }}>
    <aside className="sidebar" id="library-navigation">
      <div className="sidebar-heading">{t('nav.collection')}</div>
      <nav aria-label={t('nav.collection')} className="main-nav">
        {([{ key: 'library', icon: Library }, { key: 'albums', icon: Disc3 }, { key: 'folders', icon: FolderOpen }] as const).map(item => <button key={item.key} className={`nav-item ${page === item.key && !group ? 'active' : ''}`} onClick={() => navigate(item.key)}><item.icon size={18} /><span>{t(`nav.${item.key}`)}</span>{item.key === 'library' && libraryTracks.length > 0 && <span className="nav-count">{libraryTracks.length}</span>}</button>)}
      </nav>
      <button className={`nav-item ${page === 'favorites' ? 'active' : ''}`} onClick={() => navigate('favorites')}><Heart size={18} /><span>{t('workspace.favorites')}</span></button>
      <button className={`nav-item ${page === 'statistics' ? 'active' : ''}`} onClick={() => navigate('statistics')}><BarChart3 size={18} /><span>{t('workspace.statistics')}</span></button>
      <div className="sidebar-section-title"><button onClick={() => navigate('playlists')}>{t('nav.playlists')}</button><button className="icon-button small" aria-label={t('nav.newPlaylist')} title={t('nav.newPlaylist')} onClick={() => setNameDialog({ kind: 'playlist', value: '' })}><Plus size={16} /></button></div>
      <div className="sidebar-playlists">{snapshot.playlists.some(list => list.pinned !== false) ? snapshot.playlists.filter(list => list.pinned !== false).map(list => <button key={list.id} className={`nav-item playlist-nav ${playlistId === list.id ? 'active' : ''}`} onClick={() => navigate('playlists', list.id)}><PlaylistIcon icon={list.icon} size={17} /><span className="truncate">{list.name}</span></button>) : <button className="sidebar-create" onClick={() => setNameDialog({ kind: 'playlist', value: '' })}><span className="dashed-icon"><Plus size={16} /></span><span>{t('playlist.new')}</span></button>}</div>
      <button className={`nav-item session-nav ${page === 'sessions' ? 'active' : ''}`} onClick={() => navigate('sessions')}><Headphones size={18} /><span>{t('nav.sessions')}</span><span className="nav-count">{snapshot.sessions.length}</span></button>
      <div className="sidebar-bottom"><button className={`nav-item ${page === 'settings' ? 'active' : ''}`} onClick={() => navigate('settings')}><Settings2 size={18} /><span>{t('nav.settings')}</span></button></div>
    </aside>
    <PanelResizeHandle value={sidebarWidth} min={160} max={sidebarMax} defaultValue={220} label={t('settings.resizeSidebar')} controls="library-navigation" direction="right" style={{ left: 'var(--sidebar-width)' }} onChange={width => resizePanel('sidebar', width)} onCommit={width => resizePanel('sidebar', width, true)} />

    <main className="main-pane">
      <header className={`topbar ${page === 'settings' ? 'settings-header' : ''}`}>{page === 'settings' ? <div className="settings-topbar"><button className="icon-button" title={t('app.back')} aria-label={t('app.back')} onClick={() => navigate('library')}><ArrowLeft size={18} aria-hidden="true" /></button><h1>{t('settings.title')}</h1></div> : <div className="search-field"><Search size={17} /><input ref={searchInput} value={search} onChange={e => setSearch(e.target.value)} placeholder={t('app.search')} aria-label={t('app.searchLabel')} />{search ? <button className="icon-button small" aria-label={t('app.resetSearch')} onClick={() => setSearch('')}><X size={15} /></button> : <kbd>{formatShortcut(shortcuts.search)}</kbd>}</div>}<div className="topbar-actions">{busyVisible && <LoaderCircle size={17} className="spin busy-indicator" aria-label={t('app.busy')} />}<button className={`icon-button ${queueVisible ? 'is-on' : ''}`} title={t(queueVisible ? 'app.hideQueue' : 'app.showQueue')} aria-label={t(queueVisible ? 'app.hideQueue' : 'app.showQueue')} aria-expanded={queueVisible} aria-controls="active-queue" onClick={toggleQueue}><ListEnd size={20} /></button></div></header>
      <div className={`main-scroll ${page === 'settings' ? 'settings-scroll' : ''}`}>
        {isDesktop && snapshot.settings.defaultAppReminder && defaultHintVisible && defaultAppStatus?.supported && defaultAppStatus.missingExtensions.length > 0 && <DefaultAppNotice status={defaultAppStatus}
          onOpen={() => { void run(async () => { await api.openDefaultAppsSettings(); setDefaultHintVisible(false); }, false); }}
          onLater={() => setDefaultHintVisible(false)}
          onDontRemind={() => { void run(async () => { const state = await api.dispatch({ type: 'set_default_app_reminder', enabled: false }); setDefaultHintVisible(false); return state; }, false); }} />}
        {!isDesktop && <div className="preview-banner"><Info size={15} /><span><strong>{t('app.preview')}</strong><span className="preview-detail"> · {t('app.previewDescription')}</span></span></div>}
        {isDesktop && !snapshot.engine.available && page !== 'settings' && <div className="engine-banner"><AlertCircle size={16} /><span>{t('player.noEngine')}</span><button onClick={() => navigate('settings')}>{t('player.toSettings')}<ChevronRight size={14} /></button></div>}
        <div className={`page-content ${page === 'settings' ? 'settings-page' : ''}`}>
          {(activePlaylist || group) && <button className="back-button" onClick={() => group ? setGroup(null) : navigate('playlists')}><ArrowLeft size={15} />{t('app.back')}</button>}
          {page !== 'settings' && <section className={`page-heading ${page === 'library' ? 'library-heading' : ''}`}>
            <div className="page-heading-row"><div><h1>{pageTitle}</h1>{pageSubtitle && <p>{pageSubtitle}</p>}</div></div>
          </section>}

          {(tablePage || page === 'albums' || page === 'folders') && <div className="library-toolbar"><div className="toolbar-main"><button className="button primary" disabled={displayedTracks.length === 0} onClick={() => playTracks(displayedTracks)}><Play size={15} fill="currentColor" />{t('library.playAll')}</button><button className="button secondary shuffle-button" disabled={displayedTracks.length === 0} onClick={() => void command({ type: 'play_tracks', trackIds: displayedTracks.map(track => track.id), orderMode: 'shuffle_tracks' })}><Shuffle size={16} />{t('library.shuffle')}</button>{page === 'folders' && group?.path && <button className="button quiet" title={t('library.removeFolderHint')} onClick={() => removeFolder(group.path!)}><FolderMinus size={16} />{t('library.removeFolder')}</button>}</div><div className="toolbar-end">
            {activePlaylist && <><button className={`icon-button ${activePlaylist.pinned ? 'is-on' : ''}`} title={t('workspace.pinPlaylist')} aria-label={t('workspace.pinPlaylist')} aria-pressed={activePlaylist.pinned} onClick={() => void command({ type: 'set_playlist_pinned', playlistId: activePlaylist.id, pinned: !activePlaylist.pinned })}><Star size={17} fill={activePlaylist.pinned ? 'currentColor' : 'none'} /></button><button className="icon-button" title={t('workspace.changeIcon')} aria-label={t('workspace.changeIcon')} onClick={() => setIconPlaylist(activePlaylist)}><Image size={17} /></button><button className="icon-button" title={t('playlist.export')} onClick={() => void run(() => api.exportPlaylist(activePlaylist.id))}><ArrowDownToLine size={17} /></button><button className="icon-button" title={t('playlist.rename')} onClick={() => setNameDialog({ kind: 'rename_playlist', id: activePlaylist.id, value: activePlaylist.name })}><Settings2 size={17} /></button><button className="icon-button danger" title={t('playlist.delete')} onClick={() => void run(async () => { const state = await api.dispatch({ type: 'delete_playlist', playlistId: activePlaylist.id }); navigate('playlists'); return state; })}><Trash2 size={16} /></button></>}
            <div className="menu-wrap"><button className="button quiet import-button" onClick={() => setImportMenu(!importMenu)}><Plus size={17} /><span>{t('library.import')}</span><ChevronDown size={13} /></button>{importMenu && <><button className="menu-dismiss" aria-label={t('app.close')} onClick={() => setImportMenu(false)} /><div className="context-menu import-context"><button onClick={() => { setImportMenu(false); void run(() => api.openFolder('library')); }}><FolderOpen size={16} />{t('library.addFolder')}</button><button onClick={() => { setImportMenu(false); void run(() => api.openFiles('play')); }}><FileAudio2 size={16} />{t('library.openFiles')}</button><button onClick={() => { setImportMenu(false); void run(() => api.openFolder('play')); }}><Play size={16} />{t('library.openFolder')}</button><div className="menu-divider" /><button onClick={() => { setImportMenu(false); void run(() => api.importPlaylist()); }}><ListMusic size={16} />{t('library.importPlaylist')}</button></div></>}</div>
          </div></div>}

          {tablePage && <>
            {displayedTracks.length > 0 && <div className="list-controls"><div>{selection.size > 0 ? <span className="selection-label">{t('app.selected', { count: selection.size })}<button className="icon-button small" title={t('queue.add')} onClick={() => appendTracks(pickedTracks)}><ListPlus size={17} /></button><button className="icon-button small" title={t('queue.playNext')} onClick={() => void command({ type: 'enqueue_next_tracks', trackIds: pickedTracks.map(track => track.id) })}><ListEnd size={17} /></button><button className="icon-button small" title={t('app.cancel')} onClick={() => setSelection(new Set())}><X size={15} /></button></span> : <span>{t('library.tracks', { count: displayedTracks.length })}<span className="stat-separator">·</span>{time(displayedTracks.reduce((sum, track) => sum + track.duration, 0))}</span>}</div><div className="sort-control"><SortControl sort={sort} descending={descending} allowManual={!!activePlaylist} onChange={(mode, reverse) => { setSort(mode); setDescending(reverse); }} /></div></div>}
            {displayedTracks.length > 0 ? <TrackList tracks={displayedTracks} currentTrackId={playback?.trackId} playing={playing} playlists={snapshot.playlists} favoriteIds={snapshot.favoriteTrackIds || []} onFavorite={tracks => void command({ type: 'set_favorite', trackIds: tracks.map(track => track.id), favorite: !tracks.every(track => snapshot.favoriteTrackIds?.includes(track.id)) })} onManagePlaylists={setMembershipTracks} onEdit={setEditorTrack} onAddFolder={addContainingFolder} canAddFolder={canAddContainingFolder} selected={selection} onSelect={(index, shift) => { const next = new Set(selection); if (shift) for (let n = Math.min(lastSelected.current, index); n <= Math.max(lastSelected.current, index); n++) next.add(n); else next.has(index) ? next.delete(index) : next.add(index); lastSelected.current = index; setSelection(next); }} onSelectAll={() => setSelection(selection.size === displayedTracks.length ? new Set() : new Set(displayedTracks.map((_, index) => index)))} onPlay={playFromIndex} onAppend={appendTracks} onNext={tracks => void command({ type: 'enqueue_next_tracks', trackIds: tracks.map(track => track.id) })} onInfo={setDetails} onAddPlaylist={(id, tracks) => void command({ type: 'add_to_playlist', playlistId: id, trackIds: tracks.map(track => track.id) })} onRemove={activePlaylist ? index => { const track = displayedTracks[index]; const occurrence = displayedTracks.slice(0, index).filter(item => item.id === track.id).length; const original = activePlaylist.trackIds.map((id, position) => id === track.id ? position : -1).filter(position => position >= 0)[occurrence]; void command({ type: 'remove_from_playlist', playlistId: activePlaylist.id, index: original }); } : undefined} /> : search ? <EmptyState icon={<Search />} title={t('app.noResults')} description={t('app.noResultsDescription')}><button className="button secondary" onClick={() => setSearch('')}>{t('app.resetSearch')}</button></EmptyState> : activePlaylist ? <EmptyState icon={<ListMusic />} title={t('playlist.emptyList')} description={t('playlist.emptyListDescription')}><button className="button secondary" onClick={() => navigate('library')}><Library size={16} />{t('nav.library')}</button></EmptyState> : page === 'favorites' ? <EmptyState icon={<Heart />} title={t('workspace.emptyFavorites')} description={t('workspace.emptyFavoritesHint')}><button className="button secondary" onClick={() => navigate('library')}><Library size={16} />{t('nav.library')}</button></EmptyState> : <EmptyState icon={<FolderOpen />} title={t('library.emptyTitle')} description={t('library.emptyDescription')} large><div className="empty-actions"><button className="button primary" onClick={() => void run(() => api.openFolder('library'))}><Plus size={16} />{t('library.addFolder')}</button><button className="button secondary" onClick={() => void run(() => api.openFiles('play'))}>{t('library.openFiles')}<ChevronRight size={15} /></button></div><span className="drop-hint">{t('library.dropHint')}</span><span className="supported-formats">{t('library.formats')}</span></EmptyState>}
          </>}

          {(page === 'albums' || page === 'folders') && !group && <>{groups.length ? <div className={`collection-grid ${page === 'folders' ? 'folder-grid' : ''}`}>{groups.map(([key, entry]) => <button className="collection-card" key={key} onClick={() => { setGroup({ name: entry.name, tracks: entry.tracks, path: page === 'folders' ? key : undefined }); setSearch(''); setSort('album'); }}><div className="collection-art">{page === 'folders' ? <span className="folder-art"><Folder size={64} strokeWidth={1} /></span> : <Cover track={entry.tracks.find(track => track.coverPath) || entry.tracks[0]} size="large" album />}<span className="collection-play" onClick={e => { e.stopPropagation(); playTracks(entry.tracks); }} role="button" tabIndex={0} onKeyDown={e => { if (e.key === 'Enter') { e.stopPropagation(); playTracks(entry.tracks); } }} aria-label={t(page === 'folders' ? 'library.playFolder' : 'library.playAlbum')}><Play size={19} fill="currentColor" /></span>{page === 'folders' && <span className="collection-remove" role="button" tabIndex={0} title={`${t('library.removeFolder')} · ${t('library.removeFolderHint')}`} aria-label={`${t('library.removeFolder')}: ${entry.name}`} onClick={e => { e.stopPropagation(); removeFolder(key); }} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); removeFolder(key); } }}><FolderMinus size={16} /></span>}</div><strong className="truncate">{entry.name}</strong><span className="text-secondary truncate" title={entry.subtitle}>{entry.subtitle}</span><small>{t('library.tracks', { count: entry.tracks.length })}</small></button>)}</div> : <EmptyState icon={page === 'folders' ? <FolderOpen /> : <Disc3 />} title={search ? t('app.noResults') : t('library.emptyTitle')} description={search ? t('app.noResultsDescription') : t('library.emptyDescription')}><button className="button primary" onClick={() => void run(() => api.openFolder('library'))}><Plus size={16} />{t('library.addFolder')}</button></EmptyState>}</>}

          {page === 'playlists' && !activePlaylist && <><div className="section-toolbar"><button className="button primary" onClick={() => setNameDialog({ kind: 'playlist', value: '' })}><Plus size={16} />{t('playlist.new')}</button><button className="button secondary" onClick={() => void run(() => api.importPlaylist())}><ArrowDownToLine size={16} />{t('library.importPlaylist')}</button></div>{snapshot.playlists.length ? <div className="playlist-grid">{snapshot.playlists.filter(list => list.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())).map((list, index) => <button className="playlist-card" key={list.id} onClick={() => navigate('playlists', list.id)}><span className={`playlist-art color-${index % 4}`}><PlaylistIcon icon={list.icon} size={36} /></span><div><h3>{list.name}</h3><p>{t('library.tracks', { count: list.trackIds.length })}</p></div><ChevronRight size={17} /></button>)}</div> : <EmptyState icon={<ListMusic />} title={t('playlist.emptyTitle')} description={t('playlist.emptyDescription')} />}</>}

          {page === 'sessions' && <><div className="section-toolbar"><button className="button primary" onClick={() => setNameDialog({ kind: 'session', value: '' })}><Plus size={16} />{t('session.new')}</button><SessionCleanupMenu snapshot={snapshot} onCommand={command} /></div><div className="session-grid">{snapshot.sessions.map(item => <article className={`session-card ${item.id === snapshot.activeSessionId ? 'active' : ''}`} key={item.id}><div className="session-card-top"><span className="session-icon"><Headphones size={23} /></span>{item.id === snapshot.activeSessionId && <span className="active-badge"><span className="status-dot" />{t('session.active')}</span>}</div><h3>{item.name || t('session.defaultName')}</h3><p>{t('library.tracks', { count: item.entries.length })}<span className="stat-separator">·</span>{t('session.position', { position: time(item.position) })}</p><div className="session-card-actions"><button className="button secondary" disabled={item.id === snapshot.activeSessionId} onClick={() => void command({ type: 'activate_session', sessionId: item.id })}>{item.id === snapshot.activeSessionId ? <><Check size={15} />{t('session.active')}</> : <><Play size={14} />{t('session.activate')}</>}</button><button className="icon-button" title={t('session.rename')} onClick={() => setNameDialog({ kind: 'rename_session', id: item.id, value: item.name })}><Settings2 size={16} /></button><button className="icon-button danger" title={onlyEmptySession(snapshot) ? t('session.lastSession') : t('session.delete')} disabled={onlyEmptySession(snapshot)} onClick={() => void command({ type: 'delete_session', sessionId: item.id })}><Trash2 size={15} /></button></div></article>)}</div></>}

          {page === 'statistics' && <StatisticsPage snapshot={snapshot} onOpenSettings={openStatisticsSettings} onPlay={trackId => { void command({ type: 'play_tracks', trackIds: [trackId] }); }} />}
          {page === 'settings' && <SettingsPanel snapshot={snapshot} onCommand={command} onRun={run} onSnapshot={accept} defaultAppStatus={defaultAppStatus}
            destination={settingsDestination} onDestinationHandled={handleSettingsDestination} statisticsPanel={<StatisticsPreferences snapshot={snapshot} onCommand={featureCommand} />}
            appearancePanel={<AppearanceEditor value={snapshot.settings.appearance || DEFAULT_APPEARANCE} onPreview={setAppearancePreview}
              onChange={async appearance => {
                if (isDesktop) accept(await api.dispatch({ type: 'set_appearance', appearance }));
                else setSnapshot(previous => previous && ({ ...previous, settings: { ...previous.settings, appearance } }));
              }}
              onImport={async () => { const document = await api.openConfigurationFile(); return document === null ? null : parseThemeFile(JSON.stringify(document)); }}
              onExport={async theme => { await api.saveConfigurationFile(JSON.parse(serializeThemeFile(theme)), 'Song-Player-theme.json'); }} />}
            dataPanel={<DataSettingsPanel snapshot={snapshot} onSnapshot={accept} />} audioPanel={<><LoudnessPanel snapshot={snapshot} onCommand={featureCommand} /><EqualizerPanel snapshot={snapshot} onCommand={featureCommand} /></>} libraryPanel={<LibraryOptionsPanel snapshot={snapshot} onCommand={featureCommand} />} searchPanel={<GlobalSearchOptions snapshot={snapshot} onCommand={featureCommand} onResetLearning={resetSearchUsage} />} />}
        </div>
      </div>
    </main>

    {showQueue && <QueuePanel snapshot={snapshot} onTimerCommand={featureCommand} onCommand={command} onRun={run} onNewPlaylist={() => setNameDialog({ kind: 'queue_playlist', value: '' })} onNewSession={() => setNameDialog({ kind: 'session', value: '' })} onClose={() => setQueueVisible(false)} />}
    {showQueue && <PanelResizeHandle value={queueWidth} min={240} max={queueMax} defaultValue={310} label={t('settings.resizeQueue')} controls="active-queue" direction="left" style={{ left: 'calc(100% - var(--queue-width))' }} onChange={width => resizePanel('queue', width)} onCommit={width => resizePanel('queue', width, true)} />}

    <PlayerBar snapshot={snapshot} onCommand={command} onInfo={setDetails} onExpand={() => setNowPlaying(true)} />
    </div>}</div>
    {folderOffer}
    {notice && <div className={`docked-bar notice ${notice.error ? 'error' : 'success'}`} role={notice.error ? 'alert' : 'status'}>{notice.error ? <AlertCircle size={18} /> : <CheckCircle2 size={18} />}<span>{notice.text}</span><button className="icon-button small" title={t('app.close')} onClick={() => setNotice(null)}><X size={15} /></button></div>}
    </div>

    {quickSearch && <QuickSearch items={searchItems} tracks={trackSearchItems} onClose={() => setQuickSearch(false)} onSelect={item => { if (item.kind === 'track') void command({ type: 'play_tracks', trackIds: [item.id.slice(6)] }); else actions.find(action => action.id === item.id)?.run(); }} />}
    {membershipTracks && <PlaylistMembershipDialog tracks={membershipTracks} playlists={snapshot.playlists} onClose={() => setMembershipTracks(null)} onSave={async (playlistIds, preservePlaylistIds) => { accept(await api.dispatch({ type: 'set_playlist_membership', trackIds: membershipTracks.map(track => track.id), playlistIds, preservePlaylistIds })); }} onCreate={async name => { const existing = new Set(snapshot.playlists.map(list => list.id)); const state = await api.dispatch({ type: 'create_playlist', name }); accept(state); const created = state.playlists.find(list => !existing.has(list.id) && list.name === name); if (!created) throw new Error('Playlist was not returned after creation'); return created.id; }} />}
    {iconPlaylist && <PlaylistIconDialog playlist={iconPlaylist} onClose={() => setIconPlaylist(null)} onSave={async icon => { accept(await api.dispatch({ type: 'set_playlist_icon', playlistId: iconPlaylist.id, icon })); }} />}
    {sleepOpen && <SleepTimerDialog snapshot={snapshot} onCommand={featureCommand} onClose={() => setSleepOpen(false)} />}
    {editorTrack && <AudioEditor track={editorTrack} onClose={() => setEditorTrack(null)} />}
    {equalizerOpen && <Dialog title={t('workspace.equalizer')} onClose={() => setEqualizerOpen(false)}><div className="expansion-dialog-heading"><h2>{t('workspace.equalizer')}</h2><button className="icon-button" onClick={() => setEqualizerOpen(false)} aria-label={t('app.close')}><X size={20} /></button></div><EqualizerPanel snapshot={snapshot} onCommand={featureCommand} /></Dialog>}
    {dropActive && <div className="drop-overlay"><div><span className="drop-icon"><ArrowDownToLine size={38} /></span><h2>{t('drop.title')}</h2><p>{t('drop.description')}</p></div></div>}
    {nameDialog && <div className="modal-backdrop" onMouseDown={e => { if (e.target === e.currentTarget) setNameDialog(null); }}><form className="modal name-modal" role="dialog" aria-modal="true" aria-labelledby="name-dialog-title" onSubmit={submitName}><button type="button" className="icon-button modal-close" title={t('app.close')} onClick={() => setNameDialog(null)}><X size={19} /></button><span className="modal-icon">{nameDialog.kind.includes('session') ? <Headphones size={25} /> : <ListMusic size={25} />}</span><h2 id="name-dialog-title">{t(nameDialog.kind === 'queue_playlist' ? 'queue.save' : nameDialog.kind === 'playlist' ? 'playlist.new' : nameDialog.kind === 'session' ? 'session.new' : nameDialog.kind === 'rename_playlist' ? 'playlist.rename' : 'session.rename')}</h2>{!nameDialog.kind.startsWith('rename') && <p>{t(nameDialog.kind === 'playlist' ? 'workspace.emptyPlaylistDescription' : nameDialog.kind === 'queue_playlist' ? 'workspace.queuePlaylistDescription' : 'session.createDescription')}</p>}<label htmlFor="name-input">{t('app.name')}</label><input id="name-input" ref={nameInput} value={nameDialog.value} maxLength={150} onChange={e => setNameDialog({ ...nameDialog, value: e.target.value })} placeholder={t(nameDialog.kind.includes('session') ? 'session.namePlaceholder' : 'playlist.namePlaceholder')} /><div className="modal-actions"><button type="button" className="button secondary" onClick={() => setNameDialog(null)}>{t('app.cancel')}</button><button type="submit" className="button primary" disabled={!nameDialog.value.trim() || busy > 0}>{busyVisible ? <LoaderCircle size={15} className="spin" /> : <Check size={15} />}{t(nameDialog.kind.startsWith('rename') ? 'app.save' : 'app.create')}</button></div></form></div>}
    {details && <div className="modal-backdrop" onMouseDown={e => { if (e.target === e.currentTarget) setDetails(null); }}><section className="modal details-modal" role="dialog" aria-modal="true" aria-labelledby="details-title"><button className="icon-button modal-close" title={t('app.close')} onClick={() => setDetails(null)}><X size={19} /></button><div className="details-track"><Cover track={details} size="medium" /><div><div className="eyebrow" id="details-title">{t('details.title')}</div><h2>{details.title}</h2><p>{details.artist || t('library.unknownArtist')}</p></div></div><dl className="details-grid">{([['album', details.album], ['genre', details.genre], ['year', details.year], ['track', details.trackNumber], ['disc', details.discNumber], ['duration', time(details.duration)], ['format', details.format], ['sampleRate', details.sampleRate ? `${details.sampleRate.toLocaleString(i18n.language)} Hz` : null], ['bitrate', details.bitrate ? `${Math.round(details.bitrate / 1000)} kbps` : null], ['channels', details.channels]] as const).map(([key, value]) => <div key={key}><dt>{t(`details.${key}`)}</dt><dd>{value || '—'}</dd></div>)}</dl><div className="file-path"><span>{t('details.path')}</span><p>{details.path}</p></div><div className="modal-actions"><button className="button primary" onClick={() => setDetails(null)}>{t('app.done')}</button></div></section></div>}
  </>;
}

function EmptyState({ icon, title, description, children, large = false }: { icon: React.ReactNode; title: string; description: string; children?: React.ReactNode; large?: boolean }) {
  return <section className={`empty-state ${large ? 'empty-large' : ''}`}><div className="empty-illustration"><span className="empty-orbit orbit-one" /><span className="empty-orbit orbit-two" /><span className="empty-core">{icon}</span><span className="orbit-spark spark-one" /><span className="orbit-spark spark-two" /><span className="orbit-spark spark-three" /></div><h2>{title}</h2><p>{description}</p>{children}</section>;
}

