/**
 * Technology catalog + sprite-atlas resolution for the in-game-style tech tree.
 * Mirrors the Pal/item atlas pattern (src/lib/palDex.ts): a bundled atlas
 * (`public/mapicons/tech-atlas.webp`) plus a compact catalog derived from
 * palworld-save-pal's `technologies.json` (name, unlock level, point cost,
 * Ancient flag, what it unlocks, and its icon).
 */
import dexRaw from "../data/techDex.json";
import atlasRaw from "../data/techAtlas.json";

export type TechKind = "structure" | "item" | "other";

export interface TechMeta {
  /** Internal technology code, e.g. "DefenseWall_Wood". */
  code: string;
  /** Localized display name, e.g. "Wooden Defensive Wall". */
  name: string;
  /** Player level at which it becomes available (`level_cap`). */
  level: number;
  /** Technology-point cost to unlock. */
  cost: number;
  /** Ancient technology (costs Ancient points, defeated-boss gated). */
  boss: boolean;
  /** Whether it primarily unlocks a structure or an item recipe. */
  kind: TechKind;
  /** Atlas icon base-name ("" if none). */
  icon: string;
  /** Flavor description (may be absent). */
  desc?: string;
}

type RawEntry = Omit<TechMeta, "code">;
const DEX = dexRaw as Record<string, RawEntry>;
const ATLAS = atlasRaw as { cols: number; cell: number; keys: string[] };
const KEY_INDEX = new Map(ATLAS.keys.map((k, i) => [k, i] as const));

export const TECH_ATLAS_COLS = ATLAS.cols;
export const TECH_ATLAS_CELL = ATLAS.cell;
export const TECH_ATLAS_SIZE = ATLAS.cols * ATLAS.cell;
export const TECH_TOTAL = Object.keys(DEX).length;
export const TECH_POINT_ICON = "/mapicons/tech-point.webp";
export const ANCIENT_POINT_ICON = "/mapicons/ancient-tech-point.webp";

export function techMeta(code: string): TechMeta | null {
  const e = DEX[code];
  return e ? { code, ...e } : null;
}

/** Top-left atlas offset in cell units (col,row) for a tech, or null. */
export function techCell(code: string): { col: number; row: number } | null {
  const icon = DEX[code]?.icon;
  if (!icon) return null;
  const idx = KEY_INDEX.get(icon);
  if (idx == null) return null;
  return { col: idx % ATLAS.cols, row: Math.floor(idx / ATLAS.cols) };
}

export interface TechLevel {
  level: number;
  regular: TechMeta[];
  ancient: TechMeta | null;
}

let cachedTree: TechLevel[] | null = null;

/**
 * The full technology catalog grouped by unlock level (ascending), each row
 * carrying up to eight regular techs plus at most one Ancient tech — the same
 * shape the game and palworld-save-pal lay the tree out in.
 */
export function techTree(): TechLevel[] {
  if (cachedTree) return cachedTree;
  const byLevel = new Map<number, TechLevel>();
  for (const [code, e] of Object.entries(DEX)) {
    let row = byLevel.get(e.level);
    if (!row) {
      row = { level: e.level, regular: [], ancient: null };
      byLevel.set(e.level, row);
    }
    const meta: TechMeta = { code, ...e };
    if (e.boss) row.ancient = meta;
    else row.regular.push(meta);
  }
  for (const row of byLevel.values()) {
    row.regular.sort((a, b) => a.cost - b.cost || a.name.localeCompare(b.name));
  }
  cachedTree = [...byLevel.values()].sort((a, b) => a.level - b.level);
  return cachedTree;
}
