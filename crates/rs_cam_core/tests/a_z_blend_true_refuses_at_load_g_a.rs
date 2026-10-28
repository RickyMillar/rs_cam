//! GUI fix G-A (`planning/adaptive_research_2026-10-23/integration/
//! gui_fix_design.md` §5; operator ruling D-3, 2026-10-28): Z Blend left the
//! format. It cut the inner Contour Parallel rings deeper than one
//! Depth/Pass (4.854 mm at a 3 mm Depth/Pass on the design's probe), and
//! Geodesic Adaptive never read it.
//!
//! 1. `z_blend = false`: every 3D Rough that an older build saved holds it.
//!    The file loads, and the key is ignored: the output does not change.
//! 2. `z_blend = true` (a TOML boolean, not a string): the file refuses with
//!    `SessionError::RetiredKeys`, which names the key and the fix. A
//!    Contour Parallel op would otherwise change its output with no word.
//! 3. The parameter door (`set_toolpath_param`: MCP and the job file)
//!    refuses `z_blend` with the same fix text, for any value: the field is
//!    gone.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_z_blend_true_refuses_at_load_g_a
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, square_polygon, toolpath_config};

use std::path::PathBuf;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::session::{
    Command, ProjectFile, ProjectSession, ProjectSessionBuilder, SessionError, SetToolpathParamArgs,
};

fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(polygon_model(vec![square_polygon(10.0)], "square"));
    let tc = toolpath_config(
        "CP",
        OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    builder.build()
}

fn saved_raw(session: &ProjectSession) -> toml::Value {
    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!("rs_cam_z_blend_g_a_{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path.push("project.toml");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    toml::from_str(&text).unwrap()
}

/// The saved text with `z_blend = value` in the op's parameter table.
fn with_z_blend(raw: &toml::Value, value: bool) -> String {
    let mut raw = raw.clone();
    raw["setups"][0]["toolpaths"][0]["operation"]["params"]
        .as_table_mut()
        .unwrap()
        .insert("z_blend".to_owned(), toml::Value::Boolean(value));
    toml::to_string(&raw).unwrap()
}

#[test]
fn a_z_blend_true_refuses_at_load_g_a() {
    let raw = saved_raw(&session());
    assert!(
        raw["setups"][0]["toolpaths"][0]["operation"]["params"]
            .get("z_blend")
            .is_none(),
        "a new save writes no z_blend key"
    );

    // 1. `false` loads, the key ignored.
    let _ = ProjectFile::from_toml_str(&with_z_blend(&raw, false)).expect("z_blend = false loads");

    // 2. `true` refuses and names the key and the fix.
    let Err(err) = ProjectFile::from_toml_str(&with_z_blend(&raw, true)) else {
        panic!("z_blend = true must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    assert_eq!(
        found.iter().map(|k| k.path.as_str()).collect::<Vec<_>>(),
        vec!["setups[0].toolpaths[0].operation.params.z_blend = true"]
    );
    let message = err.to_string();
    assert!(
        message.contains("Depth/Pass") && message.contains("Delete the key"),
        "the refusal names the cause and the fix: {message}"
    );

    // 3. The parameter door refuses z_blend with the same fix, any value.
    let mut live = session();
    for value in [serde_json::json!(true), serde_json::json!(false)] {
        let Err(err) = live.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "z_blend".to_owned(),
            value: value.clone(),
        })) else {
            panic!("setting z_blend = {value} must refuse");
        };
        let message = err.to_string();
        assert!(
            message.contains("Depth/Pass") && message.contains("Delete the key"),
            "z_blend = {value}: the setter names the fix: {message}"
        );
    }
}
