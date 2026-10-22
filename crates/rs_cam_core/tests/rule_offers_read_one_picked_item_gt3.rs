//! Geometry selection GT-3 (rule L-R4 of the Geometry tab design): the
//! "Rule…" offers read the facts of ONE picked item.
//!
//! # The claim
//!
//! A drawing holds 24 circles Ø 6 and one circle Ø 10 on layer HOLES. A
//! Drill picks one Ø 6 circle and the Ø 10 circle.
//!
//! 1. The offers of the Ø 6 pick are "All circles Ø 6.00 on this model"
//!    (24 matches) and "All circles on this model" (25 matches). Nothing
//!    offers an item that the operator did not pick.
//! 2. The first offer is ONE `Replace`: its rule, the Ø 10 pick, and no Ø 6
//!    pick (the rule holds it). Applied, the selection holds 25 items.
//! 3. After that, the same offer from another Ø 6 item writes nothing (a
//!    rule holds every match), and "All circles" replaces the Ø 6 rule and
//!    the Ø 10 pick (it holds the members of both).
//!
//! Red proof: drop the `already` test in `rule_offers` (every offer gets an
//! edit). Claim 3 fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test rule_offers_read_one_picked_item_gt3
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
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{
    Item, ItemSet, KindFilter, MatchKey, SelectionEdit, Shape, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSession, SelectionTarget, ToolpathConfig,
};

fn circle(x: f64, diameter: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some("HOLES".into()),
        Shape::circle(P2::new(x, 0.0), diameter),
    )
    .unwrap()
}

fn key(item: &Item, s: &ProjectSession) -> MatchKey {
    MatchKey {
        source: SourceRef::Model(s.toolpath_configs()[0].model_id),
        ..item.key().clone()
    }
}

fn edit(s: &mut ProjectSession, edit: SelectionEdit) {
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit,
        }))
        .unwrap();
}

fn tc(s: &ProjectSession) -> ToolpathConfig {
    s.toolpath_configs()[0].clone()
}

#[test]
fn rule_offers_read_one_picked_item_gt3() {
    let small: Vec<Item> = (0..24).map(|i| circle(f64::from(i) * 12.0, 6.0)).collect();
    let large = circle(400.0, 10.0);
    let mut items = small.clone();
    items.push(large.clone());
    let mut model = polygon_model(Vec::new(), "gt3_holes");
    model.items = Arc::new(ItemSet::new(items));
    let mut s = single_op_session_with(
        stock_under(500.0, 20.0),
        endmill_tool_config(6.0),
        model,
        "gt3 drill",
        OperationConfig::new_default(OperationType::Drill),
        |_| {},
    );
    let (small_key, large_key) = (key(&small[0], &s), key(&large, &s));
    edit(&mut s, SelectionEdit::AddPick(small_key));
    edit(&mut s, SelectionEdit::AddPick(large_key));
    assert_eq!(
        tc(&s).operation.selection().unwrap().picks().len(),
        2,
        "fixture: the first pick replaced the default (O-PICK)"
    );

    // 1. The offers of the Ø 6 pick.
    let offers = s
        .rule_offers(&tc(&s), SelectionTarget::Op, &key(&small[0], &s))
        .unwrap();
    let summary: Vec<(&str, usize)> = offers.iter().map(|o| (o.label.as_str(), o.count)).collect();
    assert_eq!(
        summary,
        [
            ("All circles \u{00D8} 6.00 on this model", 24),
            ("All circles on this model", 25),
        ]
    );

    // 2. One Replace: the rule, the Ø 10 pick, no Ø 6 pick.
    let Some(SelectionEdit::Replace(next)) = offers[0].edit.clone() else {
        panic!("the offer writes one Replace: {:?}", offers[0].edit);
    };
    assert_eq!(next.rules().len(), 1);
    assert_eq!(
        next.rules()[0].filter.kinds,
        KindFilter::Only(vec![rs_cam_core::selection::ItemKind::Circle])
    );
    assert_eq!(
        next.picks().len(),
        1,
        "the rule holds the Ø 6 pick: {next:?}"
    );
    assert!(
        next.picks()[0].matches(&key(&large, &s).for_toolpath_model(tc(&s).model_id)),
        "the Ø 10 pick stays"
    );
    edit(&mut s, SelectionEdit::Replace(next));
    let report = s.selection_status(&tc(&s), SelectionTarget::Op).unwrap();
    assert_eq!(report.selected.len(), 25);

    // 3. The same offer from another Ø 6 item writes nothing; "All
    // circles" replaces the rule and the pick.
    let offers = s
        .rule_offers(&tc(&s), SelectionTarget::Op, &key(&small[5], &s))
        .unwrap();
    assert_eq!(offers[0].count, 24);
    assert!(
        offers[0].edit.is_none(),
        "a rule of the selection holds every match: {:?}",
        offers[0].edit
    );
    let Some(SelectionEdit::Replace(all)) = offers[1].edit.clone() else {
        panic!("All circles writes one Replace");
    };
    assert_eq!(all.rules().len(), 1, "the Ø 6 rule goes: {all:?}");
    assert!(all.picks().is_empty(), "the Ø 10 pick goes: {all:?}");
    assert_eq!(all.rules()[0].filter.diameter, None);
}
