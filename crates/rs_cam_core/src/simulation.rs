//! Simulation type re-exports for the viewport upload path.
//!
//! The module owns no logic. It re-exports three types that live
//! elsewhere, so a caller can name them under one simulation path.
//! [`StockMesh`] is the live one: `rs_cam_viz::app::gpu_upload` reads it
//! here at two production sites and at several test sites.
//! [`linearize_arc`] and [`RadialProfileLUT`] have no reader through this
//! path today; callers reach `crate::geometry::arc_util` and `crate::radial_profile`
//! directly.
//!
//! The module doc called this module "legacy" until L13 (tech debt
//! 2026-09-16). It is not legacy. The simulation ENGINE is
//! [`crate::dexel_stock`], and the cut record is
//! [`crate::simulation_cut`]; this file is the name the viewport uses.

pub use crate::geometry::arc_util::linearize_arc;
pub use crate::radial_profile::RadialProfileLUT;
pub use crate::stock_mesh::StockMesh;
