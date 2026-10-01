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

/// Audit WRONG #2 (2026-10-02): the legend's reach caveats print the area
/// base and the over-statement sentence once each.
///
/// The grid line used to be `ReachMap::grid_note`, which already holds both
/// sentences, and the legend printed each of them on its own line too. So
/// the hover detail said the area base twice and the over-statement twice.
/// The legend now prints `ReachMap::grid_line`. The bar here is far under
/// the floor, so the over-statement arm runs.
#[test]
fn the_legend_states_each_reach_sentence_once() {
    use std::sync::Arc;

    use rs_cam_viz::state::AppState;
    use rs_cam_viz::state::runtime::ReachStatus;
    use rs_cam_viz::ui::overlays::legend_rail::{self, RailLine};
    use rs_cam_viz::ui::overlays::registry::Legend;

    let mesh = make_test_hemisphere(10.0, 12);
    let cutter = BallEndmill::new(6.0, 25.0);
    let map = reach_map_for_mesh(&mesh, &cutter, 0.0001, 0.5);
    assert!(map.is_measured(), "the fixture must produce a population");
    assert!(
        map.tolerance_below_floor(),
        "the fixture must put the bar under the floor, or the over-statement \
         arm never runs: tol {:.4} against floor {:.4}",
        map.tolerance_mm,
        map.discretisation_floor_mm
    );
    let area = map.area_basis_note();
    let over = map.over_statement_note();
    let line = RailLine::Scale(Legend::Reach(map.ramp()));

    let mut state = AppState::new();
    state.gui.reach_overlay.status = ReachStatus::Ready(Arc::new(map));
    let text = legend_rail::caveat_lines(&state, &line)
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        text.matches(&area).count(),
        1,
        "the area base must show once in the legend:\n{text}"
    );
    assert_eq!(
        text.matches(&over).count(),
        1,
        "the over-statement sentence must show once in the legend:\n{text}"
    );
    assert!(
        text.contains("UNDER the floor"),
        "the legend must still mark the bar under the floor:\n{text}"
    );
}
