//! Decode `ItemContainerSaveData`, `CharacterContainerSaveData`, and
//! `DynamicItemSaveData` from a decoded `Level.sav`, plus resolve a player's
//! container references from their per-player `<UID>.sav`.
//!
//! ## Reference codecs (ported faithfully)
//!
//! `paltypes.py` registers custom decoders at these dotted paths; each one first
//! reads the property generically (leaving the payload as a `ByteProperty`
//! array → [`Property::Bytes`]) and then re-parses those bytes through a fresh
//! reader — exactly the nested-`RawData` pattern used by Task 6's `character.rs`.
//!
//! - `.worldSaveData.ItemContainerSaveData.Value.Slots.Slots.RawData`
//!   → `rawdata/item_container_slots.py`:
//!   ```text
//!   slot_index : i32
//!   count      : i32
//!   static_id  : fstring
//!   dynamic_id : { created_world_id: guid, local_id_in_created_world: guid }
//!   (trailing bytes, ignored)
//!   ```
//! - `.worldSaveData.CharacterContainerSaveData.Value.Slots.Slots.RawData`
//!   → `rawdata/character_container.py`:
//!   ```text
//!   player_uid          : guid
//!   instance_id         : guid   // the pal instance occupying this slot
//!   permission_tribe_id : u8
//!   (optional trailing bytes, ignored)
//!   ```
//! - `DynamicItemSaveData[*].RawData` → `rawdata/dynamic_item.py`:
//!   ```text
//!   created_world_id : guid
//!   local_id         : guid
//!   static_id        : fstring
//!   // then an armor / weapon / egg body (see `decode_dynamic_item_bytes`)
//!   ```
//!
//! The item/character container *values* themselves are generic GVAS structs
//! (`SlotNum` int + `Slots` struct-array); only the per-slot `RawData` blobs use
//! the custom codecs above.

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;

use super::decompress::{decompress_sav, SaveError};
use super::gvas::{default_skip_set, parse_gvas};
use super::model::{DynamicItem, ItemContainer, ItemContainerSlot};
use super::props::{ArrayValue, Property, StructValue};
use super::reader::Reader;

/// A character-container slot occupied by a pal: the slot index plus the pal
/// instance id stored there. Empty slots (all-zero instance id) are omitted.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotRef {
    /// Slot index within the container.
    pub slot_index: i32,
    /// The pal instance id occupying the slot (canonical hyphenated form).
    pub pal_id: String,
}

/// The seven container ids a player references, resolved from its per-player
/// `<UID>.sav` (`SaveData.PalStorageContainerId` / `OtomoCharacterContainerId`
/// and the five `InventoryInfo.*ContainerId`s). Mirrors `player.py`'s
/// `pal_box_id` / `otomo_container_id` / `_load_inventory`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerContainerIds {
    /// Pal-box character container (`PalStorageContainerId`).
    pub pal_storage: String,
    /// Party character container (`OtomoCharacterContainerId`).
    pub otomo: String,
    /// Primary/common item container (`CommonContainerId`).
    pub common: String,
    /// Essential item container (`EssentialContainerId`).
    pub essential: String,
    /// Weapon-loadout item container (`WeaponLoadOutContainerId`).
    pub weapon_loadout: String,
    /// Equipped-armor item container (`PlayerEquipArmorContainerId`).
    pub player_equip_armor: String,
    /// Food-equip item container (`FoodEquipContainerId`).
    pub food_equip: String,
}

/// Decode `DynamicItemSaveData` into a `local_id → DynamicItem` index.
///
/// The property is an `ArrayProperty<StructProperty>`; each struct carries a
/// `RawData` byte blob decoded by [`decode_dynamic_item_bytes`]. The index key
/// is the item's `local_id_in_created_world`.
pub fn decode_dynamic_items(prop: &Property) -> Result<HashMap<Uuid, DynamicItem>, SaveError> {
    let mut out = HashMap::new();
    let values = match prop {
        Property::Array {
            value: ArrayValue::Structs { values, .. },
            ..
        } => values,
        _ => {
            return Err(SaveError::ContainerData(
                "DynamicItemSaveData is not a StructProperty array".to_string(),
            ))
        }
    };
    for v in values {
        let m = match v {
            StructValue::Properties(m) => m,
            _ => continue,
        };
        let raw = m
            .get("RawData")
            .and_then(Property::as_bytes)
            .ok_or_else(|| SaveError::ContainerData("dynamic item missing RawData".to_string()))?;
        if let Some((local_id, item)) = decode_dynamic_item_bytes(raw)? {
            out.insert(local_id, item);
        }
    }
    Ok(out)
}

/// Decode `ItemContainerSaveData` into a `container_id → ItemContainer` map.
///
/// Each map entry's key struct carries the container `ID` (a `Guid`); the value
/// struct carries `SlotNum` and a `Slots` struct-array whose per-slot `RawData`
/// blobs are decoded by [`decode_item_slot_bytes`]. A slot whose item references
/// a non-empty `local_id` is resolved against `dynamic_items`.
pub fn decode_item_containers(
    map: &Property,
    dynamic_items: &HashMap<Uuid, DynamicItem>,
) -> Result<HashMap<Uuid, ItemContainer>, SaveError> {
    let entries = as_map_entries(map, "ItemContainerSaveData")?;
    let mut out = HashMap::new();

    for entry in entries {
        let id = container_id(&entry.key)?;
        let value = entry.value.as_properties().ok_or_else(|| {
            SaveError::ContainerData("ItemContainerSaveData value is not a struct".to_string())
        })?;
        let slot_num = value.get("SlotNum").and_then(as_i32).unwrap_or(0);

        let mut slots = Vec::new();
        for slot_struct in struct_array(value.get("Slots")) {
            let raw = slot_struct
                .get("RawData")
                .and_then(Property::as_bytes)
                .ok_or_else(|| {
                    SaveError::ContainerData("item container slot missing RawData".to_string())
                })?;
            if let Some(slot) = decode_item_slot_bytes(raw, dynamic_items)? {
                slots.push(slot);
            }
        }

        out.insert(
            id,
            ItemContainer {
                id: id.to_string(),
                container_type: String::new(),
                key: String::new(),
                slot_num,
                slots,
            },
        );
    }
    Ok(out)
}

/// Decode `CharacterContainerSaveData` into a `container_id → Vec<SlotRef>` map
/// (occupied slots only). This is the pal-box / party / base storage layout: a
/// container's slots list which pal instance is in each slot.
pub fn decode_character_containers(
    map: &Property,
) -> Result<HashMap<Uuid, Vec<SlotRef>>, SaveError> {
    let entries = as_map_entries(map, "CharacterContainerSaveData")?;
    let mut out = HashMap::new();

    for entry in entries {
        let id = container_id(&entry.key)?;
        let value = entry.value.as_properties().ok_or_else(|| {
            SaveError::ContainerData("CharacterContainerSaveData value is not a struct".to_string())
        })?;

        let mut slots = Vec::new();
        for slot_struct in struct_array(value.get("Slots")) {
            let slot_index = slot_struct.get("SlotIndex").and_then(as_i32).unwrap_or(0);
            let raw = slot_struct
                .get("RawData")
                .and_then(Property::as_bytes)
                .ok_or_else(|| {
                    SaveError::ContainerData("character container slot missing RawData".to_string())
                })?;
            if let Some(instance_id) = decode_character_slot_bytes(raw)? {
                slots.push(SlotRef {
                    slot_index,
                    pal_id: instance_id.to_string(),
                });
            }
        }
        out.insert(id, slots);
    }
    Ok(out)
}

/// Read the seven container ids a player references from its per-player
/// `<UID>.sav`. Decompresses + parses the GVAS envelope, then navigates
/// `SaveData` exactly as `player.py` does.
pub fn read_player_container_ids(sav_path: &Path) -> Result<PlayerContainerIds, SaveError> {
    let bytes = std::fs::read(sav_path)
        .map_err(|e| SaveError::Io(format!("{}: {e}", sav_path.display())))?;
    let raw = decompress_sav(&bytes)?;
    let gvas = parse_gvas(&raw, &default_skip_set())?;

    let save_data = gvas
        .root
        .get("SaveData")
        .ok_or_else(|| SaveError::ContainerData("player .sav missing SaveData".to_string()))?;

    // `<Field>.ID` is a Guid struct nested in the field's struct value.
    let id_of = |field: &str| -> String {
        save_data
            .get_child(field)
            .and_then(|f| f.get_child("ID"))
            .and_then(struct_guid)
            .map(|u| u.to_string())
            .unwrap_or_default()
    };
    let inv_id = |field: &str| -> String {
        save_data
            .get_child("InventoryInfo")
            .and_then(|inv| inv.get_child(field))
            .and_then(|f| f.get_child("ID"))
            .and_then(struct_guid)
            .map(|u| u.to_string())
            .unwrap_or_default()
    };

    Ok(PlayerContainerIds {
        pal_storage: id_of("PalStorageContainerId"),
        otomo: id_of("OtomoCharacterContainerId"),
        common: inv_id("CommonContainerId"),
        essential: inv_id("EssentialContainerId"),
        weapon_loadout: inv_id("WeaponLoadOutContainerId"),
        player_equip_armor: inv_id("PlayerEquipArmorContainerId"),
        food_equip: inv_id("FoodEquipContainerId"),
    })
}

// --- per-slot RawData codecs ----------------------------------------------

/// Decode an item-container slot `RawData` blob (`item_container_slots.py`).
/// Returns `None` for an empty blob (an unused slot the save still serialized).
fn decode_item_slot_bytes(
    bytes: &[u8],
    dynamic_items: &HashMap<Uuid, DynamicItem>,
) -> Result<Option<ItemContainerSlot>, SaveError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    // Fixed prefix: 4 (slot_index) + 4 (count) + fstring + 16 + 16.
    if bytes.len() < 8 {
        return Err(SaveError::ContainerData(format!(
            "item slot RawData too short: {} bytes",
            bytes.len()
        )));
    }
    let mut r = Reader::new(bytes);
    let slot_index = r.read_i32();
    let count = r.read_i32();
    let static_id = r.fstring();
    if r.remaining() < 32 {
        return Err(SaveError::ContainerData(
            "item slot RawData missing dynamic_id guids".to_string(),
        ));
    }
    let _created_world_id = r.guid();
    let local_id = r.guid();
    // trailing bytes are not needed.

    let dynamic_item = if is_empty_uuid(local_id) {
        None
    } else {
        dynamic_items.get(&local_id).cloned()
    };

    Ok(Some(ItemContainerSlot {
        slot_index,
        count,
        static_id,
        dynamic_item,
    }))
}

/// Decode a character-container slot `RawData` blob (`character_container.py`).
/// Returns the occupying pal `instance_id`, or `None` when the slot is empty
/// (all-zero instance id).
fn decode_character_slot_bytes(bytes: &[u8]) -> Result<Option<Uuid>, SaveError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    // player_uid (16) + instance_id (16) + permission_tribe_id (1).
    if bytes.len() < 33 {
        return Err(SaveError::ContainerData(format!(
            "character slot RawData too short: {} bytes",
            bytes.len()
        )));
    }
    let mut r = Reader::new(bytes);
    let _player_uid = r.guid();
    let instance_id = r.guid();
    let _permission_tribe_id = r.read_u8();
    if is_empty_uuid(instance_id) {
        Ok(None)
    } else {
        Ok(Some(instance_id))
    }
}

/// Decode a `DynamicItemSaveData` entry `RawData` blob (`dynamic_item.py`),
/// returning the `(local_id, DynamicItem)` pair.
///
/// After the fixed id prefix (`created_world_id`, `local_id`, `static_id`) the
/// body is one of armor / weapon / egg. The reference distinguishes these with
/// a try/seek probe; we reproduce the same *outcomes* with explicit bounds
/// checks so a malformed blob degrades to `item_type = ""` instead of panicking:
///
/// - remaining `== 12` → **armor**: `[4 leading][f32 durability][4 trailing]`.
/// - an exact-fit `[4 leading][f32 durability][i32 bullets][tarray<fstring>
///   passives]([fstring])?[4 trailing]` → **weapon**.
/// - otherwise, if a `character_id` fstring reads after 4 leading bytes → **egg**
///   (the egg body carries a full pal `SaveParameter` the thin DTO does not
///   model, so only the type is recorded).
/// - anything else → `item_type = ""` (unknown).
fn decode_dynamic_item_bytes(bytes: &[u8]) -> Result<Option<(Uuid, DynamicItem)>, SaveError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.len() < 32 {
        return Err(SaveError::ContainerData(format!(
            "dynamic item RawData too short: {} bytes",
            bytes.len()
        )));
    }
    let mut r = Reader::new(bytes);
    let _created_world_id = r.guid();
    let local_id = r.guid();
    if r.remaining() < 4 {
        return Err(SaveError::ContainerData(
            "dynamic item RawData missing static_id".to_string(),
        ));
    }
    let _static_id = r.fstring();

    let body = &bytes[r.pos()..];
    let mut item = DynamicItem {
        local_id: local_id.to_string(),
        ..DynamicItem::default()
    };

    if body.len() == 12 {
        // armor: [4 leading][f32 durability][4 trailing]
        let mut b = Reader::new(body);
        b.skip(4);
        item.item_type = "armor".to_string();
        item.durability = b.read_f32() as f64;
    } else if let Some((durability, bullets, passives)) = parse_weapon_body(body) {
        item.item_type = "weapon".to_string();
        item.durability = durability;
        item.remaining_bullets = bullets;
        item.passive_skill_list = passives;
    } else if is_egg_body(body) {
        item.item_type = "egg".to_string();
    }
    // else: leave item_type = "" (unknown / raw), matching the reference's
    // raw-trailer fallback for a body it cannot classify.

    Ok(Some((local_id, item)))
}

/// Try to parse a weapon body as an exact-fit
/// `[4 leading][f32 durability][i32 bullets][tarray<fstring> passives]
/// ([fstring] when >4 bytes remain)[4 trailing]`, returning `None` if any
/// bounded read would overrun or the body is not fully consumed.
fn parse_weapon_body(body: &[u8]) -> Option<(f64, i32, Vec<String>)> {
    let mut r = Reader::new(body);
    // 4 leading + 4 durability + 4 bullets + 4 passive-count.
    if r.remaining() < 16 {
        return None;
    }
    r.skip(4);
    let durability = r.read_f32() as f64;
    let bullets = r.read_i32();
    let count = r.read_u32() as usize;
    // Each passive is at minimum a 4-byte length prefix.
    if count > r.remaining() / 4 {
        return None;
    }
    let mut passives = Vec::with_capacity(count);
    for _ in 0..count {
        passives.push(guarded_fstring(&mut r, body)?);
    }
    // Optional trailing string when more than the 4 trailing bytes remain.
    if body.len().saturating_sub(r.pos()) > 4 {
        guarded_fstring(&mut r, body)?;
    }
    // Exactly the 4 trailing bytes must remain.
    if body.len().saturating_sub(r.pos()) != 4 {
        return None;
    }
    Some((durability, bullets, passives))
}

/// True when `body` begins (after 4 leading bytes) with a readable, non-empty
/// `character_id` fstring — the egg signature.
fn is_egg_body(body: &[u8]) -> bool {
    if body.len() < 4 {
        return false;
    }
    let mut r = Reader::new(body);
    r.skip(4);
    matches!(guarded_fstring(&mut r, body), Some(s) if !s.is_empty())
}

/// Read an fstring only if its declared length fits within `buf`; otherwise
/// return `None` (instead of the underlying reader panicking on underrun).
fn guarded_fstring(r: &mut Reader, buf: &[u8]) -> Option<String> {
    let pos = r.pos();
    if buf.len().saturating_sub(pos) < 4 {
        return None;
    }
    let len = i32::from_le_bytes(buf[pos..pos + 4].try_into().ok()?);
    let body_bytes: usize = if len >= 0 {
        len as usize
    } else {
        (len as i64).unsigned_abs() as usize * 2
    };
    if buf.len().saturating_sub(pos) < 4 + body_bytes {
        return None;
    }
    Some(r.fstring())
}

// --- generic-tree helpers -------------------------------------------------

/// Borrow a map property's entries, or fail with a labelled error.
fn as_map_entries<'a>(
    map: &'a Property,
    label: &str,
) -> Result<&'a [super::props::MapEntry], SaveError> {
    match map {
        Property::Map { entries, .. } => Ok(entries),
        _ => Err(SaveError::ContainerData(format!(
            "{label} is not a MapProperty"
        ))),
    }
}

/// The container `ID` (a `Guid`) from a `{ ID: Guid }` map-key struct.
fn container_id(key: &Property) -> Result<Uuid, SaveError> {
    key.get_child("ID")
        .and_then(struct_guid)
        .ok_or_else(|| SaveError::ContainerData("container key missing ID guid".to_string()))
}

/// The per-element property maps of a `StructProperty` array, or an empty slice
/// view when the property is absent / not such an array.
fn struct_array(
    prop: Option<&Property>,
) -> impl Iterator<Item = &std::collections::BTreeMap<String, Property>> {
    let values: &[StructValue] = match prop {
        Some(Property::Array {
            value: ArrayValue::Structs { values, .. },
            ..
        }) => values,
        _ => &[],
    };
    values.iter().filter_map(|v| match v {
        StructValue::Properties(m) => Some(m),
        _ => None,
    })
}

/// The `Uuid` inside a `Guid` struct property.
fn struct_guid(p: &Property) -> Option<Uuid> {
    match p {
        Property::Struct {
            value: StructValue::Guid(u),
            ..
        } => Some(*u),
        _ => None,
    }
}

/// An `i32` from an integer property.
fn as_i32(p: &Property) -> Option<i32> {
    match p {
        Property::Int(v) => Some(*v as i32),
        _ => None,
    }
}

/// True for the all-zero (nil) UUID that Palworld uses to mean "no reference".
fn is_empty_uuid(u: Uuid) -> bool {
    u.is_nil()
}
