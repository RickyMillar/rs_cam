//! G-DROPINDEX (a) — a cross-setup move lands where it was dropped.
//!
//! `ToolpathPanel` computes the drop position from the pointer
//! (`compute_drop_index`), the event carries it and the controller passed it
//! to `ProjectSession::move_toolpath_to_setup` as `_idx`, which threw it away
//! and pushed the op onto the end of the target setup. So every cross-setup
//! drag landed at the bottom whatever the operator pointed at, and re-ordering
//! it afterwards took a second gesture.
//!
//! The position is an insertion GAP in the target setup's plan order, and
//! `None` still appends — MCP `move_toolpath_to_setup` has no position
//! argument and must keep the behaviour it shipped with.
//!
//! Source: `planning/ui_review_2026-09-09/results/W03/support/r05_source_track.md`
//! §6 ("the `drop_idx` computed by the panel is passed as `_idx` and ignored")
//! and its live question 9.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
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

/// Setup 0 holds one op; setup 1 holds three. Returns the session and the
/// global index of the op in setup 0.
fn two_setups() -> (ProjectSession, usize) {
    let mut s = ProjectSession::new_empty();
    s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let tool_id = s.tools()[0].id.0;
    s.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

    let travelling = s.add_toolpath(0, toolpath("Travelling", tool_id)).unwrap();
    for name in ["Target A", "Target B", "Target C"] {
        s.add_toolpath(1, toolpath(name, tool_id)).unwrap();
    }
    (s, travelling)
}

fn names_in_setup(s: &ProjectSession, setup: usize) -> Vec<String> {
    s.list_setups()[setup]
        .toolpath_indices
        .iter()
        .map(|&i| s.toolpath_configs()[i].name.clone())
        .collect()
}

/// THE sentry. Dropped on the gap above "Target A", the op lands first — not
/// last. Pre-fix this reads `["Target A", "Target B", "Target C", "Travelling"]`
/// for every position 0..=3.
#[test]
fn a_cross_setup_drop_lands_at_the_dropped_position() {
    for (position, expected) in [
        (0usize, ["Travelling", "Target A", "Target B", "Target C"]),
        (1, ["Target A", "Travelling", "Target B", "Target C"]),
        (2, ["Target A", "Target B", "Travelling", "Target C"]),
        (3, ["Target A", "Target B", "Target C", "Travelling"]),
    ] {
        let (mut s, travelling) = two_setups();
        let _ = s
            .move_toolpath_to_setup(travelling, 1, Some(position))
            .unwrap();

        assert!(
            s.list_setups()[0].toolpath_indices.is_empty(),
            "position {position}: the op must leave its old setup"
        );
        assert_eq!(
            names_in_setup(&s, 1),
            expected,
            "position {position}: a drop is an insert at that position"
        );
    }
}

/// A gap read off a stale or shorter list is a landing at the end, never a
/// panic and never a refusal.
#[test]
fn a_position_past_the_end_clamps_to_the_end() {
    let (mut s, travelling) = two_setups();
    let _ = s.move_toolpath_to_setup(travelling, 1, Some(99)).unwrap();
    assert_eq!(
        names_in_setup(&s, 1),
        ["Target A", "Target B", "Target C", "Travelling"]
    );
}

/// `None` is the MCP path: no position argument, so append — byte-identical
/// to the pre-fix behaviour on that surface.
#[test]
fn no_position_still_appends() {
    let (mut s, travelling) = two_setups();
    let _ = s.move_toolpath_to_setup(travelling, 1, None).unwrap();
    assert_eq!(
        names_in_setup(&s, 1),
        ["Target A", "Target B", "Target C", "Travelling"]
    );
}
