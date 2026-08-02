//! `rs_cam_core` — the CAM engine and shared data model.
//!
//! The crate is layered roughly in this order:
//!
//! 1. **Import** — STL / SVG / DXF / STEP into geometry primitives
//! 2. **Tool model** — cutter geometry, holder/shank envelope, vendor metadata
//! 3. **Operations** — 2.5D + 3D toolpath generation (`adaptive`, `pocket`,
//!    `dropcutter`, `waterline`, `drill`, etc.) emitting the shared
//!    `Toolpath` IR
//! 4. **Dressups** — entry strategies, leads, dogbones, arc fitting,
//!    feed optimization, TSP rapid ordering
//! 5. **Simulation** — tri-dexel volumetric stock, cut-trace metrics,
//!    collision checks
//! 6. **Export** — G-code (`gcode`), SVG/HTML preview (`viz`),
//!    fingerprints
//!
//! The `Toolpath` IR is the boundary between planning and post / output;
//! GUI and CLI consumers depend only on the public surface of this crate.
//! See `FEATURE_CATALOG.md` and `architecture/` at the repo root for the
//! product surface and design rationale.

pub mod adaptive;
pub mod adaptive3d;
pub mod adaptive_shared;
pub mod arc_util;
pub mod arcfit;
pub mod boundary;
pub mod build_info;
pub mod chamfer;
pub mod classify_probe;
pub mod collision;
pub mod compute;
pub mod condition;
pub mod contour_extract;
pub mod crease_paths;
pub mod crest_lines;
pub mod debug_trace;
pub mod depth;
pub mod dexel;
pub mod dexel_mesh;
pub mod dexel_mesh_mc;
pub mod dexel_stock;
pub mod diagnostics;
pub mod dressup;
pub mod drill;
pub mod drill_metrics;
pub mod drill_op;
pub mod dropcutter;
pub mod dxf_input;
pub mod enriched_mesh;
pub mod face;
pub mod feed_modulation;
pub mod feedopt;
pub mod feeds;
pub mod fiber;
pub mod fingerprint;
pub mod finish_planner;
pub mod finish_setup;
pub mod gcode;
pub mod gcode_validator;
pub mod geo;
pub mod grid2;
pub mod grid_field;
pub mod horizontal_finish;
pub mod ids;
pub mod inlay;
pub mod interrupt;
pub mod io;
pub mod machine;
pub mod machine_kinematics;
pub mod machine_library;
pub mod marching_squares;
pub mod material;
pub mod measurement;
pub mod mesh;
pub mod narrate;
pub mod pencil;
pub mod pencil_dihedral;
pub mod pocket;
pub mod point_runs;
pub mod polygon;
pub mod profile;
pub mod project_curve;
pub mod pushcutter;
pub mod radial_finish;
pub mod radial_profile;
pub mod ramp_finish;
pub mod reach;
pub mod region_mask;
pub mod region_set;
pub mod rest;
pub mod rest_field;
pub mod rest_heatmap_mesh;
pub mod scallop;
pub mod scallop_math;
pub mod semantic_trace;
pub mod session;
pub mod simulation;
pub mod simulation_cut;
pub mod slope;
pub mod spiral_finish;
pub mod steep_shallow;
#[cfg(feature = "step")]
pub mod step_input;
pub mod stock_mesh;
pub mod strategy_advisor;
pub mod surface_link;
pub mod svg_input;
pub mod tool;
pub mod tool_library;
pub mod tool_load;
pub mod toolpath;
pub mod toolpath_spans;
pub mod trace;
pub mod transform_provenance;
pub mod tsp;
pub mod unified_finish;
pub mod vcarve;
pub mod viz;
pub mod waterline;
pub mod zigzag;

pub use ids::ToolpathId;
