//! Integration test for sub-1mm ball/tapered-ball coverage in the vendor LUT.
//!
//! Validates that after the 2026-05-02 coverage expansion (Item E from
//! `tool-load-fidelity-and-suggest.md`), a 1mm tapered ball + parallel + finish
//! query against a softwood material now matches at least one row.
//!
//! The hardwood case is intentionally a documented gap — see
//! `data/vendor_lut/source_manifest.json` `amana_zrn_3d_profiling.coverage_notes`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::feeds::vendor_lookup::{LookupQuery, lookup_best};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
};

#[test]
fn sub_1mm_tapered_ball_softwood_finish_matches() {
    let lut = VendorLut::embedded();
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 1.0,
        flute_count: 2,
        material_family: MaterialFamily::Softwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(600.0),
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    };
    let result = lookup_best(&lut, &query).expect(
        "1mm tapered ball + softwood + parallel/finish should match a sub-1mm ball-nose row \
         after Item E coverage expansion",
    );
    assert_eq!(
        result.observation_id, "amana-ball-softwood-parallel-1000-2f-zrn",
        "expected the 1mm 2-flute ZrN row to win for a 1mm 2-flute query"
    );
}

#[test]
fn sub_1mm_tapered_ball_hardwood_finish_extrapolates_with_scaling() {
    // Used to be a "documented gap" — pre-G5/G6/G7 (2026-05-08) the
    // [0.5, 2.0] hard ratio gate refused on the larger rows and the
    // lookup returned None. With engaged-edge scaling the lookup now
    // matches the closest hardwood tapered-ball row and scales chipload
    // bounds linearly by the diameter ratio.
    //
    // Asserts the *spirit* (closest match + scaling + extrapolation
    // flag), not a specific row id: Phase 4+ LUT promotions can
    // legitimately introduce closer matches without invalidating the
    // property. Query is 0.5 mm — well below every tapered-ball
    // hardwood row currently in the LUT.
    let lut = VendorLut::embedded();
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 0.5,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    };
    let result = lookup_best(&lut, &query)
        .expect("0.5mm hardwood tapered ball should extrapolate from the closest available row");
    assert!(
        result.is_extrapolated,
        "0.5 mm against any tapered-ball hardwood row in the LUT must trip extrapolation"
    );
    assert!(
        result.row_diameter_mm >= 1.0,
        "expected to scale up from a >=1mm row, got row diameter {}",
        result.row_diameter_mm
    );
    let expected_scale = 0.5 / result.row_diameter_mm;
    assert!((result.chipload_diameter_scale - expected_scale).abs() < 1e-6);
}

#[test]
fn embedded_count_matches_after_expansion() {
    let lut = VendorLut::embedded();
    assert_eq!(
        lut.observations.len(),
        228,
        "expected 228 embedded observations (111 baseline + 117 from the \
         2026-05-30 Phase 4 bulk promotion: 37 amana_long_tail, 47 onsrud_ocr, \
         13 whiteside_fusion360, 10 freud_solid_carbide, 10 idcwoodcraft_millmage)"
    );
}
