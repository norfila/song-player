import { useTranslation } from 'react-i18next';
import { Info, Maximize2 } from 'lucide-react';
import type { AppSnapshot, PlayerCommand, Track } from '../../lib/backend/types';
import { Cover } from '../../components/Cover';
import { SeekControl, TransportControls, VolumeControl } from './PlayerControls';
import './player.css';

export interface PlayerBarProps {
  snapshot: AppSnapshot;
  onCommand: (value: PlayerCommand) => Promise<void>;
  onInfo: (track: Track) => void;
  onExpand?: () => void;
}

export function PlayerBar({ snapshot, onCommand, onInfo, onExpand }: PlayerBarProps) {
  const { t, i18n } = useTranslation();
  const track = snapshot.tracks.find(item => item.id === snapshot.playback.trackId);
  return <footer className="playerbar">
    <div className="now-playing">
      <button className="now-cover-button" title={t('player.expand')} aria-label={t('player.expand')} onClick={onExpand} disabled={!onExpand}>
        <Cover track={track} size="medium" />
      </button>
      <button className="now-track-info now-track-button truncate" title={t('player.expand')} onClick={onExpand} disabled={!onExpand}>
        <strong className="truncate">{track?.title || t('player.nothingPlaying')}</strong>
        <span className="truncate">{track ? track.artist || t('library.unknownArtist') : t('player.chooseTrack')}</span>
        {track && <span className="now-format">{track.format}{track.sampleRate ? ` · ${new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 }).format(track.sampleRate / 1000)} kHz` : ''}</span>}
      </button>
    </div>
    <div className="playback-center">
      <TransportControls snapshot={snapshot} onCommand={onCommand} />
      <SeekControl playback={snapshot.playback} hasTrack={!!track} onCommand={onCommand} variant={snapshot.settings.seekbarStyle} />
    </div>
    <div className="playerbar-tools">
      <VolumeControl playback={snapshot.playback} onCommand={onCommand} />
      <button className="icon-button playerbar-info" title={t('library.trackInfo')} aria-label={t('library.trackInfo')} disabled={!track} onClick={() => track && onInfo(track)}><Info size={17} /></button>
      <button className="icon-button playerbar-expand" title={t('player.expand')} aria-label={t('player.expand')} onClick={onExpand} disabled={!onExpand}><Maximize2 size={17} /></button>
    </div>
  </footer>;
}
