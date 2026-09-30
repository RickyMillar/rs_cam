//! Per-tier **flute reach** advisory: the fine-tier territory the tier's tool
//! reaches only with the body above its flutes (tiered-finish plan F3c,
//! 2026-09-30).
//!
//! The tier map labels a cell by drop Z only. The drop models the whole
//! cutter profile as cutting: on a tapered ball the cone runs past
//! `cutting_length` up to the shaft diameter. Where a wall is taller than
//! the flutes, the tool can reach a cell and still rub the wall with the
//! non-fluted cone or the shank. That contact can deflect the tool.
//!
//! This module REPORTS that territory. It does not move a cell out of a tier:
//! the territory is unchanged, and the report is advisory.
//!
//! # The rule
//!
//! For every cell a fine tier OWNS (pre-band ownership,
//! [`crate::maps::tier_islands::TierIslandSet::owned_mask`]):
//!
//! 1. the tip is the tier tool's own drop at the cell centre;
//! 2. the body is [`ToolAssembly::non_fluted_body`], a flat cylinder whose
//!    bottom sits at the flute length above the tip;
//! 3. the cell binds when [`body_segment_penetration_mm`] at that tip is
//!    above [`BODY_STRIKE_THRESHOLD_MM`].
//!
//! Steps 2 and 3 are the rule [`crate::stock::collision::check_collisions`]
//! applies to a generated toolpath, called, not copied. One difference is
//! deliberate: `check_collisions` skips a segment no wider than the cutter's
//! envelope radius, and on a tapered ball whose shank diameter equals its
//! shaft diameter that skip removes the shank from the check. This module
//! does not skip it.
//!
//! # Cost
//!
//! Two drop-cutter calls per owned cell (the tool, then the body cylinder),
//! walked by rows with one cancel poll per row
//! ([`crate::maps::grid::walk_rows`]). Nothing here is cached: the tier map
//! cache holds the labels, and this pass reads them.

use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::maps::grid::walk_rows;
use crate::maps::tier_islands::TierIslands;
use crate::maps::tier_map::TierMap;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::stock::collision::{
    BODY_STRIKE_THRESHOLD_MM, ToolAssembly, body_segment_penetration_mm,
};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;

/// One fine tier's flute-reach reading.
#[derive(Debug, Clone, PartialEq)]
pub struct TierFluteReach {
    /// Ladder index of the tier (≥ 1).
    pub tier: u8,
    /// The tool's flute length (mm), [`MillingCutter::length`].
    pub cutting_length_mm: f64,
    /// Height (mm) of the body's bottom above the tip, or `None` when the
    /// assembly models no body above the flutes; nothing is then checked.
    pub body_z_offset_mm: Option<f64>,
    /// Radius (mm) of the body cylinder, or `None` with the offset.
    pub body_radius_mm: Option<f64>,
    /// Owned cells that were checked: every owned cell where the tool's drop
    /// touched the mesh.
    pub checked_cells: usize,
    /// Owned cells where the body strikes the mesh.
    pub binding_cells: usize,
    /// `binding_cells` × cell area (mm²), grid-quantised.
    pub binding_area_mm2: f64,
    /// Per-cell verdict over the tier map's grid, row-major `r·nx + c`.
    pub binding_mask: Vec<bool>,
}

impl TierFluteReach {
    /// `true` when some owned cell is reached only with the body above the
    /// flutes.
    #[must_use]
    pub const fn binds(&self) -> bool {
        self.binding_cells > 0
    }
}

impl std::fmt::Display for TierFluteReach {
    /// Names the measurement and what it means. It names no remedy: the
    /// operator decides between a longer-fluted tool and a coarser tier.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tier {}: {:.0} mm2 of owned territory ({} cells) is reached only with the body \
             above the {:.1} mm flutes",
            self.tier, self.binding_area_mm2, self.binding_cells, self.cutting_length_mm,
        )?;
        if let (Some(z), Some(r)) = (self.body_z_offset_mm, self.body_radius_mm) {
            write!(f, " (a {r:.2} mm radius body {z:.1} mm above the tip)")?;
        }
        write!(
            f,
            ". There the body above the flutes touches the wall, which can deflect the tool."
        )
    }
}

/// One tier's tool: the cutter the tier map drops, and the assembly the
/// body is read from.
pub struct TierTool<'a> {
    /// The cutter.
    pub cutter: &'a dyn MillingCutter,
    /// The cutter's holder and shank assembly.
    pub assembly: ToolAssembly,
}

/// The flute-reach reading of every fine tier in `islands`, in the same
/// order as [`TierIslands::per_tier`].
///
/// `tools` is the ladder, coarse first, indexed like the map's labels. A
/// tier whose index has no tool is skipped.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires; the walk polls once per grid row.
pub fn tier_flute_reach(
    map: &TierMap,
    islands: &TierIslands,
    tools: &[TierTool<'_>],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<Vec<TierFluteReach>, Cancelled> {
    let cell_area = map.grid.cell_mm * map.grid.cell_mm;
    let nx = map.grid.nx;
    let mut out = Vec::with_capacity(islands.per_tier.len());
    for set in &islands.per_tier {
        let Some(tool) = tools.get(usize::from(set.tier)) else {
            continue;
        };
        let body = tool.assembly.non_fluted_body();
        let owned = &set.owned_mask;
        // One entry per cell: `None` = not checked, `Some(b)` = checked.
        let verdicts: Vec<Option<bool>> = walk_rows(&map.grid, cancel, None, |row| {
            let cells = (0..nx)
                .map(|col| {
                    let i = map.grid.index_of(row, col);
                    if !owned.get(i).copied().unwrap_or(false) {
                        return None;
                    }
                    let (x, y) = (map.grid.x_of(col), map.grid.y_of(row));
                    let cl = point_drop_cutter(x, y, mesh, index, tool.cutter);
                    if !cl.contacted {
                        return None;
                    }
                    let Some((z_offset, radius)) = body else {
                        return Some(false);
                    };
                    let tip = P3::new(x, y, cl.z);
                    let binds = body_segment_penetration_mm(tip, z_offset, radius, mesh, index)
                        .is_some_and(|p| p > BODY_STRIKE_THRESHOLD_MM);
                    Some(binds)
                })
                .collect();
            (cells, 0)
        })?;
        let checked_cells = verdicts.iter().filter(|v| v.is_some()).count();
        let binding_mask: Vec<bool> = verdicts.iter().map(|v| *v == Some(true)).collect();
        let binding_cells = binding_mask.iter().filter(|&&b| b).count();
        out.push(TierFluteReach {
            tier: set.tier,
            cutting_length_mm: tool.cutter.length(),
            body_z_offset_mm: body.map(|b| b.0),
            body_radius_mm: body.map(|b| b.1),
            checked_cells,
            binding_cells,
            binding_area_mm2: binding_cells as f64 * cell_area,
            binding_mask,
        });
    }
    Ok(out)
}
