//! W2.5 (D10) — every export states the feed stage of what it emits.
//!
//! The adaptive feed modulation pass reads one simulation's cut trace and
//! writes the modulated path back into `session.results`. Export emits
//! whichever path the slot holds. Before W2.5 nothing said which, nothing
//! said when the simulation the feeds came from was gone, and under
//! `StaleResultPolicy::AcceptPreviousGeometry` the viz export emitted the
//! worker's pre-modulation copy with no word.
//!
//! The ladder this file walks, on the AS001 pocket (the F-036b fixture):
//!
//! 1. generate: the slot is `Dressed`; the header says "as planned";
//! 2. simulate with modulation on: the slot is `Modulated` from run 1; the
//!    header names run 1; the export reports no caution;
//! 3. simulate again with modulation off: the slot still holds run 1's
//!    feeds, the session holds run 2, and nothing the modulation read has
//!    changed. The header names run 1 with no stale clause (B5: the run
//!    counter does not decide currency);
//! 4. a new simulation resolution drops the simulation, and the stock the
//!    modulation read is now at another cell: the header says no
//!    simulation is current;
//! 5. simulate again with modulation off: the header and the export report
//!    both say run 1 is not the current run 3, and the export does not
//!    refuse;
//! 6. a machine edit keeps the result: a Pocket has no link stage (B7);
//! 7. an edit that drops the result, exported under
//!    `AcceptPreviousGeometry`: the worker copy carries planned feeds, and
//!    the header says "as planned".
//!
//! Each step asserts the exact header line on the core door
//! (`export_gcode_checked`) and the viz door (the one MCP and the GUI use).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::{CoolantMode, ToolLoadExportPolicy};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, FeedStage, InvalidateToolArgs, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SetMachineArgs, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
    ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;
use rs_cam_viz::io::export::{ExportedGcode, export_gcode_from_session_reporting};
use rs_cam_viz::state::runtime::{GuiState, StaleResultPolicy, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::ToolpathResult;

const PLANNED: &str = "(feeds: as planned, not modulated)";
const FROM_RUN_1: &str = "(feeds: modulated from simulation run 1)";
const RUN_1_NOT_CURRENT: &str =
    "(feeds: modulated from simulation run 1; the current simulation is run 3)";
const NO_SIMULATION: &str = "(feeds: modulated from simulation run 1; no simulation is current)";

const PERMISSIVE: ToolLoadExportPolicy = ToolLoadExportPolicy {
    accept_unmodeled: true,
    accept_exceeded: true,
};

fn end_mill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (W2.5)".to_owned();
    tool
}

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let n = 64;
    let hole = (0..n)
        .map(|i| {
            let t = f64::from(i) * TAU / f64::from(n);
            P2::new(40.0 + 10.0 * (-t).cos(), 30.0 + 10.0 * (-t).sin())
        })
        .collect();
    Polygon2::with_holes(exterior, vec![hole])
}

/// The AS001 pocket session of F-036b, with a kinematics block so the
/// modulation pass changes feeds.
fn pocket_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });
    let tool_idx = builder.add_tool(end_mill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            rounded_rect_with_island(),
        ])),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let pocket = PocketConfig {
        ramp_feed_rate: None,
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(6.0),
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
        region_ordering: rs_cam_core::compute::operation_configs::RegionOrdering::ByArea,
        selection: rs_cam_core::selection::Selection::drawing(),
        open_paths: rs_cam_core::compute::operation_configs::OpenPaths::Skip,
    };
    let tc = ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: DressupConfig::for_op(OperationType::Pocket),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tc).expect("add the pocket");
    let mut session = builder.build();
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    session
}

fn simulate(session: &mut ProjectSession, modulate: bool) {
    let opts = SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(1.0),
        skip_ids: Vec::new(),
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: modulate,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    let _ = session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation completes");
}

/// The GUI store after `drain_compute_results`: the worker copy of the
/// generated result, the same `Arc` the session slot took.
fn gui_for(session: &ProjectSession) -> GuiState {
    let result = session.get_result(0).expect("generated");
    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.run = rs_cam_viz::state::runtime::ToolpathRun::Done(ToolpathResult {
        annotated: Arc::clone(result.annotated()),
        stats: result.stats.clone(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    gui.toolpath_rt.insert(session.toolpath_configs()[0].id, rt);
    gui
}

fn core_gcode(session: &ProjectSession) -> String {
    session
        .export_gcode_checked(
            session
                .simulation_result()
                .and_then(|s| s.cut_trace.as_deref()),
            PERMISSIVE,
        )
        .expect("the core door exports")
        .gcode
}

fn viz_export(session: &ProjectSession, gui: &GuiState, stale: StaleResultPolicy) -> ExportedGcode {
    export_gcode_from_session_reporting(session, gui, &SimulationState::new(), PERMISSIVE, stale)
        .expect("the viz door exports; a feeds caution never refuses")
}

/// The feed-stage header line, read the way an operator reads it: the
/// comment directly after the toolpath's label.
fn feeds_line(gcode: &str) -> &str {
    let lines: Vec<&str> = gcode.lines().collect();
    let label = lines
        .iter()
        .position(|l| *l == "(Pocket)")
        .unwrap_or_else(|| panic!("the program names the toolpath:\n{gcode}"));
    lines[label + 1]
}

#[test]
fn every_export_states_the_feed_stage_it_emits() {
    let mut session = pocket_session();
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(0, &cancel)
        .expect("generate the pocket");
    let gui = gui_for(&session);

    // 1. Generated: planned feeds.
    assert_eq!(session.simulation_run(), None);
    assert_eq!(session.feed_stage(0), Some(FeedStage::Planned));
    assert_eq!(feeds_line(&core_gcode(&session)), PLANNED);
    let exported = viz_export(&session, &gui, StaleResultPolicy::Refuse);
    assert_eq!(feeds_line(&exported.gcode), PLANNED);
    assert!(exported.feeds_cautions.is_empty());

    // 2. Simulated with modulation on: the slot names run 1.
    simulate(&mut session, true);
    assert_eq!(session.simulation_run(), Some(1));
    assert_eq!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent { simulation_run: 1 }),
        "the AS001 pocket modulates under a kinematics block (F-036b AB2)"
    );
    assert_eq!(feeds_line(&core_gcode(&session)), FROM_RUN_1);
    let exported = viz_export(&session, &gui, StaleResultPolicy::Refuse);
    assert_eq!(feeds_line(&exported.gcode), FROM_RUN_1);
    assert!(exported.feeds_cautions.is_empty());

    // 3. Simulated again with modulation off: the slot keeps run 1's
    // feeds and the session holds run 2. Nothing the modulation read has
    // changed, so run 1's feeds are current (B5).
    simulate(&mut session, false);
    assert_eq!(session.simulation_run(), Some(2));
    assert_eq!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedCurrent { simulation_run: 1 })
    );
    assert_eq!(feeds_line(&core_gcode(&session)), FROM_RUN_1);
    let exported = viz_export(&session, &gui, StaleResultPolicy::Refuse);
    assert_eq!(feeds_line(&exported.gcode), FROM_RUN_1);
    assert!(exported.feeds_cautions.is_empty());

    // 4. A new simulation resolution drops the simulation and keeps the
    // fresh-stock result. The stock run 1 carved is at another cell now.
    let cell = session.simulation_resolution_mm() * 2.0;
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(cell),
            },
        ))
        .expect("a positive cell");
    assert!(session.get_result(0).is_some(), "the edit keeps the result");
    assert_eq!(session.simulation_run(), None);
    assert_eq!(feeds_line(&core_gcode(&session)), NO_SIMULATION);
    assert_eq!(
        feeds_line(&viz_export(&session, &gui, StaleResultPolicy::Refuse).gcode),
        NO_SIMULATION
    );

    // 5. Simulated again with modulation off: run 3. Stated, not refused.
    simulate(&mut session, false);
    assert_eq!(session.simulation_run(), Some(3));
    assert_eq!(
        session.feed_stage(0),
        Some(FeedStage::ModulatedSuperseded {
            simulation_run: 1,
            current_run: Some(3),
        })
    );
    assert_eq!(feeds_line(&core_gcode(&session)), RUN_1_NOT_CURRENT);
    let exported = viz_export(&session, &gui, StaleResultPolicy::Refuse);
    assert_eq!(feeds_line(&exported.gcode), RUN_1_NOT_CURRENT);
    assert_eq!(
        exported.feeds_cautions,
        vec![
            "'Pocket': feeds: modulated from simulation run 1; the current simulation is run 3"
                .to_owned()
        ]
    );
    let reply = rs_cam_viz::io::export::export_reply_text(
        "G-code exported".to_owned(),
        &exported.machine_safety,
        &exported.load_outcome,
        &exported.feeds_cautions,
    );
    assert!(
        reply.contains(
            "Feeds caution: 1 toolpath(s) emit feeds modulated from an earlier simulation."
        ),
        "the MCP reply carries the caution: {reply}"
    );

    // 6. A machine edit drops the simulation and keeps the result: a
    // Pocket has no link stage, so it reads no machine at generation (B7).
    let mut machine = session.machine().clone();
    machine.max_feed_mm_min += 1.0;
    let effects = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    assert!(effects.stale.is_empty(), "no result goes stale");
    assert!(session.get_result(0).is_some(), "the edit keeps the result");
    assert_eq!(session.simulation_run(), None);
    assert_eq!(feeds_line(&core_gcode(&session)), NO_SIMULATION);

    // 7. A tool edit drops the result. The accepted previous geometry is
    // the worker copy, which no modulation wrote.
    let tool_id = session.tools()[0].id.0;
    let _ = session
        .apply(Command::InvalidateTool(InvalidateToolArgs { tool_id }))
        .expect("the tool is in the session");
    assert!(session.get_result(0).is_none());
    let exported = viz_export(&session, &gui, StaleResultPolicy::AcceptPreviousGeometry);
    assert_eq!(feeds_line(&exported.gcode), PLANNED);
    assert!(exported.feeds_cautions.is_empty());
}
