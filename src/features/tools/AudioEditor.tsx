import { useEffect, useRef, useState } from 'react';
import type { PointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, LoaderCircle, Play, RotateCcw, Square, Undo2 } from 'lucide-react';
import { api, assetUrl, isDesktop } from '../../lib/backend';
import type { Track } from '../../lib/backend/types';
import { Select } from '../../components/Select';
import { ToolDialog } from './ToolDialog';
import { displayTime } from './tool-utils';
import { AudioJobProgress, useAudioJob } from './useAudioJob';
import './tools.css';

interface Selection { start: number; end: number; remove: boolean }

export function AudioEditor({ track, onClose }: { track: Track; onClose: () => void }) {
  const { t } = useTranslation();
  const duration = Number.isFinite(track.duration) ? Math.max(0, track.duration) : 0;
  const [selection, setSelection] = useState<Selection>({ start: 0, end: duration, remove: false });
  const latest = useRef(selection);
  const [history, setHistory] = useState<Selection[]>([]);
  const gesture = useRef<Selection | null>(null);
  const drag = useRef<'start' | 'end' | null>(null);
  const [waveform, setWaveform] = useState<number[]>([]);
  const [waveformBusy, setWaveformBusy] = useState(isDesktop);
  const [waveformError, setWaveformError] = useState(false);
  const [format, setFormat] = useState<'flac' | 'wav'>('flac');
  const [previewBusy, setPreviewBusy] = useState(false);
  const [previewPlaying, setPreviewPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const audio = useRef<HTMLAudioElement | null>(null);
  const previewRequest = useRef(0);
  const mounted = useRef(true);
  const { job, setJob } = useAudioJob();
  const selectedLength = selection.end - selection.start;
  const outputLength = selection.remove ? duration - selectedLength : selectedLength;
  const valid = duration > 0 && selection.start >= 0 && selection.end <= duration && selectedLength > 0 && outputLength > .001;
  const busy = exporting || job?.status === 'running';

  useEffect(() => {
    let live = true;
    if (isDesktop) void api.getAudioWaveform(track.id).then(values => { if (live) setWaveform(values.slice(0, 1000).map(value => Math.max(0, Math.min(1, Number.isFinite(value) ? Math.abs(value) : 0)))); }).catch(() => { if (live) setWaveformError(true); }).finally(() => { if (live) setWaveformBusy(false); });
    return () => { live = false; };
  }, [track.id]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; previewRequest.current++; audio.current?.pause(); if (audio.current) audio.current.src = ''; }; }, []);
  function stopPreview() {
    previewRequest.current++;
    audio.current?.pause();
    audio.current = null;
    setPreviewBusy(false); setPreviewPlaying(false);
  }
  function update(value: Selection, remember = false) {
    if (JSON.stringify(value) === JSON.stringify(latest.current)) return;
    const before = latest.current;
    if (remember) setHistory(current => [...current.slice(-39), before]);
    latest.current = value; setSelection(value); stopPreview();
  }
  function beginGesture() { gesture.current = { ...latest.current }; }
  function endGesture() {
    const before = gesture.current; gesture.current = null;
    if (before && JSON.stringify(before) !== JSON.stringify(latest.current)) setHistory(current => [...current.slice(-39), before]);
  }
  function changeBoundary(which: 'start' | 'end', value: number) {
    if (!Number.isFinite(value)) return;
    const gap = Math.min(.01, duration / 2);
    update({ ...latest.current, [which]: which === 'start' ? Math.max(0, Math.min(latest.current.end - gap, value)) : Math.min(duration, Math.max(latest.current.start + gap, value)) });
  }
  function pointerValue(event: PointerEvent<SVGSVGElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return Math.max(0, Math.min(duration, (event.clientX - rect.left) / rect.width * duration));
  }
  async function preview() {
    stopPreview();
    const request = ++previewRequest.current;
    setPreviewBusy(true); setError(null);
    try {
      const state = await api.getSnapshot();
      if (!mounted.current || request !== previewRequest.current) return;
      if (state.playback.status === 'playing') await api.dispatch({ type: 'pause' });
      if (!mounted.current || request !== previewRequest.current) return;
      const path = await api.getAudioPreview({ trackId: track.id, ...selection });
      if (!mounted.current || request !== previewRequest.current) return;
      const player = new Audio(assetUrl(path));
      player.volume = Math.max(0, Math.min(1, state.playback.volume / 100));
      player.muted = state.playback.muted;
      audio.current = player;
      player.onended = () => { if (mounted.current && audio.current === player) { setPreviewPlaying(false); audio.current = null; } };
      player.onerror = () => { if (mounted.current && audio.current === player) { setError('features.editor.previewFailed'); stopPreview(); } };
      await player.play();
      if (!mounted.current || request !== previewRequest.current) { player.pause(); return; }
      setPreviewPlaying(true);
    } catch { if (mounted.current && request === previewRequest.current) setError('features.editor.previewFailed'); }
    finally { if (mounted.current && request === previewRequest.current) setPreviewBusy(false); }
  }
  async function exportCopy() {
    setError(null); setExporting(true); stopPreview();
    try {
      const name = (track.title || 'Audio').replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').slice(0, 120);
      const outputPath = await api.saveAudioExportPath(`${name}-edited.${format}`, format);
      if (!outputPath) return;
      const started = await api.startAudioExport({ trackId: track.id, ...selection, outputPath });
      setJob(started);
    } catch { setError('features.editor.exportFailed'); }
    finally { if (mounted.current) setExporting(false); }
  }
  const x = (seconds: number) => duration > 0 ? seconds / duration * 1000 : 0;
  return <ToolDialog title={t('features.editor.title')} onClose={() => { stopPreview(); onClose(); }} className="audio-editor-dialog">
    <div className="audio-editor-track"><strong>{track.title}</strong><span>{track.artist || t('library.unknownArtist')} · {displayTime(duration)}</span></div>
    <p className="tool-description">{t('features.editor.description')}</p>
    {waveformBusy ? <div className="audio-waveform-loading" role="status"><LoaderCircle size={21} className="spin" />{t('features.editor.waveformLoading')}</div> : <div className="audio-waveform-container">
      <svg className="audio-waveform" viewBox="0 0 1000 160" preserveAspectRatio="none" role="img" aria-label={t('features.editor.waveformLabel', { start: displayTime(selection.start), end: displayTime(selection.end) })} onPointerDown={event => {
        if (!duration || busy) return;
        beginGesture(); const value = pointerValue(event);
        drag.current = Math.abs(value - latest.current.start) <= Math.abs(value - latest.current.end) ? 'start' : 'end';
        event.currentTarget.setPointerCapture(event.pointerId); changeBoundary(drag.current, value);
      }} onPointerMove={event => { if (drag.current) changeBoundary(drag.current, pointerValue(event)); }} onPointerUp={event => { if (!drag.current) return; drag.current = null; endGesture(); if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }} onPointerCancel={() => { drag.current = null; endGesture(); }}>
        <line x1="0" y1="80" x2="1000" y2="80" className="waveform-axis" />
        {waveform.map((value, index) => <line key={index} x1={(index + .5) / waveform.length * 1000} x2={(index + .5) / waveform.length * 1000} y1={80 - Math.max(1, value * 67)} y2={80 + Math.max(1, value * 67)} className="waveform-peak" />)}
        <rect x={x(selection.start)} y="0" width={Math.max(0, x(selection.end - selection.start))} height="160" className={`waveform-selection ${selection.remove ? 'is-removed' : ''}`} />
        <line x1={x(selection.start)} x2={x(selection.start)} y1="0" y2="160" className="waveform-handle" /><line x1={x(selection.end)} x2={x(selection.end)} y1="0" y2="160" className="waveform-handle" />
      </svg><div className="waveform-times"><span>0:00</span><span>{displayTime(duration)}</span></div>
    </div>}
    {(waveformError || !isDesktop) && <p className="tool-hint">{t('features.editor.waveformUnavailable')}</p>}
    {!duration && <p className="tool-error" role="alert">{t('features.editor.noDuration')}</p>}
    <div className="audio-editor-boundaries">{(['start', 'end'] as const).map(which => <label key={which} className="tool-range-field"><span>{t(`features.editor.${which}`)}<input type="number" aria-label={t(`features.editor.${which}Seconds`)} min={0} max={duration} step={.01} value={Math.round(selection[which] * 100) / 100} disabled={!duration || busy} onFocus={beginGesture} onBlur={endGesture} onChange={event => changeBoundary(which, Number(event.target.value))} /></span><input type="range" aria-label={t(`features.editor.${which}`)} min={0} max={duration || 1} step={.01} value={selection[which]} disabled={!duration || busy} onPointerDown={beginGesture} onPointerUp={endGesture} onPointerCancel={endGesture} onKeyDown={event => { if (!event.repeat) beginGesture(); }} onKeyUp={endGesture} onChange={event => changeBoundary(which, Number(event.target.value))} /></label>)}</div>
    <fieldset className="audio-editor-operation"><legend>{t('features.editor.operation')}</legend>{[false, true].map(remove => <label className="tool-checkbox" key={String(remove)}><input type="radio" name="audio-editor-operation" checked={selection.remove === remove} disabled={busy} onChange={() => update({ ...latest.current, remove }, true)} /><span>{t(remove ? 'features.editor.removeSelection' : 'features.editor.keepSelection')}</span></label>)}</fieldset>
    <div className="audio-editor-summary"><p>{t('features.editor.outputDuration', { duration: displayTime(outputLength) })}</p><div className="tool-actions"><button className="button quiet" disabled={!history.length || busy} onClick={() => { const previous = history[history.length - 1]; setHistory(history.slice(0, -1)); update(previous); }}><Undo2 size={15} />{t('features.common.undo')}</button><button className="button quiet" disabled={busy} onClick={() => update({ start: 0, end: duration, remove: false }, true)}><RotateCcw size={15} />{t('features.common.reset')}</button></div></div>
    {error && <p className="tool-error" role="alert">{t(error)}</p>}
    <div className="tool-actions audio-editor-preview"><button className="button secondary" disabled={!isDesktop || !valid || busy || previewBusy} onClick={() => { if (previewPlaying) stopPreview(); else void preview(); }}>{previewBusy ? <LoaderCircle size={16} className="spin" /> : previewPlaying ? <Square size={15} /> : <Play size={15} />}{t(previewPlaying ? 'features.editor.stopPreview' : previewBusy ? 'features.editor.preparingPreview' : 'features.editor.preview')}</button></div>
    <p className="tool-hint">{t('features.editor.previewHint')}</p>
    <div className="audio-editor-export"><label>{t('features.editor.format')}<Select label={t('features.editor.format')} value={format} options={[{ value: 'flac', label: 'FLAC' }, { value: 'wav', label: 'WAV' }]} disabled={busy} onChange={value => setFormat(value as 'flac' | 'wav')} /></label><button className="button primary" disabled={!isDesktop || !valid || busy || previewBusy} onClick={() => void exportCopy()}><Download size={16} />{t('features.editor.exportCopy')}</button></div>
    <AudioJobProgress job={job} onCancel={() => void api.cancelAudioJob().catch(() => setError('features.common.failed'))} />
    {job?.status === 'running' && job.kind !== 'export' && <p className="tool-hint">{t('features.jobs.otherRunning')}</p>}
  </ToolDialog>;
}
