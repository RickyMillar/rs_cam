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
    compute::config::{
        ArcFitParams, DogboneParams, DressupConfig, DressupEntryStyle, LeadParams,
        LinkDressupParams, SegmentMergeParams,
    },
    compute::execute::{DRESSUP_PIPELINE, apply_dressups},
    geo::P3,
    toolpath::{MoveType, Toolpath},
    trace::debug_trace::ToolpathDebugRecorder,
    trace::toolpath_spans::{AnnotatedToolpath, Span, SpanKind},
    trace::transform_provenance::ReconcileSet,
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
                link_moves: Some(LinkDressupParams {
                    max_distance: 50.0,
                    ..LinkDressupParams::default()
                }),
                ..DressupConfig::default()
            },
        ),
        (
            "arc_fitting",
            DressupConfig {
                arc_fitting: Some(ArcFitParams { tolerance: 0.05 }),
                ..DressupConfig::default()
            },
        ),
        (
            "link_and_arc",
            DressupConfig {
                link_moves: Some(LinkDressupParams {
                    max_distance: 50.0,
                    ..LinkDressupParams::default()
                }),
                arc_fitting: Some(ArcFitParams { tolerance: 0.05 }),
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
                link_moves: Some(LinkDressupParams {
                    max_distance: 50.0,
                    ..LinkDressupParams::default()
                }),
                arc_fitting: Some(ArcFitParams { tolerance: 0.05 }),
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
        rs_cam_core::compute::execute::DressupContext {
            cfg,
            nominal_feed_rate: 1000.0,
            plunge_rate_mm_min: None,
            tool_diameter,
            safe_z: /* safe_z */ 30.0,
            stock_top: /* stock_top */ 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: op.transform_capabilities(),
            debug_ctx: None,
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
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
        let output = apply_dressups(
            input,
            rs_cam_core::compute::execute::DressupContext {
                cfg: &cfg,
                nominal_feed_rate: 1000.0,
                plunge_rate_mm_min: None,
                tool_diameter: 6.0,
                safe_z: 10.0,
                stock_top: 0.0,
                prior_stock: None,
                feed_opt_stock: None,
                cutter: None,
                entry_surface: None,
                transform_capabilities: cap,
                debug_ctx: None,
                semantic_ctx: None,
            },
            &mut ReconcileSet::empty(),
        );
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
        link_moves: Some(LinkDressupParams {
            max_distance: 100.0,
            ..LinkDressupParams::default()
        }),
        ..DressupConfig::default()
    };
    let output = apply_dressups(
        input,
        rs_cam_core::compute::execute::DressupContext {
            cfg: &cfg,
            nominal_feed_rate: 1000.0,
            plunge_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: 10.0,
            stock_top: 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: cap,
            debug_ctx: None,
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    );
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
        link_moves: Some(LinkDressupParams {
            max_distance: 100.0,
            ..LinkDressupParams::default()
        }),
        arc_fitting: Some(ArcFitParams { tolerance: 0.05 }),
        ..DressupConfig::default()
    };
    let output = apply_dressups(
        input,
        rs_cam_core::compute::execute::DressupContext {
            cfg: &cfg,
            nominal_feed_rate: 1000.0,
            plunge_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: 10.0,
            stock_top: 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: cap,
            debug_ctx: None,
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    );
    assert_invariants(&output, "invalid_input_passthrough");
    assert!(
        !output.spans_valid,
        "spans_valid=false on input must propagate to output"
    );
}

// ── Per-op coverage ──────────────────────────────────────────────────────

#[test]
fn face_op_dressup_pipeline_preserves_invariants() {
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::ops::face::{FaceParams, face_toolpath};

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
        direction: rs_cam_core::ops::face::FaceDirection::OneWay,
        stock_top_z: 0.0,
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

#[test]
fn one_way_face_vetoes_rapid_order_but_runs_other_dressups() {
    use rs_cam_core::{
        compute::{config::ArcFitParams, spans::spans_from_depth_runs},
        geo::BoundingBox3,
        ops::face::{FaceDirection, FaceParams, face_toolpath},
    };

    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(40.0, 30.0, 5.0),
    };
    let raw = face_toolpath(
        &bbox,
        &FaceParams {
            tool_radius: 3.0,
            stepover: 4.0,
            depth: 4.0,
            depth_per_pass: 2.0,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_offset: 0.0,
            direction: FaceDirection::OneWay,
            stock_top_z: 0.0,
        },
    );
    let rows_before: Vec<_> = raw
        .moves
        .iter()
        .filter(|mv| matches!(mv.move_type, MoveType::Linear { .. }) && mv.target.z < 0.0)
        .map(|mv| (mv.target.x, mv.target.y, mv.target.z))
        .collect();
    let mut row_ys: Vec<_> = rows_before.iter().map(|&(_, y, _)| y).collect();
    row_ys.sort_by(f64::total_cmp);
    row_ys.dedup();
    assert!(
        row_ys.len() > 1,
        "Face fixture must contain multiple distinct row Y values"
    );
    let mut depths: Vec<_> = rows_before.iter().map(|&(_, _, z)| z).collect();
    depths.sort_by(f64::total_cmp);
    depths.dedup();
    assert!(
        depths.len() > 1,
        "Face fixture must contain multiple distinct depth levels"
    );
    let spans = spans_from_depth_runs(&raw, &[]);
    let annotated = AnnotatedToolpath::with_spans(raw, spans);
    assert!(
        !annotated.rapid_order_barriers().is_empty(),
        "the real depth-stepped Face fixture must exercise the barriered gate"
    );

    let recorder = ToolpathDebugRecorder::new("Face rapid-order veto", "Face");
    let root = recorder.root_context();
    let cfg = DressupConfig {
        arc_fitting: Some(ArcFitParams::default()),
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let output = apply_dressups(
        annotated,
        rs_cam_core::compute::execute::DressupContext {
            cfg: &cfg,
            nominal_feed_rate: 1500.0,
            plunge_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: 30.0,
            stock_top: 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::Face.transform_capabilities(),
            debug_ctx: Some(&root),
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    );
    let rows_after: Vec<_> = output
        .toolpath
        .moves
        .iter()
        .filter(|mv| matches!(mv.move_type, MoveType::Linear { .. }) && mv.target.z < 0.0)
        .map(|mv| (mv.target.x, mv.target.y, mv.target.z))
        .collect();
    assert_eq!(rows_after, rows_before, "Face cut row order changed");

    let trace = recorder.finish();
    assert!(trace.spans.iter().all(|span| span.kind != "rapid_order"));
    assert!(
        trace.spans.iter().any(|span| span.kind == "arc_fit"),
        "the enabled non-order dressup must still run"
    );
}

// ── CMP-20 / CUT-06: the pipeline is a list, and its order is pinned ─────

/// A run emits a SUBSEQUENCE of [`DRESSUP_PIPELINE`], in order.
///
/// Every stage used to write its four trace strings (`debug_key`,
/// `debug_label`, `kind`, `semantic_label`) as a literal at its own call
/// site, and those strings existed nowhere else. Nothing could enumerate
/// the pipeline, so nothing checked that the order the code runs matches
/// the order the docs claim. The stages are named constants now and
/// `DRESSUP_PIPELINE` lists them in run order.
///
/// ORDER IS LOAD-BEARING: segment merge runs after arc fitting, and the
/// unbarriered rapid-order pass runs after link moves. A stage that moved
/// would emit its key out of order and fail here.
///
/// Non-vacuity: the run must emit at least four stages, and every key it
/// emits must appear in the list.
#[test]
fn dressup_stages_run_in_the_order_the_pipeline_lists() {
    let recorder = ToolpathDebugRecorder::new("Stage order", "Pocket");
    let root = recorder.root_context();
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 5.0,
        dogbone: Some(DogboneParams { angle: 90.0 }),
        lead_in_out: Some(LeadParams {
            radius: 1.0,
            ..LeadParams::default()
        }),
        link_moves: Some(LinkDressupParams {
            max_distance: 50.0,
            ..LinkDressupParams::default()
        }),
        arc_fitting: Some(ArcFitParams { tolerance: 0.05 }),
        segment_merge: Some(SegmentMergeParams::default()),
        optimize_rapid_order: true,
        ..DressupConfig::default()
    };
    let _ = apply_dressups(
        synthetic_three_pass(),
        rs_cam_core::compute::execute::DressupContext {
            cfg: &cfg,
            nominal_feed_rate: 1000.0,
            plunge_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: 30.0,
            stock_top: 0.0,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::Pocket.transform_capabilities(),
            debug_ctx: Some(&root),
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    );
    let trace = recorder.finish();
    let emitted: Vec<String> = trace.spans.iter().map(|s| s.kind.clone()).collect();

    assert!(
        emitted.len() >= 4,
        "the run emitted only {} stage(s) ({emitted:?}); a tiny population \
         passes this test and checks nothing",
        emitted.len()
    );

    // Walk the pipeline once, consuming a row per emitted key. A key that
    // cannot be found at or after the cursor is either unknown or out of
    // order.
    let mut cursor = 0usize;
    for key in &emitted {
        let found = DRESSUP_PIPELINE[cursor..]
            .iter()
            .position(|stage| stage.debug_key == key);
        let offset = found.unwrap_or_else(|| {
            panic!(
                "dressup stage {key:?} is not in DRESSUP_PIPELINE at or after \
                 position {cursor}. Emitted order: {emitted:?}. Pipeline: {:?}",
                DRESSUP_PIPELINE
                    .iter()
                    .map(|s| s.debug_key)
                    .collect::<Vec<_>>()
            )
        });
        cursor += offset + 1;
    }
}

/// Every stage constant names a distinct `debug_label`.
///
/// `debug_key` is deliberately NOT unique — the two rapid-order passes and
/// the two entry styles share one key each — so the label is what tells two
/// rows apart in a trace. Two rows with the same label would be
/// indistinguishable on screen.
#[test]
fn dressup_stage_labels_are_distinct() {
    let mut labels: Vec<&str> = DRESSUP_PIPELINE.iter().map(|s| s.debug_label).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(
        labels.len() + 1,
        before,
        "DRESSUP_PIPELINE lists {before} rows with {} distinct labels; only \
         the two rapid-order passes may share one",
        labels.len()
    );
}
