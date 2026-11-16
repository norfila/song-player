import { useId, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, X } from 'lucide-react';
import {
  DEFAULT_SHORTCUTS, SHORTCUT_ACTIONS, formatShortcut, resolveShortcuts,
  shortcutFromEvent, validateShortcuts,
} from '../../lib/shortcuts';
import type { ShortcutAction } from '../../lib/shortcuts';
import './shortcut-editor.css';

interface Props {
  value: Record<string, string>;
  onChange: (shortcuts: Record<string, string>) => Promise<void>;
  disabled?: boolean;
}

export function ShortcutEditor({ value, onChange, disabled = false }: Props) {
  const { t } = useTranslation();
  const descriptionId = useId();
  const errorId = useId();
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const shortcuts = resolveShortcuts(value);
  const busy = disabled || saving;

  const save = async (next: Record<string, string>) => {
    setRecording(null);
    setError(null);
    setSaving(true);
    try { await onChange(next); }
    catch { setError(t('shortcuts.saveFailed')); }
    finally { setSaving(false); }
  };

  const assign = (action: ShortcutAction, binding: string) => {
    const next = { ...shortcuts, [action]: binding };
    const invalid = validateShortcuts(next).find(problem => problem.action === action || problem.otherAction === action);
    if (invalid) {
      const otherAction = invalid.action === action ? invalid.otherAction : invalid.action;
      setError(t(`shortcuts.${invalid.code}`, { action: otherAction ? t(`shortcuts.actions.${otherAction}`) : '' }));
      return;
    }
    void save(next);
  };

  const capture = (event: KeyboardEvent<HTMLButtonElement>, action: ShortcutAction) => {
    if (recording !== action) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.repeat || busy) return;
    if (event.code === 'Escape' && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      setRecording(null);
      setError(null);
      return;
    }
    if (['Backspace', 'Delete'].includes(event.code) && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      assign(action, '');
      return;
    }
    const binding = shortcutFromEvent(event.nativeEvent);
    if (binding) assign(action, binding);
  };

  return <div className="shortcut-editor">
    <div className="shortcut-editor-intro">
      <p id={descriptionId}>{t('shortcuts.description')}<span>{t('shortcuts.recordingHint')}</span></p>
      <button className="button secondary shortcut-reset" disabled={busy || SHORTCUT_ACTIONS.every(action => shortcuts[action] === DEFAULT_SHORTCUTS[action])}
        onClick={() => void save({ ...DEFAULT_SHORTCUTS })}><RotateCcw size={14} />{t('shortcuts.reset')}</button>
    </div>
    {error && <p className="shortcut-error" role="alert" id={errorId}>{error}</p>}
    <div className="shortcut-editor-list">
      {SHORTCUT_ACTIONS.map(action => <div className={`shortcut-editor-row ${recording === action ? 'is-recording' : ''}`} key={action}>
        <label htmlFor={`${descriptionId}-${action}-capture`}>{t(`shortcuts.actions.${action}`)}</label>
        <div className="shortcut-editor-control">
          <button type="button" id={`${descriptionId}-${action}-capture`} data-shortcut-recorder
            className={`shortcut-capture ${recording === action ? 'recording' : ''}`} disabled={busy}
            aria-describedby={`${descriptionId}${error ? ` ${errorId}` : ''}`}
            aria-label={`${t(`shortcuts.actions.${action}`)}: ${recording === action ? t('shortcuts.record') : `${formatShortcut(shortcuts[action]) || t('shortcuts.disabled')}. ${t('shortcuts.change')}`}`}
            aria-pressed={recording === action}
            onKeyDown={event => capture(event, action)}
            onBlur={() => { if (recording === action) setRecording(null); }}
            onClick={() => { setError(null); setRecording(recording === action ? null : action); }}>
            {recording === action ? <span className="shortcut-recording-text" aria-live="polite">{t('shortcuts.record')}</span>
              : shortcuts[action] ? <kbd>{formatShortcut(shortcuts[action])}</kbd> : <span className="shortcut-unassigned">{t('shortcuts.disabled')}</span>}
          </button>
          <button type="button" className="icon-button shortcut-clear" disabled={busy || !shortcuts[action]}
            aria-label={`${t('shortcuts.clear')}: ${t(`shortcuts.actions.${action}`)}`}
            title={t('shortcuts.clear')} onClick={() => assign(action, '')}><X size={14} /></button>
        </div>
      </div>)}
    </div>
  </div>;
}
