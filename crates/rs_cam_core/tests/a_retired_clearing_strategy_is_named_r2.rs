//! Removal step R2 (`planning/adaptive_research_2026-10-23/integration/
//! removal_design.md` §3.2; operator ruling 2026-10-28, option B): the 3D
//! clearing strategies `adaptive`, `agent_search` and `contour_spiral` left
//! the format. A file that names one refuses as a whole at load, names
//! every hit and the fix, and converts nothing: another strategy cuts
//! another toolpath, so the operator chooses.
//!
//! A unit variant saves as a VALUE (`clearing_strategy = "agent_search"`),
//! so serde alone would refuse it with "unknown variant" and name no fix.
//! The `RETIRED_VALUES` rows of `session/project_file.rs` find it first.
//!
//! 1. Control: a saved project with two 3D Rough ops (Contour Parallel and
//!    Geodesic Adaptive) loads.
//! 2. The same text with three ops on the three removed values refuses
//!    with `SessionError::RetiredKeys`, which names all three and the fix.
//! 3. The parameter door (`set_toolpath_param`: MCP and the job file) refuses
//!    a removed value with the same fix text, not serde's "unknown variant".
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_retired_clearing_strategy_is_named_r2
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
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::session::{ProjectFile, ProjectSession, ProjectSessionBuilder, SessionError};

const REMOVED: [&str; 3] = ["adaptive", "agent_search", "contour_spiral"];

fn temp_path() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rs_cam_retired_clearing_strategy_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// A session with three 3D Rough ops: Contour Parallel, Geodesic Adaptive,
/// Contour Parallel.
fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(polygon_model(vec![square_polygon(10.0)], "square"));
    for (name, strategy) in [
        ("CP", ClearingStrategy::ContourParallel),
        ("Geodesic", ClearingStrategy::GeodesicAdaptive),
        ("CP 2", ClearingStrategy::ContourParallel),
    ] {
        let tc = toolpath_config(
            name,
            OperationConfig::Adaptive3d(Adaptive3dConfig {
                clearing_strategy: strategy,
                ..Adaptive3dConfig::default()
            }),
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id,
        );
        let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    }
    builder.build()
}

fn saved_text(session: &ProjectSession) -> String {
    let path = temp_path();
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    text
}

#[test]
fn a_retired_clearing_strategy_is_named_r2() {
    let text = saved_text(&session());

    // 1. Control: the two kept values save as string values and load.
    let raw: toml::Value = toml::from_str(&text).unwrap();
    let key = |i: usize| {
        raw["setups"][0]["toolpaths"][i]["operation"]["params"]["clearing_strategy"]
            .as_str()
            .map(str::to_owned)
    };
    assert_eq!(key(0).as_deref(), Some("contour_parallel"), "{text}");
    assert_eq!(key(1).as_deref(), Some("geodesic_adaptive"), "{text}");
    let _ = ProjectFile::from_toml_str(&text).expect("control: the saved project loads");

    // 2. The three removed values, one per op.
    let mut old = raw.clone();
    let toolpaths = old["setups"][0]["toolpaths"].as_array_mut().unwrap();
    for (i, value) in REMOVED.iter().enumerate() {
        toolpaths[i]["operation"]["params"]
            .as_table_mut()
            .unwrap()
            .insert(
                "clearing_strategy".to_owned(),
                toml::Value::String((*value).to_owned()),
            );
    }
    let old = toml::to_string(&old).unwrap();
    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a project with a removed clearing strategy must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].operation.params.clearing_strategy = \"adaptive\"",
            "setups[0].toolpaths[1].operation.params.clearing_strategy = \"agent_search\"",
            "setups[0].toolpaths[2].operation.params.clearing_strategy = \"contour_spiral\"",
        ],
        "one refusal must name every removed value"
    );
    let message = err.to_string();
    assert!(
        message.contains("geodesic_adaptive") && message.contains("contour_parallel"),
        "the refusal must name the fix: {message}"
    );

    // 3. The parameter door refuses with the same fix, for each value.
    let mut live = session();
    for value in REMOVED {
        let Err(err) = live.apply(rs_cam_core::session::Command::SetToolpathParam(
            rs_cam_core::session::SetToolpathParamArgs {
                index: 0,
                param: "clearing_strategy".to_owned(),
                value: serde_json::json!(value),
            },
        )) else {
            panic!("setting clearing_strategy = {value} must refuse");
        };
        let message = err.to_string();
        assert!(
            message.contains("geodesic_adaptive")
                && message.contains("contour_parallel")
                && !message.contains("unknown variant"),
            "{value}: the setter must name the fix, not serde's error: {message}"
        );
    }
    // A kept value still sets.
    let _ = live
        .apply(rs_cam_core::session::Command::SetToolpathParam(
            rs_cam_core::session::SetToolpathParamArgs {
                index: 0,
                param: "clearing_strategy".to_owned(),
                value: serde_json::json!("geodesic_adaptive"),
            },
        ))
        .expect("a kept value sets");
}
