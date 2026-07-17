//! Edit-engine integration tests against the real `world1` fixture.
//!
//! Every test decompresses the fixture, applies an edit, and re-reads the
//! result through the full production pipeline (decompress → parse → domain
//! decode), asserting both the intended change and the preservation of
//! neighboring state. The final test round-trips through `pack_sav` +
//! `edit_sav_file` on a temp copy — the exact path the HTTP layer uses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use psm_save::save::containers::{decode_dynamic_items, decode_item_containers, read_player_save};
use psm_save::save::decompress::decompress_sav_with_type;
use psm_save::save::edit::ops::{
    edit_character, edit_player_technologies, resize_container, set_container_slot, CharTarget,
    CharacterEdits, TechEdits,
};
use psm_save::save::edit::{edit_sav_file, write::pack_sav};
use psm_save::save::gvas::{default_skip_set, parse_gvas};
use psm_save::save::model::ItemContainer;
use psm_save::save::{load_world, load_world_with_containers};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/saves/world1")
}

fn level_gvas() -> Vec<u8> {
    let bytes = std::fs::read(fixture_dir().join("Level.sav")).unwrap();
    decompress_sav_with_type(&bytes).unwrap().0
}

/// Decode containers straight from an edited GVAS buffer.
fn containers_of(buf: &[u8]) -> std::collections::HashMap<Uuid, ItemContainer> {
    let gvas = parse_gvas(buf, &default_skip_set()).unwrap();
    let wsd = gvas.root.get("worldSaveData").unwrap();
    let dyn_items = decode_dynamic_items(wsd.get_child("DynamicItemSaveData").unwrap()).unwrap();
    decode_item_containers(wsd.get_child("ItemContainerSaveData").unwrap(), &dyn_items).unwrap()
}

/// Path of a player's `<UID>.sav` under a save dir.
fn player_sav_path(dir: &Path, uid: &str) -> PathBuf {
    dir.join("Players")
        .join(format!("{}.sav", uid.replace('-', "").to_uppercase()))
}

/// Player O's common container (the one holding Wood ×77 in slot 0 per
/// `decode_world1.rs`), plus their uid.
fn player_common_container(buf: &[u8]) -> (Uuid, Uuid) {
    let dir = fixture_dir();
    let bundle = load_world_with_containers(&dir).unwrap();
    let player = &bundle.world.players[0];
    let uid = Uuid::parse_str(&player.uid).unwrap();
    let save = read_player_save(&player_sav_path(&dir, &player.uid)).unwrap();
    let cid = Uuid::parse_str(&save.containers.common).unwrap();
    // Sanity: the container exists in the freshly decompressed buffer too.
    assert!(containers_of(buf).contains_key(&cid));
    (uid, cid)
}

#[test]
fn resize_grow_bumps_slot_num_only() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let before = containers_of(&buf);
    let old = &before[&cid];
    let old_slots: Vec<_> = old.slots.iter().map(|s| (s.slot_index, s.count)).collect();
    let new_n = (old.slot_num + 10) as u32;

    let out = resize_container(&buf, cid, new_n).unwrap();
    let after = containers_of(&out);
    let c = &after[&cid];
    assert_eq!(c.slot_num, new_n as i32);
    let new_slots: Vec<_> = c.slots.iter().map(|s| (s.slot_index, s.count)).collect();
    assert_eq!(old_slots, new_slots, "growing must not touch existing slots");

    // Neighboring state intact: same container count, same world decode.
    assert_eq!(before.len(), after.len());
}

#[test]
fn resize_shrink_drops_out_of_range_slots() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let before = containers_of(&buf);
    let old = &before[&cid];
    assert!(
        old.slots.iter().any(|s| s.slot_index >= 1),
        "fixture should have an occupied slot past index 0"
    );

    // Shrink to a single slot: only slot_index 0 may survive.
    let out = resize_container(&buf, cid, 1).unwrap();
    let after = containers_of(&out);
    let c = &after[&cid];
    assert_eq!(c.slot_num, 1);
    assert!(c.slots.iter().all(|s| s.slot_index < 1));

    // The world still decodes: characters unaffected.
    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    assert!(gvas.root.contains_key("worldSaveData"));
}

#[test]
fn resize_to_zero_empties_container() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);
    let out = resize_container(&buf, cid, 0).unwrap();
    let c = &containers_of(&out)[&cid];
    assert_eq!(c.slot_num, 0);
    assert!(c.slots.is_empty());
}

#[test]
fn set_slot_overwrites_existing() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    // Slot 0 holds Wood ×77 in the fixture; overwrite it.
    let out = set_container_slot(&buf, cid, 0, "Stone", 55).unwrap();
    let c = &containers_of(&out)[&cid];
    let s = c.slots.iter().find(|s| s.slot_index == 0).unwrap();
    assert_eq!(s.static_id, "Stone");
    assert_eq!(s.count, 55);
    assert!(s.dynamic_item.is_none());
}

#[test]
fn set_slot_adds_new_entry_in_empty_slot() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let before = &containers_of(&buf)[&cid];
    // Find an index with no entry inside the container's range.
    let used: Vec<i32> = before.slots.iter().map(|s| s.slot_index).collect();
    let free = (0..before.slot_num)
        .find(|i| !used.contains(i))
        .expect("fixture container should have a free slot");

    let out = set_container_slot(&buf, cid, free, "PalSphere", 42).unwrap();
    let c = &containers_of(&out)[&cid];
    let s = c.slots.iter().find(|s| s.slot_index == free).unwrap();
    assert_eq!(s.static_id, "PalSphere");
    assert_eq!(s.count, 42);
    // Existing slots untouched.
    for old in &before.slots {
        let now = c.slots.iter().find(|s| s.slot_index == old.slot_index).unwrap();
        assert_eq!(now.static_id, old.static_id);
        assert_eq!(now.count, old.count);
    }
}

#[test]
fn clear_slot_removes_entry() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let out = set_container_slot(&buf, cid, 0, "None", 0).unwrap();
    let c = &containers_of(&out)[&cid];
    assert!(c.slots.iter().all(|s| s.slot_index != 0));

    // Clearing an already-empty slot is a no-op, not an error.
    let out2 = set_container_slot(&out, cid, 0, "None", 0).unwrap();
    assert_eq!(out, out2);
}

#[test]
fn set_slot_rejects_out_of_range_index() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);
    let n = containers_of(&buf)[&cid].slot_num;
    assert!(set_container_slot(&buf, cid, n, "Stone", 1).is_err());
    assert!(set_container_slot(&buf, cid, -1, "Stone", 1).is_err());
}

#[test]
fn edit_player_level_exp_and_status_points() {
    let buf = level_gvas();
    let dir = fixture_dir();
    let world = load_world(&dir).unwrap();
    let bundle = load_world_with_containers(&dir).unwrap();
    let player = bundle
        .players
        .iter()
        .find(|p| !p.status_point_list.is_empty())
        .unwrap_or(&bundle.players[0]);
    let uid = Uuid::parse_str(&player.uid).unwrap();

    let mut edits = CharacterEdits {
        level: Some(55),
        exp: Some(3_947_260),
        ..Default::default()
    };
    if let Some(name) = player.status_point_list.keys().next() {
        let mut pts = BTreeMap::new();
        pts.insert(name.clone(), 12);
        edits.status_points = Some(pts);
    }

    let out = edit_character(&buf, CharTarget::Player(uid), &edits).unwrap();

    // Re-decode through the character pipeline.
    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let map = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterSaveParameterMap")
        .unwrap();
    let (players, pals) = psm_save::save::character::decode_characters(map).unwrap();
    let p = players.iter().find(|p| p.uid == player.uid).unwrap();
    assert_eq!(p.level, 55);
    assert_eq!(p.exp, 3_947_260);
    if let Some(pts) = &edits.status_points {
        for (k, v) in pts {
            assert_eq!(p.status_point_list.get(k), Some(v));
        }
    }
    // Other characters untouched.
    assert_eq!(pals.len(), world.pals.len());
}

/// Raising a stat the player has never allocated must *insert* a new
/// `GotStatusPointList` entry (not error), while patching an existing one in the
/// same edit still works.
#[test]
fn edit_player_status_points_insert_new_allocation() {
    let buf = level_gvas();
    let dir = fixture_dir();
    let bundle = load_world_with_containers(&dir).unwrap();
    let player = bundle
        .players
        .iter()
        .find(|p| !p.status_point_list.is_empty())
        .expect("a player with base status entries");
    let uid = Uuid::parse_str(&player.uid).unwrap();

    // A name definitely not already present → forces the insert path.
    let new_name = "PSM_TEST_RELIC".to_string();
    assert!(!player.status_point_list.contains_key(&new_name));
    let existing_key = player.status_point_list.keys().next().unwrap().clone();

    let mut pts = BTreeMap::new();
    pts.insert(existing_key.clone(), 9); // patch existing
    pts.insert(new_name.clone(), 15); // insert new
    let edits = CharacterEdits {
        status_points: Some(pts),
        ..Default::default()
    };

    let out = edit_character(&buf, CharTarget::Player(uid), &edits).unwrap();

    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let map = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterSaveParameterMap")
        .unwrap();
    let (players, _) = psm_save::save::character::decode_characters(map).unwrap();
    let p = players.iter().find(|p| p.uid == player.uid).unwrap();
    assert_eq!(p.status_point_list.get(&existing_key), Some(&9));
    assert_eq!(p.status_point_list.get(&new_name), Some(&15));
    // The list grew by exactly the one inserted entry.
    assert_eq!(
        p.status_point_list.len(),
        player.status_point_list.len() + 1
    );
}

/// Anti-bloat rule (matches palworld-save-pal): a stat the save has no row for,
/// set to 0, must NOT append a row — the game creates rows lazily and absent
/// means rank 0. The UI sends every relic key at save time, most at 0.
#[test]
fn edit_player_status_points_zero_for_absent_appends_nothing() {
    let buf = level_gvas();
    let dir = fixture_dir();
    let bundle = load_world_with_containers(&dir).unwrap();
    let player = bundle
        .players
        .iter()
        .find(|p| !p.status_point_list.is_empty())
        .expect("a player with base status entries");
    let uid = Uuid::parse_str(&player.uid).unwrap();
    let before = player.status_point_list.len();

    let mut pts = BTreeMap::new();
    pts.insert("PSM_TEST_ABSENT_ZERO".to_string(), 0); // absent + zero → no row
    let edits = CharacterEdits {
        status_points: Some(pts),
        ..Default::default()
    };

    let out = edit_character(&buf, CharTarget::Player(uid), &edits).unwrap();
    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let map = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterSaveParameterMap")
        .unwrap();
    let (players, _) = psm_save::save::character::decode_characters(map).unwrap();
    let p = players.iter().find(|p| p.uid == player.uid).unwrap();
    assert!(!p.status_point_list.contains_key("PSM_TEST_ABSENT_ZERO"));
    assert_eq!(p.status_point_list.len(), before, "no row should be added");
}

#[test]
fn edit_pal_fields() {
    let buf = level_gvas();
    let world = load_world(&fixture_dir()).unwrap();
    let pal = &world.pals[0];
    let iid = Uuid::parse_str(&pal.instance_id).unwrap();

    let edits = CharacterEdits {
        level: Some(60),
        exp: Some(123_456_789),
        nickname: Some("Pál Söul".to_string()), // non-ASCII → UTF-16 path
        passive_skills: Some(vec![
            "CraftSpeed_up2".to_string(),
            "Rare".to_string(),
            "Legend".to_string(),
        ]),
        rank: Some(4),
        rank_hp: Some(9),
        talent_hp: Some(100),
        ..Default::default()
    };
    let out = edit_character(&buf, CharTarget::Instance(iid), &edits).unwrap();

    let gvas = parse_gvas(&out, &default_skip_set()).unwrap();
    let map = gvas
        .root
        .get("worldSaveData")
        .unwrap()
        .get_child("CharacterSaveParameterMap")
        .unwrap();
    let (_, pals) = psm_save::save::character::decode_characters(map).unwrap();
    let p = pals.iter().find(|p| p.instance_id == pal.instance_id).unwrap();
    assert_eq!(p.level, 60);
    assert_eq!(p.exp, 123_456_789);
    assert_eq!(p.nickname, "Pál Söul");
    assert_eq!(p.passive_skills, vec!["CraftSpeed_up2", "Rare", "Legend"]);
    assert_eq!(p.rank, 4);
    assert_eq!(p.rank_hp, 9);
    assert_eq!(p.talent_hp, 100);
    // Untouched fields survive.
    assert_eq!(p.character_id, pal.character_id);
    assert_eq!(p.owner_uid, pal.owner_uid);
    assert_eq!(p.storage_id, pal.storage_id);
}

#[test]
fn edit_unknown_targets_error() {
    let buf = level_gvas();
    let ghost = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
    assert!(edit_character(&buf, CharTarget::Player(ghost), &CharacterEdits::default()).is_err());
    assert!(resize_container(&buf, ghost, 10).is_err());
}

#[test]
fn edit_player_sav_technologies() {
    let dir = fixture_dir();
    let world = load_world(&dir).unwrap();
    let uid = &world.players[0].uid;
    let before = read_player_save(&player_sav_path(&dir, uid)).unwrap();

    let sav_name = uid.replace('-', "").to_uppercase();
    let bytes = std::fs::read(dir.join("Players").join(format!("{sav_name}.sav"))).unwrap();
    let (gvas_bytes, _) = decompress_sav_with_type(&bytes).unwrap();

    let edits = TechEdits {
        unlock: vec!["Workbench".to_string(), "HandTorch".to_string()],
        relock: vec![],
        technology_point: Some(99),
        boss_technology_point: Some(7),
    };
    let out = edit_player_technologies(&gvas_bytes, &edits).unwrap();

    // Write to a temp player file and re-read via the production reader.
    let tmp = std::env::temp_dir().join(format!("psm-tech-test-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("Players")).unwrap();
    std::fs::write(
        tmp.join("Players").join(format!("{sav_name}.sav")),
        pack_sav(&out, 0x31).unwrap(),
    )
    .unwrap();
    let after = read_player_save(&player_sav_path(&tmp, uid)).unwrap();
    std::fs::remove_dir_all(&tmp).ok();

    for t in ["Workbench", "HandTorch"] {
        assert!(after.technologies.contains(&t.to_string()));
    }
    for t in &before.technologies {
        assert!(after.technologies.contains(t), "pre-existing tech {t} lost");
    }
    assert_eq!(after.technology_points, 99);
    assert_eq!(after.boss_technology_points, 7);
}

#[test]
fn edit_sav_file_round_trip_with_backup() {
    // Copy the fixture world into a temp dir and run the full file-level path.
    let src = fixture_dir();
    let tmp = std::env::temp_dir().join(format!("psm-editfile-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let level = tmp.join("Level.sav");
    std::fs::copy(src.join("Level.sav"), &level).unwrap();

    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let receipt = edit_sav_file(&level, |gvas| resize_container(gvas, cid, 77)).unwrap();
    let backup_path = receipt.backup.as_ref().expect("a real edit takes a backup");
    assert!(backup_path.exists());
    assert!(receipt.bytes_written > 12);

    // The edited file re-reads through the full pipeline (now PlZ instead of
    // the fixture's original Oodle) and shows the change.
    let bytes = std::fs::read(&level).unwrap();
    assert_eq!(&bytes[8..11], b"PlZ");
    let (gvas2, save_type) = decompress_sav_with_type(&bytes).unwrap();
    assert_eq!(save_type, 0x31);
    let c = &containers_of(&gvas2)[&cid];
    assert_eq!(c.slot_num, 77);

    // Backup byte-identical to the original fixture.
    let backup_bytes = std::fs::read(backup_path).unwrap();
    let original = std::fs::read(src.join("Level.sav")).unwrap();
    assert_eq!(backup_bytes, original);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn no_op_edit_preserves_buffer_exactly() {
    // An edit plan with no changes must leave every byte untouched: clearing
    // an empty slot returns the identical buffer.
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);
    let n = containers_of(&buf)[&cid].slot_num;
    // Find an index inside range with no entry.
    let used: Vec<i32> = containers_of(&buf)[&cid].slots.iter().map(|s| s.slot_index).collect();
    if let Some(free) = (0..n).find(|i| !used.contains(i)) {
        let out = set_container_slot(&buf, cid, free, "None", 0).unwrap();
        assert_eq!(out, buf);
    }
}

// ---------------------------------------------------------------------------
// Regression tests for adversarial-review findings
// ---------------------------------------------------------------------------

/// Build a copy of the Level buffer with the target container's `SlotNum`
/// property surgically removed — simulating UE's omit-default-value
/// serialization, the state that routes `resize_container` through its
/// insert-SlotNum branch.
fn strip_slot_num(buf: &[u8], cid: Uuid) -> Vec<u8> {
    use psm_save::save::edit::locate::{find_in_stream, map_info, read_tag, Cursor};
    use psm_save::save::edit::plan::{apply as apply_plan, EditPlan};
    use psm_save::save::gvas::header_len_for_tests;

    let start = header_len_for_tests(buf).unwrap();
    let mut c = Cursor::new(buf, start);
    let wsd = find_in_stream(&mut c, "worldSaveData").unwrap().found.unwrap();
    let mut c = Cursor::new(buf, wsd.value_start);
    let map = find_in_stream(&mut c, "ItemContainerSaveData").unwrap().found.unwrap();
    let mut c = Cursor::new(buf, map.value_start);
    let info = map_info(&mut c).unwrap();

    for _ in 0..info.count {
        // Key stream: { ID: Guid }.
        let mut id = Uuid::nil();
        loop {
            match read_tag(&mut c).unwrap() {
                None => break,
                Some(t) => {
                    if t.name == "ID" && t.struct_type.as_deref() == Some("Guid") {
                        id = c.guid().unwrap();
                    }
                    c.seek(t.value_end).unwrap();
                }
            }
        }
        // Value stream.
        if id == cid {
            let scan = find_in_stream(&mut c, "SlotNum").unwrap();
            let t = scan.found.expect("fixture container has SlotNum");
            let mut plan = EditPlan::default();
            plan.delete(t.tag_start..t.value_end);
            plan.scope_u64(wsd.size_field, wsd.value_start..wsd.value_end);
            plan.scope_u64(map.size_field, map.value_start..map.value_end);
            return apply_plan(buf, &plan).unwrap();
        }
        // Skip the non-matching value stream.
        loop {
            match read_tag(&mut c).unwrap() {
                None => break,
                Some(t) => c.seek(t.value_end).unwrap(),
            }
        }
    }
    panic!("container not found");
}

#[test]
fn resize_with_missing_slot_num_inserts_before_slots_and_stays_editable() {
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);

    let stripped = strip_slot_num(&buf, cid);
    // Sanity: the stripped buffer still parses and shows slot_num == 0.
    let c0 = &containers_of(&stripped)[&cid];
    assert_eq!(c0.slot_num, 0, "SlotNum removed reads as default 0");
    assert!(!c0.slots.is_empty(), "slots survive the strip");

    // Shrink to 1 through the insert-SlotNum branch (removes elements too —
    // the exact combination that used to corrupt the Slots size fields).
    let out = resize_container(&stripped, cid, 1).unwrap();
    let c1 = &containers_of(&out)[&cid];
    assert_eq!(c1.slot_num, 1);
    assert!(c1.slots.iter().all(|s| s.slot_index < 1));

    // The regression's signature was that the container became permanently
    // un-editable ("declared size mismatch walking Slots elements") and
    // SlotNum unreadable via size-driven skips. Both must work now:
    let again = resize_container(&out, cid, 30).unwrap();
    let c2 = &containers_of(&again)[&cid];
    assert_eq!(c2.slot_num, 30);
    let final_edit = set_container_slot(&again, cid, 2, "Stone", 5).unwrap();
    assert_eq!(
        containers_of(&final_edit)[&cid]
            .slots
            .iter()
            .find(|s| s.slot_index == 2)
            .unwrap()
            .static_id,
        "Stone"
    );
}

#[test]
fn clear_container_removes_everything_in_one_edit() {
    use psm_save::save::edit::ops::clear_container;
    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);
    assert!(!containers_of(&buf)[&cid].slots.is_empty());

    let out = clear_container(&buf, cid).unwrap();
    let c = &containers_of(&out)[&cid];
    assert!(c.slots.is_empty());
    // Slot count preserved — clearing empties, it does not resize.
    assert_eq!(c.slot_num, containers_of(&buf)[&cid].slot_num);

    // Clearing an already-empty container is a byte-identical no-op.
    let again = clear_container(&out, cid).unwrap();
    assert_eq!(again, out);
}

#[test]
fn noop_edit_skips_write_and_backup() {
    let src = fixture_dir();
    let tmp = std::env::temp_dir().join(format!("psm-noop-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let level = tmp.join("Level.sav");
    std::fs::copy(src.join("Level.sav"), &level).unwrap();
    let before = std::fs::read(&level).unwrap();

    let buf = level_gvas();
    let (_uid, cid) = player_common_container(&buf);
    let free = {
        let c = &containers_of(&buf)[&cid];
        let used: Vec<i32> = c.slots.iter().map(|s| s.slot_index).collect();
        (0..c.slot_num).find(|i| !used.contains(i))
    };
    if let Some(free) = free {
        let receipt =
            edit_sav_file(&level, |gvas| set_container_slot(gvas, cid, free, "None", 0)).unwrap();
        assert!(receipt.backup.is_none(), "no-op must not take a backup");
        assert_eq!(receipt.bytes_written, 0);
        assert_eq!(std::fs::read(&level).unwrap(), before, "file untouched");
        assert!(!tmp.join("psm-backups").exists(), "no backup dir created");
    }
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn technology_matching_is_case_insensitive() {
    let dir = fixture_dir();
    let world = load_world(&dir).unwrap();
    let uid = &world.players[0].uid;
    let before = read_player_save(&player_sav_path(&dir, uid)).unwrap();
    let existing = before
        .technologies
        .iter()
        .find(|t| t.chars().any(|c| c.is_ascii_alphabetic()))
        .expect("fixture player has technologies")
        .clone();
    let flipped: String = existing
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    assert_ne!(existing, flipped);

    let bytes = std::fs::read(player_sav_path(&dir, uid)).unwrap();
    let (gvas_bytes, _) = decompress_sav_with_type(&bytes).unwrap();

    // Unlocking a differently-cased duplicate must NOT add a second entry;
    // relocking with different case MUST remove the stored one.
    let out = edit_player_technologies(
        &gvas_bytes,
        &TechEdits {
            unlock: vec![flipped.clone()],
            relock: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    let out = edit_player_technologies(
        &out,
        &TechEdits {
            unlock: vec![],
            relock: vec![flipped.clone()],
            ..Default::default()
        },
    )
    .unwrap();

    let tmp = std::env::temp_dir().join(format!("psm-techcase-test-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("Players")).unwrap();
    let sav_name = uid.replace('-', "").to_uppercase();
    std::fs::write(
        tmp.join("Players").join(format!("{sav_name}.sav")),
        pack_sav(&out, 0x31).unwrap(),
    )
    .unwrap();
    let after = read_player_save(&player_sav_path(&tmp, uid)).unwrap();
    std::fs::remove_dir_all(&tmp).ok();

    assert!(
        !after.technologies.iter().any(|t| t.eq_ignore_ascii_case(&existing)),
        "case-flipped relock must remove the stored entry"
    );
    assert_eq!(
        after.technologies.len(),
        before.technologies.len() - 1,
        "no duplicate was added by the case-flipped unlock"
    );
}
