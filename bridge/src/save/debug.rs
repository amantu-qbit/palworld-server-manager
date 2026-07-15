//! Read-only JSON projection of the generic GVAS property tree, for the desktop
//! "Raw Save" debug viewer.
//!
//! [`node_to_json`] turns a [`Property`] subtree into a `serde_json::Value` with
//! three properties that keep it safe and bounded for exploration:
//!
//! - **Byte blobs are summarized.** A [`Property::Bytes`] (Palworld's opaque
//!   `RawData` payloads and skip-decoded blobs) renders as `{"_bytes": <len>}` —
//!   the raw bytes are never copied into the output.
//! - **Big containers truncate.** Arrays / maps / sets longer than
//!   [`DumpOpts::page`] emit only the first `page` items plus `"_truncated"`.
//! - **Deep containers collapse.** Below [`DumpOpts::depth`] a container renders
//!   as `{"_type", "_count", "_collapsed": true}` so the viewer can lazily
//!   re-request that subtree by path via [`resolve`].
//!
//! [`resolve`] walks a dotted path (`worldSaveData.CharacterSaveParameterMap.0.value`)
//! so the viewer can drill in one node at a time: object/struct segments select a
//! child by key; array/map/set segments select an element by numeric index; a map
//! element then takes `key` / `value` to descend into the pair.

use std::collections::BTreeMap;

use serde_json::{json, Map, Number, Value};

use super::props::{ArrayValue, ByteVal, MapEntry, Property, SetValue, StructValue};

/// Bounds on the JSON projection so a huge tree (e.g. `Level.sav`) never
/// serializes in full.
#[derive(Debug, Clone, Copy)]
pub struct DumpOpts {
    /// Max children emitted for a map/array/set before truncating. `None` = all.
    pub page: Option<usize>,
    /// Levels of container nesting to expand below the requested node before
    /// collapsing deeper containers to a stub.
    pub depth: usize,
}

impl Default for DumpOpts {
    fn default() -> Self {
        DumpOpts { page: Some(200), depth: 2 }
    }
}

/// A position in the tree reachable by a dotted path. The tree mixes
/// [`Property`], nested property sets, [`StructValue`], and [`MapEntry`] pairs,
/// so a resolved node can be any of them.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// A property value.
    Prop(&'a Property),
    /// A property set: the GVAS root, or a struct/set serialized as named fields.
    Fields(&'a BTreeMap<String, Property>),
    /// A struct value (vector, guid, or a nested field set).
    Struct(&'a StructValue),
    /// One `key`/`value` pair of a map.
    Entry(&'a MapEntry),
}

/// Resolve a dotted `path` (empty = root) to a node, or `None` if it doesn't
/// exist. See the module docs for segment semantics.
pub fn resolve<'a>(root: &'a BTreeMap<String, Property>, path: &str) -> Option<Node<'a>> {
    let mut node = Node::Fields(root);
    if path.is_empty() {
        return Some(node);
    }
    for seg in path.split('.') {
        node = descend(node, seg)?;
    }
    Some(node)
}

fn descend<'a>(node: Node<'a>, seg: &str) -> Option<Node<'a>> {
    match node {
        Node::Fields(map) => map.get(seg).map(Node::Prop),
        Node::Entry(e) => match seg {
            "key" => Some(Node::Prop(&e.key)),
            "value" => Some(Node::Prop(&e.value)),
            _ => None,
        },
        Node::Struct(sv) => match sv {
            StructValue::Properties(map) => map.get(seg).map(Node::Prop),
            _ => None,
        },
        Node::Prop(p) => match p {
            Property::Struct { value, .. } => descend(Node::Struct(value), seg),
            Property::Map { entries, .. } => entries.get(seg.parse::<usize>().ok()?).map(Node::Entry),
            Property::Array { value: ArrayValue::Structs { values, .. }, .. } => {
                values.get(seg.parse::<usize>().ok()?).map(Node::Struct)
            }
            Property::Set { value, .. } => match value {
                SetValue::Structs { values, .. } => {
                    values.get(seg.parse::<usize>().ok()?).map(Node::Struct)
                }
                SetValue::Properties(sets) => {
                    sets.get(seg.parse::<usize>().ok()?).map(Node::Fields)
                }
            },
            _ => None,
        },
    }
}

/// Project a resolved node to JSON. The node is treated as level 0, so its own
/// children expand up to `opts.depth`.
pub fn node_to_json(node: &Node, opts: &DumpOpts) -> Value {
    match node {
        Node::Prop(p) => prop_json(p, opts, 0),
        Node::Fields(map) => fields_json("StructProperty", map, opts, 0),
        Node::Struct(sv) => struct_json(None, sv, opts, 0),
        Node::Entry(e) => json!({
            "_type": "MapEntry",
            "key": prop_json(&e.key, opts, 1),
            "value": prop_json(&e.value, opts, 1),
        }),
    }
}

fn num(f: f64) -> Value {
    // serde_json cannot represent NaN/Inf (Palworld saves allow them); null out.
    Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null)
}

fn prop_json(p: &Property, opts: &DumpOpts, level: usize) -> Value {
    match p {
        Property::Int(i) => json!(i),
        Property::Float(f) => num(*f),
        Property::Str(s) | Property::Name(s) => json!(s),
        Property::Bool(b) => json!(b),
        Property::Enum { enum_type, value } => json!({ "_type": "Enum", "enum": enum_type, "value": value }),
        Property::Byte { enum_type, value } => match value {
            ByteVal::Byte(b) => json!(b),
            ByteVal::Label(l) => json!({ "_type": "Byte", "enum": enum_type, "value": l }),
        },
        Property::Bytes(v) => json!({ "_bytes": v.len() }),
        Property::Struct { struct_type, value } => struct_json(Some(struct_type), value, opts, level),
        Property::Array { array_type, value } => {
            let (count, items) = array_items(value, opts, level);
            container(&format!("Array<{array_type}>"), count, items)
        }
        Property::Set { set_type, value } => {
            let (count, items) = set_items(value, opts, level);
            container(&format!("Set<{set_type}>"), count, items)
        }
        Property::Map { key_type, value_type, entries } => {
            let count = entries.len();
            let items = if level >= opts.depth {
                None
            } else {
                let take = opts.page.unwrap_or(usize::MAX);
                Some(
                    entries
                        .iter()
                        .take(take)
                        .map(|e| {
                            json!({
                                "key": prop_json(&e.key, opts, level + 1),
                                "value": prop_json(&e.value, opts, level + 1),
                            })
                        })
                        .collect::<Vec<_>>(),
                )
            };
            container(&format!("Map<{key_type},{value_type}>"), count, items)
        }
    }
}

fn struct_json(type_name: Option<&str>, sv: &StructValue, opts: &DumpOpts, level: usize) -> Value {
    match sv {
        StructValue::Properties(map) => {
            let t = match type_name {
                Some(t) => format!("Struct<{t}>"),
                None => "Struct".to_string(),
            };
            fields_json(&t, map, opts, level)
        }
        StructValue::Vector { x, y, z } => json!({ "_type": "Vector", "x": num(*x), "y": num(*y), "z": num(*z) }),
        StructValue::Quat { x, y, z, w } => {
            json!({ "_type": "Quat", "x": num(*x), "y": num(*y), "z": num(*z), "w": num(*w) })
        }
        StructValue::LinearColor { r, g, b, a } => {
            json!({ "_type": "LinearColor", "r": r, "g": g, "b": b, "a": a })
        }
        StructValue::Color { b, g, r, a } => json!({ "_type": "Color", "r": r, "g": g, "b": b, "a": a }),
        StructValue::DateTime(t) => json!({ "_type": "DateTime", "ticks": t }),
        StructValue::Guid(u) => json!(u.to_string()),
    }
}

fn fields_json(type_name: &str, map: &BTreeMap<String, Property>, opts: &DumpOpts, level: usize) -> Value {
    if level >= opts.depth {
        return json!({ "_type": type_name, "_count": map.len(), "_collapsed": true });
    }
    let mut obj = Map::new();
    obj.insert("_type".into(), json!(type_name));
    for (k, v) in map {
        obj.insert(k.clone(), prop_json(v, opts, level + 1));
    }
    Value::Object(obj)
}

/// Build the `{_type,_count,items|_collapsed,_truncated}` envelope shared by
/// arrays, sets, and maps. `items` is `None` when the caller collapsed.
fn container(type_name: &str, count: usize, items: Option<Vec<Value>>) -> Value {
    let mut obj = Map::new();
    obj.insert("_type".into(), json!(type_name));
    obj.insert("_count".into(), json!(count));
    match items {
        None => {
            obj.insert("_collapsed".into(), json!(true));
        }
        Some(items) => {
            if items.len() < count {
                obj.insert("_truncated".into(), json!(true));
            }
            obj.insert("items".into(), Value::Array(items));
        }
    }
    Value::Object(obj)
}

fn array_items(value: &ArrayValue, opts: &DumpOpts, level: usize) -> (usize, Option<Vec<Value>>) {
    if level >= opts.depth {
        let count = match value {
            ArrayValue::Structs { values, .. } => values.len(),
            ArrayValue::Names(v) => v.len(),
            ArrayValue::Guids(v) => v.len(),
        };
        return (count, None);
    }
    let take = opts.page.unwrap_or(usize::MAX);
    match value {
        ArrayValue::Structs { values, type_name, .. } => (
            values.len(),
            Some(values.iter().take(take).map(|s| struct_json(Some(type_name), s, opts, level + 1)).collect()),
        ),
        ArrayValue::Names(v) => (v.len(), Some(v.iter().take(take).map(|s| json!(s)).collect())),
        ArrayValue::Guids(v) => (v.len(), Some(v.iter().take(take).map(|u| json!(u.to_string())).collect())),
    }
}

fn set_items(value: &SetValue, opts: &DumpOpts, level: usize) -> (usize, Option<Vec<Value>>) {
    if level >= opts.depth {
        let count = match value {
            SetValue::Structs { values, .. } => values.len(),
            SetValue::Properties(v) => v.len(),
        };
        return (count, None);
    }
    let take = opts.page.unwrap_or(usize::MAX);
    match value {
        SetValue::Structs { values, struct_type } => (
            values.len(),
            Some(values.iter().take(take).map(|s| struct_json(Some(struct_type), s, opts, level + 1)).collect()),
        ),
        SetValue::Properties(sets) => (
            sets.len(),
            Some(sets.iter().take(take).map(|m| fields_json("Struct", m, opts, level + 1)).collect()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn props(pairs: Vec<(&str, Property)>) -> BTreeMap<String, Property> {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn scalars_and_bytes() {
        let root = props(vec![
            ("Level", Property::Int(65)),
            ("Name", Property::Name("Hero".into())),
            ("Blob", Property::Bytes(vec![0u8; 4096])),
        ]);
        let v = node_to_json(&resolve(&root, "").unwrap(), &DumpOpts::default());
        assert_eq!(v["Level"], json!(65));
        assert_eq!(v["Name"], json!("Hero"));
        // Bytes summarized, never dumped.
        assert_eq!(v["Blob"], json!({ "_bytes": 4096 }));
    }

    #[test]
    fn array_truncates_and_reports_count() {
        let names: Vec<String> = (0..500).map(|i| format!("T_{i}")).collect();
        let root = props(vec![(
            "Techs",
            Property::Array { array_type: "NameProperty".into(), value: ArrayValue::Names(names) },
        )]);
        let opts = DumpOpts { page: Some(10), depth: 3 };
        let v = node_to_json(&resolve(&root, "").unwrap(), &opts);
        assert_eq!(v["Techs"]["_count"], json!(500));
        assert_eq!(v["Techs"]["_truncated"], json!(true));
        assert_eq!(v["Techs"]["items"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn deep_container_collapses() {
        let inner = props(vec![("Deep", Property::Int(1))]);
        let mid = props(vec![(
            "Mid",
            Property::Struct { struct_type: "S".into(), value: StructValue::Properties(inner) },
        )]);
        let root = props(vec![(
            "Top",
            Property::Struct { struct_type: "S".into(), value: StructValue::Properties(mid) },
        )]);
        let opts = DumpOpts { page: None, depth: 1 };
        let v = node_to_json(&resolve(&root, "").unwrap(), &opts);
        // depth 1: root fields expand once, the nested struct collapses.
        assert_eq!(v["Top"]["_collapsed"], json!(true));
    }

    #[test]
    fn resolve_drills_into_map_entry_value() {
        let entry = MapEntry {
            key: Property::Int(7),
            value: Property::Struct {
                struct_type: "Slot".into(),
                value: StructValue::Properties(props(vec![("Count", Property::Int(42))])),
            },
        };
        let root = props(vec![(
            "Bag",
            Property::Map { key_type: "IntProperty".into(), value_type: "StructProperty".into(), entries: vec![entry] },
        )]);
        // Drill: Bag -> entry 0 -> value -> Count.
        let node = resolve(&root, "Bag.0.value").unwrap();
        let v = node_to_json(&node, &DumpOpts::default());
        assert_eq!(v["Count"], json!(42));
        assert!(resolve(&root, "Bag.9.value").is_none());
        assert!(resolve(&root, "Nope").is_none());
    }

    #[test]
    fn guid_and_vector_render() {
        let g = Uuid::nil();
        let root = props(vec![
            ("Id", Property::Struct { struct_type: "Guid".into(), value: StructValue::Guid(g) }),
            (
                "Pos",
                Property::Struct { struct_type: "Vector".into(), value: StructValue::Vector { x: 1.0, y: 2.0, z: 3.0 } },
            ),
        ]);
        let v = node_to_json(&resolve(&root, "").unwrap(), &DumpOpts::default());
        assert_eq!(v["Id"], json!(g.to_string()));
        assert_eq!(v["Pos"]["_type"], json!("Vector"));
        assert_eq!(v["Pos"]["y"], json!(2.0));
    }
}
