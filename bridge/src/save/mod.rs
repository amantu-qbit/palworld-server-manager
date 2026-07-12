//! Save-directory decoding (read-only).
pub mod character;
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
/// Player pal-ownership counts are left at `0` here (each summary's `pal_count`);
/// they are wired up once guild/group ownership is decoded in a later task. The
/// world's total pal count is available via [`World::pal_count`].
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
        .map(|p| PlayerSummary {
            uid: p.uid.clone(),
            nickname: p.nickname.clone(),
            level: p.level,
            guild_id: p.guild_id.clone(),
            // Ownership is wired up in a later task; 0 for now.
            pal_count: 0,
            last_online: None,
        })
        .collect();

    Ok(World {
        players: player_summaries,
        guilds: Vec::new(),
        pals,
    })
}
