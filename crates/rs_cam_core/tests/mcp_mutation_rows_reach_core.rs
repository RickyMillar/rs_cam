//! WP4 — every MCP mutation is a `Command` row that reaches core and
//! reports its own [`Effects`].
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP4 and §15.
//!
//! Before WP4 the MCP server carried one `McpRequestKind` variant per
//! mutation. Each variant's arm called a session setter, then RE-DERIVED
//! the stale set through `compute_stale_set`. Two producers answered one
//! question, and N15 measured them disagreeing. WP4 moves every mutation
//! onto one registry row with a core payload, so `apply` is the one
//! producer.
//!
//! This file measures the core half of that move. It takes six rows, one
//! per payload shape:
//!
//! - `SetToolpathHeights` — a plain toolpath setter;
//! - `SetStockConfig` — a whole-project setter that drops every result;
//! - `SetMachine` — a whole-profile setter that drops NO result;
//! - `AddToolpath` — an add, which reports [`Effects::created`];
//! - `SetSetupModels` — a setup setter that drops one setup's results;
//! - `SetToolpathDebugOptions` — a setter that moves no revision at all.
//!
//! Each arm asserts `apply(row)?.stale` against the OBSERVED set, in the
//! `mutation_paths_invalidate_alike_p0.rs` idiom: read every revision
//! before, run the command, and walk the results and the revisions
//! after. An expectation written as a literal set would only restate the
//! setter's code; an observation measures it.
//!
//! NOT MEASURED here: the wire. Whether the MCP server hands these rows
//! to `apply` is a viz-side question, and
//! `crates/rs_cam_viz/tests/command_registry_surfaces.rs` scans
//! `mcp_server.rs` for the tool name of every row whose `mcp` reach is
//! `Reached`.

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
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::compute::stock_config::{ModelId, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, Command, CommandId, CommandKind, LoadedModel, ProjectSession,
    Reach, SetMachineArgs, SetSetupModelsArgs, SetStockConfigArgs, SetToolpathDebugOptionsArgs,
    SetToolpathHeightsArgs, ToolpathConfig,
};

/// The clearance height the heights arm writes.
const EDITED_CLEARANCE_Z_MM: f64 = 12.5;

/// The stock width the stock arm writes, in millimetres.
const EDITED_STOCK_X_MM: f64 = 321.0;

/// The name the machine arm writes.
const EDITED_MACHINE_NAME: &str = "WP4 bench router";

/// The smallest number of MCP mutation rows the registry may declare.
///
/// MEASURED, not chosen: the WP4 brief's row table lists 32 MCP
/// mutations. Three stay hand-written holdouts (`apply_feeds`,
/// `plan_multitool_finishing`, `export_gcode`, §15 ruling 4) and
/// `set_toolpath_param` shipped in WP1, so WP4 adds 28 and the registry
/// carries 29. The bar sits one row below that count so a later package
/// that retires one row does not fail an arithmetic assertion, and far
/// enough above zero that an empty registry cannot pass.
const MIN_MCP_MUTATION_ROWS: usize = 28;

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
/// This is the P0 fixture of
/// `crates/rs_cam_core/tests/command_registry_completeness.rs`. Index 0
/// is a `Fresh` Pocket. Index 1 is a Rest that reads the stock index 0
/// leaves. Both carry a cached result.
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
}

fn revisions(s: &ProjectSession) -> Vec<u64> {
    (0..s.toolpath_count())
        .map(|i| s.toolpath_revision(i))
        .collect()
}

/// The indices whose cached result is gone, and the indices whose
/// revision moved.
fn observe(s: &ProjectSession, before: &[u64]) -> (BTreeSet<usize>, BTreeSet<usize>) {
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
    (dropped, bumped)
}

// ── the six rows ─────────────────────────────────────────────────

/// A plain toolpath setter reports the chain it dropped.
#[test]
fn the_heights_row_reports_the_chain_it_dropped() {
    let mut s = fixture();
    let before = revisions(&s);
    let heights = HeightsConfig {
        clearance_z: HeightMode::Manual(EDITED_CLEARANCE_Z_MM),
        ..HeightsConfig::default()
    };

    let effects = s
        .apply(Command::SetToolpathHeights(SetToolpathHeightsArgs {
            index: 0,
            heights,
        }))
        .expect("index 0 names a toolpath");

    let (dropped, bumped) = observe(&s, &before);
    assert!(
        !bumped.is_empty(),
        "a heights edit must move at least one revision, or this arm \
         measures nothing"
    );
    assert_eq!(
        effects.stale, bumped,
        "Effects::stale is the revision-moved set"
    );
    assert_eq!(
        effects.stale, dropped,
        "the setter walks the stock chain, so the moved set and the \
         dropped set agree here"
    );
    assert_eq!(
        effects.created, None,
        "a setter creates nothing; None means NOT CREATED"
    );
}

/// The stock row drops every toolpath result (G-FRESHSTATE).
#[test]
fn the_stock_row_drops_every_result() {
    let mut s = fixture();
    let before = revisions(&s);
    let stock = StockConfig {
        x: EDITED_STOCK_X_MM,
        ..StockConfig::default()
    };

    let effects = s.apply(Command::SetStockConfig(SetStockConfigArgs {
        stock: Box::new(stock),
    }));
    let effects = effects.expect("the stock row refuses nothing");

    let (dropped, bumped) = observe(&s, &before);
    assert_eq!(
        dropped,
        [0, 1].into_iter().collect::<BTreeSet<usize>>(),
        "a stock edit stales EVERY toolpath"
    );
    assert_eq!(effects.stale, bumped, "Effects::stale is the moved set");
    assert_eq!(effects.stale, dropped, "and here it is the dropped set");
    assert!(
        (s.stock_config().x - EDITED_STOCK_X_MM).abs() < 1e-9,
        "the row must write the stock it carries"
    );
}

/// The machine row writes the profile and drops NO toolpath result.
///
/// `invalidate_machine` clears the simulation alone: a machine edit
/// moves no geometry. The empty `stale` set here is the claim, and the
/// name assertion is what stops the arm passing on a row that wrote
/// nothing.
#[test]
fn the_machine_row_drops_no_toolpath_result() {
    let mut s = fixture();
    let before = revisions(&s);
    let machine = MachineProfile {
        name: EDITED_MACHINE_NAME.to_owned(),
        ..MachineProfile::default()
    };

    let effects = s
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");

    let (dropped, bumped) = observe(&s, &before);
    assert_eq!(
        s.machine().name,
        EDITED_MACHINE_NAME,
        "the row must write the profile it carries"
    );
    assert!(
        dropped.is_empty(),
        "a machine edit moves no geometry, so it drops no result"
    );
    assert_eq!(effects.stale, bumped, "Effects::stale is the moved set");
    assert!(effects.stale.is_empty(), "and that set is empty here");
}

/// The add row reports the index it created.
#[test]
fn the_add_toolpath_row_reports_the_new_index() {
    let mut s = fixture();
    let before = revisions(&s);
    let tool = s.tools()[0].id.0;
    let model = s.models()[0].id;
    let added = tc(
        "added",
        OperationConfig::Pocket(PocketConfig::default()),
        StockSource::Fresh,
        tool,
        model,
    );

    let effects = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(added),
        }))
        .expect("setup 0 exists");

    assert_eq!(
        effects.created,
        Some(2),
        "the add reports the index of the toolpath it appended"
    );
    assert_eq!(s.toolpath_count(), 3, "the append must land");
    let (_dropped, bumped) = observe(&s, &before);
    assert_eq!(
        effects.stale, bumped,
        "Effects::stale is the moved set. An append moves the new index \
         alone: the snapshot carried no revision for it."
    );
    assert!(
        s.get_result(0).is_some() && s.get_result(1).is_some(),
        "an append drops no existing result"
    );
}

/// The setup-models row drops every result in that setup.
#[test]
fn the_setup_models_row_drops_the_setup() {
    let mut s = fixture();
    let before = revisions(&s);
    let model = ModelId(s.models()[0].id);

    let effects = s
        .apply(Command::SetSetupModels(SetSetupModelsArgs {
            setup_index: 0,
            model_ids: vec![model],
        }))
        .expect("setup 0 exists");

    let (dropped, bumped) = observe(&s, &before);
    assert_eq!(
        s.list_setups()[0].model_ids,
        vec![model],
        "the row must write the scope it carries"
    );
    assert_eq!(
        dropped,
        [0, 1].into_iter().collect::<BTreeSet<usize>>(),
        "both toolpaths sit in setup 0, so both lose their result"
    );
    assert_eq!(effects.stale, bumped, "Effects::stale is the moved set");
    assert_eq!(effects.stale, dropped, "and here it is the dropped set");
}

/// The debug-options row writes the flag and moves no revision.
///
/// The generate door writes `debug_options` immediately before it runs,
/// so a revision move here would stale the very toolpath the caller is
/// about to generate.
#[test]
fn the_debug_options_row_moves_no_revision() {
    let mut s = fixture();
    let before = revisions(&s);

    let effects = s
        .apply(Command::SetToolpathDebugOptions(
            SetToolpathDebugOptionsArgs {
                index: 0,
                debug_options: ToolpathDebugOptions { enabled: true },
            },
        ))
        .expect("index 0 names a toolpath");

    let (dropped, bumped) = observe(&s, &before);
    assert!(
        s.toolpath_configs()[0].debug_options.enabled,
        "the row must write the option it carries"
    );
    assert!(
        bumped.is_empty(),
        "the debug flag is not a generation input, so no revision moves"
    );
    assert!(dropped.is_empty(), "and no cached result is dropped either");
    assert!(effects.stale.is_empty(), "Effects::stale reports that");
}

// ── the registry ─────────────────────────────────────────────────

/// The registry declares the MCP mutation section, and every row of it
/// carries a wire name.
///
/// The count bar is the non-vacuity guard: an empty population passes
/// every per-row assertion and looks healthy.
#[test]
fn the_registry_declares_every_mcp_mutation() {
    let mut mcp_mutations = 0_usize;
    for id in CommandId::ALL {
        if id.kind() != CommandKind::Command {
            continue;
        }
        if !matches!(id.surfaces().mcp, Reach::Reached) {
            continue;
        }
        assert!(
            !id.wire_name().is_empty(),
            "{id:?} reaches MCP with no wire name, so no tool can name it"
        );
        mcp_mutations += 1;
    }
    assert!(
        mcp_mutations >= MIN_MCP_MUTATION_ROWS,
        "the registry declares {mcp_mutations} MCP mutation rows, below \
         the measured floor of {MIN_MCP_MUTATION_ROWS}"
    );
}
