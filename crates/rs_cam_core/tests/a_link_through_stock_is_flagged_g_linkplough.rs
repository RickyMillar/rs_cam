//! G-LINKPLOUGH — a fed link that cuts stock is flagged (W1.1b, ruling R1).
//!
//! # The gap
//!
//! A link is a short FED move between two cut runs (`MoveIntent::Linking`).
//! The planner means it to travel in air or on ground the op already cut.
//! Nothing read a fed link against the stock: the rapid check reads rapids
//! only, and the holder check reads the holder against the MODEL. The
//! G-LINKCEIL defect (877c93e5: Project Curve put every fed link inside a
//! 25 mm stock) was silent on every surface.
//!
//! # The rule
//!
//! `stock::sim_triage::link_cut_observation` reads the link samples of the
//! cut trace. The bar is the larger of the noise floor
//! (`FRESH_MATERIAL_THRESHOLD_MM` + one cell) and the pass's own bite (the
//! commanded step + one cell, else 3x the median bite). A link over the bar
//! raises `project.link_cuts_stock`: Caution, Critical at 1 mm over the bar.
//!
//! # The cases
//!
//! 1. (a) A hand-built toolpath on a 10 mm block, through the real dexel
//!    simulator: a 0.5 mm engrave, then a fed link 5 mm under the stock top.
//!    The finding fires on the link move, with a 5 mm removed height. A
//!    control link that rides the cut slot stays silent.
//! 2. (b) Through `ProjectSession`, on a corrugated plate:
//!    - a slope-split raster finish with surface links, after a 3D Rough,
//!      on the remaining stock: no link sample is over the bar, and the
//!      triage (the 3D Rough included) is silent;
//!    - the raster and a pencil on FRESH stock: their links ride the mesh
//!      through raw stock (the observation exceeds the bar), and the triage
//!      leaves them out, because a finishing op builds no link ceiling on
//!      fresh stock by design (`links_on_mesh`);
//!    - a 2D Adaptive whose keep-down links are ruled to cut within the pass
//!      load (G-ADAPTLINKLOAD): its links cut its own 3 mm level, and the
//!      triage is silent under the commanded bar (step + one cell).
//!
//!    Each arm asserts that it has fed link samples.
//! 3. (c) Red proof, done once by hand: with the call in
//!    `SimulationTriage::build_with_rest_context` removed, case (a) fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_link_through_stock_is_flagged_g_linkplough
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use common::meshes::sawtooth_plate;
use common::session::{mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, DropCutterConfig, PencilConfig};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::diagnostics::{DiagnosticEvidence, Severity, ids};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    Command, ProjectEvidence, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs,
    SimulationOptions, SimulationResolution,
};
use rs_cam_core::stock::sim_measurability::MeasurabilityReport;
use rs_cam_core::stock::sim_triage::{
    Finding, LinkCutObservation, SimulationTriage, TriageInputs, link_cut_noise_floor_mm,
    link_cut_observation,
};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// ── Case (a): a hand-built link through a block ─────────────────────────

const TP: ToolpathId = ToolpathId(7);
/// The block: 10 mm tall, top at Z 10.
const STOCK_TOP_Z: f64 = 10.0;
/// The dexel cell, mm.
const CELL_MM: f64 = 0.25;
/// The trace sample step, mm: fine enough that each pass has more than the
/// 50 body bites the own-median reference needs.
const SAMPLE_STEP_MM: f64 = 0.1;
/// The engrave depth of the two cut runs, mm.
const ENGRAVE_MM: f64 = 0.5;
/// The Z of the ploughing link: 5 mm under the stock top.
const LINK_Z: f64 = 5.0;

fn block() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(27.0, 5.0, STOCK_TOP_Z),
    };
    TriDexelStock::from_bounds(&bbox, CELL_MM)
}

/// Two engrave runs, X 0..8 and X 14..22 at Z 9.5, joined by fed links.
///
/// `plough`: the links go down to Z 5 at X 8, across to X 14 THROUGH the
/// block, and up to the second run. Else the second run is cut first, and
/// the links ride back along the cut slot at the engrave Z. Returns the
/// toolpath and the index of the lateral link move.
fn engrave_with_link(plough: bool) -> (Toolpath, usize) {
    let z_cut = STOCK_TOP_Z - ENGRAVE_MM;
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, STOCK_TOP_Z + 2.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, z_cut), 300.0, MoveIntent::EntryPlunge);
    if plough {
        tp.feed_to_with_intent(P3::new(8.0, 0.0, z_cut), 1000.0, MoveIntent::FinishingCut);
        tp.feed_to_with_intent(P3::new(8.0, 0.0, LINK_Z), 1000.0, MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(14.0, 0.0, LINK_Z), 1000.0, MoveIntent::Linking);
        let lateral = tp.moves.len() - 1;
        tp.feed_to_with_intent(P3::new(14.0, 0.0, z_cut), 1000.0, MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(22.0, 0.0, z_cut), 1000.0, MoveIntent::FinishingCut);
        (tp, lateral)
    } else {
        // Cut the whole slot X 0..22, then link back along it to X 0 and
        // cut the slot again: the link rides cut ground.
        tp.feed_to_with_intent(P3::new(22.0, 0.0, z_cut), 1000.0, MoveIntent::FinishingCut);
        tp.feed_to_with_intent(P3::new(11.0, 0.0, z_cut), 1000.0, MoveIntent::Linking);
        let lateral = tp.moves.len() - 1;
        tp.feed_to_with_intent(P3::new(0.0, 0.0, z_cut), 1000.0, MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(22.0, 0.0, z_cut), 1000.0, MoveIntent::FinishingCut);
        (tp, lateral)
    }
}

/// Simulate `tp` on the block through the dexel engine and wrap the samples
/// as a cut trace.
fn simulate(tp: &Toolpath) -> SimulationCutTrace {
    let mut stock = block();
    let cutter = FlatEndmill::new(2.0, 25.0);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            tp,
            &cutter,
            StockCutDirection::FromTop,
            TP,
            18_000,
            2,
            5000.0,
            SAMPLE_STEP_MM,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation completes");
    SimulationCutTrace::from_samples(SAMPLE_STEP_MM, samples)
}

/// The link finding of the triage on `trace`, if any. `step` is the
/// commanded depth per pass of `TP`; `None` reads the own-median arm.
fn link_finding(trace: &SimulationCutTrace, step: Option<f64>) -> Option<Finding> {
    let measurability = MeasurabilityReport {
        cell_mm: Some(CELL_MM),
        ..MeasurabilityReport::default()
    };
    let steps: BTreeMap<ToolpathId, f64> = step.map(|s| (TP, s)).into_iter().collect();
    let names: BTreeMap<ToolpathId, String> = std::iter::once((TP, "Engrave".to_owned())).collect();
    let triage = SimulationTriage::build(&TriageInputs {
        trace,
        measurability: &measurability,
        diagnostics: &[],
        rapid_collisions: &[],
        toolpath_names: &names,
        holder_collisions: &[],
        tool_diameters_mm: &BTreeMap::new(),
        commanded_depth_per_pass_mm: &steps,
        kinematic_utilization: &BTreeMap::new(),
        region_of: None,
        obstacle_collisions: &[],
    });
    triage
        .actions
        .into_iter()
        .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_LINK_CUTS_STOCK)
}

#[test]
fn a_link_through_the_block_is_flagged_with_its_move_and_height() {
    let (tp, lateral) = engrave_with_link(true);
    let trace = simulate(&tp);

    // The plumbing: the simulator carries the link intent onto fed samples
    // and writes the bite there, on a sample the load gates call transit.
    let obs = link_cut_observation(&trace, TP, None, Some(CELL_MM));
    eprintln!("G-LINKPLOUGH (a) plough: {obs:?}");
    assert!(obs.link_samples > 0, "the fixture has fed link samples");
    assert!(
        obs.body_samples >= 50,
        "the own-median arm needs a body: {obs:?}"
    );

    let expected = STOCK_TOP_Z - LINK_Z;
    for step in [None, Some(1.0)] {
        let f = link_finding(&trace, step)
            .unwrap_or_else(|| panic!("step {step:?}: a 5 mm link plough must fire"));
        assert_eq!(
            f.worst.move_index, lateral,
            "step {step:?}: the lateral link"
        );
        let Some(DiagnosticEvidence::SampleRange {
            observed,
            threshold,
            unit,
            ..
        }) = &f.diagnostic.evidence
        else {
            panic!("SampleRange evidence, got {:?}", f.diagnostic.evidence);
        };
        assert!(
            (observed - expected).abs() <= CELL_MM,
            "step {step:?}: removed height {observed} against {expected} +- one cell"
        );
        assert_eq!(unit, "mm");
        let bar = threshold.expect("the bar is published");
        assert!(bar >= link_cut_noise_floor_mm(Some(CELL_MM)));
        assert_eq!(
            f.diagnostic.severity,
            Severity::Critical,
            "step {step:?}: {observed} mm is more than 1 mm over the {bar} mm bar"
        );
        let msg = &f.diagnostic.message;
        assert!(msg.contains("\"Engrave\""), "names the toolpath: {msg}");
        assert!(
            msg.contains(&format!("link move {lateral} ")),
            "names the move: {msg}"
        );
        assert!(
            msg.contains(&format!("removes {observed:.2} mm")),
            "names the height: {msg}"
        );
        // The position is on the lateral link, at the link Z.
        assert!(
            (f.worst.position[2] - LINK_Z).abs() < 1e-6,
            "{:?}",
            f.worst.position
        );
        assert!((8.0..=14.0).contains(&f.worst.position[0]));
    }
}

#[test]
fn a_link_that_rides_the_cut_slot_is_silent() {
    let (tp, _) = engrave_with_link(false);
    let trace = simulate(&tp);
    let obs = link_cut_observation(&trace, TP, None, Some(CELL_MM));
    eprintln!("G-LINKPLOUGH (a) control: {obs:?}");
    assert!(obs.link_samples > 0, "the control has fed link samples");
    assert!(obs.is_measured(), "the control is measured: {obs:?}");
    let f = link_finding(&trace, None);
    assert!(
        f.is_none(),
        "a link on the cut slot must not fire: {:?}",
        f.map(|f| f.diagnostic.message)
    );
}

// ── Case (b): finishing ops with surface links, through the session ─────

const HALF_MM: f64 = 20.0;
const PERIOD_MM: f64 = 6.0;
const AMPLITUDE_MM: f64 = 2.0;
const BALL_DIAMETER_MM: f64 = 3.0;
/// The stock top: 4 mm over the corrugation crests.
const PLATE_STOCK_Z: f64 = 6.0;
/// The session's simulation cell, mm.
const SESSION_CELL_MM: f64 = 0.25;

fn raster() -> OperationConfig {
    // The slope filter splits each row into short runs, so the link stage
    // has junctions to join (the G-LINKVISIBLE drop-cutter fixture).
    OperationConfig::DropCutter(DropCutterConfig {
        stepover: 0.5,
        slope_from: 0.0,
        slope_to: 20.0,
        hookup_mm: 3.0,
        ..DropCutterConfig::default()
    })
}

fn pencil() -> OperationConfig {
    OperationConfig::Pencil(PencilConfig {
        hookup_distance: PERIOD_MM * 1.5,
        ..PencilConfig::default()
    })
}

/// A session on the corrugated plate. With `rough`, a 3D Rough runs first
/// and the finish reads the remaining stock.
fn plate_session(finish: OperationConfig, rough: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(HALF_MM, PLATE_STOCK_Z));
    let tool_idx = builder.add_tool(ball_tool_config(BALL_DIAMETER_MM));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(mesh_model(
        sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
        "corrugated",
    ));
    if rough {
        let mut cfg = toolpath_config(
            "3D Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            tool_id,
            model_id,
        );
        cfg.heights = pinned_heights(PLATE_STOCK_Z, -1.0);
        let _ = builder.add_toolpath(0, cfg).expect("add rough");
    }
    let mut cfg = toolpath_config("Finish", finish, tool_id, model_id);
    cfg.heights = pinned_heights(2.0, -2.0);
    if rough {
        cfg.stock_source = StockSource::FromRemainingStock;
    }
    let _ = builder.add_toolpath(0, cfg).expect("add finish");
    let mut session = builder.build();
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(SESSION_CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    let count = session.toolpath_configs().len();
    for i in 0..count {
        session
            .generate_toolpath(i, &cancel)
            .expect("generation succeeds");
        session
            .run_simulation(
                &SimulationOptions {
                    resolution: SESSION_CELL_MM,
                    auto_resolution: false,
                    adaptive_feed_modulation: false,
                    ..SimulationOptions::default()
                },
                &cancel,
            )
            .expect("simulation completes");
    }
    session
}

/// The link findings of the session triage, the GUI, MCP and CLI door.
fn session_link_findings(session: &ProjectSession) -> Vec<String> {
    let sim = session.simulation_result().expect("a simulation");
    let evidence = ProjectEvidence::from_simulation_with_holder_collisions(sim, Vec::new());
    session
        .simulation_triage(&evidence)
        .actions
        .iter()
        .filter(|f| f.diagnostic.id.as_str() == ids::PROJECT_LINK_CUTS_STOCK)
        .map(|f| f.diagnostic.message.clone())
        .collect()
}

/// The link observation of the finish (the last toolpath). It asserts the
/// population: an arm with no fed link sample tests nothing.
fn finish_observation(name: &str, session: &ProjectSession) -> LinkCutObservation {
    let sim = session.simulation_result().expect("a simulation");
    let trace = sim.cut_trace.as_deref().expect("a cut trace");
    let finish_id = session.toolpath_configs().last().expect("finish").id;
    let obs = link_cut_observation(trace, finish_id, None, Some(SESSION_CELL_MM));
    eprintln!("G-LINKPLOUGH (b) {name}: {obs:?}");
    assert!(
        obs.link_samples > 0 && obs.is_measured(),
        "{name}: the finish has no measured fed link, so the arm tests nothing: {obs:?}"
    );
    obs
}

/// On remaining stock the raster builds a link ceiling from the stock, and
/// its links ride the cut ground: no link sample is over the bar, and the
/// session triage (which grades every toolpath, the 3D Rough too) is silent.
#[test]
fn a_raster_finish_on_remaining_stock_is_silent() {
    let session = plate_session(raster(), true);
    let obs = finish_observation("raster, remaining", &session);
    assert!(
        !obs.exceeds(),
        "a raster link under its stock ceiling cut stock: {obs:?}"
    );
    let hits = session_link_findings(&session);
    assert!(hits.is_empty(), "raster on remaining stock fired: {hits:?}");
}

/// On fresh stock a finishing op builds no link ceiling by design
/// (`LinkCeilingNeed::WithRemainingStock`): its links ride the mesh through
/// the same raw stock its passes cut. The observation shows that the links
/// do cut (so the arm is not vacuous), and the triage leaves them out of
/// the rule: the op, not its link, meets the raw stock.
#[test]
fn a_finishing_op_on_fresh_stock_is_silent() {
    for (name, op) in [("raster, fresh", raster()), ("pencil, fresh", pencil())] {
        let session = plate_session(op, false);
        let obs = finish_observation(name, &session);
        assert!(
            obs.exceeds(),
            "{name}: control: the fresh-stock links ride the mesh through raw \
             stock, so the observation exceeds the bar: {obs:?}"
        );
        let hits = session_link_findings(&session);
        assert!(hits.is_empty(), "{name}: fired: {hits:?}");
    }
}

/// A 2D Adaptive keep-down link is RULED to cut within the pass load
/// (G-ADAPTLINKLOAD). It cuts no deeper than its own level, so the
/// commanded bar (the step plus one cell) holds it: the triage is silent.
#[test]
fn a_2d_adaptive_keep_down_link_is_silent() {
    let session = common::adaptive_islands::adaptive_session(false);
    let trace = common::adaptive_islands::trace_of(&session);
    let id = session.toolpath_configs()[0].id;
    let obs = link_cut_observation(
        trace,
        id,
        None,
        Some(common::adaptive_islands::SIM_RESOLUTION_MM),
    );
    eprintln!("G-LINKPLOUGH 2D Adaptive (own-median arm): {obs:?}");
    assert!(obs.link_samples > 0, "the fixture has keep-down links");
    let hits = session_link_findings(&session);
    assert!(
        hits.is_empty(),
        "2D Adaptive keep-down links fired: {hits:?}"
    );
}
