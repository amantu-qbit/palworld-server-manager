//! High-level save edits, each expressed as a splice plan over the
//! decompressed GVAS buffer and validated by a strict re-parse before any
//! bytes leave this module.
//!
//! Every operation follows the same recipe:
//!
//! 1. **Locate** the target with [`super::locate`] — a targeted descent that
//!    skips siblings by declared size and records the offsets of every
//!    enclosing size field on the path.
//! 2. **Plan** patches with [`super::plan`] — content splices plus size/count
//!    scopes; deltas are computed by the plan, never by hand.
//! 3. **Apply**, then **validate**: the edited buffer is re-parsed with the
//!    production reader ([`gvas::parse_gvas`]) and the relevant domain decoder,
//!    and the expected change is asserted. Only then are bytes returned.
//!
//! Semantics ported from `palworld-save-pal` (PR #299 & `game/item_container.py`,
//! `game/pal.py`): growing a container only bumps `SlotNum` (empty slots are
//! absent entries); shrinking removes slot entries with `slot_index >= n`;
//! clearing a slot removes its entry and any orphaned `DynamicItemSaveData`
//! record; character fields live inside the `CharacterSaveParameterMap` entry's
//! `RawData` blob under `SaveParameter`.

use std::collections::BTreeMap;
use std::ops::Range;

use uuid::Uuid;

use super::super::containers::{decode_dynamic_items, decode_item_containers};
use super::super::character::decode_characters;
use super::super::decompress::SaveError;
use super::super::gvas::{self, default_skip_set, parse_gvas};
use super::super::props::Property;
use super::enc;
use super::locate::{array_info, find_in_stream, map_info, read_tag, ArrayInfo, Cursor, PropTag};
use super::plan::{apply, EditPlan};

fn edit_err(msg: impl Into<String>) -> SaveError {
    SaveError::Edit(msg.into())
}

fn is_nil(u: Uuid) -> bool {
    u == Uuid::nil()
}

// ---------------------------------------------------------------------------
// Shared descent helpers
// ---------------------------------------------------------------------------

/// `worldSaveData` located: the struct's tag plus a cursor positioned at the
/// start of its body stream.
fn world_save_data(buf: &[u8]) -> Result<PropTag, SaveError> {
    let start = gvas::header_len(buf)?;
    let mut c = Cursor::new(buf, start);
    let scan = find_in_stream(&mut c, "worldSaveData")?;
    scan.found
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("Level.sav missing worldSaveData"))
}

/// Find a named property inside a body region starting at `body_start`.
fn find_in_body(
    buf: &[u8],
    body_start: usize,
    name: &str,
) -> Result<Option<PropTag>, SaveError> {
    let mut c = Cursor::new(buf, body_start);
    Ok(find_in_stream(&mut c, name)?.found)
}

/// Read a property stream of `Guid`-struct members (a map key such as
/// `{ ID }` or `{ PlayerUId, InstanceId }`), leaving the cursor past the
/// stream's terminator.
fn read_guid_key_stream(c: &mut Cursor) -> Result<BTreeMap<String, Uuid>, SaveError> {
    let mut out = BTreeMap::new();
    loop {
        match read_tag(c)? {
            None => return Ok(out),
            Some(tag) => {
                if tag.struct_type.as_deref() == Some("Guid") && tag.size == 16 {
                    // Cursor sits at value_start after read_tag.
                    out.insert(tag.name.clone(), c.guid()?);
                }
                c.seek(tag.value_end)?;
            }
        }
    }
}

/// Skip a whole property stream from the cursor, returning the offset just
/// past its `"None"` terminator.
fn skip_value_stream(c: &mut Cursor) -> Result<usize, SaveError> {
    super::locate::skip_stream(c)?;
    Ok(c.pos())
}

// ---------------------------------------------------------------------------
// Container location
// ---------------------------------------------------------------------------

/// One `Slots` array element (a property stream), with its `RawData` blob
/// parsed. `slot_index` is `None` for an empty (zero-length) blob.
#[derive(Debug)]
struct SlotElem {
    /// The whole element (property stream incl. its `"None"` terminator).
    range: Range<usize>,
    /// `RawData` property size field (`u64`).
    raw_size_field: usize,
    /// `RawData` byte-count word (`u32`, equals blob length).
    raw_count_offset: usize,
    /// The blob bytes (after the count word).
    blob: Range<usize>,
    slot_index: Option<i32>,
    /// On-disk bytes of `created_world_id` (16, verbatim).
    created_world: Range<usize>,
    local_id: Uuid,
    /// Everything after `local_id` in the blob (trailing bytes, verbatim).
    trailing: Range<usize>,
}

/// A located item container: every offset an edit needs, plus the scopes of
/// the enclosing size fields.
struct ContainerLoc {
    /// (`u64` size-field offset, body) pairs to register on any plan touching
    /// this container: `worldSaveData`, the `ItemContainerSaveData` map, and
    /// (when slots are touched) the `Slots` array outer + inner sizes.
    wsd_scope: (usize, Range<usize>),
    map_scope: (usize, Range<usize>),
    slot_num: i32,
    /// `SlotNum` `IntProperty` value offset (4 bytes), if present.
    slot_num_value: Option<usize>,
    /// Offset of the `Slots` property's name fstring — where a missing
    /// `SlotNum` property is inserted. Inserting *before* the Slots tag
    /// (rather than at the stream terminator) keeps the insert strictly
    /// outside the Slots size scopes: a boundary insert at a scope's end
    /// would otherwise be attributed inside it by the plan's inclusive-end
    /// containment test and inflate the array's two size fields.
    slots_tag_start: usize,
    slots_scope: (usize, Range<usize>),
    slots_inner_scope: (usize, Range<usize>),
    slots_count_offset: usize,
    slots_elems_end: usize,
    elements: Vec<SlotElem>,
}

/// Walk one `Slots` element (cursor at its start): find `RawData`, parse the
/// blob prefix, and skip to the element's end.
fn read_slot_elem(buf: &[u8], c: &mut Cursor) -> Result<SlotElem, SaveError> {
    let start = c.pos();
    let scan = find_in_stream(c, "RawData")?;
    let raw = scan
        .found
        .filter(|t| t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("ByteProperty"))
        .ok_or_else(|| edit_err("Slots element missing RawData byte array"))?;

    let raw_count_offset = raw.value_start;
    let blob = raw.value_start + 4..raw.value_end;

    let (slot_index, created_world, local_id, trailing) = if blob.is_empty() {
        (None, blob.start..blob.start, Uuid::nil(), blob.start..blob.start)
    } else {
        let mut b = Cursor::new(buf, blob.start);
        let idx = b.read_i32()?;
        let _count = b.read_i32()?;
        let _static_id = b.fstring()?;
        let cw_start = b.pos();
        let _cw = b.guid()?;
        let cw = cw_start..b.pos();
        let local_id = b.guid()?;
        let trailing = b.pos()..blob.end;
        (Some(idx), cw, local_id, trailing)
    };

    // Skip the rest of the element's property stream (CustomVersionData, …),
    // then advance the caller's cursor to the element end so the next element
    // parses from the right offset.
    let mut e = Cursor::new(buf, raw.value_end);
    let end = skip_value_stream(&mut e)?;
    c.seek(end)?;

    Ok(SlotElem {
        range: start..end,
        raw_size_field: raw.size_field,
        raw_count_offset,
        blob,
        slot_index,
        created_world,
        local_id,
        trailing,
    })
}

/// Locate `container_id` inside `ItemContainerSaveData`, parsing its `SlotNum`
/// and every `Slots` element.
fn locate_container(buf: &[u8], container_id: Uuid) -> Result<ContainerLoc, SaveError> {
    let wsd = world_save_data(buf)?;
    let map_tag = find_in_body(buf, wsd.value_start, "ItemContainerSaveData")?
        .filter(|t| t.type_name == "MapProperty")
        .ok_or_else(|| edit_err("worldSaveData missing ItemContainerSaveData"))?;

    let mut c = Cursor::new(buf, map_tag.value_start);
    let info = map_info(&mut c)?;

    for _ in 0..info.count {
        // Key: property stream `{ ID: Guid }`.
        let key = read_guid_key_stream(&mut c)?;
        let id = key
            .get("ID")
            .copied()
            .ok_or_else(|| edit_err("ItemContainerSaveData key missing ID"))?;

        if id != container_id {
            skip_value_stream(&mut c)?;
            continue;
        }

        // Value: property stream `{ SlotNum, Slots, ... }`.
        let value_start = c.pos();
        let slot_num_tag = find_in_body(buf, value_start, "SlotNum")?;
        let (slot_num, slot_num_value) = match &slot_num_tag {
            Some(t) if t.type_name == "IntProperty" => {
                let mut v = Cursor::new(buf, t.value_start);
                (v.read_i32()?, Some(t.value_start))
            }
            _ => (0, None),
        };

        let slots_tag = find_in_body(buf, value_start, "Slots")?
            .filter(|t| t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("StructProperty"))
            .ok_or_else(|| edit_err("container missing Slots array"))?;

        let mut a = Cursor::new(buf, slots_tag.value_start);
        let arr: ArrayInfo = array_info(&mut a, &slots_tag)?;
        let inner = arr
            .inner
            .clone()
            .ok_or_else(|| edit_err("Slots array missing inner struct header"))?;

        let mut elements = Vec::with_capacity(arr.count as usize);
        let mut e = Cursor::new(buf, arr.elems_start);
        for _ in 0..arr.count {
            elements.push(read_slot_elem(buf, &mut e)?);
        }
        e.expect_at(slots_tag.value_end, "Slots elements")?;

        return Ok(ContainerLoc {
            wsd_scope: (wsd.size_field, wsd.value_start..wsd.value_end),
            map_scope: (map_tag.size_field, map_tag.value_start..map_tag.value_end),
            slot_num,
            slot_num_value,
            slots_tag_start: slots_tag.tag_start,
            slots_scope: (slots_tag.size_field, slots_tag.value_start..slots_tag.value_end),
            slots_inner_scope: (inner.size_field, arr.elems_start..slots_tag.value_end),
            slots_count_offset: arr.count_offset,
            slots_elems_end: slots_tag.value_end,
            elements,
        });
    }

    Err(edit_err(format!("container {container_id} not found")))
}

/// Register a container's enclosing size scopes on a plan.
fn container_scopes(plan: &mut EditPlan, loc: &ContainerLoc, touch_slots: bool) {
    plan.scope_u64(loc.wsd_scope.0, loc.wsd_scope.1.clone());
    plan.scope_u64(loc.map_scope.0, loc.map_scope.1.clone());
    if touch_slots {
        plan.scope_u64(loc.slots_scope.0, loc.slots_scope.1.clone());
        plan.scope_u64(loc.slots_inner_scope.0, loc.slots_inner_scope.1.clone());
    }
}

// ---------------------------------------------------------------------------
// Dynamic-item removal (shared by clear/overwrite/shrink paths)
// ---------------------------------------------------------------------------

/// Delete `DynamicItemSaveData` entries whose `local_id` is in `targets`,
/// adding the deletions (and the array's scopes/count fix) to `plan`.
fn remove_dynamic_items(
    buf: &[u8],
    plan: &mut EditPlan,
    targets: &[Uuid],
) -> Result<usize, SaveError> {
    if targets.is_empty() {
        return Ok(0);
    }
    let wsd = world_save_data(buf)?;
    let Some(arr_tag) = find_in_body(buf, wsd.value_start, "DynamicItemSaveData")? else {
        return Ok(0);
    };
    if arr_tag.type_name != "ArrayProperty" || arr_tag.elem_type.as_deref() != Some("StructProperty")
    {
        return Ok(0);
    }
    let mut a = Cursor::new(buf, arr_tag.value_start);
    let arr = array_info(&mut a, &arr_tag)?;
    let inner = arr
        .inner
        .clone()
        .ok_or_else(|| edit_err("DynamicItemSaveData missing inner header"))?;

    let mut removed = 0usize;
    let mut c = Cursor::new(buf, arr.elems_start);
    for _ in 0..arr.count {
        let start = c.pos();
        // Element: property stream with a RawData blob whose prefix is
        // `created_world_id (16) | local_id (16) | ...`.
        let scan = find_in_stream(&mut c, "RawData")?;
        let raw = scan
            .found
            .filter(|t| t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("ByteProperty"))
            .ok_or_else(|| edit_err("DynamicItemSaveData element missing RawData"))?;
        let blob_start = raw.value_start + 4;
        let local_id = if raw.value_end.saturating_sub(blob_start) >= 32 {
            let mut b = Cursor::new(buf, blob_start);
            let _cw = b.guid()?;
            b.guid()?
        } else {
            Uuid::nil()
        };
        let mut e = Cursor::new(buf, raw.value_end);
        let end = skip_value_stream(&mut e)?;
        c.seek(end)?;

        if targets.contains(&local_id) {
            plan.delete(start..end);
            removed += 1;
        }
    }
    c.expect_at(arr_tag.value_end, "DynamicItemSaveData elements")?;

    if removed > 0 {
        plan.scope_u64(arr_tag.size_field, arr_tag.value_start..arr_tag.value_end);
        plan.scope_u64(inner.size_field, arr.elems_start..arr_tag.value_end);
        plan.count(arr.count_offset, -(removed as i64));
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Public ops: containers
// ---------------------------------------------------------------------------

/// Change a container's slot count (PR #299 semantics): growing bumps
/// `SlotNum` only; shrinking also removes slot entries with
/// `slot_index >= new_slot_num` and their orphaned dynamic items.
pub fn resize_container(
    buf: &[u8],
    container_id: Uuid,
    new_slot_num: u32,
) -> Result<Vec<u8>, SaveError> {
    if new_slot_num > 9999 {
        return Err(edit_err("slot_num out of range (0..=9999)"));
    }
    let loc = locate_container(buf, container_id)?;
    let mut plan = EditPlan::default();

    match loc.slot_num_value {
        Some(off) => plan.patch(off..off + 4, (new_slot_num as i32).to_le_bytes().to_vec()),
        // Missing SlotNum: insert it just before the Slots property — never at
        // the stream terminator, which can coincide with the Slots scopes' end
        // boundary and be miscounted into the array's size fields (see
        // `ContainerLoc::slots_tag_start`).
        None => plan.insert(
            loc.slots_tag_start,
            enc::int_prop("SlotNum", new_slot_num as i32),
        ),
    }

    let mut orphans = Vec::new();
    let mut removed = 0i64;
    for e in &loc.elements {
        if let Some(idx) = e.slot_index {
            if idx >= new_slot_num as i32 {
                plan.delete(e.range.clone());
                removed += 1;
                if !is_nil(e.local_id) {
                    orphans.push(e.local_id);
                }
            }
        }
    }
    if removed > 0 {
        plan.count(loc.slots_count_offset, -removed);
    }
    container_scopes(&mut plan, &loc, removed > 0);
    remove_dynamic_items(buf, &mut plan, &orphans)?;

    let out = apply(buf, &plan)?;
    validate_container(&out, container_id, |c| {
        if c.slot_num != new_slot_num as i32 {
            return Err(edit_err(format!(
                "post-edit slot_num is {}, expected {new_slot_num}",
                c.slot_num
            )));
        }
        if let Some(bad) = c.slots.iter().find(|s| s.slot_index >= new_slot_num as i32) {
            return Err(edit_err(format!(
                "post-edit slot {} survived shrink to {new_slot_num}",
                bad.slot_index
            )));
        }
        Ok(())
    })?;
    Ok(out)
}

/// Set or clear one container slot. `static_id == "None"` (or `count <= 0`)
/// clears the slot, removing its entry and any orphaned dynamic item. Setting
/// writes a static item (no dynamic payload); an existing dynamic item in the
/// slot is removed like the reference's `_clean_up_inventory`.
pub fn set_container_slot(
    buf: &[u8],
    container_id: Uuid,
    slot_index: i32,
    static_id: &str,
    count: i32,
) -> Result<Vec<u8>, SaveError> {
    let loc = locate_container(buf, container_id)?;
    let clearing = static_id == "None" || static_id.is_empty() || count <= 0;
    if !clearing && (slot_index < 0 || slot_index >= loc.slot_num) {
        return Err(edit_err(format!(
            "slot_index {slot_index} out of range (container has {} slots)",
            loc.slot_num
        )));
    }

    let existing = loc.elements.iter().find(|e| e.slot_index == Some(slot_index));
    let mut plan = EditPlan::default();
    let mut orphans = Vec::new();

    match (existing, clearing) {
        (None, true) => {
            // Clearing an already-empty slot: nothing to do.
            return Ok(buf.to_vec());
        }
        (Some(e), true) => {
            plan.delete(e.range.clone());
            plan.count(loc.slots_count_offset, -1);
            if !is_nil(e.local_id) {
                orphans.push(e.local_id);
            }
        }
        (Some(e), false) => {
            // Rebuild the blob prefix in place; keep created_world_id and
            // trailing bytes verbatim, null the dynamic id (static item).
            let mut blob = Vec::new();
            blob.extend_from_slice(&slot_index.to_le_bytes());
            blob.extend_from_slice(&count.to_le_bytes());
            blob.extend(enc::fstring(static_id));
            blob.extend_from_slice(&buf[e.created_world.clone()]);
            blob.extend(enc::nil_guid());
            blob.extend_from_slice(&buf[e.trailing.clone()]);
            plan.patch(e.blob.clone(), blob);
            plan.scope_u64(e.raw_size_field, e.raw_count_offset..e.blob.end);
            plan.scope_u32(e.raw_count_offset, e.blob.clone());
            if !is_nil(e.local_id) {
                orphans.push(e.local_id);
            }
        }
        (None, false) => {
            // New slot entry: rebuild from a sibling template so the element
            // carries whatever extra properties (CustomVersionData, …) this
            // save's slots have.
            let template = loc
                .elements
                .iter()
                .find(|e| !e.blob.is_empty())
                .ok_or_else(|| {
                    edit_err("container has no template slot to copy; add an item in-game first")
                })?;
            let mut blob = Vec::new();
            blob.extend_from_slice(&slot_index.to_le_bytes());
            blob.extend_from_slice(&count.to_le_bytes());
            blob.extend(enc::fstring(static_id));
            blob.extend(enc::nil_guid());
            blob.extend(enc::nil_guid());
            blob.extend_from_slice(&buf[template.trailing.clone()]);

            let elem = rebuild_element(buf, template, &blob);
            plan.insert(loc.slots_elems_end, elem);
            plan.count(loc.slots_count_offset, 1);
        }
    }

    container_scopes(&mut plan, &loc, true);
    remove_dynamic_items(buf, &mut plan, &orphans)?;

    let out = apply(buf, &plan)?;
    let expect_static = static_id.to_string();
    validate_container(&out, container_id, move |c| {
        let slot = c.slots.iter().find(|s| s.slot_index == slot_index);
        if clearing {
            if slot.is_some() {
                return Err(edit_err("post-edit slot still present after clear"));
            }
        } else {
            let s = slot.ok_or_else(|| edit_err("post-edit slot missing"))?;
            if s.static_id != expect_static || s.count != count {
                return Err(edit_err(format!(
                    "post-edit slot mismatch: {} x{}",
                    s.static_id, s.count
                )));
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// Remove every occupied slot entry from a container in one write — a bulk
/// clear takes one backup instead of one per stack — and remove the orphaned
/// dynamic items. Clearing an already-empty container returns the buffer
/// unchanged.
pub fn clear_container(buf: &[u8], container_id: Uuid) -> Result<Vec<u8>, SaveError> {
    let loc = locate_container(buf, container_id)?;
    let mut plan = EditPlan::default();
    let mut orphans = Vec::new();
    let mut removed = 0i64;
    for e in &loc.elements {
        if e.slot_index.is_some() {
            plan.delete(e.range.clone());
            removed += 1;
            if !is_nil(e.local_id) {
                orphans.push(e.local_id);
            }
        }
    }
    if removed == 0 {
        return Ok(buf.to_vec());
    }
    plan.count(loc.slots_count_offset, -removed);
    container_scopes(&mut plan, &loc, true);
    remove_dynamic_items(buf, &mut plan, &orphans)?;

    let out = apply(buf, &plan)?;
    validate_container(&out, container_id, |c| {
        if !c.slots.is_empty() {
            return Err(edit_err("post-edit container still has occupied slots"));
        }
        Ok(())
    })?;
    Ok(out)
}

/// Rebuild a slot element from `template`, replacing its `RawData` blob with
/// `new_blob` and copying every other property verbatim.
fn rebuild_element(buf: &[u8], template: &SlotElem, new_blob: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // Bytes before the RawData property: locate RawData's tag start by
    // scanning from the element start.
    let mut c = Cursor::new(buf, template.range.start);
    // find_in_stream over a template we already parsed cannot fail here.
    let scan = find_in_stream(&mut c, "RawData").expect("template rescan");
    let raw = scan.found.expect("template has RawData");

    out.extend_from_slice(&buf[template.range.start..raw.tag_start]);
    // RawData property with the new blob.
    let mut body = Vec::new();
    body.extend_from_slice(&(new_blob.len() as u32).to_le_bytes());
    body.extend_from_slice(new_blob);
    out.extend(enc::fstring("RawData"));
    out.extend(enc::fstring("ArrayProperty"));
    out.extend_from_slice(&(body.len() as u64).to_le_bytes());
    out.extend(enc::fstring("ByteProperty"));
    out.push(0); // optional_guid absent
    out.extend(body);
    // Everything after the RawData property (CustomVersionData, …, "None").
    out.extend_from_slice(&buf[raw.value_end..template.range.end]);
    out
}

/// Re-parse an edited Level.sav buffer, decode its containers, and run
/// `check` against `container_id`'s decoded state.
fn validate_container<F>(new_buf: &[u8], container_id: Uuid, check: F) -> Result<(), SaveError>
where
    F: FnOnce(&super::super::model::ItemContainer) -> Result<(), SaveError>,
{
    let gvas = parse_gvas(new_buf, &default_skip_set())?;
    let wsd = gvas
        .root
        .get("worldSaveData")
        .ok_or_else(|| edit_err("post-edit parse lost worldSaveData"))?;
    let dynamic = match wsd.get_child("DynamicItemSaveData") {
        Some(p) => decode_dynamic_items(p)?,
        None => Default::default(),
    };
    let containers = match wsd.get_child("ItemContainerSaveData") {
        Some(p) => decode_item_containers(p, &dynamic)?,
        None => Default::default(),
    };
    let c = containers
        .get(&container_id)
        .ok_or_else(|| edit_err("post-edit parse lost the container"))?;
    check(c)
}

// ---------------------------------------------------------------------------
// Public ops: characters (players + pals in Level.sav)
// ---------------------------------------------------------------------------

/// Which `CharacterSaveParameterMap` entry to edit.
#[derive(Debug, Clone, Copy)]
pub enum CharTarget {
    /// Match on the key's `PlayerUId`.
    Player(Uuid),
    /// Match on the key's `InstanceId` (pals; also works for players).
    Instance(Uuid),
}

/// Field edits applied inside a character's `SaveParameter`. `None` = leave
/// unchanged. List/map fields replace the stored value wholesale.
#[derive(Debug, Clone, Default)]
pub struct CharacterEdits {
    pub level: Option<u8>,
    pub exp: Option<i64>,
    pub nickname: Option<String>,
    /// `PassiveSkillList` (NameProperty elements).
    pub passive_skills: Option<Vec<String>>,
    /// `EquipWaza` (EnumProperty elements).
    pub active_skills: Option<Vec<String>>,
    /// `MasteredWaza` (EnumProperty elements).
    pub learned_skills: Option<Vec<String>>,
    pub rank: Option<u8>,
    pub rank_hp: Option<u8>,
    pub rank_attack: Option<u8>,
    pub rank_defense: Option<u8>,
    pub rank_craftspeed: Option<u8>,
    pub talent_hp: Option<u8>,
    pub talent_shot: Option<u8>,
    pub talent_defense: Option<u8>,
    /// `GotStatusPointList` updates, keyed by the exact on-disk status name.
    pub status_points: Option<BTreeMap<String, i32>>,
    /// `GotExStatusPointList` updates, keyed by the exact on-disk status name.
    pub ext_status_points: Option<BTreeMap<String, i32>>,
}

/// Apply `edits` to the matching character. Returns the edited buffer.
pub fn edit_character(
    buf: &[u8],
    target: CharTarget,
    edits: &CharacterEdits,
) -> Result<Vec<u8>, SaveError> {
    let wsd = world_save_data(buf)?;
    let map_tag = find_in_body(buf, wsd.value_start, "CharacterSaveParameterMap")?
        .filter(|t| t.type_name == "MapProperty")
        .ok_or_else(|| edit_err("worldSaveData missing CharacterSaveParameterMap"))?;

    let mut c = Cursor::new(buf, map_tag.value_start);
    let info = map_info(&mut c)?;

    for _ in 0..info.count {
        let key = read_guid_key_stream(&mut c)?;
        let player_uid = key.get("PlayerUId").copied().unwrap_or_default();
        let instance_id = key.get("InstanceId").copied().unwrap_or_default();

        let matched = match target {
            CharTarget::Player(uid) => player_uid == uid && !is_nil(uid),
            CharTarget::Instance(iid) => instance_id == iid && !is_nil(iid),
        };
        if !matched {
            skip_value_stream(&mut c)?;
            continue;
        }

        // Value stream: `{ RawData: Array<Byte> }`.
        let value_start = c.pos();
        let raw = find_in_body(buf, value_start, "RawData")?
            .filter(|t| {
                t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("ByteProperty")
            })
            .ok_or_else(|| edit_err("character entry missing RawData"))?;
        let blob = raw.value_start + 4..raw.value_end;

        let mut plan = EditPlan::default();
        plan.scope_u64(wsd.size_field, wsd.value_start..wsd.value_end);
        plan.scope_u64(map_tag.size_field, map_tag.value_start..map_tag.value_end);
        plan.scope_u64(raw.size_field, raw.value_start..raw.value_end);
        plan.scope_u32(raw.value_start, blob.clone());

        plan_save_parameter_edits(buf, blob.clone(), edits, &mut plan)?;

        let out = apply(buf, &plan)?;
        validate_character(&out, target, edits)?;
        return Ok(out);
    }

    Err(edit_err(match target {
        CharTarget::Player(u) => format!("player {u} not found in CharacterSaveParameterMap"),
        CharTarget::Instance(u) => format!("character instance {u} not found"),
    }))
}

/// Plan every requested edit inside a character `RawData` blob. The blob is an
/// inner property stream (whose `SaveParameter` struct carries the fields)
/// followed by a 24-byte trailer that is left untouched.
fn plan_save_parameter_edits(
    buf: &[u8],
    blob: Range<usize>,
    edits: &CharacterEdits,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    let mut c = Cursor::new(buf, blob.start);
    let sp = find_in_stream(&mut c, "SaveParameter")?
        .found
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("character RawData missing SaveParameter"))?;
    plan.scope_u64(sp.size_field, sp.value_start..sp.value_end);

    let body = sp.value_start;

    // Byte-valued fields (`"None"`-typed ByteProperty, 1-byte payload).
    let byte_fields: [(&str, Option<u8>); 8] = [
        ("Level", edits.level),
        ("Rank", edits.rank),
        ("Rank_HP", edits.rank_hp),
        ("Rank_Attack", edits.rank_attack),
        ("Rank_Defence", edits.rank_defense), // British spelling on disk
        ("Rank_CraftSpeed", edits.rank_craftspeed),
        ("Talent_HP", edits.talent_hp),
        ("Talent_Shot", edits.talent_shot),
    ];
    for (name, val) in byte_fields {
        if let Some(v) = val {
            plan_byte_field(buf, body, name, v, plan)?;
        }
    }
    if let Some(v) = edits.talent_defense {
        plan_byte_field(buf, body, "Talent_Defense", v, plan)?;
    }

    if let Some(exp) = edits.exp {
        match find_in_body(buf, body, "Exp")? {
            Some(t) if t.type_name == "Int64Property" => {
                plan.patch(t.value_start..t.value_start + 8, exp.to_le_bytes().to_vec());
            }
            Some(t) => {
                // Legacy IntProperty: replace the whole property with Int64.
                plan.patch(t.tag_start..t.value_end, enc::int64_prop("Exp", exp));
            }
            None => insert_at_terminator(buf, body, enc::int64_prop("Exp", exp), plan)?,
        }
    }

    if let Some(nick) = &edits.nickname {
        match find_in_body(buf, body, "NickName")? {
            Some(t) if t.type_name == "StrProperty" => {
                plan.patch(t.value_start..t.value_end, enc::fstring(nick));
                plan.scope_u64(t.size_field, t.value_start..t.value_end);
            }
            Some(t) => {
                plan.patch(t.tag_start..t.value_end, enc::str_prop("NickName", nick));
            }
            None => insert_at_terminator(buf, body, enc::str_prop("NickName", nick), plan)?,
        }
    }

    let list_fields: [(&str, &str, &Option<Vec<String>>); 3] = [
        ("PassiveSkillList", "NameProperty", &edits.passive_skills),
        ("EquipWaza", "EnumProperty", &edits.active_skills),
        ("MasteredWaza", "EnumProperty", &edits.learned_skills),
    ];
    for (name, default_elem, values) in list_fields {
        if let Some(values) = values {
            plan_names_array(buf, body, name, default_elem, values, plan)?;
        }
    }

    if let Some(points) = &edits.status_points {
        plan_status_points(buf, body, "GotStatusPointList", points, plan)?;
    }
    if let Some(points) = &edits.ext_status_points {
        plan_status_points(buf, body, "GotExStatusPointList", points, plan)?;
    }

    Ok(())
}

/// Patch or insert a 1-byte `ByteProperty` field.
fn plan_byte_field(
    buf: &[u8],
    body: usize,
    name: &str,
    value: u8,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    match find_in_body(buf, body, name)? {
        Some(t) if t.type_name == "ByteProperty" && t.size == 1 => {
            plan.patch(t.value_start..t.value_start + 1, vec![value]);
            Ok(())
        }
        Some(t) => Err(edit_err(format!(
            "{name} has unexpected type {} (size {})",
            t.type_name, t.size
        ))),
        None => insert_at_terminator(buf, body, enc::byte_prop(name, value), plan),
    }
}

/// Replace (or insert) a names/enum array property's elements wholesale.
fn plan_names_array(
    buf: &[u8],
    body: usize,
    name: &str,
    default_elem: &str,
    values: &[String],
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    match find_in_body(buf, body, name)? {
        Some(t) if t.type_name == "ArrayProperty" => {
            let mut a = Cursor::new(buf, t.value_start);
            let arr = array_info(&mut a, &t)?;
            plan.patch(arr.elems_start..t.value_end, enc::names_elements(values));
            plan.count(arr.count_offset, values.len() as i64 - arr.count as i64);
            plan.scope_u64(t.size_field, t.value_start..t.value_end);
            Ok(())
        }
        Some(t) => Err(edit_err(format!("{name} is not an array ({})", t.type_name))),
        None => insert_at_terminator(
            buf,
            body,
            enc::names_array_prop(name, default_elem, values),
            plan,
        ),
    }
}

/// Update `StatusPoint` values inside a `Got(Ex)StatusPointList` array of
/// `{ StatusName, StatusPoint }` structs. Only existing names are updated; an
/// unknown name is an error (the caller echoes back names it read from us).
fn plan_status_points(
    buf: &[u8],
    body: usize,
    list_name: &str,
    points: &BTreeMap<String, i32>,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    let t = find_in_body(buf, body, list_name)?
        .filter(|t| t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("StructProperty"))
        .ok_or_else(|| edit_err(format!("{list_name} missing or not a struct array")))?;
    let mut a = Cursor::new(buf, t.value_start);
    let arr = array_info(&mut a, &t)?;

    let mut remaining: BTreeMap<&str, i32> =
        points.iter().map(|(k, v)| (k.as_str(), *v)).collect();

    let mut c = Cursor::new(buf, arr.elems_start);
    for _ in 0..arr.count {
        // Element: property stream { StatusName: Name, StatusPoint: Int }.
        let mut name = None;
        let mut point_off = None;
        loop {
            match read_tag(&mut c)? {
                None => break,
                Some(tag) => {
                    if tag.name == "StatusName" {
                        let mut v = Cursor::new(buf, tag.value_start);
                        name = Some(v.fstring()?);
                    } else if tag.name == "StatusPoint" && tag.type_name == "IntProperty" {
                        point_off = Some(tag.value_start);
                    }
                    c.seek(tag.value_end)?;
                }
            }
        }
        if let (Some(n), Some(off)) = (name, point_off) {
            if let Some(v) = remaining.remove(n.as_str()) {
                plan.patch(off..off + 4, v.to_le_bytes().to_vec());
            }
        }
    }
    c.expect_at(t.value_end, list_name)?;

    if let Some((n, _)) = remaining.iter().next() {
        return Err(edit_err(format!("unknown status name `{n}` in {list_name}")));
    }
    Ok(())
}

/// Insert an encoded property just before a stream's `"None"` terminator.
fn insert_at_terminator(
    buf: &[u8],
    body: usize,
    bytes: Vec<u8>,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    let mut c = Cursor::new(buf, body);
    let scan = find_in_stream(&mut c, "\u{0}--never-matches--")?;
    plan.insert(scan.terminator, bytes);
    Ok(())
}

/// Re-parse the edited buffer and assert the requested character fields.
fn validate_character(
    new_buf: &[u8],
    target: CharTarget,
    edits: &CharacterEdits,
) -> Result<(), SaveError> {
    let gvas = parse_gvas(new_buf, &default_skip_set())?;
    let wsd = gvas
        .root
        .get("worldSaveData")
        .ok_or_else(|| edit_err("post-edit parse lost worldSaveData"))?;
    let map = wsd
        .get_child("CharacterSaveParameterMap")
        .ok_or_else(|| edit_err("post-edit parse lost CharacterSaveParameterMap"))?;
    let (players, pals) = decode_characters(map)?;

    let (level, exp, nickname, passives) = match target {
        CharTarget::Player(uid) => {
            let p = players
                .iter()
                .find(|p| p.uid == uid.to_string())
                .ok_or_else(|| edit_err("post-edit parse lost the player"))?;
            (p.level, p.exp, p.nickname.clone(), Vec::new())
        }
        CharTarget::Instance(iid) => {
            if let Some(p) = pals.iter().find(|p| p.instance_id == iid.to_string()) {
                (p.level, p.exp, p.nickname.clone(), p.passive_skills.clone())
            } else {
                let p = players
                    .iter()
                    .find(|p| p.instance_id == iid.to_string())
                    .ok_or_else(|| edit_err("post-edit parse lost the character"))?;
                (p.level, p.exp, p.nickname.clone(), Vec::new())
            }
        }
    };

    if let Some(want) = edits.level {
        if level != want as i32 {
            return Err(edit_err(format!("post-edit level {level} != {want}")));
        }
    }
    if let Some(want) = edits.exp {
        if exp != want {
            return Err(edit_err(format!("post-edit exp {exp} != {want}")));
        }
    }
    if let Some(want) = &edits.nickname {
        if &nickname != want {
            return Err(edit_err("post-edit nickname mismatch"));
        }
    }
    if let (Some(want), CharTarget::Instance(_)) = (&edits.passive_skills, target) {
        if &passives != want {
            return Err(edit_err("post-edit passive skills mismatch"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public ops: per-player .sav (technologies + points)
// ---------------------------------------------------------------------------

/// Technology edits applied to a per-player `.sav` buffer.
#[derive(Debug, Clone, Default)]
pub struct TechEdits {
    /// Technology ids to add to `UnlockedRecipeTechnologyNames`.
    pub unlock: Vec<String>,
    /// Technology ids to remove.
    pub relock: Vec<String>,
    pub technology_point: Option<i32>,
    pub boss_technology_point: Option<i32>,
}

/// Apply `edits` to a decompressed per-player `.sav` buffer.
pub fn edit_player_technologies(buf: &[u8], edits: &TechEdits) -> Result<Vec<u8>, SaveError> {
    let start = gvas::header_len(buf)?;
    let mut c = Cursor::new(buf, start);
    let sd = find_in_stream(&mut c, "SaveData")?
        .found
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("player save missing SaveData"))?;

    let mut plan = EditPlan::default();
    plan.scope_u64(sd.size_field, sd.value_start..sd.value_end);
    let body = sd.value_start;

    // --- unlocked technology names ------------------------------------------
    let mut new_list: Option<Vec<String>> = None;
    if !edits.unlock.is_empty() || !edits.relock.is_empty() {
        let existing: Vec<String> = match find_in_body(buf, body, "UnlockedRecipeTechnologyNames")? {
            Some(t) if t.type_name == "ArrayProperty" => {
                let mut a = Cursor::new(buf, t.value_start);
                let arr = array_info(&mut a, &t)?;
                let mut v = Vec::with_capacity(arr.count as usize);
                let mut e = Cursor::new(buf, arr.elems_start);
                for _ in 0..arr.count {
                    v.push(e.fstring()?);
                }
                e.expect_at(t.value_end, "UnlockedRecipeTechnologyNames")?;
                v
            }
            _ => Vec::new(),
        };
        // The game stores technology names in inconsistent case (e.g. the
        // catalog's `Workbench` may be `workbench` on disk), so membership
        // tests must be case-insensitive or a relock silently misses and an
        // unlock duplicates.
        let contains_ci = |list: &[String], t: &str| list.iter().any(|x| x.eq_ignore_ascii_case(t));
        let mut list = existing.clone();
        for add in &edits.unlock {
            if !contains_ci(&list, add) {
                list.push(add.clone());
            }
        }
        list.retain(|t| !contains_ci(&edits.relock, t));

        match find_in_body(buf, body, "UnlockedRecipeTechnologyNames")? {
            Some(t) => {
                let mut a = Cursor::new(buf, t.value_start);
                let arr = array_info(&mut a, &t)?;
                plan.patch(arr.elems_start..t.value_end, enc::names_elements(&list));
                plan.count(arr.count_offset, list.len() as i64 - arr.count as i64);
                plan.scope_u64(t.size_field, t.value_start..t.value_end);
            }
            None => insert_at_terminator(
                buf,
                body,
                enc::names_array_prop("UnlockedRecipeTechnologyNames", "NameProperty", &list),
                &mut plan,
            )?,
        }
        new_list = Some(list);
    }

    // --- point fields ---------------------------------------------------------
    for (name, val) in [
        ("TechnologyPoint", edits.technology_point),
        ("bossTechnologyPoint", edits.boss_technology_point),
    ] {
        if let Some(v) = val {
            match find_in_body(buf, body, name)? {
                Some(t) if t.type_name == "IntProperty" => {
                    plan.patch(t.value_start..t.value_start + 4, v.to_le_bytes().to_vec());
                }
                Some(t) => {
                    return Err(edit_err(format!("{name} has unexpected type {}", t.type_name)))
                }
                None => insert_at_terminator(buf, body, enc::int_prop(name, v), &mut plan)?,
            }
        }
    }

    let out = apply(buf, &plan)?;

    // Validate with the strict parser.
    let gvas = parse_gvas(&out, &default_skip_set())?;
    let sd = gvas
        .root
        .get("SaveData")
        .ok_or_else(|| edit_err("post-edit parse lost SaveData"))?;
    if let Some(want) = new_list {
        let got: Vec<String> = match sd.get_child("UnlockedRecipeTechnologyNames") {
            Some(Property::Array {
                value: super::super::props::ArrayValue::Names(v),
                ..
            }) => v.clone(),
            _ => Vec::new(),
        };
        if got != want {
            return Err(edit_err("post-edit technology list mismatch"));
        }
    }
    if let Some(v) = edits.technology_point {
        match sd.get_child("TechnologyPoint") {
            Some(Property::Int(got)) if *got == v as i64 => {}
            other => {
                return Err(edit_err(format!(
                    "post-edit TechnologyPoint mismatch: {other:?}"
                )))
            }
        }
    }
    Ok(out)
}
