//! Geometry selection P4.D, fix 1: a 2D op reads the faces of a STEP model.
//!
//! # The claim
//!
//! The operator opened a STEP part only, and every 2D op was greyed out:
//! the Add toolpath menu and the Geometry check gated the 2D ops on a
//! drawing item only. The gate is now ONE rule, a drawing item OR a face
//! item (`ItemSet::has_selectable_item`, the rule of the "Selection"
//! boundary source). On a STEP-only project:
//!
//! 1. The add menu offers Pocket, Profile and Trace
//!    (`ui::toolpath_panel::add_menu_offers`), and the Geometry check of a
//!    new Pocket gives no geometry error.
//! 2. A new Pocket starts with the default rule, which takes no face (D2),
//!    so its selection refuses `Empty` ("pick a face").
//! 3. A face click (`ProjectSession::click_edit`, the GUI click route) adds
//!    that face. The Pocket then generates, and every cutting move lies
//!    inside that face.
//! 4. A Trace takes the same face as a closed line (a face's rings are
//!    closed curves): it generates, and its cut stays round that face.
//!
//! # The fixture
//!
//! `fixtures/gui_step/stepped_block.step`, read through the import door
//! (`io::load_model_file`). The case face is the largest flat up face.
//!
//! Red proofs (each one run, each one went red, each one reverted):
//!
//! - Claim 1: `has_selectable_item` as `has_drawing` (the old gate): the
//!   menu did not offer Pocket.
//! - Claim 4: `ItemKind::serves` with `(Face, Curves) => false`: the Trace
//!   refused `KindNotCut`.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_2d_op_cuts_a_step_face_p4_d
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{MatchKey, RefusalCause, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{
    AddToolpathArgs, ClickEdit, Command, EditSelectionArgs, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, ToolpathConfig,
};
use rs_cam_core::toolpath::MoveType;
use rs_cam_viz::ui::properties::{ToolpathValidationContext, validate_toolpath_config};
use rs_cam_viz::ui::toolpath_panel::add_menu_offers;

/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 3.0;
/// The float noise of an XY read back from the toolpath (mm).
const EPS: f64 = 1e-3;
/// Samples along each cutting segment, ends included.
const SEGMENT_SAMPLES: usize = 5;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gui_step/stepped_block.step")
}

/// A STEP-only project: one tool, the stepped block, a stock box round it.
fn step_only_session() -> ProjectSession {
    let model = rs_cam_core::io::load_model_file(
        &fixture(),
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the STEP fixture loads");
    assert!(
        !model.items.has_drawing(),
        "the case is a STEP model with faces only"
    );
    let bbox = model.mesh.as_ref().expect("a STEP model has a mesh").bbox;
    let stock = StockConfig {
        x: bbox.max.x - bbox.min.x,
        y: bbox.max.y - bbox.min.y,
        z: bbox.max.z - bbox.min.z,
        origin_x: bbox.min.x,
        origin_y: bbox.min.y,
        origin_z: bbox.min.z,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = TOOL_DIAMETER;
    let mut builder = ProjectSessionBuilder::new().stock(stock).tool(tool);
    let _ = builder.add_model(model);
    builder.build()
}

/// Add a default toolpath of `op` on the model; returns its index.
fn add_op(session: &mut ProjectSession, op: OperationType) -> usize {
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: op.label().to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: session.models()[0].id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }))
        .unwrap();
    session.toolpath_configs().len() - 1
}

/// The largest flat up face of the model: its pick key (as a click stores
/// it) and its rings.
fn case_face(session: &ProjectSession) -> (MatchKey, Polygon2) {
    let model = &session.models()[0];
    let (item, face) = model
        .items
        .iter()
        .filter_map(|item| match item.shape() {
            Shape::Face(face) if face.normal.z > 0.99 => Some((item, face)),
            _ => None,
        })
        .max_by(|a, b| a.1.rings.area().total_cmp(&b.1.rings.area()))
        .expect("the stepped block has a flat up face");
    let key = MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    };
    (key, face.rings.clone())
}

/// One viewport click on `key` for the op selection of toolpath `index`:
/// the GUI route (`click_edit`, then `EditSelection`).
fn click(session: &mut ProjectSession, index: usize, key: MatchKey) {
    let tc = session.toolpath_configs()[index].clone();
    let edit = match session.click_edit(&tc, SelectionTarget::Op, key).unwrap() {
        ClickEdit::Selection(edit) => edit,
        other @ ClickEdit::BoundaryToSelection { .. } => {
            panic!("a click on a 2D op edits its op selection: {other:?}")
        }
    };
    assert!(
        matches!(edit, SelectionEdit::AddPick(_)),
        "a first click adds the face: {edit:?}"
    );
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index,
            target: SelectionTarget::Op,
            edit,
        }))
        .unwrap();
}

/// XY samples along every cutting move of a fresh generation of `index`.
fn cutting_samples(session: &mut ProjectSession, index: usize) -> Vec<P2> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(index, &cancel)
        .expect("the op generates from the picked face");
    let moves = &result.toolpath().moves;
    let mut samples = Vec::new();
    for pair in moves.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        if let MoveType::Linear { .. } = to.move_type {
            for k in 0..SEGMENT_SAMPLES {
                let t = k as f64 / (SEGMENT_SAMPLES - 1) as f64;
                samples.push(P2::new(
                    from.target.x + t * (to.target.x - from.target.x),
                    from.target.y + t * (to.target.y - from.target.y),
                ));
            }
        }
    }
    samples
}

#[test]
fn the_add_menu_offers_2d_ops_on_a_step_only_project_p4_d() {
    let mut session = step_only_session();
    for op in [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Trace,
    ] {
        assert!(
            add_menu_offers(&session, op),
            "a STEP-only project must offer {op:?}: a face is an item a 2D op reads"
        );
    }
    let index = add_op(&mut session, OperationType::Pocket);
    let ctx = ToolpathValidationContext::from_session(&session);
    let errs = validate_toolpath_config(&session.toolpath_configs()[index], &ctx);
    assert!(
        !errs.iter().any(|e| e.contains("2D geometry")),
        "the Geometry check must accept a STEP model for a Pocket: {errs:?}"
    );
}

#[test]
fn a_new_pocket_on_a_step_model_asks_for_a_face_p4_d() {
    let mut session = step_only_session();
    let index = add_op(&mut session, OperationType::Pocket);
    let tc = session.toolpath_configs()[index].clone();
    let report = session
        .selection_status(&tc, SelectionTarget::Op)
        .expect("the op selection resolves");
    let refusal = report
        .refusal
        .as_ref()
        .expect("the default rule takes no face (D2): nothing to cut");
    assert!(
        matches!(refusal.cause, RefusalCause::Empty { .. }),
        "the refusal is Empty, which the Selection section answers with \
         \"pick a face\": {refusal}"
    );
}

#[test]
fn a_pocket_cuts_inside_the_clicked_step_face_p4_d() {
    let mut session = step_only_session();
    let index = add_op(&mut session, OperationType::Pocket);
    let (key, face) = case_face(&session);
    click(&mut session, index, key);

    let samples = cutting_samples(&mut session, index);
    assert!(
        samples.len() > 20,
        "a pocket that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let outside: Vec<&P2> = samples
        .iter()
        .filter(|p| !face.contains_point_eps(p, EPS))
        .collect();
    assert!(
        outside.is_empty(),
        "{} of {} cutting samples lie outside the picked face. First: {:?}",
        outside.len(),
        samples.len(),
        outside.first()
    );
}

#[test]
fn a_trace_follows_the_clicked_step_face_p4_d() {
    let mut session = step_only_session();
    let index = add_op(&mut session, OperationType::Trace);
    let (key, face) = case_face(&session);
    click(&mut session, index, key);

    let samples = cutting_samples(&mut session, index);
    assert!(
        samples.len() > 4,
        "a Trace of the face must cut: {} samples",
        samples.len()
    );
    // The cut follows the face outline: every sample lies within one tool
    // diameter of the face.
    let (min, max) = face.exterior.iter().fold(
        (P2::new(f64::MAX, f64::MAX), P2::new(f64::MIN, f64::MIN)),
        |(lo, hi), p| {
            (
                P2::new(lo.x.min(p.x), lo.y.min(p.y)),
                P2::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    );
    let reach = TOOL_DIAMETER + EPS;
    assert!(
        samples.iter().all(|p| p.x >= min.x - reach
            && p.x <= max.x + reach
            && p.y >= min.y - reach
            && p.y <= max.y + reach),
        "the Trace leaves the face outline"
    );
}
