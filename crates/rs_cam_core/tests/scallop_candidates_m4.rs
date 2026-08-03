//! **M4 research phase B — the candidates, scored by the validated oracle.**
//!
//! Phase A (`scallop_oracle_validation_m4.rs`) built an instrument and checked
//! it against closed-form ground truth. This file points it at the shipped
//! ring cascade and at every candidate the plan names, on fixtures with known
//! shape.
//!
//! # The arms
//!
//! `ScallopStepoverPolicy` makes each stacked compensation selectable, so the
//! arms are not "algorithm A vs algorithm B" but a **decomposition**: turn one
//! compensation off at a time and the oracle says what that one was costing.
//! The plan's ordered candidate list maps onto them as:
//!
//! | plan item | arm(s) |
//! |---|---|
//! | 1 — segmented / local stepover rings | `A1` per-polygon, `A2` every-vertex, `A6`/`A7` corrected law |
//! | 2 — variable-distance polygon offset | **assessed, not built** — see `CHECKPOINT_C_EVIDENCE.md`; `offset_polygon` has no variable-distance mode and M5 owns that file |
//! | 3 — level-set / iso-scallop field | `A8`, `A9` (`scallop_isofield`) |
//! | 4 — median-ratio clamp (benchmark only) | `A4` |
//!
//! # Running the evidence
//!
//! ```text
//! cargo test --release -p rs_cam_core --test scallop_candidates_m4 \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The non-ignored tests are the parity and non-vacuity guards and run in
//! seconds; they are what stops the seam rotting.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout, clippy::print_stderr)]

mod common;

use std::time::Instant;

use common::scallop_oracle::{
    EnvelopeOracle, OracleGrid, OracleParams, OracleReport, PathStructure, SlopeBand, StampKernel,
    path_structure, render_field,
};
use common::{meshes, tools};
use rs_cam_core::finish_setup::FinishResolutionPolicy;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::scallop::{
    CurvaturePolicy, PolygonReduce, RingReducer, RingSampling, RingSource, ScallopDirection,
    ScallopParams, ScallopRingBudget, ScallopStepoverPolicy, ScallopStepoverTrace,
    StepoverGeometry, scallop_toolpath_research,
    scallop_toolpath_structured_annotated_with_resolution,
};
use rs_cam_core::tool::MillingCutter;

// ---------------------------------------------------------------------------
// Constants — deliberately Checkpoint B's, so the two evidence packs compare
// ---------------------------------------------------------------------------

const TOLERANCE_MM: f64 = 0.10;
const DIAL_MM: f64 = 0.020;
const HALF: f64 = 8.0;
const MESH_STEP: f64 = 0.25;
/// Oracle cell. `scallop_oracle_validation_m4::oracle_converges_in_path_step_and_in_cell`
/// shows 0.020 mm reproduces the closed-form cusp exactly for this tool scale.
const ORACLE_CELL_MM: f64 = 0.020;
/// The stamp is capped at 2× the tip radius. The wanaka taper's ENVELOPE is
/// its Ø6 shank, 36× the tip area, and the flank cannot touch below 83° of
/// slope — pinned by `taper_stamp_radius_cap_is_safe_below_the_flank_contact_slope`.
const STAMP_CAP_TIP_MULTIPLE: f64 = 2.0;

fn cutter() -> impl MillingCutter {
    tools::wanaka_taper()
}

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
        // Candidate-selection fixture predates A/M7; keep the old
        // per-ring retract so the candidate set doesn't move underneath it.
        intra_pass_hookup_mm: 0.0,
        link_kinematics: None,
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct Fixture {
    name: &'static str,
    mesh: TriangleMesh,
    index: SpatialIndex,
}

impl Fixture {
    fn new(name: &'static str, mesh: TriangleMesh) -> Self {
        let index = SpatialIndex::build(&mesh, 2.0);
        Self { name, mesh, index }
    }
}

/// Five shapes, each chosen because it isolates something.
///
/// * **flat ground** — the control. Slope is 0 everywhere, so every arm's
///   stepover law collapses to the same flat formula and any difference here
///   is the *machinery*, not the geometry.
/// * **grooved block** — C6's shared trapezoidal groove: two long constant
///   slope walls and a flat floor, i.e. a ring that genuinely crosses
///   homogeneous spans of different slope. The plan asks for exactly this.
/// * **narrow ridge** — Checkpoint B's, ~76° flanks 2 mm apart: high curvature
///   and high slope on the same feature.
/// * **mixed-slope ribbon** — Checkpoint B's 6°/60°/85° bands. One ring
///   crosses all three, which is the min-across-ring worst case by design.
/// * **dome** — smooth, everywhere-curved, no sharp features at all: the
///   fixture where a curvature-driven collapse cannot hide behind a crease.
fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new(
            "flat ground",
            meshes::height_field(HALF, MESH_STEP, |_, _| 0.0),
        ),
        Fixture::new("grooved block", meshes::grooved_block(6.0, 60.0, 3.0)),
        Fixture::new(
            "narrow ridge",
            meshes::height_field(HALF, MESH_STEP, |x, _| 4.0 * (1.0 - x.abs()).max(0.0)),
        ),
        Fixture::new("mixed-slope ribbon", mixed_slope_ribbon()),
        Fixture::new(
            "dome",
            meshes::height_field(HALF, MESH_STEP, |x, y| {
                let r2 = x * x + y * y;
                (144.0 - r2).max(0.0).sqrt() - 12.0
            }),
        ),
    ]
}

/// Checkpoint B's ribbon, reproduced knot for knot: bands at roughly
/// 6° / 60° / 85°.
fn mixed_slope_ribbon() -> TriangleMesh {
    meshes::height_field(HALF, MESH_STEP, |x, _| {
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

// ---------------------------------------------------------------------------
// Arms
// ---------------------------------------------------------------------------

struct Arm {
    id: &'static str,
    what: &'static str,
    policy: ScallopStepoverPolicy,
}

fn arms() -> Vec<Arm> {
    let s = ScallopStepoverPolicy::SHIPPED;
    vec![
        Arm {
            id: "A0",
            what: "shipped",
            policy: s,
        },
        Arm {
            id: "A1",
            what: "retire min-across-POLYGONS",
            policy: ScallopStepoverPolicy {
                across_polygons: PolygonReduce::PerPolygon,
                ..s
            },
        },
        Arm {
            id: "A2",
            what: "retire fixed-20 sampling",
            policy: ScallopStepoverPolicy {
                sampling: RingSampling::EveryVertex,
                ..s
            },
        },
        Arm {
            id: "A3",
            what: "retire min-across-RING (p10)",
            policy: ScallopStepoverPolicy {
                reducer: RingReducer::P10,
                sampling: RingSampling::EveryVertex,
                ..s
            },
        },
        Arm {
            id: "A4",
            what: "median-ratio clamp (benchmark only)",
            policy: ScallopStepoverPolicy {
                reducer: RingReducer::Median,
                sampling: RingSampling::EveryVertex,
                ..s
            },
        },
        Arm {
            id: "A5",
            what: "tool-limited curvature",
            policy: ScallopStepoverPolicy {
                curvature: CurvaturePolicy::ToolLimited,
                ..s
            },
        },
        Arm {
            id: "A6",
            what: "corrected slope law (cosθ)",
            policy: ScallopStepoverPolicy {
                geometry: StepoverGeometry::CosineSlope,
                ..s
            },
        },
        Arm {
            id: "A7",
            what: "corrected cascade (cosθ + κ cap + every vertex + per polygon, MIN kept)",
            policy: ScallopStepoverPolicy {
                geometry: StepoverGeometry::CosineSlope,
                curvature: CurvaturePolicy::ToolLimited,
                sampling: RingSampling::EveryVertex,
                across_polygons: PolygonReduce::PerPolygon,
                reducer: RingReducer::Min,
                ..s
            },
        },
        Arm {
            id: "A8",
            what: "iso-field, shipped stepover law",
            policy: ScallopStepoverPolicy {
                ring_source: RingSource::IsoField,
                ..s
            },
        },
        Arm {
            id: "A9",
            what: "iso-field, corrected stepover law",
            policy: ScallopStepoverPolicy {
                ring_source: RingSource::IsoField,
                geometry: StepoverGeometry::CosineSlope,
                curvature: CurvaturePolicy::ToolLimited,
                ..s
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// Running and scoring one arm
// ---------------------------------------------------------------------------

struct Run {
    seconds: f64,
    rings: usize,
    cascade_rings: usize,
    uncut_core_mm2: f64,
    #[allow(dead_code)]
    // reported by hand in the ad-hoc probes, kept for parity with Checkpoint B's table
    moves: usize,
    report: OracleReport,
    structure: PathStructure,
    trace: ScallopStepoverTrace,
    residuals: Vec<f64>,
    grid: OracleGrid,
}

fn run_arm(
    fixture: &Fixture,
    policy: ScallopStepoverPolicy,
    resolution: FinishResolutionPolicy,
) -> Run {
    let tool = cutter();
    let p = params();
    let cancel = || false;

    let t0 = Instant::now();
    let (tp, anns, report, trace) = scallop_toolpath_research(
        &fixture.mesh,
        &fixture.index,
        &tool,
        &p,
        None,
        None,
        resolution,
        ScallopRingBudget::FlatGroundStepover,
        policy,
        &cancel,
    )
    .expect("never-cancel");
    let seconds = t0.elapsed().as_secs_f64();

    let grid = OracleGrid::for_mesh(&fixture.mesh, &tool, ORACLE_CELL_MM);
    let truth = EnvelopeOracle::true_surface_from_mesh(grid, &fixture.mesh, &fixture.index);
    let kernel = StampKernel::new(
        &tool,
        ORACLE_CELL_MM,
        Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
    );
    let oracle = EnvelopeOracle::score(grid, truth, &tp, &kernel, ORACLE_CELL_MM);
    let mut op = OracleParams::new(DIAL_MM);
    op.stock_to_leave = p.stock_to_leave;

    let ring_starts: Vec<usize> = anns.iter().map(|a| a.move_index).collect();
    Run {
        seconds,
        rings: report.ring_count,
        cascade_rings: report.cascade_ring_count,
        uncut_core_mm2: report.uncut_core_mm2,
        moves: tp.moves.len(),
        report: oracle.report(op),
        structure: path_structure(&tp, &ring_starts, tool.cusp_radius_mm().max(0.25)),
        trace,
        residuals: oracle.residual_map(p.stock_to_leave),
        grid,
    }
}

// ---------------------------------------------------------------------------
// Guards — fast, always run
// ---------------------------------------------------------------------------

/// The research seam must be a no-op at its shipped setting. If this ever
/// fails, every number in `CHECKPOINT_C_EVIDENCE.md` is measured against the
/// wrong baseline.
#[test]
fn shipped_policy_reproduces_the_shipped_path_byte_for_byte() {
    let tool = cutter();
    let p = params();
    let cancel = || false;
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);

    for fixture in fixtures() {
        let (a, a_anns, a_rep) = scallop_toolpath_structured_annotated_with_resolution(
            &fixture.mesh,
            &fixture.index,
            &tool,
            &p,
            None,
            None,
            resolution,
            &cancel,
        )
        .expect("never-cancel");
        let (b, b_anns, b_rep, _trace) = scallop_toolpath_research(
            &fixture.mesh,
            &fixture.index,
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
            "{}: SHIPPED policy must reproduce the shipped toolpath exactly",
            fixture.name
        );
        assert_eq!(a_anns.len(), b_anns.len(), "{}", fixture.name);
        assert_eq!(a_rep.ring_count, b_rep.ring_count, "{}", fixture.name);
        assert_eq!(
            a_rep.uncut_core_mm2, b_rep.uncut_core_mm2,
            "{}",
            fixture.name
        );
    }
    assert!(ScallopStepoverPolicy::SHIPPED.is_shipped());
    assert!(ScallopStepoverPolicy::default().is_shipped());
}

/// Non-vacuity: every arm must be a different policy, and on a fixture with
/// slope they must not all produce the same toolpath. A comparison between
/// ten identical arms is worse than no comparison.
#[test]
fn the_arms_are_actually_different() {
    let all = arms();
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            assert_ne!(
                a.policy, b.policy,
                "arms {} and {} are the same policy",
                a.id, b.id
            );
        }
    }

    let fixture = Fixture::new("ribbon", mixed_slope_ribbon());
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&cutter(), TOLERANCE_MM);
    let tool = cutter();
    let p = params();
    let cancel = || false;
    let mut prints = Vec::new();
    for arm in &all {
        let (tp, ..) = scallop_toolpath_research(
            &fixture.mesh,
            &fixture.index,
            &tool,
            &p,
            None,
            None,
            resolution,
            ScallopRingBudget::FlatGroundStepover,
            arm.policy,
            &cancel,
        )
        .expect("never-cancel");
        prints.push((arm.id, common::fingerprint::move_hash(&tp), tp.moves.len()));
    }
    let distinct: std::collections::HashSet<u64> = prints.iter().map(|(_, h, _)| *h).collect();
    for (id, hash, moves) in &prints {
        eprintln!("{id}: {moves} moves, fingerprint {hash:016x}");
    }
    let print_of = |id: &str| -> u64 {
        prints
            .iter()
            .find(|(i, ..)| *i == id)
            .map(|(_, h, _)| *h)
            .unwrap_or_default()
    };

    // Wave 14 (arc-carrying cascade): **A1 coincides with A0 on this fixture**,
    // and the reason is arithmetic rather than a lost distinction.
    //
    // **A1 (`PerPolygon`) ≡ A0 (`MinAcross`)** — the ribbon's cascade never
    // splits into more than one polygon, and a minimum over one number is that
    // number. The old chord-flattened cascade did split, because the arc-join
    // debris eventually pinched a ring in two; that split was an artefact of
    // the defect, not a feature of the shape.
    //
    // It is pinned rather than papered over: if the mechanism changes, this
    // fails and says so.
    assert_eq!(
        print_of("A0"),
        print_of("A1"),
        "A1 must coincide with A0 while the ribbon cascade stays single-polygon \
         (a MIN over one polygon is that polygon)"
    );
    // **A2 must NOT coincide with A0, and this assertion is an erratum.**
    //
    // A mid-wave-14 draft of this file asserted A2 ≡ A0, reasoning that
    // `Fixed20`'s stride is `1.max(ring.len() / 20)` — i.e. 1, therefore
    // every-vertex — for any ring under 40 vertices, and that the arc
    // cascade's rings are that sparse. The reasoning was sound and the
    // premise was measured on a build that does not ship: it predated
    // `scallop::RingSampleBound`, which subdivides a ring's straight runs at
    // the flat-ground stepover so that scallop gets the surface samples it
    // reads every ring vertex as. With the bound the rings clear 40 vertices
    // comfortably, the stride exceeds 1, and the two arms separate again
    // (6132 vs 6589 moves).
    //
    // Recorded rather than quietly deleted because the failure is the
    // instrument working: the claim named its own mechanism, so when the
    // mechanism moved the test said which one had moved instead of just
    // going red.
    assert_ne!(
        print_of("A0"),
        print_of("A2"),
        "A2 must differ from A0 while rings carry more than 40 vertices \
         (Fixed20's stride exceeds 1 there, so it is NOT sampling every vertex)"
    );
    let expected = all.len() - 1;
    assert_eq!(
        distinct.len(),
        expected,
        "on a 6°/60°/85° ribbon the arms must not collapse onto each other \
         beyond the single arithmetic coincidence named above: {} distinct \
         fingerprints from {} arms",
        distinct.len(),
        all.len()
    );
}

/// The corrected slope law must actually be *tighter* on slope and identical
/// on flat ground — the two properties the whole `A6`/`A7`/`A9` family rests
/// on. Cheap enough to be a permanent guard rather than an evidence-run note.
#[test]
fn the_corrected_law_matches_flat_and_tightens_on_slope() {
    let cusp_r = cutter().cusp_radius_mm();
    let flat_shipped =
        StepoverGeometry::ALL[0].label().len() as f64 * 0.0 + shipped_so(cusp_r, 0.0, 0.0);
    let flat_fixed = corrected_so(cusp_r, 0.0, 0.0);
    assert!(
        (flat_shipped - flat_fixed).abs() < 1e-9,
        "on flat ground with zero curvature the two laws must agree: \
         {flat_shipped:.6} vs {flat_fixed:.6}"
    );
    for deg in [15.0_f64, 30.0, 45.0, 60.0, 75.0] {
        let th = deg.to_radians();
        let shipped = shipped_so(cusp_r, th, 0.0);
        let fixed = corrected_so(cusp_r, th, 0.0);
        assert!(
            fixed < shipped,
            "at {deg}° the corrected law must be tighter: shipped {shipped:.4} \
             vs corrected {fixed:.4}"
        );
        // And it must match the geometry: d(θ) = d_flat · cos θ.
        assert!(
            (fixed - flat_fixed * th.cos()).abs() < 1e-9,
            "at {deg}° the corrected law must be exactly d_flat·cosθ"
        );
    }
}

fn shipped_so(cusp_r: f64, angle: f64, curvature: f64) -> f64 {
    rs_cam_core::scallop_math::variable_stepover(cusp_r, DIAL_MM, angle, curvature)
}

fn corrected_so(cusp_r: f64, angle: f64, curvature: f64) -> f64 {
    rs_cam_core::scallop_math::stepover_from_scallop_curved(cusp_r, DIAL_MM, curvature)
        * angle.cos()
}

// ---------------------------------------------------------------------------
// Evidence run 1 — the candidate matrix
// ---------------------------------------------------------------------------

#[test]
#[ignore = "M4 phase B evidence — minutes; produces the Checkpoint C tables and diff maps"]
fn candidate_matrix() {
    let tool = cutter();
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);
    println!(
        "\n# M4 candidate matrix — dial {:.3} mm, wanaka taper Ø1 tip / 7° / Ø6 shank, generation grid {} ({:.3} mm), oracle cell {ORACLE_CELL_MM} mm\n",
        DIAL_MM,
        resolution.mode().label(),
        resolution.cell_mm(),
    );

    for fixture in fixtures() {
        println!("\n## {}\n", fixture.name);
        println!(
            "| arm | what | gen s | rings | uncut core mm² | achieved cusp µm (normal) | ×dial | on dial | untouched mm² | standing mm² | gouge mm² | deepest µm | stepover p50 mm | min seg mm | seg p01 mm | segs <10µm |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
        let mut baseline: Option<Vec<f64>> = None;
        for arm in arms() {
            let run = run_arm(&fixture, arm.policy, resolution);
            let r = &run.report;
            println!(
                "| {} | {} | {:.2} | {} | {:.2} | {:.1} | {:.2}× | {:.1}% | {:.2} | {:.2} | {:.3} | {:.1} | {:.4} | {:.4} | {:.4} | {} ({:.1}%) |",
                arm.id,
                arm.what,
                run.seconds,
                run.rings,
                run.uncut_core_mm2,
                r.achieved_cusp_normal_um,
                r.cusp_ratio_normal(),
                r.on_dial_frac * 100.0,
                r.untouched_mm2,
                r.standing_mm2,
                r.gouge_mm2,
                r.deepest_gouge_um,
                run.structure.stepover_p50_mm,
                run.structure.min_segment_mm,
                run.structure.seg_p01_mm,
                run.structure.segs_under_10um,
                run.structure.segs_under_10um_frac * 100.0,
            );

            // The v3 rule: render before reading a verdict.
            let tag = format!("{}_{}", fixture.name.replace([' ', '-'], "_"), arm.id);
            render_field(&run.grid, &run.residuals, 100.0, &format!("resid_{tag}"));
            match baseline {
                None => baseline = Some(run.residuals.clone()),
                Some(ref base) => {
                    let diff: Vec<f64> = base
                        .iter()
                        .zip(&run.residuals)
                        .map(|(a, b)| {
                            if a.is_nan() || b.is_nan() || !a.is_finite() || !b.is_finite() {
                                f64::NAN
                            } else {
                                b - a
                            }
                        })
                        .collect();
                    render_field(&run.grid, &diff, 50.0, &format!("diff_{tag}_minus_A0"));
                }
            }
        }

        // Per-slope-band detail for the two headline arms.
        for arm in arms()
            .into_iter()
            .filter(|a| a.id == "A0" || a.id == "A7" || a.id == "A9")
        {
            let run = run_arm(&fixture, arm.policy, resolution);
            println!("\n**{} band detail** — {}\n", arm.id, arm.what);
            println!("| band | area mm² | cusp p50 µm | cusp p99 µm | max µm | over dial |");
            println!("|---|---|---|---|---|---|");
            for (band, sc) in &run.report.bands {
                println!(
                    "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1}% |",
                    band.label(),
                    sc.area_mm2,
                    sc.p50_um,
                    sc.p99_um,
                    sc.max_um,
                    sc.over_dial_frac * 100.0
                );
            }
        }
    }
    println!(
        "\nDiff maps and residual maps: `{}`",
        common::scallop_oracle::out_dir().display()
    );
}

// ---------------------------------------------------------------------------
// Evidence run 2 — how much does min-across-ring alone explain?
// ---------------------------------------------------------------------------

#[test]
#[ignore = "M4 phase B evidence — the min-across-ring decomposition"]
fn min_across_ring_decomposition() {
    let tool = cutter();
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);
    println!("\n# How much of the defect is min-across-ring?\n");
    println!(
        "`selected / sample p50` is the ratio by which the reducer slows the \
         cascade below what the typical point on the ring would allow. 1.00 \
         means the reduction costs nothing.\n"
    );
    println!(
        "| fixture | arm | iterations | selected p50 mm | sample-min p50 mm | sample-p50 p50 mm | collapse ratio | rings | uncut mm² |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");

    for fixture in fixtures() {
        for arm in arms().into_iter().filter(|a| {
            matches!(a.policy.ring_source, RingSource::OffsetCascade)
                && (a.id == "A0" || a.id == "A2" || a.id == "A3" || a.id == "A4" || a.id == "A7")
        }) {
            let run = run_arm(&fixture, arm.policy, resolution);
            let t = &run.trace;
            let med = |v: &mut Vec<f64>| {
                if v.is_empty() {
                    return f64::NAN;
                }
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            };
            let mut selected = t.selected_mm.clone();
            let mut smin: Vec<f64> = t.sample_spread.iter().map(|s| s.0).collect();
            let mut smid: Vec<f64> = t.sample_spread.iter().map(|s| s.1).collect();
            let (sel, lo, mid) = (med(&mut selected), med(&mut smin), med(&mut smid));
            println!(
                "| {} | {} | {} | {:.4} | {:.4} | {:.4} | {:.2}× | {} | {:.2} |",
                fixture.name,
                arm.id,
                t.selected_mm.len(),
                sel,
                lo,
                mid,
                if sel > 0.0 { mid / sel } else { f64::NAN },
                run.cascade_rings,
                run.uncut_core_mm2,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Evidence run 3 — does Checkpoint B's cusp/4 rejection survive the fix?
// ---------------------------------------------------------------------------

/// Checkpoint B rejected moving scallop to a `cusp/4` generation grid because
/// at that resolution the cascade left 19–33 mm² standing that `envelope/4`
/// did not — `max_rings` truncating a cascade the finer grid had slowed down.
/// Its addendum said the fix belonged in `ring_stepover`. This is that test.
#[test]
#[ignore = "M4 phase B evidence — the cusp/4 re-test under the culprit fix"]
fn cusp_quarter_rejection_under_the_culprit_fix() {
    let tool = cutter();
    let legacy = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);
    let fine = FinishResolutionPolicy::cusp_quarter(&tool, TOLERANCE_MM);
    println!(
        "\n# cusp/4 re-test — legacy cell {:.3} mm vs cusp/4 cell {:.3} mm\n",
        legacy.cell_mm(),
        fine.cell_mm()
    );
    println!(
        "| fixture | arm | grid | gen s | rings | cascade rings | uncut core mm² | oracle untouched mm² | standing mm² | cusp µm | ×dial |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");

    for fixture in fixtures() {
        for arm in arms()
            .into_iter()
            .filter(|a| a.id == "A0" || a.id == "A7" || a.id == "A9")
        {
            for (label, res) in [("envelope/4", legacy), ("cusp/4", fine)] {
                let run = run_arm(&fixture, arm.policy, res);
                let r = &run.report;
                println!(
                    "| {} | {} | {} | {:.2} | {} | {} | {:.2} | {:.2} | {:.2} | {:.1} | {:.2}× |",
                    fixture.name,
                    arm.id,
                    label,
                    run.seconds,
                    run.rings,
                    run.cascade_rings,
                    run.uncut_core_mm2,
                    r.untouched_mm2,
                    r.standing_mm2,
                    r.achieved_cusp_normal_um,
                    r.cusp_ratio_normal(),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Evidence run 4 — the real terrain
// ---------------------------------------------------------------------------

#[test]
#[ignore = "M4 phase B evidence — the committed terrain fixture, headline arms only"]
fn terrain_headline_arms() {
    let path = common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    let Ok(full) = TriangleMesh::from_stl(&path) else {
        eprintln!("terrain fixture not readable at {}", path.display());
        return;
    };
    let window: f64 = std::env::var("M4_TERRAIN_WINDOW_MM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    let mesh = crop(&full, window);
    println!(
        "\n# terrain.stl, {window} mm window — {} triangles\n",
        mesh.triangles.len()
    );
    let fixture = Fixture::new("terrain", mesh);
    let tool = cutter();
    let resolution = FinishResolutionPolicy::legacy_envelope_quarter(&tool, TOLERANCE_MM);

    println!(
        "| arm | what | gen s | rings | uncut core mm² | cusp µm (normal) | ×dial | on dial | untouched mm² | standing mm² | gouge mm² | deepest µm |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut baseline: Option<Vec<f64>> = None;
    for arm in arms() {
        let run = run_arm(&fixture, arm.policy, resolution);
        let r = &run.report;
        println!(
            "| {} | {} | {:.2} | {} | {:.2} | {:.1} | {:.2}× | {:.1}% | {:.2} | {:.2} | {:.3} | {:.1} |",
            arm.id,
            arm.what,
            run.seconds,
            run.rings,
            run.uncut_core_mm2,
            r.achieved_cusp_normal_um,
            r.cusp_ratio_normal(),
            r.on_dial_frac * 100.0,
            r.untouched_mm2,
            r.standing_mm2,
            r.gouge_mm2,
            r.deepest_gouge_um,
        );
        render_field(
            &run.grid,
            &run.residuals,
            100.0,
            &format!("terrain_resid_{}", arm.id),
        );
        match baseline {
            None => baseline = Some(run.residuals.clone()),
            Some(ref base) => {
                let diff: Vec<f64> = base
                    .iter()
                    .zip(&run.residuals)
                    .map(|(a, b)| {
                        if a.is_nan() || b.is_nan() || !a.is_finite() || !b.is_finite() {
                            f64::NAN
                        } else {
                            b - a
                        }
                    })
                    .collect();
                render_field(
                    &run.grid,
                    &diff,
                    50.0,
                    &format!("terrain_diff_{}_minus_A0", arm.id),
                );
            }
        }
    }

    // What is the best this tool COULD do here? A Ø1 tip cannot enter relief
    // narrower than 0.5 mm, and terrain has plenty; without this floor the
    // table below ranks candidates on a residual none of them controls.
    {
        let grid = OracleGrid::for_mesh(&fixture.mesh, &tool, ORACLE_CELL_MM);
        let truth = EnvelopeOracle::true_surface_from_mesh(grid, &fixture.mesh, &fixture.index);
        let kernel = StampKernel::new(
            &tool,
            ORACLE_CELL_MM,
            Some(tool.cusp_radius_mm() * STAMP_CAP_TIP_MULTIPLE),
        );
        let floor = EnvelopeOracle::tool_reach_floor(
            grid,
            &truth,
            &fixture.mesh,
            &fixture.index,
            &tool,
            &kernel,
        );
        let mut v: Vec<f64> = floor.iter().copied().filter(|x| !x.is_nan()).collect();
        v.sort_by(f64::total_cmp);
        println!(
            "\n**Tool-reach floor** — the residual an infinitely dense path with this              cutter would still leave: p50 {:.1} µm, p90 {:.1} µm, p99 {:.1} µm, max {:.1} µm              over {} cells.\n",
            common::scallop_oracle::quantile(&v, 0.50) * 1000.0,
            common::scallop_oracle::quantile(&v, 0.90) * 1000.0,
            common::scallop_oracle::quantile(&v, 0.99) * 1000.0,
            v.last().copied().unwrap_or(f64::NAN) * 1000.0,
            v.len()
        );
        render_field(&grid, &floor, 100.0, "terrain_tool_reach_floor");
    }

    // Is ring PLACEMENT even the binding constraint here? Every arm lands
    // within 4 µm of every other, which on a fixture this coarse points
    // downstream — at the ring DECIMATION (0.75 × cell) and the chord
    // refinement probe (0.5 × cell), both of which scale with the generation
    // grid. Re-running the two extreme arms at cusp/4 discriminates: if the
    // cusp collapses when only the cell changes, placement was not the
    // problem on this fixture.
    println!("\n**Is it placement, or is it the grid?**\n");
    println!(
        "| arm | grid | gen s | rings | cusp µm (normal) | ×dial | on dial | standing mm² | gouge mm² |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    let fine = FinishResolutionPolicy::cusp_quarter(&tool, TOLERANCE_MM);
    for arm in arms().into_iter().filter(|a| a.id == "A0" || a.id == "A9") {
        for (label, res) in [("envelope/4", resolution), ("cusp/4", fine)] {
            let run = run_arm(&fixture, arm.policy, res);
            let r = &run.report;
            println!(
                "| {} | {} | {:.2} | {} | {:.1} | {:.2}× | {:.1}% | {:.2} | {:.3} |",
                arm.id,
                label,
                run.seconds,
                run.rings,
                r.achieved_cusp_normal_um,
                r.cusp_ratio_normal(),
                r.on_dial_frac * 100.0,
                r.standing_mm2,
                r.gouge_mm2,
            );
            if label == "cusp/4" {
                render_field(
                    &run.grid,
                    &run.residuals,
                    100.0,
                    &format!("terrain_cusp4_resid_{}", arm.id),
                );
            }
        }
    }

    println!("\n**A0 vs A9 band detail**\n");
    for arm in arms().into_iter().filter(|a| a.id == "A0" || a.id == "A9") {
        let run = run_arm(&fixture, arm.policy, resolution);
        println!("\n{} — {}\n", arm.id, arm.what);
        println!("| band | area mm² | cusp p50 µm | cusp p99 µm | max µm | over dial |");
        println!("|---|---|---|---|---|---|");
        for (band, sc) in &run.report.bands {
            println!(
                "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1}% |",
                band.label(),
                sc.area_mm2,
                sc.p50_um,
                sc.p99_um,
                sc.max_um,
                sc.over_dial_frac * 100.0
            );
        }
    }
    let _ = SlopeBand::ALL;
}

/// Keep only triangles with all three vertices inside a centred window — the
/// same crop `classification_columns_ab_m3.rs` uses, so no partial facets and
/// the two evidence packs are looking at the same ground.
fn crop(mesh: &TriangleMesh, window_mm: f64) -> TriangleMesh {
    if window_mm <= 0.0 {
        return mesh.clone();
    }
    let cx = 0.5 * (mesh.bbox.min.x + mesh.bbox.max.x);
    let cy = 0.5 * (mesh.bbox.min.y + mesh.bbox.max.y);
    let half = 0.5 * window_mm;
    let inside = |p: &rs_cam_core::geo::P3| (p.x - cx).abs() <= half && (p.y - cy).abs() <= half;

    let mut remap: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut vertices: Vec<rs_cam_core::geo::P3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for tri in &mesh.triangles {
        if !tri.iter().all(|&i| inside(&mesh.vertices[i as usize])) {
            continue;
        }
        let mut out = [0u32; 3];
        for (slot, &src) in out.iter_mut().zip(tri.iter()) {
            *slot = *remap.entry(src).or_insert_with(|| {
                vertices.push(mesh.vertices[src as usize]);
                (vertices.len() - 1) as u32
            });
        }
        triangles.push(out);
    }
    assert!(
        triangles.len() > 500,
        "the {window_mm} mm crop kept only {} triangle(s)",
        triangles.len()
    );
    TriangleMesh::from_raw(vertices, triangles)
}
