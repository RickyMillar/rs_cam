//! G3 v8 — the printed Amana ZrN v8 tip row serves every 3D finish.
//!
//! Operator ruling 2026-10-01 ("yes"): the Amana ZrN 2D/3D carving chart
//! (v8) prints one chip load per tip diameter, flute count and material,
//! and names no operation. The LUT files its tapered rows under
//! (Parallel, Finish) as an assignment (the row notes,
//! `data/vendor_lut/observations/amana_zrn_tapered_v8.json`). The v8 family
//! rule (`feeds::extrapolation::FAMILY_RULES`, home (Parallel, Finish))
//! serves Contour, Scallop and Trace, so every 3D finishing operation of a
//! tapered ball reads the printed row.
//!
//! Before the ruling, a Ø1 tapered tip in hardwood shipped the printed band
//! on DropCutter and refused on Scallop: the only Scallop row was the
//! Onsrud 77-100 1/4 in pocket row (Ø6.35), outside the 0.5x-2x window of
//! a tool under 1.5 mm.
//!
//! The arm: a Ø1 2-flute tapered ball in hardwood (`GenericHardwood`, Janka
//! 1450 = the row's 1450, so no hardness scale) on each 3D finishing
//! operation. Suggest, the envelope resolver and the modulator's door read
//! `amana-tapered-hardwood-parallel-1000-2f-zrn-v8`, size basis `Exact`,
//! with the band printed x 25.4: 0.00075-0.002 in = 0.01905-0.0508 mm
//! (`data/vendor_lut/sources/amana_zrn_3d_v8.txt:9`, column 1mm, row
//! "Wood, MDF, Sign-Foam"). The family basis is `Printed` on the
//! Parallel-family operations and `Transferred` from (Parallel, Finish) on
//! the others.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{FamilyBasis, SizeBasis};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, feeds_input_for_operation, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::find_best_chip_envelope_row;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsSupport, SpindleStrategy};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool_load::chipload_envelope_for_toolpath;

const TOL: f64 = 1e-12;

/// The printed Ø1 2-flute hardwood row.
const ROW: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";
/// Printed chip load per tooth, inches, `amana_zrn_3d_v8.txt:9`.
const PRINTED_MIN_IN: f64 = 0.000_75;
const PRINTED_MAX_IN: f64 = 0.002;
/// Inches to millimetres, by definition.
const MM_PER_IN: f64 = 25.4;

/// A tapered ball with a Ø1 tip (the lookup key, ruling A1), 2 flutes.
fn tapered_one_mm() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    t.diameter = 1.0;
    t.flute_count = 2;
    t.cutting_length = 12.0;
    t.taper_half_angle = 7.0;
    t.shank_diameter = 6.0;
    t.shaft_diameter = 6.0;
    t.stickout = 20.0;
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// The 3D finishing operations and the LUT family each one queries
/// (`compute::catalog::registry`, `feeds_family`).
const FINISHES: &[(OperationType, LutOperationFamily)] = &[
    (OperationType::Scallop, LutOperationFamily::Scallop),
    (OperationType::UnifiedFinish, LutOperationFamily::Scallop),
    (OperationType::SpiralFinish, LutOperationFamily::Scallop),
    (OperationType::Waterline, LutOperationFamily::Contour),
    (OperationType::SteepShallow, LutOperationFamily::Contour),
    (OperationType::Pencil, LutOperationFamily::Trace),
    (OperationType::DropCutter, LutOperationFamily::Parallel),
    (OperationType::RampFinish, LutOperationFamily::Parallel),
    (OperationType::RadialFinish, LutOperationFamily::Parallel),
    (
        OperationType::HorizontalFinish,
        LutOperationFamily::Parallel,
    ),
];

/// A Ø1 tapered tip in hardwood gets the printed v8 band on every 3D
/// finishing operation, at every consumer.
#[test]
fn a_one_millimetre_tapered_tip_reads_the_printed_band_on_every_3d_finish_g3v8() {
    let tool = tapered_one_mm();
    let material = hardwood();
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    let lo = PRINTED_MIN_IN * MM_PER_IN;
    let hi = PRINTED_MAX_IN * MM_PER_IN;
    let mut checked = 0;
    for &(op, family) in FINISHES {
        let label = format!("{op:?}");

        // Suggest: the support arm and the pre-derate band.
        let s = suggest_params(SuggestParamsInput {
            op_type: op,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock,
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .unwrap_or_else(|e| panic!("{label}: a Ø1 tapered finish in hardwood ships, got {e}"));
        let recipe = s
            .feeds_result
            .matched_lut_row
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: a vendor row answered"));
        assert_eq!(recipe.observation_id, ROW, "{label}");
        assert_eq!(recipe.size_basis, SizeBasis::Exact, "{label}");
        assert!(
            (recipe.chip_load_min_mm.expect("a printed minimum") - lo).abs() < TOL,
            "{label}: {recipe:?}"
        );
        assert!(
            (recipe.chip_load_max_mm.expect("a printed maximum") - hi).abs() < TOL,
            "{label}: {recipe:?}"
        );
        if family == LutOperationFamily::Parallel {
            assert_eq!(recipe.family_basis, FamilyBasis::Printed, "{label}");
        } else {
            let FeedsSupport::FamilyTransferred {
                family: claim,
                size,
            } = &s.feeds_result.support
            else {
                panic!(
                    "{label}: expected FamilyTransferred, got {:?}",
                    s.feeds_result.support
                );
            };
            assert!(size.is_none(), "{label}: the tip is the printed size");
            assert_eq!(claim.source_rows, vec![ROW.to_owned()], "{label}");
            assert_eq!(
                claim.home,
                (LutOperationFamily::Parallel, LutPassRole::Finish),
                "{label}"
            );
            assert_eq!(claim.query.0, family, "{label}");
        }

        // The gate's resolver: the same row and the same band.
        let operation = OperationConfig::new_default(op);
        let input = feeds_input_for_operation(
            &operation,
            &tool,
            &material,
            &machine,
            &EMBEDDED_LUT,
            SpindleStrategy::MatchChart,
        );
        let query = to_lookup_query(&input).expect("the routing answers");
        assert_eq!(query.operation_family, family, "{label}");
        let env = find_best_chip_envelope_row(&EMBEDDED_LUT, &query, &input.tool_geometry)
            .unwrap_or_else(|| panic!("{label}: a chipload-bearing row matches"));
        assert_eq!(env.observation_id, ROW, "{label}");
        assert_eq!(env.size_basis, SizeBasis::Exact, "{label}");
        assert_eq!(env.chip_load_min_mm, recipe.chip_load_min_mm, "{label}");
        assert_eq!(env.chip_load_max_mm, recipe.chip_load_max_mm, "{label}");

        // The modulator's and the advisor's door (no simulation, so no
        // depth de-rate): the printed band.
        let door =
            chipload_envelope_for_toolpath(&material, &tool, &operation, ToolpathId(0), None)
                .unwrap_or_else(|| panic!("{label}: the door gave no band"));
        assert!((door.start - lo).abs() < TOL, "{label}: {door:?}");
        assert!((door.end - hi).abs() < TOL, "{label}: {door:?}");
        checked += 1;
    }
    // Non-vacuity: every operation in the table was checked.
    assert_eq!(checked, FINISHES.len());
    assert_eq!(checked, 10);
}
