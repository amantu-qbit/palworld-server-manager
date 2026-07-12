import type { Actor } from "../types/api";
import { worldToUv } from "./mapProject";
import ftRaw from "../data/mapdata/fast_travel.json";
import effigyRaw from "../data/mapdata/effigies.json";
import objRaw from "../data/mapdata/map_objects.json";

export type MarkerKind =
  | "player"
  | "otomopal"
  | "basepal"
  | "wildpal"
  | "npc"
  | "fasttravel"
  | "dungeon"
  | "effigy"
  | "boss";

export interface MapMarker {
  id: string;
  kind: MarkerKind;
  /** normalised map position, precomputed */
  u: number;
  v: number;
  /** world coords */
  x: number;
  y: number;
  name: string;
  sub?: string;
  /** lowercased pal key for boss markers → /mapicons/pals/{palKey}.webp */
  palKey?: string;
  /** present for live actors from the game-data snapshot */
  actor?: Actor;
  /** for player markers: currently connected? */
  online?: boolean;
}

interface KindInfo {
  label: string;
  color: string;
  group: "live" | "landmark";
  icon?: string;
  on: boolean;
}

export const KIND_META: Record<MarkerKind, KindInfo> = {
  player: { label: "Players", color: "#34d3ea", group: "live", icon: "/mapicons/player.webp", on: true },
  otomopal: { label: "Party Pals", color: "#4cc2f0", group: "live", on: true },
  basepal: { label: "Base Pals", color: "#3ad19a", group: "live", on: true },
  wildpal: { label: "Wild Pals", color: "#aab2c0", group: "live", on: true },
  npc: { label: "NPCs", color: "#e6b450", group: "live", on: false },
  fasttravel: { label: "Fast Travel", color: "#7fe3f0", group: "landmark", icon: "/mapicons/fasttravel.webp", on: true },
  dungeon: { label: "Dungeons", color: "#c58bf0", group: "landmark", icon: "/mapicons/dungeon.webp", on: true },
  boss: { label: "Boss Pals", color: "#ec6a6a", group: "landmark", icon: "/mapicons/boss.webp", on: true },
  effigy: { label: "Effigies", color: "#8fe388", group: "landmark", icon: "/mapicons/effigy.webp", on: false },
};

export const MARKER_ORDER = Object.keys(KIND_META) as MarkerKind[];

const ACTOR_KIND: Record<string, MarkerKind> = {
  Player: "player",
  WildPal: "wildpal",
  BaseCampPal: "basepal",
  OtomoPal: "otomopal",
  NPC: "npc",
};

const spaceWords = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, "$1 $2");

/**
 * Normalise a Palworld character id/class to its icon key (matches the bundled
 * /mapicons/pals/{key}.webp files). Ported from palworld-save-pal.
 */
export function cleanseCharacterId(id: string): string {
  return id
    .toLowerCase()
    .replace("predator_", "")
    .replace("_oilrig", "")
    .replace("raid_", "")
    .replace("summon_", "")
    .replace("_max", "")
    .replace(/_\d+$/, "")
    .replace("boss_", "")
    .replace("quest_farmer03_", "")
    .replace("_otomo", "");
}

const PAL_KINDS = new Set<MarkerKind>(["wildpal", "basepal", "otomopal"]);

interface RawPoint {
  x: number;
  y: number;
  z?: number;
  id?: string;
  localized_name?: string;
}
interface RawObj {
  x: number;
  y: number;
  type: string;
  pal?: string;
}

function landmark(id: string, kind: MarkerKind, x: number, y: number, name: string, sub?: string): MapMarker {
  const { u, v } = worldToUv(x, y);
  return { id, kind, u, v, x, y, name, sub };
}

const fastTravel = Object.entries(ftRaw as Record<string, RawPoint>).map(([g, p]) =>
  landmark(`ft-${g}`, "fasttravel", p.x, p.y, p.localized_name || "Fast Travel Point"),
);
const effigies = Object.entries(effigyRaw as Record<string, RawPoint>).map(([g, p]) =>
  landmark(`ef-${g}`, "effigy", p.x, p.y, "Lifmunk Effigy"),
);
const objs = objRaw as RawObj[];
const dungeons = objs
  .filter((o) => o.type === "dungeon")
  .map((o, i) => landmark(`dg-${i}`, "dungeon", o.x, o.y, "Dungeon"));
const bosses = objs
  .filter((o) => o.type === "alpha_pal" || o.type === "predator_pal")
  .map((o, i) => {
    const m = landmark(
      `bs-${i}`,
      "boss",
      o.x,
      o.y,
      o.pal ? spaceWords(o.pal) : "Boss Pal",
      o.type === "predator_pal" ? "Predator" : "Field Alpha",
    );
    if (o.pal) m.palKey = cleanseCharacterId(o.pal);
    return m;
  });

/** All bundled static landmarks. */
export const LANDMARK_MARKERS: MapMarker[] = [...fastTravel, ...dungeons, ...bosses, ...effigies];

/** Unique boss pal keys, for icon preloading. */
export const BOSS_PAL_KEYS: string[] = [...new Set(bosses.map((b) => b.palKey).filter(Boolean) as string[])];

/**
 * Bounding box (normalised) of the actual playable content — the islands — so the
 * default view fills the viewport with land instead of empty ocean.
 */
function bbox(ms: MapMarker[]) {
  let uMin = 1;
  let uMax = 0;
  let vMin = 1;
  let vMax = 0;
  for (const m of ms) {
    if (m.u < uMin) uMin = m.u;
    if (m.u > uMax) uMax = m.u;
    if (m.v < vMin) vMin = m.v;
    if (m.v > vMax) vMax = m.v;
  }
  return { uMin, uMax, vMin, vMax };
}
const _b = bbox([...fastTravel, ...dungeons, ...bosses]);
const PAD = 0.02;
export const CONTENT = {
  uMin: Math.max(0, _b.uMin - PAD),
  uMax: Math.min(1, _b.uMax + PAD),
  vMin: Math.max(0, _b.vMin - PAD),
  vMax: Math.min(1, _b.vMax + PAD),
};

/**
 * Convert a live actor into a marker. A player is "online" if it matches the live
 * /players list (by userId or name) — offline pawns that linger in the snapshot don't.
 */
export function actorToMarker(a: Actor, i: number, onlineKeys: Set<string>): MapMarker {
  const { u, v } = worldToUv(a.LocationX, a.LocationY);
  const kind = ACTOR_KIND[a.UnitType] ?? "wildpal";
  let online: boolean | undefined;
  if (kind === "player") {
    online =
      onlineKeys.size === 0
        ? true
        : onlineKeys.has(a.userid ?? "\0") || onlineKeys.has((a.NickName ?? "").toLowerCase());
  }
  const palKey = PAL_KINDS.has(kind) && a.Class ? cleanseCharacterId(a.Class) : undefined;
  return {
    id: a.InstanceID || `${a.UnitType}-${i}`,
    kind,
    u,
    v,
    x: a.LocationX,
    y: a.LocationY,
    name: a.NickName || (a.Class ? spaceWords(a.Class) : a.UnitType),
    sub: a.GuildName,
    palKey,
    actor: a,
    online,
  };
}
