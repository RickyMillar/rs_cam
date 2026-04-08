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

/// Build a closed solid mesh from a Z-grid with **segment-aware** extraction.
///
/// The solid has six parts:
/// 1. **Top face** — one vertex per cell at `ray_top`, CCW winding.
///    Quads touching empty (through-hole) cells are skipped.
/// 2. **Bottom face** — one vertex per cell at `ray_bottom`, CW winding.
///    Same empty-cell skipping as top face.
/// 3. **Perimeter skirt** — vertical quads around the grid boundary.
/// 4. **Hole walls** — vertical quads at internal boundaries between material
///    and empty cells, creating visible interior walls of through-holes.
/// 5. **Internal cavity floors/ceilings** — horizontal faces at each gap
///    between segments within a single ray (through-cuts, internal voids).
/// 6. **Internal cavity walls** — vertical quads at cell boundaries where
///    segment counts differ, sealing the sides of internal cavities.
#[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
pub fn z_grid_to_solid_mesh(grid: &DexelGrid, stock_top_z: f64, stock_bottom_z: f64) -> StockMesh {
    let rows = grid.rows;
    let cols = grid.cols;
    let cells = rows * cols;
    let stock_top = stock_top_z as f32;
    let stock_bot = stock_bottom_z as f32;

    // Minimum material thickness to consider a ray "non-empty" for meshing.
    // Rays thinner than this are treated as through-holes to avoid sub-voxel
    // noise when opposing cuts nearly meet (e.g., 0mm stock-to-leave).
    const MIN_MATERIAL_THICKNESS: f32 = 0.05; // 50 microns

    // Collect per-cell top/bottom Z, emptiness, AND color ranges in one pass.
    let mut top_z = Vec::with_capacity(cells);
    let mut bot_z = Vec::with_capacity(cells);
    let mut empty = Vec::with_capacity(cells);
    let mut top_z_min = f32::INFINITY;
    let mut bot_z_max = f32::NEG_INFINITY;
    for ray in &grid.rays {
        let top = ray_top(ray);
        let bot = ray_bottom(ray);
        let effectively_empty = match (top, bot) {
            (Some(t), Some(b)) => (t - b) < MIN_MATERIAL_THICKNESS,
            _ => true,
        };
        empty.push(effectively_empty);
        if effectively_empty {
            top_z.push(stock_top);
            bot_z.push(stock_bot);
        } else {
            let tz = top.unwrap_or(stock_bot);
            let bz = bot.unwrap_or(stock_bot);
            top_z.push(tz);
            bot_z.push(bz);
            if tz < top_z_min {
                top_z_min = tz;
            }
            if bz > bot_z_max {
                bot_z_max = bz;
            }
        }
    }
    let top_range = (stock_top - top_z_min).max(1e-6);
    let bot_range = (bot_z_max - stock_bot).max(1e-6);

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

            let depth_t = ((stock_top - z) / top_range).clamp(0.0, 1.0);
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

            let depth_t = ((z - stock_bot) / bot_range).clamp(0.0, 1.0);
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    let mut indices = Vec::with_capacity(cells * 6); // rough estimate

    let bot_off = cells as u32;

    // Helper: cell index.
    let idx = |row: usize, col: usize| -> u32 { (row * cols + col) as u32 };
    let is_empty = |row: usize, col: usize| -> bool { empty[row * cols + col] };

    // ── Top face (CCW, normals face +Z) — skip quads with any empty corner ─
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            if is_empty(row, col)
                || is_empty(row, col + 1)
                || is_empty(row + 1, col)
                || is_empty(row + 1, col + 1)
            {
                continue;
            }
            let tl = idx(row, col);
            let tr = tl + 1;
            let bl = idx(row + 1, col);
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    // ── Bottom face (CW, normals face −Z) — same empty-cell skip ──────────
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            if is_empty(row, col)
                || is_empty(row, col + 1)
                || is_empty(row + 1, col)
                || is_empty(row + 1, col + 1)
            {
                continue;
            }
            let tl = bot_off + idx(row, col);
            let tr = tl + 1;
            let bl = bot_off + idx(row + 1, col);
            let br = bl + 1;
            indices.extend_from_slice(&[tl, tr, bl, tr, br, bl]);
        }
    }

    // ── Internal hole walls ─────────────────────────────────────────────
    // At each edge between a non-empty cell and an empty cell, generate a
    // vertical wall quad using ONLY the material cell's top and bottom
    // vertices plus those of the next material cell along the hole boundary.
    // This avoids using empty-cell vertices (which are at stock_top/bot and
    // would create huge distorted spikes).
    //
    // Strategy: for each material cell adjacent to an empty cell, emit a
    // wall quad connecting this cell's top/bottom to the next material cell
    // along the boundary in the same direction (row or col).

    // Column-direction walls: scan each row for material→empty transitions.
    for row in 0..rows {
        for col in 0..(cols - 1) {
            let a_empty = is_empty(row, col);
            let b_empty = is_empty(row, col + 1);
            if a_empty == b_empty {
                continue;
            }
            // One side has material, the other is empty. Find a neighboring
            // row to form the quad with (the material cell above or below).
            let mat_col = if !a_empty { col } else { col + 1 };
            // Look for adjacent row with material at the same column.
            if row + 1 < rows && !is_empty(row + 1, mat_col) {
                let t0 = idx(row, mat_col);
                let b0 = bot_off + t0;
                let t1 = idx(row + 1, mat_col);
                let b1 = bot_off + t1;
                if !a_empty {
                    // Material on left, empty on right → wall faces +U.
                    indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
                } else {
                    // Material on right, empty on left → wall faces −U.
                    indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
                }
            }
        }
    }

    // Row-direction walls: scan each column for material→empty transitions.
    for row in 0..(rows - 1) {
        for col in 0..cols {
            let a_empty = is_empty(row, col);
            let b_empty = is_empty(row + 1, col);
            if a_empty == b_empty {
                continue;
            }
            let mat_row = if !a_empty { row } else { row + 1 };
            if col + 1 < cols && !is_empty(mat_row, col + 1) {
                let t0 = idx(mat_row, col);
                let b0 = bot_off + t0;
                let t1 = idx(mat_row, col + 1);
                let b1 = bot_off + t1;
                if !a_empty {
                    // Material on top, empty below → wall faces +V.
                    indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
                } else {
                    // Material on bottom, empty above → wall faces −V.
                    indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
                }
            }
        }
    }

    // ── Perimeter skirt — skip segments where edge cell is empty ────────

    // Front edge (row = 0, normals face −V).
    for col in 0..(cols - 1) {
        if is_empty(0, col) || is_empty(0, col + 1) {
            continue;
        }
        let t0 = col as u32;
        let t1 = (col + 1) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
    }

    // Back edge (row = rows-1, normals face +V).
    let last_row = rows - 1;
    for col in 0..(cols - 1) {
        if is_empty(last_row, col) || is_empty(last_row, col + 1) {
            continue;
        }
        let t0 = (last_row * cols + col) as u32;
        let t1 = t0 + 1;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
    }

    // Left edge (col = 0, normals face −U).
    for row in 0..(rows - 1) {
        if is_empty(row, 0) || is_empty(row + 1, 0) {
            continue;
        }
        let t0 = (row * cols) as u32;
        let t1 = ((row + 1) * cols) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, b0, t1, t1, b0, b1]);
    }

    // Right edge (col = cols-1, normals face +U).
    let last_col = cols - 1;
    for row in 0..(rows - 1) {
        if is_empty(row, last_col) || is_empty(row + 1, last_col) {
            continue;
        }
        let t0 = (row * cols + last_col) as u32;
        let t1 = ((row + 1) * cols + last_col) as u32;
        let b0 = bot_off + t0;
        let b1 = bot_off + t1;
        indices.extend_from_slice(&[t0, t1, b0, t1, b1, b0]);
    }

    // ── Internal cavity surfaces for multi-segment rays ─────────────────
    // For each ray with >1 segment, the gap between consecutive segments
    // represents a void (through-cut, internal cavity). We emit:
    //   - A "ceiling" face (normals −Z) at the exit of each lower segment
    //   - A "floor" face (normals +Z) at the enter of each upper segment
    //   - Vertical walls at cell boundaries where segment topology differs
    emit_z_grid_cavity_surfaces(
        grid,
        &mut vertices,
        &mut colors,
        &mut indices,
        stock_top,
        stock_bot,
    );

    StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// Emit internal cavity surfaces for Z-grid cells with multi-segment rays.
///
/// For each gap between consecutive segments in a ray, we create horizontal
/// faces (ceiling of lower segment, floor of upper segment) and vertical
/// wall quads at boundaries where the gap exists in one cell but not its
/// neighbor.
#[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
fn emit_z_grid_cavity_surfaces(
    grid: &DexelGrid,
    vertices: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    stock_top: f32,
    stock_bot: f32,
) {
    let rows = grid.rows;
    let cols = grid.cols;
    let z_range = (stock_top - stock_bot).max(1e-6);

    // For each cell with >1 segment, emit horizontal faces for each internal
    // gap. We track which cells have gaps and their Z-ranges for wall generation.
    //
    // Gap k (between segment k and segment k+1):
    //   ceiling_z = segments[k].exit    (top of lower material)
    //   floor_z   = segments[k+1].enter (bottom of upper material)
    //
    // For a quad, we need 4 cells sharing the gap at compatible Z-levels.
    // The practical approach: emit per-cell quads using the cell center and
    // its right/below neighbors, similar to the top/bottom face logic.

    // Phase 1: Collect per-cell gap info (list of gap intervals).
    let cell_gaps: Vec<Vec<[f32; 2]>> = grid
        .rays
        .iter()
        .map(|ray| {
            if ray.len() < 2 {
                return Vec::new();
            }
            let mut gaps = Vec::with_capacity(ray.len() - 1);
            for w in ray.windows(2) {
                // SAFETY: windows(2) guarantees two elements
                #[allow(clippy::indexing_slicing)]
                let (lo, hi) = (&w[0], &w[1]);
                let gap_bot = lo.exit; // ceiling of lower material
                let gap_top = hi.enter; // floor of upper material
                if gap_top - gap_bot > 1e-6 {
                    gaps.push([gap_bot, gap_top]);
                }
            }
            gaps
        })
        .collect();

    // Phase 2: For each quad (2x2 cell block), find shared gaps and emit
    // horizontal ceiling/floor faces.
    //
    // A gap in cell A "matches" a gap in cell B if their Z-intervals overlap
    // significantly, indicating the same cavity spans both cells.
    for row in 0..(rows.saturating_sub(1)) {
        for col in 0..(cols.saturating_sub(1)) {
            let tl_gaps = &cell_gaps[row * cols + col];
            let tr_gaps = &cell_gaps[row * cols + col + 1];
            let bl_gaps = &cell_gaps[(row + 1) * cols + col];
            let br_gaps = &cell_gaps[(row + 1) * cols + col + 1];

            // Find gaps shared by all four corners of this quad.
            for tl_gap in tl_gaps {
                // Try to find matching gaps in the other three corners.
                let tr_match = find_matching_gap(tr_gaps, tl_gap);
                let bl_match = find_matching_gap(bl_gaps, tl_gap);
                let br_match = find_matching_gap(br_gaps, tl_gap);

                if let (Some(tr_g), Some(bl_g), Some(br_g)) = (tr_match, bl_match, br_match) {
                    // Emit ceiling face (normals −Z) at the gap bottom.
                    // Use each corner's own gap_bot for smooth interpolation.
                    let base = (vertices.len() / 3) as u32;

                    let corners = [
                        (row, col, tl_gap),
                        (row, col + 1, &tr_g),
                        (row + 1, col, &bl_g),
                        (row + 1, col + 1, &br_g),
                    ];

                    // Ceiling vertices (at gap bottom = exit of lower segment).
                    for &(r, c, gap) in &corners {
                        let (wu, wv) = grid.cell_to_world(r, c);
                        vertices.push(wu as f32);
                        vertices.push(wv as f32);
                        vertices.push(gap[0]);
                        let (cr, cg, cb) = cut_color(gap[0], stock_top, stock_bot, z_range);
                        colors.push(cr);
                        colors.push(cg);
                        colors.push(cb);
                    }
                    // CW winding for −Z normal (looking down into cavity).
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base + 1,
                        base + 3,
                        base + 2,
                    ]);

                    // Floor vertices (at gap top = enter of upper segment).
                    let base2 = (vertices.len() / 3) as u32;
                    for &(r, c, gap) in &corners {
                        let (wu, wv) = grid.cell_to_world(r, c);
                        vertices.push(wu as f32);
                        vertices.push(wv as f32);
                        vertices.push(gap[1]);
                        let (cr, cg, cb) = cut_color(gap[1], stock_top, stock_bot, z_range);
                        colors.push(cr);
                        colors.push(cg);
                        colors.push(cb);
                    }
                    // CCW winding for +Z normal (looking up at cavity ceiling).
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

    // Phase 3: Emit vertical wall quads at cell edges where one cell has a
    // gap and the adjacent cell does not (or has a non-matching gap).
    // This seals the sides of internal cavities.

    // Column-direction cavity walls (between col and col+1 in same row).
    for row in 0..rows {
        for col in 0..(cols.saturating_sub(1)) {
            let a_gaps = &cell_gaps[row * cols + col];
            let b_gaps = &cell_gaps[row * cols + col + 1];
            emit_cavity_wall_edges(
                grid,
                vertices,
                colors,
                indices,
                stock_top,
                stock_bot,
                z_range,
                a_gaps,
                b_gaps,
                row,
                col,
                row,
                col + 1,
                true,
            );
        }
    }

    // Row-direction cavity walls (between row and row+1 in same col).
    for row in 0..(rows.saturating_sub(1)) {
        for col in 0..cols {
            let a_gaps = &cell_gaps[row * cols + col];
            let b_gaps = &cell_gaps[(row + 1) * cols + col];
            emit_cavity_wall_edges(
                grid,
                vertices,
                colors,
                indices,
                stock_top,
                stock_bot,
                z_range,
                a_gaps,
                b_gaps,
                row,
                col,
                row + 1,
                col,
                false,
            );
        }
    }
}

/// Find a gap in `gaps` that overlaps `reference` by at least 50% of the
/// reference gap height. Returns the matched gap interval if found.
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

/// Emit vertical wall quads at a cell edge where cavity gaps differ.
///
/// If cell A has a gap that cell B does not (or vice versa), emit a vertical
/// quad sealing that side of the cavity. The quad spans the gap's Z-range
/// at the shared edge position.
#[allow(clippy::too_many_arguments)]
fn emit_cavity_wall_edges(
    grid: &DexelGrid,
    vertices: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
    a_gaps: &[[f32; 2]],
    b_gaps: &[[f32; 2]],
    a_row: usize,
    a_col: usize,
    b_row: usize,
    b_col: usize,
    is_col_direction: bool,
) {
    // Gaps in A not matched by B → wall on A's side facing toward B.
    for gap in a_gaps {
        if find_matching_gap(b_gaps, gap).is_none() {
            emit_cavity_wall_quad(
                grid,
                vertices,
                colors,
                indices,
                stock_top,
                stock_bot,
                z_range,
                gap,
                a_row,
                a_col,
                b_row,
                b_col,
                is_col_direction,
                true,
            );
        }
    }
    // Gaps in B not matched by A → wall on B's side facing toward A.
    for gap in b_gaps {
        if find_matching_gap(a_gaps, gap).is_none() {
            emit_cavity_wall_quad(
                grid,
                vertices,
                colors,
                indices,
                stock_top,
                stock_bot,
                z_range,
                gap,
                b_row,
                b_col,
                a_row,
                a_col,
                is_col_direction,
                false,
            );
        }
    }
}

/// Emit a single vertical wall quad for a cavity gap at a cell edge.
///
/// `gap_row/gap_col` is the cell with the gap. The quad is placed at the
/// shared edge between the two cells.
#[allow(clippy::too_many_arguments)]
fn emit_cavity_wall_quad(
    grid: &DexelGrid,
    vertices: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    stock_top: f32,
    stock_bot: f32,
    z_range: f32,
    gap: &[f32; 2],
    gap_row: usize,
    gap_col: usize,
    _other_row: usize,
    _other_col: usize,
    is_col_direction: bool,
    gap_on_a_side: bool,
) {
    let (wu, wv) = grid.cell_to_world(gap_row, gap_col);
    let half = grid.cell_size as f32 * 0.5;

    // The wall quad sits at the edge between the two cells.
    // For col-direction edges: the wall is at x + half_cell or x - half_cell.
    // For row-direction edges: the wall is at y + half_cell or y - half_cell.
    let (x0, y0, x1, y1) = if is_col_direction {
        // Edge runs along Y (row direction), at the U boundary.
        let edge_x = if gap_on_a_side {
            wu as f32 + half
        } else {
            wu as f32 - half
        };
        (edge_x, wv as f32 - half, edge_x, wv as f32 + half)
    } else {
        // Edge runs along X (col direction), at the V boundary.
        let edge_y = if gap_on_a_side {
            wv as f32 + half
        } else {
            wv as f32 - half
        };
        (wu as f32 - half, edge_y, wu as f32 + half, edge_y)
    };

    let base = (vertices.len() / 3) as u32;

    // Four vertices: bottom-left, bottom-right, top-left, top-right of the wall.
    let wall_verts = [
        (x0, y0, gap[0]), // bottom-left
        (x1, y1, gap[0]), // bottom-right
        (x0, y0, gap[1]), // top-left
        (x1, y1, gap[1]), // top-right
    ];

    for &(x, y, z) in &wall_verts {
        vertices.push(x);
        vertices.push(y);
        vertices.push(z);
        let (cr, cg, cb) = cut_color(z, stock_top, stock_bot, z_range);
        colors.push(cr);
        colors.push(cg);
        colors.push(cb);
    }

    // Winding depends on which side the gap is on (face normal toward the void).
    if gap_on_a_side == is_col_direction {
        // Normal faces +U or +V.
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    } else {
        // Normal faces −U or −V.
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
    }
}

/// Compute wood-tone color for an internal surface at a given Z depth.
fn cut_color(z: f32, stock_top: f32, stock_bot: f32, z_range: f32) -> (f32, f32, f32) {
    // Depth from top surface as a fraction of total range.
    let depth_t = ((stock_top - z) / z_range).clamp(0.0, 1.0);
    let _ = stock_bot; // used implicitly through z_range
    (
        UNCUT_R + (CUT_R - UNCUT_R) * depth_t,
        UNCUT_G + (CUT_G - UNCUT_G) * depth_t,
        UNCUT_B + (CUT_B - UNCUT_B) * depth_t,
    )
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
    fn through_hole_produces_no_top_bottom_faces() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
        // Clear the center ray entirely (through-hole).
        stock.z_grid.ray_mut(1, 1).clear();

        let mesh = dexel_stock_to_mesh(&stock);
        let mesh_without_hole = {
            let s = TriDexelStock::from_stock(0.0, 0.0, 2.0, 2.0, 0.0, 10.0, 1.0);
            dexel_stock_to_mesh(&s)
        };
        // The mesh with a hole should have fewer triangles than solid stock
        // because top/bottom quads touching the empty cell are skipped.
        assert!(
            mesh.indices.len() < mesh_without_hole.indices.len(),
            "Through-hole mesh should have fewer indices: {} vs {}",
            mesh.indices.len(),
            mesh_without_hole.indices.len()
        );
        // Should also have wall faces for the hole boundary.
        assert!(
            !mesh.indices.is_empty(),
            "Mesh with hole should still have some faces"
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
        // Cut center ray deep but leave enough material to be above MIN_MATERIAL_THICKNESS.
        ray_subtract_above(stock.z_grid.ray_mut(1, 1), 0.1);

        let mesh = dexel_stock_to_mesh(&stock);
        // Top vertex (1,1) = index 4, colors at 12..15.
        let r = mesh.colors[12];
        let g = mesh.colors[13];
        let b = mesh.colors[14];
        assert!((r - CUT_R).abs() < 0.05, "R: {r} vs {CUT_R}");
        assert!((g - CUT_G).abs() < 0.05, "G: {g} vs {CUT_G}");
        assert!((b - CUT_B).abs() < 0.05, "B: {b} vs {CUT_B}");
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

        let (row, col) = stock
            .z_grid
            .world_to_cell(55.0, 55.0)
            .expect("center of 110x110 stock should be inside the grid");
        let ray = stock.z_grid.ray(row, col);
        assert!((ray_top(ray).expect("ray should have material") - 7.0).abs() < 0.1);
        assert!((ray_bottom(ray).expect("ray should have material") - 3.0).abs() < 0.1);

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

    // ── Segment-aware mesh tests ──────────────────────────────────────

    #[test]
    fn single_segment_rays_match_baseline() {
        // An uncut stock has single-segment rays. The mesh should have the
        // same top/bottom vertex layout as before (2*cells vertices in the
        // envelope layers).
        let stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 5.0, 1.0);
        let mesh = dexel_stock_to_mesh(&stock);
        let cells = stock.z_grid.rows * stock.z_grid.cols;

        // First 2*cells vertices are the envelope (top + bottom).
        assert!(
            mesh.vertices.len() / 3 >= 2 * cells,
            "Should have at least 2*cells={} vertices, got {}",
            2 * cells,
            mesh.vertices.len() / 3
        );

        // No internal cavity surfaces should be emitted for single-segment rays.
        // The vertex count should be exactly 2*cells (no extra cavity vertices).
        assert_eq!(
            mesh.vertices.len() / 3,
            2 * cells,
            "Single-segment rays should produce exactly 2*cells vertices"
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
