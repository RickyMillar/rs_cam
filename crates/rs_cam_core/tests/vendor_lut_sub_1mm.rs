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
fn sub_1mm_tapered_ball_hardwood_finish_documented_gap() {
    // Refusal-first: the Amana ZrN 3D Profiling chart conflates softwood/hardwood
    // under one "Wood" row. We deliberately did not emit hardwood rows from that
    // source for sub-1mm tools. A 1mm tapered ball + hardwood + parallel/finish
    // query should NOT match any sub-1mm row (the 3.175mm rows are out of the
    // 0.5x-2x diameter ratio range for a 1mm query).
    let lut = VendorLut::embedded();
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 1.0,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    };
    assert!(
        lookup_best(&lut, &query).is_none(),
        "sub-1mm hardwood ball/tapered-ball is a documented coverage gap; \
         see source_manifest.json amana_zrn_3d_profiling.coverage_notes"
    );
}

#[test]
fn embedded_count_matches_after_expansion() {
    let lut = VendorLut::embedded();
    assert_eq!(
        lut.observations.len(),
        67,
        "expected 67 embedded observations after Item E expansion (was 61 + 6 new sub-1mm rows)"
    );
}
