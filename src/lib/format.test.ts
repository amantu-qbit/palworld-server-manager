import { describe, expect, it } from "vitest";
import { formatInt, formatUptime, humanLabel, uptimeParts } from "./format";

describe("formatUptime", () => {
  it("shows days + hours for multi-day uptime", () => {
    expect(formatUptime(4 * 86400 + 12 * 3600)).toBe("4d 12h");
  });
  it("shows hours + minutes under a day", () => {
    expect(formatUptime(5 * 3600 + 9 * 60)).toBe("5h 9m");
  });
  it("shows minutes + seconds under an hour", () => {
    expect(formatUptime(90)).toBe("1m 30s");
  });
  it("shows seconds under a minute", () => {
    expect(formatUptime(42)).toBe("42s");
  });
  it("never goes negative", () => {
    expect(formatUptime(-5)).toBe("0s");
  });
});

describe("uptimeParts", () => {
  it("splits into value/unit pairs", () => {
    expect(uptimeParts(4 * 86400 + 12 * 3600)).toEqual([
      { value: 4, unit: "d" },
      { value: 12, unit: "h" },
    ]);
  });
});

describe("formatInt", () => {
  it("adds thousands separators", () => {
    expect(formatInt(3000)).toBe("3,000");
  });
});

describe("humanLabel", () => {
  it("drops the boolean b-prefix and spaces camelCase", () => {
    expect(humanLabel("bEnableFastTravel")).toBe("Enable Fast Travel");
  });
  it("handles rate keys", () => {
    expect(humanLabel("PalCaptureRate")).toBe("Pal Capture Rate");
  });
  it("keeps underscores as separators", () => {
    expect(humanLabel("DropItemMaxNum_UNKO")).toBe("Drop Item Max Num UNKO");
  });
});
