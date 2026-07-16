//! Tier-2 pal operations against the world1 fixture: heal, gender, work
//! suitability, delete, clone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use psm_save::save::character::decode_characters;
use psm_save::save::containers::decode_character_containers;
use psm_save::save::decompress::decompress_sav_with_type;
use psm_save::save::edit::ops::{
    clone_pal, delete_pal, edit_character, heal_pal, CharTarget, CharacterEdits, HealValues,
};
use psm_save::save::gvas::{default_skip_set, parse_gvas};
use psm_save::save::model::Pal;
use psm_save::save::reference::{load_pal_stats, max_hp};
use psm_save::save::load_world_with_containers;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/saves/world1")
}

fn level_gvas() -> Vec<u8> {
    let bytes = std::fs::read(fixture_dir().join("Level.sav")).unwrap();
    decompress_sav_with_type(&bytes).unwrap().0
}

fn pals_of(buf: &[u8]) -> Vec<Pal> {
    let gvas = parse_gvas(buf, &default_skip_set()).unwrap();
    let map = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterSaveParameterMap")
        .unwrap();
    decode_characters(map).unwrap().1
}

fn boxed_pal(buf: &[u8]) -> Pal {
    // A pal that lives in a character container (has a storage_id).
    pals_of(buf)
        .into_iter()
        .find(|p| !p.storage_id.is_empty() && !p.owner_uid.is_empty())
        .expect("fixture has an owned, boxed pal")
}

#[test]
fn heal_restores_vitals_and_removes_sick_state() {
    let buf = level_gvas();
    let pal = boxed_pal(&buf);
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();

    let stats = load_pal_stats()
        .for_character_id(&pal.character_id)
        .expect("species stats known");
    let hp = max_hp(
        stats,
        pal.level,
        pal.talent_hp,
        pal.rank,
        pal.rank_hp,
        pal.is_boss || pal.is_lucky,
    );
    let heal = HealValues {
        hp: Some(hp),
        stomach: stats.stomach as f32,
        sanity: 100.0,
    };
    let out = heal_pal(&buf, iid, &heal).unwrap();

    let healed = pals_of(&out)
        .into_iter()
        .find(|p| p.instance_id == pal.instance_id)
        .unwrap();
    assert_eq!(i64::from(healed.hp), hp, "hp restored to computed max");
    assert_eq!(healed.sanity, 100);
    assert_eq!(healed.stomach, stats.stomach as i32);
    // Untouched identity preserved.
    assert_eq!(healed.character_id, pal.character_id);
    assert_eq!(healed.level, pal.level);
}

#[test]
fn gender_and_work_suitability_edits() {
    let buf = level_gvas();
    let pal = boxed_pal(&buf);
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();

    let flipped = if pal.gender.contains("Female") { "Male" } else { "Female" };
    let mut ws = BTreeMap::new();
    // Update an existing suitability if the pal has one, and add a new one.
    if let Some((code, _)) = pal.work_suitability.iter().next() {
        ws.insert(code.clone(), 4);
    }
    ws.insert("EPalWorkSuitability::Cool".to_string(), 3);

    let edits = CharacterEdits {
        gender: Some(flipped.to_string()),
        work_suitability: Some(ws.clone()),
        ..Default::default()
    };
    let out = edit_character(&buf, CharTarget::Instance(iid), &edits).unwrap();

    let p = pals_of(&out)
        .into_iter()
        .find(|p| p.instance_id == pal.instance_id)
        .unwrap();
    assert_eq!(p.gender, format!("EPalGenderType::{flipped}"));
    for (code, rank) in &ws {
        assert_eq!(p.work_suitability.get(code), Some(rank), "{code}");
    }
}

#[test]
fn delete_removes_pal_and_container_refs() {
    let buf = level_gvas();
    let before = load_world_with_containers(&fixture_dir()).unwrap();
    let pal = boxed_pal(&buf);
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();

    let out = delete_pal(&buf, iid).unwrap();

    let pals = pals_of(&out);
    assert!(pals.iter().all(|p| p.instance_id != pal.instance_id));
    assert_eq!(pals.len(), before.world.pals.len() - 1, "exactly one pal removed");

    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let ccs = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterContainerSaveData")
        .unwrap();
    let containers = decode_character_containers(ccs).unwrap();
    assert!(containers.values().flatten().all(|s| s.pal_id != pal.instance_id));
}

#[test]
fn clone_duplicates_into_target_container() {
    let buf = level_gvas();
    let pal = boxed_pal(&buf);
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();
    // Clone into the owner's PAL BOX (the party the pal may sit in is only 5
    // slots and can be full) — the same target the endpoint uses.
    let save = psm_save::save::containers::read_player_save(
        &fixture_dir()
            .join("Players")
            .join(format!("{}.sav", pal.owner_uid.replace('-', "").to_uppercase())),
    )
    .unwrap();
    let target = Uuid::parse_str(&save.containers.pal_storage).unwrap();
    let new_iid = Uuid::parse_str("7e57ab1e-c10e-4e57-9000-000000000001").unwrap();

    let out = clone_pal(&buf, iid, target, new_iid).unwrap();

    let pals = pals_of(&out);
    let src = pals.iter().find(|p| p.instance_id == pal.instance_id).unwrap();
    let dup = pals.iter().find(|p| p.instance_id == new_iid.to_string()).unwrap();
    assert_eq!(dup.character_id, src.character_id);
    assert_eq!(dup.level, src.level);
    assert_eq!(dup.passive_skills, src.passive_skills);
    assert_eq!(dup.owner_uid, src.owner_uid);
    assert_eq!(dup.storage_id, target.to_string());
    assert_ne!(dup.storage_slot, src.storage_slot, "clone gets its own slot");

    // Container slot exists for the clone.
    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let ccs = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterContainerSaveData")
        .unwrap();
    let containers = decode_character_containers(ccs).unwrap();
    let slots = &containers[&target];
    assert!(slots.iter().any(|s| s.pal_id == new_iid.to_string()));

    // Clone of a clone also works (fresh free-slot discovery).
    let new_iid2 = Uuid::parse_str("7e57ab1e-c10e-4e57-9000-000000000002").unwrap();
    let out2 = clone_pal(&out, new_iid, target, new_iid2).unwrap();
    assert!(pals_of(&out2).iter().any(|p| p.instance_id == new_iid2.to_string()));
}

#[test]
fn delete_then_clone_reuses_freed_slot() {
    // Clone into the slot a deleted pal vacated — exercises free-slot reuse.
    let buf = level_gvas();
    let pal = boxed_pal(&buf);
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();
    let target = Uuid::parse_str(&pal.storage_id).unwrap();

    let pals = pals_of(&buf);
    let other = pals
        .iter()
        .find(|p| p.storage_id == pal.storage_id && p.instance_id != pal.instance_id);
    let Some(other) = other else {
        return; // fixture has no second pal in the same container — nothing to test
    };

    let deleted = delete_pal(&buf, iid).unwrap();
    let new_iid = Uuid::parse_str("7e57ab1e-c10e-4e57-9000-00000000000f").unwrap();
    let other_iid = Uuid::parse_str(&other.instance_id).unwrap();
    let out = clone_pal(&deleted, other_iid, target, new_iid).unwrap();
    assert!(pals_of(&out).iter().any(|p| p.instance_id == new_iid.to_string()));
}
