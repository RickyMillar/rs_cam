//! CONTRACT K1 (feeds contract P4): every op states its cut.
//!
//! `OperationConfig::cut_spec` builds one `CutSpec` from the op's own
//! config. The registry row states the `CutRule` that the arm follows. This
//! sentry walks `OperationType::ALL` and checks two claims for each op:
//!
//! 1. The kind of each axis of `cut_spec()` is the kind that the row's
//!    `cut_rule` names. A rule that reads the config runs in both states:
//!    DropCutter `scallop_height` set and not set, Pencil
//!    `num_offset_passes` 1 and 2, Adaptive `slot_clearing` true and false,
//!    and each `Option` depth set and not set.
//! 2. Each field that the rule names reaches the spec bit for bit. The
//!    probe value comes from the field's own `ParamDef` range in the op's
//!    `PARAMS`: the midpoint of a closed range, else twice the default
//!    inside the lower bound. No probe number is typed here.
//!
//! The test does not freeze a table of answers. It checks that the op's own
//! field reaches the spec. It replaces the frozen `NONE` table of
//! `operation_feeds_hints_is_an_explicit_per_op_decision` (Phase 5 deletes
//! that test).
//!
//! The config goes through `serde_json::to_value` and `from_value`. Both
//! hold an `f64` as an `f64` (no text), so the probe stays bit for bit.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{
    Axial, AxialRule, CutRule, CutSpec, FirstPass, OperationConfig, OperationType, ParamDef,
    Radial, RadialRule, TotalDepth,
};
use serde_json::{Value, json};

fn defs(op: OperationType) -> &'static [ParamDef] {
    op.registry_entry().params.defs
}

fn def(op: OperationType, field: &str) -> Option<&'static ParamDef> {
    defs(op).iter().find(|d| d.name == field)
}

fn default_params(op: OperationType) -> Value {
    serde_json::to_value(OperationConfig::new_default(op)).unwrap()["params"].clone()
}

/// The probe value of `field` of `op`: the midpoint of a closed range, else
/// twice the default inside the lower bound. An `Option` field with no
/// default and no closed range of its own (DropCutter `scallop_height`)
/// takes the midpoint of the first closed range of a field of the same
/// name in another op.
fn probe(op: OperationType, field: &str) -> f64 {
    let own = def(op, field).unwrap_or_else(|| panic!("{op:?}: no ParamDef `{field}`"));
    if let Some(mid) = closed_midpoint(own) {
        return mid;
    }
    if let Some(default) = default_params(op)[field].as_f64() {
        let twice = 2.0 * default;
        let range = own
            .range
            .unwrap_or_else(|| panic!("{op:?}.{field}: no range"));
        assert!(range.accepts(twice), "{op:?}.{field}: {twice} outside");
        return twice;
    }
    OperationType::ALL
        .iter()
        .filter_map(|&other| def(other, field))
        .find_map(closed_midpoint)
        .unwrap_or_else(|| panic!("{op:?}.{field}: no closed range in any op"))
}

fn closed_midpoint(def: &ParamDef) -> Option<f64> {
    let range = def.range?;
    let mid = 0.5 * (range.min? + range.max?);
    assert!(range.accepts(mid), "{}: midpoint {mid} outside", def.name);
    Some(mid)
}

/// The op's default config with each edit applied, and its params as JSON.
fn config(op: OperationType, edits: &[(&str, Value)]) -> (OperationConfig, Value) {
    let mut value = serde_json::to_value(OperationConfig::new_default(op)).unwrap();
    for (field, edit) in edits {
        value["params"][*field] = edit.clone();
    }
    let cfg: OperationConfig = serde_json::from_value(value).unwrap();
    let params = serde_json::to_value(&cfg).unwrap()["params"].clone();
    (cfg, params)
}

/// The field that holds the one-pass depth of an op whose row rule is
/// `AxialRule::OnePass`.
fn one_pass_field(op: OperationType) -> &'static str {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    match op {
        OperationType::Inlay => "pocket_depth",
        OperationType::ProjectCurve => "depth",
        _ => panic!("{op:?}: OnePass names no field here"),
    }
}

/// The field that holds the surface cap of an op whose row rule is
/// `AxialRule::SurfaceCapped`.
fn surface_cap_field(op: OperationType) -> &'static str {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    match op {
        OperationType::VCarve => "max_depth",
        OperationType::RampFinish => "max_stepdown",
        _ => panic!("{op:?}: SurfaceCapped names no field here"),
    }
}

/// The fields that the rule reads.
fn rule_fields(op: OperationType, rule: CutRule) -> Vec<&'static str> {
    let mut fields: Vec<&'static str> = match rule.radial {
        RadialRule::Stepover => vec!["stepover"],
        RadialRule::Scallop => vec!["scallop_height"],
        RadialRule::ScallopElseStepover => vec!["scallop_height", "stepover"],
        RadialRule::OffsetStepoverElseSurface => vec!["offset_stepover"],
        RadialRule::AngularStep => vec!["angular_step"],
        RadialRule::Slot
        | RadialRule::Groove
        | RadialRule::SurfaceFollow
        | RadialRule::NoRadial => {
            vec![]
        }
    };
    fields.extend(match rule.axial {
        AxialRule::DepthPerPass => Some("depth_per_pass"),
        AxialRule::ZStep => Some("z_step"),
        AxialRule::OnePass => Some(one_pass_field(op)),
        AxialRule::Peck => Some("peck_depth"),
        AxialRule::HelixPitch => Some("pitch"),
        AxialRule::ChamferWidth => Some("chamfer_width"),
        AxialRule::SurfaceCapped => Some(surface_cap_field(op)),
        AxialRule::Surface => None,
    });
    fields
}

/// The states of the config-dependent rules: each entry is one set of
/// edits on top of the probes.
fn states(
    op: OperationType,
    rule: CutRule,
    fields: &[&'static str],
) -> Vec<Vec<(&'static str, Value)>> {
    let mut states: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    // Each `Option` field the rule reads: set, and not set.
    for &field in fields {
        if def(op, field).unwrap().optional {
            states.push(vec![(field, Value::Null)]);
        }
    }
    if rule.radial == RadialRule::OffsetStepoverElseSurface {
        states = vec![
            vec![("num_offset_passes", json!(1))],
            vec![("num_offset_passes", json!(2))],
        ];
    }
    // Adaptive: the first pass reads `slot_clearing`.
    if op == OperationType::Adaptive {
        states = vec![
            vec![("slot_clearing", json!(true))],
            vec![("slot_clearing", json!(false))],
        ];
    }
    // Total depth: an `Option` depth that no axis rule reads (Trace
    // `depth`, InlayPlug `pocket_depth`) runs set and not set.
    for field in ["depth", "pocket_depth"] {
        if !fields.contains(&field) && def(op, field).is_some_and(|d| d.optional) {
            states.push(vec![(field, Value::Null)]);
        }
    }
    states
}

fn mm_of(params: &Value, field: &str) -> Option<f64> {
    params[field].as_f64()
}

/// P10: a tool-derived field is unset with no tool; the spec then holds
/// `None`.
fn optional_bits(spec: Option<f64>, params: &Value, field: &str, what: &str) {
    match spec {
        Some(v) => same_bits(v, mm_of(params, field), what),
        None => assert!(params[field].is_null(), "{what}: {field} is set"),
    }
}

fn same_bits(spec: f64, field: Option<f64>, what: &str) {
    let field = field.unwrap_or_else(|| panic!("{what}: the field is not set"));
    assert_eq!(spec.to_bits(), field.to_bits(), "{what}: {spec} != {field}");
}

fn check_radial(op: OperationType, rule: RadialRule, radial: Radial, params: &Value) {
    let what = format!("{op:?} radial {rule:?} {radial:?}");
    match (rule, radial) {
        (RadialRule::Stepover, Radial::Width(w)) => {
            optional_bits(w.map(|w| w.mm()), params, "stepover", &what);
        }
        (RadialRule::Scallop, Radial::Scallop(s)) => {
            same_bits(s.mm(), mm_of(params, "scallop_height"), &what);
        }
        (RadialRule::ScallopElseStepover, Radial::Scallop(s)) => {
            same_bits(s.mm(), mm_of(params, "scallop_height"), &what);
        }
        (RadialRule::ScallopElseStepover, Radial::Width(w)) => {
            assert!(params["scallop_height"].is_null(), "{what}: scallop set");
            optional_bits(w.map(|w| w.mm()), params, "stepover", &what);
        }
        (RadialRule::OffsetStepoverElseSurface, Radial::Width(w)) => {
            assert!(params["num_offset_passes"].as_u64().unwrap() > 1, "{what}");
            optional_bits(w.map(|w| w.mm()), params, "offset_stepover", &what);
        }
        (RadialRule::OffsetStepoverElseSurface, Radial::SurfaceFollow) => {
            assert!(params["num_offset_passes"].as_u64().unwrap() <= 1, "{what}");
        }
        (RadialRule::AngularStep, Radial::AngularStep(a)) => {
            same_bits(a.get(), mm_of(params, "angular_step"), &what);
        }
        (RadialRule::Slot, Radial::Slot)
        | (RadialRule::Groove, Radial::Groove)
        | (RadialRule::SurfaceFollow, Radial::SurfaceFollow)
        | (RadialRule::NoRadial, Radial::Axial) => {}
        (
            RadialRule::Stepover
            | RadialRule::Slot
            | RadialRule::Groove
            | RadialRule::Scallop
            | RadialRule::ScallopElseStepover
            | RadialRule::OffsetStepoverElseSurface
            | RadialRule::AngularStep
            | RadialRule::SurfaceFollow
            | RadialRule::NoRadial,
            Radial::Width(_)
            | Radial::Slot
            | Radial::Groove
            | Radial::Scallop(_)
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial,
        ) => panic!("{what}: the spec kind is not the row rule"),
    }
}

fn check_axial(op: OperationType, rule: AxialRule, axial: Axial, params: &Value) {
    let what = format!("{op:?} axial {rule:?} {axial:?}");
    let optional = |field: &str, value: Option<f64>| match value {
        Some(v) => same_bits(v, mm_of(params, field), &what),
        None => assert!(params[field].is_null(), "{what}: {field} is set"),
    };
    match (rule, axial) {
        (AxialRule::DepthPerPass, Axial::PerPass(d)) => {
            optional_bits(d.map(|d| d.mm()), params, "depth_per_pass", &what);
        }
        (AxialRule::ZStep, Axial::ZStep(d)) => {
            optional_bits(d.map(|d| d.mm()), params, "z_step", &what);
        }
        (AxialRule::Peck, Axial::Peck(d)) => {
            optional_bits(d.map(|d| d.mm()), params, "peck_depth", &what);
        }
        (AxialRule::HelixPitch, Axial::HelixPitch(d)) => {
            same_bits(d.mm(), mm_of(params, "pitch"), &what);
        }
        (AxialRule::ChamferWidth, Axial::ChamferWidth(d)) => {
            same_bits(d.mm(), mm_of(params, "chamfer_width"), &what);
        }
        (AxialRule::OnePass, Axial::OnePass(d)) => {
            optional(one_pass_field(op), d.map(|d| d.mm()));
        }
        (AxialRule::SurfaceCapped, Axial::Surface { max }) => {
            optional(surface_cap_field(op), max.map(|d| d.mm()));
        }
        (AxialRule::Surface, Axial::Surface { max }) => {
            assert!(max.is_none(), "{what}: an uncapped surface has a cap");
        }
        (
            AxialRule::DepthPerPass
            | AxialRule::ZStep
            | AxialRule::OnePass
            | AxialRule::Peck
            | AxialRule::HelixPitch
            | AxialRule::ChamferWidth
            | AxialRule::Surface
            | AxialRule::SurfaceCapped,
            Axial::PerPass(_)
            | Axial::ZStep(_)
            | Axial::OnePass(_)
            | Axial::Peck(_)
            | Axial::HelixPitch(_)
            | Axial::ChamferWidth(_)
            | Axial::Surface { .. },
        ) => panic!("{what}: the spec kind is not the row rule"),
    }
}

/// The total depth: a reason is not empty; a known depth is not negative; an
/// unset depth has an unset `Option` depth field.
fn check_total(op: OperationType, spec: &CutSpec, params: &Value) {
    match spec.total {
        TotalDepth::Known(d) => assert!(d.mm() >= 0.0, "{op:?}: known total {}", d.mm()),
        TotalDepth::FromHeights => {}
        TotalDepth::Unset => assert!(
            ["depth", "pocket_depth"]
                .iter()
                .any(|f| def(op, f).is_some_and(|d| d.optional) && params[*f].is_null()),
            "{op:?}: total Unset with every depth set"
        ),
        TotalDepth::None(reason) => assert!(!reason.is_empty(), "{op:?}: empty reason"),
    }
}

#[test]
fn every_op_states_its_cut_k1() {
    let mut runs = 0_usize;
    for &op in OperationType::ALL {
        let rule = op.capabilities().cut_rule;
        let fields = rule_fields(op, rule);
        let probes: Vec<(&'static str, Value)> = fields
            .iter()
            .map(|&field| {
                let p = probe(op, field);
                let default = default_params(op)[field].as_f64();
                assert_ne!(
                    default.map(f64::to_bits),
                    Some(p.to_bits()),
                    "{op:?}.{field}: the probe equals the default, so it proves nothing"
                );
                (field, json!(p))
            })
            .collect();
        for state in states(op, rule, &fields) {
            let mut edits = probes.clone();
            edits.extend(state);
            let (cfg, params) = config(op, &edits);
            assert_eq!(
                cfg.capabilities().cut_rule,
                rule,
                "{op:?}: config overrides the row"
            );
            let spec = cfg.cut_spec();
            check_radial(op, rule.radial, spec.radial, &params);
            check_axial(op, rule.axial, spec.axial, &params);
            check_total(op, &spec, &params);
            if op == OperationType::Adaptive {
                let slot = params["slot_clearing"].as_bool().unwrap();
                let want = if slot {
                    FirstPass::Slot
                } else {
                    FirstPass::NotSlot
                };
                assert_eq!(spec.first_pass, want, "{op:?}: first pass");
            }
            runs += 1;
        }
    }
    assert!(
        runs > OperationType::ALL.len(),
        "the config-dependent states ran"
    );
}
