//! Band-cell ownership — the planner's label grid as a queryable map, and
//! the union audit's per-owner attribution.
//!
//! # Why this exists (Track H V1 finding 2)
//!
//! A band's REGION POLYGON is not its territory. The `overlap_mm` dilation
//! grows each polygon over neighbouring bands' cells, extraction can fill
//! small holes, and min-area absorption relabels small islands — so a
//! Shallow region polygon can hold large steep inclusions the Shallow band
//! never claimed. On the wanaka V1 region, 42.9 % of the polygon's 3D area
//! is steeper than the 45° derate clamp. A coverage audit that reasons over
//! region polygons therefore reads a healthy Shallow op as ~half failed —
//! by construction, not by measurement
//! (`planning/valley_tracing_2026-09-02/FINDINGS.md`, V1 findings 1–2).
//!
//! The authoritative territory statement is the planner's
//! post-conditioning CELL label grid (`PlannedRegions::labels`).
//! [`BandOwnership`] wraps that grid with its frame so any audit can ask
//! "which band owns this ground" at any XY point, independent of the
//! audit's own resolution.
//!
//! # Attribution, not exoneration
//!
//! [`attribute_above_spec`] charges each above-spec cell of the
//! whole-board union audit ([`super::union_coverage`]) to the band that
//! owns the ground under it. Ground owned by NO band lands in
//! [`OwnershipAttribution::unowned`] — that class IS the G-UNIONCOV
//! union-gap signal, now measured instead of inferred. The attribution
//! never shrinks the union total: the classes partition
//! `above_spec_cells`, and the unowned class is a finding, not a filter.
//!
//! # Resolution and frames (measurement contract, rule 4)
//!
//! The ownership grid keeps the CLASSIFICATION cell size; the audit runs on
//! the SIMULATION grid. [`BandOwnership::owner_at`] answers by nearest
//! classification cell, so the attribution is resolution-independent up to
//! half a classification cell at band boundaries. The ownership grid and
//! the audited stock must share a frame — for a session simulation both
//! sides are handled in the same zero-rooted stock frame as
//! `audit_stock_vs_model` (see that module's Frames note).

use crate::dexel_stock::TriDexelStock;
use crate::finish_planner::FinishBand;
use crate::geo::P2;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::metrology::monge::surface_z;
use crate::metrology::union_coverage::UnionCoverageParams;
use crate::slope::SlopeMap;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// The planner's post-conditioning band label grid, with its frame.
///
/// Build it from the same `decompose` output the op planned with. `labels`
/// is row-major `rows × cols`; `None` = the cell is not covered.
#[derive(Debug, Clone)]
pub struct BandOwnership {
    labels: Vec<Option<FinishBand>>,
    rows: usize,
    cols: usize,
    cell_mm: f64,
    origin_x: f64,
    origin_y: f64,
}

impl BandOwnership {
    /// Wrap a decomposition's label grid with the slope map that framed it.
    ///
    /// Returns `None` when the label grid is empty (a `decompose` early
    /// return) or does not match the slope map's dimensions — an empty
    /// ownership map would silently attribute everything to `unowned`.
    #[must_use]
    pub fn from_labels(labels: Vec<Option<FinishBand>>, slope_map: &SlopeMap) -> Option<Self> {
        if labels.is_empty() || labels.len() != slope_map.rows * slope_map.cols {
            return None;
        }
        Some(Self {
            labels,
            rows: slope_map.rows,
            cols: slope_map.cols,
            cell_mm: slope_map.cell_size,
            origin_x: slope_map.origin_x,
            origin_y: slope_map.origin_y,
        })
    }

    /// The band owning the ground at `p`, by nearest classification cell.
    /// `None` = no band owns it (uncovered, or outside the grid).
    #[must_use]
    pub fn owner_at(&self, p: P2) -> Option<FinishBand> {
        let col = ((p.x - self.origin_x) / self.cell_mm).round();
        let row = ((p.y - self.origin_y) / self.cell_mm).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (col, row) = (col as usize, row as usize);
        if row >= self.rows || col >= self.cols {
            return None;
        }
        self.labels.get(row * self.cols + col).copied().flatten()
    }

    /// XY-projected area (mm²) the given band owns — cell count × cell².
    /// Classification-grid resolution-conditional.
    #[must_use]
    pub fn owned_area_mm2(&self, band: FinishBand) -> f64 {
        let cells = self.labels.iter().filter(|l| **l == Some(band)).count();
        cells as f64 * self.cell_mm * self.cell_mm
    }

    /// The classification cell size this map answers at (mm).
    #[must_use]
    pub fn cell_mm(&self) -> f64 {
        self.cell_mm
    }
}

/// One owner class's share of the above-spec population.
#[derive(Debug, Clone, Copy, Default)]
pub struct OwnedAboveSpec {
    pub cells: usize,
    /// XY-projected, on the AUDIT grid's cell size.
    pub area_mm2: f64,
    /// Worst vertical standing in the class (mm). `f64::NEG_INFINITY`
    /// when the class is empty.
    pub max_standing_mm: f64,
}

impl OwnedAboveSpec {
    fn add(&mut self, standing: f64, cell_area: f64) {
        self.cells += 1;
        self.area_mm2 += cell_area;
        if standing > self.max_standing_mm {
            self.max_standing_mm = standing;
        }
    }
}

/// The union audit's above-spec population, partitioned by owner.
///
/// The four classes partition the audit's `above_spec_cells` exactly (same
/// grid, same spec bar). `unowned` is the G-UNIONCOV gap class: standing
/// material on ground no band claims.
#[derive(Debug, Clone)]
pub struct OwnershipAttribution {
    /// The audit grid's cell (mm) — areas scale with it (rule 4).
    pub cell_mm: f64,
    pub shallow: OwnedAboveSpec,
    pub mid_steep: OwnedAboveSpec,
    pub very_steep: OwnedAboveSpec,
    /// Above-spec ground owned by NO layer — the union-gap finding.
    pub unowned: OwnedAboveSpec,
    /// Total above-spec cells — must equal the union report's
    /// `above_spec_cells` when both ran with the same params and stock.
    pub above_spec_cells: usize,
}

/// Partition the union audit's above-spec population by band ownership.
///
/// `layers` are the ownership maps of every finishing op on the board, in
/// precedence order: the FIRST layer that owns a cell names its band. (Two
/// tiers can both claim ground near a tier seam; precedence keeps the
/// partition a partition. Pass the finer tier first so seam ground is
/// charged to the op expected to finish it.)
///
/// Everything else — grid, spec bar, frames — matches
/// [`super::union_coverage::audit_stock_vs_model`]; run both with the same
/// `params` so the populations agree.
#[must_use]
pub fn attribute_above_spec(
    stock: &TriDexelStock,
    model: &TriangleMesh,
    index: &SpatialIndex,
    params: &UnionCoverageParams,
    layers: &[&BandOwnership],
) -> OwnershipAttribution {
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let (rows, cols) = (grid.rows, grid.cols);
    let cell_area = cell * cell;

    // Same standing definition as `audit_stock_vs_model`: vertical, against
    // model + stock_to_leave; NAN = off-model; cut-through columns are a
    // gouge class, never above-spec.
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

    let mut out = OwnershipAttribution {
        cell_mm: cell,
        shallow: OwnedAboveSpec {
            max_standing_mm: f64::NEG_INFINITY,
            ..OwnedAboveSpec::default()
        },
        mid_steep: OwnedAboveSpec {
            max_standing_mm: f64::NEG_INFINITY,
            ..OwnedAboveSpec::default()
        },
        very_steep: OwnedAboveSpec {
            max_standing_mm: f64::NEG_INFINITY,
            ..OwnedAboveSpec::default()
        },
        unowned: OwnedAboveSpec {
            max_standing_mm: f64::NEG_INFINITY,
            ..OwnedAboveSpec::default()
        },
        above_spec_cells: 0,
    };
    for (row, row_vals) in standing.iter().enumerate() {
        for (col, &s) in row_vals.iter().enumerate() {
            if !s.is_finite() || s <= params.spec_tolerance_mm {
                continue;
            }
            out.above_spec_cells += 1;
            let (x, y) = grid.cell_to_world(row, col);
            let p = P2::new(x, y);
            let owner = layers.iter().find_map(|l| l.owner_at(p));
            match owner {
                Some(FinishBand::Shallow) => out.shallow.add(s, cell_area),
                Some(FinishBand::MidSteep) => out.mid_steep.add(s, cell_area),
                Some(FinishBand::VerySteep) => out.very_steep.add(s, cell_area),
                None => out.unowned.add(s, cell_area),
            }
        }
    }
    out
}
