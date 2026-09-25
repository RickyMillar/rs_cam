//! Wood species library provenance gate.
//!
//! Pins every `wood_species_library()` entry's `source_id` to an entry
//! in `crates/rs_cam_core/data/vendor_lut/source_manifest.json` so a
//! library row that cites a missing/typo'd source can't slip in
//! silently. Phase E (completion plan, 2026-05-31).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::path::PathBuf;

use rs_cam_core::material::wood_species_library::{find_by_display_name, wood_species_library};

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
    for entry in wood_species_library() {
        if !manifest_ids.contains(entry.source_id.as_str()) {
            missing.push((&entry.display_name, &entry.source_id));
        }
    }
    assert!(
        missing.is_empty(),
        "wood_species_library() entries reference source_ids not present in \
         source_manifest.json: {missing:?}. Add the missing manifest entries \
         or fix the typo in data/wood_species.toml."
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
        wood_species_library().len() >= 100,
        "wood_species_library() shrank to {} entries (expected >=100). \
         Check phaseE_2026-05-31.md for the expected size.",
        wood_species_library().len()
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

// ── Per-arm density provenance (ruling B6, 2026-09-25) ──────────────────
//
// Ruling B6 replaced the scalar `Kc` and its folklore tags
// (`KcProvenance`, EDG-01) with one force line per material. A solid-wood
// line reads a density, so the provenance question is now: which FPL row
// gives the density. `WoodSpecies::fpl_density` answers it per arm, and
// these cases pin the source kind per arm. The density values are pinned
// in `the_force_line_is_printed_per_family_b6.rs`.

use rs_cam_core::material::force_line::{DensitySource, ForceLineRefusal};
use rs_cam_core::material::{Material, WoodSpecies};

/// The three species with no FPL Table 5-3a row. They read the Table 5-5a
/// basic SG, converted to 12 % MC by FPL Ch.4 Eq. (4-11).
const TABLE_5_5A_SPECIES: [WoodSpecies; 3] = [
    WoodSpecies::RadiataPine,
    WoodSpecies::Jarrah,
    WoodSpecies::Ipe,
];

#[test]
fn every_species_reports_a_density_source() {
    assert_eq!(
        WoodSpecies::ALL.len(),
        10,
        "WoodSpecies::ALL must list every species; a new arm needs a \
         density row or a refusal too"
    );
    for species in WoodSpecies::ALL {
        let density = species
            .fpl_density()
            .unwrap_or_else(|| panic!("{} must carry an FPL density", species.label()));
        assert!(
            density.rho_kg_m3().is_finite() && density.rho_kg_m3() > 0.0,
            "{} reports a density that is not a positive number",
            species.label()
        );
        assert!(
            !density.source().row_names().is_empty(),
            "{} must name the FPL row(s) its density comes from",
            species.label()
        );
    }
}

#[test]
fn only_the_three_table_5_3a_absent_species_read_table_5_5a() {
    for species in WoodSpecies::ALL {
        let density = species.fpl_density().expect("asserted in the arm above");
        let reads_5_5a = matches!(density.source(), DensitySource::FplBasicRow(_));
        let expected = TABLE_5_5A_SPECIES.contains(&species);
        assert_eq!(
            reads_5_5a,
            expected,
            "{} reads density source {} — expected a Table 5-5a row: {expected}. \
             RadiataPine, Jarrah and Ipe have no FPL Table 5-3a row; every other \
             species does.",
            species.label(),
            density.source().kind_id()
        );
        if reads_5_5a {
            assert!(
                density.basic_specific_gravity().is_some(),
                "{} must keep the printed basic SG next to the converted G12",
                species.label()
            );
        }
    }
    // Ipe's converted density (1207 kg/m³) is above the Curti range, so the
    // row exists and the force line still refuses.
    assert!(matches!(
        Material::SolidWood {
            species: WoodSpecies::Ipe
        }
        .force_line(),
        Err(ForceLineRefusal::DensityOutOfRange { .. })
    ));
}

#[test]
fn a_generic_species_is_a_mean_of_rows_not_a_row() {
    // The two generic stand-ins average the rows their old `Kc` comment
    // named. Reporting them as one FPL row would claim a citation that
    // does not exist.
    for species in [WoodSpecies::GenericSoftwood, WoodSpecies::GenericHardwood] {
        let source = species.fpl_density().expect("generic species").source();
        assert!(
            matches!(source, DensitySource::FplMean(rows) if rows.len() == 3),
            "{} must be the mean of three FPL rows, got {}",
            species.label(),
            source.kind_id()
        );
    }
    let white_oak = WoodSpecies::WhiteOak
        .fpl_density()
        .expect("white oak")
        .source();
    assert!(
        matches!(white_oak, DensitySource::FplRow(row) if row.row == "Oak, white"),
        "White oak is the Quercus alba row of FPL Table 5-3a, got {}",
        white_oak.row_names()
    );
}

#[test]
fn only_an_fpl_row_carries_a_specific_gravity() {
    // `specific_gravity_12` is the FPL Table 5-3a SG. A Wood Database row
    // carries Janka only, so it must not carry an SG, and its force line
    // refuses. Every FPL row that carries an SG has a force line.
    let mut fpl_with_sg = 0_usize;
    let mut refused = 0_usize;
    for entry in wood_species_library() {
        let material = Material::SolidWoodByJanka {
            janka_lbf: entry.janka_lbf,
            label: entry.display_name.clone(),
            source_id: entry.source_id.clone(),
        };
        match entry.specific_gravity_12 {
            Some(sg) => {
                assert_eq!(
                    entry.source_id, "fpl_ch5_2010",
                    "{} carries an SG but is not an FPL row",
                    entry.display_name
                );
                assert!(
                    material.force_line().is_ok(),
                    "{} (FPL SG {sg}) must have a force line",
                    entry.display_name
                );
                fpl_with_sg += 1;
            }
            None => {
                assert_eq!(
                    material.force_line(),
                    Err(ForceLineRefusal::NoDensity),
                    "{} carries no SG, so its force line must refuse",
                    entry.display_name
                );
                refused += 1;
            }
        }
    }
    assert!(
        fpl_with_sg > 0 && refused > 0,
        "the population must hold both kinds: {fpl_with_sg} FPL rows with an SG, \
         {refused} rows without"
    );
}
