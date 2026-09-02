//! The union-coverage audit — G-UNIONCOV's fix
//! (`planning/finishing_status_2026-09-01.md` §12).
//!
//! # Why this exists
//!
//! Every shipped coverage finding is measured PER OP against that op's own
//! boundary. A multi-op tier chain can have every op clean inside its
//! territory while strips or patches between the territories belong to no
//! op. The `wanaka200_mt2_overlap02` variant read clean on every wire
//! (`untouched_material_mm2 0.0`, zero collisions, air-cut OK) while
//! leaving standing material across whole patches of the fine-detail
//! territory; the operator caught it by eye. This audit is the missing
//! ruler: ONE whole-board comparison of the final simulated stock against
//! the target surface plus `stock_to_leave`, per project.
//!
//! # What it measures, exactly
//!
//! For every column of the final stock's dexel z-grid that has model
//! surface under it:
//!
//! ```text
//! standing = stock_top_z − (model_top_z + stock_to_leave)
//! ```
//!
//! * `standing > spec_tolerance` — the cell is ABOVE SPEC: material no op
//!   removed. Summed into `above_spec_area_mm2` and clustered into located
//!   [`Hotspot`]s.
//! * `standing < −gouge_tolerance` — the cell is GOUGED below the target.
//! * a column with model under it and NO material at all is counted in
//!   `cut_through_cells` (also a gouge class — the stock was severed).
//!
//! `standing` is VERTICAL (column-frame). On a wall at slope `θ` the
//! surface-normal excess is `standing × cos θ` — the same convention as
//! the dexel COLUMNS instrument, stated so nobody reads a wall column as a
//! flat-ground defect.
//!
//! # What it deliberately does NOT reuse
//!
//! `SimulationResult::column_deviations` looks like this measurement but
//! is not: `collect_column_deviations` drops any column whose stock top is
//! farther than `max(model_thickness/2, 2 mm)` from BOTH model faces — a
//! relevance filter for deviation heat-maps. Standing material taller than
//! that filter (a fully-missed patch at full stock height over a valley)
//! silently leaves that population. This audit walks every grid column and
//! has no relevance filter.
//!
//! # Resolution-conditional (measurement contract, rule 4)
//!
//! Areas are XY-projected cell counts × `cell_mm²` on the SIMULATION grid.
//! The report carries `cell_mm`; never compare two reports taken at
//! different resolutions. Areas are projected XY, not 3-D surface areas.
//!
//! # Frames
//!
//! The stock and the model must be handed to [`audit_stock_vs_model`] in
//! the SAME frame. For a session simulation the final checkpoint stock is
//! in the zero-rooted stock-relative global frame, so the caller
//! translates the world model by `−stock_origin` — the same contract as
//! `SimulationRequest::model_mesh`.

//! # Per-owner attribution
//!
//! [`super::ownership::attribute_above_spec`] partitions this audit's
//! above-spec population by band-cell ownership (Track H V1 finding 2): a
//! region POLYGON is not a band's territory, so an audit that reasons over
//! polygons reads a Shallow op as failed on steep inclusions it never
//! owned. The partition's `unowned` class is the located G-UNIONCOV gap
//! signal.

use crate::dexel_stock::TriDexelStock;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::metrology::monge::surface_z;

use crate::geo::P2;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Dials for the audit. No defaults on purpose: `stock_to_leave` and the
/// spec tolerance are project statements, not constants of the ruler.
#[derive(Debug, Clone, Copy)]
pub struct UnionCoverageParams {
    /// The project's commanded stock-to-leave (mm, vertical). The target
    /// surface is `model + this`.
    pub stock_to_leave_mm: f64,
    /// Standing above the target beyond this is ABOVE SPEC (mm). Set it
    /// from the finish spec (for example the commanded cusp height plus
    /// one sim cell of quantisation).
    pub spec_tolerance_mm: f64,
    /// Below `−this`, a cell is counted as gouged (mm).
    pub gouge_tolerance_mm: f64,
    /// Hotspots reported (largest by area first). The counts and areas
    /// always cover ALL hotspots; only the located list is capped.
    pub max_hotspots: usize,
}

/// One connected above-spec patch, located.
#[derive(Debug, Clone, Copy)]
pub struct Hotspot {
    /// XY-projected area (mm²).
    pub area_mm2: f64,
    pub cells: usize,
    /// Worst vertical standing in the patch (mm above target + spec).
    pub max_standing_mm: f64,
    /// Area-centroid of the patch, in the audited frame (mm).
    pub centroid: P2,
    /// XY bbox of the patch, `(min_x, min_y, max_x, max_y)` (mm).
    pub bbox: [f64; 4],
}

/// The whole-board verdict. Every count is measured on every call — a zero
/// is a measured zero (measurement contract, rule 1).
#[derive(Debug, Clone)]
pub struct UnionCoverageReport {
    /// The grid cell the audit ran on (mm). Areas scale with it.
    pub cell_mm: f64,
    pub stock_to_leave_mm: f64,
    pub spec_tolerance_mm: f64,
    pub gouge_tolerance_mm: f64,
    /// Columns with model surface under them — the audit population.
    pub compared_cells: usize,
    /// XY-projected area of the population (mm²).
    pub compared_area_mm2: f64,
    /// Grid columns with NO model under them (rim margin, outside the
    /// footprint). Not defects; counted so the population is accountable.
    pub off_model_cells: usize,
    /// Cells standing above target + tolerance.
    pub above_spec_cells: usize,
    pub above_spec_area_mm2: f64,
    /// `above_spec_area_mm2 / compared_area_mm2`.
    pub above_spec_fraction: f64,
    /// Worst vertical standing over the population (mm; can be negative
    /// when the whole board is at or below target).
    pub max_standing_mm: f64,
    /// p99 of vertical standing over the population (mm).
    pub p99_standing_mm: f64,
    /// Cells cut below target − gouge tolerance.
    pub gouged_cells: usize,
    pub gouged_area_mm2: f64,
    /// Deepest vertical undercut (mm, positive number).
    pub max_undercut_mm: f64,
    /// Columns with model under them and no stock material at all.
    pub cut_through_cells: usize,
    /// Above-spec patches, largest first, capped at
    /// [`UnionCoverageParams::max_hotspots`].
    pub hotspots: Vec<Hotspot>,
    /// Total patch count before the cap.
    pub hotspot_count: usize,
}

/// The loud failure a comparison instrument gets from
/// [`UnionCoverageReport::assert_within`]. Its `Display` prints the areas
/// and every reported hotspot, so a failing arm names WHERE it failed.
#[derive(Debug, Clone)]
pub struct UnionCoverageFailure {
    pub above_spec_area_mm2: f64,
    pub allowed_mm2: f64,
    pub report: UnionCoverageReport,
}

impl std::fmt::Display for UnionCoverageFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "UNION COVERAGE FAILED: {:.1} mm2 above spec (allowed {:.1} mm2), \
             {:.2} % of {:.0} mm2 compared at cell {:.3} mm; max standing {:.3} mm, \
             {} hotspots:",
            self.above_spec_area_mm2,
            self.allowed_mm2,
            100.0 * self.report.above_spec_fraction,
            self.report.compared_area_mm2,
            self.report.cell_mm,
            self.report.max_standing_mm,
            self.report.hotspot_count,
        )?;
        for h in &self.report.hotspots {
            writeln!(
                f,
                "  {:.1} mm2 at ({:.1}, {:.1}), bbox [{:.1}, {:.1}]..[{:.1}, {:.1}], \
                 max standing {:.3} mm",
                h.area_mm2,
                h.centroid.x,
                h.centroid.y,
                h.bbox[0],
                h.bbox[1],
                h.bbox[2],
                h.bbox[3],
                h.max_standing_mm,
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for UnionCoverageFailure {}

impl UnionCoverageReport {
    /// Loud threshold gate for comparison runs: `Err` when the above-spec
    /// area exceeds `allowed_mm2`, carrying the located hotspots. An
    /// instrument unwraps this so a failing arm fails NAMED.
    pub fn assert_within(&self, allowed_mm2: f64) -> Result<(), Box<UnionCoverageFailure>> {
        if self.above_spec_area_mm2 <= allowed_mm2 {
            Ok(())
        } else {
            Err(Box::new(UnionCoverageFailure {
                above_spec_area_mm2: self.above_spec_area_mm2,
                allowed_mm2,
                report: self.clone(),
            }))
        }
    }
}

/// Audit a final simulated stock against a model, whole board.
///
/// `stock` and `model` must share a frame (see the module doc). `index` is
/// the model's spatial index.
#[must_use]
pub fn audit_stock_vs_model(
    stock: &TriDexelStock,
    model: &TriangleMesh,
    index: &SpatialIndex,
    params: &UnionCoverageParams,
) -> UnionCoverageReport {
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let (rows, cols) = (grid.rows, grid.cols);

    // Per-column standing, row-major. `NAN` = off-model; `-INFINITY` = model
    // under the column but the stock is cut through (no material).
    let standing_row = |row: usize| -> Vec<f64> {
        (0..cols)
            .map(|col| {
                let (u, v) = grid.cell_to_world(row, col);
                let Some(model_top) = surface_z(model, index, P2::new(u, v)) else {
                    return f64::NAN;
                };
                match grid.top_z_at(row, col) {
                    Some(top) => f64::from(top) - (model_top + params.stock_to_leave_mm),
                    None => f64::NEG_INFINITY,
                }
            })
            .collect()
    };
    #[cfg(feature = "parallel")]
    let standing: Vec<Vec<f64>> = (0..rows).into_par_iter().map(standing_row).collect();
    #[cfg(not(feature = "parallel"))]
    let standing: Vec<Vec<f64>> = (0..rows).map(standing_row).collect();

    let at = |row: usize, col: usize| -> f64 {
        standing
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(f64::NAN)
    };

    let mut compared_cells = 0usize;
    let mut off_model_cells = 0usize;
    let mut cut_through_cells = 0usize;
    let mut gouged_cells = 0usize;
    let mut above_spec_cells = 0usize;
    let mut max_standing = f64::NEG_INFINITY;
    let mut max_undercut = 0.0f64;
    let mut finite_standing: Vec<f64> = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let s = at(row, col);
            if s.is_nan() {
                off_model_cells += 1;
                continue;
            }
            compared_cells += 1;
            if s == f64::NEG_INFINITY {
                cut_through_cells += 1;
                gouged_cells += 1;
                continue;
            }
            finite_standing.push(s);
            if s > max_standing {
                max_standing = s;
            }
            if s > params.spec_tolerance_mm {
                above_spec_cells += 1;
            }
            if s < -params.gouge_tolerance_mm {
                gouged_cells += 1;
                max_undercut = max_undercut.max(-s);
            }
        }
    }
    finite_standing.sort_by(f64::total_cmp);
    let p99 = crate::metrology::spacing::quantile(&finite_standing, 0.99);

    // Hotspots: 4-neighbour flood over above-spec cells.
    let above = |row: usize, col: usize| -> bool { at(row, col) > params.spec_tolerance_mm };
    let mut visited = vec![false; rows * cols];
    let mut hotspots: Vec<Hotspot> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            if visited.get(idx).copied().unwrap_or(true) || !above(row, col) {
                continue;
            }
            let mut cells = 0usize;
            let mut worst = f64::NEG_INFINITY;
            let mut sum_x = 0.0f64;
            let mut sum_y = 0.0f64;
            let mut bbox = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            stack.push((row, col));
            if let Some(v) = visited.get_mut(idx) {
                *v = true;
            }
            while let Some((r, c)) = stack.pop() {
                cells += 1;
                let s = at(r, c);
                if s > worst {
                    worst = s;
                }
                let (x, y) = grid.cell_to_world(r, c);
                sum_x += x;
                sum_y += y;
                bbox[0] = bbox[0].min(x);
                bbox[1] = bbox[1].min(y);
                bbox[2] = bbox[2].max(x);
                bbox[3] = bbox[3].max(y);
                let neighbours = [
                    (r.wrapping_sub(1), c),
                    (r + 1, c),
                    (r, c.wrapping_sub(1)),
                    (r, c + 1),
                ];
                for (nr, nc) in neighbours {
                    if nr >= rows || nc >= cols {
                        continue;
                    }
                    let nidx = nr * cols + nc;
                    if visited.get(nidx).copied().unwrap_or(true) || !above(nr, nc) {
                        continue;
                    }
                    if let Some(v) = visited.get_mut(nidx) {
                        *v = true;
                    }
                    stack.push((nr, nc));
                }
            }
            hotspots.push(Hotspot {
                area_mm2: cells as f64 * cell * cell,
                cells,
                max_standing_mm: worst,
                centroid: P2::new(sum_x / cells as f64, sum_y / cells as f64),
                bbox,
            });
        }
    }
    hotspots.sort_by(|a, b| b.area_mm2.total_cmp(&a.area_mm2));
    let hotspot_count = hotspots.len();
    hotspots.truncate(params.max_hotspots);

    let cell_area = cell * cell;
    let compared_area_mm2 = compared_cells as f64 * cell_area;
    let above_spec_area_mm2 = above_spec_cells as f64 * cell_area;
    UnionCoverageReport {
        cell_mm: cell,
        stock_to_leave_mm: params.stock_to_leave_mm,
        spec_tolerance_mm: params.spec_tolerance_mm,
        gouge_tolerance_mm: params.gouge_tolerance_mm,
        compared_cells,
        compared_area_mm2,
        off_model_cells,
        above_spec_cells,
        above_spec_area_mm2,
        above_spec_fraction: if compared_area_mm2 > 0.0 {
            above_spec_area_mm2 / compared_area_mm2
        } else {
            f64::NAN
        },
        max_standing_mm: if finite_standing.is_empty() {
            f64::NAN
        } else {
            max_standing
        },
        p99_standing_mm: p99,
        gouged_cells,
        gouged_area_mm2: gouged_cells as f64 * cell_area,
        max_undercut_mm: max_undercut,
        cut_through_cells,
        hotspots,
        hotspot_count,
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
    use super::{UnionCoverageParams, audit_stock_vs_model};
    use crate::dexel_stock::TriDexelStock;
    use crate::geo::{BoundingBox3, P3};
    use crate::mesh::{SpatialIndex, TriangleMesh};

    fn flat_model(size: f64, z: f64) -> TriangleMesh {
        TriangleMesh::from_raw(
            vec![
                P3::new(0.0, 0.0, z),
                P3::new(size, 0.0, z),
                P3::new(size, size, z),
                P3::new(0.0, size, z),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    fn params() -> UnionCoverageParams {
        UnionCoverageParams {
            stock_to_leave_mm: 0.0,
            spec_tolerance_mm: 0.05,
            gouge_tolerance_mm: 0.05,
            max_hotspots: 8,
        }
    }

    /// New pin: uncut stock over a flat target is one hotspot covering the
    /// footprint; stock cleared exactly to the target is clean; a stock
    /// cleared below it reads gouged, never above-spec.
    #[test]
    fn audit_separates_standing_clean_and_gouged() {
        let size = 10.0;
        let model = flat_model(size, 5.0);
        let index = SpatialIndex::build_auto(&model);
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(size, size, 8.0),
        };

        // Arm 1: untouched stock — 3 mm standing everywhere on the model.
        let stock = TriDexelStock::from_bounds(&bbox, 0.5);
        let report = audit_stock_vs_model(&stock, &model, &index, &params());
        assert!(report.compared_cells > 0);
        assert!(
            report.above_spec_fraction > 0.99,
            "{}",
            report.above_spec_fraction
        );
        assert!((report.max_standing_mm - 3.0).abs() < 0.51); // one cell slack
        assert_eq!(report.hotspot_count, 1);
        assert!(report.assert_within(1.0).is_err());
        // The loud failure names the hotspot.
        let failure = report.assert_within(1.0).unwrap_err();
        assert!(format!("{failure}").contains("hotspots"));

        // Arm 2: cleared exactly to the target.
        let mut cleared = TriDexelStock::from_bounds(&bbox, 0.5);
        for row in 0..cleared.z_grid.rows {
            for col in 0..cleared.z_grid.cols {
                cleared.clear_above_at(row, col, 5.0);
            }
        }
        let report = audit_stock_vs_model(&cleared, &model, &index, &params());
        assert_eq!(report.above_spec_cells, 0);
        assert_eq!(report.gouged_cells, 0);
        assert!(report.assert_within(0.0).is_ok());

        // Arm 3: cleared 1 mm below the target — gouged, not above-spec.
        let mut gouged = TriDexelStock::from_bounds(&bbox, 0.5);
        for row in 0..gouged.z_grid.rows {
            for col in 0..gouged.z_grid.cols {
                gouged.clear_above_at(row, col, 4.0);
            }
        }
        let report = audit_stock_vs_model(&gouged, &model, &index, &params());
        assert_eq!(report.above_spec_cells, 0);
        assert!(report.gouged_cells > 0);
        assert!((report.max_undercut_mm - 1.0).abs() < 0.51);
    }
}
