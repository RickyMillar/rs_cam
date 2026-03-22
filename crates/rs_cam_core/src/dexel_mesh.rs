//! Mesh extraction from a tri-dexel stock.
//!
//! Z-grid → heightmap-compatible mesh.  Side-face grids (X, Y) produce
//! additional surface meshes that are concatenated into one `HeightmapMesh`.
//! Reads the top segment exit of each ray to produce a triangle mesh with
//! wood-tone depth coloring.

use crate::dexel::{DexelAxis, DexelGrid, ray_top};
use crate::dexel_stock::TriDexelStock;
use crate::simulation::HeightmapMesh;

// Wood colors: uncut = light tan, cut = dark walnut.
const UNCUT_R: f32 = 0.76;
const UNCUT_G: f32 = 0.60;
const UNCUT_B: f32 = 0.42;
const CUT_R: f32 = 0.45;
const CUT_G: f32 = 0.25;
const CUT_B: f32 = 0.10;

/// Extract a combined mesh from all active grids of a [`TriDexelStock`].
///
/// Always includes the Z-grid surface.  If X or Y grids have been lazily
/// created (by side-face stamping), their surfaces are appended with index
/// offsets so all grids combine into one `HeightmapMesh`.
pub fn dexel_stock_to_mesh(stock: &TriDexelStock) -> HeightmapMesh {
    let mut mesh = z_grid_to_heightmap_mesh(
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

/// Convert a Z-grid to a `HeightmapMesh`.
///
/// `stock_top_z` is the original uncut stock top (for color normalization).
/// `stock_bottom_z` is the Z value used for empty (through-hole) rays.
pub fn z_grid_to_heightmap_mesh(
    grid: &DexelGrid,
    stock_top_z: f64,
    stock_bottom_z: f64,
) -> HeightmapMesh {
    let rows = grid.rows;
    let cols = grid.cols;
    let num_verts = rows * cols;

    let mut vertices = Vec::with_capacity(num_verts * 3);
    let mut colors = Vec::with_capacity(num_verts * 3);

    // Collect surface Z for every ray (top of last segment, or stock bottom).
    let mut z_vals: Vec<f64> = Vec::with_capacity(num_verts);
    for ray in &grid.rays {
        let z = match ray_top(ray) {
            Some(top) => top as f64,
            None => stock_bottom_z,
        };
        z_vals.push(z);
    }

    // Color normalization range.
    let z_min = z_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let z_range = (stock_top_z - z_min).max(1e-6);

    for row in 0..rows {
        for col in 0..cols {
            let (wu, wv) = grid.cell_to_world(row, col);
            let z = z_vals[row * cols + col];
            vertices.push(wu as f32);
            vertices.push(wv as f32);
            vertices.push(z as f32);

            let depth_t = ((stock_top_z - z) / z_range).clamp(0.0, 1.0) as f32;
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    // 2 triangles per quad, (rows-1)*(cols-1) quads.
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

    HeightmapMesh {
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
) -> HeightmapMesh {
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

    HeightmapMesh {
        vertices,
        indices,
        colors,
    }
}

/// Append `other` mesh onto `base`, offsetting indices.
fn append_mesh(base: &mut HeightmapMesh, other: &HeightmapMesh) {
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
    use crate::geo::P3;
    use crate::simulation::{Heightmap, heightmap_to_mesh, simulate_toolpath};
    use crate::tool::{FlatEndmill, MillingCutter};
    use crate::toolpath::Toolpath;

    #[test]
    fn uncut_mesh_matches_heightmap() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let hm = Heightmap::from_stock(0.0, 0.0, 4.0, 4.0, 5.0, 1.0);

        let dex_mesh = dexel_stock_to_mesh(&stock);
        let hm_mesh = heightmap_to_mesh(&hm);

        assert_eq!(dex_mesh.vertices.len(), hm_mesh.vertices.len());
        assert_eq!(dex_mesh.indices.len(), hm_mesh.indices.len());
        assert_eq!(dex_mesh.colors.len(), hm_mesh.colors.len());

        // Vertices should match.
        for i in 0..dex_mesh.vertices.len() {
            assert!(
                (dex_mesh.vertices[i] - hm_mesh.vertices[i]).abs() < 0.01,
                "vertex[{i}]: dex={}, hm={}",
                dex_mesh.vertices[i],
                hm_mesh.vertices[i]
            );
        }

        // Indices should be identical.
        assert_eq!(dex_mesh.indices, hm_mesh.indices);

        // Colors should match (all uncut).
        for i in 0..dex_mesh.colors.len() {
            assert!(
                (dex_mesh.colors[i] - hm_mesh.colors[i]).abs() < 0.01,
                "color[{i}]: dex={}, hm={}",
                dex_mesh.colors[i],
                hm_mesh.colors[i]
            );
        }
    }

    #[test]
    fn cut_mesh_matches_heightmap() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let mut stock = TriDexelStock::from_stock(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);
        let mut hm = Heightmap::from_stock(-5.0, -5.0, 15.0, 5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);
        simulate_toolpath(&tp, &tool, &mut hm);

        let dex_mesh = dexel_stock_to_mesh(&stock);
        let hm_mesh = heightmap_to_mesh(&hm);

        assert_eq!(dex_mesh.vertices.len(), hm_mesh.vertices.len());
        assert_eq!(dex_mesh.indices.len(), hm_mesh.indices.len());

        // Vertex positions should be very close.
        let mut max_z_diff: f32 = 0.0;
        for i in (0..dex_mesh.vertices.len()).step_by(3) {
            let diff = (dex_mesh.vertices[i + 2] - hm_mesh.vertices[i + 2]).abs();
            max_z_diff = max_z_diff.max(diff);
        }
        assert!(
            max_z_diff < 0.02,
            "Max Z diff between dexel and heightmap mesh: {max_z_diff:.4}"
        );
    }

    #[test]
    fn mesh_vertex_count() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        // 5x5 grid = 25 verts
        assert_eq!(mesh.vertices.len(), 75);
        assert_eq!(mesh.colors.len(), 75);
        // 4x4 quads = 16 quads × 6 indices
        assert_eq!(mesh.indices.len(), 96);
    }

    #[test]
    fn through_hole_uses_stock_bottom() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
        // Clear the center ray entirely.
        let ray = stock.z_grid.ray_mut(1, 1);
        ray.clear();

        let mesh = dexel_stock_to_mesh(&stock);
        // Vertex at (1,1) = index 4, Z at offset 4*3+2=14
        let z = mesh.vertices[14];
        assert!(
            (z - 0.0).abs() < 0.01,
            "Through-hole Z should be stock bottom, got {z}"
        );
    }

    #[test]
    fn uncut_colors_are_light_tan() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 1.0, 1.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        for i in (0..mesh.colors.len()).step_by(3) {
            assert!((mesh.colors[i] - 0.76).abs() < 0.01);
            assert!((mesh.colors[i + 1] - 0.60).abs() < 0.01);
            assert!((mesh.colors[i + 2] - 0.42).abs() < 0.01);
        }
    }

    #[test]
    fn deep_cut_colors_are_dark_walnut() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 5.0, 1.0);
        // Cut center ray to the bottom.
        ray_subtract_above(stock.z_grid.ray_mut(1, 1), 0.0);

        let mesh = dexel_stock_to_mesh(&stock);
        // Vertex (1,1) = index 4, colors at 12..15.
        let r = mesh.colors[12];
        let g = mesh.colors[13];
        let b = mesh.colors[14];
        assert!((r - 0.45).abs() < 0.01);
        assert!((g - 0.25).abs() < 0.01);
        assert!((b - 0.10).abs() < 0.01);
    }

    #[test]
    fn multi_grid_mesh_has_more_vertices() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 1.0);

        // Z-grid only mesh.
        let z_only_mesh = dexel_stock_to_mesh(&stock);

        // Stamp on Y-grid to create it.
        let tool = FlatEndmill::new(4.0, 20.0);
        let lut = crate::simulation::RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,
            5.0, // global y=5 → depth axis for Y-grid
            5.0, // global z=5 → v axis for Y-grid
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
