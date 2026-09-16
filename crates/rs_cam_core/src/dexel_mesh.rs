//! Mesh extraction from a tri-dexel stock.
//!
//! The Z-grid produces a **closed solid mesh** — top face, bottom face,
//! perimeter skirt walls, and **internal cavity surfaces** — so the simulation
//! looks like a block of material being progressively carved, with proper
//! through-cuts and multi-segment voids.
//!
//! Side-face grids (X, Y) produce per-segment surface meshes appended with
//! index offsets.

use crate::dexel::{DexelAxis, DexelGrid, DexelSegment, ray_bottom, ray_top};
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::stock_mesh::StockMesh;

// Wood colors: uncut = light tan, cut = dark walnut. This is their one
// home; `dexel_mesh_mc` reads them from here.
pub(crate) const UNCUT_R: f32 = 0.76;
pub(crate) const UNCUT_G: f32 = 0.60;
pub(crate) const UNCUT_B: f32 = 0.42;
pub(crate) const CUT_R: f32 = 0.45;
pub(crate) const CUT_G: f32 = 0.25;
pub(crate) const CUT_B: f32 = 0.10;

/// Extract a fast preview mesh from the active tool-entry side.
///
/// This intentionally skips bottom faces, side walls, and internal cavity
/// surfaces. It is meant for interactive playback where smooth frame cadence
/// matters more than a fully closed solid. The final/pause mesh still uses
/// [`dexel_stock_to_mesh`]. Direction matters for multi-setup playback: a
/// bottom setup previews the Z-grid bottom surface, and side setups preview
/// the appropriate X/Y side grid.
#[allow(clippy::indexing_slicing)] // grid indexing bounded by row/col loops
pub fn dexel_stock_to_entry_surface_mesh(
    stock: &TriDexelStock,
    direction: StockCutDirection,
) -> StockMesh {
    let Some(grid) = preview_grid_for_direction(stock, direction) else {
        return StockMesh::empty();
    };
    let rows = grid.rows;
    let cols = grid.cols;
    if rows < 2 || cols < 2 {
        return StockMesh::empty();
    }

    let cells = rows * cols;
    let from_high = direction.cuts_from_high_side();
    let (axis_min, axis_max) = axis_depth_bounds(stock, direction.grid_axis());
    let fallback_depth = if from_high { axis_max } else { axis_min };
    let axis_range = (axis_max - axis_min).max(1e-6);

    let mut vertices = Vec::with_capacity(cells * 3);
    let mut colors = Vec::with_capacity(cells * 3);
    let mut depths = Vec::with_capacity(cells);

    for ray in &grid.rays {
        let depth = if from_high {
            ray_top(ray)
        } else {
            ray_bottom(ray)
        }
        .unwrap_or(fallback_depth);
        depths.push(depth);
    }

    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let (u, v) = grid.cell_to_world(row, col);
            let depth = depths[idx];
            let (x, y, z) = preview_vertex_from_grid(grid.axis, u as f32, v as f32, depth);
            vertices.push(x);
            vertices.push(y);
            vertices.push(z);

            let depth_t = if from_high {
                ((axis_max - depth) / axis_range).clamp(0.0, 1.0)
            } else {
                ((depth - axis_min) / axis_range).clamp(0.0, 1.0)
            };
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    let mut indices = Vec::with_capacity((rows - 1) * (cols - 1) * 6);
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let tl = (row * cols + col) as u32;
            let tr = tl + 1;
            let bl = ((row + 1) * cols + col) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// Backwards-compatible helper for callers that specifically want the top
/// surface. Prefer [`dexel_stock_to_entry_surface_mesh`] for playback.
///
/// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
/// only callers. No production path reads it.
pub fn dexel_stock_to_top_surface_mesh(stock: &TriDexelStock) -> StockMesh {
    dexel_stock_to_entry_surface_mesh(stock, StockCutDirection::FromTop)
}

fn preview_grid_for_direction(
    stock: &TriDexelStock,
    direction: StockCutDirection,
) -> Option<&DexelGrid> {
    match direction.grid_axis() {
        DexelAxis::Z => Some(&stock.z_grid),
        DexelAxis::Y => stock.y_grid.as_ref(),
        DexelAxis::X => stock.x_grid.as_ref(),
    }
}

fn axis_depth_bounds(stock: &TriDexelStock, axis: DexelAxis) -> (f32, f32) {
    match axis {
        DexelAxis::X => (stock.stock_bbox.min.x as f32, stock.stock_bbox.max.x as f32),
        DexelAxis::Y => (stock.stock_bbox.min.y as f32, stock.stock_bbox.max.y as f32),
        DexelAxis::Z => (stock.stock_bbox.min.z as f32, stock.stock_bbox.max.z as f32),
    }
}

fn preview_vertex_from_grid(axis: DexelAxis, u: f32, v: f32, depth: f32) -> (f32, f32, f32) {
    match axis {
        DexelAxis::Z => (u, v, depth),
        // Y-grid stores u=X, v=Z, depth=Y.
        DexelAxis::Y => (u, depth, v),
        // X-grid stores u=Y, v=Z, depth=X.
        DexelAxis::X => (depth, u, v),
    }
}

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

/// Build a closed solid mesh from a Z-grid via marching cubes
/// (DEXEL roadmap Step 5 — J, see `planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.J).
///
/// Delegates to [`crate::dexel_mesh_mc::z_grid_marching_cubes`]. The MC path
/// is watertight, topology-aware, and composes single-segment, multi-segment,
/// cavity, through-hole, and dual-direction (top + bottom) cuts uniformly via
/// a per-cell SDF derived from the ray data. Replaces the prior heightmap-
/// style six-part decomposition (top/bottom faces + perimeter skirt + hole
/// walls + cavity floors/ceilings + cavity walls) with a single MC pass.
///
/// The MC mesh has consistent CCW winding around outward-facing normals,
/// matching the renderer's CPU-side normal computation in
/// `crates/rs_cam_viz/src/render/sim_render.rs::from_heightmap_mesh`.
pub fn z_grid_to_solid_mesh(grid: &DexelGrid, stock_top_z: f64, stock_bottom_z: f64) -> StockMesh {
    crate::dexel_mesh_mc::z_grid_marching_cubes(grid, stock_top_z, stock_bottom_z)
}

/// Extract a **per-segment** surface mesh from a side-face grid (X or Y).
///
/// For each segment in each ray, a vertex is emitted at the segment's `exit`
/// (the outermost material surface toward the tool entry direction) **and** at
/// its `enter`. This correctly represents cuts that leave multiple material
/// layers visible from the side.
///
/// Vertex positions are mapped back to (x, y, z) world coordinates:
/// - Y-grid (u=X, v=Z, depth=Y): vertex = (u, depth, v)
/// - X-grid (u=Y, v=Z, depth=X): vertex = (depth, u, v)
#[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
fn side_grid_to_mesh(grid: &DexelGrid, stock_top_depth: f64, stock_bottom_depth: f64) -> StockMesh {
    let rows = grid.rows;
    let cols = grid.cols;

    // For single-segment rays, this produces the same result as before.
    // For multi-segment rays, we emit one surface per segment boundary.

    // Phase 1: collect per-cell segment data.
    let cell_segs: Vec<&[DexelSegment]> = grid.rays.iter().map(|r| r.as_slice()).collect();

    // Compute depth range for coloring.
    let mut d_min = f64::INFINITY;
    for segs in &cell_segs {
        for seg in *segs {
            let d = seg.exit as f64;
            if d < d_min {
                d_min = d;
            }
        }
    }
    if d_min == f64::INFINITY {
        d_min = stock_bottom_depth;
    }
    let d_range = (stock_top_depth - d_min).max(1e-6);

    // Phase 2: For each segment index k, emit a heightmap-like surface at
    // segment[k].exit for cells that have at least k+1 segments.
    // Also emit surfaces at segment[k].enter for internal boundaries.
    //
    // Find the maximum segment count across all cells.
    let max_segs = cell_segs.iter().map(|s| s.len()).max().unwrap_or(0);
    if max_segs == 0 {
        return StockMesh::empty();
    }

    let mut vertices = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    // For each segment layer, emit the outermost surface (exit).
    // The topmost segment's exit is the most visible from the tool side.
    for seg_idx in 0..max_segs {
        let base = (vertices.len() / 3) as u32;

        // Emit one vertex per cell at this segment's exit depth (or fallback).
        for row in 0..rows {
            for col in 0..cols {
                let (wu, wv) = grid.cell_to_world(row, col);
                let segs = cell_segs[row * cols + col];

                // Use this segment's exit if it exists, otherwise skip in tiling.
                let d = if seg_idx < segs.len() {
                    segs[seg_idx].exit as f64
                } else {
                    stock_bottom_depth
                };

                let (x, y, z) = match grid.axis {
                    DexelAxis::Y => (wu, d, wv),
                    DexelAxis::X => (d, wu, wv),
                    DexelAxis::Z => (wu, wv, d),
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

        // Tile quads only where all four corners have this segment.
        for row in 0..(rows.saturating_sub(1)) {
            for col in 0..(cols.saturating_sub(1)) {
                let has_seg = |r: usize, c: usize| cell_segs[r * cols + c].len() > seg_idx;
                if !has_seg(row, col)
                    || !has_seg(row, col + 1)
                    || !has_seg(row + 1, col)
                    || !has_seg(row + 1, col + 1)
                {
                    continue;
                }
                let tl = base + (row * cols + col) as u32;
                let tr = tl + 1;
                let bl = base + ((row + 1) * cols + col) as u32;
                let br = bl + 1;
                indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
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

/// Append analytic drill-hole geometry to a stock mesh — DEXEL roadmap §6.E
/// Step 3.
///
/// Each [`crate::drill_op::DrillOp`] adds a 16-sided cylinder side wall
/// (and a flat-bottom cap for [`crate::drill_op::ToolProfile::Flat`])
/// inside the existing heightmap mesh. This gives clean circular walls
/// at low dexel resolutions instead of cell-stepped approximations.
///
/// Composition: call AFTER [`dexel_stock_to_mesh`] so the analytic
/// cylinders sit on top of the heightmap-rendered approximation. The
/// "seam" between heightmap walls and analytic cylinders is visible
/// at low dexel resolution — Step 5 (marching cubes) replaces the
/// heightmap walls so they align cleanly.
pub fn append_drill_cylinders(base: &mut StockMesh, drill_ops: &[&crate::drill_op::DrillOp]) {
    const AZIMUTH_SEGMENTS: usize = 16;
    for drill_op in drill_ops {
        let radius = drill_op.tool_diameter_mm as f32 * 0.5;
        if radius <= 0.0 {
            continue;
        }
        // Cone-tip protrusion for non-flat profiles. For Flat, tip is at
        // bottom_z; for coned profiles, the cylindrical shoulder starts
        // at bottom_z + tip_protrusion.
        let tip_protrusion = drill_op
            .tool_profile
            .tip_protrusion_mm(drill_op.tool_diameter_mm * 0.5) as f32;

        for hole in &drill_op.holes {
            let cx = hole.xy[0] as f32;
            let cy = hole.xy[1] as f32;
            let bottom_z = hole.bottom_z as f32;
            let top_z = hole.top_z as f32;
            // Skip degenerate holes (e.g. zero-depth).
            if top_z <= bottom_z {
                continue;
            }
            // Cylinder section bottom is at the shoulder where the cone
            // meets the cylindrical body (or `bottom_z` for Flat).
            let cylinder_bottom = bottom_z + tip_protrusion;
            let is_flat = matches!(drill_op.tool_profile, crate::drill_op::ToolProfile::Flat);

            let azimuth: Vec<(f32, f32)> = (0..AZIMUTH_SEGMENTS)
                .map(|i| {
                    let theta = (i as f32) * std::f32::consts::TAU / AZIMUTH_SEGMENTS as f32;
                    (theta.cos(), theta.sin())
                })
                .collect();

            let mut cyl = StockMesh::empty();

            // ── Cylinder side wall (cylinder_bottom → top_z) ─────────
            // Vertices: ring at cylinder_bottom + ring at top_z.
            for &(cos_t, sin_t) in &azimuth {
                let px = cx + radius * cos_t;
                let py = cy + radius * sin_t;
                cyl.vertices.extend_from_slice(&[px, py, cylinder_bottom]);
                cyl.colors.extend_from_slice(&[CUT_R, CUT_G, CUT_B]);
            }
            for &(cos_t, sin_t) in &azimuth {
                let px = cx + radius * cos_t;
                let py = cy + radius * sin_t;
                cyl.vertices.extend_from_slice(&[px, py, top_z]);
                cyl.colors.extend_from_slice(&[CUT_R, CUT_G, CUT_B]);
            }
            // Triangles: 2 per quad, inward-facing normals (visible when
            // viewed from inside the hole). Winding `bot_i, bot_{i+1},
            // top_i` gives a normal cross product pointing toward the
            // axis at θ=0 (`+θ × +z = -r`).
            for i in 0..AZIMUTH_SEGMENTS {
                let next = (i + 1) % AZIMUTH_SEGMENTS;
                let bi = i as u32;
                let bn = next as u32;
                let ti = (AZIMUTH_SEGMENTS + i) as u32;
                let tn = (AZIMUTH_SEGMENTS + next) as u32;
                cyl.indices.extend_from_slice(&[bi, bn, ti, ti, bn, tn]);
            }

            // ── Bottom cap or cone tip ────────────────────────────────
            if is_flat {
                // Flat-bottom cap: fan from center (axis, bottom_z) out
                // to the cylinder_bottom ring. Wound CCW from below so
                // the visible side is the *top* face inside the hole.
                let center_idx = cyl.vertices.len() / 3;
                cyl.vertices.extend_from_slice(&[cx, cy, bottom_z]);
                cyl.colors.extend_from_slice(&[CUT_R, CUT_G, CUT_B]);
                for i in 0..AZIMUTH_SEGMENTS {
                    let next = (i + 1) % AZIMUTH_SEGMENTS;
                    cyl.indices
                        .extend_from_slice(&[center_idx as u32, next as u32, i as u32]);
                }
            } else {
                // Conical tip: apex at (axis, bottom_z), base on the
                // cylinder_bottom ring.
                let apex_idx = cyl.vertices.len() / 3;
                cyl.vertices.extend_from_slice(&[cx, cy, bottom_z]);
                cyl.colors.extend_from_slice(&[CUT_R, CUT_G, CUT_B]);
                for i in 0..AZIMUTH_SEGMENTS {
                    let next = (i + 1) % AZIMUTH_SEGMENTS;
                    cyl.indices
                        .extend_from_slice(&[apex_idx as u32, next as u32, i as u32]);
                }
            }

            append_mesh(base, &cyl);
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::dexel::ray_subtract_above;
    use crate::dexel_stock::{StockCutDirection, TriDexelStock};
    use crate::tool::{FlatEndmill, MillingCutter};

    #[test]
    fn solid_mesh_is_non_empty_and_well_formed() {
        // DEXEL roadmap Step 5 (J): MC heightmap mesh replaces the hard-coded
        // 50-vertex / 288-index layout of the legacy heightmap. Assert
        // topology invariants instead.
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        assert!(
            !mesh.vertices.is_empty(),
            "uncut block must produce vertices"
        );
        assert_eq!(mesh.vertices.len() % 3, 0);
        assert_eq!(mesh.indices.len() % 3, 0);
        assert_eq!(mesh.colors.len(), mesh.vertices.len());
        // No indices should be out of range.
        let n_verts = mesh.vertices.len() / 3;
        for &i in &mesh.indices {
            assert!(
                (i as usize) < n_verts,
                "index {i} out of range (verts: {n_verts})"
            );
        }
    }

    #[test]
    fn uncut_solid_includes_stock_top_and_bottom() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        let mut saw_top = false;
        let mut saw_bot = false;
        for i in 0..mesh.vertices.len() / 3 {
            let z = mesh.vertices[i * 3 + 2];
            if (z - 5.0).abs() < 0.01 {
                saw_top = true;
            }
            if z.abs() < 0.01 {
                saw_bot = true;
            }
        }
        assert!(saw_top, "uncut mesh must include a stock-top vertex");
        assert!(saw_bot, "uncut mesh must include a stock-bottom vertex");
    }

    #[test]
    fn through_hole_alters_mesh_and_emits_walls() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
        // Clear the center ray entirely (through-hole).
        stock.z_grid.ray_mut(1, 1).clear();

        let mesh = dexel_stock_to_mesh(&stock);
        let mesh_without_hole = {
            let s = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
            dexel_stock_to_mesh(&s)
        };
        // MC heightmap mesh: hole skips one top + one bottom quad and adds
        // four hole-wall quads. Net index count differs from the solid case
        // (typically larger, since 4 wall quads > 2 skipped face quads).
        assert_ne!(
            mesh.indices.len(),
            mesh_without_hole.indices.len(),
            "Hole should alter the index count vs solid"
        );
        assert!(
            !mesh.indices.is_empty(),
            "Mesh with hole must still have faces"
        );
        // Verify wall presence: hole-wall corners sit at u or v ≈ 0.5 / 1.5
        // (cell-corner positions on the (1,1) cell boundary) and z=stock_top
        // (= 10) or z=stock_bottom (= 0).
        let mut found_wall = false;
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if ((x - 0.5).abs() < 0.01 || (x - 1.5).abs() < 0.01)
                && ((y - 0.5).abs() < 0.01 || (y - 1.5).abs() < 0.01)
                && ((z - 10.0).abs() < 0.01 || z.abs() < 0.01)
            {
                found_wall = true;
                break;
            }
        }
        assert!(
            found_wall,
            "expected hole-wall vertex on the (1,1) cell perimeter"
        );
    }

    #[test]
    fn uncut_top_face_colors_are_light_tan() {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 1.0, 1.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        // Find vertices at z = stock_top — they should all be uncut color.
        let mut found = 0;
        for i in 0..mesh.vertices.len() / 3 {
            let z = mesh.vertices[i * 3 + 2];
            if (z - 5.0).abs() < 0.01 {
                let r = mesh.colors[i * 3];
                let g = mesh.colors[i * 3 + 1];
                let b = mesh.colors[i * 3 + 2];
                assert!((r - UNCUT_R).abs() < 0.01, "top-face vertex {i} R={r}");
                assert!((g - UNCUT_G).abs() < 0.01, "top-face vertex {i} G={g}");
                assert!((b - UNCUT_B).abs() < 0.01, "top-face vertex {i} B={b}");
                found += 1;
            }
        }
        assert!(found > 0, "expected at least one top-face vertex");
    }

    #[test]
    fn deep_cut_produces_dark_walnut_colors() {
        // Cut a 3×3 region deep — corner-bilinear average produces interior
        // corners at the cut depth, yielding dark-walnut top-face vertices.
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 6.0, 6.0, 0.0, 5.0, 1.0);
        for r in 2..=4 {
            for c in 2..=4 {
                ray_subtract_above(stock.z_grid.ray_mut(r, c), 0.1);
            }
        }
        let mesh = dexel_stock_to_mesh(&stock);
        // Find a vertex near the centre with z near the cut depth.
        let mut found_dark = false;
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if (x - 3.0).abs() < 1.0 && (y - 3.0).abs() < 1.0 && z < 1.0 {
                let r = mesh.colors[i * 3];
                let g = mesh.colors[i * 3 + 1];
                let b = mesh.colors[i * 3 + 2];
                if (r - CUT_R).abs() < 0.05 && (g - CUT_G).abs() < 0.05 && (b - CUT_B).abs() < 0.05
                {
                    found_dark = true;
                    break;
                }
            }
        }
        assert!(found_dark, "expected dark-walnut vertex near deep cut");
    }

    /// Top + Bottom two-setup simulation: the solid mesh must have both
    /// top-surface vertices (from ray_top) and bottom-surface vertices
    /// (from ray_bottom) reflecting cuts from both directions. Under MC
    /// heightmap extraction (Step 5), assert via point-cloud query rather
    /// than the legacy fixed-index layout.
    #[test]
    fn top_bottom_job_mesh_shows_both_cuts() {
        use crate::dexel::{ray_bottom, ray_top};
        use crate::radial_profile::RadialProfileLUT;

        let stock_h = 10.6;
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 110.0, 110.0, 0.0, stock_h, 1.0);

        let tool = FlatEndmill::new(6.35, 25.0);
        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);

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

        let (row, col) = stock
            .z_grid
            .world_to_cell(55.0, 55.0)
            .expect("center of 110x110 stock should be inside the grid");
        let ray = stock.z_grid.ray(row, col);
        assert!((ray_top(ray).expect("ray should have material") - 7.0).abs() < 0.1);
        assert!((ray_bottom(ray).expect("ray should have material") - 3.0).abs() < 0.1);

        let mesh = dexel_stock_to_mesh(&stock);
        // The cutter footprint (Ø6.35) clears several cells; MC heightmap
        // emits top-face vertices at z≈7 and bottom-face vertices at z≈3
        // within the cut region. Query by point cloud.
        let mut hits_top = 0;
        let mut hits_bot = 0;
        for i in 0..mesh.vertices.len() / 3 {
            let x = mesh.vertices[i * 3];
            let y = mesh.vertices[i * 3 + 1];
            let z = mesh.vertices[i * 3 + 2];
            if (x - 55.0).abs() < 2.0 && (y - 55.0).abs() < 2.0 {
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
            "expected MC vertices at both cut levels: top_hits={hits_top}, bot_hits={hits_bot}"
        );
    }

    #[test]
    fn multi_grid_mesh_has_more_vertices() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 1.0);
        let z_only_mesh = dexel_stock_to_mesh(&stock);

        let tool = FlatEndmill::new(4.0, 20.0);
        let lut = crate::radial_profile::RadialProfileLUT::from_cutter(
            &tool,
            crate::radial_profile::LUT_SAMPLES,
        );
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

    // ── Segment-aware mesh tests ──────────────────────────────────────

    #[test]
    fn single_segment_rays_produce_no_cavity_emission() {
        // Under MC heightmap extraction (Step 5), single-segment rays bypass
        // the per-gap cavity fallback. The mesh should be exactly the
        // top + bottom + perimeter envelope; introducing a multi-segment ray
        // strictly grows the vertex count via cavity emission.
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let envelope_mesh = dexel_stock_to_mesh(&stock);
        let envelope_verts = envelope_mesh.vertices.len() / 3;
        assert!(envelope_verts > 0);

        // Introduce a 2×2 region of through-cuts so the cavity fallback
        // (which matches gaps across 2x2 cell blocks) actually emits
        // floor/ceiling quads. A single-cell gap won't match neighbours
        // and will not trigger the cavity fallback.
        let mut stock_with_gap = stock;
        for r in 2..=3 {
            for c in 2..=3 {
                crate::dexel::ray_subtract_interval(stock_with_gap.z_grid.ray_mut(r, c), 2.0, 3.0);
            }
        }
        let gap_mesh = dexel_stock_to_mesh(&stock_with_gap);
        assert!(
            gap_mesh.vertices.len() / 3 > envelope_verts,
            "multi-segment ray should produce more vertices than single-segment envelope: \
             gap={}, envelope={}",
            gap_mesh.vertices.len() / 3,
            envelope_verts
        );
    }

    #[test]
    fn multi_segment_through_cut_produces_internal_surfaces() {
        use crate::dexel::ray_subtract_interval;

        // Create a 4x4 grid (5x5 cells), cut a through-slot in the middle.
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);
        let cells = stock.z_grid.rows * stock.z_grid.cols;

        // Create multi-segment rays by subtracting an interval from the
        // center rows. This simulates a through-cut creating two segments
        // per ray: [0,3] and [7,10].
        for row in 1..=3 {
            for col in 1..=3 {
                let ray = stock.z_grid.ray_mut(row, col);
                ray_subtract_interval(ray, 3.0, 7.0);
                // Verify we now have 2 segments.
                assert_eq!(
                    ray.len(),
                    2,
                    "Ray ({row},{col}) should have 2 segments after interval subtract"
                );
            }
        }

        let mesh = dexel_stock_to_mesh(&stock);

        // The mesh should have MORE vertices than 2*cells because of
        // internal cavity surfaces (ceiling and floor faces).
        assert!(
            mesh.vertices.len() / 3 > 2 * cells,
            "Multi-segment rays should produce extra cavity vertices: got {} vs baseline {}",
            mesh.vertices.len() / 3,
            2 * cells
        );

        // The mesh should also have more indices than an uncut stock
        // because of the internal horizontal and vertical faces.
        let uncut_mesh = {
            let s = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);
            dexel_stock_to_mesh(&s)
        };
        assert!(
            mesh.indices.len() > uncut_mesh.indices.len(),
            "Multi-segment mesh should have more indices: {} vs {}",
            mesh.indices.len(),
            uncut_mesh.indices.len()
        );

        // Verify that internal surface vertices exist at the gap boundaries.
        // Gap is [3.0, 7.0], so we expect vertices at Z=3.0 (ceiling) and
        // Z=7.0 (floor).
        let extra_start = 2 * cells;
        let extra_verts = mesh.vertices.len() / 3 - extra_start;
        assert!(extra_verts > 0, "Should have extra cavity vertices");

        let mut has_ceiling = false;
        let mut has_floor = false;
        for i in extra_start..(mesh.vertices.len() / 3) {
            let z = mesh.vertices[i * 3 + 2];
            if (z - 3.0).abs() < 0.01 {
                has_ceiling = true;
            }
            if (z - 7.0).abs() < 0.01 {
                has_floor = true;
            }
        }
        assert!(has_ceiling, "Should have ceiling vertices at Z=3.0");
        assert!(has_floor, "Should have floor vertices at Z=7.0");
    }

    #[test]
    fn empty_ray_next_to_multi_segment_produces_walls() {
        use crate::dexel::ray_subtract_interval;

        // Create a 3x3 grid (4x4 cells).
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 3.0, 3.0, 0.0, 10.0, 1.0);

        // Create multi-segment ray at (1,1) and (1,2).
        for col in 1..=2 {
            ray_subtract_interval(stock.z_grid.ray_mut(1, col), 3.0, 7.0);
        }
        // Clear ray at (1,0) — empty next to multi-segment.
        stock.z_grid.ray_mut(1, 0).clear();

        let mesh = dexel_stock_to_mesh(&stock);

        // The mesh should still be valid (non-empty) and have wall faces
        // at the boundary between empty and material cells.
        assert!(
            !mesh.indices.is_empty(),
            "Mesh should have faces despite empty neighbor"
        );
        assert_eq!(
            mesh.vertices.len() / 3,
            mesh.colors.len() / 3,
            "Vertex and color counts must match"
        );
    }

    #[test]
    fn vertex_count_increases_with_segment_count() {
        use crate::dexel::ray_subtract_interval;

        // Create a 3x3 grid (4x4 cells).
        let stock_1seg = TriDexelStock::from_stock(0.0, 0.0, 3.0, 3.0, 0.0, 10.0, 1.0);
        let mesh_1seg = dexel_stock_to_mesh(&stock_1seg);

        // Create 2-segment rays in a 2x2 block.
        let mut stock_2seg = TriDexelStock::from_stock(0.0, 0.0, 3.0, 3.0, 0.0, 10.0, 1.0);
        for row in 1..=2 {
            for col in 1..=2 {
                ray_subtract_interval(stock_2seg.z_grid.ray_mut(row, col), 3.0, 7.0);
            }
        }
        let mesh_2seg = dexel_stock_to_mesh(&stock_2seg);

        // Create 3-segment rays (two gaps).
        let mut stock_3seg = TriDexelStock::from_stock(0.0, 0.0, 3.0, 3.0, 0.0, 10.0, 1.0);
        for row in 1..=2 {
            for col in 1..=2 {
                ray_subtract_interval(stock_3seg.z_grid.ray_mut(row, col), 2.0, 4.0);
                ray_subtract_interval(stock_3seg.z_grid.ray_mut(row, col), 6.0, 8.0);
            }
        }
        let mesh_3seg = dexel_stock_to_mesh(&stock_3seg);

        let v1 = mesh_1seg.vertices.len() / 3;
        let v2 = mesh_2seg.vertices.len() / 3;
        let v3 = mesh_3seg.vertices.len() / 3;

        assert!(
            v2 > v1,
            "2-segment mesh ({v2} verts) should have more vertices than 1-segment ({v1})"
        );
        assert!(
            v3 > v2,
            "3-segment mesh ({v3} verts) should have more vertices than 2-segment ({v2})"
        );
    }
}
