//! Project Curve on Surface — projects 2D polygon paths onto a 3D mesh for engraving.
//!
//! Given a 2D polygon (exterior + holes), resamples each ring at a fine spacing,
//! finds the tip Z of each point and builds a toolpath that follows the
//! projected contour at a specified depth below the surface.
//!
//! Two projection modes find the tip Z ([`ProjectProjection`]):
//! - `Cutter` drops the real cutter (`point_drop_cutter`). The tool body
//!   stops on the surface, so the surface is protected.
//! - `Point` reads the surface Z directly under the point
//!   ([`crate::maps::reach_map::surface_z_at`]) and adds the depth. The tool
//!   body is not tested against the surface.

use crate::geo::{P2, P3, resample_polyline};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::maps::reach_map::surface_z_at;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::surface::dropcutter::{point_drop_cutter, point_is_over_mesh_xy};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// How many resampled points the serial path projects between cancel polls
/// in `project_tip_zs`. This is frequent enough to stay responsive and
/// coarse enough to avoid an atomic load per point.
#[cfg(not(feature = "parallel"))]
const CANCEL_POLL_STRIDE: usize = 64;

/// How many resampled points the parallel path projects in one rayon
/// batch. The calling thread polls cancel between batches, because a
/// `&dyn CancelCheck` is not `Sync` and cannot enter a rayon closure.
///
/// 1024 = 16 x the serial stride of 64. On 16 or more threads, one batch
/// takes about the wall time of 64 serial points, so the cancel latency is
/// the same as the serial path. On one thread it is 16 x longer. A batch of
/// 1024 also gives each of 24 threads about 40 points, so the fork-join
/// cost per batch is small against the drop-cutter work.
#[cfg(feature = "parallel")]
const PARALLEL_CHUNK: usize = 1024;

/// Direction from which the curve is projected onto the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDirection {
    /// Project from above (Z-down). Tool contacts the top surface.
    FromAbove,
    /// Project from below (Z-up). Tool contacts the bottom surface.
    FromBelow,
}

/// Tool-radius compensation side for closed rings. Open rings ignore this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSide {
    /// Tool centerline rides exactly on the curve.
    #[default]
    Center,
    /// Offset inward by tool radius (closed rings only).
    Inside,
    /// Offset outward by tool radius (closed rings only).
    Outside,
}

/// How the tip Z of a sample is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectProjection {
    /// Drop the real cutter on the surface. The tip stops where the tool
    /// body first touches the surface.
    #[default]
    Cutter,
    /// Tip Z = the surface Z directly under the sample, plus the depth. The
    /// tool body is not tested against the surface, so it can cut a wall
    /// that is steeper than the tool flank.
    ///
    /// Sample rule: every vertex of the curve is a sample, and each curve
    /// segment is split into equal steps of at most `point_spacing`. At a
    /// vertex the tip is exact; between two samples the tool moves on the
    /// chord, so the error is the chord error of the surface over one step.
    Point,
}

/// P29-9 9b: THE sign of the Project Curve depth. The depth (mm) of the
/// tip into the material, measured from the side the machine cuts from
/// (+Z of the emitted frame), for a stated `depth`.
///
/// - From above: the tip goes `depth` below the top surface.
/// - From below on a flipped setup (`SetupTransformInfo::is_z_flipped`):
///   the setup transform has inverted the mesh, so the kernel runs as from
///   above in the local frame: `depth`.
/// - From below on a setup that is not flipped: the kernel finds the
///   bottom surface and puts the tip `depth` above it in the world frame.
///   The top is never below the bottom, so the tip is at least `-depth`
///   below the top surface (exact on a sheet).
///
/// The kernel emits its Z through this rule, and the declared depth of the
/// cut (`compute::catalog::TipDepth`) reads it. A value above zero is a
/// lower bound of the depth into the material; the stock above the surface
/// adds to it. Measured on the emitted moves in `P29-9_DESIGN.md` section
/// 16.
#[must_use]
pub fn tip_depth_into_material_mm(
    depth: f64,
    direction: ProjectDirection,
    setup_z_flipped: bool,
) -> f64 {
    match direction {
        ProjectDirection::FromAbove => depth,
        ProjectDirection::FromBelow => {
            if setup_z_flipped {
                depth
            } else {
                -depth
            }
        }
    }
}

impl From<crate::compute::operation_configs::ProjectCurveDirection> for ProjectDirection {
    fn from(d: crate::compute::operation_configs::ProjectCurveDirection) -> Self {
        use crate::compute::operation_configs::ProjectCurveDirection as Config;
        match d {
            Config::FromAbove => Self::FromAbove,
            Config::FromBelow => Self::FromBelow,
        }
    }
}

impl From<crate::compute::operation_configs::ProjectCurveSide> for ProjectSide {
    fn from(s: crate::compute::operation_configs::ProjectCurveSide) -> Self {
        use crate::compute::operation_configs::ProjectCurveSide as Config;
        match s {
            Config::Center => Self::Center,
            Config::Inside => Self::Inside,
            Config::Outside => Self::Outside,
        }
    }
}

impl From<crate::compute::operation_configs::ProjectCurveProjection> for ProjectProjection {
    fn from(p: crate::compute::operation_configs::ProjectCurveProjection) -> Self {
        use crate::compute::operation_configs::ProjectCurveProjection as Config;
        match p {
            Config::Cutter => Self::Cutter,
            Config::Point => Self::Point,
        }
    }
}

/// Parameters for the project-curve-on-surface operation.
pub struct ProjectCurveParams {
    /// Cut depth below (or above) the mesh surface (positive = into material).
    pub depth: f64,
    /// Feed rate for lateral moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for Z-descent moves (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapids above the workpiece.
    pub safe_z: f64,
    /// Resample spacing along polygon edges (mm). Smaller = smoother projection.
    pub point_spacing: f64,
    /// Which side of the mesh to project onto.
    pub direction: ProjectDirection,
    /// Tool radius for compensation (used when `side != Center`).
    pub tool_radius: f64,
    /// Compensation side. Closed rings only.
    pub side: ProjectSide,
    /// How the tip Z of a sample is found.
    pub projection: ProjectProjection,
    /// When true, the mesh has already been Z-inverted by a bottom-facing setup
    /// transform. `FromBelow` should NOT apply its own Z-flip in this case
    /// (it would double-flip, cancelling the setup transform). The Curve
    /// runner sets it from `ExecutionContext::z_flipped` (W6.5).
    pub setup_z_flipped: bool,
}

/// Close a ring by appending the first point if not already duplicated.
fn close_ring(ring: &[P2]) -> Vec<P2> {
    if ring.len() < 2 {
        return ring.to_vec();
    }
    #[expect(clippy::indexing_slicing, reason = "len >= 2 checked above")]
    let first = ring[0];
    #[expect(clippy::expect_used, reason = "ring.len() >= 2 checked above")]
    let last = *ring.last().expect("len >= 2");
    let d = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
    if d > 1e-9 {
        let mut closed = ring.to_vec();
        closed.push(first);
        closed
    } else {
        ring.to_vec()
    }
}

/// Sample a polyline for [`ProjectProjection::Point`]: every input vertex,
/// and each segment split into `ceil(len / spacing)` equal steps, so no
/// step is longer than `spacing`. A segment shorter than 1e-9 mm adds no
/// sample. A `spacing` at or below 1e-6 mm returns the vertices.
fn sample_vertices_and_steps(points: &[P2], spacing: f64) -> Vec<P2> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    if points.len() < 2 || spacing <= 1e-6 {
        return points.to_vec();
    }
    let mut out = vec![*first];
    for w in points.windows(2) {
        let (Some(a), Some(b)) = (w.first(), w.get(1)) else {
            continue;
        };
        let delta = b - a;
        let len = delta.norm();
        if len < 1e-9 {
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "`len / spacing` is finite and positive here (len >= 1e-9, spacing > 1e-6); a curve segment never needs 2^32 steps."
        )]
        let steps = (len / spacing).ceil().max(1.0) as usize;
        for k in 1..steps {
            #[expect(clippy::cast_precision_loss, reason = "k < steps, a small count")]
            let t = k as f64 / steps as f64;
            out.push(a + delta * t);
        }
        out.push(*b);
    }
    out
}

/// Project a curve onto the mesh: the kernel the Curve runner calls
/// (`compute::execute::generate_project_curve`). Polls `cancel` between
/// batches of resampled points inside the per-ring projection
/// (`project_tip_zs`, P2 of planning/perf_sweep_2026-11-03/PLAN.md).
/// planning/finishing_stack_review_2026-07.md S.5 named "project_curve
/// (drop-cutter per point)" the worst per-op loop of the flat-2D family.
pub fn project_curve_toolpath_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    match params.direction {
        ProjectDirection::FromAbove => {
            project_curve_inner_with_cancel(polygon, mesh, index, cutter, params, false, cancel)
        }
        ProjectDirection::FromBelow => {
            if params.setup_z_flipped {
                // The mesh is already Z-inverted by a bottom-facing setup transform.
                // The drop cutter finds correct surface contact without an additional
                // flip. Depth goes below the surface in local frame (same as FromAbove).
                project_curve_inner_with_cancel(polygon, mesh, index, cutter, params, false, cancel)
            } else {
                // Standalone (no setup transform): flip the mesh Z so the bottom
                // surface becomes the top for the drop cutter.
                let flipped = mesh.z_flipped();
                let flipped_index = SpatialIndex::build_auto(&flipped);
                project_curve_inner_with_cancel(
                    polygon,
                    &flipped,
                    &flipped_index,
                    cutter,
                    params,
                    true,
                    cancel,
                )
            }
        }
    }
}

/// Inner projection loop. When `z_flip` is true, the mesh was Z-flipped before
/// calling, so the output Z coordinates are negated back to world space and
/// depth goes upward (into the bottom surface).
fn project_curve_inner_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    z_flip: bool,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut tp = Toolpath::new();

    // Apply tool-radius compensation for closed polygons. Open polygons cannot
    // be meaningfully offset (no inside/outside) — fall back to Center.
    // Sign convention in offset_polygon: distance > 0 = inward (shrink),
    // distance < 0 = outward (grow). Center carries no offset distance, so
    // the match stays exhaustive without an unreachable arm.
    let offset_distance = match params.side {
        ProjectSide::Inside => Some(params.tool_radius),
        ProjectSide::Outside => Some(-params.tool_radius),
        ProjectSide::Center => None,
    };
    let offset_polys: Vec<Polygon2> = if polygon.closed {
        match offset_distance {
            Some(distance) => offset_polygon(polygon, distance),
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let polys_iter: Vec<&Polygon2> = if offset_polys.is_empty() {
        // Either Center, open ring, or offset collapsed → use original.
        vec![polygon]
    } else {
        offset_polys.iter().collect()
    };

    for poly in polys_iter {
        project_polygon_rings_with_cancel(
            poly, mesh, index, cutter, params, z_flip, &mut tp, cancel,
        )?;
    }

    tp.final_retract(params.safe_z);
    Ok(tp)
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn project_polygon_rings_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    z_flip: bool,
    tp: &mut Toolpath,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Collect all rings: exterior first, then holes
    let mut rings: Vec<&Vec<P2>> = Vec::with_capacity(1 + polygon.holes.len());
    rings.push(&polygon.exterior);
    for hole in &polygon.holes {
        rings.push(hole);
    }

    for (ring_idx, ring) in rings.iter().enumerate() {
        if ring.len() < 2 {
            continue;
        }

        // Only close the ring for true polygon rings (holes always close, and
        // the exterior closes when polygon.closed is true). Open paths
        // (Polygon2::open_path — unclosed DXF polylines, SVG paths without Z)
        // must stay open, otherwise we emit a phantom segment from the last
        // point back to the first, which carves a straight line across the
        // stock at cut depth.
        let is_hole = ring_idx > 0;
        let ring_points: Vec<P2> = if polygon.closed || is_hole {
            close_ring(ring)
        } else {
            (*ring).clone()
        };
        let resampled = match params.projection {
            ProjectProjection::Cutter => resample_polyline(&ring_points, params.point_spacing),
            ProjectProjection::Point => {
                sample_vertices_and_steps(&ring_points, params.point_spacing)
            }
        };

        // Project each 2D point onto the mesh (in parallel when the
        // `parallel` feature is on), then build the chains serially.
        let tip_zs = project_tip_zs(&resampled, mesh, index, cutter, params.projection, cancel)?;
        let mut current_chain: Vec<P3> = Vec::new();

        // P29-9 9b: the one sign rule of the depth.
        let into =
            tip_depth_into_material_mm(params.depth, params.direction, params.setup_z_flipped);
        for (pt, surface_tip_z) in resampled.iter().zip(tip_zs) {
            if let Some(tip_z) = surface_tip_z {
                let z = if z_flip {
                    // Flip Z back to world space: the tip is `depth` above
                    // the bottom surface (`into` = -depth).
                    -tip_z - into
                } else {
                    tip_z - into
                };
                current_chain.push(P3::new(pt.x, pt.y, z));
            } else {
                // Gap over air (or over a mesh hole) — flush any
                // accumulated chain so the next contiguous stretch starts
                // fresh with a rapid-plunge rather than a feed bridging it.
                if !current_chain.is_empty() {
                    tp.emit_path_segment_with_intent(
                        &current_chain,
                        params.safe_z,
                        params.feed_rate,
                        params.plunge_rate,
                        crate::toolpath::MoveIntent::FinishingCut,
                    );
                    current_chain.clear();
                }
            }
        }

        // Flush remaining chain
        if !current_chain.is_empty() {
            tp.emit_path_segment_with_intent(
                &current_chain,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
    }

    Ok(())
}

/// P35 F: the curves of one drawn curve after the lateral offset.
#[derive(Debug, Clone)]
pub struct LateralCurves {
    /// The tool-centre curves before `side`: open pieces in the drawn
    /// direction, or closed rings in the drawn winding. Empty when the
    /// offset leaves no room.
    pub curves: Vec<Polygon2>,
    /// The offset calls that came back with an `OffsetFailure`.
    pub failures: usize,
    /// One note for each ring or path that is not one piece.
    pub notes: Vec<crate::compute::toolpath_stats::TabNote>,
}

/// P35 F (operator ruling 2026-11-16, "2 mm tool slot"): `curve` moved by
/// the lateral offset `offset` (mm) in XY, before the projection.
///
/// - An open path: a positive value is to the LEFT of the drawn direction
///   (the first point to the last), the sign of
///   [`crate::polygon::offset_open_path_reported`] and of Trace `left`.
///   `drawing_mirrored` (a Bottom setup) flips the sign, so the side holds
///   in the drawing frame, as `TraceParams::drawing_mirrored` does.
/// - A closed ring: a positive value moves the ring OUT of the area that
///   it closes ([`crate::ops::profile::rings_moved_out`], the rule of
///   Profile side `On`). Each ring of the polygon is its own loop.
///
/// A path or a ring that the offset does not give back as one piece is
/// cut piece by piece (none, when the offset leaves no room), and one
/// `TabNote::OffsetSplit` names it. No offset routine lives here.
#[must_use]
pub fn lateral_curves(curve: &Polygon2, offset: f64, drawing_mirrored: bool) -> LateralCurves {
    use crate::compute::toolpath_stats::TabNote;
    let note = |first: Option<&P2>, pieces: usize, closed: bool| {
        (pieces != 1).then(|| TabNote::OffsetSplit {
            start: first.map_or([0.0, 0.0], |p| [p.x, p.y]),
            offset,
            pieces,
            closed,
        })
    };
    let mut out = LateralCurves {
        curves: Vec::new(),
        failures: 0,
        notes: Vec::new(),
    };
    if !curve.closed {
        let distance = if drawing_mirrored { -offset } else { offset };
        let (pieces, failure) = crate::polygon::offset_open_path_reported(curve, distance);
        out.failures += usize::from(failure.is_some());
        out.notes
            .extend(note(curve.exterior.first(), pieces.len(), false));
        out.curves = pieces;
        return out;
    }
    for ring in std::iter::once(&curve.exterior)
        .chain(&curve.holes)
        .filter(|ring| ring.len() >= 3)
    {
        let (rings, failures) = crate::ops::profile::rings_moved_out(ring, offset);
        out.failures += failures;
        out.notes.extend(note(ring.first(), rings.len(), true));
        out.curves.extend(rings.into_iter().map(Polygon2::new));
    }
    out
}

/// P33 (operator ruling 2026-11-15: "For multiple passes that should be
/// built into the toolpath, not multiple tool paths"): the depth ladder of
/// a Project Curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassLadder {
    /// The Z of the material top: the op top Z of the heights.
    pub top_z: f64,
    /// The largest depth (mm) of one pass.
    pub depth_per_pass: f64,
}

/// The fed chains of a planned path: the targets of each run of fed moves
/// between two rapids. The kernel emits one such run for each chain
/// (`Toolpath::emit_path_segment_with_intent`), and the plunge target is
/// the first point of the chain.
fn fed_chains(planned: &Toolpath) -> Vec<Vec<P3>> {
    let mut chains: Vec<Vec<P3>> = Vec::new();
    let mut open = false;
    for m in &planned.moves {
        match m.move_type {
            crate::toolpath::MoveType::Rapid => open = false,
            crate::toolpath::MoveType::Linear { .. }
            | crate::toolpath::MoveType::ArcCW { .. }
            | crate::toolpath::MoveType::ArcCCW { .. } => {
                if !open {
                    chains.push(Vec::new());
                    open = true;
                }
                if let Some(chain) = chains.last_mut() {
                    chain.push(m.target);
                }
            }
        }
    }
    chains
}

/// P33: the passes of a Project Curve, the first pass first.
///
/// `planned` is the path at the full depth: every curve of the op, as
/// [`project_curve_toolpath_with_cancel`] gives it. The convention is the
/// one of the river channel passes of `planning/rivmap_block_job/
/// derive_wanaka21.py`: each pass is the SAME projected curve moved up
/// from the full depth by a whole number of steps, a pass covers every
/// curve, and the deepest pass is the last.
///
/// - The levels are the crate's one ladder ([`crate::ops::depth::z_ladder`],
///   the 2.5D arm `EvenRedistributed`) from `ladder.top_z` down to the
///   lowest tip Z of `planned`: `n = ceil(total / depth_per_pass)` equal
///   steps, the top itself no level.
/// - Pass `i` is each chain moved up by `level_i - lowest tip Z`.
/// - A sample of an earlier pass at or above `ladder.top_z` is left out,
///   and the chain splits there as it does at a gap. So an earlier pass
///   feeds only below the op top, and each pass cuts at most one step
///   below the op top or below the pass before it (plus the Z change of
///   the surface over one sample step, where a chain starts).
/// - The last pass is `planned` itself: the full depth, every sample.
/// - Each chain of each pass enters as a chain of today: a rapid at
///   `params.safe_z`, a straight plunge at `params.plunge_rate`, the cut,
///   a rapid retract. No rapid goes below `params.safe_z`.
///
/// The ladder reads the emitted Z, where the tool always comes from +Z, so
/// it holds in each frame of [`tip_depth_into_material_mm`]. With no level
/// above the lowest tip (the path is at or above the top, or one step
/// reaches it) the result is the one pass `planned`.
#[must_use]
pub fn ladder_passes(
    planned: Toolpath,
    ladder: PassLadder,
    params: &ProjectCurveParams,
) -> Vec<Toolpath> {
    let chains = fed_chains(&planned);
    let lowest = chains
        .iter()
        .flatten()
        .map(|p| p.z)
        .fold(f64::INFINITY, f64::min);
    if !lowest.is_finite() {
        return vec![planned];
    }
    let levels = crate::ops::depth::z_ladder(
        ladder.top_z,
        lowest,
        ladder.depth_per_pass,
        crate::ops::depth::LadderMode::EvenRedistributed,
    );
    let Some((_full_depth, earlier)) = levels.split_last() else {
        return vec![planned];
    };
    let mut passes: Vec<Toolpath> = Vec::with_capacity(levels.len());
    for level in earlier {
        let lift = level - lowest;
        let mut pass = Toolpath::new();
        let mut run: Vec<P3> = Vec::new();
        let flush = |run: &mut Vec<P3>, pass: &mut Toolpath| {
            if !run.is_empty() {
                pass.emit_path_segment_with_intent(
                    run,
                    params.safe_z,
                    params.feed_rate,
                    params.plunge_rate,
                    crate::toolpath::MoveIntent::FinishingCut,
                );
                run.clear();
            }
        };
        for chain in &chains {
            for p in chain {
                let z = p.z + lift;
                if z < ladder.top_z {
                    run.push(P3::new(p.x, p.y, z));
                } else {
                    flush(&mut run, &mut pass);
                }
            }
            flush(&mut run, &mut pass);
        }
        if !pass.moves.is_empty() {
            passes.push(pass);
        }
    }
    passes.push(planned);
    passes
}

/// The tip Z of one point in the projection frame, or `None` where no
/// surface is directly under the point.
fn surface_tip_z(
    pt: &P2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    projection: ProjectProjection,
) -> Option<f64> {
    match projection {
        ProjectProjection::Cutter => {
            let cl = point_drop_cutter(pt.x, pt.y, mesh, index, cutter);
            // `point_drop_cutter` marks `contacted=true` whenever the cutter
            // (which has radius) touches ANY nearby triangle — including the
            // edge of a hole in the mesh. For project_curve we want to trace
            // the surface *directly below* the 2D path, so reject any point
            // whose vertical ray doesn't actually pass through a triangle.
            // Otherwise the tool rides on the hole's rim and produces
            // phantom bridges across the gap, visible as straight cuts
            // crossing the stock.
            let has_surface_below = cl.contacted && point_is_over_mesh_xy(pt.x, pt.y, mesh, index);
            has_surface_below.then_some(cl.z)
        }
        // The zero-radius drop: the highest surface under the point
        // in the projection frame. It uses the same containment test
        // as `point_is_over_mesh_xy`, so a mesh hole is a gap here too.
        ProjectProjection::Point => surface_z_at(pt.x, pt.y, mesh, index),
    }
}

/// The tip Z of each point of `points`, in order. Each point is
/// independent, so the parallel path gives the same values in the same
/// order as the serial path. Both paths poll `cancel` before the first
/// point and then once per batch (parallel) or stride (serial).
fn project_tip_zs(
    points: &[P2],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    projection: ProjectProjection,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Option<f64>>, Cancelled> {
    let mut out = Vec::with_capacity(points.len());

    #[cfg(feature = "parallel")]
    for chunk in points.chunks(PARALLEL_CHUNK) {
        check_cancel(cancel)?;
        out.par_extend(
            chunk
                .par_iter()
                .map(|pt| surface_tip_z(pt, mesh, index, cutter, projection)),
        );
    }

    #[cfg(not(feature = "parallel"))]
    for (i, pt) in points.iter().enumerate() {
        if i % CANCEL_POLL_STRIDE == 0 {
            check_cancel(cancel)?;
        }
        out.push(surface_tip_z(pt, mesh, index, cutter, projection));
    }

    Ok(out)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// P29-9 9b: one row per frame of the sign rule (`P29-9_DESIGN.md`
    /// section 16.2): from above, from below on a flipped setup, and from
    /// below on a setup that is not flipped.
    #[test]
    fn the_tip_depth_rule_has_one_row_per_frame() {
        let d = 2.417_820_455;
        assert_eq!(
            tip_depth_into_material_mm(d, ProjectDirection::FromAbove, false),
            d
        );
        assert_eq!(
            tip_depth_into_material_mm(d, ProjectDirection::FromAbove, true),
            d
        );
        assert_eq!(
            tip_depth_into_material_mm(d, ProjectDirection::FromBelow, true),
            d
        );
        assert_eq!(
            tip_depth_into_material_mm(-d, ProjectDirection::FromBelow, false),
            d
        );
    }

    #[test]
    fn test_resample_polyline_basic() {
        // A straight line from (0,0) to (10,0) resampled at spacing 3.0
        let pts = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)];
        let resampled = resample_polyline(&pts, 3.0);

        // Should get points at 0, 3, 6, 9, 10
        assert_eq!(resampled.len(), 5);
        assert!((resampled[0].x - 0.0).abs() < 1e-9);
        assert!((resampled[1].x - 3.0).abs() < 1e-9);
        assert!((resampled[2].x - 6.0).abs() < 1e-9);
        assert!((resampled[3].x - 9.0).abs() < 1e-9);
        assert!((resampled[4].x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_resample_polyline_multi_segment() {
        // L-shaped path: (0,0) -> (4,0) -> (4,4), total length 8, spacing 3
        let pts = vec![P2::new(0.0, 0.0), P2::new(4.0, 0.0), P2::new(4.0, 4.0)];
        let resampled = resample_polyline(&pts, 3.0);

        // Samples at distance 0, 3, 6 along the path, plus endpoint at distance 8
        assert_eq!(resampled.len(), 4);
        assert!((resampled[0].x - 0.0).abs() < 1e-9);
        assert!((resampled[0].y - 0.0).abs() < 1e-9);
        // distance 3 is at (3, 0)
        assert!((resampled[1].x - 3.0).abs() < 1e-9);
        assert!((resampled[1].y - 0.0).abs() < 1e-9);
        // distance 6 is at (4, 2)
        assert!((resampled[2].x - 4.0).abs() < 1e-9);
        assert!((resampled[2].y - 2.0).abs() < 1e-9);
        // endpoint at (4, 4)
        assert!((resampled[3].x - 4.0).abs() < 1e-9);
        assert!((resampled[3].y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_resample_single_point() {
        let pts = vec![P2::new(5.0, 5.0)];
        let resampled = resample_polyline(&pts, 1.0);
        assert_eq!(resampled.len(), 1);
    }

    #[test]
    fn test_resample_zero_spacing() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)];
        let resampled = resample_polyline(&pts, 0.0);
        // With zero spacing, returns original points
        assert_eq!(resampled.len(), 2);
    }

    #[test]
    fn test_close_ring() {
        let ring = vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)];
        let closed = close_ring(&ring);
        assert_eq!(closed.len(), 4);
        assert!((closed[3].x - 0.0).abs() < 1e-9);
        assert!((closed[3].y - 0.0).abs() < 1e-9);
    }

    // ── Point projection ────────────────────────────────────────────

    /// Valley half-width (mm) of the synthetic surface, and its wall slope.
    const VALLEY_HALF_WIDTH: f64 = 10.0;
    /// Wall slope: tan(60°). The 90° V-bit flank is at 45° from the
    /// horizontal, so the cone touches both walls before the tip reaches
    /// the crease.
    const WALL_SLOPE: f64 = 1.732_050_807_568_877_2;
    /// Crease Z of the original surface; the walls fall away from it.
    const CREASE_Z: f64 = 5.0;

    /// The original surface: `z = CREASE_Z - WALL_SLOPE * |x|`. Seen from
    /// below, this is a sharp V groove whose walls are at 60°. The standalone
    /// `FromBelow` arm flips it, so the drop cutter sees a V valley.
    /// The projected curve through the kernel the Curve runner calls.
    fn project(
        polygon: &Polygon2,
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        params: &ProjectCurveParams,
    ) -> Toolpath {
        crate::interrupt::run_uncancellable(|cancel| {
            project_curve_toolpath_with_cancel(polygon, mesh, index, cutter, params, cancel)
        })
    }

    fn original_surface_z(x: f64) -> f64 {
        CREASE_Z - WALL_SLOPE * x.abs()
    }

    /// A heightfield of the surface above, with a vertex column on the
    /// crease (x = 0) so each cell is planar. 1 mm cells, upward winding.
    fn valley_mesh() -> TriangleMesh {
        let n = 20_u32;
        let mut vertices = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let x = -VALLEY_HALF_WIDTH + f64::from(i);
                let y = -VALLEY_HALF_WIDTH + f64::from(j);
                vertices.push(P3::new(x, y, original_surface_z(x)));
            }
        }
        let mut triangles = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let v00 = j * (n + 1) + i;
                let v10 = v00 + 1;
                let v01 = v00 + n + 1;
                let v11 = v01 + 1;
                triangles.push([v00, v10, v11]);
                triangles.push([v00, v11, v01]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// The fed points of a toolpath: every `Linear` target. The plunge
    /// target is the first chain point, so the list is the chain points.
    fn fed_points(tp: &Toolpath) -> Vec<P3> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target)
            .collect()
    }

    fn valley_params(
        projection: ProjectProjection,
        direction: ProjectDirection,
        depth: f64,
    ) -> ProjectCurveParams {
        // F102: the engine has no `Default` (its values were not the
        // shipped `ProjectCurveConfig` ones); a test names every field.
        ProjectCurveParams {
            depth,
            point_spacing: 0.5,
            safe_z: 30.0,
            direction,
            projection,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            tool_radius: 0.0,
            side: ProjectSide::Center,
            setup_z_flipped: false,
        }
    }

    /// The test curve. It runs along the crease, and its crease vertices
    /// are NOT a whole number of spacings from the start (3.3 mm, then
    /// 5.7 mm), so the arc-length resample of `Cutter` skips them.
    fn valley_curve() -> Polygon2 {
        Polygon2::open_path(vec![
            P2::new(-3.3, -2.0),
            P2::new(0.0, -2.0),
            P2::new(0.0, 3.7),
            P2::new(2.9, 3.7),
        ])
    }

    /// On a steep V valley cut from the far side, Point puts the tip at the
    /// surface Z plus the depth at every sample, and every curve vertex is a
    /// sample. Cutter, on the same curve, stops the cone on the walls, so
    /// its tip is lower in world Z (past the surface by more than the depth)
    /// by the cone interference: for a 90° cone of radius 3 mm in 60° walls,
    /// 3 * (tan 60° - 1) = 2.196 mm.
    #[test]
    fn point_projection_follows_the_surface_where_the_cutter_cannot() {
        let mesh = valley_mesh();
        let index = SpatialIndex::build_auto(&mesh);
        let vbit = crate::tool::VBitEndmill::new(6.0, 90.0, 20.0).unwrap();
        let depth = -0.4;

        // From below, standalone: the case of the RivMap back-side V lines.
        let point = project(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(ProjectProjection::Point, ProjectDirection::FromBelow, depth),
        );
        let pts = fed_points(&point);
        assert!(
            pts.len() > 20,
            "Point emits the sampled curve, got {}",
            pts.len()
        );
        for p in &pts {
            let want = original_surface_z(p.x) + depth;
            // P29-9 9b: from below on a setup that is not flipped, the
            // tip is `-depth` into the material from the cut side.
            assert_eq!(
                want.to_bits(),
                (original_surface_z(p.x)
                    - tip_depth_into_material_mm(depth, ProjectDirection::FromBelow, false))
                .to_bits()
            );
            assert!(
                (p.z - want).abs() < 1e-6,
                "Point tip at ({:.3}, {:.3}) is {:.9}, want surface + depth = {want:.9}",
                p.x,
                p.y,
                p.z
            );
        }
        for v in &valley_curve().exterior {
            assert!(
                pts.iter().any(|p| (p.x - v.x).hypot(p.y - v.y) < 1e-12),
                "curve vertex ({}, {}) is a Point sample",
                v.x,
                v.y
            );
        }
        // No step between samples is longer than the spacing.
        for w in pts.windows(2) {
            assert!((w[1].x - w[0].x).hypot(w[1].y - w[0].y) <= 0.5 + 1e-12);
        }

        let cutter = project(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(
                ProjectProjection::Cutter,
                ProjectDirection::FromBelow,
                depth,
            ),
        );
        let interference = 3.0 * (WALL_SLOPE - 1.0);
        let crease: Vec<P3> = fed_points(&cutter)
            .into_iter()
            .filter(|p| p.x.abs() < 1e-9 && p.y > -1.0 && p.y < 2.7)
            .collect();
        assert!(!crease.is_empty(), "Cutter has samples on the crease");
        for p in &crease {
            let point_z = CREASE_Z + depth;
            assert!(
                (p.z - (point_z - interference)).abs() < 1e-6,
                "Cutter tip on the crease is {:.6}; Point gives {point_z:.6}; the cone \
                 interference is {interference:.6} mm",
                p.z
            );
        }

        // From above, the same rule: tip = surface - depth at every sample.
        let above = project(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(ProjectProjection::Point, ProjectDirection::FromAbove, 0.4),
        );
        for p in fed_points(&above) {
            let want = original_surface_z(p.x) - 0.4;
            // P29-9 9b: from above, the tip is `depth` into the material.
            assert_eq!(
                want.to_bits(),
                (original_surface_z(p.x)
                    - tip_depth_into_material_mm(0.4, ProjectDirection::FromAbove, false))
                .to_bits()
            );
            assert!(
                (p.z - want).abs() < 1e-6,
                "FromAbove Point tip {} != {want}",
                p.z
            );
        }
    }

    /// On a flat surface the two modes agree: with curve vertices on the
    /// spacing grid both modes take the same samples, and the cutter
    /// touches the plane with its tip.
    #[test]
    fn point_and_cutter_agree_on_a_flat_surface() {
        let mesh = crate::mesh::make_test_flat(100.0);
        let index = SpatialIndex::build_auto(&mesh);
        let vbit = crate::tool::VBitEndmill::new(6.0, 20.0, 20.0).unwrap();
        let curve = Polygon2::open_path(vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)]);
        for (direction, depth) in [
            (ProjectDirection::FromAbove, 1.0),
            (ProjectDirection::FromBelow, -2.2685),
        ] {
            let run = |projection| {
                fed_points(&project(
                    &curve,
                    &mesh,
                    &index,
                    &vbit,
                    &valley_params(projection, direction, depth),
                ))
            };
            let point = run(ProjectProjection::Point);
            let cutter = run(ProjectProjection::Cutter);
            assert_eq!(point.len(), 21, "{direction:?}: 10 mm at 0.5 mm, both ends");
            assert_eq!(point.len(), cutter.len(), "{direction:?}: same samples");
            for (a, b) in point.iter().zip(&cutter) {
                assert!((a - b).norm() < 1e-9, "{direction:?}: {a} != {b}");
            }
        }
    }

    // ── P33: the depth ladder ───────────────────────────────────────

    /// P33, prediction P-A3: a tip ABOVE the model surface (a negative
    /// depth, the river channel of the operator job). The flat surface is
    /// at Z 0, the depth is -4.0 (the tip at Z 4), the op top is Z 10 and
    /// the depth per pass is 2.5. The total is 6, so the one ladder gives
    /// `ceil(6 / 2.5) = 3` equal steps of 2.0: the passes cut at Z 8, 6, 4.
    #[test]
    fn the_ladder_steps_from_the_op_top_to_the_full_depth_p33() {
        let mesh = crate::mesh::make_test_flat(100.0);
        let index = SpatialIndex::build_auto(&mesh);
        let tool = crate::tool::FlatEndmill::new(6.0, 25.0);
        let curve = Polygon2::open_path(vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)]);
        let params = valley_params(ProjectProjection::Point, ProjectDirection::FromAbove, -4.0);
        let planned = project(&curve, &mesh, &index, &tool, &params);
        let full_depth = fed_points(&planned);
        assert!(full_depth.iter().all(|p| (p.z - 4.0).abs() < 1e-9));

        let passes = ladder_passes(
            planned.clone(),
            PassLadder {
                top_z: 10.0,
                depth_per_pass: 2.5,
            },
            &params,
        );
        let zs: Vec<f64> = passes
            .iter()
            .map(|pass| {
                let pts = fed_points(pass);
                assert_eq!(pts.len(), full_depth.len(), "each pass is the whole curve");
                assert!(pts.iter().all(|p| (p.z - pts[0].z).abs() < 1e-9));
                pts[0].z
            })
            .collect();
        assert_eq!(zs.len(), 3, "passes at {zs:?}");
        for (got, want) in zs.iter().zip([8.0, 6.0, 4.0]) {
            assert!((got - want).abs() < 1e-9, "passes at {zs:?}");
        }
        assert_eq!(passes[2].moves.len(), planned.moves.len());

        // One step reaches the full depth: one pass, the planned path.
        let one = ladder_passes(
            planned.clone(),
            PassLadder {
                top_z: 10.0,
                depth_per_pass: 6.0,
            },
            &params,
        );
        assert_eq!(one.len(), 1);
        // The path is at or above the op top: an empty ladder, one pass.
        let above = ladder_passes(
            planned.clone(),
            PassLadder {
                top_z: 4.0,
                depth_per_pass: 1.0,
            },
            &params,
        );
        assert_eq!(above.len(), 1);
        assert_eq!(above[0].moves.len(), planned.moves.len());
    }

    /// P33, prediction P-A4: on a surface that is not flat, an earlier
    /// pass feeds only below the op top, and no pass cuts more than one
    /// step: each fed point of a pass is within one step of the op top, or
    /// the pass before it cut the same XY one step higher.
    #[test]
    fn no_pass_cuts_more_than_one_step_p33() {
        let mesh = valley_mesh();
        let index = SpatialIndex::build_auto(&mesh);
        let tool = crate::tool::FlatEndmill::new(6.0, 25.0);
        let params = valley_params(ProjectProjection::Point, ProjectDirection::FromAbove, 0.4);
        let planned = project(&valley_curve(), &mesh, &index, &tool, &params);
        let full_depth = fed_points(&planned);
        let lowest = full_depth.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
        // The lowest tip is the curve end at x = -3.3: the surface less 0.4.
        assert!((lowest - (original_surface_z(-3.3) - 0.4)).abs() < 1e-6);

        let top_z = CREASE_Z;
        let depth_per_pass = 2.0;
        // (5 + 1.116) / 2.0 -> 4 passes.
        let count = ((top_z - lowest) / depth_per_pass).ceil();
        let step = (top_z - lowest) / count;
        let passes = ladder_passes(
            planned.clone(),
            PassLadder {
                top_z,
                depth_per_pass,
            },
            &params,
        );
        assert_eq!(passes.len(), 4, "count {count}");
        let same_xy = |a: &P3, b: &P3| (a.x - b.x).hypot(a.y - b.y) < 1e-12;
        for (i, pass) in passes.iter().enumerate() {
            let pts = fed_points(pass);
            assert!(!pts.is_empty(), "pass {i} cuts");
            if i + 1 < passes.len() {
                assert!(
                    pts.len() < full_depth.len(),
                    "pass {i} leaves out the samples above the op top"
                );
                assert!(
                    pts.iter().all(|p| p.z < top_z),
                    "pass {i} feeds above the top"
                );
            }
            for p in &pts {
                let from_top = top_z - p.z <= step + 1e-9;
                let below_the_pass_before = i > 0
                    && fed_points(&passes[i - 1])
                        .iter()
                        .any(|q| same_xy(p, q) && (q.z - p.z - step).abs() < 1e-9);
                assert!(
                    from_top || below_the_pass_before,
                    "pass {i} cuts more than one step at ({}, {}, {})",
                    p.x,
                    p.y,
                    p.z
                );
            }
            // Each chain enters as a chain of today.
            for m in &pass.moves {
                if matches!(m.move_type, crate::toolpath::MoveType::Rapid) {
                    assert!((m.target.z - params.safe_z).abs() < 1e-9);
                }
            }
        }
        // The last pass is the planned path, move for move.
        let last = &passes[3];
        assert_eq!(last.moves.len(), planned.moves.len());
        for (a, b) in last.moves.iter().zip(&planned.moves) {
            assert_eq!(a.target, b.target);
            assert_eq!(a.move_type, b.move_type);
        }
    }

    #[test]
    fn point_samples_keep_vertices_and_cap_the_step() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(1.3, 0.0), P2::new(1.3, 0.2)];
        let s = sample_vertices_and_steps(&pts, 0.5);
        // 1.3 mm -> 3 steps of 0.4333; 0.2 mm -> 1 step.
        assert_eq!(s.len(), 5);
        assert!((s[3] - P2::new(1.3, 0.0)).norm() < 1e-12);
        assert!((s[4] - P2::new(1.3, 0.2)).norm() < 1e-12);
        assert!((s[1].x - 1.3 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn default_projection_is_cutter() {
        // F102: the shipped default is the config's, not an engine copy.
        assert_eq!(
            ProjectProjection::from(
                crate::compute::operation_configs::ProjectCurveConfig::default().projection
            ),
            ProjectProjection::Cutter
        );
        assert_eq!(ProjectProjection::default(), ProjectProjection::Cutter);
    }

    #[test]
    fn test_close_ring_already_closed() {
        let ring = vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 0.0),
        ];
        let closed = close_ring(&ring);
        assert_eq!(closed.len(), 4); // Should not add duplicate
    }
}
