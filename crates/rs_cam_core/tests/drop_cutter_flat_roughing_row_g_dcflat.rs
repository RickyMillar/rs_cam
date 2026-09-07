//! Sentry — a FLAT end mill on `drop_cutter` resolves a ROUGHING
//! chipload row (G-DCFLAT, 2026-09-08).
//!
//! The defect: `DropCutter` declares `(Parallel, Finish)`,
//! `vendor_lookup::passes_must_match` hard-filters on the operation
//! family, and the embedded LUT publishes no `flat_end` row in the
//! `Parallel` family. A 6 mm flat on a drop-cutter raster therefore read
//! `Unmodeled(NoVendorData)` on the chipload gate, and the feed modulator
//! had no band to drive (measured on the wanaka Arm G rough,
//! `planning/roughing_strategy_ab_results_2026-09-07.md`, addendum).
//!
//! The fix is one arm in `vendor_normalize::lut_query_for`, the single
//! routing site both Suggest and the gate call: a flat tool on
//! `DropCutter` routes to `(Pocket, Roughing)`. This file pins the fix
//! as a ROUTE IDENTITY: the routed flat drop-cutter query resolves the
//! SAME observation the `Adaptive3d` rough resolves on the same tool and
//! material, so the band cannot be read as an invented row. It also
//! keeps the pre-fix reproduction (the unrouted query still finds no
//! row) and pins that ball tools on `DropCutter` are untouched.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, find_best_chip_envelope_row};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use rs_cam_core::feeds::vendor_normalize::lut_query_for;

/// The Arm G tuple: 6 mm two-flute flat end mill in Baltic birch
/// plywood (Janka 1200, `PlywoodGrade::BalticBirch`).
fn flat_6mm_plywood_query(family: LutOperationFamily, role: LutPassRole) -> LookupQuery {
    LookupQuery {
        tool_family: ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm: 6.0,
        flute_count: 2,
        material_family: MaterialFamily::PlywoodHardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1200.0),
        operation_family: family,
        pass_role: role,
    }
}

/// The observation the `Adaptive3d` rough resolves for this tuple. The
/// route identity below is the load-bearing assertion; this id is the
/// human-readable cross-check against the LUT file.
const EXPECTED_ROW: &str = "amana-flat-plywood-hardwood-pocket-6000-2f";

#[test]
fn flat_drop_cutter_routes_to_the_roughing_pocket_row() {
    let routed = lut_query_for(
        OperationType::DropCutter,
        ToolFamily::FlatEnd,
        LutOperationFamily::Parallel,
        LutPassRole::Finish,
    )
    .expect("a flat tool on drop_cutter is routed, never refused");
    assert_eq!(
        routed,
        (LutOperationFamily::Pocket, LutPassRole::Roughing),
        "G-DCFLAT: a flat tool on drop_cutter is a roughing use"
    );
}

#[test]
fn flat_drop_cutter_resolves_the_same_row_as_the_adaptive3d_rough() {
    let lut = embedded_vendor_lut();
    let (dc_family, dc_role) = lut_query_for(
        OperationType::DropCutter,
        ToolFamily::FlatEnd,
        LutOperationFamily::Parallel,
        LutPassRole::Finish,
    )
    .unwrap();
    let (ad_family, ad_role) = lut_query_for(
        OperationType::Adaptive3d,
        ToolFamily::FlatEnd,
        LutOperationFamily::Adaptive,
        LutPassRole::Roughing,
    )
    .unwrap();

    let dc_row = find_best_chip_envelope_row(
        lut,
        &flat_6mm_plywood_query(dc_family, dc_role),
        &ToolGeometryHint::Flat,
    )
    .expect("the routed flat drop_cutter query must resolve a chipload row");
    let ad_row = find_best_chip_envelope_row(
        lut,
        &flat_6mm_plywood_query(ad_family, ad_role),
        &ToolGeometryHint::Flat,
    )
    .expect("the adaptive3d rough resolves a chipload row on this tuple");

    assert_eq!(
        dc_row.observation_id, ad_row.observation_id,
        "route identity: the flat drop_cutter and the adaptive3d rough must read ONE row"
    );
    assert_eq!(dc_row.observation_id, EXPECTED_ROW);
    assert_eq!(dc_row.chip_load_min_mm, ad_row.chip_load_min_mm);
    assert_eq!(dc_row.chip_load_max_mm, ad_row.chip_load_max_mm);
    assert!(
        dc_row.chip_load_min_mm.is_some() && dc_row.chip_load_max_mm.is_some(),
        "the modulator needs BOTH bounds; the row must publish a full band"
    );

    // The raw row bounds, before the diameter and hardness scaling laws.
    let raw = lut
        .observations
        .iter()
        .find(|obs| obs.observation_id == EXPECTED_ROW)
        .expect("the expected row is in the embedded LUT");
    assert_eq!(raw.chipload_min_mm_tooth, Some(0.035));
    assert_eq!(raw.chipload_max_mm_tooth, Some(0.06));
    assert_eq!(raw.operation_family, LutOperationFamily::Pocket);
    assert_eq!(raw.pass_role, LutPassRole::Roughing);
}

/// The pre-fix reproduction: the UNROUTED `(Parallel, Finish)` query on
/// a flat tool finds no chipload row. If a flat-end Parallel row is ever
/// added to the LUT this test starts failing, and the routing arm should
/// then be re-judged against that row rather than kept by inertia.
#[test]
fn unrouted_flat_parallel_finish_has_no_row() {
    let lut = embedded_vendor_lut();
    let row = find_best_chip_envelope_row(
        lut,
        &flat_6mm_plywood_query(LutOperationFamily::Parallel, LutPassRole::Finish),
        &ToolGeometryHint::Flat,
    );
    assert!(
        row.is_none(),
        "the LUT now has a flat-end Parallel row ({}); re-judge the G-DCFLAT route",
        row.map(|r| r.observation_id).unwrap_or_default()
    );
}

#[test]
fn ball_tools_on_drop_cutter_keep_parallel_finish() {
    for tool_family in [
        ToolFamily::BallNose,
        ToolFamily::TaperedBallNose,
        ToolFamily::BullNose,
    ] {
        let routed = lut_query_for(
            OperationType::DropCutter,
            tool_family,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Parallel, LutPassRole::Finish)),
            "{tool_family:?} on drop_cutter is not rerouted"
        );
    }
}

/// The other raster finishes with the same hole are NOT routed by
/// G-DCFLAT. This pins the scope so a widened reroute is a deliberate
/// change with its own sentry, not a side effect.
#[test]
fn other_parallel_ops_on_a_flat_tool_are_not_rerouted() {
    for op in [
        OperationType::Waterline,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
        OperationType::SteepShallow,
        OperationType::RampFinish,
    ] {
        let routed = lut_query_for(
            op,
            ToolFamily::FlatEnd,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        );
        assert_eq!(
            routed,
            Some((LutOperationFamily::Parallel, LutPassRole::Finish)),
            "{op:?} keeps its declared family (listed follow-up, not in G-DCFLAT scope)"
        );
    }
}
