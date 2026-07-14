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

/** Pretty label for a player/pal status-point name. */
const STATUS_LABELS: Record<string, string> = {
  MaxHP: "Health",
  MaxSP: "Stamina",
  Attack: "Attack",
  ShotAttack: "Attack",
  Weight: "Weight",
  CaptureRate: "Capture Power",
  WorkSpeed: "Work Speed",
  Support: "Support",
};
export function statusLabel(code: string): string {
  const key = code.replace(/^EPalStatusName::/, "");
  return STATUS_LABELS[key] ?? humanize(code);
}
