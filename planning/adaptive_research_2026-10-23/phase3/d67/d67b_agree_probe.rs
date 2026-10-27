//! D67b probe (C24's follow-up: the outside cells where planner and clipped sim agree; a copy of
//! d67_waterleak_probe.rs with one more test). D67 probe (temporary instrument, not committed; a copy is kept in
//! planning/adaptive_research_2026-10-23/phase3/d67/). It reuses the fixture of
//! `adaptive3d_boundary_clear_parity.rs` (hemisphere, left-half containment
//! rectangle, the same params) and prints, per strategy:
//! 1. B2 (pre-clip non-rapid move endpoints outside the containment) split by
//!    section (clearing level or waterline cleanup, from the runtime
//!    annotations), MoveIntent and move kind;
//! 2. B2 after `clip_toolpath_to_boundary(&toolpath, &effective[0], SAFE_Z)`;
//! 3. the sim cells outside the containment whose top is below STOCK_TOP_Z,
//!    and the deepest such cut, for the clipped path (and, for reference, the
//!    unclipped path);
//! 4. the same with `params.geometry.centre_boundary = effective`.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a probe that prints its measurements"
)]

use std::collections::BTreeMap;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams,
    Adaptive3dRuntimeEvent, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
    debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::boundary::{
    ToolContainment, clip_toolpath_to_boundary, effective_boundary_reported,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};

const TOOL_RADIUS: f64 = 3.175;
const STOCK_TOP_Z: f64 = 25.0;
const SAFE_Z: f64 = 30.0;
const WORLD_XY: (f64, f64, f64, f64) = (-1.0e4, -1.0e4, 1.0e4, 1.0e4);

fn base_params(strategy: ClearingStrategy3d) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: 1.0,
            tolerance: 0.5,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stock_top_z: STOCK_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
        },
        trochoid_cap_mult: 1.6,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Plunge,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        initial_stock: None,
        clearing_strategy: strategy,
        z_blend: false,
    }
}

fn kind(m: &MoveType) -> &'static str {
    match m {
        MoveType::Rapid => "Rapid",
        MoveType::Linear { .. } => "Linear",
        MoveType::ArcCW { .. } => "ArcCW",
        MoveType::ArcCCW { .. } => "ArcCCW",
    }
}

/// B2 split: (section, intent, kind, vertical?) -> count.
fn b2_split(
    tp: &Toolpath,
    sections: &[(usize, &'static str)],
    b: &Polygon2,
) -> (usize, BTreeMap<String, usize>) {
    let mut out = BTreeMap::new();
    let mut n = 0;
    let mut prev = None;
    for (i, m) in tp.moves.iter().enumerate() {
        let from = prev;
        prev = Some(m.target);
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        if b.contains_point(&P2::new(m.target.x, m.target.y)) {
            continue;
        }
        n += 1;
        let sec = sections
            .iter()
            .rev()
            .find(|(start, _)| *start <= i)
            .map_or("before any level", |(_, s)| *s);
        let shape = match from {
            Some(f) if (f.x - m.target.x).hypot(f.y - m.target.y) < 1e-9 && m.target.z < f.z => "down",
            Some(f) if (f.x - m.target.x).hypot(f.y - m.target.y) < 1e-9 && m.target.z > f.z => "up",
            Some(f) if (f.z - m.target.z).abs() < 1e-9 => "flat",
            _ => "sloped",
        };
        *out.entry(format!(
            "{sec} | {:?} | {} | {shape}",
            m.intent,
            kind(&m.move_type)
        ))
        .or_insert(0) += 1;
    }
    (n, out)
}

thread_local! { static AGREE_DUMP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

struct Run {
    b2: usize,
    split: BTreeMap<String, usize>,
    b2_clipped: usize,
    out_cut_clipped: (usize, f64, f64, usize),
    out_cut_unclipped: (usize, f64, f64, usize),
}

fn run(strategy: ClearingStrategy3d, centre: bool) -> Run {
    let mesh: TriangleMesh = make_test_hemisphere(20.0, 16);
    let si = SpatialIndex::build(&mesh, 10.0);
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
    let boundary = Polygon2::rectangle(-23.5, -23.5, 0.0, 23.5);
    let (effective, _) = effective_boundary_reported(&boundary, ToolContainment::Inside, r);
    let params = {
        let mut p = base_params(strategy);
        p.initial_stock = Some(initial_stock.clone());
        p.geometry.world_stock_xy_bbox = Some(WORLD_XY);
        p.geometry.boundary = Some(boundary.clone());
        if centre {
            p.geometry.centre_boundary = effective.clone();
        }
        p
    };
    let never = || false;
    let (tops_raw, rows, cols, pg_cell, pg_ou, pg_ov, pg_zmin, _) =
        debug_adaptive_3d_segments_for_f029_probe(&mesh, &si, &cutter, &params, &never)
            .expect("planner probe");
    let out = rs_cam_core::adaptive3d::adaptive_3d_toolpath_output_with_cancel(
        &mesh, &si, &cutter, &params, &never, None,
    )
    .expect("toolpath");
    let mut sections: Vec<(usize, &'static str)> = Vec::new();
    for a in &out.annotations {
        match a.event {
            Adaptive3dRuntimeEvent::GlobalZLevel { .. } | Adaptive3dRuntimeEvent::RegionZLevel { .. } => {
                sections.push((a.move_index, "clearing level"));
            }
            Adaptive3dRuntimeEvent::WaterlineCleanup => sections.push((a.move_index, "waterline cleanup")),
            _ => {}
        }
    }
    sections.sort_by_key(|s| s.0);
    let tp = out.toolpath;
    let (b2, split) = b2_split(&tp, &sections, &boundary);
    let clipped = clip_toolpath_to_boundary(&tp, &effective[0], SAFE_Z);
    let (b2_clipped, _) = b2_split(&clipped, &[], &boundary);
    let sim_out = |path: &Toolpath| -> (usize, f64, f64, usize) {
        let mut s = initial_stock.clone();
        s.simulate_toolpath_with_metrics_with_cancel(
            path,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never,
        )
        .expect("sim");
        let mut n = 0;
        let mut deepest: f64 = 0.0;
        let mut far: f64 = 0.0;
        let mut beyond_one_cell = 0usize;
        for row in 0..rows {
            for col in 0..cols {
                let x = pg_ou + col as f64 * pg_cell;
                let y = pg_ov + row as f64 * pg_cell;
                if boundary.contains_point(&P2::new(x, y)) {
                    continue;
                }
                let ray = s.z_grid.ray(row, col);
                let top = ray.iter().map(|seg| seg.exit).fold(f32::NEG_INFINITY, f32::max);
                let top = if top.is_finite() { top as f64 } else { s.stock_bbox.min.z };
                let cut = STOCK_TOP_Z - top;
                if cut > 1e-3 {
                    n += 1;
                    deepest = deepest.max(cut);
                    // distance of the cell centre outside the rectangle (-23.5..0, +-23.5)
                    let dx = (x - 0.0).max(-23.5 - x).max(0.0);
                    let dy = (y.abs() - 23.5).max(0.0);
                    let d = dx.hypot(dy);
                    far = far.max(d);
                    if d > pg_cell { beyond_one_cell += 1; }
                }
            }
        }
        if AGREE_DUMP.with(|f| f.get()) && path.moves.len() != tp.moves.len() {
            let tol = cell_size;
            let mut k = 0usize;
            for row in 0..rows {
                for col in 0..cols {
                    let x = pg_ou + col as f64 * pg_cell;
                    let y = pg_ov + row as f64 * pg_cell;
                    if boundary.contains_point(&P2::new(x, y)) {
                        continue;
                    }
                    let p = { let z = tops_raw[row * cols + col]; if z.is_finite() { z as f64 } else { pg_zmin } };
                    let ray = s.z_grid.ray(row, col);
                    let top = ray.iter().map(|seg| seg.exit).fold(f32::NEG_INFINITY, f32::max);
                    let st = if top.is_finite() { top as f64 } else { s.stock_bbox.min.z };
                    if (p - st).abs() <= tol {
                        k += 1;
                        let dx = (x - 0.0).max(-23.5 - x).max(0.0);
                        let dy = (y.abs() - 23.5).max(0.0);
                        eprintln!("AGREE {k:3} x {x:8.3} y {y:8.3} dist_outside {:6.3} planner {p:8.3} sim {st:8.3} stock_top {STOCK_TOP_Z} planner_floor {pg_zmin:.3}", dx.hypot(dy));
                    }
                }
            }
        }
        (n, deepest, far, beyond_one_cell)
    };
    let out_cut_clipped = sim_out(&clipped);
    let out_cut_unclipped = sim_out(&tp);
    Run { b2, split, b2_clipped, out_cut_clipped, out_cut_unclipped }
}

#[test]
fn d67b_agree_cells_outside() {
    for (label, strategy) in [
        ("ContourParallel", ClearingStrategy3d::ContourParallel),
        ("GeodesicAdaptive", ClearingStrategy3d::GeodesicAdaptive),
    ] {
        eprintln!("\n=== D67b {label}, centre_boundary EMPTY, clipped sim: outside cells where |planner - sim| <= one cell ===");
        AGREE_DUMP.with(|f| f.set(true));
        let _ = run(strategy, false);
        AGREE_DUMP.with(|f| f.set(false));
    }
}

#[test]
fn d67_waterleak_probe() {
    for (label, strategy) in [
        ("ContourParallel", ClearingStrategy3d::ContourParallel),
        ("GeodesicAdaptive", ClearingStrategy3d::GeodesicAdaptive),
    ] {
        for centre in [false, true] {
            let r = run(strategy, centre);
            eprintln!(
                "\n=== D67 {label}, centre_boundary {} ===",
                if centre { "= effective (as a session sets it)" } else { "EMPTY (the fixture)" }
            );
            eprintln!("B2 pre-clip: {}", r.b2);
            for (k, v) in &r.split {
                eprintln!("  {v:5}  {k}");
            }
            eprintln!("B2 after clip_toolpath_to_boundary: {}", r.b2_clipped);
            eprintln!(
                "sim cells OUTSIDE the containment cut below STOCK_TOP_Z: clipped path {} (deepest {:.3} mm; farthest cell centre {:.3} mm outside; {} cells more than one cell ({:.3} mm) outside); unclipped path {} (deepest {:.3} mm; farthest {:.3} mm; {} beyond one cell)",
                r.out_cut_clipped.0, r.out_cut_clipped.1, r.out_cut_clipped.2, r.out_cut_clipped.3, (TOOL_RADIUS / 6.0).max(0.1),
                r.out_cut_unclipped.0, r.out_cut_unclipped.1, r.out_cut_unclipped.2, r.out_cut_unclipped.3
            );
        }
    }
}
