import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderPlus, X } from 'lucide-react';
import { basename } from '../lib/format';
import './expansion.css';
export function ExternalFolderOffer({ folders, onAdd, onClose }: { folders: string[]; onAdd: () => Promise<void>; onClose: () => void }) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false), [focused, setFocused] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(false);
  const close = useRef(onClose); close.current = onClose;
  useEffect(() => { setError(false); }, [folders]);
  useEffect(() => { if (hovered || focused || busy) return; const timer = setTimeout(() => close.current(), 15000); return () => clearTimeout(timer); }, [hovered, focused, busy, folders]);
  return <aside className="docked-bar" role="status" onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)} onFocusCapture={() => setFocused(true)} onBlurCapture={event => { if (!event.currentTarget.contains(event.relatedTarget as Node)) setFocused(false); }}><FolderPlus size={20} /><span>{t(error ? 'errors.unknown' : 'workspace.addOpenedFolder', { name: folders.length === 1 ? basename(folders[0]) : t('workspace.folderCount', { count: folders.length }) })}</span><button className="button primary" disabled={busy} onClick={() => { setBusy(true); void onAdd().then(onClose).catch(() => setError(true)).finally(() => setBusy(false)); }}>{t('workspace.addFolder')}</button><button className="icon-button" disabled={busy} aria-label={t('app.close')} onClick={onClose}><X size={16} /></button></aside>;
}
