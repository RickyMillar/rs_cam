//! G-MCPREBIND — a toolpath's TOOL and INPUT MODEL bindings have their own
//! setters, and a rebind invalidates what any other binding change does.
//!
//! Tasks F3.7 / F3.8 of the UI/UX fix programme
//! (`planning/ui_fix_2026-09-09/PLAN.md` §5), specified by
//! `planning/ui_fix_2026-09-09/research/R0.3.md` §4 and its §7 Q4.
//!
//! ## The gap this closes
//!
//! Before these setters existed there was no route to a toolpath's
//! `tool_id` / `model_id` except the GUI inspector's Tool: / Input:
//! combos. `ProjectSession::set_toolpath_param` cannot reach them: it
//! writes the OPERATION's params through a serde round-trip, and no
//! operation config carries a `tool_id` or `model_id` field, so the key
//! is refused. Measured on the pre-fix tree, on a Pocket op with two
//! tools in the project:
//!
//! ```text
//! set_toolpath_param(0, "tool_id", 1)
//!   -> Err(InvalidParam("unknown parameter 'tool_id' for Pocket operation. \
//!      Valid parameters: stepover, depth, depth_per_pass, feed_rate, \
//!      plunge_rate, climb, pattern, angle, finishing_passes, spindle_rpm"))
//! set_toolpath_param(0, "model_id", 7)   -> same shape
//! after both: tc.tool_id = 0 (unchanged), tc.model_id = 0 (unchanged)
//! ```
//!
//! That refusal is KEPT — `set_toolpath_param` owns operation params and
//! nothing else. `rebind_is_not_reachable_through_the_param_route` pins
//! it so the two namespaces cannot merge later by accident. Note the
//! near miss: Rest's `prev_tool_id` IS a real operation param (the
//! rest-analysis reference tool), which is a different thing from the
//! cutter the toolpath runs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSession, SessionError, ToolpathConfig,
};

// ── fixture ──────────────────────────────────────────────────────

/// Deliver a computed result the way the compute lane does.
///
/// `insert_result` was the public door before WP3. A completion now
/// carries the revision the lane started from, and the door refuses one
/// that answers a superseded parameter set.
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

fn tc(name: &str, op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
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
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0, // overwritten by `add_model`
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// One setup, two tools (`Ø6 end mill` id 0, `Ø3 ball nose` id 1), two
/// models, one Pocket toolpath bound to tool 0 / model 0.
fn seed() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::BallNose));
    let _ = s.add_model(empty_model("first.svg"));
    let _ = s.add_model(empty_model("second.svg"));
    let tool_a = s.tools()[0].id.0;
    let model_a = s.models()[0].id;
    let _ = s
        .add_toolpath(
            0,
            tc(
                "rough",
                OperationConfig::Pocket(PocketConfig::default()),
                tool_a,
                model_a,
            ),
        )
        .unwrap();
    s
}

// ── the rebind itself ────────────────────────────────────────────

#[test]
fn set_toolpath_tool_rebinds_and_invalidates_the_cached_result() {
    let mut s = seed();
    let tool_b = s.tools()[1].id.0;
    assert_ne!(s.toolpath_configs()[0].tool_id, tool_b);
    adopt(&mut s, 0);
    assert!(
        s.get_result(0).is_some(),
        "a result must be cached before the rebind, or the invalidation \
         assertion below proves nothing"
    );

    let _ = s.set_toolpath_tool(0, tool_b).unwrap();

    assert_eq!(s.toolpath_configs()[0].tool_id, tool_b, "binding moved");
    assert!(
        s.get_result(0).is_none(),
        "the cached result was generated with the OLD tool and must be dropped"
    );
}

#[test]
fn set_toolpath_model_rebinds_and_invalidates_the_cached_result() {
    let mut s = seed();
    let model_b = s.models()[1].id;
    assert_ne!(s.toolpath_configs()[0].model_id, model_b);
    adopt(&mut s, 0);
    assert!(s.get_result(0).is_some());

    let _ = s.set_toolpath_model(0, model_b).unwrap();

    assert_eq!(s.toolpath_configs()[0].model_id, model_b);
    assert!(s.get_result(0).is_none());
}

/// The reason a rebind uses `invalidate_result_chain` and not a bare
/// `results.remove`: a different cutter (or a different input) removes
/// different material, so a downstream `FromRemainingStock` operation's
/// cached result is stale too.
#[test]
fn a_rebind_invalidates_downstream_remaining_stock_results() {
    let mut s = seed();
    let tool_a = s.tools()[0].id.0;
    let tool_b = s.tools()[1].id.0;
    let model_a = s.models()[0].id;
    let mut downstream = tc(
        "rest",
        OperationConfig::Rest(RestConfig::default()),
        tool_b,
        model_a,
    );
    downstream.stock_source = StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, downstream).unwrap();

    // Both rows carry a result; only the upstream one is rebound.
    adopt(&mut s, 0);
    adopt(&mut s, 1);

    let _ = s.set_toolpath_tool(0, tool_b).unwrap();
    assert!(s.get_result(0).is_none(), "the rebound op's own result");
    assert!(
        s.get_result(1).is_none(),
        "the downstream FromRemainingStock op was planned against stock the \
         old tool left; it must be stale too"
    );

    // And the same for the model door.
    let _ = s.set_toolpath_tool(0, tool_a).unwrap();
    let model_b = s.models()[1].id;
    adopt(&mut s, 0);
    adopt(&mut s, 1);
    let _ = s.set_toolpath_model(0, model_b).unwrap();
    assert!(s.get_result(0).is_none());
    assert!(
        s.get_result(1).is_none(),
        "a changed input changes the stock the downstream op inherits"
    );
}

#[test]
fn rebinding_to_the_same_id_is_a_no_op_and_keeps_the_result() {
    let mut s = seed();
    let tool_a = s.tools()[0].id.0;
    let model_a = s.models()[0].id;
    adopt(&mut s, 0);

    let _ = s.set_toolpath_tool(0, tool_a).unwrap();
    assert!(
        s.get_result(0).is_some(),
        "no binding moved, so nothing is stale"
    );
    let _ = s.set_toolpath_model(0, model_a).unwrap();
    assert!(s.get_result(0).is_some());
}

// ── refusals ─────────────────────────────────────────────────────

#[test]
fn a_tool_id_that_resolves_to_nothing_is_refused() {
    let mut s = seed();
    let tool_a = s.tools()[0].id.0;
    let err = s.set_toolpath_tool(0, 999).unwrap_err();
    assert!(
        matches!(err, SessionError::ToolNotFound(ToolId(999))),
        "expected ToolNotFound, got {err:?}"
    );
    assert_eq!(
        s.toolpath_configs()[0].tool_id,
        tool_a,
        "a refused rebind must not move the binding"
    );
}

#[test]
fn a_model_id_that_resolves_to_nothing_is_refused_and_names_the_valid_ids() {
    let mut s = seed();
    let model_a = s.models()[0].id;
    let err = s.set_toolpath_model(0, 999).unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("999") && text.contains("project model ids"),
        "the refusal must name what WAS valid: {text}"
    );
    assert_eq!(s.toolpath_configs()[0].model_id, model_a);
}

#[test]
fn an_out_of_range_toolpath_index_is_refused_by_both_setters() {
    let mut s = seed();
    let tool_b = s.tools()[1].id.0;
    let model_b = s.models()[1].id;
    assert!(matches!(
        s.set_toolpath_tool(9, tool_b),
        Err(SessionError::ToolpathNotFound(9))
    ));
    assert!(matches!(
        s.set_toolpath_model(9, model_b),
        Err(SessionError::ToolpathNotFound(9))
    ));
}

/// The shape refusal is NOT this setter's job. An operation whose
/// registry entry rejects the bound tool is a *blocked* operation the
/// operator repairs by rebinding — refusing the rebind would remove the
/// only repair. The refusal stays at `ToolConstraintsDef::allows`, read
/// by the generators (R0.3 §2.2 row E1).
#[test]
fn a_rebind_to_a_tool_the_operation_rejects_is_allowed_and_the_generator_still_refuses() {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::BallNose));
    let _ = s.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let _ = s.add_model(empty_model("terrain.stl"));
    let ball = s.tools()[0].id.0;
    let flat = s.tools()[1].id.0;
    let model = s.models()[0].id;
    let _ = s
        .add_toolpath(
            0,
            tc(
                "scallop",
                OperationConfig::Scallop(
                    rs_cam_core::compute::operation_configs::ScallopConfig::default(),
                ),
                ball,
                model,
            ),
        )
        .unwrap();

    // Scallop's registry entry requires a ball tip. The rebind still lands.
    let _ = s.set_toolpath_tool(0, flat).unwrap();
    assert_eq!(s.toolpath_configs()[0].tool_id, flat);

    // …and the registry predicate the generator reads still says no.
    // (`generate_scallop` refuses on exactly this call, BEFORE it asks for
    // a mesh — `compute/execute.rs:generate_scallop`.)
    let constraints = OperationType::Scallop.registry_entry().tool_constraints;
    assert!(
        !constraints.allows(ToolType::EndMill.cutter_kind()),
        "the shape refusal must still exist after a rebind lands the tool"
    );
    assert!(constraints.allows(ToolType::BallNose.cutter_kind()));

    // The repair route works: rebind back, and the op is generatable again.
    let _ = s.set_toolpath_tool(0, ball).unwrap();
    assert_eq!(s.toolpath_configs()[0].tool_id, ball);
}

// ── the param route keeps its own meaning ────────────────────────

/// The pre-fix dead end, pinned so it stays a dead end. `tool_id` and
/// `model_id` are toolpath BINDINGS; `set_toolpath_param` writes
/// OPERATION params. No operation config declares either name, so the
/// key is refused with the list of params that ARE valid — which is the
/// signal that sent an agent to the right tool.
#[test]
fn rebind_is_not_reachable_through_the_param_route() {
    let mut s = seed();
    let tool_a = s.tools()[0].id.0;
    let tool_b = s.tools()[1].id.0;
    let model_a = s.models()[0].id;

    for key in ["tool_id", "model_id"] {
        let err = s
            .set_toolpath_param(0, key, serde_json::json!(tool_b))
            .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains(&format!("unknown parameter '{key}'")),
            "`{key}` must stay unknown to the param route: {text}"
        );
    }
    assert_eq!(s.toolpath_configs()[0].tool_id, tool_a);
    assert_eq!(s.toolpath_configs()[0].model_id, model_a);

    // The near miss: `prev_tool_id` IS an operation param on Rest, and it
    // is a DIFFERENT thing — the rest-analysis reference tool, not the
    // cutter this toolpath runs.
    let mut rest = tc(
        "rest",
        OperationConfig::Rest(RestConfig::default()),
        tool_a,
        model_a,
    );
    rest.stock_source = StockSource::Fresh;
    let _ = s.add_toolpath(0, rest).unwrap();
    let _ = s
        .set_toolpath_param(1, "prev_tool_id", serde_json::json!(tool_b))
        .expect("prev_tool_id is a real Rest param");
    assert_eq!(
        s.toolpath_configs()[1].tool_id,
        tool_a,
        "setting prev_tool_id must NOT move the toolpath's own tool binding"
    );
}

// ── helper ───────────────────────────────────────────────────────

fn fake_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::config::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}
