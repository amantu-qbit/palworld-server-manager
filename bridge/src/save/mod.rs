//! Save-directory decoding, plus surgical save editing in [`edit`].
pub mod base_camp;
pub mod character;
pub mod containers;
pub mod debug;
pub mod decompress;
pub mod edit;
pub mod guild;
pub mod gvas;
pub mod model;
pub mod props;
pub mod reader;
pub mod reference;

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;

use decompress::{decompress_sav, SaveError};
use gvas::{default_skip_set, parse_gvas, Gvas};
use model::{DynamicItem, ItemContainer, Player, PlayerSummary, World};

/// The `Level.sav` GVAS parse plus every container/index derived from it in
/// one pass: the decoded [`World`] (players/pals/guilds) and the resolved
/// item-container + dynamic-item maps needed to answer inventory queries.
///
/// Built by [`load_world_with_containers`], which parses `Level.sav` exactly
/// once and produces all three fields from that single parse — see that
/// function's doc comment.
#[derive(Debug, Clone, Default)]
pub struct WorldBundle {
    /// Players, pals, and guilds — identical to [`load_world`]'s result.
    pub world: World,
    /// The full per-player records (level, exp, status-point allocations)
    /// behind `world.players`' summaries — used by the player-detail endpoint.
    pub players: Vec<Player>,
    /// `ItemContainerSaveData`, decoded and keyed by container id. Each
    /// slot's `dynamic_item` is already resolved against `dynamic_items`.
    pub item_containers: HashMap<Uuid, ItemContainer>,
    /// `DynamicItemSaveData`, decoded and keyed by `local_id`.
    pub dynamic_items: HashMap<Uuid, DynamicItem>,
    /// `GuildExtraSaveDataMap`, reduced to guild-id → guild-chest container id.
    pub guild_chests: HashMap<Uuid, Uuid>,
}

/// Load a save directory's `Level.sav` into a [`World`].
///
/// Reads `<dir>/Level.sav`, decompresses it, parses the GVAS envelope, decodes
/// `CharacterSaveParameterMap` into players + pals, and assembles the world.
///
/// Each pal already carries its `owner_uid` (from its `SaveParameter`), so a
/// player's `pal_count` is simply the number of pals whose `owner_uid` matches
/// that player's uid — the same owner-uid filter the reference's
/// `_get_player_pals` uses. This includes pals stationed at a base or out on
/// an expedition (they keep the owner's `owner_uid` but live in *base*
/// character containers, not the player's pal-box/party), so `pal_count` is
/// not limited to pal-box + party occupancy (see [`crate::save::containers`],
/// which decodes the container → pal-slot mapping and the per-player
/// container ids).
/// The world's total pal count is available via [`World::pal_count`].
pub fn load_world(dir: &Path) -> Result<World, SaveError> {
    let gvas = parse_level_sav(dir)?;
    Ok(build_world(&gvas)?.0)
}

/// Load a save directory's `Level.sav` into a [`WorldBundle`]: the [`World`]
/// plus the decoded item-container and dynamic-item indexes, all from a
/// single decompress + GVAS parse of `Level.sav`.
///
/// This is the parse-once counterpart to calling [`load_world`] and then
/// separately re-reading/re-parsing `Level.sav` to decode
/// `ItemContainerSaveData`/`DynamicItemSaveData` (as
/// `bridge/tests/decode_world1.rs` does today) — here both come from the same
/// in-memory [`Gvas`] tree that [`build_world`] also consumes.
///
/// `ItemContainerSaveData`/`DynamicItemSaveData` are expected top-level
/// members of `worldSaveData` in every real Palworld save; mirroring the
/// existing `GroupSaveDataMap` handling, a save that happens to omit one
/// yields an empty map for it rather than an error, so a minimal/edge-case
/// fixture doesn't fail to load just because it has no items.
pub fn load_world_with_containers(dir: &Path) -> Result<WorldBundle, SaveError> {
    let gvas = parse_level_sav(dir)?;
    let (mut world, players) = build_world(&gvas)?;

    let world_save_data = gvas
        .root
        .get("worldSaveData")
        .ok_or_else(|| SaveError::CharacterData("Level.sav missing worldSaveData".to_string()))?;

    let dynamic_items = match world_save_data.get_child("DynamicItemSaveData") {
        Some(prop) => containers::decode_dynamic_items(prop)?,
        None => HashMap::new(),
    };
    let item_containers = match world_save_data.get_child("ItemContainerSaveData") {
        Some(prop) => containers::decode_item_containers(prop, &dynamic_items)?,
        None => HashMap::new(),
    };
    let guild_chests = match world_save_data.get_child("GuildExtraSaveDataMap") {
        Some(prop) => guild::decode_guild_chests(prop)?,
        None => HashMap::new(),
    };
    let base_camps = match world_save_data.get_child("BaseCampSaveData") {
        Some(prop) => base_camp::decode_base_camps(prop)?,
        None => HashMap::new(),
    };

    // Back-fill each guild's chest with its resolved container so `/v1/guilds`
    // consumers see it without a second lookup.
    for g in &mut world.guilds {
        let Ok(gid) = Uuid::parse_str(&g.id) else {
            continue;
        };
        if let Some(cid) = guild_chests.get(&gid) {
            if let Some(container) = item_containers.get(cid) {
                let mut chest = container.clone();
                chest.container_type = "GuildChest".to_string();
                g.guild_chest = Some(chest);
            }
        }
        // Back-fill each base's name/area_range/position from BaseCampSaveData
        // (the guild only carries base ids; the camp data lives in a separate
        // map keyed by the same id).
        for b in &mut g.bases {
            if let Ok(bid) = Uuid::parse_str(&b.id) {
                if let Some(info) = base_camps.get(&bid) {
                    b.name = info.name.clone();
                    b.area_range = info.area_range as f64;
                    b.position = Some(info.position);
                }
            }
        }
    }

    Ok(WorldBundle {
        world,
        players,
        item_containers,
        dynamic_items,
        guild_chests,
    })
}

/// Read, decompress, and GVAS-parse `<dir>/Level.sav`. Shared by
/// [`load_world`] and [`load_world_with_containers`] so both perform exactly
/// one decompress + parse pass over the file.
fn parse_level_sav(dir: &Path) -> Result<Gvas, SaveError> {
    let level_path = dir.join("Level.sav");
    let bytes = std::fs::read(&level_path)
        .map_err(|e| SaveError::Io(format!("{}: {e}", level_path.display())))?;

    let raw = decompress_sav(&bytes)?;
    parse_gvas(&raw, &default_skip_set())
}

/// Decode the [`World`] (players/pals/guilds) from an already-parsed
/// `Level.sav` [`Gvas`] tree. Shared by [`load_world`] and
/// [`load_world_with_containers`].
fn build_world(gvas: &Gvas) -> Result<(World, Vec<Player>), SaveError> {
    let world_save_data = gvas
        .root
        .get("worldSaveData")
        .ok_or_else(|| SaveError::CharacterData("Level.sav missing worldSaveData".to_string()))?;
    let character_map = world_save_data
        .get_child("CharacterSaveParameterMap")
        .ok_or_else(|| {
            SaveError::CharacterData("worldSaveData missing CharacterSaveParameterMap".to_string())
        })?;

    let (players, pals) = character::decode_characters(character_map)?;

    // Guilds come from `GroupSaveDataMap` (Guild-type groups only). A save with
    // no group map yields no guilds rather than an error.
    let guilds = match world_save_data.get_child("GroupSaveDataMap") {
        Some(group_map) => guild::decode_guilds(group_map)?,
        None => Vec::new(),
    };

    let player_summaries = players
        .iter()
        .map(|p| {
            // A player owns every pal whose `owner_uid` is this player's uid.
            let pal_count = pals.iter().filter(|pal| pal.owner_uid == p.uid).count() as i32;
            // Back-fill guild membership: the guild whose decoded members list
            // contains this player's uid (uid strings are the same canonical
            // hyphenated form on both sides).
            let guild_id = guilds
                .iter()
                .find(|g| g.players.contains(&p.uid))
                .map(|g| g.id.clone());
            PlayerSummary {
                uid: p.uid.clone(),
                nickname: p.nickname.clone(),
                level: p.level,
                guild_id,
                pal_count,
                last_online: None,
            }
        })
        .collect();

    let world = World {
        players: player_summaries,
        guilds,
        pals,
    };
    Ok((world, players))
}
