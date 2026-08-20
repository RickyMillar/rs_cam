//! Free-function stamping helpers used by `TriDexelStock::simulate_*` and
//! `TriDexelStock::stamp_*` methods.
//!
//! These functions operate directly on a `DexelGrid` and are axis-agnostic:
//! the caller decomposes world coordinates into `(grid_u, grid_v, ray_depth)`
//! via `StockCutDirection::decompose` and then calls into this module.
//!
//! All items are `pub(super)` so they are reachable from `mod.rs` and
//! `simulation.rs` within the `dexel_stock` module, but do not leak out of
//! the crate.

use super::band::GridBand;
use super::tile_mip::TileMaxTop;
use crate::dexel::{
    DexelGrid, ray_blend_above, ray_blend_below, ray_material_length, ray_material_length_above,
};
use crate::geo::P3;
use crate::ids::ToolpathId;
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::{CutKinematics, SimulationCutSample};
use crate::toolpath_spans::SpanId;

// ── Sub-cell coverage helpers (F.a, see DEXEL_Z_ONLY_INVESTIGATION.md §6.F) ─

/// Sub-sampling fan-out per cell for fractional disk/segment coverage.
///
/// 4×4 = 16 sub-samples gives coverage in increments of 1/16 ≈ 6.25 % at
/// boundary cells. Engagement deltas across the WANAKA corpus shift by
/// ~1–3 percentage points per §8 Step 4, so 1/16 resolution is fine.
const COVERAGE_SUBSAMPLES_PER_AXIS: usize = 4;

/// Half-extent of the 4×4 sub-sample grid as a fraction of cell size.
/// Sub-samples at axis offsets `{−3, −1, +1, +3} · cs/8`; the outermost
/// sub-sample sits at `±3·cs/8` from the cell center.
const SUBSAMPLE_HALF_EXTENT: f64 = 3.0 / 8.0;

/// Sub-sample axis offsets as fractions of `cs`. Hoisted out of the kernel
/// inner loops so the arithmetic `start + i·step` doesn't run per cell.
const SUBSAMPLE_OFFSETS_FRAC: [f64; 4] = [-3.0 / 8.0, -1.0 / 8.0, 1.0 / 8.0, 3.0 / 8.0];

/// Fractional coverage of a square cell by a disk centered at the origin
/// (offsets pre-shifted so disk center is implicit zero).
///
/// Returns `coverage ∈ [0, 1]` — the area fraction of the cell inside the
/// disk. Fast-paths fully-inside (all 16 sub-samples inside) and fully-
/// outside (all 16 sub-samples outside) using the *sub-sample* extent
/// (not the cell corner) so the fast-path bounds match what the sub-
/// sampling kernel would compute. Falls back to 4×4 sub-sampling for the
/// boundary band.
#[inline]
fn point_cell_coverage(du: f64, dv: f64, r_sq: f64, cs: f64) -> f32 {
    let extent = cs * SUBSAMPLE_HALF_EXTENT;
    let abs_du = du.abs();
    let abs_dv = dv.abs();

    // Farthest sub-sample position from disk center (in this cell's
    // worst-case quadrant). If this is inside the disk, all 16 are.
    let far_u = abs_du + extent;
    let far_v = abs_dv + extent;
    let far_sq = far_u * far_u + far_v * far_v;
    if far_sq <= r_sq {
        return 1.0;
    }

    // Nearest sub-sample position to disk center (clamped to 0 in any axis
    // where the cell straddles the center). If this is outside the disk,
    // all 16 are.
    let near_u = (abs_du - extent).max(0.0);
    let near_v = (abs_dv - extent).max(0.0);
    let near_sq = near_u * near_u + near_v * near_v;
    if near_sq >= r_sq {
        return 0.0;
    }

    // Boundary cell: sub-sample.
    let mut inside = 0u32;
    for &v_off in &SUBSAMPLE_OFFSETS_FRAC {
        let py = dv + v_off * cs;
        let py_sq = py * py;
        for &u_off in &SUBSAMPLE_OFFSETS_FRAC {
            let px = du + u_off * cs;
            if px * px + py_sq <= r_sq {
                inside += 1;
            }
        }
    }
    inside as f32 / (COVERAGE_SUBSAMPLES_PER_AXIS * COVERAGE_SUBSAMPLES_PER_AXIS) as f32
}

/// Bound on the ULP walk in [`CoverageFastPath::new`]. The naive squared
/// bound is within ~2 ULP of the exact flip point; 64 is four orders of
/// slack, and the loops are bounded so a NaN or infinity cannot spin.
const FAST_PATH_ULP_SEARCH_LIMIT: usize = 64;

/// The two squared-space thresholds `segment_cell_coverage`'s fast paths test
/// against, solved once per stamp instead of per cell (PERF_REVIEW S7).
///
/// The fast paths ask whether the *worst-case* sub-sample of a cell is outside
/// the cutter (`center_d − ext_diag ≥ r`) or the *best*-case one is inside
/// (`center_d + ext_diag ≤ r`), where `ext_diag = cs·SUBSAMPLE_HALF_EXTENT·√2`
/// is the corner sub-sample's offset from the cell centre. The kernel already
/// holds `center_d_sq`, so answering these without a per-cell `sqrt` is worth
/// real time — the loop used to pay TWO, and one of them (`r_sq.sqrt()`) was a
/// loop invariant being recomputed for every cell in the swept bounding box.
///
/// # Why this is not just `(r ± ext_diag)²`
///
/// It is tempting — and `PERF_REVIEW.md` S7 says so — that the tests are
/// "exact in squared space against hoisted `(r ± ext_diag)²`". **They are
/// not.** In exact arithmetic they would be; in `f64` the two forms round
/// differently, and `squared_fast_paths_agree_with_the_sqrt_form` catches it
/// at the boundary. The clearest case: at `d = fl(r + ext_diag)` the squared
/// form sees `d² == outer_sq` and takes the fast path, while the sqrt form
/// evaluates `fl(fl(r + ext_diag) − ext_diag)`, which lands *below* `r`, and
/// falls through to sub-sampling. A denser sweep finds the disagreement going
/// the other way too, and no choice of strict-vs-non-strict comparison removes
/// it — the rounding paths simply differ by an ULP either side of both bounds.
///
/// # What is done instead
///
/// Both legacy predicates are **monotone in `d_sq`** (`sqrt` is monotone and
/// correctly rounded; adding a constant and comparing preserve that). A
/// monotone predicate over a totally ordered domain has an exact flip point,
/// so rather than approximating it, this *solves* for it: start from the naive
/// squared bound and walk ULPs until the legacy predicate's own answer flips.
/// The result is exact **by construction** — the threshold is defined as the
/// boundary of the old test, not derived from an algebraic identity that only
/// holds over the reals. The walk costs a handful of `sqrt`s once per stamp
/// and buys back one per cell.
#[derive(Clone, Copy)]
struct CoverageFastPath {
    /// Smallest `d_sq` for which the legacy test `√d_sq − ext_diag ≥ r` holds.
    /// At or past it, every sub-sample is outside the cutter ⇒ coverage 0.
    outer_sq: f64,
    /// Largest `d_sq` for which the legacy test `√d_sq + ext_diag ≤ r` holds,
    /// or `-1.0` when `ext_diag > r`.
    ///
    /// The negative sentinel matters: when the sub-sample fan is wider than
    /// the cutter — a Ø0.5 tool on a 1 mm grid, which is a real fine-tool
    /// regime — no cell can satisfy the legacy test at all, because distances
    /// are non-negative. `-1.0` makes `center_d_sq <= inner_sq` false for
    /// every cell. Squaring the negative `r − ext_diag` instead would flip the
    /// test's sense and report FULL coverage for cells near the tool axis.
    inner_sq: f64,
}

impl CoverageFastPath {
    fn new(r_sq: f64, cs: f64) -> Self {
        let ext_diag = cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;
        let r = r_sq.sqrt();

        // `outer`: smallest t with `√t − ext_diag ≥ r`. Monotone increasing,
        // so walk up until it holds, then down to the first t that still does.
        let outside = |t: f64| t.sqrt() - ext_diag >= r;
        let mut outer = (r + ext_diag) * (r + ext_diag);
        for _ in 0..FAST_PATH_ULP_SEARCH_LIMIT {
            if outside(outer) {
                break;
            }
            outer = outer.next_up();
        }
        for _ in 0..FAST_PATH_ULP_SEARCH_LIMIT {
            let down = outer.next_down();
            if down < 0.0 || !outside(down) {
                break;
            }
            outer = down;
        }

        // `inner`: largest t with `√t + ext_diag ≤ r`. Monotone DEcreasing, so
        // walk down until it holds, then up to the last t that still does.
        let inside = |t: f64| t.sqrt() + ext_diag <= r;
        let inner = if ext_diag > r {
            // Unsatisfiable for every cell — see the field docs.
            -1.0
        } else {
            let mut inner = (r - ext_diag) * (r - ext_diag);
            for _ in 0..FAST_PATH_ULP_SEARCH_LIMIT {
                if inner <= 0.0 || inside(inner) {
                    break;
                }
                inner = inner.next_down();
            }
            for _ in 0..FAST_PATH_ULP_SEARCH_LIMIT {
                let up = inner.next_up();
                if !inside(up) {
                    break;
                }
                inner = up;
            }
            inner.max(0.0)
        };

        Self {
            outer_sq: outer,
            inner_sq: inner,
        }
    }
}

/// Fractional coverage of a square cell by a swept-segment stadium.
///
/// `(cu, cv)` is the cell center; the segment goes from `(su, sv)` through
/// direction `(seg_du, seg_dv)` with length² = `seg_len_sq`. `r_sq` is the
/// cutter radius squared and `fast` carries its
/// [`CoverageFastPath`] bounds.
///
/// Returns `(coverage, t_at_closest, near_dist_sq)`:
/// - `coverage` ∈ [0, 1] — area fraction inside the stadium.
/// - `t_at_closest` — parameter t ∈ [0, 1] at the closest point on segment
///   to cell center (used for per-cell depth interpolation).
/// - `near_dist_sq` — squared distance from cell center's segment-projection
///   to the cell (the "closest in-stadium point" distance), useful as the
///   LUT query (§6.F gap 3).
#[allow(clippy::too_many_arguments)]
#[inline]
fn segment_cell_coverage(
    cu: f64,
    cv: f64,
    su: f64,
    sv: f64,
    seg_du: f64,
    seg_dv: f64,
    inv_seg_len_sq: f64,
    r_sq: f64,
    cs: f64,
    fast: CoverageFastPath,
) -> (f32, f64, f64) {
    // Project cell center onto segment.
    let pu = cu - su;
    let pv = cv - sv;
    let t_center = ((pu * seg_du + pv * seg_dv) * inv_seg_len_sq).clamp(0.0, 1.0);
    let closest_u = t_center * seg_du;
    let closest_v = t_center * seg_dv;
    let du = pu - closest_u;
    let dv = pv - closest_v;
    let center_d_sq = du * du + dv * dv;

    // Fast paths using the sub-sample extent (matching the kernel below).
    // The 16 sub-samples sit within `cs·SUBSAMPLE_HALF_EXTENT·√2` of the
    // cell center (worst-case corner sub-sample), so the swept-stadium
    // distance from any sub-sample lies within `[center_d − ext_diag,
    // center_d + ext_diag]` of the cell-center's distance to the segment.
    // Both tests are done in squared space — see [`CoverageFastPath`].
    if center_d_sq >= fast.outer_sq {
        return (0.0, t_center, center_d_sq);
    }
    if center_d_sq <= fast.inner_sq {
        return (1.0, t_center, center_d_sq);
    }

    // Boundary cell: sub-sample. Each sub-sample re-projects onto segment.
    let mut inside = 0u32;
    for &v_off in &SUBSAMPLE_OFFSETS_FRAC {
        let py = cv + v_off * cs - sv;
        for &u_off in &SUBSAMPLE_OFFSETS_FRAC {
            let px = cu + u_off * cs - su;
            let t = ((px * seg_du + py * seg_dv) * inv_seg_len_sq).clamp(0.0, 1.0);
            let qu = px - t * seg_du;
            let qv = py - t * seg_dv;
            if qu * qu + qv * qv <= r_sq {
                inside += 1;
            }
        }
    }
    let cov = inside as f32 / (COVERAGE_SUBSAMPLES_PER_AXIS * COVERAGE_SUBSAMPLES_PER_AXIS) as f32;
    (cov, t_center, center_d_sq)
}

/// F.a coverage gate for perp-extent contribution (§6.F). Multiplicative
/// sub-cell blend leaves residual material at boundary cells (any cell with
/// coverage < 1) that subsequent passes "bite", which would otherwise inflate
/// radial engagement on repeated passes over already-cleared territory (e.g.
/// `radial_engagement_air_cut_reads_zero`).
///
/// Requiring `coverage ≥ 0.95` means only cells that this stamp covers
/// essentially-fully contribute to the width-of-cut measurement. With 4×4
/// sub-sampling (1/16 quantization), the gate is equivalent to "cov = 1.0" —
/// only fast-path fully-inside cells contribute. For a full slot this still
/// yields radial ≈ (2r − 2·cell_size_subsample) / (2r) ≈ 0.97 (above the
/// existing `> 0.85` slot assertion), and on air cuts over previously-cleared
/// paths it reads exactly zero (the cov=1.0 cells were cleared by the prior
/// pass, so pre_fresh = 0).
///
/// This is census §5.1's **Floor 2**, and unlike
/// [`FRESH_MATERIAL_THRESHOLD_MM`] it *is* a function of cell size: two
/// distinct qualifying cell centres at different perpendicular offsets are
/// needed for a width to exist at all, so a round-tip tool at cut depth `d`
/// needs `cell ≲ √(2·R_tip·d − d²)`. A Ø1 mm ball tip at `d = 0.05 mm`
/// has a contact radius of ≈ 0.218 mm, so a 0.25 mm grid yields at most one
/// qualifying cell and reads zero. That is the quantitative form of the
/// standing rule "sim cell must be well below the tool TIP radius".
const PERP_COVERAGE_GATE: f32 = 0.95;

/// Threshold for "fresh material exists above the cutter at this cell",
/// millimetres. A cell contributes to the perpendicular-extent measurement —
/// and therefore to `radial_engagement`, to the engagement arc derived from
/// it, and to everything downstream (`average_engagement`, `air_cut_time_s`
/// and both its percentages, and chip thickness) — only if it held more than
/// this much material above the cutter surface *before* the stamp.
///
/// **The chipload gate is no longer on that downstream list** (2026-08-06):
/// it observes advance per tooth, which is kinematic and does not read this
/// floor. Chip thickness still does, and so does everything above.
///
/// **This is a documented measurement limit, not a tunable.** Ruled at
/// Checkpoint D (Q2/D-7, 2026-08-04) after
/// `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §5.1 measured what it does: a pass
/// removing 0.02 mm per stamp removes real material (63.7 mm³ on the census
/// fixture), reports its removed *height* correctly, and reports radial
/// engagement of **exactly zero** and ~96% air cut. Lowering the number
/// re-admits the float-noise cells it exists to reject — near-flush dexel
/// artifacts where a previous stamp left material fractionally above the
/// cutter surface. Real bites are mm-scale.
///
/// The honest fix is therefore not a smaller constant but an **abstention**:
/// [`crate::sim_measurability`] detects passes sitting under this floor and
/// marks the engagement-derived metrics `NotMeasurable`, so the gates that
/// consume them decline to produce a verdict instead of publishing a
/// precise-looking percentage that clears every bar. Collision detection and
/// gross material removal are unaffected and stay live.
///
/// Note this floor is **independent of cell size**. There is a second,
/// separate lateral-resolution condition (`cell ≲ √(2·R_tip·d − d²)`,
/// census §5.1 "Floor 2") governed by [`PERP_COVERAGE_GATE`]; the two fail
/// for different reasons and `sim_measurability` reports them apart.
pub const FRESH_MATERIAL_THRESHOLD_MM: f64 = 0.05;

/// Coverage at or above which a cell counts as **completely** swept, and the
/// sliver-safe bound (`DexelGrid::conservative_top`) may be lowered.
///
/// A/M10. Not `1.0` exactly: coverage is measured by a 4×4 sub-sample fan
/// (`SUBSAMPLE_N`), so a cell genuinely inside the cutter reports `1.0` only
/// up to that quantisation. `1.0 - 1/32` sits half a sub-sample below full
/// and cannot be reached by a cell that has any sub-sample outside the
/// cutter.
const FULL_COVERAGE: f32 = 1.0 - 1.0 / 32.0;

/// Upper bound of the removal surface across a WHOLE cell, for the
/// sliver-safe channel.
///
/// The stamping kernels evaluate the cutter profile at the cell CENTRE,
/// which is the right answer for the cell's own sample and the wrong one for
/// a bound: every cutter profile in `crate::tool` rises monotonically with
/// radial distance, so the highest point of the cutter surface over a square
/// cell is at the sub-sample farthest from the tool axis. `near_dist` is the
/// centre's distance; the half-diagonal `cs·√2/2` reaches the corner.
///
/// `depth_max` is the highest tip position the stamp reaches over the cell —
/// for a swept segment that is the higher of its two endpoints, not the
/// interpolated value at the cell centre.
#[inline]
fn cell_upper_bound_surface(
    lut: &RadialProfileLUT,
    near_dist_sq: f64,
    cs: f64,
    depth_max: f64,
) -> Option<f64> {
    let far = near_dist_sq.sqrt() + cs * std::f64::consts::SQRT_2 * 0.5;
    let far_sq = (far * far).min(lut.radius_sq());
    lut_h_with_edge_fallback(lut, far_sq).map(|h| depth_max + h)
}

/// LUT query for an annular cell at squared distance `dist_sq` from disk
/// center: clamp to the cutter edge if the cell center sits outside the
/// disk (§6.F gap 3). Returns `None` only if the LUT returns `None` at the
/// query point (e.g., cutter has a hollow center — not used in practice).
#[inline]
fn lut_h_with_edge_fallback(lut: &RadialProfileLUT, dist_sq: f64) -> Option<f64> {
    if dist_sq <= lut.radius_sq() {
        lut.height_at_dist_sq(dist_sq)
    } else {
        // Annular cell: query at the cutter edge.
        lut.height_at_dist_sq(lut.radius_sq())
    }
}

/// The lowest tip position the segment kernel can evaluate, in the same
/// floating-point arithmetic the kernel itself uses (PERF_REVIEW S2).
///
/// The per-cell tip height is `sd + t_center * seg_dd` with
/// `t_center ∈ [0, 1]` — `clamp`ed, so both endpoints are attainable.
/// `min(sd, ed)` is the obvious answer and it is **wrong at the last bit**:
/// `seg_dd` is `fl(ed − sd)`, and `fl(sd + seg_dd)` need not equal `ed`. Both
/// `fl(t·seg_dd)` and `fl(sd + x)` are monotone in their arguments, so the
/// attainable minimum is the value at whichever endpoint minimises them —
/// which is what this returns, exactly.
///
/// Getting this wrong by one ULP is precisely the class of defect that made
/// the review's S7 "exact in squared space" claim false and G3's
/// `bbox.max.z <= cl.z` reject unsound. The skip is only exact if the bound
/// it compares against is the bound the kernel actually reaches.
#[inline]
fn segment_tip_low(sd: f64, seg_dd: f64) -> f64 {
    if seg_dd < 0.0 { sd + seg_dd } else { sd }
}

/// Can a stamp whose lowest tip position is `tip_lo` remove anything at a cell
/// whose sliver-safe bound is `conservative_top`?
///
/// `false` ⇒ the cell is **inert** for this stamp: `ray_blend_above` is a
/// no-op, `ray_material_length_above` at the cutter surface is `0.0`, and
/// `lower_conservative_top` is a no-op. The argument, and every step of it is
/// load-bearing:
///
/// 1. `ray_top <= conservative_top` — maintained by design (see
///    [`DexelGrid::conservative_top`]); it is lowered only to an upper bound
///    of the cutter surface across the whole cell, and only when the cell is
///    covered end to end, in which case the blend is a plain
///    `ray_subtract_above` to a surface at or below that bound.
/// 2. `conservative_top <= tip_lo <= tip <= tip + h(d)` — the second step
///    needs `h >= 0`, which is why the caller gates on
///    [`RadialProfileLUT::profile_is_nonneg_total`] rather than assuming it.
/// 3. The surface the kernel actually writes is `(tip + h) as f32`. Rounding
///    to nearest is **monotone**, and `conservative_top` is already an `f32`,
///    so `f32(tip + h) >= f32(conservative_top as f64) = conservative_top >=
///    ray_top`. The `f32`/`f64` boundary does not open a gap.
///
/// Non-strict `<=` throughout is deliberate and checked: at exact equality
/// `ray_blend_above` skips the segment (`seg.exit <= above_lo`),
/// `ray_material_length_above` skips it (`seg.exit <= z`), and
/// `lower_conservative_top` skips it (`surface < self.conservative_top[idx]`
/// is false). Equality is the common case, not a corner one — a flat end mill
/// re-passing ground it already cut at the same Z hits it on every cell.
#[inline]
fn cell_can_remove(conservative_top: f32, tip_lo: f64) -> bool {
    let top = conservative_top as f64;
    // Spelled out rather than `!(top <= tip_lo)` so the NaN arm is explicit:
    // an unorderable bound must never be read as "inert".
    top > tip_lo || top.is_nan() || tip_lo.is_nan()
}

#[derive(Clone, Copy)]
pub(super) struct CuttingCaptureParams<'a> {
    pub(super) toolpath_id: ToolpathId,
    pub(super) move_index: usize,
    pub(super) feed_rate_mm_min: f64,
    pub(super) spindle_rpm: u32,
    pub(super) flute_count: u32,
    pub(super) semantic_item_id: Option<u64>,
    pub(super) span_path: &'a [SpanId],
    pub(super) sample_step_mm: f64,
    pub(super) cut_kinematics: CutKinematics,
    pub(super) capture_arc_engagement: bool,
    /// P3: move sits in a transit-style span (Entry/LinkBridge/LeadOut/
    /// WaterlineCleanup/DressupArtifact). Marks every sample emitted from
    /// this move as `in_transit_span = true`.
    pub(super) in_transit_span: bool,
    /// R-11: the generator's own `MoveIntent` for this move, carried onto
    /// every emitted sample. See `SimulationCutSample::source_intent`.
    pub(super) source_intent: Option<crate::toolpath::MoveIntent>,
}

// ── Grid-generic stamp helpers ───────────────────────────────────────────

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Stamp a tool at a single position on a grid (axis-agnostic).
///
/// `(cu, cv)` is the tool center in the grid's planar axes.
/// `tip_depth` is the tool tip coordinate along the grid's ray axis.
/// `from_high` selects `subtract_above` (true) or `subtract_below` (false).
///
/// Uses sub-cell area-weighted coverage (F.a, see
/// `DEXEL_Z_ONLY_INVESTIGATION.md` §6.F): boundary cells are blended toward
/// the cutter surface by their fractional coverage `f` instead of flipping
/// binary on/off at the cell-center crossing.
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp_point_on_grid(
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    cu: f64,
    cv: f64,
    tip_depth: f64,
    from_high: bool,
    mip: Option<&mut TileMaxTop>,
) {
    // S2. This kernel has no metric accumulators at all, so an inert cell can
    // simply be dropped — no `pre_volume`/`post_volume` bookkeeping to
    // reproduce, unlike `stamp_segment_with_metrics`.
    let air_skip = mip.is_some() && from_high && lut.profile_is_nonneg_total();
    let mut mip = if air_skip { mip } else { None };
    let cs = grid.cell_size;
    // §6.F gap 3: extend bounding box past `r` so annular cells (centers
    // outside disk but sub-samples reaching into it) are visited. The
    // sub-sample half-extent diagonal is `cs·SUBSAMPLE_HALF_EXTENT·√2 ≈
    // 0.53 cs`; floor/ceil rounding adds a further ~cs margin. The kernel
    // returns coverage=0 for genuinely-outside cells via its fast-outside
    // check, so a tight scan_radius is fine.
    let scan_radius = radius + cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;

    let col_min = ((cu - scan_radius - grid.origin_u) / cs).floor() as isize;
    let col_max = ((cu + scan_radius - grid.origin_u) / cs).ceil() as isize;
    let row_min = ((cv - scan_radius - grid.origin_v) / cs).floor() as isize;
    let row_max = ((cv + scan_radius - grid.origin_v) / cs).ceil() as isize;

    let col_lo = col_min.max(0) as usize;
    let col_hi = (col_max as usize).min(grid.cols.saturating_sub(1));
    let row_lo = row_min.max(0) as usize;
    let row_hi = (row_max as usize).min(grid.rows.saturating_sub(1));

    if let Some(m) = mip.as_mut() {
        let bbox_cells = (row_hi + 1 - row_lo).saturating_mul(col_hi + 1 - col_lo);
        m.refresh_if_due(grid);
        m.charge(bbox_cells as u64);
        if !cell_can_remove(m.max_over(row_lo, row_hi, col_lo, col_hi), tip_depth) {
            m.note_stamp_skipped();
            return;
        }
        m.note_stamp_run(bbox_cells as u64, 0);
    }

    let r_sq = lut.radius_sq();

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        let dv = cell_v - cv;
        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let du = cell_u - cu;
            let coverage = point_cell_coverage(du, dv, r_sq, cs);
            if coverage <= 0.0 {
                continue;
            }
            if air_skip && !cell_can_remove(grid.conservative_top_at(row, col), tip_depth) {
                continue;
            }
            // §6.F gap 3: interior cells query h at the cell center; annular
            // cells (where the center sits outside the disk) fall back to h
            // at the cutter edge.
            let dist_sq = du * du + dv * dv;
            let Some(h) = lut_h_with_edge_fallback(lut, dist_sq) else {
                continue;
            };
            let idx = row * grid.cols + col;
            let ray = &mut grid.rays[idx];
            if from_high {
                let surface = (tip_depth + h) as f32;
                ray_blend_above(ray, surface, coverage);
            } else {
                let surface = (tip_depth - h) as f32;
                ray_blend_below(ray, surface, coverage);
            }
            // A/M10: only a cell swept end to end may lower the sliver-safe
            // bound, and only to the cutter's highest point across that cell.
            if from_high
                && coverage >= FULL_COVERAGE
                && let Some(ub) = cell_upper_bound_surface(lut, dist_sq, cs, tip_depth)
            {
                grid.lower_conservative_top(idx, ub as f32);
            }
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Stamp a tool along a linear segment on a grid (axis-agnostic).
///
/// `start` and `end` are `(u, v, depth)` — the segment endpoints decomposed
/// into the grid's planar axes (u, v) and ray-depth axis (depth).
///
/// Uses sub-cell coverage (F.a — see [`stamp_point_on_grid`]) to area-weight
/// per-cell ray updates against the swept-stadium footprint.
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp_segment_on_grid(
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    from_high: bool,
    mip: Option<&mut TileMaxTop>,
) {
    let (su, sv, sd) = start;
    let (eu, ev, ed) = end;
    let seg_du = eu - su;
    let seg_dv = ev - sv;
    let seg_dd = ed - sd;
    let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;

    // S2 — see `stamp_point_on_grid`. No accumulators here either.
    let air_skip = mip.is_some() && from_high && lut.profile_is_nonneg_total();
    let mut mip = if air_skip { mip } else { None };

    // Degenerate segment (zero planar length) — stamp at the min depth.
    if seg_len_sq < 1e-20 {
        let d = sd.min(ed);
        stamp_point_on_grid(grid, lut, radius, su, sv, d, from_high, mip);
        return;
    }

    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let cs = grid.cell_size;
    let r_sq = lut.radius_sq();
    // S7: the coverage fast-path radii are constant across the whole stamp.
    let fast = CoverageFastPath::new(r_sq, cs);
    // §6.F gap 3: extend scan by cs·√2 to capture annular cells.
    let scan_radius = radius + cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;

    let u_min = su.min(eu) - scan_radius;
    let u_max = su.max(eu) + scan_radius;
    let v_min = sv.min(ev) - scan_radius;
    let v_max = sv.max(ev) + scan_radius;

    let col_lo = ((u_min - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_hi = (((u_max - grid.origin_u) / cs).ceil() as usize).min(grid.cols.saturating_sub(1));
    let row_lo = ((v_min - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_hi = (((v_max - grid.origin_v) / cs).ceil() as usize).min(grid.rows.saturating_sub(1));

    let tip_lo = segment_tip_low(sd, seg_dd);
    if let Some(m) = mip.as_mut() {
        let bbox_cells = (row_hi + 1 - row_lo).saturating_mul(col_hi + 1 - col_lo);
        m.refresh_if_due(grid);
        m.charge(bbox_cells as u64);
        if !cell_can_remove(m.max_over(row_lo, row_hi, col_lo, col_hi), tip_lo) {
            m.note_stamp_skipped();
            return;
        }
        m.note_stamp_run(bbox_cells as u64, 0);
    }

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let (coverage, t_center, center_d_sq) = segment_cell_coverage(
                cell_u,
                cell_v,
                su,
                sv,
                seg_du,
                seg_dv,
                inv_seg_len_sq,
                r_sq,
                cs,
                fast,
            );
            if coverage <= 0.0 {
                continue;
            }
            if air_skip && !cell_can_remove(grid.conservative_top_at(row, col), tip_lo) {
                continue;
            }
            let Some(h) = lut_h_with_edge_fallback(lut, center_d_sq) else {
                continue;
            };
            let depth = sd + t_center * seg_dd;
            let idx = row * grid.cols + col;
            let ray = &mut grid.rays[idx];
            if from_high {
                let surface = (depth + h) as f32;
                ray_blend_above(ray, surface, coverage);
            } else {
                let surface = (depth - h) as f32;
                ray_blend_below(ray, surface, coverage);
            }
            // A/M10 — see `stamp_point_on_grid`. The tip height is
            // interpolated at the cell centre, so the bound takes the higher
            // endpoint: a ramping segment must not be credited with the
            // deeper end of its own travel.
            if from_high
                && coverage >= FULL_COVERAGE
                && let Some(ub) = cell_upper_bound_surface(lut, center_d_sq, cs, sd.max(ed))
            {
                grid.lower_conservative_top(idx, ub as f32);
            }
        }
    }
}

/// One row band's share of a single stamp's metrics (PERF_REVIEW S3).
///
/// The metric kernel used to return the four published numbers directly. It
/// now returns the *accumulators* instead, so a stamp split across row bands
/// can be reduced before those numbers are formed. With one band covering
/// every row — the serial path — [`Self::finish`] reproduces the pre-S3
/// expressions verbatim.
///
/// # Which of these reduce exactly and which do not
///
/// `max_penetration` is a max, `perp_min`/`perp_max` are a min and a max:
/// order-independent, so banding cannot move them. `pre_volume` and
/// `post_volume` are **sums**, and `(a₁+a₂)+(a₃+a₄)` is not
/// `((a₁+a₂)+a₃)+a₄` in `f64` — so banding reassociates the removed-volume
/// sum. That is a real, if tiny, departure from the serial value, and it is
/// why the review's "bit-identical" claim for S3 does not survive contact
/// with the volume channel; see `band.rs`.
#[derive(Clone, Copy, Debug)]
pub(super) struct StampPartial {
    pub(super) pre_volume: f64,
    pub(super) post_volume: f64,
    pub(super) max_penetration: f64,
    pub(super) perp_min: f64,
    pub(super) perp_max: f64,
    /// The pure-vertical branch, which has one running sum instead of a
    /// pre/post pair. Every band agrees on this — it is a property of the
    /// segment, not of the cells.
    pub(super) degenerate: bool,
    pub(super) descent: f64,
    pub(super) removed_volume: f64,
    /// Cells inside this band's clipped bounding box. Diagnostics for the S2
    /// sentries and the delta doc; no production reader.
    pub(super) bbox_cells: u64,
    pub(super) cells_skipped: u64,
    pub(super) stamp_skipped: bool,
}

impl StampPartial {
    pub(super) fn empty() -> Self {
        Self {
            pre_volume: 0.0,
            post_volume: 0.0,
            max_penetration: 0.0,
            perp_min: f64::INFINITY,
            perp_max: f64::NEG_INFINITY,
            degenerate: false,
            descent: 0.0,
            removed_volume: 0.0,
            bbox_cells: 0,
            cells_skipped: 0,
            stamp_skipped: true,
        }
    }

    /// Fold another band's share of the SAME stamp into this one.
    ///
    /// Bands are merged in row order, so the reassociation is fixed by the
    /// grid geometry and not by which thread finished first.
    pub(super) fn merge(&mut self, other: &Self) {
        self.pre_volume += other.pre_volume;
        self.post_volume += other.post_volume;
        self.removed_volume += other.removed_volume;
        if other.max_penetration > self.max_penetration {
            self.max_penetration = other.max_penetration;
        }
        if other.perp_min < self.perp_min {
            self.perp_min = other.perp_min;
        }
        if other.perp_max > self.perp_max {
            self.perp_max = other.perp_max;
        }
        self.degenerate |= other.degenerate;
        if other.descent > self.descent {
            self.descent = other.descent;
        }
        self.bbox_cells += other.bbox_cells;
        self.cells_skipped += other.cells_skipped;
        self.stamp_skipped &= other.stamp_skipped;
    }

    /// Form the four published numbers. Byte-for-byte the pre-S3 expressions.
    pub(super) fn finish(
        &self,
        radius: f64,
        capture_arc_engagement: bool,
    ) -> (f64, f64, Option<f64>, f64) {
        if self.degenerate {
            let radial = if self.removed_volume > 1e-9 { 1.0 } else { 0.0 };
            return (self.descent, radial, None, self.removed_volume);
        }
        // Width of cut perpendicular to motion / tool diameter.
        let radial_engagement = if self.perp_max > self.perp_min {
            ((self.perp_max - self.perp_min) / (2.0 * radius)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Engagement arc derived geometrically from radial engagement. The
        // conventional CAM relationship (one-sided side bite of width W = D ·
        // radial_engagement on a cutter of diameter D): arc = arccos(1 − 2 ·
        // radial). Reads π/2 at half-immersion (RWoC=0.5) and π at full slot
        // (RWoC=1.0) — matching the tooth-load arc convention the chipload
        // formula expects.
        //
        // Computing this from radial rather than per-cell bearing binning
        // avoids the dense-sample lune artifact: a per-cell scan over engaged
        // cells in the midpoint disk only sees the thin sliver of fresh
        // material between consecutive overlapping samples, and its bearing
        // extent is much smaller than the steady-state engagement arc the
        // chipload formula expects. The radial measurement (perp extent of
        // fresh cells) is more robust because the bite zone has nontrivial
        // perp extent even when the sliver is thin.
        let arc_engagement_radians = if capture_arc_engagement {
            let arc = if radial_engagement > 0.0 {
                let one_minus_two_w_over_d = 1.0 - 2.0 * radial_engagement;
                one_minus_two_w_over_d.clamp(-1.0, 1.0).acos()
            } else {
                0.0
            };
            Some(arc.clamp(0.0, std::f64::consts::TAU))
        } else {
            None
        };
        (
            self.max_penetration.max(0.0),
            radial_engagement,
            arc_engagement_radians,
            (self.pre_volume - self.post_volume).max(0.0),
        )
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
#[allow(clippy::too_many_arguments)]
/// Stamp a tool along a linear segment AND compute metrics in a single pass.
///
/// Fuses the work of `stamp_segment_on_grid`, `estimate_disk_cut_metrics`, and
/// two calls to `window_material_volume_for_segment` into one row/col loop.
///
/// `radial_engagement` is the conventional CAM "radial width of cut over tool
/// diameter" — measured as the perpendicular extent of cells that contained
/// fresh material above the cutter surface BEFORE the stamp. This is
/// independent of sample density: dense overlapping samples in already-cleared
/// territory don't double-count, but a leading-edge bite of stepover σ
/// reads σ/(2R) regardless of how finely we sample the toolpath.
///
/// `arc_engagement_radians` uses the same pre-stamp-fresh-material gate
/// (binned by bearing relative to motion, restricted to the leading half).
/// The chipload formula consumes this value, so the same density-independence
/// matters: dense samples on adaptive cuts must not collapse arc engagement
/// to a thin sliver.
///
/// Returns `(axial_doc_mm, radial_engagement, arc_engagement_radians, volume_removed_mm3)`,
/// where `axial_doc_mm` is the maximum per-cell material length removed by
/// this stamp.
///
/// # The S2 air-skip
///
/// `mip`, when supplied, enables the tile early-out of `PERF_REVIEW.md` S2 at
/// two granularities. Both are **exact** — they produce bit-identical grids,
/// bit-identical `conservative_top`, and a bit-identical return tuple — and
/// the exactness argument is written out at [`cell_can_remove`].
///
/// * **Whole stamp.** If the mip's bound over the swept bounding box is at or
///   below the lowest tip position, every cell is inert and the function
///   returns without touching the grid. `pre_volume` and `post_volume` would
///   have accumulated *identical addend sequences in identical order*, so
///   their difference is exactly `0.0`; `perp_max > perp_min` is false so
///   radial is `0.0`; `max_penetration` never leaves `0.0`.
/// * **Per cell.** A cell whose own `conservative_top` is at or below the tip
///   floor still has to contribute `pre_len·cell_area` to **both** volume
///   accumulators, because `post_len == pre_len` bit-for-bit on an inert cell
///   and the two sums are taken *separately* and differenced at the end.
///   Dropping the pair — the literal "skip the tile" of the review's text —
///   would reassociate the volume sum and move `removed_volume_est_mm3` in its
///   last bits. Keeping the pair costs one ray walk and buys the LUT probe,
///   `ray_material_length_above`, the blend, the second ray walk, the
///   `cell_upper_bound_surface` sqrt, the `conservative_top` read-modify-write
///   and the whole engagement block.
pub(super) fn stamp_segment_with_metrics(
    band: &mut GridBand<'_>,
    lut: &RadialProfileLUT,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    mid_u: f64,
    mid_v: f64,
    from_high: bool,
    mip: Option<&TileMaxTop>,
) -> StampPartial {
    let (su, sv, sd) = start;
    let (eu, ev, ed) = end;
    let seg_du = eu - su;
    let seg_dv = ev - sv;
    let seg_dd = ed - sd;
    let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;
    let degenerate = seg_len_sq < 1e-20;

    // S2: the skip reasons about `tip + h(d)` from below, so it needs
    // `h >= 0`, and it reproduces the kernel's own `None`-means-no-
    // contribution branch, so it needs the profile to be total. Both are read
    // off the table that will actually be queried. `from_high` because
    // `conservative_top` is a high-side channel only.
    let air_skip = mip.is_some() && from_high && lut.profile_is_nonneg_total();

    let mut out = StampPartial::empty();
    out.degenerate = degenerate;
    if degenerate {
        out.descent = (sd - ed).abs();
    }
    if band.rows == 0 || band.cols == 0 {
        return out;
    }

    let cs = band.cell_size;
    let cell_area = cs * cs;
    // §6.F gap 3: extend scan by cs·√2 to capture annular cells.
    let scan_radius = radius + cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;

    // Degenerate segment (pure-vertical, e.g. drill plunge): planar offset is
    // zero, so the metric formulas below would divide by zero. Stamp at the
    // segment's bottom and compute volume by measuring the ray-by-ray drop
    // in material height. axial_doc is the Z descent; radial_engagement is
    // 1.0 when material is actually being removed (drill bites full-flute).
    if degenerate {
        let d = sd.min(ed);
        let r_sq = lut.radius_sq();

        let col_min = ((su - scan_radius - band.origin_u) / cs).floor() as isize;
        let col_max = ((su + scan_radius - band.origin_u) / cs).ceil() as isize;
        let row_min = ((sv - scan_radius - band.origin_v) / cs).floor() as isize;
        let row_max = ((sv + scan_radius - band.origin_v) / cs).ceil() as isize;
        // GLOBAL bounding box first — the mip is asked about the whole stamp,
        // not about this band's share of it. See the note at the swept
        // branch's own query: a band-local whole-stamp skip is not exact.
        let col_lo_g = col_min.max(0) as usize;
        let col_hi_g = col_max.max(0) as usize;
        let row_lo_g = row_min.max(0) as usize;
        let row_hi_g = row_max.max(0) as usize;
        if let Some(m) = mip
            && air_skip
            && !cell_can_remove(m.max_over(row_lo_g, row_hi_g, col_lo_g, col_hi_g), d)
        {
            out.stamp_skipped = true;
            return out;
        }

        let col_lo = col_lo_g;
        let col_hi = col_hi_g.min(band.cols - 1);
        let row_lo = row_lo_g.max(band.row_offset);
        let row_hi = row_hi_g.min(band.last_row());
        if row_lo > row_hi || col_lo > col_hi {
            return out;
        }
        out.bbox_cells = ((row_hi + 1 - row_lo) * (col_hi + 1 - col_lo)) as u64;
        out.stamp_skipped = false;

        for row in row_lo..=row_hi {
            let cell_v = band.origin_v + row as f64 * cs;
            let dv = cell_v - sv;
            for col in col_lo..=col_hi {
                let cell_u = band.origin_u + col as f64 * cs;
                let du = cell_u - su;
                let coverage = point_cell_coverage(du, dv, r_sq, cs);
                if coverage <= 0.0 {
                    continue;
                }
                let idx = band.local(row, col);
                if air_skip && !cell_can_remove(band.conservative_top[idx], d) {
                    out.cells_skipped += 1;
                    continue;
                }
                let dist_sq = du * du + dv * dv;
                let Some(h) = lut_h_with_edge_fallback(lut, dist_sq) else {
                    continue;
                };
                let ray = &mut band.rays[idx];
                if from_high {
                    let surface = (d + h) as f32;
                    let above = ray_material_length_above(ray, surface) as f64;
                    ray_blend_above(ray, surface, coverage);
                    // §6.F gap 1: scale per-cell removed volume by coverage so
                    // the annular-cell rebalance under fractional stamping is
                    // accounted for. The non-degenerate branch is self-
                    // correcting (pre/post diff); the degenerate branch
                    // accumulates `above` directly and would otherwise
                    // overcount by 1/f for boundary cells.
                    out.removed_volume += coverage as f64 * above * cell_area;
                } else {
                    let surface = (d - h) as f32;
                    let total_before = ray_material_length(ray) as f64;
                    ray_blend_below(ray, surface, coverage);
                    let total_after = ray_material_length(ray) as f64;
                    out.removed_volume += (total_before - total_after) * cell_area;
                }
                // A/M10 — see `stamp_point_on_grid`.
                if from_high
                    && coverage >= FULL_COVERAGE
                    && let Some(ub) = cell_upper_bound_surface(lut, dist_sq, cs, d)
                {
                    band.lower_conservative_top(idx, ub as f32);
                }
            }
        }

        return out;
    }

    // Bounding box of segment sweep + tool radius (superset of all footprints).
    let u_min = su.min(eu) - scan_radius;
    let u_max = su.max(eu) + scan_radius;
    let v_min = sv.min(ev) - scan_radius;
    let v_max = sv.max(ev) + scan_radius;

    // S2 — see the function docs. `tip_lo` is the lowest tip position any cell
    // in this loop can see, computed in the kernel's own arithmetic.
    let tip_lo = segment_tip_low(sd, seg_dd);

    // The mip is queried over the stamp's GLOBAL bounding box, deliberately,
    // even though this band will only walk its own slice of it.
    //
    // A band-local whole-stamp skip is **not exact**, and the exactness sentry
    // caught it the first time this kernel was banded. `pre_volume` and
    // `post_volume` are separate running sums differenced at the end; a band
    // that skips its share drops the same addend from both, and
    // `(a + X) − (b + X) != a − b`. The whole-stamp skip is only exact when
    // EVERY band takes it, because then both sums stay at `0.0` and their
    // difference is exactly zero. Asking about the global box makes every band
    // reach the same verdict, so it is all of them or none.
    let col_lo_g = ((u_min - band.origin_u) / cs).floor().max(0.0) as usize;
    let col_hi_g = ((u_max - band.origin_u) / cs).ceil().max(0.0) as usize;
    let row_lo_g = ((v_min - band.origin_v) / cs).floor().max(0.0) as usize;
    let row_hi_g = ((v_max - band.origin_v) / cs).ceil().max(0.0) as usize;
    if let Some(m) = mip
        && air_skip
        && !cell_can_remove(m.max_over(row_lo_g, row_hi_g, col_lo_g, col_hi_g), tip_lo)
    {
        out.stamp_skipped = true;
        return out;
    }

    let col_lo = col_lo_g;
    let col_hi = col_hi_g.min(band.cols - 1);
    let row_lo = row_lo_g.max(band.row_offset);
    let row_hi = row_hi_g.min(band.last_row());
    if row_lo > row_hi || col_lo > col_hi {
        return out;
    }
    out.bbox_cells = ((row_hi + 1 - row_lo) * (col_hi + 1 - col_lo)) as u64;
    out.stamp_skipped = false;

    // Per-stamp setup, deliberately placed AFTER every early return.
    // `CoverageFastPath::new` costs a handful of `sqrt`s and up to four bounded
    // ULP walks; under S3 this function runs once per BAND rather than once per
    // stamp, and hoisting it above the band-range test cost ~20 % on the
    // fine-cell lateral arm and ~40 % end-to-end before it was moved.
    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let radius_sq = lut.radius_sq();
    // S7: the coverage fast-path radii are constant across the whole stamp.
    let fast = CoverageFastPath::new(radius_sq, cs);

    // Metrics accumulators.
    let mut pre_volume = 0.0f64;
    let mut post_volume = 0.0f64;
    let mut max_penetration = 0.0f64;
    // Perpendicular-to-bearing extent of cells that contained fresh material
    // above the cutter surface before the stamp. Used to derive the
    // conventional "radial width of cut / diameter" engagement metric in a
    // way that is independent of sample density.
    let mut perp_min = f64::INFINITY;
    let mut perp_max = f64::NEG_INFINITY;
    let seg_len = seg_len_sq.sqrt();
    let inv_seg_len = if seg_len > 1e-9 { 1.0 / seg_len } else { 0.0 };
    // Both measurement floors this loop applies — the fixed material floor
    // `FRESH_MATERIAL_THRESHOLD_MM` and the lateral-resolution gate
    // `PERP_COVERAGE_GATE` — are module-level constants; see their docs.

    for row in row_lo..=row_hi {
        let cell_v = band.origin_v + row as f64 * cs;
        for col in col_lo..=col_hi {
            let cell_u = band.origin_u + col as f64 * cs;

            let (coverage, t_center, center_d_sq) = segment_cell_coverage(
                cell_u,
                cell_v,
                su,
                sv,
                seg_du,
                seg_dv,
                inv_seg_len_sq,
                radius_sq,
                cs,
                fast,
            );
            if coverage <= 0.0 {
                continue;
            }
            let idx = band.local(row, col);
            // S2 per-cell early-out. The two volume accumulators still take
            // their (identical) addends — see the function docs for why
            // dropping them would move `removed_volume_est_mm3`.
            if air_skip && !cell_can_remove(band.conservative_top[idx], tip_lo) {
                let inert = ray_material_length(&band.rays[idx]) as f64 * cell_area;
                pre_volume += inert;
                post_volume += inert;
                out.cells_skipped += 1;
                continue;
            }
            let Some(h) = lut_h_with_edge_fallback(lut, center_d_sq) else {
                continue;
            };

            let ray = &mut band.rays[idx];

            // 1. Pre-stamp material totals (read before mutation). pre_len
            //    is total material height; pre_fresh is material above the
            //    cutter surface at this cell — i.e. material this stamp
            //    will (or already would have) removed. pre_fresh drives
            //    the radial engagement metric: independent of sample density
            //    because dense overlapping samples in already-cleared
            //    territory see pre_fresh ≈ 0.
            let pre_len = ray_material_length(ray) as f64;
            pre_volume += pre_len * cell_area;
            let depth = sd + t_center * seg_dd;
            let cell_tool_surface = if from_high { depth + h } else { depth - h };
            let above = ray_material_length_above(ray, cell_tool_surface as f32) as f64;
            let pre_fresh = if from_high { above } else { pre_len - above };

            // 2. Apply the stamp under coverage-weighted blend. f=1 (fully
            //    covered) is identical to the prior subtract-above call;
            //    f<1 leaves (1-f) of the above-surface slice intact.
            if from_high {
                ray_blend_above(ray, cell_tool_surface as f32, coverage);
            } else {
                ray_blend_below(ray, cell_tool_surface as f32, coverage);
            }
            // 3. Post-stamp material height. The pre/post diff naturally
            //    scales with coverage — no separate volume correction needed
            //    (unlike the degenerate branch, §6.F gap 1).
            let post_len = ray_material_length(ray) as f64;
            post_volume += post_len * cell_area;

            // A/M10 — see `stamp_segment_on_grid`. This is the kernel the
            // simulator actually runs, so it is the one that decides whether
            // `prior_stocks` carries a sliver-safe bound at all. Placed
            // after the last read of `ray`: the update takes `&mut band`,
            // and the ray borrow is still live above it.
            if from_high
                && coverage >= FULL_COVERAGE
                && let Some(ub) = cell_upper_bound_surface(lut, center_d_sq, cs, sd.max(ed))
            {
                band.lower_conservative_top(idx, ub as f32);
            }

            // 4. Engagement metrics. The midpoint disk defines the
            //    reference footprint for both arc binning and the
            //    width-of-cut measurement. radial_engagement and
            //    arc_engagement_radians both gate on pre-stamp fresh
            //    material above the cutter surface (independent of
            //    sample density). max_penetration (axial DOC) still
            //    gates on actual removal — it's the per-cell removed
            //    height, which is well-defined per stamp regardless of
            //    overlap with prior stamps.
            let dm_u = cell_u - mid_u;
            let dm_v = cell_v - mid_v;
            let mid_dist_sq = dm_u * dm_u + dm_v * dm_v;
            if mid_dist_sq <= radius_sq && lut.height_at_dist_sq(mid_dist_sq).is_some() {
                if pre_fresh > FRESH_MATERIAL_THRESHOLD_MM && coverage >= PERP_COVERAGE_GATE {
                    let perp = (-seg_dv * dm_u + seg_du * dm_v) * inv_seg_len;
                    if perp < perp_min {
                        perp_min = perp;
                    }
                    if perp > perp_max {
                        perp_max = perp;
                    }
                }
                let removed_here = (pre_len - post_len).max(0.0);
                if removed_here > 1e-6 {
                    max_penetration = max_penetration.max(removed_here);
                }
            }
        }
    }

    out.pre_volume = pre_volume;
    out.post_volume = post_volume;
    out.max_penetration = max_penetration;
    out.perp_min = perp_min;
    out.perp_max = perp_max;
    out
}
/// Bundled parameters for `sample_segment_runtime`.
pub(super) struct SegmentSampleParams<'a> {
    pub(super) move_index: usize,
    pub(super) toolpath_id: ToolpathId,
    pub(super) sample_step_mm: f64,
    pub(super) feed_rate_mm_min: f64,
    pub(super) is_cutting: bool,
    pub(super) cut_kinematics: CutKinematics,
    pub(super) spindle_rpm: u32,
    pub(super) flute_count: u32,
    pub(super) semantic_item_id: Option<u64>,
    pub(super) span_path: &'a [SpanId],
    /// P3: move sits in a transit-style span. See `CuttingCaptureParams`.
    pub(super) in_transit_span: bool,
    /// R-11: the generator's own `MoveIntent` for this move, carried onto
    /// every emitted sample. See `SimulationCutSample::source_intent`.
    pub(super) source_intent: Option<crate::toolpath::MoveIntent>,
}

pub(super) fn sample_segment_runtime(
    start: P3,
    end: P3,
    params: &SegmentSampleParams<'_>,
    cumulative_time_s: &mut f64,
    next_sample_index: &mut usize,
    samples: &mut Vec<SimulationCutSample>,
) {
    let segment_length = (end - start).norm();
    if segment_length <= 1e-9 {
        return;
    }

    let subsegments = ((segment_length / params.sample_step_mm.max(1e-3)).ceil() as usize).max(1);
    for subsegment in 0..subsegments {
        let t0 = subsegment as f64 / subsegments as f64;
        let t1 = (subsegment + 1) as f64 / subsegments as f64;
        let seg_start = lerp_point(start, end, t0);
        let seg_end = lerp_point(start, end, t1);
        let midpoint = lerp_point(seg_start, seg_end, 0.5);
        let segment_len = (seg_end - seg_start).norm();
        if segment_len <= 1e-9 {
            continue;
        }
        let segment_time_s = (segment_len / params.feed_rate_mm_min.max(1.0)) * 60.0;
        *cumulative_time_s += segment_time_s;
        samples.push(SimulationCutSample {
            toolpath_id: params.toolpath_id,
            move_index: params.move_index,
            sample_index: *next_sample_index,
            position: [midpoint.x, midpoint.y, midpoint.z],
            cumulative_time_s: *cumulative_time_s,
            segment_time_s,
            is_cutting: params.is_cutting,
            cut_kinematics: params.cut_kinematics,
            feed_rate_mm_min: params.feed_rate_mm_min,
            spindle_rpm: params.spindle_rpm,
            flute_count: params.flute_count,
            axial_doc_mm: 0.0,
            axial_engagement_mm: 0.0,
            plunge_descent_mm: 0.0,
            arc_engagement_radians: None,
            chipload_mm_per_tooth: 0.0,
            effective_chip_thickness_mm: None,
            engagement: crate::simulation_cut::Engagement::with_radial_woc(0.0),
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: params.semantic_item_id,
            span_path: params.span_path.to_vec(),
            in_transit_span: params.in_transit_span,
            source_intent: params.source_intent,
        });
        *next_sample_index += 1;
    }
}

pub(super) fn chipload_mm_per_tooth(
    feed_rate_mm_min: f64,
    spindle_rpm: u32,
    flute_count: u32,
) -> f64 {
    if spindle_rpm == 0 || flute_count == 0 {
        0.0
    } else {
        feed_rate_mm_min / spindle_rpm as f64 / flute_count as f64
    }
}

pub(super) fn lerp_point(start: P3, end: P3, t: f64) -> P3 {
    start + (end - start) * t
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn build_move_semantic_lookup(
    move_count: usize,
    trace: Option<&ToolpathSemanticTrace>,
) -> Vec<Option<u64>> {
    let Some(trace) = trace else {
        return vec![None; move_count];
    };

    let mut item_index_by_id = std::collections::HashMap::with_capacity(trace.items.len());
    for (item_index, item) in trace.items.iter().enumerate() {
        item_index_by_id.insert(item.id, item_index);
    }

    let mut depths = vec![0usize; trace.items.len()];
    for (item_index, item) in trace.items.iter().enumerate() {
        let mut depth = 0usize;
        let mut parent = item.parent_id;
        while let Some(parent_id) = parent {
            depth += 1;
            parent = item_index_by_id
                .get(&parent_id)
                .and_then(|parent_index| trace.items.get(*parent_index))
                .and_then(|parent_item| parent_item.parent_id);
        }
        depths[item_index] = depth;
    }

    let mut lookup = vec![None; move_count];
    let mut best_depth = vec![0usize; move_count];
    let mut best_span = vec![usize::MAX; move_count];

    for (item_index, item) in trace.items.iter().enumerate() {
        let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
            continue;
        };
        if move_count == 0 || move_start >= move_count {
            continue;
        }
        let last = move_end.min(move_count.saturating_sub(1));
        let span = last.saturating_sub(move_start);
        for move_index in move_start..=last {
            let replace = lookup[move_index].is_none()
                || depths[item_index] > best_depth[move_index]
                || (depths[item_index] == best_depth[move_index] && span < best_span[move_index]);
            if replace {
                lookup[move_index] = Some(item.id);
                best_depth[move_index] = depths[item_index];
                best_span[move_index] = span;
            }
        }
    }

    lookup
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::super::tile_mip::SkipStats;
    use super::*;
    use crate::geo::BoundingBox3;
    use crate::tool::{BallEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill};

    // ── S2: the air-skip must change nothing at all ─────────────────────

    type StampOut = (f64, f64, Option<f64>, f64);

    /// Replay a scripted stamp sequence, with the S2 mip either supplied or
    /// withheld, and hand back everything the kernel could possibly have
    /// touched: the rays, the sliver-safe channel, and every returned tuple.
    fn replay(
        cutter: &dyn MillingCutter,
        cell_size: f64,
        from_high: bool,
        air_skip: bool,
    ) -> (DexelGrid, Vec<StampOut>, Option<SkipStats>) {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(30.0, 20.0, 8.0),
        };
        let mut grid = DexelGrid::z_grid_from_bounds(&bbox, cell_size);
        let lut = RadialProfileLUT::from_cutter(cutter, crate::radial_profile::LUT_SAMPLES);
        let radius = cutter.radius();
        let mut mip = if air_skip {
            Some(TileMaxTop::build(&grid))
        } else {
            None
        };
        let mut out = Vec::new();
        let stamp = |grid: &mut DexelGrid,
                     out: &mut Vec<StampOut>,
                     mip: &mut Option<TileMaxTop>,
                     s: (f64, f64, f64),
                     e: (f64, f64, f64)| {
            if let Some(m) = mip.as_mut() {
                m.refresh_if_due(grid);
            }
            let mut reduced = StampPartial::empty();
            let view = mip.as_ref();
            let (row_lo, row_hi) = crate::dexel_stock::band::stamp_row_span(grid, radius, s, e);
            for mut band in grid.serial_bands(row_lo, row_hi) {
                reduced.merge(&stamp_segment_with_metrics(
                    &mut band,
                    &lut,
                    radius,
                    s,
                    e,
                    (s.0 + e.0) * 0.5,
                    (s.1 + e.1) * 0.5,
                    from_high,
                    view,
                ));
            }
            if let Some(m) = mip.as_mut() {
                m.absorb(&reduced);
            }
            out.push(reduced.finish(radius, true));
        };

        // Three depth ladders. The third REPEATS the second, which is the
        // case the non-strict comparisons in `cell_can_remove` exist for:
        // re-passing ground already cut to exactly this tip height, where
        // `conservative_top == tip` holds bit-for-bit on a flat end mill.
        for depth in [6.0_f64, 4.0, 4.0] {
            for line in 0..7 {
                let y = 3.0 + 2.0 * line as f64;
                // Overlapping sub-steps along the line: consecutive stamps
                // share most of their footprint, which is where the per-cell
                // early-out lives.
                for step in 0..12 {
                    let x0 = 3.0 + 2.0 * step as f64;
                    stamp(
                        &mut grid,
                        &mut out,
                        &mut mip,
                        (x0, y, depth),
                        (x0 + 2.0, y, depth),
                    );
                }
            }
            // A ramp — the tip varies along the segment, so `segment_tip_low`
            // rather than the endpoint decides.
            stamp(
                &mut grid,
                &mut out,
                &mut mip,
                (5.0, 15.0, depth + 1.5),
                (20.0, 15.0, depth),
            );
            // A pure plunge — the degenerate branch, whose accumulator is a
            // single running sum rather than a pre/post pair.
            stamp(
                &mut grid,
                &mut out,
                &mut mip,
                (25.0, 10.0, depth + 1.5),
                (25.0, 10.0, depth),
            );
            // GENUINE AIR: a pass at and above the untouched stock top, over
            // ground the ladders have already cut well below it. This is the
            // regime the whole-stamp early-out exists for, and without it the
            // sweep only exercises the per-cell one.
            for line in 0..4 {
                let y = 4.0 + 4.0 * line as f64;
                stamp(&mut grid, &mut out, &mut mip, (4.0, y, 8.5), (26.0, y, 8.5));
            }
            // …and a plunge through air, for the degenerate branch's own
            // whole-stamp return.
            stamp(
                &mut grid,
                &mut out,
                &mut mip,
                (15.0, 9.0, 9.5),
                (15.0, 9.0, 8.5),
            );
        }
        let stats = mip.map(|m| m.stats());
        (grid, out, stats)
    }

    fn assert_grids_bit_identical(a: &DexelGrid, b: &DexelGrid, what: &str) {
        assert_eq!(a.rays.len(), b.rays.len(), "{what}: ray count");
        for (i, (ra, rb)) in a.rays.iter().zip(b.rays.iter()).enumerate() {
            assert_eq!(ra.len(), rb.len(), "{what}: cell {i} segment count");
            for (sa, sb) in ra.iter().zip(rb.iter()) {
                assert_eq!(
                    (sa.enter.to_bits(), sa.exit.to_bits()),
                    (sb.enter.to_bits(), sb.exit.to_bits()),
                    "{what}: cell {i} segment bits"
                );
            }
        }
        for (i, (ca, cb)) in a
            .conservative_top
            .iter()
            .zip(b.conservative_top.iter())
            .enumerate()
        {
            assert_eq!(
                ca.to_bits(),
                cb.to_bits(),
                "{what}: conservative_top bits at cell {i}"
            );
        }
    }

    /// **The S2 net.** The tile early-out claims to be an *exact* skip, not a
    /// tolerated approximation, so this compares the two runs at bit level on
    /// every channel the kernel writes — the rays, `conservative_top`, and all
    /// four returned metrics — with no tolerance anywhere.
    ///
    /// Two things this is deliberately built to catch, both of which the
    /// review's one-line prescription ("`tile_max_top <= depth_min` ⇒ skip the
    /// tile, exactly") would have got wrong:
    ///
    /// 1. **The volume accumulators.** `pre_volume` and `post_volume` are
    ///    summed *separately* over covered cells and differenced at the end.
    ///    An inert cell contributes the same addend to both, so dropping it
    ///    outright reassociates the sum and moves `removed_volume_est_mm3` in
    ///    its last bits. The per-cell early-out therefore keeps the pair.
    /// 2. **The tip floor.** `min(sd, ed)` is NOT the lowest tip the kernel
    ///    evaluates, because `fl(sd + fl(ed − sd))` need not be `ed`. See
    ///    [`segment_tip_low`].
    ///
    /// The `stats` assertions are the anti-vacuity half: a skip that never
    /// fires is exactly identical and worth nothing.
    #[test]
    fn the_air_skip_is_bit_exact_and_not_vacuous() {
        let cutters: Vec<(&str, Box<dyn MillingCutter>)> = vec![
            ("flat6", Box::new(FlatEndmill::new(6.0, 25.0))),
            ("ball6", Box::new(BallEndmill::new(6.0, 25.0))),
            ("vbit60", Box::new(VBitEndmill::new(6.0, 60.0, 20.0))),
            (
                "tapered_ball1",
                Box::new(TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)),
            ),
        ];
        let mut total_skipped_stamps = 0u64;
        let mut flat_skipped_cells = 0u64;
        let mut flat_bbox_cells = 0u64;

        for (name, cutter) in &cutters {
            for &cell_size in &[0.2_f64, 0.5] {
                for &from_high in &[true, false] {
                    let (g_off, out_off, _) = replay(cutter.as_ref(), cell_size, from_high, false);
                    let (g_on, out_on, stats) = replay(cutter.as_ref(), cell_size, from_high, true);
                    let what = format!("{name} cs={cell_size} from_high={from_high}");
                    assert_grids_bit_identical(&g_off, &g_on, &what);
                    assert_eq!(out_off.len(), out_on.len(), "{what}: stamp count");
                    for (i, (a, b)) in out_off.iter().zip(out_on.iter()).enumerate() {
                        assert_eq!(a.0.to_bits(), b.0.to_bits(), "{what}: stamp {i} axial");
                        assert_eq!(a.1.to_bits(), b.1.to_bits(), "{what}: stamp {i} radial");
                        assert_eq!(
                            a.2.map(f64::to_bits),
                            b.2.map(f64::to_bits),
                            "{what}: stamp {i} arc"
                        );
                        assert_eq!(a.3.to_bits(), b.3.to_bits(), "{what}: stamp {i} volume");
                    }
                    if let Some(stats) = stats
                        && from_high
                    {
                        total_skipped_stamps += stats.stamps_skipped;
                        if *name == "flat6" {
                            flat_skipped_cells += stats.cells_skipped;
                            flat_bbox_cells += stats.cells_in_bbox;
                        }
                    }
                }
            }
        }

        assert!(
            total_skipped_stamps > 0,
            "no whole-stamp early-out fired anywhere in the sweep — the test \
             proves nothing"
        );
        // The per-cell rate is asserted on the FLAT tool only, and that
        // restriction is a finding, not a convenience. `conservative_top` is
        // lowered to `tip + h(near_dist + cs·√2/2)`, so on a round-profile
        // cutter a re-pass over its own ground still reads a bound strictly
        // ABOVE the tip — correctly, because the rim cells of the old stamp
        // really do still hold material the new stamp will take. Overlap
        // therefore only becomes skippable when `h ≡ 0` across the cell; for
        // ball, v-bit and tapered tools the early-out fires on genuine air
        // and not on overlap. Measured across this sweep: ~15 % of in-bbox
        // cells overall, ~40 % on the flat arm alone.
        assert!(
            flat_skipped_cells * 4 > flat_bbox_cells,
            "the per-cell early-out took only {flat_skipped_cells} of \
             {flat_bbox_cells} in-bbox cells on the flat arm; the fixture has \
             stopped exercising the overlap regime S2 exists for"
        );
    }

    /// `segment_tip_low` must return the exact minimum of the tip expression
    /// the kernel evaluates, not the algebraically-equal `min(sd, ed)`.
    /// Swept over pairs whose `fl(sd + fl(ed − sd))` differs from `ed`.
    #[test]
    fn segment_tip_low_is_the_true_floor_of_the_kernels_own_expression() {
        let mut disagreements = 0usize;
        let mut probes = 0usize;
        // The pairs that matter are the ones where `fl(sd + fl(ed − sd))`
        // is not `ed`. `(1.0, 1e-20)` is the clean one: `1e-20 − 1.0` rounds
        // to exactly `-1.0`, so the kernel evaluates a tip of `0.0` at
        // `t = 1` — BELOW `min(sd, ed)`. The naive floor is unsound there.
        for &sd in &[0.0_f64, 1.0, -3.25, 1e6, -1e-7, 12.700000000000001] {
            for &ed in &[
                0.0_f64,
                1.0,
                -3.25,
                1e6,
                -1e-7,
                12.700000000000001,
                1e-16,
                1e-20,
                1e-25,
            ] {
                for k in -3i32..=3 {
                    let mut ed = ed;
                    for _ in 0..k.abs() {
                        ed = if k > 0 { ed.next_up() } else { ed.next_down() };
                    }
                    let seg_dd = ed - sd;
                    let floor = segment_tip_low(sd, seg_dd);
                    // The kernel's own expression, at both attainable ends and
                    // a spread of interior parameters.
                    for i in 0..=64 {
                        let t = i as f64 / 64.0;
                        let depth = sd + t * seg_dd;
                        probes += 1;
                        assert!(
                            depth >= floor,
                            "sd={sd} ed={ed} t={t}: depth {depth} < floor {floor}"
                        );
                    }
                    if floor != sd.min(ed) {
                        disagreements += 1;
                    }
                }
            }
        }
        assert!(probes > 10_000, "sweep thinned to {probes} probes");
        assert!(
            disagreements > 0,
            "no (sd, ed) pair made `segment_tip_low` differ from `min(sd, ed)` \
             — the sweep is not reaching the rounding case the function exists \
             for, so it would pass with the naive form"
        );
    }

    /// The pre-S7 fast-path test, copied verbatim from the sqrt form it
    /// replaced. Kept as an oracle rather than a pinned constant so it cannot
    /// rot: if someone changes `CoverageFastPath`, this still says what the
    /// old code said.
    fn legacy_fast_path(center_d_sq: f64, r_sq: f64, cs: f64) -> Option<f32> {
        let ext_diag = cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;
        let r = r_sq.sqrt();
        let center_d = center_d_sq.sqrt();
        if center_d - ext_diag >= r {
            return Some(0.0);
        }
        if center_d + ext_diag <= r {
            return Some(1.0);
        }
        None
    }

    fn s7_fast_path(center_d_sq: f64, r_sq: f64, cs: f64) -> Option<f32> {
        let fast = CoverageFastPath::new(r_sq, cs);
        if center_d_sq >= fast.outer_sq {
            return Some(0.0);
        }
        if center_d_sq <= fast.inner_sq {
            return Some(1.0);
        }
        None
    }

    /// S7 claims the de-sqrt is exact. This is where that claim is checked, at
    /// **exact equality of `Option<f32>`** — there is deliberately no tolerance
    /// here to widen, because the whole point of the change is that it moves
    /// nothing.
    ///
    /// Two things this covers that the review's text does not mention:
    ///
    /// 1. The regime where the sub-sample fan is WIDER than the cutter
    ///    (`ext_diag > r`) and the "fully inside" bound goes negative. Naively
    ///    squaring `r − ext_diag` there flips the test's sense and reports full
    ///    coverage for cells near the tool axis. `sentinel_rows` asserts that
    ///    regime is actually exercised rather than silently absent.
    /// 2. The **boundary ties**, which is where the review's "exact in squared
    ///    space" claim actually fails. `(r ± ext_diag)²` is NOT the flip point
    ///    of the sqrt form in `f64`; this sweep lands on both boundaries and
    ///    walks four ULPs either side of each, and it found real disagreements
    ///    against the naive squared bound. That is why `CoverageFastPath`
    ///    solves for the threshold instead of computing it algebraically.
    #[test]
    fn squared_fast_paths_agree_with_the_sqrt_form() {
        // Radii from a Ø0.2 engraver to a Ø25.4 shell mill; cells from a fine
        // 0.02 mm sim grid up to 5 mm. Several pairs put `ext_diag` above `r`,
        // which is the sentinel case.
        let radii = [
            0.1_f64, 0.25, 0.4, 0.5, 0.75, 1.0, 1.5, 3.0, 6.0, 10.0, 12.7,
        ];
        let cells = [0.02_f64, 0.05, 0.1, 0.2, 0.25, 0.4, 0.5, 1.0, 2.0, 3.0, 5.0];
        const SWEEP_STEPS: usize = 600;
        const ULP_NEIGHBOURHOOD: i32 = 4;

        let mut disagreements = Vec::new();
        let mut sentinel_rows = 0usize;
        let mut probes_checked = 0usize;

        for &r in &radii {
            let r_sq = r * r;
            for &cs in &cells {
                let ext_diag = cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;
                if ext_diag > r {
                    sentinel_rows += 1;
                }
                // Sweep the whole decision band …
                let mut probes: Vec<f64> = (0..=SWEEP_STEPS)
                    .map(|i| (r + 2.0 * ext_diag) * i as f64 / SWEEP_STEPS as f64)
                    .collect();
                // … then land ON each interesting value and walk ULPs either
                // side of it. The disagreements this test exists to catch live
                // within one ULP of a boundary, so sweeping alone misses them.
                for base in [r + ext_diag, (r - ext_diag).max(0.0), r, ext_diag, 0.0] {
                    for k in -ULP_NEIGHBOURHOOD..=ULP_NEIGHBOURHOOD {
                        let mut p = base;
                        for _ in 0..k.abs() {
                            p = if k > 0 { p.next_up() } else { p.next_down() };
                        }
                        probes.push(p.max(0.0));
                    }
                }
                for d in probes {
                    let d_sq = d * d;
                    probes_checked += 1;
                    let legacy = legacy_fast_path(d_sq, r_sq, cs);
                    let s7 = s7_fast_path(d_sq, r_sq, cs);
                    if legacy != s7 {
                        disagreements.push(format!(
                            "r={r} cs={cs} d={d:.17e}: legacy {legacy:?}, S7 {s7:?}"
                        ));
                    }
                }
            }
        }

        assert!(
            sentinel_rows > 0,
            "no (radius, cell) pair exercised the ext_diag > r sentinel — the \
             test is not covering the case it exists for"
        );
        assert!(
            probes_checked > 70_000,
            "only {probes_checked} probes ran — the sweep has been thinned, and \
             a thinned sweep is how this test goes green without being true"
        );
        assert!(
            disagreements.is_empty(),
            "the squared-space fast paths disagree with the sqrt form in {} \
             place(s) out of {probes_checked}:\n{}",
            disagreements.len(),
            disagreements
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
