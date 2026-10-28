//! W1b (operator ruling 2026-10-28): an old project's Waterline loads helical.
//!
//! Pre-registration:
//! `planning/adaptive_research_2026-10-23/integration/W1b_PREREG.md`. Before
//! D69 the Waterline dial was the key `continuous`, and no generator read
//! it, so every stored value was the old default. W1b renames the key to
//! `helical` (default `true`). The loader reads an old `continuous` key as
//! the old default: the op loads helical, and one
//! `ProjectLoadWarning::WaterlineHelical` names the change.
//!
//! 1. An old file (`continuous = false`, no `helical`) loads helical, with
//!    one warning.
//! 2. A new file with `helical = false` loads `false`, with no warning.
//! 3. Save, load, save keeps `helical = false` and `helical = true`, and
//!    the writer never writes `continuous`.
//! 4. `set_toolpath_param` takes `continuous` as an alias of `helical` on a
//!    Waterline, for one release (lead condition 2026-10-28).
//!
//! The warning is ONE short line (the operator's rule against text
//! clutter).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::WaterlineConfig;
use rs_cam_core::session::{
    Command, ProjectLoadWarning, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs,
};

/// A directory of its own for each call: the tests run in parallel.
fn temp_path() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push(format!("rs_cam_w1b_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// A session with one Waterline at `helical`.
fn session_with(helical: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let tc = toolpath_config(
        "Waterline",
        OperationConfig::Waterline(WaterlineConfig {
            helical,
            ..WaterlineConfig::default()
        }),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        rs_cam_core::ids::ModelId::from_raw(0),
    );
    let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    builder.build()
}

fn saved_text(session: &ProjectSession) -> String {
    let path = temp_path();
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    text
}

/// Load `text` through the file door; the Waterline's dial and the W1b
/// warnings.
fn load(text: &str) -> (bool, Vec<ProjectLoadWarning>, ProjectSession) {
    let path = temp_path();
    std::fs::write(&path, text).unwrap();
    let (session, warnings) = ProjectSession::load_with_warnings(&path).expect("the file loads");
    let _ = std::fs::remove_file(&path);
    let OperationConfig::Waterline(cfg) = &session.toolpath_configs()[0].operation else {
        panic!("toolpath 0 is the Waterline");
    };
    let helical = cfg.helical;
    let w1b = warnings
        .into_iter()
        .filter(|w| matches!(w, ProjectLoadWarning::WaterlineHelical { .. }))
        .collect();
    (helical, w1b, session)
}

/// The Waterline params table of a saved text.
fn params(text: &str) -> toml::Value {
    let raw: toml::Value = toml::from_str(text).unwrap();
    raw["setups"][0]["toolpaths"][0]["operation"]["params"].clone()
}

#[test]
fn an_old_file_loads_helical_with_one_warning() {
    let text = saved_text(&session_with(false));
    // The file as a build before W1b wrote it: `continuous`, no `helical`.
    let mut raw: toml::Value = toml::from_str(&text).unwrap();
    let table = raw["setups"][0]["toolpaths"][0]["operation"]["params"]
        .as_table_mut()
        .unwrap();
    assert!(
        table.remove("helical").is_some(),
        "the writer writes helical"
    );
    table.insert("continuous".to_owned(), toml::Value::Boolean(false));
    let old = toml::to_string(&raw).unwrap();

    let (helical, warnings, session) = load(&old);
    assert!(helical, "an old Waterline loads helical");
    assert_eq!(
        warnings,
        vec![ProjectLoadWarning::WaterlineHelical {
            toolpath: "Waterline".to_owned(),
            stored: false,
        }],
        "one warning names the change"
    );
    let line = warnings[0].message();
    assert!(
        !line.contains('\n')
            && line == "Waterline 'Waterline': helical descent is now on (was continuous = false).",
        "one short line: {line}"
    );
    // The save writes the new key only.
    let resaved = params(&saved_text(&session));
    assert_eq!(
        resaved.get("helical").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(resaved.get("continuous").is_none(), "no old key is written");
}

#[test]
fn a_new_file_keeps_an_explicit_false() {
    let text = saved_text(&session_with(false));
    assert_eq!(
        params(&text).get("helical").and_then(toml::Value::as_bool),
        Some(false)
    );
    let (helical, warnings, _) = load(&text);
    assert!(!helical, "an explicit helical = false stays stacked");
    assert!(
        warnings.is_empty(),
        "no migration on a new file: {warnings:?}"
    );
}

#[test]
fn a_round_trip_keeps_the_value() {
    for value in [false, true] {
        let first = saved_text(&session_with(value));
        let (helical, warnings, session) = load(&first);
        assert_eq!(helical, value);
        assert!(warnings.is_empty());
        let second = saved_text(&session);
        assert_eq!(params(&first), params(&second), "helical = {value}");
        assert!(params(&second).get("continuous").is_none());
    }
}

#[test]
fn the_param_door_takes_the_old_name_as_an_alias() {
    let mut session = session_with(true);
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "continuous".to_owned(),
            value: serde_json::Value::Bool(false),
        }))
        .expect("the alias sets the dial");
    let OperationConfig::Waterline(cfg) = &session.toolpath_configs()[0].operation else {
        panic!("toolpath 0 is the Waterline");
    };
    assert!(!cfg.helical, "continuous = false sets helical = false");
    assert!(
        cfg.legacy_continuous.is_none(),
        "the alias writes the new field"
    );
}
