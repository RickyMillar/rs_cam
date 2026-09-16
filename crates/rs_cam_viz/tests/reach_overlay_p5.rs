//! P5 (GUI half) — the reach-map overlay's colour seam.
//!
//! The overlay draws the MODEL mesh through `colored_opaque_pipeline` with one
//! reach colour per vertex, so the whole overlay rests on one arithmetic
//! contract: **the colour vector carries exactly one entry per mesh vertex,
//! in mesh vertex order.** A vector that is short, long, or reordered does not
//! fail loudly — it paints the wrong part of the part, which is the failure
//! mode an operator cannot see.
//!
//! `rs_cam_viz::state::runtime::reach_overlay_colors` is the single
//! construction site for those colours, shared by the Reach compute lane (which
//! builds them beside the map, off the frame loop) and by the GPU upload pass
//! (which only checks the length before interleaving them). These sentries pin
//! the contract at that seam.
//!
//! The vertex-ORDER half of the claim is pinned by
//! `the_setup_transform_preserves_vertex_order`: the map is measured on the
//! setup-transformed mesh while the viewport draws the same mesh through
//! `transform_mesh`, and both are
//! `SetupTransformInfo::apply_to_mesh`. The two frames' coordinates differ; the
//! ordering must not.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::maps::reach_map::{ReachMap, reach_map_for_mesh};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::tool::BallEndmill;

use rs_cam_viz::state::runtime::reach_overlay_colors;

/// A hemisphere and a Ø6 ball, with a spatial index over the same mesh the
/// map is measured on. 0.5 mm cells and a 0.05 mm tolerance are the shipped
/// defaults' order of magnitude.
fn hemisphere_map() -> (TriangleMesh, SpatialIndex, ReachMap) {
    let mesh = make_test_hemisphere(10.0, 12);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(6.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &cutter, 0.05, 0.5);
    (mesh, index, map)
}

/// The load-bearing contract: one colour per mesh vertex.
#[test]
fn the_overlay_carries_exactly_one_colour_per_model_vertex() {
    let (mesh, index, map) = hemisphere_map();
    let colors = reach_overlay_colors(&map, &mesh, &index);
    assert_eq!(
        colors.len(),
        mesh.vertices.len(),
        "the reach overlay must colour every model vertex exactly once — \
         a mismatched vector paints the wrong part of the part"
    );
    assert!(
        !mesh.vertices.is_empty(),
        "the fixture must carry vertices, or the assertion above is vacuous"
    );
}

/// Every colour is a finite RGB triple inside `0..=1`.
///
/// A `NaN` gap is NOT an error here — it means "not measured", and
/// `reach_color` answers it with the model shader's own neutral diffuse
/// colour. What must never reach the vertex buffer is a non-finite or
/// out-of-range channel, which renders as an undefined colour rather than as
/// an honest abstention.
#[test]
fn every_reach_colour_is_a_finite_channel_triple() {
    let (mesh, index, map) = hemisphere_map();
    let colors = reach_overlay_colors(&map, &mesh, &index);
    for (i, color) in colors.iter().enumerate() {
        for (channel, value) in color.iter().enumerate() {
            assert!(
                value.is_finite() && (0.0..=1.0).contains(value),
                "vertex {i} channel {channel} is {value}, which is not a colour"
            );
        }
    }
}

/// The frame claim the overlay rests on.
///
/// The map is measured on the setup-transformed mesh; the viewport draws the
/// same mesh through `transform_mesh`. Both are
/// `SetupTransformInfo::apply_to_mesh`, which maps vertices one to one and
/// clones the triangle list. The transformed mesh therefore takes the map's
/// colours by index, even though its COORDINATES differ.
#[test]
fn the_setup_transform_preserves_vertex_order() {
    use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};

    let (mesh, index, map) = hemisphere_map();
    let colors = reach_overlay_colors(&map, &mesh, &index);

    let info = SetupTransformInfo {
        face_up: FaceUp::Top,
        z_rotation: ZRotation::Deg0,
        stock_x: 40.0,
        stock_y: 40.0,
        stock_z: 20.0,
        stock_origin_x: 3.0,
        stock_origin_y: -4.0,
        stock_origin_z: -20.0,
    };
    let displayed = info.apply_to_mesh(&mesh);

    assert_eq!(
        displayed.vertices.len(),
        colors.len(),
        "the display transform changed the vertex count — the overlay's \
         colours would no longer line up with the drawn mesh"
    );
    assert_eq!(
        displayed.triangles, mesh.triangles,
        "the display transform reordered or rewrote the triangle list — \
         vertex indices are no longer shared between the two frames"
    );
    // The coordinates DO move; only the ordering is claimed.
    assert!(
        (displayed.vertices[0].x - mesh.vertices[0].x).abs() > 1e-9,
        "the fixture's stock origin must actually shift the mesh, or this \
         test would pass on an identity transform"
    );
}
