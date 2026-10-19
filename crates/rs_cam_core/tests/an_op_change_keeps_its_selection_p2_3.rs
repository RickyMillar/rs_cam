//! Geometry selection P2.3b: `SetToolpathOperation` carries the selection
//! across a change of the op kind (DESIGN 3.1).
//!
//! # The claim
//!
//! - Pocket with a layer rule, changed to Adaptive 2D: the Adaptive op
//!   holds the same rule, although the new config came with its default.
//!   `Effects::selection_dropped` is false.
//! - Pocket with a layer rule, changed to Drop Cutter (a 3D op, which holds
//!   no selection): the selection drops, and `Effects::selection_dropped`
//!   says so.
//! - Pocket with its default selection, changed to Drop Cutter: nothing
//!   that the user chose is lost, so `selection_dropped` is false.
//! - A replace with the same kind installs the new config as it is.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test an_op_change_keeps_its_selection_p2_3
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::session::toolpath_config;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{AdaptiveConfig, DropCutterConfig, PocketConfig};
use rs_cam_core::selection::{Filter, Rule, Selection, SourceRef};
use rs_cam_core::session::{
    Command, Effects, ProjectSession, ProjectSessionBuilder, SetToolpathOperationArgs,
};

fn layer_rule(layer: &str) -> Selection {
    Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            ..Filter::default()
        },
    }])
}

fn pocket_session(selection: Selection) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(endmill_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    let op = OperationConfig::Pocket(PocketConfig {
        selection,
        ..PocketConfig::default()
    });
    let _ = builder
        .add_toolpath(0, toolpath_config("pocket", op, tool_id, 0))
        .unwrap();
    builder.build()
}

fn change(session: &mut ProjectSession, operation: OperationConfig) -> Effects {
    session
        .apply(Command::SetToolpathOperation(SetToolpathOperationArgs {
            index: 0,
            operation: Box::new(operation),
        }))
        .unwrap()
}

#[test]
fn an_op_change_keeps_its_selection() {
    let mut s = pocket_session(layer_rule("CUT"));
    let effects = change(&mut s, OperationConfig::Adaptive(AdaptiveConfig::default()));
    let op = &s.toolpath_configs()[0].operation;
    assert!(matches!(op, OperationConfig::Adaptive(_)));
    assert_eq!(op.selection(), Some(&layer_rule("CUT")));
    assert!(!effects.selection_dropped);
}

#[test]
fn a_change_to_an_op_with_no_selection_reports_the_drop() {
    let mut s = pocket_session(layer_rule("CUT"));
    let effects = change(
        &mut s,
        OperationConfig::DropCutter(DropCutterConfig::default()),
    );
    assert!(s.toolpath_configs()[0].operation.selection().is_none());
    assert!(
        effects.selection_dropped,
        "a dropped layer rule must be reported"
    );

    let mut s = pocket_session(Selection::drawing());
    let effects = change(
        &mut s,
        OperationConfig::DropCutter(DropCutterConfig::default()),
    );
    assert!(
        !effects.selection_dropped,
        "a default selection holds no user choice"
    );
}

#[test]
fn a_replace_of_the_same_kind_installs_its_own_selection() {
    let mut s = pocket_session(layer_rule("CUT"));
    let effects = change(
        &mut s,
        OperationConfig::Pocket(PocketConfig {
            selection: layer_rule("OTHER"),
            ..PocketConfig::default()
        }),
    );
    assert_eq!(
        s.toolpath_configs()[0].operation.selection(),
        Some(&layer_rule("OTHER"))
    );
    assert!(!effects.selection_dropped);
}
