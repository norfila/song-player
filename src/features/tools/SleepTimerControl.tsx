import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Moon, X } from 'lucide-react';
import type { AppSnapshot, PlayerCommand } from '../../lib/backend/types';
import { displayTime } from './tool-utils';
import { ToolDialog } from './ToolDialog';
import './tools.css';

interface Props { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void>; compact?: boolean }

export function SleepTimerControl({ snapshot, onCommand, compact = false }: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [now, setNow] = useState(() => Date.now() / 1000);
  const timer = snapshot.sleepTimer;
  useEffect(() => {
    if (!timer?.endsAt) return;
    setNow(Date.now() / 1000);
    const interval = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(interval);
  }, [timer?.endsAt]);
  const remaining = timer?.endsAt ? displayTime(Math.max(0, timer.endsAt - now)) : timer ? t(`features.sleep.${timer.mode === 'track' ? 'afterTrackShort' : 'afterQueueShort'}`) : '';
  const label = timer ? `${t('features.sleep.title')}: ${remaining}` : t('features.sleep.title');
  return <>
    <button type="button" className={`${compact ? 'sleep-timer-trigger compact' : 'sleep-timer-trigger'} ${timer ? 'is-active' : ''}`} title={label} aria-label={label} aria-haspopup="dialog" onClick={() => setOpen(true)}><Moon size={16} /><span>{timer ? remaining : t('features.sleep.title')}</span></button>
    {open && <SleepTimerDialog snapshot={snapshot} onCommand={onCommand} onClose={() => setOpen(false)} />}
  </>;
}

export function SleepTimerDialog({ snapshot, onCommand, onClose }: Omit<Props, 'compact'> & { onClose: () => void }) {
  const { t } = useTranslation();
  const [minutes, setMinutes] = useState(30);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  async function apply(mode: 'minutes' | 'track' | 'queue', duration = minutes) {
    if (mode === 'minutes' && (!Number.isFinite(duration) || duration < 1 || duration > 1440)) return;
    setBusy(true); setError(false);
    try { await onCommand({ type: 'set_sleep_timer', mode, minutes: duration }); onClose(); }
    catch { setError(true); }
    finally { setBusy(false); }
  }
  return <ToolDialog title={t('features.sleep.title')} onClose={onClose} className="sleep-timer-dialog">
    <p className="tool-description">{t('features.sleep.description')}</p>
    {error && <p role="alert" className="tool-error">{t('features.common.failed')}</p>}
    <div className="sleep-presets">{[15, 30, 60].map(value => <button key={value} className="button secondary" disabled={busy} onClick={() => void apply('minutes', value)}>{t('features.sleep.minutes', { count: value })}</button>)}</div>
    <form className="sleep-custom" onSubmit={event => { event.preventDefault(); void apply('minutes'); }}>
      <label htmlFor="sleep-custom-minutes">{t('features.sleep.custom')}</label><div><input id="sleep-custom-minutes" type="number" min={1} max={1440} step={1} value={minutes} disabled={busy} onChange={event => setMinutes(Number(event.target.value))} /><span>{t('features.sleep.minuteUnit')}</span><button className="button primary" disabled={busy || !Number.isInteger(minutes) || minutes < 1 || minutes > 1440}>{t('features.sleep.start')}</button></div>
    </form>
    <div className="sleep-end-options"><button className="button quiet" disabled={busy || !snapshot.playback.trackId} onClick={() => void apply('track')}>{t('features.sleep.afterTrack')}</button><button className="button quiet" disabled={busy || !(snapshot.sessions.find(item => item.id === snapshot.activeSessionId)?.entries.length)} onClick={() => void apply('queue')}>{t('features.sleep.afterQueue')}</button></div>
    <p className="tool-hint">{t('features.sleep.queueHint')}</p>
    {snapshot.sleepTimer && <button className="button quiet" disabled={busy} onClick={() => {
      setBusy(true); void onCommand({ type: 'cancel_sleep_timer' }).then(onClose).catch(() => setError(true)).finally(() => setBusy(false));
    }}><X size={15} />{t('features.sleep.cancel')}</button>}
  </ToolDialog>;
}
