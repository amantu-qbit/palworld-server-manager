import { describe, expect, it } from "vitest";
import {
  DISABLED_ITEMS,
  DISABLED_PALS,
  DISABLED_PASSIVES,
  SKILL_META,
  passiveRank,
  passiveRankColor,
} from "./skillMeta";

describe("skillMeta catalog shape", () => {
  it("carries a rank for every passive", () => {
    const entries = Object.entries(SKILL_META.passiveRank);
    expect(entries.length).toBeGreaterThan(400);
    for (const [id, rank] of entries) {
      expect(typeof id).toBe("string");
      expect(Number.isInteger(rank)).toBe(true);
    }
  });

  it("lists disabled content as non-empty string arrays", () => {
    for (const list of [SKILL_META.disabledItems, SKILL_META.disabledPassives, SKILL_META.disabledPals]) {
      expect(list.length).toBeGreaterThan(0);
      expect(list.every((id) => typeof id === "string" && id.length > 0)).toBe(true);
    }
  });

  it("only disables passives that exist in the rank map", () => {
    for (const id of SKILL_META.disabledPassives) {
      expect(SKILL_META.passiveRank[id], `${id} should have a rank`).toBeDefined();
    }
  });

  it("exposes known entries", () => {
    expect(passiveRank("Legend")).toBe(4);
    expect(DISABLED_ITEMS.has("Beer")).toBe(true);
    expect(DISABLED_PASSIVES.size).toBe(SKILL_META.disabledPassives.length);
    expect(DISABLED_PALS.size).toBe(SKILL_META.disabledPals.length);
  });

  it("reads unknown passives as rank 0", () => {
    expect(passiveRank("NotARealPassive_zzz")).toBe(0);
  });
});

describe("passiveRankColor", () => {
  it("ports the palworld-save-pal rank palette", () => {
    expect(passiveRankColor(-3)).toBe("#ec6a6a");
    expect(passiveRankColor(-1)).toBe("#ec6a6a");
    expect(passiveRankColor(0)).toBe("#62646c");
    expect(passiveRankColor(1)).toBe("#62646c");
    expect(passiveRankColor(2)).toBe("#fcdf19");
    expect(passiveRankColor(3)).toBe("#fcdf19");
    expect(passiveRankColor(4)).toBe("#68ffd8");
    expect(passiveRankColor(5)).toBe("#68ffd8");
    expect(passiveRankColor(9)).toBe("#68ffd8"); // a few passives carry rank 9
  });
});
