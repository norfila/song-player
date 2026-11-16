/** Reduces a peak list (0–1) to `count` bars by taking each bucket's maximum,
 * then lifts quiet bars slightly so the shape stays readable as a seek bar. */
export function waveformBars(peaks: readonly number[], count: number): number[] {
  const bars = Math.max(1, Math.floor(count));
  if (!peaks.length) return Array.from({ length: bars }, () => 0);
  const clean = peaks.map(value => (Number.isFinite(value) ? Math.min(1, Math.max(0, Math.abs(value))) : 0));
  const result: number[] = [];
  for (let index = 0; index < bars; index++) {
    const start = Math.floor((index * clean.length) / bars);
    const end = Math.max(start + 1, Math.floor(((index + 1) * clean.length) / bars));
    let peak = 0;
    for (let sample = start; sample < Math.min(end, clean.length); sample++) peak = Math.max(peak, clean[sample]);
    result.push(peak);
  }
  const loudest = Math.max(...result);
  if (loudest <= 0) return result;
  return result.map(value => Math.sqrt(value / loudest));
}
