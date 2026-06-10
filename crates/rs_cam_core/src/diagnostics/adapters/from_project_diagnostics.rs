//! Adapter: [`crate::session::ProjectDiagnostics`] → [`Diagnostic`]
//! list.
//!
//! Maps project-wide verdicts (collisions, plunge stress, air-cut
//! high, generated-empty) into the unified schema. The legacy
//! [`crate::session::VerdictSeverity`] ladder
//! (`Critical < Important < Polish`) maps cleanly to
//! [`Severity`]:
//!
//! - `Critical` (holder/rapid collision) → `Critical`
//! - `Important` (plunge stress, generated-empty) → `Caution`
//! - `Polish` (air-cut high, slow cycle) → `Hint`

use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::session::{ProjectDiagnostics, Verdict, VerdictKind, VerdictSeverity};

pub fn diagnostics_from_project(diag: &ProjectDiagnostics) -> Vec<Diagnostic> {
    diag.verdicts.iter().map(verdict_to_diagnostic).collect()
}

fn verdict_to_diagnostic(v: &Verdict) -> Diagnostic {
    let (id, category) = id_and_category_for(v.kind);
    let evidence = if v.offender_toolpath_ids.is_empty() {
        v.evidence
            .move_index
            .map(|move_index| DiagnosticEvidence::Move {
                toolpath_id: crate::ids::ToolpathId(0),
                move_index,
                position: None,
            })
    } else {
        Some(DiagnosticEvidence::Counts {
            count: v.evidence.count.unwrap_or(v.offender_toolpath_ids.len()),
            offender_toolpath_ids: v.offender_toolpath_ids.clone(),
        })
    };
    let scope = match v.offender_toolpath_ids.as_slice() {
        [only] => Scope::Toolpath { id: *only },
        _ => Scope::Project,
    };
    let message = if v.fix_hint.is_empty() {
        v.headline.clone()
    } else {
        format!("{} — {}", v.headline, v.fix_hint)
    };
    Diagnostic {
        id: DiagnosticId::from(id),
        scope,
        category,
        severity: severity_from_legacy(v.severity),
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::Simulation,
        message,
        evidence,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }
}

fn severity_from_legacy(s: VerdictSeverity) -> Severity {
    match s {
        VerdictSeverity::Critical => Severity::Critical,
        VerdictSeverity::Important => Severity::Caution,
        VerdictSeverity::Polish => Severity::Hint,
    }
}

fn id_and_category_for(kind: VerdictKind) -> (&'static str, Category) {
    match kind {
        VerdictKind::HolderCollision => (ids::PROJECT_HOLDER_COLLISION, Category::Safety),
        VerdictKind::RapidCollision => (ids::PROJECT_RAPID_COLLISION, Category::Safety),
        VerdictKind::PlungeStress => (ids::PROJECT_PLUNGE_STRESS, Category::Safety),
        VerdictKind::AirCut => (ids::PROJECT_AIR_CUT_HIGH, Category::Efficiency),
        VerdictKind::GeneratedEmpty => (ids::PROJECT_GENERATED_EMPTY, Category::State),
    }
}
