//! Spiral finishing strategy for 3D surface machining.
//!
//! Generates a continuous Archimedean spiral toolpath from center outward
//! (or reversed for outside-in), drop-cutting each point onto the mesh surface.
//! This produces a single uninterrupted cut with no retract-reposition cycles,
//! ideal for smooth concave surfaces like bowls and dishes.
//!
//! Algorithm:
//! 1. Compute mesh bounding box center and max radius to farthest corner.
//! 2. Walk an Archimedean spiral r(θ) = stepover·θ/(2π) with adaptive angular
//!    stepping for consistent point density.
//! 3. Drop-cutter each spiral point onto the mesh.
//! 4. Build a single continuous toolpath segment.

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
#[cfg(test)]
use crate::polygon::Polygon2;
use crate::region_set::RegionSet;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Cadence for cooperative-cancel polling inside the dense per-point spiral
/// loops below — checked every this-many points rather than every point, to
/// keep the check off the hot path.
const CANCEL_POLL_STRIDE: usize = 256;

/// Whether the spiral cuts from center outward or rim inward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpiralDirection {
    /// Start at the center and spiral outward to the rim.
    InsideOut,
    /// Start at the rim and spiral inward to the center.
    OutsideIn,
}

/// Parameters for spiral finishing.
pub struct SpiralFinishParams {
    /// Radial distance between adjacent spiral revolutions (mm).
    pub stepover: f64,
    /// Spiral traversal direction.
    pub direction: SpiralDirection,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for initial descent (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid positioning (mm).
    pub safe_z: f64,
    /// Extra material to leave on the surface (mm). Added to drop-cutter Z
    /// so the tool stays above the surface rather than cutting into it.
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpiralFinishRuntimeEvent {
    Ring {
        ring_index: usize,
        ring_total: usize,
        radius_mm: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpiralFinishRuntimeAnnotation {
    pub move_index: usize,
    pub event: SpiralFinishRuntimeEvent,
}

impl SpiralFinishRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ring { ring_index, .. } => format!("Ring {ring_index}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SpiralSample2d {
    x: f64,
    y: f64,
    theta: f64,
    radius: f64,
}

/// Generate a spiral finishing toolpath over a 3D mesh.
///
/// Produces an Archimedean spiral of drop-cutter points covering the mesh XY
/// footprint. Points that miss the mesh entirely are skipped.
pub fn spiral_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
) -> Toolpath {
    let (tp, _) =
        spiral_finish_toolpath_structured_annotated(mesh, index, cutter, params, None, None);
    tp
}

/// Cancellable variant of [`spiral_finish_toolpath`].
#[allow(clippy::expect_used)]
pub fn spiral_finish_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = spiral_finish_toolpath_structured_annotated_with_cancel(
        mesh, index, cutter, params, None, None, cancel,
    )?;
    Ok(tp)
}

fn runtime_annotations_to_labels(
    annotations: &[SpiralFinishRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn spiral_finish_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
) -> (Toolpath, Vec<SpiralFinishRuntimeAnnotation>) {
    let never_cancel = || false;
    spiral_finish_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        &never_cancel,
    )
    .expect("non-cancellable spiral finish toolpath should never be cancelled")
}

/// Cancellable variant of [`spiral_finish_toolpath_structured_annotated`].
/// Polls `cancel` every [`CANCEL_POLL_STRIDE`] points in both the drop-cutter
/// sampling loop and the toolpath-emission loop (a full-radius fine-stepover
/// spiral can carry tens of thousands of points).
///
/// `boundary_regions` (P2.3): when `Some`, a spiral sample point outside
/// every region is skipped BEFORE the drop-cutter query — cheap XY
/// containment gates the expensive query. The point is recorded as a `None`
/// marker, same as a point that misses the mesh, so the existing run-split
/// below treats it as a gap. Regions are already dilated by fine-tool
/// radius + margin at derivation and the post-generation boundary clip
/// still enforces exact containment — this is a conservative superset
/// filter for performance. `None` reproduces today's full-mesh sampling
/// byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn spiral_finish_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<SpiralFinishRuntimeAnnotation>), Cancelled> {
    check_cancel(cancel)?;
    if params.stepover <= 0.0 {
        return Ok((Toolpath::new(), Vec::new()));
    }

    let bbox = &mesh.bbox;
    let cx = (bbox.min.x + bbox.max.x) * 0.5;
    let cy = (bbox.min.y + bbox.max.y) * 0.5;

    // Max radius: distance from center to farthest bounding-box corner, plus
    // one cutter radius so the tool fully covers the edge.
    let max_radius = bbox.max_corner_distance_xy(cx, cy) + cutter.radius();

    // Generate spiral XY coordinates.
    let spiral_xy = generate_spiral_samples(cx, cy, max_radius, params.stepover);

    // Drop-cut each point onto the mesh. Points that miss the mesh entirely
    // (outside the footprint, or over a hole) are kept as `None` markers —
    // rather than being dropped outright — so a run split downstream can
    // tell a genuine gap from mere adjacency in the surviving-point list.
    // Joining survivors across a dropped gap with a plain cutting feed would
    // chord straight across whatever the spiral skipped over.
    type SpiralHit = (P3, usize, f64);
    let mut samples: Vec<Option<SpiralHit>> = Vec::with_capacity(spiral_xy.len());
    for (i, sample) in spiral_xy.iter().enumerate() {
        if i % CANCEL_POLL_STRIDE == 0 {
            check_cancel(cancel)?;
        }
        let in_region =
            boundary_regions.is_none_or(|regions| regions.contains(&P2::new(sample.x, sample.y)));
        if !in_region {
            samples.push(None);
            continue;
        }
        let cl = point_drop_cutter(sample.x, sample.y, mesh, index, cutter);
        if cl.contacted {
            let z = cl.z + params.stock_to_leave;
            let ring_index = (sample.theta / std::f64::consts::TAU).floor() as usize + 1;
            samples.push(Some((P3::new(cl.x, cl.y, z), ring_index, sample.radius)));
        } else {
            samples.push(None);
        }
    }

    // Reverse for outside-in cutting.
    if params.direction == SpiralDirection::OutsideIn {
        samples.reverse();
    }

    let ring_total = samples
        .iter()
        .filter_map(|s| s.as_ref().map(|&(_, ring_index, _)| ring_index))
        .max()
        .unwrap_or(0);

    let runs = crate::point_runs::split_runs(
        &samples,
        |_, s: &Option<SpiralHit>| s.is_some(),
        crate::point_runs::RunTopology::Open,
        1,
    );
    if runs.is_empty() {
        return Ok((Toolpath::new(), Vec::new()));
    }

    // Build toolpath: rapid → plunge → feed → retract per contacted run, so
    // a gap in the middle of the spiral (a hole, or the mesh's own edge) is
    // bridged by a retract/replunge rather than a cutting-feed chord.
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    let mut current_ring: Option<usize> = None;
    let mut poll_counter: usize = 0;

    for run in &runs {
        let pts: Vec<SpiralHit> = run.iter().filter_map(|s| *s).collect();
        let Some(&(first_point, first_ring_index, first_radius)) = pts.first() else {
            continue;
        };

        tp.rapid_to_with_intent(
            P3::new(first_point.x, first_point.y, params.safe_z),
            crate::toolpath::MoveIntent::Linking,
        );
        let rapid_move_index = tp.moves.len().saturating_sub(1);
        tp.feed_to_with_intent(
            first_point,
            params.plunge_rate,
            crate::toolpath::MoveIntent::EntryPlunge,
        );
        if current_ring != Some(first_ring_index) {
            current_ring = Some(first_ring_index);
            annotations.push(SpiralFinishRuntimeAnnotation {
                move_index: rapid_move_index,
                event: SpiralFinishRuntimeEvent::Ring {
                    ring_index: first_ring_index,
                    ring_total,
                    radius_mm: first_radius,
                },
            });
        }

        for &(point, ring_index, radius_mm) in pts.iter().skip(1) {
            poll_counter += 1;
            if poll_counter.is_multiple_of(CANCEL_POLL_STRIDE) {
                check_cancel(cancel)?;
            }
            if current_ring != Some(ring_index) {
                current_ring = Some(ring_index);
                annotations.push(SpiralFinishRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: SpiralFinishRuntimeEvent::Ring {
                        ring_index,
                        ring_total,
                        radius_mm,
                    },
                });
            }
            tp.feed_to_with_intent(
                point,
                params.feed_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
    }
    tp.final_retract(params.safe_z);

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    Ok((tp, annotations))
}

pub fn spiral_finish_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) =
        spiral_finish_toolpath_structured_annotated(mesh, index, cutter, params, debug, None);
    (tp, runtime_annotations_to_labels(&annotations))
}

// ── helpers ────────────────────────────────────────────────────────────────

/// Walk an Archimedean spiral from center (cx,cy) outward.
///
/// r(θ) = stepover · θ / (2π)
///
/// The angular increment is adaptive: dθ = stepover / max(r, stepover) so that
/// the linear spacing between consecutive points stays roughly constant.
fn generate_spiral_samples(
    cx: f64,
    cy: f64,
    max_radius: f64,
    stepover: f64,
) -> Vec<SpiralSample2d> {
    let two_pi = std::f64::consts::TAU;
    // θ_max where r(θ_max) = max_radius  ⟹  θ_max = max_radius * 2π / stepover
    let theta_max = max_radius * two_pi / stepover;

    let mut points = Vec::new();
    let mut theta = 0.0_f64;

    while theta <= theta_max {
        let r = stepover * theta / two_pi;
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin();
        points.push(SpiralSample2d {
            x,
            y,
            theta,
            radius: r,
        });

        // Adaptive step: at small r use a minimum to avoid near-zero division.
        let dtheta = stepover / r.max(stepover);
        theta += dtheta;
    }

    // Include the outermost point exactly at max_radius.
    let r_last = stepover * theta_max / two_pi;
    if (r_last - max_radius).abs() > stepover * 0.1 {
        let x = cx + max_radius * theta_max.cos();
        let y = cy + max_radius * theta_max.sin();
        points.push(SpiralSample2d {
            x,
            y,
            theta: theta_max,
            radius: max_radius,
        });
    }

    points
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
    use crate::geo::BoundingBox3;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    /// Build a flat 50×50 mm mesh at z=0 with its spatial index.
    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    /// (x,y)-only view of [`generate_spiral_samples`] for tests that don't
    /// care about the theta/radius bookkeeping.
    fn generate_spiral_points(cx: f64, cy: f64, max_radius: f64, stepover: f64) -> Vec<(f64, f64)> {
        generate_spiral_samples(cx, cy, max_radius, stepover)
            .into_iter()
            .map(|sample| (sample.x, sample.y))
            .collect()
    }

    // ── Spiral point generation ────────────────────────────────────────

    #[test]
    fn spiral_points_cover_radius() {
        let pts = generate_spiral_points(0.0, 0.0, 20.0, 2.0);
        assert!(
            pts.len() > 50,
            "Should produce many points, got {}",
            pts.len()
        );

        // The last few points should be near the max radius.
        let last = pts.last().expect("non-empty");
        let r_last = (last.0 * last.0 + last.1 * last.1).sqrt();
        assert!(
            r_last >= 18.0,
            "Outermost point should be near max_radius=20, got r={:.2}",
            r_last,
        );
    }

    #[test]
    fn spiral_points_start_at_center() {
        let pts = generate_spiral_points(5.0, 10.0, 20.0, 2.0);
        let first = pts[0];
        assert!(
            (first.0 - 5.0).abs() < 0.01 && (first.1 - 10.0).abs() < 0.01,
            "First point should be at center (5,10), got ({:.2},{:.2})",
            first.0,
            first.1,
        );
    }

    // ── Corner distance helper (shared geo.rs BoundingBox3 method) ─────

    #[test]
    fn corner_distance_square() {
        let bbox = BoundingBox3 {
            min: P3::new(-10.0, -10.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let d = bbox.max_corner_distance_xy(0.0, 0.0);
        // Diagonal of a 20×20 square / 2 = √200 ≈ 14.14
        assert!(
            (d - 14.142).abs() < 0.1,
            "Corner distance should be ~14.14, got {:.3}",
            d,
        );
    }

    // ── Full toolpath integration ──────────────────────────────────────

    #[test]
    fn flat_mesh_produces_nonempty_toolpath() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 2.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Flat mesh spiral should produce moves, got {}",
            tp.moves.len(),
        );
        assert!(
            tp.total_cutting_distance() > 50.0,
            "Cutting distance should be substantial, got {:.1}",
            tp.total_cutting_distance(),
        );
    }

    #[test]
    fn outside_in_reverses_direction() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let base = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp_in = spiral_finish_toolpath(&mesh, &si, &cutter, &base);
        let tp_out = spiral_finish_toolpath(
            &mesh,
            &si,
            &cutter,
            &SpiralFinishParams {
                direction: SpiralDirection::OutsideIn,
                ..base
            },
        );

        // Both should have moves.
        assert!(tp_in.moves.len() > 5);
        assert!(tp_out.moves.len() > 5);

        // First cutting move (index 2: after rapid-to-safe-z and rapid-to-XY) should
        // differ in XY between the two directions. Inside-out starts near center,
        // outside-in starts near the rim.
        let first_cut_in = &tp_in.moves[2].target;
        let first_cut_out = &tp_out.moves[2].target;

        let r_in = (first_cut_in.x * first_cut_in.x + first_cut_in.y * first_cut_in.y).sqrt();
        let r_out = (first_cut_out.x * first_cut_out.x + first_cut_out.y * first_cut_out.y).sqrt();

        assert!(
            r_out > r_in + 1.0,
            "OutsideIn first cut should be farther from center: r_in={:.1}, r_out={:.1}",
            r_in,
            r_out,
        );
    }

    #[test]
    fn stock_to_leave_raises_z() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let base = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp_zero = spiral_finish_toolpath(&mesh, &si, &cutter, &base);
        let tp_leave = spiral_finish_toolpath(
            &mesh,
            &si,
            &cutter,
            &SpiralFinishParams {
                stock_to_leave: 1.0,
                ..base
            },
        );

        // Find the minimum Z among cutting (non-rapid) moves.
        let min_z = |tp: &Toolpath| -> f64 {
            tp.moves
                .iter()
                .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
                .map(|m| m.target.z)
                .fold(f64::INFINITY, f64::min)
        };

        let z0 = min_z(&tp_zero);
        let z1 = min_z(&tp_leave);

        // Positive stock_to_leave must raise the cutter above the s=0
        // baseline, never push it below (which would gouge the finished
        // surface).
        assert!(
            (z1 - z0 - 1.0).abs() < 0.1,
            "stock_to_leave=1 should raise Z by ~1mm: z0={:.3}, z1={:.3}",
            z0,
            z1,
        );
        assert!(
            z1 + 1e-6 >= z0,
            "stock_to_leave should never lower cutting Z below the s=0 baseline: z0={:.3}, z1={:.3}",
            z0,
            z1,
        );
    }

    #[test]
    fn zero_stepover_returns_empty() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 0.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };
        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.is_empty(),
            "Zero stepover should return empty toolpath, got {} moves",
            tp.moves.len(),
        );
    }

    #[test]
    fn safe_z_respected() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 42.0,
            stock_to_leave: 0.0,
        };

        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);

        // First and last moves should be rapids at safe_z.
        let first = &tp.moves[0];
        assert!(
            (first.target.z - 42.0).abs() < 0.01,
            "First rapid should be at safe_z=42, got {:.2}",
            first.target.z,
        );
        let last = &tp.moves[tp.moves.len() - 1];
        assert!(
            (last.target.z - 42.0).abs() < 0.01,
            "Last rapid should be at safe_z=42, got {:.2}",
            last.target.z,
        );
    }

    // ── Regression: spiral must not chord across a disjoint-patch gap ──

    /// Two disjoint flat patches: a hub square centered at the origin and
    /// an outer square offset along +X, separated by empty space. Before
    /// the point-runs migration, non-contacted spiral samples were simply
    /// skipped and the survivors joined sequentially — so the last
    /// contacted point on the hub was chorded straight across the gap to
    /// the first contacted point on the outer patch, at cutting feed. This
    /// regression asserts every cutting move is close to its predecessor
    /// (a real cut), never a long chord across the excluded gap.
    #[test]
    fn spiral_finish_no_chord_across_disjoint_patch_gap() {
        let z = 0.0;
        let mut vertices = vec![
            // Hub patch: x,y in [-11, 11].
            P3::new(-11.0, -11.0, z),
            P3::new(11.0, -11.0, z),
            P3::new(11.0, 11.0, z),
            P3::new(-11.0, 11.0, z),
            // Outer patch: x,y in [29, 51].
            P3::new(29.0, -11.0, z),
            P3::new(51.0, -11.0, z),
            P3::new(51.0, 11.0, z),
            P3::new(29.0, 11.0, z),
        ];
        let triangles = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]];
        // Padding-only vertices (not part of any triangle) keep the mesh
        // bounding box — and hence the spiral center — symmetric about the
        // origin, same as the equivalent radial_finish regression mesh.
        vertices.push(P3::new(-51.0, -51.0, z));
        vertices.push(P3::new(51.0, 51.0, z));
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let si = SpatialIndex::build(&mesh, 10.0);

        let cutter = ball_cutter();
        let stepover = 1.5;
        let params = SpiralFinishParams {
            stepover,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);

        // No consecutive pair of cutting (FinishingCut) moves may be
        // farther apart than a small multiple of the spiral's own point
        // spacing — a larger jump means a cutting move chorded across the
        // gap between the two patches.
        let max_allowed = stepover * 3.0;
        let mut prev_cut: Option<P3> = None;
        let mut saw_any_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_any_cut = true;
                if let Some(prev) = prev_cut {
                    let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                    assert!(
                        d <= max_allowed,
                        "cutting move chorded across the disjoint-patch gap: {:.2}mm \
                         (allowed {:.2}mm) from ({:.2},{:.2}) to ({:.2},{:.2})",
                        d,
                        max_allowed,
                        prev.x,
                        prev.y,
                        m.target.x,
                        m.target.y
                    );
                }
                prev_cut = Some(m.target);
            } else {
                // A rapid or plunge breaks the run — the next cutting move
                // starts a fresh pass and shouldn't be distance-checked
                // against whatever preceded the break.
                prev_cut = None;
            }
        }
        assert!(saw_any_cut, "expected at least one cutting move");
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn spiral_boundary_regions_none_matches_call_without_param() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        let tp_default = spiral_finish_toolpath(&mesh, &si, &cutter, &params);
        let (tp_none, _) = spiral_finish_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(tp_default.moves.len(), tp_none.moves.len());
        for (a, b) in tp_default.moves.iter().zip(tp_none.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-9);
            assert!((a.target.y - b.target.y).abs() < 1e-9);
            assert!((a.target.z - b.target.z).abs() < 1e-9);
        }
    }

    #[test]
    fn spiral_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 2.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        // Left half of the 50mm flat mesh (bbox [-25,25]).
        let left_half = Polygon2::new(vec![
            P2::new(-25.0, -25.0),
            P2::new(0.0, -25.0),
            P2::new(0.0, 25.0),
            P2::new(-25.0, 25.0),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let (tp, _) = spiral_finish_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "cutting move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one cutting move");
    }
}
