//! The outcome narrative of an `OptimizeOutcome`.
//!
//! A1 of `planning/OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md` made
//! the free `explanation: String` into typed fields that the GUI and
//! an MCP client render.
//!
//! [`OutcomeNarrative`] attaches to every [`super::OutcomeKind`]. A
//! field that does not apply to a kind stays empty and is not written
//! to the wire.
//!
//! The builders here are pure functions over the attempted candidate
//! set; `rank_candidates` calls them.

use serde::{Deserialize, Serialize};

use crate::tool_load::verdict::{
    ChipSide, ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
};

use super::OptimizeCandidate;

/// The narrative of every [`super::OptimizeOutcome`] kind. Roadmap F.7
/// made this one shape, so MCP agents and the GUI modal render every
/// outcome the same way.
///
/// The fields each kind fills:
///
/// - **Ranked**: `headline` + `envelope`.
/// - **NoSafeImprovement**: `headline`, `explanation`, `envelope`,
///   `limiting_gates` (the exceeded rows of the fastest candidate).
/// - **Skipped**: empty (the search evaluated no candidate).
///
/// `unmodeled` and `refused_cells` can fill on `Ranked` and on
/// `NoSafeImprovement`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutcomeNarrative {
    /// One-line headline for the modal, in operator language.
    pub headline: String,
    /// The text the modal shows under the headline. Empty when the
    /// kind needs none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub explanation: String,
    /// The extent of each parameter across the attempted candidates, so
    /// the GUI can say "we tried feeds up to X mm/min" without a second
    /// computation.
    #[serde(default)]
    pub envelope: SearchEnvelopeReached,
    /// The exceeded rows of the fastest non-baseline candidate. Only
    /// `NoSafeImprovement` fills it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limiting_gates: Vec<LimitingGate>,
    /// The statement when the stock has no force line: "Power and
    /// deflection are not modelled: <reason>; stock source
    /// `<source_id>`". The search then ran on the bounds that remain.
    /// `None` when a force line exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmodeled: Option<String>,
    /// One line for each cell the search did not simulate: the
    /// calculator refused it or moved its depth, or its evaluation
    /// failed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused_cells: Vec<String>,
}

/// One exceeded criterion row of a candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitingGate {
    pub gate: GateKind,
    /// Only the chipload row has a side (Low = burn, High = breakage).
    /// `None` for the other rows.
    pub side: Option<ChipSide>,
    /// Observed value at the limiting sample.
    pub observed: f64,
    /// The bound the gate compared the observation with. Chipload Low:
    /// `min_mm_per_tooth`; chipload High: `max_mm_per_tooth`; power:
    /// `available_kw`; deflection: `bounds.exceeds_mm`; depth: the
    /// rigidity cap in mm.
    pub bound: f64,
    /// Signed fractional overshoot: `(observed - bound) / bound`.
    /// Positive above the bound, negative below it (chipload Low).
    pub overshoot_fraction: f64,
    /// Always `false`. The optimizer judges with the strict bands
    /// (`ToleranceBands::default()`), so no `Within` reading is past
    /// its bound. The field stays on the wire for the GUI and MCP
    /// readers.
    pub band_admitted: bool,
    /// G17 A3 — short operator label for the locality of the
    /// triggering sample: "slot section", "heavy engagement", "plunge
    /// entry", "helix entry". `None` for a steady-state sample or when
    /// the simulation did not capture the arc engagement. The source is
    /// `SampleEvidence.locality`, which the gates fill through
    /// [`crate::tool_load::locality::classify_sample_locality`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
}

/// The criterion row a [`LimitingGate`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    Chipload,
    Power,
    Deflection,
    /// The depth-of-cut row. Its bound is a rule of thumb, so the row
    /// is reported and never refuses an export (S3).
    Depth,
}

/// The extent of each parameter across the attempted candidates. A field
/// is `Some` when at least one candidate (or the baseline) carries that
/// parameter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchEnvelopeReached {
    pub feed_mm_min: Option<AxisExtent>,
    pub spindle_rpm: Option<AxisExtent>,
    pub stepover_mm: Option<AxisExtent>,
    pub depth_per_pass_mm: Option<AxisExtent>,
    pub scallop_height_mm: Option<AxisExtent>,
}

/// The minimum and maximum of one parameter across the attempted set.
/// `min == max` when the search tried only one value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AxisExtent {
    pub min: f64,
    pub max: f64,
}

/// The narrative of an outcome whose operation has no lever: no depth
/// per pass to change and links already on.
pub(crate) fn build_no_search_narrative(baseline: &OptimizeCandidate) -> OutcomeNarrative {
    OutcomeNarrative {
        headline: super::NO_SEARCH_HEADLINE.to_owned(),
        explanation: super::NO_SEARCH_EXPLANATION.to_owned(),
        envelope: envelope_across(baseline, &[]),
        ..OutcomeNarrative::default()
    }
}

/// The narrative of a `Ranked` outcome: the headline, the envelope, and a
/// note when the recommended candidate's depth row exceeds its rule of
/// thumb.
/// `ordered[0]` is the baseline and `ordered[1]` the recommended
/// candidate. `attempted` is the count of simulated candidates.
pub(crate) fn build_ranked_narrative(
    ordered: &[OptimizeCandidate],
    attempted: usize,
) -> OutcomeNarrative {
    let Some((baseline, rest)) = ordered.split_first() else {
        return OutcomeNarrative::default();
    };
    let envelope = envelope_across(baseline, rest);
    let headline = match rest.first() {
        Some(best) if baseline.cycle_time_s > 0.0 => {
            let delta_pct =
                (best.cycle_time_s - baseline.cycle_time_s) / baseline.cycle_time_s * 100.0;
            format!(
                "Tried {attempted} {}. The fastest safe full cut takes {:.0} s against {:.0} s \
                 ({delta_pct:+.1} %).",
                candidates_word(attempted),
                best.cycle_time_s,
                baseline.cycle_time_s,
            )
        }
        _ => format!("Tried {attempted} {}.", candidates_word(attempted)),
    };
    // The recommended candidate can carry a row over a bound that does
    // not refuse an export (the depth row, S3). Say so, with the reading,
    // so the operator does not read a red badge with no text (F1).
    let explanation = rest
        .first()
        .map(|best| {
            limiting_gates_for_verdict(&best.verdict)
                .iter()
                .filter(|g| g.gate == GateKind::Depth)
                .map(|g| {
                    format!(
                        "The recommended candidate's depth of cut {:.2} mm is over the {:.2} mm \
                         rule of thumb (reported, not a limit).",
                        g.observed, g.bound
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    OutcomeNarrative {
        headline,
        explanation,
        envelope,
        ..OutcomeNarrative::default()
    }
}

/// The narrative of a `NoSafeImprovement` outcome after a search.
/// `attempted[0]` is the baseline. `limiting_gates` are the exceeded
/// rows of the fastest non-baseline candidate, and the explanation names
/// why that candidate is not recommended: a row that refuses an export,
/// its full-cut verdict, or a cycle time that is not shorter.
pub(crate) fn build_no_safe_narrative(attempted: &[OptimizeCandidate]) -> OutcomeNarrative {
    let Some((baseline, rest)) = attempted.split_first() else {
        return OutcomeNarrative::default();
    };
    let envelope = envelope_across(baseline, rest);
    let fastest = rest
        .iter()
        .min_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));
    let limiting_gates = fastest
        .map(|c| limiting_gates_for_verdict(&c.verdict))
        .unwrap_or_default();
    let headline = headline_no_safe(rest.len(), &limiting_gates);
    let base = NO_SAFE_BASE;
    let explanation = match fastest {
        None => format!("{base}. No cell reached a simulation; the refused cells name the cause."),
        Some(c) => {
            let mut why: Vec<String> = Vec::new();
            let criteria = c.verdict.criteria();
            let refusing: Vec<&str> = crate::gcode::refusing_exceedances(&criteria)
                .iter()
                .map(|row| row.kind.label())
                .collect();
            if !refusing.is_empty() {
                why.push(format!(
                    "its {} row refuses an export",
                    refusing.join(" and ")
                ));
            }
            if let Some(reason) = c.full_cut.verdict.reason() {
                let lead = if c.full_cut.verdict.holds() {
                    ""
                } else if matches!(
                    c.full_cut.verdict,
                    super::full_cut::FullCutVerdict::NotMeasured { .. }
                ) {
                    "the full cut is not proved, "
                } else {
                    "the full-cut check fails: "
                };
                why.push(format!("{lead}{reason}"));
            }
            if c.cycle_time_s >= baseline.cycle_time_s {
                why.push(format!(
                    "it takes {:.0} s against {:.0} s on the baseline",
                    c.cycle_time_s, baseline.cycle_time_s
                ));
            }
            if why.is_empty() {
                format!("{base}.")
            } else {
                format!(
                    "{base}. The fastest candidate ({:.0} s): {}.",
                    c.cycle_time_s,
                    why.join("; ")
                )
            }
        }
    };
    OutcomeNarrative {
        headline,
        explanation,
        envelope,
        limiting_gates,
        ..OutcomeNarrative::default()
    }
}

/// The first sentence of a `NoSafeImprovement` explanation after a
/// search. It names the three conditions of a recommendation.
const NO_SAFE_BASE: &str = "No candidate was faster than the baseline, free of a criterion row \
     that refuses an export, and a full cut";

fn candidates_word(count: usize) -> &'static str {
    if count == 1 {
        "candidate"
    } else {
        "candidates"
    }
}

/// Every `Exceeds` row of one candidate's verdict: chipload, power,
/// deflection and depth of cut. Returns an empty `Vec` when no row
/// exceeds. The GUI uses it for the "what stopped this candidate" badge
/// of each row.
///
/// A depth row is in the list when it exceeds, but it does not make the
/// candidate unsafe: its bound is a rule of thumb (S3).
pub fn limiting_gates_for_verdict(verdict: &ToolpathLoadVerdict) -> Vec<LimitingGate> {
    let mut out = Vec::new();
    if let ChiploadVerdict::Exceeds {
        side, triggering, ..
    } = &verdict.chipload
    {
        let bound = match side {
            ChipSide::High => triggering.bounds.max_mm_per_tooth,
            ChipSide::Low => triggering.bounds.min_mm_per_tooth.unwrap_or(f64::NAN),
        };
        out.push(LimitingGate {
            gate: GateKind::Chipload,
            side: Some(*side),
            observed: triggering.observed_mm_per_tooth,
            bound,
            overshoot_fraction: signed_overshoot(triggering.observed_mm_per_tooth, bound),
            band_admitted: false,
            locality: triggering.evidence.locality.clone(),
        });
    }
    if let PowerVerdict::Exceeds {
        peak_kw,
        available_kw,
        evidence,
        ..
    } = &verdict.power
    {
        out.push(LimitingGate {
            gate: GateKind::Power,
            side: None,
            observed: *peak_kw,
            bound: *available_kw,
            overshoot_fraction: signed_overshoot(*peak_kw, *available_kw),
            band_admitted: false,
            locality: evidence.locality.clone(),
        });
    }
    if let DeflectionVerdict::Exceeds {
        peak_mm,
        bounds,
        evidence,
        ..
    } = &verdict.deflection
    {
        out.push(LimitingGate {
            gate: GateKind::Deflection,
            side: None,
            observed: *peak_mm,
            bound: bounds.exceeds_mm,
            overshoot_fraction: signed_overshoot(*peak_mm, bounds.exceeds_mm),
            band_admitted: false,
            locality: evidence.locality.clone(),
        });
    }
    if let DepthVerdict::Exceeds {
        peak_mm,
        bound,
        evidence,
        ..
    } = &verdict.depth
    {
        let cap_mm = bound.cap_mm();
        out.push(LimitingGate {
            gate: GateKind::Depth,
            side: None,
            observed: *peak_mm,
            bound: cap_mm,
            overshoot_fraction: signed_overshoot(*peak_mm, cap_mm),
            band_admitted: false,
            locality: evidence.locality.clone(),
        });
    }
    out
}

fn envelope_across(
    baseline: &OptimizeCandidate,
    candidates: &[OptimizeCandidate],
) -> SearchEnvelopeReached {
    let mut env = SearchEnvelopeReached::default();
    let configs = std::iter::once(&baseline.params).chain(candidates.iter().map(|c| &c.params));
    for cfg in configs {
        let (feed, rpm, stepover, doc, scallop) = config_axes(cfg);
        if let Some(v) = feed {
            extend_extent(&mut env.feed_mm_min, v);
        }
        if let Some(v) = rpm {
            extend_extent(&mut env.spindle_rpm, v);
        }
        if let Some(v) = stepover {
            extend_extent(&mut env.stepover_mm, v);
        }
        if let Some(v) = doc {
            extend_extent(&mut env.depth_per_pass_mm, v);
        }
        if let Some(v) = scallop {
            extend_extent(&mut env.scallop_height_mm, v);
        }
    }
    env
}

/// `(feed, rpm, stepover, depth, scallop_height)` tuple.
type ConfigAxes = (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);

/// Read the five parameters off an `OperationConfig`. The function
/// names four operation families (Pocket, Adaptive, Adaptive3d,
/// Scallop) and returns `(None, …)` for every other one.
fn config_axes(cfg: &crate::compute::catalog::OperationConfig) -> ConfigAxes {
    use crate::compute::catalog::OperationConfig as Op;
    match cfg {
        Op::Pocket(p) => (
            Some(p.feed_rate),
            None,
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Adaptive(p) => (
            Some(p.feed_rate),
            None,
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Adaptive3d(p) => (
            Some(p.feed_rate),
            p.spindle_rpm.map(|r| r as f64),
            Some(p.stepover),
            Some(p.depth_per_pass),
            None,
        ),
        Op::Scallop(p) => (Some(p.feed_rate), None, None, None, Some(p.scallop_height)),
        _ => (None, None, None, None, None),
    }
}

fn extend_extent(extent: &mut Option<AxisExtent>, value: f64) {
    if !value.is_finite() {
        return;
    }
    match extent {
        Some(e) => {
            if value < e.min {
                e.min = value;
            }
            if value > e.max {
                e.max = value;
            }
        }
        None => {
            *extent = Some(AxisExtent {
                min: value,
                max: value,
            });
        }
    }
}

fn signed_overshoot(observed: f64, bound: f64) -> f64 {
    if bound.abs() < 1e-12 {
        return 0.0;
    }
    (observed - bound) / bound
}

fn headline_no_safe(non_baseline_count: usize, limiting: &[LimitingGate]) -> String {
    let lead = format!(
        "Tried {non_baseline_count} {}; none was a safe full cut faster than the baseline.",
        candidates_word(non_baseline_count)
    );
    if limiting.is_empty() {
        return lead;
    }
    let pieces: Vec<String> = limiting
        .iter()
        .map(|g| match g.gate {
            GateKind::Chipload => match g.side {
                Some(ChipSide::High) => format!(
                    "chipload {:.4} mm/tooth ({:+.0} % over the LUT max {:.4})",
                    g.observed,
                    g.overshoot_fraction * 100.0,
                    g.bound,
                ),
                Some(ChipSide::Low) => format!(
                    "chipload {:.4} mm/tooth ({:+.0} % below the LUT min {:.4})",
                    g.observed,
                    g.overshoot_fraction * 100.0,
                    g.bound,
                ),
                None => format!("chipload {:.4} mm/tooth", g.observed),
            },
            GateKind::Power => format!(
                "power {:.2} kW ({:+.0} % over the available {:.2})",
                g.observed,
                g.overshoot_fraction * 100.0,
                g.bound,
            ),
            GateKind::Deflection => format!(
                "deflection {:.0} µm ({:+.0} % over the {:.0} µm bound)",
                g.observed * 1000.0,
                g.overshoot_fraction * 100.0,
                g.bound * 1000.0,
            ),
            GateKind::Depth => format!(
                "depth of cut {:.2} mm ({:+.0} % over the {:.2} mm rule of thumb; reported, not a limit)",
                g.observed,
                g.overshoot_fraction * 100.0,
                g.bound,
            ),
        })
        .collect();
    format!("{lead} The fastest one exceeds: {}.", pieces.join("; "))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::ids::ToolpathId;
    use crate::tool_load::optimize::{OptimizeCandidate, ParamDelta, SearchStage};
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
        DeflectionBounds, SampleEvidence, ToolpathLoadVerdict,
    };

    fn candidate(feed: f64, cycle: f64, verdict: ToolpathLoadVerdict) -> OptimizeCandidate {
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig {
                feed_rate: feed,
                ..PocketConfig::default()
            }),
            delta: ParamDelta {
                feed_mm_min: Some(feed),
                ..Default::default()
            },
            dressups: Box::default(),
            cycle_time_s: cycle,
            verdict,
            full_cut: crate::tool_load::optimize::FullCutCheck::default(),
            stage: SearchStage::Measured,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            air_cut_fraction_of_total_runtime: None,
        }
    }

    fn band() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn within_chipload(observed: f64) -> ChiploadVerdict {
        ChiploadVerdict::Within {
            approach_to_min: Some(ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::empty(),
                bounds: band(),
            }),
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: band(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
        }
    }

    fn exceeds_chipload_high(peak: f64) -> ChiploadVerdict {
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at(0),
                bounds: band(),
            },
            confidence: Confidence::Validated,
        }
    }

    fn exceeds_deflection(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    fn within_power() -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw: 0.05,
            available_kw: 0.6,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
            bound_source: None,
        }
    }

    fn within_deflection(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    fn vd(chipload: ChiploadVerdict, defl: DeflectionVerdict) -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload,
            power: within_power(),
            deflection: defl,
            // S3: a hand-built fixture states no measured depth; the
            // row keeps the posture of the gates beside it.
            depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
            material_split: None,
        }
    }

    /// The narrative names the exceeded rows of the FASTEST candidate,
    /// and the envelope spans every attempted feed.
    #[test]
    fn no_safe_narrative_names_the_fastest_candidate() {
        let baseline = candidate(
            3150.0,
            773.0,
            vd(within_chipload(0.050), within_deflection(0.158)),
        );
        let attempted = vec![
            baseline,
            candidate(
                3600.0,
                474.0,
                vd(exceeds_chipload_high(0.0707), within_deflection(0.158)),
            ),
            candidate(
                4000.0,
                431.0,
                vd(exceeds_chipload_high(0.0707), exceeds_deflection(0.237)),
            ),
        ];
        let n = build_no_safe_narrative(&attempted);
        assert!(
            n.headline.contains("chipload") && n.headline.contains("deflection"),
            "the headline names the rows of the fastest candidate, got: {}",
            n.headline,
        );
        assert_eq!(n.limiting_gates.len(), 2, "chipload + deflection exceed");
        let chipload_gate = n
            .limiting_gates
            .iter()
            .find(|g| matches!(g.gate, GateKind::Chipload))
            .expect("chipload gate present");
        assert_eq!(chipload_gate.side, Some(ChipSide::High));
        assert!(!chipload_gate.band_admitted);
        assert!(chipload_gate.overshoot_fraction > 0.0);
        let feed = n.envelope.feed_mm_min.expect("feed extent present");
        assert!((feed.min - 3150.0).abs() < 1e-9);
        assert!((feed.max - 4000.0).abs() < 1e-9);
        assert!(!n.explanation.is_empty());
    }

    /// The depth row names itself with `GateKind::Depth` and reads its
    /// bound from the rigidity cap.
    #[test]
    fn an_exceeded_depth_row_names_itself() {
        let cap = crate::machine::RigidityDepthCap {
            factor: 0.25,
            diameter_mm: 6.0,
        };
        let mut verdict = vd(within_chipload(0.050), within_deflection(0.10));
        verdict.depth = DepthVerdict::Exceeds {
            peak_mm: 3.0,
            bound: cap,
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        };
        let gates = limiting_gates_for_verdict(&verdict);
        assert_eq!(gates.len(), 1, "only the depth row exceeds: {gates:?}");
        let depth = &gates[0];
        assert_eq!(depth.gate, GateKind::Depth);
        assert!((depth.bound - cap.cap_mm()).abs() < 1e-12);
        assert!((depth.observed - 3.0).abs() < 1e-12);
        assert!(depth.overshoot_fraction > 0.0);
    }

    /// A clean verdict names no row.
    #[test]
    fn a_clean_verdict_names_no_row() {
        let verdict = vd(within_chipload(0.050), within_deflection(0.10));
        assert!(limiting_gates_for_verdict(&verdict).is_empty());
    }
}
