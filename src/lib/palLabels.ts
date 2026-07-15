/** Human-readable labels for Pal/player data codes. */
import techRaw from "../data/techNames.json";

interface TechEntry {
  name: string;
  level: number;
  boss: boolean;
}
const TECH = techRaw as Record<string, TechEntry>;

export function humanize(s: string): string {
  return s
    .replace(/^EPal\w+::/, "")
    .replace(/^E\w+::/, "")
    .replace(/_/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim();
}

/** Resolve a technology code to `{ name, level, boss }` (in-game unlock level). */
export function techInfo(code: string): TechEntry {
  return TECH[code] ?? { name: humanize(code), level: 0, boss: false };
}

/** Pretty label for a Pal work-suitability enum. */
const WORK_LABELS: Record<string, string> = {
  EmitFlame: "Kindling",
  Watering: "Watering",
  Seeding: "Planting",
  GenerateElectricity: "Generating Electricity",
  Handcraft: "Handiwork",
  Collection: "Gathering",
  Deforest: "Lumbering",
  Mining: "Mining",
  OilExtraction: "Extracting",
  ProductMedicine: "Medicine Production",
  Cool: "Cooling",
  Transport: "Transporting",
  MonsterFarm: "Farming",
};
export function workLabel(code: string): string {
  const key = code.replace(/^EPalWorkSuitability::/, "");
  return WORK_LABELS[key] ?? humanize(code);
}

/**
 * Pretty label for a player/pal status-point name. Palworld stores these as the
 * *localized display string* in the server's language (not an enum), so the same
 * stat reads e.g. "最大HP" on a Japanese server. Map the known enum + localized
 * forms to a canonical English label; unknown strings fall back to themselves.
 */
const STATUS_LABELS: Record<string, string> = {
  // Enum / internal forms.
  MaxHP: "Health",
  MaxSP: "Stamina",
  Attack: "Attack",
  ShotAttack: "Attack",
  Weight: "Weight",
  CaptureRate: "Capture Power",
  WorkSpeed: "Work Speed",
  Support: "Support",
  // Japanese display forms (the language most servers surfaced in these saves).
  最大HP: "Health",
  最大SP: "Stamina",
  攻撃力: "Attack",
  所持重量: "Weight",
  捕獲率: "Capture Power",
  作業速度: "Work Speed",
  滑空速度: "Glide Speed",
  移動速度アップ: "Movement Speed",
  泳ぎ速度: "Swim Speed",
  ジャンプ力: "Jump Power",
  空腹率低減: "Hunger Resistance",
  パルスフィアホーミング: "Pal Sphere Aim",
};
export function statusLabel(code: string): string {
  const key = code.replace(/^EPalStatusName::/, "");
  return STATUS_LABELS[code] ?? STATUS_LABELS[key] ?? humanize(code);
}
