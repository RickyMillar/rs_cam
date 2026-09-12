//! WP8 — `RestoreToolpathSnapshot` invalidates what the setter
//! invalidates, and a drill pick does too (N14, N6).
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP8 and §17 "WP8 rulings".
//!
//! Two rows close here. Both describe one defect: an edit that changes
//! what the machine cuts dropped the edited toolpath's own result and
//! left the downstream `StockSource::FromRemainingStock` result cached.
//! The downstream row then answered a stock that no longer exists.
//!
//! - **N14.** The GUI undo stack, the GUI redo stack and the three
//!   optimizer apply paths all ended in `apply_toolpath_param_snapshot`,
//!   which called `drop_result` plus `simulation = None`. The four GUI
//!   call sites now take `Command::RestoreToolpathSnapshot`, which calls
//!   `invalidate_result_chain`.
//! - **N6.** `set_drill_selected_holes` had the same narrow ending, while
//!   its sibling `set_alignment_pin_drill_holes`, 30 lines away in the
//!   same file, walked the chain. A drill pick changes which holes the
//!   operation cuts, so it changes the stock the downstream row reads.
//!
//! The core optimizer keeps a narrow path on purpose — see
//! `apply_toolpath_param_snapshot_narrow`'s own doc. This file does not
//! reach it: `pub(crate)` keeps it inside the crate. The in-crate test
//! `the_narrow_path_leaves_the_neighbour_result_cached`
//! (`session/mutation.rs`) pins that half.
//!
//! The fixture is the one `mutation_paths_invalidate_alike_p0.rs` uses:
//! one setup, two enabled toolpaths in plan order, index 1 reading the
//! stock index 0 leaves, both carrying a cached result.
//!
//! NOT MEASURED: `Effects::simulation_cleared`. The `simulation` field is
//! private and the crate publishes no setter, so an integration test
//! cannot seed a simulation. Every arm here reads the `false` case only,
//! and that is not evidence about the `true` one.

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
use rs_cam_core::compute::operation_configs::{DrillConfig, PocketConfig, RestConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::feeds::{FeedsProvenance, ValueProvenance};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptResultArgs, Command, Effects, LoadedModel, ProjectSession, ProjectSessionBuilder,
    RestoreToolpathSnapshotArgs, SetDrillSelectedHolesArgs, ToolpathConfig,
};

/// The one feed value the arms write.
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
        feeds_provenance: FeedsProvenance::default(),
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

/// One setup, one tool, one model, two enabled toolpaths in plan order.
///
/// - index 0 carries the operation the caller names, on `Fresh` stock.
/// - index 1 is a Rest on `FromRemainingStock`.
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

/// The non-vacuity guard. A dropped-set assertion proves nothing unless
/// both results exist first, and unless the downstream row really reads
/// the stock the upstream row leaves.
fn assert_fixture_is_live(s: &ProjectSession) {
    assert!(s.get_result(0).is_some(), "index 0 needs a cached result");
    assert!(s.get_result(1).is_some(), "index 1 needs a cached result");
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

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

/// Restore index 0's own current triple, with one operation swapped in.
///
/// The caller names the operation, so an arm can restore a changed one or
/// a byte-identical one through the same helper.
fn restore(
    s: &mut ProjectSession,
    operation: OperationConfig,
    feeds_provenance: Option<FeedsProvenance>,
) -> Effects {
    let dressups = s.toolpath_configs()[0].dressups.clone();
    let face_selection = s.toolpath_configs()[0].face_selection.clone();
    s.apply(Command::RestoreToolpathSnapshot(
        RestoreToolpathSnapshotArgs {
            index: 0,
            operation: Box::new(operation),
            dressups: Box::new(dressups),
            face_selection,
            feeds_provenance: feeds_provenance.map(Box::new),
        },
    ))
    .expect("index 0 exists")
}

// ── (a) N14 — the restore walks the chain ────────────────────────

/// N14 closed. The command drops the edited row AND the row that reads
/// the stock it leaves, which is the set `set_toolpath_param` drops.
#[test]
fn the_restore_drops_the_set_the_setter_drops_n14() {
    let mut s = fixture(OperationConfig::Pocket(PocketConfig::default()));
    let mut operation = s.toolpath_configs()[0].operation.clone();
    operation.set_feed_rate(EDITED_FEED_RATE);

    let effects = restore(&mut s, operation, None);

    let feed = s.toolpath_configs()[0].operation.feed_rate();
    assert!(
        (feed - EDITED_FEED_RATE).abs() < 1e-9,
        "the command must write the operation, or it invalidates on a \
         refusal and measures nothing. I read {feed}"
    );
    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "N14: undo, redo and the optimizer apply used to drop {{0}} \
         alone, so the downstream Rest answered a stock that had moved. \
         The command now walks the same chain the setter walks."
    );
    assert!(
        s.get_result(0).is_none() && s.get_result(1).is_none(),
        "Effects::stale must name results that really went away"
    );
    assert_eq!(
        effects.revision,
        Some(s.toolpath_revision(0)),
        "the command names one toolpath, so it reports that toolpath's \
         own revision"
    );
}

// ── (b) F2.5 — a byte-identical restore still drops ──────────────

/// The restore is unconditional by contract. A signature gate here would
/// break F2.5.
///
/// Nothing records which configuration a cached result answers. The
/// snapshot restores three of the nine fields that decide the geometry,
/// so an undo that puts back the exact triple the retained geometry was
/// generated from still cannot claim the geometry is current. Being
/// wrong this way costs a regeneration. Being wrong the other way
/// exports a program that does not match the project.
#[test]
fn a_byte_identical_restore_still_drops_the_chain_f2_5() {
    let mut s = fixture(OperationConfig::Pocket(PocketConfig::default()));
    let unchanged = s.toolpath_configs()[0].operation.clone();

    let effects = restore(&mut s, unchanged.clone(), None);

    assert_eq!(
        s.toolpath_configs()[0].operation.feed_rate(),
        unchanged.feed_rate(),
        "the restore put back the configuration it was handed"
    );
    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "F2.5: the restore drops the chain whether or not the \
         configuration moved. `controller/tests.rs` \
         `an_undone_param_edit_does_not_resurrect_the_old_result` pins \
         the GUI half of the same ruling."
    );
    assert!(
        s.get_result(0).is_none() && s.get_result(1).is_none(),
        "and the results really went away"
    );
}

// ── (c) the payload carries the feeds provenance ─────────────────

/// `Some` provenance lands on the configuration.
///
/// Three optimizer paths used to call `set_feeds_provenance` right after
/// the snapshot, so an undo restored the pre-optimizer values and left
/// the `Optimizer` stamp standing on them. The payload carries the stamp
/// now, and one command writes both.
#[test]
fn the_payload_writes_the_feeds_provenance() {
    let mut s = fixture(OperationConfig::Pocket(PocketConfig::default()));
    let operation = s.toolpath_configs()[0].operation.clone();
    let provenance = FeedsProvenance {
        feed_rate: Some(ValueProvenance::optimizer()),
        ..FeedsProvenance::default()
    };

    let _ = restore(&mut s, operation.clone(), Some(provenance.clone()));

    assert_eq!(
        s.toolpath_configs()[0].feeds_provenance,
        provenance,
        "the command writes the provenance the payload carries"
    );

    // `None` means the caller restores no provenance. The toolpath keeps
    // what it has; the field is not cleared.
    let _ = restore(&mut s, operation, None);
    assert_eq!(
        s.toolpath_configs()[0].feeds_provenance,
        provenance,
        "a `None` payload writes nothing, and never clears the stamp"
    );
}

// ── (d) N6 — a drill pick walks the chain ────────────────────────

/// N6 closed. A drill pick changes which holes the operation cuts, so it
/// changes the stock the downstream row inherits.
///
/// `set_alignment_pin_drill_holes` already walked the chain. The two
/// setters sit 30 lines apart in one file and gave opposite answers to
/// two edits of the same kind.
#[test]
fn a_drill_pick_drops_the_chain_n6() {
    let mut s = fixture(OperationConfig::Drill(DrillConfig::default()));

    let effects = s
        .apply(Command::SetDrillSelectedHoles(SetDrillSelectedHolesArgs {
            index: 0,
            selected_holes: Some(vec![[1.0, 2.0]]),
        }))
        .expect("index 0 is a Drill operation");

    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "N6: the drill-pick setter used to drop {{0}} alone. It now \
         answers what set_alignment_pin_drill_holes answers."
    );
    assert!(
        s.get_result(0).is_none() && s.get_result(1).is_none(),
        "Effects::stale must name results that really went away"
    );
}
