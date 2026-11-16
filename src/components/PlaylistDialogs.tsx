import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Plus, Search, Upload, X } from 'lucide-react';
import type { Playlist, PlaylistIcon as IconValue, Track } from '../lib/backend/types';
import { api, isDesktop } from '../lib/backend';
import { Dialog } from './Dialog';
import { PLAYLIST_ICONS, PlaylistIcon } from './PlaylistIcon';
import './expansion.css';

export function PlaylistMembershipDialog({ tracks, playlists, onSave, onCreate, onClose }: { tracks: Track[]; playlists: Playlist[]; onSave: (ids: string[], preserve: string[]) => Promise<void>; onCreate: (name: string) => Promise<string>; onClose: () => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [choices, setChoices] = useState<Partial<Record<string, boolean>>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const membership = (list: Playlist) => {
    const count = tracks.filter(track => list.trackIds.includes(track.id)).length;
    return count === 0 ? false : count === tracks.length ? true : 'mixed';
  };
  const save = async () => {
    setBusy(true); setError(false);
    try { await onSave(playlists.filter(list => choices[list.id] ?? membership(list) === true).map(list => list.id), playlists.filter(list => choices[list.id] === undefined && membership(list) === 'mixed').map(list => list.id)); onClose(); }
    catch { setError(true); } finally { setBusy(false); }
  };
  const results = playlists.filter(list => list.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  return <Dialog title={t('workspace.membership')} onClose={() => { if (!busy) onClose(); }} className="membership-dialog">
    <div className="expansion-dialog-heading"><div><h2>{t('workspace.membership')}</h2><p>{t('workspace.membershipHint', { count: tracks.length })}</p></div><button className="icon-button" aria-label={t('app.close')} disabled={busy} onClick={onClose}><X size={20} /></button></div>
    <div className="expansion-search"><Search size={17} /><input data-autofocus value={query} onChange={event => setQuery(event.target.value)} placeholder={t('workspace.searchPlaylists')} aria-label={t('workspace.searchPlaylists')} /></div>
    <div className="membership-results">{results.map(list => {
      const state = choices[list.id] ?? membership(list);
      return <label className="membership-choice" key={list.id}><input type="checkbox" checked={state === true} ref={input => { if (input) input.indeterminate = state === 'mixed'; }} disabled={busy} onChange={event => setChoices(current => ({ ...current, [list.id]: event.target.checked }))} /><PlaylistIcon icon={list.icon} /><span>{list.name}{state === 'mixed' && <small>{t('workspace.someSelected')}</small>}</span><small>{t('library.tracks', { count: list.trackIds.length })}</small></label>;
    })}{!results.length && <p className="text-secondary">{t('app.noResults')}</p>}</div>
    {error && <p className="expansion-error" role="alert">{t('errors.unknown')}</p>}
    <div className="expansion-dialog-actions"><button className="button secondary" disabled={busy || !query.trim()} onClick={() => { setBusy(true); setError(false); const name = query.trim(); void onCreate(name).then(id => { setChoices(current => ({ ...current, [id]: true })); setQuery(name); }).catch(() => setError(true)).finally(() => setBusy(false)); }}><Plus size={16} />{t('workspace.createNamedPlaylist')}</button><button className="button primary" disabled={busy} onClick={() => void save()}><Check size={16} />{t('appearance.apply')}</button></div>
  </Dialog>;
}

export function PlaylistIconDialog({ playlist, onSave, onClose }: { playlist: Playlist; onSave: (icon: IconValue) => Promise<void>; onClose: () => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false), [error, setError] = useState(false);
  const save = async (value: IconValue) => { setBusy(true); setError(false); try { await onSave(value); onClose(); } catch { setError(true); } finally { setBusy(false); } };
  return <Dialog title={t('workspace.changeIcon')} onClose={() => { if (!busy) onClose(); }}>
    <div className="expansion-dialog-heading"><h2>{t('workspace.changeIcon')}</h2><button className="icon-button" disabled={busy} onClick={onClose} aria-label={t('app.close')}><X size={20} /></button></div><p className="text-secondary">{playlist.name}</p>
    <div className="playlist-icon-picker">{Object.keys(PLAYLIST_ICONS).map(value => <button className={`icon-button ${playlist.icon?.kind === 'builtin' && playlist.icon.value === value ? 'is-on' : ''}`} key={value} aria-label={t(`workspace.icons.${value}`)} disabled={busy} onClick={() => void save({ kind: 'builtin', value })}><PlaylistIcon icon={{ kind: 'builtin', value }} size={27} /></button>)}</div>
    {error && <p className="expansion-error" role="alert">{t('errors.unknown')}</p>}
    <button className="button secondary" disabled={!isDesktop || busy} onClick={() => { setBusy(true); setError(false); void api.importPlaylistIcon().then(icon => icon ? save(icon) : undefined).catch(() => setError(true)).finally(() => setBusy(false)); }}><Upload size={16} />{t('workspace.uploadIcon')}</button>
  </Dialog>;
}
