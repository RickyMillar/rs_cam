//! The shared fixture for the two limits-surface sentries (V1, V2, V3).
//!
//! # Why one simulated project and not a hand-built report
//!
//! Every bound the surface paints comes from the gate that judged it
//! (`CriterionStatus::bound`, S4). A hand-built `ToolLoadReport` would let a
//! test agree with itself: the fixture would carry the bound the assertion
//! expects. So this module runs the SHIPPED path — `generate_toolpath`, then
//! `run_simulation` with metrics, then `gcode::project_load_report` through
//! the panel's own producer. The bounds the assertions read are the ones the
//! shipped gates set.
//!
//! The freshness gate is the reason the trace cannot be hand-built either.
//! `gcode::sim_trace_is_fresh` compares `SimulationProvenance::toolpath_hashes`
//! against `hash_toolpath` of each enabled toolpath's cached result, and
//! `hash_toolpath` is `pub(crate)`. A trace this crate could assemble would
//! therefore always read STALE, and every row would paint `—` for a reason
//! that has nothing to do with the code under test.
//!
//! # Layout
//!
//! Cargo builds one test binary per `.rs` file DIRECTLY under `tests/`. This
//! is a directory pulled in with `mod limits_fixture;`, so it compiles into
//! each consuming binary and is never a target of its own — the same
//! convention `rs_cam_core/tests/common/` uses.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, SimulationOptions, ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;
use rs_cam_viz::state::simulation::{SimulationResults, SimulationState};
use rs_cam_viz::state::{AppState, Workspace};
use rs_cam_viz::ui::tokens;

/// The feed the fixture cuts at, in mm/min.
///
/// At the project's 18 000 rpm and the Ø6 end mill's two flutes this is
/// 0.0417 mm/tooth, inside the matched vendor band (floor 0.0320). The
/// default 1000 mm/min sits below the floor and trips the chipload gate,
/// which would refuse the export the depth arm checks is NOT refused.
const BAND_FEED_MM_MIN: f64 = 1_225.0;

/// Room for the whole panel, so nothing the arms read is clipped away.
/// Tall enough for the Inspector with every section forced open: the five
/// cut-metric cards (sim-cut-metrics package C, 2026-09-23) take about
/// 1000 px on top of the Project and Findings sections. egui does not
/// paint a run below the screen, so a short screen reads as a missing row.
pub const SCREEN: egui::Vec2 = egui::Vec2::new(900.0, 3200.0);

/// The Readiness workspace's centred column, pinned at `app.rs`'s
/// `set_max_width(560.0)`. The Readiness rows must fit the geometry the
/// workspace actually gives them, so the sentry draws them in it.
pub const READINESS_COLUMN: f32 = 560.0;

/// The shipped demo project: one 6 mm two-flute end mill, one SVG model,
/// generic softwood stock. It is the fixture the F2.4 phantom-axial detector
/// generates and simulates, so it is known to produce steady-state cutting
/// samples.
fn project_path() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `crates/rs_cam_viz`; the repository root is two
    // levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test_data")
        .join("ux_2d_pocket.toml")
}

/// How deep the one pocket cuts per pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    /// A depth per pass under the machine rigidity cap. Every milling row is
    /// modelled and the depth row is `Within`.
    InsideTheRigidityCap,
    /// A hand-set depth per pass ABOVE the cap. The depth row exceeds; its
    /// bound is a `RigidityRuleOfThumb`, so the export gate does not refuse.
    PastTheRigidityCap,
}

/// The one enabled toolpath's depth per pass, in mm, for each arm.
///
/// The cap is `RigidityProfile::doc_roughing_factor × D`. Both figures are
/// derived from the project's own machine profile and tool at build time —
/// see [`rigidity_cap_mm`] — so neither arm carries a typed constant that a
/// profile change could silently invalidate.
fn depth_per_pass_for(cap_mm: f64, cut: Cut) -> f64 {
    match cut {
        // Two thirds of the cap: modelled, comfortably inside, and still a
        // positive peak (the non-vacuity the arms need).
        Cut::InsideTheRigidityCap => cap_mm * (2.0 / 3.0),
        // Twice the cap. The simulated engagement cannot reach the commanded
        // depth if the stock is thinner, so the arm that reads this checks
        // the painted percent rather than assuming one.
        Cut::PastTheRigidityCap => cap_mm * 2.0,
    }
}

/// The rigidity depth cap the project's own machine profile sets for a
/// roughing pocket at `diameter_mm`, in mm.
///
/// `RigidityProfile::depth_cap_mm` is the one producer of this figure, and
/// the depth gate and the Suggest clamp both read it. The fixture reads it
/// too, so no arm carries a typed cap that a profile change could invalidate.
pub fn rigidity_depth_cap_mm(session: &ProjectSession, diameter_mm: f64) -> f64 {
    session
        .machine()
        .rigidity
        .depth_cap_mm(
            rs_cam_core::feeds::OperationFamily::Pocket,
            rs_cam_core::feeds::PassRole::Roughing,
            diameter_mm,
        )
        .expect("a milling family has a rigidity depth cap")
        .cap_mm()
}

/// The cap for the one toolpath this fixture builds.
pub fn rigidity_cap_mm(session: &ProjectSession) -> f64 {
    rigidity_depth_cap_mm(session, 6.0)
}

/// Build the whole state: the demo project generated and simulated, with the
/// resulting trace and boundaries installed in the viz simulation state.
///
/// `sim_trace_is_fresh` returns false as soon as ANY enabled toolpath has no
/// compute result, so the fixture generates the one toolpath it adds before
/// it simulates. Without that every row would paint `—` for a reason that has
/// nothing to do with the code under test.
pub fn simulated_state(cut: Cut) -> AppState {
    let mut session = ProjectSession::load(&project_path()).expect("load ux_2d_pocket.toml");

    // The demo project ships `toolpaths = []`, so the fixture adds the one
    // operation it measures. A pocket in the Ø6 end mill: a roughing family,
    // which is the arm `RigidityProfile::doc_roughing_factor` bounds.
    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("the demo project defines a 6 mm end mill");
    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("the demo project loads demo_pocket.svg");

    let cap = rigidity_depth_cap_mm(&session, 6.0);
    let dpp = depth_per_pass_for(cap, cut);
    let operation = OperationConfig::Pocket(PocketConfig {
        // Four passes at the shallow depth, two at the deep one, so both arms
        // run steady-state cutting rather than one entry move.
        depth: cap * 8.0 / 3.0,
        depth_per_pass: dpp,
        // Inside the matched vendor chip band on both sides; see the
        // constant's own doc for the window it sits in.
        feed_rate: BAND_FEED_MM_MIN,
        ..PocketConfig::default()
    });
    let op_type = operation.op_type();
    let config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Limits fixture".to_owned(),
        enabled: true,
        operation,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _effects = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }))
        .expect("add the limits fixture toolpath");
    let keep = 0usize;

    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(keep, &cancel)
        .expect("generate the pocket");
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim_result = session.simulation_result().expect("a simulation result");
    let boundaries = sim_result.boundaries.clone();
    let cut_trace = sim_result.cut_trace.clone();
    let total_moves = sim_result.total_moves;
    let column_grid_cell_mm = sim_result.column_grid_cell_mm;
    assert!(
        cut_trace.is_some(),
        "the fixture must carry a metric cut trace, or every row paints \
         `simulation has not been run` and the arms below measure nothing"
    );

    let mut simulation = SimulationState::new();
    simulation.results = Some(SimulationResults {
        mesh: rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves,
        boundaries,
        setup_boundaries: Vec::new(),
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: session.stock_bbox(),
        cut_trace,
        cut_trace_path: None,
        column_grid_cell_mm,
        prior_stocks: HashMap::new(),
    });

    let mut state = AppState::new();
    state.session = session;
    state.simulation = simulation;
    state.workspace = Workspace::Readiness;
    state
}

/// The id of the one toolpath this fixture simulates.
pub fn only_toolpath(state: &AppState) -> ToolpathId {
    state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.enabled)
        .expect("one enabled toolpath")
        .id
}

/// An `egui::Context` that paints hovers and opens disclosures.
///
/// `set_everything_is_visible` is what makes a RENDERED hover readable: the
/// limit rows sit inside a `CollapsingHeader` that DC6 keeps closed, and the
/// provenance lives in an `on_hover_text`. Both reach the paint list under
/// this flag and neither does without it.
pub fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Run `body` through a real context and return every text run it painted.
///
/// Two passes: the first settles the layout and the disclosure state, the
/// second is the steady frame the arms read.
pub fn painted(mut body: impl FnMut(&mut egui::Ui)) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| body(ui));
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// Every text run the Simulation Inspector paints.
pub fn inspector_text(state: &mut AppState) -> Vec<String> {
    let AppState {
        simulation,
        session,
        gui,
        ..
    } = state;
    painted(|ui| {
        let mut events = Vec::new();
        rs_cam_viz::ui::sim_diagnostics::draw(ui, simulation, session, gui, &mut events);
    })
}

/// Every text run the Simulation op list paints — the left rail of the
/// Simulation workspace, where `toolpath_status_flags` puts its triage chips.
pub fn op_list_text(state: &mut AppState) -> Vec<String> {
    let AppState {
        simulation,
        session,
        gui,
        viewport,
        ..
    } = state;
    painted(|ui| {
        let mut events = Vec::new();
        rs_cam_viz::ui::sim_op_list::draw(ui, simulation, session, gui, viewport, &mut events);
    })
}

/// Every text run the Readiness page paints, in the column width the
/// workspace gives it.
pub fn readiness_text(state: &AppState) -> Vec<String> {
    painted(|ui| {
        ui.set_max_width(READINESS_COLUMN);
        let mut events = Vec::new();
        rs_cam_viz::ui::readiness_panel::draw(ui, state, &mut events);
    })
}

/// The single-line runs — the faces and the captions. A multi-line run is a
/// hover and is read separately.
pub fn faces(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|t| !t.contains('\n'))
        .cloned()
        .collect()
}

/// The multi-line runs — the hovers.
pub fn hovers(texts: &[String]) -> Vec<String> {
    texts.iter().filter(|t| t.contains('\n')).cloned().collect()
}

/// The project load report the panels read, through the panel's own producer.
pub fn load_report(state: &AppState) -> rs_cam_core::tool_load::ToolLoadReport {
    rs_cam_viz::ui::readiness::load_report(state)
}
