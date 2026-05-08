//! Build [`Span`]s from operation runtime events.
//!
//! This is the bridge between op-specific `RuntimeAnnotation` events and the
//! generic span model in [`crate::toolpath_spans`]. Each generator can still
//! return its own events for narration/debug/semantic trace construction; this
//! module translates the subset that is structurally meaningful into [`Span`]s
//! for the dressup pipeline.

use crate::toolpath::{MoveType, Toolpath};
use crate::toolpath_spans::{Span, SpanKind, SpanPayload};

/// Build the default span vector for an operation's freshly-generated toolpath.
///
/// Always emits a top-level `Operation` span covering `[0, n_moves)`. Empty
/// toolpaths are still wrapped — the span is just zero-width.
pub fn operation_spans(n_moves: usize) -> Vec<Span> {
    vec![Span::new(0, n_moves, SpanKind::Operation)]
}

/// Build structural spans from a sequence of operation-specific runtime
/// annotations represented as `(move_index, label)` pairs.
///
/// Each annotation opens a `Region` span that extends to the next annotation
/// or to the end of the toolpath. This keeps annotation-aware finish
/// operations navigable in simulation without requiring a new `SpanKind` for
/// every operation family.
pub fn spans_from_labeled_events<I>(n_moves: usize, events: I) -> Vec<Span>
where
    I: IntoIterator<Item = (usize, String)>,
{
    let mut spans = operation_spans(n_moves);
    let mut starts: Vec<(usize, String)> = events
        .into_iter()
        .filter(|(move_index, _)| *move_index < n_moves)
        .collect();
    starts.sort_by(|left, right| left.0.cmp(&right.0));

    for (event_index, (start, label)) in starts.iter().enumerate() {
        let end = starts
            .get(event_index + 1)
            .map(|next| next.0)
            .unwrap_or(n_moves);
        if end <= *start {
            continue;
        }
        spans.push(
            Span::new(*start, end, SpanKind::Region)
                .with_label(label.clone())
                .with_payload(SpanPayload::Region {
                    region_id: event_index as u32,
                }),
        );
    }

    spans
}

/// Build structural spans from a generated toolpath by inspecting move
/// structure.
///
/// This is the generic fallback used by operations that do not yet emit
/// operation-specific runtime annotations. It always emits `Operation`, then
/// one `DepthPass` span for each contiguous depth section, and one `Region`
/// span for every contiguous cutting run. When `levels` is non-empty, cutting
/// runs are associated with the nearest configured level; otherwise their
/// deepest cutting Z is used as the inferred level.
///
/// The emitted `DepthPass` starts also become rapid-order barriers through
/// [`crate::toolpath_spans::AnnotatedToolpath::rapid_order_barriers`], so use
/// this only for operations where depth order matters.
pub fn spans_from_depth_runs(toolpath: &Toolpath, levels: &[f64]) -> Vec<Span> {
    let runs = cutting_runs(toolpath);
    if runs.is_empty() {
        return operation_spans(toolpath.moves.len());
    }

    let mut spans = operation_spans(toolpath.moves.len());
    let mut depth_sections: Vec<DepthSection> = Vec::new();

    for run in &runs {
        let z = run_nominal_z(toolpath, run).unwrap_or(run.z_min);
        let level_z = nearest_level(z, levels).unwrap_or(z);
        let same_as_current = depth_sections
            .last()
            .is_some_and(|section| approx_eq(section.z_level, level_z));
        if same_as_current {
            if let Some(section) = depth_sections.last_mut() {
                section.end_move = section.end_move.max(run.end_move);
            }
        } else {
            depth_sections.push(DepthSection {
                start_move: run.start_move,
                end_move: run.end_move,
                z_level: level_z,
            });
        }
    }

    for section in &depth_sections {
        spans.push(Span::boundary(
            section.start_move,
            SpanKind::RapidOrderBarrier,
        ));
    }
    for (pass_index, section) in depth_sections.iter().enumerate() {
        if section.end_move <= section.start_move {
            continue;
        }
        spans.push(
            Span::new(section.start_move, section.end_move, SpanKind::DepthPass)
                .with_label(format!("Depth pass {}", pass_index + 1))
                .with_payload(SpanPayload::DepthPass {
                    z_level: section.z_level,
                    pass_index: pass_index as u32,
                }),
        );
    }

    push_run_region_spans(&mut spans, &runs, "Run");
    spans
}

/// Build structural spans for operations whose useful structure is just a set
/// of contiguous cutting runs (rows, rays, contours, projected chains, etc.).
///
/// This intentionally avoids `DepthPass` spans so it does not introduce
/// rapid-order barriers for operations where global segment reordering is safe.
pub fn spans_from_cutting_runs(toolpath: &Toolpath, label_prefix: &str) -> Vec<Span> {
    let runs = cutting_runs(toolpath);
    let mut spans = operation_spans(toolpath.moves.len());
    push_run_region_spans(&mut spans, &runs, label_prefix);
    spans
}

/// Build structural spans for drill-like operations.
///
/// Holes and individual feed plunges are represented as labeled `Region`
/// spans rather than `DepthPass` spans. This keeps per-hole global rapid-order
/// optimization safe: `DepthPass` spans double as TSP barriers, while drill
/// pecks are local to a hole and should not prevent hole order optimization.
pub fn spans_from_drill_holes(toolpath: &Toolpath) -> Vec<Span> {
    let mut spans = operation_spans(toolpath.moves.len());
    let holes = drill_hole_sections(toolpath);
    let mut plunge_region_id = holes.len() as u32;

    for (hole_index, hole) in holes.iter().enumerate() {
        spans.push(
            Span::new(hole.start_move, hole.end_move, SpanKind::Region)
                .with_label(format!("Hole {}", hole_index + 1))
                .with_payload(SpanPayload::Region {
                    region_id: hole_index as u32,
                }),
        );

        for (peck_index, plunge) in hole.plunges.iter().enumerate() {
            spans.push(
                Span::new(plunge.start_move, plunge.end_move, SpanKind::Region)
                    .with_label(format!("Hole {} plunge {}", hole_index + 1, peck_index + 1))
                    .with_payload(SpanPayload::Region {
                        region_id: plunge_region_id,
                    }),
            );
            plunge_region_id = plunge_region_id.saturating_add(1);
        }
    }

    spans
}

#[derive(Debug, Clone, Copy)]
struct CutRun {
    start_move: usize,
    end_move: usize,
    z_min: f64,
}

#[derive(Debug, Clone, Copy)]
struct DepthSection {
    start_move: usize,
    end_move: usize,
    z_level: f64,
}

#[derive(Debug, Clone)]
struct DrillHoleSection {
    start_move: usize,
    end_move: usize,
    plunges: Vec<DrillPlungeSection>,
}

#[derive(Debug, Clone, Copy)]
struct DrillPlungeSection {
    start_move: usize,
    end_move: usize,
}

fn push_run_region_spans(spans: &mut Vec<Span>, runs: &[CutRun], label_prefix: &str) {
    for (run_index, run) in runs.iter().enumerate() {
        if run.end_move <= run.start_move {
            continue;
        }
        spans.push(
            Span::new(run.start_move, run.end_move, SpanKind::Region)
                .with_label(format!("{label_prefix} {}", run_index + 1))
                .with_payload(SpanPayload::Region {
                    region_id: run_index as u32,
                }),
        );
    }
}

fn cutting_runs(toolpath: &Toolpath) -> Vec<CutRun> {
    let mut runs = Vec::new();
    let mut active_start: Option<usize> = None;
    let mut z_min = f64::INFINITY;

    for (move_index, mv) in toolpath.moves.iter().enumerate() {
        let is_cut = is_cutting_move(&mv.move_type);
        if is_cut {
            if active_start.is_none() {
                active_start = Some(move_index.saturating_sub(1));
                z_min = f64::INFINITY;
            }
            z_min = z_min.min(mv.target.z);
        }

        let next_is_cut = toolpath
            .moves
            .get(move_index + 1)
            .is_some_and(|next| is_cutting_move(&next.move_type));
        if active_start.is_some() && is_cut && !next_is_cut {
            let start = active_start.take().unwrap_or(0);
            let end = (move_index + 1).min(toolpath.moves.len());
            runs.push(CutRun {
                start_move: start,
                end_move: end,
                z_min,
            });
        }
    }

    runs
}

fn run_nominal_z(toolpath: &Toolpath, run: &CutRun) -> Option<f64> {
    toolpath
        .moves
        .iter()
        .enumerate()
        .skip(run.start_move)
        .take(run.end_move.saturating_sub(run.start_move))
        .filter(|(_, mv)| is_cutting_move(&mv.move_type))
        .map(|(_, mv)| mv.target.z)
        .min_by(f64::total_cmp)
}

fn nearest_level(z: f64, levels: &[f64]) -> Option<f64> {
    levels
        .iter()
        .copied()
        .min_by(|left, right| (z - *left).abs().total_cmp(&(z - *right).abs()))
}

fn is_cutting_move(move_type: &MoveType) -> bool {
    matches!(
        move_type,
        MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
    )
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-6
}

fn drill_hole_sections(toolpath: &Toolpath) -> Vec<DrillHoleSection> {
    let mut holes = Vec::new();
    let mut i = 0;
    while i < toolpath.moves.len() {
        let Some(first) = toolpath.moves.get(i) else {
            break;
        };
        let x = first.target.x;
        let y = first.target.y;
        let start = i;
        let mut end = i + 1;
        let mut plunges = Vec::new();

        while let Some(mv) = toolpath.moves.get(end) {
            if !same_xy(mv.target.x, mv.target.y, x, y) {
                break;
            }
            end += 1;
        }

        for move_index in start..end {
            if let Some(mv) = toolpath.moves.get(move_index)
                && matches!(mv.move_type, MoveType::Linear { .. })
            {
                plunges.push(DrillPlungeSection {
                    start_move: move_index,
                    end_move: move_index + 1,
                });
            }
        }

        if !plunges.is_empty() {
            holes.push(DrillHoleSection {
                start_move: start,
                end_move: end,
                plunges,
            });
        }
        i = end;
    }
    holes
}

fn same_xy(ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    (ax - bx).abs() <= 1.0e-6 && (ay - by).abs() <= 1.0e-6
}

/// Build structural spans from Adaptive3d runtime events.
///
/// Emits the top-level `Operation` span plus `Region`, `DepthPass`, and
/// `RapidOrderBarrier` spans derived from adaptive3d's event stream.
pub fn spans_from_adaptive3d_annotations(
    events: &[crate::adaptive3d::Adaptive3dRuntimeAnnotation],
    n_moves: usize,
) -> Vec<Span> {
    let mut spans = operation_spans(n_moves);
    push_adaptive3d_spans(&mut spans, events, n_moves);
    spans
}

/// Translate Adaptive3d runtime events into structural spans.
///
/// - `RegionZLevel | GlobalZLevel | WaterlineCleanup`: zero-width
///   `RapidOrderBarrier` spans at the move index.
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
            span = span
                .with_label(format!("Z level {}", idx + 1))
                .with_payload(SpanPayload::DepthPass {
                    z_level: z,
                    pass_index: idx,
                });
        } else {
            span = span.with_label("Waterline cleanup");
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
            Span::new(*start, end, SpanKind::Region)
                .with_label(format!("Adaptive region {}", region_id + 1))
                .with_payload(SpanPayload::Region {
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
    fn operation_spans_yields_only_operation_span() {
        let spans = operation_spans(42);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, SpanKind::Operation);
        assert_eq!(spans[0].start_move, 0);
        assert_eq!(spans[0].end_move, 42);
    }

    #[test]
    fn empty_toolpath_still_gets_operation_span() {
        let spans = operation_spans(0);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].is_boundary());
    }

    #[test]
    fn labeled_events_emit_region_spans() {
        let spans = spans_from_labeled_events(
            30,
            vec![(0, "Ring 1".to_owned()), (12, "Ring 2".to_owned())],
        );
        let regions: Vec<_> = spans
            .iter()
            .filter(|span| span.kind == SpanKind::Region)
            .collect();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].range(), 0..12);
        assert_eq!(regions[0].label, "Ring 1");
        assert_eq!(regions[1].range(), 12..30);
        assert_eq!(regions[1].label, "Ring 2");
    }

    #[test]
    fn adaptive3d_emits_rapid_order_barriers() {
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

        let spans = spans_from_adaptive3d_annotations(&events, 150);
        let mut tp = Toolpath::new();
        for _ in 0..150 {
            tp.feed_to(crate::geo::P3::new(0.0, 0.0, 0.0), 1000.0);
        }
        let derived = AnnotatedToolpath::with_spans(tp, spans).rapid_order_barriers();

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
        let spans = spans_from_adaptive3d_annotations(&events, 60);
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
        let spans = spans_from_adaptive3d_annotations(&events, 100);
        let regions: Vec<_> = spans
            .iter()
            .filter(|s| s.kind == SpanKind::Region)
            .collect();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].range(), 0..40);
        assert_eq!(regions[1].range(), 40..100);
    }
}
