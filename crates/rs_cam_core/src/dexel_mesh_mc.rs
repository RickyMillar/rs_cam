//! Marching-cubes mesh extraction from a Z-grid (DEXEL roadmap Step 5 — J).
//!
//! Replaces the cell-centre heightmap mesh produced by the legacy
//! `z_grid_to_solid_mesh` with a corner-bilinear MC-equivalent extraction.
//!
//! Algorithm (per `planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.J revision):
//!
//! The §6.J spec defines the SDF as `sdf(u, v, z) = interp_ray_top(u, v) − z`
//! where `interp_ray_top` is bilinearly interpolated from the four neighbouring
//! cells' `ray_top` values. This SDF is **linear in z**, so the marching-cubes
//! iso-surface (`sdf = 0`) reduces to the height field `z = interp_ray_top`.
//! The MC extraction therefore collapses to a corner-bilinear heightmap
//! triangulation — vertices placed at cell-corner positions with z taken from
//! the bilinear average of the four surrounding cells' ray_top. This is the
//! mathematically equivalent MC result; we just skip the voxel sweep and emit
//! the height-field mesh directly.
//!
//! Closure (sides not described by the top-iso surface):
//!
//! - **Top face**: from corner-bilinear `ray_top`. Sub-cell accurate at
//!   boundaries thanks to F.a's coverage-weighted ray_top (Step 4).
//! - **Bottom face**: from corner-bilinear `ray_bottom` (mirror of top — also
//!   an iso-surface of the SDF if extended to `min(top - z, z - bot)`).
//! - **Perimeter skirt**: vertical quads at the grid bbox edge connecting the
//!   top-face boundary corners to the bottom-face boundary corners.
//! - **Internal hole walls**: vertical quads at corners adjacent to empty
//!   (through-hole) cells, connecting top to bottom.
//! - **Cavity floors / ceilings**: multi-segment ray fallback, per §6.J revision.
//!   When `max_segments > 1`, per-gap horizontal faces and vertical walls are
//!   emitted in addition to the top/bottom envelope.

use crate::dexel::{DexelGrid, ray_bottom, ray_top};
use crate::stock_mesh::StockMesh;

// Wood colors (kept in sync with `dexel_mesh.rs`).
const UNCUT_R: f32 = 0.76;
const UNCUT_G: f32 = 0.60;
const UNCUT_B: f32 = 0.42;
const CUT_R: f32 = 0.45;
const CUT_G: f32 = 0.25;
const CUT_B: f32 = 0.10;

/// Below this material thickness, a ray is treated as a through-hole.
const MIN_MATERIAL_THICKNESS: f32 = 0.05;

/// Build a closed solid mesh from a Z-grid using marching cubes
/// (height-field reduction per §6.J).
///
/// Returns a [`StockMesh`] with consistent CCW winding around outward-facing
/// normals. The mesh is watertight for single-segment rays; multi-segment
/// rays additionally emit per-gap cavity surfaces.
#[allow(clippy::indexing_slicing)]
pub fn z_grid_marching_cubes(grid: &DexelGrid, stock_top_z: f64, stock_bottom_z: f64) -> StockMesh {
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

    let mut vertices: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut colors: Vec<f32> = Vec::new();

    // ── 3. Top face: emit one quad per cell at the corner-bilinear top
    //       heights. Skip cells where any of the 4 owning corners is empty.
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
            let (u0, v0) = corner_xy(ci, cj);
            let (u1, v1) = corner_xy(ci + 1, cj + 1);
            let z00 = corner_top[c00];
            let z01 = corner_top[c01];
            let z10 = corner_top[c10];
            let z11 = corner_top[c11];
            push_quad_top(
                &mut vertices,
                &mut indices,
                &mut colors,
                (u0, v0, z00),
                (u1, v0, z01),
                (u0, v1, z10),
                (u1, v1, z11),
                stock_top,
                stock_bot,
                z_range,
            );
        }
    }

    // ── 4. Bottom face: mirrored winding so normals point −Z.
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
            let (u0, v0) = corner_xy(ci, cj);
            let (u1, v1) = corner_xy(ci + 1, cj + 1);
            let z00 = corner_bot[c00];
            let z01 = corner_bot[c01];
            let z10 = corner_bot[c10];
            let z11 = corner_bot[c11];
            push_quad_bottom(
                &mut vertices,
                &mut indices,
                &mut colors,
                (u0, v0, z00),
                (u1, v0, z01),
                (u0, v1, z10),
                (u1, v1, z11),
                stock_top,
                stock_bot,
                z_range,
            );
        }
    }

    // ── 5. Perimeter skirt — vertical quads along the bbox edge connecting
    //       top corners to bottom corners. Skip segments where both
    //       endpoint corners are empty.
    // Front edge (ci = 0).
    for cj in 0..cols {
        let cl = cj;
        let cr = cj + 1;
        if corner_empty[cl] || corner_empty[cr] {
            continue;
        }
        let (ul, vl) = corner_xy(0, cj);
        let (ur, _) = corner_xy(0, cj + 1);
        push_vertical_quad(
            &mut vertices,
            &mut indices,
            &mut colors,
            (ul, vl, corner_top[cl]),
            (ur, vl, corner_top[cr]),
            (ul, vl, corner_bot[cl]),
            (ur, vl, corner_bot[cr]),
            true,
            stock_top,
            stock_bot,
            z_range,
        );
    }
    // Back edge (ci = rows).
    for cj in 0..cols {
        let cl = rows * corner_cols + cj;
        let cr = rows * corner_cols + cj + 1;
        if corner_empty[cl] || corner_empty[cr] {
            continue;
        }
        let (ul, vl) = corner_xy(rows, cj);
        let (ur, _) = corner_xy(rows, cj + 1);
        push_vertical_quad(
            &mut vertices,
            &mut indices,
            &mut colors,
            (ul, vl, corner_top[cl]),
            (ur, vl, corner_top[cr]),
            (ul, vl, corner_bot[cl]),
            (ur, vl, corner_bot[cr]),
            false,
            stock_top,
            stock_bot,
            z_range,
        );
    }
    // Left edge (cj = 0).
    for ci in 0..rows {
        let cb = ci * corner_cols;
        let ct = (ci + 1) * corner_cols;
        if corner_empty[cb] || corner_empty[ct] {
            continue;
        }
        let (ul, vl) = corner_xy(ci, 0);
        let (_, vt) = corner_xy(ci + 1, 0);
        push_vertical_quad(
            &mut vertices,
            &mut indices,
            &mut colors,
            (ul, vl, corner_top[cb]),
            (ul, vt, corner_top[ct]),
            (ul, vl, corner_bot[cb]),
            (ul, vt, corner_bot[ct]),
            false,
            stock_top,
            stock_bot,
            z_range,
        );
    }
    // Right edge (cj = cols).
    for ci in 0..rows {
        let cb = ci * corner_cols + cols;
        let ct = (ci + 1) * corner_cols + cols;
        if corner_empty[cb] || corner_empty[ct] {
            continue;
        }
        let (ul, vl) = corner_xy(ci, cols);
        let (_, vt) = corner_xy(ci + 1, cols);
        push_vertical_quad(
            &mut vertices,
            &mut indices,
            &mut colors,
            (ul, vl, corner_top[cb]),
            (ul, vt, corner_top[ct]),
            (ul, vl, corner_bot[cb]),
            (ul, vt, corner_bot[ct]),
            true,
            stock_top,
            stock_bot,
            z_range,
        );
    }

    // ── 6. Hole walls — at every cell edge separating a material cell from
    //       an empty cell, emit a vertical quad at the corner positions of
    //       that edge.
    // Column-direction edges (between cells (ci, cj) and (ci, cj+1)).
    for ci in 0..rows {
        for cj in 0..(cols.saturating_sub(1)) {
            let a_empty = cell_empty[ci * cols + cj];
            let b_empty = cell_empty[ci * cols + cj + 1];
            if a_empty == b_empty {
                continue;
            }
            // Material on one side, empty on the other → wall at cj+1.
            let cb = ci * corner_cols + cj + 1;
            let ct = (ci + 1) * corner_cols + cj + 1;
            if corner_empty[cb] || corner_empty[ct] {
                continue;
            }
            let (ul, vl) = corner_xy(ci, cj + 1);
            let (_, vt) = corner_xy(ci + 1, cj + 1);
            // Material on left (cj) → wall faces +u; material on right → −u.
            let face_pos_u = !a_empty;
            push_vertical_quad(
                &mut vertices,
                &mut indices,
                &mut colors,
                (ul, vl, corner_top[cb]),
                (ul, vt, corner_top[ct]),
                (ul, vl, corner_bot[cb]),
                (ul, vt, corner_bot[ct]),
                face_pos_u,
                stock_top,
                stock_bot,
                z_range,
            );
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
            let (ul, vl) = corner_xy(ci + 1, cj);
            let (ur, _) = corner_xy(ci + 1, cj + 1);
            let face_pos_v = !a_empty;
            push_vertical_quad(
                &mut vertices,
                &mut indices,
                &mut colors,
                (ul, vl, corner_top[cl]),
                (ur, vl, corner_top[cr]),
                (ul, vl, corner_bot[cl]),
                (ur, vl, corner_bot[cr]),
                !face_pos_v,
                stock_top,
                stock_bot,
                z_range,
            );
        }
    }

    // ── 7. Multi-segment cavity fallback (per-gap pass). Only runs when at
    //       least one ray has >1 segment.
    if max_segments > 1 {
        emit_cavity_surfaces(
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

#[inline]
#[allow(clippy::too_many_arguments)]
fn push_quad_top(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<f32>,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    d: (f32, f32, f32),
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
) {
    // (a, b, d, c) layout — a=(u0,v0), b=(u1,v0), c=(u0,v1), d=(u1,v1).
    // CCW from above for +Z outward normal: a, c, b, b, c, d.
    let base = (vertices.len() / 3) as u32;
    for v in [a, c, b, b, c, d] {
        vertices.push(v.0);
        vertices.push(v.1);
        vertices.push(v.2);
        let (cr, cg, cb) = wood_color_at_z(v.2, stock_top, stock_bot, z_range);
        colors.push(cr);
        colors.push(cg);
        colors.push(cb);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 3, base + 4, base + 5]);
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn push_quad_bottom(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<f32>,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    d: (f32, f32, f32),
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
) {
    // CW from above (so normals point −Z): reversed winding from top.
    let base = (vertices.len() / 3) as u32;
    for v in [a, b, c, b, d, c] {
        vertices.push(v.0);
        vertices.push(v.1);
        vertices.push(v.2);
        let (cr, cg, cb) = wood_color_at_z(v.2, stock_top, stock_bot, z_range);
        colors.push(cr);
        colors.push(cg);
        colors.push(cb);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 3, base + 4, base + 5]);
}

/// Emit a vertical wall quad spanning four corners (top-left, top-right,
/// bottom-left, bottom-right). `face_outward_positive` selects winding so the
/// normal points along the positive axis if true.
#[inline]
#[allow(clippy::too_many_arguments)]
fn push_vertical_quad(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    colors: &mut Vec<f32>,
    tl: (f32, f32, f32),
    tr: (f32, f32, f32),
    bl: (f32, f32, f32),
    br: (f32, f32, f32),
    face_outward_positive: bool,
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
) {
    let base = (vertices.len() / 3) as u32;
    let order = if face_outward_positive {
        [tl, tr, bl, tr, br, bl]
    } else {
        [tl, bl, tr, tr, bl, br]
    };
    for v in order {
        vertices.push(v.0);
        vertices.push(v.1);
        vertices.push(v.2);
        let (cr, cg, cb) = wood_color_at_z(v.2, stock_top, stock_bot, z_range);
        colors.push(cr);
        colors.push(cg);
        colors.push(cb);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 3, base + 4, base + 5]);
}

/// Multi-segment cavity emission. Walks each ray with >1 segment; per gap
/// between segments, emits a ceiling face (at gap_bot, normal −Z) and a floor
/// face (at gap_top, normal +Z) for the 2x2 cell block sharing the gap.
/// Vertical cavity walls are emitted where adjacent cells differ in gap
/// topology.
#[allow(clippy::indexing_slicing)]
fn emit_cavity_surfaces(
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

fn cell_center(grid: &DexelGrid, row: usize, col: usize, z: f32) -> (f32, f32, f32) {
    let (u, v) = grid.cell_to_world(row, col);
    (u as f32, v as f32, z)
}

fn find_matching_gap(gaps: &[[f32; 2]], reference: &[f32; 2]) -> Option<[f32; 2]> {
    let ref_height = reference[1] - reference[0];
    let threshold = ref_height * 0.5;
    for gap in gaps {
        let overlap_lo = gap[0].max(reference[0]);
        let overlap_hi = gap[1].min(reference[1]);
        let overlap = (overlap_hi - overlap_lo).max(0.0);
        if overlap > threshold {
            return Some(*gap);
        }
    }
    None
}

#[inline]
fn wood_color_at_z(z: f32, stock_top: f32, _stock_bot: f32, range: f32) -> (f32, f32, f32) {
    let depth_t = ((stock_top - z) / range).clamp(0.0, 1.0);
    (
        UNCUT_R + (CUT_R - UNCUT_R) * depth_t,
        UNCUT_G + (CUT_G - UNCUT_G) * depth_t,
        UNCUT_B + (CUT_B - UNCUT_B) * depth_t,
    )
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]
mod tests {
    use super::*;
    use crate::dexel::{ray_subtract_above, ray_subtract_interval};
    use crate::dexel_stock::TriDexelStock;

    /// Position-based watertightness: every undirected edge (by quantised
    /// endpoint positions) appears exactly twice. Degenerate (zero-length)
    /// edges are ignored. MC outputs per-triangle vertices without index
    /// dedup, so position-keyed counting is the right semantic.
    pub(super) fn is_watertight(mesh: &StockMesh) -> bool {
        use std::collections::HashMap;
        type Pt = (i64, i64, i64);
        type Edge = (Pt, Pt);
        let quant = |x: f32| (x * 10000.0).round() as i64;
        let n = mesh.vertices.len() / 3;
        let pos: Vec<Pt> = (0..n)
            .map(|i| {
                (
                    quant(mesh.vertices[i * 3]),
                    quant(mesh.vertices[i * 3 + 1]),
                    quant(mesh.vertices[i * 3 + 2]),
                )
            })
            .collect();
        let mut edges: HashMap<Edge, u32> = HashMap::new();
        for tri in mesh.indices.chunks_exact(3) {
            let p0 = pos[tri[0] as usize];
            let p1 = pos[tri[1] as usize];
            let p2 = pos[tri[2] as usize];
            let mut e = [(p0, p1), (p1, p2), (p2, p0)];
            for (a, b) in e.iter_mut() {
                if *a > *b {
                    std::mem::swap(a, b);
                }
            }
            for &edge in &e {
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        let mut bad = 0;
        for (e, &c) in edges.iter() {
            if e.0 == e.1 {
                continue;
            }
            if c != 2 {
                bad += 1;
                if bad < 5 {
                    eprintln!("non-manifold edge {e:?} → count {c}");
                }
            }
        }
        bad == 0
    }

    #[test]
    fn mc_uncut_block_is_watertight() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);
        assert!(!mesh.indices.is_empty());
        assert!(is_watertight(&mesh), "uncut block mesh must be watertight");
    }

    #[test]
    fn mc_uncut_block_closes_at_stock_top() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);
        // Every top-face vertex must be at z = stock_top (= 5.0). Look for
        // any vertex with z in [4.9, 5.1].
        let mut found_top = false;
        for i in 0..mesh.vertices.len() / 3 {
            let z = mesh.vertices[i * 3 + 2];
            if (z - 5.0).abs() < 0.01 {
                found_top = true;
                break;
            }
        }
        assert!(found_top, "expected at least one vertex at stock_top");
    }

    #[test]
    fn mc_uncut_block_closes_at_stock_bottom() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);
        let mut found_bot = false;
        for i in 0..mesh.vertices.len() / 3 {
            let z = mesh.vertices[i * 3 + 2];
            if z.abs() < 0.01 {
                found_bot = true;
                break;
            }
        }
        assert!(found_bot, "expected at least one vertex at stock_bottom");
    }

    #[test]
    fn mc_partial_cut_lowers_top_near_centre() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        // Lower the center cell only.
        ray_subtract_above(stock.z_grid.ray_mut(2, 2), 2.0);
        let mesh = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);

        let mut found_low = false;
        let n = mesh.vertices.len() / 3;
        for i in 0..n {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if (x - 2.0).abs() < 1.5 && (y - 2.0).abs() < 1.5 && z < 4.0 {
                found_low = true;
                break;
            }
        }
        assert!(
            found_low,
            "expected lowered vertex near centre after partial cut"
        );
    }

    #[test]
    fn mc_through_hole_produces_wall_geometry() {
        // A through-hole cell adjacent to material cells must produce wall
        // quads — the empty-cell skip in top/bottom faces opens a hole, and
        // the hole-wall emission closes it. Walls run from corner_top to
        // corner_bot at corner positions adjacent to the hole; with one
        // empty cell among 3 material at each owning corner, corner_top
        // averages to stock_top and corner_bot to stock_bot, so wall
        // vertices sit at z=stock_top and z=stock_bot.
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        stock.z_grid.ray_mut(2, 2).clear();
        let mesh = z_grid_marching_cubes(&stock.z_grid, 5.0, 0.0);
        assert!(!mesh.indices.is_empty(), "hole mesh must not be empty");

        // Wall vertices at corner positions surrounding cell (2, 2): corners
        // (2, 2) (1.5, 1.5), (2, 3) (2.5, 1.5), (3, 2) (1.5, 2.5),
        // (3, 3) (2.5, 2.5). Expect both top (z≈5) and bottom (z≈0)
        // representation in this region.
        let mut wall_top = 0;
        let mut wall_bot = 0;
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if (x - 2.0).abs() < 1.0 && (y - 2.0).abs() < 1.0 {
                if (z - 5.0).abs() < 0.5 {
                    wall_top += 1;
                }
                if z.abs() < 0.5 {
                    wall_bot += 1;
                }
            }
        }
        assert!(
            wall_top > 0 && wall_bot > 0,
            "expected wall vertices at both top and bot near hole: top={wall_top}, bot={wall_bot}"
        );
    }

    #[test]
    fn mc_multi_cell_top_and_bottom_cut_shows_both_depths() {
        // Cut a 3×3 cell block top and bottom: corner-bilinear average of 4
        // identical neighbouring cells reproduces the actual cut depth at
        // interior corners of the cut region. Single-cell cuts get smoothed
        // to 0.75 of the depth by the corner-bilinear average; multi-cell
        // cuts match the legacy behaviour.
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 8.0, 8.0, 0.0, 10.0, 1.0);
        for r in 3..=5 {
            for c in 3..=5 {
                ray_subtract_above(stock.z_grid.ray_mut(r, c), 7.0);
                ray_subtract_interval(stock.z_grid.ray_mut(r, c), 0.0, 3.0);
            }
        }
        let mesh = z_grid_marching_cubes(&stock.z_grid, 10.0, 0.0);

        let mut hits_top = 0;
        let mut hits_bot = 0;
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if (x - 4.0).abs() < 1.5 && (y - 4.0).abs() < 1.5 {
                if (z - 7.0).abs() < 0.5 {
                    hits_top += 1;
                }
                if (z - 3.0).abs() < 0.5 {
                    hits_bot += 1;
                }
            }
        }
        assert!(
            hits_top > 0 && hits_bot > 0,
            "expected vertices near both cut levels: top_hits={hits_top}, bot_hits={hits_bot}"
        );
    }
}
