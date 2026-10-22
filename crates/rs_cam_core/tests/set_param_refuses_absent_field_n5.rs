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
//!    counts (12 and 16) are asserted, so the test cannot pass
//!    vacuously.
//! 2. A refusal has no side effect: the cached result survives and the
//!    provenance record does not move.
//! 3. No setter alias remains (C1, D2). `stepover` on Pencil and
//!    `depth_per_pass` on RampFinish refuse, and the field behind the
//!    feeds accessor (`offset_stepover`, `max_stepdown`) does not move.
//!    Each field takes its published name. Assertion 1's two populations
//!    read the registry alone.
//! 4. Positive control: Pocket accepts `stepover` and stamps `Manual`.
//! 5. The five numeric named arms and the generic arm call one shared
//!    range helper, read from the source text.
//! 6. Every name the setter accepts is published (CMP-08).
//!
//! ## Why assertion 5 asserts wiring and not a refusal
//!
//! Since F185 (2026-10-20) every published number declares a domain, the
//! named params included. This test still asserts wiring only: the
//! refusal of each domain is covered by
//! `a_zero_feed_is_refused_on_every_op_f185.rs` (the four feed fields on
//! every op) and by
//! `set_toolpath_param_refuses_a_peck_depth_the_emitter_would_refuse`
//! in `session::compute`'s unit tests, and by
//! `radial_finish_ranges_n10.rs`; both drive it through the generic
//! arm. This test asserts only that the five arms call the same helper,
//! so the two routes cannot drift apart.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session, square_polygon, stock_under, toolpath_config,
};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::TraceConfig;
use rs_cam_core::feeds::{FeedsField, ProvenanceSource};
use rs_cam_core::session::{Command, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs};
use serde_json::json;

/// The source of `set_toolpath_param`, read for assertion 5.
///
/// P4 moved `set_toolpath_param_impl` out of `session/compute.rs` into the
/// `params` child. The needle is the method, so this path follows it.
const SESSION_COMPUTE_SRC: &str = include_str!("../src/session/compute/params.rs");

/// Operations that publish no `stepover`. D2 deleted the Pencil alias of
/// `offset_stepover`, so Pencil joined this set (12). P6.6 added Bore,
/// which publishes neither (13).
const EXPECTED_STEPOVER_REJECTS: usize = 13;

/// Operations that publish no `depth_per_pass`. C1 deleted the Waterline
/// alias of `z_step` and D2 the RampFinish alias of `max_stepdown`, so
/// both joined this set (16). P6.6 added Bore: its helix pitch is its
/// step (17).
const EXPECTED_DEPTH_PER_PASS_REJECTS: usize = 17;

// ── the two populations, from the registry ───────────────────────

/// `stepover` reaches a field on this operation.
///
/// The registry's `param_defs` is the record for EVERY operation, so
/// this predicate derives. Pencil's `offset_stepover` has no alias (D2).
fn accepts_stepover(op: OperationType) -> bool {
    OperationConfig::param_names_for_type(op).contains(&"stepover")
}

/// `depth_per_pass` reaches a field on this operation.
///
/// Waterline `z_step` (C1) and RampFinish `max_stepdown` (D2) have no
/// alias, so both refuse this name.
fn accepts_depth_per_pass(op: OperationType) -> bool {
    OperationConfig::param_names_for_type(op).contains(&"depth_per_pass")
}

// ── fixtures ─────────────────────────────────────────────────────

/// One toolpath per operation type, in `OperationType::ALL` order, so a
/// toolpath index IS an index into that list.
///
/// `add_toolpath` validates nothing, so an operation whose tool
/// constraint an end mill fails still gets a record here. That is what
/// this test wants: the param route is under test, not the generator.
fn all_ops_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(20.0, 4.0));
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model = polygon_model(vec![square_polygon(2.0)], "n5_all_ops");
    let model_id = builder.add_model(model);
    for op in OperationType::ALL {
        let cfg = toolpath_config(
            op.label(),
            OperationConfig::new_default(*op),
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id,
        );
        let _ = builder.add_toolpath(0, cfg).expect("add one op per type");
    }
    builder.build()
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
            let outcome = session.apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: field.to_owned(),
                value: json!(0.7),
            }));
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
        session.result_at(0).is_some(),
        "fixture: the Trace must generate a cached result"
    );

    let before = session
        .toolpath_config_at(0)
        .expect("trace config exists")
        .feeds_provenance
        .clone();
    assert!(
        before.get(FeedsField::Stepover).is_none(),
        "fixture: a fresh Trace carries no stepover stamp"
    );

    let outcome = session.apply(Command::SetToolpathParam(SetToolpathParamArgs {
        index: 0,
        param: "stepover".to_owned(),
        value: json!(1.1),
    }));
    let Err(err) = outcome else {
        panic!("Trace has no `stepover` field; the row must refuse");
    };
    assert!(
        err.to_string().contains("unknown parameter 'stepover'"),
        "the refusal must name the parameter: {err}"
    );

    assert!(
        session.result_at(0).is_some(),
        "a refused write must not invalidate the cached result"
    );
    let after = &session
        .toolpath_config_at(0)
        .expect("trace config still exists")
        .feeds_provenance;
    assert_eq!(
        after, &before,
        "a refused write must not stamp provenance on a field the config lacks"
    );
}

// ── 3. no setter alias ───────────────────────────────────────────

fn pencil_offset_stepover(op: &OperationConfig) -> f64 {
    let OperationConfig::Pencil(cfg) = op else {
        panic!("expected Pencil");
    };
    cfg.offset_stepover
}

fn ramp_finish_max_stepdown(op: &OperationConfig) -> f64 {
    let OperationConfig::RampFinish(cfg) = op else {
        panic!("expected RampFinish");
    };
    cfg.max_stepdown
}

/// Write the retired `alias` on a default `op`: the setter refuses and
/// the field does not move. Then the published `name` writes the field.
#[track_caller]
fn alias_refuses_and_name_writes(
    op: OperationType,
    alias: &str,
    name: &str,
    value: f64,
    read: fn(&OperationConfig) -> f64,
) {
    let before = read(&OperationConfig::new_default(op));
    assert!(
        (before - value).abs() > 1e-12,
        "fixture: {op:?} must start away from the written value"
    );

    let mut session = single_op("N5 alias", OperationConfig::new_default(op));
    let write = |session: &mut ProjectSession, param: &str| {
        session.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: param.to_owned(),
            value: json!(value),
        }))
    };
    let Err(err) = write(&mut session, alias) else {
        panic!("{op:?} accepted the retired alias `{alias}`");
    };
    assert!(
        err.to_string()
            .contains(&format!("unknown parameter '{alias}'")),
        "{op:?}: the refusal must reuse the generic arm's wording: {err}"
    );
    let tc = session.toolpath_config_at(0).expect("config exists");
    assert!(
        (read(&tc.operation) - before).abs() < 1e-12,
        "{op:?}: the refused `{alias}` moved the field"
    );

    if let Err(e) = write(&mut session, name) {
        panic!("{op:?} must accept its published `{name}`: {e}");
    }
    let tc = session.toolpath_config_at(0).expect("config exists");
    let after = read(&tc.operation);
    assert!(
        (after - value).abs() < 1e-12,
        "{op:?}: `{name}` must reach its field; got {after}"
    );
}

/// Assertion 6 (CMP-08). Every name this setter accepts appears in the
/// list its own refusal message prints.
///
/// The defect: `set_toolpath_param(…, "depth_per_pass", …)` succeeded on
/// Waterline while `get_operation_schema("waterline")` listed `z_step`
/// and not `depth_per_pass`, and the refusal for a genuinely unknown name
/// printed that same wrong list. Wrong in both directions, on three
/// operations. `debug_enabled` was a fourth accepted name in no schema at
/// all; it is published under `toolpath_params`, because it writes
/// toolpath state and not the operation.
///
/// Driven off `OperationType::ALL` and the five generic names, so it
/// cannot go stale against a new operation.
#[test]
fn every_name_the_setter_accepts_is_published() {
    const GENERIC: &[&str] = &[
        "feed_rate",
        "plunge_rate",
        "stepover",
        "depth_per_pass",
        "spindle_rpm",
    ];

    let mut accepted = 0;
    let mut gaps: Vec<String> = Vec::new();
    for (index, op) in OperationType::ALL.iter().enumerate() {
        let published = OperationConfig::param_names_for_type(*op);
        for name in GENERIC {
            // A fresh session per case: an accepted write mutates, and a
            // later case must not read the earlier one's state.
            let mut session = all_ops_session();
            let outcome = session.apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: (*name).to_owned(),
                value: json!(0.5),
            }));
            if outcome.is_err() {
                continue;
            }
            accepted += 1;
            if !published.contains(name) {
                gaps.push(format!("{}:{name}", op.name()));
                continue;
            }
            assert!(
                published.contains(name),
                "{op:?} accepts `{name}` and the registry does not publish it.                  `get_operation_schema` omits it, and the refusal message for an                  unknown name prints a valid set that is missing a valid name."
            );
        }
    }
    assert!(
        accepted > 24,
        "only {accepted} accepted writes over 24 operations — the sweep is near-vacuous"
    );

    // The residue, named rather than hidden. Both are a DIFFERENT defect
    // from CMP-08: the two drill families carry no `plunge_rate` field at
    // all, `OperationParams::set_plunge_rate` returns `()` and its default
    // body discards the value, so the arm reports success, stamps manual
    // provenance and stales the result chain — this file's assertion 2,
    // for a third name. `set_stepover` and `set_depth_per_pass` return
    // `bool` and are refused; `set_feed_rate` and `set_plunge_rate` do not
    // and cannot be. Closing it means changing the trait, which is one
    // row further than CMP-08 reaches.
    const ACCEPTED_AND_UNPUBLISHED: &[&str] =
        &["Drill:plunge_rate", "AlignmentPinDrill:plunge_rate"];
    assert_eq!(
        gaps, ACCEPTED_AND_UNPUBLISHED,
        "the accepted-but-unpublished population moved. A new entry is a name the setter \
         takes and the schema omits — publish it, or refuse it."
    );

    // `debug_enabled` is accepted on every operation and is not an
    // operation parameter. It must be published somewhere, and the
    // somewhere must not be `params`.
    let mut session = all_ops_session();
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "debug_enabled".to_owned(),
            value: json!(true),
        }))
        .expect("`debug_enabled` is accepted on every operation");
    let schema = OperationConfig::schema_for_type(OperationType::ALL[0]);
    assert!(
        schema
            .toolpath_params
            .iter()
            .any(|p| p.name == "debug_enabled"),
        "`debug_enabled` is accepted and published nowhere"
    );
    assert!(
        !schema.params.iter().any(|p| p.name == "debug_enabled"),
        "`debug_enabled` writes toolpath state, not the operation config; it must not          appear among the operation's own parameters"
    );
}

/// Assertion 3. C1 deleted the Waterline alias (`depth_per_pass` onto
/// `z_step`); D2 deleted the last two. The feeds accessor still reaches
/// these fields, but the setter accepts only the published name.
#[test]
fn a_retired_alias_refuses_and_the_published_name_writes() {
    alias_refuses_and_name_writes(
        OperationType::Pencil,
        "stepover",
        "offset_stepover",
        0.37,
        pencil_offset_stepover,
    );
    alias_refuses_and_name_writes(
        OperationType::RampFinish,
        "depth_per_pass",
        "max_stepdown",
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
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "stepover".to_owned(),
            value: json!(2.75),
        }))
        .expect("Pocket carries a stepover field");

    let tc = session.toolpath_config_at(0).expect("pocket config exists");
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
    // marker appears once in `session/compute/params.rs`, in this order.
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
            .unwrap_or_else(|| panic!("arm marker {start} is not in session/compute/params.rs"));
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
