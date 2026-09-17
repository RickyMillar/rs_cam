//! **Phase F1 steps 1–5 — the direction-field evidence instrument.**
//! (`planning/conformal_finish_2026-08-28/PROGRAMME.md` §F1; design note
//! `research/conformal_finish_2026-08-28.md` §A and §C.3.)
//!
//! # What this measures
//!
//! One question, staged, on the captured Wanaka region 1: does a
//! direction-field iso-curve family cover that region with *fewer* path
//! fragments, and less integrated time, than the straight-sweep evidence
//! already published for it?
//!
//! | stage | contract item | what it prints |
//! |---|---|---|
//! | **F1-A** | 1, 4 | the full [`rs_cam_core::finish::direction_field::FieldReport`] plus the **cheap falsifier**: total polylines vs the 141-fragment PCA-cell reference |
//! | **F1-B** | 6 | one SVG of every iso-curve over the captured boundary |
//! | **F1-C** | 3 | measured spacing between adjacent levels vs the target stepover — geometry only, no simulation |
//! | **F1-D** | 1, 2, 4 | contact→cutter-centre conversion, a boundary containment proof, and the F-034 cost |
//! | **F1-E** | 1, 2 | ball-end 0° and PCA-minor raster baselines, single-variable, plus labelled cross-cutter context |
//!
//! # F1-S — the SEGMENTED arm (added 2026-08-31), a second test in this file
//!
//! [`wanaka_direction_field_segmented_f1`] is a separate `#[ignore]` run and
//! the staged run above is **untouched**: it is the record
//! `planning/conformal_finish_2026-08-28/FINDINGS.md` §F1-1 cites.
//!
//! §F1-1 falsified the whole-region field at **6,589 polylines** (253 levels,
//! median **28 components per level**, 2,340 orientation inconsistencies).
//! `planning/finishing_synthesis_2026-08-30.md` §11 withdrew the closure that
//! followed: region 1's *measured* curvature anisotropy gives the method a
//! **+9.75 % ceiling at R = 1.0** (median `W_max/W_min` 1.0950), and both
//! scale rules confirm that is real landscape rather than tessellation. So
//! the premise is fine and the failure has to be explained some other way.
//!
//! The literature's pipeline
//! (`planning/conformal_finish_2026-08-28/preferred_direction_field_research.md`)
//! is: direction rule → detect degeneracies → classify → trace separatrices →
//! **segment** → per-patch solve. We built the direction rule and none of the
//! other four. **Segmentation is what stops one global scalar's level sets
//! threading every branch at once**, which is exactly the measured failure.
//! F1-S asks the one question that discriminates: *does segmenting region 1
//! before solving collapse the fragmentation?*
//!
//! Three deliberate choices, all disclosed at their definitions:
//!
//! * **`t₁` is computed here, not read from `direction_field`.**
//!   `crest_lines::Curvature` is `pub(crate)` and unreachable from an
//!   integration test, and no `src/` change was in scope. F1-S therefore
//!   restates the Monge-quadric estimator of `wanaka_curvature_anisotropy.rs`
//!   (committed `5e4ae866`) and takes `t₁` from the **shape operator's**
//!   eigenvector — see [`fit_from_derivatives`].
//! * **The solve runs through
//!   [`rs_cam_core::finish::direction_field::solve_paths_with_target`]**, not
//!   `solve_field_paths`. That is the entry point the module header reserves
//!   for exactly this ("how the F1 evidence instrument substitutes an
//!   externally-computed field"), and it buys two things: the direction the
//!   segmentation clusters on and the direction the Poisson solve integrates
//!   are the **same** `t₁` — no estimator confound between the two halves —
//!   and it avoids re-running a whole-mesh curvature pass once per patch.
//! * **The 180° arm is the control.** At a 180° tolerance nothing is ever
//!   rejected, so the region grows as one patch: same `V`, same code path,
//!   same estimator, differing from the segmented arms **only** in the
//!   decomposition. That, not the historical 6,589, is what the
//!   pre-registered verdict reads. The estimator changed between the two, and
//!   the run says so before it prints a number.
//!
//! Running it, and what it costs:
//!
//! ```text
//! THIN_ORGANIC_SVG_DIR=/home/ricky/Downloads/svg \
//! cargo test -p rs_cam_core --test direction_field_wanaka_f1 \
//!   wanaka_direction_field_segmented_f1 -- --ignored --nocapture
//! ```
//!
//! Nine arms, each a full per-patch solve over ~44k triangles, plus one
//! `relink_and_cost` for every arm under the cost ceiling. `build_region_mesh`
//! rebuilds an edge→face map on **every** patch, so wall-clock scales with
//! *patch count* × *region size*: a tight tolerance that produces thousands of
//! patches is the slow case, and [`region_submesh`] is what keeps that
//! multiplier at 44k rather than 661k. Budget tens of minutes, not seconds.
//!
//! `wanaka_curvature_anisotropy.rs` (`5e4ae866`) is restated at:
//! `solve_sym6` (:547–602), `finalise_normal_equations` (:652–664),
//! `fit_quadric` (:668–761), `fit_from_derivatives` (:606–642, **extended**
//! with the principal-direction eigenvector), `kappa_perp_zou` (:779–791) and
//! `Scratch` (:440–460).
//!
//! # The single-variable rule this file obeys
//!
//! The candidate **and** every baseline in the final table run a true
//! [`rs_cam_core::tool::BallEndmill`] of radius 1.0 mm at the equal-cusp
//! stepover for `h = 0.03`. That is design note §C.4: both source papers are
//! ball-end native, and the shipped R1.5/R1.0 evidence is **tapered** ball.
//! The historical tapered numbers (875.9 s PCA cells, 963.0 s PCA-minor
//! undivided, 1053.7 s 0° undivided) are printed as **cross-cutter context**
//! and are explicitly *not* this table's bar — a different cutter is a second
//! variable.
//!
//! The one place the tapered ladder still rules is **region identity**: the
//! capture at `test_data/wanaka_region1_boundary_f1.json` was derived through
//! the frozen tapered chain (see `wanaka_region_capture_f1.rs`), and this file
//! only ever *loads* it. Drift detection lives in that file, not here.
//!
//! # Skip semantics
//!
//! Both inputs are outside this repo's control, so every staged test SKIPs
//! (prints and returns) rather than failing when either is missing:
//!
//! * the operator's 661k-triangle Wanaka mesh, machine-local by nature; and
//! * the region-1 capture, which does not exist until
//!   `wanaka_region_capture_f1::capture_wanaka_region_1_boundary` has been run
//!   once.
//!
//! The small tests at the bottom of this file are **not** `#[ignore]`d: they
//! need neither input and pin this instrument's own arithmetic.
//!
//! # Restated, not imported
//!
//! Integration tests cannot import from each other. Following the precedent
//! the capture file states outright ("a later F1 evidence test is expected to
//! **restate** the ~20-line loader"), and the repo's own rule that *an
//! instrument should show its own arithmetic*, the following are restated
//! from `thin_organic_island_widths.rs` at the noted lines:
//!
//! * `equal_cusp_stepover_mm` (:129–137)
//! * the pinned Shapeoko Pro XXL kinematics + feeds (:662–672)
//! * `grid_for_direction` (:1478–1493) — with the ball-end cutter
//! * `pca_minor_and_elongation` (:1644–1683)
//! * `cell_svg_output_dir` (:1741–1752) and `svg_path` (:1754–1770)
//! * `CandidateCost` (:1861–1878) and `relink_and_cost` (:1939–2013) — the
//!   `RelinkParams` block field-for-field
//! * `raster_candidate` (:2015–2038)
//!
//! and `load_region1_capture` from `wanaka_region_capture_f1.rs:440–476`.
//! Two deliberate trims are noted at their definitions.
//!
//! # Running it
//!
//! ```text
//! THIN_ORGANIC_SVG_DIR=/home/ricky/Downloads/svg \
//! cargo test -p rs_cam_core --test direction_field_wanaka_f1 \
//!   wanaka_direction_field_f1 -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::finish::direction_field::{self, FieldPathResult, FieldReport};
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CandidateCost, CostingContext, CostingFeeds, relink_and_cost as metrology_relink_and_cost,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// ── inputs ──────────────────────────────────────────────────────────────

/// The operator's wanaka board. Absolute, outside the repo, by nature.
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// ── the tooling decision (design note §C.4) ─────────────────────────────

/// Ball-end cutter radius (mm) for the candidate AND every baseline. Matches
/// the tier-1 tapered tool's **cusp radius**, which is what the equal-cusp
/// stepover law consumes — so the two cutters lay passes at the same spacing
/// and only the cutter *shape* differs between this table and the historical
/// one.
const BALL_RADIUS_MM: f64 = 1.0;

/// Cutting length of the ball control fixture (mm), matching the tier-1
/// R1.0 tapered tool's. Inert for every measurement here — drop-cutter
/// geometry reads `radius()`/`height_at_radius()` only — but a cutter needs
/// one, and an arbitrary value would invite a later reader to wonder.
const BALL_CUTTING_LENGTH_MM: f64 = 20.0;

/// Scallop-height constraint `h` (mm): the cusp target both the level
/// schedule and the baselines' stepover are derived from.
const CUSP_HEIGHT_MM: f64 = 0.03;

// ── the pinned machine (thin_organic_island_widths.rs:662-672) ──────────
//
// Shapeoko Pro XXL, read from `wanaka200_mt2.toml`'s `[job.machine]` /
// `[job.machine.kinematics]`, so the integrator sees the operator's real
// envelope rather than a default.

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
/// Tier-1 op feeds from the same file.
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

// ── the references this candidate is scored against ─────────────────────
//
// `planning/thin_organic_2026-08-27/FINDINGS.md` §"the bar" table. Fragment
// counts feed the Stage A falsifier; the times are CROSS-CUTTER context only
// (tapered R1.5 ladder), never this table's bar.

/// PCA-minor + monotone cells, tapered: 69 cells, **141 fragments**.
const REF_PCA_CELL_FRAGMENTS: usize = 141;
/// PCA-minor undivided, tapered: 491 fragments.
const REF_PCA_UNDIVIDED_FRAGMENTS: usize = 491;
/// 0° undivided, tapered: **564 fragments**.
const REF_RASTER0_FRAGMENTS: usize = 564;

const REF_PCA_CELL_TIME_S: f64 = 875.9;
const REF_PCA_UNDIVIDED_TIME_S: f64 = 963.0;
const REF_RASTER0_TIME_S: f64 = 1053.7;

/// The historical PCA-minor pass direction on this region, for comparison
/// with the angle this file recomputes from the capture.
const REF_PCA_MINOR_DEG: f64 = 119.6;

/// Stage A stops the run when the field fragments more than this multiple of
/// the PCA-cell reference. Design note §C.5 states the falsifier as "fragment
/// worse than the 141 the PCA cells did"; a *bare* 141 would stop on a
/// candidate that is merely no better, which is a result worth costing. 5× is
/// the "premise is dead" band — at 705 fragments there is nothing a relinker
/// can do that the straight sweeps have not already been measured doing.
const FALSIFIER_MULTIPLE: usize = 5;

/// The `link_ceiling: None` label the charter makes mandatory. Printed at
/// EVERY costing surface, because a table gets quoted without its preamble.
const FRESH_STOCK_LABEL: &str = "link_ceiling: None — FRESH-STOCK FIRST-EXPERIMENT EXCEPTION (PROGRAMME.md \
     'Shared experimental contract'). NO TIME CLAIM HERE IS FINAL until this \
     candidate is re-run under the corrected rest-stock ceiling.";

// ── restated arithmetic ─────────────────────────────────────────────────

/// `s = 2·√(2Rh − h²)` — the equal-cusp law. One site in production
/// (`session::multitool`); restated here because an instrument should show
/// its own arithmetic (`thin_organic_island_widths.rs:129-137`).
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// Absolute path to the workspace root, canonicalized.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// Where the committed region-1 capture lives.
fn capture_path() -> PathBuf {
    repo_root().join("test_data/wanaka_region1_boundary_f1.json")
}

/// Load `test_data/wanaka_region1_boundary_f1.json`.
///
/// **Restated from `wanaka_region_capture_f1.rs:440-476`**, which documents
/// exactly this expectation: integration tests cannot import from each other,
/// so the ~20-line loader is copied rather than depended on. The polygon is
/// rebuilt with [`Polygon2::with_holes_closed`] and **no** `ensure_winding` —
/// the captured vertex order is preserved exactly.
///
/// `None` when the artifact is absent or unreadable.
fn load_region1_capture() -> Option<Polygon2> {
    let raw = std::fs::read_to_string(capture_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let ring = |val: &serde_json::Value| -> Vec<P2> {
        val.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some(P2::new(p[0].as_f64()?, p[1].as_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let exterior = ring(&v["exterior"]);
    let holes: Vec<Vec<P2>> = v["holes"]
        .as_array()
        .map(|a| a.iter().map(ring).collect())
        .unwrap_or_default();
    let closed = v["closed"].as_bool().unwrap_or(true);
    (exterior.len() >= 3).then(|| Polygon2::with_holes_closed(exterior, holes, closed))
}

/// 2×2 second-moment eigen-decomposition of the polygon's interior, sampled
/// on a `cell` lattice. Returns `(PCA-minor axis in degrees, elongation)`.
/// **Restated from `thin_organic_island_widths.rs:1644-1683`.**
fn pca_minor_and_elongation(poly: &Polygon2, cell: f64) -> Option<(f64, f64)> {
    let [x0, y0, x1, y1] = poly.bbox();
    let nx = (((x1 - x0) / cell).ceil() as usize).saturating_add(2);
    let ny = (((y1 - y0) / cell).ceil() as usize).saturating_add(2);
    let mut points = Vec::new();
    for row in 0..ny {
        for col in 0..nx {
            let point = P2::new(x0 + col as f64 * cell, y0 + row as f64 * cell);
            if poly.contains_point(&point) {
                points.push(point);
            }
        }
    }
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let (sum_x, sum_y) = points.iter().fold((0.0_f64, 0.0_f64), |(sx, sy), point| {
        (sx + point.x, sy + point.y)
    });
    let (cx, cy) = (sum_x / n, sum_y / n);
    let (sxx, syy, sxy) = points
        .iter()
        .fold((0.0_f64, 0.0_f64, 0.0_f64), |(xx, yy, xy), point| {
            let (dx, dy) = (point.x - cx, point.y - cy);
            (xx + dx * dx, yy + dy * dy, xy + dx * dy)
        });
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let major = 0.5 * (2.0 * sxy).atan2(sxx - syy).to_degrees();
    let minor = (major + 90.0).rem_euclid(180.0);
    let trace = sxx + syy;
    let determinant = sxx * syy - sxy * sxy;
    let spread = ((trace * trace / 4.0) - determinant).max(0.0).sqrt();
    let large = trace / 2.0 + spread;
    let small = trace / 2.0 - spread;
    if small <= 1e-9 {
        return Some((minor, f64::INFINITY));
    }
    Some((minor, (large / small).sqrt()))
}

/// Rotated drop-cutter lattice. **Restated from
/// `thin_organic_island_widths.rs:1478-1493`**, with the ball-end control
/// fixture in place of the tapered cutter — the single variable this file
/// changes.
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

/// **Restated from `thin_organic_island_widths.rs:2015-2038`.**
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

/// **Restated from `thin_organic_island_widths.rs:1861-1878`**, with the two
/// ceiling-regime fields (`slower_than_retract`, `ceiling_above_safe_z`)
/// **trimmed**: they are read only by that file's Stage L, which costs a
/// candidate under a rest-stock ceiling. Every arm here runs
/// `link_ceiling: None`, where `ceiling_above_safe_z` is structurally 0
/// (`surface_link.rs:302-308`), so carrying them would be a write-only field.
/// The six fields below are exactly the ones the F1 brief's table quotes.
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

// ── SVG (restated from thin_organic_island_widths.rs:1741-1770) ─────────

/// Where the operator-facing debug SVGs land. The environment override lets
/// an evidence run put the artifacts straight where the operator asked, while
/// the default remains a disposable build artifact. Same variable as the
/// reference instrument (`THIN_ORGANIC_SVG_DIR`), a different default
/// directory so the two runs cannot overwrite each other.
fn svg_output_dir() -> PathBuf {
    std::env::var_os("THIN_ORGANIC_SVG_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("conformal_f1")
        },
        PathBuf::from,
    )
}

/// One polygon as an SVG path, exterior then holes, `evenodd`-ready.
/// **Restated from `thin_organic_island_widths.rs:1754-1770`.**
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

// ── candidate construction ──────────────────────────────────────────────

/// Contact-point polylines → cutter-centre (CL) polylines.
///
/// `direction_field` emits **cutter-contact points on the mesh surface** (its
/// module header says so); the rs_cam convention is that the cutter-centre
/// conversion is a drop-cutter projection at the contact point's XY. That is
/// what `project_curve` and every finishing generator do.
///
/// Two guards, both counted rather than silently absorbed:
///
/// * a CL point whose drop-cutter never contacted carries `z = -∞`
///   (`tool/mod.rs` `CLPoint::contacted`), which would poison the F-034
///   integration — dropped and counted;
/// * a polyline left with fewer than two points after dropping is not a path.
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

/// Assemble arbitrary CL polylines into a raw `Toolpath`.
///
/// Move-intent tagging mirrors `toolpath::raster_toolpath_from_grid`
/// (`toolpath.rs:695-744`) exactly, because the relinker and the F-034
/// integrator read those tags: per polyline a `Linking` rapid across at
/// `safe_z`, an `EntryPlunge` feed at the plunge rate, then `FinishingCut`
/// feeds along the curve; a `Retract` rapid at the **previous** polyline's
/// exit XY separates two polylines, and one final `Retract` closes the path.
///
/// It deliberately does NOT link, reorder or shortcut anything: the whole
/// point of the comparison is that `relink_and_cost` applies the SAME
/// production relink to every arm.
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

// ── geometry helpers ────────────────────────────────────────────────────

/// Squared 3D distance from `p` to the segment `a`–`b`.
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

/// Percentile of an already-sorted slice, nearest-rank.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

// ── STAGE F1-A — field solve + cheap falsifier ──────────────────────────

/// Summary statistics of a `Vec<usize>` histogram, for the per-level
/// component counts (the raw vector can be thousands of entries long).
fn min_median_max(values: &[usize]) -> (usize, usize, usize) {
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

fn stage_a(report: &FieldReport, result: &FieldPathResult, stepover_mm: f64) -> bool {
    eprintln!("========== STAGE F1-A — direction-field solve + cheap falsifier ==========\n");
    eprintln!(
        "   cutter: BallEndmill r = {BALL_RADIUS_MM:.3} mm (diameter {:.3}), \
         h = {CUSP_HEIGHT_MM} mm, equal-cusp stepover {stepover_mm:.4} mm",
        BALL_RADIUS_MM * 2.0
    );
    eprintln!(
        "   field:  D = t1 (max signed principal direction), V = D-perp, |V| = sqrt((k_s + 1/r)/8)\n"
    );

    eprintln!("   -- region --");
    eprintln!(
        "     region triangles              {:>10}",
        report.region_triangles
    );
    eprintln!(
        "     region vertices               {:>10}",
        report.region_vertices
    );
    eprintln!(
        "     vertex components             {:>10}",
        report.vertex_components
    );
    eprintln!("   -- direction field (§4.2) --");
    eprintln!(
        "     BFS orientation seeds         {:>10}",
        report.direction_seeds
    );
    eprintln!(
        "     singular (isotropic) tris     {:>10}",
        report.degenerate_triangles
    );
    eprintln!(
        "     transported tris              {:>10}",
        report.transported_triangles
    );
    eprintln!(
        "     orientation inconsistencies   {:>10}",
        report.orientation_inconsistencies
    );
    eprintln!(
        "     unoriented tris (want 0)      {:>10}",
        report.unoriented_triangles
    );
    eprintln!("   -- target field V (Eq. 13) --");
    eprintln!(
        "     clamped-magnitude tris        {:>10}",
        report.clamped_magnitude_triangles
    );
    eprintln!(
        "     |V| min / mean / max          {:>10.6} / {:.6} / {:.6}",
        report.min_target_magnitude, report.mean_target_magnitude, report.max_target_magnitude
    );
    eprintln!("   -- Poisson solve (Eq. 15, matrix-free Jacobi-CG) --");
    eprintln!(
        "     CG iterations                 {:>10}",
        report.cg_iterations
    );
    eprintln!(
        "     CG relative residual          {:>10.3e}",
        report.cg_residual
    );
    eprintln!(
        "     CG converged                  {:>10}",
        report.cg_converged
    );
    eprintln!("   -- level schedule + marching triangles (§3.3) --");
    let (min_c, med_c, max_c) = min_median_max(&report.level_component_counts);
    eprintln!(
        "     levels                        {:>10}",
        result.levels.len()
    );
    eprintln!("     components/level min/med/max  {min_c:>10} / {med_c} / {max_c}");
    eprintln!(
        "     closed loops                  {:>10}",
        report.closed_loops
    );
    eprintln!(
        "     saddle/degenerate crossings   {:>10}",
        report.degenerate_crossings
    );
    eprintln!(
        "     floored increments            {:>10}",
        report.floored_increments
    );
    eprintln!(
        "     level cap hit                 {:>10}",
        report.level_cap_hit
    );
    eprintln!(
        "     TOTAL polylines               {:>10}",
        report.total_polylines
    );
    if let (Some(first), Some(last)) = (result.levels.first(), result.levels.last()) {
        eprintln!("     level range                   {first:>10.6} .. {last:.6}");
    }

    let total = report.total_polylines;
    let ceiling = REF_PCA_CELL_FRAGMENTS * FALSIFIER_MULTIPLE;
    eprintln!("\n   -- CHEAP FALSIFIER (design note §C.5) --");
    eprintln!(
        "     candidate polylines {total}  vs  PCA-cell reference {REF_PCA_CELL_FRAGMENTS} \
         fragments, 0° undivided {REF_RASTER0_FRAGMENTS}"
    );
    eprintln!(
        "     (both references are TAPERED-ball fragment counts — they bound the \n\
         \x20     fragmentation the straight sweeps produce on this region, which is the \n\
         \x20     quantity the falsifier is about, not a cutter-matched cost.)"
    );
    if total > ceiling {
        eprintln!(
            "     FAIL / STOP: {total} > {FALSIFIER_MULTIPLE}x{REF_PCA_CELL_FRAGMENTS} = {ceiling}. \
             The premise 'follows branches with fewer turns' is dead on this region;\n\
             \x20    stages C-E are SKIPPED. Stage B still runs — diagnosis needs the picture.\n"
        );
        return false;
    }
    if total > REF_PCA_CELL_FRAGMENTS {
        eprintln!(
            "     PASS (marginal): {total} > {REF_PCA_CELL_FRAGMENTS} but <= {ceiling}. \
             More fragments than the PCA cells, still inside the\n\
             \x20    'worth costing' band — the relinker, not the fragment count, decides.\n"
        );
    } else {
        eprintln!(
            "     PASS: {total} <= {REF_PCA_CELL_FRAGMENTS}. Fewer fragments than the PCA-cell \
             reference.\n"
        );
    }
    true
}

// ── STAGE F1-B — SVG overlay ────────────────────────────────────────────

fn stage_b(boundary: &Polygon2, result: &FieldPathResult) {
    /// Two strokes, alternating on level parity, so adjacent passes are
    /// distinguishable at a glance — the artifact's whole job is to show
    /// whether the curves march or interleave.
    const LEVEL_COLOURS: [&str; 2] = ["#377eb8", "#e41a1c"];

    eprintln!("========== STAGE F1-B — iso-curve SVG over the captured boundary ==========\n");
    let [x0, y0, x1, y1] = boundary.bbox();
    let padding = 2.0;
    let (view_x, view_y) = (x0 - padding, y0 - padding);
    let (view_w, view_h) = (x1 - x0 + 2.0 * padding, y1 - y0 + 2.0 * padding);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_x:.3} {view_y:.3} {view_w:.3} {view_h:.3}\" width=\"1400\" height=\"1400\">\n\
         <title>Wanaka region 1: {} direction-field iso-curves at {} levels</title>\n\
         <rect x=\"{view_x:.3}\" y=\"{view_y:.3}\" width=\"{view_w:.3}\" height=\"{view_h:.3}\" fill=\"white\"/>\n",
        result.polylines.len(),
        result.levels.len()
    );
    for (line, &level) in result.polylines.iter().zip(result.polyline_levels.iter()) {
        let Some(first) = line.first() else { continue };
        let colour = LEVEL_COLOURS[level % LEVEL_COLOURS.len()];
        let mut d = format!("M {:.3} {:.3}", first.x, first.y);
        for point in &line[1..] {
            write!(d, " L {:.3} {:.3}", point.x, point.y).expect("write SVG curve");
        }
        writeln!(
            svg,
            "<path d=\"{d}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"0.06\"/>"
        )
        .expect("write SVG curve element");
    }
    // Boundary last, on top: exterior in black, holes in a distinct stroke so
    // a curve escaping INTO a hole is visible rather than hidden under one
    // uniform outline.
    let exterior_only = Polygon2::new(boundary.exterior.clone());
    writeln!(
        svg,
        "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.22\"/>",
        svg_path(&exterior_only)
    )
    .expect("write SVG exterior");
    for hole in &boundary.holes {
        writeln!(
            svg,
            "<path d=\"{}\" fill=\"none\" stroke=\"#ff7f00\" stroke-width=\"0.22\" stroke-dasharray=\"1.0 0.6\"/>",
            svg_path(&Polygon2::new(hole.clone()))
        )
        .expect("write SVG hole");
    }
    svg.push_str("</svg>\n");

    let output_dir = svg_output_dir();
    std::fs::create_dir_all(&output_dir).expect("create F1 SVG output directory");
    let path = output_dir.join("wanaka_region1_direction_field_f1.svg");
    std::fs::write(&path, svg).expect("write F1 iso-curve SVG");
    eprintln!(
        "   {} curves at {} levels; exterior black, holes dashed orange, \
         curve stroke alternates on level parity.",
        result.polylines.len(),
        result.levels.len()
    );
    eprintln!("   SVG: {}\n", path.display());
}

// ── STAGE F1-C — measured spacing (contract item 3) ─────────────────────

fn stage_c(result: &FieldPathResult, stepover_mm: f64) {
    /// Sample every Nth point of the level-(i+1) curves. Spacing is a smooth
    /// quantity along a curve; every point would multiply the work without
    /// adding a distinct measurement.
    const SAMPLE_STRIDE: usize = 5;
    /// Per-adjacent-pair budget on distance evaluations. When a pair would
    /// exceed it, the stride widens (and the widening is reported), so a
    /// pathological level cannot turn a geometry stage into a long run.
    const MAX_PAIR_OPS: usize = 4_000_000;
    /// Contract band: how far from the target stepover a sample may sit
    /// before it counts as off-target.
    const BAND: f64 = 0.25;

    eprintln!("========== STAGE F1-C — measured spacing between adjacent levels ==========\n");
    eprintln!(
        "   Method: every {SAMPLE_STRIDE}th point of level i+1, 3D distance to the nearest\n\
         \x20  SEGMENT (not vertex) of level i. Geometry only — no simulation. Target is the\n\
         \x20  equal-cusp stepover {stepover_mm:.4} mm at R={BALL_RADIUS_MM}, h={CUSP_HEIGHT_MM}.\n\
         \x20  Note the scallop constraint is SOFT (Eq. 14 fits |grad phi| in least squares;\n\
         \x20  the paper's own measured scallop error is < 4% over h = 0.01-0.1 mm).\n"
    );

    let level_count = result.levels.len();
    if level_count < 2 {
        eprintln!("   SKIP: fewer than two levels — nothing adjacent to measure.\n");
        return;
    }
    let mut by_level: Vec<Vec<&Vec<P3>>> = vec![Vec::new(); level_count];
    for (line, &level) in result.polylines.iter().zip(result.polyline_levels.iter()) {
        if let Some(slot) = by_level.get_mut(level) {
            slot.push(line);
        }
    }

    let mut spacings: Vec<f64> = Vec::new();
    let mut widened_pairs = 0usize;
    let mut empty_pairs = 0usize;
    for index in 1..level_count {
        let previous = &by_level[index - 1];
        let current = &by_level[index];
        if previous.is_empty() || current.is_empty() {
            empty_pairs += 1;
            continue;
        }
        let segment_count: usize = previous
            .iter()
            .map(|line| line.len().saturating_sub(1))
            .sum();
        let point_count: usize = current.iter().map(|line| line.len()).sum();
        if segment_count == 0 {
            empty_pairs += 1;
            continue;
        }
        let raw_ops = point_count.saturating_mul(segment_count);
        let stride = if raw_ops > MAX_PAIR_OPS {
            widened_pairs += 1;
            SAMPLE_STRIDE.max(raw_ops.div_ceil(MAX_PAIR_OPS))
        } else {
            SAMPLE_STRIDE
        };
        for line in current {
            for point in line.iter().step_by(stride) {
                let mut best = f64::INFINITY;
                for previous_line in previous {
                    for pair in previous_line.windows(2) {
                        let distance = point_segment_distance_sq(*point, pair[0], pair[1]);
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

    if spacings.is_empty() {
        eprintln!("   SKIP: no adjacent-level sample pairs produced a measurement.\n");
        return;
    }
    spacings.sort_by(f64::total_cmp);
    let n = spacings.len() as f64;
    let mean = spacings.iter().sum::<f64>() / n;
    let (low, high) = (stepover_mm * (1.0 - BAND), stepover_mm * (1.0 + BAND));
    let below = spacings.iter().filter(|&&s| s < low).count();
    let above = spacings.iter().filter(|&&s| s > high).count();

    eprintln!("     {:>26}  {:>12}", "statistic", "mm");
    eprintln!("     {:>26}  {:>12.4}", "TARGET stepover", stepover_mm);
    eprintln!("     {:>26}  {:>12.3}", "min", percentile(&spacings, 0.0));
    eprintln!("     {:>26}  {:>12.3}", "p10", percentile(&spacings, 0.10));
    eprintln!(
        "     {:>26}  {:>12.3}",
        "median",
        percentile(&spacings, 0.50)
    );
    eprintln!("     {:>26}  {:>12.3}", "mean", mean);
    eprintln!("     {:>26}  {:>12.3}", "p90", percentile(&spacings, 0.90));
    eprintln!("     {:>26}  {:>12.3}", "max", percentile(&spacings, 1.0));
    eprintln!(
        "\n     samples {}, band +/-{:.0}% = [{low:.4}, {high:.4}] mm",
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
    eprintln!(
        "     adjacent level pairs with an empty side: {empty_pairs}; \
         pairs whose stride widened for budget: {widened_pairs}"
    );
    eprintln!(
        "     NOTE: a level-i+1 point measures to the nearest level-i curve ANYWHERE, so\n\
         \x20    where a level fragments across a branch the nearest neighbour may be a\n\
         \x20    different branch's curve. Read the distribution, not any single tail value.\n"
    );
}

// ── STAGE F1-D — CL conversion, containment, cost ───────────────────────

/// Everything every arm shares. Bundled rather than passed as five
/// parameters for the same reason `LinkRegime` exists in the reference
/// instrument: clippy's `too_many_arguments` fires at eight, and Stage E
/// needs eight otherwise. All fields are `Copy`, so it rides by value.
#[derive(Clone, Copy)]
struct Fixture<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a BallEndmill,
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
    /// `mesh.bbox.max.z + 5.0`, as the reference instrument computes it.
    safe_z: f64,
    /// `mesh.bbox.min.z - 0.1`, as the reference instrument computes it.
    effective_min_z: f64,
}

struct StageDOutcome {
    cost: CandidateCost,
    cl_polylines: usize,
}

fn stage_d(
    fixture: Fixture<'_>,
    boundary: &Polygon2,
    result: &FieldPathResult,
) -> Option<StageDOutcome> {
    use rs_cam_core::geometry::region_set::RegionSet;

    let Fixture {
        mesh,
        index,
        cutter,
        kinematics,
        safe_z,
        ..
    } = fixture;

    eprintln!("========== STAGE F1-D — CL conversion, containment proof, F-034 cost ==========\n");

    let converted = cl_polylines(&result.polylines, mesh, index, cutter);
    eprintln!("   -- contact -> cutter-centre (point_drop_cutter, rs_cam convention) --");
    eprintln!(
        "     contact polylines in          {:>10}",
        result.polylines.len()
    );
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
        "     CL polylines out              {:>10}",
        converted.polylines.len()
    );
    if converted.polylines.is_empty() {
        eprintln!("\n   REFUSE: no CL polyline survived the conversion.\n");
        return None;
    }

    let raw = polylines_to_toolpath(&converted.polylines, FEED_MM_MIN, PLUNGE_MM_MIN, safe_z);

    // -- containment proof (contract item 4) --
    let outside = |tp: &Toolpath| -> (usize, usize) {
        let mut cutting = 0usize;
        let mut escapes = 0usize;
        for mv in &tp.moves {
            if !mv.move_type.is_cutting() {
                continue;
            }
            cutting += 1;
            if !boundary.contains_point(&P2::new(mv.target.x, mv.target.y)) {
                escapes += 1;
            }
        }
        (cutting, escapes)
    };
    let (cutting_moves, escapes) = outside(&raw);
    eprintln!("\n   -- containment proof: every cutting move inside the captured boundary --");
    eprintln!("     cutting moves                 {cutting_moves:>10}");
    eprintln!("     outside the boundary          {escapes:>10}");
    let priced = if escapes == 0 {
        eprintln!("     VERDICT: CONTAINED (0 escapes) — no clip applied.");
        raw
    } else {
        // The curves live on region triangles, but drop-cutter CL shifts
        // laterally against a slope, so a contact point just inside the
        // boundary can land its cutter centre just outside it.
        let before_mm = raw.total_cutting_distance();
        let clipped =
            rs_cam_core::geometry::boundary::clip_toolpath_to_boundary(&raw, boundary, safe_z);
        let after_mm = clipped.total_cutting_distance();
        let (clipped_cutting, clipped_escapes) = outside(&clipped);
        eprintln!(
            "     VERDICT: {escapes} escapes of {cutting_moves} cutting moves \
             ({:.3}%) — CLIPPED with boundary::clip_toolpath_to_boundary.",
            100.0 * escapes as f64 / cutting_moves.max(1) as f64
        );
        eprintln!(
            "     cutting mm {before_mm:.1} -> {after_mm:.1} (removed {:.1} mm, {:.3}%)",
            before_mm - after_mm,
            100.0 * (before_mm - after_mm) / before_mm.max(1e-9)
        );
        eprintln!(
            "     post-clip cutting moves {clipped_cutting}, remaining outside \
             {clipped_escapes} (a residual is the clip's own segment-endpoint \
             tolerance, not an uncontained path)."
        );
        clipped
    };

    // -- cost --
    eprintln!("\n   -- F-034 cost through the SAME production relink as every baseline --");
    eprintln!("     {FRESH_STOCK_LABEL}");
    let region = RegionSet::new(vec![boundary.clone()]);
    let cost = relink_and_cost(priced, mesh, index, cutter, &region, &kinematics, safe_z);
    eprintln!(
        "     moves {}, fragments {}, linked {}, kept retracts {}, cutting {:.1} mm, \
         time {:.1} s\n",
        cost.moves, cost.fragments, cost.linked, cost.kept_retracts, cost.cutting_mm, cost.time_s
    );
    Some(StageDOutcome {
        cost,
        cl_polylines: converted.polylines.len(),
    })
}

// ── STAGE F1-E — ball-end baselines, single variable ────────────────────

fn stage_e(
    fixture: Fixture<'_>,
    boundary: &Polygon2,
    stepover_mm: f64,
    candidate: Option<&StageDOutcome>,
) {
    use rs_cam_core::geometry::region_set::RegionSet;

    let Fixture {
        mesh,
        index,
        cutter,
        kinematics,
        safe_z,
        effective_min_z,
    } = fixture;

    eprintln!("========== STAGE F1-E — ball-end baselines (single-variable table) ==========\n");
    eprintln!(
        "   Every row below runs the SAME BallEndmill r={BALL_RADIUS_MM} at the SAME \
         {stepover_mm:.4} mm\n\
         \x20  stepover, the same feeds, the same Shapeoko kinematics, the same boundary and\n\
         \x20  the same production relink. The only difference is the path pattern.\n"
    );
    eprintln!("   {FRESH_STOCK_LABEL}\n");

    let region = RegionSet::new(vec![boundary.clone()]);

    let pca = pca_minor_and_elongation(boundary, stepover_mm);
    match pca {
        Some((minor, elongation)) => eprintln!(
            "   region 1 PCA: elongation {elongation:.2}, minor axis {minor:.1}° \
             (historical value on this region: {REF_PCA_MINOR_DEG}°)"
        ),
        None => eprintln!("   region 1 PCA: no axis (fewer than 3 sampled interior points)"),
    }

    let mut rows: Vec<(String, CandidateCost)> = Vec::new();
    let zero_grid = grid_for_direction(mesh, index, cutter, stepover_mm, 0.0);
    rows.push((
        "0° raster (ball)".to_owned(),
        relink_and_cost(
            raster_candidate(
                &zero_grid,
                std::slice::from_ref(boundary),
                safe_z,
                effective_min_z,
            ),
            mesh,
            index,
            cutter,
            &region,
            &kinematics,
            safe_z,
        ),
    ));
    if let Some((minor, _)) = pca {
        let rotated_grid = grid_for_direction(mesh, index, cutter, stepover_mm, minor);
        rows.push((
            format!("PCA-minor {minor:.1}° raster (ball)"),
            relink_and_cost(
                raster_candidate(
                    &rotated_grid,
                    std::slice::from_ref(boundary),
                    safe_z,
                    effective_min_z,
                ),
                mesh,
                index,
                cutter,
                &region,
                &kinematics,
                safe_z,
            ),
        ));
    }

    // The PCA-minor + monotone-CELLS baseline is deliberately absent — see
    // the note printed after the table.

    eprintln!(
        "\n     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10} {:>9}",
        "arm", "moves", "fragments", "linked", "retracts", "cut mm", "time s"
    );
    if let Some(outcome) = candidate {
        eprintln!(
            "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10.1} {:>9.1}",
            format!("direction field ({} curves)", outcome.cl_polylines),
            outcome.cost.moves,
            outcome.cost.fragments,
            outcome.cost.linked,
            outcome.cost.kept_retracts,
            outcome.cost.cutting_mm,
            outcome.cost.time_s
        );
    } else {
        eprintln!(
            "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10} {:>9}",
            "direction field", "—", "—", "—", "—", "—", "NOT COSTED"
        );
    }
    for (label, cost) in &rows {
        eprintln!(
            "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10.1} {:>9.1}",
            label,
            cost.moves,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.cutting_mm,
            cost.time_s
        );
    }

    let best_baseline = rows.iter().map(|(_, cost)| cost.time_s).reduce(f64::min);
    if let Some(outcome) = candidate
        && let Some(best) = best_baseline
        && outcome.cost.time_s > 0.0
    {
        eprintln!(
            "\n     candidate vs best ball baseline: {:.1} s vs {best:.1} s ({:.3}x)",
            outcome.cost.time_s,
            best / outcome.cost.time_s
        );
    }

    eprintln!("\n     -- CROSS-CUTTER CONTEXT — NOT THIS TABLE'S BAR --");
    eprintln!(
        "     The three rows below were measured with the TAPERED R1.5 ball\n\
         \x20    (planning/thin_organic_2026-08-27/FINDINGS.md). A different cutter is a\n\
         \x20    second variable: they are printed for orientation, and the F1 advance bar\n\
         \x20    ('beats 875.9 s by a material margin') must be re-cut on the ball fixture\n\
         \x20    before any candidate is compared to it."
    );
    eprintln!(
        "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10} {:>9}",
        "tapered R1.5: 0° undivided",
        "—",
        REF_RASTER0_FRAGMENTS,
        "—",
        97,
        "8687",
        format!("{REF_RASTER0_TIME_S:.1}")
    );
    eprintln!(
        "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10} {:>9}",
        "tapered R1.5: PCA-minor undivided",
        "—",
        REF_PCA_UNDIVIDED_FRAGMENTS,
        "—",
        74,
        "8501",
        format!("{REF_PCA_UNDIVIDED_TIME_S:.1}")
    );
    eprintln!(
        "     {:<34} {:>8} {:>10} {:>8} {:>9} {:>10} {:>9}",
        "tapered R1.5: PCA-minor CELLS",
        "—",
        REF_PCA_CELL_FRAGMENTS,
        "—",
        53,
        "8279",
        format!("{REF_PCA_CELL_TIME_S:.1}")
    );

    eprintln!(
        "\n     -- DEFERRED: the PCA-minor + monotone-CELLS ball-end baseline --\n\
         \x20    Restating `lattice_boustrophedon_cells` alone is ~64 lines, but it closes\n\
         \x20    over `polygons_for_lattice_cell` (~44), `runs_in_row`, `runs_overlap`,\n\
         \x20    `grid_frame_to_world` and `GridRun` (~32 more) — and a cells arm is a\n\
         \x20    LATTICE candidate, so design note §C.3 keeps `cell_membership_matches`\n\
         \x20    (~26 lines) authoritative over it. The reference instrument never costs a\n\
         \x20    cells arm without that gate. Total closure ~165 lines, well past the ~80\n\
         \x20    the brief allows, and a version without the membership gate would be the\n\
         \x20    'partial version' the brief forbids. Cut it deliberately as its own step\n\
         \x20    if the candidate survives this table.\n"
    );
}

// ── the staged evidence run ─────────────────────────────────────────────

/// Phase F1 steps 1–5, end to end.
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo) and the region-1 capture"]
fn wanaka_direction_field_f1() {
    use rs_cam_core::machine::kinematics::MachineKinematics;

    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let Some(boundary) = load_region1_capture() else {
        eprintln!(
            "SKIP: no region-1 capture at {} — run the Phase F1 step-0 capture first:\n\
             \x20 cargo test -p rs_cam_core --test wanaka_region_capture_f1 \\\n\
             \x20   capture_wanaka_region_1_boundary -- --ignored --nocapture",
            capture_path().display()
        );
        return;
    };

    let stepover_mm = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    eprintln!(
        "\n########## PHASE F1 — direction-field evidence on captured Wanaka region 1 ##########\n"
    );
    eprintln!(
        "   capture: {} ({:.1} mm², {} exterior vertices, {} holes)",
        capture_path().display(),
        boundary.area(),
        boundary.exterior.len(),
        boundary.holes.len()
    );
    eprintln!(
        "   NOTE: region identity comes from the FROZEN TAPERED chain in \
         wanaka_region_capture_f1.rs.\n\
         \x20  This file only LOADS it; drift detection is that file's sentry, not this one's.\n"
    );

    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    eprintln!(
        "   mesh: {} triangles; safe_z {:.3}, effective min_z {:.3}",
        mesh.triangles.len(),
        safe_z,
        mesh.bbox.min.z - 0.1
    );
    eprintln!(
        "   machine: accel [{:.0},{:.0},{:.0}] mm/s², junction dev {JUNCTION_DEVIATION_MM} mm, \
         rapid {RAPID_FEED_MM_MIN:.0}; feed {FEED_MM_MIN:.0}, plunge {PLUNGE_MM_MIN:.0} mm/min\n",
        MACHINE_ACCEL_XYZ[0], MACHINE_ACCEL_XYZ[1], MACHINE_ACCEL_XYZ[2]
    );

    // Region triangles: centroid-inside test. `Polygon2::contains_point`
    // honours holes, so a triangle over a hole is excluded.
    let region_triangles = direction_field::triangles_where(&mesh, |_, centroid| {
        boundary.contains_point(&P2::new(centroid.x, centroid.y))
    });
    if region_triangles.is_empty() {
        eprintln!("SKIP: no mesh triangle has its centroid inside the captured boundary.");
        return;
    }

    let (result, report) = direction_field::solve_field_paths(
        &mesh,
        &region_triangles,
        BALL_RADIUS_MM,
        CUSP_HEIGHT_MM,
    );

    let proceed = stage_a(&report, &result, stepover_mm);
    stage_b(&boundary, &result);
    if !proceed {
        eprintln!(
            "########## STOPPED after Stage B by the Stage A falsifier — C/D/E skipped. ##########\n"
        );
        return;
    }
    stage_c(&result, stepover_mm);
    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        kinematics,
        safe_z,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };
    let outcome = stage_d(fixture, &boundary, &result);
    stage_e(fixture, &boundary, stepover_mm, outcome.as_ref());
    eprintln!("########## PHASE F1 evidence run complete. ##########\n");
}

// ════════════════════════════════════════════════════════════════════════
// PHASE F1-S — the SEGMENTED direction-field arm on region 1
// ════════════════════════════════════════════════════════════════════════

// ── F1-S constants ──────────────────────────────────────────────────────

/// Monge-fit radius (mm). `wanaka_curvature_anisotropy.rs`'s own
/// `VERDICT_FIT_RADIUS_MM`: ≈3× the mesh's 0.335 mm median facet edge, the
/// radius its verdict reads and the one its scale rules cleared at.
const SEG_FIT_RADIUS_MM: f64 = 1.0;

/// Minimum gathered vertices for a fit: 6 quadric parameters + 1 degree of
/// freedom. **Restated** (`wanaka_curvature_anisotropy.rs:389`).
const SEG_MIN_FIT_POINTS: usize = 7;

/// Pivot ratio below which a fit is called rank deficient rather than solved.
/// **Restated** (`wanaka_curvature_anisotropy.rs:394`).
const SEG_MIN_PIVOT_RATIO: f64 = 1e-9;

/// Below this a 2-vector is treated as having no direction.
const SEG_EPS_VEC: f64 = 1e-12;

/// Below this, `k_s + 1/r` is treated as non-positive — the Eq. 13 gouge
/// degeneracy. Same role as `direction_field::EPS_DENOM`.
const SEG_EPS_DENOM: f64 = 1e-12;

/// **Restated from `direction_field::FieldParams`'s defaults** (`:172-179`),
/// because F1-S makes the same singular/degenerate call on its own estimator
/// and the two must agree on what "no preferred direction" means: a triangle
/// is singular when `|κ₁ − κ₂| ≤ max(abs, rel · max(|κ₁|, |κ₂|))`.
const SEG_ISOTROPY_REL_TOL: f64 = 0.10;
/// Absolute isotropy floor (1/mm) — see [`SEG_ISOTROPY_REL_TOL`].
const SEG_ISOTROPY_ABS_TOL: f64 = 1e-6;

/// The tolerance sweep. **180° first and it is the CONTROL**: at 180° the
/// admission gate never rejects, so the whole region grows as one patch —
/// same `V`, same code path, same estimator, and the decomposition is the
/// only variable against every row below it.
const SEG_TOLERANCES_DEG: [f64; 5] = [180.0, 45.0, 30.0, 20.0, 10.0];

/// A patch below this area (mm²) cannot hold even four passes at the 0.4862 mm
/// stepover, so it is counted as a **sliver**. It is still solved and still
/// costed — counted, never dropped.
const SEG_SLIVER_AREA_MM2: f64 = 1.0;

/// Arms emitting more polylines than this get a fragmentation-only row with no
/// cost. Same number and the same reasoning as [`FALSIFIER_MULTIPLE`] ×
/// [`REF_PCA_CELL_FRAGMENTS`] = 705: Stage A already rules that past this
/// point "there is nothing a relinker can do that the straight sweeps have not
/// already been measured doing", and relinking thousands of fragments over a
/// 661k-triangle mesh has never been run — the F1 falsifier stopped before
/// F1-D every time.
const SEG_COST_CEILING_POLYLINES: usize = REF_PCA_CELL_FRAGMENTS * FALSIFIER_MULTIPLE;

/// The §F1-1 whole-region result, for the historical row. **Measured with the
/// Rusinkiewicz per-vertex estimator inside `direction_field`, not with this
/// file's Monge fit** — see the pre-registration.
const REF_F1_WHOLE_REGION_POLYLINES: usize = 6_589;
/// Levels in that same run.
const REF_F1_WHOLE_REGION_LEVELS: usize = 253;
/// Median components per level in that same run — the mechanism number.
const REF_F1_WHOLE_REGION_MEDIAN_COMPONENTS: usize = 28;
/// Orientation inconsistencies in that same run.
const REF_F1_WHOLE_REGION_INCONSISTENCIES: usize = 2_340;

/// Region 1's measured prize ceiling at R = 1.0
/// (`planning/finishing_synthesis_2026-08-30.md` §11).
const REGION1_PRIZE_CEILING_PCT: f64 = 9.75;

/// This fixture's median facet edge (mm), from the anisotropy instrument.
const WANAKA_MEDIAN_FACET_EDGE_MM: f64 = 0.335;

/// The honesty rail every distance and time figure in F1-S carries.
const FIXTURE_LIMIT_LABEL: &str = "FIXTURE-LIMITED: wanaka's median facet edge is 0.335 mm against a \
     0.4862 mm stepover\n\
     \x20    (ratio 0.69), below this programme's >=3x bar (§F2-1 withdrawal). SPACING AND \n\
     \x20    CUTTING-DISTANCE FIGURES ON THIS MESH ARE NOT TRUSTWORTHY, and no cost number \n\
     \x20    here may be quoted as a verdict. FRAGMENTATION IS A TOPOLOGICAL COUNT and is \n\
     \x20    unaffected — it is the headline.";

// ── restated Monge-quadric machinery ────────────────────────────────────

/// Per-sample reusable buffers. **Restated from
/// `wanaka_curvature_anisotropy.rs:440-460`**, minus its rayon `map_init`
/// wrapper: F1-S fits one triangle centroid at a time on a single thread
/// (~44k fits, a fraction of one Poisson solve), so nothing is shared.
struct Scratch {
    query: QueryScratch,
    tris: Vec<usize>,
    /// Generation stamp per mesh vertex — dedups the triangle→vertex expansion
    /// without clearing a bitset per sample.
    stamp: Vec<u32>,
    generation: u32,
}

impl Scratch {
    fn new(vertex_count: usize) -> Self {
        Self {
            query: QueryScratch::default(),
            tris: Vec::new(),
            // Stamps start at 0 and `generation` is incremented *before* use.
            stamp: vec![0u32; vertex_count],
            generation: 0,
        }
    }
}

/// The local differential geometry at one triangle centroid.
///
/// **Restated from `wanaka_curvature_anisotropy.rs:464-490`** with the
/// reporting-only fields (`residual_rms`, `points`, `gather_rms`,
/// `pivot_ratio`, `area_weight`) trimmed — F1-S reports a census of fit
/// *outcomes*, not per-fit conditioning — and **extended** with `axis`, the
/// principal direction the anisotropy instrument never needed.
#[derive(Clone, Copy)]
struct Fit {
    /// Max principal curvature, **convex-positive** (Zou). `kappa1 >= kappa2`.
    kappa1: f64,
    /// Min principal curvature, convex-positive.
    kappa2: f64,
    /// First fundamental form.
    form_e: f64,
    form_f: f64,
    form_g: f64,
    /// Second fundamental form, Monge/upward-normal (convex reads negative;
    /// [`kappa_perp_zou`] applies the flip).
    form_l: f64,
    form_m: f64,
    form_n: f64,
    /// Unit **XY** direction of `kappa1` — the `t₁` line field. `None` at an
    /// umbilic, where the shape operator is a multiple of the identity and no
    /// direction exists. Sign is arbitrary: this is a LINE field.
    axis: Option<[f64; 2]>,
}

/// Why a sample produced no fit — counted, never silently dropped.
enum FitOutcome {
    Fitted(Fit),
    /// Fewer than [`SEG_MIN_FIT_POINTS`] vertices in the disc.
    UnderDetermined,
    /// Rank-deficient normal matrix.
    IllConditioned,
}

/// Solve the symmetric 6×6 system `A x = b` by Gauss-Jordan with partial
/// pivoting, returning `(x, min|pivot| / max|pivot|)`. **Restated verbatim
/// from `wanaka_curvature_anisotropy.rs:547-602`**, `#[allow]` included.
#[allow(clippy::needless_range_loop)] // Gauss-Jordan indexes three arrays by the same counter.
fn solve_sym6(a: &[[f64; 6]; 6], b: &[f64; 6]) -> Option<([f64; 6], f64)> {
    let mut m = [[0.0f64; 7]; 6];
    for row in 0..6 {
        for col in 0..6 {
            m[row][col] = a[row][col];
        }
        m[row][6] = b[row];
    }
    let mut pivot_min = f64::INFINITY;
    let mut pivot_max = 0.0f64;
    for col in 0..6 {
        let mut best = col;
        for row in (col + 1)..6 {
            if m[row][col].abs() > m[best][col].abs() {
                best = row;
            }
        }
        m.swap(col, best);
        let pivot = m[col][col];
        let mag = pivot.abs();
        pivot_min = pivot_min.min(mag);
        pivot_max = pivot_max.max(mag);
        if mag < f64::MIN_POSITIVE {
            return None;
        }
        let inv = 1.0 / pivot;
        for k in col..7 {
            m[col][k] *= inv;
        }
        for row in 0..6 {
            if row == col {
                continue;
            }
            let factor = m[row][col];
            if factor == 0.0 {
                continue;
            }
            for k in col..7 {
                let pivot_row = m[col][k];
                m[row][k] -= factor * pivot_row;
            }
        }
    }
    let mut x = [0.0f64; 6];
    for row in 0..6 {
        x[row] = m[row][6];
    }
    let ratio = if pivot_max > 0.0 {
        pivot_min / pivot_max
    } else {
        0.0
    };
    Some((x, ratio))
}

/// Mirror the accumulated upper triangle into the lower one and scale by
/// `inv = 1/count`. **Restated verbatim from
/// `wanaka_curvature_anisotropy.rs:652-664`**, `#[allow]` included.
#[allow(clippy::needless_range_loop)] // symmetric mirror indexes both [i][j] and [j][i]
fn finalise_normal_equations(normal: &mut [[f64; 6]; 6], rhs: &mut [f64; 6], inv: f64) {
    for i in 0..6 {
        for j in 0..6 {
            if j < i {
                let mirrored = normal[j][i];
                normal[i][j] = mirrored;
            } else {
                normal[i][j] *= inv;
            }
        }
        rhs[i] *= inv;
    }
}

/// Build a [`Fit`] from the derivatives of the fitted heightfield.
///
/// **Restated from `wanaka_curvature_anisotropy.rs:606-642` and EXTENDED**
/// with the principal direction, which that instrument never needed because
/// it only ever asked for `κ` along a *given* direction.
///
/// # Where `t₁` comes from, and why the smaller eigenvalue
///
/// The shape operator is `S = I⁻¹ II`, so with `det = EG − F²`
///
/// ```text
///   S = 1/det · [[ GL − FM ,  GM − FN ],
///                [ EM − FL ,  EN − FM ]]
/// ```
///
/// whose trace/2 is the instrument's `mean` and whose determinant is its
/// `gauss`; the eigenvalues are therefore `mean ± spread` exactly. The
/// instrument's convex-positive flip is `kappa1 = −(mean − spread)`, so the
/// eigenvalue belonging to `κ₁` is the **smaller** one, `mean − spread`.
///
/// That is the direction `direction_field` wants: its module header states
/// `D = t₁` is the *maximum signed principal direction*, chosen so that the
/// *minimum* signed curvature sits perpendicular and the side-step
/// `√(8h/(k_s + 1/r))` is widest. Worked check, on the bowl
/// `z = ½(A x² + B y²)` with `A > B > 0`: `S = diag(A, B)`, the smaller
/// eigenvalue is `B`, its eigenvector is `ŷ`, and `κ₁ = −B ≥ −A = κ₂`. Feed
/// along `ŷ` (the gentler direction) and step over in `x̂` (the sharply
/// concave one) — which is where the ball fits deepest and the strip is
/// widest. Pinned by [`shape_operator_t1_is_the_gentle_direction_of_a_bowl`].
///
/// A parameter-space direction `(p, q)` on a Monge patch is
/// `p·(1,0,f_x) + q·(0,1,f_y)`, whose XY part is exactly `(p, q)` — so the
/// eigenvector IS the XY direction, no conversion needed.
fn fit_from_derivatives((f_x, f_y): (f64, f64), (f_xx, f_xy, f_yy): (f64, f64, f64)) -> Fit {
    let area_weight = (1.0 + f_x * f_x + f_y * f_y).sqrt();
    let form_e = 1.0 + f_x * f_x;
    let form_f = f_x * f_y;
    let form_g = 1.0 + f_y * f_y;
    let form_l = f_xx / area_weight;
    let form_m = f_xy / area_weight;
    let form_n = f_yy / area_weight;
    // EG − F² = 𝒲² ≥ 1: no degenerate metric here.
    let det = form_e * form_g - form_f * form_f;
    let gauss = (form_l * form_n - form_m * form_m) / det;
    let mean = (form_e * form_n - 2.0 * form_f * form_m + form_g * form_l) / (2.0 * det);
    let spread = (mean * mean - gauss).max(0.0).sqrt();

    // Shape operator entries, S = I⁻¹ II.
    let s_a = (form_g * form_l - form_f * form_m) / det;
    let s_b = (form_g * form_m - form_f * form_n) / det;
    let s_c = (form_e * form_m - form_f * form_l) / det;
    let s_d = (form_e * form_n - form_f * form_m) / det;
    let lambda = mean - spread;
    // Both rows of (S − λI)v = 0; the better-conditioned one wins.
    let row1 = [s_b, lambda - s_a];
    let row2 = [s_d - lambda, -s_c];
    let n1 = (row1[0] * row1[0] + row1[1] * row1[1]).sqrt();
    let n2 = (row2[0] * row2[0] + row2[1] * row2[1]).sqrt();
    // Relative floor: on this terrain the operator's entries are ~1e-3 1/mm,
    // so an absolute epsilon would call nothing umbilic.
    let magnitude = s_a.abs().max(s_b.abs()).max(s_c.abs()).max(s_d.abs());
    let floor = (magnitude * 1e-9).max(f64::MIN_POSITIVE);
    let axis = if n1 >= n2 && n1 > floor {
        Some([row1[0] / n1, row1[1] / n1])
    } else if n2 > floor {
        Some([row2[0] / n2, row2[1] / n2])
    } else {
        None
    };

    Fit {
        // Convex-positive flip.
        kappa1: -(mean - spread),
        kappa2: -(mean + spread),
        form_e,
        form_f,
        form_g,
        form_l,
        form_m,
        form_n,
        axis,
    }
}

/// Fit the local quadric at `at` (surface height `z0`) over `radius`.
/// **Restated from `wanaka_curvature_anisotropy.rs:668-761`**, with its
/// residual/gather reporting trimmed (F1-S reports outcome counts, not
/// per-fit conditioning) — the normal equations, the scale normalisation
/// `u = (x − x₀)/r` and the two refusal arms are unchanged.
fn fit_quadric(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    scratch: &mut Scratch,
    (at, z0): (P2, f64),
    radius: f64,
) -> FitOutcome {
    let mut tris = std::mem::take(&mut scratch.tris);
    index.query_rect_into(
        at.x - radius,
        at.x + radius,
        at.y - radius,
        at.y + radius,
        &mut scratch.query,
        &mut tris,
    );
    scratch.generation = scratch.generation.wrapping_add(1);
    let generation = scratch.generation;

    let mut normal = [[0.0f64; 6]; 6];
    let mut rhs = [0.0f64; 6];
    let mut count = 0usize;
    let r2 = radius * radius;

    for &t in &tris {
        for &vi in &mesh.triangles[t] {
            let vi = vi as usize;
            if scratch.stamp[vi] == generation {
                continue;
            }
            scratch.stamp[vi] = generation;
            let p = mesh.vertices[vi];
            let dx = p.x - at.x;
            let dy = p.y - at.y;
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let u = dx / radius;
            let v = dy / radius;
            let w = p.z - z0;
            let basis = [u * u, u * v, v * v, u, v, 1.0];
            for (i, &bi) in basis.iter().enumerate() {
                for (j, &bj) in basis.iter().enumerate().skip(i) {
                    normal[i][j] += bi * bj;
                }
                rhs[i] += bi * w;
            }
            count += 1;
        }
    }
    scratch.tris = tris;

    if count < SEG_MIN_FIT_POINTS {
        return FitOutcome::UnderDetermined;
    }
    finalise_normal_equations(&mut normal, &mut rhs, 1.0 / count as f64);

    let Some((beta, pivot_ratio)) = solve_sym6(&normal, &rhs) else {
        return FitOutcome::IllConditioned;
    };
    if !pivot_ratio.is_finite() || pivot_ratio < SEG_MIN_PIVOT_RATIO {
        return FitOutcome::IllConditioned;
    }
    if !beta.iter().all(|c| c.is_finite()) {
        return FitOutcome::IllConditioned;
    }

    let f_x = beta[3] / radius;
    let f_y = beta[4] / radius;
    let f_xx = 2.0 * beta[0] / (radius * radius);
    let f_xy = beta[1] / (radius * radius);
    let f_yy = 2.0 * beta[2] / (radius * radius);
    FitOutcome::Fitted(fit_from_derivatives((f_x, f_y), (f_xx, f_xy, f_yy)))
}

/// Normal curvature **perpendicular** to the XY direction `dir`,
/// convex-positive. **Restated verbatim from
/// `wanaka_curvature_anisotropy.rs:779-791`** including its proof that
/// `(p′, q′) = (−(Fp + Gq), Ep + Fq)` is the tangent-plane perpendicular in
/// the surface metric. This is Eq. 13's `k_s`.
fn kappa_perp_zou(fit: &Fit, dir: [f64; 2]) -> f64 {
    let (p, q) = (dir[0], dir[1]);
    let pp = -(fit.form_f * p + fit.form_g * q);
    let qq = fit.form_e * p + fit.form_f * q;
    let num = fit.form_l * pp * pp + 2.0 * fit.form_m * pp * qq + fit.form_n * qq * qq;
    let den = fit.form_e * pp * pp + 2.0 * fit.form_f * pp * qq + fit.form_g * qq * qq;
    if den.abs() < SEG_EPS_DENOM {
        return 0.5 * (fit.kappa1 + fit.kappa2);
    }
    -num / den
}

// ── per-triangle geometry over the region ───────────────────────────────

/// Everything F1-S needs about one region triangle.
struct TriGeom {
    /// Unit XY `t₁` axis, sign arbitrary. `None` when the fit failed or the
    /// point is umbilic.
    axis: Option<[f64; 2]>,
    /// `|κ₁ − κ₂|` — the anisotropy that seeds the growth and that decides
    /// singularity.
    anisotropy: f64,
    /// `|V|` of Eq. 13, already clamped. Computed **once** for the whole
    /// region and shared by every arm — see [`region_geometry`].
    magnitude: f64,
    /// Surface area (mm²), for the sliver census.
    area_mm2: f64,
    /// Fitted AND above the isotropy floor: may seed a patch and may be gated
    /// on. An untrusted triangle is admitted to whichever patch reaches it
    /// first and adopts that patch's direction.
    trusted: bool,
}

/// Fit outcomes over the region, all counted.
#[derive(Default)]
struct GeomCensus {
    fitted: usize,
    under_determined: usize,
    ill_conditioned: usize,
    /// Fitted, but the shape operator is a multiple of the identity.
    umbilic: usize,
    /// Fitted with an axis, but `|κ₁ − κ₂|` at or below the isotropy floor —
    /// `t₁` exists numerically and is not believed.
    below_isotropy_floor: usize,
    /// `k_s + 1/r ≤ 0` (the ball does not fit the concavity), so `|V|` fell
    /// back to the region minimum. Same guard `direction_field` applies.
    clamped_magnitude: usize,
}

impl GeomCensus {
    /// Triangles with no trustworthy preferred direction — the population the
    /// literature would send to degeneracy classification and separatrix
    /// tracing, and which F1-S instead lets adopt a neighbour's direction.
    fn untrusted(&self) -> usize {
        self.under_determined + self.ill_conditioned + self.umbilic + self.below_isotropy_floor
    }
}

/// Surface area (mm²) of one mesh triangle.
fn triangle_area_mm2(mesh: &TriangleMesh, global_tri: u32) -> f64 {
    let tri = mesh.triangles[global_tri as usize];
    let a = mesh.vertices[tri[0] as usize];
    let b = mesh.vertices[tri[1] as usize];
    let c = mesh.vertices[tri[2] as usize];
    0.5 * (b - a).cross(&(c - a)).norm()
}

/// Fit every region triangle and build the shared target-field magnitudes.
///
/// The magnitudes are built **once, region-wide, and reused by every arm**.
/// That is exact rather than approximate: `k_s` is a quadratic form, so it
/// does not depend on the *sign* the segmentation gives `t₁`, only on its
/// axis — and the axis is a property of the fit, not of the decomposition. On
/// an untrusted triangle the axis is either absent (region-minimum fallback,
/// counted) or near-umbilic, where `k_s` is very nearly the same in every
/// direction anyway. So `V` differs between arms only in the **sign** of the
/// step-over direction, which is exactly the variable segmentation exists to
/// control.
fn region_geometry(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    region_tris: &[u32],
    ball_radius_mm: f64,
) -> (Vec<TriGeom>, GeomCensus) {
    let inv_r = if ball_radius_mm > SEG_EPS_DENOM {
        1.0 / ball_radius_mm
    } else {
        0.0
    };
    /// One triangle before the region-wide magnitude fallback is known.
    /// `magnitude` is `None` exactly when `k_s + 1/r <= 0`.
    struct Pending {
        axis: Option<[f64; 2]>,
        anisotropy: f64,
        magnitude: Option<f64>,
        area_mm2: f64,
        trusted: bool,
    }

    let mut scratch = Scratch::new(mesh.vertices.len());
    let mut census = GeomCensus::default();
    let mut partial: Vec<Pending> = Vec::with_capacity(region_tris.len());

    for &global in region_tris {
        let tri = mesh.triangles[global as usize];
        let a = mesh.vertices[tri[0] as usize];
        let b = mesh.vertices[tri[1] as usize];
        let c = mesh.vertices[tri[2] as usize];
        let centre = P2::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
        let z0 = (a.z + b.z + c.z) / 3.0;
        let area = triangle_area_mm2(mesh, global);

        match fit_quadric(mesh, index, &mut scratch, (centre, z0), SEG_FIT_RADIUS_MM) {
            FitOutcome::Fitted(fit) => {
                census.fitted += 1;
                let anisotropy = (fit.kappa1 - fit.kappa2).abs();
                let scale = fit.kappa1.abs().max(fit.kappa2.abs());
                let floor = SEG_ISOTROPY_ABS_TOL.max(SEG_ISOTROPY_REL_TOL * scale);
                let singular = anisotropy <= floor;
                if fit.axis.is_none() {
                    census.umbilic += 1;
                } else if singular {
                    census.below_isotropy_floor += 1;
                }
                let k_s = match fit.axis {
                    Some(axis) => kappa_perp_zou(&fit, axis),
                    None => 0.5 * (fit.kappa1 + fit.kappa2),
                };
                let denominator = k_s + inv_r;
                let magnitude = (denominator > SEG_EPS_DENOM).then(|| (denominator / 8.0).sqrt());
                partial.push(Pending {
                    axis: fit.axis,
                    anisotropy,
                    magnitude,
                    area_mm2: area,
                    trusted: fit.axis.is_some() && !singular,
                });
            }
            FitOutcome::UnderDetermined => {
                census.under_determined += 1;
                partial.push(Pending {
                    axis: None,
                    anisotropy: 0.0,
                    magnitude: None,
                    area_mm2: area,
                    trusted: false,
                });
            }
            FitOutcome::IllConditioned => {
                census.ill_conditioned += 1;
                partial.push(Pending {
                    axis: None,
                    anisotropy: 0.0,
                    magnitude: None,
                    area_mm2: area,
                    trusted: false,
                });
            }
        }
    }

    // Region-wide fallback magnitude, exactly as `direction_field`'s
    // `build_target_field` does it: the smallest valid magnitude, or the
    // flat-surface magnitude when nothing on the region is valid.
    let min_valid = partial
        .iter()
        .filter_map(|entry| entry.magnitude)
        .fold(f64::INFINITY, f64::min);
    let fallback = if min_valid.is_finite() {
        min_valid
    } else {
        (inv_r / 8.0).sqrt()
    };

    let geom = partial
        .into_iter()
        .map(|entry| {
            let magnitude = match entry.magnitude {
                Some(value) => value,
                None => {
                    census.clamped_magnitude += 1;
                    fallback
                }
            };
            TriGeom {
                axis: entry.axis,
                anisotropy: entry.anisotropy,
                magnitude,
                area_mm2: entry.area_mm2,
                trusted: entry.trusted,
            }
        })
        .collect();
    (geom, census)
}

/// Compact the region into its own [`TriangleMesh`], triangle `i` of the
/// submesh being region slot `i`.
///
/// **This is a performance requirement, not a convenience.**
/// `direction_field::build_region_mesh` calls
/// `pencil_dihedral::build_edge_adjacency(mesh)` — a **whole-mesh** edge→face
/// map — on *every* `solve_paths_with_target` call. F1-S makes one call per
/// patch, and a fine tolerance produces thousands of patches, so against the
/// 661k-triangle wanaka mesh that is thousands of 661k-triangle hashes. Against
/// the ~44k-triangle submesh it is 15× smaller each time.
///
/// It is also **semantically identical**: `build_region_mesh` already filters
/// the edge map through `global_to_local`, so a neighbour outside the region
/// was never reachable; vertex positions, `+Z`-oriented normals and areas are
/// copied unchanged. Curvature is NOT computed here — [`region_geometry`] fits
/// against the **full** mesh, so a region-border triangle still sees the
/// terrain around it, which is the same rule `direction_field` states for its
/// own estimator.
fn region_submesh(mesh: &TriangleMesh, region_tris: &[u32]) -> TriangleMesh {
    let mut vertex_map: HashMap<u32, u32> = HashMap::new();
    let mut vertices: Vec<P3> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::with_capacity(region_tris.len());
    for &global in region_tris {
        let tri = mesh.triangles[global as usize];
        let mut local = [0u32; 3];
        for (slot, &vertex) in local.iter_mut().zip(tri.iter()) {
            let next = vertices.len() as u32;
            let id = *vertex_map.entry(vertex).or_insert(next);
            if id == next {
                vertices.push(mesh.vertices[vertex as usize]);
            }
            *slot = id;
        }
        triangles.push(local);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Edge adjacency over the region, as region-slot neighbour lists.
///
/// Keyed on the welded global vertex pair (the STL loader welds; §F1-1's
/// 43,783 triangles over 23,641 vertices is the proof), sorted rather than
/// hashed so the neighbour order — and therefore the growth — is
/// deterministic. A non-manifold edge with more than two incident region
/// triangles links all of them pairwise.
fn region_edge_adjacency(mesh: &TriangleMesh, region_tris: &[u32]) -> Vec<Vec<usize>> {
    let mut keyed: Vec<(u64, usize)> = Vec::with_capacity(region_tris.len() * 3);
    for (slot, &global) in region_tris.iter().enumerate() {
        let tri = mesh.triangles[global as usize];
        for corner in 0..3 {
            let a = tri[corner];
            let b = tri[(corner + 1) % 3];
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            keyed.push(((u64::from(lo) << 32) | u64::from(hi), slot));
        }
    }
    keyed.sort_unstable();

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); region_tris.len()];
    let mut start = 0usize;
    while start < keyed.len() {
        let mut end = start + 1;
        while end < keyed.len() && keyed[end].0 == keyed[start].0 {
            end += 1;
        }
        for left in start..end {
            for right in (left + 1)..end {
                let (x, y) = (keyed[left].1, keyed[right].1);
                if x != y {
                    adjacency[x].push(y);
                    adjacency[y].push(x);
                }
            }
        }
        start = end;
    }
    for list in &mut adjacency {
        list.sort_unstable();
        list.dedup();
    }
    adjacency
}

// ── [REPO] the segmentation stage ───────────────────────────────────────

/// Which reference direction the admission gate compares a candidate against.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Criterion {
    /// The patch's running mean direction — the brief's specification. A patch
    /// cannot bend away from where it started.
    RunningMean,
    /// The direction of the triangle being expanded from — closer to Zou
    /// Eq. 18, which is a metric between two *neighbouring* points and lets a
    /// patch follow a gradual bend. Carried as an uncosted sensitivity arm.
    NeighbourLocal,
}

impl Criterion {
    fn tag(self) -> &'static str {
        match self {
            Self::RunningMean => "mean",
            Self::NeighbourLocal => "nbr",
        }
    }
}

/// The outcome of one segmentation, everything counted.
struct Segmentation {
    /// One `Vec` per patch, holding whatever ids the caller passed as
    /// `region_tris` — in the evidence run those are **submesh** triangle
    /// indices, which is what [`solve_segmented`] and [`segmented_svg`] both
    /// want.
    patches: Vec<Vec<u32>>,
    /// Signed unit XY direction per region slot, after orientation.
    oriented: Vec<[f64; 2]>,
    /// Patch areas (mm²), parallel to `patches`.
    areas: Vec<f64>,
    /// Patches below [`SEG_SLIVER_AREA_MM2`]. Counted, still solved, still
    /// costed.
    slivers: usize,
    /// Their total area (mm²).
    sliver_area_mm2: f64,
    /// Triangles admitted with no trustworthy `t₁`, which adopted a
    /// neighbour's direction instead. The literature would classify these and
    /// trace separatrices through them; F1-S does neither.
    adopted_untrusted: usize,
    /// Patches grown by the leftover flood — components consisting entirely of
    /// untrusted triangles, unreachable from any seeded patch.
    untrusted_only_patches: usize,
    /// Edge-adjacent pairs inside one patch whose oriented directions
    /// disagree (`dot < 0`). The analogue of §F1-1's 2,340.
    inconsistencies: usize,
    /// Edge-adjacent pairs inside one patch, the denominator for the above.
    same_patch_pairs: usize,
}

/// `v / |v|`, or `None` when `v` has no direction.
fn normalise2(v: [f64; 2]) -> Option<[f64; 2]> {
    let length = (v[0] * v[0] + v[1] * v[1]).sqrt();
    (length > SEG_EPS_VEC).then(|| [v[0] / length, v[1] / length])
}

fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// **[REPO]** Segment the region by direction coherence — a defensible cheap
/// stand-in for Zou §4.3, which is Eq. 18's closeness metric
/// `exp(−(1 − d₁·d₂)²/2σ²)` (σ = 0.67) fed through **Laplacian Eigenmaps**
/// and **K-Means**. Neither of those is built here.
///
/// What is built instead: greedy region growing over triangle edge adjacency,
/// seeded at the most anisotropic unassigned triangle (ties by triangle id, so
/// the run is deterministic), admitting a neighbour while its `t₁` stays
/// within `tolerance_deg` of the [`Criterion`]'s reference direction. `t₁` is
/// a **line** field, so the gate is `|d₁·d₂| ≥ cos(tolerance)` — a direction
/// and its negation are the same feed.
///
/// Three properties of this stand-in an eigenmap+K-Means implementation would
/// not share, all of them disclosed rather than tuned away:
///
/// * `k` is not chosen — patch count is whatever the tolerance produces. That
///   is arguably an improvement: the paper never states how its `k` was
///   picked (extraction gap 7).
/// * Patches are **edge-connected by construction**, which the paper's 1-D
///   embedding + K-Means does not guarantee. That matters here: a disconnected
///   patch would re-introduce the very threading the segmentation is meant to
///   stop.
/// * A patch grows greedily from one seed rather than being cut at a global
///   optimum, so the *boundaries* are not the paper's. The authors call patch
///   borders "a serious limitation" of their own method, so neither placement
///   is a clean answer.
///
/// **Orientation is separate from admission.** The sign of an admitted
/// direction is always fixed against the triangle it was reached from — that
/// is §4.2's BFS flip rule, and it is identical in every arm — while only the
/// *gate* reads the [`Criterion`]. Without that split the 180° control would
/// be orienting against a global mean rather than propagating, and it would
/// not be a control for anything.
///
/// An **untrusted** triangle (no fit, umbilic, or below the isotropy floor) is
/// never a seed and is never gated: it joins whichever patch reaches it first
/// and adopts that patch's direction. This is where a real implementation
/// would classify the degeneracy and trace a separatrix; F1-S counts the
/// population instead and prints it, so a reader can see how much of the
/// region is being carried by a fallback.
fn segment_region(
    region_tris: &[u32],
    geom: &[TriGeom],
    adjacency: &[Vec<usize>],
    tolerance_deg: f64,
    criterion: Criterion,
) -> Segmentation {
    let count = region_tris.len();
    let cos_tolerance = tolerance_deg.to_radians().cos();
    let mut patch_of = vec![usize::MAX; count];
    let mut oriented = vec![[0.0f64, 0.0f64]; count];
    let mut patches: Vec<Vec<u32>> = Vec::new();
    let mut adopted_untrusted = 0usize;

    // Seed order: most anisotropic first, ties by slot so the run repeats.
    let mut seeds: Vec<usize> = (0..count).filter(|&slot| geom[slot].trusted).collect();
    seeds.sort_by(|&a, &b| {
        geom[b]
            .anisotropy
            .total_cmp(&geom[a].anisotropy)
            .then(a.cmp(&b))
    });

    for &seed in &seeds {
        if patch_of[seed] != usize::MAX {
            continue;
        }
        let Some(seed_axis) = geom[seed].axis else {
            continue;
        };
        let patch = patches.len();
        patch_of[seed] = patch;
        oriented[seed] = seed_axis;
        let mut sum = seed_axis;
        let mut members = vec![region_tris[seed]];
        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(seed);

        while let Some(current) = queue.pop_front() {
            let from = oriented[current];
            for &next in &adjacency[current] {
                if patch_of[next] != usize::MAX {
                    continue;
                }
                let gate_reference = match criterion {
                    Criterion::RunningMean => normalise2(sum).unwrap_or(from),
                    Criterion::NeighbourLocal => from,
                };
                let direction = match geom[next].axis {
                    Some(axis) if geom[next].trusted => {
                        if dot2(axis, gate_reference).abs() < cos_tolerance {
                            continue;
                        }
                        // §4.2 BFS flip: sign always against the neighbour.
                        if dot2(axis, from) < 0.0 {
                            [-axis[0], -axis[1]]
                        } else {
                            axis
                        }
                    }
                    _ => {
                        adopted_untrusted += 1;
                        from
                    }
                };
                patch_of[next] = patch;
                oriented[next] = direction;
                sum = [sum[0] + direction[0], sum[1] + direction[1]];
                members.push(region_tris[next]);
                queue.push_back(next);
            }
        }
        patches.push(members);
    }

    // Leftovers: connected components made entirely of untrusted triangles,
    // which no seeded patch could reach. Flooded into their own patches rather
    // than left as singletons, and counted.
    let mut untrusted_only_patches = 0usize;
    for start in 0..count {
        if patch_of[start] != usize::MAX {
            continue;
        }
        let patch = patches.len();
        let base = geom[start].axis.unwrap_or([1.0, 0.0]);
        patch_of[start] = patch;
        oriented[start] = base;
        let mut members = vec![region_tris[start]];
        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(start);
        while let Some(current) = queue.pop_front() {
            let from = oriented[current];
            for &next in &adjacency[current] {
                if patch_of[next] != usize::MAX {
                    continue;
                }
                let axis = geom[next].axis.unwrap_or(from);
                let direction = if dot2(axis, from) < 0.0 {
                    [-axis[0], -axis[1]]
                } else {
                    axis
                };
                patch_of[next] = patch;
                oriented[next] = direction;
                members.push(region_tris[next]);
                queue.push_back(next);
            }
        }
        patches.push(members);
        untrusted_only_patches += 1;
    }

    let mut areas = vec![0.0f64; patches.len()];
    for slot in 0..count {
        areas[patch_of[slot]] += geom[slot].area_mm2;
    }
    let slivers = areas.iter().filter(|a| **a < SEG_SLIVER_AREA_MM2).count();
    let sliver_area_mm2 = areas
        .iter()
        .filter(|a| **a < SEG_SLIVER_AREA_MM2)
        .sum::<f64>();

    let mut inconsistencies = 0usize;
    let mut same_patch_pairs = 0usize;
    for slot in 0..count {
        for &next in &adjacency[slot] {
            if next <= slot || patch_of[next] != patch_of[slot] {
                continue;
            }
            same_patch_pairs += 1;
            if dot2(oriented[slot], oriented[next]) < 0.0 {
                inconsistencies += 1;
            }
        }
    }

    Segmentation {
        patches,
        oriented,
        areas,
        slivers,
        sliver_area_mm2,
        adopted_untrusted,
        untrusted_only_patches,
        inconsistencies,
        same_patch_pairs,
    }
}

// ── the per-patch solve ─────────────────────────────────────────────────

/// What one arm's solve produced.
struct SolveOutcome {
    polylines: Vec<Vec<P3>>,
    /// Sum over patches of that patch's level count — the denominator of the
    /// mechanism check.
    levels_total: usize,
    /// The busiest single patch's level count.
    max_levels_in_a_patch: usize,
    /// Patches whose CG did not reach tolerance.
    non_converged: usize,
    /// Patches that hit `FieldParams::max_levels`.
    level_cap_hits: usize,
    /// Patches that produced no polyline at all.
    empty_patches: usize,
}

/// Solve Eq. 15 **per patch** with the shared target field, and concatenate.
///
/// `mesh` is the region SUBMESH from [`region_submesh`], so `slot_of` maps its
/// triangle index to a region slot and is the identity — it is still passed
/// explicitly rather than assumed, because a silent identity is exactly the
/// kind of assumption that survives a refactor and then indexes the wrong
/// array. A lookup outside `geom` yields a zero field rather than a panic.
///
/// Every arm calls
/// [`rs_cam_core::finish::direction_field::solve_paths_with_target`] with the same
/// magnitudes and the same lift; the patch list is the only thing that moves.
fn solve_segmented(
    mesh: &TriangleMesh,
    slot_of: &[u32],
    geom: &[TriGeom],
    seg: &Segmentation,
    params: &direction_field::FieldParams,
) -> SolveOutcome {
    let mut outcome = SolveOutcome {
        polylines: Vec::new(),
        levels_total: 0,
        max_levels_in_a_patch: 0,
        non_converged: 0,
        level_cap_hits: 0,
        empty_patches: 0,
    };

    for patch in &seg.patches {
        let (result, report) = direction_field::solve_paths_with_target(
            mesh,
            patch,
            |global, _centroid, normal| {
                let slot = slot_of[global] as usize;
                let Some(entry) = geom.get(slot) else {
                    return V3::zeros();
                };
                let planar_xy = seg.oriented[slot];
                let planar = V3::new(planar_xy[0], planar_xy[1], 0.0);
                // Lift the XY axis into the triangle plane. On a heightfield
                // facet the +Z-oriented normal is never parallel to a
                // horizontal vector, so the fallback is unreachable in
                // practice and exists so the field can never be NaN.
                let tangent = planar - normal * planar.dot(&normal);
                let feed = if tangent.norm() > SEG_EPS_VEC {
                    tangent.normalize()
                } else {
                    let alternative = normal.cross(&V3::new(0.0, 0.0, 1.0));
                    if alternative.norm() > SEG_EPS_VEC {
                        alternative.normalize()
                    } else {
                        V3::new(1.0, 0.0, 0.0)
                    }
                };
                // Eq. 13: D rotated 90° about the normal, scaled by |V|.
                let step_over = normal.cross(&feed);
                if step_over.norm() > SEG_EPS_VEC {
                    step_over.normalize() * entry.magnitude
                } else {
                    V3::zeros()
                }
            },
            params,
        );
        outcome.levels_total += result.levels.len();
        outcome.max_levels_in_a_patch = outcome.max_levels_in_a_patch.max(result.levels.len());
        if !report.cg_converged {
            outcome.non_converged += 1;
        }
        if report.level_cap_hit {
            outcome.level_cap_hits += 1;
        }
        if result.polylines.is_empty() {
            outcome.empty_patches += 1;
        }
        outcome.polylines.extend(result.polylines);
    }
    outcome
}

// ── one row of the F1-S table ───────────────────────────────────────────

struct ArmRow {
    label: String,
    patches: usize,
    slivers: usize,
    levels_total: usize,
    polylines: usize,
    cl_polylines: usize,
    inconsistencies: usize,
    cost: Option<CandidateCost>,
    /// Why the arm was not costed, when it was not.
    uncosted: Option<String>,
}

impl ArmRow {
    /// Emitted polylines ÷ total levels. §F1-1's median was **28**; on a
    /// segmented region this is the mean number of separate curve components
    /// one level of one patch broke into, and the number the pre-registered
    /// mechanism check reads.
    fn components_per_level(&self) -> f64 {
        if self.levels_total == 0 {
            f64::NAN
        } else {
            self.polylines as f64 / self.levels_total as f64
        }
    }
}

/// Convert one arm's contact polylines to CL, contain them, and cost them
/// through the SAME `relink_and_cost` every other arm in this file uses.
///
/// Returns `Err` with a printable reason when the arm is above
/// [`SEG_COST_CEILING_POLYLINES`] or nothing survived conversion.
fn cost_arm(
    fixture: Fixture<'_>,
    boundary: &Polygon2,
    polylines: &[Vec<P3>],
) -> Result<(CandidateCost, usize), String> {
    use rs_cam_core::geometry::region_set::RegionSet;

    if polylines.len() > SEG_COST_CEILING_POLYLINES {
        return Err(format!(
            "{} polylines > {SEG_COST_CEILING_POLYLINES} (= {FALSIFIER_MULTIPLE}x{REF_PCA_CELL_FRAGMENTS}), \
             the Stage A 'nothing a relinker can do' ceiling",
            polylines.len()
        ));
    }
    let converted = cl_polylines(polylines, fixture.mesh, fixture.index, fixture.cutter);
    if converted.polylines.is_empty() {
        return Err("no CL polyline survived the drop-cutter conversion".to_owned());
    }
    let raw = polylines_to_toolpath(
        &converted.polylines,
        FEED_MM_MIN,
        PLUNGE_MM_MIN,
        fixture.safe_z,
    );
    let escapes = raw
        .moves
        .iter()
        .filter(|mv| {
            mv.move_type.is_cutting()
                && !boundary.contains_point(&P2::new(mv.target.x, mv.target.y))
        })
        .count();
    let priced = if escapes == 0 {
        raw
    } else {
        rs_cam_core::geometry::boundary::clip_toolpath_to_boundary(&raw, boundary, fixture.safe_z)
    };
    let region = RegionSet::new(vec![boundary.clone()]);
    let cost = relink_and_cost(
        priced,
        fixture.mesh,
        fixture.index,
        fixture.cutter,
        &region,
        &fixture.kinematics,
        fixture.safe_z,
    );
    Ok((cost, converted.polylines.len()))
}

// ── F1-S SVG: patches under the paths ───────────────────────────────────

/// Distinct-enough patch fills by golden-angle hue rotation. Two patches can
/// still land on similar hues; the artifact's job is to show whether patches
/// follow the ribbon's ARMS, which reads off the shapes, not the exact colour.
fn patch_fill(patch: usize) -> String {
    let hue = (patch as f64 * 137.507_764_05) % 360.0;
    format!("hsl({hue:.1}, 62%, 74%)")
}

/// Patches in colour with the emitted paths over them, in the file's existing
/// SVG style (same viewBox padding, same boundary strokes).
fn segmented_svg(
    mesh: &TriangleMesh,
    boundary: &Polygon2,
    seg: &Segmentation,
    polylines: &[Vec<P3>],
    tag: &str,
) -> PathBuf {
    let [x0, y0, x1, y1] = boundary.bbox();
    let padding = 2.0;
    let (view_x, view_y) = (x0 - padding, y0 - padding);
    let (view_w, view_h) = (x1 - x0 + 2.0 * padding, y1 - y0 + 2.0 * padding);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_x:.3} {view_y:.3} {view_w:.3} {view_h:.3}\" width=\"1400\" height=\"1400\">\n\
         <title>Wanaka region 1: {} direction-coherence patches, {} emitted paths ({tag})</title>\n\
         <rect x=\"{view_x:.3}\" y=\"{view_y:.3}\" width=\"{view_w:.3}\" height=\"{view_h:.3}\" fill=\"white\"/>\n",
        seg.patches.len(),
        polylines.len()
    );

    // One <path> element per patch, all its triangles as subpaths — a path per
    // TRIANGLE would be ~44k elements and no renderer thanks you for that.
    for (index, patch) in seg.patches.iter().enumerate() {
        let mut d = String::with_capacity(patch.len() * 56);
        for &global in patch {
            let tri = mesh.triangles[global as usize];
            let a = mesh.vertices[tri[0] as usize];
            let b = mesh.vertices[tri[1] as usize];
            let c = mesh.vertices[tri[2] as usize];
            write!(
                d,
                "M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z",
                a.x, a.y, b.x, b.y, c.x, c.y
            )
            .expect("write patch subpath");
        }
        writeln!(
            svg,
            "<path d=\"{d}\" fill=\"{}\" stroke=\"none\"/>",
            patch_fill(index)
        )
        .expect("write patch element");
    }

    for line in polylines {
        let Some(first) = line.first() else { continue };
        let mut d = format!("M {:.3} {:.3}", first.x, first.y);
        for point in &line[1..] {
            write!(d, " L {:.3} {:.3}", point.x, point.y).expect("write path");
        }
        writeln!(
            svg,
            "<path d=\"{d}\" fill=\"none\" stroke=\"#111111\" stroke-width=\"0.05\"/>"
        )
        .expect("write path element");
    }

    let exterior_only = Polygon2::new(boundary.exterior.clone());
    writeln!(
        svg,
        "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.22\"/>",
        svg_path(&exterior_only)
    )
    .expect("write SVG exterior");
    for hole in &boundary.holes {
        writeln!(
            svg,
            "<path d=\"{}\" fill=\"none\" stroke=\"#ff7f00\" stroke-width=\"0.22\" stroke-dasharray=\"1.0 0.6\"/>",
            svg_path(&Polygon2::new(hole.clone()))
        )
        .expect("write SVG hole");
    }
    svg.push_str("</svg>\n");

    let output_dir = svg_output_dir();
    std::fs::create_dir_all(&output_dir).expect("create F1-S SVG output directory");
    let path = output_dir.join(format!(
        "wanaka_region1_direction_field_segmented_{tag}.svg"
    ));
    std::fs::write(&path, svg).expect("write F1-S SVG");
    path
}

// ── the pre-registration, printed BEFORE any number ─────────────────────

fn print_pre_registration() {
    eprintln!("========== F1-S PRE-REGISTERED VERDICT (written before the numbers) ==========\n");
    eprintln!(
        "   THE QUESTION: does segmenting region 1 by direction coherence BEFORE solving\n\
         \x20  collapse the fragmentation that falsified F1?\n"
    );
    eprintln!(
        "   PRIMARY comparison — the 180 deg UNDIVIDED CONTROL row of this same table.\n\
         \x20  It runs the same target field V, the same solve_paths_with_target code path and\n\
         \x20  the same Monge estimator as every segmented row; the ONLY variable between them\n\
         \x20  is the decomposition. That is what the verdict reads.\n"
    );
    eprintln!(
        "     * SEGMENTATION EXPLAINS THE F1 FAILURE if total polylines fall by at least an\n\
         \x20      ORDER OF MAGNITUDE against that control (thousands -> a few hundred), AND\n\
         \x20      the mechanism check holds: components-per-level (polylines / total levels)\n\
         \x20      collapses from the whole-region 28 toward ~1-2. Falling to roughly\n\
         \x20      patch-count x levels-per-patch IS that mechanism, confirmed."
    );
    eprintln!(
        "     * SEGMENTATION IS NOT THE FIX if polylines stay in the THOUSANDS. Then the\n\
         \x20      premise survives — region 1's measured +{REGION1_PRIZE_CEILING_PCT}% anisotropy prize is real\n\
         \x20      landscape (synthesis §11) — and the METHOD is still not for us; §11's\n\
         \x20      reopening was too generous.\n"
    );
    eprintln!(
        "   SECONDARY, historical: §F1-1 measured {REF_F1_WHOLE_REGION_POLYLINES} polylines / \
         {REF_F1_WHOLE_REGION_LEVELS} levels / median\n\
         \x20  {REF_F1_WHOLE_REGION_MEDIAN_COMPONENTS} components per level / \
         {REF_F1_WHOLE_REGION_INCONSISTENCIES} orientation inconsistencies, with the RUSINKIEWICZ\n\
         \x20  per-vertex estimator inside direction_field. THIS FILE USES A MONGE-QUADRIC FIT\n\
         \x20  AT A CONTROLLED 1.0 mm RADIUS, which is a low-pass filter by comparison. If the\n\
         \x20  180 deg control below lands far under {REF_F1_WHOLE_REGION_POLYLINES}, THAT SHARE OF THE DROP BELONGS TO\n\
         \x20  THE ESTIMATOR, NOT TO SEGMENTATION. Only the segmented-vs-control margin may be\n\
         \x20  read as segmentation's.\n"
    );
    eprintln!("   {FIXTURE_LIMIT_LABEL}\n");
    eprintln!("   {FRESH_STOCK_LABEL}\n");
    eprintln!(
        "   NOT BUILT, and the answer is conditional on it: the literature pipeline is\n\
         \x20  direction rule -> DETECT DEGENERACIES -> CLASSIFY (trisector/wedge/merged) ->\n\
         \x20  TRACE SEPARATRICES -> SEGMENT -> per-patch solve. F1-S adds a cheap [REPO]\n\
         \x20  stand-in for the last stage only. The degeneracy census below is printed so a\n\
         \x20  reader can see how much of the region the three missing stages would have\n\
         \x20  owned.\n"
    );
}

// ── the F1-S evidence run ───────────────────────────────────────────────

/// Phase F1-S: does segmentation collapse the fragmentation?
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo) and the region-1 capture"]
fn wanaka_direction_field_segmented_f1() {
    use rs_cam_core::machine::kinematics::MachineKinematics;

    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let Some(boundary) = load_region1_capture() else {
        eprintln!(
            "SKIP: no region-1 capture at {} — run the Phase F1 step-0 capture first:\n\
             \x20 cargo test -p rs_cam_core --test wanaka_region_capture_f1 \\\n\
             \x20   capture_wanaka_region_1_boundary -- --ignored --nocapture",
            capture_path().display()
        );
        return;
    };

    eprintln!(
        "\n########## PHASE F1-S — SEGMENTED direction field on captured Wanaka region 1 ##########\n"
    );
    print_pre_registration();

    let stepover_mm = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let fixture = Fixture {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        kinematics,
        safe_z,
        effective_min_z: mesh.bbox.min.z - 0.1,
    };
    eprintln!(
        "   capture {} ({:.1} mm², {} exterior vertices, {} holes); mesh {} triangles;\n\
         \x20  BallEndmill r = {BALL_RADIUS_MM}, h = {CUSP_HEIGHT_MM}, equal-cusp stepover {stepover_mm:.4} mm\n",
        capture_path().display(),
        boundary.area(),
        boundary.exterior.len(),
        boundary.holes.len(),
        mesh.triangles.len()
    );

    let region_tris = direction_field::triangles_where(&mesh, |_, centroid| {
        boundary.contains_point(&P2::new(centroid.x, centroid.y))
    });
    if region_tris.is_empty() {
        eprintln!("SKIP: no mesh triangle has its centroid inside the captured boundary.");
        return;
    }

    // ---- stage S1: per-triangle geometry + degeneracy census ----
    eprintln!("========== STAGE F1-S1 — Monge t1 field and the DEGENERACY CENSUS ==========\n");
    eprintln!(
        "   Estimator: Monge quadric fitted to mesh vertices inside {SEG_FIT_RADIUS_MM:.1} mm of each\n\
         \x20  triangle centroid (~3x the {WANAKA_MEDIAN_FACET_EDGE_MM} mm median facet edge); t1 = shape-operator\n\
         \x20  eigenvector for the SMALLER eigenvalue (= convex-positive kappa1). Restated from\n\
         \x20  wanaka_curvature_anisotropy.rs (5e4ae866) because crest_lines::Curvature is\n\
         \x20  pub(crate) and no src/ change was in scope.\n"
    );
    let (geom, census) = region_geometry(&mesh, &index, &region_tris, BALL_RADIUS_MM);
    let region_area: f64 = geom.iter().map(|g| g.area_mm2).sum();
    let total = region_tris.len();
    let pct = |n: usize| 100.0 * n as f64 / total.max(1) as f64;
    eprintln!("     region triangles                {total:>10}");
    eprintln!("     region surface area (mm²)       {region_area:>10.1}");
    eprintln!(
        "     quadric fits OK                 {:>10}  ({:.2}%)",
        census.fitted,
        pct(census.fitted)
    );
    eprintln!(
        "     fit refused: under-determined   {:>10}  ({:.2}%)",
        census.under_determined,
        pct(census.under_determined)
    );
    eprintln!(
        "     fit refused: ill-conditioned    {:>10}  ({:.2}%)",
        census.ill_conditioned,
        pct(census.ill_conditioned)
    );
    eprintln!(
        "     umbilic (no t1 exists)          {:>10}  ({:.2}%)",
        census.umbilic,
        pct(census.umbilic)
    );
    eprintln!(
        "     below the isotropy floor        {:>10}  ({:.2}%)",
        census.below_isotropy_floor,
        pct(census.below_isotropy_floor)
    );
    eprintln!(
        "     -> UNTRUSTED t1, total          {:>10}  ({:.2}%)",
        census.untrusted(),
        pct(census.untrusted())
    );
    eprintln!(
        "     |V| clamped (k_s + 1/r <= 0)    {:>10}  ({:.2}%)",
        census.clamped_magnitude,
        pct(census.clamped_magnitude)
    );
    eprintln!(
        "\n     The UNTRUSTED population is exactly what the literature would send to\n\
         \x20    degeneracy classification (trisector / wedge / merged, by a discriminant sign)\n\
         \x20    and separatrix tracing. F1-S builds NEITHER: an untrusted triangle adopts the\n\
         \x20    direction of whichever patch reaches it first, so it can BRIDGE two otherwise\n\
         \x20    incoherent patches. Every patch count below is optimistic by that amount.\n\
         \x20    The |V| clamp count is identical in every arm by construction — the magnitudes\n\
         \x20    are built once, region-wide, and shared.\n"
    );

    // Every arm below solves on the region SUBMESH — triangle `i` is region
    // slot `i` — so `slot_of` is the identity and the per-patch solve does not
    // pay a whole-mesh edge map each time. See `region_submesh`. The full mesh
    // stays in `fixture` for the drop-cutter conversion and the relink, which
    // must see the terrain the cutter actually meets.
    let submesh = region_submesh(&mesh, &region_tris);
    let slot_of: Vec<u32> = (0..region_tris.len() as u32).collect();
    let adjacency = region_edge_adjacency(&submesh, &slot_of);
    let params = direction_field::FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    eprintln!(
        "   region submesh: {} triangles, {} vertices (solve domain); curvature was fitted \
         against the FULL mesh.\n",
        submesh.triangles.len(),
        submesh.vertices.len()
    );

    // ---- stage S2: the costed sweep, running-mean criterion ----
    eprintln!("========== STAGE F1-S2 — tolerance sweep (running-mean gate), COSTED ==========\n");
    eprintln!(
        "   Each arm solves Eq. 15 once PER PATCH, and `build_region_mesh` rebuilds an\n\
         \x20  edge->face map every call, so a tight tolerance with thousands of patches is the\n\
         \x20  slow case. Expect minutes per arm, not seconds.\n"
    );
    let mut rows: Vec<ArmRow> = Vec::new();
    for &tolerance in &SEG_TOLERANCES_DEG {
        let seg = segment_region(
            &slot_of,
            &geom,
            &adjacency,
            tolerance,
            Criterion::RunningMean,
        );
        let solved = solve_segmented(&submesh, &slot_of, &geom, &seg, &params);
        let tag = format!("{}_{:03.0}deg", Criterion::RunningMean.tag(), tolerance);
        let svg = segmented_svg(&submesh, &boundary, &seg, &solved.polylines, &tag);

        let label = if (tolerance - 180.0).abs() < 1e-9 {
            "180° UNDIVIDED CONTROL".to_owned()
        } else {
            format!("{tolerance:.0}° coherence patches")
        };
        eprintln!("   -- {label} --");
        eprintln!(
            "     patches {} ({} slivers < {SEG_SLIVER_AREA_MM2} mm², {:.2} mm² total, {:.3}% of region);\n\
             \x20    largest patch {:.1} mm², untrusted-only patches {}, adopted-untrusted tris {}",
            seg.patches.len(),
            seg.slivers,
            seg.sliver_area_mm2,
            100.0 * seg.sliver_area_mm2 / region_area.max(1e-9),
            seg.areas.iter().copied().fold(0.0_f64, f64::max),
            seg.untrusted_only_patches,
            seg.adopted_untrusted
        );
        eprintln!(
            "     orientation inconsistencies {} of {} same-patch adjacent pairs ({:.2}%)",
            seg.inconsistencies,
            seg.same_patch_pairs,
            100.0 * seg.inconsistencies as f64 / seg.same_patch_pairs.max(1) as f64
        );
        eprintln!(
            "     levels total {} (max in one patch {}), polylines {}, empty patches {},\n\
             \x20    CG non-converged {}, level-cap hits {}",
            solved.levels_total,
            solved.max_levels_in_a_patch,
            solved.polylines.len(),
            solved.empty_patches,
            solved.non_converged,
            solved.level_cap_hits
        );
        eprintln!("     SVG: {}", svg.display());

        let (cost, cl_polylines, uncosted) = match cost_arm(fixture, &boundary, &solved.polylines) {
            Ok((cost, cl)) => {
                eprintln!(
                    "     costed: fragments {}, linked {}, retracts {}, cut {:.1} mm, {:.1} s",
                    cost.fragments, cost.linked, cost.kept_retracts, cost.cutting_mm, cost.time_s
                );
                (Some(cost), cl, None)
            }
            Err(reason) => {
                eprintln!("     NOT COSTED: {reason}");
                (None, 0, Some(reason))
            }
        };
        eprintln!();
        rows.push(ArmRow {
            label,
            patches: seg.patches.len(),
            slivers: seg.slivers,
            levels_total: solved.levels_total,
            polylines: solved.polylines.len(),
            cl_polylines,
            inconsistencies: seg.inconsistencies,
            cost,
            uncosted,
        });
    }

    // ---- stage S3: the discriminating table ----
    eprintln!("========== STAGE F1-S3 — the discriminating table ==========\n");
    eprintln!("   {FRESH_STOCK_LABEL}\n");
    eprintln!("   {FIXTURE_LIMIT_LABEL}\n");
    eprintln!(
        "   'fragments' is the relinker's INPUT fragment count (surface_link::RelinkReport\n\
         \x20  ::fragments), which is what the 141 / 491 / 564 reference rows are, so the\n\
         \x20  column stays comparable. 'retracts' is kept retract links = pieces − 1.\n"
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
        "arm",
        "patches",
        "slivers",
        "polylines",
        "fragments",
        "links",
        "retracts",
        "cut mm",
        "F-034 s"
    );
    for row in &rows {
        match &row.cost {
            Some(cost) => eprintln!(
                "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10.1} {:>9.1}",
                row.label,
                row.patches,
                row.slivers,
                row.polylines,
                cost.fragments,
                cost.linked,
                cost.kept_retracts,
                cost.cutting_mm,
                cost.time_s
            ),
            None => eprintln!(
                "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
                row.label, row.patches, row.slivers, row.polylines, "—", "—", "—", "—", "—"
            ),
        }
    }
    eprintln!(
        "\n     -- REFERENCE ROWS, all link_ceiling: None (the chartered fresh-stock exception) --"
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
        "F1 whole-region field",
        "1",
        "—",
        REF_F1_WHOLE_REGION_POLYLINES,
        "—",
        "—",
        "—",
        "—",
        "not costed"
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
        "PCA-minor CELLS (tapered)",
        "69",
        "—",
        "—",
        REF_PCA_CELL_FRAGMENTS,
        "—",
        53,
        "8279",
        format!("{REF_PCA_CELL_TIME_S:.1}")
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
        "0° undivided (tapered)",
        "1",
        "—",
        "—",
        REF_RASTER0_FRAGMENTS,
        "—",
        97,
        "8687",
        format!("{REF_RASTER0_TIME_S:.1}")
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>10} {:>7} {:>9} {:>10} {:>9}",
        "PCA-minor undivided (tap.)",
        "1",
        "—",
        "—",
        REF_PCA_UNDIVIDED_FRAGMENTS,
        "—",
        74,
        "8501",
        format!("{REF_PCA_UNDIVIDED_TIME_S:.1}")
    );
    eprintln!(
        "     The three tapered rows are CROSS-CUTTER context (tapered R1.5 ball, not this\n\
         \x20    file's R1.0 true ball); the F1 row is cross-ESTIMATOR. Neither is this table's\n\
         \x20    bar — the 180° control is.\n"
    );

    // ---- stage S4: the mechanism check ----
    eprintln!("========== STAGE F1-S4 — mechanism check and verdict ==========\n");
    eprintln!(
        "     {:<26} {:>10} {:>12} {:>22} {:>14}",
        "arm", "polylines", "levels", "components/level", "CL polylines"
    );
    for row in &rows {
        eprintln!(
            "     {:<26} {:>10} {:>12} {:>22.2} {:>14}",
            row.label,
            row.polylines,
            row.levels_total,
            row.components_per_level(),
            if row.cl_polylines == 0 {
                "—".to_owned()
            } else {
                row.cl_polylines.to_string()
            }
        );
    }
    eprintln!(
        "     {:<26} {:>10} {:>12} {:>22} {:>14}",
        "F1 whole-region (Rusin.)",
        REF_F1_WHOLE_REGION_POLYLINES,
        REF_F1_WHOLE_REGION_LEVELS,
        format!("{REF_F1_WHOLE_REGION_MEDIAN_COMPONENTS} (median)"),
        "—"
    );

    if let Some(control) = rows.first() {
        eprintln!(
            "\n     Control (180°): {} polylines, {} levels, {:.2} components/level, {} \
             orientation inconsistencies.",
            control.polylines,
            control.levels_total,
            control.components_per_level(),
            control.inconsistencies
        );
        for row in rows.iter().skip(1) {
            let ratio = control.polylines as f64 / row.polylines.max(1) as f64;
            eprintln!(
                "     {:<26} {:>7} polylines = control / {ratio:.2}   ({} orientation inconsistencies)",
                row.label, row.polylines, row.inconsistencies
            );
        }
        let best = rows
            .iter()
            .skip(1)
            .min_by_key(|row| row.polylines)
            .map(|row| (row.label.clone(), row.polylines));
        if let Some((label, polylines)) = best {
            let fell = control.polylines as f64 / polylines.max(1) as f64;
            eprintln!(
                "\n     PRE-REGISTERED READING: best segmented arm is '{label}' at {polylines} \
                 polylines,\n\
                 \x20    a {fell:.2}x fall against the control. The bar was an ORDER OF MAGNITUDE \
                 (>=10x)\n\
                 \x20    AND components/level collapsing toward ~1-2."
            );
            if fell >= 10.0 {
                eprintln!(
                    "     -> the ORDER-OF-MAGNITUDE half of the bar is MET. Read the \
                     components/level\n\
                     \x20    column before calling the mechanism confirmed."
                );
            } else {
                eprintln!(
                    "     -> the ORDER-OF-MAGNITUDE half of the bar is NOT met. On the \
                     pre-registration,\n\
                     \x20    segmentation is NOT the explanation for the F1 failure; the premise \
                     (region 1's\n\
                     \x20    measured +{REGION1_PRIZE_CEILING_PCT}% anisotropy prize) still stands \
                     and the METHOD does not."
                );
            }
        }
    }
    for row in &rows {
        if let Some(reason) = &row.uncosted {
            eprintln!("     ({}: {reason})", row.label);
        }
    }

    // ---- stage S5: criterion sensitivity, fragmentation only ----
    eprintln!("\n========== STAGE F1-S5 — gate-criterion sensitivity (UNCOSTED) ==========\n");
    eprintln!(
        "   The running-mean gate cannot let a patch BEND: a long curved ribbon arm is cut\n\
         \x20  wherever it has turned past the tolerance from where its seed started. Zou\n\
         \x20  Eq. 18 is a metric between two NEIGHBOURING points, which does permit a bend.\n\
         \x20  This block runs the same sweep with the neighbour-local gate. Fragmentation\n\
         \x20  only — no relink, no cost, deliberately, so the sensitivity cannot be quoted as\n\
         \x20  a time claim on a fixture whose facet/stepover ratio forbids one.\n"
    );
    eprintln!(
        "     {:<26} {:>8} {:>8} {:>10} {:>12} {:>20}",
        "arm (neighbour-local)", "patches", "slivers", "polylines", "levels", "components/level"
    );
    for &tolerance in &SEG_TOLERANCES_DEG {
        if (tolerance - 180.0).abs() < 1e-9 {
            continue; // identical to the control: the gate never rejects.
        }
        let seg = segment_region(
            &slot_of,
            &geom,
            &adjacency,
            tolerance,
            Criterion::NeighbourLocal,
        );
        let solved = solve_segmented(&submesh, &slot_of, &geom, &seg, &params);
        let tag = format!("{}_{:03.0}deg", Criterion::NeighbourLocal.tag(), tolerance);
        let svg = segmented_svg(&submesh, &boundary, &seg, &solved.polylines, &tag);
        let per_level = if solved.levels_total == 0 {
            f64::NAN
        } else {
            solved.polylines.len() as f64 / solved.levels_total as f64
        };
        eprintln!(
            "     {:<26} {:>8} {:>8} {:>10} {:>12} {:>20.2}",
            format!("{tolerance:.0}° neighbour-local"),
            seg.patches.len(),
            seg.slivers,
            solved.polylines.len(),
            solved.levels_total,
            per_level
        );
        eprintln!("     {:<26} SVG: {}", "", svg.display());
    }

    eprintln!("\n########## PHASE F1-S evidence run complete. ##########\n");
}

// ── smoke tests: this instrument's own arithmetic, no external inputs ───

/// The stepover every arm of the F1 table runs at. If this moves, every
/// number in the table moves with it, so it is pinned here rather than only
/// printed.
#[test]
fn equal_cusp_stepover_at_r1_h003_is_0_4862() {
    let s = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    assert!(
        (s - 0.486_209_831).abs() < 1e-6,
        "equal-cusp stepover at R=1.0, h=0.03 should be 0.4862 mm, got {s}"
    );
    // Degenerate inputs return 0 rather than NaN.
    assert_eq!(equal_cusp_stepover_mm(1.0, 0.0), 0.0);
    assert_eq!(equal_cusp_stepover_mm(1.0, 1.0), 0.0);
}

/// The restated PCA helper must name the SHORT axis. A 40 × 10 rectangle is
/// long in X, so the minor axis is 90° and the elongation is 40/10 = 4.
#[test]
fn pca_minor_of_a_wide_rectangle_is_the_short_axis() {
    let rect = Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(40.0, 0.0),
        P2::new(40.0, 10.0),
        P2::new(0.0, 10.0),
    ]);
    let (minor, elongation) =
        pca_minor_and_elongation(&rect, 0.25).expect("rectangle has a PCA axis");
    assert!(
        (minor - 90.0).abs() < 1.0,
        "minor axis of a 40x10 rectangle should be ~90°, got {minor}"
    );
    assert!(
        (elongation - 4.0).abs() < 0.15,
        "elongation of a 40x10 rectangle should be ~4.0, got {elongation}"
    );
}

/// The candidate's toolpath assembly must tag moves exactly the way
/// `raster_toolpath_from_grid` does, because the relinker and the F-034
/// integrator read those tags. Pinned without the mesh.
#[test]
fn polyline_toolpath_matches_the_raster_move_intent_sequence() {
    use rs_cam_core::toolpath::MoveType;

    let first = vec![
        P3::new(0.0, 0.0, -1.0),
        P3::new(1.0, 0.0, -1.0),
        P3::new(2.0, 0.0, -1.0),
    ];
    let second = vec![P3::new(0.0, 5.0, -1.5), P3::new(1.0, 5.0, -1.5)];
    let tp = polylines_to_toolpath(&[first, second], 735.0, 180.0, 12.0);

    let actual: Vec<(MoveIntent, bool)> = tp
        .moves
        .iter()
        .map(|mv| (mv.intent, mv.move_type.is_cutting()))
        .collect();
    let expected = vec![
        (MoveIntent::Linking, false),
        (MoveIntent::EntryPlunge, true),
        (MoveIntent::FinishingCut, true),
        (MoveIntent::FinishingCut, true),
        (MoveIntent::Retract, false),
        (MoveIntent::Linking, false),
        (MoveIntent::EntryPlunge, true),
        (MoveIntent::FinishingCut, true),
        (MoveIntent::Retract, false),
    ];
    assert_eq!(actual, expected, "move intent sequence");

    // The separating retract lifts at the PREVIOUS polyline's exit XY, not at
    // the next one's entry — the raster generator's rule, and the one that
    // keeps a retract from dragging the cutter across uncut ground.
    assert_eq!(tp.moves[4].target, P3::new(2.0, 0.0, 12.0));
    assert_eq!(tp.moves[5].target, P3::new(0.0, 5.0, 12.0));
    assert_eq!(tp.moves[8].target, P3::new(1.0, 5.0, 12.0));

    // Plunge and cut feeds are distinct, and both are Linear.
    assert!(matches!(
        tp.moves[1].move_type,
        MoveType::Linear { feed_rate } if (feed_rate - 180.0).abs() < 1e-9
    ));
    assert!(matches!(
        tp.moves[2].move_type,
        MoveType::Linear { feed_rate } if (feed_rate - 735.0).abs() < 1e-9
    ));
}

/// A polyline with fewer than two points cannot be a path; the assembler must
/// not emit a plunge with no cut after it.
#[test]
fn polyline_toolpath_is_empty_for_no_polylines() {
    let tp = polylines_to_toolpath(&[], 735.0, 180.0, 12.0);
    assert!(tp.moves.is_empty());
}

/// The point-to-segment distance the spacing stage depends on: a point
/// abreast of a segment measures the perpendicular, not the nearer endpoint.
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

// ── F1-S smoke tests: no mesh, no capture ───────────────────────────────

/// The sign algebra `t₁` rests on, pinned on a closed form.
///
/// On the bowl `z = ½(A x² + B y²)` with `A > B > 0` the shape operator is
/// `diag(A, B)`, the convex-positive curvatures are `κ₁ = −B ≥ κ₂ = −A`, and
/// `t₁` is `ŷ` — the **gentle** direction. Feeding along it leaves the sharply
/// concave `x̂` as the step-over direction, where the ball sits deepest and the
/// strip is widest. Get the eigenvalue the wrong way round and the method
/// feeds along the *worst* axis everywhere, silently.
#[test]
fn shape_operator_t1_is_the_gentle_direction_of_a_bowl() {
    let sharp = 0.020_f64;
    let gentle = 0.005_f64;

    // z = ½(A x² + B y²) at the origin: f_x = f_y = f_xy = 0.
    let fit = fit_from_derivatives((0.0, 0.0), (sharp, 0.0, gentle));
    assert!(
        (fit.kappa1 - -gentle).abs() < 1e-12,
        "kappa1 should be the convex-positive max, -B = {}, got {}",
        -gentle,
        fit.kappa1
    );
    assert!(
        (fit.kappa2 - -sharp).abs() < 1e-12,
        "kappa2 should be -A = {}, got {}",
        -sharp,
        fit.kappa2
    );
    assert!(fit.kappa1 >= fit.kappa2, "kappa1 must be the larger signed");

    let axis = fit
        .axis
        .expect("an anisotropic bowl has a principal direction");
    assert!(
        axis[0].abs() < 1e-9 && (axis[1].abs() - 1.0).abs() < 1e-9,
        "t1 of a bowl sharper in x must be +/-y, got {axis:?}"
    );

    // k_s — the curvature PERPENDICULAR to the feed — must come back as the
    // other principal curvature, the sharp one. That is what makes the Eq. 13
    // magnitude smallest and the side-step widest along t1.
    let k_s = kappa_perp_zou(&fit, axis);
    assert!(
        (k_s - -sharp).abs() < 1e-12,
        "k_s perpendicular to t1 should be kappa2 = {}, got {k_s}",
        -sharp
    );
    // And feeding the other way really is worse: a bigger k_s is a bigger
    // denominator in sqrt(8h/(k_s + 1/r)) and therefore a narrower strip.
    let k_s_wrong = kappa_perp_zou(&fit, [1.0, 0.0]);
    assert!(
        k_s_wrong > k_s,
        "feeding along the sharp axis must leave a LARGER k_s ({k_s_wrong} vs {k_s})"
    );
}

/// The same fit with the axes swapped must move `t₁` with them — a test that
/// only ever sees one orientation cannot catch a transposed shape operator.
#[test]
fn shape_operator_t1_follows_the_swapped_axes() {
    let fit = fit_from_derivatives((0.0, 0.0), (0.005, 0.0, 0.020));
    let axis = fit
        .axis
        .expect("an anisotropic bowl has a principal direction");
    assert!(
        (axis[0].abs() - 1.0).abs() < 1e-9 && axis[1].abs() < 1e-9,
        "t1 of a bowl sharper in y must be +/-x, got {axis:?}"
    );
    // A sphere cap is umbilic: no direction exists, and the estimator must say
    // so rather than returning whichever way round-off pointed.
    let umbilic = fit_from_derivatives((0.0, 0.0), (0.01, 0.0, 0.01));
    assert!(
        umbilic.axis.is_none(),
        "an umbilic point must report NO principal direction"
    );
}

/// A chain of triangles whose `t₁` turns 90° halfway along must segment into
/// exactly two patches below a 90° tolerance, and into one above it.
#[test]
fn segmentation_splits_a_two_direction_chain() {
    // Six slots in a line; anisotropy descending so the seed order is fixed.
    let along_x = Some([1.0, 0.0]);
    let along_y = Some([0.0, 1.0]);
    let geom: Vec<TriGeom> = (0..6_usize)
        .map(|slot| TriGeom {
            axis: if slot < 3 { along_x } else { along_y },
            anisotropy: 6.0 - slot as f64,
            magnitude: 0.3,
            area_mm2: 4.0,
            trusted: true,
        })
        .collect();
    let region: Vec<u32> = (0..6_u32).collect();
    let adjacency: Vec<Vec<usize>> = (0..6_usize)
        .map(|slot| {
            let mut list = Vec::new();
            if slot > 0 {
                list.push(slot - 1);
            }
            if slot < 5 {
                list.push(slot + 1);
            }
            list
        })
        .collect();

    let split = segment_region(&region, &geom, &adjacency, 45.0, Criterion::RunningMean);
    assert_eq!(split.patches.len(), 2, "a 90° turn must cut the chain");
    assert_eq!(split.patches[0], vec![0, 1, 2]);
    assert_eq!(split.patches[1], vec![3, 4, 5]);
    assert_eq!(split.adopted_untrusted, 0);
    assert_eq!(split.untrusted_only_patches, 0);
    // Both patches are 12 mm², well over the sliver bar.
    assert_eq!(split.slivers, 0);

    // At 180° the gate never rejects: one patch, which is the control arm.
    let whole = segment_region(&region, &geom, &adjacency, 180.0, Criterion::RunningMean);
    assert_eq!(whole.patches.len(), 1, "the 180° control must be undivided");
    assert_eq!(whole.patches[0].len(), 6);
    // The 90° join is now INSIDE one patch, and the orientation census says so
    // is not the same thing as saying the directions agree — perpendicular
    // neighbours have dot 0, which is not negative.
    assert_eq!(whole.same_patch_pairs, 5);
}

/// A triangle with no trustworthy `t₁` must be admitted, adopt the direction
/// it was reached with, and be COUNTED — the population the missing
/// classification and separatrix stages would have owned.
#[test]
fn segmentation_adopts_and_counts_untrusted_triangles() {
    let geom = vec![
        TriGeom {
            axis: Some([1.0, 0.0]),
            anisotropy: 2.0,
            magnitude: 0.3,
            area_mm2: 0.4,
            trusted: true,
        },
        TriGeom {
            axis: None,
            anisotropy: 0.0,
            magnitude: 0.3,
            area_mm2: 0.4,
            trusted: false,
        },
        TriGeom {
            axis: Some([0.0, 1.0]),
            anisotropy: 1.0,
            magnitude: 0.3,
            area_mm2: 0.4,
            trusted: true,
        },
    ];
    let region: Vec<u32> = vec![10, 11, 12];
    let adjacency = vec![vec![1], vec![0, 2], vec![1]];

    let seg = segment_region(&region, &geom, &adjacency, 45.0, Criterion::RunningMean);
    assert_eq!(seg.adopted_untrusted, 1, "the middle triangle has no t1");
    assert_eq!(seg.patches.len(), 2);
    // Seeded at slot 0 (most anisotropic), swallows the untrusted slot 1, then
    // is stopped by slot 2's perpendicular direction.
    assert_eq!(seg.patches[0], vec![10, 11]);
    assert_eq!(seg.patches[1], vec![12]);
    // The adopted triangle took its patch's direction, not a zero vector.
    assert!((seg.oriented[1][0] - 1.0).abs() < 1e-12);
    // Both patches are under 1 mm², so both are slivers — counted, not dropped.
    assert_eq!(seg.slivers, 2);
    assert!((seg.sliver_area_mm2 - 1.2).abs() < 1e-9);
    assert_eq!(seg.patches.iter().map(Vec::len).sum::<usize>(), 3);
}

/// A region made only of untrusted triangles has no seed at all. It must still
/// be covered — by the leftover flood — and the flood must be counted, because
/// a silently empty segmentation would print zero patches and zero fragments
/// and read like a triumph.
#[test]
fn segmentation_floods_a_region_with_no_seed() {
    let geom: Vec<TriGeom> = (0..3_usize)
        .map(|_| TriGeom {
            axis: None,
            anisotropy: 0.0,
            magnitude: 0.3,
            area_mm2: 5.0,
            trusted: false,
        })
        .collect();
    let region: Vec<u32> = vec![0, 1, 2];
    let adjacency = vec![vec![1], vec![0, 2], vec![1]];
    let seg = segment_region(&region, &geom, &adjacency, 30.0, Criterion::RunningMean);
    assert_eq!(seg.patches.len(), 1);
    assert_eq!(seg.untrusted_only_patches, 1);
    assert_eq!(seg.patches[0], vec![0, 1, 2]);
    // No triangle is left without a direction, so the Poisson target can never
    // be a zero field by accident.
    assert!(seg.oriented.iter().all(|d| d[0].abs() + d[1].abs() > 0.0));
}

/// Adjacency must link triangles that share an EDGE and nothing else — a
/// vertex-only touch is not adjacency, and if it were, patches would leak
/// across a pinch point exactly where the ribbon's arms meet.
#[test]
fn region_edge_adjacency_links_only_shared_edges() {
    // Two triangles sharing edge (1,2); a third touching only at vertex 3.
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.0),
        P3::new(0.0, 1.0, 0.0),
        P3::new(1.0, 1.0, 0.0),
        P3::new(2.0, 2.0, 0.0),
        P3::new(1.0, 2.0, 0.0),
    ];
    let triangles = vec![[0, 1, 2], [1, 3, 2], [3, 4, 5]];
    let mesh = TriangleMesh::from_raw(vertices, triangles);

    let adjacency = region_edge_adjacency(&mesh, &[0, 1, 2]);
    assert_eq!(adjacency[0], vec![1], "0 and 1 share edge (1,2)");
    assert_eq!(adjacency[1], vec![0], "1 touches 2 only at vertex 3");
    assert!(adjacency[2].is_empty());

    // Restricting the region drops the link with it.
    let single = region_edge_adjacency(&mesh, &[0]);
    assert!(single[0].is_empty());
}

/// **The property every index in F1-S rests on**: submesh triangle `i` is
/// region slot `i`, in the order the region list gave, with geometry copied
/// unchanged and vertices deduped. `segment_region`'s emitted ids,
/// `solve_segmented`'s identity `slot_of`, and `segmented_svg`'s triangle
/// lookup all assume it silently; if the compaction ever reordered or
/// re-welded, every arm would still run and every number would be wrong.
#[test]
fn region_submesh_preserves_order_and_geometry() {
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.5),
        P3::new(0.0, 1.0, 0.25),
        P3::new(1.0, 1.0, 0.75),
        P3::new(2.0, 2.0, 1.0),
        P3::new(1.0, 2.0, 1.5),
    ];
    let triangles = vec![[0, 1, 2], [1, 3, 2], [3, 4, 5]];
    let mesh = TriangleMesh::from_raw(vertices, triangles);

    // Deliberately NOT contiguous and NOT the whole mesh: slot 0 is global 2,
    // slot 1 is global 0. Order must follow the region list, not the ids.
    let region: Vec<u32> = vec![2, 0];
    let sub = region_submesh(&mesh, &region);

    assert_eq!(
        sub.triangles.len(),
        2,
        "one submesh triangle per region slot"
    );
    // Two disjoint triangles share no vertex, so nothing dedupes here.
    assert_eq!(sub.vertices.len(), 6);

    for (slot, &global) in region.iter().enumerate() {
        let original = mesh.triangles[global as usize];
        let compacted = sub.triangles[slot];
        for corner in 0..3 {
            let want = mesh.vertices[original[corner] as usize];
            let got = sub.vertices[compacted[corner] as usize];
            assert!(
                (want - got).norm() < 1e-12,
                "slot {slot} corner {corner}: submesh vertex moved ({want:?} vs {got:?})"
            );
        }
    }

    // A shared vertex must weld to ONE submesh vertex, or the Poisson domain
    // would be two disconnected sheets where the surface is one.
    let joined = region_submesh(&mesh, &[0, 1]);
    assert_eq!(joined.triangles.len(), 2);
    assert_eq!(
        joined.vertices.len(),
        4,
        "triangles [0,1,2] and [1,3,2] share vertices 1 and 2"
    );
    // And the shared edge survives compaction, so adjacency still finds it.
    assert_eq!(region_edge_adjacency(&joined, &[0, 1])[0], vec![1]);
}

/// The cost ceiling must be the same number Stage A already uses, and the
/// sliver bar must be small enough to mean "cannot hold a pass" rather than
/// "small patch".
#[test]
fn f1s_thresholds_agree_with_stage_a() {
    assert_eq!(SEG_COST_CEILING_POLYLINES, 705);
    // A square patch at the sliver bar is 1 mm on a side, which holds barely
    // two passes at the 0.4862 mm stepover. The bar means "cannot hold a
    // pass", not "small".
    let stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let passes_across = SEG_SLIVER_AREA_MM2.sqrt() / stepover;
    assert!(
        (1.0..4.0).contains(&passes_across),
        "the sliver bar should be a few stepovers across, got {passes_across:.2}"
    );
    // The 180° control must be the FIRST row of the sweep, because the whole
    // verdict is written against `rows.first()`.
    assert!((SEG_TOLERANCES_DEG[0] - 180.0).abs() < 1e-12);
    assert!(SEG_TOLERANCES_DEG.iter().skip(1).all(|t| *t < 180.0));
}
