//! F1 (P5.1, 2026-09-08) — WHERE the reach map's residual comes from, on
//! surfaces whose answer is known in closed form.
//!
//! # The report that started it
//!
//! On the wanaka terrain (661 212 triangles, Ø4 tapered ball R2.0, raster
//! stepover 1.5 mm) `reach_map` painted almost the whole model red: **68.2 %**
//! of the measured area unreachable at the 0.05 mm default, worst gap 4.35 mm,
//! grid cell 0.645 mm. `preview_tier_map` handed roughly **24 %** of the same
//! board down to finer tools. Two instruments, one tool, one model, and a
//! threefold disagreement.
//!
//! # What the investigation found, before any code moved
//!
//! An independent instrument rasterised that STL onto a 0.15 mm lattice by
//! exact barycentric plane evaluation and took the grayscale closing by the
//! ball profile — the same law this module implements, sharing no code with
//! it. Cross-checks: the rasterised mesh's 3D area came out at 56 295 mm²,
//! the figure `reach_map` itself reports, and the worst gap agreed to three
//! figures (4.32 against 4.35 mm).
//!
//! | tolerance | by cell | by 3D area |
//! |-----------|---------|-----------|
//! | 0.05 mm   | 50.5 %  | 55.0 %  |
//! | 0.146 mm  | 35.2 %  | 39.5 %  |
//! | 0.30 mm   | 21.9 %  | 25.2 %  |
//!
//! **P5.2 correction: that table is on the WRONG BASE and must not be
//! compared with the map's numbers.** It weights by a gradient-derived cell
//! area over the WHOLE board. The map weights by true triangle 3D area over
//! the RIM-ERODED population. Put on the map's own base the same closing
//! reads:
//!
//! | tolerance | truth, map's base | map (cell 0.645) |
//! |-----------|-------------------|------------------|
//! | 0.05 mm   | **58.6 %**        | 59.05 %          |
//! | 0.146 mm  | **42.1 %**        | 51.11 %          |
//! | 0.30 mm   | **26.8 %**        | 36.36 %          |
//!
//! So 3.5 of the apparent 12-point offset was the rasteriser's own base. The
//! rest is the discretisation this file already documents, and it survives
//! the tolerance because it is additive in the GAP, not in the percentage:
//! over the same mask the truth's gaps run median 0.064 / p90 0.591 and the
//! map's scheme runs median 0.118 / p90 0.725, and shifting the truth's own
//! CDF by that +0.054 mm predicts 48.5 % at the 0.146 bar against the
//! replicated 48.85 %. The TAPER contributes +0.01 pp — the Ø4 tool's cone
//! rises 19 mm per mm of radius, so it almost never rests on a neighbouring
//! flank; only 0.13 % of cells see any change from it.
//!
//! Three conclusions, in the order they matter:
//!
//! 1. **68.2 % is not an artefact.** The true answer is 55 %. That terrain's
//!    own facet roughness puts its MEDIAN gap (0.051 mm) exactly at the
//!    0.05 mm bar, and 48.5 % of its x-axis curvature radii are under
//!    2.0 mm — under the tool's own tip radius. The "broad smooth slopes a
//!    R2.0 ball must form" the report expected do not exist on this mesh at
//!    that bar. The lever for that is the BAR, not the algorithm: F2 derives
//!    it from the raster's own cusp, 0.146 mm here, where the true answer is
//!    39.5 %.
//! 2. **Hypothesis (a) as written is refuted.** A cell sweep of the whole
//!    closing gave 37.1 % (0.9 mm), 43.0 % (0.645), 47.0 % (0.4), 48.8 %
//!    (0.3), 50.0 % (0.2), 50.5 % (0.15). The share does not *fall* with a
//!    finer cell — it RISES and converges. A coarser grid understates.
//! 3. **The residual is discretisation, and the operator surfaces never said
//!    so.** Replicating this module's own sampling scheme at 0.645 mm
//!    reproduced 63.7 % with a median gap of 0.101 mm against a true
//!    0.051 mm, and it converged on refinement (59.5 % at 0.4 mm, 58.6 % at
//!    0.3 mm). Note carefully what the floor was doing: `sampling_floor_mm`
//!    published **0.132 mm** for that tool and cell — already far above the
//!    0.05 mm bar it was being read against — so the number needed to
//!    distrust the reading was on the wire, and **no surface compared the
//!    two**. The verdict counted the whole sub-floor band as unreachable.
//!    The fix therefore does two things: the floor now also measures the
//!    curvature term (0.234 mm on the same case, 1.8x the plane-only
//!    figure), and the verdict ABSTAINS on the band between the bar and the
//!    floor instead of calling it tool geometry.
//!
//! # Measured before and after, same tool, same cell (0.645 mm)
//!
//! | tolerance | reported before | reported after | unresolved after | truth |
//! |---|---|---|---|---|
//! | 0.05 mm  | 68.2 % | **59.05 %** | 9.14 % | 55.0 % |
//! | 0.146 mm | —      | **51.11 %** | 1.71 % | 39.5 % |
//! | 0.30 mm  | —      | **36.36 %** | 0.03 % | 25.2 % |
//!
//! The error against the independent truth at the default bar falls from
//! +13.2 pp to +4.05 pp. What remains is the kink regime the floor does not
//! bound — see `curvature_floor_plane`. Reproduce the table with
//! `the_wanaka_terrain_reach_table` below.
//!
//! # What these fixtures pin
//!
//! Every one has a closed-form answer, so a failure names arithmetic rather
//! than taste:
//!
//! * a smooth sinusoid whose minimum radius of curvature (6.1 mm) is three
//!   times the tool's, read at three cells — the hypothesis-(a) fixture;
//! * a 45° plane, exact for any profile;
//! * a 90° V-groove, apex gap `R·(csc 45° − 1)` = 0.828 mm for R2;
//! * **a concave trough of radius 3 mm, which a R2.0 ball fits exactly** —
//!   true gap zero everywhere, and the fixture that reproduces the wanaka
//!   mechanism in isolation, because `tip_z` there has curvature
//!   `1/(ρ − R)` = 1.0 /mm and the interpolation error is
//!   `cell²/8` = 0.052 mm at 0.645 mm cells, the whole default tolerance;
//! * a concave trough too tight for the same ball, whose bridge gap is
//!   `h_a + sqrt(R² − a²) − R` — a REAL miss the abstention must not swallow,
//!   and the fixture that found the floor's regime limit: `tip_z` KINKS where
//!   the ball's contact switches rim, so the over-read there is
//!   `O(cell · Δslope)` and the curvature floor, a C² bound, does not cover
//!   it (`curvature_floor_plane` carries the measured table);
//! * the tier map on the V-groove, to show what the two instruments each
//!   answer.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// This file is an instrument as well as a gate: the F1 tables are printed so
// a re-run reproduces the numbers in the doc above rather than only asserting
// on them. `print_stderr` is denied workspace-wide, including in tests.
#![allow(clippy::print_stderr)]

use rs_cam_core::geo::P3;
use rs_cam_core::maps::reach_map::{ReachMap, reach_map_for_mesh};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};

// ── fixtures ───────────────────────────────────────────────────────────────

/// A height field `z = f(x)`, constant along Y, tessellated at `pitch` in X.
///
/// Constant in Y on purpose: it isolates the curvature to one axis, so a
/// failure cannot be blamed on a two-axis interaction. Y is tessellated
/// coarsely because the surface does not vary there — the triangles are
/// slivers whose centroid X is still resolved to `pitch`.
fn ridges(half_width: f64, length: f64, pitch: f64, f: impl Fn(f64) -> f64) -> TriangleMesh {
    let cols = ((2.0 * half_width) / pitch).ceil().max(2.0) as usize;
    let rows = (length / 1.0).ceil().max(1.0) as usize;
    let mut vertices = Vec::with_capacity((cols + 1) * (rows + 1));
    for r in 0..=rows {
        let y = length * r as f64 / rows as f64;
        for c in 0..=cols {
            let x = -half_width + 2.0 * half_width * c as f64 / cols as f64;
            vertices.push(P3::new(x, y, f(x)));
        }
    }
    let stride = cols + 1;
    let mut triangles = Vec::with_capacity(cols * rows * 2);
    for r in 0..rows {
        for c in 0..cols {
            let a = (r * stride + c) as u32;
            let b = a + 1;
            let d = ((r + 1) * stride + c) as u32;
            let e = d + 1;
            triangles.push([a, b, e]);
            triangles.push([a, e, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A sinusoidal ridge field of wavelength `lambda` and amplitude `amp`.
///
/// Minimum radius of curvature at crest and trough is
/// `lambda² / (4 pi² amp)`. `lambda = 12`, `amp = 0.6` gives **6.08 mm**, so a
/// R2.0 ball fits everywhere and the true gap is zero over the whole field.
fn sinusoid(lambda: f64, amp: f64, half_width: f64, pitch: f64) -> TriangleMesh {
    let k = std::f64::consts::TAU / lambda;
    ridges(half_width, 20.0, pitch, move |x| amp * (k * x).sin())
}

/// Minimum radius of curvature of that sinusoid, in mm.
fn sinusoid_min_curvature_radius(lambda: f64, amp: f64) -> f64 {
    let k = std::f64::consts::TAU / lambda;
    1.0 / (amp * k * k)
}

/// A concave circular trough of radius `rho`, spanning `|x| <= a`, flanked by
/// flat pads at the rim height so the tool has somewhere to stand.
///
/// The trough bottom is at `z = 0`; the rim sits at
/// `h_a = rho - sqrt(rho² - a²)`. The arc-to-pad junction is CONVEX (the
/// slope drops from `a/sqrt(rho² - a²)` to zero), and a ball reproduces a
/// convex feature exactly, so the whole fixture's true gap is set by the
/// trough alone.
fn trough(rho: f64, a: f64, pad: f64, pitch: f64) -> TriangleMesh {
    let h_a = rho - (rho * rho - a * a).max(0.0).sqrt();
    ridges(a + pad, 20.0, pitch, move |x| {
        if x.abs() <= a {
            rho - (rho * rho - x * x).max(0.0).sqrt()
        } else {
            h_a
        }
    })
}

/// The gap a ball of radius `R` leaves at the bottom of a [`trough`] it is
/// too big for: it bridges the two rims at `(±a, h_a)`, so its centre sits at
/// `h_a + sqrt(R² − a²)` and its tip `R` below that.
///
/// `None` when `R <= a`, where the ball reaches inside the arc instead of
/// bridging it.
fn trough_bridge_gap_mm(radius: f64, rho: f64, a: f64) -> Option<f64> {
    if radius <= a || radius <= rho {
        return None;
    }
    let h_a = rho - (rho * rho - a * a).max(0.0).sqrt();
    Some(h_a + (radius * radius - a * a).sqrt() - radius)
}

/// A V trench along Y, apex on `x = 0`, walls `half_angle_deg` from the
/// vertical centreline — so an included angle of `2 * half_angle_deg`.
fn v_trench(half_width: f64, half_angle_deg: f64, pitch: f64) -> TriangleMesh {
    let cot = 1.0 / half_angle_deg.to_radians().tan();
    ridges(half_width, 20.0, pitch, move |x| x.abs() * cot)
}

/// The closed form for a ball of radius `R` bottomed out in a V whose walls
/// stand `beta` from the vertical centreline: `R·(csc beta − 1)`.
fn apex_gap_mm(radius_mm: f64, half_angle_deg: f64) -> f64 {
    radius_mm * (1.0 / half_angle_deg.to_radians().sin() - 1.0)
}

/// One table row, so a re-run reprints the F1 evidence.
fn row(label: &str, map: &ReachMap) {
    eprintln!(
        "  {label:38} cell {:.3}  unreachable {:6.2} %  unresolved {:6.2} %  \
         max gap {:.4}  floor {:.4} (profile {:.4} / curvature p95 {:.4})  \
         measured {:.0}/{:.0} mm2",
        map.grid.cell_mm,
        map.unreachable_pct(),
        map.unresolved_pct(),
        map.max_gap_mm,
        map.discretisation_floor_mm,
        map.profile_floor_mm,
        map.curvature_floor_p95_mm,
        map.measured_area_mm2,
        map.surface_area_mm2,
    );
}

// ── hypothesis (a): the cell, on a surface the tool fits ────────────────

/// A smooth field whose curvature is three times gentler than the tool's own
/// tip reads CLEAN at every cell, and the percentage does not walk.
///
/// This is the brief's hypothesis-(a) fixture. It is here to be *falsified*,
/// and it is: the bilinear error on this surface is
/// `cell² / (8·(ρ − R)) = cell² / 32.6`, which is 0.013 mm even at the
/// coarsest cell — a quarter of the bar. Discretisation alone therefore does
/// not explain a 68 % reading, and the brief's "if the % falls strongly with
/// cell, this is the mechanism" test comes back negative.
///
/// The mesh pitch is held FINE and fixed while only the cell moves. Facet
/// chord error on a 6.1 mm radius at a 0.2 mm vertex pitch is
/// `p²/(8ρ)` = 0.0008 mm; a coarse fixture mesh would show gap of its own and
/// it would be attributed to the grid.
#[test]
fn a_smooth_field_the_ball_fits_reads_clean_at_every_cell() {
    let (lambda, amp) = (12.0, 0.6);
    let rho = sinusoid_min_curvature_radius(lambda, amp);
    let ball = BallEndmill::new(4.0, 25.0);
    assert!(
        rho > 3.0 * ball.cusp_radius_mm(),
        "the fixture must be far gentler than the tool: rho {rho:.3} mm against \
         R {:.3} mm",
        ball.cusp_radius_mm()
    );
    let mesh = sinusoid(lambda, amp, 20.0, 0.2);

    eprintln!(
        "F1 (a) sinusoid lambda {lambda} amp {amp}, min curvature radius {rho:.3} mm, \
         R{:.1} ball, tol 0.05 mm \u{2014} TRUE gap is zero everywhere:",
        ball.cusp_radius_mm()
    );
    let mut shares = Vec::new();
    for cell in [0.2, 0.4, 0.645] {
        let map = reach_map_for_mesh(&mesh, &ball, 0.05, cell);
        row(&format!("cell {cell}"), &map);
        assert!(map.is_measured(), "cell {cell} produced no population");
        shares.push(map.unreachable_pct());
        assert!(
            map.unreachable_pct() < 1.0,
            "a surface three times gentler than the tool must not read \
             unreachable; cell {cell} gave {:.2} % (max gap {:.4} mm, floor \
             {:.4} mm)",
            map.unreachable_pct(),
            map.max_gap_mm,
            map.discretisation_floor_mm
        );
    }
    let span = shares.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - shares.iter().copied().fold(f64::INFINITY, f64::min);
    // The brief's bar, in its own words: "a percentage must not move by 30
    // points between cells on a surface whose true answer is 0".
    assert!(
        span < 1.0,
        "the percentage walked {span:.2} points across cells 0.2/0.4/0.645 on a \
         surface whose true answer is 0: {shares:?}"
    );
}

// ── hypothesis (b): mixed sampling, on closed-form geometry ─────────────

/// A 45° plane is exactly reachable, for a spherical tip and a flat one
/// alike, and its curvature floor is exactly zero.
///
/// The floor being zero is the load-bearing half here. `tip_z` is LINEAR over
/// a plane, so the second difference the curvature term measures vanishes —
/// which is what makes the term safe to gate a verdict on. A curvature floor
/// that read positive on a plane would abstain over every flank of every
/// part.
#[test]
fn a_plane_is_exact_and_its_curvature_floor_is_zero() {
    let mesh = ridges(20.0, 20.0, 0.2, |x| x * 45.0_f64.to_radians().tan());
    eprintln!("F1 (b) 45 deg plane \u{2014} TRUE gap is zero:");
    let ball: Box<dyn MillingCutter> = Box::new(BallEndmill::new(4.0, 25.0));
    let flat: Box<dyn MillingCutter> = Box::new(FlatEndmill::new(6.0, 25.0));
    for (label, cutter) in [("R2.0 ball", &ball), ("\u{00D8}6 flat", &flat)] {
        let map = reach_map_for_mesh(&mesh, cutter.as_ref(), 0.05, 0.4);
        row(label, &map);
        // A fifth of the bar, not literally zero, and the slack is named.
        // `tip_z` is exactly linear over the plane's interior, but the
        // quantile's population is the REPORTED cells and the innermost of
        // those sit one erosion width in, where a neighbour two cells further
        // out has the ball resting on the part EDGE rather than on the plane.
        // That ring is a small tail, so p95 lands in the linear interior; the
        // claim is that a plane accumulates no floor worth reading, not that
        // it accumulates none at all.
        assert!(
            map.curvature_floor_p95_mm < 0.01,
            "{label}: tip_z is linear over a plane, so the curvature floor must \
             stay well under the 0.05 mm bar; got {:.6} mm",
            map.curvature_floor_p95_mm
        );
        assert!(
            map.unreachable_pct() < 0.5,
            "{label}: a 45 deg plane must read reachable; got {:.3} % \
             (max gap {:.4} mm)",
            map.unreachable_pct(),
            map.max_gap_mm
        );
    }
}

/// A 90° V-groove reads its analytic apex gap, and the abstention does not
/// swallow it.
#[test]
fn a_ninety_degree_v_groove_reads_its_closed_form_apex_gap() {
    let half_angle = 45.0_f64;
    let ball = BallEndmill::new(4.0, 25.0);
    let expected = apex_gap_mm(ball.cusp_radius_mm(), half_angle);
    let mesh = v_trench(10.0, half_angle, 0.1);
    let map = reach_map_for_mesh(&mesh, &ball, 0.05, 0.4);
    eprintln!(
        "F1 (b) 90 deg V-groove, R{:.1} ball \u{2014} analytic apex gap \
         {expected:.4} mm:",
        ball.cusp_radius_mm()
    );
    row("V-groove", &map);
    assert!(
        (map.max_gap_mm - expected).abs() < 0.06,
        "the apex gap must be R(csc 45 - 1) = {expected:.4} mm; got {:.4} mm",
        map.max_gap_mm
    );
    assert!(
        map.unreachable_pct() > 1.0,
        "a 0.83 mm miss is sixteen times the bar and must be reported as \
         unreachable, not abstained on; got {:.3} % unreachable and {:.3} % \
         unresolved with a {:.4} mm floor",
        map.unreachable_pct(),
        map.unresolved_pct(),
        map.discretisation_floor_mm
    );
}

/// **The F1 fixture.** A concave trough a R2.0 ball fits EXACTLY — true gap
/// zero everywhere — reproduces the wanaka mechanism in isolation, and the
/// map must not call it unreachable.
///
/// `tip_z` over this trough is the ball centre's own locus, a circle of
/// radius `ρ − R` = 1 mm, so its curvature is 1.0 /mm and the sampled
/// minimum over-reads by about `cell²/8`. That is **0.052 mm at a 0.645 mm
/// cell** — a hair over the 0.05 mm default bar, on a surface the tool forms
/// perfectly. Before the F1 fix this fixture read unreachable at the shipped
/// cell, and nothing on any operator surface compared the bar against the
/// floor — which is exactly how a terrain full of `ρ` slightly over `R` came
/// back red.
#[test]
fn a_trough_the_ball_fits_is_never_called_unreachable() {
    let (rho, a) = (3.0, 2.4);
    let ball = BallEndmill::new(4.0, 25.0);
    assert!(
        rho > ball.cusp_radius_mm(),
        "the fixture must be a trough the ball FITS"
    );
    assert!(
        trough_bridge_gap_mm(ball.cusp_radius_mm(), rho, a).is_none(),
        "the ball must reach inside the arc, not bridge it"
    );
    let mesh = trough(rho, a, 6.0, 0.1);

    eprintln!(
        "F1 concave trough rho {rho} (R{:.1} ball FITS, true gap 0), predicted \
         over-read cell^2/(8(rho-R)):",
        ball.cusp_radius_mm()
    );
    for cell in [0.3, 0.4, 0.645] {
        let map = reach_map_for_mesh(&mesh, &ball, 0.05, cell);
        let predicted = cell * cell / (8.0 * (rho - ball.cusp_radius_mm()));
        eprintln!("    predicted over-read at cell {cell}: {predicted:.4} mm");
        row(&format!("cell {cell}"), &map);
        assert!(map.is_measured(), "cell {cell} produced no population");
        // The contract of the fix: a surface the tool forms is never
        // reported as unreachable, at any cell the map is allowed to pick.
        //
        // 2 %, not 0 %, and the slack is named rather than tuned. The floor
        // bounds the BILINEAR term exactly; the polar tap set's own pitch
        // adds up to `(pitch/cell)²` more of the same curvature, which the
        // 3 x 3 dilation absorbs in practice but is not *proved* to. Before
        // the fix this fixture read the whole trough-bottom band unreachable
        // at the shipped cell — tens of percent — so a 2 % bar separates the
        // two states by more than an order of magnitude while staying honest
        // about which term is bounded and which is merely covered.
        assert!(
            map.unreachable_pct() < 2.0,
            "cell {cell}: a trough the ball FITS must not read unreachable. \
             Got {:.2} % unreachable, {:.2} % unresolved, max gap {:.4} mm \
             against a floor of {:.4} mm (profile {:.4}, curvature p95 {:.4}). \
             The predicted discretisation over-read here is {predicted:.4} mm.",
            map.unreachable_pct(),
            map.unresolved_pct(),
            map.max_gap_mm,
            map.discretisation_floor_mm,
            map.profile_floor_mm,
            map.curvature_floor_p95_mm,
        );
    }

    // And the floor must SAY so: on this surface the curvature term is the
    // one that carries the grid's limit, and the plane-only term cannot see
    // it. This is the assertion that pins the F1 diagnosis rather than the
    // symptom.
    let coarse = reach_map_for_mesh(&mesh, &ball, 0.05, 0.645);
    assert!(
        coarse.curvature_floor_p95_mm > coarse.profile_floor_mm,
        "on a curved surface the curvature term must exceed the plane-only \
         profile term, or the published floor is still the old plane number: \
         curvature p95 {:.5} mm against profile {:.5} mm",
        coarse.curvature_floor_p95_mm,
        coarse.profile_floor_mm
    );
    assert!(
        coarse.curvature_floor_p95_mm > 0.01,
        "the curvature term on a rho {rho} trough at a 0.645 mm cell should be \
         of order cell^2/8 = {:.4} mm; got {:.5} mm",
        0.645 * 0.645 / 8.0,
        coarse.curvature_floor_p95_mm
    );
}

/// A trough the same ball does NOT fit still reports its analytic bridge gap.
///
/// The other half of the fix's contract. An abstention that swallowed a real
/// miss would be worse than the over-reading it replaced, so this fixture
/// sits beside the one above with the SAME tool and the same cells, differing
/// only in whether the tool fits.
#[test]
fn a_trough_the_ball_does_not_fit_still_reports_its_bridge_gap() {
    let (rho, a) = (1.5, 1.2);
    let ball = BallEndmill::new(4.0, 25.0);
    let radius = ball.cusp_radius_mm();
    let expected = trough_bridge_gap_mm(radius, rho, a).expect("R2.0 must bridge a rho 1.5 trough");
    // `tip_z` over this trough is the ball riding one rim or the other, so it
    // KINKS at the centre. The one-sided slope there is `a / sqrt(R^2 - a^2)`,
    // and the worst bilinear over-read across a kink straddling a cell is
    // `s * cell / 2` — linear in the cell, not quadratic, which is why the
    // curvature floor does not bound it. See `curvature_floor_plane`.
    let kink_slope = a / (radius * radius - a * a).sqrt();
    let mesh = trough(rho, a, 6.0, 0.1);
    eprintln!(
        "F1 concave trough rho {rho} (R{radius:.1} ball does NOT fit), analytic \
         bridge gap {expected:.4} mm, tip_z kink slope {kink_slope:.3}:"
    );
    let mut exact_somewhere = false;
    for cell in [0.2, 0.3, 0.4, 0.5, 0.6, 0.645] {
        let map = reach_map_for_mesh(&mesh, &ball, 0.05, cell);
        row(&format!("cell {cell}"), &map);
        let excess = map.max_gap_mm - expected;
        eprintln!(
            "      cell {cell}: excess {excess:+.4} mm \u{2014} floor {:.4}, \
             kink envelope {:.4}",
            map.discretisation_floor_mm,
            kink_slope * cell / 2.0,
        );
        exact_somewhere |= excess.abs() < 0.01;

        // 1. The real miss SURVIVES the abstention, at every cell. This is
        //    the half of the fix that matters: an instrument that stopped
        //    reporting a 0.2 mm miss would be worse than the over-reading it
        //    replaced.
        assert!(
            map.unreachable_pct() > 0.5,
            "cell {cell}: a {expected:.3} mm miss is four times the bar and must \
             survive the abstention; got {:.3} % unreachable, {:.3} % unresolved, \
             floor {:.4} mm",
            map.unreachable_pct(),
            map.unresolved_pct(),
            map.discretisation_floor_mm
        );

        // 2. The map never UNDER-states this miss. A drop-cutter minimum over
        //    a subset can only come out high, so an under-statement would be a
        //    sign error, not a resolution limit.
        assert!(
            excess > -0.01,
            "cell {cell}: the map under-stated a real miss by {:.4} mm, which no \
             sampling error can produce; got {:.4} against {expected:.4}",
            -excess,
            map.max_gap_mm
        );

        // 3. The over-statement stays inside the KINK envelope. Asserted
        //    against `s * cell / 2` and not against a fixed tolerance,
        //    because the excess is not a fixed tolerance: measured 0.0000 at
        //    cells 0.3, 0.4 and 0.6 and 0.1471 at 0.5, non-monotone, which is
        //    grid PHASE against a feature four cells wide. A fixed bar would
        //    be a bar on the phase.
        assert!(
            excess <= kink_slope * cell / 2.0 + 1e-9,
            "cell {cell}: the over-statement {excess:.4} mm broke the kink \
             envelope {:.4} mm. That envelope is the analytic worst case for a \
             slope discontinuity in tip_z, so a break here is a NEW mechanism, \
             not the known one.",
            kink_slope * cell / 2.0
        );
    }
    // 4. And the closed form is reachable: at least one cell reproduces it.
    assert!(
        exact_somewhere,
        "no cell reproduced the analytic bridge gap {expected:.4} mm to 0.01 mm, \
         so the fixture is not measuring the closed form at all"
    );
}

// ── (c) the two instruments answer different questions ─────────────────

/// The reach map and the tier map disagree BY CONSTRUCTION, and the V-groove
/// says why in closed form.
///
/// The tier map's residual is `drop_z(tool_k) − drop_z(finest)`. Where NO
/// tool in the ladder follows the surface, both drops are held up by the same
/// feature and the residual is small — so a tier map reports little to hand
/// down while a reach map reports a large miss. Both are right; they answer
/// different questions.
///
/// At this V's apex, in closed form for the ladder R2.0 → R0.5:
///
/// * reach gap for R2.0 against the true mesh: `2·(csc 45° − 1)` = 0.828 mm
/// * reach gap for R0.5 against the true mesh: `0.5·(csc 45° − 1)` = 0.207 mm
/// * the tier residual between them: 0.828 − 0.207 = **0.621 mm**, a quarter
///   less than the reach map's own figure — and the fine tool cannot close
///   the remaining 0.207 mm either.
///
/// The same arithmetic explains the wanaka threefold split quantitatively.
/// The independent instrument measured that terrain at 50.5 % unreachable by
/// cell for R2.0 and **13.5 % for R0.5**: most of what R2.0 misses there is
/// sub-millimetre facet roughness that R0.5 misses too, so it never appears
/// in a tool-versus-tool residual. `preview_tier_map`'s ~24 % is not a
/// smaller estimate of the reach map's 55 %; it is the answer to "how much
/// would a finer tool improve".
#[test]
fn the_tier_map_and_the_reach_map_answer_different_questions() {
    use rs_cam_core::maps::tier_map::{
        ResidualTreatment, TierLadder, TierMapParams, compute_tier_map,
    };

    let half_angle = 45.0_f64;
    let mesh = v_trench(10.0, half_angle, 0.1);
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = BallEndmill::new(4.0, 25.0);
    let fine = BallEndmill::new(1.0, 25.0);

    let coarse_gap = apex_gap_mm(coarse.cusp_radius_mm(), half_angle);
    let fine_gap = apex_gap_mm(fine.cusp_radius_mm(), half_angle);
    let tier_residual = coarse_gap - fine_gap;

    let coarse_map = reach_map_for_mesh(&mesh, &coarse, 0.05, 0.4);
    let fine_map = reach_map_for_mesh(&mesh, &fine, 0.05, 0.4);
    eprintln!(
        "F1 (c) 90 deg V-groove: reach gap R{:.1} = {coarse_gap:.4} mm, \
         R{:.2} = {fine_gap:.4} mm, tier residual between them = \
         {tier_residual:.4} mm",
        coarse.cusp_radius_mm(),
        fine.cusp_radius_mm()
    );
    row("reach, R2.0", &coarse_map);
    row("reach, R0.5", &fine_map);

    let tools: Vec<&dyn MillingCutter> = vec![&coarse, &fine];
    let ladder = TierLadder::new(&tools).expect("a coarse-to-fine pair is a valid ladder");
    let never_cancel = || false;
    let tiers = compute_tier_map(
        &mesh,
        &index,
        &ladder,
        &TierMapParams {
            cell_mm: 0.4,
            tolerance_mm: 0.05,
            margin_mm: 0.5,
            treatment: ResidualTreatment::SlopeCompensated,
        },
        &never_cancel,
    )
    .expect("the tier walk must finish on a V-groove");
    eprintln!(
        "    tier areas: coarse {:.1} mm2, fine {:.1} mm2",
        tiers.tier_area_mm2(0),
        tiers.tier_area_mm2(1)
    );

    // The claim: the FINE tool's own reach map still reports a miss at the
    // apex. So the territory the tier map hands down is territory the fine
    // tool cannot fully fix, and a tier residual is not an answer to "can
    // this tool form the surface".
    assert!(
        (fine_map.max_gap_mm - fine_gap).abs() < 0.05,
        "the fine tool's own apex gap must read {fine_gap:.4} mm; got {:.4} mm",
        fine_map.max_gap_mm
    );
    assert!(
        fine_map.unreachable_pct() > 0.0,
        "the fine tool must ALSO report the apex unreachable, or the point of \
         this test does not hold"
    );
    assert!(
        tier_residual < coarse_gap,
        "a tool-versus-tool residual is strictly smaller than the coarse \
         tool's residual against the true mesh: {tier_residual:.4} against \
         {coarse_gap:.4}"
    );
    assert!(
        tiers.tier_area_mm2(1) > 0.0,
        "the ladder must hand the apex band down, or the comparison is vacuous"
    );
}

// ── the wanaka instrument ───────────────────────────────────────────────

/// **Instrument, not a gate.** The operator's own case, end to end, so the
/// F1/F2 numbers in this file's header can be reproduced rather than trusted.
///
/// Run it explicitly:
///
/// ```text
/// cargo test -p rs_cam_core --test reach_map_residual_p5_1 \
///     -- --ignored --nocapture the_wanaka_terrain
/// ```
///
/// It reads the terrain STL the shipped project points at
/// (`planning/deep_doc_modulation_2026-09-08/Q2_r20_s15.toml`) and skips with
/// a printed line when that path is absent, because the mesh lives outside
/// the repo. Nothing is asserted about the percentage: the point is the
/// TABLE — the same tool, the same grid, three bars, with the floor and the
/// unresolved share beside each — against the independent measurement in
/// this file's header (55.0 % at 0.05 mm, 39.5 % at 0.146, 25.2 % at 0.30).
#[test]
#[ignore = "instrument: needs the wanaka terrain STL, which lives outside the repo"]
fn the_wanaka_terrain_reach_table() {
    use rs_cam_core::tool::TaperedBallEndmill;

    let path = std::path::Path::new("/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl");
    if !path.exists() {
        eprintln!("SKIP: {} is not on this machine", path.display());
        return;
    }
    let mesh = match TriangleMesh::from_stl(path) {
        Ok(mesh) => mesh,
        Err(e) => {
            eprintln!("SKIP: {} could not be read — {e}", path.display());
            return;
        }
    };
    // The shipped Q2 pass: Ø4 tapered ball, 3° half-angle, Ø6 shank.
    let taper = TaperedBallEndmill::new(4.0, 3.0, 6.0, 25.0);
    let cell = rs_cam_core::maps::reach_map::ReachMapParams::for_cutter(&taper, 0.05).cell_mm;
    eprintln!(
        "wanaka terrain: {} triangles, R{:.2} tip, envelope {:.2}, \
         cell rule gives {cell:.3} mm before the cell budget",
        mesh.faces.len(),
        taper.cusp_radius_mm(),
        taper.envelope_radius_mm(),
    );
    // ON THE MAP'S OWN BASE — 3D triangle area over the rim-eroded
    // population. The whole-board planar-ish figures (55.0 / 39.5 / 25.2)
    // that this line used to print are a DIFFERENT question and reading them
    // beside the rows below is what produced a phantom 12-point offset
    // (P5.2).
    eprintln!(
        "  independent ground truth, 0.15 mm lattice, 3D-area weighted over \
         the rim-eroded mask: 58.6 % at 0.050, 42.1 % at 0.146, 26.8 % at 0.300"
    );
    eprintln!(
        "  (whole-board planar base, NOT comparable with the rows below: \
         50.5 / 35.2 / 21.8; the Ø4 taper's cone adds +0.01 pp over a pure \
         R2.0 ball, so the profile is not the difference)"
    );
    // TWO cells, and the reason is that the fix moved both dials. 0.645 mm is
    // the grid the reported 68.2 % sat on, so it is the only cell whose rows
    // are comparable with the 68.2 / 63.7 / 55.0 column above. `cell` is what
    // the new rule picks for this tool and board. Reading one table as the
    // "after" of the other would credit the verdict change with a resolution
    // change, or the reverse.
    for (label, at) in [("old grid", 0.645), ("cell rule", cell)] {
        eprintln!("  --- {label}: cell {at:.3} mm ---");
        for tol in [0.05, 0.146, 0.3] {
            let map = reach_map_for_mesh(&mesh, &taper, tol, at);
            row(&format!("{label} tol {tol}"), &map);
            // The operator-facing sentence itself, verbatim, so the surfaces
            // can be read rather than reconstructed from the fields.
            eprintln!("      grid_note: {}", map.grid_note());
        }
    }
}
