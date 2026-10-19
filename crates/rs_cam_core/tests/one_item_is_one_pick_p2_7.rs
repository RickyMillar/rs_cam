//! Geometry selection P2.7: one item is one pick, whatever surface picks it.
//!
//! # The claim
//!
//! An agent picks an item with the key that `list_model_items` gives
//! (source `SourceRef::Model(id)`). The viewport picks the same item with
//! the key of the drill picker (source `SourceRef::ToolpathModel`). Both
//! go through the `edit_toolpath_selection` command, and the viewport click
//! goes through `ProjectSession::pick_edit` (the controller's `TogglePick`).
//!
//! - After the agent pick, a viewport click on the item removes the pick:
//!   the selection holds no pick and no exclude.
//! - A second click picks it again: one pick, stored as `ToolpathModel`.
//! - The agent pick of the same item after that adds nothing.
//!
//! - DESIGN 9.2: the stored pick follows a re-target of the toolpath. On a
//!   model with no such circle it is lost and flagged. A pick on ANOTHER
//!   model keeps its `Model(id)` source and still resolves after the
//!   re-target.
//!
//! Before P2.7 the command stored each key as given. `MatchKey::matches`
//! compares the source, so the click's `RemovePick` matched no pick and
//! added an exclude beside the agent pick: the item stayed selected, and
//! the file held two spellings of one item.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test one_item_is_one_pick_p2_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;

use common::session::{TestHole, polygon_model, single_op_session, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{ItemKind, MatchKey, Selection, SelectionEdit, SourceRef};
use rs_cam_core::session::{
    AddModelArgs, Command, EditSelectionArgs, ListItemsArgs, ProjectSession, Query, QueryAnswer,
    SetToolpathModelArgs,
};

/// The two circles of the drawing (centre X, centre Y), diameter 6 mm.
const HOLES: [[f64; 2]; 2] = [[10.0, 10.0], [-10.0, -10.0]];
const HOLE_DIAMETER: f64 = 6.0;

fn session() -> ProjectSession {
    let plate = Polygon2::new(vec![
        P2::new(-20.0, -20.0),
        P2::new(20.0, -20.0),
        P2::new(20.0, 20.0),
        P2::new(-20.0, 20.0),
    ]);
    let holes: Vec<TestHole> = HOLES
        .iter()
        .map(|&[x, y]| TestHole {
            x,
            y,
            layer: "HOLES".to_owned(),
            diameter: Some(HOLE_DIAMETER),
        })
        .collect();
    let mut model = polygon_model(Vec::new(), "p2_7_one_pick");
    model.items = Arc::new(common::session::drawing_items(vec![plate], &holes));
    single_op_session(
        stock_under(25.0, 10.0),
        endmill_tool_config(3.0),
        model,
        "drill",
        OperationConfig::Drill(DrillConfig {
            selection: Selection::empty(),
            ..DrillConfig::default()
        }),
    )
}

/// The `list_model_items` key of the first circle: source `Model(id)`.
fn listed_key(s: &ProjectSession) -> MatchKey {
    let model_id = s.toolpath_configs()[0].model_id;
    let answer = s
        .query(Query::ListItems(ListItemsArgs {
            model_id,
            layer: None,
            kind: Some(ItemKind::Circle),
        }))
        .expect("the model exists");
    let QueryAnswer::ListItems(answer) = answer else {
        panic!("the list_model_items read answered {answer:?}");
    };
    let key = answer.items[0].key.clone();
    assert!(
        matches!(key.source, SourceRef::Model(_)),
        "the listed key names its model: {key:?}"
    );
    key
}

/// The viewport key of the same circle: the drill picker's key, source
/// `ToolpathModel`.
fn viewport_key(s: &ProjectSession) -> MatchKey {
    let tc = s.toolpath_configs()[0].clone();
    let picker = s.hole_selection(&tc).expect("a drill reads holes");
    let key = picker.items[0].key.clone();
    assert_eq!(key.source, SourceRef::ToolpathModel);
    key
}

fn edit(s: &mut ProjectSession, edit: SelectionEdit) {
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: rs_cam_core::session::SelectionTarget::Op,
            edit,
        }))
        .expect("the drill holds a selection");
}

/// The viewport click: `pick_edit` reads the memo and chooses the edit.
fn click(s: &mut ProjectSession, key: MatchKey) {
    let tc = s.toolpath_configs()[0].clone();
    let e = s
        .pick_edit(&tc, rs_cam_core::session::SelectionTarget::Op, key)
        .expect("the selection resolves");
    edit(s, e);
}

fn stored(s: &ProjectSession) -> Selection {
    s.toolpath_configs()[0]
        .operation
        .selection()
        .expect("a drill holds a selection")
        .clone()
}

#[test]
fn an_agent_pick_and_a_click_on_one_item_are_one_pick() {
    let mut s = session();
    let listed = listed_key(&s);
    let viewport = viewport_key(&s);
    assert!(
        listed.names_same_geometry(&viewport),
        "the two keys name the first circle"
    );

    // The agent picks the circle.
    edit(&mut s, SelectionEdit::AddPick(listed.clone()));
    let sel = stored(&s);
    assert_eq!(sel.picks().len(), 1);
    assert_eq!(
        sel.picks()[0].source,
        SourceRef::ToolpathModel,
        "a pick on the toolpath model is stored as ToolpathModel"
    );

    // A click on the picked circle removes the pick and adds no exclude.
    click(&mut s, viewport.clone());
    let sel = stored(&s);
    assert!(
        sel.picks().is_empty() && sel.excludes().is_empty(),
        "the click must remove the agent pick, not exclude beside it: {sel:?}"
    );

    // A second click picks it again.
    click(&mut s, viewport);
    let sel = stored(&s);
    assert_eq!(sel.picks().len(), 1, "{sel:?}");
    assert!(sel.excludes().is_empty(), "{sel:?}");

    // The agent pick of the same circle adds nothing.
    edit(&mut s, SelectionEdit::AddPick(listed));
    let sel = stored(&s);
    assert_eq!(sel.picks().len(), 1, "one item is one pick: {sel:?}");

    // The status counts one hole.
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
        .expect("the selection resolves");
    assert_eq!(report.counts.get(&ItemKind::Circle), Some(&1));
}

#[test]
fn an_agent_remove_of_a_clicked_pick_removes_it() {
    let mut s = session();
    let listed = listed_key(&s);
    let viewport = viewport_key(&s);
    click(&mut s, viewport);
    assert_eq!(stored(&s).picks().len(), 1);
    edit(&mut s, SelectionEdit::RemovePick(listed));
    let sel = stored(&s);
    assert!(
        sel.picks().is_empty() && sel.excludes().is_empty(),
        "the agent remove must remove the clicked pick: {sel:?}"
    );
}

/// Add a second model (id 1): the plate alone, with no circle.
fn add_plain_model(s: &mut ProjectSession) {
    let plate = Polygon2::new(vec![
        P2::new(-20.0, -20.0),
        P2::new(20.0, -20.0),
        P2::new(20.0, 20.0),
        P2::new(-20.0, 20.0),
    ]);
    let mut model = polygon_model(vec![plate], "p2_7_plain");
    model.id = 1;
    let _ = s
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("the model is added");
    assert!(s.models().iter().any(|m| m.id == 1), "the model keeps id 1");
}

fn retarget(s: &mut ProjectSession, model_id: usize) {
    let _ = s
        .apply(Command::SetToolpathModel(SetToolpathModelArgs {
            index: 0,
            model_id,
        }))
        .expect("the toolpath takes the model");
}

#[test]
fn a_pick_on_the_own_model_follows_a_retarget_and_flags() {
    let mut s = session();
    let listed = listed_key(&s);
    edit(&mut s, SelectionEdit::AddPick(listed));
    add_plain_model(&mut s);
    retarget(&mut s, 1);
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
        .expect("the selection resolves");
    assert_eq!(
        report.lost.len(),
        1,
        "the pick reads the new model, which has no such circle: {report:?}"
    );
    assert!(report.refusal.is_some(), "a lost pick refuses");
}

#[test]
fn a_pick_on_another_model_stays_bound_to_it() {
    let mut s = session();
    add_plain_model(&mut s);
    let listed = listed_key(&s);
    // The toolpath reads model 1; the pick names a circle of model 0.
    retarget(&mut s, 1);
    edit(&mut s, SelectionEdit::AddPick(listed));
    let sel = stored(&s);
    assert_eq!(sel.picks().len(), 1);
    assert!(
        matches!(sel.picks()[0].source, SourceRef::Model(_)),
        "a pick on another model keeps its model: {sel:?}"
    );
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
        .expect("the selection resolves");
    assert!(report.lost.is_empty(), "{report:?}");
    assert_eq!(report.counts.get(&ItemKind::Circle), Some(&1));
}
