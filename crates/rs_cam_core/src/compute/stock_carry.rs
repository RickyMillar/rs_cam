//! S0 (`planning/stock_additions_2026-10-09/PLAN.md`): carry the stock across
//! setups.
//!
//! Each Z-axis setup group starts from the final stock of the Z-axis group
//! before it, mapped into its own frame. Before S0, every group started from
//! a full block, so the metrics, the gates, `prior_stocks` and the checkpoint
//! meshes of setup N+1 did not see the cuts of setup N
//! (`planning/stock_fill_2026-10-09/DESIGN.md` §1.3, finding F1).
//!
//! # The map
//!
//! [`carry_stock_into_group`] is ONE general resample, not a table of cases.
//! For each node of the new grid it maps the node position from the new
//! group's frame to the zero-rooted global frame and on to the old group's
//! frame, and takes the ray of the nearest old node. Each segment end maps
//! through the same two frames in the other direction. The one map covers:
//!
//! - a Top/Bottom flip (rows reverse, `[a, b] -> [H - b, H - a]`, the
//!   segment order reverses);
//! - an identity group (world frame) to a non-identity group (zero-rooted
//!   frame): a shift by the stock origin;
//! - a Z rotation of 90°, 180° or 270°: a transpose or a reversal of the
//!   rows and the columns.
//!
//! The map is exact when the stock extent is a whole number of cells along
//! each mirrored axis. Else a mirrored node misses the old grid by up to
//! `ceil(D / cell) * cell - D`; the resample takes the nearest node and
//! [`CarryMap::max_node_offset_mm`] records the largest miss.
//!
//! A lateral group (Front, Back, Left, Right) is neither a target nor a
//! source of the carry. Its tool axis is a global X or Y dexel axis, and the
//! side grids do not boolean (G-LATERALSCRUB), so it keeps a fresh stock.
//! The Z-axis group after it carries from the last Z-axis group before it.

use std::sync::Arc;

use crate::compute::simulate::{SimGroupEntry, global_point_to_group, group_point_to_global};
use crate::compute::transform::SetupTransformInfo;
use crate::dexel_stock::TriDexelStock;
use crate::geo::{BoundingBox3, P3};
use crate::stock::dexel::{DexelRay, DexelSegment};

/// How a simulation group got the stock it started from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StockCarry {
    /// The first Z-axis group of the run: a full block.
    FreshFirst,
    /// A lateral group (Front, Back, Left, Right): a full block. The side
    /// grids do not boolean (G-LATERALSCRUB), so a lateral group does not
    /// see the cuts of the setups before it.
    FreshLateral,
    /// The final stock of group `from_group`, mapped into this group's
    /// frame by [`carry_stock_into_group`].
    Carried {
        /// The ordinal of the Z-axis group whose final stock this group
        /// starts from.
        from_group: usize,
        /// The map reversed Z (a Top/Bottom pair).
        z_flipped: bool,
        /// The largest XY distance, in mm, between a new node's mapped
        /// position and the old node it took its ray from. Zero (to float
        /// noise) when the extent is a whole number of cells.
        max_node_offset_mm: f64,
    },
}

/// The start-stock record of one simulation group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimGroupStart {
    /// The index of the group in `SimulationRequest::groups`.
    pub group_ordinal: usize,
    /// Where the group's start stock came from.
    pub carry: StockCarry,
}

/// What [`carry_stock_into_group`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarryMap {
    /// The map reversed Z.
    pub z_flipped: bool,
    /// See [`StockCarry::Carried::max_node_offset_mm`].
    pub max_node_offset_mm: f64,
}

/// The final stock of the last Z-axis group, held for the next Z-axis group.
///
/// The `Arc` is the same one as the last checkpoint's `mesh_stock` of that
/// group, so the carry costs no grid of its own.
#[derive(Clone)]
pub(crate) struct CarrySource {
    pub(crate) group_ordinal: usize,
    pub(crate) stock: Arc<TriDexelStock>,
    pub(crate) frame: Option<SetupTransformInfo>,
}

/// `true` when the group's tool axis is a global X or Y dexel axis.
///
/// One predicate for the simulator's three lateral decisions: the global
/// playback stamp (G-LATERALSCRUB), the checkpoint frame and the stock
/// carry.
#[must_use]
pub fn group_is_lateral(group: &SimGroupEntry) -> bool {
    group
        .local_to_global
        .as_ref()
        .is_some_and(|info| info.face_up.is_lateral())
}

/// Build the start stock of a group from the final stock of an earlier
/// Z-axis group.
///
/// - `src` is in `src_frame`; the result is a new grid over `dst_bbox` in
///   `dst_frame`. A frame is a group's `local_to_global`: `None` is the
///   world frame of an identity group, `Some` a zero-rooted setup frame.
/// - `stock_min` is `SimulationRequest::stock_bbox.min`, which the identity
///   arm of the frame map reads.
/// - Both frames must have their tool axis on global Z. The caller does not
///   carry into or out of a lateral group.
///
/// `conservative_top` follows the rays. When Z keeps its sense, the old
/// sliver-safe bound maps with the ray. When Z reverses, the old bound
/// describes the far face and means nothing here, so the new bound of a
/// cell is the highest ray top in its 3 x 3 neighbourhood: the material
/// edge lies somewhere between two nodes, and the cell spans half a cell
/// to each side.
#[must_use]
pub fn carry_stock_into_group(
    src: &TriDexelStock,
    src_frame: &Option<SetupTransformInfo>,
    dst_bbox: &BoundingBox3,
    dst_frame: &Option<SetupTransformInfo>,
    stock_min: P3,
    resolution: f64,
) -> (TriDexelStock, CarryMap) {
    let to_dst = |p: P3| {
        global_point_to_group(
            group_point_to_global(p, src_frame, stock_min),
            dst_frame,
            stock_min,
        )
    };
    let to_src = |p: P3| {
        global_point_to_group(
            group_point_to_global(p, dst_frame, stock_min),
            src_frame,
            stock_min,
        )
    };

    // The Z map is affine with slope +1 or -1 for two Z-axis frames, and it
    // does not depend on XY: z_dst = z_offset + z_sign * z_src.
    let probe = P3::new(src.z_grid.origin_u, src.z_grid.origin_v, 0.0);
    let z_offset = to_dst(probe).z;
    let z_sign = if to_dst(P3::new(probe.x, probe.y, 1.0)).z - z_offset < 0.0 {
        -1.0
    } else {
        1.0
    };
    let z_flipped = z_sign < 0.0;
    let map_z = |z: f32| (z_offset + z_sign * f64::from(z)) as f32;

    let mut dst = TriDexelStock::from_bounds(dst_bbox, resolution);
    let src_grid = &src.z_grid;
    let cols = dst.z_grid.cols;
    let (dst_origin_u, dst_origin_v, dst_cell) = (
        dst.z_grid.origin_u,
        dst.z_grid.origin_v,
        dst.z_grid.cell_size,
    );
    let nearest = |value: f64, origin: f64, count: usize| -> usize {
        let index = ((value - origin) / src_grid.cell_size).round();
        let last = count.saturating_sub(1) as f64;
        index.clamp(0.0, last) as usize
    };

    let mut max_node_offset_mm = 0.0_f64;
    let dst_grid = &mut dst.z_grid;
    for (idx, (ray, top)) in dst_grid
        .rays
        .iter_mut()
        .zip(dst_grid.conservative_top.iter_mut())
        .enumerate()
    {
        let (row, col) = (idx / cols, idx % cols);
        let u = dst_origin_u + col as f64 * dst_cell;
        let v = dst_origin_v + row as f64 * dst_cell;
        let at = to_src(P3::new(u, v, 0.0));
        let src_col = nearest(at.x, src_grid.origin_u, src_grid.cols);
        let src_row = nearest(at.y, src_grid.origin_v, src_grid.rows);
        let (node_u, node_v) = src_grid.cell_to_world(src_row, src_col);
        max_node_offset_mm = max_node_offset_mm.max((at.x - node_u).hypot(at.y - node_v));

        let src_ray = src_grid.ray(src_row, src_col);
        *ray = if z_flipped {
            src_ray
                .iter()
                .rev()
                .map(|seg| DexelSegment::new(map_z(seg.exit), map_z(seg.enter)))
                .collect::<DexelRay>()
        } else {
            src_ray
                .iter()
                .map(|seg| DexelSegment::new(map_z(seg.enter), map_z(seg.exit)))
                .collect::<DexelRay>()
        };
        if !z_flipped {
            *top = map_z(src_grid.conservative_top_at(src_row, src_col));
        }
    }

    if z_flipped {
        recompute_conservative_top(dst_grid, dst_bbox.min.z as f32);
    }

    (
        dst,
        CarryMap {
            z_flipped,
            max_node_offset_mm,
        },
    )
}

/// Map a group's stock into the ZERO-ROOTED stock-relative global frame of
/// the playback stock (`SimCheckpointMesh::stock` of a Z-axis group).
///
/// The live scrub reads it: when the playhead crosses into a Z-axis group,
/// playback resumes from that group's carried start stock (its
/// `prior_stocks` entry), mapped here, and not from the playback stock of the
/// group before it. `stock_bbox` is `SimulationRequest::stock_bbox` (world).
#[must_use]
pub fn map_stock_to_global(
    src: &TriDexelStock,
    src_frame: &Option<SetupTransformInfo>,
    stock_bbox: &BoundingBox3,
) -> TriDexelStock {
    let (w, d, h) = (
        stock_bbox.max.x - stock_bbox.min.x,
        stock_bbox.max.y - stock_bbox.min.y,
        stock_bbox.max.z - stock_bbox.min.z,
    );
    // Top at 0° maps a point to itself: the zero-rooted global frame.
    let global = Some(SetupTransformInfo {
        stock_x: w,
        stock_y: d,
        stock_z: h,
        stock_origin_x: stock_bbox.min.x,
        stock_origin_y: stock_bbox.min.y,
        stock_origin_z: stock_bbox.min.z,
        ..SetupTransformInfo::default()
    });
    let dst_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(w, d, h),
    };
    carry_stock_into_group(
        src,
        src_frame,
        &dst_bbox,
        &global,
        stock_bbox.min,
        src.z_grid.cell_size,
    )
    .0
}

/// Set each cell's `conservative_top` to the highest ray top in its 3 x 3
/// neighbourhood. An empty ray counts as `floor`.
fn recompute_conservative_top(grid: &mut crate::stock::dexel::DexelGrid, floor: f32) {
    let (rows, cols) = (grid.rows, grid.cols);
    let tops: Vec<f32> = grid
        .rays
        .iter()
        .map(|ray| ray.last().map_or(floor, |seg| seg.exit))
        .collect();
    for (idx, top) in grid.conservative_top.iter_mut().enumerate() {
        let (row, col) = (idx / cols, idx % cols);
        let mut highest = floor;
        for r in row.saturating_sub(1)..=(row + 1).min(rows - 1) {
            for c in col.saturating_sub(1)..=(col + 1).min(cols - 1) {
                if let Some(&t) = tops.get(r * cols + c) {
                    highest = highest.max(t);
                }
            }
        }
        *top = highest;
    }
}
