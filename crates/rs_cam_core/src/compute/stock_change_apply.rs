//! S3 (`planning/stock_additions_2026-10-09/PLAN.md`): apply a setup's stock
//! changes to the simulated stock.
//!
//! The simulation calls [`apply_group_stock_changes`] once per group, after
//! the S0 carry and before the group's first toolpath. For each enabled
//! change, in order:
//!
//! 1. The geometry goes into the SETUP frame: the outlines like a 2D drawing
//!    (`SetupTransformInfo::apply_to_drawing_polygons`), the mesh like the
//!    model mesh of a toolpath (`SetupTransformInfo::apply_to_mesh`). An
//!    identity group has no transform: its frame is the world frame.
//! 2. A cell scan (`dexel_stock::stock_edit`) makes the column edits on the
//!    group stock. The outline fill reads the group stock as it is after
//!    the changes before it.
//! 3. The edits go to the group stock: `Add` unions them as the change's
//!    material (a slot from `MaterialSlotTable::slot_for`), `Remove`
//!    subtracts them from every material.
//! 4. The same edits go to the playback stock (`global_stock`), through the
//!    group-to-global point map that the cuts use
//!    ([`super::simulate::group_point_to_global`]). A lateral group skips this
//!    step, as its cuts do (G-LATERALSCRUB).
//!
//! The result records the volume that each change added and removed
//! ([`StockChangeVolume`]), measured on the group stock.

use serde::{Deserialize, Serialize};

use crate::compute::simulate::{SimGroupEntry, SimulationError, group_point_to_global};
use crate::compute::stock_change::{
    ResolvedStockChange, StockChangeOp, StockChangeSource, StockGeometry, is_closed_outline,
};
use crate::dexel_stock::TriDexelStock;
use crate::dexel_stock::stock_edit::{
    ColumnEdit, ColumnEditOp, ColumnIntervals, MESH_ODD_CELL_SHARE_LIMIT, apply_column_edits,
    mesh_cast_edits, outline_extrude_edits, outline_fill_edits,
};
use crate::geo::P3;
use crate::ids::StockChangeId;
use crate::polygon::Polygon2;
use crate::stock::material_slot::MaterialSlot;

/// The volume one stock change added to and removed from its group stock.
///
/// The volume is the change in material length of each edited column times
/// the cell area: the measure the stamp kernels use for removed volume. An
/// `Add` that overlaps material adds only the empty part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockChangeVolume {
    /// The `SetupData::id` of the setup that owns the change.
    pub setup_id: usize,
    /// The change id, unique within the setup.
    pub change_id: StockChangeId,
    /// The change name, for a surface to print.
    pub name: String,
    /// Add or remove.
    pub op: StockChangeOp,
    /// The material slot that an `Add` wrote. `None` for a `Remove`.
    pub material_slot: Option<MaterialSlot>,
    /// The volume added, in mm³.
    pub added_mm3: f64,
    /// The volume removed, in mm³.
    pub removed_mm3: f64,
}

impl StockChangeVolume {
    /// The volume added, in ml (1 ml = 1000 mm³).
    #[must_use]
    pub fn added_ml(&self) -> f64 {
        self.added_mm3 / 1000.0
    }

    /// The volume removed, in ml (1 ml = 1000 mm³).
    #[must_use]
    pub fn removed_ml(&self) -> f64 {
        self.removed_mm3 / 1000.0
    }
}

/// The refusal for one change, with the change named.
fn refuse(resolved: &ResolvedStockChange, reason: String) -> SimulationError {
    SimulationError::StockChangeRefused {
        setup_id: resolved.setup_id,
        change_id: resolved.change.id,
        change_name: resolved.change.name.clone(),
        reason,
    }
}

/// The closed outlines of every source, in the setup frame, as one union.
fn setup_outlines(
    resolved: &ResolvedStockChange,
    group: &SimGroupEntry,
) -> Result<Vec<Polygon2>, SimulationError> {
    let mut closed: Vec<Polygon2> = Vec::new();
    for source in &resolved.sources {
        match source {
            StockChangeSource::Outlines(polygons) => {
                closed.extend(polygons.iter().filter(|p| is_closed_outline(p)).cloned());
            }
            StockChangeSource::Missing(model_id) => {
                return Err(missing(resolved, model_id.0));
            }
            StockChangeSource::Mesh(_) => {
                return Err(refuse(
                    resolved,
                    "an outline geometry received a mesh source".to_owned(),
                ));
            }
        }
    }
    let closed = match &group.local_to_global {
        Some(info) => info.apply_to_drawing_polygons(&closed),
        None => closed,
    };
    let union = Polygon2::union_all(&closed);
    if union.is_empty() {
        return Err(refuse(
            resolved,
            "the outline models hold no closed outline with an area".to_owned(),
        ));
    }
    Ok(union)
}

fn missing(resolved: &ResolvedStockChange, model_id: usize) -> SimulationError {
    refuse(
        resolved,
        format!(
            "model id {model_id} is missing, or it holds no geometry of the kind the {} needs",
            resolved.change.geometry.kind_label()
        ),
    )
}

/// The column edits of one change on the group stock, in the setup frame.
fn change_edits(
    resolved: &ResolvedStockChange,
    group: &SimGroupEntry,
    stock: &TriDexelStock,
) -> Result<Vec<ColumnEdit>, SimulationError> {
    let grid = &stock.z_grid;
    match &resolved.change.geometry {
        StockGeometry::OutlineFill { level_z, .. } => {
            if resolved.change.op == StockChangeOp::Remove {
                return Err(refuse(
                    resolved,
                    "a remove cannot use an outline fill: a fill describes empty space and \
                     holds no material to remove; use an outline extrude or a model"
                        .to_owned(),
                ));
            }
            let outlines = setup_outlines(resolved, group)?;
            Ok(outline_fill_edits(
                grid,
                &outlines,
                *level_z as f32,
                stock.stock_bbox.min.z as f32,
            ))
        }
        StockGeometry::OutlineExtrude {
            z_bottom, z_top, ..
        } => {
            let outlines = setup_outlines(resolved, group)?;
            Ok(outline_extrude_edits(
                grid,
                &outlines,
                *z_bottom as f32,
                *z_top as f32,
            ))
        }
        StockGeometry::Model { model_id } => {
            let mesh = match resolved.sources.first() {
                Some(StockChangeSource::Mesh(mesh)) => mesh,
                Some(StockChangeSource::Missing(id)) => return Err(missing(resolved, id.0)),
                _ => return Err(missing(resolved, model_id.0)),
            };
            let local;
            let mesh_in_setup = match &group.local_to_global {
                Some(info) => {
                    local = info.apply_to_mesh(mesh);
                    &local
                }
                None => mesh.as_ref(),
            };
            let cast = mesh_cast_edits(grid, mesh_in_setup);
            if cast.is_open() {
                return Err(refuse(
                    resolved,
                    format!(
                        "model id {} is not a closed mesh: {} of the {} columns that \
                         cross it cross it an odd number of times ({:.1} %, the limit is \
                         {:.1} %)",
                        model_id.0,
                        cast.odd_cells,
                        cast.hit_cells,
                        cast.odd_share() * 100.0,
                        MESH_ODD_CELL_SHARE_LIMIT * 100.0
                    ),
                ));
            }
            Ok(cast.edits)
        }
    }
}

/// Map the column edits of a group stock to the playback stock.
///
/// Each interval end goes through [`group_point_to_global`] at its column
/// node: the map the cuts take. A Bottom setup reverses Z, so the two ends
/// are sorted again. Edits whose node maps off the playback grid are dropped.
fn edits_to_global(
    edits: &[ColumnEdit],
    stock: &TriDexelStock,
    group: &SimGroupEntry,
    global: &TriDexelStock,
    stock_min: P3,
) -> Vec<ColumnEdit> {
    let local = &stock.z_grid;
    let target = &global.z_grid;
    let mut mapped = Vec::with_capacity(edits.len());
    for edit in edits {
        let (row, col) = (edit.idx / local.cols, edit.idx % local.cols);
        let (u, v) = local.cell_to_world(row, col);
        let mut cell = None;
        let mut intervals = ColumnIntervals::new();
        for &(a, b) in &edit.intervals {
            let pa = group_point_to_global(
                P3::new(u, v, f64::from(a)),
                &group.local_to_global,
                stock_min,
            );
            let pb = group_point_to_global(
                P3::new(u, v, f64::from(b)),
                &group.local_to_global,
                stock_min,
            );
            if cell.is_none() {
                cell = target.world_to_cell(pa.x, pa.y);
            }
            let (lo, hi) = if pa.z <= pb.z {
                (pa.z, pb.z)
            } else {
                (pb.z, pa.z)
            };
            intervals.push((lo as f32, hi as f32));
        }
        if let Some((r, c)) = cell {
            intervals.sort_by(|x, y| x.0.total_cmp(&y.0));
            mapped.push(ColumnEdit {
                idx: r * target.cols + c,
                intervals,
            });
        }
    }
    mapped
}

/// Apply the group's stock changes, in order, to the group stock and, when
/// `global` is `Some`, to the playback stock.
///
/// `stock_min` is `SimulationRequest::stock_bbox.min`, which the identity
/// arm of the group-to-global map reads. A group with no stock change
/// touches neither stock.
///
/// # Errors
/// [`SimulationError::StockChangeRefused`] for the first change that cannot
/// apply: a missing model, an outline set with no area, a mesh that is not
/// closed, an `OutlineFill` with `Remove` (the command doors accept it; the
/// simulation refuses it), or a full material slot table.
pub(crate) fn apply_group_stock_changes(
    stock: &mut TriDexelStock,
    mut global: Option<&mut TriDexelStock>,
    group: &SimGroupEntry,
    stock_min: P3,
) -> Result<Vec<StockChangeVolume>, SimulationError> {
    let mut volumes = Vec::with_capacity(group.stock_changes.len());
    for resolved in &group.stock_changes {
        let edits = change_edits(resolved, group, stock)?;
        let (op, slot) = match resolved.change.op {
            StockChangeOp::Add => {
                let slot = stock
                    .materials
                    .slot_for(&resolved.change.material)
                    .map_err(|full| refuse(resolved, full.to_string()))?;
                (ColumnEditOp::Union(slot), Some(slot))
            }
            StockChangeOp::Remove => (ColumnEditOp::Subtract, None),
        };
        let delta = apply_column_edits(&mut stock.z_grid, &edits, op);
        if let Some(global) = global.as_deref_mut() {
            let global_op = match resolved.change.op {
                StockChangeOp::Add => ColumnEditOp::Union(
                    global
                        .materials
                        .slot_for(&resolved.change.material)
                        .map_err(|full| refuse(resolved, full.to_string()))?,
                ),
                StockChangeOp::Remove => ColumnEditOp::Subtract,
            };
            let mapped = edits_to_global(&edits, stock, group, global, stock_min);
            apply_column_edits(&mut global.z_grid, &mapped, global_op);
        }
        volumes.push(StockChangeVolume {
            setup_id: resolved.setup_id,
            change_id: resolved.change.id,
            name: resolved.change.name.clone(),
            op: resolved.change.op,
            material_slot: slot,
            added_mm3: delta.max(0.0),
            removed_mm3: (-delta).max(0.0),
        });
    }
    Ok(volumes)
}
