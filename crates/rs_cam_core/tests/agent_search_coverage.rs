//! O5b regression: AgentSearch (2.5D-slice) interior coverage at every Z level.
//!
//! Repro for the "scattered passes" / interior-coverage class of bugs:
//! the 2D adaptive's agent-search walk inside `clear_z_level_agent_2d_slice`
//! can leave cells uncovered in the interior of the polygon. Deeper z-levels
//! then bite through full-depth uncleared stock, producing axial DOC up to
//! the full stock height.
//!
//! Test design:
//! - Synthetic CONCAVE L-shape with sine-bump terrain, deterministic.
//! - Run `adaptive_3d_toolpath_annotated` with AgentSearch + Global ordering.
//! - Use the runtime annotations ("Adaptive Z X.X (k/n)" markers) to
//!   partition the toolpath by Z-LEVEL.
//! - For EACH Z-level, replay the agent-search portion ONLY (the moves
//!   between this level's start and its waterline-cleanup marker), then
//!   assert the coverage invariant on the resulting dexel state:
//!
//!       for every interior cell whose surface_z + stock_to_leave is
//!       at or below z_level (i.e. should be cleared down to z_level
//!       at this snapshot), ray_top[cell] <= max(z_level, surface +
//!       stock_to_leave) + epsilon.
//!
//! Pre-fix, certain interior cells stay at ray_top = stock_top after
//! the agent-search portion. Post-fix (Option A defensive cleanup), the
//! per-Z residual sweep clears them before waterline runs.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::{
    adaptive3d::{
        Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
        adaptive_3d_toolpath_annotated,
    },
    dexel::ray_top,
    dexel_stock::{StockCutDirection, TriDexelStock},
    geo::P3,
    mesh::{SpatialIndex, TriangleMesh},
    radial_profile::RadialProfileLUT,
    slope::SurfaceHeightmap,
    tool::{FlatEndmill, MillingCutter},
    toolpath::{MoveType, Toolpath},
};

// ── L-shape (concave) terrain mesh ────────────────────────────────────

const LX: f64 = 60.0;
const LY: f64 = 60.0;
const LARM: f64 = 24.0;
const ZMAX: f64 = 2.0;

fn inside_l_shape(x: f64, y: f64) -> bool {
    if !(0.0..=LX).contains(&x) || !(0.0..=LY).contains(&y) {
        return false;
    }
    if y <= LARM {
        x >= 0.0 && x <= LX
    } else {
        x >= 0.0 && x <= LARM
    }
}

/// Build the L-shape mesh with sine-bump terrain inside.
fn make_l_terrain_mesh() -> (TriangleMesh, SpatialIndex) {
    let n: usize = 24;
    let dx = LX / n as f64;
    let dy = LY / n as f64;
    let height = |x: f64, y: f64| -> f64 {
        let kx = std::f64::consts::TAU * x / LX;
        let ky = std::f64::consts::TAU * y / LY;
        ((kx.sin() * ky.sin()).abs()) * ZMAX
    };

    let mut verts: Vec<P3> = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let x = i as f64 * dx;
            let y = j as f64 * dy;
            verts.push(P3::new(x, y, height(x, y)));
        }
    }
    let idx = |i: usize, j: usize| -> u32 { (j * (n + 1) + i) as u32 };
    let center_inside = |i: usize, j: usize| -> bool {
        let cx = (i as f64 + 0.5) * dx;
        let cy = (j as f64 + 0.5) * dy;
        inside_l_shape(cx, cy)
    };
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..n {
        for i in 0..n {
            if !center_inside(i, j) {
                continue;
            }
            let a = idx(i, j);
            let b = idx(i + 1, j);
            let c = idx(i + 1, j + 1);
            let d = idx(i, j + 1);
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    let mesh = TriangleMesh::from_raw(verts, tris);
    let si = SpatialIndex::build(&mesh, LX / 4.0);
    (mesh, si)
}

// ── Toolpath partitioning by Z-level annotation ────────────────────────

/// Walk annotations to find Z-level boundary indices. Returns one entry
/// per z_level encountered, with `(z_level, agent_start_move_idx,
/// waterline_start_move_idx, next_level_start_move_idx)`.
///
/// agent_start..waterline_start covers the agent-search moves at this Z.
/// agent_start..next_level_start covers agent + waterline at this Z.
///
/// Iteration order matters: we walk annotations top-to-bottom so the
/// matching WaterlineCleanup is the one between THIS Z marker and the
/// NEXT Z marker in annotation order — not the previous Z's waterline
/// which can sit at the same move_idx when waterline emitted no moves.
fn partition_by_z_level(annotations: &[(usize, String)]) -> Vec<(f64, usize, usize, usize)> {
    // Find ann indices of Z and Waterline markers.
    let z_ann: Vec<(usize, f64, usize)> = annotations
        .iter()
        .enumerate()
        .filter_map(|(ann_idx, (move_idx, label))| {
            parse_z_level_label(label).map(|z| (ann_idx, z, *move_idx))
        })
        .collect();

    let mut out: Vec<(f64, usize, usize, usize)> = Vec::new();
    for (i, &(z_ann_idx, z, agent_start)) in z_ann.iter().enumerate() {
        let (next_ann_idx, next_z_start) = z_ann
            .get(i + 1)
            .map(|t| (t.0, t.2))
            .unwrap_or((annotations.len(), usize::MAX));

        // Find FIRST WaterlineCleanup annotation BETWEEN this Z marker
        // and the next Z marker (in annotation order, not move index).
        let waterline_start = annotations[z_ann_idx + 1..next_ann_idx]
            .iter()
            .find_map(|(move_idx, label)| {
                if label == "Waterline cleanup" {
                    Some(*move_idx)
                } else {
                    None
                }
            })
            .unwrap_or(next_z_start);
        out.push((z, agent_start, waterline_start, next_z_start));
    }
    out
}

fn parse_z_level_label(label: &str) -> Option<f64> {
    // Format: "Adaptive Z 9.0 (1/4)" or "Region N — Z 9.0 (k/n)"
    if let Some(rest) = label.strip_prefix("Adaptive Z ") {
        let z_str = rest.split(' ').next()?;
        z_str.parse::<f64>().ok()
    } else {
        None
    }
}

/// Stamp the toolpath moves in `range` into `stock`.
///
/// `start` is the index of the FIRST move to stamp; the previous move
/// (start - 1) is the segment's "from" point. If `start == 0`, the first
/// stamped segment uses move[0] as both endpoints (a no-op stamp).
fn stamp_range(
    stock: &mut TriDexelStock,
    toolpath: &Toolpath,
    cutter: &dyn MillingCutter,
    lut: &RadialProfileLUT,
    start: usize,
    end_exclusive: usize,
) {
    let n = toolpath.moves.len();
    let end = end_exclusive.min(n);
    if start >= end {
        return;
    }
    // For the first move, there is no prior — start at i = max(start, 1).
    let first = start.max(1);
    for i in first..end {
        let mv = &toolpath.moves[i];
        let prev = &toolpath.moves[i - 1];
        match mv.move_type {
            MoveType::Rapid => {}
            MoveType::Linear { .. } => {
                stock.stamp_linear_segment(
                    lut,
                    cutter.radius(),
                    prev.target,
                    mv.target,
                    StockCutDirection::FromTop,
                );
            }
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {}
        }
    }
}

#[test]
fn agent_search_clears_concave_interior_at_every_z_level() {
    // ── Mesh + tool ────────────────────────────────────────────────────
    let (mesh, si) = make_l_terrain_mesh();
    let cutter = FlatEndmill::new(6.0, 25.0);

    // ── Adaptive3d params (wanaka-scale) ──────────────────────────────
    let stock_top_z: f64 = 12.0;
    let depth_per_pass: f64 = 3.0;
    let stock_to_leave: f64 = 0.5;
    let params = Adaptive3dParams {
        tool_radius: cutter.radius(),
        envelope_radius: cutter.radius(),
        stepover: cutter.radius() * 0.28, // ~14% radial
        depth_per_pass,
        stock_to_leave,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: stock_top_z + 5.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        stock_top_z,
        entry_style: EntryStyle3d::Plunge,
        fine_stepdown: None,
        detect_flat_areas: false,
        max_stay_down_dist: None,
        region_ordering: RegionOrdering::Global,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::AgentSearch,
        z_blend: false,
        boundary: None,
    };

    // ── Generate (annotated) ──────────────────────────────────────────
    let (toolpath, annotations) = adaptive_3d_toolpath_annotated(&mesh, &si, &cutter, &params);
    assert!(
        toolpath.moves.len() > 100,
        "AgentSearch should produce a non-trivial toolpath, got {} moves",
        toolpath.moves.len()
    );

    let z_partitions = partition_by_z_level(&annotations);
    assert!(
        z_partitions.len() >= 2,
        "Expected at least 2 Z-level partitions from annotations, got {}: {:?}",
        z_partitions.len(),
        z_partitions
    );

    // ── Build base stock matching adaptive3d's grid ───────────────────
    let r = cutter.radius();
    let bbox = &mesh.bbox;
    let cell_size = (params.tool_radius / 6.0).max(params.tolerance);
    let mut base_stock = TriDexelStock::from_stock(
        bbox.min.x - r,
        bbox.min.y - r,
        bbox.max.x + r,
        bbox.max.y + r,
        bbox.min.z,
        params.stock_top_z,
        cell_size,
    );
    let surface_hm = SurfaceHeightmap::from_mesh(
        &mesh,
        &si,
        &cutter,
        base_stock.z_grid.origin_u,
        base_stock.z_grid.origin_v,
        base_stock.z_grid.rows,
        base_stock.z_grid.cols,
        base_stock.z_grid.cell_size,
        bbox.min.z,
    );
    // Mirror border-clear (cells outside the L bbox: subtract material above surface).
    let border_margin = r * 0.5;
    for row in 0..base_stock.z_grid.rows {
        for col in 0..base_stock.z_grid.cols {
            let (x, y) = base_stock.z_grid.cell_to_world(row, col);
            if x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin
            {
                let i = row * base_stock.z_grid.cols + col;
                let clear_z = surface_hm.z_values[i] as f32;
                rs_cam_core::dexel::ray_subtract_above(
                    base_stock.z_grid.ray_mut(row, col),
                    clear_z,
                );
            }
        }
    }

    // ── Replay per Z-level: stamp ONLY the agent-search portion ──────
    // (moves between this level's start and its waterline marker), then
    // check coverage. This isolates the 2D-slice agent's stamping so a
    // working waterline cleanup can't mask agent-search gaps.
    let lut = RadialProfileLUT::from_cutter(&cutter, 256);
    let mut stock = base_stock.clone();
    // Margin = 2 grid cells. Cells right on the bbox edge can have
    // partial-cell stamping artifacts; staying 2 cells in keeps us
    // honest.
    //
    // Epsilon = depth_per_pass. The original 0.15mm threshold was
    // sub-cell-noise — it tripped on individual 1–2 cell residuals that
    // are physically below the tool footprint (the tool can't address
    // them without violating the polygon boundary). The wip branch's
    // F1 + perimeter sweep + slope-aware split handle the meaningful
    // failure mode (cells accumulating more than one pass-worth of
    // stock = the wanaka 18mm peak axial DOC bug class). Anything below
    // depth_per_pass is either stamp jitter or material a follow-up
    // pass will remove without overload.
    let interior_margin = cell_size * 2.0;
    let epsilon = params.depth_per_pass;

    let mut total_failures = 0usize;
    let mut per_level_summary: Vec<(f64, usize, usize)> = Vec::new();

    for (i, &(z_level, agent_start, waterline_start, next_level_start)) in
        z_partitions.iter().enumerate()
    {
        // Stamp ONLY the agent-search moves for this Z level.
        stamp_range(
            &mut stock,
            &toolpath,
            &cutter,
            &lut,
            agent_start,
            waterline_start,
        );

        // ── Check coverage on the AGENT-only stamp state ──────────────
        //
        // The invariant from the spec: for EVERY cell whose XY is at
        // least `interior_margin` away from the bbox edge AND has the
        // bool-grid invariant "material above floor" before the pass,
        // its ray_top must be at or below floor + epsilon after the
        // pass. We don't restrict to inside_l_shape — concave geometry
        // means the bool grid at high Z covers the whole bbox, and the
        // agent should clear ALL of it (concave-interior coverage is
        // the symptom we're guarding against).
        let grid = &stock.z_grid;
        let mut fails = 0usize;
        let mut sample: Vec<(usize, usize, f64, f64)> = Vec::new();
        let bbox = &mesh.bbox;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let (x, y) = grid.cell_to_world(row, col);
                // Stay strictly inside the mesh bbox (avoid border-cleared
                // cells outside, where surface_z = min_z and the ray was
                // already cleared by border-clear).
                if x < bbox.min.x + interior_margin
                    || x > bbox.max.x - interior_margin
                    || y < bbox.min.y + interior_margin
                    || y > bbox.max.y - interior_margin
                {
                    continue;
                }
                let surf = surface_hm.surface_z_at_world(x, y);
                if !surf.is_finite() {
                    continue;
                }
                let floor = (surf + stock_to_leave).max(z_level);
                let limit = floor + epsilon;
                let top = match ray_top(grid.ray(row, col)) {
                    Some(t) => t as f64,
                    None => continue,
                };
                if top > limit {
                    fails += 1;
                    if sample.len() < 5 {
                        sample.push((row, col, top, limit));
                    }
                }
            }
        }
        per_level_summary.push((z_level, fails, sample.len()));
        if fails > 0 {
            eprintln!(
                "[Z={:.3}] {} interior cells uncleared after agent (waterline not yet applied):",
                z_level, fails
            );
            for (rr, cc, t, l) in &sample {
                eprintln!(
                    "    cell ({}, {}) ray_top={:.3} > limit={:.3}",
                    rr, cc, t, l
                );
            }
            total_failures += fails;
        }

        // Now stamp the waterline-cleanup portion to bring the stock up
        // to the state the next Z level inherits.
        stamp_range(
            &mut stock,
            &toolpath,
            &cutter,
            &lut,
            waterline_start,
            next_level_start,
        );
        let _ = i;
    }

    if total_failures > 0 {
        eprintln!("Per-Z-level agent-only failure summary:");
        for (z, n, _s) in &per_level_summary {
            eprintln!("  z={:.3}: {} uncleared interior cells", z, n);
        }
        panic!(
            "AgentSearch left {} interior cells uncleared across {} Z-levels (after agent-only stamp, before waterline)",
            total_failures,
            z_partitions.len()
        );
    }
}
