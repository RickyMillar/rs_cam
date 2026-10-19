//! Geometry selection P2.5 (DESIGN 9.2) — a re-import of a model stales
//! every toolpath that reads the model, and only those.
//!
//! # The claim
//!
//! `ToolpathConfig::models_read` lists every model a toolpath reads: its
//! own model, every `SourceRef::Model` of its op selection (a rule, a pick
//! or an exclude), the Project Curve surface model, and every
//! `SourceRef::Model` of an ENABLED boundary selection.
//! `drop_results_for_model` (the door of `AdoptModelGeometry` and
//! `InvalidateModel`) reads it. Before P2.5 that door matched the own model
//! and an enabled `ModelOutline` boundary only, so a Project Curve whose
//! surface model was re-imported kept a result cut on the old surface.
//!
//! # The fixture
//!
//! Two drawings, A and B, and seven toolpaths, each with a stored result.
//! Model B is re-imported. The toolpaths:
//!
//! | index | reads B through | stale |
//! |---|---|---|
//! | 0 | its own model | yes |
//! | 1 | an enabled boundary selection | yes |
//! | 2 | a DISABLED boundary selection | no |
//! | 3 | a rule of its op selection | yes |
//! | 4 | the Project Curve surface model | yes |
//! | 5 | an exclude of its op selection | yes |
//! | 6 | nothing (control) | no |

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;

use common::session::{drawing_items, toolpath_config};
use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::{PocketConfig, ProjectCurveConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::toolpath_stats::ToolpathStats;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::ops::drill_op::OpData;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{ItemKind, MatchKey, Selection, SelectionEdit, SourceRef};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, DatumConfig, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SetupData, ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The model every reader does not re-import.
const MODEL_A: usize = 4;
/// The model the re-import replaces.
const MODEL_B: usize = 9;
/// The tool every toolpath binds.
const TOOL: usize = 7;

fn square_model(id: usize, name: &str, side_mm: f64) -> LoadedModel {
    let square = Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(side_mm, 0.0),
        P2::new(side_mm, side_mm),
        P2::new(0.0, side_mm),
    ]);
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        items: Arc::new(drawing_items(vec![square], &[])),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
    }
}

fn setup() -> SetupData {
    SetupData {
        id: 1,
        name: "Setup 1".to_owned(),
        face_up: FaceUp::Top,
        z_rotation: ZRotation::default(),
        datum: DatumConfig::default(),
        model_ids: Vec::new(),
        obstacles: Vec::new(),
        stock_changes: Vec::new(),
        toolpath_indices: Vec::new(),
        pause_message: None,
    }
}

fn fake_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: OpData::Toolpath(Arc::new(AnnotatedToolpath::new(Toolpath::new()))),
        stats: ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    }
}

fn pocket(name: &str, model_id: usize, selection: Selection) -> ToolpathConfig {
    toolpath_config(
        name,
        OperationConfig::Pocket(PocketConfig {
            selection,
            ..PocketConfig::default()
        }),
        TOOL,
        model_id,
    )
}

fn with_boundary(mut tc: ToolpathConfig, enabled: bool) -> ToolpathConfig {
    tc.boundary = BoundaryConfig {
        enabled,
        source: BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId(MODEL_B)),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    tc
}

/// The drawing selection, less the square of model B (an exclude).
fn excludes_b() -> Selection {
    let mut selection = Selection::drawing();
    let key = MatchKey {
        source: SourceRef::Model(ModelId(MODEL_B)),
        kind: ItemKind::Loop,
        layer: None,
        anchor: [10.0, 10.0],
        measure: 20.0,
        brep: None,
    };
    selection
        .apply(SelectionEdit::RemovePick(key), &Selection::drawing())
        .expect("an exclude edit applies");
    assert_eq!(selection.excludes().len(), 1, "the edit stores an exclude");
    selection
}

fn fixture() -> ProjectSession {
    let readers: Vec<ToolpathConfig> = vec![
        pocket("own model", MODEL_B, Selection::drawing()),
        with_boundary(pocket("boundary", MODEL_A, Selection::drawing()), true),
        with_boundary(
            pocket("disabled boundary", MODEL_A, Selection::drawing()),
            false,
        ),
        pocket(
            "op rule",
            MODEL_A,
            Selection::closed_shapes_of(ModelId(MODEL_B)),
        ),
        toolpath_config(
            "surface",
            OperationConfig::ProjectCurve(ProjectCurveConfig {
                surface_model_id: Some(ModelId(MODEL_B)),
                ..ProjectCurveConfig::default()
            }),
            TOOL,
            MODEL_A,
        ),
        pocket("op exclude", MODEL_A, excludes_b()),
        pocket("control", MODEL_A, Selection::drawing()),
    ];
    let mut builder = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill))
        .model(square_model(MODEL_A, "a.svg", 40.0))
        .model(square_model(MODEL_B, "b.svg", 20.0))
        .setup(setup());
    let count = readers.len();
    for (index, mut tc) in readers.into_iter().enumerate() {
        tc.id = ToolpathId(10 + index);
        builder = builder.toolpath(tc);
    }
    for index in 0..count {
        builder = builder.result(index, fake_result());
    }
    builder.build()
}

#[test]
fn a_reimport_stales_every_reader_of_the_model_p2_5() {
    let mut session = fixture();
    let read: Vec<bool> = session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.models_read().contains(&MODEL_B))
        .collect();
    assert_eq!(
        read,
        vec![true, true, false, true, true, true, false],
        "models_read must list model B for each reader, and for no other"
    );

    let effects = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: MODEL_B,
            geometry: Box::new(square_model(MODEL_B, "b.svg", 30.0)),
            units: None,
        }))
        .expect("model B exists, so the re-import applies");
    let stale: Vec<usize> = effects.stale.iter().copied().collect();
    assert_eq!(
        stale,
        vec![0, 1, 3, 4, 5],
        "the re-import must stale every reader of model B (own model, enabled \
         boundary, op rule, surface model, op exclude), and no other"
    );
    for index in [2, 6] {
        assert!(
            session.get_result(index).is_some(),
            "toolpath {index} does not read model B; it keeps its result"
        );
    }
}

/// Geometry selection P2.6: a REMOVAL of model B stales the same readers
/// as a re-import (one rule, `drop_results_for_model`). Before P2.6
/// `remove_model` dropped no result, so a toolpath that read model B
/// through a boundary, a rule, an exclude or a surface kept a result cut
/// on a model that no longer exists.
///
/// The own-model reader (index 0) stays in the project. Its selection now
/// names a model that does not exist, so `selection_status` (= generation)
/// refuses `MissingSource` with the id.
#[test]
fn a_model_removal_stales_every_reader_p2_6() {
    let mut session = fixture();
    let index_of_b = session
        .models()
        .iter()
        .position(|m| m.id == MODEL_B)
        .unwrap();
    let effects = session
        .apply(Command::RemoveModel(
            rs_cam_core::session::RemoveModelArgs { index: index_of_b },
        ))
        .expect("model B exists, so the removal applies");
    let stale: Vec<usize> = effects.stale.iter().copied().collect();
    assert_eq!(
        stale,
        vec![0, 1, 3, 4, 5],
        "the removal must stale every reader of model B (own model, enabled \
         boundary, op rule, surface model, op exclude), and no other"
    );
    for index in [0, 1, 3, 4, 5] {
        assert!(
            session.get_result(index).is_none(),
            "toolpath {index} reads the removed model; its result must drop"
        );
    }
    for index in [2, 6] {
        assert!(
            session.get_result(index).is_some(),
            "toolpath {index} does not read model B; it keeps its result"
        );
    }
    assert_eq!(session.toolpath_count(), 7, "a removal deletes no toolpath");
    let own = session.toolpath_configs()[0].clone();
    let refusal = session
        .selection_status(&own, rs_cam_core::session::SelectionTarget::Op)
        .unwrap()
        .refusal
        .clone()
        .expect("the own-model reader refuses");
    assert_eq!(
        refusal.cause,
        rs_cam_core::selection::RefusalCause::MissingSource(SourceRef::Model(ModelId(MODEL_B))),
        "the refusal names the removed model: {refusal}"
    );
}
