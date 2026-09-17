//! Tests for `optimize_toolpath`'s early-skip paths — the cases
//! that don't require running a real sim. End-to-end tests with
//! actual sims are deferred to integration tests in
//! `tests/optimize_smoke.rs` (slow path).
//!
//! Moved out of `tool_load/optimize/mod.rs` by P4; the module body is
//! unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::refusal::{bipolar_prescription, deflection_setup_prescription};
use super::*;
use crate::compute::catalog::OperationConfig;
use crate::compute::config::DressupConfig;
use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::feeds::OperationFamily;
use crate::session::ToolpathConfig;
use crate::stock::simulation_cut::SimulationCutTrace;

fn make_tool() -> ToolConfig {
    ToolConfig::new_default(ToolId(0), ToolType::EndMill)
}

fn empty_trace() -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        ..SimulationCutTrace::test_fixture()
    }
}

fn make_tc(operation: OperationConfig, tool_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: "test".to_owned(),
        enabled: true,
        operation,
        dressups: DressupConfig::default(),
        heights: crate::compute::config::HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: crate::compute::config::BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::compute::config::StockSource::Fresh,
        coolant: crate::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn session_with_op(operation: OperationConfig) -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(operation, s.tools()[0].id.0))
        .unwrap();
    s
}

/// F4.3 — every outcome (including refusals/skips) carries the
/// machine snapshot the run consumed, with the cutting ceiling
/// distinct from the travel rate.
#[test]
fn outcome_carries_machine_snapshot() {
    let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    let snap = outcome
        .machine_snapshot
        .as_ref()
        .expect("snapshot on every outcome, even Skipped");
    assert_eq!(snap.name, session.machine().name);
    assert!((snap.max_feed_mm_min - session.machine().max_feed_mm_min).abs() < 1e-9);
    assert!(
        (snap.cutting_feed_ceiling_mm_min - session.machine().cutting_feed_ceiling_mm_min()).abs()
            < 1e-9
    );
    assert!(snap.cutting_feed_ceiling_mm_min <= snap.max_feed_mm_min);
}

#[test]
fn drill_op_yields_skipped() {
    let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped, "got {outcome:?}");
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

#[test]
fn alignment_pin_drill_yields_skipped() {
    let mut session = session_with_op(OperationConfig::AlignmentPinDrill(
        AlignmentPinDrillConfig::default(),
    ));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

#[test]
fn custom_material_yields_skipped() {
    use crate::compute::stock_config::StockConfig;
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let mut stock = session.stock_config().clone();
    // The Custom-material check fires on the variant tag, not on
    // the kc/hardness scalars — any Custom value triggers the
    // MaterialUnvalidated skip. Pre-S3-13 this site hand-built a
    // `{kc: 30.0, hardness: 1.0}` triplet; the test_fixture helper
    // uses `{kc: 10.0, hardness: 1.0}` and the assertion holds
    // identically.
    stock.material = crate::material::Material::test_fixture_custom("test");
    let _ = session.set_stock_config(stock);
    let _ = StockConfig::default(); // silence unused-import false positive

    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(outcome.reason, Some(RefuseReason::MaterialUnvalidated));
}

#[test]
fn invalid_toolpath_index_yields_skipped() {
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 99, &cancel);
    assert!(outcome.kind == OutcomeKind::Skipped);
}

#[test]
fn empty_trace_yields_skipped() {
    // Pocket op (supported), but trace has no samples for the
    // toolpath — Skipped per "no steady-state samples".
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

#[test]
fn cancel_before_run_yields_no_safe_improvement() {
    // Trace has a baseline samples for this toolpath_id, so the
    // early skip paths don't trigger; cancel flag is set before
    // any candidates are evaluated.
    use crate::stock::simulation_cut::SimulationToolpathCutSummary;
    let mut trace = empty_trace();
    trace.toolpath_summaries.push(SimulationToolpathCutSummary {
        toolpath_id: ToolpathId(0),
        sample_count: 100,
        total_runtime_s: 60.0,
        cutting_runtime_s: 50.0,
        rapid_runtime_s: 10.0,
        air_cut_time_s: 0.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.5,
        peak_chipload_mm_per_tooth: 0.04,
        peak_axial_doc_mm: 2.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 100.0,
        average_mrr_mm3_s: 2.0,
        metrics_not_applicable: false,
        per_kinematics: std::collections::BTreeMap::new(),
        runtime_by_intent: None,
    });

    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let cancel = AtomicBool::new(true); // cancelled up-front
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    // Cancel-up-front should not produce a Ranked outcome.
    assert!(
        outcome.kind != OutcomeKind::Ranked,
        "cancelled run should not produce Ranked, got {outcome:?}"
    );
}

// ── Pre-flight refusals (Commit #1) ──────────────────────────────

/// Build a populated trace whose `toolpath_summaries[0]` reports a
/// non-zero cycle time, so `optimize_toolpath` walks past the
/// "no cycle time" skip path and reaches the pre-flight classifier.
/// Adds cutting samples for `toolpath_id = 0` so the force-aware
/// deflection gate has data to evaluate.
fn trace_with_summary_and_high_force_samples() -> SimulationCutTrace {
    use crate::stock::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationToolpathCutSummary,
    };
    let mut trace = empty_trace();
    trace.toolpath_summaries.push(SimulationToolpathCutSummary {
        toolpath_id: ToolpathId(0),
        sample_count: 4,
        total_runtime_s: 60.0,
        cutting_runtime_s: 50.0,
        rapid_runtime_s: 10.0,
        air_cut_time_s: 0.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.8,
        peak_chipload_mm_per_tooth: 0.06,
        peak_axial_doc_mm: 6.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 100.0,
        average_mrr_mm3_s: 2.0,
        metrics_not_applicable: false,
        per_kinematics: std::collections::BTreeMap::new(),
        runtime_by_intent: None,
    });
    // Slot at 6 mm DOC on a 6.35 mm cutter at full π arc — force
    // peaks at Kc × 6 × 6.35. With softwood Kc=6 and the default
    // 45 mm stickout carbide tool, this produces δ around 230 µm,
    // tripping the 200 µm Exceeds threshold.
    for i in 0..4 {
        trace.samples.push(SimulationCutSample {
            move_index: i,
            sample_index: i,
            position: [i as f64, 0.0, -6.0],
            cumulative_time_s: 0.1 * i as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: 1500.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 6.0,
            axial_engagement_mm: 6.0,
            arc_engagement_radians: Some(std::f64::consts::PI),
            chipload_mm_per_tooth: 0.04,
            effective_chip_thickness_mm: Some(0.04),
            engagement: crate::stock::simulation_cut::Engagement::with_radial_woc(1.0),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            ..SimulationCutSample::test_fixture()
        });
    }
    trace
}

/// F2.3 — a deflection-Exceeds baseline whose minimum-force corner
/// (hard-floor DOC × hard-floor stepover) clears the bound must NOT
/// refuse `DeflectionSetupLocked`; the optimizer proceeds into the
/// per-gate retarget search. Pre-F2.3 this exact fixture refused at
/// pre-flight, contradicting the `DeflectionDocRetargeter` (which
/// existed to fix exactly this case) and leaving it dead code.
#[test]
fn deflection_exceeds_with_reachable_corner_searches_instead_of_refusing() {
    // Default endmill (stickout 45 mm, diameter 6.35 mm), 6 mm slot
    // in HardMaple: baseline δ ≈ 350 µm → Exceeds. But at the
    // search-space corner (DOC 0.05 mm × stepover 0.05 mm) the
    // closed-form δ is far under 200 µm, so the levers CAN reach a
    // Within answer.
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let mut stock = session.stock_config().clone();
    stock.material = crate::material::Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };
    let _ = session.set_stock_config(stock);
    let trace = trace_with_summary_and_high_force_samples();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_ne!(
        outcome.reason,
        Some(RefuseReason::DeflectionSetupLocked),
        "reachable corner must not refuse setup-locked, got {outcome:?}"
    );
    // The empty test session can't actually sim candidates, so the
    // search comes back empty-handed — but it must have SEARCHED
    // (NoImprovementFound), not asserted the search is pointless.
    assert_eq!(outcome.reason, Some(RefuseReason::NoImprovementFound));
}

/// F2.3 — a setup whose minimum-force corner still exceeds the
/// bound (1 mm HSS endmill at 60 mm stickout) keeps the
/// `DeflectionSetupLocked` refusal, with the structured detail on
/// the narrative.
#[test]
fn deflection_exceeds_with_unreachable_corner_refuses_setup_locked() {
    let mut tool = make_tool();
    tool.diameter = 1.0;
    tool.shank_diameter = 1.0;
    tool.stickout = 60.0;
    tool.cutting_length = 50.0;
    tool.tool_material = crate::compute::tool_config::ToolMaterial::Hss;
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(tool);
    let tool_id = s.tools()[0].id.0;
    let _ = s
        .add_toolpath(
            0,
            make_tc(OperationConfig::Pocket(PocketConfig::default()), tool_id),
        )
        .unwrap();
    let mut stock = s.stock_config().clone();
    stock.material = crate::material::Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };
    let _ = s.set_stock_config(stock);

    let trace = trace_with_summary_and_high_force_samples();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut s, &trace, 0, &cancel);
    assert_eq!(
        outcome.kind,
        OutcomeKind::NoSafeImprovement,
        "got {outcome:?}"
    );
    assert_eq!(outcome.reason, Some(RefuseReason::DeflectionSetupLocked));
    let explanation = &outcome.narrative.explanation;
    assert!(
        explanation.contains("µm"),
        "explanation should report deflection in µm, got: {explanation}"
    );
    assert!(
        explanation.contains("stickout"),
        "explanation should point at the stickout lever, got: {explanation}"
    );
    // G-EXPL-HIDDEN (2026-08-16): the shape the GUI depends on. A
    // pre-flight refusal builds its narrative from
    // `OutcomeNarrative::default()` plus an explanation, so the
    // headline is EMPTY and the prescription above is the only
    // prose there is. The modal's `NoSafeImprovement` branch used to
    // render `headline` alone, which is why this text has been
    // computed and unshown since F2.3 landed.
    assert!(
        outcome.narrative.headline.is_empty(),
        "a pre-flight refusal carries no headline — the prescription is the whole \
         prose, so any surface rendering only `headline` shows nothing; got: {}",
        outcome.narrative.headline
    );
    let detail = outcome
        .narrative
        .deflection_setup
        .as_ref()
        .expect("structured DeflectionSetupDetail on the narrative");
    assert!(detail.peak_um > detail.bound_um);
    assert!((detail.bound_um - 200.0).abs() < 1e-9);
    assert!(detail.target_stickout_mm > 0.0 && detail.target_stickout_mm < 60.0);
    assert_eq!(
        outcome.candidates.len(),
        1,
        "deflection refusal should not burn any candidate sims — only the baseline \
         candidate should be attempted"
    );
}

#[test]
fn deflection_setup_locked_explanation_carries_target_stickout() {
    // For peak δ=215 µm and current stickout=45 mm, the cube-root
    // scaling target stickout for δ=50 µm is
    //   target_L = 45 × (50/215)^(1/3) ≈ 27.8 mm  →  rounds to "28".
    let tool = crate::tool::ToolDefinition::new(
        Box::new(crate::tool::FlatEndmill::new(6.35, 20.0)),
        6.35,
        20.0,
        40.0,
        45.0,
        2,
        crate::compute::tool_config::ToolMaterial::Carbide,
    );
    let peak_delta_mm = 0.215;
    let p = deflection_setup_prescription(&tool, peak_delta_mm);
    assert!(p.text.contains("215"), "peak µm not in '{}'", p.text);
    assert!(
        p.text.contains("28"),
        "target stickout ~28mm not in '{}'",
        p.text
    );
    assert!(
        p.text.contains("stickout"),
        "stickout lever not in '{}'",
        p.text
    );
    // F2.3 — structured fields mirror the prose, and both bounds
    // (200 µm Exceeds limit, 50 µm Within target) are stated so the
    // prescription can't drift inconsistent again.
    assert!((p.peak_um - 215.0).abs() < 1e-9);
    assert!((p.bound_um - 200.0).abs() < 1e-9);
    assert!((p.target_stickout_mm - 27.8).abs() < 0.5);
    assert!(p.text.contains("200"), "Exceeds limit not in '{}'", p.text);
    assert!(p.text.contains("50"), "Within target not in '{}'", p.text);
}

// ── Q-NARROW (c): the refusal on the outcome ─────────────────────

fn narrow_band_refusal() -> retarget::RetargetRefusal {
    use crate::tool_load::verdict::{ChipBoundsSource, ChipSide};
    retarget::RetargetRefusal::ChiploadBandNarrowerThanHeadroom(retarget::NarrowChipBandRefusal {
        side: ChipSide::High,
        band_min_mm_per_tooth: Some(0.09),
        band_max_mm_per_tooth: 0.10,
        band_source: ChipBoundsSource::VendorLut,
        low_headroom: 1.20,
        high_headroom: 1.20,
        low_target_mm_per_tooth: Some(0.108),
        high_target_mm_per_tooth: 0.10 / 1.20,
        observed_mm_per_tooth: 0.20,
    })
}

fn no_safe_outcome(narrative: OutcomeNarrative) -> OptimizeOutcome {
    OptimizeOutcome::no_safe_improvement(Vec::new(), RefuseReason::NoImprovementFound, narrative)
}

/// The typed reason replaces the generic one, the structured record
/// lands, and the prose says which band and which dial.
#[test]
fn a_refused_retarget_names_itself_on_the_outcome() {
    let outcome = attach_retarget_refusals(
        no_safe_outcome(OutcomeNarrative::default()),
        &[narrow_band_refusal()],
    );
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::ChiploadBandNarrowerThanHeadroom),
        "the generic 'searched and found nothing' reason claims a search \
         that never ran"
    );
    let detail = outcome
        .narrative
        .chipload_band_refusal
        .as_ref()
        .expect("the machine-readable half must land too");
    assert_eq!(detail.band_min_mm_per_tooth, Some(0.09));
    assert!(outcome.narrative.explanation.contains("0.0900"));
    // Nothing was attempted, so the stock headline would have said the
    // search space is empty — which the refusal falsifies.
    assert!(outcome.narrative.headline.contains("narrower than"));
    // G-EXPL-HIDDEN (2026-08-16): the headline used to repeat the
    // band because the modal rendered `headline` and never
    // `explanation`. The modal now renders both, so the numbers are
    // stated once — on the explanation — and the headline keeps only
    // the claim it alone makes: a refusal, not a failed search.
    assert!(
        !outcome.narrative.headline.contains("mm/tooth"),
        "the headline must not duplicate the band the explanation states; got: {}",
        outcome.narrative.headline
    );
}

/// A reason set by a closer classifier is left alone; the structured
/// record still lands, because both facts are true.
#[test]
fn a_refusal_does_not_overwrite_a_more_specific_reason() {
    let outcome = attach_retarget_refusals(
        OptimizeOutcome::no_safe_improvement(
            Vec::new(),
            RefuseReason::DeflectionSetupLocked,
            OutcomeNarrative {
                explanation: "deflection prescription".to_owned(),
                ..OutcomeNarrative::default()
            },
        ),
        &[narrow_band_refusal()],
    );
    assert_eq!(outcome.reason, Some(RefuseReason::DeflectionSetupLocked));
    assert_eq!(outcome.narrative.explanation, "deflection prescription");
    assert!(outcome.narrative.chipload_band_refusal.is_some());
}

/// No refusals — every field is untouched. The control that keeps the
/// two tests above from being true of every outcome.
#[test]
fn no_refusal_leaves_the_outcome_alone() {
    let before = no_safe_outcome(OutcomeNarrative {
        headline: "Tried 3 candidates".to_owned(),
        ..OutcomeNarrative::default()
    });
    let after = attach_retarget_refusals(before.clone(), &[]);
    assert_eq!(after.reason, before.reason);
    assert_eq!(after.narrative.headline, before.narrative.headline);
    assert!(after.narrative.chipload_band_refusal.is_none());
}

#[test]
fn bipolar_prescription_for_doc_knob_op_points_at_doc_or_stepover() {
    // Adaptive3d / Pocket / Adaptive / Rest / Face all have a DOC
    // knob the user can adjust to reduce engagement variance.
    let s = bipolar_prescription(OperationType::Adaptive3d, OperationFamily::Adaptive);
    assert!(
        s.contains("stepover") || s.contains("depth-per-pass"),
        "DOC-knob op should suggest stepover or DOC, got: {s}"
    );
}

#[test]
fn bipolar_prescription_for_3d_finishing_op_points_at_setup() {
    // Parallel and Scallop have no DOC knob — engagement variance
    // is driven by geometry. The prescription should point at a
    // setup-level lever, not a DOC tweak.
    let s = bipolar_prescription(OperationType::Scallop, OperationFamily::Parallel);
    assert!(
        !s.contains("depth-per-pass"),
        "non-DOC-knob op should NOT suggest DOC tweak, got: {s}"
    );
    assert!(
        s.contains("stepover") || s.contains("cutter"),
        "3D finishing prescription should point at stepover or tool, got: {s}"
    );
}

#[test]
fn bipolar_prescription_for_contour_points_at_geometry() {
    // Contour and Trace are profile-following ops — variance comes
    // from the part geometry, not stepover.
    let s = bipolar_prescription(OperationType::Profile, OperationFamily::Contour);
    assert!(
        s.contains("part geometry")
            || s.contains("multiple passes")
            || s.contains("smaller cutter"),
        "contour prescription should point at geometry-driven levers, got: {s}"
    );
}
