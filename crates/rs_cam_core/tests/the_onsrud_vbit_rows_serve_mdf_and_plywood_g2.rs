//! G2 — the Onsrud 37-series V-bit rows serve MDF and plywood (extrapolation
//! P2 step 1, `planning/extrapolation_2026-09-24/P2_PLAN.md`, orchestrator
//! decision 1).
//!
//! `data/vendor_lut/observations/onsrud_vbit_37.json` holds 60 printed rows:
//! the 37-series of the five Onsrud wood sheets and of the Laminated
//! Chipboard table, 10 rows per table. Two sets of printed cells are parked
//! and not loaded:
//!
//! - the 11 laminated plywood rows. Mapped to `plywood_hardwood` they tie
//!   with the hard plywood rows and win on id;
//! - the 11 37-80 rows at 1 1/4 in and 2 in. They have no angle, and a row
//!   with no angle passes the V-bit angle gate for any angle.
//!
//! The sheets print one row for 37-00/37-20 under the 1/4 in shank column.
//! The LUT holds it as two rows, 60 and 30 deg (PCT-19 page 20), with no
//! diameter.
//!
//! The sentry pins:
//!
//! - a 60 deg 2-flute V-bit on a trace pass in MDF and in plywood gets the
//!   Onsrud 37-80 1 in row of the MDF sheet and of the hard plywood sheet
//!   (not a laminated row), at both matrix sizes;
//! - a 150 deg V-bit gets no row in any wood family, and every wood V-bit
//!   row carries an angle;
//! - a 1-flute 60 deg V-bit in MDF gets the 37-00 row, unscaled;
//! - Suggest ships a 60 deg V-bit Trace in MDF from the 37-80 row.
//!
//! How the numbers were derived (python3 over the JSON files, with the
//! scorer of `vendor_lookup.rs`): at 60 deg, 6.35 mm, 2 flutes, the 37-80
//! 1 in row scores 1905 in MDF and the 37-00 row 1855 (flute term 80 against
//! 30); in Baltic birch (1200 lbf) the hard plywood rows score 1889 and 1839.
//! The 37-80 band is 0.1016-0.1524 mm at 25.4 mm; the V-bit size basis is
//! `VBitExempt`, so the generic law gives (6.35 / 25.4)^0.61 =
//! 0.429282718219 and a band of 0.043615124171-0.065422686257 mm in MDF
//! (1100 lbf on a 1100 lbf row: hardness scale 1.0).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::SizeBasis;
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, LookupResult, find_best_vbit_row};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsSupport, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, SheetGoodKind};

/// `(6.35 / 25.4)^0.61`: the generic size law from the 1 in row to 1/4 in.
const SCALE_1_IN_TO_1_4_IN: f64 = 0.429_282_718_219;

fn trace_query(diameter_mm: f64, flutes: u32, material: MaterialFamily, janka: f64) -> LookupQuery {
    LookupQuery {
        tool_family: ToolFamily::ChamferVbit,
        tool_subfamily: None,
        diameter_mm,
        flute_count: flutes,
        material_family: material,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(janka),
        operation_family: LutOperationFamily::Trace,
        pass_role: LutPassRole::Finish,
    }
}

fn row_at(query: &LookupQuery, angle_deg: f64) -> LookupResult {
    find_best_vbit_row(&EMBEDDED_LUT, query, Some(angle_deg)).unwrap_or_else(|| {
        panic!(
            "a {angle_deg} deg V-bit in {:?} at {} mm must get a row",
            query.material_family, query.diameter_mm
        )
    })
}

/// The source id of a LUT row.
fn source_of(observation_id: &str) -> &str {
    EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == observation_id)
        .map(|o| o.source_id.as_str())
        .expect("the matched row is in the LUT")
}

/// The MDF and plywood trace rows: the 37-80 1 in row of the MDF sheet and
/// of the hard plywood sheet, at both matrix sizes (6.35 and 12.7 mm).
#[test]
fn the_mdf_and_plywood_trace_rows_are_the_onsrud_sheet_rows_g2() {
    // MDF 1100 lbf (SheetGoodKind::Mdf); Baltic birch 1200 lbf and
    // hardwood-faced 1000 lbf (PlywoodGrade), both plywood_hardwood.
    for d in [6.35, 12.7] {
        let mdf = row_at(&trace_query(d, 2, MaterialFamily::Mdf, 1100.0), 60.0);
        assert_eq!(mdf.observation_id, "onsrud-mdf-37-80-1-trace");
        assert_eq!(source_of(&mdf.observation_id), "onsrud_mdf_cutting_data");
        for janka in [1200.0, 1000.0] {
            let ply = row_at(
                &trace_query(d, 2, MaterialFamily::PlywoodHardwood, janka),
                60.0,
            );
            assert_eq!(
                ply.observation_id, "onsrud-plywood-hardwood-37-80-1-trace",
                "the plywood row is the hard plywood sheet row, not a laminated row"
            );
            assert_eq!(
                source_of(&ply.observation_id),
                "onsrud_hard_plywood_cutting_data"
            );
        }
    }

    // The MDF band at 1/4 in: the 1 in row carried by the generic law,
    // hardness scale 1.0, flagged extrapolated (raw ratio 0.25).
    let mdf = row_at(&trace_query(6.35, 2, MaterialFamily::Mdf, 1100.0), 60.0);
    assert!(matches!(mdf.size_basis, SizeBasis::VBitExempt));
    assert!((mdf.chipload_diameter_scale - SCALE_1_IN_TO_1_4_IN).abs() < 1e-12);
    assert!((mdf.chipload_hardness_scale - 1.0).abs() < 1e-15);
    assert!(mdf.is_extrapolated);
    let min = mdf.chip_load_min_mm.expect("the row prints a band");
    let max = mdf.chip_load_max_mm.expect("the row prints a band");
    assert!((min - 0.043_615_124_171).abs() < 1e-12, "min {min}");
    assert!((max - 0.065_422_686_257).abs() < 1e-12, "max {max}");
}

/// A 150 deg V-bit gets no row in any wood family: every wood V-bit row
/// carries an angle, and the nearest angle is 120 deg (30 deg away).
#[test]
fn a_150_degree_vbit_gets_no_row_g2() {
    let wood = [
        MaterialFamily::Softwood,
        MaterialFamily::Hardwood,
        MaterialFamily::PlywoodSoftwood,
        MaterialFamily::PlywoodHardwood,
        MaterialFamily::Mdf,
        MaterialFamily::Hdf,
        MaterialFamily::Particleboard,
    ];
    for obs in &EMBEDDED_LUT.observations {
        if obs.tool_family == ToolFamily::ChamferVbit && wood.contains(&obs.material_family) {
            assert!(
                obs.included_angle_deg.is_some(),
                "{}: a wood V-bit row with no angle passes the angle gate for any angle",
                obs.observation_id
            );
        }
    }
    for material in wood {
        for d in [3.175, 6.35, 12.7, 25.4, 50.8] {
            for flutes in [1, 2] {
                let query = trace_query(d, flutes, material, 1000.0);
                let hit = find_best_vbit_row(&EMBEDDED_LUT, &query, Some(150.0));
                assert!(
                    hit.is_none(),
                    "a 150 deg V-bit in {material:?} at {d} mm, {flutes} flutes got {:?}",
                    hit.map(|r| r.observation_id)
                );
            }
        }
    }
}

/// A 1-flute 60 deg V-bit in MDF gets the 37-00 row, unscaled: the row has
/// no diameter, and the query hardness equals the row's (1100 lbf). A
/// 1-flute 30 deg V-bit gets the 37-20 row.
#[test]
fn a_one_flute_60_degree_vbit_gets_the_37_00_row_unscaled_g2() {
    let row = row_at(&trace_query(6.35, 1, MaterialFamily::Mdf, 1100.0), 60.0);
    assert_eq!(row.observation_id, "onsrud-mdf-37-00-trace");
    assert!(
        row.row_diameter_mm < f64::EPSILON,
        "the 37-00 row has no diameter"
    );
    assert!((row.chipload_diameter_scale - 1.0).abs() < 1e-15);
    assert!((row.chipload_hardness_scale - 1.0).abs() < 1e-15);
    assert!(!row.is_extrapolated);
    let min = row.chip_load_min_mm.expect("the row prints a band");
    let max = row.chip_load_max_mm.expect("the row prints a band");
    // .004-.006 in/tooth as printed.
    assert!((min - 0.1016).abs() < 1e-15, "min {min}");
    assert!((max - 0.1524).abs() < 1e-15, "max {max}");

    let row_30 = row_at(&trace_query(6.35, 1, MaterialFamily::Mdf, 1100.0), 30.0);
    assert_eq!(row_30.observation_id, "onsrud-mdf-37-20-trace");
}

/// Suggest ships a 60 deg V-bit Trace in MDF from the 37-80 row (the cell
/// refused before G2).
#[test]
fn suggest_ships_a_vbit_trace_in_mdf_from_the_onsrud_row_g2() {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::VBit);
    tool.diameter = 6.35;
    tool.flute_count = 2;
    tool.included_angle = 60.0;
    tool.cutting_length = 19.05;
    tool.shank_diameter = 6.35;
    tool.shaft_diameter = 6.35;
    tool.stickout = 27.05;
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    let material = Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    };
    let params = suggest_params(SuggestParamsInput {
        op_type: OperationType::Trace,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("a V-bit Trace in MDF ships from the Onsrud 37-80 row");
    assert_eq!(params.feeds_result.support, FeedsSupport::VendorBacked);
    let row = params
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("the recipe names its row");
    assert_eq!(row.observation_id, "onsrud-mdf-37-80-1-trace");
}
