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

  it("resolves a bare code name (keeps boss/static markers working)", () => {
    expect(palIconKey("GrassMammoth")).toBe("grassmammoth");
    expect(palIconKey("BlueDragon")).toBe("bluedragon");
  });

  it("unwraps UE blueprint classes and object paths", () => {
    expect(palIconKey("BP_BlueDragon_C")).toBe("bluedragon");
    expect(palIconKey("BP_FlowerDoll_C")).toBe("flowerdoll");
    expect(
      palIconKey("/Game/Pal/Blueprints/Monster/BlueDragon/BP_BlueDragon.BP_BlueDragon_C"),
    ).toBe("bluedragon");
  });

  it("peels power-tier variants (_BOSS, _MiddleBoss) to the base creature icon", () => {
    // Confirmed from live server Actor.Class values.
    expect(palIconKey("BP_Manticore_BOSS_C")).toBe("manticore"); // Blazehowl (alpha)
    expect(palIconKey("BP_RedArmorBird_BOSS_C")).toBe("redarmorbird"); // Ragnahawk (alpha)
    expect(palIconKey("BP_Anubis_MiddleBoss_C")).toBe("anubis"); // Anubis (mini-boss party Pal)
  });

  it("keeps genuinely distinct variant icons instead of over-stripping", () => {
    // These are their own icon files — must not collapse to a base.
    expect(palIconKey("BP_Human_GrassBoss_C")).toBe("human_grassboss");
    expect(palIconKey("BP_FlowerRabbit_Quest_C")).toBe("flowerrabbit_quest");
  });

  it("degrades unknown species to a stable no-icon key (a dot, never the wrong Pal)", () => {
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
