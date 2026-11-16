import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties, KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Shuffle } from 'lucide-react';
import type { PlayerCommand, RepeatMode } from '../../lib/backend/types';
import { RepeatModeIcon } from './RepeatModeIcon';
import './playback-modes.css';

type Command = (value: PlayerCommand) => Promise<void>;
interface ModeControlProps {
  onCommand: Command;
  disabled?: boolean;
  size?: number;
}

export function ShuffleControl({ enabled, onCommand, disabled = false, size = 17 }: ModeControlProps & { enabled: boolean }) {
  const { t } = useTranslation();
  const label = t(enabled ? 'player.shuffleOn' : 'player.shuffleOff');
  return <button
    type="button"
    className="playback-mode-button icon-button transport-mode"
    title={label}
    aria-label={label}
    aria-pressed={enabled}
    disabled={disabled}
    onClick={() => void onCommand({ type: 'set_shuffle', enabled: !enabled })}
  >
    {enabled ? <Shuffle size={size} strokeWidth={1.8} aria-hidden="true" /> : <svg
      width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"
    >
      <path d="M3 7h18M17 3l4 4-4 4" />
      <path d="M3 17h18M17 13l4 4-4 4" />
    </svg>}
  </button>;
}

const modes: RepeatMode[] = ['off', 'all', 'one'];
const repeatKeys = { off: 'player.repeatOff', all: 'player.repeatAll', one: 'player.repeatOne' } as const;

/** One shared repeat menu for the mini player and Now Playing page. */
export function RepeatControl({ mode, onCommand, disabled = false, size = 17 }: ModeControlProps & { mode: RepeatMode }) {
  const { t } = useTranslation();
  const menuId = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const focusOnOpen = useRef(true);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<CSSProperties>({ position: 'fixed', visibility: 'hidden' });
  const stateLabel = t(repeatKeys[mode]);

  function close(restoreFocus = false) {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus({ preventScroll: true });
  }

  function choose(value: RepeatMode) {
    close(true);
    if (value !== mode) void onCommand({ type: 'set_repeat', mode: value });
  }

  function onMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const current = items.current.indexOf(document.activeElement as HTMLButtonElement);
    let next: number;
    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        close(true);
        return;
      case 'Tab':
        // Resume the normal tab order from the trigger, rather than from the portal.
        close(true);
        return;
      case 'ArrowDown': next = (current + 1) % modes.length; break;
      case 'ArrowUp': next = (current - 1 + modes.length) % modes.length; break;
      case 'Home': next = 0; break;
      case 'End': next = modes.length - 1; break;
      default: return;
    }
    event.preventDefault();
    event.stopPropagation();
    items.current[next]?.focus({ preventScroll: true });
  }

  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      if (!trigger.current || !menu.current) return;
      const rect = trigger.current.getBoundingClientRect();
      const margin = 8;
      const gap = 7;
      const viewportWidth = document.documentElement.clientWidth;
      const viewportHeight = document.documentElement.clientHeight;
      const below = Math.max(0, viewportHeight - rect.bottom - gap - margin);
      const above = Math.max(0, rect.top - gap - margin);
      const menuHeight = menu.current.scrollHeight;
      const goesUp = below < menuHeight && above > below;
      const width = Math.min(Math.max(rect.width, 278), viewportWidth - margin * 2);
      setPosition({
        position: 'fixed',
        left: Math.max(margin, Math.min(rect.left, viewportWidth - width - margin)),
        width,
        maxHeight: Math.min(320, goesUp ? above : below),
        ...(goesUp ? { bottom: viewportHeight - rect.top + gap } : { top: rect.bottom + gap }),
      });
    }
    place();
    const observer = new ResizeObserver(place);
    if (trigger.current) observer.observe(trigger.current);
    function onScroll(event: Event) {
      if (event.target instanceof Node && menu.current?.contains(event.target)) return;
      place();
    }
    window.addEventListener('resize', place);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open, t]);

  useLayoutEffect(() => {
    if (!open) {
      focusOnOpen.current = true;
      return;
    }
    // Initial placement starts hidden. Wait for that state update before focusing;
    // browsers cannot focus an item while its portal is visibility:hidden.
    if (focusOnOpen.current && position.visibility !== 'hidden') {
      items.current[modes.indexOf(mode)]?.focus({ preventScroll: true });
      focusOnOpen.current = false;
    }
  }, [open, mode, position]);

  useEffect(() => {
    if (!open) return;
    function dismiss(event: Event) {
      if (event.target instanceof Node && !trigger.current?.contains(event.target) && !menu.current?.contains(event.target)) setOpen(false);
    }
    document.addEventListener('pointerdown', dismiss, true);
    document.addEventListener('focusin', dismiss, true);
    return () => {
      document.removeEventListener('pointerdown', dismiss, true);
      document.removeEventListener('focusin', dismiss, true);
    };
  }, [open]);

  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);

  return <>
    <button
      ref={trigger}
      type="button"
      className={`playback-mode-button icon-button transport-mode ${mode !== 'off' ? 'is-on' : ''} ${open ? 'is-open' : ''}`}
      title={stateLabel}
      aria-label={`${t('player.repeat')}: ${stateLabel}`}
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? menuId : undefined}
      disabled={disabled}
      onClick={() => setOpen(value => !value)}
      onKeyDown={event => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault();
          event.stopPropagation();
          setOpen(true);
        }
      }}
    >
      <RepeatModeIcon mode={mode} size={size} />
    </button>
    {open && createPortal(<div
      ref={menu}
      id={menuId}
      role="menu"
      aria-label={t('player.repeat')}
      className="playback-repeat-menu"
      style={position}
      onKeyDown={onMenuKeyDown}
    >
      {modes.map((value, index) => {
        return <button
          ref={element => { items.current[index] = element; }}
          key={value}
          type="button"
          role="menuitemradio"
          aria-checked={value === mode}
          tabIndex={-1}
          className="playback-repeat-option"
          onClick={() => choose(value)}
        >
          <RepeatModeIcon mode={value} size={18} />
          <span>{t(repeatKeys[value])}</span>
          <span className="playback-repeat-radio" aria-hidden="true" />
        </button>;
      })}
    </div>, document.body)}
  </>;
}
