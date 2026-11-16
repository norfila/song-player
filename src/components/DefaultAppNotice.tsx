import { useTranslation } from 'react-i18next';
import { FileAudio2 } from 'lucide-react';
import type { DefaultAppStatus } from '../lib/backend';

export function DefaultAppNotice({ status, onOpen, onLater, onDontRemind }: {
  status: DefaultAppStatus;
  onOpen: () => void;
  onLater: () => void;
  onDontRemind: () => void;
}) {
  const { t } = useTranslation();
  return <aside className="default-app-notice" aria-label={t('defaultApps.title')}>
    <FileAudio2 size={20} />
    <div className="default-app-copy"><strong>{t('defaultApps.reminder')}</strong><p>{t('defaultApps.reminderDescription')}</p><details><summary>{t('defaultApps.formats', { count: status.missingExtensions.length })}</summary><span>{status.missingExtensions.join(' · ')}</span></details></div>
    <div className="default-app-actions"><button className="button secondary" onClick={onOpen}>{t('defaultApps.openSettings')}</button><button className="button quiet" onClick={onLater}>{t('defaultApps.later')}</button><button className="button quiet" onClick={onDontRemind}>{t('defaultApps.dontRemind')}</button></div>
  </aside>;
}
