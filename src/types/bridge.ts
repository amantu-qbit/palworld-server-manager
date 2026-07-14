/**
 * Types for the PSM Bridge Tier-2 REST API (`http://<host>:<port>/v1`, Bearer auth).
 * Field names are snake_case to match the bridge's serde model exactly
 * (bridge/src/save/model.rs). Phase 1 is read-only.
 */

export interface BridgeHealth {
  version: string;
  capabilities: string[];
  save_detected: boolean;
  writes_enabled: boolean;
}

/** `GET /v1/server/status` — process-supervisor state. */
export interface ServerStatus {
  /** Whether `[server_process]` is configured in bridge.toml. */
  configured: boolean;
  /** Whether a supervised server process is currently running. */
  running: boolean;
  pid: number | null;
  uptime_secs: number | null;
}

export interface PlayerSummary {
  uid: string;
  nickname: string;
  level: number;
  guild_id: string | null;
  pal_count: number;
  last_online: string | null;
}

export interface Pal {
  instance_id: string;
  owner_uid: string;
  character_id: string;
  nickname: string;
  gender: string;
  is_lucky: boolean;
  is_boss: boolean;
  is_tower: boolean;
  storage_id: string;
  storage_slot: number;
  level: number;
  exp: number;
  rank: number;
  rank_hp: number;
  rank_attack: number;
  rank_defense: number;
  rank_craftspeed: number;
  talent_hp: number;
  talent_shot: number;
  talent_defense: number;
  hp: number;
  max_hp: number;
  sanity: number;
  stomach: number;
  learned_skills: string[];
  active_skills: string[];
  passive_skills: string[];
  work_suitability: Record<string, number>;
  friendship_point: number;
  group_id: string;
}

export interface EggParams {
  steps_remaining: number;
}

export interface DynamicItem {
  local_id: string;
  item_type: string;
  durability: number;
  passive_skill_list: string[];
  remaining_bullets: number;
  egg_params: EggParams | null;
}

export interface ItemContainerSlot {
  slot_index: number;
  count: number;
  static_id: string;
  dynamic_item: DynamicItem | null;
}

export interface ItemContainer {
  id: string;
  container_type: string;
  key: string;
  slot_num: number;
  slots: ItemContainerSlot[];
}

export interface PlayerDetail {
  summary: PlayerSummary;
  level: number;
  exp: number;
  /** Stat-point allocations, e.g. { MaxHP: 3, MaxSP: 2, ... }. */
  status_points: Record<string, number>;
  ext_status_points: Record<string, number>;
  /** Unlocked technology codes (resolve names with src/data/techNames.json). */
  technologies: string[];
  technology_points: number;
  boss_technology_points: number;
  /** Container ids so a pal's location (party / box / base) can be labelled. */
  pal_box_container: string;
  party_container: string;
  pals: Pal[];
  inventory: ItemContainer[];
}

export interface Base {
  id: string;
  name: string;
  area_range: number;
  storage_containers: string[];
  pals: string[];
}

export interface Guild {
  id: string;
  name: string;
  base_camp_level: number;
  guild_chest: ItemContainer | null;
  lab_research: string[];
  bases: Base[];
  players: string[];
  admin_player_uid: string;
}

/** id → display-name map from `GET /v1/reference/{catalog}`. */
export type ReferenceCatalog = Record<string, string>;
