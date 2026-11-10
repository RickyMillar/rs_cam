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
//!
//! Operator ruling Q3 (feeds contract P5, D2): on the Parallel-family
//! operations the SpeTool 1.0 mm row (`spetool-tapered-hardwood-parallel-
//! 1000-2f`, one printed value 0.001 in = 0.0254 mm, held as a point, A2)
//! ties with the v8 row, and the lower printed chip wins: those operations
//! read the SpeTool row, and the vendor band (P16 item 8) names the v8
//! row. G1 orchestrator decision 5 reversed by operator ruling Q3. The SpeTool row is not a
//! candidate on Contour, Scallop and Trace (no family rule transfers it), so
//! those operations still read the v8 row, Transferred. Open data question
//! (lead, 2026-11-03): one Ø1 tapered tool reads two rows by finish family.
//!
//! **RE-PINNED 2026-11-10, feeds contract P23c (operator ruling
//! 2026-11-10).** The SpeTool 2D/3D tapered chart now serves the 3D finish
//! families by its own family rule, so the SpeTool row is a candidate on
//! Contour, Scallop and Trace too. Both rows print the 1.0 mm tip (exact),
//! and the lower printed chip ships (Q3): every 3D finish reads the SpeTool
//! point 0.0254 mm, was the v8 band 0.01905-0.0508 on Contour, Scallop and
//! Trace. The family basis there is `Transferred` from the SpeTool row.
//! The open data question is closed: one tool reads one row.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolFact;
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
use rs_cam_core::tool_load::{ChipTarget, chip_target_for_toolpath};

const TOL: f64 = 1e-12;

/// The printed Ø1 2-flute hardwood row.
const ROW: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";
/// The Q3 winner on the Parallel-family operations.
const SPETOOL_ROW: &str = "spetool-tapered-hardwood-parallel-1000-2f";
/// The SpeTool printed chip load per tooth, inches (one value).
const SPETOOL_PRINTED_IN: f64 = 0.001;
/// Printed chip load per tooth, inches, `amana_zrn_3d_v8.txt:9`.
const PRINTED_MIN_IN: f64 = 0.000_75;
const PRINTED_MAX_IN: f64 = 0.002;
/// Inches to millimetres, by definition.
const MM_PER_IN: f64 = 25.4;

/// A tapered ball with a Ø1 tip (the lookup key, ruling A1), 2 flutes.
fn tapered_one_mm() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::TaperedBallNose);
    t.diameter = 1.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(12.0));
    t.set_taper_half_angle(ToolFact::Stated(7.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(20.0));
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
        stock_z: Some(18.0),
        stock_padding: 2.0,
    };
    let spetool = SPETOOL_PRINTED_IN * MM_PER_IN;
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
        assert_eq!(recipe.size_basis, SizeBasis::Exact, "{label}");
        let parallel = family == LutOperationFamily::Parallel;
        {
            // Q3: the SpeTool point wins and names the v8 row. P23c: on every
            // family (the SpeTool rows transfer to Contour, Scallop and Trace).
            assert_eq!(recipe.observation_id, SPETOOL_ROW, "{label}");
            assert_eq!(
                recipe
                    .band
                    .members
                    .iter()
                    .map(|m| m.row.id())
                    .find(|id| *id != recipe.observation_id),
                Some(ROW),
                "{label}: the Q3 winner names the other row"
            );
            // The band holds the v8 row with its printed band.
            let v8 = recipe
                .band
                .members
                .iter()
                .find(|m| m.row.id() == ROW)
                .unwrap_or_else(|| panic!("{label}: the band names the v8 row"));
            let (lo, hi) = (PRINTED_MIN_IN * MM_PER_IN, PRINTED_MAX_IN * MM_PER_IN);
            assert!(
                (v8.chip_min_mm.expect("a printed minimum") - lo).abs() < TOL
                    && (v8.chip_max_mm.expect("a printed maximum") - hi).abs() < TOL,
                "{label}: {v8:?}"
            );
            assert_eq!(recipe.chip_load_min_mm, None, "{label}: a point");
            assert!(
                (recipe.chip_load_max_mm.expect("a printed value") - spetool).abs() < TOL,
                "{label}: {recipe:?}"
            );
        }
        if parallel {
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
            assert_eq!(claim.source_rows, vec![SPETOOL_ROW.to_owned()], "{label}");
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
        assert_eq!(env.observation_id, recipe.observation_id, "{label}");
        assert_eq!(env.size_basis, SizeBasis::Exact, "{label}");
        assert_eq!(env.chip_load_min_mm, recipe.chip_load_min_mm, "{label}");
        assert_eq!(env.chip_load_max_mm, recipe.chip_load_max_mm, "{label}");

        // The modulator's and the advisor's door (no simulation, so no
        // depth de-rate): the printed band.
        let target =
            chip_target_for_toolpath(&material, &tool, &operation, ToolpathId::from_raw(0), None)
                .unwrap_or_else(|| panic!("{label}: the door gave no target"));
        {
            // A2: a point has no band; the door holds the printed point.
            match target {
                ChipTarget::Point(v) => {
                    assert!((v - spetool).abs() < TOL, "{label}: {target:?}");
                }
                ChipTarget::Band(_) => panic!("{label}: a point row gave a band: {target:?}"),
            }
        }
        checked += 1;
    }
    // Non-vacuity: every operation in the table was checked.
    assert_eq!(checked, FINISHES.len());
    assert_eq!(checked, 10);
}
