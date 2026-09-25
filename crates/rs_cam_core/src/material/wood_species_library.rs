//! Wood species library — Janka hardness lookup for the parametric
//! `Material::SolidWoodByJanka` variant. Surfaces species from the
//! FPL Wood Handbook Ch.5 (peer-reviewed, 2010) and The Wood
//! Database (cross-checked single-source), totalling ~130 species
//! beyond the 10 first-class `WoodSpecies` enum variants.
//!
//! The rows are **data**: `data/wood_species.toml` holds them, the
//! same way `posts/*.toml` holds the four post-processors and
//! `io/tool_library.rs` / `io/machine_library.rs` read the two tool and
//! machine catalogues. `include_str!` embeds the file at build time, so
//! a running binary needs no file on disk; adding a species is a data
//! edit and a re-pin of the library hash, not a hand-written Rust row.
//!
//! The file header carries the source, the dedup precedence and the
//! column meanings. `source_id` is the per-row provenance column.
//! `specific_gravity_12` is the FPL Table 5-3a SG at 12 % MC; the force
//! line of `Material::SolidWoodByJanka` reads it through [`fpl_density`].

use serde::Deserialize;
use std::sync::LazyLock;

use super::force_line::{DensitySource, WoodDensity};

/// The `source_id` of the FPL Wood Handbook Chapter 5 rows.
pub const FPL_CH5_SOURCE_ID: &str = "fpl_ch5_2010";

/// A single wood species entry in the library.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WoodSpeciesEntry {
    /// Human-readable common name as printed by the source.
    pub display_name: String,
    /// Binomial scientific name when known; `None` for species the
    /// staging source didn't pair with a binomial.
    #[serde(default)]
    pub scientific_name: Option<String>,
    /// Janka side-hardness in lbf at 12% MC. For FPL entries this is
    /// converted from the published Newton value (`lbf = N / 4.448`,
    /// rounded to 1 decimal); Wood Database entries are quoted in lbf
    /// directly.
    pub janka_lbf: f64,
    /// FPL Table 5-3a specific gravity at 12 % MC ("based on weight when
    /// ovendry and volume at 12% moisture content"). `None` on a Wood
    /// Database row and on an FPL row where the table prints "—".
    #[serde(default)]
    pub specific_gravity_12: Option<f64>,
    /// Citation key — matches an entry in
    /// `crates/rs_cam_core/data/vendor_lut/source_manifest.json`.
    pub source_id: String,
}

/// The shipped file's shape: one `[[species]]` table per row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeciesFile {
    species: Vec<WoodSpeciesEntry>,
}

/// The curated library, embedded at build time.
const WOOD_SPECIES_TOML: &str = include_str!("../../data/wood_species.toml");

/// Parsed once per process. Entries keep the published order of each
/// source (FPL first, Wood Database fill-ins second).
static WOOD_SPECIES: LazyLock<Vec<WoodSpeciesEntry>> = LazyLock::new(|| {
    // SAFETY: the shipped TOML is validated by the
    // `wood_species_library_provenance` tests, which parse it, pin its
    // row hash and check every `source_id`. A malformed shipped file
    // fails those tests before it reaches production, so parse-on-init
    // may panic here.
    #[allow(clippy::expect_used)]
    let file: SpeciesFile =
        toml::from_str(WOOD_SPECIES_TOML).expect("shipped wood_species.toml must parse");
    file.species
});

/// Curated wood-species library. Use [`find_by_display_name`] for
/// case-insensitive lookup.
pub fn wood_species_library() -> &'static [WoodSpeciesEntry] {
    &WOOD_SPECIES
}

/// Case-insensitive lookup by display name. Returns `None` when no
/// library entry matches.
pub fn find_by_display_name(name: &str) -> Option<&'static WoodSpeciesEntry> {
    let lower = name.to_lowercase();
    wood_species_library()
        .iter()
        .find(|e| e.display_name.to_lowercase() == lower)
}

/// The FPL density of the library row named `display_name` under
/// `source_id`, for the force line (ruling B6).
///
/// `None` when no row matches exactly, when the row is not an FPL row
/// ([`FPL_CH5_SOURCE_ID`]), or when the row carries no
/// `specific_gravity_12`. The caller then refuses with
/// `ForceLineRefusal::NoDensity`.
#[must_use]
pub fn fpl_density(display_name: &str, source_id: &str) -> Option<WoodDensity> {
    if source_id != FPL_CH5_SOURCE_ID {
        return None;
    }
    let entry = wood_species_library()
        .iter()
        .find(|e| e.source_id == source_id && e.display_name == display_name)?;
    let sg = entry.specific_gravity_12?;
    Some(WoodDensity::from_fpl_specific_gravity(
        sg,
        DensitySource::FplLibraryRow(entry.display_name.as_str()),
    ))
}
