import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BarChart3, Clock3, Music2, Play, RefreshCw, Settings2, Users } from 'lucide-react';
import { api } from '../../lib/backend';
import type { AppSnapshot, Statistics } from '../../lib/backend/types';
import { Select } from '../../components/Select';
import { displayTime } from '../tools/tool-utils';
import '../tools/tools.css';
import './statistics.css';

type Period = 'month' | 'year' | 'all';
const EMPTY: Statistics = { plays: 0, listenedSeconds: 0, tracks: [], artists: [] };

export function statisticsRange(period: Period, year: number, month: number): [number | null, number | null] {
  if (period === 'all') return [null, null];
  return [Math.floor(new Date(year, period === 'year' ? 0 : month, 1).getTime() / 1000), Math.floor(new Date(period === 'year' ? year + 1 : year, period === 'year' ? 0 : month + 1, 1).getTime() / 1000)];
}

export function StatisticsPage({ snapshot, onPlay, onOpenSettings }: { snapshot: AppSnapshot; onPlay: (trackId: string) => void; onOpenSettings: () => void }) {
  const { t, i18n } = useTranslation();
  const now = useMemo(() => new Date(), []);
  const [period, setPeriod] = useState<Period>('month');
  const [year, setYear] = useState(now.getFullYear());
  const [years, setYears] = useState<number[]>([now.getFullYear()]);
  const [month, setMonth] = useState(now.getMonth());
  const [statistics, setStatistics] = useState<Statistics>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [metric, setMetric] = useState('plays');
  const [visible, setVisible] = useState(50);
  const saved = snapshot.settings.statistics;
  const requests = useRef(0);
  const range = statisticsRange(period, year, month);
  const rangeRef = useRef(range);
  rangeRef.current = range;
  async function refresh() {
    const request = ++requests.current;
    setBusy(true); setError(false);
    try {
      const [result, recordedYears] = await Promise.all([api.getStatistics(...rangeRef.current), api.getStatisticsYears()]);
      if (request === requests.current) {
        const availableYears = [...new Set(recordedYears)].sort((a, b) => b - a);
        if (!availableYears.length) availableYears.push(now.getFullYear());
        setStatistics(result);
        setYears(availableYears);
        setYear(selected => availableYears.includes(selected) ? selected : availableYears[0]);
      }
    }
    catch { if (request === requests.current) setError(true); }
    finally { if (request === requests.current) setBusy(false); }
  }
  useEffect(() => {
    setVisible(50); void refresh();
    const interval = window.setInterval(() => { if (document.visibilityState === 'visible') void refresh(); }, 30000);
    return () => { window.clearInterval(interval); requests.current++; };
  }, [period, year, month, saved.enabled, saved.percent, saved.seconds]);
  const tracks = useMemo(() => [...statistics.tracks].sort((a, b) => metric === 'time' ? b.listenedSeconds - a.listenedSeconds || b.plays - a.plays : b.plays - a.plays || b.listenedSeconds - a.listenedSeconds), [statistics, metric]);
  const artists = useMemo(() => [...statistics.artists].sort((a, b) => metric === 'time' ? b.listenedSeconds - a.listenedSeconds || b.plays - a.plays : b.plays - a.plays || b.listenedSeconds - a.listenedSeconds), [statistics, metric]);
  const knownIds = useMemo(() => new Set(snapshot.tracks.map(track => track.id)), [snapshot.tracks]);
  const months = Array.from({ length: 12 }, (_, index) => ({ value: String(index), label: new Intl.DateTimeFormat(i18n.language, { month: 'long' }).format(new Date(2024, index, 1)) }));
  return <section className="statistics-page" aria-label={t('features.statistics.title')}>
    <div className="statistics-toolbar"><div className="statistics-periods" role="group" aria-label={t('features.statistics.period')}>{(['month', 'year', 'all'] as Period[]).map(value => <button key={value} className={period === value ? 'active' : ''} aria-pressed={period === value} onClick={() => setPeriod(value)}>{t(`features.statistics.periods.${value}`)}</button>)}</div>
      {period !== 'all' && <div className="statistics-date-fields">{period === 'month' && <Select className="statistics-month-select" label={t('features.statistics.month')} value={String(month)} options={months} onChange={value => setMonth(Number(value))} />}<Select className="statistics-year-select" label={t('features.statistics.year')} value={String(year)} options={years.map(value => ({ value: String(value), label: String(value) }))} onChange={value => setYear(Number(value))} /></div>}
      <div className="statistics-toolbar-actions"><button className={`icon-button ${busy ? 'spin' : ''}`} aria-label={t('features.statistics.refresh')} title={t('features.statistics.refresh')} disabled={busy} onClick={() => void refresh()}><RefreshCw size={17} /></button><button className="icon-button" aria-label={t('features.statistics.openSettings')} title={t('features.statistics.openSettings')} onClick={onOpenSettings}><Settings2 size={17} /></button></div>
    </div>
    {error && <p className="tool-error" role="alert">{t('features.common.failed')}</p>}
    {!saved.enabled && <p className="statistics-disabled"><BarChart3 size={17} />{t('features.statistics.disabled')}</p>}
    <div className="statistics-totals" aria-busy={busy}>
      <div><Music2 size={19} /><strong>{new Intl.NumberFormat(i18n.language).format(statistics.plays)}</strong><span>{t('features.statistics.plays')}</span></div>
      <div><Clock3 size={19} /><strong>{displayTime(statistics.listenedSeconds)}</strong><span>{t('features.statistics.listeningTime')}</span></div>
      <div><Users size={19} /><strong>{new Intl.NumberFormat(i18n.language).format(statistics.artists.length)}</strong><span>{t('features.statistics.artists')}</span></div>
    </div>
    <div className="statistics-ranking-heading"><h2>{t('features.statistics.mostPlayed')}</h2><Select value={metric} onChange={setMetric} label={t('features.statistics.rankBy')} options={[{ value: 'plays', label: t('features.statistics.byPlays') }, { value: 'time', label: t('features.statistics.byTime') }]} /></div>
    {!tracks.length ? <div className="statistics-empty"><BarChart3 size={32} /><h3>{t('features.statistics.empty')}</h3><p>{t('features.statistics.emptyHint')}</p></div> : <div className="statistics-rankings">
      <div className="statistics-table-wrap"><h3>{t('features.statistics.songs')}</h3><table><thead><tr><th scope="col">{t('features.statistics.song')}</th><th scope="col">{t('features.statistics.plays')}</th><th scope="col">{t('features.statistics.listeningTime')}</th></tr></thead><tbody>{tracks.slice(0, visible).map((track, index) => <tr key={track.trackId}><th scope="row"><div className="statistics-track"><span className="statistics-position">{index + 1}</span><button className="icon-button small" disabled={!knownIds.has(track.trackId)} aria-label={t('features.statistics.playSong', { title: track.title })} onClick={() => onPlay(track.trackId)}><Play size={14} /></button><span><strong>{track.title}</strong><small>{track.artist || t('library.unknownArtist')}</small></span></div></th><td>{track.plays}</td><td>{displayTime(track.listenedSeconds)}</td></tr>)}</tbody></table></div>
      <div className="statistics-table-wrap statistics-artists"><h3>{t('features.statistics.artists')}</h3><table><thead><tr><th scope="col">{t('features.statistics.artist')}</th><th scope="col">{t('features.statistics.plays')}</th><th scope="col">{t('features.statistics.listeningTime')}</th></tr></thead><tbody>{artists.slice(0, visible).map(artist => <tr key={artist.artist}><th scope="row">{artist.artist || t('library.unknownArtist')}</th><td>{artist.plays}</td><td>{displayTime(artist.listenedSeconds)}</td></tr>)}</tbody></table></div>
    </div>}
    {(tracks.length > visible || artists.length > visible) && <button className="button quiet statistics-more" onClick={() => setVisible(value => value + 50)}>{t('features.statistics.showMore')}</button>}
  </section>;
}
