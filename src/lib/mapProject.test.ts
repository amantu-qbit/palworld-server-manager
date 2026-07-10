import { describe, expect, it } from "vitest";
import type { Bounds } from "./mapProject";
import { computeBounds, projectToRadar } from "./mapProject";

const SIZE = 560;

describe("computeBounds", () => {
  it("returns a safe default for an empty actor list", () => {
    expect(computeBounds([])).toEqual({ minX: -500000, maxX: 500000, minY: -500000, maxY: 500000 });
  });

  it("returns a safe default when every actor shares one point (zero span)", () => {
    const b = computeBounds([
      { LocationX: 100, LocationY: 100 },
      { LocationX: 100, LocationY: 100 },
    ]);
    expect(b).toEqual({ minX: -500000, maxX: 500000, minY: -500000, maxY: 500000 });
  });

  it("pads the min/max span outward on both axes", () => {
    const b = computeBounds(
      [
        { LocationX: 0, LocationY: 0 },
        { LocationX: 100, LocationY: 200 },
      ],
      0.1,
    );
    expect(b.minX).toBeLessThan(0);
    expect(b.maxX).toBeGreaterThan(100);
    expect(b.minY).toBeLessThan(0);
    expect(b.maxY).toBeGreaterThan(200);
  });
});

describe("projectToRadar", () => {
  const b = computeBounds(
    [
      { LocationX: -1000, LocationY: -1000 },
      { LocationX: 1000, LocationY: 1000 },
    ],
    0,
  );

  it("maps the center of the bounds near size/2 on both axes", () => {
    const cx = (b.minX + b.maxX) / 2;
    const cy = (b.minY + b.maxY) / 2;
    const p = projectToRadar(cx, cy, b, SIZE);
    expect(p.x).toBeCloseTo(SIZE / 2);
    expect(p.y).toBeCloseTo(SIZE / 2);
  });

  it("keeps a corner within [0, size]", () => {
    const p = projectToRadar(b.minX, b.maxY, b, SIZE);
    expect(p.x).toBeGreaterThanOrEqual(0);
    expect(p.x).toBeLessThanOrEqual(SIZE);
    expect(p.y).toBeGreaterThanOrEqual(0);
    expect(p.y).toBeLessThanOrEqual(SIZE);
  });

  it("returns finite, in-range values for zero-span bounds", () => {
    const zero: Bounds = { minX: 5, maxX: 5, minY: 5, maxY: 5 };
    const p = projectToRadar(5, 5, zero, SIZE);
    expect(Number.isFinite(p.x)).toBe(true);
    expect(Number.isFinite(p.y)).toBe(true);
    expect(p.x).toBeGreaterThanOrEqual(0);
    expect(p.x).toBeLessThanOrEqual(SIZE);
    expect(p.y).toBeGreaterThanOrEqual(0);
    expect(p.y).toBeLessThanOrEqual(SIZE);
  });
});
