//! Integration test for sub-1mm ball/tapered-ball coverage in the vendor LUT.
//!
//! Validates that after the 2026-05-02 coverage expansion (Item E from
//! `tool-load-fidelity-and-suggest.md`), a 1mm tapered ball + parallel + finish
//! query against a softwood material now matches at least one row.
//!
//! Extrapolation P1 step 1 (2026-09-24) loads the Amana ZrN v8 and the SpeTool
//! 2D/3D tapered charts as `tapered_ball_nose` rows keyed on the tip. Ruling A4
//! serves hardwood from the shared "Wood, MDF, Sign-Foam" row as derived,
//! grade b. The hardwood sub-1mm case is therefore no longer a gap. See
//! `data/vendor_lut/source_manifest.json` `amana_zrn_3d_profiling_v8` and
//! `spetool_2d3d_tapered_router_bit_chart`.

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
        "1mm tapered ball + softwood + parallel/finish should match a sub-1mm tapered-ball row \
         after Item E coverage expansion",
    );
    // The v8 tapered row and the SpeTool 1.0 mm row both score 1825. The
    // smaller id wins the tie, so the Amana id must sort first. The kept
    // ball_nose row (for the straight ball 46471) scores 1795 on the family
    // fallback.
    assert_eq!(
        result.observation_id, "amana-tapered-softwood-parallel-1000-2f-zrn-v8",
        "expected the 1mm 2-flute ZrN v8 tapered row to win for a 1mm 2-flute query"
    );
}

#[test]
fn sub_1mm_tapered_ball_hardwood_finish_matches_the_printed_tip_row() {
    // History. Before 2026-05-08 a [0.5, 2.0] ratio gate refused this query.
    // From 2026-05-08 the lookup scaled the closest >= 1 mm hardwood row down
    // to 0.5 mm and set `is_extrapolated`.
    //
    // Extrapolation P1 step 1 (2026-09-24) loads the SpeTool 2D/3D tapered
    // chart. It prints a 0.5 mm tip row (0.0007 in/tooth, one value), and
    // ruling A4 serves hardwood from its shared "Wood" row (derived, grade b).
    // A 0.5 mm query now matches that row at a ratio of 1.0.
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
        .expect("0.5mm hardwood tapered ball should match the SpeTool 0.5 mm tip row");
    assert_eq!(
        result.observation_id,
        "spetool-tapered-hardwood-parallel-0500-2f"
    );
    assert!(
        (result.row_diameter_mm - 0.5).abs() < 1e-9,
        "expected the exact 0.5 mm row, got {} mm",
        result.row_diameter_mm
    );
    assert!(
        !result.is_extrapolated,
        "0.5 mm against the 0.5 mm row must not trip extrapolation"
    );
    assert!((result.chipload_diameter_ratio_raw - 1.0).abs() < 1e-12);
    assert!((result.chipload_diameter_scale - 1.0).abs() < 1e-12);
    assert!((result.chip_load_mm - 0.01778).abs() < 1e-9);
}

#[test]
fn sub_half_mm_tapered_ball_hardwood_finish_extrapolates_with_scaling() {
    // **G-SUB1MM, fixed at Checkpoint K (f1), 2026-08-13.**
    //
    // This test used to read `chipload_diameter_scale` against
    // `query / row_diameter_mm`. It had been red since 2026-08-06, when
    // `CHIPLOAD_DIAMETER_EXPONENT` moved from 1.0 to 0.61 (B-lit §4.1). At an
    // exponent of 1.0 the raw transfer ratio and the applied scale are equal,
    // so one field could stand for both. At 0.61 they are different. The crate
    // publishes both (`LookupResult::chipload_diameter_ratio_raw`), so that
    // they cannot be conflated.
    //
    // Extrapolation P1 step 1 (2026-09-24) adds a printed 0.5 mm tip row, so a
    // 0.5 mm query no longer extrapolates (see the test above). This test
    // keeps the guard on a 0.3 mm query against that row (ratio 0.6). This is
    // a raw lookup test: the lookup itself does not refuse a tip under 0.5 mm.
    // Only the Suggest support arm refuses it (`support::TAPERED_MIN_TIP_MM`,
    // ruling B1, P1 step 2). Step 3 puts the size claim into `build_result`:
    // the lookup still matches the 0.5 mm row, but its G1 basis is `Refused`
    // (the tip floor), so the row publishes no band. The two ratio fields keep
    // their meaning, because a refusal is not a claim and applies no scale of
    // its own.
    let lut = VendorLut::embedded();
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: 0.3,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Parallel,
        pass_role: LutPassRole::Finish,
    };
    let result = lookup_best(&lut, &query)
        .expect("0.3mm hardwood tapered ball should extrapolate from the closest available row");
    assert!(
        result.is_extrapolated,
        "0.3 mm against the smallest tapered-ball hardwood row must trip extrapolation"
    );
    assert!(
        (result.row_diameter_mm - 0.5).abs() < 1e-9,
        "expected to scale down from the 0.5 mm row, got row diameter {}",
        result.row_diameter_mm
    );
    let expected_ratio = 0.3 / result.row_diameter_mm;
    assert!(
        (result.chipload_diameter_ratio_raw - expected_ratio).abs() < 1e-6,
        "raw diameter transfer ratio must be the bare 0.3/{:.4} = {expected_ratio:.9}; got {:.9}",
        result.row_diameter_mm,
        result.chipload_diameter_ratio_raw
    );
    assert!(
        result.size_basis.is_refused(),
        "a 0.3 mm tip is under the 0.5 mm floor: {:?}",
        result.size_basis
    );
    assert_eq!(result.chip_load_min_mm, None);
    assert_eq!(result.chip_load_max_mm, None);
    let expected_scale = result
        .chipload_diameter_ratio_raw
        .powf(rs_cam_core::feeds::vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT);
    assert!(
        (result.chipload_diameter_scale - expected_scale).abs() < 1e-12,
        "applied scale must be raw^CHIPLOAD_DIAMETER_EXPONENT ({:.9}^{} = {expected_scale:.9}); \
         got {:.9}. If these ever coincide again the exponent went back to 1.0 — say so, \
         because the two fields then stop being distinguishable and this test stops \
         measuring the conflation it exists for.",
        result.chipload_diameter_ratio_raw,
        rs_cam_core::feeds::vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT,
        result.chipload_diameter_scale
    );
}

#[test]
fn embedded_count_matches_after_expansion() {
    let lut = VendorLut::embedded();
    assert_eq!(
        lut.observations.len(),
        476,
        "expected 476 embedded observations, the same total that \
         vendor_lut::tests::test_embedded_loads_all_observations pins: 389 \
         after feeds matrix R5 (2026-09-23), - 2 Amana ZrN ball_nose rows in \
         cells that list only tapered tools, + 27 Amana ZrN v8 tapered rows, \
         + 27 SpeTool 2D/3D tapered rows (extrapolation P1, 2026-09-24), \
         + 40 Onsrud 37-series V-bit rows (MDF, plywood, chipboard) and + 15 Amana ball v7 pocket rows \
         (extrapolation P2, G2) = 496, - 24 Onsrud 77-100 copy rows (4 sheets \
         x 1/8 and 1/4 in x parallel / scallop / adaptive; A3 step 3, G3) = 472, \
         - 2 derived onsrud-bull softwood and hardwood rows, + 6 printed Amana \
         corner-radius bull rows (A3 step 4, G3) = 476"
    );
}
