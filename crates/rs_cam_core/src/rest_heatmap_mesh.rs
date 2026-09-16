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
//! `crate::dexel_mesh_mc::z_grid_marching_cubes` for hole/cavity quads.
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
//!
//! # Second producer: the multi-tool tier preview
//!
//! [`tier_map_to_heatmap_mesh`] builds the same [`StockMesh`] product from a
//! [`TierMap`] + [`TierIslands`] instead of a [`RestGrid`], for the planner's
//! pre-generation veto overlay (`ORCHESTRATION_PLAN.md` Phase U). It shares
//! this module because it shares the whole downstream path — one vertex per
//! grid cell, draped and lifted, NaN corners skipping their quads, uploaded
//! and drawn by the viewport's depth-read-only overlay pipeline. What it does
//! NOT share is the colour law: a rest depth is a scalar and ramps, a tier
//! label is categorical and gets a per-tier fill. The two overlays have
//! independent visibility flags and can be on at once.

use crate::geo::P2;
use crate::rest_field::RestGrid;
use crate::stock_mesh::StockMesh;
use crate::tier_islands::TierIslands;
use crate::tier_map::{NO_TIER, TierMap};

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

// ── Multi-tool tier preview ─────────────────────────────────────────────

/// Vertical lift (mm) applied to every tier-preview vertex above the tier
/// map's reference drop plane. Larger than [`REST_HEATMAP_LIFT_MM`] on
/// purpose: the plane this overlay drapes on is the FINEST tool's drop
/// surface, which sits a tool radius above the machined surface the rest
/// heatmap uses, and the two overlays can be on at once.
pub const TIER_PREVIEW_LIFT_MM: f32 = 0.1;

/// One fill colour per fine tier, indexed `(tier - 1) % len`. Saturated and
/// well separated so a tier reads at a glance; tier 0 draws nothing at all, so
/// it needs no entry.
const TIER_FILL_COLORS: [[f32; 3]; 6] = [
    [0.20, 0.55, 0.95],
    [0.95, 0.55, 0.10],
    [0.30, 0.75, 0.35],
    [0.85, 0.25, 0.55],
    [0.55, 0.35, 0.85],
    [0.90, 0.80, 0.20],
];

/// The tier's fill blended halfway to white — the seam-blend band, i.e. the
/// `overlap_mm` ring by which a fine tier's `machining` set reaches past its
/// `owned` set into coarser territory.
///
/// Derived from the tier's own colour rather than being one flat tint, so on a
/// three-tier ladder the operator can still see WHOSE band a strip is. The
/// derivation is a midpoint with white, which cannot land on another entry of
/// [`TIER_FILL_COLORS`] — pinned by
/// `no_tier_fill_collides_with_an_overlap_tint`.
fn tier_overlap_tint(fill: [f32; 3]) -> [f32; 3] {
    [
        (fill[0] + 1.0) * 0.5,
        (fill[1] + 1.0) * 0.5,
        (fill[2] + 1.0) * 0.5,
    ]
}

/// The fill colour for fine tier `k`. `pub` so the planner dialog's per-tier
/// table and legend can swatch the exact colour the 3D overlay draws, rather
/// than a hand-rolled copy that is free to drift — the same argument
/// [`rest_ramp_color`] makes for the rest legend.
///
/// Tier 0 has no colour: it is the coarse tool's complement and the overlay
/// deliberately draws nothing there.
#[must_use]
pub fn tier_fill_color(tier: u8) -> Option<[f32; 3]> {
    let index = usize::from(tier).checked_sub(1)?;
    TIER_FILL_COLORS
        .get(index % TIER_FILL_COLORS.len())
        .copied()
}

/// The overlap-band tint for fine tier `k`. See [`tier_fill_color`].
#[must_use]
pub fn tier_overlap_color(tier: u8) -> Option<[f32; 3]> {
    tier_fill_color(tier).map(tier_overlap_tint)
}

/// What one preview cell is painted as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellPaint {
    /// Nothing is drawn here: tier 0's complement, or off the part.
    None,
    /// Owned by this fine tier.
    Owned(u8),
    /// Inside this fine tier's overlap band but owned by a coarser tier.
    Overlap(u8),
}

/// Colored mesh for the viewport overlay: one fill color per fine tier over
/// its owned cells, a visually distinct tint for the overlap band
/// (`machining` minus `owned`), nothing for tier 0 / [`NO_TIER`].
///
/// Draped on the tier map's reference-drop plane ([`TierMap::finest_z`]) and
/// lifted by [`TIER_PREVIEW_LIFT_MM`], exactly as
/// [`rest_grid_to_heatmap_mesh`] drapes on `RestGrid::surface_z` — same
/// `StockMesh` product, same quad topology, same "any invalid corner skips the
/// quad" rule, so the viewport can upload it through the same path.
///
/// Returns `None` on the same three conditions the rest heatmap uses: a grid
/// too small to form a quad, no paintable cell, or no surviving quad. An empty
/// [`TierIslands`] is the second of those — the coarse tool holds the whole
/// board, which is a planning outcome and not an error.
///
/// # The overlap band is measured, not assumed
///
/// `TierIslandSet` publishes `owned_mask` per cell but publishes `machining`
/// only as polygons, and it does not carry `overlap_mm`, so the band cannot be
/// re-derived by dilating the mask — it is rasterised from the polygons
/// themselves, each one over its own bounding box, which is what keeps this
/// linear in band area rather than in board area. A cell already owned by any
/// tier keeps its fill; where two tiers' bands overlap the first tier in
/// `per_tier` order (coarsest first) wins, deterministically.
#[must_use]
pub fn tier_map_to_heatmap_mesh(map: &TierMap, islands: &TierIslands) -> Option<StockMesh> {
    if map.nx < 2 || map.ny < 2 {
        return None;
    }
    let cells = map.nx.checked_mul(map.ny)?;
    if map.labels.len() != cells
        || map.finest_z.len() != cells
        || map.cell_mm <= 0.0
        || !map.cell_mm.is_finite()
    {
        return None;
    }

    let paint = paint_cells(map, islands, cells);
    if paint.iter().all(|p| *p == CellPaint::None) {
        return None;
    }

    let z_at = |i: usize| -> Option<f32> { map.finest_z.get(i).copied().filter(|v| v.is_finite()) };
    let color_at = |i: usize| -> Option<[f32; 3]> {
        match paint.get(i).copied().unwrap_or(CellPaint::None) {
            CellPaint::None => None,
            CellPaint::Owned(tier) => tier_fill_color(tier),
            CellPaint::Overlap(tier) => tier_overlap_color(tier),
        }
    };
    let valid = |i: usize| -> bool { z_at(i).is_some() && color_at(i).is_some() };

    let mut vertices = Vec::with_capacity(cells * 3);
    let mut colors = Vec::with_capacity(cells * 3);
    for r in 0..map.ny {
        for c in 0..map.nx {
            let i = r * map.nx + c;
            let x = map.origin_x + c as f64 * map.cell_mm;
            let y = map.origin_y + r as f64 * map.cell_mm;
            vertices.push(x as f32);
            vertices.push(y as f32);
            vertices.push(z_at(i).unwrap_or(0.0) + TIER_PREVIEW_LIFT_MM);
            let color = color_at(i).unwrap_or([0.0, 0.0, 0.0]);
            colors.push(color[0]);
            colors.push(color[1]);
            colors.push(color[2]);
        }
    }

    let mut indices = Vec::with_capacity((map.ny - 1) * (map.nx - 1) * 6);
    for r in 0..(map.ny - 1) {
        for c in 0..(map.nx - 1) {
            let tl_i = r * map.nx + c;
            let tr_i = tl_i + 1;
            let bl_i = (r + 1) * map.nx + c;
            let br_i = bl_i + 1;
            if !valid(tl_i) || !valid(tr_i) || !valid(bl_i) || !valid(br_i) {
                continue;
            }
            indices.extend_from_slice(&[
                tl_i as u32,
                bl_i as u32,
                tr_i as u32,
                tr_i as u32,
                bl_i as u32,
                br_i as u32,
            ]);
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

/// Per-cell paint classification: ownership first (a straight read of
/// `owned_mask`), then the overlap bands rasterised per polygon.
fn paint_cells(map: &TierMap, islands: &TierIslands, cells: usize) -> Vec<CellPaint> {
    let mut paint = vec![CellPaint::None; cells];
    for set in &islands.per_tier {
        for (i, owned) in set.owned_mask.iter().enumerate() {
            if *owned && let Some(slot) = paint.get_mut(i) {
                *slot = CellPaint::Owned(set.tier);
            }
        }
    }

    for set in &islands.per_tier {
        for polygon in set.machining.as_slice() {
            let [min_x, min_y, max_x, max_y] = polygon.bbox();
            let (Some(col_lo), Some(row_lo)) = (
                cell_index_floor(min_x - map.origin_x, map.cell_mm),
                cell_index_floor(min_y - map.origin_y, map.cell_mm),
            ) else {
                continue;
            };
            let col_hi = cell_index_ceil(max_x - map.origin_x, map.cell_mm).min(map.nx);
            let row_hi = cell_index_ceil(max_y - map.origin_y, map.cell_mm).min(map.ny);
            for row in row_lo..row_hi {
                for col in col_lo..col_hi {
                    let i = row * map.nx + col;
                    if paint.get(i).copied().unwrap_or(CellPaint::None) != CellPaint::None {
                        continue;
                    }
                    if map.labels.get(i).copied().unwrap_or(NO_TIER) == NO_TIER {
                        continue;
                    }
                    let x = map.origin_x + col as f64 * map.cell_mm;
                    let y = map.origin_y + row as f64 * map.cell_mm;
                    if polygon.contains_point(&P2::new(x, y))
                        && let Some(slot) = paint.get_mut(i)
                    {
                        *slot = CellPaint::Overlap(set.tier);
                    }
                }
            }
        }
    }
    paint
}

/// Grid index at or below `offset / cell`, clamped at zero. `None` for a
/// non-finite offset — a polygon with a `NaN` corner has no raster footprint.
fn cell_index_floor(offset_mm: f64, cell_mm: f64) -> Option<usize> {
    let raw = (offset_mm / cell_mm).floor();
    raw.is_finite().then(|| raw.max(0.0) as usize)
}

/// Grid index one past `offset / cell`, clamped at zero. Saturates rather than
/// wrapping on an absurd polygon extent; the caller then clamps to the grid.
fn cell_index_ceil(offset_mm: f64, cell_mm: f64) -> usize {
    let raw = (offset_mm / cell_mm).ceil() + 1.0;
    if raw.is_finite() {
        raw.max(0.0) as usize
    } else {
        0
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

    // ── Multi-tool tier preview ─────────────────────────────────────────

    use crate::geometry::region_set::RegionSet;
    use crate::polygon::Polygon2;
    use crate::tier_islands::{TierCapReport, TierIslandSet};
    use crate::tier_map::ResidualTreatment;

    const PREVIEW_NX: usize = 6;
    const PREVIEW_NY: usize = 6;
    const PREVIEW_CELLS: usize = PREVIEW_NX * PREVIEW_NY;

    fn preview_map(labels: Vec<u8>, tier_count: usize) -> TierMap {
        TierMap {
            nx: PREVIEW_NX,
            ny: PREVIEW_NY,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: 1.0,
            labels,
            finest_z: vec![-4.0f32; PREVIEW_CELLS],
            tier_count,
            tolerance_mm: 0.05,
            treatment: ResidualTreatment::SlopeCompensated,
        }
    }

    fn island_set(tier: u8, owned_cells: &[usize], machining: Vec<Polygon2>) -> TierIslandSet {
        let mut owned_mask = vec![false; PREVIEW_CELLS];
        for &i in owned_cells {
            owned_mask[i] = true;
        }
        let machining_area: f64 = machining.iter().map(Polygon2::area).sum();
        TierIslandSet {
            tier,
            islands: 1,
            raw_island_count: 1,
            owned: RegionSet::new(Vec::new()),
            machining: RegionSet::new(machining),
            owned_area_mm2: owned_cells.len() as f64,
            machining_area_mm2: machining_area,
            owned_hole_count: 0,
            machining_hole_count: 0,
            median_owned_hole_area_mm2: None,
            overlap_mm: 0.0,
            owned_cells: owned_cells.len(),
            owned_mask,
            cap: TierCapReport {
                islands_after_close: 1,
                islands_after_min_area: 1,
                kept: 1,
                cap: 24,
                close_raises: 0,
                first_close_radius_mm: 0.5,
                final_close_radius_mm: 0.5,
            },
            close_radius_mm: 0.5,
            min_region_area_mm2: 1.0,
        }
    }

    fn cell(row: usize, col: usize) -> usize {
        row * PREVIEW_NX + col
    }

    fn vertex_color(mesh: &StockMesh, index: usize) -> [f32; 3] {
        [
            mesh.colors[index * 3],
            mesh.colors[index * 3 + 1],
            mesh.colors[index * 3 + 2],
        ]
    }

    /// The coarse tool holding the whole board is a planning OUTCOME, not an
    /// error — and it draws nothing, so the empty convention here is the same
    /// `None` `rest_grid_to_heatmap_mesh` uses for a grid with no trusted
    /// cell.
    #[test]
    fn an_empty_island_set_draws_nothing() {
        let map = preview_map(vec![0u8; PREVIEW_CELLS], 2);
        let islands = TierIslands {
            per_tier: Vec::new(),
            cell_mm: 1.0,
            tier_count: 2,
        };
        assert!(tier_map_to_heatmap_mesh(&map, &islands).is_none());
    }

    /// Tier 0 is the coarse tool's complement: the overlay must leave it
    /// alone, so a map whose only fine tier kept nothing is indistinguishable
    /// from no plan at all.
    #[test]
    fn a_tier_that_kept_no_cells_draws_nothing() {
        let map = preview_map(vec![0u8; PREVIEW_CELLS], 2);
        let islands = TierIslands {
            per_tier: vec![island_set(1, &[], Vec::new())],
            cell_mm: 1.0,
            tier_count: 2,
        };
        assert!(tier_map_to_heatmap_mesh(&map, &islands).is_none());
    }

    #[test]
    fn two_tiers_get_distinct_fills_and_the_band_gets_its_own_tint() {
        let mut labels = vec![0u8; PREVIEW_CELLS];
        for &i in &[cell(1, 1), cell(1, 2), cell(2, 1), cell(2, 2)] {
            labels[i] = 1;
        }
        for &i in &[cell(3, 3), cell(3, 4), cell(4, 3), cell(4, 4)] {
            labels[i] = 2;
        }
        let map = preview_map(labels, 3);

        // Tier 1's machining set reaches one cell past its ownership on the
        // low side — that ring is the `overlap_mm` blend band.
        let tier1 = island_set(
            1,
            &[cell(1, 1), cell(1, 2), cell(2, 1), cell(2, 2)],
            vec![Polygon2::rectangle(-0.5, -0.5, 2.5, 2.5)],
        );
        // Tier 2's machining set is exactly its ownership: no band.
        let tier2 = island_set(
            2,
            &[cell(3, 3), cell(3, 4), cell(4, 3), cell(4, 4)],
            vec![Polygon2::rectangle(2.5, 2.5, 4.5, 4.5)],
        );
        let islands = TierIslands {
            per_tier: vec![tier1, tier2],
            cell_mm: 1.0,
            tier_count: 3,
        };

        let mesh = tier_map_to_heatmap_mesh(&map, &islands).expect("mesh");
        let fill1 = tier_fill_color(1).expect("tier 1 has a fill");
        let fill2 = tier_fill_color(2).expect("tier 2 has a fill");
        let band1 = tier_overlap_color(1).expect("tier 1 has a band tint");
        assert_ne!(fill1, fill2, "two tiers must not share a colour");
        assert_ne!(fill1, band1, "the band must not read as ownership");

        assert_eq!(vertex_color(&mesh, cell(1, 1)), fill1);
        assert_eq!(vertex_color(&mesh, cell(4, 4)), fill2);
        // (0, 0) is inside tier 1's machining rectangle but owned by nobody.
        assert_eq!(vertex_color(&mesh, cell(0, 0)), band1);
    }

    #[test]
    fn the_preview_mesh_has_one_vertex_per_cell_and_closed_quads() {
        let mut labels = vec![0u8; PREVIEW_CELLS];
        for row in 0..PREVIEW_NY {
            for col in 0..PREVIEW_NX {
                labels[cell(row, col)] = 1;
            }
        }
        let map = preview_map(labels, 2);
        let owned: Vec<usize> = (0..PREVIEW_CELLS).collect();
        let islands = TierIslands {
            per_tier: vec![island_set(1, &owned, Vec::new())],
            cell_mm: 1.0,
            tier_count: 2,
        };

        let mesh = tier_map_to_heatmap_mesh(&map, &islands).expect("mesh");
        assert_eq!(mesh.vertices.len(), PREVIEW_CELLS * 3);
        assert_eq!(mesh.colors.len(), mesh.vertices.len());
        // Every cell painted -> every one of the (nx-1)(ny-1) quads survives.
        assert_eq!(mesh.indices.len(), (PREVIEW_NX - 1) * (PREVIEW_NY - 1) * 6);
        let max_index = (PREVIEW_CELLS - 1) as u32;
        assert!(mesh.indices.iter().all(|&i| i <= max_index));
        for v in mesh.vertices.chunks(3) {
            assert!((v[2] - (-4.0 + TIER_PREVIEW_LIFT_MM)).abs() < 1e-6);
        }
    }

    /// The band tint is DERIVED from the tier's own fill (midpoint with
    /// white) so a three-tier ladder still says whose band a strip is. That
    /// only works while no derived tint lands on another tier's fill.
    #[test]
    fn no_tier_fill_collides_with_an_overlap_tint() {
        for (a, fill) in TIER_FILL_COLORS.iter().enumerate() {
            for (b, base) in TIER_FILL_COLORS.iter().enumerate() {
                let tint = tier_overlap_tint(*base);
                assert_ne!(*fill, tint, "fill {a} equals tier {b}'s band tint");
            }
        }
    }

    #[test]
    fn tier_zero_has_no_colour_at_all() {
        assert_eq!(tier_fill_color(0), None);
        assert_eq!(tier_overlap_color(0), None);
        // The palette wraps rather than running out on a long ladder.
        assert_eq!(
            tier_fill_color(1),
            tier_fill_color(1 + TIER_FILL_COLORS.len() as u8)
        );
    }
}
