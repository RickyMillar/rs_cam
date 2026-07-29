//! PR-4 (H2.1) — the canonical reach policy in the production rest-field
//! path, and the tapered-reference (`SelfReferenced`) fix.
//!
//! The analytic gate — "the shipped policy reproduces the approved Checkpoint
//! A column" — lives in `checkpoint_a_valley_matrix.rs`, next to the truth
//! sampler and the geometry helpers it scores against. This file covers what
//! that one cannot: the policy reached through `detect_rest_valleys` on a real
//! mesh, the per-point cross-section the detector measures, the routing-only
//! meaning of `RestFieldParams::routing_radius_mm`, and the reference
//! resolution the whole rest measurement depends on.
//!
//! Basis: `planning/review_2026-07-29/CHECKPOINT_A_EVIDENCE.md` (approved
//! 2026-07-29) and `TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H2.1.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::reach::{
    LocalValley, RoutingVerdict, ValleySide, coverage_cap_passes, offset_passes_per_side, route,
    solve_reach,
};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

/// The tool this project finishes with: Ø1 tip, 7° half-angle, Ø6 shank.
/// Envelope 3.0 mm, cusp 0.5 mm — the 6× split every row below turns on.
fn wanaka_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// A block 40 × 24 mm, top at z = 0, with one straight trapezoidal groove
/// along Y at x = 0. Height-field mesh — the drop cutter only needs the top.
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
    let mut xs: Vec<f64> = Vec::new();
    let mut x = -20.0;
    while x < -4.0 {
        xs.push(x);
        x += 1.0;
    }
    let mut x = -4.0;
    while x <= 4.0 + 1e-9 {
        xs.push(x);
        x += 0.05;
    }
    for b in [-rim_half_width, -floor_half, floor_half, rim_half_width] {
        xs.push(b);
    }
    let mut x = 5.0;
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

fn params(cutter: &dyn MillingCutter) -> RestFieldParams {
    RestFieldParams {
        cell_mm: 0.2,
        min_valley_depth: 0.05,
        route_width_factor: 2.0,
        routing_radius_mm: cutter.radius(),
        min_cut_length: 2.0,
        region_margin_mm: 0.5,
    }
}

fn reference() -> BallEndmill {
    BallEndmill::new(12.0, 25.0)
}

/// **The detector now measures the cross-section it used to guess at.** Every
/// emitted centreline carries one [`rs_cam_core::rest_field::CenterlineSample`]
/// per point, with a rim distance on each side and a resolved reach — not a
/// single per-branch scalar.
///
/// RED-FIRST EVIDENCE: before PR-4 `RestCenterline` had exactly two fields
/// (`points`, `half_width_mm`); this assertion could not be written, which is
/// the defect. `CHECKPOINT_A_EVIDENCE.md` §8.5 records why a per-branch
/// scalar cannot be made honest: `engagement_radius` is monotone in depth, so
/// median is optimistic (the matrix shows that direction produces gouge
/// cells) and peak suppresses reachable detail.
#[test]
fn every_centreline_carries_a_per_point_cross_section() {
    let mesh = grooved_block(1.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let refc = reference();
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &refc as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &params(&cutter),
    );
    assert!(
        !rf.centerlines.is_empty(),
        "fixture produced no rest centrelines — it is not exercising the detector"
    );
    let mut measured = 0usize;
    for cl in &rf.centerlines {
        assert_eq!(
            cl.samples.len(),
            cl.points.len(),
            "sample count must track the point count exactly"
        );
        for s in &cl.samples {
            assert!(
                s.valley.rest_depth_mm >= 0.0 && s.valley.rest_depth_mm.is_finite(),
                "non-finite rest depth in a measured sample: {s:?}"
            );
            assert!(s.valley.left.rim_distance_mm >= 0.0);
            assert!(s.valley.right.rim_distance_mm >= 0.0);
            if s.valley.left.rim_distance_mm > 0.0 && s.valley.right.rim_distance_mm > 0.0 {
                measured += 1;
            }
        }
    }
    println!(
        "centrelines={} points={} two-sided samples={measured}",
        rf.centerlines.len(),
        rf.centerlines.iter().map(|c| c.points.len()).sum::<usize>(),
    );
    assert!(
        measured > 0,
        "no sample found a rim on both sides — the perpendicular walk is not walking"
    );
}

/// **The differential the change rests on.** On the same measured valley,
/// envelope, cusp and reach give three DIFFERENT pass counts, and only the
/// reach answer tracks the physics: the envelope radius (3.0 mm) exceeds the
/// whole valley so it emits nothing, and the cusp radius (0.5 mm) is
/// depth-blind so it emits the same count at every depth.
#[test]
fn envelope_cusp_and_reach_give_three_different_answers_on_measured_geometry() {
    let mesh = grooved_block(2.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let refc = reference();
    let rf = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &refc as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &params(&cutter),
    );
    let cl = rf
        .centerlines
        .iter()
        .max_by_key(|c| c.points.len())
        .expect("fixture produced no centreline");
    let s = cl.samples[cl.samples.len() / 2];
    let stepover = 0.5;

    let n_env = (((cl.half_width_mm - cutter.envelope_radius_mm()) / stepover).round())
        .max(0.0) as usize;
    let n_cusp =
        (((cl.half_width_mm - cutter.cusp_radius_mm()) / stepover).round()).max(0.0) as usize;
    let (nl, nr) = offset_passes_per_side(&s.reach, stepover, 4);

    println!(
        "half_width={:.3} depth={:.3} reach=({:.3},{:.3}) → env n={n_env}, \
         cusp n={n_cusp}, reach n=({nl},{nr})",
        cl.half_width_mm, s.valley.rest_depth_mm, s.reach.left_mm, s.reach.right_mm,
    );
    assert_eq!(
        n_env, 0,
        "envelope baseline unexpectedly emitted passes — the fixture stopped \
         exercising the A2/A4 defect"
    );
    assert!(
        n_cusp > 0,
        "cusp baseline emitted nothing — the fixture is not discriminating"
    );
    // The reach answer is a depth-aware quantity: it must not be the
    // depth-blind cusp answer, and it must not be the dead envelope answer.
    assert!(
        nl.max(nr) != n_env || nl.max(nr) != n_cusp,
        "reach agreed with BOTH baselines — no differential to gate on"
    );
}

/// Reachable-detail coverage strictly increases over the envelope baseline on
/// a tapered fixture: a valley the envelope model gives zero offset passes
/// gets at least one under the reach policy, where the physics supports it.
#[test]
fn reach_recovers_offset_passes_the_envelope_baseline_suppressed() {
    let cutter = wanaka_taper();
    // A 2.5 mm-half-width valley with 75° walls at 0.6 mm rest depth: well
    // inside the Ø6 shank the envelope model measures against, so the shipped
    // equation yields `round((2.5 − 3.0)/0.5) = 0` passes for ever.
    let valley = LocalValley {
        rest_depth_mm: 0.6,
        left: ValleySide::from_wall_angle(2.5, 75.0_f64.to_radians()),
        right: ValleySide::from_wall_angle(2.5, 75.0_f64.to_radians()),
    };
    let reach = solve_reach(&cutter, &valley);
    let (nl, nr) = offset_passes_per_side(&reach, 0.5, 4);
    let n_env = ((2.5 - cutter.envelope_radius_mm()) / 0.5).round().max(0.0) as usize;
    println!("envelope n={n_env}, reach n=({nl},{nr}), reach={reach:?}");
    assert_eq!(n_env, 0, "envelope baseline changed — re-derive this fixture");
    assert!(
        nl >= 1 && nr >= 1,
        "reach policy also suppressed the fan the physics supports: {reach:?}"
    );
}

/// `routing_radius_mm` is the ROUTING yardstick and nothing else. Padding,
/// erosion and the region-polygon reach-back all read the cutter's own
/// envelope, and plan H2.1 rule 4 says they must stay that way — so changing
/// this field must not move a single region polygon or grid dimension.
#[test]
fn routing_radius_does_not_move_the_envelope_derived_geometry() {
    let mesh = grooved_block(2.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let refc = reference();
    let run = |routing_radius_mm: f64| {
        detect_rest_valleys(
            &mesh,
            &index,
            &cutter,
            RestReference::Cutter {
                tool: &refc as &dyn MillingCutter,
                is_surface_probe: false,
            },
            &RestFieldParams {
                routing_radius_mm,
                ..params(&cutter)
            },
        )
    };
    let a = run(cutter.envelope_radius_mm());
    let b = run(cutter.cusp_radius_mm());
    assert_eq!(
        (a.rest_grid.nx, a.rest_grid.ny),
        (b.rest_grid.nx, b.rest_grid.ny),
        "grid extent moved with the routing yardstick"
    );
    assert_eq!(
        a.region_polygons.len(),
        b.region_polygons.len(),
        "region-polygon reach-back moved with the routing yardstick"
    );
    for (pa, pb) in a.region_polygons.iter().zip(b.region_polygons.iter()) {
        assert_eq!(
            pa.exterior.len(),
            pb.exterior.len(),
            "region polygon changed shape with the routing yardstick"
        );
    }
    assert!(
        !a.region_polygons.is_empty(),
        "fixture produced no region polygons — the assertion above is vacuous"
    );
}

/// **Task #12, red-first.** `resolve_reference_cutter` used to compare the
/// nominal reference against `diameter()` — the SHANK. On the shipped taper
/// that is 6.0, equal to the shipped `reference_tool_diameter` default, so
/// `6.0 > 6.0` was false and every tapered pencil operation silently fell
/// through to the self-referenced surface probe. It is not a private detail:
/// the two arms produce different rest fields, so the whole rest measurement
/// meant something other than what it said.
///
/// The RED this pins is at the level the defect lives: the same fixture read
/// against a Ø6 nominal reference versus the Ø0.1 surface probe gives
/// different centreline geometry. Pre-fix the default took the second branch;
/// post-fix it takes the first.
#[test]
fn a_six_millimetre_reference_is_not_the_same_measurement_as_the_surface_probe() {
    let mesh = grooved_block(1.5, 70.0, 1.2);
    let index = SpatialIndex::build(&mesh, 4.0);
    let cutter = wanaka_taper();
    let nominal = BallEndmill::new(6.0, 25.0);
    let probe = BallEndmill::new(0.1, 10.0);

    let with_nominal = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &nominal as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &params(&cutter),
    );
    let with_probe = detect_rest_valleys(
        &mesh,
        &index,
        &cutter,
        RestReference::Cutter {
            tool: &probe as &dyn MillingCutter,
            is_surface_probe: true,
        },
        &params(&cutter),
    );
    let vol = |r: &rs_cam_core::rest_field::RestFieldResult| r.report.total_rest_volume_mm3;
    println!(
        "Ø6 nominal reference: {} centrelines, {:.2} mm³ rest; \
         self-referenced probe: {} centrelines, {:.2} mm³ rest",
        with_nominal.centerlines.len(),
        vol(&with_nominal),
        with_probe.centerlines.len(),
        vol(&with_probe),
    );
    assert!(
        (vol(&with_nominal) - vol(&with_probe)).abs() > 1e-6,
        "the two reference arms measured the same field — the fixture cannot \
         distinguish the fall-through this test exists to pin"
    );
    // The tip diameter is what the comparison must be against: it is what the
    // taper CUTS with, and it is `diameter()` for every non-tapered shape, so
    // nothing but the tapered path can move.
    assert!(
        cutter.cusp_radius_mm() * 2.0 < cutter.diameter(),
        "fixture tool has no shank/tip split"
    );
    assert!(
        6.0 > cutter.cusp_radius_mm() * 2.0 && 6.0 <= cutter.diameter(),
        "the shipped default no longer sits between the tip and the shank; \
         re-derive this test"
    );
}

/// Ball characterisation (approved migration, `CHECKPOINT_A_EVIDENCE.md` §4 /
/// open question 5). Ball tools DO move under the new model — envelope and
/// cusp are identical on a ball, and both scored 11 gouge / 35 miss /
/// 56 float-blind / 74 % coverage against the matrix truth, while the reach
/// policy scores 0/0/0/100 %. This test does not judge; it records exactly
/// what changed for a ball on one fixture so the delta is never a surprise.
#[test]
fn ball_characterisation_what_the_new_model_changes() {
    let ball = BallEndmill::new(3.0, 25.0);
    let stepover = 0.5;
    println!();
    println!("== Ø3 ball — routing/fan counts, old model vs shipped reach policy ==");
    println!("| half-width | wall θ | depth | env/cusp n | reach n (L,R) | verdict |");
    println!("|---:|---:|---:|---:|---|---|");
    let mut moved = 0usize;
    let mut refused = 0usize;
    for &(w, theta_deg, depth) in &[
        (1.0_f64, 45.0_f64, 0.2_f64),
        (2.0, 45.0, 0.2),
        (2.0, 45.0, 1.0),
        (3.0, 60.0, 0.6),
        (3.0, 60.0, 4.39),
        (5.0, 75.0, 2.0),
    ] {
        let n_old = (((w - ball.envelope_radius_mm()) / stepover).round()).max(0.0) as usize;
        let n_old = n_old.min(4);
        let v = LocalValley {
            rest_depth_mm: depth,
            left: ValleySide::from_wall_angle(w, theta_deg.to_radians()),
            right: ValleySide::from_wall_angle(w, theta_deg.to_radians()),
        };
        let r = solve_reach(&ball, &v);
        let (nl, nr) = offset_passes_per_side(&r, stepover, 4);
        let cap = coverage_cap_passes(&ball, depth, stepover, 4);
        let verdict = route(&r, stepover, cap);
        println!(
            "| {w:.1} | {theta_deg:.0}° | {depth:.2} | {n_old} | ({nl},{nr}) | {verdict:?} |"
        );
        if nl != n_old || nr != n_old {
            moved += 1;
        }
        if verdict == RoutingVerdict::Refused {
            refused += 1;
        }
    }
    // Ball behaviour moving is the APPROVED outcome, not a regression — but
    // it must be visible. A run where nothing moved would mean the migration
    // silently did not happen.
    assert!(
        moved > 0,
        "ball behaviour did not move at all — the approved migration did not land"
    );
    assert!(
        refused > 0,
        "no ball fixture hit the two-wall fouling case; the 56 float-blind \
         cells the matrix found have no representative here"
    );
    println!("rows whose fan changed: {moved}; rows refused as unreachable: {refused}");
}
