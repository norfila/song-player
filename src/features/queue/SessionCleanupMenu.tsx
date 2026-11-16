import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trash2 } from 'lucide-react';
import type { AppSnapshot, PlayerCommand } from '../../lib/backend/types';

/** True when the only queue left is an unnamed empty one, so deleting it would change nothing. */
export function onlyEmptySession(snapshot: AppSnapshot): boolean {
  const [only] = snapshot.sessions;
  return snapshot.sessions.length === 1 && !only.entries.length && !only.name;
}

/** Bulk delete for the queues page. The backend always keeps one queue: deleting
 * every queue leaves a fresh empty one. */
export function SessionCleanupMenu({ snapshot, onCommand }: { snapshot: AppSnapshot; onCommand: (value: PlayerCommand) => Promise<void> }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    const close = (event: KeyboardEvent) => { if (event.key === 'Escape') setOpen(false); };
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, [open]);

  const inactive = snapshot.sessions.filter(item => item.id !== snapshot.activeSessionId);
  const options = [
    { key: 'session.deleteAll', ids: onlyEmptySession(snapshot) ? [] : snapshot.sessions.map(item => item.id) },
    { key: 'session.deleteInactive', ids: inactive.map(item => item.id) },
    { key: 'session.deleteSmall', ids: inactive.filter(item => item.entries.length <= 1).map(item => item.id) },
  ];
  const remove = (sessionIds: string[]) => { setOpen(false); void onCommand({ type: 'delete_sessions', sessionIds }); };

  return <div className="menu-wrap">
    <button className="icon-button danger session-cleanup-button" title={t('session.cleanup')} aria-label={t('session.cleanup')} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}><Trash2 size={17} /></button>
    {open && <>
      <button className="menu-dismiss" aria-label={t('app.close')} onClick={() => setOpen(false)} />
      <div className="context-menu session-cleanup-context" role="menu" aria-label={t('session.cleanup')}>
        {options.map(option => <button key={option.key} role="menuitem" className="danger" disabled={!option.ids.length} onClick={() => remove(option.ids)}>
          <Trash2 size={15} /><span>{t(option.key)}</span><span className="menu-count">{option.ids.length}</span>
        </button>)}
      </div>
    </>}
  </div>;
}
