//! Step 3 PR2 end-to-end test: drill ops produce `DrillToolpathSummary` +
//! drill gates + drill cylinders in the composite mesh, end-to-end through
//! a `ProjectSession`.
//!
//! Substitutes for the WANAKA TP0/TP2 revalidation step (the §9 plan gate);
//! the real WANAKA TOML referenced in `wanaka_e2e_chipload_gate.rs` lives
//! outside the repo and varies by machine. This synthetic test constructs
//! the same end-to-end shape — load via session API, generate, simulate,
//! inspect the trace — so the regression bar lives in-repo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ids::ToolpathId;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::{AlignmentPin, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};

fn make_drill_tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = diameter;
    tool.flute_count = 2;
    tool.name = "Test Drill".to_owned();
    tool
}

fn make_drill_toolpath(tool_id: usize, peck_depth: f64) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Pin Drill".to_owned(),
        enabled: true,
        operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            holes: vec![[20.0, 20.0], [80.0, 80.0]],
            spoilboard_penetration: 1.0,
            cycle: DrillCycleType::Peck,
            peck_depth,
            feed_rate: 300.0,
            retract_z: 2.0,
            spindle_rpm: Some(18_000),
            selected_holes: None,
            selected_layers: Vec::new(),
        }),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
    }
}

fn build_drill_session(peck_depth: f64, tool_diameter: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        // Keep total hole depth (stock z + spoilboard penetration) under
        // 0.75 × chip-welding threshold for softwood (8 × 4mm tool / 4 =
        // 8 mm safe per unit D, so 0.75 × 8 = 6 → 24mm safe at D=4mm).
        z: 14.0,
        auto_from_model: false,
        alignment_pins: vec![
            AlignmentPin::new(20.0, 20.0, 4.0),
            AlignmentPin::new(80.0, 80.0, 4.0),
        ],
        material: Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_drill_tool(tool_diameter));
    let tool_id = session.tools()[tool_idx].id.0;
    let tc = make_drill_toolpath(tool_id, peck_depth);
    session.add_toolpath(0, tc).expect("add drill toolpath");
    session
}

/// End-to-end: a session with one AlignmentPinDrill toolpath produces a
/// populated `DrillToolpathSummary` in the simulation trace, with peck
/// count matching the cycle's nominal pecks.
#[test]
fn drill_session_produces_drill_summary_with_pecks() {
    let mut session = build_drill_session(3.0, 4.0); // 15mm depth / 3mm peck → 5 pecks
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate drill toolpath");

    // Pre-sim assertion: the compute result carries the DrillOp variant
    // (PR1 invariant).
    let result = session.get_result(0).expect("result exists");
    assert!(
        result.is_drill_op(),
        "AlignmentPinDrill toolpath should carry OpData::DrillOp"
    );
    let drill_op = result.drill_op().expect("drill_op present");
    assert_eq!(drill_op.holes.len(), 2, "two holes from config");

    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let trace = sim.cut_trace.as_ref().expect("cut trace present");

    // PR2 assertion 1: drill_summaries is populated for the drill toolpath.
    let summary = trace
        .drill_summary_for(ToolpathId(0))
        .expect("drill summary for TP0 must be present");
    assert_eq!(summary.toolpath_id, ToolpathId(0));
    assert_eq!(summary.hole_count, 2);
    assert!(
        summary.peck_count >= 2,
        "expected at least one peck per hole; got peck_count={}",
        summary.peck_count
    );
    assert!(
        summary.deepest_hole_mm > 0.0,
        "deepest_hole_mm must be positive; got {}",
        summary.deepest_hole_mm
    );
    assert!(
        summary.peck_pattern_adequate,
        "Ø4 peck depth 3mm in softwood should be adequate (3/4 = 0.75 < 2.0 threshold)"
    );

    // PR2 assertion 2: drill_samples carries one entry per peck per hole.
    let drill_samples_for_tp: Vec<_> = trace
        .drill_samples
        .iter()
        .filter(|s| s.toolpath_id == ToolpathId(0))
        .collect();
    assert_eq!(
        drill_samples_for_tp.len(),
        summary.peck_count,
        "drill_samples count should match summary.peck_count"
    );
    assert!(
        drill_samples_for_tp
            .iter()
            .all(|s| s.chip_evacuation_score > 0.99),
        "Peck cycle should produce chip_evacuation_score ~1.0 across pecks; got values: {:?}",
        drill_samples_for_tp
            .iter()
            .map(|s| s.chip_evacuation_score)
            .collect::<Vec<_>>()
    );

    // PR2 assertion 3: tool-load report carries drill_gates for the drill op.
    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == ToolpathId(0))
        .expect("tool-load verdict for drill TP");
    let drill_gates = verdict
        .drill_gates
        .as_ref()
        .expect("drill_gates verdict populated for drill op");
    assert!(
        !drill_gates.chip_welding.is_exceeded(),
        "Ø4 / 15mm softwood drill (D/d=3.75 vs softwood threshold 8) should NOT trigger chip welding: {:?}",
        drill_gates.chip_welding
    );
    assert!(
        !drill_gates.peck_adequacy.is_exceeded(),
        "Ø4 / 3mm peck softwood (D/d=0.75) should NOT trigger peck adequacy: {:?}",
        drill_gates.peck_adequacy
    );

    // PR2 assertion 4: composite simulation mesh includes drill cylinder
    // geometry (analytic, from `append_drill_cylinders`). Two holes × 33
    // verts/hole × 3 floats/vert = 198 floats minimum added on top of the
    // dexel mesh.
    assert!(
        sim.mesh.vertices.len() > 198,
        "composite mesh should carry drill-cylinder verts on top of dexel mesh; got {} verts",
        sim.mesh.vertices.len() / 3
    );
}

/// Drill with an oversized peck depth in softwood should trip the
/// peck-adequacy gate.
#[test]
fn drill_session_oversize_peck_trips_peck_adequacy_gate() {
    // peck=13mm on Ø2 → peck/D = 6.5 vs the Janka-banded softwood
    // per-peck threshold 6.0 (`Material::drill_per_peck_max_dtd`,
    // raised from the flat 2.0 in the 2026-06-03 banding fix) →
    // inadequate. A Ø4 tool can't trip this any more — the 15mm hole
    // caps per-peck D/d at 3.75.
    let mut session = build_drill_session(13.0, 2.0);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate drill toolpath");
    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .expect("cut trace");
    let summary = trace
        .drill_summary_for(ToolpathId(0))
        .expect("drill summary");
    assert!(
        !summary.peck_pattern_adequate,
        "13mm peck on Ø2 softwood (peck/D=6.5 vs threshold 6.0) should flag inadequate"
    );

    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == ToolpathId(0))
        .expect("verdict for TP0");
    let drill_gates = verdict.drill_gates.as_ref().expect("drill_gates populated");
    assert!(
        drill_gates.peck_adequacy.is_exceeded(),
        "oversize peck must trip peck-adequacy gate: {:?}",
        drill_gates.peck_adequacy
    );
}
