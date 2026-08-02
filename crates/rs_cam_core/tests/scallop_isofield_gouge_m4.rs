//! **M4 phase C — the iso-field's localised gouge, pinned then fixed.**
//!
//! `CHECKPOINT_C_EVIDENCE.md` §3.8 flagged the adoption-blocking defect: on
//! the grooved block the iso-field ring source drove the deepest single point
//! to **−1115 µm (A8) / −995 µm (A9)** against the shipped cascade's
//! −108.6 µm, while gouge *area* went the other way (2.4–5.0 mm² vs 12.3).
//! Localised, not systemic — which is a mechanism, not a tuning problem.
//!
//! This file finds the mechanism and then gates it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout, clippy::print_stderr)]

mod common;

use common::scallop_oracle::{EnvelopeOracle, OracleGrid, OracleParams, StampKernel, render_field};
use common::{meshes, tools};
use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::finish_setup::FinishResolutionPolicy;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::scallop::{
    CurvaturePolicy, RingSource, ScallopDirection, ScallopParams, ScallopRingBudget,
    ScallopStepoverPolicy, StepoverGeometry, scallop_toolpath_research,
};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const TOLERANCE_MM: f64 = 0.10;
const DIAL_MM: f64 = 0.020;
const ORACLE_CELL_MM: f64 = 0.020;
const STAMP_CAP_TIP_MULTIPLE: f64 = 2.0;

fn params() -> ScallopParams {
    ScallopParams {
        scallop_height: DIAL_MM,
        tolerance: TOLERANCE_MM,
        direction: ScallopDirection::OutsideIn,
        continuous: false,
        slope_from: 0.0,
        slope_to: 90.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        stock_to_leave: 0.0,
    }
}

fn a9() -> ScallopStepoverPolicy {
    ScallopStepoverPolicy {
        ring_source: RingSource::IsoField,
        geometry: StepoverGeometry::CosineSlope,
        curvature: CurvaturePolicy::ToolLimited,
        ..ScallopStepoverPolicy::SHIPPED
    }
}

fn a8() -> ScallopStepoverPolicy {
    ScallopStepoverPolicy {
        ring_source: RingSource::IsoField,
        ..ScallopStepoverPolicy::SHIPPED
    }
}

struct Fx {
    name: &'static str,
    mesh: TriangleMesh,
    index: SpatialIndex,
}

fn fx(name: &'static str, mesh: TriangleMesh) -> Fx {
    let index = SpatialIndex::build(&mesh, 2.0);
    Fx { name, mesh, index }
}

fn grooved() -> Fx {
    fx("grooved block", meshes::grooved_block(6.0, 60.0, 3.0))
}

fn run(fixture: &Fx, policy: ScallopStepoverPolicy) -> Toolpath {
    let tool = tools::wanaka_taper();
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);
    let cancel = || false;
    let (tp, ..) = scallop_toolpath_research(
        &fixture.mesh,
        &fixture.index,
        &tool,
        &params(),
        None,
        None,
        resolution,
        ScallopRingBudget::FlatGroundStepover,
        policy,
        &cancel,
    )
    .expect("never-cancel");
    tp
}

/// Deepest gouge (µm, negative) the analytic envelope oracle sees.
fn deepest_gouge_um(fixture: &Fx, tp: &Toolpath) -> f64 {
    let tool = tools::wanaka_taper();
    let grid = OracleGrid::for_mesh(&fixture.mesh, &tool, ORACLE_CELL_MM);
    let truth = EnvelopeOracle::true_surface_from_mesh(grid, &fixture.mesh, &fixture.index);
    let kernel = StampKernel::new(
        &tool,
        ORACLE_CELL_MM,
        Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
    );
    let oracle = EnvelopeOracle::score(grid, truth, tp, &kernel, ORACLE_CELL_MM);
    oracle.report(OracleParams::new(DIAL_MM)).deepest_gouge_um
}

// ---------------------------------------------------------------------------
// Diagnosis
// ---------------------------------------------------------------------------

/// Separate the two ways a path can end up below the surface:
///
/// * **vertex dive** — an emitted point's own Z is below the drop-cutter
///   contact at its own XY. Ring lift is an exact query, so this can only come
///   from the `min_z` fallback for an uncovered point leaking into a fed chord.
/// * **chord sag** — both endpoints are exact but the straight line between
///   them passes under the surface. This is what chord refinement exists to
///   bound, and it is bounded by `chord_tolerance` only where refinement
///   actually runs.
#[test]
#[ignore = "M4 phase C diagnosis — prints the gouge anatomy"]
fn where_does_the_isofield_gouge_come_from() {
    let tool = tools::wanaka_taper();
    for (label, policy) in [
        ("A0 shipped", ScallopStepoverPolicy::SHIPPED),
        ("A8 iso-field", a8()),
        ("A9 iso-field+cosθ", a9()),
    ] {
        let f = grooved();
        let tp = run(&f, policy);
        let mut worst_vertex = 0.0_f64;
        let mut worst_vertex_at = None;
        let mut worst_sag = 0.0_f64;
        let mut worst_sag_at = None;
        let mut longest = 0.0_f64;
        let mut longest_at = None;
        let mut seg_count = 0usize;

        let mut prev: Option<P3> = None;
        for mv in &tp.moves {
            let cutting = !matches!(mv.move_type, MoveType::Rapid);
            let b = mv.target;
            if cutting {
                // vertex dive at b
                let cl = point_drop_cutter(b.x, b.y, &f.mesh, &f.index, &tool);
                if cl.z.is_finite() {
                    let dive = cl.z - b.z;
                    if dive > worst_vertex {
                        worst_vertex = dive;
                        worst_vertex_at = Some(b);
                    }
                }
                if let Some(a) = prev {
                    seg_count += 1;
                    let len = (b.x - a.x).hypot(b.y - a.y);
                    if len > longest {
                        longest = len;
                        longest_at = Some((a, b));
                    }
                    // chord sag: densely probe the chord
                    let n = ((len / 0.01).ceil() as usize).clamp(1, 4000);
                    for i in 1..n {
                        let t = i as f64 / n as f64;
                        let x = a.x + (b.x - a.x) * t;
                        let y = a.y + (b.y - a.y) * t;
                        let cz = a.z + (b.z - a.z) * t;
                        let cl = point_drop_cutter(x, y, &f.mesh, &f.index, &tool);
                        if !cl.z.is_finite() {
                            continue;
                        }
                        let sag = cl.z - cz;
                        if sag > worst_sag {
                            worst_sag = sag;
                            worst_sag_at = Some((a, b, t, cl.z, cz));
                        }
                    }
                }
            }
            prev = Some(b);
        }

        println!("\n== {label} ==");
        println!("  cutting segments      : {seg_count}");
        println!(
            "  worst VERTEX dive     : {:.1} µm at {:?}",
            worst_vertex * 1000.0,
            worst_vertex_at.map(|p| (
                (p.x * 1000.0).round() / 1000.0,
                (p.y * 1000.0).round() / 1000.0,
                (p.z * 1000.0).round() / 1000.0
            ))
        );
        println!("  worst CHORD sag       : {:.1} µm", worst_sag * 1000.0);
        if let Some((a, b, t, sz, cz)) = worst_sag_at {
            println!(
                "     chord ({:.3},{:.3},{:.3}) -> ({:.3},{:.3},{:.3}) len {:.4} mm, t={t:.3}, surface {sz:.4} vs chord {cz:.4}",
                a.x,
                a.y,
                a.z,
                b.x,
                b.y,
                b.z,
                (b.x - a.x).hypot(b.y - a.y)
            );
        }
        println!("  longest cutting chord : {:.4} mm", longest);
        if let Some((a, b)) = longest_at {
            println!(
                "     ({:.3},{:.3},{:.3}) -> ({:.3},{:.3},{:.3})",
                a.x, a.y, a.z, b.x, b.y, b.z
            );
        }
        println!(
            "  oracle deepest gouge  : {:.1} µm",
            deepest_gouge_um(&f, &tp)
        );
    }
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

/// **The red-first pin.** Every chord scallop feeds must track the surface
/// within the op's chord tolerance — from *either* ring source.
///
/// Before the M4 phase C fix this failed at 426–778 µm against a 100 µm
/// tolerance on the iso-field, and at ~98 µm (under the bar by luck, with the
/// true maximum unprobed) on the shipped cascade. Three independent causes,
/// all of which had to go:
///
/// 1. `refine_chord` declined to probe any chord shorter than `probe_step`,
///    which is every chord refinement itself produces by splitting;
/// 2. the iso-field's level sets were interpolated across the region edge
///    from grid nodes, landing rings off the part where drop-cutter answers
///    with a rim contact ~1 mm low;
/// 3. the tolerance check was sampled at the generation grid's pitch, so on a
///    0.75 mm cell a 0.7 mm chord got one probe, at its midpoint.
///
/// The slack below is the stamp/probe aliasing band, not licence: it is the
/// gap between "no probe found more than tolerance" and "no point on the
/// chord is more than tolerance under", which a finite probe set cannot
/// close.
#[test]
fn chord_refinement_bounds_every_chord_from_either_ring_source() {
    let tool = tools::wanaka_taper();
    const SLACK: f64 = 1.35;

    for (label, policy) in [
        ("shipped cascade", ScallopStepoverPolicy::SHIPPED),
        ("iso-field", a9()),
    ] {
        for f in &[
            grooved(),
            fx(
                "narrow ridge",
                meshes::height_field(8.0, 0.25, |x, _| 4.0 * (1.0 - x.abs()).max(0.0)),
            ),
            fx("mixed-slope ribbon", ribbon()),
        ] {
            let tp = run(f, policy);
            let mut worst = 0.0_f64;
            let mut worst_at = None;
            let mut prev: Option<P3> = None;
            for mv in &tp.moves {
                let b = mv.target;
                if !matches!(mv.move_type, MoveType::Rapid)
                    && let Some(a) = prev
                {
                    let len = (b.x - a.x).hypot(b.y - a.y);
                    let n = ((len / 0.02).ceil() as usize).clamp(1, 500);
                    for i in 1..n {
                        let t = i as f64 / n as f64;
                        let x = a.x + (b.x - a.x) * t;
                        let y = a.y + (b.y - a.y) * t;
                        let cl = point_drop_cutter(x, y, &f.mesh, &f.index, &tool);
                        if !cl.z.is_finite() {
                            continue;
                        }
                        // PERPENDICULAR distance from the surface point to the
                        // chord, not the vertical drop between them. The chord
                        // tolerance is a distance; on a 76° flank the vertical
                        // difference is 4.1x the real deviation, so a vertical
                        // measure condemns a chord that tracks the wall perfectly
                        // — and, on the ridge fixture, reports a 174 µm
                        // "violation" of a 100 µm tolerance that is really 42 µm.
                        let sag = point_to_segment_distance(P3::new(x, y, cl.z), a, b);
                        if sag > worst {
                            worst = sag;
                            worst_at = Some((a, b, t));
                        }
                    }
                }
                prev = Some(b);
            }
            assert!(
                worst <= TOLERANCE_MM * SLACK,
                "{label} on {}: a fed chord passes {:.1} µm under the surface, \
             against a {:.0} µm chord tolerance (slack {SLACK}x). Worst chord \
             {:?}. Chord refinement is not bounding what it emits — see this \
             file's header for the three mechanisms that produced exactly this.",
                f.name,
                worst * 1000.0,
                TOLERANCE_MM * 1000.0,
                worst_at.map(|(a, b, t)| (
                    (a.x, a.y, a.z),
                    (b.x, b.y, b.z),
                    (t * 1000.0).round() / 1000.0
                )),
            );
        }
    }
}

/// The iso-field must not emit the sub-10 µm segment population
/// `CHECKPOINT_C_EVIDENCE.md` §3.8 flagged (0.0–0.5% of segments, worst case
/// 32 on the grooved block), now that it inherits the ring decimation the
/// cascade always had and refinement is floored at
/// `CHORD_REFINE_MIN_SPLIT_MM`.
#[test]
fn neither_ring_source_emits_sub_ten_micron_segments() {
    for (label, policy) in [
        ("shipped cascade", ScallopStepoverPolicy::SHIPPED),
        ("iso-field", a9()),
    ] {
        let f = grooved();
        let tp = run(&f, policy);
        let mut short = 0usize;
        let mut total = 0usize;
        let mut shortest = f64::INFINITY;
        let mut prev: Option<P3> = None;
        for mv in &tp.moves {
            if !matches!(mv.move_type, MoveType::Rapid)
                && let Some(a) = prev
            {
                let d = ((mv.target.x - a.x).powi(2)
                    + (mv.target.y - a.y).powi(2)
                    + (mv.target.z - a.z).powi(2))
                .sqrt();
                if d > 1e-12 {
                    total += 1;
                    shortest = shortest.min(d);
                    if d < 0.010 {
                        short += 1;
                    }
                }
            }
            prev = Some(mv.target);
        }
        assert_eq!(
            short, 0,
            "{label}: {short} of {total} emitted segments are under 10 µm \
             (shortest {:.4} mm). Every junction costs the controller a \
             decel/accel pair regardless of how short the move is.",
            shortest
        );
    }
}

/// Render the residual field for the three arms so the gouge can be LOOKED at
/// before anything is claimed about it (the v3 rule).
#[test]
#[ignore = "M4 phase C diagnosis — writes residual maps"]
fn render_the_gouge() {
    let tool = tools::wanaka_taper();
    for (tag, policy) in [
        ("A0", ScallopStepoverPolicy::SHIPPED),
        ("A8", a8()),
        ("A9", a9()),
    ] {
        let f = grooved();
        let tp = run(&f, policy);
        let grid = OracleGrid::for_mesh(&f.mesh, &tool, ORACLE_CELL_MM);
        let truth = EnvelopeOracle::true_surface_from_mesh(grid, &f.mesh, &f.index);
        let kernel = StampKernel::new(
            &tool,
            ORACLE_CELL_MM,
            Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
        );
        let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, ORACLE_CELL_MM);
        let resid = oracle.residual_map(0.0);
        render_field(&grid, &resid, 100.0, &format!("m4c_grooved_{tag}"));
        // A second render saturating at 1 mm so the deep gouge is legible
        // rather than a saturated blue blob.
        render_field(&grid, &resid, 1000.0, &format!("m4c_grooved_{tag}_1mm"));
        println!("{}: {tag} rendered", f.name);
    }
}

/// The gouge gate across the whole M4 fixture matrix, not just the fixture
/// that flagged it: deepest single point, gouge area, and the segment-length
/// tail, shipped cascade vs iso-field.
#[test]
#[ignore = "M4 phase C evidence — the gouge matrix across all five fixtures"]
fn gouge_matrix_across_the_fixture_set() {
    let tool = tools::wanaka_taper();
    let fixtures: Vec<Fx> = vec![
        fx("flat ground", meshes::height_field(8.0, 0.25, |_, _| 0.0)),
        grooved(),
        fx(
            "narrow ridge",
            meshes::height_field(8.0, 0.25, |x, _| 4.0 * (1.0 - x.abs()).max(0.0)),
        ),
        fx("mixed-slope ribbon", ribbon()),
        fx(
            "dome",
            meshes::height_field(8.0, 0.25, |x, y| {
                (144.0 - (x * x + y * y)).max(0.0).sqrt() - 12.0
            }),
        ),
    ];
    println!(
        "\n| fixture | arm | deepest gouge µm (normal) | deepest gouge µm (vertical) | gouge mm² | cusp ×dial | standing mm² | untouched mm² | min seg mm | segs <10µm |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for f in &fixtures {
        for (label, policy) in [
            ("A0 shipped", ScallopStepoverPolicy::SHIPPED),
            ("A9 iso-field", a9()),
        ] {
            let tp = run(f, policy);
            let grid = OracleGrid::for_mesh(&f.mesh, &tool, ORACLE_CELL_MM);
            let truth = EnvelopeOracle::true_surface_from_mesh(grid, &f.mesh, &f.index);
            let kernel = StampKernel::new(
                &tool,
                ORACLE_CELL_MM,
                Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
            );
            let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, ORACLE_CELL_MM);
            let r = oracle.report(OracleParams::new(DIAL_MM));
            let (short, total, shortest) = segment_stats(&tp);
            println!(
                "| {} | {} | {:.1} | {:.1} | {:.3} | {:.2}× | {:.2} | {:.2} | {:.4} | {} ({:.2}%) |",
                f.name,
                label,
                r.deepest_gouge_normal_um,
                r.deepest_gouge_um,
                r.gouge_mm2,
                r.cusp_ratio_normal(),
                r.standing_mm2,
                r.untouched_mm2,
                shortest,
                short,
                if total > 0 {
                    short as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
            );
        }
    }
}

fn ribbon() -> TriangleMesh {
    meshes::height_field(8.0, 0.25, |x, _| {
        let a = x.abs();
        if a < 2.0 {
            a * 0.105
        } else if a < 4.0 {
            0.21 + (a - 2.0) * 1.732
        } else if a < 4.5 {
            3.674 + (a - 4.0) * 11.43
        } else {
            9.39
        }
    })
}

/// Shortest 3D distance from `p` to the segment `a`–`b`.
fn point_to_segment_distance(p: P3, a: P3, b: P3) -> f64 {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let len_sq = dx * dx + dy * dy + dz * dz;
    let t = if len_sq <= f64::EPSILON {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy + (p.z - a.z) * dz) / len_sq).clamp(0.0, 1.0)
    };
    ((p.x - (a.x + dx * t)).powi(2)
        + (p.y - (a.y + dy * t)).powi(2)
        + (p.z - (a.z + dz * t)).powi(2))
    .sqrt()
}

fn segment_stats(tp: &Toolpath) -> (usize, usize, f64) {
    let (mut short, mut total, mut shortest) = (0usize, 0usize, f64::INFINITY);
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        if !matches!(mv.move_type, MoveType::Rapid)
            && let Some(a) = prev
        {
            let d = ((mv.target.x - a.x).powi(2)
                + (mv.target.y - a.y).powi(2)
                + (mv.target.z - a.z).powi(2))
            .sqrt();
            if d > 1e-12 {
                total += 1;
                shortest = shortest.min(d);
                if d < 0.010 {
                    short += 1;
                }
            }
        }
        prev = Some(mv.target);
    }
    (short, total, shortest)
}
