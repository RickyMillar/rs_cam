//! H2.1 / R7 sentry — there is exactly ONE join from `GenerationFindings`
//! onto `ToolpathStats`, and it carries every field.
//!
//! Background (`planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md`
//! §H2, and `FINDINGS_PIPELINE_CENSUS.md` in the same directory): the core
//! session path and the GUI compute worker each used to hand-copy this
//! mapping field by field. Neither copy had a compile-time guard against a
//! new finding, and the worker's was patched **five separate times** — the
//! last comment in it read "this is the fifth field to need these lines",
//! and the worst instance (`generate_via_core` narrowing its return to
//! `(toolpath, spans)`) dropped every annotated side-channel at once. A
//! finding that reaches the session path and not the worker is invisible in
//! exactly the product the operator uses.
//!
//! `compute::stats::stats_with_findings` is now that single join. Its
//! structural guard is a destructure with no `..`, so a new
//! `GenerationFindings` field is `E0027` there until someone routes it —
//! the same mechanism the CLI's `ToolpathDiagnostic::from_core` uses to hold
//! its wire honest.
//!
//! Three things are pinned here:
//!
//! (a) every one of the eleven findings survives the join, asserted through
//!     an EXHAUSTIVE destructure of the result — so a new `ToolpathStats`
//!     field fails this sentry as well as the join;
//! (b) the join changes nothing about the move-derived half: it is
//!     `compute_stats_with_spans` plus findings, never a second opinion on
//!     move count, distances, or retract trips;
//! (c) default findings produce the X-19 "not measured" state — `None` and
//!     empty, never a fabricated `Some(0.0)`.
//!
//! Report-only channels throughout: no gate consumes any of these and no
//! verdict moves on them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::config::{
    BoundaryClipDroppedFinding, BoundaryContainment, ClaimsReferenceFinding, ClippedBandFinding,
    DeprecatedDialFinding, DerivedStepoverFinding, DroppedBandFinding, TipFloatFinding,
    ToolpathStats, ZeroRemovalFinding,
};
use rs_cam_core::compute::execute::GenerationFindings;
use rs_cam_core::compute::{compute_stats_with_spans, stats_with_findings};
use rs_cam_core::geo::P3;
use rs_cam_core::measurement::{MeasurementDomain, MeasurementProvenance, MeasurementStage};
use rs_cam_core::ramp_finish::RampReachClamp;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::unified_finish::ClaimsReferenceResolution;

/// A hand-traced move list with a known move count, cutting distance, rapid
/// distance and retract-trip total, so the move-derived half of the join can
/// be asserted by inspection rather than re-derived.
///
/// idx 0 feed (0,0,0) · 1 feed (1,0,0) · 2 rapid (3,0,0) · 3 feed (4,0,0)
/// → 4 moves, 2 mm cutting, 2 mm rapid, one retract trip.
fn traced_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
    tp.feed_to(P3::new(1.0, 0.0, 0.0), 500.0);
    tp.rapid_to(P3::new(3.0, 0.0, 0.0));
    tp.feed_to(P3::new(4.0, 0.0, 0.0), 500.0);
    tp
}

fn provenance() -> MeasurementProvenance {
    MeasurementProvenance::new(
        MeasurementDomain::ProjectedXyArea,
        MeasurementStage::PolygonExtraction,
    )
}

/// Every field set to a value distinguishable from its default, so a field
/// silently dropped by the join reads as `None` / empty / zero rather than
/// coincidentally matching.
fn every_finding_recorded() -> GenerationFindings {
    GenerationFindings {
        truncated_core_mm2: Some(11.0),
        untouched_material_mm2: Some(12.0),
        reached_uncut_estimate_mm2: Some(13.0),
        dropped_band: Some(DroppedBandFinding {
            band_label: "MidSteep",
            region_count: 2,
            area_mm2: 14.0,
            clip_z_mm: -1.5,
            clip_label: "bottom_z",
            provenance: provenance(),
        }),
        clipped_band: Some(ClippedBandFinding {
            band_label: "Shallow",
            region_count: 1,
            area_mm2: 15.0,
            clip_z_mm: -2.5,
            clip_label: "top_z",
            requested_top_z_mm: 0.0,
            requested_bottom_z_mm: -3.0,
            delivered_top_z_mm: -0.5,
            delivered_bottom_z_mm: -2.5,
            planned_levels: 6,
            resolved_levels: 4,
            max_lost_height_mm: 0.5,
            provenance: provenance(),
        }),
        tip_float: Some(TipFloatFinding {
            centreline_points: 100,
            floating_points: 7,
            max_float_mm: 0.25,
        }),
        deprecated_dial: Some(DeprecatedDialFinding {
            dial: "route_width_factor",
            value: 1.5,
            default_value: 1.0,
            replaced_by: "the canonical reach policy",
        }),
        derived_stepovers: vec![
            DerivedStepoverFinding {
                site: "UnifiedFinish crease/pencil claims",
                stepover_mm: 0.20,
                reference_depth_mm: 0.5,
                reference_depth_basis: "cusp",
                envelope_rule_mm: 0.60,
            },
            DerivedStepoverFinding {
                site: "generic rest analysis routing",
                stepover_mm: 0.35,
                reference_depth_mm: 0.5,
                reference_depth_basis: "cusp",
                envelope_rule_mm: 0.60,
            },
        ],
        ramp_reach_clamp: Some(RampReachClamp {
            clamped_points: 3,
            ramp_points: 40,
            max_lift_mm: 0.8,
            requested_bottom_z_mm: -4.0,
            holdable_bottom_z_mm: -3.2,
            lifted_area_mm2: Some(16.0),
        }),
        claims_reference: Some(ClaimsReferenceFinding {
            resolution: ClaimsReferenceResolution::ExplicitSelfProbeOverridingPrior,
            territory_clip_requested: true,
        }),
        zero_removal: Some(ZeroRemovalFinding {
            deepest_engagement_mm: 0.004,
            sampled_positions: 250,
            cutting_distance_mm: 900.0,
            floor_mm: 0.01,
        }),
        // Checkpoint C (Q1 / D-2). `Some(2)` rather than `Some(0)` on
        // purpose: this fixture's job is to make a dropped field read as its
        // default, and `Some(0)` is itself a meaningful measured value here.
        offset_library_failures: Some(2),
        // Checkpoint C (Q2). Recorded by the boundary clip, which runs after
        // the adapter returns — the join must carry it anyway.
        boundary_clip_dropped: Some(BoundaryClipDroppedFinding {
            containment: BoundaryContainment::Inside,
            tool_diameter_mm: 6.0,
            source_region_count: 3,
        }),
    }
}

/// (a) Every recorded finding reaches `ToolpathStats`.
///
/// The destructure below is load-bearing and deliberately has no `..`: a new
/// `ToolpathStats` field fails this sentry until someone states whether the
/// join owes it a value. That is the same contract
/// `stats_with_findings` enforces on the other side, restated where a
/// reader of the channel will see it.
#[test]
fn every_recorded_finding_survives_the_single_join() {
    let tp = traced_toolpath();
    let findings = every_finding_recorded();

    let stats = stats_with_findings(&tp, None, findings.clone());

    let ToolpathStats {
        // Move-derived — pinned separately by
        // `the_join_leaves_the_move_derived_half_alone`.
        move_count: _,
        cutting_distance: _,
        rapid_distance: _,
        retract_trips: _,
        // Generation-owned: every one of these must have come across.
        truncated_core_mm2,
        untouched_material_mm2,
        reached_uncut_estimate_mm2,
        dropped_band,
        tip_float,
        deprecated_dial,
        derived_stepovers,
        clipped_band,
        ramp_reach_clamp,
        claims_reference,
        zero_removal,
        offset_library_failures,
        boundary_clip_dropped,
    } = stats;

    assert_eq!(truncated_core_mm2, findings.truncated_core_mm2);
    assert_eq!(untouched_material_mm2, findings.untouched_material_mm2);
    assert_eq!(
        reached_uncut_estimate_mm2,
        findings.reached_uncut_estimate_mm2
    );
    assert_eq!(
        dropped_band.as_deref().copied(),
        findings.dropped_band,
        "the boxing on the stats side must not change the value"
    );
    assert_eq!(tip_float, findings.tip_float);
    assert_eq!(
        deprecated_dial.as_deref().copied(),
        findings.deprecated_dial
    );
    assert_eq!(
        derived_stepovers, findings.derived_stepovers,
        "a Vec channel: both derivations must survive, in firing order"
    );
    assert_eq!(clipped_band.as_deref().copied(), findings.clipped_band);
    assert_eq!(
        ramp_reach_clamp.as_deref().copied(),
        findings.ramp_reach_clamp
    );
    assert_eq!(claims_reference, findings.claims_reference);
    assert_eq!(zero_removal, findings.zero_removal);
    assert_eq!(offset_library_failures, findings.offset_library_failures);
    assert_eq!(boundary_clip_dropped, findings.boundary_clip_dropped);
}

/// (b) The join is `compute_stats_with_spans` PLUS findings — it does not
/// re-measure the move list, and it does not lose the retract-trip channel
/// that helper computes.
///
/// This is what lets the GUI worker and the core session path share it: both
/// used to derive the move-derived half themselves (the session through
/// `Toolpath::total_cutting_distance`, the worker through this helper), and
/// the two agree because `MoveType` has exactly four variants — `Rapid` plus
/// the three the session's helper counts.
#[test]
fn the_join_leaves_the_move_derived_half_alone() {
    let tp = traced_toolpath();
    let moves_only = compute_stats_with_spans(&tp, None);
    let joined = stats_with_findings(&tp, None, every_finding_recorded());

    assert_eq!(joined.move_count, moves_only.move_count);
    assert_eq!(joined.move_count, 4);
    assert!((joined.cutting_distance - moves_only.cutting_distance).abs() < 1e-12);
    assert!((joined.cutting_distance - 2.0).abs() < 1e-12);
    assert!((joined.rapid_distance - moves_only.rapid_distance).abs() < 1e-12);
    assert!((joined.rapid_distance - 2.0).abs() < 1e-12);
    assert_eq!(joined.retract_trips, moves_only.retract_trips);
    assert_eq!(
        joined.retract_trips.map(|t| t.total),
        Some(1),
        "one contiguous rapid run — the channel must survive the join"
    );
}

/// (c) A generation that recorded nothing reports NOT MEASURED, not zero
/// (`MEASUREMENT_DOMAINS.md` X-19). The join must not manufacture a
/// `Some(0.0)` on the way through — that is the exact reading a ratio would
/// be built on.
#[test]
fn an_unrecorded_generation_still_reads_as_not_measured() {
    let tp = traced_toolpath();
    let stats = stats_with_findings(&tp, None, GenerationFindings::default());

    assert_eq!(stats.truncated_core_mm2, None);
    assert_eq!(stats.untouched_material_mm2, None);
    assert_eq!(stats.reached_uncut_estimate_mm2, None);
    assert!(stats.dropped_band.is_none());
    assert!(stats.clipped_band.is_none());
    assert_eq!(stats.tip_float, None);
    assert!(stats.deprecated_dial.is_none());
    assert!(
        stats.derived_stepovers.is_empty(),
        "the documented Vec exception: empty means nothing derived"
    );
    assert!(stats.ramp_reach_clamp.is_none());
    assert_eq!(stats.claims_reference, None);
    assert_eq!(stats.zero_removal, None);
    assert_eq!(
        stats.offset_library_failures, None,
        "an operation that ran no reporting offset has measured nothing —          `Some(0)` here would claim every offset was clean"
    );
    assert_eq!(stats.boundary_clip_dropped, None);
    assert!(
        stats.retract_trips.is_some(),
        "retract trips are computed after generation from the move list, so \
         they are measured even when no finding was recorded"
    );
}
