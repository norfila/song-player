export function displayTime(seconds: number): string {
  const whole = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  return whole >= 3600 ? `${Math.floor(whole / 3600)}:${String(Math.floor(whole / 60) % 60).padStart(2, '0')}:${String(whole % 60).padStart(2, '0')}` : `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

export function errorCode(error: unknown): string {
  if (typeof error === 'object' && error && 'code' in error && typeof error.code === 'string') return error.code;
  return 'unknown';
}
