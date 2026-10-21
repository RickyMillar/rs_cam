//! One level of a 2.5D op through the kernels its Layered adapter calls
//! (`compute/execute/clearing_2d.rs`): the Z-free geometry once
//! (`prepare`), then the stamp at one Z (`emit_level`).
//!
//! D3 deleted the public per-op wrappers (`pocket_toolpath`,
//! `profile_toolpath`, `zigzag_toolpath`). A test that measures the planner
//! alone calls these. A test that wants the op's toolpath goes through the
//! session (`op_fixtures::generated_op`). The ladder and the link between
//! levels belong to `run_layered`; no helper here composes levels.

use rs_cam_core::interrupt::NeverCancel;
use rs_cam_core::ops::pocket::{
    PocketParams, pocket_contours_reported_with_cancel, pocket_contours_to_sided_toolpath,
};
use rs_cam_core::ops::profile::{ProfileParams, profile_path_reported, profile_path_to_toolpath};
use rs_cam_core::ops::zigzag::{ZigzagParams, lines_to_toolpath, zigzag_lines_reported};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::Toolpath;

/// One pocket level: the ring cascade, then the stamp at `params.cut_depth`.
pub fn pocket_level(polygon: &Polygon2, params: &PocketParams) -> Toolpath {
    let (rings, _report) = pocket_contours_reported_with_cancel(
        polygon,
        params.tool_radius,
        params.stepover,
        &NeverCancel,
    )
    .expect("never cancelled");
    pocket_contours_to_sided_toolpath(&rings, params).toolpath
}

/// One profile level: the side offset, then the stamp. A collapsed offset
/// gives no contour and an empty toolpath.
pub fn profile_level(polygon: &Polygon2, params: &ProfileParams) -> Toolpath {
    match profile_path_reported(polygon, params).0 {
        Some(contour) => {
            profile_path_to_toolpath(&contour, params, params.side.ring_material()).toolpath
        }
        None => Toolpath::new(),
    }
}

/// One zigzag level: the scan rows, then the stamp. The rows sit on the
/// polygon's own lattice, which is the outline lattice the adapter uses
/// when the region is the whole outline (B9).
pub fn zigzag_level(polygon: &Polygon2, params: &ZigzagParams) -> Toolpath {
    let (lines, _failures) =
        zigzag_lines_reported(polygon, params.tool_radius, params.stepover, params.angle);
    lines_to_toolpath(&lines, params)
}
