//! Discover each base camp's ground-storage containers (the built chests) from
//! `MapObjectSaveData`.
//!
//! Ports palworld-save-pal's base-storage association. A base's storage boxes
//! are ordinary map objects; each one is tied to a base by
//! `Model.RawData.base_camp_id_belong_to`, and carries its inventory as an
//! `ItemContainer` module under `ConcreteModel.ModuleMap` whose
//! `target_container_id` points into `ItemContainerSaveData` (where `SlotNum` +
//! slots live). Both fields are at **fixed offsets** in their `RawData` blobs
//! (per `palworld-save-tools` `rawdata/map_model.py` and
//! `rawdata/map_concrete_model_module.py`):
//!
//! - `Model.RawData`: `instance_id:guid | concrete_model_instance_id:guid |
//!   base_camp_id_belong_to:guid | …` → the base id is the guid at **offset 32**.
//! - ItemContainer module `RawData`: `target_container_id:guid | …` → the
//!   container id is the guid at **offset 0**.
//!
//! This is the one place we must walk the whole `MapObjectSaveData` array (the
//! largest structure in a world save), but the parser already materialises it,
//! and per object we only read two small fixed-offset GUIDs.

use std::collections::HashMap;

use uuid::Uuid;

use super::props::{ArrayValue, Property, StructValue};
use super::reader::Reader;

const ITEM_CONTAINER_MODULE: &str = "EPalMapObjectConcreteModelModuleType::ItemContainer";

/// Build a `base_id → [storage container id]` index from `MapObjectSaveData`.
/// A non-array property, or objects missing the fields, are skipped rather than
/// failing the load.
pub fn decode_base_storage(map_objects: &Property) -> HashMap<Uuid, Vec<Uuid>> {
    let mut out: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

    let values = match map_objects {
        Property::Array {
            value: ArrayValue::Structs { values, .. },
            ..
        } => values,
        _ => return out,
    };

    for v in values {
        let obj = match v {
            StructValue::Properties(m) => m,
            _ => continue,
        };

        // The base this object belongs to (guid at offset 32 of Model.RawData).
        let Some(base_id) = obj
            .get("Model")
            .and_then(|m| m.get_child("RawData"))
            .and_then(Property::as_bytes)
            .and_then(|raw| guid_at(raw, 32))
        else {
            continue;
        };

        // Its ItemContainer module's target container (guid at offset 0).
        let Some(module_map) = obj
            .get("ConcreteModel")
            .and_then(|c| c.get_child("ModuleMap"))
        else {
            continue;
        };
        let Property::Map { entries, .. } = module_map else {
            continue;
        };
        for entry in entries {
            if key_str(&entry.key) != Some(ITEM_CONTAINER_MODULE) {
                continue;
            }
            if let Some(cid) = entry
                .value
                .get_child("RawData")
                .and_then(Property::as_bytes)
                .and_then(|raw| guid_at(raw, 0))
            {
                out.entry(base_id).or_default().push(cid);
            }
        }
    }
    out
}

/// The 16-byte GUID at `offset` in `raw` (same LE→BE word swap as
/// [`Reader::guid`]), or `None` if it would overrun or is the nil id.
fn guid_at(raw: &[u8], offset: usize) -> Option<Uuid> {
    if raw.len() < offset + 16 {
        return None;
    }
    let id = Reader::new(&raw[offset..]).guid();
    (id != Uuid::nil()).then_some(id)
}

/// The string of a `NameProperty`/`EnumProperty`/`StrProperty` map key.
fn key_str(p: &Property) -> Option<&str> {
    match p {
        Property::Name(s) | Property::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::props::MapEntry;

    fn bytes_prop(blob: Vec<u8>) -> Property {
        Property::Bytes(blob)
    }

    fn struct_props(pairs: Vec<(&str, Property)>) -> Property {
        let map = pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Property::Struct {
            struct_type: "S".to_string(),
            value: StructValue::Properties(map),
        }
    }

    /// A guid whose on-disk word A is `01 00 00 00` → canonical
    /// `00000001-0000-0000-0000-000000000000`.
    fn guid_blob(word_a: u8, len: usize, at: usize) -> Vec<u8> {
        let mut b = vec![0u8; len];
        b[at] = word_a;
        b
    }

    #[test]
    fn associates_container_to_base_by_offsets() {
        // Model.RawData: base id (word-a=7) at offset 32.
        let model = struct_props(vec![("RawData", bytes_prop(guid_blob(7, 48, 32)))]);
        // ItemContainer module RawData: container id (word-a=9) at offset 0.
        let module_val = struct_props(vec![("RawData", bytes_prop(guid_blob(9, 16, 0)))]);
        let module_map = Property::Map {
            key_type: "NameProperty".to_string(),
            value_type: "StructProperty".to_string(),
            entries: vec![MapEntry {
                key: Property::Name(ITEM_CONTAINER_MODULE.to_string()),
                value: module_val,
            }],
        };
        let concrete = struct_props(vec![("ModuleMap", module_map)]);
        let object = StructValue::Properties(
            vec![
                ("Model".to_string(), model),
                ("ConcreteModel".to_string(), concrete),
            ]
            .into_iter()
            .collect(),
        );
        let map_objects = Property::Array {
            array_type: "StructProperty".to_string(),
            value: ArrayValue::Structs {
                prop_name: "MapObjectSaveData".to_string(),
                prop_type: "StructProperty".to_string(),
                type_name: "MapObjectSaveData".to_string(),
                values: vec![object],
            },
        };

        let base_id = Uuid::parse_str("00000007-0000-0000-0000-000000000000").unwrap();
        let container_id = Uuid::parse_str("00000009-0000-0000-0000-000000000000").unwrap();
        let index = decode_base_storage(&map_objects);
        assert_eq!(index.get(&base_id), Some(&vec![container_id]));
    }

    #[test]
    fn non_array_yields_empty() {
        assert!(decode_base_storage(&Property::Int(0)).is_empty());
    }
}
