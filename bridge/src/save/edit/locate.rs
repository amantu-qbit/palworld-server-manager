//! Targeted GVAS locator: descend to a property and record byte spans.
//!
//! The read pipeline ([`super::super::gvas`]) builds a value tree and discards
//! offsets; editing needs the opposite — exact byte ranges plus the offsets of
//! every enclosing declared-size field, and no value tree at all. This module
//! walks the same wire format as `gvas.rs` (each dispatch arm mirrors the
//! reader's) but instead of materializing values it returns [`PropTag`]s: the
//! property's tag fields and the absolute span of its declared-size region.
//!
//! ## Size semantics (UE property tag)
//!
//! Every property is serialized as
//! `name: fstring, type: fstring, size: u64, <type-specific tag fields>, <value>`
//! where `size` counts only the `<value>` bytes. The type-specific tag fields
//! (excluded from `size`) are exactly the ones `gvas.rs::skip_decode` consumes
//! before grabbing `size` bytes:
//!
//! | type            | tag fields (excluded from `size`)                  |
//! |-----------------|----------------------------------------------------|
//! | scalar/Str/Name | optional_guid                                      |
//! | Enum/Byte       | enum_type fstring, optional_guid                   |
//! | Bool            | value byte, optional_guid (`size == 0`)            |
//! | Struct          | struct_type fstring, struct guid, optional_guid    |
//! | Array/Set       | element-type fstring, optional_guid                |
//! | Map             | key/value-type fstrings, optional_guid             |
//!
//! Siblings on the descent path are skipped by seeking `size` bytes past the
//! tag. Wherever the locator *does* descend (and wherever an edit will splice),
//! callers assert the walked length equals the declared size via
//! [`Cursor::expect_at`] — so every size field an edit depends on is validated
//! against reality before any patch is planned. Post-edit, ops re-parse the
//! whole buffer with the strict reader as a final gate.

use uuid::Uuid;

use super::super::decompress::SaveError;

/// Absolute-offset cursor over the decompressed GVAS buffer.
///
/// Unlike [`super::super::reader::Reader`] (which panics on underrun by
/// contract), the locator returns `Err` on malformed input: edits run against
/// user-selected files and must fail as errors, not panics, before any write.
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

fn edit_err(msg: impl Into<String>) -> SaveError {
    SaveError::Edit(msg.into())
}

impl<'a> Cursor<'a> {
    pub fn new(buf: &'a [u8], pos: usize) -> Self {
        Self { buf, pos }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) -> Result<(), SaveError> {
        if pos > self.buf.len() {
            return Err(edit_err(format!("seek past EOF: {pos} > {}", self.buf.len())));
        }
        self.pos = pos;
        Ok(())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.buf.len())
            .ok_or_else(|| edit_err(format!("underrun: need {n} bytes at {}", self.pos)))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn read_u8(&mut self) -> Result<u8, SaveError> {
        Ok(self.take(1)?[0])
    }

    pub fn read_i32(&mut self) -> Result<i32, SaveError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn read_u32(&mut self) -> Result<u32, SaveError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn read_u64(&mut self) -> Result<u64, SaveError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// UE length-prefixed string; layout identical to `Reader::fstring`.
    pub fn fstring(&mut self) -> Result<String, SaveError> {
        let size = self.read_i32()?;
        if size == 0 {
            return Ok(String::new());
        }
        if size < 0 {
            let units = (-(size as i64)) as usize;
            let bytes = self.take(units.checked_mul(2).ok_or_else(|| edit_err("fstring overflow"))?)?;
            let body = &bytes[..bytes.len() - 2];
            let u16s: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&u16s))
        } else {
            let bytes = self.take(size as usize)?;
            let body = &bytes[..bytes.len() - 1];
            Ok(String::from_utf8_lossy(body).into_owned())
        }
    }

    /// 16-byte UE GUID with the same per-word LE→BE swap as `Reader::guid`.
    pub fn guid(&mut self) -> Result<Uuid, SaveError> {
        let r = self.take(16)?;
        Ok(Uuid::from_bytes([
            r[3], r[2], r[1], r[0], r[7], r[6], r[5], r[4], r[11], r[10], r[9], r[8], r[15],
            r[14], r[13], r[12],
        ]))
    }

    pub fn optional_guid(&mut self) -> Result<Option<Uuid>, SaveError> {
        if self.read_u8()? != 0 {
            Ok(Some(self.guid()?))
        } else {
            Ok(None)
        }
    }

    pub fn skip(&mut self, n: usize) -> Result<(), SaveError> {
        self.take(n).map(|_| ())
    }

    /// Assert the cursor sits exactly at `expected` — used after walking a
    /// region whose declared size an edit will later fix up, so a
    /// size-vs-reality mismatch is caught before any patch is planned.
    pub fn expect_at(&self, expected: usize, what: &str) -> Result<(), SaveError> {
        if self.pos != expected {
            return Err(edit_err(format!(
                "declared size mismatch walking {what}: at {} expected {expected}",
                self.pos
            )));
        }
        Ok(())
    }
}

/// A located property: tag fields plus the absolute offsets an edit needs.
#[derive(Debug, Clone)]
pub struct PropTag {
    pub name: String,
    pub type_name: String,
    /// Offset of the property's name fstring (start of the whole property).
    pub tag_start: usize,
    /// Offset of the `u64` declared-size field.
    pub size_field: usize,
    pub size: u64,
    /// Start of the declared-size region (`size` counts from here).
    pub value_start: usize,
    /// `value_start + size`.
    pub value_end: usize,
    /// `StructProperty` only: the struct type name.
    pub struct_type: Option<String>,
    /// `ArrayProperty`/`SetProperty` only: the element type name.
    pub elem_type: Option<String>,
    /// `BoolProperty` only: offset of the 1-byte value inside the tag.
    pub bool_value_offset: Option<usize>,
}

/// Result of scanning one property stream: the match (if any) and the offset
/// of the stream's `"None"` terminator — the insertion point for new
/// properties.
#[derive(Debug, Clone)]
pub struct StreamScan {
    pub found: Option<PropTag>,
    /// Offset of the terminating `"None"` fstring.
    pub terminator: usize,
    /// Offset just past the terminator (where trailing/blob bytes begin).
    pub end: usize,
}

/// Read one property tag at the cursor (which must sit on a property name).
/// Returns `Ok(None)` on the `"None"` stream terminator. On success the
/// cursor is left at `value_start`.
pub fn read_tag(c: &mut Cursor) -> Result<Option<PropTag>, SaveError> {
    let tag_start = c.pos();
    let name = c.fstring()?;
    if name == "None" {
        return Ok(None);
    }
    let type_name = c.fstring()?;
    let size_field = c.pos();
    let size = c.read_u64()?;

    let mut struct_type = None;
    let mut elem_type = None;
    let mut bool_value_offset = None;

    // Mirrors `gvas.rs::read_property`'s per-type tag fields (the bytes the
    // declared size does NOT cover).
    match type_name.as_str() {
        "StructProperty" => {
            struct_type = Some(c.fstring()?);
            let _struct_id = c.guid()?;
            let _ = c.optional_guid()?;
        }
        "ArrayProperty" | "SetProperty" => {
            elem_type = Some(c.fstring()?);
            let _ = c.optional_guid()?;
        }
        "MapProperty" => {
            let _key_type = c.fstring()?;
            let _value_type = c.fstring()?;
            let _ = c.optional_guid()?;
        }
        "EnumProperty" | "ByteProperty" => {
            let _enum_type = c.fstring()?;
            let _ = c.optional_guid()?;
        }
        "BoolProperty" => {
            bool_value_offset = Some(c.pos());
            let _value = c.read_u8()?;
            let _ = c.optional_guid()?;
        }
        "IntProperty" | "Int64Property" | "UInt16Property" | "UInt32Property"
        | "UInt64Property" | "FixedPoint64Property" | "FloatProperty" | "StrProperty"
        | "NameProperty" => {
            let _ = c.optional_guid()?;
        }
        other => {
            return Err(edit_err(format!(
                "unhandled property type `{other}` at offset {tag_start}"
            )))
        }
    }

    let value_start = c.pos();
    let value_end = value_start
        .checked_add(size as usize)
        .ok_or_else(|| edit_err("declared size overflow"))?;
    Ok(Some(PropTag {
        name,
        type_name,
        tag_start,
        size_field,
        size,
        value_start,
        value_end,
        struct_type,
        elem_type,
        bool_value_offset,
    }))
}

/// Scan a property stream from the cursor for `name`, skipping non-matching
/// siblings by their declared size. Stops at the match or the `"None"`
/// terminator. On a match the cursor is at the match's `value_start`.
pub fn find_in_stream(c: &mut Cursor, name: &str) -> Result<StreamScan, SaveError> {
    loop {
        let tag_start = c.pos();
        match read_tag(c)? {
            None => {
                return Ok(StreamScan {
                    found: None,
                    terminator: tag_start,
                    end: c.pos(),
                })
            }
            Some(tag) => {
                if tag.name == name {
                    return Ok(StreamScan {
                        found: Some(tag),
                        terminator: 0,
                        end: 0,
                    });
                }
                c.seek(tag.value_end)?;
            }
        }
    }
}

/// Skip a whole property stream (to and past its `"None"` terminator),
/// returning the terminator offset.
pub fn skip_stream(c: &mut Cursor) -> Result<usize, SaveError> {
    loop {
        let tag_start = c.pos();
        match read_tag(c)? {
            None => return Ok(tag_start),
            Some(tag) => c.seek(tag.value_end)?,
        }
    }
}

/// Header of an `ArrayProperty` value region, cursor left at `elems_start`.
#[derive(Debug, Clone)]
pub struct ArrayInfo {
    /// Offset of the `u32` element count.
    pub count_offset: usize,
    pub count: u32,
    /// `StructProperty` arrays only: the shared inner header.
    pub inner: Option<InnerStructHeader>,
    /// Offset of the first element.
    pub elems_start: usize,
}

/// The shared inner header of an `ArrayProperty<StructProperty>`.
#[derive(Debug, Clone)]
pub struct InnerStructHeader {
    pub prop_name: String,
    pub type_name: String,
    /// Offset of the inner `u64` size (counts the elements' bytes only).
    pub size_field: usize,
    pub size: u64,
}

/// Read an array value-region header. `tag` must be an `ArrayProperty` and the
/// cursor at its `value_start`.
pub fn array_info(c: &mut Cursor, tag: &PropTag) -> Result<ArrayInfo, SaveError> {
    let count_offset = c.pos();
    let count = c.read_u32()?;
    let inner = if tag.elem_type.as_deref() == Some("StructProperty") {
        let prop_name = c.fstring()?;
        let _prop_type = c.fstring()?;
        let size_field = c.pos();
        let size = c.read_u64()?;
        let type_name = c.fstring()?;
        let _inner_id = c.guid()?;
        c.skip(1)?;
        Some(InnerStructHeader {
            prop_name,
            type_name,
            size_field,
            size,
        })
    } else {
        None
    };
    Ok(ArrayInfo {
        count_offset,
        count,
        inner,
        elems_start: c.pos(),
    })
}

/// Header of a `MapProperty` value region, cursor left at `entries_start`.
#[derive(Debug, Clone)]
pub struct MapInfo {
    /// Offset of the `u32` entry count.
    pub count_offset: usize,
    pub count: u32,
    pub entries_start: usize,
}

/// Read a map value-region header (`u32` padding, then `u32` count). `tag`
/// must be a `MapProperty` with the cursor at its `value_start`.
pub fn map_info(c: &mut Cursor) -> Result<MapInfo, SaveError> {
    let _padding = c.read_u32()?;
    let count_offset = c.pos();
    let count = c.read_u32()?;
    Ok(MapInfo {
        count_offset,
        count,
        entries_start: c.pos(),
    })
}
