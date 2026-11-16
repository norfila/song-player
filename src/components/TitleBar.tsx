import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioLines, Check, Copy, Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, isDesktop } from '../lib/backend';
import { findShortcutAction, formatShortcut, resolveShortcuts } from '../lib/shortcuts';
import type { AppSnapshot, PlayerCommand, RepeatMode } from '../lib/backend/types';
import './titlebar.css';

interface Props {
  snapshot: AppSnapshot | null;
  onCommand: (command: PlayerCommand) => Promise<void>;
  onRun: (operation: () => Promise<AppSnapshot | null | void>) => Promise<void>;
  onNowPlaying: () => void;
  onSettings: () => void;
  onToggleQueue: () => void;
  queueVisible: boolean;
  onError: (error: unknown) => void;
}
type MenuName = 'file' | 'playback' | 'view';
type Item = { label: string; action: () => void; shortcut?: string; checked?: boolean; disabled?: boolean; separator?: boolean };

export function TitleBar({ snapshot, onCommand, onRun, onNowPlaying, onSettings, onToggleQueue, queueVisible, onError }: Props) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState<MenuName | null>(null);
  const [maximized, setMaximized] = useState(false);
  const root = useRef<HTMLElement>(null);
  const trigger = useRef<Partial<Record<MenuName, HTMLButtonElement | null>>>({});
  const errorRef = useRef(onError);
  errorRef.current = onError;
  const names: MenuName[] = ['file', 'playback', 'view'];
  const native = (operation: () => Promise<unknown>) => { if (isDesktop) void operation().catch(error => errorRef.current(error)); };
  const closeWindow = () => native(() => getCurrentWindow().close());
  const toggleMaximize = () => native(() => getCurrentWindow().toggleMaximize());

  useEffect(() => {
    if (!isDesktop) return;
    let alive = true;
    let off: (() => void) | undefined;
    const current = getCurrentWindow();
    const update = () => { void current.isMaximized().then(max => { if (alive) setMaximized(max); }).catch(error => errorRef.current(error)); };
    update();
    void current.onResized(update).then(unlisten => { if (alive) off = unlisten; else unlisten(); }).catch(error => errorRef.current(error));
    return () => { alive = false; off?.(); };
  }, []);

  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.defaultPrevented || (event.target as HTMLElement).closest('[data-shortcut-recorder]')) return;
      if (document.querySelector('[aria-modal="true"]')) return;
      if (findShortcutAction(event, snapshot?.settings.shortcuts)) return;
      if (event.altKey && !event.ctrlKey && !event.metaKey) {
        const next = ['f', 'd'].includes(event.key.toLowerCase()) ? 'file' : ['p', 'w'].includes(event.key.toLowerCase()) ? 'playback' : ['v', 'a'].includes(event.key.toLowerCase()) ? 'view' : null;
        if (next) { event.preventDefault(); setMenu(next); }
      }
    };
    window.addEventListener('keydown', keys);
    return () => window.removeEventListener('keydown', keys);
  }, [snapshot?.settings.shortcuts]);

  useEffect(() => {
    if (!menu) return;
    const frame = requestAnimationFrame(() => root.current?.querySelector<HTMLButtonElement>('[role="menu"] button:not(:disabled)')?.focus());
    const outside = (event: PointerEvent) => { if (!root.current?.querySelector('.titlebar-menus')?.contains(event.target as Node)) setMenu(null); };
    const blur = () => setMenu(null);
    window.addEventListener('pointerdown', outside);
    window.addEventListener('blur', blur);
    return () => { cancelAnimationFrame(frame); window.removeEventListener('pointerdown', outside); window.removeEventListener('blur', blur); };
  }, [menu]);

  const active = snapshot?.sessions.find(session => session.id === snapshot.activeSessionId);
  const shortcuts = resolveShortcuts(snapshot?.settings.shortcuts);
  const available = !!active?.entries.length && !!snapshot?.engine.available;
  const runCommand = (command: PlayerCommand) => { void onCommand(command); };
  const repeatItem = (mode: RepeatMode): Item => ({ label: t(mode === 'off' ? 'player.repeatOff' : mode === 'one' ? 'player.repeatOne' : 'player.repeatAll'), checked: active?.repeat === mode, action: () => runCommand({ type: 'set_repeat', mode }), disabled: !snapshot });
  const items: Record<MenuName, Item[]> = {
    file: [
      { label: t('native.openFiles'), shortcut: formatShortcut(shortcuts.openFiles), action: () => void onRun(() => api.openFiles('play')) },
      { label: t('native.openFolder'), shortcut: formatShortcut(shortcuts.addFolder), action: () => void onRun(() => api.openFolder('library')) },
      { label: t('library.openFolder'), action: () => void onRun(() => api.openFolder('play')) },
      { label: t('library.importPlaylist'), separator: true, action: () => void onRun(() => api.importPlaylist()) },
      { label: t('native.quit'), shortcut: formatShortcut(shortcuts.quit), separator: true, action: () => void onRun(() => api.quit()), disabled: !isDesktop },
    ],
    playback: [
      { label: t('native.playPause'), shortcut: formatShortcut(shortcuts.playPause), disabled: !available, action: () => runCommand({ type: 'play_pause' }) },
      { label: t('player.previous'), shortcut: formatShortcut(shortcuts.previous), disabled: !available, action: () => runCommand({ type: 'previous' }) },
      { label: t('player.next'), shortcut: formatShortcut(shortcuts.next), disabled: !available, action: () => runCommand({ type: 'next' }) },
      { label: t('player.shuffle'), separator: true, checked: !!active?.orderMode.startsWith('shuffle'), disabled: !active?.entries.length, action: () => runCommand({ type: 'set_shuffle', enabled: !active?.orderMode.startsWith('shuffle') }) },
      { ...repeatItem('off'), separator: true }, repeatItem('all'), repeatItem('one'),
      { label: t('features.loudness.title'), separator: true, checked: !!snapshot?.settings.replayGain, disabled: !snapshot?.engine.available, action: () => runCommand({ type: 'set_replay_gain', enabled: !snapshot?.settings.replayGain }) },
    ],
    view: [
      { label: t('player.expand'), shortcut: formatShortcut(shortcuts.nowPlaying), action: onNowPlaying, disabled: !snapshot },
      { label: t(queueVisible ? 'app.hideQueue' : 'app.showQueue'), shortcut: formatShortcut(shortcuts.toggleQueue), checked: queueVisible, action: onToggleQueue },
      { label: t('settings.title'), separator: true, action: onSettings },
    ],
  };
  const choose = (item: Item) => { const current = menu; setMenu(null); if (current) trigger.current[current]?.focus(); item.action(); };

  return <header className="window-titlebar" ref={root}>
    <div className="titlebar-brand" data-tauri-drag-region><AudioLines size={19} data-tauri-drag-region /><span data-tauri-drag-region>{t('app.productName')}</span></div>
    <nav className="titlebar-menus" role="menubar" aria-label={t('window.menu')}>
      {names.map(name => <div key={name} className="titlebar-menu-wrap">
        <button ref={element => { trigger.current[name] = element; }} role="menuitem" aria-haspopup="menu" aria-expanded={menu === name} className={`titlebar-menu-button ${menu === name ? 'open' : ''}`} onClick={() => setMenu(menu === name ? null : name)} onPointerEnter={() => { if (menu) setMenu(name); }} onKeyDown={event => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); setMenu(name); }
          if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') { event.preventDefault(); trigger.current[names[(names.indexOf(name) + (event.key === 'ArrowRight' ? 1 : 2)) % 3]]?.focus(); }
        }}>{t(name === 'view' ? 'window.view' : `native.${name}`)}</button>
        {menu === name && <div className="titlebar-popup" role="menu" aria-label={t(name === 'view' ? 'window.view' : `native.${name}`)} onBlur={event => { if (!root.current?.querySelector('.titlebar-menus')?.contains(event.relatedTarget as Node)) setMenu(null); }} onKeyDown={event => {
          if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); setMenu(null); trigger.current[name]?.focus(); }
          const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
          const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
          if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) { event.preventDefault(); buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length]?.focus(); }
          if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') { event.preventDefault(); setMenu(names[(names.indexOf(name) + (event.key === 'ArrowRight' ? 1 : 2)) % 3]); }
        }}>
          {items[name].map((item, index) => <div key={index}>{item.separator && <div className="menu-divider" role="separator" />}<button role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'} aria-checked={item.checked} disabled={item.disabled} onClick={() => choose(item)}><span className="titlebar-check">{item.checked && <Check size={14} />}</span><span>{item.label}</span>{item.shortcut && <kbd>{item.shortcut}</kbd>}</button></div>)}
        </div>}
      </div>)}
    </nav>
    <div className="titlebar-drag" data-tauri-drag-region aria-hidden="true" />
    <div className="window-controls">
      <button title={t('window.minimize')} aria-label={t('window.minimize')} disabled={!isDesktop} onClick={() => native(() => getCurrentWindow().minimize())}><Minus size={16} /></button>
      <button title={t(maximized ? 'window.restore' : 'window.maximize')} aria-label={t(maximized ? 'window.restore' : 'window.maximize')} disabled={!isDesktop} onClick={toggleMaximize}>{maximized ? <Copy size={13} /> : <Square size={13} />}</button>
      <button className="window-close" title={t('window.close')} aria-label={t('window.close')} disabled={!isDesktop} onClick={closeWindow}><X size={18} /></button>
    </div>
  </header>;
}
