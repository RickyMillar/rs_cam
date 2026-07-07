//! Mask → closed-polygon extraction, shared by rest analysis and the P2
//! unified-finish planner.
//!
//! [`region_polygons_from_mask`] is the one mask→polygon extractor with
//! dilation (whole-grid EDT) + containment grouping used across the
//! rest-depth pencil pipeline (`crate::rest_field`) and, going forward, the
//! finish planner's steep/shallow region decomposition — see
//! `planning/unified_finish_planner_design.md`.

use tracing::warn;

use crate::contour_extract::marching_squares_bool_grid;
use crate::grid_field::distance_transform_2d;
use crate::grid2::Grid2;
use crate::polygon::{Polygon2, detect_containment, shoelace_area};

/// Backstop against threshold-below-cusp sliver storms: when the rest
/// threshold is dialed below the prior pass's scallop cusp height, the mask
/// becomes the cusp pattern itself — hundreds of hair-thin stripe islands,
/// each becoming its own per-island generation pass. 64 is far above any
/// intentional region set (a genuine rest-region job is a handful to a few
/// dozen islands); the 2026-07-06 incident that motivated this cap produced
/// hundreds. [`region_polygons_from_mask`] keeps the largest `MAX_REST_REGIONS`
/// by area and warns when the raw count exceeds it — see
/// [`crate::rest_field::classify_rest_regions`] for the operator-facing
/// diagnosis surfaced separately in the GUI.
pub const MAX_REST_REGIONS: usize = 64;

/// Dilate a boolean mask by `dilate_mm` (via a whole-grid Euclidean distance
/// transform, not a per-cell radius search) and extract the dilated region(s)
/// as closed [`Polygon2`]s with holes grouped by even-odd containment depth
/// (an island inside a hole stays a top-level polygon).
///
/// `origin_x`/`origin_y` are the world coordinates of cell `(0, 0)`'s centre;
/// `cell_mm` is the cell size. `dilate_mm <= 0.0` skips dilation (uses `mask`
/// as-is). Degenerate marching-squares loops (fewer than 3 points, or
/// enclosed area under one cell) are dropped before grouping. Returns an
/// empty vec for an empty or all-`false` mask.
///
/// Regions are sorted by exterior area descending (ties broken by the
/// exterior's first-vertex position, for determinism). When the grouped
/// count exceeds [`MAX_REST_REGIONS`] — a threshold-below-cusp sliver storm,
/// the only way this has ever happened — only the largest `MAX_REST_REGIONS`
/// are kept and a `tracing::warn!` reports the total/kept counts and the
/// dropped fraction of total region area. No area floor is applied beyond
/// the cap: a genuine long thin stripe of real rest material is not
/// distinguishable from a sliver by area alone, so the cap plus warning is
/// the guard, not a per-region size filter.
pub fn region_polygons_from_mask(
    mask: &Grid2<bool>,
    origin_x: f64,
    origin_y: f64,
    cell_mm: f64,
    dilate_mm: f64,
) -> Vec<Polygon2> {
    let nx = mask.nx();
    let ny = mask.ny();
    if nx == 0 || ny == 0 || mask.as_slice().iter().all(|&v| !v) {
        return Vec::new();
    }
    let cell = cell_mm.max(1e-9);

    let dilated: std::borrow::Cow<'_, [bool]> = if dilate_mm > 0.0 {
        // Whole-grid EDT (O(cells)), not a per-cell radius loop: distance in
        // cell units from every cell to the nearest `true` cell.
        let dist = distance_transform_2d(mask.as_slice(), ny, nx);
        let radius_cells = dilate_mm / cell;
        std::borrow::Cow::Owned(dist.iter().map(|&d| d <= radius_cells).collect())
    } else {
        std::borrow::Cow::Borrowed(mask.as_slice())
    };

    // `marching_squares_bool_grid` takes `(rows, cols)`; this grid's
    // row-major layout is `r*nx + c` with `r` along Y and `c` along X (same
    // convention `detect_rest_valleys` uses via `row_major_rc`), so
    // rows = ny, cols = nx here matches the `distance_transform_2d` call
    // above and the world-coordinate mapping `x = origin_x + c*cell`,
    // `y = origin_y + r*cell` that `marching_squares_bool_grid` itself uses
    // internally — no row/col transposition needed at this boundary.
    let loops = marching_squares_bool_grid(&dilated, ny, nx, origin_x, origin_y, cell);

    let min_area = cell * cell;
    let candidate_polys: Vec<Polygon2> = loops
        .into_iter()
        .filter(|pts| pts.len() >= 3 && shoelace_area(pts).abs() >= min_area)
        .map(Polygon2::new)
        .collect();

    let mut grouped = detect_containment(candidate_polys);
    for poly in &mut grouped {
        poly.ensure_winding();
    }

    // Largest exterior area first; deterministic tie-break by the exterior's
    // first vertex so equal-area regions (e.g. the synthetic-mask unit test
    // below) sort the same way on every run.
    grouped.sort_by(|a, b| {
        let area_a = a.signed_area();
        let area_b = b.signed_area();
        match area_b.partial_cmp(&area_a) {
            Some(std::cmp::Ordering::Equal) | None => {
                let key = |p: &Polygon2| p.exterior.first().map_or((0.0, 0.0), |v| (v.x, v.y));
                let (ax, ay) = key(a);
                let (bx, by) = key(b);
                ax.partial_cmp(&bx)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal))
            }
            Some(ord) => ord,
        }
    });

    if grouped.len() > MAX_REST_REGIONS {
        let total_count = grouped.len();
        let total_area: f64 = grouped.iter().map(Polygon2::signed_area).sum();
        let kept_area: f64 = grouped
            .iter()
            .take(MAX_REST_REGIONS)
            .map(Polygon2::signed_area)
            .sum();
        let dropped_pct = if total_area > 0.0 {
            100.0 * (total_area - kept_area) / total_area
        } else {
            0.0
        };
        warn!(
            total_count = total_count,
            kept_count = MAX_REST_REGIONS,
            dropped_area_pct = format!("{dropped_pct:.1}"),
            "Rest-region count exceeds MAX_REST_REGIONS; keeping the largest by area and \
             dropping the rest. This usually means the rest threshold is below the prior \
             pass's cusp height — raise min_valley_depth."
        );
        grouped.truncate(MAX_REST_REGIONS);
    }

    grouped
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;

    // ── region_polygons_from_mask ──────────────────────────────────────

    /// Build a `size`×`size` all-false mask with a `w`×`h` true block whose
    /// top-left corner sits at `(r0, c0)`.
    fn mask_with_block(size: usize, r0: usize, c0: usize, w: usize, h: usize) -> Grid2<bool> {
        let mut mask = Grid2::new_fill(size, size, false);
        for r in r0..r0 + h {
            for c in c0..c0 + w {
                mask.set(r, c, true);
            }
        }
        mask
    }

    fn assert_closed_ccw_polys(polys: &[Polygon2]) {
        for p in polys {
            assert!(p.closed, "region polygons must be closed");
            assert!(
                p.has_correct_winding(),
                "exterior must be CCW / holes CW: area={}",
                p.signed_area()
            );
            assert!(p.area() > 0.0, "region polygon must have positive area");
        }
    }

    /// Bounding box of a polygon's exterior (min_x, min_y, max_x, max_y).
    fn poly_bbox(p: &Polygon2) -> (f64, f64, f64, f64) {
        let xs = p.exterior.iter().map(|q| q.x);
        let ys = p.exterior.iter().map(|q| q.y);
        (
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        )
    }

    #[test]
    fn region_polygons_empty_mask_yields_empty_vec() {
        let mask = Grid2::new_fill(20, 20, false);
        let polys = region_polygons_from_mask(&mask, 0.0, 0.0, 1.0, 0.0);
        assert!(polys.is_empty(), "an all-false mask has no regions");
    }

    #[test]
    fn region_polygons_no_dilation_matches_block_extent() {
        let cell = 1.0;
        let mask = mask_with_block(20, 8, 8, 3, 3); // rows 8..11, cols 8..11
        let polys = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 0.0);
        assert_eq!(polys.len(), 1, "one isolated block -> one polygon");
        assert_closed_ccw_polys(&polys);
        let (minx, miny, maxx, maxy) = poly_bbox(&polys[0]);
        // Marching-squares boundary sits at cell edges around the 3x3 block:
        // world x/y in [8, 11] (±1 cell for corner-vs-centre geometry).
        assert!((minx - 8.0).abs() <= 1.0, "minx = {minx}");
        assert!((miny - 8.0).abs() <= 1.0, "miny = {miny}");
        assert!((maxx - 11.0).abs() <= 1.0, "maxx = {maxx}");
        assert!((maxy - 11.0).abs() <= 1.0, "maxy = {maxy}");
    }

    #[test]
    fn region_polygons_dilation_grows_the_bbox() {
        let cell = 1.0;
        let mask = mask_with_block(30, 10, 10, 3, 3);
        let base = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 0.0);
        let dilated = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 2.0);
        assert_eq!(base.len(), 1);
        assert_eq!(
            dilated.len(),
            1,
            "dilation must not fragment a single block"
        );
        assert_closed_ccw_polys(&dilated);
        let (bminx, bminy, bmaxx, bmaxy) = poly_bbox(&base[0]);
        let (dminx, dminy, dmaxx, dmaxy) = poly_bbox(&dilated[0]);
        // ~2 cells of extra margin on every side (±1 cell tolerance for MS
        // corner-vs-centre geometry).
        assert!(
            (bminx - dminx - 2.0).abs() <= 1.0,
            "left growth: base={bminx} dilated={dminx}"
        );
        assert!(
            (bminy - dminy - 2.0).abs() <= 1.0,
            "bottom growth: base={bminy} dilated={dminy}"
        );
        assert!(
            (dmaxx - bmaxx - 2.0).abs() <= 1.0,
            "right growth: base={bmaxx} dilated={dmaxx}"
        );
        assert!(
            (dmaxy - bmaxy - 2.0).abs() <= 1.0,
            "top growth: base={bmaxy} dilated={dmaxy}"
        );
    }

    #[test]
    fn region_polygons_dilation_merges_nearby_blocks() {
        let cell = 1.0;
        let mut mask = Grid2::new_fill(30, 30, false);
        // Two 3x3 blocks separated by a 3-cell gap (cols 10..13 and 16..19).
        for r in 10..13 {
            for c in 10..13 {
                mask.set(r, c, true);
            }
        }
        for r in 10..13 {
            for c in 16..19 {
                mask.set(r, c, true);
            }
        }
        let apart = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 0.0);
        assert_eq!(apart.len(), 2, "no dilation: two separate regions");

        let merged = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 2.0);
        assert_eq!(
            merged.len(),
            1,
            "dilation of 2 cells should bridge a 3-cell gap into one region"
        );
        assert_closed_ccw_polys(&merged);
    }

    #[test]
    fn region_polygons_caps_at_max_rest_regions_keeping_largest() {
        // 70 well-separated 3x3-point blocks (a threshold-below-cusp sliver
        // storm of small islands — `mask_with_block`-shaped, same block
        // geometry `region_polygons_no_dilation_matches_block_extent` above
        // already validates traces to a ~3x3 region) plus one clearly-largest
        // 15x15-point block, all spaced with a 3-cell gap so marching squares
        // never merges anything across groups. 71 total regions must be
        // capped to MAX_REST_REGIONS, and the giant block (unmistakably the
        // largest) must survive the cut.
        let cell = 1.0;
        let block = 3usize;
        let step = 6usize; // 3-cell gap between block edges: never touches, even diagonally
        let cols = 10usize;
        let rows = 7usize; // 10 * 7 = 70 small blocks
        let small_ny = (rows - 1) * step + block;
        let nx = (cols - 1) * step + block + 2;
        let giant = 15usize;
        let giant_row0 = small_ny + 6;
        let ny = giant_row0 + giant + 2;

        // +1 offset on every coordinate: marching squares cannot close a
        // loop that touches the grid boundary (production masks always carry
        // a non-contact margin ring), so blocks flush against row/col 0
        // silently trace to nothing — the exact failure this fixture first
        // shipped with (all ten row-0 blocks dropped, 61 regions, cap never
        // fired). The `+ 2` slack in nx/ny absorbs the shift.
        let mut mask = Grid2::new_fill(nx, ny, false);
        for gr in 0..rows {
            for gc in 0..cols {
                for r in 0..block {
                    for c in 0..block {
                        mask.set(gr * step + r + 1, gc * step + c + 1, true);
                    }
                }
            }
        }
        for r in 0..giant {
            for c in 0..giant {
                mask.set(giant_row0 + r, c + 1, true);
            }
        }

        let polys = region_polygons_from_mask(&mask, 0.0, 0.0, cell, 0.0);
        assert_eq!(
            polys.len(),
            MAX_REST_REGIONS,
            "71 separated blobs must be capped to MAX_REST_REGIONS"
        );
        assert_closed_ccw_polys(&polys);
        let max_area = polys.iter().map(Polygon2::area).fold(0.0, f64::max);
        assert!(
            max_area > 100.0,
            "the 15x15 giant block must survive the cap: max kept area = {max_area}"
        );
    }
}
