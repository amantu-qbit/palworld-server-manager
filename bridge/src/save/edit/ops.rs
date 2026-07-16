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
    /// `Gender`: `"Male"`/`"Female"` (bare or `EPalGenderType::`-prefixed).
    pub gender: Option<String>,
    /// `GotWorkSuitabilityAddRankList` updates keyed by
    /// `EPalWorkSuitability::…` code; codes not yet on the pal are added.
    pub work_suitability: Option<BTreeMap<String, i32>>,
}

/// Apply `edits` to the matching character. Returns the edited buffer.
pub fn edit_character(
    buf: &[u8],
    target: CharTarget,
    edits: &CharacterEdits,
) -> Result<Vec<u8>, SaveError> {
    let loc = locate_character_entry(buf, target)?;
    let mut plan = EditPlan::default();
    character_scopes(&mut plan, &loc);
    plan_save_parameter_edits(buf, loc.blob.clone(), edits, &mut plan)?;

    let out = apply(buf, &plan)?;
    validate_character(&out, target, edits)?;
    Ok(out)
}

/// A located `CharacterSaveParameterMap` entry.
struct CharEntryLoc {
    wsd_scope: (usize, Range<usize>),
    map_scope: (usize, Range<usize>),
    /// The `u32` entry-count field of the map.
    map_count_offset: usize,
    /// The whole entry: key stream start .. value stream end.
    entry: Range<usize>,
    /// Absolute offset of the key's `InstanceId` 16 on-disk guid bytes.
    key_instance_offset: usize,
    /// `RawData` property size field / value region.
    raw_size_field: usize,
    raw_value: Range<usize>,
    /// The RawData blob bytes (after the count word).
    blob: Range<usize>,
}

/// Register the enclosing scopes for edits inside a character entry's blob.
fn character_scopes(plan: &mut EditPlan, loc: &CharEntryLoc) {
    plan.scope_u64(loc.wsd_scope.0, loc.wsd_scope.1.clone());
    plan.scope_u64(loc.map_scope.0, loc.map_scope.1.clone());
    plan.scope_u64(loc.raw_size_field, loc.raw_value.clone());
    plan.scope_u32(loc.raw_value.start, loc.blob.clone());
}

/// Locate one character entry, capturing every span the edit/delete/clone
/// paths need.
fn locate_character_entry(buf: &[u8], target: CharTarget) -> Result<CharEntryLoc, SaveError> {
    let wsd = world_save_data(buf)?;
    let map_tag = find_in_body(buf, wsd.value_start, "CharacterSaveParameterMap")?
        .filter(|t| t.type_name == "MapProperty")
        .ok_or_else(|| edit_err("worldSaveData missing CharacterSaveParameterMap"))?;

    let mut c = Cursor::new(buf, map_tag.value_start);
    let info = map_info(&mut c)?;

    for _ in 0..info.count {
        let key_start = c.pos();
        // Key stream `{ PlayerUId: Guid, InstanceId: Guid }`, recording the
        // InstanceId's on-disk guid offset for the clone path.
        let mut player_uid = Uuid::nil();
        let mut instance_id = Uuid::nil();
        let mut key_instance_offset = 0usize;
        loop {
            match read_tag(&mut c)? {
                None => break,
                Some(t) => {
                    if t.struct_type.as_deref() == Some("Guid") && t.size == 16 {
                        let g = c.guid()?;
                        match t.name.as_str() {
                            "PlayerUId" => player_uid = g,
                            "InstanceId" => {
                                instance_id = g;
                                key_instance_offset = t.value_start;
                            }
                            _ => {}
                        }
                    }
                    c.seek(t.value_end)?;
                }
            }
        }

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
        let mut e = Cursor::new(buf, value_start);
        let value_end = skip_value_stream(&mut e)?;

        let _ = player_uid;
        return Ok(CharEntryLoc {
            wsd_scope: (wsd.size_field, wsd.value_start..wsd.value_end),
            map_scope: (map_tag.size_field, map_tag.value_start..map_tag.value_end),
            map_count_offset: info.count_offset,
            entry: key_start..value_end,
            key_instance_offset,
            raw_size_field: raw.size_field,
            raw_value: raw.value_start..raw.value_end,
            blob,
        });
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

    if let Some(g) = &edits.gender {
        let value = if g.starts_with("EPalGenderType::") {
            g.clone()
        } else {
            format!("EPalGenderType::{g}")
        };
        match find_in_body(buf, body, "Gender")? {
            Some(t) if t.type_name == "EnumProperty" => {
                plan.patch(t.value_start..t.value_end, enc::fstring(&value));
                plan.scope_u64(t.size_field, t.value_start..t.value_end);
            }
            Some(t) => {
                return Err(edit_err(format!("Gender has unexpected type {}", t.type_name)))
            }
            None => insert_at_terminator(
                buf,
                body,
                enc::enum_prop("Gender", "EPalGenderType", &value),
                plan,
            )?,
        }
    }

    if let Some(ws) = &edits.work_suitability {
        plan_work_suitability(buf, body, ws, plan)?;
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

// ---------------------------------------------------------------------------
// Work suitability
// ---------------------------------------------------------------------------

/// Update/add entries in `GotWorkSuitabilityAddRankList` (an array of
/// `{ WorkSuitability: Enum, Rank: Int }` structs). Existing codes get their
/// `Rank` patched; new codes get a fresh element appended.
fn plan_work_suitability(
    buf: &[u8],
    body: usize,
    ranks: &BTreeMap<String, i32>,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    let mut remaining: BTreeMap<&str, i32> = ranks.iter().map(|(k, v)| (k.as_str(), *v)).collect();

    match find_in_body(buf, body, "GotWorkSuitabilityAddRankList")? {
        Some(t) if t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("StructProperty") => {
            let mut a = Cursor::new(buf, t.value_start);
            let arr = array_info(&mut a, &t)?;
            let inner = arr
                .inner
                .clone()
                .ok_or_else(|| edit_err("GotWorkSuitabilityAddRankList missing inner header"))?;

            let mut c = Cursor::new(buf, arr.elems_start);
            for _ in 0..arr.count {
                let mut work = None;
                let mut rank_off = None;
                loop {
                    match read_tag(&mut c)? {
                        None => break,
                        Some(tag) => {
                            if tag.name == "WorkSuitability" && tag.type_name == "EnumProperty" {
                                let mut v = Cursor::new(buf, tag.value_start);
                                work = Some(v.fstring()?);
                            } else if tag.name == "Rank" && tag.type_name == "IntProperty" {
                                rank_off = Some(tag.value_start);
                            }
                            c.seek(tag.value_end)?;
                        }
                    }
                }
                if let (Some(w), Some(off)) = (work, rank_off) {
                    if let Some(v) = remaining.remove(w.as_str()) {
                        plan.patch(off..off + 4, v.to_le_bytes().to_vec());
                    }
                }
            }
            c.expect_at(t.value_end, "GotWorkSuitabilityAddRankList")?;

            if !remaining.is_empty() {
                // Append new elements at the array end; the boundary insert is
                // deliberately inside the array scopes so both size fields grow.
                let mut added = 0i64;
                for (code, v) in &remaining {
                    let mut elem = enc::enum_prop("WorkSuitability", "EPalWorkSuitability", code);
                    elem.extend(enc::int_prop("Rank", *v));
                    elem.extend(enc::fstring("None"));
                    plan.insert(t.value_end, elem);
                    added += 1;
                }
                plan.count(arr.count_offset, added);
                plan.scope_u64(t.size_field, t.value_start..t.value_end);
                plan.scope_u64(inner.size_field, arr.elems_start..t.value_end);
            }
            Ok(())
        }
        Some(t) => Err(edit_err(format!(
            "GotWorkSuitabilityAddRankList has unexpected shape ({})",
            t.type_name
        ))),
        None => Err(edit_err(
            "pal has no GotWorkSuitabilityAddRankList to edit".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Heal
// ---------------------------------------------------------------------------

/// Vitals written by [`heal_pal`]. `hp` is the on-disk fixed-point value
/// (max HP × 1000), computed by the caller from the species catalog.
#[derive(Debug, Clone, Copy)]
pub struct HealValues {
    pub hp: Option<i64>,
    pub stomach: f32,
    pub sanity: f32,
}

/// Fully restore a pal, porting `palworld-save-pal pal.py::heal` (+ the
/// `hp = max_hp` revive step): remove the sick/faint state properties, reset
/// sanity and stomach, and set HP to the computed maximum.
pub fn heal_pal(buf: &[u8], instance_id: Uuid, heal: &HealValues) -> Result<Vec<u8>, SaveError> {
    let loc = locate_character_entry(buf, CharTarget::Instance(instance_id))?;
    let mut plan = EditPlan::default();
    character_scopes(&mut plan, &loc);

    let mut c = Cursor::new(buf, loc.blob.start);
    let sp = find_in_stream(&mut c, "SaveParameter")?
        .found
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("character RawData missing SaveParameter"))?;
    plan.scope_u64(sp.size_field, sp.value_start..sp.value_end);
    let body = sp.value_start;

    // The sick/faint state properties upstream's heal removes (PAL_SICK_TYPES
    // minus SanityValue, which is reset below instead of removed).
    for sick in ["PalReviveTimer", "PhysicalHealth", "WorkerSick", "HungerType"] {
        if let Some(t) = find_in_body(buf, body, sick)? {
            plan.delete(t.tag_start..t.value_end);
        }
    }

    plan_float_field(buf, body, "SanityValue", heal.sanity, &mut plan)?;
    plan_float_field(buf, body, "FullStomach", heal.stomach, &mut plan)?;

    if let Some(hp) = heal.hp {
        match find_in_body(buf, body, "Hp")? {
            Some(t) if t.type_name == "StructProperty" => {
                let value = find_in_body(buf, t.value_start, "Value")?;
                match value {
                    Some(v) if v.type_name == "Int64Property" => {
                        plan.patch(v.value_start..v.value_start + 8, hp.to_le_bytes().to_vec());
                    }
                    _ => {
                        plan.patch(t.tag_start..t.value_end, enc::fixed_point64_prop("Hp", hp));
                    }
                }
            }
            Some(t) => {
                plan.patch(t.tag_start..t.value_end, enc::fixed_point64_prop("Hp", hp));
            }
            None => insert_at_terminator(buf, body, enc::fixed_point64_prop("Hp", hp), &mut plan)?,
        }
    }

    let out = apply(buf, &plan)?;

    // Validate through the production decoder.
    let gvas = parse_gvas(&out, &default_skip_set())?;
    let map = gvas
        .root
        .get("worldSaveData")
        .and_then(|w| w.get_child("CharacterSaveParameterMap"))
        .ok_or_else(|| edit_err("post-edit parse lost CharacterSaveParameterMap"))?;
    let (_, pals) = decode_characters(map)?;
    let p = pals
        .iter()
        .find(|p| p.instance_id == instance_id.to_string())
        .ok_or_else(|| edit_err("post-edit parse lost the pal"))?;
    if p.sanity != heal.sanity as i32 {
        return Err(edit_err(format!("post-heal sanity {} != {}", p.sanity, heal.sanity)));
    }
    if let Some(hp) = heal.hp {
        if i64::from(p.hp) != hp && p.hp != i32::MAX {
            return Err(edit_err(format!("post-heal hp {} != {hp}", p.hp)));
        }
    }
    Ok(out)
}

/// Patch or insert a `FloatProperty` field.
fn plan_float_field(
    buf: &[u8],
    body: usize,
    name: &str,
    value: f32,
    plan: &mut EditPlan,
) -> Result<(), SaveError> {
    match find_in_body(buf, body, name)? {
        Some(t) if t.type_name == "FloatProperty" => {
            plan.patch(t.value_start..t.value_start + 4, value.to_le_bytes().to_vec());
            Ok(())
        }
        Some(t) => Err(edit_err(format!("{name} has unexpected type {}", t.type_name))),
        None => insert_at_terminator(buf, body, enc::float_prop(name, value), plan),
    }
}

// ---------------------------------------------------------------------------
// Character containers (pal box / party / base) — location helpers
// ---------------------------------------------------------------------------

/// One `CharacterContainerSaveData` slot element: `{ SlotIndex: Int,
/// RawData: Bytes(player_uid ‖ instance_id ‖ tribe) }`.
struct CcSlotElem {
    range: Range<usize>,
    slot_index: i32,
    /// Absolute offset of the `SlotIndex` 4-byte value.
    slot_index_value: usize,
    /// The 33-byte RawData blob.
    blob: Range<usize>,
    instance: Uuid,
}

/// One located character container.
struct CcLoc {
    container_id: Uuid,
    slot_num: i32,
    arr_size_field: usize,
    arr_value: Range<usize>,
    inner_size_field: usize,
    elems_start: usize,
    count_offset: usize,
    elements: Vec<CcSlotElem>,
}

/// Locate every character container (with the map/wsd scopes shared by all).
struct CcIndex {
    map_scope: (usize, Range<usize>),
    containers: Vec<CcLoc>,
}

fn locate_character_containers(buf: &[u8]) -> Result<CcIndex, SaveError> {
    let wsd = world_save_data(buf)?;
    let map_tag = find_in_body(buf, wsd.value_start, "CharacterContainerSaveData")?
        .filter(|t| t.type_name == "MapProperty")
        .ok_or_else(|| edit_err("worldSaveData missing CharacterContainerSaveData"))?;

    let mut c = Cursor::new(buf, map_tag.value_start);
    let info = map_info(&mut c)?;
    let mut containers = Vec::with_capacity(info.count as usize);

    for _ in 0..info.count {
        let key = read_guid_key_stream(&mut c)?;
        let container_id = key
            .get("ID")
            .copied()
            .ok_or_else(|| edit_err("CharacterContainerSaveData key missing ID"))?;

        let value_start = c.pos();
        let slot_num = match find_in_body(buf, value_start, "SlotNum")? {
            Some(t) if t.type_name == "IntProperty" => {
                let mut v = Cursor::new(buf, t.value_start);
                v.read_i32()?
            }
            _ => 0,
        };
        let slots_tag = find_in_body(buf, value_start, "Slots")?
            .filter(|t| {
                t.type_name == "ArrayProperty" && t.elem_type.as_deref() == Some("StructProperty")
            })
            .ok_or_else(|| edit_err("character container missing Slots array"))?;
        let mut a = Cursor::new(buf, slots_tag.value_start);
        let arr = array_info(&mut a, &slots_tag)?;
        let inner = arr
            .inner
            .clone()
            .ok_or_else(|| edit_err("character container Slots missing inner header"))?;

        let mut elements = Vec::with_capacity(arr.count as usize);
        let mut e = Cursor::new(buf, arr.elems_start);
        for _ in 0..arr.count {
            let start = e.pos();
            let mut slot_index = 0i32;
            let mut slot_index_value = 0usize;
            let mut blob = 0..0;
            let mut instance = Uuid::nil();
            loop {
                match read_tag(&mut e)? {
                    None => break,
                    Some(tag) => {
                        if tag.name == "SlotIndex" && tag.type_name == "IntProperty" {
                            let mut v = Cursor::new(buf, tag.value_start);
                            slot_index = v.read_i32()?;
                            slot_index_value = tag.value_start;
                        } else if tag.name == "RawData"
                            && tag.type_name == "ArrayProperty"
                            && tag.elem_type.as_deref() == Some("ByteProperty")
                        {
                            blob = tag.value_start + 4..tag.value_end;
                            if blob.len() >= 32 {
                                let mut v = Cursor::new(buf, blob.start);
                                let _player = v.guid()?;
                                instance = v.guid()?;
                            }
                        }
                        e.seek(tag.value_end)?;
                    }
                }
            }
            elements.push(CcSlotElem {
                range: start..e.pos(),
                slot_index,
                slot_index_value,
                blob,
                instance,
            });
        }
        e.expect_at(slots_tag.value_end, "character container Slots")?;

        // Advance the shared map cursor past this entry's value stream so the
        // next iteration starts on the next key (the sub-parses above all used
        // their own cursors).
        let mut v = Cursor::new(buf, value_start);
        let value_end = skip_value_stream(&mut v)?;
        c.seek(value_end)?;

        containers.push(CcLoc {
            container_id,
            slot_num,
            arr_size_field: slots_tag.size_field,
            arr_value: slots_tag.value_start..slots_tag.value_end,
            inner_size_field: inner.size_field,
            elems_start: arr.elems_start,
            count_offset: arr.count_offset,
            elements,
        });
    }

    let _ = &wsd;
    Ok(CcIndex {
        map_scope: (map_tag.size_field, map_tag.value_start..map_tag.value_end),
        containers,
    })
}

// ---------------------------------------------------------------------------
// Delete pal
// ---------------------------------------------------------------------------

/// Remove a pal: its `CharacterSaveParameterMap` entry plus every character
/// container slot referencing it (upstream `remove_pal` removes the slot
/// element outright — slots carry explicit `SlotIndex`es, they are not
/// positional).
pub fn delete_pal(buf: &[u8], instance_id: Uuid) -> Result<Vec<u8>, SaveError> {
    let loc = locate_character_entry(buf, CharTarget::Instance(instance_id))?;
    let mut plan = EditPlan::default();
    plan.scope_u64(loc.wsd_scope.0, loc.wsd_scope.1.clone());
    plan.scope_u64(loc.map_scope.0, loc.map_scope.1.clone());
    plan.delete(loc.entry.clone());
    plan.count(loc.map_count_offset, -1);

    let cc = locate_character_containers(buf)?;
    plan.scope_u64(cc.map_scope.0, cc.map_scope.1.clone());
    for container in &cc.containers {
        let doomed: Vec<&CcSlotElem> = container
            .elements
            .iter()
            .filter(|e| e.instance == instance_id)
            .collect();
        if doomed.is_empty() {
            continue;
        }
        for e in &doomed {
            plan.delete(e.range.clone());
        }
        plan.count(container.count_offset, -(doomed.len() as i64));
        plan.scope_u64(container.arr_size_field, container.arr_value.clone());
        plan.scope_u64(container.inner_size_field, container.elems_start..container.arr_value.end);
    }

    let out = apply(buf, &plan)?;

    let gvas = parse_gvas(&out, &default_skip_set())?;
    let wsd = gvas
        .root
        .get("worldSaveData")
        .ok_or_else(|| edit_err("post-edit parse lost worldSaveData"))?;
    let (_, pals) = decode_characters(
        wsd.get_child("CharacterSaveParameterMap")
            .ok_or_else(|| edit_err("post-edit parse lost CharacterSaveParameterMap"))?,
    )?;
    if pals.iter().any(|p| p.instance_id == instance_id.to_string()) {
        return Err(edit_err("post-delete parse still finds the pal"));
    }
    if let Some(ccs) = wsd.get_child("CharacterContainerSaveData") {
        let containers = super::super::containers::decode_character_containers(ccs)?;
        if containers
            .values()
            .flatten()
            .any(|s| s.pal_id == instance_id.to_string())
        {
            return Err(edit_err("post-delete container slot still references the pal"));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Clone pal
// ---------------------------------------------------------------------------

/// Duplicate a pal into `target_container` (normally the owner's pal box)
/// under `new_instance_id`. The copy keeps every stat/skill; only its
/// identity and box slot differ.
pub fn clone_pal(
    buf: &[u8],
    instance_id: Uuid,
    target_container: Uuid,
    new_instance_id: Uuid,
) -> Result<Vec<u8>, SaveError> {
    if is_nil(new_instance_id) || new_instance_id == instance_id {
        return Err(edit_err("invalid new instance id"));
    }
    let loc = locate_character_entry(buf, CharTarget::Instance(instance_id))?;
    let cc = locate_character_containers(buf)?;
    let target = cc
        .containers
        .iter()
        .find(|c| c.container_id == target_container)
        .ok_or_else(|| edit_err(format!("character container {target_container} not found")))?;

    // Free slot: first index in 0..slot_num not taken by a live entry.
    let used: std::collections::HashSet<i32> = target
        .elements
        .iter()
        .filter(|e| !is_nil(e.instance))
        .map(|e| e.slot_index)
        .collect();
    let free_slot = (0..target.slot_num.max(0))
        .find(|i| !used.contains(i))
        .ok_or_else(|| edit_err("target container is full"))?;

    // Template slot element: reuse a dead (nil-instance) element in place if
    // one exists, else append a patched copy of the source pal's own element.
    let source_elem = cc
        .containers
        .iter()
        .flat_map(|c| c.elements.iter())
        .find(|e| e.instance == instance_id)
        .ok_or_else(|| edit_err("source pal has no container slot to copy"))?;

    let mut plan = EditPlan::default();
    plan.scope_u64(loc.wsd_scope.0, loc.wsd_scope.1.clone());
    plan.scope_u64(loc.map_scope.0, loc.map_scope.1.clone());
    plan.scope_u64(cc.map_scope.0, cc.map_scope.1.clone());

    // --- new character map entry -----------------------------------------
    let mut entry = buf[loc.entry.clone()].to_vec();
    let key_rel = loc.key_instance_offset - loc.entry.start;
    entry[key_rel..key_rel + 16].copy_from_slice(&enc::guid(new_instance_id));
    // Rewrite SlotID (container + index) inside the copied blob so the clone
    // points at its own box slot. All patches are same-length, so the copy
    // needs no size fixups.
    rewrite_clone_slot_id(&mut entry, &loc, target_container, free_slot)?;
    plan.insert(loc.map_scope.1.end, entry);
    plan.count(loc.map_count_offset, 1);

    // --- container slot ----------------------------------------------------
    if let Some(dead) = target
        .elements
        .iter()
        .find(|e| is_nil(e.instance) && e.slot_index == free_slot)
    {
        // Reuse the dead element in place: same-length patches only.
        let mut blob_patch = buf[dead.blob.clone()].to_vec();
        blob_patch[16..32].copy_from_slice(&enc::guid(new_instance_id));
        plan.patch(dead.blob.clone(), blob_patch);
    } else {
        let mut elem = buf[source_elem.range.clone()].to_vec();
        let si_rel = source_elem.slot_index_value - source_elem.range.start;
        elem[si_rel..si_rel + 4].copy_from_slice(&free_slot.to_le_bytes());
        let blob_rel = source_elem.blob.start - source_elem.range.start;
        elem[blob_rel + 16..blob_rel + 32].copy_from_slice(&enc::guid(new_instance_id));
        plan.insert(target.arr_value.end, elem);
        plan.count(target.count_offset, 1);
        plan.scope_u64(target.arr_size_field, target.arr_value.clone());
        plan.scope_u64(target.inner_size_field, target.elems_start..target.arr_value.end);
    }

    let out = apply(buf, &plan)?;

    // Validate: the clone decodes with the source's species, in the target box.
    let gvas = parse_gvas(&out, &default_skip_set())?;
    let map = gvas
        .root
        .get("worldSaveData")
        .and_then(|w| w.get_child("CharacterSaveParameterMap"))
        .ok_or_else(|| edit_err("post-clone parse lost CharacterSaveParameterMap"))?;
    let (_, pals) = decode_characters(map)?;
    let source = pals
        .iter()
        .find(|p| p.instance_id == instance_id.to_string())
        .ok_or_else(|| edit_err("post-clone parse lost the source pal"))?;
    let clone = pals
        .iter()
        .find(|p| p.instance_id == new_instance_id.to_string())
        .ok_or_else(|| edit_err("post-clone parse cannot find the clone"))?;
    if clone.character_id != source.character_id
        || clone.level != source.level
        || clone.storage_id != target_container.to_string()
        || clone.storage_slot != free_slot
    {
        return Err(edit_err("post-clone validation mismatch"));
    }
    Ok(out)
}

/// Inside a copied character-map entry, retarget `SaveParameter.SlotID`
/// (`ContainerId.ID` guid + `SlotIndex` int) with same-length patches.
fn rewrite_clone_slot_id(
    entry: &mut [u8],
    loc: &CharEntryLoc,
    container: Uuid,
    slot: i32,
) -> Result<(), SaveError> {
    let blob_rel = (loc.blob.start - loc.entry.start)..(loc.blob.end - loc.entry.start);
    let snapshot = entry.to_vec();
    let mut c = Cursor::new(&snapshot, blob_rel.start);
    let sp = find_in_stream(&mut c, "SaveParameter")?
        .found
        .ok_or_else(|| edit_err("clone: RawData missing SaveParameter"))?;
    // `SlotID` (current) with a legacy `SlotId` fallback, mirroring the reader.
    let slot_tag = match find_in_body(&snapshot, sp.value_start, "SlotID")? {
        Some(t) => Some(t),
        None => find_in_body(&snapshot, sp.value_start, "SlotId")?,
    };
    let slot_tag = slot_tag
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("clone: SaveParameter missing SlotID"))?;

    let cid = find_in_body(&snapshot, slot_tag.value_start, "ContainerId")?
        .filter(|t| t.type_name == "StructProperty")
        .ok_or_else(|| edit_err("clone: SlotID missing ContainerId"))?;
    let id = find_in_body(&snapshot, cid.value_start, "ID")?
        .filter(|t| t.struct_type.as_deref() == Some("Guid"))
        .ok_or_else(|| edit_err("clone: ContainerId missing ID"))?;
    entry[id.value_start..id.value_start + 16].copy_from_slice(&enc::guid(container));

    let si = find_in_body(&snapshot, slot_tag.value_start, "SlotIndex")?
        .filter(|t| t.type_name == "IntProperty")
        .ok_or_else(|| edit_err("clone: SlotID missing SlotIndex"))?;
    entry[si.value_start..si.value_start + 4].copy_from_slice(&slot.to_le_bytes());
    Ok(())
}
