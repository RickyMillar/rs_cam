//! **N7** — with no kinematics block the modulation retime must not integrate
//! and must not relabel.
//!
//! # The ruling
//!
//! Operator ruling 2026-09-11, row N7 (a) in
//! `planning/arch_consolidation_2026-09-09/STATUS.md`.
//!
//! # The observation
//!
//! `MachineProfile::kinematics` is the F-034 feature flag. Its own doc
//! (`machine.rs`) says the `None` value keeps the live-sim `total_runtime_s`
//! byte-identical, so every preset keeps it `None`. The simulator honours
//! that: `compute::simulate` calls `apply_kinematics_cycle_time` only when the
//! request carries a `KinematicsContext`, and `build_sim_request` builds that
//! context from `self.machine.kinematics`, not from the fallback.
//!
//! `ProjectSession::apply_adaptive_feed_modulation` did not honour it. Its
//! re-time block read `self.machine.effective_kinematics()`, which falls back
//! to `MachineKinematics::generic_wood_router()`, so an unconfigured machine
//! got a full re-integration whenever modulation ran. Modulation defaults ON
//! (`SimulationOptions::adaptive_feed_modulation`), so the production path
//! always reached it.
//!
//! # What the operator saw
//!
//! `readiness::toolpath_cycle_time` reads `toolpath_runtimes` first and labels
//! that answer `CycleTimeBasis::MachineModel`. Its second arm reads
//! `runtime_by_intent` and labels a `Some` the same way. The re-time stamped
//! both on a machine that carries no kinematics at all, so the GUI told the
//! operator that a generic-wood-router guess was a machine model, and
//! `total_runtime_s` moved with the modulated feeds against a doc that
//! promises it does not move.
//!
//! # What this file pins
//!
//! With `kinematics: None`, the flag-ON run and the flag-OFF run publish the
//! same runtimes: an empty `toolpath_runtimes`, `runtime_by_intent` absent
//! everywhere, and one `total_runtime_s`. The modulation of feeds still
//! applies — `modulation_really_moves_this_fixture` asserts it — so only the
//! cycle-time re-integration is guarded.
//!
//! # The rebase decision this file assumes
//!
//! The G-AIRDENOM rebase (`simulation_cut::rebase_cutting_times`) is skipped
//! with the rest of the block. It takes the integrator's own
//! `CycleTimeBreakdown` as its target clock, so with no integration there is
//! no clock to move the cutting seconds onto. Running it against a fallback
//! integration while `total_runtime_s` stays naive would rebuild the exact
//! mixed-base defect G-AIRDENOM closed. The naive dexel seconds at the
//! commanded feed stay the one time base, on both sides of the flag.
//!
//! # What it deliberately does NOT assert
//!
//! With `kinematics: None` a drill toolpath publishes neither an engagement
//! summary nor a runtime row, so its seconds are absent from the project
//! total in BOTH arms. That gap predates N7 and belongs to the F-034 flag, not
//! to this ruling. This file asserts nothing about the drill's share.

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
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    LoadedModel, ProjectSession, ProjectSessionBuilder, SimulationOptions, ToolpathConfig,
};
use rs_cam_core::simulation_cut::SimulationCutTrace;

/// The AS001 pocket, which produces an engagement summary.
const POCKET: ToolpathId = ToolpathId(0);
/// The drill, which produces no engagement summary.
const DRILL: ToolpathId = ToolpathId(1);

// ----- fixture (the N2 fixture, with the kinematics block as a knob) -

fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (N7 test)".to_owned();
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

/// Two holes, both OUTSIDE the pocket footprint and inside the stock, so the
/// two operations do not machine the same ground.
fn drill_model() -> LoadedModel {
    let targets = vec![
        rs_cam_core::dxf_input::DrillTarget {
            x: 85.0,
            y: 20.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
        },
        rs_cam_core::dxf_input::DrillTarget {
            x: 85.0,
            y: 70.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
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
        path: PathBuf::from("synthetic://n7_drill_holes.svg"),
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
    // `DrillCycleType::Simple` on purpose, as in the N2 fixture: G82 dwell is
    // not motion, so a dwelling cycle would move the runtime identities for a
    // reason that has nothing to do with N7.
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

/// The AS001 stock: 100 x 100 x 12, top at Z = 0.
///
/// `kinematics` is the knob. `None` is the shipped preset value and the case
/// this file is about; `Some` builds the control.
fn build_session(kinematics: Option<MachineKinematics>) -> ProjectSession {
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
    machine.kinematics = kinematics;
    let _ = session.set_machine(machine);
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
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn run(kinematics: Option<MachineKinematics>, modulate: bool) -> ProjectSession {
    let mut session = build_session(kinematics);
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

/// The case under test: the shipped preset value.
fn run_unconfigured(modulate: bool) -> ProjectSession {
    run(None, modulate)
}

fn trace(session: &ProjectSession) -> &SimulationCutTrace {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metrics are on, so a cut trace exists")
}

// ----- non-vacuity, first --------------------------------------------

/// **Non-vacuity 1.** The fixture must really carry no kinematics block, and
/// the fallback the defect used must really be available. If
/// `effective_kinematics` stopped falling back, the re-time would have no
/// clock of its own and every assertion below would pass for the wrong
/// reason.
#[test]
fn the_fixture_really_has_no_kinematics_block() {
    let session = run_unconfigured(true);
    assert!(
        session.machine().kinematics.is_none(),
        "the fixture must carry no kinematics block — that None is the F-034 \
         feature flag this file is about"
    );
    let fallback = session.machine().effective_kinematics();
    assert!(
        fallback.acceleration_mm_s2 > 0.0,
        "effective_kinematics must still return a usable fallback model, \
         because that fallback is what the re-time used to integrate on"
    );
}

/// **Non-vacuity 2.** The claims below are claims about a project that mixes
/// one toolpath WITH an engagement summary and one WITHOUT.
#[test]
fn the_fixture_really_is_the_defects_shape() {
    let session = run_unconfigured(true);
    let t = trace(&session);

    assert!(
        t.toolpath_summaries.iter().any(|s| s.toolpath_id == POCKET),
        "the pocket must produce an engagement summary"
    );
    assert!(
        !t.toolpath_summaries.iter().any(|s| s.toolpath_id == DRILL),
        "the drill must have NO engagement summary. If this fires, drills now \
         produce summaries and this file needs a rewrite rather than a \
         relaxed bar."
    );
    assert!(
        t.drill_summaries.iter().any(|d| d.toolpath_id == DRILL),
        "the drill must publish a drill_summaries row instead"
    );
}

/// **Non-vacuity 3.** The re-time block only misreports a runtime when
/// modulation actually runs and moves a feed. Count the emitted cutting moves
/// that left the commanded value, the way F-036b AB5 does.
///
/// This is also the positive half of the ruling: the modulation of feeds keeps
/// applying with no kinematics block. Only the re-integration is guarded.
///
/// Note what this file does NOT copy from the N2 fixture: N2 also asserts that
/// the pocket's published runtime MOVES between the two arms. Under this
/// ruling it must not move. That is `the_total_runtime_does_not_move` below.
#[test]
fn modulation_really_moves_this_fixture() {
    use rs_cam_core::toolpath::MoveIntent;

    let session_on = run_unconfigured(true);
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
         commanded {commanded_feed} mm/min, so the re-time block was never \
         reached. Recalibrate the fixture until it moves a feed; do not relax \
         the assertions below."
    );
}

// ----- the ruling ----------------------------------------------------

/// **Assertion 1 — a contract pin.** `toolpath_runtimes` answers "was this
/// integrated?". With no kinematics block nothing integrates, so the list is
/// empty on both sides of the modulation flag.
///
/// This one passes before the fix as well: the pre-fix re-time built its map
/// from the already-empty `toolpath_runtimes`, so `publish_cycle_times` wrote
/// an empty list back. It is here to pin the contract, and to keep a later
/// change from filling the slot from the summaries loop instead.
#[test]
fn no_kinematics_publishes_no_integrated_runtimes() {
    for modulate in [true, false] {
        let session = run_unconfigured(modulate);
        let t = trace(&session);
        assert!(
            t.toolpath_runtimes.is_empty(),
            "with no kinematics block nothing is integrated, so \
             toolpath_runtimes must stay empty (modulation = {modulate}); got \
             {} rows",
            t.toolpath_runtimes.len()
        );
    }
}

/// **Assertion 2.** `runtime_by_intent` is the field
/// `readiness::toolpath_cycle_time` reads to label a summary
/// `CycleTimeBasis::MachineModel`. With no machine model it must stay absent,
/// because `None` means NOT MEASURED and a `Some` here is a claim the profile
/// cannot support.
#[test]
fn no_kinematics_leaves_every_summary_unlabelled() {
    for modulate in [true, false] {
        let session = run_unconfigured(modulate);
        let t = trace(&session);
        for tp_summary in &t.toolpath_summaries {
            assert!(
                tp_summary.runtime_by_intent.is_none(),
                "toolpath {:?} carries runtime_by_intent = Some with no \
                 kinematics block (modulation = {modulate}). The readiness \
                 panel then labels a generic-wood-router guess \
                 CycleTimeBasis::MachineModel.",
                tp_summary.toolpath_id
            );
        }
    }
}

/// **Assertion 3.** The same claim at the project level.
#[test]
fn no_kinematics_leaves_the_project_summary_unlabelled() {
    for modulate in [true, false] {
        let session = run_unconfigured(modulate);
        let t = trace(&session);
        assert!(
            t.summary.runtime_by_intent.is_none(),
            "the project summary carries runtime_by_intent = Some with no \
             kinematics block (modulation = {modulate})"
        );
    }
}

/// **Assertion 4 — the F-034 contract.** `machine.rs` says the `None`
/// kinematics field keeps the live-sim `total_runtime_s` byte-identical. The
/// modulation flag must not move it either, because the re-time is the only
/// thing that would, and with no machine model it has nothing to integrate on
/// but a generic guess.
///
/// The tolerance is 1e-9, not a band: both arms must publish the SAME naive
/// dexel seconds, not two close numbers.
#[test]
fn the_total_runtime_does_not_move() {
    let on = trace(&run_unconfigured(true)).summary.total_runtime_s;
    let off = trace(&run_unconfigured(false)).summary.total_runtime_s;
    assert!(
        on > 0.0,
        "the fixture must publish a real runtime, got {on} s"
    );
    assert!(
        (on - off).abs() < 1e-9,
        "with no kinematics block the modulation flag must not move \
         total_runtime_s: got {on} s with modulation on against {off} s with \
         it off. machine.rs promises this value is byte-identical."
    );
}

/// **Assertion 5 — the rebase decision, pinned.** The G-AIRDENOM rebase moves
/// each cutting sample's seconds onto the integrator's clock. With no
/// integration there is no such clock, so the rebase is skipped with the rest
/// of the block and the cutting slices keep the live sim's naive dexel
/// seconds.
///
/// This is the one assertion that catches a "guard the integration, keep the
/// rebase" variant. That variant would move the cutting seconds onto a
/// fallback clock while `total_runtime_s` stays naive — one numerator over two
/// time models, which is the defect G-AIRDENOM closed.
///
/// It is also how the identity `cutting_runtime_s + rapid_runtime_s ==
/// total_runtime_s` survives here: both arms publish the same flag-OFF live-sim
/// values, so whatever relation the live sim published holds in both.
#[test]
fn the_cutting_times_do_not_move() {
    let session_on = run_unconfigured(true);
    let session_off = run_unconfigured(false);
    let on = trace(&session_on);
    let off = trace(&session_off);

    let project: [(&str, f64, f64); 3] = [
        (
            "cutting_runtime_s",
            on.summary.cutting_runtime_s,
            off.summary.cutting_runtime_s,
        ),
        (
            "rapid_runtime_s",
            on.summary.rapid_runtime_s,
            off.summary.rapid_runtime_s,
        ),
        (
            "air_cut_time_s",
            on.summary.air_cut_time_s,
            off.summary.air_cut_time_s,
        ),
    ];
    for (field, a, b) in project {
        assert!(
            (a - b).abs() < 1e-9,
            "with no kinematics block the modulation flag must not move the \
             project summary's {field}: got {a} s with modulation on against \
             {b} s with it off. The rebase belongs to the re-time and is \
             skipped with it."
        );
    }

    let pocket_cutting = |t: &SimulationCutTrace| {
        t.toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == POCKET)
            .map(|s| s.cutting_runtime_s)
            .expect("the pocket must carry an engagement summary")
    };
    let a = pocket_cutting(on);
    let b = pocket_cutting(off);
    assert!(
        (a - b).abs() < 1e-9,
        "with no kinematics block the modulation flag must not move the \
         pocket's cutting_runtime_s: got {a} s with modulation on against \
         {b} s with it off"
    );
}

/// **The control.** The guard must not go too far. The same fixture WITH a
/// kinematics block still integrates every toolpath, drill included — that is
/// G-DRILLTIME's contract, re-asserted by N2 one layer up.
#[test]
fn a_configured_machine_still_integrates_every_toolpath() {
    let session = run(Some(MachineKinematics::shapeoko_xxl_stock()), true);
    let t = trace(&session);
    assert!(
        t.toolpath_runtimes.iter().any(|r| r.toolpath_id == POCKET),
        "the integrator must publish a runtime for the pocket when the \
         machine carries kinematics"
    );
    assert!(
        t.toolpath_runtimes.iter().any(|r| r.toolpath_id == DRILL),
        "the integrator must publish a runtime for the drill when the machine \
         carries kinematics (G-DRILLTIME)"
    );
    assert!(
        t.summary.runtime_by_intent.is_some(),
        "a configured machine must still label the project summary"
    );
}
