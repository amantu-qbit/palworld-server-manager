//! Integration test: decode the `world1` fixture's characters into players and
//! pals via `load_world`.
//!
//! Ground truth for these assertions comes from the reference decoder
//! (`palworld-save-tools`) run against this exact fixture, and is corroborated
//! by `palworld-save-pal`'s own `tests/game/test_pal.py` (JetDragon is level 65;
//! 11 pals; all owned by player `8c2f1930-...`).

use std::path::Path;

use psm_save::save::load_world;
use psm_save::save::model::World;

fn world1() -> World {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world1");
    load_world(Path::new(dir)).expect("load world1")
}

#[test]
fn world1_has_expected_characters() {
    let w = world1();

    assert_eq!(w.players.len(), 2, "world1 has exactly 2 players");
    let mut uids: Vec<String> = w.players.iter().map(|p| p.uid.clone()).collect();
    uids.sort();
    assert_eq!(
        uids,
        vec![
            "43797f87-0000-0000-0000-000000000000".to_string(),
            "8c2f1930-0000-0000-0000-000000000000".to_string(),
        ],
        "player UIDs match the fixture's two player .sav file names"
    );

    assert_eq!(w.pal_count(), 11, "world1 has exactly 11 pals");
}

#[test]
fn world1_jetdragon_fields_match_reference() {
    // GROUND TRUTH (palworld-save-tools reference decoder on this fixture):
    //   character_id = "JetDragon", Level = 65, Talent_HP = 100,
    //   Rank_Defence = 20, OwnerPlayerUId = 8c2f1930-...
    let w = world1();

    let jet = w
        .pals
        .iter()
        .find(|p| p.character_id == "JetDragon")
        .expect("world1 contains a JetDragon");

    assert_eq!(jet.character_id, "JetDragon");
    assert_eq!(jet.level, 65, "JetDragon is level 65");
    assert_eq!(jet.talent_hp, 100, "JetDragon Talent_HP");
    assert_eq!(jet.rank_defense, 20, "JetDragon Rank_Defence (British spelling)");
    assert_eq!(
        jet.owner_uid, "8c2f1930-0000-0000-0000-000000000000",
        "JetDragon is owned by player O"
    );
}
