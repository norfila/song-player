import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoaderCircle, X } from 'lucide-react';
import { api, isDesktop } from '../../lib/backend';
import type { AudioJob } from '../../lib/backend/types';

export function useAudioJob() {
  const [job, setJob] = useState<AudioJob | null>(null);
  function acceptJob(incoming: AudioJob | null) {
    setJob(current => current && incoming && current.id === incoming.id && ((current.status !== 'running' && incoming.status === 'running') || (current.status === 'running' && incoming.status === 'running' && incoming.progress < current.progress)) ? current : incoming);
  }
  useEffect(() => {
    if (!isDesktop) return;
    let live = true;
    let cleanup: (() => void) | undefined;
    let events = 0;
    void api.subscribeAudioJob(value => { events++; if (live) acceptJob(value); }).then(stop => { if (live) cleanup = stop; else stop(); }).catch(() => undefined);
    void api.getAudioJob().then(value => { if (live && events === 0) acceptJob(value); }).catch(() => undefined);
    return () => { live = false; cleanup?.(); };
  }, []);
  return { job, setJob: acceptJob };
}

export function AudioJobProgress({ job, onCancel }: { job: AudioJob | null; onCancel: () => void }) {
  const { t } = useTranslation();
  if (!job) return null;
  const running = job.status === 'running';
  return <div className={`tool-job ${job.status === 'failed' ? 'tool-error' : ''}`} role={job.status === 'failed' ? 'alert' : 'status'}>
    <div className="tool-job-heading"><span>{running && <LoaderCircle size={15} className="spin" />}{t(`features.jobs.${job.kind}.${job.status}`)}</span>{running && <button className="icon-button" title={t('features.common.cancel')} aria-label={t('features.common.cancel')} onClick={onCancel}><X size={16} /></button>}</div>
    {running && <progress max={1} value={Math.max(0, Math.min(1, job.progress))} aria-label={t(`features.jobs.${job.kind}.running`)} />}
    {job.status === 'completed' && job.outputPath && <p className="tool-output-path">{job.outputPath}</p>}
    {job.status === 'failed' && <p>{t('features.jobs.failedHint')}</p>}
    {job.status === 'failed' && job.message && <details className="tool-job-details"><summary>{t('features.jobs.details')}</summary><pre>{job.message.slice(0, 12000)}</pre></details>}
  </div>;
}
