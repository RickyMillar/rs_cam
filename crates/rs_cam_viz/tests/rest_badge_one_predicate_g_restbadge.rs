//! G-RESTBADGE sentry: the Rest card badge and the static validator answer
//! "does this Rest op have a predecessor?" with ONE rule.
//!
//! R05 §2 defect 2 (`planning/ui_review_2026-09-09/results/W03/support/
//! r05_source_track.md`): the badge looked for any other toolpath in the
//! setup with the previous tool, in any order and any enabled state. The
//! validator (`has_prior_rest_source`) required an EARLIER, ENABLED toolpath
//! in the SAME setup on the SAME model with the previous tool. A Rest card
//! dragged above its roughing pass kept a green `dep` while Generate was
//! refused.
//!
//! Every case here asks both surfaces and asserts they agree:
//! (a) the only candidate is BELOW the Rest op;
//! (b) the candidate is above but disabled;
//! (c) the candidate is above, enabled, same setup and model, previous tool;
//! (d) the candidate is in another setup.
//! Plus the two rule inputs the review lists that neither surface may skip:
//! a different model, and a different tool.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddSetupArgs, AddToolpathArgs, AdoptResultArgs, Command, InvalidateToolpathInputsArgs,
    LoadedModel, ProjectSessionBuilder, SetToolpathEnabledArgs, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::job::{ModelId, ModelKind, ModelUnits};
use rs_cam_viz::state::rest_dependency::{RestCandidate, rest_predecessors};
use rs_cam_viz::state::runtime::{ComputeStatus, ToolpathRuntime};
use rs_cam_viz::state::toolpath::{OperationType, ToolpathEntry};
use rs_cam_viz::ui::properties::{ToolpathValidationContext, validate_toolpath};
use rs_cam_viz::ui::toolpath_panel::{RestBadge, rest_badge};

const ROUGH_TOOL: usize = 1;
const REST_TOOL: usize = 2;
const MODEL_A: usize = 4;
const MODEL_B: usize = 5;

const VALIDATOR_REFUSAL: &str = "earlier enabled operation";

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn tool(id: usize, diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(id), ToolType::EndMill);
    tool.diameter = diameter;
    tool
}

fn toolpath(name: &str, tool_id: usize, model_id: usize, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // assigned by session.add_toolpath
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

fn rest_op() -> OperationConfig {
    let mut op = OperationConfig::Rest(Default::default());
    if let OperationConfig::Rest(cfg) = &mut op {
        cfg.prev_tool_id = Some(ToolId(ROUGH_TOOL));
    }
    op
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(Default::default())
}

/// A state with two tools, two 2D models and one setup. No toolpaths yet.
fn fresh_state() -> AppState {
    let mut state = AppState::new();
    state.session = ProjectSessionBuilder::new()
        .tool(tool(ROUGH_TOOL, 10.0))
        .tool(tool(REST_TOOL, 6.0))
        .model(polygon_model(MODEL_A))
        .model(polygon_model(MODEL_B))
        .build();
    state
}

/// Add a toolpath to `setup_idx`, register a generated, fresh runtime for it,
/// and return its id.
fn add(state: &mut AppState, setup_idx: usize, tc: ToolpathConfig) -> ToolpathId {
    let idx = state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: setup_idx,
            config: Box::new(tc),
        }))
        .unwrap()
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let id = state.session.toolpath_configs()[idx].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    state.gui.toolpath_rt.insert(id, rt);
    // F2.2: "generated" now means the CORE holds a result, because that is
    // what `FreshnessState::Current` is derived from — a `Done` status over
    // an empty core slot is precisely the edited-since state the badge must
    // not read as ready. Before F2.2 the badge asked
    // `ComputeStatus::needs_generation() || stale_since.is_some()`, so a
    // status alone was enough to model a generated predecessor here.
    let revision = state.session.toolpath_revision(idx);
    let _ = state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: idx,
            revision,
            result: Box::new(generated_result()),
        }))
        .expect("index is in range");
    id
}

/// A cached core result, standing for "this toolpath has been generated".
fn generated_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Both surfaces for the Rest op `rest_id` (stored model `model_id`).
fn both_surfaces(state: &AppState, rest_id: ToolpathId, model_id: usize) -> (RestBadge, bool) {
    let tc = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == rest_id)
        .expect("rest op is in the session");
    let OperationConfig::Rest(rest_cfg) = &tc.operation else {
        panic!("expected a Rest op");
    };
    let badge = rest_badge(state, rest_cfg, rest_id);

    let mut entry = ToolpathEntry::for_operation(
        rest_id,
        tc.name.clone(),
        ToolId(REST_TOOL),
        ModelId(model_id),
        OperationType::Rest,
    );
    if let OperationConfig::Rest(cfg) = &mut entry.operation {
        cfg.prev_tool_id = rest_cfg.prev_tool_id;
    }
    let errs = validate_toolpath(
        &entry,
        &ToolpathValidationContext::from_session(&state.session),
    );
    let validator_blocks = errs.iter().any(|e| e.contains(VALIDATOR_REFUSAL));
    (badge, validator_blocks)
}

fn assert_no_dependency(case: &str, badge: RestBadge, validator_blocks: bool) {
    assert_eq!(
        badge,
        RestBadge::Missing,
        "{case}: the badge must read `no dep`, it reads `{}` ({badge:?})",
        badge.text()
    );
    assert!(
        validator_blocks,
        "{case}: the validator must refuse with {VALIDATOR_REFUSAL:?}"
    );
}

fn assert_dependency(case: &str, badge: RestBadge, validator_blocks: bool) {
    assert_ne!(
        badge,
        RestBadge::Missing,
        "{case}: the badge must read `dep`, it reads `no dep`"
    );
    assert!(
        !validator_blocks,
        "{case}: the validator must not refuse with {VALIDATOR_REFUSAL:?}"
    );
}

// ── (a) the only candidate is BELOW the Rest op ─────────────────────────

#[test]
fn a_candidate_below_the_rest_op_is_no_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let _rough_below = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_no_dependency("(a) predecessor below", badge, validator_blocks);
}

// ── (b) the candidate is above but disabled ─────────────────────────────

#[test]
fn b_disabled_candidate_above_is_no_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let rough_idx = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == rough_id)
        .unwrap();
    let _ = state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: rough_idx,
            enabled: false,
        }))
        .unwrap();

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_no_dependency("(b) predecessor disabled", badge, validator_blocks);
}

// ── (c) the candidate qualifies ─────────────────────────────────────────

#[test]
fn c_enabled_same_setup_same_model_previous_tool_above_is_a_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let _rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_dependency("(c) qualifying predecessor", badge, validator_blocks);
    assert_eq!(
        badge,
        RestBadge::Resolved,
        "(c): a generated, fresh predecessor is Resolved, not Stale"
    );
}

#[test]
fn c2_a_qualifying_predecessor_that_needs_generation_reads_stale_dep() {
    let mut state = fresh_state();
    let rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    // Not generated: no core result, and the lane says so. F2.2 — dropping
    // the core result is what "needs generation" now means; the status alone
    // would leave `freshness` reading `Current` off the cached result.
    let rough_idx = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == rough_id)
        .unwrap();
    let _ = state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: rough_idx },
        ))
        .unwrap();
    state.gui.toolpath_rt.get_mut(&rough_id).unwrap().status = ComputeStatus::Pending;
    state.gui.toolpath_rt.get_mut(&rough_id).unwrap().result = None;

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_dependency("(c2) pending predecessor", badge, validator_blocks);
    assert_eq!(badge, RestBadge::Stale);
    assert_eq!(badge.text(), "dep");
}

// ── (d) the candidate is in another setup ───────────────────────────────

#[test]
fn d_candidate_in_another_setup_is_no_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let flip = state
        .session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Flip".to_owned()),
            face_up: FaceUp::Bottom,
        }))
        .expect("the session accepts a second setup")
        .created
        .expect("the AddSetup row reports the new setup index");
    let _rough_other_setup = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        flip,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_no_dependency("(d) predecessor in another setup", badge, validator_blocks);
}

// ── the remaining rule inputs ───────────────────────────────────────────

#[test]
fn a_candidate_on_another_model_is_no_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let _rough_other_model = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_B, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_no_dependency("predecessor on another model", badge, validator_blocks);
}

#[test]
fn a_candidate_with_another_tool_is_no_dependency_on_both_surfaces() {
    let mut state = fresh_state();
    let _rough_same_tool = add(
        &mut state,
        0,
        toolpath("Pocket", REST_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );

    let (badge, validator_blocks) = both_surfaces(&state, rest_id, MODEL_A);
    assert_no_dependency("predecessor with the wrong tool", badge, validator_blocks);
}

#[test]
fn no_previous_tool_configured_is_no_dependency_on_the_badge() {
    let mut state = fresh_state();
    let _rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath(
            "Rest",
            REST_TOOL,
            MODEL_A,
            OperationConfig::Rest(Default::default()),
        ),
    );
    let (badge, _) = both_surfaces(&state, rest_id, MODEL_A);
    assert_eq!(badge, RestBadge::Missing);
}

// ── the predicate itself ────────────────────────────────────────────────

#[test]
fn the_predicate_returns_qualifying_predecessors_in_plan_order() {
    let c = |id: usize, tool_id: usize, model_id: usize, enabled: bool| RestCandidate {
        id: ToolpathId(id),
        tool_id: ToolId(tool_id),
        model_id: ModelId(model_id),
        enabled,
    };
    let setup = [
        c(10, ROUGH_TOOL, MODEL_A, true),  // qualifies
        c(11, ROUGH_TOOL, MODEL_A, false), // disabled
        c(12, ROUGH_TOOL, MODEL_B, true),  // other model
        c(13, REST_TOOL, MODEL_A, true),   // other tool
        c(14, ROUGH_TOOL, MODEL_A, true),  // qualifies
        c(20, REST_TOOL, MODEL_A, true),   // the Rest op
        c(15, ROUGH_TOOL, MODEL_A, true),  // below
    ];
    assert_eq!(
        rest_predecessors(&setup, ToolpathId(20), ModelId(MODEL_A), ToolId(ROUGH_TOOL)),
        vec![ToolpathId(10), ToolpathId(14)]
    );
    // A Rest op that is not in the list has no predecessor in it.
    assert!(
        rest_predecessors(&setup, ToolpathId(99), ModelId(MODEL_A), ToolId(ROUGH_TOOL)).is_empty()
    );
}
