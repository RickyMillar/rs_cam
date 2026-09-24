//! G1 — the micro tapered finish ships the printed tip row (extrapolation
//! P1, the acceptance case at unit level; `P1_PLAN.md` §0 finding 1,
//! orchestrator decision 5).
//!
//! The case: the wanaka 3D Finish tool, a 1.0 mm tapered ball tip, 2 flutes,
//! on a Parallel finish in hardwood (Janka 1450). After ruling A1 the lookup
//! key is the 1.0 mm tip. Two printed 1.0 mm 2-flute hardwood rows exist, and
//! both score 1825:
//!
//! - `amana-tapered-hardwood-parallel-1000-2f-zrn-v8`, band 0.01905-0.0508
//!   mm/tooth (Amana ZrN v8, "Wood, MDF, Sign-Foam", derived grade b by
//!   ruling A4);
//! - `spetool-tapered-hardwood-parallel-1000-2f`, 0.0254 mm/tooth (max only).
//!
//! The score, from `vendor_lookup::score_observation` over the two JSON rows:
//! base 1000 + family 220 + derived 70 + grade b 30 + flutes 80 + diameter
//! 200 + hardness 80 (1450 against 1450) + pass role 45 + material 100 =
//! 1825 (no subfamily term: the query carries none). The tie goes to the
//! smaller id (`beats`), so the Amana row wins. Decision 5 pins the id AND
//! the band, and the SpeTool value lies inside the Amana band: the only
//! two-vendor agreement at a micro size (EXTRAPOLATION_G1 §3.3).
//!
//! The row is at the printed size, so its G1 basis is `Exact` and the arm is
//! `VendorBacked`, not `Extrapolated`. Both scales are exactly 1.0
//! (`1.0^0.61` and `(1450 / 1450)^0.5`), so the band ships as printed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::SizeBasis;
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, find_best_chip_envelope_row, find_best_row_for_geometry, lookup_best,
};
use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ObservationKind,
    ToolFamily, VendorLut,
};
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsSupport, SpindleStrategy, ToolGeometryHint, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const AMANA_ID: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";
const SPETOOL_ID: &str = "spetool-tapered-hardwood-parallel-1000-2f";
/// The printed Amana band (0.00075-0.002 in per tooth).
const BAND: (f64, f64) = (0.01905, 0.0508);
/// The printed SpeTool value (0.001 in per tooth).
const SPETOOL_VALUE: f64 = 0.0254;
/// The A4 card sentence the row carries (ruling A4).
const A4_LABEL: &str = "Wood, MDF, Sign-Foam (one printed row); hardwood is not printed apart";

fn query() -> LookupQuery {
    LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 1.0,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    }
}

fn tapered_geometry() -> ToolGeometryHint {
    ToolGeometryHint::TaperedBall {
        tip_radius: 0.5,
        taper_angle_deg: 7.0,
    }
}

/// The lookup picks the Amana row by id on a 1825 tie, at the printed band.
#[test]
fn the_lookup_picks_the_printed_amana_tip_row_g1() {
    let lut = embedded_vendor_lut();
    let q = query();
    let row = lookup_best(lut, &q).expect("a printed 1.0 mm tapered hardwood row exists");
    assert_eq!(row.observation_id, AMANA_ID);
    assert_eq!(row.score, 1825);
    assert_eq!(row.size_basis, SizeBasis::Exact);
    assert!((row.chip_load_min_mm.unwrap() - BAND.0).abs() < 1e-12);
    assert!((row.chip_load_max_mm.unwrap() - BAND.1).abs() < 1e-12);
    assert!((row.chipload_diameter_scale - 1.0).abs() < 1e-12);
    assert!((row.chipload_hardness_scale - 1.0).abs() < 1e-12);
    assert!(!row.is_extrapolated);
    // The A4 channel: the row says that hardwood is read from a shared
    // "Wood, MDF, Sign-Foam" row, derived grade b.
    assert_eq!(row.material_label, A4_LABEL);
    assert_eq!(row.evidence_grade, EvidenceGrade::B);
    assert_eq!(row.row_kind, ObservationKind::Derived);

    // The recipe resolver (Suggest) and the envelope resolver (the gate)
    // read the same row.
    let geom = tapered_geometry();
    let recipe = find_best_row_for_geometry(lut, &q, &geom).unwrap();
    let envelope = find_best_chip_envelope_row(lut, &q, &geom).unwrap();
    assert_eq!(recipe.observation_id, AMANA_ID);
    assert_eq!(envelope.observation_id, AMANA_ID);
}

/// The tie is real: without the Amana row, the SpeTool row wins at the same
/// 1825, and its printed value lies inside the Amana band.
#[test]
fn the_spetool_row_ties_and_lies_inside_the_amana_band_g1() {
    let without_amana = VendorLut {
        observations: embedded_vendor_lut()
            .observations
            .iter()
            .filter(|o| o.observation_id != AMANA_ID)
            .cloned()
            .collect(),
    };
    let row = lookup_best(&without_amana, &query()).expect("the SpeTool row matches");
    assert_eq!(row.observation_id, SPETOOL_ID);
    assert_eq!(row.score, 1825, "the two rows tie, so the id decides");
    assert!(AMANA_ID < SPETOOL_ID, "the tie-break keeps the smaller id");
    assert_eq!(row.chip_load_min_mm, None);
    assert!((row.chip_load_max_mm.unwrap() - SPETOOL_VALUE).abs() < 1e-12);
    assert!(
        (BAND.0..=BAND.1).contains(&SPETOOL_VALUE),
        "SpeTool {SPETOOL_VALUE} must lie inside the Amana band {BAND:?}"
    );
    assert_eq!(row.material_label, A4_LABEL);
}

/// Suggest ships the printed row: `VendorBacked`, the Amana id, and the
/// printed band before the depth de-rate.
#[test]
fn suggest_ships_the_printed_tip_row_g1() {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    tool.diameter = 1.0;
    tool.flute_count = 2;
    tool.taper_half_angle = 7.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.0;
    tool.shaft_diameter = 6.0;
    tool.stickout = 35.0;
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::default();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    // DropCutter routes to Parallel / Finish.
    let s = suggest_params(SuggestParamsInput {
        op_type: OperationType::DropCutter,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("the printed 1.0 mm tip row answers");
    assert_eq!(s.feeds_result.support, FeedsSupport::VendorBacked);
    assert_eq!(s.feeds_result.vendor_source.as_deref(), Some(AMANA_ID));
    let row = s
        .feeds_result
        .matched_lut_row
        .as_ref()
        .expect("a vendor row answered");
    assert_eq!(row.observation_id, AMANA_ID);
    assert_eq!(row.size_basis, SizeBasis::Exact);
    assert!((row.chip_load_min_mm.unwrap() - BAND.0).abs() < 1e-12);
    assert!((row.chip_load_max_mm.unwrap() - BAND.1).abs() < 1e-12);
    assert_eq!(row.material_label, A4_LABEL);
}
