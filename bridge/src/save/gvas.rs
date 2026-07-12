//! GVAS envelope parser + generic UE property-tree reader.
//!
//! Ports the envelope from `palworld_save_tools/gvas.py::GvasHeader` /
//! `GvasFile.read` and the generic property (de)serialization from
//! `palworld_save_tools/archive.py::FArchiveReader` (`property`,
//! `properties_until_end`, `struct`/`struct_value`, `array_property`,
//! `_read_MapProperty`, `_read_SetProperty`, `prop_value`).
//!
//! Two deliberate departures from the reference, both required by Task 4's
//! scope:
//!
//! 1. **No Palworld custom decoders.** `palworld_save_tools`/`palworld-save-pal`
//!    register per-path custom readers for the heavy `RawData` blobs
//!    (character, item container, group, …). Every one of those readers first
//!    calls `reader.property(...)` to consume the property *generically*, then
//!    reinterprets the already-read bytes through a separate in-memory reader —
//!    so a generic-only parse consumes byte-identical input and simply leaves
//!    each `RawData` as its `ByteProperty` array (→ [`Property::Bytes`]). Tasks
//!    6–8 re-parse those bytes.
//!
//! 2. **Skip-decode contract.** A handful of blobs (see [`default_skip_set`])
//!    are *not* valid generic GVAS beyond their property header; the reference
//!    reads their header then grabs the declared `size` bytes verbatim
//!    (`palworld-save-pal`'s `gvas_codec.skip_decode`). We do the same and store
//!    the payload as [`Property::Bytes`].
//!
//! The `type_hints` used by `_read_MapProperty` / `_read_SetProperty` to resolve
//! struct key/value types are ported verbatim in [`type_hint`]; without them the
//! map key/value struct dispatch would desync from the reference.

use std::collections::{BTreeMap, HashSet};

use uuid::Uuid;

use super::decompress::SaveError;
use super::props::{ArrayValue, ByteVal, MapEntry, Property, SetValue, StructValue};
use super::reader::Reader;

/// GVAS file-type tag: the little-endian `i32` reading of the ASCII bytes
/// `GVAS`.
const GVAS_MAGIC: i32 = 0x5341_5647;

/// A parsed GVAS file: the opaque envelope header plus the root property set.
#[derive(Debug, Clone)]
pub struct Gvas {
    /// Raw envelope bytes (magic through save-game class name). Retained
    /// verbatim so a future writer can round-trip the header unchanged.
    pub header: Vec<u8>,
    /// The root property set, keyed by property name.
    pub root: BTreeMap<String, Property>,
}

/// A set of dotted property paths whose values are stored verbatim as
/// [`Property::Bytes`] instead of being decoded (the "verbatim bytes"
/// contract). Paths use the same leading-dot convention as the reference,
/// e.g. `.worldSaveData.GameTimeSaveData`.
pub type SkipSet = HashSet<String>;

/// The default skip set: the ten heavy blobs that `palworld-save-pal`'s
/// `gvas_codec` registers with `skip_decode`.
pub fn default_skip_set() -> SkipSet {
    [
        ".worldSaveData.FoliageGridSaveDataMap",
        ".worldSaveData.MapObjectSpawnerInStageSaveData",
        ".worldSaveData.DungeonSaveData",
        ".worldSaveData.EnemyCampSaveData",
        ".worldSaveData.InvaderSaveData",
        ".worldSaveData.DungeonPointMarkerSaveData",
        ".worldSaveData.GameTimeSaveData",
        ".worldSaveData.OilrigSaveData",
        ".worldSaveData.SupplySaveData",
        ".worldSaveData.BaseCampSaveData.Value.ModuleMap",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Parse a decompressed GVAS byte buffer into its envelope + property tree.
///
/// `raw` is the output of [`super::decompress::decompress_sav`]. `skip` selects
/// the properties kept as verbatim [`Property::Bytes`].
pub fn parse_gvas(raw: &[u8], skip: &SkipSet) -> Result<Gvas, SaveError> {
    let mut r = Reader::new(raw);
    read_header(&mut r)?;
    let header = raw[..r.pos()].to_vec();
    let root = read_properties_until_end(&mut r, "", skip)?;
    Ok(Gvas { header, root })
}

/// Consume the GVAS envelope header (`GvasHeader.read`), advancing the cursor to
/// the start of the root property set. Only the file-type tag is validated;
/// version/engine fields are read to advance the cursor but not asserted, so a
/// point-release bump does not make an otherwise-parseable save fail.
fn read_header(r: &mut Reader) -> Result<(), SaveError> {
    if r.read_i32() != GVAS_MAGIC {
        return Err(SaveError::BadGvasMagic);
    }
    let _save_game_version = r.read_i32();
    let _package_file_version_ue4 = r.read_i32();
    let _package_file_version_ue5 = r.read_i32();
    let _engine_major = r.read_u16();
    let _engine_minor = r.read_u16();
    let _engine_patch = r.read_u16();
    let _engine_changelist = r.read_u32();
    let _engine_branch = r.fstring();
    let _custom_version_format = r.read_i32();
    // CustomVersions: a `tarray` of (guid, i32) pairs.
    let custom_version_count = r.read_u32();
    for _ in 0..custom_version_count {
        let _ = r.guid();
        let _ = r.read_i32();
    }
    let _save_game_class_name = r.fstring();
    Ok(())
}

/// Read a property set until the `"None"` terminator name
/// (`properties_until_end`). Each property is keyed by name; its dotted path is
/// `{path}.{name}`.
///
/// Exposed to sibling decoders (e.g. [`super::character`]) so they can re-parse a
/// Palworld `RawData` blob as an inner GVAS property stream — mirroring how the
/// reference `character.decode` wraps a fresh reader around the raw bytes and
/// calls `properties_until_end`.
pub(crate) fn read_properties_until_end(
    r: &mut Reader,
    path: &str,
    skip: &SkipSet,
) -> Result<BTreeMap<String, Property>, SaveError> {
    let mut props = BTreeMap::new();
    loop {
        let name = r.fstring();
        if name == "None" {
            break;
        }
        let type_name = r.fstring();
        let size = r.read_u64();
        let child_path = format!("{path}.{name}");
        let prop = read_property(r, &type_name, size, &child_path, skip)?;
        props.insert(name, prop);
    }
    Ok(props)
}

/// Dispatch a single property by type (`FArchiveReader.property`), honoring the
/// skip-decode contract before the type dispatch.
fn read_property(
    r: &mut Reader,
    type_name: &str,
    size: u64,
    path: &str,
    skip: &SkipSet,
) -> Result<Property, SaveError> {
    if skip.contains(path) {
        return skip_decode(r, type_name, size, path);
    }
    match type_name {
        "StructProperty" => read_struct(r, path, skip),
        "IntProperty" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_i32() as i64))
        }
        "Int64Property" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_i64()))
        }
        "UInt16Property" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_u16() as i64))
        }
        "UInt32Property" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_u32() as i64))
        }
        "UInt64Property" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_u64() as i64))
        }
        "FixedPoint64Property" => {
            let _ = r.optional_guid();
            Ok(Property::Int(r.read_i32() as i64))
        }
        "FloatProperty" => {
            let _ = r.optional_guid();
            Ok(Property::Float(r.read_f32() as f64))
        }
        "StrProperty" => {
            let _ = r.optional_guid();
            Ok(Property::Str(r.fstring()))
        }
        "NameProperty" => {
            let _ = r.optional_guid();
            Ok(Property::Name(r.fstring()))
        }
        "EnumProperty" => {
            let enum_type = r.fstring();
            let _ = r.optional_guid();
            let value = r.fstring();
            Ok(Property::Enum { enum_type, value })
        }
        "BoolProperty" => {
            let value = r.read_bool();
            let _ = r.optional_guid();
            Ok(Property::Bool(value))
        }
        "ByteProperty" => {
            let enum_type = r.fstring();
            let _ = r.optional_guid();
            let value = if enum_type == "None" {
                ByteVal::Byte(r.read_u8())
            } else {
                ByteVal::Label(r.fstring())
            };
            Ok(Property::Byte { enum_type, value })
        }
        "ArrayProperty" => read_array(r, size, path, skip),
        "MapProperty" => read_map(r, path, skip),
        "SetProperty" => read_set(r, path, skip),
        other => Err(SaveError::UnhandledType(other.to_string(), path.to_string())),
    }
}

/// Read a full `StructProperty` header (`struct`): type name, mandatory struct
/// GUID, optional GUID, then the struct value.
fn read_struct(r: &mut Reader, path: &str, skip: &SkipSet) -> Result<Property, SaveError> {
    let struct_type = r.fstring();
    let _struct_id = r.guid();
    let _id = r.optional_guid();
    let value = read_struct_value(r, &struct_type, path, skip)?;
    Ok(Property::Struct { struct_type, value })
}

/// Read a bare struct value (`struct_value`): well-known primitive struct types
/// decode to their fields; any other type is a nested property set.
fn read_struct_value(
    r: &mut Reader,
    struct_type: &str,
    path: &str,
    skip: &SkipSet,
) -> Result<StructValue, SaveError> {
    Ok(match struct_type {
        "Vector" => StructValue::Vector {
            x: r.read_f64(),
            y: r.read_f64(),
            z: r.read_f64(),
        },
        "Quat" => StructValue::Quat {
            x: r.read_f64(),
            y: r.read_f64(),
            z: r.read_f64(),
            w: r.read_f64(),
        },
        "LinearColor" => StructValue::LinearColor {
            r: r.read_f32(),
            g: r.read_f32(),
            b: r.read_f32(),
            a: r.read_f32(),
        },
        "Color" => StructValue::Color {
            b: r.read_u8(),
            g: r.read_u8(),
            r: r.read_u8(),
            a: r.read_u8(),
        },
        "DateTime" => StructValue::DateTime(r.read_u64()),
        "Guid" => StructValue::Guid(r.guid()),
        _ => StructValue::Properties(read_properties_until_end(r, path, skip)?),
    })
}

/// Minimum on-disk bytes a single [`StructValue`] of `struct_type` can
/// occupy. Used only to sanity-check a file-controlled `u32` element count
/// against the reader's remaining bytes *before* pre-allocating a `Vec` for
/// that many elements (Phase-1b hardening, review finding S2) — mirrors the
/// `count > r.remaining() / 4` guard in `containers.rs::parse_weapon_body`.
/// Known primitive structs have a fixed encoded size; any other struct type
/// is a nested property set, whose smallest possible instance is a single
/// `"None"` terminator name (a 4-byte length prefix + 5-byte ASCII body).
fn min_struct_value_bytes(struct_type: &str) -> usize {
    match struct_type {
        "Vector" => 24,
        "Quat" => 32,
        "LinearColor" => 16,
        "Color" => 4,
        "DateTime" => 8,
        "Guid" => 16,
        _ => 9,
    }
}

/// Minimum on-disk bytes a single bare `prop_value` of `type_name` can
/// occupy (mirrors [`read_prop_value`]'s dispatch). Same purpose as
/// [`min_struct_value_bytes`], for bounding a `MapProperty` entry count.
fn min_prop_value_bytes(type_name: &str, struct_type: Option<&str>) -> usize {
    match type_name {
        "StructProperty" => min_struct_value_bytes(struct_type.unwrap_or("StructProperty")),
        "EnumProperty" | "NameProperty" | "StrProperty" => 4,
        "IntProperty" | "UInt32Property" => 4,
        "Int64Property" => 8,
        "BoolProperty" => 1,
        _ => 1,
    }
}

/// Ceiling on the actual in-memory bytes a single count-driven
/// `Vec::with_capacity(count)` call may request. The `remaining()`-guards above
/// ([`min_struct_value_bytes`] / [`min_prop_value_bytes`]) bound `count`
/// against the smallest possible *on-disk* encoding of one element, but
/// `Vec::with_capacity(count)` allocates `count * size_of::<Elem>()` bytes of
/// *in-memory* storage — and in-memory element sizes can be far larger than
/// their smallest on-disk encoding (e.g. a [`MapEntry`] is hundreds of bytes,
/// while a `BoolProperty`/`BoolProperty` map entry's smallest on-disk encoding
/// is 2 bytes). A spec-valid file that fits comfortably under
/// `decompress::MAX_DECOMPRESSED` can still declare a `count` that would
/// request tens of gigabytes here, so this second, size-aware guard is
/// necessary in addition to (not instead of) the on-disk `remaining()`-guards.
const MAX_ELEM_ALLOC_BYTES: usize = 256 * 1024 * 1024;

/// Reject `count` if `Vec::<Elem>::with_capacity(count)` would request more
/// than [`MAX_ELEM_ALLOC_BYTES`] bytes, where `Elem` is the real in-memory type
/// the `Vec` at that call site collects (not its on-disk encoding). Call this
/// alongside, not instead of, the existing `remaining()`-guard at each
/// count-driven allocation site — defense in depth against an implausible
/// on-disk length and an implausible in-memory allocation, respectively.
fn check_elem_alloc<Elem>(count: usize) -> Result<(), SaveError> {
    if count.saturating_mul(std::mem::size_of::<Elem>()) > MAX_ELEM_ALLOC_BYTES {
        return Err(SaveError::TooLarge);
    }
    Ok(())
}

/// Read an `ArrayProperty` (`_read_ArrayProperty` + `array_property` +
/// `array_value`). A `ByteProperty` array collapses to [`Property::Bytes`]; a
/// `StructProperty` array reads its shared inner struct header once.
fn read_array(
    r: &mut Reader,
    size: u64,
    path: &str,
    skip: &SkipSet,
) -> Result<Property, SaveError> {
    let array_type = r.fstring();
    let _id = r.optional_guid();
    // `size` counts from here (after array_type + optional_guid); the reference
    // passes `size - 4` past the count word to `array_property`.
    let body_size = size.saturating_sub(4) as usize;
    let count = r.read_u32() as usize;

    if array_type == "StructProperty" {
        let prop_name = r.fstring();
        let prop_type = r.fstring();
        let _inner_size = r.read_u64();
        let type_name = r.fstring();
        let _inner_id = r.guid();
        r.skip(1);
        let elem_path = format!("{path}.{prop_name}");
        let min_elem = min_struct_value_bytes(&type_name);
        if count > r.remaining() / min_elem {
            return Err(SaveError::TooLarge);
        }
        check_elem_alloc::<StructValue>(count)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read_struct_value(r, &type_name, &elem_path, skip)?);
        }
        return Ok(Property::Array {
            array_type,
            value: ArrayValue::Structs {
                prop_name,
                prop_type,
                type_name,
                values,
            },
        });
    }

    match array_type.as_str() {
        "ByteProperty" => {
            // Fast path: raw byte blob (body_size == count). Labelled byte
            // arrays are not produced by the reference reader.
            if body_size != count {
                return Err(SaveError::UnhandledType(
                    "labelled ByteProperty array".to_string(),
                    path.to_string(),
                ));
            }
            Ok(Property::Bytes(r.read(count).to_vec()))
        }
        "EnumProperty" | "NameProperty" => {
            // Each element is at minimum an empty fstring (a 4-byte length
            // prefix with no body).
            if count > r.remaining() / 4 {
                return Err(SaveError::TooLarge);
            }
            check_elem_alloc::<String>(count)?;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(r.fstring());
            }
            Ok(Property::Array {
                array_type,
                value: ArrayValue::Names(values),
            })
        }
        "Guid" => {
            // Each element is a fixed 16-byte guid.
            if count > r.remaining() / 16 {
                return Err(SaveError::TooLarge);
            }
            check_elem_alloc::<Uuid>(count)?;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(r.guid());
            }
            Ok(Property::Array {
                array_type,
                value: ArrayValue::Guids(values),
            })
        }
        other => Err(SaveError::UnhandledType(
            format!("ArrayProperty<{other}>"),
            path.to_string(),
        )),
    }
}

/// Read a `SetProperty` (`_read_SetProperty`).
fn read_set(r: &mut Reader, path: &str, skip: &SkipSet) -> Result<Property, SaveError> {
    let set_type = r.fstring();
    let _id = r.optional_guid();
    let _padding = r.read_u32();
    let count = r.read_u32() as usize;

    if set_type == "StructProperty" {
        let struct_type = type_hint(&format!("{path}.StructProperty"))
            .unwrap_or("StructProperty")
            .to_string();
        let elem_path = format!("{path}.StructProperty");
        let min_elem = min_struct_value_bytes(&struct_type);
        if count > r.remaining() / min_elem {
            return Err(SaveError::TooLarge);
        }
        check_elem_alloc::<StructValue>(count)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read_struct_value(r, &struct_type, &elem_path, skip)?);
        }
        Ok(Property::Set {
            set_type,
            value: SetValue::Structs {
                struct_type,
                values,
            },
        })
    } else {
        // Non-struct sets: each element is a nested property set. The reference
        // resets the path to "" here. Its smallest possible instance is a
        // single "None" terminator name (4-byte length prefix + 5-byte body).
        if count > r.remaining() / 9 {
            return Err(SaveError::TooLarge);
        }
        check_elem_alloc::<BTreeMap<String, Property>>(count)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read_properties_until_end(r, "", skip)?);
        }
        Ok(Property::Set {
            set_type,
            value: SetValue::Properties(values),
        })
    }
}

/// Read a `MapProperty` (`_read_MapProperty`). Struct key/value types are
/// resolved through [`type_hint`] with the reference's defaults (`Guid` for
/// keys, `StructProperty` for values).
fn read_map(r: &mut Reader, path: &str, skip: &SkipSet) -> Result<Property, SaveError> {
    let key_type = r.fstring();
    let value_type = r.fstring();
    let _id = r.optional_guid();
    let _padding = r.read_u32();
    let count = r.read_u32() as usize;

    let key_path = format!("{path}.Key");
    let key_struct_type = if key_type == "StructProperty" {
        Some(type_hint(&key_path).unwrap_or("Guid").to_string())
    } else {
        None
    };
    let value_path = format!("{path}.Value");
    let value_struct_type = if value_type == "StructProperty" {
        Some(type_hint(&value_path).unwrap_or("StructProperty").to_string())
    } else {
        None
    };

    let min_entry = min_prop_value_bytes(&key_type, key_struct_type.as_deref())
        + min_prop_value_bytes(&value_type, value_struct_type.as_deref());
    if count > r.remaining() / min_entry {
        return Err(SaveError::TooLarge);
    }
    check_elem_alloc::<MapEntry>(count)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let key = read_prop_value(r, &key_type, key_struct_type.as_deref(), &key_path, skip)?;
        let value = read_prop_value(
            r,
            &value_type,
            value_struct_type.as_deref(),
            &value_path,
            skip,
        )?;
        entries.push(MapEntry { key, value });
    }
    Ok(Property::Map {
        key_type,
        value_type,
        entries,
    })
}

/// Read a bare map key/value (`prop_value`): no property header, just the value
/// for the given type.
fn read_prop_value(
    r: &mut Reader,
    type_name: &str,
    struct_type: Option<&str>,
    path: &str,
    skip: &SkipSet,
) -> Result<Property, SaveError> {
    match type_name {
        "StructProperty" => {
            let struct_type = struct_type.unwrap_or("StructProperty").to_string();
            let value = read_struct_value(r, &struct_type, path, skip)?;
            Ok(Property::Struct { struct_type, value })
        }
        "EnumProperty" | "NameProperty" => Ok(Property::Name(r.fstring())),
        "StrProperty" => Ok(Property::Str(r.fstring())),
        "IntProperty" => Ok(Property::Int(r.read_i32() as i64)),
        "Int64Property" => Ok(Property::Int(r.read_i64())),
        "UInt32Property" => Ok(Property::Int(r.read_u32() as i64)),
        "BoolProperty" => Ok(Property::Bool(r.read_bool())),
        other => Err(SaveError::UnhandledType(other.to_string(), path.to_string())),
    }
}

/// The skip-decode reader (`palworld-save-pal`'s `gvas_codec.skip_decode`):
/// consume only the property header, then grab the declared `size` bytes
/// verbatim into [`Property::Bytes`].
fn skip_decode(
    r: &mut Reader,
    type_name: &str,
    size: u64,
    path: &str,
) -> Result<Property, SaveError> {
    match type_name {
        "ArrayProperty" => {
            let _array_type = r.fstring();
            let _id = r.optional_guid();
        }
        "MapProperty" => {
            let _key_type = r.fstring();
            let _value_type = r.fstring();
            let _id = r.optional_guid();
        }
        "StructProperty" => {
            let _struct_type = r.fstring();
            let _struct_id = r.guid();
            let _id = r.optional_guid();
        }
        other => {
            return Err(SaveError::UnhandledType(
                format!("skip_decode {other}"),
                path.to_string(),
            ))
        }
    }
    Ok(Property::Bytes(r.read(size as usize).to_vec()))
}

/// The struct type hint for a dotted path (`PALWORLD_TYPE_HINTS`). Used only to
/// resolve struct key/value types inside `MapProperty` / `SetProperty`; a
/// missing hint falls back to the caller's default.
fn type_hint(path: &str) -> Option<&'static str> {
    let hint = match path {
        ".worldSaveData.CharacterContainerSaveData.Key" => "StructProperty",
        ".worldSaveData.CharacterSaveParameterMap.Key" => "StructProperty",
        ".worldSaveData.CharacterSaveParameterMap.Value" => "StructProperty",
        ".worldSaveData.FoliageGridSaveDataMap.Key" => "StructProperty",
        ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value" => "StructProperty",
        ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.InstanceDataMap.Key" => {
            "StructProperty"
        }
        ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.InstanceDataMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.FoliageGridSaveDataMap.Value" => "StructProperty",
        ".worldSaveData.ItemContainerSaveData.Key" => "StructProperty",
        ".worldSaveData.MapObjectSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.MapObjectSaveData.MapObjectSaveData.Model.EffectMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.MapObjectSpawnerInStageSaveData.Key" => "StructProperty",
        ".worldSaveData.MapObjectSpawnerInStageSaveData.Value" => "StructProperty",
        ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Key" => {
            "Guid"
        }
        ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Value" => {
            "StructProperty"
        }
        ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Value.ItemMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.WorkSaveData.WorkSaveData.WorkAssignMap.Value" => "StructProperty",
        ".worldSaveData.BaseCampSaveData.Key" => "Guid",
        ".worldSaveData.BaseCampSaveData.Value" => "StructProperty",
        ".worldSaveData.BaseCampSaveData.Value.ModuleMap.Value" => "StructProperty",
        ".worldSaveData.ItemContainerSaveData.Value" => "StructProperty",
        ".worldSaveData.CharacterContainerSaveData.Value" => "StructProperty",
        ".worldSaveData.GroupSaveDataMap.Key" => "Guid",
        ".worldSaveData.GroupSaveDataMap.Value" => "StructProperty",
        ".worldSaveData.EnemyCampSaveData.EnemyCampStatusMap.Value" => "StructProperty",
        ".worldSaveData.DungeonSaveData.DungeonSaveData.MapObjectSaveData.MapObjectSaveData.Model.EffectMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.DungeonSaveData.DungeonSaveData.MapObjectSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value" => {
            "StructProperty"
        }
        ".worldSaveData.InvaderSaveData.Key" => "Guid",
        ".worldSaveData.InvaderSaveData.Value" => "StructProperty",
        ".worldSaveData.OilrigSaveData.OilrigMap.Value" => "StructProperty",
        ".worldSaveData.InvaderDeclarationSaveData.ValidatedStartPointIds.StructProperty" => "Guid",
        ".worldSaveData.SupplySaveData.SupplyInfos.Key" => "Guid",
        ".worldSaveData.SupplySaveData.SupplyInfos.Value" => "StructProperty",
        ".worldSaveData.GuildExtraSaveDataMap.Key" => "Guid",
        ".worldSaveData.GuildExtraSaveDataMap.Value" => "StructProperty",
        ".worldSaveData.EnemyCampSaveData.EnemyCampStatusMap.Value.TreasureBoxInfoMapBySpawnerName.Value" => {
            "StructProperty"
        }
        ".worldSaveData.DungeonSaveData.DungeonSaveData.RewardSaveDataMap.Key" => "Guid",
        ".worldSaveData.DungeonSaveData.DungeonSaveData.RewardSaveDataMap.Value" => "StructProperty",
        ".SaveData.Local_MaxFriendshipPalIds.Key" => "Guid",
        ".SaveData.Local_MaxFriendshipPalIds.Value" => "StructProperty",
        _ => return None,
    };
    Some(hint)
}
