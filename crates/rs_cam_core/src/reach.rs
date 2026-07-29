//! The canonical valley-reach policy: how wide a band around a rest
//! centreline can this cutter actually work, and how many offset passes fit.
//!
//! This module is the ONE implementation of that question. Before it existed
//! four production sites answered it with `cutter.radius()` — the ENVELOPE,
//! 3.0 mm on the shipped Ø1-tip / 7° / Ø6-shank taper, a number nothing at
//! finishing depth is anywhere near. `planning/review_2026-07-29/`
//! `CHECKPOINT_A_EVIDENCE.md` scored five candidate models on an analytic
//! valley matrix (176 cells × 3 tools) and the human review gate approved the
//! winner:
//!
//! | model | gouge / miss / coverage on the shipped taper |
//! |---|---|
//! | envelope (shipped) | 1 / 82 / **2 %** |
//! | cusp | 13 / 0 / 100 % |
//! | engagement at depth | 9 / 0 / 100 % |
//! | profile clearance, vertical-slot reading | 9 / 0 / 98 % |
//! | **profile clearance + local wall angle (this module)** | **0 / 0 / 100 %** |
//!
//! Ball tools migrate too (approved ruling — the matrix is the plan's
//! "separately justified correction": envelope/cusp score 11 gouge / 35 miss /
//! 56 float-blind / 74 % on the Ø3 ball control, this model scores 0/0/0/100).
//!
//! # The model
//!
//! Locally, a rest valley is a V: the centreline sits at the apex, and on each
//! side the surface rises to a rim `rim_distance_mm` away, `wall_rise_mm`
//! higher. Write `cot θ = rim_distance / wall_rise` for that side's wall run
//! per unit rise. A cutter whose tip is `δ` below the rim can stand at most
//!
//! ```text
//!     X = rim_distance − G(δ, cot θ)
//!     G(δ, cot θ) = max over u ∈ [0, δ] of [ width_at_height(u) + (δ − u)·cot θ ]
//! ```
//!
//! laterally from the apex before its profile fouls that wall. `G` is the
//! wall's horizontal position at the tip's own level, seen through the tool's
//! profile: at each height `u` above the tip the wall has run back `(δ−u)·cot θ`
//! and the tool is `width_at_height(u)` wide, and the binding height is
//! whichever of those trades worst.
//!
//! Two properties make this the right shape rather than one more scalar:
//!
//! * **It degenerates correctly.** A vertical wall (`cot θ = 0`) gives
//!   `G = width_at_height(δ)` — exactly [`MillingCutter::engagement_radius_mm`],
//!   i.e. the `CLR` candidate. So the "no wall angle available" fallback is
//!   not a second code path, it is this one function called with `cot θ = 0`.
//! * **It sees two-wall fouling.** The reach toward one wall is bounded below
//!   by the *other* wall's intrusion; when `X_left + X_right < 0` the cutter
//!   wedges between the walls and cannot hold `δ` on the apex at all. That is
//!   the tip-float case ([`crate::compute::config::TipFloatFinding`], up to
//!   5.248 mm of residual on the matrix's taper cells), and it is the reason
//!   `CLR` still scored 3 float-blind cells where this model scores 0.
//!
//! The `height_at_radius(w) == None` reading the oracle originally proposed
//! ("the valley is wider than the whole cutter, so refuse") is **not** used
//! here, and deliberately: it cost the ball control 95 points of fit coverage
//! (`CHECKPOINT_A_EVIDENCE.md` §10). A rim wider than the envelope means the
//! rim does not constrain the tool; the clearing decision comes from the
//! coverage criterion below, never from a `None`.
//!
//! # Routing
//!
//! The pencil/clearing decision is a COVERAGE question, not a tool-scale
//! question:
//!
//! ```text
//!     pencil  ⟺  X_reach ≤ cap × offset_stepover
//! ```
//!
//! — can a centreline plus the offsets this operation is permitted to emit
//! actually cover the reachable band? It replaces
//! `half_width ≤ route_width_factor × radius`, which fed the SAME scalar to
//! the routing rule and the fit equation: shrinking that scalar to a
//! depth-aware value fixes the fan and simultaneously routes 64 of 176 truth-
//! pencil cells to clearing, giving the whole coverage gain back
//! (`CHECKPOINT_A_EVIDENCE.md` §8.3). The two decisions must move together,
//! which is why they live in one module.
//!
//! See [`coverage_cap_passes`] for how `cap` is chosen.
//!
//! # Conservatism
//!
//! `G` is evaluated by scanning `u`. `width_at_height` is monotone
//! non-decreasing and `(δ−u)·cot θ` is decreasing with slope `cot θ`, so on
//! any sample interval of width `Δ` the true value exceeds the sampled one by
//! at most `cot θ · Δ`. [`profile_rise`] adds that bound, so `G` is never
//! understated, so `X` is never overstated, so the pass count and the
//! refusal are both conservative — the direction the plan's acceptance bar
//! asks for. The scan is sized to keep that bound at
//! [`REACH_SCAN_RESOLUTION_MM`].
//!
//! # Known limitation (recorded, not hidden)
//!
//! The V is a *model* of the local cross-section. A trapezoidal groove — flat
//! floor, steep walls — is not a V, and reading a single wall angle for it
//! either understates the wall (secant from the apex to the rim) or ignores
//! the floor (steepest local gradient). Production takes the steepest local
//! gradient inside the depth band, which is the conservative-on-refusal,
//! optimistic-on-width choice. Solving the erosion directly against the
//! sampled `RestGrid::surface_z` cross-section would need no wall angle at
//! all and would handle trapezoids, curvature and asymmetry natively; it is
//! a strict generalisation of this function (this is its V special case) and
//! is the natural wave-B follow-up.

use crate::tool::MillingCutter;

/// Lateral resolution (mm) the `G` scan is sized to hold. See the module
/// doc's conservatism note — this is the width of the one-sided error bound
/// that gets ADDED to `G`, not a tolerance that is ignored.
pub const REACH_SCAN_RESOLUTION_MM: f64 = 0.002;

/// Hard ceiling on the `G` scan so a pathological (very shallow wall, very
/// deep rest) sample cannot make the detector quadratic. When it binds the
/// error bound grows above [`REACH_SCAN_RESOLUTION_MM`] — still added, still
/// conservative, just coarser.
const REACH_SCAN_MAX_STEPS: usize = 4096;

/// Wall gradients below this (≈ 0.6°) are read as "no wall on this side"
/// rather than as an almost-horizontal wall.
///
/// `cot θ = 1/slope` blows up as the slope goes to zero, and a literally flat
/// surface has no wall to foul — the two readings are on opposite ends of the
/// same limit, so the limit has to be named. Below the floor the side is
/// treated as VERTICAL (`cot θ = 0`), which is the "the rim does not
/// constrain the tool" reading, not the "refuse everything" one.
pub const WALL_SLOPE_FLOOR: f64 = 0.01;

/// One side of the local valley cross-section, as measured.
///
/// Constructed from what the rest field actually has: a distance to the rim
/// and a rise over it. [`Self::from_wall_angle`] is the analytic form the
/// Checkpoint A matrix drives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValleySide {
    /// Horizontal distance (mm) from the centreline to this side's rim.
    pub rim_distance_mm: f64,
    /// How far (mm) the surface rises over that distance. `f64::INFINITY` (or
    /// any non-positive value) reads as a vertical wall — see
    /// [`WALL_SLOPE_FLOOR`].
    pub wall_rise_mm: f64,
}

impl ValleySide {
    /// A side measured as a distance and a rise.
    #[must_use]
    pub fn new(rim_distance_mm: f64, wall_rise_mm: f64) -> Self {
        Self {
            rim_distance_mm: rim_distance_mm.max(0.0),
            wall_rise_mm,
        }
    }

    /// A side described by a wall angle from HORIZONTAL (radians) — the form
    /// the analytic valley matrix uses. `rise = distance · tan θ`.
    #[must_use]
    pub fn from_wall_angle(rim_distance_mm: f64, wall_angle_rad: f64) -> Self {
        Self::new(rim_distance_mm, rim_distance_mm * wall_angle_rad.tan())
    }

    /// A side whose wall is vertical: the rim never constrains the profile,
    /// only the tool's own engaged width does. This is the `CLR` reading and
    /// the fallback when no wall angle could be measured.
    #[must_use]
    pub fn vertical(rim_distance_mm: f64) -> Self {
        Self::new(rim_distance_mm, f64::INFINITY)
    }

    /// The wall's horizontal run per unit rise (`cot θ`). Zero means vertical
    /// — including the flat and the non-finite readings, per
    /// [`WALL_SLOPE_FLOOR`].
    #[must_use]
    pub fn wall_run(&self) -> f64 {
        if !self.wall_rise_mm.is_finite() || self.wall_rise_mm <= 0.0 {
            return 0.0;
        }
        let slope = self.wall_rise_mm / self.rim_distance_mm.max(1e-9);
        if slope < WALL_SLOPE_FLOOR {
            return 0.0;
        }
        1.0 / slope
    }

    /// The wall angle from horizontal (radians) this side reads as. `π/2` for
    /// every vertical / flat / non-finite case.
    #[must_use]
    pub fn wall_angle_rad(&self) -> f64 {
        let run = self.wall_run();
        if run <= 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            (1.0 / run).atan()
        }
    }

    /// The apex depth (mm) this side's V implies. The reach solve cannot ask
    /// for more depth than the side's own wall provides.
    #[must_use]
    pub fn implied_depth_mm(&self) -> f64 {
        if self.wall_rise_mm.is_finite() && self.wall_run() > 0.0 {
            self.wall_rise_mm
        } else {
            f64::INFINITY
        }
    }
}

/// The local valley cross-section at ONE point on a centreline.
///
/// Per-point rather than per-branch by ruling: a branch 0.2 mm deep at one
/// end and 2 mm at the other has no honest single answer, and
/// `engagement_radius` is monotone in depth so any per-branch scalar is
/// either optimistic (median) or suppresses reachable detail (peak).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalValley {
    /// Rest depth (mm) at this point — how much deeper than the reference the
    /// cutter has to reach.
    pub rest_depth_mm: f64,
    pub left: ValleySide,
    pub right: ValleySide,
}

/// What the policy resolved for one point: how far the cutter can stand from
/// the centreline on each side, and whether it can stand there at all.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Reach {
    /// Max lateral offset (mm) toward the left rim holding the local depth.
    pub left_mm: f64,
    /// Max lateral offset (mm) toward the right rim holding the local depth.
    pub right_mm: f64,
    /// The cutter wedges between the two walls and cannot hold the local
    /// depth on the centreline itself — the tip-float case.
    pub refused: bool,
}

impl Reach {
    /// The narrow side — the scalar the routing criterion compares.
    #[must_use]
    pub fn min_mm(&self) -> f64 {
        self.left_mm.min(self.right_mm)
    }

    /// The wide side.
    #[must_use]
    pub fn max_mm(&self) -> f64 {
        self.left_mm.max(self.right_mm)
    }
}

/// Where a branch should be machined from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingVerdict {
    /// A pencil centreline (plus its fan) covers the reachable band.
    Pencil,
    /// The reachable band is wider than the fan this operation can emit —
    /// hand it to a clearing strategy.
    Clearing,
    /// The cutter cannot hold the local depth here at all. Not a pencil job;
    /// emitting a centreline would drive the tool along material it floats
    /// above.
    Refused,
}

/// `G(δ, cot θ)` — the wall's horizontal position at the tip's own level,
/// seen through the cutter's profile. See the module doc.
///
/// Never understated: the returned value carries the one-sided sampling bound
/// `cot θ · Δ` already added.
#[must_use]
pub fn profile_rise(cutter: &dyn MillingCutter, wall_run: f64, depth_mm: f64) -> f64 {
    let delta = depth_mm.max(0.0);
    if delta <= 0.0 {
        return 0.0;
    }
    if wall_run <= 0.0 {
        // Vertical wall: `(δ−u)·cot θ` vanishes and `width_at_height` is
        // monotone, so the max sits at `u = δ`. This IS the engagement radius,
        // exactly and without a scan.
        return cutter.width_at_height(delta);
    }
    let ideal = (delta * wall_run / REACH_SCAN_RESOLUTION_MM).ceil();
    let steps = if ideal.is_finite() && ideal >= 1.0 {
        (ideal as usize).min(REACH_SCAN_MAX_STEPS)
    } else {
        1
    };
    let step = delta / steps as f64;
    let mut g = f64::NEG_INFINITY;
    for i in 0..=steps {
        let u = (i as f64 * step).min(delta);
        let v = cutter.width_at_height(u) + (delta - u) * wall_run;
        if v > g {
            g = v;
        }
    }
    g + wall_run * step
}

/// Solve the reach at one point. The single entry point every routing and fit
/// decision in the finishing stack goes through.
#[must_use]
pub fn solve_reach(cutter: &dyn MillingCutter, valley: &LocalValley) -> Reach {
    let depth = valley.rest_depth_mm.max(0.0);
    let side = |s: &ValleySide| -> f64 {
        // A side's own wall cannot be asked for more depth than it has: the V
        // bottoms out at `wall_rise`, and beyond that the model has nothing to
        // say (the matrix marks those cells N/A).
        let d = depth.min(s.implied_depth_mm());
        s.rim_distance_mm - profile_rise(cutter, s.wall_run(), d)
    };
    let x_left = side(&valley.left);
    let x_right = side(&valley.right);
    // Two-wall fouling: reach toward one wall is bounded below by the other
    // wall's intrusion, so the pair is inconsistent exactly when the sum goes
    // negative. Symmetric in the two sides by construction.
    Reach {
        left_mm: x_left.max(0.0),
        right_mm: x_right.max(0.0),
        refused: x_left + x_right < 0.0,
    }
}

/// The half-band ONE pass works at `depth_mm`: the cutter's own engaged
/// half-width there, floored at the cusp radius.
///
/// The floor is not cosmetic — every ball-tipped shape reports zero engaged
/// width at zero depth, and a zero here collapses both consumers below
/// ([`coverage_cap_passes`] to a zero cap, [`suggested_offset_stepover_mm`]
/// to a zero stepover). This is the ONE expression of "how wide is this
/// cutter, here"; both consumers call it rather than restating it.
#[must_use]
pub fn working_half_width_mm(cutter: &dyn MillingCutter, depth_mm: f64) -> f64 {
    cutter
        .engagement_radius_mm(depth_mm.max(0.0))
        .max(cutter.cusp_radius_mm())
}

/// Fraction of [`working_half_width_mm`] the policy sizes an offset stepover
/// at — 50 % of the band one pass works, i.e. a half-width overlap between
/// neighbouring passes in the fan.
pub const SUGGESTED_STEPOVER_OVERLAP: f64 = 0.5;

/// The offset stepover (mm) this policy sizes for a fan working at
/// `depth_mm`.
///
/// # Why the policy owns this number
///
/// Before PR-6a the one remaining routing/fit scalar in the finishing stack
/// was `UnifiedFinish`'s `cutter.envelope_radius_mm() * 0.5` — 1.5 mm on the
/// shipped Ø1-tip / 7° / Ø6-shank taper, i.e. half the SHANK, three times
/// wider than the whole tip. A fan spaced on the shank cannot describe passes
/// a Ø1 tip cuts, and it fed the routing criterion (`cap × stepover`) as well
/// as the emission, so it overstated both sides of the coverage question at
/// once.
///
/// The replacement is deliberately the same SHAPE — a fixed fraction of a
/// radius — with the envelope radius swapped for the radius the tool actually
/// works with at this depth. It is therefore consistent with
/// [`coverage_cap_passes`] by construction: at this stepover the cap floor is
/// exactly `ceil(1 / SUGGESTED_STEPOVER_OVERLAP) = 2` passes, so the
/// centreline plus one offset per side covers the band the centreline pass
/// itself works, and no more.
///
/// # Conservatism
///
/// `width_at_height` is monotone non-decreasing and saturates at the envelope
/// radius, so `working_half_width_mm ≤ envelope_radius_mm` for every cutter at
/// every depth: this value is never COARSER than the number it replaces, only
/// equal or finer. Finer is the safe direction — [`offset_passes_per_side`]
/// FLOORS `reach / stepover`, so a finer stepover can only place passes the
/// reach solve already said the cutter can hold, and the routing threshold
/// `cap × stepover` quantises the reachable band more tightly instead of
/// rounding it up. `suggested_stepover_is_never_coarser_than_the_envelope_rule`
/// asserts the bound rather than assuming it.
///
/// For a plain ball the two are numerically IDENTICAL (its cusp radius is its
/// envelope radius), so the migration moves tapered tools only.
#[must_use]
pub fn suggested_offset_stepover_mm(cutter: &dyn MillingCutter, depth_mm: f64) -> f64 {
    working_half_width_mm(cutter, depth_mm) * SUGGESTED_STEPOVER_OVERLAP
}

/// The `cap` in `X_reach ≤ cap × offset_stepover`.
///
/// # Derivation
///
/// The fan an operation can emit is a centreline plus `num_offset_passes`
/// offsets per side at `offset_stepover` each, so it works a half-band of
/// `num_offset_passes × offset_stepover`. Comparing the reachable band
/// against exactly that is the coverage question, and it is what makes the
/// matrix's routing column score 0 over-claims and 0 under-claims: the
/// matrix's own ground truth calls a cell "clearing" iff
/// `X_true > cap · stepover` with `cap = num_offset_passes`.
///
/// Two floors are applied on top, and only ever UPWARD, so the matrix is
/// reproduced exactly wherever they do not bind:
///
/// 1. **The centreline pass has a width of its own.** Even at
///    `num_offset_passes = 0` — the shipped `PencilParams` default — the
///    single centreline pass works a band of `engagement_radius_mm(δ)`
///    (floored at `cusp_radius_mm()`, since every ball-tipped shape reports
///    zero engaged width at zero depth). Without this floor `cap × stepover`
///    is literally `0` at the default dial and EVERY branch routes to
///    clearing — the pencil operation would stop cutting.
/// 2. **At least one stepover.** A degenerate `engagement ≈ 0` must not
///    reintroduce the same collapse.
///
/// On every cell of the Checkpoint A matrix (`num_offset_passes = 4`,
/// `stepover = 0.5`) the floor is 3 or less on the Ø3 ball and 2 or less on
/// both tapers, so it never binds and the approved routing column is
/// reproduced unchanged. That is asserted, not assumed, by
/// `checkpoint_a_valley_matrix::coverage_cap_floor_never_binds_on_the_matrix`.
#[must_use]
pub fn coverage_cap_passes(
    cutter: &dyn MillingCutter,
    depth_mm: f64,
    offset_stepover_mm: f64,
    num_offset_passes: usize,
) -> usize {
    let stepover = offset_stepover_mm.max(1e-6);
    let own_width = working_half_width_mm(cutter, depth_mm);
    let floor = ((own_width / stepover).ceil().max(1.0) as usize).max(1);
    num_offset_passes.max(floor)
}

/// The coverage routing criterion. `cap_passes` comes from
/// [`coverage_cap_passes`].
#[must_use]
pub fn route(reach: &Reach, offset_stepover_mm: f64, cap_passes: usize) -> RoutingVerdict {
    if reach.refused {
        return RoutingVerdict::Refused;
    }
    if reach.min_mm() <= cap_passes as f64 * offset_stepover_mm.max(1e-6) {
        RoutingVerdict::Pencil
    } else {
        RoutingVerdict::Clearing
    }
}

/// Offset passes that physically fit on each side, before the operation's own
/// cap is applied.
///
/// `floor`, not `round`: a pass at an offset the cutter cannot hold is the
/// gouge class the whole model exists to eliminate, and rounding up half a
/// stepover is exactly how the shipped equation produced them.
#[must_use]
pub fn offset_passes_per_side(
    reach: &Reach,
    offset_stepover_mm: f64,
    cap: usize,
) -> (usize, usize) {
    if reach.refused {
        return (0, 0);
    }
    let stepover = offset_stepover_mm.max(1e-6);
    let count = |x: f64| -> usize {
        let n = (x / stepover).floor();
        if n.is_finite() && n > 0.0 {
            (n as usize).min(cap)
        } else {
            0
        }
    };
    (count(reach.left_mm), count(reach.right_mm))
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
    use crate::tool::{BallEndmill, TaperedBallEndmill};

    fn taper() -> TaperedBallEndmill {
        TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
    }

    /// A vertical wall must reproduce `engagement_radius_mm` EXACTLY — the
    /// `CLR` candidate is this function's degenerate case, not a second
    /// implementation. If this ever drifts, the module has two models again.
    #[test]
    fn vertical_wall_reproduces_the_engagement_radius() {
        let t = taper();
        for depth in [0.0, 0.05, 0.2, 0.4391, 1.0, 2.0, 5.0] {
            let g = profile_rise(&t, 0.0, depth);
            assert!(
                (g - t.engagement_radius_mm(depth)).abs() < 1e-12,
                "depth {depth}: G={g} vs engagement={}",
                t.engagement_radius_mm(depth)
            );
        }
    }

    /// The scan is conservative in the stated direction: it never reports a
    /// smaller `G` than a much finer scan would.
    #[test]
    fn the_scan_never_understates_the_profile_rise() {
        let t = taper();
        for &run in &[0.1_f64, 0.577, 1.0, 1.732] {
            for &depth in &[0.1_f64, 0.5, 1.0, 2.0] {
                let coarse = profile_rise(&t, run, depth);
                // Brute force at 100× the scan resolution.
                let n = 200_000;
                let mut fine = f64::NEG_INFINITY;
                for i in 0..=n {
                    let u = depth * i as f64 / n as f64;
                    fine = fine.max(t.width_at_height(u) + (depth - u) * run);
                }
                assert!(
                    coarse >= fine - 1e-12,
                    "run {run} depth {depth}: coarse {coarse} < fine {fine}"
                );
                assert!(
                    coarse - fine < 0.01,
                    "run {run} depth {depth}: bound too loose ({} mm)",
                    coarse - fine
                );
            }
        }
    }

    /// Envelope, cusp and reach give DIFFERENT answers on the shipped taper —
    /// the differential the whole change rests on. A 1.5 mm-half-width valley
    /// with 70° walls at 0.6 mm rest depth: the envelope radius (3.0) exceeds
    /// the whole valley, the cusp radius (0.5) is depth-blind, the reach
    /// solve sits between them.
    #[test]
    fn envelope_cusp_and_reach_disagree_on_a_tapered_valley() {
        let t = taper();
        let v = LocalValley {
            rest_depth_mm: 0.6,
            left: ValleySide::from_wall_angle(1.5, 70.0_f64.to_radians()),
            right: ValleySide::from_wall_angle(1.5, 70.0_f64.to_radians()),
        };
        let reach = solve_reach(&t, &v);
        let n_env = ((1.5 - t.envelope_radius_mm()) / 0.5).floor().max(0.0) as usize;
        let n_cusp = ((1.5 - t.cusp_radius_mm()) / 0.5).floor().max(0.0) as usize;
        let (nl, nr) = offset_passes_per_side(&reach, 0.5, 4);
        assert_eq!(n_env, 0, "envelope baseline is supposed to emit nothing");
        assert_eq!(n_cusp, 2);
        assert_eq!(nl, nr, "symmetric valley must give a symmetric fan");
        assert!(
            nl != n_env && nl != n_cusp,
            "reach ({nl}) coincides with envelope ({n_env}) or cusp ({n_cusp}) \
             — the fixture is not discriminating"
        );
    }

    /// A Ø3 ball wedged in a narrow 60° V cannot hold the commanded depth on
    /// the apex — the two-wall fouling case, and the reason the vertical-slot
    /// reading still scored float-blind cells.
    #[test]
    fn two_wall_fouling_refuses_instead_of_routing_a_floating_centreline() {
        let ball = BallEndmill::new(3.0, 25.0);
        let narrow = LocalValley {
            rest_depth_mm: 4.39,
            left: ValleySide::from_wall_angle(3.0, 60.0_f64.to_radians()),
            right: ValleySide::from_wall_angle(3.0, 60.0_f64.to_radians()),
        };
        let r = solve_reach(&ball, &narrow);
        assert!(r.refused, "expected two-wall fouling, got {r:?}");
        assert_eq!(route(&r, 0.5, 4), RoutingVerdict::Refused);
        assert_eq!(offset_passes_per_side(&r, 0.5, 4), (0, 0));

        // The SAME valley at a depth the ball can hold is not refused.
        let shallow = LocalValley {
            rest_depth_mm: 0.2,
            ..narrow
        };
        assert!(!solve_reach(&ball, &shallow).refused);
    }

    /// Per-side asymmetry survives: a valley with a shallow left wall and a
    /// steep right wall must not be collapsed to one scalar.
    #[test]
    fn asymmetric_walls_produce_an_asymmetric_fan() {
        let t = taper();
        let v = LocalValley {
            rest_depth_mm: 0.6,
            left: ValleySide::from_wall_angle(3.0, 30.0_f64.to_radians()),
            right: ValleySide::from_wall_angle(3.0, 85.0_f64.to_radians()),
        };
        let r = solve_reach(&t, &v);
        assert!(
            r.right_mm > r.left_mm + 0.5,
            "steep wall must leave more room than a shallow one: {r:?}"
        );
        let (nl, nr) = offset_passes_per_side(&r, 0.5, 8);
        assert!(nr > nl, "asymmetric reach collapsed to a symmetric fan");
    }

    /// The coverage cap must never be zero, whatever the dials say — the
    /// shipped `num_offset_passes` default IS zero, and a zero cap routes
    /// every branch to clearing.
    #[test]
    fn coverage_cap_never_collapses_to_zero() {
        let t = taper();
        for depth in [0.0, 0.05, 1.0, 3.0] {
            for passes in [0usize, 1, 4] {
                let cap = coverage_cap_passes(&t, depth, 0.5, passes);
                assert!(cap >= 1, "cap collapsed at depth {depth}, passes {passes}");
                assert!(cap >= passes, "cap must never be below the user's dial");
            }
        }
        // Ball control: the floor is the ball radius over the stepover.
        let ball = BallEndmill::new(3.0, 25.0);
        assert_eq!(coverage_cap_passes(&ball, 2.0, 0.5, 0), 3);
    }

    /// PR-6a (H2.3): the derived stepover is never COARSER than the envelope
    /// rule it replaces — `width_at_height` saturates at the envelope radius,
    /// so the bound holds at every depth, and finer is the conservative
    /// direction (see [`suggested_offset_stepover_mm`]).
    #[test]
    fn suggested_stepover_is_never_coarser_than_the_envelope_rule() {
        let t = taper();
        let ball = BallEndmill::new(6.0, 25.0);
        for depth in [0.0, 0.01, 0.05, 0.2, 0.6, 1.2, 3.0, 8.0, 40.0] {
            for cutter in [&t as &dyn MillingCutter, &ball as &dyn MillingCutter] {
                let derived = suggested_offset_stepover_mm(cutter, depth);
                let envelope_rule = cutter.envelope_radius_mm() * 0.5;
                assert!(
                    derived > 0.0,
                    "a zero stepover collapses the fan (depth {depth})"
                );
                assert!(
                    derived <= envelope_rule + 1e-12,
                    "depth {depth}: derived {derived} coarser than the retired \
                     envelope rule {envelope_rule}"
                );
            }
        }
    }

    /// The shipped taper is the discriminating case: the envelope rule sizes
    /// the fan off the Ø6 SHANK (1.5 mm), the policy off the Ø1 tip.
    #[test]
    fn the_derived_stepover_moves_the_taper_and_leaves_the_ball_alone() {
        let t = taper();
        // Shallow rest — the cusp floor binds: half the Ø1 tip radius.
        assert!((suggested_offset_stepover_mm(&t, 0.05) - 0.25).abs() < 1e-12);
        assert!((t.envelope_radius_mm() * 0.5 - 1.5).abs() < 1e-12);
        // Deeper rest engages more of the cone, so the stepover grows —
        // monotonically, and never past the envelope rule.
        let deep = suggested_offset_stepover_mm(&t, 2.0);
        assert!(deep > 0.25 && deep < 1.5, "got {deep}");

        // A plain ball's cusp radius IS its envelope radius: identity.
        let ball = BallEndmill::new(6.0, 25.0);
        for depth in [0.0, 0.3, 3.0, 12.0] {
            assert!(
                (suggested_offset_stepover_mm(&ball, depth) - ball.envelope_radius_mm() * 0.5)
                    .abs()
                    < 1e-12,
                "the ball must not move at depth {depth}"
            );
        }
    }

    /// Routing and emission agree by construction at the derived stepover:
    /// the coverage cap floor is exactly 2 passes, whatever the tool or the
    /// depth. If [`SUGGESTED_STEPOVER_OVERLAP`] and the floor in
    /// [`coverage_cap_passes`] ever drift apart, this fails.
    #[test]
    fn the_derived_stepover_puts_the_coverage_cap_floor_at_two() {
        let t = taper();
        let ball = BallEndmill::new(3.0, 25.0);
        for cutter in [&t as &dyn MillingCutter, &ball as &dyn MillingCutter] {
            for depth in [0.0, 0.05, 0.6, 2.0, 5.0] {
                let s = suggested_offset_stepover_mm(cutter, depth);
                assert_eq!(
                    coverage_cap_passes(cutter, depth, s, 0),
                    2,
                    "depth {depth}, stepover {s}"
                );
            }
        }
    }
}
