//! Boustrophedon (monotone-cell) decomposition of a finish region, on the
//! region's OWN raster lattice.
//!
//! # What this is, and what measured it
//!
//! Track C2 of `planning/thin_organic_2026-08-27/PROGRAMME.md`. The measured
//! winner (`FINDINGS.md` §0i Table 2, §0j/§0k verdicts) is exactly one shape:
//!
//! > **Shared-lattice monotone decomposition + ONE elongation-gated global
//! > PCA-minor rotation per region, raster pattern everywhere.**
//!
//! Per finish region: compute its PCA elongation. Above
//! [`ELONGATION_GATE`], rotate the working frame to the region's PCA-MINOR
//! axis, decompose into monotone cells **in that frame**, and raster every
//! cell on **that frame's one shared lattice**. Below the gate, decompose
//! and raster at 0°.
//!
//! # The asymmetry that is the whole point (§0i vs §0j)
//!
//! The decomposition and the lattice **share one frame, always**. §0i's
//! winner (69 cells, 755.3 s, 1.215× against the 0° undivided ceiling arm)
//! decomposes IN the rotated frame. Decomposing at 0° and then re-sweeping
//! the resulting cells at an angle is a DIFFERENT candidate, and §0j
//! measured it as a **cost**: 0.917× region 1 / 0.921× top-three, clean of
//! phase. Misaligned per-cell lattices break the cross-cell serpentine
//! chords the relinker stitches on a shared lattice (+9% cutting distance).
//!
//! Three further candidates were measured and refuted; do not reintroduce
//! them without new evidence:
//!
//! * **per-cell sweep direction** — §0j, the cost above;
//! * **cell TSP / visit order** — §0j, greedy nearest-neighbour cell order
//!   produced byte-identical output to emission order, because the
//!   production relinker's `reorder: true` already owns inter-fragment
//!   order;
//! * **contour rings per cell** — §0k, 0.686×/0.710×, worse than §0d's
//!   whole-region 0.91× refutation.
//!
//! # Why the cells are read off the LATTICE, not off `Polygon2`
//!
//! A cell here owns one contiguous run of emitted raster lattice points per
//! scan row; a split or merge of those runs closes the old cell(s) and opens
//! new one(s). That is the standard sweep-line cell-event rule (Choset &
//! Pignon 1997; Acar & Choset 2002 — `FINDINGS.md` §5), sampled at the
//! points the raster will actually emit rather than at an exact polygon
//! decomposition.
//!
//! The consequence is a **guarantee the caller must check, not assume**: the
//! cell polygons are reconstructed by marching squares, so their union must
//! be verified to select exactly the lattice population the undivided region
//! selects. [`cells_select_same_lattice`] is that check, and it is the same
//! REFUSE discipline the measurement rig ran under (`tests/
//! thin_organic_island_widths.rs`, Stage J). A caller that cannot verify it
//! must fall back to the undivided raster — a decomposition that silently
//! drops lattice points is uncut material.

use crate::contour_extract::marching_squares_bool_grid;
use crate::dropcutter::DropCutterGrid;
use crate::geo::P2;
use crate::polygon::{Polygon2, detect_containment, shoelace_area};

/// The §0f elongation gate: a region rotates to its PCA-minor axis only
/// above this.
///
/// **Not a dial.** §0f measured the PCA predictor as credible only where the
/// shape has a real axis, and this is the threshold that measurement fixed
/// (of the three largest wanaka shallow regions, only region 1 passes it, at
/// 4.15). Making it configurable would ship unmeasured surface; the dial the
/// operator gets is whether the decomposition runs at all.
pub const ELONGATION_GATE: f64 = 3.0;

/// The working frame chosen for one region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionFrame {
    /// Raster pass direction (degrees) the region's lattice is built at.
    /// `0.0` whenever the gate refuses a rotation.
    pub direction_deg: f64,
    /// The region's measured PCA elongation, when it has an axis at all.
    /// `None` means the polygon was too small to sample three interior
    /// points — **not measured**, never "round".
    pub elongation: Option<f64>,
    /// Whether [`Self::direction_deg`] came from the PCA-minor axis (gate
    /// passed) rather than from the 0° fallback.
    pub rotated: bool,
}

/// One region's decomposition on one lattice.
#[derive(Debug, Clone, Default)]
pub struct MonotoneCells {
    /// Reconstructed cell polygons, in emission order.
    ///
    /// Emission order is deliberate and sufficient: §0j measured a greedy
    /// nearest-neighbour reorder of these as byte-identical, because the
    /// production relinker reorders fragments itself.
    pub cells: Vec<Polygon2>,
    /// How many cells the sweep-line rule found, BEFORE marching squares
    /// turned each into polygons. A cell can extract to more than one loop
    /// (or, on a single-point cell, to none), so this and `cells.len()` are
    /// different measurements and both are reported.
    pub topology_cells: usize,
}

/// Nudge a requested raster direction off `batch_drop_cutter`'s dishonest
/// axis-aligned fast paths.
///
/// `batch_drop_cutter_with_cancel` skips its rotation entirely within 0.01°
/// of 0 / 90 / 180 / 360 and returns an **axis-aligned** grid still labelled
/// with the requested angle (`dropcutter.rs:118-165`, kept for backward
/// compatibility). At 0° that fast path is honest — the rotation there IS
/// the identity — but at 90° and 180° the grid's `u_start` / `v_start` and
/// its `direction_deg` then disagree, which would make this module's
/// grid-frame → world mapping wrong by a right angle.
///
/// So: fold 180° back onto 0° (the same axis, and 0° is honest), and nudge
/// 90° to 89.9°. That is the measurement rig's own precedent
/// (`tests/thin_organic_island_widths.rs:1164-1170`). The fast path's
/// semantics are deliberately NOT changed here — other callers depend on
/// them.
#[must_use]
pub fn honest_raster_direction_deg(direction_deg: f64) -> f64 {
    const SNAP: f64 = 0.05;
    const NUDGED_RIGHT_ANGLE: f64 = 89.9;
    let folded = direction_deg.rem_euclid(180.0);
    if !(SNAP..=180.0 - SNAP).contains(&folded) {
        return 0.0;
    }
    if (folded - 90.0).abs() < SNAP {
        return NUDGED_RIGHT_ANGLE;
    }
    folded
}

/// The region's PCA-minor axis (degrees, in `[0, 180)`) and its elongation.
///
/// Sampled on a regular `sample_mm` grid over the polygon's interior — the
/// same construction the C1/§0f measurement used, so a production frame and
/// the measured one agree. `None` when fewer than three interior samples
/// land, which is **not measured**, not "no axis".
///
/// The MINOR axis is the pass direction: passes run ACROSS the narrow
/// dimension so each pass is long and the number of passes is small.
#[must_use]
pub fn pca_minor_and_elongation(polygon: &Polygon2, sample_mm: f64) -> Option<(f64, f64)> {
    if sample_mm <= 0.0 || !sample_mm.is_finite() {
        return None;
    }
    let [x0, y0, x1, y1] = polygon.bbox();
    let nx = (((x1 - x0) / sample_mm).ceil() as usize).saturating_add(2);
    let ny = (((y1 - y0) / sample_mm).ceil() as usize).saturating_add(2);
    let mut points = Vec::new();
    for row in 0..ny {
        for col in 0..nx {
            let point = P2::new(x0 + col as f64 * sample_mm, y0 + row as f64 * sample_mm);
            if polygon.contains_point(&point) {
                points.push(point);
            }
        }
    }
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let (sum_x, sum_y) = points
        .iter()
        .fold((0.0_f64, 0.0_f64), |(sx, sy), p| (sx + p.x, sy + p.y));
    let (cx, cy) = (sum_x / n, sum_y / n);
    let (sxx, syy, sxy) = points
        .iter()
        .fold((0.0_f64, 0.0_f64, 0.0_f64), |(xx, yy, xy), p| {
            let (dx, dy) = (p.x - cx, p.y - cy);
            (xx + dx * dx, yy + dy * dy, xy + dx * dy)
        });
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let major = 0.5 * (2.0 * sxy).atan2(sxx - syy).to_degrees();
    let minor = (major + 90.0).rem_euclid(180.0);
    let trace = sxx + syy;
    let determinant = sxx * syy - sxy * sxy;
    let spread = ((trace * trace / 4.0) - determinant).max(0.0).sqrt();
    let large = trace / 2.0 + spread;
    let small = trace / 2.0 - spread;
    if small <= 1e-9 {
        return Some((minor, f64::INFINITY));
    }
    Some((minor, (large / small).sqrt()))
}

/// Choose one region's working frame: PCA-minor above [`ELONGATION_GATE`],
/// 0° otherwise.
///
/// The returned angle is already passed through
/// [`honest_raster_direction_deg`], so it may be fed straight to
/// `batch_drop_cutter*` without landing on a dishonest fast path.
#[must_use]
pub fn region_frame(polygon: &Polygon2, sample_mm: f64) -> RegionFrame {
    match pca_minor_and_elongation(polygon, sample_mm) {
        Some((minor, elongation)) if elongation > ELONGATION_GATE => RegionFrame {
            direction_deg: honest_raster_direction_deg(minor),
            elongation: Some(elongation),
            rotated: true,
        },
        Some((_, elongation)) => RegionFrame {
            direction_deg: 0.0,
            elongation: Some(elongation),
            rotated: false,
        },
        None => RegionFrame {
            direction_deg: 0.0,
            elongation: None,
            rotated: false,
        },
    }
}

/// Map a point from the grid's sampling (U/V) frame into world XY.
///
/// Only correct because every angle this module hands to the grid builder
/// went through [`honest_raster_direction_deg`] first — see its doc.
fn grid_frame_to_world(grid: &DropCutterGrid, point: P2) -> P2 {
    let angle = grid.direction_deg.to_radians();
    let (cosine, sine) = (angle.cos(), angle.sin());
    P2::new(
        point.x * cosine - point.y * sine,
        point.x * sine + point.y * cosine,
    )
}

/// The maximal contiguous runs of `true` in one row of a row-major mask.
fn runs_in_row(mask: &[bool], row: usize, cols: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for col in 0..cols {
        let inside = mask.get(row * cols + col).copied().unwrap_or(false);
        if inside && start.is_none() {
            start = Some(col);
        } else if !inside && let Some(first) = start.take() {
            out.push((first, col.saturating_sub(1)));
        }
    }
    if let Some(first) = start {
        out.push((first, cols.saturating_sub(1)));
    }
    out
}

const fn runs_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

/// One sweep-line run, carrying the cell it belongs to.
struct GridRun {
    start: usize,
    end: usize,
    cell: usize,
}

/// Reconstruct polygons for one cell's lattice-point set.
///
/// The mask is padded by one ring so marching squares always closes a loop
/// around a cell that touches the grid edge. Loops come out in the grid's
/// sampling frame and are mapped back to world XY, because a rotated grid's
/// `u`/`v` are not world `x`/`y`.
///
/// **No area floor.** A one-lattice-point cell is kept: the membership check
/// refuses any candidate that loses a baseline emitted point, so dropping
/// small cells here would hide a coverage loss rather than prevent one.
fn polygons_for_lattice_cell(grid: &DropCutterGrid, positions: &[usize]) -> Vec<Polygon2> {
    if grid.cols == 0 {
        return Vec::new();
    }
    let rows = grid.rows.saturating_add(2);
    let cols = grid.cols.saturating_add(2);
    let mut mask = vec![false; rows * cols];
    for &position in positions {
        let row = position / grid.cols;
        let col = position % grid.cols;
        if let Some(slot) = mask.get_mut((row + 1) * cols + col + 1) {
            *slot = true;
        }
    }
    let loops = marching_squares_bool_grid(
        &mask,
        rows,
        cols,
        grid.u_start - grid.x_step,
        grid.v_start - grid.y_step,
        grid.x_step,
    );
    let candidates: Vec<Polygon2> = loops
        .into_iter()
        .filter(|points| points.len() >= 3 && shoelace_area(points).abs() > 1e-12)
        .map(|points| {
            Polygon2::new(
                points
                    .into_iter()
                    .map(|point| grid_frame_to_world(grid, point))
                    .collect(),
            )
        })
        .collect();
    let mut polygons = detect_containment(candidates);
    for polygon in &mut polygons {
        polygon.ensure_winding();
    }
    polygons
}

/// Decompose `boundary` into monotone cells **on `grid`'s own lattice**.
///
/// A lattice point belongs to the region when its drop-cutter Z is above
/// `min_z` (i.e. the raster will emit it) and it lies inside the boundary —
/// the same two predicates `raster_toolpath_from_grid` applies, so the cell
/// map and the emission agree by construction. `min_z` must therefore be the
/// caller's *effective* floor, including any `stock_to_leave` lift.
///
/// The scan direction is the grid's: cells are monotone in the grid's row
/// direction, so a raster pass inside one cell cannot fragment.
#[must_use]
pub fn lattice_monotone_cells(
    grid: &DropCutterGrid,
    boundary: &Polygon2,
    min_z: f64,
) -> MonotoneCells {
    /// Guard band above the emission floor, mirroring
    /// `raster_toolpath_from_grid`'s own filter.
    const CLAMP_EPS: f64 = 0.001;

    if grid.rows == 0 || grid.cols == 0 {
        return MonotoneCells::default();
    }
    let mut inside = vec![false; grid.rows * grid.cols];
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let point = grid.get(row, col);
            if let Some(slot) = inside.get_mut(row * grid.cols + col) {
                *slot = point.z > min_z + CLAMP_EPS
                    && boundary.contains_point(&P2::new(point.x, point.y));
            }
        }
    }

    let mut positions: Vec<Vec<usize>> = Vec::new();
    let mut previous: Vec<GridRun> = Vec::new();
    for row in 0..grid.rows {
        let current_runs = runs_in_row(&inside, row, grid.cols);
        // How many of THIS row's runs each previous run touches. A previous
        // run that touches two is a SPLIT and closes its cell, so the
        // one-to-one test below needs both directions.
        let old_to_new: Vec<usize> = previous
            .iter()
            .map(|old| {
                current_runs
                    .iter()
                    .filter(|run| runs_overlap((old.start, old.end), **run))
                    .count()
            })
            .collect();
        let mut current = Vec::new();
        for run in current_runs {
            let connected: Vec<usize> = previous
                .iter()
                .enumerate()
                .filter(|(_, old)| runs_overlap((old.start, old.end), run))
                .map(|(i, _)| i)
                .collect();
            // A run continues its predecessor's cell only across a
            // ONE-TO-ONE overlap: exactly one previous run reaches it, and
            // that previous run reaches only this one. Any other shape is a
            // split or a merge, and closes the old cell(s).
            let continues = match connected.as_slice() {
                [only] if old_to_new.get(*only).copied() == Some(1) => {
                    previous.get(*only).map(|old| old.cell)
                }
                _ => None,
            };
            let cell = match continues {
                Some(cell) => cell,
                None => {
                    positions.push(Vec::new());
                    positions.len().saturating_sub(1)
                }
            };
            for col in run.0..=run.1 {
                if let Some(cell_positions) = positions.get_mut(cell) {
                    cell_positions.push(row * grid.cols + col);
                }
            }
            current.push(GridRun {
                start: run.0,
                end: run.1,
                cell,
            });
        }
        previous = current;
    }

    let topology_cells = positions.len();
    let cells = positions
        .iter()
        .flat_map(|positions| polygons_for_lattice_cell(grid, positions))
        .collect();
    MonotoneCells {
        cells,
        topology_cells,
    }
}

/// How many of `grid`'s lattice points the cell set and the undivided
/// boundary DISAGREE about.
///
/// `0` is the only shippable answer: the cells are a reconstruction, and a
/// candidate that selects a different lattice population is either leaving
/// material uncut or cutting outside its region. Callers must fall back to
/// the undivided raster on a non-zero count and report it.
///
/// This is the exact guard the measurement rig imposed on every cell arm it
/// costed (Stage J, `FINDINGS.md` §0g) — the same frame on both sides, so it
/// is an exact test rather than the coverage proxy §0j had to fall back on.
#[must_use]
pub fn cells_select_same_lattice(
    grid: &DropCutterGrid,
    boundary: &Polygon2,
    cells: &[Polygon2],
    min_z: f64,
) -> usize {
    const CLAMP_EPS: f64 = 0.001;

    let cells = crate::region_set::RegionSet::new(cells.to_vec());
    let mut mismatches = 0usize;
    for point in &grid.points {
        let emitted = point.z > min_z + CLAMP_EPS;
        let baseline = emitted && boundary.contains_point(&P2::new(point.x, point.y));
        let candidate = emitted && cells.contains(&P2::new(point.x, point.y));
        if baseline != candidate {
            mismatches = mismatches.saturating_add(1);
        }
    }
    mismatches
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
    use crate::tool::CLPoint;

    /// A synthetic 0°-frame grid whose Z is `1.0` inside `inside` and the
    /// floor elsewhere. Lets the cell rule be tested without a mesh.
    fn grid_from_mask(rows: usize, cols: usize, step: f64, inside: &[bool]) -> DropCutterGrid {
        let mut points = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            for col in 0..cols {
                let z = if inside[row * cols + col] { 1.0 } else { -1.0 };
                points.push(CLPoint {
                    x: col as f64 * step,
                    y: row as f64 * step,
                    z,
                    contacted: inside[row * cols + col],
                });
            }
        }
        DropCutterGrid {
            points,
            rows,
            cols,
            u_start: 0.0,
            v_start: 0.0,
            x_step: step,
            y_step: step,
            direction_deg: 0.0,
        }
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::new(vec![
            P2::new(x0, y0),
            P2::new(x1, y0),
            P2::new(x1, y1),
            P2::new(x0, y1),
        ])
    }

    #[test]
    fn the_right_angle_fast_path_is_nudged_and_zero_is_left_alone() {
        // 0° and 180° are the same axis and 0°'s fast path is the honest
        // identity, so both fold to exactly 0.0.
        assert!((honest_raster_direction_deg(0.0) - 0.0).abs() < 1e-12);
        assert!((honest_raster_direction_deg(180.0) - 0.0).abs() < 1e-12);
        assert!((honest_raster_direction_deg(359.999) - 0.0).abs() < 1e-12);
        // 90° is the dishonest one: an axis-aligned grid labelled 90.
        assert!((honest_raster_direction_deg(90.0) - 89.9).abs() < 1e-12);
        // Everything else passes through, folded into [0, 180).
        assert!((honest_raster_direction_deg(119.6) - 119.6).abs() < 1e-9);
        assert!((honest_raster_direction_deg(-60.4) - 119.6).abs() < 1e-9);
    }

    #[test]
    fn an_elongated_rectangle_selects_its_minor_axis() {
        // 40 x 4 along world X: the MAJOR axis is X (0°), so the pass
        // direction — the minor axis — is 90°, which the nudge moves to
        // 89.9° rather than letting the fast path lie about it.
        let frame = region_frame(&rect(0.0, 0.0, 40.0, 4.0), 0.5);
        assert!(frame.rotated, "elongation 10:1 must clear the gate");
        assert!((frame.direction_deg - 89.9).abs() < 1e-9);
        assert!(frame.elongation.unwrap() > ELONGATION_GATE);
    }

    #[test]
    fn a_square_region_stays_at_zero_degrees() {
        let frame = region_frame(&rect(0.0, 0.0, 20.0, 20.0), 0.5);
        assert!(!frame.rotated);
        assert!((frame.direction_deg - 0.0).abs() < 1e-12);
        assert!(frame.elongation.unwrap() < ELONGATION_GATE);
    }

    #[test]
    fn a_two_lobe_region_decomposes_into_more_than_one_cell() {
        // A "U": two legs joined along the bottom two rows. Sweeping rows
        // bottom-to-top, the single bottom run SPLITS into two — the
        // canonical boustrophedon event.
        let (rows, cols, step) = (8usize, 9usize, 1.0);
        let mut mask = vec![false; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let legs = col <= 2 || col >= 6;
                let base = row <= 1;
                mask[row * cols + col] = base || legs;
            }
        }
        let grid = grid_from_mask(rows, cols, step, &mask);
        let boundary = rect(-0.5, -0.5, 8.5, 7.5);
        let decomposed = lattice_monotone_cells(&grid, &boundary, 0.0);
        assert!(
            decomposed.topology_cells > 1,
            "a split must open new cells, got {}",
            decomposed.topology_cells
        );
        assert_eq!(
            cells_select_same_lattice(&grid, &boundary, &decomposed.cells, 0.0),
            0,
            "the cells must select exactly the undivided lattice population"
        );
    }

    #[test]
    fn a_simple_convex_region_is_one_cell_and_membership_is_exact() {
        let (rows, cols, step) = (6usize, 6usize, 1.0);
        let mask = vec![true; rows * cols];
        let grid = grid_from_mask(rows, cols, step, &mask);
        let boundary = rect(-0.5, -0.5, 5.5, 5.5);
        let decomposed = lattice_monotone_cells(&grid, &boundary, 0.0);
        assert_eq!(decomposed.topology_cells, 1);
        assert_eq!(
            cells_select_same_lattice(&grid, &boundary, &decomposed.cells, 0.0),
            0
        );
    }

    #[test]
    fn an_empty_region_yields_no_cells_rather_than_a_phantom_one() {
        let (rows, cols, step) = (4usize, 4usize, 1.0);
        let mask = vec![false; rows * cols];
        let grid = grid_from_mask(rows, cols, step, &mask);
        let boundary = rect(-0.5, -0.5, 3.5, 3.5);
        let decomposed = lattice_monotone_cells(&grid, &boundary, 0.0);
        assert_eq!(decomposed.topology_cells, 0);
        assert!(decomposed.cells.is_empty());
    }
}
