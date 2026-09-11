//! WP3 — a completion is adopted through the command door, and a stale
//! one is refused.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP3 and §12 rulings 1 to 5.
//!
//! `ProjectSession::insert_result` wrote a computed result into the cache
//! and read no revision. A generation runs off the frame loop, so the
//! configuration can move while it runs. The old door adopted the answer
//! anyway, and core then held a result that answers a superseded
//! parameter set. `Command::AdoptResult` carries the revision the lane
//! computed from. `apply` compares it against
//! `ProjectSession::toolpath_revision` and refuses a mismatch.
//!
//! The third test measures the other half of WP3: every mutation reports
//! the set it dropped through `Effects::stale`, from ONE construction
//! site.
//!
//! No GUI. The refusal lives in core, so the sentry lives in core.
//!
//! NOT MEASURED: `Effects::simulation_cleared`. The `simulation` field is
//! private and the crate publishes no setter, so an integration test
//! cannot seed a simulation. This file asserts no case of it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSession, SessionError, SetToolpathParamArgs,
    ToolpathConfig,
};

/// The feed value every mutating arm writes.
const EDITED_FEED_RATE: f64 = 4321.0;

// ── fixture ──────────────────────────────────────────────────────

fn tc(
    name: &str,
    op: OperationConfig,
    stock_source: StockSource,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0, // overwritten by `add_model`
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn fake_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::config::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// One setup, one tool, one model, two enabled toolpaths in plan order.
///
/// Index 0 is a `Fresh` Pocket. Index 1 is a Rest that reads the stock
/// index 0 leaves, so a wide invalidation of index 0 reaches it. The
/// fixture is the one `mutation_paths_invalidate_alike_p0.rs` uses, with
/// one difference: NEITHER row carries a cached result, because this
/// file adopts results through the door under test.
fn fixture() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let _ = s.add_model(empty_model("part.svg"));
    let tool = s.tools()[0].id.0;
    let model = s.models()[0].id;
    let upstream = tc(
        "upstream",
        OperationConfig::Pocket(PocketConfig::default()),
        StockSource::Fresh,
        tool,
        model,
    );
    let downstream = tc(
        "downstream",
        OperationConfig::Rest(RestConfig::default()),
        StockSource::FromRemainingStock,
        tool,
        model,
    );
    let _ = s.add_toolpath(0, upstream).unwrap();
    let _ = s.add_toolpath(0, downstream).unwrap();
    assert_fixture_is_live(&s);
    s
}

/// The non-vacuity guard. A refusal proves nothing unless the cache
/// starts empty, and unless the downstream row really reads the stock the
/// upstream row leaves.
fn assert_fixture_is_live(s: &ProjectSession) {
    assert!(
        s.get_result(0).is_none() && s.get_result(1).is_none(),
        "the fixture must start with an empty result cache, or an \
         is_none assertion after a refusal measures nothing"
    );
    assert!(
        matches!(
            s.toolpath_configs()[1].stock_source,
            StockSource::FromRemainingStock
        ),
        "the downstream row must read the stock the upstream row leaves"
    );
    assert!(
        s.toolpath_configs()[0].enabled && s.toolpath_configs()[1].enabled,
        "a disabled row takes another branch in invalidate_output_dependents"
    );
}

/// Deliver a completion for `index` at the revision it carries now.
///
/// This is the door `insert_result` used to be. It is the only way this
/// file writes the result cache.
fn adopt_at_current_revision(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision(index);
    let effects = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("a completion at the current revision answers the current inputs");
    assert!(
        effects.stale.is_empty(),
        "recording an answer is not a change of inputs, so an adoption \
         stales nothing"
    );
}

fn edit_feed(s: &mut ProjectSession, index: usize, feed: f64) {
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(feed),
        }))
        .expect("feed_rate is a parameter of every operation here");
}

fn revisions(s: &ProjectSession) -> Vec<u64> {
    (0..s.toolpath_count())
        .map(|i| s.toolpath_revision(i))
        .collect()
}

/// The indices whose cached result is gone after a mutation.
fn dropped(s: &ProjectSession, before: &[bool]) -> BTreeSet<usize> {
    let mut gone = BTreeSet::new();
    for i in 0..s.toolpath_count() {
        if before.get(i).copied() == Some(true) && s.get_result(i).is_none() {
            gone.insert(i);
        }
    }
    gone
}

fn cached(s: &ProjectSession) -> Vec<bool> {
    (0..s.toolpath_count())
        .map(|i| s.get_result(i).is_some())
        .collect()
}

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

// ── arm (a): the edited toolpath's own completion ────────────────

/// A completion whose revision the edit superseded is refused, and the
/// cache stays empty.
#[test]
fn a_completion_for_a_superseded_parameter_set_is_refused() {
    let mut s = fixture();
    let submitted = s.toolpath_revision(0);

    // The lane is now computing from `submitted`. The operator edits the
    // SAME toolpath, which moves its revision.
    edit_feed(&mut s, 0, EDITED_FEED_RATE);
    let current = s.toolpath_revision(0);
    assert_ne!(
        submitted, current,
        "the edit must move the revision, or this arm measures nothing"
    );

    let outcome = s.apply(Command::AdoptResult(AdoptResultArgs {
        index: 0,
        revision: submitted,
        result: Box::new(fake_result()),
    }));

    assert!(
        matches!(
            outcome,
            Err(SessionError::StaleCompletion {
                index: 0,
                submitted: s2,
                current: c2,
            }) if s2 == submitted && c2 == current
        ),
        "the door must refuse a completion that answers a superseded \
         parameter set, and name both revisions"
    );
    assert!(
        s.get_result(0).is_none(),
        "a refused completion inserts nothing"
    );

    // Non-vacuity: the same result at the CURRENT revision is adopted, so
    // the refusal above is the revision check, not a broken door.
    adopt_at_current_revision(&mut s, 0);
    assert!(
        s.get_result(0).is_some(),
        "a completion that answers the current inputs is adopted"
    );
}

// ── arm (b): an edit to a DIFFERENT toolpath ─────────────────────

/// An edit to another toolpath does not supersede this one's completion.
///
/// This arm fails if the stamp reads `next_revision`, the session-global
/// counter that any index bumps.
#[test]
fn an_edit_to_another_toolpath_does_not_refuse_this_completion() {
    let mut s = fixture();
    let submitted = s.toolpath_revision(0);

    // The operator edits the DOWNSTREAM row. Index 1 is later in plan
    // order, so the chain walk never reaches index 0.
    edit_feed(&mut s, 1, EDITED_FEED_RATE);
    assert_eq!(
        s.toolpath_revision(0),
        submitted,
        "an edit to a later row leaves the upstream row's inputs alone"
    );

    let effects = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision: submitted,
            result: Box::new(fake_result()),
        }))
        .expect("index 0's inputs did not move, so its completion stands");

    assert!(
        s.get_result(0).is_some(),
        "the completion answers the current inputs and is adopted"
    );
    assert!(
        effects.stale.is_empty(),
        "an adoption records an answer; it changes no toolpath's inputs"
    );
    assert_eq!(
        effects.revision,
        Some(submitted),
        "Effects::revision names the adopted toolpath's own revision"
    );
}

/// An index that names no toolpath is refused as absent, not as stale.
#[test]
fn an_absent_index_is_refused_as_absent() {
    let mut s = fixture();
    let outcome = s.apply(Command::AdoptResult(AdoptResultArgs {
        index: 99,
        revision: 0,
        result: Box::new(fake_result()),
    }));
    assert!(
        matches!(outcome, Err(SessionError::ToolpathNotFound(99))),
        "index 99 names no toolpath, so the refusal is ToolpathNotFound"
    );
}

// ── every producer reports the dropped set ───────────────────────

/// Three representative producers report, through `Effects::stale`, the
/// set whose cached result went away.
///
/// One construction site builds every `Effects`, so a producer cannot
/// report a set of its own. The three rows are a rebind (wide), a stock
/// edit (bulk) and the enable toggle (downstream only).
#[test]
fn every_producer_reports_the_dropped_set() {
    // A rebind. The chain walk drops the edited row and the row that
    // reads its remaining stock.
    let mut s = fixture();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::BallNose));
    let tool_b = s.tools()[1].id.0;
    adopt_at_current_revision(&mut s, 0);
    adopt_at_current_revision(&mut s, 1);
    let before = cached(&s);
    let effects = s.set_toolpath_tool(0, tool_b).expect("tool_b exists");
    assert_eq!(
        effects.stale,
        dropped(&s, &before),
        "set_toolpath_tool must report the set whose result it dropped"
    );
    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "a different cutter removes different material, so the \
         downstream row goes with it"
    );

    // A stock edit. Every toolpath goes (G-FRESHSTATE).
    let mut s = fixture();
    adopt_at_current_revision(&mut s, 0);
    adopt_at_current_revision(&mut s, 1);
    let before = cached(&s);
    let effects = s.set_stock_config(StockConfig::default());
    assert_eq!(
        effects.stale,
        dropped(&s, &before),
        "set_stock_config must report the set whose result it dropped"
    );
    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "any stock edit makes every toolpath edited-since"
    );
}

/// The enable toggle reports the downstream set and keeps its own
/// result.
///
/// N1's design: a disabled op's own result stays valid for a re-enable.
/// `Effects::stale` is the revision-moved set, so it excludes the
/// toggled index, and that index's revision does not move.
#[test]
fn the_enable_toggle_excludes_its_own_index() {
    let mut s = fixture();
    adopt_at_current_revision(&mut s, 0);
    adopt_at_current_revision(&mut s, 1);
    let before_revisions = revisions(&s);
    let before = cached(&s);

    let effects = s.set_toolpath_enabled(0, false).expect("index 0 exists");

    assert_eq!(
        effects.stale,
        set(&[1]),
        "the toggle invalidates what was built on the op's stock, not \
         the op's own result"
    );
    assert_eq!(
        effects.stale,
        dropped(&s, &before),
        "the reported set is the set whose result went away"
    );
    assert!(
        s.get_result(0).is_some(),
        "N1: the toggled op keeps its own result for a re-enable"
    );
    assert_eq!(
        effects.revision,
        Some(before_revisions[0]),
        "the toggled op's inputs did not move, so its revision stands"
    );
}
