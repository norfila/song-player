import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioLines, ScanLine } from 'lucide-react';
import { api, isDesktop } from '../../lib/backend';
import type { AppSnapshot, GainGrouping, PlayerCommand } from '../../lib/backend/types';
import { Select } from '../../components/Select';
import { AudioJobProgress, useAudioJob } from './useAudioJob';
import { errorCode } from './tool-utils';
import './tools.css';

export function LoudnessPanel({ snapshot, onCommand }: { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void> }) {
  const { t } = useTranslation();
  const [scope, setScope] = useState('library');
  const [writeTags, setWriteTags] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { job, setJob } = useAudioJob();
  const settings = snapshot.settings;
  const session = snapshot.sessions.find(item => item.id === snapshot.activeSessionId);
  const ids = useMemo(() => [...new Set(scope === 'current' ? snapshot.playback.trackId ? [snapshot.playback.trackId] : [] : scope === 'queue' ? session?.entries.map(item => item.track.id) ?? [] : snapshot.libraryTrackIds)], [scope, snapshot.playback.trackId, session?.entries, snapshot.libraryTrackIds]);
  const calculated = useMemo(() => { const selected = new Set(ids); return new Set(snapshot.loudnessValues.filter(value => value.grouping === settings.gainGrouping && selected.has(value.trackId)).map(value => value.trackId)).size; }, [snapshot.loudnessValues, settings.gainGrouping, ids]);
  const busy = starting || job?.status === 'running';
  async function change(command: PlayerCommand) {
    setError(null);
    try { await onCommand(command); }
    catch { setError('features.common.failed'); }
  }
  async function analyze() {
    setStarting(true); setError(null);
    try { setJob(await api.startLoudnessAnalysis({ trackIds: ids, grouping: settings.gainGrouping, writeTags: settings.gainGrouping !== 'folder' && writeTags })); }
    catch (failure) { setError(errorCode(failure) === 'unsupported_tags' ? 'features.loudness.unsupportedTags' : 'features.loudness.failed'); }
    finally { setStarting(false); }
  }
  return <section className="tool-card loudness-panel" data-setting-id="loudness" tabIndex={-1}>
    <div className="tool-toggle-row" data-setting-id="replaygain" tabIndex={-1}><div><h3><AudioLines size={18} />{t('features.loudness.title')}</h3><p>{t('features.loudness.description')}</p></div><button role="switch" aria-label={t('features.loudness.title')} aria-checked={settings.replayGain} className={`switch ${settings.replayGain ? 'enabled' : ''}`} disabled={!snapshot.engine.available} onClick={() => void change({ type: 'set_replay_gain', enabled: !settings.replayGain })}><span /></button></div>
    <p className="tool-hint">{t('features.loudness.missingHint')}</p>
    {error && <p role="alert" className="tool-error">{t(error)}</p>}
    <details className="tool-advanced"><summary>{t('features.loudness.advanced')}</summary>
      <label className="tool-field">{t('features.loudness.grouping')}<Select label={t('features.loudness.grouping')} value={settings.gainGrouping} options={(['track', 'album', 'folder'] as GainGrouping[]).map(value => ({ value, label: t(`features.loudness.groups.${value}`) }))} disabled={!isDesktop || busy} onChange={grouping => void change({ type: 'set_gain_grouping', grouping: grouping as GainGrouping })} /></label>
      <p className="tool-hint">{t(`features.loudness.groupHints.${settings.gainGrouping}`)}</p>
      <div className="tool-toggle-row" data-setting-id="normalization" tabIndex={-1}><div><label id="loudness-dynamics-label">{t('features.loudness.dynamics')}</label><p>{t('features.loudness.dynamicsHint')}</p></div><button role="switch" aria-labelledby="loudness-dynamics-label" aria-checked={settings.loudnessNormalization} className={`switch ${settings.loudnessNormalization ? 'enabled' : ''}`} disabled={!snapshot.engine.available} onClick={() => void change({ type: 'set_loudness_normalization', enabled: !settings.loudnessNormalization })}><span /></button></div>
    </details>
    <div className="loudness-analysis"><h4>{t('features.loudness.analyzeTitle')}</h4><label className="tool-field">{t('features.loudness.scope')}<Select label={t('features.loudness.scope')} value={scope} options={['library', 'queue', 'current'].map(value => ({ value, label: t(`features.loudness.scopes.${value}`) }))} disabled={busy} onChange={setScope} /></label>
      <p className="tool-hint">{t('features.loudness.scopeCount', { count: ids.length })}</p>
      <p className="tool-hint">{t('features.loudness.calculatedCount', { calculated, total: ids.length })}</p>
      <label className="tool-checkbox"><input type="checkbox" checked={writeTags && settings.gainGrouping !== 'folder'} disabled={busy || settings.gainGrouping === 'folder'} onChange={event => setWriteTags(event.target.checked)} /><span>{t('features.loudness.writeTags')}</span></label>
      <p className="tool-hint">{t(settings.gainGrouping === 'folder' ? 'features.loudness.folderTagsHint' : writeTags ? 'features.loudness.writeTagsHint' : 'features.loudness.localHint')}</p>
      <button className="button secondary" disabled={!isDesktop || busy || !ids.length} onClick={() => void analyze()}><ScanLine size={16} />{t('features.loudness.analyze')}</button>
    </div>
    {job && <AudioJobProgress job={job} onCancel={() => void api.cancelAudioJob().catch(() => setError('features.common.failed'))} />}
  </section>;
}
