//! G2 — one Janka table for the row and the query (extrapolation P2 step 3,
//! `planning/extrapolation_2026-09-24/P2_PLAN.md` §3, orchestrator
//! decision 3).
//!
//! Before this step the lookup had two Janka tables. The query read
//! `WoodSpecies`, `PlywoodGrade` and `SheetGoodKind`. A row with no per-row
//! hardness read `vendor_lookup::family_default_janka`, which had its own
//! values (softwood 500, hardwood 1290, MDF 700, plywood 550 / 1100). So a
//! printed row got a hardness scale on a query of its own family. A printed
//! MDF row read (700 / 1100)^0.5 = x0.80 on an MDF query and was flagged
//! "extrapolated" (extrapolation INVENTORY §2, item 2).
//!
//! The rules now:
//!
//! - a composite-board query (plywood, MDF, HDF, particleboard) gets no
//!   hardness scale. This applies to per-row values and to defaults. No
//!   source prints a Janka value for these boards; the only sourced figure is
//!   the particleboard minimum of 500 lbf (ANSI A208.1);
//! - a solid-wood row with no Janka reads the query table:
//!   `WoodSpecies::GenericSoftwood` (600) or `GenericHardwood` (1450).
//!
//! The sentry pins:
//!
//! - the MDF v7 6.35 mm pocket row (`amana-ball-mdf-pocket-6350-2f-v7`, no
//!   per-row Janka) reads its printed band 0.1524-0.2032 mm/tooth on an MDF
//!   query, with hardness scale 1.0, and it is not flagged extrapolated;
//! - HDF on an MDF row gives 1.0, for a row with no Janka and for a row with
//!   a per-row 1100;
//! - Baltic birch (1200) on the `onsrud-plywood-hardwood-60-100mw` rows (no
//!   Janka) and on an Onsrud 77-100 plywood row (per-row 1000) gives 1.0;
//! - a hardwood row with no Janka gives 1.0 on a `GenericHardwood` query. On
//!   a `GenericSoftwood` query it reads the raw ratio 1450 / 600 and the law
//!   `(1450 / 600)^0.5`, which the flat-end soft/hard cap stops at 1.43 (P2
//!   step 4);
//! - Ipe still derates below a `GenericHardwood` query on that row.
//!
//! Each arm except the first reads a one-row LUT built from the embedded
//! row, so the scorer cannot choose another row and the arm tests the
//! hardness law alone.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::feeds::EMBEDDED_LUT;
use rs_cam_core::feeds::extrapolation::{HardnessBasis, JankaFrom};
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, LookupResult, lookup_best};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
    VendorObservation,
};
use rs_cam_core::material::{PlywoodGrade, SheetGoodKind, WoodSpecies};

const TOL: f64 = 1e-12;

/// A hardwood row with no per-row Janka (Onsrud 60-100mw, 1/4 in, contour
/// finish, 0.3556-0.4064 mm/tooth).
const HARDWOOD_ROW_NO_JANKA: &str = "onsrud-hardwood-60-100mw-1_4-finish";

/// The embedded row with this id.
fn row(id: &str) -> VendorObservation {
    EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("the embedded LUT has no row {id}"))
        .clone()
}

/// The lookup on a LUT that holds only the row `id`, with a query at the
/// row's own tool, size, operation and pass role, in `family` at `janka`.
fn alone(id: &str, family: MaterialFamily, janka: f64) -> LookupResult {
    let obs = row(id);
    let query = LookupQuery {
        tool_family: obs.tool_family,
        tool_subfamily: obs.tool_subfamily.clone(),
        diameter_mm: obs.diameter_mm.expect("the row has a diameter"),
        flute_count: obs.flute_count,
        material_family: family,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(janka),
        operation_family: obs.operation_family,
        pass_role: obs.pass_role,
    };
    let lut = VendorLut {
        observations: vec![obs],
    };
    lookup_best(&lut, &query)
        .unwrap_or_else(|| panic!("the one-row LUT must answer its own row {id}"))
}

/// The MDF v7 6.35 mm pocket row reaches an MDF query unscaled. Before step
/// 3 it read (700 / 1100)^0.5 = 0.797724 (band 0.121573-0.162098) and the
/// raw ratio 0.636 set the extrapolation flag.
#[test]
fn the_mdf_v7_pocket_row_reads_its_printed_band_g2() {
    let query = LookupQuery {
        tool_family: ToolFamily::BallNose,
        tool_subfamily: None,
        diameter_mm: 6.35,
        flute_count: 2,
        material_family: MaterialFamily::Mdf,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(SheetGoodKind::Mdf.effective_janka_lbf()),
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
    };
    let r = lookup_best(&EMBEDDED_LUT, &query).expect("an MDF ball pocket row matches");
    assert_eq!(r.observation_id, "amana-ball-mdf-pocket-6350-2f-v7");
    assert_eq!(
        row(&r.observation_id).hardness_value,
        None,
        "premise: the row has no per-row Janka"
    );
    assert!((r.chipload_hardness_ratio_raw - 1.0).abs() < TOL, "{r:?}");
    assert!((r.chipload_hardness_scale - 1.0).abs() < TOL, "{r:?}");
    assert!((r.chipload_diameter_scale - 1.0).abs() < TOL, "{r:?}");
    assert!(
        !r.is_extrapolated,
        "a printed row on its own size and family"
    );
    let min = r.chip_load_min_mm.expect("the row prints a band");
    let max = r.chip_load_max_mm.expect("the row prints a band");
    assert!((min - 0.1524).abs() < TOL, "min {min}");
    assert!((max - 0.2032).abs() < TOL, "max {max}");
}

/// HDF (1300) on an MDF row: 1.0 on a row with no Janka (was
/// (700 / 1300)^0.5 = 0.7338) and on a row with a per-row 1100 (was
/// (1100 / 1300)^0.5 = 0.9199).
#[test]
fn hdf_on_an_mdf_row_has_no_hardness_scale_g2() {
    let hdf = SheetGoodKind::Hdf.effective_janka_lbf();
    for id in [
        "onsrud-mdf-60-100mw-1_4-finish",
        "amana-ball-mdf-pocket-9525-2f-v7",
    ] {
        let r = alone(id, MaterialFamily::Hdf, hdf);
        assert!(
            (r.chipload_hardness_ratio_raw - 1.0).abs() < TOL,
            "{id}: {r:?}"
        );
        assert!((r.chipload_hardness_scale - 1.0).abs() < TOL, "{id}: {r:?}");
    }
    assert_eq!(
        row("amana-ball-mdf-pocket-9525-2f-v7").hardness_value,
        Some(1100.0),
        "premise: the second row has a per-row Janka, so the rule beats it"
    );
}

/// Baltic birch (1200) on hard-plywood rows: 1.0 on the Onsrud 60-100mw
/// rows with no Janka (was (1100 / 1200)^0.5 = 0.9574) and on an Onsrud
/// 77-100 row with a per-row 1000 (was (1000 / 1200)^0.5 = 0.9129).
#[test]
fn baltic_birch_on_a_hard_plywood_row_has_no_hardness_scale_g2() {
    let birch = PlywoodGrade::BalticBirch.effective_janka_lbf();
    let no_janka: Vec<&str> = EMBEDDED_LUT
        .observations
        .iter()
        .filter(|o| {
            o.observation_id
                .starts_with("onsrud-plywood-hardwood-60-100mw")
        })
        .map(|o| o.observation_id.as_str())
        .collect();
    assert!(
        !no_janka.is_empty(),
        "premise: the Onsrud 60-100mw hard plywood rows are loaded"
    );
    for id in no_janka
        .into_iter()
        .chain(["onsrud-plywood-hardwood-77-100-1_4-parallel"])
    {
        let r = alone(id, MaterialFamily::PlywoodHardwood, birch);
        assert!(
            (r.chipload_hardness_ratio_raw - 1.0).abs() < TOL,
            "{id}: {r:?}"
        );
        assert!((r.chipload_hardness_scale - 1.0).abs() < TOL, "{id}: {r:?}");
    }
}

/// A hardwood row with no Janka reads `GenericHardwood` (1450). On a
/// `GenericHardwood` query the scale is 1.0 (was (1290 / 1450)^0.5 =
/// 0.9432).
///
/// On a `GenericSoftwood` query (600) the raw ratio is 1450 / 600 =
/// 2.416667, and the law gives `(1450 / 600)^0.5 = 1.554563175515` (step
/// 3 pinned this value; before step 3 it was (1290 / 600)^0.5 = 1.4663).
/// RE-PINNED 2026-09-24 (P2 step 4, the soft/hard cap): the row is a flat
/// end mill, and the largest softwood/hardwood ratio that flat-end charts
/// print is 1.43 (EXTRAPOLATION_G2 §1.2). So the applied scale is 1.43 and
/// the band is `0.3556 * 1.43 = 0.508508` to `0.4064 * 1.43 = 0.581152`.
/// The basis keeps the law value 1.554563175515. The raw ratio and the flag
/// stay.
#[test]
fn a_hardwood_row_with_no_janka_reads_the_query_table_g2() {
    assert_eq!(
        row(HARDWOOD_ROW_NO_JANKA).hardness_value,
        None,
        "premise: the row has no per-row Janka"
    );
    let hard = alone(
        HARDWOOD_ROW_NO_JANKA,
        MaterialFamily::Hardwood,
        WoodSpecies::GenericHardwood.janka_lbf(),
    );
    assert!(
        (hard.chipload_hardness_ratio_raw - 1.0).abs() < TOL,
        "{hard:?}"
    );
    assert!((hard.chipload_hardness_scale - 1.0).abs() < TOL, "{hard:?}");
    assert!(!hard.is_extrapolated);

    let soft = alone(
        HARDWOOD_ROW_NO_JANKA,
        MaterialFamily::Softwood,
        WoodSpecies::GenericSoftwood.janka_lbf(),
    );
    assert!(
        (soft.chipload_hardness_ratio_raw - 1450.0 / 600.0).abs() < TOL,
        "{soft:?}"
    );
    assert!(
        (soft.chipload_hardness_scale - 1.43).abs() < TOL,
        "{soft:?}"
    );
    let HardnessBasis::Capped {
        law_scale,
        from,
        cap,
        ..
    } = &soft.hardness_basis
    else {
        panic!("expected the flat-end cap: {:?}", soft.hardness_basis);
    };
    assert!((law_scale - 1.554_563_175_515).abs() < 1e-11, "{law_scale}");
    assert_eq!(*from, JankaFrom::FamilyGeneric);
    assert_eq!(cap.family, ToolFamily::FlatEnd);
    assert!(!cap.borrowed_from_flat);
    assert!((soft.chip_load_min_mm.unwrap() - 0.508_508).abs() < 1e-11);
    assert!((soft.chip_load_max_mm.unwrap() - 0.581_152).abs() < 1e-11);
    assert!(soft.is_extrapolated, "the raw ratio 2.42 is past 1.4");
}

/// Ipe (3510) on the same row derates below the `GenericHardwood` query:
/// `(1450 / 3510)^0.5 = 0.642732769590` (was (1290 / 3510)^0.5 = 0.6062).
#[test]
fn ipe_still_derates_below_generic_hardwood_g2() {
    let hard = alone(
        HARDWOOD_ROW_NO_JANKA,
        MaterialFamily::Hardwood,
        WoodSpecies::GenericHardwood.janka_lbf(),
    );
    let ipe = alone(
        HARDWOOD_ROW_NO_JANKA,
        MaterialFamily::Hardwood,
        WoodSpecies::Ipe.janka_lbf(),
    );
    assert!(
        (ipe.chipload_hardness_scale - 0.642_732_769_590).abs() < 1e-11,
        "{ipe:?}"
    );
    assert!(ipe.chipload_hardness_scale < hard.chipload_hardness_scale);
    if let (Some(i), Some(h)) = (ipe.chip_load_max_mm, hard.chip_load_max_mm) {
        assert!(i < h, "Ipe band max {i} must be under the hardwood {h}");
    }
}
