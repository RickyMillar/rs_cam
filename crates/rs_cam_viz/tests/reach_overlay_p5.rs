//! P5 (GUI half) — the reach-map overlay's colour seam.
//!
//! The overlay draws the MODEL mesh, refined to the map cell (round 7),
//! through `reach_mesh_pipeline` with one reach colour per vertex, so the
//! whole overlay rests on one arithmetic
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
//!
//! Round 7 (FOLLOW_UPS §12): the `_r7` arms. A STEP block tessellates to
//! 48 vertices, each on the part outline or at a wall foot, so a colour per
//! model vertex showed no reach colour at all. The overlay refines the mesh
//! to the map cell (`reach_overlay_surface`), the upload replays the
//! refinement on the drawn mesh, and the colour scale is one tip radius.

#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the r7 arm prints its counts"
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
    let colors = reach_overlay_colors(&map, &mesh, &index, map.ramp());
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
    let colors = reach_overlay_colors(&map, &mesh, &index, map.ramp());
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
    let colors = reach_overlay_colors(&map, &mesh, &index, map.ramp());

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

/// The stepped block fixture of the round 7 live look, its Ø10 ball, the
/// cell the session takes for that ball (`ReachMapParams::for_cutter`) and
/// the bar of the live operation (its declared scallop, 0.01 mm).
fn stepped_block_map() -> (TriangleMesh, SpatialIndex, ReachMap, f64) {
    use rs_cam_core::maps::reach_map::ReachMapParams;
    use rs_cam_core::tool::MillingCutter;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gui_step/stepped_block.step");
    let enriched = rs_cam_core::io::step_input::load_step(&path, 0.01)
        .unwrap_or_else(|e| panic!("the fixture {} does not load: {e}", path.display()));
    let mesh = enriched.mesh.as_ref().clone();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(10.0, 25.0);
    let cell = ReachMapParams::for_cutter(&cutter, 0.01).cell_mm;
    let map = reach_map_for_mesh(&mesh, &cutter, 0.01, cell);
    (mesh, index, map, cutter.cusp_radius_mm())
}

/// The edges by position that one triangle only uses. A closed solid has
/// none; a T-junction or a crack adds them.
fn open_edges(mesh: &TriangleMesh) -> usize {
    use std::collections::HashMap;
    let bits = |i: u32| {
        let p = mesh.vertices[i as usize];
        [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
    };
    let mut uses: HashMap<[[u64; 3]; 2], usize> = HashMap::new();
    for t in &mesh.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (bits(a), bits(b));
            let key = if a <= b { [a, b] } else { [b, a] };
            *uses.entry(key).or_default() += 1;
        }
    }
    uses.values().filter(|&&n| n == 1).count()
}

/// The root cause and the fix. On the model vertices every colour is the
/// model material (no vertex is measured), although the map holds measured
/// cells. On the refined surface the faces show the field: a reached green
/// and a miss colour both appear, and the refinement adds no open edge.
#[test]
fn a_step_block_shows_the_reach_field_on_its_faces_r7() {
    use rs_cam_core::maps::reach_map::reach_color;
    use rs_cam_viz::render::colors::MODEL_SURFACE;
    use rs_cam_viz::state::runtime::reach_overlay_surface;

    let (mesh, index, map, radius) = stepped_block_map();
    assert!(
        map.cells.iter().flatten().count() > 0,
        "non-vacuity: the map must hold measured cells"
    );
    let plain = reach_overlay_colors(&map, &mesh, &index, map.ramp());
    assert!(
        plain.iter().all(|c| *c == MODEL_SURFACE.array()),
        "the model vertices are expected to carry no measured gap (the root cause): {plain:?}"
    );

    let surface = reach_overlay_surface(&map, &mesh, &index, radius);
    eprintln!(
        "r7 block: {} model vertices, {} added vertices, {} triangles, vertex budget {}",
        mesh.vertices.len(),
        surface.midpoints.len(),
        surface.triangles.len(),
        rs_cam_viz::state::runtime::reach_refine_budget(&map)
    );
    assert!(
        !surface.budget_hit,
        "the block must refine inside its budget"
    );
    assert!(
        surface.midpoints.len() <= rs_cam_viz::state::runtime::reach_refine_budget(&map),
        "the block passed the vertex budget in its last round"
    );
    let refined = surface
        .refined_mesh(&mesh)
        .unwrap_or_else(|| panic!("the surface must replay on its own mesh"));
    assert!(
        refined.vertices.len() > mesh.vertices.len(),
        "the refinement added no vertex"
    );
    assert_eq!(surface.colors.len(), refined.vertices.len());
    assert_eq!(open_edges(&mesh), 0, "non-vacuity: the block is closed");
    assert_eq!(
        open_edges(&refined),
        0,
        "the refinement opened an edge: a T-junction between two faces"
    );
    let reached = reach_color(0.0, f32::NAN, surface.ramp);
    assert!(
        surface.colors.contains(&reached),
        "no vertex is reached green on the faces"
    );
    let miss = surface.colors.iter().any(|c| c[0] > c[1] && c[0] > c[2]);
    assert!(miss, "no vertex takes a miss colour near the walls");
}

/// The upload replays the refinement on the drawn mesh. The display frame
/// is an affine image of the measured frame, so the refined drawn mesh is
/// the image of the refined measured mesh, vertex for vertex.
#[test]
fn the_refinement_replays_on_the_display_frame_r7() {
    use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
    use rs_cam_viz::state::runtime::reach_overlay_surface;

    let (mesh, index, map, radius) = stepped_block_map();
    let surface = reach_overlay_surface(&map, &mesh, &index, radius);
    let info = SetupTransformInfo {
        face_up: FaceUp::Top,
        z_rotation: ZRotation::Deg90,
        stock_x: 70.0,
        stock_y: 50.0,
        stock_z: 35.0,
        stock_origin_x: 3.0,
        stock_origin_y: -4.0,
        stock_origin_z: -35.0,
    };
    let measured = surface
        .refined_mesh(&mesh)
        .unwrap_or_else(|| panic!("replay on the measured mesh"));
    let drawn = surface
        .refined_mesh(&info.apply_to_mesh(&mesh))
        .unwrap_or_else(|| panic!("replay on the drawn mesh"));
    let expected = info.apply_to_mesh(&measured);
    assert_eq!(drawn.triangles, expected.triangles);
    for (a, b) in drawn.vertices.iter().zip(&expected.vertices) {
        assert!((a - b).norm() < 1e-9, "{a:?} against {b:?}");
    }
    assert!(
        (drawn.vertices[0] - measured.vertices[0]).norm() > 1e-6,
        "non-vacuity: the transform must move the mesh"
    );
}

/// The colour scale is one tip radius: the ramp ends at the radius, so a
/// gap of one radius is the full colour. A radius that is not above the
/// bar keeps the map's own deepest gap, because a log ramp then has no
/// span.
#[test]
fn the_colour_scale_is_one_tip_radius_r7() {
    use rs_cam_viz::state::runtime::reach_display_ramp;

    let (_, _, map, radius) = stepped_block_map();
    let ramp = reach_display_ramp(&map, radius);
    assert!(
        radius > ramp.bar_mm(),
        "non-vacuity: R {radius} over the bar"
    );
    assert!((ramp.max_gap_mm - radius).abs() < 1e-12, "{ramp:?}");
    assert!((ramp.depth_t(radius) - 1.0).abs() < 1e-12);
    assert!((ramp.depth_t(map.max_gap_mm) - 1.0).abs() < 1e-12);
    let fallback = reach_display_ramp(&map, ramp.bar_mm() * 0.5);
    assert!((fallback.max_gap_mm - map.max_gap_mm).abs() < 1e-12);
}

/// The legend strip names the tip radius: the top label of a
/// tool-relative strip reads "R = <radius> mm", the words of
/// `reach_scale_note`. A ramp that is not the surface's own (or no
/// surface) keeps the plain "<top> mm".
#[test]
fn the_legend_strip_names_the_tip_radius_r7() {
    use rs_cam_viz::state::AppState;
    use rs_cam_viz::state::runtime::reach_overlay_surface;
    use rs_cam_viz::ui::overlays::legend_rail::{self, RailLine};
    use rs_cam_viz::ui::overlays::registry::Legend;
    use std::sync::Arc;

    let (mesh, index, map, radius) = stepped_block_map();
    let surface = reach_overlay_surface(&map, &mesh, &index, radius);
    assert!(
        surface.tip_relative,
        "non-vacuity: the scale is tool-relative"
    );
    let line = RailLine::Scale(Legend::Reach(surface.ramp));
    let map_line = RailLine::Scale(Legend::Reach(map.ramp()));
    let tip_label = format!("R = {radius:.2} mm");

    let mut state = AppState::new();
    assert!(
        !legend_rail::key_labels(&state, &line).contains(&tip_label),
        "no surface: no R"
    );
    state.gui.reach_overlay.surface = Some(Arc::new(surface));
    let labels = legend_rail::key_labels(&state, &line);
    assert!(
        labels.contains(&tip_label),
        "the strip names the tip radius {tip_label:?}: {labels:?}"
    );
    assert!(
        !legend_rail::key_labels(&state, &map_line)
            .iter()
            .any(|l| l.starts_with("R = ")),
        "a ramp that is not the surface's keeps the plain label"
    );
}
