import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, AudioLines, FolderPlus, Info, Ellipsis, Heart, ListPlus, Scissors, SlidersHorizontal } from 'lucide-react';
import type { AppSnapshot, PlayerCommand, Track } from '../../lib/backend/types';
import { Cover } from '../../components/Cover';
import { SeekControl, TransportControls, VolumeControl } from './PlayerControls';
import './player.css';
import { SleepTimerControl } from '../tools';

export interface NowPlayingProps {
  snapshot: AppSnapshot;
  onTimerCommand?: (value: PlayerCommand) => Promise<void>;
  onCommand: (value: PlayerCommand) => Promise<void>;
  onClose: () => void;
  onDetails: (track: Track) => void;
  onPlaylist: (track: Track) => void;
  onFavorite: (track: Track) => void;
  onEdit: (track: Track) => void;
  onEqualizer: () => void;
  onAddFolder?: (track: Track) => void;
  canAddFolder?: boolean;
}

export function NowPlaying({ snapshot, onTimerCommand, onCommand, onClose, onDetails, onPlaylist, onFavorite, onEdit, onEqualizer, onAddFolder, canAddFolder }: NowPlayingProps) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState(false);
  const page = useRef<HTMLElement>(null);
  const track = snapshot.tracks.find(item => item.id === snapshot.playback.trackId);
  // Focus the view (not the back button) so Escape works without a focus ring on open.
  useEffect(() => { page.current?.focus({ preventScroll: true }); }, []);

  return <section ref={page} tabIndex={-1} className="now-playing-page" aria-labelledby="now-playing-title" onKeyDown={event => {
    if (event.key === 'Escape' && !event.defaultPrevented) { event.stopPropagation(); if (menu) setMenu(false); else onClose(); }
  }}>
    <header className="now-page-header">
      <button className="button quiet now-back" onClick={onClose}><ArrowLeft size={17} /><span>{t('player.collapse')}</span></button>
      <span className="now-page-heading" id="now-playing-title">{t('player.nowPlaying')}</span>
      <div className="now-header-actions"><button className={`icon-button ${snapshot.favoriteTrackIds?.includes(track?.id || '') ? 'favorite-on' : ''}`} title={t(snapshot.favoriteTrackIds?.includes(track?.id || '') ? 'workspace.unfavorite' : 'workspace.favorite')} aria-label={t(snapshot.favoriteTrackIds?.includes(track?.id || '') ? 'workspace.unfavorite' : 'workspace.favorite')} aria-pressed={snapshot.favoriteTrackIds?.includes(track?.id || '') || false} disabled={!track} onClick={() => track && onFavorite(track)}><Heart size={20} fill={snapshot.favoriteTrackIds?.includes(track?.id || '') ? 'currentColor' : 'none'} /></button><button className="icon-button" title={t('workspace.membership')} aria-label={t('workspace.membership')} disabled={!track} onClick={() => track && onPlaylist(track)}><ListPlus size={20} /></button><div className="menu-wrap"><button className="icon-button" title={t('app.more')} aria-label={t('app.more')} aria-expanded={menu} onClick={() => setMenu(value => !value)}><Ellipsis size={20} /></button>{menu && <><button className="menu-dismiss" aria-label={t('app.close')} onClick={() => setMenu(false)} /><div className="context-menu"><SleepTimerControl snapshot={snapshot} onCommand={onTimerCommand || onCommand} /><button onClick={() => { setMenu(false); onEqualizer(); }}><SlidersHorizontal size={16} />{t('workspace.equalizer')}</button><button disabled={!track} onClick={() => { setMenu(false); if (track) onEdit(track); }}><Scissors size={16} />{t('workspace.editAudio')}</button>{track && onAddFolder && canAddFolder && <button onClick={() => { setMenu(false); onAddFolder(track); }}><FolderPlus size={16} />{t('workspace.addContainingFolder')}</button>}<button disabled={!track} onClick={() => { setMenu(false); if (track) onDetails(track); }}><Info size={16} />{t('library.trackInfo')}</button></div></>}</div></div>
    </header>
    <div className="now-page-content">
      <div className="now-page-art"><Cover track={track} size="large" /></div>
      <div className="now-page-controls">
        <div className="now-page-track">
          <h1 className={(track?.title.length ?? 0) > 32 ? 'is-long' : undefined} title={track?.title}>{track?.title || t('player.nothingPlaying')}</h1>
          <p className="now-page-artist">{track ? track.artist || t('library.unknownArtist') : t('player.chooseTrack')}</p>
          {track?.album && <p className="now-page-album">{track.album}{track.year ? ` · ${track.year}` : ''}</p>}
        </div>
        <SeekControl playback={snapshot.playback} hasTrack={!!track} onCommand={onCommand} variant={snapshot.settings.seekbarStyle} />
        <TransportControls snapshot={snapshot} onCommand={onCommand} expanded />
        <div className="now-audio-row">
          <span className="now-control-label">{t('player.volume')}</span>
          <VolumeControl playback={snapshot.playback} onCommand={onCommand} />
        </div>
        <div className="now-normalization-row">
          <div className="now-normalization-label"><AudioLines size={18} /><span id="now-normalize-label">{t('features.loudness.title')}</span></div>
          <button role="switch" aria-checked={snapshot.settings.replayGain} aria-labelledby="now-normalize-label" title={t('features.loudness.missingHint')} className={`switch ${snapshot.settings.replayGain ? 'enabled' : ''}`} disabled={!snapshot.engine.available} onClick={() => void onCommand({ type: 'set_replay_gain', enabled: !snapshot.settings.replayGain })}><span /></button>
        </div>
        {snapshot.sleepTimer && <SleepTimerControl snapshot={snapshot} onCommand={onTimerCommand || onCommand} compact />}
      </div>
    </div>
  </section>;
}
