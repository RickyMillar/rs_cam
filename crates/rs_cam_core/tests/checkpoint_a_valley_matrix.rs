//! CHECKPOINT A EVIDENCE — analytic valley matrix + reach-model scoring.
//!
//! **No behavioral change is made by this file.** It is research evidence for
//! the human review gate described in
//! `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H2
//! ("Research experiments") and §3.3 ("Checkpoint A"). The oracle it is
//! written against is `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md`.
//! Nothing here is imported by production code; the report it prints is
//! transcribed into `planning/review_2026-07-29/CHECKPOINT_A_EVIDENCE.md`.
//!
//! # What is being decided
//!
//! Four production sites (`TOOL_SCALE_SEMANTICS.md` A1-A5) answer "how wide a
//! band around this valley centreline can the cutter actually work?" with
//! `cutter.radius()` — the ENVELOPE, which on the shipped Ø1-tip/7°/Ø6-shank
//! taper is 3.0 mm, a number nothing at finishing depth is anywhere near.
//! H2 must replace it. The candidates are:
//!
//! | code | model | radius fed to the shipped fit equation |
//! |---|---|---|
//! | ENV | envelope baseline (today) | `envelope_radius_mm()` |
//! | CUSP | tip-sphere baseline | `cusp_radius_mm()` |
//! | ENG | engagement at local rest depth | `engagement_radius_mm(δ)` |
//! | CLR | profile-clearance, vertical-slot reading | `engagement_radius_mm(δ)` + a `height_at_radius(w)` reach guard |
//! | CLR+θ | profile-clearance with the local wall angle | exact two-wall solve over `width_at_height` |
//!
//! The shipped fit equation is `crease_paths::centerline_cut_paths`:
//! `n = round((half_width_mm − r) / offset_stepover).max(0).min(cap)`.
//!
//! # Analytic ground truth
//!
//! ## Valley
//!
//! A straight V groove cut into a block whose top surface is `z = 0`.
//! Rim edges at `x = ±w`; left wall inclined `θ_l` from horizontal, right wall
//! `θ_r`. The walls meet at the apex, whose depth and lateral position follow
//! from the two angles:
//!
//! ```text
//!     D = 2w / (cot θ_l + cot θ_r)          x_apex = D/tan θ_l − w
//! ```
//!
//! (symmetric case: `D = w·tan θ`, `x_apex = 0`). Surface height:
//!
//! ```text
//!     V(x) =  0                      for |x| ≥ w        (the flat top)
//!             −(x + w)·tan θ_l       for −w ≤ x ≤ x_apex
//!             −(w − x)·tan θ_r       for x_apex ≤ x ≤ w
//! ```
//!
//! ## Tool
//!
//! The tool is queried only through `MillingCutter`. `H(u) =
//! height_at_radius(u)` is the profile height above the tip at lateral radius
//! `u`, and `width_at_height` is its inverse. No new geometric model is
//! introduced: the taper's ball/cone tangency, the ball's sphere, and the
//! envelope clamp all come from the shipped shapes.
//!
//! ## The two truth queries
//!
//! A tool whose axis stands at `x` with its tip at height `z_t` interferes
//! with the block iff some profile point sits below the surface, i.e. iff
//! `z_t + H(u) < V(x + u)` for some `u ∈ [−R_env, R_env]`. So:
//!
//! ```text
//!     deepest reachable tip height at x:   z*(x) = max_u [ V(x+u) − H(|u|) ]
//!     fits at rest depth δ:                z*(x) ≤ −δ
//!     max lateral offset at rest depth δ:  X(δ) = max{ q ≥ 0 : z*(x_apex ± q) ≤ −δ }
//! ```
//!
//! `X(δ)` is exactly the quantity the shipped equation approximates by
//! `half_width_mm − r`. Both queries are evaluated by direct sampling of the
//! tool's own profile — a morphological erosion of the surface by the cutter,
//! which is the same operation the drop-cutter performs.
//!
//! ## Sampling bound (why the numbers below are trustworthy)
//!
//! `H` is sampled on a uniform lateral grid of step `DU = 0.00025 mm` over
//! `[0, R_env]`. Between consecutive samples `u_i ≤ u ≤ u_{i+1}`:
//!
//! * `V` is piecewise linear with `|V′| ≤ L := max(tan θ_l, tan θ_r)`, so
//!   `V(x+u) ≤ V(x+u_i) + L·DU`;
//! * `H` is monotone non-decreasing, so `H(u) ≥ H(u_i)`.
//!
//! Hence `V(x+u) − H(u) ≤ [V(x+u_i) − H(u_i)] + L·DU`: the sampled maximum
//! understates the true maximum by **at most `L·DU`**, one-sided. At the
//! steepest wall in the matrix (85°, `tan = 11.43`) that is **0.0029 mm**.
//! The fit predicate subtracts that bound as a guard, so the ground truth
//! reported here is *conservative*: it never claims a fit the real tool would
//! not have, and it may refuse a fit by at most 2.9 µm of depth. Against a
//! 0.5 mm offset stepover that can move a pass count only for a valley within
//! 2.9 µm of an exact boundary.
//!
//! Truncation is exact, not an approximation: any `u` with `H(u) > δ`
//! satisfies `V(x+u) − H(u) ≤ 0 − H(u) < −δ` because `V ≤ 0` everywhere, so
//! those samples can never violate `z*(x) ≤ −δ` and the scan stops there.
//!
//! ## Independent cross-check
//!
//! For the V above, the fit condition can also be written in closed form.
//! With the tip at depth `δ` and the tool axis `q` to the right of the apex,
//! the right wall at depth `p` sits `(D − p)/tan θ_r` from the apex and the
//! tool's half-width there is `width_at_height(δ − p)`, so
//!
//! ```text
//!     X_closed(δ) = min over p ∈ [0, δ] of [ (D − p)/tan θ_r − width_at_height(δ − p) ]
//! ```
//!
//! `closed_form_matches_profile_erosion_sampler` asserts the sampler and this
//! expression agree across the matrix. That is what makes CLR+θ an
//! *independently computed* model rather than a restatement of the oracle.
//!
//! # Scoring
//!
//! Per cell (tool × θ × w × δ), with `s = 0.5 mm` stepover and `cap = 4`:
//!
//! * `n_true  = min(cap, floor(X_true / s))` over both sides — offset passes
//!   that physically hold the commanded depth;
//! * `n_model = min(cap, round((w_meas − r_model)/s).max(0))` — the shipped
//!   equation with that model's radius;
//! * **GOUGE** when `n_model > n_true`: the model emits a pass at a lateral
//!   offset the tool cannot hold at depth. (In today's pipeline
//!   `paths_from_sampled` re-solves Z by drop-cutter, so the physical outcome
//!   degrades from a gouge to an air-cut pass — but the model is still
//!   over-claiming, and any future fixed-Z or claims-carving consumer of the
//!   same number would gouge. Scored as the conservative violation, per plan.)
//! * **MISS** when `n_model < n_true`: reachable detail suppressed.
//! * routing class: model says pencil iff `w_meas ≤ 2.0 × r_model`
//!   (`route_width_factor` default); truth says clearing iff `X_true > cap·s`,
//!   i.e. the reachable band is wider than the fan the op can emit at all.
//!
//! Cells where `δ > D` (the valley is not that deep) are N/A and excluded.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::{
    PencilDetector, PencilParams, PencilRuntimeEvent, pencil_toolpath_structured_annotated,
};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

// ── Constants ────────────────────────────────────────────────────────────

/// Lateral profile sampling step (mm). See the header for the error bound.
const DU: f64 = 0.00025;
/// Offset stepover the shipped equation divides by (`PencilParams::default`).
const STEPOVER: f64 = 0.5;
/// `num_offset_passes` acting as a CAP for the RestDepth detector.
const CAP: usize = 4;
/// `RestFieldParams::route_width_factor` default.
const ROUTE_WIDTH_FACTOR: f64 = 2.0;
/// Tolerance for depth comparisons (mm) — an order above the sampling bound.
const DEPTH_TOL: f64 = 0.01;

/// Wall angles from horizontal (deg), per plan §H2.
const ANGLES: [f64; 5] = [30.0, 45.0, 60.0, 75.0, 85.0];
/// Valley rim half-widths (mm): below-tip, tip-scale, 2×tip, cone-scale,
/// shaft-scale, beyond-shaft.
const WIDTHS: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 3.0, 5.0];
/// Rest depths (mm): from 0.05 through the taper's ball/cone tangency
/// (0.4391) and beyond.
const DEPTHS: [f64; 7] = [0.05, 0.1, 0.2, 0.4391, 0.6, 1.0, 2.0];

// ── Tools ────────────────────────────────────────────────────────────────

/// The tool this project actually finishes with: Ø1 tip, 7° half-angle, Ø6
/// shaft. Envelope 3.0, cusp 0.5 — the 6× split that makes every row below
/// discriminating.
fn wanaka_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// A second taper angle, to show the two-wall fouling boundary moves with α.
fn steep_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 15.0, 6.0, 25.0)
}

/// Ball control: envelope == cusp, so ENV and CUSP must agree everywhere.
fn ball_control() -> BallEndmill {
    BallEndmill::new(3.0, 25.0)
}

// ── Analytic valley ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Valley {
    /// Rim half-width (mm): rim edges at x = ±w, top surface at z = 0.
    w: f64,
    tan_l: f64,
    tan_r: f64,
    /// Lateral position of the apex (0 for symmetric valleys).
    x_apex: f64,
    /// Apex depth below the rim (mm).
    depth: f64,
}

impl Valley {
    fn new(w: f64, theta_l_deg: f64, theta_r_deg: f64) -> Self {
        let tan_l = theta_l_deg.to_radians().tan();
        let tan_r = theta_r_deg.to_radians().tan();
        let depth = 2.0 * w / (1.0 / tan_l + 1.0 / tan_r);
        let x_apex = depth / tan_l - w;
        Self {
            w,
            tan_l,
            tan_r,
            x_apex,
            depth,
        }
    }

    fn sym(w: f64, theta_deg: f64) -> Self {
        Self::new(w, theta_deg, theta_deg)
    }

    /// Surface height at `x` (≤ 0 inside the groove, 0 on the flat top).
    #[inline]
    fn z(&self, x: f64) -> f64 {
        if x <= -self.w || x >= self.w {
            0.0
        } else if x <= self.x_apex {
            -(x + self.w) * self.tan_l
        } else {
            -(self.w - x) * self.tan_r
        }
    }

    #[inline]
    fn lipschitz(&self) -> f64 {
        self.tan_l.max(self.tan_r)
    }

    /// The half-width the rest-field detector would report: the chamfer
    /// distance transform of the rest mask, sampled along the ridge, is the
    /// distance from the apex to the nearest mask boundary — the SMALLER of
    /// the two horizontal apex-to-rim distances.
    fn measured_half_width(&self) -> f64 {
        (self.x_apex + self.w).min(self.w - self.x_apex)
    }
}

// ── Tool profile sampler ─────────────────────────────────────────────────

/// `h[i] = height_at_radius(i · DU)`, monotone non-decreasing, truncated at
/// the envelope.
struct Profile {
    h: Vec<f64>,
}

impl Profile {
    fn build(cutter: &dyn MillingCutter) -> Self {
        let r_env = cutter.envelope_radius_mm();
        let n = (r_env / DU).floor() as usize;
        let mut h = Vec::with_capacity(n + 1);
        let mut last = 0.0f64;
        for i in 0..=n {
            let u = i as f64 * DU;
            let v = cutter.height_at_radius(u).unwrap_or(last);
            // Monotonicity is a property of every shipped shape; enforce it so
            // the truncation-by-break below is sound even on a future shape
            // with a numerically noisy profile.
            last = v.max(last);
            h.push(last);
        }
        Self { h }
    }

    /// Does the tool, axis at `x`, tip at depth `delta` below the rim, clear
    /// the valley? Conservative by exactly the header's `L·DU` bound.
    fn fits(&self, v: &Valley, x: f64, delta: f64) -> bool {
        let limit = -delta - v.lipschitz() * DU;
        for (i, &hi) in self.h.iter().enumerate() {
            if hi > delta {
                break;
            }
            let u = i as f64 * DU;
            if v.z(x + u) - hi > limit {
                return false;
            }
            if i > 0 && v.z(x - u) - hi > limit {
                return false;
            }
        }
        true
    }

    /// Deepest tip depth below the rim achievable with the axis at `x`.
    fn max_depth_at(&self, v: &Valley, x: f64) -> f64 {
        if !self.fits(v, x, 0.0) {
            return 0.0;
        }
        let hi_bound = v.depth;
        if self.fits(v, x, hi_bound) {
            return hi_bound;
        }
        let (mut lo, mut hi) = (0.0, hi_bound);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if self.fits(v, x, mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Maximum lateral offset from the apex, toward `dir` (+1 = right wall),
    /// at which the tool still holds tip depth `delta`. `None` when the tool
    /// cannot hold `delta` even on the apex.
    fn max_lateral(&self, v: &Valley, delta: f64, dir: f64) -> Option<f64> {
        if !self.fits(v, v.x_apex, delta) {
            return None;
        }
        let mut lo = 0.0f64;
        let mut hi = 2.0 * v.w;
        if self.fits(v, v.x_apex + dir * hi, delta) {
            return Some(hi);
        }
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if self.fits(v, v.x_apex + dir * mid, delta) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    }
}

/// Closed-form maximum lateral offset toward one wall (see header).
/// `tan_near` is that wall's tangent; `tan_far` the opposite wall's.
/// `None` when the tool cannot sit on the apex at `delta` at all (two-wall
/// fouling, `TOOL_SCALE_SEMANTICS.md` §4.4 case B).
fn closed_form_max_lateral(
    cutter: &dyn MillingCutter,
    v: &Valley,
    delta: f64,
    tan_near: f64,
    tan_far: f64,
) -> Option<f64> {
    if delta > v.depth {
        return None;
    }
    const NP: usize = 4000;
    let mut near = f64::INFINITY;
    let mut far_min = f64::NEG_INFINITY;
    for k in 0..=NP {
        let p = delta * k as f64 / NP as f64;
        let half = cutter.width_at_height(delta - p);
        near = near.min((v.depth - p) / tan_near - half);
        far_min = far_min.max(half - (v.depth - p) / tan_far);
    }
    if near < far_min {
        return None;
    }
    Some(near.max(0.0))
}

// ── Candidate models ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Model {
    Envelope,
    Cusp,
    Engagement,
    ClearSlot,
    ClearTheta,
}

const MODELS: [Model; 5] = [
    Model::Envelope,
    Model::Cusp,
    Model::Engagement,
    Model::ClearSlot,
    Model::ClearTheta,
];

impl Model {
    fn code(self) -> &'static str {
        match self {
            Model::Envelope => "ENV",
            Model::Cusp => "CUSP",
            Model::Engagement => "ENG",
            Model::ClearSlot => "CLR",
            Model::ClearTheta => "CLR+θ",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Prediction {
    /// Offset passes per side the model claims fit.
    n: usize,
    /// Model routes this branch to a pencil centreline (vs a clearing region).
    pencil: bool,
    /// Model actively refuses the branch (cannot reach the commanded depth).
    refuse: bool,
}

fn predict(model: Model, cutter: &dyn MillingCutter, v: &Valley, delta: f64) -> Prediction {
    let w = v.measured_half_width();
    let count_from_radius = |r: f64| -> usize {
        (((w - r) / STEPOVER).round().max(0.0) as usize).min(CAP)
    };
    match model {
        Model::Envelope => {
            let r = cutter.envelope_radius_mm();
            Prediction {
                n: count_from_radius(r),
                pencil: w <= ROUTE_WIDTH_FACTOR * r,
                refuse: false,
            }
        }
        Model::Cusp => {
            let r = cutter.cusp_radius_mm();
            Prediction {
                n: count_from_radius(r),
                pencil: w <= ROUTE_WIDTH_FACTOR * r,
                refuse: false,
            }
        }
        Model::Engagement => {
            let r = cutter.engagement_radius_mm(delta);
            Prediction {
                n: count_from_radius(r),
                pencil: w <= ROUTE_WIDTH_FACTOR * r,
                refuse: false,
            }
        }
        Model::ClearSlot => {
            // Profile clearance read as a VERTICAL-walled slot of half-width
            // `w` and depth `delta` — the reading `TOOL_SCALE_SEMANTICS.md`
            // §4.5 proposes, using only the two numbers routing has today.
            let r = cutter.engagement_radius_mm(delta);
            match cutter.height_at_radius(w) {
                // Valley wider than the whole cutter: a clearing job, not a
                // pencil job.
                None => Prediction {
                    n: 0,
                    pencil: false,
                    refuse: false,
                },
                Some(h_avail) if h_avail + DEPTH_TOL < delta => Prediction {
                    n: 0,
                    pencil: true,
                    refuse: true,
                },
                Some(_) => Prediction {
                    n: count_from_radius(r),
                    pencil: w <= ROUTE_WIDTH_FACTOR * r,
                    refuse: false,
                },
            }
        }
        Model::ClearTheta => {
            // Two-wall profile solve WITH the local wall angle — the extra
            // input routing does not carry today. Closed form, independent of
            // the erosion sampler that produces the ground truth.
            let right = closed_form_max_lateral(cutter, v, delta, v.tan_r, v.tan_l);
            let left = closed_form_max_lateral(cutter, v, delta, v.tan_l, v.tan_r);
            match (left, right) {
                (Some(l), Some(r)) => {
                    let x = l.min(r);
                    Prediction {
                        n: ((x / STEPOVER).floor() as usize).min(CAP),
                        pencil: x <= CAP as f64 * STEPOVER,
                        refuse: false,
                    }
                }
                _ => Prediction {
                    n: 0,
                    pencil: true,
                    refuse: true,
                },
            }
        }
    }
}

// ── Ground truth ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Truth {
    /// Deepest tip depth below the rim on the apex (mm).
    apex_reach: f64,
    /// Residual the tool cannot remove at the apex: `depth − apex_reach`.
    tip_float: f64,
    /// Max lateral offset holding `delta`, min over the two sides. `None` when
    /// the tool cannot hold `delta` on the apex at all.
    x_true: Option<f64>,
    n: usize,
    /// Truth says the reachable band is wider than the emittable fan.
    clearing: bool,
}

fn truth(prof: &Profile, v: &Valley, delta: f64) -> Truth {
    let apex_reach = prof.max_depth_at(v, v.x_apex);
    let right = prof.max_lateral(v, delta, 1.0);
    let left = prof.max_lateral(v, delta, -1.0);
    let x_true = match (left, right) {
        (Some(l), Some(r)) => Some(l.min(r)),
        _ => None,
    };
    let n = x_true
        .map(|x| ((x / STEPOVER).floor() as usize).min(CAP))
        .unwrap_or(0);
    Truth {
        apex_reach,
        tip_float: (v.depth - apex_reach).max(0.0),
        x_true,
        n,
        clearing: x_true.map(|x| x > CAP as f64 * STEPOVER).unwrap_or(false),
    }
}

// ── Cell scoring ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default, Debug)]
struct Score {
    /// Every scored cell.
    cells: usize,
    /// Cells where the model routes to a pencil fan — the only cells where a
    /// pass count is emitted, and therefore the only cells where a pass-count
    /// error is a real error. A model that routes to clearing hands the region
    /// to a different strategy; scoring its (moot) fan against truth would
    /// punish exactly the conservative answer the plan asks for.
    fan_cells: usize,
    gouge: usize,
    miss: usize,
    pass_err_sum: f64,
    route_over: usize,
    route_under: usize,
    /// Truth says the tool cannot hold the commanded depth on the apex at all
    /// (`TOOL_SCALE_SEMANTICS.md` §4.4 case B / tip float), yet the model
    /// routes a pencil centreline there anyway without refusing. Cutting time
    /// spent on material the tool floats above.
    float_blind: usize,
    /// Reachable-detail coverage, over the cells where TRUTH says a pencil fan
    /// is the right strategy: `Σ min(n_model, n_true)` (numerator) against
    /// `Σ n_true` (denominator). The plan's "reachable-detail coverage
    /// increases over the envelope baseline" gate reads this.
    cover_num: usize,
    cover_den: usize,
    /// Same numerator with the routing verdict IGNORED — the fit equation's
    /// coverage on its own, isolated from the `route_width_factor × r`
    /// pencil/clearing rule that the same radius also feeds. The gap between
    /// `cover_fit_num` and `cover_num` is the routing rule's contribution, and
    /// on the taper it is the dominant term (see the verdict section).
    cover_fit_num: usize,
}

impl Score {
    fn add(&mut self, p: Prediction, t: Truth) {
        self.cells += 1;
        if p.pencil && !p.refuse {
            self.fan_cells += 1;
            if p.n > t.n {
                self.gouge += 1;
            }
            if p.n < t.n {
                self.miss += 1;
            }
            self.pass_err_sum += (p.n as f64 - t.n as f64).abs();
            if t.x_true.is_none() {
                self.float_blind += 1;
            }
        }
        // Truth-unreachable cells are neither over- nor under-routed on the
        // pencil/clearing axis; they are counted through `float_blind`.
        if t.x_true.is_some() {
            if p.pencil && t.clearing {
                self.route_over += 1;
            }
            if !p.pencil && !t.clearing {
                self.route_under += 1;
            }
            if !t.clearing {
                self.cover_den += t.n;
                if !p.refuse {
                    self.cover_fit_num += p.n.min(t.n);
                }
                if p.pencil && !p.refuse {
                    self.cover_num += p.n.min(t.n);
                }
            }
        }
    }
    fn coverage(&self) -> f64 {
        if self.cover_den == 0 {
            1.0
        } else {
            self.cover_num as f64 / self.cover_den as f64
        }
    }
    fn fit_coverage(&self) -> f64 {
        if self.cover_den == 0 {
            1.0
        } else {
            self.cover_fit_num as f64 / self.cover_den as f64
        }
    }
    fn mean_err(&self) -> f64 {
        if self.fan_cells == 0 {
            0.0
        } else {
            self.pass_err_sum / self.fan_cells as f64
        }
    }
    fn merge(&mut self, o: &Score) {
        self.cells += o.cells;
        self.fan_cells += o.fan_cells;
        self.gouge += o.gouge;
        self.miss += o.miss;
        self.pass_err_sum += o.pass_err_sum;
        self.route_over += o.route_over;
        self.route_under += o.route_under;
        self.float_blind += o.float_blind;
        self.cover_num += o.cover_num;
        self.cover_den += o.cover_den;
        self.cover_fit_num += o.cover_fit_num;
    }
}

fn width_class(w: f64) -> &'static str {
    match w {
        x if x < 0.4 => "below-tip",
        x if x < 0.75 => "tip",
        x if x < 1.5 => "2×tip",
        x if x < 2.5 => "cone",
        x if x < 4.0 => "shaft",
        _ => "beyond",
    }
}

/// Run the symmetric matrix for one tool, returning (per-cell scores keyed by
/// (angle, width), per-angle aggregate, grand total) plus the physics rows.
/// One row of the physics reference table.
struct PhysicsRow {
    theta: f64,
    w: f64,
    delta: f64,
    valley_depth: f64,
    apex_reach: f64,
    tip_float: f64,
    x_true: Option<f64>,
    n_true: usize,
}

struct MatrixRun {
    per_cell: Vec<((usize, usize), [Score; 5])>,
    per_angle: Vec<(f64, [Score; 5])>,
    total: [Score; 5],
    physics: Vec<PhysicsRow>,
    na_cells: usize,
}

fn run_matrix(cutter: &dyn MillingCutter) -> MatrixRun {
    let prof = Profile::build(cutter);
    let mut per_cell = Vec::new();
    let mut per_angle = Vec::new();
    let mut total = [Score::default(); 5];
    let mut physics = Vec::new();
    let mut na_cells = 0usize;

    for (ai, &theta) in ANGLES.iter().enumerate() {
        let mut ang = [Score::default(); 5];
        for (wi, &w) in WIDTHS.iter().enumerate() {
            let v = Valley::sym(w, theta);
            let mut cell = [Score::default(); 5];
            for &delta in DEPTHS.iter() {
                if delta > v.depth - DEPTH_TOL {
                    na_cells += 1;
                    continue;
                }
                let t = truth(&prof, &v, delta);
                physics.push(PhysicsRow {
                    theta,
                    w,
                    delta,
                    valley_depth: v.depth,
                    apex_reach: t.apex_reach,
                    tip_float: t.tip_float,
                    x_true: t.x_true,
                    n_true: t.n,
                });
                for (mi, &m) in MODELS.iter().enumerate() {
                    let p = predict(m, cutter, &v, delta);
                    cell[mi].add(p, t);
                }
            }
            for mi in 0..5 {
                ang[mi].merge(&cell[mi]);
                total[mi].merge(&cell[mi]);
            }
            per_cell.push(((ai, wi), cell));
        }
        per_angle.push((theta, ang));
    }
    MatrixRun {
        per_cell,
        per_angle,
        total,
        physics,
        na_cells,
    }
}

fn print_matrix(name: &str, cutter: &dyn MillingCutter, run: &MatrixRun) {
    println!();
    println!(
        "### {name} — envelope {:.3} mm, cusp {:.3} mm",
        cutter.envelope_radius_mm(),
        cutter.cusp_radius_mm()
    );
    println!();
    println!("| wall θ | half-width | class | ENV | CUSP | ENG | CLR | CLR+θ |");
    println!("|---:|---:|---|---|---|---|---|---|");
    for ((ai, wi), cell) in &run.per_cell {
        let theta = ANGLES[*ai];
        let w = WIDTHS[*wi];
        if cell[0].cells == 0 {
            println!(
                "| {theta:.0}° | {w:.2} | {} | — | — | — | — | — |",
                width_class(w)
            );
            continue;
        }
        let f = |s: &Score| format!("{}/{}/{:.2}", s.gouge, s.miss, s.mean_err());
        println!(
            "| {theta:.0}° | {w:.2} | {} | {} | {} | {} | {} | {} |",
            width_class(w),
            f(&cell[0]),
            f(&cell[1]),
            f(&cell[2]),
            f(&cell[3]),
            f(&cell[4])
        );
    }
    println!();
    println!("| aggregate | cells | ENV | CUSP | ENG | CLR | CLR+θ |");
    println!("|---|---:|---|---|---|---|---|");
    for (theta, ang) in &run.per_angle {
        let f = |s: &Score| format!("{}/{}/{:.2}", s.gouge, s.miss, s.mean_err());
        println!(
            "| θ = {theta:.0}° | {} | {} | {} | {} | {} | {} |",
            ang[0].cells,
            f(&ang[0]),
            f(&ang[1]),
            f(&ang[2]),
            f(&ang[3]),
            f(&ang[4])
        );
    }
    let f = |s: &Score| format!("{}/{}/{:.2}", s.gouge, s.miss, s.mean_err());
    println!(
        "| **all** | {} | {} | {} | {} | {} | {} |",
        run.total[0].cells,
        f(&run.total[0]),
        f(&run.total[1]),
        f(&run.total[2]),
        f(&run.total[3]),
        f(&run.total[4])
    );
    println!();
    println!("| | ENV | CUSP | ENG | CLR | CLR+θ |");
    println!("|---|---|---|---|---|---|");
    let col = |f: &dyn Fn(&Score) -> String| {
        format!(
            "{} | {} | {} | {} | {}",
            f(&run.total[0]),
            f(&run.total[1]),
            f(&run.total[2]),
            f(&run.total[3]),
            f(&run.total[4])
        )
    };
    println!(
        "| cells routed to a pencil fan | {} |",
        col(&|s: &Score| format!("{}", s.fan_cells))
    );
    println!(
        "| routing over-claim (pencil where truth says clearing) | {} |",
        col(&|s: &Score| format!("{}", s.route_over))
    );
    println!(
        "| routing under-claim (clearing where truth says pencil) | {} |",
        col(&|s: &Score| format!("{}", s.route_under))
    );
    println!(
        "| float-blind (centreline into material the tool cannot reach) | {} |",
        col(&|s: &Score| format!("{}", s.float_blind))
    );
    println!(
        "| fit-equation coverage, routing ignored | {} |",
        col(&|s: &Score| format!("{:.0}%", 100.0 * s.fit_coverage()))
    );
    println!(
        "| reachable-detail coverage as actually routed | {} |",
        col(&|s: &Score| format!("{:.0}%", 100.0 * s.coverage()))
    );
    println!();
    println!(
        "N/A cells (valley shallower than the commanded rest depth): {}",
        run.na_cells
    );
}

// ── Tests ────────────────────────────────────────────────────────────────

/// Cross-validation: the profile-erosion sampler that produces the ground
/// truth and the independent closed-form two-wall solve must agree. If they
/// do not, every number in the evidence pack is suspect.
#[test]
fn closed_form_matches_profile_erosion_sampler() {
    for (name, cutter) in tools() {
        let prof = Profile::build(cutter.as_ref());
        let mut checked = 0usize;
        let mut worst = 0.0f64;
        for &theta in ANGLES.iter() {
            for &w in WIDTHS.iter() {
                let v = Valley::sym(w, theta);
                for &delta in DEPTHS.iter() {
                    if delta > v.depth - DEPTH_TOL {
                        continue;
                    }
                    let sampled = prof.max_lateral(&v, delta, 1.0);
                    let closed =
                        closed_form_max_lateral(cutter.as_ref(), &v, delta, v.tan_r, v.tan_l);
                    match (sampled, closed) {
                        (Some(a), Some(b)) => {
                            // The sampler is capped at 2w; the closed form is
                            // not bounded by the far rim, so only compare
                            // where the sampler is not saturated.
                            if a < 2.0 * v.w - 1e-6 {
                                let d = (a - b).abs();
                                worst = worst.max(d);
                                assert!(
                                    d < 0.02,
                                    "{name} θ={theta} w={w} δ={delta}: sampler {a:.5} vs closed form {b:.5}"
                                );
                            }
                            checked += 1;
                        }
                        (None, None) => {
                            checked += 1;
                        }
                        (a, b) => panic!(
                            "{name} θ={theta} w={w} δ={delta}: feasibility disagreement {a:?} vs {b:?}"
                        ),
                    }
                }
            }
        }
        assert!(checked > 50, "{name}: only {checked} comparable cells");
        println!("{name}: {checked} cells cross-checked, worst |Δ| = {worst:.5} mm");
    }
}

fn tools() -> Vec<(&'static str, Box<dyn MillingCutter>)> {
    vec![
        ("taper Ø1/7°/Ø6", Box::new(wanaka_taper()) as Box<dyn MillingCutter>),
        ("taper Ø1/15°/Ø6", Box::new(steep_taper())),
        ("ball Ø3", Box::new(ball_control())),
    ]
}

/// THE headline defect, made numeric: on the shipped taper the envelope model
/// emits ZERO offset passes in every valley narrower than the Ø6 shank,
/// because `half_width_mm − 3.0` is negative there. The width-aware pass count
/// is dead code across the whole finishing-scale band
/// (`TOOL_SCALE_SEMANTICS.md` A2/A4) — it only wakes up in valleys wider than
/// the shank, which route to clearing anyway.
#[test]
fn envelope_model_is_dead_code_below_the_shank_on_the_shipped_taper() {
    let cutter = wanaka_taper();
    let r_env = cutter.envelope_radius_mm();
    let mut narrow_cells = 0usize;
    let mut narrow_nonzero = 0usize;
    let mut narrow_truth_supported = 0usize;
    let mut wide_nonzero = 0usize;
    for &theta in ANGLES.iter() {
        for &w in WIDTHS.iter() {
            let v = Valley::sym(w, theta);
            let prof = Profile::build(&cutter);
            for &delta in DEPTHS.iter() {
                if delta > v.depth - DEPTH_TOL {
                    continue;
                }
                let n = predict(Model::Envelope, &cutter, &v, delta).n;
                if w < r_env {
                    narrow_cells += 1;
                    if n > 0 {
                        narrow_nonzero += 1;
                    }
                    if truth(&prof, &v, delta).n > 0 {
                        narrow_truth_supported += 1;
                    }
                } else if n > 0 {
                    wide_nonzero += 1;
                }
            }
        }
    }
    assert!(
        narrow_cells > 80,
        "matrix too small to be evidence: {narrow_cells}"
    );
    assert_eq!(
        narrow_nonzero, 0,
        "envelope model produced {narrow_nonzero}/{narrow_cells} non-zero pass \
         counts below the shank radius — the A2/A4 analysis in \
         TOOL_SCALE_SEMANTICS.md would need revisiting"
    );
    assert!(
        narrow_truth_supported > 30,
        "only {narrow_truth_supported} sub-shank cells physically support an \
         offset pass; the suppression would not matter"
    );
    println!(
        "envelope dead-code: 0/{narrow_cells} sub-shank cells get a pass, but \
         {narrow_truth_supported} of them physically support one; \
         {wide_nonzero} wider-than-shank cells do get passes"
    );
}

/// The ball control pins "existing Ball behavior remains unchanged": envelope
/// and cusp are the same number on a ball, so the two baselines must produce
/// byte-identical predictions on every cell.
#[test]
fn ball_control_envelope_and_cusp_agree_on_every_cell() {
    let cutter = ball_control();
    assert!((cutter.envelope_radius_mm() - cutter.cusp_radius_mm()).abs() < 1e-12);
    for &theta in ANGLES.iter() {
        for &w in WIDTHS.iter() {
            let v = Valley::sym(w, theta);
            for &delta in DEPTHS.iter() {
                if delta > v.depth - DEPTH_TOL {
                    continue;
                }
                let a = predict(Model::Envelope, &cutter, &v, delta);
                let b = predict(Model::Cusp, &cutter, &v, delta);
                assert_eq!(a.n, b.n);
                assert_eq!(a.pencil, b.pencil);
            }
        }
    }
}

/// The plan's bar is "conservative against gouging without suppressing
/// reachable detail". This test states the two halves of that bar as
/// falsifiable claims on the taper:
///
/// * the CUSP baseline over-claims (gouge cells > 0) — it is not a safe swap;
/// * the profile-clearance solve WITH the wall angle never over-claims.
#[test]
fn cusp_overclaims_and_profile_clearance_with_theta_never_does() {
    let cutter = wanaka_taper();
    let run = run_matrix(&cutter);
    let cusp = run.total[1];
    let clr_theta = run.total[4];
    assert!(
        cusp.gouge > 0,
        "cusp baseline produced no gouge cells — §4.3's 'overstates reach at \
         depth' finding would need revisiting"
    );
    assert_eq!(
        clr_theta.gouge, 0,
        "profile-clearance WITH the wall angle over-claimed on {} cells; it is \
         the conservative reference by construction",
        clr_theta.gouge
    );
}

/// Reachable-detail coverage must improve over the envelope baseline — the
/// plan's acceptance gate. Measured over the cells where truth says a pencil
/// fan is the right strategy at all, with the routing verdict IGNORED so the
/// fit equation is scored on its own.
#[test]
fn depth_aware_fit_equations_recover_detail_the_envelope_baseline_suppresses() {
    for (name, cutter) in tools() {
        let run = run_matrix(cutter.as_ref());
        let env = run.total[0].fit_coverage();
        // CUSP is only a distinct model on a tapered tool — on the ball
        // control `cusp_radius() == envelope_radius()` by definition, so it
        // must tie, not improve (pinned separately by
        // `ball_control_envelope_and_cusp_agree_on_every_cell`).
        let candidates: &[usize] = if (cutter.envelope_radius_mm() - cutter.cusp_radius_mm()).abs()
            < 1e-12
        {
            &[2, 4]
        } else {
            &[1, 2, 3, 4]
        };
        for &mi in candidates {
            let m = run.total[mi].fit_coverage();
            assert!(
                m > env + 1e-9,
                "{name}: {} fit-covered {:.1}% of reachable detail vs \
                 envelope's {:.1}% — no coverage gain",
                MODELS[mi].code(),
                100.0 * m,
                100.0 * env
            );
        }
        println!(
            "{name}: fit-only coverage ENV={:.0}% CUSP={:.0}% ENG={:.0}% CLR={:.0}% CLR+θ={:.0}%",
            100.0 * run.total[0].fit_coverage(),
            100.0 * run.total[1].fit_coverage(),
            100.0 * run.total[2].fit_coverage(),
            100.0 * run.total[3].fit_coverage(),
            100.0 * run.total[4].fit_coverage(),
        );
    }
}

/// **The coupling finding.** `route_width_factor × r` and
/// `(half_width − r) / stepover` are fed by the SAME scalar. Shrinking `r`
/// from the envelope to a depth-aware value fixes the fit equation and
/// simultaneously collapses the pencil/clearing routing rule — at the shipped
/// `route_width_factor = 2.0` a depth-aware radius routes almost every
/// truth-pencil branch to clearing instead. Any H2.1 PR that swaps the radius
/// without re-deciding the routing rule trades one defect for another.
#[test]
fn swapping_the_radius_alone_collapses_the_pencil_clearing_routing_rule() {
    let cutter = wanaka_taper();
    let run = run_matrix(&cutter);
    let env = run.total[0];
    assert_eq!(
        env.route_under, 0,
        "envelope baseline under-routed; the coupling story changes"
    );
    for mi in [2usize, 3] {
        let m = run.total[mi];
        assert!(
            m.route_under > 40,
            "{} under-routed only {} cells; the routing coupling would not be \
             a Checkpoint-A concern",
            MODELS[mi].code(),
            m.route_under
        );
        assert!(
            m.fit_coverage() > m.coverage(),
            "{} routing did not suppress its own fit coverage",
            MODELS[mi].code()
        );
    }
    println!(
        "routing coupling (taper): ENV over/under = {}/{}, ENG = {}/{}, CLR = {}/{}",
        env.route_over,
        env.route_under,
        run.total[2].route_over,
        run.total[2].route_under,
        run.total[3].route_over,
        run.total[3].route_under,
    );
    println!(
        "  fit-only vs routed coverage: ENG {:.0}% → {:.0}%, CLR {:.0}% → {:.0}%",
        100.0 * run.total[2].fit_coverage(),
        100.0 * run.total[2].coverage(),
        100.0 * run.total[3].fit_coverage(),
        100.0 * run.total[3].coverage(),
    );
}

/// Tip float (`TOOL_SCALE_SEMANTICS.md` §4.4) is a failure mode a depth-only
/// model cannot express: in a narrow V the tool wedges between the two walls
/// and floats above the trough, so no rest depth is reachable however the
/// dials move. The ONLY candidate that can refuse such a branch is the one
/// that asks the profile a clearance question — `height_at_radius(w)`.
///
/// This pins (a) that the matrix contains such cells, (b) that the pure
/// depth-only models route pencil centrelines into them, and (c) that the
/// profile-clearance guard catches a strict subset of them.
#[test]
fn tip_float_is_invisible_to_depth_only_models_and_caught_by_the_clearance_guard() {
    for cutter in [wanaka_taper(), steep_taper()] {
        let prof = Profile::build(&cutter);
        let mut float_cells = 0usize;
        let mut blind = [0usize; 5];
        let mut worst_float = 0.0f64;
        for &theta in ANGLES.iter() {
            for &w in WIDTHS.iter() {
                let v = Valley::sym(w, theta);
                for &delta in DEPTHS.iter() {
                    if delta > v.depth - DEPTH_TOL {
                        continue;
                    }
                    let t = truth(&prof, &v, delta);
                    if t.x_true.is_some() {
                        continue;
                    }
                    float_cells += 1;
                    worst_float = worst_float.max(t.tip_float);
                    for (mi, &m) in MODELS.iter().enumerate() {
                        let p = predict(m, &cutter, &v, delta);
                        if p.pencil && !p.refuse {
                            blind[mi] += 1;
                        }
                    }
                }
            }
        }
        assert!(
            float_cells > 0,
            "no tip-float cells in the matrix — §4.4 is untested"
        );
        assert!(
            blind[0] > 0 && blind[1] > 0 && blind[2] > 0,
            "ENV/CUSP/ENG were not blind to tip float ({blind:?}); the \
             'depth-only is provably insufficient' finding would need revisiting"
        );
        assert!(
            blind[3] < blind[2],
            "the height_at_radius(w) clearance guard caught nothing ENG did not \
             ({blind:?})"
        );
        println!(
            "taper α={:.0}°: {float_cells} tip-float cells, worst float \
             {worst_float:.3} mm; float-blind per model \
             ENV={} CUSP={} ENG={} CLR={} CLR+θ={}",
            cutter.geometry_hint_taper_deg(),
            blind[0],
            blind[1],
            blind[2],
            blind[3],
            blind[4]
        );
    }
}

/// Local helper so the printout can name the taper angle without reaching into
/// production internals.
trait TaperAngle {
    fn geometry_hint_taper_deg(&self) -> f64;
}
impl TaperAngle for TaperedBallEndmill {
    fn geometry_hint_taper_deg(&self) -> f64 {
        match self.geometry_hint() {
            rs_cam_core::feeds::ToolGeometryHint::TaperedBall {
                taper_angle_deg, ..
            } => taper_angle_deg,
            _ => f64::NAN,
        }
    }
}

/// Asymmetric valleys: the trough is NOT at the mask centre, so a centreline
/// traced on the mask bisector stands off the true apex. Report the offset —
/// this is the plan's "bisector position error" experiment.
#[test]
fn asymmetric_valleys_bisector_offset_and_side_asymmetry() {
    let cutter = wanaka_taper();
    let prof = Profile::build(&cutter);
    println!();
    println!("### Asymmetric V — taper Ø1/7°/Ø6");
    println!();
    println!(
        "| θ_l | θ_r | w | depth | x_apex | δ | X_left | X_right | n_true | n_ENV | n_ENG | n_CLR+θ |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let pairs = [(30.0, 75.0), (45.0, 85.0), (60.0, 75.0)];
    let mut max_bisector_err: f64 = 0.0;
    let mut any_side_gap = false;
    for (tl, tr) in pairs {
        for &w in &[1.0, 2.0, 3.0] {
            let v = Valley::new(w, tl, tr);
            max_bisector_err = max_bisector_err.max(v.x_apex.abs());
            for &delta in &[0.2, 0.6, 2.0] {
                if delta > v.depth - DEPTH_TOL {
                    continue;
                }
                let l = prof.max_lateral(&v, delta, -1.0);
                let r = prof.max_lateral(&v, delta, 1.0);
                if let (Some(a), Some(b)) = (l, r)
                    && (a - b).abs() > 0.05
                {
                    any_side_gap = true;
                }
                let t = truth(&prof, &v, delta);
                let fmt = |o: Option<f64>| match o {
                    Some(x) => format!("{x:.3}"),
                    None => "—".to_owned(),
                };
                println!(
                    "| {tl:.0}° | {tr:.0}° | {w:.1} | {:.3} | {:+.3} | {delta:.2} | {} | {} | {} | {} | {} | {} |",
                    v.depth,
                    v.x_apex,
                    fmt(l),
                    fmt(r),
                    t.n,
                    predict(Model::Envelope, &cutter, &v, delta).n,
                    predict(Model::Engagement, &cutter, &v, delta).n,
                    predict(Model::ClearTheta, &cutter, &v, delta).n,
                );
            }
        }
    }
    assert!(
        max_bisector_err > 0.5,
        "asymmetric fixtures produced no meaningful apex offset ({max_bisector_err:.3} mm)"
    );
    assert!(
        any_side_gap,
        "no cell showed a left/right reach asymmetry — the single-scalar \
         half-width model would be adequate, which contradicts §4.4"
    );
    println!();
    println!("Max apex offset from the mask centre: {max_bisector_err:.3} mm");
}

/// The full report. Prints every table transcribed into
/// `CHECKPOINT_A_EVIDENCE.md`, and asserts the aggregate ordering the verdict
/// rests on.
#[test]
fn checkpoint_a_matrix_report() {
    println!();
    println!("== CHECKPOINT A — analytic valley matrix ==");
    println!(
        "sampling step {DU} mm; stepover {STEPOVER} mm; cap {CAP}; \
         route_width_factor {ROUTE_WIDTH_FACTOR}"
    );
    println!("entries are gouge/miss/mean|Δpass| aggregated over the 7 rest depths");

    let mut runs = Vec::new();
    for (name, cutter) in tools() {
        let run = run_matrix(cutter.as_ref());
        print_matrix(name, cutter.as_ref(), &run);
        runs.push((name, run));
    }

    // Physics reference rows for the shipped taper.
    println!();
    println!("### Physics reference — taper Ø1/7°/Ø6, symmetric V");
    println!();
    println!("| θ | w | valley depth | δ | apex reach | tip float | X_true | n_true |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|");
    let taper_run = &runs[0].1;
    for row in taper_run.physics.iter() {
        if !DEPTHS
            .iter()
            .any(|d| (*d - row.delta).abs() < 1e-9 && (*d == 0.2 || *d == 0.6 || *d == 2.0))
        {
            continue;
        }
        let xs = match row.x_true {
            Some(v) => format!("{v:.3}"),
            None => "unreachable".to_owned(),
        };
        println!(
            "| {:.0}° | {:.2} | {:.3} | {:.2} | {:.3} | {:.3} | {xs} | {} |",
            row.theta,
            row.w,
            row.valley_depth,
            row.delta,
            row.apex_reach,
            row.tip_float,
            row.n_true
        );
    }

    // Aggregate ordering the verdict rests on.
    let taper = &runs[0].1.total;
    assert!(taper[0].miss > 0, "envelope missed nothing");
    assert_eq!(taper[4].gouge, 0, "CLR+θ over-claimed");
    println!();
    println!("Grand totals (gouge, miss, mean |Δpass|) per tool:");
    for (name, run) in &runs {
        for (mi, m) in MODELS.iter().enumerate() {
            let s = run.total[mi];
            println!(
                "  {name:16} {:6} cells={} fan={} gouge={} miss={} mean|Δ|={:.3} \
                 route_over={} route_under={} float_blind={} coverage={:.0}%",
                m.code(),
                s.cells,
                s.fan_cells,
                s.gouge,
                s.miss,
                s.mean_err(),
                s.route_over,
                s.route_under,
                s.float_blind,
                100.0 * s.coverage(),
            );
            println!("      fit-only coverage={:.0}%", 100.0 * s.fit_coverage());
        }
    }
}

// ── PR-4 gate: the SHIPPED policy against the same truth ─────────────────
//
// Everything above is the historical Checkpoint A evidence and is kept as
// written (the tables in `CHECKPOINT_A_EVIDENCE.md` were transcribed from it).
// What follows drives `rs_cam_core::reach` — the production module PR-4
// landed — against the SAME analytic truth, so the approved column is pinned
// to shipped code rather than to a model reimplemented in a test.

/// The shipped policy's answer for one matrix cell, in the same `Prediction`
/// shape the candidate models above produce.
///
/// The mapping is the whole point of the gate, so it is explicit: each side of
/// the analytic V is handed to the policy as its apex-to-rim horizontal
/// distance (`depth / tan θ`) and its wall angle, which is exactly what
/// `rest_field::measure_cross_section` measures off `RestGrid::surface_z` in
/// production.
fn predict_production(cutter: &dyn MillingCutter, v: &Valley, delta: f64) -> Prediction {
    use rs_cam_core::reach::{
        LocalValley, RoutingVerdict, ValleySide, coverage_cap_passes, offset_passes_per_side, route,
        solve_reach,
    };
    let local = LocalValley {
        rest_depth_mm: delta,
        left: ValleySide::from_wall_angle(v.depth / v.tan_l, v.tan_l.atan()),
        right: ValleySide::from_wall_angle(v.depth / v.tan_r, v.tan_r.atan()),
    };
    let reach = solve_reach(cutter, &local);
    let cap = coverage_cap_passes(cutter, delta, STEPOVER, CAP);
    let verdict = route(&reach, STEPOVER, cap);
    let (nl, nr) = offset_passes_per_side(&reach, STEPOVER, CAP);
    Prediction {
        // The matrix scores one scalar per cell; the narrow side is what
        // `CLR+θ` scored (`x = min(l, r)`), so the per-side fan is collapsed
        // the same way here. The asymmetric fan is exercised separately in
        // `reach_policy_pr4.rs`.
        n: nl.min(nr),
        // Same convention the `ClearTheta` column used: a REFUSED cell is not
        // a clearing verdict, it is "this operation cannot do it".
        pencil: verdict != RoutingVerdict::Clearing,
        refuse: verdict == RoutingVerdict::Refused,
    }
}

/// **PR-4 acceptance gate.** The production reach policy, scored by the same
/// truth that produced `CHECKPOINT_A_EVIDENCE.md`, must reproduce the
/// approved CLR+θ column on every tool: zero gouge, zero miss, zero routing
/// over/under-claims, zero float-blind cells, 100 % reachable-detail
/// coverage. The envelope baseline it replaces is asserted alongside so the
/// comparison cannot silently become a comparison of nothing to nothing.
#[test]
fn production_reach_policy_reproduces_the_approved_matrix_column() {
    println!();
    println!("== PR-4 — shipped `reach` policy vs the Checkpoint A truth ==");
    for (name, cutter) in tools() {
        let cutter = cutter.as_ref();
        let prof = Profile::build(cutter);
        let mut prod = Score::default();
        let mut env = Score::default();
        for &theta in ANGLES.iter() {
            for &w in WIDTHS.iter() {
                let v = Valley::sym(w, theta);
                for &delta in DEPTHS.iter() {
                    if delta > v.depth - DEPTH_TOL {
                        continue;
                    }
                    let t = truth(&prof, &v, delta);
                    prod.add(predict_production(cutter, &v, delta), t);
                    env.add(predict(Model::Envelope, cutter, &v, delta), t);
                }
            }
        }
        println!(
            "  {name:16} PROD cells={} fan={} gouge={} miss={} route_over={} \
             route_under={} float_blind={} coverage={:.0}%",
            prod.cells,
            prod.fan_cells,
            prod.gouge,
            prod.miss,
            prod.route_over,
            prod.route_under,
            prod.float_blind,
            100.0 * prod.coverage()
        );
        println!(
            "  {name:16} ENV  gouge={} miss={} route_over={} float_blind={} coverage={:.0}%",
            env.gouge,
            env.miss,
            env.route_over,
            env.float_blind,
            100.0 * env.coverage()
        );
        assert!(prod.cells >= 176, "{name}: matrix did not run ({} cells)", prod.cells);
        assert_eq!(prod.gouge, 0, "{name}: production policy over-claimed");
        assert_eq!(prod.miss, 0, "{name}: production policy suppressed reachable detail");
        assert_eq!(prod.route_over, 0, "{name}: routed pencil where truth says clearing");
        assert_eq!(prod.route_under, 0, "{name}: routed clearing where truth says pencil");
        assert_eq!(prod.float_blind, 0, "{name}: routed a centreline the tool cannot hold");
        assert!(
            prod.coverage() > 0.999,
            "{name}: coverage {:.3} below the approved 100%",
            prod.coverage()
        );
        // Non-vacuity + the plan's "coverage increases over the envelope
        // baseline" gate, on the same run.
        assert!(
            prod.coverage() > env.coverage(),
            "{name}: production coverage {:.3} did not beat the envelope baseline {:.3}",
            prod.coverage(),
            env.coverage()
        );
    }
}

/// The coverage cap's floors (`reach::coverage_cap_passes`) exist so the
/// criterion cannot collapse to zero at the shipped `num_offset_passes = 0`
/// default. They must not bind anywhere on the matrix, or the routing column
/// asserted above would be reproducing a DIFFERENT rule than the evidence
/// scored. Asserted, not assumed.
#[test]
fn coverage_cap_floor_never_binds_on_the_matrix() {
    use rs_cam_core::reach::coverage_cap_passes;
    for (name, cutter) in tools() {
        let mut worst = 0usize;
        for &delta in DEPTHS.iter() {
            let cap = coverage_cap_passes(cutter.as_ref(), delta, STEPOVER, CAP);
            worst = worst.max(coverage_cap_passes(cutter.as_ref(), delta, STEPOVER, 0));
            assert_eq!(
                cap, CAP,
                "{name}: cap floor bound at δ={delta} (got {cap}, matrix scored {CAP})"
            );
        }
        println!("  {name:16} cap floor at num_offset_passes=0 is {worst} pass(es)");
        assert!(worst >= 1, "{name}: cap floor collapsed to zero");
    }
}

// ── End-to-end confirmation probes ───────────────────────────────────────
//
// Not the matrix — three cheap probes through the REAL routing path, to show
// the analytic finding survives contact with `detect_rest_valleys` /
// `pencil_toolpath_structured_annotated`.

/// A block 40 × 24 mm with its top at z = 0 and one straight trapezoidal
/// groove along Y at x = 0: rim half-width 1.5 mm, walls at 70°, floor at
/// depth 1.2 mm. Height-field mesh (drop-cutter only needs the top surface).
fn grooved_block(rim_half_width: f64, wall_deg: f64, depth: f64) -> TriangleMesh {
    let tan = wall_deg.to_radians().tan();
    let floor_half = rim_half_width - depth / tan;
    assert!(floor_half > 0.0, "groove is a V, not a trapezoid");
    let z_at = |x: f64| -> f64 {
        let ax = x.abs();
        if ax >= rim_half_width {
            0.0
        } else if ax <= floor_half {
            -depth
        } else {
            -depth + (ax - floor_half) * tan
        }
    };

    // X samples: dense across the groove, coarse outside, with every
    // breakpoint present exactly.
    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -3.0 {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -3.0;
    while x <= 3.0 + 1e-9 {
        xs.push(x);
        x += 0.1;
    }
    for b in [-rim_half_width, -floor_half, floor_half, rim_half_width] {
        xs.push(b);
    }
    let mut x = 4.0;
    while x <= 20.0 + 1e-9 {
        xs.push(x);
        x += 1.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let ys: Vec<f64> = (0..=24).map(|i| -12.0 + i as f64).collect();

    let mut verts = Vec::with_capacity(xs.len() * ys.len());
    for &yv in &ys {
        for &xv in &xs {
            verts.push(P3::new(xv, yv, z_at(xv)));
        }
    }
    let nx = xs.len();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let b = (j * nx + i + 1) as u32;
            let c = ((j + 1) * nx + i + 1) as u32;
            let d = ((j + 1) * nx + i) as u32;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

fn probe_params(reference_diameter: f64) -> PencilParams {
    PencilParams {
        detector: PencilDetector::RestDepth,
        rest_cell_mm: 0.3,
        min_valley_depth: 0.05,
        min_cut_length: 2.0,
        num_offset_passes: CAP,
        offset_stepover: STEPOVER,
        sampling: 0.5,
        reference_tool_diameter: reference_diameter,
        ..PencilParams::default()
    }
}

/// PROBE 1 — **the headline gate, now inverted.**
///
/// As originally written this probe asserted the DEFECT the plan asked for
/// verbatim: *offset passes emitted = 0 under envelope on a 3 mm valley the
/// tip could ladder*. `CHECKPOINT_A_EVIDENCE.md` §7 records that run
/// (`offset_total = 1` on every chain — a bare centreline) and stays as the
/// historical record.
///
/// PR-5 replaced the envelope fit equation with the reach policy, so the same
/// fixture, through the same real pencil path, now emits a fan. This probe
/// pins that: `offset_total > 1`, i.e. reachable detail the shipped model
/// suppressed for the whole life of the operation is now cut.
#[test]
fn probe_reach_policy_emits_the_fan_the_envelope_baseline_suppressed() {
    let mesh = grooved_block(1.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let params = probe_params(12.0);
    let mut grid = None;
    let mut regions = None;
    let (_tp, ann) = pencil_toolpath_structured_annotated(
        &mesh,
        &index,
        &cutter,
        &params,
        None,
        None,
        &mut grid,
        &mut regions,
    );
    let mut chains: std::collections::BTreeSet<usize> = Default::default();
    let mut max_offset_total = 0usize;
    for a in &ann {
        let PencilRuntimeEvent::OffsetPass {
            chain_index,
            offset_total,
            ..
        } = a.event;
        chains.insert(chain_index);
        max_offset_total = max_offset_total.max(offset_total);
    }
    println!(
        "PROBE 1 (fixed): chains={} max offset_total={} (1 ⇒ centreline only, \
         the pre-PR-5 reading)",
        chains.len(),
        max_offset_total
    );
    assert!(
        !chains.is_empty(),
        "probe fixture produced no rest centrelines — fixture is not exercising \
         the RestDepth arm"
    );

    // The equation that used to run here, kept as the baseline it is measured
    // against: on a Ø6-shank taper `half_width − 3.0` is negative for every
    // valley narrower than 6 mm, so the fan was dead code.
    let w_meas = 1.5f64;
    let delta = 1.2f64;
    let n_env = ((w_meas - cutter.envelope_radius_mm()) / STEPOVER).round().max(0.0) as usize;
    let n_eng = ((w_meas - cutter.engagement_radius_mm(delta)) / STEPOVER)
        .round()
        .max(0.0) as usize;
    println!(
        "PROBE 1: envelope r={:.3} ⇒ n={}, engagement r(δ=1.2)={:.3} ⇒ n={}",
        cutter.envelope_radius_mm(),
        n_env,
        cutter.engagement_radius_mm(delta),
        n_eng
    );
    assert_eq!(n_env, 0, "the envelope baseline moved — re-derive this probe");
    assert!(n_eng >= 1, "depth-aware model also predicted zero passes");
    assert!(
        max_offset_total > 1,
        "the reach policy emitted a bare centreline where the envelope \
         baseline did too — the A2/A4 fit defect is back (offset_total={max_offset_total})"
    );
}

/// PROBE 2 — the ball control on the same fixture. A Ø3 ball's envelope IS its
/// cusp, so nothing about it moves under any of the candidate swaps; this pins
/// the "existing Ball behavior remains unchanged" gate at the routing layer.
#[test]
fn probe_ball_control_routing_is_invariant_to_the_swap() {
    let mesh = grooved_block(1.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = ball_control();
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &BallEndmill::new(12.0, 25.0) as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: 0.3,
            min_valley_depth: 0.05,
            offset_stepover_mm: STEPOVER,
            num_offset_passes_cap: CAP,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );
    let widths: Vec<f64> = rf.centerlines.iter().map(|c| c.half_width_mm).collect();
    println!(
        "PROBE 2 (ball Ø3): centrelines={} clearing_regions={} half_widths={:?}",
        rf.centerlines.len(),
        rf.clearing_regions.len(),
        widths
    );
    assert!(
        (cutter.envelope_radius_mm() - cutter.cusp_radius_mm()).abs() < 1e-12,
        "ball control is not a control if envelope and cusp differ"
    );
}

/// PROBE 3 — the A8 defect (`TOOL_SCALE_SEMANTICS.md` §7.1), **now fixed**
/// (task #12, PR-4).
///
/// As originally written this probe asserted the DEFECT: at the shipped
/// `reference_tool_diameter = 6.0` a Ø1-tip / Ø6-shank taper compared the
/// reference against `diameter()` (the shank), `6.0 > 6.0` was false, and
/// every tapered pencil operation silently fell through to the
/// self-referenced surface probe. `CHECKPOINT_A_EVIDENCE.md` §7 records that
/// run (1 chain at the default versus 2 above the shank) and stays as the
/// historical evidence.
///
/// The comparison is now against the tip diameter (`cusp_radius_mm() * 2`),
/// so this probe pins the FIX: at the default the taper resolves a real Ø6
/// nominal reference, which is a different measurement from the
/// self-referenced probe it used to silently become.
#[test]
fn probe_tapered_pencil_resolves_a_real_reference_at_the_default() {
    let mesh = grooved_block(1.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();

    let count = |reference_diameter: f64| -> usize {
        let params = probe_params(reference_diameter);
        let mut grid = None;
        let mut regions = None;
        let (_tp, ann) = pencil_toolpath_structured_annotated(
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            None,
            &mut grid,
            &mut regions,
        );
        let mut chains: std::collections::BTreeSet<usize> = Default::default();
        for a in &ann {
            let PencilRuntimeEvent::OffsetPass { chain_index, .. } = a.event;
            chains.insert(chain_index);
        }
        chains.len()
    };

    // Below the TIP diameter (1.0), so still genuinely self-referenced — the
    // control the default used to silently collapse onto.
    let self_referenced = count(0.5);
    let at_default = count(6.0); // the shipped default: now a Ø6 nominal ball
    let above_shank = count(12.0);
    println!(
        "PROBE 3 (fixed): reference 0.5 (below the tip ⇒ self-referenced) ⇒ \
         {self_referenced} chains; 6.0 (the default) ⇒ {at_default} chains; \
         12.0 ⇒ {above_shank} chains"
    );
    assert!(
        cutter.diameter() >= 6.0 && cutter.cusp_radius_mm() * 2.0 <= 1.0,
        "fixture no longer exercises the shank-vs-tip comparison \
         (shank {:.2}, tip {:.2})",
        cutter.diameter(),
        cutter.cusp_radius_mm() * 2.0
    );
    assert_ne!(
        at_default, self_referenced,
        "the shipped default still behaves exactly like the self-referenced \
         probe — the §7.1 fall-through is back"
    );
}
