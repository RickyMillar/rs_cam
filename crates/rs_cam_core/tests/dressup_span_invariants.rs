//! Cross-cutting span invariant tests for the dressup pipeline (Phase 5 / #46).
//!
//! After Phase 3 (sub-tasks #50–#58) every dressup is span-aware, and Phase 4
//! (#44) routes the GUI through the same core pipeline. This test asserts
//! that — regardless of which combination of dressups runs — the resulting
//! `AnnotatedToolpath` always satisfies [`AnnotatedToolpath::check_invariants`]:
//!
//! - All span ranges are within the toolpath's move count
//! - No span is inverted (start_move > end_move)
//! - `RapidOrderBarrier` spans are zero-width
//!
//! Plus the post-link-moves invariant from Phase 3d (#53), generalized here
//! at the `apply_dressups` boundary: a `LinkBridge` must never straddle a
//! `RapidOrderBarrier`.
//!
//! Per-op coverage of `apply_dressups` already lives in
//! `capability_link_moves_safety.rs` (10 ops with material-state assertions)
//! and in the `param_sweep` fixtures (54 sweeps × 22 operations); this file
//! adds the *invariant* assertion they don't make, on top of a synthetic
//! multi-pass span fixture that exercises every interesting dressup combo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_arguments
)]

use rs_cam_core::{
    compute::catalog::OperationType,
    compute::config::DressupConfig,
    compute::execute::apply_dressups,
    geo::P3,
    toolpath::Toolpath,
    toolpath_spans::{AnnotatedToolpath, Span, SpanKind},
};

// ── Helpers ──────────────────────────────────────────────────────────────

fn assert_invariants(out: &AnnotatedToolpath, label: &str) {
    out.check_invariants()
        .unwrap_or_else(|e| panic!("{label}: span invariant violated: {e}"));
}

fn assert_operation_span_tracks_moves(out: &AnnotatedToolpath, label: &str) {
    if !out.spans_valid {
        return;
    }
    let n = out.toolpath.moves.len();
    let op_span = out
        .spans
        .iter()
        .find(|s| s.kind == SpanKind::Operation)
        .unwrap_or_else(|| panic!("{label}: missing Operation span"));
    assert_eq!(
        op_span.start_move, 0,
        "{label}: Operation span should start at 0"
    );
    assert_eq!(
        op_span.end_move, n,
        "{label}: Operation span end_move {} should equal n_moves {}",
        op_span.end_move, n
    );
}

/// All meaningful dressup combos that exercise at least one move-mutating
/// step. Skips the "all off" combo since it's a no-op.
fn dressup_combos() -> Vec<(&'static str, DressupConfig)> {
    vec![
        (
            "link_moves",
            DressupConfig {
                link_moves: true,
                link_max_distance: 50.0,
                ..DressupConfig::default()
            },
        ),
        (
            "arc_fitting",
            DressupConfig {
                arc_fitting: true,
                arc_tolerance: 0.05,
                ..DressupConfig::default()
            },
        ),
        (
            "link_and_arc",
            DressupConfig {
                link_moves: true,
                link_max_distance: 50.0,
                arc_fitting: true,
                arc_tolerance: 0.05,
                ..DressupConfig::default()
            },
        ),
        (
            "rapid_order",
            DressupConfig {
                optimize_rapid_order: true,
                ..DressupConfig::default()
            },
        ),
        (
            "everything",
            DressupConfig {
                link_moves: true,
                link_max_distance: 50.0,
                arc_fitting: true,
                arc_tolerance: 0.05,
                optimize_rapid_order: true,
                ..DressupConfig::default()
            },
        ),
    ]
}

fn run_full_pipeline(
    annotated: AnnotatedToolpath,
    cfg: &DressupConfig,
    op: OperationType,
    tool_diameter: f64,
) -> AnnotatedToolpath {
    apply_dressups(
        annotated,
        cfg,
        tool_diameter,
        /* safe_z */ 30.0,
        None,
        None,
        None,
        op.transform_capabilities(),
        None,
        None,
    )
}

// ── Synthetic fixture ────────────────────────────────────────────────────

/// 3-pass synthetic toolpath: 3 cutting strokes at descending Z separated by
/// retract → reposition → plunge. Carries an Operation span, three DepthPass
/// spans (one per pass), and `RapidOrderBarrier` spans between passes.
fn synthetic_three_pass() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    let mut pass_starts = Vec::new();
    let z_levels = [-3.0, -6.0, -9.0];
    for (idx, &z) in z_levels.iter().enumerate() {
        if idx == 0 {
            tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        } else {
            tp.rapid_to(P3::new(0.0, 0.0, 10.0));
            tp.rapid_to(P3::new(2.0 + idx as f64, 0.0, 10.0));
        }
        pass_starts.push(tp.moves.len());
        tp.feed_to(P3::new(2.0 + idx as f64, 0.0, z), 500.0);
        tp.feed_to(P3::new(20.0 + idx as f64, 0.0, z), 1000.0);
        tp.rapid_to(P3::new(20.0 + idx as f64, 0.0, 10.0));
    }
    let n = tp.moves.len();
    let mut spans = vec![Span::new(0, n, SpanKind::Operation)];
    for (i, &start) in pass_starts.iter().enumerate() {
        let end = pass_starts.get(i + 1).copied().unwrap_or(n);
        spans.push(Span::new(start, end, SpanKind::DepthPass));
        if start > 0 {
            spans.push(Span::boundary(start, SpanKind::RapidOrderBarrier));
        }
    }
    AnnotatedToolpath::with_spans(tp, spans)
}

#[test]
fn synthetic_three_pass_preserves_invariants_across_all_combos() {
    let cap = OperationType::Adaptive3d.transform_capabilities();
    for (label, cfg) in dressup_combos() {
        let input = synthetic_three_pass();
        let n_in = input.toolpath.moves.len();
        let output = apply_dressups(input, &cfg, 6.0, 10.0, None, None, None, cap, None, None);
        assert_invariants(&output, label);
        assert_operation_span_tracks_moves(&output, label);
        assert!(
            output.toolpath.moves.len() <= n_in,
            "{label}: dressups should never increase move count for synthetic \
             toolpath (was {}, became {})",
            n_in,
            output.toolpath.moves.len()
        );
    }
}

#[test]
fn synthetic_three_pass_link_moves_never_straddles_barrier() {
    // Phase 3d / #53 invariant generalized to apply_dressups: even when
    // link_moves is enabled, no LinkBridge span may end up straddling a
    // depth-pass barrier in the output.
    let cap = OperationType::Adaptive3d.transform_capabilities();
    let input = synthetic_three_pass();
    let cfg = DressupConfig {
        link_moves: true,
        link_max_distance: 100.0,
        ..DressupConfig::default()
    };
    let output = apply_dressups(input, &cfg, 6.0, 10.0, None, None, None, cap, None, None);
    assert_invariants(&output, "link_moves_barrier_check");
    if !output.spans_valid {
        return;
    }
    let barriers: Vec<usize> = output
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::RapidOrderBarrier)
        .map(|s| s.start_move)
        .collect();
    for bridge in output
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LinkBridge)
    {
        for &b in &barriers {
            assert!(
                bridge.end_move <= b || bridge.start_move >= b,
                "LinkBridge {:?} straddles barrier at {}",
                bridge.range(),
                b
            );
        }
    }
}

#[test]
fn synthetic_with_invalid_input_spans_stays_invalid() {
    // If the input is flagged spans_valid=false, the pipeline must not
    // suddenly claim the output is valid. (It may still succeed structurally —
    // check_invariants just verifies shape, not freshness.)
    let cap = OperationType::Adaptive3d.transform_capabilities();
    let mut input = synthetic_three_pass();
    input.spans_valid = false;
    let cfg = DressupConfig {
        link_moves: true,
        link_max_distance: 100.0,
        arc_fitting: true,
        arc_tolerance: 0.05,
        ..DressupConfig::default()
    };
    let output = apply_dressups(input, &cfg, 6.0, 10.0, None, None, None, cap, None, None);
    assert_invariants(&output, "invalid_input_passthrough");
    assert!(
        !output.spans_valid,
        "spans_valid=false on input must propagate to output"
    );
}

// ── Per-op coverage ──────────────────────────────────────────────────────

#[test]
fn face_op_dressup_pipeline_preserves_invariants() {
    use rs_cam_core::face::{FaceParams, face_toolpath};
    use rs_cam_core::geo::BoundingBox3;

    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 30.0, 5.0),
    };
    let params = FaceParams {
        tool_radius: 3.0,
        stepover: 4.0,
        depth: 2.0,
        depth_per_pass: 2.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        stock_offset: 0.0,
        direction: rs_cam_core::face::FaceDirection::OneWay,
    };
    let raw = face_toolpath(&bbox, &params);
    assert!(!raw.moves.is_empty(), "face fixture should produce moves");
    let n = raw.moves.len();
    let annotated = AnnotatedToolpath::with_spans(raw, vec![Span::new(0, n, SpanKind::Operation)]);
    for (label, cfg) in dressup_combos() {
        let output = run_full_pipeline(annotated.clone(), &cfg, OperationType::Face, 6.0);
        let scope = format!("face+{label}");
        assert_invariants(&output, &scope);
        assert_operation_span_tracks_moves(&output, &scope);
    }
}
