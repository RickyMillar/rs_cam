//! P10 (feeds contract, operator ruling D8): an op has no geometry default
//! without a tool; with a tool each tool-derived field is a rule of it.
//!
//! # The claim
//!
//! For every op and every tool-derived field (`stepover`,
//! `depth_per_pass`, `peck_depth`, `offset_stepover`, `z_step`), through
//! THE add door (`ProjectSession::draft_toolpath`):
//!
//! 1. The registry row states one rule per field the op reads, and each
//!    rule names a slot of the config (`geometry_default::slot_mut`).
//! 2. Two tools of D and 2D give values in ratio 2. An axial field is
//!    `k x D` of its F201 row; a radial field is the width the calculator
//!    reads on an unstated axis (`feeds::default_radial_width`, after the
//!    tool, flute, material and machine adjustments: lead gate A).
//! 3. A scallop field on a ball or tapered-ball tool with a scallop height
//!    h: the value is the chord `2 sqrt(2 R h - h^2)`, R = D / 2.
//! 4. The draft stamps each value with the stamp of its rule.
//! 5. With no tool, each field is unset, and the op stops with
//!    `NotSetKind::Geometry` ("set a tool").
//!
//! Red proof (HEAD b92b78090, before P10): every field held its fixed
//! default, so D 6 and D 12 gave equal values and the no-tool draft held a
//! number (the red run is in the P10 report).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test every_geometry_default_is_a_fraction_of_the_tool_d8
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::geometry_default::{self, KRow, ToolRule};
use rs_cam_core::compute::not_set::NotSetKind;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::{CutterKind, FeedsProvenance};
use rs_cam_core::session::{AddToolArgs, Command, DraftRequest, ProjectSession};

fn kind_of(t: ToolType) -> CutterKind {
    match t {
        ToolType::EndMill => CutterKind::Flat,
        ToolType::BallNose => CutterKind::Ball,
        ToolType::BullNose => CutterKind::Bull,
        ToolType::VBit => CutterKind::VBit,
        ToolType::TaperedBallNose => CutterKind::TaperedBall,
    }
}

/// The op, its stamps and the tool, drafted through THE add door.
fn draft(op: OperationType, tool: Option<(ToolType, f64)>) -> (OperationConfig, FeedsProvenance) {
    let mut s = ProjectSession::new_empty();
    let mut request = DraftRequest::new(op, 0, "op");
    if let Some((tool_type, d)) = tool {
        let _ = s
            .apply(Command::AddTool(AddToolArgs {
                tool: Box::new(tool_at(tool_type, d)),
            }))
            .expect("the tool");
        request = request.tool(s.tools()[0].id);
    }
    let draft = s.draft_toolpath(request).expect("the draft");
    (draft.config.operation, draft.config.feeds_provenance)
}

/// The tool of a draft. The shaft at 2 D keeps a tapered tip inside its
/// domain.
fn tool_at(tool_type: ToolType, d: f64) -> ToolConfig {
    ToolConfig::builder(ToolId::from_raw(0), tool_type)
        .diameter(d)
        .shaft_mm(2.0 * d)
        .shank_mm(2.0 * d)
        .build()
}

/// The radial width the calculator reads on an unstated axis
/// (`feeds::default_radial_width`, lead gate A), for the session defaults.
fn calculator_width(op: &OperationConfig, tool: &ToolConfig) -> f64 {
    let s = ProjectSession::new_empty();
    let input = rs_cam_core::feeds::suggest::feeds_input_for_operation(
        op,
        tool,
        &s.stock_config().material,
        s.machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        rs_cam_core::feeds::SpindleStrategy::default(),
    );
    rs_cam_core::feeds::default_radial_width(&input, s.machine()).0
}

fn value(op: &OperationConfig, field: &str) -> Option<f64> {
    op.params_value_including_nulls()
        .get(field)
        .and_then(serde_json::Value::as_f64)
}

/// The chord of a tip circle of radius `r` at the cusp height `h` (H2),
/// written here from the circle: (s/2)^2 + (r - h)^2 = r^2.
fn chord(r: f64, h: f64) -> f64 {
    2.0 * (r * r - (r - h) * (r - h)).sqrt()
}

#[test]
fn every_geometry_default_is_a_fraction_of_the_tool_d8() {
    let d = 6.0;
    let mut checked_fraction = 0_usize;
    let mut checked_chord = 0_usize;
    let mut failures = Vec::new();
    for &op in OperationType::ALL {
        let rules = geometry_default::rules(op);
        // 1. One rule per field the op reads, and a slot for each.
        let mut fresh = OperationConfig::new_default(op);
        let read: Vec<&str> = geometry_default::read_fields(&fresh)
            .into_iter()
            .map(|(f, _)| f)
            .collect();
        for field in &read {
            if !rules.iter().any(|r| r.field == *field) {
                failures.push(format!("{op:?}.{field}: read but no rule"));
            }
        }
        for rule in rules {
            if geometry_default::slot_mut(&mut fresh, rule.field).is_none() {
                failures.push(format!("{op:?}.{}: a rule with no slot", rule.field));
            }
        }
        // 5. No tool: unset, and the op stops with "set a tool".
        let (none, _) = draft(op, None);
        for rule in rules {
            if let Some(v) = value(&none, rule.field) {
                failures.push(format!(
                    "{op:?}.{}: no tool -> {v} (want unset)",
                    rule.field
                ));
            }
        }
        if !geometry_default::unset_geometry(&none).is_empty() {
            let kind = none.not_set().map(|n| n.kind);
            let depth_first = none.unset_depths().is_empty();
            if depth_first && kind != Some(NotSetKind::Geometry) {
                failures.push(format!("{op:?}: no tool stops with {kind:?}"));
            }
        }
        // 2-4. Each accepted tool type at D and 2D.
        for &tool_type in ToolType::ALL {
            if op.tool_refusal(kind_of(tool_type)).is_some() {
                continue;
            }
            let (at_d, stamps) = draft(op, Some((tool_type, d)));
            let (at_2d, _) = draft(op, Some((tool_type, 2.0 * d)));
            let tool = tool_at(tool_type, d);
            let tool_2d = tool_at(tool_type, 2.0 * d);
            let session = ProjectSession::new_empty();
            let ctx = geometry_default::RuleContext {
                tool: &tool,
                material: &session.stock_config().material,
                machine: session.machine(),
            };
            for rule in rules {
                let what = format!("{op:?}.{} {tool_type:?}", rule.field);
                let (Some(a), Some(b)) = (value(&at_d, rule.field), value(&at_2d, rule.field))
                else {
                    failures.push(format!("{what}: unset with a tool"));
                    continue;
                };
                let derived = rule.derive(&at_d, ctx);
                let slot = geometry_default::stamp_slot(&at_d, rule.field);
                if stamps.get(slot) != Some(&derived.stamp) {
                    failures.push(format!("{what}: stamp {:?}", stamps.get(slot)));
                }
                let h = at_d.scallop_height();
                let chord_case = matches!(rule.rule, ToolRule::ScallopChordElse(_))
                    && tool_type.has_ball_tip()
                    && h.is_some_and(|h| h > 0.0 && h < d / 2.0);
                if chord_case {
                    let h = h.expect("the chord case has h");
                    let want = chord(d / 2.0, h);
                    if (a - want).abs() > 1e-9 {
                        failures.push(format!("{what}: chord {a} != {want}"));
                    }
                    checked_chord += 1;
                    continue;
                }
                let k = match rule.rule {
                    ToolRule::FractionOfD(k) | ToolRule::ScallopChordElse(k) => k,
                };
                // A radial rule is the calculator's own width (gate A); an
                // axial rule is k x D of its row.
                let (want_d, want_2d) = match k {
                    KRow::FamilyProfile(_) => (
                        calculator_width(&at_d, &tool),
                        calculator_width(&at_2d, &tool_2d),
                    ),
                    KRow::Constant(_) => {
                        let k = k.row(op).value;
                        (k * d, k * 2.0 * d)
                    }
                };
                let range = OperationConfig::param_range_for_type(op, rule.field)
                    .map(|r| r.inclusive_bounds());
                let held = |v: f64| range.as_ref().map_or(v, |r| v.clamp(*r.start(), *r.end()));
                if (a - held(want_d)).abs() > 1e-9 {
                    failures.push(format!("{what}: {a} != the rule {want_d} at D {d}"));
                }
                // Ratio 2 where the range holds neither value.
                if (held(want_d) - want_d).abs() < 1e-12
                    && (held(want_2d) - want_2d).abs() < 1e-12
                    && ((b / a) - 2.0).abs() > 1e-9
                {
                    failures.push(format!("{what}: D {d} -> {a}, D {} -> {b}", 2.0 * d));
                }
                checked_fraction += 1;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // Non-vacuity: both rule forms ran.
    assert!(checked_fraction > 40, "fraction cases: {checked_fraction}");
    assert!(checked_chord >= 2, "chord cases: {checked_chord}");
}
