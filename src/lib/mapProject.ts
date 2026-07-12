/**
 * Palworld world coordinates → position on the in-game world map texture.
 *
 * The REST API returns Unreal world coordinates (LocationX/LocationY, the same
 * space as the .sav file). This converter is ported from the community
 * palworld-save-pal project, calibrated against the 8192×8192 `t_worldmap`
 * texture we ship as `public/palworld-map.webp`.
 *
 * Note the axis swap that trips most people up: the map's HORIZONTAL axis
 * follows world Y (west→east) and its VERTICAL axis follows world X (north→south).
 *
 * Pipeline: world → game coords (÷scale, with translation) → pixel (affine,
 * y-up like OpenLayers) → normalised (u, v) for our y-down CSS layer:
 *   u: 0 = west edge  → 1 = east edge
 *   v: 0 = north edge → 1 = south edge
 */

/** Native size of the square map texture, in pixels. */
export const MAP_SIZE = 8192;

const SCALE = 459;
const TRANSLATION_X = 123930;
const TRANSLATION_Y = 157935;

// Game-coordinate extents of the map, used to derive the game→pixel affine.
const GAME_MIN_X = -1951;
const GAME_MAX_X = 1198;
const GAME_MIN_Y = -1893;
const GAME_MAX_Y = 1243;

// game → texture-pixel transform (B/D are hand-calibrated so the origin lands right).
const TRANSFORM_A = MAP_SIZE / (GAME_MAX_X - GAME_MIN_X); // x scale
const TRANSFORM_B = 5075.45; // x offset
const TRANSFORM_C = -MAP_SIZE / (GAME_MAX_Y - GAME_MIN_Y); // y scale (inverts axis)
const TRANSFORM_D = 4960.62; // y offset

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}

/**
 * (worldX, worldY) → in-game map coordinate as shown to players (the numbers on
 * the game's own map readout). Rounded to whole units.
 */
export function worldToGameCoords(worldX: number, worldY: number): { x: number; y: number } {
  return {
    x: Math.round((worldY - TRANSLATION_Y) / SCALE),
    y: Math.round((worldX + TRANSLATION_X) / SCALE),
  };
}

/**
 * (worldX, worldY) → pixel on the 8192² texture, in OpenLayers' y-up space
 * (0 = bottom). Exposed mainly for testing against the reference converter.
 */
export function worldToPixel(worldX: number, worldY: number): { px: number; py: number } {
  const mapX = Math.round((worldY - TRANSLATION_Y) / SCALE);
  const mapY = Math.round((worldX + TRANSLATION_X) / SCALE) * -1;
  return {
    px: TRANSFORM_A * mapX + TRANSFORM_B,
    py: TRANSFORM_C * mapY + TRANSFORM_D,
  };
}

/**
 * (worldX, worldY) → { u, v } in [0,1] over the map image.
 * u: west→east (left→right), v: north→south (top→bottom). The v axis flips the
 * y-up pixel space into our y-down CSS layer.
 */
export function worldToUv(worldX: number, worldY: number): { u: number; v: number } {
  const { px, py } = worldToPixel(worldX, worldY);
  return {
    u: clamp01(px / MAP_SIZE),
    v: clamp01(1 - py / MAP_SIZE),
  };
}
