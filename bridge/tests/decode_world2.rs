//! Integration test: decode the `world2` fixture via `load_world`.
//!
//! `world2` is a second, independent save fixture used upstream
//! (palworld-save-pal `tests/game/`) as a minimal player-transfer *target*
//! world: one player ("O", uid `8c2f1930-...`), a fresh guild, no pals, no
//! bases. Ground truth: palworld-save-pal's own `test_world2_loads`
//! (`tests/game/test_save_manager.py`) asserts `len(get_player_summaries())
//! == 1` for this exact fixture.
//!
//! Despite having no pals, this is still a meaningful generalization check:
//! it proves `load_world` handles an empty `CharacterSaveParameterMap` pal
//! set and a guild with an empty `bases` list without erroring — cases
//! `world1` (11 pals, a guild with a base) does not exercise.

use std::path::Path;

use psm_save::save::load_world;
use psm_save::save::model::World;

const WORLD2_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world2");

fn world2() -> World {
    load_world(Path::new(WORLD2_DIR)).expect("load world2")
}

#[test]
fn world2_decodes_without_error_and_has_players() {
    let w = world2();
    assert!(!w.players.is_empty(), "world2 has at least one player");
}

#[test]
fn world2_player_o_matches_reference_ground_truth() {
    // GROUND TRUTH (palworld-save-pal `test_world2_loads`): exactly 1 player.
    let w = world2();
    assert_eq!(w.players.len(), 1, "world2 has exactly 1 player");

    let o = &w.players[0];
    assert_eq!(o.uid, "8c2f1930-0000-0000-0000-000000000000", "world2's player is O");
    assert_eq!(o.nickname, "O");
    assert_eq!(o.pal_count, 0, "world2's player owns no pals (fresh transfer target)");

    // world2 is a fresh save: no pals decoded at all.
    assert_eq!(w.pal_count(), 0, "world2 has no pals");

    // Player O is in a fresh guild with no base yet.
    assert_eq!(w.guilds.len(), 1, "world2 has exactly 1 guild");
    assert!(w.guilds[0].bases.is_empty(), "world2's guild has no base yet");
}

#[test]
fn world2_every_pal_has_a_character_id() {
    let w = world2();
    for pal in &w.pals {
        assert!(
            !pal.character_id.is_empty(),
            "pal {} has a non-empty character_id",
            pal.instance_id
        );
    }
}

#[test]
fn world2_every_player_guild_id_resolves_to_a_decoded_guild() {
    let w = world2();
    for p in &w.players {
        if let Some(gid) = &p.guild_id {
            assert!(
                w.guilds.iter().any(|g| &g.id == gid),
                "player {}'s guild_id {gid} resolves to a decoded guild",
                p.uid
            );
        }
    }
}
