//! The 3D finishing strategies, the unified planner and the surface
//! analysers the strategies own.
//!
//! `classify_probe`, `crest_lines`, `pencil_dihedral` and
//! `direction_field` sit here rather than in `surface` because each one
//! reads a finishing module. That placement keeps `surface` free of an
//! upward dependency. Roughing lives in `adaptive` and `adaptive3d`.

pub mod classify_probe;
pub mod conformal_spiral;
pub mod crease_paths;
pub mod crest_lines;
pub mod direction_field;
pub mod finish_planner;
pub mod finish_setup;
pub mod horizontal_finish;
pub mod pencil;
pub mod pencil_dihedral;
pub mod radial_finish;
pub mod ramp_finish;
pub mod scallop;
pub mod scallop_isofield;
pub mod scallop_math;
pub mod spiral_finish;
pub mod spiral_finish_compact;
pub mod steep_shallow;
pub mod surface_link;
pub mod unified_finish;
