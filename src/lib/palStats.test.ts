import { describe, expect, it } from "vitest";
import { computePalStats, hasPalStats, palScaling } from "./palStats";

// Melpaca ("Alpaca" code) has scaling hp 90 / attack 75 / defense 90.
const base = {
  characterId: "Alpaca",
  level: 1,
  talentHp: 0,
  talentShot: 0,
  talentDefense: 0,
  rankHp: 0,
  rankAttack: 0,
  rankDefense: 0,
  rankCraftspeed: 0,
  rank: 1,
  passiveSkills: [] as string[],
  isBoss: false,
  isLucky: false,
};

describe("palScaling", () => {
  it("resolves a species' scaling via cleansed character id", () => {
    expect(palScaling("Alpaca")).toEqual({ hp: 90, attack: 75, defense: 90 });
    expect(palScaling("BOSS_Alpaca")).toEqual({ hp: 90, attack: 75, defense: 90 });
    expect(hasPalStats("Alpaca")).toBe(true);
    expect(hasPalStats("NotAPal")).toBe(false);
  });
});

describe("computePalStats", () => {
  it("returns null for an unknown species", () => {
    expect(computePalStats({ ...base, characterId: "NotAPal" })).toBeNull();
  });

  it("matches the reference formula at baseline (level 1, no IVs/souls/passives)", () => {
    const s = computePalStats(base)!;
    // hp = floor(500 + 5 + 90*0.5) = 550
    expect(s.hp).toBe(550);
    // attack = floor(75*0.075) = floor(5.625) = 5
    expect(s.attack).toBe(5);
    // defense = floor(50 + 90*0.075) = floor(56.75) = 56
    expect(s.defense).toBe(56);
    expect(s.workSpeed).toBe(70);
  });

  it("applies self-targeted passive bonuses (Legend = +20% attack & defense)", () => {
    const s = computePalStats({ ...base, passiveSkills: ["Legend"] })!;
    expect(s.attack).toBe(6); // floor(5 * 1.2)
    expect(s.defense).toBe(67); // floor(56 * 1.2)
  });

  it("scales HP with level, IVs, souls, condenser and alpha", () => {
    const s = computePalStats({
      ...base,
      level: 50,
      talentHp: 100,
      rankHp: 20,
      rank: 5,
      isLucky: true,
    })!;
    // hpBase = floor(500 + 250 + 90*0.5*50*(1+0.3)*1.2) = floor(750 + 3510) = 4260
    // hp = floor(4260 * (1+0.2) * (1+0.6)) = floor(4260 * 1.2 * 1.6) = 8179
    expect(s.hp).toBe(8179);
  });

  it("caps effective level at the owner's level", () => {
    const high = computePalStats({ ...base, level: 60 })!;
    const capped = computePalStats({ ...base, level: 60, ownerLevel: 30 })!;
    expect(capped.hp).toBeLessThan(high.hp);
    // Equivalent to computing at level 30.
    expect(capped.hp).toBe(computePalStats({ ...base, level: 30 })!.hp);
  });
});
