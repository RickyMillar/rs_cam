//! **M5 research, step 3+4: the candidates, and what they cost the surface.**
//!
//! `offset_growth_m5.rs` established the mechanism: `Polygon2::from_pline`
//! discards the bulge of every arc join cavalier emits and keeps its two
//! endpoints, so one reflex corner becomes two shallower reflex corners that
//! each arc-join on the next pass. Growth is a doubling, and the count of
//! added vertices equals the count of arc segments, 1:1.
//!
//! This file runs the plan's candidate list against that mechanism:
//!
//! | plan item | arm |
//! |---|---|
//! | (a) exact arc preservation | `C5` (and `C5-ctl`, the same cascade flattened per ring — the isolator) |
//! | (b) tolerance-bounded simplification | `C4` (`polygon::simplify_bounded`, RDP + self-intersection guard) |
//! | (c) collinear / near-duplicate cleanup | `C2` (cavalier's own `remove_redundant`), `C3` (`polygon::cleanup_collinear`) |
//! | (d) alternate backend | `C7` (`geo::Buffer` — i_overlay, already in the dependency graph, so it is MEASURED rather than estimated) |
//! | today | `C0` raw, `C1` scallop's drop-only decimation |
//!
//! Plus `C6`, which is not on the plan's list and should be: flatten every
//! ring, but with `arcs_to_approx_lines(tol)` instead of a single chord. The
//! chord drop is not merely a vertex-count problem — it is an UNBOUNDED
//! corner cut (sagitta `r(1 - cos(θ/2))`, i.e. 29% of the offset distance at
//! a 90° join) that no consumer has ever bounded.
//!
//! ```text
//! cargo test --release -p rs_cam_core --test offset_candidates_m5 \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! # Quality, not just vertex counts
//!
//! Two oracles, because a candidate that changes ring geometry has to be
//! measured on the surface:
//!
//! * **2D** — [`common::offset_lab::erosion_error`]: every vertex of ring `k`
//!   must sit at exactly `k · step` from the original boundary. Exact, no
//!   tolerance dial. Plus a rasterised area check, which catches an arm that
//!   keeps its vertices honest while losing whole regions.
//! * **3D** — M4's `EnvelopeOracle`, driven through the `RingCleanup`
//!   research seam on scallop's own cascade, so achieved cusp and gouge are
//!   read off the same instrument Checkpoint C used.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout, clippy::print_stderr)]

mod common;

use std::time::Instant;

use common::offset_lab::{
    ArcPolicy, Cascade, CascadeBudget, Cleanup, OffsetFixture, cascade, erosion_area,
    erosion_error, fixtures, geo_cascade, out_dir, pline_cascade, rosette, signed_area_with_holes,
    square, total_vertices,
};
use common::scallop_oracle::{EnvelopeOracle, OracleGrid, OracleParams, StampKernel};
use common::{meshes, tools};
use rs_cam_core::finish_setup::FinishResolutionPolicy;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pocket::pocket_contours_with_cancel;
use rs_cam_core::polygon::{Polygon2, cleanup_collinear, simplify_bounded};
use rs_cam_core::scallop::{
    RingCleanup, ScallopDirection, ScallopParams, ScallopRingBudget, ScallopStepoverPolicy,
    scallop_toolpath_research, scallop_toolpath_structured_annotated_with_resolution,
};
use rs_cam_core::tool::MillingCutter;

/// The flattening tolerance every arc-carrying arm is scored at: 1 µm, a
/// hundredth of the coarsest finish chord tolerance in the workspace and the
/// same order as the coarsest post's coordinate quantum.
const FLATTEN_TOL_MM: f64 = 1.0e-3;

fn arms(fx: &OffsetFixture) -> Vec<Arm> {
    vec![
        Arm::Poly(Cleanup::Raw),
        Arm::Poly(Cleanup::DropOnly {
            min_spacing: fx.authored_spacing * 0.75,
        }),
        Arm::Poly(Cleanup::RemoveRedundant { eps: 1e-5 }),
        Arm::Poly(Cleanup::CollinearDedup { tol: 1e-6 }),
        Arm::Poly(Cleanup::Simplify { tol: 1e-2 }),
        Arm::Pline(ArcPolicy::ChordPerRing),
        Arm::Pline(ArcPolicy::Preserve),
        Arm::Pline(ArcPolicy::BoundedPerRing {
            tol: FLATTEN_TOL_MM,
        }),
        Arm::Geo,
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Arm {
    Poly(Cleanup),
    Pline(ArcPolicy),
    Geo,
}

impl Arm {
    fn id(self) -> &'static str {
        match self {
            Self::Poly(c) => c.id(),
            Self::Pline(p) => p.id(),
            Self::Geo => "C7",
        }
    }
    fn label(self) -> String {
        match self {
            Self::Poly(c) => c.label(),
            Self::Pline(p) => p.label(),
            Self::Geo => "C7 geo::Buffer (i_overlay)".to_owned(),
        }
    }
    fn run(self, fx: &OffsetFixture, budget: &CascadeBudget, keep: bool) -> Cascade {
        match self {
            Self::Poly(c) => cascade(fx, c, budget, keep),
            Self::Pline(p) => pline_cascade(fx, p, FLATTEN_TOL_MM, budget, keep),
            Self::Geo => geo_cascade(fx, budget, keep),
        }
    }
}

// ---------------------------------------------------------------------------
// Instrument validation — M4's lesson: check the oracle before believing it
// ---------------------------------------------------------------------------

/// The erosion oracle must read zero on a case whose erosion is known in
/// closed form: a square inset `k` times by `step` is a square inset once by
/// `k · step`, and every vertex of it is exactly `k · step` from the original.
#[test]
fn the_erosion_oracle_reads_zero_on_a_known_erosion() {
    let fx = OffsetFixture {
        name: "square",
        what: "closed form",
        poly: square(100.0),
        step: 0.5,
        authored_spacing: 0.5,
    };
    let run = cascade(
        &fx,
        Cleanup::Raw,
        &CascadeBudget {
            max_rings: 20,
            ..CascadeBudget::default()
        },
        true,
    );
    for (i, ring) in run.rings.iter().enumerate() {
        let k = (i + 1) as f64;
        let e = erosion_error(&fx.poly, ring, k * fx.step);
        assert!(
            e.max_abs_mm < 1e-9,
            "oracle reads {:.3e} mm on ring {} of a square — it is not exact",
            e.max_abs_mm,
            i + 1
        );
    }
    // And it is not vacuously zero: a ring at the WRONG depth must read the
    // difference exactly.
    let e = erosion_error(&fx.poly, &run.rings[0], 2.0 * fx.step);
    assert!((e.max_abs_mm - fx.step).abs() < 1e-9, "{:?}", e.max_abs_mm);
}

/// The rasterised area truth must agree with the closed form on the same
/// square, to within its own cell size.
#[test]
fn the_area_oracle_agrees_with_the_closed_form() {
    let poly = square(100.0);
    let truth = erosion_area(&poly, 5.0, 0.25);
    let exact = 90.0 * 90.0;
    assert!(
        (truth - exact).abs() / exact < 0.01,
        "rasterised erosion area {truth:.0} vs exact {exact:.0}"
    );
}

/// `simplify_bounded` must keep its promise: every vertex of the input is
/// within `tol` of the simplified chain. This is the property the whole
/// candidate rests on, and it is checked on the real fixtures, not a toy.
#[test]
fn simplify_bounded_respects_its_tolerance() {
    let tol = 1e-2;
    for fx in fixtures() {
        let Some(simple) = simplify_bounded(&fx.poly, tol) else {
            panic!("{}: simplify returned nothing", fx.name)
        };
        let worst = fx
            .poly
            .exterior
            .iter()
            .map(|p| common::offset_lab::distance_to_boundary(&simple, p))
            .fold(0.0_f64, f64::max);
        assert!(
            worst <= tol * 1.001,
            "{}: simplification moved the boundary by {worst:.5} mm at tol {tol}",
            fx.name
        );
    }
}

/// And `cleanup_collinear` at 1 nm must move nothing measurable at all.
#[test]
fn cleanup_collinear_is_lossless() {
    for fx in fixtures() {
        let Some(clean) = cleanup_collinear(&fx.poly, 1e-5, 1e-6) else {
            continue;
        };
        let worst = fx
            .poly
            .exterior
            .iter()
            .map(|p| common::offset_lab::distance_to_boundary(&clean, p))
            .fold(0.0_f64, f64::max);
        assert!(
            worst < 1e-5,
            "{}: lossless cleanup moved the boundary by {worst:.3e} mm",
            fx.name
        );
    }
}

/// Non-vacuity: the arms must be distinct policies, and on the stress fixture
/// they must not all produce the same cascade.
#[test]
fn the_arms_are_actually_different() {
    let fx = OffsetFixture {
        name: "rosette-24",
        what: "stress",
        poly: rosette(60.0, 8.0, 24, 720),
        step: 0.1,
        authored_spacing: 0.5,
    };
    let all = arms(&fx);
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            assert_ne!(a, b, "duplicate arm {} / {}", a.id(), b.id());
        }
    }
    let budget = CascadeBudget {
        max_rings: 5,
        vertex_cap: 60_000,
        seconds: 30.0,
    };
    let counts: Vec<usize> = all
        .iter()
        .map(|a| a.run(&fx, &budget, false).last().map_or(0, |s| s.verts))
        .collect();
    assert!(
        counts
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 3,
        "arms produced {counts:?} — too few distinct outcomes to be a comparison"
    );
}

/// The scallop research seam must be a no-op at its shipped setting, or every
/// number in the oracle table below is measured against the wrong baseline.
#[test]
fn shipped_ring_cleanup_reproduces_the_shipped_scallop_path() {
    let tool = tools::wanaka_taper();
    let cancel = || false;
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);

    for (name, mesh, p) in oracle_fixtures() {
        let index = SpatialIndex::build(&mesh, 2.0);
        let (a, _, a_rep) = scallop_toolpath_structured_annotated_with_resolution(
            &mesh, &index, &tool, &p, None, None, resolution, &cancel,
        )
        .expect("never-cancel");
        let (b, _, b_rep, _) = scallop_toolpath_research(
            &mesh,
            &index,
            &tool,
            &p,
            None,
            None,
            resolution,
            ScallopRingBudget::FlatGroundStepover,
            ScallopStepoverPolicy::SHIPPED,
            &cancel,
        )
        .expect("never-cancel");
        assert_eq!(
            common::fingerprint::move_fingerprint(&a),
            common::fingerprint::move_fingerprint(&b),
            "{name}: SHIPPED policy must reproduce the shipped toolpath exactly"
        );
        assert_eq!(a_rep.ring_count, b_rep.ring_count, "{name}");
    }
    assert!(ScallopStepoverPolicy::SHIPPED.is_shipped());
    assert!(ScallopStepoverPolicy::default().is_shipped());
    // Wave 14: the shipped cleanup is the arc-carrying cascade. Checkpoint D
    // adopted it; `DecimateAtCell` survives only as a research arm.
    assert_eq!(RingCleanup::default(), RingCleanup::ArcCascade);
    assert!(RingCleanup::ArcCascade.carries_arcs());
    assert!(!RingCleanup::DecimateAtCell.carries_arcs());
}

// ---------------------------------------------------------------------------
// Evidence: the candidate table
// ---------------------------------------------------------------------------

#[test]
#[ignore = "M5 evidence: 9 arms x 8 fixtures; --ignored --nocapture"]
fn candidate_arms_on_every_fixture() {
    let budget = CascadeBudget {
        max_rings: 50,
        vertex_cap: 120_000,
        seconds: 25.0,
    };
    let mut csv = String::from(
        "fixture,arm,rings,verts_end,flat_verts_end,pct_per_ring,secs,oracle_ring,err_max_um,err_p50_um,err_mean_signed_um,stop\n",
    );
    for fx in fixtures() {
        println!("\n### {} — {}", fx.name, fx.what);
        println!(
            "| arm    | policy                        | rings | verts_end | flat_end | %/ring | secs  | err@K max | p50    | signed | stop        |"
        );
        println!(
            "|--------|-------------------------------|-------|-----------|----------|--------|-------|-----------|--------|--------|-------------|"
        );
        // Score every arm at the same depth, so the comparison is a
        // comparison: the deepest ring the WEAKEST arm reached, capped at 20.
        let runs: Vec<(Arm, Cascade)> = arms(&fx)
            .into_iter()
            .map(|a| {
                let run = a.run(&fx, &budget, true);
                (a, run)
            })
            .collect();
        let k = runs
            .iter()
            .map(|(_, r)| r.rings.len())
            .min()
            .unwrap_or(0)
            .min(20);
        for (arm, run) in &runs {
            let last = run.last();
            let (max_um, p50_um, signed_um) = if k > 0 {
                let e = erosion_error(&fx.poly, &run.rings[k - 1], k as f64 * fx.step);
                (
                    e.max_abs_mm * 1000.0,
                    e.p50_mm * 1000.0,
                    e.mean_signed_mm * 1000.0,
                )
            } else {
                (f64::NAN, f64::NAN, f64::NAN)
            };
            println!(
                "| {:<6} | {:<29} | {:>5} | {:>9} | {:>8} | {:>6.2} | {:>5.1} | {:>9.1} | {:>6.1} | {:>+6.1} | {:<11} |",
                arm.id(),
                arm.label(),
                run.stats.len(),
                last.map_or(0, |s| s.verts),
                last.map_or(0, |s| s.flat_verts),
                run.geometric_growth_pct(),
                run.total_secs,
                max_um,
                p50_um,
                signed_um,
                run.stopped.label(),
            );
            csv.push_str(&format!(
                "{},{},{},{},{},{:.3},{:.3},{},{:.3},{:.3},{:.3},{}\n",
                fx.name,
                arm.id(),
                run.stats.len(),
                last.map_or(0, |s| s.verts),
                last.map_or(0, |s| s.flat_verts),
                run.geometric_growth_pct(),
                run.total_secs,
                k,
                max_um,
                p50_um,
                signed_um,
                run.stopped.label()
            ));
        }
        println!("(err@K: distance of every ring-{k} vertex from k*step, in µm — 0 is exact)");
    }
    let path = out_dir().join("candidates.csv");
    std::fs::write(&path, csv).expect("write candidate CSV");
    println!("\nCSV: {}", path.display());
}

/// The second half of the 2D oracle: does the arm still enclose the right
/// AREA? An arm can keep every vertex at the right distance and still have
/// dropped an island.
#[test]
#[ignore = "M5 evidence: rasterised erosion area; --ignored --nocapture"]
fn candidate_arms_keep_the_eroded_area() {
    let budget = CascadeBudget {
        max_rings: 20,
        vertex_cap: 120_000,
        seconds: 25.0,
    };
    for fx in fixtures().into_iter().filter(|f| {
        matches!(
            f.name,
            "rosette-24" | "comb-16" | "terrain-midsteep" | "holed-9"
        )
    }) {
        let k = 20usize;
        let truth = erosion_area(&fx.poly, k as f64 * fx.step, 0.2);
        println!(
            "\n### {} — rasterised erosion area at {:.1} mm inset: {truth:.1} mm^2",
            fx.name,
            k as f64 * fx.step
        );
        println!("| arm    | policy                        | rings | area mm^2 | vs truth |");
        println!("|--------|-------------------------------|-------|-----------|----------|");
        for arm in arms(&fx) {
            let run = arm.run(&fx, &budget, true);
            let area = run
                .rings
                .get(k - 1)
                .map(|r| r.iter().map(signed_area_with_holes).sum::<f64>())
                .unwrap_or(f64::NAN);
            println!(
                "| {:<6} | {:<29} | {:>5} | {:>9.1} | {:>7.2}% |",
                arm.id(),
                arm.label(),
                run.stats.len(),
                area,
                100.0 * (area - truth) / truth,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Evidence: a 2D consumer — pocket
// ---------------------------------------------------------------------------

/// `pocket_offsets` is the library's own concentric-ring cascade and it takes
/// `offset_polygon` raw. Score its rings against the analytic erosion, on a
/// synthetic shape whose answer is known.
///
/// **`pocket_offsets` is NOT called here, deliberately.** It has no deadline
/// and no cancel hook, and on this 12-vertex cross it ran 13 minutes to
/// 386 MB without returning (M5, 2026-08-03) — the raw doubling, in the
/// library's own cascade. The production entry point
/// `pocket_contours_with_cancel` is used instead, with a wall-clock deadline
/// as its cancel, which both bounds this test and measures the thing worth
/// measuring: how far a real pocket gets before a user would give up.
#[test]
#[ignore = "M5 evidence: pocket-boundary fidelity; --ignored --nocapture"]
fn pocket_boundary_fidelity_against_the_analytic_erosion() {
    // A plus/cross: four reflex corners, an exactly known erosion, and the
    // shape class 2.5D pocketing actually meets.
    let a = 20.0;
    let b = 60.0;
    let poly = Polygon2::new(vec![
        P2::new(a, 0.0),
        P2::new(b, 0.0),
        P2::new(b, a),
        P2::new(b + a, a),
        P2::new(b + a, b),
        P2::new(b, b),
        P2::new(b, b + a),
        P2::new(a, b + a),
        P2::new(a, b),
        P2::new(0.0, b),
        P2::new(0.0, a),
        P2::new(a, a),
    ]);
    let stepover = 0.5;

    // The production pocket path, with a 20-second deadline as its cancel.
    let deadline = Instant::now() + std::time::Duration::from_secs(20);
    let expired = move || Instant::now() > deadline;
    let t0 = Instant::now();
    let contours = pocket_contours_with_cancel(&poly, 0.0, stepover, &expired);
    let secs = t0.elapsed().as_secs_f64();
    match &contours {
        Ok(c) => println!(
            "\npocket_contours_with_cancel: FINISHED in {secs:.1} s — {} contours, {} points",
            c.len(),
            c.iter().map(Vec::len).sum::<usize>()
        ),
        Err(_) => println!(
            "\npocket_contours_with_cancel: **DID NOT FINISH** — cancelled by the 20 s deadline \
             on a 12-vertex cross at {stepover} mm stepover"
        ),
    }
    // How deep does the same cascade get before it is unusable? Bounded run,
    // measured against the analytic erosion at every ring.
    let fx = OffsetFixture {
        name: "cross",
        what: "pocket fidelity",
        poly,
        step: stepover,
        authored_spacing: 0.5,
    };
    let raw = cascade(
        &fx,
        Cleanup::Raw,
        &CascadeBudget {
            max_rings: 40,
            vertex_cap: 500_000,
            seconds: 20.0,
        },
        true,
    );
    println!(
        "raw cascade on the same cross: {} rings in {:.1} s, stopped on {}",
        raw.stats.len(),
        raw.total_secs,
        raw.stopped.label()
    );
    println!("| ring | polys | verts | err max µm | err p50 µm | signed µm |");
    println!("|------|-------|-------|------------|------------|-----------|");
    for (i, layer) in raw.rings.iter().enumerate() {
        let k = (i + 1) as f64;
        let e = erosion_error(&fx.poly, layer, k * stepover);
        if i < 6 || i % 5 == 0 || i + 1 == raw.rings.len() {
            println!(
                "| {:>4} | {:>5} | {:>5} | {:>10.2} | {:>10.2} | {:>+9.2} |",
                i + 1,
                layer.len(),
                total_vertices(layer),
                e.max_abs_mm * 1000.0,
                e.p50_mm * 1000.0,
                e.mean_signed_mm * 1000.0,
            );
        }
    }

    // Same shape, same stepover, through each candidate.
    let budget = CascadeBudget {
        max_rings: 40,
        vertex_cap: 120_000,
        seconds: 25.0,
    };
    println!(
        "\n| arm    | policy                        | rings | verts_end | err max µm | signed µm |"
    );
    println!(
        "|--------|-------------------------------|-------|-----------|------------|-----------|"
    );
    for arm in arms(&fx) {
        let run = arm.run(&fx, &budget, true);
        let k = run.rings.len();
        let e = if k > 0 {
            erosion_error(&fx.poly, &run.rings[k - 1], k as f64 * fx.step)
        } else {
            continue;
        };
        println!(
            "| {:<6} | {:<29} | {:>5} | {:>9} | {:>10.2} | {:>+9.2} |",
            arm.id(),
            arm.label(),
            k,
            run.last().map_or(0, |s| s.flat_verts),
            e.max_abs_mm * 1000.0,
            e.mean_signed_mm * 1000.0,
        );
    }
}

// ---------------------------------------------------------------------------
// Evidence: the 3D oracle, through scallop's own cascade
// ---------------------------------------------------------------------------

const TOLERANCE_MM: f64 = 0.10;
const DIAL_MM: f64 = 0.020;
const HALF: f64 = 8.0;
const MESH_STEP: f64 = 0.25;
const ORACLE_CELL_MM: f64 = 0.020;
const STAMP_CAP_TIP_MULTIPLE: f64 = 2.0;

fn scallop_params() -> ScallopParams {
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
        intra_pass_hookup_mm: 0.0,
        link_kinematics: None,
    }
}

/// Checkpoint C's shapes, so the two evidence packs compare: a groove with
/// genuine constant-slope walls, and a high-curvature ridge — plus, since
/// wave 14, one fixture whose REGION BOUNDARY IS CONCAVE.
///
/// That third entry is not decoration. Checkpoint C's two fixtures both run
/// scallop over its default boundary, the mesh-bbox **rectangle**, and an
/// inward parallel offset of a convex polygon **creates no arc joins at
/// all** — the offset corners simply intersect. So on those two fixtures
/// every arc-related cleanup is a no-op by construction, and the whole
/// M4 cleanup table there is measuring one thing only: whether drop-only
/// decimation kills the last ring or two as the rectangle shrinks below its
/// spacing. §15 of `CHECKPOINT_D_EVIDENCE.md` recorded the resulting
/// "identical cleanup rows" as unexplained; this is the explanation, and the
/// remedy is a boundary with reflex corners in it.
///
/// A slope band supplies a concave region honestly — restrict the op to the
/// groove FLANKS (`slope_from: 20.0`) and the region becomes stripes whose
/// marching-squares boundary is full of reflex corners. **That variant was
/// built, run, and is NOT in the list below**, because the oracle scores the
/// whole mesh: with the flats deliberately unmachined it reports 640 mm²
/// "unfinished" and a 216x cusp ratio, both of which are territory the
/// operation was never asked to cut. Scoring a band-restricted op needs a
/// band-restricted population, and that is M4's instrument to extend, not
/// this file's. The concave evidence in this wave comes from instruments that
/// can carry it: the 2D erosion oracle (exact, §6), the 200-ring growth gate
/// (§7), the pocket cross (§8), the chord-sag gate on the mixed-slope ribbon,
/// and `crease_own_region_pr6b`'s region-scoped unified-finish fingerprints.
fn oracle_fixtures() -> Vec<(&'static str, TriangleMesh, ScallopParams)> {
    vec![
        (
            "grooved block",
            meshes::grooved_block(6.0, 60.0, 3.0),
            scallop_params(),
        ),
        (
            "narrow ridge",
            meshes::height_field(HALF, MESH_STEP, |x, _| 4.0 * (1.0 - x.abs()).max(0.0)),
            scallop_params(),
        ),
    ]
}

#[test]
#[ignore = "M5 evidence: M4 envelope oracle across ring cleanups; --ignored --nocapture"]
fn scallop_oracle_across_ring_cleanups() {
    let tool = tools::wanaka_taper();
    let cancel = || false;
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);

    for (name, mesh, p) in oracle_fixtures() {
        let index = SpatialIndex::build(&mesh, 2.0);
        println!("\n### {name} — scallop cascade under each ring cleanup");
        println!(
            "| cleanup           | secs  | rings | moves  | cusp p99 µm | ratio | gouge-normal µm | unfinished mm^2 | gouge mm^2 |"
        );
        println!(
            "|-------------------|-------|-------|--------|-------------|-------|-----------------|-----------------|------------|"
        );
        for cleanup in [
            RingCleanup::ArcCascade,
            RingCleanup::DecimateAtCell,
            RingCleanup::KeepEverything,
            RingCleanup::CollinearDedup,
            RingCleanup::SimplifyBounded,
        ] {
            let policy = ScallopStepoverPolicy {
                cleanup,
                ..ScallopStepoverPolicy::SHIPPED
            };
            let t0 = Instant::now();
            let Ok((tp, _anns, report, _trace)) = scallop_toolpath_research(
                &mesh,
                &index,
                &tool,
                &p,
                None,
                None,
                resolution,
                ScallopRingBudget::FlatGroundStepover,
                policy,
                &cancel,
            ) else {
                println!("| {:<17} | cancelled |", cleanup.label());
                continue;
            };
            let seconds = t0.elapsed().as_secs_f64();

            let grid = OracleGrid::for_mesh(&mesh, &tool, ORACLE_CELL_MM);
            let truth = EnvelopeOracle::true_surface_from_mesh(grid, &mesh, &index);
            let kernel = StampKernel::new(
                &tool,
                ORACLE_CELL_MM,
                Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
            );
            let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, ORACLE_CELL_MM);
            let mut op = OracleParams::new(DIAL_MM);
            op.stock_to_leave = p.stock_to_leave;
            let rep = oracle.report(op);

            println!(
                "| {:<17} | {:>5.1} | {:>5} | {:>6} | {:>11.1} | {:>5.2} | {:>15.1} | {:>15.2} | {:>10.2} |",
                cleanup.label(),
                seconds,
                report.ring_count,
                tp.moves.len(),
                rep.achieved_cusp_normal_um,
                rep.cusp_ratio_normal(),
                rep.deepest_gouge_normal_um,
                rep.unfinished_mm2(),
                rep.gouge_mm2,
            );
        }
    }
}

/// **M5's acceptance gate, measured**: "vertex count grows linearly or
/// remains bounded across 200 repeated offsets on the stress fixture".
///
/// C0 and the two arms that inflate are included so the gate has a
/// falsifying side.
#[test]
#[ignore = "M5 evidence: the 200-ring acceptance gate; --ignored --nocapture"]
fn two_hundred_offsets_on_the_stress_fixtures() {
    let budget = CascadeBudget {
        max_rings: 200,
        vertex_cap: 150_000,
        seconds: 30.0,
    };
    for fx in fixtures()
        .into_iter()
        .filter(|f| matches!(f.name, "rosette-24" | "dendrite" | "terrain-midsteep"))
    {
        println!("\n### {} — 200 repeated offsets", fx.name);
        println!(
            "| arm    | policy                        | rings | verts@50 | verts@100 | verts@200 | secs  | stop        |"
        );
        println!(
            "|--------|-------------------------------|-------|----------|-----------|-----------|-------|-------------|"
        );
        for arm in arms(&fx) {
            let run = arm.run(&fx, &budget, false);
            let at = |k: usize| -> String {
                run.stats
                    .get(k - 1)
                    .map_or_else(|| "—".to_owned(), |s| s.verts.to_string())
            };
            println!(
                "| {:<6} | {:<29} | {:>5} | {:>8} | {:>9} | {:>9} | {:>5.1} | {:<11} |",
                arm.id(),
                arm.label(),
                run.stats.len(),
                at(50),
                at(100),
                at(200),
                run.total_secs,
                run.stopped.label(),
            );
        }
    }
}
