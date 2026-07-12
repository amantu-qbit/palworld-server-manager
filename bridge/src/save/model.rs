//! Read-model DTO structs for the save-game data.
//! All structs serialize/deserialize with snake_case field names.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A Pal (catchable creature) in the game world.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Pal {
    /// Unique instance ID for this Pal.
    pub instance_id: String,
    /// The player UID who owns this Pal (empty if uncaught/wild).
    pub owner_uid: String,
    /// Species/character ID (e.g., "SheepBall").
    pub character_id: String,
    /// User-assigned nickname.
    pub nickname: String,
    /// Gender (e.g., "Male", "Female", or value of EPalGenderType).
    pub gender: String,
    /// Whether this Pal is a rare/shiny variant.
    pub is_lucky: bool,
    /// Whether this Pal is a boss.
    pub is_boss: bool,
    /// Whether this Pal is tower-related.
    pub is_tower: bool,
    /// Container ID where this Pal is stored.
    pub storage_id: String,
    /// Slot index within the container.
    pub storage_slot: i32,
    /// Current level.
    pub level: i32,
    /// Experience points.
    pub exp: i32,
    /// Condenser rank (1–5).
    pub rank: i32,
    /// Rank HP soul count.
    pub rank_hp: i32,
    /// Rank Attack soul count.
    pub rank_attack: i32,
    /// Rank Defense soul count (note: GVAS spells this "Rank_Defence" with British spelling).
    pub rank_defense: i32,
    /// Rank CraftSpeed soul count.
    pub rank_craftspeed: i32,
    /// IV (talent) for HP.
    pub talent_hp: i32,
    /// IV (talent) for Shot/Ranged Attack.
    pub talent_shot: i32,
    /// IV (talent) for Defense.
    pub talent_defense: i32,
    /// Current HP.
    pub hp: i32,
    /// Maximum HP.
    pub max_hp: i32,
    /// Sanity/tiredness value.
    pub sanity: i32,
    /// Stomach fullness value.
    pub stomach: i32,
    /// Skills this Pal has learned (mastered).
    pub learned_skills: Vec<String>,
    /// Skills currently equipped/active.
    pub active_skills: Vec<String>,
    /// Passive skills.
    pub passive_skills: Vec<String>,
    /// Work suitability map (job type → effectiveness level).
    pub work_suitability: BTreeMap<String, i32>,
    /// Friendship/trust points with owner.
    pub friendship_point: i32,
    /// Group/team ID if in a guild group.
    pub group_id: String,
}

/// A player character (server member).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Player {
    /// Unique player UID.
    pub uid: String,
    /// Instance ID (may differ from UID).
    pub instance_id: String,
    /// Player's nickname.
    pub nickname: String,
    /// Guild ID if member of one.
    pub guild_id: String,
    /// Character level.
    pub level: i32,
    /// Experience points.
    pub exp: i32,
    /// Current HP.
    pub hp: i32,
    /// Current stomach fullness.
    pub stomach: i32,
    /// Current sanity value.
    pub sanity: i32,
    /// Status point allocation (stat key → points).
    pub status_point_list: BTreeMap<String, i32>,
    /// Extended status points.
    pub ext_status_point_list: BTreeMap<String, i32>,
    /// Technologies/recipes unlocked.
    pub technologies: Vec<String>,
    /// Technology points available.
    pub technology_points: i32,
    /// Boss-fight technology points.
    pub boss_technology_points: i32,
    /// Pal storage container ID.
    pub pal_storage_container_id: String,
    /// Otomo (party) character container ID.
    pub otomo_character_container_id: String,
    /// Player's primary inventory container ID.
    pub player_inventory_container_id: String,
    /// Essential items container ID.
    pub essential_container_id: String,
    /// Weapon loadout container ID.
    pub weapon_loadout_container_id: String,
    /// Player armor container ID.
    pub player_equip_armor_container_id: String,
    /// Food equipment container ID.
    pub food_equip_container_id: String,
    /// Currently active missions.
    pub current_missions: Vec<String>,
    /// Completed missions.
    pub completed_missions: Vec<String>,
    /// Unlocked fast-travel points.
    pub unlocked_fast_travel_points: Vec<String>,
    /// Collected effigies.
    pub collected_effigies: Vec<String>,
}

/// A summary of player info (for lists, reduced fields).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct PlayerSummary {
    /// Player UID.
    pub uid: String,
    /// Player nickname.
    pub nickname: String,
    /// Character level.
    pub level: i32,
    /// Guild ID.
    pub guild_id: String,
    /// Count of Pals owned.
    pub pal_count: i32,
    /// Last online timestamp (ISO string or epoch).
    pub last_online: Option<String>,
}

/// A container for items or Pals.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ItemContainer {
    /// Container ID.
    pub id: String,
    /// Container type (Common, Essential, WeaponLoadOut, PlayerEquipArmor, FoodEquip, Base, GuildChest).
    pub container_type: String,
    /// Container key/name.
    pub key: String,
    /// Number of slots.
    pub slot_num: i32,
    /// Slots in this container.
    pub slots: Vec<ItemContainerSlot>,
}

/// A single slot in an item container.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ItemContainerSlot {
    /// Slot index (0-based).
    pub slot_index: i32,
    /// Stack count.
    pub count: i32,
    /// Static item ID (references the game's item database).
    pub static_id: String,
    /// Dynamic item data (if applicable: durability, passives, bullets, etc.).
    pub dynamic_item: Option<DynamicItem>,
}

/// Dynamic item data (instance-specific modifiers for weapons, armor, eggs, etc.).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct DynamicItem {
    /// Local instance ID for this dynamic item.
    pub local_id: String,
    /// Item type (weapon, armor, egg, etc.).
    pub item_type: String,
    /// Current durability (for weapons/armor).
    pub durability: f64,
    /// Passive skill list (for weapons).
    pub passive_skill_list: Vec<String>,
    /// Remaining bullets/ammo (for ranged weapons).
    pub remaining_bullets: i32,
    /// Egg parameters (for Pal eggs).
    pub egg_params: Option<EggParams>,
}

/// Parameters specific to a Pal egg.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct EggParams {
    /// Steps remaining until hatch.
    pub steps_remaining: i32,
}

/// A guild (player-run organization).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Guild {
    /// Guild ID.
    pub id: String,
    /// Guild name.
    pub name: String,
    /// Base camp level.
    pub base_camp_level: i32,
    /// Guild chest container.
    pub guild_chest: Option<ItemContainer>,
    /// Lab research data.
    pub lab_research: Vec<String>,
    /// Bases owned by this guild.
    pub bases: Vec<Base>,
    /// Member UIDs.
    pub players: Vec<String>,
    /// Admin player UID.
    pub admin_player_uid: String,
}

/// A base (deployed campsite for a guild).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Base {
    /// Base ID.
    pub id: String,
    /// Base name.
    pub name: String,
    /// Area range (radius or coverage level).
    pub area_range: i32,
    /// Storage container IDs at this base.
    pub storage_containers: Vec<String>,
    /// Pal instance IDs stationed here.
    pub pals: Vec<String>,
}

/// The entire game world state (read model).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct World {
    /// All players in the world.
    pub players: Vec<PlayerSummary>,
    /// All guilds in the world.
    pub guilds: Vec<Guild>,
}
