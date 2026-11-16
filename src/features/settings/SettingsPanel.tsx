import { useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, AudioLines, BarChart3, CheckCircle2, ChevronRight, Database, Folder, FolderOpen, Info, Keyboard, Palette, Plus, RefreshCw, Search, Settings2, X } from 'lucide-react';
import { Select } from '../../components/Select';
import { ShortcutEditor } from './ShortcutEditor';
import { api, isDesktop } from '../../lib/backend';
import type { DefaultAppStatus } from '../../lib/backend';
import type { AppSnapshot, AudioDevice, PlayerCommand } from '../../lib/backend/types';
import type { AppearanceSettings } from '../../lib/theme';
import { basename } from '../../lib/format';
import { findShortcutAction, formatShortcut, resolveShortcuts, SHORTCUT_ACTIONS } from '../../lib/shortcuts';
import { version } from '../../../package.json';
import './settings-navigation.css';

interface Props {
  snapshot: AppSnapshot;
  onCommand: (value: PlayerCommand) => Promise<void>;
  onRun: (operation: () => Promise<AppSnapshot | null | void>) => Promise<void>;
  onSnapshot: (snapshot: AppSnapshot) => void;
  defaultAppStatus: DefaultAppStatus | null;
  appearancePanel?: ReactNode;
  dataPanel?: ReactNode;
  audioPanel?: ReactNode;
  libraryPanel?: ReactNode;
  searchPanel?: ReactNode;
  statisticsPanel?: ReactNode;
  destination?: SettingsDestination | null;
  onDestinationHandled?: () => void;
  onAppearancePreview?: (appearance: AppearanceSettings | null) => void;
}

const CATEGORIES = [
  { id: 'general', icon: Settings2 },
  { id: 'appearance', icon: Palette },
  { id: 'audio', icon: AudioLines },
  { id: 'library', icon: FolderOpen },
  { id: 'shortcuts', icon: Keyboard },
  { id: 'statistics', icon: BarChart3 },
  { id: 'data', icon: Database },
  { id: 'about', icon: Info },
] as const;
type Category = typeof CATEGORIES[number]['id'];
export interface SettingsDestination { category: Category; id: string }
interface SearchEntry {
  id: string;
  category: Category;
  title: string;
  description: string;
  keywords: string;
  focusSelector?: string;
}
interface SearchTarget { id: string; focusSelector?: string }
const searchable = (value: string) => value.toLocaleLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '');

export function SettingsPanel({ snapshot, onCommand: command, onRun: run, onSnapshot: setSnapshot, defaultAppStatus, appearancePanel, dataPanel, audioPanel, libraryPanel, searchPanel, statisticsPanel, destination, onDestinationHandled }: Props) {
  const { t, i18n } = useTranslation();
  const [category, setCategory] = useState<Category>(destination?.category ?? 'general');
  const [query, setQuery] = useState('');
  const [pendingTarget, setPendingTarget] = useState<SearchTarget | null>(null);
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const searchRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const shortcuts = useMemo(() => resolveShortcuts(snapshot.settings.shortcuts), [snapshot.settings.shortcuts]);
  const refreshDevices = () => { void run(async () => { setDevices(await api.getAudioDevices()); }); };
  useEffect(() => { if (isDesktop) refreshDevices(); }, []);
  const audioUnavailable = !isDesktop || !snapshot.engine.available;

  useEffect(() => {
    if (!destination) return;
    setCategory(destination.category);
    setQuery('');
    setHighlighted(null);
    setPendingTarget({ id: destination.id });
    onDestinationHandled?.();
  }, [destination, onDestinationHandled]);

  // Settings owns search while mounted; shortcut recording still gets its keys.
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      const element = event.target as HTMLElement | null;
      if (element?.closest('[data-shortcut-recorder], [role="dialog"]')) return;
      const editing = element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element?.isContentEditable;
      const enteringText = editing && event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey;
      if (!enteringText && findShortcutAction(event, shortcuts) === 'search') {
        event.preventDefault();
        event.stopPropagation();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (event.code === 'Escape' && query) {
        event.preventDefault();
        event.stopPropagation();
        setQuery('');
        searchRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handle, true);
    return () => window.removeEventListener('keydown', handle, true);
  }, [query, shortcuts]);

  useEffect(() => {
    if (!pendingTarget || query) return;
    const frame = requestAnimationFrame(() => {
      const fallbackId = ['replaygain', 'normalization'].includes(pendingTarget.id) ? 'loudness' : pendingTarget.id.startsWith('appearance-') ? 'appearance' : pendingTarget.id.startsWith('settings-') ? 'data' : pendingTarget.id;
      const target = contentRef.current?.querySelector<HTMLElement>(`[data-setting-id="${pendingTarget.id}"]`)
        ?? contentRef.current?.querySelector<HTMLElement>(`[data-setting-id="${fallbackId}"]`);
      if (target) {
        let ancestor = target.parentElement; while (ancestor && ancestor !== contentRef.current) { if (ancestor instanceof HTMLDetailsElement) ancestor.open = true; ancestor = ancestor.parentElement; }
        target.scrollIntoView({ block: 'nearest', behavior: 'auto' });
        const focusTarget = pendingTarget.focusSelector
          ? target.querySelector<HTMLElement>(pendingTarget.focusSelector)
          : target.querySelector<HTMLElement>('input:not(:disabled),button:not(:disabled),select:not(:disabled),summary');
        (focusTarget ?? target).focus({ preventScroll: true });
        setHighlighted(target.dataset.settingId ?? pendingTarget.id);
      }
      setPendingTarget(null);
    });
    return () => cancelAnimationFrame(frame);
  }, [category, pendingTarget, query]);
  useEffect(() => {
    if (!highlighted) return;
    const target = contentRef.current?.querySelector<HTMLElement>(`[data-setting-id="${highlighted}"]`);
    target?.classList.add('setting-search-highlight');
    const timeout = window.setTimeout(() => setHighlighted(null), 2800);
    return () => { window.clearTimeout(timeout); target?.classList.remove('setting-search-highlight'); };
  }, [highlighted]);

  const entries = useMemo<SearchEntry[]>(() => {
    const entry = (id: string, entryCategory: Category, title: string, description: string, keyword: string, focusSelector?: string): SearchEntry => ({
      id, category: entryCategory, title: t(title), description: t(description), keywords: t(`settingsNav.keywords.${keyword}`), focusSelector,
    });
    return [
      entry('language', 'general', 'settings.language', 'settings.languageDescription', 'language'),
      entry('close-to-tray', 'general', 'settings.closeToTray', 'settings.closeToTrayDescription', 'tray'),
      ...(defaultAppStatus?.supported ? [entry('default-apps', 'general', 'defaultApps.title', 'defaultApps.settingsDescription', 'defaultApps')] : []),
      entry('appearance-presets', 'appearance', 'settingsNav.find.themes', 'settingsNav.find.themesDescription', 'themes'),
      entry('appearance-customize', 'appearance', 'settingsNav.find.customize', 'settingsNav.find.customizeDescription', 'customize'),
      entry('appearance-typography', 'appearance', 'settingsNav.find.typography', 'settingsNav.find.typographyDescription', 'typography'),
      entry('appearance-files', 'appearance', 'settingsNav.find.themeFiles', 'settingsNav.find.themeFilesDescription', 'themeFiles'),
      entry('seekbar', 'appearance', 'settings.seekbar', 'settings.seekbarDescription', 'seekbar'),
      entry('output-device', 'audio', 'settings.output', 'settings.outputDescription', 'output'),
      entry('normalization', 'audio', 'features.loudness.dynamics', 'features.loudness.dynamicsHint', 'normalization'),
      entry('replaygain', 'audio', 'features.loudness.title', 'features.loudness.description', 'replaygain'),
      entry('equalizer', 'audio', 'workspace.equalizer', 'features.equalizer.description', 'output'),
      entry('library-options', 'library', 'features.library.title', 'features.library.description', 'folders'),
      entry('global-search', 'shortcuts', 'features.globalSearch.title', 'features.globalSearch.description', 'shortcuts'),
      entry('folders', 'library', 'settings.folders', 'settingsNav.find.foldersDescription', 'folders'),
      entry('shortcuts', 'shortcuts', 'settings.shortcuts', 'shortcuts.description', 'shortcuts'),
      ...SHORTCUT_ACTIONS.map(action => entry('shortcuts', 'shortcuts', `shortcuts.actions.${action}`, 'settingsNav.find.shortcutDescription', 'shortcuts', `button[id$="-${action}-capture"]`)),
      entry('statistics-preferences', 'statistics', 'features.statistics.settings', 'features.statistics.thresholdHint', 'statistics'),
      entry('settings-export', 'data', 'settingsNav.find.export', 'settingsNav.find.exportDescription', 'export'),
      entry('settings-import', 'data', 'settingsNav.find.import', 'settingsNav.find.importDescription', 'import'),
      entry('settings-undo', 'data', 'settingsNav.find.undo', 'settingsNav.find.undoDescription', 'undo'),
      entry('engine', 'about', 'settings.engine', 'settings.engineDescription', 'engine'),
      entry('about', 'about', 'settingsNav.find.version', 'settings.aboutDescription', 'version'),
      ...(snapshot.warnings.length ? [entry('warnings', 'about', 'settings.warnings', 'settingsNav.find.warningsDescription', 'warnings')] : []),
    ];
  }, [t, i18n.language, defaultAppStatus?.supported, snapshot.warnings.length]);

  const terms = searchable(query.trim()).split(/\s+/).filter(Boolean);
  const results = terms.length ? entries.filter(entry => {
    const text = searchable(`${entry.title} ${entry.description} ${entry.keywords} ${t(`settingsNav.categories.${entry.category}`)}`);
    return terms.every(term => text.includes(term));
  }) : [];
  const openCategory = (next: Category) => {
    setCategory(next);
    setQuery('');
    setHighlighted(null);
    setPendingTarget(null);
    requestAnimationFrame(() => {
      contentRef.current?.scrollTo({ top: 0 });
      contentRef.current?.querySelector<HTMLElement>('.settings-category-heading')?.focus({ preventScroll: true });
    });
  };
  const openResult = (entry: SearchEntry) => {
    setCategory(entry.category);
    setQuery('');
    setHighlighted(null);
    setPendingTarget({ id: entry.id, focusSelector: entry.focusSelector });
  };
  const anchorProps = (id: string) => ({
    'data-setting-id': id,
    tabIndex: -1,
    className: `setting-row${highlighted === id ? ' setting-search-highlight' : ''}`,
  });
  const CategoryIcon = CATEGORIES.find(item => item.id === category)!.icon;

  return <div className="settings-content settings-workspace">
    <div className="settings-search">
      <Search size={17} aria-hidden="true" />
      <input ref={searchRef} data-settings-search type="search" value={query} onChange={event => setQuery(event.target.value)}
        placeholder={t('settingsNav.searchPlaceholder')} aria-label={t('settingsNav.searchPlaceholder')}
        aria-controls="settings-category-content" autoComplete="off" spellCheck={false} />
      {query ? <button className="icon-button" type="button" title={t('settingsNav.clearSearch')} aria-label={t('settingsNav.clearSearch')}
        onClick={() => { setQuery(''); searchRef.current?.focus(); }}><X size={15} /></button> : shortcuts.search ? <kbd>{formatShortcut(shortcuts.search)}</kbd> : null}
    </div>
    <div className="settings-workspace-body">
      <nav className="settings-category-nav" aria-label={t('settingsNav.navigation')}>
        {CATEGORIES.map(({ id, icon: Icon }) => <button type="button" key={id} className={`settings-category-button${category === id && !terms.length ? ' active' : ''}`}
          aria-current={category === id && !terms.length ? 'page' : undefined} aria-controls="settings-category-content"
          onClick={() => openCategory(id)}><Icon size={17} aria-hidden="true" /><span>{t(`settingsNav.categories.${id}`)}</span></button>)}
      </nav>
      <div className="settings-category-content" ref={contentRef} id="settings-category-content">
        {terms.length > 0 && <section className="settings-search-results" aria-labelledby="settings-search-heading">
          <h2 className="settings-category-heading" id="settings-search-heading">{t('settingsNav.searchResults')}</h2>
          <p className="settings-category-description" role="status">{t('settingsNav.resultCount', { count: results.length })}</p>
          {results.length > 0 ? <div className="settings-results-list">{results.map((entry, index) => <button className="settings-search-result" type="button" key={`${entry.id}-${index}`} onClick={() => openResult(entry)}>
            <span><small>{t('settings.title')}<ChevronRight size={10} aria-hidden="true" />{t(`settingsNav.categories.${entry.category}`)}</small><strong>{entry.title}</strong><span>{entry.description}</span></span>
            <ChevronRight size={16} aria-hidden="true" />
          </button>)}</div> : <div className="settings-search-empty"><Search size={27} /><p>{t('settingsNav.noResults')}</p><span>{t('settingsNav.noResultsHint')}</span></div>}
        </section>}
        <div className="settings-current-category" hidden={terms.length > 0}>
          <div className="settings-category-intro">
            <h2 className="settings-category-heading" tabIndex={-1}><CategoryIcon size={22} aria-hidden="true" />{t(`settingsNav.categories.${category}`)}</h2>
            <p className="settings-category-description">{t(`settingsNav.descriptions.${category}`)}</p>
          </div>
          {category === 'general' && <SettingsSection title={t('settings.general')}>
            <div {...anchorProps('language')}>
              <div><label htmlFor="language">{t('settings.language')}</label><p>{t('settings.languageDescription')}</p></div>
              <Select id="language" className="input-select" label={t('settings.language')}
                value={i18n.language.startsWith('en') ? 'en' : 'de'}
                options={[{ value: 'de', label: 'Deutsch' }, { value: 'en', label: 'English' }]}
                onChange={language => {
                  void i18n.changeLanguage(language);
                  if (isDesktop) void command({ type: 'set_language', language });
                  else setSnapshot({ ...snapshot, settings: { ...snapshot.settings, language } });
                }} />
            </div>
            <div {...anchorProps('close-to-tray')}>
              <div><label id="close-to-tray-label">{t('settings.closeToTray')}</label><p>{t('settings.closeToTrayDescription')}</p></div>
              <button role="switch" aria-checked={snapshot.settings.closeToTray} aria-labelledby="close-to-tray-label" className={`switch ${snapshot.settings.closeToTray ? 'enabled' : ''}`} disabled={!isDesktop} onClick={() => void command({ type: 'set_close_to_tray', enabled: !snapshot.settings.closeToTray })}><span /></button>
            </div>
            {defaultAppStatus?.supported && <div {...anchorProps('default-apps')}><div><label>{t('defaultApps.title')}</label><p>{t(defaultAppStatus.missingExtensions.length ? 'defaultApps.settingsDescription' : 'defaultApps.allAssigned')}</p></div><button className="button secondary" disabled={!isDesktop} onClick={() => void run(() => api.openDefaultAppsSettings())}>{t('defaultApps.openSettings')}</button></div>}
          </SettingsSection>}
          {category === 'appearance' && <><div data-setting-id="appearance" tabIndex={-1} className={`settings-external-panel${highlighted === 'appearance' ? ' setting-search-highlight' : ''}`}>{appearancePanel}</div>
            <SettingsSection title={t('settings.playerDisplay')}>
              <div {...anchorProps('seekbar')}>
                <div><label htmlFor="seekbar-style">{t('settings.seekbar')}</label><p>{t('settings.seekbarDescription')}</p></div>
                <Select id="seekbar-style" className="input-select" label={t('settings.seekbar')}
                  value={snapshot.settings.seekbarStyle ?? 'line'}
                  options={[{ value: 'line', label: t('settings.seekbarLine') }, { value: 'waveform', label: t('settings.seekbarWaveform') }]}
                  onChange={style => {
                    const value = style === 'waveform' ? 'waveform' : 'line';
                    if (isDesktop) void command({ type: 'set_seekbar_style', style: value });
                    else setSnapshot({ ...snapshot, settings: { ...snapshot.settings, seekbarStyle: value } });
                  }} />
              </div>
            </SettingsSection></>}
          {category === 'audio' && <><SettingsSection title={t('settings.audio')}>
            <div {...anchorProps('output-device')}>
              <div><label htmlFor="output-device">{t('settings.output')}</label><p>{t('settings.outputDescription')}</p></div>
              <div className="setting-control">
                <Select id="output-device" className="input-select" label={t('settings.output')}
                  value={snapshot.settings.outputDevice} disabled={audioUnavailable}
                  onChange={id => void command({ type: 'set_audio_device', id })}
                  options={[{ value: 'auto', label: t('settings.defaultOutput') }, ...devices.filter(device => device.id !== 'auto').map(device => ({ value: device.id, label: device.name }))]} />
                <button className="icon-button" disabled={!isDesktop} title={t('settings.refreshDevices')} onClick={refreshDevices}><RefreshCw size={15} /></button>
              </div>
            </div>
          </SettingsSection>{audioPanel}</>}
          {category === 'library' && <><SettingsSection title={t('settings.folders')}>
            <div data-setting-id="folders" tabIndex={-1} className={highlighted === 'folders' ? 'setting-search-highlight' : ''}>
              {snapshot.libraryFolders.length ? snapshot.libraryFolders.map(path => <div className="setting-row folder-setting" key={path}>
                <Folder size={19} /><div className="truncate"><label>{basename(path)}</label><p title={path} className="truncate">{path}</p></div>
                <button className="icon-button danger" title={`${t('library.removeFolder')} · ${t('library.removeFolderHint')}`} onClick={() => void command({ type: 'remove_library_folder', path })}><X size={16} /></button>
              </div>) : <p className="setting-empty">{t('settings.noFolders')}</p>}
              <button className="button secondary setting-add-folder" onClick={() => void run(() => api.openFolder('library'))}><Plus size={15} />{t('library.addFolder')}</button>
            </div>
          </SettingsSection>{libraryPanel}</>}
          {category === 'shortcuts' && <><SettingsSection title={t('settings.shortcuts')}>
            <div data-setting-id="shortcuts" tabIndex={-1} className={highlighted === 'shortcuts' ? 'setting-search-highlight' : ''}>
              <ShortcutEditor value={snapshot.settings.shortcuts} disabled={!isDesktop} onChange={async shortcuts => { await api.dispatch({ type: 'set_shortcuts', shortcuts }).then(setSnapshot); }} />
            </div>
          </SettingsSection>{searchPanel}</>}
          {category === 'statistics' && statisticsPanel}
          {category === 'data' && <div data-setting-id="data" tabIndex={-1} className={`settings-external-panel${highlighted === 'data' ? ' setting-search-highlight' : ''}`}>{dataPanel}</div>}
          {category === 'about' && <>
            <div data-setting-id="about" tabIndex={-1} className={`about-sonora${highlighted === 'about' ? ' setting-search-highlight' : ''}`}><AudioLines size={29} /><div><h3>{t('app.productName')}</h3><p>{t('settings.aboutDescription')}</p><small>{t('settings.version', { version })}</small></div></div>
            <SettingsSection title={t('settings.engine')}>
              <div {...anchorProps('engine')}>
                <div><label>{t('settings.engine')}</label><p>{t('settings.engineDescription')}</p></div>
                <span className={`engine-state ${snapshot.engine.available ? 'ready' : ''}`}>{snapshot.engine.available ? <CheckCircle2 size={15} /> : <AlertCircle size={15} />}{t(snapshot.engine.available ? 'settings.engineReady' : 'settings.engineUnavailable')}</span>
              </div>
              <details className="technical-details"><summary>{t('settings.technicalDetails')}</summary><pre>{snapshot.engine.version || 'libmpv'}{snapshot.engine.error ? `\n${snapshot.engine.error}` : ''}</pre></details>
            </SettingsSection>
            {snapshot.warnings.length > 0 && <SettingsSection title={t('settings.warnings')}><div data-setting-id="warnings" tabIndex={-1} className={highlighted === 'warnings' ? 'setting-search-highlight' : ''}>{snapshot.warnings.map((warning, index) => <p className="warning-text" key={index}>{warning}</p>)}</div></SettingsSection>}
          </>}
        </div>
      </div>
    </div>
  </div>;
}

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return <section className="settings-section" aria-label={title}><div className="settings-card">{children}</div></section>;
}
