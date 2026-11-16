/** Physical key codes keep saved shortcuts independent of the keyboard layout. */
export const SHORTCUT_ACTIONS = [
  'playPause', 'previous', 'next', 'seekBack', 'seekForward', 'openFiles',
  'addFolder', 'search', 'nowPlaying', 'toggleQueue', 'quit',
] as const;

export type ShortcutAction = typeof SHORTCUT_ACTIONS[number];
export type Shortcuts = Record<ShortcutAction, string>;

export const DEFAULT_SHORTCUTS: Readonly<Shortcuts> = Object.freeze({
  playPause: 'Space',
  previous: 'Ctrl+ArrowLeft',
  next: 'Ctrl+ArrowRight',
  seekBack: 'ArrowLeft',
  seekForward: 'ArrowRight',
  openFiles: 'Ctrl+KeyO',
  addFolder: 'Ctrl+Shift+KeyO',
  search: 'Ctrl+KeyF',
  nowPlaying: 'Ctrl+Shift+KeyP',
  toggleQueue: 'Ctrl+Shift+KeyQ',
  quit: 'Ctrl+KeyQ',
});

const KEY_CODE = /^(Key[A-Z]|Digit[0-9]|F(?:[1-9]|1[0-9]|2[0-4])|Arrow(?:Left|Right|Up|Down)|Space|Enter|Escape|Home|End|PageUp|PageDown|Insert|Delete|Backspace|Tab|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|IntlBackslash|Semicolon|Quote|Comma|Period|Slash|Numpad(?:[0-9]|Add|Subtract|Multiply|Divide|Decimal|Enter|Equal))$/;
const MODIFIER_CODES = /^(Control|Alt|Shift|Meta)(Left|Right)$/;
const RESERVED = new Set(['Alt+F4', 'Alt+Tab', 'Alt+Escape', 'Alt+Space', 'Ctrl+Alt+Delete', 'Ctrl+Escape', 'Ctrl+Shift+Escape']);
const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Meta'];

export interface ShortcutKeyEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  isComposing?: boolean;
  getModifierState?: (key: string) => boolean;
}

export interface ShortcutError {
  code: 'invalid' | 'reserved' | 'conflict';
  action: ShortcutAction;
  otherAction?: ShortcutAction;
}

/** Missing keys use defaults, while an explicit empty value disables an action. */
export function resolveShortcuts(saved?: Record<string, string> | null): Shortcuts {
  return Object.fromEntries(SHORTCUT_ACTIONS.map(action => [action, saved?.[action] ?? DEFAULT_SHORTCUTS[action]])) as Shortcuts;
}

export function shortcutFromEvent(event: ShortcutKeyEvent): string | null {
  if (!event.code || event.isComposing || MODIFIER_CODES.test(event.code) || event.getModifierState?.('AltGraph')) return null;
  return [event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Meta', event.code].filter(Boolean).join('+');
}

export function validateShortcut(binding: string): 'invalid' | 'reserved' | null {
  if (binding === '') return null;
  const parts = binding.split('+');
  const code = parts.pop() ?? '';
  if (parts.includes('Meta')) return 'reserved';
  if (!KEY_CODE.test(code) || parts.some(modifier => !MODIFIERS.includes(modifier))) return 'invalid';
  if ([...new Set(parts)].join('+') !== parts.join('+') || MODIFIERS.filter(modifier => parts.includes(modifier)).join('+') !== parts.join('+')) return 'invalid';
  if (RESERVED.has(binding)) return 'reserved';
  return null;
}

export function validateShortcuts(shortcuts: Record<string, string>): ShortcutError[] {
  const seen = new Map<string, ShortcutAction>();
  const errors: ShortcutError[] = [];
  for (const action of SHORTCUT_ACTIONS) {
    const binding = shortcuts[action];
    if (typeof binding !== 'string') { errors.push({ code: 'invalid', action }); continue; }
    const error = validateShortcut(binding);
    if (error) { errors.push({ code: error, action }); continue; }
    if (!binding) continue;
    const otherAction = seen.get(binding);
    if (otherAction) errors.push({ code: 'conflict', action, otherAction });
    else seen.set(binding, action);
  }
  return errors;
}

/** Matches exact modifiers so Ctrl+Shift+O never also triggers Ctrl+O. */
export function findShortcutAction(event: ShortcutKeyEvent, saved?: Record<string, string> | null): ShortcutAction | null {
  const binding = shortcutFromEvent(event);
  if (!binding || validateShortcut(binding)) return null;
  const shortcuts = resolveShortcuts(saved);
  return SHORTCUT_ACTIONS.find(action => shortcuts[action] === binding) ?? null;
}

const KEY_LABELS: Record<string, string> = {
  ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓',
  Escape: 'Esc', Backspace: 'Backspace', PageUp: 'Page Up', PageDown: 'Page Down',
  Backquote: '`', Minus: '−', Equal: '=', BracketLeft: '[', BracketRight: ']',
  Backslash: '\\', IntlBackslash: '\\', Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/',
  NumpadAdd: 'Num +', NumpadSubtract: 'Num −', NumpadMultiply: 'Num ×', NumpadDivide: 'Num ÷',
  NumpadDecimal: 'Num .', NumpadEnter: 'Num Enter', NumpadEqual: 'Num =',
};

export function formatShortcut(binding: string): string {
  return binding.split('+').map(part => KEY_LABELS[part] ?? (/^Key[A-Z]$/.test(part) ? part.slice(3) : /^Digit[0-9]$/.test(part) ? part.slice(5) : /^Numpad[0-9]$/.test(part) ? `Num ${part.slice(6)}` : part)).join(' + ');
}
