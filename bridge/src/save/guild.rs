//! Decode `GroupSaveDataMap` entries of type `EPalGroupType::Guild` into guilds
//! plus their base summaries.
//!
//! Ports `palworld_save_tools/rawdata/group.py::decode` / `decode_bytes` (the
//! Guild branch) and the field mapping in `palworld-save-pal`'s
//! `game/guild.py` + `mixins/guild_ops.py`.
//!
//! ## Structure (from the reference)
//!
//! `paltypes.py` registers `group.decode` at
//! `.worldSaveData.GroupSaveDataMap.Value.RawData`. Unlike the character map,
//! the *group type* is not inside the blob — the reference reads it from the map
//! value's generic `GroupType` `EnumProperty` and passes it into `decode_bytes`.
//! We do the same: filter each entry by its `GroupType`, and only re-parse the
//! `RawData` byte blob (left as [`Property::Bytes`] by Task 4) for Guild groups.
//!
//! The Guild `RawData` byte layout (`group.py::decode_bytes`, Guild branch) is:
//!
//! ```text
//! group_id                         : guid (16)
//! group_name                       : fstring
//! individual_character_handle_ids  : tarray< guid(16) + instance_id(16) >
//! org_type                         : u8            // Guild ∈ {Guild, IndependentGuild, Organization}
//! leading_bytes                    : 4
//! base_ids                         : tarray< guid(16) >   // the guild's base-camp ids
//! unknown_1                        : i32
//! base_camp_level                  : i32
//! map_object_instance_ids_...      : tarray< guid(16) >
//! guild_name                       : fstring
//! last_guild_name_modifier_uid     : guid (16)
//! guild_markers                    : tarray< marker(60) > // 0 on pre-marker saves
//! <guild tail>                                            // v2-then-v1, see below
//! <EOF>
//! ```
//!
//! The **tail** has two layouts and no version flag, so — exactly as
//! `group.py::_read_guild_tail` — the newer "v2" (2026-07) layout is attempted
//! first and accepted only if it consumes the remaining bytes precisely to EOF;
//! otherwise the pre-update "v1" layout is used:
//!
//! ```text
//! v2: guild_chest_allowed_roles : tarray<byte>
//!     unknown_i32               : i32
//!     admin_player_uid          : guid (16)
//!     players                   : tarray< guid(16) + i64 + fstring + role:byte >
//!     role_permissions          : tarray< byte + tarray<byte> >
//!     trailing_bytes            : 4
//! v1: admin_player_uid          : guid (16)
//!     players                   : tarray< guid(16) + i64 + fstring >
//!     trailing_bytes            : 4
//! ```
//!
//! Only base *summaries* (id) are produced here; a base's internal `ModuleMap`
//! is skip-decoded (Task 4) and deep base contents are out of scope.

use uuid::Uuid;

use super::decompress::SaveError;
use super::model::{Base, Guild};
use super::props::{ByteVal, Property, StructValue};
use super::reader::Reader;

/// Decode a `GroupSaveDataMap` [`Property::Map`] into its guilds. Non-guild
/// groups (`Organization`, `IndependentGuild`, neutral, …) are skipped.
pub fn decode_guilds(map: &Property) -> Result<Vec<Guild>, SaveError> {
    let entries = match map {
        Property::Map { entries, .. } => entries,
        _ => {
            return Err(SaveError::GroupData(
                "GroupSaveDataMap is not a MapProperty".to_string(),
            ))
        }
    };

    let mut guilds = Vec::new();
    for entry in entries {
        if group_type(&entry.value).as_deref() != Some("EPalGroupType::Guild") {
            continue;
        }
        // Guild identity is the map *key* (`palworld_save_pal/game/guild.py`'s
        // `as_uuid(self._group_save_data["key"])`), not the blob's internal
        // `group_id` — the key is the reference-canonical id.
        let key_id = struct_guid(&entry.key)
            .ok_or_else(|| SaveError::GroupData("guild map key is not a Guid".to_string()))?;
        let raw = entry
            .value
            .get_child("RawData")
            .and_then(Property::as_bytes)
            .ok_or_else(|| SaveError::GroupData("guild group missing RawData".to_string()))?;
        guilds.push(decode_guild_bytes(key_id, raw)?);
    }
    Ok(guilds)
}

/// The `Uuid` inside a bare `Guid` struct property (`GroupSaveDataMap`'s map
/// key, per `paltypes.py:46` — a bare `Guid`, unlike the `{ ID: Guid }` /
/// `{ PlayerUId: Guid, InstanceId: Guid }` wrapper structs used elsewhere).
fn struct_guid(p: &Property) -> Option<Uuid> {
    match p {
        Property::Struct {
            value: StructValue::Guid(u),
            ..
        } => Some(*u),
        _ => None,
    }
}

/// The `EPalGroupType::*` string of a group map value, from its generic
/// `GroupType` property (an `EnumProperty`, or a labelled `ByteProperty` on
/// older saves).
fn group_type(value: &Property) -> Option<String> {
    match value.get_child("GroupType")? {
        Property::Enum { value, .. } => Some(value.clone()),
        Property::Byte {
            value: ByteVal::Label(s),
            ..
        } => Some(s.clone()),
        _ => None,
    }
}

/// Re-parse a Guild `RawData` blob into a [`Guild`] (`group.py`, Guild branch).
/// Fails loud if the blob is not consumed exactly to EOF.
///
/// `id` is the `GroupSaveDataMap` entry's map key — the reference-canonical
/// guild identity — and is what ends up in [`Guild::id`]. The blob's own
/// `group_id` is still read (the byte layout requires it), and is expected to
/// agree with `id`; a mismatch is not a fatal error (the key wins), just a
/// signal worth catching in development.
fn decode_guild_bytes(id: Uuid, bytes: &[u8]) -> Result<Guild, SaveError> {
    let mut r = Reader::new(bytes);

    let group_id = r.guid();
    debug_assert_eq!(
        group_id, id,
        "guild RawData group_id disagrees with the GroupSaveDataMap key"
    );
    let _group_name = r.fstring();
    skip_instance_handle_ids(&mut r); // tarray< guid + instance_id >
    let _org_type = r.read_u8(); // Guild is in the org-type-tagged set

    // --- Guild-specific body ------------------------------------------------
    let _leading_bytes = r.read(4);
    let base_ids = read_guid_array(&mut r);
    let _unknown_1 = r.read_i32();
    let base_camp_level = r.read_i32();
    let _base_camp_points = read_guid_array(&mut r);
    let guild_name = r.fstring();
    let _last_name_modifier_uid = r.guid();
    skip_guild_markers(&mut r); // tarray< 60-byte marker >, 0 on old saves

    // --- tail: try the 2026-07 layout, else the pre-update one --------------
    let tail = &bytes[r.pos()..];
    let (admin_player_uid, player_uids) = read_guild_tail(tail)?;

    let bases = base_ids
        .iter()
        .map(|id| Base {
            id: id.to_string(),
            ..Base::default()
        })
        .collect();

    Ok(Guild {
        id: id.to_string(),
        name: guild_name,
        base_camp_level,
        guild_chest: None,
        lab_research: Vec::new(),
        bases,
        players: player_uids.iter().map(Uuid::to_string).collect(),
        admin_player_uid: admin_player_uid.to_string(),
    })
}

// --- fixed-shape array skippers -------------------------------------------

/// `individual_character_handle_ids`: a `tarray` of `instance_id_reader`
/// elements (`guid` + `instance_id`, 32 bytes each). Only the count matters for
/// the guild summary, so the bodies are skipped.
fn skip_instance_handle_ids(r: &mut Reader) {
    let count = r.read_u32() as usize;
    r.skip(count * 32);
}

/// A `tarray< guid >` (`uuid_reader`), collected as canonical `Uuid`s.
fn read_guid_array(r: &mut Reader) -> Vec<Uuid> {
    let count = r.read_u32() as usize;
    (0..count).map(|_| r.guid()).collect()
}

/// `guild_markers`: a `tarray` of `FPalGuildMarkerData` (60 bytes each). The DTO
/// does not model markers, so the bodies are skipped. On saves that predate map
/// markers this reads a zero count (the field's old "4 unknown bytes").
fn skip_guild_markers(r: &mut Reader) {
    let count = r.read_u32() as usize;
    r.skip(count * 60);
}

// --- guild tail (v2-then-v1) ----------------------------------------------

/// Parse the guild tail, returning `(admin_player_uid, member_uids)`.
///
/// Mirrors `group.py::_read_guild_tail`: attempt the newer "v2" layout, accept
/// it only if it lands exactly on EOF, otherwise fall back to the pre-update
/// "v1" layout. The v2 attempt is fully bounds-checked (it returns `None` rather
/// than reading past `tail`), so no version flag or panic-recovery is needed.
fn read_guild_tail(tail: &[u8]) -> Result<(Uuid, Vec<Uuid>), SaveError> {
    if let Some(res) = try_read_guild_tail_v2(tail) {
        return Ok(res);
    }
    read_guild_tail_v1(tail)
}

/// Try the 2026-07 tail. Returns `None` (rejecting v2) if any read would overrun
/// `tail` or the layout does not consume it exactly to the 4 trailing bytes.
fn try_read_guild_tail_v2(tail: &[u8]) -> Option<(Uuid, Vec<Uuid>)> {
    let mut r = Reader::new(tail);
    skip_byte_array_checked(&mut r)?; // guild_chest_allowed_roles
    read_exact_checked(&mut r, 4)?; // unknown_i32
    let admin = read_guid_checked(&mut r)?;
    let players = read_guild_players_checked(&mut r, true)?;
    skip_role_permissions_checked(&mut r)?;
    // Exactly the 4 trailing bytes must remain — the EOF discriminator.
    if r.remaining() != 4 {
        return None;
    }
    Some((admin, players))
}

/// The pre-update tail, read directly (fail-loud on a malformed blob, matching
/// the reference's final "EOF not reached" guard).
fn read_guild_tail_v1(tail: &[u8]) -> Result<(Uuid, Vec<Uuid>), SaveError> {
    let mut r = Reader::new(tail);
    if r.remaining() < 16 {
        return Err(SaveError::GroupData(format!(
            "guild tail too short for admin uid: {} bytes",
            r.remaining()
        )));
    }
    let admin = r.guid();
    let players = read_guild_players_v1(&mut r)?;
    if r.remaining() != 4 {
        return Err(SaveError::GroupData(format!(
            "guild tail not consumed to EOF: {} bytes before/after trailing",
            r.remaining()
        )));
    }
    Ok((admin, players))
}

/// v1 members: `tarray< guid(16) + i64 + fstring >`, collecting the uids.
fn read_guild_players_v1(r: &mut Reader) -> Result<Vec<Uuid>, SaveError> {
    let count = r.read_u32() as usize;
    // Each member is at minimum a 16-byte guid + 8-byte i64 + a 4-byte empty
    // fstring (length-prefix-only, no body) — sanity-check the file-controlled
    // count against the reader's remaining bytes before pre-allocating.
    const MIN_PLAYER_BYTES: usize = 16 + 8 + 4;
    if count > r.remaining() / MIN_PLAYER_BYTES {
        return Err(SaveError::TooLarge);
    }
    let mut uids = Vec::with_capacity(count);
    for _ in 0..count {
        let uid = r.guid();
        let _last_online = r.read_i64();
        let _player_name = r.fstring();
        uids.push(uid);
    }
    Ok(uids)
}

// --- bounds-checked primitives for the speculative v2 attempt --------------

/// Advance past `n` bytes, or `None` if fewer remain.
fn read_exact_checked(r: &mut Reader, n: usize) -> Option<()> {
    if r.remaining() < n {
        return None;
    }
    r.skip(n);
    Some(())
}

/// Read a 16-byte guid, or `None` if fewer remain.
fn read_guid_checked(r: &mut Reader) -> Option<Uuid> {
    if r.remaining() < 16 {
        return None;
    }
    Some(r.guid())
}

/// `tarray<byte>`: a `u32` count then `count` raw bytes, skipped. `None` on
/// overrun.
fn skip_byte_array_checked(r: &mut Reader) -> Option<()> {
    if r.remaining() < 4 {
        return None;
    }
    let count = r.read_u32() as usize;
    read_exact_checked(r, count)
}

/// Advance past one length-prefixed `fstring` without decoding it, or `None` if
/// its declared body would overrun.
fn skip_fstring_checked(r: &mut Reader) -> Option<()> {
    if r.remaining() < 4 {
        return None;
    }
    let len = r.read_i32();
    // Positive: `len` bytes (incl. NUL). Negative: `2*|len|` UTF-16 bytes.
    let body = if len >= 0 {
        len as usize
    } else {
        (len as i64).unsigned_abs() as usize * 2
    };
    read_exact_checked(r, body)
}

/// v2 members: `tarray< guid(16) + i64 + fstring + role:byte >`. Fully
/// bounds-checked; `None` on any overrun. Does not pre-allocate from the
/// (possibly garbage, during the speculative attempt) count.
fn read_guild_players_checked(r: &mut Reader, with_role: bool) -> Option<Vec<Uuid>> {
    if r.remaining() < 4 {
        return None;
    }
    let count = r.read_u32() as usize;
    let mut uids = Vec::new();
    for _ in 0..count {
        let uid = read_guid_checked(r)?;
        read_exact_checked(r, 8)?; // last_online_real_time : i64
        skip_fstring_checked(r)?; // player_name
        if with_role {
            read_exact_checked(r, 1)?; // EPalGuildRole
        }
        uids.push(uid);
    }
    Some(uids)
}

/// v2 `role_permissions`: `tarray< role:byte + tarray<byte> >`. Fully
/// bounds-checked; `None` on any overrun.
fn skip_role_permissions_checked(r: &mut Reader) -> Option<()> {
    if r.remaining() < 4 {
        return None;
    }
    let count = r.read_u32() as usize;
    for _ in 0..count {
        read_exact_checked(r, 1)?; // role
        skip_byte_array_checked(r)?; // permissions
    }
    Some(())
}

/// Decode `GuildExtraSaveDataMap` into a guild-id → guild-chest container-id
/// index.
///
/// Ports the `oMaN-Rod/palworld-save-tools` fork's
/// `rawdata/guild_item_storage.py`: each map entry's key is the guild id (a
/// bare `Guid`), and its value's `GuildItemStorage.RawData` blob is simply a
/// 16-byte container guid (plus optional trailing bytes, ignored on read).
/// Entries without a chest (or with a nil id) are skipped, matching the
/// reference's `Optional[UUID]` behavior.
pub fn decode_guild_chests(
    map: &Property,
) -> Result<std::collections::HashMap<Uuid, Uuid>, SaveError> {
    let entries = match map {
        Property::Map { entries, .. } => entries,
        _ => {
            return Err(SaveError::GroupData(
                "GuildExtraSaveDataMap is not a MapProperty".to_string(),
            ))
        }
    };

    let mut out = std::collections::HashMap::new();
    for entry in entries {
        let Some(guild_id) = struct_guid(&entry.key) else {
            continue;
        };
        let Some(raw) = entry
            .value
            .get_child("GuildItemStorage")
            .and_then(|s| s.get_child("RawData"))
            .and_then(Property::as_bytes)
        else {
            continue;
        };
        if raw.len() < 16 {
            continue;
        }
        let container_id = Reader::new(raw).guid();
        if container_id != Uuid::nil() {
            out.insert(guild_id, container_id);
        }
    }
    Ok(out)
}
