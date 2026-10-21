//! F115 — the feed modulation runs inside `Command::AdoptSimulation`, and
//! it writes the same feeds that `run_simulation` writes.
//!
//! The GUI and MCP path simulate on a lane and adopt the answer. It used
//! to call the public `modulate_simulation_trace` on the frame loop, which
//! wrote `session.results` outside `ProjectSession::apply`. The pass now
//! runs in the `AdoptSimulation` arm, after the epoch check and before
//! the store, which is the order `run_simulation` (the CLI path) uses.
//!
//! Two claims:
//!
//! 1. Parity: a run adopted with a `FeedModulation` leaves the same
//!    toolpath moves in the result cache as `run_simulation` with
//!    modulation on. `Effects::modulated` names the rewritten index, and
//!    the slot is `Modulated`. The modulation must change a feed, or the
//!    claim is vacuous.
//! 2. A stale run modulates nothing: the epoch refusal comes first.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
use rs_cam_core::geo::P2;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AdoptSimulationArgs, Command, FeedModulation, ProjectSession, ProjectSessionBuilder,
    ResultProvenance, SessionError, SetMachineArgs, SimulationOptions,
};

const MODULATION: FeedModulation = FeedModulation {
    strategy: ModulationStrategy::ConstrainedMax,
    feed_scale: 1.0,
};

fn end_mill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (F115)".to_owned();
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

/// The AS001 pocket of F-036b, generated, on a machine with a kinematics
/// block so the modulation pass changes feeds.
fn generated_pocket() -> ProjectSession {
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
    let model_id = builder.add_model(common::session::polygon_model(
        vec![rect_with_island()],
        "as001_pocket",
    ));
    let pocket = PocketConfig {
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(6.0),
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        spindle_rpm: Some(18_000),
        selection: rs_cam_core::selection::Selection::drawing(),
        ..PocketConfig::default()
    };
    let tc = common::session::toolpath_config(
        "Pocket",
        OperationConfig::Pocket(pocket),
        tool_id,
        model_id,
    );
    let _ = builder.add_toolpath(0, tc).expect("add the pocket");
    let mut session = builder.build();
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    let _ = session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("generate the pocket");
    session
}

fn simulate(session: &mut ProjectSession, modulate: bool) {
    let opts = SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(1.0),
        adaptive_feed_modulation: modulate,
        modulation_strategy: MODULATION.strategy,
        modulation_feed_scale: MODULATION.feed_scale,
        ..SimulationOptions::default()
    };
    let _ = session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the simulation completes");
}

fn moves_of(session: &ProjectSession) -> String {
    format!(
        "{:?}",
        session.get_result(0).expect("generated").toolpath().moves
    )
}

/// Claim 1.
#[test]
fn the_adopt_writes_the_feeds_run_simulation_writes() {
    // The CLI path: `run_simulation` modulates and stores.
    let mut cli = generated_pocket();
    simulate(&mut cli, true);
    let cli_moves = moves_of(&cli);

    // The lane path: an unmodulated run, then the same answer adopted with
    // a modulation, as the GUI adopts its lane result.
    let mut lane = generated_pocket();
    simulate(&mut lane, false);
    let planned_moves = moves_of(&lane);
    assert_ne!(
        planned_moves, cli_moves,
        "the fixture must modulate a feed, or the parity below is vacuous"
    );
    let result = lane
        .simulation_result()
        .expect("the unmodulated run is stored")
        .clone();
    let epoch = lane.simulation_epoch();
    let effects = lane
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(result),
            epoch,
            modulation: Some(MODULATION),
        }))
        .expect("the adopt is at the live epoch");

    assert_eq!(
        effects.modulated.iter().copied().collect::<Vec<_>>(),
        vec![0],
        "the answer names the index whose result the pass rewrote"
    );
    assert!(effects.stale.is_empty(), "a modulation moves no revision");
    assert!(!effects.simulation_cleared);
    assert_eq!(
        moves_of(&lane),
        cli_moves,
        "the adopt and run_simulation must write the same moves"
    );
    assert!(matches!(
        lane.get_result(0).expect("generated").provenance,
        ResultProvenance::Modulated { .. }
    ));
    let trace = lane
        .simulation_result()
        .and_then(|stored| stored.cut_trace.as_ref())
        .expect("the stored run keeps its trace");
    assert!(
        !trace.modulation_summaries.is_empty(),
        "the stored trace is the modulated one"
    );
}

/// Claim 2.
#[test]
fn a_stale_run_modulates_nothing() {
    let mut lane = generated_pocket();
    simulate(&mut lane, false);
    let planned_moves = moves_of(&lane);
    let result = lane
        .simulation_result()
        .expect("the unmodulated run is stored")
        .clone();
    let stale_epoch = lane.simulation_epoch().wrapping_add(1);
    let refused = lane.apply(Command::AdoptSimulation(AdoptSimulationArgs {
        result: Box::new(result),
        epoch: stale_epoch,
        modulation: Some(MODULATION),
    }));
    assert!(matches!(refused, Err(SessionError::StaleSimulation { .. })));
    assert_eq!(
        moves_of(&lane),
        planned_moves,
        "a refused adopt writes no feed"
    );
}
