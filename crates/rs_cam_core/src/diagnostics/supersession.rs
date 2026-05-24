//! Supersession reducer: drop heuristic / pre-sim hints when a
//! verified diagnostic covers the same dimension.
//!
//! Each [`super::Diagnostic`] declares the rule IDs it supersedes via
//! [`super::Diagnostic::supersedes`]. The reducer applies one rule:
//!
//! > A diagnostic D is dropped iff some other Current-state
//! > diagnostic A has `D.id ∈ A.supersedes`.
//!
//! The state-gated condition is deliberate: a stale or
//! needs-simulation chipload verdict should not suppress the pre-sim
//! chipload-vs-LUT hints — the operator needs the preflight nudge
//! while the rigorous evidence is missing. Only Current evidence
//! carries enough authority to silence its heuristic shadow.
//!
//! Severity of the superseder is irrelevant: a Current
//! `load.chipload.low` (Caution, exceeds) silences the same preflight
//! family that a Current `load.chipload.within` (Info, ok) would
//! silence. Either way, the operator is reading the rigorous source.

use std::collections::HashSet;

use super::{Diagnostic, DiagnosticId, DiagnosticState};

/// Drop diagnostics whose `id` is listed in any Current-state
/// diagnostic's `supersedes` field. Surviving Current-state
/// diagnostics get a populated [`Diagnostic::suppressed_diagnostics`]
/// listing the IDs they actually silenced — empty when no shadow
/// heuristics were present.
///
/// Stable wrt input order: surviving diagnostics keep their relative
/// positions. Idempotent: `apply_supersession(apply_supersession(xs))
/// == apply_supersession(xs)`.
pub fn apply_supersession(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    // Build the set of IDs present in the input — used to populate
    // `suppressed_diagnostics` only with ids that actually existed
    // before this pass (no false-positive "would have hidden X").
    let present_ids: HashSet<String> = diagnostics.iter().map(|d| d.id.0.clone()).collect();

    // Build the set of IDs that an active diagnostic claims to
    // supersede. These are the IDs that will be dropped.
    let mut superseded_ids: HashSet<String> = HashSet::new();
    for d in &diagnostics {
        if d.state == DiagnosticState::Current {
            for sid in &d.supersedes {
                superseded_ids.insert(sid.0.clone());
            }
        }
    }

    if superseded_ids.is_empty() {
        return diagnostics;
    }

    diagnostics
        .into_iter()
        .filter_map(|mut d| {
            if superseded_ids.contains(&d.id.0) {
                return None;
            }
            // For surviving Current-state diagnostics, record the
            // IDs they actually superseded so debug consumers can
            // explain a missing heuristic. Only include IDs that were
            // present in the input — supersedes lists declare what
            // *could* be silenced, not what *was*.
            if d.state == DiagnosticState::Current && !d.supersedes.is_empty() {
                let suppressed: Vec<DiagnosticId> = d
                    .supersedes
                    .iter()
                    .filter(|sid| present_ids.contains(&sid.0))
                    .cloned()
                    .collect();
                if !suppressed.is_empty() {
                    d.suppressed_diagnostics = suppressed;
                }
            }
            Some(d)
        })
        .collect()
}
