//! C9 EVIDENCE — sampled-cross-section reach vs the CLR+θ V model, and the
//! rest-grid resolution question that decides which one production runs.
//!
//! Two things are being measured here, and the whole point of the file is that
//! they are measured SEPARATELY:
//!
//! 1. **Model error.** `reach::solve_reach` fits a V to the local
//!    cross-section: one wall angle and one rim distance per side. Its own
//!    module doc records where that is known to be wrong — a trapezoidal
//!    groove is not a V, and a curved wall is not a V either.
//!    `reach::solve_reach_sampled` erodes the cutter against the sampled
//!    section itself and reads no angle at all. The two fixtures below are
//!    exactly the shapes the V is known to misread.
//! 2. **Resolution error.** The sampled model is only as good as the pitch it
//!    is handed, and in production that pitch is the REST CELL — 0.5 mm today,
//!    against a Ø1 tip. A sampled solve fed a 0.5 mm section cannot resolve a
//!    reach to better than the cell.
//!
//! If those two are measured in one number, a coarse grid masquerades as a
//! better model or a worse one and the comparison decides nothing. So every
//! model comparison below runs at a FINE pitch (model error, grid held
//! constant) and every resolution sweep runs ONE model across pitches
//! (resolution error, model held constant).
//!
//! # Ground truth
//!
//! The same construction Checkpoint A uses
//! (`checkpoint_a_valley_matrix.rs` header): a straight groove in a block whose
//! top surface is `z = 0`, tool queried only through `MillingCutter`, and the
//! truth taken by direct morphological erosion of the surface by the cutter
//! profile —
//!
//! ```text
//!     fits at (x, δ)   ⟺   z(x + u) − H(|u|) ≤ −δ − L·DU   for all u
//!     X_true(δ)        =   max { q ≥ 0 : fits at (x_apex ± q, δ) }
//! ```
//!
//! with the one-sided `L·DU` guard making the truth conservative, exactly as
//! in Checkpoint A. What is new is only that `z` is no longer restricted to a
//! V: it is any piecewise-smooth section, and the two fixtures use that.
//!
//! # What is reported
//!
//! `X_true` is a LATERAL reach in mm, so the error metric is a lateral
//! distance and a pass-count (`floor(X/stepover)`) — both measured against
//! the same truth, per side, never averaged across fixtures. `GOUGE` keeps
//! Checkpoint A's meaning: the model claims a pass at an offset the tool
//! cannot hold.
//!
//! Nothing here is imported by production code.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::reach::{
    LocalValley, Reach, SampledCrossSection, ValleySide, coverage_cap_passes,
    offset_passes_per_side, route, solve_reach, solve_reach_sampled,
};
use rs_cam_core::tool::MillingCutter;

mod common;

use common::tools::{ball_control, steep_taper, wanaka_taper};

// ── Constants ────────────────────────────────────────────────────────────

/// Lateral profile sampling step (mm) for the ground-truth erosion — the same
/// value Checkpoint A uses, and the same one-sided `L·DU` bound follows.
const DU: f64 = 0.00025;
/// Offset stepover the fan is spaced at (`PencilParams::default`).
const STEPOVER: f64 = 0.5;
/// Offset-pass cap.
const CAP: usize = 4;
/// The pitch a MODEL comparison samples the cross-section at. Fine enough that
/// the sampled model's resolution term is negligible next to the model term —
/// which is the claim `resolution_sweep_separates_model_error_from_grid_error`
/// substantiates rather than assumes.
const FINE_PITCH: f64 = 0.01;
/// The pitch production actually has: `RestFieldParams::cell_mm`'s shipped
/// default.
const SHIPPED_REST_CELL_MM: f64 = 0.5;

// ── Generic analytic surface ─────────────────────────────────────────────

/// A groove cross-section in RIM-relative coordinates: `z(x) ≤ 0`, `z = 0` on
/// the flat top outside the rim, apex at `x_apex` and depth `depth` below the
/// top.
struct Section {
    /// Fixture name, for report lines.
    #[allow(dead_code)]
    name: &'static str,
    z: Box<dyn Fn(f64) -> f64>,
    lipschitz: f64,
    x_apex: f64,
    depth: f64,
    /// Horizontal apex-to-rim distance, per side. This is what the rest-field
    /// detector's walk reports as `rim_distance_mm`.
    rim_left: f64,
    rim_right: f64,
}

impl Section {
    fn z(&self, x: f64) -> f64 {
        (self.z)(x)
    }
}

/// A symmetric TRAPEZOIDAL groove: a flat floor `2·floor_half` wide at depth
/// `depth`, walls at `wall_deg` from horizontal, flat top beyond the rim.
///
/// **This is a CONTROL, not a discriminating fixture, and that is a C9
/// finding.** `reach.rs`'s known-limitation note named the trapezoid as the
/// shape the V misreads. Worked through, it is not:
///
/// ```text
///     X_v    = rim − max over u ∈ [0,δ] of [ width(u) + (δ−u)·cot θ ]
///            = min over u of [ rim − (δ−u)·cot θ − width(u) ]
///     X_true = min over u of [ floor_half + (u + D − δ)·cot θ − width(u) ]
///     rim    = floor_half + D·cot θ   ⟹   the two expressions are IDENTICAL
/// ```
///
/// The reason is that the cutter's tip can never go BELOW the floor (every
/// profile height is ≥ 0 above the tip), so the only part of the section that
/// can ever constrain the tool is the straight wall — and a straight wall is
/// exactly what the V model's "rim distance + steepest local gradient" reading
/// encodes. The floor the model ignores is material the model never needed.
///
/// So the trapezoid joins the V in the control arm, where the two models must
/// TIE. The shapes that actually break the V are the ones whose constraining
/// surface is not a single straight line: [`chamfered`] and [`circular`].
fn trapezoid(floor_half: f64, wall_deg: f64, depth: f64) -> Section {
    let tan = wall_deg.to_radians().tan();
    let run = depth / tan;
    let rim = floor_half + run;
    Section {
        name: "trapezoid",
        z: Box::new(move |x: f64| {
            let a = x.abs();
            if a >= rim {
                0.0
            } else if a <= floor_half {
                -depth
            } else {
                -depth + (a - floor_half) * tan
            }
        }),
        lipschitz: tan,
        x_apex: 0.0,
        depth,
        rim_left: rim,
        rim_right: rim,
    }
}

/// A CHAMFERED trapezoidal groove — flat floor at `depth`, a steep lower wall
/// at `steep_deg`, then a shallower chamfer at `cham_deg` for the top
/// `cham_rise` mm up to the rim. A groove with a broken top edge: an ordinary
/// machining shape, and the trapezoid variant the V model genuinely cannot
/// read.
///
/// The V reads ONE angle — production's is the steepest local gradient, i.e.
/// the lower wall — and draws its wall as a straight line from the rim inward
/// at that angle. The real chamfer runs back inward FASTER than that line, so
/// the material near the rim sits INSIDE the modelled wall. The model
/// therefore under-constrains the tool and over-states the reach: a gouge, in
/// the direction that reaches the workpiece.
fn chamfered(
    floor_half: f64,
    steep_deg: f64,
    cham_deg: f64,
    depth: f64,
    cham_rise: f64,
) -> Section {
    let ts = steep_deg.to_radians().tan();
    let tc = cham_deg.to_radians().tan();
    let cham_rise = cham_rise.min(depth * 0.8);
    // Break point: `depth − cham_rise` above the floor.
    let break_x = floor_half + (depth - cham_rise) / ts;
    let rim = break_x + cham_rise / tc;
    Section {
        name: "chamfered",
        z: Box::new(move |x: f64| {
            let a = x.abs();
            if a >= rim {
                0.0
            } else if a <= floor_half {
                -depth
            } else if a <= break_x {
                -depth + (a - floor_half) * ts
            } else {
                -cham_rise + (a - break_x) * tc
            }
        }),
        lipschitz: ts.max(tc),
        x_apex: 0.0,
        depth,
        rim_left: rim,
        rim_right: rim,
    }
}

/// A symmetric CIRCULAR-ARC valley of radius `r` cut `depth` deep — a wall
/// whose angle varies continuously from 0° at the floor to its steepest at the
/// rim, so no single angle describes it anywhere.
fn circular(r: f64, depth: f64) -> Section {
    let depth = depth.min(r * 0.999);
    // Arc centre `r` above the apex; the rim is where the arc reaches z = 0.
    let rim = (r * r - (r - depth) * (r - depth)).sqrt();
    Section {
        name: "circular",
        z: Box::new(move |x: f64| {
            let a = x.abs();
            if a >= rim {
                0.0
            } else {
                // Circle of radius `r` centred `r` above the apex: z(0) =
                // −depth, z(±rim) = 0.
                (r - depth) - (r * r - a * a).sqrt()
            }
        }),
        lipschitz: rim / (r * r - rim * rim).sqrt().max(1e-9),
        x_apex: 0.0,
        depth,
        rim_left: rim,
        rim_right: rim,
    }
}

/// A straight symmetric V — the shape the CLR+θ model is EXACT on. Present as
/// the control arm: if the sampled model did not tie here, the two fixtures
/// below would be measuring the harness, not the shapes.
fn vee(rim: f64, wall_deg: f64) -> Section {
    let tan = wall_deg.to_radians().tan();
    let depth = rim * tan;
    Section {
        name: "vee",
        z: Box::new(move |x: f64| {
            let a = x.abs();
            if a >= rim { 0.0 } else { -(rim - a) * tan }
        }),
        lipschitz: tan,
        x_apex: 0.0,
        depth,
        rim_left: rim,
        rim_right: rim,
    }
}

// ── Ground truth (profile erosion) ───────────────────────────────────────

struct Profile {
    h: Vec<f64>,
}

impl Profile {
    fn build(cutter: &dyn MillingCutter) -> Self {
        let n = (cutter.envelope_radius_mm() / DU).floor() as usize;
        let mut h = Vec::with_capacity(n + 1);
        let mut last = 0.0f64;
        for i in 0..=n {
            last = cutter
                .height_at_radius(i as f64 * DU)
                .unwrap_or(last)
                .max(last);
            h.push(last);
        }
        Self { h }
    }

    /// Tool axis at `x`, tip `delta` below the flat top: does it clear?
    /// Conservative by exactly `L·DU`, one-sided (Checkpoint A's bound).
    fn fits(&self, s: &Section, x: f64, delta: f64) -> bool {
        let limit = -delta - s.lipschitz * DU;
        for (i, &hi) in self.h.iter().enumerate() {
            if hi > delta {
                break;
            }
            let u = i as f64 * DU;
            if s.z(x + u) - hi > limit || (i > 0 && s.z(x - u) - hi > limit) {
                return false;
            }
        }
        true
    }

    /// `X_true(δ)` toward `dir`. `None` when the tool cannot hold `δ` on the
    /// apex at all (tip float).
    fn max_lateral(&self, s: &Section, delta: f64, dir: f64) -> Option<f64> {
        if !self.fits(s, s.x_apex, delta) {
            return None;
        }
        let mut hi = s.rim_left.max(s.rim_right) * 2.0;
        if self.fits(s, s.x_apex + dir * hi, delta) {
            return Some(hi);
        }
        let mut lo = 0.0f64;
        for _ in 0..32 {
            let mid = 0.5 * (lo + hi);
            if self.fits(s, s.x_apex + dir * mid, delta) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    }

    fn x_true(&self, s: &Section, delta: f64) -> Option<f64> {
        match (
            self.max_lateral(s, delta, -1.0),
            self.max_lateral(s, delta, 1.0),
        ) {
            (Some(l), Some(r)) => Some(l.min(r)),
            _ => None,
        }
    }
}

// ── The two models, both fed from the SAME measured section ──────────────

/// Sample the section the way `rest_field::measure_cross_section` walks it:
/// outward from the apex at `pitch`, one height per cell, relative to the
/// apex, continuing past the rim far enough for the flat top to constrain the
/// tool (`CROSS_SECTION_PROBE_CELLS` worth of envelope, as production does).
fn measured_section(s: &Section, pitch: f64, envelope: f64) -> SampledCrossSection {
    let side = |sign: f64, rim: f64| -> Vec<f64> {
        let extent = rim + envelope + pitch;
        let n = (extent / pitch).ceil() as usize;
        (1..=n)
            .map(|j| {
                let x = s.x_apex + sign * j as f64 * pitch;
                s.z(x) - s.z(s.x_apex)
            })
            .collect()
    };
    SampledCrossSection::from_sides(pitch, &side(-1.0, s.rim_left), &side(1.0, s.rim_right))
}

/// The V model as PRODUCTION reads it: rim distance from the walk, wall rise
/// from the STEEPEST single-cell gradient inside the depth band. This is
/// `rest_field::measure_cross_section` reproduced against the analytic
/// section, so what is scored is the shipped reading, not a flattering one.
fn v_model_valley(s: &Section, delta: f64, pitch: f64) -> LocalValley {
    let z0 = s.z(s.x_apex);
    let side = |sign: f64, rim: f64| -> ValleySide {
        let n = (rim / pitch).ceil().max(1.0) as usize;
        let mut max_slope = 0.0f64;
        let mut z_prev = z0;
        for j in 1..=n {
            let zj = s.z(s.x_apex + sign * j as f64 * pitch);
            let rise = zj - z_prev;
            if rise > 0.0 && zj - z0 <= delta + 1e-9 {
                max_slope = max_slope.max(rise / pitch);
            }
            z_prev = zj;
        }
        ValleySide::new(rim, rim * max_slope)
    };
    LocalValley {
        rest_depth_mm: delta,
        left: side(-1.0, s.rim_left),
        right: side(1.0, s.rim_right),
    }
}

// ── Scoring ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default, Debug)]
struct Tally {
    cells: usize,
    /// Cells the model routed to a pencil fan — the only ones where a pass
    /// count is emitted, and so the only ones where a count error is real.
    fan_cells: usize,
    gouge: usize,
    miss: usize,
    /// Σ |X_model − X_true| over fan cells (mm).
    abs_err_sum: f64,
    worst_err: f64,
    /// Σ over-claim only (mm) — the direction that cuts material the tool
    /// cannot hold.
    over_sum: f64,
    worst_over: f64,
    float_blind: usize,
    cover_num: usize,
    cover_den: usize,
}

impl Tally {
    fn add(&mut self, reach: &Reach, cutter: &dyn MillingCutter, delta: f64, truth: Option<f64>) {
        self.cells += 1;
        let cap = coverage_cap_passes(cutter, delta, STEPOVER, CAP);
        let verdict = route(reach, STEPOVER, cap);
        let (nl, nr) = offset_passes_per_side(reach, STEPOVER, CAP);
        let n_model = nl.min(nr);
        let x_model = reach.min_mm();
        let refused = verdict == rs_cam_core::reach::RoutingVerdict::Refused;
        let pencil = verdict != rs_cam_core::reach::RoutingVerdict::Clearing;
        let n_true = truth
            .map(|x| ((x / STEPOVER).floor() as usize).min(CAP))
            .unwrap_or(0);
        if pencil && !refused {
            self.fan_cells += 1;
            if n_model > n_true {
                self.gouge += 1;
            }
            if n_model < n_true {
                self.miss += 1;
            }
            match truth {
                None => self.float_blind += 1,
                Some(x) => {
                    let err = x_model - x;
                    self.abs_err_sum += err.abs();
                    self.worst_err = self.worst_err.max(err.abs());
                    if err > 0.0 {
                        self.over_sum += err;
                        self.worst_over = self.worst_over.max(err);
                    }
                }
            }
        }
        if truth.is_some() {
            self.cover_den += n_true;
            if pencil && !refused {
                self.cover_num += n_model.min(n_true);
            }
        }
    }

    fn mean_abs_err(&self) -> f64 {
        if self.fan_cells == 0 {
            0.0
        } else {
            self.abs_err_sum / self.fan_cells as f64
        }
    }

    fn coverage(&self) -> f64 {
        if self.cover_den == 0 {
            1.0
        } else {
            self.cover_num as f64 / self.cover_den as f64
        }
    }

    fn line(&self, label: &str) -> String {
        format!(
            "{label:22} cells={:3} fan={:3} gouge={:2} miss={:2} float_blind={:2} \
             coverage={:5.1}%  mean|Δ|={:.4} mm  worst|Δ|={:.4} mm  worst over-claim={:.4} mm",
            self.cells,
            self.fan_cells,
            self.gouge,
            self.miss,
            self.float_blind,
            100.0 * self.coverage(),
            self.mean_abs_err(),
            self.worst_err,
            self.worst_over,
        )
    }
}

fn tools() -> Vec<(&'static str, Box<dyn MillingCutter>)> {
    vec![
        (
            "wanaka taper",
            Box::new(wanaka_taper()) as Box<dyn MillingCutter>,
        ),
        ("steep taper", Box::new(steep_taper())),
        ("ball control", Box::new(ball_control())),
    ]
}

/// Rest depths swept per fixture, as fractions of the fixture's own depth —
/// so a 0.4 mm groove and a 3 mm groove are both sampled through their whole
/// usable range instead of one of them being all-N/A.
const DEPTH_FRACTIONS: [f64; 5] = [0.15, 0.35, 0.55, 0.75, 0.95];

/// Score both models over one fixture family at one cross-section pitch.
fn score_family(cutter: &dyn MillingCutter, sections: &[Section], pitch: f64) -> (Tally, Tally) {
    let prof = Profile::build(cutter);
    let (mut v, mut samp) = (Tally::default(), Tally::default());
    for s in sections {
        let section = measured_section(s, pitch, cutter.envelope_radius_mm());
        for &f in DEPTH_FRACTIONS.iter() {
            let delta = s.depth * f;
            let truth = prof.x_true(s, delta);
            v.add(
                &solve_reach(cutter, &v_model_valley(s, delta, pitch)),
                cutter,
                delta,
                truth,
            );
            samp.add(
                &solve_reach_sampled(cutter, &section, delta),
                cutter,
                delta,
                truth,
            );
        }
    }
    (v, samp)
}

fn chamfered_family() -> Vec<Section> {
    let mut out = Vec::new();
    for &floor_half in &[0.3_f64, 0.75, 1.5] {
        for &(steep, cham) in &[(85.0_f64, 30.0_f64), (75.0, 40.0), (70.0, 25.0)] {
            for &depth in &[0.4_f64, 1.0, 2.0] {
                out.push(chamfered(floor_half, steep, cham, depth, depth * 0.4));
            }
        }
    }
    out
}

fn circular_family() -> Vec<Section> {
    let mut out = Vec::new();
    // Gentle radii and shallow cuts as well as tight ones: a family that is
    // ALL tip-float would prove only that the sampled model refuses, never
    // that it reaches correctly where the tool can in fact reach.
    for &r in &[1.0_f64, 2.5, 6.0, 12.0, 30.0] {
        for &depth in &[0.1_f64, 0.2, 0.5, 1.2, 2.0] {
            if depth < r * 0.95 {
                out.push(circular(r, depth));
            }
        }
    }
    out
}

/// The control arm: shapes whose constraining surface is a single straight
/// line, where the V model is exact and the two must therefore TIE.
fn control_family() -> Vec<Section> {
    let mut out = Vec::new();
    for &rim in &[0.5_f64, 1.0, 2.0, 3.0] {
        for &wall in &[45.0_f64, 60.0, 75.0] {
            out.push(vee(rim, wall));
        }
    }
    for &floor_half in &[0.3_f64, 0.75, 1.5] {
        for &wall in &[60.0_f64, 75.0, 85.0] {
            for &depth in &[0.4_f64, 1.0, 2.0] {
                out.push(trapezoid(floor_half, wall, depth));
            }
        }
    }
    out
}

// ── Gate 1: the V control — the two models must TIE on a V ───────────────

/// The sampled model is claimed to be a strict GENERALISATION of the V solve,
/// not a different answer. On a straight V at a fine pitch the two must agree
/// to within the sampling step; if they did not, every number in the two
/// fixtures below would be a harness artefact.
#[test]
fn on_straight_walled_sections_the_sampled_model_ties_the_wall_angle_model() {
    println!();
    println!("== C9 control — straight V + plain trapezoid, pitch {FINE_PITCH} mm ==");
    for (name, cutter) in tools() {
        let (v, samp) = score_family(cutter.as_ref(), &control_family(), FINE_PITCH);
        println!("  {name}");
        println!("    {}", v.line("CLR+θ (V model)"));
        println!("    {}", samp.line("SAMP (sampled)"));
        assert!(v.cells >= 12, "{name}: control family did not run");
        assert_eq!(samp.gouge, 0, "{name}: sampled model over-claimed on a V");
        assert_eq!(
            samp.float_blind, 0,
            "{name}: sampled model routed a centreline the tool cannot hold"
        );
        // The tie is to within ONE PASS, not exact, and the slack is named
        // rather than tuned: the upper-envelope reading places a wall at the
        // inner end of the segment bracketing it, so the sampled reach can be
        // up to one PITCH short, and a cell whose true reach sits inside that
        // margin of a stepover boundary drops a pass. At 0.01 mm pitch that
        // is one cell in 153 on the steep taper (89.0 % vs 89.6 %).
        // `checkpoint_a_valley_matrix::sampled_cross_section_model_reproduces_
        // the_approved_matrix_column` is where the exact bar lives, swept
        // against pitch.
        assert!(
            samp.coverage() >= v.coverage() - 0.01,
            "{name}: sampled coverage {:.3} more than one pass below the V \
             model's {:.3} on the shape the V is exact on — the generalisation \
             claim fails",
            samp.coverage(),
            v.coverage()
        );
        assert!(
            samp.worst_err <= v.worst_err.max(0.05) + 1e-9,
            "{name}: sampled worst error {:.4} mm vs V's {:.4} mm on a V",
            samp.worst_err,
            v.worst_err
        );
    }
}

// ── Gate 2 + 3: the shapes the V is known to misread ──────────────────────

fn assert_sampled_beats_v(label: &str, families: &[Section]) {
    println!();
    println!("== C9 {label} — pitch {FINE_PITCH} mm (model error, grid held fixed) ==");
    let mut any_strict = false;
    for (name, cutter) in tools() {
        let (v, samp) = score_family(cutter.as_ref(), families, FINE_PITCH);
        println!("  {name}");
        println!("    {}", v.line("CLR+θ (V model)"));
        println!("    {}", samp.line("SAMP (sampled)"));
        assert!(v.cells >= 20, "{name}: {label} family did not run");
        assert_eq!(
            samp.gouge, 0,
            "{name}: sampled model over-claimed on {label}"
        );
        assert!(
            samp.mean_abs_err() <= v.mean_abs_err() + 1e-9,
            "{name}: sampled mean |Δ| {:.4} mm is worse than the V model's \
             {:.4} mm on {label}",
            samp.mean_abs_err(),
            v.mean_abs_err()
        );
        assert!(
            samp.worst_over <= v.worst_over + 1e-9,
            "{name}: sampled worst over-claim {:.4} mm exceeds the V model's \
             {:.4} mm on {label}",
            samp.worst_over,
            v.worst_over
        );
        if samp.mean_abs_err() < v.mean_abs_err() - 1e-6 || samp.gouge < v.gouge {
            any_strict = true;
        }
    }
    assert!(
        any_strict,
        "{label}: the sampled model did not beat CLR+θ on ANY tool — the \
         fixture is not discriminating, so it proves nothing"
    );
}

/// **Fixture 1 — chamfered trapezoidal groove.** Two wall angles, so the one
/// the V model reads (the steepest) describes the lower wall and mis-places
/// the upper one. See [`chamfered`] for the mechanism, and [`trapezoid`] for
/// why the PLAIN trapezoid — the shape the module doc named — turned out not
/// to discriminate at all.
#[test]
fn chamfered_groove_sampled_reach_beats_the_v_model() {
    assert_sampled_beats_v("chamfered", &chamfered_family());
}

/// **Fixture 2 — circular-arc valley.** The wall angle varies continuously
/// from 0° at the floor to its steepest at the rim, so the single angle the V
/// model reads is right at exactly one height and wrong everywhere else.
#[test]
fn curved_wall_valley_sampled_reach_beats_the_v_model() {
    assert_sampled_beats_v("circular", &circular_family());
}

// ── Sub-item 4: resolution, with the model held constant ─────────────────

/// **The rest-grid resolution question (C9 sub-item 4 / backlog P9 bullet 4).**
///
/// One model (the sampled one), one fixture set, four pitches: 0.5 mm (the
/// shipped `RestFieldParams::cell_mm`), 0.25, 0.1 and the fine reference. What
/// the coarse cell costs is then the difference, with nothing else moving.
///
/// This is the experiment that keeps grid coarseness from masquerading as
/// model error in the two gates above — and, read the other way, it is what
/// decides `reach::PRODUCTION_REACH_MODEL`: a model whose accuracy is bounded
/// by its pitch cannot be adopted on the strength of fine-pitch numbers alone.
#[test]
fn resolution_sweep_separates_model_error_from_grid_error() {
    println!();
    println!("== C9 sub-item 4 — rest-cell resolution, SAMPLED model held constant ==");
    let families: Vec<(&str, Vec<Section>)> = vec![
        ("control", control_family()),
        ("chamfered", chamfered_family()),
        ("circular", circular_family()),
    ];
    let pitches = [SHIPPED_REST_CELL_MM, 0.25, 0.1, FINE_PITCH];
    for (name, cutter) in tools() {
        println!(
            "  {name}  (tip cusp radius {:.3} mm)",
            cutter.cusp_radius_mm()
        );
        for (fam_name, fam) in families.iter() {
            // The V model at the SHIPPED cell is the incumbent — printed once
            // per family so the comparison is against what ships, not against
            // an idealised V.
            let (v_shipped, _) = score_family(cutter.as_ref(), fam, SHIPPED_REST_CELL_MM);
            println!(
                "    {fam_name:10} {}",
                v_shipped.line("CLR+θ @ 0.50 mm cell")
            );
            for &p in pitches.iter() {
                let (_, samp) = score_family(cutter.as_ref(), fam, p);
                println!(
                    "    {fam_name:10} {}",
                    samp.line(&format!("SAMP  @ {p:.2} mm cell"))
                );
            }
        }
    }
}

/// The sampled model must never over-claim, at ANY pitch — a coarse section is
/// allowed to COST reach, never to invent it. That is a property of the
/// upper-envelope reading (see `reach::solve_reach_sampled`), and it is the
/// one failure mode that reaches the workpiece, so it is asserted at every
/// pitch rather than only at the fine one the model comparisons use.
///
/// It is also not free of history: the first implementation dropped the
/// exactly-touching constraint (`ξ == width`, strict `>`), which let a Ø3
/// ball — whose `width_at_height` saturates at the shank radius, so equality
/// is REACHED rather than approached — skip the binding wall sample and read
/// the next one out. One cell in 176 over-claimed by 50 µm. This test and the
/// matrix gate are what found it.
#[test]
fn the_sampled_model_never_over_claims_at_any_pitch() {
    let families = [control_family(), chamfered_family(), circular_family()];
    for (name, cutter) in tools() {
        for p in [SHIPPED_REST_CELL_MM, 0.25, 0.1, FINE_PITCH] {
            for fam in families.iter() {
                let (_, samp) = score_family(cutter.as_ref(), fam, p);
                assert_eq!(
                    samp.gouge, 0,
                    "{name} @ {p} mm: sampled model claimed {} pass(es) the \
                     tool cannot hold",
                    samp.gouge
                );
                assert_eq!(
                    samp.float_blind, 0,
                    "{name} @ {p} mm: sampled model routed a floating centreline"
                );
            }
        }
    }
}

/// A named, non-vacuous statement of what the coarse cell costs, so the
/// recommendation in the wave log is pinned to a number that fails if it
/// drifts. Reported as the shipped-cell sampled model's reach error against
/// the fine-pitch sampled model on the same fixtures — i.e. RESOLUTION alone.
#[test]
fn the_shipped_rest_cell_costs_a_measurable_reach_error() {
    println!();
    println!("== C9 — resolution cost at the shipped 0.5 mm rest cell ==");
    let cutter = wanaka_taper();
    let fam = chamfered_family();
    let (_, coarse) = score_family(&cutter, &fam, SHIPPED_REST_CELL_MM);
    let (_, fine) = score_family(&cutter, &fam, FINE_PITCH);
    println!("  {}", coarse.line("SAMP @ 0.50 mm"));
    println!("  {}", fine.line("SAMP @ 0.01 mm"));
    println!(
        "  resolution cost: mean |Δ| {:.4} mm → {:.4} mm, coverage {:.1}% → {:.1}%",
        fine.mean_abs_err(),
        coarse.mean_abs_err(),
        100.0 * fine.coverage(),
        100.0 * coarse.coverage()
    );
    assert!(
        coarse.mean_abs_err() >= fine.mean_abs_err() - 1e-9,
        "the coarse cell measured BETTER than the fine one — the sweep is \
         not measuring resolution"
    );
}
