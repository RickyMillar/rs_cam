//! S83: `apply_boundary_clip` precisely remaps spans through the clip
//! using the per-input-move provenance map from
//! `clip_toolpath_to_boundary_with_provenance`.
//!
//! These tests call the function directly with a small synthesized toolpath
//! and assert the new contract: spans pass through with `spans_valid = true`
//! and their move ranges remapped to the post-clip output indices.

#![allow(clippy::indexing_slicing)]

use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::semantic_trace::ToolpathSemanticRecorder;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

#[test]
fn boundary_clip_preserves_spans_when_all_moves_inside() {
    // 10×10 stock with a fully-inside toolpath. The clipper passes every
    // input move through 1:1, so spans must remap to identical ranges.
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -10.0),
        max: P3::new(10.0, 10.0, 0.0),
    };

    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(2.0, 2.0, 5.0));
    tp.feed_to(P3::new(2.0, 2.0, -1.0), 500.0);
    tp.feed_to(P3::new(8.0, 2.0, -1.0), 1000.0);
    tp.feed_to(P3::new(8.0, 8.0, -1.0), 1000.0);
    tp.feed_to(P3::new(2.0, 8.0, -1.0), 1000.0);
    let n_moves = tp.moves.len();

    let spans = vec![
        Span::new(0, n_moves, SpanKind::Operation).with_label("op"),
        Span::new(1, n_moves, SpanKind::DepthPass).with_label("pass-0"),
    ];
    let annotated = AnnotatedToolpath::with_spans(tp, spans);

    let boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };

    let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
    let semantic_ctx = recorder.root_context();

    let clipped = ProjectSession::apply_boundary_clip(
        annotated,
        &boundary,
        &stock_bbox,
        None,
        &[],
        2.0,
        20.0,
        &semantic_ctx,
    );

    assert!(
        clipped.spans_valid,
        "apply_boundary_clip must keep spans_valid = true after S83 precise remap",
    );
    assert_eq!(clipped.spans.len(), 2, "spans vector preserved");
    assert_eq!(clipped.spans[0].kind, SpanKind::Operation);
    assert_eq!(clipped.spans[0].start_move, 0);
    assert_eq!(clipped.spans[0].end_move, clipped.toolpath.moves.len());
    assert_eq!(clipped.spans[1].kind, SpanKind::DepthPass);
    // Pass span starts at input move 1; with a 1:1 mapping that's still 1.
    assert_eq!(clipped.spans[1].start_move, 1);
}

#[test]
fn boundary_clip_with_no_input_spans_emits_no_spans() {
    // No input spans → no output spans, regardless of clip behaviour.
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -10.0),
        max: P3::new(10.0, 10.0, 0.0),
    };

    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 5.0, 5.0));
    tp.feed_to(P3::new(5.0, 5.0, -1.0), 500.0);

    let annotated = AnnotatedToolpath::new(tp);

    let boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };

    let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
    let semantic_ctx = recorder.root_context();

    let clipped = ProjectSession::apply_boundary_clip(
        annotated,
        &boundary,
        &stock_bbox,
        None,
        &[],
        2.0,
        20.0,
        &semantic_ctx,
    );

    assert!(clipped.spans_valid);
    assert!(clipped.spans.is_empty());
}
