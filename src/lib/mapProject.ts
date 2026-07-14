/**
 * Palworld world coordinates → position on the in-game world-map textures.
 *
 * Ported from the community palworld-save-pal project's Palworld-1.0 map code
 * (`ui/src/lib/components/map/utils.ts`). The projection is no longer
 * hand-calibrated: each map **area** carries the world min/max bounds taken
 * straight from the game's `DT_WorldMapUIData`, so pixels are exact by
 * construction. Palworld 1.0 has two areas — the main Palpagos map and the new
 * **World Tree** region — each with its own 8192² texture and bounds.
 *
 * Axis note (the thing that trips everyone up): the map's HORIZONTAL axis
 * follows world **+Y** (west→east) and its VERTICAL axis follows world **X**.
 * `worldToPixel` returns OpenLayers-style y-up pixels (0 = bottom); our canvas
 * layer is y-down, so `worldToUv` flips the v axis.
 */

/** Native size of each square map texture, in pixels. */
export const MAP_SIZE = 8192;

export type MapArea = "MainMap" | "Tree";

interface AreaBounds {
  /** Public texture path we ship for this area. */
  texture: string;
  /** World-coordinate min/max (Unreal cm) from the game's DT_WorldMapUIData. */
  min: { x: number; y: number };
  max: { x: number; y: number };
}

/**
 * Bounds + textures for each map area. `Tree` is listed first because it carries
 * the game's WorldMapPriority 1: where the two rectangles overlap, it wins (see
 * `mapOf`, which iterates in this order).
 */
export const MAP_AREAS: Record<MapArea, AreaBounds> = {
  Tree: {
    texture: "/palworld-treemap.webp",
    min: { x: 347351.5, y: -818197.0 },
    max: { x: 689148.5, y: -476400.0 },
  },
  MainMap: {
    texture: "/palworld-map.webp",
    min: { x: -1099400.0, y: -724400.0 },
    max: { x: 349400.0, y: 724400.0 },
  },
};

/** Left-to-right order for a UI area switcher (main map first). */
export const MAP_AREA_ORDER: MapArea[] = ["MainMap", "Tree"];

export const DEFAULT_MAP_AREA: MapArea = "MainMap";

// The in-game coordinate readout (the numbers shown in the game's own UI) is a
// separate concern from pixel placement and keeps its original constants.
const TRANSLATION_X = 123930.0;
const TRANSLATION_Y = 157935.0;
const SCALE = 459.0;

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

/** Centimetres of world space per texture pixel, derived from the area bounds. */
export function cmPerPx(area: MapArea): number {
  const { min, max } = MAP_AREAS[area];
  return (max.x - min.x) / MAP_SIZE;
}

/**
 * (worldX, worldY) → pixel on the area's 8192² texture, in OpenLayers' y-up
 * space (0 = bottom). Map horizontal axis is world +Y; the vertical flip
 * cancels in y-up, leaving pixelY = (worldX − min.x) / cm.
 */
export function worldToPixel(worldX: number, worldY: number, area: MapArea): [number, number] {
  const { min } = MAP_AREAS[area];
  const cm = cmPerPx(area);
  return [(worldY - min.y) / cm, (worldX - min.x) / cm];
}

/**
 * Which map area a world position belongs to — the game's own rule, in
 * priority order (World Tree first). Returns null if the point is off every map.
 */
export function mapOf(worldX: number, worldY: number): MapArea | null {
  for (const area of Object.keys(MAP_AREAS) as MapArea[]) {
    const { min, max } = MAP_AREAS[area];
    if (worldX >= min.x && worldX <= max.x && worldY >= min.y && worldY <= max.y) {
      return area;
    }
  }
  return null;
}

/**
 * (worldX, worldY) → { u, v } in [0,1] over the area's texture.
 * u: west→east (left→right), v: north→south (top→bottom, flipped from the y-up
 * pixel space into our y-down canvas layer).
 */
export function worldToUv(worldX: number, worldY: number, area: MapArea): { u: number; v: number } {
  const [px, py] = worldToPixel(worldX, worldY, area);
  return {
    u: clamp01(px / MAP_SIZE),
    v: clamp01(1 - py / MAP_SIZE),
  };
}

/**
 * Convenience: resolve a world position to its area and UV in one call. A point
 * off every area's exact rectangle falls back to `MainMap`, so a stray
 * coordinate never silently vanishes.
 */
export function projectWorld(
  worldX: number,
  worldY: number,
): { area: MapArea; u: number; v: number } {
  const area = mapOf(worldX, worldY) ?? DEFAULT_MAP_AREA;
  return { area, ...worldToUv(worldX, worldY, area) };
}

/**
 * (worldX, worldY) → in-game map coordinate as shown to players (the numbers on
 * the game's own map readout). Rounded to whole units. This matches the value
 * the game (and palworld-save-pal's tooltips) display for a world position.
 */
export function worldToGameCoords(worldX: number, worldY: number): { x: number; y: number } {
  return {
    x: Math.round((worldY - TRANSLATION_Y) / SCALE),
    y: Math.round((worldX + TRANSLATION_X) / SCALE),
  };
}
