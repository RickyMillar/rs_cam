//! **Phase F2 steps 1–2 — the conformal-spiral evidence instrument.**
//! (`planning/conformal_finish_2026-08-28/PROGRAMME.md` §"Phase F2"; design
//! note `research/conformal_finish_2026-08-28.md` §B.4.)
//!
//! # What this measures
//!
//! One question, staged, on the **in-repo** `fixtures/terrain_small.stl`
//! (40,342 triangles, 100 × 73.3 × 52.6 mm) with a hole-free elliptical
//! region: does [`rs_cam_core::conformal_spiral::plan_spiral`] produce a
//! single continuous, non-self-intersecting, correctly-spaced spiral over a
//! simply-connected region of a real terrain mesh — and at what length
//! overhead over the pure rings it was bridged from?
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
//! STOP the run when any of the three holds:
//!
//! * `uncovered_after_bridging > 0` — incomplete coverage **after** the
//!   §2.2.2 step-2 bridge repair;
//! * `disk_self_intersections > 0` — any disk-domain self-intersection;
//! * `bridge_overhead_pct > 25.0` — bridge length overhead vs pure rings
//!   (inside the §0k +43 % trap with margin).
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

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::conformal_spiral::{
    self, PAPER_START_ANGLE_STEP, SpiralParams, SpiralReport, SpiralResult,
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
    polygon: Polygon2,
    triangles: Vec<u32>,
    /// Semi-axes actually used (mm).
    semi_axes: (f64, f64),
    /// How many 2 mm shrinks were needed.
    shrinks: usize,
}

/// Build the region and plan on it, shrinking the ellipse on a topology
/// refusal.
///
/// **Spec resolution (2026-08-30).** The brief named `NotSimplyConnected` /
/// `NotADisk` as the shrink triggers. `NonManifoldBoundary` is added: a
/// centroid selection over ragged terrain triangles produces a boundary pinch
/// (two outgoing boundary half-edges at one vertex) exactly as readily as it
/// produces a second loop, and both are the same "the fringe is ragged"
/// symptom. Every other refusal arm — `EmptyRegion`, `FlattenDidNotConverge`,
/// `NoSurfaceSamples`, `RingSearchStalled`, `RingLimitReached` — is a
/// *mechanism* failure that a smaller ellipse would only hide, so those are
/// reported and returned, not retried.
fn plan_with_shrink(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    params: &SpiralParams,
) -> Option<(Region, SpiralResult, SpiralReport)> {
    use rs_cam_core::conformal_spiral::SpiralRefusal;

    let bb = &mesh.bbox;
    let (cx, cy) = (0.5 * (bb.min.x + bb.max.x), 0.5 * (bb.min.y + bb.max.y));
    let ax0 = 0.5 * (bb.max.x - bb.min.x) - REGION_INSET_MM;
    let ay0 = 0.5 * (bb.max.y - bb.min.y) - REGION_INSET_MM;

    for shrink in 0..=MAX_SHRINKS {
        let shrink_mm = shrink as f64 * SHRINK_STEP_MM;
        let (ax, ay) = (ax0 - shrink_mm, ay0 - shrink_mm);
        if ax <= 0.0 || ay <= 0.0 {
            eprintln!("   REFUSE: ellipse shrank to nothing at step {shrink}.");
            return None;
        }
        let polygon = ellipse_polygon(cx, cy, ax, ay, ELLIPSE_VERTICES);
        let triangles = conformal_spiral_region(mesh, &polygon);
        eprintln!(
            "   region try {shrink}: ellipse centre ({cx:.3}, {cy:.3}), semi-axes \
             ({ax:.3}, {ay:.3}) mm, {ELLIPSE_VERTICES} vertices, \
             polygon area {:.1} mm², {} triangles selected",
            polygon.area(),
            triangles.len()
        );
        if triangles.is_empty() {
            eprintln!("     no triangle centroid inside — shrinking.");
            continue;
        }
        match conformal_spiral::plan_spiral(mesh, index, &triangles, params) {
            Ok((result, report)) => {
                return Some((
                    Region {
                        polygon,
                        triangles,
                        semi_axes: (ax, ay),
                        shrinks: shrink,
                    },
                    result,
                    report,
                ));
            }
            Err(refusal) => {
                let retryable = matches!(
                    refusal,
                    SpiralRefusal::NotSimplyConnected { .. }
                        | SpiralRefusal::NotADisk { .. }
                        | SpiralRefusal::NonManifoldBoundary { .. }
                );
                eprintln!("     REFUSAL: {refusal:?}");
                if !retryable {
                    eprintln!(
                        "     NOT a topology refusal — a smaller ellipse would hide a \
                         mechanism failure, so this is reported, not retried."
                    );
                    return None;
                }
                eprintln!("     topology refusal — shrinking by {SHRINK_STEP_MM} mm and retrying.");
            }
        }
    }
    eprintln!("   REFUSE: {MAX_SHRINKS} shrinks exhausted without a disk-topology region.");
    None
}

/// Triangles whose centroid is inside the region polygon.
/// `Polygon2::contains_point` honours holes; this polygon has none.
fn conformal_spiral_region(mesh: &TriangleMesh, polygon: &Polygon2) -> Vec<u32> {
    rs_cam_core::direction_field::triangles_where(mesh, |_, centroid| {
        polygon.contains_point(&P2::new(centroid.x, centroid.y))
    })
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

    eprintln!("\n   -- flattening: the BAD-MAP vs BAD-MECHANISM table --");
    eprintln!(
        "     interior vertices (solve size){:>12}",
        report.flatten_interior_vertices
    );
    eprintln!(
        "     CG iterations u / v           {:>12} / {}",
        report.flatten_cg_iterations[0], report.flatten_cg_iterations[1]
    );
    eprintln!(
        "     CG residual u / v             {:>12.3e} / {:.3e}",
        report.flatten_residual[0], report.flatten_residual[1]
    );
    eprintln!(
        "     flipped triangles (want 0)    {:>12}",
        report.flipped_triangles
    );
    eprintln!(
        "     orientation sign              {:>12.1}",
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
        "     READ THIS FIRST if anything below disappoints: a HARMONIC map is a labelled\n\
         \x20    [REPO] substitution for the paper's conformal slit map (PROGRAMME.md §F2 step 1).\n\
         \x20    Nonzero flips are a DISCRETISATION symptom (obtuse triangles, negative cotangent\n\
         \x20    weights), not a mechanism verdict; a wide area-distortion spread is expected and\n\
         \x20    is what the sampled coverage check exists to compensate."
    );

    eprintln!("\n   -- S^h sampling + coverage-driven rings (Eqs. 1-4) --");
    eprintln!(
        "     surface samples N_S placed    {:>12}",
        report.surface_samples
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

    // -- N_S adequacy, per the module's OWN rule --
    eprintln!("\n   -- N_S ADEQUACY (SpiralParams::n_surface_samples doc, G-SAMPLING) --");
    let implied_spacing = if report.surface_samples > 0 && region_area_3d_mm2 > 0.0 {
        (region_area_3d_mm2 / report.surface_samples as f64).sqrt()
    } else {
        f64::NAN
    };
    let half_stepover = 0.5 * stepover_mm;
    eprintln!("     region 3D area (mm²)          {region_area_3d_mm2:>12.1}");
    eprintln!(
        "     samples placed                {:>12}",
        report.surface_samples
    );
    eprintln!("     implied sample spacing (mm)   {implied_spacing:>12.4}   = sqrt(area / N_S)");
    eprintln!("     stepover / 2 (mm)             {half_stepover:>12.4}");
    eprintln!(
        "     ratio spacing / (stepover/2)  {:>12.3}",
        implied_spacing / half_stepover
    );
    eprintln!(
        "     The module's own rule: \"sample spacing must be well under half the expected\n\
         \x20    stepover or ring spacing reads long.\" A ratio at or above 1.0 means the\n\
         \x20    DEFAULTS violate that rule on this region, and the failure is SILENT in the\n\
         \x20    coverage counters — uncovered_after_rings still reads 0 (every sample IS\n\
         \x20    covered; there simply were not enough of them). It surfaces only as inflated\n\
         \x20    Stage-C spacing. That is [SOURCE-2025] Table 1 case 1.5, a documented failure.\n\
         \x20    The defaults are NOT changed here — measuring what the defaults do is the\n\
         \x20    instrument's job — but without this line a Stage-C overshoot would be\n\
         \x20    unattributable between MAP, MECHANISM and SAMPLING. Stage E varies N_S."
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

    // -- the falsifier --
    eprintln!("\n   -- FALSIFIER (research note §B.4, concretised 2026-08-30) --");
    let mut fails: Vec<String> = Vec::new();
    if report.uncovered_after_bridging > 0 {
        fails.push(format!(
            "incomplete coverage after bridge repair: uncovered_after_bridging = {} of {} samples",
            report.uncovered_after_bridging, report.surface_samples
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
    eprintln!(
        "     coverage after bridging   {} (bar: 0)",
        report.uncovered_after_bridging
    );
    eprintln!(
        "     disk self-intersections   {} (bar: 0)",
        report.disk_self_intersections
    );
    eprintln!(
        "     bridge overhead           {:.3}% (bar: <= {MAX_BRIDGE_OVERHEAD_PCT:.1}%, inside the \
         §0k +43% trap with margin)",
        report.bridge_overhead_pct
    );

    // Structural claim gets a hard assert, not a print.
    assert_eq!(
        report.retract_count, 0,
        "conformal_spiral claims exactly one polyline and no lift anywhere in the \
         construction; retract_count must be 0 by construction"
    );

    if fails.is_empty() {
        eprintln!("\n     PASS: all three §B.4 conditions clear. Stages C and D run.\n");
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

fn stage_b(region: &Region, result: &SpiralResult, report: &SpiralReport) {
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
         <title>terrain_small F2: spiral in the unit-disk domain, {} rings, {} points</title>\n\
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
    let disk_path = output_dir.join("terrain_small_conformal_spiral_disk_f2.svg");
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
         <title>terrain_small F2: conformal spiral over the elliptical region, \
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
    let world_path = output_dir.join("terrain_small_conformal_spiral_xy_f2.svg");
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

fn stage_c(result: &SpiralResult, stepover_mm: f64, curvature: &CurvatureCensus) {
    /// Sample every Nth point of ring i+1. Restated from F1's stage C
    /// (`direction_field_wanaka_f1.rs:772`): spacing is a smooth quantity
    /// along a curve, so every point would multiply the work without adding a
    /// distinct measurement.
    const SAMPLE_STRIDE: usize = 5;
    /// Contract band around the target stepover, as F1 uses.
    const BAND: f64 = 0.25;

    eprintln!("========== STAGE F2-C — measured adjacent-ring spacing + scallop ==========\n");
    eprintln!(
        "   Method: every {SAMPLE_STRIDE}th point of ring i+1, 3D distance to the nearest\n\
         \x20  SEGMENT (not vertex) of ring i, over the PRE-BRIDGE rings (rings_contact,\n\
         \x20  outermost first). Geometry only — no simulation. Contact points, not CL.\n"
    );

    if result.rings_contact.len() < 2 {
        eprintln!("   SKIP: fewer than two rings — nothing adjacent to measure.\n");
        return;
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
        return;
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
}

// ── STAGE F2-D — CL conversion, containment, F-034 cost ─────────────────

struct StageDOutcome {
    cost: CandidateCost,
    cl_points: usize,
}

fn stage_d(
    fixture: Fixture<'_>,
    region: &Region,
    result: &SpiralResult,
    stepover_mm: f64,
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
    if spiral_cost.time_s > 0.0 {
        eprintln!(
            "     time ratio raster / spiral: {:.3}x",
            raster_cost.time_s / spiral_cost.time_s
        );
    }
    eprintln!(
        "\n     -- F-034 vs RASTER IS CONTEXT, NOT A PHASE-1 BAR --\n\
         \x20    research/conformal_finish_2026-08-28.md §B.4, verbatim: \"F-034 vs ball-end\n\
         \x20    raster on the friendly fixture is context, not a bar.\" The region here is a\n\
         \x20    CONVEX, hole-free ellipse 8 mm inside the mesh — chosen so every centroid test\n\
         \x20    is clean, which is precisely the geometry a raster handles best. The phase-1\n\
         \x20    bars are the three §B.4 conditions in Stage A. PROGRAMME.md's advance bar for\n\
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
        outcome: Result<SpiralReport, rs_cam_core::conformal_spiral::SpiralRefusal>,
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
        let outcome = match conformal_spiral::plan_spiral(mesh, index, &region.triangles, &params) {
            Ok((_, report)) => Ok(report),
            Err(refusal) => {
                eprintln!("     REFUSAL: {refusal:?}");
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

    eprintln!(
        "\n     {:<24} {:>8} {:>6} {:>7} {:>12} {:>12} {:>10} {:>12}",
        "row", "N_S", "N_C", "rings", "uncov(rings)", "uncov(bridge)", "overhead%", "spiral mm"
    );
    for row in &rows {
        match &row.outcome {
            Ok(r) => eprintln!(
                "     {:<24} {:>8} {:>6} {:>7} {:>12} {:>12} {:>10.3} {:>12.1}",
                row.label,
                row.n_s,
                row.n_c,
                r.ring_count,
                r.uncovered_after_rings,
                r.uncovered_after_bridging,
                r.bridge_overhead_pct,
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
        "\n     What to read here. `uncovered_*` at 0 across all three rows does NOT mean the\n\
         \x20    sampling is adequate — it means every sample that EXISTS is covered, which is\n\
         \x20    trivially easier with fewer samples. The signal is `rings` and `spiral mm`:\n\
         \x20    if halving N_S REDUCES the ring count, the coverage predicate was already\n\
         \x20    optimistic at the baseline and the spacing is reading long — Table 1 case 1.5.\n\
         \x20    Halving N_C coarsens the shared angular lattice that both runs and bridges are\n\
         \x20    sampled on, so it moves chord length, and through it the measured spiral\n\
         \x20    length and the disk self-intersection count, without moving the mechanism.\n"
    );
}

// ── the staged evidence run ─────────────────────────────────────────────

/// Phase F2 steps 1–2, end to end.
///
/// `#[ignore]` is for **runtime**, not for a missing input: this test needs no
/// external file — the fixture is in the repo — but it runs `plan_spiral`
/// three times over a 40k-triangle region, each of which sweeps start angles,
/// binary-searches every ring against tens of thousands of coverage samples,
/// and then costs two arms through the production relinker.
#[test]
#[ignore = "evidence run — long runtime (3 full plan_spiral solves on the 40k-triangle fixture); needs NO external files, the fixture is in-repo"]
fn terrain_small_conformal_spiral_f2() {
    use rs_cam_core::machine_kinematics::MachineKinematics;

    let path = fixture_path();
    assert!(
        path.exists(),
        "in-repo fixture missing: {} — this test needs no EXTERNAL file, but it does need this one",
        path.display()
    );

    eprintln!(
        "\n########## PHASE F2 — conformal-spiral evidence on fixtures/terrain_small.stl ##########\n"
    );

    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let stepover_mm = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);

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
    eprintln!("   safe_z {safe_z:.3}, effective min_z {effective_min_z:.3}");
    eprintln!(
        "   machine: accel [{:.0},{:.0},{:.0}] mm/s², junction dev {JUNCTION_DEVIATION_MM} mm, \
         rapid {RAPID_FEED_MM_MIN:.0}; feed {FEED_MM_MIN:.0}, plunge {PLUNGE_MM_MIN:.0} mm/min",
        MACHINE_ACCEL_XYZ[0], MACHINE_ACCEL_XYZ[1], MACHINE_ACCEL_XYZ[2]
    );
    eprintln!(
        "   equal-cusp stepover at R={BALL_RADIUS_MM}, h={CUSP_HEIGHT_MM}: {stepover_mm:.4} mm\n"
    );

    let mut params = SpiralParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    params.start_angle_step = SWEEP_START_ANGLE_STEP;

    eprintln!("   -- region selection (inset {REGION_INSET_MM} mm, ellipse, centroid test) --");
    let Some((region, result, report)) = plan_with_shrink(&mesh, &index, &params) else {
        panic!(
            "plan_spiral produced no plan on any of the {} ellipses tried — see the refusals \
             printed above",
            MAX_SHRINKS + 1
        );
    };
    eprintln!(
        "   region ACCEPTED after {} shrink(s): semi-axes ({:.3}, {:.3}) mm, \
         {} triangles, polygon area {:.1} mm²\n",
        region.shrinks,
        region.semi_axes.0,
        region.semi_axes.1,
        region.triangles.len(),
        region.polygon.area()
    );

    let region_area_3d = region_area_mm2(&mesh, &region.triangles);
    let proceed = stage_a(&report, &params, stepover_mm, region_area_3d);
    stage_b(&region, &result, &report);

    if proceed {
        let curvature = curvature_census(&mesh, &region.triangles);
        stage_c(&result, stepover_mm, &curvature);
        let fixture = Fixture {
            mesh: &mesh,
            index: &index,
            cutter: &cutter,
            kinematics,
            safe_z,
            effective_min_z,
        };
        if let Some(outcome) = stage_d(fixture, &region, &result, stepover_mm) {
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
        eprintln!("########## Stages C and D SKIPPED by the Stage A falsifier. ##########\n");
    }

    // Stage E runs on BOTH paths, deliberately: if the falsifier tripped, the
    // sampling sensitivity rows are the evidence for WHY it tripped.
    stage_e(&mesh, &index, &region, &params, &report);

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
    let trips = |r: &SpiralReport| -> bool {
        r.uncovered_after_bridging > 0
            || r.disk_self_intersections > 0
            || r.bridge_overhead_pct > MAX_BRIDGE_OVERHEAD_PCT
    };
    let clean = SpiralReport {
        bridge_overhead_pct: 4.2,
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
    // Exactly at the bar is a PASS — the condition is strictly greater.
    assert!(!trips(&SpiralReport {
        bridge_overhead_pct: MAX_BRIDGE_OVERHEAD_PCT,
        ..clean
    }));
}
