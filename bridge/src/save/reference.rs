//! Bundled reference catalogs for item/skill/element display-name lookup.
//!
//! The client decodes inventory items and pal skills as raw Palworld
//! internal ids (e.g. `"Wood"`, `"EPalWazaID::AirBlade"`). To render those as
//! human-readable names we ship a small, read-only id -> display-name
//! catalog, vendored at build time via `include_str!` from `bridge/data/`.
//!
//! Data source: palworld-save-pal / palworld-save-tools. See
//! `bridge/data/ATTRIBUTION.md` for full attribution.

use std::collections::HashMap;

use serde::Deserialize;

const ITEMS_JSON: &str = include_str!("../../data/items.json");
const ACTIVE_SKILLS_JSON: &str = include_str!("../../data/active_skills.json");
const PASSIVE_SKILLS_JSON: &str = include_str!("../../data/passive_skills.json");
const ELEMENTS_JSON: &str = include_str!("../../data/elements.json");

/// One entry in a vendored catalog file (extra fields, e.g. `description`,
/// are ignored).
#[derive(Deserialize)]
struct CatalogEntry {
    localized_name: String,
}

/// Read-only lookup of Palworld internal ids to their English display name,
/// covering items, active (attack) skills, passive skills, and elements.
///
/// Build with [`load_reference`].
pub struct Reference {
    items: HashMap<String, String>,
    active_skills: HashMap<String, String>,
    passive_skills: HashMap<String, String>,
    elements: HashMap<String, String>,
}

impl Reference {
    /// Display name for an item's internal static id, e.g. `"Wood"`.
    pub fn item_name(&self, static_id: &str) -> Option<&str> {
        self.items.get(static_id).map(String::as_str)
    }

    /// Display name for an active (attack) skill id, e.g.
    /// `"EPalWazaID::AirBlade"`.
    pub fn active_skill_name(&self, id: &str) -> Option<&str> {
        self.active_skills.get(id).map(String::as_str)
    }

    /// Display name for a passive skill id, e.g. `"AirDash_1"`.
    pub fn passive_skill_name(&self, id: &str) -> Option<&str> {
        self.passive_skills.get(id).map(String::as_str)
    }

    /// Display name for an element id, e.g. `"Fire"`.
    pub fn element_name(&self, id: &str) -> Option<&str> {
        self.elements.get(id).map(String::as_str)
    }

    /// The full item catalog (internal id -> display name).
    pub fn items(&self) -> &HashMap<String, String> {
        &self.items
    }

    /// The full active (attack) skill catalog (internal id -> display name).
    pub fn active_skills(&self) -> &HashMap<String, String> {
        &self.active_skills
    }

    /// The full passive skill catalog (internal id -> display name).
    pub fn passive_skills(&self) -> &HashMap<String, String> {
        &self.passive_skills
    }

    /// The full element catalog (internal id -> display name).
    pub fn elements(&self) -> &HashMap<String, String> {
        &self.elements
    }
}

/// Parse a vendored catalog JSON blob (`{ id: { localized_name, ... }, ... }`)
/// into an id -> display-name map.
fn parse_catalog(json: &str) -> HashMap<String, String> {
    let raw: HashMap<String, CatalogEntry> =
        serde_json::from_str(json).expect("vendored reference JSON must be valid");
    raw.into_iter()
        .map(|(id, entry)| (id, entry.localized_name))
        .collect()
}

/// Load the bundled reference catalogs (items, active/passive skills,
/// elements) from the vendored JSON in `bridge/data/`.
pub fn load_reference() -> Reference {
    Reference {
        items: parse_catalog(ITEMS_JSON),
        active_skills: parse_catalog(ACTIVE_SKILLS_JSON),
        passive_skills: parse_catalog(PASSIVE_SKILLS_JSON),
        elements: parse_catalog(ELEMENTS_JSON),
    }
}
