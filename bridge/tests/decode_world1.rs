//! Integration test: decode the `world1` fixture's characters into players and
//! pals via `load_world`.
//!
//! Ground truth for these assertions comes from the reference decoder
//! (`palworld-save-tools`) run against this exact fixture, and is corroborated
//! by `palworld-save-pal`'s own `tests/game/test_pal.py` (JetDragon is level 65;
//! 11 pals; all owned by player `8c2f1930-...`).

use std::path::Path;

use psm_save::save::containers::{
    decode_character_containers, decode_dynamic_items, decode_item_containers,
    read_player_container_ids,
};
use psm_save::save::decompress::decompress_sav;
use psm_save::save::gvas::{default_skip_set, parse_gvas};
use psm_save::save::{load_world, load_world_with_containers};
use psm_save::save::model::World;

const WORLD1_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world1");

fn world1() -> World {
    load_world(Path::new(WORLD1_DIR)).expect("load world1")
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

#[test]
fn player_pal_ownership_matches_fixture() {
    let w = world1();
    let sky = w
        .players
        .iter()
        .find(|p| p.uid.to_string().to_lowercase().starts_with("43797f87"))
        .unwrap();
    assert_eq!(sky.pal_count, 0, "Sky owns no pals in world1");
    // The other player therefore owns some of the 11.
    let o = w
        .players
        .iter()
        .find(|p| p.uid.to_string().to_lowercase().starts_with("8c2f1930"))
        .unwrap();
    assert!(o.pal_count > 0);
    // Player O owns all 11 pals; Sky owns none.
    assert_eq!(o.pal_count, 11, "player O owns all 11 pals");
}

#[test]
fn world1_has_guild_with_base() {
    let w = world1();
    assert!(!w.guilds.is_empty(), "at least one guild");
    assert!(
        w.guilds.iter().any(|g| !g.bases.is_empty()),
        "at least one guild has a base"
    );
    // Every player with a guild has a guild_id that resolves to a decoded guild.
    for p in &w.players {
        if let Some(gid) = &p.guild_id {
            assert!(w.guilds.iter().any(|g| &g.id == gid));
        }
    }
}

#[test]
fn player_o_inventory_has_real_items() {
    // GROUND TRUTH (palworld-save-tools reference decoder on this fixture):
    //   player O (8c2f1930-...)'s CommonContainerId is e204737a-... and its
    //   first four slots are Wood x77, Stone x74, Pal_crystal_S x99, SFArrow x997.
    let bytes = std::fs::read(format!("{WORLD1_DIR}/Level.sav")).expect("read Level.sav");
    let raw = decompress_sav(&bytes).expect("decompress");
    let gvas = parse_gvas(&raw, &default_skip_set()).expect("parse gvas");
    let wsd = gvas.root.get("worldSaveData").expect("worldSaveData");

    let dynamic_items =
        decode_dynamic_items(wsd.get_child("DynamicItemSaveData").expect("DynamicItemSaveData"))
            .expect("decode dynamic items");
    let item_containers = decode_item_containers(
        wsd.get_child("ItemContainerSaveData").expect("ItemContainerSaveData"),
        &dynamic_items,
    )
    .expect("decode item containers");

    // Player O's five inventory container ids come from its per-player .sav.
    let ids = read_player_container_ids(Path::new(&format!(
        "{WORLD1_DIR}/Players/8C2F1930000000000000000000000000.sav"
    )))
    .expect("read player O container ids");
    assert_eq!(ids.common, "e204737a-49d2-8cb5-526c-598291dc30f6");

    let common = item_containers
        .get(&ids.common.parse().expect("uuid"))
        .expect("player O common container present in Level.sav");

    // At least one non-empty slot with a real (non-"None") static_id.
    let real: Vec<&str> = common
        .slots
        .iter()
        .filter(|s| !s.static_id.is_empty() && s.static_id != "None")
        .map(|s| s.static_id.as_str())
        .collect();
    assert!(
        !real.is_empty(),
        "player O's common inventory has at least one real item"
    );

    // Exact ground-truth item: 77x Wood in slot 0.
    let wood = common
        .slots
        .iter()
        .find(|s| s.static_id == "Wood")
        .expect("player O has Wood in common inventory");
    assert_eq!(wood.slot_index, 0);
    assert_eq!(wood.count, 77, "player O has 77 Wood");
}

#[test]
fn character_containers_hold_all_pals() {
    // GROUND TRUTH (palworld-save-tools reference decoder on this fixture):
    //   every one of the 11 pals sits in a pal-box/party character container;
    //   player O's pal-box holds 6 and its party holds 5.
    let bytes = std::fs::read(format!("{WORLD1_DIR}/Level.sav")).expect("read Level.sav");
    let raw = decompress_sav(&bytes).expect("decompress");
    let gvas = parse_gvas(&raw, &default_skip_set()).expect("parse gvas");
    let wsd = gvas.root.get("worldSaveData").expect("worldSaveData");

    let char_containers = decode_character_containers(
        wsd.get_child("CharacterContainerSaveData").expect("CharacterContainerSaveData"),
    )
    .expect("decode character containers");

    // Total occupied slots across all containers == total pals in the world.
    let total_occupied: usize = char_containers.values().map(|v| v.len()).sum();
    assert_eq!(total_occupied, 11, "all 11 pals occupy a character-container slot");

    // Player O's pal-box (6) + party (5) == its 11 owned pals.
    let ids = read_player_container_ids(Path::new(&format!(
        "{WORLD1_DIR}/Players/8C2F1930000000000000000000000000.sav"
    )))
    .expect("read player O container ids");
    let pal_box = char_containers
        .get(&ids.pal_storage.parse().expect("uuid"))
        .expect("player O pal-box present");
    let party = char_containers
        .get(&ids.otomo.parse().expect("uuid"))
        .expect("player O party present");
    assert_eq!(pal_box.len(), 6, "player O pal-box holds 6 pals");
    assert_eq!(party.len(), 5, "player O party holds 5 pals");

    // Sky's pal-box + party are empty.
    let sky = read_player_container_ids(Path::new(&format!(
        "{WORLD1_DIR}/Players/43797F87000000000000000000000000.sav"
    )))
    .expect("read Sky container ids");
    assert_eq!(char_containers[&sky.pal_storage.parse().unwrap()].len(), 0);
    assert_eq!(char_containers[&sky.otomo.parse().unwrap()].len(), 0);
}

#[test]
fn load_world_with_containers_matches_load_world_and_has_item_containers() {
    // `load_world_with_containers` must decode Level.sav exactly once and
    // still agree with `load_world` on the world contents (2 players, 11
    // pals), while also yielding a non-empty `item_containers` index.
    let bundle =
        load_world_with_containers(Path::new(WORLD1_DIR)).expect("load world1 with containers");

    assert_eq!(bundle.world.players.len(), 2, "world1 has exactly 2 players");
    assert_eq!(bundle.world.pal_count(), 11, "world1 has exactly 11 pals");
    // `load_world_with_containers` additionally resolves each guild's chest
    // from `GuildExtraSaveDataMap` (which the plain `load_world` cannot — it
    // decodes no containers), so compare with chests stripped, then assert
    // the chest back-fill separately.
    let mut world_sans_chests = bundle.world.clone();
    for g in &mut world_sans_chests.guilds {
        g.guild_chest = None;
    }
    assert_eq!(
        world_sans_chests,
        world1(),
        "bundle.world (minus guild chests) matches load_world's result"
    );
    assert!(
        bundle.world.guilds.iter().any(|g| g.guild_chest.is_some()),
        "world1's guild has a resolvable guild chest"
    );
    assert!(!bundle.guild_chests.is_empty(), "guild_chests index populated");

    assert!(
        !bundle.item_containers.is_empty(),
        "world1's item_containers must be non-empty"
    );

    // Player O's common container (ground truth from `player_o_inventory_has_real_items`)
    // must be resolvable through the bundle's item_containers index, with the
    // same Wood x77 slot.
    let ids = read_player_container_ids(Path::new(&format!(
        "{WORLD1_DIR}/Players/8C2F1930000000000000000000000000.sav"
    )))
    .expect("read player O container ids");
    let common = bundle
        .item_containers
        .get(&ids.common.parse().expect("uuid"))
        .expect("player O common container present in bundle");
    let wood = common
        .slots
        .iter()
        .find(|s| s.static_id == "Wood")
        .expect("player O has Wood in common inventory");
    assert_eq!(wood.count, 77, "player O has 77 Wood");
}
