export interface VolumeWheelInput {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  ctrlKey: boolean;
  metaKey: boolean;
}

export interface VolumeWheelResult {
  handled: boolean;
  volume: number;
  remainder: number;
}

/** One usual wheel notch (100 pixels or three lines) changes volume by two points. */
export function consumeVolumeWheel(volume: number, remainder: number, input: VolumeWheelInput): VolumeWheelResult {
  const { deltaX, deltaY, deltaMode, ctrlKey, metaKey } = input;
  if (ctrlKey || metaKey || !Number.isFinite(deltaY) || !deltaY || Math.abs(deltaX) >= Math.abs(deltaY)) {
    return { handled: false, volume, remainder };
  }
  const delta = -deltaY * (deltaMode === 1 ? 2 / 3 : deltaMode === 2 ? 2 : 2 / 100);
  // A reversal should respond immediately instead of cancelling the previous gesture's fraction.
  const accumulated = delta + (Math.sign(delta) === Math.sign(remainder) ? remainder : 0);
  const points = Math.trunc(accumulated);
  const next = Math.max(0, Math.min(100, volume + points));
  const atLimit = (next === 100 && delta > 0) || (next === 0 && delta < 0);
  return { handled: true, volume: next, remainder: atLimit ? 0 : accumulated - points };
}
