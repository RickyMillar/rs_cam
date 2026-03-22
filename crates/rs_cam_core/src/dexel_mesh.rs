//! Mesh extraction from a tri-dexel stock.
//!
//! The Z-grid produces a **closed solid mesh** — top face, bottom face, and
//! perimeter skirt walls — so the simulation looks like a block of material
//! being progressively carved.  Side-face grids (X, Y) produce additional
//! surface meshes appended with index offsets.

use crate::dexel::{DexelAxis, DexelGrid, ray_bottom, ray_top};
use crate::dexel_stock::TriDexelStock;
use crate::stock_mesh::StockMesh;

// Wood colors: uncut = light tan, cut = dark walnut.
const UNCUT_R: f32 = 0.76;
const UNCUT_G: f32 = 0.60;
const UNCUT_B: f32 = 0.42;
const CUT_R: f32 = 0.45;
const CUT_G: f32 = 0.25;
const CUT_B: f32 = 0.10;

/// Extract a combined mesh from all active grids of a [`TriDexelStock`].
pub fn dexel_stock_to_mesh(stock: &TriDexelStock) -> StockMesh {
    let mut mesh = z_grid_to_solid_mesh(
        &stock.z_grid,
        stock.stock_bbox.max.z,
        stock.stock_bbox.min.z,
    );

    if let Some(y_grid) = &stock.y_grid {
        let y_mesh = side_grid_to_mesh(y_grid, stock.stock_bbox.max.y, stock.stock_bbox.min.y);
        append_mesh(&mut mesh, &y_mesh);
    }

    if let Some(x_grid) = &stock.x_grid {
        let x_mesh = side_grid_to_mesh(x_grid, stock.stock_bbox.max.x, stock.stock_bbox.min.x);
        append_mesh(&mut mesh, &x_mesh);
    }

    mesh
}

/// Build a closed solid mesh from a Z-grid.
///
/// The solid has three parts:
/// 1. **Top face** — one vertex per cell at `(x, y, ray_top)`, CCW winding.
/// 2. **Bottom face** — one vertex per cell at `(x, y, ray_bottom)`, CW winding.
/// 3. **Perimeter skirt** — vertical quads connecting top/bottom edges around
///    the grid boundary so the solid is closed on the sides.
///
/// Empty rays (through-holes) collapse both top and bottom to `stock_bottom_z`,
/// producing degenerate zero-area triangles.
pub fn z_grid_to_solid_mesh(
    grid: &DexelGrid,
    stock_top_z: f64,
    stock_bottom_z: f64,
) -> StockMesh {
    let rows = grid.rows;
    let cols = grid.cols;
    let cells = rows * cols;

    // Collect per-cell top and bottom Z values.
    let mut top_z = Vec::with_capacity(cells);
    let mut bot_z = Vec::with_capacity(cells);
    for ray in &grid.rays {
        top_z.push(ray_top(ray).map_or(stock_bottom_z as f32, |t| t));
        bot_z.push(ray_bottom(ray).map_or(stock_bottom_z as f32, |b| b));
    }

    // Color normalization ranges.
    let top_z_min = top_z.iter().copied().fold(f32::INFINITY, f32::min);
    let top_range = ((stock_top_z as f32) - top_z_min).max(1e-6);
    let bot_z_max = bot_z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let bot_range = (bot_z_max - stock_bottom_z as f32).max(1e-6);

    // ── Vertices: top layer [0..cells), bottom layer [cells..2*cells) ──
    let total_verts = 2 * cells;
    let mut vertices = Vec::with_capacity(total_verts * 3);
    let mut colors = Vec::with_capacity(total_verts * 3);

    // Top vertices.
    for row in 0..rows {
        for col in 0..cols {
            let (wu, wv) = grid.cell_to_world(row, col);
            let z = top_z[row * cols + col];
            vertices.push(wu as f32);
            vertices.push(wv as f32);
            vertices.push(z);

            let depth_t = ((stock_top_z as f32 - z) / top_range).clamp(0.0, 1.0);
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    // Bottom vertices.
    for row in 0..rows {
        for col in 0..cols {
            let (wu, wv) = grid.cell_to_world(row, col);
            let z = bot_z[row * cols + col];
            vertices.push(wu as f32);
            vertices.push(wv as f32);
            vertices.push(z);

            let depth_t = ((z - stock_bottom_z as f32) / bot_range).clamp(0.0, 1.0);
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    let top_quads = (rows - 1) * (cols - 1);
    let wall_quads = 2 * ((rows - 1) + (cols - 1));
    let total_tris = 2 * top_quads   // top face
                   + 2 * top_quads   // bottom face
                   + 2 * wall_quads; // perimeter skirt
    let mut indices = Vec::with_capacity(total_tris * 3);

    let bot_off = cells as u32; // index offset to bottom layer

    // ── Top face (CCW, normals face +Z) ────────────────────────────────
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let tl = (row * cols + col) as u32;
            let tr = tl + 1;
            let bl = ((row + 1) * cols + col) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    // ── Bottom face (CW, normals face −Z) ──────────────────────────────
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let tl = bot_off + (row * cols + col) as u32;
            let tr = tl + 1;
            let bl = bot_off + ((row + 1) * cols + col) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, tr, bl, tr, br, bl]);
        }
    }

    // ── Perimeter skirt walls ──────────────────────────────────────────
    // Each wall quad connects a top edge vertex to its bottom counterpart.
    // Winding is chosen so normals face outward.

    // Front edge (row = 0, normals face −V).
    for col in 0..(cols - 1) {
        let t0 = col as u32;
        let t1 = (col + 1) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
    }

    // Back edge (row = rows-1, normals face +V).
    let last_row = rows - 1;
    for col in 0..(cols - 1) {
        let t0 = (last_row * cols + col) as u32;
        let t1 = t0 + 1;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
    }

    // Left edge (col = 0, normals face −U).
    for row in 0..(rows - 1) {
        let t0 = (row * cols) as u32;
        let t1 = ((row + 1) * cols) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
    }

    // Right edge (col = cols-1, normals face +U).
    let last_col = cols - 1;
    for row in 0..(rows - 1) {
        let t0 = (row * cols + last_col) as u32;
        let t1 = ((row + 1) * cols + last_col) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
    }

    StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// Extract a heightmap-style mesh from a side-face grid (X or Y).
///
/// Uses `ray_top()` per cell, analogous to the Z-grid extraction.
/// Vertex positions are mapped back to (x, y, z) world coordinates:
/// - Y-grid (u=X, v=Z, depth=Y): vertex = (u, ray_top, v)
/// - X-grid (u=Y, v=Z, depth=X): vertex = (ray_top, u, v)
fn side_grid_to_mesh(
    grid: &DexelGrid,
    stock_top_depth: f64,
    stock_bottom_depth: f64,
) -> StockMesh {
    let rows = grid.rows;
    let cols = grid.cols;
    let num_verts = rows * cols;

    let mut vertices = Vec::with_capacity(num_verts * 3);
    let mut colors = Vec::with_capacity(num_verts * 3);

    let mut depth_vals: Vec<f64> = Vec::with_capacity(num_verts);
    for ray in &grid.rays {
        let d = match ray_top(ray) {
            Some(top) => top as f64,
            None => stock_bottom_depth,
        };
        depth_vals.push(d);
    }

    let d_min = depth_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let d_range = (stock_top_depth - d_min).max(1e-6);

    for row in 0..rows {
        for col in 0..cols {
            let (wu, wv) = grid.cell_to_world(row, col);
            let d = depth_vals[row * cols + col];

            // Map (u, v, depth) back to world (x, y, z).
            let (x, y, z) = match grid.axis {
                DexelAxis::Y => (wu, d, wv), // u=X, depth=Y, v=Z
                DexelAxis::X => (d, wu, wv), // depth=X, u=Y, v=Z
                DexelAxis::Z => (wu, wv, d), // fallback (shouldn't happen)
            };
            vertices.push(x as f32);
            vertices.push(y as f32);
            vertices.push(z as f32);

            let depth_t = ((stock_top_depth - d) / d_range).clamp(0.0, 1.0) as f32;
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    let num_quads = (rows - 1) * (cols - 1);
    let mut indices = Vec::with_capacity(num_quads * 6);

    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let tl = (row * cols + col) as u32;
            let tr = tl + 1;
            let bl = ((row + 1) * cols + col) as u32;
            let br = bl + 1;
            indices.push(tl);
            indices.push(bl);
            indices.push(tr);
            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// Append `other` mesh onto `base`, offsetting indices.
fn append_mesh(base: &mut StockMesh, other: &StockMesh) {
    let index_offset = (base.vertices.len() / 3) as u32;
    base.vertices.extend_from_slice(&other.vertices);
    base.colors.extend_from_slice(&other.colors);
    base.indices
        .extend(other.indices.iter().map(|i| i + index_offset));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dexel::ray_subtract_above;
    use crate::dexel_stock::{StockCutDirection, TriDexelStock};
    use crate::tool::{FlatEndmill, MillingCutter};

    #[test]
    fn solid_mesh_vertex_count() {
        // 5×5 grid (stock 4×4 at cell_size 1.0) = 25 cells.
        // Solid: 25 top + 25 bottom = 50 vertices.
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        assert_eq!(mesh.vertices.len() / 3, 50);
        assert_eq!(mesh.colors.len() / 3, 50);

        // Top face: 4×4 quads × 2 tris = 32 tris
        // Bottom face: same = 32 tris
        // Perimeter: 2×(4+4) = 16 quads × 2 tris = 32 tris
        // Total: 96 tris × 3 = 288 indices
        assert_eq!(mesh.indices.len(), 288);
    }

    #[test]
    fn uncut_solid_top_at_stock_top() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);

        // Top vertex at (0,0) = index 0, Z at offset 2.
        let top_z = mesh.vertices[2];
        assert!(
            (top_z - 5.0).abs() < 0.01,
            "Top vertex Z should be stock top"
        );

        // Bottom vertex at (0,0) = index 25 (rows*cols), Z at offset 25*3+2.
        let rows = stock.z_grid.rows;
        let cols = stock.z_grid.cols;
        let bot_z = mesh.vertices[(rows * cols) * 3 + 2];
        assert!(
            (bot_z - 0.0).abs() < 0.01,
            "Bottom vertex Z should be stock bottom"
        );
    }

    #[test]
    fn through_hole_collapses_to_stock_bottom() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
        // Clear the center ray entirely.
        stock.z_grid.ray_mut(1, 1).clear();

        let mesh = dexel_stock_to_mesh(&stock);
        let cols = stock.z_grid.cols;
        // Top vertex at (1,1) = index 1*3+1 = 4, Z at 4*3+2 = 14.
        let top_z = mesh.vertices[14];
        // Bottom vertex at (1,1) = index (3*3)+4 = 13, Z at 13*3+2 = 41.
        let bot_idx = (stock.z_grid.rows * cols) + cols + 1;
        let bot_z = mesh.vertices[bot_idx * 3 + 2];
        assert!(
            (top_z - 0.0).abs() < 0.01,
            "Through-hole top Z should collapse to stock bottom, got {top_z}"
        );
        assert!(
            (bot_z - 0.0).abs() < 0.01,
            "Through-hole bottom Z should collapse to stock bottom, got {bot_z}"
        );
    }

    #[test]
    fn uncut_colors_are_light_tan() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 1.0, 1.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        let cells = stock.z_grid.rows * stock.z_grid.cols;
        // Top vertices (first `cells` verts) should all be uncut (light tan).
        for i in 0..cells {
            assert!(
                (mesh.colors[i * 3] - UNCUT_R).abs() < 0.01,
                "Top vertex {i} R"
            );
            assert!(
                (mesh.colors[i * 3 + 1] - UNCUT_G).abs() < 0.01,
                "Top vertex {i} G"
            );
            assert!(
                (mesh.colors[i * 3 + 2] - UNCUT_B).abs() < 0.01,
                "Top vertex {i} B"
            );
        }
    }

    #[test]
    fn deep_cut_colors_are_dark_walnut() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 5.0, 1.0);
        // Cut center ray to the bottom.
        ray_subtract_above(stock.z_grid.ray_mut(1, 1), 0.0);

        let mesh = dexel_stock_to_mesh(&stock);
        // Top vertex (1,1) = index 4, colors at 12..15.
        let r = mesh.colors[12];
        let g = mesh.colors[13];
        let b = mesh.colors[14];
        assert!((r - CUT_R).abs() < 0.01);
        assert!((g - CUT_G).abs() < 0.01);
        assert!((b - CUT_B).abs() < 0.01);
    }

    /// Top + Bottom two-setup simulation: the solid mesh must have both
    /// top-surface vertices (from ray_top) and bottom-surface vertices
    /// (from ray_bottom) reflecting cuts from both directions.
    #[test]
    fn top_bottom_job_mesh_shows_both_cuts() {
        use crate::dexel::{ray_bottom, ray_top};
        use crate::radial_profile::RadialProfileLUT;

        let stock_h = 10.6;
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 110.0, 110.0, 0.0, stock_h, 1.0);

        let tool = FlatEndmill::new(6.35, 25.0);
        let lut = RadialProfileLUT::from_cutter(&tool, 256);

        // Top cut: ray_top → 7.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            55.0,
            55.0,
            7.0,
            StockCutDirection::FromTop,
        );
        // Bottom cut: ray_bottom → 3.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            55.0,
            55.0,
            3.0,
            StockCutDirection::FromBottom,
        );

        let (row, col) = stock.z_grid.world_to_cell(55.0, 55.0).unwrap();
        let ray = stock.z_grid.ray(row, col);
        assert!((ray_top(ray).unwrap() - 7.0).abs() < 0.1);
        assert!((ray_bottom(ray).unwrap() - 3.0).abs() < 0.1);

        let mesh = dexel_stock_to_mesh(&stock);
        let cols = stock.z_grid.cols;
        let cells = stock.z_grid.rows * cols;

        // Top vertex at (row, col).
        let top_idx = row * cols + col;
        let top_z = mesh.vertices[top_idx * 3 + 2];
        assert!(
            (top_z - 7.0).abs() < 0.1,
            "Top surface Z should be ~7, got {top_z}"
        );

        // Bottom vertex at (row, col).
        let bot_idx = cells + row * cols + col;
        let bot_z = mesh.vertices[bot_idx * 3 + 2];
        assert!(
            (bot_z - 3.0).abs() < 0.1,
            "Bottom surface Z should be ~3, got {bot_z}"
        );
    }

    #[test]
    fn multi_grid_mesh_has_more_vertices() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 1.0);
        let z_only_mesh = dexel_stock_to_mesh(&stock);

        let tool = FlatEndmill::new(4.0, 20.0);
        let lut = crate::radial_profile::RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,
            5.0,
            5.0,
            StockCutDirection::FromBack,
        );

        let multi_mesh = dexel_stock_to_mesh(&stock);
        assert!(
            multi_mesh.vertices.len() > z_only_mesh.vertices.len(),
            "Multi-grid mesh ({} verts) should have more vertices than Z-only ({} verts)",
            multi_mesh.vertices.len() / 3,
            z_only_mesh.vertices.len() / 3,
        );
    }
}
