/**
 * Item icon resolution: maps a bridge item `static_id` to its cell in the
 * bundled item sprite atlas (`public/mapicons/items-atlas.webp`), via the
 * item→icon-name map. Item *names* come from the bridge's `items` reference.
 */
import atlas from "../data/itemAtlas.json";
import icons from "../data/itemIcons.json";

const ATLAS = atlas as { cols: number; cell: number; keys: string[] };
const ICONS = icons as Record<string, string>;
const KEY_INDEX = new Map(ATLAS.keys.map((k, i) => [k, i] as const));

export const ITEM_ATLAS_CELL = ATLAS.cell;
export const ITEM_ATLAS_W = ATLAS.cols * ATLAS.cell;
export const ITEM_ATLAS_H = Math.ceil(ATLAS.keys.length / ATLAS.cols) * ATLAS.cell;

/** Atlas cell (col,row) for an item `static_id`, or null if it has no icon. */
export function itemCell(staticId: string): { col: number; row: number } | null {
  const iconName = ICONS[staticId];
  if (!iconName) return null;
  const idx = KEY_INDEX.get(iconName);
  if (idx == null) return null;
  return { col: idx % ATLAS.cols, row: Math.floor(idx / ATLAS.cols) };
}
