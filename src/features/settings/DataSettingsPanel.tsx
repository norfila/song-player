import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, FileJson, LoaderCircle, RotateCcw, Upload, X } from 'lucide-react';
import { api, isDesktop } from '../../lib/backend';
import type { AppSnapshot } from '../../lib/backend/types';
import { availableSections, defaultImportSections, parseSettingsDocument, PORTABLE_SECTIONS, TRANSFER_SECTIONS } from '../../lib/settings-transfer';
import type { SettingsDocument, TransferSection } from '../../lib/settings-transfer';
import { formatShortcut } from '../../lib/shortcuts';
import { DEFAULT_APPEARANCE, resolveTheme } from '../../lib/theme';
import type { AppearanceSettings } from '../../lib/theme';
import './configuration-transfer.css';

interface Props { snapshot: AppSnapshot; onSnapshot: (value: AppSnapshot) => void }

export function DataSettingsPanel({ snapshot, onSnapshot }: Props) {
  const { t } = useTranslation();
  const [exportSections, setExportSections] = useState<TransferSection[]>([...PORTABLE_SECTIONS]);
  const [pending, setPending] = useState<SettingsDocument | null>(null);
  const [importSections, setImportSections] = useState<TransferSection[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ key: string; error?: boolean } | null>(null);
  const toggle = (current: TransferSection[], section: TransferSection) => current.includes(section) ? current.filter(item => item !== section) : [...current, section];

  async function perform(operation: () => Promise<void>, errorKey = 'configuration.failed') {
    setBusy(true); setMessage(null);
    try { await operation(); }
    catch { setMessage({ key: errorKey, error: true }); }
    finally { setBusy(false); }
  }

  function sectionOptions(selected: TransferSection[], update: (value: TransferSection[]) => void, choices: readonly TransferSection[]) {
    return <div className="configuration-sections">{choices.map(section => <label className="configuration-section-choice" key={section}>
      <input type="checkbox" checked={selected.includes(section)} disabled={busy} onChange={() => update(toggle(selected, section))} />
      <span><strong>{t(`configuration.sections.${section}`)}</strong>{(section === 'device' || section === 'library') && <small>{t(`configuration.${section}Hint`)}</small>}</span>
    </label>)}</div>;
  }

  function importDetails(section: TransferSection) {
    if (!pending) return null;
    const item = pending.sections;
    const yesNo = (value: boolean) => t(value ? 'configuration.on' : 'configuration.off');
    const current = snapshot.settings;
    const themeName = (appearance: AppearanceSettings) => {
      const theme = resolveTheme(appearance);
      return ['light', 'dark', 'gold'].includes(theme.id) ? t(`appearance.presets.${theme.id}`) : theme.name;
    };
    const customThemeNames = (appearance: AppearanceSettings) => appearance.customThemes.map(theme => theme.name).join(', ') || '—';
    const deviceName = (id: string) => id === 'auto' ? t('settings.defaultOutput') : id;
    const rows: [string, string, string][] = [];
    if (section === 'general' && item.general) rows.push(
      [t('settings.language'), current.language === 'de' ? 'Deutsch' : 'English', item.general.language === 'de' ? 'Deutsch' : 'English'],
      [t('settings.closeToTray'), yesNo(current.closeToTray), yesNo(item.general.closeToTray)],
    );
    if (section === 'general' && item.general) {
      if (item.general.openExternalNowPlaying !== undefined) rows.push([t('features.library.openExternalNowPlaying'), yesNo(current.openExternalNowPlaying), yesNo(item.general.openExternalNowPlaying)]);
      if (item.general.suggestExternalFolders !== undefined) rows.push([t('features.library.suggestExternalFolders'), yesNo(current.suggestExternalFolders), yesNo(item.general.suggestExternalFolders)]);
      if (item.general.statistics) rows.push(
        [t('features.statistics.record'), yesNo(current.statistics.enabled), yesNo(item.general.statistics.enabled)],
        [t('features.statistics.percent'), `${current.statistics.percent}%`, `${item.general.statistics.percent}%`],
        [t('features.statistics.seconds'), `${current.statistics.seconds} s`, `${item.general.statistics.seconds} s`],
      );
    }
    if (section === 'appearance' && item.appearance) rows.push(
      [t('configuration.sections.appearance'), themeName(current.appearance || DEFAULT_APPEARANCE), themeName(item.appearance)],
      [t('configuration.customThemes'), customThemeNames(current.appearance || DEFAULT_APPEARANCE), customThemeNames(item.appearance)],
    );
    if (section === 'audio' && item.audio) rows.push(
      [t('player.volume'), `${Math.round(current.volume)}%`, `${Math.round(item.audio.volume)}%`],
      [t('player.mute'), yesNo(current.muted), yesNo(item.audio.muted)],
      [t('features.loudness.dynamics'), yesNo(current.loudnessNormalization), yesNo(item.audio.loudnessNormalization)],
      [t('features.loudness.title'), yesNo(current.replayGain), yesNo(item.audio.replayGain)],
    );
    if (section === 'audio' && item.audio) {
      if (item.audio.gainGrouping) rows.push([t('features.loudness.grouping'), t(`features.loudness.groups.${current.gainGrouping}`), t(`features.loudness.groups.${item.audio.gainGrouping}`)]);
      if (item.audio.equalizer) rows.push(
        [t('features.equalizer.enable'), yesNo(current.equalizer.enabled), yesNo(item.audio.equalizer.enabled)],
        ...['60 Hz', '250 Hz', '1 kHz', '4 kHz', '12 kHz'].map((frequency, index): [string, string, string] => [frequency, `${current.equalizer.bands[index]} dB`, `${item.audio!.equalizer!.bands[index]} dB`]),
        [t('features.equalizer.preamp'), `${current.equalizer.preamp} dB`, `${item.audio.equalizer.preamp} dB`],
      );
    }
    if (section === 'shortcuts' && item.shortcuts) for (const [action, binding] of Object.entries(item.shortcuts)) rows.push([t(`shortcuts.actions.${action}`), formatShortcut(current.shortcuts[action] || '') || '—', formatShortcut(binding) || '—']);
    if (section === 'layout' && item.layout) rows.push(
      [t('settings.resizeSidebar'), `${current.panelWidths.sidebar} px`, `${item.layout.panelWidths.sidebar} px`],
      [t('settings.resizeQueue'), `${current.panelWidths.queue} px`, `${item.layout.panelWidths.queue} px`],
    );
    if (section === 'layout' && item.layout?.seekbarStyle) {
      const style = (value: string | undefined) => t(value === 'waveform' ? 'settings.seekbarWaveform' : 'settings.seekbarLine');
      rows.push([t('settings.seekbar'), style(current.seekbarStyle), style(item.layout.seekbarStyle)]);
    }
    if (section === 'device' && item.device) rows.push([t('settings.output'), deviceName(current.outputDevice), deviceName(item.device.outputDevice)]);
    if (section === 'device' && item.device?.globalSearch) rows.push(
      [t('features.globalSearch.enable'), yesNo(current.globalSearch.enabled), yesNo(item.device.globalSearch.enabled)],
      [t('features.globalSearch.binding'), formatShortcut(current.globalSearch.binding), formatShortcut(item.device.globalSearch.binding)],
    );
    if (section === 'library' && item.library) rows.push([t('settings.folders'), snapshot.libraryFolders.join(', ') || '—', [...new Set([...snapshot.libraryFolders, ...item.library.folders])].join(', ') || '—']);
    return <details className="configuration-details" key={section}>
      <summary>{t(`configuration.sections.${section}`)}</summary>
      <table><thead><tr><th>{t('configuration.setting')}</th><th>{t('configuration.current')}</th><th>{t('configuration.imported')}</th></tr></thead>
        <tbody>{rows.map(([label, before, after], index) => <tr key={index}><th scope="row">{label}</th><td>{before}</td><td>{after}</td></tr>)}</tbody>
      </table>
    </details>;
  }

  return <div className="configuration-panel">
    <p className="configuration-intro">{t('configuration.description')}</p>
    {message && <p className={`configuration-message ${message.error ? 'is-error' : ''}`} role={message.error ? 'alert' : 'status'}>{t(message.key)}</p>}
    <section className="configuration-card" data-setting-id="settings-export" tabIndex={-1} aria-labelledby="configuration-export-heading">
      <h3 id="configuration-export-heading"><Download size={18} />{t('configuration.exportTitle')}</h3>
      <p>{t('configuration.exportDescription')}</p>
      {sectionOptions(exportSections, setExportSections, TRANSFER_SECTIONS)}
      <button className="button primary" disabled={!isDesktop || busy || !exportSections.length} onClick={() => void perform(async () => {
        const document = await api.exportSettingsDocument(exportSections);
        if (await api.saveConfigurationFile(document, 'Song-Player-settings.json')) setMessage({ key: 'configuration.exported' });
      })}><Download size={16} />{t('configuration.export')}</button>
    </section>
    <section className="configuration-card" data-setting-id="settings-import" tabIndex={-1} aria-labelledby="configuration-import-heading">
      <h3 id="configuration-import-heading"><Upload size={18} />{t('configuration.importTitle')}</h3>
      <p>{t('configuration.importDescription')}</p>
      <button className="button secondary" disabled={!isDesktop || busy} onClick={() => void perform(async () => {
        const value = await api.openConfigurationFile();
        if (value === null) return;
        setPending(null); setImportSections([]);
        const document = parseSettingsDocument(value);
        setPending(document); setImportSections(defaultImportSections(document));
      }, 'configuration.invalidFile')}><FileJson size={16} />{t('configuration.chooseFile')}</button>
      {pending && <div className="configuration-review" role="region" aria-label={t('configuration.reviewTitle')}>
        <div className="configuration-review-heading"><h4>{t('configuration.reviewTitle')}</h4><button className="icon-button" aria-label={t('app.cancel')} disabled={busy} onClick={() => setPending(null)}><X size={16} /></button></div>
        <p>{t('configuration.reviewDescription')}</p>
        {sectionOptions(importSections, setImportSections, availableSections(pending))}
        {importSections.map(importDetails)}
        <div className="configuration-actions"><button className="button quiet" disabled={busy} onClick={() => setPending(null)}>{t('app.cancel')}</button>
          <button className="button primary" disabled={busy || !importSections.length} onClick={() => void perform(async () => {
            const state = await api.dispatch({ type: 'import_settings', document: pending, sections: importSections });
            onSnapshot(state); setPending(null); setMessage({ key: 'configuration.importedSuccess' });
          })}>{busy ? <LoaderCircle size={16} className="spin" /> : <Upload size={16} />}{t('configuration.apply')}</button>
        </div>
      </div>}
    </section>
    <section className="configuration-card" data-setting-id="settings-undo" tabIndex={-1} aria-labelledby="configuration-undo-heading">
      <h3 id="configuration-undo-heading"><RotateCcw size={18} />{t('configuration.undoTitle')}</h3>
      <p>{t('configuration.undoDescription')}</p>
      <button className="button secondary" disabled={!isDesktop || busy || !snapshot.settingsUndoAvailable} onClick={() => void perform(async () => {
        onSnapshot(await api.dispatch({ type: 'undo_settings_import' })); setMessage({ key: 'configuration.restored' });
      })}><RotateCcw size={16} />{t('configuration.undo')}</button>
    </section>
  </div>;
}
