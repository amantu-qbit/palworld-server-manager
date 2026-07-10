import { describe, expect, it } from "vitest";
import { WORLD, worldToPaldex, worldToUv } from "./mapProject";

describe("worldToUv", () => {
  it("maps the north-west corner (minX, maxY) to the top-left", () => {
    const p = worldToUv(WORLD.minX, WORLD.maxY);
    expect(p.u).toBeCloseTo(0);
    expect(p.v).toBeCloseTo(0);
  });

  it("maps the south-east corner (maxX, minY) to the bottom-right", () => {
    const p = worldToUv(WORLD.maxX, WORLD.minY);
    expect(p.u).toBeCloseTo(1);
    expect(p.v).toBeCloseTo(1);
  });

  it("maps the world centre to the map centre", () => {
    const p = worldToUv(-123888, 158000);
    expect(p.u).toBeCloseTo(0.5);
    expect(p.v).toBeCloseTo(0.5);
  });

  it("clamps out-of-world coordinates into [0,1]", () => {
    const p = worldToUv(9_000_000, -9_000_000);
    expect(p.u).toBe(1);
    expect(p.v).toBe(1);
  });
});

describe("worldToPaldex", () => {
  it("maps the world centre to (0, 0)", () => {
    expect(worldToPaldex(-123888, 158000)).toEqual({ x: 0, y: 0 });
  });

  it("maps the south-east extreme to (1000, 1000)", () => {
    expect(worldToPaldex(WORLD.maxX, WORLD.maxY)).toEqual({ x: 1000, y: 1000 });
  });
});
