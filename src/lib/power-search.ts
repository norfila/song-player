export interface SearchDocument { id: string; label: string; detail: string; keywords?: string; kind: 'track' | 'action'; disabled?: boolean }
export type SearchUsage = Record<string, { count: number; usedAt: number }>;
export const normalizeSearch = (value: string) => value.toLocaleLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
function distance(a: string, b: string) {
  let row = Array.from({ length: b.length + 1 }, (_, index) => index);
  let previous: number[] | null = null;
  for (let i = 1; i <= a.length; i++) {
    const next = [i];
    for (let j = 1; j <= b.length; j++) {
      next[j] = Math.min(next[j - 1] + 1, row[j] + 1, row[j - 1] + Number(a[i - 1] !== b[j - 1]));
      if (previous && i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) next[j] = Math.min(next[j], previous[j - 2] + 1);
    }
    previous = row;
    row = next;
  }
  return row[b.length];
}
export function rankSearch(items: SearchDocument[], query: string, usage: SearchUsage = {}, now = Date.now()) {
  const commands = query.trimStart().startsWith('>');
  const normalized = normalizeSearch(commands ? query.trimStart().slice(1) : query);
  const terms = normalized.split(' ').filter(Boolean);
  return items.filter(item => item.kind === (commands ? 'action' : 'track') && !item.disabled).map(item => {
    const label = normalizeSearch(item.label), haystack = normalizeSearch(`${item.label} ${item.detail} ${item.keywords ?? ''}`), words = haystack.split(' ');
    let score = !normalized ? 10 : label === normalized ? 100 : label.startsWith(normalized) ? 85 : haystack.includes(normalized) ? 70 : 0;
    if (!score && terms.length) {
      const values = terms.map(term => {
        if (words.some(word => word.startsWith(term))) return 60;
        if (words.some(word => word.includes(term))) return 52;
        const limit = term.length >= 6 ? 2 : term.length >= 3 ? 1 : 0;
        if (!limit) return 0;
        const nearest = Math.min(...words.filter(word => Math.abs(word.length - term.length) <= limit).map(word => distance(term, word)));
        return nearest <= limit ? 44 - nearest * 4 : 0;
      });
      if (values.every(Boolean)) score = Math.min(...values);
    }
    const learned = usage[`${normalized}|${item.id}`];
    const boost = learned ? Math.min(8, Math.log2(1 + learned.count) * 2) / (1 + Math.max(0, now - learned.usedAt) / (90 * 86400000)) : 0;
    return { item, score: score ? score + boost : 0 };
  }).filter(result => result.score > 0).sort((a, b) => b.score - a.score || a.item.label.localeCompare(b.item.label)).slice(0, 60);
}
export const SEARCH_USAGE_KEY = 'audio-player.search-usage.v1';
export function loadSearchUsage(): SearchUsage {
  try { const value: unknown = JSON.parse(localStorage.getItem(SEARCH_USAGE_KEY) || '{}'); if (!value || typeof value !== 'object' || Array.isArray(value)) return {}; return Object.fromEntries(Object.entries(value).filter(([, entry]) => entry && typeof entry.count === 'number' && entry.count > 0 && Number.isFinite(entry.usedAt)).slice(-2000)); } catch { return {}; }
}
export function rememberSearch(query: string, id: string) {
  const usage = loadSearchUsage(), key = `${normalizeSearch(query.replace(/^\s*>/, ''))}|${id}`;
  usage[key] = { count: Math.min(10000, (usage[key]?.count || 0) + 1), usedAt: Date.now() };
  try { localStorage.setItem(SEARCH_USAGE_KEY, JSON.stringify(Object.fromEntries(Object.entries(usage).sort((a, b) => a[1].usedAt - b[1].usedAt).slice(-2000)))); } catch { /* Search also works when browser storage is unavailable. */ }
}
export function resetSearchUsage() { try { localStorage.removeItem(SEARCH_USAGE_KEY); } catch { /* No saved learning available. */ } }
