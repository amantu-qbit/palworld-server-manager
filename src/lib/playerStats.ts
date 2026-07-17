/**
 * Player status-point catalog + derived-stat formulas.
 *
 * Ported from palworld-save-pal:
 *  - on-disk `StatusName` → canonical key: `psp-core` `STATUS_NAME_MAP`
 *    (Japanese strings, verified against real Palworld 1.0 saves — they are the
 *    game's *universal* internal ids, written on every server regardless of the
 *    display language, so no language detection is needed to read or write them).
 *  - total formulas: `ui/.../player/PlayerStats.svelte`.
 *  - relic caps: `data/json/relic_data.json`.
 *
 * The game creates a `{ StatusName, StatusPoint }` row lazily: a stat at rank 0
 * simply has no row. The base/hero stats + `捕獲率` are always present; the other
 * relic stats appear only once allocated. Editing an existing row is a plain
 * patch; raising a stat from 0 appends a new row (the bridge does this, keyed by
 * the canonical Japanese string).
 */

export type StatKind = "hero" | "relic";

export interface StatDef {
  /** Canonical key (`max_hp`, `attack`, `glider_speed`, …). */
  key: string;
  /** On-disk `StatusName` — the game's universal internal id. */
  jp: string;
  /** English UI label. */
  label: string;
  /** In-game maximum rank. */
  cap: number;
  kind: StatKind;
}

/** Hero stats: always present on disk, cap 50, drive the big derived totals.
 *  Order matches the in-game status screen. */
export const HERO_STATS: StatDef[] = [
  { key: "max_hp", jp: "最大HP", label: "Health", cap: 50, kind: "hero" },
  { key: "max_sp", jp: "最大SP", label: "Stamina", cap: 50, kind: "hero" },
  { key: "attack", jp: "攻撃力", label: "Attack", cap: 50, kind: "hero" },
  { key: "work_speed", jp: "作業速度", label: "Work Speed", cap: 50, kind: "hero" },
  { key: "weight", jp: "所持重量", label: "Weight", cap: 50, kind: "hero" },
];

/** Relic / utility stats. `capture_rate` is always present; the rest appear once
 *  allocated. Order = palworld-save-pal `RELIC_ORDER`. */
export const RELIC_STATS: StatDef[] = [
  { key: "capture_rate", jp: "捕獲率", label: "Capture Power", cap: 15, kind: "relic" },
  { key: "hunger_reduction", jp: "空腹率低減", label: "Hunger Resistance", cap: 20, kind: "relic" },
  { key: "swim_speed", jp: "泳ぎ速度", label: "Swim Speed", cap: 20, kind: "relic" },
  { key: "food_decay_reduction", jp: "食料腐敗低減", label: "Food Preservation", cap: 20, kind: "relic" },
  { key: "jump_power", jp: "ジャンプ力", label: "Jump Power", cap: 20, kind: "relic" },
  { key: "glider_speed", jp: "滑空速度", label: "Glide Speed", cap: 20, kind: "relic" },
  { key: "climb_speed", jp: "崖登り速度", label: "Climb Speed", cap: 20, kind: "relic" },
  { key: "status_ailment_resist", jp: "状態異常耐性", label: "Ailment Resist", cap: 20, kind: "relic" },
  { key: "stamina_reduction", jp: "スタミナ消費軽減", label: "Stamina Cost", cap: 20, kind: "relic" },
  { key: "sphere_homing", jp: "パルスフィアホーミング", label: "Pal Sphere Aim", cap: 4, kind: "relic" },
  { key: "exp_bonus", jp: "経験値ボーナス", label: "EXP Bonus", cap: 4, kind: "relic" },
  { key: "rainbow_passive_rate", jp: "虹パッシブ率", label: "Rare Passive Rate", cap: 4, kind: "relic" },
  { key: "move_speed", jp: "移動速度アップ", label: "Movement Speed", cap: 92, kind: "relic" },
];

export const ALL_STATS: StatDef[] = [...HERO_STATS, ...RELIC_STATS];

/** The extended status list (`GotExStatusPointList`) only carries the hero five. */
export const EXT_STATS: StatDef[] = HERO_STATS;

/** Hero-stat total formula: `base + per * rank` (palworld-save-pal parity). */
export interface TotalDef {
  key: string;
  label: string;
  base: number;
  per: number;
}
export const TOTAL_STATS: TotalDef[] = [
  { key: "max_hp", label: "Health", base: 500, per: 100 },
  { key: "max_sp", label: "Stamina", base: 100, per: 10 },
  { key: "attack", label: "Attack", base: 100, per: 2 },
  { key: "work_speed", label: "Work Speed", base: 100, per: 50 },
  { key: "weight", label: "Weight", base: 300, per: 50 },
];

/** On-disk `StatusName` (any known form) → canonical key. The catalog's Japanese
 *  strings are authoritative; a few tolerant aliases cover older/variant strings
 *  and English enum forms some tools emit, so reading never drops a real row. */
const NAME_TO_KEY: Record<string, string> = (() => {
  const m: Record<string, string> = {};
  for (const s of ALL_STATS) m[s.jp] = s.key;
  Object.assign(m, {
    移動速度: "move_speed",
    登り速度: "climb_speed",
    レアパッシブ率: "rainbow_passive_rate",
    MaxHP: "max_hp",
    MaxSP: "max_sp",
    Attack: "attack",
    Weight: "weight",
    CaptureRate: "capture_rate",
    WorkSpeed: "work_speed",
  });
  return m;
})();

/** Canonical key for an on-disk `StatusName`, or `undefined` if unrecognized. */
export function canonicalKeyFor(onDiskName: string): string | undefined {
  return NAME_TO_KEY[onDiskName];
}

export interface ResolvedStat {
  def: StatDef;
  /** Current allocated rank (0 when the save has no row). */
  value: number;
  /** The name to send back on save: an existing row's exact on-disk name, or the
   *  canonical Japanese string when appending a new row. */
  onDiskName: string;
  /** Whether the save already has a row for this stat. */
  present: boolean;
}

/** Resolve a save's on-disk status map (name → rank) into the catalog stats. */
export function resolveStats(
  onDisk: Record<string, number>,
  catalog: StatDef[] = ALL_STATS,
): ResolvedStat[] {
  const byKey: Record<string, { onDiskName: string; value: number }> = {};
  for (const [name, value] of Object.entries(onDisk)) {
    const key = NAME_TO_KEY[name];
    if (key && byKey[key] === undefined) byKey[key] = { onDiskName: name, value };
  }
  return catalog.map((def) => {
    const hit = byKey[def.key];
    return {
      def,
      value: hit?.value ?? 0,
      onDiskName: hit?.onDiskName ?? def.jp,
      present: hit !== undefined,
    };
  });
}

export interface ComputedTotal {
  key: string;
  label: string;
  value: number;
}

/** Derived hero-stat totals from canonical ranks (palworld-save-pal parity). */
export function computeTotals(rankByKey: Record<string, number>): ComputedTotal[] {
  return TOTAL_STATS.map((t) => ({
    key: t.key,
    label: t.label,
    value: t.base + t.per * (rankByKey[t.key] ?? 0),
  }));
}
