import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowRight, Command, Music2, Search, X } from 'lucide-react';
import { Dialog } from './Dialog';
import { loadSearchUsage, rankSearch, rememberSearch } from '../lib/power-search';
import type { SearchDocument } from '../lib/power-search';
import './expansion.css';
import './quick-search.css';
export function QuickSearch({ items, tracks, onSelect, onClose }: { items: SearchDocument[]; tracks: SearchDocument[]; onSelect: (item: SearchDocument) => void; onClose: () => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState(''), [selected, setSelected] = useState(0);
  const usage = useMemo(loadSearchUsage, []);
  const actionKey = JSON.stringify(items);
  const results = useMemo(() => rankSearch([...tracks, ...items], query, usage), [tracks, actionKey, query, usage]);
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => { setSelected(0); }, [query]);
  useEffect(() => { container.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }); }, [selected]);
  const choose = (item: SearchDocument) => { rememberSearch(query, item.id); onSelect(item); onClose(); };
  return <Dialog title={t('workspace.quickSearch')} onClose={onClose} className="quick-search-dialog"><div className="expansion-search"><Search size={20} /><input data-autofocus value={query} role="combobox" aria-label={t('workspace.quickSearch')} placeholder={t('workspace.quickSearchHint')} aria-controls="quick-search-results" aria-expanded="true" aria-autocomplete="list" aria-activedescendant={results[selected] ? `quick-result-${selected}` : undefined} onChange={event => setQuery(event.target.value)} onKeyDown={event => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); setSelected(index => (index + (event.key === 'ArrowDown' ? 1 : -1) + Math.max(1, results.length)) % Math.max(1, results.length)); }
    if (event.key === 'Enter') { event.preventDefault(); if (results[selected]) choose(results[selected].item); }
  }} /><button className="icon-button" aria-label={t('app.close')} onClick={onClose}><X size={18} /></button></div><p className="quick-search-caption">{t(query.trimStart().startsWith('>') ? 'workspace.actionSearchHint' : 'workspace.songSearchHint')}</p>
    <div id="quick-search-results" role="listbox" ref={container} className="quick-search-results" aria-label={t('workspace.quickSearch')}>{results.map(({ item }, index) => <button role="option" aria-selected={selected === index} id={`quick-result-${index}`} tabIndex={-1} key={item.id} className="quick-search-result" onMouseEnter={() => setSelected(index)} onClick={() => choose(item)}>{item.kind === 'action' ? <Command size={18} /> : <Music2 size={18} />}<span><strong>{item.label}</strong><small>{item.detail}</small></span><ArrowRight size={16} /></button>)}{!results.length && <p className="text-secondary">{t('app.noResults')}</p>}</div><div className="quick-search-footer"><span>↑ ↓ {t('workspace.choose')}</span><span>Enter {t('workspace.execute')}</span><span>Esc {t('app.close')}</span></div></Dialog>;
}
