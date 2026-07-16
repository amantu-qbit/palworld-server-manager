import { describe, expect, it } from "vitest";
import {
  containerLabel,
  containerOwner,
  groupContainers,
  occupiedSlots,
  slotsDeletedByResize,
} from "./containers";
import type { ContainerInfo, ContainerKind, ItemContainerSlot } from "../types/bridge";

const slot = (slot_index: number, static_id = "Wood", count = 1): ItemContainerSlot => ({
  slot_index,
  count,
  static_id,
  dynamic_item: null,
});

const container = (over: Partial<ContainerInfo> & { kind: ContainerKind }): ContainerInfo => ({
  id: `${over.kind}-${over.owner_uid ?? over.guild_id ?? "x"}`,
  label: "",
  owner_uid: null,
  owner_name: null,
  guild_id: null,
  guild_name: null,
  slot_num: 42,
  used: 0,
  slots: [],
  ...over,
});

describe("slotsDeletedByResize", () => {
  const slots = [slot(0), slot(3, "Stone", 55), slot(7, "None", 0), slot(9, "Wood", 0), slot(12)];

  it("keeps everything when growing or matching the highest occupied slot", () => {
    expect(slotsDeletedByResize(slots, 13)).toEqual([]);
    expect(slotsDeletedByResize(slots, 9999)).toEqual([]);
  });

  it("flags only occupied slots at or past the new size", () => {
    const doomed = slotsDeletedByResize(slots, 4);
    expect(doomed.map((s) => s.slot_index)).toEqual([12]);
  });

  it("ignores cleared slots (\"None\" or zero count)", () => {
    const doomed = slotsDeletedByResize(slots, 0);
    expect(doomed.map((s) => s.slot_index)).toEqual([0, 3, 12]);
  });

  it("returns the doomed slots ordered by index", () => {
    const shuffled = [slot(12), slot(0), slot(3)];
    expect(slotsDeletedByResize(shuffled, 0).map((s) => s.slot_index)).toEqual([0, 3, 12]);
  });
});

describe("occupiedSlots", () => {
  it("filters empty, cleared, and zero-count slots", () => {
    const occ = occupiedSlots([slot(0), slot(1, "None", 5), slot(2, "Stone", 0), slot(3, "Stone", 2)]);
    expect(occ.map((s) => s.slot_index)).toEqual([0, 3]);
  });
});

describe("container labels", () => {
  it("maps the five player bag kinds to fixed labels", () => {
    expect(containerLabel(container({ kind: "common" }))).toBe("Inventory");
    expect(containerLabel(container({ kind: "essential" }))).toBe("Key Items");
    expect(containerLabel(container({ kind: "weapon_loadout" }))).toBe("Weapons");
    expect(containerLabel(container({ kind: "player_equip_armor" }))).toBe("Armor");
    expect(containerLabel(container({ kind: "food_equip" }))).toBe("Food");
  });

  it("names guild chests after their guild", () => {
    expect(containerLabel(container({ kind: "guild_chest", guild_name: "Night Raid" }))).toBe(
      "Guild Chest — Night Raid",
    );
    expect(containerLabel(container({ kind: "guild_chest" }))).toBe("Guild Chest");
  });

  it("resolves the owner for chips", () => {
    expect(containerOwner(container({ kind: "common", owner_name: "Riko" }))).toBe("Riko");
    expect(containerOwner(container({ kind: "guild_chest", guild_name: "Night Raid" }))).toBe("Night Raid");
    expect(containerOwner(container({ kind: "common" }))).toBe("Unknown player");
  });
});

describe("groupContainers", () => {
  const list: ContainerInfo[] = [
    container({ kind: "food_equip", owner_uid: "u1", owner_name: "Riko" }),
    container({ kind: "common", owner_uid: "u1", owner_name: "Riko" }),
    container({ kind: "guild_chest", guild_id: "g1", guild_name: "Night Raid" }),
    container({ kind: "common", owner_uid: "u2", owner_name: "Ash" }),
    container({ kind: "weapon_loadout", owner_uid: "u1", owner_name: "Riko" }),
  ];

  it("groups per player (sorted by name) with the Guilds section last", () => {
    const groups = groupContainers(list);
    expect(groups.map((g) => g.title)).toEqual(["Ash", "Riko", "Guilds"]);
  });

  it("orders each player's bags Inventory → Food", () => {
    const riko = groupContainers(list).find((g) => g.title === "Riko")!;
    expect(riko.containers.map((c) => containerLabel(c))).toEqual(["Inventory", "Weapons", "Food"]);
  });

  it("omits the Guilds section when there are no chests", () => {
    const groups = groupContainers(list.filter((c) => c.kind !== "guild_chest"));
    expect(groups.some((g) => g.title === "Guilds")).toBe(false);
  });
});
