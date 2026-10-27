//! The floor the 3D Rough plan intends to leave under a tool centre: ONE
//! rule, for the planner and for the coverage reading (G-ROUGHCOV A3).
//!
//! # The rule (from the planner's own code)
//!
//! * The planner's surface is a drop-cutter heightmap of the op's cutter
//!   (`SurfaceHeightmap::from_mesh`, floored at `mesh.bbox.min.z` where the
//!   cutter meets nothing). It is the tool-tip Z at a centre, not the model
//!   Z under it.
//! * One level at `z_level` cuts a centre down to [`level_floor_z`]:
//!   `max(surface + stock_to_leave, z_level)`. The material test
//!   (`clearing.rs::cell_has_material`), the Geodesic Adaptive lift and the
//!   ContourParallel clean-up raster all read this function. The emitter's
//!   drape (`path.rs::drape_path_to_leave`) only lifts a point to
//!   `point_drop_cutter + stock_to_leave`, the same surface, so at a grid
//!   centre it changes nothing.
//! * The last level is [`plan_bottom_z`]: `max(surface_bottom +
//!   stock_to_leave, pinned bottom_z)`. Every level drapes, and no level
//!   goes below the last one, so over the whole plan a centre's floor is
//!   `level_floor_z(surface, stock_to_leave, plan_bottom_z)`.
//!
//! No rule in the planner holds a level as intent on a steep slope. The
//! one hold is the F-029 bite clamp of the ContourParallel clean-up
//! (`stock_top − depth_per_pass − tolerance`); it reads the current stock,
//! so it limits one pass and is not a floor the plan intends.

use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::slope::SurfaceHeightmap;
use crate::tool::MillingCutter;

/// The floor one level at `z_level` cuts a centre down to, where the
/// planner's surface (drop-cutter tip Z) at that centre is `surface_z`.
#[inline]
#[must_use]
pub(super) fn level_floor_z(surface_z: f64, stock_to_leave: f64, z_level: f64) -> f64 {
    (surface_z + stock_to_leave).max(z_level)
}

/// The Z of the plan's last level: the lowest surface plus the stock to
/// leave, raised to a pinned `bottom_z` when one is set.
#[inline]
#[must_use]
pub(super) fn plan_bottom_z(
    surface_bottom: f64,
    stock_to_leave: f64,
    pinned_floor_z: Option<f64>,
) -> f64 {
    (surface_bottom + stock_to_leave).max(pinned_floor_z.unwrap_or(f64::NEG_INFINITY))
}

/// The grid the intended floor is read on: row-major, cell `(row, col)`
/// centred at `(origin_u + col × cell, origin_v + row × cell)`.
#[derive(Debug, Clone, Copy)]
pub struct FloorGrid {
    pub origin_u: f64,
    pub origin_v: f64,
    pub rows: usize,
    pub cols: usize,
    pub cell_mm: f64,
}

/// The plan's intended floor (tool-tip Z) at every centre of `grid`,
/// row-major: `level_floor_z(surface, stock_to_leave, plan_bottom_z)`.
///
/// The surface is the planner's heightmap, built by the same constructor
/// with the same floor (`mesh.bbox.min.z`), on `grid`. `surface_bottom` is
/// the lowest value on `grid`.
///
/// Limits: the planner samples its own grid (cell `max(R / 6, tolerance)`,
/// one tool radius past the mesh bbox). This grid samples the same
/// drop-cutter function at other points, so a value differs from the
/// planner's only by where the two grids sample. `surface_bottom` is the
/// mesh-bbox floor on both grids whenever each holds a centre farther than
/// one tool radius from the model.
#[must_use]
pub fn intended_floor_grid(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    grid: FloorGrid,
    stock_to_leave: f64,
    pinned_floor_z: Option<f64>,
) -> Vec<f64> {
    let surface = SurfaceHeightmap::from_mesh(
        mesh,
        index,
        cutter,
        grid.origin_u,
        grid.origin_v,
        grid.rows,
        grid.cols,
        grid.cell_mm,
        mesh.bbox.min.z,
    );
    let bottom = plan_bottom_z(
        surface.min_z_or_bbox_floor(),
        stock_to_leave,
        pinned_floor_z,
    );
    surface
        .z_or_bbox_floor_values()
        .iter()
        .map(|&s| level_floor_z(s, stock_to_leave, bottom))
        .collect()
}
