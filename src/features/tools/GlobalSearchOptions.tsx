import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Keyboard, RotateCcw } from 'lucide-react';
import { api, isDesktop } from '../../lib/backend';
import type { AppSnapshot, PlayerCommand } from '../../lib/backend/types';
import { formatShortcut, shortcutFromEvent, validateShortcut } from '../../lib/shortcuts';
import './tools.css';

export function GlobalSearchOptions({ snapshot, onResetLearning }: { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void>; onResetLearning?: () => void }) {
  const { t } = useTranslation();
  const saved = snapshot.settings.globalSearch;
  const [binding, setBinding] = useState(saved.binding);
  const [enabled, setEnabled] = useState(saved.enabled);
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState(false);
  const recorder = useRef<HTMLButtonElement>(null);
  useEffect(() => { setBinding(saved.binding); setEnabled(saved.enabled); }, [saved.binding, saved.enabled]);
  async function save() {
    setBusy(true); setError(null); setMessage(false);
    try { await api.configureGlobalSearch({ enabled, binding }); setMessage(true); }
    catch { setError('features.globalSearch.conflict'); }
    finally { setBusy(false); }
  }
  return <section className="tool-card" data-setting-id="global-search" tabIndex={-1}>
    <div className="tool-section-heading"><h3><Keyboard size={18} />{t('features.globalSearch.title')}</h3><button role="switch" aria-label={t('features.globalSearch.enable')} aria-checked={enabled} className={`switch ${enabled ? 'enabled' : ''}`} disabled={!isDesktop || busy} onClick={() => { setEnabled(!enabled); setMessage(false); }}><span /></button></div>
    <p className="tool-description">{t('features.globalSearch.description')}</p>
    <div className="tool-inline-fields"><label>{t('features.globalSearch.binding')}<button ref={recorder} data-shortcut-recorder="true" className={`button secondary shortcut-recorder ${recording ? 'recording' : ''}`} aria-pressed={recording} disabled={!isDesktop || busy} onClick={() => { setRecording(!recording); setError(null); }} onBlur={() => setRecording(false)} onKeyDown={event => {
      if (!recording) return;
      event.preventDefault(); event.stopPropagation();
      if (event.key === 'Escape') { setRecording(false); return; }
      const next = shortcutFromEvent(event.nativeEvent);
      if (!next) return;
      if (validateShortcut(next) || !(event.ctrlKey || event.altKey) || next === 'Ctrl+KeyF') { setError('features.globalSearch.invalidBinding'); return; }
      setBinding(next); setRecording(false); setError(null); setMessage(false);
    }}>{recording ? t('features.globalSearch.pressKeys') : formatShortcut(binding)}</button></label><button className="button primary" disabled={!isDesktop || busy || recording || (binding === saved.binding && enabled === saved.enabled)} onClick={() => void save()}>{t('features.common.apply')}</button></div>
    <p className="tool-hint">{t('features.globalSearch.bindingHint')}</p>
    {error && <p className="tool-error" role="alert">{t(error)}</p>}{message && <p className="tool-success" role="status">{t('features.globalSearch.saved')}</p>}
    {onResetLearning && <div className="tool-learning-reset"><p className="tool-hint">{t('features.globalSearch.learningHint')}</p><button className="button quiet" onClick={() => { onResetLearning(); setMessage(false); }}><RotateCcw size={15} />{t('features.globalSearch.resetLearning')}</button></div>}
  </section>;
}
