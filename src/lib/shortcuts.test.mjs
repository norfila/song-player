import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DEFAULT_SHORTCUTS, SHORTCUT_ACTIONS, findShortcutAction, formatShortcut, resolveShortcuts, shortcutFromEvent, validateShortcut, validateShortcuts } from './shortcuts.ts';

const key = (code, modifiers = {}) => ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...modifiers });

test('all defaults are complete, unique and valid', () => {
  assert.deepEqual(Object.keys(DEFAULT_SHORTCUTS), SHORTCUT_ACTIONS);
  assert.deepEqual(validateShortcuts(DEFAULT_SHORTCUTS), []);
  assert.equal(findShortcutAction(key('KeyP', { ctrlKey: true, shiftKey: true })), 'nowPlaying');
});

test('saved preferences preserve disabled bindings and migrate newly introduced actions', () => {
  const saved = { openFiles: '', playPause: 'Ctrl+Space', oldAction: 'KeyX' };
  const resolved = resolveShortcuts(saved);
  assert.equal(resolved.openFiles, '');
  assert.equal(resolved.playPause, 'Ctrl+Space');
  assert.equal(resolved.nowPlaying, DEFAULT_SHORTCUTS.nowPlaying);
  assert.equal('oldAction' in resolved, false);
  assert.equal(findShortcutAction(key('KeyO', { ctrlKey: true }), saved), null);
  assert.equal(saved.oldAction, 'KeyX');
});

test('matching uses physical codes and exact modifier combinations', () => {
  assert.equal(findShortcutAction({ ...key('KeyO', { ctrlKey: true }), key: 'ö' }), 'openFiles');
  assert.equal(findShortcutAction(key('KeyO', { ctrlKey: true, shiftKey: true })), 'addFolder');
  assert.equal(findShortcutAction(key('KeyO', { ctrlKey: true, altKey: true })), null);
  assert.equal(findShortcutAction(key('ArrowLeft')), 'seekBack');
  assert.equal(findShortcutAction(key('ArrowLeft', { ctrlKey: true })), 'previous');
});

test('composition, AltGraph and standalone modifiers never become shortcuts', () => {
  assert.equal(shortcutFromEvent(key('ControlLeft', { ctrlKey: true })), null);
  assert.equal(shortcutFromEvent(key('KeyQ', { ctrlKey: true, altKey: true, getModifierState: state => state === 'AltGraph' })), null);
  assert.equal(shortcutFromEvent(key('KeyA', { isComposing: true })), null);
  assert.equal(findShortcutAction(key('Space', { isComposing: true })), null);
});

test('canonical validation rejects malformed, duplicate and out-of-order modifiers', () => {
  for (const binding of ['Shift+Ctrl+KeyA', 'Ctrl+Ctrl+KeyA', 'Control+KeyA', 'Ctrl+', 'Keya', 'F25', 'ShiftLeft', 'Ctrl+KeyA+KeyB']) {
    assert.equal(validateShortcut(binding), 'invalid', binding);
  }
  for (const binding of ['', 'Ctrl+Alt+Shift+KeyA', 'F24', 'NumpadEqual', 'Ctrl+IntlBackslash']) {
    assert.equal(validateShortcut(binding), null, binding);
  }
});

test('operating-system reserved shortcuts are rejected', () => {
  for (const binding of ['Alt+F4', 'Alt+Tab', 'Alt+Escape', 'Alt+Space', 'Ctrl+Alt+Delete', 'Ctrl+Escape', 'Ctrl+Shift+Escape', 'Meta+KeyL', 'Ctrl+Meta+KeyA']) {
    assert.equal(validateShortcut(binding), 'reserved', binding);
  }
  assert.equal(findShortcutAction(key('KeyQ', { metaKey: true })), null);
});

test('conflicts identify the action to resolve, while several disabled actions are valid', () => {
  const conflicted = { ...DEFAULT_SHORTCUTS, next: 'Space' };
  assert.deepEqual(validateShortcuts(conflicted), [{ code: 'conflict', action: 'next', otherAction: 'playPause' }]);
  assert.deepEqual(validateShortcuts({ ...DEFAULT_SHORTCUTS, next: '', previous: '' }), []);
  assert.equal(validateShortcuts({ ...DEFAULT_SHORTCUTS, playPause: 'Alt+F4' })[0].code, 'reserved');
});

test('display labels remove DOM prefixes and distinguish numpad codes', () => {
  assert.equal(formatShortcut('Ctrl+Shift+KeyP'), 'Ctrl + Shift + P');
  assert.equal(formatShortcut('Ctrl+ArrowLeft'), 'Ctrl + ←');
  assert.equal(formatShortcut('Alt+Digit1'), 'Alt + 1');
  assert.equal(formatShortcut('Numpad1'), 'Num 1');
  assert.equal(formatShortcut(''), '');
});
