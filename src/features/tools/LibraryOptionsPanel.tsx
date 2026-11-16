import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AppSnapshot, PlayerCommand } from '../../lib/backend/types';
import { isDesktop } from '../../lib/backend';
import './tools.css';

export function LibraryOptionsPanel({ snapshot, onCommand }: { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void> }) {
  const { t } = useTranslation();
  const prefix = useId();
  const settings = snapshot.settings;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  async function change(key: 'openExternalNowPlaying' | 'suggestExternalFolders') {
    setBusy(true); setError(false);
    try { await onCommand({ type: 'set_library_options', openExternalNowPlaying: key === 'openExternalNowPlaying' ? !settings[key] : settings.openExternalNowPlaying, suggestExternalFolders: key === 'suggestExternalFolders' ? !settings[key] : settings.suggestExternalFolders }); }
    catch { setError(true); }
    finally { setBusy(false); }
  }
  return <section className="tool-card" data-setting-id="library-options" tabIndex={-1}>
    <h3>{t('features.library.title')}</h3>
    {error && <p role="alert" className="tool-error">{t('features.common.failed')}</p>}
    {(['openExternalNowPlaying', 'suggestExternalFolders'] as const).map(key => <div className="tool-toggle-row" key={key}>
      <div><label id={`${prefix}-${key}`}>{t(`features.library.${key}`)}</label><p>{t(`features.library.${key}Hint`)}</p></div>
      <button role="switch" aria-checked={settings[key]} aria-labelledby={`${prefix}-${key}`} className={`switch ${settings[key] ? 'enabled' : ''}`} disabled={!isDesktop || busy} onClick={() => void change(key)}><span /></button>
    </div>)}
  </section>;
}
