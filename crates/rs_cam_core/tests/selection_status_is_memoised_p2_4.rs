//! Geometry selection P2.4: the `selection_status` report is built once
//! per change of what it reads.
//!
//! # The claim
//!
//! The GUI asks for the report on every frame (the drill panel, the
//! viewport markers, the validator). The memo
//! (`session/selection_status.rs`) keys each report on what the selection
//! reads, by value or by allocation identity:
//!
//! - a second read with no edit builds nothing;
//! - an edit that the selection does not read (the feed rate, the tool)
//!   builds nothing;
//! - each input builds again: a selection edit, a model re-import, a setup
//!   change, a stock change. After each, the report answers the new state.
//!
//! The count comes from `ProjectSession::selection_status_builds`. The
//! `selection_status` query row reads the same memo.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test selection_status_is_memoised_p2_4
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;

use common::session::{polygon_model, single_op_session, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{
    Filter, Item, ItemKind, ItemSet, Rule, Selection, SelectionEdit, Shape, SourceRef,
};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, EditSelectionArgs, LoadedModel, ProjectSession, Query,
    QueryAnswer, SelectionReport, SelectionStatusArgs, SetSetupFaceArgs, SetStockConfigArgs,
    SetToolpathParamArgs,
};

const HALF: f64 = 6.0;
const STOCK_HALF: f64 = 40.0;
const STOCK_HEIGHT: f64 = 10.0;
const TOOL_DIAMETER: f64 = 3.0;

fn loop_item(layer: &str, cx: f64) -> Item {
    let ring = Polygon2::new(vec![
        P2::new(cx - HALF, -HALF),
        P2::new(cx + HALF, -HALF),
        P2::new(cx + HALF, HALF),
        P2::new(cx - HALF, HALF),
    ]);
    Item::new(
        SourceRef::ToolpathModel,
        Some(layer.into()),
        Shape::Loop(ring),
    )
    .unwrap()
}

fn model(loops: usize) -> LoadedModel {
    let mut m = polygon_model(Vec::new(), "p2_4_memo");
    let items = (0..loops)
        .map(|i| loop_item(if i == 0 { "A" } else { "B" }, 20.0 * i as f64))
        .collect();
    m.items = Arc::new(ItemSet::new(items));
    m
}

fn session() -> ProjectSession {
    single_op_session(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_DIAMETER),
        model(2),
        "memo pocket",
        OperationConfig::Pocket(PocketConfig::default()),
    )
}

/// The report of toolpath 0 through the query door.
fn status(s: &ProjectSession) -> SelectionReport {
    match s
        .query(Query::SelectionStatus(SelectionStatusArgs {
            index: 0,
            target: rs_cam_core::session::SelectionTarget::Op,
        }))
        .unwrap()
    {
        QueryAnswer::SelectionStatus(report) => report,
        other => panic!("the selection_status query answered {other:?}"),
    }
}

fn loops(report: &SelectionReport) -> usize {
    report.counts.get(&ItemKind::Loop).copied().unwrap_or(0)
}

/// Read the status, and return how many builds that read cost.
fn builds_of_a_read(s: &ProjectSession) -> (SelectionReport, u64) {
    let before = s.selection_status_builds();
    let report = status(s);
    (report, s.selection_status_builds() - before)
}

#[test]
fn a_read_builds_once_and_only_a_selection_input_builds_again() {
    let mut s = session();

    let (first, built) = builds_of_a_read(&s);
    assert_eq!(built, 1, "the first read must build");
    assert_eq!(loops(&first), 2);
    let (_, built) = builds_of_a_read(&s);
    assert_eq!(built, 0, "a second read with no edit built again");

    // An edit that the selection does not read.
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(1234.0),
        }))
        .unwrap();
    let (_, built) = builds_of_a_read(&s);
    assert_eq!(built, 0, "a feed edit built the selection status again");

    // A selection edit.
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: rs_cam_core::session::SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::from_rules(vec![Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter {
                    layers: vec!["A".to_owned()],
                    ..Filter::default()
                },
            }]))),
        }))
        .unwrap();
    let (after_edit, built) = builds_of_a_read(&s);
    assert_eq!(built, 1, "a selection edit did not build again");
    assert_eq!(loops(&after_edit), 1, "the report does not follow the edit");

    // A model re-import: a third loop on layer A.
    let model_id = s.toolpath_configs()[0].model_id;
    let mut fresh = model(2);
    let mut items: Vec<Item> = fresh.items.iter().cloned().collect();
    items.push(loop_item("A", -20.0));
    fresh.items = Arc::new(ItemSet::new(items));
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(fresh),
            units: None,
        }))
        .unwrap();
    let (after_import, built) = builds_of_a_read(&s);
    assert_eq!(built, 1, "a model re-import did not build again");
    assert_eq!(
        loops(&after_import),
        2,
        "the report does not follow the re-import"
    );

    // A setup change.
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .unwrap();
    let (_, built) = builds_of_a_read(&s);
    assert_eq!(built, 1, "a setup change did not build again");

    // A stock change.
    let mut stock = s.stock_config().clone();
    stock.x += 10.0;
    let _ = s
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .unwrap();
    let (_, built) = builds_of_a_read(&s);
    assert_eq!(built, 1, "a stock change did not build again");

    let (_, built) = builds_of_a_read(&s);
    assert_eq!(built, 0, "a read after the edits built again");
}
