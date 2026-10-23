//! Operator ruling O-PICK (2026-10-22): a pick replaces the default rule.
//! With it, operator ruling GT-Q8: to drop one item from a rule, the rule
//! turns into picks.
//!
//! # The defect
//!
//! Fixture A (`fixtures/selection_p4_4/states.dxf`), a Profile with the
//! default rule "this model, every layer, served kinds". The operator
//! picked one square. The pick ADDED to the default rule, so the Profile
//! also cut the Ø 4 circle and every other ring, which the operator never
//! picked ("because I didnt select the circle in the op. it would not have
//! been considered").
//!
//! # The claim
//!
//! The rule lives in `Selection::apply` (and the click door
//! `ProjectSession::pick_edit` sends `AddPick` while the selection is the
//! default), so every surface takes it:
//!
//! 1. On a default selection, the first pick (GUI click: `pick_edit`, then
//!    `EditSelection`) replaces the default: the selection is that one
//!    pick, the square is `Cut`, every other item (the circle too) is
//!    `NotSelected`, and generation cuts only near the square.
//! 2. A second click adds a pick.
//! 3. A click on each picked item removes it. The empty selection refuses
//!    as an empty selection does (`RefusalCause::Empty`) at the report
//!    and at generation.
//! 4. `Reset` brings the default rule back, with the item states of the
//!    start.
//! 5. The MCP route (`add_pick` sent as it is) gives the same selection and
//!    the same item states as the GUI route.
//! 6. A layer rule (shift-click, `layer_rule_edit`) on the default replaces
//!    it; a later click adds a pick beside the rule.
//! 7. GT-Q8: a click on an item that a rule holds turns the rule into
//!    picks of all its members less that item, in ONE command. The MCP
//!    `remove_pick` gives the same selection. On the default rule, the
//!    members are the items that the op cuts: the open path that the
//!    Profile leaves out does not become a pick.
//!
//! Red proofs (run, red, reverted):
//! - The `AddPick` replace arm of `Selection::apply` off: claim 1 failed
//!   (the selection kept the default rule beside the pick).
//! - The O-PICK early return of `pick_edit` off: claims 1 and 5 failed
//!   (the first click on a served square sent `RemovePick`, and GT-Q8 then
//!   turned the default rule into picks of every other item).
//! - The `rule_drop_edit` call of `edit_toolpath_selection` off: claim 7
//!   failed (the rule stayed, with an exclude).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_pick_replaces_the_default_rule_o_pick
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::single_op_session;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::selection::{
    Filter, ItemKind, MatchKey, RefusalCause, Rule, Selection, SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession, SelectionTarget,
    SessionError,
};
use rs_cam_core::toolpath::MoveType;

/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 3.0;
/// The SPARE square of the fixture: X 100..110, Y 0..10 (mm).
const SPARE_BOX: [f64; 4] = [100.0, 0.0, 110.0, 10.0];
/// How far a cutting move may lie outside the picked square (mm): the
/// tool diameter, which holds the tool radius of a Profile Outside and its
/// lead moves.
const CUT_MARGIN: f64 = TOOL_DIAMETER;

fn load() -> LoadedModel {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/selection_p4_4/states.dxf");
    rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads")
}

fn stock() -> StockConfig {
    StockConfig {
        x: 120.0,
        y: 90.0,
        z: Some(10.0),
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A Profile with its default config (the default rule) on fixture A.
fn session() -> ProjectSession {
    single_op_session(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        load(),
        OperationType::Profile.kind_str(),
        OperationConfig::new_default(OperationType::Profile),
    )
}

fn apply(s: &mut ProjectSession, edit: SelectionEdit) {
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit,
        }))
        .expect("the edit applies");
}

/// A viewport click on `key`: the GUI route (`pick_edit`, then one
/// `EditSelection`). Returns the edit that the click sent.
fn click(s: &mut ProjectSession, key: &MatchKey) -> SelectionEdit {
    let tc = s.toolpath_configs()[0].clone();
    let edit = s
        .pick_edit(&tc, SelectionTarget::Op, key.clone())
        .expect("the click reads the selection");
    apply(s, edit.clone());
    edit
}

/// The key of the item on `layer` of kind `kind` whose anchor is nearest
/// `at`.
fn key(s: &ProjectSession, layer: &str, kind: ItemKind, at: [f64; 2]) -> MatchKey {
    s.models()[0]
        .items
        .iter()
        .filter(|i| i.layer() == Some(layer) && i.kind() == kind)
        .min_by(|a, b| {
            let d = |i: &rs_cam_core::selection::Item| {
                let k = i.key();
                (k.anchor[0] - at[0]).hypot(k.anchor[1] - at[1])
            };
            d(a).total_cmp(&d(b))
        })
        .map(|i| i.key().clone())
        .expect("the fixture holds the item")
}

fn stored(k: &MatchKey) -> MatchKey {
    k.clone()
        .for_toolpath_model(rs_cam_core::ids::ModelId::from_raw(0))
}

fn selection(s: &ProjectSession) -> Selection {
    s.toolpath_configs()[0]
        .operation
        .selection()
        .expect("a Profile holds a selection")
        .clone()
}

/// The state of each item of the model, index-aligned with its items.
fn states(s: &ProjectSession) -> Vec<ItemState> {
    let tc = s.toolpath_configs()[0].clone();
    s.selection_status(&tc, SelectionTarget::Op)
        .unwrap()
        .item_states
        .clone()
}

fn state_of(s: &ProjectSession, k: &MatchKey) -> ItemState {
    let index = s.models()[0]
        .items
        .iter()
        .position(|i| i.key().matches(k))
        .expect("the key names an item");
    states(s)[index]
}

/// The keys of the items in state `state`.
fn in_state(s: &ProjectSession, state: ItemState) -> Vec<MatchKey> {
    s.models()[0]
        .items
        .iter()
        .zip(states(s))
        .filter(|(_, st)| *st == state)
        .map(|(i, _)| i.key().clone())
        .collect()
}

fn generate(s: &mut ProjectSession) -> Result<(), SessionError> {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel).map(|_| ())
}

/// Claims 1 to 4: the GUI route on the operator's case.
#[test]
fn a_click_on_the_default_cuts_only_the_picked_square() {
    let mut s = session();
    let spare = key(&s, "SPARE", ItemKind::Loop, [105.0, 5.0]);
    let keep = key(&s, "KEEP", ItemKind::Loop, [80.0, 40.0]);
    let circle = key(&s, "HOLES", ItemKind::Circle, [5.0, 5.0]);
    let start = states(&s);
    // Non-vacuity: the default rule cuts the circle and the square.
    assert!(
        matches!(state_of(&s, &circle), ItemState::Cut | ItemState::Island),
        "the default rule holds the circle"
    );
    assert_eq!(state_of(&s, &spare), ItemState::Cut);

    // Claim 1: the first click replaces the default.
    assert_eq!(click(&mut s, &spare), SelectionEdit::AddPick(spare.clone()));
    assert_eq!(selection(&s), Selection::from_picks(vec![stored(&spare)]));
    assert_eq!(state_of(&s, &spare), ItemState::Cut);
    assert_eq!(state_of(&s, &circle), ItemState::NotSelected);
    assert_eq!(in_state(&s, ItemState::Cut), vec![spare.clone()]);
    generate(&mut s).expect("the pick generates");
    let toolpath = s.result_at(0).expect("a result").toolpath().clone();
    let cuts: Vec<_> = toolpath
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .collect();
    assert!(!cuts.is_empty(), "the pick cuts");
    for m in cuts {
        let p = m.target;
        assert!(
            p.x >= SPARE_BOX[0] - CUT_MARGIN
                && p.x <= SPARE_BOX[2] + CUT_MARGIN
                && p.y >= SPARE_BOX[1] - CUT_MARGIN
                && p.y <= SPARE_BOX[3] + CUT_MARGIN,
            "a cutting move at ({:.3}, {:.3}) is not at the picked square",
            p.x,
            p.y
        );
    }

    // Claim 2: a second click adds.
    assert_eq!(click(&mut s, &keep), SelectionEdit::AddPick(keep.clone()));
    assert_eq!(
        selection(&s),
        Selection::from_picks(vec![stored(&spare), stored(&keep)])
    );
    assert_eq!(state_of(&s, &keep), ItemState::Cut);
    assert_eq!(state_of(&s, &circle), ItemState::NotSelected);

    // Claim 3: a click on each picked item removes it; the empty selection
    // refuses.
    assert_eq!(
        click(&mut s, &spare),
        SelectionEdit::RemovePick(spare.clone())
    );
    assert_eq!(
        click(&mut s, &keep),
        SelectionEdit::RemovePick(keep.clone())
    );
    assert!(selection(&s).is_empty(), "{:?}", selection(&s));
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(
        matches!(
            report.refusal.as_ref().map(|r| &r.cause),
            Some(RefusalCause::Empty { .. })
        ),
        "the empty selection refuses: {:?}",
        report.refusal
    );
    assert!(
        states(&s).iter().all(|st| *st == ItemState::NotSelected),
        "an empty selection holds no item"
    );
    assert!(
        matches!(generate(&mut s), Err(SessionError::Selection(_))),
        "generation refuses the empty selection as the report does"
    );

    // Claim 4: Reset brings the default back.
    apply(&mut s, SelectionEdit::Reset);
    assert_eq!(selection(&s), Selection::drawing());
    assert_eq!(states(&s), start);
}

/// Claim 5: the MCP route (an `add_pick` as it is) and the GUI route give
/// one selection and one set of item states.
#[test]
fn the_mcp_pick_and_the_click_give_one_result() {
    let mut gui = session();
    let mut mcp = session();
    let spare = key(&gui, "SPARE", ItemKind::Loop, [105.0, 5.0]);
    let _ = click(&mut gui, &spare);
    apply(&mut mcp, SelectionEdit::AddPick(spare));
    assert_eq!(selection(&gui), selection(&mcp));
    assert_eq!(states(&gui), states(&mcp));
    // The MCP `add_rule` on the default replaces it too.
    let mut rule_session = session();
    let rule = Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["KEEP".to_owned()],
            ..Filter::default()
        },
    };
    apply(&mut rule_session, SelectionEdit::AddRule(rule.clone()));
    assert_eq!(selection(&rule_session), Selection::from_rules(vec![rule]));
}

/// Claim 6: a layer rule on the default replaces it; a later click adds.
#[test]
fn a_layer_rule_replaces_the_default_and_a_click_then_adds() {
    let mut s = session();
    let keep = key(&s, "KEEP", ItemKind::Loop, [80.0, 40.0]);
    let spare = key(&s, "SPARE", ItemKind::Loop, [105.0, 5.0]);
    let tc = s.toolpath_configs()[0].clone();
    let edit = s
        .layer_rule_edit(&tc, SelectionTarget::Op, &keep, &[ItemKind::Loop], None)
        .unwrap();
    apply(&mut s, edit);
    let rules = selection(&s).rules().to_vec();
    assert_eq!(rules.len(), 1, "{rules:?}");
    assert_eq!(rules[0].filter.layers, vec!["KEEP".to_owned()]);
    assert!(selection(&s).picks().is_empty());
    assert_eq!(click(&mut s, &spare), SelectionEdit::AddPick(spare.clone()));
    assert_eq!(selection(&s).rules(), rules.as_slice());
    assert_eq!(selection(&s).picks(), &[stored(&spare)]);
}

/// Claim 7 (GT-Q8): a click on an item that a rule holds turns the rule
/// into picks, in one command; the MCP `remove_pick` agrees.
#[test]
fn a_click_on_a_rule_item_turns_the_rule_into_picks_gt_q8() {
    let outer_rule = Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["OUTER".to_owned()],
            ..Filter::default()
        },
    };
    let mut gui = session();
    apply(
        &mut gui,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![outer_rule.clone()]))),
    );
    let rings: Vec<MatchKey> = gui.models()[0]
        .items
        .iter()
        .filter(|i| i.layer() == Some("OUTER"))
        .map(|i| i.key().clone())
        .collect();
    assert_eq!(rings.len(), 3, "the OUTER layer holds three squares");
    // The 10..50 square: its ring starts at X 10.
    let middle = gui.models()[0]
        .items
        .iter()
        .filter(|i| i.layer() == Some("OUTER"))
        .find(|i| {
            i.shape().drawing_ring().is_some_and(|r| {
                let x0 = r.exterior.iter().map(|p| p.x).fold(f64::MAX, f64::min);
                (x0 - 10.0).abs() < 1e-6
            })
        })
        .map(|i| i.key().clone())
        .expect("the 10..50 square");
    assert_eq!(
        state_of(&gui, &middle),
        ItemState::Island,
        "non-vacuity: the rule holds the square"
    );
    // One click is one `EditSelection`: one command, one undo step.
    let edit = click(&mut gui, &middle);
    assert_eq!(edit, SelectionEdit::RemovePick(middle.clone()));
    let after = selection(&gui);
    assert!(after.rules().is_empty(), "the rule became picks: {after:?}");
    assert!(after.excludes().is_empty(), "{after:?}");
    let mut want: Vec<MatchKey> = rings
        .iter()
        .filter(|k| !k.matches(&middle))
        .map(stored)
        .collect();
    let mut got = after.picks().to_vec();
    let order = |k: &MatchKey| (k.anchor[0].to_bits(), k.measure.to_bits());
    want.sort_by_key(order);
    got.sort_by_key(order);
    assert_eq!(got, want, "all the rule members less the dropped one");
    assert_eq!(state_of(&gui, &middle), ItemState::NotSelected);

    // The MCP route: `remove_pick` as it is.
    let mut mcp = session();
    apply(
        &mut mcp,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![outer_rule]))),
    );
    apply(&mut mcp, SelectionEdit::RemovePick(middle));
    assert_eq!(selection(&mcp), after);
    assert_eq!(states(&mcp), states(&gui));

    // On the default rule: the members are the items the op cuts, so the
    // open path that the Profile leaves out does not become a pick.
    let mut dflt = session();
    let circle = key(&dflt, "HOLES", ItemKind::Circle, [5.0, 5.0]);
    let cut_before: Vec<MatchKey> = in_state(&dflt, ItemState::Cut)
        .into_iter()
        .chain(in_state(&dflt, ItemState::Island))
        .collect();
    assert!(
        dflt.models()[0]
            .items
            .iter()
            .any(|i| i.kind() == ItemKind::Path),
        "non-vacuity: the fixture holds an open path"
    );
    apply(&mut dflt, SelectionEdit::RemovePick(circle.clone()));
    let picked = selection(&dflt);
    assert!(picked.rules().is_empty(), "{picked:?}");
    assert!(
        picked.picks().iter().all(|k| k.kind != ItemKind::Path),
        "an open path the Profile leaves out is not picked: {picked:?}"
    );
    assert_eq!(picked.picks().len(), cut_before.len() - 1);
    assert_eq!(state_of(&dflt, &circle), ItemState::NotSelected);
    let tc = dflt.toolpath_configs()[0].clone();
    assert!(
        dflt.selection_status(&tc, SelectionTarget::Op)
            .unwrap()
            .refusal
            .is_none(),
        "the picks of the default members do not refuse"
    );
}
