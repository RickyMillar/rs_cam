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

use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticEvidence, DiagnosticId, DiagnosticState, Scope,
    Severity, Source, ids,
};
use crate::ids::ToolpathId;
use crate::machine::kinematic_utilization::ToolpathKinematicUtilization;
use crate::stock::collision::{AttributedRapidCollision, HolderCollisionCheck};
use crate::stock::sim_measurability::MeasurabilityReport;
use crate::stock::simulation_cut::SimulationCutTrace;
use crate::trace::toolpath_spans::{RegionSpanRole, SpanId};
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
pub(crate) const MIN_SPATIAL_BUCKET_MM: f64 = 10.0;

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
    ///
    /// **Test door.** The `#[cfg(test)]` module of this file is the only
    /// caller. No production path reads it (S29, tech debt 2026-09-16).
    #[cfg(test)]
    pub(crate) fn worst_severity(&self) -> Option<Severity> {
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
pub(crate) type RegionResolver<'a> =
    &'a dyn Fn(ToolpathId, &[SpanId]) -> Option<(SpanId, RegionSpanRole)>;

/// Everything the builder needs. Borrowed, so no caller has to clone a trace.
pub struct TriageInputs<'a> {
    pub trace: &'a SimulationCutTrace,
    pub measurability: &'a MeasurabilityReport,
    /// Project verdicts, already computed. Reused rather than re-derived so
    /// the triage cannot drift from the verdicts the rest of the system acts
    /// on.
    pub diagnostics: &'a [Diagnostic],
    /// Every rapid collision in both move frames, from
    /// `ProjectEvidence::attributed_rapid_collisions` (G-RAPIDFRAME). The
    /// triage names the toolpath and the local move from this record; it
    /// does not attribute a move itself.
    pub rapid_collisions: &'a [AttributedRapidCollision],
    /// The toolpath names, for the collision lines. A missing name prints
    /// the id alone.
    pub toolpath_names: &'a BTreeMap<ToolpathId, String>,
    /// Per-toolpath holder/shank collision outcomes.
    ///
    /// CMP-14: three states, not a count. This list used to be filtered
    /// with `n > 0`, so a toolpath whose check FAILED was indistinguishable
    /// from a clean one — on the one question that wrecks a machine. A
    /// failed check now raises its own safety finding.
    pub holder_collisions: &'a [(ToolpathId, HolderCollisionCheck)],
    /// Tool diameter per toolpath, for the spatial bucket edge.
    pub tool_diameters_mm: &'a BTreeMap<ToolpathId, f64>,
    /// G-OVERCUTREF — the commanded depth per pass of each toolpath that
    /// steps down, in mm: the step its levels really cut. The session
    /// builder writes the realised step of a 2.5D op
    /// (`ops::depth::realised_step_down` of its total depth and its depth
    /// per pass) and the raw `depth_per_pass()` of a 3D op that has no
    /// total depth. A toolpath absent from the map has no commanded step,
    /// and the standing-material rule reads it against its own median
    /// bite.
    pub commanded_depth_per_pass_mm: &'a BTreeMap<ToolpathId, f64>,
    /// Phase 4 — the per-toolpath kinematic reading, keyed by toolpath id.
    /// Built by [`crate::session::ProjectSession::kinematic_utilizations`], or
    /// borrowed from the load report's rows.
    /// An EMPTY map means the caller measured nothing, so
    /// `project.plunge_class_load` is simply absent — it never reads as
    /// clean.
    pub kinematic_utilization: &'a BTreeMap<ToolpathId, &'a ToolpathKinematicUtilization>,
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
        Self::build_with_rest_context(inputs, &BTreeSet::new(), &BTreeSet::new(), &BTreeMap::new())
    }

    /// [`Self::build`] plus the set of rest-driven toolpath ids
    /// (`StockSource::FromRemainingStock` — the same predicate that arms the
    /// pencil entry ramp, `session/compute.rs` `gen_initial_stock`).
    /// `project.entry_load` (G-ENTRYLOAD) is graded only on these: an entry
    /// carving rest material is the defect class the id names, while a
    /// fresh-stock entry plunge is planned motion.
    ///
    /// `links_on_mesh` is the set of toolpaths whose links were planned on
    /// the mesh with no stock reading: a finishing op
    /// (`LinkCeilingNeed::WithRemainingStock`) that is not rest-driven. Its
    /// links ride the mesh through the same stock its passes cut, so
    /// `project.link_cuts_stock` (G-LINKPLOUGH) does not grade them. See
    /// [`link_cut_observation`].
    ///
    /// `fresh_twin` holds, for each rest operation whose comparison a
    /// surface asked for and that is still current, its cycle time and the
    /// cycle time of the same operation on fresh stock
    /// (`ProjectSession::current_rest_fresh_comparisons`). See
    /// [`rest_slower_than_fresh_finding`].
    pub fn build_with_rest_context(
        inputs: &TriageInputs<'_>,
        rest_driven: &BTreeSet<ToolpathId>,
        links_on_mesh: &BTreeSet<ToolpathId>,
        fresh_twin: &BTreeMap<ToolpathId, crate::compute::toolpath_stats::FreshTwinTime>,
    ) -> Self {
        let mut safety = Vec::new();
        let mut actions = Vec::new();

        // ── Class A ────────────────────────────────────────────────────
        // Never deduped, never capped. Two collisions in one bucket stay
        // two entries: they are two places the machine will be damaged.
        for (i, hit) in inputs.rapid_collisions.iter().enumerate() {
            let (toolpath_id, evidence_move) = match hit.toolpath {
                Some((id, local)) => (Some(id), local),
                None => (None, hit.global_move),
            };
            let position = hit
                .collision
                .as_ref()
                .map_or([0.0, 0.0, 0.0], |c| [c.start.x, c.start.y, c.start.z]);
            safety.push(collision_finding(
                ids::PROJECT_RAPID_COLLISION,
                rapid_collision_message(hit, inputs.toolpath_names),
                toolpath_id,
                evidence_move,
                hit.global_move,
                position,
                i,
            ));
        }
        for (tp, check) in inputs.holder_collisions {
            if check.failed() {
                // Not a collision. The ABSENCE of an answer, which class A
                // reports because "we do not know" is a safety statement
                // and silence would read as clean (CMP-14).
                safety.push(collision_finding(
                    ids::PROJECT_HOLDER_CHECK_FAILED,
                    "Holder/shank collision check failed on this toolpath — holder clearance \
                     is UNKNOWN, not clear"
                        .to_owned(),
                    Some(*tp),
                    0,
                    0,
                    [0.0, 0.0, 0.0],
                    0,
                ));
                continue;
            }
            let count = check.collisions();
            for i in 0..count {
                safety.push(collision_finding(
                    ids::PROJECT_HOLDER_COLLISION,
                    format!("Holder/shank collision ({} on this toolpath)", count),
                    Some(*tp),
                    0,
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
        // G-LINKPLOUGH — the same pass's fed LINKS, graded against the stock.
        for tp in &inputs.trace.toolpath_summaries {
            if tp.metrics_not_applicable {
                continue;
            }
            if let Some(f) = standing_material_finding(
                inputs.trace,
                tp.toolpath_id,
                inputs
                    .commanded_depth_per_pass_mm
                    .get(&tp.toolpath_id)
                    .copied(),
                inputs.measurability.cell_mm,
            ) {
                actions.push(f);
            }
            if rest_driven.contains(&tp.toolpath_id)
                && let Some(f) = entry_load_finding(inputs.trace, tp.toolpath_id)
            {
                actions.push(f);
            }
            // G-LINKPLOUGH: see `link_cut_observation` for the scope.
            if !links_on_mesh.contains(&tp.toolpath_id)
                && let Some(f) = link_cut_finding(
                    inputs.trace,
                    tp.toolpath_id,
                    inputs
                        .toolpath_names
                        .get(&tp.toolpath_id)
                        .map(String::as_str),
                    inputs
                        .commanded_depth_per_pass_mm
                        .get(&tp.toolpath_id)
                        .copied(),
                    inputs.measurability.cell_mm,
                )
            {
                actions.push(f);
            }
        }

        // Phase 4 — the plunge-class backstop. Iterated over the utilization
        // map, NOT over `toolpath_summaries`: that list drops drill ops
        // (`metrics_not_applicable`) and covers only what the dexel run
        // touched, while this reading is over EMITTED motion and applies to
        // every operation family. It is also not gated on `rest_driven` —
        // a descent faster than its own plunge rate is a defect on fresh
        // stock exactly as it is on rest stock.
        for util in inputs.kinematic_utilization.values() {
            if let Some(f) = plunge_class_finding(util) {
                actions.push(f);
            }
        }

        // Operator ruling 2026-10-15: a rest operation that runs longer
        // than the same operation on fresh stock is told so, with both
        // times. Information, not a defect.
        for (&id, &times) in fresh_twin {
            if let Some(f) = rest_slower_than_fresh_finding(
                id,
                times,
                inputs.toolpath_names.get(&id).map(String::as_str),
            ) {
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

/// Operator ruling 2026-10-15: report a rest operation that takes longer
/// than its fresh twin.
///
/// Both times are one measure, the cycle-time model at the commanded feeds
/// of each dressed plan (`FreshTwinTime`). With feed modulation off this is
/// the simulator's own runtime; with it on, the simulated time reads the
/// modulated feeds, which the fresh twin never had. Equal or faster is
/// silent.
fn rest_slower_than_fresh_finding(
    toolpath_id: ToolpathId,
    times: crate::compute::toolpath_stats::FreshTwinTime,
    name: Option<&str>,
) -> Option<Finding> {
    let crate::compute::toolpath_stats::FreshTwinTime { rest_s, fresh_s } = times;
    if !(rest_s.is_finite() && fresh_s.is_finite()) || rest_s <= fresh_s {
        return None;
    }
    let label = name.map_or_else(
        || format!("toolpath {}", toolpath_id.0),
        |n| format!("'{n}'"),
    );
    Some(Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(ids::PROJECT_REST_SLOWER_THAN_FRESH),
            toolpath_id: Some(toolpath_id),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_REST_SLOWER_THAN_FRESH),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::Efficiency,
            severity: Severity::Info,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: format!(
                "rest operation {label} takes {rest_s:.1} s; the same operation on fresh stock \
                 takes {fresh_s:.1} s (cycle-time model, commanded feeds). Rest can take \
                 longer: each island needs its own entry and rapids."
            ),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: 1,
        worst: WorstEvidence {
            duration_s: rest_s,
            ..WorstEvidence::default()
        },
    })
}

/// Fraction of a pass's cutting samples that must sit over the reference
/// bite before the pass is reported as crossing standing material.
///
/// The census fixture measured 4.7% of samples over 10x the commanded
/// offset against 94.1% at or below 1.5x — a clear bimodal signature, not a
/// tail. 2% is well under the measured case and well over the handful of
/// samples any pass produces at a step or a corner. On the commanded arm
/// (G-OVERCUTREF) the same floor keeps a few stray samples from becoming a
/// finding; it is a population floor there, not a statistical one.
pub(crate) const STANDING_MATERIAL_SAMPLE_FRACTION: f64 = 0.02;

/// How many times its own median bite a sample must remove to count. Used
/// only on the own-median arm (an op with no commanded depth per pass).
pub(crate) const STANDING_MATERIAL_MULTIPLE: f64 = 3.0;

/// Steady-state cutting samples required before the rule reads a pass.
///
/// On the own-median arm, fewer bites give no meaningful median. On the
/// commanded arm, the 2% fraction of fewer than 50 samples is less than one
/// sample, so the fraction floor would not hold.
pub(crate) const STANDING_MATERIAL_MIN_SAMPLES: usize = 50;

/// The bite one sample is measured against.
#[derive(Debug, Clone, Copy)]
enum BiteReference {
    /// G-OVERCUTREF. The op steps down by a commanded depth per pass. A
    /// sample over-cuts when its bite exceeds `step_mm + tolerance_mm`.
    Commanded { step_mm: f64, tolerance_mm: f64 },
    /// The op has no commanded step. A sample over-cuts when its bite
    /// exceeds `STANDING_MATERIAL_MULTIPLE` times the pass's own median.
    OwnMedian { median_mm: f64 },
}

impl BiteReference {
    fn bar_mm(self) -> f64 {
        match self {
            Self::Commanded {
                step_mm,
                tolerance_mm,
            } => step_mm + tolerance_mm,
            Self::OwnMedian { median_mm } => median_mm * STANDING_MATERIAL_MULTIPLE,
        }
    }
}

/// The comparison tolerance on the commanded arm, in mm: one dexel cell.
///
/// The cell also covers the draped levels of the 3D ops that carry a step (`Adaptive3d`). When the
/// caller does not know the cell, the floor is the `f32` resolution of a
/// removed column, [`crate::tool_load::depth::DEXEL_DEPTH_RESOLUTION_MM`]
/// (EVIDENCE 5.1-6), which the depth gate uses on the same field.
fn commanded_tolerance_mm(cell_mm: Option<f64>) -> f64 {
    let floor = crate::tool_load::depth::DEXEL_DEPTH_RESOLUTION_MM;
    cell_mm
        .filter(|c| c.is_finite() && *c > 0.0)
        .map_or(floor, |c| c.max(floor))
}

/// The step one level of `operation` really cuts, in mm, or `None` when the
/// operation has no commanded depth per pass (G-OVERCUTREF).
///
/// - A 2.5D op with a total depth: the realised step,
///   [`crate::ops::depth::realised_step_down`] of the total depth and the
///   depth per pass. Generation cuts `total / ceil(total / dpp)`, which is
///   never above the requested value, so this is the depth the machine
///   cuts and not the number the operator typed.
/// - A 3D op with a step and no total depth (`Adaptive3d`, `Waterline`
///   `z_step`, `RampFinish` `max_stepdown`): the raw step.
/// - A total depth that describes no stepped cut (a `Face` at depth 0 cuts
///   one level at the top): `None`.
///
/// Every surface reads the triage through the session builder, which calls
/// this function, so the GUI, MCP and CLI see one reference.
#[must_use]
pub(crate) fn commanded_step_mm(
    operation: &crate::compute::catalog::OperationConfig,
) -> Option<f64> {
    let dpp = operation
        .depth_per_pass()
        .filter(|d| d.is_finite() && *d > 0.0)?;
    match operation.total_depth() {
        Some(total) => crate::ops::depth::realised_step_down(total.abs(), dpp),
        None => Some(dpp),
    }
}

/// The population and the reference of the standing-material rule for one
/// toolpath. The ONE definition: [`standing_material_finding`] calls it.
///
/// The population is the steady-state cutting samples of the toolpath that
/// removed material on the lateral axis; see [`standing_material_finding`]
/// for why. The reference is `None` only when the population is empty.
fn standing_material_population(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
    commanded_step: Option<f64>,
    cell_mm: Option<f64>,
) -> (
    Vec<&crate::stock::simulation_cut::SimulationCutSample>,
    Option<BiteReference>,
) {
    let population: Vec<&crate::stock::simulation_cut::SimulationCutSample> = trace
        .samples
        .iter()
        .filter(|s| {
            s.toolpath_id == toolpath_id
                && s.is_cutting
                && s.axial_engagement_mm > 0.0
                && crate::tool_load::locality::is_steady_state_for_gate(s, None)
        })
        .collect();
    let reference = if population.is_empty() {
        None
    } else {
        bite_reference(&population, commanded_step, cell_mm)
    };
    (population, reference)
}

/// The bite reference of a population: the commanded arm when the op has a
/// step, else the population's own median removed height. On the own-median
/// arm the result is `None` when the population is empty or its median is
/// zero; the commanded arm does not read the population. The ONE
/// definition: the standing-material rule and the link rule both read it.
fn bite_reference(
    population: &[&crate::stock::simulation_cut::SimulationCutSample],
    commanded_step: Option<f64>,
    cell_mm: Option<f64>,
) -> Option<BiteReference> {
    match commanded_step.filter(|s| s.is_finite() && *s > 0.0) {
        Some(step_mm) => Some(BiteReference::Commanded {
            step_mm,
            tolerance_mm: commanded_tolerance_mm(cell_mm),
        }),
        None => {
            let mut bites: Vec<f64> = population.iter().map(|s| s.axial_engagement_mm).collect();
            bites.sort_by(f64::total_cmp);
            bites
                .get(bites.len() / 2)
                .copied()
                .filter(|m| *m > 0.0)
                .map(|median_mm| BiteReference::OwnMedian { median_mm })
        }
    }
}

/// Report a pass that cuts through material deeper than it was planned to
/// cut — material an upstream operation left standing.
///
/// # The population
///
/// The steady-state cutting samples of the toolpath that removed material
/// on the lateral axis (`axial_engagement_mm > 0`). Transit samples go
/// through the one predicate,
/// [`crate::tool_load::locality::is_steady_state_for_gate`]: a link-bridge
/// sample reads `stock_top - cutter_z` over NEIGHBOURING stock, and that
/// inflated reading is not a bite. The triage has no span lookup, so the
/// predicate drops every `in_transit_span` sample.
///
/// This is a change on the own-median arm too (G-OVERCUTREF, 2026-10-12).
/// Before it, that arm read every cutting sample; now it also drops the
/// transit samples and the configured-entry samples, so its median and its
/// peak can differ from the earlier reading on the same trace.
///
/// [`standing_material_population`] holds this population and the
/// reference below.
///
/// # The reference (G-OVERCUTREF, 2026-10-12)
///
/// **An op with a commanded depth per pass** (`commanded_step` is
/// `Some`): the reference is that step, plus one dexel cell
/// ([`commanded_tolerance_mm`]). On the stock it really meets, a level
/// cuts at most one step: the earlier levels of the same pass removed the
/// material above it. So a bite over the step is material that stood
/// deeper than the plan, by construction. A fresh-stock pass at its own
/// step never fires. The median rule is NOT used for these ops. It was
/// measured wrong on a pocket: the ring overlap and the entry ramps take
/// less than a full level, so the median bite is far below the step, and
/// a pass that cut its OWN commanded depth read as "standing material".
/// The verdict also changed with the stepover, because the stepover moves
/// the median (`Channel BACK_CHANNEL_Z22.00`, dpp 0.8148: fired at
/// stepover 0.838, silent at 1.05).
///
/// **An op with no commanded step** (surface-following finishes,
/// `project_curve`, drop cutter): the reference is the pass's OWN median
/// removed height, times [`STANDING_MATERIAL_MULTIPLE`]. On these ops the
/// body bite IS the layer the upstream op left (its stock-to-leave and its
/// scallop), and it is even over the pass; so three times the median is a
/// step in the upstream stock, not a fraction of the pass's own level.
/// Census §6.4 traced a `project_curve` op reported as "20.4x commanded" —
/// a ratio formed by dividing a removed HEIGHT by a surface OFFSET, on an
/// op that has no axial step to divide by. The surviving explanation was
/// confirmed by a direct upstream-stock reading of 5.20 mm standing above
/// the cutter. That bimodal signature — a surface-following majority plus
/// a distinct population ploughing through a step — is what this arm
/// reports.
///
/// On both arms `observed` is the peak bite in mm and `threshold` is the
/// bar.
fn standing_material_finding(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
    commanded_step: Option<f64>,
    cell_mm: Option<f64>,
) -> Option<Finding> {
    let (population, reference) =
        standing_material_population(trace, toolpath_id, commanded_step, cell_mm);
    if population.len() < STANDING_MATERIAL_MIN_SAMPLES {
        // Too few bites to read; saying nothing is better than reporting
        // noise as a structural fact.
        return None;
    }
    let reference = reference?;

    let bar = reference.bar_mm();
    let over = population
        .iter()
        .filter(|s| s.axial_engagement_mm > bar)
        .count();
    let fraction = over as f64 / population.len() as f64;
    if fraction < STANDING_MATERIAL_SAMPLE_FRACTION {
        return None;
    }
    let worst_sample = population
        .iter()
        .copied()
        .max_by(|a, b| a.axial_engagement_mm.total_cmp(&b.axial_engagement_mm))?;
    let peak = worst_sample.axial_engagement_mm;

    let message = match reference {
        BiteReference::Commanded {
            step_mm,
            tolerance_mm,
        } => format!(
            "crosses standing material deeper than this pass was planned to cut: {:.1}% of cutting samples remove more than the commanded {:.2} mm depth per pass by over {:.3} mm, peaking at {:.2} mm",
            fraction * 100.0,
            step_mm,
            tolerance_mm,
            peak
        ),
        BiteReference::OwnMedian { median_mm } => format!(
            "crosses material an upstream op left standing: {:.1}% of cutting samples remove more than {:.2} mm (3x this pass's own {:.2} mm median bite), peaking at {:.2} mm",
            fraction * 100.0,
            bar,
            median_mm,
            peak
        ),
    };

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
            message,
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
        occurrences: over,
        worst: WorstEvidence {
            position: worst_sample.position,
            move_index: worst_sample.move_index,
            duration_s: 0.0,
            wasted_runtime_s: 0.0,
            min_radial_engagement: worst_sample.engagement.radial_woc_fraction,
            sample_count: over,
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
/// ([`crate::finish::pencil::entry_bite_budget_mm`]) is budgeted to stay inside.
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
pub(crate) const ENTRY_LOAD_SEVERE_PEAK_MM: f64 = 1.0;

/// Body samples required before this pass's median is a reference rather than
/// noise. Same bar, for the same reason, as `standing_material_finding`'s.
pub(crate) const ENTRY_LOAD_MIN_BODY_SAMPLES: usize = 50;

/// Noise floor (mm): a peak below this is not reported however large a
/// multiple of the median it is.
///
/// This rule reports LOAD, not ratios. On a pass whose median bite is 0.02 mm
/// — an ordinary fine finishing pass — a 0.06 mm entry is 3x the median and
/// nothing at all in the spindle. The floor is
/// [`crate::finish::pencil::ENTRY_RAMP_MIN_BITE_MM`], the smallest per-lap budget any
/// tool earns from the generator side: an entry that removes less than the
/// least a ramp lap is allowed to take is, by that same standard, not an
/// event. The case this rule exists for is 37x it.
pub const ENTRY_LOAD_MIN_PEAK_MM: f64 = crate::finish::pencil::ENTRY_RAMP_MIN_BITE_MM;

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
/// specific mistake G-ENTRYLOAD is named after. The entry rule and the link
/// rule (G-LINKPLOUGH) both read this one helper: a vertical fed link, such
/// as the descent of a 3D Rough stay-down link, carries its removal on the
/// plunge axis.
fn removed_height_mm(s: &crate::stock::simulation_cut::SimulationCutSample) -> f64 {
    s.axial_engagement_mm.max(s.plunge_descent_mm).max(0.0)
}

/// True when the generator tagged this sample's move as an entry.
///
/// Source-keyed (`source_intent`), so it survives arc-fitting and TSP
/// reordering — programme rule 5. `Drilling` is deliberately NOT an entry
/// here: drill cycles have their own three gates on
/// `ToolpathLoadVerdict::drill_gates` and their own removal model.
fn is_entry_intent(s: &crate::stock::simulation_cut::SimulationCutSample) -> bool {
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
/// `standing_material_finding` uses its own median on its no-step arm: no
/// commanded axial step exists on a surface-following op to divide by. Entries are excluded from
/// that denominator — grading a population against a median it is itself part
/// of is how a pass made mostly of bad entries exonerates itself.
pub fn entry_load_observation(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> EntryLoadObservation {
    let mut body: Vec<f64> = Vec::new();
    let mut entry_samples = 0usize;
    let mut peak: Option<f64> = None;
    let mut worst: Option<&crate::stock::simulation_cut::SimulationCutSample> = None;

    for s in trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
    {
        if is_entry_intent(s) {
            entry_samples += 1;
            let bite = removed_height_mm(s);
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
                && removed_height_mm(s) > bar
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

// ── G-LINKPLOUGH (W1.1b, 2026-10-14) ──────────────────────────────────────
//
// A link is a short FED move between two cut runs (`MoveIntent::Linking`).
// The planner means it to travel in air or on ground the op already cut. No
// check read a fed link against the stock: the rapid check reads rapids
// only, and the holder check reads the holder against the MODEL. A link that
// ploughed through standing stock was silent. This rule reads the link
// samples of the cut trace that the simulator already writes.

/// The noise floor of the link rule, in mm: a link sample that removes this
/// much or less is not a cut.
///
/// `FRESH_MATERIAL_THRESHOLD_MM` is the dexel engine's own floor for "fresh
/// material exists above the cutter" (a near-flush cell under it is float
/// noise). The engine reads a column at its cell centre. A link that rides a
/// surface the pass cut sees that surface one cell off in XY, and on a slope
/// up to 45 degrees one cell in XY is at most one cell in Z. So the floor is
/// that threshold plus one cell ([`commanded_tolerance_mm`], the tolerance
/// the standing-material rule adds for the same quantisation).
#[must_use]
pub fn link_cut_noise_floor_mm(cell_mm: Option<f64>) -> f64 {
    crate::dexel_stock::FRESH_MATERIAL_THRESHOLD_MM + commanded_tolerance_mm(cell_mm)
}

/// Removed height (mm) OVER the bar at which a link finding escalates from
/// `Caution` to `Critical`.
///
/// The same millimetre as [`ENTRY_LOAD_SEVERE_PEAK_MM`], for the same
/// reason: the only tool figure this module has is the ENVELOPE diameter,
/// which overstates a tapered tip, so a tool fraction would go quiet on the
/// small tips that break. The case transfers: an entry runs at the ramp feed,
/// which is at or under the cut feed (`entry_feed_rate()`), and a link runs at
/// the cut feed itself. So one millimetre of unplanned axial engagement is at
/// least as severe on a link as on an entry. The rule reads the EXCESS over
/// the bar, so a stepped op is Critical only at its own step plus one cell
/// plus one millimetre.
pub(crate) const LINK_CUT_SEVERE_EXCESS_MM: f64 = ENTRY_LOAD_SEVERE_PEAK_MM;

/// What the link rule measured on one toolpath. It is published whether or
/// not the rule fires, so "no fed link" and "the links were clean" never read
/// as the same thing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkCutObservation {
    /// Fed samples whose generator intent is `Linking`. **The population.**
    /// Rapid samples are not in it: a rapid through stock is the rapid
    /// collision check's channel.
    pub link_samples: usize,
    /// Steady-state cutting samples that are not links and removed material:
    /// the pass's own bites, the population of the own-median reference.
    pub body_samples: usize,
    /// [`link_cut_noise_floor_mm`] for this trace's cell.
    pub noise_floor_mm: f64,
    /// The pass's own reference bar (mm): the commanded step plus one cell,
    /// or 3x the own median bite. `None` when the op has no step and the
    /// body has fewer than [`STANDING_MATERIAL_MIN_SAMPLES`] bites.
    pub pass_bar_mm: Option<f64>,
    /// True when `pass_bar_mm` is the commanded arm.
    pub pass_bar_is_commanded: bool,
    /// `max(noise_floor_mm, pass_bar_mm)`. `None` when not measured.
    pub bar_mm: Option<f64>,
    /// Peak removed height (mm) over the link samples,
    /// `max(axial_engagement_mm, plunge_descent_mm)`. `None` = no link
    /// samples, never 0.0.
    pub peak_removed_mm: Option<f64>,
    /// Link samples over the bar.
    pub over_bar_samples: usize,
    /// Distinct link moves with a sample over the bar.
    pub over_bar_moves: usize,
    pub worst_position: Option<[f64; 3]>,
    pub worst_move_index: Option<usize>,
    pub worst_sample_index: Option<usize>,
}

impl LinkCutObservation {
    /// True when the rule has a population and a bar. False means "not
    /// measured", never "clean".
    pub fn is_measured(&self) -> bool {
        self.link_samples > 0 && self.bar_mm.is_some()
    }

    /// True when a measured link sample removes more than the bar.
    pub fn exceeds(&self) -> bool {
        self.is_measured() && self.over_bar_samples > 0
    }
}

/// True when the generator tagged this sample's move as a link.
fn is_link_intent(s: &crate::stock::simulation_cut::SimulationCutSample) -> bool {
    s.source_intent == Some(crate::toolpath::MoveIntent::Linking)
}

/// Measure one toolpath's fed links against the stock they cross.
///
/// # The population
///
/// The FED samples (`is_cutting`) whose source intent is `Linking`. The
/// bite is [`removed_height_mm`], on both removal axes. The rule does NOT
/// filter transit samples: a link sample is a transit sample by
/// definition, and what the simulator removes there is exactly what this
/// rule reports. The load gates drop it because it is not a steady-state
/// load, not because the removal is false.
///
/// # The bar
///
/// The bar is the larger of two numbers:
///
/// - [`link_cut_noise_floor_mm`]: a link that rides the cut surface removes
///   no more than this.
/// - The pass's own bite ([`bite_reference`], the standing-material
///   reference): the commanded step plus one cell for a stepped op, else 3x
///   the median bite of the pass's non-link cutting samples. A link that
///   takes no more than the pass itself is not reported, for two reasons.
///   A 2D Adaptive keep-down link is RULED to cut within the pass load
///   (G-ADAPTLINKLOAD), and a 3D Rough link is read against its own step.
///
/// # The scope
///
/// `project.entry_load` reads only rest-driven toolpaths, because a
/// fresh-stock entry descends through stock the generator planned for. A
/// link is different: the generator plans most links in air or on cut
/// ground on fresh stock too. Project Curve always builds a ceiling, and
/// the G-LINKCEIL defect (a pinned `top_z` put every fed link inside the
/// stock) was a fresh-stock case. So the triage grades every toolpath
/// except one class, the `links_on_mesh` set of
/// [`SimulationTriage::build_with_rest_context`]: a finishing op
/// (`LinkCeilingNeed::WithRemainingStock`) that is not rest-driven builds
/// no ceiling by design. Its links ride the mesh through the same stock its
/// passes cut, so the whole op meets raw stock, and the link is not the
/// defect. Measured on the G-LINKPLOUGH corrugated plate on fresh stock:
/// the raster links removed up to 5.03 mm against a 3.29 mm pass bar, and
/// the pencil links up to 4.99 mm.
#[must_use]
pub fn link_cut_observation(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
    commanded_step: Option<f64>,
    cell_mm: Option<f64>,
) -> LinkCutObservation {
    let mut obs = LinkCutObservation {
        noise_floor_mm: link_cut_noise_floor_mm(cell_mm),
        ..LinkCutObservation::default()
    };
    let mut links: Vec<&crate::stock::simulation_cut::SimulationCutSample> = Vec::new();
    let mut body: Vec<&crate::stock::simulation_cut::SimulationCutSample> = Vec::new();
    for s in trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
    {
        if is_link_intent(s) {
            links.push(s);
        } else if s.axial_engagement_mm > 0.0
            && crate::tool_load::locality::is_steady_state_for_gate(s, None)
        {
            body.push(s);
        }
    }
    obs.link_samples = links.len();
    obs.body_samples = body.len();
    let worst = links
        .iter()
        .copied()
        .max_by(|a, b| removed_height_mm(a).total_cmp(&removed_height_mm(b)));
    obs.peak_removed_mm = worst.map(removed_height_mm);
    obs.worst_position = worst.map(|s| s.position);
    obs.worst_move_index = worst.map(|s| s.move_index);
    obs.worst_sample_index = worst.map(|s| s.sample_index);

    let commanded = commanded_step.is_some_and(|s| s.is_finite() && s > 0.0);
    let reference = if commanded || body.len() >= STANDING_MATERIAL_MIN_SAMPLES {
        bite_reference(&body, commanded_step, cell_mm)
    } else {
        None
    };
    let Some(reference) = reference else {
        return obs;
    };
    let pass_bar = reference.bar_mm();
    let bar = pass_bar.max(obs.noise_floor_mm);
    obs.pass_bar_mm = Some(pass_bar);
    obs.pass_bar_is_commanded = matches!(reference, BiteReference::Commanded { .. });
    obs.bar_mm = Some(bar);
    let over: Vec<&crate::stock::simulation_cut::SimulationCutSample> = links
        .iter()
        .copied()
        .filter(|s| removed_height_mm(s) > bar)
        .collect();
    obs.over_bar_samples = over.len();
    obs.over_bar_moves = over
        .iter()
        .map(|s| s.move_index)
        .collect::<BTreeSet<usize>>()
        .len();
    obs
}

/// Report a toolpath whose fed LINK cuts stock it was planned to cross in
/// air or on cut ground (G-LINKPLOUGH). See [`link_cut_observation`] for
/// the population and the bar, and [`LINK_CUT_SEVERE_EXCESS_MM`] for the
/// severity.
///
/// Fires only on a measured reading; an op with no fed link is absent from
/// the list, never reported clean.
fn link_cut_finding(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
    toolpath_name: Option<&str>,
    commanded_step: Option<f64>,
    cell_mm: Option<f64>,
) -> Option<Finding> {
    let obs = link_cut_observation(trace, toolpath_id, commanded_step, cell_mm);
    if !obs.exceeds() {
        return None;
    }
    let peak = obs.peak_removed_mm?;
    let bar = obs.bar_mm?;
    let position = obs.worst_position.unwrap_or_default();
    let move_index = obs.worst_move_index.unwrap_or_default();
    let sample_index = obs.worst_sample_index.unwrap_or_default();
    let severity = if peak - bar >= LINK_CUT_SEVERE_EXCESS_MM {
        Severity::Critical
    } else {
        Severity::Caution
    };
    let reference = match (obs.pass_bar_mm, obs.pass_bar_is_commanded) {
        (Some(pass), true) if pass >= obs.noise_floor_mm => {
            format!("this pass's commanded depth per pass plus one cell, {pass:.2} mm")
        }
        (Some(pass), false) if pass >= obs.noise_floor_mm => {
            format!("3x this pass's own median bite, {pass:.2} mm")
        }
        _ => format!(
            "the {:.2} mm noise floor of a link on cut ground",
            obs.noise_floor_mm
        ),
    };
    let name = toolpath_name.map_or_else(String::new, |n| format!("\"{n}\": "));
    Some(Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(ids::PROJECT_LINK_CUTS_STOCK),
            toolpath_id: Some(toolpath_id),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_LINK_CUTS_STOCK),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::ToolLoad,
            severity,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: format!(
                "{name}a fed link cuts stock it was planned to cross in air or on cut ground: \
                 link move {move_index} removes {peak:.2} mm at ({:.1}, {:.1}, {:.2}). \
                 {} of {} link samples on {} link moves remove more than {bar:.2} mm \
                 ({reference}).",
                position[0],
                position[1],
                position[2],
                obs.over_bar_samples,
                obs.link_samples,
                obs.over_bar_moves,
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
        occurrences: obs.over_bar_samples,
        worst: WorstEvidence {
            position,
            move_index,
            duration_s: 0.0,
            wasted_runtime_s: 0.0,
            min_radial_engagement: 0.0,
            sample_count: obs.over_bar_samples,
        },
    })
}

/// Report a pass whose VERTICAL-DOMINANT fed motion descends faster than the
/// operation's own `plunge_rate` (Phase 4, 2026-09-07).
///
/// The physics: a sloped fed descent is a RAMP — the flutes cut laterally at
/// the ramp angle and the chipload gate governs it, so its Z component
/// exceeding `plunge_rate` is not a hazard. Only a vertical-dominant descent
/// is a plunge (centre cutting, chip evacuation, tip load), and only that
/// class is graded here. [`crate::machine::kinematic_utilization`] does the
/// classification; this function only reads the verdict.
///
/// NON-BLOCKING. `fix: None`, no export gate reads it, and it grades EMITTED
/// motion after the machine's rate clamp — not the plan, and not a configured
/// number. Complementary to the static `project.plunge_stress` rule, which
/// checks the configured plunge rate against a ball-tip cap and excludes flat
/// and V tools.
///
/// Fires only on a measured reading with a real population
/// ([`ToolpathKinematicUtilization::plunge_is_measured`]); an op with no
/// plunge-class moves is absent from the list, never reported clean.
pub fn plunge_class_finding(util: &ToolpathKinematicUtilization) -> Option<Finding> {
    if !util.plunge_is_measured() {
        return None;
    }
    let ratio = util.plunge.peak_ratio?;
    if ratio <= 1.0 {
        return None;
    }
    let severity = if ratio > 2.0 {
        Severity::Critical
    } else {
        Severity::Caution
    };
    let toolpath_id = util.toolpath_id;
    let plunge_rate = util.plunge.plunge_rate_mm_min;
    // The achieved Z rate the ratio was taken from. When the kernel did not
    // publish it, restate the ratio against the op's own plunge rate — the
    // same quantity, expressed from the number that IS published, rather
    // than a zero standing in for a missing measurement.
    let achieved_z = util
        .plunge
        .worst_achieved_z_rate_mm_min
        .unwrap_or(ratio * plunge_rate);
    let position = util.plunge.worst_position;
    let at = position.unwrap_or_default();
    let move_index = util.plunge.worst_move_index;

    Some(Finding {
        dedup_key: DedupKey {
            id: DiagnosticId::from(ids::PROJECT_PLUNGE_CLASS_LOAD),
            toolpath_id: Some(toolpath_id),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(ids::PROJECT_PLUNGE_CLASS_LOAD),
            scope: Scope::Toolpath { id: toolpath_id },
            category: Category::ToolLoad,
            severity,
            confidence: Confidence::Verified,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: format!(
                "plunge-class motion descends faster than this op's own plunge rate: {} of \
                 {} vertical-dominant moves exceed {:.0} mm/min, peaking at {:.0} mm/min \
                 ({:.1}x) at move {} ({:.1},{:.1},{:.2}). The modulator lifts untagged \
                 descents to the lateral band; the load gates skip nothing here — this \
                 reading is emitted motion after the machine's rate clamp.",
                util.plunge.over_1x,
                util.plunge.population,
                plunge_rate,
                achieved_z,
                ratio,
                move_index.unwrap_or_default(),
                at[0],
                at[1],
                at[2],
            ),
            evidence: move_index.map(|move_index| DiagnosticEvidence::Move {
                toolpath_id,
                move_index,
                position,
            }),
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: util.plunge.over_1x.max(1),
        worst: WorstEvidence {
            position: at,
            move_index: move_index.unwrap_or_default(),
            duration_s: 0.0,
            wasted_runtime_s: 0.0,
            min_radial_engagement: 0.0,
            sample_count: util.plunge.over_1x,
        },
    })
}

/// The triage line of one rapid collision (G-RAPIDFRAME).
///
/// It states what the simulation measured and nothing more: the toolpath,
/// the move in both frames, and the start and end of the rapid. The
/// coordinates are in the simulation frame of the toolpath's setup. The
/// line gives no cause, because the simulation does not measure one.
fn rapid_collision_message(
    hit: &AttributedRapidCollision,
    names: &BTreeMap<ToolpathId, String>,
) -> String {
    let at = match hit.toolpath {
        Some((id, local)) => {
            let name = names
                .get(&id)
                .map_or_else(String::new, |name| format!(" '{name}'"));
            format!(
                "on TP{id}{name} at local move {local} (run move {global})",
                global = hit.global_move
            )
        }
        None => format!(
            "at run move {} (no simulated toolpath holds this move)",
            hit.global_move
        ),
    };
    match hit.collision.as_ref() {
        Some(c) => format!(
            "Rapid passes through uncut stock {at}: ({:.1}, {:.1}, {:.3}) -> \
             ({:.1}, {:.1}, {:.3})",
            c.start.x, c.start.y, c.start.z, c.end.x, c.end.y, c.end.z
        ),
        None => format!("Rapid passes through uncut stock {at}"),
    }
}

/// One class-A finding.
///
/// `move_index` is the move the diagnostic evidence names: the toolpath's
/// OWN move index when `toolpath_id` is `Some`. `run_move` is the run-global
/// move index; it orders the findings in run order.
fn collision_finding(
    id: &str,
    message: String,
    toolpath_id: Option<ToolpathId>,
    move_index: usize,
    run_move: usize,
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
            move_index: run_move,
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
    use crate::stock::collision::RapidCollision;
    use crate::stock::simulation_cut::{Engagement, SimulationCutSample};

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

    /// No toolpath names: the collision lines print the id alone.
    static NO_NAMES: BTreeMap<ToolpathId, String> = BTreeMap::new();

    /// No commanded steps: every toolpath is read on the own-median arm.
    static NO_STEPS: BTreeMap<ToolpathId, f64> = BTreeMap::new();

    /// A rapid collision that no boundary holds, at run move `move_index`.
    fn unattributed(rc: RapidCollision) -> AttributedRapidCollision {
        AttributedRapidCollision {
            global_move: rc.move_index,
            toolpath: None,
            collision: Some(rc),
        }
    }

    fn inputs<'a>(
        trace: &'a SimulationCutTrace,
        m: &'a MeasurabilityReport,
        diags: &'a [Diagnostic],
        rapids: &'a [AttributedRapidCollision],
        holders: &'a [(ToolpathId, HolderCollisionCheck)],
        diameters: &'a BTreeMap<ToolpathId, f64>,
        kinematics: &'a BTreeMap<ToolpathId, &'a ToolpathKinematicUtilization>,
    ) -> TriageInputs<'a> {
        TriageInputs {
            trace,
            measurability: m,
            diagnostics: diags,
            rapid_collisions: rapids,
            toolpath_names: &NO_NAMES,
            holder_collisions: holders,
            tool_diameters_mm: diameters,
            commanded_depth_per_pass_mm: &NO_STEPS,
            kinematic_utilization: kinematics,
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
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d, &BTreeMap::new()));

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

        let rapid = unattributed(RapidCollision {
            move_index: 4211,
            start: crate::geo::P3::new(3.0, 4.0, 5.0),
            end: crate::geo::P3::new(9.0, 4.0, 5.0),
        });
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
            &[(ToolpathId(1), HolderCollisionCheck::Measured(1))],
            &d,
            &BTreeMap::new(),
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
            unattributed(RapidCollision {
                move_index: 10,
                start: crate::geo::P3::new(1.0, 1.0, 1.0),
                end: crate::geo::P3::new(2.0, 1.0, 1.0),
            }),
            unattributed(RapidCollision {
                move_index: 11,
                start: crate::geo::P3::new(1.5, 1.0, 1.0),
                end: crate::geo::P3::new(2.5, 1.0, 1.0),
            }),
        ];
        let m = MeasurabilityReport::default();
        let d = BTreeMap::new();
        let k = BTreeMap::new();
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &rapids, &[], &d, &k));

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
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d, &BTreeMap::new()));

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
        let t = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d, &BTreeMap::new()));
        assert!(
            !t.actions
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_CROSSES_STANDING_MATERIAL),
            "an even pass must not be reported"
        );
    }

    // ── G-OVERCUTREF ───────────────────────────────────────────────────
    //
    // A stepped op is read against its commanded step plus one cell. The
    // sentry `tests/a_fresh_stock_pass_at_its_own_depth_is_not_an_overcut_g_overcutref.rs`
    // holds the fresh-stock and the thick-bite cases; these pin the
    // tolerance, the population and the session's step.

    /// The realised step of the measured pocket: 22 mm at a requested
    /// 0.8148 mm is `22 / ceil(22 / 0.8148)` = 22 / 27.
    const STEP_MM: f64 = 22.0 / 27.0;
    /// The dexel cell of these hand-built traces, in mm.
    const CELL_MM: f64 = 0.25;

    /// 200 cutting samples on toolpath 1 at `step`, with the first
    /// `n_odd` at `odd` and flagged `in_transit_span` when `transit`.
    fn stepped_trace(n_odd: usize, odd: f64, transit: bool) -> SimulationCutTrace {
        let samples = (0..200)
            .map(|i| SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                is_cutting: true,
                axial_engagement_mm: if i < n_odd { odd } else { STEP_MM },
                in_transit_span: transit && i < n_odd,
                engagement: Engagement::with_radial_woc(0.3),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            })
            .collect();
        SimulationCutTrace::from_samples(0.5, samples)
    }

    fn stepped_finding(trace: &SimulationCutTrace) -> Option<Finding> {
        let m = MeasurabilityReport {
            cell_mm: Some(CELL_MM),
            ..MeasurabilityReport::default()
        };
        let d = BTreeMap::new();
        let steps: BTreeMap<ToolpathId, f64> = [(ToolpathId(1), STEP_MM)].into_iter().collect();
        let k = BTreeMap::new();
        let t = SimulationTriage::build(&TriageInputs {
            commanded_depth_per_pass_mm: &steps,
            ..inputs(trace, &m, &[], &[], &[], &d, &k)
        });
        t.actions
            .into_iter()
            .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_CROSSES_STANDING_MATERIAL)
    }

    #[test]
    fn g_overcutref_a_bite_inside_one_cell_of_the_step_is_not_an_overcut() {
        // 10 % of samples at step + half a cell: the dexel cannot tell this
        // from the step.
        let f = stepped_finding(&stepped_trace(20, STEP_MM + 0.5 * CELL_MM, false));
        assert!(f.is_none(), "a bite inside one cell must not fire: {f:?}");
    }

    #[test]
    fn g_overcutref_a_bite_two_cells_over_the_step_is_an_overcut() {
        let deep = STEP_MM + 2.0 * CELL_MM;
        let f = stepped_finding(&stepped_trace(20, deep, false))
            .expect("a bite two cells over the step must fire");
        assert_eq!(f.occurrences, 20);
        match f.diagnostic.evidence {
            Some(DiagnosticEvidence::SampleRange {
                observed,
                threshold,
                ..
            }) => {
                assert!((observed - deep).abs() < 1e-9, "observed {observed}");
                let bar = threshold.expect("the bar is published");
                assert!((bar - (STEP_MM + CELL_MM)).abs() < 1e-9, "bar {bar}");
            }
            other => panic!("expected SampleRange evidence, got {other:?}"),
        }
    }

    #[test]
    fn g_overcutref_the_tolerance_is_one_cell_and_the_f32_floor_without_one() {
        let floor = crate::tool_load::depth::DEXEL_DEPTH_RESOLUTION_MM;
        assert!((commanded_tolerance_mm(Some(CELL_MM)) - CELL_MM).abs() < 1e-15);
        assert!((commanded_tolerance_mm(None) - floor).abs() < 1e-15);
        assert!((commanded_tolerance_mm(Some(0.0)) - floor).abs() < 1e-15);
        assert!((commanded_tolerance_mm(Some(f64::NAN)) - floor).abs() < 1e-15);
    }

    #[test]
    fn g_overcutref_a_transit_sample_is_not_a_bite() {
        // A link-bridge sample reads `stock_top - cutter_z` over the
        // neighbouring stock. The one steady-state predicate drops it.
        let f = stepped_finding(&stepped_trace(20, 3.0 * STEP_MM, true));
        assert!(f.is_none(), "phantom transit readings must not fire: {f:?}");
    }

    #[test]
    fn g_overcutref_the_session_step_is_the_realised_step() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::{FaceConfig, PocketConfig, ScallopConfig};

        // 22 mm at 0.9 mm asks for 24.4 levels, so the pocket cuts 25
        // levels of 0.88 mm.
        let pocket = OperationConfig::Pocket(PocketConfig {
            depth: 22.0,
            depth_per_pass: 0.9,
            ..PocketConfig::default()
        });
        let step = commanded_step_mm(&pocket).expect("a pocket steps down");
        assert!((step - 22.0 / 25.0).abs() < 1e-12, "step {step}");

        // A face at depth 0 cuts one level at the top: no commanded step.
        let face = OperationConfig::Face(FaceConfig {
            depth: 0.0,
            ..FaceConfig::default()
        });
        assert!(commanded_step_mm(&face).is_none());

        // A surface-following finish has no step at all.
        let scallop = OperationConfig::Scallop(ScallopConfig::default());
        assert!(commanded_step_mm(&scallop).is_none());
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
            &BTreeMap::new(),
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
            standing_material_finding(&trace, ToolpathId(1), None, None).is_none(),
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
        let k = BTreeMap::new();
        let plain = SimulationTriage::build(&inputs(&trace, &m, &[], &[], &[], &d, &k));
        assert!(
            !plain
                .actions
                .iter()
                .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_ENTRY_LOAD),
            "entry_load must stay silent without rest context"
        );

        let rest: BTreeSet<ToolpathId> = [ToolpathId(1)].into_iter().collect();
        let t = SimulationTriage::build_with_rest_context(
            &inputs(&trace, &m, &[], &[], &[], &d, &BTreeMap::new()),
            &rest,
            &BTreeSet::new(),
            &BTreeMap::new(),
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
