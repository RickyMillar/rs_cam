//! **M4 research phase A — validating the oracle before pointing it at anything.**
//!
//! The plan's phase A exists because scallop quality has been judged through
//! instruments with known artefacts. Replacing them with a *new* instrument is
//! only progress if the new one is checked against ground truth first — so
//! every test in this file scores a path whose correct answer is known in
//! closed form, and asserts the oracle reproduces it.
//!
//! What is pinned here, in order:
//!
//! | # | claim | ground truth |
//! |---|---|---|
//! | 1 | the oracle's max residual **is** the cusp | `h = R − √(R² − (d/2)²)` |
//! | 2 | its quantiles follow the closed-form CDF, so `p99 ≈ 0.98·h` is a fair robust stand-in | `r_q = R − √(R² − (q·d/2)²)` |
//! | 3 | the two discretisations (path resample step, grid cell) **converge** | monotone approach to (1) |
//! | 4 | a deliberate uniform gouge reads back at its exact depth | −0.100 mm |
//! | 5 | *never reached* and *reached but left high* are separated, and both are measured in mm² | a half-covered field, a stepped field |
//! | 6 | the M3 tile-raster true-surface reference is accurate enough not to be confused with a candidate's error | analytic dome |
//! | 7 | **the slope law**: to hold a constant surface-normal cusp the XY stepover must scale as `cos θ` | envelope of circles centred on an inclined line |
//! | 8 | the tapered-tool stamp radius cap is safe below the flank-contact slope | `90° − taper half-angle` |
//!
//! Claim 7 is not a housekeeping item. It is the measurement that turns M4's
//! "min-across-ring is the culprit" hypothesis into a testable competition,
//! because `scallop_math::variable_stepover` scales the stepover the OTHER
//! way (`R/cos θ`, i.e. wider on slope). See `scallop_candidates_m4.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout)]

mod common;

use common::scallop_oracle::{
    EnvelopeOracle, OracleGrid, OracleParams, StampKernel, analytic_raster, closed_form_cusp,
    closed_form_cusp_quantile, render_field,
};
use common::{meshes, tools};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::{BallEndmill, MillingCutter};

/// Ø1 ball: small enough that a 20 µm cusp needs only a 0.28 mm pitch, so the
/// validation fixtures stay cheap enough to run in a debug test binary.
fn probe_ball() -> BallEndmill {
    BallEndmill::new(1.0, 10.0)
}

const DIAL_MM: f64 = 0.020;

/// The pitch that yields exactly `DIAL_MM` on flat ground with `probe_ball`:
/// `d = 2√(2Rh − h²) = 2√(2·0.5·0.02 − 0.0004) = 0.28` exactly.
const FLAT_PITCH_MM: f64 = 0.28;

fn flat_grid(cell: f64) -> OracleGrid {
    OracleGrid {
        origin_x: -1.6,
        origin_y: -1.1,
        rows: (2.2 / cell).round() as usize + 1,
        cols: (3.2 / cell).round() as usize + 1,
        cell,
    }
}

/// Score a synthetic raster over a flat plane at `z = 0`, restricting the
/// scored window to the interior so the outermost passes' edges never enter
/// the statistics.
fn flat_arm(cell: f64, pitch: f64, path_step: f64) -> (EnvelopeOracle, f64) {
    let ball = probe_ball();
    let grid = flat_grid(cell);
    // Passes at x = −1.12 .. +1.12 (9 of them at 0.28); scored window is
    // strictly inside the outer pair.
    let tp = analytic_raster(-1.12, 1.12, -0.7, 0.7, pitch, path_step, |_, _| 0.0);
    let truth = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
        if x.abs() <= 1.0 && y.abs() <= 0.5 {
            0.0
        } else {
            f64::NAN
        }
    });
    let kernel = StampKernel::new(&ball, cell, None);
    let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, path_step);
    (oracle, closed_form_cusp(ball.radius(), pitch))
}

// ---------------------------------------------------------------------------
// 1 + 2 — the cusp and its quantile law
// ---------------------------------------------------------------------------

#[test]
fn flat_ground_max_residual_is_the_closed_form_cusp() {
    let cell = 0.005;
    let (oracle, expect) = flat_arm(cell, FLAT_PITCH_MM, cell);
    let rep = oracle.report(OracleParams::new(DIAL_MM));

    assert!(
        rep.scored_cells > 10_000,
        "{} cells scored",
        rep.scored_cells
    );
    assert_eq!(rep.untouched_mm2, 0.0, "the raster covers the whole window");

    let measured_mm = rep.resid_max_um / 1000.0;
    let err = (measured_mm - expect).abs() / expect;
    println!(
        "closed form {:.6} mm, oracle max {:.6} mm, error {:.2}%",
        expect,
        measured_mm,
        err * 100.0
    );
    assert!(
        err < 0.02,
        "oracle max must reproduce h = R − √(R² − (d/2)²) to 2%: \
         expected {expect:.6} mm, got {measured_mm:.6} mm ({:.2}% off)",
        err * 100.0
    );

    // And it must land on the dial, because the pitch was chosen to.
    assert!(
        (rep.cusp_ratio() - 0.98).abs() < 0.06,
        "p99 on a uniform field should sit at ~0.98 of the dial, got {:.3}",
        rep.cusp_ratio()
    );
}

#[test]
fn flat_ground_cusp_quantiles_match_closed_form() {
    let cell = 0.005;
    let (oracle, _) = flat_arm(cell, FLAT_PITCH_MM, cell);
    let r = probe_ball().radius();
    let mut resid: Vec<f64> = oracle
        .residuals(0.0)
        .into_iter()
        .filter(|v| !v.is_nan())
        .collect();
    resid.sort_by(f64::total_cmp);

    // On a uniform raster the residual over the cross-pass coordinate is
    // r(u) = R − √(R² − u²) with u uniform on [−d/2, d/2], so the q-quantile
    // is the value at |u| = q·d/2. This is the law that licenses reading p99
    // as "the cusp" — pinned, not assumed.
    for q in [0.50_f64, 0.90, 0.99] {
        let expect = closed_form_cusp_quantile(r, FLAT_PITCH_MM, q);
        let got = common::scallop_oracle::quantile(&resid, q);
        let err = (got - expect).abs() / expect;
        println!(
            "q={q:.2}  closed form {expect:.6}  oracle {got:.6}  err {:.2}%",
            err * 100.0
        );
        assert!(
            err < 0.06,
            "quantile {q} must follow the closed form: expected {expect:.6}, got {got:.6}"
        );
    }

    // The specific constant the oracle's doc claims.
    let ratio =
        closed_form_cusp_quantile(r, FLAT_PITCH_MM, 0.99) / closed_form_cusp(r, FLAT_PITCH_MM);
    assert!(
        (ratio - 0.98).abs() < 0.01,
        "p99/max on a uniform field is {ratio:.4}, the doc claims 0.98"
    );
}

// ---------------------------------------------------------------------------
// 3 — convergence of the oracle's own two discretisations
// ---------------------------------------------------------------------------

#[test]
fn oracle_converges_in_path_step_and_in_cell() {
    let expect = closed_form_cusp(probe_ball().radius(), FLAT_PITCH_MM);

    println!("\n| cell mm | path step mm | max residual mm | error % |");
    println!("|---|---|---|---|");
    let mut cell_errors = Vec::new();
    for &cell in &[0.020_f64, 0.010, 0.005] {
        let (oracle, _) = flat_arm(cell, FLAT_PITCH_MM, cell);
        let rep = oracle.report(OracleParams::new(DIAL_MM));
        let err = (rep.resid_max_um / 1000.0 - expect).abs() / expect;
        println!(
            "| {cell:.3} | {cell:.3} | {:.6} | {:.2} |",
            rep.resid_max_um / 1000.0,
            err * 100.0
        );
        cell_errors.push(err);
    }
    assert!(
        cell_errors[2] <= cell_errors[0] + 1e-9,
        "refining the cell must not make the oracle worse: {cell_errors:?}"
    );
    assert!(
        cell_errors[2] < 0.02,
        "at cell 0.005 the oracle must be within 2%: {:.3}%",
        cell_errors[2] * 100.0
    );

    // Path step: too coarse and the envelope shows phantom ridges BETWEEN
    // samples along the pass. Refining it must converge on the same answer.
    let mut step_maxima = Vec::new();
    for &step in &[0.080_f64, 0.020, 0.005] {
        let (oracle, _) = flat_arm(0.005, FLAT_PITCH_MM, step);
        let rep = oracle.report(OracleParams::new(DIAL_MM));
        println!(
            "| 0.005 | {step:.3} | {:.6} | {:.2} |",
            rep.resid_max_um / 1000.0,
            (rep.resid_max_um / 1000.0 - expect).abs() / expect * 100.0
        );
        step_maxima.push(rep.resid_max_um / 1000.0);
    }
    assert!(
        step_maxima[0] > step_maxima[2],
        "a coarse path step must OVER-report the cusp (phantom along-pass \
         ridges); got {step_maxima:?}"
    );
    assert!(
        (step_maxima[2] - expect).abs() / expect < 0.02,
        "path step at the cell size must be converged: {:.6} vs {expect:.6}",
        step_maxima[2]
    );
}

// ---------------------------------------------------------------------------
// 4 — a known gouge
// ---------------------------------------------------------------------------

#[test]
fn a_known_uniform_gouge_reads_back_at_its_exact_depth() {
    let cell = 0.005;
    let ball = probe_ball();
    let grid = flat_grid(cell);
    // The path is draped 0.100 mm BELOW the surface it is scored against.
    let tp = analytic_raster(-1.12, 1.12, -0.7, 0.7, FLAT_PITCH_MM, cell, |_, _| -0.100);
    let truth = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
        if x.abs() <= 1.0 && y.abs() <= 0.5 {
            0.0
        } else {
            f64::NAN
        }
    });
    let kernel = StampKernel::new(&ball, cell, None);
    let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, cell);
    let rep = oracle.report(OracleParams::new(DIAL_MM));

    println!(
        "deepest gouge {:.1} µm (expected −100.0), gouge area {:.3} mm²",
        rep.deepest_gouge_um, rep.gouge_mm2
    );
    assert!(
        (rep.deepest_gouge_um + 100.0).abs() < 1.0,
        "a uniform 100 µm gouge must read back as −100 µm, got {:.2}",
        rep.deepest_gouge_um
    );
    // Everything is gouged, so nothing is on dial and nothing stands.
    assert!(rep.on_dial_frac < 0.01, "{}", rep.on_dial_frac);
    assert_eq!(rep.standing_mm2, 0.0);
}

// ---------------------------------------------------------------------------
// 5 — untouched vs standing, the distinction M4 turns on
// ---------------------------------------------------------------------------

#[test]
fn untouched_and_standing_are_measured_separately() {
    let cell = 0.010;
    let ball = probe_ball();
    let grid = flat_grid(cell);

    // (a) The path only covers x ≤ 0. The x > 0 half is NEVER REACHED — this
    //     is the shape `max_rings` truncation produces, and it must NOT be
    //     reported as a cusp defect.
    let tp = analytic_raster(-1.12, -0.02, -0.7, 0.7, FLAT_PITCH_MM, cell, |_, _| 0.0);
    let truth = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
        if x.abs() <= 1.0 && y.abs() <= 0.5 {
            0.0
        } else {
            f64::NAN
        }
    });
    let kernel = StampKernel::new(&ball, cell, None);
    let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, cell);
    let rep = oracle.report(OracleParams::new(DIAL_MM));

    // Scored window is 2.0 × 1.0 mm = 2.0 mm²; the tool has radius so it
    // reaches ~0.5 mm past its last pass. Untouched must be the bulk of the
    // right half and standing must be zero.
    println!(
        "half-covered: untouched {:.3} mm², standing {:.3} mm², scored {:.3} mm²",
        rep.untouched_mm2,
        rep.standing_mm2,
        rep.scored_cells as f64 * rep.cell_area_mm2
    );
    assert!(
        rep.untouched_mm2 > 0.3,
        "never-reached material must be measured, got {:.3} mm²",
        rep.untouched_mm2
    );
    // The band immediately past the last pass IS touched — by the ball's
    // rim, which leaves a fillet standing well above the surface. That the
    // oracle calls it STANDING and the ground beyond it UNTOUCHED is the
    // distinction working, not a leak: the rim band is `R` wide (0.5 mm here)
    // and must stay a minority of the defect.
    assert!(
        rep.standing_mm2 > 0.0,
        "the ball's rim leaves a standing fillet past the last pass"
    );
    assert!(
        rep.untouched_mm2 > rep.standing_mm2 * 2.0,
        "the never-reached half must dominate the rim band: untouched \
         {:.3} mm² vs standing {:.3} mm²",
        rep.untouched_mm2,
        rep.standing_mm2
    );

    // (b) A path that DOES cover the field but sits 0.5 mm high over half of
    //     it: reached, and left standing.
    let tp2 = analytic_raster(-1.12, 1.12, -0.7, 0.7, FLAT_PITCH_MM, cell, |x, _| {
        if x > 0.0 { 0.5 } else { 0.0 }
    });
    let truth2 = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
        if x.abs() <= 1.0 && y.abs() <= 0.5 {
            0.0
        } else {
            f64::NAN
        }
    });
    let oracle2 = EnvelopeOracle::score(grid, truth2, &tp2, &kernel, cell);
    let rep2 = oracle2.report(OracleParams::new(DIAL_MM));
    println!(
        "stepped: untouched {:.3} mm², standing {:.3} mm²",
        rep2.untouched_mm2, rep2.standing_mm2
    );
    assert_eq!(rep2.untouched_mm2, 0.0, "every cell was covered");
    assert!(
        rep2.standing_mm2 > 0.7,
        "half the 2 mm² window is left 0.5 mm high; standing = {:.3} mm²",
        rep2.standing_mm2
    );

    render_field(
        &grid,
        &oracle2.residuals(0.0),
        200.0,
        "validation_stepped_residual",
    );
}

// ---------------------------------------------------------------------------
// 6 — the true-surface reference's own error budget
// ---------------------------------------------------------------------------

#[test]
fn tile_raster_true_surface_tracks_the_analytic_surface() {
    // A dome, meshed at 0.1 mm, sampled at 0.05 mm. The reference's error is
    // the MESH's faceting, not the sampler's — which is the point: any
    // candidate difference smaller than this is not a real difference.
    let mesh = meshes::height_field(6.0, 0.1, |x, y| {
        let r2 = x * x + y * y;
        (36.0 - r2).max(0.0).sqrt() - 6.0
    });
    let index = SpatialIndex::build(&mesh, 2.0);
    let ball = probe_ball();
    let grid = OracleGrid::for_mesh(&mesh, &ball, 0.05);

    let sampled = EnvelopeOracle::true_surface_from_mesh(grid, &mesh, &index);
    let analytic = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
        let r2 = x * x + y * y;
        if r2 <= 16.0 {
            (36.0 - r2).max(0.0).sqrt() - 6.0
        } else {
            f64::NAN
        }
    });

    let mut errs: Vec<f64> = sampled
        .iter()
        .zip(&analytic)
        .filter(|(s, a)| !s.is_nan() && !a.is_nan())
        .map(|(s, a)| (s - a).abs())
        .collect();
    assert!(errs.len() > 10_000, "{} comparable cells", errs.len());
    errs.sort_by(f64::total_cmp);
    let p50 = common::scallop_oracle::quantile(&errs, 0.50) * 1000.0;
    let p99 = common::scallop_oracle::quantile(&errs, 0.99) * 1000.0;
    println!(
        "true-surface reference error: p50 {p50:.2} µm, p99 {p99:.2} µm over {} cells",
        errs.len()
    );
    assert!(
        p99 < 20.0,
        "the M3 tile-raster reference must be good to well under a dial on a \
         0.1 mm mesh; p99 = {p99:.2} µm"
    );
}

// ---------------------------------------------------------------------------
// 7 — THE SLOPE LAW
// ---------------------------------------------------------------------------

/// On a plane inclined at θ, the tool CENTRES sit on a line inclined at θ, so
/// an XY stepover `d` puts adjacent centres `d·sec θ` apart along that line.
/// The material left, measured NORMAL to the plane, is therefore
/// `R − √(R² − (d·sec θ / 2)²)` — strictly more than the flat-ground cusp for
/// the same `d`.
///
/// Consequence, and this is the number M4 needs: **to hold a constant
/// surface-normal cusp, the XY stepover must be scaled by `cos θ`.**
///
/// `scallop_math::variable_stepover` scales it by `1/√cos θ` instead (it
/// computes `R_eff = R/cos θ` and feeds that to the flat formula), so it opens
/// the stepover on slope where the geometry closes it. This test is the
/// ground-truth measurement that claim rests on.
///
/// # The margin, learned the hard way
///
/// The pass that makes tangent contact at horizontal position `x` is the one
/// `R·sin θ` **downhill** of it, not the one above it. A scored window that
/// runs to the last pass therefore reports an edge artefact, not the cusp —
/// the first draft of this test read 1.76× the law at 45° for exactly that
/// reason. The raster is extended `R·sin θ + d` past the window on both sides.
#[test]
fn inclined_plane_cusp_follows_the_secant_law() {
    let ball = probe_ball();
    let r = ball.radius();
    let cell = 0.005_f64;
    let pitch = FLAT_PITCH_MM;

    println!(
        "\n| slope ° | predicted normal cusp mm | oracle normal cusp mm | err % | vs flat dial |"
    );
    println!("|---|---|---|---|---|");

    // 60° is the last slope this fixture can adjudicate: at 75° a 0.28 mm XY
    // stepover is 1.08 mm along a surface a Ø1 ball can only span 1.0 mm of,
    // so the closed form saturates at R and the "cusp" is a gap, not a scallop.
    // That regime boundary is itself the law's statement about steep ground.
    for &deg in &[0.0_f64, 15.0, 30.0, 45.0, 60.0] {
        let th = deg.to_radians();
        let (tan, sec) = (th.tan(), 1.0 / th.cos());
        // Drop-cutter drape: a ball tangent to a plane at slope θ has its TIP
        // at plane_z + R(sec θ − 1), not on the plane.
        let drape = move |x: f64, _y: f64| -x * tan + r * (sec - 1.0);
        let plane = move |x: f64, _y: f64| -x * tan;

        let grid = OracleGrid {
            origin_x: -2.6,
            origin_y: -1.1,
            rows: (2.2 / cell).round() as usize + 1,
            cols: (5.2 / cell).round() as usize + 1,
            cell,
        };
        let tp = analytic_raster(-1.96, 1.96, -0.7, 0.7, pitch, cell, drape);
        let truth = EnvelopeOracle::true_surface_analytic(grid, |x, y| {
            if x.abs() <= 1.0 && y.abs() <= 0.5 {
                plane(x, y)
            } else {
                f64::NAN
            }
        });
        let kernel = StampKernel::new(&ball, cell, None);
        let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, cell);

        // Max NORMAL residual = max vertical residual × cos θ.
        let max_vert = oracle
            .residuals(0.0)
            .into_iter()
            .filter(|v| !v.is_nan())
            .fold(f64::NEG_INFINITY, f64::max);
        let measured_normal = max_vert * th.cos();
        let predicted = closed_form_cusp(r, pitch * sec);
        let err = (measured_normal - predicted).abs() / predicted;

        println!(
            "| {deg:.0} | {predicted:.6} | {measured_normal:.6} | {:.2} | {:.2}× |",
            err * 100.0,
            measured_normal / DIAL_MM
        );
        assert!(
            err < 0.06,
            "slope {deg}°: the secant law predicts {predicted:.6} mm, oracle \
             measured {measured_normal:.6} mm ({:.1}% off)",
            err * 100.0
        );
    }

    // The law restated as the stepover rule, which is what a candidate has to
    // implement. Non-vacuity: it must actually DISAGREE with the shipped
    // formula, and in the opposite direction.
    for &deg in &[30.0_f64, 45.0, 60.0] {
        let th = deg.to_radians();
        let required = pitch * th.cos();
        let shipped = rs_cam_core::scallop_math::variable_stepover(r, DIAL_MM, th, 0.0);
        println!(
            "slope {deg}°: geometry requires stepover {required:.4} mm, \
             variable_stepover returns {shipped:.4} mm ({:.2}× too wide)",
            shipped / required
        );
        assert!(
            shipped > required * 1.2,
            "the shipped slope term must be measurably WIDER than geometry \
             allows at {deg}° — required {required:.4}, shipped {shipped:.4}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8 — the tapered stamp radius cap
// ---------------------------------------------------------------------------

/// A tapered ball's envelope is its SHANK (Ø6 on the wanaka tool), so a full
/// stamp kernel is 36× the area of a tip-only one. Capping the stamp is safe
/// exactly while the flank cannot touch: a taper of half-angle `α` contacts
/// its flank only where the surface slope exceeds `90° − α` (83° for the
/// shipped 7° tool). This pins that, so a harness that caps knows its limit.
#[test]
fn taper_stamp_radius_cap_is_safe_below_the_flank_contact_slope() {
    let taper = tools::wanaka_taper();
    let r_tip = taper.cusp_radius_mm();
    let cell = 0.010_f64;

    let full = StampKernel::new(&taper, cell, None);
    let capped = StampKernel::new(&taper, cell, Some(2.0 * r_tip));
    println!(
        "taper envelope {:.2} mm ({} stamp cells) vs cap {:.2} mm ({} cells) — {:.1}× cheaper",
        taper.envelope_radius_mm(),
        full.cells(),
        2.0 * r_tip,
        capped.cells(),
        full.cells() as f64 / capped.cells() as f64
    );
    assert!(
        capped.cells() * 8 < full.cells(),
        "the cap must actually save work: {} vs {} cells",
        capped.cells(),
        full.cells()
    );

    // At 60° the tip alone carries the contact, so the two kernels must agree.
    for &deg in &[0.0_f64, 30.0, 60.0] {
        let th = deg.to_radians();
        let (tan, sec) = (th.tan(), 1.0 / th.cos());
        let drape = move |x: f64, _y: f64| -x * tan + r_tip * (sec - 1.0);
        let grid = OracleGrid {
            origin_x: -1.6,
            origin_y: -1.1,
            rows: (2.2 / cell).round() as usize + 1,
            cols: (3.2 / cell).round() as usize + 1,
            cell,
        };
        let tp = analytic_raster(-1.12, 1.12, -0.7, 0.7, 0.20, cell, drape);
        let truth = |g: OracleGrid| {
            EnvelopeOracle::true_surface_analytic(g, |x, y| {
                if x.abs() <= 1.0 && y.abs() <= 0.5 {
                    -x * tan
                } else {
                    f64::NAN
                }
            })
        };
        let a = EnvelopeOracle::score(grid, truth(grid), &tp, &full, cell)
            .report(OracleParams::new(DIAL_MM));
        let b = EnvelopeOracle::score(grid, truth(grid), &tp, &capped, cell)
            .report(OracleParams::new(DIAL_MM));
        println!(
            "  slope {deg:>2}°: full max {:.2} µm, capped max {:.2} µm",
            a.resid_max_um, b.resid_max_um
        );
        assert!(
            (a.resid_max_um - b.resid_max_um).abs() < 1.0,
            "below the flank-contact slope (83° for this 7° taper) the cap \
             must not change the answer: {deg}° gave {:.3} vs {:.3} µm",
            a.resid_max_um,
            b.resid_max_um
        );
    }
}
