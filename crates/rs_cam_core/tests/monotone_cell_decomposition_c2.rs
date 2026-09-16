//! C2 sentries — shallow-band monotone cell decomposition.
//!
//! Track C2 of `planning/thin_organic_2026-08-27/PROGRAMME.md`, whose spec is
//! `FINDINGS.md` §0k's verdict: **shared-lattice monotone decomposition + ONE
//! elongation-gated global PCA-minor rotation per region, raster everywhere.**
//!
//! These are synthetic and fast. They are NOT the acceptance evidence — that
//! is a real-fixture A/B through `relink_and_cost_under` in
//! `tests/thin_organic_island_widths.rs` plus the C4 rendered-surface review,
//! and it needs the operator's mesh, which is not in this repo. What lives
//! here is the set of properties that must hold on ANY input:
//!
//! 1. **X5 — dial off is inert.** The band emits the undivided raster, and
//!    the report records nothing (`None` = not measured).
//! 2. **Membership.** With the dial on, the union of the emitted cells
//!    selects exactly the lattice points the undivided region selects. Both
//!    arms sit on the same frame here, so this is an exact population test,
//!    not the coverage proxy §0j had to fall back on across lattices.
//! 3. **The gate.** A region below `ELONGATION_GATE` stays at 0°; one above
//!    it gets its PCA-minor axis — and never the dishonest 90°/180° that
//!    `batch_drop_cutter`'s axis-aligned fast path would silently return
//!    (`dropcutter.rs:118-165`).
//! 4. **Direction of effect.** A two-lobe region decomposes into more than
//!    one cell, its raw emission stops crossing the notch once per scan row,
//!    and the relinked cell arm is not worse than the relinked undivided
//!    arm. A DIRECTION assertion: the magnitudes (1.155× / 1.215×) are rig
//!    ceilings on one real relief under a machined-stock link ceiling, and
//!    nothing synthetic may be read as reproducing them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::finish_planner::FinishPlannerParams;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::monotone_cells::{
    ELONGATION_GATE, cells_select_same_lattice, honest_raster_direction_deg,
    lattice_monotone_cells, region_frame,
};
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CostingContext, CostingFeeds, relink_and_cost as metrology_relink_and_cost,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::surface::dropcutter::{DropCutterGrid, batch_drop_cutter};
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid};
use rs_cam_core::unified_finish::{UnifiedFinishParams, unified_finish_toolpath_with_cancel};

const STEPOVER_MM: f64 = 0.8;
const FEED_MM_MIN: f64 = 1000.0;
const PLUNGE_MM_MIN: f64 = 500.0;

// ── fixture ────────────────────────────────────────────────────────────
//
// A flat plateau at z = 0 whose FOOTPRINT is a "U": two legs joined along
// the base. Flat, so `finish_planner::decompose` puts all of it in the
// Shallow band, and the two-lobe topology comes from the footprint rather
// than from slope — which is what makes the cell rule's split event
// reachable without a relief mesh.
//
// Swept at 0° the scan rows run X. Rows below the notch floor cross ONE
// run; rows above it cross TWO. That single split is the canonical
// boustrophedon event, and it is the whole point of the fixture.

const U_X0: f64 = 0.0;
const U_X1: f64 = 40.0;
const U_Y0: f64 = 0.0;
const U_Y1: f64 = 30.0;
/// The notch: open at the top, so the legs stay joined only along the base.
const NOTCH_X0: f64 = 13.0;
const NOTCH_X1: f64 = 27.0;
const NOTCH_Y0: f64 = 10.0;

fn inside_u(x: f64, y: f64) -> bool {
    let in_outer = x > U_X0 && x < U_X1 && y > U_Y0 && y < U_Y1;
    let in_notch = x > NOTCH_X0 && x < NOTCH_X1 && y > NOTCH_Y0;
    in_outer && !in_notch
}

/// A flat plate covering exactly the U footprint, tessellated on a `cell`
/// grid. Quads whose centre falls outside the U are simply not emitted, so
/// the mesh has a hole and a drop-cutter grid over it reads the sentinel
/// floor there.
fn u_plateau_mesh(cell: f64) -> TriangleMesh {
    let nx = ((U_X1 - U_X0) / cell).round() as usize;
    let ny = ((U_Y1 - U_Y0) / cell).round() as usize;
    let stride = nx + 1;
    let mut verts = Vec::with_capacity(stride * (ny + 1));
    for iy in 0..=ny {
        for ix in 0..=nx {
            verts.push(P3::new(
                U_X0 + ix as f64 * cell,
                U_Y0 + iy as f64 * cell,
                0.0,
            ));
        }
    }
    let mut tris = Vec::new();
    for iy in 0..ny {
        for ix in 0..nx {
            let cx = U_X0 + (ix as f64 + 0.5) * cell;
            let cy = U_Y0 + (iy as f64 + 0.5) * cell;
            if !inside_u(cx, cy) {
                continue;
            }
            let a = (iy * stride + ix) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])
}

/// The bounding rectangle of the U, generously padded. The GRID (through
/// `min_z`) is what carves the U out of it, exactly as production does: a
/// lattice point belongs to the region only if the raster would emit it.
fn u_bounding_region() -> Polygon2 {
    rect(U_X0 - 5.0, U_Y0 - 5.0, U_X1 + 5.0, U_Y1 + 5.0)
}

struct Fixture {
    mesh: TriangleMesh,
    index: SpatialIndex,
    cutter: BallEndmill,
}

fn fixture() -> Fixture {
    let mesh = u_plateau_mesh(1.0);
    let index = SpatialIndex::build_auto(&mesh);
    Fixture {
        mesh,
        index,
        cutter: BallEndmill::new(2.0, 10.0),
    }
}

/// The production shallow lattice, rebuilt here from the public primitives.
///
/// The off-mesh guard is NOT optional and is the reason this helper exists
/// rather than a bare `batch_drop_cutter` call: `point_drop_cutter` marks a
/// point contacted whenever the cutter — which has radius — touches ANY
/// nearby triangle, including the rim of the notch this fixture cuts out of
/// the plate. Production drops those points
/// (`unified_finish::build_shallow_raster_grid`, replicated from
/// `compute::execute::generate_drop_cutter`); a reference that skipped the
/// guard would carry a fringe of phantom lattice points around the notch and
/// make the X5 comparison fail for a reason that has nothing to do with C2.
fn zero_degree_grid(f: &Fixture, min_z: f64) -> DropCutterGrid {
    let mut grid = batch_drop_cutter(&f.mesh, &f.index, &f.cutter, STEPOVER_MM, 0.0, min_z);
    for pt in &mut grid.points {
        let over = f
            .index
            .query(pt.x, pt.y, 0.0)
            .iter()
            .any(|&i| f.mesh.faces[i].contains_point_xy(pt.x, pt.y));
        if !over {
            pt.z = min_z;
            pt.contacted = false;
        }
    }
    grid
}

fn raster_over(grid: &DropCutterGrid, polygons: &[Polygon2], min_z: f64, safe_z: f64) -> Toolpath {
    let mut out = Toolpath::new();
    for polygon in polygons {
        let region = RegionSet::new(vec![polygon.clone()]);
        let tp = raster_toolpath_from_grid(
            grid,
            FEED_MM_MIN,
            PLUNGE_MM_MIN,
            safe_z,
            Some(min_z),
            Some(&region),
        );
        out.moves.extend(tp.moves);
    }
    out
}

/// Relink both arms with the SAME production parameters before comparing —
/// the E3 lesson (`PROGRAMME.md` Track E): an unfair comparison is worse
/// than none. Returns `(kept retracts, rapid mm)`.
/// PROMOTED (Track M, 2026-09-02): the relink+measure kernel is
/// `rs_cam_core::metrology::costing::relink_and_cost`. This file's copy was
/// the ONE genuine divergence among the six: it passed
/// `link_kinematics: None`, so the relink keeps any gouge-safe link instead
/// of costing each link against the retract it replaces. That behavior is
/// preserved as the explicit `kinematics: None` arm of `CostingContext`
/// (`time_s` then reads `NaN` — not measured).
fn relink_and_measure(
    f: &Fixture,
    raw: Toolpath,
    boundary: &Polygon2,
    safe_z: f64,
) -> (usize, f64) {
    let region = RegionSet::new(vec![boundary.clone()]);
    let ctx = CostingContext {
        mesh: &f.mesh,
        index: &f.index,
        cutter: &f.cutter,
        kinematics: None,
        feeds: CostingFeeds {
            feed_mm_min: FEED_MM_MIN,
            plunge_mm_min: PLUNGE_MM_MIN,
            // Inert under `kinematics: None`: no link costing, no
            // integrator. This file never had these pins.
            max_feed_mm_min: FEED_MM_MIN,
            rapid_feed_mm_min: FEED_MM_MIN,
        },
    };
    let cost = metrology_relink_and_cost(&ctx, raw, &region, safe_z);
    (cost.kept_retracts, cost.rapid_mm)
}

/// Every distinct XY the toolpath visits, quantised so float formatting
/// cannot make two identical positions look different.
fn visited_xy(tp: &Toolpath) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = tp
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn params_with(dial: bool) -> UnifiedFinishParams {
    UnifiedFinishParams {
        raster_stepover: STEPOVER_MM,
        tolerance: 0.5,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z: 10.0,
        // Relink OFF so the two arms are compared on their EMISSION. With
        // it on, a link's interpolated points would enter `visited_xy` and
        // the membership assertion would be about the relinker instead.
        intra_region_hookup_mm: 0.0,
        monotone_cell_decomposition: dial,
        ..UnifiedFinishParams::default()
    }
}

type UnifiedRun = (Toolpath, rs_cam_core::unified_finish::UnifiedFinishReport);

fn run_unified(f: &Fixture, dial: bool) -> UnifiedRun {
    let params = params_with(dial);
    let planner = FinishPlannerParams::for_tool(1.0);
    let never_cancel = || false;
    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &f.mesh,
        &f.index,
        &f.cutter,
        5.0,
        -5.0,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .expect("the flat U plateau must generate");
    (tp, report)
}

// ── 1. X5: the dial ships inert ────────────────────────────────────────

/// With the dial off the band must emit the undivided raster and MEASURE
/// NOTHING.
///
/// The `None` half is as load-bearing as the geometry half: `Some(default)`
/// there would claim a decomposition ran and was clean, which is exactly the
/// reading the `ToolpathStats` finding contract forbids.
#[test]
fn dial_off_emits_the_undivided_raster_and_records_nothing() {
    let f = fixture();
    let (tp_off, report_off) = run_unified(&f, false);

    assert!(
        report_off.monotone_cells.is_none(),
        "dial off must measure nothing, got {:?}",
        report_off.monotone_cells
    );
    assert!(
        !tp_off.moves.is_empty(),
        "the fixture must actually cut, or every assertion below is vacuous"
    );

    // The pre-C2 band's lattice, rebuilt from the same public primitives:
    // the shared 0° drop-cutter grid with the off-mesh guard, rastered over
    // the region.
    //
    // **What this pins, exactly.** The orchestrator clips to
    // `finish_planner::decompose`'s CONDITIONED region polygons, which are a
    // morphological close/min-area/overlap treatment of the shallow mask —
    // not this fixture's bare footprint — so the honest relation is
    // CONTAINMENT, not equality: every point the band visits must be a point
    // of the shared 0° lattice. That is the assertion with teeth. A band
    // that silently rotated its frame, moved its lattice origin or changed
    // its stepover would put visited points off this lattice entirely and
    // fail on the first one, which is precisely the regression the C2 edit
    // could introduce on the dial-OFF path.
    //
    // The EXACT emission comparison lives in
    // `dial_on_covers_the_same_lattice_as_dial_off`, where both arms are the
    // same orchestrator on the same conditioned regions and equality is
    // therefore meaningful.
    let min_z = f.mesh.bbox.min.z - 0.1;
    let grid = zero_degree_grid(&f, min_z);
    let reference = raster_over(&grid, &[u_bounding_region()], min_z, 10.0);
    let lattice = visited_xy(&reference);
    let visited = visited_xy(&tp_off);
    assert!(
        !lattice.is_empty(),
        "the reference raster must cut, or the containment below is vacuous"
    );
    let strays: Vec<(i64, i64)> = visited
        .iter()
        .filter(|xy| lattice.binary_search(xy).is_err())
        .copied()
        .collect();
    assert!(
        strays.is_empty(),
        "dial off must stay on the shared 0° lattice; {} stray position(s), \
         first {:?}",
        strays.len(),
        strays.first()
    );
    assert!(
        visited.len() * 2 > lattice.len(),
        "dial off must cover most of the lattice — {} of {} suggests the band \
         clipped to something unexpected rather than merely being conditioned",
        visited.len(),
        lattice.len()
    );
}

/// Determinism: the same input twice is the same output. A decomposition
/// that depended on iteration order would show up here before it showed up
/// as an unreproducible A/B.
#[test]
fn the_dial_on_arm_is_deterministic() {
    let f = fixture();
    let (a, _) = run_unified(&f, true);
    let (b, _) = run_unified(&f, true);
    assert_eq!(a.moves.len(), b.moves.len());
    assert_eq!(visited_xy(&a), visited_xy(&b));
}

// ── 2. membership ──────────────────────────────────────────────────────

/// The union of the emitted cells must select exactly the lattice
/// population the undivided region selects.
///
/// Both arms are on the SAME frame here (the U's elongation is below the
/// gate, so neither rotates), which is what makes this an exact test rather
/// than §0j's cross-lattice coverage proxy. A cell set that lost a lattice
/// point would be uncut material.
#[test]
fn dial_on_covers_the_same_lattice_as_dial_off() {
    let f = fixture();
    let (tp_off, _) = run_unified(&f, false);
    let (tp_on, report_on) = run_unified(&f, true);

    let totals = report_on
        .monotone_cells
        .expect("dial on over a shallow region must MEASURE the decomposition");
    assert!(totals.regions >= 1, "{totals:?}");
    // The fixture is deliberately BELOW the elongation gate (40 × 30), so
    // both arms share the 0° frame and the equality below is exact rather
    // than a proxy. A failure here is a statement about the fixture — the
    // conditioned regions are not the footprint this test assumed — and the
    // equality assertion under it would then be meaningless, not merely red.
    assert_eq!(
        totals.regions_rotated, 0,
        "the U fixture must not clear the gate: {totals:?}"
    );
    assert_eq!(
        totals.membership_fallbacks, 0,
        "the decomposition must reproduce the region's own lattice: {totals:?}"
    );
    assert_eq!(totals.empty_fallbacks, 0, "{totals:?}");
    assert!(
        totals.cells_emitted >= totals.regions,
        "every decomposed region must emit at least one cell: {totals:?}"
    );

    assert_eq!(
        visited_xy(&tp_on),
        visited_xy(&tp_off),
        "cell-segmenting the raster must not add or drop a single lattice point"
    );
}

// ── 3. the gate, and the 89.9°-not-90° trap ────────────────────────────

/// A region below the elongation gate keeps the 0° frame. The U is 40 × 30
/// — it has no credible axis, and §0f refused to give one to a region like
/// it.
#[test]
fn a_region_below_the_gate_stays_at_zero_degrees() {
    let frame = region_frame(&rect(U_X0, U_Y0, U_X1, U_Y1), STEPOVER_MM);
    assert!(!frame.rotated, "{frame:?}");
    assert!((frame.direction_deg - 0.0).abs() < 1e-12, "{frame:?}");
    assert!(frame.elongation.unwrap() < ELONGATION_GATE, "{frame:?}");
}

/// A region above the gate gets its PCA-MINOR axis — passes run ACROSS the
/// narrow dimension — and the returned angle is one `batch_drop_cutter`
/// will honour.
///
/// The 60 × 6 case is deliberately the trap case: its minor axis is exactly
/// 90°, which `batch_drop_cutter` answers with an AXIS-ALIGNED grid still
/// labelled 90° (`dropcutter.rs:118-165`). A frame that returned a literal
/// 90 would silently sweep at 0° while every downstream frame mapping
/// believed it had rotated a right angle.
#[test]
fn a_gate_passing_region_gets_its_pca_minor_axis_and_never_a_literal_ninety() {
    let frame = region_frame(&rect(0.0, 0.0, 60.0, 6.0), STEPOVER_MM);
    assert!(frame.rotated, "10:1 must clear the gate: {frame:?}");
    assert!(frame.elongation.unwrap() > ELONGATION_GATE, "{frame:?}");
    assert!(
        (frame.direction_deg - 89.9).abs() < 1e-9,
        "the minor axis is 90°, which must be nudged off the dishonest fast \
         path, got {frame:?}"
    );
    assert!(
        (frame.direction_deg - 90.0).abs() > 0.01,
        "0.01° is the fast path's own snap window (dropcutter.rs:118-165); a \
         nudge inside it buys nothing: {frame:?}"
    );
}

/// An oblique axis passes through untouched, folded into `[0, 180)`. The
/// nudge must be surgical — it exists for two angles, not as a general
/// perturbation of the operator's geometry.
#[test]
fn an_oblique_axis_is_not_perturbed() {
    // A 60 × 6 rectangle rotated 30°: major axis 30°, minor 120°.
    let angle = 30.0_f64.to_radians();
    let (c, s) = (angle.cos(), angle.sin());
    let corners = [(0.0, 0.0), (60.0, 0.0), (60.0, 6.0), (0.0, 6.0)];
    let poly = Polygon2::new(
        corners
            .iter()
            .map(|&(x, y)| P2::new(x * c - y * s, x * s + y * c))
            .collect(),
    );
    let frame = region_frame(&poly, STEPOVER_MM);
    assert!(frame.rotated, "{frame:?}");
    assert!(
        (frame.direction_deg - 120.0).abs() < 1.0,
        "expected the PCA-minor axis near 120°, got {frame:?}"
    );
    assert!(
        (honest_raster_direction_deg(frame.direction_deg) - frame.direction_deg).abs() < 1e-12,
        "an oblique angle must be its own honest form"
    );
}

// ── 4. direction of effect ─────────────────────────────────────────────

/// The two-lobe fixture must decompose into more than one cell, its raw
/// emission must stop crossing the notch once per scan row, and the relinked
/// cell arm must not be worse than the relinked undivided arm.
///
/// **A direction assertion, deliberately.** The measured magnitudes
/// (1.155× top-three, 1.215× on the gate-passing region — `FINDINGS.md`
/// §0i) are rig figures on one real relief under a machined-stock link
/// ceiling. Nothing about a 40 × 30 synthetic plateau reproduces them, and a
/// sentry that pretended otherwise would be pinning noise. What must hold on
/// any input is the SIGN: cutting the cross-notch traverses out cannot make
/// the path longer.
#[test]
fn a_two_lobe_region_decomposes_and_the_relinked_cells_do_not_travel_further() {
    let f = fixture();
    let min_z = f.mesh.bbox.min.z - 0.1;
    let safe_z = 10.0;
    let grid = zero_degree_grid(&f, min_z);
    let boundary = u_bounding_region();

    let decomposed = lattice_monotone_cells(&grid, &boundary, min_z);
    assert!(
        decomposed.topology_cells > 1,
        "the U's notch must split the sweep into more than one cell, got {}",
        decomposed.topology_cells
    );
    assert!(!decomposed.cells.is_empty());
    assert_eq!(
        cells_select_same_lattice(&grid, &boundary, &decomposed.cells, min_z),
        0,
        "the cells must select exactly the undivided lattice population"
    );

    let base_raw = raster_over(&grid, std::slice::from_ref(&boundary), min_z, safe_z);
    let cell_raw = raster_over(&grid, &decomposed.cells, min_z, safe_z);
    // Coverage equality is already pinned above, by the lattice-population
    // check — deliberately NOT by comparing cutting distance, which counts
    // the per-run plunge/retract legs and therefore moves with the FRAGMENT
    // count rather than with what was cut.
    // The EMISSION change is what C2 owns, and it is strict by
    // construction: a row-major raster over the whole U crosses the notch
    // once per scan row above it; a cell-major one crosses it about twice in
    // total.
    let base_raw_rapid = base_raw.total_rapid_distance();
    let cell_raw_rapid = cell_raw.total_rapid_distance();
    assert!(
        cell_raw_rapid < base_raw_rapid,
        "cell-major emission must not cross the notch once per row: raw rapids \
         {base_raw_rapid:.1} -> {cell_raw_rapid:.1} mm"
    );

    // After the relink the two arms may CONVERGE, and that is a real result
    // rather than a failure of the candidate: `reorder: true` is greedy
    // nearest-first, and on a topology this clean it rediscovers the cell
    // structure from the undivided arm by itself. §0g measured the same
    // thing from the other side — "the relinker had already eliminated most
    // of the original junctions" — and the wanaka win survived only because
    // a dendritic island defeats greedy nearest-first. So the relinked
    // assertion is NOT-WORSE, not better; the acceptance evidence for
    // "better" is the real-fixture A/B (see the module doc), never this.
    let (base_retracts, base_rapid_mm) = relink_and_measure(&f, base_raw, &boundary, safe_z);
    let (cell_retracts, cell_rapid_mm) = relink_and_measure(&f, cell_raw, &boundary, safe_z);
    assert!(
        cell_retracts <= base_retracts || cell_rapid_mm <= base_rapid_mm,
        "the cell arm must not be worse on BOTH channels: retracts {base_retracts} \
         -> {cell_retracts}, rapids {base_rapid_mm:.1} -> {cell_rapid_mm:.1} mm"
    );
}
