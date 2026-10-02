//! Scallop ring generation: ring decimation, drop-cutter lifting of a 2D
//! ring to 3D, chord refinement, and the offset/iso-field ring cascade.
//!
//! Split out of `finish/scallop.rs` by P4; every item is unchanged apart
//! from its visibility. The stepover policy model these functions read, and
//! the report type they fill, stay in the parent.

use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::surface::chord_refine::{CHORD_REFINE_MAX_DEPTH, refine_chord};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;

/// The scallop's name for the shared chord-refinement context: the rings,
/// the ring connectors and their probe step (`ChordRefineCtx::from_cell`).
pub(super) use crate::surface::chord_refine::ChordRefineCtx as RingLiftCtx;
/// A continuous-mode ring-to-ring connector, refined like a ring chord
/// (G-TIERBURIAL). The shared name is `refine_track`.
pub(super) use crate::surface::chord_refine::refine_track as refine_connector;

use super::{
    PolygonReduce, RingCascade, RingCascadeMetrics, RingSource, RingStepoverDecision,
    ScallopReport, ScallopStepoverPolicy, ScallopStepoverTrace, ring_stepover_with_policy,
};

/// Whether `(x, y)` sits over real mesh surface, EXACTLY.
///
/// Delegates to [`crate::surface::dropcutter::point_is_over_mesh_xy`] — a zero-radius
/// spatial-index query plus point-in-triangle, the same predicate the Shallow
/// raster band uses (`unified_finish.rs`, "the tool rides the edge and carves
/// a trench around the part").
///
/// **Wave F2 / D-16.1.** This used to read the *generation* heightmap's
/// per-cell `covered` mask with nearest-cell rounding. That mask answers for
/// the nearest cell, and the generation cell is `envelope_radius / 4` —
/// 0.75 mm on the reference tool, six times the 0.125 mm classification cell
/// the bands are decided on. So the guard admitted ring vertices up to half a
/// generation cell (**0.375 mm**) outside the true footprint, `point_drop_cutter`
/// returned a rim-riding CL there, and the tip cut `r − √(r² − d²)` below the
/// surface. Measured on `grooved_block(2.5, 70°, 1.2)`: 131 off-footprint
/// cutting targets, worst run-off 0.3750 mm — the quantization bound on the
/// nose (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §2.2,
/// §2.8). The guard was already the right idea; it was merely mis-quantized.
///
/// The mask is no longer consulted at all, so the heightmap's cell size no
/// longer decides where a ring is allowed to cut — only where it is *sampled*
/// (`RingLiftCtx::probe_step`, ring decimation spacing), which is what that
/// grid is actually for.
///
/// The predicate itself lives in [`crate::surface::chord_refine`] with the
/// chord refinement that shares it (G-TIERBURIAL).
fn point_is_covered(ctx: &RingLiftCtx<'_>, x: f64, y: f64) -> bool {
    crate::surface::chord_refine::point_is_covered(ctx, x, y)
}

/// Lift a 2D polygon ring to 3D by drop-cutter Z queries, pairing each point
/// with whether it sits over real mesh surface.
///
/// The `bool` is `true` only when `point_drop_cutter` found a finite contact
/// AND [`point_is_covered`] — the exact point-in-triangle test — agrees the
/// vertical ray at that XY actually passed through the mesh. `point_drop_cutter`
/// alone can't tell that apart from cutter-radius rim contact just past a
/// hole or the mesh edge, and rim contact is an overcut, not a surface.
///
/// Excluded (`false`) points still carry a Z (`min_z + stock_to_leave`) so
/// the tuple is always well-formed, but callers must run rings through the
/// shared run-splitter (`crate::geometry::point_runs`) and treat `false` stretches as
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
pub(super) fn decimate_ring_polygon(poly: &Polygon2, min_spacing: f64) -> Option<Polygon2> {
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

pub(super) fn ring_to_3d(ring: &[P2], ctx: &RingLiftCtx<'_>) -> Vec<(P3, bool)> {
    let lifted: Vec<(P3, bool)> = ring
        .iter()
        .map(|p| {
            let cl = point_drop_cutter(p.x, p.y, ctx.mesh, ctx.index, ctx.cutter);
            let finite = cl.z.is_finite();
            let kept = finite && point_is_covered(ctx, p.x, p.y);
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

/// Sum, over one lifted ring, of the XY perimeter length "owned" by points
/// the keep predicate DROPPED (`kept == false`, `ring_to_3d`'s coverage
/// flag): half the incoming segment plus half the outgoing one, so a fully
/// kept ring contributes `0.0` and the owned lengths of every point — kept
/// or not — sum to exactly the ring's perimeter.
///
/// Feeds [`ScallopReport::standing_mm2`]: multiplied by the stepover that
/// produced the ring, this turns "how much of this ring's length got
/// excluded from the toolpath" into an area estimate.
fn dropped_arc_length_mm(ring: &[(P3, bool)]) -> f64 {
    let n = ring.len();
    if n < 2 {
        return 0.0;
    }
    // `seg[i]` is the XY length of the segment from `ring[i]` to
    // `ring[(i + 1) % n]`.
    let mut seg: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        // SAFETY: `i` and `(i + 1) % n` are both in `0..n`.
        #[allow(clippy::indexing_slicing)]
        let (a, b) = (ring[i].0, ring[(i + 1) % n].0);
        seg.push(((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt());
    }
    let mut total = 0.0;
    for i in 0..n {
        // SAFETY: `i` is in `0..n`, so `ring[i]` is in bounds.
        #[allow(clippy::indexing_slicing)]
        let kept = ring[i].1;
        if kept {
            continue;
        }
        let prev = (i + n - 1) % n;
        // SAFETY: `prev` and `i` are both in `0..n`, and `seg` has length
        // `n` (built above from the same range).
        #[allow(clippy::indexing_slicing)]
        {
            total += 0.5 * (seg[prev] + seg[i]);
        }
    }
    total
}

/// Generate concentric offset rings from the outer boundary inward.
///
/// Uses variable stepover: at each ring, samples the slope map to compute
/// the average stepover that maintains constant scallop height, then offsets
/// by that amount.
///
/// **Test/research seam, `pub`.** Production goes through
/// `generate_scallop_rings_with_cancel` (private); this never-cancel
/// convenience wrapper exists so the in-crate unit tests below — and the
/// external `scallop_untouched_standing_h4` sentry, which cannot see a
/// private item — can drive the cascade directly without a full mesh or
/// toolpath generation.
#[allow(clippy::too_many_arguments, clippy::expect_used)]
pub fn generate_scallop_rings(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::surface::slope::SlopeMap,
    heightmap: &crate::surface::slope::SurfaceHeightmap,
    cusp_r: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
    chord_tolerance: f64,
) -> RingCascade {
    crate::interrupt::run_uncancellable(|cancel| {
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
            ScallopStepoverPolicy::SHIPPED,
            None,
            cancel,
        )
    })
}

/// Cancellable variant of [`generate_scallop_rings`]. Polls `cancel` once per
/// ring (the expensive step: `ring_to_3d` runs a `point_drop_cutter` query per
/// ring point).
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_scallop_rings_with_cancel(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::surface::slope::SlopeMap,
    heightmap: &crate::surface::slope::SurfaceHeightmap,
    cusp_r: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
    chord_tolerance: f64,
    // M4 research seam; production passes `ScallopStepoverPolicy::SHIPPED`
    // and `None`, and the loop below is byte-identical under those values.
    policy: ScallopStepoverPolicy,
    mut trace: Option<&mut ScallopStepoverTrace>,
    cancel: &dyn CancelCheck,
) -> Result<RingCascade, Cancelled> {
    let lift_ctx = RingLiftCtx::from_cell(
        mesh,
        index,
        cutter,
        stock_to_leave,
        min_z,
        chord_tolerance,
        heightmap.cell_size,
    );
    let mut rings_3d: Vec<Vec<(P3, bool)>> = Vec::new();

    // First ring: the boundary itself, lifted to 3D
    let first_ring = ring_to_3d(&boundary.exterior, &lift_ctx);
    if first_ring.len() < 3 {
        // Degenerate boundary — nothing was cut, but nothing was LEFT
        // uncut either: there is no region interior to report.
        return Ok((rings_3d, RingCascadeMetrics::default()));
    }
    // M4 §5b: the seed boundary ring is excluded from `standing_mm2` — it
    // was pushed before the offset loop starts, so no offset stepover
    // "produced" it (the estimator's formula is arc length x THAT ring's
    // stepover). Any of its own dropped points (e.g. a boundary rectangle
    // whose corner sits off the mesh footprint) are therefore not counted;
    // see `ScallopReport::standing_mm2`'s doc for this and the estimator's
    // other stated limitations.
    rings_3d.push(first_ring);
    // G-HOLERING: a region's HOLES are boundary too. The cascade grows every
    // hole by the ring stepover each iteration (cavalier offsets the holes
    // with the exterior), so only lifting `exterior` left every grown hole
    // ring unemitted: the ground between a hole's edge and the point where
    // the inward exterior front met the grown hole was never visited by any
    // ring. On a unified finish the MidSteep band is one region with the
    // Shallow / VerySteep islands as holes, so that ground is a band around
    // every island (tier trial T5, `tests/unified_tier_coverage_probe.rs`).
    for hole in &boundary.holes {
        let hole_ring = ring_to_3d(hole, &lift_ctx);
        if hole_ring.len() >= 3 {
            rings_3d.push(hole_ring);
        }
    }

    // M4 research candidate 3: take the rings from an iso-scallop field
    // instead of an offset cascade. Everything downstream — the 3D lift, the
    // chord refinement, the emission — is shared with the cascade branch, so
    // a comparison across this switch isolates ring PLACEMENT alone.
    if matches!(policy.ring_source, RingSource::IsoField) {
        let field = crate::finish::scallop_isofield::build_field(boundary, slope_map, &|x, y| {
            policy.point_stepover(slope_map, cusp_r, scallop_height, x, y)
        });
        if let Some(t) = trace.as_deref_mut() {
            // The field has one decision per CELL, not per ring; report the
            // field's own spread so the table column means something.
            let mut finite: Vec<f64> = field
                .stepover_mm
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            finite.sort_by(f64::total_cmp);
            if !finite.is_empty() {
                // SAFETY: non-empty checked on the line above.
                #[allow(clippy::indexing_slicing)]
                let spread = (
                    finite[0],
                    finite[finite.len() / 2],
                    finite[finite.len() - 1],
                );
                t.sample_spread.push(spread);
                t.selected_mm.push(spread.1);
            }
        }
        // Marching-squares vertices land wherever a level crosses a cell
        // edge, so consecutive points can be an arbitrarily small fraction of
        // a cell apart. The offset cascade below floors its ring vertex
        // spacing at `0.75 × cell` (`decimate_ring_polygon`), and TWO
        // downstream contracts silently depend on that floor:
        //
        // 1. `refine_chord` only probes a chord it can fit at least one
        //    interior probe into. Under the cascade's floor every chord
        //    clears that bar, so chord refinement is universal; an
        //    undecimated iso-field ring emits sub-probe-step chords that
        //    refinement skipped entirely. On the grooved block that put a
        //    0.25 mm chord across the groove rim spanning a 1.0 mm Z step
        //    with nothing checking it — the −995/−1115 µm localised gouge
        //    of `CHECKPOINT_C_EVIDENCE.md` §3.8. (`refine_chord` no longer
        //    relies on the floor for correctness — see its own guard — but
        //    the floor is still what keeps refinement cheap.)
        // 2. The emitted segment-length distribution. §3.8's second flag,
        //    0.0–0.5% of segments under 10 µm against the cascade's zero,
        //    is the same missing pass.
        //
        // Decimation only ever DROPS points, never moves one, so ring
        // PLACEMENT — the whole subject of the iso-field comparison — is
        // untouched; chord refinement puts detail back exactly where the
        // surface demands it.
        let ring_min_spacing = heightmap.cell_size * 0.75;
        for ring in crate::finish::scallop_isofield::extract_rings(&field) {
            check_cancel(cancel)?;
            if ring.len() < 3 {
                continue;
            }
            let Some(ring) = decimate_closed_ring(&ring, ring_min_spacing) else {
                continue;
            };
            let ring_3d = ring_to_3d(&ring, &lift_ctx);
            if ring_3d.len() >= 3 {
                rings_3d.push(ring_3d);
            }
        }
        // The field's termination is exact — every interior cell has a finite
        // pass index and every integer level below the maximum was extracted —
        // so there is no truncated core to report. The oracle checks that
        // claim independently rather than taking it. `standing_mm2` is left
        // at its default `0.0` too: this branch is a research seam no
        // production entry point selects (`ScallopStepoverPolicy::SHIPPED`
        // is `RingSource::OffsetCascade`), so the estimator was never
        // extended to the iso-field's per-cell stepover.
        return Ok((rings_3d, RingCascadeMetrics::default()));
    }

    // Iteratively offset inward.
    //
    // Checkpoint D (2026-08-03): under the shipped `RingCleanup::ArcCascade`
    // the cascade STATE is `arc_rings` — cavalier's own arc-carrying
    // polylines — and `current_polys` is the flattened VIEW of it, produced
    // once per ring by the single `FlattenPolicy` below and used for exactly
    // two things: sampling the next stepover, and lifting to 3D. Nothing ever
    // feeds a flattened ring back into an offset, which is what the whole
    // vertex-doubling defect was. The research arms keep `arc_rings` at
    // `None` and run the flattened cascade they were measured on.
    // The flatten policy carries BOTH halves of what a ring flattening
    // decides: how far a chord may sit from the arc it replaces (a tenth of
    // the op's chord tolerance) and how long a chord may be before the ring
    // stops being a usable sample of the surface.
    //
    // The sampling bound is the flat-ground stepover — the SAME spacing
    // `scallop_toolpath_research` seeds the outer boundary rectangle at, and
    // therefore the density every straight run in the shipped cascade has
    // always carried. Straight runs are the case that matters: an arc-join
    // gets its points from the deviation budget above, but a 50 mm straight
    // edge gets none, and scallop reads every ring vertex as a drop-cutter
    // sample plus a coverage-mask lookup.
    //
    // **This bound costs +88% emitted moves on the PR-3 ridge, so it was put
    // on the envelope oracle rather than argued about**
    // (`tests/ring_sample_bound_w14.rs`, three fixtures × three bounds). What
    // came back is not what the +88% suggests:
    //
    // * On **achieved cusp it is parity, everywhere** — 68.1 µm bounded vs
    //   68.1 µm unbounded on the ridge, 51.0 vs 51.0 on the tight grooved
    //   block. The extra samples do not tighten the finish.
    // * On **gouge it is decisive**: unbounded ships **1.570 mm² at -115.5 µm**
    //   on the tight grooved block and **9.200 mm² at -116.5 µm** on the loose
    //   one, against **0.000** and **0.176 mm²** bounded. A straight run whose
    //   endpoints bracket a groove wall has no interior sample, so the chord
    //   is lifted over the feature and cuts through it.
    //
    // So the bound is load-bearing for GOUGE CONTAINMENT, not for cusp — and
    // a smooth fixture cannot show that (the ridge reads 0.000 mm² gouge on
    // all three arms). Sampling density is a safety property of the surface,
    // which is why it is not derived from the tolerance dial; see below.
    //
    // Three other candidates were measured and rejected:
    //
    // * **No bound** — the honest reading of "flatten to a tolerance". Wrong
    //   twice over: the gouge above, and on a mesh of four disjoint islands
    //   the four surviving corners all sit off the part and the operation
    //   emits nothing at all.
    // * **Tolerance-scaled** (`2·√(2·r·tol)`, the sampling analogue of the
    //   scallop law) — the intuitive fix, and it gets the sign backwards. On
    //   tight dials it costs ~2× the moves of the flat-ground bound
    //   (5076 vs 2674 on the ridge; 12787 vs 6639 on the grooved block) for
    //   ±0.2pp on-dial and 0.0 µm of cusp; on loose dials it saves moves and
    //   pays for them in gouge (0.576 vs 0.176 mm²). The chord tolerance
    //   governs how well a curve is approximated, and has nothing to say
    //   about how far apart a surface may be sampled — it is retained as a
    //   research arm precisely because that distinction is easy to lose.
    // * **`0.75 × heightmap cell`**, the number drop-only decimation used as
    //   its FLOOR, re-used as a ceiling — over-densifies by ~8× on this
    //   fixture scale (+55% moves) for no fidelity the chord refinement was
    //   not already going to add where the surface asks for it. Decimation
    //   never added a point; reading its floor as a ceiling misreads what it
    //   was doing.
    let flatten = {
        let base = crate::polygon::FlattenPolicy::from_chord_tolerance(chord_tolerance);
        match policy
            .sample_bound
            .max_segment_mm(cusp_r, scallop_height, chord_tolerance)
        {
            Some(mm) => base.with_max_segment(mm),
            None => base,
        }
    };
    let mut arc_rings = policy
        .cleanup
        .carries_arcs()
        .then(|| crate::polygon::OffsetRingSet::from_polygon(boundary));
    let mut current_polys = vec![boundary.clone()];

    // The loop's REAL terminator is the cascade collapsing to nothing
    // (`next_polys.is_empty()`); `max_rings` is only a runaway guard. If it
    // ever binds, the rings stop part-way and the INTERIOR of the region is
    // left uncut — silently, because the loop simply ends. That is what
    // this tracks (see the exhaustion warning below).
    let mut exhausted = true;

    // M4 §5b: accumulated regardless of `exhausted` — a cascade that
    // collapses normally can still have dropped points on individual rings
    // (e.g. a ring brushing the edge of the mesh footprint), and
    // `ScallopReport::standing_mm2` reports that independently of whether
    // the cascade was truncated.
    let mut standing_mm2 = 0.0_f64;

    for _ in 0..max_rings {
        check_cancel(cancel)?;
        // Ring stepover from the current rings' slope/curvature — MIN
        // across polygons (matching `ring_stepover`'s min-across-samples)
        // so the cusp guarantee holds on every branch of a multi-polygon
        // cascade, not just the length-weighted average one.
        //
        // M4: `policy.across_polygons` selects whether that second minimum
        // is taken at all. `PolygonReduce::MinAcross` is shipped and this
        // block is byte-identical to what it always was.
        let decisions: Vec<RingStepoverDecision> = current_polys
            .iter()
            .map(|poly| {
                ring_stepover_with_policy(&poly.exterior, slope_map, cusp_r, scallop_height, policy)
            })
            .collect();
        let clamp = |so: f64| {
            let so = if so.is_finite() {
                so
            } else {
                crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, scallop_height)
            };
            // Clamp stepover to reasonable bounds
            so.max(cusp_r * 0.05) // At least 5% of the cusp radius
                .min(cusp_r * 3.0) // At most 3× the cusp radius
        };
        let ring_so = decisions
            .iter()
            .map(|d| d.selected)
            .fold(f64::INFINITY, f64::min);
        let stepover = clamp(ring_so);

        if let Some(t) = trace.as_deref_mut() {
            let lo = decisions
                .iter()
                .map(|d| d.sample_min)
                .fold(f64::INFINITY, f64::min);
            let mid = decisions
                .iter()
                .map(|d| d.sample_p50)
                .fold(f64::INFINITY, f64::min);
            let hi = decisions
                .iter()
                .map(|d| d.sample_max)
                .fold(f64::NEG_INFINITY, f64::max);
            t.selected_mm.push(stepover);
            t.sample_spread.push((lo, mid, hi));
        }

        // Offset every live ring inward.
        //
        // History, because the shape of this code is the shape of a defect
        // that took three waves to name. Flattening cavalier's arc joins to
        // chords between offsets ADDS vertices on every call — one reflex
        // corner becomes two shallower reflex corners, each of which
        // arc-joins on the next pass — so an iterated cascade DOUBLES on
        // concave boundaries (measured exponential on wanaka's dendritic
        // mid-steep band: 1178 → 261 000 vertices by ring 25, 10 s per
        // offset; 2026-07-08, P2.c probe). P2.f damped it by dropping
        // sub-cell points; M5 measured what that cost (unbounded in world
        // units, and +9.83% eroded area where features are narrow), and
        // Checkpoint D removed the mechanism instead: the arcs are never
        // thrown away in the first place, so there is nothing to compound
        // and nothing to damp. The cascade's vertex count now SHRINKS as the
        // boundary erodes, which is what a healthy cascade does.
        //
        // Slivers whose flattened perimeter can't keep 3 points die here.
        let ring_min_spacing = heightmap.cell_size * 0.75;
        // Per-polygon offset distance. `decisions` was built by mapping over
        // `current_polys`, and the arc cascade's groups are one-to-one and
        // in-order with `current_polys`, so the two stay in lockstep on both
        // branches.
        let distance_for = |i: usize| match policy.across_polygons {
            PolygonReduce::MinAcross => stepover,
            PolygonReduce::PerPolygon => decisions.get(i).map_or(stepover, |d| clamp(d.selected)),
        };
        let next_polys = if let Some(rings) = arc_rings.take() {
            let distances: Vec<f64> = (0..rings.group_count().max(1)).map(distance_for).collect();
            let next = rings.offset_per_group(&distances);
            let polys = next.to_polygons(flatten);
            arc_rings = Some(next);
            polys
                .into_iter()
                .filter(|p| p.exterior.len() >= 3)
                .collect()
        } else {
            let mut next_polys = Vec::new();
            for (i, poly) in current_polys.iter().enumerate() {
                for offset in offset_polygon(poly, distance_for(i)) {
                    if let Some(reduced) =
                        policy
                            .cleanup
                            .apply(&offset, ring_min_spacing, chord_tolerance)
                    {
                        next_polys.push(reduced);
                    }
                }
            }
            next_polys
        };

        if next_polys.is_empty() {
            exhausted = false;
            break; // Collapsed to nothing — the intended exit
        }

        // Lift each new polygon ring to 3D
        for poly in &next_polys {
            if poly.exterior.len() < 3 {
                continue;
            }
            // G-HOLERING: the exterior, then every (grown) hole — see the
            // seed ring above.
            for ring in std::iter::once(&poly.exterior).chain(poly.holes.iter()) {
                if ring.len() < 3 {
                    continue;
                }
                let ring_3d = ring_to_3d(ring, &lift_ctx);
                if ring_3d.len() >= 3 {
                    // M4 §5b: this ring's dropped points, valued at the
                    // stepover that just produced it — see
                    // `ScallopReport::standing_mm2`'s doc for the formula.
                    standing_mm2 += dropped_arc_length_mm(&ring_3d) * stepover;
                    rings_3d.push(ring_3d);
                }
            }
        }

        current_polys = next_polys;
    }

    let mut uncut_core_mm2 = 0.0;
    // M4 §5b: hole-aware net area — `Polygon2::area()` is `|exterior| -
    // Σ|hole|`, which is exactly `uncut_core_mm2`'s exterior-only sum
    // corrected for holes. Floored at 0 per polygon rather than summed
    // signed, so one polygon's degenerate hole geometry cannot pull an
    // unrelated polygon's honest residual negative.
    let mut untouched_mm2 = 0.0;
    if exhausted {
        let remaining: f64 = current_polys
            .iter()
            .map(|p| crate::polygon::shoelace_area(&p.exterior).abs())
            .sum();
        uncut_core_mm2 = remaining;
        untouched_mm2 = current_polys.iter().map(|p| p.area().max(0.0)).sum();
        tracing::warn!(
            max_rings,
            rings_emitted = rings_3d.len(),
            uncut_core_mm2 = remaining,
            untouched_mm2,
            standing_mm2,
            // M1 §4.3: state the domain on the line that carries the number.
            measurement = %ScallopReport::PROVENANCE,
            "scallop: ring cascade hit max_rings without collapsing — the \
             INTERIOR of the region is LEFT UNCUT. Known defect: the cap is \
             budgeted from the flat-ground (widest) stepover, and raising it \
             is worse until `ring_stepover`'s min-across-ring collapse and \
             chord refinement are fixed — see the cap's derivation comment."
        );
    }

    Ok((
        rings_3d,
        RingCascadeMetrics {
            uncut_core_mm2,
            untouched_mm2,
            standing_mm2,
        },
    ))
}

/// Index of the point on `ring` closest to `target` among points that
/// satisfy `keep`; `None` when nothing on the ring survives the predicate.
///
/// Continuous mode rotates each ring to start here so the ring-to-ring
/// hop is as short as the *surviving* geometry allows — rotating to the
/// globally-closest point (kept or not) let the connector target an
/// excluded point while the tool's real position sat elsewhere, which is
/// exactly the long-cutting-chord shape the P0.4 regression tests pin.
pub(super) fn closest_kept_point_idx<F>(ring: &[(P3, bool)], target: &P3, keep: F) -> Option<usize>
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
pub(super) fn rotate_ring(ring: &[(P3, bool)], start_idx: usize) -> Vec<(P3, bool)> {
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
