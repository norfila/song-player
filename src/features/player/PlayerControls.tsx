import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { Pause, Play, SkipBack, SkipForward, Volume1, Volume2, VolumeX } from 'lucide-react';
import type { AppSnapshot, Playback, PlayerCommand, SeekbarStyle } from '../../lib/backend/types';
import { time } from '../../lib/format';
import { consumeVolumeWheel } from '../../lib/volume-wheel';
import { RepeatControl, ShuffleControl } from './PlaybackModes';
import { WaveformSeek } from './WaveformSeek';

type Command = (value: PlayerCommand) => Promise<void>;

export function SeekControl({ playback, hasTrack, onCommand, variant = 'line' }: { playback: Playback; hasTrack: boolean; onCommand: Command; variant?: SeekbarStyle }) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<number | null>(null);
  useEffect(() => setPreview(null), [playback.trackId]);
  const [waveformFailed, setWaveformFailed] = useState<string | null>(null);
  const duration = Math.max(0, playback.duration);
  const position = Math.min(preview ?? playback.position, Math.max(1, duration));
  const commit = (value: number) => { void onCommand({ type: 'seek', position: value }); setPreview(null); };
  const waveform = variant === 'waveform' && hasTrack && waveformFailed !== playback.trackId;
  const slider = <input type="range" min={0} max={Math.max(1, duration)} step={0.1} value={position}
      aria-label={t('player.seek')} aria-valuetext={`${time(position)} / ${time(duration)}`}
      disabled={!hasTrack || !duration}
      style={{ '--range-progress': `${Math.min(100, position / Math.max(1, duration) * 100)}%` } as CSSProperties}
      onChange={event => setPreview(Number(event.target.value))}
      onPointerDown={event => event.currentTarget.setPointerCapture(event.pointerId)}
      onPointerUp={event => commit(Number(event.currentTarget.value))}
      onPointerCancel={() => setPreview(null)} onBlur={() => setPreview(null)}
      onKeyUp={event => {
        if (['ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown'].includes(event.key)) commit(Number(event.currentTarget.value));
      }} />;
  return <div className={`seek-row${waveform ? ' has-waveform' : ''}`}>
    <span>{time(position)}</span>
    {waveform ? <WaveformSeek trackId={playback.trackId} progress={position / Math.max(1, duration)}
      onUnavailable={() => setWaveformFailed(playback.trackId)}>{slider}</WaveformSeek> : slider}
    <span>{time(duration)}</span>
  </div>;
}

export function VolumeControl({ playback, onCommand }: { playback: Playback; onCommand: Command }) {
  const { t } = useTranslation();
  const container = useRef<HTMLDivElement>(null);
  const command = useRef(onCommand);
  const targetVolume = useRef(playback.volume);
  const wheelRemainder = useRef(0);
  const sending = useRef(false);
  const [preview, setPreview] = useState<number | null>(null);
  const volume = preview ?? playback.volume;

  useEffect(() => { command.current = onCommand; }, [onCommand]);
  useEffect(() => {
    if (!sending.current) targetVolume.current = playback.volume;
  }, [playback.volume, preview]);

  const setVolume = useCallback((value: number) => {
    targetVolume.current = value;
    setPreview(value);
    if (sending.current) return;
    sending.current = true;
    // Keep an optimistic target while commands are in flight, and send the latest
    // target after each acknowledgement rather than rebasing on older snapshots.
    void (async () => {
      try {
        let submitted: number;
        do {
          submitted = targetVolume.current;
          await command.current({ type: 'set_volume', volume: submitted });
        } while (submitted !== targetVolume.current);
      } finally {
        sending.current = false;
        setPreview(null);
      }
    })();
  }, []);

  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      const result = consumeVolumeWheel(targetVolume.current, wheelRemainder.current, event);
      if (!result.handled || !event.cancelable) return;
      event.preventDefault();
      wheelRemainder.current = result.remainder;
      if (result.volume !== targetVolume.current) setVolume(result.volume);
    };
    const resetWheel = () => { wheelRemainder.current = 0; };
    // React's delegated wheel listeners are passive, so register directly to keep
    // the settings/Now Playing page still when volume consumes vertical scrolling.
    element.addEventListener('wheel', onWheel, { passive: false });
    element.addEventListener('mouseleave', resetWheel);
    return () => {
      element.removeEventListener('wheel', onWheel);
      element.removeEventListener('mouseleave', resetWheel);
    };
  }, [setVolume]);

  return <div className="volume-controls" ref={container}>
    <button className="icon-button" title={t(playback.muted ? 'player.unmute' : 'player.mute')} aria-label={t(playback.muted ? 'player.unmute' : 'player.mute')} onClick={() => void onCommand({ type: 'set_muted', muted: !playback.muted })}>
      {playback.muted || !volume ? <VolumeX size={20} /> : volume < 50 ? <Volume1 size={20} /> : <Volume2 size={20} />}
    </button>
    <input type="range" min={0} max={100} value={volume} aria-label={t('player.volume')} aria-valuetext={`${Math.round(volume)}%`}
      style={{ '--range-progress': `${playback.muted ? 0 : volume}%` } as CSSProperties}
      onChange={event => { wheelRemainder.current = 0; setVolume(Number(event.target.value)); }} />
    <span className="volume-value">{Math.round(volume)}%</span>
  </div>;
}

export function TransportControls({ snapshot, onCommand, expanded = false }: { snapshot: AppSnapshot; onCommand: Command; expanded?: boolean }) {
  const { t } = useTranslation();
  const session = snapshot.sessions.find(item => item.id === snapshot.activeSessionId);
  const playing = snapshot.playback.status === 'playing';
  const disabled = !session?.entries.length;
  const repeat = session?.repeat || 'off';
  const shuffle = session?.orderMode.startsWith('shuffle') || false;
  return <div className={`transport ${expanded ? 'transport-expanded' : ''}`}>
    <ShuffleControl enabled={shuffle} disabled={disabled} onCommand={onCommand} size={expanded ? 22 : 17} />
    <button className="icon-button transport-skip" title={t('player.previous')} aria-label={t('player.previous')} disabled={disabled} onClick={() => void onCommand({ type: 'previous' })}><SkipBack size={expanded ? 25 : 20} fill="currentColor" /></button>
    <button className="play-button" title={t(playing ? 'player.pause' : 'player.play')} aria-label={t(playing ? 'player.pause' : 'player.play')} disabled={disabled} onClick={() => void onCommand({ type: 'play_pause' })}>{playing ? <Pause size={expanded ? 30 : 20} fill="currentColor" /> : <Play size={expanded ? 30 : 20} fill="currentColor" />}</button>
    <button className="icon-button transport-skip" title={t('player.next')} aria-label={t('player.next')} disabled={disabled} onClick={() => void onCommand({ type: 'next' })}><SkipForward size={expanded ? 25 : 20} fill="currentColor" /></button>
    <RepeatControl mode={repeat} disabled={disabled} onCommand={onCommand} size={expanded ? 22 : 17} />
  </div>;
}
