//! W2b (toolpath inspector programme, 2026-10-30): THE placement of the
//! diagnostics of one toolpath in the inspector.
//!
//! [`place`] is the ONLY reader of the toolpath's diagnostic list in the
//! inspector. It reads `Diagnostic.anchor` (core
//! `diagnostics::presentation`) and gives each finding ONE place:
//!
//! - `Anchor::Tool` — the header Tool row: its Danger or Caution state, and
//!   the faces in its hover. No other text.
//! - `Anchor::Field { key }` — the `PropRow` of that key: its state and the
//!   faces and details in its hover. A field whose row is not a `PropRow`
//!   yet (every key not in [`PROP_ROW_FIELDS`]; W3 moves the rows) goes to
//!   the issue stack. Either way the tab that holds the field shows a dot
//!   ([`field_tab`]).
//! - `Anchor::Operation` — the issue stack under the header.
//! - `Anchor::Note`, and any finding of severity Info or Hint — the notes
//!   counter of the status line. An advisory never takes an issue row.
//! - `Anchor::Project`, `Anchor::View`, and a finding that is not current
//!   (it needs a simulation, or its evidence is stale) — nothing in the
//!   toolpath inspector. Readiness, the rail and the viewport own them.

use std::collections::BTreeMap;

use rs_cam_core::diagnostics::{
    Anchor, Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticState, FieldKey,
    Severity,
};

use super::ToolpathTab;

/// The field keys whose row is a `PropRow` that draws the field state. A
/// finding anchored to any other field key falls back to the issue stack.
///
/// W2b: no field row is a `PropRow` yet, so this list is empty and every
/// field finding falls back. W3 adds each key here when it moves the row
/// onto `PropRow`; the work list is every key that core can anchor to
/// (`diagnostics::presentation::known_field_names`).
pub(crate) const PROP_ROW_FIELDS: &[&str] = &[];

/// Where each diagnostic of one toolpath goes ([`place`]).
#[derive(Debug, Default)]
pub(crate) struct Placement<'a> {
    /// The findings each field `PropRow` draws, by the field key.
    pub by_field: BTreeMap<&'static str, Vec<&'a Diagnostic>>,
    /// The findings of the header Tool row.
    pub tool: Vec<&'a Diagnostic>,
    /// The issue rows under the header, in list order.
    pub issues: Vec<&'a Diagnostic>,
    /// The notes of the status line counter.
    pub notes: Vec<&'a Diagnostic>,
    /// The findings the toolpath inspector does not draw: a project or
    /// view anchor, or a finding that is not current.
    pub elsewhere: Vec<&'a Diagnostic>,
    /// The dot of each tab that holds a field with a finding: the tab and
    /// the role of the worst finding on it.
    pub dots: Vec<(ToolpathTab, crate::ui::components::Role)>,
}

/// True when the row of `key` is a `PropRow` ([`PROP_ROW_FIELDS`]).
pub(crate) fn field_row_is_prop_row(key: FieldKey) -> bool {
    PROP_ROW_FIELDS.contains(&key.as_str())
}

/// The feeds fields of the Feeds tab.
const FEEDS_FIELDS: &[&str] = &["feed_rate", "plunge_rate", "spindle_rpm", "ramp_feed_rate"];

/// The tab that draws the field `key`.
///
/// - The heights and the op floor (the "Bottom height" row, D6): Heights.
/// - The dressup fields (`DressupConfig::FIELD_DEFS`): Linking.
/// - The feeds: Feeds.
/// - The selection: Geometry.
/// - Every other op parameter: Passes.
///
/// No field key is drawn in the header: the tool is not a field
/// (`Anchor::Tool`).
#[must_use]
pub(crate) fn field_tab(key: FieldKey) -> ToolpathTab {
    use rs_cam_core::compute::config::DressupConfig;
    use rs_cam_core::diagnostics::presentation::{HEIGHT_KEYS, SELECTION_KEY};
    let name = key.as_str();
    if HEIGHT_KEYS.contains(&name) || name == "floor" {
        ToolpathTab::Heights
    } else if DressupConfig::FIELD_DEFS.iter().any(|d| d.name == name) {
        ToolpathTab::Linking
    } else if FEEDS_FIELDS.contains(&name) {
        ToolpathTab::FeedsSpeeds
    } else if name == SELECTION_KEY {
        ToolpathTab::Geometry
    } else {
        ToolpathTab::Passes
    }
}

/// The issue role of a finding: Danger for a finding that blocks or is
/// critical, Caution else.
#[must_use]
pub(crate) fn role_of(d: &Diagnostic) -> crate::ui::components::Role {
    use crate::ui::components::Role;
    match d.severity {
        Severity::Blocking | Severity::Critical => Role::Danger,
        Severity::Caution => Role::Caution,
        Severity::Info | Severity::Hint => Role::Info,
    }
}

/// True for an advisory finding: it informs and asks for no fix.
fn is_advisory(d: &Diagnostic) -> bool {
    matches!(d.severity, Severity::Info | Severity::Hint) || matches!(d.category, Category::State)
}

/// Place each diagnostic of one toolpath. The module doc gives the rule.
#[must_use]
pub(crate) fn place(diagnostics: &[Diagnostic]) -> Placement<'_> {
    let mut out = Placement::default();
    for d in diagnostics {
        if d.state != DiagnosticState::Current {
            out.elsewhere.push(d);
            continue;
        }
        match d.anchor {
            Anchor::Project | Anchor::View => out.elsewhere.push(d),
            Anchor::Tool => out.tool.push(d),
            Anchor::Note => out.notes.push(d),
            _ if is_advisory(d) => out.notes.push(d),
            Anchor::Operation => out.issues.push(d),
            Anchor::Field { key } => {
                // A Danger or Caution dot on the tab that holds the field.
                // Efficiency findings stay quiet on the strip, as before W2b.
                if d.category != Category::Efficiency {
                    raise_dot(&mut out.dots, field_tab(key), role_of(d));
                }
                if field_row_is_prop_row(key) {
                    out.by_field.entry(key.as_str()).or_default().push(d);
                } else {
                    out.issues.push(d);
                }
            }
        }
    }
    out
}

/// Raise the dot of `tab` to `role` when `role` ranks higher.
fn raise_dot(
    dots: &mut Vec<(ToolpathTab, crate::ui::components::Role)>,
    tab: ToolpathTab,
    role: crate::ui::components::Role,
) {
    match dots.iter_mut().find(|(t, _)| *t == tab) {
        Some((_, old)) if old.severity_rank() > role.severity_rank() => *old = role,
        Some(_) => {}
        None => dots.push((tab, role)),
    }
}

/// The hover detail of a finding: the full message, then the confidence
/// and the evidence. The face is the row text.
#[must_use]
pub(crate) fn detail(d: &Diagnostic) -> String {
    let mut out = d.message.clone();
    let chip = confidence_label(d.confidence);
    if !chip.is_empty() && !is_advisory(d) {
        out.push_str(&format!("\nConfidence: {chip}"));
    }
    if let Some(line) = d.evidence.as_ref().and_then(evidence_line) {
        out.push_str(&format!("\n{line}"));
    }
    out
}

/// The hover of the header Tool row for its findings: one face per line,
/// then the detail of each.
#[must_use]
pub(crate) fn tool_hover(tool: &[&Diagnostic]) -> Option<String> {
    if tool.is_empty() {
        return None;
    }
    Some(
        tool.iter()
            .map(|d| format!("{}\n{}", d.face, detail(d)))
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

/// The state of the header Tool row from its findings: Danger for one that
/// blocks, Caution for a caution, else none.
#[must_use]
pub(crate) fn tool_role(tool: &[&Diagnostic]) -> Option<crate::ui::components::Role> {
    use crate::ui::components::Role;
    tool.iter()
        .map(|d| role_of(d))
        .filter(|role| matches!(role, Role::Danger | Role::Caution))
        .min_by_key(|role| role.severity_rank())
}

fn confidence_label(c: Confidence) -> &'static str {
    match c {
        Confidence::Verified => "verified",
        Confidence::Approximate => "approximate",
        Confidence::Static => "",
        Confidence::Heuristic => "heuristic",
    }
}

/// One-line evidence summary for the hover. `None` when the evidence has
/// nothing to show.
fn evidence_line(ev: &DiagnosticEvidence) -> Option<String> {
    use DiagnosticEvidence as E;
    Some(match ev {
        E::SampleRange {
            sample_start,
            sample_end,
            observed,
            threshold,
            unit,
            locality,
            ..
        } => {
            let thr = threshold
                .map(|t| format!(", threshold {t:.3}"))
                .unwrap_or_default();
            let loc = if locality.is_empty() {
                String::new()
            } else {
                format!(" [{}]", locality.as_str())
            };
            format!(
                "at samples {sample_start}-{sample_end}: observed {observed:.3} {unit}{thr}{loc}"
            )
        }
        E::LutCitation {
            row_id,
            min,
            max,
            observed,
            unit,
            extrapolated,
        } => {
            let min_s = min
                .map(|v| format!("min {v:.3}"))
                .unwrap_or_else(|| "min \u{2014}".to_owned());
            let max_s = max.map(|m| format!(", max {m:.3}")).unwrap_or_default();
            let ext = if *extrapolated { " (extrapolated)" } else { "" };
            format!("LUT `{row_id}`: observed {observed:.3} {unit} ({min_s}{max_s}){ext}")
        }
        E::GeometryCompare {
            lhs_label,
            lhs_value,
            rhs_label,
            rhs_value,
            unit,
        } => format!("{lhs_label} ({lhs_value:.3} {unit}) vs {rhs_label} ({rhs_value:.3} {unit})"),
        E::Move {
            move_index,
            position,
            ..
        } => {
            if let Some([x, y, z]) = position {
                format!("at move {move_index} ({x:.2}, {y:.2}, {z:.2})")
            } else {
                format!("at move {move_index}")
            }
        }
        E::Counts {
            count,
            offender_toolpath_ids,
        } if *count > 0 => {
            if offender_toolpath_ids.is_empty() {
                format!("{count} occurrences")
            } else {
                let ids: Vec<String> = offender_toolpath_ids
                    .iter()
                    .map(|i| i.to_string())
                    .collect();
                format!("{count} occurrences across toolpaths {}", ids.join(", "))
            }
        }
        E::Counts { .. } => return None,
    })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use rs_cam_core::diagnostics::presentation::known_field_names;

    /// W2b: every field key an anchor can name has a tab that draws it, and
    /// that tab is on the strip of every op that publishes the field. This
    /// replaces the F24 `OWNER_TAB` coverage test: the tab comes from the
    /// anchor, not from a second id table.
    #[test]
    fn every_anchor_field_has_a_tab_w2b() {
        use rs_cam_core::compute::catalog::OperationType;
        use rs_cam_core::diagnostics::presentation::op_has_field;
        let mut seen = 0_usize;
        for name in known_field_names() {
            let key = FieldKey::parse(name).expect("a known name parses");
            let tab = field_tab(key);
            seen += 1;
            for op in OperationType::ALL {
                if op_has_field(*op, name) {
                    let config = rs_cam_core::compute::catalog::OperationConfig::new_default(*op);
                    assert!(
                        tab.applies(&config),
                        "{name} is on {tab:?}, which {op:?} does not draw"
                    );
                }
            }
        }
        assert!(seen > 30, "non-vacuity: only {seen} field names");
        let tab = |n: &str| field_tab(FieldKey::parse(n).expect("a known name"));
        assert_eq!(tab("feed_rate"), ToolpathTab::FeedsSpeeds);
        assert_eq!(tab("spindle_rpm"), ToolpathTab::FeedsSpeeds);
        assert_eq!(tab("retract_z"), ToolpathTab::Heights);
        assert_eq!(tab("entry_style"), ToolpathTab::Linking);
        assert_eq!(tab("selection"), ToolpathTab::Geometry);
        assert_eq!(tab("stepover"), ToolpathTab::Passes);
    }

    fn finding(id: &str, anchor: Anchor, severity: Severity) -> Diagnostic {
        Diagnostic {
            id: id.into(),
            scope: rs_cam_core::diagnostics::Scope::Toolpath {
                id: rs_cam_core::ToolpathId::from_raw(0),
            },
            category: Category::Geometry,
            severity,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: rs_cam_core::diagnostics::Source::StaticValidation,
            message: format!("{id} message"),
            face: format!("{id} face"),
            anchor,
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        }
    }

    /// W2b: each finding goes to exactly ONE place.
    #[test]
    fn each_finding_has_one_place_w2b() {
        let mut stale = finding("e", Anchor::Operation, Severity::Caution);
        stale.state = DiagnosticState::NeedsSimulation;
        let list = vec![
            finding("a", Anchor::Tool, Severity::Blocking),
            finding("b", Anchor::field("stepover"), Severity::Caution),
            finding("c", Anchor::Operation, Severity::Critical),
            finding("d", Anchor::Note, Severity::Info),
            stale,
            finding("f", Anchor::Project, Severity::Critical),
            finding("g", Anchor::View, Severity::Info),
            finding("h", Anchor::Operation, Severity::Hint),
        ];
        let p = place(&list);
        let ids = |v: &[&Diagnostic]| {
            v.iter()
                .map(|d| d.id.as_str().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&p.tool), ["a"]);
        assert_eq!(ids(&p.issues), ["b", "c"], "no field is a PropRow yet");
        assert_eq!(ids(&p.notes), ["d", "h"]);
        assert_eq!(ids(&p.elsewhere), ["e", "f", "g"]);
        assert!(p.by_field.is_empty());
        let placed = p.tool.len() + p.issues.len() + p.notes.len() + p.elsewhere.len();
        assert_eq!(placed, list.len(), "every finding has a place");
        assert_eq!(
            p.dots,
            vec![(ToolpathTab::Passes, crate::ui::components::Role::Caution)],
            "the field finding dots its tab"
        );
        assert_eq!(
            tool_role(&p.tool),
            Some(crate::ui::components::Role::Danger)
        );
    }
}
