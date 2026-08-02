//! PR-6a sentry (H2.3) — `UnifiedFinish`'s crease/pencil offset stepover is
//! sized by the canonical reach policy, not by the cutter ENVELOPE.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Guards**: the H2.3 slice. Before it,
//!   `unified_finish.rs` derived the stepover it fed to BOTH the coverage
//!   routing criterion and the emitted offset fan as
//!   `cutter.envelope_radius_mm() * 0.5` — 1.5 mm on the shipped Ø1-tip /
//!   7° / Ø6-shank taper, i.e. half the SHANK. That was the last
//!   envelope-scaled routing/fit number in the finishing stack
//!   (`planning/review_2026-07-29/ORCHESTRATION_LOG.md`, wave-A entry).
//! * **Variables held fixed**: one mesh, one set of planner thresholds, one
//!   tolerance, pinned heights. The only variable between the tapered and
//!   the ball run is the tool.
//! * **Metric domains**: `offset_stepover_mm` and `envelope_rule_stepover_mm`
//!   are both LATERAL distances in mm on the XY plane of the rest grid.
//!   `crease_path_length_mm` is 3-D cutting length of `FinishingCut` moves.
//!   Nothing here relates a length to an area.
//!
//! ## Why this cannot pass vacuously
//!
//! Gate 1 asserts the claims pipeline actually RAN (paths emitted, non-zero
//! cutting length) before it asserts anything about the stepover. Gate 2
//! measures pass spacing on emitted geometry, and asserts BOTH arms emitted
//! passes before comparing them. Gate 3's ball control demands non-vacuity
//! the same way.
//!
//! ## Why it would have been red before the fix
//!
//! Gate 1's central assertion is that the derived stepover is NOT the
//! envelope rule on a tapered tool; pre-fix it was that number by
//! construction. Gate 2 measures the emission consequence on the shipped
//! pencil emitter, so a revert that kept the telemetry honest but restored
//! the envelope spacing still fails.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

// ── Tool geometry (the wanaka-class finisher, as in M2.1) ───────────────

const TIP_DIAMETER_MM: f64 = 1.0;
const SHANK_DIAMETER_MM: f64 = 6.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;
const BALL_DIAMETER_MM: f64 = 3.0;

fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(
        TIP_DIAMETER_MM,
        TAPER_HALF_ANGLE_DEG,
        SHANK_DIAMETER_MM,
        25.0,
    )
}

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: BALL_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

// ── Mesh fixture ────────────────────────────────────────────────────────

fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A 30 × 30 plateau with two grooves the Ø1 tip can work and the Ø6 shank
/// cannot: a 4 mm-wide 45° V (a valley WIDE enough that a fan actually
/// fits — the envelope rule's `half_width − 3.0` is negative here, which is
/// exactly the dead-code symptom Checkpoint A recorded) and a narrow steep
/// one.
fn two_groove_plateau() -> TriangleMesh {
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-8.0, 0.0),
        (-6.0, -2.0),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    extrude_profile(&profile, -15.0, 15.0)
}

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "two_groove_plateau".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://two_groove_plateau.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        // The whole point: the claims pipeline is the site that derives the
        // stepover. Default-off, so the fixture must turn it on.
        pencil_claims: true,
        ..UnifiedFinishConfig::default()
    })
}

fn toolpath(op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Unified Finish".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-9.0),
            ..HeightsConfig::default()
        },
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn stock() -> StockConfig {
    StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The REAL production entry point, so the derivation is exercised through
/// the wiring the GUI and the CLI share.
fn generate_through_session(tool: ToolConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    session
        .add_toolpath(0, toolpath(unified_finish_op(), tool_id, model_id))
        .expect("add unified-finish toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("UnifiedFinish must generate on the two-groove plateau");
    session
}

// ── Gate 1: the derived value, through production wiring ────────────────

/// The stepover that steers the crease fan comes from
/// [`rs_cam_core::reach::suggested_offset_stepover_mm`], and on the shipped
/// taper it is NOT the envelope rule.
#[test]
fn the_claims_stepover_is_the_reach_policy_value_not_the_envelope_rule() {
    let session = generate_through_session(tapered_ball_tool());
    let stats = session
        .get_result(0)
        .expect("a generated result")
        .stats
        .clone();

    // C8: a collection now. Exactly one derivation is expected here — the
    // claims pipeline's own — and asserting that is stronger than taking
    // whichever one happened to be recorded first.
    assert_eq!(
        stats.derived_stepovers.len(),
        1,
        "expected exactly one derivation: {:?}",
        stats.derived_stepovers
    );
    let finding = stats.derived_stepovers[0];

    println!(
        "PR-6a taper: derived {:.4} mm, envelope rule {:.4} mm, ref depth {:.4} mm ({})",
        finding.stepover_mm,
        finding.envelope_rule_mm,
        finding.reference_depth_mm,
        finding.reference_depth_basis,
    );

    let t = taper();
    // The retired number, stated as a literal so a change to
    // `envelope_radius_mm` cannot silently redefine the baseline.
    assert!(
        (finding.envelope_rule_mm - 1.5).abs() < 1e-9,
        "the retired envelope rule on this taper is half the Ø6 SHANK: got {}",
        finding.envelope_rule_mm
    );
    // The policy value, recomputed from the policy itself — one
    // implementation, asserted rather than duplicated.
    let expected = rs_cam_core::reach::suggested_offset_stepover_mm(&t, finding.reference_depth_mm);
    assert!(
        (finding.stepover_mm - expected).abs() < 1e-12,
        "the site must call the policy, not restate it: {} vs {expected}",
        finding.stepover_mm
    );
    assert!(
        !finding.matches_the_envelope_rule(),
        "on a Ø1-tip / Ø6-shank taper the two MUST differ — if they agree, \
         the envelope-scaled number is back"
    );
    assert!(
        finding.stepover_mm < finding.envelope_rule_mm,
        "the policy value is never coarser than the envelope rule"
    );
    // 0.25 mm — half the Ø1 tip's cusp radius, the cusp floor binding at
    // the detector's shallowest reported rest.
    assert!(
        (finding.stepover_mm - 0.25).abs() < 1e-9,
        "expected the tip-scale 0.25 mm, got {}",
        finding.stepover_mm
    );
}

/// The number reaches the operator. Report-only, `Info`, and it names both
/// the value and what it replaced.
#[test]
fn the_derived_stepover_is_reported_as_a_diagnostic() {
    let session = generate_through_session(tapered_ball_tool());
    let stats = session
        .get_result(0)
        .expect("a generated result")
        .stats
        .clone();
    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(0),
        &stats,
    );
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_DERIVED_STEPOVER)
        .expect("the taper's derived stepover differs from the envelope rule, so it must report");
    assert_eq!(d.severity, rs_cam_core::diagnostics::Severity::Info);
    assert!(
        d.message.contains("0.250") && d.message.contains("1.500"),
        "both numbers must appear: {}",
        d.message
    );
}

// ── Gate 2: the EMISSION consequence, measured as pass spacing ──────────

/// Pass spacing on emitted geometry — **claim rewritten by C9, and the
/// direction it moved is the point.**
///
/// PR-6a's claim was that feeding the emitter the DERIVED stepover instead of
/// the envelope rule resolves more distinct pass positions: the caller's
/// scalar decided the spacing, and PR-6a made callers pass a better scalar.
/// C9's per-point fan retired the scalar itself
/// (`crease_paths::centerline_cut_paths`): on a MEASURED centreline every
/// point now gets `reach::suggested_offset_stepover_mm` at its OWN depth, and
/// the `offset_stepover` argument is only the fallback for centrelines with no
/// cross-section. So the two arms below are now identical BY CONSTRUCTION, and
/// the old assertion asserted a lever that no longer exists.
///
/// What replaces it is strictly stronger, so this stays a sentry rather than a
/// deletion. PR-6a's property — the fan is spaced off the TIP, not the Ø6
/// shank — used to hold only when the caller chose to pass the right scalar.
/// It now holds whatever the caller passes:
///
/// 1. both arms emit (non-vacuity first, as before);
/// 2. the two arms agree exactly — the caller's scalar no longer decides
///    measured spacing (the C9 change, named, so a revert to scalar spacing
///    makes this red);
/// 3. the emitted spacing is still finer than the envelope rule — measured
///    0.280 mm against the retired 1.500 mm — so "identical" cannot be
///    satisfied by both arms collapsing to the coarse behaviour, which is the
///    failure mode assertion 2 alone would not catch.
#[test]
fn the_per_point_fan_spaces_passes_off_the_tip_whatever_scalar_the_caller_passes() {
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::pencil::{PencilDetector, PencilParams, pencil_toolpath};
    use rs_cam_core::toolpath::MoveIntent;

    let finding = generate_through_session(tapered_ball_tool())
        .get_result(0)
        .expect("a generated result")
        .stats
        .derived_stepovers
        .first()
        .copied()
        .expect("the claims pipeline ran");

    let mesh = two_groove_plateau();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();

    // Distinct X positions (the grooves run along Y, so a fan spreads in X)
    // of cutting moves, bucketed at 0.02 mm.
    let lateral_positions = |stepover: f64| -> Vec<f64> {
        let params = PencilParams {
            detector: PencilDetector::RestDepth,
            offset_stepover: stepover,
            num_offset_passes: 4,
            ..PencilParams::default()
        };
        let tp = pencil_toolpath(&mesh, &index, &t, &params);
        let mut xs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::FinishingCut)
            .map(|m| (m.target.x / 0.02).round() * 0.02)
            .collect();
        xs.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        xs
    };

    let envelope_arm = lateral_positions(finding.envelope_rule_mm);
    let policy_arm = lateral_positions(finding.stepover_mm);

    let spread = |xs: &[f64]| -> f64 {
        match (xs.first(), xs.last()) {
            (Some(a), Some(b)) => b - a,
            _ => 0.0,
        }
    };
    let min_gap = |xs: &[f64]| -> f64 {
        xs.windows(2)
            .map(|w| w[1] - w[0])
            .fold(f64::INFINITY, f64::min)
    };
    println!(
        "PR-6a pass spacing: envelope arm {} distinct X, spread {:.3} mm, min gap {:.3} mm",
        envelope_arm.len(),
        spread(&envelope_arm),
        min_gap(&envelope_arm)
    );
    println!(
        "PR-6a pass spacing: policy   arm {} distinct X, spread {:.3} mm, min gap {:.3} mm",
        policy_arm.len(),
        spread(&policy_arm),
        min_gap(&policy_arm)
    );

    // 1. Non-vacuity FIRST: both arms must actually cut.
    assert!(
        !envelope_arm.is_empty() && !policy_arm.is_empty(),
        "both arms must emit cutting moves or the comparison is empty"
    );
    // 2. The C9 change: the caller's scalar no longer decides the spacing of a
    //    measured centreline's fan, so the two arms agree exactly.
    assert_eq!(
        envelope_arm, policy_arm,
        "the emitted fan still depends on the caller's `offset_stepover` \
         scalar — the per-point fan did not take effect"
    );
    // 3. …and they agree on the FINE spacing, not the coarse one. Without
    //    this, assertion 2 would be satisfied by a regression that put both
    //    arms back on the Ø6 shank.
    assert!(
        min_gap(&policy_arm) < finding.envelope_rule_mm,
        "the fan is spaced at {:.3} mm, no finer than the retired envelope \
         rule's {:.3} mm — the tip-scaled spacing is gone",
        min_gap(&policy_arm),
        finding.envelope_rule_mm
    );
}

// ── Gate 3: the ball control ────────────────────────────────────────────

/// A plain ball's cusp radius IS its envelope radius, so the migration is an
/// identity there — and the diagnostic stays silent, because a notice on
/// every toolpath is a notice nobody reads.
#[test]
fn the_ball_control_does_not_move_and_reports_nothing() {
    let ball = BallEndmill::new(BALL_DIAMETER_MM, 25.0);
    assert!(
        (ball.cusp_radius() - ball.radius()).abs() < 1e-12,
        "the premise of this control"
    );

    let session = generate_through_session(ball_tool());
    let stats = session
        .get_result(0)
        .expect("a generated result")
        .stats
        .clone();
    let finding =
        stats.derived_stepovers.first().copied().expect(
            "the claims pipeline ran on the ball too — 'not measured' would hide a regression",
        );
    println!(
        "PR-6a ball: derived {:.4} mm, envelope rule {:.4} mm",
        finding.stepover_mm, finding.envelope_rule_mm
    );
    assert!(
        finding.matches_the_envelope_rule(),
        "a ball must not move: {} vs {}",
        finding.stepover_mm,
        finding.envelope_rule_mm
    );
    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        ToolpathId(0),
        &stats,
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::CONFIG_DERIVED_STEPOVER),
        "nothing moved on a ball, so nothing may be reported"
    );
}
