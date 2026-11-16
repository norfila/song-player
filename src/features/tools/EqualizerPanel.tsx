import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, SlidersHorizontal } from 'lucide-react';
import type { AppSnapshot, EqualizerSettings, PlayerCommand } from '../../lib/backend/types';
import { Select } from '../../components/Select';
import './tools.css';

const FREQUENCIES = ['60 Hz', '250 Hz', '1 kHz', '4 kHz', '12 kHz'];
const PRESETS: Record<string, number[]> = { flat: [0, 0, 0, 0, 0], bass: [4, 2, 0, -1, -1], voice: [-2, 0, 3, 2, -1], soft: [0, 0, -1, -3, -4] };
interface Props { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void> }

export function EqualizerPanel({ snapshot, onCommand }: Props) {
  const { t } = useTranslation();
  const saved = snapshot.settings.equalizer;
  const [draft, setDraft] = useState(saved);
  const latest = useRef(draft);
  const dirty = useRef(false);
  const [error, setError] = useState(false);
  const version = useRef(0);
  const serialized = JSON.stringify(saved);
  useEffect(() => { if (!dirty.current) { setDraft(saved); latest.current = saved; } }, [serialized]);
  function update(value: EqualizerSettings) { dirty.current = true; latest.current = value; setDraft(value); }
  async function save(value = latest.current) {
    if (!dirty.current && JSON.stringify(value) === serialized) return;
    const ownVersion = ++version.current;
    setError(false);
    try { await onCommand({ type: 'set_equalizer', equalizer: value }); if (ownVersion === version.current && JSON.stringify(latest.current) === JSON.stringify(value)) dirty.current = false; }
    catch { if (ownVersion === version.current) setError(true); }
  }
  function apply(value: EqualizerSettings) { update(value); void save(value); }
  const preset = Object.keys(PRESETS).find(key => PRESETS[key].every((value, index) => value === draft.bands[index])) || 'custom';
  const safePreamp = -Math.max(0, ...draft.bands);
  return <section className="tool-card equalizer-panel" data-setting-id="equalizer" tabIndex={-1}>
    <div className="tool-section-heading"><h3><SlidersHorizontal size={18} />{t('features.equalizer.title')}</h3><button role="switch" className={`switch ${draft.enabled ? 'enabled' : ''}`} aria-label={t('features.equalizer.enable')} aria-checked={draft.enabled} disabled={!snapshot.engine.available} onClick={() => apply({ ...draft, enabled: !draft.enabled })}><span /></button></div>
    <p className="tool-description">{t('features.equalizer.description')}</p>
    {error && <p className="tool-error" role="alert">{t('features.common.failed')} <button className="tool-text-button" onClick={() => void save()}>{t('features.common.retry')}</button></p>}
    <div className="tool-inline-fields"><label>{t('features.equalizer.preset')}<Select label={t('features.equalizer.preset')} value={preset} options={[...Object.keys(PRESETS).map(key => ({ value: key, label: t(`features.equalizer.presets.${key}`) })), { value: 'custom', label: t('features.equalizer.presets.custom'), disabled: true }]} disabled={!snapshot.engine.available} onChange={key => { const bands = [...PRESETS[key]]; apply({ ...draft, bands, preamp: -Math.max(0, ...bands) }); }} /></label><button className="button quiet" disabled={!snapshot.engine.available} onClick={() => apply({ ...draft, bands: [0, 0, 0, 0, 0], preamp: 0 })}><RotateCcw size={15} />{t('features.common.reset')}</button></div>
    <div className="equalizer-bands">{draft.bands.map((gain, index) => <label className="equalizer-band" key={index}><span>{FREQUENCIES[index]}</span><input type="range" min={-12} max={12} step={.5} value={gain} disabled={!snapshot.engine.available} aria-label={t('features.equalizer.band', { frequency: FREQUENCIES[index] })} aria-valuetext={`${gain > 0 ? '+' : ''}${gain} dB`} onChange={event => {
      const bands = [...latest.current.bands]; bands[index] = Number(event.target.value);
      update({ ...latest.current, bands, preamp: Math.min(latest.current.preamp, -Math.max(0, ...bands)) });
    }} onPointerUp={() => void save()} onKeyUp={() => void save()} onBlur={() => void save()} /><output>{gain > 0 ? '+' : ''}{gain} dB</output></label>)}</div>
    <label className="tool-range-field"><span>{t('features.equalizer.preamp')}<output>{draft.preamp} dB</output></span><input type="range" min={-24} max={0} step={.5} value={draft.preamp} disabled={!snapshot.engine.available} onChange={event => update({ ...latest.current, preamp: Number(event.target.value) })} onPointerUp={() => void save()} onKeyUp={() => void save()} onBlur={() => void save()} /></label>
    <p className="tool-hint">{t('features.equalizer.headroomHint')}</p>
    {draft.preamp > safePreamp && <p className="tool-warning">{t('features.equalizer.clipping')} <button className="tool-text-button" onClick={() => apply({ ...draft, preamp: safePreamp })}>{t('features.equalizer.compensate')}</button></p>}
  </section>;
}
