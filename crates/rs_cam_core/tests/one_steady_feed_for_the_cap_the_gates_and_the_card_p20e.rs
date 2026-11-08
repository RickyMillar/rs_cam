//! P20e (feeds contract, 2026-11-08): one steady feed for the op cap, the
//! gates and the Suggest power figure.
//!
//! Defect (`planning/feeds_contract_2026-11-03/phases/P20_chip_gate_abstain.md`):
//! the op dressup `feed_max_rate` caps every modulated move, but the chip
//! gate steady-state filter compared each sample feed with 0.95 x the
//! UNCAPPED op `feed_rate`. On an op with `feed_max_rate < 0.95 x feed_rate`
//! no sample passed, and the gate returned `SteadyStateSamplesNotPresent`.
//! The Suggest power figure also read the uncapped op feed.
//!
//! Claims:
//!
//! - (a) An op with `feed_rate` 8475 and `feed_max_rate` 3000 grades its
//!   chip. Two arms: a hand-built trace at the cap, and a session pocket
//!   simulated twice (the second run reads the modulated, capped IR, as a
//!   GUI re-simulation does).
//! - (b) An all-ramp op at its ramp feed stays unmodeled
//!   (`SteadyStateSamplesNotPresent`).
//! - (c) The Suggest power figure of a capped op is the figure at the cap,
//!   and it names the op feed cap (`shipped_power_json` `feed_limit`).
//! - (d) The modulator cap and the gate reference come from one function,
//!   `MachineProfile::op_cutting_ceiling`: the modulated feeds stay at or
//!   below it, the gate reference equals it, and the source of both paths
//!   calls it.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
#[path = "common/source_scan.rs"]
mod source_scan;

use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolMaterial, ToolType};
use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::machine::{MachineProfile, SteadyFeedLimit};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetMachineArgs, SimulationOptions,
};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChiploadVerdict, UnmodeledReason};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext, chipload};

/// The wanaka "Back bulk rough 18 mm" op feed and cap
/// (`P20_chip_gate_abstain.md` section 3).
const OP_FEED: f64 = 8475.0;
const CAP: f64 = 3000.0;
const RAMP_FEED: f64 = 1000.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
const TP: ToolpathId = ToolpathId::from_raw(0);
/// The chip gate's steady-state fraction (`chipload.rs`,
/// `STEADY_STATE_FEED_FRACTION`, crate-private).
const STEADY_FRACTION: f64 = 0.95;

/// Non-vacuity of arm (a): a sample at the cap is below 0.95 x the stated
/// op feed, so the old reference rejects every one.
const _CAP_BELOW_OLD_REFERENCE: () = assert!(CAP < STEADY_FRACTION * OP_FEED);
/// Arm (b): the ramp feed is below 0.95 x the steady feed (the cap).
const _RAMP_IS_TRANSIENT: () = assert!(RAMP_FEED < STEADY_FRACTION * CAP);

/// A machine whose cutting ceiling is above `OP_FEED`, so the op cap is
/// the one limit that binds.
fn open_machine() -> MachineProfile {
    let mut machine = MachineProfile::shapeoko_makita();
    machine.max_feed_mm_min = 4.0 * OP_FEED;
    machine.max_cutting_feed_mm_min = Some(2.0 * OP_FEED);
    machine
}

fn gate_tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.35, 20.0)),
        6.35,
        30.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// One in-cut sample at `feed` (mm/min).
fn sample(idx: usize, feed: f64) -> SimulationCutSample {
    SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [f64::from(u32::try_from(idx).expect("small")), 0.0, -1.0],
        cumulative_time_s: 0.1 * f64::from(u32::try_from(idx).expect("small")),
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: feed,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: 1.0,
        axial_engagement_mm: 1.0,
        arc_engagement_radians: Some(1.2),
        chipload_mm_per_tooth: feed / (f64::from(RPM) * f64::from(FLUTES)),
        effective_chip_thickness_mm: Some(feed / (f64::from(RPM) * f64::from(FLUTES))),
        engagement: Engagement::with_radial_woc(0.4),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace_at(feed: f64) -> SimulationCutTrace {
    SimulationCutTrace {
        samples: (0..12).map(|i| sample(i, feed)).collect(),
        sample_step_mm: 1.0,
        ..SimulationCutTrace::test_fixture()
    }
}

/// The chip verdict of a hand-built trace, with the gate reference built
/// by THE door (`MachineProfile::op_steady_feed`).
fn chip_verdict(trace: &SimulationCutTrace, feed_max_rate: Option<f64>) -> ChiploadVerdict {
    let tool = gate_tool();
    let material = hardwood();
    let machine = open_machine();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: OP_FEED,
        steady_feed: machine.op_steady_feed(OP_FEED, feed_max_rate),
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
        commanded_thinning_lift: None,
        radial_stock_to_leave_mm: None,
    };
    let env = GateEnv {
        sim_trace: Some(trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    chipload::evaluate(&ctx, &env)
}

fn is_steady_abstain(v: &ChiploadVerdict) -> bool {
    matches!(
        v,
        ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SteadyStateSamplesNotPresent
        }
    )
}

#[test]
fn a_capped_op_grades_its_chip_on_a_hand_built_trace_a() {
    let machine = open_machine();
    let steady = machine.op_steady_feed(OP_FEED, Some(CAP));
    assert_eq!(steady.mm_min(), CAP, "the cap binds");
    assert_eq!(steady.limit(), SteadyFeedLimit::OpFeedCap);
    assert_eq!(steady.op_feed_mm_min(), OP_FEED);

    let verdict = chip_verdict(&trace_at(CAP), Some(CAP));
    assert!(
        !is_steady_abstain(&verdict),
        "a capped op must grade its chip, not abstain: {verdict:?}"
    );
}

#[test]
fn an_all_ramp_op_stays_unmodeled_b() {
    // Every in-cut sample runs at the ramp feed, below 0.95 x the steady
    // feed (the cap): `RAMP_IS_TRANSIENT`.
    let verdict = chip_verdict(&trace_at(RAMP_FEED), Some(CAP));
    assert!(
        is_steady_abstain(&verdict),
        "an all-ramp op must stay SteadyStateSamplesNotPresent: {verdict:?}"
    );
}

fn end_mill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    tool.name = "End Mill 6mm (P20e)".to_owned();
    tool
}

fn rect_with_island() -> Polygon2 {
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

/// The F115 pocket (G-FEEDMAX fixture) at `feed`, with `feed_max_rate`.
fn pocket_session(feed: f64, feed_max_rate: Option<f64>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: 100.0,
        y: 100.0,
        z: Some(12.0),
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
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(common::session::polygon_model(
        vec![rect_with_island()],
        "p20e_pocket",
    ));
    let pocket = PocketConfig {
        stepover: Some(2.0),
        floor: rs_cam_core::compute::floor::Floor::top(4.0),
        depth_per_pass: Some(2.0),
        feed_rate: Some(feed),
        plunge_rate: Some(385.0),
        climb: true,
        pattern: PocketPattern::Contour,
        spindle_rpm: Some(RPM),
        selection: rs_cam_core::selection::Selection::drawing(),
        ..PocketConfig::default()
    };
    let mut tc = common::session::toolpath_config(
        "Pocket",
        OperationConfig::Pocket(pocket),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    tc.dressups.feed_max_rate = feed_max_rate;
    let _ = builder.add_toolpath(0, tc).expect("add the pocket");
    let mut session = builder.build();
    let mut machine = open_machine();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    // The XXL axis rates sit below the op feed; the stated cutting cap
    // keeps the machine ceiling above it, so only the op cap binds.
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    session
}

fn simulate(session: &mut ProjectSession) -> std::sync::Arc<SimulationCutTrace> {
    let opts = SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(1.0),
        adaptive_feed_modulation: true,
        modulation_strategy: ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
        ..SimulationOptions::default()
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation completes")
        .cut_trace
        .clone()
        .expect("the simulation records a trace")
}

fn fed_feeds(session: &ProjectSession) -> Vec<f64> {
    session
        .result_at(0)
        .expect("generated")
        .toolpath()
        .moves
        .iter()
        .filter_map(|m| m.move_type.feed_rate())
        .collect()
}

/// (a) on a session, and (d) on both paths: the second simulation reads the
/// modulated IR (every cut at or below the cap); the gate grades it, and the
/// modulator cap and the gate reference are the one ceiling.
#[test]
fn a_capped_session_op_grades_its_chip_and_the_cap_is_one_function_a_d() {
    let mut session = pocket_session(OP_FEED, Some(CAP));
    let _ = session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("generate the pocket");
    let _ = simulate(&mut session);

    let ceiling = session.machine().op_cutting_ceiling(Some(CAP));
    assert_eq!(ceiling.mm_min(), CAP, "the op cap binds the ceiling");

    // (d) path 1, the modulator: no fed move is above THE op ceiling.
    let feeds = fed_feeds(&session);
    let max_feed = feeds.iter().copied().fold(0.0, f64::max);
    assert!(
        max_feed <= ceiling.mm_min() + 1e-6,
        "a modulated feed {max_feed} is above the op ceiling {}",
        ceiling.mm_min()
    );

    // (d) path 2, the gate: the reference is THE op ceiling's steady feed.
    let (tp_id, steady) = {
        let parts = session.load_context_parts(0).expect("the op has a context");
        (parts.toolpath_id, parts.ctx().steady_feed)
    };
    assert_eq!(steady, ceiling.steady_feed(OP_FEED));
    assert_eq!(steady.mm_min(), CAP);
    assert_eq!(steady.limit(), SteadyFeedLimit::OpFeedCap);

    // A GUI re-simulation reads the modulated IR, so the samples carry the
    // capped feeds.
    let trace = simulate(&mut session);
    let in_cut: Vec<f64> = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == tp_id && !s.is_air())
        .map(|s| s.feed_rate_mm_min)
        .collect();
    assert!(!in_cut.is_empty(), "the pocket cuts material");
    // Non-vacuity: the old reference (0.95 x the op feed) rejects every
    // in-cut sample.
    assert!(
        in_cut.iter().all(|f| *f < STEADY_FRACTION * OP_FEED),
        "a sample runs above 0.95 x the op feed; the claim is vacuous"
    );
    assert!(
        in_cut.iter().any(|f| *f >= STEADY_FRACTION * CAP),
        "no sample runs near the cap; the fixture does not reach it"
    );
    let report = session.tool_load_report();
    let verdict = &report.per_toolpath[0].chipload;
    assert!(
        !is_steady_abstain(verdict),
        "the capped pocket must grade its chip: {verdict:?}"
    );
}

/// (d) the source of both paths calls the one function.
#[test]
fn the_modulator_and_the_gate_call_one_ceiling_d() {
    let root = env!("CARGO_MANIFEST_DIR");
    // Comments stripped and every space removed, so neither a comment nor
    // a rustfmt line break decides the match.
    let code = |p: &str| -> String {
        let src = std::fs::read_to_string(format!("{root}/{p}")).expect("the source file");
        source_scan::strip_comments(&src)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    };
    let needle = |n: &str| -> String { n.chars().filter(|c| !c.is_whitespace()).collect() };
    let sim = code("src/session/compute/simulation.rs");
    assert!(
        sim.contains(&needle(
            "let max_feed = self.machine.op_cutting_ceiling(tc.dressups.feed_max_rate);"
        )),
        "the modulator cap must call MachineProfile::op_cutting_ceiling"
    );
    let diag = code("src/session/compute/toolpath_diagnostics.rs");
    assert!(
        diag.contains(&needle(
            "steady_feed: self.machine.op_steady_feed(op_feed, tc.dressups.feed_max_rate)"
        )),
        "the gate reference must call MachineProfile::op_steady_feed"
    );
    let machine = code("src/machine/mod.rs");
    assert!(
        machine.contains(&needle(
            "-> SteadyFeedMmMin { self.op_cutting_ceiling(feed_max_rate).steady_feed(op_feed_mm_min) }"
        )),
        "op_steady_feed must go through op_cutting_ceiling"
    );
}

/// (c) the Suggest power figure of a capped op is the figure at the cap,
/// and it names the op feed cap on the MCP / CLI shape.
#[test]
fn the_suggest_power_figure_grades_the_capped_feed_c() {
    // A cap far below any Suggest feed for this cut, so it binds on the
    // shipped operation whatever feed Suggest ships.
    let cap = 100.0;
    let session = pocket_session(OP_FEED, Some(cap));
    let tc = session.toolpath_configs().last().expect("one toolpath");
    let profile = session.cutter_op_profile(tc).expect("the tool resolves");
    let shipped_feed = profile
        .shipped_operation()
        .feed_rate()
        .expect("Suggest ships a feed");
    assert!(shipped_feed > cap, "the cap must bind: {shipped_feed}");

    let figure = profile
        .shipped_power()
        .expect("Suggest serves the fixture")
        .expect("the pocket has a power figure");
    assert_eq!(figure.feed_mm_min, cap, "the figure grades the cap");
    let steady = figure.steady_feed.expect("the shipped door names the feed");
    assert_eq!(steady.limit(), SteadyFeedLimit::OpFeedCap);
    assert_eq!(steady.op_feed_mm_min(), shipped_feed);

    // The figure equals the power of the same cut at the cap.
    let mut at_cap = profile.shipped_operation().clone();
    at_cap.set_feed_rate(cap);
    let feeds = profile.feeds.as_ref().expect("Suggest serves the fixture");
    let reference = rs_cam_core::feeds::power_at_operating_point(
        &at_cap,
        profile.tool_cfg,
        profile.material,
        profile.machine,
        Some(rs_cam_core::feeds::suggest::CalculatorOperatingPoint::of(
            feeds,
        )),
    )
    .expect("the cut at the cap has a figure");
    assert!(
        (figure.required_kw - reference.required_kw).abs() <= 1e-12,
        "shipped {} kW, at the cap {} kW",
        figure.required_kw,
        reference.required_kw
    );
    // Non-vacuity: the figure at the stated op feed is larger.
    let uncapped = rs_cam_core::feeds::power_at_operating_point(
        profile.shipped_operation(),
        profile.tool_cfg,
        profile.material,
        profile.machine,
        Some(rs_cam_core::feeds::suggest::CalculatorOperatingPoint::of(
            feeds,
        )),
    )
    .expect("the cut at the op feed has a figure");
    assert!(uncapped.required_kw > figure.required_kw);

    // The MCP `get_suggest_rationale` / CLI `suggest` shape.
    let json = rs_cam_core::feeds::operating_point::shipped_power_json(&Ok(figure));
    assert_eq!(json["feed_limit"], "op_feed_cap", "{json}");
    assert_eq!(json["feed_mm_min"].as_f64(), Some(cap), "{json}");
    assert_eq!(
        json["op_feed_mm_min"].as_f64(),
        Some(shipped_feed),
        "{json}"
    );

    // With no cap the op feed binds and the figure is the uncapped one.
    let open = pocket_session(OP_FEED, None);
    let tc = open.toolpath_configs().last().expect("one toolpath");
    let profile = open.cutter_op_profile(tc).expect("the tool resolves");
    let figure = profile
        .shipped_power()
        .expect("Suggest serves the fixture")
        .expect("the pocket has a power figure");
    assert_eq!(
        figure.steady_feed.map(|s| s.limit()),
        Some(SteadyFeedLimit::OpFeed)
    );
}
