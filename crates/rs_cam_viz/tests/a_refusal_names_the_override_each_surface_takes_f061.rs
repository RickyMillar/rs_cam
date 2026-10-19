//! F061 (tech-debt wave W5): a load refusal names its override the way
//! each surface spells it.
//!
//! The refusal text used to say "Pass `accept_exceeded=true`", the core
//! field name, which no surface accepts. Core now prints the spellings in
//! `rs_cam_core::gcode::LoadOverride`. This file checks that each spelling
//! is the one its surface really takes:
//!
//! - the MCP `export_gcode` key deserialises into `ExportParam`;
//! - the GUI pre-flight checkbox label starts with `gui_label()`.
//!
//! The CLI flags are named from `cli_flag()` at the clap attribute, so the
//! compiler holds that one.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::gcode::LoadOverride;

const BOTH: [LoadOverride; 2] = [LoadOverride::AcceptExceeded, LoadOverride::AcceptUnmodeled];

#[cfg(feature = "mcp")]
#[test]
fn the_mcp_key_is_the_export_param_field() {
    for kind in BOTH {
        let json = format!(r#"{{"path": "out.nc", "{}": true}}"#, kind.mcp_key());
        let param: rs_cam_mcp::server::ExportParam =
            serde_json::from_str(&json).expect("the key parses");
        let set = match kind {
            LoadOverride::AcceptExceeded => param.accept_exceeded_tool_load,
            LoadOverride::AcceptUnmodeled => param.accept_unmodeled_tool_load,
        };
        assert!(set, "`{}` sets its own override", kind.mcp_key());
    }
}

#[test]
fn the_gui_label_is_the_preflight_checkbox() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/preflight.rs"))
            .expect("read the pre-flight source");
    // Non-vacuity: the checkboxes are in this file.
    assert!(source.contains("ui.checkbox("), "the anchor moved");
    for kind in BOTH {
        let needle = format!("\"{}", kind.gui_label());
        assert!(
            source
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .any(|l| l.contains(&needle)),
            "no pre-flight label starts with {needle:?}"
        );
    }
}
