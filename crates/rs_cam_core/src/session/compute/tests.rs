//! Unit tests for the compute side of `ProjectSession`. Moved out of
//! `session/compute.rs` by P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::geo::P3;
use crate::session::{ProjectEvidence, VerdictKind, VerdictSeverity};

use super::diagnostics::{air_cut_offenders_for_toolpaths, plunge_stress_offenders_for_session};
use super::*;
use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
use crate::compute::operation_configs::{DrillConfig, PocketConfig, RestConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::toolpath_stats::ToolpathStats;
use crate::gcode::CoolantMode;
use crate::session::ToolpathConfig;
use crate::trace::debug_trace::ToolpathDebugOptions;
use serde_json::json;

fn make_session() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    let _ = s.add_tool(tool);
    s
}

fn make_tc(tool_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: "test".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::session::StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

// ── set_toolpath_param ───────────────────────────────────────

#[test]
fn set_toolpath_param_feed_rate() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "feed_rate", json!(2000.0)).unwrap();
    // Verify via OperationParams trait
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!((cfg.feed_rate - 2000.0).abs() < 1e-9),
        _ => panic!("expected Pocket"),
    }
}

#[test]
fn set_toolpath_param_plunge_rate() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let _ = s
        .set_toolpath_param(0, "plunge_rate", json!(500.0))
        .unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!((cfg.plunge_rate - 500.0).abs() < 1e-9),
        _ => panic!("expected Pocket"),
    }
}

#[test]
fn set_toolpath_param_stepover() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "stepover", json!(0.5)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!((cfg.stepover - 0.5).abs() < 1e-9),
        _ => panic!("expected Pocket"),
    }
}

#[test]
fn set_toolpath_param_depth_per_pass() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let _ = s
        .set_toolpath_param(0, "depth_per_pass", json!(1.5))
        .unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!((cfg.depth_per_pass - 1.5).abs() < 1e-9),
        _ => panic!("expected Pocket"),
    }
}

/// **DR-LIVE sentry.** `peck_depth`'s `ParamDef` now declares the
/// domain its emitter actually accepts, and `set_toolpath_param`
/// refuses outside it.
///
/// The pre-fix state (`TECH_DEBT_2_CLOSEOUT.md` §4.4, DR-LIVE):
/// `ParamDef::required("peck_depth", "f64")` carried **no range**, so
/// an agent could set `0` or `-3` through MCP and the setter said
/// `Ok`. `drill::fed_descents` then silently degraded the cycle to
/// one full-depth descent — a `Peck` cycle that does not peck, which
/// no surface reports. The GUI's `0.5..=50.0` widget clamp was the
/// only thing that had ever stopped it, and MCP does not go through
/// the widget.
///
/// Boundaries asserted, in the order that matters: `0.0` refused
/// (the emitter's exact `peck <= 0.0` guard), a negative refused, the
/// smallest sane positive accepted, and **the refused value not
/// applied** — a setter that rejects and mutates anyway is worse than
/// one that accepts.
#[test]
fn set_toolpath_param_refuses_a_peck_depth_the_emitter_would_refuse() {
    let mut s = make_session();
    let mut tc = make_tc(s.tools()[0].id.0);
    tc.operation = OperationConfig::Drill(DrillConfig {
        peck_depth: 3.0,
        ..DrillConfig::default()
    });
    let _ = s.add_toolpath(0, tc).unwrap();

    let peck = |s: &ProjectSession| -> f64 {
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
            other => panic!("expected Drill, got {other:?}"),
        }
    };

    for bad in [0.0_f64, -3.0, -0.000_001] {
        let err = s
            .set_toolpath_param(0, "peck_depth", json!(bad))
            .expect_err(
                "a peck the emitter degrades to a single full-depth descent must be \
                 refused at the setter, not accepted and silently neutered",
            );
        let msg = format!("{err}");
        assert!(
            msg.contains("peck_depth") && msg.contains("outside the accepted range"),
            "the refusal must name the param and the domain; got: {msg}"
        );
        assert!(
            (peck(&s) - 3.0).abs() < 1e-9,
            "a refused set must leave the value untouched; it became {}",
            peck(&s)
        );
    }

    // The accepting side of the same boundary.
    let _ = s.set_toolpath_param(0, "peck_depth", json!(0.5)).unwrap();
    assert!((peck(&s) - 0.5).abs() < 1e-9);

    // And the domain is published, so an agent can read it before
    // guessing: `get_operation_schema` carries it.
    let schema = ProjectSession::operation_schema("drill").unwrap();
    let entry = schema
        .params
        .iter()
        .find(|p| p.name == "peck_depth")
        .expect("drill schema must list peck_depth");
    let range = entry
        .range
        .as_ref()
        .expect("peck_depth must publish its range");
    assert_eq!(range["min"], json!(0.0));
    assert_eq!(range["min_exclusive"], json!(true));
    assert_eq!(range["finite"], json!(true));
}

/// The **residual** the sentry above deliberately does not close, so
/// it is on the record rather than implied away.
///
/// `ParamRange::greater_than(0.0)` matches `drill::fed_descents`'
/// guard exactly (`!peck.is_finite() || peck <= 0.0`). It bounds the
/// *sign and finiteness* of the peck. It does **not** bound the peck
/// COUNT: `fed_descents` has no cap, so descents scale as
/// `depth / peck` without limit, and a positive-but-tiny peck set
/// through MCP is still a practical hang (allocation-bound, not a
/// spin). This is measured at safe magnitudes and asserted as a
/// TREND, not run at the magnitude that would take the machine down.
///
/// Not fixed here: capping the emitter is a behavioural change to
/// generation, and S-5's brief is to align the ParamDef. Reported as
/// TD3 intake in this wave's log entry.
#[test]
fn a_positive_peck_still_has_no_descent_cap() {
    use crate::ops::drill::{DrillCycle, fed_descents};

    let counts: Vec<usize> = [1.0_f64, 0.1, 0.01, 0.001]
        .iter()
        .map(|p| fed_descents(DrillCycle::Peck(*p), -10.0, 5.0).len())
        .collect();

    // Tolerant by one step at each magnitude: the loop accumulates
    // `current_z - peck` in f64 and the final step is clamped, so the
    // last descent can land on either side of the boundary. The
    // CLAIM is the 10× growth, not the exact integer.
    for (i, (&count, expected)) in counts.iter().zip([15, 150, 1500, 15_000]).enumerate() {
        assert!(
            count.abs_diff(expected) <= 1,
            "descents must scale as (retract - bottom) / peck with no cap — at \
             magnitude {i} expected ~{expected}, got {count}. If this list stops \
             growing linearly a cap has been added, and the DR-LIVE residual \
             recorded in this test's doc can be closed"
        );
    }
}

#[test]
fn set_toolpath_param_coerces_integer_to_bool() {
    // MCP clients that can only produce JSON numbers should still be
    // able to set boolean params like `climb`, `z_blend`, etc.
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    // Pocket default has climb=true; flip it via integer 0.
    let _ = s.set_toolpath_param(0, "climb", json!(0)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
        _ => panic!("expected Pocket"),
    }
    // Flip back with integer 1.
    let _ = s.set_toolpath_param(0, "climb", json!(1)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!(cfg.climb),
        _ => panic!("expected Pocket"),
    }
    // Non-0/1 integers fall through to serde, which will reject them.
    let result = s.set_toolpath_param(0, "climb", json!(42));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    // Actual booleans still work.
    let _ = s.set_toolpath_param(0, "climb", json!(false)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
        _ => panic!("expected Pocket"),
    }
}

fn make_rest_tc(tool_id: usize) -> ToolpathConfig {
    let mut tc = make_tc(tool_id);
    tc.operation = OperationConfig::Rest(RestConfig::default());
    tc
}

fn make_drill_tc(tool_id: usize) -> ToolpathConfig {
    let mut tc = make_tc(tool_id);
    tc.operation = OperationConfig::Drill(DrillConfig::default());
    tc
}

#[test]
fn set_toolpath_param_drill_plunge_rate_updates_feed_rate() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_drill_tc(s.tools()[0].id.0)).unwrap();
    let _ = s
        .set_toolpath_param(0, "plunge_rate", json!(250.0))
        .unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Drill(cfg) => assert_eq!(cfg.feed_rate, 250.0),
        _ => panic!("expected Drill"),
    }
}

#[test]
fn set_toolpath_param_prev_tool_id_accepts_int() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "prev_tool_id", json!(1)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
        _ => panic!("expected Rest"),
    }
}

#[test]
fn set_toolpath_param_prev_tool_id_accepts_string() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "prev_tool_id", json!("1")).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
        _ => panic!("expected Rest"),
    }
}

#[test]
fn set_toolpath_param_prev_tool_id_accepts_float_wire_number() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "prev_tool_id", json!(1.0)).unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, Some(ToolId(1))),
        _ => panic!("expected Rest"),
    }
}

#[test]
fn set_toolpath_param_prev_tool_id_accepts_null_to_clear() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_rest_tc(s.tools()[0].id.0)).unwrap();
    let _ = s.set_toolpath_param(0, "prev_tool_id", json!(1)).unwrap();
    let _ = s
        .set_toolpath_param(0, "prev_tool_id", serde_json::Value::Null)
        .unwrap();
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Rest(cfg) => assert_eq!(cfg.prev_tool_id, None),
        _ => panic!("expected Rest"),
    }
}

#[test]
fn get_operation_schema_rest_lists_prev_tool_id() {
    let schema = ProjectSession::operation_schema("rest").unwrap();
    let prev = schema
        .params
        .iter()
        .find(|param| param.name == "prev_tool_id")
        .unwrap();
    assert_eq!(prev.type_name, "option<usize>");
    assert!(prev.optional);
    assert_eq!(prev.default, serde_json::Value::Null);
}

#[test]
fn get_operation_schema_unknown_op_returns_error() {
    let err = ProjectSession::operation_schema("not_real")
        .unwrap_err()
        .to_string();
    assert!(err.contains("Valid operation_type values"));
    assert!(err.contains("rest"));
}

#[test]
fn get_operation_schema_drill_has_drill_specific_fields() {
    let schema = ProjectSession::operation_schema("drill").unwrap();
    let names: std::collections::HashSet<_> = schema
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    assert!(names.contains("cycle"));
    assert!(names.contains("peck_depth"));
    assert!(names.contains("retract_z"));
}

#[test]
fn operation_schema_params_match_params_with_nulls_for_every_op() {
    for &op_type in crate::compute::catalog::OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let params = op.params_value_including_nulls();
        let param_obj = params.as_object().unwrap();
        let schema = OperationConfig::schema_for_type(op_type);
        let schema_names: std::collections::HashSet<_> = schema
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        let param_names: std::collections::HashSet<_> =
            param_obj.keys().map(String::as_str).collect();
        assert_eq!(schema_names, param_names, "schema mismatch for {op_type:?}");
    }
}

#[test]
fn set_toolpath_param_wrong_type_errors() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let result = s.set_toolpath_param(0, "feed_rate", json!("not a number"));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
}

#[test]
fn set_toolpath_param_unknown_param() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let result = s.set_toolpath_param(0, "totally_fake_param", json!(42.0));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown parameter 'totally_fake_param'"));
    assert!(err.contains("Valid parameters"));
    assert!(err.contains("stepover"));
}

#[test]
fn set_toolpath_param_invalid_index() {
    let mut s = make_session();
    let result = s.set_toolpath_param(99, "feed_rate", json!(100.0));
    assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
}

#[test]
fn set_toolpath_param_spindle_rpm() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    // Default is None.
    assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
    let _ = s
        .set_toolpath_param(0, "spindle_rpm", json!(15000))
        .unwrap();
    assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(15000));
}

#[test]
fn set_toolpath_param_spindle_rpm_null_clears() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let _ = s
        .set_toolpath_param(0, "spindle_rpm", json!(20_000))
        .unwrap();
    assert_eq!(
        s.toolpath_configs()[0].operation.spindle_rpm(),
        Some(20_000)
    );
    let _ = s
        .set_toolpath_param(0, "spindle_rpm", serde_json::Value::Null)
        .unwrap();
    assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
}

#[test]
fn set_toolpath_param_spindle_rpm_invalid_type() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let result = s.set_toolpath_param(0, "spindle_rpm", json!("not a number"));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    // Negative numbers fail u64 conversion.
    let result = s.set_toolpath_param(0, "spindle_rpm", json!(-1));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
}

/// F2 — MCP / JSON-RPC clients sometimes serialize integer literals as
/// f64 (so 13500 arrives as 13500.0). The router accepts integer-valued
/// floats and parseable numeric strings as well as plain integers.
#[test]
fn set_toolpath_param_spindle_rpm_accepts_f64_and_string() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    // Integer-valued f64.
    let _ = s
        .set_toolpath_param(0, "spindle_rpm", json!(13500.0))
        .unwrap();
    assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(13500));
    // Numeric string.
    let _ = s
        .set_toolpath_param(0, "spindle_rpm", json!("18000"))
        .unwrap();
    assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(18000));
    // Non-integer float is rejected (would lose precision).
    let result = s.set_toolpath_param(0, "spindle_rpm", json!(13500.5));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
}

#[test]
fn set_toolpath_param_invalidates_result() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    s.results.insert(
        0,
        ToolpathComputeResult {
            op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(
                crate::trace::toolpath_spans::AnnotatedToolpath::new(
                    crate::toolpath::Toolpath::new(),
                ),
            )),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        },
    );
    let _ = s.set_toolpath_param(0, "feed_rate", json!(1000.0)).unwrap();
    assert!(!s.results.contains_key(&0));
}

// ── set_tool_param ───────────────────────────────────────────

fn feed_vs_lut_high_recommended_value(s: &ProjectSession) -> f64 {
    let diagnostics = s.diagnose_toolpath(0).unwrap();
    let diag = diagnostics
        .iter()
        .find(|d| d.id.0 == crate::diagnostics::ids::FEEDS_FEED_VS_LUT_HIGH)
        .expect("feeds.feed_vs_lut.high diagnostic");
    match diag.evidence.as_ref().expect("diagnostic evidence") {
        crate::diagnostics::DiagnosticEvidence::GeometryCompare {
            rhs_label,
            rhs_value,
            ..
        } => {
            assert_eq!(rhs_label, "recommended");
            *rhs_value
        }
        other => panic!("unexpected evidence: {other:?}"),
    }
}

#[test]
fn suggest_output_matches_feed_vs_lut_high_diagnostic_recommendation() {
    let mut s = make_session();
    let tool = s.tools()[0].clone();
    let mut tc = make_tc(tool.id.0);
    let suggested = crate::feeds::suggest::suggest_for_operation(
        crate::feeds::suggest::SuggestForOperationInput {
            operation: &tc.operation,
            tool: &tool,
            machine: s.machine(),
            material: &s.stock_config().material,
            workholding: s.stock_config().workholding_rigidity,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: crate::feeds::suggest::SuggestContext::default(),
        },
    )
    .expect("test fixture pairs a flat endmill with a Pocket op — not a refused combination");
    tc.operation
        .set_feed_rate(suggested.feeds_result.feed_rate_mm_min * 3.0);
    let _ = s.add_toolpath(0, tc).unwrap();

    let diagnostic_rec = feed_vs_lut_high_recommended_value(&s);
    assert!((diagnostic_rec - suggested.feeds_result.feed_rate_mm_min).abs() < 1e-6);
}

#[test]
fn workholding_changes_suggest_output_and_diagnostic_baseline_consistently() {
    fn session_for_workholding(
        workholding: crate::feeds::WorkholdingRigidity,
    ) -> (ProjectSession, f64) {
        let mut s = make_session();
        let mut stock = s.stock_config().clone();
        stock.workholding_rigidity = workholding;
        // Use a Custom material so the suggest path takes the
        // hardness/Kc fallback model rather than a vendor-LUT match.
        // The 2026-05-31 Phase 4 promotion added Onsrud-grade 6.35 mm
        // softwood pocket rows whose chipload max saturates the
        // suggested feed at both rigidity levels — that's correct
        // behavior for the suggest pipeline but defeats this test's
        // *intent*, which is to verify rigidity flows consistently
        // through both `suggest_for_operation` and the diagnostic
        // baseline. Custom material isolates the rigidity scaler.
        stock.material = crate::material::Material::Custom {
            name: "test_workholding_fixture".to_owned(),
            feed_scale_factor: 1.5,
            kc: 25.0,
        };
        let _ = s.set_stock_config(stock);
        let tool = s.tools()[0].clone();
        let mut tc = make_tc(tool.id.0);
        let suggested = crate::feeds::suggest::suggest_for_operation(
            crate::feeds::suggest::SuggestForOperationInput {
                operation: &tc.operation,
                tool: &tool,
                machine: s.machine(),
                material: &s.stock_config().material,
                workholding,
                lut: crate::feeds::embedded_vendor_lut(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
                context: crate::feeds::suggest::SuggestContext::default(),
            },
        )
        .expect("test fixture pairs a flat endmill with a Pocket op — not a refused combination");
        tc.operation
            .set_feed_rate(suggested.feeds_result.feed_rate_mm_min * 3.0);
        let _ = s.add_toolpath(0, tc).unwrap();
        (s, suggested.feeds_result.feed_rate_mm_min)
    }

    let (medium, medium_suggest) =
        session_for_workholding(crate::feeds::WorkholdingRigidity::Medium);
    let (high, high_suggest) = session_for_workholding(crate::feeds::WorkholdingRigidity::High);

    let medium_diag = feed_vs_lut_high_recommended_value(&medium);
    let high_diag = feed_vs_lut_high_recommended_value(&high);
    assert!(high_suggest > medium_suggest);
    assert!((medium_diag - medium_suggest).abs() < 1e-6);
    assert!((high_diag - high_suggest).abs() < 1e-6);
}

// WP28 deleted `compute_stale_set` and `MutationKind`. Three unit
// tests stood here and pinned the tag-driven answer for a tool
// param, a setup change and a stock change. `Effects::stale` is the
// one answer now, and `tests/command_registry_completeness.rs`
// measures it against the set the setter dropped.

#[test]
fn set_tool_param_diameter() {
    let mut s = make_session();
    let _ = s.set_tool_param(0, "diameter", &json!(6.0)).unwrap();
    assert!((s.tools()[0].diameter - 6.0).abs() < 1e-9);
}

#[test]
fn set_tool_param_flute_count() {
    let mut s = make_session();
    let _ = s.set_tool_param(0, "flute_count", &json!(4)).unwrap();
    assert_eq!(s.tools()[0].flute_count, 4);
}

#[test]
fn set_tool_param_stickout() {
    let mut s = make_session();
    let _ = s.set_tool_param(0, "stickout", &json!(25.0)).unwrap();
    assert!((s.tools()[0].stickout - 25.0).abs() < 1e-9);
}

#[test]
fn set_tool_param_corner_radius() {
    let mut s = make_session();
    let _ = s.set_tool_param(0, "corner_radius", &json!(0.5)).unwrap();
    assert!((s.tools()[0].corner_radius - 0.5).abs() < 1e-9);
}

#[test]
fn set_tool_param_cutting_length() {
    let mut s = make_session();
    let _ = s.set_tool_param(0, "cutting_length", &json!(20.0)).unwrap();
    assert!((s.tools()[0].cutting_length - 20.0).abs() < 1e-9);
}

#[test]
fn set_tool_param_invalid_index() {
    let mut s = make_session();
    let result = s.set_tool_param(99, "diameter", &json!(6.0));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
}

#[test]
fn set_tool_param_wrong_type() {
    let mut s = make_session();
    let result = s.set_tool_param(0, "diameter", &json!("not a number"));
    assert!(matches!(result, Err(SessionError::InvalidParam(_))));
}

#[test]
fn set_tool_param_invalidates_toolpath_results() {
    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    s.results.insert(
        0,
        ToolpathComputeResult {
            op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(
                crate::trace::toolpath_spans::AnnotatedToolpath::new(
                    crate::toolpath::Toolpath::new(),
                ),
            )),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        },
    );

    let _ = s.set_tool_param(0, "diameter", &json!(8.0)).unwrap();
    assert!(!s.results.contains_key(&0));
}

// ── generate_toolpath error paths ────────────────────────────

#[test]
fn generate_toolpath_not_found() {
    let mut s = make_session();
    let cancel = AtomicBool::new(false);
    let result = s.generate_toolpath(99, &cancel);
    assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
}

/// Rest machining FAILS HARD instead of silently clearing fresh stock.
/// A `FromRemainingStock` op with no simulated remaining-stock snapshot must
/// error at generate time — regression net for the fresh-fallback runaway
/// where a fine rest tool, seeded with fresh stock, cleared the whole part
/// (unbounded compute). The precondition is checked at `generate_toolpath`
/// entry, before any geometry work.
#[test]
fn generate_from_remaining_stock_without_sim_errors_hard() {
    let mut s = make_session();
    let mut tc = make_tc(s.tools()[0].id.0);
    tc.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, tc).unwrap();
    let cancel = AtomicBool::new(false);
    match s.generate_toolpath(0, &cancel) {
        Err(SessionError::OperationFailed(msg)) => assert!(
            msg.contains("remaining stock"),
            "error should name the missing rest-stock snapshot: {msg}"
        ),
        Err(other) => panic!("expected OperationFailed, got: {other}"),
        Ok(_) => panic!("rest op without a prior sim must error, not clear fresh stock"),
    }
}

/// Fixture for the F.4 phantom-prior-stock tests below: one tool plus a
/// small square-polygon model at `model_id == 0` (matching `make_tc`'s
/// default), so a `Pocket` op generates a real, multi-move toolpath.
/// `run_simulation`'s request builder skips any toolpath with fewer
/// than 2 moves, so an empty/geometry-less fixture would never
/// populate `prior_stocks` at all.
fn make_session_with_pocket_model() -> ProjectSession {
    let mut s = make_session();
    let polygon = crate::polygon::Polygon2::new(vec![
        crate::geo::P2::new(0.0, 0.0),
        crate::geo::P2::new(30.0, 0.0),
        crate::geo::P2::new(30.0, 30.0),
        crate::geo::P2::new(0.0, 30.0),
    ]);
    let model = crate::session::LoadedModel {
        id: 0,
        name: "phantom_prior_stock_fixture".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://phantom_prior_stock_fixture.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let _ = s.add_model(model);
    s
}

/// F.4 — the core half of the regression net for the
/// `FromRemainingStock` regeneration catch-22. TP1 is preceded by a
/// generated TP0 in the same setup: before any simulation, TP1 must
/// still fail hard (unchanged precondition); after `run_simulation`
/// populates the phantom `prior_stocks` snapshot for TP1 (the first
/// pending toolpath in its group), TP1 must regenerate successfully —
/// closing the catch-22 where an ungenerated toolpath, never present
/// in a `SimGroupEntry`, could never receive a snapshot at all.
#[test]
fn phantom_prior_stock_unlocks_regeneration_after_sim() {
    let mut s = make_session_with_pocket_model();
    let tool_id = s.tools()[0].id.0;
    let _ = s.add_toolpath(0, make_tc(tool_id)).unwrap();
    let mut rest_tc = make_tc(tool_id);
    rest_tc.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest_tc).unwrap();

    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("TP0 (Fresh) should generate against real polygon geometry");

    // Before any simulation: TP1 still fails hard (unchanged
    // precondition — generating never falls back to fresh stock).
    match s.generate_toolpath(1, &cancel) {
        Err(SessionError::OperationFailed(_)) => {}
        Err(other) => panic!("expected OperationFailed before any sim, got error: {other:?}"),
        Ok(_) => panic!("expected OperationFailed before any sim, got a generated toolpath"),
    }

    s.run_simulation(&SimulationOptions::default(), &cancel)
        .expect("simulation over TP0 should succeed and populate prior_stocks");

    // F.4: TP1 is the first (and only) pending toolpath in its group,
    // so `run_simulation` recorded a phantom snapshot for it — it must
    // now regenerate.
    s.generate_toolpath(1, &cancel)
        .expect("TP1 should regenerate once the phantom prior-stock snapshot exists");
}

/// F.4 ladder rule: with TWO consecutive pending `FromRemainingStock`
/// toolpaths after a generated TP0, one simulation run unlocks only
/// the FIRST pending toolpath (TP1). TP2 stays gated — its snapshot
/// would be missing TP1's cuts (TP1 hasn't itself been generated and
/// re-simulated yet), which for a rest-machining op means real
/// overcut risk, not just a stale preview.
#[test]
fn phantom_prior_stock_ladder_unlocks_only_first_pending_op() {
    let mut s = make_session_with_pocket_model();
    let tool_id = s.tools()[0].id.0;
    let _ = s.add_toolpath(0, make_tc(tool_id)).unwrap();
    let mut rest_tc_1 = make_tc(tool_id);
    rest_tc_1.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest_tc_1).unwrap();
    let mut rest_tc_2 = make_tc(tool_id);
    rest_tc_2.stock_source = crate::session::StockSource::FromRemainingStock;
    let _ = s.add_toolpath(0, rest_tc_2).unwrap();

    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("TP0 (Fresh) should generate against real polygon geometry");

    s.run_simulation(&SimulationOptions::default(), &cancel)
        .expect("simulation over TP0 should succeed");

    // TP1 is the first pending op in the group — unlocked.
    s.generate_toolpath(1, &cancel)
        .expect("TP1 should regenerate: first pending op in its group");

    // TP2 is still pending behind TP1, which hasn't itself been
    // generated + re-simulated — the ladder rule keeps it gated.
    match s.generate_toolpath(2, &cancel) {
        Err(SessionError::OperationFailed(_)) => {}
        Err(other) => panic!(
            "TP2 must stay gated until TP1 is regenerated and re-simulated, got error: \
             {other:?}"
        ),
        Ok(_) => panic!(
            "TP2 must stay gated until TP1 is regenerated and re-simulated, but it generated"
        ),
    }
}

// ── diagnostics ──────────────────────────────────────────────

#[test]
fn diagnostics_empty() {
    let s = ProjectSession::new_empty();
    let diag = s.diagnostics();
    assert!(diag.per_toolpath.is_empty());
    assert!(diag.verdicts.is_empty());
}

// ── Verdict layer (PR-1: A4 + B8 + B1-verdict + A12 + C7) ──────

fn make_session_with_two_tps() -> ProjectSession {
    let mut s = make_session();
    // TP0: Pocket — exercising A4 (TP-named verdict) and C7 (empty cut).
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    // TP1: Drill — exercises A12 (op_kind tag).
    let mut drill_tc = make_tc(s.tools()[0].id.0);
    drill_tc.operation =
        OperationConfig::Drill(crate::compute::operation_configs::DrillConfig::default());
    drill_tc.name = "Pin holes".to_owned();
    let _ = s.add_toolpath(0, drill_tc).unwrap();
    s
}

fn empty_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(
            crate::trace::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new()),
        )),
        stats: ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Setup-tab lag fix (2026-06-11): holder/shank collisions are
/// EVIDENCE consumed by `diagnostics_with_evidence`, not something
/// it computes. Empty evidence → no holder verdict even though
/// results exist; supplied counts → verdict with exactly those
/// counts. (The old behavior ran a full `collision_check` sweep per
/// toolpath inside diagnostics, which the GUI setup panel then
/// executed every frame.)
#[test]
fn diagnostics_with_evidence_consumes_holder_counts_instead_of_computing() {
    let mut s = make_session_with_two_tps();
    let mut r = empty_result();
    r.stats.cutting_distance = 100.0;
    s.results.insert(0, r);

    // No holder evidence → no holder verdict, zero count.
    let diag = s.diagnostics_with_evidence(&ProjectEvidence::default());
    assert_eq!(diag.collision_count, 0);
    assert_eq!(diag.collision_checks_failed, 0);
    assert!(
        !diag
            .verdicts
            .iter()
            .any(|v| matches!(v.kind, crate::session::VerdictKind::HolderCollision)),
        "no holder verdict without holder evidence"
    );
    // CMP-14: a toolpath with NO evidence is not measured, so it carries
    // no count at all. It used to carry a zero.
    assert_eq!(diag.per_toolpath[0].collision_count, None);

    // Supplied counts surface verbatim.
    let tp0_id = s.toolpath_configs()[0].id;
    let evidence = ProjectEvidence {
        holder_collisions: vec![(
            tp0_id,
            crate::stock::collision::HolderCollisionCheck::Measured(3),
        )],
        ..ProjectEvidence::default()
    };
    let diag = s.diagnostics_with_evidence(&evidence);
    assert_eq!(diag.collision_count, 3);
    let verdict = diag
        .verdicts
        .iter()
        .find(|v| matches!(v.kind, crate::session::VerdictKind::HolderCollision))
        .expect("holder verdict from supplied evidence");
    assert_eq!(verdict.evidence.count, Some(3));
    assert_eq!(verdict.offender_toolpath_ids, vec![tp0_id]);
}

/// CMP-24: the holder-collision sweep builds ONE spatial index per
/// DISTINCT model, not one per toolpath.
///
/// The build is the expensive half of a collision check. Before this row
/// the only entry point built its own index on every call, so a project
/// whose N toolpaths bind one model paid for the same index N times — and
/// the session's answer to that cost was a doc line telling callers not to
/// call it.
#[test]
fn the_collision_sweep_builds_one_index_per_model() {
    fn mesh_model(id: usize, name: &str) -> crate::session::LoadedModel {
        crate::session::LoadedModel {
            id,
            name: name.to_owned(),
            mesh: Some(Arc::new(crate::mesh::make_test_hemisphere(10.0, 8))),
            polygons: None,
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            path: std::path::PathBuf::from(format!("synthetic://{name}.stl")),
            kind: None,
            units: None,
            enriched_mesh: None,
            winding_report: None,
            load_error: None,
        }
    }

    let mut s = make_session();
    s.models.push(mesh_model(0, "one"));
    for _ in 0..3 {
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    }
    for index in 0..3 {
        s.results.insert(index, empty_result());
    }

    assert_eq!(
        s.holder_collision_indices().len(),
        1,
        "three toolpaths on one model must share one spatial index"
    );

    // And the hoist is per MODEL, not a blanket "build one": a second model
    // earns a second index.
    s.models.push(mesh_model(1, "two"));
    let mut second = make_tc(s.tools()[0].id.0);
    second.model_id = 1;
    let _ = s.add_toolpath(0, second).unwrap();
    s.results.insert(3, empty_result());

    assert_eq!(
        s.holder_collision_indices().len(),
        2,
        "a toolpath on a second model needs that model's own index"
    );
}

/// A12: per-toolpath diagnostics carry an `op_kind` snake_case tag so
/// downstream consumers can suppress rapid:cut-ratio signals on
/// drill / pin-drill ops.
#[test]
fn diagnostics_tags_per_tp_with_op_kind() {
    let mut s = make_session_with_two_tps();
    // Both TPs need a result to appear in per_toolpath; cutting_distance
    // is non-zero so the C7 GeneratedEmpty verdict doesn't fire for TP0.
    let mut r = empty_result();
    r.stats.cutting_distance = 100.0;
    s.results.insert(0, r);
    let mut r1 = empty_result();
    r1.stats.cutting_distance = 50.0;
    s.results.insert(1, r1);

    let diag = s.diagnostics();
    assert_eq!(diag.per_toolpath.len(), 2);
    let pocket = diag
        .per_toolpath
        .iter()
        .find(|d| d.name == "test")
        .expect("pocket TP present");
    assert_eq!(pocket.op_kind, "pocket");
    let drill = diag
        .per_toolpath
        .iter()
        .find(|d| d.name == "Pin holes")
        .expect("drill TP present");
    assert_eq!(drill.op_kind, "drill");
}

/// C7: a non-drill toolpath that generated successfully but laid down
/// zero in-material cut emits a GeneratedEmpty verdict naming the TP.
/// Drill toolpaths with zero cutting distance are intentionally
/// exempt — the dexel cutting metric doesn't apply to Z-only ops.
#[test]
fn diagnostics_emits_generated_empty_for_zero_cut_non_drill() {
    let mut s = make_session_with_two_tps();
    s.results.insert(0, empty_result()); // pocket, cutting_distance == 0
    // Drill also has zero cutting_distance but should NOT trigger C7.
    s.results.insert(1, empty_result());

    let diag = s.diagnostics();
    let empty_verdicts: Vec<_> = diag
        .verdicts
        .iter()
        .filter(|v| v.kind == VerdictKind::GeneratedEmpty)
        .collect();
    assert_eq!(
        empty_verdicts.len(),
        1,
        "expected one GeneratedEmpty verdict (pocket); drill must be exempt: {:?}",
        diag.verdicts
    );
    let v = empty_verdicts[0];
    assert_eq!(v.severity, VerdictSeverity::Important);
    assert!(
        v.headline.contains("'test'"),
        "headline should name the offending TP: {}",
        v.headline
    );
    assert_eq!(v.offender_toolpath_ids, vec![s.toolpath_configs[0].id]);
    assert!(!v.fix_hint.is_empty(), "fix_hint must be populated");
}

/// B8: verdicts are severity-ranked (Critical → Important → Polish).
/// When several conditions are present the list returns all of them
/// in priority order instead of picking only one (the old early-exit).
#[test]
fn diagnostics_ranks_verdicts_by_severity() {
    use crate::compute::simulate::{SimBoundary, SimulationResult};
    use crate::dexel_stock::StockCutDirection;
    use crate::stock::stock_mesh::StockMesh;

    let mut s = make_session_with_two_tps();
    // TP0 (pocket): zero cut → C7 GeneratedEmpty (Important).
    s.results.insert(0, empty_result());
    // TP1 (drill): nonzero cut so it stays out of GeneratedEmpty.
    let mut r1 = empty_result();
    r1.stats.cutting_distance = 50.0;
    s.results.insert(1, r1);

    // Inject a simulation result that carries a rapid-through-stock
    // collision on TP0 → triggers a Critical RapidCollision verdict.
    let pocket_tp_id = s.toolpath_configs[0].id;
    s.simulation = Some(SimulationResult {
        mesh: StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 5,
        deviations: None,
        column_deviations: None,
        boundaries: vec![SimBoundary {
            id: pocket_tp_id,
            name: "test".to_owned(),
            tool_name: "EM".to_owned(),
            start_move: 0,
            end_move: 5,
            direction: StockCutDirection::FromTop,
        }],
        checkpoints: Vec::new(),
        rapid_collisions: vec![crate::stock::collision::RapidCollision {
            move_index: 2,
            start: P3::new(0.0, 0.0, 5.0),
            end: P3::new(0.0, 0.0, -2.5),
        }],
        rapid_collision_move_indices: vec![2],
        cut_trace: None,
        resolution_clamped: false,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    });

    let diag = s.diagnostics();
    assert!(
        diag.verdicts.len() >= 2,
        "expected both Critical + Important verdicts; got {:?}",
        diag.verdicts
    );
    // Critical comes before Important.
    assert_eq!(diag.verdicts[0].severity, VerdictSeverity::Critical);
    assert_eq!(diag.verdicts[0].kind, VerdictKind::RapidCollision);
    let importants: Vec<_> = diag
        .verdicts
        .iter()
        .filter(|v| v.severity == VerdictSeverity::Important)
        .collect();
    assert!(
        importants
            .iter()
            .any(|v| v.kind == VerdictKind::GeneratedEmpty),
        "GeneratedEmpty must still surface alongside RapidCollision"
    );
}

/// B1 (verdict half) + A4: rapid-collision verdict names the offending
/// TP, quotes the collision count, cites the worst move's z, and offers
/// a fix hint pointing at retract_z / safe-Z / boundary config.
#[test]
fn diagnostics_rapid_collision_verdict_carries_evidence() {
    use crate::compute::simulate::{SimBoundary, SimulationResult};
    use crate::dexel_stock::StockCutDirection;
    use crate::stock::stock_mesh::StockMesh;

    let mut s = make_session();
    let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
    let mut r = empty_result();
    r.stats.cutting_distance = 100.0;
    s.results.insert(0, r);

    let tp_id = s.toolpath_configs[0].id;
    s.simulation = Some(SimulationResult {
        mesh: StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 10,
        deviations: None,
        column_deviations: None,
        boundaries: vec![SimBoundary {
            id: tp_id,
            name: "test".to_owned(),
            tool_name: "EM".to_owned(),
            start_move: 0,
            end_move: 10,
            direction: StockCutDirection::FromTop,
        }],
        checkpoints: Vec::new(),
        rapid_collisions: vec![
            crate::stock::collision::RapidCollision {
                move_index: 1,
                start: P3::new(0.0, 0.0, 5.0),
                end: P3::new(0.0, 0.0, 1.0),
            },
            // Deepest rapid — this should be cited as the worst move.
            crate::stock::collision::RapidCollision {
                move_index: 7,
                start: P3::new(1.0, 1.0, 5.0),
                end: P3::new(1.0, 1.0, -3.25),
            },
        ],
        rapid_collision_move_indices: vec![1, 7],
        cut_trace: None,
        resolution_clamped: false,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    });

    let diag = s.diagnostics();
    let v = diag
        .verdicts
        .iter()
        .find(|v| v.kind == VerdictKind::RapidCollision)
        .expect("rapid collision verdict must fire");
    assert_eq!(v.severity, VerdictSeverity::Critical);
    // A4: names the TP.
    assert!(
        v.headline.contains("'test'"),
        "headline names offending TP: {}",
        v.headline
    );
    // Count is quoted.
    assert!(
        v.headline.contains("2 collisions"),
        "headline: {}",
        v.headline
    );
    // Worst-move evidence cites move_index=7 and z=-3.250.
    assert_eq!(v.evidence.move_index, Some(7));
    assert_eq!(v.evidence.count, Some(2));
    assert!(
        v.evidence.z_value.is_some() && (v.evidence.z_value.unwrap_or(0.0) - (-3.25)).abs() < 1e-6,
        "evidence.z_value: {:?}",
        v.evidence.z_value
    );
    // Fix hint mentions retract_z (the operator's lever).
    let hint_lc = v.fix_hint.to_lowercase();
    assert!(
        hint_lc.contains("retract_z") || hint_lc.contains("safe-z") || hint_lc.contains("boundary"),
        "fix_hint must point at retract_z / safe-Z / boundary: {}",
        v.fix_hint
    );
    assert_eq!(v.offender_toolpath_ids, vec![tp_id]);
}

// ── P1: op-kind-aware air-cut thresholds ──────────────────────

use crate::compute::operation_configs::{
    Adaptive3dConfig, AlignmentPinDrillConfig, DropCutterConfig, ProjectCurveConfig,
};
use crate::stock::simulation_cut::SimulationToolpathCutSummary;

fn make_tp(id: usize, name: &str, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: 0,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::session::StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn summary(id: usize, air_pct: f64) -> SimulationToolpathCutSummary {
    let total = 100.0;
    SimulationToolpathCutSummary {
        toolpath_id: ToolpathId(id),
        sample_count: 0,
        total_runtime_s: total,
        cutting_runtime_s: total * (1.0 - air_pct / 100.0),
        rapid_runtime_s: 0.0,
        air_cut_time_s: total * air_pct / 100.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.0,
        peak_chipload_mm_per_tooth: 0.0,
        peak_axial_doc_mm: 0.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 0.0,
        average_mrr_mm3_s: 0.0,
        metrics_not_applicable: false,
        per_kinematics: std::collections::BTreeMap::new(),
        runtime_by_intent: None,
    }
}

#[test]
fn air_cut_offenders_silent_on_sparse_project_curve() {
    // Wanaka TP3 rivers: sparse-by-construction air-cut is intrinsic, not
    // a defect. W5B-F4: the fixture used to be 92.1, the pre-swept-kernel
    // reading the old 97 band was fitted to. Under the shipped swept
    // kernel the SAME project's rivers read 15.97 (lakes 10.90), so the
    // fixture now carries the measured post-flip number and the band is 60
    // (`DELTA_w5b_f4_aircut_DECISION.md` §3.d / §5.2). 92.1 is no longer a
    // ProjectCurve baseline anywhere in the repo — it was the artifact.
    let tps = vec![make_tp(
        0,
        "Rivers (back)",
        OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
    )];
    let sums = vec![summary(0, 15.97)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert!(
        offenders.is_empty(),
        "ProjectCurve at baseline air-cut should not warn; got {offenders:?}"
    );
}

#[test]
fn air_cut_offenders_silent_on_adaptive3d_below_threshold() {
    // Wanaka TP1: 28.5% air-cut on Adaptive3d should be silent.
    let tps = vec![make_tp(
        0,
        "Back Rough",
        OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
    )];
    let sums = vec![summary(0, 28.5)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert!(
        offenders.is_empty(),
        "Adaptive3d below 40% threshold should be silent; got {offenders:?}"
    );
}

#[test]
fn air_cut_offenders_silent_on_drop_cutter_finish() {
    // Wanaka TP7: 11.5% air-cut on DropCutter is healthy.
    let tps = vec![make_tp(
        0,
        "3D Finish 6",
        OperationConfig::DropCutter(DropCutterConfig::default()),
    )];
    let sums = vec![summary(0, 11.5)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert!(offenders.is_empty(), "DropCutter at 11.5% should be silent");
}

#[test]
fn air_cut_gate_ignores_disabled_toolpaths() {
    // Census D5. A disabled toolpath is not part of the job, but its
    // summary survives the toggle, so the verdict used to keep warning
    // about an op that will never run — with no way to silence it. The
    // sibling plunge-stress scan has always checked `enabled`.
    let mut tps = vec![make_tp(
        0,
        "Switched Off",
        OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
    )];
    let sums = vec![summary(0, 60.0)];
    assert_eq!(
        air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default())
            .offenders
            .len(),
        1,
        "enabled toolpath over the band must warn"
    );

    tps[0].enabled = false;
    assert!(
        air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default())
            .offenders
            .is_empty(),
        "a disabled toolpath must not raise a verdict"
    );
}

#[test]
fn air_cut_offenders_carry_their_own_id_not_a_name_lookup() {
    // Census D6 / R-7. Two toolpaths, same name, only the SECOND over
    // the band. Resolving the offender by name found the first match and
    // pointed the operator at the innocent toolpath.
    let tps = vec![
        make_tp(
            0,
            "Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        ),
        make_tp(
            1,
            "Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        ),
    ];
    let sums = vec![summary(0, 5.0), summary(1, 60.0)];
    let scan = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default());
    assert_eq!(scan.offenders.len(), 1);
    assert_eq!(
        scan.offenders[0].id,
        ToolpathId(1),
        "the offender must be the toolpath that actually breached, not the \
         first one sharing its name"
    );
}

#[test]
fn air_cut_gate_abstains_instead_of_warning_when_engagement_is_unmeasurable() {
    // Checkpoint D Q2. The census's shallow arm: a pass under the 0.05 mm
    // fresh-material floor reads ~96% air cut while removing material
    // perfectly well. RED-FIRST: with an empty (all-measurable) report
    // the gate fires, which is the shipped behaviour and the defect.
    use crate::stock::sim_measurability::{
        Measurability, MeasurabilityReason, MeasurabilityReport, MetricMeasurability, SimMetric,
    };

    let tps = vec![make_tp(
        0,
        "Spring Pass",
        OperationConfig::Pocket(PocketConfig::default()),
    )];
    let sums = vec![summary(0, 95.9)];

    let fired = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default());
    assert_eq!(
        fired.offenders.len(),
        1,
        "without a measurability report the gate compares the unmeasurable \
         number against the band and warns — this is the behaviour being fixed"
    );
    assert!(fired.abstentions.is_empty());

    // GREEN: told the metric is not a measurement, the gate declines.
    let reason = MeasurabilityReason::BelowFreshMaterialFloor {
        peak_removed_mm: 0.02,
        floor_mm: 0.05,
        blind_fraction: 0.98,
    };
    let report = MeasurabilityReport {
        entries: vec![MetricMeasurability {
            toolpath_id: ToolpathId(0),
            metric: SimMetric::AirCut,
            measurability: Measurability::NotMeasurable(reason),
        }],
        cell_mm: Some(0.25),
    };
    let scan = air_cut_offenders_for_toolpaths(&sums, &tps, &report);
    assert!(
        scan.offenders.is_empty(),
        "a NotMeasurable metric must stop feeding its gate; got {:?}",
        scan.offenders
    );
    assert_eq!(
        scan.abstentions.len(),
        1,
        "the abstention must be RECORDED, not swallowed — a silent decline \
         is indistinguishable from a pass"
    );
    assert_eq!(scan.abstentions[0].name, "Spring Pass");
}

#[test]
fn air_cut_gate_still_warns_when_the_metric_is_only_degraded() {
    // `Degraded` is not an abstention: the reading still describes the
    // measurable majority of the pass, and declining there would hide
    // more than it protects.
    use crate::stock::sim_measurability::{
        Measurability, MeasurabilityReason, MeasurabilityReport, MetricMeasurability, SimMetric,
    };

    let tps = vec![make_tp(
        0,
        "Mostly Measured",
        OperationConfig::Pocket(PocketConfig::default()),
    )];
    let sums = vec![summary(0, 95.9)];
    let report = MeasurabilityReport {
        entries: vec![MetricMeasurability {
            toolpath_id: ToolpathId(0),
            metric: SimMetric::AirCut,
            measurability: Measurability::Degraded(MeasurabilityReason::BelowFreshMaterialFloor {
                peak_removed_mm: 0.02,
                floor_mm: 0.05,
                blind_fraction: 0.2,
            }),
        }],
        cell_mm: Some(0.25),
    };
    let scan = air_cut_offenders_for_toolpaths(&sums, &tps, &report);
    assert_eq!(scan.offenders.len(), 1, "Degraded must NOT abstain");
    assert!(scan.abstentions.is_empty());
}

#[test]
fn air_cut_offenders_warns_on_adaptive3d_above_threshold() {
    // 60% air-cut on Adaptive3d is well above the 40% high-water mark.
    let tps = vec![make_tp(
        0,
        "Bad Rough",
        OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
    )];
    let sums = vec![summary(0, 60.0)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert_eq!(offenders.len(), 1, "Adaptive3d at 60% should warn");
    assert_eq!(offenders[0].name, "Bad Rough");
    assert!((offenders[0].air_cut_pct - 60.0).abs() < 1e-6);
}

#[test]
fn air_cut_offenders_warns_on_drop_cutter_above_threshold() {
    // 55% air-cut on DropCutter exceeds the 45% finish threshold.
    // W5B-F4: the fixture used to be 40.0 against a 30 band. 40 is now
    // INSIDE the measured defect-free finish cluster (34.3–42.5 on clean
    // geometry with the correct tool), so a test that called 40 "sloppy"
    // was pinning a false alarm. 55 is above the cluster and matches the
    // real offenders the band is for — the 3D golden's stacked waterline
    // (54.26) and wanaka tp9 pencil (55.83).
    let tps = vec![make_tp(
        0,
        "Sloppy Finish",
        OperationConfig::DropCutter(DropCutterConfig::default()),
    )];
    let sums = vec![summary(0, 55.0)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert_eq!(offenders.len(), 1, "DropCutter at 55% should warn");
}

#[test]
fn air_cut_offenders_warns_on_project_curve_near_total_air() {
    // 99% air-cut on ProjectCurve indicates a misconfigured TP — flag it.
    let tps = vec![make_tp(
        0,
        "Empty Rivers",
        OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
    )];
    let sums = vec![summary(0, 99.0)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert_eq!(offenders.len(), 1, "ProjectCurve at 99% should warn");
}

#[test]
fn air_cut_offenders_suppresses_drill_ops_entirely() {
    // Drill kinematics: air-cut metric is unusable. Never warn.
    let tps = vec![make_tp(
        0,
        "Pin Drill",
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
    )];
    let sums = vec![summary(0, 100.0)]; // dexel reports 100% always
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert!(
        offenders.is_empty(),
        "AlignmentPinDrill must never trigger air-cut warning (P4 suppression)"
    );
}

// ── P2: plunge-stress gate at session level ──────────────────

fn make_tapered_ball_tool(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
    t.diameter = diameter; // tip diameter
    t.shaft_diameter = (diameter + 2.0).max(3.0);
    t.taper_half_angle = 7.0;
    t
}

fn make_tp_with_plunge(
    id: usize,
    name: &str,
    op: OperationConfig,
    plunge_rate: f64,
) -> ToolpathConfig {
    let mut tc = make_tp(id, name, op);
    tc.operation.as_params_mut().set_plunge_rate(plunge_rate);
    tc
}

#[test]
fn plunge_stress_warns_on_wanaka_tp7_pattern() {
    // 1 mm tapered ball at 750 mm/min plunge — the TP7 finding.
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tapered_ball_tool(1.0));
    let tp = make_tp_with_plunge(
        0,
        "3D Finish 6",
        OperationConfig::DropCutter(DropCutterConfig::default()),
        750.0,
    );
    let _ = s.add_toolpath(0, tp).unwrap();
    let offenders = plunge_stress_offenders_for_session(&s);
    assert_eq!(offenders.len(), 1);
    assert_eq!(offenders[0].0, "3D Finish 6");
    assert!((offenders[0].1 - 750.0).abs() < 1e-6);
    assert!((offenders[0].2 - 150.0).abs() < 1e-6);
}

#[test]
fn plunge_stress_silent_for_flat_em_at_750() {
    // 6 mm flat end-mill at 750 mm/min — no cap applies.
    let mut s = ProjectSession::new_empty();
    let tp = make_tp_with_plunge(
        0,
        "Back Rough",
        OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        750.0,
    );
    let _ = s.add_toolpath(0, tp).unwrap();
    let offenders = plunge_stress_offenders_for_session(&s);
    assert!(
        offenders.is_empty(),
        "flat EM should be silent on plunge stress; got {offenders:?}"
    );
}

#[test]
fn plunge_stress_silent_when_at_or_below_cap() {
    // 1 mm tapered ball at 150 mm/min — exactly at cap.
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tapered_ball_tool(1.0));
    let tp = make_tp_with_plunge(
        0,
        "Engrave",
        OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
        150.0,
    );
    let _ = s.add_toolpath(0, tp).unwrap();
    let offenders = plunge_stress_offenders_for_session(&s);
    assert!(offenders.is_empty(), "150 mm/min on 1 mm TB is at cap");
}

#[test]
fn plunge_stress_ignores_disabled_toolpaths() {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tapered_ball_tool(1.0));
    let mut tp = make_tp_with_plunge(
        0,
        "Disabled",
        OperationConfig::DropCutter(DropCutterConfig::default()),
        750.0,
    );
    tp.enabled = false;
    let _ = s.add_toolpath(0, tp).unwrap();
    let offenders = plunge_stress_offenders_for_session(&s);
    assert!(offenders.is_empty(), "disabled TPs should be ignored");
}

#[test]
fn air_cut_offenders_isolates_bad_tp_in_mixed_project() {
    // Wanaka-like mix: ProjectCurve at its measured post-flip baseline
    // (noise) + Adaptive3d at 60% (signal). Only the Adaptive3d should be
    // flagged. W5B-F4: the ProjectCurve arm was 92.0 — the pre-swept
    // artifact reading; the same project's rivers now read 15.97.
    let tps = vec![
        make_tp(
            0,
            "Rivers",
            OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
        ),
        make_tp(
            1,
            "Bad Rough",
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        ),
    ];
    let sums = vec![summary(0, 15.97), summary(1, 60.0)];
    let offenders = air_cut_offenders_for_toolpaths(&sums, &tps, &Default::default()).offenders;
    assert_eq!(offenders.len(), 1);
    assert_eq!(offenders[0].name, "Bad Rough");
}

// ── strategy advisor: optimized-candidate modulation (step 5) ────

/// Load `ux_3d_terrain.toml` and add an AS013-shape adaptive3d op with the
/// given clearing strategy — mirrors the `strategy_advisor_smoke` fixture
/// so the advisor's per-candidate optimization can be exercised in-crate
/// (the private `optimized_candidate` is not reachable from the integration
/// test).
fn terrain_adaptive3d_session(strategy: ClearingStrategy) -> ProjectSession {
    use crate::compute::operation_configs::{
        Adaptive3dConfig, Adaptive3dEntryStyle, RegionOrdering,
    };
    let toml_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test_data/ux_3d_terrain.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");
    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_3d_terrain.toml defines a 6 mm end mill");
    let model_id = session
        .models()
        .iter()
        .find(|m| m.mesh.is_some())
        .map(|m| m.id)
        .expect("ux_3d_terrain.toml loads terrain_small.stl");
    let adaptive3d = Adaptive3dConfig {
        trochoid_cap_mult: 1.6,
        engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
        stepover: 1.2,
        depth_per_pass: 3.0,
        stock_to_leave_axial: 0.5,
        feed_rate: 2500.0,
        plunge_rate: 500.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ramp_angle_deg: 3.0,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        fine_stepdown: 0.0,
        detect_flat_areas: false,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: strategy,
        z_blend: false,
        mill_shallow_areas: false,
        shallow_angle_deg: None,
        shallow_stepdown: None,
        spindle_rpm: Some(18_000),
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    };
    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "AS013 adaptive3d".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(adaptive3d),
        dressups: DressupConfig::for_op(crate::compute::catalog::OperationType::Adaptive3d),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::compute::config::StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _ = session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");
    session
}

fn cut_move_feed(m: &crate::toolpath::Move) -> Option<f64> {
    match m.move_type {
        crate::toolpath::MoveType::Linear { feed_rate }
        | crate::toolpath::MoveType::ArcCW { feed_rate, .. }
        | crate::toolpath::MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
        crate::toolpath::MoveType::Rapid => None,
    }
}

/// Step-5 sentry: the advisor times the *modulated* path, not the raw
/// Suggest-feed path. Proves `optimized_candidate` rewrites at least one
/// cut-move feed (so the wall-clock the advisor compares reflects F-039
/// optimization) and returns a modelled binding regime. If step 5 were
/// reverted to timing raw paths this test fails: feeds would be untouched.
#[test]
fn advisor_modulates_candidate_feeds_before_timing() {
    let session = terrain_adaptive3d_session(ClearingStrategy::ContourSpiral);
    let cancel = AtomicBool::new(false);
    let resolved = session
        .resolve_generation_inputs(0, &cancel)
        .expect("resolve generation inputs for the adaptive3d op");

    // Build the raw candidate path exactly as `recommend_clearing_strategy`
    // does (minus the Suggest load-limit — modulation rewrites whatever
    // feeds the planned path carries, so the commanded 2500 mm/min is a
    // fair starting point for the "did feeds change?" check).
    let findings = std::cell::RefCell::new(crate::compute::execute::GenerationFindings::default());
    let ctx = crate::compute::execute::ExecutionContext {
        mesh: resolved.mesh.as_deref(),
        index: resolved.spatial_index(),
        polygons: resolved.polygons.as_deref().map(|v| v.as_slice()),
        prev_tool_radius: resolved.prev_tool_radius,
        boundary: resolved.pre_boundary.as_ref(),
        ..crate::compute::execute::ExecutionContext::new(
            &findings,
            &resolved.tool_def,
            &resolved.tool,
            &resolved.heights,
            &resolved.cutting_levels,
            &resolved.emission_stock_bbox,
            &cancel,
        )
    };
    let annotated = crate::compute::execute::execute_operation_annotated(&ctx, &resolved.operation)
        .expect("plan the spiral candidate");
    let annotated_arc = Arc::new(annotated);
    let raw_feeds: Vec<Option<f64>> = annotated_arc
        .toolpath
        .moves
        .iter()
        .map(cut_move_feed)
        .collect();

    // WP14a moved the optimizer behind the captured `AdvisorContext`, so
    // the test reads the same context the job's step (ii) reads.
    let handle = session
        .capture_recommend_clearing_strategy(0, &cancel)
        .expect("capture the advisor job for the adaptive3d op");
    let (modulated, regime) = optimized_candidate(
        &handle.context,
        &annotated_arc,
        &resolved.tool,
        &resolved.operation,
        &cancel,
    )
    .expect("advisor optimizes the candidate (effective_kinematics is always Some)");

    // Geometry is untouched; only feeds change.
    assert_eq!(
        modulated.moves.len(),
        annotated_arc.toolpath.moves.len(),
        "modulation rewrites feeds, not geometry"
    );
    let changed = modulated
        .moves
        .iter()
        .zip(&raw_feeds)
        .filter(|(m, raw)| match (cut_move_feed(m), raw) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            _ => false,
        })
        .count();
    assert!(
        changed > 0,
        "ConstrainedMax modulation must rewrite at least one cut-move feed \
         before the advisor times the path (else it's timing the raw path)"
    );
    assert!(
        matches!(
            regime,
            crate::machine::strategy_advisor::LoadRegime::ToolLimited
                | crate::machine::strategy_advisor::LoadRegime::MachineLimited
                | crate::machine::strategy_advisor::LoadRegime::Unconstrained
        ),
        "regime must be a modelled binding value derived from the optimized path"
    );
}

// ── DerivedRestRegions boundary (P2.2) ───────────────────────

fn derived_boundary(source_id: usize) -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: crate::compute::config::BoundarySource::DerivedRestRegions {
            source_toolpath_id: ToolpathId(source_id),
        },
        ..BoundaryConfig::default()
    }
}

/// A minimal cached generation result whose annotated toolpath carries
/// (or lacks) `rest_regions`, for staleness-precondition tests.
fn fake_result_with_regions(
    regions: Option<Vec<crate::polygon::Polygon2>>,
) -> ToolpathComputeResult {
    let mut at =
        crate::trace::toolpath_spans::AnnotatedToolpath::new(crate::toolpath::Toolpath::new());
    at.rest_regions = regions.map(Arc::new);
    ToolpathComputeResult {
        op_data: crate::ops::drill_op::OpData::Toolpath(Arc::new(at)),
        stats: ToolpathStats {
            move_count: 0,
            cutting_distance: 0.0,
            rapid_distance: 0.0,
            // Not measured: this fake never ran a cascade, planned no
            // bands and emitted no centrelines.
            truncated_core_mm2: None,
            untouched_material_mm2: None,
            reached_uncut_estimate_mm2: None,
            dropped_band: None,
            tip_float: None,
            deprecated_dial: None,
            derived_stepovers: Vec::new(),
            clipped_band: None,
            ramp_reach_clamp: None,
            claims_reference: None,
            zero_removal: None,
            offset_library_failures: None,
            boundary_clip_dropped: None,
            // Nor a waterline ladder (R3 / R4).
            waterline_ladder: None,
            inert_claims_dial: None,
            // Nor did it run a rest-region extraction (F3).
            region_cap: None,
            // Nor an intra-region relink (Phase O).
            relink: None,
            // Nor a pencil link stage (G-LINKVISIBLE).
            pencil_link: None,
            retract_trips: None,
            // Nor a monotone-cell decomposition (C2).
            monotone_cells: None,
            // Nor did it consume any machined stock.
            stock_snapshot: None,
        },
        debug_trace: None,
        semantic_trace: None,
    }
}

fn expect_operation_failed(result: Result<&ToolpathComputeResult, SessionError>) -> String {
    match result {
        Err(SessionError::OperationFailed(msg)) => msg,
        Err(other) => panic!("expected OperationFailed, got {other:?}"),
        Ok(_) => panic!("expected OperationFailed, got Ok"),
    }
}

#[test]
fn derived_rest_regions_boundary_missing_source_errors() {
    let mut s = make_session();
    let mut tc = make_tc(s.tools()[0].id.0);
    tc.boundary = derived_boundary(999);
    let _ = s.add_toolpath(0, tc).unwrap();

    let cancel = AtomicBool::new(false);
    let msg = expect_operation_failed(s.generate_toolpath(0, &cancel));
    assert!(
        msg.contains("999"),
        "error should name the missing source id: {msg}"
    );
    assert!(
        msg.contains("no longer") || msg.contains("no toolpath with that id"),
        "error should say the referenced toolpath doesn't exist: {msg}"
    );
}

#[test]
fn derived_rest_regions_boundary_self_reference_errors() {
    let mut s = make_session();
    let mut tc = make_tc(s.tools()[0].id.0);
    // First add_toolpath assigns id 0, so referencing id 0 is a
    // self-reference.
    tc.boundary = derived_boundary(0);
    let _ = s.add_toolpath(0, tc).unwrap();

    let cancel = AtomicBool::new(false);
    let msg = expect_operation_failed(s.generate_toolpath(0, &cancel));
    assert!(
        msg.contains("itself") || msg.contains("own rest regions"),
        "error should reject the self-reference: {msg}"
    );
}

#[test]
fn derived_rest_regions_boundary_ungenerated_source_errors() {
    let mut s = make_session();
    let mut source_tc = make_tc(s.tools()[0].id.0);
    source_tc.name = "Pencil Rest".to_owned();
    let _ = s.add_toolpath(0, source_tc).unwrap(); // gets id 0

    let mut tc = make_tc(s.tools()[0].id.0);
    tc.boundary = derived_boundary(0);
    let _ = s.add_toolpath(0, tc).unwrap(); // gets id 1, index 1

    let cancel = AtomicBool::new(false);
    let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
    assert!(
        msg.contains("Pencil Rest"),
        "error should name the source toolpath: {msg}"
    );
    assert!(
        msg.contains("generate"),
        "error should tell the user to generate the source first: {msg}"
    );
}

#[test]
fn derived_rest_regions_boundary_source_without_regions_errors() {
    let mut s = make_session();
    let mut source_tc = make_tc(s.tools()[0].id.0);
    source_tc.name = "Pencil Rest".to_owned();
    let _ = s.add_toolpath(0, source_tc).unwrap(); // id 0, index 0

    let mut tc = make_tc(s.tools()[0].id.0);
    tc.boundary = derived_boundary(0);
    let _ = s.add_toolpath(0, tc).unwrap(); // id 1, index 1

    // Source has a cached result, but its rest_regions is None (e.g. a
    // pencil op without the rest-depth detector, or any other op kind).
    s.results.insert(0, fake_result_with_regions(None));

    let cancel = AtomicBool::new(false);
    let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
    assert!(
        msg.contains("Pencil Rest"),
        "error should name the source toolpath: {msg}"
    );
    assert!(
        msg.contains("no rest regions"),
        "error should explain the source produced no regions: {msg}"
    );

    // Empty (rather than absent) regions fail the same way.
    s.results
        .insert(0, fake_result_with_regions(Some(Vec::new())));
    let msg = expect_operation_failed(s.generate_toolpath(1, &cancel));
    assert!(msg.contains("no rest regions"), "empty regions: {msg}");
}

#[test]
fn derived_rest_regions_resolve_happy_path_returns_regions() {
    let mut s = make_session();
    let mut source_tc = make_tc(s.tools()[0].id.0);
    source_tc.name = "Pencil Rest".to_owned();
    let _ = s.add_toolpath(0, source_tc).unwrap(); // id 0, index 0

    let mut tc = make_tc(s.tools()[0].id.0);
    tc.boundary = derived_boundary(0);
    let _ = s.add_toolpath(0, tc).unwrap(); // id 1, index 1

    let regions = vec![
        crate::polygon::Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        crate::polygon::Polygon2::rectangle(30.0, 30.0, 40.0, 40.0),
    ];
    s.results.insert(0, fake_result_with_regions(Some(regions)));

    let resolved = s
        .resolve_derived_rest_region_polys(1, ToolpathId(0))
        .expect("regions present on the source result");
    assert_eq!(resolved.len(), 2, "both disjoint regions come through");
}

#[test]
fn apply_boundary_clip_multi_clips_to_disjoint_regions() {
    use crate::trace::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

    // Two disjoint regions; a 3-move path visiting region A, the gap,
    // then region B. The gap move must become a rapid at safe_z, the two
    // region moves must survive, and spans must stay valid.
    let regions = vec![
        crate::polygon::Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        crate::polygon::Polygon2::rectangle(30.0, 30.0, 40.0, 40.0),
    ];

    let mut tp = crate::toolpath::Toolpath::new();
    tp.feed_to(P3::new(5.0, 5.0, -1.0), 1000.0); // region A
    tp.feed_to(P3::new(20.0, 20.0, -1.0), 1000.0); // gap
    tp.feed_to(P3::new(35.0, 35.0, -1.0), 1000.0); // region B
    let n_moves = tp.moves.len();
    let annotated =
        AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n_moves, SpanKind::Operation)]);

    let boundary = derived_boundary(0);
    let safe_z = 20.0;
    let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
    let semantic_ctx = recorder.root_context();

    let clipped = ProjectSession::apply_boundary_clip_multi(
        annotated,
        &boundary,
        &regions,
        &[],
        2.0,
        safe_z,
        // No operation here — the re-entry keeps the crossing move's
        // cut feed, which is what this test has always pinned.
        None,
        &semantic_ctx,
        &mut crate::trace::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        &mut crate::compute::execute::GenerationFindings::default(),
    )
    .expect("a boundary that resolves cannot refuse");

    assert!(clipped.spans_valid, "spans stay valid through the set clip");
    assert_eq!(clipped.spans.len(), 1);
    assert_eq!(
        clipped.spans[0].end_move,
        clipped.toolpath.moves.len(),
        "operation span covers the whole clipped path"
    );

    // Gap move became a rapid at safe_z.
    let gap = clipped
        .toolpath
        .moves
        .iter()
        .find(|m| (m.target.x - 20.0).abs() < 1e-10)
        .expect("gap move present");
    assert_eq!(gap.move_type, crate::toolpath::MoveType::Rapid);
    assert!((gap.target.z - safe_z).abs() < 1e-10);

    // Both region moves survive as cuts.
    for (x, y) in [(5.0, 5.0), (35.0, 35.0)] {
        assert!(
            clipped.toolpath.moves.iter().any(|m| {
                m.move_type != crate::toolpath::MoveType::Rapid
                    && (m.target.x - x).abs() < 1e-10
                    && (m.target.y - y).abs() < 1e-10
            }),
            "cut at ({x}, {y}) should survive the set clip"
        );
    }
}

#[test]
fn apply_boundary_clip_multi_all_regions_collapsed_returns_original() {
    use crate::trace::toolpath_spans::AnnotatedToolpath;

    // A tiny region with a large negative user offset collapses; with
    // every region gone the toolpath must pass through unchanged (the
    // single-polygon path's "boundary collapsed" semantics).
    let regions = vec![crate::polygon::Polygon2::rectangle(0.0, 0.0, 2.0, 2.0)];

    let mut tp = crate::toolpath::Toolpath::new();
    tp.feed_to(P3::new(50.0, 50.0, -1.0), 1000.0);
    tp.feed_to(P3::new(60.0, 50.0, -1.0), 1000.0);
    let move_count = tp.moves.len();
    let annotated = AnnotatedToolpath::new(tp);

    let mut boundary = derived_boundary(0);
    boundary.offset = -10.0; // shrink by 10mm — eats the 2mm square

    let recorder = ToolpathSemanticRecorder::new("test-tp", "Pocket");
    let semantic_ctx = recorder.root_context();
    let mut findings = crate::compute::execute::GenerationFindings::default();

    let clipped = ProjectSession::apply_boundary_clip_multi(
        annotated,
        &boundary,
        &regions,
        &[],
        2.0,
        20.0,
        // No operation here — the re-entry keeps the crossing move's
        // cut feed, which is what this test has always pinned.
        None,
        &semantic_ctx,
        &mut crate::trace::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        &mut findings,
    )
    .expect(
        "a GENUINE collapse still passes through — Checkpoint C only \
             refuses when the offset FAILED",
    );

    assert_eq!(
        clipped.toolpath.moves.len(),
        move_count,
        "collapsed boundary set must leave the toolpath unchanged"
    );
    assert!(
        clipped
            .toolpath
            .moves
            .iter()
            .all(|m| m.move_type != crate::toolpath::MoveType::Rapid),
        "no retracts inserted when the boundary collapses"
    );
    // Checkpoint C, Q2: the pass-through is kept, and it is no longer
    // silent. Before this the operator got an unclipped path and a
    // `tracing::warn!` in a process with no subscriber.
    let dropped = findings
        .boundary_clip_dropped
        .expect("a dropped containment must be recorded as a finding");
    assert_eq!(
        dropped.containment,
        crate::compute::config::BoundaryContainment::default(),
        "the finding names the containment that was requested"
    );
    assert_eq!(
        dropped.source_region_count, 1,
        "the finding names how many source regions all collapsed"
    );
}

// ── STK-06: the group-stock cut-direction rule ───────────────────

/// CONTRACT — STK-06. The rule answers for all six faces, from one home.
///
/// A group stock lives in the setup-local frame, where local −Z is the
/// tool axis on every face. Only `FaceUp::Bottom` runs its local Z against
/// the global Z the dexel columns index on.
#[test]
fn the_group_stock_rule_answers_for_every_face() {
    use crate::compute::simulate::group_stock_cut_direction;
    use crate::compute::transform::FaceUp;
    use crate::dexel_stock::StockCutDirection;

    let table = [
        (FaceUp::Top, StockCutDirection::FromTop),
        (FaceUp::Bottom, StockCutDirection::FromBottom),
        (FaceUp::Front, StockCutDirection::FromTop),
        (FaceUp::Back, StockCutDirection::FromTop),
        (FaceUp::Left, StockCutDirection::FromTop),
        (FaceUp::Right, StockCutDirection::FromTop),
    ];
    let mut checked = 0;
    for (face_up, expected) in table {
        assert_eq!(
            group_stock_cut_direction(face_up),
            expected,
            "{face_up:?}: the group stock is stamped in the setup-local \
             frame. The S5 prefix hash reads this value, so a change here \
             is a cache break, not a rename."
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        FaceUp::ALL.len(),
        "every FaceUp variant is checked"
    );
}

/// CONTRACT — STK-06. The group rule and `cut_direction()` answer two
/// DIFFERENT questions, and their divergence is the design.
///
/// `SetupTransformInfo::cut_direction()` names the side the tool arrives
/// from in the stock-relative GLOBAL frame; it feeds the global playback
/// stock. The group rule names the side it arrives from in the
/// SETUP-LOCAL frame; it feeds the per-setup stock every metric, gate and
/// collision check reads.
///
/// This test goes red the day someone "repairs" the rule into a call to
/// the accessor. `compute/transform.rs:493` records two defects that came
/// from reading one of these questions as the other.
#[test]
fn the_group_rule_and_the_global_accessor_diverge_on_the_laterals() {
    use crate::compute::simulate::group_stock_cut_direction;
    use crate::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};

    let info = |face_up| SetupTransformInfo {
        face_up,
        z_rotation: ZRotation::Deg0,
        stock_x: 40.0,
        stock_y: 30.0,
        stock_z: 25.0,
        ..Default::default()
    };

    for face_up in [FaceUp::Top, FaceUp::Bottom] {
        assert_eq!(
            group_stock_cut_direction(face_up),
            info(face_up).cut_direction(),
            "{face_up:?}: local Z IS global Z on the two Z faces, so the \
             two frames must agree"
        );
    }
    for face_up in [FaceUp::Front, FaceUp::Back, FaceUp::Left, FaceUp::Right] {
        assert_ne!(
            group_stock_cut_direction(face_up),
            info(face_up).cut_direction(),
            "{face_up:?}: the accessor answers in the global frame and \
             names a lateral variant. The group stock is local, so it \
             stays FromTop. Do not collapse the two."
        );
    }
}
