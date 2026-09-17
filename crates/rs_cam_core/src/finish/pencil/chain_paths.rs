//! Chain-to-path machinery: the shared pipeline that turns one sampled
//! valley polyline into the `PencilPath`s a detector arm emits.
//!
//! Fair the XY line, lift it to the surface, measure the reach gap, offset
//! the centreline into passes, and split a lifted pass into its on-surface
//! runs. Split out of `finish/pencil.rs` (P4). The detector arms that call
//! `paths_from_sampled` live in `detectors`; the pass emitter lives in
//! `emission`.

use crate::compute::config::TipFloatFinding;
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;

use super::PencilPath;
use super::detectors::{SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM};

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate an offset polyline by shifting each point perpendicular to the
/// path direction in XY, by a PER-POINT offset distance (`offsets[i]` for
/// `points[i]`; a short `offsets` reads the missing tail as `0.0`).
///
/// # Why variable, not just scalar (C9)
///
/// A pencil fan's stepover used to be one constant applied to every point on
/// a pass ([`offset_polyline`], now a thin `offset == offsets[i]` caller of
/// this). That is still correct for the Dihedral/Curvature arms and for a
/// centreline the reach policy never measured ([`OffsetFan::stepover`]
/// empty), but a MEASURED centreline's own working width is depth-dependent
/// ([`crate::surface::reach::suggested_offset_stepover_mm`]), so a branch that runs
/// shallow at one end and deep at the other has no single honest stepover —
/// see [`paths_from_sampled`]'s doc. One geometry implementation serves both
/// cases; only the offset each point reads differs.
fn offset_polyline_variable(points: &[P3], offsets: &[f64]) -> Vec<P3> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len());

    for i in 0..points.len() {
        // Compute tangent direction at this point
        let tangent = if i == 0 {
            let d = points[1] - points[0];
            nalgebra::Vector2::new(d.x, d.y)
        } else if i == points.len() - 1 {
            let d = points[i] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        } else {
            let d = points[i + 1] - points[i - 1];
            nalgebra::Vector2::new(d.x, d.y)
        };

        let len = tangent.norm();
        if len < 1e-10 {
            result.push(points[i]);
            continue;
        }

        // Perpendicular direction in XY (rotate tangent 90° CCW)
        let normal = nalgebra::Vector2::new(-tangent.y, tangent.x) / len;
        let offset = offsets.get(i).copied().unwrap_or(0.0);

        result.push(P3::new(
            points[i].x + normal.x * offset,
            points[i].y + normal.y * offset,
            points[i].z,
        ));
    }

    result
}

/// Generate an offset polyline by shifting each point perpendicular to the
/// path direction in XY by the SAME offset distance. Thin caller of
/// [`offset_polyline_variable`] — see its doc for why the variable form
/// exists.
fn offset_polyline(points: &[P3], offset: f64) -> Vec<P3> {
    offset_polyline_variable(points, &vec![offset; points.len()])
}

/// Default fairing strength: how far each interior point moves toward the
/// midpoint of its neighbours per pass (0 = none, 1 = full Laplacian step).
pub(crate) const FAIRING_STRENGTH: f64 = 0.5;
/// Default number of fairing passes applied to each sampled chain.
pub(crate) const FAIRING_PASSES: usize = 2;

/// Lightly fair a sampled polyline in XY to remove facet-scale jaggedness. The
/// raw chain hops between 0.5mm mesh-edge vertices, so the centerline zig-zags
/// at triangulation scale; a couple of Laplacian passes straighten it without
/// moving it materially off the valley floor. Z is left untouched — the
/// subsequent `lift_to_surface` drop re-solves it gouge-safely from the faired
/// X,Y — and endpoints are pinned so chains don't shrink at their tips.
#[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
pub(super) fn fair_polyline_xy(points: &[P3], passes: usize, strength: f64) -> Vec<P3> {
    if points.len() < 3 || passes == 0 || strength <= 0.0 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    for _ in 0..passes {
        let prev = pts.clone();
        for i in 1..prev.len() - 1 {
            let a = prev[i - 1];
            let b = prev[i];
            let c = prev[i + 1];
            // Move toward the midpoint of the two neighbours (Laplacian).
            pts[i].x = b.x + strength * ((a.x + c.x) * 0.5 - b.x);
            pts[i].y = b.y + strength * ((a.y + c.y) * 0.5 - b.y);
        }
    }
    pts
}

/// Lift 2D polyline points to the mesh surface using drop-cutter. The per-point
/// drop is the hot work, so it runs in parallel when the `parallel` feature is on.
///
/// Points where the cutter makes no contact (off the mesh — this happens when
/// bisector-shifted offset points get pushed past the mesh edge) come back
/// with `z = f64::NAN` instead of the original, un-lifted Z. This makes
/// non-contact explicit so the emit loop can split the polyline at the gap
/// instead of stitching a cutting move across missing material.
fn lift_to_surface(
    points: &[P3],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
) -> Vec<P3> {
    let lift = |p: &P3| {
        let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
        if cl.contacted {
            P3::new(p.x, p.y, cl.z + stock_to_leave)
        } else {
            // Outside mesh — mark non-contact explicitly so the emit loop
            // splits the pass here instead of bridging the gap.
            P3::new(p.x, p.y, f64::NAN)
        }
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        points.par_iter().map(lift).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        points.iter().map(lift).collect()
    }
}

/// Tool-radius-aware reach gap at a concave edge's midpoint: how far the tool's
/// resting reference floats above the true surface there.
///
/// The edge midpoint lies on the mesh surface, so its Z *is* the true surface
/// height at that (x,y) — no ray needed. Dropping the actual cutter at the same
/// (x,y) gives the footprint-aware rest height (`cl.z`): on a flat or gentle
/// slope, or a triangulation-scale crease the tool simply rides over, the tool
/// reaches the surface and `gap ≈ 0`; in a concavity tighter than the tool
/// radius the tool bridges the walls and its reference floats above the floor,
/// so `gap > 0` (verified: a 6mm ball over a 0.5-slope V-valley rests 0.354mm
/// above the seam). This makes detection scale-aware — reject mesh noise, keep
/// genuine valleys — instead of trusting raw per-edge dihedral. Returns `None`
/// if the cutter makes no contact at the midpoint.
pub(super) fn reach_gap_at_point(
    x: f64,
    y: f64,
    surf_z: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) -> Option<f64> {
    let cl = point_drop_cutter(x, y, mesh, index, cutter);
    if !cl.contacted {
        return None;
    }
    Some(cl.z - surf_z)
}

/// Surface tolerance (mm): uncut depth at a concave seam above which we treat it
/// as genuine rest material worth a pencil pass. ABSOLUTE, not scaled to the tool
/// — a large tool that bridges a valley leaving e.g. 0.3mm should still be
/// cleaned, while triangulation-scale creases the tool rides over leave
/// sub-tolerance gaps and are rejected. Any noise that slips through is further
/// removed by `min_cut_length` chain filtering (isolated noise points don't form
/// long chains). Tunable; calibrate against the real mesh harness.
pub(crate) fn reach_gap_threshold() -> f64 {
    0.05
}

/// Build the centreline + offset `PencilPath`s for one already-sampled valley
/// polyline. Shared by all three detector arms: it fairs the XY line, lifts
/// it to the surface with the real cutter, and emits the centreline plus its
/// offset fan. `chain_index` is 1-based.
///
/// # The fan is ASYMMETRIC (PR-5, H2.2)
///
/// `fan` carries a count PER SIDE, not one count applied to both. The
/// Checkpoint A matrix measured left/right reach differing by up to 22×
/// (2.982 mm versus 0.136 mm on one asymmetric fixture, `§6`), and a single
/// scalar is forced to the narrow side, so the wide side was under-covered
/// BY CONSTRUCTION. The Dihedral/Curvature arms pass
/// `params.num_offset_passes` on both sides (unchanged behaviour); the
/// RestDepth arm (via [`crate::finish::crease_paths::centerline_cut_paths`]) passes
/// what the reach policy resolved.
///
/// `fan.reach` additionally truncates each pass POINT BY POINT: a pass runs
/// only where the local reach supports its offset, and elsewhere its Z is
/// marked non-contact so [`contact_runs`] splits it into the runs that are
/// real. Truncating instead of dropping is what lets a branch that is
/// reachable at one end and pinched at the other keep the reachable part —
/// the per-sample depth statistic Checkpoint A ruled for (§8.5). An empty
/// `reach` means "not measured": every pass runs full length, exactly as
/// before.
///
/// # The fan is also PER-POINT WIDE (C9)
///
/// `fan.stepover`, when non-empty, replaces the single `offset_stepover`
/// argument with one value per point: pass `k`'s offset at point `i` is
/// `sign * k * fan.stepover[i]`, not `sign * k * offset_stepover`. Before
/// C9 a MEASURED centreline (one with genuine [`RestCenterline::samples`])
/// still spaced its whole fan at one constant — usually sized from the
/// branch's shallowest reported depth
/// (`unified_finish.rs`'s `claims_offset_stepover_mm`,
/// `rest_depth_arm`'s `params.offset_stepover`) — even though
/// [`crate::surface::reach::suggested_offset_stepover_mm`] is monotone
/// non-decreasing in depth, so every deeper point on the same branch got a
/// stepover too small for its own working width: under a fixed pass-count
/// cap, passes packed closer together than necessary instead of reaching
/// out, leaving the outer part of that point's reach uncovered
/// (`tests/per_point_claims_fan_c9.rs` measures the shortfall on the
/// shipped taper). `fan.reach` still does the per-point TRUNCATION — now
/// compared against `pass_num * fan.stepover[i]` instead of
/// `pass_num * offset_stepover` — so the coverage criterion
/// `k · stepover_i ≤ reach_i` holds pointwise, not just at the reference
/// depth the scalar was sized from.
///
/// An empty `fan.stepover` means "not measured", exactly like an empty
/// `fan.reach`: every pass falls back to the flat `offset_stepover`
/// argument for every point, unchanged from before C9. This is what the
/// Dihedral/Curvature arms and an unmeasured `RestCenterline`
/// (`without_samples`) still get via [`OffsetFan::symmetric`].
///
/// [`PencilPath::offset_mm`] cannot describe a per-point-wide pass with one
/// number; it becomes the MEAN of that pass's per-point offsets when
/// `fan.stepover` drove the emission, and stays the exact scalar offset
/// otherwise. See its doc.
///
/// Takes `stock_to_leave`/`offset_stepover` as plain scalars (rather than a
/// `&PencilParams`) so non-pencil callers — currently
/// [`crate::finish::crease_paths::centerline_cut_paths`] — don't need a full
/// `PencilParams` just to emit cut paths. `pub(crate)` for that same reason.
///
/// # Tip float (Wave D1)
///
/// `float` accumulates the per-point TIP-FLOAT tally for the centreline this
/// call emits — see [`TipFloatFinding`]. It is measured here, and only here,
/// because this is the one place that both solves the cutter's resting Z and
/// still has the valley in hand; every detector arm and the unified-finish
/// crease node funnel through it, so one measurement covers all of them.
///
/// The valley floor is NOT `sampled[i].z`. The rest-depth arm's centrelines
/// already carry "Z from the pencil drop" (`RestCenterline::points`), so
/// differencing against them would measure zero by construction. The floor
/// is re-solved with the Ø0.1 mm surface probe ball the rest-depth reference
/// chain already uses for exactly this question, at the same XY. That probe
/// has its own (tiny) float in a sharp V, so the reported residual is a
/// slight UNDER-estimate — never an over-claim.
#[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
pub(crate) fn paths_from_sampled(
    sampled: &[P3],
    chain_index: usize,
    chain_total: usize,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    offset_stepover: f64,
    fan: OffsetFan<'_>,
    all_paths: &mut Vec<PencilPath>,
    float: &mut TipFloatFinding,
) {
    if sampled.len() < 2 {
        return;
    }
    // De-jag the facet-scale zig-zag before lifting; offsets derive from the
    // faired centreline so they inherit it.
    let sampled = fair_polyline_xy(sampled, FAIRING_PASSES, FAIRING_STRENGTH);
    // Per-point reach only applies when it lines up with the points it is
    // describing; fairing preserves the count, but a caller mismatch must
    // degrade to "not measured" rather than mis-attribute one point's reach
    // to another.
    let reach: &[crate::surface::reach::Reach] = if fan.reach.len() == sampled.len() {
        fan.reach
    } else {
        &[]
    };
    // Same length-match discipline as `reach`, and the same fallback: a
    // caller mismatch (or a genuinely unmeasured centreline, which passes
    // an empty slice on purpose) degrades to the flat scalar rather than
    // reading past the end or mis-attributing one point's stepover to
    // another.
    let stepover: &[f64] = if fan.stepover.len() == sampled.len() {
        fan.stepover
    } else {
        &[]
    };
    let offset_total = 1 + fan.left + fan.right;

    let centerline = lift_to_surface(&sampled, mesh, index, cutter, stock_to_leave);
    // Wave D1 instrument. Offset passes are deliberately excluded: they are
    // MEANT to ride up the walls, so "float" is not a defect there.
    let probe =
        crate::tool::BallEndmill::new(SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM);
    let valley_floor = lift_to_surface(&sampled, mesh, index, &probe, 0.0);
    for (tool_pt, floor_pt) in centerline.iter().zip(valley_floor.iter()) {
        // `stock_to_leave` is a commanded offset, not float — back it out so
        // a finishing allowance never reads as unreachable material.
        float.record((tool_pt.z - stock_to_leave) - floor_pt.z);
    }
    all_paths.push(PencilPath {
        points: centerline,
        chain_index,
        chain_total,
        offset_index: 1,
        offset_total,
        offset_mm: 0.0,
        is_centerline: true,
    });

    // Left fan, then right fan. Indices run sequentially rather than
    // even/odd because the two sides no longer have equal counts.
    let mut offset_index = 1usize;
    for (sign, passes) in [(1.0_f64, fan.left), (-1.0_f64, fan.right)] {
        for pass_num in 1..=passes {
            // Per-point offsets when the fan carries per-point stepovers
            // (C9); otherwise every point reads the same flat
            // `offset_stepover`, exactly as before. One geometry call
            // either way — `offset_polyline_variable` — so there is only
            // one implementation of "shift this polyline sideways".
            let (pts, offset_mm) = if stepover.is_empty() {
                let offset = sign * pass_num as f64 * offset_stepover;
                (offset_polyline(&sampled, offset), offset)
            } else {
                let offsets: Vec<f64> = stepover
                    .iter()
                    .map(|&s| sign * pass_num as f64 * s)
                    .collect();
                // `offset_mm` on the emitted path is a SUMMARY (the mean)
                // when the true offset varies per point — see
                // `PencilPath::offset_mm`'s doc.
                let mean = offsets.iter().sum::<f64>() / offsets.len().max(1) as f64;
                (offset_polyline_variable(&sampled, &offsets), mean)
            };
            let mut lifted = lift_to_surface(&pts, mesh, index, cutter, stock_to_leave);
            if !reach.is_empty() {
                for (i, p) in lifted.iter_mut().enumerate() {
                    // `want` is the offset THIS point's pass sits at: the
                    // per-point stepover when measured, the flat scalar
                    // otherwise — the same value that placed `p` in `pts`
                    // above, so the truncation test and the placement agree
                    // pointwise.
                    let want =
                        pass_num as f64 * stepover.get(i).copied().unwrap_or(offset_stepover);
                    let supported = reach.get(i).is_some_and(|r| {
                        !r.refused && (if sign > 0.0 { r.left_mm } else { r.right_mm }) >= want
                    });
                    if !supported {
                        // Same marker `lift_to_surface` uses for "no contact
                        // here"; `contact_runs` splits the pass on it.
                        p.z = f64::NAN;
                    }
                }
            }
            offset_index += 1;
            all_paths.push(PencilPath {
                points: lifted,
                chain_index,
                chain_total,
                offset_index,
                offset_total,
                offset_mm,
                is_centerline: false,
            });
        }
    }
}

/// The offset fan one call to [`paths_from_sampled`] should emit.
///
/// Per-side counts plus the optional per-point reach that truncates them and
/// the optional per-point stepover that spaces them — see
/// [`paths_from_sampled`]'s doc for why all three exist.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OffsetFan<'a> {
    /// Passes to emit on the `+offset` side.
    pub left: usize,
    /// Passes to emit on the `-offset` side.
    pub right: usize,
    /// Per-point reach, aligned 1:1 with the polyline handed to
    /// [`paths_from_sampled`]. Empty = not measured; no truncation.
    pub reach: &'a [crate::surface::reach::Reach],
    /// Per-point offset stepover (mm), aligned 1:1 with the polyline handed
    /// to [`paths_from_sampled`] (C9). Empty = not measured; every pass
    /// falls back to the flat `offset_stepover` argument for every point —
    /// see [`Self::symmetric`] and [`paths_from_sampled`]'s doc.
    pub stepover: &'a [f64],
}

impl OffsetFan<'_> {
    /// The legacy symmetric fan with no per-point truncation or spacing —
    /// what the Dihedral and Curvature detector arms emit, unchanged.
    pub(crate) fn symmetric(passes: usize) -> Self {
        Self {
            left: passes,
            right: passes,
            reach: &[],
            stepover: &[],
        }
    }
}

/// Split a `lift_to_surface`-lifted polyline into contiguous on-surface runs.
/// A point with `z.is_nan()` marks a spot where the cutter made no contact
/// (off the mesh); it ends the current run and is dropped rather than kept —
/// a single isolated non-contact point must not stitch its two on-surface
/// neighbors together with a cutting move. Runs of length < 2 are still
/// returned; callers skip those (mirrors the pre-split `len() < 2` guard).
pub(super) fn contact_runs(points: &[P3]) -> Vec<&[P3]> {
    crate::geometry::point_runs::split_run_ranges(points, |_, p: &P3| !p.z.is_nan(), 1)
        .into_iter()
        .filter_map(|(s, e)| points.get(s..=e))
        .collect()
}
