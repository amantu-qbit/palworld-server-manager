//! Decode `CharacterSaveParameterMap` entries into players and pals.
//!
//! Ports `palworld_save_tools/rawdata/character.py::decode` and the field
//! accessors in `palworld-save-pal`'s `game/pal.py`, `game/player.py`, and
//! `game/pal_objects.py`.
//!
//! ## Structure (from the reference)
//!
//! `paltypes.py` registers `character.decode` at the dotted path
//! `.worldSaveData.CharacterSaveParameterMap.Value.RawData`, so each character
//! lives inside the map entry's `RawData` byte blob. Task 4's generic parser
//! leaves that blob as a [`Property::Bytes`]; here we re-parse it.
//!
//! The `RawData` bytes are an **inner GVAS property stream** followed by a small
//! trailer (`character.py::decode_bytes`):
//!
//! ```text
//! object        := properties_until_end()   // the "SaveParameter" struct etc.
//! unknown_bytes := 4 bytes                   // consumed, unused
//! group_id      := guid (16 bytes)           // the pal/player's group UID
//! trailing      := 4 bytes                   // consumed, unused
//! <EOF>
//! ```
//!
//! The map **key** is a struct of `PlayerUId` + `InstanceId` (both `Guid`
//! structs). The character fields live under `object.SaveParameter.value`. An
//! entry is a **player** iff its `SaveParameter` has `IsPlayer == true`;
//! otherwise it is a pal.

use std::collections::BTreeMap;

use uuid::Uuid;

use super::decompress::SaveError;
use super::gvas::read_properties_until_end;
use super::model::{Pal, Player};
use super::props::{ArrayValue, ByteVal, Property, StructValue};
use super::reader::Reader;

/// Decode a `CharacterSaveParameterMap` [`Property::Map`] into its players and
/// pals, split by the `IsPlayer` discriminator.
///
/// Players are returned "lite": only the fields carried by the character
/// `SaveParameter` (identity, nickname, level, and the basic vitals) are
/// populated; container/mission/technology data comes from the per-player
/// `<uid>.sav` files decoded in a later task.
pub fn decode_characters(map: &Property) -> Result<(Vec<Player>, Vec<Pal>), SaveError> {
    let entries = match map {
        Property::Map { entries, .. } => entries,
        _ => {
            return Err(SaveError::CharacterData(
                "CharacterSaveParameterMap is not a MapProperty".to_string(),
            ))
        }
    };

    let mut players = Vec::new();
    let mut pals = Vec::new();

    for entry in entries {
        // --- map key: PlayerUId + InstanceId ---------------------------------
        let player_uid = entry
            .key
            .get_child("PlayerUId")
            .and_then(struct_guid)
            .ok_or_else(|| SaveError::CharacterData("entry key missing PlayerUId".to_string()))?;
        let instance_id = entry
            .key
            .get_child("InstanceId")
            .and_then(struct_guid)
            .ok_or_else(|| SaveError::CharacterData("entry key missing InstanceId".to_string()))?;

        // --- map value: RawData byte blob ------------------------------------
        let raw = entry
            .value
            .get_child("RawData")
            .and_then(Property::as_bytes)
            .ok_or_else(|| SaveError::CharacterData("entry value missing RawData".to_string()))?;

        let (object, group_id) = decode_raw_data(raw)?;

        let save_parameter = object
            .get("SaveParameter")
            .and_then(Property::as_properties)
            .ok_or_else(|| {
                SaveError::CharacterData("RawData object missing SaveParameter".to_string())
            })?;

        if get_bool(save_parameter, "IsPlayer") {
            players.push(build_player(save_parameter, player_uid, instance_id));
        } else {
            pals.push(build_pal(save_parameter, player_uid, instance_id, group_id));
        }
    }

    Ok((players, pals))
}

/// Re-parse a character `RawData` blob: the inner GVAS property stream plus the
/// `unknown_bytes` / `group_id` / `trailing_bytes` trailer
/// (`character.py::decode_bytes`). Fails loud if the trailer does not land
/// exactly at EOF, matching the reference's "EOF not reached" guard.
fn decode_raw_data(bytes: &[u8]) -> Result<(BTreeMap<String, Property>, Uuid), SaveError> {
    let mut r = Reader::new(bytes);
    // The reference wraps a fresh reader and calls `properties_until_end()` with
    // an empty path; no inner path is ever in the skip set.
    let object = read_properties_until_end(&mut r, "", &Default::default())?;
    if r.remaining() < 24 {
        return Err(SaveError::CharacterData(format!(
            "RawData trailer truncated: {} bytes left, need 24",
            r.remaining()
        )));
    }
    let _unknown_bytes = r.read(4);
    let group_id = r.guid();
    let _trailing_bytes = r.read(4);
    if !r.eof() {
        return Err(SaveError::CharacterData(format!(
            "RawData not fully consumed: {} trailing bytes",
            r.remaining()
        )));
    }
    Ok((object, group_id))
}

/// Build a "lite" [`Player`] from its `SaveParameter` and map-key identity.
fn build_player(sp: &BTreeMap<String, Property>, uid: Uuid, instance_id: Uuid) -> Player {
    Player {
        uid: guid_string(uid),
        instance_id: guid_string(instance_id),
        nickname: get_str(sp, "NickName"),
        level: get_byte(sp, "Level", 1),
        exp: get_int(sp, "Exp", 0),
        hp: get_fixed_point64(sp, "Hp"),
        stomach: get_float(sp, "FullStomach", 150.0) as i32,
        sanity: get_float(sp, "SanityValue", 100.0) as i32,
        ..Player::default()
    }
}

/// Build a [`Pal`] from its `SaveParameter`, map-key identity, and group UID.
fn build_pal(
    sp: &BTreeMap<String, Property>,
    _player_uid: Uuid,
    instance_id: Uuid,
    group_id: Uuid,
) -> Pal {
    let character_id = get_str(sp, "CharacterID");
    let is_lucky = get_bool(sp, "IsRarePal");
    let is_boss = character_id.to_uppercase().starts_with("BOSS_") && !is_lucky;
    let is_tower = character_id.starts_with("GYM_");

    // `SlotID` (current spelling) with a legacy `SlotId` fallback, mirroring
    // `pal.py`.
    let slot = sp.get("SlotID").or_else(|| sp.get("SlotId"));
    let storage_id = slot
        .and_then(|s| s.get_child("ContainerId"))
        .and_then(|c| c.get_child("ID"))
        .and_then(struct_guid)
        .map(guid_string)
        .unwrap_or_default();
    let storage_slot = slot
        .and_then(|s| s.get_child("SlotIndex"))
        .and_then(as_int)
        .map(|v| v as i32)
        .unwrap_or(0);

    Pal {
        instance_id: guid_string(instance_id),
        owner_uid: sp
            .get("OwnerPlayerUId")
            .and_then(struct_guid)
            .map(guid_string)
            .unwrap_or_default(),
        character_id,
        nickname: get_str(sp, "NickName"),
        gender: get_enum(sp, "Gender"),
        is_lucky,
        is_boss,
        is_tower,
        storage_id,
        storage_slot,
        level: get_byte(sp, "Level", 1),
        exp: get_int(sp, "Exp", 0),
        rank: get_byte(sp, "Rank", 0),
        rank_hp: get_byte(sp, "Rank_HP", 0),
        rank_attack: get_byte(sp, "Rank_Attack", 0),
        // `Rank_Defence` — British spelling on disk (see model.rs note).
        rank_defense: get_byte(sp, "Rank_Defence", 0),
        rank_craftspeed: get_byte(sp, "Rank_CraftSpeed", 0),
        talent_hp: get_byte(sp, "Talent_HP", 0),
        talent_shot: get_byte(sp, "Talent_Shot", 0),
        talent_defense: get_byte(sp, "Talent_Defense", 0),
        hp: get_fixed_point64(sp, "Hp"),
        // `max_hp` is a scaling-table computation (species base stats) not
        // available from the save alone; left at 0 here.
        max_hp: 0,
        sanity: get_float(sp, "SanityValue", 100.0) as i32,
        stomach: get_float(sp, "FullStomach", 150.0) as i32,
        learned_skills: get_names(sp, "MasteredWaza"),
        active_skills: get_names(sp, "EquipWaza"),
        passive_skills: get_names(sp, "PassiveSkillList"),
        work_suitability: get_work_suitability(sp),
        friendship_point: get_int(sp, "FriendshipPoint", 0),
        group_id: guid_string(group_id),
    }
}

// --- accessors ------------------------------------------------------------
//
// Each mirrors a `PalObjects.get_*` helper. All are total: a missing key or a
// type mismatch yields the supplied default rather than panicking, matching the
// reference's `get_nested(..., default=...)` guards.

/// The `Uuid` inside a `Guid` struct property (`OwnerPlayerUId`, `PlayerUId`, …).
fn struct_guid(p: &Property) -> Option<Uuid> {
    match p {
        Property::Struct {
            value: StructValue::Guid(u),
            ..
        } => Some(*u),
        _ => None,
    }
}

/// Canonical string form for a UID: hyphenated lowercase (`Uuid::to_string`).
fn guid_string(u: Uuid) -> String {
    u.to_string()
}

/// The `i64` of an integer property (`IntProperty`, `Int64Property`, …).
fn as_int(p: &Property) -> Option<i64> {
    match p {
        Property::Int(v) => Some(*v),
        _ => None,
    }
}

/// `StrProperty` / `NameProperty` value, or `""` if absent.
fn get_str(m: &BTreeMap<String, Property>, k: &str) -> String {
    match m.get(k) {
        Some(Property::Str(s)) | Some(Property::Name(s)) => s.clone(),
        _ => String::new(),
    }
}

/// `IntProperty`-family value as `i32` (`get_value`), or `default`.
fn get_int(m: &BTreeMap<String, Property>, k: &str, default: i32) -> i32 {
    m.get(k).and_then(as_int).map(|v| v as i32).unwrap_or(default)
}

/// A "None"-typed `ByteProperty` value as `i32` (`get_byte_property`), or
/// `default`. Labelled byte enums are not expected for these keys.
fn get_byte(m: &BTreeMap<String, Property>, k: &str, default: i32) -> i32 {
    match m.get(k) {
        Some(Property::Byte {
            value: ByteVal::Byte(b),
            ..
        }) => *b as i32,
        _ => default,
    }
}

/// `BoolProperty` value (`get_value`), or `false` if absent.
fn get_bool(m: &BTreeMap<String, Property>, k: &str) -> bool {
    matches!(m.get(k), Some(Property::Bool(true)))
}

/// `FloatProperty` value (`get_value`), or `default`.
fn get_float(m: &BTreeMap<String, Property>, k: &str, default: f64) -> f64 {
    match m.get(k) {
        Some(Property::Float(f)) => *f,
        _ => default,
    }
}

/// `EnumProperty` selected-variant string (`get_enum_property`), or `""`.
fn get_enum(m: &BTreeMap<String, Property>, k: &str) -> String {
    match m.get(k) {
        Some(Property::Enum { value, .. }) => value.clone(),
        _ => String::new(),
    }
}

/// A `FixedPoint64` struct's inner `Value` `Int64Property` (`get_fixed_point64`),
/// or `0`. `Hp` uses this shape; a legacy uppercase `HP` key is also honored.
fn get_fixed_point64(m: &BTreeMap<String, Property>, k: &str) -> i32 {
    m.get(k)
        .or_else(|| m.get("HP"))
        .and_then(|p| p.get_child("Value"))
        .and_then(as_int)
        .map(|v| v as i32)
        .unwrap_or(0)
}

/// The string elements of an `EnumProperty`/`NameProperty` array
/// (`get_array_property`), or an empty list.
fn get_names(m: &BTreeMap<String, Property>, k: &str) -> Vec<String> {
    match m.get(k) {
        Some(Property::Array {
            value: ArrayValue::Names(v),
            ..
        }) => v.clone(),
        _ => Vec::new(),
    }
}

/// `GotWorkSuitabilityAddRankList`: an array of `{ WorkSuitability: Enum, Rank:
/// Int }` structs, flattened to a `work-type → rank` map.
fn get_work_suitability(m: &BTreeMap<String, Property>) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    if let Some(Property::Array {
        value: ArrayValue::Structs { values, .. },
        ..
    }) = m.get("GotWorkSuitabilityAddRankList")
    {
        for v in values {
            if let StructValue::Properties(pm) = v {
                let work = match pm.get("WorkSuitability") {
                    Some(Property::Enum { value, .. }) => value.clone(),
                    _ => continue,
                };
                let rank = pm.get("Rank").and_then(as_int).unwrap_or(0) as i32;
                out.insert(work, rank);
            }
        }
    }
    out
}
