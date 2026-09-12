//! WP5 — `ReplaceToolpathConfig` gates its invalidation on the
//! generation-inputs signature.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP5 and §17 "WP5 rulings".
//!
//! The GUI inspector holds no commit event. It rebuilds an owned entry,
//! lets the widgets write it, and writes the entry back on EVERY frame
//! the panel is open. So the door it takes runs on every frame too, and
//! the door — not the panel — decides whether the cached geometry
//! survives.
//!
//! The rule this file pins has two halves, and they are independent:
//!
//! - **The command always writes the configuration.** Name, coolant and
//!   the pre and post G-code sit OUTSIDE the signature: they dirty the
//!   project and change no motion. An early return before the write would
//!   stop them landing, and no other test covers them.
//! - **The command invalidates only when the signature moved.** The
//!   signature is
//!   [`rs_cam_core::session::ToolpathConfig::generation_inputs_signature`],
//!   the one definition. Before WP5 the viz panel held a copy of it and
//!   `ProjectSession::replace_toolpath_config` invalidated
//!   unconditionally, so a panel that applied every frame would have
//!   dropped every cached result every frame.
//!
//! The third arm pins the boundary between the command and its caller.
//! `id`, `boundary_inherit` and `planner_origin` are fields the inspector
//! entry cannot supply. The COMMAND does not protect them — it writes
//! whatever the caller passed. The viz projection protects them, by
//! cloning the stored configuration first
//! (`ui/properties/mod.rs::project_entry_onto`), and
//! `crates/rs_cam_viz/src/controller/tests.rs` pins that half.
//!
//! RED before WP5: the registry declares no `ReplaceToolpathConfig` row,
//! and `ToolpathConfig` carries no `generation_inputs_signature` method,
//! so this file does not compile.

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
    AdoptResultArgs, Command, LoadedModel, PlannerOrigin, ProjectSession, ProjectSessionBuilder,
    ReplaceToolpathConfigArgs, ToolpathConfig,
};

/// The feed value the signature-moving arm writes.
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
/// - index 0, `upstream`, `StockSource::Fresh`, a Pocket. Every arm
///   replaces this row.
/// - index 1, `downstream`, a Rest with `StockSource::FromRemainingStock`.
///   A chain walk drops this result. A gated replacement keeps it.
///
/// Both rows carry a cached result before the caller replaces anything.
fn fixture() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
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
    let _ = builder.add_toolpath(0, upstream).unwrap();
    let _ = builder.add_toolpath(0, downstream).unwrap();
    let mut s = builder.build();
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    assert_fixture_is_live(&s);
    s
}

/// The non-vacuity guard. A kept-result assertion proves nothing unless
/// both results exist first, and a dropped-set assertion proves nothing
/// unless the downstream row really depends on the upstream one.
fn assert_fixture_is_live(s: &ProjectSession) {
    assert!(
        s.get_result(0).is_some(),
        "index 0 must carry a cached result before the replacement"
    );
    assert!(
        s.get_result(1).is_some(),
        "index 1 must carry a cached result before the replacement"
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

fn set(indices: &[usize]) -> BTreeSet<usize> {
    indices.iter().copied().collect()
}

/// Replace index 0 and report the effects.
fn replace(s: &mut ProjectSession, config: ToolpathConfig) -> rs_cam_core::session::Effects {
    s.apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
        index: 0,
        config: Box::new(config),
    }))
    .expect("index 0 exists")
}

// ── the three arms ───────────────────────────────────────────────

/// Arm (a). The four fields outside the signature land, and nothing goes
/// stale.
///
/// This is the frame the panel spends most of its life in: the operator
/// types in the Name box, or picks a coolant mode, and the generated
/// geometry is still the answer to every input that decides it.
#[test]
fn a_replacement_outside_the_signature_lands_and_drops_nothing() {
    let mut s = fixture();
    let revision_before = s.toolpath_revision(0);
    let signature_before = s.toolpath_configs()[0].generation_inputs_signature();

    let mut edited = s.toolpath_configs()[0].clone();
    edited.name = "renamed".to_owned();
    edited.coolant = CoolantMode::Flood;
    edited.pre_gcode = Some("M3 S18000".to_owned());
    edited.post_gcode = Some("M5".to_owned());
    assert_eq!(
        edited.generation_inputs_signature(),
        signature_before,
        "the arm must move no generation input. Otherwise it measures \
         the other half of the gate."
    );

    let effects = replace(&mut s, edited);

    assert_eq!(
        effects.stale,
        BTreeSet::new(),
        "name, coolant and the pre and post G-code change no motion, so \
         the command must drop nothing"
    );
    assert!(
        s.get_result(0).is_some(),
        "the replaced row keeps the geometry its inputs still answer for"
    );
    assert!(
        s.get_result(1).is_some(),
        "the downstream row keeps its result too. A chain walk here would \
         cost a full regeneration for a rename."
    );
    assert_eq!(
        effects.revision,
        Some(revision_before),
        "the generation-input revision must not move when no generation \
         input moved"
    );

    let stored = &s.toolpath_configs()[0];
    assert_eq!(stored.name, "renamed", "the name must land");
    assert_eq!(stored.coolant, CoolantMode::Flood, "the coolant must land");
    assert_eq!(
        stored.pre_gcode.as_deref(),
        Some("M3 S18000"),
        "the pre G-code must land"
    );
    assert_eq!(
        stored.post_gcode.as_deref(),
        Some("M5"),
        "the post G-code must land"
    );
}

/// Arm (b). A moved generation input drops the row and the chain below
/// it.
#[test]
fn a_replacement_that_moves_the_signature_drops_the_chain() {
    let mut s = fixture();
    let signature_before = s.toolpath_configs()[0].generation_inputs_signature();

    let mut edited = s.toolpath_configs()[0].clone();
    edited.operation.set_feed_rate(EDITED_FEED_RATE);
    assert_ne!(
        edited.generation_inputs_signature(),
        signature_before,
        "the arm must move a generation input. Otherwise it measures the \
         other half of the gate."
    );

    let effects = replace(&mut s, edited);

    assert_eq!(
        effects.stale,
        set(&[0, 1]),
        "the operation is a generation input, so the command walks the \
         stock chain: the edited row and the downstream \
         FromRemainingStock row both go stale"
    );
    assert!(
        s.get_result(0).is_none(),
        "the core must not keep geometry the moved inputs no longer \
         answer for"
    );
    assert!(
        s.get_result(1).is_none(),
        "the downstream row reads the stock the edited row leaves"
    );
    let feed = s.toolpath_configs()[0].operation.feed_rate();
    assert!(
        (feed - EDITED_FEED_RATE).abs() < 1e-9,
        "the feed must land. Otherwise the arm invalidates for a reason \
         this file does not measure. I read {feed}"
    );
}

/// Arm (c). The command writes `id`, `boundary_inherit` and
/// `planner_origin` verbatim.
///
/// It protects none of the three. The viz projection does, by cloning the
/// stored configuration before it applies the sixteen entry fields. A
/// command that silently kept the stored values would hide a caller that
/// built a fresh literal, which is the defect class G-BOUNDARYINHERIT
/// names.
#[test]
fn the_command_writes_the_three_unsupplied_fields_verbatim() {
    let mut s = fixture();
    let stored_id = s.toolpath_configs()[0].id;
    assert!(
        s.toolpath_configs()[0].boundary_inherit,
        "the fixture must start with boundary_inherit true, or the flip \
         below asserts nothing"
    );
    assert!(
        s.toolpath_configs()[0].planner_origin.is_none(),
        "the fixture must start hand-owned, or the stamp below asserts \
         nothing"
    );

    let origin = PlannerOrigin {
        plan_id: 7,
        tier: 1,
        tier_count: 2,
    };
    let mut edited = s.toolpath_configs()[0].clone();
    edited.id = ToolpathId(4242);
    edited.boundary_inherit = false;
    edited.planner_origin = Some(origin.clone());

    let effects = replace(&mut s, edited);

    let stored = &s.toolpath_configs()[0];
    assert_eq!(
        stored.id,
        ToolpathId(4242),
        "the command writes the id the caller passed. It protects \
         nothing; the projection does."
    );
    assert_ne!(
        stored.id, stored_id,
        "the guard above must be live: the arm passes a different id"
    );
    assert!(
        !stored.boundary_inherit,
        "the command writes boundary_inherit verbatim"
    );
    assert_eq!(
        stored.planner_origin.as_ref(),
        Some(&origin),
        "the command writes planner_origin verbatim"
    );
    assert_eq!(
        effects.stale,
        BTreeSet::new(),
        "none of the three is a generation input, so the command drops \
         nothing"
    );
}

/// The registry row declares the surfaces WP5 gives it.
#[test]
fn the_row_declares_its_surfaces() {
    use rs_cam_core::session::{CommandId, CommandKind, Reach};

    assert_eq!(
        CommandId::ReplaceToolpathConfig.wire_name(),
        "replace_toolpath_config",
        "the wire name is the row's operator-visible identity"
    );
    assert_eq!(
        CommandId::ReplaceToolpathConfig.kind(),
        CommandKind::Command,
        "a replacement mutates the session"
    );
    let surfaces = CommandId::ReplaceToolpathConfig.surfaces();
    assert_eq!(
        surfaces.gui,
        Reach::Reached,
        "the GUI inspector is the caller WP5 adds"
    );
    assert!(
        matches!(surfaces.mcp, Reach::Skip(_)),
        "the MCP door edits one named parameter, so it reaches another row"
    );
    assert_eq!(
        surfaces.cli,
        Reach::Reached,
        "the CLI project door writes a toolpath's configuration through this row (WP6b)"
    );
}
