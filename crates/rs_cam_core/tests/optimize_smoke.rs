//! End-to-end smoke test for the optimizer.
//!
//! The 60+ unit tests in `tool_load::optimize` cover the algorithm
//! pieces (Stage 0 scaling math, DOC variant grid, refusal narratives,
//! candidate ranking) but no test exercises the full
//! apply -> regen -> sim -> gate -> outcome path against a real
//! toolpath. This file does.
//!
//! The fixture is the already-checked-in `fixtures/demo_pocket.svg` —
//! a small rectangle with a circular island, parses fast and produces
//! a deterministic pocket toolpath.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::clone_on_ref_ptr
)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, FeedsAutoMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::tool_load::optimize::{
    NoProgress, OptimizeOutcome, optimize_project, optimize_toolpath,
};

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
        rs_cam_core::svg_input::load_svg(&svg_path, 0.1).expect("demo_pocket.svg should parse");
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
    session.set_stock_config(stock);

    // Add a 6mm end mill (matches the optimize gate's typical wood
    // router setup). add_tool returns the vec index; we want the
    // assigned ToolId.0 for the toolpath link.
    let tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    session.add_tool(tool);
    let tool_id = session.tools()[0].id.0;

    // Add the SVG model.
    let model = LoadedModel {
        id: 0, // overwritten by add_model
        name: "demo_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(polygons)),
        path: svg_path,
        kind: Some(ModelKind::Svg),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    // Add the pocket toolpath. Defaults from PocketConfig give a
    // sensible starting point; we set a finite depth bounded by stock.
    let pocket = PocketConfig {
        depth: 2.0,
        depth_per_pass: 1.5,
        feed_rate: 1500.0,
        ..Default::default()
    };
    let tc = ToolpathConfig {
        id: 0, // overwritten by add_toolpath
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
        feeds_auto: FeedsAutoMode::default(),
        debug_options: ToolpathDebugOptions::default(),
    };
    let toolpath_index = session.add_toolpath(0, tc).expect("add_toolpath");
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
        metrics_enabled: true,
        auto_resolution: false,
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

    // Snapshot the toolpath's params so we can verify the
    // BaselineRestoreGuard restored them after optimize_toolpath
    // returns. Use feed_rate as the tell-tale field — it's part of
    // every operation config and the optimizer routinely scales it.
    let baseline_op_before = session
        .get_toolpath_config(toolpath_index)
        .expect("toolpath_index in range")
        .operation
        .clone();
    let baseline_feed_before = baseline_op_before.feed_rate();

    // Run the optimizer end-to-end.
    let outcome = optimize_toolpath(&mut session, &baseline_trace, toolpath_index, &cancel);

    // Verify session is restored regardless of outcome.
    let baseline_op_after = session
        .get_toolpath_config(toolpath_index)
        .expect("toolpath still present")
        .operation
        .clone();
    let feed_after = baseline_op_after.feed_rate();
    assert!(
        (feed_after - baseline_feed_before).abs() < 1e-6,
        "BaselineRestoreGuard must restore feed_rate: {} -> {}",
        baseline_feed_before,
        feed_after,
    );

    // Outcome shape: must be one of the three variants. For a tiny
    // pocket that's typically Skipped (no LUT row matches the
    // hardcoded tool/material) or Ranked. We assert only that the
    // outcome was produced — not the specific variant — because
    // LUT-routing outcomes drift as the calibration data evolves.
    match outcome {
        OptimizeOutcome::Ranked(candidates) => {
            assert!(
                !candidates.is_empty(),
                "Ranked outcome must carry at least the baseline candidate"
            );
            // Index 0 is always the baseline.
            assert!(
                !candidates[0].delta.has_changes(),
                "Index 0 must be baseline (no delta)"
            );
        }
        OptimizeOutcome::NoSafeImprovement { explanation, .. } => {
            assert!(
                !explanation.is_empty(),
                "NoSafeImprovement must carry an explanation"
            );
        }
        OptimizeOutcome::Skipped { .. } => {
            // Acceptable for a fixture with no LUT-matching tool.
        }
    }
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
        metrics_enabled: true,
        auto_resolution: false,
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

    // Report has one entry per enabled toolpath (just one).
    assert_eq!(
        report.per_toolpath.len(),
        1,
        "Expected one per_toolpath row (one enabled toolpath)"
    );
    // Baseline cycle time was sourced from the trace — should be > 0
    // for a non-trivial pocket op, but a tiny SVG with cutting paths
    // could be very fast. Just assert non-negative.
    assert!(
        report.baseline_cycle_time_s >= 0.0,
        "baseline_cycle_time_s must be non-negative, got {}",
        report.baseline_cycle_time_s
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
        metrics_enabled: true,
        auto_resolution: false,
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
        !matches!(outcome, OptimizeOutcome::Ranked(_)),
        "Pre-cancelled run should not produce Ranked: {:?}",
        outcome
    );
}
