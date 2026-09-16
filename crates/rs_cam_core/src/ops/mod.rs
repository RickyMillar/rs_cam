//! The 2.5D and drilling operations, with the depth-stepping helper they
//! share.
//!
//! `adaptive_shared` is operation-layer engagement math that `adaptive` and
//! `adaptive3d` share; both engines stay in their own folders.
//! `trace_path` is the follow-path operation. The toolpath records live in
//! `trace`, which is a different thing.

pub mod adaptive_shared;
pub mod chamfer;
pub mod depth;
pub mod drill;
pub mod drill_metrics;
pub mod drill_op;
pub mod face;
pub mod inlay;
pub mod pocket;
pub mod profile;
pub mod project_curve;
pub mod rest;
pub mod trace_path;
pub mod vcarve;
pub mod waterline;
pub mod zigzag;
