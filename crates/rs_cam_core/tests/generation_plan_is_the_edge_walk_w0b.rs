//! W0b: "the generation plan is the edge walk, and the loader normalises
//! the rest-analysis demand".
//!
//! Programme: `planning/gen_sim_rest_ux_2026-09-18/PLAN.md` §4.2, briefs
//! `IMPL_W1.md` §2.1 to §2.4, `IMPL_W5.md` §8 and `IMPL_W2.md` §3.4.
//! `session::generation_plan::plan` is the one answer to "what work makes
//! this current". Before it, the GUI, the MCP server and the CLI each held
//! a round-based fixpoint of their own.
//!
//! ## What the file pins
//!
//! 1. `the_cold_project_plans_one_simulation_per_rest_op`: the exact step
//!    sequence, in plan order, with no snapshots. This is the F.4 property.
//! 2. `a_present_snapshot_removes_its_simulate_step`: one snapshot, one
//!    Simulate step fewer, and the rest of the sequence unmoved.
//! 3. `the_ancestor_scope_walks_the_edges`: `Ancestors(rest2)` gives
//!    rough, Simulate, rest1, Simulate, rest2, and leaves the other setup
//!    out.
//! 4. `the_ancestor_closure_is_not_the_nearest_source_only`: the arm that
//!    parts a closure over `edges` from one over `primary_edges`.
//! 5. `a_disabled_op_gets_no_step`.
//! 6. `the_loader_normalises_the_rest_analysis_demand`: a round trip
//!    through the save and load doors switches a stray producer off and a
//!    consumed producer on.
//!
//! ## The fixture's plan order is NOT its numeric order
//!
//! Setup 1 owns indices `[0, 1, 3]` and setup 2 owns `[2]`. So the walk
//! must read `SetupData::toolpath_indices`, not `toolpath_configs`
//! positions: a numeric walk would put the second setup's operation
//! between `rest1` and `rest2`.

// SAFETY: test-module lint allowances, per `crates/rs_cam_core/CLAUDE.md`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundarySource, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::{SetupId, ToolpathId};
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    AdoptSimulationArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SetBoundaryConfigArgs, SetRestAnalysisConfigArgs, SetStockSourceArgs, SetToolpathEnabledArgs,
    ToolpathConfig,
};
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ── the four rows ────────────────────────────────────────────────

const ROUGH: usize = 0;
const REST1: usize = 1;
/// Setup 2's only row. It sits BETWEEN the two rest rows numerically and
/// AFTER both of them in plan order.
const BACK: usize = 2;
const REST2: usize = 3;

// ── fixture ──────────────────────────────────────────────────────

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0, // overwritten by `add_model`
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn tc(name: &str, stock_source: StockSource, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // overwritten by `add_toolpath`
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: false,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn id_of(session: &ProjectSession, index: usize) -> ToolpathId {
    session.toolpath_configs()[index].id
}

fn setup_id_of(session: &ProjectSession, position: usize) -> SetupId {
    SetupId(session.list_setups()[position].id)
}

/// Two setups, four rows, one tool, one model.
///
/// Setup 1, in plan order: `rough` (`Fresh`), `rest1`
/// (`FromRemainingStock`), `rest2` (`FromRemainingStock`).
/// Setup 2: `back` (`Fresh`).
///
/// The rows are added in the order rough, rest1, back, rest2, so setup 1
/// owns indices `[0, 1, 3]`. Plan order and numeric order differ, which is
/// the non-vacuity of the ordering rule.
fn fixture() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
    let setup_2 = builder.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

    let _ = builder
        .add_toolpath(0, tc("rough", StockSource::Fresh, tool, model))
        .unwrap();
    let _ = builder
        .add_toolpath(0, tc("rest1", StockSource::FromRemainingStock, tool, model))
        .unwrap();
    let _ = builder
        .add_toolpath(setup_2, tc("back", StockSource::Fresh, tool, model))
        .unwrap();
    let _ = builder
        .add_toolpath(0, tc("rest2", StockSource::FromRemainingStock, tool, model))
        .unwrap();

    let session = builder.build();
    assert_fixture_is_live(&session);
    session
}

fn assert_fixture_is_live(session: &ProjectSession) {
    assert_eq!(session.toolpath_count(), 4, "the fixture holds four rows");
    assert_eq!(
        session.list_setups()[0].toolpath_indices,
        vec![ROUGH, REST1, REST2],
        "setup 1's plan order is not its numeric order, or the test is vacuous"
    );
    assert_eq!(
        session.list_setups()[1].toolpath_indices,
        vec![BACK],
        "setup 2 owns the row between the two rest rows"
    );
    assert!(
        session.simulation_result().is_none(),
        "the fixture starts cold"
    );
}

fn a_stock() -> Arc<TriDexelStock> {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    };
    Arc::new(TriDexelStock::from_bounds(&bbox, 2.0))
}

/// Store a simulation whose `prior_stocks` cover exactly the named rows.
fn adopt_simulation(session: &mut ProjectSession, covered: &[usize]) {
    let mut prior_stocks = HashMap::new();
    for &index in covered {
        prior_stocks.insert(id_of(session, index), a_stock());
    }
    let _ = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(SimulationResult {
                mesh: StockMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    colors: Vec::new(),
                },
                total_moves: 0,
                deviations: None,
                column_deviations: None,
                boundaries: Vec::new(),
                checkpoints: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: None,
                resolution_clamped: false,
                column_grid_cell_mm: 2.0,
                prior_stocks,
            }),
            epoch: session.simulation_epoch(),
        }))
        .expect("the fixture stores a simulation");
}

fn generate(session: &ProjectSession, index: usize) -> Step {
    Step::Generate {
        toolpath: id_of(session, index),
        index,
    }
}

fn simulate(session: &ProjectSession, setup_position: usize, upto: usize) -> Step {
    Step::Simulate {
        setup: setup_id_of(session, setup_position),
        upto: id_of(session, upto),
    }
}

// ── claim 1 ──────────────────────────────────────────────────────

#[test]
fn the_cold_project_plans_one_simulation_per_rest_op() {
    let session = fixture();
    let steps = plan(&session, Scope::Project);

    assert_eq!(
        steps,
        vec![
            generate(&session, ROUGH),
            simulate(&session, 0, REST1),
            generate(&session, REST1),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
            generate(&session, BACK),
        ],
        "the cold plan runs setup order, then plan order inside a setup, \
         with one Simulate step before each rest operation"
    );

    // F.4, stated as a count: k rest operations in one setup give k
    // Simulate steps.
    let simulations = steps
        .iter()
        .filter(|s| matches!(s, Step::Simulate { .. }))
        .count();
    assert_eq!(
        simulations, 2,
        "two rest operations give two Simulate steps"
    );

    // The walk appends no full simulation. W1 owns that decision.
    assert!(
        matches!(steps.last(), Some(Step::Generate { .. })),
        "core appends no trailing SimulateAll step"
    );
}

// ── claim 2 ──────────────────────────────────────────────────────

#[test]
fn a_present_snapshot_removes_its_simulate_step() {
    let mut session = fixture();
    adopt_simulation(&mut session, &[REST1]);

    assert_eq!(
        plan(&session, Scope::Project),
        vec![
            generate(&session, ROUGH),
            generate(&session, REST1),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
            generate(&session, BACK),
        ],
        "the snapshot the consumer reads removes exactly its own Simulate step"
    );

    // Non-vacuity: cover both rest rows and no Simulate step survives.
    adopt_simulation(&mut session, &[REST1, REST2]);
    assert!(
        plan(&session, Scope::Project)
            .iter()
            .all(|s| matches!(s, Step::Generate { .. })),
        "a covering simulation plans no Simulate step at all"
    );
}

// ── claim 3 ──────────────────────────────────────────────────────

#[test]
fn the_ancestor_scope_walks_the_edges() {
    let session = fixture();

    assert_eq!(
        plan(&session, Scope::Ancestors(id_of(&session, REST2))),
        vec![
            generate(&session, ROUGH),
            simulate(&session, 0, REST1),
            generate(&session, REST1),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
        ],
        "the ancestor closure reads EVERY enabled same-setup predecessor, \
         not only the nearest one, and the walk keeps plan order"
    );

    // The other setup is not an ancestor: a Stock edge never crosses a
    // setup.
    assert_eq!(
        plan(&session, Scope::Ancestors(id_of(&session, ROUGH))),
        vec![generate(&session, ROUGH)],
        "the first operation in a setup depends on nothing"
    );

    // The setup scope names one setup's rows.
    assert_eq!(
        plan(&session, Scope::Setup(setup_id_of(&session, 1))),
        vec![generate(&session, BACK)],
        "the setup scope plans that setup's operations"
    );
}

/// The defect injection claim 3 cannot make on its own.
///
/// While `rest1` starts from the remaining stock it carries its own Stock
/// edges, so a closure over `primary_edges` still reaches `rough`
/// TRANSITIVELY (rest2 to rest1 to rough) and gives the same plan. Make
/// `rest1` start from fresh stock and the two closures part: `rest1`
/// declares no Stock edge, `primary_edges` keeps only `rest2`'s NEAREST
/// source, and `rough` disappears from the plan. That is defect D6, and
/// the plan must not inherit it.
#[test]
fn the_ancestor_closure_is_not_the_nearest_source_only() {
    let mut session = fixture();
    let _ = session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index: REST1,
            source: StockSource::Fresh,
        }))
        .expect("the setter writes the stock source");

    assert_eq!(
        plan(&session, Scope::Ancestors(id_of(&session, REST2))),
        vec![
            generate(&session, ROUGH),
            generate(&session, REST1),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
        ],
        "the closure keeps EVERY enabled predecessor. A nearest-only \
         closure would drop `rough` here, because `rest1` no longer \
         carries a Stock edge of its own"
    );
}

// ── claim 4 ──────────────────────────────────────────────────────

#[test]
fn a_disabled_op_gets_no_step() {
    let mut session = fixture();
    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: REST1,
            enabled: false,
        }))
        .expect("the toggle writes enabled");

    let steps = plan(&session, Scope::Project);
    assert_eq!(
        steps,
        vec![
            generate(&session, ROUGH),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
            generate(&session, BACK),
        ],
        "a disabled operation gets neither a Generate step nor a Simulate step"
    );

    // A disabled ancestor enters the closure and the walk drops it.
    assert_eq!(
        plan(&session, Scope::Ancestors(id_of(&session, REST2))),
        vec![
            generate(&session, ROUGH),
            simulate(&session, 0, REST2),
            generate(&session, REST2),
        ],
        "the ancestor scope drops a disabled ancestor at the walk, not at the closure"
    );
}

// ── claim 5: the load post-pass ──────────────────────────────────

/// Round-trip one session through `ProjectSession::save` and
/// `ProjectSession::load`.
fn round_trip(session: &ProjectSession, tag: &str) -> ProjectSession {
    let dir = std::env::temp_dir().join(format!("rs_cam_w0b_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the test makes its own directory");
    let path = dir.join("project.toml");
    session.save(&path).expect("the save door writes the file");
    let loaded = ProjectSession::load(&path).expect("the load door reads the file");
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

#[test]
fn the_loader_normalises_the_rest_analysis_demand() {
    let mut session = fixture();
    let rough_id = id_of(&session, ROUGH);
    let rest1_id = id_of(&session, REST1);

    // The boundary write comes FIRST, because it runs the producer hook
    // and switches `rest1` on. The two wrong values go in after it, which
    // is the shape a project file saved before the hook shipped carries.
    let _ = session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: REST2,
            boundary: BoundaryConfig {
                enabled: true,
                source: BoundarySource::DerivedRestRegions {
                    source_toolpath_id: rest1_id,
                },
                ..BoundaryConfig::default()
            },
        }))
        .expect("the boundary door writes a derived source");
    // `rest1` produces regions a boundary consumes, and its flag is off.
    let _ = session
        .apply(Command::SetRestAnalysisConfig(SetRestAnalysisConfigArgs {
            index: REST1,
            rest_analysis: RestAnalysisConfig::default(),
        }))
        .expect("the setter writes the rest-analysis block");
    // `rough` carries a stray flag nothing consumes.
    let _ = session
        .apply(Command::SetRestAnalysisConfig(SetRestAnalysisConfigArgs {
            index: ROUGH,
            rest_analysis: RestAnalysisConfig {
                enabled: true,
                ..RestAnalysisConfig::default()
            },
        }))
        .expect("the setter writes the rest-analysis block");

    // Non-vacuity: the session in memory holds both wrong values.
    {
        let configs = session.toolpath_configs();
        assert!(
            configs[ROUGH].rest_analysis.enabled,
            "the stray producer is on before the round trip"
        );
        assert!(
            !configs[REST1].rest_analysis.enabled,
            "the consumed producer is off before the round trip"
        );
        assert!(
            configs[REST2].boundary.enabled,
            "the consumer declares the demand before the round trip"
        );
    }

    let loaded = round_trip(&session, "restdemand");
    let reloaded_rough = loaded
        .find_toolpath_config_by_id(rough_id)
        .expect("the round trip keeps the ids");
    let reloaded_rest1 = loaded
        .find_toolpath_config_by_id(rest1_id)
        .expect("the round trip keeps the ids");

    assert!(
        !reloaded_rough.1.rest_analysis.enabled,
        "the loader switches off a rest analysis no boundary consumes"
    );
    assert!(
        reloaded_rest1.1.rest_analysis.enabled,
        "the loader switches on a rest analysis a boundary consumes"
    );
}
