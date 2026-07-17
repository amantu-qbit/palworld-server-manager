import { describe, expect, it } from "vitest";
import {
  ALL_STATS,
  canonicalKeyFor,
  computeTotals,
  HERO_STATS,
  RELIC_STATS,
  resolveStats,
} from "./playerStats";

describe("playerStats catalog", () => {
  it("has the 18 Palworld 1.0 status stats (5 hero + 13 relic)", () => {
    expect(HERO_STATS).toHaveLength(5);
    expect(RELIC_STATS).toHaveLength(13);
    expect(ALL_STATS).toHaveLength(18);
  });

  it("uses the authoritative Japanese ids for the strings palworld-save-pal corrected", () => {
    const byKey = Object.fromEntries(ALL_STATS.map((s) => [s.key, s.jp]));
    expect(byKey.climb_speed).toBe("崖登り速度");
    expect(byKey.rainbow_passive_rate).toBe("虹パッシブ率");
    expect(byKey.move_speed).toBe("移動速度アップ");
  });

  it("maps on-disk Japanese names (and tolerant aliases) to canonical keys", () => {
    expect(canonicalKeyFor("最大HP")).toBe("max_hp");
    expect(canonicalKeyFor("捕獲率")).toBe("capture_rate");
    // Alias forms still resolve so a real row is never dropped.
    expect(canonicalKeyFor("移動速度")).toBe("move_speed");
    expect(canonicalKeyFor("MaxHP")).toBe("max_hp");
    expect(canonicalKeyFor("nonsense")).toBeUndefined();
  });
});

describe("resolveStats", () => {
  it("reads present rows and defaults absent stats to rank 0", () => {
    const resolved = resolveStats({ 最大HP: 28, 滑空速度: 3 });
    const hp = resolved.find((r) => r.def.key === "max_hp")!;
    expect(hp.value).toBe(28);
    expect(hp.present).toBe(true);
    expect(hp.onDiskName).toBe("最大HP");

    const glide = resolved.find((r) => r.def.key === "glider_speed")!;
    expect(glide.value).toBe(3);
    expect(glide.present).toBe(true);

    // A stat with no row: rank 0, not present, ready to append under its jp id.
    const climb = resolved.find((r) => r.def.key === "climb_speed")!;
    expect(climb.value).toBe(0);
    expect(climb.present).toBe(false);
    expect(climb.onDiskName).toBe("崖登り速度");
  });
});

describe("computeTotals", () => {
  it("matches palworld-save-pal hero-stat formulas", () => {
    // A fully-maxed hero build.
    const maxed = { max_hp: 50, max_sp: 50, attack: 50, work_speed: 50, weight: 50 };
    const totals = Object.fromEntries(
      computeTotals(maxed).map((t) => [t.key, t.value]),
    );
    expect(totals.max_hp).toBe(5500); // 500 + 50*100
    expect(totals.max_sp).toBe(600); // 100 + 50*10
    expect(totals.attack).toBe(200); // 100 + 50*2
    expect(totals.work_speed).toBe(2600); // 100 + 50*50
    expect(totals.weight).toBe(2800); // 300 + 50*50
  });

  it("treats missing ranks as 0 (base values)", () => {
    const totals = Object.fromEntries(computeTotals({}).map((t) => [t.key, t.value]));
    expect(totals.max_hp).toBe(500);
    expect(totals.weight).toBe(300);
  });
});
