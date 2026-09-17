//! WP4 — every MCP mutation travels in one `McpRequestKind::Core` arm,
//! and the describe step answers for every registry row.
//!
//! Before WP4 the dispatch in `app/mcp.rs` carried one arm and one
//! hand-written handler per mutation: twenty-nine arms, twenty-nine
//! replies, and two of them re-derived a stale set of their own through
//! `compute_stale_set` instead of reading the setter's `Effects` (N15
//! measured what that costs). The wire now carries the request, the GUI
//! thread converts it to a `Command`, `ProjectSession::apply` runs it,
//! and one describe step builds the reply.
//!
//! **NOT MEASURED here: the dispatch itself.** `RsCamApp` needs an
//! `eframe::CreationContext`, so no test can drive `handle_mcp_request`
//! — the same limitation `mcp_toasts_report_outcome_g_mcptoast.rs`
//! records. What IS measured: the typed link from a registry row to the
//! wrapper (a real `CoreRequest` per row, and its own `id()`), and that
//! the describe step names every row.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::session::{CommandId, CommandKind, Reach};
use rs_cam_viz::mcp_bridge::CoreRequest;

/// The describe step and the dispatch, read as text.
const COMMANDS_SRC: &str = include_str!("../src/app/mcp/commands.rs");
const MCP_SRC: &str = include_str!("../src/app/mcp.rs");

/// One request per row the wire reaches, built from each parameter
/// struct's own `Default`.
///
/// **SHL-02 (2026-09-18).** This was a hand-written list of all
/// twenty-nine variants — a fifth file to edit when adding one MCP
/// command, and the one an engineer forgets last, because forgetting it
/// makes this file's rules measure LESS rather than fail. It now reads
/// the `declare_core_requests!` table that generates the enum itself, so
/// a new row reaches every rule here on its own.
///
/// The non-vacuity guard is below: the row count is asserted against the
/// registry, never against a number typed here.
fn wire_requests() -> Vec<CoreRequest> {
    CoreRequest::all_defaults()
}

/// Every variant of the table answers the workspace question.
///
/// The column is data, so this reads it rather than proving it. What it
/// pins is the SHAPE: at least one row moves the view to each workspace
/// the MCP surface uses, and at least one row leaves it alone. A table
/// whose column collapsed to one answer would pass every other rule here
/// and quietly stop moving the operator's view.
#[test]
fn the_table_answers_the_workspace_question_for_every_row_shl02() {
    use rs_cam_viz::state::Workspace;

    let rows = CoreRequest::all_defaults();
    let answers: Vec<Option<Workspace>> = rows.iter().map(CoreRequest::workspace_after).collect();
    assert_eq!(
        answers.len(),
        rows.len(),
        "every row answers, because the column is not optional"
    );
    for expected in [Some(Workspace::Setup), Some(Workspace::Toolpaths), None] {
        assert!(
            answers.contains(&expected),
            "no table row answers {expected:?}; the workspace column has \
             collapsed and an MCP mutation no longer moves the view to the \
             surface it changes"
        );
    }
}

/// Every row the registry says MCP reaches travels in the wrapper, and
/// the wrapper names that row.
#[test]
fn every_mcp_reached_row_has_a_core_request() {
    let ids: Vec<CommandId> = wire_requests().iter().map(CoreRequest::id).collect();
    let mut reached = 0_usize;
    for id in CommandId::ALL {
        // `CoreRequest` is the MUTATION wrapper, so the rule reads the
        // `Command` rows alone. A `Query` row answers through the read
        // door and a `Job` row through its own three steps; neither
        // travels in this arm, and WP10's `GenerateToolpath` is the
        // first `Job` row the wire reaches.
        if id.kind() != CommandKind::Command {
            continue;
        }
        if !matches!(id.surfaces().mcp, Reach::Reached) {
            continue;
        }
        assert!(
            ids.contains(id),
            "the registry says MCP reaches {id:?} ({}), but no CoreRequest \
             variant reports that id — the wire cannot dispatch it through \
             the one Core arm",
            id.wire_name()
        );
        reached += 1;
    }
    assert!(
        reached >= 29,
        "only {reached} Command rows declare mcp: Reached; WP4 landed 29, \
         so this rule is measuring less than it was written for"
    );
    assert_eq!(
        ids.len(),
        reached,
        "the request table and the Reached rows must be the same set; the \
         table holds {} entries against {reached} rows",
        ids.len()
    );
}

/// No two requests claim one row.
#[test]
fn each_core_request_names_a_distinct_row() {
    let ids: Vec<CommandId> = wire_requests().iter().map(CoreRequest::id).collect();
    for (at, id) in ids.iter().enumerate() {
        assert!(
            !ids[..at].contains(id),
            "two CoreRequest variants both report {id:?}"
        );
    }
}

/// The describe step answers for EVERY registry row, including the rows
/// the wire cannot reach.
///
/// The match in `describe_core` is exhaustive, so this cannot fail while
/// the code compiles — which is the point. It fails loudly if someone
/// replaces the exhaustive match with a wildcard, and that wildcard is
/// exactly how a new row would ship with no reply of its own.
#[test]
fn the_describe_step_names_every_registry_row() {
    for id in CommandId::ALL {
        let needle = format!("CommandId::{id:?}");
        assert!(
            COMMANDS_SRC.contains(&needle),
            "the describe step names no arm for {needle}; a row with no arm \
             reaches the wire with no reply of its own"
        );
    }
    // A wildcard at the top level of `match id` would let a new row
    // inherit another row's reply instead of failing to compile. The
    // arms sit at twelve spaces; a nested match's arms sit deeper.
    let body = describe_core_body();
    let wildcard = body
        .lines()
        .find(|line| line.starts_with("            _ ") && !line.starts_with("             "));
    assert!(
        wildcard.is_none(),
        "describe_core carries a wildcard arm: {wildcard:?}"
    );
}

/// The body of `describe_core`, from its signature to the next function.
fn describe_core_body() -> &'static str {
    let at = COMMANDS_SRC
        .find("    pub(crate) fn describe_core(")
        .expect("describe_core is declared in app/mcp/commands.rs");
    let tail = &COMMANDS_SRC[at..];
    let end = tail[1..]
        .find("\n    fn ")
        .map_or(tail.len(), |offset| offset + 1);
    &tail[..end]
}

/// The dispatch holds ONE command arm, and that arm takes the one
/// mutation door.
#[test]
fn the_dispatch_holds_one_command_arm() {
    assert_eq!(
        MCP_SRC
            .matches("McpRequestKind::Core(request) => {")
            .count(),
        1,
        "app/mcp.rs must hold exactly one McpRequestKind::Core arm"
    );
    let at = MCP_SRC
        .find("McpRequestKind::Core(request) => {")
        .expect("the Core arm");
    let arm = &MCP_SRC[at..MCP_SRC.len().min(at + 2_000)];
    for needle in [
        "self.mcp_before_core(&request)",
        "self.core_command_for(request)",
        ".session.apply(command)",
        "self.describe_core(id, outcome, &before)",
        "push_mcp_outcome(",
    ] {
        assert!(
            arm.contains(needle),
            "the Core arm must call {needle}; it reads:\n{arm}"
        );
    }
}
