//! M3 classification-sampler harness — the equivalence instrument of wave 7a,
//! and since wave 7b the **parity gate on the production classifier**.
//!
//! `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` M3 is an
//! optimisation whose acceptance gate is not speed but sameness — the
//! classifier decides region ownership, so a faster classifier that moves a
//! boundary has not optimised anything, it has changed the product. This file
//! is the instrument that decided which candidate was allowed to be
//! considered; `CLASSIFICATION_PERF_STUDY.md` is its write-up.
//!
//! # What changed at wave 7b
//!
//! Production classification now runs
//! [`ClassificationSampler::PRODUCTION`] (tile raster). Three consequences
//! for this file:
//!
//! 1. The direct arms' divergence numbers stopped being characterisation and
//!    became **tripwires** — [`LABEL_MOVE_TRIPWIRES`], one ceiling per fixture
//!    from the wave-7a table.
//! 2. The plan's acceptance gate "no loss of narrow steep regions relative to
//!    the current fine classifier" is restated as
//!    `production_loses_no_region_against_the_true_surface`. The reference is
//!    the SURFACE — the shipped classifier's own algorithm at a 100×-smaller
//!    probe — because §5.3 measured the shipped classifier's extra regions to
//!    be its probe artefact, and holding a true-surface classifier to a
//!    probe-artefact reference would reject it for being right.
//! 3. A block of gates now drives
//!    `finish_setup::build_classification_surface_*` itself, not just the
//!    samplers, so a rewiring of the builder cannot pass by keeping the arms
//!    honest.
//!
//! # What is compared, and why not area
//!
//! Per `CHECKPOINT_B_EVIDENCE.md` §A.0, **area is not a safe invariant**: on
//! all four Checkpoint-B fixtures the broken coarse grid reported 34–93% MORE
//! non-shallow area than the truth while simultaneously collapsing the
//! topology to nothing, so an "area must not shrink" gate passes a classifier
//! that has stopped working. What this harness reports instead:
//!
//! - **coverage mask**, cell for cell (exact — it is the same predicate);
//! - **Z**, as max/RMS deviation, *bounded* rather than exact across the
//!   probe-CL / true-surface boundary (see below);
//! - **label grid**: per-class cell counts and per-cell disagreement;
//! - **topology**: 4-connected components per class, read **twice** — raw,
//!   and with a minimum component size. The raw count is scale-sensitive for
//!   band-shaped classes (P7 / Checkpoint B §8.3: a 45° annulus fragments
//!   into 39 components at fine resolution), so a bare component-count
//!   equality would be noise. The minimum-size reading is the stable one.
//! - **narrow-steep survival**: every oracle component above the minimum size
//!   must still have at least one cell of its own class in the candidate.
//!   This is the M3 acceptance gate "no loss of narrow steep regions", and it
//!   is the one that cannot be satisfied by a compensating error elsewhere.
//!
//! # Why bit-equality is the wrong bar for two of the arms
//!
//! The shipped classifier samples the **CL surface of a Ø0.05 mm ball**, not
//! the model surface: on a facet of normal `n` the CL sits
//! `R·(1 − n.z)/n.z` above the surface — 10 µm at 45°, 119 µm at 80°, 1.4 mm
//! at 89°. The direct candidates evaluate the model surface itself, so they
//! *cannot* be bit-identical and demanding it would reject them for being
//! more accurate. They are scored on bounded Z deviation and on the label
//! grid, which is what the planner actually consumes.
//!
//! ```text
//! cargo test -p rs_cam_core --test classification_strategy_m3            # analytic, seconds
//! cargo test -p rs_cam_core --test classification_strategy_m3 -- --ignored --nocapture
//! ```
//!
//! The ignored rows carry the terrain fixture and the 143²/425²/849² timing
//! table; run them `--release`, with `--test-threads=1`, and with the machine
//! quiet. A `TIMING_LOCK` enforces the serialisation part even if the flag is
//! forgotten — see `cpu_time`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rs_cam_core::classify_probe::{
    ClassificationGridSpec, ClassificationSampler, TILE_EDGE_CELLS, sample_classification_grid,
};
use rs_cam_core::finish_setup::FinishResolutionPolicy;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::slope::{SlopeMap, SurfaceHeightmap};
use rs_cam_core::tool::MillingCutter;

use common::meshes;

/// Slope-band thresholds, matching `checkpoint_b_resolution_ab.rs` so the two
/// studies' label grids mean the same thing.
const STEEP_DEG: f64 = 45.0;
const VERY_STEEP_DEG: f64 = 75.0;

/// Components smaller than this are below the resolution at which component
/// COUNT is a stable statistic (P7 / Checkpoint B §8.3). Regions at or above
/// it are the ones the survival gate is allowed to depend on.
const MIN_COMPONENT_CELLS: usize = 8;

/// Tolerance pinned well below every arm's cell so the `.max(tolerance)` floor
/// never sets the grid.
const TOLERANCE_MM: f64 = 0.02;

fn never() -> impl Fn() -> bool + Send + Sync {
    || false
}

// ── Fixtures ────────────────────────────────────────────────────────────

struct Fixture {
    name: &'static str,
    /// What about the mesh this fixture is here to stress.
    edge_case: &'static str,
    mesh: TriangleMesh,
}

/// A mixed-slope relief: flats, a 45°-ish flank and a near-vertical wall, all
/// in one grid, so a single fixture exercises every band.
fn mixed_slope_relief(half: f64, step: f64) -> TriangleMesh {
    let z = |x: f64, y: f64| -> f64 {
        let ridge = 3.0 * (-(x * x) / 8.0).exp();
        let terrace = if y > 2.0 { 1.5 } else { 0.0 };
        let ramp = 0.6 * y;
        ridge + terrace + ramp
    };
    meshes::height_field(half, step, z)
}

fn analytic_fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "plateau",
            edge_case: "vertical walls; only the top face is ever sampled",
            mesh: meshes::plateau(16.0, 6.0),
        },
        Fixture {
            name: "grooved-block",
            edge_case: "two facing walls at a known angle, sharp rims",
            mesh: meshes::grooved_block(3.0, 60.0, 4.0),
        },
        Fixture {
            name: "mixed-slope",
            edge_case: "flats, mid slope, near-vertical, and a one-cell terrace step",
            mesh: mixed_slope_relief(8.0, 0.25),
        },
        Fixture {
            name: "plate-with-hole",
            edge_case: "holes / uncovered cells in an open mesh",
            mesh: meshes::plate_with_hole(20.0, 8.0, 3.0),
        },
        Fixture {
            name: "stacked-shelf",
            edge_case: "stacked triangles; the topmost face is wound DOWN",
            mesh: meshes::stacked_shelf(16.0, 8.0, 4.0),
        },
        Fixture {
            name: "non-manifold-fin",
            edge_case: "three faces on one edge, a free boundary, a detached flyer",
            mesh: meshes::non_manifold_fin(16.0, 4.0),
        },
    ]
}

/// Per-fixture label-movement measured by wave 7a
/// (`CLASSIFICATION_PERF_STUDY.md` §5.2), and the ceiling wave 7b pins it
/// under.
///
/// The study characterised the direct arms; wave 7b made one of them
/// production, so those characterisation numbers become **tripwires**. A
/// change that moves MORE cells across a band boundary than the study
/// measured is a change to region ownership that nobody decided, and it must
/// fail here rather than surface as a different toolpath.
///
/// The three fixtures with no slope-varying geometry the probe can bias
/// (`plateau`, `plate-with-hole`, `stacked-shelf`) measured **exactly zero**
/// and are pinned at zero — no headroom, because a single moved label there
/// means the direct arm has started disagreeing about flat ground. The rest
/// carry 25% headroom over the study value: enough that an ulp-level
/// arithmetic change on a rim cell does not fail the build, far too little to
/// hide a real redistribution.
const LABEL_MOVE_TRIPWIRES: &[(&str, f64, f64)] = &[
    // (fixture, wave-7a label Δ %, ceiling %)
    ("plateau", 0.000, 0.000),
    ("plate-with-hole", 0.000, 0.000),
    ("stacked-shelf", 0.000, 0.000),
    ("grooved-block", 0.434, 0.543),
    ("non-manifold-fin", 0.166, 0.208),
    ("mixed-slope", 4.347, 5.434),
    // Terrain rows are the `#[ignore]`d study rows; same rule.
    ("terrain@143", 3.467, 4.334),
    ("terrain@425", 1.757, 2.196),
    ("terrain@849", 2.454, 3.068),
];

/// Assert every direct-arm row in `rows` against [`LABEL_MOVE_TRIPWIRES`].
///
/// Fails loudly on an unknown fixture name rather than passing it: a
/// characterisation with no ceiling is a number nobody is holding.
fn assert_label_move_tripwires(rows: &[Equivalence]) {
    for row in rows.iter().filter(|r| !r.arm.samples_probe_cl_surface()) {
        let (_, study, ceiling) = LABEL_MOVE_TRIPWIRES
            .iter()
            .find(|(name, _, _)| *name == row.fixture)
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "fixture '{}' has no label-movement tripwire; add its wave-7a number to \
                     LABEL_MOVE_TRIPWIRES rather than leaving it unbounded",
                    row.fixture
                )
            });
        let share = 100.0 * row.label_disagree as f64 / row.cells as f64;
        assert!(
            share <= ceiling,
            "{} moves {:.3}% of {} cells across a band boundary; wave 7a measured {study:.3}% \
             and the ceiling is {ceiling:.3}%. This is a change in REGION OWNERSHIP — which \
             operation cuts which territory — not a tolerance drift.",
            row.arm.label(),
            share,
            row.fixture,
        );
    }
}

fn terrain_path() -> Option<PathBuf> {
    [
        common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl"),
        common::repo_root().join("fixtures/terrain_small.stl"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

// ── Label grid ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Uncovered,
    Shallow,
    MidSteep,
    VerySteep,
}

impl Class {
    const BANDS: [Self; 3] = [Self::Shallow, Self::MidSteep, Self::VerySteep];
    fn label(self) -> &'static str {
        match self {
            Self::Uncovered => "uncovered",
            Self::Shallow => "shallow",
            Self::MidSteep => "mid-steep",
            Self::VerySteep => "very-steep",
        }
    }
}

/// The label grid a candidate would hand the planner: the classification
/// stencil (max-of-one-sided gradients, exactly as `finish_setup` selects it)
/// banded at the shipped thresholds, with coverage carried through.
fn label_grid(hm: &SurfaceHeightmap) -> Vec<Class> {
    let slope: SlopeMap = hm.slope_map_max_gradient();
    (0..hm.rows * hm.cols)
        .map(|i| {
            if !hm.covered_flags()[i] {
                return Class::Uncovered;
            }
            let deg = slope.angles[i].to_degrees();
            if deg < STEEP_DEG {
                Class::Shallow
            } else if deg < VERY_STEEP_DEG {
                Class::MidSteep
            } else {
                Class::VerySteep
            }
        })
        .collect()
}

/// 4-connected components of `class`, as `(cell indices)` per component.
fn components(labels: &[Class], rows: usize, cols: usize, class: Class) -> Vec<Vec<usize>> {
    let mut seen = vec![false; labels.len()];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..labels.len() {
        if labels[start] != class || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut group = Vec::new();
        while let Some(idx) = stack.pop() {
            group.push(idx);
            let (r, c) = (idx / cols, idx % cols);
            let push = |n: usize, stack: &mut Vec<usize>, seen: &mut Vec<bool>| {
                if labels[n] == class && !seen[n] {
                    seen[n] = true;
                    stack.push(n);
                }
            };
            if r > 0 {
                push(idx - cols, &mut stack, &mut seen);
            }
            if r + 1 < rows {
                push(idx + cols, &mut stack, &mut seen);
            }
            if c > 0 {
                push(idx - 1, &mut stack, &mut seen);
            }
            if c + 1 < cols {
                push(idx + 1, &mut stack, &mut seen);
            }
        }
        out.push(group);
    }
    out
}

#[derive(Debug, Clone)]
struct Equivalence {
    arm: ClassificationSampler,
    fixture: String,
    cells: usize,
    covered_mismatch: usize,
    z_max_abs: f64,
    z_rms: f64,
    z_exact: bool,
    /// Per-class (oracle count, candidate count) over the same grid.
    class_counts: [(usize, usize); 3],
    label_disagree: usize,
    /// Per class: (oracle raw, candidate raw, oracle ≥ min, candidate ≥ min).
    topology: [(usize, usize, usize, usize); 3],
    /// Per class: oracle components ≥ min size with NO candidate cell of the
    /// same class anywhere inside them.
    vanished: [usize; 3],
}

impl Equivalence {
    fn verdict(&self) -> &'static str {
        if self.covered_mismatch == 0 && self.z_exact && self.label_disagree == 0 {
            "EXACT"
        } else if self.vanished.iter().sum::<usize>() == 0 && self.covered_mismatch == 0 {
            "BOUNDED"
        } else {
            "FAILED"
        }
    }
}

fn compare(
    arm: ClassificationSampler,
    fixture: &str,
    oracle: &SurfaceHeightmap,
    candidate: &SurfaceHeightmap,
) -> Equivalence {
    let (rows, cols) = (oracle.rows, oracle.cols);
    let cells = rows * cols;
    assert_eq!(candidate.rows, rows, "{arm:?} changed the grid shape");
    assert_eq!(candidate.cols, cols, "{arm:?} changed the grid shape");

    let covered_mismatch = (0..cells)
        .filter(|&i| oracle.covered_flags()[i] != candidate.covered_flags()[i])
        .count();

    let (mut z_max_abs, mut sq, mut n) = (0.0f64, 0.0f64, 0usize);
    let mut z_exact = true;
    for i in 0..cells {
        let a = oracle.z_or_bbox_floor_values()[i];
        let b = candidate.z_or_bbox_floor_values()[i];
        if a.to_bits() != b.to_bits() {
            z_exact = false;
        }
        let d = b - a;
        z_max_abs = z_max_abs.max(d.abs());
        sq += d * d;
        n += 1;
    }
    let z_rms = (sq / n.max(1) as f64).sqrt();

    let la = label_grid(oracle);
    let lb = label_grid(candidate);
    let label_disagree = (0..cells).filter(|&i| la[i] != lb[i]).count();

    let mut class_counts = [(0usize, 0usize); 3];
    let mut topology = [(0usize, 0usize, 0usize, 0usize); 3];
    let mut vanished = [0usize; 3];
    for (bi, class) in Class::BANDS.into_iter().enumerate() {
        class_counts[bi] = (
            la.iter().filter(|&&c| c == class).count(),
            lb.iter().filter(|&&c| c == class).count(),
        );
        let ca = components(&la, rows, cols, class);
        let cb = components(&lb, rows, cols, class);
        let big_a: Vec<&Vec<usize>> = ca
            .iter()
            .filter(|g| g.len() >= MIN_COMPONENT_CELLS)
            .collect();
        topology[bi] = (
            ca.len(),
            cb.len(),
            big_a.len(),
            cb.iter().filter(|g| g.len() >= MIN_COMPONENT_CELLS).count(),
        );
        vanished[bi] = big_a
            .iter()
            .filter(|g| !g.iter().any(|&i| lb[i] == class))
            .count();
    }

    Equivalence {
        arm,
        fixture: fixture.to_owned(),
        cells,
        covered_mismatch,
        z_max_abs,
        z_rms,
        z_exact,
        class_counts,
        label_disagree,
        topology,
        vanished,
    }
}

fn print_equivalence_rows(rows: &[Equivalence]) {
    println!(
        "\n| fixture | arm | verdict | cov Δ | z max Δ mm | z RMS mm | label Δ | \
         cells shallow / mid / very (oracle→arm) | \
         mid regions (raw / ≥{MIN_COMPONENT_CELLS}) | very regions (raw / ≥{MIN_COMPONENT_CELLS}) | lost |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for r in rows {
        println!(
            "| {} | {} | {} | {} | {:.6} | {:.6} | {} ({:.3}%) | {}→{} / {}→{} / {}→{} | {}→{} / {}→{} | {}→{} / {}→{} | {} |",
            r.fixture,
            r.arm.label(),
            r.verdict(),
            r.covered_mismatch,
            r.z_max_abs,
            r.z_rms,
            r.label_disagree,
            100.0 * r.label_disagree as f64 / r.cells as f64,
            r.class_counts[0].0,
            r.class_counts[0].1,
            r.class_counts[1].0,
            r.class_counts[1].1,
            r.class_counts[2].0,
            r.class_counts[2].1,
            r.topology[1].0,
            r.topology[1].1,
            r.topology[1].2,
            r.topology[1].3,
            r.topology[2].0,
            r.topology[2].1,
            r.topology[2].2,
            r.topology[2].3,
            r.vanished.iter().sum::<usize>(),
        );
    }
}

// ── Grid construction ───────────────────────────────────────────────────

/// Production's classification grid for this mesh + tool: `cusp/4`, padded by
/// one envelope radius.
fn production_spec(mesh: &TriangleMesh, cutter: &dyn MillingCutter) -> ClassificationGridSpec {
    let policy = FinishResolutionPolicy::cusp_quarter(cutter, TOLERANCE_MM);
    ClassificationGridSpec::for_mesh(mesh, cutter, policy.cell_mm())
}

/// The same grid, resized so each axis has exactly `side` cells — the timing
/// table's independent variable.
fn spec_at_side(
    mesh: &TriangleMesh,
    cutter: &dyn MillingCutter,
    side: usize,
) -> ClassificationGridSpec {
    let base = production_spec(mesh, cutter);
    let span_x = (base.cols - 1) as f64 * base.cell_size;
    let span_y = (base.rows - 1) as f64 * base.cell_size;
    ClassificationGridSpec {
        rows: side,
        cols: side,
        cell_size: span_x.max(span_y) / (side - 1) as f64,
        ..base
    }
}

fn run(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    arm: ClassificationSampler,
) -> SurfaceHeightmap {
    let cancel = never();
    sample_classification_grid(mesh, index, spec, arm, &cancel).expect("never cancelled")
}

// ── Production-path gates (wave 7b) ─────────────────────────────────────
//
// Everything above scores SAMPLERS against each other. The block below scores
// the thing production actually calls —
// `finish_setup::build_classification_surface_with_policy_and_cancel` — so a
// future change that keeps every arm honest while quietly rewiring the
// builder still fails.

/// The probe diameter at which the shipped drop-cutter classifier IS the model
/// surface, to well under a nanometre of Z.
///
/// §5.3: label disagreement against the direct arms hits zero at the first
/// 10× shrink on every fixture, and the maximum Z difference falls exactly
/// 10× per 10× of radius — the `R·(1 − n.z)/n.z` law. 100× smaller than the
/// shipped Ø0.05 leaves a worst-case offset of 14 µm at 89° and under 0.3 µm
/// anywhere below 80°.
const TRUE_SURFACE_PROBE_MM: f64 = 0.0005;

/// Sample the true-surface reference: the SHIPPED classifier's own algorithm,
/// run at a probe small enough that its artefact is gone.
///
/// Deliberately not "one of the direct arms": the M3 gate is *no loss of
/// narrow steep regions*, and using a direct arm as its own reference would
/// make the gate a tautology. Shrinking the oracle's probe gives an
/// independent construction of the same surface — different code path,
/// different arithmetic, same answer if and only if both are right.
fn true_surface_reference(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
) -> SurfaceHeightmap {
    let cancel = never();
    rs_cam_core::classify_probe::sample_with_probe_diameter(
        mesh,
        index,
        spec,
        TRUE_SURFACE_PROBE_MM,
        &cancel,
    )
    .expect("never cancelled")
}

#[test]
fn production_loses_no_region_against_the_true_surface() {
    // **The M3 acceptance gate, restated.**
    //
    // The plan reads "no loss of narrow steep regions relative to the current
    // fine classifier". Wave 7a measured that the direct arms DO lose regions
    // relative to the shipped classifier — 2 mid-steep on mixed-slope, 5 on
    // terrain at 849² — and then measured WHY: those regions are the probe's
    // own CL offset, not the surface (§5.3). Holding a true-surface classifier
    // to a probe-artefact reference would reject it for being right.
    //
    // So the reference is the surface, reached by shrinking the oracle's own
    // probe 100× (`true_surface_reference`), and the gate is the same
    // sentence against it: no component the true surface reports above the
    // noise floor may vanish. Non-vacuity is asserted at the bottom — the
    // SHIPPED classifier must fail this gate somewhere, or the reference is
    // not discriminating and the whole restatement is empty.
    let cutter = common::tools::wanaka_taper();
    let mut production_rows = Vec::new();
    let mut shipped_rows = Vec::new();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let truth = true_surface_reference(&fx.mesh, &index, spec);
        let production = run(&fx.mesh, &index, spec, ClassificationSampler::PRODUCTION);
        let shipped = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
        );
        production_rows.push(compare(
            ClassificationSampler::PRODUCTION,
            fx.name,
            &truth,
            &production,
        ));
        shipped_rows.push(compare(
            ClassificationSampler::DropCutterProbe,
            fx.name,
            &truth,
            &shipped,
        ));
    }

    println!("\n### vs the true surface (oracle at Ø{TRUE_SURFACE_PROBE_MM} mm)\n");
    print_equivalence_rows(&production_rows);
    print_equivalence_rows(&shipped_rows);

    for row in &production_rows {
        for (bi, class) in Class::BANDS.into_iter().enumerate() {
            assert_eq!(
                row.vanished[bi],
                0,
                "the PRODUCTION classifier loses {} {} region(s) that the true surface has, on {}",
                row.vanished[bi],
                class.label(),
                row.fixture
            );
        }
        assert_eq!(
            row.covered_mismatch, 0,
            "the PRODUCTION classifier moved the coverage mask on {}",
            row.fixture
        );
    }

    // Non-vacuity, and it is worth reading carefully, because the direction
    // is the opposite of the plan's phrasing. Against the true surface the
    // shipped classifier does not LOSE regions — it **fabricates** them: its
    // slope-dependent CL offset adds gradient of its own, so mixed-slope's 6
    // real mid-steep components become 8. That is why the plan's gate had to
    // be restated rather than merely re-pointed. What must be non-zero for
    // this test to mean anything is that the reference *discriminates* — that
    // the shipped classifier and the true surface disagree at all.
    let shipped_disagreement: usize = shipped_rows.iter().map(|r| r.label_disagree).sum();
    assert!(
        shipped_disagreement > 0,
        "the SHIPPED classifier agreed with the true-surface reference everywhere — the \
         reference does not discriminate, so 'production loses nothing' is a vacuous pass \
         rather than a gate"
    );
    let fabricated: usize = shipped_rows
        .iter()
        .flat_map(|r| r.topology.iter())
        .map(|&(_, _, truth_big, shipped_big)| shipped_big.saturating_sub(truth_big))
        .sum();
    assert!(
        fabricated > 0,
        "the shipped probe no longer fabricates any region against the true surface; \
         `CLASSIFICATION_PERF_STUDY.md` §5.3's attribution — and therefore the reason M3 \
         switched classifiers — would need re-deriving"
    );
    // And the production sampler must be the surface, not merely close to it.
    for row in &production_rows {
        assert_eq!(
            row.label_disagree, 0,
            "the production sampler disagrees with the true surface on {} cells of {} — \
             it is supposed to BE the surface",
            row.label_disagree, row.fixture
        );
    }
    println!(
        "non-vacuity: against the true surface the shipped Ø0.05 probe moves \
         {shipped_disagreement} label(s) and FABRICATES {fabricated} region(s) ≥{MIN_COMPONENT_CELLS} cells; \
         the production sampler moves 0 and loses 0"
    );
}

#[test]
fn the_production_builder_is_deterministic_across_thread_counts() {
    // The arm-level version of this lives in
    // `arms_are_deterministic_across_thread_counts`. This one runs the whole
    // production entry point — grid arithmetic, sampler dispatch, stencil —
    // because that is what a GUI worker on N threads and a headless run on 1
    // must agree about.
    use rs_cam_core::finish_setup::build_classification_surface_with_policy_and_cancel;
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2]; // mixed-slope: every band populated
    let index = SpatialIndex::build_auto(&fx.mesh);
    let policy = FinishResolutionPolicy::cusp_quarter(&cutter, TOLERANCE_MM);
    let cancel = never();
    let mut built = Vec::new();
    for threads in [1usize, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        built.push(pool.install(|| {
            build_classification_surface_with_policy_and_cancel(
                &fx.mesh, &index, &cutter, policy, &cancel,
            )
            .expect("never cancelled")
        }));
    }
    assert_eq!(
        built[0].heightmap.z_or_bbox_floor_values(),
        built[1].heightmap.z_or_bbox_floor_values(),
        "the production classification builder is not thread-count deterministic (Z)"
    );
    assert_eq!(
        built[0].heightmap.covered_flags(),
        built[1].heightmap.covered_flags(),
        "the production classification builder is not thread-count deterministic (coverage)"
    );
    assert_eq!(
        label_grid(&built[0].heightmap),
        label_grid(&built[1].heightmap),
        "the production classification builder is not thread-count deterministic (labels)"
    );
    // Non-vacuity: a grid with only one band would agree trivially.
    let labels = label_grid(&built[0].heightmap);
    for class in Class::BANDS {
        assert!(
            labels.contains(&class),
            "the determinism fixture has no {} cells",
            class.label()
        );
    }
}

#[test]
fn the_production_builder_cancels_and_discards_the_partial_grid() {
    use rs_cam_core::finish_setup::build_classification_surface_with_policy_and_cancel;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2];
    let index = SpatialIndex::build_auto(&fx.mesh);
    let policy = FinishResolutionPolicy::cusp_quarter(&cutter, TOLERANCE_MM);
    let polls = AtomicUsize::new(0);
    let cancel = || polls.fetch_add(1, Ordering::Relaxed) >= 1;
    let out = build_classification_surface_with_policy_and_cancel(
        &fx.mesh, &index, &cutter, policy, &cancel,
    );
    assert!(
        out.is_err(),
        "the production classification builder ignored a cancel raised mid-grid — \
         the compute worker's cancel flag cannot interrupt it"
    );
    // `Result` makes the discard structural: there is no half-built
    // `FinishSurface` to leak. Asserting the latency too keeps the window
    // claim honest at the production seam.
    assert!(
        polls.load(Ordering::Relaxed) <= 3,
        "the production builder kept polling after the cancel"
    );
}

#[test]
fn the_unified_finish_op_defaults_to_the_production_sampler() {
    // The op-level end of the wiring. `UnifiedFinishConfig` carries the
    // sampler so the COLUMNS A/B can drive both classifiers through the
    // production pipeline; the default must be production, an absent field
    // must LOAD as production, and a production value must not be written
    // into any project file.
    use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
    use rs_cam_core::unified_finish::UnifiedFinishParams;

    assert_eq!(
        UnifiedFinishParams::default().classification_sampler,
        ClassificationSampler::PRODUCTION
    );
    let cfg = UnifiedFinishConfig::default();
    assert_eq!(
        cfg.classification_sampler,
        ClassificationSampler::PRODUCTION
    );

    let written = toml::to_string(&cfg).expect("serialise a default UnifiedFinishConfig");
    assert!(
        !written.contains("classification_sampler"),
        "a default config wrote a classification_sampler key; every existing project file \
         would grow one on the next save:\n{written}"
    );

    // An absent field loads as production — this is what every project file
    // written before wave 7b looks like.
    let loaded: UnifiedFinishConfig =
        toml::from_str(&written).expect("round-trip a default UnifiedFinishConfig");
    assert_eq!(
        loaded.classification_sampler,
        ClassificationSampler::PRODUCTION
    );

    // A deliberately pinned non-production sampler DOES round-trip, so the
    // documented escape hatch is real rather than silently dropped.
    let pinned = UnifiedFinishConfig {
        classification_sampler: ClassificationSampler::DropCutterProbe,
        ..UnifiedFinishConfig::default()
    };
    let text = toml::to_string(&pinned).expect("serialise a pinned config");
    assert!(text.contains("classification_sampler"));
    let back: UnifiedFinishConfig = toml::from_str(&text).expect("round-trip a pinned config");
    assert_eq!(
        back.classification_sampler,
        ClassificationSampler::DropCutterProbe
    );
}

// ── Analytic-fixture gates (default, fast) ──────────────────────────────

#[test]
fn analytic_equivalence_matrix() {
    let cutter = common::tools::wanaka_taper();
    let mut rows = Vec::new();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let oracle = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
        );
        for arm in ClassificationSampler::ALL {
            if arm == ClassificationSampler::DropCutterProbe {
                continue;
            }
            let got = run(&fx.mesh, &index, spec, arm);
            rows.push(compare(arm, fx.name, &oracle, &got));
        }
    }
    print_equivalence_rows(&rows);

    // The answer-preserving arm must be EXACT on every fixture — that is a
    // gate, and it is the only equivalence gate this matrix can enforce.
    for row in rows
        .iter()
        .filter(|r| r.arm == ClassificationSampler::DropCutterProbeScratch)
    {
        assert_eq!(
            row.verdict(),
            "EXACT",
            "the answer-preserving arm diverged on {}",
            row.fixture
        );
    }

    // The direct arms sample the model surface where the shipped classifier
    // samples the probe's CL surface, so they are expected to differ (see
    // `direct_arm_divergence_is_the_probe_offset` for the proof that the
    // difference IS the probe). Since wave 7b one of them is production, so
    // the divergence is no longer merely characterised — it is pinned:
    //
    // - the coverage mask never moves (that predicate is shared);
    // - per-cell label disagreement stays under its per-fixture tripwire.
    for row in rows.iter().filter(|r| !r.arm.samples_probe_cl_surface()) {
        assert_eq!(
            row.covered_mismatch,
            0,
            "{} moved the coverage mask on {}",
            row.arm.label(),
            row.fixture
        );
    }
    assert_label_move_tripwires(&rows);

    // Non-vacuity: if the direct arms ever became bit-identical to the
    // shipped one, every "bounded divergence" number above would be
    // measuring a grid against itself.
    assert!(
        rows.iter()
            .any(|r| !r.arm.samples_probe_cl_surface() && r.label_disagree > 0),
        "no direct arm disagreed with the shipped classifier anywhere — \
         the divergence measurement has gone vacuous"
    );
}

#[test]
fn direct_arm_divergence_is_the_probe_offset() {
    // THE decisive experiment. The shipped classifier and the direct arms
    // disagree; there are exactly two candidate causes:
    //
    //   (a) the direct arms sample the surface wrongly, or
    //   (b) the oracle's Ø0.05 mm probe sits `R·(1 − n.z)/n.z` above the
    //       surface — a SLOPE-DEPENDENT offset, so it does not cancel in a
    //       gradient and biases the classification stencil.
    //
    // Shrinking the probe discriminates. Under (b) the oracle must converge
    // onto the direct arms as the diameter falls; under (a) it cannot.
    let cutter = common::tools::wanaka_taper();
    let cancel = never();
    println!("\n| fixture | probe Ø mm | label Δ vs vertical ray | z max Δ mm |");
    println!("|---|---|---|---|");
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let direct = run(&fx.mesh, &index, spec, ClassificationSampler::VerticalRay);
        let mut disagreements = Vec::new();
        for diameter in [0.05f64, 0.005, 0.0005] {
            let probed = rs_cam_core::classify_probe::sample_with_probe_diameter(
                &fx.mesh, &index, spec, diameter, &cancel,
            )
            .expect("never cancelled");
            let cmp = compare(
                ClassificationSampler::VerticalRay,
                fx.name,
                &probed,
                &direct,
            );
            println!(
                "| {} | {diameter} | {} | {:.6} |",
                fx.name, cmp.label_disagree, cmp.z_max_abs
            );
            disagreements.push(cmp.label_disagree);
        }
        // Monotone convergence, and the 100×-smaller probe must land within a
        // handful of cells of the direct arm.
        assert!(
            disagreements[2] <= disagreements[1] && disagreements[1] <= disagreements[0],
            "{}: shrinking the probe did not monotonically close the gap ({disagreements:?}) — \
             the divergence is NOT (only) the probe offset and the study's attribution is wrong",
            fx.name
        );
    }
}

#[test]
fn coverage_mask_is_exact_on_every_arm_and_fixture() {
    // Coverage is the one quantity that can be exact across the probe-CL /
    // true-surface boundary: it is the same `contains_point_xy` predicate over
    // the same triangles. If an arm moves it, the arm is wrong — not merely
    // different — because uncovered cells are what keeps a finish pass out of
    // holes and off the margin ring.
    let cutter = common::tools::wanaka_taper();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let oracle = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
        );
        for arm in ClassificationSampler::ALL {
            let got = run(&fx.mesh, &index, spec, arm);
            assert_eq!(
                oracle.covered_flags(),
                got.covered_flags(),
                "{} moved the coverage mask on {} ({})",
                arm.label(),
                fx.name,
                fx.edge_case
            );
        }
    }
}

#[test]
fn scratch_arm_is_bit_identical_everywhere() {
    // Candidate 0's whole claim is that it changes no geometry decision. It is
    // the only arm allowed to be held to bit-equality, and it must be.
    let cutter = common::tools::wanaka_taper();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let oracle = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
        );
        let scratch = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbeScratch,
        );
        assert_eq!(
            oracle.z_or_bbox_floor_values(),
            scratch.z_or_bbox_floor_values(),
            "the scratch-query arm moved a Z on {}",
            fx.name
        );
    }
}

#[test]
fn direct_arms_agree_to_the_last_ulp_and_on_every_label() {
    // Scatter (raster), gather (ray) and tiled scatter are three spellings of
    // one function, and a real lattice or bbox bug in any of them shows up
    // here as a cell that differs by a visible amount.
    //
    // They are NOT bit-identical, and the reason is the plan's
    // "deterministic ties" edge case rather than a defect. Each arm keeps the
    // first strictly-greater candidate, and the two visit triangles in
    // different orders (mesh order vs index-cell order). Where a cell centre
    // lands on an edge shared by two triangles — every profile breakpoint of
    // the grooved block, 386 cells of it — both triangles contain the point
    // and evaluate the same plane height to within one ulp, or to `+0.0` and
    // `−0.0`. Whichever is seen first wins. Each arm is internally
    // deterministic (asserted separately); they simply break an exact tie
    // differently, which no downstream consumer can observe.
    //
    // So the assertion is: equal as IEEE numbers (`−0.0 == 0.0`) or within a
    // few ulp, AND identical after banding. Anything coarser would let a real
    // bug through; anything finer would fail on arithmetic that is correct.
    let cutter = common::tools::wanaka_taper();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let reference = run(&fx.mesh, &index, spec, ClassificationSampler::VerticalRay);
        for arm in [
            ClassificationSampler::TriangleRaster,
            ClassificationSampler::TileRaster,
        ] {
            let got = run(&fx.mesh, &index, spec, arm);
            let a = reference.z_or_bbox_floor_values();
            let b = got.z_or_bbox_floor_values();
            // One picometre, absolute plus relative. Six orders below the
            // finest length anything in this codebase decides on, and well
            // clear of the ~1e-15 mm cancellation residue a plane evaluation
            // leaves when a cell centre sits exactly on a profile breakpoint
            // (the grooved block's rim: `−depth + (x − floor)·tan α` at the
            // rim is a difference of near-equal numbers).
            let material: Vec<usize> = (0..a.len())
                .filter(|&i| {
                    let (x, y) = (a[i], b[i]);
                    (x - y).abs() > 1e-12 * (1.0 + x.abs().max(y.abs()))
                })
                .collect();
            for &i in material.iter().take(6) {
                let (row, col) = (i / spec.cols, i % spec.cols);
                println!(
                    "{} vs vertical ray on {}: cell ({row},{col}) at ({:.6}, {:.6}) — ray {:e}, arm {:e}",
                    arm.label(),
                    fx.name,
                    spec.origin_x + col as f64 * spec.cell_size,
                    spec.origin_y + row as f64 * spec.cell_size,
                    a[i],
                    b[i],
                );
            }
            assert!(
                material.is_empty(),
                "{} disagrees materially with the vertical-ray arm on {} cells of {}",
                arm.label(),
                material.len(),
                fx.name
            );
            assert_eq!(
                label_grid(&reference),
                label_grid(&got),
                "{} produces different LABELS from the vertical-ray arm on {}",
                arm.label(),
                fx.name
            );
        }
    }
}

#[test]
fn narrow_steep_regions_survive_on_the_answer_preserving_arm() {
    // The M3 acceptance gate, stated directly and read the annulus-aware way
    // (P7): not "the same number of components" — that statistic is unstable
    // for band-shaped classes — but "no component the oracle found above the
    // noise floor has vanished".
    //
    // It applies to the arm that CLAIMS to preserve the answer. The direct
    // arms do not make that claim and measurably do not preserve it; the
    // characterisation below records what they do instead, so the fact is a
    // pinned number rather than a permanently red gate.
    let cutter = common::tools::wanaka_taper();
    for fx in analytic_fixtures() {
        let index = SpatialIndex::build_auto(&fx.mesh);
        let spec = production_spec(&fx.mesh, &cutter);
        let oracle = run(
            &fx.mesh,
            &index,
            spec,
            ClassificationSampler::DropCutterProbe,
        );
        let arm = ClassificationSampler::DropCutterProbeScratch;
        let cmp = compare(arm, fx.name, &oracle, &run(&fx.mesh, &index, spec, arm));
        for (bi, class) in Class::BANDS.into_iter().enumerate() {
            assert_eq!(
                cmp.vanished[bi],
                0,
                "{} lost {} {} region(s) on {} ({})",
                arm.label(),
                cmp.vanished[bi],
                class.label(),
                fx.name,
                fx.edge_case
            );
        }
    }
}

#[test]
fn direct_arms_redistribute_steep_territory_on_the_mixed_slope_fixture() {
    // Characterisation, red-first in wave 7a and pinned here: on the fixture
    // with a one-cell terrace step, the direct arms do NOT merely perturb the
    // labels — they drop whole mid-steep regions the shipped classifier
    // reports. The shipped grid over-reports mid-steep there because the
    // probe's CL offset grows with slope and so adds gradient of its own.
    //
    // The number is asserted so that a future change which makes the direct
    // arms MORE divergent is caught, and so that the study's "this is not a
    // free swap" claim has a committed measurement behind it.
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2];
    let index = SpatialIndex::build_auto(&fx.mesh);
    let spec = production_spec(&fx.mesh, &cutter);
    let oracle = run(
        &fx.mesh,
        &index,
        spec,
        ClassificationSampler::DropCutterProbe,
    );
    let arm = ClassificationSampler::VerticalRay;
    let cmp = compare(arm, fx.name, &oracle, &run(&fx.mesh, &index, spec, arm));
    assert!(
        cmp.vanished[1] > 0,
        "the mixed-slope divergence has disappeared; this characterisation is now vacuous"
    );
    assert!(
        cmp.vanished[1] <= 4,
        "the direct arm now loses {} mid-steep regions on mixed-slope; wave 7a measured 2",
        cmp.vanished[1]
    );
    assert_eq!(
        cmp.vanished[2], 0,
        "the direct arm has started losing VERY-steep regions, which wave 7a did not see"
    );
    println!(
        "mixed-slope: direct arm loses {} mid-steep region(s) ≥{} cells, \
         moves {} of {} cells across a band boundary ({:.2}%)",
        cmp.vanished[1],
        MIN_COMPONENT_CELLS,
        cmp.label_disagree,
        cmp.cells,
        100.0 * cmp.label_disagree as f64 / cmp.cells as f64,
    );
}

#[test]
fn arms_are_deterministic_across_thread_counts() {
    // Two runs, 1 thread and 8, diffed exactly. A `max`-reduce is
    // order-independent in exact arithmetic, but that is an argument, not a
    // measurement, and a tiled decomposition could still leak a boundary.
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2]; // mixed-slope: every band populated
    let index = SpatialIndex::build_auto(&fx.mesh);
    let spec = production_spec(&fx.mesh, &cutter);
    for arm in ClassificationSampler::ALL {
        let mut grids = Vec::new();
        for threads in [1usize, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("thread pool");
            grids.push(pool.install(|| run(&fx.mesh, &index, spec, arm)));
        }
        assert_eq!(
            grids[0].z_or_bbox_floor_values(),
            grids[1].z_or_bbox_floor_values(),
            "{} is not thread-count deterministic (Z)",
            arm.label()
        );
        assert_eq!(
            grids[0].covered_flags(),
            grids[1].covered_flags(),
            "{} is not thread-count deterministic (coverage)",
            arm.label()
        );
        assert_eq!(
            label_grid(&grids[0]),
            label_grid(&grids[1]),
            "{} is not thread-count deterministic (labels)",
            arm.label()
        );
    }
}

#[test]
fn cancellation_is_bounded_by_one_chunk() {
    // A cancel raised part-way must be observed within one batch/tile of work
    // and must discard the partial result rather than returning a half grid.
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2];
    let index = SpatialIndex::build_auto(&fx.mesh);
    let spec = production_spec(&fx.mesh, &cutter);
    // The tiled arm's cancellation window is ONE TILE BAND, and the fixture
    // has to be big enough in BANDS for "cancel on the second poll" to be
    // mid-grid — otherwise the cancel lands on the last band and the test
    // proves nothing. Wave 7b's first attempt cancelled on the third poll of
    // a 3-band grid and this guard is what caught it.
    let bands = spec.rows.div_ceil(TILE_EDGE_CELLS);
    assert!(
        bands >= 3,
        "the cancellation fixture is only {bands} tile band(s) — a cancel on the second poll \
         would be at or past the end of the grid, so this test would be vacuous"
    );

    for arm in ClassificationSampler::ALL {
        let polls = AtomicUsize::new(0);
        // Let the first poll through, then cancel: every arm must surface
        // `Cancelled` rather than a truncated grid. The mixed-slope fixture is
        // 8192 faces on a ~177² grid, so every arm's window (8192 cells /
        // 1024 faces / one 64-cell tile band) leaves work after poll 2 — the
        // cancel really does land mid-grid, not before it starts and not on
        // the final chunk.
        let cancel = || polls.fetch_add(1, Ordering::Relaxed) >= 1;
        let out = sample_classification_grid(&fx.mesh, &index, spec, arm, &cancel);
        assert!(
            out.is_err(),
            "{} ignored a cancel raised mid-grid",
            arm.label()
        );
        // LATENCY, not just observance. The second poll is the first that
        // reports `true`; an arm that keeps working past it has a window
        // larger than it documents. One extra poll is allowed for a wrapper
        // that re-checks on the way out; more means the arm ran on.
        let observed = polls.load(Ordering::Relaxed);
        assert!(
            observed <= 3,
            "{} polled {observed} times for a cancel raised on poll 2 of a {bands}-band grid — \
             it kept working past the window it documents",
            arm.label()
        );
    }
}

// ── Timing / real fixture (ignored) ─────────────────────────────────────

fn read_proc_kb(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|l| {
        let rest = l.strip_prefix(key)?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

/// Process CPU time (user + system, all threads) from `/proc/self/stat`.
///
/// **Process-wide**, so it counts every thread in the test binary — including
/// another `#[test]` running concurrently. Wave 7a's first release run took
/// its timings with cargo's default two test threads and got CPU/wall ratios
/// of 14–25× on a 24-core box together with sub-millisecond walls for arms
/// that cannot be that fast: the two timing tests were measuring each other.
/// Study rows must be run with `--test-threads=1`; `timing_rows_are_serial`
/// refuses to let that be forgotten silently.
fn cpu_time() -> Option<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Field 2 (comm) may contain spaces inside parens; split after the last ')'.
    let tail = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = tail.split_whitespace().collect();
    // After comm and state, utime is field 14 overall → index 11 here.
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    // USER_HZ is 100 on every Linux target this project builds for.
    Some(Duration::from_millis((utime + stime) * 10))
}

/// Serialises the timing rows against each other whatever `--test-threads`
/// says. `cpu_time()` is process-wide and rayon will happily give a second
/// concurrent test the same 24 cores, so two timing tests running at once
/// measure each other rather than themselves — which is exactly what wave
/// 7a's first release run did. A lock is used rather than a check because a
/// check that only warns produces a plausible-looking table nobody
/// re-measures.
static TIMING_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Timing {
    arm: ClassificationSampler,
    side: usize,
    wall: Duration,
    cpu: Duration,
    rss_after_kb: u64,
    peak_kb: u64,
}

fn time_arm(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    spec: ClassificationGridSpec,
    arm: ClassificationSampler,
) -> (SurfaceHeightmap, Timing) {
    let cpu0 = cpu_time();
    let t0 = Instant::now();
    let grid = run(mesh, index, spec, arm);
    let wall = t0.elapsed();
    let cpu = match (cpu0, cpu_time()) {
        (Some(a), Some(b)) => b.saturating_sub(a),
        _ => Duration::ZERO,
    };
    let timing = Timing {
        arm,
        side: spec.rows,
        wall,
        cpu,
        rss_after_kb: read_proc_kb("VmRSS:").unwrap_or(0),
        peak_kb: read_proc_kb("VmHWM:").unwrap_or(0),
    };
    (grid, timing)
}

fn print_timings(title: &str, rows: &[Timing]) {
    println!("\n### {title}\n");
    println!("| grid | arm | wall s | CPU s | CPU/wall | RSS after MB | VmHWM MB (cumulative) |");
    println!("|---|---|---|---|---|---|---|");
    for t in rows {
        println!(
            "| {0}² | {1} | {2:.3} | {3:.3} | {4:.1}× | {5:.0} | {6:.0} |",
            t.side,
            t.arm.label(),
            t.wall.as_secs_f64(),
            t.cpu.as_secs_f64(),
            if t.wall.as_secs_f64() > 0.0 {
                t.cpu.as_secs_f64() / t.wall.as_secs_f64()
            } else {
                0.0
            },
            t.rss_after_kb as f64 / 1024.0,
            t.peak_kb as f64 / 1024.0,
        );
    }
}

#[test]
#[ignore = "M3 study row: loads the 10 MB terrain and sweeps to 849²; minutes, release only"]
fn terrain_equivalence_and_timing() {
    let _serial = TIMING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = terrain_path() else {
        panic!("no terrain fixture found; expected crates/rs_cam_core/tests/fixtures/terrain.stl");
    };
    println!("fixture: {}", path.display());
    let mesh = TriangleMesh::from_stl(&path).expect("load terrain");
    println!(
        "triangles: {}, bbox {:?}..{:?}",
        mesh.faces.len(),
        mesh.bbox.min,
        mesh.bbox.max
    );
    let cutter = common::tools::wanaka_taper();
    let index = SpatialIndex::build_auto(&mesh);

    let mut timings = Vec::new();
    let mut equivalences = Vec::new();
    for side in [143usize, 425, 849] {
        let spec = spec_at_side(&mesh, &cutter, side);
        let (oracle, t) = time_arm(&mesh, &index, spec, ClassificationSampler::DropCutterProbe);
        timings.push(t);
        for arm in ClassificationSampler::ALL {
            if arm == ClassificationSampler::DropCutterProbe {
                continue;
            }
            let (got, t) = time_arm(&mesh, &index, spec, arm);
            timings.push(t);
            equivalences.push(compare(arm, &format!("terrain@{side}"), &oracle, &got));
        }
    }
    print_timings("terrain timing", &timings);
    print_equivalence_rows(&equivalences);

    // Gate, on the arm that claims answer preservation only.
    for row in equivalences
        .iter()
        .filter(|r| r.arm == ClassificationSampler::DropCutterProbeScratch)
    {
        assert_eq!(
            row.verdict(),
            "EXACT",
            "the answer-preserving arm diverged on {}",
            row.fixture
        );
    }
    // The direct arms are production since wave 7b, so their terrain rows are
    // gated by the same tripwires as the analytic ones.
    for row in &equivalences {
        if !row.arm.samples_probe_cl_surface() {
            assert_eq!(
                row.covered_mismatch,
                0,
                "{} moved the coverage mask on {}",
                row.arm.label(),
                row.fixture
            );
        }
    }
    assert_label_move_tripwires(&equivalences);
}

#[test]
#[ignore = "M3 study row: analytic timing sweep, release only"]
fn analytic_timing_sweep() {
    let _serial = TIMING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cutter = common::tools::wanaka_taper();
    let fx = &analytic_fixtures()[2];
    let index = SpatialIndex::build_auto(&fx.mesh);
    let mut timings = Vec::new();
    for side in [143usize, 425, 849] {
        let spec = spec_at_side(&fx.mesh, &cutter, side);
        for arm in ClassificationSampler::ALL {
            let (_, t) = time_arm(&fx.mesh, &index, spec, arm);
            timings.push(t);
        }
    }
    print_timings("mixed-slope analytic timing", &timings);
}
