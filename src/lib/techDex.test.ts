import { describe, expect, it } from "vitest";
import {
  TECH_TOTAL,
  techCell,
  techMeta,
  techTree,
} from "./techDex";

describe("techDex", () => {
  const tree = techTree();

  it("groups the full catalog by ascending level", () => {
    const levels = tree.map((r) => r.level);
    expect(levels).toEqual([...levels].sort((a, b) => a - b));
    expect(tree.length).toBeGreaterThan(0);
  });

  it("keeps regular and Ancient techs separated, ≤8 regular + ≤1 Ancient per level", () => {
    for (const row of tree) {
      expect(row.regular.length).toBeLessThanOrEqual(8);
      expect(row.regular.every((m) => !m.boss)).toBe(true);
      if (row.ancient) expect(row.ancient.boss).toBe(true);
    }
  });

  it("accounts for every technology exactly once", () => {
    const counted = tree.reduce((n, r) => n + r.regular.length + (r.ancient ? 1 : 0), 0);
    expect(counted).toBe(TECH_TOTAL);
    expect(TECH_TOTAL).toBeGreaterThan(500);
  });

  it("resolves an atlas cell for every tech that has an icon", () => {
    for (const row of tree) {
      for (const m of [...row.regular, row.ancient]) {
        if (!m || !m.icon) continue;
        const cell = techCell(m.code);
        expect(cell, `${m.code} should resolve a cell`).not.toBeNull();
        expect(cell!.col).toBeGreaterThanOrEqual(0);
        expect(cell!.row).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("returns null metadata / cell for an unknown code", () => {
    expect(techMeta("NotARealTech_zzz")).toBeNull();
    expect(techCell("NotARealTech_zzz")).toBeNull();
  });
});
