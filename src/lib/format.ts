export function time(value: number): string {
  const seconds = Math.max(0, Math.floor(Number.isFinite(value) ? value : 0));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = String(seconds % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

export function parentFolder(path: string): string {
  const normal = path.replaceAll('\\', '/');
  return normal.slice(0, normal.lastIndexOf('/'));
}

export function basename(path: string): string {
  return path.replaceAll('\\', '/').split('/').filter(Boolean).at(-1) || path;
}
