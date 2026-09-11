//! N5 — `set_toolpath_param` refuses a `stepover` or `depth_per_pass` the
//! operation has no field for.
//!
//! ## The defect this closes
//!
//! `ProjectSession::set_toolpath_param` handles six parameter names in
//! their own match arms and routes every other name through a serde
//! round-trip. The generic arm VERIFIES that the operation consumed the
//! key, and refuses an unknown name with the list of valid ones. The
//! named arms verify nothing.
//!
//! Two of those names are optional on the `OperationParams` trait:
//! `set_stepover` and `set_depth_per_pass` carry empty default bodies.
//! An operation whose config has no such field inherits a setter that
//! discards the value. Eleven operations do not override `set_stepover`
//! and fourteen do not override `set_depth_per_pass`.
//!
//! Pre-fix the arm then did two more things after the discard:
//!
//! - it stamped `feeds_provenance` `Manual` on a field the config lacks;
//! - it ran `invalidate_result_chain`, which drops the cached result of
//!   this toolpath and of every operation downstream of the stock it
//!   leaves.
//!
//! The caller read `Ok(())`. No geometry moved. The project went stale,
//! and a false provenance stamp stayed on the record.
//!
//! ## What this sentry pins
//!
//! 1. Every operation whose config declares the field accepts the write.
//!    Every operation that does not gets the generic arm's own
//!    `unknown parameter '<name>'` refusal. The two populations come
//!    from the registry, never from a literal list, and the reject
//!    counts (11 and 14) are asserted, so the test cannot pass
//!    vacuously.
//! 2. A refusal has no side effect: the cached result survives and the
//!    provenance record does not move.
//! 3. The three ALIAS setters keep working. `stepover` on Pencil writes
//!    `offset_stepover`, `depth_per_pass` on Waterline writes `z_step`,
//!    and `depth_per_pass` on RampFinish writes `max_stepdown`. The
//!    registry publishes none of those three names, so the named arm is
//!    their only route. A fix that deletes the arms breaks them.
//! 4. Positive control: Pocket accepts `stepover` and stamps `Manual`.
//! 5. The five numeric named arms and the generic arm call one shared
//!    range helper, read from the source text.
//!
//! ## Why assertion 5 asserts wiring and not a refusal
//!
//! The registry declares a `ParamRange` on six param defs only
//! (`peck_depth` on the two drill families, `chain_distance_mm` on
//! ProjectCurve, and — since N10, 2026-09-11 — `angular_step` and
//! `point_spacing` on RadialFinish). No named param declares a range on
//! any operation, so the population an out-of-range assertion draws from
//! is EMPTY, and a gate handed an empty population passes and looks
//! healthy. The helper's own refusal behaviour is covered by
//! `set_toolpath_param_refuses_a_peck_depth_the_emitter_would_refuse`
//! in `session::compute`'s unit tests, and by
//! `radial_finish_ranges_n10.rs`; both drive it through the generic
//! arm. This test asserts only that the five arms call the same helper,
//! so the two routes cannot drift apart.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session, square_polygon, stock_under, toolpath_config,
};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::feeds::{FeedsField, ProvenanceSource};
use rs_cam_core::session::ProjectSession;
use serde_json::json;

/// The source of `set_toolpath_param`, read for assertion 5.
const SESSION_COMPUTE_SRC: &str = include_str!("../src/session/compute.rs");

/// Operations whose config has no `stepover` field.
const EXPECTED_STEPOVER_REJECTS: usize = 11;

/// Operations whose config has no `depth_per_pass` field.
const EXPECTED_DEPTH_PER_PASS_REJECTS: usize = 14;

// ── the two populations, from the registry ───────────────────────

/// `stepover` reaches a field on this operation.
///
/// The registry's `param_defs` is the record for every operation except
/// Pencil, whose named arm writes `offset_stepover` — a field the
/// registry does not publish under this name.
fn accepts_stepover(op: OperationType) -> bool {
    OperationConfig::param_names_for_type(op).contains(&"stepover") || op == OperationType::Pencil
}

/// `depth_per_pass` reaches a field on this operation.
///
/// Waterline writes `z_step` and RampFinish writes `max_stepdown`. Both
/// are alias-only, like Pencil above.
fn accepts_depth_per_pass(op: OperationType) -> bool {
    OperationConfig::param_names_for_type(op).contains(&"depth_per_pass")
        || matches!(op, OperationType::Waterline | OperationType::RampFinish)
}

// ── fixtures ─────────────────────────────────────────────────────

/// One toolpath per operation type, in `OperationType::ALL` order, so a
/// toolpath index IS an index into that list.
///
/// `add_toolpath` validates nothing, so an operation whose tool
/// constraint an end mill fails still gets a record here. That is what
/// this test wants: the param route is under test, not the generator.
fn all_ops_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(stock_under(20.0, 4.0));
    let tool_idx = session
        .add_tool(make_endmill_6mm())
        .created
        .expect("add_tool reports the new tool index");
    let tool_id = session.tools()[tool_idx].id.0;
    let model = polygon_model(vec![square_polygon(2.0)], "n5_all_ops");
    let model_id = session
        .add_model(model)
        .created
        .expect("add_model reports the new model id");
    for op in OperationType::ALL {
        let cfg = toolpath_config(
            op.label(),
            OperationConfig::new_default(*op),
            tool_id,
            model_id,
        );
        let _ = session.add_toolpath(0, cfg).expect("add one op per type");
    }
    session
}

/// A Trace that generates real motion on the 2D fixture. Trace carries
/// a `depth_per_pass` field and carries NO `stepover` field, so one
/// operation serves as both the generate-able fixture and a reject case.
fn trace_op() -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: 1.0,
        depth_per_pass: 1.0,
        ..TraceConfig::default()
    })
}

fn single_op(name: &str, op: OperationConfig) -> ProjectSession {
    single_op_session(
        stock_under(20.0, 4.0),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(2.0)], "n5_single"),
        name,
        op,
    )
}

// ── 1. the two populations, both directions ──────────────────────

/// Assertion 1. For every operation and both fields: a config that
/// carries the field accepts the write, and a config that does not gets
/// the generic arm's refusal, naming the parameter and the operation.
///
/// Both directions matter. A fix that refuses EVERY write satisfies the
/// reject counts on its own.
#[test]
fn set_toolpath_param_refuses_a_field_the_operation_does_not_carry() {
    let mut session = all_ops_session();

    let mut stepover_rejects: Vec<OperationType> = Vec::new();
    let mut depth_per_pass_rejects: Vec<OperationType> = Vec::new();

    for (index, op) in OperationType::ALL.iter().enumerate() {
        let cases = [
            ("stepover", accepts_stepover(*op)),
            ("depth_per_pass", accepts_depth_per_pass(*op)),
        ];
        for (field, accepted) in cases {
            let outcome = session.set_toolpath_param(index, field, json!(0.7));
            if accepted {
                if let Err(e) = outcome {
                    panic!("{op:?} carries `{field}`; the setter must write it: {e}");
                }
                continue;
            }
            let Err(err) = outcome else {
                panic!(
                    "{op:?} has no `{field}` field. The setter reported success, \
                     discarded the value, stamped manual provenance and staled \
                     the result chain."
                );
            };
            let text = err.to_string();
            assert!(
                text.contains(&format!("unknown parameter '{field}'")),
                "the refusal must reuse the generic arm's wording for {op:?}: {text}"
            );
            assert!(
                text.contains(op.label()),
                "the refusal must name the operation for {op:?}: {text}"
            );
            if field == "stepover" {
                stepover_rejects.push(*op);
            } else {
                depth_per_pass_rejects.push(*op);
            }
        }
    }

    assert_eq!(
        stepover_rejects.len(),
        EXPECTED_STEPOVER_REJECTS,
        "the `stepover` reject population moved; it is now {stepover_rejects:?}"
    );
    assert_eq!(
        depth_per_pass_rejects.len(),
        EXPECTED_DEPTH_PER_PASS_REJECTS,
        "the `depth_per_pass` reject population moved; it is now {depth_per_pass_rejects:?}"
    );
}

// ── 2. a refusal has no side effect ──────────────────────────────

/// Assertion 2. The pre-fix arm invalidated the result chain and
/// stamped provenance on every call, refusal or not. A refusal must
/// reach neither.
///
/// Trace is the fixture: it generates cheap real motion on the 2D
/// square, and it carries no `stepover` field, so one session covers
/// both halves. The `depth_per_pass` arm has the same statement shape,
/// so one field is enough to pin the ordering.
#[test]
fn a_refused_write_keeps_the_cached_result_and_the_provenance() {
    let mut session = single_op("N5 trace", trace_op());
    generate(&mut session, 0);
    assert!(
        session.get_result(0).is_some(),
        "fixture: the Trace must generate a cached result"
    );

    let before = session
        .get_toolpath_config(0)
        .expect("trace config exists")
        .feeds_provenance
        .clone();
    assert!(
        before.get(FeedsField::Stepover).is_none(),
        "fixture: a fresh Trace carries no stepover stamp"
    );

    let Err(err) = session.set_toolpath_param(0, "stepover", json!(1.1)) else {
        panic!("Trace has no `stepover` field; the setter must refuse");
    };
    assert!(
        err.to_string().contains("unknown parameter 'stepover'"),
        "the refusal must name the parameter: {err}"
    );

    assert!(
        session.get_result(0).is_some(),
        "a refused write must not invalidate the cached result"
    );
    let after = &session
        .get_toolpath_config(0)
        .expect("trace config still exists")
        .feeds_provenance;
    assert_eq!(
        after, &before,
        "a refused write must not stamp provenance on a field the config lacks"
    );
}

// ── 3. the three alias setters ───────────────────────────────────

fn pencil_offset_stepover(op: &OperationConfig) -> f64 {
    let OperationConfig::Pencil(cfg) = op else {
        panic!("expected Pencil");
    };
    cfg.offset_stepover
}

fn waterline_z_step(op: &OperationConfig) -> f64 {
    let OperationConfig::Waterline(cfg) = op else {
        panic!("expected Waterline");
    };
    cfg.z_step
}

fn ramp_finish_max_stepdown(op: &OperationConfig) -> f64 {
    let OperationConfig::RampFinish(cfg) = op else {
        panic!("expected RampFinish");
    };
    cfg.max_stepdown
}

/// Write `param` on a default `op` and read the aliased field back.
#[track_caller]
fn alias_writes_through(
    op: OperationType,
    param: &str,
    value: f64,
    read: fn(&OperationConfig) -> f64,
) {
    let before = read(&OperationConfig::new_default(op));
    assert!(
        (before - value).abs() > 1e-12,
        "fixture: {op:?} must start away from the written value"
    );

    let mut session = single_op("N5 alias", OperationConfig::new_default(op));
    if let Err(e) = session.set_toolpath_param(0, param, json!(value)) {
        panic!("{op:?} aliases `{param}` onto a real field: {e}");
    }

    let tc = session.get_toolpath_config(0).expect("alias config exists");
    let after = read(&tc.operation);
    assert!(
        (after - value).abs() < 1e-12,
        "{op:?}: `{param}` must reach the aliased field; got {after}"
    );
}

/// Assertion 3. The named arms are the only route to these three
/// fields, because the registry publishes none of the three names.
#[test]
fn the_alias_setters_still_write_through_the_named_arms() {
    alias_writes_through(
        OperationType::Pencil,
        "stepover",
        0.37,
        pencil_offset_stepover,
    );
    alias_writes_through(
        OperationType::Waterline,
        "depth_per_pass",
        0.23,
        waterline_z_step,
    );
    alias_writes_through(
        OperationType::RampFinish,
        "depth_per_pass",
        0.41,
        ramp_finish_max_stepdown,
    );
}

// ── 4. positive control ──────────────────────────────────────────

/// Assertion 4. Pocket carries `stepover`. The write lands, and the
/// provenance says a person set it.
#[test]
fn pocket_still_takes_a_stepover_and_stamps_manual_provenance() {
    let pocket = OperationConfig::new_default(OperationType::Pocket);
    let mut session = single_op("N5 pocket", pocket);
    let _ = session
        .set_toolpath_param(0, "stepover", json!(2.75))
        .expect("Pocket carries a stepover field");

    let tc = session
        .get_toolpath_config(0)
        .expect("pocket config exists");
    let OperationConfig::Pocket(cfg) = &tc.operation else {
        panic!("expected Pocket");
    };
    assert!(
        (cfg.stepover - 2.75).abs() < 1e-12,
        "the write must land; got {}",
        cfg.stepover
    );
    let stamp = tc
        .feeds_provenance
        .get(FeedsField::Stepover)
        .expect("an accepted write stamps provenance");
    assert_eq!(stamp.source, ProvenanceSource::Manual);
}

// ── 5. one range gate, five arms ─────────────────────────────────

/// Assertion 5. Every numeric named arm and the generic arm call the
/// SAME range helper, so the DR-LIVE gate cannot be wired to one route
/// and not the other. Source text, because no shipped operation
/// declares a range on a named param — see this file's module doc.
#[test]
fn every_numeric_named_arm_calls_the_shared_range_helper() {
    const HELPER: &str = "check_param_range(";
    // Each named arm, and the marker that starts the next one. Every
    // marker appears once in `session/compute.rs`, in this order.
    let arms = [
        ("\"feed_rate\" =>", "\"plunge_rate\" =>"),
        ("\"plunge_rate\" =>", "\"stepover\" =>"),
        ("\"stepover\" =>", "\"depth_per_pass\" =>"),
        ("\"depth_per_pass\" =>", "\"spindle_rpm\" =>"),
        ("\"spindle_rpm\" =>", "\"debug_enabled\" =>"),
        // The generic arm runs from `debug_enabled` to the invalidation
        // comment that closes the match.
        ("\"debug_enabled\" =>", "// Invalidate cached result"),
    ];
    for (start, end) in arms {
        let from = SESSION_COMPUTE_SRC
            .find(start)
            .unwrap_or_else(|| panic!("arm marker {start} is not in session/compute.rs"));
        let len = SESSION_COMPUTE_SRC[from..]
            .find(end)
            .unwrap_or_else(|| panic!("arm end marker {end} is not after {start}"));
        let arm = &SESSION_COMPUTE_SRC[from..from + len];
        assert!(
            arm.contains(HELPER),
            "the {start} arm must call `{HELPER}` so one helper serves every route"
        );
    }
}
