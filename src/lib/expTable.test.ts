import { describe, expect, it } from "vitest";
import { EXP_TABLE, LEVEL_CAP, MAX_LEVEL, levelProgress } from "./expTable";

describe("expTable", () => {
  it("carries the full 1.0 curve", () => {
    expect(LEVEL_CAP).toBe(80);
    expect(MAX_LEVEL).toBe(100);
    expect(EXP_TABLE.total).toHaveLength(MAX_LEVEL);
    expect(EXP_TABLE.palTotal).toHaveLength(MAX_LEVEL);
    expect(EXP_TABLE.total[0]).toBe(0);
    expect(EXP_TABLE.palTotal[0]).toBe(0);
  });

  it("is monotonically increasing on both tracks", () => {
    for (let i = 1; i < MAX_LEVEL; i++) {
      expect(EXP_TABLE.total[i]).toBeGreaterThan(EXP_TABLE.total[i - 1]);
      expect(EXP_TABLE.palTotal[i]).toBeGreaterThan(EXP_TABLE.palTotal[i - 1]);
    }
  });
});

describe("levelProgress", () => {
  it("starts level 1 at zero progress", () => {
    const p = levelProgress(0, 1, false);
    expect(p.into).toBe(0);
    expect(p.pct).toBe(0);
    expect(p.next).toBe(EXP_TABLE.total[1]);
  });

  it("tracks partial progress within a level", () => {
    const need = EXP_TABLE.total[1]; // level 1 → 2
    const p = levelProgress(need / 2, 1, false);
    expect(p.pct).toBeCloseTo(50);
    expect(p.into).toBe(need / 2);
    expect(p.next).toBe(need);
  });

  it("still shows a next level at the display cap (80 < table max)", () => {
    const p = levelProgress(EXP_TABLE.total[LEVEL_CAP - 1], LEVEL_CAP, false);
    expect(p.into).toBe(0);
    expect(p.next).toBe(EXP_TABLE.total[LEVEL_CAP] - EXP_TABLE.total[LEVEL_CAP - 1]);
    expect(p.next).toBeGreaterThan(0);
  });

  it("saturates at the table max level", () => {
    const p = levelProgress(EXP_TABLE.total[MAX_LEVEL - 1] + 12345, MAX_LEVEL, false);
    expect(p.pct).toBe(100);
    expect(p.next).toBeNull();
    expect(p.into).toBe(12345);
  });

  it("clamps out-of-range levels", () => {
    expect(levelProgress(0, 0, false).next).toBe(EXP_TABLE.total[1]); // treated as level 1
    expect(levelProgress(0, 999, false).next).toBeNull(); // clamped to max
  });

  it("never reports negative progress when exp is below the level threshold", () => {
    const p = levelProgress(0, 50, false);
    expect(p.into).toBe(0);
    expect(p.pct).toBe(0);
  });

  it("uses the Pal curve when pal is true", () => {
    const p = levelProgress(EXP_TABLE.palTotal[4], 5, true);
    expect(p.into).toBe(0);
    expect(p.next).toBe(EXP_TABLE.palTotal[5] - EXP_TABLE.palTotal[4]);
    // The two curves differ, so the same exp reads differently per track.
    expect(EXP_TABLE.palTotal[5]).not.toBe(EXP_TABLE.total[5]);
  });
});
