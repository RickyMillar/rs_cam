//! P2.5 safety item — a toolpath that names a missing tool id refuses by
//! the id. It never uses another tool.
//!
//! # The defect
//!
//! `ProjectSession::find_tool_by_raw_id` ended in
//! `.or_else(|| self.tools.first())`. A toolpath whose tool id named no
//! tool (a project file edited by hand, a tool list that changed) then
//! generated with the FIRST tool of the project. A wrong tool is a wrong
//! cut: a 6 mm end mill path cut with a 1 mm bit, or the reverse. P2.4
//! removed the same fallback from `find_model_by_raw_id`.
//!
//! # The fixture
//!
//! A pocket on a 20 mm square drawing. The project holds two tools (ids 0
//! and 1). The toolpath names tool id 999.
//!
//! # What is asserted
//!
//! 1. Generation refuses, and the message names id 999. No result is
//!    stored.
//! 2. The toolpath list names the missing id in place of a tool name.
//! 3. Non-vacuity: the same toolpath on tool id 1 generates.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, square_polygon, stock_over, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};

const MISSING_TOOL_ID: usize = 999;

/// The session: two tools, a square drawing, one pocket on `tool_id`.
fn session_on(tool_id: Option<usize>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(30.0, 10.0));
    let first = builder.add_tool(endmill_tool_config(6.0));
    let second = builder.add_tool(endmill_tool_config(3.0));
    assert_ne!(first, second);
    let second_id = builder.tools()[second].id.0;
    let model_id = builder.add_model(polygon_model(vec![square_polygon(10.0)], "square"));
    let op = OperationConfig::new_default(OperationType::Pocket);
    let cfg = toolpath_config("Pocket", op, tool_id.unwrap_or(second_id), model_id);
    builder.add_toolpath(0, cfg).expect("add the toolpath");
    builder.build()
}

#[test]
fn a_missing_tool_id_refuses_p2_5() {
    let mut session = session_on(Some(MISSING_TOOL_ID));
    assert!(
        session.tools().iter().all(|t| t.id.0 != MISSING_TOOL_ID),
        "the fixture must not hold the missing id"
    );
    let cancel = AtomicBool::new(false);
    let err = session
        .generate_toolpath(0, &cancel)
        .map(|_| ())
        .expect_err("a toolpath on a missing tool must refuse, not take the first tool")
        .to_string();
    assert!(
        err.contains(&MISSING_TOOL_ID.to_string()),
        "the refusal must name the missing tool id: {err}"
    );
    assert!(
        session.get_result(0).is_none(),
        "a refused generation must store no result"
    );

    let listed = session.list_toolpaths();
    assert!(
        listed[0].tool_name.contains(&MISSING_TOOL_ID.to_string()),
        "the toolpath list must name the missing tool id, not another tool: {}",
        listed[0].tool_name
    );
}

/// Non-vacuity: the same toolpath on a tool that exists generates.
#[test]
fn the_same_toolpath_on_an_existing_tool_generates_p2_5() {
    let mut session = session_on(None);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the pocket on an existing tool must generate");
}
