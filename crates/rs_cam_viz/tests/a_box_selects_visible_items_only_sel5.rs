//! SEL-5 (operator, live look 2026-10-29): the box selects only what the
//! eye sees. "Looking from the top, a box over the top three faces also
//! selects the base face underneath. We should only select things visible."
//!
//! # The claims
//!
//! 1. `visible_keys` on a two-face stack (an STL: two 20 mm squares, one at
//!    Z 0 and one at Z 10, one footprint): from above the upper face is
//!    visible and the lower face is not; from below the reverse. No egui.
//!    The two faces have two keys because an STL face key holds its plane
//!    Z (round 4, P3).
//! 2. The stepped block (`fixtures/gui_step/stepped_block.step`) from the
//!    Top view: a box over the whole part takes every up-facing step face
//!    and NOT the base face (it faces down) nor the Z 0 part outline.
//! 3. The same box from below: the base face, not the up faces; the Z 0
//!    outline is on the base face, so it is visible too.
//! 4. From the -X side at a low pitch, 12° (the block steps down along +X): the
//!    tall step face in front is taken; the two lower step faces behind it
//!    are not. (The block's walls give no face item: only flat up and down
//!    faces are items.)
//!
//! 5. A click takes the first VISIBLE candidate (operator ruling, live
//!    look round 5): on the stepped block in a Profile, a click at the
//!    middle step face picks that face, from the top and from a low side
//!    pose where the Z 0 outline (the base face's ring, under the part)
//!    passes within the pick threshold on the screen. Tab still lists the
//!    base face; with the loop chip alone the hidden Z 0 outline is not
//!    picked.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_box_selects_visible_items_only_sel5
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints the items it read"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::{Item, ItemKind, MatchKey, Shape, SourceRef};
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::interaction::PickHit;
use rs_cam_viz::interaction::picking::{
    PickContext, ScreenRect, pick_candidates, pick_items, pick_items_in_rect, visible_keys,
};
use rs_cam_viz::render::camera::{CameraPose, OrbitCamera};
use rs_cam_viz::state::overlays::ModelVisibility;
use rs_cam_viz::state::pick::{PickFilter, held_kinds};
use rs_cam_viz::state::viewport_input::BoxMode;

const VIEW_W: f32 = 800.0;
const VIEW_H: f32 = 600.0;

/// A session of `model` with one toolpath of `op` on it, at setup 0, and a
/// stock box round the model.
fn session_of(model: LoadedModel, op: OperationType) -> ProjectSession {
    let bbox = model.mesh.as_ref().expect("a mesh model").bbox;
    let stock = StockConfig {
        x: bbox.max.x - bbox.min.x,
        y: bbox.max.y - bbox.min.y,
        z: Some((bbox.max.z - bbox.min.z).max(1.0)),
        origin_x: bbox.min.x,
        origin_y: bbox.min.y,
        origin_z: bbox.min.z,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().stock(stock).tool(tool);
    let model_id = builder.add_model(model);
    let mut operation = OperationConfig::new_default(op);
    rs_cam_core::compute::fixture_values::fill_unset_keep_floor(&mut operation);
    let tc = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: op.label().to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(ToolId::from_raw(1)),
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tc).unwrap();
    builder.build()
}

/// A camera that frames the model box, at `yaw` and `pitch` (radians).
fn camera(session: &ProjectSession, yaw: f32, pitch: f32) -> OrbitCamera {
    let bbox = session.models()[0].mesh.as_ref().unwrap().bbox;
    let mut camera = OrbitCamera::new();
    let fit = camera.fit_pose(
        [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32],
        [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32],
    );
    camera.set_pose(CameraPose { yaw, pitch, ..fit });
    camera
}

/// The keys a box over the whole viewport takes (enclose), before and after
/// the visibility rule.
fn whole_box(session: &ProjectSession, camera: &OrbitCamera) -> (Vec<MatchKey>, Vec<MatchKey>) {
    let visibility = ModelVisibility::default();
    let ctx = PickContext {
        camera,
        screen_x: 0.0,
        screen_y: 0.0,
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &visibility,
    };
    let tc = &session.toolpath_configs()[0];
    let item_use = tc.operation.capabilities().item_use;
    let filter = PickFilter::served(item_use);
    let held = held_kinds(&session.models()[0]);
    let kinds = filter.effective_kinds(item_use, &held);
    let candidates = pick_items_in_rect(
        &ctx,
        session,
        tc.id,
        &filter,
        &kinds,
        ScreenRect::from_corners([0.0, 0.0], [f64::from(VIEW_W), f64::from(VIEW_H)]),
        BoxMode::Enclose,
    );
    let visible = visible_keys(&ctx, session, tc.id, candidates.clone());
    (candidates, visible)
}

fn holds(keys: &[MatchKey], item: &Item) -> bool {
    keys.iter().any(|k| k.names_same_geometry(item.key()))
}

// ── claim 1: a two-face stack, no egui ───────────────────────────────

/// Two 20 mm squares on one footprint: one at Z 0 (facing down), one at
/// Z 10 (facing up). Each is fully behind the other from one side.
fn stack_mesh() -> TriangleMesh {
    let v = |x: f64, y: f64, z: f64| P3::new(x, y, z);
    let vertices = vec![
        v(0.0, 0.0, 0.0),
        v(20.0, 0.0, 0.0),
        v(20.0, 20.0, 0.0),
        v(0.0, 20.0, 0.0),
        v(0.0, 0.0, 10.0),
        v(20.0, 0.0, 10.0),
        v(20.0, 20.0, 10.0),
        v(0.0, 20.0, 10.0),
    ];
    // Lower square wound clockwise from above (faces down); upper square
    // counter-clockwise (faces up).
    let triangles = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7]];
    TriangleMesh::from_raw(vertices, triangles)
}

#[test]
fn a_face_behind_another_is_not_visible_sel5() {
    let mesh = Arc::new(stack_mesh());
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "stack.stl".to_owned(),
        mesh: Some(Arc::clone(&mesh)),
        items: Default::default(),
        path: PathBuf::from("stack.stl"),
        kind: Some(ModelKind::Stl),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let session = session_of(model, OperationType::Pocket);
    // The flat-region faces of the STL (the explicit build, as a face
    // listing does).
    let faces = rs_cam_core::maps::geom_cache::cached_flat_regions(&mesh);
    let face_at = |z: f64| -> MatchKey {
        let item = faces
            .iter()
            .find(|i| matches!(i.shape(), Shape::Face(f) if (f.plane_z - z).abs() < 1e-6))
            .unwrap_or_else(|| panic!("a flat face at Z {z}"));
        MatchKey {
            source: SourceRef::ToolpathModel,
            ..item.key().clone()
        }
    };
    let (lower, upper) = (face_at(0.0), face_at(10.0));
    assert!(
        !lower.names_same_geometry(&upper),
        "P3: two STL faces on one footprint at two heights have two keys"
    );
    let visibility = ModelVisibility::default();
    let tc_id = session.toolpath_configs()[0].id;
    let limit = 89.0f32.to_radians();
    for (pitch, seen, hidden) in [(limit, &upper, &lower), (-limit, &lower, &upper)] {
        let camera = camera(&session, -std::f32::consts::FRAC_PI_2, pitch);
        let ctx = PickContext {
            camera: &camera,
            screen_x: 0.0,
            screen_y: 0.0,
            aspect: VIEW_W / VIEW_H,
            vw: VIEW_W,
            vh: VIEW_H,
            visibility: &visibility,
        };
        let kept = visible_keys(&ctx, &session, tc_id, vec![upper.clone(), lower.clone()]);
        assert!(
            kept.iter().any(|k| k.names_same_geometry(seen)),
            "pitch {pitch}: the near face is visible: {kept:?}"
        );
        assert!(
            !kept.iter().any(|k| k.names_same_geometry(hidden)),
            "pitch {pitch}: the face behind is not visible: {kept:?}"
        );
    }
}

// ── claims 2 to 4: the stepped block ─────────────────────────────────

fn stepped_block() -> ProjectSession {
    stepped_block_with(OperationType::Pocket)
}

fn stepped_block_with(op: OperationType) -> ProjectSession {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gui_step/stepped_block.step");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the STEP fixture loads");
    session_of(model, op)
}

/// The face items of the block, by facing: (up, down, walls).
fn faces_by_facing(session: &ProjectSession) -> (Vec<&Item>, Vec<&Item>, Vec<(&Item, P3)>) {
    let (mut up, mut down, mut walls) = (Vec::new(), Vec::new(), Vec::new());
    for item in session.models()[0].items.iter() {
        if let Shape::Face(face) = item.shape() {
            let n = face.normal;
            if n.z > 0.99 {
                up.push(item);
            } else if n.z < -0.99 {
                down.push(item);
            } else {
                walls.push((item, P3::new(n.x, n.y, n.z)));
            }
        }
    }
    (up, down, walls)
}

fn outline(session: &ProjectSession) -> &Item {
    session.models()[0]
        .items
        .iter()
        .find(|i| i.is_part_outline())
        .expect("a STEP model holds its part outline")
}

#[test]
fn from_the_top_the_box_takes_the_up_faces_not_the_base_sel5() {
    let session = stepped_block();
    let (up, down, walls) = faces_by_facing(&session);
    eprintln!(
        "stepped block: {} up faces, {} down faces, {} wall faces",
        up.len(),
        down.len(),
        walls.len()
    );
    assert_eq!(up.len(), 3, "fixture: three up-facing step faces");
    assert_eq!(down.len(), 1, "fixture: one base face");
    let top = 89.0f32.to_radians();
    let camera = camera(&session, -std::f32::consts::FRAC_PI_2, top);
    let (candidates, visible) = whole_box(&session, &camera);
    for item in up.iter().chain(down.iter()) {
        assert!(
            holds(&candidates, item),
            "fixture: the box holds every flat face"
        );
    }
    for item in &up {
        assert!(holds(&visible, item), "an up face is visible from the top");
    }
    assert!(
        !holds(&visible, down[0]),
        "the base face underneath is not visible from the top"
    );
    let outline = outline(&session);
    assert!(
        holds(&candidates, outline),
        "fixture: the box holds the Z 0 outline"
    );
    assert!(
        !holds(&visible, outline),
        "the Z 0 outline under the part is not visible from the top"
    );
}

#[test]
fn from_below_the_box_takes_the_base_face_only_sel5() {
    let session = stepped_block();
    let (up, down, _) = faces_by_facing(&session);
    let below = -89.0f32.to_radians();
    let camera = camera(&session, -std::f32::consts::FRAC_PI_2, below);
    let (_, visible) = whole_box(&session, &camera);
    assert!(
        holds(&visible, down[0]),
        "the base face is visible from below"
    );
    for item in &up {
        assert!(!holds(&visible, item), "an up face is hidden from below");
    }
    assert!(
        holds(&visible, outline(&session)),
        "the Z 0 outline lies on the base face: visible from below"
    );
}

#[test]
fn from_the_low_side_the_box_takes_the_near_face_not_the_steps_behind_sel5() {
    let session = stepped_block();
    let (up, _, _) = faces_by_facing(&session);
    // The block steps down along +X: Z 30 at X 0..20, Z 20 at X 20..40,
    // Z 10 at X 40..60. The camera at -X (yaw 180°) looks along +X at a
    // pitch of 12°: the eye (108 mm from the box centre) sits at Z 37, above
    // the tall step, and the tall step hides the lower steps behind it. (At
    // 5° the eye is at Z 24, under the tall step's top, which is then
    // rightly hidden too.)
    let camera = camera(&session, std::f32::consts::PI, 12.0f32.to_radians());
    let (candidates, visible) = whole_box(&session, &camera);
    let at = |z: f64| -> &Item {
        up.iter()
            .copied()
            .find(|i| i.plane_z().is_some_and(|p| (p - z).abs() < 1e-6))
            .unwrap_or_else(|| panic!("fixture: an up face at Z {z}"))
    };
    for z in [30.0, 20.0, 10.0] {
        assert!(
            holds(&candidates, at(z)),
            "fixture: the box holds the face at Z {z}"
        );
    }
    assert!(
        holds(&visible, at(30.0)),
        "the near (tall) step face is taken"
    );
    for z in [20.0, 10.0] {
        assert!(
            !holds(&visible, at(z)),
            "the step face at Z {z}, behind the tall step, is not taken"
        );
    }
}

#[test]
fn the_box_release_runs_the_visibility_rule_sel5() {
    let viewport = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/viewport.rs"),
    )
    .unwrap();
    let at = viewport
        .find("fn handle_box_release(")
        .expect("the box release handler");
    let body = &viewport[at..];
    let rect = body
        .find("pick_items_in_rect(")
        .expect("the box candidates");
    let visible = body.find("visible_keys(").expect("the visibility rule");
    let release = body
        .find("ViewportGesture::BoxRelease")
        .expect("the gesture");
    assert!(
        rect < visible && visible < release,
        "the box filters its candidates by visibility before the edit (both boxes)"
    );
}

// ── claim 5: a click takes the first visible candidate ───────────────

/// The pick-mode click and the Tab candidates at the display point `at`
/// of a Profile on the stepped block, with the camera at `yaw`, `pitch`.
fn click_at(
    session: &ProjectSession,
    yaw: f32,
    pitch: f32,
    at: [f32; 3],
    kinds: Option<&[ItemKind]>,
) -> (Option<PickHit>, Vec<PickHit>) {
    let camera = camera(session, yaw.to_radians(), pitch.to_radians());
    let s = camera
        .project_to_screen(at, VIEW_W / VIEW_H, VIEW_W, VIEW_H)
        .expect("the point is in front of the camera");
    let visibility = ModelVisibility::default();
    let ctx = PickContext {
        camera: &camera,
        screen_x: s[0],
        screen_y: s[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &visibility,
    };
    let tc = &session.toolpath_configs()[0];
    let item_use = tc.operation.capabilities().item_use;
    let filter = PickFilter::served(item_use);
    let held = held_kinds(&session.models()[0]);
    let served = filter.effective_kinds(item_use, &held);
    let kinds = kinds.unwrap_or(&served);
    (
        pick_items(&ctx, session, tc.id, &filter, kinds),
        pick_candidates(&ctx, session, tc.id, &filter, kinds),
    )
}

fn face_id(hit: &PickHit) -> Option<u16> {
    match hit {
        PickHit::ModelFace { face_id, .. } => Some(face_id.0),
        _ => None,
    }
}

fn loop_z(hit: &PickHit) -> Option<f64> {
    match hit {
        PickHit::Item { key, .. } if key.kind == ItemKind::Loop => key.z,
        _ => None,
    }
}

#[test]
fn a_click_takes_the_visible_face_not_the_base_under_it_sel5() {
    let session = stepped_block_with(OperationType::Profile);
    let (up, down, _) = faces_by_facing(&session);
    let middle = up
        .iter()
        .find(|i| i.plane_z().is_some_and(|z| (z - 20.0).abs() < 1e-6))
        .expect("fixture: the middle step face at Z 20");
    let middle_id = middle.key().brep.expect("a STEP face item");
    let base_id = down[0].key().brep.expect("a STEP face item");
    let [ax, ay] = middle.key().anchor;
    // From the top at the centre of the face. From a low side pose the
    // tall step hides the centre, so that arm clicks a visible point of
    // the face, 10 mm off the centre in Y, where the Z 0 outline (the base
    // face's ring, under the part) passes within the pick threshold on the
    // screen; before the rule that ring won the click.
    let arms = [
        (-90.0, 89.0, [ax as f32, ay as f32, 20.0]),
        (-150.0, 50.0, [ax as f32, ay as f32 + 10.0, 20.0]),
    ];
    for (yaw, pitch, at) in arms {
        let (hit, candidates) = click_at(&session, yaw, pitch, at, None);
        let hit = hit.expect("the click picks");
        assert_eq!(
            face_id(&hit),
            Some(middle_id),
            "yaw {yaw}, pitch {pitch}: the click picks the middle face, got {hit:?}"
        );
        // Tab still reaches what the click passes over: from the top the
        // base face under the middle face, from the side the Z 0 outline.
        assert!(
            candidates
                .iter()
                .any(|c| face_id(c) == Some(base_id) || loop_z(c) == Some(0.0)),
            "yaw {yaw}, pitch {pitch}: Tab still reaches a hidden candidate"
        );
    }
    let low = arms[1];
    // With the loop chip alone, the hidden Z 0 outline is not picked.
    let (hit, candidates) = click_at(&session, low.0, low.1, low.2, Some(&[ItemKind::Loop]));
    assert!(
        candidates.iter().any(|c| loop_z(c) == Some(0.0)),
        "fixture: the Z 0 outline passes near the centre on the screen"
    );
    assert!(
        hit.as_ref().and_then(loop_z) != Some(0.0),
        "the click does not pick the Z 0 outline under the part, got {hit:?}"
    );
}
