//! Monge-quadric curvature estimation — PROMOTED to
//! `rs_cam_core::metrology::monge` (Track M, 2026-09-02). This shim keeps
//! every instrument's `common::monge::` import path working; the code,
//! the contract and the derivation pointers live on the library module.

#[allow(unused_imports)]
pub use rs_cam_core::metrology::monge::{
    EPS_DENOM, ISOTROPY_ABS_TOL, ISOTROPY_REL_TOL, MIN_FIT_POINTS, MIN_PIVOT_RATIO, MongeFit,
    MongeOutcome, MongeScratch, Quantiles, axis_cos, dominant_axis, fit_from_derivatives,
    fit_quadric, kappa_perp_zou, lattice, median, quantiles, strip_width, surface_z,
    tessellate_heightfield, weighted_pick,
};
