import { describe, expect, it } from "vitest";
import { clamp, dashoffset } from "./gauge";

describe("dashoffset", () => {
  const C = 515;
  it("is the full circumference at value 0 (empty ring)", () => {
    expect(dashoffset(0, 60, C)).toBe(C);
  });
  it("is 0 at value == max (full ring)", () => {
    expect(dashoffset(60, 60, C)).toBe(0);
  });
  it("is half the circumference at the midpoint", () => {
    expect(dashoffset(30, 60, C)).toBeCloseTo(C / 2);
  });
  it("clamps values above max", () => {
    expect(dashoffset(120, 60, C)).toBe(0);
  });
  it("clamps negative values", () => {
    expect(dashoffset(-10, 60, C)).toBe(C);
  });
  it("handles a zero max without dividing by zero", () => {
    expect(dashoffset(5, 0, C)).toBe(C);
  });
});

describe("clamp", () => {
  it("bounds within range", () => {
    expect(clamp(5, 0, 10)).toBe(5);
    expect(clamp(-1, 0, 10)).toBe(0);
    expect(clamp(11, 0, 10)).toBe(10);
  });
});
