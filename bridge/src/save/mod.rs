//! Save-directory decoding (read-only).
pub mod character;
pub mod containers;
pub mod decompress;
pub mod gvas;
pub mod model;
pub mod props;
pub mod reader;

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
/// player's `pal_count` is the number of pals it owns. For this fixture that is
/// identical to the reference's pal-box + party occupancy: every owned pal is in
/// one of those two character containers (see [`crate::save::containers`], which
/// decodes the container → pal-slot mapping and the per-player container ids).
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

    let player_summaries = players
        .iter()
        .map(|p| {
            // A player owns every pal whose `owner_uid` is this player's uid.
            let pal_count = pals.iter().filter(|pal| pal.owner_uid == p.uid).count() as i32;
            PlayerSummary {
                uid: p.uid.clone(),
                nickname: p.nickname.clone(),
                level: p.level,
                guild_id: p.guild_id.clone(),
                pal_count,
                last_online: None,
            }
        })
        .collect();

    Ok(World {
        players: player_summaries,
        guilds: Vec::new(),
        pals,
    })
}
