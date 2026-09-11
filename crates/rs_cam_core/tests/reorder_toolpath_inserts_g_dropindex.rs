//! G-DROPINDEX (b) — a same-setup re-order is an INSERT, not a swap.
//!
//! `ProjectSession::reorder_toolpath` used to `Vec::swap` the two positions in
//! the owning setup's `toolpath_indices`. Dragging the last op to the top
//! therefore also sent the top op to the bottom: two ops moved when the
//! operator asked for one, and the plan order between them was untouched.
//!
//! **The ruling on intent.** Three affordances reach this one method — drag
//! and drop, Move Up / Move Down, and the card context menu. Move Up / Down
//! always pass the ADJACENT op's index, and for adjacent positions an insert
//! and a swap are the same permutation, so those two menu items behave
//! identically before and after this change. Drag and drop passes the
//! insertion gap the pointer was over (`compute_drop_index` returns `0..=len`,
//! a gap, not a card), which only an insert can honour. So the two gestures
//! were never two policies in conflict; they are one meaning — "put this op
//! here" — and only the drag could tell the difference. That is why this is a
//! change to `reorder_toolpath` itself rather than a second method beside it.
//!
//! Source: `planning/ui_review_2026-09-09/results/W03/support/r05_source_track.md`
//! §6 ("`reorder_toolpath` swaps the two `toolpath_indices` entries") and §9.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSession, ToolpathConfig};

fn toolpath(name: &str, tool_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: rs_cam_core::compute::config::StockSource::Fresh,
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// One setup holding A B C D E, each op's global index equal to its position.
fn five_ops() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let tool_id = s.tools()[0].id.0;
    for name in ["A", "B", "C", "D", "E"] {
        let _ = s.add_toolpath(0, toolpath(name, tool_id)).unwrap();
    }
    s
}

fn plan_order(s: &ProjectSession) -> Vec<String> {
    s.list_setups()[0]
        .toolpath_indices
        .iter()
        .map(|&i| s.toolpath_configs()[i].name.clone())
        .collect()
}

/// THE sentry. Moving E onto A's place carries E to the front and shifts
/// A B C D down one. Under the swap this read `E B C D A` — two ops moved.
#[test]
fn a_long_move_up_inserts_and_shifts_the_rest() {
    let mut s = five_ops();
    let _ = s.reorder_toolpath(4, 0).unwrap();
    assert_eq!(plan_order(&s), ["E", "A", "B", "C", "D"]);
}

/// The same downward. A onto E's place puts A last; under the swap this read
/// `E B C D A`, which is the same permutation the upward drag produced —
/// the operator could not express the difference at all.
#[test]
fn a_long_move_down_inserts_and_shifts_the_rest() {
    let mut s = five_ops();
    let _ = s.reorder_toolpath(0, 4).unwrap();
    assert_eq!(plan_order(&s), ["B", "C", "D", "E", "A"]);
}

/// A mid-list move keeps every op that is not between the two ends where it
/// was. C onto B's place is a one-rank promotion, not a C/B exchange of ranks
/// — here the two agree, and that is the point of the next test.
#[test]
fn a_mid_list_move_touches_only_the_span_between_the_ends() {
    let mut s = five_ops();
    let _ = s.reorder_toolpath(3, 1).unwrap();
    assert_eq!(plan_order(&s), ["A", "D", "B", "C", "E"]);
}

/// THE compatibility half of the ruling: for ADJACENT positions — the only
/// ones Move Up / Move Down ever pass — an insert and a swap are the same
/// permutation. These two menu items are unchanged by this fix, which is why
/// it did not need a second entry point for them.
#[test]
fn adjacent_moves_match_the_swap_they_replaced() {
    for (from, to, expected) in [
        (0usize, 1usize, ["B", "A", "C", "D", "E"]),
        (1, 0, ["B", "A", "C", "D", "E"]),
        (2, 3, ["A", "B", "D", "C", "E"]),
        (4, 3, ["A", "B", "C", "E", "D"]),
    ] {
        let mut s = five_ops();
        let _ = s.reorder_toolpath(from, to).unwrap();
        assert_eq!(
            plan_order(&s),
            expected,
            "reorder({from}, {to}) must stay the swap it used to be"
        );
    }
}

/// Global indices and the results cache are keyed independently of plan
/// order, so a re-order must not renumber anything: only the setup's
/// `toolpath_indices` permutation moves.
#[test]
fn a_reorder_does_not_renumber_the_configs() {
    let mut s = five_ops();
    let before: Vec<String> = s
        .toolpath_configs()
        .iter()
        .map(|tc| tc.name.clone())
        .collect();
    let _ = s.reorder_toolpath(4, 0).unwrap();
    let after: Vec<String> = s
        .toolpath_configs()
        .iter()
        .map(|tc| tc.name.clone())
        .collect();
    assert_eq!(before, after, "config storage order is a stable key");
}
