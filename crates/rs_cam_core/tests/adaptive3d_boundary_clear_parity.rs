//! Boundary-clear planner↔simulator parity — the THIRD arm of the adaptive3d
//! parity family.
//!
//! ## What this arm exists for
//!
//! The two shipped arms (`adaptive3d_planner_sim_dexel_parity.rs` and the
//! in-lib `run_planner_sim_parity_with_mesh` pair) both run with
//! `Adaptive3dParams::boundary == None`. Setting a `ToolContainment::Inside`
//! containment boundary — a shipped, GUI-reachable configuration — is
//! un-measured by every test in the suite. `DELTA_w5b_f1_planner_boundary.md`
//! §7(a) flagged it as OPEN ITEM 1; `RESEARCH_boundary_clear_parity.md`
//! measured it on 2026-08-21 and this file is that measurement, pinned.
//!
//! ## The two mechanisms it pins
//!
//! 1. **The pre-clear polygon and the clip polygon differ by one tool
//!    radius.** `session::compute::resolve_containment_polygon` hands
//!    adaptive3d the *un-inset* containment as `params.boundary`, and
//!    `path.rs:408-436` clears the planner's internal stock outside it. The
//!    post-generation clip (`session::compute::apply_boundary_clip`) instead
//!    gates cutter **centres** against `containment ⊖ r`. The planner
//!    therefore places its outermost ring's centres in the band between the
//!    two polygons, the clip turns those cutting moves into rapids, and the
//!    material stays standing while the planner's own dexel records it as
//!    removed. Measured here: a ~2 r wall of in-scope, tool-reachable, uncut
//!    material along every containment edge (**bar B1**).
//! 2. **`waterline_cleanup` is boundary-unaware.** `adaptive3d/clearing.rs`
//!    takes no `boundary` parameter at all, so it traces mesh contours across
//!    the full silhouette regardless of containment. Its segments are the
//!    emitted cutting moves that land fully *outside* the containment — the
//!    one emitter the `params.boundary` pre-clear does not reach, i.e. O5b
//!    surviving in a corner (**bar B2**).
//!
//! ## Severity, so nobody over-reads a red here
//!
//! Traced in the research doc §6: `final_material_stock` (the planner claim
//! grid) has **no consumer outside `adaptive3d`** — rest machining runs
//! sim→planner, never planner→sim, so a boundary-clipped rough does not make
//! a downstream rest op skip the standing wall. Safety is LOW (the divergence
//! is one-sided in the benign direction: material left standing, not extra
//! material removed). Machining quality is MEDIUM. What this sentry primarily
//! buys is that the blind spot stops being blind.
//!
//! ## The bars
//!
//! | # | bar | measured 2026-08-21 (CP / AS) | pinned as |
//! |---|---|---|---|
//! | P1 | pre-clear zone > 0 **and** `effective_boundary` returned exactly 1 polygon | 4005 cells, 1 poly | asserted |
//! | P2 | F-027 border-clear zone == 0 | 0 | `assert_eq!` |
//! | B1 | in-scope over-claim, as a fraction of the in-containment population | 27.7% / 27.1% | ≤ 30.0% growth pin |
//! | B2 | emitted cutting-move endpoints fully outside the containment | 138 / 138 | exact `assert_eq!` |
//! | B3 | outside-containment `sim_higher` | 4005 / 4005 | **counted, deliberately NOT gated** |
//! | B4 | `planner_higher` inside the containment (benign direction) | 9 / 38 | ≤ 60 loose pin |
//!
//! B1 is a *percentage*, not zero, because the defect is real and open: the
//! fix is decision **D4** (inset the pre-clear to `containment ⊖ r`, and give
//! `waterline_cleanup` a boundary parameter), deferred out of this wave. The
//! `#[ignore]`d arm at the bottom of this file carries that fix contract, so
//! the day D4 lands the bar is un-ignored rather than re-derived.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
    adaptive_3d_toolpath_with_cancel, debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::boundary::{
    ToolContainment, clip_toolpath_to_boundary, effective_boundary_reported,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::MoveType;

const TOOL_RADIUS: f64 = 3.175;
const STOCK_TOP_Z: f64 = 25.0;
const SAFE_Z: f64 = 30.0;

/// B1 — in-scope over-claim ceiling, as a fraction of the in-containment cell
/// population. Measured 27.7% (ContourParallel) / 27.1% (AgentSearch).
const MAX_IN_SCOPE_OVER_CLAIM_FRACTION: f64 = 0.30;
/// B2 — emitted cutting-move endpoints fully outside the containment.
/// Deterministic and strategy-independent: `waterline_cleanup` traces the same
/// mesh contours whichever clearing strategy ran.
const WATERLINE_LEAK_CUT_MOVES: usize = 138;
/// B4 — `planner_higher` inside the containment. The benign direction; a loose
/// growth pin only (measured 9 / 38).
const MAX_IN_SCOPE_PLANNER_HIGHER: u64 = 60;

fn hemisphere() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(20.0, 16);
    let si = SpatialIndex::build(&mesh, 10.0);
    (mesh, si)
}

/// The left half of the stock footprint in X. Chosen so ~half the planner grid
/// falls outside the containment and the wall runs down the middle of the mesh.
fn half_stock_boundary() -> Polygon2 {
    Polygon2::rectangle(-23.5, -23.5, 0.0, 23.5)
}

fn base_params(strategy: ClearingStrategy3d) -> Adaptive3dParams {
    Adaptive3dParams {
        trochoid_cap_mult: 1.6,
        tool_radius: TOOL_RADIUS,
        envelope_radius: TOOL_RADIUS,
        z_floor: None,
        stepover: 1.0,
        depth_per_pass: 3.0,
        stock_to_leave: 0.5,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        tolerance: 0.5,
        min_cutting_radius: 0.0,
        stock_top_z: STOCK_TOP_Z,
        entry_style: EntryStyle3d::Plunge,
        fine_stepdown: None,
        detect_flat_areas: false,
        max_stay_down_dist: None,
        region_ordering: RegionOrdering::Global,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        initial_stock: None,
        clearing_strategy: strategy,
        z_blend: false,
        boundary: None,
        mill_shallow_areas: false,
        shallow_angle_rad: None,
        shallow_stepdown: None,
        world_stock_xy_bbox: None,
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    }
}

/// The world stock footprint declared to the planner. Deliberately **wider
/// than the grid**: `TriDexelStock::from_stock` rounds the grid extent UP, so
/// declaring the nominal stock rect leaves the last row/column outside it and
/// re-admits 177 border-cleared cells plus 21 spurious `sim_higher`. Production
/// is not exposed (`execute.rs` declares the auto-grown stock bbox and the
/// simulator's grid is bounded by the same box), but a fixture must declare it
/// wide or bar P2 stops being about the boundary clear.
const WORLD_XY: (f64, f64, f64, f64) = (-1.0e4, -1.0e4, 1.0e4, 1.0e4);

/// Planner tops, normalised the way the in-lib `stock_top_z_at` does: an empty
/// ray reads the grid floor, not `-inf`.
struct PlannerGrid {
    tops: Vec<f64>,
    rows: usize,
    cols: usize,
    cell_size: f64,
    origin_u: f64,
    origin_v: f64,
}

impl PlannerGrid {
    fn cell_to_world(&self, row: usize, col: usize) -> (f64, f64) {
        (
            self.origin_u + col as f64 * self.cell_size,
            self.origin_v + row as f64 * self.cell_size,
        )
    }
    fn at(&self, row: usize, col: usize) -> f64 {
        self.tops[row * self.cols + col]
    }
}

fn sim_top(stock: &TriDexelStock, row: usize, col: usize) -> f64 {
    let ray = stock.z_grid.ray(row, col);
    let top = ray.iter().map(|s| s.exit).fold(f32::NEG_INFINITY, f32::max);
    if top.is_finite() {
        top as f64
    } else {
        stock.stock_bbox.min.z
    }
}

#[derive(Default, Clone, Copy)]
struct ZoneTally {
    total: u64,
    agree: u64,
    planner_higher: u64,
    sim_higher: u64,
    max_dz: f64,
}

impl ZoneTally {
    fn row(&self, name: &str) -> String {
        format!(
            "| {name:<28} | {:>6} | {:>6} | {:>6} | {:>6} | {:>7.3} | {:>6.1}% |",
            self.total,
            self.agree,
            self.planner_higher,
            self.sim_higher,
            self.max_dz,
            self.sim_higher as f64 / self.total.max(1) as f64 * 100.0,
        )
    }
}

struct Measured {
    label: String,
    /// Cells inside `containment ⊖ r` — a cutter centre is allowed here.
    inside_eff: ZoneTally,
    /// Cells inside the containment but outside `containment ⊖ r`. Material
    /// here is tool-reachable (a centre on the `⊖ r` ring sweeps out to the
    /// containment edge) but a centre here is not allowed.
    band: ZoneTally,
    /// Cells outside the containment — the pre-clear's zone. Unreachable by a
    /// compliant tool. Bar B3: counted, never gated.
    outside: ZoneTally,
    whole: ZoneTally,
    grid_cells: u64,
    /// P1 — cells the boundary pre-clear covers.
    preclear_zone: u64,
    /// P2 — cells the F-027 border clear covers, reproducing `path.rs:344-373`'s
    /// predicate exactly.
    border_clear_zone: u64,
    /// P1 — polygon count from `effective_boundary_reported`. Zero means the
    /// offset collapsed and the path ships UNCLIPPED (the `boundary_clip_dropped`
    /// escape hatch), which would make this whole fixture measure something else.
    effective_polys: usize,
    /// B2 — pre-clip emitted cutting-move endpoints fully outside the containment.
    cut_moves_outside_containment: usize,
    /// Pre-clip emitted cutting-move endpoints in the reachable band. The clip
    /// converts every one of these into a rapid.
    cut_moves_in_band: usize,
    cut_moves_inside_effective: usize,
    /// Post-clip move counts, for the report line only.
    sim_moves: usize,
    sim_cut_moves: usize,
}

impl Measured {
    /// B1 numerator — `sim_higher` cells *inside* the containment. Both the
    /// effective interior and the reachable band count: material in the band is
    /// reachable by a compliant cutter whose centre sits on the `⊖ r` ring.
    fn in_scope_over_claim(&self) -> u64 {
        self.inside_eff.sim_higher + self.band.sim_higher
    }
    /// B1 denominator — the in-containment cell population.
    fn in_containment_population(&self) -> u64 {
        self.inside_eff.total + self.band.total
    }
    fn in_scope_over_claim_fraction(&self) -> f64 {
        self.in_scope_over_claim() as f64 / self.in_containment_population().max(1) as f64
    }
    fn in_scope_planner_higher(&self) -> u64 {
        self.inside_eff.planner_higher + self.band.planner_higher
    }
}

/// One planner + simulator run with a containment boundary set, mirroring
/// production's two boundary steps: the planner's internal pre-clear against
/// the **un-inset** containment (`path.rs:408-436`, fed by
/// `session::compute::resolve_containment_polygon`), and the post-generation
/// clip against `containment ⊖ r` (`session::compute::apply_boundary_clip`).
///
/// Planner state comes from the public `debug_adaptive_3d_segments_for_f029_probe`,
/// which re-runs `adaptive_3d_segments` with the same params — deterministic, so
/// it is the same planner state the emitted toolpath was built from.
fn measure(label: &str, strategy: ClearingStrategy3d, boundary: Option<&Polygon2>) -> Measured {
    let (mesh, si) = hemisphere();
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);
    let bbox = mesh.bbox;
    let r = TOOL_RADIUS;
    let cell_size = (TOOL_RADIUS / 6.0).max(0.1);

    let initial_stock = TriDexelStock::from_stock(
        bbox.min.x - r,
        bbox.min.y - r,
        bbox.max.x + r,
        bbox.max.y + r,
        bbox.min.z,
        STOCK_TOP_Z,
        cell_size,
    );

    let params = Adaptive3dParams {
        initial_stock: Some(initial_stock.clone()),
        world_stock_xy_bbox: Some(WORLD_XY),
        boundary: boundary.cloned(),
        ..base_params(strategy)
    };

    let never_cancel = || false;
    let (tops_raw, rows, cols, pg_cell, pg_ou, pg_ov, pg_zmin, _pg_zmax) =
        debug_adaptive_3d_segments_for_f029_probe(&mesh, &si, &cutter, &params, &never_cancel)
            .expect("planner probe should succeed");
    let planner = PlannerGrid {
        tops: tops_raw
            .iter()
            .map(|&z| if z.is_finite() { z as f64 } else { pg_zmin })
            .collect(),
        rows,
        cols,
        cell_size: pg_cell,
        origin_u: pg_ou,
        origin_v: pg_ov,
    };

    let toolpath = adaptive_3d_toolpath_with_cancel(&mesh, &si, &cutter, &params, &never_cancel)
        .expect("toolpath should succeed");

    let (effective, offset_failure) = match boundary {
        Some(b) => effective_boundary_reported(b, ToolContainment::Inside, r),
        None => (Vec::new(), None),
    };
    if boundary.is_some() {
        eprintln!(
            "[{label}] effective_boundary: {} polygon(s), offset_failure {:?} \
             (empty => COLLAPSE escape hatch: the path ships UNCLIPPED with only the \
             report-only boundary_clip_dropped finding, and this fixture would be \
             measuring the wrong thing — bar P1 refuses that)",
            effective.len(),
            offset_failure.as_ref().map(|f| f.describe()),
        );
    }

    let sim_path = if effective.is_empty() {
        toolpath.clone()
    } else {
        clip_toolpath_to_boundary(&toolpath, &effective[0], SAFE_Z)
    };

    let mut sim_stock = initial_stock;
    sim_stock
        .simulate_toolpath_with_metrics_with_cancel(
            &sim_path,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulator should succeed");

    let tol = cell_size;
    let border_margin = r * 0.5;
    let mut inside_eff = ZoneTally::default();
    let mut band = ZoneTally::default();
    let mut outside = ZoneTally::default();
    let mut whole = ZoneTally::default();
    let mut preclear_zone = 0u64;
    let mut border_clear_zone = 0u64;

    for row in 0..planner.rows {
        for col in 0..planner.cols {
            let (x, y) = planner.cell_to_world(row, col);
            let p = planner.at(row, col);
            let s = sim_top(&sim_stock, row, col);
            let dz = (p - s).abs();

            // P2: reproduce `path.rs:344-373`'s border-clear predicate exactly
            // — outside the mesh bbox by more than `r/2`, AND not inside the
            // declared world stock bbox.
            let outside_mesh = x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin;
            let (wx_min, wy_min, wx_max, wy_max) = WORLD_XY;
            let inside_world = x >= wx_min && x <= wx_max && y >= wy_min && y <= wy_max;
            if outside_mesh && !inside_world {
                border_clear_zone += 1;
            }

            let in_boundary = boundary.is_none_or(|b| b.contains_point(&P2::new(x, y)));
            let in_effective = effective.iter().any(|e| e.contains_point(&P2::new(x, y)));
            if !in_boundary {
                preclear_zone += 1;
            }

            let zone: &mut ZoneTally = if !in_boundary {
                &mut outside
            } else if in_effective {
                &mut inside_eff
            } else {
                &mut band
            };
            let bump = |z: &mut ZoneTally| {
                z.total += 1;
                z.max_dz = z.max_dz.max(dz);
                if dz <= tol {
                    z.agree += 1;
                } else if p > s + tol {
                    z.planner_higher += 1;
                } else {
                    z.sim_higher += 1;
                }
            };
            bump(zone);
            bump(&mut whole);
        }
    }

    // Move-level probe on the PRE-CLIP toolpath: where do the emitted cutting
    // moves actually put the cutter centre, relative to the containment and to
    // `containment ⊖ r`?
    let mut cut_moves_outside_containment = 0usize;
    let mut cut_moves_in_band = 0usize;
    let mut cut_moves_inside_effective = 0usize;
    let mut max_cut_endpoint_x = f64::NEG_INFINITY;
    let mut leak_r_min = f64::INFINITY;
    let mut leak_r_max = f64::NEG_INFINITY;
    if let Some(b) = boundary {
        for m in &toolpath.moves {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let p = P2::new(m.target.x, m.target.y);
            max_cut_endpoint_x = max_cut_endpoint_x.max(m.target.x);
            if !b.contains_point(&p) {
                cut_moves_outside_containment += 1;
                let radius = (p.x * p.x + p.y * p.y).sqrt();
                leak_r_min = leak_r_min.min(radius);
                leak_r_max = leak_r_max.max(radius);
            } else if effective.iter().any(|e| e.contains_point(&p)) {
                cut_moves_inside_effective += 1;
            } else {
                cut_moves_in_band += 1;
            }
        }
        eprintln!(
            "[{label}] EMITTED (pre-clip) cutting endpoints: {cut_moves_inside_effective} inside \
             containment⊖r, {cut_moves_in_band} in the reachable band (the clip rapids every one \
             of these), {cut_moves_outside_containment} fully OUTSIDE the containment \
             (waterline_cleanup leak, radius {leak_r_min:.3}..{leak_r_max:.3} mm vs mesh radius \
             20.0 + tool radius {r:.3}). max cut-endpoint x = {max_cut_endpoint_x:.3}, \
             containment edge x = 0.0."
        );
    }

    let sim_cut_moves = sim_path
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .count();

    Measured {
        label: label.to_owned(),
        inside_eff,
        band,
        outside,
        whole,
        grid_cells: (planner.rows * planner.cols) as u64,
        preclear_zone,
        border_clear_zone,
        effective_polys: effective.len(),
        cut_moves_outside_containment,
        cut_moves_in_band,
        cut_moves_inside_effective,
        sim_moves: sim_path.moves.len(),
        sim_cut_moves,
    }
}

fn report(m: &Measured) {
    eprintln!("\n=== {} ===", m.label);
    eprintln!(
        "grid {} cells; boundary pre-clear zone {} cells ({:.1}%); F-027 border-clear zone {} \
         cells; simulated moves {} ({} cutting)",
        m.grid_cells,
        m.preclear_zone,
        m.preclear_zone as f64 / m.grid_cells.max(1) as f64 * 100.0,
        m.border_clear_zone,
        m.sim_moves,
        m.sim_cut_moves,
    );
    eprintln!(
        "| {:<28} | {:>6} | {:>6} | {:>6} | {:>6} | {:>7} | {:>7} |",
        "zone", "cells", "agree", "plan>", "sim>", "max dz", "sim>%"
    );
    eprintln!("{}", m.inside_eff.row("inside effective boundary"));
    eprintln!("{}", m.band.row("reachable band (b\\eff)"));
    eprintln!("{}", m.outside.row("OUTSIDE boundary (precleared)"));
    eprintln!("{}", m.whole.row("whole grid"));
    eprintln!(
        "B1 in-scope over-claim: {} of {} in-containment cells = {:.2}%",
        m.in_scope_over_claim(),
        m.in_containment_population(),
        m.in_scope_over_claim_fraction() * 100.0,
    );
}

/// P1 + P2 + B1 + B2 + B3 + B4. Shared by both strategy arms.
fn assert_containment_parity_bars(m: &Measured) {
    let label = &m.label;

    // --- P1: the fixture is not vacuous, and it is on the normal clip path.
    assert!(
        m.preclear_zone > 0,
        "[{label}] P1: the boundary pre-clear zone is EMPTY ({} cells outside the containment of \
         {} grid cells). Every bar below would then be measuring an unbounded run with a \
         boundary-shaped label. Expected ~4005 cells (50.6%).",
        m.preclear_zone,
        m.grid_cells,
    );
    assert_eq!(
        m.effective_polys, 1,
        "[{label}] P1: `effective_boundary_reported` must return exactly ONE polygon for this \
         rectangle. Zero means the offset COLLAPSED and production would ship the path UNCLIPPED \
         behind the report-only `boundary_clip_dropped` finding — a different configuration with \
         different (lower!) divergence, which this fixture must not silently slide onto. More \
         than one means the inset split and `clip_toolpath_to_boundary`'s single-polygon call \
         below is no longer the production analogue."
    );

    // --- P2: this arm measures the BOUNDARY clear, not W5B-F1's border clear.
    assert_eq!(
        m.border_clear_zone, 0,
        "[{label}] P2: the F-027 border-clear zone must be empty so `sim_higher` cannot be \
         attributed to it. `world_stock_xy_bbox` is declared far wider than the grid precisely \
         to inhibit it. A non-zero count means the fixture drifted and this arm has quietly \
         become a second copy of W5B-F1's border-clear arm."
    );

    // --- B1: the consequential term. In-scope, tool-reachable standing material.
    let frac = m.in_scope_over_claim_fraction();
    assert!(
        frac <= MAX_IN_SCOPE_OVER_CLAIM_FRACTION,
        "[{label}] B1: in-scope over-claim is {} of {} in-containment cells ({:.2}%) — the pin is \
         {:.1}%. These cells are INSIDE the containment, reachable by a compliant cutter, standing \
         up to {:.3} mm deep, and the planner's own dexel records them as removed. Measured \
         2026-08-21: 27.7% (ContourParallel) / 27.1% (AgentSearch). This is a GROWTH pin on a \
         known-open defect (decision D4), not a health bar — see the #[ignore]d arm below for the \
         fix contract.",
        m.in_scope_over_claim(),
        m.in_containment_population(),
        frac * 100.0,
        MAX_IN_SCOPE_OVER_CLAIM_FRACTION * 100.0,
        m.band.max_dz.max(m.inside_eff.max_dz),
    );

    // --- B2: the waterline leak. Exact, because a count over ~1500 moves is
    // sensitive to one move where a percentage is not.
    assert_eq!(
        m.cut_moves_outside_containment, WATERLINE_LEAK_CUT_MOVES,
        "[{label}] B2: {} emitted cutting-move endpoints land fully OUTSIDE the containment; the \
         pin is {WATERLINE_LEAK_CUT_MOVES}. These are `waterline_cleanup` contours \
         (`adaptive3d/clearing.rs`), which take no `boundary` parameter at all — the count is \
         identical on both clearing strategies for exactly that reason. This is an EXACT bar: \
         change it only with a measurement, and a DROP to 0 means the D4 fix (F-B: pass \
         `params.boundary` into `waterline_cleanup`) landed — un-ignore the arm below rather \
         than re-baselining here.",
        m.cut_moves_outside_containment,
    );

    // --- B3: deliberately NOT gated. Assert only that the population is
    // measured, so the exclusion is explicit rather than an accidental omission.
    assert_eq!(
        m.outside.total, m.preclear_zone,
        "[{label}] B3: every outside-containment cell must be tallied, so the decision NOT to \
         gate them is a stated exclusion and not a silently empty population."
    );
    assert!(
        m.outside.sim_higher > 0,
        "[{label}] B3: the outside-containment `sim_higher` population is empty. It is not gated \
         — those cells are unreachable by a compliant tool and the planner claim grid has no \
         consumer, so pinning them would repeat W5B-F1's mistake of gating a deliberate \
         pre-clear as if it were a defect — but it must still be COUNTED. An empty population \
         here means the pre-clear stopped engaging, which would silently deflate B1's denominator \
         story. Measured 2026-08-21: 4005 of 4005."
    );

    // --- B4: the benign direction, loose growth pin.
    assert!(
        m.in_scope_planner_higher() <= MAX_IN_SCOPE_PLANNER_HIGHER,
        "[{label}] B4: {} in-containment cells read `planner_higher` (the simulator removed more \
         than the planner claimed) — the loose pin is {MAX_IN_SCOPE_PLANNER_HIGHER}. Measured \
         2026-08-21: 9 (ContourParallel) / 38 (AgentSearch). This is the benign direction and the \
         pin exists only to catch a large swing.",
        m.in_scope_planner_higher(),
    );

    // Reported, never gated: the clip's cost in thrown-away motion.
    eprintln!(
        "[{label}] clip cost (reported, not gated): {} of {} emitted cutting moves \
         ({:.1}%) sit in the reachable band and become rapids.",
        m.cut_moves_in_band,
        m.cut_moves_in_band + m.cut_moves_inside_effective + m.cut_moves_outside_containment,
        m.cut_moves_in_band as f64
            / (m.cut_moves_in_band + m.cut_moves_inside_effective + m.cut_moves_outside_containment)
                .max(1) as f64
            * 100.0,
    );
}

#[test]
fn boundary_clear_parity_contour_parallel() {
    let m = measure(
        "ContourParallel / boundary=left-half / clipped",
        ClearingStrategy3d::ContourParallel,
        Some(&half_stock_boundary()),
    );
    report(&m);
    assert_containment_parity_bars(&m);
}

#[test]
fn boundary_clear_parity_agent_search() {
    let m = measure(
        "AgentSearch / boundary=left-half / clipped",
        ClearingStrategy3d::AgentSearch,
        Some(&half_stock_boundary()),
    );
    report(&m);
    assert_containment_parity_bars(&m);
}

/// P1's hard half — does the pre-clear at `path.rs:408-436` actually engage?
///
/// Compares the PLANNER's own stock outside the containment with and without
/// the boundary set, same params otherwise. If the pre-clear runs, the boundary
/// run reads "cleared to the grid floor" on cells the control still carries as
/// stock. Without this the whole file could pass with `params.boundary` silently
/// ignored: the post-generation clip alone reproduces most of the divergence.
///
/// ("cleared to the grid floor" only counts cells with no mesh under them —
/// covered cells clear to the *surface* Z, which is still a clear but not to the
/// floor. Measured 2026-08-21: 1745 of 4005, against 0 for the control.)
#[test]
fn boundary_clear_preclear_engages_precondition() {
    let (mesh, si) = hemisphere();
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);
    let bbox = mesh.bbox;
    let r = TOOL_RADIUS;
    let cell_size = (TOOL_RADIUS / 6.0).max(0.1);
    let initial_stock = TriDexelStock::from_stock(
        bbox.min.x - r,
        bbox.min.y - r,
        bbox.max.x + r,
        bbox.max.y + r,
        bbox.min.z,
        STOCK_TOP_Z,
        cell_size,
    );
    let never_cancel = || false;
    let probe = |boundary: Option<Polygon2>| {
        let params = Adaptive3dParams {
            initial_stock: Some(initial_stock.clone()),
            world_stock_xy_bbox: Some(WORLD_XY),
            boundary,
            ..base_params(ClearingStrategy3d::ContourParallel)
        };
        debug_adaptive_3d_segments_for_f029_probe(&mesh, &si, &cutter, &params, &never_cancel)
            .expect("planner probe should succeed")
    };
    let (with_tops, rows, cols, cs, ou, ov, zmin, _) = probe(Some(half_stock_boundary()));
    let (without_tops, ..) = probe(None);

    let b = half_stock_boundary();
    let mut outside = 0u64;
    let mut cleared_with = 0u64;
    let mut cleared_without = 0u64;
    for row in 0..rows {
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let y = ov + row as f64 * cs;
            if b.contains_point(&P2::new(x, y)) {
                continue;
            }
            outside += 1;
            let i = row * cols + col;
            if (with_tops[i] as f64) <= zmin + 1e-3 {
                cleared_with += 1;
            }
            if (without_tops[i] as f64) <= zmin + 1e-3 {
                cleared_without += 1;
            }
        }
    }
    eprintln!(
        "PRECLEAR ENGAGEMENT: {outside} cells outside the containment; the planner reads \
         fully-cleared on {cleared_with} with the boundary set vs {cleared_without} without it."
    );
    assert!(outside > 0, "the containment must exclude some grid cells");
    assert!(
        cleared_with > cleared_without,
        "the boundary pre-clear at `path.rs:408-436` did not engage — {cleared_with} cleared \
         with the boundary set vs {cleared_without} without it. Every other bar in this file \
         would then be measuring the post-generation clip alone under a boundary-shaped label."
    );
}

/// ASPIRATIONAL — the fix contract for decision **D4**, carried here so that
/// when the fix lands the bar is un-ignored rather than re-derived.
///
/// # What has to change for this to go green
///
/// **F-A — stop conflating "where material is" with "where a cutter centre may
/// go."** `session::compute::resolve_containment_polygon` hands adaptive3d the
/// un-inset containment; `path.rs:408-436` then clears the planner's internal
/// stock outside it, while `apply_boundary_clip` gates centres against
/// `containment ⊖ r`. Either pre-clear on `containment ⊖ r` (one line — but it
/// re-opens O5b for the band, so probably wrong on its own) or, correctly,
/// bound the clearing strategy's centre placement by `containment ⊖ r` while
/// keeping the material grid at the containment.
///
/// **F-B — pass `params.boundary` into `waterline_cleanup`** and drop contour
/// points outside it. Small and self-contained; takes bar B2 from 138 to 0.
///
/// F-A is what takes B1 from 27.7% to 0; F-B alone does not (the leak is
/// outside the containment, where B1 does not look). Both are generated-geometry
/// changes near containment edges, which is why they are a separate decision
/// package with a before/after rather than part of this sentry's wave.
#[test]
#[ignore = "aspirational: red until D4 boundary fix (inset pre-clear + waterline_cleanup boundary param) lands"]
fn boundary_clear_in_scope_overclaim_eliminated_d4() {
    for (label, strategy) in [
        (
            "ContourParallel / boundary=left-half / clipped",
            ClearingStrategy3d::ContourParallel,
        ),
        (
            "AgentSearch / boundary=left-half / clipped",
            ClearingStrategy3d::AgentSearch,
        ),
    ] {
        let m = measure(label, strategy, Some(&half_stock_boundary()));
        report(&m);
        assert_eq!(
            m.in_scope_over_claim(),
            0,
            "[{label}] D4: {} of {} in-containment cells ({:.2}%) still read `sim_higher` — \
             in-scope, tool-reachable material the planner believes it removed. F-A is not \
             (fully) in place.",
            m.in_scope_over_claim(),
            m.in_containment_population(),
            m.in_scope_over_claim_fraction() * 100.0,
        );
        assert_eq!(
            m.cut_moves_outside_containment, 0,
            "[{label}] D4: {} emitted cutting endpoints still land outside the containment — \
             F-B (`waterline_cleanup` boundary parameter) is not in place.",
            m.cut_moves_outside_containment,
        );
    }
}
