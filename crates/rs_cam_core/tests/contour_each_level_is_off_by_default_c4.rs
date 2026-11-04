//! C4 (operator ruling D69, 2026-10-28): "Contour each level" is one
//! visible 3D Rough option, off by default.
//!
//! 1. The default is off; a config with no key reads off; a config with the
//!    option off writes no key (saved projects do not change); the operation
//!    schema lists the field with its help; `set_toolpath_param` sets it.
//! 2. On a FRESH rough the option changes nothing: the contour acts only on
//!    remaining stock, where the boundary clear takes the reach band out of
//!    the planner stock. The rest case is
//!    `a_rest_op_holds_one_depth_at_the_band_c4`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: an expect or a panic is how a test reports a failure"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{Command, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs};
use std::sync::atomic::AtomicBool;

const KEY: &str = "contour_each_level";

fn rough(contour: bool) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(2.0),
        depth_per_pass: Some(3.0),
        clearing_strategy: ClearingStrategy::ContourParallel,
        contour_each_level: contour,
        ..Adaptive3dConfig::default()
    })
}

/// A fresh rough of a pocket inside a `Stock` boundary with `Inside`
/// containment.
fn fresh_session(contour: bool) -> ProjectSession {
    let mesh = height_field(20.0, 0.5, |x, y| {
        if x.abs() <= 10.0 && y.abs() <= 10.0 {
            4.0
        } else {
            10.0
        }
    });
    let mut stock = stock_over(20.0, 10.0);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(6.0));
    let tool = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "pocket"));
    let mut op = toolpath_config("rough", rough(contour), tool, model_id);
    op.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Inside,
        offset: 0.0,
    };
    builder.add_toolpath(0, op).expect("add the rough");
    builder.build()
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn contour_each_level_is_off_by_default_c4() {
    // The default is off.
    assert!(!Adaptive3dConfig::default().contour_each_level);

    // Off writes no key, so a saved project does not change.
    let off = toml::to_string(&Adaptive3dConfig::default()).expect("serialise");
    assert!(!off.contains(KEY), "the option off writes a key:\n{off}");

    // No key reads off; the key reads back.
    let back: Adaptive3dConfig = toml::from_str(&off).expect("deserialise");
    assert!(!back.contour_each_level);
    let on = toml::to_string(&Adaptive3dConfig {
        contour_each_level: true,
        ..Adaptive3dConfig::default()
    })
    .expect("serialise");
    assert!(on.contains(KEY));
    let back: Adaptive3dConfig = toml::from_str(&on).expect("deserialise");
    assert!(back.contour_each_level);

    // The schema lists it, with its help.
    let schema = ProjectSession::operation_schema("adaptive3d").expect("the 3D Rough schema");
    let field = schema
        .params
        .iter()
        .find(|p| p.name == KEY)
        .expect("the schema lists contour_each_level");
    assert!(
        field
            .description
            .as_deref()
            .is_some_and(|d| d.contains("Depth/Pass")),
        "the field has no help: {:?}",
        field.description
    );

    // `set_toolpath_param` sets it.
    let mut session = fresh_session(false);
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: KEY.to_owned(),
            value: serde_json::Value::Bool(true),
        }))
        .expect("set_toolpath_param contour_each_level");
    match &session.toolpath_configs()[0].operation {
        OperationConfig::Adaptive3d(cfg) => assert!(cfg.contour_each_level),
        other => panic!("not a 3D Rough: {other:?}"),
    }
}

#[test]
fn a_fresh_rough_ignores_contour_each_level_c4() {
    let moves = |contour: bool| {
        let mut session = fresh_session(contour);
        session
            .generate_toolpath(0, &AtomicBool::new(false))
            .expect("generate the rough");
        format!(
            "{:?}",
            session.result_at(0).expect("the result").toolpath().moves
        )
    };
    let off = moves(false);
    assert!(
        off.len() > 100,
        "the rough cut nothing: the fixture tests nothing"
    );
    assert_eq!(
        moves(true),
        off,
        "on a fresh rough the option must change nothing"
    );
}
