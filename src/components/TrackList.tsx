import { useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderPlus, Heart, Scissors, Check, Clock3, Ellipsis, Info, ListEnd, ListPlus, Play, Plus, X } from 'lucide-react';
import type { Playlist, Track } from '../lib/backend/types';
import { Cover } from './Cover';
import { time } from '../lib/format';

interface Props {
  tracks: Track[];
  currentTrackId?: string | null;
  playing: boolean;
  playlists: Playlist[];
  selected: Set<number>;
  onSelect: (index: number, shift: boolean) => void;
  onSelectAll: () => void;
  onPlay: (index: number) => void;
  onAppend: (tracks: Track[]) => void;
  onNext: (tracks: Track[]) => void;
  onInfo: (track: Track) => void;
  onAddPlaylist: (playlistId: string, tracks: Track[]) => void;
  favoriteIds?: string[];
  onFavorite?: (tracks: Track[]) => void;
  onManagePlaylists?: (tracks: Track[]) => void;
  onEdit?: (track: Track) => void;
  onAddFolder?: (track: Track) => void;
  canAddFolder?: (track: Track) => boolean;
  onRemove?: (index: number) => void;
}

export function TrackList(p: Props) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState<number | null>(null);
  const [menuPosition, setMenuPosition] = useState({ top: 0, right: 0 });
  const rowsRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState({ start: 0, end: 50 });
  const virtual = p.tracks.length > 100;
  useLayoutEffect(() => {
    if (!virtual) return;
    const rows = rowsRef.current;
    const scrollPane = rows?.closest<HTMLElement>('.main-scroll');
    if (!rows || !scrollPane) return;
    let frame = 0;
    const measure = () => {
      const top = Math.max(0, scrollPane.getBoundingClientRect().top - rows.getBoundingClientRect().top);
      const start = Math.max(0, Math.floor(top / 66) - 7);
      const end = Math.min(p.tracks.length, Math.ceil((top + scrollPane.clientHeight) / 66) + 7);
      setViewport(current => current.start === start && current.end === end ? current : { start, end });
    };
    const scroll = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(measure); setMenu(null); };
    const observer = new ResizeObserver(measure);
    observer.observe(scrollPane);
    scrollPane.addEventListener('scroll', scroll, { passive: true });
    measure();
    return () => { cancelAnimationFrame(frame); observer.disconnect(); scrollPane.removeEventListener('scroll', scroll); };
  }, [p.tracks, virtual]);
  const selectedTracks = p.tracks.filter((_, i) => p.selected.has(i));
  return <div className="track-list">
    <div className="track-table-head track-grid">
      <button className={`check-button ${p.selected.size === p.tracks.length && p.tracks.length > 0 ? 'checked' : ''}`} aria-label={t('app.selectAll')} onClick={p.onSelectAll}><Check size={11} /></button>
      <span>{t('library.titleColumn')}</span><span className="album-cell">{t('library.albumColumn')}</span><span className="format-cell">{t('library.formatColumn')}</span><span className="duration-cell"><Clock3 size={14} /><span className="sr-only">{t('library.durationColumn')}</span></span><span />
    </div>
    <div role="list" className="track-rows" ref={rowsRef} style={virtual ? { position: 'relative', height: p.tracks.length * 66 } : undefined}>
      {(virtual ? p.tracks.slice(viewport.start, viewport.end) : p.tracks).map((track, rowIndex) => {
        const index = rowIndex + (virtual ? viewport.start : 0);
        const current = p.currentTrackId === track.id;
        const targets = p.selected.has(index) ? selectedTracks : [track];
        return <div role="listitem" aria-setsize={p.tracks.length} aria-posinset={index + 1} className={`track-row track-grid ${current ? 'is-current' : ''} ${p.selected.has(index) ? 'selected' : ''}`} style={virtual ? { position: 'absolute', top: index * 66, left: 0, right: 0, height: 66 } : undefined} key={`${track.id}-${index}`} onDoubleClick={() => p.onPlay(index)}>
          <button className={`check-button ${p.selected.has(index) ? 'checked' : ''}`} aria-label={`${t('app.selectAll')}: ${track.title}`} aria-pressed={p.selected.has(index)} onClick={e => p.onSelect(index, e.shiftKey)}><Check size={11} /></button>
          <div className="track-identity"><button className="track-cover-button" title={t('player.play')} onClick={() => p.onPlay(index)}><Cover track={track} /><span className="cover-play"><Play size={15} fill="currentColor" /></span></button><div className="truncate"><span className="track-name">{track.title}{p.favoriteIds?.includes(track.id) && <Heart size={12} className="favorite-on" fill="currentColor" aria-label={t('workspace.favorites')} />}</span><span className="track-artist">{track.artist || t('library.unknownArtist')}</span></div>{current && p.playing && <span className="equalizer" aria-label={t('player.playing')}><i /><i /><i /></span>}</div>
          <span className="album-cell text-secondary truncate">{track.album || t('library.unknownAlbum')}</span><span className="format-cell"><span className="format-badge">{track.format || basenameFormat(track.path)}</span></span><span className="duration-cell text-secondary">{time(track.duration)}</span>
          <div className="row-menu-wrap"><button className={`icon-button small ${menu === index ? 'is-on' : ''}`} title={t('app.more')} aria-label={`${t('app.more')}: ${track.title}`} onClick={event => { const rect = event.currentTarget.getBoundingClientRect(); setMenuPosition({ top: Math.max(12, Math.min(rect.bottom + 4, window.innerHeight - 340)), right: window.innerWidth - rect.right }); setMenu(menu === index ? null : index); }}><Ellipsis size={18} /></button>
            {menu === index && <><button className="menu-dismiss" aria-label={t('app.close')} onClick={() => setMenu(null)} /><div className="context-menu row-context-menu" style={{ position: 'fixed', top: menuPosition.top, right: menuPosition.right, maxHeight: 'calc(100dvh - 30px)', overflowY: 'auto' }}>
              <button onClick={() => { p.onPlay(index); setMenu(null); }}><Play size={15} />{t('player.play')}</button>
              <button onClick={() => { p.onNext(targets); setMenu(null); }}><ListPlus size={15} />{t('queue.playNext')}</button>
              <button onClick={() => { p.onAppend(targets); setMenu(null); }}><ListEnd size={15} />{t('queue.add')}</button>
              <button onClick={() => { p.onManagePlaylists?.(targets); setMenu(null); }}><Plus size={15} />{t('workspace.membership')}</button>
              <button onClick={() => { p.onFavorite?.(targets); setMenu(null); }}><Heart size={15} fill={targets.every(item => p.favoriteIds?.includes(item.id)) ? 'currentColor' : 'none'} />{t(targets.every(item => p.favoriteIds?.includes(item.id)) ? 'workspace.unfavorite' : 'workspace.favorite')}</button>
              <button onClick={() => { p.onEdit?.(track); setMenu(null); }}><Scissors size={15} />{t('workspace.editAudio')}</button>
              {p.onAddFolder && p.canAddFolder?.(track) && <button onClick={() => { p.onAddFolder?.(track); setMenu(null); }}><FolderPlus size={15} />{t('workspace.addContainingFolder')}</button>}
              <div className="menu-divider" /><button onClick={() => { p.onInfo(track); setMenu(null); }}><Info size={15} />{t('library.trackInfo')}</button>
              {p.onRemove && <button className="danger" onClick={() => { p.onRemove!(index); setMenu(null); }}><X size={15} />{t('playlist.removeTrack')}</button>}
            </div></>}
          </div>
        </div>;
      })}
    </div>
  </div>;
}

function basenameFormat(path: string) { return path.split('.').at(-1)?.toUpperCase() || '—'; }
