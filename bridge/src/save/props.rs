//! Generic Unreal Engine property value tree.
//!
//! A faithful in-memory model of the values produced by the GVAS property
//! (de)serializer in `palworld_save_tools/archive.py`. The parser in
//! [`super::gvas`] walks the byte stream and builds a tree of these nodes;
//! this module only defines the shapes and the navigation helpers.
//!
//! Scope for Task 4: this is the *generic* GVAS representation only. Palworld's
//! custom `RawData` blobs (character/item-container/group/… payloads) are **not**
//! interpreted here — they remain their raw GVAS form:
//! - a `ByteProperty` array collapses to [`Property::Bytes`], and
//! - a skip-decoded blob (see [`super::gvas::default_skip_set`]) is stored as
//!   [`Property::Bytes`] verbatim.
//!
//! Later tasks (6–8) re-parse those bytes into structured Palworld types.

use std::collections::BTreeMap;

use uuid::Uuid;

/// A generic UE property value.
///
/// Scalars carry only their value (the optional per-property GUID that UE
/// serializes is consumed but not retained — no current consumer needs it).
#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    /// Any signed/unsigned integer property (`IntProperty`, `Int64Property`,
    /// `UInt16/32/64Property`, `FixedPoint64Property`), widened to `i64`.
    Int(i64),
    /// `FloatProperty` (widened from `f32`).
    Float(f64),
    /// `StrProperty`.
    Str(String),
    /// `NameProperty`.
    Name(String),
    /// `BoolProperty`.
    Bool(bool),
    /// `EnumProperty`: the enum type name plus the selected variant.
    Enum { enum_type: String, value: String },
    /// `ByteProperty`: either a raw byte (when the enum type is `"None"`) or a
    /// labelled enum-variant string.
    Byte { enum_type: String, value: ByteVal },
    /// `StructProperty` (or a bare struct value inside a map/array/set).
    Struct { struct_type: String, value: StructValue },
    /// `ArrayProperty` for non-byte element types. Byte arrays collapse to
    /// [`Property::Bytes`] instead.
    Array { array_type: String, value: ArrayValue },
    /// `SetProperty`.
    Set { set_type: String, value: SetValue },
    /// `MapProperty`.
    Map {
        key_type: String,
        value_type: String,
        entries: Vec<MapEntry>,
    },
    /// Raw GVAS bytes: a `ByteProperty` array payload, or the verbatim body of a
    /// skip-decoded property.
    Bytes(Vec<u8>),
}

/// A single key/value pair inside a [`Property::Map`].
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: Property,
    pub value: Property,
}

/// The value carried by a `ByteProperty`.
#[derive(Debug, Clone, PartialEq)]
pub enum ByteVal {
    /// Raw byte, when the property's enum type is `"None"`.
    Byte(u8),
    /// Enum-variant name, when the property is a labelled byte.
    Label(String),
}

/// The value of a `StructProperty`.
///
/// Well-known primitive struct types are decoded to their fields; every other
/// struct type is a nested property set (read until the `"None"` terminator).
#[derive(Debug, Clone, PartialEq)]
pub enum StructValue {
    /// A generic `UStruct` serialized as a nested property set.
    Properties(BTreeMap<String, Property>),
    Vector { x: f64, y: f64, z: f64 },
    Quat { x: f64, y: f64, z: f64, w: f64 },
    LinearColor { r: f32, g: f32, b: f32, a: f32 },
    Color { b: u8, g: u8, r: u8, a: u8 },
    /// `DateTime` ticks.
    DateTime(u64),
    Guid(Uuid),
}

/// The value of an `ArrayProperty` (non-byte element types).
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayValue {
    /// An array of structs, all of the same `type_name`.
    Structs {
        prop_name: String,
        prop_type: String,
        type_name: String,
        values: Vec<StructValue>,
    },
    /// `EnumProperty` / `NameProperty` element arrays (a list of strings).
    Names(Vec<String>),
    /// `Guid` element arrays.
    Guids(Vec<Uuid>),
}

/// The value of a `SetProperty`.
#[derive(Debug, Clone, PartialEq)]
pub enum SetValue {
    /// A set of structs, all of the same `struct_type`.
    Structs {
        struct_type: String,
        values: Vec<StructValue>,
    },
    /// A set whose elements are nested property sets.
    Properties(Vec<BTreeMap<String, Property>>),
}

impl Property {
    /// Look up a named child of a generic struct property.
    ///
    /// Returns `Some` only when `self` is a [`Property::Struct`] whose value is a
    /// nested property set ([`StructValue::Properties`]) containing `key`.
    pub fn get_child(&self, key: &str) -> Option<&Property> {
        match self {
            Property::Struct {
                value: StructValue::Properties(m),
                ..
            } => m.get(key),
            _ => None,
        }
    }

    /// True when [`get_child`](Property::get_child) would return `Some`.
    pub fn has_child(&self, key: &str) -> bool {
        self.get_child(key).is_some()
    }

    /// The nested property set of a generic struct, if this is one.
    pub fn as_properties(&self) -> Option<&BTreeMap<String, Property>> {
        match self {
            Property::Struct {
                value: StructValue::Properties(m),
                ..
            } => Some(m),
            _ => None,
        }
    }

    /// The raw bytes of a [`Property::Bytes`] node (skip-decoded blob or byte
    /// array), if this is one.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Property::Bytes(b) => Some(b),
            _ => None,
        }
    }
}
