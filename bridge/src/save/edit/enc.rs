//! UE property-archive primitive encoders — the write-side mirror of
//! [`super::super::reader::Reader`], byte-for-byte inverse of the layouts the
//! reader consumes (and of `palworld_save_tools/archive.py::FArchiveWriter`).

use uuid::Uuid;

/// Encode an fstring: ASCII fast path (positive length incl. NUL), UTF-16LE
/// otherwise (negative length in code units incl. NUL) — matching
/// `FArchiveWriter.fstring`.
pub fn fstring(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    if s.is_empty() {
        out.extend_from_slice(&0i32.to_le_bytes());
        return out;
    }
    if s.is_ascii() {
        let len = (s.len() + 1) as i32;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    } else {
        let units: Vec<u16> = s.encode_utf16().collect();
        let len = -((units.len() + 1) as i32);
        out.extend_from_slice(&len.to_le_bytes());
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}

/// Encode a UE GUID: the inverse of `Reader::guid`'s per-word LE→BE swap.
pub fn guid(u: Uuid) -> Vec<u8> {
    let b = u.as_bytes();
    vec![
        b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4], b[11], b[10], b[9], b[8], b[15], b[14],
        b[13], b[12],
    ]
}

/// The all-zero GUID (an empty `dynamic_id`).
pub fn nil_guid() -> Vec<u8> {
    vec![0u8; 16]
}

/// Property tag prefix: `name, type, size(u64)`.
fn tag(name: &str, type_name: &str, size: u64) -> Vec<u8> {
    let mut out = fstring(name);
    out.extend(fstring(type_name));
    out.extend_from_slice(&size.to_le_bytes());
    out
}

/// A `"None"`-typed `ByteProperty` holding one raw byte (`Level`, `Rank`,
/// `Talent_*`, …). Tag fields: enum_type `"None"`, optional-guid flag 0.
pub fn byte_prop(name: &str, value: u8) -> Vec<u8> {
    let mut out = tag(name, "ByteProperty", 1);
    out.extend(fstring("None"));
    out.push(0); // optional_guid absent
    out.push(value);
    out
}

/// An `IntProperty` (4-byte LE value).
pub fn int_prop(name: &str, value: i32) -> Vec<u8> {
    let mut out = tag(name, "IntProperty", 4);
    out.push(0);
    out.extend_from_slice(&value.to_le_bytes());
    out
}

/// An `Int64Property` (8-byte LE value).
pub fn int64_prop(name: &str, value: i64) -> Vec<u8> {
    let mut out = tag(name, "Int64Property", 8);
    out.push(0);
    out.extend_from_slice(&value.to_le_bytes());
    out
}

/// A `StrProperty`.
pub fn str_prop(name: &str, value: &str) -> Vec<u8> {
    let body = fstring(value);
    let mut out = tag(name, "StrProperty", body.len() as u64);
    out.push(0);
    out.extend(body);
    out
}

/// An `ArrayProperty` of `NameProperty`/`EnumProperty` elements (a list of
/// fstrings). `elem_type` selects which UE element type the array declares —
/// pass the type the surrounding save already uses for that field (e.g.
/// `PassiveSkillList` is `NameProperty`, `EquipWaza` is `EnumProperty`).
pub fn names_array_prop(name: &str, elem_type: &str, values: &[String]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for v in values {
        body.extend(fstring(v));
    }
    let mut out = tag(name, "ArrayProperty", body.len() as u64);
    out.extend(fstring(elem_type));
    out.push(0);
    out.extend(body);
    out
}

/// Concatenated fstrings — the element bytes of a names array (no count).
pub fn names_elements(values: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in values {
        out.extend(fstring(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::reader::Reader;

    #[test]
    fn fstring_round_trips_through_reader() {
        for s in ["", "Wood", "Pál", "ニックネーム"] {
            let enc = fstring(s);
            let mut r = Reader::new(&enc);
            assert_eq!(r.fstring(), s, "round-trip failed for {s:?}");
            assert!(r.eof());
        }
    }

    #[test]
    fn guid_round_trips_through_reader() {
        let u = uuid::Uuid::parse_str("8c2f1930-0000-4000-8000-00000000abcd").unwrap();
        let enc = guid(u);
        let mut r = Reader::new(&enc);
        assert_eq!(r.guid(), u);
    }
}
