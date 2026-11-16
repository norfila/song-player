import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties, KeyboardEvent, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
import './select.css';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  label: string;
  className?: string;
  disabled?: boolean;
  id?: string;
  leadingIcon?: ReactNode;
}

/** A select-only combobox. Focus stays on the trigger while its listbox is open. */
export function Select({ value, onChange, options, label, className = '', disabled = false, id, leadingIcon }: SelectProps) {
  const generatedId = useId();
  const listId = `${generatedId}-list`;
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const search = useRef({ text: '', timestamp: 0 });
  const selected = options.findIndex(option => option.value === value);
  const enabled = options.map((option, index) => option.disabled ? -1 : index).filter(index => index >= 0);
  const unavailable = disabled || enabled.length === 0;
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(selected);
  const [position, setPosition] = useState<CSSProperties>({ position: 'fixed', visibility: 'hidden' });

  function show(index = selected) {
    if (unavailable) return;
    search.current = { text: '', timestamp: 0 };
    setActive(enabled.includes(index) ? index : enabled[0]);
    setOpen(true);
  }

  function choose(index: number) {
    const option = options[index];
    if (!option || option.disabled) return;
    setOpen(false);
    search.current = { text: '', timestamp: 0 };
    if (option.value !== value) onChange(option.value);
    trigger.current?.focus({ preventScroll: true });
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (unavailable) return;
    const { key } = event;
    if (key === 'Tab') {
      setOpen(false);
      return;
    }
    if (key === 'Escape') {
      if (open) {
        event.preventDefault();
        event.stopPropagation();
        setOpen(false);
      }
      return;
    }
    if (key === 'ArrowDown' || key === 'ArrowUp' || key === 'Home' || key === 'End') {
      event.preventDefault();
      event.stopPropagation();
      if (event.altKey && key === 'ArrowUp' && open) {
        choose(active);
        return;
      }
      if (!open) {
        show(key === 'Home' ? enabled[0] : key === 'End' ? enabled[enabled.length - 1] : selected);
        return;
      }
      const current = enabled.indexOf(active);
      const next = key === 'Home' ? 0 : key === 'End' ? enabled.length - 1
        : (current + (key === 'ArrowDown' ? 1 : -1) + enabled.length) % enabled.length;
      setActive(enabled[next]);
      return;
    }
    if (key === 'Enter' || key === ' ') {
      event.preventDefault();
      event.stopPropagation();
      if (open) choose(active);
      else show();
      return;
    }
    if (key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      event.stopPropagation();
      const now = Date.now();
      const prior = now - search.current.timestamp < 700 ? search.current.text : '';
      const text = `${prior}${key.toLocaleLowerCase()}`;
      search.current = { text, timestamp: now };
      // Repeating one letter cycles through every matching option.
      const query = [...text].every(letter => letter === text[0]) ? text[0] : text;
      const current = open ? active : selected;
      const start = query.length === 1 ? current + 1 : Math.max(current, 0);
      for (let offset = 0; offset < options.length; offset += 1) {
        const index = (start + offset + options.length) % options.length;
        if (!options[index].disabled && options[index].label.trim().toLocaleLowerCase().startsWith(query)) {
          setActive(index);
          setOpen(true);
          break;
        }
      }
    }
  }

  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      if (!trigger.current) return;
      const rect = trigger.current.getBoundingClientRect();
      const margin = 8;
      const gap = 6;
      const viewportWidth = document.documentElement.clientWidth;
      const viewportHeight = document.documentElement.clientHeight;
      const below = viewportHeight - rect.bottom - gap - margin;
      const above = rect.top - gap - margin;
      const goesUp = below < 180 && above > below;
      const available = Math.max(0, goesUp ? above : below);
      const width = Math.min(Math.max(rect.width, 180), viewportWidth - margin * 2);
      setPosition({
        position: 'fixed',
        left: Math.max(margin, Math.min(rect.left, viewportWidth - width - margin)),
        width,
        maxHeight: Math.min(340, available),
        ...(goesUp ? { bottom: viewportHeight - rect.top + gap } : { top: rect.bottom + gap }),
      });
    }
    place();
    const observer = new ResizeObserver(place);
    if (trigger.current) observer.observe(trigger.current);
    function onScroll(event: Event) {
      if (event.target instanceof Node && list.current?.contains(event.target)) return;
      place();
    }
    window.addEventListener('resize', place);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open, options]);

  useLayoutEffect(() => {
    if (!open) return;
    list.current?.querySelector<HTMLElement>(`[data-option-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [open, active]);

  useEffect(() => {
    if (!open) return;
    function dismiss(event: Event) {
      if (event.target instanceof Node && !trigger.current?.contains(event.target) && !list.current?.contains(event.target)) setOpen(false);
    }
    document.addEventListener('pointerdown', dismiss, true);
    document.addEventListener('focusin', dismiss, true);
    return () => {
      document.removeEventListener('pointerdown', dismiss, true);
      document.removeEventListener('focusin', dismiss, true);
    };
  }, [open]);

  // A device refresh or queue deletion can invalidate the current popup.
  useEffect(() => {
    if (unavailable) setOpen(false);
    if (open && !enabled.includes(active)) setActive(enabled.includes(selected) ? selected : enabled[0] ?? -1);
  }, [unavailable, open, options, active, selected]);

  return <>
    <button
      ref={trigger}
      id={id}
      type="button"
      role="combobox"
      aria-label={label}
      aria-expanded={open}
      aria-haspopup="listbox"
      aria-controls={open ? listId : undefined}
      aria-activedescendant={open && active >= 0 ? `${listId}-${active}` : undefined}
      className={`app-select ${open ? 'is-open' : ''} ${className}`}
      disabled={unavailable}
      onClick={() => open ? setOpen(false) : show()}
      onKeyDown={onKeyDown}
    >
      {leadingIcon && <span className="app-select-icon" aria-hidden="true">{leadingIcon}</span>}
      <span className="app-select-value">{options[selected]?.label || label}</span>
      <ChevronDown className="app-select-chevron" size={14} aria-hidden="true" />
    </button>
    {open && createPortal(<div
      ref={list}
      id={listId}
      role="listbox"
      aria-label={label}
      className="app-select-list"
      style={position}
      onPointerDown={event => event.preventDefault()}
    >
      {options.map((option, index) => <div
        key={option.value}
        id={`${listId}-${index}`}
        role="option"
        aria-selected={option.value === value}
        aria-disabled={option.disabled || undefined}
        data-option-index={index}
        className={`app-select-option ${index === active ? 'is-active' : ''} ${option.value === value ? 'is-selected' : ''}`}
        onPointerMove={() => { if (!option.disabled && active !== index) setActive(index); }}
        onClick={() => choose(index)}
      ><span>{option.label}</span><Check size={14} className="app-select-check" aria-hidden="true" /></div>)}
    </div>, document.body)}
  </>;
}
