//! Literature cell schema (serde deserializer).
//!
//! Mirrors the TOML format defined in
//! `planning/feeds_literature_matrix_2026-06-03.md`. Phase 0 covers what the
//! one normal-use cell needs; the schema is intentionally permissive (most
//! fields optional) so Phase 1 cells slot in without changes.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct CellsFile {
    #[serde(rename = "cell", default)]
    pub cells: Vec<LiteratureCell>,
}

#[derive(Debug, Deserialize)]
pub struct LiteratureCell {
    pub id: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub profile_tag: Option<String>,

    pub inputs: CellInputs,

    #[serde(default)]
    pub fixed_inputs: Option<FixedInputs>,

    #[serde(default)]
    pub expected: ExpectedBands,

    #[serde(default)]
    pub expected_behaviour: Option<ExpectedBehaviour>,

    #[serde(default)]
    pub invariants: Vec<Invariant>,

    #[serde(default)]
    pub anti_patterns: Vec<AntiPattern>,
}

#[derive(Debug, Deserialize)]
pub struct CellInputs {
    pub tool_class: String,
    pub diameter_mm: f64,
    pub flute_count: u32,
    #[serde(default)]
    pub flute_length_mm: Option<f64>,
    #[serde(default)]
    pub stickout_mm: Option<f64>,
    #[serde(default)]
    pub corner_radius_mm: Option<f64>,
    #[serde(default)]
    pub included_angle_deg: Option<f64>,
    #[serde(default)]
    pub tip_diameter_mm: Option<f64>,
    #[serde(default)]
    pub tip_radius_mm: Option<f64>,
    #[serde(default)]
    pub taper_half_angle_deg: Option<f64>,
    pub operation: String,
    pub material: String,
    #[serde(default)]
    pub material_class: Option<String>,
    #[serde(default)]
    pub janka_lbf: Option<f64>,
    #[serde(default)]
    pub machine_class: Option<String>,
    /// Target scallop cusp height (mm) for scallop / drop-cutter cells.
    /// When set, the shim plumbs this through to `ScallopConfig` /
    /// `DropCutterConfig` so the Suggest path derives stepover via the
    /// chord-height formula instead of the legacy `ae_factor × D`.
    #[serde(default)]
    pub scallop_height_mm: Option<f64>,
    /// Maximum cut depth (mm) for v-carve cells. Feeds the engaged-D
    /// calculation in the V-bit feeds path (engaged D = 2 × max_depth ×
    /// tan(included_angle/2)).
    #[serde(default)]
    pub max_depth_mm: Option<f64>,
    /// Total drill-through depth (mm) for drill cells. When absent the
    /// drill shim falls back to a `5 × diameter` heuristic so drill
    /// cycles aren't accidentally shallow.
    #[serde(default)]
    pub drill_depth_mm: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FixedInputs {
    /// Either a number (mm) or the string "free".
    #[serde(default)]
    pub woc_mm: Option<toml::Value>,
    #[serde(default)]
    pub doc_mm: Option<toml::Value>,
}

impl FixedInputs {
    pub fn woc_pinned_mm(&self) -> Option<f64> {
        self.woc_mm.as_ref().and_then(toml_number)
    }
    pub fn doc_pinned_mm(&self) -> Option<f64> {
        self.doc_mm.as_ref().and_then(toml_number)
    }
}

fn toml_number(v: &toml::Value) -> Option<f64> {
    match v {
        toml::Value::Integer(i) => Some(*i as f64),
        toml::Value::Float(f) => Some(*f),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct ExpectedBands {
    #[serde(default)]
    pub rpm: Option<Band>,
    #[serde(default)]
    pub feed_per_tooth: Option<Band>,
    #[serde(default)]
    pub axial_doc: Option<Band>,
    #[serde(default)]
    pub radial_woc: Option<Band>,
    #[serde(default)]
    pub plunge_feed: Option<Band>,
    /// Catch-all for additional param bands future cells may add.
    #[serde(flatten, default)]
    pub extras: BTreeMap<String, Band>,
}

/// A single output-parameter band. `mode` selects which fields are read:
///   - `band`     : min + max (both required)
///   - `ceiling`  : max (min defaults to 0)
///   - `floor`    : min
///   - `fraction` : min_fraction_of_feed + max_fraction_of_feed (used by
///     plunge_feed; band-checked against plunge / feed)
///   - `na`       : skip evaluation entirely
#[derive(Debug, Deserialize, Default)]
pub struct Band {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub hobby_derate: Option<f64>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub min_fraction_of_feed: Option<f64>,
    #[serde(default)]
    pub max_fraction_of_feed: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExpectedBehaviour {
    /// "values" | "unadvised" | "unusable" | "refuse"
    pub mode: String,
    #[serde(default)]
    pub expected_derate: Option<f64>,
    #[serde(default)]
    pub expected_warning_pattern: Option<String>,
    #[serde(default)]
    pub expected_refuse_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Invariant {
    pub name: String,
    /// Closed-form expression for arithmetic/inequality invariants.
    /// Absent for `type = "convex_hull"`.
    #[serde(default)]
    pub expr: Option<String>,
    /// Type discriminator. Defaults to "expr" (arithmetic with
    /// optional floor/ceiling). Other supported value: "convex_hull".
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub floor: Option<f64>,
    #[serde(default)]
    pub ceiling: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default = "default_severity")]
    pub severity_on_fail: String,

    // convex_hull-specific fields
    #[serde(default)]
    pub vars: Vec<String>,
    #[serde(default)]
    pub vertices: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct AntiPattern {
    pub name: String,
    pub expr: String,
    #[serde(default = "default_severity")]
    pub severity: String,
}

fn default_severity() -> String {
    "moderate".to_owned()
}
