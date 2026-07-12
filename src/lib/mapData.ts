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
  /** present for live actors from the game-data snapshot */
  actor?: Actor;
  /** for player markers: currently connected? */
  online?: boolean;
}

interface KindInfo {
  label: string;
  color: string;
  group: "live" | "landmark";
  /** icon served from public/mapicons; live pals draw as dots instead */
  icon?: string;
  /** default layer visibility */
  on: boolean;
}

// Order here drives the layer-panel order.
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
  .map((o, i) =>
    landmark(
      `bs-${i}`,
      "boss",
      o.x,
      o.y,
      o.pal ? spaceWords(o.pal) : "Boss Pal",
      o.type === "predator_pal" ? "Predator" : "Field Alpha",
    ),
  );

/** All bundled static landmarks (fast travel, dungeons, effigies, bosses). */
export const LANDMARK_MARKERS: MapMarker[] = [...fastTravel, ...dungeons, ...bosses, ...effigies];

/** Convert a live game-data actor into a marker. */
export function actorToMarker(a: Actor, i: number, onlineIds: Set<string>): MapMarker {
  const { u, v } = worldToUv(a.LocationX, a.LocationY);
  const kind = ACTOR_KIND[a.UnitType] ?? "wildpal";
  const online =
    kind === "player"
      ? a.userid
        ? onlineIds.has(a.userid)
        : a.IsActive !== "false"
      : undefined;
  return {
    id: a.InstanceID || `${a.UnitType}-${i}`,
    kind,
    u,
    v,
    x: a.LocationX,
    y: a.LocationY,
    name: a.NickName || a.Class || a.UnitType,
    sub: a.GuildName,
    actor: a,
    online,
  };
}
