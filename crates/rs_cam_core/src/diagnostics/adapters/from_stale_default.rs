//! Adapter: [`crate::compute::validate::StaleDefault`] → [`Diagnostic`]
//! list.
//!
//! Stale-default defects carry their own per-rule `id`, title,
//! detail, current/new values, and a canonical fix path
//! ([`crate::compute::validate::apply_stale_default_fix`]). The
//! adapter wraps each into a `Caution`-severity diagnostic with an
//! [`crate::diagnostics::DiagnosticFix::ApplyStaleDefault`] payload
//! so consumers can render a one-click Fix button.

use crate::compute::validate::{StaleDefault, StaleDefaultRule};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticFix, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};

pub fn diagnostics_from_stale_defaults(defects: &[StaleDefault]) -> Vec<Diagnostic> {
    defects.iter().map(stale_default_to_diagnostic).collect()
}

fn stale_default_to_diagnostic(d: &StaleDefault) -> Diagnostic {
    let id = id_for_rule(d.rule_id);
    Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: d.toolpath_id },
        category: Category::Safety,
        severity: Severity::Caution,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::StaleDefault,
        message: format!("{} — {}", d.title, d.detail),
        evidence: None,
        fix: Some(DiagnosticFix::ApplyStaleDefault {
            rule_id: rule_id_string(d.rule_id),
            toolpath_id: d.toolpath_id,
            new_value: d.new_value,
        }),
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }
}

fn id_for_rule(rule: StaleDefaultRule) -> &'static str {
    match rule {
        StaleDefaultRule::DropCutterMinZPreB1 => ids::STALE_DROP_CUTTER_MIN_Z,
        StaleDefaultRule::TaperedBallPlungePreFix2 => ids::STALE_TAPERED_BALL_PLUNGE,
        StaleDefaultRule::WoodAdaptiveStepoverPreFix1 => ids::STALE_WOOD_ADAPTIVE_STEPOVER,
        StaleDefaultRule::ProjectCurveNegativeDepth => ids::STALE_PROJECT_CURVE_NEGATIVE_DEPTH,
    }
}

fn rule_id_string(rule: StaleDefaultRule) -> String {
    // Re-use the existing canonical id() so a downstream consumer
    // matching on `legacy_rule_id` can branch identically against
    // either source.
    rule.id().to_owned()
}
