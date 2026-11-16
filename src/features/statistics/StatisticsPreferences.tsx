import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Trash2 } from 'lucide-react';
import { api, isDesktop } from '../../lib/backend';
import type { AppSnapshot, PlayerCommand } from '../../lib/backend/types';
import { ToolDialog } from '../tools/ToolDialog';
import '../tools/tools.css';
import './statistics-preferences.css';

export function StatisticsPreferences({ snapshot, onCommand }: { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void> }) {
  const { t } = useTranslation();
  const prefix = useId();
  const saved = snapshot.settings.statistics;
  const [percent, setPercent] = useState(saved.percent);
  const [seconds, setSeconds] = useState(saved.seconds);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [feedback, setFeedback] = useState<'exported' | 'cleared' | null>(null);
  const [clearOpen, setClearOpen] = useState(false);
  useEffect(() => { setPercent(saved.percent); setSeconds(saved.seconds); }, [saved.percent, saved.seconds]);
  const validThreshold = Number.isInteger(percent) && percent >= 1 && percent <= 100 && Number.isInteger(seconds) && seconds >= 0 && seconds <= 3600;
  async function operation(action: () => Promise<void>) {
    setBusy(true); setError(false); setFeedback(null);
    try { await action(); }
    catch { setError(true); }
    finally { setBusy(false); }
  }
  return <section className="tool-card statistics-settings-card" data-setting-id="statistics-preferences" tabIndex={-1} aria-labelledby={`${prefix}-title`}>
    <h3 id={`${prefix}-title`}>{t('features.statistics.settings')}</h3>
    {error && !clearOpen && <p className="tool-error" role="alert">{t('features.common.failed')}</p>}
    {feedback && <p className="tool-success" role="status">{t(`features.statistics.${feedback}`)}</p>}
    <div className="tool-toggle-row">
      <div><label id={`${prefix}-enabled`}>{t('features.statistics.record')}</label><p>{t('features.statistics.recordHint')}</p></div>
      <button type="button" role="switch" aria-labelledby={`${prefix}-enabled`} aria-checked={saved.enabled} className={`switch ${saved.enabled ? 'enabled' : ''}`} disabled={!isDesktop || busy} onClick={() => void operation(() => onCommand({ type: 'set_statistics_settings', settings: { ...saved, enabled: !saved.enabled } }))}><span /></button>
    </div>
    <div className="statistics-threshold-settings">
      <h4>{t('features.statistics.thresholdTitle')}</h4>
      <p className="tool-description">{t('features.statistics.thresholdHint')}</p>
      <form onSubmit={event => { event.preventDefault(); if (validThreshold && !busy && isDesktop) void operation(() => onCommand({ type: 'set_statistics_settings', settings: { enabled: saved.enabled, percent, seconds } })); }}>
        <div className="tool-inline-fields statistics-threshold-fields">
          <label>{t('features.statistics.percent')}<input type="number" min={1} max={100} step={1} value={percent} disabled={busy} onChange={event => setPercent(Number(event.target.value))} /></label>
          <label>{t('features.statistics.seconds')}<input type="number" min={0} max={3600} step={1} value={seconds} disabled={busy} onChange={event => setSeconds(Number(event.target.value))} /></label>
          <button className="button secondary" disabled={!isDesktop || busy || !validThreshold || (saved.percent === percent && saved.seconds === seconds)}>{t('features.common.apply')}</button>
        </div>
      </form>
      <p className="tool-hint">{t('features.statistics.recalculateHint')}</p>
    </div>
    <div className="tool-actions statistics-data-actions">
      <button type="button" className="button quiet" disabled={!isDesktop || busy} onClick={() => void operation(async () => { if (await api.exportHistoryFile()) setFeedback('exported'); })}><Download size={15} />{t('features.statistics.export')}</button>
      <button type="button" className="button quiet danger" disabled={!isDesktop || busy} onClick={() => { setError(false); setClearOpen(true); }}><Trash2 size={15} />{t('features.statistics.clear')}</button>
    </div>
    {clearOpen && <ToolDialog title={t('features.statistics.clearTitle')} onClose={() => { if (!busy) setClearOpen(false); }}>
      <p className="tool-description">{t('features.statistics.clearHint')}</p>
      {error && <p className="tool-error" role="alert">{t('features.common.failed')}</p>}
      <div className="tool-actions tool-dialog-actions">
        <button type="button" className="button quiet" disabled={busy} onClick={() => setClearOpen(false)}>{t('features.common.cancel')}</button>
        <button type="button" className="button danger tool-destructive" disabled={busy} onClick={() => void operation(async () => { await api.clearStatistics(); setClearOpen(false); setFeedback('cleared'); })}><Trash2 size={15} />{t('features.statistics.clearConfirm')}</button>
      </div>
    </ToolDialog>}
  </section>;
}
