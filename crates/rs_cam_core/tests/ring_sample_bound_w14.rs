//! **Wave 14 — what the ring SAMPLING bound buys, on the oracle.**
//!
//! Checkpoint D's arc-carrying cascade separated two things that used to
//! arrive fused. `polygon::FlattenPolicy` has a deviation budget, which puts
//! points where a ring CURVES; and it has a segment bound, which puts points
//! along a ring's STRAIGHT runs, where the deviation budget owes it none.
//! Scallop needs the second because it reads every ring vertex as a
//! drop-cutter sample, not as a corner of a shape.
//!
//! The implementation wave picked the flat-ground stepover for that bound and
//! it cost **+88% moves on the tight-tolerance PR-3 fixture**. That is the
//! whole question this file exists to answer, and it is not answerable by
//! fingerprint: more moves is not better or worse, it is *more*. The M4
//! envelope oracle scores the surface, so it can say whether the extra
//! samples land as achieved cusp or as waste.
//!
//! # The two dials, deliberately
//!
//! | fixture | dial (cusp) | tolerance | ratio |
//! |---|---|---|---|
//! | PR-3 ridge | 0.050 mm | **0.010 mm** | tol = dial/5 — TIGHT |
//! | M4 grooved block | 0.020 mm | **0.100 mm** | tol = 5×dial — LOOSE |
//!
//! A bound read off the cusp dial (`FlatGroundStepover`) is blind to that
//! column. A bound read off the chord tolerance (`ToleranceScaled`) is not.
//! Scoring both dials is what distinguishes "the tight fixture is owed the
//! density" from "the bound is simply over-dense everywhere".
//!
//! ```text
//! cargo test -p rs_cam_core --test ring_sample_bound_w14 \
//!     -- --ignored --nocapture --test-threads=1
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout, clippy::print_stderr)]

mod common;

use std::time::Instant;

use common::scallop_oracle::{EnvelopeOracle, OracleGrid, OracleParams, OracleReport, StampKernel};
use common::{meshes, tools};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::scallop::{
    RingSampleBound, ScallopDirection, ScallopParams, ScallopRingBudget, ScallopStepoverPolicy,
    scallop_generation_resolution, scallop_toolpath_research,
};
use rs_cam_core::tool::MillingCutter;

/// The stamp cap M4 uses, for the same reason: the wanaka taper's envelope is
/// its Ø6 shank and the flank cannot touch below 83° of slope.
const STAMP_CAP_TIP_MULTIPLE: f64 = 2.0;
const ORACLE_CELL_MM: f64 = 0.020;

/// PR-3's ridge, copied verbatim from `finish_resolution_policy_pr3.rs` — the
/// fixture whose fingerprint moved, so the +88% is measured on the surface the
/// move was counted on.
fn pr3_ridge() -> TriangleMesh {
    let n: usize = 21;
    let mut verts = Vec::with_capacity(n * n);
    for iy in 0..n {
        for ix in 0..n {
            let x = ix as f64;
            let y = iy as f64;
            let z = 3.0 * (1.0 - (x - 10.0).abs() / 10.0) + 0.5 * (y * 0.3).sin();
            verts.push(P3::new(x, y, z));
        }
    }
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for iy in 0..(n - 1) {
        for ix in 0..(n - 1) {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

struct Case {
    name: &'static str,
    mesh: TriangleMesh,
    dial_mm: f64,
    tolerance_mm: f64,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "PR-3 ridge (tol = dial/5)",
            mesh: pr3_ridge(),
            dial_mm: 0.050,
            tolerance_mm: 0.010,
        },
        Case {
            name: "grooved block (tol = 5x dial)",
            mesh: meshes::grooved_block(6.0, 60.0, 3.0),
            dial_mm: 0.020,
            tolerance_mm: 0.100,
        },
        // The same tight-tolerance question on a fixture with real slope
        // variety, so the answer is not a property of one ridge.
        Case {
            name: "grooved block, TIGHT (tol = dial/5)",
            mesh: meshes::grooved_block(6.0, 60.0, 3.0),
            dial_mm: 0.050,
            tolerance_mm: 0.010,
        },
    ]
}

struct Run {
    moves: usize,
    seconds: f64,
    rings: usize,
    report: OracleReport,
    sample_mm: Option<f64>,
}

fn run(case: &Case, bound: RingSampleBound) -> Run {
    let tool = tools::wanaka_taper();
    let index = SpatialIndex::build(&case.mesh, 2.0);
    let params = ScallopParams {
        scallop_height: case.dial_mm,
        tolerance: case.tolerance_mm,
        direction: ScallopDirection::OutsideIn,
        continuous: false,
        slope_from: 0.0,
        slope_to: 90.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 30.0,
        stock_to_leave: 0.0,
        intra_pass_hookup_mm: 0.0,
        link_kinematics: None,
    };
    let policy = ScallopStepoverPolicy {
        sample_bound: bound,
        ..ScallopStepoverPolicy::SHIPPED
    };
    let resolution = scallop_generation_resolution(&tool, case.tolerance_mm);
    let cancel = || false;

    let t0 = Instant::now();
    let (tp, _anns, report, _trace) = scallop_toolpath_research(
        &case.mesh,
        &index,
        &tool,
        &params,
        None,
        None,
        resolution,
        ScallopRingBudget::FlatGroundStepover,
        policy,
        &cancel,
    )
    .expect("never-cancel");
    let seconds = t0.elapsed().as_secs_f64();

    let grid = OracleGrid::for_mesh(&case.mesh, &tool, ORACLE_CELL_MM);
    let truth = EnvelopeOracle::true_surface_from_mesh(grid, &case.mesh, &index);
    let kernel = StampKernel::new(
        &tool,
        ORACLE_CELL_MM,
        Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
    );
    let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, ORACLE_CELL_MM);
    let mut op = OracleParams::new(case.dial_mm);
    op.stock_to_leave = params.stock_to_leave;

    Run {
        moves: tp.moves.len(),
        seconds,
        rings: report.ring_count,
        report: oracle.report(op),
        sample_mm: bound.max_segment_mm(tool.cusp_radius_mm(), case.dial_mm, case.tolerance_mm),
    }
}

/// The evidence run. Every column the decision rule needs, per fixture.
#[test]
#[ignore = "wave 14 evidence — the +88%-moves adjudication"]
fn sample_bound_matrix() {
    let tool = tools::wanaka_taper();
    println!(
        "\n# ring sampling bound — cusp radius {:.4} mm\n",
        tool.cusp_radius_mm()
    );
    for case in cases() {
        println!(
            "\n## {} — dial {:.3} mm, tolerance {:.3} mm",
            case.name, case.dial_mm, case.tolerance_mm
        );
        println!(
            "\n| bound | sample mm | moves | gen s | rings | cusp µm (normal) | ×dial | on dial | untouched mm² | standing mm² | gouge mm² | deepest µm |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
        let mut baseline: Option<(usize, f64)> = None;
        for bound in RingSampleBound::ALL {
            let r = run(&case, bound);
            let s = r
                .sample_mm
                .map_or_else(|| "none".to_owned(), |v| format!("{v:.4}"));
            println!(
                "| {} | {} | {} | {:.2} | {} | {:.1} | {:.2} | {:.1}% | {:.3} | {:.3} | {:.3} | {:.1} |",
                bound.label(),
                s,
                r.moves,
                r.seconds,
                r.rings,
                r.report.achieved_cusp_normal_um,
                r.report.cusp_ratio_normal(),
                r.report.on_dial_frac * 100.0,
                r.report.untouched_mm2,
                r.report.standing_mm2,
                r.report.gouge_mm2,
                r.report.deepest_gouge_normal_um,
            );
            if bound == RingSampleBound::FlatGroundStepover {
                baseline = Some((r.moves, r.report.achieved_cusp_normal_um));
            }
            if let (Some((bm, bc)), true) = (baseline, bound != RingSampleBound::FlatGroundStepover)
            {
                let dm = (r.moves as f64 - bm as f64) / bm as f64 * 100.0;
                let dc = r.report.achieved_cusp_normal_um - bc;
                println!(
                    "| ^ vs flat-ground | | {dm:+.1}% moves | | | {dc:+.1} µm cusp | | | | | | |"
                );
            }
        }
    }
}

/// Non-vacuity: the three bounds must actually produce different sampling on
/// the tight fixture, or the matrix above is comparing one thing to itself.
#[test]
fn the_bounds_are_actually_different() {
    let tool = tools::wanaka_taper();
    let cusp_r = tool.cusp_radius_mm();
    let (dial, tol) = (0.050, 0.010);
    let flat = RingSampleBound::FlatGroundStepover
        .max_segment_mm(cusp_r, dial, tol)
        .expect("bounded");
    let scaled = RingSampleBound::ToleranceScaled
        .max_segment_mm(cusp_r, dial, tol)
        .expect("bounded");
    assert!(
        RingSampleBound::ToleranceOnly
            .max_segment_mm(cusp_r, dial, tol)
            .is_none(),
        "the unbounded arm must be unbounded"
    );
    // tol < dial, and the law is monotone in its height argument, so the
    // tolerance-scaled bound must be the DENSER of the two here. If this ever
    // inverts, the two arms have swapped meaning and every reading is wrong.
    assert!(
        scaled < flat,
        "at tol {tol} < dial {dial} the tolerance-scaled sample ({scaled:.4}) \
         must be denser than the flat-ground one ({flat:.4})"
    );
}
