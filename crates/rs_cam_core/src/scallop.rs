//! Scallop finishing strategy — constant scallop height across the surface.
//!
//! Generates concentric offset contours with variable stepover that maintains
//! constant scallop height regardless of surface slope and curvature. On steep
//! walls the stepover is wider (ball endmill has larger effective radius), on
//! shallow convex areas it is tighter.
//!
//! Algorithm:
//! 1. Project mesh boundary onto XY → outer polygon
//! 2. Compute variable stepover from scallop height, local slope, and curvature
//! 3. Iteratively offset polygon inward by stepover
//! 4. Drop-cutter Z at each ring's points → 3D contour
//! 5. Chain rings into toolpath (with optional spiral connection)
//!
//! From Fusion 360 docs: "passes follow sloping and vertical walls to maintain
//! the stepover."

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::finish_setup::FinishResolutionPolicy;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::region_set::RegionSet;
use crate::scallop_math::variable_stepover;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};

use tracing::info;

/// Direction for scallop contouring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScallopDirection {
    /// Start from boundary, work inward (default).
    #[default]
    OutsideIn,
    /// Start from center, work outward.
    InsideOut,
}

/// Parameters for scallop finishing.
pub struct ScallopParams {
    /// Desired scallop height (mm). This is the PRIMARY parameter.
    pub scallop_height: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
    /// Direction of contouring.
    pub direction: ScallopDirection,
    /// Connect contours into a continuous spiral (fewer retracts).
    pub continuous: bool,
    /// Slope confinement: only machine slopes steeper than this (degrees).
    pub slope_from: f64,
    /// Slope confinement: only machine slopes shallower than this (degrees).
    pub slope_to: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScallopRuntimeEvent {
    Ring {
        ring_index: usize,
        ring_total: usize,
        continuous: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScallopRuntimeAnnotation {
    pub move_index: usize,
    pub event: ScallopRuntimeEvent,
}

impl ScallopRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ring { ring_index, .. } => format!("Ring {ring_index}"),
        }
    }
}

impl Default for ScallopParams {
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: false,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        }
    }
}

/// Radius of the sphere that actually forms the cusp between adjacent
/// passes: the ball TIP for tapered tools, the full radius otherwise.
///
/// P2.f (user-caught "coarse steep stepover", 2026-07-09): the cusp math
/// previously used `cutter.radius()`, which for a tapered ball is the
/// SHANK radius — on the wanaka tool (Ø2 tip on a Ø6 shank) that computed
/// 0.51 mm ring spacing for h = 0.011 where the 1 mm tip needs 0.30 mm,
/// i.e. the real cusp was ~3× the dial. The tip sphere is the exactly
/// correct contact for every slope shallower than `90° − taper_half`
/// (~83° on a 7° taper) — the cone flank never forms the cusp inside the
/// scallop band. The feeds side already bounds on tip radius
/// (`cutter_constraints`'s TaperedBall arm); this brings generation in
/// line with it. Drop-cutter Z lifts always used the full tool profile —
/// only this scalar spacing dial was wrong.
/// Compute the ring's stepover from the slope map and scallop math —
/// the MINIMUM over the sampled points, so the scallop-height guarantee
/// holds at the ring's most demanding (steepest / most convex) stretch.
///
/// P2.f (2026-07-09): this was the MEAN, which on a dendritic mixed-slope
/// region under-tightens exactly where the terrain is steepest — the
/// residual +0.3–0.5 mm leftover band the fidelity instrument showed
/// after the chord fix, and the visibly coarse steep stepover the user
/// flagged. A per-ring constant is inherently a compromise (offsetting is
/// uniform per ring); min is its conservative end.
fn ring_stepover(
    ring: &[P2],
    slope_map: &crate::slope::SlopeMap,
    cusp_r: f64,
    scallop_height: f64,
) -> f64 {
    if ring.is_empty() {
        return crate::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height);
    }

    let sample_step = 1.max(ring.len() / 20);
    let mut min_so = f64::INFINITY;

    for pt in ring.iter().step_by(sample_step) {
        let angle = slope_map.angle_at_world(pt.x, pt.y).unwrap_or(0.0);
        // SlopeMap convention: negative = physically convex (see slope.rs doc on
        // `curvatures`). scallop_math::variable_stepover expects the opposite
        // (positive = convex), so negate at this boundary.
        let curvature = -slope_map.curvature_at_world(pt.x, pt.y).unwrap_or(0.0);
        let so = variable_stepover(cusp_r, scallop_height, angle, curvature);
        if so > 0.01 {
            min_so = min_so.min(so);
        }
    }

    if min_so.is_finite() {
        min_so
    } else {
        crate::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height)
    }
}

/// Whether `(x, y)` lands on a [`crate::slope::SurfaceHeightmap`] cell whose
/// vertical ray actually hit the mesh (`SurfaceHeightmap::covered`).
/// Mirrors `SlopeMap::world_to_cell`'s nearest-cell rounding so lookups
/// resolve consistently with the sibling `slope_map` built from the same
/// heightmap grid (both share `origin_x`/`origin_y`/`cell_size`/`rows`/`cols`).
fn heightmap_covered_at_world(hm: &crate::slope::SurfaceHeightmap, x: f64, y: f64) -> bool {
    let col_f = (x - hm.origin_x) / hm.cell_size;
    let row_f = (y - hm.origin_y) / hm.cell_size;
    if col_f < -0.5 || row_f < -0.5 {
        return false;
    }
    let col = col_f.round();
    let row = row_f.round();
    if col < 0.0 || row < 0.0 || col >= hm.cols as f64 || row >= hm.rows as f64 {
        return false;
    }
    // SAFETY: bounds checked above against hm.cols/hm.rows.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let covered = hm.covered_at(row as usize, col as usize);
    covered
}

/// Lift a 2D polygon ring to 3D by drop-cutter Z queries, pairing each point
/// with whether it sits over real mesh surface.
///
/// The `bool` is `true` only when `point_drop_cutter` found a finite contact
/// AND the surface heightmap's per-cell `covered` mask agrees the vertical
/// ray at that XY actually passed through the mesh — `point_drop_cutter`
/// alone can't tell that apart from cutter-radius rim contact just past a
/// hole or the mesh edge (see `SurfaceHeightmap::covered`'s doc comment).
///
/// Excluded (`false`) points still carry a Z (`min_z + stock_to_leave`) so
/// the tuple is always well-formed, but callers must run rings through the
/// shared run-splitter (`crate::point_runs`) and treat `false` stretches as
/// gaps — feeding straight through them used to dive the cutter to `min_z`
/// at every off-footprint corner instead of retracting around the gap
/// (P2.3 bonus fix; tracker `planning/finishing_stack_review_2026-07.md`).
/// Decimate a closed ring polygon (exterior + holes): drop vertices closer
/// than `min_spacing` to the previously KEPT vertex. Never adds points, so
/// rings already at or above `min_spacing` density pass through untouched —
/// classic convex scallop rings see zero change. Returns `None` when the
/// exterior can't keep 3 points (a degenerate sliver — culled). Holes that
/// collapse below 3 points are dropped individually.
///
/// Exists to keep the iterated `offset_polygon` cascade in
/// [`generate_scallop_rings`] linear — see the call-site comment for the
/// measured exponential this prevents.
fn decimate_ring_polygon(poly: &Polygon2, min_spacing: f64) -> Option<Polygon2> {
    let min_spacing = min_spacing.max(1e-3);
    let exterior = decimate_closed_ring(&poly.exterior, min_spacing)?;
    let holes: Vec<Vec<P2>> = poly
        .holes
        .iter()
        .filter_map(|h| decimate_closed_ring(h, min_spacing))
        .collect();
    let mut out = Polygon2::new(exterior);
    out.holes = holes;
    Some(out)
}

/// Keep the first vertex, then every vertex at least `min_spacing` from the
/// last kept one; drop a closing vertex that lands within half a spacing of
/// the head. `None` when fewer than 3 points survive.
fn decimate_closed_ring(ring: &[P2], min_spacing: f64) -> Option<Vec<P2>> {
    if ring.len() < 3 {
        return None;
    }
    let min_sq = min_spacing * min_spacing;
    let mut out: Vec<P2> = Vec::new();
    let mut last_kept = *ring.first()?;
    out.push(last_kept);
    for p in ring.iter().skip(1) {
        let dx = p.x - last_kept.x;
        let dy = p.y - last_kept.y;
        if dx * dx + dy * dy >= min_sq {
            out.push(*p);
            last_kept = *p;
        }
    }
    if out.len() >= 2
        && let (Some(first), Some(last)) = (out.first().copied(), out.last().copied())
    {
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        if dx * dx + dy * dy < min_sq * 0.25 {
            out.pop();
        }
    }
    (out.len() >= 3).then_some(out)
}

/// Everything a ring lift + chord refinement needs, bundled so the
/// per-ring call sites don't each thread eight loose arguments.
struct RingLiftCtx<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a dyn MillingCutter,
    heightmap: &'a crate::slope::SurfaceHeightmap,
    stock_to_leave: f64,
    min_z: f64,
    /// Max allowed gap between a straight feed chord and the true
    /// drop-cutter surface under it (the op's path tolerance).
    chord_tolerance: f64,
    /// Spacing at which chords are probed against the surface:
    /// `max(cell_size / 2, CHORD_REFINE_MIN_SEG_MM)`.
    probe_step: f64,
}

/// Floor (mm) on chord-refinement probe/segment spacing. Refinement must
/// not fragment paths into segments the junction/accel integrator pays
/// dearly for (P0 probe: sub-0.3 mm segment junctions dominate finishing
/// runtime) — 0.15 mm is half the wanaka raster reference pitch, i.e.
/// refined scallop is never more finely segmented than 2× the quality
/// reference it is chasing.
const CHORD_REFINE_MIN_SEG_MM: f64 = 0.15;

/// Depth cap on recursive chord splitting. Combined with the probe-step
/// floor this bounds worst-case insertion on cliff edges, where the chord
/// error never converges and every level would otherwise split.
const CHORD_REFINE_MAX_DEPTH: usize = 5;

fn ring_to_3d(ring: &[P2], ctx: &RingLiftCtx<'_>) -> Vec<(P3, bool)> {
    let lifted: Vec<(P3, bool)> = ring
        .iter()
        .map(|p| {
            let cl = point_drop_cutter(p.x, p.y, ctx.mesh, ctx.index, ctx.cutter);
            let finite = cl.z.is_finite();
            let kept = finite && heightmap_covered_at_world(ctx.heightmap, p.x, p.y);
            let z = if finite {
                cl.z + ctx.stock_to_leave
            } else {
                ctx.min_z + ctx.stock_to_leave
            };
            (P3::new(p.x, p.y, z), kept)
        })
        .collect();
    refine_ring_chords(lifted, ctx)
}

/// P2.f band-fidelity fix (2026-07-08): ring vertices are EXACT drop-cutter
/// points, but the straight feed chords BETWEEN them were never checked
/// against the surface. Ring vertex spacing tracks the generation grid
/// (`decimate_ring_polygon` floors it at `cell_size * 0.75` — 0.56 mm on
/// wanaka's Ø6/0.75 mm-cell setup), so any terrain feature narrower than a
/// chord got beheaded: two on-surface endpoints, a straight cut through
/// the knob between them ("smooshed mountains", user-caught in the live
/// sim). Placement stays on the coarse offset-cascade grid; this pass
/// restores fidelity where the surface actually demands it by probing each
/// kept→kept chord at `probe_step` and recursively splitting at the
/// worst-error probe until the chord tracks the surface within
/// `chord_tolerance`. Smooth/flat stretches insert nothing.
fn refine_ring_chords(ring: Vec<(P3, bool)>, ctx: &RingLiftCtx<'_>) -> Vec<(P3, bool)> {
    let n = ring.len();
    if n < 2 {
        return ring;
    }
    let mut out: Vec<(P3, bool)> = Vec::with_capacity(n * 2);
    for i in 0..n {
        // SAFETY: i and (i + 1) % n are both in 0..n.
        #[allow(clippy::indexing_slicing)]
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        out.push(a);
        // Only chords between two KEPT points are ever fed along; the
        // run-splitters already retract around excluded stretches. The
        // wrap chord (last → first) is included: discrete mode closes
        // fully-kept loops with a straight feed back to the start.
        if a.1 && b.1 {
            refine_chord(a.0, b.0, ctx, CHORD_REFINE_MAX_DEPTH, &mut out);
        }
    }
    out
}

/// Probe the open interval between `a` and `b`; if the worst deviation
/// between chord and drop-cutter surface exceeds tolerance, insert the
/// exact surface point there and recurse into both halves. Pushes only
/// INTERIOR points (in order); the caller owns the endpoints. A coverage
/// gap under the chord (hole / mesh edge) pushes one excluded point so the
/// emission run-splitter retracts around it instead of feeding across.
fn refine_chord(a: P3, b: P3, ctx: &RingLiftCtx<'_>, depth: usize, out: &mut Vec<(P3, bool)>) {
    if depth == 0 {
        return;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() {
        return;
    }
    let segments = (len / ctx.probe_step).ceil();
    if segments < 2.0 {
        return; // nothing to probe between endpoints at this scale
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let segments = segments as usize;

    // (t, surface_z, |err|) of the worst interior probe.
    let mut worst: Option<(f64, f64, f64)> = None;
    for i in 1..segments {
        let t = i as f64 / segments as f64;
        let x = a.x + dx * t;
        let y = a.y + dy * t;
        let cl = point_drop_cutter(x, y, ctx.mesh, ctx.index, ctx.cutter);
        if !cl.z.is_finite() || !heightmap_covered_at_world(ctx.heightmap, x, y) {
            out.push((P3::new(x, y, ctx.min_z + ctx.stock_to_leave), false));
            return;
        }
        let surface_z = cl.z + ctx.stock_to_leave;
        let chord_z = a.z + (b.z - a.z) * t;
        let err = (surface_z - chord_z).abs();
        if worst.is_none_or(|(_, _, we)| err > we) {
            worst = Some((t, surface_z, err));
        }
    }
    let Some((t, surface_z, err)) = worst else {
        return;
    };
    if err <= ctx.chord_tolerance {
        return;
    }
    let w = P3::new(a.x + dx * t, a.y + dy * t, surface_z);
    refine_chord(a, w, ctx, depth - 1, out);
    out.push((w, true));
    refine_chord(w, b, ctx, depth - 1, out);
}

/// Generate concentric offset rings from the outer boundary inward.
///
/// Uses variable stepover: at each ring, samples the slope map to compute
/// the average stepover that maintains constant scallop height, then offsets
/// by that amount.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::too_many_arguments, clippy::expect_used)]
// Production goes through the _with_cancel variant; this never-cancel
// convenience wrapper is exercised by the unit tests below.
#[cfg_attr(not(test), allow(dead_code))]
fn generate_scallop_rings(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::slope::SlopeMap,
    heightmap: &crate::slope::SurfaceHeightmap,
    cusp_r: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
    chord_tolerance: f64,
) -> RingCascade {
    let never_cancel = || false;
    generate_scallop_rings_with_cancel(
        boundary,
        mesh,
        index,
        cutter,
        slope_map,
        heightmap,
        cusp_r,
        scallop_height,
        stock_to_leave,
        min_z,
        max_rings,
        chord_tolerance,
        &never_cancel,
    )
    .expect("non-cancellable scallop ring generation should never be cancelled")
}

/// Cancellable variant of [`generate_scallop_rings`]. Polls `cancel` once per
/// ring (the expensive step: `ring_to_3d` runs a `point_drop_cutter` query per
/// ring point).
#[allow(clippy::too_many_arguments)]
fn generate_scallop_rings_with_cancel(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::slope::SlopeMap,
    heightmap: &crate::slope::SurfaceHeightmap,
    cusp_r: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
    chord_tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<RingCascade, Cancelled> {
    let lift_ctx = RingLiftCtx {
        mesh,
        index,
        cutter,
        heightmap,
        stock_to_leave,
        min_z,
        chord_tolerance,
        probe_step: (heightmap.cell_size * 0.5).max(CHORD_REFINE_MIN_SEG_MM),
    };
    let mut rings_3d: Vec<Vec<(P3, bool)>> = Vec::new();

    // First ring: the boundary itself, lifted to 3D
    let first_ring = ring_to_3d(&boundary.exterior, &lift_ctx);
    if first_ring.len() < 3 {
        // Degenerate boundary — nothing was cut, but nothing was LEFT
        // uncut either: there is no region interior to report.
        return Ok((rings_3d, 0.0));
    }
    rings_3d.push(first_ring);

    // Iteratively offset inward
    let mut current_polys = vec![boundary.clone()];

    // The loop's REAL terminator is the cascade collapsing to nothing
    // (`next_polys.is_empty()`); `max_rings` is only a runaway guard. If it
    // ever binds, the rings stop part-way and the INTERIOR of the region is
    // left uncut — silently, because the loop simply ends. That is what
    // this tracks (see the exhaustion warning below).
    let mut exhausted = true;

    for _ in 0..max_rings {
        check_cancel(cancel)?;
        // Ring stepover from the current rings' slope/curvature — MIN
        // across polygons (matching `ring_stepover`'s min-across-samples)
        // so the cusp guarantee holds on every branch of a multi-polygon
        // cascade, not just the length-weighted average one.
        let ring_so = current_polys
            .iter()
            .map(|poly| ring_stepover(&poly.exterior, slope_map, cusp_r, scallop_height))
            .fold(f64::INFINITY, f64::min);
        let ring_so = if ring_so.is_finite() {
            ring_so
        } else {
            crate::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height)
        };

        // Clamp stepover to reasonable bounds
        let stepover = ring_so
            .max(cusp_r * 0.05) // At least 5% of the cusp radius
            .min(cusp_r * 3.0); // At most 3× the cusp radius

        // Offset all current polygons inward, then DECIMATE each result
        // back to at most the heightmap's own sampling density.
        // `offset_polygon` ADDS vertices on every call (concave corners
        // sprout arc-approximation points; none are ever removed), so an
        // iterated cascade compounds ~15–25% vertices per ring on concave
        // boundaries — measured exponential on wanaka's dendritic
        // mid-steep band (1178 → 261 000 vertices by ring 25, 10 s per
        // offset and doubling; 2026-07-08, P2.c probe). Dropping only
        // sub-cell points keeps the cascade linear while never touching a
        // ring that is already at design density — classic convex
        // boundaries (the full-footprint rectangle path, ~stepover-spaced)
        // pass through byte-identical, which is why this stayed invisible
        // until region-scoped scallop met a dendritic band. Slivers whose
        // perimeter can't keep 3 points die here too.
        let ring_min_spacing = heightmap.cell_size * 0.75;
        let mut next_polys = Vec::new();
        for poly in &current_polys {
            for offset in offset_polygon(poly, stepover) {
                if let Some(decimated) = decimate_ring_polygon(&offset, ring_min_spacing) {
                    next_polys.push(decimated);
                }
            }
        }

        if next_polys.is_empty() {
            exhausted = false;
            break; // Collapsed to nothing — the intended exit
        }

        // Lift each new polygon ring to 3D
        for poly in &next_polys {
            if poly.exterior.len() < 3 {
                continue;
            }
            let ring_3d = ring_to_3d(&poly.exterior, &lift_ctx);
            if ring_3d.len() >= 3 {
                rings_3d.push(ring_3d);
            }
        }

        current_polys = next_polys;
    }

    let mut uncut_core_mm2 = 0.0;
    if exhausted {
        let remaining: f64 = current_polys
            .iter()
            .map(|p| crate::polygon::shoelace_area(&p.exterior).abs())
            .sum();
        uncut_core_mm2 = remaining;
        tracing::warn!(
            max_rings,
            rings_emitted = rings_3d.len(),
            uncut_core_mm2 = remaining,
            // M1 §4.3: state the domain on the line that carries the number.
            measurement = %ScallopReport::PROVENANCE,
            "scallop: ring cascade hit max_rings without collapsing — the \
             INTERIOR of the region is LEFT UNCUT. Known defect: the cap is \
             budgeted from the flat-ground (widest) stepover, and raising it \
             is worse until `ring_stepover`'s min-across-ring collapse and \
             chord refinement are fixed — see the cap's derivation comment."
        );
    }

    Ok((rings_3d, uncut_core_mm2))
}

/// Index of the point on `ring` closest to `target` among points that
/// satisfy `keep`; `None` when nothing on the ring survives the predicate.
///
/// Continuous mode rotates each ring to start here so the ring-to-ring
/// hop is as short as the *surviving* geometry allows — rotating to the
/// globally-closest point (kept or not) let the connector target an
/// excluded point while the tool's real position sat elsewhere, which is
/// exactly the long-cutting-chord shape the P0.4 regression tests pin.
fn closest_kept_point_idx<F>(ring: &[(P3, bool)], target: &P3, keep: F) -> Option<usize>
where
    F: Fn(&(P3, bool)) -> bool,
{
    ring.iter()
        .enumerate()
        .filter(|(_, pt)| keep(pt))
        .min_by(|(_, a), (_, b)| {
            let da = (a.0.x - target.x).powi(2) + (a.0.y - target.y).powi(2);
            let db = (b.0.x - target.x).powi(2) + (b.0.y - target.y).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Reorder a ring to start at the given index.
fn rotate_ring(ring: &[(P3, bool)], start_idx: usize) -> Vec<(P3, bool)> {
    let n = ring.len();
    if n == 0 || start_idx == 0 {
        return ring.to_vec();
    }
    let mut result = Vec::with_capacity(n);
    // SAFETY: (start_idx + i) % n is always in 0..n
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        result.push(ring[(start_idx + i) % n]);
    }
    result
}

/// One boundary region's lifted rings, paired with the interior area the
/// cascade failed to reach. `bool` per point is `ring_to_3d`'s keep flag.
type RingCascade = (Vec<Vec<(P3, bool)>>, f64);

/// What the ring cascade found out about the geometry it just cut.
///
/// Exists so a generation-time finding can reach the diagnostics pipeline
/// instead of dying in a `tracing::warn!`. The campaign lost weeks to a
/// 28 mm block of standing material that was warned about on every run and
/// visible to nobody — see `planning/unified_v3_design.md` §13/§14c.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScallopReport {
    /// Region-interior area (mm²) the ring cascade LEFT UNCUT because it
    /// hit `max_rings` before the offsets collapsed, summed over every
    /// boundary region. `0.0` when every cascade collapsed normally.
    ///
    /// Known defect (`planning/v3_workplan.md`): the cap is budgeted from
    /// the FLAT-ground (widest) stepover while the loop selects a smaller
    /// one on every slope. Raising the cap alone measured far worse
    /// (Op B +92% time, deep over-cut 34×), so this reports the symptom
    /// until `ring_stepover`'s min-across-ring collapse is fixed.
    pub uncut_core_mm2: f64,
}

impl ScallopReport {
    /// What [`Self::uncut_core_mm2`] means (M1). Fixed for this measure, so it
    /// is a constant rather than a settable field — a field could drift from
    /// the code that fills it, and this number's whole failure mode is being
    /// read as something it is not.
    ///
    /// This is the single source the user-visible standing-material strings
    /// are derived from
    /// ([`crate::compute::config::STANDING_MATERIAL_DOMAIN`] and friends).
    pub const PROVENANCE: crate::measurement::MeasurementProvenance =
        crate::measurement::MeasurementProvenance::new(
            crate::measurement::MeasurementDomain::ProjectedXyArea,
            crate::measurement::MeasurementStage::RingCascadeResidual,
        )
        .with_resolution_note(
            "ring polygons decimated at 0.75x the finish heightmap cell (exterior shoelace)",
        );

    /// [`Self::uncut_core_mm2`] tagged with its domain — XY-projected, never
    /// a 3D surface area and never a share of one.
    #[must_use]
    pub const fn uncut_core(&self) -> crate::measurement::ProjectedXyAreaMm2 {
        crate::measurement::ProjectedXyAreaMm2::new(self.uncut_core_mm2)
    }
}

/// The resolution policy scallop generates on (H3 step 2).
///
/// Scallop selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the same
/// `(envelope_radius/4).max(tolerance)` cell it has always used, now stated
/// here rather than inherited from a shared builder. Because the choice is
/// local, moving scallop onto `FinishResolutionMode::CuspQuarter` (H3 step 4
/// starts with scallop: it has the strongest fidelity instrument) is an edit
/// to this one function and does not move `ramp_finish` or `steep_shallow`.
///
/// NOTE the asymmetry this function makes visible: scallop's STEPOVER is
/// cusp-scaled (`cusp_radius_mm()` below) while its GRID is envelope-scaled,
/// so on a tapered ball the rings are ~6× finer than the surface they are
/// sampled from. That is the H3 question, not a bug fixed here.
#[must_use]
pub fn scallop_generation_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
}

/// Generate a scallop finishing toolpath.
///
/// Produces concentric offset contours with variable stepover that maintains
/// constant scallop height across the surface regardless of slope and curvature.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(scallop_height = params.scallop_height))]
#[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
pub fn scallop_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
) -> Toolpath {
    let (tp, _, _) = scallop_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

fn runtime_annotations_to_labels(annotations: &[ScallopRuntimeAnnotation]) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn scallop_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport) {
    let never_cancel = || false;
    scallop_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        debug,
        None,
        &never_cancel,
    )
    .expect("non-cancellable scallop toolpath should never be cancelled")
}

/// Cancellable variant of [`scallop_toolpath_structured_annotated`]. Polls
/// `cancel` once per ring during 3D ring generation (`ring_to_3d`'s
/// per-point drop-cutter queries are the expensive step) and once per ring
/// again while chaining rings into the toolpath.
///
/// `boundary_regions` (P2.3): when `Some`, generation is pre-clipped to
/// these machining-boundary regions instead of the full mesh footprint —
/// exactly one region is used directly as the ring boundary (replacing the
/// hardcoded mesh-bbox rectangle below); multiple disjoint regions each get
/// their own independent ring set, concatenated (regions are disjoint by
/// construction, so no de-duplication is needed). `None` reproduces
/// today's single mesh-bbox-rectangle behavior byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    check_cancel(cancel)?;
    let mut uncut_core_mm2 = 0.0_f64;
    // Physical extent (heightmap padding / grid coverage) keeps the FULL
    // tool radius; all cusp/stepover math uses the cusp-forming radius
    // (tip sphere for tapered tools — see `cusp_radius`).
    let tool_radius = cutter.envelope_radius_mm();
    let cusp_r = cutter.cusp_radius_mm();
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs).
    // The RESOLUTION is scallop's own choice (H3 step 2) — see
    // `scallop_generation_resolution`.
    let surface = crate::finish_setup::build_finish_surface_with_policy_and_cancel(
        mesh,
        index,
        cutter,
        scallop_generation_resolution(cutter, params.tolerance),
        cancel,
    )?;
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;

    // Kept alongside the shared heightmap builder above (which derives the
    // same values internally) because `max_rings` below still needs the raw
    // extent — not just the resulting grid.
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;

    // Outer boundary: mesh footprint as a rectangle, sampled densely enough
    // for polygon offset to work correctly. Point spacing = stepover.
    let bx0 = bbox.min.x;
    let by0 = bbox.min.y;
    let bx1 = bbox.max.x;
    let by1 = bbox.max.y;
    let flat_so = crate::scallop_math::stepover_from_scallop_flat(cusp_r, params.scallop_height)
        .max(cusp_r * 0.1);
    let boundary = {
        let mut pts = Vec::new();
        // Bottom edge
        let mut x = bx0;
        while x < bx1 {
            pts.push(P2::new(x, by0));
            x += flat_so;
        }
        // Right edge
        let mut y = by0;
        while y < by1 {
            pts.push(P2::new(bx1, y));
            y += flat_so;
        }
        // Top edge (reversed)
        let mut x = bx1;
        while x > bx0 {
            pts.push(P2::new(x, by1));
            x -= flat_so;
        }
        // Left edge (reversed)
        let mut y = by1;
        while y > by0 {
            pts.push(P2::new(bx0, y));
            y -= flat_so;
        }
        if pts.len() < 4 {
            // Fallback to simple rectangle
            pts = vec![
                P2::new(bx0, by0),
                P2::new(bx1, by0),
                P2::new(bx1, by1),
                P2::new(bx0, by1),
            ];
        }
        Polygon2::new(pts)
    };

    // Max rings, budgeted from the FLAT-ground stepover — which is the
    // WIDEST spacing the cusp target allows, so this under-counts whenever
    // the terrain has slope. That is a known, deliberate compromise, and
    // the warning at the end of the ring loop reports when it bites.
    //
    // Why it is not simply raised (measured, wanaka ×2, 2026-07-28):
    // budgeting instead from the loop's own `cusp_r * 0.05` clamp floor —
    // the smallest stepover it can select — is correct in principle and
    // does fill the ~28 mm block of standing material this cap was leaving
    // in the middle of the part. But it is far WORSE overall, because the
    // cap was masking a deeper flaw rather than causing one:
    //
    //   * `ring_stepover` takes the MIN across samples on a ring, so a
    //     single steep sample sets the advance for the WHOLE ring. On
    //     terrain every large ring touches steep ground, so the cascade
    //     crawls at the worst-case rate.
    //   * Uncapped, that produced thousands of rings at ~25 µm spacing.
    //     Op B went 38 896 s -> 74 731 s (+92%) while removing LESS
    //     material, and deep over-cut columns went 937 -> 32 221 (34x)
    //     because dense rings mean short chords, and `refine_chord` never
    //     probes a chord shorter than `2 * probe_step`.
    //
    // So the real fix is in `ring_stepover` (per-segment advance instead
    // of min-across-ring, or a physically sensible floor) plus chord
    // refinement — not here. Until then this cap stays, and the loop warns
    // loudly when it truncates so the standing material is a KNOWN defect
    // rather than a silent one.
    let min_stepover =
        crate::scallop_math::stepover_from_scallop_flat(cusp_r, params.scallop_height)
            .max(cusp_r * 0.05);
    let max_extent = (extent_x - origin_x).max(extent_y - origin_y);
    let max_rings = ((max_extent / min_stepover) * 0.5).ceil() as usize + 10;

    info!(
        max_rings = max_rings,
        min_stepover = format!("{:.3}", min_stepover),
        "Generating scallop rings"
    );

    // P2.3: scan the machining-boundary regions instead of the hardcoded
    // mesh-bbox rectangle when they're available. A single region is used
    // directly as the ring boundary; multiple disjoint regions each get an
    // independent ring set (still bounded by the same `max_rings` safety
    // valve — it's an upper bound on ring count, not an exact prediction, so
    // reusing it per-region is safe). `None`/empty falls back to exactly the
    // one hardcoded-rectangle boundary generated above, so the ring output
    // is byte-identical to pre-P2.3 behavior in that case.
    let region_boundaries: Vec<Polygon2> = match boundary_regions {
        Some(regions) if !regions.is_empty() => regions.as_slice().to_vec(),
        _ => vec![boundary],
    };

    // Generate 3D rings, one region at a time, concatenated in region order.
    let mut rings: Vec<Vec<(P3, bool)>> = Vec::new();
    for region_boundary in &region_boundaries {
        check_cancel(cancel)?;
        let (region_rings, region_uncut) = generate_scallop_rings_with_cancel(
            region_boundary,
            mesh,
            index,
            cutter,
            &slope_map,
            &surface_hm,
            cusp_r,
            params.scallop_height,
            params.stock_to_leave,
            bbox.min.z,
            max_rings,
            params.tolerance,
            cancel,
        )?;
        rings.extend(region_rings);
        uncut_core_mm2 += region_uncut;
    }

    info!(rings = rings.len(), "Scallop rings generated");

    if rings.is_empty() {
        return Ok((
            Toolpath::new(),
            Vec::new(),
            ScallopReport { uncut_core_mm2 },
        ));
    }

    // Apply direction
    if matches!(params.direction, ScallopDirection::InsideOut) {
        rings.reverse();
    }

    // Slope confinement
    let use_slope_filter =
        crate::finish_setup::slope_filter_active(params.slope_from, params.slope_to);
    let slope_from_rad = params.slope_from.to_radians();
    let slope_to_rad = params.slope_to.to_radians();

    // Combined per-point keep predicate: real mesh coverage (P2.3 bonus
    // fix — see `ring_to_3d`) AND, when active, the slope band AND the
    // machining-boundary regions. Always run points through this (rather
    // than only when a filter is "active") — with every filter a no-op the
    // run-splitter below degenerates to "the whole ring survives as one
    // run", which is byte-identical to the plain unfiltered emission this
    // replaces.
    let region_ok = |p: &P3| -> bool {
        boundary_regions.is_none_or(|regions| regions.contains(&P2::new(p.x, p.y)))
    };
    let keep_point = |pt: &(P3, bool)| -> bool {
        let (p, covered) = pt;
        *covered
            && (!use_slope_filter
                || slope_map
                    .angle_at_world(p.x, p.y)
                    .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad))
            && region_ok(p)
    };

    // Convert rings to toolpath
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    if params.continuous && rings.len() >= 2 {
        // Continuous spiral mode: connect adjacent rings at their nearest
        // KEPT points. A ring-to-ring connector is a *cutting* feed only
        // when it is a genuine helical transition — the tool is already
        // down and the hop is no longer than the widest ring spacing the
        // generator can produce (its stepover is clamped to at most
        // `cusp_r * 3.0` above). Anything longer — a kept set that
        // shifted to the far side of the ring under the combined keep
        // predicate, or the gap between two disjoint P2.3 boundary regions
        // — gets a retract/rapid/replunge link instead of chording across
        // excluded material at cutting feed (the P0.4 gouge class the
        // no-chord regression tests pin).
        let link_threshold = cusp_r * 3.0;
        // The tool's last emitted position. Seeded from the first ring's
        // geometric end (the pre-existing rotation seed) and updated to the
        // REAL last emitted point after every run — anchoring rotation on
        // the geometric ring end let the connector hop diverge arbitrarily
        // far from where the tool actually stopped.
        // SAFETY: rings.len() >= 2 checked above; rings entries have >= 3 points.
        #[allow(clippy::indexing_slicing)]
        let mut anchor: P3 = rings[0].last().map_or(rings[0][0].0, |&(p, _)| p);
        let mut tool_down = false;

        for (i, ring) in rings.iter().enumerate() {
            check_cancel(cancel)?;
            // Rotate the ring to start at the kept point closest to the
            // tool position. A ring with no kept points emits nothing —
            // pre-P2.3 this still plunged to the ring's rotation start,
            // which for an off-footprint boundary corner meant diving on a
            // rim-contact Z (see `ring_to_3d`).
            let Some(start_idx) = closest_kept_point_idx(ring, &anchor, |pt| keep_point(pt)) else {
                continue;
            };
            let rotated = rotate_ring(ring, start_idx);
            annotations.push(ScallopRuntimeAnnotation {
                move_index: tp.moves.len(),
                event: ScallopRuntimeEvent::Ring {
                    ring_index: i + 1,
                    ring_total: rings.len(),
                    continuous: true,
                },
            });

            // Contiguous kept runs across the whole rotated ring (index 0
            // is kept by construction, so the first run starts at 0).
            let idx_runs = crate::point_runs::split_run_ranges(
                &rotated,
                |_, pt: &(P3, bool)| keep_point(pt),
                1,
            );
            for (run_idx, (s, e)) in idx_runs.into_iter().enumerate() {
                let Some(run) = rotated.get(s..=e) else {
                    continue;
                };
                let Some(&(run_first, _)) = run.first() else {
                    continue;
                };
                let hop =
                    ((run_first.x - anchor.x).powi(2) + (run_first.y - anchor.y).powi(2)).sqrt();
                // Helical transition: only the ring's FIRST run can continue
                // the previous ring's cut, and only when the tool is down
                // and the hop is within one ring spacing.
                let helical_link = run_idx == 0 && s == 0 && tool_down && hop <= link_threshold;
                let mut iter = run.iter();
                if helical_link {
                    if let Some(&(p, _)) = iter.next() {
                        tp.feed_to_with_intent(p, params.feed_rate, MoveIntent::FinishingCut);
                    }
                } else {
                    if tool_down {
                        tp.rapid_to_with_intent(
                            P3::new(anchor.x, anchor.y, params.safe_z),
                            MoveIntent::Retract,
                        );
                        tool_down = false;
                    }
                    if let Some(&(p, _)) = iter.next() {
                        tp.rapid_to_with_intent(
                            P3::new(p.x, p.y, params.safe_z),
                            MoveIntent::Linking,
                        );
                        tp.feed_to_with_intent(p, params.plunge_rate, MoveIntent::EntryPlunge);
                    }
                }
                for &(pt, _) in iter {
                    tp.feed_to_with_intent(pt, params.feed_rate, MoveIntent::FinishingCut);
                }
                if let Some(&(last, _)) = run.last() {
                    anchor = last;
                    tool_down = true;
                }
            }
        }

        // Final retract from wherever the tool actually ended.
        if tool_down {
            tp.rapid_to_with_intent(
                P3::new(anchor.x, anchor.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    } else {
        // Discrete ring mode: rapid between rings. Each ring is split into
        // contiguous runs that survive the combined keep predicate — a
        // ring is only safe to close back to its own start when EVERY
        // point on it survives (a partial survivor set closing across the
        // excluded gap would chord straight through material this pass
        // must not touch).
        let mut emitted_runs: Vec<(Vec<P3>, bool)> = Vec::new();
        for ring in &rings {
            if ring.len() < 3 {
                continue;
            }

            let runs = crate::point_runs::split_runs(
                ring,
                |_, pt: &(P3, bool)| keep_point(pt),
                crate::point_runs::RunTopology::Closed,
                3,
            );
            for run in runs {
                let is_closed_loop = run.len() == ring.len();
                let pts: Vec<P3> = run.iter().map(|&(p, _)| p).collect();
                emitted_runs.push((pts, is_closed_loop));
            }
        }

        for (ring_index, (points, close_loop)) in emitted_runs.iter().enumerate() {
            check_cancel(cancel)?;
            let Some(&first) = points.first() else {
                continue;
            };
            let move_index = tp.moves.len();
            annotations.push(ScallopRuntimeAnnotation {
                move_index,
                event: ScallopRuntimeEvent::Ring {
                    ring_index: ring_index + 1,
                    ring_total: emitted_runs.len(),
                    continuous: false,
                },
            });
            tp.rapid_to_with_intent(
                P3::new(first.x, first.y, params.safe_z),
                MoveIntent::Linking,
            );
            tp.feed_to_with_intent(first, params.plunge_rate, MoveIntent::EntryPlunge);
            for pt in points.iter().skip(1) {
                tp.feed_to_with_intent(*pt, params.feed_rate, MoveIntent::FinishingCut);
            }
            let retract_at = if *close_loop {
                // Close the ring: the closing feed brings the cutter back
                // to the first point, so retract from there.
                tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::FinishingCut);
                first
            } else {
                // Open arc: the cutter is at the run's last point — retract
                // there rather than chording back to the run's start.
                points.last().copied().unwrap_or(first)
            };
            tp.rapid_to_with_intent(
                P3::new(retract_at.x, retract_at.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    }

    if let Some(last) = tp.moves.last()
        && !matches!(last.move_type, crate::toolpath::MoveType::Rapid)
    {
        tp.rapid_to_with_intent(
            P3::new(last.target.x, last.target.y, params.safe_z),
            MoveIntent::Retract,
        );
    }

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Scallop toolpath complete"
    );

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    Ok((tp, annotations, ScallopReport { uncut_core_mm2 }))
}

pub fn scallop_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations, _report) =
        scallop_toolpath_structured_annotated(mesh, index, cutter, params, debug);
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    // ── Ring generation tests ───────────────────────────────────────

    #[test]
    fn test_scallop_flat_constant_stepover() {
        // On a flat surface, variable stepover should equal the flat formula everywhere
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let scallop_height = 0.1;

        let expected_so =
            crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

        let cell_size = 1.0;
        let never_cancel = || false;
        // SAFETY: never_cancel always returns false
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh,
            &si,
            &cutter,
            cell_size,
            &never_cancel,
        )
        .unwrap();
        let slope_map = surface.slope_map;

        // Sample stepover from the slope map at the center (min == mean on
        // a uniform flat surface, so the min-selection change is inert here)
        let so = ring_stepover(
            &[P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)],
            &slope_map,
            tool_radius,
            scallop_height,
        );

        assert!(
            (so - expected_so).abs() < expected_so * 0.3,
            "Flat surface stepover ({:.3}) should be near flat formula ({:.3})",
            so,
            expected_so
        );
    }

    #[test]
    fn test_convex_dome_curvature_sign_tightens_stepover() {
        // Regression pin for the SlopeMap/scallop_math sign-convention mismatch:
        // SlopeMap reports NEGATIVE curvature at a physically convex dome peak
        // (see slope.rs::test_curvature_convex), but scallop_math::variable_stepover
        // expects POSITIVE = convex. ring_stepover negates the raw
        // SlopeMap value before calling variable_stepover. If that negation is
        // ever removed, this test fails: a convex dome must produce a TIGHTER
        // stepover than a flat surface, not a wider one.
        fn make_dome_z_grid(rows: usize, cols: usize, cell_size: f64, radius: f64) -> Vec<f64> {
            let cx = (cols - 1) as f64 * cell_size * 0.5;
            let cy = (rows - 1) as f64 * cell_size * 0.5;
            let mut z = vec![0.0; rows * cols];
            for row in 0..rows {
                for col in 0..cols {
                    let x = col as f64 * cell_size - cx;
                    let y = row as f64 * cell_size - cy;
                    let r_sq = radius * radius - x * x - y * y;
                    z[row * cols + col] = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                }
            }
            z
        }

        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let slope_map = crate::slope::SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);

        let tool_radius = ball_cutter().radius();
        let scallop_height = 0.1;

        // Dome peak, same cell used by slope.rs::test_curvature_convex.
        let raw_curvature = slope_map.curvature_at(10, 10);
        assert!(
            raw_curvature < 0.0,
            "Dome peak curvature should be negative in SlopeMap convention, got {:.6}",
            raw_curvature
        );

        // Same negation applied at the production call site in
        // ring_stepover.
        let curvature = -raw_curvature;
        let angle = slope_map.angle_at(10, 10);

        let so_dome = variable_stepover(tool_radius, scallop_height, angle, curvature);
        let so_flat = crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

        assert!(
            so_dome < so_flat,
            "Convex dome stepover ({:.4}) should be tighter than flat stepover ({:.4}) \
             once the sign convention is corrected",
            so_dome,
            so_flat
        );
    }

    /// Regression sentry: a ring cascade that hits `max_rings` before the
    /// offsets collapse must REPORT the interior area it left standing.
    ///
    /// The value was previously computed only to build a `tracing::warn!`,
    /// which meant nobody saw it: the campaign shipped a 28 mm block of
    /// unmachined material for weeks because no harness run installed a
    /// subscriber, and the live GUI has no diagnostic channel for it at all
    /// (`planning/unified_v3_design.md` §13/§14c). Returning it is what lets
    /// it reach one.
    ///
    /// Paired with `test_scallop_rings_converge`, which asserts the same
    /// fixture reports 0.0 when its cap is adequate — so this cannot pass by
    /// reporting a non-zero area unconditionally.
    #[test]
    fn ring_cascade_reports_uncut_core_when_capped() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();

        let bbox = &mesh.bbox;
        let boundary = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);

        let never_cancel = || false;
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh, &si, &cutter, 1.0, &never_cancel,
        )
        .unwrap();

        // Three rings on a 50 mm square cannot reach the middle.
        let (rings, uncut_core_mm2) = generate_scallop_rings(
            &boundary,
            &mesh,
            &si,
            &cutter,
            &surface.slope_map,
            &surface.heightmap,
            tool_radius,
            0.1,
            0.0,
            mesh.bbox.min.z,
            3,
            0.5,
        );

        // `max_rings` bounds the OFFSET LOOP; the boundary itself is pushed
        // before it, so a bound cap emits `max_rings + 1` rings. The field
        // logs say the same thing (`max_rings=504 rings_emitted=505`).
        assert_eq!(
            rings.len(),
            4,
            "boundary ring + max_rings offset iterations"
        );
        assert!(
            uncut_core_mm2 > 100.0,
            "a 50 mm square capped at 3 rings leaves a large uncut core, \
             got {uncut_core_mm2} mm²"
        );
    }

    #[test]
    fn test_scallop_rings_converge() {
        // Rings should progressively shrink until the polygon collapses
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();

        let bbox = &mesh.bbox;
        let boundary = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);

        let cell_size = 1.0;
        let never_cancel = || false;
        // SAFETY: never_cancel always returns false
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh,
            &si,
            &cutter,
            cell_size,
            &never_cancel,
        )
        .unwrap();
        let surface_hm = surface.heightmap;
        let slope_map = surface.slope_map;

        let (rings, uncut_core_mm2) = generate_scallop_rings(
            &boundary,
            &mesh,
            &si,
            &cutter,
            &slope_map,
            &surface_hm,
            tool_radius,
            0.1,
            0.0,
            bbox.min.z,
            100,
            0.5,
        );

        assert!(
            rings.len() >= 3,
            "Should produce multiple rings on 50mm flat, got {}",
            rings.len()
        );

        // The cascade collapsed on its own, so nothing was left standing.
        // This is the control for `ring_cascade_reports_uncut_core_when_capped`
        // below: same fixture, adequate cap, zero standing material.
        assert_eq!(
            uncut_core_mm2, 0.0,
            "a cascade that collapses within its cap leaves no uncut core"
        );

        // Ring count should be bounded (polygon eventually collapses)
        let flat_so = crate::scallop_math::stepover_from_scallop_flat(tool_radius, 0.1);
        let expected_max = (25.0 / flat_so).ceil() as usize + 5; // half extent / stepover
        assert!(
            rings.len() <= expected_max,
            "Too many rings ({}), expected at most ~{}",
            rings.len(),
            expected_max
        );
    }

    #[test]
    fn test_scallop_z_from_dropcutter() {
        // Ring Z values should match drop-cutter queries
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();

        let params = ScallopParams {
            scallop_height: 0.1,
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // On flat mesh (z≈0), all cutting Z should be near 0
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.target.z < params.safe_z - 1.0
            {
                assert!(
                    m.target.z.abs() < 2.0,
                    "Flat mesh cutting Z should be near 0, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_scallop_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5, // Coarse for speed
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Hemisphere scallop should produce moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_scallop_continuous_no_rapids_between_rings() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            continuous: true,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // In continuous mode, there should be very few rapids
        // (just the initial approach and final retract)
        let rapid_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
            .count();

        assert!(
            rapid_count <= 4,
            "Continuous scallop should have minimal rapids, got {}",
            rapid_count
        );
    }

    #[test]
    fn test_scallop_inside_out() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            direction: ScallopDirection::InsideOut,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 5,
            "Inside-out scallop should produce moves, got {}",
            tp.moves.len()
        );
    }

    // ── Regression: slope-confined passes must not chord across gaps ──

    /// Shared assertion: no consecutive pair of cutting (`FinishingCut`)
    /// moves may be farther apart than `max_allowed` — a bigger jump means
    /// a cutting move chorded across an excluded slope gap instead of
    /// retracting. A rapid or plunge in between resets the check, since
    /// that's exactly the retract/replunge link the fix introduces.
    fn assert_no_chord_across_gap(tp: &Toolpath, max_allowed: f64) {
        let mut prev_cut: Option<P3> = None;
        let mut saw_any_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == MoveIntent::FinishingCut;
            if is_cut {
                saw_any_cut = true;
                if let Some(prev) = prev_cut {
                    let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                    assert!(
                        d <= max_allowed,
                        "cutting move chorded across the excluded slope gap: {:.2}mm \
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
                prev_cut = None;
            }
        }
        assert!(saw_any_cut, "expected at least one cutting move");
    }

    /// Regression for the discrete-ring chording bug: a slope-confined ring
    /// used to filter out-of-band points and feed straight between the
    /// remaining survivors (and even close the loop back to the first
    /// survivor), chording across the excluded stretch. A hemisphere's
    /// slope varies continuously with radius, and a ring's own perimeter
    /// varies in distance from center (it's an offset of the roughly
    /// rectangular mesh-footprint boundary, not a circle), so a slope band
    /// like 20-60° is guaranteed to include only part of most rings.
    #[test]
    fn test_scallop_discrete_no_chord_across_excluded_slope_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.3,
            tolerance: 0.3,
            continuous: false,
            slope_from: 20.0,
            slope_to: 60.0,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        let stepover =
            crate::scallop_math::stepover_from_scallop_flat(cutter.radius(), params.scallop_height);
        assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
    }

    /// Companion regression for continuous (spiral) mode: same slope band,
    /// same hemisphere, `continuous: true` this time.
    #[test]
    fn test_scallop_continuous_no_chord_across_excluded_slope_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.3,
            tolerance: 0.3,
            continuous: true,
            slope_from: 20.0,
            slope_to: 60.0,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        let stepover =
            crate::scallop_math::stepover_from_scallop_flat(cutter.radius(), params.scallop_height);
        assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
    }

    // ── Helper tests ────────────────────────────────────────────────

    #[test]
    fn test_closest_kept_point_idx() {
        let ring = vec![
            (P3::new(0.0, 0.0, 0.0), true),
            (P3::new(10.0, 0.0, 0.0), true),
            (P3::new(10.0, 10.0, 0.0), true),
            (P3::new(0.0, 10.0, 0.0), true),
        ];
        let target = P3::new(9.0, 9.0, 0.0);
        let idx = closest_kept_point_idx(&ring, &target, |pt| pt.1);
        assert_eq!(idx, Some(2), "Closest to (9,9) should be index 2 (10,10)");

        // Excluding the geometrically-closest point must fall through to
        // the next-closest KEPT point, not return the excluded one.
        let mut masked = ring.clone();
        masked[2].1 = false;
        let idx = closest_kept_point_idx(&masked, &target, |pt| pt.1);
        assert_eq!(
            idx,
            Some(1),
            "With (10,10) excluded, closest kept to (9,9) is index 1 (10,0)"
        );

        // Nothing kept -> None (the caller skips the ring entirely).
        let idx = closest_kept_point_idx(&ring, &target, |_| false);
        assert_eq!(idx, None);
    }

    #[test]
    fn test_rotate_ring() {
        let ring = vec![
            (P3::new(0.0, 0.0, 0.0), true),
            (P3::new(1.0, 0.0, 0.0), true),
            (P3::new(2.0, 0.0, 0.0), true),
            (P3::new(3.0, 0.0, 0.0), true),
        ];
        let rotated = rotate_ring(&ring, 2);
        assert!((rotated[0].0.x - 2.0).abs() < 0.01);
        assert!((rotated[1].0.x - 3.0).abs() < 0.01);
        assert!((rotated[2].0.x - 0.0).abs() < 0.01);
        assert!((rotated[3].0.x - 1.0).abs() < 0.01);
    }

    // ── P2.f: chord refinement ────────────────────────────────────────

    /// A flat strip with a sharp triangular ridge running along Y at x=0
    /// (apex z=2, base half-width 0.4 mm) — a terrain feature narrower
    /// than typical ring point spacing, i.e. the minimal "smooshed
    /// mountain" reproducer: exact endpoints either side, a knob between.
    fn make_ridge_mesh() -> (TriangleMesh, SpatialIndex) {
        let xs = [-10.0, -0.4, 0.0, 0.4, 10.0];
        let zs = [0.0, 0.0, 2.0, 0.0, 0.0];
        let mut vertices = Vec::new();
        for y in [-10.0, 10.0] {
            for (x, z) in xs.iter().zip(zs.iter()) {
                vertices.push(P3::new(*x, y, *z));
            }
        }
        let mut triangles = Vec::new();
        for i in 0..4u32 {
            triangles.push([i, i + 1, 5 + i]);
            triangles.push([i + 1, 5 + i + 1, 5 + i]);
        }
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let si = SpatialIndex::build(&mesh, 5.0);
        (mesh, si)
    }

    fn lift_ctx_for<'a>(
        mesh: &'a TriangleMesh,
        si: &'a SpatialIndex,
        cutter: &'a BallEndmill,
        heightmap: &'a crate::slope::SurfaceHeightmap,
        probe_step: f64,
    ) -> RingLiftCtx<'a> {
        RingLiftCtx {
            mesh,
            index: si,
            cutter,
            heightmap,
            stock_to_leave: 0.0,
            min_z: mesh.bbox.min.z,
            chord_tolerance: 0.05,
            probe_step,
        }
    }

    #[test]
    fn chord_refinement_lifts_path_over_sharp_ridge() {
        let (mesh, si) = make_ridge_mesh();
        let cutter = BallEndmill::new(1.0, 10.0);
        let never = || false;
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh, &si, &cutter, 0.5, &never,
        )
        .unwrap();
        let ctx = lift_ctx_for(&mesh, &si, &cutter, &surface.heightmap, 0.25);

        // Two exact surface points on the flats either side of the ridge —
        // pre-fix, the emitted chord between them cut straight through the
        // ridge at z ≈ 0, beheading it.
        let ring = vec![P2::new(-2.0, 0.0), P2::new(2.0, 0.0)];
        let refined = ring_to_3d(&ring, &ctx);

        assert!(
            refined.len() > 2,
            "refinement must insert points over the ridge"
        );
        let apex = refined
            .iter()
            .filter(|(p, kept)| *kept && p.x.abs() < 0.5)
            .map(|(p, _)| p.z)
            .fold(f64::MIN, f64::max);
        assert!(
            apex > 1.5,
            "refined path must climb over the ridge apex (z≈2), got max z {apex:.3}"
        );
        // Post-refinement, no kept→kept chord may deviate from the surface
        // by more than tolerance + probe aliasing slack.
        for w in refined.windows(2) {
            let (a, ka) = w[0];
            let (b, kb) = w[1];
            if !(ka && kb) {
                continue;
            }
            let mx = (a.x + b.x) * 0.5;
            let my = (a.y + b.y) * 0.5;
            let cl = point_drop_cutter(mx, my, &mesh, &si, &cutter);
            if !cl.z.is_finite() {
                continue;
            }
            let chord_z = (a.z + b.z) * 0.5;
            assert!(
                (cl.z - chord_z).abs() < 0.3,
                "residual chord error {:.3} at ({mx:.2},{my:.2})",
                (cl.z - chord_z).abs()
            );
        }
    }

    #[test]
    fn chord_refinement_no_op_on_flat() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let never = || false;
        let surface = crate::finish_setup::build_finish_surface_with_cell_size_and_cancel(
            &mesh, &si, &cutter, 1.0, &never,
        )
        .unwrap();
        let ctx = lift_ctx_for(&mesh, &si, &cutter, &surface.heightmap, 0.5);

        let ring = vec![
            P2::new(-20.0, -20.0),
            P2::new(20.0, -20.0),
            P2::new(20.0, 20.0),
            P2::new(-20.0, 20.0),
        ];
        let refined = ring_to_3d(&ring, &ctx);
        assert_eq!(
            refined.len(),
            4,
            "flat chords already within tolerance must not gain points (segment-count/runtime guard)"
        );
        assert!(refined.iter().all(|&(p, kept)| kept && p.z.abs() < 0.01));
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    /// `boundary_regions = None` reproduces the unrestricted toolpath
    /// (behaviorally — the P2.3 bonus covered-fix changes the *baseline*
    /// slightly from pre-P2.3 code, but the parameter itself must be a
    /// no-op): every cutting move on a flat mesh with no boundary passed
    /// should land somewhere on the mesh footprint.
    #[test]
    fn scallop_boundary_regions_none_is_unrestricted() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            tp.moves.len() > 10,
            "unrestricted scallop should produce moves, got {}",
            tp.moves.len()
        );
    }

    /// `boundary_regions = Some(&[left-half])` confines every cutting move's
    /// XY to that region (a small containment tolerance absorbs the ring's
    /// own point spacing landing right on the region edge).
    #[test]
    fn scallop_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let bbox = &mesh.bbox;
        let left_half = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(0.0, bbox.min.y),
            P2::new(0.0, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
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
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == MoveIntent::FinishingCut
            {
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

    /// Two disjoint boundary regions each get their own ring set — cuts
    /// land in both regions and never in the excluded gap between them.
    #[test]
    fn scallop_boundary_regions_two_disjoint_regions_both_cut() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let bbox = &mesh.bbox;
        // Left third and right third of the mesh, with a gap in the middle.
        let left = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(-15.0, bbox.min.y),
            P2::new(-15.0, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);
        let right = Polygon2::new(vec![
            P2::new(15.0, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(15.0, bbox.max.y),
        ]);
        let regions = vec![left, right];
        let region_set = RegionSet::from_slice(&regions);
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            ..ScallopParams::default()
        };
        let never_cancel = || false;

        let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let mut saw_left = false;
        let mut saw_right = false;
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == MoveIntent::FinishingCut
            {
                assert!(
                    m.target.x <= -15.0 + 1e-6 || m.target.x >= 15.0 - 1e-6,
                    "cutting move X={:.3} landed in the excluded gap between regions",
                    m.target.x
                );
                if m.target.x <= -15.0 + 1e-6 {
                    saw_left = true;
                }
                if m.target.x >= 15.0 - 1e-6 {
                    saw_right = true;
                }
            }
        }
        assert!(saw_left, "expected at least one cut in the left region");
        assert!(saw_right, "expected at least one cut in the right region");
    }
}
