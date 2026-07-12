//! Save-directory decoding (read-only).
pub mod character;
pub mod containers;
pub mod decompress;
pub mod guild;
pub mod gvas;
pub mod model;
pub mod props;
pub mod reader;
pub mod reference;

use std::path::Path;

use decompress::{decompress_sav, SaveError};
use gvas::{default_skip_set, parse_gvas};
use model::{PlayerSummary, World};

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
    let level_path = dir.join("Level.sav");
    let bytes = std::fs::read(&level_path)
        .map_err(|e| SaveError::Io(format!("{}: {e}", level_path.display())))?;

    let raw = decompress_sav(&bytes)?;
    let gvas = parse_gvas(&raw, &default_skip_set())?;

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

    Ok(World {
        players: player_summaries,
        guilds,
        pals,
    })
}
