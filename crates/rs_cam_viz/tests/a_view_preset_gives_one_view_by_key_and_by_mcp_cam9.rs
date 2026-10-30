//! CAM-9 (design A, commit C5): a view preset, a fit and a pose give one
//! view by key and by MCP.
//!
//! # The claim
//!
//! 1. For each preset (R5b: the six view cube faces and Iso) and each
//!    projection, the key path
//!    (`CameraState::preset`) and the MCP door (`apply_camera_request` with
//!    `preset`) give a bitwise-equal `view_proj(16/9)`, and the echo names
//!    the preset. The wire tokens of the three new presets parse to them.
//! 2. `fit: model` and Reset View (`fit_to_first_model`) give an equal
//!    camera.
//! 3. The pose echo replays: degrees on the wire become radians in the
//!    crate, and the echo of a replayed echo equals the first echo within
//!    1e-9 degrees. The first trip from a typed degree value is within
//!    1e-4 degrees, because the crate holds the angle as an `f32`.
//! 4. A C6: the F key (`fit_selection`, eased, run to its end) and MCP
//!    `fit: selection` give a bitwise-equal view; with no selected
//!    toolpath the key falls back to Reset View.

#![cfg(feature = "mcp")]
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::ItemSet;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
use rs_cam_mcp::server::{CameraParam, CameraPoseParam};
use rs_cam_viz::render::camera::{ProjectionMode, ViewPreset};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::camera::{
    CameraEcho, CameraRequest, CameraWrite, FitTarget, apply_camera_request, fit_selection,
    fit_to_first_model,
};

fn bits(m: [[f32; 4]; 4]) -> [[u32; 4]; 4] {
    m.map(|row| row.map(f32::to_bits))
}

/// A state with one model: a triangle 40 x 30 x 0 mm.
fn state_with_a_model() -> AppState {
    let mesh = TriangleMesh::from_raw(
        vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(40.0, 0.0, 0.0),
            P3::new(0.0, 30.0, 5.0),
        ],
        vec![[0, 1, 2]],
    );
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "triangle.stl".to_owned(),
        mesh: Some(Arc::new(mesh)),
        items: Arc::new(ItemSet::default()),
        path: PathBuf::from("triangle.stl"),
        kind: Some(ModelKind::Stl),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_model(model);
    let mut state = AppState::new();
    state.replace_session(builder.build());
    state
}

#[test]
fn a_view_preset_gives_one_view_by_key_and_by_mcp_cam9() {
    let mut checked = 0;
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        for preset in ViewPreset::ALL {
            let mut by_key = AppState::new();
            by_key.camera.set_projection(projection);
            by_key.camera.preset(preset, CameraWrite::Instant);
            let mut by_mcp = AppState::new();
            let echo = apply_camera_request(
                &mut by_mcp,
                &CameraRequest {
                    preset: Some(preset),
                    projection: Some(projection),
                    ..CameraRequest::default()
                },
            )
            .expect("a preset is never refused");
            assert_eq!(
                bits(by_key.camera.pose().view_proj(16.0 / 9.0)),
                bits(by_mcp.camera.pose().view_proj(16.0 / 9.0)),
                "{projection:?} {preset:?}"
            );
            assert_eq!(echo.view_preset, Some(preset), "the echo names the preset");
            checked += 1;
        }
    }
    assert_eq!(checked, 14, "every arm ran: seven presets, two projections");
}

/// A C4: the eased key path ends on the same view as the MCP preset.
#[test]
fn an_eased_preset_ends_on_the_mcp_view_cam9() {
    let duration = 0.24;
    for preset in ViewPreset::ALL {
        let mut by_key = AppState::new();
        by_key
            .camera
            .preset(preset, CameraWrite::Eased { now: 0.0, duration });
        assert!(!by_key.camera.advance(duration + 1.0), "the move ended");
        let mut by_mcp = AppState::new();
        let _ = apply_camera_request(
            &mut by_mcp,
            &CameraRequest {
                preset: Some(preset),
                ..CameraRequest::default()
            },
        )
        .expect("a preset is never refused");
        assert_eq!(
            bits(by_key.camera.pose().view_proj(16.0 / 9.0)),
            bits(by_mcp.camera.pose().view_proj(16.0 / 9.0)),
            "{preset:?}"
        );
    }
}

#[test]
fn fit_model_is_reset_view_cam9() {
    let mut by_key = state_with_a_model();
    fit_to_first_model(&mut by_key, CameraWrite::Instant);
    let mut by_mcp = state_with_a_model();
    let echo = apply_camera_request(
        &mut by_mcp,
        &CameraRequest {
            fit: Some(FitTarget::Model),
            ..CameraRequest::default()
        },
    )
    .expect("the project has a model");
    assert_eq!(by_key.camera.pose(), by_mcp.camera.pose());
    let framed = echo.framed.expect("the echo carries the framed box");
    assert_eq!(
        [framed.max.x, framed.max.y, framed.max.z],
        [40.0, 30.0, 5.0]
    );
}

/// The degrees of an echo, as the reply writes them.
fn degrees(echo: &CameraEcho) -> (f64, f64) {
    (
        f64::from(echo.pose.yaw).to_degrees(),
        f64::from(echo.pose.pitch).to_degrees(),
    )
}

fn pose_request(yaw_deg: f64, pitch_deg: f64) -> CameraRequest {
    CameraParam {
        pose: Some(CameraPoseParam {
            yaw_deg: Some(yaw_deg),
            pitch_deg: Some(pitch_deg),
            ..CameraPoseParam::default()
        }),
        ..CameraParam::default()
    }
    .into()
}

#[test]
fn the_pose_echo_replays_cam9() {
    for (yaw, pitch) in [(30.0, 20.0), (-135.5, -45.25), (0.1, 89.4)] {
        let mut state = AppState::new();
        let first = apply_camera_request(&mut state, &pose_request(yaw, pitch))
            .expect("a pose in range is never refused");
        let (yaw1, pitch1) = degrees(&first);
        assert!(
            (yaw1 - yaw).abs() < 1e-4 && (pitch1 - pitch).abs() < 1e-4,
            "({yaw}, {pitch}) came back as ({yaw1}, {pitch1})"
        );
        let second = apply_camera_request(&mut state, &pose_request(yaw1, pitch1))
            .expect("the echo replays");
        let (yaw2, pitch2) = degrees(&second);
        assert!(
            (yaw2 - yaw1).abs() < 1e-9 && (pitch2 - pitch1).abs() < 1e-9,
            "the replayed echo moved: ({yaw1}, {pitch1}) -> ({yaw2}, {pitch2})"
        );
    }
}

/// A state with a drawing of two loops, one Pocket at its default
/// selection, and that toolpath focused.
fn state_with_a_selected_pocket() -> AppState {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::geo::P2;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::selection::{Item, Shape, SourceRef};
    let square = |x: f64| {
        Item::new(
            SourceRef::ToolpathModel,
            Some("CUT".into()),
            Shape::Loop(Polygon2::new(vec![
                P2::new(x, 0.0),
                P2::new(x + 10.0, 0.0),
                P2::new(x + 10.0, 10.0),
                P2::new(x, 10.0),
            ])),
        )
        .expect("a square loop")
    };
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "loops.dxf".to_owned(),
        mesh: None,
        items: Arc::new(ItemSet::new(vec![square(0.0), square(30.0)])),
        path: PathBuf::from("loops.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new();
    let model_id = builder.add_model(model);
    let tc = rs_cam_core::session::ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: None,
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
    let _ = builder.add_toolpath(0, tc).expect("setup 0");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let id = state.session().toolpath_configs()[0].id;
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(id));
    state
}

/// A C6: the F key (`fit_selection`, eased, run to its end) gives the view
/// of MCP `fit: selection`; with no selected toolpath it is Reset View.
#[test]
fn the_f_key_fits_the_selection_as_mcp_does_cam6() {
    let duration = 0.24;
    let mut by_key = state_with_a_selected_pocket();
    fit_selection(&mut by_key, CameraWrite::Eased { now: 0.0, duration });
    assert!(by_key.camera.is_moving(), "the key path eases");
    assert!(!by_key.camera.advance(duration + 1.0), "the move ended");
    let mut by_mcp = state_with_a_selected_pocket();
    let echo = apply_camera_request(
        &mut by_mcp,
        &CameraRequest {
            fit: Some(FitTarget::Selection),
            ..CameraRequest::default()
        },
    )
    .expect("the selected Pocket holds items");
    assert!(echo.framed.is_some());
    assert_eq!(
        bits(by_key.camera.pose().view_proj(16.0 / 9.0)),
        bits(by_mcp.camera.pose().view_proj(16.0 / 9.0)),
        "the F key and MCP fit: selection give one view"
    );

    // The fallback: no selected toolpath, so the key path is Reset View.
    let mut by_key = state_with_a_model();
    fit_selection(&mut by_key, CameraWrite::Instant);
    let mut reset = state_with_a_model();
    fit_to_first_model(&mut reset, CameraWrite::Instant);
    assert_eq!(by_key.camera.pose(), reset.camera.pose());
}

/// R5b: the wire tokens of the three new presets parse to them, and a pose
/// at each preset echoes its preset name.
#[test]
fn the_three_new_presets_have_wire_tokens_cam9() {
    for (token, preset) in [
        ("bottom", ViewPreset::Bottom),
        ("back", ViewPreset::Back),
        ("left", ViewPreset::Left),
    ] {
        let param: CameraParam =
            serde_json::from_value(serde_json::json!({ "preset": token })).expect("a known token");
        let request: CameraRequest = param.into();
        assert_eq!(request.preset, Some(preset), "`{token}`");
        let mut state = AppState::new();
        let echo = apply_camera_request(&mut state, &request).expect("a preset is never refused");
        assert_eq!(echo.view_preset, Some(preset), "the echo names `{token}`");
    }
}
