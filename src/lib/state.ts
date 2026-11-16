import type { AppSnapshot, Playback } from './backend/types';

/** Structural state and playback events have independent monotonic clocks. */
export function mergeSnapshot(previous: AppSnapshot | null, incoming: AppSnapshot): AppSnapshot {
  if (!previous) return incoming;
  if (incoming.revision < previous.revision) return previous;
  return incoming.playback.sequence < previous.playback.sequence
    ? { ...incoming, playback: previous.playback }
    : incoming;
}

export function mergePlayback(previous: AppSnapshot | null, incoming: Playback): AppSnapshot | null {
  if (!previous || incoming.sequence < previous.playback.sequence) return previous;
  return { ...previous, playback: incoming };
}
