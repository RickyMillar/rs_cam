//! Grid-walk maps and their bounded caches.
//!
//! One walk over the grid labels every cell. These modules build those maps,
//! cut them into islands, render them and memoise them.

pub mod finish_surface_cache;
pub mod geom_cache;
// The walk grid the full-board maps share. `GridSpec` is public — the three
// map result types hold one (FLD-01) — and `walk_rows` stays crate-private.
pub mod grid;
// The bounded mesh-identity memo the map caches share; private to this folder.
mod memo;
pub mod reach_map;
pub mod reach_map_cache;
pub mod rest_heatmap_mesh;
pub mod tier_islands;
pub mod tier_map;
pub mod tier_map_cache;
pub(crate) mod tool_shape_key;
