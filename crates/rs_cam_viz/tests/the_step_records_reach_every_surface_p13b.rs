//! Feeds contract P13b (K8, ruling Q7): the GUI, MCP and the CLI show the
//! same step records for one project.
//!
//! The pinned rivmap100 project (core `tests/fixtures/rivmap100_rough_2026-11-04_7effc804`)
//! takes the envelope proposal on its first toolpath, then a typed feed (so
//! the chain ends with a `stated_by_operator` record). No toolpath is
//! generated, so every surface shows the stored lineage. The claims:
//!
//! - MCP `get_toolpath_lineage` (`state::freshness::toolpath_lineage_reply`)
//!   names the run state and returns the records.
//! - The CLI (`suggest` and `summary.json` print
//!   `ProjectSession::lineage_measured(index, ..).to_json()`) gives the same
//!   records as MCP (the CLI unit test
//!   `the_suggest_record_carries_the_stored_lineage_p13b` holds that the CLI
//!   prints that call).
//! - The GUI inspector (`toolpath_panel_snapshot`, `Lineage::stored`,
//!   `lineage_rows`) has one row per quantity, and its hover is the
//!   `step_line` of each MCP record of that quantity, in order.
//!
//! Red at HEAD 255d8db90: the test does not compile (`StepRecord`,
//! `toolpath_lineage_reply`, `step_line` do not exist).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::session::{ApplyFeedsArgs, Command, ProjectSession, SetToolpathParamArgs};
use rs_cam_core::trace::lineage::{
    Lineage, STEP_STATED_BY_OPERATOR, StepQuantity, StepRecord, records_of,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::freshness::toolpath_lineage_reply;
use rs_cam_viz::ui::properties::lineage::{lineage_rows, step_line};
use rs_cam_viz::ui::properties::toolpath_panel_snapshot;

fn session() -> ProjectSession {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml");
    let mut session = ProjectSession::load(&path).expect("the fixture loads");
    let _ = session
        .apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index: 0,
            pick: Some(PickRequest::Proposal),
            explored: None,
        }))
        .expect("the proposal applies");
    let feed = session.toolpath_configs()[0]
        .operation
        .feed_rate()
        .expect("a feed");
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!((feed * 0.9).round()),
        }))
        .expect("the feed edit");
    session
}

#[test]
fn the_step_records_reach_every_surface_p13b() {
    let mut state = AppState::new();
    state.replace_session(session());

    // MCP.
    let reply = toolpath_lineage_reply(&state, 0);
    assert!(
        reply
            .get("run_state")
            .is_some_and(serde_json::Value::is_string),
        "the reply names the run state: {reply}"
    );
    let mcp_json = reply
        .get("lineage")
        .and_then(|l| l.get("steps"))
        .cloned()
        .expect("the reply carries the steps");
    let mcp: Vec<StepRecord> =
        serde_json::from_value(mcp_json.clone()).expect("the MCP steps are step records");
    assert!(!mcp.is_empty(), "MCP shows no step record");
    assert!(
        mcp.iter().any(|r| r.step == STEP_STATED_BY_OPERATOR),
        "the typed feed ends its chain with a record on MCP"
    );

    // The CLI prints this core call.
    let cli_json = state
        .session()
        .lineage_measured(0, None)
        .map(|l| l.to_json())
        .and_then(|j| j.get("steps").cloned())
        .expect("the CLI lineage");
    assert_eq!(cli_json, mcp_json, "the CLI and MCP records differ");

    // The GUI inspector.
    let tc = &state.session().toolpath_configs()[0];
    let snapshot =
        toolpath_panel_snapshot(tc.id, state.session(), &state.gui).expect("the panel snapshot");
    let tool = state.session().toolpath_tool(tc).expect("a tool");
    let lineage = Lineage::stored(
        &snapshot.entry.operation,
        tool,
        &snapshot.entry.feeds_provenance,
    );
    assert_eq!(lineage.steps, mcp, "the GUI lineage and MCP records differ");
    let rows = lineage_rows(&lineage);
    let mut quantities = 0;
    for q in StepQuantity::ALL {
        let records: Vec<&StepRecord> = records_of(&mcp, q).collect();
        let name = format!("{} steps", q.label());
        let row = rows.iter().find(|r| r.name == name);
        if records.is_empty() {
            assert!(row.is_none(), "the GUI has a {name} row with no record");
            continue;
        }
        quantities += 1;
        let row = row.unwrap_or_else(|| panic!("the GUI has no {name} row"));
        let expected = records
            .iter()
            .map(|r| step_line(r))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(row.hover, expected, "the GUI {name} hover differs from MCP");
    }
    assert!(quantities > 0);
}
