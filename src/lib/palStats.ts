/**
 * Computed Pal combat stats — HP, Attack, Defense, Work Speed.
 *
 * A direct port of palworld-save-pal's `ui/src/lib/utils/stats.ts::getStats`.
 * These totals are not stored in the save; the game derives them from the
 * species' scaling (src/data/palScaling.json), the Pal's level, IVs (talents),
 * souls (ranks), condenser rank, boss/lucky alpha scaling, and self-targeted
 * passive-skill bonuses (src/data/passiveEffects.json).
 *
 * Displayed HP is the game-facing value; the save's internal `Hp` is this × 1000.
 */
import scalingRaw from "../data/palScaling.json";
import passiveRaw from "../data/passiveEffects.json";
import { cleanseCharacterId } from "./mapData";

interface Scaling {
  hp: number;
  attack: number;
  defense: number;
}
interface PassiveEffect {
  attack?: number;
  defense?: number;
  work_speed?: number;
}

const SCALING = scalingRaw as Record<string, Scaling>;
const PASSIVE = passiveRaw as Record<string, PassiveEffect>;

/** Species scaling for a `character_id`, or `undefined` for unknown species. */
export function palScaling(characterId: string): Scaling | undefined {
  return SCALING[cleanseCharacterId(characterId)];
}

/** Whether we can compute stats for this species (scaling data is bundled). */
export function hasPalStats(characterId: string): boolean {
  return palScaling(characterId) !== undefined;
}

export interface PalStatInputs {
  characterId: string;
  level: number;
  /** Talent_HP (HP IV, 0–100). */
  talentHp: number;
  /** Talent_Shot (Attack IV, 0–100). */
  talentShot: number;
  /** Talent_Defense (Defense IV, 0–100). */
  talentDefense: number;
  /** Rank_HP soul (0–20). */
  rankHp: number;
  /** Rank_Attack soul (0–20). */
  rankAttack: number;
  /** Rank_Defence soul (0–20). */
  rankDefense: number;
  /** Rank_CraftSpeed soul (0–20). */
  rankCraftspeed: number;
  /** Condenser rank, 1-based (1 = no stars, 5 = 4 stars). */
  rank: number;
  passiveSkills: string[];
  isBoss: boolean;
  isLucky: boolean;
  /** Owner's level; the game caps a Pal's effective level at its owner's. */
  ownerLevel?: number;
}

export interface PalComputedStats {
  hp: number;
  attack: number;
  defense: number;
  workSpeed: number;
}

/** Sum the self-targeted attack/defense/work-speed bonuses of a passive set. */
function passiveBonuses(skills: string[]): {
  attack: number;
  defense: number;
  workSpeed: number;
} {
  let attack = 0;
  let defense = 0;
  let workSpeed = 0;
  for (const code of skills) {
    const e = PASSIVE[code];
    if (!e) continue;
    attack += e.attack ?? 0;
    defense += e.defense ?? 0;
    workSpeed += e.work_speed ?? 0;
  }
  return { attack, defense, workSpeed };
}

/**
 * Compute a Pal's HP / Attack / Defense / Work Speed from its current (possibly
 * edited) stat inputs. Returns `null` for species we have no scaling for.
 */
export function computePalStats(i: PalStatInputs): PalComputedStats | null {
  const scaling = SCALING[cleanseCharacterId(i.characterId)];
  if (!scaling) return null;

  const level =
    i.ownerLevel !== undefined ? Math.min(i.ownerLevel, i.level) : i.level;
  const { attack: attackBonus, defense: defenseBonus, workSpeed: workSpeedBonus } =
    passiveBonuses(i.passiveSkills);

  // Condenser rank is 1-based; clamp so an unranked pal never goes negative.
  const condenserBonus = (Math.max(1, i.rank) - 1) * 0.05;
  const alphaScaling = i.isBoss || i.isLucky ? 1.2 : 1;

  // HP (displayed value; the save stores this × 1000).
  const hpIv = (i.talentHp * 0.3) / 100;
  const hpSoulBonus = i.rankHp * 0.03;
  const hpBase = Math.floor(
    500 + 5 * level + scaling.hp * 0.5 * level * (1 + hpIv) * alphaScaling,
  );
  const hp = Math.floor(hpBase * (1 + condenserBonus) * (1 + hpSoulBonus));

  // Attack.
  const attackIv = (i.talentShot * 0.3) / 100;
  const attackSoulBonus = i.rankAttack * 0.03;
  const attackBase = Math.floor(scaling.attack * 0.075 * level * (1 + attackIv));
  const attack = Math.floor(
    attackBase * (1 + condenserBonus) * (1 + attackSoulBonus) * (1 + attackBonus),
  );

  // Defense.
  const defenseIv = (i.talentDefense * 0.3) / 100;
  const defenseSoulBonus = i.rankDefense * 0.03;
  const defenseBase = Math.floor(50 + scaling.defense * 0.075 * level * (1 + defenseIv));
  const defense = Math.floor(
    defenseBase * (1 + condenserBonus) * (1 + defenseSoulBonus) * (1 + defenseBonus),
  );

  // Work speed: base 70, modified only by passive bonuses (Rank_CraftSpeed
  // souls raise the in-work rate, not this base figure — matching the reference).
  const workSpeed = Math.round(70 * (1 + workSpeedBonus));

  return { hp, attack, defense, workSpeed };
}
