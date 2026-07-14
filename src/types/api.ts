/**
 * Shared types for the Palworld REST API (v1).
 * Base URL: http://<host>:<port>/v1/api  ·  HTTP Basic auth (admin / AdminPassword)
 */

export interface ServerInfo {
  version: string;
  servername: string;
  description: string;
  worldguid: string;
}

export interface Metrics {
  serverfps: number;
  currentplayernum: number;
  serverframetime: number;
  maxplayernum: number;
  uptime: number; // seconds
}

export interface Player {
  name: string;
  accountName: string;
  playerId: string;
  userId: string;
  ip: string;
  ping: number;
  location_x: number;
  location_y: number;
  level: number;
  building_count: number;
}

export interface PlayersResponse {
  players: Player[];
}

/** ~60 documented keys; values are string | number | boolean. See lib/settingsSchema. */
export type Settings = Record<string, string | number | boolean>;

export type ActorUnitType =
  | "Player"
  | "OtomoPal"
  | "BaseCampPal"
  | "WildPal"
  | "NPC"
  | (string & {});

export interface Actor {
  Type: string;
  InstanceID: string;
  UnitType: ActorUnitType;
  NickName?: string;
  TrainerInstanceID?: string;
  TrainerNickName?: string;
  TrainerClass?: string;
  userid?: string;
  ip?: string;
  level?: number;
  HP?: number;
  MaxHP?: number;
  GuildID?: string;
  GuildName?: string;
  Class?: string;
  Action?: string;
  AI_Action?: string;
  LocationX: number;
  LocationY: number;
  LocationZ: number;
  RotationX?: number;
  RotationY?: number;
  RotationZ?: number;
  Stage?: string;
  IsActive?: string;
}

export interface GameData {
  Time: string;
  FPS: number;
  AverageFPS: number;
  ActorData: Actor[];
}

export interface Connection {
  host: string;
  port: number;
  password: string; // username is always "admin"
  /** Optional Tier-2 bridge (psm-bridge.exe) port. Absence ⇒ Tier 1 only. */
  bridgePort?: number;
  /** Optional Tier-2 bridge Bearer token. */
  bridgeToken?: string;
}

export interface ActionResult {
  ok: boolean;
  message?: string;
}
