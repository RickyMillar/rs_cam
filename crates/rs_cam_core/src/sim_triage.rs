//! One typed answer to "what should I act on?" — for the operator reading a
//! panel and the agent reading JSON, from the same object.
//!
//! # Why this module exists
//!
//! `SIMULATION_ISSUE_CHANNEL_CENSUS.md` found the simulation issue channel
//! publishing **three different quantities under the name "issue count"**
//! (coalesced segments, per-sample tallies, and a post-filter MCP count —
//! 43x apart on one trace), through five surfaces that each concatenated,
//! ranked and truncated differently, with **no deduplication anywhere** and
//! **no cap** on the largest list. The GUI's own source comments record
//! 24,800 air-cut entries beside 14 hotspots and a headline pill reading
//! "issues 46751".
//!
//! The failure that matters is not the size of the number. It is that a
//! rapid-through-stock and an air-cut run were **the same kind of thing** in
//! the list an operator scans before pressing go.
//!
//! # The correction that shaped this design
//!
//! Census §2 measured where the volume actually comes from, and it is not
//! where the plan assumed:
//!
//! - the all-air toolpath is the **cheap** case — 20,328 of 20,328 cutting
//!   samples flagged, but only **42** segments, because its air is one long
//!   contiguous run;
//! - a perfectly ordinary 6 mm pocket produced **759** segments.
//!
//! Segment count scales with **transition density in ordinary cutting**, not
//! with how much air there is. So a cap and a spatial dedup work; more
//! aggressive contiguity coalescing does not. That is why the bound here is
//! a hard cap plus a spatial key, and not a smarter run-length encoder.
//!
//! # The classes (census §3.1)
//!
//! | Class | Definition | Retention |
//! |---|---|---|
//! | A safety | physical damage if run | never dropped, **never deduped** |
//! | B action | costs a run or a part | one per (rule, toolpath) |
//! | C advisory | worth seeing, capped | hard cap + "N more" |
//! | D sample | raw evidence | stays on the trace, never a list item |
//! | E noise | the model has no other way to say "nothing here" | not retained |
//!
//! [`ChannelCounts`] is where the class-D tallies live, each under a name
//! that says which population it counts. Nothing is hidden; it is demoted to
//! a footer and labelled.

use crate::collision::RapidCollision;
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::ids::ToolpathId;
use crate::sim_measurability::MeasurabilityReport;
use crate::simulation_cut::SimulationCutTrace;
use crate::toolpath_spans::{RegionSpanRole, SpanId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Advisories retained per toolpath before truncation.
pub const ADVISORY_CAP_PER_TOOLPATH: usize = 10;
/// Advisories retained per project before truncation.
pub const ADVISORY_CAP_PER_PROJECT: usize = 50;

/// Minimum spatial bucket edge (mm) when the tool diameter is unknown or
/// tiny. Buckets are tool-relative (`4 x diameter`) so one bucket is a
/// recognisable place on the part at any tool size, and this floor keeps a
/// Ø1 finishing tool from generating a bucket per millimetre.
pub const MIN_SPATIAL_BUCKET_MM: f64 = 10.0;

/// A list that knows it is incomplete.
///
/// The census's recurring failure was lists that truncated silently: the MCP
/// capped its arrays with no flag, the CLI stopped at ten hotspots with no
/// "N more", and the GUI concatenated five sources with no cap at all. A
/// bound is only honest if the reader can see it was applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounded<T> {
    pub items: Vec<T>,
    /// True when `total_matching > items.len()`.
    pub truncated: bool,
    /// How many findings existed before the cap.
    pub total_matching: usize,
}

impl<T> Default for Bounded<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            truncated: false,
            total_matching: 0,
        }
    }
}

impl<T> Bounded<T> {
    /// Bound `items` to `cap`, reporting `total_matching` as the count
    /// BEFORE any capping — including caps a caller applied earlier.
    ///
    /// Taking the total from the incoming vector would under-report whenever
    /// an upstream pass had already trimmed it, which is exactly what the
    /// per-toolpath cap does: 300 hotspots on one toolpath arrive here as 10,
    /// and a naive `items.len()` would then declare the list complete. A
    /// bound that mis-states how much it hid is worse than no bound, because
    /// the reader stops looking.
    fn with_total(mut items: Vec<T>, cap: usize, total_matching: usize) -> Self {
        items.truncate(cap);
        Self {
            truncated: total_matching > items.len(),
            items,
            total_matching,
        }
    }

    /// How many were withheld — what a "N more" line prints.
    pub fn hidden(&self) -> usize {
        self.total_matching.saturating_sub(self.items.len())
    }
}

/// The key two findings must share to be merged.
///
/// `region` is a **source** key — the nearest `SpanKind::Region` ancestor and
/// its role — which satisfies programme rule 5: it survives arc-fitting and
/// TSP reordering because it is structural ancestry, not a post-transform
/// label. Falls back to the semantic item id, then to nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedupKey {
    pub id: DiagnosticId,
    pub toolpath_id: Option<ToolpathId>,
    pub region: Option<(SpanId, RegionSpanRole)>,
    pub semantic_item_id: Option<u64>,
    /// `(floor(x/B), floor(y/B), floor(z/B))`, `B = max(4 x diameter, 10 mm)`.
    pub bucket: [i64; 3],
}

/// What justified a finding — kept from the **worst** contributing instance,
/// so merging never softens the reading that motivated the entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct WorstEvidence {
    pub position: [f64; 3],
    pub move_index: usize,
    pub wasted_runtime_s: f64,
    pub duration_s: f64,
    pub min_radial_engagement: f64,
    pub sample_count: usize,
}

/// One entry in a triage list: the existing [`Diagnostic`] contract plus what
/// merging needs to stay honest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub diagnostic: Diagnostic,
    pub dedup_key: DedupKey,
    /// How many findings merged into this one. Always >= 1.
    pub occurrences: usize,
    pub worst: WorstEvidence,
}

/// The class-D tallies, each named for the population it counts.
///
/// R-9. All three of these shipped under names close enough to be mistaken
/// for one another, differing by 43x on the census fixture. None of the
/// values changed; they are now stated together, where the difference is
/// visible instead of inferable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelCounts {
    /// Cutting samples with `radial_woc_fraction < 0.02`. **One per sample.**
    /// Was `air_cut_issue_count`.
    pub flagged_samples_air: usize,
    /// Cutting samples in `0.02..0.10`. One per sample. Was
    /// `low_engagement_issue_count`.
    pub flagged_samples_low: usize,
    /// **LEGACY.** Contiguous air/low-engagement RUNS, coalesced per
    /// toolpath — `SimulationCutSummary::issue_count`, unchanged in value and
    /// still published on the wire under that name. This is the number every
    /// MCP and CLI surface has been printing as "issues", and it is 43x
    /// smaller than the sample tallies above. It is a diagnostic-sample
    /// count, not a defect count, and nothing should gate on it.
    pub issue_segments: usize,
    pub hotspots_total: usize,
    pub samples_total: usize,
}

impl ChannelCounts {
    pub fn from_trace(trace: &SimulationCutTrace) -> Self {
        // Counted from the samples directly, with the accumulator's own
        // thresholds. The published `air_cut_issue_count` /
        // `low_engagement_issue_count` fields live only on the SEMANTIC
        // summaries, which exist only when the caller supplied a semantic
        // trace — so reading them would silently report zero on exactly the
        // traces that have no semantics, which is the opposite of what a
        // counts footer is for.
        let mut air = 0usize;
        let mut low = 0usize;
        for s in trace.samples.iter().filter(|s| s.is_cutting) {
            let r = s.engagement.radial_woc_fraction;
            if r < 0.02 {
                air += 1;
            } else if r < 0.10 {
                low += 1;
            }
        }
        Self {
            flagged_samples_air: air,
            flagged_samples_low: low,
            issue_segments: trace.summary.issue_count,
            hotspots_total: trace.hotspots.len(),
            samples_total: trace.samples.len(),
        }
    }
}

/// The page-one answer. One object, four consumers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulationTriage {
    /// **Read first** — it qualifies everything below. A metric marked
    /// `NotMeasurable` here has no verdict in `actions`, by rule.
    pub measurability: MeasurabilityReport,
    /// Class A. Uncapped, never deduped.
    pub safety: Vec<Finding>,
    /// Class B. Uncapped (the rule set bounds it).
    pub actions: Vec<Finding>,
    /// Class C. Capped, with the count it withheld.
    pub advisories: Bounded<Finding>,
    /// Class D tallies, explicitly labelled.
    pub counts: ChannelCounts,
}

impl SimulationTriage {
    /// True when there is nothing the operator must look at.
    pub fn is_clear(&self) -> bool {
        self.safety.is_empty() && self.actions.is_empty()
    }

    /// Highest severity anywhere in the actionable lists.
    pub fn worst_severity(&self) -> Option<Severity> {
        self.safety
            .iter()
            .chain(self.actions.iter())
            .map(|f| f.diagnostic.severity)
            .max()
    }
}

/// Resolves a sample's span path to its nearest `Region` ancestor.
///
/// Supplied by callers that hold the annotated toolpaths. Callers that do not
/// pass `None` and dedup falls back to the semantic item id — coarser, but
/// never wrong.
pub type RegionResolver<'a> = &'a dyn Fn(ToolpathId, &[SpanId]) -> Option<(SpanId, RegionSpanRole)>;

/// Everything the builder needs. Borrowed, so no caller has to clone a trace.
pub struct TriageInputs<'a> {
    pub trace: &'a SimulationCutTrace,
    pub measurability: &'a MeasurabilityReport,
    /// Project verdicts, already computed. Reused rather than re-derived so
    /// the triage cannot drift from the verdicts the rest of the system acts
    /// on.
    pub diagnostics: &'a [Diagnostic],
    pub rapid_collisions: &'a [RapidCollision],
    /// Per-toolpath holder/shank collision counts.
    pub holder_collisions: &'a [(ToolpathId, usize)],
    /// Tool diameter per toolpath, for the spatial bucket edge.
    pub tool_diameters_mm: &'a BTreeMap<ToolpathId, f64>,
    pub region_of: Option<RegionResolver<'a>>,
}

impl SimulationTriage {
    /// Build the page-one answer.
    ///
    /// Without rest context (this form), `project.entry_load` is never
    /// emitted: a fresh-stock operation's entries descend through virgin
    /// material the generator planned for, so grading them against the body
    /// median flags normal plunge entries (measured on the Phase 0 perf
    /// golden: a fresh-stock drop-cutter fixture tripped it). Callers that
    /// know which toolpaths are rest-driven use
    /// [`Self::build_with_rest_context`]; the session does.
    pub fn build(inputs: &TriageInputs<'_>) -> Self {
        Self::build_with_rest_context(inputs, &BTreeSet::new())
    }

    /// [`Self::build`] plus the set of rest-driven toolpath ids
    /// (`StockSource::FromRemainingStock` — the same predicate that arms the
    /// pencil entry ramp, `session/compute.rs` `gen_initial_stock`).
    /// `project.entry_load` (G-ENTRYLOAD) is graded only on these: an entry
    /// carving rest material is the defect class the id names, while a
    /// fresh-stock entry plunge is planned motion.
    pub fn build_with_rest_context(
        inputs: &TriageInputs<'_>,
        rest_driven: &BTreeSet<ToolpathId>,
    ) -> Self {
        let mut safety = Vec::new();
        let mut actions = Vec::new();

        // ── Class A ────────────────────────────────────────────────────
        // Never deduped, never capped. Two collisions in one bucket stay
        // two entries: they are two places the machine will be damaged.
        for (i, c) in inputs.rapid_collisions.iter().enumerate() {
            safety.push(collision_finding(
                ids::PROJECT_RAPID_COLLISION,
                format!(
                    "Rapid passes through stock at move {} ({:.1}, {:.1}, {:.3}) -> \
                     ({:.1}, {:.1}, {:.3})",
                    c.move_index, c.start.x, c.start.y, c.start.z, c.end.x, c.end.y, c.end.z
                ),
                // `RapidCollision` carries no toolpath id; the caller
                // attributes it by move range. Scoped to the project rather
                // than guessed at.
                None,
                c.move_index,
                [c.start.x, c.start.y, c.start.z],
                i,
            ));
        }
        for (tp, count) in inputs.holder_collisions.iter().filter(|(_, n)| *n > 0) {
            for i in 0..*count {
                safety.push(collision_finding(
                    ids::PROJECT_HOLDER_COLLISION,
                    format!("Holder/shank collision ({} on this toolpath)", count),
                    Some(*tp),
                    0,
                    [0.0, 0.0, 0.0],
                    i,
                ));
            }
        }

        // ── Class B ────────────────────────────────────────────────────
        // Taken from the diagnostics the rest of the system already acts
        // on. Abstentions are NOT actions — they belong to the
        // measurability strip and would otherwise read as findings about
        // the toolpath, which is exactly the confusion the ruling removes.
        for d in inputs.diagnostics {
            if d.id.as_str() == ids::PROJECT_MEASURABILITY_ABSTAINED {
                continue;
            }
            if matches!(d.category, Category::Safety)
                && (d.id.as_str() == ids::PROJECT_RAPID_COLLISION
                    || d.id.as_str() == ids::PROJECT_HOLDER_COLLISION)
            {
                // Already represented, per-event, in `safety`.
                continue;
            }
            let toolpath_id = match d.scope {
                Scope::Toolpath { id }
                | Scope::MoveRange {
                    toolpath_id: id, ..
                } => Some(id),
                _ => None,
            };
            actions.push(Finding {
                dedup_key: DedupKey {
                    id: d.id.clone(),
                    toolpath_id,
                    region: None,
                    semantic_item_id: None,
                    bucket: [0, 0, 0],
                },
                diagnostic: d.clone(),
                occurrences: 1,
                worst: WorstEvidence::default(),
            });
        }

        // R-12 (census §6.4) — the actionable half of "Rivers B4".
        // G-ENTRYLOAD — the same pass's ENTRY motion, graded against its own
        // envelope because the load gates decline to look at it.
        for tp in &inputs.trace.toolpath_summaries {
            if tp.metrics_not_applicable {
                continue;
            }
            if let Some(f) = standing_material_finding(inputs.trace, tp.toolpath_id) {
                actions.push(f);
            }
            if rest_driven.contains(&tp.toolpath_id)
                && let Some(f) = entry_load_finding(inputs.trace, tp.toolpath_id)
            {
                actions.push(f);
            }
        }

        // ── Class C ────────────────────────────────────────────────────
        let mut advisories: Vec<Finding> = Vec::new();
        for (i, h) in inputs.trace.hotspots.iter().enumerate() {
            let bucket_mm = bucket_edge_mm(inputs.tool_diameters_mm.get(&h.toolpath_id).copied());
            let region = inputs
                .region_of
                .and_then(|f| f(h.toolpath_id, &h.span_path));
            advisories.push(Finding {
                dedup_key: DedupKey {
                    id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                    toolpath_id: Some(h.toolpath_id),
                    region,
                    semantic_item_id: h.semantic_item_id,
                    bucket: spatial_bucket(h.representative_position, bucket_mm),
                },
                diagnostic: Diagnostic {
                    id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                    scope: Scope::Toolpath { id: h.toolpath_id },
                    category: Category::Efficiency,
                    severity: Severity::Hint,
                    confidence: Confidence::Approximate,
                    state: DiagnosticState::Current,
                    source: Source::Simulation,
                    message: format!(
                        "{:.1}s wasted around ({:.1}, {:.1}, {:.1}) — engagement {:.2}",
                        h.wasted_runtime_s,
                        h.representative_position[0],
                        h.representative_position[1],
                        h.representative_position[2],
                        h.average_engagement
                    ),
                    evidence: Some(DiagnosticEvidence::Counts {
                        count: i + 1,
                        offender_toolpath_ids: vec![h.toolpath_id],
                    }),
                    fix: None,
                    supersedes: vec![],
                    suppressed_diagnostics: vec![],
                },
                occurrences: 1,
                worst: WorstEvidence {
                    position: h.representative_position,
                    move_index: h.move_start,
                    wasted_runtime_s: h.wasted_runtime_s,
                    duration_s: h.total_runtime_s,
                    min_radial_engagement: h.average_engagement,
                    sample_count: h.sample_index_end.saturating_sub(h.sample_index_start) + 1,
                },
            });
        }

        let advisories = dedup(advisories);
        // The honest denominator: distinct findings after merging, before
        // either cap. Both caps are display budgets; this is the truth they
        // are budgeting against.
        let advisories_total = advisories.len();
        let advisories = cap_per_toolpath(advisories, ADVISORY_CAP_PER_TOOLPATH);

        sort_findings(&mut safety);
        sort_findings(&mut actions);

        Self {
            measurability: inputs.measurability.clone(),
            safety,
            actions,
            advisories: Bounded::with_total(advisories, ADVISORY_CAP_PER_PROJECT, advisories_total),
            counts: ChannelCounts::from_trace(inputs.trace),
        }
    }
}

/// Fraction of a pass's cutting samples that must sit far above its own
/// typical bite before the pass is reported as crossing standing material.
///
/// The census fixture measured 4.7% of samples over 10x the commanded
/// offset against 94.1% at or below 1.5x — a clear bimodal signature, not a
/// tail. 2% is well under the measured case and well over the handful of
/// samples any pass produces at a step or a corner.
pub const STANDING_MATERIAL_SAMPLE_FRACTION: f64 = 0.02;

/// How many times its own median bite a sample must remove to count.
pub const STANDING_MATERIAL_MULTIPLE: f64 = 3.0;

/// Report a finishing/surface-following pass that is cutting through material
/// an upstream operation deliberately left standing.
///
/// **The reference is the pass's OWN median removed height, not a commanded
/// depth.** That is the whole point of the finding. Census §6.4 traced a
/// `project_curve` op reported as "20.4x commanded" — a ratio formed by
/// dividing a removed HEIGHT by a surface OFFSET, two different quantities,
/// on an op that has no axial step to divide by at all. Every competing
/// explanation (lift bridging, entry transients, arc-fit artifacts) was
/// refuted by four independent structural tags; the surviving one was
/// confirmed by a direct upstream-stock reading of 5.20 mm standing above
/// the cutter.
///
/// The simulator was right. What was missing was anyone saying what its
/// reading MEANT. Measuring against the pass's own median gives a
/// denominator that exists for every op, and it is what makes the bimodal
/// signature — a surface-following majority plus a distinct population
/// ploughing through a step — visible as one sentence.
fn standing_material_finding(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> Option<Finding> {
    let mut heights: Vec<f64> = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting && s.axial_engagement_mm > 0.0)
        .map(|s| s.axial_engagement_mm)
        .collect();
    if heights.len() < 50 {
        // Too few bites to have a meaningful median; saying nothing is
        // better than reporting noise as a structural fact.
        return None;
    }
    heights.sort_by(f64::total_cmp);
    let median = *heights.get(heights.len() / 2)?;
    if median <= 0.0 {
        return None;
    }

    let bar = median * STANDING_MATERIAL_MULTIPLE;
    let over: Vec<f64> = heights.iter().copied().filter(|h| *h > bar).collect();
    let fraction = over.len() as f64 / heights.len() as f64;
    if fraction < STANDING_MATERIAL_SAMPLE_FRACTION {
        return None;
    }
    let peak = over.last().copied().unwrap_or(bar);

    let worst_sample = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
        .max_by(|a, b| a.axial_engagement_mm.total_cmp(&b.axial_engagement_mm))?;

    Some(Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(ids::PROJECT_CROSSES_STANDING_MATERIAL),
            toolpath_id: Some(toolpath_id),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_CROSSES_STANDING_MATERIAL),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::ToolLoad,
            severity: Severity::Caution,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: format!(
                "crosses material an upstream op left standing: {:.1}% of cutting                  samples remove more than {:.2} mm (3x this pass's own {:.2} mm                  median bite), peaking at {:.2} mm",
                fraction * 100.0,
                bar,
                median,
                peak
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id,
                sample_start: worst_sample.sample_index,
                sample_end: worst_sample.sample_index,
                observed: peak,
                threshold: Some(bar),
                unit: "mm".to_owned(),
                locality: Default::default(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: over.len(),
        worst: WorstEvidence {
            position: worst_sample.position,
            move_index: worst_sample.move_index,
            duration_s: 0.0,
            wasted_runtime_s: 0.0,
            min_radial_engagement: worst_sample.engagement.radial_woc_fraction,
            sample_count: over.len(),
        },
    })
}

// ── G-ENTRYLOAD: entries get their own envelope ─────────────────────────
//
// The load gates (chipload, deflection, power) filter their populations to
// steady-state samples via `tool_load::locality::is_steady_state_for_gate`,
// which drops entry and transit spans wholesale. That filter is correct and
// is NOT weakened here: a dexel reading at a transit sample reports
// `stock_top - cutter_z` over neighbouring stock, not steady-state
// engagement, and gate verdicts built on it were exactly the P3 lift-bridge
// artifact.
//
// The consequence, though, was that entry motion had NO grader at all:
//
//  * the three load gates exclude it by span;
//  * `standing_material_finding` above reads `axial_engagement_mm`, and a
//    pure-vertical descent samples as `CutKinematics::Plunge`, whose removal
//    the simulator deliberately writes to `plunge_descent_mm` instead — so a
//    3.72 mm bite taken by an R0.5 tapered-ball TIP at entry read as
//    `axial_engagement_mm = 0.0` there;
//  * nothing else reads `plunge_descent_mm` at all outside the drill gates.
//
// So the operator found it on the part ("ramp entries are cutting through the
// stock, surprised this isn't flagged") before any surface did. This grades
// entry-intent samples against their OWN envelope, on BOTH removal axes.

/// How many times its own median BODY bite an entry-intent sample may remove
/// before the entry is reported — the `k` of G-ENTRYLOAD, and the same `k`
/// the pencil generator's entry ramp
/// ([`crate::pencil::entry_bite_budget_mm`]) is budgeted to stay inside.
pub const ENTRY_LOAD_MEDIAN_MULTIPLE: f64 = 2.0;

/// Peak entry bite (mm) at which the finding escalates `Caution` ->
/// `Critical` (visible as a red row; it does NOT block export — nothing about
/// this rule is a refusal).
///
/// A fixed millimetre, deliberately NOT a tool fraction. The only per-toolpath
/// tool figure this module has is [`TriageInputs::tool_diameters_mm`], which
/// carries the ENVELOPE diameter — on a tapered ball that overstates the
/// cutting tip by up to 14x (the radius programme's R-12), and an escalation
/// floor built on it would go quiet on precisely the small-tip tools that
/// break. A millimetre of unplanned axial engagement at entry is a
/// broken-cutter event on any tool a pencil/finishing pass runs.
pub const ENTRY_LOAD_SEVERE_PEAK_MM: f64 = 1.0;

/// Body samples required before this pass's median is a reference rather than
/// noise. Same bar, for the same reason, as `standing_material_finding`'s.
pub const ENTRY_LOAD_MIN_BODY_SAMPLES: usize = 50;

/// Noise floor (mm): a peak below this is not reported however large a
/// multiple of the median it is.
///
/// This rule reports LOAD, not ratios. On a pass whose median bite is 0.02 mm
/// — an ordinary fine finishing pass — a 0.06 mm entry is 3x the median and
/// nothing at all in the spindle. The floor is
/// [`crate::pencil::ENTRY_RAMP_MIN_BITE_MM`], the smallest per-lap budget any
/// tool earns from the generator side: an entry that removes less than the
/// least a ramp lap is allowed to take is, by that same standard, not an
/// event. The case this rule exists for is 37x it.
pub const ENTRY_LOAD_MIN_PEAK_MM: f64 = crate::pencil::ENTRY_RAMP_MIN_BITE_MM;

/// What the entry-load rule measured — published whether or not it fires, so
/// "no entry samples" and "entries were clean" can never be read as the same
/// thing.
///
/// A gate handed an empty population passes and looks healthy (measured
/// 2026-08-05: three gates returned `Within` with `sample_range 0..0`). This
/// type is the answer to that: [`Self::is_measured`] is false when either
/// population is missing, and no finding is ever built from an unmeasured
/// reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct EntryLoadObservation {
    /// Cutting samples whose generator intent was an entry
    /// (`EntryPlunge` / `EntryRamp` / `EntryHelix`). **The population.**
    pub entry_samples: usize,
    /// Cutting samples that are NOT entries and removed material — the
    /// denominator's population.
    pub body_samples: usize,
    /// Median body bite (mm). `None` = not measured, never 0.0.
    pub median_body_bite_mm: Option<f64>,
    /// Peak entry bite (mm), `max(axial_engagement_mm, plunge_descent_mm)`.
    /// `None` = no entry samples, never 0.0.
    pub peak_entry_bite_mm: Option<f64>,
    /// `median x ENTRY_LOAD_MEDIAN_MULTIPLE`. `None` when not measured.
    pub threshold_mm: Option<f64>,
    /// Entry samples over the threshold.
    pub over_threshold_samples: usize,
    pub worst_position: Option<[f64; 3]>,
    pub worst_move_index: Option<usize>,
    pub worst_sample_index: Option<usize>,
}

impl EntryLoadObservation {
    /// True only when BOTH populations were real. A false here means "not
    /// measured" — it never means "clean".
    pub fn is_measured(&self) -> bool {
        self.entry_samples > 0
            && self.body_samples >= ENTRY_LOAD_MIN_BODY_SAMPLES
            && self.median_body_bite_mm.is_some()
    }

    /// True when a measured reading is over its own threshold AND over the
    /// [`ENTRY_LOAD_MIN_PEAK_MM`] noise floor.
    pub fn exceeds(&self) -> bool {
        match (
            self.is_measured(),
            self.peak_entry_bite_mm,
            self.threshold_mm,
        ) {
            (true, Some(peak), Some(bar)) => peak > bar && peak >= ENTRY_LOAD_MIN_PEAK_MM,
            _ => false,
        }
    }
}

/// Removal at one sample, on whichever axis carried it.
///
/// **Both axes, by construction.** `axial_engagement_mm` is zeroed on
/// plunge-kinematics samples and `plunge_descent_mm` is zeroed on every other
/// kind (`dexel_stock::simulation::apply_subsegment_metrics`), so reading one
/// of them is reading half the entries. Reading the axial axis alone is the
/// specific mistake G-ENTRYLOAD is named after.
fn entry_bite_mm(s: &crate::simulation_cut::SimulationCutSample) -> f64 {
    s.axial_engagement_mm.max(s.plunge_descent_mm).max(0.0)
}

/// True when the generator tagged this sample's move as an entry.
///
/// Source-keyed (`source_intent`), so it survives arc-fitting and TSP
/// reordering — programme rule 5. `Drilling` is deliberately NOT an entry
/// here: drill cycles have their own three gates on
/// `ToolpathLoadVerdict::drill_gates` and their own removal model.
fn is_entry_intent(s: &crate::simulation_cut::SimulationCutSample) -> bool {
    matches!(
        s.source_intent,
        Some(
            crate::toolpath::MoveIntent::EntryPlunge
                | crate::toolpath::MoveIntent::EntryRamp
                | crate::toolpath::MoveIntent::EntryHelix
        )
    )
}

/// Measure one toolpath's entry motion against its own body.
///
/// The reference is the pass's OWN median body bite, for the same reason
/// `standing_material_finding` uses its own median: no commanded axial step
/// exists on a surface-following op to divide by. Entries are excluded from
/// that denominator — grading a population against a median it is itself part
/// of is how a pass made mostly of bad entries exonerates itself.
pub fn entry_load_observation(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> EntryLoadObservation {
    let mut body: Vec<f64> = Vec::new();
    let mut entry_samples = 0usize;
    let mut peak: Option<f64> = None;
    let mut worst: Option<&crate::simulation_cut::SimulationCutSample> = None;

    for s in trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
    {
        if is_entry_intent(s) {
            entry_samples += 1;
            let bite = entry_bite_mm(s);
            if peak.is_none_or(|p| bite > p) {
                peak = Some(bite);
                worst = Some(s);
            }
        } else if s.axial_engagement_mm > 0.0 {
            body.push(s.axial_engagement_mm);
        }
    }

    let mut obs = EntryLoadObservation {
        entry_samples,
        body_samples: body.len(),
        peak_entry_bite_mm: peak,
        worst_position: worst.map(|s| s.position),
        worst_move_index: worst.map(|s| s.move_index),
        worst_sample_index: worst.map(|s| s.sample_index),
        ..EntryLoadObservation::default()
    };

    if body.len() < ENTRY_LOAD_MIN_BODY_SAMPLES {
        return obs;
    }
    body.sort_by(f64::total_cmp);
    let Some(&median) = body.get(body.len() / 2) else {
        return obs;
    };
    if median <= 0.0 {
        return obs;
    }
    let bar = median * ENTRY_LOAD_MEDIAN_MULTIPLE;
    obs.median_body_bite_mm = Some(median);
    obs.threshold_mm = Some(bar);
    obs.over_threshold_samples = trace
        .samples
        .iter()
        .filter(|s| {
            s.toolpath_id == toolpath_id
                && s.is_cutting
                && is_entry_intent(s)
                && entry_bite_mm(s) > bar
        })
        .count();
    obs
}

/// Report a pass whose ENTRY motion removes multiples of what the pass itself
/// takes in steady cutting.
///
/// Fires only on a measured reading with a real population (see
/// [`EntryLoadObservation::is_measured`]); an op with no entry samples is
/// absent from the list, never reported clean.
fn entry_load_finding(trace: &SimulationCutTrace, toolpath_id: ToolpathId) -> Option<Finding> {
    let obs = entry_load_observation(trace, toolpath_id);
    if !obs.exceeds() {
        return None;
    }
    let peak = obs.peak_entry_bite_mm?;
    let bar = obs.threshold_mm?;
    let median = obs.median_body_bite_mm?;
    let position = obs.worst_position.unwrap_or_default();
    let move_index = obs.worst_move_index.unwrap_or_default();
    let sample_index = obs.worst_sample_index.unwrap_or_default();
    let severity = if peak >= ENTRY_LOAD_SEVERE_PEAK_MM {
        Severity::Critical
    } else {
        Severity::Caution
    };

    Some(Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(ids::PROJECT_ENTRY_LOAD),
            toolpath_id: Some(toolpath_id),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_ENTRY_LOAD),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::ToolLoad,
            severity,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: format!(
                "entry motion cuts far harder than the pass does: {} of {} entry samples \
                 remove more than {:.2} mm ({:.0}x this pass's own {:.2} mm median body \
                 bite), peaking at {:.2} mm at ({:.1}, {:.1}, {:.2}). The load gates skip \
                 entry spans, so nothing else grades this.",
                obs.over_threshold_samples,
                obs.entry_samples,
                bar,
                ENTRY_LOAD_MEDIAN_MULTIPLE,
                median,
                peak,
                position[0],
                position[1],
                position[2],
            ),
            evidence: Some(DiagnosticEvidence::SampleRange {
                toolpath_id,
                sample_start: sample_index,
                sample_end: sample_index,
                observed: peak,
                threshold: Some(bar),
                unit: "mm".to_owned(),
                locality: Default::default(),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: obs.over_threshold_samples,
        worst: WorstEvidence {
            position,
            move_index,
            duration_s: 0.0,
            wasted_runtime_s: 0.0,
            min_radial_engagement: 0.0,
            sample_count: obs.over_threshold_samples,
        },
    })
}

fn collision_finding(
    id: &str,
    message: String,
    toolpath_id: Option<ToolpathId>,
    move_index: usize,
    position: [f64; 3],
    ordinal: usize,
) -> Finding {
    Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(id),
            toolpath_id,
            region: None,
            semantic_item_id: None,
            // Class A is dedup-exempt. The ordinal guarantees that even two
            // collisions at the same point in the same bucket keep distinct
            // keys, so no future merge pass can silently fuse them.
            bucket: [ordinal as i64, i64::MIN, i64::MIN],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(id),
            scope: toolpath_id.map_or(Scope::Project, |id| Scope::Toolpath { id }),
            category: Category::Safety,
            severity: Severity::Critical,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message,
            evidence: toolpath_id.map(|id| DiagnosticEvidence::Move {
                toolpath_id: id,
                move_index,
                position: Some(position),
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: 1,
        worst: WorstEvidence {
            position,
            move_index,
            ..WorstEvidence::default()
        },
    }
}

fn bucket_edge_mm(tool_diameter_mm: Option<f64>) -> f64 {
    tool_diameter_mm
        .map(|d| d * 4.0)
        .unwrap_or(MIN_SPATIAL_BUCKET_MM)
        .max(MIN_SPATIAL_BUCKET_MM)
}

fn spatial_bucket(position: [f64; 3], edge_mm: f64) -> [i64; 3] {
    let e = edge_mm.max(1e-6);
    [
        (position[0] / e).floor() as i64,
        (position[1] / e).floor() as i64,
        (position[2] / e).floor() as i64,
    ]
}

/// Merge findings sharing a [`DedupKey`], keeping the worst evidence.
///
/// Merging keeps the **maximum** wasted runtime, the **minimum** engagement
/// and the **longest** duration, so a merged entry is never softer than the
/// worst thing it absorbed — the point of a bound is to shorten the list,
/// not to lower the reading.
fn dedup(findings: Vec<Finding>) -> Vec<Finding> {
    let mut by_key: Vec<Finding> = Vec::new();
    for f in findings {
        if let Some(existing) = by_key.iter_mut().find(|e| e.dedup_key == f.dedup_key) {
            existing.occurrences += 1;
            if f.diagnostic.severity > existing.diagnostic.severity {
                existing.diagnostic.severity = f.diagnostic.severity;
            }
            existing.worst.wasted_runtime_s = existing
                .worst
                .wasted_runtime_s
                .max(f.worst.wasted_runtime_s);
            existing.worst.duration_s = existing.worst.duration_s.max(f.worst.duration_s);
            existing.worst.min_radial_engagement = existing
                .worst
                .min_radial_engagement
                .min(f.worst.min_radial_engagement);
            existing.worst.sample_count += f.worst.sample_count;
        } else {
            by_key.push(f);
        }
    }
    by_key
}

/// Apply the per-toolpath cap before the project cap, so one pathological
/// toolpath cannot consume the whole project budget and hide every other
/// toolpath's advisories.
fn cap_per_toolpath(findings: Vec<Finding>, cap: usize) -> Vec<Finding> {
    let mut sorted = findings;
    sorted.sort_by(|a, b| {
        b.worst
            .wasted_runtime_s
            .total_cmp(&a.worst.wasted_runtime_s)
            .then_with(|| b.occurrences.cmp(&a.occurrences))
            .then_with(|| a.worst.move_index.cmp(&b.worst.move_index))
    });
    let mut per_tp: BTreeMap<Option<ToolpathId>, usize> = BTreeMap::new();
    sorted.retain(|f| {
        let n = per_tp.entry(f.dedup_key.toolpath_id).or_insert(0);
        *n += 1;
        *n <= cap
    });
    sorted
}

/// The one ordering, replacing the two contradictory ranks the census found.
///
/// `severity desc -> category -> occurrences desc -> first move index`.
/// `move_index` is the LAST key, not the first: "what should I look at" is
/// answered before "where is it".
fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        b.diagnostic
            .severity
            .cmp(&a.diagnostic.severity)
            .then_with(|| {
                category_rank(a.diagnostic.category).cmp(&category_rank(b.diagnostic.category))
            })
            .then_with(|| b.occurrences.cmp(&a.occurrences))
            .then_with(|| a.worst.move_index.cmp(&b.worst.move_index))
    });
}

fn category_rank(c: Category) -> u8 {
    match c {
        Category::Safety => 0,
        Category::ToolLoad => 1,
        Category::Geometry => 2,
        Category::Quality => 3,
        Category::Efficiency => 4,
        Category::State => 5,
    }
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
    use crate::simulation_cut::{Engagement, SimulationCutSample};

    fn air_sample(i: usize, tp: usize) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId(tp),
            move_index: i,
            sample_index: i,
            segment_time_s: 0.01,
            cumulative_time_s: i as f64 * 0.01,
            is_cutting: true,
            position: [(i % 200) as f64, 0.0, -1.0],
            engagement: Engagement::with_radial_woc(0.0),
            semantic_item_id: Some((i % 300) as u64),
            ..SimulationCutSample::test_fixture()
        }
    }

    fn inputs<'a>(
        trace: &'a SimulationCutTrace,
        m: &'a MeasurabilityReport,
        diags: &'a [Diagnostic],
        rapids: &'a [RapidCollision],
        holders: &'a [(ToolpathId, usize)],
        diameters: &'a BTreeMap<ToolpathId, f64>,
    ) -> TriageInputs<'a> {
        TriageInputs {
            trace,
            measurability: m,
            diagnostics: diags,
            rapid_collisions: rapids,
            holder_collisions: holders,
            tool_diameters_mm: diameters,
            region_of: None,
        }
    }

    /// GATE 1 — an all-air synthetic trace cannot create an unbounded
    /// user-facing list.
    #[test]
    fn gate_1_an_all_air_trace_cannot_produce_an_unbounded_list() {
        // 6000 air samples across 300 semantic items => hundreds of
        // hotspots, one per (toolpath, semantic item).
        let samples: Vec<_> = (0..6000).map(|i| air_sample(i, 1)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        assert!(
            trace.hotspots.len() > ADVISORY_CAP_PER_PROJECT,
            "fixture must overflow the cap to test it; got {} hotspots",
            trace.hotspots.len()
        );

        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d));

        assert!(t.advisories.items.len() <= ADVISORY_CAP_PER_PROJECT);
        assert!(t.advisories.truncated, "the cap must announce itself");
        assert!(t.advisories.hidden() > 0);
        // The withheld count must be measured against the PRE-cap total, not
        // against whatever an earlier cap already left behind.
        assert_eq!(
            t.advisories.total_matching,
            trace.hotspots.len(),
            "total_matching must be the true finding count, not a post-cap one"
        );
        assert_eq!(
            t.advisories.hidden(),
            trace.hotspots.len() - t.advisories.items.len()
        );
        // And the raw tallies are still available — demoted, not deleted.
        assert!(t.counts.flagged_samples_air > 0);
        assert!(t.counts.issue_segments > 0);
    }

    /// GATE 2 — one rapid collision + one holder collision + one
    /// material-removal warning stay visible among thousands of air samples.
    #[test]
    fn gate_2_real_events_survive_thousands_of_air_samples() {
        let samples: Vec<_> = (0..6000).map(|i| air_sample(i, 1)).collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);

        let rapid = RapidCollision {
            move_index: 4211,
            start: crate::geo::P3::new(3.0, 4.0, 5.0),
            end: crate::geo::P3::new(9.0, 4.0, 5.0),
        };
        let removal_warning = Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_GENERATED_EMPTY),
            scope: Scope::Toolpath { id: ToolpathId(1) },
            category: Category::Geometry,
            severity: Severity::Caution,
            confidence: Confidence::Static,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: "removed no material".to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        };

        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(
            &trace,
            &m,
            std::slice::from_ref(&removal_warning),
            std::slice::from_ref(&rapid),
            &[(ToolpathId(1), 1)],
            &d,
        ));

        assert_eq!(t.safety.len(), 2, "one rapid + one holder collision");
        assert!(
            t.safety
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_RAPID_COLLISION)
        );
        assert!(
            t.safety
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_HOLDER_COLLISION)
        );
        assert_eq!(t.actions.len(), 1, "the removal warning survives");
        // The point of the whole exercise: they are not in the same list as
        // the air-cut advisories, so no cap and no volume of noise can bury
        // them.
        assert!(!t.is_clear());
        assert_eq!(t.worst_severity(), Some(Severity::Critical));
    }

    /// GATE 3 — dedup never hides a distinct safety event.
    #[test]
    fn gate_3_two_collisions_in_one_bucket_stay_two_entries() {
        let trace = SimulationCutTrace::from_samples(0.5, vec![]);
        // Same toolpath, same spatial bucket, one millimetre apart.
        let rapids = vec![
            RapidCollision {
                move_index: 10,
                start: crate::geo::P3::new(1.0, 1.0, 1.0),
                end: crate::geo::P3::new(2.0, 1.0, 1.0),
            },
            RapidCollision {
                move_index: 11,
                start: crate::geo::P3::new(1.5, 1.0, 1.0),
                end: crate::geo::P3::new(2.5, 1.0, 1.0),
            },
        ];
        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &rapids, &[], &d));

        assert_eq!(
            t.safety.len(),
            2,
            "class A is dedup-exempt: two collisions in one bucket are two \
             places the machine gets damaged"
        );
        assert!(t.safety.iter().all(|f| f.occurrences == 1));
        assert_ne!(t.safety[0].dedup_key, t.safety[1].dedup_key);
    }

    #[test]
    fn dedup_keeps_the_worst_evidence_not_the_first() {
        let a = |wasted: f64, eng: f64| Finding {
            dedup_key: DedupKey {
                id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                toolpath_id: Some(ToolpathId(1)),
                region: None,
                semantic_item_id: None,
                bucket: [0, 0, 0],
            },
            diagnostic: Diagnostic {
                id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                scope: Scope::Toolpath { id: ToolpathId(1) },
                category: Category::Efficiency,
                severity: Severity::Hint,
                confidence: Confidence::Approximate,
                state: DiagnosticState::Current,
                source: Source::Simulation,
                message: String::new(),
                evidence: None,
                fix: None,
                supersedes: vec![],
                suppressed_diagnostics: vec![],
            },
            occurrences: 1,
            worst: WorstEvidence {
                wasted_runtime_s: wasted,
                min_radial_engagement: eng,
                sample_count: 1,
                ..WorstEvidence::default()
            },
        };
        let merged = dedup(vec![a(1.0, 0.5), a(9.0, 0.01), a(3.0, 0.2)]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].occurrences, 3);
        assert!((merged[0].worst.wasted_runtime_s - 9.0).abs() < 1e-9);
        assert!((merged[0].worst.min_radial_engagement - 0.01).abs() < 1e-9);
        assert_eq!(merged[0].worst.sample_count, 3);
    }

    #[test]
    fn one_pathological_toolpath_cannot_eat_the_whole_project_budget() {
        let mut findings = Vec::new();
        for tp in [1usize, 2] {
            for i in 0..40 {
                findings.push(Finding {
                    dedup_key: DedupKey {
                        id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                        toolpath_id: Some(ToolpathId(tp)),
                        region: None,
                        semantic_item_id: Some(i),
                        bucket: [i as i64, 0, 0],
                    },
                    diagnostic: Diagnostic {
                        id: DiagnosticId::from(ids::PROJECT_AIR_CUT_HIGH),
                        scope: Scope::Toolpath { id: ToolpathId(tp) },
                        category: Category::Efficiency,
                        severity: Severity::Hint,
                        confidence: Confidence::Approximate,
                        state: DiagnosticState::Current,
                        source: Source::Simulation,
                        message: String::new(),
                        evidence: None,
                        fix: None,
                        supersedes: vec![],
                        suppressed_diagnostics: vec![],
                    },
                    occurrences: 1,
                    // Toolpath 1 wastes far more, so an unqualified sort
                    // would fill the entire budget with it.
                    worst: WorstEvidence {
                        wasted_runtime_s: if tp == 1 { 1000.0 } else { 1.0 },
                        ..WorstEvidence::default()
                    },
                });
            }
        }
        let capped = cap_per_toolpath(findings, ADVISORY_CAP_PER_TOOLPATH);
        for tp in [1usize, 2] {
            let n = capped
                .iter()
                .filter(|f| f.dedup_key.toolpath_id == Some(ToolpathId(tp)))
                .count();
            assert_eq!(
                n, ADVISORY_CAP_PER_TOOLPATH,
                "toolpath {tp} should keep exactly its own budget"
            );
        }
    }

    #[test]
    fn r12_a_pass_ploughing_through_an_unroughed_step_is_reported() {
        // Census §6.4's signature, reproduced in miniature: a
        // surface-following majority plus a distinct population removing
        // multiples of it. The reference is the pass's OWN median, because
        // the op that produced the original reading has no commanded axial
        // step to divide by — that missing denominator is what turned a
        // correct measurement into a "20.4x commanded" anomaly report.
        let mut samples = Vec::new();
        for i in 0..200 {
            let deep = i % 20 == 0; // 5% of samples
            samples.push(SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: if deep { 4.0 } else { 0.2 },
                engagement: Engagement::with_radial_woc(0.3),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            });
        }
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d));

        let f = t
            .actions
            .iter()
            .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_CROSSES_STANDING_MATERIAL)
            .expect("the standing-material finding must fire");
        assert_eq!(f.diagnostic.severity, Severity::Caution);
        assert!(
            f.diagnostic.message.contains("own"),
            "the message must name its reference as the pass's OWN median, so \
             nobody re-forms the ratio that started this; got: {}",
            f.diagnostic.message
        );
        assert_eq!(f.occurrences, 10, "5% of 200 samples");
    }

    #[test]
    fn r12_stays_quiet_on_an_even_surface_following_pass() {
        let samples: Vec<_> = (0..200)
            .map(|i| SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: 0.2,
                engagement: Engagement::with_radial_woc(0.3),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            })
            .collect();
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d));
        assert!(
            !t.actions
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_CROSSES_STANDING_MATERIAL),
            "an even pass must not be reported"
        );
    }

    #[test]
    fn an_abstention_is_not_an_action() {
        // Checkpoint D Q2: the abstention describes the SIMULATION, not the
        // toolpath. Putting it in `actions` would recreate the confusion the
        // ruling removed.
        let trace = SimulationCutTrace::from_samples(0.5, vec![]);
        let abstention = Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_MEASURABILITY_ABSTAINED),
            scope: Scope::Project,
            category: Category::State,
            severity: Severity::Hint,
            confidence: Confidence::Static,
            state: DiagnosticState::NotApplicable,
            source: Source::Simulation,
            message: "air-cut % withheld".to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        };
        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(
            &trace,
            &m,
            std::slice::from_ref(&abstention),
            &[],
            &[],
            &d,
        ));
        assert!(t.actions.is_empty());
        assert!(t.is_clear());
    }

    // ── G-ENTRYLOAD ────────────────────────────────────────────────────
    //
    // The blind spot in one sentence: the three load gates filter to
    // steady-state samples and drop entry spans, and a vertical entry's
    // removal lands on `plunge_descent_mm`, which the crosses-standing rule
    // does not read. So a 3.72 mm bite at entry was graded by nothing.

    /// A pass body: 200 well-behaved surface-following samples at 0.20 mm.
    fn body_samples(tp: usize) -> Vec<SimulationCutSample> {
        (0..200)
            .map(|i| SimulationCutSample {
                toolpath_id: ToolpathId(tp),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: 0.2,
                engagement: Engagement::with_radial_woc(0.3),
                source_intent: Some(crate::toolpath::MoveIntent::FinishingCut),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            })
            .collect()
    }

    /// GATE — a vertical entry carving through standing material is
    /// reported, and it is reported off the PLUNGE axis. This is the
    /// wanaka200 shape in miniature: `axial_engagement_mm` reads 0.0 on
    /// every one of these samples, exactly as the simulator writes it for
    /// plunge kinematics, and the finding must still fire.
    #[test]
    fn g_entryload_a_vertical_entry_through_standing_material_is_reported() {
        let mut samples = body_samples(1);
        for i in 0..6 {
            samples.push(SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 500 + i,
                sample_index: 500 + i,
                segment_time_s: 0.01,
                is_cutting: true,
                // The axis the crosses-standing rule reads: silent.
                axial_engagement_mm: 0.0,
                // The axis that actually carried it.
                plunge_descent_mm: 3.72,
                position: [44.5, 92.1, 1.64],
                source_intent: Some(crate::toolpath::MoveIntent::EntryPlunge),
                ..SimulationCutSample::test_fixture()
            });
        }
        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Control: the axial-only rule stays silent on exactly this trace —
        // which is why the entry needed its own.
        assert!(
            standing_material_finding(&trace, ToolpathId(1)).is_none(),
            "the crosses-standing rule reads the axial axis; if it fires here \
             this sentry is no longer testing the blind spot it was written for"
        );

        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert!(obs.is_measured(), "both populations are real: {obs:?}");
        assert_eq!(obs.entry_samples, 6, "the population must be stated");
        assert_eq!(obs.body_samples, 200);
        assert_eq!(obs.over_threshold_samples, 6);
        assert_eq!(obs.median_body_bite_mm, Some(0.2));
        assert_eq!(obs.threshold_mm, Some(0.4));
        assert_eq!(obs.peak_entry_bite_mm, Some(3.72));

        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();

        // Without rest context the rule must NOT reach actions — a
        // fresh-stock entry plunge is planned motion, and the perf golden's
        // drop-cutter fixture pins that silence.
        let plain = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d));
        assert!(
            !plain
                .actions
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_ENTRY_LOAD),
            "entry_load must stay silent without rest context"
        );

        let rest: BTreeSet<ToolpathId> = [ToolpathId(1)].into_iter().collect();
        let t = SimulationTriage::build_with_rest_context(
            &inputs(&trace, &m, &[], &[], &[], &d),
            &rest,
        );
        let f = t
            .actions
            .iter()
            .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_ENTRY_LOAD)
            .expect("the entry-load finding must reach triage actions");
        assert_eq!(
            f.diagnostic.severity,
            Severity::Critical,
            "3.72 mm is over the {ENTRY_LOAD_SEVERE_PEAK_MM} mm absolute floor"
        );
        assert_eq!(f.occurrences, 6);
        assert_eq!(f.worst.position, [44.5, 92.1, 1.64]);
        assert!(
            f.diagnostic.message.contains("6 of 6 entry samples"),
            "the payload must state its population: {}",
            f.diagnostic.message
        );
    }

    /// GATE — a ramped entry that stays inside its budget says nothing.
    /// The half a stub that always fired would fail.
    #[test]
    fn g_entryload_a_budgeted_ramp_entry_stays_silent() {
        let mut samples = body_samples(1);
        for i in 0..40 {
            samples.push(SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 500 + i,
                sample_index: 500 + i,
                segment_time_s: 0.01,
                is_cutting: true,
                // 0.25 mm laps against a 0.20 mm median = 1.25x, inside k.
                axial_engagement_mm: 0.25,
                engagement: Engagement::with_radial_woc(0.3),
                source_intent: Some(crate::toolpath::MoveIntent::EntryRamp),
                ..SimulationCutSample::test_fixture()
            });
        }
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert!(obs.is_measured(), "a clean entry is still a MEASURED entry");
        assert_eq!(obs.entry_samples, 40);
        assert_eq!(obs.over_threshold_samples, 0);
        assert!(!obs.exceeds());
        assert!(entry_load_finding(&trace, ToolpathId(1)).is_none());
    }

    /// GATE — the empty-population trap. A pass with NO entry samples must
    /// report not-measured, never "clean". Measured 2026-08-05: three gates
    /// returned `Within` on `sample_range 0..0` and were indistinguishable
    /// from a measured clean cut on every surface.
    #[test]
    fn g_entryload_an_op_with_no_entry_samples_is_not_measured_not_clean() {
        let trace = SimulationCutTrace::from_samples(0.5, body_samples(1));
        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert_eq!(obs.entry_samples, 0, "the fixture must have no entries");
        assert!(
            !obs.is_measured(),
            "an empty entry population is NOT a clean verdict: {obs:?}"
        );
        assert_eq!(obs.peak_entry_bite_mm, None, "None, never 0.0");
        assert!(!obs.exceeds());
        assert!(entry_load_finding(&trace, ToolpathId(1)).is_none());
    }

    /// GATE — the noise floor. A fine pass whose entry is a large MULTIPLE of
    /// a tiny median, but a small absolute bite, is not an event: this rule
    /// reports load, not ratios.
    #[test]
    fn g_entryload_a_large_multiple_of_a_tiny_median_is_still_not_a_load_event() {
        let mut samples: Vec<_> = (0..200)
            .map(|i| SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: 0.02,
                engagement: Engagement::with_radial_woc(0.3),
                source_intent: Some(crate::toolpath::MoveIntent::FinishingCut),
                ..SimulationCutSample::test_fixture()
            })
            .collect();
        for i in 0..5 {
            samples.push(SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 700 + i,
                sample_index: 700 + i,
                segment_time_s: 0.01,
                is_cutting: true,
                // 3x the median, and 0.06 mm — nothing at all in the spindle.
                plunge_descent_mm: 0.06,
                source_intent: Some(crate::toolpath::MoveIntent::EntryPlunge),
                ..SimulationCutSample::test_fixture()
            });
        }
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert!(
            obs.is_measured(),
            "it WAS measured — it just is not a fault"
        );
        assert_eq!(obs.over_threshold_samples, 5, "over the ratio bar");
        assert!(
            !obs.exceeds(),
            "0.06 mm is under the {ENTRY_LOAD_MIN_PEAK_MM} mm floor"
        );
        assert!(entry_load_finding(&trace, ToolpathId(1)).is_none());
    }

    /// GATE — a pass with entries but too small a body population also
    /// reports not-measured. The median is the whole reference; without one
    /// there is no verdict to give.
    #[test]
    fn g_entryload_too_few_body_samples_is_not_measured() {
        let mut samples: Vec<_> = body_samples(1).into_iter().take(10).collect();
        samples.push(SimulationCutSample {
            toolpath_id: ToolpathId(1),
            move_index: 900,
            sample_index: 900,
            segment_time_s: 0.01,
            is_cutting: true,
            plunge_descent_mm: 9.0,
            source_intent: Some(crate::toolpath::MoveIntent::EntryPlunge),
            ..SimulationCutSample::test_fixture()
        });
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert_eq!(obs.entry_samples, 1);
        assert_eq!(obs.body_samples, 10);
        assert_eq!(obs.median_body_bite_mm, None, "None, never 0.0");
        assert!(!obs.is_measured());
        assert!(entry_load_finding(&trace, ToolpathId(1)).is_none());
    }

    /// GATE — entries are not in their own denominator. A pass made mostly
    /// of bad entries must not exonerate itself by dragging its own median
    /// up.
    #[test]
    fn g_entryload_entries_are_excluded_from_their_own_reference() {
        let mut samples = body_samples(1);
        for i in 0..300 {
            samples.push(SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 1000 + i,
                sample_index: 1000 + i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: 3.0,
                engagement: Engagement::with_radial_woc(0.3),
                source_intent: Some(crate::toolpath::MoveIntent::EntryRamp),
                ..SimulationCutSample::test_fixture()
            });
        }
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let obs = entry_load_observation(&trace, ToolpathId(1));
        assert_eq!(
            obs.median_body_bite_mm,
            Some(0.2),
            "300 entry samples must not move the body median"
        );
        assert_eq!(obs.over_threshold_samples, 300);
        assert!(obs.exceeds());
    }
}
