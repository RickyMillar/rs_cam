//! Wood species library provenance gate.
//!
//! Pins every `WOOD_SPECIES_LIBRARY` entry's `source_id` to an entry
//! in `crates/rs_cam_core/data/vendor_lut/source_manifest.json` so a
//! library row that cites a missing/typo'd source can't slip in
//! silently. Phase E (completion plan, 2026-05-31).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::path::PathBuf;

use rs_cam_core::material::wood_species_library::{WOOD_SPECIES_LIBRARY, find_by_display_name};

fn manifest_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("data/vendor_lut/source_manifest.json")
}

fn manifest_source_ids() -> HashSet<String> {
    let path = manifest_path();
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("source_manifest.json read failed at {path:?}: {e}"));
    let value: serde_json::Value = serde_json::from_str(&contents)
        .unwrap_or_else(|e| panic!("source_manifest.json parse failed: {e}"));
    value
        .get("sources")
        .and_then(|s| s.as_array())
        .expect("source_manifest.json missing top-level `sources` array")
        .iter()
        .filter_map(|entry| {
            entry
                .get("source_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn every_library_source_id_exists_in_manifest() {
    let manifest_ids = manifest_source_ids();
    let mut missing = Vec::new();
    for entry in WOOD_SPECIES_LIBRARY {
        if !manifest_ids.contains(entry.source_id) {
            missing.push((entry.display_name, entry.source_id));
        }
    }
    assert!(
        missing.is_empty(),
        "WOOD_SPECIES_LIBRARY entries reference source_ids not present in \
         source_manifest.json: {missing:?}. Add the missing manifest entries \
         or fix the typo in wood_species_library.rs."
    );
}

#[test]
fn library_has_substantial_coverage() {
    // Phase E shipped 132 unique species. A regression that accidentally
    // truncates the const (e.g. a partial regen) would slip past silently
    // without a coverage floor. The threshold is deliberately loose
    // (>=100) so a curated trim — e.g. dropping suspect Wood Database
    // entries — doesn't fire this test, but a structural breakage does.
    assert!(
        WOOD_SPECIES_LIBRARY.len() >= 100,
        "WOOD_SPECIES_LIBRARY shrank to {} entries (expected >=100). \
         Check phaseE_2026-05-31.md for the expected size.",
        WOOD_SPECIES_LIBRARY.len()
    );
}

#[test]
fn find_by_display_name_is_case_insensitive() {
    // Smoke test for the lookup helper.
    let upper = find_by_display_name("RED OAK (NORTHERN)");
    let lower = find_by_display_name("red oak (northern)");
    let mixed = find_by_display_name("Red Oak (Northern)");
    assert!(
        upper.is_some(),
        "uppercase variant of a known entry must match"
    );
    assert_eq!(upper, lower);
    assert_eq!(lower, mixed);
}

// ── Per-arm Kc provenance (EDG-01, 2026-09-17) ──────────────────────────
//
// `Material::kc_n_per_mm2` hands every solid-wood species the same
// `Some(value)` shape. Three of the ten arms are folklore, not a citation.
// `WoodSpecies::kc_provenance` is the only thing that says which, so these
// cases pin the tag per arm. They do not pin a Kc value.

use rs_cam_core::material::{KcProvenance, Material, WoodSpecies};

/// The three species absent from FPL Chapter 5.
const FOLKLORE_SPECIES: [WoodSpecies; 3] = [
    WoodSpecies::RadiataPine,
    WoodSpecies::Jarrah,
    WoodSpecies::Ipe,
];

#[test]
fn every_species_reports_a_kc_provenance() {
    assert_eq!(
        WoodSpecies::ALL.len(),
        10,
        "WoodSpecies::ALL must list every species; a new arm needs a \
         provenance tag too"
    );
    for species in WoodSpecies::ALL {
        let tag = species.kc_provenance();
        assert!(
            KcProvenance::ALL.contains(&tag),
            "{} reports a provenance outside KcProvenance::ALL",
            species.label()
        );
        assert!(
            Material::SolidWood { species }.kc_n_per_mm2().is_some(),
            "{} must still carry a Kc value; EDG-01 changes the tag, not \
             the number",
            species.label()
        );
    }
}

#[test]
fn only_the_three_uncited_species_report_folklore() {
    for species in WoodSpecies::ALL {
        let is_folklore = species.kc_provenance().is_folklore();
        let expected = FOLKLORE_SPECIES.contains(&species);
        assert_eq!(
            is_folklore,
            expected,
            "{} reports provenance {} — expected folklore: {expected}. \
             RadiataPine, Jarrah and Ipe have no FPL Ch.5 row; every other \
             species does. Source the value before you retag it.",
            species.label(),
            species.kc_provenance().label()
        );
    }
}

#[test]
fn a_generic_species_is_a_band_midpoint_not_a_row() {
    // The two generic stand-ins average several cited rows. Reporting them
    // as one FPL row would claim a citation that does not exist.
    assert_eq!(
        WoodSpecies::GenericSoftwood.kc_provenance(),
        KcProvenance::FplBandMidpoint
    );
    assert_eq!(
        WoodSpecies::GenericHardwood.kc_provenance(),
        KcProvenance::FplBandMidpoint
    );
    assert_eq!(
        WoodSpecies::WhiteOak.kc_provenance(),
        KcProvenance::FplTableRow,
        "White oak is the Quercus alba row of FPL Table 5-3a"
    );
}
