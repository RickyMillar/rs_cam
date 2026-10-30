//! W2b (toolpath inspector programme, 2026-10-30): THE placement of the
//! diagnostics of one toolpath in the inspector.
//!
//! [`place`] is the ONLY reader of the toolpath's diagnostic list in the
//! inspector. It reads `Diagnostic.anchor` (core
//! `diagnostics::presentation`) and gives each finding ONE place:
//!
//! - `Anchor::Tool` — the header Tool row: its Danger or Caution state, and
//!   the faces in its hover. No other text.
//! - `Anchor::Field { key }` — the `PropRow` of that key, when THIS op
//!   draws the row in its current config ([`field_tab`]): its state and
//!   the faces and details in its hover, and a dot on the tab that holds
//!   the row ([`field_tab`]). A field that the op does not draw goes to
//!   the issue stack, with no dot.
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

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::DressupConfig;

use super::ToolpathTab;

// ── The record of the drawn field rows (tests only) ─────────────────

#[cfg(test)]
thread_local! {
    /// The field keys that the rows asked a state for, while a test records.
    static DRAWN: std::cell::RefCell<Option<std::collections::BTreeSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Note that a field row of `key` is drawn: the row read its state. Every
/// reader of a field state calls it (`Fields::state`, the heights rows, the
/// Cuts row). It does nothing outside a test record.
pub(crate) fn note_drawn(key: &str) {
    #[cfg(test)]
    DRAWN.with(|d| {
        if let Some(set) = d.borrow_mut().as_mut() {
            let _ = set.insert(key.to_owned());
        }
    });
    #[cfg(not(test))]
    let _ = key;
}

/// True while a test records the drawn rows. The `advanced` section then
/// opens, so that its rows draw and record.
pub(crate) fn recording() -> bool {
    #[cfg(test)]
    return DRAWN.with(|d| d.borrow().is_some());
    #[cfg(not(test))]
    false
}

/// Run `draw` and give the field keys that its rows read a state for.
#[cfg(test)]
pub(crate) fn record_drawn(draw: impl FnOnce()) -> std::collections::BTreeSet<String> {
    DRAWN.with(|d| *d.borrow_mut() = Some(Default::default()));
    draw();
    DRAWN.with(|d| d.borrow_mut().take().unwrap_or_default())
}

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

/// The tab whose `PropRow` draws the field `key` for the op `config`, or
/// `None` when this op draws no such row in this config (W3f).
///
/// The rule is per tab, from the same predicates that the tabs draw with:
///
/// - Heights: the five heights (`HEIGHT_KEYS`). An `OpFloor` op draws the
///   `bottom_z` state on its Bottom height row.
/// - Geometry: the selection, on an op with a selection (the Cuts row).
/// - Feeds: `feed_rate` and `spindle_rpm`; `plunge_rate` on an op that is
///   not Z-only; `ramp_feed_rate` on an op that has it.
/// - Linking: `entry_style` where core `DressupPolicy::entry_style_applies`;
///   the 3D Rough draws its own Entry style row on Passes.
/// - Passes: every other parameter of the op, with the exceptions of its
///   editor: `peck_depth` only for a Peck or Chip-break cycle;
///   `scallop_height` not on the Drop Cutter.
///
/// A drill draws `feed_rate` and `retract_z` on Passes too (its cycle
/// rows); the tab here is the canonical one (Feeds, Heights).
///
/// `drawn_field_rows_match_the_painted_rows_w3f` (`properties::tests`)
/// draws every tab of every op headless and holds this rule against the
/// rows that read a field state, for each key of
/// `presentation::anchor_field_names`.
#[must_use]
pub(crate) fn field_tab(key: FieldKey, config: &OperationConfig) -> Option<ToolpathTab> {
    use rs_cam_core::diagnostics::presentation::{HEIGHT_KEYS, SELECTION_KEY, op_has_field};
    let op = config.op_type();
    let name = key.as_str();
    if !op_has_field(op, name) {
        return None;
    }
    let tab = match name {
        _ if HEIGHT_KEYS.contains(&name) => ToolpathTab::Heights,
        SELECTION_KEY => {
            let _ = config.selection()?;
            ToolpathTab::Geometry
        }
        "feed_rate" | "spindle_rpm" => ToolpathTab::FeedsSpeeds,
        "plunge_rate" => {
            if op.is_drill_kinematics() {
                return None;
            }
            ToolpathTab::FeedsSpeeds
        }
        "ramp_feed_rate" => ToolpathTab::FeedsSpeeds,
        "entry_style" if op == OperationType::Adaptive3d => ToolpathTab::Passes,
        "entry_style" => {
            if !op.registry_entry().dressup_policy.entry_style_applies() {
                return None;
            }
            ToolpathTab::Linking
        }
        "peck_depth" => {
            let cycle = match config {
                OperationConfig::Drill(c) => c.cycle,
                OperationConfig::AlignmentPinDrill(c) => c.cycle,
                _ => return None,
            };
            if !super::operations::cycle_draws_peck(cycle) {
                return None;
            }
            ToolpathTab::Passes
        }
        "scallop_height" if op == OperationType::DropCutter => return None,
        _ if DressupConfig::FIELD_DEFS.iter().any(|d| d.name == name) => ToolpathTab::Linking,
        _ => ToolpathTab::Passes,
    };
    tab.applies(config).then_some(tab)
}

/// The field rows that the op `config` draws, for each key a finding can
/// anchor to (`presentation::anchor_field_names`): the key and its tab
/// ([`field_tab`]). The sentries read it; `place` asks `field_tab` per
/// finding.
#[cfg(test)]
#[must_use]
pub(crate) fn drawn_fields(config: &OperationConfig) -> BTreeMap<&'static str, ToolpathTab> {
    rs_cam_core::diagnostics::presentation::anchor_field_names()
        .into_iter()
        .filter_map(|name| {
            let key = FieldKey::parse(name)?;
            field_tab(key, config).map(|tab| (name, tab))
        })
        .collect()
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

/// Place each diagnostic of one toolpath, whose op is `config`. The
/// module doc gives the rule.
#[must_use]
pub(crate) fn place<'a>(diagnostics: &'a [Diagnostic], config: &OperationConfig) -> Placement<'a> {
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
            Anchor::Field { key } => match field_tab(key, config) {
                Some(tab) => {
                    // A Danger or Caution dot on the tab that holds the
                    // row. Efficiency findings stay quiet on the strip, as
                    // before W2b.
                    if d.category != Category::Efficiency {
                        raise_dot(&mut out.dots, tab, role_of(d));
                    }
                    out.by_field.entry(key.as_str()).or_default().push(d);
                }
                // W3f: this op draws no row for the key, so the finding
                // is an issue row.
                None => out.issues.push(d),
            },
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

    /// W3f: the tab of each anchored field follows the op. A key that the
    /// op does not draw has no tab; a drawn key is on a tab that the op
    /// shows. `properties::tests::drawn_field_rows_match_the_painted_rows_w3f`
    /// holds the rule against the painted rows.
    #[test]
    fn the_tab_of_a_field_follows_the_op_w3f() {
        use rs_cam_core::compute::operation_configs::DrillCycleType;
        let mut drawn = 0_usize;
        for op in OperationType::ALL {
            let config = OperationConfig::new_default(*op);
            for (_, tab) in drawn_fields(&config) {
                drawn += 1;
                assert!(tab.applies(&config), "{op:?} does not show {tab:?}");
            }
        }
        assert!(drawn > 100, "non-vacuity: only {drawn} drawn rows");
        let tab = |op: OperationType, n: &str| {
            field_tab(
                FieldKey::parse(n).expect("a known name"),
                &OperationConfig::new_default(op),
            )
        };
        assert_eq!(
            tab(OperationType::Pocket, "feed_rate"),
            Some(ToolpathTab::FeedsSpeeds)
        );
        assert_eq!(
            tab(OperationType::Pocket, "retract_z"),
            Some(ToolpathTab::Heights)
        );
        assert_eq!(
            tab(OperationType::Pocket, "entry_style"),
            Some(ToolpathTab::Linking)
        );
        assert_eq!(
            tab(OperationType::Adaptive3d, "entry_style"),
            Some(ToolpathTab::Passes)
        );
        assert_eq!(tab(OperationType::Trace, "entry_style"), None);
        assert_eq!(
            tab(OperationType::Pocket, "selection"),
            Some(ToolpathTab::Geometry)
        );
        assert_eq!(tab(OperationType::DropCutter, "selection"), None);
        assert_eq!(
            tab(OperationType::Pocket, "depth_per_pass"),
            Some(ToolpathTab::Passes)
        );
        assert_eq!(
            tab(OperationType::Scallop, "scallop_height"),
            Some(ToolpathTab::Passes)
        );
        assert_eq!(tab(OperationType::DropCutter, "scallop_height"), None);
        let mut simple = OperationConfig::new_default(OperationType::Drill);
        if let OperationConfig::Drill(c) = &mut simple {
            c.cycle = DrillCycleType::Simple;
        }
        let peck = FieldKey::parse("peck_depth").expect("a known name");
        assert_eq!(
            field_tab(peck, &simple),
            None,
            "a Simple cycle draws no peck row"
        );
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
            // A Drop Cutter has no selection, so no Cuts row (W3f: the op
            // decides). W2b dotted the Geometry tab for it.
            finding("b", Anchor::field("selection"), Severity::Caution),
            finding("i", Anchor::field("stepover"), Severity::Caution),
            finding("c", Anchor::Operation, Severity::Critical),
            finding("d", Anchor::Note, Severity::Info),
            stale,
            finding("f", Anchor::Project, Severity::Critical),
            finding("g", Anchor::View, Severity::Info),
            finding("h", Anchor::Operation, Severity::Hint),
        ];
        let p = place(
            &list,
            &OperationConfig::new_default(OperationType::DropCutter),
        );
        let ids = |v: &[&Diagnostic]| {
            v.iter()
                .map(|d| d.id.as_str().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&p.tool), ["a"]);
        assert_eq!(
            ids(&p.issues),
            ["b", "c"],
            "a Drop Cutter draws no Cuts row"
        );
        assert_eq!(ids(&p.notes), ["d", "h"]);
        assert_eq!(ids(&p.elsewhere), ["e", "f", "g"]);
        assert_eq!(
            p.by_field.keys().copied().collect::<Vec<_>>(),
            ["stepover"],
            "a Drop Cutter draws the stepover row"
        );
        let placed = p.tool.len()
            + p.issues.len()
            + p.notes.len()
            + p.elsewhere.len()
            + p.by_field.values().map(Vec::len).sum::<usize>();
        assert_eq!(placed, list.len(), "every finding has a place");
        assert_eq!(
            p.dots,
            vec![(ToolpathTab::Passes, crate::ui::components::Role::Caution)],
            "the drawn field dots its tab; the undrawn one does not"
        );
        assert_eq!(
            tool_role(&p.tool),
            Some(crate::ui::components::Role::Danger)
        );
    }
}
