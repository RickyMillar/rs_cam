//! End-to-end smoke test for the optimizer.
//!
//! The unit tests in `tool_load::optimize` cover the pieces (the
//! outcome builder, the narrative, the skip paths). This file runs the
//! real entry points against a generated and simulated toolpath.
//!
//! **Phase 1 contract** (`planning/optimize_review_2026-10-12/PLAN.md`
//! §1): the search has no levers. A run scores the baseline from the
//! trace, evaluates no candidate, and returns `NoSafeImprovement` with
//! candidate 0 = the baseline and the "no search" explanation. The
//! guard restores the parameters. Phase 2 replaces these assertions.
//!
//! The fixture is the checked-in `fixtures/demo_pocket.svg`: a small
//! rectangle with a circular island. It parses fast and gives a
//! deterministic pocket toolpath. With no candidate to simulate, each
//! test costs one generation and one simulation at a 1 mm cell.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::clone_on_ref_ptr
)]

use rs_cam_core::ids::ToolpathId;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AddModelArgs, AddToolArgs, AddToolpathArgs, Command, LoadedModel, ProjectSession,
    SetStockConfigArgs, SimulationOptions, ToolpathConfig,
};
use rs_cam_core::tool_load::optimize::{
    NoProgress, OutcomeKind, SearchStage, optimize_project, optimize_toolpath,
};

/// The Phase 1 explanation. The GUI matches the same text.
const NO_SEARCH_EXPLANATION: &str =
    "The search has no levers yet; the post-sim tuner is being rebuilt.";
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// Build a session with the demo_pocket SVG, an end mill, and a
/// pocket op. Returns the session and the toolpath index. Returns
/// `None` if the fixture is missing.
fn build_pocket_session() -> Option<(ProjectSession, usize)> {
    let svg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join("demo_pocket.svg");
    if !svg_path.exists() {
        eprintln!("Skipping: fixture not found at {:?}", svg_path);
        return None;
    }
    let polygons =
        rs_cam_core::io::svg_input::load_svg(&svg_path, 0.1).expect("demo_pocket.svg should parse");
    if polygons.is_empty() {
        eprintln!("Skipping: demo_pocket.svg parsed empty");
        return None;
    }

    let mut session = ProjectSession::new_empty();

    // Set stock so the pocket has somewhere to cut. demo_pocket.svg
    // is in an 80mm viewbox; give it a 100mm × 100mm × 10mm stock so
    // pocket op clears within bounds.
    let mut stock = session.stock_config().clone();
    stock.x = 100.0;
    stock.y = 100.0;
    stock.z = 10.0;
    let _ = session
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock row refuses nothing");

    // Add a 6mm end mill (matches the optimize gate's typical wood
    // router setup). add_tool returns the vec index; we want the
    // assigned ToolId.0 for the toolpath link.
    let tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    let _ = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        }))
        .expect("the tool row refuses nothing");
    let tool_id = session.tools()[0].id.0;

    // Add the SVG model.
    let model = LoadedModel {
        id: 0, // overwritten by add_model
        name: "demo_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: svg_path,
        kind: Some(ModelKind::Svg),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("the model row refuses nothing")
        .created
        .expect("add_model reports the new model id");

    // Add the pocket toolpath. Defaults from PocketConfig give a
    // sensible starting point; we set a finite depth bounded by stock.
    let pocket = PocketConfig {
        depth: 2.0,
        depth_per_pass: 1.5,
        feed_rate: 1500.0,
        ..Default::default()
    };
    let tc = ToolpathConfig {
        id: ToolpathId(0), // overwritten by add_toolpath
        name: "smoke pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let toolpath_index = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(tc),
        }))
        .expect("add_toolpath")
        .created
        .expect("add_toolpath reports the new toolpath index");
    Some((session, toolpath_index))
}

#[test]
fn optimize_toolpath_full_pipeline() {
    let Some((mut session, toolpath_index)) = build_pocket_session() else {
        return;
    };
    let cancel = AtomicBool::new(false);

    // Generate the toolpath so the simulator has something to run.
    session
        .generate_toolpath(toolpath_index, &cancel)
        .expect("baseline generate");

    // Baseline sim — produces the trace the optimizer scores against.
    let opts = SimulationOptions {
        resolution: 1.0, // faster than 0.5; the optimizer rescales internally
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    let baseline_trace = {
        let sim = session
            .run_simulation(&opts, &cancel)
            .expect("baseline sim");
        sim.cut_trace
            .as_ref()
            .expect("baseline trace populated")
            .clone()
    };

    // Snapshot the toolpath's params, so the test can show that the
    // BaselineRestoreGuard restored them after `optimize_toolpath`.
    let baseline_op_before = session
        .get_toolpath_config(toolpath_index)
        .expect("toolpath_index in range")
        .operation
        .clone();

    // Run the optimizer end-to-end.
    let outcome = optimize_toolpath(&mut session, &baseline_trace, toolpath_index, &cancel);

    // The guard restored the parameters.
    let baseline_op_after = session
        .get_toolpath_config(toolpath_index)
        .expect("toolpath still present")
        .operation
        .clone();
    assert_eq!(
        format!("{baseline_op_after:?}"),
        format!("{baseline_op_before:?}"),
        "BaselineRestoreGuard must restore the operation parameters"
    );

    // Phase 1: one candidate, the baseline, and the "no search" text.
    assert_eq!(
        outcome.kind,
        OutcomeKind::NoSafeImprovement,
        "Phase 1 searches nothing, so the outcome is NoSafeImprovement: {outcome:?}"
    );
    assert_eq!(outcome.candidates.len(), 1, "only the baseline candidate");
    let baseline = &outcome.candidates[0];
    assert_eq!(baseline.stage, SearchStage::Baseline);
    assert!(
        !baseline.delta.has_changes(),
        "index 0 is the baseline (no delta)"
    );
    assert!(
        baseline.cycle_time_s > 0.0,
        "the baseline cycle time comes from the trace"
    );
    assert_eq!(outcome.narrative.explanation, NO_SEARCH_EXPLANATION);
    assert_eq!(outcome.recommended_index, None);
    assert!(outcome.machine_snapshot.is_some());
    assert!(outcome.assumptions.is_some());
}

#[test]
fn optimize_project_full_pipeline() {
    let Some((mut session, _toolpath_index)) = build_pocket_session() else {
        return;
    };
    let cancel = AtomicBool::new(false);

    // Generate every toolpath (just the one) and run baseline sim.
    for idx in 0..session.toolpath_configs().len() {
        session
            .generate_toolpath(idx, &cancel)
            .expect("baseline generate");
    }
    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    let baseline_trace = {
        let sim = session
            .run_simulation(&opts, &cancel)
            .expect("baseline sim");
        sim.cut_trace
            .as_ref()
            .expect("baseline trace populated")
            .clone()
    };

    let report = optimize_project(&mut session, &baseline_trace, &NoProgress, &cancel);

    // One row per enabled toolpath (just one).
    assert_eq!(
        report.per_toolpath.len(),
        1,
        "Expected one per_toolpath row (one enabled toolpath)"
    );
    // The baseline cycle sum comes from the trace.
    assert!(
        report.baseline_cycle_time_s > 0.0,
        "baseline_cycle_time_s must come from the trace, got {}",
        report.baseline_cycle_time_s
    );
    // One toolpath carries all of the runtime, so it is the bottleneck.
    assert_eq!(report.bottleneck_index, Some(report.per_toolpath[0].0));
    // Phase 1: the row is the "no search" outcome, and the walk saves
    // nothing.
    let outcome = &report.per_toolpath[0].1;
    assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement);
    assert_eq!(outcome.narrative.explanation, NO_SEARCH_EXPLANATION);
    assert!(
        (report.optimized_cycle_time_s(&[true]) - report.baseline_cycle_time_s).abs() < 1e-12,
        "no candidate means no saving"
    );
}

#[test]
fn optimize_toolpath_cancel_returns_quickly() {
    // When cancel is set up-front the optimizer should not produce
    // a Ranked outcome — it should bail with a NoSafeImprovement or
    // Skipped before doing any sim work.
    let Some((mut session, toolpath_index)) = build_pocket_session() else {
        return;
    };
    let cancel = AtomicBool::new(true); // pre-cancelled

    // Still need a baseline trace for the up-front data lookup.
    let cancel_off = AtomicBool::new(false);
    session
        .generate_toolpath(toolpath_index, &cancel_off)
        .expect("baseline generate");
    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    let baseline_trace = {
        let sim = session
            .run_simulation(&opts, &cancel_off)
            .expect("baseline sim");
        sim.cut_trace
            .as_ref()
            .expect("baseline trace populated")
            .clone()
    };

    let outcome = optimize_toolpath(&mut session, &baseline_trace, toolpath_index, &cancel);
    assert!(
        outcome.kind != OutcomeKind::Ranked,
        "Pre-cancelled run should not produce Ranked: {:?}",
        outcome
    );
}
