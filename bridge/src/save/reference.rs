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

// --- Species stat catalog (heal support) ------------------------------------

const PAL_STATS_JSON: &str = include_str!("../../data/pal_stats.json");

/// Per-species stat inputs for the max-HP / max-stomach computations
/// (`scripts/gen-pal-stats.mjs`, sourced from palworld-save-pal `pals.json`).
#[derive(serde::Deserialize, Clone, Copy, Debug, Default)]
pub struct PalStats {
    /// The species' HP scaling stat (`scaling.hp`).
    #[serde(default)]
    pub hp: f64,
    /// `max_full_stomach`.
    #[serde(default)]
    pub stomach: f64,
}

/// Case-insensitive species-stats lookup, keyed by lowercased code name.
/// Save files spell `character_id` differently from the catalog (e.g.
/// `SheepBall` vs `Sheepball`), and boss/lucky pals carry a `BOSS_` prefix.
pub struct PalStatsCatalog {
    by_lower: HashMap<String, PalStats>,
}

impl PalStatsCatalog {
    /// Stats for a raw save-file `character_id` (handles `BOSS_` prefixes and
    /// case differences). `None` for humans and unknown species.
    pub fn for_character_id(&self, character_id: &str) -> Option<PalStats> {
        let lower = character_id.to_ascii_lowercase();
        let key = lower.strip_prefix("boss_").unwrap_or(&lower);
        self.by_lower.get(key).copied()
    }
}

/// Parse the vendored species-stats catalog (panics on invalid vendored
/// JSON, same contract as [`load_reference`]).
pub fn load_pal_stats() -> PalStatsCatalog {
    let raw: HashMap<String, PalStats> =
        serde_json::from_str(PAL_STATS_JSON).expect("vendored pal_stats.json is valid JSON");
    PalStatsCatalog {
        by_lower: raw
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect(),
    }
}

/// Max HP for a pal, ported from palworld-save-pal `pal.py::max_hp`:
/// `floor(500 + 5·level + hp_scaling·0.5·level·(1 + IV·0.003)·alpha)`
/// `· (1 + 0.05·(rank−1)) · (1 + 0.03·rank_hp) · 1000`
/// where alpha is 1.2 for boss/lucky pals. Returns the on-disk fixed-point
/// value (×1000).
#[allow(clippy::too_many_arguments)]
pub fn max_hp(
    stats: PalStats,
    level: i32,
    talent_hp: i32,
    rank: i32,
    rank_hp: i32,
    alpha: bool,
) -> i64 {
    let alpha_scaling = if alpha { 1.2 } else { 1.0 };
    let condenser_bonus = ((rank - 1).max(0) as f64) * 0.05;
    let hp_iv = (talent_hp as f64) * 0.3 / 100.0;
    let hp = (500.0
        + 5.0 * level as f64
        + stats.hp * 0.5 * level as f64 * (1.0 + hp_iv) * alpha_scaling)
        .floor();
    ((hp * (1.0 + condenser_bonus) * (1.0 + hp_soul_bonus(rank_hp))).floor() as i64) * 1000
}

fn hp_soul_bonus(rank_hp: i32) -> f64 {
    1.0f64.mul_add(0.0, (rank_hp as f64) * 0.03)
}
