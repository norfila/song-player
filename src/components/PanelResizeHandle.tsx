import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, KeyboardEvent, PointerEvent } from 'react';
import './panel-resize.css';

interface PanelResizeHandleProps {
  value: number;
  min: number;
  max: number;
  defaultValue: number;
  label: string;
  controls?: string;
  /** Side toward which the panel grows: right for the sidebar, left for the queue. */
  direction?: 'right' | 'left';
  className?: string;
  style?: CSSProperties;
  onChange: (value: number) => void;
  /** Save the width only after the gesture, rather than for every pointer move. */
  onCommit?: (value: number) => void;
}

/** A captured pointer keeps resizing independent of queue-item dragging. */
export function PanelResizeHandle({ value, min, max, defaultValue, label, controls, direction = 'right', className = '', style, onChange, onCommit }: PanelResizeHandleProps) {
  const handle = useRef<HTMLDivElement>(null);
  const drag = useRef<{ pointerId: number; startX: number; startValue: number } | null>(null);
  const liveValue = useRef(value);
  const [dragging, setDragging] = useState(false);
  liveValue.current = value;
  const sign = direction === 'right' ? 1 : -1;
  const upperBound = Math.max(min, max);
  const clamp = (width: number) => Math.min(upperBound, Math.max(min, Math.round(width)));

  useEffect(() => {
    if (!dragging) return;
    document.documentElement.classList.add('is-panel-resizing');
    return () => document.documentElement.classList.remove('is-panel-resizing');
  }, [dragging]);

  function change(width: number) {
    const next = clamp(width);
    if (next !== liveValue.current) {
      liveValue.current = next;
      onChange(next);
    }
    return next;
  }

  function finish(cancel = false) {
    const current = drag.current;
    if (!current) return;
    drag.current = null;
    setDragging(false);
    if (handle.current?.hasPointerCapture(current.pointerId)) handle.current.releasePointerCapture(current.pointerId);
    if (cancel) change(current.startValue);
    else if (liveValue.current !== current.startValue) onCommit?.(liveValue.current);
  }

  function pointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || drag.current) return;
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { pointerId: event.pointerId, startX: event.clientX, startValue: value };
    setDragging(true);
  }

  function pointerMove(event: PointerEvent<HTMLDivElement>) {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    change(current.startValue + (event.clientX - current.startX) * sign);
  }

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape' && drag.current) {
      event.preventDefault();
      event.stopPropagation();
      finish(true);
      return;
    }
    let next: number;
    if (event.key === 'Home') next = min;
    else if (event.key === 'End') next = upperBound;
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      next = value + (event.key === 'ArrowRight' ? 1 : -1) * sign * (event.shiftKey ? 32 : 8);
    } else return;
    event.preventDefault();
    event.stopPropagation();
    const committed = change(next);
    onCommit?.(committed);
  }

  return <div
    ref={handle}
    role="separator"
    tabIndex={0}
    aria-label={label}
    aria-controls={controls}
    aria-orientation="vertical"
    aria-valuemin={min}
    aria-valuemax={upperBound}
    aria-valuenow={Math.round(value)}
    aria-valuetext={`${Math.round(value)} px`}
    title={label}
    className={`panel-resize-handle ${dragging ? 'is-dragging' : ''} ${className}`}
    style={style}
    onPointerDown={pointerDown}
    onPointerMove={pointerMove}
    onPointerUp={event => { if (drag.current?.pointerId === event.pointerId) finish(); }}
    onPointerCancel={event => { if (drag.current?.pointerId === event.pointerId) finish(true); }}
    onLostPointerCapture={() => finish()}
    onKeyDown={keyDown}
    onDoubleClick={event => { event.preventDefault(); event.stopPropagation(); const reset = change(defaultValue); onCommit?.(reset); }}
  />;
}
