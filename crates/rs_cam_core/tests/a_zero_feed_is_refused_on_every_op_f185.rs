//! F185: a published number states its domain, and the setter refuses a
//! feed, a plunge feed, a ramp feed or a spindle speed of zero or below.
//!
//! ## The defect this closes
//!
//! `check_param_range` returns `Ok` when a param has no range. Before F185,
//! 107 of the published numbers had none: `feed_rate`, `plunge_rate`,
//! `ramp_feed_rate` and `spindle_rpm` on almost every op, and 17 dials. MCP
//! and the CLI accepted `feed_rate = 0` and a negative feed on every op.
//!
//! The fix is in the types: `#[derive(OpParams)]` does not compile a number
//! field with no bound, unless it says `frame_dependent` or
//! `unbounded = "<reason>"`. The `compile_fail` doctests in
//! `compute/catalog/schema.rs` (`op_params_refusals`) pin that refusal.
//!
//! ## What this sentry pins
//!
//! 1. Every published number of every op carries a range or is
//!    `frame_dependent`. No field uses `unbounded` today, so the allow
//!    list is empty. A new `unbounded` field must join it with its reason.
//! 2. `ParamDef::kind` agrees with the wire text: a `List` publishes a
//!    `vec<` type and nothing else does (F023 keeps `TYPE_NAME`).
//! 3. Through the command door, zero and a negative value of each of the
//!    four feeds is refused on every op that publishes the name, and a
//!    positive value is accepted (the positive control).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, square_polygon, stock_under, toolpath_config};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType, ParamKind};
use rs_cam_core::session::{Command, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs};
use serde_json::json;

/// The published numbers that say `unbounded = "<reason>"`, as
/// `(op label, field)`. Empty: every number has a bound from math or
/// physics, or is an absolute Z.
const UNBOUNDED: &[(&str, &str)] = &[];

/// Assertion 1.
#[test]
fn every_published_number_states_its_domain() {
    let mut numbers = 0;
    let mut bounded = 0;
    let mut frame_dependent = 0;
    for op in OperationType::ALL {
        for def in op.registry_entry().params.defs {
            if !def.kind.is_number() {
                assert!(
                    def.range.is_none(),
                    "{op:?}.{}: a {:?} param takes no domain",
                    def.name,
                    def.kind
                );
                continue;
            }
            numbers += 1;
            if def.range.is_some() {
                bounded += 1;
            } else if def.frame_dependent {
                frame_dependent += 1;
            } else {
                assert!(
                    UNBOUNDED.contains(&(op.label(), def.name)),
                    "{op:?}.{} is a published number with no domain and no \
                     `unbounded` reason",
                    def.name
                );
            }
        }
    }
    // Non-vacuity: the four feeds alone are more than 80 numbers, and the
    // two drill families carry an absolute retract Z.
    assert!(numbers > 200, "only {numbers} published numbers found");
    assert!(frame_dependent >= 2, "{frame_dependent} frame-dependent");
    assert_eq!(numbers, bounded + frame_dependent + UNBOUNDED.len());
}

/// Assertion 2.
#[test]
fn the_param_kind_agrees_with_the_wire_type() {
    let mut lists = 0;
    for op in OperationType::ALL {
        for def in op.registry_entry().params.defs {
            let wire_list = def.type_name.starts_with("vec<");
            assert_eq!(
                def.kind == ParamKind::List,
                wire_list,
                "{op:?}.{}: kind {:?}, type {}",
                def.name,
                def.kind,
                def.type_name
            );
            lists += usize::from(wire_list);
        }
    }
    assert!(lists >= 1, "the Profile `tab_points` list is published");
}

/// One toolpath per operation type, in `OperationType::ALL` order, so a
/// toolpath index IS an index into that list. `add_toolpath` validates
/// nothing; the param route is under test, not the generator.
fn all_ops_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_under(20.0, 4.0));
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model = polygon_model(vec![square_polygon(2.0)], "f185_all_ops");
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

fn set(
    session: &mut ProjectSession,
    index: usize,
    param: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value,
        }))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Assertion 3.
#[test]
fn a_zero_or_negative_feed_is_refused_on_every_op() {
    let mut session = all_ops_session();
    // (name, an accepted positive value)
    let feeds = [
        ("feed_rate", json!(600.0)),
        ("plunge_rate", json!(200.0)),
        ("ramp_feed_rate", json!(300.0)),
        ("spindle_rpm", json!(12000)),
    ];
    let mut refused = [0usize; 4];
    for (index, op) in OperationType::ALL.iter().enumerate() {
        let names = OperationConfig::param_names_for_type(*op);
        for (slot, (name, good)) in feeds.iter().enumerate() {
            if !names.contains(name) {
                continue;
            }
            for bad in [json!(0), json!(-1)] {
                let Err(e) = set(&mut session, index, name, bad.clone()) else {
                    panic!("{op:?} accepted {name} = {bad}");
                };
                // Zero reaches the domain check. A negative spindle speed
                // is refused before it, by the `u32` parse of its own arm.
                let domain = e.contains("outside the accepted range");
                let integer = *name == "spindle_rpm" && e.contains("out of range");
                assert!(
                    domain || (bad != json!(0) && integer),
                    "{op:?} {name} = {bad}: {e}"
                );
            }
            refused[slot] += 1;
            if let Err(e) = set(&mut session, index, name, good.clone()) {
                panic!("{op:?} refused {name} = {good}: {e}");
            }
        }
    }
    let all = OperationType::ALL.len();
    // Every op publishes a feed and a spindle speed. The two drill
    // families publish no separate plunge feed and no ramp feed.
    assert_eq!(refused[0], all, "feed_rate population");
    assert_eq!(refused[3], all, "spindle_rpm population");
    assert_eq!(refused[1], all - 2, "plunge_rate population");
    assert_eq!(refused[2], all - 2, "ramp_feed_rate population");
}
