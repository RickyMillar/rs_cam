//! S2 — a setup's stock changes: the four command rows, the named
//! refusals, the invalidation rule and the project file.
//!
//! Plan: `planning/stock_additions_2026-10-09/PLAN.md`, package S2.
//!
//! The claims:
//!
//! 1. `AddStockChange`, `ReplaceStockChange`, `MoveStockChange` and
//!    `RemoveStockChange` write the setup's list through
//!    `ProjectSession::apply`.
//! 2. Each validation rule refuses by name, and a refusal writes nothing.
//! 3. A change in setup N drops the result of every `FromRemainingStock`
//!    operation of setup N and of every later setup, and clears the
//!    simulation with `SimulationDropCause::StockChanges`. Setups before N
//!    and operations that do not read the remaining stock keep their
//!    results. A rename, or an edit to a change that stays disabled, drops
//!    nothing.
//! 4. A model refresh under an enabled stock change runs the same rule.
//! 5. `[[setups.stock_changes]]` round-trips through the project file, and a
//!    project with no stock change writes no such key.
//!
//! The request side (the simulation groups carry the list, and the rest
//! record lists it) is in `session/compute/tests.rs`; the cache key is in
//! `compute/sim_prefix.rs`.

#![allow(
    // SAFETY: test code; a failed fixture is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::compute::stock_change::{
    StockChange, StockChangeOp, StockChangeRefusal, StockGeometry,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::{ModelId, StockChangeId};
use rs_cam_core::material::Material;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddStockChangeArgs, AdoptModelGeometryArgs, AdoptResultArgs, Command, LoadedModel,
    MoveStockChangeArgs, ProjectSession, ProjectSessionBuilder, RemoveStockChangeArgs,
    ReplaceStockChangeArgs, SessionError, SimulationDropCause, ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ── fixture ──────────────────────────────────────────────────────

fn tc(
    name: &str,
    op: OperationConfig,
    stock_source: StockSource,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn model(name: &str) -> LoadedModel {
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

fn square() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)
}

fn outline_model() -> LoadedModel {
    let mut m = model("outline.dxf");
    m.polygons = Some(Arc::new(vec![square()]));
    m
}

fn open_path_model() -> LoadedModel {
    let mut path = Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(5.0, 0.0),
        P2::new(5.0, 5.0),
    ]);
    path.closed = false;
    let mut m = model("river.dxf");
    m.polygons = Some(Arc::new(vec![path]));
    m
}

fn mesh_model() -> LoadedModel {
    let mut m = model("insert.stl");
    m.mesh = Some(Arc::new(TriangleMesh::from_raw(
        vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(0.0, 1.0, 0.0),
            P3::new(0.0, 0.0, 1.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    )));
    m
}

fn fake_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn adopt(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision(index);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("the fixture adopts at the current revision");
}

/// The model ids the fixture holds.
struct Models {
    outline: ModelId,
    open_path: ModelId,
    mesh: ModelId,
}

/// Three setups (Top, Bottom, Top). Each holds a `Fresh` pocket and then a
/// `FromRemainingStock` rest operation, and every operation holds a cached
/// result. Toolpath index `2 * setup + 0` is the pocket, `2 * setup + 1`
/// the rest operation.
fn fixture() -> (ProjectSession, Models) {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    // The toolpaths name this model, so a refresh of the other three
    // reaches the toolpaths only through a stock change.
    let part = builder.add_model(model("part.svg"));
    let outline = ModelId(builder.add_model(outline_model()));
    let open_path = ModelId(builder.add_model(open_path_model()));
    let mesh = ModelId(builder.add_model(mesh_model()));
    let _ = builder.add_setup("Setup 2".to_owned(), FaceUp::Bottom);
    let _ = builder.add_setup("Setup 3".to_owned(), FaceUp::Top);
    let tool = builder.tools()[0].id.0;
    for setup in 0..3 {
        let _ = builder
            .add_toolpath(
                setup,
                tc(
                    &format!("pocket {setup}"),
                    OperationConfig::Pocket(PocketConfig::default()),
                    StockSource::Fresh,
                    tool,
                    part,
                ),
            )
            .unwrap();
        let _ = builder
            .add_toolpath(
                setup,
                tc(
                    &format!("rest {setup}"),
                    OperationConfig::Rest(RestConfig::default()),
                    StockSource::FromRemainingStock,
                    tool,
                    part,
                ),
            )
            .unwrap();
    }
    let mut s = builder.build();
    assert_eq!(s.setup_count(), 3);
    assert_eq!(s.toolpath_count(), 6);
    for index in 0..6 {
        adopt(&mut s, index);
    }
    assert!(live(&s).len() == 6, "the fixture holds six cached results");
    (
        s,
        Models {
            outline,
            open_path,
            mesh,
        },
    )
}

fn live(s: &ProjectSession) -> BTreeSet<usize> {
    (0..s.toolpath_count())
        .filter(|&i| s.get_result(i).is_some())
        .collect()
}

fn fill(id: usize, model: ModelId, level_z: f64) -> StockChange {
    StockChange {
        id: StockChangeId(id),
        name: format!("Fill {id}"),
        enabled: true,
        op: StockChangeOp::Add,
        geometry: StockGeometry::OutlineFill {
            model_ids: vec![model],
            level_z,
        },
        material: Material::Custom {
            name: "Filler".to_owned(),
            feed_scale_factor: 1.0,
        },
    }
}

fn add(
    s: &mut ProjectSession,
    setup_index: usize,
    change: StockChange,
) -> Result<(), SessionError> {
    s.apply(Command::AddStockChange(AddStockChangeArgs {
        setup_index,
        change: Box::new(change),
    }))
    .map(|_| ())
}

fn replace(
    s: &mut ProjectSession,
    setup_index: usize,
    change: StockChange,
) -> Result<(), SessionError> {
    s.apply(Command::ReplaceStockChange(ReplaceStockChangeArgs {
        setup_index,
        change_id: change.id,
        change: Box::new(change),
    }))
    .map(|_| ())
}

fn ids(s: &ProjectSession, setup_index: usize) -> Vec<usize> {
    s.list_setups()[setup_index]
        .stock_changes
        .iter()
        .map(|c| c.id.0)
        .collect()
}

// ── 1. the command round trip ────────────────────────────────────

#[test]
fn the_four_rows_add_edit_move_and_remove() {
    let (mut s, m) = fixture();
    add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
    let extrude = StockChange {
        id: StockChangeId(2),
        name: "Riser".to_owned(),
        enabled: true,
        op: StockChangeOp::Add,
        geometry: StockGeometry::OutlineExtrude {
            model_ids: vec![m.outline],
            z_bottom: 10.0,
            z_top: 14.0,
        },
        material: Material::default(),
    };
    add(&mut s, 1, extrude).unwrap();
    let cut_free = StockChange {
        id: StockChangeId(3),
        name: "Cut free".to_owned(),
        enabled: true,
        op: StockChangeOp::Remove,
        geometry: StockGeometry::Model { model_id: m.mesh },
        material: Material::default(),
    };
    add(&mut s, 1, cut_free).unwrap();
    assert_eq!(ids(&s, 1), vec![1, 2, 3]);

    // An edit, in place.
    replace(&mut s, 1, fill(1, m.outline, 6.5)).unwrap();
    let edited = &s.list_setups()[1].stock_changes[0];
    assert!(matches!(
        edited.geometry,
        StockGeometry::OutlineFill { level_z, .. } if (level_z - 6.5).abs() < 1e-12
    ));

    // The disable flip is an edit too.
    let mut off = fill(1, m.outline, 6.5);
    off.enabled = false;
    replace(&mut s, 1, off).unwrap();
    assert!(!s.list_setups()[1].stock_changes[0].enabled);

    // A move.
    let _ = s
        .apply(Command::MoveStockChange(MoveStockChangeArgs {
            setup_index: 1,
            change_id: StockChangeId(3),
            to_position: 0,
        }))
        .unwrap();
    assert_eq!(ids(&s, 1), vec![3, 1, 2]);

    // A removal.
    let _ = s
        .apply(Command::RemoveStockChange(RemoveStockChangeArgs {
            setup_index: 1,
            change_id: StockChangeId(1),
        }))
        .unwrap();
    assert_eq!(ids(&s, 1), vec![3, 2]);
    assert!(s.list_setups()[0].stock_changes.is_empty());
    assert!(s.list_setups()[2].stock_changes.is_empty());
}

// ── 2. the refusals ──────────────────────────────────────────────

fn refused(result: Result<(), SessionError>) -> StockChangeRefusal {
    match result {
        Err(SessionError::StockChangeRefused(refusal)) => refusal,
        other => panic!("expected a stock-change refusal, got {other:?}"),
    }
}

#[test]
fn each_rule_refuses_by_name_and_writes_nothing() {
    let (mut s, m) = fixture();
    let before = live(&s);
    let epoch = s.simulation_epoch();

    // An unknown model id.
    assert_eq!(
        refused(add(&mut s, 0, fill(1, ModelId(99), 1.0))),
        StockChangeRefusal::UnknownModel {
            model_id: ModelId(99)
        }
    );
    // A `Model` geometry needs a mesh.
    let mut on_outline = fill(1, m.outline, 1.0);
    on_outline.geometry = StockGeometry::Model {
        model_id: m.outline,
    };
    assert!(matches!(
        refused(add(&mut s, 0, on_outline)),
        StockChangeRefusal::ModelIsNotAMesh { .. }
    ));
    // An outline geometry needs closed 2D outlines: a mesh has none.
    assert!(matches!(
        refused(add(&mut s, 0, fill(1, m.mesh, 1.0))),
        StockChangeRefusal::ModelHasNoClosedOutline { .. }
    ));
    // ... and an open path is not a closed outline.
    assert!(matches!(
        refused(add(&mut s, 0, fill(1, m.open_path, 1.0))),
        StockChangeRefusal::ModelHasNoClosedOutline { .. }
    ));
    // An outline geometry needs at least one model.
    let mut none = fill(1, m.outline, 1.0);
    none.geometry = StockGeometry::OutlineFill {
        model_ids: Vec::new(),
        level_z: 1.0,
    };
    assert_eq!(
        refused(add(&mut s, 0, none)),
        StockChangeRefusal::NoOutlineModels
    );
    // The Z checks.
    assert_eq!(
        refused(add(&mut s, 0, fill(1, m.outline, f64::NAN))),
        StockChangeRefusal::ZNotFinite { field: "level_z" }
    );
    let mut flat = fill(1, m.outline, 1.0);
    flat.geometry = StockGeometry::OutlineExtrude {
        model_ids: vec![m.outline],
        z_bottom: 4.0,
        z_top: 4.0,
    };
    assert!(matches!(
        refused(add(&mut s, 0, flat)),
        StockChangeRefusal::EmptyZRange { .. }
    ));
    let mut upside_down = fill(1, m.outline, 1.0);
    upside_down.geometry = StockGeometry::OutlineExtrude {
        model_ids: vec![m.outline],
        z_bottom: 5.0,
        z_top: 4.0,
    };
    assert!(matches!(
        refused(add(&mut s, 0, upside_down)),
        StockChangeRefusal::EmptyZRange { .. }
    ));
    let mut infinite = fill(1, m.outline, 1.0);
    infinite.geometry = StockGeometry::OutlineExtrude {
        model_ids: vec![m.outline],
        z_bottom: 0.0,
        z_top: f64::INFINITY,
    };
    assert_eq!(
        refused(add(&mut s, 0, infinite)),
        StockChangeRefusal::ZNotFinite { field: "z_top" }
    );

    // Nothing above wrote the list or dropped a result.
    assert!(
        s.list_setups()
            .iter()
            .all(|setup| setup.stock_changes.is_empty())
    );
    assert_eq!(live(&s), before);
    assert_eq!(s.simulation_epoch(), epoch);

    // A duplicate id.
    add(&mut s, 0, fill(1, m.outline, 1.0)).unwrap();
    assert_eq!(
        refused(add(&mut s, 0, fill(1, m.outline, 2.0))),
        StockChangeRefusal::DuplicateId {
            id: StockChangeId(1)
        }
    );
    assert_eq!(ids(&s, 0), vec![1]);

    // A replacement must pass the same rules.
    assert!(matches!(
        refused(replace(&mut s, 0, fill(1, m.mesh, 1.0))),
        StockChangeRefusal::ModelHasNoClosedOutline { .. }
    ));

    // An unknown change id, an id mismatch and a position out of range
    // refuse as invalid parameters.
    assert!(matches!(
        replace(&mut s, 0, fill(7, m.outline, 1.0)),
        Err(SessionError::InvalidParam(_))
    ));
    assert!(matches!(
        s.apply(Command::ReplaceStockChange(ReplaceStockChangeArgs {
            setup_index: 0,
            change_id: StockChangeId(1),
            change: Box::new(fill(2, m.outline, 1.0)),
        })),
        Err(SessionError::InvalidParam(_))
    ));
    assert!(matches!(
        s.apply(Command::MoveStockChange(MoveStockChangeArgs {
            setup_index: 0,
            change_id: StockChangeId(1),
            to_position: 1,
        })),
        Err(SessionError::InvalidParam(_))
    ));
    assert!(matches!(
        s.apply(Command::RemoveStockChange(RemoveStockChangeArgs {
            setup_index: 0,
            change_id: StockChangeId(7),
        })),
        Err(SessionError::InvalidParam(_))
    ));
    assert!(matches!(
        add(&mut s, 9, fill(2, m.outline, 1.0)),
        Err(SessionError::SetupNotFound(9))
    ));

    // A `Remove` with a material is allowed; the material has no effect.
    let mut remove = fill(2, m.outline, 1.0);
    remove.op = StockChangeOp::Remove;
    add(&mut s, 0, remove).unwrap();
    assert_eq!(ids(&s, 0), vec![1, 2]);
}

// ── 3. the invalidation rule ─────────────────────────────────────

/// A change in setup index 1 stales the rest operations of setups 1 and 2,
/// and nothing of setup 0. The pockets keep their results.
#[test]
fn a_change_in_setup_two_stales_its_rest_ops_and_the_later_ones() {
    let (mut s, m) = fixture();
    let epoch = s.simulation_epoch();
    let effects = s
        .apply(Command::AddStockChange(AddStockChangeArgs {
            setup_index: 1,
            change: Box::new(fill(1, m.outline, 5.0)),
        }))
        .unwrap();
    assert_eq!(
        live(&s),
        BTreeSet::from([0, 1, 2, 4]),
        "rest 1 (index 3) and rest 2 (index 5) read the stock the change moved"
    );
    assert_eq!(effects.stale, BTreeSet::from([3, 5]));
    // The fixture holds no simulation, so `simulation_cleared` stays false;
    // the epoch is the evidence that the door ran.
    assert!(s.simulation_epoch() > epoch, "the simulation epoch moved");
    let causes = s.simulation_drop_causes_since(epoch);
    assert!(causes.contains(SimulationDropCause::StockChanges));
    assert!(
        !causes.contains(SimulationDropCause::Operations),
        "the drop names its own cause"
    );
}

#[test]
fn a_change_in_the_last_setup_stales_only_that_setup() {
    let (mut s, m) = fixture();
    add(&mut s, 2, fill(1, m.outline, 5.0)).unwrap();
    assert_eq!(live(&s), BTreeSet::from([0, 1, 2, 3, 4]));
}

#[test]
fn a_change_in_the_first_setup_stales_every_rest_op() {
    let (mut s, m) = fixture();
    add(&mut s, 0, fill(1, m.outline, 5.0)).unwrap();
    assert_eq!(live(&s), BTreeSet::from([0, 2, 4]));
}

/// Every edit that moves the effect drops; a rename does not. An edit to a
/// change that stays disabled does not.
#[test]
fn only_an_edit_that_moves_the_effect_drops() {
    let (mut s, m) = fixture();
    add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
    let mut second = fill(2, m.outline, 3.0);
    second.op = StockChangeOp::Remove;
    add(&mut s, 1, second).unwrap();

    let reset = |s: &mut ProjectSession| {
        for index in [3, 5] {
            adopt(s, index);
        }
        assert_eq!(live(s).len(), 6);
    };
    let drops = |s: &mut ProjectSession, command: Command| -> bool {
        let epoch = s.simulation_epoch();
        let _ = s.apply(command).unwrap();
        let dropped = live(s) == BTreeSet::from([0, 1, 2, 4]);
        let cleared = s
            .simulation_drop_causes_since(epoch)
            .contains(SimulationDropCause::StockChanges);
        assert_eq!(dropped, cleared, "the results and the simulation agree");
        dropped
    };
    let replace_cmd = |change: StockChange| {
        Command::ReplaceStockChange(ReplaceStockChangeArgs {
            setup_index: 1,
            change_id: change.id,
            change: Box::new(change),
        })
    };

    reset(&mut s);
    let mut renamed = fill(1, m.outline, 5.0);
    renamed.name = "Pour".to_owned();
    assert!(
        !drops(&mut s, replace_cmd(renamed)),
        "a rename drops nothing"
    );

    reset(&mut s);
    assert!(
        drops(&mut s, replace_cmd(fill(1, m.outline, 5.5))),
        "a new level drops"
    );

    reset(&mut s);
    let mut other_material = fill(1, m.outline, 5.5);
    other_material.material = Material::default();
    assert!(
        drops(&mut s, replace_cmd(other_material)),
        "an Add material drops"
    );

    reset(&mut s);
    let mut remove_material = fill(2, m.outline, 3.0);
    remove_material.op = StockChangeOp::Remove;
    remove_material.material = Material::default();
    assert!(
        !drops(&mut s, replace_cmd(remove_material)),
        "a Remove ignores its material"
    );

    reset(&mut s);
    let mut off = fill(2, m.outline, 3.0);
    off.op = StockChangeOp::Remove;
    off.enabled = false;
    assert!(drops(&mut s, replace_cmd(off.clone())), "a disable drops");

    reset(&mut s);
    off.geometry = StockGeometry::OutlineFill {
        model_ids: vec![m.outline],
        level_z: 9.0,
    };
    assert!(
        !drops(&mut s, replace_cmd(off)),
        "an edit to a change that stays disabled drops nothing"
    );

    reset(&mut s);
    assert!(
        !drops(
            &mut s,
            Command::MoveStockChange(MoveStockChangeArgs {
                setup_index: 1,
                change_id: StockChangeId(2),
                to_position: 0,
            })
        ),
        "a move past a disabled change keeps the order of the enabled ones"
    );

    reset(&mut s);
    let mut on = fill(2, m.outline, 9.0);
    on.op = StockChangeOp::Remove;
    assert!(drops(&mut s, replace_cmd(on)), "an enable drops");

    reset(&mut s);
    assert!(
        drops(
            &mut s,
            Command::MoveStockChange(MoveStockChangeArgs {
                setup_index: 1,
                change_id: StockChangeId(2),
                to_position: 1,
            })
        ),
        "a reorder of two enabled changes drops"
    );

    reset(&mut s);
    assert!(
        drops(
            &mut s,
            Command::RemoveStockChange(RemoveStockChangeArgs {
                setup_index: 1,
                change_id: StockChangeId(1),
            })
        ),
        "a removal drops"
    );
}

/// A disabled rest operation loses its result too: it read the old stock,
/// and a re-enable must not bring that result back.
#[test]
fn a_disabled_rest_op_loses_its_result_too() {
    let (mut s, m) = fixture();
    let _ = s
        .apply(Command::SetToolpathEnabled(
            rs_cam_core::session::SetToolpathEnabledArgs {
                index: 5,
                enabled: false,
            },
        ))
        .unwrap();
    adopt(&mut s, 5);
    assert!(s.get_result(5).is_some());
    add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
    assert!(s.get_result(5).is_none());
}

// ── 4. a model refresh under a stock change ──────────────────────

#[test]
fn a_model_refresh_under_a_stock_change_stales_from_that_setup_on() {
    let (mut s, m) = fixture();
    add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
    for index in [3, 5] {
        adopt(&mut s, index);
    }
    assert!(
        s.toolpath_configs()
            .iter()
            .all(|tc| tc.model_id != m.outline.0),
        "no toolpath reads the outline model itself"
    );
    let epoch = s.simulation_epoch();
    let mut geometry = outline_model();
    geometry.polygons = Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 12.0, 12.0)]));
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: m.outline.0,
            geometry: Box::new(geometry),
            units: None,
        }))
        .unwrap();
    assert_eq!(live(&s), BTreeSet::from([0, 1, 2, 4]));
    assert!(
        s.simulation_drop_causes_since(epoch)
            .contains(SimulationDropCause::StockChanges)
    );
}

#[test]
fn a_model_refresh_under_a_stock_change_spares_the_earlier_setups() {
    let (mut s, m) = fixture();
    let mut insert = fill(1, m.mesh, 0.0);
    insert.geometry = StockGeometry::Model { model_id: m.mesh };
    add(&mut s, 2, insert).unwrap();
    adopt(&mut s, 5);
    assert_eq!(live(&s).len(), 6);
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: m.mesh.0,
            geometry: Box::new(mesh_model()),
            units: None,
        }))
        .unwrap();
    assert_eq!(live(&s), BTreeSet::from([0, 1, 2, 3, 4]));
}

// ── 5. the project file ──────────────────────────────────────────

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rs_cam_s2_{tag}_{}_{}.toml",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "_")
    ))
}

#[test]
fn the_project_file_round_trips_the_stock_changes() {
    let (mut s, m) = fixture();
    add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
    let mut riser = fill(2, m.outline, 0.0);
    riser.enabled = false;
    riser.geometry = StockGeometry::OutlineExtrude {
        model_ids: vec![m.outline],
        z_bottom: 10.0,
        z_top: 14.0,
    };
    add(&mut s, 1, riser).unwrap();
    let mut cut_free = fill(3, m.mesh, 0.0);
    cut_free.op = StockChangeOp::Remove;
    cut_free.geometry = StockGeometry::Model { model_id: m.mesh };
    add(&mut s, 2, cut_free).unwrap();

    let path = temp_path("round_trip");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[[setups.stock_changes]]"), "{text}");
    assert!(text.contains("kind = \"outline_fill\""), "{text}");
    let loaded = ProjectSession::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    for setup_index in 0..3 {
        assert_eq!(
            loaded.list_setups()[setup_index].stock_changes,
            s.list_setups()[setup_index].stock_changes,
            "setup {setup_index}"
        );
    }
    assert_eq!(ids(&loaded, 1), vec![1, 2]);
}

#[test]
fn a_project_with_no_stock_change_writes_no_key() {
    let (s, _) = fixture();
    let path = temp_path("no_key");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(!text.contains("stock_changes"), "{text}");
}

/// A removed setup takes its enabled stock changes with it, so the rest
/// operations of the later setups lose their results. Without the stock
/// change, rest 2 keeps its result: pocket 2 is above it in its own setup,
/// so the removal reaches it through no dependency edge.
#[test]
fn a_removed_setup_with_a_stock_change_stales_the_later_rest_ops() {
    for with_change in [false, true] {
        let (mut s, m) = fixture();
        if with_change {
            add(&mut s, 1, fill(1, m.outline, 5.0)).unwrap();
            for index in [3, 5] {
                adopt(&mut s, index);
            }
        }
        let _ = s
            .apply(Command::RemoveSetup(
                rs_cam_core::session::RemoveSetupArgs { setup_index: 1 },
            ))
            .unwrap();
        assert_eq!(s.toolpath_count(), 4);
        assert_eq!(s.toolpath_configs()[3].name, "rest 2");
        let expected = if with_change {
            BTreeSet::from([0, 1, 2])
        } else {
            BTreeSet::from([0, 1, 2, 3])
        };
        assert_eq!(live(&s), expected, "with_change = {with_change}");
    }
}
