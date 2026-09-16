//! **Track C research module — shape-selected spiral for COMPACT regions.**
//!
//! Research-only. Nothing here is on a production path: no operation, no
//! generator, no GUI surface and no MCP tool reaches this module. It follows
//! the precedent of [`crate::finish::conformal_spiral`] (Phase F2) and
//! [`crate::finish::direction_field`] (Phase F1) — unshipped research candidates that
//! document their own limitations.
//!
//! # What this module does
//!
//! It takes the **nested closed level sets** of a scalar field on a surface —
//! in Track C, the medial/EDT field of `planning/finishing_synthesis_2026-08-30.md`
//! §9, whose level sets are iso-distance offsets of the region boundary — and
//! bridges them into **one continuous spiral** with the F2 log-rectangle
//! bridging blend ([`crate::finish::conformal_spiral::blend_sigma`], [SOURCE-2024
//! arXiv:2309.10655 Eq. A-11]). The synthesis §2a states the design fact this
//! module rests on: *the bridging is a property of the connection step, not of
//! the conformal map* — any field whose level sets are nested closed loops can
//! be bridged the same way. **No conformal map appears anywhere in this
//! module.**
//!
//! # The mechanism
//!
//! 1. **Validate, refusal-first.** Each level must contribute exactly one
//!    closed loop. A branched region splits its offsets at every branch point
//!    — that shape gets a typed [`CompactSpiralRefusal`], never a silent
//!    fallback (`FINDINGS.md` §9: the medial field fragments MORE than a sweep
//!    on branched geometry; this module is for the compact case only).
//! 2. **Polar parameterisation.** The hub is the innermost loop's vertex mean.
//!    Every loop must encircle the hub exactly once and be star-shaped about
//!    it within a stated backtrack tolerance; both are refusals otherwise.
//! 3. **Nesting check.** On a shared angular lattice, each ring's radius
//!    function must sit strictly inside its outer neighbour's.
//! 4. **Bridge.** Each ring runs a full revolution at its own radius function,
//!    then a bridge sector blends `r_i(θ) → r_{i+1}(θ)` with `σ(t)` — the F2
//!    structure ([SOURCE-2025 arXiv:2504.06310 Eqs. 7–9] bookkeeping), with
//!    the paper's near-centre rule. The start angle is swept and the shortest
//!    spiral kept, as F2 does.
//! 5. **Optional hub blend.** When the innermost ring is farther from the hub
//!    than the tool's lateral reach, a final revolution spirals to the hub, so
//!    the centre is covered. Counted in the report, never silent.
//!
//! Points in and out are **cutter-contact points**; gouge protection is the
//! caller's job (the Track C instrument re-drops every point through the
//! drop-cutter, exactly as the F2 evidence instrument does, because the source
//! papers have no gouge handling at all).

use std::f64::consts::TAU;

use crate::finish::conformal_spiral::{DEFAULT_BLEND_P, PAPER_INITIAL_BRIDGE_SHIFT, blend_sigma};
use crate::geo::P3;

/// Consecutive spiral points closer than this (mm) are merged.
const EPS_POINT_MM: f64 = 1e-9;

/// A radial revisit of one lattice cell must not INCREASE by more than this
/// (mm) — the polar-domain self-intersection tripwire.
const EPS_ORDER_MM: f64 = 1e-6;

/// Tolerance on the winding number of one loop about the hub, in turns.
const WINDING_TOL_TURNS: f64 = 0.02;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Tunables for [`bridge_nested_levels`]. Every default is either the F2
/// module's own constant or a stated [REPO] choice.
#[derive(Debug, Clone)]
pub struct CompactSpiralParams {
    /// Grading parameter `p` of the bridge blend `σ` —
    /// [`crate::finish::conformal_spiral::DEFAULT_BLEND_P`].
    pub blend_p: f64,
    /// Ceiling on one lattice chord (mm) at the outermost ring; sets the
    /// shared angular lattice density. [REPO] default 0.1.
    pub max_chord_mm: f64,
    /// A loop whose first and last points are farther apart than this (mm) is
    /// refused as open. [REPO] default 0.05.
    pub closure_tol_mm: f64,
    /// Largest tolerated angular backtrack (rad) while walking a loop about
    /// the hub. Beyond it the loop is refused as not star-shaped; below it the
    /// backtracking vertices are dropped and counted. [REPO] default 0.35.
    pub max_backtrack_rad: f64,
    /// Bridge sector span (rad) away from the hub —
    /// [`crate::finish::conformal_spiral::PAPER_INITIAL_BRIDGE_SHIFT`] (`π/10`).
    pub bridge_span_rad: f64,
    /// A ring whose mean radius is below this fraction of the outermost
    /// ring's mean radius takes the near-hub bridge span — the paper's
    /// near-centre rule restated on the mm domain. Default 0.3, the paper's
    /// disk-domain constant.
    pub near_hub_fraction: f64,
    /// Near-hub bridge sector span (rad) — the paper's `2π`.
    pub near_hub_bridge_span_rad: f64,
    /// Start angles tried; the shortest spiral is kept. [REPO] default 10 —
    /// the F2 evidence runs' own sweep resolution (`π/5`).
    pub start_angle_candidates: usize,
    /// The tool's lateral reach (mm). When the innermost ring's maximum hub
    /// distance exceeds it, a terminal revolution blends to the hub. `0.0`
    /// disables the hub blend.
    pub hub_cover_reach_mm: f64,
    /// Floor on the shared lattice size.
    pub min_lattice: usize,
    /// Ceiling on the shared lattice size.
    pub max_lattice: usize,
}

impl Default for CompactSpiralParams {
    fn default() -> Self {
        Self {
            blend_p: DEFAULT_BLEND_P,
            max_chord_mm: 0.1,
            closure_tol_mm: 0.05,
            max_backtrack_rad: 0.35,
            bridge_span_rad: PAPER_INITIAL_BRIDGE_SHIFT,
            near_hub_fraction: 0.3,
            near_hub_bridge_span_rad: TAU,
            start_angle_candidates: 10,
            hub_cover_reach_mm: 0.0,
            min_lattice: 64,
            max_lattice: 8192,
        }
    }
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why [`bridge_nested_levels`] declined. Refusal-first: a region whose level
/// sets are not nested closed loops gets one of these, never a silent
/// fallback.
#[derive(Debug, Clone, PartialEq)]
pub enum CompactSpiralRefusal {
    /// No usable level was handed in.
    NoLevels,
    /// A level contributed a number of loops other than one — the branched
    /// signature: offsets split at every branch point.
    LevelNotSingleLoop {
        /// Index into the input `levels` slice.
        level: usize,
        /// Loops found at that level.
        loops: usize,
    },
    /// A level's loop does not close on itself.
    OpenLevelSet {
        /// Index into the input `levels` slice.
        level: usize,
        /// First-to-last gap (mm).
        gap_mm: f64,
    },
    /// A loop with fewer than four distinct points cannot be parameterised.
    TooFewVertices {
        /// Index into the input `levels` slice.
        level: usize,
        /// Distinct points found.
        vertices: usize,
    },
    /// A loop's winding about the hub is not one full turn — it does not
    /// encircle the hub, so it is not a nested ring of this family.
    DoesNotEncircleHub {
        /// Index into the input `levels` slice.
        level: usize,
        /// Measured winding, in turns; nominal is ±1.
        winding_turns: f64,
    },
    /// A loop backtracks in hub angle beyond
    /// [`CompactSpiralParams::max_backtrack_rad`] — not star-shaped about the
    /// hub, so a polar radius function does not exist for it.
    NotStarShaped {
        /// Index into the input `levels` slice.
        level: usize,
        /// Largest angular backtrack found (rad).
        backtrack_rad: f64,
    },
    /// Two rings cross or touch: the inner one is not strictly inside the
    /// outer one at every lattice angle.
    NotNested {
        /// Ring index (outer→inner order) of the outer ring of the pair.
        outer_ring: usize,
        /// Ring index of the inner ring of the pair.
        inner_ring: usize,
        /// Smallest radial separation found (mm); `<= 0` is the violation.
        min_separation_mm: f64,
    },
}

// ---------------------------------------------------------------------------
// Report + result
// ---------------------------------------------------------------------------

/// min / p50 / p90 / max over a measured population.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeparationStats {
    /// Smallest value.
    pub min: f64,
    /// Median.
    pub p50: f64,
    /// 90th percentile.
    pub p90: f64,
    /// Largest value.
    pub max: f64,
    /// Population size. Zero means nothing was measured.
    pub samples: usize,
}

/// Everything the Track C contract wants counted rather than assumed. The
/// report survives a refusal, filled as far as validation got.
#[derive(Debug, Clone, Default)]
pub struct CompactSpiralReport {
    /// Levels handed in.
    pub levels_in: usize,
    /// Rings accepted (equals `levels_in` on success).
    pub rings: usize,
    /// Shared angular lattice size.
    pub lattice_angles: usize,
    /// Hub (XY, mm) — the innermost loop's vertex mean.
    pub hub_x: f64,
    /// See [`CompactSpiralReport::hub_x`].
    pub hub_y: f64,
    /// Vertices dropped by the monotonisation walk, all rings. Each was a
    /// backtrack below the refusal tolerance.
    pub monotonized_vertices: usize,
    /// `true` when sorting by area changed the input order beyond a plain
    /// reversal — the input levels were not radially ordered.
    pub levels_reordered: bool,
    /// Smallest radial separation between adjacent rings at any lattice angle
    /// (mm). `> 0` on success by construction.
    pub min_ring_separation_mm: f64,
    /// 3D distance between adjacent rings at matched lattice angles — the
    /// achieved pass spacing.
    pub pass_separation_mm: SeparationStats,
    /// Summed 3D length of the pure ring runs (mm).
    pub ring_run_length_mm: f64,
    /// Summed 3D length of the bridge sectors and the hub blend (mm).
    pub bridge_length_mm: f64,
    /// `bridge_length / ring_run_length` — the continuity overhead.
    pub bridge_overhead_fraction: f64,
    /// Bridge sectors emitted.
    pub bridge_count: usize,
    /// Whether a terminal hub blend was appended.
    pub hub_blend_added: bool,
    /// Start angle of the kept spiral (rad).
    pub start_angle_rad: f64,
    /// Start angles tried.
    pub start_candidates: usize,
    /// Lattice cells whose radius INCREASED on a revisit — the polar-domain
    /// self-intersection count. Must be 0; a nonzero is a defect, not noise.
    pub polar_order_violations: usize,
    /// Points in the emitted spiral.
    pub spiral_points: usize,
}

/// The bridged spiral: one continuous cutter-contact polyline, outermost ring
/// first, hub last.
///
/// **Test door.** Stays `pub` for two reasons: `bridge_nested_levels`
/// returns it, and the harness `tests/whole_board_spiral_ledger_g1.rs`
/// binds it (S29, 2026-09-16).
#[derive(Debug, Clone, Default)]
pub struct CompactSpiral {
    /// Cutter-contact points. `z` is interpolated from the input rings and is
    /// advisory: the caller re-drops every point for gouge protection.
    pub contact: Vec<P3>,
}

// ---------------------------------------------------------------------------
// Internal ring form
// ---------------------------------------------------------------------------

/// One loop as a polar radius function about the hub: strictly ascending
/// unwrapped angles spanning one full turn, with a sentinel copy of the first
/// point at `θ₀ + 2π` so interpolation never runs off the seam.
struct PolarRing {
    theta: Vec<f64>,
    radius: Vec<f64>,
    z: Vec<f64>,
    mean_radius: f64,
    max_radius: f64,
}

impl PolarRing {
    /// `(r, z)` at absolute angle `q`, periodic.
    fn sample(&self, q: f64) -> (f64, f64) {
        let (Some(&first), Some(&last)) = (self.theta.first(), self.theta.last()) else {
            return (0.0, 0.0);
        };
        let span = last - first;
        if span <= f64::MIN_POSITIVE {
            return (
                self.radius.first().copied().unwrap_or(0.0),
                self.z.first().copied().unwrap_or(0.0),
            );
        }
        let mut q = (q - first) % TAU;
        if q < 0.0 {
            q += TAU;
        }
        let q = first + q.min(span);
        let i = match self
            .theta
            .binary_search_by(|t| t.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let i = i.min(self.theta.len().saturating_sub(2));
        let (t0, t1) = (
            self.theta.get(i).copied().unwrap_or(first),
            self.theta.get(i + 1).copied().unwrap_or(last),
        );
        let f = if t1 - t0 > f64::MIN_POSITIVE {
            ((q - t0) / (t1 - t0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let r0 = self.radius.get(i).copied().unwrap_or(0.0);
        let r1 = self.radius.get(i + 1).copied().unwrap_or(r0);
        let z0 = self.z.get(i).copied().unwrap_or(0.0);
        let z1 = self.z.get(i + 1).copied().unwrap_or(z0);
        (r0 + (r1 - r0) * f, z0 + (z1 - z0) * f)
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Bridge the nested closed level sets in `levels` into one continuous
/// contact-space spiral.
///
/// `levels` is one entry per field level, each holding that level's loops as
/// closed 3D polylines (a closed loop repeats its first point as its last —
/// [`crate::finish::direction_field::FieldPathResult`]'s convention; a helper for
/// that type is [`levels_from_field_result`]). Level order need not be
/// radial; rings are sorted by enclosed XY area and the reordering is
/// reported.
///
/// Returns the spiral or a typed refusal, plus a report filled as far as the
/// pipeline got.
pub fn bridge_nested_levels(
    levels: &[Vec<Vec<P3>>],
    params: &CompactSpiralParams,
) -> (
    Result<CompactSpiral, CompactSpiralRefusal>,
    Box<CompactSpiralReport>,
) {
    let mut report = Box::new(CompactSpiralReport {
        levels_in: levels.len(),
        start_candidates: params.start_angle_candidates.max(1),
        ..CompactSpiralReport::default()
    });
    if levels.is_empty() {
        return (Err(CompactSpiralRefusal::NoLevels), report);
    }

    // -- 1. one closed loop per level, closing duplicate stripped ---------
    let mut loops: Vec<(usize, Vec<P3>, f64)> = Vec::with_capacity(levels.len());
    for (level, polylines) in levels.iter().enumerate() {
        if polylines.len() != 1 {
            return (
                Err(CompactSpiralRefusal::LevelNotSingleLoop {
                    level,
                    loops: polylines.len(),
                }),
                report,
            );
        }
        let Some(line) = polylines.first() else {
            return (
                Err(CompactSpiralRefusal::LevelNotSingleLoop { level, loops: 0 }),
                report,
            );
        };
        let (Some(first), Some(last)) = (line.first(), line.last()) else {
            return (
                Err(CompactSpiralRefusal::TooFewVertices { level, vertices: 0 }),
                report,
            );
        };
        let gap = (first - last).norm();
        if gap > params.closure_tol_mm {
            return (
                Err(CompactSpiralRefusal::OpenLevelSet { level, gap_mm: gap }),
                report,
            );
        }
        // Strip the closing duplicate and any repeated tail points.
        let mut pts: Vec<P3> = Vec::with_capacity(line.len());
        for &p in line {
            if pts.last().is_none_or(|&q| (p - q).norm() > EPS_POINT_MM) {
                pts.push(p);
            }
        }
        if pts
            .last()
            .zip(pts.first())
            .is_some_and(|(&a, &b)| (a - b).norm() <= params.closure_tol_mm)
            && pts.len() > 1
        {
            pts.pop();
        }
        if pts.len() < 4 {
            return (
                Err(CompactSpiralRefusal::TooFewVertices {
                    level,
                    vertices: pts.len(),
                }),
                report,
            );
        }
        let area = shoelace_area_abs(&pts);
        loops.push((level, pts, area));
    }

    // -- 2. hub from the smallest loop, rings sorted outer→inner ----------
    let input_order: Vec<usize> = loops.iter().map(|(level, _, _)| *level).collect();
    loops.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let sorted_order: Vec<usize> = loops.iter().map(|(level, _, _)| *level).collect();
    let mut reversed = input_order.clone();
    reversed.reverse();
    report.levels_reordered = sorted_order != input_order && sorted_order != reversed;

    let (hub_x, hub_y) = loops
        .last()
        .map(|(_, pts, _)| {
            let n = pts.len().max(1) as f64;
            let sx: f64 = pts.iter().map(|p| p.x).sum();
            let sy: f64 = pts.iter().map(|p| p.y).sum();
            (sx / n, sy / n)
        })
        .unwrap_or((0.0, 0.0));
    report.hub_x = hub_x;
    report.hub_y = hub_y;

    // -- 3. polar parameterisation, refusal-first --------------------------
    let mut rings: Vec<PolarRing> = Vec::with_capacity(loops.len());
    for (level, pts, _) in &loops {
        match polar_ring(pts, hub_x, hub_y, params, &mut report.monotonized_vertices) {
            Ok(ring) => rings.push(ring),
            Err(kind) => {
                let refusal = match kind {
                    PolarFailure::Winding(turns) => CompactSpiralRefusal::DoesNotEncircleHub {
                        level: *level,
                        winding_turns: turns,
                    },
                    PolarFailure::Backtrack(rad) => CompactSpiralRefusal::NotStarShaped {
                        level: *level,
                        backtrack_rad: rad,
                    },
                    PolarFailure::Degenerate => CompactSpiralRefusal::TooFewVertices {
                        level: *level,
                        vertices: pts.len(),
                    },
                };
                return (Err(refusal), report);
            }
        }
    }
    report.rings = rings.len();

    // -- 4. shared lattice + strict nesting --------------------------------
    let outer_max_r = rings.first().map_or(0.0, |r| r.max_radius);
    let lattice = ((TAU * outer_max_r / params.max_chord_mm.max(1e-6)).ceil() as usize)
        .clamp(params.min_lattice.max(8), params.max_lattice.max(8));
    report.lattice_angles = lattice;
    let dtheta = TAU / (lattice as f64);

    let mut min_sep = f64::INFINITY;
    let mut pass_sep: Vec<f64> = Vec::new();
    for pair in 0..rings.len().saturating_sub(1) {
        let (Some(outer), Some(inner)) = (rings.get(pair), rings.get(pair + 1)) else {
            continue;
        };
        let mut pair_min = f64::INFINITY;
        for k in 0..lattice {
            let q = (k as f64) * dtheta;
            let (ro, zo) = outer.sample(q);
            let (ri, zi) = inner.sample(q);
            let sep = ro - ri;
            pair_min = pair_min.min(sep);
            let dz = zo - zi;
            pass_sep.push((sep * sep + dz * dz).sqrt());
        }
        min_sep = min_sep.min(pair_min);
        if pair_min <= 0.0 {
            report.min_ring_separation_mm = pair_min;
            return (
                Err(CompactSpiralRefusal::NotNested {
                    outer_ring: pair,
                    inner_ring: pair + 1,
                    min_separation_mm: pair_min,
                }),
                report,
            );
        }
    }
    report.min_ring_separation_mm = if min_sep.is_finite() { min_sep } else { 0.0 };
    report.pass_separation_mm = summarise(&mut pass_sep);

    // -- 5. start-angle sweep: keep the shortest spiral ---------------------
    let candidates = params.start_angle_candidates.max(1);
    let mut best: Option<(f64, BuiltSpiral)> = None;
    for c in 0..candidates {
        let a0 = ((c as f64) * (lattice as f64) / (candidates as f64)).round() * dtheta;
        let built = build_spiral(&rings, hub_x, hub_y, a0, lattice, dtheta, params);
        let total = built.ring_length_mm + built.bridge_length_mm;
        if best.as_ref().is_none_or(|(len, _)| total < *len) {
            best = Some((total, built));
        }
    }
    let Some((_, built)) = best else {
        return (Err(CompactSpiralRefusal::NoLevels), report);
    };

    report.ring_run_length_mm = built.ring_length_mm;
    report.bridge_length_mm = built.bridge_length_mm;
    report.bridge_overhead_fraction = if built.ring_length_mm > f64::MIN_POSITIVE {
        built.bridge_length_mm / built.ring_length_mm
    } else {
        0.0
    };
    report.bridge_count = built.bridge_count;
    report.hub_blend_added = built.hub_blend_added;
    report.start_angle_rad = built.start_angle;
    report.polar_order_violations = built.order_violations;
    report.spiral_points = built.contact.len();

    (
        Ok(CompactSpiral {
            contact: built.contact,
        }),
        report,
    )
}

/// Group a [`crate::finish::direction_field::FieldPathResult`]'s polylines by level,
/// in level order — the shape [`bridge_nested_levels`] takes.
#[must_use]
pub fn levels_from_field_result(
    result: &crate::finish::direction_field::FieldPathResult,
) -> Vec<Vec<Vec<P3>>> {
    let mut out: Vec<Vec<Vec<P3>>> = vec![Vec::new(); result.levels.len()];
    for (line, &level) in result.polylines.iter().zip(result.polyline_levels.iter()) {
        if let Some(slot) = out.get_mut(level) {
            slot.push(line.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

enum PolarFailure {
    Winding(f64),
    Backtrack(f64),
    Degenerate,
}

/// Unsigned XY shoelace area of a loop (closing edge implied).
fn shoelace_area_abs(pts: &[P3]) -> f64 {
    let mut twice = 0.0;
    for (i, a) in pts.iter().enumerate() {
        let b = pts.get((i + 1) % pts.len().max(1)).unwrap_or(a);
        twice += a.x * b.y - b.x * a.y;
    }
    (twice * 0.5).abs()
}

/// Parameterise one loop as a polar radius function about the hub.
fn polar_ring(
    pts: &[P3],
    hub_x: f64,
    hub_y: f64,
    params: &CompactSpiralParams,
    monotonized: &mut usize,
) -> Result<PolarRing, PolarFailure> {
    // Winding about the hub over the closed cycle.
    let mut winding = 0.0_f64;
    let mut prev_angle: Option<f64> = None;
    for p in pts.iter().chain(pts.first()) {
        let a = (p.y - hub_y).atan2(p.x - hub_x);
        if let Some(prev) = prev_angle {
            let mut d = a - prev;
            while d > std::f64::consts::PI {
                d -= TAU;
            }
            while d < -std::f64::consts::PI {
                d += TAU;
            }
            winding += d;
        }
        prev_angle = Some(a);
    }
    let turns = winding / TAU;
    if (turns.abs() - 1.0).abs() > WINDING_TOL_TURNS {
        return Err(PolarFailure::Winding(turns));
    }

    // Orient counter-clockwise.
    let ordered: Vec<&P3> = if turns >= 0.0 {
        pts.iter().collect()
    } else {
        pts.iter().rev().collect()
    };

    // Unwrapped angles, monotonised with a counted drop below the refusal
    // tolerance and a refusal above it.
    let mut theta: Vec<f64> = Vec::with_capacity(pts.len() + 1);
    let mut radius: Vec<f64> = Vec::with_capacity(pts.len() + 1);
    let mut zs: Vec<f64> = Vec::with_capacity(pts.len() + 1);
    // `running` is the cumulative unwrapped angle over ALL vertices, kept or
    // dropped, so a dropped vertex's angular step is never lost.
    let mut running = f64::NAN;
    let mut prev = 0.0_f64;
    let mut sum_r = 0.0_f64;
    let mut max_r = 0.0_f64;
    for p in &ordered {
        let a = (p.y - hub_y).atan2(p.x - hub_x);
        if running.is_nan() {
            running = a;
        } else {
            let mut d = a - prev;
            while d > std::f64::consts::PI {
                d -= TAU;
            }
            while d < -std::f64::consts::PI {
                d += TAU;
            }
            running += d;
        }
        prev = a;
        let r = (p.x - hub_x).hypot(p.y - hub_y);
        sum_r += r;
        max_r = max_r.max(r);
        match theta.last() {
            None => {
                theta.push(running);
                radius.push(r);
                zs.push(p.z);
            }
            Some(&last) if running > last + f64::MIN_POSITIVE => {
                theta.push(running);
                radius.push(r);
                zs.push(p.z);
            }
            Some(&last) => {
                let back = last - running;
                if back > params.max_backtrack_rad {
                    return Err(PolarFailure::Backtrack(back));
                }
                *monotonized += 1;
            }
        }
    }
    if theta.len() < 4 {
        return Err(PolarFailure::Degenerate);
    }
    // Sentinel: the first point again at θ₀ + 2π, so sampling wraps cleanly.
    // A trailing vertex past θ₀ + 2π (the closing edge steps back over the
    // seam) would break monotonicity against the sentinel; drop and count it.
    let (Some(&t0), Some(&r0), Some(&z0)) = (theta.first(), radius.first(), zs.first()) else {
        return Err(PolarFailure::Degenerate);
    };
    while theta.last().is_some_and(|&t| t >= t0 + TAU) && theta.len() > 4 {
        theta.pop();
        radius.pop();
        zs.pop();
        *monotonized += 1;
    }
    theta.push(t0 + TAU);
    radius.push(r0);
    zs.push(z0);
    let mean_radius = sum_r / (ordered.len().max(1) as f64);
    Ok(PolarRing {
        theta,
        radius,
        z: zs,
        mean_radius,
        max_radius: max_r,
    })
}

struct BuiltSpiral {
    contact: Vec<P3>,
    ring_length_mm: f64,
    bridge_length_mm: f64,
    bridge_count: usize,
    hub_blend_added: bool,
    start_angle: f64,
    order_violations: usize,
}

/// Emit the spiral for one start angle. Outer ring first; each ring runs a
/// full revolution, then a bridge sector blends to the next; an optional
/// terminal revolution blends to the hub.
fn build_spiral(
    rings: &[PolarRing],
    hub_x: f64,
    hub_y: f64,
    a0: f64,
    lattice: usize,
    dtheta: f64,
    params: &CompactSpiralParams,
) -> BuiltSpiral {
    let hub_switch = rings.first().map_or(0.0, |r| r.mean_radius) * params.near_hub_fraction;
    let mut contact: Vec<P3> = Vec::new();
    let mut ring_len = 0.0_f64;
    let mut bridge_len = 0.0_f64;
    let mut bridge_count = 0_usize;
    let mut order_violations = 0_usize;
    let mut last_r_at_cell: Vec<f64> = vec![f64::INFINITY; lattice];

    // Every emitted point sits on the shared lattice, and `a0` is snapped to
    // it by the caller, so the walk is INTEGER cell arithmetic: `cursor + j`
    // is the absolute lattice step of a point and `% lattice` its cell. A
    // float `floor(q / δ)` here would flip cells at boundaries (one ulp of
    // accumulated `2π` per revolution) and misread ring wobble between
    // neighbouring cells as a radial increase — measured as 270 phantom
    // violations on the first sphere run.
    let a0_cells = (a0 / dtheta).round().max(0.0) as usize;
    let mut cursor = a0_cells;
    let mut push = |contact: &mut Vec<P3>, len_acc: &mut f64, step: usize, r: f64, z: f64| {
        let q = (step as f64) * dtheta;
        let p = P3::new(hub_x + r * q.cos(), hub_y + r * q.sin(), z);
        if let Some(&last) = contact.last() {
            let d = (p - last).norm();
            if d <= EPS_POINT_MM {
                return;
            }
            *len_acc += d;
        }
        if lattice > 0
            && let Some(slot) = last_r_at_cell.get_mut(step % lattice)
        {
            if r > *slot + EPS_ORDER_MM {
                order_violations += 1;
            }
            *slot = r;
        }
        contact.push(p);
    };

    for (i, ring) in rings.iter().enumerate() {
        let first = usize::from(i > 0);
        for j in first..=lattice {
            let step = cursor + j;
            let q = (step as f64) * dtheta;
            let (r, z) = ring.sample(q);
            push(&mut contact, &mut ring_len, step, r, z);
        }
        cursor += lattice;

        if let Some(next) = rings.get(i + 1) {
            let span = if ring.mean_radius > hub_switch {
                params.bridge_span_rad
            } else {
                params.near_hub_bridge_span_rad
            };
            let bc = ((span / dtheta).round() as usize).max(1);
            for j in 1..=bc {
                let t = (j as f64) / (bc as f64);
                let s = blend_sigma(TAU * t, params.blend_p) / TAU;
                let step = cursor + j;
                let q = (step as f64) * dtheta;
                let (r0, z0) = ring.sample(q);
                let (r1, z1) = next.sample(q);
                push(
                    &mut contact,
                    &mut bridge_len,
                    step,
                    r0 + (r1 - r0) * s,
                    z0 + (z1 - z0) * s,
                );
            }
            cursor += bc;
            bridge_count += 1;
        }
    }

    // Terminal hub blend when the innermost ring leaves an uncovered core.
    let mut hub_blend_added = false;
    if params.hub_cover_reach_mm > 0.0
        && let Some(inner) = rings.last()
        && inner.max_radius > params.hub_cover_reach_mm
    {
        for j in 1..=lattice {
            let t = (j as f64) / (lattice as f64);
            let s = blend_sigma(TAU * t, params.blend_p) / TAU;
            let step = cursor + j;
            let q = (step as f64) * dtheta;
            let (r0, z0) = inner.sample(q);
            push(&mut contact, &mut bridge_len, step, r0 * (1.0 - s), z0);
        }
        hub_blend_added = true;
        bridge_count += 1;
    }

    BuiltSpiral {
        contact,
        ring_length_mm: ring_len,
        bridge_length_mm: bridge_len,
        bridge_count,
        hub_blend_added,
        start_angle: a0,
        order_violations,
    }
}

/// Sort in place and take min/p50/p90/max.
fn summarise(values: &mut [f64]) -> SeparationStats {
    if values.is_empty() {
        return SeparationStats::default();
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |f: f64| -> f64 {
        let idx = (((values.len() - 1) as f64) * f).round() as usize;
        values
            .get(idx.min(values.len() - 1))
            .copied()
            .unwrap_or(f64::NAN)
    };
    SeparationStats {
        min: pick(0.0),
        p50: pick(0.5),
        p90: pick(0.9),
        max: pick(1.0),
        samples: values.len(),
    }
}

// ---------------------------------------------------------------------------
// Tests — refusal-first, plus the concentric happy path
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn circle(cx: f64, cy: f64, r: f64, z: f64, n: usize) -> Vec<P3> {
        let mut pts: Vec<P3> = (0..n)
            .map(|k| {
                let t = TAU * (k as f64) / (n as f64);
                P3::new(cx + r * t.cos(), cy + r * t.sin(), z)
            })
            .collect();
        pts.push(pts[0]);
        pts
    }

    #[test]
    fn concentric_circles_bridge_into_one_spiral() {
        let levels: Vec<Vec<Vec<P3>>> = (0..6)
            .map(|i| vec![circle(0.0, 0.0, 6.0 - (i as f64) * 0.5, 0.0, 256)])
            .collect();
        let params = CompactSpiralParams {
            hub_cover_reach_mm: 0.25,
            ..CompactSpiralParams::default()
        };
        let (result, report) = bridge_nested_levels(&levels, &params);
        let spiral = result.expect("concentric circles must bridge");
        assert_eq!(report.rings, 6);
        assert_eq!(report.polar_order_violations, 0);
        assert!(report.hub_blend_added, "3.5 mm inner ring > 0.25 mm reach");
        assert!(report.min_ring_separation_mm > 0.49);
        assert!(spiral.contact.len() > 6 * 200);
        // The bridging must not dominate: 5 bridges of pi/10 over 6 rings plus
        // the terminal revolution is well under 40 % here.
        assert!(report.bridge_overhead_fraction < 0.40);
        // Pass spacing is the ring spacing on circles.
        assert!((report.pass_separation_mm.p50 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_branched_level_refuses_with_loop_count() {
        let levels = vec![
            vec![circle(0.0, 0.0, 6.0, 0.0, 128)],
            // The branched signature: one level, two loops.
            vec![
                circle(-2.0, 0.0, 1.0, 0.0, 64),
                circle(2.0, 0.0, 1.0, 0.0, 64),
            ],
        ];
        let (result, _) = bridge_nested_levels(&levels, &CompactSpiralParams::default());
        assert_eq!(
            result.unwrap_err(),
            CompactSpiralRefusal::LevelNotSingleLoop { level: 1, loops: 2 }
        );
    }

    #[test]
    fn an_open_level_set_refuses() {
        let mut open = circle(0.0, 0.0, 4.0, 0.0, 64);
        open.pop();
        open.truncate(40);
        let levels = vec![vec![circle(0.0, 0.0, 6.0, 0.0, 128)], vec![open]];
        let (result, _) = bridge_nested_levels(&levels, &CompactSpiralParams::default());
        assert!(matches!(
            result.unwrap_err(),
            CompactSpiralRefusal::OpenLevelSet { level: 1, .. }
        ));
    }

    #[test]
    fn crossing_rings_refuse_as_not_nested() {
        // Same radius, offset centres: they cross.
        let levels = vec![
            vec![circle(0.0, 0.0, 3.0, 0.0, 128)],
            vec![circle(1.5, 0.0, 3.0, 0.0, 128)],
        ];
        let (result, _) = bridge_nested_levels(&levels, &CompactSpiralParams::default());
        assert!(matches!(
            result.unwrap_err(),
            CompactSpiralRefusal::NotNested { .. }
        ));
    }

    #[test]
    fn a_loop_that_misses_the_hub_refuses() {
        // The inner loop (smallest area) sets the hub at (4, 0); the outer
        // ring around the origin still encircles it, but a side lobe at
        // (-4, 0) does not.
        let levels = vec![
            vec![circle(0.0, 0.0, 6.0, 0.0, 256)],
            vec![circle(-4.0, 0.0, 1.2, 0.0, 64)],
            vec![circle(4.0, 0.0, 1.0, 0.0, 64)],
        ];
        let (result, _) = bridge_nested_levels(&levels, &CompactSpiralParams::default());
        assert!(matches!(
            result.unwrap_err(),
            CompactSpiralRefusal::DoesNotEncircleHub { .. }
        ));
    }

    #[test]
    fn empty_input_refuses() {
        let (result, _) = bridge_nested_levels(&[], &CompactSpiralParams::default());
        assert_eq!(result.unwrap_err(), CompactSpiralRefusal::NoLevels);
    }
}
