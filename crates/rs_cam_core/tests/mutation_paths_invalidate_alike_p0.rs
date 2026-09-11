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
//! | 3 | feeds Apply funnel | a direct field write, then `invalidate_toolpath_inputs` | WIDE |
//! | 4 | wholesale config replacement | `ProjectSession::apply` -> `Command::ReplaceToolpathConfig` | WIDE |
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
//! - `the_setter_the_inspector_door_and_the_replacement_agree`: arms 1, 3
//!   and 4 drop the same set, `{0, 1}`.
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
//! PINNED DIVERGENCE — each of these asserts TODAY's answer. Each names
//! the intended answer and the phase that inverts it. `PLAN.md:111`
//! forbids a test that freezes a defect as desired behaviour, so no
//! assertion here states a divergence as the contract:
//!
//! - `n15_compute_stale_set_still_reports_one_index_pinned_divergence`
//!   — the narrow answer `compute_stale_set` gives. WP1 took the
//!   `set_toolpath_param` MCP arm off that function; WP3 and WP4 move
//!   the remaining arms.
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
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, MutationKind, ProjectSession, ReplaceToolpathConfigArgs,
    RestoreToolpathSnapshotArgs, SetToolpathParamArgs, ToolpathConfig, compute_stale_set,
};

/// The one feed value every arm writes.
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
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let _ = s.add_model(empty_model("part.svg"));
    let tool = s.tools()[0].id.0;
    let model = s.models()[0].id;
    let upstream_tc = tc("upstream", upstream, StockSource::Fresh, tool, model);
    let downstream_tc = tc(
        "downstream",
        OperationConfig::Rest(RestConfig::default()),
        StockSource::FromRemainingStock,
        tool,
        model,
    );
    let _ = s.add_toolpath(0, upstream_tc).unwrap();
    let _ = s.add_toolpath(0, downstream_tc).unwrap();
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    assert_fixture_is_live(&s);
    s
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig::default())
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

/// Arm 1 — `ProjectSession::set_toolpath_param`, the MCP and CLI door.
fn arm_setter() -> Observation {
    let mut s = fixture(pocket());
    let before = revisions(&s);
    let _ = s
        .set_toolpath_param(0, "feed_rate", serde_json::json!(EDITED_FEED_RATE))
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

/// Arm 3 — a direct field write, then the public door
/// `ProjectSession::invalidate_toolpath_inputs`.
///
/// The GUI inspector took this door until WP5; the feeds Apply funnel
/// takes it now (N13). The arm stays raw on purpose. Pointing it at
/// `Command::ReplaceToolpathConfig` would make the comparison below read
/// arm 4 against arm 4, and this file's non-vacuity guard exists to stop
/// exactly that.
fn arm_inspector_door() -> Observation {
    let mut s = fixture(pocket());
    let before = revisions(&s);
    let configs = s.toolpath_configs_mut();
    configs[0].operation.set_feed_rate(EDITED_FEED_RATE);
    let _ = s.invalidate_toolpath_inputs(0);
    assert_feed_landed(&s);
    observe(&s, &before)
}

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

/// CONTRACT. Three wide paths, one logical edit, one invalidated set.
#[test]
fn the_setter_the_inspector_door_and_the_replacement_agree() {
    let setter = arm_setter();
    let inspector = arm_inspector_door();
    let replacement = arm_replace_config();

    assert_eq!(
        setter.dropped,
        set(&[0, 1]),
        "set_toolpath_param walks the stock chain. The edited row and the \
         downstream FromRemainingStock row both go stale."
    );
    assert_eq!(
        inspector.dropped, setter.dropped,
        "invalidate_toolpath_inputs is the feeds funnel's door onto the \
         same chain walk. The two must not diverge."
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
        ("invalidate_toolpath_inputs", arm_inspector_door()),
        ("Command::ReplaceToolpathConfig", arm_replace_config()),
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
        .set_drill_selected_holes(0, Some(vec![[1.0, 2.0]]))
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
        .set_alignment_pin_drill_holes(0, vec![[1.0, 2.0]])
        .expect("index 0 is an AlignmentPinDrill operation");
    let obs = observe(&s, &before);

    assert_eq!(
        obs.dropped,
        set(&[0, 1]),
        "the pin-hole setter walks the chain. This is the answer the \
         drill-pick setter above reaches too, since WP8."
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

/// PINNED DIVERGENCE — N15. `compute_stale_set` keeps the narrow answer.
///
/// `compute_stale_set` is the OTHER staleness model. WP1 took the
/// `set_toolpath_param` arm of the MCP reply off it, but the function
/// itself is unchanged and other MCP arms still call it. WP3 and WP4
/// move those arms onto the command door. This assertion inverts there.
///
/// WP1 deleted `compute_stale_set_for_toolpath_param_returns_single_toolpath`
/// in `session/compute.rs`, which pinned the same narrow answer as a
/// unit test. That test was vacuous-green: its session held one `Fresh`
/// toolpath, so a chain-aware answer is also `[0]`. This fixture holds a
/// chain, so the narrow answer here is a real divergence.
#[test]
fn n15_compute_stale_set_still_reports_one_index_pinned_divergence() {
    let s = fixture(pocket());
    let mutation = MutationKind::ToolpathParamChanged { toolpath_index: 0 };
    let reported = compute_stale_set(&s, mutation);
    assert_eq!(
        reported.toolpath_indices,
        vec![0],
        "N15: compute_stale_set(ToolpathParamChanged) returns exactly one \
         index (`compute.rs:53` `compute_stale_set`), with no chain walk, \
         while the setter on this fixture drops {{0, 1}}."
    );
}
