//! N1 — a disabled toolpath may retain its generated result for re-enable,
//! but export must omit its motion and phase metadata.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, stock_under, toolpath_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::gcode::{ToolLoadExportPolicy, export_gcode_checked};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use std::sync::atomic::AtomicBool;

const DISABLED_LABEL: &str = "N1 disabled trace";
const ENABLED_LABEL: &str = "N1 enabled trace";
const DISABLED_MARKER: &str = "N1_DISABLED_PRE";
const ENABLED_MARKER: &str = "N1_ENABLED_PRE";

fn square_at(x: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x, 0.0),
        P2::new(x + 2.0, 0.0),
        P2::new(x + 2.0, 2.0),
        P2::new(x, 2.0),
    ])
}

fn trace_op(spindle_rpm: u32) -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: 1.0,
        depth_per_pass: 1.0,
        spindle_rpm: Some(spindle_rpm),
        ..TraceConfig::default()
    })
}

#[test]
fn disabled_cached_toolpath_is_absent_from_checked_export() {
    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(stock_under(20.0, 4.0));

    let mut disabled_tool = make_endmill_6mm();
    disabled_tool.name = "N1 Disabled Tool".to_owned();
    disabled_tool.tool_number = 17;
    let disabled_tool_idx = session
        .add_tool(disabled_tool)
        .created
        .expect("add_tool reports the new tool index");
    let disabled_tool_id = session.tools()[disabled_tool_idx].id.0;

    let mut enabled_tool = make_endmill_6mm();
    enabled_tool.name = "N1 Enabled Tool".to_owned();
    enabled_tool.tool_number = 23;
    let enabled_tool_idx = session
        .add_tool(enabled_tool)
        .created
        .expect("add_tool reports the new tool index");
    let enabled_tool_id = session.tools()[enabled_tool_idx].id.0;

    let disabled_model_id = session
        .add_model(polygon_model(vec![square_at(1.0)], "n1_disabled"))
        .created
        .expect("add_model reports the new model id");
    let enabled_model_id = session
        .add_model(polygon_model(vec![square_at(11.0)], "n1_enabled"))
        .created
        .expect("add_model reports the new model id");

    let mut disabled = toolpath_config(
        DISABLED_LABEL,
        trace_op(11_111),
        disabled_tool_id,
        disabled_model_id,
    );
    disabled.pre_gcode = Some(DISABLED_MARKER.to_owned());
    disabled.post_gcode = Some("N1_DISABLED_POST".to_owned());
    let _ = session
        .add_toolpath(0, disabled)
        .expect("add disabled trace");

    let mut enabled = toolpath_config(
        ENABLED_LABEL,
        trace_op(22_222),
        enabled_tool_id,
        enabled_model_id,
    );
    enabled.pre_gcode = Some(ENABLED_MARKER.to_owned());
    enabled.post_gcode = Some("N1_ENABLED_POST".to_owned());
    let _ = session.add_toolpath(0, enabled).expect("add enabled trace");

    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate disabled trace");
    session
        .generate_toolpath(1, &cancel)
        .expect("generate enabled trace");
    assert!(
        !session
            .get_result(0)
            .expect("disabled trace result exists")
            .toolpath()
            .moves
            .is_empty(),
        "disabled fixture must generate motion"
    );
    assert!(
        !session
            .get_result(1)
            .expect("enabled trace result exists")
            .toolpath()
            .moves
            .is_empty(),
        "enabled fixture must generate motion"
    );

    let _ = session
        .set_toolpath_enabled(0, false)
        .expect("disable first trace");
    assert!(
        session.get_result(0).is_some(),
        "disabled trace cache is intentionally retained for re-enable"
    );

    let gcode = export_gcode_checked(
        &session,
        None,
        ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        },
    )
    .expect("export with load gates explicitly accepted");

    assert!(
        gcode.contains(ENABLED_LABEL),
        "enabled phase label is present"
    );
    assert!(gcode.contains("N1 Enabled Tool"), "enabled tool is present");
    assert!(
        gcode.contains(ENABLED_MARKER),
        "enabled pre-gcode is present"
    );
    assert!(gcode.contains("S22222"), "enabled spindle RPM is present");
    assert!(gcode.contains("X31.000"), "enabled trace motion is present");
    assert!(!gcode.contains(DISABLED_LABEL));
    assert!(!gcode.contains("N1 Disabled Tool"));
    assert!(!gcode.contains(DISABLED_MARKER));
    assert!(!gcode.contains("N1_DISABLED_POST"));
    assert!(!gcode.contains("S11111"));
    assert!(!gcode.contains("X21.000"));

    let _ = session
        .set_toolpath_enabled(0, true)
        .expect("re-enable first trace");
    assert!(
        session
            .get_toolpath_config(0)
            .expect("first trace config exists")
            .enabled,
        "re-enabling restores the trace config enabled state"
    );
    assert!(
        session.get_result(0).is_some(),
        "re-enabling retains the cached trace result"
    );

    let gcode = export_gcode_checked(
        &session,
        None,
        ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        },
    )
    .expect("export re-enabled cached trace with load gates explicitly accepted");

    assert!(
        gcode.contains(DISABLED_LABEL),
        "disabled phase label is restored"
    );
    assert!(
        gcode.contains("N1 Disabled Tool"),
        "disabled tool is restored"
    );
    assert!(
        gcode.contains(DISABLED_MARKER),
        "disabled pre-gcode is restored"
    );
    assert!(
        gcode.contains("N1_DISABLED_POST"),
        "disabled post-gcode is restored"
    );
    assert!(gcode.contains("S11111"), "disabled spindle RPM is restored");
    assert!(
        gcode.contains("X21.000"),
        "disabled trace motion is restored"
    );
}
