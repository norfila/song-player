import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { KeyboardEvent, PointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowDownToLine, AudioLines, Ellipsis, GripVertical, Headphones, ListMusic, ListPlus, Play, Plus, SlidersHorizontal, Trash2, X } from 'lucide-react';
import { api } from '../../lib/backend';
import type { AppSnapshot, PlayerCommand, PlayOrder } from '../../lib/backend/types';
import { Cover } from '../../components/Cover';
import { Select } from '../../components/Select';
import { SleepTimerControl } from '../tools';
import { time } from '../../lib/format';

const queueSortOptions: { value: PlayOrder; key: string }[] = [
  { value: 'manual', key: 'queue.manual' },
  { value: 'title', key: 'library.sortOptions.titleAZ' },
  { value: 'artist', key: 'library.sortOptions.artistAZ' },
  { value: 'album', key: 'library.sortOptions.albumAZ' },
  { value: 'filename', key: 'library.sortOptions.filenameAZ' },
  { value: 'year', key: 'library.sortOptions.yearOldest' },
  { value: 'duration', key: 'library.sortOptions.durationShortest' },
  { value: 'added', key: 'library.sortOptions.addedOldest' },
];
const ROW_HEIGHT = 55;

interface Props {
  snapshot: AppSnapshot;
  onTimerCommand?: (value: PlayerCommand) => Promise<void>;
  onCommand: (value: PlayerCommand) => Promise<void>;
  onRun: (operation: () => Promise<AppSnapshot | null | void>) => Promise<void>;
  onNewPlaylist: () => void;
  onNewSession: () => void;
  onClose: () => void;
}

export function QueuePanel({ snapshot, onTimerCommand, onCommand: command, onRun: run, onNewPlaylist, onNewSession, onClose }: Props) {
  const { t } = useTranslation();
  const [queueMenu, setQueueMenu] = useState(false);
  const [draggedEntry, setDraggedEntry] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [viewport, setViewport] = useState({ start: 0, end: 40 });
  const scrollRef = useRef<HTMLDivElement>(null);
  const pointer = useRef<{ entryId: string; x: number; y: number; active: boolean; before: string | null } | null>(null);
  const session = snapshot.sessions.find(item => item.id === snapshot.activeSessionId);
  const entries = session?.entries || [];
  const playback = snapshot.playback;
  const playing = playback.status === 'playing';
  const virtual = entries.length > 100;
  const visibleRows = virtual ? entries.slice(viewport.start, viewport.end).map((entry, index) => ({ entry, index: index + viewport.start })) : entries.map((entry, index) => ({ entry, index }));
  // Keep the captured pointer's element mounted while scrolling a long queue.
  if (virtual && draggedEntry) {
    const index = entries.findIndex(entry => entry.id === draggedEntry);
    if (index >= 0 && (index < viewport.start || index >= viewport.end)) visibleRows.push({ entry: entries[index], index });
  }
  useEffect(() => {
    const close = (event: globalThis.KeyboardEvent) => { if (event.key === 'Escape') setQueueMenu(false); };
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, []);

  useLayoutEffect(() => {
    const pane = scrollRef.current;
    if (!pane || !virtual) return;
    let frame = 0;
    const measure = () => {
      const start = Math.max(0, Math.floor(pane.scrollTop / ROW_HEIGHT) - 6);
      const end = Math.min(entries.length, Math.ceil((pane.scrollTop + pane.clientHeight) / ROW_HEIGHT) + 6);
      setViewport(current => current.start === start && current.end === end ? current : { start, end });
    };
    const scroll = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(measure); };
    const observer = new ResizeObserver(measure);
    observer.observe(pane);
    pane.addEventListener('scroll', scroll, { passive: true });
    measure();
    return () => { cancelAnimationFrame(frame); observer.disconnect(); pane.removeEventListener('scroll', scroll); };
  }, [virtual, entries.length, session?.id]);

  const startDrag = (event: PointerEvent<HTMLButtonElement>, entryId: string) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    pointer.current = { entryId, x: event.clientX, y: event.clientY, active: false, before: entryId };
  };
  const moveDrag = (event: PointerEvent<HTMLButtonElement>) => {
    const drag = pointer.current;
    const pane = scrollRef.current;
    if (!drag || !pane) return;
    if (!drag.active && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 5) return;
    if (!drag.active) { drag.active = true; setDraggedEntry(drag.entryId); }
    const rect = pane.getBoundingClientRect();
    if (event.clientY < rect.top + 30) pane.scrollTop -= 12;
    if (event.clientY > rect.bottom - 30) pane.scrollTop += 12;
    const y = event.clientY - rect.top + pane.scrollTop - 11;
    const boundary = Math.max(0, Math.min(entries.length, Math.round(y / ROW_HEIGHT)));
    drag.before = entries[boundary]?.id || null;
    setDropTarget(drag.before || 'end');
  };
  const finishDrag = (event: PointerEvent<HTMLButtonElement>, cancel = false) => {
    const drag = pointer.current;
    pointer.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    if (drag?.active && !cancel && drag.before !== drag.entryId) void command({ type: 'move_entry', entryId: drag.entryId, beforeEntryId: drag.before });
    setDraggedEntry(null);
    setDropTarget(null);
  };
  const moveWithKeyboard = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    event.preventDefault();
    if (event.key === 'ArrowUp' && index > 0) void command({ type: 'move_entry', entryId: entries[index].id, beforeEntryId: entries[index - 1].id });
    if (event.key === 'ArrowDown' && index < entries.length - 1) void command({ type: 'move_entry', entryId: entries[index].id, beforeEntryId: entries[index + 2]?.id || null });
  };

  return <aside className="queue-pane" id="active-queue">
    <div className="queue-header">
      <h2>{t('queue.title')}</h2>
      <span className="queue-count">{t('library.tracks', { count: entries.length })}</span>
      <div className="menu-wrap">
        <button className="icon-button" title={t('app.more')} aria-label={t('app.more')} aria-expanded={queueMenu} onClick={() => setQueueMenu(!queueMenu)}><Ellipsis size={20} /></button>
        {queueMenu && <>
          <button className="menu-dismiss" aria-label={t('app.close')} onClick={() => setQueueMenu(false)} />
          <div className="context-menu queue-context">
            <SleepTimerControl snapshot={snapshot} onCommand={onTimerCommand || command} />
            <button onClick={() => { setQueueMenu(false); onNewPlaylist(); }}><ListPlus size={16} />{t('queue.save')}</button>
            <button onClick={() => { setQueueMenu(false); void run(() => api.exportPlaylist()); }}><ArrowDownToLine size={16} />{t('queue.export')}</button>
            <button onClick={() => { setQueueMenu(false); onNewSession(); }}><Plus size={16} />{t('session.new')}</button>
            <div className="queue-sort-action"><Select leadingIcon={<SlidersHorizontal size={15} />} label={t('queue.sort')} disabled={!entries.length} value={session?.orderMode || 'manual'} options={queueSortOptions.map(option => ({ value: option.value, label: t(option.key) }))} onChange={value => { setQueueMenu(false); void command({ type: 'set_order', mode: value as PlayOrder }); }} /></div>
            <div className="menu-divider" />
            <button className="danger" disabled={!entries.length} onClick={() => { setQueueMenu(false); void command({ type: 'clear_queue' }); }}><Trash2 size={15} />{t('queue.clear')}</button>
          </div>
        </>}
      </div>
      <button className="icon-button small queue-close" title={t('app.hideQueue')} onClick={onClose}><X size={16} /></button>
    </div>
    <Select className="session-selector" leadingIcon={<Headphones size={15} />} label={t('queue.session')} value={snapshot.activeSessionId} onChange={value => void command({ type: 'activate_session', sessionId: value })} options={snapshot.sessions.map(item => ({ value: item.id, label: item.name || t('session.defaultName') }))} />
    {snapshot.sleepTimer && <SleepTimerControl snapshot={snapshot} onCommand={onTimerCommand || command} compact />}
    <div className="queue-scroll" ref={scrollRef}>
      {entries.length > 0 ? <div className="queue-tracks" style={virtual ? { position: 'relative', height: entries.length * ROW_HEIGHT + 46, flexShrink: 0 } : undefined}>
        {visibleRows.map(({ entry, index }) => {
          return <div className={`queue-track ${entry.id === playback.currentEntryId ? 'current' : ''} ${dropTarget === entry.id ? 'drop-before' : ''} ${draggedEntry === entry.id ? 'dragging' : ''}`} key={entry.id} style={virtual ? { position: 'absolute', top: index * ROW_HEIGHT + 11, left: 0, right: 0, height: ROW_HEIGHT } : undefined} onDoubleClick={() => void command({ type: 'play_entry', entryId: entry.id })}>
            <span className="queue-track-index">{entry.id === playback.currentEntryId && playing ? <AudioLines size={14} /> : index + 1}</span>
            <button className="icon-button small queue-drag-handle" title={t('queue.drag')} aria-label={`${t('queue.drag')}: ${entry.track.title}`} onPointerDown={event => startDrag(event, entry.id)} onPointerMove={moveDrag} onPointerUp={event => finishDrag(event)} onPointerCancel={event => finishDrag(event, true)} onLostPointerCapture={() => { if (pointer.current) { pointer.current = null; setDraggedEntry(null); setDropTarget(null); } }} onKeyDown={event => moveWithKeyboard(event, index)}><GripVertical size={14} /></button>
            <button className="queue-cover-button" title={t('player.play')} onClick={() => void command({ type: 'play_entry', entryId: entry.id })}><Cover track={entry.track} /><span className="cover-play"><Play size={13} fill="currentColor" /></span></button>
            <div className="queue-track-info truncate"><strong className="truncate">{entry.track.title}</strong><span className="truncate">{entry.track.artist || t('library.unknownArtist')}</span></div>
            <span className="queue-track-time">{time(entry.track.duration)}</span>
            <button className="icon-button small queue-remove" title={t('queue.remove')} onClick={() => void command({ type: 'remove_entry', entryId: entry.id })}><X size={14} /></button>
          </div>;
        })}
        <div className={`queue-end-drop ${dropTarget === 'end' ? 'drop-before' : ''}`} style={virtual ? { position: 'absolute', top: entries.length * ROW_HEIGHT + 11, left: 0, right: 0 } : undefined} />
      </div> : <div className="queue-empty"><div className="queue-empty-art"><ListMusic size={29} strokeWidth={1.3} /><span className="queue-art-dot" /></div><h3>{t('queue.emptyTitle')}</h3><p>{t('queue.emptyDescription')}</p></div>}
    </div>
    <div className="queue-footer"><button className="button secondary" disabled={!entries.length} onClick={onNewPlaylist}><Plus size={15} />{t('queue.save')}</button></div>
  </aside>;
}
