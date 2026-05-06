//! Build [`Span`]s from per-operation runtime annotations.
//!
//! This is the bridge between the legacy op-specific
//! [`OperationAnnotations`](crate::compute::execute::OperationAnnotations)
//! enum and the generic span model in [`crate::toolpath_spans`]. Each
//! generator still returns its own `RuntimeAnnotation` events for
//! narrate / debug / GUI consumers; this module translates the subset
//! that is structurally meaningful (depth-pass and region boundaries,
//! TSP barriers) into [`Span`]s for the dressup pipeline.

use crate::compute::execute::OperationAnnotations;
use crate::toolpath_spans::{Span, SpanKind, SpanPayload};

/// Build the span vector for an operation's freshly-generated toolpath.
///
/// Always emits a top-level `Operation` span covering `[0, n_moves)`.
/// For operations whose annotations carry structural information (today:
/// only Adaptive3d), additional `Region`, `DepthPass`, and
/// `RapidOrderBarrier` spans are appended. Other ops fall back to just
/// the top-level span until their phase-3 wiring is added.
///
/// The set of spans here must produce barriers via
/// [`crate::toolpath_spans::AnnotatedToolpath::rapid_order_barriers`]
/// that exactly match the legacy
/// [`OperationAnnotations::rapid_order_barriers`] — the
/// `adaptive3d_post_tsp_z_monotonicity` regression test will fail if
/// barrier derivation drifts.
pub fn spans_from_annotations(annotations: &OperationAnnotations, n_moves: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();

    // Top-level Operation span. Empty toolpaths are still wrapped — the
    // span is just zero-width.
    spans.push(Span::new(0, n_moves, SpanKind::Operation));

    match annotations {
        OperationAnnotations::None
        | OperationAnnotations::Adaptive2d(_)
        | OperationAnnotations::Scallop(_)
        | OperationAnnotations::RampFinish(_)
        | OperationAnnotations::SpiralFinish(_)
        | OperationAnnotations::Pencil(_) => {
            // Phase-2 scope: only adaptive3d emits structural spans.
            // The other annotation types carry debug labels that don't
            // map to barriers / depth-passes; their span derivation is
            // a follow-up.
        }
        OperationAnnotations::Adaptive3d(events) => {
            push_adaptive3d_spans(&mut spans, events, n_moves);
        }
    }

    spans
}

/// Translate Adaptive3d runtime events into structural spans.
///
/// - `RegionZLevel | GlobalZLevel | WaterlineCleanup`: zero-width
///   `RapidOrderBarrier` spans at the move index. Match the legacy
///   `OperationAnnotations::rapid_order_barriers()` exactly.
/// - Consecutive `RegionZLevel` / `GlobalZLevel` / `WaterlineCleanup`
///   events also delimit `DepthPass` spans (each pass runs from the
///   event's `move_index` to the next event of the same family, or to
///   `n_moves`).
/// - `RegionStart` events delimit `Region` spans (start at the event's
///   `move_index`, end at the next `RegionStart` or `n_moves`).
fn push_adaptive3d_spans(
    spans: &mut Vec<Span>,
    events: &[crate::adaptive3d::Adaptive3dRuntimeAnnotation],
    n_moves: usize,
) {
    use crate::adaptive3d::Adaptive3dRuntimeEvent as E;

    // Pass 1: barriers + depth-pass starts (in move-index order).
    let depth_starts: Vec<(usize, Option<f64>, Option<u32>)> = events
        .iter()
        .filter_map(|a| match &a.event {
            E::RegionZLevel {
                z_level,
                level_index,
                ..
            } => Some((a.move_index, Some(*z_level), Some(*level_index as u32))),
            E::GlobalZLevel {
                z_level,
                level_index,
                ..
            } => Some((a.move_index, Some(*z_level), Some(*level_index as u32))),
            E::WaterlineCleanup => Some((a.move_index, None, None)),
            E::RegionStart { .. }
            | E::PassEntry { .. }
            | E::PassPreflightSkip { .. }
            | E::PassSummary { .. } => None,
        })
        .collect();

    // Emit RapidOrderBarrier spans (one per depth-pass start).
    for (idx, _, _) in &depth_starts {
        spans.push(Span::boundary(*idx, SpanKind::RapidOrderBarrier));
    }

    // Emit DepthPass spans. Each pass runs from its start move to the
    // next pass start, or to n_moves for the last one.
    for (i, (start, z, level_index)) in depth_starts.iter().enumerate() {
        let end = depth_starts
            .get(i + 1)
            .map(|next| next.0)
            .unwrap_or(n_moves);
        if end <= *start {
            continue;
        }
        let mut span = Span::new(*start, end, SpanKind::DepthPass);
        if let (Some(z), Some(idx)) = (*z, *level_index) {
            span = span.with_payload(SpanPayload::DepthPass {
                z_level: z,
                pass_index: idx,
            });
        }
        spans.push(span);
    }

    // Pass 2: regions.
    let region_starts: Vec<(usize, u32)> = events
        .iter()
        .filter_map(|a| match &a.event {
            E::RegionStart { region_index, .. } => Some((a.move_index, *region_index as u32)),
            _ => None,
        })
        .collect();
    for (i, (start, region_id)) in region_starts.iter().enumerate() {
        let end = region_starts
            .get(i + 1)
            .map(|next| next.0)
            .unwrap_or(n_moves);
        if end <= *start {
            continue;
        }
        spans.push(
            Span::new(*start, end, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: *region_id,
            }),
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::adaptive3d::{
        Adaptive3dRuntimeAnnotation, Adaptive3dRuntimeEvent, ZLevelPlanMetrics,
    };
    use crate::toolpath::Toolpath;
    use crate::toolpath_spans::AnnotatedToolpath;

    fn metrics() -> ZLevelPlanMetrics {
        ZLevelPlanMetrics::default()
    }

    fn ev(move_index: usize, event: Adaptive3dRuntimeEvent) -> Adaptive3dRuntimeAnnotation {
        Adaptive3dRuntimeAnnotation { move_index, event }
    }

    #[test]
    fn none_annotations_yields_only_operation_span() {
        let spans = spans_from_annotations(&OperationAnnotations::None, 42);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, SpanKind::Operation);
        assert_eq!(spans[0].start_move, 0);
        assert_eq!(spans[0].end_move, 42);
    }

    #[test]
    fn empty_toolpath_still_gets_operation_span() {
        let spans = spans_from_annotations(&OperationAnnotations::None, 0);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].is_boundary());
    }

    #[test]
    fn adaptive3d_barriers_match_legacy_rapid_order_barriers() {
        // Build a representative adaptive3d annotation stream.
        let events = vec![
            ev(
                0,
                Adaptive3dRuntimeEvent::RegionStart {
                    region_index: 0,
                    region_total: 1,
                    cell_count: 100,
                },
            ),
            ev(
                0,
                Adaptive3dRuntimeEvent::RegionZLevel {
                    region_index: 0,
                    z_level: 10.0,
                    level_index: 0,
                    level_total: 3,
                    metrics: metrics(),
                },
            ),
            ev(
                50,
                Adaptive3dRuntimeEvent::RegionZLevel {
                    region_index: 0,
                    z_level: 5.0,
                    level_index: 1,
                    level_total: 3,
                    metrics: metrics(),
                },
            ),
            ev(100, Adaptive3dRuntimeEvent::WaterlineCleanup),
            ev(
                120,
                Adaptive3dRuntimeEvent::PassSummary {
                    pass_index: 0,
                    step_count: 50,
                    exit_reason: "ok".into(),
                    yield_ratio: 0.9,
                    short: false,
                },
            ),
        ];
        let annotations = OperationAnnotations::Adaptive3d(events);

        // Legacy method.
        let legacy = annotations.rapid_order_barriers();

        // New method via spans.
        let spans = spans_from_annotations(&annotations, 150);
        let mut tp = Toolpath::new();
        for _ in 0..150 {
            tp.feed_to(crate::geo::P3::new(0.0, 0.0, 0.0), 1000.0);
        }
        let derived = AnnotatedToolpath::with_spans(tp, spans).rapid_order_barriers();

        assert_eq!(derived, legacy, "span-derived barriers must match legacy");
        assert_eq!(derived, vec![0, 50, 100]);
    }

    #[test]
    fn adaptive3d_emits_depth_pass_spans_with_payload() {
        let events = vec![
            ev(
                0,
                Adaptive3dRuntimeEvent::RegionZLevel {
                    region_index: 0,
                    z_level: 10.0,
                    level_index: 0,
                    level_total: 2,
                    metrics: metrics(),
                },
            ),
            ev(
                30,
                Adaptive3dRuntimeEvent::RegionZLevel {
                    region_index: 0,
                    z_level: 5.0,
                    level_index: 1,
                    level_total: 2,
                    metrics: metrics(),
                },
            ),
        ];
        let spans = spans_from_annotations(&OperationAnnotations::Adaptive3d(events), 60);
        let depth: Vec<_> = spans
            .iter()
            .filter(|s| s.kind == SpanKind::DepthPass)
            .collect();
        assert_eq!(depth.len(), 2);
        assert_eq!(depth[0].start_move, 0);
        assert_eq!(depth[0].end_move, 30);
        assert!(matches!(
            depth[0].payload,
            Some(SpanPayload::DepthPass { z_level, pass_index: 0 }) if (z_level - 10.0).abs() < 1e-9
        ));
        assert_eq!(depth[1].start_move, 30);
        assert_eq!(depth[1].end_move, 60);
    }

    #[test]
    fn adaptive3d_emits_region_spans() {
        let events = vec![
            ev(
                0,
                Adaptive3dRuntimeEvent::RegionStart {
                    region_index: 0,
                    region_total: 2,
                    cell_count: 50,
                },
            ),
            ev(
                40,
                Adaptive3dRuntimeEvent::RegionStart {
                    region_index: 1,
                    region_total: 2,
                    cell_count: 50,
                },
            ),
        ];
        let spans = spans_from_annotations(&OperationAnnotations::Adaptive3d(events), 100);
        let regions: Vec<_> = spans
            .iter()
            .filter(|s| s.kind == SpanKind::Region)
            .collect();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].range(), 0..40);
        assert_eq!(regions[1].range(), 40..100);
    }
}
