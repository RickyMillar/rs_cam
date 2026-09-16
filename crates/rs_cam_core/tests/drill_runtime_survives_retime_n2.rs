//! **N2** — the modulation retime must republish every runtime, not only the
//! engagement summaries.
//!
//! # The observation
//!
//! G-DRILLTIME (2026-08-22) gave the kinematics integrator its own slot,
//! `SimulationCutTrace::toolpath_runtimes`, so a drill's seconds survive the
//! fold. `drill_cycle_time_integration_g_drilltime.rs:296` pins that identity
//! at the `run_simulation` layer.
//!
//! One layer above it, `ProjectSession::run_simulation` runs the adaptive feed
//! modulation post-pass. That pass re-times the trace. Before N2 it folded
//! **only** over `trace.toolpath_summaries`, so the defect G-DRILLTIME closed
//! re-opened one layer up. `adaptive_feed_modulation` defaults on, so the
//! production path always reaches the re-time.
//!
//! # The two defects, from one omission
//!
//! **Defect A — the project total drops every drill.** A drill toolpath sets
//! `metrics_not_applicable` and publishes a `drill_summaries` row instead of a
//! `toolpath_summaries` row. The re-time reset the project total to zero and
//! rebuilt it over the summary list alone, so the drill's seconds vanished
//! from `trace.summary.total_runtime_s`. That field reaches
//! `ProjectDiagnostics::total_runtime_s`, MCP `get_diagnostics`, the CLI
//! `project` report and the GUI diagnostics panel.
//!
//! **Defect B — every milling runtime stays pre-modulation.** The re-time
//! never rewrote `toolpath_runtimes`. `readiness::toolpath_cycle_time` reads
//! that slot FIRST, so readiness, pre-flight, the export wizard, the setup
//! sheet, the toolpath panel and the timeline all read the runtime of the
//! commanded feeds, not of the emitted ones.
//!
//! # What this file pins
//!
//! It re-asserts G-DRILLTIME's identity — project total equals the sum over
//! `toolpath_runtimes` — **after** the modulation re-time, and it pins the
//! agreement between the two published runtimes for one milling toolpath.
//!
//! # What it deliberately does NOT assert
//!
//! It does not assert that the drill's runtime is unchanged across the
//! re-time. The fix re-integrates every toolpath on the re-time's own clock
//! (`effective_kinematics` and the cutting feed ceiling), so the drill's
//! number may move. One clock for every published runtime is the ruling; a
//! byte-identical drill would contradict it.

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
use rs_cam_core::compute::operation_configs::{
    DrillConfig, DrillCycleType, PocketConfig, PocketPattern,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetMachineArgs, SimulationOptions,
    ToolpathConfig,
};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The AS001 pocket, which produces an engagement summary.
const POCKET: ToolpathId = ToolpathId(0);
/// The drill, which produces no engagement summary. That absence is the
/// defect's mechanism, so `the_fixture_really_is_the_defects_shape` asserts
/// it rather than assumes it.
const DRILL: ToolpathId = ToolpathId(1);

// ----- fixture (AS001 pocket, copied from F-036b, plus a drill) ------

fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (N2 test)".to_owned();
    tool
}

/// The AS001 pocket footprint: 5..75 in X, 5..55 in Y, with a round island.
fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let cx = 40.0;
    let cy = 30.0;
    let r = 10.0;
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

fn unit_square_at(cx: f64, cy: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - 1.0, cy - 1.0),
        P2::new(cx + 1.0, cy - 1.0),
        P2::new(cx + 1.0, cy + 1.0),
        P2::new(cx - 1.0, cy + 1.0),
    ])
}

fn pocket_model() -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![rounded_rect_with_island()])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// Two holes, both OUTSIDE the pocket footprint (X = 85 clears the pocket's
/// 75 mm edge) and inside the stock, so the two operations do not machine the
/// same ground.
fn drill_model() -> LoadedModel {
    let targets = vec![
        rs_cam_core::io::dxf_input::DrillTarget {
            x: 85.0,
            y: 20.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::io::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
        },
        rs_cam_core::io::dxf_input::DrillTarget {
            x: 85.0,
            y: 70.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::io::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
        },
    ];
    LoadedModel {
        id: 0,
        name: "holes".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![
            unit_square_at(85.0, 20.0),
            unit_square_at(85.0, 70.0),
        ])),
        drill_targets: Arc::new(targets),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://n2_drill_holes.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn pocket_toolpath(tool_id: usize, model_id: usize) -> ToolpathConfig {
    let pocket = PocketConfig {
        stepover: 2.0,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };
    ToolpathConfig {
        id: POCKET,
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
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn drill_toolpath(tool_id: usize, model_id: usize) -> ToolpathConfig {
    // `DrillCycleType::Simple` on purpose. G82 dwell is not motion, so it is
    // NOT in `toolpath_runtimes` — it is reported separately as
    // `DrillToolpathSummary::dwell_time_s`, and no surface adds the two. A
    // dwelling cycle would therefore make the runtime identities below read
    // short for a reason that has nothing to do with N2.
    let drill = DrillConfig {
        depth: 10.0,
        cycle: DrillCycleType::Simple,
        feed_rate: 300.0,
        ..DrillConfig::default()
    };
    ToolpathConfig {
        id: DRILL,
        name: "Holes".to_owned(),
        enabled: true,
        operation: OperationConfig::Drill(drill),
        dressups: DressupConfig::for_op(OperationType::Drill),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

/// The AS001 stock: 100 x 100 x 12, top at Z = 0. The drill depth of 10 mm
/// stays inside it.
fn build_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let stock = StockConfig {
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
    };
    builder = builder.stock(stock);

    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;

    let pocket_model_id = builder.add_model(pocket_model());
    let drill_model_id = builder.add_model(drill_model());

    let _ = builder
        .add_toolpath(0, pocket_toolpath(tool_id, pocket_model_id))
        .expect("add pocket toolpath");
    let _ = builder
        .add_toolpath(0, drill_toolpath(tool_id, drill_model_id))
        .expect("add drill toolpath");
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

fn opts(adaptive_feed_modulation: bool) -> SimulationOptions {
    SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn run(modulate: bool) -> ProjectSession {
    let mut session = build_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    session
        .generate_toolpath(1, &cancel)
        .expect("generate drill toolpath");
    session
        .run_simulation(&opts(modulate), &cancel)
        .expect("simulation completes");
    session
}

fn trace(session: &ProjectSession) -> &SimulationCutTrace {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metrics are on, so a cut trace exists")
}

fn runtime_of(t: &SimulationCutTrace, id: ToolpathId) -> f64 {
    t.toolpath_runtimes
        .iter()
        .find(|r| r.toolpath_id == id)
        .map(|r| r.breakdown.total_s)
        .expect("the integrator walks every toolpath in the request")
}

fn summary_runtime_of(t: &SimulationCutTrace, id: ToolpathId) -> f64 {
    t.toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == id)
        .map(|s| s.total_runtime_s)
        .expect("this toolpath must carry an engagement summary")
}

// ----- non-vacuity, first --------------------------------------------

/// **Non-vacuity 1.** Every claim below is a claim about a project that mixes
/// one toolpath WITH an engagement summary and one WITHOUT. If either
/// precondition stops holding, the rest of this file tests nothing.
#[test]
fn the_fixture_really_is_the_defects_shape() {
    let session = run(true);
    let t = trace(&session);

    assert!(
        t.toolpath_summaries.iter().any(|s| s.toolpath_id == POCKET),
        "the pocket must produce an engagement summary"
    );
    assert!(
        !t.toolpath_summaries.iter().any(|s| s.toolpath_id == DRILL),
        "the drill must have NO engagement summary — that absence is the \
         defect's mechanism. If this fires, drills now produce summaries and \
         this file needs a rewrite rather than a relaxed bar."
    );
    assert!(
        t.drill_summaries.iter().any(|d| d.toolpath_id == DRILL),
        "the drill must publish a drill_summaries row instead"
    );
    assert!(
        t.toolpath_runtimes.iter().any(|r| r.toolpath_id == POCKET),
        "the integrator must publish a runtime for the pocket"
    );
    assert!(
        t.toolpath_runtimes.iter().any(|r| r.toolpath_id == DRILL),
        "the integrator must publish a runtime for the drill (G-DRILLTIME)"
    );
}

/// **Non-vacuity 2.** The re-time only misreports a runtime when modulation
/// actually moves a feed, so the verdict reads the EMITTED feeds in the
/// pocket's IR: count the cutting moves that left the commanded value.
///
/// A runtime comparison alone is the weaker signal, because the flag-on and
/// flag-off paths integrate on two different ceilings —
/// `cutting_feed_ceiling_mm_min()` and `max_feed_mm_min`. They agree on the
/// default profile, so the second assertion below stands, but the feed count
/// is what carries the verdict.
#[test]
fn modulation_really_moves_this_fixture() {
    use rs_cam_core::toolpath::MoveIntent;

    let session_on = run(true);
    let tc = session_on
        .get_toolpath_config(0)
        .expect("toolpath config 0");
    let commanded_feed = tc.operation.feed_rate();
    let result = session_on
        .get_result(0)
        .expect("session result for toolpath 0");
    let moved = result
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|mv| {
            matches!(
                mv.intent,
                MoveIntent::ClearingCut | MoveIntent::FinishingCut
            )
        })
        .filter_map(|mv| mv.move_type.feed_rate())
        .filter(|&feed| (feed - commanded_feed).abs() > 0.5)
        .count();
    assert!(
        moved > 0,
        "the fixture is VACUOUS: the modulator left every pocket move at the \
         commanded {commanded_feed} mm/min. Recalibrate the fixture until it \
         moves a feed; do not relax the assertions below."
    );

    // And the pocket's published runtime moves with those feeds. The control
    // is the same project with the flag off. The pocket's OWN summary is the
    // signal here — never the project total, which is what defect A is about.
    let session_off = run(false);
    let off = summary_runtime_of(trace(&session_off), POCKET);
    let on = summary_runtime_of(trace(&session_on), POCKET);
    assert!(
        (on - off).abs() > 1e-6,
        "the modulator moved {moved} pocket feeds, so the pocket's runtime \
         must move too: got {on} s against {off} s"
    );
}

// ----- defect A: the project total drops the drill -------------------

/// **Defect A.** After the re-time, the project total must still equal the sum
/// over every integrated toolpath. This is G-DRILLTIME's identity, re-asserted
/// one layer above `drill_cycle_time_integration_g_drilltime.rs:296`.
#[test]
fn the_retimed_project_total_still_includes_the_drill() {
    let session = run(true);
    let t = trace(&session);

    let sum: f64 = t
        .toolpath_runtimes
        .iter()
        .map(|r| r.breakdown.total_s)
        .sum();
    assert!(
        (t.summary.total_runtime_s - sum).abs() < 1e-6,
        "project total {} must equal the sum of every integrated toolpath \
         {} after the modulation re-time",
        t.summary.total_runtime_s,
        sum
    );

    // The control that gives the assertion above its teeth: dropping the
    // drill's contribution must change the answer. Without it the total could
    // sum the pocket alone and still "match".
    let drill_s = runtime_of(t, DRILL);
    assert!(
        (t.summary.total_runtime_s - (sum - drill_s)).abs() > 1e-6,
        "the drill contributes {drill_s} s, so the total must differ from the \
         sum that omits it. If it does not, this test proves nothing."
    );
}

// ----- defect B: toolpath_runtimes stays pre-modulation --------------

/// **Defect B.** The two published runtimes for one milling toolpath must
/// agree. `readiness::toolpath_cycle_time` reads `toolpath_runtimes` first, so
/// a stale entry there is what every operator surface shows.
#[test]
fn the_published_runtimes_agree_after_the_retime() {
    let session = run(true);
    let t = trace(&session);

    let from_runtimes = runtime_of(t, POCKET);
    let from_summary = summary_runtime_of(t, POCKET);
    assert!(
        (from_runtimes - from_summary).abs() < 1e-6,
        "toolpath_runtimes reports {from_runtimes} s for the pocket and \
         toolpath_summaries reports {from_summary} s. Both are published, and \
         different surfaces read different ones."
    );
}

/// The drill's runtime must be a real quantity, not a zero that satisfies
/// "is present" and proves nothing. Two holes at 10 mm and 300 mm/min is
/// about 4 s of fed descent alone, before rapids and before accel ramps.
#[test]
fn the_drills_runtime_is_a_real_quantity() {
    let session = run(true);
    let drill_s = runtime_of(trace(&session), DRILL);
    assert!(
        drill_s > 1.0,
        "the drill's integrated runtime must be a real quantity, got {drill_s}"
    );
}
