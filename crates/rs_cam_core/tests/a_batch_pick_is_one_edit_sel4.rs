//! SEL-4 (design E §1): a batch pick is one edit.
//!
//! # The claim
//!
//! `SelectionEdit::AddPicks` and `RemovePicks` go through the one
//! `EditSelection` door:
//!
//! 1. A batch of two keys on the op default replaces the default with the
//!    two picks (O-PICK, once per batch).
//! 2. On a selection that holds a rule, the picks join the rule.
//! 3. `RemovePicks` of rule members turns each rule that holds any of them
//!    into picks of its members less ALL the keys (GT-Q8), with no exclude.
//! 4. A batch with one key that names no item refuses `PickNamesNoItem`,
//!    and the stored selection does not change (rule 4: no silent
//!    fallback). A single `AddPick` of such a key refuses too (the break).
//! 5. An empty list refuses `NoKey`.
//! 6. A key that the list holds twice is stored once (P2.7).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_batch_pick_is_one_edit_sel4
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::Arc;

use common::session::{polygon_model, single_op_session_with, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{
    Filter, Item, ItemKind, ItemSet, KindFilter, MatchKey, RefusalCause, Rule, Selection,
    SelectionEdit, Shape, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSession, SelectionTarget, SessionError,
};

/// The half side of each square loop (mm).
const HALF: f64 = 4.0;

fn loop_item(layer: &str, cx: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some(layer.into()),
        Shape::Loop(Polygon2::new(vec![
            P2::new(cx - HALF, -HALF),
            P2::new(cx + HALF, -HALF),
            P2::new(cx + HALF, HALF),
            P2::new(cx - HALF, HALF),
        ])),
    )
    .unwrap()
}

/// Loops a, b on layer CUT and c, d on layer OTHER; one Pocket at its
/// default selection.
fn session() -> (ProjectSession, [MatchKey; 4]) {
    let items = vec![
        loop_item("CUT", -30.0),
        loop_item("CUT", -10.0),
        loop_item("OTHER", 10.0),
        loop_item("OTHER", 30.0),
    ];
    let keys = [0, 1, 2, 3].map(|i| MatchKey {
        source: SourceRef::ToolpathModel,
        ..items[i].key().clone()
    });
    let mut model = polygon_model(Vec::new(), "sel4_drawing");
    model.items = Arc::new(ItemSet::new(items));
    let s = single_op_session_with(
        stock_under(60.0, 10.0),
        endmill_tool_config(2.0),
        model,
        "sel4 pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        |_| {},
    );
    (s, keys)
}

fn edit(s: &mut ProjectSession, edit: SelectionEdit) -> Result<(), SessionError> {
    s.apply(Command::EditSelection(EditSelectionArgs {
        index: 0,
        target: SelectionTarget::Op,
        edit,
    }))
    .map(|_| ())
}

fn stored(s: &ProjectSession) -> Selection {
    s.toolpath_configs()[0]
        .operation
        .selection()
        .unwrap()
        .clone()
}

fn layer_rule(layer: &str) -> Rule {
    Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            kinds: KindFilter::Only(vec![ItemKind::Loop]),
            ..Filter::default()
        },
    }
}

fn holds(selection: &Selection, key: &MatchKey) -> bool {
    selection.picks().iter().any(|k| k.matches(key))
}

#[test]
fn a_batch_on_the_default_replaces_it_once_sel4() {
    let (mut s, [a, b, _, _]) = session();
    edit(&mut s, SelectionEdit::AddPicks(vec![a.clone(), b.clone()])).unwrap();
    let sel = stored(&s);
    assert!(sel.rules().is_empty(), "the default rule is gone: {sel:?}");
    assert_eq!(sel.picks().len(), 2);
    assert!(holds(&sel, &a) && holds(&sel, &b));
}

#[test]
fn a_batch_on_a_rule_joins_it_sel4() {
    let (mut s, [a, b, _, _]) = session();
    edit(
        &mut s,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![layer_rule("OTHER")]))),
    )
    .unwrap();
    edit(&mut s, SelectionEdit::AddPicks(vec![a, b])).unwrap();
    let sel = stored(&s);
    assert_eq!(sel.rules().len(), 1, "the rule stays: {sel:?}");
    assert_eq!(sel.picks().len(), 2);
}

#[test]
fn a_batch_remove_turns_each_rule_into_picks_once_gt_q8() {
    let (mut s, [a, b, c, d]) = session();
    edit(
        &mut s,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![
            layer_rule("CUT"),
            layer_rule("OTHER"),
        ]))),
    )
    .unwrap();
    edit(
        &mut s,
        SelectionEdit::RemovePicks(vec![a.clone(), c.clone()]),
    )
    .unwrap();
    let sel = stored(&s);
    assert!(sel.rules().is_empty(), "both rules held a key: {sel:?}");
    assert!(sel.excludes().is_empty(), "no exclude: {sel:?}");
    assert_eq!(sel.picks().len(), 2);
    assert!(holds(&sel, &b) && holds(&sel, &d));
    assert!(!holds(&sel, &a) && !holds(&sel, &c));
}

#[test]
fn a_batch_with_a_lost_key_writes_nothing_sel4() {
    let (mut s, [a, b, _, _]) = session();
    let lost = MatchKey {
        anchor: [500.0, 500.0],
        ..a.clone()
    };
    edit(&mut s, SelectionEdit::AddPicks(vec![a])).unwrap();
    let before = stored(&s);
    for bad in [
        SelectionEdit::AddPicks(vec![b, lost.clone()]),
        SelectionEdit::AddPick(lost.clone()),
    ] {
        let Err(SessionError::Selection(r)) = edit(&mut s, bad.clone()) else {
            panic!("{bad:?} did not refuse as a selection refusal");
        };
        let RefusalCause::PickNamesNoItem { key, matches } = &r.cause else {
            panic!("{bad:?} refused with {:?}", r.cause);
        };
        assert_eq!(*matches, 0);
        assert!(key.matches(&lost), "the refusal names the lost key");
        assert!(
            r.to_string().contains("list_model_items"),
            "the fix names the key source: {r}"
        );
        assert_eq!(stored(&s), before, "{bad:?} wrote the selection");
    }
}

#[test]
fn an_empty_batch_refuses_sel4() {
    let (mut s, _) = session();
    let before = stored(&s);
    for empty in [
        SelectionEdit::AddPicks(Vec::new()),
        SelectionEdit::RemovePicks(Vec::new()),
    ] {
        let Err(SessionError::Selection(r)) = edit(&mut s, empty.clone()) else {
            panic!("{empty:?} did not refuse");
        };
        assert_eq!(r.cause, RefusalCause::NoKey, "{empty:?}");
        assert_eq!(stored(&s), before);
    }
}

#[test]
fn a_key_listed_twice_is_stored_once_p2_7() {
    let (mut s, [a, b, _, _]) = session();
    edit(&mut s, SelectionEdit::AddPicks(vec![a.clone(), a.clone()])).unwrap();
    assert_eq!(stored(&s).picks().len(), 1, "on the default");
    edit(&mut s, SelectionEdit::AddPicks(vec![b.clone(), b, a])).unwrap();
    assert_eq!(stored(&s).picks().len(), 2, "on the picks");
}
