import { describe, expect, it } from "vitest";
import {
  MAP_AREAS,
  MAP_SIZE,
  cmPerPx,
  mapOf,
  projectWorld,
  worldToGameCoords,
  worldToPixel,
  worldToUv,
} from "./mapProject";

// Ground-truth values from the reference palworld-save-pal 1.0 map converter
// (ui/src/lib/components/map/utils.ts + its utils.test.ts). Our port must
// reproduce these exactly, so markers land where that tool places them.

describe("cmPerPx", () => {
  it("derives the scale from the DataTable bounds", () => {
    expect(cmPerPx("MainMap")).toBeCloseTo(176.85546875, 6);
    expect(cmPerPx("Tree")).toBeCloseTo(41.723266, 5);
  });
});

describe("worldToPixel", () => {
  it("maps each area's corner to the extent corner", () => {
    for (const area of ["MainMap", "Tree"] as const) {
      const { min, max } = MAP_AREAS[area];
      expect(worldToPixel(min.x, min.y, area)).toEqual([0, 0]);
      const [px, py] = worldToPixel(max.x, max.y, area);
      expect(px).toBeCloseTo(MAP_SIZE, 4);
      expect(py).toBeCloseTo(MAP_SIZE, 4);
    }
  });

  it("places World Tree fast-travel statues on their landmarks", () => {
    const [ax, ay] = worldToPixel(512112, -510663, "Tree"); // WorldTree_A
    expect(ax).toBeCloseTo(7370.8, 1);
    expect(ay).toBeCloseTo(3948.89, 1);

    const [bx, by] = worldToPixel(501010, -748555, "Tree"); // WorldTree_LastBoss
    expect(bx).toBeCloseTo(1669.14, 1);
    expect(by).toBeCloseTo(3682.8, 1);
  });
});

describe("worldToUv", () => {
  it("puts the main-map world origin dead-centre horizontally", () => {
    const p = worldToUv(0, 0, "MainMap");
    expect(p.u).toBeCloseTo(0.5, 4); // world Y 0 is exactly mid-longitude
    expect(p.v).toBeCloseTo(0.24116, 4);
  });

  it("places a World Tree statue on the tree texture", () => {
    const p = worldToUv(512112, -510663, "Tree");
    expect(p.u).toBeCloseTo(0.899756, 4);
    expect(p.v).toBeCloseTo(0.517958, 4);
  });

  // Axis orientation: the Palworld map swaps world axes.
  it("moves east (u increases) as world Y increases", () => {
    expect(worldToUv(0, 200000, "MainMap").u).toBeGreaterThan(worldToUv(0, -200000, "MainMap").u);
  });

  it("moves north (v decreases) as world X increases", () => {
    expect(worldToUv(200000, 0, "MainMap").v).toBeLessThan(worldToUv(-200000, 0, "MainMap").v);
  });

  it("clamps far out-of-world coordinates into [0,1]", () => {
    const ne = worldToUv(9_000_000, 9_000_000, "MainMap");
    expect(ne.u).toBe(1);
    expect(ne.v).toBe(0);
  });
});

describe("mapOf", () => {
  it("assigns Palpagos coordinates to the main map", () => {
    expect(mapOf(0, 0)).toBe("MainMap");
  });

  it("assigns World Tree coordinates to the Tree area", () => {
    expect(mapOf(512112, -510663)).toBe("Tree");
  });

  it("returns null for a point off every map", () => {
    expect(mapOf(9_000_000, 9_000_000)).toBeNull();
  });
});

describe("projectWorld", () => {
  it("resolves area + uv for a Tree point", () => {
    const p = projectWorld(512112, -510663);
    expect(p.area).toBe("Tree");
    expect(p.u).toBeCloseTo(0.899756, 4);
  });

  it("falls back to the main map for an off-map point (never vanishes)", () => {
    expect(projectWorld(9_000_000, 9_000_000).area).toBe("MainMap");
  });
});

describe("worldToGameCoords", () => {
  it("maps the game origin to (0, 0)", () => {
    expect(worldToGameCoords(-123930, 157935)).toEqual({ x: 0, y: 0 });
  });

  it("reports the in-game readout for the world origin", () => {
    expect(worldToGameCoords(0, 0)).toEqual({ x: -344, y: 270 });
  });
});
