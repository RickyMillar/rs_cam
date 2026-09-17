//! DEP-1: "the dependency edges ARE the invalidation walker's rules".
//!
//! Programme: `planning/gen_sim_rest_ux_2026-09-18/PLAN.md` §4.1, brief
//! `IMPL_W0.md` §6. Before this file the walker held its own copy of the
//! rules and the card held another. `session::dependencies::edges` is now
//! the one statement, and the walker reads it.
//!
//! ## What the file pins
//!
//! 1. `edges_name_every_declaration`: one edge of each kind exists, and a
//!    toolpath that declares nothing gets no edge.
//! 2. `every_walker_drop_is_an_edge`: every index the walker drops is the
//!    `from` of some edge.
//! 3. `every_edge_drop_the_rules_predict_is_made`: a fixpoint computed in
//!    this file from `edges()` alone equals the walker's drop set. The
//!    claim is NOT "every edge makes a drop": a Stock edge whose source is
//!    clean makes none.
//! 4. `regenerating_a_source_drops_its_consumers`: D1. Adopting a result
//!    for a source drops its Regions and PrevTool consumers, leaves its
//!    Stock consumers alone, and keeps the simulation.
//! 5. `edge_state_reads_the_session`: the three `EdgeState` answers, and
//!    the split that makes them differ: the edit door clears the
//!    simulation, the adopt door does not.
//!
//! Read `edges()` AFTER `apply`, never before. The walker derives its edges
//! after the mutation wrote, so a pre-apply snapshot of the disable case
//! still holds `rest2d -> rough` and predicts a drop the walker never
//! makes.
//!
//! The fixture builds no geometry, so the file runs in seconds.

// SAFETY: test-module lint allowances, per `crates/rs_cam_core/CLAUDE.md`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundarySource, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::dependencies::{Edge, EdgeKind, EdgeState, edges, primary_edges, state};
use rs_cam_core::session::{
    AdoptResultArgs, AdoptSimulationArgs, Command, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SetBoundaryConfigArgs, SetToolpathEnabledArgs, SetToolpathParamArgs,
    ToolpathConfig,
};
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

// ── the seven rows ───────────────────────────────────────────────

const ROUGH: usize = 0;
const PROFILE: usize = 1;
const FINISH: usize = 2;
const LAKES: usize = 3;
const REST2D: usize = 4;
const BACK_ROUGH: usize = 5;
const BACK_FINISH: usize = 6;

/// The one feed value the edit arms write.
const EDITED_FEED_RATE: f64 = 4321.0;

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

fn tc(
    name: &str,
    op: OperationConfig,
    stock_source: StockSource,
    tool_id: usize,
    model_id: usize,
    boundary: BoundaryConfig,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // overwritten by `add_toolpath`
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary,
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

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig::default())
}

fn a_square() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(10.0, 0.0),
        P2::new(10.0, 10.0),
        P2::new(0.0, 10.0),
    ])
}

fn result_with_regions(
    regions: Option<Vec<Polygon2>>,
) -> rs_cam_core::session::ToolpathComputeResult {
    let mut annotated = AnnotatedToolpath::new(rs_cam_core::toolpath::Toolpath::new());
    annotated.rest_regions = regions.map(Arc::new);
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(annotated)),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Deliver a computed result the way the compute lane does.
///
/// Seed strictly in index order: since D1 an adopt of an EARLIER index
/// drops a LATER consumer's result.
fn adopt(session: &mut ProjectSession, index: usize, regions: Option<Vec<Polygon2>>) {
    let revision = session.toolpath_revision(index);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(result_with_regions(regions)),
        }))
        .expect("the fixture adopts at the current revision");
}

fn a_stock() -> Arc<TriDexelStock> {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    };
    Arc::new(TriDexelStock::from_bounds(&bbox, 2.0))
}

/// Store a simulation whose `prior_stocks` cover the two `FromRemainingStock`
/// rows. That is what makes a `Ready` Stock edge reachable from an
/// integration test.
fn adopt_simulation(session: &mut ProjectSession) {
    let mut prior_stocks = HashMap::new();
    prior_stocks.insert(id_of(session, FINISH), a_stock());
    prior_stocks.insert(id_of(session, BACK_FINISH), a_stock());
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
        }))
        .expect("the fixture stores a simulation");
}

fn id_of(session: &ProjectSession, index: usize) -> ToolpathId {
    session.toolpath_configs()[index].id
}

/// Two setups, seven rows, one model, two tools.
///
/// Setup 1: `0` rough (`Fresh`, rough tool); `1` profile (`Fresh`, fine
/// tool); `2` finish (`FromRemainingStock`); `3` lakes (`Fresh`, boundary
/// `DerivedRestRegions` on `0`); `4` rest2d (`Rest` with
/// `prev_tool_id = rough`).
///
/// Setup 2: `5` back rough (`Fresh`, rough tool); `6` back finish
/// (`FromRemainingStock`).
///
/// Index 1 is the non-vacuity of "many Stock edges, not just the nearest":
/// it sits between the edited row and the consumer and declares nothing.
/// Setup 2 is the non-vacuity of the setup scope. Only `0` and `5` carry
/// the rough tool, so index 4's PrevTool edge can land nowhere else.
fn fixture() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_tool(ToolConfig::new_default(ToolId(1), ToolType::BallNose));
    builder.add_model(empty_model("part.svg"));
    let rough_tool = builder.tools()[0].id.0;
    let fine_tool = builder.tools()[1].id.0;
    let model = builder.models()[0].id;
    let setup_2 = builder.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

    let plain = BoundaryConfig::default();
    let rough_index = builder
        .add_toolpath(
            0,
            tc(
                "rough",
                pocket(),
                StockSource::Fresh,
                rough_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            0,
            tc(
                "profile",
                pocket(),
                StockSource::Fresh,
                fine_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            0,
            tc(
                "finish",
                pocket(),
                StockSource::FromRemainingStock,
                fine_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            0,
            tc(
                "lakes",
                pocket(),
                StockSource::Fresh,
                fine_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            0,
            tc(
                "rest2d",
                OperationConfig::Rest(RestConfig {
                    prev_tool_id: Some(ToolId(rough_tool)),
                    ..RestConfig::default()
                }),
                StockSource::Fresh,
                fine_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            setup_2,
            tc(
                "back rough",
                pocket(),
                StockSource::Fresh,
                rough_tool,
                model,
                plain.clone(),
            ),
        )
        .unwrap();
    let _ = builder
        .add_toolpath(
            setup_2,
            tc(
                "back finish",
                pocket(),
                StockSource::FromRemainingStock,
                fine_tool,
                model,
                plain,
            ),
        )
        .unwrap();

    let mut session = builder.build();
    // The Regions consumer needs the source's STABLE id, and the session
    // allocates it, so read it back rather than guessing.
    let rough_id = id_of(&session, rough_index);
    let _ = session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: LAKES,
            boundary: BoundaryConfig {
                enabled: true,
                source: BoundarySource::DerivedRestRegions {
                    source_toolpath_id: rough_id,
                },
                ..BoundaryConfig::default()
            },
        }))
        .expect("the boundary door writes a derived source");
    for index in 0..7 {
        let regions = (index == ROUGH).then(|| vec![a_square()]);
        adopt(&mut session, index, regions);
    }
    adopt_simulation(&mut session);
    assert_fixture_is_live(&session);
    session
}

fn assert_fixture_is_live(session: &ProjectSession) {
    assert_eq!(
        session.toolpath_count(),
        7,
        "the fixture holds seven toolpaths"
    );
    for index in 0..7 {
        assert!(
            session.get_result(index).is_some(),
            "row {index} starts with a cached result"
        );
    }
    assert!(
        session
            .get_result(ROUGH)
            .and_then(|r| r.annotated().rest_regions.as_deref())
            .is_some_and(|regions| !regions.is_empty()),
        "the Regions source carries regions, or claim 5 reads Broken for the wrong reason"
    );
    assert!(
        session.simulation_result().is_some(),
        "the fixture stores a simulation"
    );
}

// ── claim 1 ──────────────────────────────────────────────────────

#[test]
fn edges_name_every_declaration() {
    let session = fixture();
    let all = edges(&session);

    let stock: Vec<&Edge> = all.iter().filter(|e| e.kind == EdgeKind::Stock).collect();
    let regions: Vec<&Edge> = all.iter().filter(|e| e.kind == EdgeKind::Regions).collect();
    let prev: Vec<&Edge> = all
        .iter()
        .filter(|e| e.kind == EdgeKind::PrevTool)
        .collect();

    // Non-vacuity: one edge of each kind exists.
    assert!(!stock.is_empty(), "the fixture declares a Stock edge");
    assert_eq!(regions.len(), 1, "the fixture declares one Regions edge");
    assert_eq!(prev.len(), 1, "the fixture declares one PrevTool edge");

    // Stock: MANY edges for `finish`, one per op above it in its setup.
    let finish_sources: BTreeSet<Option<ToolpathId>> = stock
        .iter()
        .filter(|e| e.from == id_of(&session, FINISH))
        .map(|e| e.on)
        .collect();
    assert_eq!(
        finish_sources,
        BTreeSet::from([Some(id_of(&session, ROUGH)), Some(id_of(&session, PROFILE)),]),
        "a Stock consumer names EVERY op above it, not only the nearest"
    );

    // The setup scope: `back finish` names only its own setup's row.
    let back_sources: BTreeSet<Option<ToolpathId>> = stock
        .iter()
        .filter(|e| e.from == id_of(&session, BACK_FINISH))
        .map(|e| e.on)
        .collect();
    assert_eq!(
        back_sources,
        BTreeSet::from([Some(id_of(&session, BACK_ROUGH))]),
        "a Stock edge never crosses a setup"
    );

    assert_eq!(
        regions[0].on,
        Some(id_of(&session, ROUGH)),
        "the Regions edge names the source the boundary declares"
    );
    assert_eq!(
        prev[0].from,
        id_of(&session, REST2D),
        "the Rest op declares the PrevTool edge"
    );
    assert_eq!(
        prev[0].on,
        Some(id_of(&session, ROUGH)),
        "the PrevTool edge lands on the enabled same-setup predecessor with that cutter"
    );

    // A row that declares nothing gets no edge.
    assert!(
        all.iter().all(|e| e.from != id_of(&session, PROFILE)),
        "index 1 declares no dependency"
    );

    // `primary_edges` collapses the Stock set to the nearest enabled source.
    let primary: Vec<Edge> = primary_edges(&session)
        .into_iter()
        .filter(|e| e.from == id_of(&session, FINISH) && e.kind == EdgeKind::Stock)
        .collect();
    assert_eq!(primary.len(), 1, "a surface draws one Stock line");
    assert_eq!(
        primary[0].on,
        Some(id_of(&session, PROFILE)),
        "the drawn Stock line names the NEAREST enabled source"
    );
}

// ── the arms claims 2 and 3 run ──────────────────────────────────

/// What one arm does, and the seeds it hands the walker.
struct Arm {
    name: &'static str,
    /// The indices the arm itself dirties. `Effects.stale` is compared
    /// after subtracting this set.
    seeds: BTreeSet<usize>,
    /// The subset whose stock contribution moved.
    chain_seeds: BTreeSet<usize>,
    run: fn(&mut ProjectSession) -> BTreeSet<usize>,
}

fn edit_rough(session: &mut ProjectSession) -> BTreeSet<usize> {
    session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: ROUGH,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("the setter writes feed_rate")
        .stale
}

fn edit_finish(session: &mut ProjectSession) -> BTreeSet<usize> {
    session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: FINISH,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("the setter writes feed_rate")
        .stale
}

fn disable_rough(session: &mut ProjectSession) -> BTreeSet<usize> {
    session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: ROUGH,
            enabled: false,
        }))
        .expect("the toggle writes enabled")
        .stale
}

fn adopt_rough_again(session: &mut ProjectSession) -> BTreeSet<usize> {
    let revision = session.toolpath_revision(ROUGH);
    session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: ROUGH,
            revision,
            result: Box::new(result_with_regions(Some(vec![a_square()]))),
        }))
        .expect("the lane adopts at the current revision")
        .stale
}

fn arms() -> Vec<Arm> {
    vec![
        Arm {
            name: "edit rough",
            seeds: BTreeSet::from([ROUGH]),
            chain_seeds: BTreeSet::from([ROUGH]),
            run: edit_rough,
        },
        Arm {
            name: "disable rough",
            seeds: BTreeSet::from([ROUGH]),
            chain_seeds: BTreeSet::from([ROUGH]),
            run: disable_rough,
        },
        Arm {
            name: "edit finish",
            seeds: BTreeSet::from([FINISH]),
            chain_seeds: BTreeSet::from([FINISH]),
            run: edit_finish,
        },
        Arm {
            name: "adopt rough",
            seeds: BTreeSet::from([ROUGH]),
            chain_seeds: BTreeSet::new(),
            run: adopt_rough_again,
        },
    ]
}

// ── claim 2 ──────────────────────────────────────────────────────

#[test]
fn every_walker_drop_is_an_edge() {
    let mut saw_a_drop = false;
    for arm in arms() {
        let mut session = fixture();
        let stale = (arm.run)(&mut session);
        let all = edges(&session);
        let consumers: BTreeSet<ToolpathId> = all.iter().map(|e| e.from).collect();
        for index in stale.difference(&arm.seeds) {
            saw_a_drop = true;
            let id = id_of(&session, *index);
            assert!(
                consumers.contains(&id),
                "arm '{}' dropped index {index}, which declares no edge",
                arm.name
            );
        }
    }
    // Non-vacuity: at least one arm dropped something.
    assert!(saw_a_drop, "no arm reached a consumer");
}

// ── claim 3 ──────────────────────────────────────────────────────

/// The walker's fixpoint, computed HERE from `edges()` alone.
///
/// Stock hits on the chain-dirty set; Regions and PrevTool hit on the
/// dirty set. A dropped enabled consumer joins the chain-dirty set.
fn predicted_drops(session: &ProjectSession, arm: &Arm) -> BTreeSet<usize> {
    let all = edges(session);
    let index_of: HashMap<ToolpathId, usize> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .map(|(index, tc)| (tc.id, index))
        .collect();
    let mut dirty = arm.seeds.clone();
    let mut chain = arm.chain_seeds.clone();
    let mut dropped: BTreeSet<usize> = BTreeSet::new();
    loop {
        let to_ids = |set: &BTreeSet<usize>| -> BTreeSet<ToolpathId> {
            set.iter()
                .filter_map(|index| session.toolpath_configs().get(*index).map(|tc| tc.id))
                .collect()
        };
        let dirty_ids = to_ids(&dirty);
        let chain_ids = to_ids(&chain);
        let mut newly: BTreeSet<usize> = BTreeSet::new();
        for edge in &all {
            let Some(source_id) = edge.on else { continue };
            let hit = match edge.kind {
                EdgeKind::Stock => chain_ids.contains(&source_id),
                EdgeKind::Regions | EdgeKind::PrevTool => dirty_ids.contains(&source_id),
            };
            let Some(&index) = index_of.get(&edge.from) else {
                continue;
            };
            if hit && !dirty.contains(&index) {
                let _ = newly.insert(index);
            }
        }
        if newly.is_empty() {
            break;
        }
        for index in newly {
            let _ = dirty.insert(index);
            let _ = dropped.insert(index);
            if session.toolpath_configs()[index].enabled {
                let _ = chain.insert(index);
            }
        }
    }
    dropped
}

#[test]
fn every_edge_drop_the_rules_predict_is_made() {
    for arm in arms() {
        let mut session = fixture();
        let stale = (arm.run)(&mut session);
        // AFTER `apply`: the walker derives its edges after the mutation
        // wrote, and the disable arm reads differently before and after.
        let predicted = predicted_drops(&session, &arm);
        let measured: BTreeSet<usize> = stale.difference(&arm.seeds).copied().collect();
        assert_eq!(
            measured, predicted,
            "arm '{}': the walker and the edges disagree",
            arm.name
        );
    }
}

/// The drop set each arm makes, spelled out, so a change of rule shows up
/// as a changed table and not only as a changed derivation.
#[test]
fn the_arms_drop_the_named_rows() {
    let table: [(&str, BTreeSet<usize>); 4] = [
        // The Stock consumer, the Regions consumer, and (rule (c)) the
        // Rest consumer.
        ("edit rough", BTreeSet::from([FINISH, LAKES, REST2D])),
        // The predecessor is now disabled, so the PrevTool edge resolves
        // to nothing and rule (c) does not fire.
        ("disable rough", BTreeSet::from([FINISH, LAKES])),
        // Nothing in the project depends on `finish`.
        ("edit finish", BTreeSet::new()),
        // D1: rules (b) and (c) only.
        ("adopt rough", BTreeSet::from([LAKES, REST2D])),
    ];
    for arm in arms() {
        let expected = &table
            .iter()
            .find(|(name, _)| *name == arm.name)
            .expect("every arm has a row")
            .1;
        let mut session = fixture();
        let stale = (arm.run)(&mut session);
        let measured: BTreeSet<usize> = stale.difference(&arm.seeds).copied().collect();
        assert_eq!(&measured, expected, "arm '{}'", arm.name);
    }
}

// ── claim 4: D1 ──────────────────────────────────────────────────

#[test]
fn regenerating_a_source_drops_its_consumers() {
    let mut session = fixture();
    let stale = adopt_rough_again(&mut session);

    assert_eq!(
        stale,
        BTreeSet::from([LAKES, REST2D]),
        "an adopt stales the Regions and PrevTool consumers, and nothing else"
    );
    assert!(
        session.get_result(LAKES).is_none(),
        "the Regions consumer's result is gone"
    );
    assert!(
        session.get_result(REST2D).is_none(),
        "the PrevTool consumer's result is gone"
    );
    assert!(
        session.get_result(ROUGH).is_some(),
        "the adopted result itself stays"
    );
    assert!(
        session.get_result(FINISH).is_some(),
        "rule (a) does not fire on an adopt: recording an answer removes no material"
    );
    assert!(
        session.get_result(BACK_FINISH).is_some(),
        "the other setup's Stock consumer stays"
    );
    let simulation = session
        .simulation_result()
        .expect("the adopt door keeps the simulation the ladder reads next");
    assert!(
        simulation
            .prior_stocks
            .contains_key(&id_of(&session, FINISH)),
        "the prior-stock snapshot survives the adopt"
    );
}

// ── claim 5: the three states ────────────────────────────────────

fn state_of(session: &ProjectSession, consumer: usize, kind: EdgeKind) -> EdgeState {
    let id = id_of(session, consumer);
    let all = edges(session);
    // The nearest declared source answers for the row.
    let edge = all
        .iter()
        .rfind(|e| e.from == id && e.kind == kind)
        .unwrap_or_else(|| panic!("row {consumer} declares a {kind:?} edge"));
    state(edge, session)
}

#[test]
fn edge_state_reads_the_session() {
    let session = fixture();
    assert_eq!(
        state_of(&session, FINISH, EdgeKind::Stock),
        EdgeState::Ready,
        "a Stock edge with a snapshot and a generated source is Ready"
    );
    assert_eq!(
        state_of(&session, BACK_FINISH, EdgeKind::Stock),
        EdgeState::Ready,
        "the second setup reads its own snapshot"
    );
    assert_eq!(
        state_of(&session, LAKES, EdgeKind::Regions),
        EdgeState::Ready,
        "a Regions edge whose source's RESULT carries regions is Ready"
    );
    assert_eq!(
        state_of(&session, REST2D, EdgeKind::PrevTool),
        EdgeState::Ready,
        "a PrevTool edge whose predecessor has generated is Ready"
    );

    // The edit door clears the simulation, so the snapshot goes.
    let mut edited = fixture();
    let _ = edit_rough(&mut edited);
    assert_eq!(
        state_of(&edited, FINISH, EdgeKind::Stock),
        EdgeState::Pending,
        "an edit clears the simulation, so the Stock edge waits"
    );

    // The adopt door does NOT. This is the §4.1 split, measured.
    let mut adopted = fixture();
    let _ = adopt_rough_again(&mut adopted);
    assert_eq!(
        state_of(&adopted, FINISH, EdgeKind::Stock),
        EdgeState::Ready,
        "an adopt keeps the simulation, so the Stock edge stays Ready"
    );

    // A disabled source breaks both of its edges.
    let mut disabled = fixture();
    let _ = disable_rough(&mut disabled);
    assert_eq!(
        state_of(&disabled, REST2D, EdgeKind::PrevTool),
        EdgeState::Broken,
        "a disabled op is not a predecessor, so the edge resolves to nothing"
    );
    let stock_on_rough = edges(&disabled)
        .into_iter()
        .find(|e| {
            e.kind == EdgeKind::Stock
                && e.from == id_of(&disabled, FINISH)
                && e.on == Some(id_of(&disabled, ROUGH))
        })
        .expect("a Stock edge is NOT filtered on the source's enabled flag");
    assert_eq!(
        state(&stock_on_rough, &disabled),
        EdgeState::Broken,
        "the edge survives the disable; its STATE reports the disabled source"
    );

    // A Regions source with no regions is Broken, not Pending.
    let mut no_regions = fixture();
    let revision = no_regions.toolpath_revision(ROUGH);
    let _ = no_regions
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: ROUGH,
            revision,
            result: Box::new(result_with_regions(None)),
        }))
        .expect("the lane adopts at the current revision");
    assert_eq!(
        state_of(&no_regions, LAKES, EdgeKind::Regions),
        EdgeState::Broken,
        "the Regions state reads the result payload, not the rest-analysis flag"
    );
}
