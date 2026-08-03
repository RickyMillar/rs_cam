//! Adapter: [`ToolpathLoadVerdict`] → [`Diagnostic`] list.
//!
//! Maps the sim-backed load gates (chipload / power / deflection) and
//! the drill-specific gates (chip welding / peck adequacy / plunge
//! feed) into the unified schema. Sets `state` correctly:
//!
//! - `Current` — the gate has a `Within` or `Exceeds` verdict
//! - `NeedsSimulation` — `Unmodeled::SimulationRequired` /
//!   `Unmodeled::ArcEngagementNotCaptured` /
//!   `Unmodeled::SteadyStateSamplesNotPresent` /
//!   `Unmodeled::AllSamplesAirCutOrRapid`
//! - `StaleEvidence` — `Unmodeled::StaleSimulation`
//! - `NotApplicable` — `Unmodeled::NotApplicableForOp` *only* when the
//!   gate is a drill-cycle's milling gate paired with valid drill_gates
//!   (everything else stays `NeedsSimulation`)
//!
//! The chipload / power / deflection diagnostics declare
//! `supersedes` over the heuristic pre-sim hint family so the reducer
//! drops those once verified evidence lands.

use crate::diagnostics::evidence::EvidenceLocality;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::ids::ToolpathId;
use crate::tool_load::drill_gates::{DrillGateOutcome, DrillGateSeverity, DrillGatesVerdict};
use crate::tool_load::verdict::{
    ChipBoundsSource, ChiploadVerdict, Confidence as VerdictConfidence, DeflectionVerdict,
    PowerVerdict, ToolpathLoadVerdict, UnmodeledReason,
};

/// Translate one toolpath's load verdict into diagnostic rows.
///
/// Empty when every gate is `Unmodeled::NotApplicableForOp` *and* the
/// drill_gates surface is present — PR-1 already established that
/// pattern as "drill ops only see drill gates."
pub fn diagnostics_from_load_verdict(verdict: &ToolpathLoadVerdict) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let scope = Scope::Toolpath {
        id: verdict.toolpath_id,
    };

    // For drill ops with a drill_gates surface, milling gates that
    // report NotApplicableForOp are dropped silently. Mirrors PR-1's
    // `any_unmodeled` change.
    let suppress_milling_na = verdict.drill_gates.is_some() && all_milling_not_applicable(verdict);

    if !suppress_milling_na {
        if let Some(d) = chipload_to_diagnostic(verdict.toolpath_id, &verdict.chipload) {
            out.push(d);
        }
        if let Some(d) = power_to_diagnostic(verdict.toolpath_id, &verdict.power) {
            out.push(d);
        }
        if let Some(d) = deflection_to_diagnostic(verdict.toolpath_id, &verdict.deflection) {
            out.push(d);
        }
    }

    if let Some(drill) = verdict.drill_gates.as_ref() {
        out.extend(drill_gates_to_diagnostics(
            verdict.toolpath_id,
            drill,
            &scope,
        ));
    }

    out
}

fn all_milling_not_applicable(v: &ToolpathLoadVerdict) -> bool {
    is_not_applicable(v.chipload.unmodeled_reason())
        && is_not_applicable(v.power.unmodeled_reason())
        && is_not_applicable(v.deflection.unmodeled_reason())
}

fn is_not_applicable(reason: Option<&UnmodeledReason>) -> bool {
    matches!(reason, Some(UnmodeledReason::NotApplicableForOp(_)))
}

// ── chipload ────────────────────────────────────────────────────────

fn chipload_to_diagnostic(tp_id: ToolpathId, v: &ChiploadVerdict) -> Option<Diagnostic> {
    match v {
        ChiploadVerdict::Within {
            approach_to_max,
            burn_advisory,
            ..
        } => {
            // Emit only when there's something to say — Within rows
            // carry no severity headline, but a Current diagnostic is
            // needed so the supersession reducer can silence the
            // pre-sim heuristics.
            //
            // H4 (wave 15) — the `Within` arm is NOT always a pass.
            // F3.3 routes a genuine low-side trip here whenever the burn
            // floor's provenance is too weak to refuse on
            // (`ChipBoundsSource::low_side_is_advisory`): the verdict
            // stays `Within` by design, but it carries a `burn_advisory`
            // saying the median chip thickness sat BELOW the floor.
            // This arm used to ignore that field entirely and render
            // "Chipload within band (0.0007 mm/tooth)" beside evidence
            // reading `min 0.00458` — a message its own citation
            // contradicts on its face. The live validation of 2026-07-30
            // filed exactly that as CONCERN 1, and could not tell it from
            // a real pass.
            //
            // The VERDICT is deliberately unchanged (that is F3.3's
            // ruling, and re-litigating it is not a reporting job), as is
            // the id — `LOAD_CHIPLOAD_WITHIN` is what the supersession
            // reducer keys on to silence the pre-sim heuristics — and so
            // is the `Info` severity, because raising it would move badge
            // counts, which is a product decision and not this wave's.
            // What changes is that the message and the citation now say
            // what happened.
            let citation_metric = burn_advisory.as_deref().unwrap_or(approach_to_max);
            let message = match burn_advisory.as_deref() {
                Some(advisory) => format!(
                    "Chipload {:.4} mm/tooth is BELOW the {:.4} burn floor — not refused because \
                     the floor's provenance is {} (advisory only)",
                    advisory.observed_mm_per_tooth,
                    advisory.bounds.min_mm_per_tooth.unwrap_or(f64::NAN),
                    advisory.bounds.source.row_id(),
                ),
                None => format!(
                    "Chipload within band ({:.4} mm/tooth)",
                    approach_to_max.observed_mm_per_tooth
                ),
            };
            Some(Diagnostic {
                id: DiagnosticId::from(ids::LOAD_CHIPLOAD_WITHIN),
                scope: Scope::Toolpath { id: tp_id },
                category: Category::ToolLoad,
                severity: Severity::Info,
                confidence: chipload_confidence(&approach_to_max.bounds.source),
                state: DiagnosticState::Current,
                source: Source::ToolLoad,
                message,
                evidence: Some(DiagnosticEvidence::LutCitation {
                    // Was hard-coded `"vendor_lut"`, which named a
                    // calibrated row even when the bounds were derived.
                    row_id: citation_metric.bounds.source.row_id().to_owned(),
                    min: citation_metric.bounds.min_mm_per_tooth,
                    max: Some(citation_metric.bounds.max_mm_per_tooth),
                    observed: citation_metric.observed_mm_per_tooth,
                    unit: "mm/tooth".to_owned(),
                    extrapolated: matches!(
                        citation_metric.bounds.source,
                        ChipBoundsSource::VendorLutExtrapolated
                    ),
                }),
                fix: None,
                supersedes: chipload_supersedes(),
                suppressed_diagnostics: vec![],
            })
        }
        ChiploadVerdict::Exceeds {
            side,
            triggering,
            confidence,
        } => {
            let (id_str, msg_prefix) = match side {
                crate::tool_load::verdict::ChipSide::Low => (
                    ids::LOAD_CHIPLOAD_LOW,
                    "Chipload too low — burn / rubbing risk",
                ),
                crate::tool_load::verdict::ChipSide::High => {
                    (ids::LOAD_CHIPLOAD_HIGH, "Chipload too high — breakage risk")
                }
            };
            Some(Diagnostic {
                id: DiagnosticId::from(id_str),
                scope: Scope::Toolpath { id: tp_id },
                category: Category::ToolLoad,
                severity: Severity::Caution,
                confidence: confidence_to_kind(confidence),
                state: DiagnosticState::Current,
                source: Source::ToolLoad,
                message: format!(
                    "{msg_prefix}: {:.4} mm/tooth",
                    triggering.observed_mm_per_tooth
                ),
                evidence: Some(DiagnosticEvidence::SampleRange {
                    toolpath_id: tp_id,
                    sample_start: triggering.evidence.sample_range.start,
                    sample_end: triggering.evidence.sample_range.end,
                    observed: triggering.observed_mm_per_tooth,
                    threshold: match side {
                        crate::tool_load::verdict::ChipSide::Low => {
                            triggering.bounds.min_mm_per_tooth
                        }
                        crate::tool_load::verdict::ChipSide::High => {
                            Some(triggering.bounds.max_mm_per_tooth)
                        }
                    },
                    unit: "mm/tooth".to_owned(),
                    locality: triggering
                        .evidence
                        .locality
                        .clone()
                        .map(EvidenceLocality::new)
                        .unwrap_or_default(),
                }),
                fix: None,
                supersedes: chipload_supersedes(),
                suppressed_diagnostics: vec![],
            })
        }
        ChiploadVerdict::Unmodeled { reason } => unmodeled_to_diagnostic(
            tp_id,
            ids::LOAD_CHIPLOAD_WITHIN,
            "Chipload",
            reason,
            chipload_supersedes,
        ),
    }
}

fn chipload_supersedes() -> Vec<DiagnosticId> {
    vec![
        DiagnosticId::from(ids::FEEDS_FEED_VS_LUT_HIGH),
        DiagnosticId::from(ids::FEEDS_FEED_VS_LUT_LOW),
        DiagnosticId::from(ids::FEEDS_STEPOVER_VS_LUT),
    ]
}

fn chipload_confidence(src: &ChipBoundsSource) -> Confidence {
    match src {
        ChipBoundsSource::VendorLut => Confidence::Verified,
        // F3.3 weak-provenance sources — same demotion as extrapolated.
        ChipBoundsSource::VendorLutExtrapolated
        | ChipBoundsSource::VendorLutPointPreset
        | ChipBoundsSource::VendorLutMissingAe => Confidence::Approximate,
    }
}

// ── power ───────────────────────────────────────────────────────────

fn power_to_diagnostic(tp_id: ToolpathId, v: &PowerVerdict) -> Option<Diagnostic> {
    match v {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            evidence,
            confidence,
            ..
        } => Some(Diagnostic {
            id: DiagnosticId::from(ids::LOAD_POWER_WITHIN),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: confidence_to_kind(confidence),
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!(
                "Power within budget ({:.2}/{:.2} kW peak)",
                peak_kw, available_kw
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id: tp_id,
                sample_start: evidence.sample_range.start,
                sample_end: evidence.sample_range.end,
                observed: *peak_kw,
                threshold: Some(*available_kw),
                unit: "kW".to_owned(),
                locality: evidence
                    .locality
                    .clone()
                    .map(EvidenceLocality::new)
                    .unwrap_or_default(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        }),
        PowerVerdict::Exceeds {
            peak_kw,
            available_kw,
            evidence,
            confidence,
        } => Some(Diagnostic {
            id: DiagnosticId::from(ids::LOAD_POWER_EXCEEDS),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: confidence_to_kind(confidence),
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!(
                "Spindle power exceeded: {:.2} kW peak > {:.2} kW available",
                peak_kw, available_kw
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id: tp_id,
                sample_start: evidence.sample_range.start,
                sample_end: evidence.sample_range.end,
                observed: *peak_kw,
                threshold: Some(*available_kw),
                unit: "kW".to_owned(),
                locality: evidence
                    .locality
                    .clone()
                    .map(EvidenceLocality::new)
                    .unwrap_or_default(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        }),
        PowerVerdict::Unmodeled { reason } => {
            unmodeled_to_diagnostic(tp_id, ids::LOAD_POWER_WITHIN, "Power", reason, Vec::new)
        }
    }
}

// ── deflection ──────────────────────────────────────────────────────

fn deflection_to_diagnostic(tp_id: ToolpathId, v: &DeflectionVerdict) -> Option<Diagnostic> {
    match v {
        DeflectionVerdict::Within {
            peak_mm,
            bounds,
            evidence,
            confidence,
            ..
        } => Some(Diagnostic {
            id: DiagnosticId::from(ids::LOAD_DEFLECTION_WITHIN),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: confidence_to_kind(confidence),
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!(
                "Tip deflection within limit ({:.0} µm / {:.0} µm)",
                peak_mm * 1000.0,
                bounds.exceeds_mm * 1000.0
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id: tp_id,
                sample_start: evidence.sample_range.start,
                sample_end: evidence.sample_range.end,
                observed: *peak_mm,
                threshold: Some(bounds.exceeds_mm),
                unit: "mm".to_owned(),
                locality: evidence
                    .locality
                    .clone()
                    .map(EvidenceLocality::new)
                    .unwrap_or_default(),
            }),
            fix: None,
            supersedes: deflection_supersedes(),
            suppressed_diagnostics: vec![],
        }),
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds,
            evidence,
            confidence,
        } => Some(Diagnostic {
            id: DiagnosticId::from(ids::LOAD_DEFLECTION_EXCEEDS),
            scope: Scope::Toolpath { id: tp_id },
            category: Category::ToolLoad,
            severity: Severity::Critical,
            confidence: confidence_to_kind(confidence),
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!(
                "Tip deflection exceeds limit: {:.0} µm > {:.0} µm",
                peak_mm * 1000.0,
                bounds.exceeds_mm * 1000.0
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id: tp_id,
                sample_start: evidence.sample_range.start,
                sample_end: evidence.sample_range.end,
                observed: *peak_mm,
                threshold: Some(bounds.exceeds_mm),
                unit: "mm".to_owned(),
                locality: evidence
                    .locality
                    .clone()
                    .map(EvidenceLocality::new)
                    .unwrap_or_default(),
            }),
            fix: None,
            supersedes: deflection_supersedes(),
            suppressed_diagnostics: vec![],
        }),
        DeflectionVerdict::Unmodeled { reason } => unmodeled_to_diagnostic(
            tp_id,
            ids::LOAD_DEFLECTION_WITHIN,
            "Deflection",
            reason,
            deflection_supersedes,
        ),
    }
}

fn deflection_supersedes() -> Vec<DiagnosticId> {
    vec![
        DiagnosticId::from(ids::GEOM_DPP_OVER_1_5X_DIAMETER),
        DiagnosticId::from(ids::FEEDS_DPP_VS_LUT),
    ]
}

// ── drill gates ─────────────────────────────────────────────────────

fn drill_gates_to_diagnostics(
    tp_id: ToolpathId,
    drill: &DrillGatesVerdict,
    scope: &Scope,
) -> Vec<Diagnostic> {
    vec![
        drill_gate_to_diagnostic(
            ids::DRILL_CHIP_WELDING,
            "Chip welding (D/d)",
            &drill.chip_welding,
            scope.clone(),
            tp_id,
            "ratio",
        ),
        drill_gate_to_diagnostic(
            ids::DRILL_PECK_ADEQUACY,
            "Peck adequacy (peck/D)",
            &drill.peck_adequacy,
            scope.clone(),
            tp_id,
            "ratio",
        ),
        drill_gate_to_diagnostic(
            ids::DRILL_PLUNGE_FEED,
            "Plunge feed envelope",
            &drill.plunge_feed,
            scope.clone(),
            tp_id,
            "mm/min per mm Ø",
        ),
    ]
}

fn drill_gate_to_diagnostic(
    id: &str,
    label: &str,
    outcome: &DrillGateOutcome,
    scope: Scope,
    tp_id: ToolpathId,
    unit: &str,
) -> Diagnostic {
    match outcome {
        DrillGateOutcome::Within {
            observed,
            threshold,
            ..
        } => Diagnostic {
            id: DiagnosticId::from(id),
            scope,
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!("{label} within ({observed:.2})"),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: label.to_owned(),
                lhs_value: *observed,
                rhs_label: "threshold".to_owned(),
                rhs_value: *threshold,
                unit: unit.to_owned(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity,
            ..
        } => Diagnostic {
            id: DiagnosticId::from(id),
            scope,
            category: Category::ToolLoad,
            severity: match severity {
                DrillGateSeverity::Elevated => Severity::Caution,
                DrillGateSeverity::Critical => Severity::Critical,
            },
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!("{label} exceeds: {observed:.2} vs {threshold:.2}"),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id: tp_id,
                sample_start: 0,
                sample_end: 0,
                observed: *observed,
                threshold: Some(*threshold),
                unit: unit.to_owned(),
                locality: EvidenceLocality::default(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
    }
}

// ── shared ──────────────────────────────────────────────────────────

fn confidence_to_kind(c: &VerdictConfidence) -> Confidence {
    match c {
        VerdictConfidence::Validated => Confidence::Verified,
        VerdictConfidence::Approximate(_) => Confidence::Approximate,
    }
}

fn unmodeled_to_diagnostic(
    tp_id: ToolpathId,
    id: &str,
    label: &str,
    reason: &UnmodeledReason,
    supersedes_fn: fn() -> Vec<DiagnosticId>,
) -> Option<Diagnostic> {
    // PR-7 polish: every Unmodeled reason gets a user-actionable
    // message rather than a flat "needs current simulation".
    let (state, msg) = match reason {
        UnmodeledReason::SimulationRequired => (
            DiagnosticState::NeedsSimulation,
            format!("{label}: run simulation to evaluate"),
        ),
        UnmodeledReason::ArcEngagementNotCaptured => (
            DiagnosticState::NeedsSimulation,
            format!(
                "{label}: simulation captured without arc engagement — re-run with \
                 metrics enabled"
            ),
        ),
        UnmodeledReason::SteadyStateSamplesNotPresent => (
            DiagnosticState::NeedsSimulation,
            format!(
                "{label}: not enough steady-state cutting samples to evaluate \
                 (toolpath may be all entry/exit ramps)"
            ),
        ),
        UnmodeledReason::AllSamplesAirCutOrRapid => (
            DiagnosticState::NeedsSimulation,
            format!(
                "{label}: every sample is air-cut or rapid — verify stock alignment \
                 and depth"
            ),
        ),
        UnmodeledReason::StaleSimulation => (
            DiagnosticState::StaleEvidence,
            format!("{label}: simulation stale — re-run to verify"),
        ),
        UnmodeledReason::NoVendorData => (
            DiagnosticState::NeedsSimulation,
            format!(
                "{label}: no vendor LUT row matches this tool/material — supply \
                 vendor data to enable this gate"
            ),
        ),
        UnmodeledReason::MaterialUnvalidated => (
            DiagnosticState::NeedsSimulation,
            format!(
                "{label}: material has no validated Kc — set a validated material \
                 on the stock to model this"
            ),
        ),
        UnmodeledReason::CutterModeUnsupported(detail) => (
            DiagnosticState::NotApplicable,
            format!("{label}: cutter mode unsupported ({detail})"),
        ),
        UnmodeledReason::NotImplemented(detail) => (
            DiagnosticState::NeedsSimulation,
            format!("{label}: model deferred ({detail})"),
        ),
        UnmodeledReason::NotApplicableForOp(_) => return None,
    };
    Some(Diagnostic {
        id: DiagnosticId::from(id),
        scope: Scope::Toolpath { id: tp_id },
        category: Category::State,
        severity: Severity::Info,
        confidence: Confidence::Static,
        state,
        source: Source::ToolLoad,
        message: msg,
        // No `supersedes` when the diagnostic is non-Current; the
        // reducer is state-gated. But we still attach the family so
        // the consumer can render a "would supersede" hint if needed.
        // Reducer ignores it because state != Current.
        evidence: None,
        fix: None,
        supersedes: supersedes_fn(),
        suppressed_diagnostics: vec![],
    })
}
