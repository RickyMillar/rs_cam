//! Display-stride tests (memory programme 2026-10-01, wave 3 group J).
//!
//! Plan: `planning/memory_budget_2026-10-01/PLAN.md`, Architecture 4b.
//!
//! 1. The refactor of `z_grid_marching_cubes` into shared helpers keeps the
//!    mesh bit for bit: the legacy body below is a verbatim copy of the
//!    function at `b3a4a7ac`, and each fixture compares the two.
//! 2. The block path at `k = 1` gives the same mesh as the full build.
//! 3. Stride 2 keeps about 1/4 of the vertices and the same bounding box.
//! 4. `display_stride_for` arithmetic, on the plan's rivmap350 numbers.

// SAFETY: test module; the legacy oracle and the fixtures index in bounds,
// and a failed precondition panics on purpose.
#![allow(clippy::indexing_slicing, clippy::panic)]

use super::{
    MIN_MATERIAL_THICKNESS, cell_center, display_stride_for, find_matching_gap,
    marching_cubes_on_blocks, wood_color_at_z, z_grid_marching_cubes,
    z_grid_marching_cubes_strided,
};
use crate::dexel_stock::TriDexelStock;
use crate::stock::dexel::{
    DexelGrid, ray_bottom, ray_subtract_above, ray_subtract_interval, ray_top,
};
use crate::stock::stock_mesh::StockMesh;

// ── The legacy oracle: verbatim from `b3a4a7ac` (renamed only). ──────────

#[allow(clippy::indexing_slicing)]
fn legacy_z_grid_marching_cubes(
    grid: &DexelGrid,
    stock_top_z: f64,
    stock_bottom_z: f64,
) -> StockMesh {
    let rows = grid.rows;
    let cols = grid.cols;
    if rows < 1 || cols < 1 {
        return StockMesh::empty();
    }

    let stock_top = stock_top_z as f32;
    let stock_bot = stock_bottom_z as f32;
    let z_range = (stock_top - stock_bot).max(1e-6);

    // ── 1. Per-cell envelope: top / bottom Z, emptiness flag.
    let cells = rows * cols;
    let mut cell_top: Vec<f32> = Vec::with_capacity(cells);
    let mut cell_bot: Vec<f32> = Vec::with_capacity(cells);
    let mut cell_empty: Vec<bool> = Vec::with_capacity(cells);
    let mut max_segments = 1usize;
    for ray in &grid.rays {
        let top = ray_top(ray);
        let bot = ray_bottom(ray);
        let effectively_empty = match (top, bot) {
            (Some(t), Some(b)) => (t - b) < MIN_MATERIAL_THICKNESS,
            _ => true,
        };
        cell_empty.push(effectively_empty);
        cell_top.push(top.unwrap_or(stock_bot));
        cell_bot.push(bot.unwrap_or(stock_bot));
        if ray.len() > max_segments {
            max_segments = ray.len();
        }
    }

    // ── 2. Per-corner bilinear top/bottom (rows+1) × (cols+1). Out-of-grid
    //       and effectively-empty cells contribute `stock_bot` so the
    //       corner-average dives at boundaries → sub-cell-accurate walls.
    let corner_rows = rows + 1;
    let corner_cols = cols + 1;
    let corner_count = corner_rows * corner_cols;
    let mut corner_top: Vec<f32> = vec![0.0; corner_count];
    let mut corner_bot: Vec<f32> = vec![0.0; corner_count];
    let mut corner_empty: Vec<bool> = vec![false; corner_count];
    for ci in 0..corner_rows {
        for cj in 0..corner_cols {
            let mut sum_top = 0.0_f32;
            let mut sum_bot = 0.0_f32;
            let mut count = 0.0_f32;
            let mut all_empty = true;
            for di in 0..2usize {
                if ci + di == 0 || ci + di > rows {
                    continue;
                }
                let r = ci + di - 1;
                for dj in 0..2usize {
                    if cj + dj == 0 || cj + dj > cols {
                        continue;
                    }
                    let c = cj + dj - 1;
                    let idx = r * cols + c;
                    if !cell_empty[idx] {
                        sum_top += cell_top[idx];
                        sum_bot += cell_bot[idx];
                        count += 1.0;
                        all_empty = false;
                    }
                }
            }
            let cidx = ci * corner_cols + cj;
            if all_empty || count == 0.0 {
                corner_empty[cidx] = true;
                corner_top[cidx] = stock_top;
                corner_bot[cidx] = stock_bot;
            } else {
                corner_top[cidx] = sum_top / count;
                corner_bot[cidx] = sum_bot / count;
            }
        }
    }

    // Corner world position helper. Cell-corner (ci, cj) sits at
    // `(origin_u + (cj - 0.5) * cs, origin_v + (ci - 0.5) * cs)` — exactly the
    // stock bbox boundary at ci=0, cj=0, etc.
    let corner_xy = |ci: usize, cj: usize| -> (f32, f32) {
        let u = grid.origin_u + (cj as f64 - 0.5) * grid.cell_size;
        let v = grid.origin_v + (ci as f64 - 0.5) * grid.cell_size;
        (u as f32, v as f32)
    };

    // ── 3. Shared-vertex emit: build vertices at each non-empty corner once
    //       (top + bottom), record their indices, then emit triangles by
    //       index. This reduces vertex count from 6×cells to 2×corners — a
    //       ~6× reduction in allocations and per-vertex work.
    let mut top_idx: Vec<u32> = vec![u32::MAX; corner_count];
    let mut bot_idx: Vec<u32> = vec![u32::MAX; corner_count];
    // Reasonable capacity: top + bottom corner counts (most corners
    // non-empty), plus a small overhead for cavity emissions.
    let cap = (corner_count * 2 * 3) + (rows + cols) * 12;
    let mut vertices: Vec<f32> = Vec::with_capacity(cap);
    let mut colors: Vec<f32> = Vec::with_capacity(cap);
    let mut indices: Vec<u32> = Vec::with_capacity(cells * 12);

    for ci in 0..corner_rows {
        for cj in 0..corner_cols {
            let cidx = ci * corner_cols + cj;
            if corner_empty[cidx] {
                continue;
            }
            let (u, v) = corner_xy(ci, cj);
            // Top vertex.
            let ti = (vertices.len() / 3) as u32;
            let tz = corner_top[cidx];
            vertices.extend_from_slice(&[u, v, tz]);
            let (cr, cg, cb) = wood_color_at_z(tz, stock_top, stock_bot, z_range);
            colors.extend_from_slice(&[cr, cg, cb]);
            top_idx[cidx] = ti;
            // Bottom vertex.
            let bi = (vertices.len() / 3) as u32;
            let bz = corner_bot[cidx];
            vertices.extend_from_slice(&[u, v, bz]);
            let (cr, cg, cb) = wood_color_at_z(bz, stock_top, stock_bot, z_range);
            colors.extend_from_slice(&[cr, cg, cb]);
            bot_idx[cidx] = bi;
        }
    }

    // ── 4. Top face: 2 triangles per cell using shared corner indices.
    for ci in 0..rows {
        for cj in 0..cols {
            if cell_empty[ci * cols + cj] {
                continue;
            }
            let c00 = ci * corner_cols + cj;
            let c01 = ci * corner_cols + cj + 1;
            let c10 = (ci + 1) * corner_cols + cj;
            let c11 = (ci + 1) * corner_cols + cj + 1;
            if corner_empty[c00] || corner_empty[c01] || corner_empty[c10] || corner_empty[c11] {
                continue;
            }
            // Winding for +Z normal (CCW viewed from +Z): (c00, c10, c01),
            // (c01, c10, c11).
            let v00 = top_idx[c00];
            let v01 = top_idx[c01];
            let v10 = top_idx[c10];
            let v11 = top_idx[c11];
            indices.extend_from_slice(&[v00, v10, v01, v01, v10, v11]);
        }
    }

    // ── 5. Bottom face: same cells, reversed winding (−Z normal).
    for ci in 0..rows {
        for cj in 0..cols {
            if cell_empty[ci * cols + cj] {
                continue;
            }
            let c00 = ci * corner_cols + cj;
            let c01 = ci * corner_cols + cj + 1;
            let c10 = (ci + 1) * corner_cols + cj;
            let c11 = (ci + 1) * corner_cols + cj + 1;
            if corner_empty[c00] || corner_empty[c01] || corner_empty[c10] || corner_empty[c11] {
                continue;
            }
            let v00 = bot_idx[c00];
            let v01 = bot_idx[c01];
            let v10 = bot_idx[c10];
            let v11 = bot_idx[c11];
            indices.extend_from_slice(&[v00, v01, v10, v01, v11, v10]);
        }
    }

    // ── 6. Perimeter skirt — vertical quads using shared corner indices.
    // Front edge (ci = 0): normals face −V → CCW from −V.
    for cj in 0..cols {
        let cl = cj;
        let cr = cj + 1;
        if corner_empty[cl] || corner_empty[cr] {
            continue;
        }
        let tl = top_idx[cl];
        let tr = top_idx[cr];
        let bl = bot_idx[cl];
        let br = bot_idx[cr];
        indices.extend_from_slice(&[tl, tr, bl, tr, br, bl]);
    }
    // Back edge (ci = rows): normals face +V.
    for cj in 0..cols {
        let cl = rows * corner_cols + cj;
        let cr = rows * corner_cols + cj + 1;
        if corner_empty[cl] || corner_empty[cr] {
            continue;
        }
        let tl = top_idx[cl];
        let tr = top_idx[cr];
        let bl = bot_idx[cl];
        let br = bot_idx[cr];
        indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }
    // Left edge (cj = 0): normals face −U.
    for ci in 0..rows {
        let cb = ci * corner_cols;
        let ct = (ci + 1) * corner_cols;
        if corner_empty[cb] || corner_empty[ct] {
            continue;
        }
        let tb = top_idx[cb];
        let tt = top_idx[ct];
        let bb = bot_idx[cb];
        let bt = bot_idx[ct];
        indices.extend_from_slice(&[tb, bb, tt, tt, bb, bt]);
    }
    // Right edge (cj = cols): normals face +U.
    for ci in 0..rows {
        let cb = ci * corner_cols + cols;
        let ct = (ci + 1) * corner_cols + cols;
        if corner_empty[cb] || corner_empty[ct] {
            continue;
        }
        let tb = top_idx[cb];
        let tt = top_idx[ct];
        let bb = bot_idx[cb];
        let bt = bot_idx[ct];
        indices.extend_from_slice(&[tb, tt, bb, tt, bt, bb]);
    }

    // ── 7. Hole walls — vertical quads at material/empty cell boundaries.
    // Column-direction edges (between cells (ci, cj) and (ci, cj+1)).
    for ci in 0..rows {
        for cj in 0..(cols.saturating_sub(1)) {
            let a_empty = cell_empty[ci * cols + cj];
            let b_empty = cell_empty[ci * cols + cj + 1];
            if a_empty == b_empty {
                continue;
            }
            let cb = ci * corner_cols + cj + 1;
            let ct = (ci + 1) * corner_cols + cj + 1;
            if corner_empty[cb] || corner_empty[ct] {
                continue;
            }
            let tb = top_idx[cb];
            let tt = top_idx[ct];
            let bb = bot_idx[cb];
            let bt = bot_idx[ct];
            if !a_empty {
                indices.extend_from_slice(&[tb, tt, bb, tt, bt, bb]);
            } else {
                indices.extend_from_slice(&[tb, bb, tt, tt, bb, bt]);
            }
        }
    }
    // Row-direction edges (between cells (ci, cj) and (ci+1, cj)).
    for ci in 0..(rows.saturating_sub(1)) {
        for cj in 0..cols {
            let a_empty = cell_empty[ci * cols + cj];
            let b_empty = cell_empty[(ci + 1) * cols + cj];
            if a_empty == b_empty {
                continue;
            }
            let cl = (ci + 1) * corner_cols + cj;
            let cr = (ci + 1) * corner_cols + cj + 1;
            if corner_empty[cl] || corner_empty[cr] {
                continue;
            }
            let tl = top_idx[cl];
            let tr = top_idx[cr];
            let bl = bot_idx[cl];
            let br = bot_idx[cr];
            if !a_empty {
                indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            } else {
                indices.extend_from_slice(&[tl, tr, bl, tr, br, bl]);
            }
        }
    }

    // ── 7. Multi-segment cavity fallback (per-gap pass). Only runs when at
    //       least one ray has >1 segment.
    if max_segments > 1 {
        legacy_emit_cavity_surfaces(
            grid,
            &mut vertices,
            &mut indices,
            &mut colors,
            stock_top,
            stock_bot,
            z_range,
        );
    }

    StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// Multi-segment cavity emission. Walks each ray with >1 segment; per gap
/// between segments, emits a ceiling face (at gap_bot, normal −Z) and a floor
/// face (at gap_top, normal +Z) for the 2x2 cell block sharing the gap.
/// Vertical cavity walls are emitted where adjacent cells differ in gap
/// topology.
#[allow(clippy::indexing_slicing)]
fn legacy_emit_cavity_surfaces(
    grid: &DexelGrid,
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<f32>,
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
) {
    let rows = grid.rows;
    let cols = grid.cols;
    // Phase 1: per-cell gap intervals.
    let cell_gaps: Vec<Vec<[f32; 2]>> = grid
        .rays
        .iter()
        .map(|ray| {
            if ray.len() < 2 {
                return Vec::new();
            }
            let mut gaps = Vec::with_capacity(ray.len() - 1);
            for w in ray.windows(2) {
                let (lo, hi) = (&w[0], &w[1]);
                let gap_bot = lo.exit;
                let gap_top = hi.enter;
                if gap_top - gap_bot > 1e-6 {
                    gaps.push([gap_bot, gap_top]);
                }
            }
            gaps
        })
        .collect();

    // Phase 2: 2x2 quad floors/ceilings.
    for row in 0..rows.saturating_sub(1) {
        for col in 0..cols.saturating_sub(1) {
            let tl_gaps = &cell_gaps[row * cols + col];
            let tr_gaps = &cell_gaps[row * cols + col + 1];
            let bl_gaps = &cell_gaps[(row + 1) * cols + col];
            let br_gaps = &cell_gaps[(row + 1) * cols + col + 1];

            for tl_gap in tl_gaps {
                let tr_m = find_matching_gap(tr_gaps, tl_gap);
                let bl_m = find_matching_gap(bl_gaps, tl_gap);
                let br_m = find_matching_gap(br_gaps, tl_gap);
                if let (Some(tr_g), Some(bl_g), Some(br_g)) = (tr_m, bl_m, br_m) {
                    let pts = [
                        cell_center(grid, row, col, tl_gap[0]),
                        cell_center(grid, row, col + 1, tr_g[0]),
                        cell_center(grid, row + 1, col, bl_g[0]),
                        cell_center(grid, row + 1, col + 1, br_g[0]),
                    ];
                    let base = (vertices.len() / 3) as u32;
                    for &p in &pts {
                        vertices.push(p.0);
                        vertices.push(p.1);
                        vertices.push(p.2);
                        let (cr, cg, cb) = wood_color_at_z(p.2, stock_top, stock_bot, z_range);
                        colors.push(cr);
                        colors.push(cg);
                        colors.push(cb);
                    }
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base + 1,
                        base + 3,
                        base + 2,
                    ]);

                    let pts2 = [
                        cell_center(grid, row, col, tl_gap[1]),
                        cell_center(grid, row, col + 1, tr_g[1]),
                        cell_center(grid, row + 1, col, bl_g[1]),
                        cell_center(grid, row + 1, col + 1, br_g[1]),
                    ];
                    let base2 = (vertices.len() / 3) as u32;
                    for &p in &pts2 {
                        vertices.push(p.0);
                        vertices.push(p.1);
                        vertices.push(p.2);
                        let (cr, cg, cb) = wood_color_at_z(p.2, stock_top, stock_bot, z_range);
                        colors.push(cr);
                        colors.push(cg);
                        colors.push(cb);
                    }
                    indices.extend_from_slice(&[
                        base2,
                        base2 + 2,
                        base2 + 1,
                        base2 + 1,
                        base2 + 2,
                        base2 + 3,
                    ]);
                }
            }
        }
    }
}

// ── Fixtures ──────────────────────────────────────────────────────────────

/// Named grids that reach every branch of the extraction: the plain block,
/// a partial cut, a through-hole, a top-and-bottom cut and a cavity
/// (multi-segment rays).
fn fixtures() -> Vec<(&'static str, TriDexelStock, f64, f64)> {
    let mut out = Vec::new();

    let uncut = TriDexelStock::from_stock(0.0, 0.0, 9.0, 7.0, 0.0, 5.0, 1.0);
    out.push(("uncut", uncut, 5.0, 0.0));

    let mut partial = TriDexelStock::from_stock(0.0, 0.0, 9.0, 7.0, 0.0, 5.0, 1.0);
    for (r, c, z) in [(2, 2, 2.0_f32), (2, 3, 3.5), (3, 4, 1.25), (5, 8, 4.0)] {
        ray_subtract_above(partial.z_grid.ray_mut(r, c), z);
    }
    out.push(("partial", partial, 5.0, 0.0));

    let mut hole = TriDexelStock::from_stock(0.0, 0.0, 9.0, 7.0, 0.0, 5.0, 1.0);
    hole.z_grid.ray_mut(3, 3).clear();
    hole.z_grid.ray_mut(3, 4).clear();
    out.push(("through_hole", hole, 5.0, 0.0));

    let mut both = TriDexelStock::from_stock(0.0, 0.0, 8.0, 8.0, 0.0, 10.0, 1.0);
    for r in 3..=5 {
        for c in 3..=5 {
            ray_subtract_above(both.z_grid.ray_mut(r, c), 7.0);
            ray_subtract_interval(both.z_grid.ray_mut(r, c), 0.0, 3.0);
        }
    }
    out.push(("top_and_bottom", both, 10.0, 0.0));

    let mut cavity = TriDexelStock::from_stock(0.0, 0.0, 8.0, 8.0, 0.0, 10.0, 1.0);
    for r in 2..=5 {
        for c in 2..=5 {
            ray_subtract_interval(cavity.z_grid.ray_mut(r, c), 4.0, 6.0);
        }
    }
    out.push(("cavity", cavity, 10.0, 0.0));

    out
}

/// Bit-level equality of two meshes. `f32 ==` would treat `-0.0` and `0.0`
/// as equal; `to_bits` does not.
fn assert_bit_identical(name: &str, a: &StockMesh, b: &StockMesh) {
    let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
    assert_eq!(
        bits(&a.vertices),
        bits(&b.vertices),
        "{name}: vertices differ"
    );
    assert_eq!(a.indices, b.indices, "{name}: indices differ");
    assert_eq!(bits(&a.colors), bits(&b.colors), "{name}: colours differ");
}

/// (min_x, min_y, min_z, max_x, max_y, max_z) of a mesh's vertices.
fn bbox(mesh: &StockMesh) -> [f32; 6] {
    let mut b = [
        f32::INFINITY,
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for p in mesh.vertices.as_chunks::<3>().0 {
        for (axis, &x) in p.iter().enumerate() {
            b[axis] = b[axis].min(x);
            b[axis + 3] = b[axis + 3].max(x);
        }
    }
    b
}

// ── 1 + 2. Default behaviour is bit-identical. ────────────────────────────

#[test]
fn the_refactored_full_build_matches_the_legacy_body() {
    let mut cavity_seen = false;
    for (name, stock, top, bot) in fixtures() {
        let legacy = legacy_z_grid_marching_cubes(&stock.z_grid, top, bot);
        let now = z_grid_marching_cubes(&stock.z_grid, top, bot);
        assert!(!legacy.indices.is_empty(), "{name}: vacuous fixture");
        assert_bit_identical(name, &legacy, &now);
        cavity_seen |= stock.z_grid.rays.iter().any(|r| r.len() > 1);
    }
    assert!(
        cavity_seen,
        "no fixture reaches the multi-segment cavity pass"
    );
}

#[test]
fn strided_one_is_the_full_build() {
    for (name, stock, top, bot) in fixtures() {
        let full = z_grid_marching_cubes(&stock.z_grid, top, bot);
        let blocks = marching_cubes_on_blocks(&stock.z_grid, top, bot, 1);
        assert_bit_identical(name, &full, &blocks);
        // Stride 0 and 1 both mean "full resolution" on the public door.
        for stride in [0, 1] {
            let public = z_grid_marching_cubes_strided(&stock.z_grid, top, bot, stride);
            assert_bit_identical(name, &full, &public);
        }
    }
}

// ── 3. Stride 2: about 1/4 of the vertices, the same bounding box. ─────────

#[test]
fn stride_two_keeps_about_a_quarter_of_the_vertices_and_the_bbox() {
    // 40 x 30 mm at 1 mm: 41 x 31 cells, so stride 2 leaves a partial last
    // block on both axes. The bbox must still match.
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 5.0, 1.0);
    for r in 10..20 {
        for c in 10..30 {
            ray_subtract_above(stock.z_grid.ray_mut(r, c), 2.0);
        }
    }
    let full = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);
    let half = z_grid_marching_cubes_strided(&stock.z_grid, 5.0, 0.0, 2);

    let ratio = half.vertex_count() as f64 / full.vertex_count() as f64;
    assert!(
        (0.2..=0.32).contains(&ratio),
        "stride 2 vertex ratio {ratio} (full {}, strided {})",
        full.vertex_count(),
        half.vertex_count()
    );
    let index_ratio = half.indices.len() as f64 / full.indices.len() as f64;
    assert!(
        (0.2..=0.32).contains(&index_ratio),
        "stride 2 index ratio {index_ratio}"
    );
    assert_eq!(
        half.colors.len(),
        half.vertices.len(),
        "one colour per vertex"
    );
    assert_eq!(bbox(&full), bbox(&half), "the bbox must not move");
}

#[test]
fn a_thin_rib_survives_the_stride_at_full_height() {
    // One uncut column of cells in a field cut down to 1 mm. The corner
    // average lowers the rib in the full build too, so the claim is: the
    // maximum rule shows the rib as high as the full build does. A minimum
    // or a centre-sample rule would show 1 mm at some strides.
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 5.0, 1.0);
    let grid: &mut DexelGrid = &mut stock.z_grid;
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            if c != 9 {
                ray_subtract_above(grid.ray_mut(r, c), 1.0);
            }
        }
    }
    let full_top = bbox(&z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0))[5];
    assert!(
        full_top > 1.5,
        "vacuous fixture: no rib in the full build ({full_top})"
    );
    for stride in [2, 3, 4] {
        let mesh = z_grid_marching_cubes_strided(&stock.z_grid, 5.0, 0.0, stride);
        let top = bbox(&mesh)[5];
        assert!(
            (top - full_top).abs() < 1e-4,
            "stride {stride}: rib top {top}, the full build shows {full_top}"
        );
    }
}

#[test]
fn the_strided_mesh_is_watertight() {
    for (name, stock, top, bot) in fixtures() {
        if stock.z_grid.rays.iter().any(|r| r.len() > 1) {
            // Cavity faces are open sheets in the full build too.
            continue;
        }
        let mesh = z_grid_marching_cubes_strided(&stock.z_grid, top, bot, 2);
        assert!(
            super::tests::is_watertight(&mesh),
            "{name}: stride 2 mesh is not watertight"
        );
    }
}

// ── 4. `display_stride_for`. ───────────────────────────────────────────────

/// The plan's rivmap350 grid at 0.2 mm: 1901 x 2551 cells.
const RIVMAP350_CELLS: u64 = 1901 * 2551;
const MB: u64 = 1_000_000;

#[test]
fn display_stride_for_rivmap350_numbers() {
    let per_cell = crate::budget::estimate::mesh_cell_bytes();
    // The full mesh: 4 849 451 cells x 96 B = about 466 MB.
    assert_eq!(
        display_stride_for(500 * MB, RIVMAP350_CELLS, per_cell),
        Some(1)
    );
    assert_eq!(
        display_stride_for(200 * MB, RIVMAP350_CELLS, per_cell),
        Some(2)
    );
    assert_eq!(
        display_stride_for(50 * MB, RIVMAP350_CELLS, per_cell),
        Some(4)
    );
}

#[test]
fn display_stride_for_is_the_smallest_stride_that_fits() {
    let per_cell = 96;
    for allowance in [1_000, 10_000, 123_457, 4 * MB, 77 * MB, 465 * MB] {
        let Some(k) = display_stride_for(allowance, RIVMAP350_CELLS, per_cell) else {
            panic!("allowance {allowance} has room for one cell");
        };
        let k = u64::from(k);
        let bytes_at = |k: u64| RIVMAP350_CELLS.div_ceil(k * k) * per_cell;
        assert!(bytes_at(k) <= allowance, "k = {k} does not fit {allowance}");
        if k > 1 {
            assert!(
                bytes_at(k - 1) > allowance,
                "k = {k} is not the smallest stride for {allowance}"
            );
        }
    }
}

#[test]
fn display_stride_for_edge_cases() {
    // Everything fits: no degrade.
    assert_eq!(display_stride_for(u64::MAX, RIVMAP350_CELLS, 96), Some(1));
    assert_eq!(display_stride_for(0, 0, 96), Some(1));
    assert_eq!(display_stride_for(0, RIVMAP350_CELLS, 0), Some(1));
    // Not even one cell fits: no stride helps.
    assert_eq!(display_stride_for(0, RIVMAP350_CELLS, 96), None);
    assert_eq!(display_stride_for(95, RIVMAP350_CELLS, 96), None);
    // Exactly one cell fits: the stride covers the whole grid.
    let k = display_stride_for(96, RIVMAP350_CELLS, 96).map(u64::from);
    assert!(k.is_some_and(|k| RIVMAP350_CELLS.div_ceil(k * k) == 1));
}
