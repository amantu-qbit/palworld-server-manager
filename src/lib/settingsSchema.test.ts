import { describe, expect, it } from "vitest";
import { groupOf, labelFor } from "./settingsSchema";

describe("groupOf", () => {
  it("maps pal keys to Pals", () => {
    expect(groupOf("PalCaptureRate")).toBe("Pals");
  });
  it("maps PvP keys to PvP & Guild", () => {
    expect(groupOf("bIsPvP")).toBe("PvP & Guild");
  });
  it("maps network keys to Server & Network", () => {
    expect(groupOf("RESTAPIPort")).toBe("Server & Network");
  });
  it("falls back to Misc for unknown keys", () => {
    expect(groupOf("TotallyUnknownKey")).toBe("Misc");
  });
});

describe("labelFor", () => {
  it("humanizes the key", () => {
    expect(labelFor("PalCaptureRate")).toBe("Pal Capture Rate");
  });
});
