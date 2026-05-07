//! Phase 3i / #58: `apply_boundary_clip` accepts an `AnnotatedToolpath` and
//! always emits `spans_valid = false` until a precise span remap is built.
//!
//! This test calls the function directly with a small synthesized toolpath
//! and a unit rectangle stock bbox, and asserts the invalidation contract.

use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::semantic_trace::ToolpathSemanticRecorder;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

#[test]
fn boundary_clip_invalidates_spans() {
    // Build a 10×10 stock bbox so the default rectangle boundary fully
    // contains a small interior toolpath — the actual clip is a no-op on
    // moves, but it still must invalidate the spans.
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
        2.0,  // tool diameter
        20.0, // safe Z
        &semantic_ctx,
    );

    // Phase 3i contract: boundary clip always invalidates spans on output.
    assert!(
        !clipped.spans_valid,
        "apply_boundary_clip must set spans_valid = false (precise remap deferred)",
    );
    // The spans payload itself is preserved (we only flip the flag) so callers
    // that want to inspect what was invalidated still can.
    assert_eq!(
        clipped.spans.len(),
        2,
        "spans vector should be carried through (only the validity flag flips)",
    );
}

#[test]
fn boundary_clip_with_no_input_spans_still_invalidates() {
    // Empty spans → no warning is emitted, but the output flag is still
    // documented as `false` to match the contract.
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

    assert!(!clipped.spans_valid);
    assert!(clipped.spans.is_empty());
}
