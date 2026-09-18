//! Push-cutter algorithm — pushes a cutter horizontally along a Fiber at constant Z.
//!
//! For each triangle, computes the interval(s) on the fiber where the cutter
//! would gouge (contact the triangle). Three contact types are tested:
//! vertex, facet, and edge — analogous to drop-cutter but in the horizontal plane.
//!
//! Used by the waterline algorithm to find contours at constant Z heights.

use crate::geo::{P3, Triangle};
use crate::geometry::fiber::{Fiber, Interval};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

/// Push a cutter along a fiber against a single triangle.
/// Adds any gouge intervals to the fiber.
pub fn push_cutter_triangle(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    vertex_push(fiber, tri, cutter);
    facet_push(fiber, tri, cutter);
    edge_push(fiber, tri, cutter);
}

/// Millimetres of headroom added to the fiber's lateral query band on top of
/// the cutter's envelope radius (G1, `PERF_REVIEW.md`).
///
/// The band bound below is exact in real arithmetic — every contact test in
/// this module rejects geometry further than `envelope_radius` from the fiber
/// segment. But three of those tests carry their own comparison slack, and a
/// bound that is tight to the last ULP would turn one of them into a
/// *different answer* rather than a faster one:
///
/// - [`vertex_push`] and [`edge_push_single`] reject at `perp_dist > w +
///   1e-10`, so a vertex exactly `envelope_radius` away is **accepted** and
///   contributes a zero-width interval;
/// - [`facet_push`] accepts the fiber parameter over `-1e-8..=1.0 + 1e-8`, so
///   its contact point can sit a hair beyond either fiber endpoint —
///   `1e-8 · fiber_length`, i.e. 1e-6 mm on a 100 mm fiber;
/// - `Triangle::contains_point_xy` accepts barycentric coordinates down to
///   `-1e-8`, which is a distance of `1e-8 · triangle_scale` outside the
///   triangle — 1e-4 mm even on a 10 m triangle.
///
/// 1e-3 mm clears all three by at least an order of magnitude while costing
/// nothing: the band is `2·radius` wide, so a Ø6 cutter's band grows by
/// 0.03%, and the index quantises to whole cells (≥ 0.1 mm) anyway, so on
/// almost every query it does not change the cell range at all.
pub(crate) const PUSH_QUERY_SLACK_MM: f64 = 1e-3;

/// How far, laterally, a cutter swept along a fiber can reach off the fiber
/// line — the half-width of the band the spatial index needs to return.
///
/// `envelope_radius_mm()` is the documented "maximum lateral extent any part
/// of the cutter sweeps, at any height" (`tool/mod.rs`), which bounds
/// [`MillingCutter::width_at_height`] — the quantity `vertex_push` and
/// `edge_push_single` compare their perpendicular distance against. The facet
/// contact offsets by `xy_normal_length · n̂_xy + normal_length · n`, whose XY
/// magnitude is at most `xy_normal_length + normal_length`; that sum equals
/// the envelope radius for every shipped shape (flat `R+0`, ball `0+R`,
/// bullnose `r1+r2 = R`, V-bit `R+0`, tapered ball `0 + r_ball ≤ R`), but it
/// is taken as a `max` here rather than assumed, so a future shape cannot
/// silently under-size the band.
#[must_use]
pub fn fiber_lateral_reach_mm(cutter: &dyn MillingCutter) -> f64 {
    let envelope = cutter.envelope_radius_mm();
    debug_assert!(
        {
            // The envelope contract, checked rather than trusted: sample the
            // profile and confirm nothing pokes outside it.
            let len = cutter.length().max(0.0);
            (0..=16).all(|i| {
                let h = len * (i as f64 / 16.0);
                cutter.width_at_height(h) <= envelope + 1e-9
            })
        },
        "cutter profile exceeds its own envelope_radius_mm; fiber band would under-query"
    );
    envelope.max(cutter.xy_normal_length() + cutter.normal_length()) + PUSH_QUERY_SLACK_MM
}

/// Push a cutter along a fiber against all triangles near it using the spatial index.
///
/// Allocates a candidate buffer and dedup scratch per call. Callers with many
/// fibers should use [`push_cutter_fiber_into`] (or [`batch_push_cutter`],
/// which already does) and hand the same buffers back each time.
pub fn push_cutter_fiber(
    fiber: &mut Fiber,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) {
    let mut scratch = QueryScratch::new();
    let mut candidates = Vec::new();
    push_cutter_fiber_into(fiber, mesh, index, cutter, &mut scratch, &mut candidates);
}

/// Collect the spatial-index candidates for `fiber` into `candidates`.
///
/// **G1**: the query is the fiber's own XY bounding box inflated by
/// [`fiber_lateral_reach_mm`] — for an X-fiber at row `y` that is the band
/// `y ± reach`, not a square of side `fiber_length + 2·reach` centred on the
/// fiber's midpoint. The old square was sized by the fiber's *length*, and
/// waterline fibers span the whole mesh bbox, so it selected every cell in
/// the index and pruned nothing whatsoever.
///
/// Soundness: contact is only possible where some point of the triangle lies
/// within `envelope_radius` of the swept cutter centre, and the cutter centre
/// never leaves the fiber segment — so a triangle whose XY bbox misses the
/// inflated segment bbox cannot contribute. Every push test in this module
/// rejects on exactly that distance (`perp_dist > w`, `contains_point_xy` of a
/// point offset by at most the envelope radius); `PUSH_QUERY_SLACK_MM`
/// documents the comparison epsilons that headroom absorbs. The X extent is
/// unchanged from the old query — `[x_min - reach, x_max + reach]` was already
/// exactly what the square gave along the fiber.
pub fn fiber_query_candidates(
    fiber: &Fiber,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    scratch: &mut QueryScratch,
    candidates: &mut Vec<usize>,
) {
    let w = FiberWindow::of(fiber, cutter);
    index.query_rect_into(w.x_min, w.x_max, w.y_min, w.y_max, scratch, candidates);
}

/// The fiber's XY reach window: its own segment bbox inflated by
/// [`fiber_lateral_reach_mm`]. Used both to size the index query and to reject
/// individual candidates the cell-granular query rounded in.
#[derive(Debug, Clone, Copy)]
struct FiberWindow {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

impl FiberWindow {
    fn of(fiber: &Fiber, cutter: &dyn MillingCutter) -> Self {
        let reach = fiber_lateral_reach_mm(cutter);
        Self {
            x_min: fiber.p1.x.min(fiber.p2.x) - reach,
            x_max: fiber.p1.x.max(fiber.p2.x) + reach,
            y_min: fiber.p1.y.min(fiber.p2.y) - reach,
            y_max: fiber.p1.y.max(fiber.p2.y) + reach,
        }
    }

    /// Does this triangle's XY bounding box meet the window at all?
    ///
    /// `Triangle::bbox` is built from the three vertices at construction
    /// (`geo.rs`), so this costs two loads and four compares and is exact —
    /// where the index query is quantised to whole cells and therefore
    /// admits up to one cell row of slop on each side of the band.
    #[inline]
    fn admits(&self, tri: &Triangle) -> bool {
        tri.bbox.min.x <= self.x_max
            && tri.bbox.max.x >= self.x_min
            && tri.bbox.min.y <= self.y_max
            && tri.bbox.max.y >= self.y_min
    }
}

/// Buffer-reusing form of [`push_cutter_fiber`].
///
/// `candidates` is overwritten; `scratch` must be a [`QueryScratch`] the
/// caller keeps alive across calls (it is left zeroed, so it can be shared
/// freely between successive fibers on one thread).
pub fn push_cutter_fiber_into(
    fiber: &mut Fiber,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    scratch: &mut QueryScratch,
    candidates: &mut Vec<usize>,
) {
    fiber_query_candidates(fiber, index, cutter, scratch, candidates);
    push_cutter_fiber_over(fiber, mesh, cutter, candidates);
}

/// Run the contact tests for `fiber` over an already-collected candidate list.
///
/// Split out from [`push_cutter_fiber_into`] because the candidate list for a
/// given fiber ROW is identical at every Z level — the waterline fiber grid's
/// XY does not move as the plane descends — so a multi-level caller can pay
/// for the query once and replay the contact tests per level.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(crate) fn push_cutter_fiber_over(
    fiber: &mut Fiber,
    mesh: &TriangleMesh,
    cutter: &dyn MillingCutter,
    candidates: &[usize],
) {
    let z_min = fiber.z();
    let z_max = fiber.z() + cutter.length();
    let window = FiberWindow::of(fiber, cutter);

    for &tri_idx in candidates {
        let tri = &mesh.faces[tri_idx];
        // Quick Z check: skip triangles entirely above or below the cutter at this Z
        let tri_z_min = tri.v[0].z.min(tri.v[1].z).min(tri.v[2].z);
        let tri_z_max = tri.v[0].z.max(tri.v[1].z).max(tri.v[2].z);
        if tri_z_min > z_max || tri_z_max < z_min {
            continue;
        }
        // Exact XY reject, one cell finer than the query could be. Same bound
        // as the query window, so it can only drop candidates the contact
        // tests below would have rejected anyway.
        if !window.admits(tri) {
            continue;
        }
        push_cutter_triangle(fiber, tri, cutter);
    }
}

/// Batch push-cutter: process multiple fibers against the mesh.
pub fn batch_push_cutter(
    fibers: &mut [Fiber],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) {
    let _ =
        batch_push_cutter_with_cancel(fibers, mesh, index, cutter, &crate::interrupt::NeverCancel);
}

pub fn batch_push_cutter_with_cancel(
    fibers: &mut [Fiber],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    use rayon::prelude::*;

    // Process fibers in parallel chunks. Check cancel between chunks.
    const CHUNK_SIZE: usize = 64;
    for chunk in fibers.chunks_mut(CHUNK_SIZE) {
        check_cancel(cancel)?;
        // `for_each_init` gives each rayon worker its own candidate Vec and
        // dedup bitset, reused across every fiber that worker handles. The
        // per-query allocation was measured at only 0.99× on the
        // classification grid (`CLASSIFICATION_PERF_STUDY.md`) so this is not
        // where the win is — but with the band query in place the candidate
        // lists are small and the buffers are genuinely free to keep.
        chunk
            .par_iter_mut()
            .for_each_init(QueryState::default, |state, fiber| {
                push_cutter_fiber_into(
                    fiber,
                    mesh,
                    index,
                    cutter,
                    &mut state.scratch,
                    &mut state.candidates,
                );
            });
    }

    Ok(())
}

/// Per-worker buffers for [`batch_push_cutter_with_cancel`].
#[derive(Default)]
struct QueryState {
    scratch: QueryScratch,
    candidates: Vec<usize>,
}

/// Vertex push: for each triangle vertex, compute the interval on the fiber
/// where the cutter would contact that vertex.
fn vertex_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    for v in &tri.v {
        // Height of vertex above fiber Z
        let h = v.z - fiber.z();
        if h < -1e-10 || h > cutter.length() {
            continue; // vertex below fiber or above cutter
        }

        // Cutter width at this height
        let w = cutter.width_at_height(h);
        if w < 1e-15 {
            continue;
        }

        // Perpendicular distance from vertex to fiber line (in XY)
        let fiber_dx = fiber.p2.x - fiber.p1.x;
        let fiber_dy = fiber.p2.y - fiber.p1.y;
        let fiber_len_sq = fiber_dx * fiber_dx + fiber_dy * fiber_dy;
        if fiber_len_sq < 1e-20 {
            return;
        }
        let fiber_len = fiber_len_sq.sqrt();

        // Signed perpendicular distance
        let qx = v.x - fiber.p1.x;
        let qy = v.y - fiber.p1.y;
        let perp_dist = (qx * fiber_dy - qy * fiber_dx).abs() / fiber_len;

        if perp_dist > w + 1e-10 {
            continue; // vertex too far from fiber
        }

        // t parameter for the projection of vertex onto fiber
        let t_proj = (qx * fiber_dx + qy * fiber_dy) / fiber_len_sq;

        // Half-width along fiber at this perpendicular distance
        let half_len = (w * w - perp_dist * perp_dist).max(0.0).sqrt() / fiber_len;

        let t_lower = t_proj - half_len;
        let t_upper = t_proj + half_len;

        if t_upper >= 0.0 && t_lower <= 1.0 {
            fiber.add_interval(Interval::new(t_lower, t_upper));
        }
    }
}

/// Facet push: find the interval where the cutter contacts the triangle face.
fn facet_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    let n = &tri.normal;

    // A vertical facet has its own arm: the contact formula below divides
    // by `n.z`, and the side of the cutter meets the facet's interior, not
    // its tip (R9).
    if n.z.abs() < 1e-12 {
        vertical_facet_push(fiber, tri, cutter);
        return;
    }

    let r1 = cutter.xy_normal_length();
    let r2 = cutter.normal_length();
    let ch = cutter.center_height();

    let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
    let (xy_nx, xy_ny) = if nxy_len > 1e-15 {
        (n.x / nxy_len, n.y / nxy_len)
    } else {
        (0.0, 0.0)
    };

    // For each point on the fiber at parameter t:
    //   CL = fiber.point(t) = (p1.x + t*dx, p1.y + t*dy, z)
    //   CC = CL - r1*(xy_nx, xy_ny) - r2*(n.x, n.y)
    //
    // CC must lie on the triangle plane AND inside the triangle.
    // We solve for t where cc_z (from plane equation) gives:
    //   tip_z = cc_z + r2*n.z - ch
    // and tip_z = fiber.z (constant Z fiber)
    //
    // This gives a single t value (one contact point for the whole fiber).

    let fiber_dx = fiber.p2.x - fiber.p1.x;
    let fiber_dy = fiber.p2.y - fiber.p1.y;
    let z = fiber.z();

    // CC in terms of t
    let cc_x0 = fiber.p1.x - r1 * xy_nx - r2 * n.x;
    let cc_y0 = fiber.p1.y - r1 * xy_ny - r2 * n.y;
    let cc_dx = fiber_dx;
    let cc_dy = fiber_dy;

    // CC on triangle plane: n.x*(cc_x - v0.x) + n.y*(cc_y - v0.y) + n.z*(cc_z - v0.z) = 0
    // => cc_z = v0.z - (n.x*(cc_x - v0.x) + n.y*(cc_y - v0.y)) / n.z
    let v0 = &tri.v[0];
    let num0 = n.x * (cc_x0 - v0.x) + n.y * (cc_y0 - v0.y);
    let num_dt = n.x * cc_dx + n.y * cc_dy;

    // cc_z(t) = v0.z - (num0 + num_dt * t) / n.z
    // tip_z(t) = cc_z(t) + r2 * n.z - ch
    // We want tip_z(t) = z
    // => v0.z - (num0 + num_dt*t)/n.z + r2*n.z - ch = z
    // => -(num0 + num_dt*t)/n.z = z - v0.z - r2*n.z + ch
    // => num0 + num_dt*t = -n.z * (z - v0.z - r2*n.z + ch)
    // => t = (-n.z * (z - v0.z - r2*n.z + ch) - num0) / num_dt

    if num_dt.abs() < 1e-15 {
        // Fiber is parallel to the facet intersection — check if it's at the right Z
        let cc_z = v0.z - num0 / n.z;
        let tip_z = cc_z + r2 * n.z - ch;
        if (tip_z - z).abs() > 1e-8 {
            return;
        }
        // The entire fiber could be in contact — check endpoints
        for t in [0.0, 1.0] {
            let cc_x = cc_x0 + t * cc_dx;
            let cc_y = cc_y0 + t * cc_dy;
            if tri.contains_point_xy(cc_x, cc_y) {
                // The whole fiber is potentially in contact
                fiber.add_interval(Interval::new(0.0, 1.0));
                return;
            }
        }
        return;
    }

    let rhs = -n.z * (z - v0.z - r2 * n.z + ch) - num0;
    let t = rhs / num_dt;

    if !(-1e-8..=1.0 + 1e-8).contains(&t) {
        return;
    }

    let cc_x = cc_x0 + t * cc_dx;
    let cc_y = cc_y0 + t * cc_dy;

    if !tri.contains_point_xy(cc_x, cc_y) {
        return;
    }

    // The facet contact is a single point on the fiber, but we add a tiny interval
    // to mark it as blocked
    let eps = 1e-6;
    fiber.add_interval(Interval::new(t - eps, t + eps));
}

/// Push the side of the cutter against a vertical facet (R9, the Corne case,
/// `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.5).
///
/// `facet_push` used to return at once for a facet with `n.z == 0` ("no
/// horizontal contact"). A vertical wall was therefore seen only through its
/// vertices and edges. That is enough while every wall edge lies inside the
/// slab `[z, z + length]` and the edge push finds it. It is not enough when
/// the slab clips the facet: a wall taller than the cutting length, or a
/// level through the middle of a tall wall triangle, leaves a clipped
/// polygon whose top or bottom side is a slab cut, not a mesh edge, and no
/// test covered its interior. A fiber through the middle of such a wall came
/// back free, and the waterline walked through the wall.
///
/// This arm clips the facet to the slab, projects the clipped polygon to XY
/// (one segment, because the facet is vertical) and pushes that segment with
/// the widest cutter profile over the clipped height range
/// ([`push_segment_xy`]). For a constant-width cutter that is exact. For a
/// profile that widens with height it is conservative: the whole segment is
/// pushed with the width at the top of the clipped range, so the contour
/// keeps extra distance from a vertical facet whose outline narrows upward.
/// A rectangular wall is two triangles whose clipped polygons together span
/// the full width at every height, so on a wall the union is exact for
/// every profile; only a gable-shaped vertical facet sees the margin.
fn vertical_facet_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    let z_lo = fiber.z();
    let z_hi = z_lo + cutter.length();
    let tri_z_min = tri.v[0].z.min(tri.v[1].z).min(tri.v[2].z);
    let tri_z_max = tri.v[0].z.max(tri.v[1].z).max(tri.v[2].z);
    if tri_z_max < z_lo - 1e-10 || tri_z_min > z_hi + 1e-10 {
        return;
    }

    let n = &tri.normal;
    let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
    if nxy_len < 1e-12 {
        return;
    }
    // Unit direction along the facet's XY trace.
    let dir_x = -n.y / nxy_len;
    let dir_y = n.x / nxy_len;

    // The clipped polygon's vertices: the triangle vertices inside the slab
    // and the crossings of its edges with the two slab planes. Every one of
    // them lies on the facet's XY trace, so the two extremes along `dir`
    // bound the projected segment.
    let mut lo: Option<(f64, (f64, f64))> = None;
    let mut hi: Option<(f64, (f64, f64))> = None;
    let mut take = |x: f64, y: f64| {
        let s = x * dir_x + y * dir_y;
        if lo.is_none_or(|(s_lo, _)| s < s_lo) {
            lo = Some((s, (x, y)));
        }
        if hi.is_none_or(|(s_hi, _)| s > s_hi) {
            hi = Some((s, (x, y)));
        }
    };
    let edges = [
        (&tri.v[0], &tri.v[1]),
        (&tri.v[1], &tri.v[2]),
        (&tri.v[2], &tri.v[0]),
    ];
    for (p, q) in edges {
        if p.z >= z_lo - 1e-10 && p.z <= z_hi + 1e-10 {
            take(p.x, p.y);
        }
        for plane in [z_lo, z_hi] {
            if (p.z - plane) * (q.z - plane) < 0.0 {
                let s = (plane - p.z) / (q.z - p.z);
                take(p.x + s * (q.x - p.x), p.y + s * (q.y - p.y));
            }
        }
    }
    let (Some((_, a)), Some((_, b))) = (lo, hi) else {
        return;
    };

    // The widest profile over the clipped height range. Sampled, not
    // assumed monotone: the trait does not promise a monotone profile.
    let h_lo = tri_z_min.max(z_lo) - z_lo;
    let h_hi = tri_z_max.min(z_hi) - z_lo;
    let w = (0..=16)
        .map(|i| cutter.width_at_height(h_lo + (h_hi - h_lo) * (i as f64 / 16.0)))
        .fold(0.0_f64, f64::max);
    if w < 1e-15 {
        return;
    }
    push_segment_xy(fiber, a, b, w);
}

/// Exact blocked interval of a cutter of constant width `w`, swept along the
/// fiber, against the XY segment `a`–`b`: every `t` where the distance from
/// `fiber.point(t)` to the segment is at most `w`.
///
/// The blocked set is one interval (a disc swept along a line against a
/// convex segment). Each end of it is a contact position: either the disc
/// rim passes through a segment endpoint, or the disc is tangent to the
/// segment's interior. Both families have closed forms, so nothing is
/// sampled. Every candidate is a contact position, so it lies inside the
/// interval, and the interval is the min and max over the candidates.
fn push_segment_xy(fiber: &mut Fiber, a: (f64, f64), b: (f64, f64), w: f64) {
    let fdx = fiber.p2.x - fiber.p1.x;
    let fdy = fiber.p2.y - fiber.p1.y;
    let fiber_len_sq = fdx * fdx + fdy * fdy;
    if fiber_len_sq < 1e-20 {
        return;
    }
    let fiber_len = fiber_len_sq.sqrt();

    let mut t_min = f64::INFINITY;
    let mut t_max = f64::NEG_INFINITY;
    let mut take = |t: f64| {
        t_min = t_min.min(t);
        t_max = t_max.max(t);
    };

    // Endpoint contacts: the rim through `a` or `b`.
    for (px, py) in [a, b] {
        let qx = px - fiber.p1.x;
        let qy = py - fiber.p1.y;
        let perp = (qx * fdy - qy * fdx).abs() / fiber_len;
        if perp > w + 1e-10 {
            continue;
        }
        let t_proj = (qx * fdx + qy * fdy) / fiber_len_sq;
        let half = (w * w - perp * perp).max(0.0).sqrt() / fiber_len;
        take(t_proj - half);
        take(t_proj + half);
    }

    // Interior tangency: the fiber point at signed distance `±w` from the
    // segment's line, kept when its foot lies inside the segment.
    let ex = b.0 - a.0;
    let ey = b.1 - a.1;
    let e_len_sq = ex * ex + ey * ey;
    if e_len_sq > 1e-20 {
        let e_len = e_len_sq.sqrt();
        let g0 = ((fiber.p1.x - a.0) * ey - (fiber.p1.y - a.1) * ex) / e_len;
        let g1 = (fdx * ey - fdy * ex) / e_len;
        if g1.abs() > 1e-15 {
            for sign in [-1.0, 1.0] {
                let t = (sign * w - g0) / g1;
                let fx = fiber.p1.x + t * fdx - a.0;
                let fy = fiber.p1.y + t * fdy - a.1;
                let s = (fx * ex + fy * ey) / e_len_sq;
                if (-1e-10..=1.0 + 1e-10).contains(&s) {
                    take(t);
                }
            }
        }
    }

    if t_min <= t_max && t_max >= 0.0 && t_min <= 1.0 {
        fiber.add_interval(Interval::new(t_min, t_max));
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Edge push: for each triangle edge, compute the interval on the fiber
/// where the cutter contacts that edge.
fn edge_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    for i in 0..3 {
        let p1 = &tri.v[i];
        let p2 = &tri.v[(i + 1) % 3];
        edge_push_single(fiber, p1, p2, cutter);
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Push-cutter for a single edge against a fiber.
///
/// Finds where the cutter profile, swept along the fiber at constant Z,
/// contacts the edge. This is done by sampling contact points along the edge
/// and checking if the cutter width at the contact height covers the fiber.
///
/// **The samples cover the candidate window, not the whole edge** (R9, the
/// Corne case, `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.5).
/// The perpendicular distance from the fiber line and the height above the
/// fiber are both linear along the edge, so the part of the edge that can
/// touch the cutter at all is one closed sub-range of `s`:
/// `|perp(s)| <= envelope_radius` and `0 <= h(s) <= length`.
///
/// The nine coarse samples used to sit at `s = k/8` over the WHOLE edge, and
/// the bisection only ran between two coarse samples that disagreed. An edge
/// that crosses the fiber has a contact window about `2·w` long. On the
/// Corne tray the wall edges are 55.26 mm long (coarse pitch 6.9 mm) and the
/// 6 mm flat cutter's 6 mm window fell between two coarse samples on every
/// fiber row at that pitch: no sample made contact, no bisection ran, and
/// the edge added nothing. The fiber passed through the 3 mm wall and the
/// waterline notched into it at a 7 mm pitch on every level. The rows next
/// to a missed row lost up to 0.5 mm at the interval ends for the same
/// reason: the ends came from the bisection, not from the window.
///
/// With the samples confined to the window, both window ends are always
/// sampled, so a constant-width cutter gets its exact interval ends, and a
/// window narrower than an eighth of the edge can no longer fall between
/// two samples. For a profile that varies with height the contact set is a
/// sub-range of the window; the coarse-plus-bisection search inside the
/// window finds it as before, now at window resolution instead of edge
/// resolution. `tests/waterline_respects_a_vertical_wall_r9.rs` pins the
/// tray case.
fn edge_push_single(fiber: &mut Fiber, p1: &P3, p2: &P3, cutter: &dyn MillingCutter) {
    let z = fiber.z();

    let fiber_dx = fiber.p2.x - fiber.p1.x;
    let fiber_dy = fiber.p2.y - fiber.p1.y;
    let fiber_len_sq = fiber_dx * fiber_dx + fiber_dy * fiber_dy;
    if fiber_len_sq < 1e-20 {
        return;
    }
    let fiber_len = fiber_len_sq.sqrt();

    // Edge vector
    let ex = p2.x - p1.x;
    let ey = p2.y - p1.y;
    let ez = p2.z - p1.z;
    let edge_len_sq = ex * ex + ey * ey + ez * ez;
    if edge_len_sq < 1e-20 {
        return;
    }

    // The candidate window `[s_lo, s_hi]` of the edge parameter.
    //
    // Signed perpendicular distance of the edge point at `s` to the fiber
    // line: `g0 + g1·s`. No profile is wider than the envelope radius
    // (`fiber_lateral_reach_mm` checks that contract), so a contact needs
    // `|g0 + g1·s| <= envelope + 1e-10`, the slack `eval_at` uses. The
    // window takes half that slack, so a rounding hair at a window end
    // cannot push the end sample past the evaluator's own bound.
    let q0x = p1.x - fiber.p1.x;
    let q0y = p1.y - fiber.p1.y;
    let g0 = (q0x * fiber_dy - q0y * fiber_dx) / fiber_len;
    let g1 = (ex * fiber_dy - ey * fiber_dx) / fiber_len;
    let reach = cutter.envelope_radius_mm() + 5e-11;
    let mut s_lo = 0.0_f64;
    let mut s_hi = 1.0_f64;
    if g1.abs() < 1e-15 {
        if g0.abs() > reach {
            return;
        }
    } else {
        let a = (-reach - g0) / g1;
        let b = (reach - g0) / g1;
        s_lo = s_lo.max(a.min(b));
        s_hi = s_hi.min(a.max(b));
    }
    // Height above the fiber: `h0 + ez·s`, inside `[-1e-10, length]`. The
    // window again keeps half the slack on each side.
    let h0 = p1.z - z;
    if ez.abs() < 1e-15 {
        if h0 < -1e-10 || h0 > cutter.length() {
            return;
        }
    } else {
        let a = (-5e-11 - h0) / ez;
        let b = (cutter.length() - 5e-11 - h0) / ez;
        s_lo = s_lo.max(a.min(b));
        s_hi = s_hi.min(a.max(b));
    }
    if s_lo > s_hi {
        return;
    }
    // Map a unit sample position onto the window.
    let s_of = |u: f64| s_lo + u * (s_hi - s_lo);

    // Coarse+bisection sampling: 9 coarse samples to find contact intervals,
    // then bisect at boundaries for higher accuracy with fewer evaluations.
    let n_coarse = 8;
    let mut t_min = f64::INFINITY;
    let mut t_max = f64::NEG_INFINITY;

    // Helper: evaluate contact at parameter s along the edge.
    // Returns the (t_lo, t_hi) fiber interval if contact, else None.
    let eval_at = |s: f64| -> Option<(f64, f64)> {
        let edge_x = p1.x + s * ex;
        let edge_y = p1.y + s * ey;
        let edge_z = p1.z + s * ez;

        let h = edge_z - z;
        if h < -1e-10 || h > cutter.length() {
            return None;
        }

        let w = cutter.width_at_height(h);
        if w < 1e-15 {
            return None;
        }

        let qx = edge_x - fiber.p1.x;
        let qy = edge_y - fiber.p1.y;
        let perp_dist = (qx * fiber_dy - qy * fiber_dx).abs() / fiber_len;

        if perp_dist > w + 1e-10 {
            return None;
        }

        let t_proj = (qx * fiber_dx + qy * fiber_dy) / fiber_len_sq;
        let half_len = (w * w - perp_dist * perp_dist).max(0.0).sqrt() / fiber_len;

        let tl = t_proj - half_len;
        let tu = t_proj + half_len;

        if tu >= 0.0 && tl <= 1.0 {
            Some((tl, tu))
        } else {
            None
        }
    };

    // Phase 1: 9 coarse samples at u = 0, 1/8, 2/8, ..., 1 of the window
    let mut coarse_contact = [false; 9];
    for (i, contacted) in coarse_contact.iter_mut().enumerate().take(n_coarse + 1) {
        let u = i as f64 / n_coarse as f64;
        if let Some((tl, tu)) = eval_at(s_of(u)) {
            t_min = t_min.min(tl);
            t_max = t_max.max(tu);
            *contacted = true;
        }
    }

    // Phase 2: Bisect at boundaries (contact ↔ no-contact transitions)
    for i in 0..n_coarse {
        if coarse_contact[i] != coarse_contact[i + 1] {
            let mut u_lo = i as f64 / n_coarse as f64;
            let mut u_hi = (i + 1) as f64 / n_coarse as f64;
            // 5 bisection iterations → 1/32 of interval precision
            for _ in 0..5 {
                let u_mid = (u_lo + u_hi) * 0.5;
                let has_contact = eval_at(s_of(u_mid)).is_some();
                if has_contact == coarse_contact[i] {
                    u_lo = u_mid;
                } else {
                    u_hi = u_mid;
                }
                // Evaluate at boundary for t_min/t_max update
                if let Some((tl, tu)) = eval_at(s_of(u_mid)) {
                    t_min = t_min.min(tl);
                    t_max = t_max.max(tu);
                }
            }
        }
    }

    if t_min < t_max {
        fiber.add_interval(Interval::new(t_min, t_max));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tool::{BallEndmill, FlatEndmill};

    fn horizontal_tri(z: f64) -> Triangle {
        Triangle::new(
            P3::new(-50.0, -50.0, z),
            P3::new(50.0, -50.0, z),
            P3::new(0.0, 50.0, z),
        )
    }

    #[test]
    fn test_vertex_push_below_fiber() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 5.0, -50.0, 50.0);
        let tri = horizontal_tri(0.0);
        vertex_push(&mut fiber, &tri, &tool);
        // All vertices at z=0, fiber at z=5. h = 0 - 5 = -5 < 0. No contact.
        assert!(fiber.intervals().is_empty());
    }

    #[test]
    fn test_vertex_push_at_fiber_height() {
        let tool = FlatEndmill::new(10.0, 25.0);
        // Fiber at y=0, vertices near the fiber (within cutter radius = 5)
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        let tri = Triangle::new(
            P3::new(-10.0, 2.0, 0.0), // y=2, within R=5
            P3::new(10.0, 2.0, 0.0),
            P3::new(0.0, -2.0, 0.0),
        );
        vertex_push(&mut fiber, &tri, &tool);
        // Vertices at z=0 = fiber.z, h=0, width=R=5, perp dist=2 < 5. Should find contact.
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_vertex_push_ball_endmill() {
        let tool = BallEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        // Vertex at (0, 0, 3) — h=3, width_at_height(3) = sqrt(2*5*3 - 9) = sqrt(21) ≈ 4.58
        let tri = Triangle::new(
            P3::new(0.0, 0.0, 3.0),
            P3::new(100.0, 100.0, 100.0),
            P3::new(-100.0, 100.0, 100.0),
        );
        vertex_push(&mut fiber, &tri, &tool);
        // Only the first vertex should contribute (others above cutter)
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_facet_push_horizontal_at_fiber() {
        let tool = FlatEndmill::new(10.0, 25.0);
        // Use a big triangle that definitely contains the fiber's CC points
        let mut fiber = Fiber::new_x(0.0, 0.0, -10.0, 10.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 0.0),
            P3::new(50.0, -50.0, 0.0),
            P3::new(0.0, 50.0, 0.0),
        );
        facet_push(&mut fiber, &tri, &tool);
        // Horizontal triangle at fiber Z: flat endmill has n=(0,0,1), r1=R, r2=0
        // CC = CL (since xy_normal=0 when nxy_len=0). Fiber endpoints are inside triangle.
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_push_cutter_triangle_combines() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        let tri = horizontal_tri(0.0);
        push_cutter_triangle(&mut fiber, &tri, &tool);
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_push_cutter_fiber_with_mesh() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Fiber at z=10 (midway through hemisphere)
        let mut fiber = Fiber::new_x(0.0, 10.0, -30.0, 30.0);
        push_cutter_fiber(&mut fiber, &mesh, &index, &tool);

        // Should have intervals (the cutter contacts the hemisphere)
        assert!(
            !fiber.intervals().is_empty(),
            "Should find contacts on hemisphere at z=10"
        );
    }

    #[test]
    fn test_push_cutter_no_contact_above() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Fiber at z=25 (above hemisphere, apex at z=20)
        let mut fiber = Fiber::new_x(0.0, 25.0, -30.0, 30.0);
        push_cutter_fiber(&mut fiber, &mesh, &index, &tool);
        assert!(fiber.intervals().is_empty(), "No contact above hemisphere");
    }

    #[test]
    fn test_batch_push_cutter() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let mut fibers: Vec<Fiber> = (-5..=5)
            .map(|i| Fiber::new_x(i as f64 * 4.0, 10.0, -30.0, 30.0))
            .collect();

        batch_push_cutter(&mut fibers, &mesh, &index, &tool);

        // Center fibers should have intervals, outer ones may not
        let center_fiber = &fibers[5]; // y=0
        assert!(!center_fiber.intervals().is_empty());
    }

    #[test]
    fn test_edge_push_diagonal_flat() {
        // Known diagonal edge with flat endmill, verify interval accuracy.
        let tool = FlatEndmill::new(10.0, 25.0); // R=5
        let mut fiber = Fiber::new_x(0.0, 0.0, -30.0, 30.0);
        // Diagonal edge from (-10, -2, 0) to (10, 2, 0) at z=0 = fiber.z
        // h=0 → width = R = 5, perp_dist from y=0 fiber varies along edge
        let p1 = P3::new(-10.0, -2.0, 0.0);
        let p2 = P3::new(10.0, 2.0, 0.0);
        edge_push_single(&mut fiber, &p1, &p2, &tool);

        let intervals = fiber.intervals();
        assert!(
            !intervals.is_empty(),
            "Diagonal edge at fiber height should produce contact"
        );

        // The contact interval should be roughly centered near t=0.5 (x=0)
        // and span a reasonable fraction of the fiber
        let (lo, hi) = (intervals[0].lower, intervals[0].upper);
        let fiber_range = 60.0; // fiber from -30 to 30
        let contact_len = (hi - lo) * fiber_range;
        assert!(
            contact_len > 5.0 && contact_len < 40.0,
            "Contact length {:.1} should be reasonable for R=5 tool on diagonal edge",
            contact_len
        );
    }

    #[test]
    fn test_edge_push_diagonal_ball() {
        // Same diagonal edge with ball endmill.
        let tool = BallEndmill::new(10.0, 25.0); // R=5
        let mut fiber = Fiber::new_x(0.0, 3.0, -30.0, 30.0); // fiber at z=3
        // Edge from (-10, -2, 0) to (10, 2, 6) — crosses fiber z=3 midway
        let p1 = P3::new(-10.0, -2.0, 0.0);
        let p2 = P3::new(10.0, 2.0, 6.0);
        edge_push_single(&mut fiber, &p1, &p2, &tool);

        let intervals = fiber.intervals();
        assert!(
            !intervals.is_empty(),
            "Diagonal edge crossing fiber height should produce contact with ball endmill"
        );

        // Ball endmill has narrower width at height — interval should be narrower than flat
        let (lo, hi) = (intervals[0].lower, intervals[0].upper);
        assert!(
            hi > lo,
            "Should have non-zero contact interval: lo={:.3}, hi={:.3}",
            lo,
            hi
        );
    }

    /// R9: a long edge that crosses the fiber is found even when its 6 mm
    /// contact window sits between two of the old whole-edge coarse samples.
    /// The edge is 56 mm long (coarse pitch 7 mm) and crosses the fiber at
    /// `y = 0` with the window `[-3, 3]` centred at `s = 0.5 + 3.5/56`, so
    /// the old samples at `s = 4/8` (`y = -3.5`) and `s = 5/8` (`y = 3.5`)
    /// both missed it.
    #[test]
    fn long_crossing_edge_blocks_the_fiber_between_coarse_samples() {
        let tool = FlatEndmill::new(6.0, 25.0); // R=3
        let mut fiber = Fiber::new_x(0.0, 10.0, -50.0, 50.0);
        let p1 = P3::new(20.0, -31.5, 15.0);
        let p2 = P3::new(20.0, 24.5, 15.0);
        edge_push_single(&mut fiber, &p1, &p2, &tool);
        let intervals = fiber.intervals();
        assert_eq!(intervals.len(), 1, "one contact interval expected");
        let lo = fiber.point(intervals[0].lower).x;
        let hi = fiber.point(intervals[0].upper).x;
        assert!(
            (lo - 17.0).abs() < 1e-6,
            "interval starts at x={lo}, want 17"
        );
        assert!((hi - 23.0).abs() < 1e-6, "interval ends at x={hi}, want 23");
    }

    /// R9: a vertical facet taller than the cutting length blocks the fiber
    /// across its whole XY trace, not only near its in-slab edges.
    #[test]
    fn tall_vertical_facet_blocks_across_its_trace() {
        let tool = FlatEndmill::new(6.0, 12.0); // R=3, cutting length 12
        // A wall triangle in the plane x = 20, spanning y -40..40 and
        // z 0..60. At fiber z = 5 the slab is [5, 17]: the top vertex is
        // out of reach and the diagonal covers only part of the width.
        let tri = Triangle::new(
            P3::new(20.0, -40.0, 0.0),
            P3::new(20.0, 40.0, 0.0),
            P3::new(20.0, -40.0, 60.0),
        );
        // A Y-fiber through the middle of the wall, 3 mm short of its face.
        let mut fiber = Fiber::new_y(17.5, 5.0, -60.0, 60.0);
        push_cutter_triangle(&mut fiber, &tri, &tool);
        // The clipped facet at z in [5, 17] spans y from -40 to
        // 40 - 80·(17/60) ≈ 17.33 at the slab top and up to 33.33 at the
        // slab bottom; its XY trace is y in [-40, 33.33]. The fiber at 2.5
        // from the face is blocked over that trace, widened by the rim.
        assert!(
            fiber.is_blocked(fiber.tval(&P3::new(17.5, 0.0, 5.0))),
            "fiber free in the middle of a tall vertical wall"
        );
        assert!(
            fiber.is_blocked(fiber.tval(&P3::new(17.5, 30.0, 5.0))),
            "fiber free under the clipped diagonal"
        );
        assert!(
            !fiber.is_blocked(fiber.tval(&P3::new(17.5, 40.0, 5.0))),
            "fiber blocked beyond the clipped facet's trace plus the radius"
        );
    }

    /// `push_segment_xy` is exact for a constant width: an oblique segment
    /// gives the closed-form tangency ends, and a segment crossing the
    /// fiber gives both sides.
    #[test]
    fn push_segment_xy_is_exact_for_a_constant_width() {
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        // A segment along y crossing the fiber at x = 10: blocked [7, 13].
        push_segment_xy(&mut fiber, (10.0, -20.0), (10.0, 20.0), 3.0);
        let iv = fiber.intervals()[0];
        assert!((fiber.point(iv.lower).x - 7.0).abs() < 1e-9);
        assert!((fiber.point(iv.upper).x - 13.0).abs() < 1e-9);

        // A segment parallel to the fiber at perpendicular distance 1.8,
        // x from -30 to -20: the rim through each end reaches
        // sqrt(9 - 3.24) = 2.4 along the fiber.
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        push_segment_xy(&mut fiber, (-30.0, 1.8), (-20.0, 1.8), 3.0);
        let iv = fiber.intervals()[0];
        assert!((fiber.point(iv.lower).x - (-32.4)).abs() < 1e-9);
        assert!((fiber.point(iv.upper).x - (-17.6)).abs() < 1e-9);

        // Too far away: nothing.
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        push_segment_xy(&mut fiber, (-30.0, 3.5), (-20.0, 3.5), 3.0);
        assert!(fiber.intervals().is_empty());
    }

    #[test]
    fn test_edge_push_sloped_edge() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 5.0, -50.0, 50.0);
        // Triangle with edge crossing fiber height
        let tri = Triangle::new(
            P3::new(0.0, -1.0, 0.0),
            P3::new(0.0, 1.0, 10.0),
            P3::new(20.0, 0.0, 5.0),
        );
        edge_push(&mut fiber, &tri, &tool);
        // Edge from z=0 to z=10 crosses fiber z=5
        assert!(
            !fiber.intervals().is_empty(),
            "Should find edge contact at z=5"
        );
    }
}
