/**
 * Palworld world coordinates → position on the Palpagos map image.
 *
 * The REST API returns Unreal world coordinates (LocationX/LocationY, the same
 * space as the .sav file). The community Paldex conversion maps that square world
 * space onto the map:
 *   world min (-582888, -301000), max (335112, 617000)  → a 918000-unit square
 *   paldex = ((x + 123888) / 459, (y - 158000) / 459)   → -1000..1000, origin centre
 *
 * For rendering we express it as a normalised (u, v) in [0,1] over the map image:
 *   u: 0 = west edge  → 1 = east edge
 *   v: 0 = north edge → 1 = south edge   (image top is north)
 */

export const WORLD = {
  minX: -582888,
  maxX: 335112,
  minY: -301000,
  maxY: 617000,
  span: 918000, // maxX-minX === maxY-minY
} as const;

export const PALDEX_SCALE = 459;
export const PALDEX_OFFSET = { x: 123888, y: 158000 } as const;

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

/** (worldX, worldY) → { u, v } in [0,1]. u: west→east, v: north→south. */
export function worldToUv(worldX: number, worldY: number): { u: number; v: number } {
  return {
    u: clamp01((worldX - WORLD.minX) / WORLD.span),
    v: clamp01((WORLD.maxY - worldY) / WORLD.span),
  };
}

/** (worldX, worldY) → in-game Paldex map coordinate (-1000..1000, origin centre). */
export function worldToPaldex(worldX: number, worldY: number): { x: number; y: number } {
  return {
    x: Math.round((worldX + PALDEX_OFFSET.x) / PALDEX_SCALE),
    y: Math.round((worldY - PALDEX_OFFSET.y) / PALDEX_SCALE),
  };
}
