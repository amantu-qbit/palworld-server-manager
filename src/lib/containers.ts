/**
 * Pure helpers for the Storage screen: bag labels/ordering, grouping the flat
 * `GET /v1/containers` list into per-player and per-guild sections, and the
 * shrink-deletion computation the resize dialog warns about.
 */
import type { ContainerInfo, ContainerKind, ItemContainerSlot } from "../types/bridge";

/** Display label + rail order for each player bag kind. */
const BAG: Record<
  Exclude<ContainerKind, "guild_chest" | "base_storage">,
  { label: string; order: number }
> = {
  common: { label: "Inventory", order: 0 },
  essential: { label: "Key Items", order: 1 },
  weapon_loadout: { label: "Weapons", order: 2 },
  player_equip_armor: { label: "Armor", order: 3 },
  food_equip: { label: "Food", order: 4 },
};

/** Rail/header label for a container. */
export function containerLabel(c: ContainerInfo): string {
  if (c.kind === "guild_chest") {
    return c.guild_name ? `Guild Chest — ${c.guild_name}` : "Guild Chest";
  }
  if (c.kind === "base_storage") {
    return c.guild_name ? `Base Storage — ${c.guild_name}` : "Base Storage";
  }
  return BAG[c.kind]?.label ?? c.label;
}

/** Who a container belongs to, for header chips ("PlayerName" / guild name). */
export function containerOwner(c: ContainerInfo): string {
  if (c.kind === "guild_chest" || c.kind === "base_storage") {
    return c.guild_name ?? "Unknown guild";
  }
  return c.owner_name ?? "Unknown player";
}

export interface ContainerGroup {
  /** Section heading: player nickname, or "Guilds" for the chest section. */
  title: string;
  containers: ContainerInfo[];
}

/**
 * Group containers for the left rail: one section per player (their five bags
 * in fixed Inventory→Food order), then a single "Guilds" section with every
 * guild chest. Players sort by name; unknown kinds keep their server order.
 */
export function groupContainers(containers: ContainerInfo[]): ContainerGroup[] {
  const players = new Map<string, { title: string; containers: ContainerInfo[] }>();
  const guilds: ContainerInfo[] = [];
  for (const c of containers) {
    if (c.kind === "guild_chest") {
      guilds.push(c);
      continue;
    }
    // Base-storage chests are edited in the Guilds screen, not the player rail.
    if (c.kind === "base_storage") {
      continue;
    }
    const key = c.owner_uid ?? c.owner_name ?? c.id;
    let g = players.get(key);
    if (!g) {
      g = { title: c.owner_name ?? "Unknown player", containers: [] };
      players.set(key, g);
    }
    g.containers.push(c);
  }
  const order = (c: ContainerInfo) =>
    c.kind === "guild_chest" || c.kind === "base_storage" ? 99 : (BAG[c.kind]?.order ?? 98);
  const groups = [...players.values()].sort((a, b) => a.title.localeCompare(b.title));
  for (const g of groups) g.containers.sort((a, b) => order(a) - order(b));
  guilds.sort((a, b) => (a.guild_name ?? "").localeCompare(b.guild_name ?? ""));
  if (guilds.length) groups.push({ title: "Guilds", containers: guilds });
  return groups;
}

/**
 * Slots that shrinking a container to `slotNum` would delete: every occupied
 * slot with `slot_index >= slotNum` (mirrors the bridge's shrink semantics,
 * ported from palworld-save-pal PR #299).
 */
export function slotsDeletedByResize(
  slots: ItemContainerSlot[],
  slotNum: number,
): ItemContainerSlot[] {
  return slots
    .filter((s) => s.static_id && s.static_id !== "None" && s.count > 0 && s.slot_index >= slotNum)
    .sort((a, b) => a.slot_index - b.slot_index);
}

/** Occupied slots of a container (what "Clear container" will remove). */
export function occupiedSlots(slots: ItemContainerSlot[]): ItemContainerSlot[] {
  return slots.filter((s) => s.static_id && s.static_id !== "None" && s.count > 0);
}
