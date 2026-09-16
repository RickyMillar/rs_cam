//! Derived geometry: regions, grids, distance fields, contours and the
//! machining boundary.
//!
//! The primitives themselves — `geo`, `polygon` and `mesh` — stay at the
//! crate root. This folder holds what the crate builds on them.

pub mod arc_util;
pub mod boundary;
pub mod contour_extract;
pub mod edge_distance;
pub mod enriched_mesh;
pub mod fiber;
pub mod grid2;
pub mod grid_field;
pub mod marching_squares;
pub mod monotone_cells;
pub(crate) mod nn_order;
pub mod point_runs;
pub mod region_mask;
pub mod region_set;
