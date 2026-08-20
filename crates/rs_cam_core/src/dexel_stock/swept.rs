//! Swept-volume stamping — `PERF_REVIEW.md` **S1**.
//!
//! The shipped kernel stamps **one subsegment at a time**. A cutting move is
//! cut into `max(⌈len/sample_step⌉, ⌈|Δz|/0.02⌉)` subsegments, and each of them
//! walks its own tool-radius-inflated bounding box. Consecutive subsegments'
//! boxes overlap by `2R − s` out of `2R + s`, so a Ø6 cutter at `s = 0.25 mm`
//! visits each cell ~25 times to remove material once.
//!
//! This module stamps a **run of consecutive subsegments in one pass**: one
//! stadium over the chunk, one visit per cell. `segment_cell_coverage` already
//! returns the cell's closest-approach parameter `t_center`, so the cell's
//! metric contributions are binned into `floor(t_center · bins)` — the
//! subsegment the cell is nearest to — and every accumulator in
//! [`StampPartial`] is associative, so the bins reduce exactly as the per-stamp
//! partials do.
//!
//! # What this changes, stated up front
//!
//! Swept stamping is **not** metric-neutral, and it is not advertised as such.
//! Three things move, and they move for three different reasons:
//!
//! 1. **`pre_fresh` is measured once per cell instead of once per subsegment.**
//!    The per-stamp kernel's density-independence comes from a cell reading
//!    `pre_fresh ≈ 0` on every subsegment after the first one that reaches it,
//!    so only the leading crescent contributes to `perp_min`/`perp_max`. A
//!    swept pass sees each cell exactly once, in its closest-approach bin, and
//!    that cell is fresh. The *quantity* the radial-engagement metric measures
//!    is therefore the perpendicular extent of fresh material in a
//!    **longitudinal slice**, not in the **leading crescent**. On a slot and on
//!    a steady side-bite the two agree closely; they are not the same
//!    definition.
//! 2. **Attribution shifts along the path.** A cell's material is removed by
//!    the tool's *leading edge*, roughly `R/s` subsegments before the tool
//!    centre passes it; binning credits it to the closest-approach subsegment.
//!    Per-sample `removed_volume_est_mm3` and `axial_doc_mm` therefore lag by
//!    up to `R` of travel relative to the per-stamp stream. Sums over a
//!    toolpath are unaffected except by (3).
//! 3. **The tip depth is evaluated at the cell's own `t_center` instead of at
//!    the subsegment's.** On a descending move this is *more* accurate, not
//!    less — it is the limit the `MAX_SUBSEGMENT_Z_DROP_MM = 0.02` subdivision
//!    was approximating — but it is a different number.
//!
//! The pure-vertical arm below is a separate story: it is **bit-identical** to
//! the per-stamp path. See [`stamp_plunge_chunk`].

use rayon::prelude::*;

use super::band::{self, BAND_ROWS, GridBand};
use super::stamping::{
    CoverageFastPath, FRESH_MATERIAL_THRESHOLD_MM, FULL_COVERAGE, PERP_COVERAGE_GATE,
    SUBSAMPLE_HALF_EXTENT, StampPartial, cell_can_remove, cell_upper_bound_height,
    cell_upper_bound_surface, clamped_cell_bbox, lut_h_with_edge_fallback, point_cell_coverage,
    segment_cell_coverage, segment_tip_low, stamp_segment_with_metrics,
};
use super::tile_mip::TileMaxTop;
use crate::dexel::{
    DexelGrid, ray_blend_above, ray_blend_below, ray_material_length, ray_material_length_above,
};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;

use super::whole_path::StampDispatchStats;

/// Componentwise `start + (end − start) · t` on a decomposed
/// `(u, v, depth)` triple.
///
/// **This must stay bit-for-bit the driver's own `lerp_point`.**
/// `StockCutDirection::decompose` is a pure coordinate *permutation* — no
/// negation, no arithmetic — so `decompose(lerp_point(S, E, t))` and
/// `lerp3(decompose(S), decompose(E), t)` are the same three `f64`
/// expressions evaluated in the same order. That equality is what lets a job
/// carry the *move's* endpoints and reconstruct any subsegment's endpoints
/// exactly, instead of carrying 250 pre-lerped copies of them.
#[inline]
fn lerp3(s: (f64, f64, f64), e: (f64, f64, f64), t: f64) -> (f64, f64, f64) {
    (
        s.0 + (e.0 - s.0) * t,
        s.1 + (e.1 - s.1) * t,
        s.2 + (e.2 - s.2) * t,
    )
}

/// How a chunk's bins are stamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChunkKind {
    /// Lateral / ramping: one stadium pass over the whole chunk, metrics binned
    /// by `t_center`. The metric-changing arm.
    Swept,
    /// Exactly-vertical descent (`Δu == 0.0 && Δv == 0.0` in the decomposed
    /// frame): one XY pass, the per-cell invariants hoisted out of the bin
    /// loop. **Bit-identical** to per-stamp.
    PureVertical,
    /// Degenerate by the kernel's `1e-20` planar test but *not* exactly
    /// vertical — a near-vertical move whose subsegments each have a nonzero
    /// but sub-threshold planar step. The hoist is unsound there (each bin has
    /// its own `(su, sv)` and therefore its own coverage), so these bins are
    /// replayed one at a time through the shipped kernel. Correct, not faster;
    /// it exists so the classification can never silently mis-hoist.
    PerBin,
}

/// One chunk of consecutive subsegments of one move, deferred until its batch
/// runs.
///
/// `m_start`/`m_end` are the **move's** endpoints in the grid's decomposed
/// frame; `bin0..bin0+bins` index into the move's `subsegments` subdivision.
/// Sample slots are `first_slot..first_slot + bins`, which the enumerator
/// guarantees are consecutive because it pushes one sample per subsegment and
/// a chunk is closed whenever a subsegment is skipped.
#[derive(Clone, Copy, Debug)]
pub(super) struct SweptJob {
    pub(super) m_start: (f64, f64, f64),
    pub(super) m_end: (f64, f64, f64),
    pub(super) subsegments: usize,
    pub(super) bin0: usize,
    pub(super) bins: usize,
    pub(super) first_slot: usize,
    pub(super) kind: ChunkKind,
}

impl SweptJob {
    /// Endpoints of global subsegment `bin0 + b`, reconstructed exactly.
    #[inline]
    fn bin_segment(&self, b: usize) -> ((f64, f64, f64), (f64, f64, f64)) {
        let n = self.subsegments as f64;
        let j = (self.bin0 + b) as f64;
        (
            lerp3(self.m_start, self.m_end, j / n),
            lerp3(self.m_start, self.m_end, (j + 1.0) / n),
        )
    }

    /// The chunk's own endpoints — subsegment `bin0`'s start and subsegment
    /// `bin0 + bins − 1`'s end.
    #[inline]
    fn chunk_segment(&self) -> ((f64, f64, f64), (f64, f64, f64)) {
        let n = self.subsegments as f64;
        (
            lerp3(self.m_start, self.m_end, self.bin0 as f64 / n),
            lerp3(self.m_start, self.m_end, (self.bin0 + self.bins) as f64 / n),
        )
    }
}

// ── Chunk sizing ────────────────────────────────────────────────────────

/// How much of a chunk's bounding box may be outside its own swept stadium
/// before the chunk is closed.
///
/// **This is the whole reason chunks exist rather than whole moves.** A swept
/// pass costs the chunk's *bounding box*, not its stadium, and for a diagonal
/// move those diverge quadratically: a 100 mm move at 45° with a Ø6 cutter has
/// a bbox of ~11 300 mm² around a stadium of ~630 mm². Stamping that move in
/// one pass would be **slower** than the per-subsegment kernel it replaces.
/// Capping the ratio keeps the win where the finding claimed it, and it is why
/// the measured speed-up is a function of path direction and not only of
/// `2R/s`.
///
/// Axis-aligned moves never trip it (their bbox *is* the stadium's bbox), so a
/// raster pass chunks only at [`SWEPT_MAX_BINS_PER_JOB`].
const SWEPT_MAX_BBOX_WASTE: f64 = 2.0;

/// Hard cap on bins per chunk, so one long move cannot dominate a batch's
/// partial budget on its own.
const SWEPT_MAX_BINS_PER_JOB: usize = 4_096;

/// Would a chunk of `bins` subsegments starting at `bin0` still be worth
/// stamping in one pass?
///
/// Uses the same estimates as `band::stamp_bbox_cells` (scan radius
/// `radius + cs`) so the scheduling arithmetic is consistent with the batch
/// budget's.
fn chunk_fits(job: &SweptJob, radius: f64, cs: f64) -> bool {
    if cs <= 0.0 {
        return true;
    }
    let (p0, p1) = job.chunk_segment();
    let du = (p1.0 - p0.0).abs();
    let dv = (p1.1 - p0.1).abs();
    let len = (du * du + dv * dv).sqrt();
    let scan = radius + cs;
    let bbox = (du / cs + 2.0 * scan / cs + 2.0) * (dv / cs + 2.0 * scan / cs + 2.0);
    let stadium = (2.0 * scan * len + std::f64::consts::PI * scan * scan) / (cs * cs);
    bbox <= SWEPT_MAX_BBOX_WASTE * stadium.max(1.0)
}

// ── Kernels ─────────────────────────────────────────────────────────────

/// Stamp one chunk into one band, writing one [`StampPartial`] per bin.
///
/// `out.len()` must be `job.bins`; anything shorter is a programming error and
/// the extra bins are dropped rather than panicking (this runs inside a rayon
/// closure).
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp_chunk(
    band: &mut GridBand<'_>,
    lut: &RadialProfileLUT,
    radius: f64,
    job: &SweptJob,
    from_high: bool,
    mip: Option<&TileMaxTop>,
    out: &mut [StampPartial],
) {
    match job.kind {
        ChunkKind::PerBin => {
            for (b, slot) in out.iter_mut().enumerate() {
                let (s, e) = job.bin_segment(b);
                let mid = lerp3(s, e, 0.5);
                *slot = stamp_segment_with_metrics(
                    band, lut, radius, s, e, mid.0, mid.1, from_high, mip,
                );
            }
        }
        ChunkKind::PureVertical => stamp_plunge_chunk(band, lut, radius, job, from_high, mip, out),
        ChunkKind::Swept => stamp_swept_chunk(band, lut, radius, job, from_high, mip, out),
    }
}

/// The pure-vertical arm — **bit-identical** to stamping each bin separately.
///
/// # Why the by_z arm did not need an approximation
///
/// `PERF_REVIEW.md` S1(b) proposes replacing the `Δz / 0.02 mm` subdivision
/// with an analytic minimisation of `z(t) + h(d(t))`. On a pure descent that
/// is unnecessary, because the redundancy is **loop order, not subdivision**:
/// `(su, sv)` is the same `f64` for every subsegment (`s + (e − s)·t` with
/// `e − s` exactly `0.0`), so `point_cell_coverage`, the cell's `dist_sq`, its
/// LUT probe and the `sqrt` + probe inside `cell_upper_bound_surface` are all
/// **loop invariants** that the shipped kernel recomputes 250 times per cell.
///
/// Inverting the loops — cells outside, bins inside — hoists every one of them
/// and changes nothing else:
///
/// * each cell still sees its blends in ascending bin order, which is what
///   `ray_blend_above`'s non-commutativity at `f < 1` requires;
/// * for a fixed bin, `removed_volume` still accumulates over cells in
///   row-major order, so the sum is not reassociated;
/// * the whole-stamp mip verdict is per bin, precomputed against the same
///   immutable mip and the same global bounding box the per-stamp path queries.
///
/// So the 250 stamps become 250 *blends* over one coverage evaluation, and the
/// answer is the same bit pattern. What the analytic proposal would have bought
/// on top of that — collapsing the 250 blends into one — is **not** available
/// without changing the sample stream: each of those 250 subsegments carries
/// its own `SimulationCutSample` with its own `plunge_descent_mm` and its own
/// share of the removed volume, and a drill-adjacent gate reads them.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
fn stamp_plunge_chunk(
    band: &mut GridBand<'_>,
    lut: &RadialProfileLUT,
    radius: f64,
    job: &SweptJob,
    from_high: bool,
    mip: Option<&TileMaxTop>,
    out: &mut [StampPartial],
) {
    let bins = out.len();
    if bins == 0 {
        return;
    }
    let air_skip = mip.is_some() && from_high && lut.profile_is_nonneg_total();

    // Per-bin tip depth, verbatim from the shipped degenerate branch:
    // `d = sd.min(ed)` on the bin's own reconstructed endpoints.
    let mut depths = Vec::with_capacity(bins);
    for b in 0..bins {
        let (s, e) = job.bin_segment(b);
        depths.push(s.2.min(e.2));
        out[b] = StampPartial::empty();
        out[b].degenerate = true;
        out[b].descent = (s.2 - e.2).abs();
    }

    let (su, sv, _) = job.bin_segment(0).0;
    let cs = band.cell_size;
    let cell_area = cs * cs;
    let scan_radius = radius + cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;

    let col_min = ((su - scan_radius - band.origin_u) / cs).floor() as isize;
    let col_max = ((su + scan_radius - band.origin_u) / cs).ceil() as isize;
    let row_min = ((sv - scan_radius - band.origin_v) / cs).floor() as isize;
    let row_max = ((sv + scan_radius - band.origin_v) / cs).ceil() as isize;
    let Some((col_lo_g, col_hi_g, row_lo_g, row_hi_g)) = clamped_cell_bbox(
        col_min,
        col_max,
        row_min,
        row_max,
        band.cols,
        band.grid_rows,
    ) else {
        return;
    };

    // The whole-stamp mip verdict, per bin. Identical query to the per-stamp
    // path's: the stamp's GLOBAL bounding box against the bin's own tip.
    let mut bin_skipped = vec![false; bins];
    let mut any_runs = false;
    for b in 0..bins {
        let skipped = match mip {
            Some(m) if air_skip => !cell_can_remove(
                m.max_over(row_lo_g, row_hi_g, col_lo_g, col_hi_g),
                depths[b],
            ),
            _ => false,
        };
        bin_skipped[b] = skipped;
        out[b].stamp_skipped = skipped;
        any_runs |= !skipped;
    }
    if !any_runs {
        return;
    }

    let col_lo = col_lo_g;
    let col_hi = col_hi_g.min(band.cols - 1);
    let row_lo = row_lo_g.max(band.row_offset);
    let row_hi = row_hi_g.min(band.last_row());
    if row_lo > row_hi || col_lo > col_hi {
        return;
    }
    // Chunk-level diagnostics, carried on bin 0 — this band walked its slice of
    // the bounding box ONCE, not once per bin, and `TileMaxTop::absorb` charges
    // its refresh budget from exactly this field.
    out[0].bbox_cells = ((row_hi + 1 - row_lo) * (col_hi + 1 - col_lo)) as u64;

    let r_sq = lut.radius_sq();
    for row in row_lo..=row_hi {
        let cell_v = band.origin_v + row as f64 * cs;
        let dv = cell_v - sv;
        for col in col_lo..=col_hi {
            let cell_u = band.origin_u + col as f64 * cs;
            let du = cell_u - su;
            // ── the three hoisted invariants ──
            let coverage = point_cell_coverage(du, dv, r_sq, cs);
            if coverage <= 0.0 {
                continue;
            }
            let dist_sq = du * du + dv * dv;
            let Some(h) = lut_h_with_edge_fallback(lut, dist_sq) else {
                continue;
            };
            let ub_h = if from_high && coverage >= FULL_COVERAGE {
                cell_upper_bound_height(lut, dist_sq, cs)
            } else {
                None
            };
            let idx = band.local(row, col);

            for b in 0..bins {
                if bin_skipped[b] {
                    continue;
                }
                let d = depths[b];
                if air_skip && !cell_can_remove(band.conservative_top[idx], d) {
                    out[0].cells_skipped += 1;
                    continue;
                }
                let ray = &mut band.rays[idx];
                if from_high {
                    let surface = (d + h) as f32;
                    let above = ray_material_length_above(ray, surface) as f64;
                    ray_blend_above(ray, surface, coverage);
                    out[b].removed_volume += coverage as f64 * above * cell_area;
                } else {
                    let surface = (d - h) as f32;
                    let total_before = ray_material_length(ray) as f64;
                    ray_blend_below(ray, surface, coverage);
                    let total_after = ray_material_length(ray) as f64;
                    out[b].removed_volume += (total_before - total_after) * cell_area;
                }
                if let Some(uh) = ub_h {
                    band.lower_conservative_top(idx, (d + uh) as f32);
                }
            }
        }
    }
}

/// The swept arm — one stadium pass over the chunk, metrics binned by
/// `t_center`.
///
/// At `bins == 1` this reduces to the shipped kernel: the chunk *is* the
/// subsegment, `mid_t` is `0.5`, the bin's depth bound is the subsegment's, and
/// the only surviving difference is that the per-cell air-skip compares against
/// the cell's own exact tip depth `sd + t_center·Δd` rather than the segment's
/// lowest attainable tip. That is a strictly tighter *exact* test — the surface
/// this cell would write is `depth + h ≥ depth`, so a cell whose sliver-safe
/// bound sits at or below `depth` is inert by the same argument written out at
/// `cell_can_remove` — so the one-bin case is bit-identical, which
/// `swept_with_one_bin_matches_per_stamp_bit_for_bit` pins.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
fn stamp_swept_chunk(
    band: &mut GridBand<'_>,
    lut: &RadialProfileLUT,
    radius: f64,
    job: &SweptJob,
    from_high: bool,
    mip: Option<&TileMaxTop>,
    out: &mut [StampPartial],
) {
    let bins = out.len();
    if bins == 0 {
        return;
    }
    for slot in out.iter_mut() {
        *slot = StampPartial::empty();
    }
    let (start, end) = job.chunk_segment();
    let (su, sv, sd) = start;
    let (eu, ev, ed) = end;
    let seg_du = eu - su;
    let seg_dv = ev - sv;
    let seg_dd = ed - sd;
    let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;
    if seg_len_sq < 1e-20 || band.rows == 0 || band.cols == 0 {
        return;
    }

    let air_skip = mip.is_some() && from_high && lut.profile_is_nonneg_total();
    let cs = band.cell_size;
    let cell_area = cs * cs;
    let scan_radius = radius + cs * SUBSAMPLE_HALF_EXTENT * std::f64::consts::SQRT_2;

    let u_min = su.min(eu) - scan_radius;
    let u_max = su.max(eu) + scan_radius;
    let v_min = sv.min(ev) - scan_radius;
    let v_max = sv.max(ev) + scan_radius;
    let Some((col_lo_g, col_hi_g, row_lo_g, row_hi_g)) = clamped_cell_bbox(
        ((u_min - band.origin_u) / cs).floor() as isize,
        ((u_max - band.origin_u) / cs).ceil() as isize,
        ((v_min - band.origin_v) / cs).floor() as isize,
        ((v_max - band.origin_v) / cs).ceil() as isize,
        band.cols,
        band.grid_rows,
    ) else {
        return;
    };

    // Whole-chunk early-out, over the GLOBAL box and the chunk's lowest
    // attainable tip — the same all-bands-or-none construction the per-stamp
    // path uses, for the same reason (`DELTA_sim_w2.md` §3b).
    let tip_lo = segment_tip_low(sd, seg_dd);
    if let Some(m) = mip
        && air_skip
        && !cell_can_remove(m.max_over(row_lo_g, row_hi_g, col_lo_g, col_hi_g), tip_lo)
    {
        for slot in out.iter_mut() {
            slot.stamp_skipped = true;
        }
        return;
    }

    let col_lo = col_lo_g;
    let col_hi = col_hi_g.min(band.cols - 1);
    let row_lo = row_lo_g.max(band.row_offset);
    let row_hi = row_hi_g.min(band.last_row());
    if row_lo > row_hi || col_lo > col_hi {
        return;
    }
    // Chunk-level; see the note in `stamp_plunge_chunk`.
    out[0].bbox_cells = ((row_hi + 1 - row_lo) * (col_hi + 1 - col_lo)) as u64;
    for slot in out.iter_mut() {
        slot.stamp_skipped = false;
    }

    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let radius_sq = lut.radius_sq();
    let fast = CoverageFastPath::new(radius_sq, cs);
    let seg_len = seg_len_sq.sqrt();
    let inv_seg_len = if seg_len > 1e-9 { 1.0 / seg_len } else { 0.0 };
    let bins_f = bins as f64;

    // ── Why a cell is SMEARED across bins rather than dropped into one ──
    //
    // The obvious binning — `floor(t_center · bins)`, which is what
    // `PERF_REVIEW.md` S1(a) proposes verbatim — **aliases**, and measurably.
    // A bin is one subsegment wide (`sample_step`, typically 0.25 mm); a cell
    // is `cell_size` wide. Whenever `sample_step < cell_size` — which is the
    // shipped regime for every coarse-grid simulation, including wanaka at
    // 0.4 mm — whole bins have no cell centre in them at all, so their sample
    // reports **zero removal** while its neighbour reports double. Measured on
    // the mixed fixture at `cs = 0.5` before this was added: 35 % of cutting
    // samples came back with `removed_volume_est_mm3 == 0`, against 0 % on the
    // shipped kernel. That is not a change of measure, it is a sampling
    // artifact of the new estimator, and it would have been read downstream as
    // a third of the cut being air.
    //
    // A cell's material is not removed at an instant; it is removed while the
    // cutter crosses it. So the cell's contribution is spread over the
    // parameter interval its own footprint occupies, `t_center ± half_t`, and
    // split between bins by overlap. The half-extent is the projection of a
    // `cs × cs` cell onto the path direction, which makes consecutive cells'
    // intervals **tile** the path (for an axis-aligned move the spacing in `t`
    // is `cs/L` and the width is exactly `cs/L`; at 45° both are `cs√2/L`), so
    // no bin between the first and last covered can be empty. The floor at half
    // a bin width covers the fine-grid case, where a cell is narrower than a
    // subsegment.
    //
    // Weights sum to 1 over the clamped interval (to rounding), so
    // `Σ_b (pre_b − post_b)` is the cell's own removal — the total is
    // conserved and only its distribution over the samples changes. At
    // `bins == 1` the single weight is exactly `1.0`, which is what keeps the
    // one-bin case bit-identical to the shipped kernel; see the note at the
    // division for why that is a division and not a hoisted reciprocal.
    let ux = seg_du * inv_seg_len;
    let uy = seg_dv * inv_seg_len;
    let half_t = (0.5 * cs * (ux.abs() + uy.abs()) * inv_seg_len).max(0.5 / bins_f);

    // Per-bin midpoint (the reference disk for the engagement gate) and per-bin
    // upper tip bound (for the sliver-safe channel). Both are properties of the
    // bin, not of the cell, so they are precomputed once per chunk.
    let mut bin_mid = Vec::with_capacity(bins);
    let mut bin_depth_max = Vec::with_capacity(bins);
    // `sd + 1.0·seg_dd` is not `ed` — `fl(sd + fl(ed − sd))` need not round back
    // (the same last-bit trap `segment_tip_low` documents). The endpoints are
    // therefore taken literally, which is both more accurate and what makes the
    // single-bin case reproduce the shipped kernel's `sd.max(ed)` exactly.
    let d_at = |t: f64| {
        if t == 0.0 {
            sd
        } else if t == 1.0 {
            ed
        } else {
            sd + t * seg_dd
        }
    };
    for b in 0..bins {
        let mid_t = (b as f64 + 0.5) / bins_f;
        bin_mid.push((su + mid_t * seg_du, sv + mid_t * seg_dv));
        let d_lo = d_at(b as f64 / bins_f);
        let d_hi = d_at((b as f64 + 1.0) / bins_f);
        bin_depth_max.push(d_lo.max(d_hi));
    }

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
            // The cell's own subsegment: `t_center` is clamped to [0, 1], so
            // `t_center == 1.0` has to fold into the last bin rather than run
            // off the end.
            let bin = ((t_center * bins_f) as usize).min(bins - 1);
            // The smeared interval and its bin range, clamped to the chunk.
            let lo_t = (t_center - half_t).max(0.0);
            let hi_t = (t_center + half_t).min(1.0);
            let b_lo = ((lo_t * bins_f) as usize).min(bins - 1);
            let b_hi = ((hi_t * bins_f) as usize).min(bins - 1);
            // A DIVISION, not a multiply by a hoisted reciprocal. The
            // single-bin case needs `w` to be exactly `1.0` — that is what
            // keeps `bins == 1` bit-identical to the shipped kernel — and
            // `span * (1.0 / span)` is NOT `1.0` in IEEE754, while `span /
            // span` is. Caught by the plunge-only sentry at 2 ULP on one
            // sample out of 1693, which is the fourth "obviously exact" step
            // in this review to be wrong at the last bit (after S7, G3 and
            // S2's `min(sd, ed)`).
            let span = hi_t - lo_t;
            let idx = band.local(row, col);
            let depth = sd + t_center * seg_dd;

            // S2 per-cell early-out. The cell contributes to exactly one bin,
            // so the inert pair goes to that bin and its difference is
            // unaffected — the reassociation hazard `DELTA_sim_w2.md` §2c
            // describes cannot arise across bins.
            // Overlap weight of bin `b` with the cell's smeared interval.
            let weight = |b: usize| -> f64 {
                let s0 = (b as f64 / bins_f).max(lo_t);
                let s1 = ((b + 1) as f64 / bins_f).min(hi_t);
                if s1 > s0 && span > 0.0 {
                    (s1 - s0) / span
                } else {
                    0.0
                }
            };

            if air_skip && !cell_can_remove(band.conservative_top[idx], depth) {
                let inert = ray_material_length(&band.rays[idx]) as f64 * cell_area;
                for b in b_lo..=b_hi {
                    let w = weight(b);
                    if w <= 0.0 {
                        continue;
                    }
                    out[b].pre_volume += w * inert;
                    out[b].post_volume += w * inert;
                }
                out[0].cells_skipped += 1;
                continue;
            }
            let Some(h) = lut_h_with_edge_fallback(lut, center_d_sq) else {
                continue;
            };

            let ray = &mut band.rays[idx];
            let pre_len = ray_material_length(ray) as f64;
            let cell_tool_surface = if from_high { depth + h } else { depth - h };
            let above = ray_material_length_above(ray, cell_tool_surface as f32) as f64;
            let pre_fresh = if from_high { above } else { pre_len - above };

            if from_high {
                ray_blend_above(ray, cell_tool_surface as f32, coverage);
            } else {
                ray_blend_below(ray, cell_tool_surface as f32, coverage);
            }
            let post_len = ray_material_length(ray) as f64;
            for b in b_lo..=b_hi {
                let w = weight(b);
                if w <= 0.0 {
                    continue;
                }
                out[b].pre_volume += w * pre_len * cell_area;
                out[b].post_volume += w * post_len * cell_area;
            }

            if from_high
                && coverage >= FULL_COVERAGE
                && let Some(ub) = cell_upper_bound_surface(lut, center_d_sq, cs, bin_depth_max[bin])
            {
                band.lower_conservative_top(idx, ub as f32);
            }

            // The engagement block. The reference disk is the CENTRE bin's
            // midpoint — the bin the cell is actually closest to — but the two
            // order statistics are contributed to every bin the cell's
            // footprint spans, **unweighted**. A width and a depth are not
            // quantities you can apportion: the cutter really was that wide and
            // that deep for the whole time it was over this cell, so splitting
            // them by overlap would understate both. Only the volumes, which
            // are extensive, carry weights.
            //
            // Note `perp` itself needs no bin correction at all: every bin's
            // midpoint lies ON the chunk's own line, so the perpendicular
            // projection is the same number whichever midpoint it is measured
            // from. Only the `mid_dist_sq <= radius_sq` reachability gate is
            // bin-dependent.
            let (mid_u, mid_v) = bin_mid[bin];
            let dm_u = cell_u - mid_u;
            let dm_v = cell_v - mid_v;
            let mid_dist_sq = dm_u * dm_u + dm_v * dm_v;
            if mid_dist_sq <= radius_sq && lut.height_at_dist_sq(mid_dist_sq).is_some() {
                let fresh_enough =
                    pre_fresh > FRESH_MATERIAL_THRESHOLD_MM && coverage >= PERP_COVERAGE_GATE;
                let perp = (-seg_dv * dm_u + seg_du * dm_v) * inv_seg_len;
                let removed_here = (pre_len - post_len).max(0.0);
                for b in b_lo..=b_hi {
                    if weight(b) <= 0.0 {
                        continue;
                    }
                    if fresh_enough {
                        if perp < out[b].perp_min {
                            out[b].perp_min = perp;
                        }
                        if perp > out[b].perp_max {
                            out[b].perp_max = perp;
                        }
                    }
                    if removed_here > 1e-6 && removed_here > out[b].max_penetration {
                        out[b].max_penetration = removed_here;
                    }
                }
            }
        }
    }
}

// ── Batching driver ─────────────────────────────────────────────────────

/// Hard cap on chunks per batch.
const MAX_JOBS_PER_BATCH: usize = 8_192;

/// Hard cap on `(band, bin)` partials per batch — the memory bound.
///
/// Sized identically to the whole-path dispatcher's, and it bounds the same
/// product: a chunk of `k` bins touching `m` bands costs `k · m` partials,
/// exactly as `k` separate stamps touching `m` bands each would. Swept
/// dispatch therefore does **not** enlarge the working set relative to wave 4's
/// — it enlarges the *unit of scheduling*, which is the point.
const MAX_PARTIALS_PER_BATCH: usize = 262_144;

/// Fewest chunks a batch takes before the visit budget may close it.
const MIN_JOBS_PER_BATCH: usize = 8;

/// Grid-passes of estimated cell-visits a batch absorbs. Same dial, same
/// reasoning, as `whole_path::BATCH_VISIT_BUDGET_PASSES` — it is the mip's
/// staleness bound, not a memory one.
const BATCH_VISIT_BUDGET_PASSES: u64 = 4;

/// Fewest bands a grid must have before swept dispatch bothers batching.
const MIN_BANDS_FOR_SWEPT: usize = 2;

/// The swept batching driver — the same three phases as
/// [`super::whole_path::BandDispatch`], over chunks instead of stamps.
pub(super) struct SweptDispatch {
    jobs: Vec<SweptJob>,
    /// Offset of each job's bins in `reduced`.
    job_offsets: Vec<usize>,
    total_bins: usize,
    /// Job indices per band, in job order.
    buckets: Vec<Vec<u32>>,
    /// Offset of each bucket entry's bins inside `outs[band]`.
    bucket_offsets: Vec<Vec<usize>>,
    /// Per-band flat partial arrays, positionally aligned with `bucket_offsets`.
    outs: Vec<Vec<StampPartial>>,
    /// Per-bin reduced partial, indexed by `job_offsets[j] + b`.
    reduced: Vec<StampPartial>,
    bands: usize,
    active_rows: Option<(usize, usize)>,
    pending_partials: usize,
    pending_visits: u64,
    visit_budget: u64,
    /// May a `ChunkKind::Swept` chunk hold more than one bin?
    ///
    /// `false` is `StampDispatch::SweptPlungeOnly`: lateral chunks stay at one
    /// bin, where the swept kernel reproduces the shipped one bit-for-bit, so
    /// only the exactly-vertical hoist — which is bit-identical at any length —
    /// actually changes the schedule.
    lateral_chunking: bool,
    stats: StampDispatchStats,
}

impl SweptDispatch {
    pub(super) fn for_grid(grid: &DexelGrid, lateral_chunking: bool) -> Option<Self> {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        if bands < MIN_BANDS_FOR_SWEPT || grid.cols == 0 {
            return None;
        }
        let cells = (grid.rows as u64).saturating_mul(grid.cols as u64).max(1);
        Some(Self {
            jobs: Vec::new(),
            job_offsets: Vec::new(),
            total_bins: 0,
            buckets: vec![Vec::new(); bands],
            bucket_offsets: vec![Vec::new(); bands],
            outs: vec![Vec::new(); bands],
            reduced: Vec::new(),
            bands,
            active_rows: None,
            pending_partials: 0,
            pending_visits: 0,
            visit_budget: cells.saturating_mul(BATCH_VISIT_BUDGET_PASSES),
            lateral_chunking,
            stats: StampDispatchStats {
                bands,
                ..StampDispatchStats::default()
            },
        })
    }

    /// May `job` grow by one more bin and still be worth stamping in one pass?
    pub(super) fn chunk_may_grow(&self, job: &SweptJob, radius: f64, cs: f64) -> bool {
        if job.bins >= SWEPT_MAX_BINS_PER_JOB {
            return false;
        }
        match job.kind {
            // Every bin has the same footprint, so growth is free.
            ChunkKind::PureVertical | ChunkKind::PerBin => true,
            ChunkKind::Swept => {
                if !self.lateral_chunking {
                    return false;
                }
                let mut grown = *job;
                grown.bins += 1;
                chunk_fits(&grown, radius, cs)
            }
        }
    }

    pub(super) fn push(&mut self, grid: &DexelGrid, radius: f64, job: SweptJob) {
        let (s, e) = job.chunk_segment();
        if let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, s, e) {
            self.pending_partials += grid.band_span(row_lo, row_hi).1 * job.bins;
        }
        self.pending_visits = self
            .pending_visits
            .saturating_add(band::stamp_bbox_cells(grid, radius, s, e) as u64);
        self.jobs.push(job);
    }

    pub(super) fn batch_is_due(&self) -> bool {
        self.jobs.len() >= MAX_JOBS_PER_BATCH
            || self.pending_partials >= MAX_PARTIALS_PER_BATCH
            || (self.jobs.len() >= MIN_JOBS_PER_BATCH && self.pending_visits >= self.visit_budget)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub(super) fn stats(&self) -> StampDispatchStats {
        self.stats
    }

    fn build_buckets(&mut self, grid: &DexelGrid, radius: f64) {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        if bands != self.bands {
            self.buckets = vec![Vec::new(); bands];
            self.bucket_offsets = vec![Vec::new(); bands];
            self.outs = vec![Vec::new(); bands];
            self.bands = bands;
        } else {
            for bucket in self.buckets.iter_mut() {
                bucket.clear();
            }
            for offsets in self.bucket_offsets.iter_mut() {
                offsets.clear();
            }
        }
        self.pending_partials = 0;
        self.active_rows = None;
        self.job_offsets.clear();
        self.total_bins = 0;
        let mut band_fill = vec![0usize; bands];
        for (idx, job) in self.jobs.iter().enumerate() {
            self.job_offsets.push(self.total_bins);
            self.total_bins += job.bins;
            let (s, e) = job.chunk_segment();
            let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, s, e) else {
                continue;
            };
            self.active_rows = Some(match self.active_rows {
                None => (row_lo, row_hi),
                Some((lo, hi)) => (lo.min(row_lo), hi.max(row_hi)),
            });
            let (skip, take) = grid.band_span(row_lo, row_hi);
            for b in skip..(skip + take) {
                let (Some(bucket), Some(offsets), Some(fill)) = (
                    self.buckets.get_mut(b),
                    self.bucket_offsets.get_mut(b),
                    band_fill.get_mut(b),
                ) else {
                    continue;
                };
                bucket.push(idx as u32);
                offsets.push(*fill);
                *fill += job.bins;
            }
            self.pending_partials += take * job.bins;
        }
        for (out, fill) in self.outs.iter_mut().zip(band_fill.iter()) {
            out.clear();
            out.resize(*fill, StampPartial::empty());
        }
    }

    /// Run the queued batch. Cancellation granularity is the batch, exactly as
    /// in `whole_path::BandDispatch::run_batch` and for the same reason
    /// (`CancelCheck` is not `Sync`); the enumerator still polls per
    /// subsegment.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_batch(
        &mut self,
        grid: &mut DexelGrid,
        lut: &RadialProfileLUT,
        radius: f64,
        from_high: bool,
        capture_arc_engagement: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
        mut patch: impl FnMut(usize, (f64, f64, Option<f64>, f64)),
    ) -> Result<(), Cancelled> {
        if self.jobs.is_empty() {
            return Ok(());
        }
        check_cancel(cancel)?;
        if let Some(m) = air_mip.as_mut() {
            m.refresh_if_due(grid);
        }
        self.build_buckets(grid, radius);
        self.stats.batches += 1;
        self.stats.max_jobs_in_a_batch = self.stats.max_jobs_in_a_batch.max(self.jobs.len());
        self.stats.max_partials_in_a_batch = self
            .stats
            .max_partials_in_a_batch
            .max(self.pending_partials);

        let Self {
            jobs,
            buckets,
            bucket_offsets,
            outs,
            reduced,
            job_offsets,
            active_rows,
            total_bins,
            ..
        } = self;
        let active_rows = *active_rows;
        let mip = air_mip.as_ref();

        // Dispatch only the bands the batch can reach — wave 4's fan-out fix,
        // unchanged in substance.
        let dispatch_rows = active_rows.and_then(|(row_lo, row_hi)| {
            let (skip, take) = grid.band_span(row_lo, row_hi);
            if skip + take <= buckets.len()
                && skip + take <= outs.len()
                && skip + take <= bucket_offsets.len()
                && take > 0
            {
                Some((row_lo, row_hi, skip, take))
            } else if grid.rows > 0 {
                Some((
                    0,
                    grid.rows - 1,
                    0,
                    buckets.len().min(outs.len()).min(bucket_offsets.len()),
                ))
            } else {
                None
            }
        });
        if let Some((row_lo, row_hi, skip, take)) = dispatch_rows {
            let (Some(buckets), Some(offsets), Some(outs)) = (
                buckets.get(skip..skip + take),
                bucket_offsets.get(skip..skip + take),
                outs.get_mut(skip..skip + take),
            ) else {
                return Ok(());
            };
            grid.par_bands(row_lo, row_hi)
                .zip(buckets.par_iter())
                .zip(offsets.par_iter())
                .zip(outs.par_iter_mut())
                .for_each(|(((mut band, bucket), offsets), out)| {
                    for (k, &j) in bucket.iter().enumerate() {
                        let (Some(job), Some(&off)) = (jobs.get(j as usize), offsets.get(k)) else {
                            continue;
                        };
                        let Some(slice) = out.get_mut(off..off + job.bins) else {
                            continue;
                        };
                        stamp_chunk(&mut band, lut, radius, job, from_high, mip, slice);
                    }
                });
        }

        // ── Phase 3a: reduce, ascending band order per bin ──
        reduced.clear();
        reduced.resize(*total_bins, StampPartial::empty());
        for ((bucket, offsets), out) in buckets.iter().zip(bucket_offsets.iter()).zip(outs.iter()) {
            for (k, &j) in bucket.iter().enumerate() {
                let (Some(job), Some(&off), Some(&base)) = (
                    jobs.get(j as usize),
                    offsets.get(k),
                    job_offsets.get(j as usize),
                ) else {
                    continue;
                };
                for b in 0..job.bins {
                    let (Some(slot), Some(partial)) = (reduced.get_mut(base + b), out.get(off + b))
                    else {
                        continue;
                    };
                    slot.merge(partial);
                }
            }
        }

        // ── Phase 3b: driver-side fixes, mip bookkeeping, patch ──
        for (j, job) in jobs.iter().enumerate() {
            let Some(&base) = job_offsets.get(j) else {
                continue;
            };
            for b in 0..job.bins {
                let Some(r) = reduced.get_mut(base + b) else {
                    continue;
                };
                let (s, e) = job.bin_segment(b);
                let du = e.0 - s.0;
                let dv = e.1 - s.1;
                if du * du + dv * dv < 1e-20 {
                    r.degenerate = true;
                    r.descent = (s.2 - e.2).abs();
                }
                // Charge the mip once per chunk: the chunk walked its bounding
                // box once, and bin 0 is where the kernel recorded that.
                if b == 0
                    && let Some(m) = air_mip.as_mut()
                {
                    m.absorb(r);
                }
                patch(job.first_slot + b, r.finish(radius, capture_arc_engagement));
            }
        }

        self.clear_batch();
        Ok(())
    }

    fn clear_batch(&mut self) {
        self.jobs.clear();
        self.pending_partials = 0;
        self.pending_visits = 0;
    }
}

/// Classify one subsegment the way the shipped kernel does.
///
/// The `1e-20` planar test is applied to the **subsegment**, never to the move:
/// a move with a planar length of `1e-9 mm` is non-degenerate as a whole
/// (`1e-18 > 1e-20`) and degenerate in every one of its 250 subsegments
/// (`1.6e-23 < 1e-20`). Classifying on the move would put those bins through
/// the wrong branch.
pub(super) fn classify_subsegment(
    m_start: (f64, f64, f64),
    m_end: (f64, f64, f64),
    subsegments: usize,
    j: usize,
) -> ChunkKind {
    let n = subsegments as f64;
    let s = lerp3(m_start, m_end, j as f64 / n);
    let e = lerp3(m_start, m_end, (j as f64 + 1.0) / n);
    let du = e.0 - s.0;
    let dv = e.1 - s.1;
    if du * du + dv * dv >= 1e-20 {
        return ChunkKind::Swept;
    }
    // Degenerate. The hoisted plunge kernel is only sound when the move's
    // planar delta is EXACTLY zero, because only then does `s + (e − s)·t`
    // return the same `f64` for every `t`.
    if (m_end.0 - m_start.0) == 0.0 && (m_end.1 - m_start.1) == 0.0 {
        ChunkKind::PureVertical
    } else {
        ChunkKind::PerBin
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

    /// A chunk's working set must stay inside the same bound wave 4 documented.
    #[test]
    fn swept_partial_memory_bound_holds() {
        let partial = std::mem::size_of::<StampPartial>();
        let job = std::mem::size_of::<SweptJob>();
        assert!(partial <= 80, "`StampPartial` is {partial} B");
        assert!(job <= 96, "`SweptJob` is {job} B");
        let bytes = MAX_PARTIALS_PER_BATCH * (partial + std::mem::size_of::<usize>())
            + MAX_JOBS_PER_BATCH * job;
        assert!(
            bytes <= 26 * 1024 * 1024,
            "a swept batch's working set is {} MB",
            bytes / (1024 * 1024)
        );
    }

    /// The chunker must not fire on an axis-aligned raster move (where the
    /// bounding box *is* the stadium's) and must fire on a long diagonal.
    #[test]
    fn chunking_is_driven_by_bbox_waste_not_by_length() {
        let axis = SweptJob {
            m_start: (0.0, 0.0, 0.0),
            m_end: (40.0, 0.0, 0.0),
            subsegments: 160,
            bin0: 0,
            bins: 160,
            first_slot: 0,
            kind: ChunkKind::Swept,
        };
        assert!(
            chunk_fits(&axis, 3.0, 0.1),
            "a 40 mm axis-aligned move must stamp in one pass"
        );
        let diagonal = SweptJob {
            m_end: (28.3, 28.3, 0.0),
            ..axis
        };
        assert!(
            !chunk_fits(&diagonal, 3.0, 0.1),
            "a 40 mm diagonal move's bbox is ~13x its stadium — it must be chunked"
        );
    }

    /// The kernel-equivalence sentry: at **one bin** the swept pass and the
    /// shipped per-stamp kernel must agree bit-for-bit, on the grid and on all
    /// four published metrics.
    ///
    /// This is what isolates the metric movement to the *binning*. If S1's
    /// numbers move and this test still passes, the movement is the change of
    /// measure the delta doc describes; if this test fails, the movement is a
    /// bug in the pass itself and no amount of re-baselining is honest.
    #[test]
    fn swept_with_one_bin_matches_per_stamp_bit_for_bit() {
        use crate::geo::{BoundingBox3, P3};
        use crate::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
        use crate::tool::{BallEndmill, FlatEndmill, MillingCutter};

        let cutters: [Box<dyn MillingCutter>; 2] = [
            Box::new(FlatEndmill::new(6.0, 25.0)),
            Box::new(BallEndmill::new(6.0, 25.0)),
        ];
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(20.0, 16.0, 12.0),
        };
        let segments = [
            // lateral, ramping, and a diagonal ramp (all three depth regimes)
            ((4.0, 6.0, 8.0), (9.0, 6.0, 8.0)),
            ((4.0, 6.0, 9.0), (4.0, 12.0, 7.5)),
            ((4.0, 6.0, 9.3), (9.7, 11.1, 7.25)),
        ];
        let mut checked = 0usize;
        for cutter in cutters.iter() {
            let lut = RadialProfileLUT::from_cutter(cutter.as_ref(), LUT_SAMPLES);
            let radius = cutter.radius();
            for (ms, me) in segments {
                let job = SweptJob {
                    m_start: ms,
                    m_end: me,
                    subsegments: 1,
                    bin0: 0,
                    bins: 1,
                    first_slot: 0,
                    kind: ChunkKind::Swept,
                };
                // Compare on the CHUNK's own endpoints — `s + (e − s)` need not
                // round back to `e`, and the claim is about the kernel, not
                // about `lerp3`'s last bit.
                let (s, e) = job.chunk_segment();
                let mid = lerp3(s, e, 0.5);

                let mut g_ref = DexelGrid::z_grid_from_bounds(&bbox, 0.25);
                let mut g_new = g_ref.clone();
                let last = g_ref.rows - 1;

                let mut a = StampPartial::empty();
                for mut band in g_ref.serial_bands(0, last) {
                    a.merge(&stamp_segment_with_metrics(
                        &mut band, &lut, radius, s, e, mid.0, mid.1, true, None,
                    ));
                }
                let mut b = StampPartial::empty();
                for mut band in g_new.serial_bands(0, last) {
                    let mut out = [StampPartial::empty()];
                    stamp_chunk(&mut band, &lut, radius, &job, true, None, &mut out);
                    b.merge(&out[0]);
                }

                let (fa, fb) = (a.finish(radius, true), b.finish(radius, true));
                assert_eq!(
                    (fa.0.to_bits(), fa.1.to_bits(), fa.3.to_bits()),
                    (fb.0.to_bits(), fb.1.to_bits(), fb.3.to_bits()),
                    "metrics diverged at one bin for {ms:?}..{me:?}: {fa:?} vs {fb:?}"
                );
                assert_eq!(
                    fa.2.map(f64::to_bits),
                    fb.2.map(f64::to_bits),
                    "arc diverged at one bin"
                );
                for (i, (ra, rb)) in g_ref.rays.iter().zip(g_new.rays.iter()).enumerate() {
                    assert_eq!(ra.len(), rb.len(), "cell {i} segment count");
                    for (sa, sb) in ra.iter().zip(rb.iter()) {
                        assert_eq!(
                            (sa.enter.to_bits(), sa.exit.to_bits()),
                            (sb.enter.to_bits(), sb.exit.to_bits()),
                            "cell {i} ray bits"
                        );
                    }
                }
                for (i, (ca, cb)) in g_ref
                    .conservative_top
                    .iter()
                    .zip(g_new.conservative_top.iter())
                    .enumerate()
                {
                    assert_eq!(ca.to_bits(), cb.to_bits(), "conservative_top at cell {i}");
                }
                assert!(a.bbox_cells > 100, "vacuous stamp: {} cells", a.bbox_cells);
                checked += 1;
            }
        }
        assert_eq!(checked, 6, "the sweep was thinned");
    }

    /// Degeneracy is a property of the SUBSEGMENT. A move with a planar length
    /// above the threshold whose subsegments fall below it must classify as
    /// degenerate, and — because its planar delta is not exactly zero — must
    /// take the conservative per-bin arm rather than the hoisted one.
    #[test]
    fn near_vertical_moves_classify_per_bin_not_pure_vertical() {
        let s = (0.0, 0.0, 5.0);
        let e = (1e-9, 0.0, 0.0);
        assert_eq!(classify_subsegment(s, e, 1, 0), ChunkKind::Swept);
        assert_eq!(classify_subsegment(s, e, 250, 0), ChunkKind::PerBin);
        let vertical = (0.0, 0.0, 0.0);
        assert_eq!(
            classify_subsegment(s, vertical, 250, 0),
            ChunkKind::PureVertical
        );
    }
}
