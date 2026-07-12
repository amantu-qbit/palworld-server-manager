import { describe, expect, it } from "vitest";
import { worldToGameCoords, worldToUv } from "./mapProject";

// Expected values produced by the reference palworld-save-pal converter
// (ui/src/lib/components/map/utils.ts) for known in-game landmarks. Our port
// must reproduce these exactly, so markers land where that tool places them.
describe("worldToUv", () => {
  it("places the game origin (world -123930, 157935) at the calibrated point", () => {
    const p = worldToUv(-123930, 157935);
    expect(p.u).toBeCloseTo(0.619562, 4);
    expect(p.v).toBeCloseTo(0.394456, 4);
  });

  it("matches the reference for Beach of Everlasting Summer (far south-west)", () => {
    const p = worldToUv(-383183.16, -210790.92);
    expect(p.u).toBeCloseTo(0.36456, 4);
    expect(p.v).toBeCloseTo(0.574621, 4);
  });

  it("matches the reference for Free Pal Alliance Tower Entrance", () => {
    const p = worldToUv(-103434.98, 234761.17);
    expect(p.u).toBeCloseTo(0.672594, 4);
    expect(p.v).toBeCloseTo(0.380106, 4);
  });

  it("maps world (0,0) to the reference point", () => {
    const p = worldToUv(0, 0);
    expect(p.u).toBeCloseTo(0.510321, 4);
    expect(p.v).toBeCloseTo(0.308359, 4);
  });

  // Axis orientation: the Palworld map swaps world axes.
  it("moves east (u increases) as world Y increases", () => {
    expect(worldToUv(0, 200000).u).toBeGreaterThan(worldToUv(0, -200000).u);
  });

  it("moves north (v decreases) as world X increases", () => {
    expect(worldToUv(200000, 0).v).toBeLessThan(worldToUv(-200000, 0).v);
  });

  it("clamps far out-of-world coordinates into [0,1]", () => {
    const ne = worldToUv(9_000_000, 9_000_000);
    expect(ne.u).toBe(1);
    expect(ne.v).toBe(0);
    const sw = worldToUv(-9_000_000, -9_000_000);
    expect(sw.u).toBe(0);
    expect(sw.v).toBe(1);
  });
});

describe("worldToGameCoords", () => {
  it("maps the game origin to (0, 0)", () => {
    expect(worldToGameCoords(-123930, 157935)).toEqual({ x: 0, y: 0 });
  });

  it("matches the reference readout for Ancient Ritual Site", () => {
    expect(worldToGameCoords(-257951.39, 151247.84)).toEqual({ x: -15, y: -292 });
  });
});
