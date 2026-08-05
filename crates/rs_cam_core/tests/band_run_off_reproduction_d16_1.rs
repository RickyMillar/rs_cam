//! **REPRODUCTION — D-16.1: a `UnifiedFinish` band runs off the stock and
//! the cutter is commanded below the surface where there is no surface.**
//!
//! ## READ THIS FIRST — what form this file is in
//!
//! **This is a Tier-0 probe. It runs NO simulation.** It costs one mesh
//! build plus one `unified_finish_toolpath_with_cancel` call per arm (two
//! arms total, the shipped-`overlap_mm` one cached across three tests), and
//! it decides everything from the emitted toolpath and the mesh alone. No
//! dexel grid, no stock model, no `ProjectSession`. That is deliberate: the
//! defect is a GEOMETRIC one — cut targets at XY where the model has no
//! triangle — and a simulation would only re-discover it at a hundred times
//! the price, blurred through a cell size.
//!
//! **Tests 2 and 4 pin the CURRENT, DEFECTIVE behaviour and are GREEN
//! TODAY.** A green run means the defect is still present. They must be
//! INVERTED IN PLACE when the fix lands — each says how in its own doc
//! comment — never deleted and never `#[ignore]`d. This is the red-first
//! idiom wave W2 used in `arcfit_intent_boundary_f1.rs` and wave W8's
//! sibling `shallow_band_stock_to_leave_exhibit_d16_2.rs`.
//!
//! Tests 1, 3 and 5 are the non-vacuity apparatus and are green before and
//! after the fix.
//!
//! ## The defect
//!
//! `UnifiedFinish`'s MidSteep (scallop) band emits cutting moves at XY
//! positions **outside the model's footprint**, and at those positions the
//! commanded Z sits well below the top of the stock. On the H4 grooved-block
//! fixture the filed magnitude is **−235 µm** against a dialled cusp height
//! of 22.5 µm — roughly 10×. The tool is driven into virgin material beyond
//! the part.
//!
//! ## The mechanism, in two stages
//!
//! **Stage 1 — the band polygon grows OUTWARD past the mesh.**
//! `finish_planner::decompose` extracts each band's polygon with
//! `region_polygons_from_mask(&mask, origin_x, origin_y, cell,
//! params.overlap_mm.max(0.0))` (`finish_planner.rs:473`).
//! `region_mask::region_polygons_from_mask` (`region_mask.rs:49-71`) runs a
//! whole-grid Euclidean distance transform and **dilates the mask by
//! `dilate_mm` BEFORE marching squares, with no re-clamp to coverage
//! afterwards**:
//!
//! ```text
//! let dist = distance_transform_2d(mask.as_slice(), ny, nx);
//! let radius_cells = dilate_mm / cell;
//! Cow::Owned(dist.iter().map(|&d| d <= radius_cells).collect())
//! ```
//!
//! The classification grid is padded by the ENVELOPE radius (3.0 mm on this
//! tool — `finish_setup.rs:544-570`, via `ClassificationGridSpec::for_mesh`),
//! so there is room outside the footprint for the dilation to land in, and
//! it does. A MidSteep band polygon therefore extends `overlap_mm` past the
//! last covered classification cell — past the mesh footprint.
//!
//! **The dial ships at 2.0.** Note that the two defaults DISAGREE:
//! `FinishPlannerParams::for_tool` sets `overlap_mm: 0.0`
//! (`finish_planner.rs:204`), while the operation config
//! `UnifiedFinishConfig::default()` sets `overlap_mm: 2.0`
//! (`compute/operation_configs.rs:1047`) and `compute::execute` threads it
//! over the planner's value with `planner.overlap_mm = cfg.overlap_mm;`
//! (`compute/execute.rs:1815`). Every shipped `UnifiedFinish` op therefore
//! runs at 2.0, and a direct library caller that forgets to set it runs at
//! 0.0. This file sets it explicitly on both arms so neither default is
//! silently in play.
//!
//! **Stage 2 — a QUANTIZED coverage guard lets the off-footprint ring points
//! through.** The dilated band polygon becomes the scallop ring-cascade seed
//! boundary (`scallop.rs:2180-2183`; the first ring IS that boundary).
//! `ring_to_3d` (`scallop.rs:905-921`) keeps a ring vertex when
//! `finite && heightmap_covered_at_world(...)`, and
//! `heightmap_covered_at_world` (`scallop.rs:759-774`) **rounds the query XY
//! to the nearest cell of the GENERATION heightmap**:
//!
//! ```text
//! let col = ((x - hm.origin_x) / hm.cell_size).round();
//! let row = ((y - hm.origin_y) / hm.cell_size).round();
//! ```
//!
//! That grid's cell is `envelope_radius / 4` (`scallop.rs:1819-1824` selects
//! `FinishResolutionPolicy::legacy_envelope_quarter`;
//! `finish_setup.rs:354-376` builds the grid over `bbox ± envelope_radius`)
//! — **0.75 mm on this tool, six times the 0.125 mm classification cell**.
//! So a point up to HALF a generation cell (0.375 mm) outside the true
//! footprint rounds onto the last covered cell and is admitted. At such a
//! point `point_drop_cutter` returns a finite CL — but it is a RIM-RIDING
//! contact, the flank of the tool resting on the mesh edge from outside, not
//! a surface contact — and that CL becomes the commanded Z.
//!
//! Stage 1 alone would emit nothing (points 2 mm out fail the coverage
//! test). Stage 2 alone would emit nothing (an undilated seed has no points
//! outside to admit). Test 3 removes stage 1 and the population collapses,
//! which is what makes the pair the cause rather than a coincidence.
//!
//! ## Why the OTHER two bands do not show it
//!
//! * **Shallow** has an EXACT off-mesh guard (`unified_finish.rs:1895-1913`):
//!   per grid point, `index.query(x, y, 0.0)` then
//!   `mesh.faces[tri_idx].contains_point_xy(x, y)`, and a miss sets
//!   `contacted = false` so the raster never emits there. This file uses
//!   that identical predicate as its oracle — see [`on_footprint`] — so the
//!   test is not inventing a standard, it holds the MidSteep band to the one
//!   the Shallow band already meets.
//! * **All-over Scallop** (the H4 arm-D control, not run here) seeds its
//!   cascade from the mesh-bbox RECTANGLE (`scallop.rs:2072-2114`, the
//!   `_ => vec![boundary]` arm), which lies exactly ON the footprint, and
//!   every subsequent ring offsets INWARD. It never evaluates a CL outside
//!   the footprint at all. Arm B replaces that seed with a polygon dilated
//!   `overlap_mm` OUTWARD. That is the whole difference.
//!
//! ## Why the fixture is non-vacuous, and what test 1 guards
//!
//! `common::meshes::grooved_block(2.5, 70.0, 1.2)` (`tests/common/meshes.rs`,
//! `GroovedBlock::build`) is a heightfield that is a **function of X only**,
//! extruded along Y from `-y_half` to `+y_half` (12.0) with **no end walls**
//! — an open sheet. The 70° wall bands therefore run continuously to the
//! footprint edge at `y = ±12` and OPEN onto it. A convex or closed fixture
//! (a dome, an interior terrain patch) has no band that touches its own
//! footprint boundary, so the dilation would have nothing to run off and the
//! reproduction would pass by finding nothing.
//!
//! **Test 1 is the clause "its reproduction cannot pass on a convex /
//! no-boundary fixture" is guarding.** It proves by geometry — not by
//! assumption — that (a) the mesh's Y extent reaches ±12, (b) every vertex's
//! Z is a function of its X alone, so the mesh carries no end caps and is an
//! OPEN sheet, (c) triangles touching the `y = ±12` planes carry Z values
//! strictly between floor and rim, i.e. the 70° WALL reaches the end plane,
//! and it measures ~70° — squarely inside the shipped MidSteep window
//! (45°–75°), and (d) the footprint is exactly the bbox rectangle with no
//! interior holes, which is what licenses [`past_bbox_mm`] as a
//! distance-past-the-edge measure.
//!
//! The wall strips clear the absorption floor comfortably:
//! `min_region_area_mm2 = (2·cusp_radius)² · 4 = 4 mm²` for this Ø1-tip tool,
//! and each wall strip is `1.2 / tan(70°) = 0.437 mm` wide × 24 mm long ≈
//! **10.5 mm²**.
//!
//! ## Which assertions invert when the fix lands
//!
//! | test | today | after the fix |
//! |---|---|---|
//! | 1 fixture reaches its footprint edge | green | green (fixture geometry, not behaviour) |
//! | 2 emits cutting moves outside the footprint | **green — pins the defect** | **INVERT**: `!violations.is_empty()` becomes `violations.is_empty()` |
//! | 3 the overlap dial is the cause | green | green, trivially so once both arms are empty — keep it; it is what stops a future zero being read as a broken measurement |
//! | 4 off-footprint targets command a deeper Z | **green — pins the defect** | **COLLAPSE**: with no violations there is nothing to measure; fold it into test 2's inverted form |
//! | 5 the population is rendered | green | green (renders an empty population, which is the evidence) |
//!
//! The expected fix is to re-clamp the dilated mask to coverage in
//! `region_polygons_from_mask` (or to intersect each band polygon with the
//! coverage mask after extraction), so a seed boundary can never leave the
//! footprint. Tightening `heightmap_covered_at_world` alone would shrink the
//! run-off to the sub-cell scale without removing it.
//!
//! No production code is touched by this file. It is an instrument.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;

use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::finish_planner::FinishPlannerParams;
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::MoveIntent;
use rs_cam_core::unified_finish::{UnifiedFinishParams, unified_finish_toolpath_with_cancel};

use common::meshes::grooved_block;
use common::tools::tapered_cutter;

// ── Fixture constants ────────────────────────────────────────────────────
//
// Every one of these is copied from `strategy_comparison_h4.rs` — the
// harness this reproduction is evidence for — rather than invented, so the
// arm generated here is the arm that harness reports on.

/// `GroovedBlock::new`'s rim half-width (mm) — H4's `GROOVE_RIM_HALF_WIDTH_MM`.
const GROOVE_RIM_HALF_WIDTH_MM: f64 = 2.5;
/// Wall inclination from horizontal (deg) — H4's `GROOVE_WALL_DEG`. Sits
/// between the shipped 45°/75° thresholds, i.e. MidSteep/scallop territory.
const GROOVE_WALL_DEG: f64 = 70.0;
/// Groove depth (mm) — H4's `GROOVE_DEPTH_MM`.
const GROOVE_DEPTH_MM: f64 = 1.2;
/// `GroovedBlock`'s fixed X half-extent (mm). There is no builder setter for
/// it; test 1 asserts the mesh really has it rather than trusting this.
const GROOVE_X_EXTENT_MM: f64 = 20.0;
/// `GroovedBlock`'s fixed Y half-extent (mm) — the open end plane, and the
/// edge the band runs off.
const GROOVE_Y_HALF_MM: f64 = 12.0;

/// The project's finishing tool — H4's shared tool, held fixed so this is a
/// statement about a dial and not about a cutter. `envelope_radius_mm()` =
/// 3.0 (the shank), `cusp_radius_mm()` = 0.5 (the tip): the 6× split that
/// makes the generation grid (envelope/4 = 0.75 mm) six times coarser than
/// the classification grid (cusp/4 = 0.125 mm), which is stage 2's slop.
const TIP_DIAMETER_MM: f64 = 1.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;
const SHANK_DIAMETER_MM: f64 = 6.0;

/// The shipped `overlap_mm` — `UnifiedFinishConfig::default()`
/// (`compute/operation_configs.rs:1047`), threaded onto the planner by
/// `compute/execute.rs:1815`. THE VARIABLE of tests 2 vs 3.
const SHIPPED_OVERLAP_MM: f64 = 2.0;

/// The control's `overlap_mm`: no dilation, i.e. stage 1 removed.
const NO_OVERLAP_MM: f64 = 0.0;

/// How much smaller the control arm's run-off population must be (test 3).
const CONTROL_REDUCTION_FACTOR: usize = 10;

/// `0.3² / (8 · 0.5 · 1.0)` = 22.5 µm — the flat-surface cusp a 0.3 mm
/// stepover leaves on this tool's 0.5 mm tip radius. Spelled as arithmetic,
/// copied verbatim from `strategy_comparison_h4.rs::cusp_mm`, so parity with
/// that harness's quality target is checkable.
fn cusp_mm() -> f64 {
    0.3 * 0.3 / (8.0 * 0.5 * TIP_DIAMETER_MM)
}

fn fixture_mesh() -> TriangleMesh {
    grooved_block(GROOVE_RIM_HALF_WIDTH_MM, GROOVE_WALL_DEG, GROOVE_DEPTH_MM)
}

fn fixture_cutter() -> TaperedBallEndmill {
    tapered_cutter(TIP_DIAMETER_MM, TAPER_HALF_ANGLE_DEG, SHANK_DIAMETER_MM)
}

/// H4's arm-B dial set, mapped from `UnifiedFinishConfig` onto the core
/// struct the library entry point takes. `safe_z`, `intra_region_hookup_mm`
/// and `classification_sampler` come from `UnifiedFinishParams::default()`
/// because H4's arm B leaves all three at their shipped values — this file
/// pins nothing the shipped op does not pin.
fn arm_b_params() -> UnifiedFinishParams {
    UnifiedFinishParams {
        scallop_height: cusp_mm(),
        tolerance: 0.05,
        raster_stepover: 0.3,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        ..UnifiedFinishParams::default()
    }
}

// ── The oracle ───────────────────────────────────────────────────────────

/// Is `(x, y)` inside the model's footprint?
///
/// **This is the Shallow band's own guard, character for character**
/// (`unified_finish.rs:1895-1913`): ask the spatial index for candidate
/// triangles at zero radius, then run an exact barycentric XY containment
/// test on each. Nothing here is a tolerance, a heightmap lookup or a Z
/// heuristic — a Z heuristic would be circular, since `point_drop_cutter`
/// returns a finite Z at precisely the off-footprint points this function
/// exists to identify.
fn on_footprint(mesh: &TriangleMesh, index: &SpatialIndex, x: f64, y: f64) -> bool {
    for &tri_idx in &index.query(x, y, 0.0) {
        if let Some(tri) = mesh.faces.get(tri_idx)
            && tri.contains_point_xy(x, y)
        {
            return true;
        }
    }
    false
}

/// Euclidean distance from `(x, y)` to the mesh's bbox rectangle; `0.0`
/// inside it.
///
/// Only a valid stand-in for "distance past the footprint edge" because
/// test 1 proves this fixture's footprint IS its bbox rectangle, with no
/// interior holes. On a holed mesh it would under-report, and this file
/// would need a real polygon distance.
fn past_bbox_mm(bbox: &BoundingBox3, x: f64, y: f64) -> f64 {
    let dx = (bbox.min.x - x).max(x - bbox.max.x).max(0.0);
    let dy = (bbox.min.y - y).max(y - bbox.max.y).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

// ── The population ───────────────────────────────────────────────────────

/// One emitted move target with everything the assertions need.
#[derive(Clone, Copy, Debug)]
struct Target {
    move_index: usize,
    x: f64,
    y: f64,
    z: f64,
    intent: MoveIntent,
}

/// A move is a CUT when it is not a rapid AND its generator tagged it as
/// material removal.
///
/// Selection is BY INTENT, never by a Z heuristic and never by a label
/// literal — the same rule `shallow_band_stock_to_leave_exhibit_d16_2.rs`
/// states and for the same reason. `MoveIntent::Linking` is deliberately
/// EXCLUDED from the asserted population and counted separately: intra-region
/// stay-down links (`intra_region_hookup_mm`, shipped at 6.0) are
/// surface-following feeds built by `surface_link::build_surface_link`, a
/// different emitter with its own coverage handling. An off-footprint link is
/// a real problem too, but it is not the mechanism this file traces, and
/// folding it in would make the count un-attributable.
fn is_cut(intent: MoveIntent) -> bool {
    matches!(intent, MoveIntent::FinishingCut | MoveIntent::EntryPlunge)
}

/// Everything one generation produced, reduced to plain data so it can live
/// in a `OnceLock` and be shared across tests without re-generating.
struct Probe {
    overlap_mm: f64,
    /// Every cutting-move target, in emission order.
    cutting: Vec<Target>,
    /// The subset whose `(x, y)` is OFF the model footprint — the defect.
    violations: Vec<Target>,
    /// Off-footprint targets on `Linking` moves. Reported, never asserted on.
    linking_violations: usize,
    total_moves: usize,
    shallow_moves: usize,
    mid_steep_moves: usize,
    very_steep_moves: usize,
    /// `(band/strategy label, cutting targets, violating targets)`, taken
    /// from `UnifiedFinishReport::region_table`'s `move_range` spans — so a
    /// violation is attributed to the generator that emitted it rather than
    /// guessed at from its coordinates.
    attribution: Vec<(String, usize, usize)>,
}

impl Probe {
    fn worst_past_edge_mm(&self, bbox: &BoundingBox3) -> f64 {
        self.violations
            .iter()
            .map(|t| past_bbox_mm(bbox, t.x, t.y))
            .fold(0.0_f64, f64::max)
    }

    /// `(min, max)` Y over the violating population; `(NaN, NaN)` when empty.
    fn y_range(&self) -> (f64, f64) {
        let lo = self.violations.iter().map(|t| t.y).fold(f64::NAN, f64::min);
        let hi = self.violations.iter().map(|t| t.y).fold(f64::NAN, f64::max);
        (lo, hi)
    }

    /// `(min, max)` X over the violating population; `(NaN, NaN)` when empty.
    fn x_range(&self) -> (f64, f64) {
        let lo = self.violations.iter().map(|t| t.x).fold(f64::NAN, f64::min);
        let hi = self.violations.iter().map(|t| t.x).fold(f64::NAN, f64::max);
        (lo, hi)
    }

    fn count_intent(&self, intent: MoveIntent) -> usize {
        self.violations
            .iter()
            .filter(|t| t.intent == intent)
            .count()
    }

    fn attribution_table(&self) -> String {
        let mut s = String::new();
        for (label, cuts, bad) in &self.attribution {
            let _ = writeln!(s, "         {label}: {bad} off-footprint of {cuts} cutting");
        }
        if s.is_empty() {
            s.push_str("         (no region spans reported)\n");
        }
        s
    }
}

/// Generate ONE `UnifiedFinish` arm and collect its run-off population.
///
/// **`overlap_mm` is the only argument, and the only thing tests 2 and 3
/// differ in.** Fixture, tool, every other planner dial and every
/// `UnifiedFinishParams` field come from the shared helpers above, so the
/// A/B cannot drift into a two-variable comparison.
fn probe(overlap_mm: f64) -> Probe {
    let mesh = fixture_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = fixture_cutter();
    let params = arm_b_params();
    // `for_tool`'s parameter is a CUSP radius, never `radius()` — on this
    // tapered tool `radius()` reports the 3.0 mm shank, which would put
    // `min_region_area_mm2` at 144 mm² and absorb the 10.5 mm² wall strips
    // this reproduction depends on (`finish_planner.rs`'s own doc, §14q).
    let planner = FinishPlannerParams {
        overlap_mm,
        ..FinishPlannerParams::for_tool(cutter.cusp_radius_mm())
    };
    let top_z = mesh.bbox.max.z + 2.0;
    let bottom_z = mesh.bbox.min.z - 1.0;
    let never_cancel = || false;

    let (toolpath, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        top_z,
        bottom_z,
        &params,
        &planner,
        /* machining_boundary */ None,
        /* link_kinematics */ None,
        /* claims */ None,
        /* debug */ None,
        &never_cancel,
    )
    .expect("uncancelled generation");

    let mut cutting: Vec<Target> = Vec::new();
    let mut violations: Vec<Target> = Vec::new();
    let mut linking_violations = 0usize;
    // One containment answer per distinct XY: the O(moves) sweep would
    // otherwise re-query the index for the many targets that repeat a
    // coordinate (ring closures, retract/plunge pairs).
    let mut seen: HashMap<(i64, i64), bool> = HashMap::new();

    for (move_index, mv) in toolpath.moves.iter().enumerate() {
        if !mv.move_type.is_cutting() {
            continue;
        }
        let interesting = is_cut(mv.intent) || mv.intent == MoveIntent::Linking;
        if !interesting {
            continue;
        }
        let (x, y, z) = (mv.target.x, mv.target.y, mv.target.z);
        let key = ((x * 1e6).round() as i64, (y * 1e6).round() as i64);
        let inside = match seen.get(&key) {
            Some(&v) => v,
            None => {
                let v = on_footprint(&mesh, &index, x, y);
                seen.insert(key, v);
                v
            }
        };
        if is_cut(mv.intent) {
            let t = Target {
                move_index,
                x,
                y,
                z,
                intent: mv.intent,
            };
            cutting.push(t);
            if !inside {
                violations.push(t);
            }
        } else if !inside {
            linking_violations += 1;
        }
    }

    // Attribute BY REGION SPAN (`RegionTableEntry::move_range`), never by a
    // coordinate guess. The spans tile the stitched toolpath — sentry
    // `region_node_ranges_tile_the_stitched_toolpath` — so "unattributed"
    // appearing here would itself be news.
    let violating_indices: HashSet<usize> = violations.iter().map(|t| t.move_index).collect();
    let mut attribution: Vec<(String, usize, usize)> = Vec::new();
    for t in &cutting {
        let label = report
            .region_table
            .iter()
            .find(|e| e.move_range.contains(&t.move_index))
            .map_or_else(
                || "unattributed".to_owned(),
                |e| format!("{}/{}", e.kind.band_label(), e.kind.strategy().label()),
            );
        let bad = violating_indices.contains(&t.move_index);
        match attribution.iter().position(|(l, _, _)| *l == label) {
            Some(i) => {
                attribution[i].1 += 1;
                if bad {
                    attribution[i].2 += 1;
                }
            }
            None => attribution.push((label, 1, usize::from(bad))),
        }
    }
    attribution.sort_by(|a, b| a.0.cmp(&b.0));

    Probe {
        overlap_mm,
        cutting,
        violations,
        linking_violations,
        total_moves: toolpath.moves.len(),
        shallow_moves: report.shallow.move_count,
        mid_steep_moves: report.mid_steep.move_count,
        very_steep_moves: report.very_steep.move_count,
        attribution,
    }
}

/// The shipped-`overlap_mm` arm, generated once and shared by tests 2, 4 and
/// 5. Integration tests run as threads in one process, so a `OnceLock` here
/// turns five would-be generations into two.
fn shipped_probe() -> &'static Probe {
    static CELL: OnceLock<Probe> = OnceLock::new();
    CELL.get_or_init(|| probe(SHIPPED_OVERLAP_MM))
}

// ── Test 1: the FIXTURE is non-vacuous ───────────────────────────────────

/// **Control — the grooved-block fixture actually reaches its own footprint
/// edge, with an OPEN wall.**
///
/// GREEN BEFORE AND AFTER THE FIX. This is the clause *"its reproduction
/// cannot pass on a convex / no-boundary fixture"* is guarding: a dome or an
/// interior terrain patch has no band that touches its footprint boundary,
/// so `overlap_mm`'s dilation would have nothing to run off, and tests 2–4
/// would report zero and call it clean.
///
/// Four geometric facts, each asserted rather than assumed:
///
/// 1. the bbox is `x ∈ [±20]`, `y ∈ [±12]`, `z ∈ [−1.2, 0]`;
/// 2. every vertex's Z is a function of its X alone — so the mesh carries no
///    end caps at `y = ±12` and is an OPEN sheet;
/// 3. triangles touching the `y = ±12` planes carry Z values strictly
///    between floor and rim, i.e. the 70° WALL reaches the end plane (a
///    fixture whose ends happened to be flat rim would have a boundary but
///    no mid-steep band on it), and that wall measures ~70°, inside the
///    shipped 45°–75° MidSteep window;
/// 4. the footprint is the bbox rectangle with no interior holes — which is
///    what licenses [`past_bbox_mm`] as a distance-past-the-edge measure.
#[test]
fn the_grooved_block_fixture_actually_reaches_its_footprint_edge() {
    let mesh = fixture_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let bbox = &mesh.bbox;

    // ── 1. extents ──────────────────────────────────────────────────────
    for (name, got, want) in [
        ("bbox.min.x", bbox.min.x, -GROOVE_X_EXTENT_MM),
        ("bbox.max.x", bbox.max.x, GROOVE_X_EXTENT_MM),
        ("bbox.min.y", bbox.min.y, -GROOVE_Y_HALF_MM),
        ("bbox.max.y", bbox.max.y, GROOVE_Y_HALF_MM),
        ("bbox.min.z", bbox.min.z, -GROOVE_DEPTH_MM),
        ("bbox.max.z", bbox.max.z, 0.0),
    ] {
        assert!(
            (got - want).abs() < 1e-9,
            "fixture geometry moved: {name} is {got}, expected {want}. Every \
             coordinate bar in this file is written against the \
             grooved_block(2.5, 70.0, 1.2) footprint; if GroovedBlock's \
             extents changed, re-derive them — do not relax this."
        );
    }

    // ── 2. no end caps: Z is a function of X alone ──────────────────────
    let mut z_of_x: HashMap<i64, f64> = HashMap::new();
    let mut inconsistent = 0usize;
    for v in &mesh.vertices {
        let key = (v.x * 1e6).round() as i64;
        match z_of_x.get(&key) {
            Some(&z) if (z - v.z).abs() > 1e-9 => inconsistent += 1,
            Some(_) => {}
            None => {
                z_of_x.insert(key, v.z);
            }
        }
    }
    assert_eq!(
        inconsistent, 0,
        "{inconsistent} vertices share an X with a DIFFERENT Z, so this mesh \
         is no longer a pure X-profile extrusion. The whole reason this \
         fixture reproduces D-16.1 is that it is an OPEN sheet — a capped or \
         closed solid would have a vertical end wall at y = ±{GROOVE_Y_HALF_MM}, \
         the 70° band would terminate against that wall rather than opening \
         onto the footprint edge, and the run-off would have nowhere to go."
    );

    // ── 3. the WALL, not the rim, reaches each end plane ────────────────
    let floor_z = -GROOVE_DEPTH_MM;
    let mut wall_tris_at_pos_y = 0usize;
    let mut wall_tris_at_neg_y = 0usize;
    for tri in &mesh.faces {
        let on_wall = tri.v.iter().any(|p| p.z > floor_z + 1e-6 && p.z < -1e-6);
        if !on_wall {
            continue;
        }
        if tri.v.iter().any(|p| (p.y - GROOVE_Y_HALF_MM).abs() < 1e-9) {
            wall_tris_at_pos_y += 1;
        }
        if tri.v.iter().any(|p| (p.y + GROOVE_Y_HALF_MM).abs() < 1e-9) {
            wall_tris_at_neg_y += 1;
        }
    }
    assert!(
        wall_tris_at_pos_y > 0 && wall_tris_at_neg_y > 0,
        "the 70° wall does not reach the end planes: {wall_tris_at_pos_y} \
         triangles carry a wall-height vertex at y = +{GROOVE_Y_HALF_MM} and \
         {wall_tris_at_neg_y} at y = −{GROOVE_Y_HALF_MM} (both must be > 0). \
         A fixture whose boundary is all flat rim has a footprint edge but no \
         MID-STEEP band on it, and D-16.1's mechanism runs entirely through \
         the mid-steep band's dilated polygon — so such a fixture would let \
         tests 2–4 pass by finding nothing."
    );

    // ── 3b. that wall really is mid-steep territory ─────────────────────
    let wall: Vec<(f64, f64)> = mesh
        .vertices
        .iter()
        .filter(|p| p.x > 0.0 && p.z > floor_z + 1e-6 && p.z < -1e-6)
        .map(|p| (p.x, p.z))
        .collect();
    assert!(
        wall.len() >= 2,
        "found {} vertices on the right-hand wall interior — cannot measure \
         its angle, so cannot show the fixture has a mid-steep band at all",
        wall.len()
    );
    let x_lo = wall.iter().map(|w| w.0).fold(f64::INFINITY, f64::min);
    let x_hi = wall.iter().map(|w| w.0).fold(f64::NEG_INFINITY, f64::max);
    let z_lo = wall.iter().map(|w| w.1).fold(f64::INFINITY, f64::min);
    let z_hi = wall.iter().map(|w| w.1).fold(f64::NEG_INFINITY, f64::max);
    let measured_deg = ((z_hi - z_lo) / (x_hi - x_lo)).atan().to_degrees();
    assert!(
        (measured_deg - GROOVE_WALL_DEG).abs() < 1.0,
        "the wall measures {measured_deg:.2}° from horizontal, not \
         {GROOVE_WALL_DEG}°. It must land strictly inside the shipped band \
         window (steep_threshold_deg 45 .. waterline_threshold_deg 75), or the \
         strip is classified Shallow (raster — which has an EXACT off-mesh \
         guard) or VerySteep (waterline), and the scallop ring cascade this \
         reproduction traces never runs on it."
    );

    // ── 4. the footprint IS the bbox rectangle (no interior holes) ──────
    let step = 0.2;
    let inset = 0.1;
    let mut sampled = 0usize;
    let mut uncovered = 0usize;
    let mut first_gap = (0.0_f64, 0.0_f64);
    let mut y = bbox.min.y + inset;
    while y <= bbox.max.y - inset {
        let mut x = bbox.min.x + inset;
        while x <= bbox.max.x - inset {
            sampled += 1;
            if !on_footprint(&mesh, &index, x, y) {
                if uncovered == 0 {
                    first_gap = (x, y);
                }
                uncovered += 1;
            }
            x += step;
        }
        y += step;
    }
    assert!(
        sampled > 10_000,
        "only {sampled} interior points were sampled — too few to claim the \
         footprint has no holes"
    );
    assert_eq!(
        uncovered, 0,
        "{uncovered} of {sampled} interior samples are NOT covered by any \
         triangle (first at {:.3}, {:.3}). The fixture has an interior hole, \
         so 'outside the footprint' is no longer the same as 'outside the \
         bbox rectangle', and past_bbox_mm would under-report every distance \
         in this file. Replace it with a real polygon distance before \
         trusting tests 2–5.",
        first_gap.0, first_gap.1,
    );
}

// ── Test 2: THE REPRODUCTION ─────────────────────────────────────────────

/// **REPRODUCTION — D-16.1: `UnifiedFinish` emits cutting moves at XY
/// positions the model does not cover.**
///
/// **THIS TEST PINS A DEFECT. IT IS GREEN TODAY AND IS EXPECTED TO FAIL WHEN
/// THE FIX LANDS.**
///
/// One arm, shipped dials, `overlap_mm = 2.0`. Every cutting-move target is
/// tested against [`on_footprint`] — the Shallow band's OWN guard
/// (`unified_finish.rs:1895-1913`), not a new standard invented here.
///
/// A non-empty violating population means the tool is commanded to cut where
/// the part is not. See the module header for the two-stage mechanism, and
/// test 4 for how deep it goes.
///
/// **To invert when the fix lands: replace `!p.violations.is_empty()` with
/// `p.violations.is_empty()` and turn the message inside out. Nothing else in
/// this test changes** — the population, the oracle and the arm are all
/// unchanged by the fix.
#[test]
fn unified_finish_emits_cutting_moves_outside_the_model_footprint() {
    let mesh = fixture_mesh();
    let p = shipped_probe();

    // ── non-vacuity first: a count measured over an empty population is
    //    not a measurement, whatever it comes out as. ────────────────────
    assert!(
        p.total_moves > 0,
        "the arm generated NO moves at all — every assertion below would be \
         about an empty toolpath"
    );
    assert!(
        !p.cutting.is_empty(),
        "the arm generated {} moves but NONE tagged FinishingCut/EntryPlunge, \
         so the by-intent population is empty. Investigate the intent tagging \
         rather than relaxing the filter.",
        p.total_moves
    );
    assert!(
        p.mid_steep_moves > 0,
        "THE REPRODUCTION IS VACUOUS: the MID-STEEP band emitted no moves \
         (shallow {}, mid_steep {}, very_steep {}). D-16.1's mechanism runs \
         entirely through the mid-steep scallop ring cascade — the dilated \
         band polygon becomes the cascade seed (scallop.rs:2180-2183). A 70° \
         wall strip of ~10.5 mm² against a min_region_area_mm2 of {:.1} mm² \
         must survive absorption, so an empty band means classification or \
         region conditioning moved. Fix the fixture; do not delete this \
         assertion.",
        p.shallow_moves,
        p.mid_steep_moves,
        p.very_steep_moves,
        (2.0 * fixture_cutter().cusp_radius_mm()).powi(2) * 4.0,
    );

    let n_bad = p.violations.len();
    let n_all = p.cutting.len();
    let worst_past = p.worst_past_edge_mm(&mesh.bbox);
    let (y_lo, y_hi) = p.y_range();
    let (x_lo, x_hi) = p.x_range();
    let pct = 100.0 * n_bad as f64 / n_all as f64;
    let links = p.linking_violations;
    let table = p.attribution_table();
    let half_gen_cell_mm = fixture_cutter().envelope_radius_mm() / 8.0;

    assert!(
        p.violations.is_empty(),
        "SENTRY (was D-16.1's reproduction, INVERTED 2026-08-06 when the fix \
         landed).\n\
         \n\
         UnifiedFinish emitted {n_bad} of {n_all} cutting-move targets \
         ({pct:.2}%) at XY positions NO triangle of the model covers, \
         measured with the Shallow band's own guard \
         (index.query(x, y, 0.0) + Triangle::contains_point_xy). It must emit \
         ZERO.\n\
         \n\
         Furthest past the footprint edge: {worst_past:.4} mm. Violating X \
         range {x_lo:.3} .. {x_hi:.3} mm; Y range {y_lo:.3} .. {y_hi:.3} mm — \
         the footprint ends at y = ±{GROOVE_Y_HALF_MM}. Half a generation \
         cell is {half_gen_cell_mm:.3} mm; a worst distance near that value \
         is the signature of the ORIGINAL defect returning.\n\
         \n\
         Attributed by region span:\n\
         {table}\
         Off-footprint LINKING targets (excluded from the assertion — \
         different emitter, see is_cut's doc): {links}.\n\
         \n\
         What this used to be, and what fixed it. At the shipped \
         overlap_mm = {overlap} the MidSteep band polygon is dilated 2 mm \
         OUTWARD by region_polygons_from_mask before marching squares, and \
         that polygon is the scallop ring-cascade seed \
         (scallop.rs, `generate_scallop_rings` call site). ring_to_3d's \
         coverage guard used to read the 0.75 mm GENERATION heightmap with \
         nearest-cell rounding — six times coarser than the 0.125 mm \
         classification cell — so it admitted ring vertices up to half a \
         generation cell (0.375 mm) outside the footprint, where \
         point_drop_cutter returns a rim-riding CL that cuts \
         `r − √(r² − d²)` below the surface. Measured then: 131 \
         off-footprint of 3287 MidSteep cutting targets, worst 0.3750 mm — \
         the quantization bound on the nose. F2 replaced that guard with the \
         EXACT point-in-triangle test the Shallow band next door already \
         used (`dropcutter::point_is_over_mesh_xy`), and the population went \
         to zero.\n\
         \n\
         If this test fails, a coverage guard somewhere on the scallop ring \
         path has gone back to consulting a sampled mask. Do not relax it.",
        overlap = p.overlap_mm,
    );
}

// ── Test 3: the CONTROL that makes test 2 non-vacuous ────────────────────

/// **Control — the `overlap_mm` dilation is the CAUSE, not an incidental
/// property of scallop.**
///
/// GREEN BEFORE AND AFTER THE FIX. Without it, test 2's non-empty population
/// has a second explanation: "scallop always wanders off a mesh edge, this
/// has nothing to do with the planner." Removing stage 1 — and ONLY stage 1,
/// via [`probe`]'s single argument — must collapse it.
///
/// **Why the bar is a ratio and not `== 0`.** Stage 2 (the rounded coverage
/// guard in `heightmap_covered_at_world`) is still present in the control
/// arm, and marching squares itself places a contour between grid samples, so
/// an undilated band polygon can still sit a fraction of a classification cell
/// proud of the last covered cell. A handful of surviving points would be that
/// residue, not the defect. A 10× collapse (`CONTROL_REDUCTION_FACTOR`)
/// separates "the dilation put the tool 2 mm out" from "grid quantisation put
/// it 60 µm out" without pretending the second effect does not exist. The
/// measured reduction is reported in the message either way.
#[test]
fn the_overlap_dial_is_the_cause() {
    let shipped = shipped_probe();
    let control = probe(NO_OVERLAP_MM);
    let mesh = fixture_mesh();

    // 2026-08-06: this used to assert `!shipped.violations.is_empty()` —
    // "the control is vacuous if the shipped arm has no run-off". Post-fix
    // BOTH arms are empty by design, so that guard would fail forever. The
    // control survives because what it really proves is that the dilation
    // does not, on its own, put cut targets off the footprint — and it now
    // proves that from zero on both sides, which is the stronger statement.
    // The non-vacuity that matters is BELOW: the control arm must still
    // generate a real mid-steep band, or "zero run-off" would just mean
    // "nothing was generated".
    assert!(
        !shipped.cutting.is_empty(),
        "THE CONTROL IS VACUOUS: the shipped arm emitted no cutting targets \
         at all ({} moves total)",
        shipped.total_moves
    );
    assert!(
        control.mid_steep_moves > 0,
        "THE CONTROL IS VACUOUS: at overlap_mm = {NO_OVERLAP_MM} the MID-STEEP \
         band emitted no moves (shallow {}, mid_steep {}, very_steep {}), so \
         its empty run-off population would only prove that nothing was \
         generated. The dilation must not decide whether the band EXISTS — it \
         is applied at polygon extraction, after classification and after \
         min-area absorption (finish_planner.rs step 4, then step 6) — so an \
         empty band here means something upstream moved.",
        control.shallow_moves,
        control.mid_steep_moves,
        control.very_steep_moves,
    );
    assert!(
        !control.cutting.is_empty(),
        "THE CONTROL IS VACUOUS: the control arm emitted no cutting targets \
         at all ({} moves total)",
        control.total_moves
    );

    let n_shipped = shipped.violations.len();
    let n_control = control.violations.len();
    let ratio = if n_control == 0 {
        f64::INFINITY
    } else {
        n_shipped as f64 / n_control as f64
    };
    let worst_shipped = shipped.worst_past_edge_mm(&mesh.bbox);
    let worst_control = control.worst_past_edge_mm(&mesh.bbox);

    assert!(
        n_control == 0 && n_shipped == 0,
        "BOTH arms must now be free of off-footprint cut targets — the \
         coverage guard is exact, so the overlap dial can no longer put one \
         there whatever it is set to.\n\
         \n\
         overlap_mm = {shipped_dial}: {n_shipped} off-footprint cutting \
         targets of {shipped_all}, worst {worst_shipped:.4} mm past the edge.\n\
         overlap_mm = {control_dial}: {n_control} of {control_all}, worst \
         {worst_control:.4} mm past the edge.\n\
         Ratio {ratio:.2}× (was the measure when this was a reduction test; \
         the bar was {CONTROL_REDUCTION_FACTOR}×).\n\
         \n\
         Before F2 this test measured a COLLAPSE rather than a zero: the \
         shipped arm ran 131 targets off the footprint and the undilated \
         control a small fraction of that, which is what attributed the \
         population to `overlap_mm`. That attribution is now history, and \
         the assertion is the stronger one it enabled.",
        shipped_dial = shipped.overlap_mm,
        control_dial = control.overlap_mm,
        shipped_all = shipped.cutting.len(),
        control_all = control.cutting.len(),
    );
}

// ── Test 4: no excluded sentinel reaches the emitted path ───────────────

/// **What is left of the depth-domain reproduction, once the population it
/// measured is gone.**
///
/// This test used to be `off_footprint_targets_command_a_deeper_z_than_the_
/// surface`: it took D-16.1's 131 off-footprint targets, measured each
/// against the stock top, and asserted the worst gouge exceeded 2× the
/// dialled cusp. On the filed configuration it read an order of magnitude
/// above that bar — the rim-riding CL, the flank of the tool resting on the
/// mesh edge from outside, with no surface under it at all.
///
/// **After F2 there are no off-footprint targets, so that measurement has no
/// population and was retired rather than left to pass by finding nothing.**
/// Its EXISTENCE half is test 2's, inverted. What survives here is its other
/// assertion, which never depended on the run-off population and guards a
/// separate, worse failure: `ring_to_3d` marks an off-mesh vertex excluded
/// and parks it at `min_z + stock_to_leave`, and the shared run-splitter is
/// supposed to retract around every such stretch. If one ever reaches the
/// emitted path, the tool is commanded to the bottom of the Z range in
/// mid-air. A non-finite drop-cutter probe under an EMITTED cutting target
/// is that leak's signature.
///
/// Non-vacuity: this runs over every cutting target the arm emits (thousands
/// on this fixture), not over a filtered subset, so it cannot pass by finding
/// nothing. The count is asserted.
#[test]
fn no_emitted_cut_target_sits_where_the_surface_probe_finds_nothing() {
    let mesh = fixture_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = fixture_cutter();
    let p = shipped_probe();

    assert!(
        p.cutting.len() > 1000,
        "population guard: only {} cutting targets — this fixture emits \
         thousands, so something upstream moved and a green below would mean \
         nothing",
        p.cutting.len(),
    );

    let mut nonfinite: Vec<(f64, f64, f64)> = Vec::new();
    for t in &p.cutting {
        let cl = point_drop_cutter(t.x, t.y, &mesh, &index, &cutter);
        if !cl.z.is_finite() {
            nonfinite.push((t.x, t.y, t.z));
        }
    }

    assert!(
        nonfinite.is_empty(),
        "{} of {} EMITTED cutting targets sit at an XY where point_drop_cutter \
         finds no surface at all (first: {:?}).\n\
         \n\
         A kept ring vertex requires `finite && point_is_covered(...)`, and a \
         refinement probe failing either test pushes an EXCLUDED point at \
         `min_z + stock_to_leave` and returns — which the run-splitter is \
         supposed to turn into a retract. A non-finite probe under an emitted \
         cut therefore means an excluded sentinel leaked into the path and \
         the tool is being sent to the bottom of the Z range in mid-air. That \
         is a separate and worse defect from D-16.1; file it as one.",
        nonfinite.len(),
        p.cutting.len(),
        nonfinite.first(),
    );
}

// ── Test 5: the RENDER ───────────────────────────────────────────────────

/// **The run-off population is rendered, because a verdict without a render
/// is not allowed here.**
///
/// GREEN BEFORE AND AFTER THE FIX — after it, the images show an empty
/// population against a populated footprint, which is exactly the evidence
/// wanted.
///
/// Writes to `planning/review_2026-08-04/artifacts/w8/`:
///
/// | file | what it shows |
/// |---|---|
/// | `d16_1_runoff_xy_all.pgm` | top-down XY of EVERY cutting target (dim), violating ones bright, footprint rectangle outlined |
/// | `d16_1_runoff_end_strip_y_pos.pgm` | the same, zoomed to `y ∈ [10, 15]` |
/// | `d16_1_runoff_end_strip_y_neg.pgm` | the same, zoomed to `y ∈ [−15, −10]` |
/// | `d16_1_runoff_summary.txt` | the numbers, so they survive outside an assertion message |
///
/// Raw P5 PGM, following `reference_plate_contract.rs` — a header plus an
/// 8-bit buffer, no image-crate dependency and nothing to go wrong at
/// serialisation. Greyscale codes: 0 background, 96 on-footprint cutting
/// target, 160 footprint outline, 255 off-footprint target.
#[test]
fn the_run_off_population_is_rendered_to_an_artifact() {
    let mesh = fixture_mesh();
    let p = shipped_probe();
    let bbox = &mesh.bbox;

    assert!(
        !p.cutting.is_empty(),
        "nothing to render: the arm emitted no cutting targets"
    );

    let pad = 3.5;
    let full = (
        bbox.min.x - pad,
        bbox.max.x + pad,
        bbox.min.y - pad,
        bbox.max.y + pad,
    );
    let strip_pos = (bbox.min.x - pad, bbox.max.x + pad, 10.0, 15.0);
    let strip_neg = (bbox.min.x - pad, bbox.max.x + pad, -15.0, -10.0);

    let mut written: Vec<PathBuf> = vec![
        render_pgm(
            "d16_1_runoff_xy_all.pgm",
            full,
            0.1,
            bbox,
            &p.cutting,
            &p.violations,
        ),
        render_pgm(
            "d16_1_runoff_end_strip_y_pos.pgm",
            strip_pos,
            0.05,
            bbox,
            &p.cutting,
            &p.violations,
        ),
        render_pgm(
            "d16_1_runoff_end_strip_y_neg.pgm",
            strip_neg,
            0.05,
            bbox,
            &p.cutting,
            &p.violations,
        ),
    ];

    // ── the numbers, written where a human can find them ────────────────
    let (y_lo, y_hi) = p.y_range();
    let (x_lo, x_hi) = p.x_range();
    let finishing_cuts = p.count_intent(MoveIntent::FinishingCut);
    let entry_plunges = p.count_intent(MoveIntent::EntryPlunge);
    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "D-16.1 band run-off reproduction (Tier 0 probe, NO simulation)\n\
         fixture: grooved_block({GROOVE_RIM_HALF_WIDTH_MM}, {GROOVE_WALL_DEG}, \
         {GROOVE_DEPTH_MM}) — footprint x [{:.1}, {:.1}] y [{:.1}, {:.1}] \
         z [{:.3}, {:.3}]\n\
         tool: Ø{TIP_DIAMETER_MM} tip / {TAPER_HALF_ANGLE_DEG}deg / \
         Ø{SHANK_DIAMETER_MM} shank — envelope r {:.3} mm, cusp r {:.3} mm\n\
         overlap_mm: {}\n\
         moves: {} total, {} cutting (FinishingCut|EntryPlunge)\n\
         band move counts: shallow {}, mid_steep {}, very_steep {}\n\
         OFF-FOOTPRINT cutting targets: {}\n\
         by intent: FinishingCut {finishing_cuts}, EntryPlunge {entry_plunges}\n\
         off-footprint LINKING targets (not asserted on): {}\n\
         worst distance past the footprint edge: {:.4} mm\n\
         violating X range: {x_lo:.3} .. {x_hi:.3} mm\n\
         violating Y range: {y_lo:.3} .. {y_hi:.3} mm\n\
         attribution by region span:\n{}",
        bbox.min.x,
        bbox.max.x,
        bbox.min.y,
        bbox.max.y,
        bbox.min.z,
        bbox.max.z,
        fixture_cutter().envelope_radius_mm(),
        fixture_cutter().cusp_radius_mm(),
        p.overlap_mm,
        p.total_moves,
        p.cutting.len(),
        p.shallow_moves,
        p.mid_steep_moves,
        p.very_steep_moves,
        p.violations.len(),
        p.linking_violations,
        p.worst_past_edge_mm(bbox),
        p.attribution_table(),
    );
    let summary_path = artifact_dir().join("d16_1_runoff_summary.txt");
    std::fs::write(&summary_path, summary.as_bytes()).expect("write run-off summary");
    written.push(summary_path);

    for path in &written {
        let meta = std::fs::metadata(path)
            .unwrap_or_else(|e| panic!("artifact {} was not written: {e}", path.display()));
        let bytes = meta.len();
        assert!(
            bytes > 0,
            "artifact {} is empty — a render nobody can read is not a render",
            path.display()
        );
    }
}

// ── Render plumbing ──────────────────────────────────────────────────────

fn artifact_dir() -> PathBuf {
    let dir = common::repo_root().join("planning/review_2026-08-04/artifacts/w8");
    std::fs::create_dir_all(&dir).expect("create the W8 artifact directory");
    dir
}

/// Write one top-down greyscale PGM of `all` (dim) with `violations`
/// (bright) over the footprint rectangle (mid-grey outline).
///
/// `window` is `(x0, x1, y0, y1)` in world mm; `mm_per_px` sets the
/// resolution. Y increases UPWARD in the image, so a reader looking at an end
/// strip sees the run-off above the footprint edge, where they expect it.
fn render_pgm(
    name: &str,
    window: (f64, f64, f64, f64),
    mm_per_px: f64,
    bbox: &BoundingBox3,
    all: &[Target],
    violations: &[Target],
) -> PathBuf {
    let (x0, x1, y0, y1) = window;
    let cols = (((x1 - x0) / mm_per_px).ceil() as usize).max(1);
    let rows = (((y1 - y0) / mm_per_px).ceil() as usize).max(1);
    let mut px = vec![0u8; rows * cols];

    {
        let mut put = |x: f64, y: f64, v: u8| {
            if !(x0..x1).contains(&x) || !(y0..y1).contains(&y) {
                return;
            }
            let c = ((x - x0) / mm_per_px) as usize;
            let r = ((y - y0) / mm_per_px) as usize;
            if c >= cols || r >= rows {
                return;
            }
            let i = (rows - 1 - r) * cols + c;
            if px[i] < v {
                px[i] = v;
            }
        };

        // Footprint rectangle, so "outside" is legible without a legend.
        let trace = mm_per_px * 0.5;
        let mut x = bbox.min.x;
        while x <= bbox.max.x {
            put(x, bbox.min.y, 160);
            put(x, bbox.max.y, 160);
            x += trace;
        }
        let mut y = bbox.min.y;
        while y <= bbox.max.y {
            put(bbox.min.x, y, 160);
            put(bbox.max.x, y, 160);
            y += trace;
        }

        for t in all {
            put(t.x, t.y, 96);
        }
        for t in violations {
            put(t.x, t.y, 255);
        }
    }

    let path = artifact_dir().join(name);
    let mut f = std::fs::File::create(&path).expect("create render");
    write!(f, "P5\n{cols} {rows}\n255\n").expect("pgm header");
    f.write_all(&px).expect("pgm body");
    path
}
