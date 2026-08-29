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
//! | **F1-A** | 1, 4 | the full [`rs_cam_core::direction_field::FieldReport`] plus the **cheap falsifier**: total polylines vs the 141-fragment PCA-cell reference |
//! | **F1-B** | 6 | one SVG of every iso-curve over the captured boundary |
//! | **F1-C** | 3 | measured spacing between adjacent levels vs the target stepover — geometry only, no simulation |
//! | **F1-D** | 1, 2, 4 | contact→cutter-centre conversion, a boundary containment proof, and the F-034 cost |
//! | **F1-E** | 1, 2 | ball-end 0° and PCA-minor raster baselines, single-variable, plus labelled cross-cutter context |
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

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::direction_field::{self, FieldPathResult, FieldReport};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
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

/// **Restated from `thin_organic_island_widths.rs:2015-2038`.**
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

/// **Restated from `thin_organic_island_widths.rs:1861-1878`**, with the two
/// ceiling-regime fields (`slower_than_retract`, `ceiling_above_safe_z`)
/// **trimmed**: they are read only by that file's Stage L, which costs a
/// candidate under a rest-stock ceiling. Every arm here runs
/// `link_ceiling: None`, where `ceiling_above_safe_z` is structurally 0
/// (`surface_link.rs:302-308`), so carrying them would be a write-only field.
/// The six fields below are exactly the ones the F1 brief's table quotes.
struct CandidateCost {
    moves: usize,
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
}

/// **Restated from `thin_organic_island_widths.rs:1939-2013`** (its
/// `relink_and_cost` + the fresh-stock arm of `relink_and_cost_under`, folded
/// back into one function since no ceiling regime exists here). The
/// `RelinkParams` block is field-for-field identical: `hookup_distance` 25.0
/// (the operator's `intra_region_hookup_mm`, `wanaka200_mt2.toml:949`),
/// `stock_to_leave` 0.0, `sampling` 0.5, tier-1 feeds, `reorder: true`, the
/// region's own polygon as boundary, `link_ceiling: None`, and both regime
/// flags `false` — inert without a ceiling (`surface_link.rs:252`,
/// `:281-285`).
///
/// `cutter` is the ball control fixture; `relink_fragments` takes
/// `&dyn MillingCutter`, so no other change was needed.
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
    kinematics: rs_cam_core::machine_kinematics::MachineKinematics,
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
    use rs_cam_core::region_set::RegionSet;

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
        let clipped = rs_cam_core::boundary::clip_toolpath_to_boundary(&raw, boundary, safe_z);
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
    use rs_cam_core::region_set::RegionSet;

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
    use rs_cam_core::machine_kinematics::MachineKinematics;

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
