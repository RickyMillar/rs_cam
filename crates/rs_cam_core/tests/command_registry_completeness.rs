//! WP1 — the command registry is complete, and `apply` reports the set
//! the setter dropped.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §1 and §4 WP1.
//!
//! The registry is one X-macro list. The compiler guarantees exactly two
//! things about it: a `Surfaces` literal that omits a field does not
//! compile, and a `match` over `CommandId` that omits a variant does not
//! compile. Everything else is test-time, and this file is that test:
//!
//! - every wire name is non-empty, unique, and snake_case;
//! - every `Reach::Skip` carries a reason;
//! - `Command::id()` answers the identifier of the row it carries;
//! - `CommandId::ALL` holds one entry per `Command` variant. The count
//!   comes from a second callback on the SAME list, so the two cannot
//!   drift.
//!
//! The behavioural half measures `Effects`. `apply` reads the revision
//! map before and after the mutation, so `Effects::stale` must equal the
//! set of indices whose result the chain dropped — `{0, 1}` on a fixture
//! whose second toolpath reads the stock the first one leaves. Before
//! WP1 the MCP reply re-derived a narrower answer through
//! `compute_stale_set`, which reported `{0}` (N15).
//!
//! NOT MEASURED: the `true` case of `Effects::simulation_cleared`. The
//! `simulation` field is private and the crate publishes no setter, so
//! an integration test cannot seed a simulation. This file asserts the
//! `false` case only, and a `false` reading here is not evidence about
//! the `true` one.

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
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptResultArgs, Command, CommandId, CommandKind, LoadedModel, ProjectSession, Reach,
    SetToolpathParamArgs, ToolpathConfig,
};

/// The feed value the behavioural arm writes.
const EDITED_FEED_RATE: f64 = 4321.0;

/// Counts the rows `for_each_command!` declares.
///
/// The callback never expands a row's contents, so the row columns need
/// no name in this crate's scope.
macro_rules! declare_registry_row_count {
    ($( ( $($row:tt)* ) ),+ $(,)?) => {
        /// The number of rows the registry list declares.
        const DECLARED_ROWS: usize = [$(stringify!($($row)*)),+].len();
    };
}
rs_cam_core::for_each_command!(declare_registry_row_count);

// ── registry shape ───────────────────────────────────────────────

/// Every wire name is non-empty, unique across the registry, and
/// snake_case ASCII.
///
/// A wire name is the operator-visible identity of a command. Two rows
/// that share one name make a dispatch table answer the wrong row.
#[test]
fn every_wire_name_is_unique_and_snake_case() {
    assert!(
        !CommandId::ALL.is_empty(),
        "the registry declares no command, so every assertion here is \
         vacuous"
    );

    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for id in CommandId::ALL {
        let name = id.wire_name();
        assert!(!name.is_empty(), "{id:?} carries an empty wire name");
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "{id:?}'s wire name '{name}' is not snake_case ASCII"
        );
        assert!(
            !name.starts_with('_') && !name.ends_with('_'),
            "{id:?}'s wire name '{name}' starts or ends with an underscore"
        );
        assert!(
            seen.insert(name),
            "two registry rows carry the wire name '{name}'"
        );
    }
    assert_eq!(
        seen.len(),
        CommandId::ALL.len(),
        "CommandId::ALL contains duplicates"
    );
}

/// Every declared skip says why the surface does not reach the command.
///
/// An empty reason is the defect class the Overlays registry audit
/// found: a refusal a reader cannot act on.
#[test]
fn every_skipped_surface_carries_a_reason() {
    for id in CommandId::ALL {
        let surfaces = id.surfaces();
        for (surface, reach) in [
            ("gui", surfaces.gui),
            ("mcp", surfaces.mcp),
            ("cli", surfaces.cli),
        ] {
            if let Reach::Skip(reason) = reach {
                assert!(
                    !reason.trim().is_empty(),
                    "{id:?} skips the {surface} surface with no reason"
                );
            }
        }
    }
}

/// `CommandId::ALL` holds one entry per declared row.
///
/// The count comes from a second callback on the same list, so a row
/// added to one generated block and not the other fails here.
#[test]
fn all_holds_one_entry_per_declared_row() {
    assert_eq!(
        CommandId::ALL.len(),
        DECLARED_ROWS,
        "CommandId::ALL and the registry list disagree about how many \
         commands exist"
    );
}

/// A constructed command answers its own identifier, and that row
/// declares the kind the plan gives it.
#[test]
fn a_constructed_command_answers_its_identifier() {
    let command = Command::SetToolpathParam(SetToolpathParamArgs {
        index: 0,
        param: "feed_rate".to_owned(),
        value: serde_json::json!(EDITED_FEED_RATE),
    });
    assert_eq!(command.id(), CommandId::SetToolpathParam);
    assert_eq!(
        CommandId::SetToolpathParam.wire_name(),
        "set_toolpath_param",
        "the wire name is the MCP tool name, and the viz sentry scans \
         mcp_server.rs for it"
    );
    assert_eq!(
        CommandId::SetToolpathParam.kind(),
        CommandKind::Command,
        "a synchronous validated mutation is a Command, not a Query"
    );
    assert_eq!(
        CommandId::SetToolpathParam.surfaces().mcp,
        Reach::Reached,
        "MCP reaches this row; the viz sentry checks the tool exists"
    );
}

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

/// One setup, one tool, one model, two enabled toolpaths in plan order.
///
/// Index 0 is a `Fresh` Pocket. Index 1 is a Rest that reads the stock
/// index 0 leaves. Both carry a cached result. The fixture is the one
/// `mutation_paths_invalidate_alike_p0.rs` uses.
fn fixture() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    s.add_model(empty_model("part.svg"));
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
    s.add_toolpath(0, upstream).unwrap();
    s.add_toolpath(0, downstream).unwrap();
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

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

// ── Effects ──────────────────────────────────────────────────────

/// `apply` reports every index it dropped, and the revision of the
/// toolpath the command names.
#[test]
fn apply_reports_the_set_the_setter_dropped() {
    let mut s = fixture();
    let before = revisions(&s);
    assert!(
        s.simulation_result().is_none(),
        "the fixture must hold no simulation, or the false case below \
         asserts nothing"
    );

    let effects = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");

    let (dropped, bumped) = observe(&s, &before);
    assert_eq!(
        dropped,
        set(&[0, 1]),
        "the setter walks the stock chain. The edited row and the row \
         that reads its remaining stock both lose their result."
    );
    assert_eq!(
        effects.stale, dropped,
        "Effects::stale must equal the set whose cached result the \
         command dropped"
    );
    assert_eq!(
        effects.stale, bumped,
        "Effects::stale is built from the revision map, so it must \
         equal the set whose revision moved"
    );
    assert_eq!(
        effects.revision,
        Some(s.toolpath_revision(0)),
        "Effects::revision is the named toolpath's own revision, never \
         the session-global next_revision. It is `Some` because the \
         command names one toolpath that still sits at that index."
    );
    assert!(
        effects
            .revision
            .is_some_and(|revision| revision > before[0]),
        "the edit must move the named toolpath's revision"
    );
    assert!(
        !effects.simulation_cleared,
        "no simulation existed before the command, so the command \
         cleared none. The true case is NOT MEASURED — see the module \
         doc."
    );
}

/// The write lands. Otherwise the arm above invalidates on a refusal and
/// measures nothing.
#[test]
fn the_command_writes_the_parameter() {
    let mut s = fixture();
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");
    let feed = s.toolpath_configs()[0].operation.feed_rate();
    assert!(
        (feed - EDITED_FEED_RATE).abs() < 1e-9,
        "the command must write the feed; got {feed}"
    );
}

/// The legacy setter is a wrapper over the same door, so both routes
/// drop the same set.
#[test]
fn the_legacy_setter_drops_what_the_command_drops() {
    let mut through_setter = fixture();
    let before_setter = revisions(&through_setter);
    let _ = through_setter
        .set_toolpath_param(0, "feed_rate", serde_json::json!(EDITED_FEED_RATE))
        .expect("feed_rate is a Pocket parameter");
    let (setter_dropped, _) = observe(&through_setter, &before_setter);

    let mut through_command = fixture();
    let effects = through_command
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");

    assert_eq!(
        setter_dropped, effects.stale,
        "set_toolpath_param is a thin wrapper over apply; the two \
         routes cannot drop different sets"
    );
}

/// A refused command reports no effects.
#[test]
fn a_refused_command_returns_an_error() {
    let mut s = fixture();
    let outcome = s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
        index: 99,
        param: "feed_rate".to_owned(),
        value: serde_json::json!(EDITED_FEED_RATE),
    }));
    assert!(
        outcome.is_err(),
        "index 99 names no toolpath, so the command must refuse"
    );
    assert!(
        s.get_result(0).is_some() && s.get_result(1).is_some(),
        "a refusal drops no result"
    );
}
