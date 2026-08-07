//! Rest-depth heatmap mesh — turns a [`RestGrid`] into a colored quad mesh
//! for the GUI viewport overlay (finishing-speedup backlog task #3, "render
//! the rest-heatmap"). The data (`RestGrid`, `AnnotatedToolpath::rest_grid`,
//! `translate_annotated`/`AnnotatedToolpath::translated`) was already
//! plumbed by the pencil rest-depth detector (`crate::rest_field`); this
//! module is the last mile — grid cells → a `StockMesh` the wgpu viewport
//! can upload and draw.
//!
//! One vertex per grid cell, draped on the pencil-drop surface
//! (`RestGrid::surface_z`) and lifted by [`REST_HEATMAP_LIFT_MM`] so the
//! overlay doesn't coplanar z-fight the model/stock mesh it sits on top of
//! (relevant even under a depth-read-only pipeline: read-only means the
//! overlay doesn't WRITE depth, but it still depth-TESTS against what's
//! already there, and an exactly-coplanar surface z-fights per pixel).
//!
//! Quads with any NaN corner (an untrusted cell, per `RestGrid`'s own
//! contract) are skipped — the same NaN-skip pattern used by
//! `crate::dexel_mesh::z_grid_to_solid_mesh_heightmap` for hole/cavity
//! quads.
//!
//! # Color ramp and the one-source-of-truth requirement
//!
//! Cells at or below `grid.threshold` get a faint neutral desaturated
//! blue-gray — "no significant rest material here". Cells strictly above
//! threshold ramp blue → yellow → red, normalized to the 95th percentile
//! (not the max) of above-threshold rest values, so one outlier spike
//! doesn't wash out the rest of the map's contrast.
//!
//! `grid.threshold` IS `RestFieldParams::min_valley_depth` — the same value
//! [`crate::rest_field::detect_rest_valleys`] thresholds on to build the
//! mask that becomes `RestFieldResult::region_polygons`. This module reuses
//! that exact threshold rather than inventing a second one: the heatmap's
//! "hot" region and the derived region polygons must never disagree about
//! what counts as rest material.

use crate::rest_field::RestGrid;
use crate::stock_mesh::StockMesh;

/// Vertical lift (mm) applied to every heatmap vertex above the RestGrid's
/// `surface_z`. Keeps the draped overlay from z-fighting the model/stock
/// surface it sits on.
pub const REST_HEATMAP_LIFT_MM: f32 = 0.05;

/// Faint neutral color for at-or-below-threshold cells — desaturated
/// blue-gray, reads as background rather than demanding attention.
const BELOW_THRESHOLD_COLOR: [f32; 3] = [0.35, 0.4, 0.45];

/// Convert a [`RestGrid`] into a colored quad mesh for the viewport overlay.
///
/// Returns `None` when the grid is too small to form a quad, or has no
/// trusted (non-NaN in both `rest` and `surface_z`) cells at all.
pub fn rest_grid_to_heatmap_mesh(grid: &RestGrid) -> Option<StockMesh> {
    if grid.nx < 2 || grid.ny < 2 {
        return None;
    }
    let cells = grid.nx * grid.ny;

    let rest_at = |i: usize| -> Option<f32> { grid.rest.get(i).copied().filter(|v| v.is_finite()) };
    let z_at =
        |i: usize| -> Option<f32> { grid.surface_z.get(i).copied().filter(|v| v.is_finite()) };
    let valid = |i: usize| -> bool { rest_at(i).is_some() && z_at(i).is_some() };

    if !(0..cells).any(valid) {
        return None;
    }

    // p95 of above-threshold rest values, for ramp normalization — see
    // module doc for why p95 rather than max.
    let threshold = grid.threshold as f32;
    let mut above: Vec<f32> = (0..cells)
        .filter(|&i| valid(i))
        .filter_map(rest_at)
        .filter(|&r| r > threshold)
        .collect();
    let p95 = if above.is_empty() {
        threshold + 1e-6
    } else {
        above.sort_by(f32::total_cmp);
        let last = above.len() - 1;
        let idx = ((last as f32) * 0.95).round() as usize;
        let picked = above.get(idx).or_else(|| above.last()).copied();
        picked.unwrap_or(threshold).max(threshold + 1e-6)
    };

    let mut vertices = Vec::with_capacity(cells * 3);
    let mut colors = Vec::with_capacity(cells * 3);
    for r in 0..grid.ny {
        for c in 0..grid.nx {
            let i = r * grid.nx + c;
            let x = grid.origin_x + c as f64 * grid.cell_mm;
            let y = grid.origin_y + r as f64 * grid.cell_mm;
            let (rest_v, z_v) = match (rest_at(i), z_at(i)) {
                (Some(rv), Some(zv)) => (rv, zv),
                _ => (f32::NAN, 0.0),
            };
            vertices.push(x as f32);
            vertices.push(y as f32);
            vertices.push(z_v + REST_HEATMAP_LIFT_MM);
            let color = rest_ramp_color(rest_v, threshold, p95);
            colors.push(color[0]);
            colors.push(color[1]);
            colors.push(color[2]);
        }
    }

    let mut indices = Vec::with_capacity((grid.ny - 1) * (grid.nx - 1) * 6);
    for r in 0..(grid.ny - 1) {
        for c in 0..(grid.nx - 1) {
            let tl_i = r * grid.nx + c;
            let tr_i = tl_i + 1;
            let bl_i = (r + 1) * grid.nx + c;
            let br_i = bl_i + 1;
            if !valid(tl_i) || !valid(tr_i) || !valid(bl_i) || !valid(br_i) {
                continue;
            }
            let tl = tl_i as u32;
            let tr = tr_i as u32;
            let bl = bl_i as u32;
            let br = br_i as u32;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    if indices.is_empty() {
        return None;
    }

    Some(StockMesh {
        vertices,
        indices,
        colors,
    })
}

/// Map a rest-depth value (mm) to an RGB color: faint neutral at/below
/// `threshold`, blue → yellow → red above it, normalized over
/// `threshold..=p95`. `pub` so the GUI legend swatch (which has no cheap way
/// to recompute the p95 percentile) can reuse the exact same ramp against
/// `grid.threshold`/`grid`'s max rest value instead of drifting out of sync
/// with a hand-rolled copy.
pub fn rest_ramp_color(rest: f32, threshold: f32, p95: f32) -> [f32; 3] {
    if !rest.is_finite() || rest <= threshold {
        return BELOW_THRESHOLD_COLOR;
    }
    let span = (p95 - threshold).max(1e-6);
    let t = ((rest - threshold) / span).clamp(0.0, 1.0);
    if t < 0.5 {
        let s = t * 2.0;
        [s, s, 1.0 - s] // blue -> yellow
    } else {
        let s = (t - 0.5) * 2.0;
        [1.0, 1.0 - s, 0.0] // yellow -> red
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

    fn make_grid(
        nx: usize,
        ny: usize,
        rest: Vec<f32>,
        surface_z: Vec<f32>,
        threshold: f64,
    ) -> RestGrid {
        RestGrid {
            nx,
            ny,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: 1.0,
            rest,
            surface_z,
            threshold,
        }
    }

    #[test]
    fn all_nan_grid_returns_none() {
        let n = 16;
        let grid = make_grid(4, 4, vec![f32::NAN; n], vec![f32::NAN; n], 0.05);
        assert!(rest_grid_to_heatmap_mesh(&grid).is_none());
    }

    #[test]
    fn too_small_grid_returns_none() {
        let grid = make_grid(1, 4, vec![0.1; 4], vec![0.0; 4], 0.05);
        assert!(rest_grid_to_heatmap_mesh(&grid).is_none());
    }

    #[test]
    fn nan_cell_skips_its_quads_and_shapes_stay_consistent() {
        let n = 16;
        let mut rest = vec![0.1_f32; n];
        let mut surface_z = vec![-1.0_f32; n];
        // Untrust cell (row=1, col=1) -> index 1*4+1 = 5.
        rest[5] = f32::NAN;
        surface_z[5] = f32::NAN;
        let grid = make_grid(4, 4, rest, surface_z, 0.05);

        let mesh = rest_grid_to_heatmap_mesh(&grid).expect("should produce a mesh");
        assert_eq!(mesh.vertices.len(), n * 3);
        assert_eq!(mesh.colors.len(), mesh.vertices.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        let max_vert_index = (n - 1) as u32;
        assert!(mesh.indices.iter().all(|&i| i <= max_vert_index));

        let full_grid = make_grid(4, 4, vec![0.1; n], vec![-1.0; n], 0.05);
        let full_mesh = rest_grid_to_heatmap_mesh(&full_grid).expect("full mesh");
        // The NaN cell touches up to 4 of the 9 possible quads, so strictly
        // fewer indices remain than the fully-trusted grid.
        assert!(
            mesh.indices.len() < full_mesh.indices.len(),
            "nan_indices={} full_indices={}",
            mesh.indices.len(),
            full_mesh.indices.len()
        );
    }

    #[test]
    fn below_threshold_cells_get_neutral_color() {
        let n = 16;
        let rest = vec![0.01_f32; n]; // below the 0.05 threshold everywhere
        let surface_z = vec![-2.0_f32; n];
        let grid = make_grid(4, 4, rest, surface_z, 0.05);
        let mesh = rest_grid_to_heatmap_mesh(&grid).expect("mesh");
        for chunk in mesh.colors.chunks(3) {
            assert_eq!(chunk[0], BELOW_THRESHOLD_COLOR[0]);
            assert_eq!(chunk[1], BELOW_THRESHOLD_COLOR[1]);
            assert_eq!(chunk[2], BELOW_THRESHOLD_COLOR[2]);
        }
    }

    #[test]
    fn above_threshold_cells_ramp_away_from_neutral() {
        let n = 16;
        // Mix of below/above threshold values so the ramp has real signal.
        let rest: Vec<f32> = (0..n).map(|i| 0.01 + 0.1 * i as f32).collect();
        let surface_z = vec![-3.0_f32; n];
        let grid = make_grid(4, 4, rest, surface_z, 0.05);
        let mesh = rest_grid_to_heatmap_mesh(&grid).expect("mesh");
        // The highest-rest vertex (last cell) must differ from the neutral
        // background color — it's well above threshold.
        let last = mesh.colors.len() - 3;
        let peak_color = [
            mesh.colors[last],
            mesh.colors[last + 1],
            mesh.colors[last + 2],
        ];
        assert_ne!(peak_color, BELOW_THRESHOLD_COLOR);
    }

    #[test]
    fn vertex_z_is_surface_z_plus_lift() {
        let n = 16;
        let rest = vec![0.1_f32; n];
        let surface_z = vec![-5.0_f32; n];
        let grid = make_grid(4, 4, rest, surface_z, 0.05);
        let mesh = rest_grid_to_heatmap_mesh(&grid).expect("mesh");
        for v in mesh.vertices.chunks(3) {
            assert!((v[2] - (-5.0 + REST_HEATMAP_LIFT_MM)).abs() < 1e-6);
        }
    }
}
