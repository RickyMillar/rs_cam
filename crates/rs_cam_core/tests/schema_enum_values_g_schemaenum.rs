//! G-SCHEMAENUM (F1.16) — every value the schema advertises must be a value
//! the config can hold.
//!
//! `ParamDef::type_name` carries strings of the form `enum:a|b|c`
//! (`crates/rs_cam_core/src/compute/catalog.rs`). Two surfaces show them to
//! an operator and neither validates them:
//!
//! - MCP `get_operation_schema` publishes the string.
//! - The CLI prints it in the `PARAM / TYPE / OPTIONAL` table
//!   (`crates/rs_cam_cli/src/run.rs:219`).
//!
//! `ProjectSession::set_toolpath_param` never parses the string. It merges the
//! value into the config's own JSON and hands the result to serde
//! (`session/compute.rs:407-525`). So an advertised value that serde rejects
//! is a pure untruth: the operator reads it, types it and gets
//! `unknown variant`.
//!
//! F1.16 found one. `PROFILE_PARAMS` advertised `enum:on|inside|outside`
//! while [`ProfileSide`] has only `Outside` and `Inside`. The pre-fix run of
//! this file said:
//!
//! ```text
//! G-SCHEMAENUM: profile.side advertises 'on', serde says:
//!   unknown variant `on`, expected `outside` or `inside`
//! ```
//!
//! This test is deliberately GENERIC. The claim is not "profile is fixed", it
//! is "no operation advertises a value its config cannot hold", so the next
//! drift of this class fails here rather than reaching an operator.
//!
//! Note what this does NOT check: that the generator DOES something different
//! for each advertised value. Serde accepting `climb=false` says nothing about
//! whether the generator reverses direction. That is a per-operation question
//! and it needs per-operation evidence.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};

/// Read the advertised values out of an `enum:a|b|c` type string.
fn advertised_values(type_name: &str) -> Option<Vec<&str>> {
    type_name
        .strip_prefix("enum:")
        .map(|list| list.split('|').collect())
}

/// Merge one value into the config's own JSON and ask serde to accept it.
///
/// This is the same mechanism `set_toolpath_param` uses: the tagged
/// representation is `{"kind": ..., "params": {...}}`, the value goes into
/// `params`, and the whole object round-trips through
/// `serde_json::from_value`. No numeric coercion branch applies — every
/// value here is a string.
///
/// `optional` is the def's own `optional` flag, and it decides what an ABSENT
/// key means. A REQUIRED param that the default config does not serialize is
/// drift: no such field exists, so the published name is an untruth. An
/// OPTIONAL param may legitimately be absent — `skip_serializing_if` omits it
/// while it holds its default, which is exactly how
/// `UnifiedFinishConfig::classification_sampler` and
/// `PencilConfig::link_hop_distance_mm` are written. `set_toolpath_param`
/// handles that case: it inserts the key whether or not it existed and refuses
/// only when the def is also absent (`session/compute/params.rs`, the
/// `!existed && target_type.is_none()` arm). So for an optional param this
/// asks serde the same question the setter asks. A bogus optional NAME is
/// caught by the `param_defs`-covers-the-struct sentry, not here.
fn serde_accepts(
    op: &OperationConfig,
    param: &str,
    value: &str,
    optional: bool,
) -> Result<(), String> {
    let mut json = serde_json::to_value(op).map_err(|e| e.to_string())?;
    let params = json
        .get_mut("params")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "config has no `params` object".to_owned())?;
    let present = params.contains_key(param);
    params.insert(
        param.to_owned(),
        serde_json::Value::String(value.to_owned()),
    );
    if !present && !optional {
        return Err(format!(
            "the schema names `{param}` as REQUIRED but the serialized config \
             carries no such field, so the name itself is drift"
        ));
    }
    serde_json::from_value::<OperationConfig>(json)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn every_advertised_enum_value_is_one_the_config_can_hold() {
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0_usize;
    let mut enum_params = 0_usize;

    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let hints = op.param_schema_hints();
        for param in OperationConfig::param_names_for_type(op_type) {
            let Some(type_name) = op.param_type_name(param) else {
                continue;
            };
            let Some(values) = advertised_values(type_name) else {
                continue;
            };
            let optional = hints.get(param).is_some_and(|h| h.optional);
            enum_params += 1;
            for value in values {
                checked += 1;
                if let Err(e) = serde_accepts(&op, param, value, optional) {
                    failures.push(format!(
                        "{:?}.{param} advertises '{value}' ({type_name}) — {e}",
                        op_type
                    ));
                }
            }
        }
    }

    eprintln!(
        "G-SCHEMAENUM: {} operations, {enum_params} enum params, {checked} advertised values",
        OperationType::ALL.len()
    );
    for f in &failures {
        eprintln!("  REJECTED: {f}");
    }

    // Non-vacuity: an empty population would pass this test while measuring
    // nothing. The registry has enum params today; if it ever has none, that
    // is itself the thing to look at.
    assert!(
        enum_params >= 10,
        "measured only {enum_params} enum params — too few to be the whole \
         registry, so a green result here would be vacuous"
    );
    assert!(
        failures.is_empty(),
        "the schema advertises {} value(s) no config can hold:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The specific F1.16 case, pinned so the fix cannot silently regress.
///
/// Profile's side is `inside|outside` and nothing more. The on-the-line cut
/// exists in this repo twice under its own names — `project_curve` with
/// `side: center` (labelled "On Line") and `trace` with
/// `compensation: none`, whose doc says "tool center follows the path
/// exactly" — so a third spelling on Profile would claim the capability lives
/// somewhere it does not.
#[test]
fn profile_side_advertises_only_the_two_sides_it_cuts() {
    let op = OperationConfig::new_default(OperationType::Profile);
    let type_name = op
        .param_type_name("side")
        .expect("profile advertises a side param");
    assert_eq!(
        type_name, "enum:inside|outside",
        "profile's side schema must name exactly the two ProfileSide variants"
    );

    // The capability it used to claim, where it really lives.
    let pc = OperationConfig::new_default(OperationType::ProjectCurve);
    assert_eq!(
        pc.param_type_name("side"),
        Some("enum:center|inside|outside"),
        "project_curve keeps its center (On Line) side — that is the \
         on-the-line cut, and it is truthful there"
    );
}
