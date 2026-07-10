/** Pure geometry helpers for the radial gauge (kept separate for unit testing). */

export function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}

/**
 * stroke-dashoffset for a ring where the full circumference represents `max`.
 * value=0 → full offset (empty ring); value>=max → 0 (full ring).
 */
export function dashoffset(value: number, max: number, circumference: number): number {
  const frac = max <= 0 ? 0 : clamp(value / max, 0, 1);
  return circumference * (1 - frac);
}
