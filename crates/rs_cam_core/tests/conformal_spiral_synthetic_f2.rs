//! **Phase F2 steps 1–2 — the conformal-spiral evidence instrument.**
//! (`planning/conformal_finish_2026-08-28/PROGRAMME.md` §"Phase F2"; design
//! note `research/conformal_finish_2026-08-28.md` §B.4.)
//!
//! # What this measures
//!
//! One question, staged: does
//! [`rs_cam_core::conformal_spiral::plan_spiral`] produce a single
//! continuous, non-self-intersecting, **correctly-spaced** spiral over a
//! simply-connected region — and at what length overhead over the pure rings
//! it was bridged from?
//!
//! # ⚠ Read the FINDINGS_F2 withdrawal block before quoting anything terrain
//!
//! `planning/conformal_finish_2026-08-28/FINDINGS_F2.md` opens with a
//! WITHDRAWAL of every fine-geometry number this instrument produced on
//! `fixtures/terrain_small.stl`. The reason is a **fixture** defect, not an
//! algorithm one: the quantity being measured is a **0.4862 mm** equal-cusp
//! stepover, and the terrain window the census chose has **1.42 mm median
//! triangle edges** with a **30-vertex** boundary loop on a 24 × 18 mm
//! ellipse. Facets ~3× the measurand make every spacing, curvature and cost
//! figure an artefact of faceting. PROGRAMME.md §F2 step 1 always said
//! "simply connected **SYNTHETIC** surface"; substituting the terrain STL for
//! repo-portability was the mistake.
//!
//! This file therefore carries **two analytic arms first**, on test-local
//! synthetic meshes whose triangle edge is `≤ stepover/3` (≤ 0.16 mm) over
//! the machined region, and keeps the two terrain arms afterwards **as
//! fixture-limited probes with their fine-geometry numbers withdrawn**.
//!
//! | stage | what it prints |
//! |---|---|
//! | **F2-A** | the params actually used, the full 34-row `SpiralReport` grouped, the `N_S` adequacy arithmetic, and the **§B.4 falsifier** verdict |
//! | **F2-B** | two SVGs — the unit-disk domain, and the XY world view |
//! | **F2-C** | measured adjacent-ring 3D spacing vs the flat equal-cusp stepover AND vs the curvature-corrected stepover, plus the paper's own 12 % scallop-overshoot context |
//! | **F2-D** | drop-cutter CL conversion, containment count, F-034 cost vs a ball-end 0° raster on the same region |
//! | **F2-E** | sampling sensitivity: three `plan_spiral` runs at (N_S, N_C), (N_S/2, N_C) and (N_S, N_C/2) |
//!
//! # The §B.4 phase-1 falsifier, concretised 2026-08-30
//!
//! STOP the run when any of the **four** holds:
//!
//! * `uncovered_after_bridging > 0` — incomplete coverage **after** the
//!   §2.2.2 step-2 bridge repair;
//! * `disk_self_intersections > 0` — any disk-domain self-intersection;
//! * `bridge_overhead_pct > 25.0` — bridge length overhead vs pure rings
//!   (inside the §0k +43 % trap with margin);
//! * **`coverage_audit.unmachined_area_fraction > 2 %`** — or the audit being
//!   absent at all. Added 2026-08-30; see [`MAX_UNMACHINED_FRACTION`].
//!
//! **Why the fourth condition exists, stated plainly.** Without it this
//! instrument printed `uncovered after rings 0`, `uncovered after bridging 0`
//! and a **PASSING** falsifier over an ARM SPHERE run that left a 2.783 mm-
//! radius unmachined hole at the cap centre — **23.5 % of the region area**.
//! The first three conditions are all built from the ring search's **own**
//! self-report, and that report was vacuous: `N_S` was a cap, 46 % of the
//! 37,060 triangles were apportioned **zero** samples, so the coverage
//! predicate had no population in the middle and "covered" meant nothing
//! there. A falsifier assembled entirely from a mechanism's self-report
//! cannot fail when that mechanism is starved. Condition 4 reads a witness
//! drawn from a **different population** — the mesh's own triangle centroids
//! — which is the only reason it can see the hole. The controlled comparison
//! is ARM WAVY, whose quota never fell below 1.41 and which had no hole.
//!
//! `retract_count == 0` is **asserted**, not printed: the module's own claim
//! is that the construction emits exactly one polyline and never lifts, and
//! a structural claim gets a hard assert. `max_consecutive_step_mm` is
//! printed beside it, because that — not the zero — is the evidence that no
//! hidden jump is hiding inside the single polyline.
//!
//! **F-034 vs raster is CONTEXT, not a phase-1 bar.** §B.4 says so in those
//! words: "F-034 vs ball-end raster on the friendly fixture is context, not a
//! bar." The ellipse is deliberately friendly geometry — a convex,
//! hole-free, well-inside-the-mesh region — chosen so every triangle-centroid
//! test is clean, which is exactly the geometry a raster is *good* at.
//!
//! # Four arms, analytic first
//!
//! | # | arm | fixture | facet vs measurand | status of its numbers |
//! |---|---|---|---|---|
//! | 1 | **ARM SPHERE** | [`sphere_cap_mesh`], analytic | max 0.1585 mm (median 0.1099) vs 0.486 mm | **decisive**; refusal = hard failure |
//! | 2 | **ARM WAVY** | [`wavy_patch_mesh`], analytic | max 0.1545 mm (median 0.1087) vs 0.486 mm | **valid**; refusal = hard failure |
//! | 3 | ARM STEEP | `terrain_small.stl` | 0.80 mm vs 0.486 mm | fixture-limited PROBE; fine geometry WITHDRAWN |
//! | 4 | ARM FLAT | `terrain_small.stl` | 1.42 mm vs 0.486 mm | fixture-limited; fine geometry WITHDRAWN |
//!
//! **Why a sphere cap is the decisive fixture.** On a sphere of radius `R_s`
//! machined with a ball of radius `K_c` to scallop `h`, the constant-scallop
//! spacing is **analytic** —
//! `scallop_math::stepover_from_scallop_curved(K_c, h, 1/R_s)` — so the
//! measured adjacent-ring spacing has a single known right answer, with no
//! faceting confound and no curvature census needed to guess a target. The
//! instrument additionally derives the same number a second way, straight
//! from the module's **own** coverage law (an `S^h` point is covered when it
//! is within `K_c` of the `+K_c` offset curve — [`sphere_coverage_spacing_mm`]),
//! and prints the two side by side.
//!
//! **The falsifiable split ARM SPHERE decides** (FINDINGS_F2 withdrawal
//! block). Measured terrain spacing was 0.2275 mm against a 0.4862 mm target
//! — almost exactly HALF, and 0.243 mm is precisely the lateral distance at
//! which a `K_c = 1.0` ball stops covering the `h = 0.03` iso-scallop
//! surface. So:
//!
//! * spacing near the **analytic target** ⇒ the terrain numbers were
//!   **faceting**: fixture defect, algorithm fine;
//! * spacing near **HALF** the analytic target ⇒ an **off-by-one-band defect
//!   in the ring recursion** — the search not crediting the band the
//!   *previous* ring already covered.
//!
//! The two ±25 % bands are disjoint (0.75·T = 0.356 mm > 1.25·T/2 = 0.297 mm
//! at T = 0.474 mm), so the verdict is a three-way branch with a genuine
//! INCONCLUSIVE arm for a mixed reading — not a hedge.
//!
//! ARM STEEP remains the envelope **probe**: the whole inset ellipse over
//! this fixture's full 52.6 mm of relief, whose expected outcome is a
//! *diagnosed* `RingSearchStalled`. That is a result, not a failure —
//! `plan_spiral` returns `(SpiralReport, Result<…>)`, so the report survives
//! the refusal. It fails only when a refusal arrives with **nothing to
//! attribute it by**. ARM FLAT is the terrain envelope-inside test, retained
//! so the two arms' **radial area-distortion profiles** can still be read
//! side by side — that pair is structural and survives the withdrawal.
//!
//! Stage E's sampling sensitivity runs on **ARM FLAT only**.
//!
//! An earlier revision of this paragraph justified not re-running it on the
//! analytic arms by saying "the Stage-A `N_S` adequacy ratio (≈0.31 on the
//! sphere) already answers the sampling question quantitatively there."
//! **That justification was wrong and is retracted.** The ≈0.31 was
//! `√(region_area / N_S)` — an *average*, blind to apportionment — computed
//! on the very run where 46 % of triangles held zero samples and 23.5 % of
//! the region went unmachined. It did not answer the sampling question; it
//! could not even see it. The adequacy block now reports
//! `max_local_sample_spacing_mm` (a **max** over per-triangle densities, so a
//! single starved triangle moves it) together with `triangles_without_samples`
//! as a must-be-zero tripwire, and the coverage audit answers the separate
//! question of whether the emitted path actually machined the region. Stage E
//! remains ARM-FLAT-only for runtime reasons alone — two extra solves on a
//! 37 k-triangle region — and its table now carries `starved` and
//! `unmachined%` columns so that halving `N_S`, which is exactly the lever
//! that produced the hole, cannot report three healthy rows.
//!
//! # What a fold means here, and why the flip count is now a tripwire
//!
//! The module's flattening moved from cotangent-Laplacian + CG to
//! **mean-value (Floater) weights + Gauss–Seidel**. Every mean-value weight
//! is strictly positive, so Tutte's spring-embedding theorem makes a valid
//! embedding *certain* for a manifold disk on a convex boundary: **folds are
//! structurally impossible**, not empirically rare. This instrument therefore
//! treats `flipped_triangles` as a **tripwire on a structural invariant** — a
//! nonzero count is a bug or an under-converged solve, never a discretisation
//! symptom. Earlier revisions of this file said the opposite (the cotangent
//! version measured 298 flips of 9107 on this same fixture); that reading is
//! retired and must not be carried forward.
//!
//! Why it matters to the ring search, which is what confirmed the change:
//! `FlatLocator` resolves a fold's multivaluedness **by first hit**, so over a
//! folded sector the forward and inverse maps disagree and the samples there
//! can never be swept *at any radius*. `StallContext::uncovered_outside_last_ring`
//! is the census of exactly that — still-uncovered points the ring search had
//! already certified as swept — and it is **0 for an embedding**. The
//! diagnosis reads it and `flipped_triangles` together as two views of one
//! defect.
//!
//! # Selection hygiene — and why the module's refusals stay authoritative
//!
//! "The ellipse is friendly geometry" describes the *polygon*. Turning it
//! into a triangle set is not friendly at all, and two operator runs proved
//! it before this file planned anything:
//!
//! * **run 1** — `fixtures/terrain_small.stl` is a CLOSED solid (terrain
//!   top plus skirt plus floor), so a bare XY centroid test selects the top
//!   sheet AND the floor plate under it. `NotSimplyConnected { 2 }`, at every
//!   ellipse size. Fixed by the up-facing normal filter ([`MIN_UP_NORMAL_Z`]);
//! * **run 2** — with that filter at a 0.1 threshold, every ellipse size
//!   refused `NonManifoldBoundary { BoundaryPinch }`: the ragged fringe of a
//!   per-triangle centroid test leaves bowtie vertices, and a 0.1 threshold
//!   additionally excludes steep *interior* triangles, which punches holes.
//!
//! Both are artefacts of **selection**, not facts about the surface, so
//! [`clean_selection`] repairs them here, in the test, and **nothing in
//! `src/` was touched**. The module's checks are not weakened, relaxed or
//! bypassed: the cleanup only ever *removes* triangles, and whatever survives
//! is still put to `plan_spiral` to accept or refuse on its own terms. A
//! refusal on a cleaned selection is a fact about the region and is reported
//! as one.
//!
//! # Cross-arm comparability with F1
//!
//! `BALL_RADIUS_MM = 1.0` and `CUSP_HEIGHT_MM = 0.03` are F1's
//! (`direction_field_wanaka_f1.rs:107,117`), so the equal-cusp stepover is
//! the same 0.4862 mm and the two instruments' spacing tables can be read
//! against each other. The region and the mesh are **not** shared — F1 runs
//! on the operator's machine-local Wanaka board, this file runs on an in-repo
//! fixture by design (PROGRAMME.md §F2 step 1: "keeping phase 1
//! repo-portable").
//!
//! # Restated, not imported
//!
//! Integration tests cannot import from each other, and the repo's rule is
//! that *an instrument should show its own arithmetic*. Restated from
//! `direction_field_wanaka_f1.rs` at the noted lines:
//!
//! * `equal_cusp_stepover_mm` (:174–179)
//! * the pinned Shapeoko Pro XXL kinematics + feeds (:125–132)
//! * `repo_root` (:182–187) and `svg_output_dir` (:399–410)
//! * `svg_path` (:414–430)
//! * `grid_for_direction` (:272–287) and `raster_candidate` (:290–313)
//! * `CandidateCost` (:322–329) and `relink_and_cost` (:343–390) — the
//!   `RelinkParams` block field-for-field
//! * `cl_polylines` (:454–485) and `polylines_to_toolpath` (:499–523)
//! * `point_segment_distance_sq` (:528–539) and `percentile` (:542–548)
//! * the `Fixture` bundle (:900–910) and the `FRESH_STOCK_LABEL` discipline
//!   (:165–167)
//!
//! F1 itself restates most of these from `thin_organic_island_widths.rs`; the
//! chain of provenance is recorded at each definition below.
//!
//! # Running it
//!
//! ```text
//! THIN_ORGANIC_SVG_DIR=/home/ricky/Downloads/svg \
//! cargo test -p rs_cam_core --test conformal_spiral_synthetic_f2 \
//!   terrain_small_conformal_spiral_f2 -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::f64::consts::{PI, SQRT_2, TAU};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::conformal_spiral::{
    self, DistanceStats, PAPER_START_ANGLE_STEP, SpiralParams, SpiralRefusal, SpiralReport,
    SpiralResult,
};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::scallop_math;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// ── the fixture ─────────────────────────────────────────────────────────

/// In-repo evidence fixture named by PROGRAMME.md §F2 step 1. Binary STL,
/// header-verified 2026-08-30.
const FIXTURE_RELATIVE: &str = "fixtures/terrain_small.stl";

/// Triangle count of that fixture, read from the binary STL header
/// (`2_017_184 == 84 + 50 × 40_342`, so the header agrees with the file
/// size). Pinned so a silently swapped fixture fails loudly.
const FIXTURE_TRIANGLES: usize = 40_342;

// ── the tooling decision (design note §C.4, shared with F1) ─────────────

/// Ball-end cutter radius (mm). **F1's value**
/// (`direction_field_wanaka_f1.rs:107`), so the two instruments' spacing
/// tables are on the same stepover.
const BALL_RADIUS_MM: f64 = 1.0;

/// Cutting length of the ball fixture (mm). Inert for every measurement here
/// — drop-cutter geometry reads `radius()` / `height_at_radius()` only — but
/// a cutter needs one. **[`BallEndmill::new`] takes a DIAMETER**, so the
/// constructor below is `new(2.0, 20.0)` for a radius of 1.0; both numbers
/// are printed in Stage A so a reader can check that trap.
const BALL_CUTTING_LENGTH_MM: f64 = 20.0;

/// Scallop-height constraint `h` (mm). F1's value
/// (`direction_field_wanaka_f1.rs:117`).
const CUSP_HEIGHT_MM: f64 = 0.03;

// ── the region (a [REPO] choice, stated) ────────────────────────────────

/// How far inside the mesh's XY bounding box the region's inscribing box
/// sits. PROGRAMME.md asks for "a hole-free region polygon"; 8 mm keeps the
/// whole ellipse — and the cutter-centre curve offset from it — well clear of
/// the mesh's own boundary triangles, so every centroid-in-polygon test is
/// decided by the polygon rather than by the mesh edge.
const REGION_INSET_MM: f64 = 8.0;

/// Vertices on the region ellipse. Dense enough that the polygon's chord
/// sagitta (≈ 0.007 mm at these semi-axes) is far below the ~1.5 mm mesh
/// triangle scale, so the centroid test is not deciding on discretisation.
const ELLIPSE_VERTICES: usize = 256;

/// Semi-axis shrink applied when `plan_spiral` refuses the selected region on
/// topology grounds. Centroid selection can leave a ragged one-triangle
/// fringe, which reads as a second boundary loop, a pinch, or a non-disk
/// Euler characteristic.
const SHRINK_STEP_MM: f64 = 2.0;

/// How many shrinks before giving up.
const MAX_SHRINKS: usize = 3;

// ── the falsifier (research note §B.4, concretised 2026-08-30) ──────────

/// Bridge length overhead vs pure rings, in per cent, above which the run
/// STOPs.
const MAX_BRIDGE_OVERHEAD_PCT: f64 = 25.0;

/// **[SOURCE-2025 §5]** The paper's own cutting trial overshot its nominal
/// scallop height by up to this much. The module header calls it "the
/// expected floor, not the ceiling"; Stage C flags the fraction of spacing
/// samples implying an overshoot beyond it.
const PAPER_SCALLOP_OVERSHOOT_PCT: f64 = 12.0;

// ── runtime deviation, documented ───────────────────────────────────────

/// Start-angle sweep step used by **every** `plan_spiral` call in this file,
/// Stage E's re-runs included.
///
/// [`PAPER_START_ANGLE_STEP`] is `π/50` — 99 candidates by the module's own
/// `(TAU / step).floor()` count — each of which rebuilds the whole spiral and
/// re-locates every one of its points through the flattened mesh's spatial
/// index. On a 40k-triangle region with ~10² rings that is ~5 × 10⁶ point
/// locations for the sweep alone, which projects to minutes per call and is
/// multiplied by three in Stage E.
///
/// `π/5` gives **10** candidates — exactly a tenth of the paper's resolution.
/// It changes only *which* start angle minimises total 3D length — not the
/// ring spacing, not the coverage, not the bridging mechanism, and not any
/// falsifier input. The paper's own description of this sweep is "trial and
/// error".
///
/// It is a single shared constant on purpose: if Stage E's re-runs swept at a
/// different resolution, its rows would differ in two variables (the sampling
/// dial *and* the sweep) and would stop being evidence about sampling.
/// `π/5` is also an exact whole number of lattice cells on **both** angular
/// lattices Stage E uses — 36 cells at `N_C = 360`, 18 at `N_C = 180` — so the
/// module's own snap-to-lattice of the start angle
/// (`((k · step) / dθ).round() · dθ`) is a no-op on every row and cannot make
/// one row's candidate set a different shape from another's. `π/4` would have
/// been 22.5 cells at `N_C = 180`; that is why it is not the constant here.
const SWEEP_START_ANGLE_STEP: f64 = PI / 5.0;

// ── the pinned machine (direction_field_wanaka_f1.rs:125-132) ───────────
//
// Shapeoko Pro XXL, read from `wanaka200_mt2.toml`'s `[job.machine]` /
// `[job.machine.kinematics]`. Restated unchanged so the F2 cost column is
// integrated on the same envelope as F1's, even though the geometry differs.

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

/// The `link_ceiling: None` label the charter makes mandatory, restated from
/// `direction_field_wanaka_f1.rs:165-167`. Printed at EVERY costing surface,
/// because a table gets quoted without its preamble.
const FRESH_STOCK_LABEL: &str = "link_ceiling: None — FRESH-STOCK FIRST-EXPERIMENT EXCEPTION (PROGRAMME.md \
     'Shared experimental contract'). NO TIME CLAIM HERE IS FINAL until this \
     candidate is re-run under the corrected rest-stock ceiling.";

// ── what a fixture can and cannot resolve ───────────────────────────────

/// Whether an arm's fixture can resolve the quantity the arm reports.
///
/// This is not decoration. Every fine-geometry number this instrument prints
/// is a comparison against a **0.4862 mm** stepover, and a mesh whose facets
/// are 1.42 mm across cannot produce one. The distinction is carried through
/// the staged printers so that a reader who quotes a table out of context
/// still sees which side of the withdrawal it is on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArmKind {
    /// A test-local analytic mesh with `edge ≤` [`ANALYTIC_MAX_EDGE_MM`] over
    /// the machined region. Its fine-geometry numbers are valid, and a
    /// refusal on it is a **hard failure**.
    Analytic,
    /// `fixtures/terrain_small.stl`. Retained as a probe; every fine-geometry
    /// number is **withdrawn** — see [`WITHDRAWAL_BANNER`].
    FixtureLimited,
}

/// Printed at the head of every fixture-limited arm and above every one of
/// its fine-geometry tables, because a table gets quoted without its
/// preamble — the same reason [`FRESH_STOCK_LABEL`] is repeated at every
/// costing surface.
const WITHDRAWAL_BANNER: &str = "*** FIXTURE-LIMITED PROBE — EVERY FINE-GEOMETRY NUMBER BELOW IS WITHDRAWN. See \
     planning/conformal_finish_2026-08-28/FINDINGS_F2.md, the WITHDRAWAL block at the top of the \
     file. This arm's facets (0.80 mm median mesh-wide, 1.42 mm in the censused flat window) are \
     up to ~3x the 0.4862 mm quantity being measured, and its region boundary is a 30-vertex, \
     ~4.6 mm-segment polygon. Its spacing distribution, curvature census, containment count and \
     cost table are FACETING ARTEFACTS and must not be quoted. What survives here is structural \
     and resolution-independent: continuity, retract count, self-intersection count, bridge \
     overhead, the fold census and the radial area-distortion profile. The phase-1 spacing \
     verdict is carried by ARM SPHERE and ARM WAVY. ***";

// ── restated arithmetic ─────────────────────────────────────────────────

/// `s = 2·√(2Rh − h²)` — the equal-cusp law. **Restated from
/// `direction_field_wanaka_f1.rs:174-179`** (which restates it from
/// `thin_organic_island_widths.rs:129-137`); one site in production,
/// `session::multitool`.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// Absolute path to the workspace root, canonicalized. **Restated from
/// `direction_field_wanaka_f1.rs:182-187`.**
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// The fixture path. Resolved relative to `CARGO_MANIFEST_DIR`, the same way
/// `end_to_end.rs:31-33` and `geometry_cache_g8.rs:116` do.
fn fixture_path() -> PathBuf {
    repo_root().join(FIXTURE_RELATIVE)
}

/// Where the operator-facing debug SVGs land. **Restated from
/// `direction_field_wanaka_f1.rs:399-410`**: same environment override
/// (`THIN_ORGANIC_SVG_DIR`), a different default directory so an F1 and an F2
/// run cannot overwrite each other.
fn svg_output_dir() -> PathBuf {
    std::env::var_os("THIN_ORGANIC_SVG_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("conformal_f2")
        },
        PathBuf::from,
    )
}

/// One polygon as an SVG path, exterior then holes. **Restated from
/// `direction_field_wanaka_f1.rs:414-430`.**
fn svg_path(poly: &Polygon2) -> String {
    fn append_ring(path: &mut String, ring: &[P2]) {
        let Some(first) = ring.first() else { return };
        write!(path, "M {:.3} {:.3}", first.x, first.y).expect("write SVG path");
        for point in &ring[1..] {
            write!(path, " L {:.3} {:.3}", point.x, point.y).expect("write SVG path");
        }
        path.push_str(" Z");
    }

    let mut path = String::new();
    append_ring(&mut path, &poly.exterior);
    for hole in &poly.holes {
        append_ring(&mut path, hole);
    }
    path
}

/// Rotated drop-cutter lattice. **Restated from
/// `direction_field_wanaka_f1.rs:272-287`** (itself
/// `thin_organic_island_widths.rs:1478-1493` with the ball fixture).
fn grid_for_direction(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
    stepover: f64,
    direction_deg: f64,
) -> rs_cam_core::dropcutter::DropCutterGrid {
    rs_cam_core::dropcutter::batch_drop_cutter(
        mesh,
        index,
        cutter,
        stepover,
        direction_deg,
        mesh.bbox.min.z - 0.1,
    )
}

/// **Restated from `direction_field_wanaka_f1.rs:290-313`.**
fn raster_candidate(
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> Toolpath {
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

    let mut out = Toolpath::new();
    for polygon in regions {
        let region = RegionSet::new(vec![polygon.clone()]);
        let toolpath = raster_toolpath_from_grid(
            grid,
            FEED_MM_MIN,
            PLUNGE_MM_MIN,
            safe_z,
            Some(effective_min_z),
            Some(&region),
        );
        out.moves.extend(toolpath.moves);
    }
    out
}

/// **Restated from `direction_field_wanaka_f1.rs:322-329`**, field for field.
/// The two ceiling-regime fields were already trimmed there for the reason
/// stated at that definition, and this file is likewise `link_ceiling: None`
/// throughout.
struct CandidateCost {
    moves: usize,
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
}

/// **Restated from `direction_field_wanaka_f1.rs:343-390`.** The
/// `RelinkParams` block is field-for-field identical: `hookup_distance` 25.0
/// (the operator's `intra_region_hookup_mm`, `wanaka200_mt2.toml:949`),
/// `stock_to_leave` 0.0, `sampling` 0.5, tier-1 feeds, `reorder: true`, the
/// region's own polygon as boundary, `link_ceiling: None`, and both regime
/// flags `false`.
fn relink_and_cost(
    raw: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
    boundary: &rs_cam_core::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine_kinematics::MachineKinematics,
    safe_z: f64,
) -> CandidateCost {
    use rs_cam_core::machine_kinematics::{LinkKinematics, compute_cycle_time};

    let link_kinematics = LinkKinematics {
        kinematics: *kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let params = rs_cam_core::surface_link::RelinkParams {
        hookup_distance: 25.0,
        stock_to_leave: 0.0,
        sampling: 0.5,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z,
        link_kinematics: Some(&link_kinematics),
        reorder: true,
        boundary: Some(boundary),
        link_ceiling: None,
        flush_ride: false,
        airborne_links_may_leave_territory: false,
    };
    let (linked, report) = rs_cam_core::surface_link::relink_fragments(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raw),
        mesh,
        index,
        cutter,
        &params,
    );
    let mut channels = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
    let toolpath = linked.reconcile(&mut channels).into_inner().toolpath;
    CandidateCost {
        moves: toolpath.moves.len(),
        cutting_mm: toolpath.total_cutting_distance(),
        time_s: compute_cycle_time(&toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN),
        fragments: report.fragments,
        linked: report.surface_links,
        kept_retracts: report.retract_links,
    }
}

/// Contact-point polylines → cutter-centre (CL) polylines. **Restated from
/// `direction_field_wanaka_f1.rs:454-485`**, including the `-inf` guard:
/// `conformal_spiral` emits **cutter-contact points on the mesh surface** (its
/// module header says so, and says the drop-cutter CL of the evidence
/// instrument SUPERSEDES its own normal-offset centre curve, because the paper
/// has no gouge handling at all — extraction gap 3).
///
/// Two guards, both counted rather than silently absorbed:
///
/// * a CL point whose drop-cutter never contacted carries `z = -∞`
///   (`tool/mod.rs` `CLPoint::contacted`), which would poison the F-034
///   integration — dropped and counted;
/// * a polyline left with fewer than two points is not a path.
struct ClConversion {
    polylines: Vec<Vec<P3>>,
    input_points: usize,
    dropped_points: usize,
    dropped_polylines: usize,
}

fn cl_polylines(
    contact: &[Vec<P3>],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
) -> ClConversion {
    let mut out = ClConversion {
        polylines: Vec::with_capacity(contact.len()),
        input_points: 0,
        dropped_points: 0,
        dropped_polylines: 0,
    };
    for line in contact {
        out.input_points += line.len();
        let mut cl: Vec<P3> = Vec::with_capacity(line.len());
        for point in line {
            let probe =
                rs_cam_core::dropcutter::point_drop_cutter(point.x, point.y, mesh, index, cutter);
            if probe.contacted && probe.z.is_finite() {
                cl.push(probe.position());
            } else {
                out.dropped_points += 1;
            }
        }
        if cl.len() >= 2 {
            out.polylines.push(cl);
        } else {
            out.dropped_polylines += 1;
        }
    }
    out
}

/// Assemble CL polylines into a raw `Toolpath`. **Restated from
/// `direction_field_wanaka_f1.rs:499-523`**, whose move-intent tagging mirrors
/// `toolpath::raster_toolpath_from_grid` (`toolpath.rs:695-744`) exactly,
/// because the relinker and the F-034 integrator read those tags.
///
/// For the spiral arm the input is **one** polyline, so this emits exactly:
/// a `Linking` rapid across at `safe_z`, one `EntryPlunge`, then `FinishingCut`
/// feeds all the way to the end, then one closing `Retract`. That is the
/// stay-down continuity claim rendered as motion.
fn polylines_to_toolpath(
    polylines: &[Vec<P3>],
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let mut tp = Toolpath::new();
    let mut down_at: Option<P3> = None;
    for line in polylines {
        let Some(first) = line.first() else { continue };
        if let Some(prev) = down_at.take() {
            tp.rapid_to_with_intent(P3::new(prev.x, prev.y, safe_z), MoveIntent::Retract);
        }
        tp.rapid_to_with_intent(P3::new(first.x, first.y, safe_z), MoveIntent::Linking);
        tp.feed_to_with_intent(*first, plunge_rate, MoveIntent::EntryPlunge);
        for point in &line[1..] {
            tp.feed_to_with_intent(*point, feed_rate, MoveIntent::FinishingCut);
        }
        down_at = line.last().copied();
    }
    if let Some(prev) = down_at {
        tp.rapid_to_with_intent(P3::new(prev.x, prev.y, safe_z), MoveIntent::Retract);
    }
    tp
}

/// Squared 3D distance from `p` to the segment `a`–`b`. **Restated from
/// `direction_field_wanaka_f1.rs:528-539`.**
fn point_segment_distance_sq(p: P3, a: P3, b: P3) -> f64 {
    let (abx, aby, abz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let denominator = abx * abx + aby * aby + abz * abz;
    let (apx, apy, apz) = (p.x - a.x, p.y - a.y, p.z - a.z);
    let t = if denominator <= 1e-18 {
        0.0
    } else {
        ((apx * abx + apy * aby + apz * abz) / denominator).clamp(0.0, 1.0)
    };
    let (dx, dy, dz) = (apx - t * abx, apy - t * aby, apz - t * abz);
    dx * dx + dy * dy + dz * dz
}

/// Percentile of an already-sorted slice, nearest-rank. **Restated from
/// `direction_field_wanaka_f1.rs:542-548`.**
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Everything every costed arm shares. **Restated from
/// `direction_field_wanaka_f1.rs:900-910`** for the same reason it exists
/// there: clippy's `too_many_arguments` fires at eight. All fields are `Copy`,
/// so it rides by value.
#[derive(Clone, Copy)]
struct Fixture<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a BallEndmill,
    kinematics: rs_cam_core::machine_kinematics::MachineKinematics,
    /// `mesh.bbox.max.z + 5.0`, as the reference instruments compute it.
    safe_z: f64,
    /// `mesh.bbox.min.z - 0.1`, as the reference instruments compute it.
    effective_min_z: f64,
}

// ── ANALYTIC FIXTURES (added 2026-08-30 — the withdrawal's repair) ──────
//
// Everything in this section is **test-local**: no `src/` change, no new
// dependency, and no fixture file on disk. Both generators are pure
// functions of their parameters, so what a run measured is reproducible from
// the constants below rather than from a binary blob.

/// Triangle-edge ceiling (mm) every analytic mesh honours **over the whole
/// mesh**, not just the machined region.
///
/// The brief's rule is `edge ≤ stepover / 3`, and `0.4862 / 3 = 0.16207`;
/// this constant is the round 0.16, i.e. deliberately **stricter** than the
/// rule. Both generators are sized so the longest of a triangle's THREE 3D
/// edges — the ring-band and grid-cell **diagonals** included, which is the
/// one that actually binds — clears it, and
/// [`sphere_cap_mesh_is_an_analytic_disk_fine_enough_to_measure`] /
/// [`wavy_patch_mesh_is_a_clean_disk_within_its_slope_and_curvature_bounds`]
/// measure it rather than trusting the arithmetic.
const ANALYTIC_MAX_EDGE_MM: f64 = 0.16;

/// Gauss–Seidel sweep cap for the ANALYTIC arms only.
///
/// [`SpiralParams`]'s default is 50 000, and reaching it is
/// `FlattenDidNotConverge` — a **refusal**, which on an analytic arm is a
/// hard failure by this file's own rule. The analytic regions are 20–40×
/// larger than the terrain FLAT window (37 060 vs 308 triangles, ~18 k
/// interior vertices vs ~150), and Gauss–Seidel's sweep count grows with the
/// square of the mesh's linear dimension, so the default straddles what this
/// solve plausibly needs. Raising the cap does **not** loosen the stopping
/// criterion: `solver_tolerance` is untouched, so an under-solve is still a
/// refusal, never a silent pass. It is raised on the analytic arms **only** —
/// the terrain arms keep the shipped default so their rows stay comparable
/// with the pre-withdrawal run.
const ANALYTIC_SOLVER_MAX_SWEEPS: usize = 250_000;

// ---- ARM SPHERE: the decisive analytic fixture -------------------------

/// Sphere radius `R_s` (mm) the cap is cut from.
///
/// 20 mm keeps the rim slope at `asin(6/20) = 17.46°`, so every face normal
/// is comfortably up-facing (`n_z ≥ 0.954`) and the mesh stays a heightfield,
/// while still bending the target: the curvature-corrected stepover is
/// 0.4743 mm against the flat law's 0.4862 mm. That 2.4 % correction is
/// small **on purpose** — the verdict split is target-vs-HALF-target (2×), so
/// it cannot turn on which of the two targets a reader picks.
const SPHERE_RADIUS_MM: f64 = 20.0;

/// Planar radius (mm) of the cap — the machined circle is `2 ×` this,
/// **12.0 mm across**, as the brief asks.
const SPHERE_CAP_RADIUS_MM: f64 = 6.0;

/// Radial bands in the polar triangulation. `6.0 / 55 = 0.10909 mm` planar.
const SPHERE_CAP_RINGS: usize = 55;

/// Sectors per band. Constant across bands — see [`sphere_cap_mesh`] for why
/// the simpler (if slightly denser) constant-sector mesh was chosen over a
/// graded one.
///
/// `2 × 6 × sin(π/340) = 0.11088 mm` at the rim, and the binding **diagonal**
/// measures 0.15850 mm — 1 % inside [`ANALYTIC_MAX_EDGE_MM`].
const SPHERE_CAP_SECTORS: usize = 340;

/// A spherical cap, triangulated in polar rings, as a **convex-UP dome**.
///
/// # Which way up, and why the brief's spelling is not the shape
///
/// The brief writes "`z = R - sqrt(R^2 - r^2)` style (convex UP dome)". Those
/// two halves disagree: `R − √(R²−r²)` is a **bowl** (minimum at the centre,
/// concave up). The parenthetical wins, and so does the brief's own physics —
/// it specifies `curvature = 1/R_s` **convex**, which only a dome has. This
/// generator therefore emits
///
/// ```text
/// z(r) = √(R_s² − r²) − √(R_s² − a²)
/// ```
///
/// with `a = cap_radius_mm`: apex up at `z = R_s − √(R_s²−a²)`, rim at
/// `z = 0`, sphere centre at `(0, 0, −√(R_s²−a²))`. Stated here so nobody
/// "fixes" it back to the bowl.
///
/// # Structure
///
/// Vertex 0 is the apex; then `rings` concentric rings of `sectors` vertices
/// at `r_i = a·i/rings`. Band 1 is a fan of `sectors` triangles from the
/// apex; bands 2..=rings are `2 × sectors` each. So
///
/// * triangles `= sectors · (2·rings − 1)` = **37 060** at the pinned dials,
/// * vertices `= 1 + rings · sectors` = 18 701, of which 340 are boundary,
/// * Euler `V − E + F = 18701 − 55760 + 37060 = 1` — a disk, one boundary
///   loop, exactly the rim circle.
///
/// Every triangle is wound counter-clockwise **in XY** (checked in polar
/// form: twice the signed area of a fan triangle is `r² sin Δθ > 0`, of an
/// outward band triangle `r_out(r_out − r_in) sin Δθ > 0`, and of the inward
/// one `r_in(r_out − r_in) sin Δθ > 0`), so every face normal has `n_z > 0`
/// and the smoke test asserts it rather than relying on that reading.
///
/// # Why constant sectors
///
/// A graded mesh (sector count halving toward the centre) would cut ~33 % of
/// the triangles, at the price of transition bands whose stitching cannot be
/// checked by reading. This instrument's edits ship without a local `cargo`
/// run, so provable-by-inspection beat the saving. 37 060 sits inside the
/// brief's stated ~12 k–40 k band; the price is that the innermost triangles
/// are finer than they need to be, which costs time and never accuracy.
fn sphere_cap_mesh(
    sphere_radius_mm: f64,
    cap_radius_mm: f64,
    rings: usize,
    sectors: usize,
) -> TriangleMesh {
    let rings = rings.max(1);
    let sectors = sectors.max(3);
    let rim_z = (sphere_radius_mm * sphere_radius_mm - cap_radius_mm * cap_radius_mm)
        .max(0.0)
        .sqrt();
    let height = |r: f64| {
        (sphere_radius_mm * sphere_radius_mm - r * r)
            .max(0.0)
            .sqrt()
            - rim_z
    };

    let mut verts: Vec<P3> = Vec::with_capacity(1 + rings * sectors);
    verts.push(P3::new(0.0, 0.0, height(0.0)));
    for i in 1..=rings {
        let r = cap_radius_mm * (i as f64) / (rings as f64);
        let z = height(r);
        for j in 0..sectors {
            let t = TAU * (j as f64) / (sectors as f64);
            verts.push(P3::new(r * t.cos(), r * t.sin(), z));
        }
    }

    // Ring `i` is 1-based; sector `j` wraps.
    let v = |i: usize, j: usize| -> u32 { (1 + (i - 1) * sectors + (j % sectors)) as u32 };

    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(sectors * (2 * rings - 1));
    for j in 0..sectors {
        tris.push([0, v(1, j), v(1, j + 1)]);
    }
    for i in 2..=rings {
        for j in 0..sectors {
            let (a, b) = (v(i - 1, j), v(i - 1, j + 1));
            let (c, d) = (v(i, j), v(i, j + 1));
            tris.push([a, c, d]);
            tris.push([a, d, b]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// ARM SPHERE machines the **whole** cap: the region boundary IS the mesh
/// boundary, an exact 340-vertex circle of radius 6.0 with 0.111 mm segments.
///
/// That is the direct repair of the withdrawal's third symptom — terrain's
/// region boundary was a **30-vertex, ~4.6 mm-segment** polygon for a 24 × 18
/// ellipse, and the ring search sizes each step by its single worst uncovered
/// point, so a coarse boundary polygon is what crowded the rings against the
/// rim. Here there is no centroid selection and no fringe at all, so that
/// mechanism cannot fire.
///
/// The cost of taking the whole cap rather than insetting a margin is a
/// **rim roll-off** in Stage D: a ball dropped at the rim rests on the mesh
/// edge. Both the spiral and the raster pay it identically, and Stage D says
/// so where it prints the cost table.
fn sphere_cap_region(mesh: &TriangleMesh) -> Vec<u32> {
    (0..mesh.triangles.len() as u32).collect()
}

/// The constant-scallop spacing on a convex sphere, derived from the
/// **module's own coverage law** rather than from `scallop_math`.
///
/// [`SpiralParams::scallop_h_mm`]'s doc and `conformal_spiral`'s §3.1 section
/// define it exactly: `S^h` is the contact surface offset `+h` along the
/// normal, the tool-centre curve is the contact curve offset `+K_c`, and an
/// `S^h` point is **covered** when it lies within `K_c` of that curve. On a
/// sphere of radius `R_s` those are three concentric spheres of radii `R_s`,
/// `R_h = R_s + h` and `R_c = R_s + K_c`, so for an `S^h` point sitting at
/// angular offset `α` from the nearest centre-curve point, the law of cosines
/// gives coverage exactly at
///
/// ```text
/// R_c² + R_h² − 2·R_c·R_h·cos α = K_c²
/// ```
///
/// and adjacent contact rings are `2α` of arc apart, i.e. a 3D chord of
/// `2·R_s·sin α`. At `R_s = 20, K_c = 1, h = 0.03` that is **0.474128 mm**,
/// which agrees with `scallop_math::stepover_from_scallop_curved`'s
/// **0.474312** to **0.039 %**. Two independent derivations of one target is
/// what makes the sphere arm's verdict quotable — but the instrument computes
/// and prints both rather than trusting these figures, because a stale
/// comment is a lie a reader then cites.
fn sphere_coverage_spacing_mm(sphere_radius_mm: f64, ball_radius_mm: f64, cusp_h_mm: f64) -> f64 {
    let r_c = sphere_radius_mm + ball_radius_mm;
    let r_h = sphere_radius_mm + cusp_h_mm;
    let denominator = 2.0 * r_c * r_h;
    if denominator <= 0.0 {
        return f64::NAN;
    }
    let cos_alpha =
        ((r_c * r_c + r_h * r_h - ball_radius_mm * ball_radius_mm) / denominator).clamp(-1.0, 1.0);
    2.0 * sphere_radius_mm * cos_alpha.acos().sin()
}

// ---- ARM WAVY: the "realistic but clean" analytic fixture --------------

/// Side of the square heightfield patch (mm).
const WAVY_SIZE_MM: f64 = 14.0;

/// Undulation amplitude `A` (mm). See [`wavy_patch_mesh`] for the slope and
/// concavity arithmetic that picks it.
const WAVY_AMPLITUDE_MM: f64 = 0.5;

/// Undulation wavelength `L` (mm) — 1.75 full periods across the patch.
const WAVY_WAVELENGTH_MM: f64 = 8.0;

/// Radius (mm) of the machined circle inside the wavy patch, leaving a 2 mm
/// margin of mesh outside it so the drop-cutter CL conversion never runs off
/// the patch edge.
const WAVY_REGION_RADIUS_MM: f64 = 5.0;

/// Grid cells per side for [`wavy_patch_mesh`], from the edge budget.
///
/// The binding edge of a split quad is its **diagonal**: `√2·cell` in XY,
/// plus whatever `z` the surface climbs across it. Bounding that climb by the
/// surface's steepest gradient `|∇z|_max` gives a 3D diagonal of at most
/// `√2 · cell / cos θ_max`, so
///
/// ```text
/// cell ≤ edge_mm · cos θ_max / √2
/// ```
///
/// A separate function from [`wavy_patch_mesh`] because the region selector
/// needs the same `n` to address the same cells, and two copies of this
/// arithmetic would be one copy too many.
fn wavy_grid_cells(size_mm: f64, amplitude_mm: f64, wavelength_mm: f64, edge_mm: f64) -> usize {
    let k = TAU / wavelength_mm.max(1e-9);
    let max_gradient = amplitude_mm.abs() * k;
    let cos_min = 1.0 / (1.0 + max_gradient * max_gradient).sqrt();
    let cell = edge_mm * cos_min / SQRT_2;
    ((size_mm / cell.max(1e-9)).ceil() as usize).max(2)
}

/// A gently undulating heightfield on a regular grid:
/// `z = A · sin(2πx/L) · sin(2πy/L)`, centred on the origin.
///
/// # Slope arithmetic (the ≤ 30° bound, stated as the brief asks)
///
/// With `k = 2π/L`,
///
/// ```text
/// ∂z/∂x = A·k·cos(kx)·sin(ky)
/// ∂z/∂y = A·k·sin(kx)·cos(ky)
/// |∇z|² = (A·k)²·[cos²(kx)sin²(ky) + sin²(kx)cos²(ky)]
///       = (A·k)²·[u + v − 2uv]   with u = sin²(kx), v = sin²(ky)
/// ```
///
/// `u + v − 2uv` is linear in each variable, so its maximum over `[0,1]²` is
/// at a corner and equals **1**. Hence `|∇z|_max = A·k` exactly, and at
/// `A = 0.5, L = 8` that is `0.5 · 2π/8 = 0.39270`, i.e. a maximum slope of
/// **atan(0.39270) = 21.44°** — inside the ~30° the brief asks for, with
/// margin, and far from the 90° where `n_z` would stop being positive.
///
/// # Concavity arithmetic (the bound the brief does NOT ask for, and should)
///
/// The spiral's coverage law is a **ball of radius `K_c` rolling on the
/// surface**. In a concave pocket tighter than the ball, the ball bridges and
/// the `S^h` points at the bottom become unreachable **at any spacing** — the
/// arm would stall, which on an analytic arm is a hard failure. At a critical
/// point of this surface both principal curvatures are `A·k²`, so
///
/// ```text
/// κ_max = A·k² = 0.5 · (2π/8)² = 0.30843 /mm   ⇒   R_min = 3.242 mm
/// ```
///
/// against `K_c = 1.0`. The smoke test asserts `R_min ≥ 2·K_c`, so a future
/// parameter tweak that would make the fixture unmachinable fails loudly
/// instead of arriving as a mysterious `RingSearchStalled`.
///
/// # Structure
///
/// `n = ` [`wavy_grid_cells`] `= 133` cells per side at the pinned dials, so
/// `cell = 14/133 = 0.10526 mm`, **35 378** triangles and 17 956 vertices.
/// Each cell `(row, col)` contributes triangles `2·(row·n + col)` and `+1`,
/// wound `[a, b, d]` and `[a, d, c]` — the same winding
/// [`clean_grid_mesh`] uses, counter-clockwise in XY, so `n_z > 0` throughout.
fn wavy_patch_mesh(
    size_mm: f64,
    amplitude_mm: f64,
    wavelength_mm: f64,
    edge_mm: f64,
) -> TriangleMesh {
    let n = wavy_grid_cells(size_mm, amplitude_mm, wavelength_mm, edge_mm);
    let cell = size_mm / (n as f64);
    let k = TAU / wavelength_mm.max(1e-9);
    let half = 0.5 * size_mm;

    let mut verts: Vec<P3> = Vec::with_capacity((n + 1) * (n + 1));
    for row in 0..=n {
        let y = -half + cell * (row as f64);
        for col in 0..=n {
            let x = -half + cell * (col as f64);
            verts.push(P3::new(x, y, amplitude_mm * (k * x).sin() * (k * y).sin()));
        }
    }

    let idx = |row: usize, col: usize| -> u32 { (row * (n + 1) + col) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(2 * n * n);
    for row in 0..n {
        for col in 0..n {
            let (a, b) = (idx(row, col), idx(row, col + 1));
            let (c, d) = (idx(row + 1, col), idx(row + 1, col + 1));
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// The wavy arm's region: **whole grid cells** whose centre lies inside the
/// machined circle.
///
/// Whole cells, not a per-triangle centroid test, and that is the point. A
/// centroid test splits cells on the fringe, and two diagonally-adjacent
/// surviving triangles meet at a single vertex — the bowtie
/// `region_topology` refuses as a `BoundaryPinch`, and exactly the class of
/// **selection artefact** the withdrawal is about. Taking cells whole makes
/// the region a polyomino digitisation of a disk: 4-connected, corner-free,
/// a manifold disk by construction. [`clean_selection`] is still run over it
/// in the evidence path and asserted to be a **no-op**, which is the evidence
/// that this fixture needs no hygiene at all.
///
/// The price, stated so it is not misread as a defect: the region boundary is
/// a **staircase**, so `boundary_loop_length_mm` reads roughly `4/π ≈ 1.27×`
/// the smooth circumference of the same circle. At a 0.105 mm cell the
/// staircase amplitude is ~4.6× below the 0.486 mm being measured, so it
/// cannot do what terrain's 4.6 mm boundary segments did.
fn wavy_region_triangles(size_mm: f64, cells: usize, radius_mm: f64) -> Vec<u32> {
    let cell = size_mm / (cells.max(1) as f64);
    let half = 0.5 * size_mm;
    let r2 = radius_mm * radius_mm;
    let mut out: Vec<u32> = Vec::new();
    for row in 0..cells {
        let y = -half + cell * (row as f64 + 0.5);
        for col in 0..cells {
            let x = -half + cell * (col as f64 + 0.5);
            if x * x + y * y <= r2 {
                let base = 2 * (row * cells + col);
                out.push(base as u32);
                out.push((base + 1) as u32);
            }
        }
    }
    out
}

// ---- the analytic fixtures' own acceptance census ----------------------

/// What an analytic region actually is, measured rather than asserted.
///
/// This is the census the withdrawal says was never taken on terrain. Every
/// row here is a **precondition of the measurement**, not a nice-to-have:
/// `edge_max_mm` against [`ANALYTIC_MAX_EDGE_MM`] is what makes a 0.486 mm
/// spacing resolvable at all, and `boundary_loops` / `euler` are what
/// `plan_spiral` will refuse on if they are wrong.
struct MeshCensus {
    triangles: usize,
    vertices: usize,
    edges: usize,
    boundary_edges: usize,
    boundary_loops: usize,
    euler: i64,
    edge_min_mm: f64,
    edge_median_mm: f64,
    edge_max_mm: f64,
    min_normal_z: f64,
    area_mm2: f64,
}

/// Count connected components of the boundary-edge graph — the loop count.
fn boundary_loop_count(boundary_edges: &[(u32, u32)]) -> usize {
    let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(a, b) in boundary_edges {
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }
    let mut vertices: Vec<u32> = adjacency.keys().copied().collect();
    vertices.sort_unstable();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut loops = 0usize;
    for start in vertices {
        if !seen.insert(start) {
            continue;
        }
        loops += 1;
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            let Some(neighbours) = adjacency.get(&v) else {
                continue;
            };
            for &w in neighbours {
                if seen.insert(w) {
                    stack.push(w);
                }
            }
        }
    }
    loops
}

/// Census a triangle selection: all three 3D edges of every triangle, the
/// edge-use bookkeeping, the boundary-loop count, Euler characteristic, the
/// worst (smallest) face `normal.z`, and the 3D area.
fn mesh_census(mesh: &TriangleMesh, region: &[u32]) -> MeshCensus {
    let mut used: HashMap<(u32, u32), usize> = HashMap::new();
    let mut region_vertices: HashSet<u32> = HashSet::new();
    let mut lengths: Vec<f64> = Vec::with_capacity(region.len() * 3);
    let mut min_normal_z = f64::INFINITY;
    let mut area = 0.0f64;

    for &t in region {
        let Some(tri) = mesh.triangles.get(t as usize) else {
            continue;
        };
        let Some(face) = mesh.faces.get(t as usize) else {
            continue;
        };
        min_normal_z = min_normal_z.min(face.normal.z);
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        area += 0.5 * e1.cross(&e2).norm();
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            region_vertices.insert(a);
            region_vertices.insert(b);
            *used.entry(edge_key(a, b)).or_insert(0) += 1;
            let (Some(pa), Some(pb)) =
                (mesh.vertices.get(a as usize), mesh.vertices.get(b as usize))
            else {
                continue;
            };
            lengths.push((*pb - *pa).norm());
        }
    }

    // `filter_map` on the by-value `(&K, &V)` item, NOT `filter` on `&Item`:
    // under Rust 2024's match-ergonomics rules an `&count` pattern inside a
    // pattern whose default binding mode has become `ref` — which is exactly
    // what `filter`'s `&(&K, &V)` argument produces — is an error.
    let boundary: Vec<(u32, u32)> = used
        .iter()
        .filter_map(|(&edge, &count)| (count == 1).then_some(edge))
        .collect();
    lengths.sort_by(f64::total_cmp);
    let faces = region.len();
    let edges = used.len();
    let vertices = region_vertices.len();

    MeshCensus {
        triangles: faces,
        vertices,
        edges,
        boundary_edges: boundary.len(),
        boundary_loops: boundary_loop_count(&boundary),
        euler: vertices as i64 - edges as i64 + faces as i64,
        edge_min_mm: percentile(&lengths, 0.0),
        edge_median_mm: percentile(&lengths, 0.50),
        edge_max_mm: percentile(&lengths, 1.0),
        min_normal_z: if min_normal_z.is_finite() {
            min_normal_z
        } else {
            f64::NAN
        },
        area_mm2: area,
    }
}

/// Print the census, with the one bar that makes the arm's numbers quotable
/// evaluated **in the output**.
fn print_mesh_census(label: &str, census: &MeshCensus, stepover_mm: f64) {
    let ratio = stepover_mm / census.edge_median_mm;
    eprintln!("   -- FIXTURE RESOLUTION CENSUS — {label} --");
    eprintln!(
        "     triangles / vertices / edges  {:>10} / {} / {}",
        census.triangles, census.vertices, census.edges
    );
    eprintln!(
        "     boundary edges / LOOPS        {:>10} / {}   (a disk has exactly 1 loop)",
        census.boundary_edges, census.boundary_loops
    );
    eprintln!(
        "     Euler characteristic          {:>10}   (a disk is 1)",
        census.euler
    );
    eprintln!(
        "     region 3D area (mm²)          {:>10.3}",
        census.area_mm2
    );
    eprintln!(
        "     min face normal.z             {:>10.6}   (> 0 = every face up-facing)",
        census.min_normal_z
    );
    eprintln!(
        "     3D EDGE min / median / max mm {:>10.5} / {:.5} / {:.5}",
        census.edge_min_mm, census.edge_median_mm, census.edge_max_mm
    );
    eprintln!(
        "     stepover / MEDIAN edge        {ratio:>10.2}   <<< THE BAR THE TERRAIN ARMS FAILED"
    );
    eprintln!(
        "     max edge <= {ANALYTIC_MAX_EDGE_MM:.3} mm?         {:>10}   (bar: edge <= stepover/3 = \
         {:.5} mm)",
        census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM,
        stepover_mm / 3.0
    );
    eprintln!(
        "     For scale: terrain_small's chosen FLAT window measured a 1.42 mm median edge\n\
         \x20    against this same 0.4862 mm stepover — a ratio of 0.34, i.e. the facets were\n\
         \x20    ~3x the measurand. That is the whole content of the FINDINGS_F2 withdrawal.\n"
    );
}

// ── the region, and its selection ───────────────────────────────────────

/// An ellipse polygon, `n` vertices, counter-clockwise from `θ = 0`.
///
/// **[REPO].** PROGRAMME.md §F2 step 1 asks for "a hole-free region polygon"
/// on this fixture and says nothing more; an ellipse inscribed in the inset
/// bounding box is the simplest simply-connected, convex, hole-free choice
/// whose every point is far from the mesh edge.
fn ellipse_polygon(cx: f64, cy: f64, ax: f64, ay: f64, n: usize) -> Polygon2 {
    let n = n.max(3);
    let ring: Vec<P2> = (0..n)
        .map(|k| {
            let t = TAU * (k as f64) / (n as f64);
            P2::new(cx + ax * t.cos(), cy + ay * t.sin())
        })
        .collect();
    Polygon2::new(ring)
}

/// Total 3D area (mm²) of the selected triangles. Feeds the `N_S` adequacy
/// arithmetic in Stage A.
fn region_area_mm2(mesh: &TriangleMesh, region: &[u32]) -> f64 {
    region
        .iter()
        .filter_map(|&t| mesh.faces.get(t as usize))
        .map(|f| {
            let e1 = f.v[1] - f.v[0];
            let e2 = f.v[2] - f.v[0];
            0.5 * e1.cross(&e2).norm()
        })
        .sum()
}

/// A discrete curvature census over the region's interior edges.
///
/// **[REPO] instrument arithmetic.** `scallop_math::stepover_from_scallop_curved`
/// wants a signed mean curvature (1/mm, positive convex); nothing in the core
/// publishes one for a mesh region, and this file may not touch `src/`. The
/// estimator is the standard discrete one: for an edge shared by two selected
/// triangles with unit normals `n₁, n₂` and centroids `c₁, c₂`,
///
/// ```text
/// |κ| = angle(n₁, n₂) / ‖c₂ − c₁‖ ,   sign = sign((c₂ − c₁)·(n₂ − n₁))
/// ```
///
/// The sign convention is checked against a sphere: with outward normals
/// `n = p/R`, `n₂ − n₁ = (c₂ − c₁)/R`, so the dot product is `‖c₂ − c₁‖²/R > 0`
/// — convex is positive, which is what `stepover_from_scallop_curved` expects.
///
/// Normals are **+Z-forced** before use, matching `build_region_mesh`'s
/// convention (which `conformal_spiral`, `crest_lines` and `direction_field`
/// all inherit, and which that module lists as a stated limitation).
struct CurvatureCensus {
    edges: usize,
    convex_fraction: f64,
    abs_sorted: Vec<f64>,
}

impl CurvatureCensus {
    fn median(&self) -> f64 {
        percentile(&self.abs_sorted, 0.50)
    }
    fn p90(&self) -> f64 {
        percentile(&self.abs_sorted, 0.90)
    }
}

fn curvature_census(mesh: &TriangleMesh, region: &[u32]) -> CurvatureCensus {
    let mut shared: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for &t in region {
        let Some(tri) = mesh.triangles.get(t as usize) else {
            continue;
        };
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a <= b { (a, b) } else { (b, a) };
            shared.entry(key).or_default().push(t as usize);
        }
    }

    let mut abs_sorted: Vec<f64> = Vec::new();
    let mut convex = 0usize;
    let mut edges = 0usize;
    for owners in shared.values() {
        if owners.len() != 2 {
            continue;
        }
        let (Some(f1), Some(f2)) = (mesh.faces.get(owners[0]), mesh.faces.get(owners[1])) else {
            continue;
        };
        let up = |n: rs_cam_core::geo::V3| if n.z < 0.0 { -n } else { n };
        let (n1, n2) = (up(f1.normal), up(f2.normal));
        let c1 = (f1.v[0].coords + f1.v[1].coords + f1.v[2].coords) / 3.0;
        let c2 = (f2.v[0].coords + f2.v[1].coords + f2.v[2].coords) / 3.0;
        let d = c2 - c1;
        let span = d.norm();
        if span < 1e-9 {
            continue;
        }
        let angle = n1.dot(&n2).clamp(-1.0, 1.0).acos();
        let kappa = angle / span;
        if !kappa.is_finite() {
            continue;
        }
        edges += 1;
        if d.dot(&(n2 - n1)) >= 0.0 {
            convex += 1;
        }
        abs_sorted.push(kappa);
    }
    abs_sorted.sort_by(f64::total_cmp);
    let convex_fraction = if edges == 0 {
        0.0
    } else {
        convex as f64 / edges as f64
    };
    CurvatureCensus {
        edges,
        convex_fraction,
        abs_sorted,
    }
}

/// The region polygon plus the triangles selected by centroid containment,
/// after any topology-driven shrink.
struct Region {
    /// The ellipse. Note this stays the **containment and drawing**
    /// reference even though `triangles` is a cleaned subset of what it
    /// selected: the ellipse is what the escape count in Stage D is measured
    /// against and what Stage B draws, so a cleanup that shaved a fringe
    /// triangle makes the drawn outline slightly generous, never tight.
    polygon: Polygon2,
    /// The triangles actually planned on — post-cleanup.
    triangles: Vec<u32>,
    /// Semi-axes actually used (mm).
    semi_axes: (f64, f64),
    /// How many 2 mm shrinks were needed.
    shrinks: usize,
    /// What selection hygiene did to get from the raw centroid selection to
    /// `triangles`.
    cleanup: CleanupReport,
}

/// Where one arm's ellipse sits and how big it starts.
#[derive(Clone, Copy)]
struct EllipseSpec {
    cx: f64,
    cy: f64,
    ax: f64,
    ay: f64,
}

impl EllipseSpec {
    /// The whole-region arm: the mesh's XY bounding box, inset.
    fn steep(mesh: &TriangleMesh) -> Self {
        let bb = &mesh.bbox;
        Self {
            cx: 0.5 * (bb.min.x + bb.max.x),
            cy: 0.5 * (bb.min.y + bb.max.y),
            ax: 0.5 * (bb.max.x - bb.min.x) - REGION_INSET_MM,
            ay: 0.5 * (bb.max.y - bb.min.y) - REGION_INSET_MM,
        }
    }

    /// Is `(x, y)` inside? Analytic, so the census never builds a polygon.
    fn contains(&self, x: f64, y: f64) -> bool {
        let (dx, dy) = ((x - self.cx) / self.ax, (y - self.cy) / self.ay);
        dx * dx + dy * dy <= 1.0
    }
}

/// One planned arm. The report is **always** present — that is the whole
/// point of the module's new `(report, outcome)` shape — so a refusal can be
/// attributed instead of vanishing with the error.
struct Arm {
    label: &'static str,
    region: Region,
    report: SpiralReport,
    outcome: Result<SpiralResult, SpiralRefusal>,
}

/// Build the region for one arm and plan on it, shrinking the ellipse on a
/// topology refusal.
///
/// **Spec resolution (2026-08-30).** The brief named `NotSimplyConnected` /
/// `NotADisk` as the shrink triggers. `NonManifoldBoundary` is added: a
/// centroid selection over ragged terrain triangles produces a boundary pinch
/// (two outgoing boundary half-edges at one vertex) exactly as readily as it
/// produces a second loop, and both are the same "the fringe is ragged"
/// symptom. Every other refusal arm — `EmptyRegion`, `FlattenDidNotConverge`,
/// `NoSurfaceSamples`, `RingSearchStalled`, `RingLimitReached` — is a
/// *mechanism or geometry* finding that a smaller ellipse would only hide, so
/// those are **returned with their report** rather than retried. Under the
/// old `Result`-only signature that path could only return `None`; it now
/// carries the evidence out, which is what makes the STEEP arm a result
/// instead of a dead end.
///
/// `None` means no region could be produced at all — an empty selection, or
/// every shrink refused on topology.
fn plan_arm(
    label: &'static str,
    spec: EllipseSpec,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    params: &SpiralParams,
) -> Option<Arm> {
    let (cx, cy) = (spec.cx, spec.cy);

    for shrink in 0..=MAX_SHRINKS {
        let shrink_mm = shrink as f64 * SHRINK_STEP_MM;
        let (ax, ay) = (spec.ax - shrink_mm, spec.ay - shrink_mm);
        if ax <= 0.0 || ay <= 0.0 {
            eprintln!("   REFUSE: ellipse shrank to nothing at step {shrink}.");
            return None;
        }
        let polygon = ellipse_polygon(cx, cy, ax, ay, ELLIPSE_VERTICES);
        let raw = conformal_spiral_region(mesh, &polygon);
        eprintln!(
            "   region try {shrink}: ellipse centre ({cx:.3}, {cy:.3}), semi-axes \
             ({ax:.3}, {ay:.3}) mm, {ELLIPSE_VERTICES} vertices, \
             polygon area {:.1} mm², {} triangles selected raw \
             (centroid inside AND normal.z > {MIN_UP_NORMAL_Z})",
            polygon.area(),
            raw.len()
        );
        if raw.is_empty() {
            eprintln!("     no triangle centroid inside — shrinking.");
            continue;
        }
        let (triangles, cleanup) = clean_selection(mesh, &raw);
        eprintln!(
            "     cleanup: {} -> {} triangles ({} dropped, {:.2}%); \
             components on first pass {}, components dropped {}; \
             pinch passes {}{}, pinch vertices shaved {}, over-used edges {}, \
             triangles shaved {}",
            cleanup.before,
            cleanup.after,
            cleanup.before.saturating_sub(cleanup.after),
            100.0 * (cleanup.before.saturating_sub(cleanup.after)) as f64
                / cleanup.before.max(1) as f64,
            cleanup.components_first_pass,
            cleanup.components_dropped,
            cleanup.pinch_iterations,
            if cleanup.hit_iteration_cap {
                " (HIT THE CAP — the selection was still non-manifold when the loop stopped)"
            } else {
                ""
            },
            cleanup.pinch_vertices_shaved,
            cleanup.over_used_edges,
            cleanup.triangles_shaved_by_pinch
        );
        if triangles.is_empty() {
            eprintln!("     cleanup consumed the whole selection — shrinking.");
            continue;
        }
        // The module returns the report FIRST and unconditionally; the
        // outcome is second. Everything measured before a refusal survives in
        // that report, which is what `diagnose_refusal` reads.
        let (report, outcome) = conformal_spiral::plan_spiral(mesh, index, &triangles, params);
        let retryable = match &outcome {
            Ok(_) => false,
            Err(refusal) => {
                eprintln!("     REFUSAL: {refusal:?}");
                matches!(
                    refusal,
                    SpiralRefusal::NotSimplyConnected { .. }
                        | SpiralRefusal::NotADisk { .. }
                        | SpiralRefusal::NonManifoldBoundary { .. }
                )
            }
        };
        if retryable {
            eprintln!("     topology refusal — shrinking by {SHRINK_STEP_MM} mm and retrying.");
            continue;
        }
        return Some(Arm {
            label,
            region: Region {
                polygon,
                triangles,
                semi_axes: (ax, ay),
                shrinks: shrink,
                cleanup,
            },
            report,
            outcome,
        });
    }
    eprintln!("   REFUSE: {MAX_SHRINKS} shrinks exhausted without a disk-topology region.");
    None
}

// ── the FLAT arm's region: a low-relief window, found by census ─────────

/// Candidate-centre lattice for the flat-window search.
const FLAT_SEARCH_COLS: usize = 10;
/// See [`FLAT_SEARCH_COLS`].
const FLAT_SEARCH_ROWS: usize = 8;

/// Flat-arm ellipse semi-axes (mm) — a 24 × 18 mm window, small enough that a
/// terrain of this relief plausibly contains a quiet patch of that size, big
/// enough that the equal-cusp stepover still lays down ~10² rings.
const FLAT_SEMI_AXIS_X_MM: f64 = 12.0;
/// See [`FLAT_SEMI_AXIS_X_MM`].
const FLAT_SEMI_AXIS_Y_MM: f64 = 9.0;

/// A candidate window is only considered if it holds at least this many
/// up-facing triangles. Without a floor the flattest window would be whichever
/// one happens to hold two triangles — the "gate handed an empty population"
/// failure, in census form.
const FLAT_MIN_TRIANGLES: usize = 200;

/// The relief the FLAT arm aims for. **Not a bar**: the census reports the
/// flattest window it actually found and says how that compares, because
/// whether this fixture contains an 8 mm-relief 24 × 18 window is a fact about
/// the fixture, not something to assert.
const FLAT_RELIEF_TARGET_MM: f64 = 8.0;

/// What the flat-window census found.
struct FlatCensus {
    /// The chosen window, or `None` when no candidate met
    /// [`FLAT_MIN_TRIANGLES`].
    best: Option<EllipseSpec>,
    best_relief_mm: f64,
    best_triangles: usize,
    /// Candidates that met the triangle floor.
    qualifying: usize,
    /// Candidate centres evaluated in total.
    evaluated: usize,
    relief_min_mm: f64,
    relief_median_mm: f64,
    relief_max_mm: f64,
}

/// Census the mesh's up-facing triangles for Z relief and pick the flattest
/// window the FLAT arm can sit in.
///
/// **[REPO], and the shape matters.** The relief is measured over the
/// **candidate ellipse's own footprint**, not over a grid cell: a 10 × 8 cell
/// on this fixture is ~10 × 9 mm, while the flat ellipse spans 24 × 18 mm, so
/// a per-cell relief would systematically understate what the arm actually
/// machines. Evaluating the real footprint costs `COLS × ROWS × triangles`
/// analytic point-in-ellipse tests — ~3 × 10⁶ here, milliseconds — and is
/// exact rather than approximate.
///
/// Relief is `max − min` of up-facing triangle **centroid** Z, which is the
/// same quantity the region selection tests, so the census and the selection
/// cannot disagree about which triangles they mean.
///
/// Candidate centres are constrained so the window stays [`REGION_INSET_MM`]
/// inside the mesh bounding box — the same clearance the STEEP arm has, so
/// the two arms differ in relief and size, not in edge proximity.
///
/// Deterministic: centres are scanned row-major and the comparison is
/// strictly-less, so the first (lowest row, then lowest column) of any tie
/// wins.
fn flattest_window(mesh: &TriangleMesh) -> FlatCensus {
    let mut up: Vec<(f64, f64, f64)> = Vec::new();
    for face in &mesh.faces {
        if face.normal.z <= MIN_UP_NORMAL_Z {
            continue;
        }
        let c = (face.v[0].coords + face.v[1].coords + face.v[2].coords) / 3.0;
        up.push((c.x, c.y, c.z));
    }

    let bb = &mesh.bbox;
    let (ax, ay) = (FLAT_SEMI_AXIS_X_MM, FLAT_SEMI_AXIS_Y_MM);
    let (x_lo, x_hi) = (
        bb.min.x + REGION_INSET_MM + ax,
        bb.max.x - REGION_INSET_MM - ax,
    );
    let (y_lo, y_hi) = (
        bb.min.y + REGION_INSET_MM + ay,
        bb.max.y - REGION_INSET_MM - ay,
    );

    let mut census = FlatCensus {
        best: None,
        best_relief_mm: f64::INFINITY,
        best_triangles: 0,
        qualifying: 0,
        evaluated: 0,
        relief_min_mm: f64::NAN,
        relief_median_mm: f64::NAN,
        relief_max_mm: f64::NAN,
    };
    if x_hi < x_lo || y_hi < y_lo || up.is_empty() {
        return census;
    }

    let step = |lo: f64, hi: f64, n: usize| -> f64 {
        if n <= 1 {
            0.0
        } else {
            (hi - lo) / (n - 1) as f64
        }
    };
    let dx = step(x_lo, x_hi, FLAT_SEARCH_COLS);
    let dy = step(y_lo, y_hi, FLAT_SEARCH_ROWS);

    let mut reliefs: Vec<f64> = Vec::new();
    for row in 0..FLAT_SEARCH_ROWS {
        for col in 0..FLAT_SEARCH_COLS {
            let spec = EllipseSpec {
                cx: x_lo + dx * col as f64,
                cy: y_lo + dy * row as f64,
                ax,
                ay,
            };
            census.evaluated += 1;
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut count = 0usize;
            for &(x, y, z) in &up {
                if spec.contains(x, y) {
                    count += 1;
                    lo = lo.min(z);
                    hi = hi.max(z);
                }
            }
            if count < FLAT_MIN_TRIANGLES {
                continue;
            }
            census.qualifying += 1;
            let relief = hi - lo;
            reliefs.push(relief);
            if relief < census.best_relief_mm {
                census.best_relief_mm = relief;
                census.best_triangles = count;
                census.best = Some(spec);
            }
        }
    }

    reliefs.sort_by(f64::total_cmp);
    census.relief_min_mm = percentile(&reliefs, 0.0);
    census.relief_median_mm = percentile(&reliefs, 0.50);
    census.relief_max_mm = percentile(&reliefs, 1.0);
    census
}

/// Strictly-positive normal Z is the up-facing test.
///
/// **Moved from 0.1 to 0.0 on 2026-08-30, deliberately.** The two conditions
/// are not interchangeable, and which one is right is decided by *where* each
/// one cuts:
///
/// * `> 0.0` excludes exactly the floor plate and any underside — surfaces
///   whose outward normal points down. Every triangle it removes is on a
///   sheet the selection must not have, and it removes nothing from the top
///   sheet, because a heightfield's triangles all have strictly positive
///   normal Z.
/// * `> 0.1` additionally excludes up-facing terrain steeper than ~84°.
///   Those triangles sit in the **interior** of the region, so removing them
///   punches holes in the selected patch — and a hole is a second boundary
///   loop, which is the `NotSimplyConnected` refusal this filter exists to
///   avoid, reintroduced from the other side. It also multiplies the ragged
///   fringe that produces boundary pinches.
///
/// The 0.1 band was chosen to drop the vertical skirt walls, but the skirt
/// lives on the mesh's XY perimeter and the region ellipse is 8 mm inside it,
/// so no skirt triangle was ever selectable. The threshold was buying
/// nothing and costing interior connectivity.
const MIN_UP_NORMAL_Z: f64 = 0.0;

/// Triangles whose centroid is inside the region polygon AND whose normal
/// faces up. `Polygon2::contains_point` honours holes; this polygon has none.
///
/// The up-facing filter is load-bearing, discovered on the first run:
/// `fixtures/terrain_small.stl` is a CLOSED solid (terrain top + skirt +
/// floor), so a pure XY centroid test selects the top surface AND the floor
/// plate directly beneath it — two disconnected patches, which
/// `region_topology` correctly refuses as `NotSimplyConnected { 2 }`. The
/// machinable surface of a 3-axis terrain job is the up-facing sheet, so the
/// filter matches the machining semantics, not just the topology check. See
/// [`MIN_UP_NORMAL_Z`] for why the threshold is 0.0 and not 0.1.
///
/// This is a **raw** selection: it is per-triangle and therefore says nothing
/// about connectivity or manifoldness. [`clean_selection`] is what makes it a
/// candidate disk.
fn conformal_spiral_region(mesh: &TriangleMesh, polygon: &Polygon2) -> Vec<u32> {
    rs_cam_core::direction_field::triangles_where(mesh, |i, centroid| {
        mesh.faces
            .get(i)
            .is_some_and(|f| f.normal.z > MIN_UP_NORMAL_Z)
            && polygon.contains_point(&P2::new(centroid.x, centroid.y))
    })
}

// ── selection hygiene: [REPO], test-local, no `src/` change ─────────────

/// Iteration cap on the pinch-shaving loop. Each pass removes at least one
/// triangle, so this only bounds a pathological selection; the count is
/// reported so a run that hits it is visible rather than silently truncated.
const MAX_PINCH_ITERATIONS: usize = 20;

/// Canonical undirected edge key.
fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a <= b { (a, b) } else { (b, a) }
}

/// What [`clean_selection`] did, so the run can be read rather than trusted.
#[derive(Debug, Default, Clone, Copy)]
struct CleanupReport {
    before: usize,
    after: usize,
    /// Edge-connected components found on the FIRST pass.
    components_first_pass: usize,
    /// Components discarded across every pass.
    components_dropped: usize,
    /// Pinch-shaving passes actually run.
    pinch_iterations: usize,
    /// Boundary-pinch vertices shaved, summed over passes.
    pinch_vertices_shaved: usize,
    /// Over-used (three-or-more-triangle) edges seen, summed over passes.
    /// Counted separately because it is an EDGE count, not a vertex count —
    /// the two must not be added together under one name.
    over_used_edges: usize,
    /// Triangles removed by shaving (as opposed to component pruning).
    triangles_shaved_by_pinch: usize,
    /// True when the loop stopped on [`MAX_PINCH_ITERATIONS`] rather than on
    /// a clean selection.
    hit_iteration_cap: bool,
}

/// Keep only the largest **edge**-connected component of `selected`.
///
/// Edge adjacency, not vertex adjacency: two triangles that meet at a single
/// vertex are exactly the bowtie `region_topology` refuses as a
/// `BoundaryPinch`, so treating them as connected would defeat the purpose.
///
/// Returns the kept triangles (ascending, as `selected` was) and the number
/// of components seen. Deterministic: components are discovered by scanning
/// `selected` in order, and the largest wins with the lowest component id
/// breaking a tie — neither depends on `HashMap` iteration order.
fn largest_edge_component(mesh: &TriangleMesh, selected: &[u32]) -> (Vec<u32>, usize) {
    let n = selected.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, &t) in selected.iter().enumerate() {
        let Some(tri) = mesh.triangles.get(t as usize) else {
            continue;
        };
        for k in 0..3 {
            by_edge
                .entry(edge_key(tri[k], tri[(k + 1) % 3]))
                .or_default()
                .push(i);
        }
    }

    let mut component: Vec<Option<usize>> = vec![None; n];
    let mut sizes: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if component[start].is_some() {
            continue;
        }
        let id = sizes.len();
        component[start] = Some(id);
        stack.push(start);
        let mut size = 0usize;
        while let Some(i) = stack.pop() {
            size += 1;
            let Some(&t) = selected.get(i) else { continue };
            let Some(tri) = mesh.triangles.get(t as usize) else {
                continue;
            };
            for k in 0..3 {
                let Some(list) = by_edge.get(&edge_key(tri[k], tri[(k + 1) % 3])) else {
                    continue;
                };
                for &j in list {
                    if component[j].is_none() {
                        component[j] = Some(id);
                        stack.push(j);
                    }
                }
            }
        }
        sizes.push(size);
    }

    // Largest size wins; STRICTLY greater, so the lowest component id breaks
    // a tie — and component ids are assigned by scanning `selected` in order,
    // which is why nothing here depends on `HashMap` iteration order.
    let mut best = 0usize;
    let mut best_size = 0usize;
    for (id, &size) in sizes.iter().enumerate() {
        if size > best_size {
            best_size = size;
            best = id;
        }
    }
    let mut kept: Vec<u32> = Vec::with_capacity(best_size);
    for (i, &t) in selected.iter().enumerate() {
        if component.get(i).copied().flatten() == Some(best) {
            kept.push(t);
        }
    }
    (kept, sizes.len())
}

/// Vertices of `selected` that make its boundary non-manifold.
///
/// The edge bookkeeping is built the way `conformal_spiral::region_topology`
/// builds it, on the mesh's **global** vertex ids: `build_region_mesh` maps
/// global ids to local ones without reordering a triangle's corners, so the
/// winding — and therefore every directed half-edge — is identical.
///
/// * an undirected edge used **once** is a boundary edge; the directed
///   half-edge `(a, b)` sitting on one is an **outgoing boundary half-edge**
///   of `a`. A vertex with more than one of those is a `BoundaryPinch`;
/// * an undirected edge used **three or more** times is `EdgeOverUsed`. That
///   is the other arm of the same `NonManifoldBoundary` refusal, so both its
///   endpoints go into the same shave set — folded in here because it costs
///   nothing and leaving it out would let the cleanup declare success on a
///   selection the module still refuses.
///
/// Returns `(vertices to shave, pinch vertices, over-used edges)`. The set is
/// a `BTreeSet`, so the shaving order is deterministic.
fn nonmanifold_vertices(mesh: &TriangleMesh, selected: &[u32]) -> (BTreeSet<u32>, usize, usize) {
    let mut undirected: HashMap<(u32, u32), usize> = HashMap::new();
    let mut directed: HashSet<(u32, u32)> = HashSet::new();
    for &t in selected {
        let Some(tri) = mesh.triangles.get(t as usize) else {
            continue;
        };
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if a == b {
                continue;
            }
            *undirected.entry(edge_key(a, b)).or_insert(0) += 1;
            directed.insert((a, b));
        }
    }

    let mut out_degree: HashMap<u32, usize> = HashMap::new();
    for &(a, b) in &directed {
        if undirected.get(&edge_key(a, b)).copied().unwrap_or(0) == 1 {
            *out_degree.entry(a).or_insert(0) += 1;
        }
    }
    let mut bad: BTreeSet<u32> = BTreeSet::new();
    for (&v, &degree) in &out_degree {
        if degree > 1 {
            bad.insert(v);
        }
    }
    let pinches = bad.len();

    let mut over_used = 0usize;
    for (&(a, b), &count) in &undirected {
        if count > 2 {
            over_used += 1;
            bad.insert(a);
            bad.insert(b);
        }
    }
    (bad, pinches, over_used)
}

/// Turn a raw per-triangle selection into something `plan_spiral` can be
/// asked about honestly.
///
/// **[REPO], test-local, and deliberately NOT a `src/` change.** The module's
/// refusals stay authoritative: this pass does not weaken a single check, it
/// only stops handing `plan_spiral` a selection whose defects are artefacts
/// of *centroid-in-polygon selection* rather than facts about the surface.
/// Two artefacts, in this order:
///
/// 1. **Islands.** A per-triangle test can strand triangles the ellipse
///    clipped away from the main patch. Only the largest edge-connected
///    component survives.
/// 2. **Bowtie vertices.** A ragged fringe routinely leaves a vertex whose
///    selected triangles form two disjoint fans — a `BoundaryPinch`. Every
///    selected triangle incident to such a vertex is removed. Shaving can
///    orphan new pinches (and new islands), so step 1 is re-run and the whole
///    thing loops to a fixed point, capped at [`MAX_PINCH_ITERATIONS`].
///
/// What it does NOT do: it never adds a triangle, never re-triangulates, and
/// never touches a hole. If the cleaned patch is still not a disk —
/// `NotSimplyConnected` from a genuine interior hole, say — `plan_spiral`
/// refuses and that refusal is a fact about the region, which is the point.
fn clean_selection(mesh: &TriangleMesh, selected: &[u32]) -> (Vec<u32>, CleanupReport) {
    let mut report = CleanupReport {
        before: selected.len(),
        ..CleanupReport::default()
    };
    let (mut current, components) = largest_edge_component(mesh, selected);
    report.components_first_pass = components;
    report.components_dropped += components.saturating_sub(1);

    loop {
        let (bad, pinches, over_used) = nonmanifold_vertices(mesh, &current);
        if bad.is_empty() {
            break;
        }
        if report.pinch_iterations >= MAX_PINCH_ITERATIONS {
            report.hit_iteration_cap = true;
            break;
        }
        report.pinch_iterations += 1;
        report.pinch_vertices_shaved += pinches;
        report.over_used_edges += over_used;

        let before = current.len();
        current.retain(|&t| {
            mesh.triangles
                .get(t as usize)
                .is_some_and(|tri| !tri.iter().any(|v| bad.contains(v)))
        });
        report.triangles_shaved_by_pinch += before - current.len();
        if current.is_empty() {
            break;
        }
        // Shaving can disconnect the patch; re-prune before re-testing.
        let (kept, components) = largest_edge_component(mesh, &current);
        report.components_dropped += components.saturating_sub(1);
        current = kept;
    }

    report.after = current.len();
    (current, report)
}

/// What an arm's region turned out to be. Shared by both arms so the two are
/// described in identical terms.
fn print_region_acceptance(region: &Region) {
    eprintln!(
        "   region ACCEPTED after {} shrink(s): semi-axes ({:.3}, {:.3}) mm, \
         {} triangles after cleanup (from {} raw), polygon area {:.1} mm²",
        region.shrinks,
        region.semi_axes.0,
        region.semi_axes.1,
        region.triangles.len(),
        region.cleanup.before,
        region.polygon.area()
    );
    eprintln!(
        "   The planned region is the CLEANED triangle set; the ellipse stays the reference the\n\
         \x20  Stage D escape count is measured against and the outline Stage B draws, so that\n\
         \x20  outline is slightly generous where the cleanup shaved a fringe triangle.\n"
    );
}

// ── shared report printers (Stage A AND the refusal diagnosis) ──────────

/// The flatten metrics table.
///
/// Factored out of Stage A because a **refusal** that got past the flattening
/// carries exactly these rows — the module now measures them before anything
/// downstream can refuse, precisely so they survive — and they are what
/// attributes the refusal to a bad map rather than a bad mechanism. Printing
/// two different tables on the two paths would have made the steep arm's
/// numbers unquotable next to the flat arm's.
fn print_flatten_block(report: &SpiralReport) {
    eprintln!("\n   -- flattening: the BAD-MAP vs BAD-MECHANISM table --");
    if report.area_distortion_by_disk_radius.is_empty() && report.flatten_interior_vertices == 0 {
        eprintln!("     NOT REACHED: the pipeline refused before the flattening ran.");
        return;
    }
    eprintln!(
        "     interior vertices (solve size){:>12}",
        report.flatten_interior_vertices
    );
    eprintln!(
        "     Gauss-Seidel sweeps           {:>12}",
        report.flatten_solver_sweeps
    );
    eprintln!(
        "     final sweep delta             {:>12.3e}   ABSOLUTE max coordinate change on the\n\
         \x20                                              last sweep, in UNIT-DISK units — not a\n\
         \x20                                              relative residual. Compare against\n\
         \x20                                              SpiralParams::solver_tolerance, not\n\
         \x20                                              against a norm.",
        report.flatten_solver_delta
    );

    eprintln!("\n     -- Tutte preconditions: MEASURED, not assumed --");
    eprintln!(
        "     mean-value weight min / max   {:>12.6e} / {:.6e}",
        report.mean_value_weight_min, report.mean_value_weight_max
    );
    let weights_ok =
        report.mean_value_weight_nonpositive == 0 && report.mean_value_weight_min > 0.0;
    eprintln!(
        "     NON-POSITIVE weights          {:>12}   {}",
        report.mean_value_weight_nonpositive,
        if weights_ok {
            "MUST be 0 — OK"
        } else {
            "*** MUST be 0 — VIOLATED: the Tutte embedding guarantee does NOT hold, and any \
             flips below are unsurprising rather than a solver bug ***"
        }
    );
    eprintln!(
        "     FLIPPED triangles             {:>12}   {}",
        report.flipped_triangles,
        if report.flipped_triangles == 0 {
            "MUST be 0 — OK"
        } else {
            "*** TRIPWIRE FIRED ***"
        }
    );
    if report.flipped_triangles > 0 {
        let radii = &report.flipped_triangle_disk_radii;
        eprintln!(
            "     fold-zone census: {} flipped-triangle disk radii (ascending) —\n\
             \x20      this says WHERE in the disk the invariant broke, which is what a ring-stall\n\
             \x20      postmortem needs:",
            radii.len()
        );
        // The full list can be long; print min/median/max plus a bounded head
        // so the shape is visible without flooding the log.
        eprintln!(
            "        r min / median / max   {:.6} / {:.6} / {:.6}",
            percentile(radii, 0.0),
            percentile(radii, 0.50),
            percentile(radii, 1.0)
        );
        let head: Vec<String> = radii.iter().take(24).map(|r| format!("{r:.4}")).collect();
        eprintln!(
            "        first {} of {}: {}",
            head.len(),
            radii.len(),
            head.join(", ")
        );
    }

    eprintln!(
        "\n     orientation sign              {:>12.1}",
        report.orientation_sign
    );
    eprintln!(
        "     area distortion min/med/max   {:>12.6e} / {:.6e} / {:.6e}   (1/mm²)",
        report.area_distortion_min, report.area_distortion_median, report.area_distortion_max
    );
    eprintln!(
        "     angle distortion med/max      {:>12.4} / {:.4}   (deg)",
        report.angle_distortion_median_deg, report.angle_distortion_max_deg
    );
    eprintln!(
        "     READ THIS FIRST if anything below disappoints. The flattening is a MEAN-VALUE\n\
         \x20    (Floater) map solved by Gauss-Seidel — a labelled [REPO] substitution for the\n\
         \x20    paper's conformal slit map (PROGRAMME.md §F2 step 1). Every mean-value weight is\n\
         \x20    strictly positive, so Tutte's spring-embedding theorem makes a valid embedding\n\
         \x20    CERTAIN for a manifold disk on a convex boundary: FOLDS ARE STRUCTURALLY\n\
         \x20    IMPOSSIBLE, and a nonzero flip count is a BUG (or an under-converged solve),\n\
         \x20    never bad luck and never a discretisation symptom. That framing is the opposite\n\
         \x20    of the cotangent-Laplacian version this instrument was first written against,\n\
         \x20    where flips WERE an expected symptom and 298 of 9107 were measured on this same\n\
         \x20    fixture — do not carry the old reading forward.\n\
         \x20    What IS expected: mean-value is not the harmonic map, so ANGULAR distortion may\n\
         \x20    be slightly worse than the cotangent version's, and a wide AREA-distortion\n\
         \x20    spread is normal — the paper's sampled coverage check compensates distortion by\n\
         \x20    design. It does not compensate folds at all, which is the whole reason for the\n\
         \x20    trade."
    );
}

/// `bucket[0].median / bucket[last].median`, folded so it reads ≥ 1 whichever
/// way the profile leans.
///
/// **The fold is deliberate.** Area distortion here is `flat area / 3D area`,
/// and whether a high-relief region makes that grow or shrink toward the disk
/// centre is not something this instrument can assert without measuring it —
/// so "climbing steeply" is defined as *far from unity in either direction*
/// rather than as a signed inequality that could be silently backwards. The
/// raw per-bucket medians are printed beside it so the direction is visible.
///
/// `None` when the profile was never measured, or a bucket is empty/zero.
fn radial_climb(report: &SpiralReport) -> Option<f64> {
    let first = report.area_distortion_by_disk_radius.first()?;
    let last = report.area_distortion_by_disk_radius.last()?;
    if first.triangles == 0 || last.triangles == 0 {
        return None;
    }
    let (a, b) = (first.area_distortion_median, last.area_distortion_median);
    if !(a.is_finite() && b.is_finite()) || a <= 0.0 || b <= 0.0 {
        return None;
    }
    let ratio = a / b;
    Some(ratio.max(1.0 / ratio))
}

/// The radial area-distortion profile — five buckets outward from the disk
/// centre. Printed per arm, and again side by side, because the STEEP-vs-FLAT
/// comparison of this profile is the attribution evidence for the whole
/// phase.
fn print_radial_profile(label: &str, report: &SpiralReport) {
    eprintln!("   -- radial area-distortion profile — {label} --");
    if report.area_distortion_by_disk_radius.is_empty() {
        eprintln!("     NOT MEASURED: the flattening never ran.");
        return;
    }
    eprintln!(
        "     {:>10} {:>10} {:>12} {:>14} {:>14} {:>14}",
        "r_lo", "r_hi", "triangles", "distort min", "distort med", "distort max"
    );
    for bucket in &report.area_distortion_by_disk_radius {
        eprintln!(
            "     {:>10.3} {:>10.3} {:>12} {:>14.6e} {:>14.6e} {:>14.6e}",
            bucket.r_lo,
            bucket.r_hi,
            bucket.triangles,
            bucket.area_distortion_min,
            bucket.area_distortion_median,
            bucket.area_distortion_max
        );
    }
    match radial_climb(report) {
        Some(climb) => eprintln!(
            "     climb |median(bucket 0) / median(bucket 4)|, folded to >= 1: {climb:.3}"
        ),
        None => eprintln!("     climb: NOT COMPUTABLE (an end bucket is empty or non-positive)"),
    }
}

// ── the coverage audit: the INDEPENDENT witness ─────────────────────────

/// **[REPO] falsifier bar** on `CoverageAudit::unmachined_area_fraction`.
///
/// # Why this bar exists at all
///
/// Because its absence let this instrument print `uncovered after rings 0`,
/// `uncovered after bridging 0` and a **PASSING** §B.4 falsifier over a part
/// with a **2.783 mm-radius unmachined hole at the cap centre — 23.5 % of the
/// region area**. Three green gates on a quarter-unmachined part. The cause
/// was that `N_S` was a *cap*: 46 % of ARM SPHERE's 37,060 triangles were
/// apportioned **zero** samples (central polar triangles are ~55× smaller, so
/// they starved first), the coverage predicate therefore had no population in
/// the middle, and "covered" was **vacuous**. This is the repo's own
/// "a gate handed an empty population passes and looks healthy" failure, in
/// its purest form.
///
/// # Why 2 %
///
/// **[REPO] choice, and it is a STOP bar, not a quality target.**
///
/// * The audit's population is the **mesh's triangle centroids**, which is not
///   the ring search's `S^h` sample set. The two disagree slightly at the
///   region rim, where a centroid can sit just outside the outermost ring's
///   swept band while the samples that placed that ring are covered. A little
///   rim grazing is legitimate, so a bar at exactly 0 would fire on healthy
///   runs and be turned off — a bar nobody trusts is worse than no bar.
/// * 23.5 % is what it exists to catch. 2 % is an order of magnitude below
///   that and comfortably above rim effects on these fixtures.
/// * It is **not** a claim that ≤ 2 % is good. Any nonzero fraction is
///   printed, and `CoverageAudit::uncovered_distance` is what separates a rim
///   graze (distances just above `K_c`) from a hole (distances far above it).
///   A sub-bar fraction whose distances are far above `K_c` is still a hole
///   and must be read as one — the bar does not catch that case and says so
///   in its own printed text.
const MAX_UNMACHINED_FRACTION: f64 = 0.02;

/// Print the coverage audit, and return the measured unmachined **fraction**.
///
/// `None` means the audit was **not measured** — which is not a clean reading
/// and must never be coerced to zero. Called on every arm and on the refusal
/// paths too, because "did the search think it was done" and "is the region
/// actually machined" are different questions and only this one answers the
/// second.
fn print_coverage_audit(label: &str, report: &SpiralReport, ball_radius_mm: f64) -> Option<f64> {
    eprintln!("\n   ===== COVERAGE AUDIT — {label} — THE INDEPENDENT WITNESS =====");
    let Some(audit) = report.coverage_audit.as_ref() else {
        eprintln!(
            "     NOT MEASURED — `coverage_audit` is None.\n\
             \x20    This is NOT a clean reading and must NOT be read as zero unmachined area.\n\
             \x20    `None` means the pipeline never got far enough to run the audit (or the\n\
             \x20    binary predates it); `Some(0.0)` would mean measured and clean. Any\n\
             \x20    falsifier that depends on this condition CANNOT clear it from here."
        );
        return None;
    };

    let pct = 100.0 * audit.unmachined_area_fraction;
    eprintln!(
        "     UNMACHINED AREA FRACTION      {pct:>12.4} %   <<< HEADLINE — the one number that \
         says whether the part is finished"
    );
    eprintln!(
        "     unmachined area (mm²)         {:>12.4}  of region {:.4} mm²",
        audit.unmachined_area_mm2, audit.region_area_mm2
    );
    eprintln!(
        "     uncovered centroid triangles  {:>12}  of {} tested",
        audit.uncovered_centroid_triangles, audit.centroids_tested
    );
    eprintln!(
        "     largest unmachined triangle   {:>12.6} mm²",
        audit.largest_unmachined_triangle_area_mm2
    );
    let d = &audit.uncovered_distance;
    if d.samples == 0 {
        eprintln!(
            "     uncovered distance to spiral  EMPTY (0 samples) — nothing was left uncovered"
        );
    } else {
        eprintln!(
            "     uncovered distance to spiral  n={:<7} min {:>9.4} / med {:>9.4} / max {:>9.4} mm",
            d.samples, d.min_mm, d.median_mm, d.max_mm
        );
        eprintln!(
            "       vs K_c = {ball_radius_mm:.4} mm  ->  median is {:.2}x K_c. \
             RIM GRAZE reads just above 1x;\n\
             \x20      a HOLE reads far above it. This row, not the fraction, is what tells the \
             two apart.",
            d.median_mm / ball_radius_mm.max(1e-12)
        );
    }

    eprintln!(
        "\n     WHAT MAKES THIS INDEPENDENT: the population here is the MESH's triangle\n\
         \x20    centroids — every triangle in the region, by construction — NOT the ring\n\
         \x20    search's own S^h sample set. The search cannot be its own witness: when\n\
         \x20    apportionment starved a region of samples, `uncovered_after_rings` read 0\n\
         \x20    because every sample that EXISTED was covered, and there were none in the\n\
         \x20    middle. A witness drawn from a different population is the only thing that\n\
         \x20    sees that.\n\
         \x20    CAVEAT, carried from the module: at quota 1, `barycentric_lattice` places its\n\
         \x20    single sample AT the centroid, so the two populations coincide there. The\n\
         \x20    independence is therefore POPULATION-LEVEL — it catches starvation regressions\n\
         \x20    and emission/bridging losses — not pointwise. Do not cite it as a pointwise\n\
         \x20    proof of coverage."
    );

    if audit.unmachined_area_fraction > MAX_UNMACHINED_FRACTION {
        eprintln!(
            "\n     VERDICT: *** {pct:.4} % > {:.1} % — UNMACHINED. ***",
            100.0 * MAX_UNMACHINED_FRACTION
        );
    } else {
        eprintln!(
            "\n     VERDICT: {pct:.4} % <= {:.1} % bar. NOTE this is a STOP bar, not a quality\n\
             \x20    target: read the distance row above before calling a nonzero fraction \
             harmless.",
            100.0 * MAX_UNMACHINED_FRACTION
        );
    }
    Some(audit.unmachined_area_fraction)
}

// ── the refusal diagnosis (pre-registered, printed in the output) ───────

/// `StallContext::near_band`'s median above this multiple of `2·K_c` is the
/// module's own "far larger than 2·K_c".
///
/// **It reads `near_band`, not `all_uncovered`.** The latter is dominated by
/// the untouched disk interior — a large median there is expected and means
/// nothing — so a bar applied to it would answer a different question from the
/// one asked.
const STALL_FAR_MULTIPLE: f64 = 2.0;

/// `StallContext::near_band`'s median at or below this multiple of `K_c` is
/// "near K_c" — the reading that says the search, not the geometry, gave up.
const STALL_NEAR_MULTIPLE: f64 = 1.5;

/// Folded radial climb at or above this is "climbing steeply".
const RADIAL_CLIMB_STEEP: f64 = 10.0;

/// Folded radial climb at or below this is "flat".
const RADIAL_CLIMB_FLAT: f64 = 2.0;

/// Diagnose a refusal from the report that survived it, and print the
/// pre-registered verdict logic **in the output** so the reasoning is in the
/// artifact rather than in someone's head afterwards.
///
/// Returns whether the refusal was **diagnosable** — whether the report
/// carried the rows needed to attribute it. That, not the refusal itself, is
/// what the STEEP arm is allowed to fail on.
fn diagnose_refusal(
    label: &str,
    report: &SpiralReport,
    refusal: &SpiralRefusal,
    params: &SpiralParams,
) -> bool {
    eprintln!("\n========== REFUSAL DIAGNOSIS — {label} ==========\n");
    eprintln!("   refusal: {refusal:?}");
    eprintln!(
        "   region: {} triangles, {} vertices, {} edges, Euler {}, boundary loop {} verts / \
         {:.3} mm",
        report.region_triangles,
        report.region_vertices,
        report.region_edges,
        report.euler_characteristic,
        report.boundary_loop_vertices,
        report.boundary_loop_length_mm
    );

    print_flatten_block(report);
    eprintln!();
    print_radial_profile(label, report);

    // The audit runs on the refusal path too. A refusal means no spiral was
    // emitted, so this will normally read NOT MEASURED — but printing it
    // keeps the block in the same place on every path, so a reader scanning
    // for it never has to wonder whether it was omitted or was absent.
    print_coverage_audit(label, report, params.ball_radius_mm);

    let stall_class = matches!(
        refusal,
        SpiralRefusal::RingSearchStalled { .. } | SpiralRefusal::RingLimitReached { .. }
    );

    let Some(stall) = report.stall.as_ref() else {
        eprintln!("\n   -- stall context --");
        eprintln!("     ABSENT. The ring search either never ran or closed coverage.");
        // A non-stall refusal is diagnosable as long as the report survived
        // far enough to say what the region was.
        let diagnosable = !stall_class && report.region_triangles > 0;
        eprintln!(
            "     DIAGNOSABLE: {diagnosable} (a stall-class refusal with no stall block is \
             NOT diagnosable)"
        );
        return diagnosable;
    };

    // Both sides of every ratio below come from the STALL BLOCK's own K_c,
    // not from the params this file passed in — a bar and its observation
    // must be the same measure. The params value is printed beside it purely
    // as a cross-check that the two agree.
    let k_c = stall.coverage_radius_mm;
    eprintln!("\n   -- stall context --");
    eprintln!(
        "     rings placed before the stall  {:>12}",
        stall.rings_placed
    );
    eprintln!(
        "     ring radii (first / last)      {:>12.6} / {:.6}",
        stall.ring_radii.first().copied().unwrap_or(f64::NAN),
        stall.ring_radii.last().copied().unwrap_or(f64::NAN)
    );
    eprintln!(
        "     binary-search interval lo / hi {:>12.6} / {:.6}",
        stall.search_lo, stall.search_hi
    );
    eprintln!(
        "     ANY interior radius feasible?  {:>12}   (false = the search never lowered its\n\
         \x20                                                bound: no circle strictly inside the\n\
         \x20                                                previous ring can sweep everything\n\
         \x20                                                outside it)",
        stall.interior_radius_feasible
    );
    eprintln!(
        "     S^h points still uncovered     {:>12}",
        stall.uncovered
    );
    eprintln!(
        "     coverage radius K_c (mm)       {:>12.4}   (params.ball_radius_mm {:.4} — these \
         must agree)",
        stall.coverage_radius_mm, params.ball_radius_mm
    );
    eprintln!(
        "     band width (disk units)        {:>12.6}   2*K_c through the local linear scale of\n\
         \x20                                              the map — what `near band` is restricted\n\
         \x20                                              by, printed so the restriction is\n\
         \x20                                              interpretable rather than magic",
        stall.band_width_disk
    );
    eprintln!(
        "     distances measured against     {:>12?}",
        stall.distance_reference
    );

    // -- FOLD CENSUS. Zero is the reading an embedding must produce. --
    let fold_census = stall.uncovered_outside_last_ring;
    eprintln!(
        "\n     FOLD CENSUS  uncovered_outside_last_ring  {fold_census:>8}   {}",
        if fold_census == 0 {
            "MUST be 0 for an embedding — OK"
        } else {
            "*** NONZERO: points the ring search already CERTIFIED as swept are still \
             uncovered ***"
        }
    );
    eprintln!(
        "       These are still-uncovered S^h points whose DISK radius exceeds the last placed\n\
         \x20      ring's radius. Under an injective map that set is empty by construction. A\n\
         \x20      nonzero count is the fold signature: the flat locator resolves a fold's\n\
         \x20      multivaluedness by first hit, so the forward and inverse maps disagree over\n\
         \x20      the folded sector and samples there can never be swept AT ANY RADIUS. Read it\n\
         \x20      together with `FLIPPED triangles` above — they are two views of one defect."
    );

    // -- the four distance populations, each labelled with what it means --
    // `unit` is a parameter, not a constant "mm" in the format string: three
    // of these four populations are distances in mm and the fourth is a
    // position in DISK units. A row that labels its own units wrongly is the
    // instrument telling a lie its note then has to walk back.
    let show = |name: &str, unit: &str, note: &str, stats: &DistanceStats| {
        if stats.samples == 0 {
            eprintln!("     {name:<22} EMPTY (0 samples) — {note}");
        } else {
            eprintln!(
                "     {name:<22} n={:<7} min {:>9.4} / med {:>9.4} / max {:>9.4} {unit:<5} {note}",
                stats.samples, stats.min_mm, stats.median_mm, stats.max_mm
            );
        }
    };
    eprintln!("\n     -- distance populations (all against the reference curve above) --");
    show(
        "all uncovered",
        "mm",
        "dominated by the untouched disk INTERIOR; a large median here is expected and means \
         little on its own",
        &stall.all_uncovered,
    );
    show(
        "near band",
        "mm",
        "restricted to within one band width of the last placed radius — the points the stall \
         is ABOUT",
        &stall.near_band,
    );
    show(
        "blockers  <<<",
        "mm",
        "WHAT ACTUALLY STOPPED THE DESCENT: uncovered points outside search_lo that the ring at \
         search_lo (the largest PROVEN-INFEASIBLE radius) fails to sweep. Empty means the search \
         never proved any radius infeasible.",
        &stall.blockers,
    );
    show(
        "blocker disk radius",
        "disk",
        "the same blockers' positions in the DOMAIN — disk units, NOT mm; do not compare these \
         to K_c. (The struct reuses DistanceStats, so its fields are still SPELLED `_mm`; the \
         values are not.)",
        &stall.blocker_disk_radius,
    );

    // -- the pre-registered verdict --
    //
    // The distance condition reads `near_band`, not `all_uncovered`: the
    // latter is dominated by the untouched disk interior, so a bar applied to
    // it would be answering a different question from the one asked. When the
    // near band is empty there is no distance reading at all, and the
    // conditions that depend on one are false rather than vacuously true.
    let band = &stall.near_band;
    let have_distance = band.samples > 0;
    let median = band.median_mm;
    let far_bar = STALL_FAR_MULTIPLE * 2.0 * k_c;
    let near_bar = STALL_NEAR_MULTIPLE * k_c;
    let climb = radial_climb(report);
    let far = have_distance && median > far_bar;
    let near = have_distance && median <= near_bar;
    let steep_profile = climb.is_some_and(|c| c >= RADIAL_CLIMB_STEEP);
    let flat_profile = climb.is_some_and(|c| c <= RADIAL_CLIMB_FLAT);
    let map_defect = report.flipped_triangles > 0 || fold_census > 0;
    let embedding_clean = report.flipped_triangles == 0 && fold_census == 0;

    eprintln!("\n   -- PRE-REGISTERED VERDICT LOGIC (stated before the run, evaluated here) --");
    eprintln!(
        "     A. interior_radius_feasible == false            -> {}",
        !stall.interior_radius_feasible
    );
    if have_distance {
        eprintln!(
            "     B. near-band median {median:.4} > {STALL_FAR_MULTIPLE} x 2*K_c = \
             {far_bar:.4} mm      -> {far}"
        );
        eprintln!(
            "     D. near-band median {median:.4} <= {STALL_NEAR_MULTIPLE} x K_c = \
             {near_bar:.4} mm     -> {near}"
        );
    } else {
        eprintln!(
            "     B/D. near-band population is EMPTY — no distance reading, so both\n\
             \x20          distance conditions are FALSE rather than vacuously true."
        );
    }
    eprintln!(
        "     C. radial climb >= {RADIAL_CLIMB_STEEP:.1} (\"climbing steeply\")          -> \
         {steep_profile}  (climb {})",
        climb.map_or("n/a".to_owned(), |c| format!("{c:.3}"))
    );
    eprintln!(
        "     E. radial climb <= {RADIAL_CLIMB_FLAT:.1} (\"flat profile\")               -> \
         {flat_profile}"
    );
    eprintln!(
        "     F. fold census == 0 AND flipped == 0 (embedding clean) -> {embedding_clean}  \
         (fold {fold_census}, flipped {})",
        report.flipped_triangles
    );

    if map_defect {
        eprintln!(
            "\n     VERDICT (NOT F): **MAP** finding — the flattening is not an embedding.\n\
             \x20    flipped {} / fold census {fold_census}. Under mean-value (Floater) weights\n\
             \x20    this SHOULD BE IMPOSSIBLE: every weight is strictly positive, so Tutte makes\n\
             \x20    a valid embedding certain. This branch is therefore a TRIPWIRE, not an\n\
             \x20    expected outcome — reaching it means a bug or an under-converged solve\n\
             \x20    (check the sweep delta and mean_value_weight_nonpositive above), NOT a\n\
             \x20    property of the geometry. Do not diagnose the ring search until this is\n\
             \x20    cleared.",
            report.flipped_triangles
        );
    } else {
        // `embedding_clean` is the exact complement of `map_defect`, so this
        // arm IS condition F and no third top-level arm can ever fire. The
        // INCONCLUSIVE fallback therefore lives one level down, among A-E,
        // which is the only place a mixed reading is actually possible — a
        // top-level `else` here would have been a guard that cannot trip,
        // which is the vacuous-branch pattern this repo keeps paying for.
        debug_assert!(embedding_clean);
        eprintln!(
            "\n     VERDICT (F): GENUINE COVERAGE INFEASIBILITY — a **MECHANISM / GEOMETRY**\n\
             \x20    finding. The map IS an embedding (no flips, no fold census), and the search\n\
             \x20    still stalled, so the failure is not the front-end: at this map's distortion\n\
             \x20    the surface cannot be covered ring-by-ring by concentric disk circles. Read\n\
             \x20    the `blockers` row for what stopped the descent, and conditions A-E for\n\
             \x20    which flavour:"
        );
        if !stall.interior_radius_feasible && far && steep_profile {
            eprintln!(
                "\x20      A+B+C: no interior radius was EVER feasible, the near-band points sit\n\
                 \x20      far beyond a tool diameter, and the radial profile climbs — the\n\
                 \x20      razor-thin-band reading. The lever is the front-end's DISTORTION (a\n\
                 \x20      conformal slit map, Phase F2 step 3), even though the map is valid."
            );
        } else if near && flat_profile {
            eprintln!(
                "\x20      D+E: the near-band points are within reach of the last ring and the\n\
                 \x20      radial profile is level, so a correct search should have placed another\n\
                 \x20      ring. The lever is the Eqs. 1-4 binary search itself — its monotonicity\n\
                 \x20      assumption is the module's stated, unproved premise."
            );
        } else {
            eprintln!(
                "\x20      INCONCLUSIVE: A-E do not form either sub-pattern. Report the rows; do\n\
                 \x20      NOT pick a story to fit them. This is the live fallback — it exists so\n\
                 \x20      a mixed reading cannot be quietly rounded to whichever flavour was\n\
                 \x20      expected."
            );
        }
    }

    let diagnosable = !report.area_distortion_by_disk_radius.is_empty();
    eprintln!(
        "\n     DIAGNOSABLE: {diagnosable} (stall block present, radial profile {})",
        if diagnosable { "present" } else { "MISSING" }
    );
    diagnosable
}

// ── STAGE F2-A — plan, report, falsifier ────────────────────────────────

/// Print the params a run used. Called for the main run and for every Stage E
/// re-run, so no row of that table is ambiguous about what produced it.
fn print_params(label: &str, params: &SpiralParams) {
    eprintln!(
        "     {label:<20} K_c {:.3} mm, h {:.3} mm, N_S {}, N_C {}, ring_eps {:.1e}, \
         blend_p {:.1}, start_angle_step {:.6} rad ({} candidates), \
         secondary_line_shift {:.4}, shift_step {:.6}, max_rings {}",
        params.ball_radius_mm,
        params.scallop_h_mm,
        params.n_surface_samples,
        params.n_angular_samples,
        params.ring_eps,
        params.blend_p,
        params.start_angle_step,
        (TAU / params.start_angle_step).floor().max(1.0) as usize,
        params.secondary_line_shift,
        params.shift_step,
        params.max_rings
    );
    // The solver dials are printed separately because `solver_tolerance` is an
    // ABSOLUTE max-sweep-delta bound in unit-disk units, not the relative CG
    // residual the field of the same role used to carry. A reader who assumes
    // the old semantics reads 1e-9 as far looser than it is.
    eprintln!(
        "     {:<20} Gauss-Seidel: solver_max_sweeps {}, solver_tolerance {:.1e} \
         (ABSOLUTE max coordinate change per sweep, unit-disk units — NOT a relative residual)",
        "", params.solver_max_sweeps, params.solver_tolerance
    );
}

/// Summary of a `Vec<f64>` that may be thousands of entries long.
fn min_med_max(values: &[f64]) -> (f64, f64, f64) {
    if values.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    (
        percentile(&sorted, 0.0),
        percentile(&sorted, 0.50),
        percentile(&sorted, 1.0),
    )
}

fn min_med_max_usize(values: &[usize]) -> (usize, usize, usize) {
    if values.is_empty() {
        return (0, 0, 0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    (
        sorted[0],
        sorted[sorted.len() / 2],
        sorted[sorted.len() - 1],
    )
}

fn stage_a(
    label: &str,
    report: &SpiralReport,
    params: &SpiralParams,
    stepover_mm: f64,
    region_area_3d_mm2: f64,
) -> bool {
    eprintln!("========== STAGE F2-A — plan + report + §B.4 falsifier ==========\n");
    eprintln!(
        "   cutter: BallEndmill::new(diameter {:.3}, cutting length {BALL_CUTTING_LENGTH_MM:.1}) \
         => radius {BALL_RADIUS_MM:.3} mm; SpiralParams.ball_radius_mm = {:.3} mm.\n\
         \x20  (The constructor takes a DIAMETER and the module takes a RADIUS; both are \
         printed so the trap is checkable.)",
        BALL_RADIUS_MM * 2.0,
        params.ball_radius_mm
    );
    eprintln!("   params actually used:");
    print_params("main run", params);
    eprintln!(
        "     start-angle sweep DEVIATION: PAPER_START_ANGLE_STEP = {PAPER_START_ANGLE_STEP:.6} rad \
         ({} candidates) coarsened to {SWEEP_START_ANGLE_STEP:.6} rad ({} candidates)\n\
         \x20      under the runtime clause. It changes only WHICH start angle minimises total 3D\n\
         \x20      length — not spacing, not coverage, not bridging, and no falsifier input.",
        (TAU / PAPER_START_ANGLE_STEP).floor() as usize,
        (TAU / SWEEP_START_ANGLE_STEP).floor() as usize
    );
    eprintln!(
        "     secondary_line_shift is 0.0, NOT the paper's PAPER_SECONDARY_LINE_SHIFT (8π/5)\n\
         \x20      — the module's G-BRIDGE-BOOKKEEPING resolution. Read bridge_overhead_pct below\n\
         \x20      with that in mind.\n"
    );

    eprintln!("   -- region + topology --");
    eprintln!(
        "     region triangles              {:>12}",
        report.region_triangles
    );
    eprintln!(
        "     region vertices               {:>12}",
        report.region_vertices
    );
    eprintln!(
        "     region edges                  {:>12}",
        report.region_edges
    );
    eprintln!(
        "     Euler characteristic (disk=1) {:>12}",
        report.euler_characteristic
    );
    eprintln!(
        "     boundary loop vertices        {:>12}",
        report.boundary_loop_vertices
    );
    eprintln!(
        "     boundary loop length (mm)     {:>12.3}",
        report.boundary_loop_length_mm
    );

    print_flatten_block(report);

    eprintln!("\n   -- S^h sampling + coverage-driven rings (Eqs. 1-4) --");
    eprintln!(
        "     S^h samples placed            {:>12}   (requested {}; see SAMPLING ADEQUACY below)",
        report.samples_placed, report.samples_requested
    );
    eprintln!(
        "     ring count                    {:>12}",
        report.ring_count
    );
    let (r_min, r_med, r_max) = min_med_max(&report.ring_radii);
    eprintln!("     ring disk radii min/med/max   {r_min:>12.6} / {r_med:.6} / {r_max:.6}");
    let (l_min, l_med, l_max) = min_med_max(&report.ring_lengths_mm);
    eprintln!("     ring 3D length min/med/max mm {l_min:>12.3} / {l_med:.3} / {l_max:.3}");
    let (b_min, b_med, b_max) = min_med_max_usize(&report.ring_newly_covered);
    eprintln!("     band BP_i min/med/max         {b_min:>12} / {b_med} / {b_max}");
    eprintln!(
        "     total ring length (mm)        {:>12.3}",
        report.total_ring_length_mm
    );
    eprintln!(
        "     binary search iterations      {:>12}",
        report.binary_search_iterations
    );
    eprintln!(
        "     disk points pulled back       {:>12}",
        report.ring_points_pulled_back
    );
    eprintln!(
        "     disk points UNLOCATED         {:>12}   (nonzero = a polyline has a gap)",
        report.ring_points_unlocated
    );
    eprintln!(
        "     uncovered after rings         {:>12}   (want 0)",
        report.uncovered_after_rings
    );

    // -- SAMPLING ADEQUACY, apportionment-aware --
    eprintln!("\n   -- SAMPLING ADEQUACY (G-SAMPLING) --");
    let half_stepover = 0.5 * stepover_mm;
    eprintln!(
        "     N_S requested (a FLOOR)       {:>12}",
        report.samples_requested
    );
    eprintln!(
        "     samples actually placed       {:>12}   (>= region triangles {}; N_S is no longer\n\
         \x20                                              a cap, so this may exceed the request)",
        report.samples_placed, report.region_triangles
    );

    let starved = report.triangles_without_samples;
    eprintln!(
        "     TRIANGLES WITH ZERO SAMPLES   {starved:>12}   {}",
        if starved == 0 {
            "MUST be 0 — OK"
        } else {
            "*** TRIPWIRE FIRED — the coverage predicate has NO POPULATION on these \
             triangles, so \"covered\" is VACUOUS there ***"
        }
    );
    eprintln!(
        "     largest unsampled triangle    {:>12.6} mm²",
        report.largest_unsampled_triangle_area_mm2
    );

    let worst = report.max_local_sample_spacing_mm;
    let ratio = worst / half_stepover.max(1e-12);
    eprintln!(
        "     MAX LOCAL sample spacing      {worst:>12.4} mm   = max over triangles of \
         sqrt(area_t / quota_t)"
    );
    eprintln!("     stepover / 2 (the bar)        {half_stepover:>12.4} mm");
    eprintln!(
        "     ratio worst / (stepover/2)    {ratio:>12.3}   {}",
        if ratio < 1.0 {
            "PASS — the worst-sampled triangle still resolves below half the stepover"
        } else {
            "*** COARSE — at least one triangle is sampled more coarsely than half the \
             stepover; ring spacing there can read long and coverage can read vacuous ***"
        }
    );
    eprintln!(
        "     WHY THIS NUMBER AND NOT sqrt(region_area / N_S). That older figure is an AVERAGE\n\
         \x20    and is APPORTIONMENT-BLIND: on the ARM SPHERE run it reported a healthy\n\
         \x20    0.076 mm on a mesh where 46%% of triangles held ZERO samples, and the run went\n\
         \x20    on to leave a 2.783 mm-radius unmachined hole under three green gates. It is\n\
         \x20    SUPERSEDED and deliberately not printed: a number that cannot see the failure\n\
         \x20    it is supposed to guard against should not sit in the adequacy block being\n\
         \x20    read as the adequacy check.\n\
         \x20    `max_local_sample_spacing_mm` is a MAX over triangles of each triangle's OWN\n\
         \x20    sample density, so a single starved triangle moves it. That is the property\n\
         \x20    the average lacked. Region 3D area for scale: {region_area_3d_mm2:.1} mm².\n\
         \x20    Even so, this block is about the SEARCH's inputs — the COVERAGE AUDIT below is\n\
         \x20    what says whether the emitted path actually machined the region."
    );

    eprintln!("\n   -- bridging (Eqs. 7-9 + A-11) --");
    eprintln!(
        "     start-angle candidates        {:>12}",
        report.start_angle_candidates
    );
    eprintln!(
        "     chosen start angle (rad)      {:>12.6}",
        report.start_angle_rad
    );
    eprintln!(
        "     bridges                       {:>12}   (expect ring_count - 1)",
        report.bridge_count
    );
    eprintln!(
        "     total bridge length (mm)      {:>12.3}",
        report.total_bridge_length_mm
    );
    eprintln!(
        "     bridge overhead (%)           {:>12.3}",
        report.bridge_overhead_pct
    );
    eprintln!(
        "     bridge repair steps           {:>12}   (0 expected: the §2.2.2 step-2 repair is a\n\
         \x20                                              no-op in the simply-connected case)",
        report.bridge_repair_steps
    );
    eprintln!(
        "     uncovered after bridging      {:>12}   (want 0)",
        report.uncovered_after_bridging
    );

    eprintln!("\n   -- the emitted spiral --");
    eprintln!(
        "     spiral 3D length (mm)         {:>12.3}",
        report.spiral_length_mm
    );
    eprintln!(
        "     spiral points                 {:>12}",
        report.spiral_points
    );
    eprintln!(
        "     disk self-intersections       {:>12}   (MEASURED, not inferred)",
        report.disk_self_intersections
    );
    eprintln!(
        "     retract count                 {:>12}   (structural; asserted below)",
        report.retract_count
    );
    eprintln!(
        "     consecutive step max / median {:>12.4} / {:.4} mm",
        report.max_consecutive_step_mm, report.median_consecutive_step_mm
    );
    eprintln!(
        "     max_consecutive_step_mm — NOT the retract zero — is the evidence that no hidden\n\
         \x20    jump is hiding inside the single polyline. Compare it with the stepover\n\
         \x20    ({stepover_mm:.4} mm): a step far above that is a lift in all but name."
    );

    // -- the INDEPENDENT witness, before the falsifier that now consumes it --
    // K_c comes from the params the MODULE was actually handed, not from this
    // file's BALL_RADIUS_MM constant: an arm that overrode the radius would
    // otherwise have its distances compared against the wrong tool.
    let unmachined = print_coverage_audit(label, report, params.ball_radius_mm);

    // -- the falsifier --
    eprintln!("\n   -- FALSIFIER (research note §B.4, concretised 2026-08-30;");
    eprintln!("      condition 4 added 2026-08-30 after the ARM SPHERE hole) --");
    let mut fails: Vec<String> = Vec::new();
    if report.uncovered_after_bridging > 0 {
        fails.push(format!(
            "incomplete coverage after bridge repair: uncovered_after_bridging = {} of {} samples",
            report.uncovered_after_bridging, report.samples_placed
        ));
    }
    if report.disk_self_intersections > 0 {
        fails.push(format!(
            "disk-domain self-intersection: {} proper crossings",
            report.disk_self_intersections
        ));
    }
    if report.bridge_overhead_pct > MAX_BRIDGE_OVERHEAD_PCT {
        fails.push(format!(
            "bridge length overhead {:.3}% > {MAX_BRIDGE_OVERHEAD_PCT:.1}%",
            report.bridge_overhead_pct
        ));
    }
    // Condition 4. An ABSENT audit is a STOP, not a pass: a falsifier cannot
    // clear a condition it has no observation for, and coercing `None` to zero
    // is precisely how an unmeasured quantity becomes a green gate.
    match unmachined {
        Some(fraction) if fraction > MAX_UNMACHINED_FRACTION => fails.push(format!(
            "UNMACHINED AREA {:.4}% > {:.1}% — the emitted path does not machine the region",
            100.0 * fraction,
            100.0 * MAX_UNMACHINED_FRACTION
        )),
        Some(_) => {}
        None => fails.push(
            "coverage audit NOT MEASURED — condition 4 cannot be cleared without an observation"
                .to_owned(),
        ),
    }

    eprintln!(
        "     1. coverage after bridging   {} (bar: 0)",
        report.uncovered_after_bridging
    );
    eprintln!(
        "     2. disk self-intersections   {} (bar: 0)",
        report.disk_self_intersections
    );
    eprintln!(
        "     3. bridge overhead           {:.3}% (bar: <= {MAX_BRIDGE_OVERHEAD_PCT:.1}%, inside \
         the §0k +43% trap with margin)",
        report.bridge_overhead_pct
    );
    eprintln!(
        "     4. UNMACHINED AREA           {} (bar: <= {:.1}%)",
        unmachined.map_or_else(
            || "NOT MEASURED".to_owned(),
            |f| format!("{:.4}%", 100.0 * f)
        ),
        100.0 * MAX_UNMACHINED_FRACTION
    );
    eprintln!(
        "     Condition 4 is the one that would have caught the ARM SPHERE hole IN ONE LINE.\n\
         \x20    Conditions 1-3 all passed on that run — 1 passed VACUOUSLY, because the search's\n\
         \x20    own sample population was empty exactly where the hole was. A falsifier built\n\
         \x20    entirely from the search's self-report cannot fail on a starved search; it needs\n\
         \x20    a witness from a different population, which is what condition 4 reads."
    );

    // Structural claim gets a hard assert, not a print.
    assert_eq!(
        report.retract_count, 0,
        "conformal_spiral claims exactly one polyline and no lift anywhere in the \
         construction; retract_count must be 0 by construction"
    );

    if fails.is_empty() {
        eprintln!("\n     PASS: all FOUR conditions clear. Stages C and D run.\n");
        true
    } else {
        eprintln!("\n     FAIL / STOP:");
        for f in &fails {
            eprintln!("       - {f}");
        }
        eprintln!(
            "     Stages C and D are SKIPPED. Stage B still runs — diagnosis needs the picture —\n\
             \x20    and Stage E still runs, because if the falsifier tripped, the sampling\n\
             \x20    sensitivity rows ARE the evidence for why.\n"
        );
        false
    }
}

// ── STAGE F2-B — the two SVGs ───────────────────────────────────────────

fn stage_b(region: &Region, result: &SpiralResult, report: &SpiralReport, slug: &str) {
    eprintln!("========== STAGE F2-B — disk-domain and XY-world SVGs ==========\n");
    let output_dir = svg_output_dir();
    std::fs::create_dir_all(&output_dir).expect("create F2 SVG output directory");

    // ---- (1) the DISK domain ------------------------------------------
    //
    // The pre-bridge rings ARE exact circles in this domain: every run is
    // emitted at one constant radius `report.ring_radii[i]`, so a <circle>
    // element is faithful rather than an approximation.
    let mut disk = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-1.12 -1.12 2.24 2.24\" \
         width=\"1400\" height=\"1400\">\n\
         <title>{slug} F2: spiral in the unit-disk domain, {} rings, {} points</title>\n\
         <rect x=\"-1.12\" y=\"-1.12\" width=\"2.24\" height=\"2.24\" fill=\"white\"/>\n\
         <circle cx=\"0\" cy=\"0\" r=\"1\" fill=\"none\" stroke=\"black\" stroke-width=\"0.005\"/>\n",
        report.ring_count,
        result.spiral_disk.len()
    );
    for r in &report.ring_radii {
        writeln!(
            disk,
            "<circle cx=\"0\" cy=\"0\" r=\"{r:.6}\" fill=\"none\" stroke=\"#bbbbbb\" \
             stroke-width=\"0.0009\"/>"
        )
        .expect("write disk ring");
    }
    if let Some(first) = result.spiral_disk.first() {
        let mut d = format!("M {:.6} {:.6}", first.0, first.1);
        for point in &result.spiral_disk[1..] {
            write!(d, " L {:.6} {:.6}", point.0, point.1).expect("write disk spiral");
        }
        writeln!(
            disk,
            "<path d=\"{d}\" fill=\"none\" stroke=\"#e41a1c\" stroke-width=\"0.0018\"/>"
        )
        .expect("write disk spiral element");
    }
    disk.push_str("</svg>\n");
    // The slug carries the ARM, not just the fixture: Stage B now runs on
    // every planned arm, and a file called "conformal_spiral_disk" would be
    // read later as whichever arm the reader had in mind. The retained
    // terrain arm keeps its historical
    // `terrain_small_conformal_spiral_flat_*` names so the withdrawn
    // artifacts named in FINDINGS_F2 §F2-1 stay findable.
    let disk_path = output_dir.join(format!("{slug}_disk_f2.svg"));
    std::fs::write(&disk_path, disk).expect("write F2 disk SVG");

    // ---- (2) the XY world view ----------------------------------------
    //
    // API gap resolved by the spec's sanctioned fallback: `SpiralResult`
    // exposes `spiral_contact` / `spiral_disk` / `rings_contact` only — the
    // per-point bridge flags (`Lifted.flags`) never leave the module, so ring
    // PARITY along the spiral is not recoverable from the public API. The
    // brief pre-authorised this ("else single colour"), so the spiral is one
    // stroke. `rings_contact` IS per-ring, so the PRE-BRIDGE rings are
    // underlaid in an alternating-parity light stroke, which is where parity
    // is free. No `src/` change was made.
    const RING_COLOURS: [&str; 2] = ["#9ecae1", "#fdae6b"];

    let [x0, y0, x1, y1] = region.polygon.bbox();
    let padding = 2.0;
    let (view_x, view_y) = (x0 - padding, y0 - padding);
    let (view_w, view_h) = (x1 - x0 + 2.0 * padding, y1 - y0 + 2.0 * padding);
    let mut world = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_x:.3} {view_y:.3} \
         {view_w:.3} {view_h:.3}\" width=\"1400\" height=\"1400\">\n\
         <title>{slug} F2: conformal spiral over the region, \
         {} contact points</title>\n\
         <rect x=\"{view_x:.3}\" y=\"{view_y:.3}\" width=\"{view_w:.3}\" height=\"{view_h:.3}\" \
         fill=\"white\"/>\n",
        result.spiral_contact.len()
    );
    for (i, ring) in result.rings_contact.iter().enumerate() {
        let Some(first) = ring.first() else { continue };
        let colour = RING_COLOURS[i % RING_COLOURS.len()];
        let mut d = format!("M {:.3} {:.3}", first.x, first.y);
        for point in &ring[1..] {
            write!(d, " L {:.3} {:.3}", point.x, point.y).expect("write ring");
        }
        writeln!(
            world,
            "<path d=\"{d}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"0.05\"/>"
        )
        .expect("write ring element");
    }
    if let Some(first) = result.spiral_contact.first() {
        let mut d = format!("M {:.3} {:.3}", first.x, first.y);
        for point in &result.spiral_contact[1..] {
            write!(d, " L {:.3} {:.3}", point.x, point.y).expect("write spiral");
        }
        writeln!(
            world,
            "<path d=\"{d}\" fill=\"none\" stroke=\"#e41a1c\" stroke-width=\"0.05\"/>"
        )
        .expect("write spiral element");
    }
    writeln!(
        world,
        "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.25\"/>",
        svg_path(&region.polygon)
    )
    .expect("write region boundary");
    world.push_str("</svg>\n");
    let world_path = output_dir.join(format!("{slug}_xy_f2.svg"));
    std::fs::write(&world_path, world).expect("write F2 world SVG");

    eprintln!(
        "   disk domain: unit circle black, {} pre-bridge ring circles light grey, \
         the spiral polyline red (single stroke).",
        report.ring_radii.len()
    );
    eprintln!("     SVG: {}", disk_path.display());
    eprintln!(
        "   XY world: region ellipse black, {} pre-bridge rings underlaid alternating \
         blue/orange on ring parity, the bridged spiral red (single stroke —\n\
         \x20  per-point bridge flags are module-internal, so spiral-side parity is not\n\
         \x20  recoverable from the public API; the brief's sanctioned fallback).",
        result.rings_contact.len()
    );
    eprintln!("     SVG: {}", world_path.display());
    eprintln!(
        "   NOTE both files follow F1's convention of NOT flipping Y, so the image is \
         mirrored vertically against the machine frame.\n"
    );
}

// ── STAGE F2-C — spacing + scallop evidence ─────────────────────────────

/// Returns the measured adjacent-ring 3D spacings, **sorted ascending**, so
/// the analytic verdict and the Stage-D fairness table read the same
/// population this stage printed rather than re-deriving it. `None` when
/// there was nothing adjacent to measure.
fn stage_c(
    result: &SpiralResult,
    stepover_mm: f64,
    curvature: &CurvatureCensus,
    kind: ArmKind,
) -> Option<Vec<f64>> {
    /// Sample every Nth point of ring i+1. Restated from F1's stage C
    /// (`direction_field_wanaka_f1.rs:772`): spacing is a smooth quantity
    /// along a curve, so every point would multiply the work without adding a
    /// distinct measurement.
    const SAMPLE_STRIDE: usize = 5;
    /// Contract band around the target stepover, as F1 uses.
    const BAND: f64 = 0.25;

    eprintln!("========== STAGE F2-C — measured adjacent-ring spacing + scallop ==========\n");
    if kind == ArmKind::FixtureLimited {
        eprintln!("   {WITHDRAWAL_BANNER}\n");
    }
    eprintln!(
        "   Method: every {SAMPLE_STRIDE}th point of ring i+1, 3D distance to the nearest\n\
         \x20  SEGMENT (not vertex) of ring i, over the PRE-BRIDGE rings (rings_contact,\n\
         \x20  outermost first). Geometry only — no simulation. Contact points, not CL.\n"
    );

    if result.rings_contact.len() < 2 {
        eprintln!("   SKIP: fewer than two rings — nothing adjacent to measure.\n");
        return None;
    }

    let mut spacings: Vec<f64> = Vec::new();
    for pair in result.rings_contact.windows(2) {
        let (outer, inner) = (&pair[0], &pair[1]);
        if outer.len() < 2 || inner.is_empty() {
            continue;
        }
        for point in inner.iter().step_by(SAMPLE_STRIDE) {
            let mut best = f64::INFINITY;
            for seg in outer.windows(2) {
                let distance = point_segment_distance_sq(*point, seg[0], seg[1]);
                if distance < best {
                    best = distance;
                }
            }
            if best.is_finite() {
                spacings.push(best.sqrt());
            }
        }
    }
    if spacings.is_empty() {
        eprintln!("   SKIP: no adjacent-ring sample pair produced a measurement.\n");
        return None;
    }
    spacings.sort_by(f64::total_cmp);
    let n = spacings.len() as f64;
    let mean = spacings.iter().sum::<f64>() / n;

    // -- the two targets --
    let flat_target = stepover_mm;
    let kappa_med = curvature.median();
    let kappa_p90 = curvature.p90();
    let curved =
        |k: f64| scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, k);

    eprintln!("   -- targets --");
    eprintln!(
        "     FLAT equal-cusp stepover, s = 2·sqrt(2Rh - h²) at R={BALL_RADIUS_MM}, \
         h={CUSP_HEIGHT_MM}:      {flat_target:.4} mm"
    );
    eprintln!(
        "     (cross-checked against scallop_math::stepover_from_scallop_flat: {:.4} mm)",
        scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM)
    );
    eprintln!(
        "\n     Terrain is CURVED, so the flat law is not the whole target. [REPO] discrete\n\
         \x20    curvature census over the region's interior edges (see `curvature_census`):\n\
         \x20      interior edges measured    {}\n\
         \x20      convex fraction            {:.3}\n\
         \x20      |kappa| min/med/p90/max    {:.5} / {:.5} / {:.5} / {:.5}  (1/mm)\n\
         \x20      => radius of curvature at med |kappa|: {:.2} mm",
        curvature.edges,
        curvature.convex_fraction,
        percentile(&curvature.abs_sorted, 0.0),
        kappa_med,
        kappa_p90,
        percentile(&curvature.abs_sorted, 1.0),
        if kappa_med > 0.0 {
            1.0 / kappa_med
        } else {
            f64::INFINITY
        }
    );
    eprintln!(
        "\n     scallop_math::stepover_from_scallop_curved at those curvatures (mm):\n\
         \x20      convex  +kappa_med  {:.4}    concave -kappa_med  {:.4}\n\
         \x20      convex  +kappa_p90  {:.4}    concave -kappa_p90  {:.4}\n\
         \x20    Convex NARROWS the admissible stepover, concave WIDENS it; a real terrain\n\
         \x20    region carries both, so the measured distribution below is compared against\n\
         \x20    the flat law as the reference and this pair as the envelope.",
        curved(kappa_med),
        curved(-kappa_med),
        curved(kappa_p90),
        curved(-kappa_p90)
    );

    eprintln!("\n   -- measured adjacent-ring 3D spacing --");
    eprintln!("     {:>26}  {:>12}", "statistic", "mm");
    eprintln!("     {:>26}  {:>12.4}", "FLAT target stepover", flat_target);
    eprintln!("     {:>26}  {:>12.4}", "min", percentile(&spacings, 0.0));
    eprintln!("     {:>26}  {:>12.4}", "p10", percentile(&spacings, 0.10));
    eprintln!(
        "     {:>26}  {:>12.4}",
        "median",
        percentile(&spacings, 0.50)
    );
    eprintln!("     {:>26}  {:>12.4}", "mean", mean);
    eprintln!("     {:>26}  {:>12.4}", "p90", percentile(&spacings, 0.90));
    eprintln!("     {:>26}  {:>12.4}", "max", percentile(&spacings, 1.0));

    let (low, high) = (flat_target * (1.0 - BAND), flat_target * (1.0 + BAND));
    let below = spacings.iter().filter(|&&s| s < low).count();
    let above = spacings.iter().filter(|&&s| s > high).count();
    eprintln!(
        "\n     samples {}, band +/-{:.0}% of the FLAT target = [{low:.4}, {high:.4}] mm",
        spacings.len(),
        BAND * 100.0
    );
    eprintln!(
        "     outside band: {} below ({:.1}%), {} above ({:.1}%), {} total ({:.1}%)",
        below,
        100.0 * below as f64 / n,
        above,
        100.0 * above as f64 / n,
        below + above,
        100.0 * (below + above) as f64 / n
    );

    // -- the paper's own 12% context, derived rather than hardcoded --
    let overshoot_h = CUSP_HEIGHT_MM * (1.0 + PAPER_SCALLOP_OVERSHOOT_PCT / 100.0);
    let overshoot_s = scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, overshoot_h);
    let overshot = spacings
        .iter()
        .filter(|&&s| scallop_math::scallop_height_flat(BALL_RADIUS_MM, s) > overshoot_h)
        .count();
    eprintln!("\n   -- PAPER CONTEXT: [SOURCE-2025 §5] measured scallop overshoot --");
    eprintln!(
        "     The paper's own cutting trial overshot its NOMINAL scallop by up to \
         {PAPER_SCALLOP_OVERSHOOT_PCT:.0}%.\n\
         \x20    The module header calls that \"the expected floor, not the ceiling\".\n\
         \x20    A spacing s implies a flat scallop scallop_height_flat({BALL_RADIUS_MM}, s);\n\
         \x20    the {PAPER_SCALLOP_OVERSHOOT_PCT:.0}% threshold is h = {overshoot_h:.5} mm, \
         which is s = {overshoot_s:.4} mm\n\
         \x20    (derived through scallop_math, not hardcoded)."
    );
    eprintln!(
        "     samples implying MORE than {PAPER_SCALLOP_OVERSHOOT_PCT:.0}% overshoot: \
         {overshot} of {} ({:.1}%)",
        spacings.len(),
        100.0 * overshot as f64 / n
    );
    eprintln!(
        "     Read this WITH the Stage-A N_S adequacy line: an implied sample spacing at or\n\
         \x20    above stepover/2 makes the coverage predicate optimistic, and inflated ring\n\
         \x20    spacing is exactly how that failure presents ([SOURCE-2025] Table 1 case 1.5).\n\
         \x20    Stage E moves N_S so the two explanations can be told apart.\n"
    );

    Some(spacings)
}

// ── STAGE F2-C2 — the two distributions the operator's eye caught ───────

/// Ring **disk**-radius distribution: where the rings actually sit in the
/// domain.
///
/// The operator's observation on the withdrawn terrain run was "way more
/// dense around the outside than the inside", and this is that observation
/// turned into a number. On a flat disk the map is exactly `z/ρ`, so equal 3D
/// spacing IS equal disk spacing and evenly-spread ring radii must have a
/// median of **0.500**. The terrain run measured **0.665** over 50 rings —
/// rings piling up against the rim. On this arm's fixture there is no coarse
/// boundary polygon for that mechanism to work through, so the median is the
/// direct test of whether the crowding was the fixture or the search.
fn print_ring_radius_distribution(report: &SpiralReport) {
    /// Bucket width over the unit disk radius.
    const BUCKET: f64 = 0.2;
    /// `1.0 / BUCKET`, as a count.
    const BUCKETS: usize = 5;

    eprintln!("   -- ring DISK-radius distribution ('denser at the outside', measured) --");
    if report.ring_radii.is_empty() {
        eprintln!("     NOT MEASURED: no rings were placed.\n");
        return;
    }
    let (r_min, r_med, r_max) = min_med_max(&report.ring_radii);
    eprintln!(
        "     rings                         {:>12}",
        report.ring_radii.len()
    );
    eprintln!("     disk radius min / med / max   {r_min:>12.4} / {r_med:.4} / {r_max:.4}");
    eprintln!(
        "     MEDIAN {r_med:.4}  vs  EVEN SPREAD REFERENCE 0.500   (terrain, WITHDRAWN: 0.665)"
    );
    let mut counts = [0usize; BUCKETS];
    for &r in &report.ring_radii {
        let bucket = ((r / BUCKET).floor().max(0.0) as usize).min(BUCKETS - 1);
        counts[bucket] += 1;
    }
    let total = report.ring_radii.len().max(1);
    eprintln!("     {:>12} {:>10} {:>10}", "disk band", "rings", "share %");
    for (b, &count) in counts.iter().enumerate() {
        eprintln!(
            "     {:>5.1}-{:<6.1} {:>10} {:>10.1}",
            BUCKET * b as f64,
            BUCKET * (b as f64 + 1.0),
            count,
            100.0 * count as f64 / total as f64
        );
    }
    eprintln!(
        "     An even spread puts ~{:.0}% in every band. A median ABOVE 0.500 means the rings\n\
         \x20    crowd the RIM; below means they crowd the centre. The map here is not exactly\n\
         \x20    affine, so 0.500 is a reference, not a bar — but a 0.665 needs a mechanism, and\n\
         \x20    on terrain that mechanism was the 30-vertex, ~4.6 mm boundary polygon the ring\n\
         \x20    search had to sweep to.\n",
        100.0 / BUCKETS as f64
    );
}

/// Consecutive-point 3D step statistics for one polyline.
struct StepStats {
    samples: usize,
    median_mm: f64,
    p99_mm: f64,
    max_mm: f64,
}

fn step_stats(points: &[P3]) -> StepStats {
    let mut steps: Vec<f64> = points.windows(2).map(|w| (w[1] - w[0]).norm()).collect();
    steps.sort_by(f64::total_cmp);
    StepStats {
        samples: steps.len(),
        median_mm: percentile(&steps, 0.50),
        p99_mm: percentile(&steps, 0.99),
        max_mm: percentile(&steps, 1.0),
    }
}

/// Consecutive-point step distribution, against the chord the angular lattice
/// implies.
///
/// The withdrawn terrain run carried **2.4 mm jumps confined to the first
/// rings** — swinging across the coarse ellipse boundary — and they were
/// invisible in the retract count because they sit *inside* the single
/// polyline. `max_consecutive_step_mm` alone cannot say WHERE; this block
/// attributes the worst step to a ring index and prints the lattice chord it
/// should have been.
fn print_step_distribution(result: &SpiralResult, report: &SpiralReport, params: &SpiralParams) {
    eprintln!("   -- consecutive-point 3D step distribution --");
    let spiral = step_stats(&result.spiral_contact);
    if spiral.samples == 0 {
        eprintln!("     NOT MEASURED: the spiral carries fewer than two points.\n");
        return;
    }

    let n_c = params.n_angular_samples.max(1) as f64;
    let (l_min, l_med, l_max) = min_med_max(&report.ring_lengths_mm);
    let implied_median = l_med / n_c;
    eprintln!(
        "     spiral steps n={:<7} med {:>9.5} / p99 {:>9.5} / max {:>9.5} mm",
        spiral.samples, spiral.median_mm, spiral.p99_mm, spiral.max_mm
    );
    eprintln!(
        "     ring 3D length min/med/max    {l_min:>10.3} / {l_med:.3} / {l_max:.3} mm over \
         N_C = {} lattice cells",
        params.n_angular_samples
    );
    eprintln!(
        "     IMPLIED CHORD (median ring)   {implied_median:>10.5} mm   = median ring length / N_C\n\
         \x20                                              — the step the angular lattice alone\n\
         \x20                                              predicts, with no bridge and no fold"
    );
    if implied_median > 0.0 {
        eprintln!(
            "     max step / implied chord      {:>10.2}x",
            spiral.max_mm / implied_median
        );
    }

    // Per-RING, so the worst step gets an index rather than an anecdote. The
    // spiral's own max may fall on a BRIDGE segment, which belongs to no
    // single ring — that is why both numbers are printed and neither is
    // presented as the other.
    let mut worst_ring = 0usize;
    let mut worst_step = f64::NEG_INFINITY;
    for (i, ring) in result.rings_contact.iter().enumerate() {
        let stats = step_stats(ring);
        if stats.samples > 0 && stats.max_mm > worst_step {
            worst_step = stats.max_mm;
            worst_ring = i;
        }
    }
    if worst_step.is_finite() {
        let disk_r = report
            .ring_radii
            .get(worst_ring)
            .copied()
            .unwrap_or(f64::NAN);
        eprintln!(
            "     WORST PER-RING STEP           {worst_step:>10.5} mm on RING INDEX {worst_ring} \
             of {} (disk radius {disk_r:.4};\n\
             \x20                                              rings_contact is OUTERMOST-FIRST, so \
             index 0 is the rim)",
            result.rings_contact.len()
        );
    } else {
        eprintln!("     WORST PER-RING STEP           NOT MEASURED (no ring had two points)");
    }
    eprintln!(
        "     A step far above the implied chord is a LIFT IN ALL BUT NAME — the retract count\n\
         \x20    stays 0 because the polyline is unbroken, so this distribution, not that zero,\n\
         \x20    is the evidence of continuity. Terrain (WITHDRAWN) carried 2.4 mm steps at path\n\
         \x20    indices < 600, i.e. on its outermost rings, from its coarse boundary loop.\n"
    );
}

// ── the falsifiable split (FINDINGS_F2 withdrawal block) ────────────────

/// Half-width of the verdict bands, as a fraction of the candidate.
///
/// The two bands are **disjoint** at any target: `0.75·T > 1.25·(T/2)`
/// because `0.75 > 0.625`. So the three branches — near target, near half,
/// neither — partition the line and the INCONCLUSIVE arm is a genuine
/// fallback rather than a hedge.
const VERDICT_BAND: f64 = 0.25;

/// Decide the withdrawal block's falsifiable split on an analytic arm.
///
/// This is the arm's whole job, so it is stated as a verdict and not hedged:
/// the two candidates are 2× apart, their ±25 % bands do not overlap, and the
/// fixture can resolve either.
fn analytic_spacing_verdict(label: &str, spacings_sorted: &[f64], analytic_target_mm: f64) {
    eprintln!("========== VERDICT — the FINDINGS_F2 falsifiable split, on {label} ==========\n");
    eprintln!(
        "   FINDINGS_F2 withdrawal block, restated: terrain measured a median 0.2275 mm against\n\
         \x20  a 0.4862 mm target — almost exactly HALF — and 0.243 mm is precisely the lateral\n\
         \x20  distance at which a K_c = {BALL_RADIUS_MM} ball stops covering the h = {CUSP_HEIGHT_MM} \
         iso-scallop\n\
         \x20  surface. Two explanations survive that observation, and this arm separates them.\n"
    );

    if spacings_sorted.is_empty() {
        eprintln!(
            "   NOT DECIDABLE: no spacing sample survived Stage C, so there is nothing to place\n\
             \x20  against either candidate. This is an instrument failure, not an inconclusive\n\
             \x20  reading — report it as such.\n"
        );
        return;
    }
    if !(analytic_target_mm.is_finite() && analytic_target_mm > 0.0) {
        eprintln!("   NOT DECIDABLE: the analytic target is not a positive number.\n");
        return;
    }

    let half = 0.5 * analytic_target_mm;
    let median = percentile(spacings_sorted, 0.50);
    let (lo_full, hi_full) = (
        analytic_target_mm * (1.0 - VERDICT_BAND),
        analytic_target_mm * (1.0 + VERDICT_BAND),
    );
    let (lo_half, hi_half) = (half * (1.0 - VERDICT_BAND), half * (1.0 + VERDICT_BAND));

    eprintln!("   -- the two candidates, both computed, and the measurement --");
    eprintln!(
        "     CANDIDATE A  full analytic spacing   {analytic_target_mm:>9.5} mm   band \
         [{lo_full:.5}, {hi_full:.5}]"
    );
    eprintln!(
        "     CANDIDATE B  HALF of it              {half:>9.5} mm   band [{lo_half:.5}, \
         {hi_half:.5}]"
    );
    eprintln!(
        "     MEASURED median adjacent-ring 3D spacing over {} samples: {median:.5} mm",
        spacings_sorted.len()
    );
    eprintln!(
        "     p10 / p50 / p90               {:>12.5} / {:.5} / {:.5} mm",
        percentile(spacings_sorted, 0.10),
        median,
        percentile(spacings_sorted, 0.90)
    );
    eprintln!(
        "     The bands are DISJOINT by construction ({lo_full:.5} > {hi_half:.5}), so exactly\n\
         \x20    one of the three branches below can fire.\n"
    );

    let in_full = median >= lo_full && median <= hi_full;
    let in_half = median >= lo_half && median <= hi_half;

    if in_full {
        eprintln!(
            "   VERDICT: **FIXTURE DEFECT CONFIRMED — the algorithm's spacing is CORRECT.**\n\
             \x20  The measured median sits inside +/-{:.0}% of the analytic constant-scallop\n\
             \x20  spacing on a surface whose facets are ~3x FINER than the quantity, so the\n\
             \x20  terrain run's 0.2275 mm was FACETING, exactly as the withdrawal block\n\
             \x20  suspected. There is no off-by-one-band defect in the ring recursion.\n\
             \x20  Consequence: FINDINGS_F2 §F2-1's spacing table, its '~2x over-cover' reading\n\
             \x20  and the '3.4x slower than raster' headline stay WITHDRAWN, and the repair is\n\
             \x20  the fixture — which is this arm.",
            VERDICT_BAND * 100.0
        );
    } else if in_half {
        eprintln!(
            "   VERDICT: **OFF-BY-ONE-BAND DEFECT IN THE RING RECURSION.**\n\
             \x20  The measured median sits inside +/-{:.0}% of HALF the analytic spacing on a\n\
             \x20  fixture that can resolve either, so faceting is excluded and the terrain\n\
             \x20  number was the same real defect seen through a bad fixture: the coverage\n\
             \x20  search is not crediting the band the PREVIOUS ring already covered, so each\n\
             \x20  ring is placed one HALF-WIDTH ({half:.5} mm) from its neighbour instead of one\n\
             \x20  full width. That is a src/ defect in conformal_spiral's Eqs. 1-4 search, and\n\
             \x20  it doubles cutting distance on every region it plans.\n\
             \x20  Note what it does NOT contradict: the module's flat-disk unit test asserts\n\
             \x20  stepover within +/-25% and passes on an exact-affine fixture, so the defect\n\
             \x20  has to be in a path that fixture does not exercise.",
            VERDICT_BAND * 100.0
        );
    } else {
        eprintln!(
            "   VERDICT: **INCONCLUSIVE** — the median lands in neither band.\n\
             \x20  Report the rows; do NOT round it to whichever story was expected. This branch\n\
             \x20  is live precisely so a mixed reading cannot be quietly resolved. Before\n\
             \x20  reading anything into it, check the Stage-A N_S adequacy ratio and the Stage-C2\n\
             \x20  ring-radius median: a ratio at or above 1.0 makes the coverage predicate\n\
             \x20  optimistic and the spacing read long, which is a THIRD mechanism and is\n\
             \x20  distinguishable from both candidates above."
        );
    }
    eprintln!(
        "\n     ratio measured / full {:.4}x, measured / half {:.4}x\n",
        median / analytic_target_mm,
        median / half
    );
}

// ── STAGE F2-D — CL conversion, containment, F-034 cost ─────────────────

struct StageDOutcome {
    cost: CandidateCost,
    cl_points: usize,
}

/// An area-weighted distribution over a triangle selection.
///
/// **Area-weighted on purpose.** The polar sphere-cap mesh's triangles are
/// far from equal in area (the innermost band's are ~55× smaller than the
/// rim's), so an unweighted percentile over triangles would report the
/// geometry of the mesh's centre rather than of the surface. Weighting by 3D
/// area makes every percentile a statement about *ground covered*, which is
/// what a finish spacing is about.
struct AreaWeighted {
    samples: usize,
    total_area_mm2: f64,
    min: f64,
    p50: f64,
    p90: f64,
    max: f64,
}

/// Area-weighted percentiles. `pairs` is `(value, area)`; it is sorted here.
fn area_weighted(mut pairs: Vec<(f64, f64)>) -> AreaWeighted {
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    let at = |fraction: f64| -> f64 {
        if pairs.is_empty() || total <= 0.0 {
            return f64::NAN;
        }
        let target = fraction * total;
        let mut running = 0.0f64;
        for &(value, weight) in &pairs {
            running += weight;
            if running >= target {
                return value;
            }
        }
        pairs.last().map_or(f64::NAN, |p| p.0)
    };
    AreaWeighted {
        samples: pairs.len(),
        total_area_mm2: total,
        min: pairs.first().map_or(f64::NAN, |p| p.0),
        p50: at(0.50),
        p90: at(0.90),
        max: pairs.last().map_or(f64::NAN, |p| p.0),
    }
}

/// What a 0° ball-end raster **actually leaves on the surface**, per region
/// triangle.
///
/// The fairness defect FINDINGS_F2 records, restated: the spiral spaces on
/// the **3D surface** while `raster_candidate` spaces in **XY projection**.
/// On sloped ground the raster's passes end up further apart along the
/// surface than the XY stepover says, so it under-covers exactly where the
/// spiral covers correctly, and the two arms were never delivering the same
/// finish ([[instrument-integrity]]: both sides of a ratio must be the same
/// measure).
///
/// Two quantities are returned, and they answer different questions:
///
/// * **cross-feed** (the headline, and the honest one for a `0°` raster):
///   passes advance along `+Y`, so what matters is the surface climb across
///   `Y` alone. Moving `Δy` in XY moves `Δy·√(1 + (n_y/n_z)²)` along the
///   surface, so `achieved = stepover · √(1 + (n_y/n_z)²)`.
/// * **isotropic** `stepover / n_z` — the direction-agnostic bound, i.e. what
///   a raster running in the *worst* direction for each triangle would leave.
///   It is an upper bound on the first, printed so a reader can see how much
///   of the gap is the raster's direction choice rather than the slope.
fn achieved_raster_spacing(
    mesh: &TriangleMesh,
    region: &[u32],
    stepover_mm: f64,
) -> (AreaWeighted, AreaWeighted) {
    let mut cross: Vec<(f64, f64)> = Vec::with_capacity(region.len());
    let mut isotropic: Vec<(f64, f64)> = Vec::with_capacity(region.len());
    for &t in region {
        let Some(face) = mesh.faces.get(t as usize) else {
            continue;
        };
        // +Z-forced, matching `build_region_mesh`'s convention (and the
        // curvature census's) so the two blocks cannot disagree about which
        // side of the sheet they mean.
        let normal = if face.normal.z < 0.0 {
            -face.normal
        } else {
            face.normal
        };
        if normal.z <= 1e-9 {
            continue;
        }
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        let area = 0.5 * e1.cross(&e2).norm();
        let ratio = normal.y / normal.z;
        cross.push((stepover_mm * (1.0 + ratio * ratio).sqrt(), area));
        isotropic.push((stepover_mm / normal.z, area));
    }
    (area_weighted(cross), area_weighted(isotropic))
}

fn stage_d(
    fixture: Fixture<'_>,
    region: &Region,
    result: &SpiralResult,
    report: &SpiralReport,
    stepover_mm: f64,
    spacings_sorted: &[f64],
    kind: ArmKind,
) -> Option<StageDOutcome> {
    use rs_cam_core::region_set::RegionSet;

    let Fixture {
        mesh,
        index,
        cutter,
        kinematics,
        safe_z,
        effective_min_z,
    } = fixture;

    eprintln!(
        "========== STAGE F2-D — CL conversion, containment, F-034 cost CONTEXT ==========\n"
    );
    if kind == ArmKind::FixtureLimited {
        eprintln!("   {WITHDRAWAL_BANNER}\n");
    }

    let contact = vec![result.spiral_contact.clone()];
    let converted = cl_polylines(&contact, mesh, index, cutter);
    eprintln!("   -- contact -> cutter-centre (point_drop_cutter, rs_cam convention) --");
    eprintln!(
        "     contact points in             {:>10}",
        converted.input_points
    );
    eprintln!(
        "     points dropped (no contact)   {:>10}",
        converted.dropped_points
    );
    eprintln!(
        "     polylines dropped (< 2 pts)   {:>10}",
        converted.dropped_polylines
    );
    eprintln!(
        "     CL polylines out              {:>10}   (1 = continuity survived the conversion)",
        converted.polylines.len()
    );
    let cl_points: usize = converted.polylines.iter().map(Vec::len).sum();
    eprintln!("     CL points out                 {cl_points:>10}");
    if converted.polylines.is_empty() {
        eprintln!("\n   REFUSE: no CL polyline survived the conversion.\n");
        return None;
    }
    eprintln!(
        "     The module header is explicit that this drop-cutter CL SUPERSEDES its own\n\
         \x20    normal-offset centre curve: the paper has no gouge handling anywhere\n\
         \x20    (extraction gap 3), and drop-cutter projection is the rs_cam convention."
    );

    let raw = polylines_to_toolpath(&converted.polylines, FEED_MM_MIN, PLUNGE_MM_MIN, safe_z);

    // -- containment: COUNTED, not clipped --
    let outside = |tp: &Toolpath| -> (usize, usize) {
        let mut cutting = 0usize;
        let mut escapes = 0usize;
        for mv in &tp.moves {
            if !mv.move_type.is_cutting() {
                continue;
            }
            cutting += 1;
            if !region
                .polygon
                .contains_point(&P2::new(mv.target.x, mv.target.y))
            {
                escapes += 1;
            }
        }
        (cutting, escapes)
    };
    let (cutting_moves, escapes) = outside(&raw);
    eprintln!("\n   -- containment: cutting moves inside the region ellipse --");
    eprintln!("     cutting moves                 {cutting_moves:>10}");
    eprintln!(
        "     outside the region            {escapes:>10}   ({:.3}%)",
        100.0 * escapes as f64 / cutting_moves.max(1) as f64
    );
    eprintln!(
        "     DELIBERATE DEVIATION FROM F1 (direction_field_wanaka_f1.rs:983-1010): escapes are\n\
         \x20    COUNTED, NOT CLIPPED. `boundary::clip_toolpath_to_boundary` SPLITS a polyline —\n\
         \x20    and single-polyline continuity is the exact quantity this phase exists to\n\
         \x20    measure, so a clip would fabricate retracts into the very arm whose retract\n\
         \x20    count is the claim. A nonzero escape count is the LATERAL CL SHIFT: a contact\n\
         \x20    point just inside the ellipse can put its cutter centre just outside it on a\n\
         \x20    slope. The ellipse is 8 mm inside the mesh, so an escape leaves the REGION,\n\
         \x20    never the MESH."
    );

    // -- cost --
    eprintln!("\n   -- F-034 cost through the SAME production relink as the baseline --");
    eprintln!("     {FRESH_STOCK_LABEL}");
    let region_set = RegionSet::new(vec![region.polygon.clone()]);
    let spiral_cost = relink_and_cost(raw, mesh, index, cutter, &region_set, &kinematics, safe_z);

    let zero_grid = grid_for_direction(mesh, index, cutter, stepover_mm, 0.0);
    let raster_cost = relink_and_cost(
        raster_candidate(
            &zero_grid,
            std::slice::from_ref(&region.polygon),
            safe_z,
            effective_min_z,
        ),
        mesh,
        index,
        cutter,
        &region_set,
        &kinematics,
        safe_z,
    );

    eprintln!(
        "\n     {:<30} {:>8} {:>10} {:>8} {:>10} {:>10} {:>9}",
        "arm", "moves", "fragments", "linked", "RETRACTS", "cut mm", "time s"
    );
    for (label, cost) in [
        ("conformal spiral (1 polyline)", &spiral_cost),
        ("0° raster (ball, same region)", &raster_cost),
    ] {
        eprintln!(
            "     {:<30} {:>8} {:>10} {:>8} {:>10} {:>10.1} {:>9.1}",
            label,
            cost.moves,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.cutting_mm,
            cost.time_s
        );
    }
    eprintln!(
        "\n     RETRACTS and cut mm are the columns this phase is about. The spiral's pitch is\n\
         \x20    STAY-DOWN CONTINUITY: it enters once and never lifts, so its raw path carries\n\
         \x20    exactly one closing Retract and `kept_retracts` should read 1 (or 0 if the\n\
         \x20    relinker absorbs it). Every retract in the raster row is a hop the spiral does\n\
         \x20    not pay for. `linked` counts surface links the relinker ADDED — a spiral with\n\
         \x20    one fragment has nothing to link."
    );
    // -- THE FAIRNESS FIX (FINDINGS_F2's second, independent defect) --
    //
    // This block exists because the raw time ratio above it was being read as
    // a verdict while the two rows were not delivering the same finish. It is
    // printed BEFORE the ratio so the ratio arrives already qualified.
    let (cross, isotropic) = achieved_raster_spacing(mesh, &region.triangles, stepover_mm);
    eprintln!("\n   -- ACHIEVED SURFACE SPACING: are the two rows even the same finish? --");
    eprintln!(
        "     The spiral spaces on the 3D SURFACE; `raster_candidate` spaces in XY PROJECTION.\n\
         \x20    On slope the raster's passes land further apart along the surface than its XY\n\
         \x20    stepover says, so it UNDER-COVERS exactly where the spiral covers correctly.\n\
         \x20    Both columns below are area-weighted over the region's own triangles.\n"
    );
    eprintln!(
        "     {:<44} {:>10} {:>10} {:>10} {:>10}",
        "quantity (mm)", "min", "median", "p90", "max"
    );
    eprintln!(
        "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "spiral: MEASURED adjacent-ring 3D spacing",
        percentile(spacings_sorted, 0.0),
        percentile(spacings_sorted, 0.50),
        percentile(spacings_sorted, 0.90),
        percentile(spacings_sorted, 1.0)
    );
    eprintln!(
        "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "raster: achieved surface spacing (0deg, Y)", cross.min, cross.p50, cross.p90, cross.max
    );
    eprintln!(
        "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "raster: isotropic bound stepover / n_z",
        isotropic.min,
        isotropic.p50,
        isotropic.p90,
        isotropic.max
    );
    eprintln!(
        "     {:<44} {:>10.5}",
        "COMMANDED XY stepover (both arms)", stepover_mm
    );
    eprintln!(
        "     ({} region triangles, {:.2} mm² of 3D area weighted)",
        cross.samples, cross.total_area_mm2
    );
    eprintln!(
        "\n     A LIKE-FOR-LIKE TIME COMPARISON IS NOT POSSIBLE WITHOUT CHANGING THE RASTER.\n\
         \x20    The raster row above would have to be re-run at an XY stepover scaled by\n\
         \x20    cos(local slope) — a variable-stepover raster this file does not have and may\n\
         \x20    not add, because `raster_candidate` is restated verbatim from F1 and changing\n\
         \x20    it here would break the cross-instrument comparability that restatement buys.\n\
         \x20    Until that exists, the ratio below is a RAW OBSERVATION, NOT A VERDICT: it\n\
         \x20    compares a path that holds the scallop against one that does not."
    );
    if kind == ArmKind::Analytic {
        eprintln!(
            "\x20    Second confound, on ARM SPHERE specifically: its region is the WHOLE cap, so\n\
             \x20    a ball dropped at the boundary rests on the mesh EDGE (rim roll-off). Both\n\
             \x20    rows pay it identically, but it depresses both. ARM WAVY does not have it —\n\
             \x20    its machined circle leaves a 2 mm margin of mesh outside the region."
        );
    }
    // -- THIRD confound, and the one the ARM SPHERE hole exposed: a time
    //    comparison between a COMPLETE path and an INCOMPLETE one is not a
    //    comparison at all. The sphere run's "spiral 18.3 s vs raster 22.0 s"
    //    was a spiral that skipped 23.5% of the region.
    eprintln!("\n   -- IS EACH ROW EVEN FINISHING THE PART? --");
    match report.coverage_audit.as_ref() {
        Some(audit) => eprintln!(
            "     spiral: UNMACHINED {:.4}% of region area ({:.4} of {:.4} mm²), \
             {} of {} triangles.",
            100.0 * audit.unmachined_area_fraction,
            audit.unmachined_area_mm2,
            audit.region_area_mm2,
            audit.uncovered_centroid_triangles,
            audit.centroids_tested
        ),
        None => eprintln!(
            "     spiral: coverage audit NOT MEASURED — completeness is UNKNOWN, so the ratio\n\
             \x20    below cannot be read as a like-for-like comparison at all."
        ),
    }
    eprintln!(
        "     raster: not audited here — `raster_candidate` is restated verbatim from F1 and\n\
         \x20    carries no coverage audit; its completeness is unmeasured on this row.\n\
         \x20    A TIME RATIO BETWEEN A COMPLETE PATH AND AN INCOMPLETE ONE IS MEANINGLESS.\n\
         \x20    On the ARM SPHERE run the spiral read 18.3 s against the raster's 22.0 s while\n\
         \x20    leaving a 2.783 mm-radius hole — the spiral was faster because it was not\n\
         \x20    machining a quarter of the part. Read the unmachined line above BEFORE the\n\
         \x20    ratio below, every time."
    );
    if spiral_cost.time_s > 0.0 {
        eprintln!(
            "     RAW time ratio raster / spiral: {:.3}x  <<< NOT A VERDICT — see the three \
             qualifications above",
            raster_cost.time_s / spiral_cost.time_s
        );
    }
    eprintln!(
        "\n     -- F-034 vs RASTER IS CONTEXT, NOT A PHASE-1 BAR --\n\
         \x20    research/conformal_finish_2026-08-28.md §B.4, verbatim: \"F-034 vs ball-end\n\
         \x20    raster on the friendly fixture is context, not a bar.\" The region here is a\n\
         \x20    CONVEX, hole-free ellipse 8 mm inside the mesh — chosen so every centroid test\n\
         \x20    is clean, which is precisely the geometry a raster handles best. The phase-1\n\
         \x20    bars are the four conditions in Stage A. PROGRAMME.md's advance bar for\n\
         \x20    F2 as a whole is explicit that \"a lower retract count alone is not\n\
         \x20    sufficient\", and it is stated against WANAKA regions (step 4), not this one.\n"
    );

    Some(StageDOutcome {
        cost: spiral_cost,
        cl_points,
    })
}

// ── STAGE F2-E — sampling sensitivity ───────────────────────────────────

fn stage_e(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    region: &Region,
    base_params: &SpiralParams,
    base_report: &SpiralReport,
) {
    eprintln!("========== STAGE F2-E — sampling sensitivity (N_S, N_C) ==========\n");
    eprintln!(
        "   G-SAMPLING: NEITHER paper states a rule for N_S, N_C or ε (2504.06310 extraction\n\
         \x20  gap 6). Both are [REPO] choices, and the 2025 paper demonstrates two failure\n\
         \x20  modes rather than a rule — Table 1 cases 1.4 and 1.5. Three rows of direct\n\
         \x20  evidence on this fixture beat a citation.\n\
         \x20  Every row sweeps the start angle at the SAME coarsened {SWEEP_START_ANGLE_STEP:.6}\n\
         \x20  rad, so the only variable per row is the sampling dial.\n"
    );

    /// A refusal is not a hole in this table — it IS the sensitivity
    /// evidence (a `RingSearchStalled` at halved `N_S` is precisely
    /// [SOURCE-2025] Table 1 case 1.5), so the typed reason is CARRIED, not
    /// flattened to the word "REFUSED".
    struct Row {
        label: String,
        n_s: usize,
        n_c: usize,
        outcome: Result<SpiralReport, SpiralRefusal>,
    }

    // The baseline row's params were printed in Stage A, three screens up.
    // Reprint them here so this table is self-contained — an evidence file
    // gets quoted as an excerpt, which is the same reason FRESH_STOCK_LABEL
    // is repeated at every costing surface.
    eprintln!("   baseline row (the Stage A run, not re-run):");
    print_params("baseline", base_params);

    let mut rows: Vec<Row> = vec![Row {
        label: "baseline (N_S, N_C)".to_owned(),
        n_s: base_params.n_surface_samples,
        n_c: base_params.n_angular_samples,
        outcome: Ok(base_report.clone()),
    }];

    for (label, n_s, n_c) in [
        (
            "half N_S (N_S/2, N_C)",
            base_params.n_surface_samples / 2,
            base_params.n_angular_samples,
        ),
        (
            "half N_C (N_S, N_C/2)",
            base_params.n_surface_samples,
            base_params.n_angular_samples / 2,
        ),
    ] {
        let mut params = base_params.clone();
        params.n_surface_samples = n_s;
        params.n_angular_samples = n_c;
        eprintln!("   re-running plan_spiral:");
        print_params(label, &params);
        // Report first, always; outcome second. On a refusal the report is
        // still the row's evidence, which is why the refusal arm carries the
        // reason rather than a bare "REFUSED".
        let (row_report, row_outcome) =
            conformal_spiral::plan_spiral(mesh, index, &region.triangles, &params);
        let outcome = match row_outcome {
            Ok(_) => Ok(row_report),
            Err(refusal) => {
                eprintln!("     REFUSAL: {refusal:?}");
                if let Some(stall) = row_report.stall.as_ref() {
                    eprintln!(
                        "       stall: {} rings placed, {} uncovered, fold census {}, \
                         near-band median {} mm against K_c {:.4}",
                        stall.rings_placed,
                        stall.uncovered,
                        stall.uncovered_outside_last_ring,
                        if stall.near_band.samples == 0 {
                            "EMPTY".to_owned()
                        } else {
                            format!("{:.4}", stall.near_band.median_mm)
                        },
                        stall.coverage_radius_mm
                    );
                }
                Err(refusal)
            }
        };
        rows.push(Row {
            label: label.to_owned(),
            n_s,
            n_c,
            outcome,
        });
    }

    // `starved` and `unmachined%` are the two columns that make this table
    // mean anything. Halving N_S is EXACTLY the lever that produced the ARM
    // SPHERE hole, so a sensitivity sweep without them would demonstrate the
    // defect and report it as three healthy rows.
    eprintln!(
        "\n     {:<24} {:>8} {:>6} {:>7} {:>9} {:>12} {:>10} {:>11} {:>10}",
        "row",
        "N_S",
        "N_C",
        "rings",
        "starved",
        "uncov(bridge)",
        "overhead%",
        "unmachined%",
        "spiral mm"
    );
    for row in &rows {
        match &row.outcome {
            Ok(r) => eprintln!(
                "     {:<24} {:>8} {:>6} {:>7} {:>9} {:>12} {:>10.3} {:>11} {:>10.1}",
                row.label,
                row.n_s,
                row.n_c,
                r.ring_count,
                r.triangles_without_samples,
                r.uncovered_after_bridging,
                r.bridge_overhead_pct,
                r.coverage_audit.as_ref().map_or_else(
                    || "NOT MEAS".to_owned(),
                    |a| format!("{:.4}", 100.0 * a.unmachined_area_fraction)
                ),
                r.spiral_length_mm
            ),
            // The typed reason, not the word "REFUSED": a refusal at halved
            // sampling is a RESULT of this stage, and which arm fired is the
            // whole content of it.
            Err(refusal) => eprintln!(
                "     {:<24} {:>8} {:>6}   REFUSED: {refusal:?}",
                row.label, row.n_s, row.n_c
            ),
        }
    }
    eprintln!(
        "\n     What to read here. `uncov(bridge)` at 0 across all three rows does NOT mean the\n\
         \x20    sampling is adequate — it means every sample that EXISTS is covered, which is\n\
         \x20    trivially easier with fewer samples. That column is the SEARCH's self-report;\n\
         \x20    on the ARM SPHERE run it read 0 while a quarter of the region went unmachined.\n\
         \x20    `starved` (triangles apportioned zero samples, must be 0) and `unmachined%`\n\
         \x20    (the independent centroid audit) are the columns that can actually fail here,\n\
         \x20    and halving N_S is precisely the lever that produced that hole — so if this\n\
         \x20    sweep is going to break anything, it breaks those two.\n\
         \x20    Beyond that: if halving N_S REDUCES the ring count, the coverage predicate was\n\
         \x20    already optimistic at the baseline and the spacing is reading long — Table 1\n\
         \x20    case 1.5. Halving N_C coarsens the shared angular lattice that both runs and\n\
         \x20    bridges are sampled on, so it moves chord length, and through it the measured\n\
         \x20    spiral length and the disk self-intersection count, without moving the\n\
         \x20    mechanism.\n"
    );
}

// ── one arm's staged evidence, end to end ───────────────────────────────

/// Everything that differs between arms once a spiral has actually been
/// planned. Bundled for the same reason [`Fixture`] is: clippy's
/// `too_many_arguments` fires at eight.
struct ArmRun<'a> {
    label: &'a str,
    /// Filename stem for this arm's two SVGs. The terrain FLAT arm keeps its
    /// historical stem so the artifacts FINDINGS_F2 §F2-1 names by filename
    /// stay findable even though their numbers are withdrawn.
    slug: &'a str,
    kind: ArmKind,
    /// The constant-scallop spacing this arm's surface implies in closed
    /// form. `Some` only on the ANALYTIC arms — terrain's would be computed
    /// from a curvature census that is itself a faceting artefact, which is
    /// exactly the circularity the withdrawal is about.
    analytic_target_mm: Option<f64>,
    /// Whether Stage E's sampling sensitivity re-runs here.
    run_stage_e: bool,
}

fn run_arm_evidence(
    run: &ArmRun<'_>,
    fixture: Fixture<'_>,
    region: &Region,
    report: &SpiralReport,
    result: &SpiralResult,
    params: &SpiralParams,
    stepover_mm: f64,
) {
    eprintln!("\n══════════ STAGED EVIDENCE — {} ══════════\n", run.label);
    if run.kind == ArmKind::FixtureLimited {
        eprintln!("   {WITHDRAWAL_BANNER}\n");
    }

    let region_area_3d = region_area_mm2(fixture.mesh, &region.triangles);
    let proceed = stage_a(run.label, report, params, stepover_mm, region_area_3d);
    stage_b(region, result, report, run.slug);

    if proceed {
        let curvature = curvature_census(fixture.mesh, &region.triangles);
        let spacings = stage_c(result, stepover_mm, &curvature, run.kind);
        let samples: &[f64] = spacings.as_deref().unwrap_or(&[]);

        eprintln!("========== STAGE F2-C2 — ring placement and step continuity ==========\n");
        if run.kind == ArmKind::FixtureLimited {
            eprintln!("   {WITHDRAWAL_BANNER}\n");
        }
        print_ring_radius_distribution(report);
        print_step_distribution(result, report, params);

        if let Some(target) = run.analytic_target_mm {
            analytic_spacing_verdict(run.label, samples, target);
        }

        if let Some(outcome) = stage_d(
            fixture,
            region,
            result,
            report,
            stepover_mm,
            samples,
            run.kind,
        ) {
            eprintln!(
                "   Stage D summary: {} CL points on one polyline, {} kept retracts, {:.1} mm \
                 cutting, {:.1} s.\n",
                outcome.cl_points,
                outcome.cost.kept_retracts,
                outcome.cost.cutting_mm,
                outcome.cost.time_s
            );
        }
    } else {
        eprintln!("########## Stages C, C2 and D SKIPPED by the Stage A falsifier. ##########\n");
    }

    if run.run_stage_e {
        stage_e(fixture.mesh, fixture.index, region, params, report);
    } else {
        eprintln!(
            "########## STAGE F2-E SKIPPED on {} — BY DECISION, NOT OMISSION. ##########\n\
             \x20  Stage E varies (N_S, N_C). On this arm the Stage-A N_S ADEQUACY block already\n\
             \x20  answers the sampling question quantitatively — implied sample spacing against\n\
             \x20  stepover/2 — and two further plan_spiral solves over a region this size buy\n\
             \x20  nothing the phase-1 spacing verdict needs. The sensitivity rows, including the\n\
             \x20  N_C-halving result (structural, and one of the few things the FINDINGS_F2\n\
             \x20  withdrawal leaves standing), are measured on ARM FLAT.\n",
            run.label
        );
    }
}

/// Plan one ANALYTIC arm on an explicit, already-clean triangle selection.
///
/// Deliberately **not** [`plan_arm`]. That function's job is to survive
/// *centroid-selection artefacts* on terrain — a ragged fringe, stranded
/// islands, bowtie vertices — by shrinking the ellipse and retrying. An
/// analytic fixture has none of those by construction, and a shrink retry
/// here would quietly machine a different region from the one whose analytic
/// target the verdict is measured against. So the selection is taken as
/// given, put through [`clean_selection`] to **prove** it needs no hygiene,
/// and handed to `plan_spiral` exactly once.
fn plan_analytic_arm(
    label: &'static str,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    triangles: Vec<u32>,
    polygon: Polygon2,
    semi_axes: (f64, f64),
    params: &SpiralParams,
) -> Arm {
    let (cleaned, cleanup) = clean_selection(mesh, &triangles);
    eprintln!(
        "   selection hygiene on the analytic region: {} -> {} triangles, {} component(s) on the\n\
         \x20  first pass, {} pinch pass(es), {} triangle(s) shaved. On an analytic fixture every\n\
         \x20  one of those must be a NO-OP — that is the evidence the region carries no\n\
         \x20  selection artefact for a measurement to inherit.",
        cleanup.before,
        cleanup.after,
        cleanup.components_first_pass,
        cleanup.pinch_iterations,
        cleanup.triangles_shaved_by_pinch
    );
    assert_eq!(
        cleaned, triangles,
        "{label}: the analytic region must need NO selection hygiene. clean_selection removed \
         triangles, which means the generator or the region selector produced a non-manifold or \
         disconnected patch, and every number downstream would inherit it."
    );

    let (report, outcome) = conformal_spiral::plan_spiral(mesh, index, &triangles, params);
    Arm {
        label,
        region: Region {
            polygon,
            triangles,
            semi_axes,
            shrinks: 0,
            cleanup,
        },
        report,
        outcome,
    }
}

// ── ARM SPHERE — the decisive analytic arm ──────────────────────────────

fn arm_sphere(
    cutter: &BallEndmill,
    kinematics: rs_cam_core::machine_kinematics::MachineKinematics,
    stepover_mm: f64,
    params: &SpiralParams,
) {
    eprintln!(
        "\n══════════ ARM SPHERE — analytic spherical cap (THE DECISIVE ARM) ══════════\n\
         \x20  A refusal on this arm is a HARD FAILURE: the region is a convex, hole-free,\n\
         \x20  exactly-manifold disk with an exact circular boundary and facets 3.1x finer than\n\
         \x20  the measurand at their WORST (4.4x at the median). There is no fixture excuse\n\
         \x20  left.\n"
    );

    let mesh = sphere_cap_mesh(
        SPHERE_RADIUS_MM,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_RINGS,
        SPHERE_CAP_SECTORS,
    );
    let index = SpatialIndex::build_auto(&mesh);
    let triangles = sphere_cap_region(&mesh);

    eprintln!(
        "   fixture: sphere_cap_mesh(R_s = {SPHERE_RADIUS_MM} mm, cap radius = \
         {SPHERE_CAP_RADIUS_MM} mm,\n\
         \x20           {SPHERE_CAP_RINGS} rings x {SPHERE_CAP_SECTORS} sectors) — a convex-UP dome,\n\
         \x20           z(r) = sqrt(R_s^2 - r^2) - sqrt(R_s^2 - a^2).\n\
         \x20  machined circle {:.1} mm across, relief {:.4} mm, convex curvature 1/R_s = \
         {:.5} 1/mm.",
        2.0 * SPHERE_CAP_RADIUS_MM,
        mesh.bbox.max.z - mesh.bbox.min.z,
        1.0 / SPHERE_RADIUS_MM
    );
    let census = mesh_census(&mesh, &triangles);
    print_mesh_census("ARM SPHERE", &census, stepover_mm);

    // -- the target, derived twice, before anything is planned --
    let curved = scallop_math::stepover_from_scallop_curved(
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
        1.0 / SPHERE_RADIUS_MM,
    );
    let coverage = sphere_coverage_spacing_mm(SPHERE_RADIUS_MM, BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let flat = scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    eprintln!("   -- THE ANALYTIC TARGET, DERIVED TWICE (this is why the sphere is decisive) --");
    eprintln!(
        "     production scallop_math::stepover_from_scallop_curved(K_c, h, +1/R_s)  \
         {curved:>9.5} mm"
    );
    eprintln!(
        "     the module's OWN coverage law on three concentric spheres            \
         {coverage:>9.5} mm"
    );
    eprintln!(
        "     the FLAT law, for scale                                              {flat:>9.5} mm"
    );
    eprintln!(
        "     agreement between the two derivations: {:.3}%. They are independent — one comes\n\
         \x20    from rs_cam's effective-radius production formula, the other straight from\n\
         \x20    SpiralParams' definition of S^h and K_c coverage — so the target is not a\n\
         \x20    restatement of the thing being tested. The curvature correction against the flat\n\
         \x20    law is only {:.1}%, which is DELIBERATE: the verdict split is target-vs-HALF\n\
         \x20    (a factor of 2), so it cannot turn on which of the two targets a reader picks.\n",
        100.0 * (curved - coverage).abs() / curved,
        100.0 * (flat - curved).abs() / flat
    );

    // The region boundary IS the mesh rim, so the containment polygon is the
    // rim itself: same vertex count, same radius, exactly coincident.
    let polygon = ellipse_polygon(
        0.0,
        0.0,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_SECTORS,
    );
    let arm = plan_analytic_arm(
        "ARM SPHERE",
        &mesh,
        &index,
        triangles,
        polygon,
        (SPHERE_CAP_RADIUS_MM, SPHERE_CAP_RADIUS_MM),
        params,
    );
    let Arm {
        label,
        region,
        report,
        outcome,
    } = arm;

    eprintln!();
    print_radial_profile(label, &report);
    let result = match outcome {
        Ok(result) => result,
        Err(refusal) => {
            diagnose_refusal(label, &report, &refusal, params);
            panic!(
                "{label} refused with {refusal:?}. On the analytic arm this is a HARD FAILURE: \
                 the fixture is a convex manifold disk with an exact circular boundary and \
                 sub-{ANALYTIC_MAX_EDGE_MM} mm facets, so no fixture defect can be blamed. The \
                 diagnosis is printed above. ONE exception to read before concluding anything \
                 about the mechanism: if the refusal is FlattenDidNotConverge, that is a SOLVER \
                 budget, not a finding — raise ANALYTIC_SOLVER_MAX_SWEEPS (currently {}) and \
                 re-run. Coarsening the mesh is NOT the fix: the sub-{ANALYTIC_MAX_EDGE_MM} mm \
                 edge is the entire reason this arm can measure a {stepover_mm:.4} mm stepover.",
                params.solver_max_sweeps
            );
        }
    };

    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter,
        kinematics,
        safe_z: mesh.bbox.max.z + 5.0,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };
    run_arm_evidence(
        &ArmRun {
            label,
            slug: "sphere_cap_conformal_spiral",
            kind: ArmKind::Analytic,
            analytic_target_mm: Some(curved),
            run_stage_e: false,
        },
        fixture,
        &region,
        &report,
        &result,
        params,
        stepover_mm,
    );
}

// ── ARM WAVY — the realistic-but-clean analytic arm ─────────────────────

fn arm_wavy(
    cutter: &BallEndmill,
    kinematics: rs_cam_core::machine_kinematics::MachineKinematics,
    stepover_mm: f64,
    params: &SpiralParams,
) {
    eprintln!(
        "\n══════════ ARM WAVY — analytic undulating heightfield ══════════\n\
         \x20  The realistic-but-clean case: curvature of BOTH signs, real slope, and still\n\
         \x20  facets under {ANALYTIC_MAX_EDGE_MM} mm. A refusal here is a HARD FAILURE.\n"
    );

    let mesh = wavy_patch_mesh(
        WAVY_SIZE_MM,
        WAVY_AMPLITUDE_MM,
        WAVY_WAVELENGTH_MM,
        ANALYTIC_MAX_EDGE_MM,
    );
    let cells = wavy_grid_cells(
        WAVY_SIZE_MM,
        WAVY_AMPLITUDE_MM,
        WAVY_WAVELENGTH_MM,
        ANALYTIC_MAX_EDGE_MM,
    );
    let index = SpatialIndex::build_auto(&mesh);
    let triangles = wavy_region_triangles(WAVY_SIZE_MM, cells, WAVY_REGION_RADIUS_MM);

    let k = TAU / WAVY_WAVELENGTH_MM;
    let max_gradient = WAVY_AMPLITUDE_MM * k;
    let max_slope_deg = max_gradient.atan().to_degrees();
    let peak_curvature = WAVY_AMPLITUDE_MM * k * k;
    eprintln!(
        "   fixture: wavy_patch_mesh(size {WAVY_SIZE_MM} mm, A {WAVY_AMPLITUDE_MM} mm, L \
         {WAVY_WAVELENGTH_MM} mm, edge <= {ANALYTIC_MAX_EDGE_MM} mm)\n\
         \x20           z = A*sin(2*pi*x/L)*sin(2*pi*y/L) on a {cells} x {cells} grid, cell \
         {:.5} mm.\n\
         \x20  |grad z|max = A*k = {max_gradient:.5}  =>  MAX SLOPE {max_slope_deg:.2} deg (bar: \
         under ~30 deg).\n\
         \x20  peak |kappa| = A*k^2 = {peak_curvature:.5} 1/mm  =>  R_min {:.3} mm against K_c \
         {BALL_RADIUS_MM} mm —\n\
         \x20  the ball fits every concavity, so no S^h point is unreachable at any spacing.\n\
         \x20  machined circle {:.1} mm across, {WAVY_REGION_RADIUS_MM} mm radius, leaving a \
         {:.1} mm margin of mesh.",
        WAVY_SIZE_MM / cells as f64,
        1.0 / peak_curvature,
        2.0 * WAVY_REGION_RADIUS_MM,
        0.5 * WAVY_SIZE_MM - WAVY_REGION_RADIUS_MM
    );
    let census = mesh_census(&mesh, &triangles);
    print_mesh_census("ARM WAVY", &census, stepover_mm);
    eprintln!(
        "   NOTE the boundary loop length: the region is whole GRID CELLS inside a circle, so its\n\
         \x20  boundary is a STAIRCASE and reads ~4/pi = 1.27x the smooth circumference\n\
         \x20  (2*pi*{WAVY_REGION_RADIUS_MM} = {:.3} mm). That is a property of the digitisation, \
         NOT a defect —\n\
         \x20  the staircase amplitude is one cell ({:.5} mm), ~4.6x below the {stepover_mm:.4} mm\n\
         \x20  being measured, so it cannot do what terrain's 4.6 mm boundary segments did.\n",
        TAU * WAVY_REGION_RADIUS_MM,
        WAVY_SIZE_MM / cells as f64
    );

    // The target: the ring search sizes every ring by its single WORST
    // uncovered point, and convex curvature is what NARROWS the admissible
    // stepover, so the peak-convex-curvature stepover is the target this arm
    // should land on. The flat law is the other end of the envelope.
    let convex_target =
        scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, peak_curvature);
    let concave_target =
        scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, -peak_curvature);
    eprintln!("   -- THE ANALYTIC ENVELOPE (this surface has no single curvature) --");
    eprintln!(
        "     peak-CONVEX  target (the binding one) {convex_target:>9.5} mm   <<< the verdict is \
         measured against this"
    );
    eprintln!("     FLAT         target                   {stepover_mm:>9.5} mm");
    eprintln!(
        "     peak-CONCAVE target                   {concave_target:>9.5} mm   (widest \
         admissible; never the binding constraint)"
    );
    eprintln!(
        "     Why the convex end is the target: a ring's radius is chosen so ONE ring sweeps\n\
         \x20    everything still uncovered outside it, so the worst point sets the step, and\n\
         \x20    convex ground is where the ball covers least. The verdict bands built on it\n\
         \x20    still separate the two candidates cleanly — {:.5} (0.75x convex target) sits\n\
         \x20    above {:.5} (1.25x half the FLAT target), so a 'half' reading anywhere in the\n\
         \x20    envelope still lands in the half band and cannot be mistaken for a full one.\n",
        0.75 * convex_target,
        1.25 * 0.5 * stepover_mm
    );

    let polygon = ellipse_polygon(
        0.0,
        0.0,
        WAVY_REGION_RADIUS_MM,
        WAVY_REGION_RADIUS_MM,
        ELLIPSE_VERTICES,
    );
    let arm = plan_analytic_arm(
        "ARM WAVY",
        &mesh,
        &index,
        triangles,
        polygon,
        (WAVY_REGION_RADIUS_MM, WAVY_REGION_RADIUS_MM),
        params,
    );
    let Arm {
        label,
        region,
        report,
        outcome,
    } = arm;

    eprintln!();
    print_radial_profile(label, &report);
    let result = match outcome {
        Ok(result) => result,
        Err(refusal) => {
            diagnose_refusal(label, &report, &refusal, params);
            panic!(
                "{label} refused with {refusal:?}. On the analytic arm this is a HARD FAILURE: \
                 the region is a manifold-disk polyomino with sub-{ANALYTIC_MAX_EDGE_MM} mm \
                 facets, slope under {max_slope_deg:.1} deg and every concavity larger than the \
                 ball. The diagnosis is printed above. ONE exception: if the refusal is \
                 FlattenDidNotConverge, that is a SOLVER budget, not a finding — raise \
                 ANALYTIC_SOLVER_MAX_SWEEPS (currently {}) and re-run.",
                params.solver_max_sweeps
            );
        }
    };

    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter,
        kinematics,
        safe_z: mesh.bbox.max.z + 5.0,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };
    run_arm_evidence(
        &ArmRun {
            label,
            slug: "wavy_patch_conformal_spiral",
            kind: ArmKind::Analytic,
            analytic_target_mm: Some(convex_target),
            run_stage_e: false,
        },
        fixture,
        &region,
        &report,
        &result,
        params,
        stepover_mm,
    );
}

// ── ARMS STEEP + FLAT — retained terrain probes ─────────────────────────

/// The two `terrain_small.stl` arms, **retained as fixture-limited probes**.
///
/// They are kept, not deleted, for two reasons that survive the withdrawal:
/// the STEEP-vs-FLAT **radial area-distortion pair** is the phase's map-vs-
/// mechanism attribution evidence and is resolution-independent, and Stage E's
/// `N_C`-halving row is a structural result about the method. Every
/// fine-geometry number they print is banner-labelled WITHDRAWN at the point
/// of printing, so an excerpt cannot be quoted without the label.
fn arm_terrain(
    cutter: &BallEndmill,
    kinematics: rs_cam_core::machine_kinematics::MachineKinematics,
    stepover_mm: f64,
    params: &SpiralParams,
) {
    let path = fixture_path();
    assert!(
        path.exists(),
        "in-repo fixture missing: {} — this test needs no EXTERNAL file, but it does need this one",
        path.display()
    );

    eprintln!(
        "\n══════════ TERRAIN ARMS — fixtures/terrain_small.stl ══════════\n\
         \x20  {WITHDRAWAL_BANNER}\n"
    );

    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    let index = SpatialIndex::build_auto(&mesh);
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;

    eprintln!(
        "   mesh: {} triangles, bbox [{:.3},{:.3},{:.3}] .. [{:.3},{:.3},{:.3}]",
        mesh.triangles.len(),
        mesh.bbox.min.x,
        mesh.bbox.min.y,
        mesh.bbox.min.z,
        mesh.bbox.max.x,
        mesh.bbox.max.y,
        mesh.bbox.max.z
    );
    eprintln!("   safe_z {safe_z:.3}, effective min_z {effective_min_z:.3}\n");

    // ═══ ARM 1 — STEEP: the whole inset region ══════════════════════════
    //
    // The envelope PROBE. Its expected outcome is a diagnosed
    // RingSearchStalled, and that is a RESULT, not a test failure: the report
    // now survives the refusal, so the stall can be attributed to the map or
    // to the search. The arm fails only if a refusal arrives with nothing to
    // attribute it by.
    eprintln!(
        "\n══════════ ARM STEEP — whole inset region (envelope PROBE) ══════════\n\
         \x20  {WITHDRAWAL_BANNER}\n\
         \x20  Expected: a DIAGNOSED refusal. A refusal here is a finding; an\n\
         \x20  UNDIAGNOSABLE refusal is the failure. THAT is what still stands on this arm —\n\
         \x20  the stall class and its attribution, not any distance it reports.\n"
    );
    eprintln!("   -- region selection (inset {REGION_INSET_MM} mm, ellipse, centroid test) --");
    let steep = plan_arm(
        "ARM STEEP",
        EllipseSpec::steep(&mesh),
        &mesh,
        &index,
        params,
    );
    let steep = match steep {
        Some(arm) => {
            print_region_acceptance(&arm.region);
            arm
        }
        None => panic!(
            "STEEP arm produced no region at all on any of the {} ellipses — see the refusals \
             printed above",
            MAX_SHRINKS + 1
        ),
    };
    match &steep.outcome {
        Ok(_) => eprintln!(
            "   ARM STEEP PLANNED. The probe's expectation (a stall on the high-relief region)\n\
             \x20  was WRONG on this fixture — say so in the write-up rather than quietly\n\
             \x20  dropping the prediction. The full staged evidence below still runs on the\n\
             \x20  FLAT arm, which is the controlled one.\n"
        ),
        Err(refusal) => {
            let diagnosable = diagnose_refusal(steep.label, &steep.report, refusal, params);
            assert!(
                diagnosable,
                "{} refused with {refusal:?} but the surviving report carried nothing to \
                 attribute it by. A refusal is a result; an unattributable one is a defect in \
                 this instrument or in the module's report plumbing.",
                steep.label
            );
        }
    }

    // ═══ ARM 2 — FLAT: a low-relief window, found by census ══════════════
    //
    // The envelope-INSIDE test. Inside the envelope the mechanism has no
    // excuse, so a refusal here is a hard failure.
    eprintln!(
        "\n══════════ ARM FLAT — low-relief window (envelope-INSIDE test) ══════════\n\
         \x20  {WITHDRAWAL_BANNER}\n\
         \x20  And note the SELECTION BIAS that made it worse, recorded in the withdrawal block:\n\
         \x20  the flat-window census below picks the FLATTEST patch, and a decimated terrain mesh\n\
         \x20  puts its LARGEST triangles exactly where the surface is flattest. The census\n\
         \x20  therefore steers this arm into the COARSEST region of the mesh (0.876 mm² median\n\
         \x20  facet against 0.277 mm² mesh-wide). It is retained anyway, because the radial\n\
         \x20  area-distortion profile it pairs with ARM STEEP is resolution-independent.\n"
    );
    let census = flattest_window(&mesh);
    eprintln!(
        "   flat-window census: {} candidate centres on a {FLAT_SEARCH_COLS}x{FLAT_SEARCH_ROWS} \
         lattice, {} met the {FLAT_MIN_TRIANGLES}-triangle floor.",
        census.evaluated, census.qualifying
    );
    eprintln!(
        "   relief over the WINDOW footprint ({:.0} x {:.0} mm), up-facing centroid Z:\n\
         \x20    across qualifying candidates  min {:.3} / median {:.3} / max {:.3} mm",
        2.0 * FLAT_SEMI_AXIS_X_MM,
        2.0 * FLAT_SEMI_AXIS_Y_MM,
        census.relief_min_mm,
        census.relief_median_mm,
        census.relief_max_mm
    );
    let Some(flat_spec) = census.best else {
        panic!(
            "no candidate window held {FLAT_MIN_TRIANGLES} up-facing triangles — the FLAT arm \
             cannot be placed on this fixture"
        )
    };
    eprintln!(
        "   CHOSEN: centre ({:.3}, {:.3}), semi-axes ({:.1}, {:.1}) mm, relief {:.3} mm over \
         {} up-facing triangles.",
        flat_spec.cx,
        flat_spec.cy,
        flat_spec.ax,
        flat_spec.ay,
        census.best_relief_mm,
        census.best_triangles
    );
    eprintln!(
        "   Target was < ~{FLAT_RELIEF_TARGET_MM} mm of relief: {}. That target is NOT a bar —\n\
         \x20  whether this fixture contains a window that quiet is a fact about the fixture, and\n\
         \x20  the census reports the flattest one available either way.\n",
        if census.best_relief_mm < FLAT_RELIEF_TARGET_MM {
            "MET"
        } else {
            "NOT met — the flattest available window is quoted above; read every FLAT-arm number \
             against that relief, not against the target"
        }
    );

    let flat = match plan_arm("ARM FLAT", flat_spec, &mesh, &index, params) {
        Some(arm) => {
            print_region_acceptance(&arm.region);
            arm
        }
        None => panic!(
            "ARM FLAT produced no region at all — see the refusals printed above. Inside the \
             envelope this is a hard failure."
        ),
    };
    let Arm {
        label: flat_label,
        region,
        report,
        outcome: flat_outcome,
    } = flat;

    // ═══ THE DISQUALIFICATION, MEASURED ON THIS RUN ═════════════════════
    //
    // FINDINGS_F2's withdrawal quotes a 1.42 mm median facet on the chosen
    // flat window. Quoting it here would make this file depend on a number in
    // a markdown file; measuring it makes each arm carry its own
    // disqualification, and makes a future fixture swap visible immediately.
    eprintln!("\n══════════ WHY THE TERRAIN ARMS' FINE GEOMETRY IS WITHDRAWN ══════════\n");
    print_mesh_census(
        "ARM STEEP (fixture-limited)",
        &mesh_census(&mesh, &steep.region.triangles),
        stepover_mm,
    );
    print_mesh_census(
        "ARM FLAT (fixture-limited)",
        &mesh_census(&mesh, &region.triangles),
        stepover_mm,
    );
    eprintln!(
        "   Compare the 'stepover / MEDIAN edge' row above with the ANALYTIC arms' (~4.4 on\n\
         \x20  both). Anything under ~1 means the facets are coarser than the quantity being\n\
         \x20  measured, and every spacing number that follows is a property of the\n\
         \x20  tessellation rather than of the algorithm.\n"
    );

    // ═══ THE MONEY TABLE — the two arms' radial profiles, side by side ═══
    eprintln!(
        "\n══════════ RADIAL DISTORTION: STEEP vs FLAT ══════════\n\
         \x20  This pair is the attribution evidence for the whole phase. The mean-value\n\
         \x20  (Floater) map is a labelled [REPO] substitution for the paper's conformal slit\n\
         \x20  map; if the steep arm's profile climbs and the flat arm's is level, the\n\
         \x20  difference between the two arms is the MAP'S DISTORTION, not the mechanism, and\n\
         \x20  the repair is Phase F2 step 3.\n\
         \x20  Note the distinction the Floater change forces: the map being a valid EMBEDDING\n\
         \x20  (no flips, no fold census) and the map being LOW-DISTORTION are now separate\n\
         \x20  questions. Tutte guarantees the first; only these profiles measure the second.\n"
    );
    print_radial_profile(steep.label, &steep.report);
    eprintln!();
    print_radial_profile(flat_label, &report);
    eprintln!();

    let result = match flat_outcome {
        Ok(result) => result,
        Err(refusal) => {
            diagnose_refusal(flat_label, &report, &refusal, params);
            panic!(
                "{flat_label} refused with {refusal:?}. Inside the envelope — a low-relief, \
                 hole-free, cleaned, simply-connected window — the mechanism has no excuse. \
                 The diagnosis is printed above."
            );
        }
    };

    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter,
        kinematics,
        safe_z,
        effective_min_z,
    };
    // Stage E runs on the FLAT arm ONLY, and this is the arm. The steep arm's
    // stall makes a sensitivity sweep there meaningless — every row would
    // refuse for the same reason and the sampling dial would explain none of
    // it — and the analytic arms skip it for the reason printed on them.
    run_arm_evidence(
        &ArmRun {
            label: flat_label,
            slug: "terrain_small_conformal_spiral_flat",
            kind: ArmKind::FixtureLimited,
            analytic_target_mm: None,
            run_stage_e: true,
        },
        fixture,
        &region,
        &report,
        &result,
        params,
        stepover_mm,
    );
}

// ── the staged evidence run ─────────────────────────────────────────────

/// Phase F2 steps 1–2, end to end, over **four arms**.
///
/// `#[ignore]` is for **runtime**, not for a missing input: the two analytic
/// arms build their own meshes and the terrain fixture is in the repo, but
/// the run performs five `plan_spiral` solves — two of them over regions of
/// ~37 k and ~14 k triangles — each sweeping start angles and
/// binary-searching every ring against tens of thousands of coverage samples,
/// and then costs two candidates per arm through the production relinker.
///
/// The name is deliberately unchanged from the pre-withdrawal revision: it is
/// what PROGRAMME.md, FINDINGS_F2 and this file's own header run-command
/// quote. It now carries the analytic arms as well as the terrain ones.
#[test]
#[ignore = "evidence run — long runtime (5 plan_spiral solves, two on ~37k/~14k-triangle analytic regions); needs NO external files, both analytic fixtures are generated and the terrain one is in-repo"]
fn terrain_small_conformal_spiral_f2() {
    use rs_cam_core::machine_kinematics::MachineKinematics;

    eprintln!(
        "\n########## PHASE F2 — conformal-spiral evidence, FOUR ARMS, ANALYTIC FIRST ##########\n"
    );
    eprintln!(
        "   ORDER AND STANDING OF THE ARMS:\n\
         \x20    1. ARM SPHERE  analytic spherical cap    — DECISIVE. Carries the phase-1 spacing\n\
         \x20                                               verdict; a refusal here is a HARD\n\
         \x20                                               FAILURE.\n\
         \x20    2. ARM WAVY    analytic wavy heightfield — the realistic-but-clean case; a\n\
         \x20                                               refusal here is a HARD FAILURE.\n\
         \x20    3. ARM STEEP   terrain_small.stl         — fixture-limited PROBE. Fine-geometry\n\
         \x20                                               numbers WITHDRAWN.\n\
         \x20    4. ARM FLAT    terrain_small.stl         — fixture-limited. Fine-geometry numbers\n\
         \x20                                               WITHDRAWN; retained for the radial\n\
         \x20                                               distortion pair and Stage E.\n\
         \x20  Why: planning/conformal_finish_2026-08-28/FINDINGS_F2.md opens with a WITHDRAWAL of\n\
         \x20  every terrain fine-geometry figure — 1.42 mm facets cannot measure a 0.4862 mm\n\
         \x20  stepover. PROGRAMME.md §F2 step 1 always said 'simply connected SYNTHETIC surface'.\n"
    );

    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let stepover_mm = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);

    eprintln!(
        "   machine: accel [{:.0},{:.0},{:.0}] mm/s², junction dev {JUNCTION_DEVIATION_MM} mm, \
         rapid {RAPID_FEED_MM_MIN:.0}; feed {FEED_MM_MIN:.0}, plunge {PLUNGE_MM_MIN:.0} mm/min",
        MACHINE_ACCEL_XYZ[0], MACHINE_ACCEL_XYZ[1], MACHINE_ACCEL_XYZ[2]
    );
    eprintln!(
        "   FLAT equal-cusp stepover at R={BALL_RADIUS_MM}, h={CUSP_HEIGHT_MM}: {stepover_mm:.4} mm \
         — every analytic mesh below is built to a {ANALYTIC_MAX_EDGE_MM} mm edge ceiling,\n\
         \x20  i.e. under stepover/3 = {:.5} mm.\n",
        stepover_mm / 3.0
    );

    let mut params = SpiralParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    params.start_angle_step = SWEEP_START_ANGLE_STEP;

    // The analytic arms get their own params so the terrain arms and Stage E
    // keep the shipped solver cap and stay comparable with the pre-withdrawal
    // run. Nothing else differs between the two.
    let mut analytic_params = params.clone();
    analytic_params.solver_max_sweeps = ANALYTIC_SOLVER_MAX_SWEEPS;
    eprintln!(
        "   SOLVER-CAP DEVIATION, analytic arms ONLY: solver_max_sweeps {} -> {}.\n\
         \x20  Reaching the cap is FlattenDidNotConverge — a refusal, which on an analytic arm is\n\
         \x20  a hard failure — and the analytic regions are 20-40x larger than the terrain FLAT\n\
         \x20  window while Gauss-Seidel's sweep count grows with the square of the mesh's linear\n\
         \x20  dimension. solver_tolerance is UNTOUCHED, so an under-solve is still a refusal and\n\
         \x20  never a silent pass. The terrain arms keep the shipped default.\n",
        params.solver_max_sweeps, analytic_params.solver_max_sweeps
    );

    arm_sphere(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_wavy(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_terrain(&cutter, kinematics, stepover_mm, &params);

    eprintln!("########## PHASE F2 evidence run complete. ##########\n");
}

// ── smoke tests: no long solves, no external inputs ─────────────────────

/// The fixture exists, loads, and is the mesh this instrument was written
/// against. 40,342 is read from the binary STL header, cross-checked against
/// the file size (`2_017_184 == 84 + 50 × 40_342`).
#[test]
fn terrain_small_fixture_loads_with_40342_triangles() {
    let path = fixture_path();
    assert!(path.exists(), "fixture missing: {}", path.display());
    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    assert_eq!(
        mesh.triangles.len(),
        FIXTURE_TRIANGLES,
        "fixture triangle count moved"
    );
    assert_eq!(
        mesh.faces.len(),
        FIXTURE_TRIANGLES,
        "faces and triangles disagree"
    );
    assert!(
        (mesh.bbox.max.x - mesh.bbox.min.x - 100.0).abs() < 1e-3,
        "X span should be 100 mm, got {}",
        mesh.bbox.max.x - mesh.bbox.min.x
    );
}

/// The elliptical region must actually select a substantial part of the mesh:
/// a region of a few dozen triangles would make every downstream number
/// noise. The evidence run's own selection is the one measured here.
#[test]
fn ellipse_region_selects_a_substantial_triangle_population() {
    let path = fixture_path();
    assert!(path.exists(), "fixture missing: {}", path.display());
    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    let bb = &mesh.bbox;
    let polygon = ellipse_polygon(
        0.5 * (bb.min.x + bb.max.x),
        0.5 * (bb.min.y + bb.max.y),
        0.5 * (bb.max.x - bb.min.x) - REGION_INSET_MM,
        0.5 * (bb.max.y - bb.min.y) - REGION_INSET_MM,
        ELLIPSE_VERTICES,
    );
    let selected = conformal_spiral_region(&mesh, &polygon);
    assert!(
        selected.len() > 1000,
        "the inset ellipse should select thousands of triangles, got {}",
        selected.len()
    );
    assert!(
        selected.len() < mesh.triangles.len(),
        "the region must be a PROPER subset — an 8 mm inset that selects everything means the \
         containment test is not being applied"
    );
    // Every selected triangle's centroid really is inside.
    for &t in selected.iter().take(500) {
        let f = &mesh.faces[t as usize];
        let c = (f.v[0].coords + f.v[1].coords + f.v[2].coords) / 3.0;
        assert!(
            polygon.contains_point(&P2::new(c.x, c.y)),
            "selected triangle {t} has its centroid outside the region"
        );
    }
}

/// The analytic containment test the census uses must agree with the polygon
/// the arm is actually selected with, or the census would be measuring relief
/// over a different footprint from the one that gets machined.
#[test]
fn ellipse_spec_containment_matches_the_polygon() {
    let spec = EllipseSpec {
        cx: 50.0,
        cy: 30.0,
        ax: 12.0,
        ay: 9.0,
    };
    let poly = ellipse_polygon(spec.cx, spec.cy, spec.ax, spec.ay, ELLIPSE_VERTICES);
    assert!(spec.contains(spec.cx, spec.cy), "the centre is inside");
    assert!(spec.contains(spec.cx + 11.9, spec.cy), "just inside on x");
    assert!(!spec.contains(spec.cx + 12.1, spec.cy), "just outside on x");
    assert!(!spec.contains(spec.cx, spec.cy + 9.1), "just outside on y");

    // The polygon is INSCRIBED in the ellipse, so the analytic test is the
    // slightly more generous of the two. Anything the polygon accepts, the
    // analytic test must accept as well — that is the direction that matters,
    // because it means the census never measures relief over less ground than
    // the arm machines.
    let mut checked = 0usize;
    for i in 0..400 {
        let t = TAU * (i as f64) / 400.0;
        for r in [0.1_f64, 0.5, 0.9, 0.99] {
            let (x, y) = (
                spec.cx + spec.ax * r * t.cos(),
                spec.cy + spec.ay * r * t.sin(),
            );
            if poly.contains_point(&P2::new(x, y)) {
                checked += 1;
                assert!(
                    spec.contains(x, y),
                    "polygon accepted ({x}, {y}) but the analytic test rejected it"
                );
            }
        }
    }
    assert!(checked > 1000, "the sweep must exercise a real population");
}

/// The flat-window census on the real fixture. Non-ignored on purpose: the
/// relief it finds is a fact about `terrain_small.stl` that the write-up needs
/// and that nobody should have to run a multi-minute evidence test to learn.
#[test]
fn flat_window_census_finds_a_window_on_the_fixture() {
    let path = fixture_path();
    assert!(path.exists(), "fixture missing: {}", path.display());
    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    let census = flattest_window(&mesh);

    eprintln!(
        "flat-window census on terrain_small.stl: {} centres evaluated, {} qualifying; \
         relief min {:.3} / median {:.3} / max {:.3} mm (target < {FLAT_RELIEF_TARGET_MM})",
        census.evaluated,
        census.qualifying,
        census.relief_min_mm,
        census.relief_median_mm,
        census.relief_max_mm
    );
    let spec = census
        .best
        .expect("the fixture must contain at least one qualifying window");
    eprintln!(
        "  chosen centre ({:.3}, {:.3}), relief {:.3} mm over {} up-facing triangles",
        spec.cx, spec.cy, census.best_relief_mm, census.best_triangles
    );

    assert!(census.qualifying > 0);
    assert!(
        census.best_triangles >= FLAT_MIN_TRIANGLES,
        "the chosen window must clear the triangle floor"
    );
    // The winner is the minimum, so it must sit at the bottom of the spread.
    assert!(
        (census.best_relief_mm - census.relief_min_mm).abs() < 1e-9,
        "the chosen window must be the flattest qualifying one: {} vs {}",
        census.best_relief_mm,
        census.relief_min_mm
    );
    assert!(
        census.best_relief_mm <= census.relief_median_mm,
        "the minimum cannot exceed the median"
    );
    // And it must sit the same distance inside the mesh as the STEEP arm.
    let bb = &mesh.bbox;
    assert!(
        spec.cx - spec.ax >= bb.min.x + REGION_INSET_MM - 1e-9
            && spec.cx + spec.ax <= bb.max.x - REGION_INSET_MM + 1e-9
            && spec.cy - spec.ay >= bb.min.y + REGION_INSET_MM - 1e-9
            && spec.cy + spec.ay <= bb.max.y - REGION_INSET_MM + 1e-9,
        "the chosen window must stay {REGION_INSET_MM} mm inside the mesh bbox"
    );
}

/// The ellipse helper's own geometry: area within 0.5 % of `π·a·b` at 256
/// vertices, and the semi-axis endpoints on the boundary.
#[test]
fn ellipse_polygon_has_the_analytic_area() {
    let poly = ellipse_polygon(50.0, 30.0, 42.0, 28.0, ELLIPSE_VERTICES);
    let analytic = PI * 42.0 * 28.0;
    let measured = poly.area();
    assert!(
        (measured - analytic).abs() / analytic < 0.005,
        "256-gon area {measured} should be within 0.5% of pi*a*b = {analytic}"
    );
    assert!(
        poly.contains_point(&P2::new(50.0, 30.0)),
        "centre is inside"
    );
    assert!(
        !poly.contains_point(&P2::new(50.0 + 42.5, 30.0)),
        "just beyond the major semi-axis is outside"
    );
    assert!(
        !poly.contains_point(&P2::new(50.0, 30.0 + 28.5)),
        "just beyond the minor semi-axis is outside"
    );
    assert_eq!(poly.exterior.len(), ELLIPSE_VERTICES);
    assert!(poly.holes.is_empty(), "the region must be hole-free");
}

/// The stepover every arm of this table and F1's runs at. Pinned identically
/// in `direction_field_wanaka_f1.rs:1316-1325`, which is what makes the two
/// instruments' spacing tables comparable.
#[test]
fn equal_cusp_stepover_at_r1_h003_is_0_4862() {
    let s = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!(
        (s - 0.486_209_831).abs() < 1e-6,
        "equal-cusp stepover at R=1.0, h=0.03 should be 0.4862 mm, got {s}"
    );
    // And it agrees with the production law the curved variant derives from.
    let flat = scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!(
        (s - flat).abs() < 1e-9,
        "the restated equal-cusp law must equal scallop_math::stepover_from_scallop_flat: \
         {s} vs {flat}"
    );
    assert_eq!(equal_cusp_stepover_mm(1.0, 0.0), 0.0);
    assert_eq!(equal_cusp_stepover_mm(1.0, 1.0), 0.0);
}

// ── smoke tests for the ANALYTIC generators ────────────────────────────
//
// These are NOT ignored. The whole point of the analytic arms is that the
// fixture's resolution is a checkable property rather than an assumption, and
// a property nobody checks without a multi-minute evidence run is a property
// nobody checks. Each of these builds a ~35 k-triangle mesh and censuses it,
// which is tens of milliseconds.

/// [`sphere_cap_mesh`] is exactly the fixture the decisive arm claims: every
/// vertex on the sphere, every face up-facing, every 3D edge under the
/// ceiling, and a manifold disk `plan_spiral` will accept.
#[test]
fn sphere_cap_mesh_is_an_analytic_disk_fine_enough_to_measure() {
    let mesh = sphere_cap_mesh(
        SPHERE_RADIUS_MM,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_RINGS,
        SPHERE_CAP_SECTORS,
    );

    assert_eq!(
        mesh.triangles.len(),
        SPHERE_CAP_SECTORS * (2 * SPHERE_CAP_RINGS - 1),
        "polar cap triangle count is sectors * (2*rings - 1)"
    );
    assert_eq!(
        mesh.vertices.len(),
        1 + SPHERE_CAP_RINGS * SPHERE_CAP_SECTORS,
        "polar cap vertex count is 1 apex + rings * sectors"
    );

    // Every vertex is ON the sphere. The centre is OFFSET: the rim sits at
    // z = 0, so the sphere centre is at -sqrt(R^2 - a^2), never the origin —
    // testing against the origin would pass on a completely different shape.
    let rim_z =
        (SPHERE_RADIUS_MM * SPHERE_RADIUS_MM - SPHERE_CAP_RADIUS_MM * SPHERE_CAP_RADIUS_MM).sqrt();
    let centre = P3::new(0.0, 0.0, -rim_z);
    for vertex in &mesh.vertices {
        let radius = (*vertex - centre).norm();
        assert!(
            (radius - SPHERE_RADIUS_MM).abs() < 1e-9,
            "vertex {vertex:?} sits {radius} from the sphere centre, not {SPHERE_RADIUS_MM}"
        );
    }

    // A DOME, not a bowl. The brief's "z = R - sqrt(R^2 - r^2)" spelling is a
    // bowl; its own "convex UP dome" and its `+1/R_s` curvature are what the
    // generator implements, and this is where that reading is pinned.
    assert!(
        mesh.vertices[0].z > 0.0,
        "vertex 0 is the APEX of a convex-up dome, so it must be the high point"
    );
    assert!(
        (mesh.bbox.max.z - (SPHERE_RADIUS_MM - rim_z)).abs() < 1e-9,
        "apex height should be R_s - sqrt(R_s^2 - a^2), got {}",
        mesh.bbox.max.z
    );
    assert!(
        mesh.bbox.min.z.abs() < 1e-9,
        "the rim must sit at z = 0, got {}",
        mesh.bbox.min.z
    );

    // Winding. Checking every normal is cheaper — and far more reliable —
    // than hand-verifying three triangulation cases by reading.
    for (i, face) in mesh.faces.iter().enumerate() {
        assert!(
            face.normal.z > 0.0,
            "face {i} is wound backwards: normal.z = {}",
            face.normal.z
        );
    }

    let region: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let census = mesh_census(&mesh, &region);
    assert_eq!(
        census.boundary_loops, 1,
        "a cap is a DISK — exactly one boundary loop, or plan_spiral refuses it"
    );
    assert_eq!(
        census.boundary_edges, SPHERE_CAP_SECTORS,
        "the boundary is the rim ring and nothing else"
    );
    assert_eq!(census.euler, 1, "V - E + F must be 1 for a disk");

    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!(
        census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM,
        "max 3D edge {} exceeds the {ANALYTIC_MAX_EDGE_MM} mm ceiling — the diagonal of a ring \
         band is the binding edge, so rings and sectors must be sized together",
        census.edge_max_mm
    );
    assert!(
        census.edge_max_mm <= stepover / 3.0,
        "max 3D edge {} must clear the brief's stepover/3 = {} bar",
        census.edge_max_mm,
        stepover / 3.0
    );

    let (kept, cleanup) = clean_selection(&mesh, &region);
    assert_eq!(
        kept, region,
        "the analytic cap must survive clean_selection intact — hygiene here would mean the \
         generator produced a non-manifold patch"
    );
    assert_eq!(cleanup.components_first_pass, 1);
    assert_eq!(cleanup.triangles_shaved_by_pinch, 0);

    eprintln!(
        "sphere cap: {} triangles, {} vertices, 3D edge min/med/max {:.5}/{:.5}/{:.5} mm, \
         stepover({stepover:.5}) / median edge = {:.2}, 3D area {:.3} mm²",
        census.triangles,
        census.vertices,
        census.edge_min_mm,
        census.edge_median_mm,
        census.edge_max_mm,
        stepover / census.edge_median_mm,
        census.area_mm2
    );
}

/// [`wavy_patch_mesh`] honours BOTH bounds it claims — the slope one the
/// brief asks for, and the concavity-vs-ball one it does not.
#[test]
fn wavy_patch_mesh_is_a_clean_disk_within_its_slope_and_curvature_bounds() {
    let mesh = wavy_patch_mesh(
        WAVY_SIZE_MM,
        WAVY_AMPLITUDE_MM,
        WAVY_WAVELENGTH_MM,
        ANALYTIC_MAX_EDGE_MM,
    );
    let cells = wavy_grid_cells(
        WAVY_SIZE_MM,
        WAVY_AMPLITUDE_MM,
        WAVY_WAVELENGTH_MM,
        ANALYTIC_MAX_EDGE_MM,
    );
    assert_eq!(mesh.triangles.len(), 2 * cells * cells);
    assert_eq!(mesh.vertices.len(), (cells + 1) * (cells + 1));

    // Slope: the analytic bound `|grad z|max = A*k`, then the SAME bound
    // measured off the face normals. Asserting only the analytic one would
    // test the comment, not the mesh.
    let k = TAU / WAVY_WAVELENGTH_MM;
    let analytic_slope_deg = (WAVY_AMPLITUDE_MM * k).atan().to_degrees();
    assert!(
        analytic_slope_deg < 30.0,
        "the analytic max slope must stay under the stated ~30 deg bound, got \
         {analytic_slope_deg} deg"
    );
    let mut min_normal_z = f64::INFINITY;
    for (i, face) in mesh.faces.iter().enumerate() {
        assert!(
            face.normal.z > 0.0,
            "face {i} is wound backwards or vertical: normal.z = {}",
            face.normal.z
        );
        min_normal_z = min_normal_z.min(face.normal.z);
    }
    let measured_slope_deg = min_normal_z.acos().to_degrees();
    assert!(
        measured_slope_deg < 30.0,
        "the MEASURED max face slope {measured_slope_deg} deg must stay under the stated 30 deg \
         bound (analytic bound was {analytic_slope_deg} deg)"
    );

    // Concavity vs the ball. Nothing in the brief asks for this, and it is
    // the bound that would turn a parameter tweak into a stall: at a critical
    // point both principal curvatures are A*k^2, and a ball larger than the
    // pocket bridges it, leaving S^h points uncoverable AT ANY SPACING.
    let tightest_concave_radius_mm = 1.0 / (WAVY_AMPLITUDE_MM * k * k);
    assert!(
        tightest_concave_radius_mm >= 2.0 * BALL_RADIUS_MM,
        "tightest concave radius {tightest_concave_radius_mm} mm must clear 2x the ball radius \
         ({}) — otherwise the ball bridges the valleys and ARM WAVY stalls, which this file \
         treats as a hard failure",
        2.0 * BALL_RADIUS_MM
    );

    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let whole: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let whole_census = mesh_census(&mesh, &whole);
    assert!(
        whole_census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM,
        "max 3D edge over the WHOLE patch is {}, above the {ANALYTIC_MAX_EDGE_MM} mm ceiling",
        whole_census.edge_max_mm
    );
    assert!(whole_census.edge_max_mm <= stepover / 3.0);

    // The machined region: whole cells inside a circle, which must come out a
    // manifold disk needing no hygiene at all.
    let region = wavy_region_triangles(WAVY_SIZE_MM, cells, WAVY_REGION_RADIUS_MM);
    assert!(
        region.len() > 10_000,
        "the machined circle should hold >10k triangles at this cell size, got {}",
        region.len()
    );
    assert!(
        region.len() < mesh.triangles.len(),
        "the region must be a PROPER subset of the patch, or the 2 mm margin is not there"
    );
    let census = mesh_census(&mesh, &region);
    assert_eq!(
        census.boundary_loops, 1,
        "the whole-cell digitisation of a disk must have ONE boundary loop"
    );
    assert_eq!(census.euler, 1, "V - E + F must be 1 for a disk");
    assert!(census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM);

    let (kept, cleanup) = clean_selection(&mesh, &region);
    assert_eq!(
        kept, region,
        "taking WHOLE cells must leave nothing for clean_selection to do — if this fails, the \
         cell-centre test produced a corner-only touching pair and the region is a wedge sum, \
         not a disk"
    );
    assert_eq!(cleanup.components_first_pass, 1);
    assert_eq!(cleanup.triangles_shaved_by_pinch, 0);

    eprintln!(
        "wavy patch: {cells}x{cells} cells ({:.5} mm), {} region triangles, 3D edge \
         min/med/max {:.5}/{:.5}/{:.5} mm, stepover({stepover:.5}) / median edge = {:.2}, \
         max slope {measured_slope_deg:.2} deg, tightest concave R {tightest_concave_radius_mm:.3} mm",
        WAVY_SIZE_MM / cells as f64,
        census.triangles,
        census.edge_min_mm,
        census.edge_median_mm,
        census.edge_max_mm,
        stepover / census.edge_median_mm
    );
}

/// The census must be able to tell a disk from an annulus, or every
/// "one boundary loop" assertion above is vacuous.
#[test]
fn mesh_census_counts_two_loops_on_an_annulus() {
    // A deliberately tiny cap: 24 * (2*6 - 1) = 264 triangles, of which the
    // first 24 are the centre fan.
    let mesh = sphere_cap_mesh(SPHERE_RADIUS_MM, SPHERE_CAP_RADIUS_MM, 6, 24);
    assert_eq!(mesh.triangles.len(), 264);

    let disk: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let disk_census = mesh_census(&mesh, &disk);
    assert_eq!(disk_census.boundary_loops, 1);
    assert_eq!(disk_census.euler, 1);

    // Punch out the centre fan: what remains is an annulus.
    let annulus: Vec<u32> = (24u32..mesh.triangles.len() as u32).collect();
    let annulus_census = mesh_census(&mesh, &annulus);
    assert_eq!(
        annulus_census.boundary_loops, 2,
        "an annulus has an inner and an outer boundary loop"
    );
    assert_eq!(
        annulus_census.euler, 0,
        "an annulus has Euler characteristic 0, not 1"
    );
    assert_eq!(annulus_census.boundary_edges, 48, "24 inner + 24 outer");
}

/// The sphere arm's target, derived two independent ways, must agree — and
/// the coverage law must collapse to the flat law as the sphere flattens.
#[test]
fn sphere_analytic_target_agrees_with_the_modules_own_coverage_law() {
    let curved = scallop_math::stepover_from_scallop_curved(
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
        1.0 / SPHERE_RADIUS_MM,
    );
    let coverage = sphere_coverage_spacing_mm(SPHERE_RADIUS_MM, BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let flat = scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    eprintln!(
        "sphere target at R_s={SPHERE_RADIUS_MM}, K_c={BALL_RADIUS_MM}, h={CUSP_HEIGHT_MM}: \
         scallop_math curved {curved:.6} mm, module coverage law {coverage:.6} mm, flat \
         {flat:.6} mm"
    );
    assert!(
        (curved - coverage).abs() / curved < 0.01,
        "two INDEPENDENT derivations of the same target must agree to 1%: {curved} vs {coverage}"
    );
    assert!(
        curved < flat,
        "convex curvature must NARROW the admissible stepover: {curved} against flat {flat}"
    );
    // As the sphere flattens, the coverage law must become the flat law.
    // R_s = 1e4 rather than 1e12: at 1e12 the law of cosines cancels ~1 part
    // in 1e24 and the check would be measuring f64, not geometry.
    let nearly_flat = sphere_coverage_spacing_mm(1.0e4, BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!(
        (nearly_flat - flat).abs() < 1e-3,
        "at R_s -> infinity the coverage law must return the flat stepover: {nearly_flat} vs \
         {flat}"
    );
}

/// The verdict's load-bearing property: its two bands cannot both fire.
#[test]
fn verdict_bands_are_disjoint_at_every_target() {
    // "within +/-25% of T" and "within +/-25% of T/2" are disjoint exactly
    // when 1 - band > (1 + band)/2, i.e. band < 1/3.
    const {
        assert!(
            1.0 - VERDICT_BAND > 0.5 * (1.0 + VERDICT_BAND),
            "the two verdict bands overlap at this VERDICT_BAND; the split would stop being a \
             partition and the verdict would be ambiguous"
        );
    }
    for target in [0.474_312_f64, 0.486_210, 0.424_060] {
        let lo_full = target * (1.0 - VERDICT_BAND);
        let hi_half = 0.5 * target * (1.0 + VERDICT_BAND);
        assert!(
            lo_full > hi_half,
            "bands overlap at target {target}: full band opens at {lo_full}, half band closes \
             at {hi_half}"
        );
    }
}

/// The fairness table weights by AREA, and this is what that buys.
#[test]
fn area_weighted_percentiles_follow_area_not_triangle_count() {
    // Nine tiny triangles reading 1.0 and one large one reading 9.0: by COUNT
    // the median is 1.0, by AREA it is 9.0. The polar sphere-cap mesh has
    // exactly this shape — its innermost triangles are ~55x smaller than its
    // rim ones — so an unweighted percentile would report the geometry of the
    // mesh's centre rather than of the ground being cut.
    let mut pairs: Vec<(f64, f64)> = (0..9).map(|_| (1.0, 0.01)).collect();
    pairs.push((9.0, 10.0));
    let stats = area_weighted(pairs);
    assert_eq!(stats.samples, 10);
    assert!((stats.total_area_mm2 - 10.09).abs() < 1e-12);
    assert!((stats.min - 1.0).abs() < 1e-12);
    assert!((stats.max - 9.0).abs() < 1e-12);
    assert!(
        (stats.p50 - 9.0).abs() < 1e-12,
        "the AREA-weighted median must be 9.0 (an unweighted one would say 1.0), got {}",
        stats.p50
    );
    // An empty population must not fabricate a number.
    let empty = area_weighted(Vec::new());
    assert_eq!(empty.samples, 0);
    assert!(empty.p50.is_nan(), "an empty population has no median");
}

/// The step distribution measures 3D distance between CONSECUTIVE points, and
/// says nothing at all when there are fewer than two.
#[test]
fn step_stats_measures_consecutive_3d_distance() {
    let points = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 3.0),
        P3::new(1.0, 4.0, 3.0),
    ];
    let stats = step_stats(&points);
    assert_eq!(stats.samples, 3);
    assert!((stats.max_mm - 4.0).abs() < 1e-12);
    assert!((stats.median_mm - 3.0).abs() < 1e-12);
    assert_eq!(step_stats(&[]).samples, 0);
    assert_eq!(step_stats(&points[..1]).samples, 0);
}

/// A 2×2 grid of quads (8 triangles, consistently wound) as a clean disk
/// fixture for the cleanup tests: one edge-connected component, one boundary
/// loop, no pinches.
fn clean_grid_mesh() -> TriangleMesh {
    let mut verts: Vec<P3> = Vec::with_capacity(9);
    for row in 0..3 {
        for col in 0..3 {
            verts.push(P3::new(col as f64, row as f64, 0.0));
        }
    }
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(8);
    for row in 0..2u32 {
        for col in 0..2u32 {
            let (a, b) = (row * 3 + col, row * 3 + col + 1);
            let (c, d) = ((row + 1) * 3 + col, (row + 1) * 3 + col + 1);
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// [`clean_grid_mesh`] with one extra triangle hung off the grid's far corner
/// (vertex 8) by that vertex alone — the bowtie `region_topology` refuses as
/// a `BoundaryPinch`.
fn bowtie_mesh() -> TriangleMesh {
    let grid = clean_grid_mesh();
    let mut verts = grid.vertices.clone();
    let mut tris = grid.triangles;
    verts.push(P3::new(3.0, 2.0, 0.0));
    verts.push(P3::new(3.0, 3.0, 0.0));
    tris.push([8, 9, 10]);
    TriangleMesh::from_raw(verts, tris)
}

/// A clean patch must pass through the cleanup untouched. If this ever starts
/// removing triangles, the pass is shaving real surface, not artefacts.
#[test]
fn cleanup_leaves_a_clean_patch_alone() {
    let mesh = clean_grid_mesh();
    let selected: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let (bad, pinches, over_used) = nonmanifold_vertices(&mesh, &selected);
    assert!(
        bad.is_empty(),
        "the grid fixture must be manifold to be a valid control: {pinches} pinches, \
         {over_used} over-used edges"
    );

    let (kept, report) = clean_selection(&mesh, &selected);
    assert_eq!(kept, selected, "a clean patch must survive intact");
    assert_eq!(report.before, 8);
    assert_eq!(report.after, 8);
    assert_eq!(report.components_first_pass, 1);
    assert_eq!(report.components_dropped, 0);
    assert_eq!(report.pinch_iterations, 0);
    assert_eq!(report.triangles_shaved_by_pinch, 0);
    assert!(!report.hit_iteration_cap);
}

/// The bowtie the module refuses as `BoundaryPinch`: a triangle attached to
/// the patch at a single VERTEX. Edge-connectivity must see it as its own
/// component and drop it — which is why step 1 uses edge adjacency and not
/// vertex adjacency.
#[test]
fn cleanup_drops_a_vertex_only_bowtie_component() {
    let mesh = bowtie_mesh();
    let selected: Vec<u32> = (0..mesh.triangles.len() as u32).collect();

    // Precondition: this really is the pinch the module would refuse.
    let (bad, pinches, _) = nonmanifold_vertices(&mesh, &selected);
    assert!(
        bad.contains(&8) && pinches >= 1,
        "vertex 8 must read as a boundary pinch before cleanup, got {bad:?}"
    );

    let (kept, report) = clean_selection(&mesh, &selected);
    assert_eq!(
        kept.len(),
        8,
        "the 8-triangle patch must win over the lone one"
    );
    assert!(
        !kept.contains(&8),
        "the bowtie triangle (index 8) must be gone"
    );
    assert_eq!(report.components_first_pass, 2);
    assert_eq!(report.components_dropped, 1);
    // Component pruning alone fixed it — no shaving was needed.
    assert_eq!(report.pinch_iterations, 0);
    assert_eq!(report.triangles_shaved_by_pinch, 0);

    // And the postcondition the whole pass exists for.
    let (after_bad, _, _) = nonmanifold_vertices(&mesh, &kept);
    assert!(
        after_bad.is_empty(),
        "cleanup must leave no non-manifold vertex, got {after_bad:?}"
    );
}

/// The invariants that hold for EVERY input, pinned on the fixtures above:
/// the output is a subset of the input in the input's own order, it is
/// manifold or empty, and the pass is idempotent.
///
/// Idempotence is the one that matters operationally. The loop re-runs
/// component pruning after each shave precisely because shaving can orphan
/// new islands and new pinches; if a second call could still remove
/// something, that fixed point was not reached and the reported
/// `pinch_iterations` would be understating the work.
///
/// Note what is deliberately NOT asserted here: a pinch that survives
/// component pruning — a genuine wedge-sum patch, edge-connected yet visiting
/// one vertex twice on its boundary. Constructing one takes a specific
/// ~8-triangle arrangement whose winding cannot be checked by reading, and a
/// fixture that turns out to be an ordinary disk would assert nothing while
/// looking like it asserted something. The shave path is justified by the
/// mechanism (and by the operator's run 2, where `BoundaryPinch` survived at
/// every ellipse size), not by a fixture this file cannot verify.
#[test]
fn cleanup_is_idempotent_and_never_adds_a_triangle() {
    for (label, mesh) in [("clean grid", clean_grid_mesh()), ("bowtie", bowtie_mesh())] {
        let selected: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
        let (first, report) = clean_selection(&mesh, &selected);

        assert!(
            first.iter().all(|t| selected.contains(t)),
            "{label}: cleanup must never invent a triangle"
        );
        assert!(
            first.windows(2).all(|w| w[0] < w[1]),
            "{label}: cleanup must preserve the input's ascending order"
        );
        assert_eq!(
            report.after,
            first.len(),
            "{label}: report.after must match"
        );
        assert!(
            report.after <= report.before,
            "{label}: cleanup must never grow the selection"
        );
        assert!(!report.hit_iteration_cap, "{label}: must converge");

        let (after_bad, _, _) = nonmanifold_vertices(&mesh, &first);
        assert!(
            after_bad.is_empty(),
            "{label}: cleanup must leave a manifold selection, got {after_bad:?}"
        );

        let (second, second_report) = clean_selection(&mesh, &first);
        assert_eq!(second, first, "{label}: cleanup must be idempotent");
        assert_eq!(
            second_report.pinch_iterations, 0,
            "{label}: a cleaned selection needs no further shaving"
        );
        assert_eq!(
            second_report.components_dropped, 0,
            "{label}: a cleaned selection is one component"
        );
    }
}

/// The point-to-segment distance Stage C depends on: a point abreast of a
/// segment measures the perpendicular, not the nearer endpoint.
/// **Restated from `direction_field_wanaka_f1.rs:1410-1421`.**
#[test]
fn point_segment_distance_measures_the_perpendicular() {
    let a = P3::new(0.0, 0.0, 0.0);
    let b = P3::new(10.0, 0.0, 0.0);
    let p = P3::new(5.0, 3.0, 0.0);
    assert!((point_segment_distance_sq(p, a, b).sqrt() - 3.0).abs() < 1e-9);
    // Beyond the end, it clamps to the endpoint.
    let beyond = P3::new(14.0, 3.0, 0.0);
    assert!((point_segment_distance_sq(beyond, a, b).sqrt() - 5.0).abs() < 1e-9);
    // A degenerate segment is a point.
    assert!((point_segment_distance_sq(p, a, a).sqrt() - 34.0_f64.sqrt()).abs() < 1e-9);
}

/// A single-polyline input must produce exactly one plunge and one closing
/// retract — the motion shape the spiral's stay-down claim reduces to.
#[test]
fn one_polyline_yields_one_plunge_and_one_retract() {
    let line = vec![
        P3::new(0.0, 0.0, -1.0),
        P3::new(1.0, 0.0, -1.0),
        P3::new(2.0, 0.5, -1.0),
    ];
    let tp = polylines_to_toolpath(&[line], FEED_MM_MIN, PLUNGE_MM_MIN, 12.0);
    let intents: Vec<MoveIntent> = tp.moves.iter().map(|mv| mv.intent).collect();
    assert_eq!(
        intents,
        vec![
            MoveIntent::Linking,
            MoveIntent::EntryPlunge,
            MoveIntent::FinishingCut,
            MoveIntent::FinishingCut,
            MoveIntent::Retract,
        ]
    );
    assert_eq!(
        intents
            .iter()
            .filter(|i| **i == MoveIntent::Retract)
            .count(),
        1,
        "a continuous spiral pays for exactly one retract"
    );
    assert!(
        polylines_to_toolpath(&[], FEED_MM_MIN, PLUNGE_MM_MIN, 12.0)
            .moves
            .is_empty()
    );
}

/// The curvature census's sign convention, checked on the case it was derived
/// from: a convex surface must read positive. Two triangles form a tent whose
/// fold runs along `y` at `x = 0`, sloping down in −x and +x; with +Z-forced
/// normals that fold is convex.
///
/// The sign is order-independent by construction — swapping the two triangles
/// flips both `d` and `n₂ − n₁`, leaving the dot product unchanged — so the
/// HashMap's iteration order cannot decide this assertion.
#[test]
fn curvature_census_calls_a_ridge_convex() {
    let verts = vec![
        P3::new(-1.0, 0.0, 0.0),
        P3::new(0.0, 0.0, 1.0),
        P3::new(0.0, 1.0, 1.0),
        P3::new(1.0, 1.0, 0.0),
    ];
    let tris = vec![[0u32, 1, 2], [1, 3, 2]];
    let mesh = TriangleMesh::from_raw(verts, tris);
    let region: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let census = curvature_census(&mesh, &region);
    assert_eq!(census.edges, 1, "the tent shares exactly the fold edge");
    assert!(
        (census.convex_fraction - 1.0).abs() < 1e-12,
        "a ridge must read CONVEX, got convex fraction {}",
        census.convex_fraction
    );
    assert!(
        census.median() > 0.0,
        "a folded surface must have nonzero |kappa|, got {}",
        census.median()
    );
    // Convex curvature narrows the admissible stepover; concave widens it.
    let k = census.median();
    let convex = scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, k);
    let concave = scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, -k);
    assert!(
        convex < concave,
        "convex stepover {convex} must be narrower than concave {concave}"
    );
}

/// A flat sheet must read as (near) zero curvature, so the census cannot
/// manufacture a curvature correction out of a plane.
#[test]
fn curvature_census_reads_zero_on_a_plane() {
    let verts = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.0),
        P3::new(1.0, 1.0, 0.0),
        P3::new(0.0, 1.0, 0.0),
    ];
    let tris = vec![[0u32, 1, 2], [0, 2, 3]];
    let mesh = TriangleMesh::from_raw(verts, tris);
    let region: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let census = curvature_census(&mesh, &region);
    assert_eq!(census.edges, 1, "the two triangles share exactly one edge");
    assert!(
        census.median() < 1e-9,
        "a plane must read zero curvature, got {}",
        census.median()
    );
    // And a zero curvature must return the flat stepover exactly.
    let curved =
        scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, census.median());
    let flat = scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!((curved - flat).abs() < 1e-9);
}

/// The coarsened sweep must snap cleanly onto both angular lattices Stage E
/// uses, or its three rows would differ in the sweep as well as the sampling.
#[test]
fn coarsened_sweep_snaps_onto_both_stage_e_lattices() {
    // The count the MODULE computes, not a nominal one: `build_best_spiral`
    // uses `((TAU / step).floor() as usize).max(1)`.
    let candidates = (TAU / SWEEP_START_ANGLE_STEP).floor() as usize;
    assert_eq!(candidates, 10, "pi/5 must give 10 start-angle candidates");
    for (n_c, expected) in [(360usize, 36.0_f64), (180, 18.0)] {
        let dtheta = TAU / n_c as f64;
        let cells = SWEEP_START_ANGLE_STEP / dtheta;
        assert!(
            (cells - expected).abs() < 1e-9,
            "pi/5 must be exactly {expected} cells of 2pi/{n_c}, got {cells}"
        );
    }
    // And it is genuinely a coarsening of the paper's step, not a change of
    // kind — the 10x ratio check below subsumes the direction of the
    // comparison, so no separate constant assert is needed.
    let ratio = SWEEP_START_ANGLE_STEP / PAPER_START_ANGLE_STEP;
    assert!(
        (ratio - 10.0).abs() < 1e-9,
        "pi/5 is 10x the paper's pi/50, got {ratio}"
    );
}

/// The falsifier's own arithmetic, without running a solve: each of the three
/// §B.4 conditions must trip on its own, and a clean report must pass.
#[test]
fn falsifier_conditions_are_independent() {
    use rs_cam_core::conformal_spiral::CoverageAudit;

    // Mirrors `stage_a`'s four conditions EXACTLY, including the rule that an
    // ABSENT audit is a STOP. If the two ever drift, this test is a docstring
    // that lies about the gate it claims to pin.
    let trips = |r: &SpiralReport| -> bool {
        r.uncovered_after_bridging > 0
            || r.disk_self_intersections > 0
            || r.bridge_overhead_pct > MAX_BRIDGE_OVERHEAD_PCT
            || r.coverage_audit
                .as_ref()
                .is_none_or(|a| a.unmachined_area_fraction > MAX_UNMACHINED_FRACTION)
    };
    let audited = |fraction: f64| {
        Some(CoverageAudit {
            unmachined_area_fraction: fraction,
            ..CoverageAudit::default()
        })
    };
    let clean = SpiralReport {
        bridge_overhead_pct: 4.2,
        coverage_audit: audited(0.0),
        ..SpiralReport::default()
    };
    assert!(!trips(&clean), "a clean report must not trip the falsifier");
    assert!(trips(&SpiralReport {
        uncovered_after_bridging: 1,
        ..clean.clone()
    }));
    assert!(trips(&SpiralReport {
        disk_self_intersections: 1,
        ..clean.clone()
    }));
    assert!(trips(&SpiralReport {
        bridge_overhead_pct: 25.001,
        ..clean.clone()
    }));
    // Condition 4, both ways round.
    assert!(
        trips(&SpiralReport {
            coverage_audit: audited(0.235),
            ..clean.clone()
        }),
        "the 23.5% ARM SPHERE hole must STOP the run — this is the whole reason condition 4 \
         exists"
    );
    assert!(
        trips(&SpiralReport {
            coverage_audit: None,
            ..clean.clone()
        }),
        "an ABSENT audit must STOP: a falsifier cannot clear a condition it has no observation \
         for, and coercing None to zero is how an unmeasured quantity becomes a green gate"
    );
    // Exactly at each bar is a PASS — both conditions are strictly greater.
    assert!(!trips(&SpiralReport {
        bridge_overhead_pct: MAX_BRIDGE_OVERHEAD_PCT,
        ..clean.clone()
    }));
    assert!(!trips(&SpiralReport {
        coverage_audit: audited(MAX_UNMACHINED_FRACTION),
        ..clean
    }));
}

/// The scenario that motivated condition 4, pinned as a regression: the
/// pre-fix ARM SPHERE report shape — search self-report all green, a quarter
/// of the region unmachined — must now STOP.
#[test]
fn the_arm_sphere_hole_would_now_be_caught() {
    use rs_cam_core::conformal_spiral::CoverageAudit;

    // Exactly what the run printed: three green self-reported conditions...
    let report = SpiralReport {
        uncovered_after_rings: 0,
        uncovered_after_bridging: 0,
        disk_self_intersections: 0,
        bridge_overhead_pct: 4.2,
        // ...on a search that had been starved...
        triangles_without_samples: 17_048,
        region_triangles: 37_060,
        // ...over a part with a 2.783 mm-radius hole.
        coverage_audit: Some(CoverageAudit {
            unmachined_area_fraction: 0.235,
            ..CoverageAudit::default()
        }),
        ..SpiralReport::default()
    };

    // Every condition built from the search's own self-report passes.
    assert_eq!(report.uncovered_after_bridging, 0);
    assert_eq!(report.disk_self_intersections, 0);
    assert!(report.bridge_overhead_pct <= MAX_BRIDGE_OVERHEAD_PCT);

    // The independent witness does not.
    let fraction = report
        .coverage_audit
        .as_ref()
        .map(|a| a.unmachined_area_fraction)
        .expect("audit present");
    assert!(
        fraction > MAX_UNMACHINED_FRACTION,
        "23.5% unmachined must exceed the {MAX_UNMACHINED_FRACTION} bar"
    );
    // And the sampling tripwire fires independently of the audit, so the two
    // are separate lines of defence rather than one restated twice.
    assert!(
        report.triangles_without_samples > 0,
        "the starvation tripwire must also fire on this shape"
    );
}
