/**
 * Pal display data: resolves a bridge `character_id` (a code name like
 * "SheepBall") to the name players actually see ("Lamball"), its elements and
 * rarity, and its position in the bundled sprite atlas — reusing the same icon
 * atlas and key-resolution the World Map uses.
 */
import dexRaw from "../data/palDex.json";
import atlasRaw from "../data/palAtlas.json";
import { cleanseCharacterId, palIconKey } from "./mapData";

interface DexEntry {
  name: string;
  elements: string[];
  rarity: number;
}

const DEX = dexRaw as Record<string, DexEntry>;
const ATLAS = atlasRaw as { cols: number; cell: number; keys: string[] };
const KEY_INDEX = new Map(ATLAS.keys.map((k, i) => [k, i] as const));

/** Full atlas pixel dimensions (square, 22×22 grid of 64px cells). */
export const ATLAS_COLS = ATLAS.cols;
export const ATLAS_CELL = ATLAS.cell;
export const ATLAS_SIZE = ATLAS.cols * ATLAS.cell;

export interface PalInfo {
  /** Localized display name, e.g. "Jetragon". */
  name: string;
  /** Element types, e.g. ["Water", "Ice"]. */
  elements: string[];
  /** Rarity 1–20 (≥8 ≈ rare/legendary). */
  rarity: number;
  /** Top-left atlas offset in cell units (col, row), or null if no icon. */
  cell: { col: number; row: number } | null;
}

const humanize = (s: string) =>
  s
    .replace(/_/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim();

/** Resolve a `character_id` to its display name, elements, rarity, and icon cell. */
export function palInfo(characterId: string): PalInfo {
  const entry = DEX[cleanseCharacterId(characterId)];
  const idx = KEY_INDEX.get(palIconKey(characterId));
  const cell =
    idx != null ? { col: idx % ATLAS.cols, row: Math.floor(idx / ATLAS.cols) } : null;
  return {
    name: entry?.name || humanize(characterId),
    elements: entry?.elements ?? [],
    rarity: entry?.rarity ?? 0,
    cell,
  };
}

/** Element → accent color (internal Palworld element names). */
export const ELEMENT_COLOR: Record<string, string> = {
  Normal: "#b9bcc6",
  Fire: "#f0743c",
  Water: "#3fa7f0",
  Leaf: "#5cc46a",
  Grass: "#5cc46a",
  Electric: "#f0c93c",
  Ice: "#6fd6e6",
  Earth: "#c79a5b",
  Ground: "#c79a5b",
  Dark: "#9b6fd6",
  Dragon: "#c76fe0",
};

export function elementColor(el: string): string {
  return ELEMENT_COLOR[el] ?? "#b9bcc6";
}

/** Rarity ≥ 8 gets the gold "rare" treatment (Jetragon, Anubis, Necromus…). */
export function isRare(rarity: number): boolean {
  return rarity >= 8;
}

/**
 * Best-effort element for an active-skill (`EPalWazaID::…`) code, keyed off
 * common element tokens in the id. Used only for a hint chip in the skill
 * picker — codes that don't map simply show no chip.
 */
const WAZA_ELEMENT_TOKENS: [RegExp, string][] = [
  [/fire|flame|ignis|burn|inferno/i, "Fire"],
  [/water|aqua|bubble|splash|hydro/i, "Water"],
  [/thunder|electric|spark|lightning|volt|plasma/i, "Electric"],
  [/ice|frost|snow|blizzard|cryst/i, "Ice"],
  [/dragon/i, "Dragon"],
  [/dark|night|shadow|abyss|spirit/i, "Dark"],
  [/sand|stone|rock|earth|mud|ground/i, "Earth"],
  [/leaf|grass|seed|bloom|wind|tornado/i, "Leaf"],
];

export function wazaElement(code: string): string | null {
  const s = code.replace(/^EPalWazaID::/, "");
  for (const [re, el] of WAZA_ELEMENT_TOKENS) if (re.test(s)) return el;
  return null;
}
