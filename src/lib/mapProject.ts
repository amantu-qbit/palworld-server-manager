/** Projection helpers for the live world radar. */

export interface Bounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
}

/** Fallback world extent when actors are missing or degenerate. */
const DEFAULT_BOUNDS: Bounds = { minX: -500000, maxX: 500000, minY: -500000, maxY: 500000 };

function clamp(v: number, lo: number, hi: number): number {
  return v < lo ? lo : v > hi ? hi : v;
}

/**
 * Tight min/max extent over the actors, grown by a fractional `pad` on each
 * axis. Returns a safe default when the list is empty or has no real span.
 */
export function computeBounds(actors: { LocationX: number; LocationY: number }[], pad = 0.06): Bounds {
  if (!actors || actors.length === 0) return { ...DEFAULT_BOUNDS };

  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;

  for (const a of actors) {
    const x = a.LocationX;
    const y = a.LocationY;
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return { ...DEFAULT_BOUNDS };

  const spanX = maxX - minX;
  const spanY = maxY - minY;
  if (Math.max(spanX, spanY) <= 0) return { ...DEFAULT_BOUNDS };

  const padX = spanX * pad;
  const padY = spanY * pad;
  return { minX: minX - padX, maxX: maxX + padX, minY: minY - padY, maxY: maxY + padY };
}

/**
 * Project a world (x, y) into radar screen space of side `size`. Uses a single
 * uniform scale (the larger axis span) so the layout keeps its aspect ratio and
 * every point stays inside [0, size]. Y is inverted so north points up. Never
 * returns NaN.
 */
export function projectToRadar(x: number, y: number, b: Bounds, size: number): { x: number; y: number } {
  const spanX = b.maxX - b.minX;
  const spanY = b.maxY - b.minY;
  const span = Math.max(spanX, spanY);
  const half = size / 2;

  if (!Number.isFinite(span) || span <= 0) return { x: half, y: half };

  const cx = (b.minX + b.maxX) / 2;
  const cy = (b.minY + b.maxY) / 2;
  const scale = size / span;

  const scaledX = half + (x - cx) * scale;
  const scaledY = half + (y - cy) * scale;

  let sx = clamp(scaledX, 0, size);
  let sy = clamp(size - scaledY, 0, size); // invert Y: north is up

  if (!Number.isFinite(sx)) sx = half;
  if (!Number.isFinite(sy)) sy = half;

  return { x: sx, y: sy };
}
