import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { api, isDesktop } from '../../lib/backend';
import { waveformBars } from '../../lib/waveform';

/** Peaks are decoded once per track by FFmpeg; keep the most recent ones. */
const cache = new Map<string, Promise<number[]>>();
function loadPeaks(trackId: string): Promise<number[]> {
  let pending = cache.get(trackId);
  if (!pending) {
    pending = api.getAudioWaveform(trackId);
    pending.catch(() => cache.delete(trackId));
    cache.set(trackId, pending);
    if (cache.size > 24) cache.delete(cache.keys().next().value!);
  }
  return pending;
}

const BAR_STEP = 4;
const PLACEHOLDER = 0.18;

/** Waveform drawn behind the seek slider. The slider stays on top (transparent)
 * so keyboard, pointer and screen-reader behaviour is unchanged. Returns null
 * while unavailable so the caller can fall back to the plain line. */
export function WaveformSeek({ trackId, progress, children, onUnavailable }: {
  trackId: string | null; progress: number; children: ReactNode; onUnavailable: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [peaks, setPeaks] = useState<{ id: string; values: number[] } | null>(null);
  const unavailable = useRef(onUnavailable); unavailable.current = onUnavailable;

  useEffect(() => {
    const element = box.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!trackId) return;
    if (!isDesktop) { unavailable.current(); return; }
    let live = true;
    void loadPeaks(trackId).then(values => { if (live) setPeaks({ id: trackId, values }); }).catch(() => { if (live) unavailable.current(); });
    return () => { live = false; };
  }, [trackId]);

  const count = Math.max(12, Math.floor(width / BAR_STEP));
  const ready = !!trackId && peaks?.id === trackId;
  const bars = ready ? waveformBars(peaks.values, count) : Array.from({ length: count }, () => PLACEHOLDER);
  const layer = (className: string) => <div className={`waveform-layer ${className}`} aria-hidden="true">
    {bars.map((value, index) => <i key={index} style={{ '--bar': Math.max(0.08, value) } as CSSProperties} />)}
  </div>;
  return <div ref={box} className={`waveform-seek${ready ? '' : ' is-loading'}`}
    style={{ '--progress': `${Math.min(100, Math.max(0, progress * 100))}%` } as CSSProperties}>
    {layer('waveform-rest')}
    {layer('waveform-played')}
    <span className="waveform-playhead" aria-hidden="true" />
    {children}
  </div>;
}
