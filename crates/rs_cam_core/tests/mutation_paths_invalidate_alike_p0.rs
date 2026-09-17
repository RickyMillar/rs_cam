//! Phase 0 — "Equivalent edits through setter, undo, and optimizer
//! application invalidate the same dependencies".
//!
//! Programme: `planning/arch_consolidation_2026-09-09/STATUS.md`,
//! Finding 2 and rows N6, N14, N15. Phase 1A owns the fix. This file owns
//! the evidence. It runs the SAME logical edit — `feed_rate = 4321.0` on
//! the toolpath at index 0 — through four core write paths. It records
//! what each path invalidates.
//!
//! ## The four paths, and what each one invalidates today
//!
//! | Arm | Path | Entry point | Today |
//! |---|---|---|---|
//! | 1 | MCP and CLI setter | `set_toolpath_param` -> `ProjectSession::apply` -> `set_toolpath_param_impl` | WIDE |
//! | 2 | GUI undo and redo | `ProjectSession::apply` -> `Command::RestoreToolpathSnapshot` | WIDE |
//! | 3 | feeds Apply funnel | a direct field write, then `invalidate_toolpath_inputs` | WIDE, in-crate since WP7 |
//! | 4 | wholesale config replacement | `ProjectSession::apply` -> `Command::ReplaceToolpathConfig` | WIDE |
//! | 5 | tool edit | `ProjectSession::apply` -> `Command::SetToolParam` -> `drop_tool_results` | WIDE, since SES-07 |
//! | 6 | model refresh | `ProjectSession::apply` -> `Command::AdoptModelGeometry` -> `drop_results_for_model` | WIDE, since SES-07 |
//!
//! Arms 5 and 6 are the SES-07 pair. `drop_tool_results` and
//! `drop_results_for_model` used to `drop_result` the DIRECTLY affected
//! toolpaths in a flat loop and stop. A downstream
//! `StockSource::FromRemainingStock` op that runs on a DIFFERENT tool kept
//! a cached result built on stock the edit had moved. Both doors now seed
//! `invalidate_output_dependents_of_set`, so they walk the chain every
//! other wide path walks.
//!
//! A WIDE path calls `invalidate_result_chain` (`mutation.rs:236`). That
//! drops the edited toolpath's own result. It then walks the setup's
//! material-removal chain (`invalidate_output_dependents`, `:246-320`).
//! A NARROW path ends at `drop_result` plus `simulation = None`, so a
//! downstream `StockSource::FromRemainingStock` result stays cached after
//! the stock above it moves.
//!
//! WP8 moved arm 2. It called `apply_toolpath_param_snapshot`, the one
//! narrow path of the four, and the GUI undo stack, the GUI redo stack
//! and three optimizer apply sites all ended there. The four GUI sites
//! now take `Command::RestoreToolpathSnapshot`, which is wide.
//!
//! The core optimizer keeps a narrow fifth path,
//! `apply_toolpath_param_snapshot_narrow`, on purpose: its candidate
//! search regenerates one index and simulates it against cached
//! neighbours. This file cannot reach that path — `pub(crate)` keeps it
//! inside the crate — so the in-crate test
//! `the_narrow_path_leaves_the_neighbour_result_cached`
//! (`session/mutation.rs`) pins it instead.
//!
//! WP7 moved ARM 3 there for the same reason. The nine `ProjectSession`
//! mutation hatches are `pub(crate)`, so this file — an external crate —
//! can no longer write a field without a command, which is what arm 3
//! measures. `session/mutation.rs` carries the arm and its two
//! assertions: `the_setter_and_the_inspector_door_agree` and
//! `the_inspector_door_drops_and_bumps_the_same_set`. Its fixture is the
//! one below, rebuilt in-crate. The non-vacuity guard stays HERE and
//! points at the arms that remain.
//!
//! Arm 4 also corrects the audit. `AUDIT.md:68` calls
//! `replace_toolpath_config` narrow. It is wide since R0.1 §4.3.
//!
//! WP5 moved arm 4 too. The dead `ProjectSession::replace_toolpath_config`
//! became `Command::ReplaceToolpathConfig`, which the GUI inspector calls
//! on every frame its panel is open. The row gates its invalidation on
//! `ToolpathConfig::generation_inputs_signature`, so this arm keeps its
//! expectation only because it edits the OPERATION, which is a generation
//! input. `replace_toolpath_config_gates_on_the_signature.rs` owns the
//! gate itself.
//!
//! ## Contract assertions, and pinned divergences
//!
//! CONTRACT — these must stay green through Phase 1A and after it:
//!
//! - `the_setter_and_the_replacement_agree`: arms 1 and 4 drop the same
//!   set, `{0, 1}`.
//! - `n15_apply_reports_the_set_the_setter_dropped`: the command door's
//!   `Effects::stale` equals the set core dropped. WP1 closed this row.
//! - `a_dropped_result_and_a_bumped_revision_are_the_same_event`: on every
//!   arm the set of indices whose result went away equals the set whose
//!   `toolpath_revision` moved. `drop_result` (`mutation.rs:1328`) is the
//!   only bump site on these four paths.
//! - `n6_set_alignment_pin_drill_holes_invalidates_the_chain`.
//! - `n14_undo_and_the_setter_invalidate_alike` — WP8 closed N14.
//! - `n6_a_drill_pick_invalidates_the_chain` — WP8 closed N6.
//! - `the_fixture_is_live`, the non-vacuity guard.
//!
//! PINNED DIVERGENCE — none remain. One stood here and named the narrow
//! answer the second staleness producer gave. WP28 part 2 deleted that
//! producer, so the divergence has no second side to pin; the test went
//! with it. `tests/stale_set_has_one_answer_wp28.rs` keeps the deletion.
//!
//! ## Scope, and what this file does NOT cover
//!
//! Core only. The viz half of the same question is not here: the
//! inspector panel write-back, `AppEvent::Undo`, the optimizer candidate
//! apply, and the feeds Apply funnel (N13). N13's writer covers the feeds
//! arm. Phase 1A adds the rest.
//!
//! Two measurement notes:
//!
//! - `ProjectSession` derives no `Clone`, so each arm rebuilds the
//!   fixture instead of cloning one.
//! - The simulation half of the observation is NOT MEASURED. The
//!   `simulation` field is private and the crate publishes no setter, so
//!   an integration test cannot seed one. Every arm reads
//!   `simulation_result() == None` after the edit only because it read
//!   `None` before it, and that is not evidence. All four paths write
//!   `simulation = None`; I read that at `mutation.rs:249,945,1249` and
//!   in `set_toolpath_param_impl`'s chain call. `Effects::simulation_cleared`
//!   has the same limit: this file asserts no case of it.

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
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillConfig, PocketConfig, RestConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptModelGeometryArgs, AdoptResultArgs, Command, LoadedModel, ProjectSession,
    ProjectSessionBuilder, ReplaceToolpathConfigArgs, RestoreToolpathSnapshotArgs,
    SetAlignmentPinDrillHolesArgs, SetDrillSelectedHolesArgs, SetToolParamArgs,
    SetToolpathParamArgs, ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The one feed value every arm writes.
const EDITED_FEED_RATE: f64 = 4321.0;

/// The tool diameter arm 5 writes. `ToolConfig::new_default` carries
/// another value, so the write is visible.
const EDITED_TOOL_DIAMETER: f64 = 7.5;

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
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::config::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Deliver a computed result the way the compute lane does.
///
/// `insert_result` was the public door before WP3. A completion now
/// carries the revision the lane started from.
fn adopt(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision(index);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("the fixture adopts at the current revision");
}

/// One setup, one tool, one model, two enabled toolpaths in plan order:
///
/// - index 0, `upstream`, `StockSource::Fresh`, the operation the caller
///   names. Every arm edits this row.
/// - index 1, `downstream`, a Rest with `StockSource::FromRemainingStock`.
///   A WIDE path drops this result. A NARROW path keeps it.
///
/// Both rows carry a cached result before the caller edits anything.
fn fixture(upstream: OperationConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
    let upstream_tc = tc("upstream", upstream, StockSource::Fresh, tool, model);
    let downstream_tc = tc(
        "downstream",
        OperationConfig::Rest(RestConfig::default()),
        StockSource::FromRemainingStock,
        tool,
        model,
    );
    let _ = builder.add_toolpath(0, upstream_tc).unwrap();
    let _ = builder.add_toolpath(0, downstream_tc).unwrap();
    let mut s = builder.build();
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    assert_fixture_is_live(&s);
    s
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig::default())
}

/// The SES-07 fixture: the same two rows, on TWO tools and TWO models.
///
/// Index 0, `upstream`, runs tool 0 on model 0 with `StockSource::Fresh`.
/// Index 1, `downstream`, is a Rest with `StockSource::FromRemainingStock`
/// that runs tool 1 on model 1.
///
/// The second tool and the second model are the non-vacuity of arms 5 and
/// 6. In [`fixture`] both rows name one tool and one model, so the OLD
/// narrow loop — which filtered on `tc.tool_id == tool_id` and on
/// `tc.model_id == model_id` — dropped index 1 directly and an arm built
/// on that fixture would pass without the fix. Here only the chain walk
/// reaches index 1.
fn fixture_split_inputs() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_tool(ToolConfig::new_default(ToolId(1), ToolType::BallNose));
    builder.add_model(empty_model("part.svg"));
    builder.add_model(empty_model("other.svg"));
    let upstream_tool = builder.tools()[0].id.0;
    let downstream_tool = builder.tools()[1].id.0;
    let upstream_model = builder.models()[0].id;
    let downstream_model = builder.models()[1].id;
    let upstream_tc = tc(
        "upstream",
        pocket(),
        StockSource::Fresh,
        upstream_tool,
        upstream_model,
    );
    let downstream_tc = tc(
        "downstream",
        OperationConfig::Rest(RestConfig::default()),
        StockSource::FromRemainingStock,
        downstream_tool,
        downstream_model,
    );
    let _ = builder.add_toolpath(0, upstream_tc).unwrap();
    let _ = builder.add_toolpath(0, downstream_tc).unwrap();
    let mut s = builder.build();
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    assert_fixture_is_live(&s);
    assert_ne!(
        s.toolpath_configs()[0].tool_id,
        s.toolpath_configs()[1].tool_id,
        "the downstream row must run another tool, or the narrow loop \
         reaches it directly and the arm measures nothing"
    );
    assert_ne!(
        s.toolpath_configs()[0].model_id,
        s.toolpath_configs()[1].model_id,
        "the downstream row must read another model, or the narrow loop \
         reaches it directly and the arm measures nothing"
    );
    s
}

/// Arm 5 — `Command::SetToolParam`, the MCP and CLI tool-edit door.
///
/// It writes the upstream row's tool diameter. A wider cutter removes
/// other material, so the stock the downstream Rest op inherits moves.
fn arm_tool_param() -> Observation {
    let mut s = fixture_split_inputs();
    let before = revisions(&s);
    let _ = s
        .apply(Command::SetToolParam(SetToolParamArgs {
            index: 0,
            param: "diameter".to_owned(),
            value: serde_json::json!(EDITED_TOOL_DIAMETER),
        }))
        .expect("diameter is a tool parameter");
    let written = s.tools()[0].diameter;
    assert!(
        (written - EDITED_TOOL_DIAMETER).abs() < 1e-9,
        "the arm must write the diameter. I read {written}"
    );
    observe(&s, &before)
}

/// Arm 6 — `Command::AdoptModelGeometry`, the GUI model-refresh door.
///
/// All three refresh doors — rescale, reload and relink — end here.
fn arm_adopt_model() -> Observation {
    let mut s = fixture_split_inputs();
    let before = revisions(&s);
    let model_id = s.toolpath_configs()[0].model_id;
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(empty_model("part.svg")),
            units: None,
        }))
        .expect("model 0 exists");
    observe(&s, &before)
}

/// The non-vacuity guard. A dropped-set assertion proves nothing unless
/// both results exist first, and unless the downstream row really depends
/// on the upstream one.
fn assert_fixture_is_live(s: &ProjectSession) {
    assert!(
        s.get_result(0).is_some(),
        "index 0 must carry a cached result before the edit"
    );
    assert!(
        s.get_result(1).is_some(),
        "index 1 must carry a cached result before the edit"
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
    assert_eq!(
        s.list_setups()[0].toolpath_indices,
        vec![0, 1],
        "FromRemainingStock carries no upstream pointer. The binding is \
         plan order inside one setup."
    );
}

// ── observation ──────────────────────────────────────────────────

/// What one write path invalidated.
#[derive(Debug)]
struct Observation {
    /// Indices whose cached result is gone after the edit.
    dropped: BTreeSet<usize>,
    /// Indices whose `toolpath_revision` moved. `next_revision` is one
    /// global monotonic counter, so a numeric delta compares nothing
    /// across arms. The SET of moved indices does.
    bumped: BTreeSet<usize>,
}

fn revisions(s: &ProjectSession) -> Vec<u64> {
    (0..s.toolpath_count())
        .map(|i| s.toolpath_revision(i))
        .collect()
}

fn observe(s: &ProjectSession, before: &[u64]) -> Observation {
    let mut dropped = BTreeSet::new();
    let mut bumped = BTreeSet::new();
    for i in 0..s.toolpath_count() {
        if s.get_result(i).is_none() {
            dropped.insert(i);
        }
        if before.get(i).copied() != Some(s.toolpath_revision(i)) {
            bumped.insert(i);
        }
    }
    Observation { dropped, bumped }
}

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

fn assert_feed_landed(s: &ProjectSession) {
    let feed = s.toolpath_configs()[0].operation.feed_rate();
    assert!(
        (feed - EDITED_FEED_RATE).abs() < 1e-9,
        "the arm must write the feed. Otherwise it invalidates nothing \
         for a reason this file does not measure. I read {feed}"
    );
}

// ── the four arms ────────────────────────────────────────────────

/// Arm 1 — `Command::SetToolpathParam`, the MCP and CLI door.
///
/// WP15b retargeted this arm. It called `ProjectSession::set_toolpath_param`,
/// which is `pub(crate)` from that package on, so an integration test cannot
/// reach it. The row runs the same body: `apply` dispatches
/// `set_toolpath_param_impl`, and the setter was already a wrapper over
/// `apply` from WP1. The arm therefore measures the same path it always
/// measured, through the one door.
fn arm_setter() -> Observation {
    let mut s = fixture(pocket());
    let before = revisions(&s);
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");
    assert_feed_landed(&s);
    observe(&s, &before)
}

/// Arm 2 — `Command::RestoreToolpathSnapshot`, the GUI undo and redo
/// door.
///
/// WP8 retargeted this arm. It called `apply_toolpath_param_snapshot`,
/// which the optimizer still calls and which stays narrow on purpose. A
/// `pub(crate)` path does not compile from `tests/`, so the arm could
/// not be flipped in place; it names the door the GUI takes now.
fn arm_snapshot() -> Observation {
    let mut s = fixture(pocket());
    let before = revisions(&s);
    let mut op = s.toolpath_configs()[0].operation.clone();
    op.set_feed_rate(EDITED_FEED_RATE);
    let dressups = s.toolpath_configs()[0].dressups.clone();
    let faces = s.toolpath_configs()[0].face_selection.clone();
    let _ = s
        .apply(Command::RestoreToolpathSnapshot(
            RestoreToolpathSnapshotArgs {
                index: 0,
                operation: Box::new(op),
                dressups: Box::new(dressups),
                face_selection: faces,
                feeds_provenance: None,
            },
        ))
        .expect("index 0 exists");
    assert_feed_landed(&s);
    observe(&s, &before)
}

// Arm 3 — a direct field write, then the public door
// `ProjectSession::invalidate_toolpath_inputs` — stood here. WP7 made
// the nine mutation hatches `pub(crate)`, so an integration test cannot
// write a field without a command any more. The arm moved in-crate, to
// the `#[cfg(test)]` module of `session/mutation.rs`, where it keeps
// both its reach and its assertions:
// `the_setter_and_the_inspector_door_agree` and
// `the_inspector_door_drops_and_bumps_the_same_set`.

/// Arm 4 — `Command::ReplaceToolpathConfig`, the wholesale replacement
/// the audit records as narrow, and the door the GUI inspector takes.
///
/// WP5 retargeted this arm. The raw `replace_toolpath_config` had zero
/// production callers and invalidated unconditionally; the row that
/// replaced it gates on the generation-inputs signature. The arm edits
/// the operation, which is inside that signature, so the expectation
/// below does not move.
fn arm_replace_config() -> Observation {
    let mut s = fixture(pocket());
    let before = revisions(&s);
    let tool = s.toolpath_configs()[0].tool_id;
    let model = s.toolpath_configs()[0].model_id;
    let id = s.toolpath_configs()[0].id;
    let mut op = s.toolpath_configs()[0].operation.clone();
    op.set_feed_rate(EDITED_FEED_RATE);
    let mut edited = tc("upstream", op, StockSource::Fresh, tool, model);
    edited.id = id;
    let _ = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(edited),
        }))
        .expect("index 0 exists");
    assert_feed_landed(&s);
    observe(&s, &before)
}

// ── contract assertions ──────────────────────────────────────────

#[test]
fn the_fixture_is_live() {
    let s = fixture(pocket());
    assert_fixture_is_live(&s);
}

/// CONTRACT. Two wide paths, one logical edit, one invalidated set.
///
/// The inspector-door arm moved in-crate at WP7 and carries the third
/// comparison there. This test keeps the two arms an integration test
/// can still drive.
#[test]
fn the_setter_and_the_replacement_agree() {
    let setter = arm_setter();
    let replacement = arm_replace_config();

    assert_eq!(
        setter.dropped,
        set(&[0, 1]),
        "set_toolpath_param walks the stock chain. The edited row and the \
         downstream FromRemainingStock row both go stale."
    );
    assert_eq!(
        replacement.dropped, setter.dropped,
        "Command::ReplaceToolpathConfig is WIDE on a moved generation \
         input. AUDIT.md:68 calls the replacement narrow and is stale."
    );
}

/// CONTRACT. `drop_result` (`mutation.rs:1328`) is the only revision-bump
/// site on these four paths, so the two observations are one event.
#[test]
fn a_dropped_result_and_a_bumped_revision_are_the_same_event() {
    let arms = [
        ("set_toolpath_param", arm_setter()),
        ("Command::RestoreToolpathSnapshot", arm_snapshot()),
        ("Command::ReplaceToolpathConfig", arm_replace_config()),
        ("Command::SetToolParam", arm_tool_param()),
        ("Command::AdoptModelGeometry", arm_adopt_model()),
    ];
    for (name, obs) in arms {
        assert_eq!(
            obs.bumped, obs.dropped,
            "{name}: a reader that compares revisions and a reader that \
             checks for a cached result must reach one answer"
        );
    }
}

// ── closed contracts, and the remaining pinned divergence ────────

/// CONTRACT — N14, closed by WP8.
///
/// The GUI undo and redo door drops the set the setter drops. Before WP8
/// it dropped the edited index alone, so a downstream
/// `FromRemainingStock` result survived an undo of the stock above it.
/// The core optimizer keeps a narrow path; the module doc says where it
/// is pinned.
#[test]
fn n14_undo_and_the_setter_invalidate_alike() {
    let snapshot = arm_snapshot();
    let setter = arm_setter();

    assert_eq!(
        snapshot.dropped,
        set(&[0, 1]),
        "N14: the restore command walks the stock chain. The edited row \
         and the downstream FromRemainingStock row both go stale."
    );
    assert_eq!(
        snapshot.dropped, setter.dropped,
        "N14: one logical edit, one invalidated set. The two paths \
         disagreed before WP8."
    );
    assert!(
        snapshot.dropped.contains(&0),
        "the edited row's own result goes on either contract"
    );
}

/// CONTRACT — N6, closed by WP8. `set_drill_selected_holes` calls
/// `invalidate_result_chain`, as its pin-hole sibling below already did.
#[test]
fn n6_a_drill_pick_invalidates_the_chain() {
    let mut s = fixture(OperationConfig::Drill(DrillConfig::default()));
    let before = revisions(&s);
    let _ = s
        .apply(Command::SetDrillSelectedHoles(SetDrillSelectedHolesArgs {
            index: 0,
            selected_holes: Some(vec![[1.0, 2.0]]),
        }))
        .expect("index 0 is a Drill operation");
    let obs = observe(&s, &before);

    assert_eq!(
        obs.dropped,
        set(&[0, 1]),
        "N6: a drill pick changes which holes the op cuts, so it changes \
         the stock the downstream row inherits. It dropped {{0}} alone \
         before WP8."
    );
    assert_eq!(obs.bumped, obs.dropped, "one event, as above");
}

/// CONTRACT, and the sibling of N6. `set_alignment_pin_drill_holes`
/// (`mutation.rs:891`) sits beside the drill-pick setter above and calls
/// `invalidate_result_chain`. It always did. The answer it gives is the
/// answer WP8 gave the drill-pick setter.
#[test]
fn n6_set_alignment_pin_drill_holes_invalidates_the_chain() {
    let op = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default());
    let mut s = fixture(op);
    let before = revisions(&s);
    let _ = s
        .apply(Command::SetAlignmentPinDrillHoles(
            SetAlignmentPinDrillHolesArgs {
                index: 0,
                holes: vec![[1.0, 2.0]],
            },
        ))
        .expect("index 0 is an AlignmentPinDrill operation");
    let obs = observe(&s, &before);

    assert_eq!(
        obs.dropped,
        set(&[0, 1]),
        "the pin-hole setter walks the chain. This is the answer the \
         drill-pick setter above reaches too, since WP8."
    );
}

/// CONTRACT — SES-07. A tool edit walks the chain the toolpath edit walks.
///
/// `drop_tool_results` filtered the toolpath list for a DIRECT dependency
/// (`tc.tool_id == tool_id`, plus a `PlannedTierRegions` ladder member) and
/// called `drop_result` on each hit. It never walked the stock chain, so a
/// downstream `StockSource::FromRemainingStock` op on ANOTHER tool kept a
/// result generated against stock the wider cutter no longer leaves.
///
/// The fixture gives the downstream row its own tool, so the direct filter
/// cannot reach index 1. Only the chain walk can.
#[test]
fn ses07_a_tool_edit_invalidates_the_chain() {
    let obs = arm_tool_param();

    assert_eq!(
        obs.dropped,
        set(&[0, 1]),
        "SES-07: a tool edit moves the stock the downstream row inherits. \
         It dropped {{0}} alone before the fix."
    );
    assert_eq!(obs.bumped, obs.dropped, "one event, as above");
}

/// CONTRACT — SES-07, the model half. `drop_results_for_model` carried the
/// same narrow loop, and all three GUI refresh doors reach it.
#[test]
fn ses07_a_model_refresh_invalidates_the_chain() {
    let obs = arm_adopt_model();

    assert_eq!(
        obs.dropped,
        set(&[0, 1]),
        "SES-07: a refreshed model replaces the geometry index 0 was \
         generated against, so the stock index 1 inherits moves. It \
         dropped {{0}} alone before the fix."
    );
    assert_eq!(obs.bumped, obs.dropped, "one event, as above");
}

/// CONTRACT — SES-07. The two doors answer what the toolpath setter
/// answers for the same logical question: "these inputs moved".
#[test]
fn ses07_the_tool_and_model_doors_invalidate_alike() {
    let tool = arm_tool_param();
    let model = arm_adopt_model();

    assert_eq!(
        tool.dropped, model.dropped,
        "SES-07: one invalidation rule, two doors onto it"
    );
    assert_eq!(
        tool.dropped,
        arm_setter().dropped,
        "SES-07: the tool door reaches the set the toolpath setter reaches"
    );
}

/// CONTRACT — N15. The command door reports what the setter dropped.
///
/// WP1 gave the setter one door, `ProjectSession::apply`. The door
/// measures `Effects::stale` from the revision map around the mutation,
/// so the reported set IS the dropped set. The MCP reply reads that
/// field, so the reply and core no longer carry two staleness models.
#[test]
fn n15_apply_reports_the_set_the_setter_dropped() {
    let mut s = fixture(pocket());
    let before = revisions(&s);

    let effects = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");

    let observed = observe(&s, &before);
    assert_eq!(
        observed.dropped,
        set(&[0, 1]),
        "the setter walks the stock chain, so both rows lose their result"
    );
    assert_eq!(
        effects.stale, observed.dropped,
        "N15: Effects::stale must equal what core dropped. Before WP1 \
         the MCP reply re-derived a narrower answer through \
         compute_stale_set and said {{0}} while core dropped {{0, 1}}."
    );
    assert_eq!(
        effects.revision,
        Some(s.toolpath_revision(0)),
        "Effects::revision names the edited toolpath's own revision. \
         It is `Some` because the command names one toolpath that still \
         sits at that index."
    );
}
