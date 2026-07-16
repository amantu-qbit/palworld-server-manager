//! Decode `BaseCampSaveData` entries into per-base summaries: display name,
//! `area_range` (the build-radius circle), and world position.
//!
//! Ports the prefix of `palworld-save-tools` `rawdata/base_camp.py::decode_bytes`
//! — everything up to `area_range`, which is all a base summary needs. The
//! `BaseCampSaveData` map key is a **bare `Guid`** (the base id, matching the
//! guild's `base_ids`); its value struct carries a `RawData` `ByteProperty` blob:
//!
//! ```text
//! id                          : guid (16)
//! name                        : fstring
//! state                       : u8 (1)
//! transform                   : FTransform  // quat(4×f64) + translation(3×f64) + scale3d(3×f64) = 80
//! area_range                  : f32 (4)      // the build radius (vanilla 3500)
//! group_id_belong_to          : guid (16)    // the owning guild id
//! fast_travel_local_transform : FTransform (80)
//! owner_map_object_instance_id: guid (16)
//! trailing                    : 4
//! ```
//!
//! Only the prefix through `group_id_belong_to` is read here; the rest is left
//! untouched (it's only needed by the byte-splice writer, which recomputes
//! offsets from the same layout). `area_range` is a fixed-offset `f32` after the
//! variable-length `name` and the fixed 80-byte transform — which is exactly why
//! the writer can overwrite it in place (see `edit::ops::edit_base`).

use std::collections::HashMap;

use uuid::Uuid;

use super::decompress::SaveError;
use super::props::{Property, StructValue};
use super::reader::Reader;

/// The `FTransform` on-disk size: quat (4× f64) + translation (3× f64) +
/// scale3d (3× f64). Palworld stores these as LWC doubles, so this is fixed.
pub const FTRANSFORM_LEN: usize = 8 * (4 + 3 + 3);

/// A base camp summary: its display name, build-area radius, world position
/// (the transform's translation), and the guild it belongs to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseCampInfo {
    /// Player-assigned base name (often empty → the game shows a default).
    pub name: String,
    /// Build-area radius (`area_range`; vanilla default 3500.0).
    pub area_range: f32,
    /// World position (transform translation x/y/z).
    pub position: [f64; 3],
    /// The owning guild id (`group_id_belong_to`) — a cross-check against the
    /// map that references this base.
    pub group_id_belong_to: Uuid,
}

/// Decode `BaseCampSaveData` (a `MapProperty` keyed by base id) into a
/// base-id → [`BaseCampInfo`] index. A blob too short to reach `area_range`
/// (never expected for a real save) is skipped rather than failing the load.
pub fn decode_base_camps(map: &Property) -> Result<HashMap<Uuid, BaseCampInfo>, SaveError> {
    let entries = match map {
        Property::Map { entries, .. } => entries,
        _ => {
            return Err(SaveError::GroupData(
                "BaseCampSaveData is not a MapProperty".to_string(),
            ))
        }
    };

    let mut out = HashMap::new();
    for entry in entries {
        let Some(base_id) = struct_guid(&entry.key) else {
            continue;
        };
        let Some(raw) = entry
            .value
            .get_child("RawData")
            .and_then(Property::as_bytes)
        else {
            continue;
        };
        if let Some(info) = decode_base_camp_bytes(raw) {
            out.insert(base_id, info);
        }
    }
    Ok(out)
}

/// Decode the prefix of a base-camp `RawData` blob through `group_id_belong_to`.
/// Returns `None` when the blob is too short (bounds are checked before each
/// group of reads so a truncated blob yields `None` instead of a panic).
fn decode_base_camp_bytes(bytes: &[u8]) -> Option<BaseCampInfo> {
    // id (16) + at least a 4-byte fstring length prefix.
    if bytes.len() < 16 + 4 {
        return None;
    }
    let mut r = Reader::new(bytes);
    let _id = r.guid();

    // `name` is a variable-length fstring; guard its declared length before the
    // reader would panic on a truncated body.
    if !fstring_fits(bytes, r.pos()) {
        return None;
    }
    let name = r.fstring();

    // state(1) + transform(80) + area_range(4) + group_id(16) must all fit.
    if r.remaining() < 1 + FTRANSFORM_LEN + 4 + 16 {
        return None;
    }
    let _state = r.read_u8();
    r.skip(8 * 4); // FTransform.rotation (quat: 4× f64)
    let x = r.read_f64();
    let y = r.read_f64();
    let z = r.read_f64();
    r.skip(8 * 3); // FTransform.scale3d (3× f64)
    let area_range = r.read_f32();
    let group_id_belong_to = r.guid();

    Some(BaseCampInfo {
        name,
        area_range,
        position: [x, y, z],
        group_id_belong_to,
    })
}

/// True if the fstring whose length prefix starts at `pos` fits within `bytes`.
/// Mirrors [`Reader::fstring`]'s length convention (positive = 1 byte/char incl.
/// NUL; negative = 2 bytes/UTF-16 unit).
fn fstring_fits(bytes: &[u8], pos: usize) -> bool {
    if bytes.len().saturating_sub(pos) < 4 {
        return false;
    }
    let len = i32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
    let body = if len >= 0 {
        len as usize
    } else {
        (len as i64).unsigned_abs() as usize * 2
    };
    bytes.len().saturating_sub(pos + 4) >= body
}

/// The `Uuid` inside a bare `Guid` struct property (the `BaseCampSaveData` key).
fn struct_guid(p: &Property) -> Option<Uuid> {
    match p {
        Property::Struct {
            value: StructValue::Guid(u),
            ..
        } => Some(*u),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a positive-length (ASCII) fstring: an i32 byte count incl. the
    /// trailing NUL, then the bytes, then the NUL.
    fn fstr(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&((s.len() as i32) + 1).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
        out
    }

    /// Build a minimal base-camp blob with a known name/position/area_range and
    /// confirm the decoder reads the fixed-offset `area_range` correctly after a
    /// variable-length name + the 80-byte transform.
    #[test]
    fn decodes_area_range_after_name_and_transform() {
        let mut blob = Vec::new();
        blob.extend_from_slice(&[0u8; 16]); // id
        blob.extend(fstr("Home Base")); // name (variable length)
        blob.push(3); // state
        blob.extend_from_slice(&[0u8; 8 * 4]); // rotation quat
        blob.extend_from_slice(&1000.0f64.to_le_bytes()); // translation.x
        blob.extend_from_slice(&2000.0f64.to_le_bytes()); // translation.y
        blob.extend_from_slice(&300.0f64.to_le_bytes()); // translation.z
        blob.extend_from_slice(&[0u8; 8 * 3]); // scale3d
        blob.extend_from_slice(&3500.0f32.to_le_bytes()); // area_range
        blob.extend_from_slice(&[0u8; 16]); // group_id_belong_to

        let info = decode_base_camp_bytes(&blob).expect("decodes");
        assert_eq!(info.name, "Home Base");
        assert_eq!(info.area_range, 3500.0);
        assert_eq!(info.position, [1000.0, 2000.0, 300.0]);
    }

    #[test]
    fn truncated_blob_yields_none_not_panic() {
        assert!(decode_base_camp_bytes(&[0u8; 10]).is_none());
        // id + name only, nothing after → too short for state+transform+area.
        let mut blob = vec![0u8; 16];
        blob.extend(fstr("x"));
        assert!(decode_base_camp_bytes(&blob).is_none());
    }
}
