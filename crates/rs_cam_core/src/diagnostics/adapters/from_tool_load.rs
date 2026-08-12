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
        if let Some(d) = chipload_to_diagnostic(
            verdict.toolpath_id,
            &verdict.chipload,
            verdict.feed_explanation.as_deref(),
        ) {
            out.push(d);
        }
        // T1.5 (census P-10) — the only same-unit comparison the pipeline
        // can make, and the only one nothing surfaced.
        if let Some(d) = commanded_above_band_to_diagnostic(
            verdict.toolpath_id,
            verdict.feed_explanation.as_deref(),
        ) {
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
        // The toolpath identity rides on `scope`; drill diagnostics
        // carry no per-sample evidence to attribute (R-7).
        out.extend(drill_gates_to_diagnostics(drill, &scope));
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

/// T1.2 — the qualifier every chipload message needs before it quotes a
/// number: which statistic, in which unit, and what separates it from
/// the commanded value an operator actually set.
///
/// Pre-T1.2 the messages said "Chipload ... mm/tooth", which named
/// neither the statistic nor the unit and read as if it were the same
/// quantity as the commanded feed-per-tooth. At the time it was not: it
/// was an arc-mean CHIP thickness renormalised to the matched row's
/// nominal arc and evaluated at the kinematically-predicted feed, and on
/// the live 2026-07-30 operation those two multipliers accounted for a
/// 97× difference with nothing on screen mentioning either.
///
/// Since the 2026-08-06 unit conversion it **is** the same quantity as
/// the commanded feed-per-tooth, evaluated at the achieved feed
/// (`tool_load::chipload`'s header). The qualifier stays, and stays
/// mandatory: it now discloses the one remaining multiplier, which is
/// the kinematic throttle — the fact the verdict document calls
/// operator-actionable. An operator reading "0.0092 mm/tooth, Within"
/// beside a commanded 0.0714 is being told something real, and the
/// clause is where it gets told.
///
/// Empty when no explanation is available, so the message degrades to
/// its old shape rather than asserting something unmeasured.
fn observed_qualifier(explanation: Option<&crate::feeds::FeedExplanation>) -> String {
    match explanation {
        // Checkpoint K (d2) — say when the commanded number was PLACED by
        // the engine rather than chosen. Renderer 1 of 3.
        Some(e) => format!(
            " [{}, {}; commanded {:.4} mm/tooth advance{}, {}]",
            e.gate.statistic.label(),
            e.gate.unit(),
            e.commanded.feed_per_tooth_mm,
            e.commanded
                .clamped_to
                .map(|c| format!(" — {}", c.label()))
                .unwrap_or_default(),
            e.multiplier_clause(),
        ),
        None => String::new(),
    }
}

/// T1.6 — row provenance on EVERY verdict, not only extrapolated ones.
///
/// Pre-T1.6 a row scaled 0.99× and a row scaled 0.38× both reported as
/// `vendor_lut`, and a `semi_finish` row winning a `finish` query was
/// invisible. Both are reported here; neither changes a verdict.
fn row_provenance_clause(explanation: Option<&crate::feeds::FeedExplanation>) -> String {
    let Some(e) = explanation else {
        return String::new();
    };
    let mut parts = vec![format!("row {}", e.band.observation_id)];
    if e.band.is_scaled() {
        parts.push(format!(
            "scaled ×{:.4} diameter × ×{:.4} hardness from Ø{:.3} mm",
            e.band.diameter_scale, e.band.hardness_scale, e.band.row_diameter_mm
        ));
    }
    if e.band.pass_role_substituted() {
        parts.push(format!(
            "pass role {:?} substituted for the requested {:?}",
            e.band.row_pass_role, e.band.queried_pass_role
        ));
    }
    format!(" ({})", parts.join("; "))
}

/// T1.5 (census P-10) — the commanded feed-per-tooth against the matched
/// band's maximum.
///
/// Both sides are a linear advance per tooth: this is the one comparison
/// in the whole chipload pipeline that needs no unit conversion and no
/// stage caveat, and it was the only one nothing surfaced.
/// `narrate.rs:426` computed the left-hand side and printed it beside
/// nothing; the right-hand side sat on the verdict.
///
/// Severity `Info` per the standing B6 ruling: this reports a COMMANDED
/// value against an AUTHORED band. It observes nothing about the cut, so
/// it must not compete with the sim-backed gates for operator attention.
fn commanded_above_band_to_diagnostic(
    tp_id: ToolpathId,
    explanation: Option<&crate::feeds::FeedExplanation>,
) -> Option<Diagnostic> {
    let e = explanation?;
    let ratio = e.commanded_over_band_max()?;
    if ratio <= 1.0 {
        return None;
    }
    Some(Diagnostic {
        id: DiagnosticId::from(ids::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND),
        scope: Scope::Toolpath { id: tp_id },
        category: Category::ToolLoad,
        severity: Severity::Info,
        // The band's own provenance decides how much this is worth:
        // an extrapolated or point-preset row is a weaker yardstick.
        confidence: chipload_confidence(&e.band.bounds_source),
        state: DiagnosticState::Current,
        source: Source::ToolLoad,
        message: format!(
            "Commanded feed-per-tooth {:.4} mm/tooth is {ratio:.1}× the matched \
             band maximum {:.4} mm/tooth — same unit, same stage, so this \
             comparison needs no conversion{}",
            e.commanded.feed_per_tooth_mm,
            e.band.max_mm_per_tooth,
            row_provenance_clause(explanation),
        ),
        evidence: Some(DiagnosticEvidence::LutCitation {
            row_id: e.band.bounds_source.row_id().to_owned(),
            min: e.band.min_mm_per_tooth,
            max: Some(e.band.max_mm_per_tooth),
            observed: e.commanded.feed_per_tooth_mm,
            unit: "mm/tooth".to_owned(),
            extrapolated: e.band.is_extrapolated,
        }),
        fix: None,
        // Deliberately supersedes nothing: this is a commanded-vs-authored
        // report and must not silence a sim-backed gate.
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    })
}

fn chipload_to_diagnostic(
    tp_id: ToolpathId,
    v: &ChiploadVerdict,
    explanation: Option<&crate::feeds::FeedExplanation>,
) -> Option<Diagnostic> {
    match v {
        ChiploadVerdict::Within {
            approach_to_max,
            burn_advisory,
            ceiling_advisory,
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
            let citation_metric = burn_advisory
                .as_deref()
                .or(ceiling_advisory.as_deref())
                .unwrap_or(approach_to_max);
            // T1.2 — name the statistic, name the unit, and name the two
            // multipliers that separate this number from the commanded
            // one. See `observed_qualifier` for why a bare "Chipload
            // ... mm/tooth" was actively misleading.
            let qualifier = observed_qualifier(explanation);
            let provenance = row_provenance_clause(explanation);
            // Checkpoint K (c2) — a recipe the engine's own rubbing-floor
            // clamp parked ON the band ceiling reads *clamped*, not
            // *within* and certainly not *exceeds*. The three
            // possibilities are mutually exclusive by construction: the
            // burn advisory is a low-side reading, the ceiling advisory a
            // high-side one.
            let message = match (burn_advisory.as_deref(), ceiling_advisory.as_deref()) {
                (Some(advisory), _) => format!(
                    "Observed feed-per-tooth {:.4} mm is BELOW the {:.4} mm/tooth burn floor \
                     — not refused because the floor's provenance is {} (advisory only)\
                     {qualifier}{provenance}",
                    advisory.observed_mm_per_tooth,
                    advisory.bounds.min_mm_per_tooth.unwrap_or(f64::NAN),
                    advisory.bounds.source.row_id(),
                ),
                (None, Some(advisory)) => format!(
                    "Observed feed-per-tooth {:.4} mm is ON the {:.4} mm/tooth band ceiling — \
                     CLAMPED there by the engine's own rubbing-floor rule, not exceeded. The \
                     whole derated band sits below the chip-formation floor, so no feed \
                     clears rubbing without leaving the band; expect burnishing\
                     {qualifier}{provenance}",
                    advisory.observed_mm_per_tooth, advisory.bounds.max_mm_per_tooth,
                ),
                (None, None) => format!(
                    "Observed feed-per-tooth within band ({:.4} mm){qualifier}{provenance}",
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
                    "Feed-per-tooth too low — burn / rubbing risk",
                ),
                crate::tool_load::verdict::ChipSide::High => (
                    ids::LOAD_CHIPLOAD_HIGH,
                    "Feed-per-tooth too high — breakage risk",
                ),
            };
            // T1.2 — same qualifier on the trip arm as on the Within
            // arm; an `Exceeds` message that hides its stage is worse
            // than a `Within` one, because it prompts an action.
            let qualifier = observed_qualifier(explanation);
            let provenance = row_provenance_clause(explanation);
            Some(Diagnostic {
                id: DiagnosticId::from(id_str),
                scope: Scope::Toolpath { id: tp_id },
                category: Category::ToolLoad,
                severity: Severity::Caution,
                confidence: confidence_to_kind(confidence),
                state: DiagnosticState::Current,
                source: Source::ToolLoad,
                message: format!(
                    "{msg_prefix}: {:.4} mm{qualifier}{provenance}",
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

fn drill_gates_to_diagnostics(drill: &DrillGatesVerdict, scope: &Scope) -> Vec<Diagnostic> {
    vec![
        drill_gate_to_diagnostic(
            ids::DRILL_CHIP_WELDING,
            "Chip welding (D/d)",
            &drill.chip_welding,
            scope.clone(),
            "ratio",
            drill.worst_hole_id,
        ),
        drill_gate_to_diagnostic(
            ids::DRILL_PECK_ADEQUACY,
            "Peck adequacy (peck/D)",
            &drill.peck_adequacy,
            scope.clone(),
            "ratio",
            drill.worst_hole_id,
        ),
        drill_gate_to_diagnostic(
            ids::DRILL_PLUNGE_FEED,
            "Plunge feed envelope",
            &drill.plunge_feed,
            scope.clone(),
            "mm/min per mm Ø",
            // Hole-independent by construction (`feed / diameter`) —
            // attributing it to a hole would be a second fabrication.
            None,
        ),
    ]
}

/// Render the band a `Within` reading sits in, when it adds anything
/// the headline number does not already say.
fn band_suffix(lo: Option<f64>, hi: Option<f64>, threshold: f64) -> String {
    match (lo, hi) {
        // Two-sided (plunge feed): the full envelope is the useful
        // context, and `threshold` is only the nearer edge of it.
        (Some(lo), Some(hi)) => format!("; band {lo:.2}–{hi:.2}"),
        // One-sided-up with the ceiling already in the headline.
        (None, Some(hi)) if (hi - threshold).abs() < 1e-9 => String::new(),
        (None, Some(hi)) => format!("; band up to {hi:.2}"),
        (Some(lo), None) => format!("; band from {lo:.2}"),
        (None, None) => String::new(),
    }
}

/// R-1 — word the relation the outcome actually describes.
///
/// One `Exceeds` variant carries three distinct relations, and the
/// shipped message asserted the same one for all three:
///
/// - *below the floor* (plunge feed, `Elevated`) — shipped as
///   `Plunge feed envelope exceeds: 16.67 vs 50.00`, where the number
///   is the genuinely violated bound but the verb points the wrong way;
/// - *inside the advisory band, below the ceiling* (chip welding,
///   `Elevated`) — shipped as `Chip welding (D/d) exceeds: 7.33 vs
///   8.00`, at `Severity::Caution`, about a value **below** the bound
///   it was said to exceed. Nothing was violated;
/// - *above the ceiling* (`Critical`) — the only case the shipped
///   wording was right about.
///
/// The relation is recoverable without new state: compare `observed`
/// against `threshold`. Below it, this is either a floor violation
/// (there is a lower band edge, so the bound is a floor) or an
/// approach to a ceiling.
fn drill_exceedance_message(
    label: &str,
    observed: f64,
    threshold: f64,
    severity: DrillGateSeverity,
    envelope_lo: Option<f64>,
    envelope_hi: Option<f64>,
    unit: &str,
) -> String {
    if observed >= threshold {
        // Genuine upward exceedance — the bound was crossed.
        return format!("{label} exceeds: {observed:.2} vs {threshold:.2} {unit}");
    }
    // `observed < threshold`. If the outcome names a band below the
    // observation's bound, the bound being reported is a FLOOR and the
    // reading fell under it.
    let below_a_floor = envelope_lo.is_some_and(|lo| observed < lo);
    if below_a_floor {
        let ceiling = envelope_hi
            .map(|hi| format!(", envelope {threshold:.2}–{hi:.2}"))
            .unwrap_or_default();
        return format!("{label} below minimum: {observed:.2} vs {threshold:.2} {unit}{ceiling}");
    }
    // Advisory band: approaching the bound, not over it. Name the band
    // it entered so the reader can see how much room is left.
    let entered = envelope_lo
        .map(|lo| format!(" (advisory band from {lo:.2})"))
        .unwrap_or_default();
    let verb = match severity {
        DrillGateSeverity::Elevated => "approaching",
        // Defensive: a Critical below its own bound is not a shape this
        // gate produces. Say what is true rather than inventing a
        // violation.
        DrillGateSeverity::Critical => "under review against",
    };
    format!("{label} {verb} limit: {observed:.2} of {threshold:.2} {unit}{entered}")
}

fn drill_gate_to_diagnostic(
    id: &str,
    label: &str,
    outcome: &DrillGateOutcome,
    scope: Scope,
    unit: &str,
    worst_hole_id: Option<usize>,
) -> Diagnostic {
    // R-7: drill gates are closed-form ratios over config + material.
    // The honest evidence for one is the comparison itself, plus the
    // hole it is about when the verdict is depth-derived — never a
    // `SampleRange`, which asserts a location in the sample stream that
    // was not measured.
    let hole_clause = worst_hole_id
        .map(|h| format!(" [deepest hole #{}]", h + 1))
        .unwrap_or_default();
    match outcome {
        DrillGateOutcome::Within {
            observed,
            threshold,
            envelope_lo,
            envelope_hi,
        } => Diagnostic {
            id: DiagnosticId::from(id),
            scope,
            category: Category::ToolLoad,
            severity: Severity::Info,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::ToolLoad,
            message: format!(
                "{label} within ({observed:.2} of {threshold:.2}{})",
                band_suffix(*envelope_lo, *envelope_hi, *threshold)
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: label.to_owned(),
                lhs_value: *observed,
                // R-3: name what this bound IS. For chip welding it is
                // the advisory boundary the verdict was decided at, not
                // the material threshold a band further out.
                rhs_label: "bound".to_owned(),
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
            envelope_lo,
            envelope_hi,
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
            message: format!(
                "{}{hole_clause}",
                drill_exceedance_message(
                    label,
                    *observed,
                    *threshold,
                    *severity,
                    *envelope_lo,
                    *envelope_hi,
                    unit,
                )
            ),
            evidence: Some(DiagnosticEvidence::GeometryCompare {
                lhs_label: label.to_owned(),
                lhs_value: *observed,
                rhs_label: "bound".to_owned(),
                rhs_value: *threshold,
                unit: unit.to_owned(),
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
