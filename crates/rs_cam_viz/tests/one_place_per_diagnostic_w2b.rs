//! W2b (toolpath inspector programme, 2026-10-30): one place per
//! diagnostic, and one run-state word for the GUI and MCP.
//!
//! # The claims
//!
//! 1. In each W0 bad state, on each tab of each op family, each diagnostic
//!    id paints at most once on the inspector screen. The count reads the
//!    placement (`properties::placed_findings`, THE rule the panel draws
//!    by) and the painted galleys: a finding paints its face only on an
//!    issue row; the Tool row, the field rows and the notes counter carry
//!    it on hover only. Before W2b one finding painted up to three times
//!    (the header line, the tab row and the per-tab footer, F3/F7).
//! 2. Every finding has exactly one place, and the face of each issue
//!    finding paints on every tab (the issue stack is above the tabs).
//! 3. A finding with a project or view anchor paints nothing in the
//!    inspector. The fixture: the stock thickness is not set, a project
//!    finding (`precondition.not_set`, `Anchor::Project`).
//! 4. GUI/MCP parity: the status word the header paints and the
//!    `run_state` of the MCP `list_toolpaths` reply name the same core
//!    state, and that state is the expected one for the W0 state. A rest
//!    op with no simulated stock reads `waiting` on both (the W1 gap: the
//!    GUI said "Waiting on upstream stock", MCP said `ready`).
//!
//! Red proof: draw an issue finding a second time (the old footer row):
//! claim 1 counts 2. Make the header word read the GUI freshness: claim 4
//! fails on the refused state, which core no longer calls failed after
//! an edit.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test one_place_per_diagnostic_w2b
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod inspector_bad_states;

use std::collections::BTreeMap;

use inspector_bad_states::{Family, Painted, State, fixture, fixture_with_stock_z, screen};
use rs_cam_core::ToolpathId;
use rs_cam_core::diagnostics::{Anchor, Diagnostic};
use rs_cam_viz::state::AppState;
use rs_cam_viz::ui::properties::{self, ToolpathTab};

/// The face of each finding of `list`, by id.
fn faces_by_id(list: &[Diagnostic]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in list {
        out.entry(d.id.as_str().to_owned())
            .or_default()
            .push(d.face.clone());
    }
    out
}

/// How many galleys of `painted` read one of `faces` exactly.
fn painted_count(painted: &[Painted], faces: &[String]) -> usize {
    painted.iter().filter(|p| faces.contains(&p.text)).count()
}

/// Every finding of the toolpath, in its one place.
fn every_finding(placed: &properties::PlacedFindings) -> Vec<Diagnostic> {
    placed
        .tool
        .iter()
        .chain(&placed.by_field)
        .chain(&placed.issues)
        .chain(&placed.notes)
        .chain(&placed.elsewhere)
        .cloned()
        .collect()
}

#[test]
fn each_diagnostic_paints_at_most_once_w2b() {
    let mut failures = Vec::new();
    let mut issue_faces_painted = 0_usize;
    let mut screens = 0_usize;
    for family in Family::ALL {
        for bad in State::ALL {
            if bad.skip_reason(family).is_some() {
                continue;
            }
            let (mut state, id) = fixture(family, bad);
            let placed = properties::placed_findings(id, &state).expect("the toolpath exists");
            let all = every_finding(&placed);
            let by_id = faces_by_id(&all);
            let issue_faces: Vec<String> = placed.issues.iter().map(|d| d.face.clone()).collect();
            for tab in ToolpathTab::ALL {
                let painted = screen(&mut state, id, *tab);
                screens += 1;
                for (finding, faces) in &by_id {
                    let n = painted_count(&painted, faces);
                    if n > 1 {
                        failures.push(format!(
                            "{} / {} / {tab:?}: `{finding}` paints {n} times ({faces:?})",
                            family.name(),
                            bad.name()
                        ));
                    }
                }
                // Claim 2: each issue face paints on every tab.
                for face in &issue_faces {
                    let n = painted_count(&painted, std::slice::from_ref(face));
                    issue_faces_painted += n;
                    if n == 0 {
                        failures.push(format!(
                            "{} / {} / {tab:?}: the issue {face:?} does not paint",
                            family.name(),
                            bad.name()
                        ));
                    }
                }
            }
        }
    }
    assert!(screens > 100, "non-vacuity: only {screens} screens");
    assert!(
        issue_faces_painted > 0,
        "non-vacuity: no bad state painted an issue row"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Claim 2 on the no-tool and wrong-tool states: the tool finding is on
/// the Tool row (state and hover), never an issue row or a note.
#[test]
fn a_tool_finding_sits_on_the_tool_row_w2b() {
    for (family, bad) in [
        (Family::Pocket, State::NoTool),
        (Family::Finish3d, State::NoTool),
        (Family::Finish3d, State::WrongTool),
    ] {
        let (state, id) = fixture(family, bad);
        let placed = properties::placed_findings(id, &state).expect("the toolpath exists");
        assert!(
            !placed.tool.is_empty(),
            "{} / {}: the Tool row holds the tool finding",
            family.name(),
            bad.name()
        );
        for d in placed
            .issues
            .iter()
            .chain(&placed.notes)
            .chain(&placed.by_field)
        {
            assert_ne!(
                d.anchor,
                Anchor::Tool,
                "{} / {}: a tool finding left the Tool row: {}",
                family.name(),
                bad.name(),
                d.id.as_str()
            );
        }
    }
}

/// Claim 3: a project finding paints nothing in the inspector.
#[test]
fn a_project_finding_paints_nothing_in_the_inspector_w2b() {
    let (mut state, id) = fixture_with_stock_z(Family::Pocket, State::Clean, None);
    let placed = properties::placed_findings(id, &state).expect("the toolpath exists");
    let project: Vec<&Diagnostic> = placed
        .elsewhere
        .iter()
        .filter(|d| matches!(d.anchor, Anchor::Project | Anchor::View))
        .collect();
    assert!(
        !project.is_empty(),
        "non-vacuity: the unset stock thickness is a project finding: {placed:?}"
    );
    for d in placed
        .tool
        .iter()
        .chain(&placed.by_field)
        .chain(&placed.issues)
        .chain(&placed.notes)
    {
        assert!(
            !matches!(d.anchor, Anchor::Project | Anchor::View),
            "a project or view finding has an inspector place: {}",
            d.id.as_str()
        );
    }
    let faces: Vec<String> = project.iter().map(|d| d.face.clone()).collect();
    for tab in ToolpathTab::ALL {
        let painted = screen(&mut state, id, *tab);
        assert_eq!(
            painted_count(&painted, &faces),
            0,
            "{tab:?}: a project finding paints in the inspector: {faces:?}"
        );
    }
}

/// The header word of each core run-state label.
fn header_word(label: &str) -> &'static str {
    match label {
        "disabled" => "Disabled",
        "not_ready" => "Not ready",
        "ready" => "Ready",
        "generating" => "Generating",
        "done" => "Done",
        "out_of_date" => "Out of date",
        "failed" => "Failed",
        "waiting" => "Waiting",
        other => panic!("no header word for the run state {other:?}"),
    }
}

/// The run state the MCP `list_toolpaths` reply gives for `id`.
#[cfg(feature = "mcp")]
fn mcp_run_state(state: &AppState, id: ToolpathId) -> String {
    let reply: serde_json::Value = serde_json::from_str(
        &rs_cam_viz::mcp_bridge::build_list_toolpaths_response(state),
    )
    .expect("the reply is JSON");
    let row = reply
        .as_array()
        .expect("a row list")
        .iter()
        .find(|row| row["id"] == serde_json::json!(id))
        .unwrap_or_else(|| panic!("no row for {id}: {reply}"));
    row["run_state"]["state"]
        .as_str()
        .unwrap_or_else(|| panic!("no run_state: {row}"))
        .to_owned()
}

/// Assert that the header paints `expected`'s word, and no other state
/// word, on the Geometry tab.
#[cfg(feature = "mcp")]
fn assert_header(state: &mut AppState, id: ToolpathId, expected: &str, what: &str) {
    let mcp = mcp_run_state(state, id);
    assert_eq!(mcp, expected, "{what}: MCP run_state");
    let painted = screen(state, id, ToolpathTab::Geometry);
    let words = [
        "Disabled",
        "Not ready",
        "Ready",
        "Generating",
        "Done",
        "Out of date",
        "Failed",
        "Waiting",
    ];
    let shown: Vec<&str> = painted
        .iter()
        .map(|p| p.text.as_str())
        .filter(|t| words.contains(t))
        .collect();
    assert_eq!(
        shown,
        vec![header_word(&mcp)],
        "{what}: the header word and MCP `{mcp}` disagree"
    );
}

/// Claim 4.
#[cfg(feature = "mcp")]
#[test]
fn the_header_word_is_the_mcp_run_state_w2b() {
    let expected = |bad: State| match bad {
        State::Clean | State::ReachOn | State::BoundaryOn => "ready",
        State::NoTool | State::FeedsUnset | State::WrongTool | State::FeedsUnsetRefused => {
            "not_ready"
        }
        State::Refused => "failed",
        State::StaleAfterEdit => "out_of_date",
    };
    let mut checked = 0_usize;
    for family in Family::ALL {
        for bad in State::ALL {
            if bad.skip_reason(family).is_some() {
                continue;
            }
            let (mut state, id) = fixture(family, bad);
            let what = format!("{} / {}", family.name(), bad.name());
            assert_header(&mut state, id, expected(bad), &what);
            checked += 1;
        }
    }
    assert!(checked >= 20, "non-vacuity: {checked} states");

    // The W1 gap: a rest op with no simulated stock waits, on both
    // surfaces.
    use rs_cam_core::compute::config::StockSource;
    use rs_cam_core::session::{Command, SetStockSourceArgs};
    let (mut state, id) = fixture(Family::Finish3d, State::Clean);
    let index = state
        .session()
        .find_toolpath_config_by_id(id)
        .map(|(index, _)| index)
        .expect("the toolpath exists");
    let _ = state
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: StockSource::FromRemainingStock,
        }))
        .expect("the stock source applies");
    assert_header(&mut state, id, "waiting", "3D Finish on remaining stock");
}
