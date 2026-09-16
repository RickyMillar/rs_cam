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
//! This file therefore carries **four analytic arms first**, on test-local
//! synthetic meshes whose triangle edge is `≤ stepover/3` (≤ 0.16 mm) over
//! the machined region, and keeps the two terrain arms afterwards **as
//! fixture-limited probes with their fine-geometry numbers withdrawn**.
//!
//! | stage | what it prints |
//! |---|---|
//! | **F2-A** | the params actually used, the full 34-row `SpiralReport` grouped, the `N_S` adequacy arithmetic, and the **§B.4 falsifier** verdict |
//! | **F2-B** | two SVGs — the unit-disk domain, and the XY world view |
//! | **F2-C** | measured adjacent-ring 3D spacing vs the flat equal-cusp stepover AND vs the curvature-corrected stepover, plus the paper's own 12 % scallop-overshoot context |
//! | **F2-D** | drop-cutter CL conversion, containment count, and the **FIVE-way** F-034 cost table — conformal spiral, ball-end 0° raster, the `direction_field` iso-curves (`D = t1`), the synthesis §4.3 iso-scallop field (`D = sweep`) and the operator's iso-scallop field (`D = medial axis`) — on one region through one relink, plus the achieved-spacing block (five rows, three bases) |
//! | **F2-D floor** | (2026-08-30) **`× FLOOR`** — `L_min = ∫∫ dA / s_max(x)` from the fixture's ANALYTIC curvature, and every candidate's `cut_mm / L_min`. See [`region_floor`] |
//! | **F2-D figures** | (2026-08-30, the operator's request) `{slug}_compare_f2.svg`: one panel per candidate at **identical scale in identically sized viewBoxes**, cutting moves solid, **surface links green**, air red dashed, **lift points as red rings**, each panel labelled with its own measured row; and `{slug}_overlay_f2.svg`, the same five superimposed at 45 % opacity |
//! | **F2-E** | sampling sensitivity: three `plan_spiral` runs at (N_S, N_C), (N_S/2, N_C) and (N_S, N_C/2) |
//!
//! # The decisive experiment (added 2026-08-30)
//!
//! `planning/finishing_synthesis_2026-08-30.md` §6 asks for one instrument and
//! names the two pieces it was missing. Both are now here.
//!
//! **§4.3 — a FOURTH candidate.** The `direction_field` arm lost on every
//! analytic arm and its adjacent-level spacing was wildly non-uniform (sphere
//! median 0.333 against a 0.474 target). The synthesis's diagnosis is that its
//! DIRECTION SOURCE is degenerate, not that the Poisson machinery is broken:
//! `D = t1` is the max-signed-principal direction and a sphere cap is UMBILIC,
//! so `t1` is noise. [`sweep_field_candidate`] feeds the SAME machinery a fixed
//! sweep direction through `solve_paths_with_target`, changing `V` and nothing
//! else — which makes the two field rows an ATTRIBUTION rather than two
//! observations.
//!
//! **§1 — the FLOOR.** `L_min = ∫∫ dA / s_max(x)` is the shortest cutting
//! distance that can meet the spec; below it a path is under-covering, full
//! stop. Nothing in this programme had ever measured a candidate against it.
//! [`region_floor`] integrates it per triangle from each fixture's closed form
//! and [`print_floor_block`] prints `cut_mm / L_min` for every row on every
//! analytic arm.
//!
//! # The operator's candidate (added 2026-08-31) — a FIFTH row
//!
//! Looking at ARM RIBBON's branched figure the operator proposed *"parallel
//! passes down each of the arms — along the length of each arm — and then a
//! spiral in the center."* The arithmetic supports them: an `L × W` arm swept
//! ALONG its axis needs `W/s` passes, swept ACROSS it needs `L/s` — same total
//! distance, **5 pass-ends against 21** on this fixture's `10 × 2.5 mm` arms,
//! and pass-ends are what links and retracts are made of. The existing `0°`
//! raster gets along-axis treatment only for the arms that happen to point
//! along `+X`.
//!
//! It is a THIRD choice of `D` in machinery already driven here, not a new
//! algorithm: `D = rotate(∇EDT, 90° about n)`, where `EDT` is the region's 2D
//! Euclidean distance transform. Its ridge runs ALONG each arm, so `∇EDT` runs
//! ACROSS it and the rotation runs along. Because `V_dir = n × D = −∇̂EDT`, the
//! solved potential is a reparameterised NEGATIVE distance transform and its
//! level sets are **iso-distance offsets of the boundary** — i.e. this row is
//! contour-parallel machining carrying the iso-scallop spacing law. See
//! [`medial_field_candidate`], [`build_medial_grid`] and the separate
//! pre-registration block in [`print_medial_preregistration`]. Its competing
//! prior evidence (`planning/thin_organic_2026-08-27/FINDINGS.md` §0j 0.917×
//! and §0k 0.686×, both COSTS) is printed beside every medial row along with
//! what differs.
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
//! # Six arms, analytic first
//!
//! | # | arm | fixture | facet vs measurand | status of its numbers |
//! |---|---|---|---|---|
//! | 1 | **ARM SPHERE** | [`sphere_cap_mesh`], analytic | max 0.1585 mm (median 0.1099) vs 0.486 mm | **decisive for SPACING**; refusal = hard failure |
//! | 2 | **ARM WAVY** | [`wavy_patch_mesh`], analytic | max 0.1545 mm (median 0.1087) vs 0.486 mm | **valid**; refusal = hard failure |
//! | 3 | **ARM RIBBON** | [`ribbon_cell_mask`] over [`ribbon_height`], analytic | `≤` [`ANALYTIC_MAX_EDGE_MM`] vs 0.486 mm | **decisive for the RETRACT TRADE**; a *diagnosed* refusal is a finding |
//! | 4 | **ARM BAND** | [`band_cell_mask`] over [`band_height`], analytic | `≤` [`ANALYTIC_MAX_EDGE_MM`] vs 0.486 mm | **decisive for TOPOLOGY**; `NotSimplyConnected` is a HEADLINE finding |
//! | 5 | ARM STEEP | `terrain_small.stl` | 0.80 mm vs 0.486 mm | fixture-limited PROBE; fine geometry WITHDRAWN |
//! | 6 | ARM FLAT | `terrain_small.stl` | 1.42 mm vs 0.486 mm | fixture-limited; fine geometry WITHDRAWN |
//!
//! # ⚠ The second benchmarking error, and the two arms that fix it
//!
//! §F2-2 measured the spiral **5–15 % slower** than a 0° raster on arms 1 and
//! 2 — and on those fixtures the raster produced **4 and 3 fragments with ZERO
//! RETRACTS**. The spiral's entire value proposition is one continuous
//! stay-down path with no lifts, so it had nothing to beat: a
//! retract-elimination method was benchmarked on geometry with no retracts to
//! eliminate. On the real target (Wanaka thin-organic region 1,
//! `planning/thin_organic_2026-08-27/FINDINGS.md`) a 0° raster produces **564
//! fragments and 97 kept retracts**, and even the tuned PCA-cell plan carries
//! **141 / 53**.
//!
//! [`ARM RIBBON`](arm_ribbon) reproduces that fragmentation class while staying
//! **simply connected**, so it isolates the *distortion* question from the
//! *topology* question. [`ARM BAND`](arm_band) is the topology question and the
//! geometry the programme actually came from: regions in this codebase are
//! selected **by slope range** (`finish_planner`'s `FinishBand`), and a slope
//! band on an undulating surface is inherently scattered and multiply
//! connected — the other bands become holes inside it. Both arms run a
//! [`print_fragmentation_gate`] **before** their cost tables, so a fixture that
//! failed to reproduce the class says so instead of producing a quotable table.
//!
//! # Two rows the module measured and nothing printed (fixed 2026-08-30)
//!
//! `SpiralReport::ring_anisotropy` and the per-triangle / per-bucket
//! quasi-conformal dilatation rows were computed on every run and printed
//! nowhere. They are now on **every** arm, via [`print_distortion_blocks`]:
//!
//! * [`print_ring_anisotropy`] — the per-ring `max/min` of the map's local
//!   radial scale. Eqs. 1–4 size each ring by its **worst sector**, so this
//!   ratio *is* the over-cover the search is forced into on that ring. `1.0` =
//!   the ring can be spaced correctly everywhere at once; `N` = it is
//!   over-covered up to N-fold outside its worst sector. On a branched region
//!   this is the number that predicts whether the method can work at all.
//! * [`print_dilatation_scalars`] and the three `K` columns in
//!   [`print_radial_profile`] — `K = 1` is conformal, and `K` is **scale
//!   invariant**, so unlike area distortion it compares across arms.
//!
//! # G-CELLHOLE — a region can be simply connected and still digitise holed
//!
//! ARM RIBBON is a union of capsules that all contain the origin, so it is
//! star-shaped about the origin and **provably simply connected in the
//! continuum**. Laid onto a 0.1057 mm cell grid it came back with **Euler −3
//! and four holes** — one cell each, at `r = 3.363 mm` on the four inter-arm
//! bisectors that run **diagonal to the axis-aligned lattice**. The pocket at a
//! sub-cell wedge apex ends up 8-connected to the open wedge and 4-connected to
//! nothing, and 4-connectivity is the only kind `region_topology` recognises.
//! It is the dual of the bowtie [`clean_selection`] already repairs.
//! [`fill_mask_holes`] closes them and [`print_mask_fill`] says exactly what it
//! closed — 0.0447 mm², 0.024 % of the region. See that function's docs for the
//! mechanism and for what the **shipped** `region_mask` extractor does and does
//! not do about the same hazard.
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

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::f64::consts::{PI, SQRT_2, TAU};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::conformal_spiral::{
    self, DistanceStats, PAPER_START_ANGLE_STEP, SpiralParams, SpiralRefusal, SpiralReport,
    SpiralResult,
};
use rs_cam_core::direction_field::{self, FieldParams, FieldPathResult, FieldReport};
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CandidateCost, CostingContext, CostingFeeds, relink_and_cost as metrology_relink_and_cost,
};
use rs_cam_core::metrology::floor::{
    AreaWeighted, FloorReport, area_weighted, region_floor as metrology_region_floor,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::scallop_math;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{Move, MoveIntent, Toolpath};

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
) -> rs_cam_core::surface::dropcutter::DropCutterGrid {
    rs_cam_core::surface::dropcutter::batch_drop_cutter(
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
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> Toolpath {
    use rs_cam_core::geometry::region_set::RegionSet;
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
/// **ONE DELIBERATE DEVIATION** from that field-for-field restatement, added
/// 2026-08-30 with the comparison SVGs: [`CandidateCost::path`], the
/// **relinked** toolpath itself.
///
/// The operator asked to *see* the comparison the tables were making, and a
/// picture drawn from the pre-relink path would be a different path from the
/// one the table charges: `relink_fragments` is exactly what converts a
/// fragment boundary into either a stay-down surface link or a
/// lift-traverse-plunge, and those two are the columns the trade turns on.
/// Drawing anything else would be the [[instrument-integrity]] failure of
/// captioning one measurement with another's picture. The field is carried,
/// never re-derived, so the SVG and the row are the same object.
// PROMOTED (Track M, 2026-09-02): the comparison kernel lives in
// `rs_cam_core::metrology::costing`, extracted from
// `thin_organic_island_widths.rs`; this file's copy was byte-equivalent up
// to the cutter's concrete type and which `CandidateCost` fields it kept.
// The adapter below keeps this instrument's original call shape; the feed
// pins are this file's own constants, unchanged.
fn relink_and_cost(
    raw: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    safe_z: f64,
) -> CandidateCost {
    let ctx = CostingContext {
        mesh,
        index,
        cutter,
        kinematics: Some(kinematics),
        feeds: CostingFeeds {
            feed_mm_min: FEED_MM_MIN,
            plunge_mm_min: PLUNGE_MM_MIN,
            max_feed_mm_min: MAX_FEED_MM_MIN,
            rapid_feed_mm_min: RAPID_FEED_MM_MIN,
        },
    };
    metrology_relink_and_cost(&ctx, raw, boundary, safe_z)
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
            let probe = rs_cam_core::surface::dropcutter::point_drop_cutter(
                point.x, point.y, mesh, index, cutter,
            );
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

// ── COMPARISON SVGs — the operator's picture (added 2026-08-30) ─────────
//
// WHY THIS EXISTS, in the operator's own words: "The spiral looks good, the
// direction map ones look good, the regions look good. It all looks good but
// the parallel does seem to win a lot. Can you map the other toolpaths you
// talk of in the svgs too so I can see the comparison?"
//
// Every arm of this instrument has been printing a cost TABLE in which the 0°
// raster wins, and drawing only the spiral. The raster — the row that keeps
// winning — has never been rendered at all. So the picture and the verdict
// were about different objects, and the one number that decides the trade
// (retracts) had no visual form whatsoever.
//
// These files fix exactly that and nothing else: same region, same scale,
// same viewBox size per panel, three candidates side by side, each labelled
// with its OWN measured row, drawn from the COSTED (post-relink) motion.

/// How one move of a costed toolpath is drawn.
///
/// The classification is on `(move_type, intent)` and is **exhaustive over
/// `MoveIntent`** on purpose: a new intent variant must fail to compile here
/// rather than fall silently into a colour that misreports it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MoveClass {
    /// Material-removing feed. Drawn solid, in the candidate's own colour.
    Cut,
    /// A **stay-down surface link** — the connection `relink_fragments` adds
    /// in place of a lift. Drawn solid green. This is the class that makes
    /// FINDINGS_F2 §F2-4C visible: 52 of the shallow band's 63 fragment
    /// boundaries never become lifts at all.
    Link,
    /// Air motion: every rapid (whatever its intent) **and** every fed descent
    /// into material (`EntryPlunge` / `EntryHelix` / `EntryRamp`). Drawn red
    /// dashed. A plunge is XY-degenerate, so it contributes a marker rather
    /// than a visible stroke — stated in the legend rather than left to be
    /// inferred from an invisible line.
    Air,
    /// **The ambiguity colour** (magenta), and a finding if it ever appears.
    /// It catches `Drilling` (no drill op exists here), `Unknown` (a
    /// generator that never tagged its moves) and — the interesting one — a
    /// **`Retract`-tagged LINEAR** move. CLAUDE.md records that last branch as
    /// unreachable from every shipped generator, pinned by a census sentry, so
    /// magenta on one of these panels is not a drawing choice to be tidied
    /// away: it is a move nobody expected to exist.
    Other,
}

impl MoveClass {
    /// Draw order, and the index every per-class array in this section uses.
    /// `Air` is drawn FIRST so cut and link strokes sit on top of it.
    const ALL: [MoveClass; 4] = [
        MoveClass::Air,
        MoveClass::Link,
        MoveClass::Cut,
        MoveClass::Other,
    ];

    fn index(self) -> usize {
        match self {
            MoveClass::Air => 0,
            MoveClass::Link => 1,
            MoveClass::Cut => 2,
            MoveClass::Other => 3,
        }
    }
}

/// See [`MoveClass`]. `Move` is classified by its OWN tags — no kinematic
/// heuristic, no Z threshold — because those tags are what the relinker wrote
/// and what the F-034 integrator reads.
fn classify_move(mv: &Move) -> MoveClass {
    if !mv.move_type.is_cutting() {
        return MoveClass::Air;
    }
    match mv.intent {
        MoveIntent::FinishingCut
        | MoveIntent::ClearingCut
        | MoveIntent::LeadIn
        | MoveIntent::LeadOut => MoveClass::Cut,
        MoveIntent::Linking => MoveClass::Link,
        MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp => MoveClass::Air,
        MoveIntent::Drilling | MoveIntent::Retract | MoveIntent::Unknown => MoveClass::Other,
    }
}

/// One toolpath reduced to what the SVG needs: four `d` attributes, the class
/// histogram, the lift points, and the XY extent.
struct PathDrawing {
    /// SVG path data per class, indexed by [`MoveClass::index`].
    d: [String; 4],
    /// Move counts per class, same indexing.
    counts: [usize; 4],
    /// XY of every `Rapid` + `Retract` move — i.e. **where the tool left the
    /// surface**. A retract is vertical, so the retract target's XY IS the
    /// lift point. Drawn as an open circle, so a "wall of retracts" reads as a
    /// field of red rings rather than as a number in a table.
    lifts: Vec<P2>,
    /// `[min_x, min_y, max_x, max_y]` over every move target, or `None` for an
    /// empty path.
    bbox: Option<[f64; 4]>,
}

/// Reduce a costed toolpath to [`PathDrawing`].
///
/// Consecutive moves of the same class are coalesced into one polyline (each
/// segment runs from the previous move's target to this one's), which keeps a
/// 5,500-move spiral to a handful of `d` attributes instead of 5,500.
fn draw_toolpath(tp: &Toolpath) -> PathDrawing {
    let mut out = PathDrawing {
        d: [String::new(), String::new(), String::new(), String::new()],
        counts: [0; 4],
        lifts: Vec::new(),
        bbox: None,
    };
    let mut cursor: Option<P3> = None;
    let mut run: Option<usize> = None;
    for mv in &tp.moves {
        let index = classify_move(mv).index();
        out.counts[index] += 1;
        let target = mv.target;
        out.bbox = Some(match out.bbox {
            None => [target.x, target.y, target.x, target.y],
            Some([x0, y0, x1, y1]) => [
                x0.min(target.x),
                y0.min(target.y),
                x1.max(target.x),
                y1.max(target.y),
            ],
        });
        if matches!(mv.move_type, rs_cam_core::toolpath::MoveType::Rapid)
            && mv.intent == MoveIntent::Retract
        {
            out.lifts.push(P2::new(target.x, target.y));
        }
        if let Some(from) = cursor {
            if run == Some(index) {
                write!(out.d[index], " L {:.4} {:.4}", target.x, target.y)
                    .expect("write move segment");
            } else {
                write!(
                    out.d[index],
                    " M {:.4} {:.4} L {:.4} {:.4}",
                    from.x, from.y, target.x, target.y
                )
                .expect("write move segment");
            }
            run = Some(index);
        }
        cursor = Some(target);
    }
    out
}

/// One column of a comparison figure.
struct ComparePanel<'a> {
    /// Panel heading, e.g. `"conformal spiral"`.
    title: &'a str,
    /// This candidate's own colour: its CUT stroke here, and its stroke in the
    /// overlay. Chosen distinct from the link green, the air red and the
    /// ambiguity magenta.
    colour: &'a str,
    /// `None` means this candidate produced no path at all. The panel is then
    /// drawn EMPTY, carrying `note`, rather than omitted — an absent panel
    /// reads as "not tried", which is exactly the wrong thing to conclude
    /// about a refusal.
    cost: Option<&'a CandidateCost>,
    /// Printed inside the panel when `cost` is `None`; appended under the
    /// numbers otherwise. Keep it to one short line.
    note: &'a str,
}

/// XML-escape a label. The notes carry `Debug` output of refusal enums, so
/// this is cheap insurance rather than a live concern.
fn xml_text(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Union of `[min_x, min_y, max_x, max_y]` boxes.
fn union_bbox(into: &mut Option<[f64; 4]>, other: [f64; 4]) {
    *into = Some(match *into {
        None => other,
        Some([x0, y0, x1, y1]) => [
            x0.min(other[0]),
            y0.min(other[1]),
            x1.max(other[2]),
            y1.max(other[3]),
        ],
    });
}

/// The link stroke — the relinker's stay-down connections.
const LINK_COLOUR: &str = "#00a000";
/// The air stroke — rapids and fed descents.
const AIR_COLOUR: &str = "#e41a1c";
/// The ambiguity stroke. See [`MoveClass::Other`].
const OTHER_COLOUR: &str = "#ff00ff";

/// Write this arm's two comparison figures and return their paths.
///
/// * `{slug}_compare_f2.svg` — one panel per candidate, **identical scale and
///   identical panel size**, each labelled with its own measured row.
/// * `{slug}_overlay_f2.svg` — every path superimposed in ONE panel
///   at 45 % opacity, so agreement and divergence are directly visible.
///
/// Both are laid out in world millimetres and follow this file's existing
/// convention of **not** flipping Y, so they are mirrored vertically against
/// the machine frame exactly as `{slug}_xy_f2.svg` is. Keeping the convention
/// matters more than fixing it: an operator comparing the new figure against
/// the old one must not have to mirror one of them in their head.
fn write_comparison_svgs(
    slug: &str,
    label: &str,
    boundary: &[Polygon2],
    panels: &[ComparePanel<'_>],
) -> (PathBuf, PathBuf) {
    let output_dir = svg_output_dir();
    std::fs::create_dir_all(&output_dir).expect("create F2 SVG output directory");

    // -- one shared extent for every panel: that IS the "same scale" claim --
    let mut extent: Option<[f64; 4]> = None;
    for polygon in boundary {
        union_bbox(&mut extent, polygon.bbox());
    }
    let drawings: Vec<Option<PathDrawing>> = panels
        .iter()
        .map(|panel| panel.cost.map(|cost| draw_toolpath(&cost.path)))
        .collect();
    for drawing in drawings.iter().flatten() {
        if let Some(bbox) = drawing.bbox {
            union_bbox(&mut extent, bbox);
        }
    }
    // A region with no polygon AND no path cannot be laid out; fall back to a
    // unit box so the figure still emits and says so, rather than panicking on
    // an arm that refused everything.
    let [mut x0, mut y0, mut x1, mut y1] = extent.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let span = (x1 - x0).max(y1 - y0).max(1e-6);
    let pad = 0.04 * span;
    x0 -= pad;
    y0 -= pad;
    x1 += pad;
    y1 += pad;
    let (pw, ph) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
    let scale = pw.max(ph);

    let gap = 0.07 * scale;
    let font = 0.030 * scale;
    let line = 1.32 * font;
    let label_h = 6.4 * line;
    let stroke = 0.0032 * scale;
    let dash = format!("{:.4} {:.4}", 4.0 * stroke, 3.0 * stroke);
    let marker_r = 2.4 * stroke;

    // The legend is a FIXED block of prose — an array, not a `Vec`, so its
    // line count and the block height it drives cannot drift apart.
    let legend = [
        format!(
            "LEGEND — every panel is the SAME REGION at the SAME SCALE in an IDENTICALLY SIZED \
             viewBox ({pw:.2} x {ph:.2} mm), so shapes compare directly by eye."
        ),
        "black = region boundary.   coloured solid = CUTTING moves (each candidate has its own \
         colour, the same one it carries in the overlay figure)."
            .to_owned(),
        "green = SURFACE LINKS the relinker ADDED (stay-down, no lift).   red dashed = AIR: \
         rapids of every intent, plus fed descents into material."
            .to_owned(),
        "red rings = LIFT POINTS (one per kept retract). A field of rings IS the retract wall.   \
         magenta = UNCLASSIFIED move (see MoveClass::Other) — expected count 0."
            .to_owned(),
        "A plunge is XY-degenerate: it draws as a point, not a line, so it shows up in the counts \
         and the markers rather than as visible stroke."
            .to_owned(),
        "Drawn from the COSTED path — after relink_fragments + reconcile — so the links and lifts \
         drawn are the ones the F-034 time under each panel charges."
            .to_owned(),
        FRESH_STOCK_LABEL.to_owned(),
        format!(
            "Y IS NOT FLIPPED (this file's existing convention, shared with {slug}_xy_f2.svg), \
             so the image is mirrored vertically against the machine frame."
        ),
    ];
    let legend_h = (legend.len() as f64 + 1.4) * line;

    let panel_count = panels.len().max(1) as f64;
    let view_w = gap + panel_count * (pw + gap);
    let view_h = gap + ph + label_h + legend_h + gap;
    // Pixel width of both figures. The pixel HEIGHT is derived per figure from
    // that figure's own aspect ratio inside `header`, so a viewer that ignores
    // `viewBox` still gets undistorted shapes.
    let px_w = 2600.0_f64;

    let header = |title: &str, w: f64, h: f64| -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.4} {h:.4}\" \
             width=\"{px_w:.0}\" height=\"{:.0}\">\n\
             <title>{}</title>\n\
             <rect x=\"0\" y=\"0\" width=\"{w:.4}\" height=\"{h:.4}\" fill=\"white\"/>\n",
            px_w * h / w,
            xml_text(title)
        )
    };

    let boundary_paths = |svg: &mut String| {
        for polygon in boundary {
            writeln!(
                svg,
                "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"{:.4}\"/>",
                svg_path(polygon),
                1.7 * stroke
            )
            .expect("write boundary");
        }
    };

    // ---- (1) the side-by-side panels ----------------------------------
    let mut svg = header(
        &format!("{label} F2: {} candidates side by side", panels.len()),
        view_w,
        view_h,
    );
    for (i, panel) in panels.iter().enumerate() {
        let ox = gap + i as f64 * (pw + gap);
        let oy = gap;
        writeln!(
            svg,
            "<rect x=\"{ox:.4}\" y=\"{oy:.4}\" width=\"{pw:.4}\" height=\"{ph:.4}\" fill=\"none\" \
             stroke=\"#999999\" stroke-width=\"{:.4}\"/>",
            0.6 * stroke
        )
        .expect("write panel frame");
        writeln!(
            svg,
            "<g transform=\"translate({:.4} {:.4})\">",
            ox - x0,
            oy - y0
        )
        .expect("open panel group");
        boundary_paths(&mut svg);
        if let Some(drawing) = drawings.get(i).and_then(Option::as_ref) {
            for class in MoveClass::ALL {
                let index = class.index();
                if drawing.d[index].is_empty() {
                    continue;
                }
                let (colour, width, extra) = match class {
                    MoveClass::Cut => (panel.colour, stroke, String::new()),
                    MoveClass::Link => (LINK_COLOUR, 1.25 * stroke, String::new()),
                    MoveClass::Air => (
                        AIR_COLOUR,
                        0.8 * stroke,
                        format!(" stroke-dasharray=\"{dash}\""),
                    ),
                    MoveClass::Other => (OTHER_COLOUR, 1.6 * stroke, String::new()),
                };
                writeln!(
                    svg,
                    "<path d=\"{}\" fill=\"none\" stroke=\"{colour}\" \
                     stroke-width=\"{width:.4}\" stroke-linecap=\"round\"{extra}/>",
                    drawing.d[index].trim_start()
                )
                .expect("write class path");
            }
            for lift in &drawing.lifts {
                writeln!(
                    svg,
                    "<circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{marker_r:.4}\" fill=\"none\" \
                     stroke=\"{AIR_COLOUR}\" stroke-width=\"{:.4}\"/>",
                    lift.x,
                    lift.y,
                    0.7 * stroke
                )
                .expect("write lift marker");
            }
        }
        svg.push_str("</g>\n");

        // Panel label: the candidate's OWN measured row, under its own picture.
        let mut rows: Vec<String> = vec![format!("{}. {}", i + 1, panel.title)];
        match panel.cost {
            Some(cost) => {
                rows.push(format!(
                    "fragments {}   links {}   RETRACTS {}",
                    cost.fragments, cost.linked, cost.kept_retracts
                ));
                rows.push(format!(
                    "cut {:.1} mm      F-034 time {:.1} s",
                    cost.cutting_mm, cost.time_s
                ));
                let drawn = drawings
                    .get(i)
                    .and_then(Option::as_ref)
                    .map_or([0usize; 4], |d| d.counts);
                rows.push(format!(
                    "moves {}  (cut {}  link {}  air {}  UNCLASSIFIED {})",
                    cost.moves,
                    drawn[MoveClass::Cut.index()],
                    drawn[MoveClass::Link.index()],
                    drawn[MoveClass::Air.index()],
                    drawn[MoveClass::Other.index()]
                ));
                if !panel.note.is_empty() {
                    rows.push(panel.note.to_owned());
                }
            }
            None => {
                rows.push("NO PATH — this candidate produced none.".to_owned());
                rows.push(panel.note.to_owned());
                rows.push(
                    "The panel is drawn EMPTY rather than omitted: an absent panel would read \
                     as 'not tried'."
                        .to_owned(),
                );
            }
        }
        for (row, text) in rows.iter().enumerate() {
            writeln!(
                svg,
                "<text x=\"{:.4}\" y=\"{:.4}\" font-family=\"monospace\" \
                 font-size=\"{font:.4}\" fill=\"#111111\">{}</text>",
                ox + 0.01 * pw,
                oy + ph + line * (row as f64 + 1.0),
                xml_text(text)
            )
            .expect("write panel label");
        }
        if panel.cost.is_none() {
            writeln!(
                svg,
                "<text x=\"{:.4}\" y=\"{:.4}\" font-family=\"monospace\" \
                 font-size=\"{:.4}\" fill=\"#999999\" text-anchor=\"middle\">{}</text>",
                ox + 0.5 * pw,
                oy + 0.5 * ph,
                1.4 * font,
                xml_text("(no path)")
            )
            .expect("write empty-panel mark");
        }
    }
    for (row, text) in legend.iter().enumerate() {
        writeln!(
            svg,
            "<text x=\"{gap:.4}\" y=\"{:.4}\" font-family=\"monospace\" font-size=\"{:.4}\" \
             fill=\"#333333\">{}</text>",
            gap + ph + label_h + line * (row as f64 + 1.0),
            0.86 * font,
            xml_text(text)
        )
        .expect("write legend");
    }
    svg.push_str("</svg>\n");
    let compare_path = output_dir.join(format!("{slug}_compare_f2.svg"));
    std::fs::write(&compare_path, svg).expect("write F2 comparison SVG");

    // ---- (2) the overlay ----------------------------------------------
    //
    // Deliberately a DIFFERENT question from the panels, and kept in its own
    // file for that reason: the panels ask "what does each one look like", the
    // overlay asks "where do they agree". Only CUT and LINK are drawn — air
    // moves would fill the frame with three overlapping rapid webs and hide
    // the answer.
    let overlay_lines = 2.0 + panels.len() as f64;
    let overlay_h = gap + ph + (overlay_lines + 1.4) * line + gap;
    let overlay_w = gap + pw + gap;
    let mut overlay = header(
        &format!("{label} F2: the {} candidates superimposed", panels.len()),
        overlay_w,
        overlay_h,
    );
    writeln!(
        overlay,
        "<g transform=\"translate({:.4} {:.4})\">",
        gap - x0,
        gap - y0
    )
    .expect("open overlay group");
    boundary_paths(&mut overlay);
    for (i, panel) in panels.iter().enumerate() {
        let Some(drawing) = drawings.get(i).and_then(Option::as_ref) else {
            continue;
        };
        for (class, extra) in [
            (MoveClass::Cut, String::new()),
            (MoveClass::Link, format!(" stroke-dasharray=\"{dash}\"")),
        ] {
            let index = class.index();
            if drawing.d[index].is_empty() {
                continue;
            }
            writeln!(
                overlay,
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.4}\" \
                 stroke-opacity=\"0.45\" stroke-linecap=\"round\"{extra}/>",
                drawing.d[index].trim_start(),
                panel.colour,
                stroke
            )
            .expect("write overlay path");
        }
    }
    overlay.push_str("</g>\n");
    let mut overlay_legend: Vec<String> = vec![format!(
        "OVERLAY — the same {} paths in ONE panel at the SAME scale, 45% opacity. Solid = \
         cutting moves, dashed = surface links. AIR MOVES ARE OMITTED here (overlapping rapid \
         webs would hide the answer); see {slug}_compare_f2.svg for those.",
        panels.len()
    )];
    for panel in panels {
        overlay_legend.push(format!(
            "{} — {}",
            panel.colour,
            match panel.cost {
                Some(cost) => format!(
                    "{}: {} fragments, {} links, {} retracts, {:.1} mm, {:.1} s",
                    panel.title,
                    cost.fragments,
                    cost.linked,
                    cost.kept_retracts,
                    cost.cutting_mm,
                    cost.time_s
                ),
                None => format!("{}: NO PATH ({})", panel.title, panel.note),
            }
        ));
    }
    overlay_legend.push(FRESH_STOCK_LABEL.to_owned());
    for (row, text) in overlay_legend.iter().enumerate() {
        writeln!(
            overlay,
            "<text x=\"{gap:.4}\" y=\"{:.4}\" font-family=\"monospace\" font-size=\"{:.4}\" \
             fill=\"#333333\">{}</text>",
            gap + ph + line * (row as f64 + 1.0),
            0.86 * font,
            xml_text(text)
        )
        .expect("write overlay legend");
    }
    overlay.push_str("</svg>\n");
    let overlay_path = output_dir.join(format!("{slug}_overlay_f2.svg"));
    std::fs::write(&overlay_path, overlay).expect("write F2 overlay SVG");

    (compare_path, overlay_path)
}

/// The candidate colours, fixed here so the panels and the overlay cannot
/// drift apart. All three are distinct from the link green, the air red and
/// the ambiguity magenta.
const SPIRAL_COLOUR: &str = "#253494";
/// See [`SPIRAL_COLOUR`].
const RASTER_COLOUR: &str = "#cc4c02";
/// See [`SPIRAL_COLOUR`].
const FIELD_COLOUR: &str = "#6a51a3";
/// See [`SPIRAL_COLOUR`]. The synthesis §4.3 candidate — deliberately a
/// different HUE from [`FIELD_COLOUR`], not a shade of it, because the two
/// field rows are the pair a reader most needs to tell apart.
const SWEEP_COLOUR: &str = "#00838f";
/// See [`SPIRAL_COLOUR`]. The operator's medial-axis candidate (2026-08-31).
/// A deep desaturated pink — a fourth distinct hue, and much darker than the
/// pure magenta [`OTHER_COLOUR`] it must not be confused with.
const MEDIAL_COLOUR: &str = "#c51b7d";

// ── THE DIRECTION-FIELD CANDIDATE (added 2026-08-30) ────────────────────
//
// `direction_field` was FALSIFIED on Wanaka thin-organic region 1 — 6,589
// polylines against the 141-fragment PCA-cell reference (FINDINGS.md §F1-1) —
// and then never run on a clean analytic fixture at all. Two reasons to run it
// here rather than treat that as settled:
//
// * the falsification was on SHALLOW TERRAIN, where the principal-curvature
//   direction it derives its field from is NOISE. On a branched ribbon the
//   curvature may genuinely align with the arms, which is the opposite regime;
// * the operator's observation is that its paths "look good". That is an
//   observation about a picture and it deserves a number, on geometry where a
//   number means something.
//
// So it is costed through the SAME `relink_and_cost` as the other two, with
// the same cutter, stepover source, feeds, kinematics, boundary and
// `link_ceiling: None`. If it refuses or produces nothing, that is printed
// plainly and its panel is drawn empty.

/// The F1 falsification figure, quoted so this arm's polyline count has
/// something to be read against. `planning/conformal_finish_2026-08-28/
/// FINDINGS.md` §F1-1, row "TOTAL polylines".
const F1_FIELD_POLYLINES_REGION1: usize = 6_589;

/// Everything one direction-field arm produced.
struct FieldCandidate {
    /// `None` when the solve produced no usable path — see `note`.
    cost: Option<CandidateCost>,
    /// Why there is no cost, or a one-line qualifier when there is one.
    note: String,
    /// Measured adjacent-LEVEL 3D spacing, sorted ascending, for the
    /// fair-comparison block. Empty when fewer than two levels carried curves.
    spacings_sorted: Vec<f64>,
}

/// Which target field `V` a field arm was solved with. The two arms run the
/// **same** Poisson solve, the same level schedule and the same
/// marching-triangles extraction — they differ ONLY in `V`, which is the whole
/// point of the experiment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldSource {
    /// `direction_field::solve_field_paths` — `D = t1`, the max-signed-principal
    /// direction, derived from the mesh's own curvature estimate.
    Curvature,
    /// `direction_field::solve_paths_with_target` — `D = d`, a fixed sweep
    /// direction, `|V|` from the fixture's ANALYTIC curvature. See
    /// [`sweep_field_candidate`].
    Sweep,
    /// `direction_field::solve_paths_with_target` — `D = rotate(∇EDT, 90°)`,
    /// the region's own MEDIAL-AXIS direction, `|V|` from the fixture's
    /// ANALYTIC curvature. See [`medial_field_candidate`].
    Medial,
}

impl FieldSource {
    /// The one-line description of `V` printed at the head of the block.
    fn field_line(self) -> &'static str {
        match self {
            Self::Curvature => {
                "field: D = t1 (max signed principal direction), V = n x D, \
                 |V| = sqrt((k_s + 1/r)/8)"
            }
            Self::Sweep => {
                "field: D = d (FIXED SWEEP DIRECTION), V = n x d, \
                 |V| = sqrt((k_s + 1/r)/8), k_s ANALYTIC"
            }
            Self::Medial => {
                "field: D = rot90(grad EDT) (MEDIAL AXIS), V = -grad_hat(EDT) * \
                 sqrt((k_s + 1/r)/8), k_s ANALYTIC"
            }
        }
    }

    /// True when this source drives `solve_paths_with_target` — i.e. the
    /// direction-field group of [`FieldReport`] never ran and its zeros are
    /// **not** observations.
    fn bypasses_direction_field(self) -> bool {
        matches!(self, Self::Sweep | Self::Medial)
    }
}

/// Print the [`FieldReport`] essentials, so a bad field is DIAGNOSABLE rather
/// than merely slow. Grouped exactly as `direction_field_wanaka_f1.rs`'s
/// Stage A groups them, so the two instruments' blocks read alike.
///
/// **The direction-field group is NOT MEASURED on the [`FieldSource::Sweep`]
/// arm.** `solve_paths_with_target`'s own docs say so in those words — "the
/// direction-field counters of `FieldReport` stay zero on this path" — so
/// printing its zeros as if they were observations would be exactly the
/// [[instrument-integrity]] failure of reading an unrun check as a clean one.
/// The rows are printed as `n/a` instead.
fn print_field_report(
    label: &str,
    result: &FieldPathResult,
    report: &FieldReport,
    source: FieldSource,
) {
    eprintln!("\n   ===== DIRECTION-FIELD SOLVE — {label} =====");
    eprintln!("     {}", source.field_line());
    eprintln!("     -- region --");
    eprintln!(
        "       region triangles            {:>10}",
        report.region_triangles
    );
    eprintln!(
        "       region vertices             {:>10}",
        report.region_vertices
    );
    eprintln!(
        "       vertex components           {:>10}",
        report.vertex_components
    );
    match source {
        FieldSource::Curvature => {
            eprintln!("     -- direction field (§4.2) --");
            eprintln!(
                "       BFS orientation seeds       {:>10}",
                report.direction_seeds
            );
            eprintln!(
                "       SINGULAR (isotropic) tris   {:>10}   <<< a flat or umbilic region has no \
                 preferred",
                report.degenerate_triangles
            );
            eprintln!(
                "\x20                                            direction; this is where a \
                 curvature-derived"
            );
            eprintln!(
                "\x20                                            field has nothing to derive from"
            );
            eprintln!(
                "       transported tris            {:>10}",
                report.transported_triangles
            );
            eprintln!(
                "       orientation inconsistencies {:>10}",
                report.orientation_inconsistencies
            );
            eprintln!(
                "       unoriented tris (want 0)    {:>10}",
                report.unoriented_triangles
            );
        }
        FieldSource::Sweep | FieldSource::Medial => {
            eprintln!("     -- direction field (§4.2) -- NOT EXERCISED ON THIS PATH --");
            eprintln!(
                "       BFS seeds / singular tris / transported / inconsistencies / unoriented\n\
                 \x20        ....................................... n/a, ALL FIVE\n\
                 \x20      `solve_paths_with_target`'s docs: \"the direction-field counters of\n\
                 \x20      FieldReport stay zero on this path\". They are zeros because the code \
                 that\n\
                 \x20      writes them never ran, NOT because a check came back clean, so they \
                 are\n\
                 \x20      printed as n/a. THAT ABSENCE IS THE EXPERIMENT: the curvature arm's\n\
                 \x20      orientation BFS is precisely the machinery this arm replaces with a\n\
                 \x20      single given direction, and it cannot be singular, cannot be\n\
                 \x20      inconsistent and has nothing to transport."
            );
        }
    }
    eprintln!("     -- target field V (Eq. 13) --");
    eprintln!(
        "       CLAMPED-magnitude tris      {:>10}{}",
        report.clamped_magnitude_triangles,
        // The module clamps inside `build_target_field`, which the
        // target-supplied paths bypass entirely, so its counter is
        // structurally zero on them for the same reason the direction-field
        // group above is.
        if source.bypasses_direction_field() {
            "   <<< n/a — the caller clamps; see this arm's own clamp counter above"
        } else {
            ""
        }
    );
    eprintln!(
        "       |V| min / mean / max        {:>10.6} / {:.6} / {:.6}",
        report.min_target_magnitude, report.mean_target_magnitude, report.max_target_magnitude
    );
    eprintln!("     -- Poisson solve (Eq. 15) --");
    eprintln!(
        "       CG iterations               {:>10}",
        report.cg_iterations
    );
    eprintln!(
        "       CG relative residual        {:>10.3e}",
        report.cg_residual
    );
    eprintln!(
        "       CG converged                {:>10}   <<< a FALSE here invalidates every number \
         below it",
        report.cg_converged
    );
    eprintln!("     -- level schedule + marching triangles (§3.3) --");
    let (min_c, med_c, max_c) = min_med_max_usize(&report.level_component_counts);
    eprintln!(
        "       levels                      {:>10}",
        result.levels.len()
    );
    eprintln!("       components/level min/med/max {min_c:>9} / {med_c} / {max_c}");
    eprintln!(
        "       closed loops                {:>10}",
        report.closed_loops
    );
    eprintln!(
        "       saddle/degenerate crossings {:>10}",
        report.degenerate_crossings
    );
    eprintln!(
        "       floored increments          {:>10}",
        report.floored_increments
    );
    eprintln!(
        "       level cap hit               {:>10}",
        report.level_cap_hit
    );
    eprintln!(
        "       TOTAL POLYLINES             {:>10}   <<< every one is a fragment the relinker \
         must",
        report.total_polylines
    );
    eprintln!("\x20                                            link or lift out of");
    if let (Some(first), Some(last)) = (result.levels.first(), result.levels.last()) {
        eprintln!("       level range                 {first:>10.6} .. {last:.6}");
    }
    match source {
        FieldSource::Curvature => eprintln!(
            "     CONTEXT: on Wanaka thin-organic region 1 this module produced \
             {F1_FIELD_POLYLINES_REGION1} polylines\n\
             \x20      against a {WANAKA_R1_PCA_FRAGMENTS}-fragment PCA-cell reference and was \
             FALSIFIED (FINDINGS.md §F1-1). That was\n\
             \x20      SHALLOW TERRAIN, where principal curvature is noise. This run is the first \
             on a clean\n\
             \x20      analytic fixture, so the count above is a NEW observation, not a re-run of \
             that one."
        ),
        FieldSource::Sweep => eprintln!(
            "     CONTEXT: this is the synthesis §4.3 candidate — the SAME Poisson machinery the\n\
             \x20      row above runs, fed a direction that cannot be noise. Read the two rows as \
             a\n\
             \x20      PAIR: they differ in V and in nothing else, so any gap between them is\n\
             \x20      attributable to the DIRECTION SOURCE alone."
        ),
        FieldSource::Medial => eprintln!(
            "     CONTEXT: this is the OPERATOR'S candidate (2026-08-31) — the SAME Poisson\n\
             \x20      machinery again, fed the region's OWN SHAPE. It makes a THREE-way \
             attribution\n\
             \x20      out of what was a pair: D = t1 (curvature, degenerate on umbilics),\n\
             \x20      D = d (one global sweep, cannot be noise but ignores the shape) and\n\
             \x20      D = rot90(grad EDT) (the shape's skeleton). Same V machinery, same level\n\
             \x20      schedule, same extraction; only the DIRECTION SOURCE moves across the \
             three."
        ),
    }
}

/// Adjacent-**level** 3D spacing of the field's iso-curves.
///
/// **Restates [`stage_c`]'s method** — every `SAMPLE_STRIDE`th point of one
/// curve family against the nearest SEGMENT of the previous one, contact
/// points, geometry only — so the number that lands in the fair-comparison
/// block is measured the same way the spiral's is. The only difference is what
/// "adjacent" means: rings for the spiral, consecutive `polyline_levels` here.
///
/// **Bounded twice**, and the bounds are stated because they change what the
/// number means: at most `MAX_LEVEL_PAIRS` level pairs, spread evenly across
/// the schedule, and at most `MAX_POINTS_PER_PAIR` points from each. The scan
/// is O(points × segments in the previous level) and a field solve can emit
/// thousands of polylines, so an unbounded form goes quadratic on exactly the
/// fixtures worth measuring. What is reported is therefore a SAMPLE of the
/// field's spacing distribution, not a census of it.
fn field_level_spacing(result: &FieldPathResult) -> Vec<f64> {
    /// Restated from [`stage_c`], and the FLOOR on the point stride below.
    const SAMPLE_STRIDE: usize = 5;
    /// Level PAIRS measured, spread evenly across the schedule rather than
    /// taken from its start — a field's first levels sit at one end of the
    /// surface, so the first `N` pairs would be a statement about that end.
    const MAX_LEVEL_PAIRS: usize = 40;
    /// Points sampled per pair. The scan is `points × segments-in-the-previous
    /// level`, and a level of a falsified field can carry tens of components;
    /// on Wanaka region 1 this module emitted 6,589 polylines over 253 levels,
    /// which is where the unbounded form would have gone quadratic.
    const MAX_POINTS_PER_PAIR: usize = 200;

    let level_count = result.levels.len();
    if level_count < 2 {
        return Vec::new();
    }
    let pair_stride = ((level_count - 1) / MAX_LEVEL_PAIRS).max(1);
    let mut spacings: Vec<f64> = Vec::new();
    for level in (1..level_count).step_by(pair_stride) {
        let previous = result.polylines_at(level - 1);
        let current = result.polylines_at(level);
        if previous.is_empty() || current.is_empty() {
            continue;
        }
        let available: usize = current.iter().map(|line| line.len()).sum();
        let stride = (available / MAX_POINTS_PER_PAIR).max(SAMPLE_STRIDE);
        for line in &current {
            for point in line.iter().step_by(stride) {
                let mut best = f64::INFINITY;
                for other in &previous {
                    for seg in other.windows(2) {
                        let distance = point_segment_distance_sq(*point, seg[0], seg[1]);
                        if distance < best {
                            best = distance;
                        }
                    }
                }
                if best.is_finite() {
                    spacings.push(best.sqrt());
                }
            }
        }
    }
    spacings.sort_by(f64::total_cmp);
    spacings
}

/// Solve the direction field on `region_triangles`, convert its contact
/// polylines to CL exactly as the spiral's are, and cost the result through
/// the SAME production relink.
fn field_candidate(
    label: &str,
    fixture: Fixture<'_>,
    region_triangles: &[u32],
    polygons: &[Polygon2],
) -> FieldCandidate {
    let (result, report) = direction_field::solve_field_paths(
        fixture.mesh,
        region_triangles,
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
    );
    print_field_report(label, &result, &report, FieldSource::Curvature);
    cost_field_result(fixture, polygons, &result, &report, FieldSource::Curvature)
}

/// The shared back half of BOTH field candidates: contact → CL, containment
/// census, F-034 cost through the production relink, and the achieved
/// adjacent-LEVEL spacing.
///
/// Split out of [`field_candidate`] on 2026-08-30 when the sweep-direction arm
/// was added, and split rather than copied for the reason this file splits
/// everything else: two costing paths would be two chances for the five rows of
/// the comparison table to stop being produced by the same call site.
fn cost_field_result(
    fixture: Fixture<'_>,
    polygons: &[Polygon2],
    result: &FieldPathResult,
    report: &FieldReport,
    source: FieldSource,
) -> FieldCandidate {
    use rs_cam_core::geometry::region_set::RegionSet;

    let Fixture {
        mesh,
        index,
        cutter,
        kinematics,
        safe_z,
        ..
    } = fixture;

    if result.polylines.is_empty() {
        let note = match source {
            FieldSource::Curvature => format!(
                "the direction-field solve produced NO polyline on {} region triangles \
                 ({} singular, CG converged {})",
                report.region_triangles, report.degenerate_triangles, report.cg_converged
            ),
            FieldSource::Sweep => format!(
                "the sweep-direction solve produced NO polyline on {} region triangles \
                 (CG converged {})",
                report.region_triangles, report.cg_converged
            ),
            FieldSource::Medial => format!(
                "the medial-axis solve produced NO polyline on {} region triangles \
                 (CG converged {})",
                report.region_triangles, report.cg_converged
            ),
        };
        eprintln!("\n     REFUSAL / EMPTY: {note}.");
        eprintln!(
            "     That is a RESULT, not a gap: it says this field had nothing to work with on \
             this\n\
             \x20      surface. The comparison figure draws its panel EMPTY and says so."
        );
        return FieldCandidate {
            cost: None,
            note,
            spacings_sorted: Vec::new(),
        };
    }

    let converted = cl_polylines(&result.polylines, mesh, index, cutter);
    eprintln!("\n     -- contact -> cutter-centre (the SAME conversion the spiral arm uses) --");
    eprintln!(
        "       contact points in           {:>10}",
        converted.input_points
    );
    eprintln!(
        "       points dropped (no contact) {:>10}",
        converted.dropped_points
    );
    eprintln!(
        "       polylines dropped (< 2 pts) {:>10}",
        converted.dropped_polylines
    );
    eprintln!(
        "       CL polylines out            {:>10}",
        converted.polylines.len()
    );
    if converted.polylines.is_empty() {
        let note = format!(
            "all {} field polylines were lost in the CL conversion",
            result.polylines.len()
        );
        eprintln!("\n     REFUSAL: {note}.");
        return FieldCandidate {
            cost: None,
            note,
            spacings_sorted: Vec::new(),
        };
    }

    let raw = polylines_to_toolpath(&converted.polylines, FEED_MM_MIN, PLUNGE_MM_MIN, safe_z);
    let mut cutting = 0usize;
    let mut escapes = 0usize;
    for mv in &raw.moves {
        if !mv.move_type.is_cutting() {
            continue;
        }
        cutting += 1;
        let point = P2::new(mv.target.x, mv.target.y);
        if !polygons.iter().any(|p| p.contains_point(&point)) {
            escapes += 1;
        }
    }
    eprintln!("\n     -- containment: COUNTED, NOT CLIPPED --");
    eprintln!("       cutting moves               {cutting:>10}");
    eprintln!(
        "       outside the region          {escapes:>10}   ({:.3}%)",
        100.0 * escapes as f64 / cutting.max(1) as f64
    );
    eprintln!(
        "       DELIBERATE DEVIATION FROM F1 (direction_field_wanaka_f1.rs:983-1010), which \
         CLIPS.\n\
         \x20      `clip_toolpath_to_boundary` SPLITS a polyline, and FRAGMENTS is one of the \
         five columns\n\
         \x20      four-way table compares — clipping one row would manufacture fragments \
         into it.\n\
         \x20      Stage D already counts-rather-than-clips the spiral for the same reason, so \
         all four\n\
         \x20      rows are now treated alike. A nonzero escape count is the LATERAL CL SHIFT, \
         not an\n\
         \x20      uncontained path."
    );

    eprintln!("\n     -- F-034 cost through the SAME production relink --");
    eprintln!("       {FRESH_STOCK_LABEL}");
    let region_set = RegionSet::new(polygons.to_vec());
    let cost = relink_and_cost(raw, mesh, index, cutter, &region_set, &kinematics, safe_z);
    eprintln!(
        "       moves {}, fragments {}, linked {}, kept retracts {}, cutting {:.1} mm, \
         time {:.1} s",
        cost.moves, cost.fragments, cost.linked, cost.kept_retracts, cost.cutting_mm, cost.time_s
    );

    let spacings_sorted = field_level_spacing(result);
    eprintln!("\n     -- ACHIEVED SPACING OF THE FIELD'S OWN ISO-CURVES (3D, contact points) --");
    if spacings_sorted.is_empty() {
        eprintln!(
            "       NOT MEASURED — fewer than two levels carried curves, so nothing was \
             adjacent to measure."
        );
    } else {
        eprintln!(
            "       samples {}, min {:.5} / median {:.5} / p90 {:.5} / max {:.5} mm",
            spacings_sorted.len(),
            percentile(&spacings_sorted, 0.0),
            percentile(&spacings_sorted, 0.50),
            percentile(&spacings_sorted, 0.90),
            percentile(&spacings_sorted, 1.0)
        );
        eprintln!(
            "       Method restated from Stage C — a sampled point of level i against the \
             nearest SEGMENT\n\
             \x20      of level i-1, contact points, geometry only — but BOUNDED: at most 40 \
             level pairs\n\
             \x20      spread evenly across the schedule, at most 200 points from each, because \
             the scan is\n\
             \x20      O(points x segments) and a field solve emits thousands of polylines. So \
             this is a\n\
             \x20      SAMPLE of the distribution, not a census of it.\n\
             \x20      It is also a THIRD spacing basis: the spiral spaces on the 3D SURFACE, \
             the raster in\n\
             \x20      XY PROJECTION, and this on the Poisson field's own LEVEL SET. Three \
             bases, five\n\
             \x20      rows — the THREE FIELD rows share this third basis, which makes THAT set \
             the one\n\
             \x20      clean spacing comparison in the table. Read the fair-comparison block in \
             Stage D\n\
             \x20      before comparing any two times."
        );
    }

    let note = format!(
        "{} polylines, {} levels, {} escapes counted (not clipped)",
        report.total_polylines,
        result.levels.len(),
        escapes
    );
    FieldCandidate {
        cost: Some(cost),
        note,
        spacings_sorted,
    }
}

// ── THE DECISIVE EXPERIMENT (added 2026-08-30) ──────────────────────────
//
// `planning/finishing_synthesis_2026-08-30.md` §6 asks for one instrument and
// names the two pieces it is missing. Both are below.
//
// **§4.3 — the FOURTH candidate.** The direction-field arm above lost on every
// analytic arm (sphere 570.9 mm / 49.3 s against a 265.3 / 23.4 spiral and a
// 243.4 / 22.0 raster) and its measured adjacent-level spacing was wildly
// non-uniform (sphere median 0.333 against a 0.474 target; ribbon median 0.098,
// max 20.1). The synthesis's diagnosis is that its DIRECTION SOURCE is
// degenerate, not that the Poisson machinery is broken: `D = t1` is the
// max-signed-principal direction, and a sphere cap is UMBILIC — every direction
// is principal, so `t1` is arbitrary noise. That is the same cause of death as
// F1 arm A on Wanaka region 1 (a near-umbilic Shallow band, 2,340 orientation
// inconsistencies). So the machinery is fed a direction that CANNOT be noise —
// a fixed sweep direction `d` — through `solve_paths_with_target`, and nothing
// else changes. See [`sweep_field_candidate`].
//
// **§1 — the FLOOR.** `L_min = ∫∫ dA / s_max(x)`: every pass spaced exactly at
// the local iso-scallop limit, nothing cut twice. Below it the finish spec is
// not met. Nothing in this programme has ever measured a candidate against it,
// and the synthesis calls that "the closest thing to an answer to 'how close to
// optimal are we'". See [`region_floor`] and [`print_floor_block`].

/// The first and second partial derivatives of a heightfield `z = f(x, y)` at
/// one point, in closed form.
///
/// **Analytic on purpose.** The alternative — estimating curvature from the
/// mesh — would put an estimator inside both the floor and the target field,
/// and every number this experiment produces would then be a statement about
/// the estimator as much as about the geometry. These fixtures are analytic
/// *precisely* so that confound can be removed; `crate::crest_lines` is
/// `pub(crate)` to `direction_field` anyway, so the estimator is not reachable
/// from a test even if it were wanted.
#[derive(Clone, Copy)]
struct SurfaceJet {
    fx: f64,
    fy: f64,
    fxx: f64,
    fxy: f64,
    fyy: f64,
}

/// One analytic fixture's closed-form surface, as the two consumers need it.
///
/// # Sign convention — convex is POSITIVE, and it is not the textbook one
///
/// With the upward unit normal `n = (−f_x, −f_y, 1)/W` the second fundamental
/// form of a DOME is negative definite: a sphere cap reads `−1/R_s`. Both
/// consumers here want the opposite sign — `scallop_math`'s doc says "positive
/// = convex", and `direction_field`'s `k_s + 1/r` must GROW on a convex surface
/// so the stepover tightens. So every curvature this type returns is NEGATED
/// into the convex-positive convention, and a sphere cap of radius `R_s` reads
/// `+1/R_s` from both methods. The two smoke tests below assert exactly that.
#[derive(Clone, Copy)]
struct AnalyticSurface {
    /// Printed at the head of the floor block, so a transcript says which
    /// closed form produced its numbers.
    name: &'static str,
    jet: fn(f64, f64) -> SurfaceJet,
}

impl AnalyticSurface {
    /// Normal curvature (convex-positive) along the tangent direction `u`.
    ///
    /// `u` is a **world** vector lying in the mesh triangle's plane. Writing
    /// `u = a·r_x + b·r_y` with `r_x = (1,0,f_x)`, `r_y = (0,1,f_y)` gives
    /// `a = u.x`, `b = u.y` immediately, and then
    ///
    /// ```text
    /// κ_n(u) = II(u,u) / I(u,u)
    ///        = (f_xx a² + 2 f_xy a b + f_yy b²) / W   ÷   (a² + b² + (a f_x + b f_y)²)
    /// ```
    ///
    /// **The first fundamental form is divided out rather than assumed to be
    /// 1.** `u` is a unit vector in the MESH TRIANGLE's plane, which only
    /// approximates the analytic surface's tangent plane at the same point, so
    /// `I(u,u)` is near 1 but not 1. Dividing removes the last approximation in
    /// the derivation; it costs one multiply.
    fn normal_curvature(self, x: f64, y: f64, u: V3) -> f64 {
        let j = (self.jet)(x, y);
        let w = (1.0 + j.fx * j.fx + j.fy * j.fy).sqrt();
        let (a, b) = (u.x, u.y);
        let tangent = a * j.fx + b * j.fy;
        let first = a * a + b * b + tangent * tangent;
        if first.is_nan() || first <= 1e-15 || !w.is_finite() {
            return 0.0;
        }
        let second = j.fxx * a * a + 2.0 * j.fxy * a * b + j.fyy * b * b;
        -second / (w * first)
    }

    /// `(κ_min, κ_max)` in the convex-positive convention.
    ///
    /// Standard heightfield forms: `E = 1+f_x²`, `F = f_x f_y`, `G = 1+f_y²`,
    /// `L = f_xx/W`, `M = f_xy/W`, `N = f_yy/W`, `EG − F² = W²`, then
    /// `H = (EN − 2FM + GL)/(2(EG−F²))`, `K = (LN − M²)/(EG−F²)` and
    /// `κ = H ± √(H² − K)`. Negating for the convex-positive convention swaps
    /// which root is the minimum, which is why the returned pair is `(−hi, −lo)`
    /// and not `(−lo, −hi)`.
    fn principal_curvatures(self, x: f64, y: f64) -> (f64, f64) {
        let j = (self.jet)(x, y);
        let w2 = 1.0 + j.fx * j.fx + j.fy * j.fy;
        let w = w2.sqrt();
        if !w.is_finite() || w2 <= 0.0 {
            return (0.0, 0.0);
        }
        let (e, f, g) = (1.0 + j.fx * j.fx, j.fx * j.fy, 1.0 + j.fy * j.fy);
        let (l, m, n) = (j.fxx / w, j.fxy / w, j.fyy / w);
        let denominator = e * g - f * f;
        if denominator.is_nan() || denominator <= 1e-15 {
            return (0.0, 0.0);
        }
        let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * denominator);
        let gauss = (l * n - m * m) / denominator;
        let discriminant = (mean * mean - gauss).max(0.0).sqrt();
        (-(mean + discriminant), -(mean - discriminant))
    }
}

/// ARM SPHERE: `z = √(R_s² − x² − y²)` (plus a constant that derivatives kill).
///
/// `f_x = −x/w`, `f_xx = −(w² + x²)/w³`, `f_xy = −xy/w³` with `w = √(R_s²−r²)`.
/// At the pole `f_xx = f_yy = −1/R_s`, i.e. `+1/R_s` convex — the number
/// [`arm_sphere`] already derives twice by other routes.
fn sphere_jet(x: f64, y: f64) -> SurfaceJet {
    let w2 = (SPHERE_RADIUS_MM * SPHERE_RADIUS_MM - x * x - y * y).max(1e-9);
    let w = w2.sqrt();
    let w3 = w2 * w;
    SurfaceJet {
        fx: -x / w,
        fy: -y / w,
        fxx: -(w2 + x * x) / w3,
        fxy: -(x * y) / w3,
        fyy: -(w2 + y * y) / w3,
    }
}

/// ARM WAVY: `z = A·sin(kx)·sin(ky)` — [`wavy_patch_mesh`]'s own closed form.
fn wavy_jet(x: f64, y: f64) -> SurfaceJet {
    let k = TAU / WAVY_WAVELENGTH_MM;
    let (sx, cx) = ((k * x).sin(), (k * x).cos());
    let (sy, cy) = ((k * y).sin(), (k * y).cos());
    let a = WAVY_AMPLITUDE_MM;
    SurfaceJet {
        fx: a * k * cx * sy,
        fy: a * k * sx * cy,
        fxx: -a * k * k * sx * sy,
        fxy: a * k * k * cx * cy,
        fyy: -a * k * k * sx * sy,
    }
}

/// ARM RIBBON: [`ribbon_height`]'s paraboloid valley plus its ripple.
fn ribbon_jet(x: f64, y: f64) -> SurfaceJet {
    let k = ribbon_ripple_k();
    let bowl = 2.0 * RIBBON_BOWL_AMPLITUDE_MM / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM);
    let (sx, cx) = ((k * x).sin(), (k * x).cos());
    let (sy, cy) = ((k * y).sin(), (k * y).cos());
    let a = RIBBON_RIPPLE_AMPLITUDE_MM;
    SurfaceJet {
        fx: bowl * x + a * k * cx * sy,
        fy: bowl * y + a * k * sx * cy,
        fxx: bowl - a * k * k * sx * sy,
        fxy: a * k * k * cx * cy,
        fyy: bowl - a * k * k * sx * sy,
    }
}

/// ARM BAND: four compact raised-cosine bumps, [`band_height`]'s closed form.
///
/// For a radial profile `g(d)` on `d = |p − c|` with `u = (p − c)/d`:
/// `f_x = g'·u_x`, `f_xx = g''·u_x² + (g'/d)(1 − u_x²)`,
/// `f_xy = (g'' − g'/d)·u_x u_y`. At `d → 0` both `g''` and `g'/d` tend to
/// `−Aπ²/(2R²)`, so the limit is isotropic and is taken explicitly rather than
/// divided by zero. The bumps have COMPACT SUPPORT and a pitch greater than
/// `2R`, so the sum below never has two active terms — [`band_height`]'s own
/// doc says so, and it is why every bound on this arm is exact.
fn band_jet(x: f64, y: f64) -> SurfaceJet {
    let (amplitude, radius) = (BAND_BUMP_AMPLITUDE_MM, BAND_BUMP_RADIUS_MM);
    let peak = amplitude * PI * PI / (2.0 * radius * radius);
    let mut jet = SurfaceJet {
        fx: 0.0,
        fy: 0.0,
        fxx: 0.0,
        fxy: 0.0,
        fyy: 0.0,
    };
    for (cx, cy) in band_bump_centres() {
        let (dx, dy) = (x - cx, y - cy);
        let d = dx.hypot(dy);
        if d >= radius {
            continue;
        }
        if d < 1e-9 {
            jet.fxx -= peak;
            jet.fyy -= peak;
            continue;
        }
        let t = PI * d / radius;
        let first = -amplitude * PI / (2.0 * radius) * t.sin();
        let second = -peak * t.cos();
        let (ux, uy) = (dx / d, dy / d);
        let radial = first / d;
        jet.fx += first * ux;
        jet.fy += first * uy;
        jet.fxx += second * ux * ux + radial * (1.0 - ux * ux);
        jet.fxy += (second - radial) * ux * uy;
        jet.fyy += second * uy * uy + radial * (1.0 - uy * uy);
    }
    jet
}

/// See [`sphere_jet`].
const SPHERE_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "sphere cap  z = sqrt(R_s^2 - x^2 - y^2),  R_s = 20 mm  (UMBILIC everywhere)",
    jet: sphere_jet,
};
/// See [`wavy_jet`].
const WAVY_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "wavy patch  z = A sin(kx) sin(ky),  A = 0.5 mm, L = 8 mm",
    jet: wavy_jet,
};
/// See [`ribbon_jet`].
const RIBBON_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "ribbon      z = A_r (x^2+y^2)/R0^2 + A_w sin(kx) sin(ky)",
    jet: ribbon_jet,
};
/// See [`band_jet`].
const BAND_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "slope band  four raised-cosine bumps, z = A/2 (1 + cos(pi d / R))",
    jet: band_jet,
};

// ---- the floor, L_min = ∫∫ dA / s_max(x) ------------------------------

// PROMOTED (Track M, 2026-09-02): `FloorReport`, `region_floor` and the
// area-weighted percentiles live in `rs_cam_core::metrology::floor`,
// extracted from this file. The full κ_min-basis rationale — and why this
// integral does not reproduce the synthesis §1 table on three of four arms —
// is on the library function. The fixture's ANALYTIC curvature stays here
// and rides in as the curvature callback, so no estimator sits inside the
// floor.
fn region_floor(mesh: &TriangleMesh, region: &[u32], surface: AnalyticSurface) -> FloorReport {
    metrology_region_floor(
        mesh,
        Some(region),
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
        &|x, y| surface.principal_curvatures(x, y),
    )
}

/// Print the floor and every candidate's `× floor`.
///
/// `candidates` is `(label, cutting_mm)`; a `None` is a candidate that produced
/// no path and is printed as such rather than omitted.
fn print_floor_block(
    label: &str,
    surface: AnalyticSurface,
    floor: &FloorReport,
    candidates: &[(&str, Option<f64>)],
) {
    eprintln!("\n   ===== x FLOOR — L_min = INTEGRAL dA / s_max(x)   ({label}) =====");
    eprintln!(
        "     `planning/finishing_synthesis_2026-08-30.md` §1. Every pass spaced exactly at the\n\
         \x20    LOCAL iso-scallop limit, nothing cut twice. It is a HARD FLOOR, not a target:\n\
         \x20    a path shorter than it has not met the finish spec. Nothing in this programme\n\
         \x20    had ever measured a candidate against it before this run.\n"
    );
    eprintln!("     surface (closed form): {}", surface.name);
    eprintln!(
        "     integrand: s_max(t) = scallop_math::stepover_from_scallop_curved(K_c = \
         {BALL_RADIUS_MM:.3}, h = {CUSP_HEIGHT_MM:.3}, kappa_t),"
    );
    eprintln!(
        "\x20               kappa_t evaluated ANALYTICALLY at the triangle centroid; sum of \
         area_t / s_max(t)."
    );
    eprintln!(
        "\n     {:<44} {:>10} {:>10} {:>10} {:>10}",
        "s_max basis (mm)", "min", "median", "p90", "max"
    );
    eprintln!(
        "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "kappa_min (least convex) — THE FLOOR's basis",
        floor.s_max.min,
        floor.s_max.p50,
        floor.s_max.p90,
        floor.s_max.max
    );
    eprintln!(
        "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "kappa_max (most convex) — direction-worst",
        floor.s_worst.min,
        floor.s_worst.p50,
        floor.s_worst.p90,
        floor.s_worst.max
    );
    eprintln!(
        "\n     region 3D area                          {:>12.4} mm^2   ({} triangles weighted)",
        floor.area_mm2, floor.s_max.samples
    );
    eprintln!(
        "     L_min  (kappa_min basis)  THE FLOOR      {:>12.4} mm",
        floor.l_min_mm
    );
    eprintln!(
        "     L_min  (kappa_max basis)  direction-worst{:>12.4} mm",
        floor.l_min_worst_mm
    );
    if floor.degenerate > 0 {
        eprintln!(
            "     *** {} TRIANGLE(S) HAD A NON-POSITIVE s_max AND ARE NOT IN THE SUM. The floor \
             is\n\
             \x20    UNDER-STATED by their area. On these fixtures this must be 0 — every one \
             asserts\n\
             \x20    R_min >= 2 K_c, so no concavity is tighter than the ball. Investigate before \
             quoting.",
            floor.degenerate
        );
    }
    eprintln!(
        "\n     {:<38} {:>12} {:>12}",
        "candidate", "cut mm", "x FLOOR"
    );
    let mut under = 0usize;
    for &(name, cutting_mm) in candidates {
        match cutting_mm {
            Some(cut) if floor.l_min_mm > 0.0 => {
                let ratio = cut / floor.l_min_mm;
                let flag = if ratio < 1.0 {
                    under += 1;
                    "   <<< BELOW 1.0"
                } else {
                    ""
                };
                eprintln!("     {name:<38} {cut:>12.1} {ratio:>12.3}{flag}");
            }
            Some(cut) => eprintln!("     {name:<38} {cut:>12.1} {:>12}", "no floor"),
            None => eprintln!("     {name:<38} {:>12} {:>12}", "NO PATH", "-"),
        }
    }
    if under > 0 {
        eprintln!(
            "\n     *********************************************************************\n\
             \x20    *** {under} CANDIDATE(S) ARE BELOW 1.0x THE FLOOR.                      ***\n\
             \x20    *** THIS IS NOT A WIN. A path shorter than L_min CANNOT have met the ***\n\
             \x20    *** finish spec: it is under-covering, and every time it was quoted  ***\n\
             \x20    *** as 'faster' it was being credited with a finish it did not       ***\n\
             \x20    *** deliver. Synthesis §1: on the sphere the raster reads 0.997x, and ***\n\
             \x20    *** the sphere is the RIGOROUS case — curvature is constant, so the  ***\n\
             \x20    *** floor is exact and the inference is airtight there.              ***\n\
             \x20    *********************************************************************"
        );
    }
    eprintln!(
        "\n     READ THE RATIO IN ONE DIRECTION ONLY. Below 1.0 PROVES under-covering. At or\n\
         \x20    above 1.0 does NOT prove the spec was met — `cutting_mm` is the whole fed\n\
         \x20    distance including entry plunges and stay-down surface links, so the ratio is\n\
         \x20    slightly generous to every row, and a path can be long AND badly placed.\n\
         \x20    Coverage is answered by the coverage audit, not by this column."
    );
}

// ---- the §4.3 candidate: iso-scallop field, D = sweep direction --------

/// The fixed sweep direction `d` an arm's field is built on.
struct SweepDirection {
    /// Unit, in the XY plane (`z = 0`), before per-triangle projection.
    d: V3,
    /// `√(λ_major / λ_minor)` of the region's area-weighted XY second-moment
    /// matrix. `1.0` is a region with no long axis at all.
    elongation: f64,
    /// True when [`PCA_ELONGATION_FLOOR`] was not cleared and `d` fell back to
    /// `+X`. **Not a defect** — see [`pca_major_axis`].
    isotropic: bool,
}

/// Below this elongation the PCA major axis is noise and `d` falls back to `+X`.
///
/// 1.05 is a 5 % axis-length difference. A region under it has no long axis in
/// any useful sense, and reporting a "PCA direction" for one would be the same
/// class of error as `D = t1` on an umbilic sphere.
const PCA_ELONGATION_FLOOR: f64 = 1.05;

/// The region's area-weighted PCA major axis, in XY.
///
/// # All four fixtures are near-isotropic, and that is worth saying out loud
///
/// The sphere cap is a disk, the wavy region is a disk, the ribbon is eight
/// arms at equal angular pitch and the band is a 2 × 2 bump lattice — every one
/// of them has a covariance matrix within a few percent of a multiple of the
/// identity. So on these fixtures `d` is **not** "the region's long axis"; it
/// is a free parameter, and this function pins it at `+X`.
///
/// That fallback is the *best* available choice rather than a concession,
/// because `+X` is exactly the direction the `0°` raster row already sweeps.
/// The sweep-field row and the raster row then differ in ONE thing — the
/// spacing law — which isolates the synthesis's §4.3 claim as cleanly as this
/// instrument can.
///
/// **What it does NOT test, stated so the result is not over-quoted:** §4.2's
/// actual proposal is a field solved INSIDE each monotone PCA cell, with that
/// cell's own sweep direction. This is one global `d` over a whole region. On a
/// branched region the two are very different things, and the difference is
/// most of the gap between what is measured here and what §4 proposes.
fn pca_major_axis(mesh: &TriangleMesh, region: &[u32]) -> SweepDirection {
    let (mut sum_area, mut sum_x, mut sum_y) = (0.0f64, 0.0f64, 0.0f64);
    let mut centroids: Vec<(f64, f64, f64)> = Vec::with_capacity(region.len());
    for &t in region {
        let Some(face) = mesh.faces.get(t as usize) else {
            continue;
        };
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        let area = 0.5 * e1.cross(&e2).norm();
        if area.is_nan() || area <= 0.0 {
            continue;
        }
        let cx = (face.v[0].x + face.v[1].x + face.v[2].x) / 3.0;
        let cy = (face.v[0].y + face.v[1].y + face.v[2].y) / 3.0;
        sum_area += area;
        sum_x += area * cx;
        sum_y += area * cy;
        centroids.push((cx, cy, area));
    }
    let fallback = SweepDirection {
        d: V3::new(1.0, 0.0, 0.0),
        elongation: 1.0,
        isotropic: true,
    };
    if sum_area.is_nan() || sum_area <= 0.0 {
        return fallback;
    }
    let (mx, my) = (sum_x / sum_area, sum_y / sum_area);
    let (mut cxx, mut cxy, mut cyy) = (0.0f64, 0.0f64, 0.0f64);
    for (cx, cy, area) in centroids {
        let (dx, dy) = (cx - mx, cy - my);
        cxx += area * dx * dx;
        cxy += area * dx * dy;
        cyy += area * dy * dy;
    }
    let trace = cxx + cyy;
    let determinant = cxx * cyy - cxy * cxy;
    let discriminant = (0.25 * trace * trace - determinant).max(0.0).sqrt();
    let (major, minor) = (0.5 * trace + discriminant, 0.5 * trace - discriminant);
    if minor.is_nan() || minor <= 1e-12 || major.is_nan() || major <= 1e-12 {
        return fallback;
    }
    let elongation = (major / minor).sqrt();
    if elongation < PCA_ELONGATION_FLOOR {
        return SweepDirection {
            elongation,
            ..fallback
        };
    }
    // Eigenvector of the major eigenvalue, taken from whichever ROW of
    // `C − λI` has the larger norm.
    //
    // The naive form `(C_xy, λ − C_xx)` is row 1's perpendicular, and on an
    // axis-aligned elongated region BOTH of its components are ~0 (`C_xy ≈ 0`
    // by symmetry AND `λ ≈ C_xx` because X is the major axis), so its direction
    // is pure rounding noise. Row 2's perpendicular `(λ − C_yy, C_xy)` is
    // well-conditioned in exactly that case, and vice versa. Choosing by row
    // norm is the standard remedy and makes the axis-aligned case — which is
    // every fixture that clears the elongation floor here — exact.
    let (r1x, r1y) = (cxx - major, cxy);
    let (r2x, r2y) = (cxy, cyy - major);
    let v = if r1x.hypot(r1y) >= r2x.hypot(r2y) {
        V3::new(cxy, major - cxx, 0.0)
    } else {
        V3::new(major - cyy, cxy, 0.0)
    };
    let norm = v.norm();
    if norm.is_nan() || norm <= 1e-12 {
        return SweepDirection {
            elongation,
            ..fallback
        };
    }
    SweepDirection {
        d: v / norm,
        elongation,
        isotropic: false,
    }
}

/// `V_dir` — the unit stepover direction, and the direction `k_s` is read
/// along, for a facet of normal `n` swept along `d`.
///
/// `V_dir = n × normalise(d − n(n·d))`. Returns `None` only when `d` projects
/// to nothing in the facet's plane, i.e. a vertical wall.
///
/// **Factored out of [`sweep_field_candidate`]'s closure solely so a test can
/// pin it.** It is the one line of the whole derivation that can be silently
/// inverted — swap `n × d` for `d`, or take the cross product the other way,
/// and every number the arm produces is still plausible and still wrong.
/// [`k_s_is_read_across_the_passes_not_along_them`] is that pin.
fn sweep_target_axis(n: V3, d: V3) -> Option<V3> {
    let projected = d - n * d.dot(&n);
    let norm = projected.norm();
    if norm.is_nan() || norm <= 1e-9 {
        return None;
    }
    let rotated = n.cross(&(projected / norm));
    let length = rotated.norm();
    if length > 1e-9 {
        Some(rotated / length)
    } else {
        None
    }
}

/// Any unit vector in the plane of `n` — used only when a facet is so steep
/// that the sweep direction projects to nothing. See [`sweep_field_candidate`].
fn any_in_plane(n: V3) -> V3 {
    let axis = if n.x.abs() < 0.9 {
        V3::new(1.0, 0.0, 0.0)
    } else {
        V3::new(0.0, 1.0, 0.0)
    };
    let v = n.cross(&axis);
    let norm = v.norm();
    if norm > 1e-12 {
        v / norm
    } else {
        V3::new(1.0, 0.0, 0.0)
    }
}

/// **The synthesis §4.3 candidate.** The same Poisson machinery the curvature
/// arm runs, fed a target field whose DIRECTION cannot be noise.
///
/// # The target field V, derived
///
/// Per region triangle, given its unit normal `n` (oriented +Z by
/// `build_region_mesh`) and its centroid:
///
/// 1. **Project the sweep direction into the triangle plane**:
///    `d_proj = normalise(d − n (n·d))`. A degenerate projection needs a facet
///    perpendicular to `d`, i.e. a vertical wall; every fixture here is bounded
///    below ~31° of slope, so the fallback is a **tripwire expected to read 0**,
///    not a routine branch.
/// 2. **Direction of V**: `V_dir = n × d_proj`, unit by construction (`n ⟂
///    d_proj`, both unit). This is `D⁹⁰°` of the paper's Eq. 12 with the feed
///    direction `D = d_proj`. The solve drives `∇φ → V`, so the level sets —
///    which run perpendicular to `∇φ` — run along `n × V = n × (n × d_proj) =
///    −d_proj`, i.e. **ALONG the sweep direction**. Long straight passes: the
///    kinematics term of synthesis §3.
/// 3. **Magnitude of V**: Zou Eq. 13, `|V| = √((k_s + 1/K_c)/8)`.
///
/// # Which direction `k_s` is evaluated in — the one thing that must not be
/// got backwards
///
/// **`k_s` is the normal curvature along `V_dir = n × d_proj` — ACROSS the
/// passes, the direction the stepover is taken in — NOT along `d`.**
///
/// The brief that commissioned this experiment says "`k_s` is the normal
/// curvature in the direction PERPENDICULAR to the pass, i.e. along `d`'s
/// in-plane projection". Those two clauses name **different** directions and
/// the second one is a slip: the passes run along `d`, so perpendicular to the
/// passes is `n × d`, not `d`. The module settles it without ambiguity —
/// `direction_field::build_target_field` computes `rotated = n.cross(&d)` and
/// then `normal_curvature(..., rotated)`, with the comment "the direction `k_s`
/// is measured along, because `k_s` is the normal curvature PERPENDICULAR to
/// the feed direction". Zou's convention is the same: `k_s` is measured
/// perpendicular to the FEED, the feed runs along the level set, so `k_s` is
/// measured across the passes — which is the direction the stepover is taken
/// in, which is the only direction whose curvature can affect a scallop
/// between two adjacent passes.
///
/// Getting it backwards would invert the whole point: on a cylinder it would
/// tighten the stepover along the flat generator and open it around the curved
/// section, i.e. exactly wrong in both places.
///
/// # The clamp
///
/// `k_s + 1/K_c ≤ 0` is the local gouge condition — a concavity tighter than
/// the ball. Every fixture here asserts `R_min ≥ 2·K_c` in its own smoke test,
/// so this too is a **tripwire expected to read 0**. When it does fire the
/// magnitude falls back to the flat-surface value `√(1/(8 K_c))` and the
/// triangle is counted. The module's own fallback is the region's minimum valid
/// magnitude, which needs two passes over the region and is therefore not
/// expressible inside a per-triangle `Fn`; the difference is immaterial at a
/// count of zero and is stated here rather than hidden.
fn sweep_field_candidate(
    label: &str,
    fixture: Fixture<'_>,
    region_triangles: &[u32],
    polygons: &[Polygon2],
    surface: AnalyticSurface,
    sweep: &SweepDirection,
) -> FieldCandidate {
    let params = FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let inverse_radius = 1.0 / BALL_RADIUS_MM;
    let flat_magnitude = (inverse_radius / 8.0).sqrt();
    let clamped = Cell::new(0usize);
    let projection_failures = Cell::new(0usize);
    let d = sweep.d;

    eprintln!("\n   ===== SWEEP-DIRECTION FIELD — {label} =====");
    eprintln!(
        "     d = ({:.5}, {:.5}) = {:.2} deg from +X.   region PCA elongation {:.4}{}",
        d.x,
        d.y,
        d.y.atan2(d.x).to_degrees(),
        sweep.elongation,
        if sweep.isotropic {
            "  <<< BELOW the 1.05 floor"
        } else {
            ""
        }
    );
    if sweep.isotropic {
        eprintln!(
            "     THE REGION HAS NO LONG AXIS, so d fell back to +X — which is the SAME \
             direction\n\
             \x20    the 0deg raster row sweeps. That is the cleanest control this instrument \
             can\n\
             \x20    produce: the raster row and this row now differ in the SPACING LAW ALONE.\n\
             \x20    On ARM SPHERE it is additionally immaterial — the cap is rotationally \
             symmetric\n\
             \x20    about its axis, so every choice of d is the same experiment rotated.\n\
             \x20    WHAT THIS DOES NOT TEST: synthesis §4.2 puts the field inside each MONOTONE\n\
             \x20    PCA CELL with that cell's own d. This is ONE GLOBAL d over a whole region."
        );
    }
    eprintln!(
        "     Same FieldParams::new(K_c, h) as the curvature arm, same Poisson solve, same level\n\
         \x20    schedule, same marching-triangles extraction. THE ONLY DIFFERENCE IS V."
    );

    let (result, report) = direction_field::solve_paths_with_target(
        fixture.mesh,
        region_triangles,
        |_global, centroid, n| {
            let unit = match sweep_target_axis(n, d) {
                Some(axis) => axis,
                None => {
                    projection_failures.set(projection_failures.get() + 1);
                    let fallback = n.cross(&any_in_plane(n));
                    let length = fallback.norm();
                    if length > 1e-9 {
                        fallback / length
                    } else {
                        return V3::zeros();
                    }
                }
            };
            // k_s ACROSS the passes — along `unit`, never along `d`. See this
            // function's docs for why, and for the module comment that rules it.
            let k_s = surface.normal_curvature(centroid.x, centroid.y, unit);
            let denominator = k_s + inverse_radius;
            let magnitude = if denominator > 1e-12 {
                (denominator / 8.0).sqrt()
            } else {
                clamped.set(clamped.get() + 1);
                flat_magnitude
            };
            unit * magnitude
        },
        &params,
    );

    eprintln!(
        "     TRIPWIRES (both must read 0 on an analytic fixture):  in-plane projection \
         failures {}, |V| clamps {}",
        projection_failures.get(),
        clamped.get()
    );
    if projection_failures.get() > 0 || clamped.get() > 0 {
        eprintln!(
            "     *** A TRIPWIRE FIRED. A projection failure needs a facet perpendicular to d \
             (a\n\
             \x20    vertical wall; max slope on these fixtures is ~31 deg) and a clamp needs a\n\
             \x20    concavity tighter than K_c (every fixture asserts R_min >= 2 K_c). Either \
             one\n\
             \x20    means the fixture is not what its own smoke test says it is."
        );
    }
    print_field_report(label, &result, &report, FieldSource::Sweep);
    cost_field_result(fixture, polygons, &result, &report, FieldSource::Sweep)
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
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
    /// `mesh.bbox.max.z + 5.0`, as the reference instruments compute it.
    safe_z: f64,
    /// `mesh.bbox.min.z - 0.1`, as the reference instruments compute it.
    effective_min_z: f64,
}

// ── THE FIFTH CANDIDATE (2026-08-31): iso-scallop field, D = MEDIAL AXIS ─
//
// **The operator's own proposal, tested.** Looking at ARM RIBBON's branched
// figure they said: *"I imagine the shortest path could look something like
// parallel passes down each of the arms. But along the length of each arm. And
// then a spiral in the center."*
//
// THE ARITHMETIC THAT MAKES IT WORTH A ROW. An arm of length `L` and width `W`
// swept ALONG its axis needs `W/s` passes of length `L`; swept ACROSS it needs
// `L/s` passes of length `W`. The total distance is the same — `L·W/s` either
// way — but the PASS COUNT is not, and pass ends are what links, turns and
// retracts are made of. On this fixture's 10 mm × 2.5 mm arms that is **5
// passes against 21**: a 4× cut in pass ends for the same ground covered. The
// existing `0°` raster gets along-axis treatment only for the two arms that
// happen to point along `+X`; the other six pay the 21.
//
// THE INSIGHT THAT MAKES IT CHEAP. This is not a new algorithm. It is a THIRD
// choice of `D` in machinery this file already drives:
// `solve_paths_with_target` takes an arbitrary per-triangle target `V`, which
// is exactly how the `D = sweep` arm above is built. The operator's "along each
// arm" IS the region's medial-axis direction, and it falls out of a distance
// transform in three lines:
//
// * the 2D Euclidean distance transform of the region mask has its RIDGE
//   running ALONG each arm (the arm's centreline is the locus farthest from
//   both walls);
// * `∇EDT` therefore points ACROSS the arm — straight at the nearest wall;
// * so `D = rotate(∇EDT, 90° about n)` points ALONG the arm, everywhere,
//   automatically, with a smooth blend wherever the arms merge.
//
// WHAT THIS CANDIDATE ACTUALLY IS, SAID PLAINLY SO NOBODY HAS TO DERIVE IT
// FROM THE FIGURE. Feeding `D = n × ĝ` through the same `V_dir = n × D_proj`
// convention the sweep arm uses gives `V_dir = n × (n × ĝ) = −ĝ`. So
// `V = −|V| · ĝ`: the target gradient is the NEGATED, scallop-density-weighted
// distance-transform gradient, and the solved potential `φ` is a
// reparameterised NEGATIVE DISTANCE TRANSFORM. Its level sets are therefore
// **iso-distance offsets of the region boundary**. In one sentence:
//
//   THIS ROW IS CONTOUR-PARALLEL (OFFSET) MACHINING WITH THE ISO-SCALLOP
//   SPACING LAW ATTACHED.
//
// On a long thin arm the offsets of a capsule ARE parallel passes down its
// length, which is the operator's first clause; around the hub they are closed
// loops encircling the centre, which is the operator's second one. Both halves
// of the prediction are structural consequences of the construction rather
// than hopes, and the run decides whether they pay.
//
// THE COMPETING PRIOR EVIDENCE, QUOTED HERE BECAUSE IT POINTS THE OTHER WAY.
// `planning/thin_organic_2026-08-27/FINDINGS.md`:
//
// * **§0j** measured per-cell sweep-direction variation at **0.917× region 1 /
//   0.921× top-three — a COST, not a win**, against `§0i`'s 1.034× for ONE
//   global rotation. Verdict recorded: "D1 is REFUTED — per-cell direction is
//   a measured COST."
// * **§0k** measured contour-per-cell at **0.686× region 1 / 0.710×
//   top-three — a LOSS**, and that is the MORE DIRECT prior for this row,
//   because this row is a contour strategy.
//
// WHAT DIFFERS, STATED SO THIS ARM CANNOT BE READ AS OVERTURNING §0j/§0k
// WITHOUT IT. Both of those measured **lattice-derived monotone CELLS**, each
// given its own rotation or its own contour set, on a rig whose neighbouring
// cells then no longer shared a lattice; §0j's own diagnosis of its loss is
// "misaligned neighbouring lattices break the cross-cell serpentine chords the
// relinker stitches (+9 % cutting distance)". This arm has **no cells and no
// lattice at all**: one continuous, SHAPE-derived field over the whole region,
// with the spacing law attached to it, so there are no cell seams to break.
// That is a different proposition — it is not a refutation of §0j/§0k and this
// file does not claim one. The number decides, and the distinction is printed
// beside the number every time.

/// How many EDT cells the medial grid puts across ONE stepover.
///
/// Four is chosen against the quantity the gradient has to resolve: the arm
/// HALF-width, `1.25 mm`, is `2.57` stepovers, so four cells per stepover puts
/// ~10 cells between an arm's wall and its ridge — enough for a central
/// difference to be a derivative rather than a difference of two boundary
/// distances, and coarse enough that the grid stays smaller than the mesh it
/// is sampled by.
const MEDIAL_CELLS_PER_STEPOVER: f64 = 4.0;

/// Floor on the medial grid's side, in cells. Below this a central difference
/// has nothing to difference.
const MEDIAL_MIN_CELLS: usize = 48;

/// Ceiling on the medial grid's side, in cells. The EDT is `O(n²)` and the
/// rasterisation is `O(n · ring vertices)`, so neither is the cost driver
/// beside a Poisson solve — but a degenerate bounding box must not be able to
/// turn a grid allocation into the run's failure mode.
const MEDIAL_MAX_CELLS: usize = 1024;

/// Margin (mm) held outside the region on every side of the medial grid, so
/// the region never touches the grid border and every boundary cell the EDT
/// measures against is a real one.
const MEDIAL_GRID_PAD_MM: f64 = 0.75;

/// Degeneracy floor on `|∇EDT|`, **in cell units**.
///
/// An exact Euclidean distance transform satisfies `|∇d| = 1` wherever the
/// nearest-boundary site is unique, and a central difference on the cell
/// lattice reproduces that at `≈ 1.0`. It collapses only where the nearest
/// site is NOT unique — which is precisely the medial axis. `0.25` is a
/// quarter of the ideal, i.e. a direction whose two candidate sites disagree
/// by more than ~150°.
///
/// **The count this threshold produces is a real diagnostic, not noise to be
/// hidden.** Near the medial axis the raw gradient is genuinely
/// ill-conditioned; a construction that claims to follow the medial axis owes
/// the reader a number for how much of the region is on it.
const MEDIAL_GRADIENT_FLOOR: f64 = 0.25;

/// Half-stencil (cells) of the SMOOTHED fallback tier's central difference.
///
/// Two, not one: the smoothed tier exists to survive a one-cell ridge, and a
/// one-cell stencil straddling a one-cell ridge differences the same two
/// opposed gradients the raw tier already failed on.
const MEDIAL_SMOOTH_OFFSET: usize = 2;

/// How far (cells) a sample point may be snapped to reach an inside cell.
///
/// Two, because the medial grid's cell is chosen from the STEPOVER while the
/// mesh's is chosen from the facet ceiling: a region triangle's centroid can
/// legitimately sit one cell on the wrong side of a re-rasterised staircase
/// edge, and two cells is that plus a margin. Anything further away is a
/// genuine off-region sample and degenerates. See [`MedialSample::snapped`].
const MEDIAL_SNAP_RADIUS: usize = 2;

/// Which tier of the gradient construction answered at one sample.
///
/// Counted, printed, and never silently collapsed: the tier mix IS the
/// evidence about how well-posed the medial direction is on a given region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientTier {
    /// The raw central difference cleared [`MEDIAL_GRADIENT_FLOOR`].
    Raw,
    /// The raw difference was degenerate and the smoothed one answered.
    Smoothed,
    /// Both were degenerate. The sample contributes `V = 0`.
    Degenerate,
}

/// One gradient reading at one XY point.
struct MedialSample {
    /// Unit XY direction ACROSS the region — `∇EDT` normalised, pointing from
    /// the wall toward the ridge's side of the arm. `None` when both tiers
    /// were degenerate.
    across: Option<(f64, f64)>,
    tier: GradientTier,
    /// `|∇EDT|` of the RAW central difference, in cell units. Reported
    /// separately from the tier because the fraction of the region sitting
    /// below [`MEDIAL_GRADIENT_FLOOR`] is the honest measure of "how much of
    /// this shape IS medial axis", independent of whether a fallback rescued
    /// it.
    raw_magnitude: f64,
    /// The sample point landed on a cell the rasterisation called OUTSIDE and
    /// was snapped to the nearest inside cell.
    ///
    /// **Counted, because it is a re-rasterisation artefact and not a fact
    /// about the shape.** The medial grid is built at a cell size chosen from
    /// the stepover, not at the mesh's own cell size, so a region triangle
    /// whose centroid sits within one cell of the boundary can land on the
    /// wrong side of a staircase edge. Outside cells carry `EDT = 0` with
    /// zero gradient, so without the snap the whole outer FRINGE of every
    /// region would degenerate to `V = 0` — which would distort the outermost
    /// level set, the one that hugs the boundary, on every arm.
    snapped: bool,
    /// A raw central difference was actually computed here.
    ///
    /// **`false` means NOT MEASURED, not "measured zero".** Off-grid samples
    /// and snap failures carry `raw_magnitude: 0.0` as a sentinel, and folding
    /// those into the below-the-floor count would report a not-measured value
    /// as a measured one — the `None`-vs-`Some(0.0)` conflation this repo's
    /// own report-only findings contract exists to prevent. The two are
    /// counted and printed separately.
    measured: bool,
}

/// The region's 2D Euclidean distance transform, on a square origin-centred
/// cell grid, plus the one blurred copy the smoothed tier differences.
///
/// # Which EDT this is, and why
///
/// `rs_cam_core::geometry::grid_field::distance_transform_2d` — the shipped
/// Felzenszwalb–Huttenlocher separable transform, the same one
/// `region_mask`, `tier_islands`, `finish_planner` and
/// `thin_organic_island_widths.rs` use. It returns the distance from each cell
/// to the nearest `true` cell **in cell units**, so it is fed the COMPLEMENT
/// of the region mask and every inside cell then reads its distance to the
/// nearest OUTSIDE cell. Nothing is computed test-locally: the transform, and
/// the hole repair applied to the mask before it, are both in-repo code this
/// file already depends on.
///
/// Cell units, not mm, on purpose: the gradient's DIRECTION is what this
/// construction consumes and it is scale-free, while the magnitude's ideal
/// value in cell units is exactly `1.0`, which is what makes
/// [`MEDIAL_GRADIENT_FLOOR`] a number a reader can check.
///
/// # The grid is square and origin-centred, and that is load-bearing
///
/// [`fill_mask_holes`] — the in-file, already-audited G-CELLHOLE repair — is
/// written for a square `cells × cells` mask of side `size_mm` centred on the
/// origin, and this grid is built to that shape so the repair applies
/// verbatim. Re-rasterising a staircase polygon at a DIFFERENT cell size than
/// the one it came from is a G-CELLHOLE re-run waiting to happen, and it would
/// happen at the worst possible locus: a spurious one-cell pocket in an
/// inter-arm wedge is a spurious interior boundary, which mints a spurious
/// medial branch, right next to the hub this arm is asked to report on. The
/// repair runs and its report is PRINTED, exactly as `arm_ribbon` prints its
/// own.
struct MedialGrid {
    cells: usize,
    size_mm: f64,
    /// EDT in CELL units: each cell's distance to the nearest cell OUTSIDE the
    /// region. Zero on outside cells.
    edt: Vec<f64>,
    /// `edt` after one 3 × 3 box blur — the smoothed tier's input.
    blurred: Vec<f64>,
    /// The repaired region mask the EDT was taken from.
    inside: Vec<bool>,
    /// What [`fill_mask_holes`] repaired on the way.
    fill: MaskFillReport,
}

impl MedialGrid {
    /// Millimetres per cell.
    fn cell_mm(&self) -> f64 {
        self.size_mm / (self.cells.max(1) as f64)
    }

    /// The cell containing world point `(x, y)`, or `None` off the grid.
    ///
    /// **Nearest cell, never interpolated.** Bilinear interpolation of a
    /// gradient field averages opposed vectors across the ridge and would
    /// manufacture exactly the degeneracy this construction is trying to
    /// measure. The grid is finer than the mesh triangles it is sampled by
    /// (printed at construction), so nearest-cell costs nothing here.
    fn cell_of(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if self.cells == 0 || self.size_mm <= 0.0 {
            return None;
        }
        let cell = self.cell_mm();
        let half = 0.5 * self.size_mm;
        let col = ((x + half) / cell).floor();
        let row = ((y + half) / cell).floor();
        if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= self.cells || row >= self.cells {
            return None;
        }
        Some((row, col))
    }

    /// Central difference of `field` at `(row, col)` over `± step` cells.
    ///
    /// Returns cell-unit partials `(∂/∂x, ∂/∂y)`. `col` indexes `+X` and `row`
    /// indexes `+Y`, matching [`cell_centre`].
    fn central(&self, field: &[f64], row: usize, col: usize, step: usize) -> Option<(f64, f64)> {
        if step == 0 || row < step || col < step {
            return None;
        }
        if row + step >= self.cells || col + step >= self.cells {
            return None;
        }
        let at = |r: usize, c: usize| field[r * self.cells + c];
        let span = 2.0 * step as f64;
        let gx = (at(row, col + step) - at(row, col - step)) / span;
        let gy = (at(row + step, col) - at(row - step, col)) / span;
        Some((gx, gy))
    }

    /// The nearest cell the repaired mask calls INSIDE, within
    /// [`MEDIAL_SNAP_RADIUS`] cells of `(row, col)`.
    ///
    /// Returns the cell itself with `snapped = false` when it is already
    /// inside. See [`MedialSample::snapped`] for why this exists at all.
    fn snap_inside(&self, row: usize, col: usize) -> Option<(usize, usize, bool)> {
        if self.inside[row * self.cells + col] {
            return Some((row, col, false));
        }
        let mut best: Option<(usize, usize, usize)> = None;
        let radius = MEDIAL_SNAP_RADIUS as i64;
        for dr in -radius..=radius {
            for dc in -radius..=radius {
                let r = row as i64 + dr;
                let c = col as i64 + dc;
                if r < 0 || c < 0 {
                    continue;
                }
                let (r, c) = (r as usize, c as usize);
                if r >= self.cells || c >= self.cells {
                    continue;
                }
                if !self.inside[r * self.cells + c] {
                    continue;
                }
                let d2 = (dr * dr + dc * dc) as usize;
                if best.is_none_or(|(_, _, seen)| d2 < seen) {
                    best = Some((r, c, d2));
                }
            }
        }
        best.map(|(r, c, _)| (r, c, true))
    }

    /// The ACROSS direction at a world XY point, with its tier.
    fn across_at(&self, x: f64, y: f64) -> MedialSample {
        let degenerate = MedialSample {
            across: None,
            tier: GradientTier::Degenerate,
            raw_magnitude: 0.0,
            snapped: false,
            measured: false,
        };
        let Some((row, col)) = self.cell_of(x, y) else {
            return degenerate;
        };
        let Some((row, col, snapped)) = self.snap_inside(row, col) else {
            return degenerate;
        };
        let raw = self.central(&self.edt, row, col, 1);
        let raw_magnitude = raw.map_or(0.0, |(gx, gy)| gx.hypot(gy));
        if let Some((gx, gy)) = raw
            && raw_magnitude >= MEDIAL_GRADIENT_FLOOR
        {
            return MedialSample {
                across: Some((gx / raw_magnitude, gy / raw_magnitude)),
                tier: GradientTier::Raw,
                raw_magnitude,
                snapped,
                measured: true,
            };
        }
        let smoothed = self.central(&self.blurred, row, col, MEDIAL_SMOOTH_OFFSET);
        if let Some((gx, gy)) = smoothed {
            let norm = gx.hypot(gy);
            if norm >= MEDIAL_GRADIENT_FLOOR {
                return MedialSample {
                    across: Some((gx / norm, gy / norm)),
                    tier: GradientTier::Smoothed,
                    raw_magnitude,
                    snapped,
                    measured: true,
                };
            }
        }
        MedialSample {
            raw_magnitude,
            snapped,
            measured: raw.is_some(),
            ..degenerate
        }
    }

    /// The largest inscribed-disk radius the grid saw, in mm — the EDT's peak.
    /// On the ribbon this is the HUB radius, and it is printed because it is
    /// the number that decides how many level sets the hub can hold.
    fn peak_inscribed_mm(&self) -> f64 {
        let peak = self.edt.iter().copied().fold(0.0f64, f64::max);
        peak * self.cell_mm()
    }

    /// Cells the repaired mask marks as region.
    fn inside_cells(&self) -> usize {
        self.inside.iter().filter(|&&v| v).count()
    }
}

/// Even-odd scanline fill of `polygons` — exteriors AND holes together — onto
/// a square `cells × cells` mask of side `size_mm` centred on the origin.
///
/// Even-odd over every ring at once is what makes holes and multiple
/// components come out right without a nesting analysis: a point inside a
/// hole crosses one extra ring and flips back to outside, and a component
/// sitting inside another's hole flips back to inside. ARM BAND / SHALLOW is
/// five components carrying four holes between them and needs exactly that.
///
/// Scanline rather than a per-cell `contains_point`: a polyomino region
/// polygon carries thousands of staircase vertices, and the per-cell form is
/// `O(cells² · vertices)` where this is `O(cells · vertices)`.
fn rasterise_polygons(polygons: &[Polygon2], size_mm: f64, cells: usize) -> Vec<bool> {
    let mut mask = vec![false; cells * cells];
    if cells == 0 || size_mm <= 0.0 || !size_mm.is_finite() {
        return mask;
    }
    let cell = size_mm / (cells as f64);
    let half = 0.5 * size_mm;
    let last = (cells - 1) as f64;
    let mut rings: Vec<&[P2]> = Vec::new();
    for polygon in polygons {
        rings.push(&polygon.exterior);
        for hole in &polygon.holes {
            rings.push(hole);
        }
    }
    let mut crossings: Vec<f64> = Vec::new();
    for row in 0..cells {
        let y = -half + cell * (row as f64 + 0.5);
        crossings.clear();
        for ring in &rings {
            let n = ring.len();
            if n < 3 {
                continue;
            }
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % n];
                // Half-open in Y: a vertex exactly on the scanline is counted
                // once, never twice, so parity survives a horizontal edge.
                if (a.y <= y) == (b.y <= y) {
                    continue;
                }
                let t = (y - a.y) / (b.y - a.y);
                let x = a.x + t * (b.x - a.x);
                if x.is_finite() {
                    crossings.push(x);
                }
            }
        }
        crossings.sort_by(f64::total_cmp);
        for pair in crossings.as_chunks::<2>().0 {
            let lo = ((pair[0] + half) / cell - 0.5).ceil();
            let hi = ((pair[1] + half) / cell - 0.5).floor();
            if !lo.is_finite() || !hi.is_finite() {
                continue;
            }
            if hi < 0.0 || lo > last || hi < lo {
                continue;
            }
            let lo = lo.max(0.0) as usize;
            let hi = hi.min(last) as usize;
            for col in lo..=hi {
                mask[row * cells + col] = true;
            }
        }
    }
    mask
}

/// Build the medial grid for one region, printing every dial it chose.
///
/// Returns `None` — with the reason printed — when the region has no finite
/// extent to build a grid over. That is a refusal, not a silent skip.
fn build_medial_grid(label: &str, polygons: &[Polygon2], stepover_mm: f64) -> Option<MedialGrid> {
    eprintln!("\n   ===== MEDIAL-AXIS DISTANCE TRANSFORM — {label} =====");
    let mut reach = 0.0f64;
    for polygon in polygons {
        let [x0, y0, x1, y1] = polygon.bbox();
        for value in [x0, y0, x1, y1] {
            if value.is_finite() {
                reach = reach.max(value.abs());
            }
        }
    }
    if reach <= 0.0 || !reach.is_finite() || stepover_mm <= 0.0 {
        eprintln!(
            "     NOT BUILT: the region has no finite extent about the origin (reach \
             {reach:.4} mm,\n\
             \x20    stepover {stepover_mm:.5} mm). This is a REFUSAL with a reason, not a \
             silent skip."
        );
        return None;
    }
    let size_mm = 2.0 * (reach + MEDIAL_GRID_PAD_MM);
    let target_cell = stepover_mm / MEDIAL_CELLS_PER_STEPOVER;
    let cells = (size_mm / target_cell).round().max(0.0);
    let cells = if cells.is_finite() {
        (cells as usize).clamp(MEDIAL_MIN_CELLS, MEDIAL_MAX_CELLS)
    } else {
        MEDIAL_MIN_CELLS
    };
    let cell_mm = size_mm / (cells as f64);

    let raw_mask = rasterise_polygons(polygons, size_mm, cells);
    let (inside, fill) = fill_mask_holes(&raw_mask, size_mm, cells);
    let outside: Vec<bool> = inside.iter().map(|&v| !v).collect();
    let edt = distance_transform_2d(&outside, cells, cells);

    // One 3 × 3 box blur — the smoothed tier's input, and the ONLY smoothing
    // anywhere in this construction. The raw tier differences the unblurred
    // transform, so a clean sample is never softened by the fallback's
    // machinery.
    let mut blurred = vec![0.0f64; cells * cells];
    for row in 0..cells {
        for col in 0..cells {
            let mut sum = 0.0f64;
            let mut count = 0.0f64;
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    let r = row as i64 + dr;
                    let c = col as i64 + dc;
                    if r < 0 || c < 0 {
                        continue;
                    }
                    let (r, c) = (r as usize, c as usize);
                    if r >= cells || c >= cells {
                        continue;
                    }
                    sum += edt[r * cells + c];
                    count += 1.0;
                }
            }
            blurred[row * cells + col] = sum / count.max(1.0);
        }
    }

    let grid = MedialGrid {
        cells,
        size_mm,
        edt,
        blurred,
        inside,
        fill,
    };
    eprintln!(
        "     EDT: rs_cam_core::geometry::grid_field::distance_transform_2d (Felzenszwalb-Huttenlocher,\n\
         \x20    the SHIPPED transform — the same one region_mask / tier_islands / \
         finish_planner call),\n\
         \x20    fed the COMPLEMENT of the region mask, so each inside cell reads its distance \
         to the\n\
         \x20    nearest OUTSIDE cell. Units are CELLS, because |grad d| = 1 exactly in those \
         units\n\
         \x20    and that is what makes the {MEDIAL_GRADIENT_FLOOR} degeneracy floor checkable."
    );
    eprintln!(
        "     grid  {cells} x {cells} cells, side {size_mm:.4} mm, cell {cell_mm:.5} mm \
         ({:.2} cells/stepover),",
        stepover_mm / cell_mm
    );
    eprintln!(
        "\x20          pad {MEDIAL_GRID_PAD_MM} mm outside a region reach of {reach:.4} mm; \
         square and ORIGIN-CENTRED so"
    );
    eprintln!(
        "\x20          fill_mask_holes (the in-file G-CELLHOLE repair) applies to it verbatim."
    );
    eprintln!(
        "     inside cells {} = {:.4} mm^2 in XY projection.   PEAK INSCRIBED RADIUS {:.4} mm",
        grid.inside_cells(),
        grid.inside_cells() as f64 * cell_mm * cell_mm,
        grid.peak_inscribed_mm()
    );
    eprintln!(
        "\x20          (the EDT's maximum: on a branched region that is the HUB, and it is what \
         decides\n\
         \x20          how many level sets the hub can hold at a {stepover_mm:.5} mm pitch — \
         about {:.1}.)",
        grid.peak_inscribed_mm() / stepover_mm
    );
    print_mask_fill(&format!("{label} — MEDIAL GRID"), &grid.fill, cell_mm);
    Some(grid)
}

/// **The operator's candidate.** The same Poisson machinery the sweep arm
/// runs, fed a target field whose direction is the region's OWN SHAPE.
///
/// # The target field V, derived
///
/// Per region triangle, given its unit normal `n` (oriented `+Z` by the
/// heightfield generators) and its centroid:
///
/// 1. **Sample `∇EDT`** at the centroid's XY by central differences on the
///    medial grid — raw first, smoothed as a counted fallback, `V = 0` as a
///    counted last resort. Call the unit result `g`, pointing ACROSS the arm.
/// 2. **Lift and project**: `g₃ = (g_x, g_y, 0)`, `ĝ = normalise(g₃ − n(n·g₃))`
///    — `g` in the triangle's own plane.
/// 3. **Rotate 90° about `n`**: `D = n × ĝ`. That is the FEED direction, and
///    on a thin arm it runs ALONG the arm's length. This is the whole idea.
/// 4. **Hand `D` to the pinned convention**: `V_dir = sweep_target_axis(n, D)`,
///    the same helper the `D = sweep` arm uses and the one
///    [`k_s_is_read_across_the_passes_not_along_them`] pins. It returns
///    `n × D = n × (n × ĝ) = −ĝ`, i.e. the stepover is taken ACROSS the arm.
/// 5. **Magnitude**: Zou Eq. 13, `|V| = √((k_s + 1/K_c)/8)`, with `k_s` the
///    ANALYTIC normal curvature along `V_dir` — read ACROSS the passes, never
///    along them, by the same code path and for the same reason the sweep arm
///    states at length. Same clamp, same fallback, same counter.
///
/// Step 4 is a round trip that could have been written as `−ĝ` directly. It is
/// not, deliberately: routing through [`sweep_target_axis`] means the one line
/// of the derivation that can be silently inverted is the SAME line in both
/// field arms, and [`medial_target_axis_is_the_negated_in_plane_gradient`]
/// pins the identity rather than trusting it.
///
/// # What the tripwires are on THIS arm
///
/// Two of the printed counters are NOT alike and the block says so. The
/// **in-plane projection** of the sampled XY gradient can genuinely degenerate
/// — it needs a facet perpendicular to that gradient, i.e. a vertical wall —
/// so it is a live tripwire expected to read `0` on fixtures capped at ~31° of
/// slope, exactly as on the sweep arm. The **`sweep_target_axis` failure**
/// cannot fire at all: `D` is constructed in the triangle's plane, so the
/// helper is handed a vector already perpendicular to `n`. It is printed as a
/// structural assertion and labelled as one, never as evidence.
///
/// **This arm's real tripwires are the GRADIENT TIERS**, and they are not
/// expected to read zero — the medial axis is where `∇EDT` is genuinely
/// ill-conditioned, and a construction named after the medial axis owes the
/// reader the size of that set. What would be a defect is a LARGE degenerate
/// fraction, which would say the grid is too coarse to resolve the shape
/// rather than that the shape has a skeleton.
fn medial_field_candidate(
    label: &str,
    fixture: Fixture<'_>,
    region_triangles: &[u32],
    polygons: &[Polygon2],
    surface: AnalyticSurface,
    grid: &MedialGrid,
) -> FieldCandidate {
    let params = FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let inverse_radius = 1.0 / BALL_RADIUS_MM;
    let flat_magnitude = (inverse_radius / 8.0).sqrt();
    let clamped = Cell::new(0usize);
    let samples = Cell::new(0usize);
    let raw_tier = Cell::new(0usize);
    let smoothed_tier = Cell::new(0usize);
    let degenerate_tier = Cell::new(0usize);
    let below_floor = Cell::new(0usize);
    let unmeasured = Cell::new(0usize);
    let snapped = Cell::new(0usize);
    let axis_failures = Cell::new(0usize);
    let projection_failures = Cell::new(0usize);

    eprintln!("\n   ===== MEDIAL-AXIS FIELD — {label} =====");
    eprintln!(
        "     D = rotate(grad EDT, 90 deg about n)  =>  V_dir = n x D = -grad_hat(EDT), so\n\
         \x20    V = -|V| * grad_hat(EDT) and phi is a REPARAMETERISED NEGATIVE DISTANCE \
         TRANSFORM.\n\
         \x20    ITS LEVEL SETS ARE ISO-DISTANCE OFFSETS OF THE REGION BOUNDARY. In one line:\n\
         \x20    THIS ROW IS CONTOUR-PARALLEL (OFFSET) MACHINING WITH THE ISO-SCALLOP SPACING \
         LAW\n\
         \x20    ATTACHED. On a thin arm those offsets run ALONG its length (the operator's \
         first\n\
         \x20    clause); around a hub they close into loops encircling it (the second)."
    );
    eprintln!(
        "     THE COMPETING PRIOR, quoted so this row cannot be read without it:\n\
         \x20    planning/thin_organic_2026-08-27/FINDINGS.md\n\
         \x20      §0j  per-cell sweep DIRECTION variation .... 0.917x r1 / 0.921x top-three \
         — a COST\n\
         \x20      §0k  contour-per-cell ...................... 0.686x r1 / 0.710x top-three \
         — a LOSS\n\
         \x20    §0k is the DIRECT prior: this row is a contour strategy and §0k measured \
         contour\n\
         \x20    losing by a third. WHAT DIFFERS: both were LATTICE-DERIVED monotone CELLS, each\n\
         \x20    re-rotated or re-contoured on its own lattice, and §0j's own diagnosis of its \
         loss\n\
         \x20    is that misaligned neighbouring lattices break the cross-cell serpentine chords\n\
         \x20    the relinker stitches (+9% cutting distance). THIS arm has no cells and no \
         lattice:\n\
         \x20    ONE continuous SHAPE-derived field over the whole region, so there are no cell\n\
         \x20    seams to break. That is a different proposition, NOT a refutation of §0j/§0k, \
         and\n\
         \x20    nothing here claims one. Let the number decide."
    );
    eprintln!(
        "     Same FieldParams::new(K_c, h), same Poisson solve, same level schedule and same\n\
         \x20    marching-triangles extraction as BOTH rows above. THE ONLY DIFFERENCE IS V."
    );

    let (result, report) = direction_field::solve_paths_with_target(
        fixture.mesh,
        region_triangles,
        |_global, centroid, n| {
            samples.set(samples.get() + 1);
            let sample = grid.across_at(centroid.x, centroid.y);
            if sample.measured {
                if sample.raw_magnitude < MEDIAL_GRADIENT_FLOOR {
                    below_floor.set(below_floor.get() + 1);
                }
            } else {
                unmeasured.set(unmeasured.get() + 1);
            }
            if sample.snapped {
                snapped.set(snapped.get() + 1);
            }
            match sample.tier {
                GradientTier::Raw => raw_tier.set(raw_tier.get() + 1),
                GradientTier::Smoothed => smoothed_tier.set(smoothed_tier.get() + 1),
                GradientTier::Degenerate => {
                    degenerate_tier.set(degenerate_tier.get() + 1);
                }
            }
            let Some((gx, gy)) = sample.across else {
                return V3::zeros();
            };
            let across = V3::new(gx, gy, 0.0);
            let projected = across - n * across.dot(&n);
            let norm = projected.norm();
            if norm.is_nan() || norm <= 1e-9 {
                projection_failures.set(projection_failures.get() + 1);
                return V3::zeros();
            }
            // D = rotate(grad EDT, 90 deg about n) — ALONG the arm.
            let feed = n.cross(&(projected / norm));
            // ... then the PINNED convention turns the feed direction into the
            // stepover direction. See this function's docs for why it is a
            // round trip on purpose.
            let Some(unit) = sweep_target_axis(n, feed) else {
                axis_failures.set(axis_failures.get() + 1);
                return V3::zeros();
            };
            let k_s = surface.normal_curvature(centroid.x, centroid.y, unit);
            let denominator = k_s + inverse_radius;
            let magnitude = if denominator > 1e-12 {
                (denominator / 8.0).sqrt()
            } else {
                clamped.set(clamped.get() + 1);
                flat_magnitude
            };
            unit * magnitude
        },
        &params,
    );

    let total = samples.get().max(1) as f64;
    eprintln!("\n     -- GRADIENT TIERS (this arm's real tripwires, and NOT expected to be 0) --");
    eprintln!("       target-field samples        {:>10}", samples.get());
    eprintln!(
        "       tier RAW (|grad| >= {MEDIAL_GRADIENT_FLOOR}) {:>10}   {:>7.3}%",
        raw_tier.get(),
        100.0 * raw_tier.get() as f64 / total
    );
    eprintln!(
        "       tier SMOOTHED (fallback)    {:>10}   {:>7.3}%",
        smoothed_tier.get(),
        100.0 * smoothed_tier.get() as f64 / total
    );
    eprintln!(
        "       tier DEGENERATE (V = 0)     {:>10}   {:>7.3}%",
        degenerate_tier.get(),
        100.0 * degenerate_tier.get() as f64 / total
    );
    eprintln!(
        "       |grad EDT| BELOW the floor  {:>10}   {:>7.3}%   <<< the honest size of the \
         MEDIAL AXIS",
        below_floor.get(),
        100.0 * below_floor.get() as f64 / total
    );
    eprintln!(
        "       NOT MEASURED (no difference)  {:>8}   {:>7.3}%",
        unmeasured.get(),
        100.0 * unmeasured.get() as f64 / total
    );
    eprintln!(
        "       ^ off-grid samples and snap failures carry raw_magnitude 0.0 as a SENTINEL, and\n\
         \x20      folding those into the row above would report a NOT-MEASURED value as a \
         measured\n\
         \x20      zero. They are counted apart for the same reason this repo's report-only \
         findings\n\
         \x20      distinguish None from Some(0.0). Expected 0 here: the snap radius covers the\n\
         \x20      fringe, so a nonzero count means the grid and the mesh disagree about the \
         region."
    );
    eprintln!(
        "       BOUNDARY SNAPS (<= {MEDIAL_SNAP_RADIUS} cells) {:>8}   {:>7.3}%   <<< a \
         RE-RASTERISATION artefact,",
        snapped.get(),
        100.0 * snapped.get() as f64 / total
    );
    eprintln!(
        "\x20                                                       NOT a fact about the shape"
    );
    eprintln!(
        "       ^ the medial grid's cell is sized from the STEPOVER, the mesh's from the facet\n\
         \x20      ceiling, so a fringe centroid can land one cell the wrong side of a staircase\n\
         \x20      edge. Outside cells carry EDT = 0 with ZERO gradient, so without the snap the\n\
         \x20      whole outer fringe would degenerate and the outermost level set — the one \
         that\n\
         \x20      hugs the boundary — would be the one distorted. Expect this to scale with\n\
         \x20      PERIMETER/AREA: a few percent on a compact region, more on a branched one."
    );
    eprintln!(
        "       An exact EDT has |grad d| = 1 in cell units wherever the nearest boundary site \
         is\n\
         \x20      UNIQUE, and collapses only where it is not — which IS the medial axis. So the\n\
         \x20      BELOW-FLOOR row is a measurement of the skeleton's discrete width, not an \
         error\n\
         \x20      rate. A LARGE degenerate fraction would be the defect: it would say the grid \
         is\n\
         \x20      too coarse to resolve the shape. The last resort is V = 0 — NOT the sweep\n\
         \x20      direction — because injecting +X at exactly the triangles this experiment is\n\
         \x20      about would make the medial arm partly the sweep arm and destroy the \
         attribution.\n\
         \x20      A zero target lets the Poisson solve interpolate the ridge from its \
         neighbourhood,\n\
         \x20      which is the physically right answer there."
    );
    eprintln!(
        "       in-plane projection failures {:>9}   <<< a TRIPWIRE, expected 0",
        projection_failures.get()
    );
    eprintln!(
        "       ^ this one CAN fire: it needs a facet perpendicular to the sampled XY gradient\n\
         \x20      (a vertical wall), exactly as on the sweep arm, and every fixture here is \
         bounded\n\
         \x20      below ~31 deg of slope. A nonzero count means the fixture is not what its own\n\
         \x20      smoke test says it is."
    );
    eprintln!(
        "       sweep_target_axis failures   {:>9}   <<< STRUCTURAL, not a live check",
        axis_failures.get()
    );
    eprintln!(
        "       ^ D is CONSTRUCTED in the triangle's plane, so the helper is handed a vector\n\
         \x20      already perpendicular to n and cannot fail. Printed as an assertion, and \
         labelled\n\
         \x20      as one rather than sitting beside the tier counts as if it were evidence."
    );
    eprintln!("       |V| clamps (gouge condition) {:>9}", clamped.get());
    if clamped.get() > 0 {
        eprintln!(
            "       *** THE CLAMP FIRED. It needs a concavity tighter than K_c, and every \
             fixture\n\
             \x20      asserts R_min >= 2 K_c in its own smoke test. The fixture is not what its \
             own\n\
             \x20      smoke test says it is."
        );
    }
    print_field_report(label, &result, &report, FieldSource::Medial);
    cost_field_result(fixture, polygons, &result, &report, FieldSource::Medial)
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

// ---- shared heightfield machinery (ARM RIBBON + ARM BAND) -------------
//
// Both new arms are heightfields on a regular square grid, exactly like
// `wavy_patch_mesh`, and both address individual CELLS to build their region.
// The generator, the cell-centre formula and the cell→triangle formula are
// shared so the two arms cannot disagree about indexing — a disagreement there
// would silently machine a different region from the one being censused.

/// Cells per side for a heightfield patch, from the **same** edge budget
/// [`wavy_grid_cells`] uses, but taking the surface's own gradient bound as a
/// parameter instead of re-deriving it from one particular closed form.
///
/// The binding edge of a split quad is its **diagonal**: `√2·cell` in XY, plus
/// whatever `z` the surface climbs across it. Bounding that climb by the
/// steepest gradient `|∇z|max` gives a 3D diagonal of at most
/// `√2·cell / cos θmax`, so
///
/// ```text
/// cell ≤ edge_mm · cos θmax / √2
/// ```
///
/// [`wavy_grid_cells`] is left exactly as it is: it is what produced ARM
/// WAVY's published census and re-expressing it through this function would
/// make a working arm's numbers depend on an edit made for a different arm.
fn heightfield_cells(size_mm: f64, max_gradient: f64, edge_mm: f64) -> usize {
    let cos_min = 1.0 / (1.0 + max_gradient * max_gradient).sqrt();
    let cell = edge_mm * cos_min / SQRT_2;
    ((size_mm / cell.max(1e-9)).ceil() as usize).max(2)
}

/// A square heightfield patch on a regular `cells × cells` grid, centred on
/// the origin.
///
/// Wound exactly as [`wavy_patch_mesh`] winds — cell `(row, col)` contributes
/// triangles `2·(row·cells + col)` and `+1`, as `[a, b, d]` then `[a, d, c]`,
/// counter-clockwise in XY — so every face normal has `n_z > 0` and
/// [`cell_triangles`] addresses the same pair `wavy_region_triangles` does.
fn heightfield_mesh(size_mm: f64, cells: usize, height: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let cells = cells.max(2);
    let cell = size_mm / (cells as f64);
    let half = 0.5 * size_mm;

    let mut verts: Vec<P3> = Vec::with_capacity((cells + 1) * (cells + 1));
    for row in 0..=cells {
        let y = -half + cell * (row as f64);
        for col in 0..=cells {
            let x = -half + cell * (col as f64);
            verts.push(P3::new(x, y, height(x, y)));
        }
    }

    let idx = |row: usize, col: usize| -> u32 { (row * (cells + 1) + col) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(2 * cells * cells);
    for row in 0..cells {
        for col in 0..cells {
            let (a, b) = (idx(row, col), idx(row, col + 1));
            let (c, d) = (idx(row + 1, col), idx(row + 1, col + 1));
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// World XY centre of grid cell `(row, col)`.
fn cell_centre(size_mm: f64, cells: usize, row: usize, col: usize) -> (f64, f64) {
    let cell = size_mm / (cells.max(1) as f64);
    let half = 0.5 * size_mm;
    (
        -half + cell * (col as f64 + 0.5),
        -half + cell * (row as f64 + 0.5),
    )
}

/// The two triangle indices of grid cell `(row, col)` — the same formula
/// [`wavy_region_triangles`] uses.
fn cell_triangles(cells: usize, row: usize, col: usize) -> [u32; 2] {
    let base = 2 * (row * cells.max(1) + col);
    [base as u32, (base + 1) as u32]
}

/// Every triangle of every `true` cell, ascending.
///
/// **Whole cells, never a per-triangle centroid test.** That discipline is
/// [`wavy_region_triangles`]' and it is load-bearing: a per-triangle test
/// splits cells on the fringe, and two diagonally-adjacent surviving triangles
/// meet at a single vertex — the bowtie `region_topology` refuses as a
/// `BoundaryPinch`, and exactly the class of **selection artefact** the
/// FINDINGS_F2 withdrawal is about. Taking cells whole makes any region a
/// polyomino: 4-connected wherever the mask is, corner-free by construction.
fn cells_to_triangles(mask: &[bool], cells: usize) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for row in 0..cells {
        for col in 0..cells {
            if mask.get(row * cells + col).copied().unwrap_or(false) {
                out.extend_from_slice(&cell_triangles(cells, row, col));
            }
        }
    }
    out
}

/// Turn a `cells × cells` boolean **cell** mask into world polygons *with
/// holes*, through the production marching-squares path.
///
/// Two properties this buys, both load-bearing:
///
/// * the polygon is derived from the **same mask** that selected the
///   triangles, so the containment reference and the machined region cannot
///   drift apart the way terrain's 30-vertex ellipse drifted from its
///   centroid-selected triangles;
/// * `polygon::detect_containment` (what `monotone_cells::region_polygons`
///   uses) nests an enclosed loop as a **HOLE** of its container rather than
///   emitting it as a separate polygon. That nesting is the whole point on ARM
///   BAND, whose headline claim is about holes.
///
/// The mask is padded with one ring of `false` so every loop closes inside the
/// array instead of being clipped at its edge. Grid index `(r, c)` of the
/// padded array sits at the **centre** of mask cell `(r−1, c−1)`, which is
/// what the origin below encodes.
fn mask_polygons(mask: &[bool], size_mm: f64, cells: usize) -> Vec<Polygon2> {
    use rs_cam_core::geometry::contour_extract::marching_squares_bool_grid;
    use rs_cam_core::polygon::{detect_containment, shoelace_area};

    let cells = cells.max(1);
    let cell = size_mm / (cells as f64);
    let half = 0.5 * size_mm;
    let padded = cells + 2;
    let mut grid = vec![false; padded * padded];
    for row in 0..cells {
        for col in 0..cells {
            if mask.get(row * cells + col).copied().unwrap_or(false) {
                grid[(row + 1) * padded + (col + 1)] = true;
            }
        }
    }
    let origin = -half - 0.5 * cell;
    let loops = marching_squares_bool_grid(&grid, padded, padded, origin, origin, cell);
    let candidates: Vec<Polygon2> = loops
        .into_iter()
        .filter(|points| points.len() >= 3 && shoelace_area(points).abs() > 1e-12)
        .map(Polygon2::new)
        .collect();
    let mut polygons = detect_containment(candidates);
    for polygon in &mut polygons {
        polygon.ensure_winding();
    }
    polygons
}

// ---- G-CELLHOLE: enclosed pockets a grid digitisation invents ----------

/// The four in-grid neighbours of a cell — **4-connectivity, deliberately**.
///
/// Two cells that share only a CORNER share no triangle edge, so
/// [`clean_selection`]'s edge adjacency and `mesh_census`'s edge bookkeeping
/// both treat them as disconnected. A complement pocket that is 8-connected to
/// the outside but **not** 4-connected is therefore an enclosed HOLE as far as
/// `region_topology` is concerned, which is exactly the case measured below.
fn four_neighbours(r: usize, c: usize, cells: usize) -> [Option<(usize, usize)>; 4] {
    let up = if r > 0 { Some((r - 1, c)) } else { None };
    let down = if r + 1 < cells {
        Some((r + 1, c))
    } else {
        None
    };
    let left = if c > 0 { Some((r, c - 1)) } else { None };
    let right = if c + 1 < cells {
        Some((r, c + 1))
    } else {
        None
    };
    [up, down, left, right]
}

/// One enclosed pocket of unselected cells that [`fill_mask_holes`] closed.
#[derive(Clone, Copy)]
struct MaskHole {
    cells: usize,
    area_mm2: f64,
    centroid_radius_mm: f64,
    centroid_angle_deg: f64,
}

/// What [`fill_mask_holes`] repaired.
///
/// **A fixture that silently repairs itself is worse than one that says what it
/// repaired**, so every field here is printed at the point of use and the
/// arm's header carries the artefact as a measured observation, not as a bug
/// that was quietly swept up.
struct MaskFillReport {
    holes: Vec<MaskHole>,
    cells_before: usize,
    cells_filled: usize,
    area_filled_mm2: f64,
}

/// Close every enclosed pocket in a cell mask, and report each one.
///
/// # G-CELLHOLE (measured 2026-08-30) — why this exists
///
/// ARM RIBBON's region is a union of capsules that **all contain the origin**.
/// Every capsule is convex, so the union is star-shaped about the origin and
/// therefore **simply connected, with exactly one boundary loop**. That
/// continuum argument is correct and is not what broke.
///
/// Digitised onto a 0.1057 mm grid, the same region came back with **Euler −3
/// and four holes**. Diagnosed: each hole is exactly **one cell**, at
/// `r = 3.363 mm` on the bisectors at **45°, 135°, 225° and 315°** — four of
/// the eight inter-arm bisectors, and precisely the four that lie **diagonal
/// to the axis-aligned cell lattice**.
///
/// The mechanism, and it is general:
///
/// * adjacent arms are 45° apart, so the wedge between them has its apex where
///   the bisector leaves both capsules: `r = w / sin(22.5°) = 1.25/0.38268 =
///   **3.2664 mm**`. Inside that radius the arms are merged; outside it the
///   wedge opens monotonically and runs to the exterior. **In the continuum
///   there is no pocket anywhere.**
/// * the first cell centre outward along a bisector that clears both capsules
///   sits at `r = 3.363`. Its four EDGE neighbours are all still inside the
///   union (the wedge is far narrower than a cell there), and the next
///   excluded cell outward is its DIAGONAL neighbour. So the pocket is
///   8-connected to the open wedge and **4-connected to nothing** — an
///   enclosed hole under the only connectivity `region_topology` recognises.
/// * it fires on the four diagonal bisectors and not on the four axis-aligned
///   ones because on an axis-aligned bisector the wedge advances along a grid
///   row or column and stays 4-connected.
///
/// This is the **dual of the bowtie** [`clean_selection`] already repairs:
/// there the SELECTION pinched at a corner, here the COMPLEMENT does. Same
/// cause — a feature thinner than a cell meeting a lattice diagonally — same
/// class of repair, and equally a fact about the digitisation rather than
/// about the surface.
///
/// Total area involved: **0.0447 mm², 0.024 % of the region**. Topologically
/// fatal, metrically nothing.
fn fill_mask_holes(mask: &[bool], size_mm: f64, cells: usize) -> (Vec<bool>, MaskFillReport) {
    let mut report = MaskFillReport {
        holes: Vec::new(),
        cells_before: mask.iter().filter(|&&v| v).count(),
        cells_filled: 0,
        area_filled_mm2: 0.0,
    };
    if cells == 0 || mask.len() != cells * cells {
        return (mask.to_vec(), report);
    }
    let cell = size_mm / (cells as f64);
    let half = 0.5 * size_mm;
    let at = |r: usize, c: usize| r * cells + c;

    // 1. Flood the OUTSIDE: unselected cells reachable from the grid border.
    let mut outside = vec![false; cells * cells];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for i in 0..cells {
        for (r, c) in [(0, i), (cells - 1, i), (i, 0), (i, cells - 1)] {
            let idx = at(r, c);
            if !mask[idx] && !outside[idx] {
                outside[idx] = true;
                stack.push((r, c));
            }
        }
    }
    while let Some((r, c)) = stack.pop() {
        for slot in four_neighbours(r, c, cells) {
            let Some((rr, cc)) = slot else { continue };
            let idx = at(rr, cc);
            if !mask[idx] && !outside[idx] {
                outside[idx] = true;
                stack.push((rr, cc));
            }
        }
    }

    // 2. Everything unselected and unreached is an enclosed pocket. Group it,
    //    measure it, then fill it.
    let mut filled = mask.to_vec();
    let mut visited = vec![false; cells * cells];
    for row in 0..cells {
        for col in 0..cells {
            let idx = at(row, col);
            if mask[idx] || outside[idx] || visited[idx] {
                continue;
            }
            visited[idx] = true;
            let mut members: Vec<(usize, usize)> = vec![(row, col)];
            let mut queue: Vec<(usize, usize)> = vec![(row, col)];
            while let Some((r, c)) = queue.pop() {
                for slot in four_neighbours(r, c, cells) {
                    let Some((rr, cc)) = slot else { continue };
                    let j = at(rr, cc);
                    if !mask[j] && !outside[j] && !visited[j] {
                        visited[j] = true;
                        members.push((rr, cc));
                        queue.push((rr, cc));
                    }
                }
            }
            let count = members.len();
            let mut sum_x = 0.0f64;
            let mut sum_y = 0.0f64;
            for &(r, c) in &members {
                filled[at(r, c)] = true;
                sum_x += -half + cell * (c as f64 + 0.5);
                sum_y += -half + cell * (r as f64 + 0.5);
            }
            let cx = sum_x / count as f64;
            let cy = sum_y / count as f64;
            report.holes.push(MaskHole {
                cells: count,
                area_mm2: count as f64 * cell * cell,
                centroid_radius_mm: cx.hypot(cy),
                centroid_angle_deg: cy.atan2(cx).to_degrees().rem_euclid(360.0),
            });
            report.cells_filled += count;
        }
    }
    report.area_filled_mm2 = report.cells_filled as f64 * cell * cell;
    report.holes.sort_by(|a, b| {
        b.cells
            .cmp(&a.cells)
            .then(a.centroid_angle_deg.total_cmp(&b.centroid_angle_deg))
    });
    (filled, report)
}

/// Print the G-CELLHOLE repair as **evidence**, not as a swept-up bug.
fn print_mask_fill(label: &str, report: &MaskFillReport, cell_mm: f64) {
    eprintln!("\n   ===== G-CELLHOLE — ENCLOSED POCKETS THE DIGITISATION INVENTED — {label} =====");
    let area_before = report.cells_before as f64 * cell_mm * cell_mm;
    if report.holes.is_empty() {
        eprintln!(
            "     NONE. The cell mask's complement is 4-connected to the grid border everywhere,\n\
             \x20    so the selection is simply connected before any repair. Nothing was filled."
        );
        return;
    }
    eprintln!(
        "     enclosed pockets FILLED       {:>12}",
        report.holes.len()
    );
    eprintln!(
        "     cells filled / region cells   {:>12} / {}",
        report.cells_filled, report.cells_before
    );
    eprintln!(
        "     area filled (mm²)             {:>12.5}   = {:.4}% of the {area_before:.3} mm² region",
        report.area_filled_mm2,
        100.0 * report.area_filled_mm2 / area_before.max(1e-12)
    );
    eprintln!(
        "     {:>6} {:>8} {:>12} {:>12} {:>12}",
        "#", "cells", "area mm²", "radius mm", "angle deg"
    );
    for (i, hole) in report.holes.iter().take(16).enumerate() {
        eprintln!(
            "     {:>6} {:>8} {:>12.5} {:>12.4} {:>12.2}",
            i, hole.cells, hole.area_mm2, hole.centroid_radius_mm, hole.centroid_angle_deg
        );
    }
    if report.holes.len() > 16 {
        eprintln!(
            "     ... {} further pocket(s) not listed.",
            report.holes.len() - 16
        );
    }
    eprintln!(
        "     WHAT THIS IS, AND WHY IT IS A RESULT RATHER THAN A BUG THAT GOT FIXED.\n\
         \x20    A region that is simply connected IN THE CONTINUUM became MULTIPLY CONNECTED the\n\
         \x20    moment it was digitised onto a ~0.1 mm cell grid. The mechanism is a wedge whose\n\
         \x20    apex is thinner than one cell meeting an axis-aligned lattice DIAGONALLY: the\n\
         \x20    pocket ends up 8-connected to the open wedge outside it and 4-connected to\n\
         \x20    nothing, and 4-connectivity is the only kind `region_topology` recognises\n\
         \x20    (two cells sharing a corner share no triangle EDGE). It is the exact dual of the\n\
         \x20    bowtie clean_selection already repairs — there the SELECTION pinches at a corner,\n\
         \x20    here the COMPLEMENT does.\n\
         \x20    Why it matters beyond this fixture: the programme's real regions are SLOPE-BANDED,\n\
         \x20    and slope banding creates holes of its own. This says a branched region acquires\n\
         \x20    holes from DIGITISATION ON TOP OF those — so a hole count measured off a grid mask\n\
         \x20    is an upper bound on the surface's own topology, never a reading of it."
    );
    eprintln!(
        "     DOES THE SHIPPED EXTRACTOR SHARE THE HAZARD? Partly, and the mitigation is\n\
         \x20    incidental rather than designed. `region_mask::region_polygons_from_mask_reported`\n\
         \x20    — what `finish_planner` extracts every band polygon with — runs the SAME pipeline\n\
         \x20    this file's `mask_polygons` does: marching squares on the cell mask, then\n\
         \x20    `polygon::detect_containment`, with NO hole-filling step anywhere. Two things\n\
         \x20    differ, and neither is a fix:\n\
         \x20      1. it drops loops below `min_area = cell*cell`. A ONE-cell pocket's marching-\n\
         \x20         squares loop is a diamond of area cell²/2, so it is dropped — but a TWO-cell\n\
         \x20         diagonal pocket's loop is 3*cell²/2 and survives. The filter clears exactly\n\
         \x20         the smallest case and nothing above it.\n\
         \x20      2. the `overlap_mm` dilation (EDT, `dist <= overlap_mm/cell`) fills any pocket\n\
         \x20         narrower than it — at the 2.0 mm the D-16.1 fixtures use, everything here.\n\
         \x20         But `FinishPlannerParams::overlap_mm` DEFAULTS TO 0.0 and the caller derives\n\
         \x20         it, so on the default path the dilation is a no-op.\n\
         \x20    And neither touches the TRIANGLE SELECTION, which is what `plan_spiral` consumes\n\
         \x20    and what refuses on Euler. On the selection side the hazard is unmitigated by\n\
         \x20    anything shipped."
    );
}

// ---- the raster-fragmentation validity gate ---------------------------

/// What a 0° raster **must** do to a region for a stay-down method to have
/// anything to beat.
///
/// # Why this is a gate and not a curiosity
///
/// FINDINGS_F2 §F2-2 measured the spiral as 5–15 % slower than a 0° raster on
/// the sphere cap and the wavy patch — and on those fixtures the raster
/// produced **4 and 3 fragments with ZERO retracts**. The spiral's entire
/// value proposition is one continuous stay-down path with no lifts, and it
/// had nothing to beat. On the real target geometry (Wanaka thin-organic
/// region 1, `planning/thin_organic_2026-08-27/FINDINGS.md`) a 0° raster
/// produces **564 fragments and 97 kept retracts**, and even the tuned PCA-cell
/// plan carries **141 fragments / 53 retracts**. A retract-elimination method
/// was benchmarked on geometry with no retracts to eliminate.
///
/// So a fixture that does not fragment a raster **proves nothing about this
/// method**, and that has to be checked *before* the cost table rather than
/// discovered afterwards.
///
/// # What it measures
///
/// The measure is `FINDINGS.md` §Stage C's, restated: scan rows at the tier's
/// own stepover and count **maximal inside-runs per row**, "which is exactly
/// what `raster_toolpath_from_grid` emits as separate fragments". Crossings are
/// gathered from every ring of every polygon — exteriors and holes alike — and
/// paired under the even-odd rule, with the half-open `y` test that makes a
/// vertex sitting exactly on the scan line count once rather than twice.
struct ScanlineCensus {
    /// Scan rows laid across the region's bounding box at the stepover pitch.
    rows: usize,
    /// Rows that met the region at all.
    rows_with_material: usize,
    /// Rows that met it in **more than one** disjoint run — every one of those
    /// is a lift a stay-down path would not pay.
    rows_multi_run: usize,
    /// Total maximal inside-runs over every row: the fragment count a 0°
    /// raster is forced into by the shape alone.
    total_runs: usize,
    /// Most runs on any single row.
    max_runs_in_row: usize,
    /// `total_runs / rows_with_material`.
    mean_runs_per_active_row: f64,
}

/// Crossings of one closed ring with the horizontal line `y`.
///
/// Half-open in `y`: an edge counts when one endpoint is at or below the line
/// and the other strictly above, so a vertex sitting exactly on the line is
/// counted once rather than twice and a horizontal edge is never counted.
fn ring_crossings(ring: &[P2], y: f64) -> usize {
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    let mut hits = 0usize;
    for i in 0..n {
        let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else {
            continue;
        };
        if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
            hits += 1;
        }
    }
    hits
}

/// Inside-runs on one horizontal scan line, by even-odd parity over all rings.
///
/// Every ring — exterior and hole alike — contributes its crossings to one
/// parity count, which is the even-odd rule; the number of maximal inside
/// intervals is then half the crossing count.
fn scanline_runs(polygons: &[Polygon2], y: f64) -> usize {
    let mut crossings = 0usize;
    for polygon in polygons {
        crossings += ring_crossings(&polygon.exterior, y);
        for hole in &polygon.holes {
            crossings += ring_crossings(hole, y);
        }
    }
    crossings / 2
}

fn scanline_census(polygons: &[Polygon2], pitch_mm: f64) -> ScanlineCensus {
    let mut y_lo = f64::INFINITY;
    let mut y_hi = f64::NEG_INFINITY;
    for polygon in polygons {
        let [_, lo, _, hi] = polygon.bbox();
        y_lo = y_lo.min(lo);
        y_hi = y_hi.max(hi);
    }
    let mut census = ScanlineCensus {
        rows: 0,
        rows_with_material: 0,
        rows_multi_run: 0,
        total_runs: 0,
        max_runs_in_row: 0,
        mean_runs_per_active_row: f64::NAN,
    };
    if !(y_lo.is_finite() && y_hi.is_finite()) || pitch_mm <= 0.0 || y_hi <= y_lo {
        return census;
    }
    let rows = (((y_hi - y_lo) / pitch_mm).floor() as usize).max(1);
    for i in 0..rows {
        let y = y_lo + pitch_mm * (i as f64 + 0.5);
        if y > y_hi {
            break;
        }
        census.rows += 1;
        let runs = scanline_runs(polygons, y);
        census.total_runs += runs;
        if runs > 0 {
            census.rows_with_material += 1;
        }
        if runs > 1 {
            census.rows_multi_run += 1;
        }
        census.max_runs_in_row = census.max_runs_in_row.max(runs);
    }
    if census.rows_with_material > 0 {
        census.mean_runs_per_active_row =
            census.total_runs as f64 / census.rows_with_material as f64;
    }
    census
}

/// Reference numbers this instrument's fixtures are standing in for.
/// `planning/thin_organic_2026-08-27/FINDINGS.md` §0i / §"Why the 20.1×
/// junction win did not translate", Wanaka thin-organic **region 1**.
const WANAKA_R1_RASTER_FRAGMENTS: usize = 564;
/// See [`WANAKA_R1_RASTER_FRAGMENTS`].
const WANAKA_R1_RASTER_RETRACTS: usize = 97;
/// See [`WANAKA_R1_RASTER_FRAGMENTS`] — the tuned PCA-minor-cell plan, which is
/// the production-validated bar, not the naive baseline.
const WANAKA_R1_PCA_FRAGMENTS: usize = 141;
/// See [`WANAKA_R1_PCA_FRAGMENTS`].
const WANAKA_R1_PCA_RETRACTS: usize = 53;

/// Rows-with-more-than-one-run below this **fraction** of the rows that meet
/// the region at all means the fixture did not reproduce the target geometry
/// class.
///
/// **[REPO] bar, and it is a validity gate on the FIXTURE, not a result about
/// the method.** A convex disk scores 0.0 here by construction — which is
/// precisely why the sphere and wavy arms could not test the claim. A third of
/// the active rows breaking is the minimum at which "a raster must lift
/// repeatedly inside this region" is a fact about the shape rather than an
/// edge effect.
const MIN_MULTI_RUN_ROW_FRACTION: f64 = 1.0 / 3.0;

/// Print the validity gate. Returns whether the fixture reproduced the class.
fn print_fragmentation_gate(label: &str, census: &ScanlineCensus, stepover_mm: f64) -> bool {
    let fraction = if census.rows_with_material == 0 {
        0.0
    } else {
        census.rows_multi_run as f64 / census.rows_with_material as f64
    };
    let valid = fraction >= MIN_MULTI_RUN_ROW_FRACTION && census.max_runs_in_row > 1;
    eprintln!("\n   ===== RASTER-FRAGMENTATION VALIDITY GATE — {label} =====");
    eprintln!(
        "     This runs BEFORE the cost table on purpose. FINDINGS_F2 §F2-2 benchmarked a\n\
         \x20    RETRACT-ELIMINATION method on two fixtures where the raster produced 4 and 3\n\
         \x20    fragments with ZERO RETRACTS. There was nothing to eliminate, so the 5-15%\n\
         \x20    'slower' verdict was measured on geometry the claim does not address. If this\n\
         \x20    fixture does not fragment a raster either, THIS ARM PROVES NOTHING and says so\n\
         \x20    here rather than letting a cost table be read as a verdict."
    );
    eprintln!(
        "     scan rows at the stepover     {:>12}   (pitch {stepover_mm:.4} mm)",
        census.rows
    );
    eprintln!(
        "     rows meeting the region       {:>12}",
        census.rows_with_material
    );
    eprintln!(
        "     rows with MORE THAN ONE run   {:>12}   = {:.1}% of active rows (bar: >= {:.1}%)",
        census.rows_multi_run,
        100.0 * fraction,
        100.0 * MIN_MULTI_RUN_ROW_FRACTION
    );
    eprintln!(
        "     total inside-runs             {:>12}   <<< the fragment count the SHAPE forces on \
         a 0deg raster",
        census.total_runs
    );
    eprintln!(
        "     most runs on one row          {:>12}",
        census.max_runs_in_row
    );
    eprintln!(
        "     mean runs per active row      {:>12.3}",
        census.mean_runs_per_active_row
    );
    eprintln!(
        "     WHAT THIS FIXTURE STANDS IN FOR — Wanaka thin-organic REGION 1\n\
         \x20    (planning/thin_organic_2026-08-27/FINDINGS.md):\n\
         \x20      0deg raster, undivided : {WANAKA_R1_RASTER_FRAGMENTS} fragments / \
         {WANAKA_R1_RASTER_RETRACTS} kept retracts\n\
         \x20      PCA-minor cells (the production-validated bar) : {WANAKA_R1_PCA_FRAGMENTS} \
         fragments / {WANAKA_R1_PCA_RETRACTS} kept retracts\n\
         \x20    Those are the numbers a stay-down method exists to attack. The relinker collapses\n\
         \x20    757 scan-line crossings to 97 ACTUAL retracts there, so read RETRACTS, not\n\
         \x20    crossings, as the thing being eliminated — the run census above is the upper\n\
         \x20    bound the relinker then works down from."
    );
    if valid {
        eprintln!(
            "\n     GATE: PASS — the fixture reproduces the target class. The cost table below \
             is\n\
             \x20    about a region a raster genuinely has to lift out of."
        );
    } else {
        eprintln!(
            "\n     GATE: *** FAIL — THIS FIXTURE DID NOT REPRODUCE THE TARGET GEOMETRY CLASS. \
             ***\n\
             \x20    Only {:.1}% of active rows break into more than one run (bar {:.1}%), max \
             runs on a row {}.\n\
             \x20    A 0deg raster over this region barely lifts, so the stay-down claim has \
             nothing to beat\n\
             \x20    and EVERY COST NUMBER BELOW IS THE SPHERE/WAVY MISTAKE REPEATED. Do not quote \
             the trade\n\
             \x20    verdict from this arm; fix the fixture (narrower arms, more of them, or a \
             finer band).",
            100.0 * fraction,
            100.0 * MIN_MULTI_RUN_ROW_FRACTION,
            census.max_runs_in_row
        );
    }
    valid
}

// ---- slope banding + component topology (ARM BAND) --------------------

/// Slope of a face from horizontal, in degrees: `acos(|n_z|)`.
///
/// `|n_z|`, not `n_z`: `build_region_mesh` (and therefore `conformal_spiral`,
/// `crest_lines` and `direction_field`) force normals to `+Z`, and the shipped
/// `SlopeMap` measures the same unsigned angle. Taking the absolute value here
/// means an inconsistently wound input cannot report a 170° "slope".
fn face_slope_deg(normal: rs_cam_core::geo::V3) -> f64 {
    let length = normal.norm();
    if length <= 1e-12 {
        return f64::NAN;
    }
    let cosine = (normal.z / length).abs().clamp(0.0, 1.0);
    cosine.acos().to_degrees()
}

/// Half-open slope band `[lo, hi)` — the shape the shipped three-way
/// `finish_planner` label uses (`VerySteep` if `>= waterline`, else
/// `MidSteep` if `>= steep`, else `Shallow`).
fn slope_in_band(slope_deg: f64, lo_deg: f64, hi_deg: f64) -> bool {
    slope_deg.is_finite() && slope_deg >= lo_deg && slope_deg < hi_deg
}

/// Every **edge**-connected component of a triangle selection, largest first.
///
/// Edge adjacency, not vertex adjacency, for [`largest_edge_component`]'s
/// reason: two triangles meeting at a single vertex are the bowtie
/// `region_topology` refuses, so treating them as connected would hide the
/// defect this census exists to find. Deterministic — components are
/// discovered by scanning `selected` in order, and the sort is by
/// `(size desc, first triangle asc)`, so nothing depends on `HashMap`
/// iteration order.
fn edge_components(mesh: &TriangleMesh, selected: &[u32]) -> Vec<Vec<u32>> {
    let n = selected.len();
    if n == 0 {
        return Vec::new();
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
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..n {
        if component[start].is_some() {
            continue;
        }
        let id = groups.len();
        component[start] = Some(id);
        stack.push(start);
        let mut members: Vec<u32> = Vec::new();
        while let Some(i) = stack.pop() {
            let Some(&t) = selected.get(i) else { continue };
            members.push(t);
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
        members.sort_unstable();
        groups.push(members);
    }
    groups.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a.first().copied().cmp(&b.first().copied()))
    });
    groups
}

/// One component of a slope band, with the topology that decides whether
/// `plan_spiral` can even be asked about it.
struct ComponentTopology {
    triangles: Vec<u32>,
    census: MeshCensus,
}

impl ComponentTopology {
    /// A topological disk: exactly one boundary loop and `V − E + F = 1`.
    /// These are the two conditions `conformal_spiral::region_topology`
    /// enforces, so this predicate is the same question the module asks.
    fn is_disk(&self) -> bool {
        self.census.boundary_loops == 1 && self.census.euler == 1
    }

    /// Genus-0 with `h` holes has `χ = 1 − h`, so this is the hole count
    /// whenever the component is a connected surface with boundary.
    fn holes(&self) -> i64 {
        1 - self.census.euler
    }
}

/// The census the coordinator's ARM BAND asks for FIRST, before anything else.
fn band_topology(mesh: &TriangleMesh, selected: &[u32]) -> Vec<ComponentTopology> {
    edge_components(mesh, selected)
        .into_iter()
        .map(|triangles| {
            let census = mesh_census(mesh, &triangles);
            ComponentTopology { triangles, census }
        })
        .collect()
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
/// The per-triangle **quasi-conformal dilatation** rows.
///
/// `K = s_max / s_min` of the 3D→flat Jacobian's singular values. `K = 1`
/// exactly iff the map is a local similarity there — i.e. **conformal**.
///
/// # Why this is printed at all (2026-08-30)
///
/// The module has measured `dilatation_min/_median/_p90/_max` and
/// `dilatation_unmeasurable` since the mean-value change, and **nothing
/// printed them**. They are the row that decides whether substituting
/// mean-value (Floater) weights for the paper's conformal slit map costs
/// *spacing* as well as buying fold-freeness: area distortion is blind to
/// radial-vs-tangential anisotropy, and it is the anisotropy — not the area —
/// that turns a well-chosen ring radius into bad spacing. Unlike area
/// distortion (1/mm², so scale- and fixture-dependent), `K` is **scale
/// invariant**, which is what makes it comparable across the sphere, the wavy
/// patch, the ribbon, the band and the terrain probes.
///
/// Read the **p90**, not the median: one badly anisotropic sector is enough to
/// mis-size a ring, and the ring search sizes every ring by its single worst
/// sector.
fn print_dilatation_scalars(report: &SpiralReport) {
    eprintln!(
        "     QUASI-CONFORMAL DILATATION K = s_max/s_min of the 3D->flat Jacobian; K = 1 is \
         CONFORMAL"
    );
    eprintln!(
        "     K min / median / p90 / max    {:>12.4} / {:.4} / {:.4} / {:.4}",
        report.dilatation_min,
        report.dilatation_median,
        report.dilatation_p90,
        report.dilatation_max
    );
    eprintln!(
        "     K unmeasurable triangles      {:>12}   (Jacobian too degenerate for singular \
         values; 0 expected once folds are impossible)",
        report.dilatation_unmeasurable
    );
    eprintln!(
        "     READ THE p90, NOT THE MEDIAN. A ring is sized by its single WORST sector, so the\n\
         \x20    tail is what reaches the spacing. K is SCALE INVARIANT (area distortion is not),\n\
         \x20    so this row — unlike the 1/mm² rows above it — is directly comparable across\n\
         \x20    every arm in this file. K >> 1 means the map stretches radially while compressing\n\
         \x20    tangentially (or the reverse), which area distortion cannot see at all: a map can\n\
         \x20    preserve area exactly and still be arbitrarily anisotropic."
    );
}

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
    print_dilatation_scalars(report);
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
    // Written as ONE literal rather than nine `{:>N}` arguments so the header
    // and the data rows below cannot drift apart under a reformat.
    eprintln!(
        "         r_lo     r_hi  triangles   distort min   distort med   distort max    K min\
         \x20   K med    K max"
    );
    for bucket in &report.area_distortion_by_disk_radius {
        eprintln!(
            "     {:>8.3} {:>8.3} {:>10} {:>13.6e} {:>13.6e} {:>13.6e} {:>8.3} {:>8.3} {:>8.3}",
            bucket.r_lo,
            bucket.r_hi,
            bucket.triangles,
            bucket.area_distortion_min,
            bucket.area_distortion_median,
            bucket.area_distortion_max,
            bucket.dilatation_min,
            bucket.dilatation_median,
            bucket.dilatation_max
        );
    }
    eprintln!(
        "     The three K columns are the QUASI-CONFORMAL DILATATION per band (K = 1 is\n\
         \x20    conformal), bucketed the same way. They were measured by the module and printed\n\
         \x20    NOWHERE until 2026-08-30. Read them beside the area columns: area distortion is\n\
         \x20    in 1/mm² and is scale-dependent, K is dimensionless and is not, so K is what\n\
         \x20    compares across arms — and K, not area, is what mis-sizes a ring."
    );
    match radial_climb(report) {
        Some(climb) => eprintln!(
            "     climb |median(bucket 0) / median(bucket 4)|, folded to >= 1: {climb:.3}"
        ),
        None => eprintln!("     climb: NOT COMPUTABLE (an end bucket is empty or non-positive)"),
    }
}

/// The **per-ring radial-scale anisotropy** — the over-cover the worst-sector
/// rule forces on each ring.
///
/// # Why this is the decisive spacing number on a branched region
///
/// A ring is **one circle at one disk radius**, and the Eqs. 1–4 binary search
/// sizes it by its **worst sector** — the single uncovered point that is
/// hardest to reach. Every other sector of that same ring is then over-covered
/// by however much the map's local radial scale varies *around* the circle. So
/// `max/min` of that scale **is** the over-cover factor the search is forced
/// into on that ring: a ring whose radial scale varies 3× cannot be spaced
/// correctly anywhere except in its worst sector, no matter how good the
/// search is.
///
/// Neither area distortion nor `K` can see it. A map can preserve area while
/// stretching radially and compressing tangentially, and a map can have a
/// uniform `K` while its radial scale still swings around a given circle. This
/// row is not derivable from any per-triangle statistic.
///
/// **The module has measured it since the mean-value change and nothing
/// printed it.** On a convex disk-like region it should read near 1 and the
/// number is uninteresting; on a **branched** region it is the number that
/// predicts whether the method can work there at all, which is the entire
/// point of ARM RIBBON.
fn print_ring_anisotropy(label: &str, report: &SpiralReport) {
    eprintln!("\n   -- RING ANISOTROPY (per-ring radial-scale ratio) — {label} --");
    let Some(anis) = report.ring_anisotropy.as_ref() else {
        eprintln!(
            "     NOT MEASURED — `ring_anisotropy` is None, which means the ring search never\n\
             \x20    placed a ring. This is NOT a reading of 1.0 and must not be coerced to one."
        );
        return;
    };
    eprintln!(
        "     MEDIAN ratio                  {:>12.4}   <<< the typical over-cover this map \
         forces",
        anis.median_ratio
    );
    let radii = &report.ring_radii;
    let worst_radius = radii.get(anis.worst_ring).copied().unwrap_or(f64::NAN);
    eprintln!(
        "     WORST ratio                   {:>12.4}   on ring {} of {}, disk radius \
         {worst_radius:.6}",
        anis.worst_ratio,
        anis.worst_ring,
        radii.len()
    );
    eprintln!(
        "     rings measured / unmeasurable {:>12} / {}   (unmeasurable = a degenerate radius, \
         or probes outside the flattened polygon)",
        anis.rings_measured, anis.rings_unmeasurable
    );
    eprintln!(
        "     probe step (disk units)       {:>12.6}",
        anis.probe_delta_disk
    );
    eprintln!(
        "     probes SKIPPED (pulled back)  {:>12}   (excluded on purpose: the radial pullback \
         ladder moves the query",
        anis.probes_skipped_pulled_back
    );
    eprintln!(
        "\x20                                              point by a fraction comparable to the \
         probe step itself, so the\n\
         \x20                                              'scale' it would report is noise. A \
         ring whose worst sectors\n\
         \x20                                              were all skipped reads as UNDER-MEASURED, \
         not as clean.)"
    );
    let (p_min, p_med, p_max) = min_med_max(&anis.per_ring_radial_scale_ratio);
    eprintln!("     per-ring min / med / max      {p_min:>12.4} / {p_med:.4} / {p_max:.4}");
    eprintln!(
        "     READING GUIDE. 1.0 = this ring can be spaced correctly EVERYWHERE AT ONCE. N = this\n\
         \x20    ring is over-covered up to N-fold outside its own worst sector, because Eqs. 1-4\n\
         \x20    size the whole circle by the single hardest-to-reach point on it. That over-cover\n\
         \x20    is paid in CUTTING TIME on every ring, every pass, and it is the price the method\n\
         \x20    charges for stay-down continuity on a region the map cannot flatten evenly.\n\
         \x20    THIS IS NOT DERIVABLE from area distortion or from K: an area-preserving map can\n\
         \x20    still stretch radially and compress tangentially, and a uniform-K map can still\n\
         \x20    swing its radial scale around one circle."
    );
}

/// The two distortion blocks that must appear on **every** arm, in one call so
/// no arm can print one and forget the other.
///
/// Replaces the bare `print_radial_profile` at every call site. Both blocks are
/// meaningful on a refusal path too — the flattening runs before anything
/// downstream can refuse — so this is called from [`diagnose_refusal`] as well.
fn print_distortion_blocks(label: &str, report: &SpiralReport) {
    print_radial_profile(label, report);
    print_ring_anisotropy(label, report);
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
    print_distortion_blocks(label, report);

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
    // A pointer, not a second copy of the block: a reader scanning the RING
    // section should not have to know that the decisive spacing number lives
    // in the distortion block printed above the stages.
    match report.ring_anisotropy.as_ref() {
        Some(anis) => eprintln!(
            "     ring anisotropy med / worst   {:>12.4} / {:.4}   over-cover the worst-sector \
             rule forces; FULL block printed above",
            anis.median_ratio, anis.worst_ratio
        ),
        None => eprintln!(
            "     ring anisotropy               {:>12}   NOT MEASURED (no ring placed) — this is \
             NOT a reading of 1.0",
            "-"
        ),
    }

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
         \x20    0.076 mm on a mesh where 46% of triangles held ZERO samples, and the run went\n\
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
    /// The 0° ball raster over the **same** region, through the **same**
    /// production relink. Carried out of the stage (rather than being printed
    /// and discarded) because the trade block ARM RIBBON and ARM BAND owe the
    /// reader states both sides — retracts avoided AND time paid — and a
    /// verdict that quotes only the spiral's half is the mistake FINDINGS_F2
    /// §F2-2 already made once.
    raster: CandidateCost,
    /// The direction-field iso-curve candidate over the SAME region through the
    /// SAME relink, added 2026-08-30. `None` when the solve produced nothing —
    /// which on an umbilic or flat surface is the expected answer, not a gap.
    field: Option<CandidateCost>,
    /// The synthesis §4.3 candidate — an iso-scallop field whose direction is
    /// the fixed sweep direction rather than curvature. `None` on an arm with
    /// no closed form, or when the solve produced nothing.
    sweep: Option<CandidateCost>,
    /// The operator's candidate (2026-08-31) — the same iso-scallop field with
    /// its direction taken from the region's MEDIAL AXIS, i.e. contour-parallel
    /// offsets carrying the scallop spacing law. `None` on an arm with no
    /// closed form, when no medial grid could be built, or when the solve
    /// produced nothing.
    medial: Option<CandidateCost>,
    cl_points: usize,
}

// `AreaWeighted` / `area_weighted` PROMOTED to
// `rs_cam_core::metrology::floor` (Track M, 2026-09-02).

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
    run: &ArmRun<'_>,
) -> Option<StageDOutcome> {
    use rs_cam_core::geometry::region_set::RegionSet;

    let Fixture {
        mesh,
        index,
        cutter,
        kinematics,
        safe_z,
        effective_min_z,
    } = fixture;
    let kind = run.kind;

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

    // -- THE THIRD CANDIDATE (2026-08-30) --
    //
    // Costed here, between the raster and the table, so that every row of the
    // table below is produced by the same `relink_and_cost` call site on the
    // same region in the same run. On a FIXTURE-LIMITED arm it is skipped: its
    // numbers would inherit the withdrawal, and a third withdrawn row is not
    // worth the solve.
    let field = if kind == ArmKind::FixtureLimited {
        eprintln!(
            "\n   DIRECTION-FIELD CANDIDATE SKIPPED on a FIXTURE-LIMITED arm — BY DECISION, NOT\n\
             \x20  OMISSION. Every fine-geometry number on this arm is withdrawn (1.42 mm facets \
             against\n\
             \x20  a {stepover_mm:.4} mm stepover), and a third withdrawn row would only be \
             quoted out of\n\
             \x20  its banner. The field arm is measured on the ANALYTIC arms."
        );
        None
    } else {
        Some(field_candidate(
            run.label,
            fixture,
            &region.triangles,
            std::slice::from_ref(&region.polygon),
        ))
    };
    let field_cost = field.as_ref().and_then(|f| f.cost.as_ref());

    // -- THE FOURTH CANDIDATE (2026-08-30): synthesis §4.3 --
    //
    // Same region, same relink, same call site, same FieldParams as the third
    // row. It differs from that row in ONE thing — the target field V — which
    // is what makes the pair an attribution rather than two observations.
    let sweep_direction = pca_major_axis(mesh, &region.triangles);
    let sweep = run.analytic.map(|surface| {
        sweep_field_candidate(
            run.label,
            fixture,
            &region.triangles,
            std::slice::from_ref(&region.polygon),
            surface,
            &sweep_direction,
        )
    });
    if sweep.is_none() {
        eprintln!(
            "\n   SWEEP-DIRECTION CANDIDATE NOT RUN on this arm — BY DECISION, NOT OMISSION. It \
             needs\n\
             \x20  an ANALYTIC curvature for |V| = sqrt((k_s + 1/K_c)/8), and this arm has no \
             closed\n\
             \x20  form. A mesh-estimated k_s would put an estimator inside the very quantity the\n\
             \x20  experiment is attributing, which is the confound the analytic fixtures exist \
             to remove."
        );
    }
    let sweep_cost = sweep.as_ref().and_then(|f| f.cost.as_ref());

    // -- THE FIFTH CANDIDATE (2026-08-31): the operator's medial axis --
    //
    // Same region, same relink, same call site, same FieldParams as the two
    // rows above it. It differs from the SWEEP row in ONE thing — where the
    // direction comes from — which is what makes the three field rows an
    // attribution over the DIRECTION SOURCE rather than three observations.
    let medial_grid = run.analytic.and_then(|_| {
        build_medial_grid(
            run.label,
            std::slice::from_ref(&region.polygon),
            stepover_mm,
        )
    });
    let medial = match (run.analytic, medial_grid.as_ref()) {
        (Some(surface), Some(grid)) => Some(medial_field_candidate(
            run.label,
            fixture,
            &region.triangles,
            std::slice::from_ref(&region.polygon),
            surface,
            grid,
        )),
        _ => None,
    };
    if medial.is_none() {
        eprintln!(
            "\n   MEDIAL-AXIS CANDIDATE NOT RUN on this arm — BY DECISION, NOT OMISSION. It \
             needs\n\
             \x20  the same ANALYTIC curvature the sweep row does for |V| = sqrt((k_s + \
             1/K_c)/8),\n\
             \x20  and a region with a finite extent to lay a distance-transform grid over. A\n\
             \x20  mesh-estimated k_s would put an estimator inside the very quantity the \
             experiment\n\
             \x20  is attributing."
        );
    }
    let medial_cost = medial.as_ref().and_then(|f| f.cost.as_ref());

    eprintln!(
        "\n     {:<34} {:>8} {:>10} {:>8} {:>10} {:>10} {:>9}",
        "arm", "moves", "fragments", "linked", "RETRACTS", "cut mm", "time s"
    );
    let mut rows: Vec<(&str, &CandidateCost)> = vec![
        ("conformal spiral (1 polyline)", &spiral_cost),
        ("0° raster (ball, same region)", &raster_cost),
    ];
    if let Some(cost) = field_cost {
        rows.push(("direction field, D = t1 (F1)", cost));
    }
    if let Some(cost) = sweep_cost {
        rows.push(("iso-scallop field, D = sweep (§4.3)", cost));
    }
    if let Some(cost) = medial_cost {
        rows.push(("iso-scallop field, D = MEDIAL AXIS", cost));
    }
    for (label, cost) in rows {
        eprintln!(
            "     {:<34} {:>8} {:>10} {:>8} {:>10} {:>10.1} {:>9.1}",
            label,
            cost.moves,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.cutting_mm,
            cost.time_s
        );
    }
    for (name, candidate) in [
        ("direction field, D = t1 (F1)", field.as_ref()),
        ("iso-scallop field, D = sweep (§4.3)", sweep.as_ref()),
        ("iso-scallop field, D = MEDIAL AXIS", medial.as_ref()),
    ] {
        if let Some(entry) = candidate
            && entry.cost.is_none()
        {
            eprintln!(
                "     {name:<34} {:>8} {:>10} {:>8} {:>10} {:>10} {:>9}",
                "-", "-", "-", "-", "-", "-"
            );
            eprintln!("       ^ NO PATH: {}", entry.note);
        }
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
    for (name, candidate) in [
        ("D = t1", field.as_ref()),
        ("D = sweep", sweep.as_ref()),
        ("D = MEDIAL", medial.as_ref()),
    ] {
        match candidate.map(|f| f.spacings_sorted.as_slice()) {
            Some(field_spacings) if !field_spacings.is_empty() => eprintln!(
                "     {:<44} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
                format!("field {name}: MEASURED adjacent-LEVEL 3D"),
                percentile(field_spacings, 0.0),
                percentile(field_spacings, 0.50),
                percentile(field_spacings, 0.90),
                percentile(field_spacings, 1.0)
            ),
            Some(_) => eprintln!(
                "     {:<44} {:>10} {:>10} {:>10} {:>10}",
                format!("field {name}: adjacent-LEVEL 3D spacing"),
                "-",
                "NOT MEAS",
                "-",
                "-"
            ),
            None => eprintln!(
                "     {:<44} {:>10} {:>10} {:>10} {:>10}",
                format!("field {name}: NOT RUN on this arm"),
                "-",
                "-",
                "-",
                "-"
            ),
        }
    }
    eprintln!(
        "     {:<44} {:>10.5}",
        "COMMANDED XY stepover (raster only)", stepover_mm
    );
    eprintln!(
        "     ({} region triangles, {:.2} mm² of 3D area weighted)",
        cross.samples, cross.total_area_mm2
    );
    eprintln!(
        "\n     A LIKE-FOR-LIKE TIME COMPARISON IS NOT POSSIBLE WITHOUT CHANGING THE RASTER.\n\
         \x20    FIVE ROWS, THREE SPACING BASES:\n\
         \x20      * the SPIRAL spaces on the 3D SURFACE — its ring step is sized by the coverage\n\
         \x20        law directly, so its measured spacing IS the finish it delivers;\n\
         \x20      * the RASTER spaces in XY PROJECTION at the commanded stepover, so on slope\n\
         \x20        its passes land further apart along the surface than that number says and it\n\
         \x20        UNDER-COVERS exactly where the spiral covers correctly;\n\
         \x20      * ALL THREE FIELD ROWS space on their own Poisson LEVEL SET, whose increment \
         is\n\
         \x20        scheduled from |V| = sqrt((k_s + 1/r)/8) — a third basis, and one that is\n\
         \x20        neither of the other two. THEY SHARE IT, which makes that TRIPLE the one\n\
         \x20        clean spacing comparison in this table: same basis, same schedule, same\n\
         \x20        extraction, three different direction sources (curvature / one global\n\
         \x20        sweep / the region's own medial axis). Any gap among them is attributable\n\
         \x20        to the DIRECTION SOURCE and to nothing else.\n\
         \x20        READ THIS RAIL BEFORE THE MEDIAL ROW: direction alone changes WHERE the\n\
         \x20        passes go, not how far apart they are, so the medial row should land at\n\
         \x20        roughly the SWEEP row's cutting distance with far fewer fragments. If its\n\
         \x20        cut mm moves a LOT instead, something other than the direction moved — the\n\
         \x20        SPACING BASIS did — and that must be said out loud rather than banked as a\n\
         \x20        direction win.\n\
         \x20    The raster row would have to be re-run at an XY stepover scaled by cos(local\n\
         \x20    slope) — a variable-stepover raster this file does not have and may not add,\n\
         \x20    because `raster_candidate` is restated verbatim from F1 and changing it here\n\
         \x20    would break the cross-instrument comparability that restatement buys. Nothing\n\
         \x20    equivalent exists for the field rows either: their spacing is an OUTPUT of the\n\
         \x20    level schedule, not a dial that can be re-commanded to match.\n\
         \x20    So the ratios below are RAW OBSERVATIONS, NOT VERDICTS. Compare the spacing\n\
         \x20    rows FIRST; a row that spaces tighter is buying finish with its time, and a row\n\
         \x20    that spaces wider is selling it. THE x FLOOR BLOCK BELOW IS THE ONE COLUMN THAT\n\
         \x20    PUTS ALL FOUR ON A SINGLE ABSOLUTE SCALE."
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
    // -- THE FLOOR (synthesis §1) — the one absolute scale in this stage --
    match run.analytic {
        Some(surface) => {
            let floor = region_floor(mesh, &region.triangles, surface);
            print_floor_block(
                run.label,
                surface,
                &floor,
                &[
                    ("conformal spiral", Some(spiral_cost.cutting_mm)),
                    ("0deg ball raster", Some(raster_cost.cutting_mm)),
                    (
                        "direction field, D = t1 (F1)",
                        field_cost.map(|c| c.cutting_mm),
                    ),
                    (
                        "iso-scallop field, D = sweep (4.3)",
                        sweep_cost.map(|c| c.cutting_mm),
                    ),
                    (
                        "iso-scallop field, D = MEDIAL AXIS",
                        medial_cost.map(|c| c.cutting_mm),
                    ),
                ],
            );
        }
        None => eprintln!(
            "\n   ===== x FLOOR — NOT MEASURABLE ON THIS ARM =====\n\
             \x20  L_min = INTEGRAL dA / s_max(x) needs s_max from a curvature that is not a mesh\n\
             \x20  estimate, and this arm's fixture has no closed form. Estimating it from the\n\
             \x20  facets would make the floor a statement about the mesh — which on a\n\
             \x20  FIXTURE-LIMITED arm is precisely the failure the FINDINGS_F2 withdrawal is\n\
             \x20  about (a 1.42 mm facet radius read as a 1.94 mm surface radius). NOT MEASURED\n\
             \x20  is the honest answer; the floor is measured on the four ANALYTIC arms."
        ),
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

    // -- THE OPERATOR'S PICTURE (2026-08-30) --
    //
    // Emitted LAST, from the three costed paths above, so the panel labels and
    // the table are the same numbers by construction rather than by care.
    if kind == ArmKind::FixtureLimited {
        eprintln!(
            "   COMPARISON SVGs SKIPPED on a FIXTURE-LIMITED arm: their panel labels carry \
             measured\n\
             \x20  numbers, and a figure travels further from its banner than a table does."
        );
    } else {
        let field_note = field
            .as_ref()
            .map_or_else(|| "not run on this arm".to_owned(), |f| f.note.clone());
        let sweep_note = sweep.as_ref().map_or_else(
            || "not run — this arm has no analytic curvature".to_owned(),
            |f| f.note.clone(),
        );
        let medial_note = medial.as_ref().map_or_else(
            || "not run — this arm has no analytic curvature".to_owned(),
            |f| f.note.clone(),
        );
        let (compare_path, overlay_path) = write_comparison_svgs(
            run.slug,
            run.label,
            std::slice::from_ref(&region.polygon),
            &[
                ComparePanel {
                    title: "conformal spiral",
                    colour: SPIRAL_COLOUR,
                    cost: Some(&spiral_cost),
                    note: "",
                },
                ComparePanel {
                    title: "0deg ball raster",
                    colour: RASTER_COLOUR,
                    cost: Some(&raster_cost),
                    note: "",
                },
                ComparePanel {
                    title: "direction field, D = t1 (F1)",
                    colour: FIELD_COLOUR,
                    cost: field_cost,
                    note: &field_note,
                },
                ComparePanel {
                    title: "iso-scallop field, D = sweep (4.3)",
                    colour: SWEEP_COLOUR,
                    cost: sweep_cost,
                    note: &sweep_note,
                },
                ComparePanel {
                    title: "iso-scallop field, D = MEDIAL AXIS",
                    colour: MEDIAL_COLOUR,
                    cost: medial_cost,
                    note: &medial_note,
                },
            ],
        );
        eprintln!("\n   -- THE COMPARISON FIGURES (the operator's request, 2026-08-30) --");
        eprintln!("     panels : {}", compare_path.display());
        eprintln!("     overlay: {}", overlay_path.display());
        eprintln!(
            "     Five panels, identical scale and identical viewBox size, each labelled with \
             its OWN\n\
             \x20    row from the table above. Cutting moves solid in the candidate's colour, \
             SURFACE\n\
             \x20    LINKS green, AIR red dashed, LIFT POINTS as red rings — so the trade this \
             phase is\n\
             \x20    about is visible as shape rather than only as a number. Drawn from the \
             COSTED path."
        );
    }

    Some(StageDOutcome {
        cost: spiral_cost,
        raster: raster_cost,
        field: field.and_then(|f| f.cost),
        sweep: sweep.and_then(|f| f.cost),
        medial: medial.and_then(|f| f.cost),
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
    /// Filename stem for this arm's SVGs — `_disk_f2` / `_xy_f2` from Stage B,
    /// and `_compare_f2` / `_overlay_f2` from Stage D. The terrain FLAT arm keeps its
    /// historical stem so the artifacts FINDINGS_F2 §F2-1 names by filename
    /// stay findable even though their numbers are withdrawn.
    slug: &'a str,
    kind: ArmKind,
    /// The constant-scallop spacing this arm's surface implies in closed
    /// form. `Some` only on the ANALYTIC arms — terrain's would be computed
    /// from a curvature census that is itself a faceting artefact, which is
    /// exactly the circularity the withdrawal is about.
    analytic_target_mm: Option<f64>,
    /// The arm's fixture as a **closed form**, when it has one.
    ///
    /// `Some` unlocks the two things added on 2026-08-30 — the synthesis §4.3
    /// sweep-direction candidate and the §1 `× floor` block — because both need
    /// a curvature that is not a mesh estimate. `None` on the terrain arms, and
    /// the floor block then prints NOT MEASURABLE with the reason rather than
    /// silently omitting itself.
    analytic: Option<AnalyticSurface>,
    /// Whether Stage E's sampling sensitivity re-runs here.
    run_stage_e: bool,
}

/// Returns Stage D's outcome — spiral cost, raster cost, CL point count — so
/// an arm can print a trade verdict over BOTH sides. `None` means Stage D did
/// not run (the Stage A falsifier stopped the run) or refused.
fn run_arm_evidence(
    run: &ArmRun<'_>,
    fixture: Fixture<'_>,
    region: &Region,
    report: &SpiralReport,
    result: &SpiralResult,
    params: &SpiralParams,
    stepover_mm: f64,
) -> Option<StageDOutcome> {
    eprintln!("\n══════════ STAGED EVIDENCE — {} ══════════\n", run.label);
    if run.kind == ArmKind::FixtureLimited {
        eprintln!("   {WITHDRAWAL_BANNER}\n");
    }

    let region_area_3d = region_area_mm2(fixture.mesh, &region.triangles);
    let proceed = stage_a(run.label, report, params, stepover_mm, region_area_3d);
    stage_b(region, result, report, run.slug);

    let mut stage_d_outcome: Option<StageDOutcome> = None;
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

        stage_d_outcome = stage_d(fixture, region, result, report, stepover_mm, samples, run);
        if let Some(outcome) = stage_d_outcome.as_ref() {
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
    stage_d_outcome
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
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
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
    print_distortion_blocks(label, &report);
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
            analytic: Some(SPHERE_SURFACE),
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
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
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
    print_distortion_blocks(label, &report);
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
            analytic: Some(WAVY_SURFACE),
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

/// Plan one arm on an explicit triangle selection that **may legitimately need
/// selection hygiene**.
///
/// [`plan_analytic_arm`] asserts the cleanup is a no-op, which is right for the
/// sphere and the wavy patch: those regions are a whole mesh and a polyomino
/// disk, so any hygiene at all would mean the generator was wrong. ARM RIBBON
/// and ARM BAND are different — a branched polyomino and a slope-banded one can
/// legitimately leave a corner-touching cell — so here the cleanup is
/// **reported** rather than asserted away, and whatever survives is put to
/// `plan_spiral` to accept or refuse on its own terms.
fn plan_selected_region(
    label: &'static str,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    triangles: &[u32],
    polygon: Polygon2,
    params: &SpiralParams,
) -> Arm {
    let (cleaned, cleanup) = clean_selection(mesh, triangles);
    let dropped = cleanup.before.saturating_sub(cleanup.after);
    let dropped_pct = 100.0 * dropped as f64 / cleanup.before.max(1) as f64;
    eprintln!(
        "   selection hygiene: {} -> {} triangles ({} dropped, {:.3}%); {} component(s) on the \
         first pass,\n\
         \x20  {} dropped; {} pinch pass(es){}, {} pinch vertex/vertices shaved, {} over-used \
         edge(s), {} triangle(s) shaved.\n\
         \x20  A NON-zero drop here is REPORTED, not asserted away: a branched or slope-banded\n\
         \x20  polyomino can legitimately leave a corner-touching cell, and what survives is put\n\
         \x20  to plan_spiral to accept or refuse on its own terms.",
        cleanup.before,
        cleanup.after,
        dropped,
        dropped_pct,
        cleanup.components_first_pass,
        cleanup.components_dropped,
        cleanup.pinch_iterations,
        if cleanup.hit_iteration_cap {
            " (HIT THE CAP — still non-manifold when the loop stopped)"
        } else {
            ""
        },
        cleanup.pinch_vertices_shaved,
        cleanup.over_used_edges,
        cleanup.triangles_shaved_by_pinch
    );

    let [x0, y0, x1, y1] = polygon.bbox();
    let (report, outcome) = conformal_spiral::plan_spiral(mesh, index, &cleaned, params);
    Arm {
        label,
        region: Region {
            polygon,
            triangles: cleaned,
            semi_axes: (0.5 * (x1 - x0), 0.5 * (y1 - y0)),
            shrinks: 0,
            cleanup,
        },
        report,
        outcome,
    }
}

/// Cells whose triangles appear in `triangles`, as a `cells × cells` mask.
///
/// Exact for a whole-cell selection (both triangles of a cell present); after
/// [`clean_selection`] has shaved a corner a cell can survive with only one
/// triangle, and this marks it anyway — so the derived polygon is at worst one
/// cell generous, never tight. Stated because a generous containment polygon
/// makes the Stage D escape count read LOW, and a reader must know which way
/// the approximation leans.
fn triangles_to_cell_mask(triangles: &[u32], cells: usize) -> Vec<bool> {
    let mut mask = vec![false; cells * cells];
    for &t in triangles {
        let cell = (t as usize) / 2;
        if cell < mask.len() {
            mask[cell] = true;
        }
    }
    mask
}

/// The trade block both new arms owe the reader.
///
/// The question is **"does eliminating N retracts beat paying an M-fold
/// over-cover?"**, and it has two sides. FINDINGS_F2 §F2-2 printed one of them
/// (time) on fixtures where the other (retracts) was structurally zero, and the
/// resulting "5–15 % slower" reads as a verdict on the method when it is a
/// verdict on the fixture. So this block prints BOTH sides and **declares no
/// winner when the numbers are mixed**.
fn print_trade_verdict(
    label: &str,
    outcome: Option<&StageDOutcome>,
    report: &SpiralReport,
    census: &ScanlineCensus,
    gate_passed: bool,
) {
    eprintln!("\n══════════ TRADE VERDICT — {label} ══════════\n");
    eprintln!("   {FRESH_STOCK_LABEL}\n");
    if !gate_passed {
        eprintln!(
            "   *** THE FRAGMENTATION GATE FAILED ON THIS FIXTURE. Everything below is\n\
             \x20  ARITHMETIC, NOT EVIDENCE — the raster had almost nothing to lift out of, so\n\
             \x20  the stay-down claim had nothing to beat, which is exactly the SPHERE/WAVY\n\
             \x20  mistake FINDINGS_F2 §F2-2 recorded. Do not quote it. ***\n"
        );
    }
    let Some(outcome) = outcome else {
        eprintln!(
            "   NO COST PAIR — Stage D did not run (the Stage A falsifier stopped the run, or the\n\
             \x20  spiral refused). There is no time side to the trade, so no trade verdict is\n\
             \x20  possible. The raster side stands on its own and is printed above; the\n\
             \x20  fragmentation census ({} inside-runs over {} active rows) is what the spiral\n\
             \x20  WOULD have had to beat.",
            census.total_runs, census.rows_with_material
        );
        return;
    };

    let spiral = &outcome.cost;
    let raster = &outcome.raster;
    eprintln!(
        "   {:<32} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "arm", "fragments", "linked", "RETRACTS", "cut mm", "time s"
    );
    eprintln!(
        "   {:<32} {:>10} {:>10} {:>10} {:>10.1} {:>10.1}",
        "conformal spiral",
        spiral.fragments,
        spiral.linked,
        spiral.kept_retracts,
        spiral.cutting_mm,
        spiral.time_s
    );
    eprintln!(
        "   {:<32} {:>10} {:>10} {:>10} {:>10.1} {:>10.1}",
        "0deg ball raster, same region",
        raster.fragments,
        raster.linked,
        raster.kept_retracts,
        raster.cutting_mm,
        raster.time_s
    );
    // The three field rows are CONTEXT, not sides of the trade. SIDE 1/2/3 below
    // are registered for spiral-vs-raster and are deliberately left alone: this
    // arm's question is whether eliminating N retracts beats an M-fold
    // over-cover, and a path with its own spacing basis does not answer it.
    // They are printed here because the operator asked to SEE all of them, and
    // a figure with five panels beside a table with two rows invites the
    // reader to assume the missing rows were hidden.
    for (name, row) in [
        ("direction field, D = t1 (F1)", outcome.field.as_ref()),
        ("iso-scallop field, D = sweep", outcome.sweep.as_ref()),
        ("iso-scallop field, D = medial", outcome.medial.as_ref()),
    ] {
        match row {
            Some(field) => eprintln!(
                "   {name:<32} {:>10} {:>10} {:>10} {:>10.1} {:>10.1}   <<< CONTEXT ROW",
                field.fragments, field.linked, field.kept_retracts, field.cutting_mm, field.time_s
            ),
            None => eprintln!(
                "   {name:<32} {:>10} {:>10} {:>10} {:>10} {:>10}   <<< NO PATH (see the solve \
                 report)",
                "-", "-", "-", "-", "-"
            ),
        }
    }
    eprintln!(
        "   {:<32} {:>10} {:>10} {:>10}",
        "Wanaka region 1, 0deg (reference)",
        WANAKA_R1_RASTER_FRAGMENTS,
        "-",
        WANAKA_R1_RASTER_RETRACTS
    );
    eprintln!(
        "   {:<32} {:>10} {:>10} {:>10}",
        "Wanaka region 1, PCA cells (the bar)",
        WANAKA_R1_PCA_FRAGMENTS,
        "-",
        WANAKA_R1_PCA_RETRACTS
    );

    let retracts_avoided = raster.kept_retracts as i64 - spiral.kept_retracts as i64;
    let time_delta = spiral.time_s - raster.time_s;
    let time_ratio = if raster.time_s > 0.0 {
        spiral.time_s / raster.time_s
    } else {
        f64::NAN
    };
    eprintln!(
        "\n   SIDE 1 — RETRACTS THE SPIRAL AVOIDS   {retracts_avoided:>10}   (raster {} minus \
         spiral {})",
        raster.kept_retracts, spiral.kept_retracts
    );
    eprintln!(
        "   SIDE 2 — TIME THE SPIRAL PAYS         {time_delta:>10.1} s  ({time_ratio:.3}x the \
         raster; > 1 means the spiral is SLOWER)"
    );
    match report.ring_anisotropy.as_ref() {
        Some(anis) => eprintln!(
            "   SIDE 3 — OVER-COVER IT PAYS FOR IT    {:>10.3}x  median per-ring radial-scale \
             ratio (worst {:.3}x)",
            anis.median_ratio, anis.worst_ratio
        ),
        None => eprintln!(
            "   SIDE 3 — OVER-COVER IT PAYS FOR IT    {:>10}   NOT MEASURED (no ring placed) — \
             not a reading of 1.0",
            "-"
        ),
    }
    eprintln!(
        "\n   TOTAL TIME INCLUDING RETRACTS is the operator's metric, not cut distance: the F-034\n\
         \x20  column above already integrates rapids, retract descents and accel through the\n\
         \x20  pinned Shapeoko envelope, so `time s` IS the comparison. Cut mm is printed only so\n\
         \x20  a reader can see WHERE the time went."
    );

    let spiral_wins = time_delta < 0.0;
    let avoids_retracts = retracts_avoided > 0;
    if spiral_wins && avoids_retracts {
        eprintln!(
            "\n   VERDICT: the spiral wins BOTH sides on this fixture — fewer retracts AND less\n\
             \x20  total time. That is the first fixture in this instrument where it does; say so\n\
             \x20  with the over-cover figure attached, because the over-cover is what it will\n\
             \x20  cost on a region the map flattens worse."
        );
    } else if !spiral_wins && !avoids_retracts {
        eprintln!(
            "\n   VERDICT: the spiral loses BOTH sides — it is slower and it did not remove\n\
             \x20  retracts the raster was paying. On this fixture the method has no case."
        );
    } else {
        let retract_side = if avoids_retracts {
            format!("REMOVES {retracts_avoided}")
        } else {
            format!("ADDS {}", -retracts_avoided)
        };
        let time_side = if spiral_wins { "WINS" } else { "LOSES" };
        eprintln!(
            "\n   VERDICT: MIXED — NO WINNER IS DECLARED, deliberately.\n\
             \x20  The spiral {retract_side} retracts and {time_side} on time. Those are different\n\
             \x20  currencies and this instrument will not convert one into the other by picking a\n\
             \x20  story: the conversion rate is the operator's, and it depends on their stock\n\
             \x20  state and their retract height, neither of which is measured here\n\
             \x20  (link_ceiling: None). Report both columns."
        );
    }
}

// ── ARM RIBBON — narrow, branched, simply connected ─────────────────────
//
// The arm that fixes the benchmarking error in FINDINGS_F2 §F2-2: a
// retract-elimination method was measured on two fixtures whose raster had
// ZERO retracts. This one is deliberately the geometry class a raster hates —
// narrow arms a scan line keeps leaving and re-entering — while staying
// SIMPLY CONNECTED, so it isolates the DISTORTION question from the topology
// question. The topology question is ARM BAND's.

/// Arms radiating from the centre. Eight, at a 22.5° phase, so **no arm is
/// horizontal or near-horizontal**.
///
/// That is not cosmetic. The 0° raster scans in `x` and advances in `y`, so an
/// arm lying nearly along `x` produces one enormously wide inside-run that
/// merges with its neighbours and the region stops fragmenting. At eight arms
/// on a 45° pitch with a 22.5° phase every arm direction has
/// `|sin θ| ≥ sin 22.5° = 0.383`, so every arm's inside-run stays narrow
/// enough to separate. A five-arm rose at an 18° phase was derived (by hand,
/// on paper) to break only ~30 % of its scan rows; this one **measures 76 %**
/// (32 of 42 active rows, 92 inside-runs, worst row 4).
const RIBBON_ARMS: usize = 8;

/// Phase of the first arm (rad) — see [`RIBBON_ARMS`].
const RIBBON_ARM_PHASE_RAD: f64 = PI / 8.0;

/// Skeleton length of one arm (mm), from the centre.
const RIBBON_ARM_LENGTH_MM: f64 = 10.0;

/// Half-width of the ribbon (mm): the arms are the points within this distance
/// of the skeleton, so an arm is **2.5 mm wide**.
///
/// The brief asks for an arm width of 4–8 × the stepover. At the 0.48620 mm
/// equal-cusp stepover that band is 1.945–3.890 mm, and 2.5 mm is **5.14 ×**
/// the stepover — near the middle. It is also the width class the real target
/// carries: Wanaka thin-organic region 1's dominant tier-1 component measures
/// **3.84 mm wide = 7.9 stepovers**
/// (`planning/thin_organic_2026-08-27/FINDINGS.md` §1.1).
const RIBBON_HALF_WIDTH_MM: f64 = 1.25;

/// Side of the square patch (mm). `2 × (arm + half-width) = 22.5`, so 26 mm
/// leaves ~1.75 mm of mesh outside the ribbon on every side — enough that the
/// drop-cutter CL conversion never runs off the patch edge, the same margin
/// discipline [`WAVY_REGION_RADIUS_MM`] uses.
const RIBBON_PATCH_MM: f64 = 26.0;

/// Paraboloid "valley floor" amplitude (mm) at [`RIBBON_BOWL_RADIUS_MM`].
const RIBBON_BOWL_AMPLITUDE_MM: f64 = 1.0;

/// Paraboloid scale radius (mm).
const RIBBON_BOWL_RADIUS_MM: f64 = 15.0;

/// Ripple amplitude (mm) laid over the valley.
const RIBBON_RIPPLE_AMPLITUDE_MM: f64 = 0.2;

/// Ripple wavelength (mm).
const RIBBON_RIPPLE_WAVELENGTH_MM: f64 = 6.0;

/// Ripple wavenumber `k = 2π/L`.
fn ribbon_ripple_k() -> f64 {
    TAU / RIBBON_RIPPLE_WAVELENGTH_MM
}

/// The ribbon fixture's heightfield: a shallow paraboloid valley with a gentle
/// ripple over it.
///
/// ```text
/// z(x, y) = A_r·(x² + y²)/R₀²  +  A_w·sin(k·x)·sin(k·y)
/// ```
///
/// A **valley floor, not a mountain** — this arm tests BRANCHING, and the steep
/// question is already answered by ARM STEEP.
fn ribbon_height(x: f64, y: f64) -> f64 {
    let k = ribbon_ripple_k();
    RIBBON_BOWL_AMPLITUDE_MM * (x * x + y * y) / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM)
        + RIBBON_RIPPLE_AMPLITUDE_MM * (k * x).sin() * (k * y).sin()
}

/// `|∇z|max` over the WHOLE patch, by the triangle inequality on the two terms.
///
/// # The arithmetic, stated as [`wavy_patch_mesh`] states its own
///
/// The paraboloid's gradient is `2·A_r·r/R₀²`, largest at the patch's furthest
/// corner `r_max = (size/2)·√2`. The ripple's is bounded by `A_w·k` exactly:
/// with `u = sin²(kx)`, `v = sin²(ky)`,
/// `|∇(A_w sin kx sin ky)|² = (A_w k)²·[u + v − 2uv]`, and `u + v − 2uv` is
/// linear in each variable so its maximum over `[0,1]²` is at a corner and
/// equals **1**. Summing the two bounds,
///
/// ```text
/// |∇z|max ≤ 2·A_r·r_max/R₀² + A_w·k
///         = 2·1.0·18.385/225 + 0.2·1.04720
///         = 0.16342 + 0.20944 = 0.37286   ⇒  MAX SLOPE 20.45°
/// ```
///
/// — well under the ~30° the brief asks for.
fn ribbon_max_gradient() -> f64 {
    let r_max = 0.5 * RIBBON_PATCH_MM * SQRT_2;
    2.0 * RIBBON_BOWL_AMPLITUDE_MM * r_max / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM)
        + RIBBON_RIPPLE_AMPLITUDE_MM * ribbon_ripple_k()
}

/// `|κ|max` over the patch — the bound that says the ball FITS EVERY CONCAVITY.
///
/// The Hessian is the sum of the paraboloid's `2·A_r/R₀²·I` and the ripple's,
/// whose largest eigenvalue magnitude at a critical point is `A_w·k²`. A
/// heightfield's normal curvature is at most the Hessian's spectral norm, so
///
/// ```text
/// |κ|max ≤ 2·A_r/R₀² + A_w·k² = 0.00889 + 0.21932 = 0.22821 /mm
///   ⇒  R_min = 4.382 mm  against  K_c = 1.0 mm
/// ```
///
/// The smoke test asserts `R_min ≥ 2·K_c`, so a future parameter tweak that
/// would make the fixture unmachinable fails loudly instead of arriving as a
/// mysterious `RingSearchStalled`.
fn ribbon_max_curvature() -> f64 {
    let k = ribbon_ripple_k();
    2.0 * RIBBON_BOWL_AMPLITUDE_MM / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM)
        + RIBBON_RIPPLE_AMPLITUDE_MM * k * k
}

/// Distance from `(x, y)` to the ribbon's **skeleton** — [`RIBBON_ARMS`]
/// segments sharing the origin as one endpoint.
///
/// The region is the sublevel set `distance ≤ RIBBON_HALF_WIDTH_MM`, i.e. a
/// union of capsules. Every capsule is convex and every one contains the
/// origin, so the union is **star-shaped about the origin** and therefore
/// SIMPLY CONNECTED with exactly one boundary loop — no hole for a slit map to
/// be needed for. That is the whole design constraint of this arm.
fn ribbon_skeleton_distance(x: f64, y: f64) -> f64 {
    let mut best = f64::INFINITY;
    for j in 0..RIBBON_ARMS {
        let theta = RIBBON_ARM_PHASE_RAD + TAU * (j as f64) / (RIBBON_ARMS as f64);
        let (ux, uy) = (theta.cos(), theta.sin());
        let t = (x * ux + y * uy).clamp(0.0, RIBBON_ARM_LENGTH_MM);
        let (dx, dy) = (x - t * ux, y - t * uy);
        best = best.min(dx.hypot(dy));
    }
    best
}

/// Whole grid cells whose CENTRE lies inside the capsule union.
fn ribbon_cell_mask(cells: usize) -> Vec<bool> {
    let mut mask = vec![false; cells * cells];
    for row in 0..cells {
        for col in 0..cells {
            let (x, y) = cell_centre(RIBBON_PATCH_MM, cells, row, col);
            mask[row * cells + col] = ribbon_skeleton_distance(x, y) <= RIBBON_HALF_WIDTH_MM;
        }
    }
    mask
}

fn arm_ribbon(
    cutter: &BallEndmill,
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
    stepover_mm: f64,
    params: &SpiralParams,
) {
    eprintln!(
        "\n══════════ ARM RIBBON — narrow BRANCHED ribbon (the honest comparator) ══════════\n\
         \x20  WHY THIS ARM EXISTS. FINDINGS_F2 §F2-2 measured the spiral 5-15% SLOWER than a\n\
         \x20  0deg raster on the sphere cap and the wavy patch — and on those fixtures the\n\
         \x20  raster produced 4 and 3 fragments with ZERO RETRACTS. The spiral's entire value\n\
         \x20  proposition is one continuous stay-down path with no lifts, and it had NOTHING TO\n\
         \x20  BEAT. On the real target (Wanaka thin-organic region 1) a 0deg raster produces\n\
         \x20  {WANAKA_R1_RASTER_FRAGMENTS} fragments / {WANAKA_R1_RASTER_RETRACTS} kept retracts, \
         and even the tuned PCA-cell plan carries\n\
         \x20  {WANAKA_R1_PCA_FRAGMENTS} / {WANAKA_R1_PCA_RETRACTS}. This arm reproduces that CLASS \
         while staying SIMPLY CONNECTED, so it\n\
         \x20  isolates the DISTORTION question from the TOPOLOGY question. Topology is ARM BAND.\n\
         \x20  A refusal here is a HARD FAILURE unless it is diagnosable — a diagnosed refusal on\n\
         \x20  a branched region IS the finding, and the diagnosis is printed.\n\
         \x20\n\
         \x20  MEASURED ON THE WAY (G-CELLHOLE, 2026-08-30) — a result in its own right:\n\
         \x20  THIS REGION IS SIMPLY CONNECTED IN THE CONTINUUM AND WAS NOT AFTER DIGITISATION.\n\
         \x20  The arms are capsules that all contain the origin, so their union is star-shaped\n\
         \x20  about it — one boundary loop, no hole, provable by inspection. Laid onto a\n\
         \x20  0.1057 mm cell grid it came back with EULER -3 AND FOUR HOLES: one cell each, at\n\
         \x20  r = 3.363 mm on the bisectors at 45/135/225/315 deg — the four inter-arm wedges\n\
         \x20  that run DIAGONAL to the axis-aligned lattice. Total 0.0447 mm², 0.024% of the\n\
         \x20  region: topologically fatal, metrically nothing. The full mechanism and the\n\
         \x20  shipped-extractor comparison are in the G-CELLHOLE block below.\n\
         \x20  Why it belongs in the programme's evidence and not just in a commit message: the\n\
         \x20  operator's real regions are SLOPE-BANDED, and banding creates holes of its own.\n\
         \x20  This says a branched region acquires holes from the GRID ON TOP OF those, so a\n\
         \x20  hole count read off a mask is an UPPER BOUND on the surface's topology, never a\n\
         \x20  reading of it — and ARM BAND's census must be read with that in mind.\n"
    );

    let max_gradient = ribbon_max_gradient();
    let max_slope_deg = max_gradient.atan().to_degrees();
    let max_curvature = ribbon_max_curvature();
    let cells = heightfield_cells(RIBBON_PATCH_MM, max_gradient, ANALYTIC_MAX_EDGE_MM);
    let mesh = heightfield_mesh(RIBBON_PATCH_MM, cells, ribbon_height);
    let index = SpatialIndex::build_auto(&mesh);
    let raw_mask = ribbon_cell_mask(cells);
    let (mask, fill) = fill_mask_holes(&raw_mask, RIBBON_PATCH_MM, cells);
    let triangles = cells_to_triangles(&mask, cells);
    let polygons = mask_polygons(&mask, RIBBON_PATCH_MM, cells);

    eprintln!(
        "   fixture: {RIBBON_ARMS} arms of length {RIBBON_ARM_LENGTH_MM} mm at a \
         {:.1} deg phase, half-width {RIBBON_HALF_WIDTH_MM} mm\n\
         \x20           => ARM WIDTH {:.3} mm = {:.2} x the {stepover_mm:.5} mm stepover (brief \
         asks 4-8x;\n\
         \x20           Wanaka region 1's dominant component is 3.84 mm = 7.9 stepovers).\n\
         \x20  heightfield z = A_r*(x^2+y^2)/R0^2 + A_w*sin(k x)*sin(k y) on a {cells} x {cells} \
         grid,\n\
         \x20           A_r {RIBBON_BOWL_AMPLITUDE_MM} mm at R0 {RIBBON_BOWL_RADIUS_MM} mm, \
         A_w {RIBBON_RIPPLE_AMPLITUDE_MM} mm at L {RIBBON_RIPPLE_WAVELENGTH_MM} mm, cell {:.5} mm.\n\
         \x20  |grad z|max = 2*A_r*r_max/R0^2 + A_w*k = {max_gradient:.5}  =>  MAX SLOPE \
         {max_slope_deg:.2} deg (bar: well under 30).\n\
         \x20  |kappa|max  = 2*A_r/R0^2 + A_w*k^2   = {max_curvature:.5} 1/mm  =>  R_min {:.3} mm \
         against K_c {BALL_RADIUS_MM} mm,\n\
         \x20           so the ball fits every concavity and no S^h point is unreachable at any \
         spacing.\n\
         \x20  A VALLEY FLOOR, NOT A MOUNTAIN: relief over the machined ribbon is ~{:.2} mm. This \
         arm tests\n\
         \x20  BRANCHING; the steep question is ARM STEEP's.",
        RIBBON_ARM_PHASE_RAD.to_degrees(),
        2.0 * RIBBON_HALF_WIDTH_MM,
        2.0 * RIBBON_HALF_WIDTH_MM / stepover_mm,
        RIBBON_PATCH_MM / cells as f64,
        1.0 / max_curvature,
        RIBBON_BOWL_AMPLITUDE_MM * (RIBBON_ARM_LENGTH_MM + RIBBON_HALF_WIDTH_MM).powi(2)
            / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM)
            + 2.0 * RIBBON_RIPPLE_AMPLITUDE_MM
    );

    // G-CELLHOLE. Printed BEFORE the census, because the census is taken on
    // the repaired mask and a reader must know what was repaired first.
    print_mask_fill("ARM RIBBON", &fill, RIBBON_PATCH_MM / cells as f64);

    let census = mesh_census(&mesh, &triangles);
    print_mesh_census("ARM RIBBON", &census, stepover_mm);
    eprintln!(
        "   region polygons extracted from the SAME (repaired) cell mask that selected the \
         triangles:\n\
         \x20  {} polygon(s), {} hole(s) in total. ONE polygon with ZERO holes is the design claim \
         — the\n\
         \x20  arms are capsules sharing the origin, every capsule is convex and contains it, so \
         the\n\
         \x20  union is STAR-SHAPED about the origin and therefore simply connected. That argument \
         is\n\
         \x20  about the CONTINUUM and it survived; what did not was the digitisation, and the\n\
         \x20  G-CELLHOLE block above says exactly what it cost and what was filled. If THIS line\n\
         \x20  still prints holes, the repair did not reach them and everything below inherits it.",
        polygons.len(),
        polygons.iter().map(|p| p.holes.len()).sum::<usize>()
    );

    // ── THE VALIDITY GATE — before the cost table, by design ────────────
    let scan = scanline_census(&polygons, stepover_mm);
    let gate_passed = print_fragmentation_gate("ARM RIBBON", &scan, stepover_mm);

    let Some(polygon) = polygons.first().cloned() else {
        panic!(
            "ARM RIBBON: the cell mask produced no polygon at all. The generator, not the \
             algorithm, is broken."
        )
    };

    // The target: the ring search sizes every ring by its single WORST
    // uncovered point, and CONVEX curvature is what narrows the admissible
    // stepover, so the peak-convex figure is the binding one — the same
    // reasoning ARM WAVY prints. On this surface the peak convex curvature is
    // the ripple's crest minus the valley's own (concave) contribution.
    let peak_convex = RIBBON_RIPPLE_AMPLITUDE_MM * ribbon_ripple_k() * ribbon_ripple_k()
        - 2.0 * RIBBON_BOWL_AMPLITUDE_MM / (RIBBON_BOWL_RADIUS_MM * RIBBON_BOWL_RADIUS_MM);
    let convex_target =
        scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, peak_convex);
    eprintln!("\n   -- THE ANALYTIC ENVELOPE (this surface has no single curvature) --");
    eprintln!("     peak-CONVEX curvature (ripple crest minus valley) {peak_convex:>9.5} 1/mm");
    eprintln!(
        "     peak-CONVEX target (the binding one)              {convex_target:>9.5} mm   <<< the \
         verdict is measured against this"
    );
    eprintln!(
        "     FLAT target, for scale                            {stepover_mm:>9.5} mm\n\
         \x20    NOTE this is an ENVELOPE BOUND over the whole patch: whether the machined ribbon\n\
         \x20    actually contains a ripple crest is a fact about where the arms landed, not an\n\
         \x20    assumption. Read the measured distribution in Stage C against BOTH ends.\n"
    );

    let arm = plan_selected_region("ARM RIBBON", &mesh, &index, &triangles, polygon, params);
    let Arm {
        label,
        region,
        report,
        outcome,
    } = arm;

    eprintln!();
    print_distortion_blocks(label, &report);

    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter,
        kinematics,
        safe_z: mesh.bbox.max.z + 5.0,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };

    let outcome = match outcome {
        Ok(result) => Some(result),
        Err(refusal) => {
            let diagnosable = diagnose_refusal(label, &report, &refusal, params);
            assert!(
                diagnosable,
                "{label} refused with {refusal:?} and the surviving report carried nothing to \
                 attribute it by. A DIAGNOSED refusal on a branched region is itself the finding \
                 and this arm reports it as one; an UNDIAGNOSABLE refusal is the hard failure."
            );
            eprintln!(
                "\n   ARM RIBBON REFUSED, AND THE REFUSAL IS THE FINDING.\n\
                 \x20  The region is a simply-connected, manifold, sub-{ANALYTIC_MAX_EDGE_MM} mm \
                 polyomino disk\n\
                 \x20  with slope under {max_slope_deg:.1} deg and every concavity larger than the \
                 ball, so no\n\
                 \x20  fixture defect can be blamed and no topology repair (F2 step 3) can help: \
                 there is\n\
                 \x20  NO HOLE HERE. What is left is DISTORTION — read the ring-anisotropy and K \
                 rows\n\
                 \x20  above. If the refusal is FlattenDidNotConverge that is a SOLVER budget \
                 instead\n\
                 \x20  (raise ANALYTIC_SOLVER_MAX_SWEEPS, currently {}); coarsening the mesh is \
                 NOT the fix.\n",
                params.solver_max_sweeps
            );
            None
        }
    };

    let stage_d_outcome = outcome.as_ref().and_then(|result| {
        run_arm_evidence(
            &ArmRun {
                label,
                slug: "branched_ribbon_conformal_spiral",
                kind: ArmKind::Analytic,
                // DELIBERATELY None, and this is not an omission.
                //
                // `analytic_spacing_verdict` is ARM SPHERE's PRE-REGISTERED
                // three-way branch, and its "near HALF the target" arm prints
                // "off-by-one-band defect in the ring recursion". That verdict
                // was registered for a fixture where the map is near-isometric
                // and the ring can be spaced correctly everywhere at once. On
                // a BRANCHED region it cannot: the worst-sector rule spaces
                // every non-worst sector tighter than target by the measured
                // ring anisotropy, so a median anisotropy of 1.5-3x puts the
                // measured median at 0.33-0.67x target — straddling the half
                // band — and the instrument would print a recursion-defect
                // diagnosis that its OWN over-cover row contradicts two
                // screens later. Running a pre-registered verdict on an arm it
                // was not registered for is the instrument-integrity failure
                // this file exists to prevent. The envelope block printed
                // above still gives the target context; the question this arm
                // asks is the TRADE, not the spacing.
                analytic_target_mm: None,
                analytic: Some(RIBBON_SURFACE),
                run_stage_e: false,
            },
            fixture,
            &region,
            &report,
            result,
            params,
            stepover_mm,
        )
    });
    let trade = stage_d_outcome.as_ref();
    print_trade_verdict(label, trade, &report, &scan, gate_passed);
}

// ── ARM BAND — the geometry the programme actually came from ────────────
//
// The operator's motivation is not "branched shapes are hard"; it is that
// SLOPE-ANGLE-BANDED REGIONS PRODUCE WALLS OF RETRACTS. Regions in this
// codebase are selected by slope range — `finish_planner`'s `FinishBand`
// Shallow / MidSteep / VerySteep decomposition — and on an undulating surface
// a slope band is inherently scattered: it carves the surface into stripes and
// islands, and rastering those means lifting constantly. Their metric is TOTAL
// TIME INCLUDING RETRACTS, not shortest cut.

/// Side of the ARM BAND patch (mm).
const BAND_PATCH_MM: f64 = 22.0;

/// Height of each bump (mm).
const BAND_BUMP_AMPLITUDE_MM: f64 = 1.5;

/// Support radius of each bump (mm) — the surface is exactly flat outside it.
const BAND_BUMP_RADIUS_MM: f64 = 4.0;

/// Centre-to-centre pitch of the 2 × 2 bump lattice (mm). Greater than
/// `2 × BAND_BUMP_RADIUS_MM`, so the four supports are **disjoint** and the
/// single-bump slope and curvature bounds below are exact for the whole
/// surface rather than being a superposition estimate.
const BAND_BUMP_PITCH_MM: f64 = 10.0;

/// Shallow/MidSteep threshold (deg) — the analogue of
/// `FinishPlannerParams::steep_threshold_deg`, which SHIPS AT **45.0**.
///
/// # Why an analogue and not the shipped number
///
/// Reaching 45° while keeping every concavity larger than the ball forces the
/// feature scale up, and the facet ceiling then forces the triangle count up
/// with the square of it: at a 50° peak slope this fixture measures **387 k
/// triangles** against the 102 k it costs at 30°, for a patch that holds the
/// same 2 × 2 bump lattice. The claim this arm makes is **topological** — how
/// many components a slope band has, and how many holes each one carries — and
/// that claim is threshold-invariant: it is a statement about a band being an
/// annulus rather than a disk, which does not care where the two thresholds
/// sit. So the arm scales the thresholds down instead of scaling the mesh up,
/// and says so.
///
/// The one thing the analogue does NOT reproduce is the shipped planner's own
/// output on this surface: at a 45° threshold the whole patch is `Shallow` and
/// the decomposition emits ONE region. That is a fact about the fixture's
/// gentleness, not about the pipeline.
const BAND_SHALLOW_MAX_DEG: f64 = 10.0;

/// MidSteep/VerySteep threshold (deg) — the analogue of
/// `FinishPlannerParams::waterline_threshold_deg`, which SHIPS AT **75.0**.
/// See [`BAND_SHALLOW_MAX_DEG`].
const BAND_STEEP_MIN_DEG: f64 = 20.0;

/// Area (mm²) below which a component is not worth planning a spiral on at
/// all: `10 × stepover²`.
///
/// **[REPO] bounding number**, asked for as a sanity figure rather than a bar.
/// Ten stepovers-squared is about twenty passes' worth of ground; below that
/// the ring search has fewer rings than a raster has passes and the comparison
/// stops meaning anything. It is reported as an area FRACTION of the band, not
/// as a component count, because one big component and nine crumbs is a
/// completely different situation from ten equal crumbs.
fn band_min_useful_area_mm2(stepover_mm: f64) -> f64 {
    10.0 * stepover_mm * stepover_mm
}

/// Centres of the 2 × 2 bump lattice.
fn band_bump_centres() -> [(f64, f64); 4] {
    let h = 0.5 * BAND_BUMP_PITCH_MM;
    [(-h, -h), (h, -h), (-h, h), (h, h)]
}

/// The ARM BAND heightfield: four **raised-cosine** bumps on a flat plane.
///
/// ```text
/// z(d) = A/2 · (1 + cos(π·d/R))   for d ≤ R,   0 otherwise
/// ```
///
/// # Why a raised cosine and not a Gaussian or another sin·sin patch
///
/// * it has **compact support**, so with a pitch greater than `2R` the four
///   bumps never overlap and every bound below is exact rather than a
///   superposition estimate;
/// * it is `C¹` at the foot (`z = 0`, `z' = 0`), so the bump joins the plane
///   without a crease that would put a spurious slope band around every foot;
/// * its slope and curvature extrema are closed-form, which is what lets this
///   arm state the band radii — and therefore its own topology prediction — by
///   hand, before the run.
///
/// A `sin(kx)·sin(ky)` patch was measured (on paper) to be the wrong fixture
/// here: its low-slope set is a scatter of isolated ~0.4 mm disks and its
/// mid-slope set is a checkerboard of squares touching at their CORNERS, which
/// is a digitisation artefact fight rather than the operator's geometry.
///
/// # Slope arithmetic
///
/// `dz/dd = −A·π/(2R)·sin(π d/R)`, so `|∇z|max = A·π/(2R)` at `d = R/2`:
/// `1.5·π/8 = 0.58905` ⇒ **MAX SLOPE 30.52°**.
///
/// # Concavity arithmetic
///
/// The radial curvature is `−A·π²/(2R²)·cos(π d/R)`, largest **positive**
/// (concave — the valley at the foot) at `d = R`; the tangential curvature
/// `(1/d)·dz/dd` is bounded by the same figure because `sin t / t ≤ 1`. So
///
/// ```text
/// |κ|max = A·π²/(2R²) = 1.5·9.8696/32 = 0.46264 /mm  ⇒  R_min = 2.1615 mm
/// ```
///
/// against `K_c = 1.0`: the ball fits every concavity, `R_min ≥ 2·K_c`.
fn band_height(x: f64, y: f64) -> f64 {
    let mut z = 0.0;
    for (cx, cy) in band_bump_centres() {
        let d = (x - cx).hypot(y - cy);
        if d < BAND_BUMP_RADIUS_MM {
            z += 0.5 * BAND_BUMP_AMPLITUDE_MM * (1.0 + (PI * d / BAND_BUMP_RADIUS_MM).cos());
        }
    }
    z
}

/// `|∇z|max = A·π/(2R)`. See [`band_height`].
fn band_max_gradient() -> f64 {
    BAND_BUMP_AMPLITUDE_MM * PI / (2.0 * BAND_BUMP_RADIUS_MM)
}

/// `|κ|max = A·π²/(2R²)`. See [`band_height`].
fn band_max_curvature() -> f64 {
    BAND_BUMP_AMPLITUDE_MM * PI * PI / (2.0 * BAND_BUMP_RADIUS_MM * BAND_BUMP_RADIUS_MM)
}

/// Radius `d` at which a bump's slope equals `slope_deg`, on the requested
/// side of the `d = R/2` crest.
///
/// `slope(d) = atan(A·π/(2R)·sin(π d/R))`, so
/// `sin(π d/R) = tan(slope) / (A·π/(2R))` and the two roots are
/// `d = R·arcsin(·)/π` (inner) and `d = R·(π − arcsin(·))/π` (outer). Used to
/// print the band radii this arm predicts BEFORE it measures them, so the run
/// confirms or refutes a stated prediction rather than merely producing
/// numbers. `None` when the slope is never attained.
fn band_radius_at_slope(slope_deg: f64, outer: bool) -> Option<f64> {
    let sine = slope_deg.to_radians().tan() / band_max_gradient();
    if !(0.0..=1.0).contains(&sine) {
        return None;
    }
    let t = sine.asin();
    let t = if outer { PI - t } else { t };
    Some(BAND_BUMP_RADIUS_MM * t / PI)
}

/// Whole grid cells **both** of whose triangles fall in the half-open slope
/// band `[lo, hi)`.
///
/// Whole cells for [`cells_to_triangles`]' reason. The predicate itself is
/// per-triangle and reads the face normal — [`face_slope_deg`] — which is what
/// the shipped `SlopeMap` measures and what the smoke test pins.
fn band_cell_mask(mesh: &TriangleMesh, cells: usize, lo_deg: f64, hi_deg: f64) -> Vec<bool> {
    let mut mask = vec![false; cells * cells];
    for row in 0..cells {
        for col in 0..cells {
            let both = cell_triangles(cells, row, col).iter().all(|&t| {
                mesh.faces
                    .get(t as usize)
                    .is_some_and(|f| slope_in_band(face_slope_deg(f.normal), lo_deg, hi_deg))
            });
            mask[row * cells + col] = both;
        }
    }
    mask
}

/// Print one band's component-by-component topology census, and return the
/// components.
fn print_band_topology(
    band: &str,
    mesh: &TriangleMesh,
    triangles: &[u32],
    lo_deg: f64,
    hi_deg: f64,
    stepover_mm: f64,
) -> Vec<ComponentTopology> {
    let components = band_topology(mesh, triangles);
    let min_useful = band_min_useful_area_mm2(stepover_mm);
    let total_area: f64 = components.iter().map(|c| c.census.area_mm2).sum();
    eprintln!(
        "\n   -- BAND {band}  slope [{lo_deg:.1}, {hi_deg:.1}) deg — {} triangles, \
         {} COMPONENT(S), {total_area:.2} mm² --",
        triangles.len(),
        components.len()
    );
    if components.is_empty() {
        eprintln!("     EMPTY — no triangle on this fixture falls in this band.");
        return components;
    }
    eprintln!(
        "     {:>4} {:>10} {:>12} {:>8} {:>7} {:>7} {:>7} {:>10}",
        "#", "triangles", "3D area mm²", "loops", "Euler", "holes", "DISK?", "too small?"
    );
    let shown = components.len().min(12);
    for (i, component) in components.iter().take(shown).enumerate() {
        eprintln!(
            "     {:>4} {:>10} {:>12.4} {:>8} {:>7} {:>7} {:>7} {:>10}",
            i,
            component.triangles.len(),
            component.census.area_mm2,
            component.census.boundary_loops,
            component.census.euler,
            component.holes(),
            if component.is_disk() { "YES" } else { "no" },
            if component.census.area_mm2 < min_useful {
                "YES"
            } else {
                "no"
            }
        );
    }
    if components.len() > shown {
        eprintln!(
            "     ... {} further component(s) not listed; the aggregates below cover ALL of them.",
            components.len() - shown
        );
    }

    let disks = components.iter().filter(|c| c.is_disk()).count();
    let disk_area: f64 = components
        .iter()
        .filter(|c| c.is_disk())
        .map(|c| c.census.area_mm2)
        .sum();
    let small_area: f64 = components
        .iter()
        .filter(|c| c.census.area_mm2 < min_useful)
        .map(|c| c.census.area_mm2)
        .sum();
    let useful_disk_area: f64 = components
        .iter()
        .filter(|c| c.is_disk() && c.census.area_mm2 >= min_useful)
        .map(|c| c.census.area_mm2)
        .sum();
    eprintln!(
        "     components that are TOPOLOGICAL DISKS (1 loop, Euler 1): {disks} of {} — \
         {disk_area:.2} mm² = {:.2}% of the band",
        components.len(),
        100.0 * disk_area / total_area.max(1e-12)
    );
    eprintln!(
        "     area in components BELOW {min_useful:.4} mm² (= 10 x stepover²): \
         {small_area:.2} mm² = {:.2}% of the band",
        100.0 * small_area / total_area.max(1e-12)
    );
    eprintln!(
        "     area a spiral could even be ASKED about (disk AND >= {min_useful:.4} mm²): \
         {useful_disk_area:.2} mm² = {:.2}% of the band",
        100.0 * useful_disk_area / total_area.max(1e-12)
    );
    components
}

/// The retract wall, quantified — item 2 of ARM BAND's brief.
fn band_raster_wall(
    label: &str,
    fixture: Fixture<'_>,
    polygons: &[Polygon2],
    region_area_mm2: f64,
    stepover_mm: f64,
) -> CandidateCost {
    use rs_cam_core::geometry::region_set::RegionSet;

    eprintln!("\n   ===== THE RETRACT WALL — 0deg BALL RASTER OVER {label} =====");
    eprintln!("     {FRESH_STOCK_LABEL}");
    let grid = grid_for_direction(
        fixture.mesh,
        fixture.index,
        fixture.cutter,
        stepover_mm,
        0.0,
    );
    let raw = raster_candidate(&grid, polygons, fixture.safe_z, fixture.effective_min_z);
    let boundary = RegionSet::new(polygons.to_vec());
    let cost = relink_and_cost(
        raw,
        fixture.mesh,
        fixture.index,
        fixture.cutter,
        &boundary,
        &fixture.kinematics,
        fixture.safe_z,
    );
    let area_cm2 = region_area_mm2 / 100.0;
    eprintln!(
        "     region                        {:>12} polygon(s), {} hole(s), {region_area_mm2:.2} \
         mm² of 3D surface",
        polygons.len(),
        polygons.iter().map(|p| p.holes.len()).sum::<usize>()
    );
    eprintln!("     moves                         {:>12}", cost.moves);
    eprintln!("     FRAGMENTS                     {:>12}", cost.fragments);
    eprintln!("     surface links the relinker ADDED {:>9}", cost.linked);
    eprintln!(
        "     KEPT RETRACTS                 {:>12}   <<< the wall. Every one is a lift-traverse-\
         plunge",
        cost.kept_retracts
    );
    eprintln!(
        "     retracts per cm² of region    {:>12.3}   (comparable across fixtures of different \
         size —",
        cost.kept_retracts as f64 / area_cm2.max(1e-12)
    );
    eprintln!(
        "\x20                                              which a raw retract count is NOT)"
    );
    eprintln!(
        "     cutting distance (mm)         {:>12.1}",
        cost.cutting_mm
    );
    eprintln!(
        "     F-034 TIME (s)                {:>12.1}   <<< TOTAL TIME INCLUDING RETRACTS — the\n\
         \x20                                              operator's metric, not cut distance",
        cost.time_s
    );
    eprintln!(
        "     Reference, Wanaka thin-organic region 1: 0deg undivided \
         {WANAKA_R1_RASTER_FRAGMENTS} frags / {WANAKA_R1_RASTER_RETRACTS} retracts;\n\
         \x20    PCA-minor cells (the production-validated bar) {WANAKA_R1_PCA_FRAGMENTS} / \
         {WANAKA_R1_PCA_RETRACTS}. THIS is the wall the whole\n\
         \x20    programme went looking for a low-retract algorithm to knock down."
    );
    cost
}

fn arm_band(
    cutter: &BallEndmill,
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
    stepover_mm: f64,
    params: &SpiralParams,
) {
    eprintln!(
        "\n══════════ ARM BAND — SLOPE-BANDED region (the geometry the programme came from) \
         ══════════\n\
         \x20  Regions in this codebase are selected by SLOPE RANGE — `finish_planner`'s\n\
         \x20  FinishBand Shallow / MidSteep / VerySteep decomposition — and on an undulating\n\
         \x20  surface a slope band is inherently scattered: it carves the surface into stripes\n\
         \x20  and islands, and rastering those means lifting constantly. That retract wall, not\n\
         \x20  path length, is why this programme went looking for a low-retract algorithm, and\n\
         \x20  the operator's metric is TOTAL TIME INCLUDING RETRACTS.\n\
         \x20  A `NotSimplyConnected` refusal on this arm is a HEADLINE FINDING, NOT A FAILURE:\n\
         \x20  it would mean the hole machinery (F2 step 3) is LOAD-BEARING for the main use\n\
         \x20  case rather than an optional extra — the opposite of what FINDINGS_F2 currently\n\
         \x20  recommends.\n"
    );

    let max_gradient = band_max_gradient();
    let max_slope_deg = max_gradient.atan().to_degrees();
    let max_curvature = band_max_curvature();
    let cells = heightfield_cells(BAND_PATCH_MM, max_gradient, ANALYTIC_MAX_EDGE_MM);
    let mesh = heightfield_mesh(BAND_PATCH_MM, cells, band_height);
    let index = SpatialIndex::build_auto(&mesh);

    eprintln!(
        "   fixture: 4 raised-cosine bumps, z = A/2*(1 + cos(pi*d/R)) for d <= R, on a flat \
         plane.\n\
         \x20           A {BAND_BUMP_AMPLITUDE_MM} mm, R {BAND_BUMP_RADIUS_MM} mm, lattice pitch \
         {BAND_BUMP_PITCH_MM} mm (> 2R, so the four supports are\n\
         \x20           DISJOINT and every bound below is exact, not a superposition estimate).\n\
         \x20           Patch {BAND_PATCH_MM} mm on a {cells} x {cells} grid, cell {:.5} mm, \
         {} triangles.\n\
         \x20  |grad z|max = A*pi/(2R)   = {max_gradient:.5}  =>  MAX SLOPE {max_slope_deg:.2} deg\n\
         \x20  |kappa|max  = A*pi^2/(2R^2) = {max_curvature:.5} 1/mm  =>  R_min {:.4} mm against \
         K_c {BALL_RADIUS_MM} mm (bar: >= {:.1} mm).",
        BAND_PATCH_MM / cells as f64,
        mesh.triangles.len(),
        1.0 / max_curvature,
        2.0 * BALL_RADIUS_MM
    );
    eprintln!(
        "   THRESHOLDS: this arm bands at [0, {BAND_SHALLOW_MAX_DEG}) / \
         [{BAND_SHALLOW_MAX_DEG}, {BAND_STEEP_MIN_DEG}) / [{BAND_STEEP_MIN_DEG}, inf) deg.\n\
         \x20  The SHIPPED FinishPlannerParams thresholds are steep_threshold_deg 45.0 and\n\
         \x20  waterline_threshold_deg 75.0. These are SCALED ANALOGUES and the substitution is\n\
         \x20  stated, not hidden: reaching 45 deg while keeping every concavity larger than the\n\
         \x20  ball forces the feature scale up and the facet ceiling then forces the triangle\n\
         \x20  count up with its square — ~387k triangles at a 50 deg peak against the {} here.\n\
         \x20  The claim this arm makes is TOPOLOGICAL (how many components, how many holes each\n\
         \x20  one has) and that is threshold-invariant. What the analogue does NOT reproduce is\n\
         \x20  the shipped planner's own output on this surface: at 45 deg the whole patch is\n\
         \x20  Shallow and the decomposition emits ONE region. That is a fact about the fixture's\n\
         \x20  gentleness, not about the pipeline.\n",
        mesh.triangles.len()
    );

    // ── THE HAND PREDICTION, stated BEFORE the census ──────────────────
    let r_in_shallow = band_radius_at_slope(BAND_SHALLOW_MAX_DEG, false);
    let r_out_shallow = band_radius_at_slope(BAND_SHALLOW_MAX_DEG, true);
    let r_in_steep = band_radius_at_slope(BAND_STEEP_MIN_DEG, false);
    let r_out_steep = band_radius_at_slope(BAND_STEEP_MIN_DEG, true);
    let fmt = |r: Option<f64>| r.map_or("n/a".to_owned(), |v| format!("{v:.4}"));
    eprintln!(
        "   ===== THE PREDICTION, COMPUTED BY HAND FROM THE CLOSED FORM, BEFORE THE CENSUS \
         =====\n\
         \x20  Band radii on ONE bump (d from its centre), from sin(pi d/R) = tan(slope)/(A pi/2R):\n\
         \x20    slope {BAND_SHALLOW_MAX_DEG} deg at d = {} (inner) and d = {} (outer)\n\
         \x20    slope {BAND_STEEP_MIN_DEG} deg at d = {} (inner) and d = {} (outer)\n\
         \x20  Therefore, per bump:\n\
         \x20    SHALLOW  = a tiny CAP (d < inner-10) + an outer ANNULUS (d > outer-10) that is\n\
         \x20               CONTINUOUS WITH THE FLAT PLANE, because the bump meets the plane at\n\
         \x20               zero slope (C1 foot).\n\
         \x20    MIDSTEEP = TWO annuli, (inner-10, inner-20) and (outer-20, outer-10).\n\
         \x20    STEEP    = ONE annulus, (inner-20, outer-20).\n\
         \x20  So the PREDICTED topology census is:\n\
         \x20    SHALLOW  : 5 components — ONE big one (plane + 4 outer annuli) with FOUR HOLES,\n\
         \x20               Euler -3 and 5 boundary loops, plus 4 cap disks of ~{:.4} mm² each\n\
         \x20               (BELOW the {:.4} mm² usefulness floor).\n\
         \x20    MIDSTEEP : 8 components, EVERY ONE AN ANNULUS — Euler 0, 2 boundary loops.\n\
         \x20    STEEP    : 4 components, EVERY ONE AN ANNULUS — Euler 0, 2 boundary loops.\n\
         \x20  If that is what the run measures, then NOT ONE band on this surface is a\n\
         \x20  topological disk at a useful size, plan_spiral refuses every one of them on\n\
         \x20  topology alone, and the slit map is LOAD-BEARING for the operator's real geometry.\n\
         \x20  If the run measures something else, the prediction was wrong and the measurement\n\
         \x20  wins — that is the point of stating it first.\n",
        fmt(r_in_shallow),
        fmt(r_out_shallow),
        fmt(r_in_steep),
        fmt(r_out_steep),
        PI * r_in_shallow.unwrap_or(0.0).powi(2),
        band_min_useful_area_mm2(stepover_mm)
    );

    // ── ITEM 1: the topology census, FIRST, before anything else ───────
    eprintln!("\n   ===== ARM BAND TOPOLOGY CENSUS (item 1 — read this before any cost) =====");
    let shallow_mask = band_cell_mask(&mesh, cells, 0.0, BAND_SHALLOW_MAX_DEG);
    let mid_mask = band_cell_mask(&mesh, cells, BAND_SHALLOW_MAX_DEG, BAND_STEEP_MIN_DEG);
    let steep_mask = band_cell_mask(&mesh, cells, BAND_STEEP_MIN_DEG, 180.0);
    let shallow = cells_to_triangles(&shallow_mask, cells);
    let mid = cells_to_triangles(&mid_mask, cells);
    let steep = cells_to_triangles(&steep_mask, cells);

    let shallow_components = print_band_topology(
        "SHALLOW",
        &mesh,
        &shallow,
        0.0,
        BAND_SHALLOW_MAX_DEG,
        stepover_mm,
    );
    print_band_topology(
        "MIDSTEEP",
        &mesh,
        &mid,
        BAND_SHALLOW_MAX_DEG,
        BAND_STEEP_MIN_DEG,
        stepover_mm,
    );
    print_band_topology(
        "VERYSTEEP",
        &mesh,
        &steep,
        BAND_STEEP_MIN_DEG,
        180.0,
        stepover_mm,
    );

    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter,
        kinematics,
        safe_z: mesh.bbox.max.z + 5.0,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };

    // ── ITEM 2: the retract wall on the band the shipped planner RASTERS ─
    //
    // `FinishBand::Shallow`'s own doc: "Slope below steep_threshold_deg —
    // parallel raster passes." So the shallow band is the one whose retract
    // count is the operator's complaint, and it is the one costed here.
    let shallow_polygons = mask_polygons(&shallow_mask, BAND_PATCH_MM, cells);
    let shallow_area = region_area_mm2(&mesh, &shallow);
    let scan = scanline_census(&shallow_polygons, stepover_mm);
    let gate_passed = print_fragmentation_gate("ARM BAND / SHALLOW", &scan, stepover_mm);
    let raster_cost = band_raster_wall(
        "ARM BAND / SHALLOW",
        fixture,
        &shallow_polygons,
        shallow_area,
        stepover_mm,
    );

    // ── ITEM 3: can a spiral be planned on ANY component at all? ────────
    eprintln!("\n   ===== ITEM 3 — CAN A SPIRAL BE PLANNED ON ANY SHALLOW COMPONENT? =====");
    let min_useful = band_min_useful_area_mm2(stepover_mm);
    let qualifying = shallow_components
        .iter()
        .find(|c| c.is_disk() && c.census.area_mm2 >= min_useful);
    let largest = shallow_components.first();

    match (qualifying, largest) {
        (Some(component), _) => {
            eprintln!(
                "     A QUALIFYING DISK EXISTS: {} triangles, {:.4} mm², 1 boundary loop, Euler 1.\n\
                 \x20    Planning the spiral on THAT COMPONENT ALONE — the prediction above said \
                 there\n\
                 \x20    would be none, so this line refutes it and the refutation is the finding.",
                component.triangles.len(),
                component.census.area_mm2
            );
            let mask = triangles_to_cell_mask(&component.triangles, cells);
            let polygons = mask_polygons(&mask, BAND_PATCH_MM, cells);
            match polygons.first().cloned() {
                Some(polygon) => {
                    let arm = plan_selected_region(
                        "ARM BAND",
                        &mesh,
                        &index,
                        &component.triangles,
                        polygon,
                        params,
                    );
                    let Arm {
                        label,
                        region,
                        report,
                        outcome,
                    } = arm;
                    eprintln!();
                    print_distortion_blocks(label, &report);
                    let stage_d_outcome = match outcome {
                        Ok(result) => run_arm_evidence(
                            &ArmRun {
                                label,
                                slug: "slope_band_conformal_spiral",
                                kind: ArmKind::Analytic,
                                analytic_target_mm: None,
                                analytic: Some(BAND_SURFACE),
                                run_stage_e: false,
                            },
                            fixture,
                            &region,
                            &report,
                            &result,
                            params,
                            stepover_mm,
                        ),
                        Err(refusal) => {
                            let ok = diagnose_refusal(label, &report, &refusal, params);
                            assert!(
                                ok,
                                "{label} refused with {refusal:?} on a component this instrument \
                                 had already measured to be a topological disk, and the report \
                                 carried nothing to attribute it by."
                            );
                            None
                        }
                    };
                    // NOTE the census handed to the verdict is the WHOLE
                    // SHALLOW BAND's, not this component's: the raster cost it
                    // is compared against is the whole band's too, so the two
                    // sides agree — but a reader must not read the gate line
                    // as a statement about this one component's shape.
                    let trade = stage_d_outcome.as_ref();
                    print_trade_verdict(label, trade, &report, &scan, gate_passed);
                }
                None => eprintln!(
                    "     the component's cell mask produced no polygon — cannot cost it."
                ),
            }
        }
        (None, Some(component)) => {
            eprintln!(
                "     NO COMPONENT OF THE SHALLOW BAND IS A TOPOLOGICAL DISK AT A USEFUL SIZE.\n\
                 \x20    The largest component has {} triangles, {:.4} mm², {} boundary loop(s) \
                 and Euler {}\n\
                 \x20    — i.e. {} hole(s). Planning on it anyway, so the refusal is on the \
                 record and\n\
                 \x20    typed rather than being asserted from the census:",
                component.triangles.len(),
                component.census.area_mm2,
                component.census.boundary_loops,
                component.census.euler,
                component.holes()
            );
            // Hygiene FIRST, then plan. Without it, one bowtie vertex anywhere
            // in this frame comes back as `NonManifoldBoundary` and the arm
            // would report "the prediction was refuted, the measurement wins"
            // when the real story is a SELECTION ARTEFACT — the exact class of
            // error the FINDINGS_F2 withdrawal is about. `clean_selection`
            // never touches a hole (its own docs say so, and it only ever
            // REMOVES triangles), so the `NotSimplyConnected` question this arm
            // exists to ask survives it intact.
            let (cleaned, cleanup) = clean_selection(&mesh, &component.triangles);
            eprintln!(
                "     selection hygiene on that component FIRST: {} -> {} triangles, {} \
                 component(s) on\n\
                 \x20    the first pass, {} pinch pass(es), {} triangle(s) shaved. This runs \
                 BEFORE the plan\n\
                 \x20    so that a single bowtie vertex cannot come back as NonManifoldBoundary \
                 and be misread\n\
                 \x20    as the topology answer. clean_selection only ever REMOVES triangles and \
                 never touches\n\
                 \x20    a hole, so the NotSimplyConnected question survives it intact.",
                cleanup.before,
                cleanup.after,
                cleanup.components_first_pass,
                cleanup.pinch_iterations,
                cleanup.triangles_shaved_by_pinch
            );
            let (report, outcome) = conformal_spiral::plan_spiral(&mesh, &index, &cleaned, params);
            // Captured for the comparison figure's empty spiral panel: the
            // picture must say WHY there is no path, in the same words the
            // stderr transcript uses.
            let spiral_note: String;
            match outcome {
                Ok(_) => {
                    spiral_note =
                        "plan_spiral ACCEPTED this component but it is not costed on this branch"
                            .to_owned();
                    eprintln!(
                        "     plan_spiral ACCEPTED it. The census above and the module disagree, \
                         which is\n\
                         \x20    a defect in ONE of them — do not proceed until that is resolved."
                    );
                }
                Err(refusal) => {
                    spiral_note = format!("plan_spiral REFUSED: {refusal:?}");
                    eprintln!("     REFUSAL: {refusal:?}");
                    match &refusal {
                        SpiralRefusal::NotSimplyConnected { boundary_loops } => eprintln!(
                            "\n     *** HEADLINE FINDING — NotSimplyConnected {{ boundary_loops: \
                             {boundary_loops} }} ***\n\
                             \x20    The operator's real target geometry is selected BY SLOPE \
                             RANGE, and a slope\n\
                             \x20    band on a bumpy surface is multiply connected BY \
                             CONSTRUCTION: the other\n\
                             \x20    bands become HOLES inside it. So the slit map / hole \
                             machinery (Phase F2\n\
                             \x20    step 3) is LOAD-BEARING FOR THE MAIN USE CASE, not an \
                             optional extra — the\n\
                             \x20    OPPOSITE of FINDINGS_F2's current recommendation, which \
                             says 'do not proceed\n\
                             \x20    to the slit map / holes on efficiency grounds'. That \
                             recommendation was\n\
                             \x20    reached on a convex ellipse and a sphere cap, neither of \
                             which can exhibit\n\
                             \x20    this. It should be revisited against this arm."
                        ),
                        other => eprintln!(
                            "\n     The refusal is {other:?}, NOT NotSimplyConnected. The \
                             prediction named\n\
                             \x20    NotSimplyConnected; this refutes it, and the measurement \
                             wins. Read the\n\
                             \x20    diagnosis below before concluding anything about the slit \
                             map."
                        ),
                    }
                    diagnose_refusal(
                        "ARM BAND (largest shallow component)",
                        &report,
                        &refusal,
                        params,
                    );
                }
            }
            eprintln!(
                "\n     There is therefore NO SPIRAL COST ON THIS ARM, and that absence is the \
                 result:\n\
                 \x20    the raster pays {} kept retracts over {:.2} mm² ({:.3} per cm²) and the \
                 method\n\
                 \x20    under test cannot be asked to beat it, because it refuses the geometry \
                 on\n\
                 \x20    topology before any spacing question is reached.",
                raster_cost.kept_retracts,
                shallow_area,
                raster_cost.kept_retracts as f64 / (shallow_area / 100.0).max(1e-12)
            );

            // ── THE OPERATOR'S PICTURE ON THE ARM THAT REFUSED ─────────
            //
            // This arm never reaches Stage D, so without this block the one
            // fixture whose retract wall motivated the whole programme would
            // be the one fixture with no figure. The spiral panel is drawn
            // EMPTY carrying its refusal, the raster panel is the wall itself,
            // and the field is run on the WHOLE SHALLOW BAND — the same region
            // the raster was costed on, so the two drawn rows are a fair pair.
            let band = "ARM BAND / SHALLOW";
            let field = field_candidate(band, fixture, &shallow, &shallow_polygons);
            // The §4.3 candidate runs here too, on the SAME multiply-connected
            // shallow band the raster was costed on. Nothing in the Poisson
            // solve or the marching-triangles extraction cares about topology —
            // that is the spiral's constraint, not the field's — so this is the
            // one arm where the two methods can be compared on the geometry the
            // shipped planner actually rasters.
            let sweep_direction = pca_major_axis(&mesh, &shallow);
            let sweep = sweep_field_candidate(
                band,
                fixture,
                &shallow,
                &shallow_polygons,
                BAND_SURFACE,
                &sweep_direction,
            );
            // THE FIFTH CANDIDATE (2026-08-31), on the arm the programme
            // actually came from. This is where it earns its row: the medial
            // axis of a PLANE WITH FOUR HOLES is a completely different animal
            // from a branched ribbon's — the skeleton of the square's own
            // edges blended with four circular ones — and a GLOBAL sweep
            // direction is least defensible on exactly this geometry. The
            // shallow band is also MULTIPLY CONNECTED, which the Poisson solve
            // and the marching-triangles extraction do not care about (that is
            // the spiral's constraint, not the field's), so all three field
            // rows are measurable here where the spiral row is NO PATH.
            let medial_grid = build_medial_grid(band, &shallow_polygons, stepover_mm);
            let medial = medial_grid.as_ref().map(|grid| {
                medial_field_candidate(
                    band,
                    fixture,
                    &shallow,
                    &shallow_polygons,
                    BAND_SURFACE,
                    grid,
                )
            });
            let medial_note = medial.as_ref().map_or_else(
                || "no medial grid could be built over this region".to_owned(),
                |f| f.note.clone(),
            );
            let medial_cost = medial.as_ref().and_then(|f| f.cost.as_ref());

            // One table, so the four costed rows on this arm are readable
            // together instead of only inside their own solve blocks. The
            // spiral row is absent because it REFUSED, which is this arm's
            // headline and is stated above rather than shown as a dash here.
            eprintln!(
                "\n     {:<36} {:>10} {:>8} {:>10} {:>10} {:>9}",
                "arm (ARM BAND / SHALLOW)", "fragments", "linked", "RETRACTS", "cut mm", "time s"
            );
            let band_rows: [(&str, Option<&CandidateCost>); 4] = [
                ("0deg ball raster (the wall)", Some(&raster_cost)),
                ("direction field, D = t1 (F1)", field.cost.as_ref()),
                ("iso-scallop field, D = sweep (4.3)", sweep.cost.as_ref()),
                ("iso-scallop field, D = MEDIAL AXIS", medial_cost),
            ];
            for (name, row) in band_rows {
                match row {
                    Some(cost) => eprintln!(
                        "     {name:<36} {:>10} {:>8} {:>10} {:>10.1} {:>9.1}",
                        cost.fragments,
                        cost.linked,
                        cost.kept_retracts,
                        cost.cutting_mm,
                        cost.time_s
                    ),
                    None => eprintln!(
                        "     {name:<36} {:>10} {:>8} {:>10} {:>10} {:>9}   <<< NO PATH",
                        "-", "-", "-", "-", "-"
                    ),
                }
            }
            eprintln!(
                "     The RETRACTS column is what this arm exists for: a slope band is \
                 scattered by\n\
                 \x20    construction and rastering it means lifting constantly. All three field \
                 rows are\n\
                 \x20    measurable here even though the spiral is not, because a Poisson solve \
                 and a\n\
                 \x20    marching-triangles extraction do not care about topology — that is the\n\
                 \x20    SPIRAL's constraint, not the field's."
            );
            let floor = region_floor(&mesh, &shallow, BAND_SURFACE);
            print_floor_block(
                band,
                BAND_SURFACE,
                &floor,
                &[
                    ("conformal spiral", None),
                    ("0deg ball raster (the wall)", Some(raster_cost.cutting_mm)),
                    (
                        "direction field, D = t1 (F1)",
                        field.cost.as_ref().map(|c| c.cutting_mm),
                    ),
                    (
                        "iso-scallop field, D = sweep (4.3)",
                        sweep.cost.as_ref().map(|c| c.cutting_mm),
                    ),
                    (
                        "iso-scallop field, D = MEDIAL AXIS",
                        medial_cost.map(|c| c.cutting_mm),
                    ),
                ],
            );
            eprintln!(
                "     The spiral row is NO PATH by refusal, not by omission — it is the arm's \
                 headline\n\
                 \x20    finding, and a floor table that quietly dropped the refusing candidate \
                 would be\n\
                 \x20    reporting a three-way race that never happened."
            );
            let (compare_path, overlay_path) = write_comparison_svgs(
                "slope_band_shallow",
                band,
                &shallow_polygons,
                &[
                    ComparePanel {
                        title: "conformal spiral",
                        colour: SPIRAL_COLOUR,
                        cost: None,
                        note: &spiral_note,
                    },
                    ComparePanel {
                        title: "0deg ball raster (the wall)",
                        colour: RASTER_COLOUR,
                        cost: Some(&raster_cost),
                        note: "",
                    },
                    ComparePanel {
                        title: "direction field, D = t1 (F1)",
                        colour: FIELD_COLOUR,
                        cost: field.cost.as_ref(),
                        note: &field.note,
                    },
                    ComparePanel {
                        title: "iso-scallop field, D = sweep (4.3)",
                        colour: SWEEP_COLOUR,
                        cost: sweep.cost.as_ref(),
                        note: &sweep.note,
                    },
                    ComparePanel {
                        title: "iso-scallop field, D = MEDIAL AXIS",
                        colour: MEDIAL_COLOUR,
                        cost: medial_cost,
                        note: &medial_note,
                    },
                ],
            );
            eprintln!("\n     -- THE COMPARISON FIGURES — ARM BAND / SHALLOW --");
            eprintln!("       panels : {}", compare_path.display());
            eprintln!("       overlay: {}", overlay_path.display());
            eprintln!(
                "       The spiral panel is EMPTY and says why. That absence is the arm's \
                 headline\n\
                 \x20      result rendered: the method under test cannot be drawn on the geometry \
                 the\n\
                 \x20      operator actually machines, because it refuses it on topology before a\n\
                 \x20      spacing question is reached. The red rings in panel 2 are the retract \
                 wall."
            );
        }
        (None, None) => eprintln!("     the shallow band is EMPTY on this fixture."),
    }
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
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
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
    print_distortion_blocks(steep.label, &steep.report);
    eprintln!();
    print_distortion_blocks(flat_label, &report);
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
            analytic: None,
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
/// and then costs **three** candidates per analytic arm through the production
/// relinker — the conformal spiral, the 0° ball raster, and (added 2026-08-30)
/// a `direction_field` iso-curve solve, which adds its own Poisson solve and
/// marching-triangles extraction per arm.
///
/// The name is deliberately unchanged from the pre-withdrawal revision: it is
/// what PROGRAMME.md, FINDINGS_F2 and this file's own header run-command
/// quote. It now carries the analytic arms as well as the terrain ones.
#[test]
#[ignore = "evidence run — long runtime (6+ plan_spiral solves, three on ~37k/~14k/~36k-triangle analytic regions, plus four analytic meshes built in-process, plus THREE direction_field Poisson solves + marching-triangles extractions per analytic arm since 2026-08-31 — the curvature field, the synthesis §4.3 sweep field and the operator's medial-axis field, the last of which additionally builds one distance-transform grid per region); needs NO external files, every analytic fixture is generated and the terrain one is in-repo"]
fn terrain_small_conformal_spiral_f2() {
    use rs_cam_core::machine::kinematics::MachineKinematics;

    eprintln!(
        "\n########## PHASE F2 — conformal-spiral evidence, SIX ARMS, ANALYTIC FIRST ##########\n"
    );
    eprintln!(
        "   ORDER AND STANDING OF THE ARMS:\n\
         \x20    1. ARM SPHERE  analytic spherical cap    — DECISIVE for SPACING. Carries the\n\
         \x20                                               phase-1 spacing verdict; a refusal\n\
         \x20                                               here is a HARD FAILURE.\n\
         \x20    2. ARM WAVY    analytic wavy heightfield — the realistic-but-clean case; a\n\
         \x20                                               refusal here is a HARD FAILURE.\n\
         \x20    3. ARM RIBBON  analytic branched ribbon  — DECISIVE for the RETRACT TRADE. Narrow\n\
         \x20                                               branched arms, SIMPLY CONNECTED, so it\n\
         \x20                                               isolates DISTORTION from TOPOLOGY. A\n\
         \x20                                               DIAGNOSED refusal here is a FINDING;\n\
         \x20                                               an undiagnosable one is the failure.\n\
         \x20    4. ARM BAND    analytic bump lattice,    — DECISIVE for TOPOLOGY. Region selected\n\
         \x20                   SLOPE-BANDED region        BY SLOPE RANGE, the way finish_planner\n\
         \x20                                               actually selects one. A\n\
         \x20                                               NotSimplyConnected refusal here is a\n\
         \x20                                               HEADLINE FINDING, not a failure.\n\
         \x20    5. ARM STEEP   terrain_small.stl         — fixture-limited PROBE. Fine-geometry\n\
         \x20                                               numbers WITHDRAWN.\n\
         \x20    6. ARM FLAT    terrain_small.stl         — fixture-limited. Fine-geometry numbers\n\
         \x20                                               WITHDRAWN; retained for the radial\n\
         \x20                                               distortion pair and Stage E.\n\
         \x20  Why arms 1-2 exist: planning/conformal_finish_2026-08-28/FINDINGS_F2.md opens with a\n\
         \x20  WITHDRAWAL of every terrain fine-geometry figure — 1.42 mm facets cannot measure a\n\
         \x20  0.4862 mm stepover. PROGRAMME.md §F2 step 1 always said 'simply connected SYNTHETIC\n\
         \x20  surface'.\n\
         \x20  Why arms 3-4 exist: §F2-2 then measured the spiral 5-15% slower than a raster on\n\
         \x20  arms 1-2 — where the raster had 4 and 3 fragments and ZERO RETRACTS. A\n\
         \x20  retract-elimination method was benchmarked on geometry with no retracts to\n\
         \x20  eliminate. Arms 3 and 4 are the honest comparators, and BOTH carry a\n\
         \x20  raster-fragmentation VALIDITY GATE that fires before their own cost tables.\n"
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

    eprintln!(
        "   FIVE CANDIDATES (four from 2026-08-30, the fifth added 2026-08-31) — the operator \
         asked\n\
         \x20  to SEE the comparison, and seeing it meant drawing the rows that keep WINNING, not \
         only\n\
         \x20  the one under test. Every ANALYTIC arm now costs FIVE paths on one region through \
         one\n\
         \x20  relink — conformal spiral, 0deg ball raster, `direction_field` iso-curves \
         (D = t1), the\n\
         \x20  synthesis §4.3 iso-scallop field (D = the sweep direction) and the OPERATOR'S \
         iso-scallop\n\
         \x20  field (D = rot90(grad EDT), the region's MEDIAL AXIS) — and emits two figures per \
         arm:\n\
         \x20    {{slug}}_compare_f2.svg   five panels, IDENTICAL SCALE and IDENTICAL viewBox \
         SIZE, each\n\
         \x20                            labelled with its own measured row; surface links GREEN, \
         air RED\n\
         \x20                            dashed, LIFT POINTS as red rings.\n\
         \x20    {{slug}}_overlay_f2.svg   the same five superimposed at 45% opacity.\n\
         \x20  The three FIELD rows are the decisive set. They run the SAME Poisson solve, the \
         SAME\n\
         \x20  level schedule and the SAME extraction, and differ ONLY in the target field V — so \
         any\n\
         \x20  gap among them is attributable to the DIRECTION SOURCE and to nothing else. The\n\
         \x20  curvature row was falsified on Wanaka region 1 ({F1_FIELD_POLYLINES_REGION1} \
         polylines vs a {WANAKA_R1_PCA_FRAGMENTS}-fragment\n\
         \x20  reference) on SHALLOW TERRAIN, where principal curvature is NOISE — and a sphere \
         cap is\n\
         \x20  UMBILIC, which is the same degeneracy in its purest form. The sweep row is the\n\
         \x20  synthesis's repair; the MEDIAL row is the operator's, and because\n\
         \x20  V = -|V| * grad_hat(EDT) it is CONTOUR-PARALLEL MACHINING CARRYING THE \
         ISO-SCALLOP\n\
         \x20  SPACING LAW — parallel passes down each arm, closed loops around a hub. It comes\n\
         \x20  with its own pre-registration block and its own competing prior evidence \
         (FINDINGS.md\n\
         \x20  §0j 0.917x, §0k 0.686x — both COSTS, both on lattice-derived per-cell rigs; the\n\
         \x20  distinction is printed beside every medial row). All three FieldReports are \
         printed in\n\
         \x20  full so a bad field is diagnosable rather than merely slow, and an empty solve is\n\
         \x20  drawn as an EMPTY PANEL, never omitted.\n\
         \x20  Every analytic arm additionally prints x FLOOR: L_min = INTEGRAL dA / s_max(x) \
         from the\n\
         \x20  fixture's ANALYTIC curvature, and each candidate's cut_mm / L_min. A ratio below \
         1.0\n\
         \x20  PROVES that candidate is under-covering — synthesis §1, and the number nobody in \
         this\n\
         \x20  programme has ever measured.\n"
    );

    print_preregistration();
    print_medial_preregistration();

    arm_sphere(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_wavy(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_ribbon(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_band(&cutter, kinematics, stepover_mm, &analytic_params);
    arm_terrain(&cutter, kinematics, stepover_mm, &params);

    eprintln!("########## PHASE F2 evidence run complete. ##########\n");
}

/// **The pre-registration.** Printed BEFORE any arm runs, so the run confirms
/// or refutes a STATED prediction instead of merely producing numbers.
///
/// This file already does this for ARM BAND's topology census (hand-derived,
/// printed first, confirmed exactly) and for the ARM SPHERE spacing verdict
/// (three-way branch with a genuine INCONCLUSIVE arm). The 2026-08-30 additions
/// get the same treatment, because they are the additions most likely to be
/// read as "the number we hoped for".
///
/// Written from the geometry alone, against the previous run's measured rows
/// for the three candidates that already existed. It is deliberately stated as
/// RANGES with a falsifier, not as point estimates.
fn print_preregistration() {
    eprintln!(
        "\n########## PRE-REGISTERED EXPECTATIONS — 2026-08-30, BEFORE THE RUN ##########\n\
         \x20 Written from the geometry, before any arm executed. The three existing candidates\n\
         \x20 are quoted at their PREVIOUS measured values, so a drift in those is itself a\n\
         \x20 finding.\n\
         \n\
         \x20 ARM SPHERE   (115.76 mm^2 3D; L_min EXACT = 115.76 / 0.47431 = 244.1 mm)\n\
         \x20   spiral         265.3 mm / 23.4 s   x floor 1.087   (measured, prior run)\n\
         \x20   0deg raster    243.4 mm / 22.0 s   x floor 0.997   <<< PREDICTED BELOW 1.0\n\
         \x20   field D = t1   570.9 mm / 49.3 s   x floor 2.339   (measured, prior run)\n\
         \x20   field D = swp  250-300 mm / 22-30 s, x floor 1.03-1.23,  ~24-30 fragments\n\
         \x20     WHY: the cap is UMBILIC, so |V| is CONSTANT and phi is a near-linear ramp\n\
         \x20     along n x d. Levels come out evenly spaced at the surface iso-scallop pitch\n\
         \x20     (Zou's sqrt(8h/(k_s+1/r)) = 0.478 mm at kappa = 0.05), i.e. ~25 passes across\n\
         \x20     a 12 mm cap. THE DECIDING NUMBER IS THE ACHIEVED LEVEL SPACING: predict a\n\
         \x20     median of 0.474-0.480 mm against D = t1's measured 0.333. That pair is what\n\
         \x20     separates 'the direction source was noise' from 'the machinery is broken'.\n\
         \n\
         \x20 ARM WAVY     (81.61 mm^2 3D; L_min PREDICTED 150-170 mm, i.e. BELOW synthesis\n\
         \x20              Section 1's 192.4, because that used the region's TIGHTEST s_max as a\n\
         \x20              constant and this integrates the local one)\n\
         \x20   spiral         209.6 mm / 18.8 s   x floor 1.23-1.40\n\
         \x20   0deg raster    172.0 mm / 16.0 s   x floor 1.01-1.15  <<< the 0.894x anomaly in\n\
         \x20     Section 1's table should DISAPPEAR once the floor is local. If it does, that is\n\
         \x20     a validation of the integrand; if the raster still reads below 1.0 here, the\n\
         \x20     wavy arm is under-covering too and Section 1 under-called it.\n\
         \x20   field D = t1   768.4 mm / 75.0 s   x floor 4.5-5.1\n\
         \x20   field D = swp  180-240 mm / 17-24 s, x floor 1.06-1.60\n\
         \x20     WHY: sin(kx)sin(ky) is UMBILIC at every crest and trough (both principal\n\
         \x20     curvatures A k^2 there), so t1 is noise across a large part of this region too.\n\
         \n\
         \x20 ARM RIBBON   (191.82 mm^2 3D; L_min PREDICTED 380-400 mm)\n\
         \x20   spiral        1400.0 mm / 116.8 s  x floor 3.5-3.7\n\
         \x20   0deg raster    489.9 mm /  62.5 s  x floor 1.2-1.35\n\
         \x20   field D = t1  3365.4 mm / 586.2 s  x floor 8.4-8.9\n\
         \x20   field D = swp  450-750 mm cut, but 150-400 FRAGMENTS and 60-120 s\n\
         \x20     WHY: the spacing law is fixed, so the DISTANCE should collapse toward the\n\
         \x20     floor — but one global level set threads every arm of the ribbon at once, so\n\
         \x20     each level lands as several disjoint curves and the CONNECTION cost explodes.\n\
         \x20     PREDICTION WITH TEETH: on this arm the sweep field wins on cut mm and may\n\
         \x20     still LOSE on time. That is not a failure of the idea — it is precisely the\n\
         \x20     synthesis Section 4.2 argument for putting the field inside MONOTONE CELLS,\n\
         \x20     which this run does not test.\n\
         \n\
         \x20 ARM BAND / SHALLOW  (320.82 mm^2 3D; L_min PREDICTED 650-680 mm — the band is\n\
         \x20              mostly the flat plane between the bumps, where s_max is the flat\n\
         \x20              0.48621, so this floor should land CLOSE to Section 1's 659.8)\n\
         \x20   spiral         NO PATH — NotSimplyConnected {{ boundary_loops: 5 }}\n\
         \x20   0deg raster    722.6 mm /  82.4 s  x floor 1.06-1.11\n\
         \x20   field D = t1   unmeasured; predict badly fragmented and long — a FLAT plane has\n\
         \x20     no principal direction at all, which is this diagnosis in its purest form.\n\
         \x20   field D = swp  700-950 mm, many fragments (the band is 5 components with 4\n\
         \x20     holes, and every one of them fragments a global level set).\n\
         \n\
         \x20 THE ONE FALSIFIER, STATED SO IT CAN FAIL:\n\
         \x20   On EVERY analytic arm, `field D = sweep` must cut LESS than `field D = t1` —\n\
         \x20   by at least 2x on ARM SPHERE, where the degeneracy is exact and total. If it\n\
         \x20   does not, the synthesis's diagnosis (\"the direction source is degenerate, not\n\
         \x20   the Poisson machinery\") is REFUTED, and the F1 arm's losses have to be\n\
         \x20   attributed to the machinery after all.\n\
         ##############################################################################\n"
    );
}

/// **The FIFTH candidate's pre-registration, added 2026-08-31.**
///
/// A SEPARATE block, printed after [`print_preregistration`] and never merged
/// into it. That block is a dated statement made before the 2026-08-30 run;
/// editing its content to absorb a candidate that did not exist then would
/// destroy the only property a pre-registration has. This one is dated in its
/// own header and stands or falls on its own.
///
/// Written from the construction alone (see [`medial_field_candidate`]), before
/// any arm executed.
fn print_medial_preregistration() {
    eprintln!(
        "\n########## PRE-REGISTERED — THE MEDIAL-AXIS CANDIDATE, 2026-08-31 ##########\n\
         \x20 THE PREDICTION IS STATED HERE, BEFORE THE NUMBERS, and it follows from the\n\
         \x20 construction rather than from hope. V = -|V| * grad_hat(EDT), so phi is a\n\
         \x20 reparameterised NEGATIVE distance transform and its level sets are ISO-DISTANCE\n\
         \x20 OFFSETS OF THE BOUNDARY. This row is CONTOUR-PARALLEL MACHINING CARRYING THE\n\
         \x20 ISO-SCALLOP SPACING LAW.\n\
         \n\
         \x20 THE ARITHMETIC THE OPERATOR OFFERED, restated as the mechanism:\n\
         \x20   an L x W arm swept ALONG its axis needs W/s passes of length L; swept ACROSS it\n\
         \x20   needs L/s passes of length W. SAME total distance (L*W/s), 4x fewer PASS ENDS on\n\
         \x20   ARM RIBBON's 10 x 2.5 mm arms (5 vs 21). Pass ends are what links, turns and\n\
         \x20   retracts are made of. SO: FEWER FRAGMENTS AND FEWER LINKS THAN THE D = sweep \
         ARM,\n\
         \x20   WITH CUTTING DISTANCE ROUGHLY UNCHANGED.\n\
         \x20   THE RAIL ON THAT: if the cutting distance moves a LOT, then something OTHER than\n\
         \x20   the direction changed — the spacing basis did — and this instrument must say so\n\
         \x20   instead of banking it as a direction win. Both field rows are checked against\n\
         \x20   the SAME x FLOOR for exactly that reason.\n\
         \n\
         \x20 ARM SPHERE   the region is a DISK, so grad(EDT) is RADIAL and D is TANGENTIAL: the\n\
         \x20   medial row should re-derive CONCENTRIC RINGS — very nearly the conformal \
         spiral's\n\
         \x20   own ring family, without the bridge that makes it one curve. Predict\n\
         \x20   250-290 mm cut, x floor 1.02-1.19, ~25 closed loops => ~25 fragments, and an\n\
         \x20   ACHIEVED LEVEL SPACING median of 0.474-0.480 mm (the same band the sweep row is\n\
         \x20   predicted into, because on an umbilic cap |V| is constant either way).\n\
         \x20   The medial axis of a disk is a single POINT, so the degenerate tier should be\n\
         \x20   tiny (well under 1% of triangles) and concentrated at the centre.\n\
         \n\
         \x20 ARM WAVY     region is a disk again => concentric rings again, but |V| now varies\n\
         \x20   with the ripple. Predict 180-240 mm, x floor 1.06-1.60, 20-40 fragments.\n\
         \n\
         \x20 ARM RIBBON   THE ARM THIS WAS PROPOSED FOR. Offsets of a capsule ARE parallel \
         passes\n\
         \x20   down its length. Arm half-width 1.25 mm = 2.57 stepovers, so ~5 passes across an\n\
         \x20   arm; hub inscribed radius w/sin(22.5deg) = 3.266 mm, so ~6-7 levels reach it.\n\
         \x20   Predict 400-550 mm cut (x floor 1.0-1.4 against an L_min of 380-400) and\n\
         \x20   5-40 FRAGMENTS — against the D = sweep row's predicted 150-400. That fragment\n\
         \x20   gap is the whole prediction.\n\
         \x20   FALSIFIER WITH TEETH, STATED SO IT CAN FAIL: on ARM RIBBON the medial row must\n\
         \x20   produce FEWER fragments AND fewer added links than the D = sweep row, with\n\
         \x20   cutting distance within roughly 1.5x of it. If it fragments MORE, the operator's\n\
         \x20   'along each arm' idea is refuted on the geometry it was proposed for, and\n\
         \x20   FINDINGS.md §0k's 0.686x contour loss is the result that carried after all.\n\
         \n\
         \x20 ARM BAND / SHALLOW   the operator's REAL geometry, and where a global sweep is\n\
         \x20   least defensible. The medial axis of a plane with four holes is the square's own\n\
         \x20   diagonal skeleton blended with four circular ones, so this row's offsets hug the\n\
         \x20   bump rims and the patch edge simultaneously. Predict 700-950 mm (x floor\n\
         \x20   1.05-1.45) and FEWER retracts than the raster wall's, because a closed offset\n\
         \x20   loop around a bump is one fragment where a raster crossing it is two.\n\
         \n\
         \x20 THE HUB — the operator predicted 'a spiral in the center'. BY CONSTRUCTION the\n\
         \x20   field circulates there: EDT has a local MAX at the hub, phi therefore a local\n\
         \x20   MIN, and level sets near a non-degenerate minimum are CLOSED LOOPS encircling \
         it.\n\
         \x20   So the topology the operator described is what the construction produces — but\n\
         \x20   as CONCENTRIC CLOSED LOOPS, not as one connected spiral: nothing in this \
         pipeline\n\
         \x20   joins consecutive levels, so the relinker has to stitch them. PREDICT TWO\n\
         \x20   ARTEFACTS AT THE HUB, both reported either way: (a) the innermost loop shrinks \
         to\n\
         \x20   nothing and leaves a residual UNCUT CORE at the EDT's peak — the classic\n\
         \x20   offset-machining leftover; (b) the BELOW-FLOOR gradient fraction spikes there and\n\
         \x20   along every arm centreline, because those are the medial axis, and the solve\n\
         \x20   interpolates the ridge from V = 0 rather than following a direction.\n\
         \x20   Expected fallback counts: RIBBON below-floor 3-10% of samples (the skeleton is\n\
         \x20   ~1-2 cells wide out of ~24 across an arm), SPHERE/WAVY well under 1%, BAND 2-6%.\n\
         \x20   Expected BOUNDARY SNAPS, which are a re-rasterisation artefact and not a fact\n\
         \x20   about the shape, and should scale with PERIMETER/AREA: RIBBON 3-15% (perimeter\n\
         \x20   ~175 mm over 192 mm^2), SPHERE/WAVY 1-5%, BAND 4-15%. A snap fraction far above\n\
         \x20   its band means the medial grid and the mesh disagree about where the region is,\n\
         \x20   and every direction on the fringe inherits that.\n\
         \x20   A degenerate tier above ~15% anywhere would mean the GRID is too coarse to \
         resolve\n\
         \x20   the shape, not that the shape has a skeleton — that reading is registered here \
         so\n\
         \x20   it cannot be reached after the fact.\n\
         ##############################################################################\n"
    );
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

// ── ARM RIBBON smoke tests ──────────────────────────────────────────────

/// The generator's own preconditions, all MEASURED rather than argued:
/// **one boundary loop, Euler 1**, max edge inside the analytic ceiling, slope
/// inside the stated bound, every concavity larger than the ball, and the arm
/// width inside the brief's 4–8 × stepover band.
///
/// A `plan_spiral` refusal on ARM RIBBON is only a *finding* if all of these
/// hold — otherwise it is a fixture defect wearing a finding's clothes, which
/// is precisely what the FINDINGS_F2 withdrawal was about.
#[test]
fn ribbon_mesh_is_a_narrow_branched_disk_fine_enough_to_measure() {
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let max_gradient = ribbon_max_gradient();
    let max_slope_deg = max_gradient.atan().to_degrees();
    let max_curvature = ribbon_max_curvature();

    // -- the two analytic bounds, before any mesh is built --
    assert!(
        max_slope_deg < 30.0,
        "ribbon slope bound {max_slope_deg:.3} deg must stay well under 30; this arm tests \
         BRANCHING, not steepness"
    );
    assert!(
        1.0 / max_curvature >= 2.0 * BALL_RADIUS_MM,
        "ribbon R_min {:.4} mm must be >= 2*K_c = {:.4} mm, or the ball bridges a concavity and \
         the arm stalls for a GEOMETRIC reason that has nothing to do with branching",
        1.0 / max_curvature,
        2.0 * BALL_RADIUS_MM
    );

    // -- the width the brief specifies --
    let width = 2.0 * RIBBON_HALF_WIDTH_MM;
    let width_in_stepovers = width / stepover;
    assert!(
        (4.0..=8.0).contains(&width_in_stepovers),
        "ribbon arm width {width:.3} mm = {width_in_stepovers:.3} stepovers, outside the brief's \
         4-8x band"
    );

    let cells = heightfield_cells(RIBBON_PATCH_MM, max_gradient, ANALYTIC_MAX_EDGE_MM);
    let mesh = heightfield_mesh(RIBBON_PATCH_MM, cells, ribbon_height);
    // The G-CELLHOLE repair is part of the fixture, not part of the assertion:
    // `ribbon_digitisation_invents_holes_g_cellhole` measures the artefact and
    // bounds it; this test checks what the ARM actually plans on.
    let (mask, _fill) = fill_mask_holes(&ribbon_cell_mask(cells), RIBBON_PATCH_MM, cells);
    let triangles = cells_to_triangles(&mask, cells);
    assert!(
        triangles.len() > 4_000,
        "ribbon region is only {} triangles — too small for any downstream number to mean \
         anything",
        triangles.len()
    );

    let census = mesh_census(&mesh, &triangles);
    assert_eq!(
        census.boundary_loops, 1,
        "the ribbon must be SIMPLY CONNECTED — one boundary loop, no holes. That is the whole \
         design constraint of this arm: it isolates DISTORTION from TOPOLOGY, and a hole here \
         would need the slit map ARM BAND is about."
    );
    assert_eq!(
        census.euler, 1,
        "the ribbon must be a topological DISK (V - E + F = 1)"
    );
    assert!(
        census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM,
        "ribbon max 3D edge {:.5} mm exceeds the {ANALYTIC_MAX_EDGE_MM} mm ceiling (bar: \
         stepover/3 = {:.5} mm). Facets coarser than the measurand is the ENTIRE content of the \
         FINDINGS_F2 withdrawal.",
        census.edge_max_mm,
        stepover / 3.0
    );
    assert!(
        census.min_normal_z > 0.0,
        "every ribbon face must be up-facing; worst normal.z {:.6}",
        census.min_normal_z
    );
    let measured_slope = census.min_normal_z.acos().to_degrees();
    assert!(
        measured_slope <= max_slope_deg + 1e-6,
        "measured worst slope {measured_slope:.4} deg exceeds the analytic bound \
         {max_slope_deg:.4} deg — the bound is wrong, not the mesh"
    );

    // -- selection hygiene must be all but a no-op on the repaired selection --
    //
    // Not asserted as EXACTLY zero, unlike the sphere and wavy arms. Those two
    // are a whole mesh and a polyomino disk, where any hygiene at all would
    // mean the generator is wrong. A capsule union has re-entrant notches
    // between adjacent arms (apex at w/sin(22.5°) = 3.266 mm from the centre),
    // and the digitisation of a wedge apex thinner than a cell is exactly what
    // G-CELLHOLE measured — so this tolerates one stray cell in a thousand and
    // reports the number rather than claiming a no-op it has not earned.
    let (cleaned, _cleanup) = clean_selection(&mesh, &triangles);
    let dropped = triangles.len() - cleaned.len();
    assert!(
        dropped * 1_000 <= triangles.len(),
        "clean_selection shaved {dropped} of {} ribbon triangles ({:.3}%). The arms are {:.3} mm \
         wide against a {:.5} mm cell — 24 cells across — so a whole-cell digitisation of a \
         capsule union should need essentially no hygiene, and this much means the digitisation \
         is producing selection artefacts every downstream number would inherit.",
        triangles.len(),
        100.0 * dropped as f64 / triangles.len().max(1) as f64,
        2.0 * RIBBON_HALF_WIDTH_MM,
        RIBBON_PATCH_MM / cells as f64
    );
}

/// **G-CELLHOLE, pinned as a measurement rather than as a count.**
///
/// A region that is provably simply connected in the continuum acquires holes
/// when it is laid onto a grid. This test does NOT assert how many — that
/// would turn a falsifiable observation into a tautology, and the count is a
/// function of the arm phase and the cell size, both of which may legitimately
/// move. What it pins is the artefact's **character**, which is what makes the
/// repair legitimate:
///
/// * every pocket is smaller than the cutter's own footprint `π·K_c²`, so
///   filling it machines nothing the region did not already claim;
/// * the whole repair is under 0.5 % of the region's area, so it is a repair
///   and not a redesign;
/// * after it, the selection is a topological disk — which is the property the
///   arm's entire purpose depends on.
#[test]
fn ribbon_digitisation_invents_holes_g_cellhole() {
    let gradient = ribbon_max_gradient();
    let cells = heightfield_cells(RIBBON_PATCH_MM, gradient, ANALYTIC_MAX_EDGE_MM);
    let cell = RIBBON_PATCH_MM / cells as f64;
    let raw = ribbon_cell_mask(cells);
    let (filled, fill) = fill_mask_holes(&raw, RIBBON_PATCH_MM, cells);

    let region_area = fill.cells_before as f64 * cell * cell;
    let tool_footprint = PI * BALL_RADIUS_MM * BALL_RADIUS_MM;
    for hole in &fill.holes {
        assert!(
            hole.area_mm2 < tool_footprint,
            "a filled pocket of {:.5} mm² at r = {:.4} mm is larger than the cutter's own \
             footprint ({tool_footprint:.4} mm²). Filling THAT is not a digitisation repair, it \
             is machining ground the region never claimed — treat it as a fixture defect, not as \
             G-CELLHOLE.",
            hole.area_mm2,
            hole.centroid_radius_mm
        );
    }
    assert!(
        fill.area_filled_mm2 < 0.005 * region_area,
        "the G-CELLHOLE repair filled {:.5} mm² of a {region_area:.3} mm² region ({:.3}%). Over \
         0.5% it stops being a repair and starts being a different fixture.",
        fill.area_filled_mm2,
        100.0 * fill.area_filled_mm2 / region_area.max(1e-12)
    );

    // The repair must actually work, and it must be the ONLY thing needed.
    let mesh = heightfield_mesh(RIBBON_PATCH_MM, cells, ribbon_height);
    let census = mesh_census(&mesh, &cells_to_triangles(&filled, cells));
    assert_eq!(
        census.boundary_loops, 1,
        "after fill_mask_holes the ribbon must have exactly ONE boundary loop"
    );
    assert_eq!(
        census.euler, 1,
        "after fill_mask_holes the ribbon must be a topological disk"
    );

    // And filling must be idempotent: a second pass finds nothing.
    let (_again, second) = fill_mask_holes(&filled, RIBBON_PATCH_MM, cells);
    assert!(
        second.holes.is_empty(),
        "fill_mask_holes is not idempotent — a second pass found {} more pocket(s), which means \
         the first pass created one",
        second.holes.len()
    );
}

/// The fill primitive itself, on masks whose answer is known by inspection:
/// a solid block has no pocket, a ring has one, and a pocket that touches the
/// grid border is OUTSIDE and must not be filled.
#[test]
fn fill_mask_holes_closes_only_enclosed_pockets() {
    let cells = 9usize;
    let size = 9.0f64;

    let solid = vec![true; cells * cells];
    let (out, report) = fill_mask_holes(&solid, size, cells);
    assert!(report.holes.is_empty(), "a solid block has no pocket");
    assert_eq!(out, solid);

    // A ring: solid with one interior cell cleared.
    let mut ring = vec![true; cells * cells];
    ring[4 * cells + 4] = false;
    let (out, report) = fill_mask_holes(&ring, size, cells);
    assert_eq!(report.holes.len(), 1, "a ring has exactly one pocket");
    assert_eq!(report.cells_filled, 1);
    assert!(out.iter().all(|&v| v), "the pocket must be filled");
    let hole = report.holes.first().copied().expect("one hole");
    assert!(
        hole.centroid_radius_mm.abs() < 1e-9,
        "the centre cell of a 9x9 patch is at the origin, got r = {:.6}",
        hole.centroid_radius_mm
    );

    // A notch open to the border is NOT a pocket, however deep.
    let mut notch = vec![true; cells * cells];
    for row in 0..5 {
        notch[row * cells + 4] = false;
    }
    let (out, report) = fill_mask_holes(&notch, size, cells);
    assert!(
        report.holes.is_empty(),
        "a channel reaching the grid border is the OUTSIDE, not a pocket; filling it would close \
         a genuine concavity"
    );
    assert_eq!(out, notch);

    // The 4-connectivity choice, stated as a test: a diagonal pair of cleared
    // cells is 8-connected but not 4-connected, so BOTH are enclosed. That is
    // the exact shape G-CELLHOLE found on the ribbon.
    let mut diagonal = vec![true; cells * cells];
    diagonal[3 * cells + 3] = false;
    diagonal[4 * cells + 4] = false;
    let (_out, report) = fill_mask_holes(&diagonal, size, cells);
    assert_eq!(
        report.cells_filled, 2,
        "two diagonally-touching cleared cells are 4-connected to nothing, so both are enclosed \
         — this is the connectivity `region_topology` uses and the reason G-CELLHOLE exists"
    );
    assert_eq!(
        report.holes.len(),
        2,
        "and they are TWO pockets under 4-connectivity, not one"
    );
}

/// The arm's own validity gate: the ribbon must **force a 0° raster to break
/// into many disjoint runs**, or the arm proves nothing.
///
/// This is the cheap geometric half of [`print_fragmentation_gate`], asserted
/// here so a parameter change that quietly turns the ribbon back into a blob
/// fails in the fast suite rather than in a 40-minute evidence run.
#[test]
fn ribbon_forces_a_zero_degree_raster_to_fragment() {
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let gradient = ribbon_max_gradient();
    let cells = heightfield_cells(RIBBON_PATCH_MM, gradient, ANALYTIC_MAX_EDGE_MM);
    let (mask, _fill) = fill_mask_holes(&ribbon_cell_mask(cells), RIBBON_PATCH_MM, cells);
    let polygons = mask_polygons(&mask, RIBBON_PATCH_MM, cells);
    // `detect_containment` returns largest-area first, which is the polygon
    // the arm plans on. It must carry essentially all the area and NO HOLES —
    // no hole is what makes this the DISTORTION arm rather than the topology
    // arm, and the topology arm is ARM BAND.
    let Some(main) = polygons.first() else {
        panic!("the ribbon mask extracted no polygon at all")
    };
    assert!(
        main.holes.is_empty(),
        "the ribbon must have NO HOLES after the G-CELLHOLE repair: it is a union of capsules that \
         all contain the origin, so the union is star-shaped about it and therefore simply \
         connected in the continuum. {} hole(s) here means `fill_mask_holes` did not reach a \
         pocket the digitisation invented.",
        main.holes.len()
    );
    let total_area: f64 = polygons.iter().map(Polygon2::area).sum();
    assert!(
        main.area() >= 0.99 * total_area,
        "the largest ribbon polygon holds only {:.3} of {total_area:.3} mm²; the rest is stray \
         islands the digitisation left behind",
        main.area()
    );

    let census = scanline_census(&polygons, stepover);
    assert!(
        census.max_runs_in_row >= 3,
        "no scan row over the ribbon breaks into 3+ runs (max {}); a 0deg raster would barely \
         lift and this arm would repeat the SPHERE/WAVY mistake",
        census.max_runs_in_row
    );
    let fraction = census.rows_multi_run as f64 / census.rows_with_material.max(1) as f64;
    assert!(
        fraction >= MIN_MULTI_RUN_ROW_FRACTION,
        "only {:.1}% of active scan rows break into more than one run (bar {:.1}%): {} of {}. \
         The fixture failed to reproduce the target geometry class.",
        100.0 * fraction,
        100.0 * MIN_MULTI_RUN_ROW_FRACTION,
        census.rows_multi_run,
        census.rows_with_material
    );
    // >= 1.5 inside-runs per active row. Derived from the arm geometry before
    // the fixture was written — |y| under 2.31 mm gives 1 run (all eight arms
    // merged near the centre), 2.31-3.27 gives 3, 3.27-5.08 gives 4, and
    // 5.08-10.49 gives 2 — and then MEASURED on the built mask at 92 runs over
    // 42 active rows, 2.19 per row, worst row 4, 32 of 42 rows multi-run.
    // Wanaka region 1, for scale, averages ~1.8 crossings per row and still
    // pays 97 kept retracts.
    assert!(
        2 * census.total_runs >= 3 * census.rows_with_material,
        "total inside-runs {} over {} active rows is under 1.5 per row; the fixture is not \
         fragmenting a raster the way the target class does (Wanaka region 1: \
         {WANAKA_R1_RASTER_FRAGMENTS} fragments / {WANAKA_R1_RASTER_RETRACTS} retracts)",
        census.total_runs,
        census.rows_with_material
    );
}

/// The scan-line run counter itself, on shapes whose answer is known by
/// inspection: one square is one run, two separated squares are two, and a
/// square with a hole through the middle of it is two.
#[test]
fn scanline_runs_counts_disjoint_intervals() {
    let square = |x0: f64, x1: f64, y0: f64, y1: f64| {
        Polygon2::new(vec![
            P2::new(x0, y0),
            P2::new(x1, y0),
            P2::new(x1, y1),
            P2::new(x0, y1),
        ])
    };
    let one = [square(0.0, 10.0, 0.0, 10.0)];
    assert_eq!(scanline_runs(&one, 5.0), 1);
    assert_eq!(scanline_runs(&one, 20.0), 0);

    let two = [square(0.0, 4.0, 0.0, 10.0), square(6.0, 10.0, 0.0, 10.0)];
    assert_eq!(scanline_runs(&two, 5.0), 2);

    let mut holed = square(0.0, 10.0, 0.0, 10.0);
    holed.holes.push(vec![
        P2::new(4.0, 4.0),
        P2::new(4.0, 6.0),
        P2::new(6.0, 6.0),
        P2::new(6.0, 4.0),
    ]);
    let holed = [holed];
    assert_eq!(
        scanline_runs(&holed, 5.0),
        2,
        "a row through the hole must read TWO runs — that is the raster lifting"
    );
    assert_eq!(
        scanline_runs(&holed, 2.0),
        1,
        "a row below the hole must read ONE run"
    );
}

// ── ARM BAND smoke tests ────────────────────────────────────────────────

/// The band predicate is `acos(|n_z|)` against half-open bounds, checked on a
/// hand-built fixture whose normals are known exactly.
#[test]
fn band_predicate_matches_acos_nz_bounds() {
    // A flat +Z normal is 0°; a 45° ramp normal is (0, -1, 1)/√2; a vertical
    // wall normal is (0, 1, 0).
    let flat = rs_cam_core::geo::V3::new(0.0, 0.0, 1.0);
    let ramp = rs_cam_core::geo::V3::new(0.0, -1.0, 1.0);
    let wall = rs_cam_core::geo::V3::new(0.0, 1.0, 0.0);
    assert!((face_slope_deg(flat) - 0.0).abs() < 1e-9);
    assert!((face_slope_deg(ramp) - 45.0).abs() < 1e-9);
    assert!((face_slope_deg(wall) - 90.0).abs() < 1e-9);

    // Sign-flipped normals must read the SAME slope: the module forces +Z.
    let flipped = rs_cam_core::geo::V3::new(0.0, 1.0, -1.0);
    assert!(
        (face_slope_deg(flipped) - 45.0).abs() < 1e-9,
        "a downward-wound face must read 45 deg, not 135 — build_region_mesh forces +Z and this \
         predicate must agree with it"
    );

    // Half-open [lo, hi): the lower bound is IN, the upper bound is OUT, which
    // is the shape finish_planner's three-way label uses.
    assert!(slope_in_band(0.0, 0.0, 10.0));
    assert!(slope_in_band(9.999, 0.0, 10.0));
    assert!(!slope_in_band(10.0, 0.0, 10.0));
    assert!(slope_in_band(10.0, 10.0, 20.0));
    assert!(!slope_in_band(f64::NAN, 0.0, 90.0));
    // A face whose normal is degenerate is NOT in any band, rather than
    // defaulting into the shallow one.
    assert!(face_slope_deg(rs_cam_core::geo::V3::new(0.0, 0.0, 0.0)).is_nan());
}

/// The component counter distinguishes two obviously-separate patches from
/// one, and does it by EDGE adjacency so a corner touch is not a connection.
#[test]
fn edge_components_separates_two_patches() {
    let mesh = clean_grid_mesh();
    let all: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let one = edge_components(&mesh, &all);
    assert_eq!(one.len(), 1, "a clean grid patch is ONE component");
    assert_eq!(one.first().map(Vec::len), Some(all.len()));

    // The 3x3-vertex grid's cells are (row, col) in {0,1}²; taking the two
    // DIAGONAL cells leaves two patches that touch only at the centre vertex.
    let mut diagonal: Vec<u32> = Vec::new();
    diagonal.extend_from_slice(&cell_triangles(2, 0, 0));
    diagonal.extend_from_slice(&cell_triangles(2, 1, 1));
    diagonal.sort_unstable();
    let two = edge_components(&mesh, &diagonal);
    assert_eq!(
        two.len(),
        2,
        "two diagonally-adjacent cells touch at ONE VERTEX and must read as TWO edge-connected \
         components — treating them as connected is exactly the bowtie region_topology refuses"
    );
    assert_eq!(two.iter().map(Vec::len).sum::<usize>(), diagonal.len());
}

/// The band fixture's own preconditions, and the closed-form band radii the
/// arm predicts its topology from.
#[test]
fn band_mesh_is_fine_enough_and_the_ball_fits_every_concavity() {
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let max_gradient = band_max_gradient();
    let max_curvature = band_max_curvature();
    assert!(
        1.0 / max_curvature >= 2.0 * BALL_RADIUS_MM,
        "band R_min {:.4} mm must be >= 2*K_c = {:.4} mm",
        1.0 / max_curvature,
        2.0 * BALL_RADIUS_MM
    );
    const {
        assert!(
            BAND_BUMP_PITCH_MM > 2.0 * BAND_BUMP_RADIUS_MM,
            "the bump supports must be DISJOINT, or the single-bump slope and \
             curvature bounds stop being exact"
        );
    }
    assert!(
        max_gradient.atan().to_degrees() > BAND_STEEP_MIN_DEG,
        "the surface must actually REACH the steep band, or there is nothing to band"
    );

    // The four band radii the arm's prediction is built from, in order.
    let inner_shallow = band_radius_at_slope(BAND_SHALLOW_MAX_DEG, false).expect("inner shallow");
    let inner_steep = band_radius_at_slope(BAND_STEEP_MIN_DEG, false).expect("inner steep");
    let outer_steep = band_radius_at_slope(BAND_STEEP_MIN_DEG, true).expect("outer steep");
    let outer_shallow = band_radius_at_slope(BAND_SHALLOW_MAX_DEG, true).expect("outer shallow");
    assert!(
        inner_shallow < inner_steep && inner_steep < outer_steep && outer_steep < outer_shallow,
        "band radii must nest: {inner_shallow:.4} < {inner_steep:.4} < {outer_steep:.4} < \
         {outer_shallow:.4}"
    );
    assert!(
        outer_shallow < BAND_BUMP_RADIUS_MM,
        "the outer shallow radius {outer_shallow:.4} must sit INSIDE the bump support \
         {BAND_BUMP_RADIUS_MM}, so the shallow band is continuous with the surrounding plane"
    );

    let cells = heightfield_cells(BAND_PATCH_MM, max_gradient, ANALYTIC_MAX_EDGE_MM);
    let mesh = heightfield_mesh(BAND_PATCH_MM, cells, band_height);
    let all: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
    let census = mesh_census(&mesh, &all);
    assert!(
        census.edge_max_mm <= ANALYTIC_MAX_EDGE_MM,
        "band max 3D edge {:.5} mm exceeds the {ANALYTIC_MAX_EDGE_MM} mm ceiling (bar: \
         stepover/3 = {:.5} mm)",
        census.edge_max_mm,
        stepover / 3.0
    );
    assert!(
        census.min_normal_z > 0.0,
        "every band-fixture face must be up-facing; worst normal.z {:.6}",
        census.min_normal_z
    );
}

/// Every one of the three slope bands must be **non-empty** on this fixture,
/// or the topology census has nothing to say.
///
/// Deliberately does NOT assert the predicted component counts. The arm states
/// its topology prediction in the evidence run and lets the measurement
/// confirm or refute it; pinning the prediction here would turn a falsifiable
/// claim into a tautology.
#[test]
fn band_selection_populates_all_three_bands() {
    let gradient = band_max_gradient();
    let cells = heightfield_cells(BAND_PATCH_MM, gradient, ANALYTIC_MAX_EDGE_MM);
    let mesh = heightfield_mesh(BAND_PATCH_MM, cells, band_height);
    // The three bands must also PARTITION nothing away silently: a cell
    // straddling a threshold (its two triangles landing in different bands)
    // belongs to none under the whole-cell rule, and that loss must stay small
    // enough not to change any band's topology.
    let mut total = 0usize;
    for (name, lo, hi) in [
        ("SHALLOW", 0.0, BAND_SHALLOW_MAX_DEG),
        ("MIDSTEEP", BAND_SHALLOW_MAX_DEG, BAND_STEEP_MIN_DEG),
        ("VERYSTEEP", BAND_STEEP_MIN_DEG, 180.0),
    ] {
        let mask = band_cell_mask(&mesh, cells, lo, hi);
        let triangles = cells_to_triangles(&mask, cells);
        assert!(
            triangles.len() > 100,
            "band {name} [{lo}, {hi}) holds only {} triangles on this fixture",
            triangles.len()
        );
        total += triangles.len();
    }
    let straddling = mesh.triangles.len() - total;
    // The threshold contours are 16 circles (four radii on each of four
    // bumps), ~201 mm of arc; a one-cell-wide straddle ring along them is
    // ~4% of the patch. That ring is removed from BOTH adjacent bands, so each
    // band shrinks by half a cell at its edges and no band's TOPOLOGY changes
    // — an annulus stays an annulus. Ten per cent is where that stops being
    // true and the thinnest band (0.46 mm, ~4.7 cells) starts breaking up.
    assert!(
        straddling * 10 < mesh.triangles.len(),
        "{straddling} of {} triangles ({:.2}%) fall in NO band because their cell straddles a \
         threshold — over 10%, which is enough to break the thinnest band into pieces and \
         report a topology that is a digitisation artefact",
        mesh.triangles.len(),
        100.0 * straddling as f64 / mesh.triangles.len().max(1) as f64
    );
}

/// `mask_polygons` must nest an enclosed loop as a HOLE, not emit it as a
/// separate polygon. ARM BAND's headline claim is about holes, so the
/// extraction that produces them is pinned here.
#[test]
fn mask_polygons_nests_a_hole() {
    let cells = 21usize;
    let size = 21.0f64;
    let mut mask = vec![true; cells * cells];
    // Punch a 3x3 hole in the middle.
    for row in 9..12 {
        for col in 9..12 {
            mask[row * cells + col] = false;
        }
    }
    let polygons = mask_polygons(&mask, size, cells);
    assert_eq!(polygons.len(), 1, "one filled square is one polygon");
    assert_eq!(
        polygons.first().map(|p| p.holes.len()),
        Some(1),
        "the punched square must come back as a HOLE of the outer polygon, not as a second \
         polygon — detect_containment is what makes ARM BAND's hole count readable"
    );
    let area = polygons.first().map_or(0.0, Polygon2::area);
    assert!(
        area > 0.0 && area < size * size,
        "the holed polygon's area {area:.3} must be positive and below the solid square's \
         {:.3}",
        size * size
    );
}

// The census must be able to tell a disk from an annulus, or every
// "one boundary loop" assertion above is vacuous.
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

/// The comparison SVGs' whole claim is that the colour under an operator's eye
/// means what the caption says. That claim is [`classify_move`], so it is
/// pinned over **every** `MoveIntent` in both kinematic classes.
///
/// The `Retract`-tagged **Linear** row is the interesting one: CLAUDE.md
/// records that branch as unreachable from every shipped generator (a census
/// sentry pins it), so it must land in [`MoveClass::Other`] — the magenta
/// ambiguity colour — and NOT be quietly folded into the air class it
/// superficially resembles.
#[test]
fn classify_move_maps_every_intent_in_both_kinematic_classes() {
    use rs_cam_core::toolpath::MoveType;

    let every_intent = [
        MoveIntent::Drilling,
        MoveIntent::EntryPlunge,
        MoveIntent::ClearingCut,
        MoveIntent::FinishingCut,
        MoveIntent::EntryHelix,
        MoveIntent::EntryRamp,
        MoveIntent::Linking,
        MoveIntent::Retract,
        MoveIntent::LeadIn,
        MoveIntent::LeadOut,
        MoveIntent::Unknown,
    ];
    // EVERY rapid is air, whatever it claims to be doing.
    for intent in every_intent {
        let rapid = Move {
            target: P3::new(0.0, 0.0, 0.0),
            move_type: MoveType::Rapid,
            intent,
        };
        assert_eq!(
            classify_move(&rapid),
            MoveClass::Air,
            "a rapid tagged {intent:?} must draw as AIR"
        );
    }

    let linear = |intent: MoveIntent| Move {
        target: P3::new(0.0, 0.0, 0.0),
        move_type: MoveType::Linear { feed_rate: 500.0 },
        intent,
    };
    for (intent, expected) in [
        (MoveIntent::FinishingCut, MoveClass::Cut),
        (MoveIntent::ClearingCut, MoveClass::Cut),
        (MoveIntent::LeadIn, MoveClass::Cut),
        (MoveIntent::LeadOut, MoveClass::Cut),
        (MoveIntent::Linking, MoveClass::Link),
        (MoveIntent::EntryPlunge, MoveClass::Air),
        (MoveIntent::EntryHelix, MoveClass::Air),
        (MoveIntent::EntryRamp, MoveClass::Air),
        (MoveIntent::Drilling, MoveClass::Other),
        (MoveIntent::Retract, MoveClass::Other),
        (MoveIntent::Unknown, MoveClass::Other),
    ] {
        assert_eq!(
            classify_move(&linear(intent)),
            expected,
            "a feed tagged {intent:?} must draw as {expected:?}"
        );
    }

    // The four indices are distinct, so no two classes share a `d` slot.
    let indices: BTreeSet<usize> = MoveClass::ALL.iter().map(|c| c.index()).collect();
    assert_eq!(indices.len(), 4, "MoveClass::index must be injective");
}

/// [`draw_toolpath`] coalesces a same-class run into ONE polyline and starts a
/// new one at every class change, and its lift markers are the retracts.
///
/// The `M`-count is the assertion that matters: without coalescing a
/// 5,500-move spiral would emit 5,500 single-segment subpaths, and the figure
/// would be unreadable long before it was wrong.
#[test]
fn draw_toolpath_coalesces_runs_and_marks_lifts() {
    let line_a = vec![
        P3::new(0.0, 0.0, -1.0),
        P3::new(1.0, 0.0, -1.0),
        P3::new(2.0, 0.0, -1.0),
    ];
    let line_b = vec![P3::new(5.0, 0.0, -1.0), P3::new(6.0, 0.0, -1.0)];
    let tp = polylines_to_toolpath(&[line_a, line_b], FEED_MM_MIN, PLUNGE_MM_MIN, 12.0);
    let drawing = draw_toolpath(&tp);

    // Two fragments: Linking rapid, plunge, 2 cuts, Retract, Linking rapid,
    // plunge, 1 cut, Retract.
    assert_eq!(
        drawing.counts[MoveClass::Cut.index()],
        3,
        "three FinishingCut feeds"
    );
    assert_eq!(
        drawing.counts[MoveClass::Link.index()],
        0,
        "polylines_to_toolpath adds no surface links — only the relinker does"
    );
    assert_eq!(
        drawing.counts[MoveClass::Other.index()],
        0,
        "nothing here is unclassified"
    );
    assert_eq!(
        drawing.lifts.len(),
        2,
        "one lift per Retract: the mid-path one and the closing one"
    );

    // The first cut run is TWO segments in ONE subpath; the second is one
    // segment in its own. So exactly two `M`s in the cut layer.
    let cut_moves = drawing.d[MoveClass::Cut.index()].matches(" M ").count();
    assert_eq!(
        cut_moves,
        2,
        "two cut runs must coalesce into two subpaths, not four: {}",
        drawing.d[MoveClass::Cut.index()]
    );
    assert_eq!(
        drawing.d[MoveClass::Cut.index()].matches(" L ").count(),
        3,
        "three cut segments in total"
    );

    let [x0, y0, x1, y1] = drawing.bbox.expect("a non-empty path has an extent");
    assert!(
        x0 <= 0.0 && y0 <= 0.0 && x1 >= 6.0 && y1 >= 0.0,
        "extent {x0:.3},{y0:.3}..{x1:.3},{y1:.3} must cover every move target"
    );
    assert!(draw_toolpath(&Toolpath::new()).bbox.is_none());
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

// ── smoke tests for the 2026-08-30 additions ────────────────────────────
//
// The floor and the sweep field are both DERIVATIONS, and a derivation that is
// only exercised by a `--ignored` evidence run is a derivation nobody checks.
// Each test below pins exactly one choice that could be silently inverted.

/// A parabolic cylinder `z = −x²/2`: a convex ridge running along `+Y`, with
/// curvature `+1 /mm` across it (`X`) and `0` along it (`Y`). Written as a jet
/// rather than a mesh because the two things under test —
/// [`AnalyticSurface::normal_curvature`] and [`sweep_target_axis`] — are both
/// point functions.
fn ridge_jet(x: f64, _y: f64) -> SurfaceJet {
    SurfaceJet {
        fx: -x,
        fy: 0.0,
        fxx: -1.0,
        fxy: 0.0,
        fyy: 0.0,
    }
}

/// **The orientation pin.** `k_s` must be the curvature ACROSS the passes.
///
/// On a ridge running along `+Y`, sweeping along `+Y` means the passes run down
/// the flat generator and step across the curved section. The correct `k_s` is
/// therefore the ACROSS-ridge value `+1`, which tightens the stepover; reading
/// it along `d` would give `0` and open the stepover to the flat-surface value
/// on the one axis that is not flat.
///
/// The brief that commissioned this experiment says "perpendicular to the pass,
/// i.e. along `d`'s in-plane projection" — two different directions, and the
/// second clause is a slip. `direction_field::build_target_field` settles it:
/// it evaluates `normal_curvature` at `n.cross(&d)`, not at `d`.
#[test]
fn k_s_is_read_across_the_passes_not_along_them() {
    let ridge = AnalyticSurface {
        name: "test ridge z = -x^2/2",
        jet: ridge_jet,
    };
    let n = V3::new(0.0, 0.0, 1.0);
    let along_x = ridge.normal_curvature(0.0, 0.0, V3::new(1.0, 0.0, 0.0));
    let along_y = ridge.normal_curvature(0.0, 0.0, V3::new(0.0, 1.0, 0.0));
    assert!(
        (along_x - 1.0).abs() < 1e-12,
        "across the ridge must read +1 /mm convex, got {along_x}"
    );
    assert!(
        along_y.abs() < 1e-12,
        "along the ridge must read 0, got {along_y}"
    );

    // Sweeping ALONG the ridge (+Y): the stepover axis is ±X, so k_s is the
    // across-ridge 1.0 — the tight one.
    let axis =
        sweep_target_axis(n, V3::new(0.0, 1.0, 0.0)).expect("a flat facet cannot degenerate");
    assert!(
        axis.x.abs() > 1.0 - 1e-12 && axis.y.abs() < 1e-12,
        "n x (+Y) must be ±X, got {axis:?}"
    );
    let k_s = ridge.normal_curvature(0.0, 0.0, axis);
    assert!(
        (k_s - 1.0).abs() < 1e-12,
        "passes ALONG the ridge must read the ACROSS-ridge curvature 1.0, not 0.0. This is the \
         inversion the whole derivation turns on; got {k_s}"
    );

    // And the converse, so the test cannot pass by reading a constant.
    let across =
        sweep_target_axis(n, V3::new(1.0, 0.0, 0.0)).expect("a flat facet cannot degenerate");
    let k_s_across = ridge.normal_curvature(0.0, 0.0, across);
    assert!(
        k_s_across.abs() < 1e-12,
        "passes ACROSS the ridge step along the flat generator, so k_s must be 0; got {k_s_across}"
    );
}

/// The convex-positive sign convention, on the one surface whose answer is
/// known in closed form from three independent directions.
///
/// A sphere cap of radius `R_s` is UMBILIC: every direction is principal and
/// every normal curvature equals `+1/R_s` in this file's convention. If the
/// sign were the textbook one the floor would use a CONCAVE effective radius
/// and `s_max` would come out too wide, silently.
/// Tolerance for the closed-form curvature assertions below.
///
/// Not machine epsilon, and the reason is worth recording. On an UMBILIC point
/// `H² − K` is exactly zero in real arithmetic, so `√(H² − K)` evaluates the
/// square root of pure cancellation noise: a relative `1e-16` on an `H²` of
/// order `2.5e-3` gives a residue near `2.5e-19`, whose square root is `5e-10`
/// — a *billion* times larger than the noise it came from. So the split between
/// `κ_min` and `κ_max` at an umbilic point is legitimately ~`1e-9`, and a
/// `1e-9` bar would be flaky by construction. `1e-7` is still 2 ppm of the
/// `0.05 /mm` being asserted, and four orders below any sign or convention
/// error this test exists to catch.
const CURVATURE_TOL: f64 = 1e-7;

#[test]
fn analytic_curvature_is_convex_positive_and_umbilic_on_the_sphere() {
    let expected = 1.0 / SPHERE_RADIUS_MM;
    for (x, y) in [(0.0, 0.0), (3.0, 0.0), (0.0, -4.5), (2.5, 2.5)] {
        let (k_min, k_max) = SPHERE_SURFACE.principal_curvatures(x, y);
        assert!(
            (k_min - expected).abs() < CURVATURE_TOL && (k_max - expected).abs() < CURVATURE_TOL,
            "the cap is umbilic: both principal curvatures must be +1/R_s = {expected} at \
             ({x}, {y}); got {k_min} / {k_max}"
        );
    }
    // A tangent direction at the pole, where the tangent plane is z = 0.
    let at_pole = SPHERE_SURFACE.normal_curvature(0.0, 0.0, V3::new(1.0, 0.0, 0.0));
    assert!(
        (at_pole - expected).abs() < CURVATURE_TOL,
        "normal curvature at the pole must agree with the principal pair; got {at_pole}"
    );

    // The wavy patch's crest, whose closed form wavy_patch_mesh states: both
    // principal curvatures are A·k² at a critical point, convex at a peak.
    let k = TAU / WAVY_WAVELENGTH_MM;
    let crest = 0.25 * WAVY_WAVELENGTH_MM;
    let (w_min, w_max) = WAVY_SURFACE.principal_curvatures(crest, crest);
    let peak = WAVY_AMPLITUDE_MM * k * k;
    assert!(
        (w_min - peak).abs() < CURVATURE_TOL && (w_max - peak).abs() < CURVATURE_TOL,
        "wavy crest must read +A k^2 = {peak} in both directions; got {w_min} / {w_max}"
    );

    // A bump apex on the band fixture: -A pi^2 / (2 R^2) in the textbook sign,
    // so +that here, isotropic.
    let (bx, by) = band_bump_centres()[0];
    let (b_min, b_max) = BAND_SURFACE.principal_curvatures(bx, by);
    let apex = band_max_curvature();
    assert!(
        (b_min - apex).abs() < CURVATURE_TOL && (b_max - apex).abs() < CURVATURE_TOL,
        "band bump apex must read +{apex} in both directions; got {b_min} / {b_max}"
    );
}

/// **The floor's arithmetic, against the one arm where it is exact.**
///
/// The sphere's curvature is constant, so `L_min = area / s_max` with a single
/// `s_max` — and that is precisely the number the synthesis §1 table computed
/// by hand (115.76 mm² / 0.47431 mm = 244.1 mm). This test asserts the
/// per-triangle integral reproduces the closed form, which is what licenses
/// reading the integral on the three arms where no closed form exists.
#[test]
fn the_sphere_floor_reproduces_its_closed_form() {
    let mesh = sphere_cap_mesh(
        SPHERE_RADIUS_MM,
        SPHERE_CAP_RADIUS_MM,
        SPHERE_CAP_RINGS,
        SPHERE_CAP_SECTORS,
    );
    let region = sphere_cap_region(&mesh);
    let floor = region_floor(&mesh, &region, SPHERE_SURFACE);
    let s_max = scallop_math::stepover_from_scallop_curved(
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
        1.0 / SPHERE_RADIUS_MM,
    );
    let closed_form = floor.area_mm2 / s_max;
    assert_eq!(
        floor.degenerate, 0,
        "no triangle on a convex cap can have a non-positive s_max"
    );
    assert!(
        (floor.l_min_mm - closed_form).abs() / closed_form < 1e-6,
        "the integral must reproduce area / s_max on a constant-curvature surface: \
         {} vs {closed_form}",
        floor.l_min_mm
    );
    assert!(
        (floor.l_min_mm - floor.l_min_worst_mm).abs() / closed_form < 1e-6,
        "an UMBILIC surface has one curvature, so the kappa_min and kappa_max bases must be \
         identical: {} vs {}",
        floor.l_min_mm,
        floor.l_min_worst_mm
    );
    assert!(
        (floor.s_max.p50 - s_max).abs() < CURVATURE_TOL,
        "the s_max distribution must collapse to the single analytic value {s_max}; median read \
         {}",
        floor.s_max.p50
    );
}

/// A disk has no long axis, and saying it does would be the same class of error
/// as `D = t1` on an umbilic sphere. The fallback must be `+X` — which is the
/// `0°` raster's own direction, and therefore the cleanest control available.
#[test]
fn pca_falls_back_to_plus_x_on_an_isotropic_region_and_finds_a_real_long_axis() {
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
    let disk = wavy_region_triangles(WAVY_SIZE_MM, cells, WAVY_REGION_RADIUS_MM);
    let sweep = pca_major_axis(&mesh, &disk);
    assert!(
        sweep.isotropic,
        "the wavy region is a digitised DISK — its elongation read {:.5}, which cleared the \
         {PCA_ELONGATION_FLOOR} floor. Either the region changed shape or the moment arithmetic \
         is wrong.",
        sweep.elongation
    );
    assert!(
        (sweep.d.x - 1.0).abs() < 1e-12 && sweep.d.y.abs() < 1e-12,
        "the isotropic fallback must be +X, the 0deg raster's own direction; got {:?}",
        sweep.d
    );
    assert!(
        (sweep.elongation - 1.0).abs() < 0.05,
        "a disk's elongation must be ~1.0; got {:.5}",
        sweep.elongation
    );

    // The positive control: a genuinely elongated strip must be FOUND, so the
    // fallback above is a fact about the fixture and not about the function.
    let cell = WAVY_SIZE_MM / (cells as f64);
    let half = 0.5 * WAVY_SIZE_MM;
    let mut strip: Vec<u32> = Vec::new();
    for row in 0..cells {
        let y = -half + cell * (row as f64 + 0.5);
        for col in 0..cells {
            let x = -half + cell * (col as f64 + 0.5);
            if y.abs() <= 1.0 && x.abs() <= 6.0 {
                let base = 2 * (row * cells + col);
                strip.push(base as u32);
                strip.push((base + 1) as u32);
            }
        }
    }
    let long = pca_major_axis(&mesh, &strip);
    assert!(
        !long.isotropic && long.elongation > 3.0,
        "a 12 x 2 mm strip must read as elongated; got {:.4}",
        long.elongation
    );
    assert!(
        long.d.y.abs() < 0.05,
        "the strip's long axis is X; got {:?}",
        long.d
    );
}

/// **The level sets must run ALONG `d`.** That is the entire kinematics claim
/// of the §4.3 candidate, and it turns on the sense of one cross product.
///
/// On a flat plate `k_s = 0` everywhere, so `|V| = √(1/(8·K_c))` is constant
/// and `φ` is exactly linear along `n × d`. Its level sets are therefore
/// straight lines perpendicular to `n × d` — i.e. parallel to `d`. Both
/// orientations are checked, so the test cannot pass on a fixture that happens
/// to be square.
#[test]
fn sweep_field_level_sets_run_along_the_sweep_direction() {
    let mesh = clean_grid_mesh();
    let region = direction_field::all_triangles(&mesh);
    let params = FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let magnitude = (1.0 / (8.0 * BALL_RADIUS_MM)).sqrt();

    for (name, d) in [
        ("+X", V3::new(1.0, 0.0, 0.0)),
        ("+Y", V3::new(0.0, 1.0, 0.0)),
    ] {
        let (result, report) = direction_field::solve_paths_with_target(
            &mesh,
            &region,
            |_i, _c, n| sweep_target_axis(n, d).map_or_else(V3::zeros, |axis| axis * magnitude),
            &params,
        );
        assert!(
            report.cg_converged,
            "{name}: the Poisson solve must converge on a flat plate"
        );
        assert!(
            !result.polylines.is_empty(),
            "{name}: a 2 mm plate at a {:.4} mm level step must carry several curves",
            (8.0 * CUSP_HEIGHT_MM * BALL_RADIUS_MM).sqrt()
        );
        for line in &result.polylines {
            for seg in line.windows(2) {
                let step = seg[1] - seg[0];
                let along = step.dot(&d).abs();
                let across = (step - d * step.dot(&d)).norm();
                assert!(
                    across <= 1e-4,
                    "{name}: every level-set segment must run ALONG d. Got a step of \
                     {across:.6e} mm across d against {along:.6e} mm along it — if these are \
                     swapped, the 90 degree rotation in sweep_target_axis has the wrong sense \
                     and the passes are running ACROSS the sweep direction."
                );
            }
        }
    }
}

/// The magnitude law, read back out of the level schedule.
///
/// `direction_field::increment_at` is `‖∇φ‖·√h/‖V‖`, which on a converged flat
/// solve is exactly `√h`; the surface step that corresponds to it is
/// `√h/‖V‖ = √(8h·K_c)` — the flat iso-scallop stepover, 0.48990 mm at
/// `K_c = 1`, `h = 0.03`. Asserting the achieved LEVEL SPACING rather than the
/// level VALUES is what makes this a check on the physics rather than on the
/// bookkeeping.
#[test]
fn sweep_field_level_spacing_is_the_flat_iso_scallop_stepover() {
    let mesh = clean_grid_mesh();
    let region = direction_field::all_triangles(&mesh);
    let params = FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let magnitude = (1.0 / (8.0 * BALL_RADIUS_MM)).sqrt();
    let d = V3::new(1.0, 0.0, 0.0);
    let (result, _report) = direction_field::solve_paths_with_target(
        &mesh,
        &region,
        |_i, _c, n| sweep_target_axis(n, d).map_or_else(V3::zeros, |axis| axis * magnitude),
        &params,
    );
    let expected = (8.0 * CUSP_HEIGHT_MM * BALL_RADIUS_MM).sqrt();
    let spacings = field_level_spacing(&result);
    assert!(
        !spacings.is_empty(),
        "at least two levels must carry curves for a spacing to exist"
    );
    let median = percentile(&spacings, 0.50);
    assert!(
        (median - expected).abs() < 0.01,
        "a flat plate's level spacing must be the flat iso-scallop stepover {expected:.5} mm; \
         measured {median:.5} mm. Note this is Zou's sqrt(8 h r) form, which sits 0.8% above \
         scallop_math's exact 2*sqrt(2 r h - h^2) = {:.5} — the two laws differ by the h^2 term \
         and both are correct in their own frames.",
        scallop_math::stepover_from_scallop_flat(BALL_RADIUS_MM, CUSP_HEIGHT_MM)
    );
}

// ── smoke tests for the MEDIAL-AXIS candidate (2026-08-31) ──────────────

/// **The one line of the medial derivation that can be silently inverted**,
/// pinned the way [`k_s_is_read_across_the_passes_not_along_them`] pins the
/// sweep arm's.
///
/// [`medial_field_candidate`] builds the FEED direction `D = n × ĝ` and then
/// hands it to [`sweep_target_axis`] rather than writing `V_dir = −ĝ`
/// directly. That round trip is only worth its keystrokes if the identity
/// actually holds, so this asserts it: for any facet normal `n` and any
/// in-plane unit `ĝ`, `sweep_target_axis(n, n × ĝ) = −ĝ`.
///
/// Getting it backwards would take the stepover ALONG each arm and run the
/// passes ACROSS it — the exact inverse of the operator's proposal, and a
/// result that would still look plausible in every printed column.
#[test]
fn medial_target_axis_is_the_negated_in_plane_gradient() {
    let normals = [
        V3::new(0.0, 0.0, 1.0),
        V3::new(0.3, -0.2, 1.0),
        V3::new(-0.5, 0.4, 1.0),
    ];
    let raw_gradients = [
        V3::new(1.0, 0.0, 0.0),
        V3::new(0.0, -1.0, 0.0),
        V3::new(0.6, 0.8, 0.0),
        V3::new(-0.3, 0.5, 0.0),
    ];
    for n in normals {
        let n = n / n.norm();
        for across in raw_gradients {
            let projected = across - n * across.dot(&n);
            let norm = projected.norm();
            assert!(
                norm > 1e-9,
                "a fixture gradient must not be parallel to its normal, or the case being \
                 pinned is not the one the medial arm takes"
            );
            let g = projected / norm;
            let feed = n.cross(&g);
            assert!(
                (feed.norm() - 1.0).abs() < 1e-12,
                "n x g must be unit when both are unit and perpendicular"
            );
            assert!(
                feed.dot(&n).abs() < 1e-12 && feed.dot(&g).abs() < 1e-12,
                "the FEED direction must lie in the facet plane, perpendicular to the \
                 across-direction: that is what 'along the arm' MEANS"
            );
            let unit =
                sweep_target_axis(n, feed).expect("an in-plane feed direction always projects");
            assert!(
                (unit + g).norm() < 1e-9,
                "sweep_target_axis(n, n x g) must be EXACTLY -g. Measured {unit:?} against \
                 -g = {:?}. If this fails the medial arm is taking its stepover ALONG each \
                 arm and running its passes ACROSS it — the inverse of the proposal, and \
                 invisible in every printed column.",
                -g
            );
        }
    }
}

/// The construction does what its name says on the simplest possible arm.
///
/// A 20 × 2.5 mm strip is one of ARM RIBBON's arms straightened out. Its EDT
/// ridge is the centreline, so:
///
/// * off the centreline the raw gradient must run **across** the strip (±Y)
///   and clear the degeneracy floor, so the feed `n × ĝ` runs **along** it
///   (±X) — the operator's "along the length of each arm", measured;
/// * **on** the centreline the raw gradient must be degenerate, because that
///   is the medial axis and both walls are equidistant. The fallback firing
///   there is the construction working, not failing.
#[test]
fn medial_grid_gradient_runs_across_a_straight_arm() {
    let half_width = 1.25;
    let strip = Polygon2::rectangle(-10.0, -half_width, 10.0, half_width);
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let grid = build_medial_grid("SMOKE strip", std::slice::from_ref(&strip), stepover)
        .expect("a 20 x 2.5 mm strip has a finite extent");

    for (y, expected_gy) in [(0.9, -1.0), (-0.9, 1.0)] {
        let sample = grid.across_at(0.0, y);
        assert_eq!(
            sample.tier,
            GradientTier::Raw,
            "at y = {y} the strip is {:.3} mm from its wall and {:.3} mm from its ridge, so \
             the raw gradient must answer; raw |grad| read {:.4} in cell units",
            half_width - y.abs(),
            y.abs(),
            sample.raw_magnitude
        );
        let (gx, gy) = sample
            .across
            .expect("a Raw tier always carries a direction");
        assert!(
            gx.abs() < 0.2 && (gy - expected_gy).abs() < 0.2,
            "grad(EDT) must point ACROSS the strip toward its ridge — expected \
             (0, {expected_gy}), measured ({gx:.4}, {gy:.4})"
        );
        let n = V3::new(0.0, 0.0, 1.0);
        let feed = n.cross(&V3::new(gx, gy, 0.0));
        assert!(
            feed.x.abs() > 0.95 && feed.y.abs() < 0.2,
            "the FEED direction D = n x grad must run ALONG the strip (+/-X). Measured \
             ({:.4}, {:.4}). This is the operator's proposal, and it is the whole point of \
             the arm.",
            feed.x,
            feed.y
        );
    }

    let ridge = grid.across_at(0.0, 0.0);
    assert!(
        ridge.raw_magnitude < MEDIAL_GRADIENT_FLOOR,
        "ON the centreline both walls are equidistant, so the raw gradient MUST be \
         degenerate — that is what a medial axis IS. Measured |grad| {:.4} against the \
         {MEDIAL_GRADIENT_FLOOR} floor. A clean reading here would mean the grid is not \
         resolving the ridge and the fallback counters are meaningless.",
        ridge.raw_magnitude
    );
    assert_ne!(
        ridge.tier,
        GradientTier::Raw,
        "the ridge sample must be answered by a FALLBACK tier, never by the raw one"
    );
}

/// A disk's medial axis is a single POINT, so the field circulates — which is
/// the hub case in its purest form, and the reason ARM SPHERE and ARM WAVY
/// should come back as concentric rings.
///
/// At any off-centre point `grad(EDT)` is RADIAL (pointing inward, since the
/// EDT rises toward the centre) and the feed `n × ĝ` is therefore TANGENTIAL.
/// Level sets of the solved potential are then circles about the centre: the
/// operator's "spiral in the center", arriving as concentric closed loops.
#[test]
fn medial_grid_circulates_on_a_disk() {
    let radius = 6.0;
    let disk = ellipse_polygon(0.0, 0.0, radius, radius, 512);
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let grid = build_medial_grid("SMOKE disk", std::slice::from_ref(&disk), stepover)
        .expect("a disk has a finite extent");

    assert!(
        (grid.peak_inscribed_mm() - radius).abs() < 0.35,
        "the EDT's peak on a disk IS its radius: expected {radius:.3} mm, measured {:.3} mm \
         (grid cell {:.4} mm). A large miss means the rasterisation or the transform's units \
         are wrong, and every direction downstream inherits it.",
        grid.peak_inscribed_mm(),
        grid.cell_mm()
    );

    for (x, y, rx, ry) in [
        (3.0, 0.0, 1.0, 0.0),
        (0.0, 3.0, 0.0, 1.0),
        (-3.0, 0.0, -1.0, 0.0),
    ] {
        let sample = grid.across_at(x, y);
        let (gx, gy) = sample
            .across
            .expect("a point 3 mm off a disk's centre is not on its medial axis");
        assert!(
            (gx + rx).abs() < 0.2 && (gy + ry).abs() < 0.2,
            "grad(EDT) must point INWARD along the radius at ({x}, {y}) — expected \
             ({:.1}, {:.1}), measured ({gx:.4}, {gy:.4})",
            -rx,
            -ry
        );
        let n = V3::new(0.0, 0.0, 1.0);
        let feed = n.cross(&V3::new(gx, gy, 0.0));
        let radial = V3::new(rx, ry, 0.0);
        assert!(
            feed.dot(&radial).abs() < 0.2,
            "the feed direction must be TANGENTIAL — perpendicular to the radius — which is \
             what makes the level sets concentric rings. Measured feed.radial = {:.4}",
            feed.dot(&radial)
        );
    }
}

/// [`rasterise_polygons`] is even-odd over EVERY ring at once, which is what
/// makes ARM BAND / SHALLOW's five components and four holes come out right
/// without a nesting analysis. This pins the hole, because a filled hole would
/// erase an interior boundary and with it the medial branch that hugs a bump
/// rim — silently, and only on the arm the programme actually came from.
#[test]
fn rasterise_polygons_honours_holes() {
    let cells = 80usize;
    let size = 20.0;
    let cell = size / cells as f64;
    let half = 0.5 * size;
    let ring = |r: f64| -> Vec<P2> {
        vec![
            P2::new(-r, -r),
            P2::new(r, -r),
            P2::new(r, r),
            P2::new(-r, r),
        ]
    };
    let annulus = Polygon2::with_holes(ring(8.0), vec![ring(2.0)]);
    let mask = rasterise_polygons(std::slice::from_ref(&annulus), size, cells);
    let at = |x: f64, y: f64| -> bool {
        let col = ((x + half) / cell).floor() as usize;
        let row = ((y + half) / cell).floor() as usize;
        mask[row * cells + col]
    };
    assert!(at(5.0, 0.0), "the annulus body must be filled");
    assert!(at(0.0, -5.0), "the annulus body must be filled");
    assert!(!at(0.0, 0.0), "the HOLE must not be filled");
    assert!(!at(1.0, 1.0), "the HOLE must not be filled");
    assert!(!at(9.0, 0.0), "outside the exterior must not be filled");
    let filled = mask.iter().filter(|&&v| v).count() as f64 * cell * cell;
    let expected = 16.0 * 16.0 - 4.0 * 4.0;
    assert!(
        (filled - expected).abs() < 2.0,
        "filled area {filled:.3} mm^2 must match the annulus's {expected:.3} mm^2 within a \
         cell of quantisation"
    );
}
