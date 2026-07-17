/**
 * Types for the PSM Bridge Tier-2 REST API (`http://<host>:<port>/v1`, Bearer auth).
 * Field names are snake_case to match the bridge's serde model exactly
 * (bridge/src/save/model.rs). Phase 2 adds labeled containers + save writes.
 */

export interface BridgeHealth {
  version: string;
  capabilities: string[];
  save_detected: boolean;
  writes_enabled: boolean;
  /** True when the game server process is detected (writes are blocked).
   *  Absent on pre-Phase-2 bridges — treat missing as false. */
  server_running?: boolean;
}

/** `GET /v1/server/status` — process-supervisor state. */
export interface ServerStatus {
  /** Whether `[server_process]` is configured in bridge.toml. */
  configured: boolean;
  /** Whether a server process is running (launched by the bridge, or
   * detected by image name after a bridge restart / external launch). */
  running: boolean;
  pid: number | null;
  uptime_secs: number | null;
  /** True when the running server wasn't launched by this bridge instance
   * (detected by name; still fully stoppable/restartable). Optional: older
   * bridges omit it. */
  adopted?: boolean;
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
  /** Build-area radius (base camp `area_range`; vanilla default 3500). */
  area_range: number;
  /** World position (transform translation x/y/z), when decoded. */
  position?: [number, number, number] | null;
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

/** Labeled item-container kinds surfaced by `GET /v1/containers`. */
export type ContainerKind =
  | "common"
  | "essential"
  | "weapon_loadout"
  | "player_equip_armor"
  | "food_equip"
  | "guild_chest"
  | "base_storage";

/** One labeled container (player bag, guild chest, or base storage chest). */
export interface ContainerInfo {
  id: string;
  kind: ContainerKind;
  label: string;
  owner_uid: string | null;
  owner_name: string | null;
  guild_id: string | null;
  guild_name: string | null;
  slot_num: number;
  /** Vanilla default slot count for this kind, when reliably known (so the UI
   *  can flag a container resized off its default). Absent when it varies. */
  default_slot_num?: number;
  used: number;
  slots: ItemContainerSlot[];
}

export interface ContainersResponse {
  containers: ContainerInfo[];
}

/** Minimum success payload of every save write; `backup` is the snapshot path (absent for a no-op). */
export interface WriteResult {
  ok: boolean;
  backup?: string;
}

/** Container writes echo the updated container back. */
export interface ContainerWriteResult extends WriteResult {
  container: ContainerInfo;
}

/** `POST /v1/pals/{id}/clone` echoes the new copy's instance id. */
export interface CloneResult extends WriteResult {
  instance_id: string;
}

/** `POST /v1/players/{id}/map` body — per-player map/progression unlocks. */
export interface PlayerMapBody {
  unlock_all_fast_travel?: boolean;
}

/** `POST /v1/bases/{id}/pals/edit` body — same edit applied to every base pal. */
export interface EditBasePalsBody {
  level?: number;
  exp?: number;
  /** Work-suitability ranks (0–5) to set on every base pal. */
  work_suitability?: Record<string, number>;
}

/** `POST /v1/guilds/{id}/edit` body — set guild name and/or base-camp level. */
export interface EditGuildBody {
  guild_name?: string;
  base_camp_level?: number;
}

/** `POST /v1/bases/{id}/edit` body — set base build-area radius and/or name. */
export interface EditBaseBody {
  area_range?: number;
  name?: string;
}

/** `POST /v1/players/{uid}/edit` body — all fields optional.
 *  `status_points` / `ext_status_points` keys must be the exact on-disk names
 *  previously returned by `GET /v1/players/{uid}` (often Japanese). */
export interface EditPlayerBody {
  level?: number;
  exp?: number;
  status_points?: Record<string, number>;
  ext_status_points?: Record<string, number>;
}

/** `POST /v1/players/{uid}/technologies` body — all fields optional. */
export interface EditPlayerTechnologiesBody {
  unlock?: string[];
  relock?: string[];
  technology_point?: number;
  boss_technology_point?: number;
}

/** `POST /v1/pals/{instance_id}/edit` body — all fields optional; list fields
 *  replace wholesale. `active_skills` are `EPalWazaID::…` codes. */
export interface EditPalBody {
  level?: number;
  exp?: number;
  nickname?: string;
  passive_skills?: string[];
  active_skills?: string[];
  learned_skills?: string[];
  rank?: number;
  rank_hp?: number;
  rank_attack?: number;
  rank_defense?: number;
  rank_craftspeed?: number;
  talent_hp?: number;
  talent_shot?: number;
  talent_defense?: number;
  gender?: "Male" | "Female";
  /** `EPalWorkSuitability::…` code → rank 0..=5. Existing codes are updated
   *  in place; codes the pal doesn't have yet are added. */
  work_suitability?: Record<string, number>;
}

/** id → display-name map from `GET /v1/reference/{catalog}`. */
export type ReferenceCatalog = Record<string, string>;

/** One `.sav` file discovered under the bridge's save dir. */
export interface SavFileInfo {
  name: string;
  rel_path: string;
  size_bytes: number;
}

/**
 * A node in the generic GVAS tree projection (bridge `save::debug`). Objects
 * carry a `_type`; containers carry `_count` and either `items`/fields or
 * `_collapsed`; byte blobs are `{ _bytes: number }`.
 */
export type SavNode = string | number | boolean | null | SavNode[] | { [k: string]: SavNode };

/** `GET /v1/debug/savtree` — one bounded subtree of a decoded `.sav`. */
export interface SavTreeResponse {
  file: string;
  path: string;
  node: SavNode;
  meta: { size_bytes: number };
}
