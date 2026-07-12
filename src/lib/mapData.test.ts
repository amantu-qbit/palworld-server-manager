import { describe, it, expect } from "vitest";
import { palIconKey, actorToMarker, cleanseCharacterId } from "./mapData";
import type { Actor } from "../types/api";

describe("palIconKey", () => {
  it("maps localized display names (what the /game-data Class field sends) to icon keys", () => {
    // These are the values a live server actually reports in Actor.Class.
    expect(palIconKey("Petallia")).toBe("flowerdoll");
    expect(palIconKey("Blazehowl")).toBe("manticore");
    expect(palIconKey("Lamball")).toBe("sheepball");
    expect(palIconKey("Cattiva")).toBe("pinkcat");
    expect(palIconKey("Jetragon")).toBe("jetdragon");
  });

  it("handles suffixed variants distinctly", () => {
    expect(palIconKey("Kitsun")).toBe("amaterasuwolf");
    expect(palIconKey("Kitsun Noct")).toBe("amaterasuwolf_dark");
  });

  it("falls back to the cleansed id for code-name style / unknown values", () => {
    // A raw code name still resolves (keeps boss/static markers working).
    expect(palIconKey("GrassMammoth")).toBe("grassmammoth");
    // An unknown species degrades to a stable key (renders as a dot, no crash).
    expect(palIconKey("TotallyNewPal9000")).toBe(cleanseCharacterId("TotallyNewPal9000"));
  });
});

describe("actorToMarker", () => {
  const base: Actor = {
    Type: "Character",
    InstanceID: "abc",
    UnitType: "BaseCampPal",
    NickName: "",
    Class: "Petallia",
    LocationX: 0,
    LocationY: 0,
    LocationZ: 0,
  };

  it("sets palKey from the display-name Class so live Pals get real icons", () => {
    const m = actorToMarker(base, 0, new Set());
    expect(m.kind).toBe("basepal");
    expect(m.palKey).toBe("flowerdoll");
  });
});
