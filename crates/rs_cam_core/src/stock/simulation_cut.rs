use crate::ids::ToolpathId;
use crate::ops::drill_metrics::{DrillSample, DrillToolpathSummary};
use crate::trace::debug_trace::TOOLPATH_DEBUG_SCHEMA_VERSION;
use crate::trace::semantic_trace::ToolpathSemanticKind;
use crate::trace::toolpath_spans::SpanId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod accumulate;
mod analysis;
mod reporting;

pub use accumulate::accumulate_by_span;
pub(crate) use reporting::publish_cycle_times;
pub use reporting::{
    prune_simulation_cut_artifacts, rebase_cutting_times, write_simulation_cut_artifact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationMetricOptions {
    pub enabled: bool,
    #[serde(default)]
    pub capture_arc_engagement: bool,
}

// v6 (2026-09-17, STK-04 + STK-05): `Engagement` loses
// `leading_edge_speed_mm_min` (an unconditional copy of `feed_rate_mm_min`)
// and `direction` (an `EngagementDirection` that only ever said `Mixed`), and
// `KinematicsSummary` loses `average_leading_edge_speed_mm_min`.
// v5 (2026-06-10, F1): `DrillToolpathSummary` gains `chip_welding_dtd`
// (evacuation-credited ratio the chip-welding risk is classified from).
pub const SIMULATION_CUT_TRACE_SCHEMA_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum CutKinematics {
    Linear = 0,
    Plunge = 1,
    Helix = 2,
    Arc = 3,
    #[default]
    Rapid = 4,
}

impl CutKinematics {
    pub const COUNT: usize = 5;

    pub const ALL: [CutKinematics; Self::COUNT] = [
        CutKinematics::Linear,
        CutKinematics::Plunge,
        CutKinematics::Helix,
        CutKinematics::Arc,
        CutKinematics::Rapid,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Structured engagement vector carried per `SimulationCutSample`. Step 2
/// of the dexel-fidelity roadmap (see `planning/DEXEL_Z_ONLY_INVESTIGATION.md`
/// §6.H + §10.3 tail PR) — the canonical engagement representation. The
/// legacy `radial_engagement: f64` scalar was removed in the §10.3 follow-up
/// PR (2026-05-20); read `engagement.radial_woc_fraction` instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Engagement {
    /// 0..1 — cylinder-side width-of-cut as a fraction of cutter diameter.
    pub radial_woc_fraction: f64,
    /// 0..1 — axial depth-of-cut as a fraction of flute length.
    ///
    /// **`None` = not measured** (C2, 2026-07-30): the emitter had no flute
    /// length to divide by — legacy traces, drill/analytical samples, and
    /// fixtures built from `Engagement::default()`. It used to be `0.0`, which
    /// consumers were asked *by a doc comment* to read as "unknown" rather
    /// than "no axial engagement"; that is exactly the silent-sentinel class
    /// A/M9 retired for standing material, so the ask is now a type.
    /// `Some(0.0)` is a measured zero. The dexel simulator always measures.
    /// Use `axial_doc_mm` on the sample for the absolute reading.
    #[serde(default)]
    pub axial_doc_fraction: Option<f64>,
    /// Engagement arc in radians (entry → exit). `None` for plunges and
    /// other Z-only moves where the concept does not apply.
    pub arc_radians: Option<f64>,
    /// Arc-AVERAGE chip thickness (mm of *chip*) — the mean of
    /// instantaneous chip thickness over the engagement arc,
    /// `ChipGeometry::mean_chip_thickness_mm` via
    /// [`crate::dexel_stock::chip_thickness_stats`]. Same value the
    /// chipload gate reads off
    /// [`SimulationCutSample::effective_chip_thickness_mm`].
    ///
    /// **This is NOT the commanded advance per tooth.** That quantity is
    /// [`SimulationCutSample::chipload_mm_per_tooth`], and below full
    /// slotting the two differ by `(2/arc)·(1 − cos(arc/2))·sin(arc)`
    /// — a factor of ~0.373 at half immersion.
    ///
    /// F-4 (census T1.3, 2026-08-04): this field used to be assigned the
    /// commanded advance per tooth while its doc comment claimed the
    /// arc-mean. Corrected at the emitter, not by rewording.
    pub mean_chip_thickness_mm: Option<f64>,
    /// Arc-PEAK chip thickness (mm of *chip*) — what the flute
    /// experiences at its most-engaged angular position,
    /// `ChipGeometry::max_chip_thickness_mm` via
    /// [`crate::dexel_stock::peak_chip_thickness_mm`].
    ///
    /// Always `>= mean_chip_thickness_mm`. F-4: this field used to be
    /// assigned the arc-MEAN, so it read *below* the sibling named
    /// "mean" on every partial-immersion cut.
    ///
    /// No gate consumes it — the chipload gate is deliberately
    /// calibrated on the arc-average (`tests/chipload_formula_calibration.rs`).
    pub peak_chip_thickness_mm: Option<f64>,
}

impl Engagement {
    /// Convenience for tests/fixtures that only need to express the radial
    /// width-of-cut fraction; remaining axes take their `Default` values.
    pub fn with_radial_woc(radial_woc_fraction: f64) -> Self {
        Self {
            radial_woc_fraction,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationProvenance {
    pub trace_schema_version: u32,
    pub captured_arc_engagement: bool,
    pub toolpath_hashes: BTreeMap<ToolpathId, u64>,
    /// Per-toolpath hash of the tool geometry used by that toolpath.
    /// NB: keyed by **toolpath** id (like `toolpath_hashes`), not tool id.
    pub tool_hashes: BTreeMap<ToolpathId, u64>,
    /// Hash of each toolpath's `OperationConfig` at sim time. New in
    /// PR-4 polish — lets [`crate::gcode::sim_trace_is_fresh`] catch
    /// config-only edits (e.g. `feed_rate` changes that don't change
    /// move geometry but do invalidate cached load verdicts). Empty
    /// for traces captured before this field landed; the freshness
    /// check treats a missing entry as a config match for
    /// backward-compatibility.
    #[serde(default)]
    pub operation_config_hashes: BTreeMap<ToolpathId, u64>,
    pub stock_hash: u64,
    pub machine_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationCutIssueKind {
    AirCut,
    LowEngagement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutSample {
    pub toolpath_id: ToolpathId,
    pub move_index: usize,
    pub sample_index: usize,
    pub position: [f64; 3],
    pub cumulative_time_s: f64,
    pub segment_time_s: f64,
    pub is_cutting: bool,
    #[serde(default)]
    pub cut_kinematics: CutKinematics,
    pub feed_rate_mm_min: f64,
    pub spindle_rpm: u32,
    pub flute_count: u32,
    /// Legacy wire name for axial cutting engagement. Pure-vertical plunges
    /// now report `0.0` here; read `plunge_descent_mm` for Z-only descent.
    pub axial_doc_mm: f64,
    /// Maximum material height engaged by lateral/arc/helix cutting at this
    /// sample. Deflection and chip-geometry gates consume this axis.
    #[serde(default)]
    pub axial_engagement_mm: f64,
    /// Z descent represented by a pure-vertical plunge sample. Lateral/arc
    /// samples leave this at zero so DOC and plunge distance do not share one
    /// scalar.
    #[serde(default)]
    pub plunge_descent_mm: f64,
    #[serde(default)]
    pub arc_engagement_radians: Option<f64>,
    /// Commanded feed per tooth: feed_rate / spindle_rpm / flute_count.
    pub chipload_mm_per_tooth: f64,
    #[serde(default)]
    pub effective_chip_thickness_mm: Option<f64>,
    /// Structured engagement vector — see [`Engagement`]. Production samples
    /// (dexel simulator) populate every applicable axis. Legacy traces
    /// deserialised without this field receive `Engagement::default()` and
    /// will report zero engagement; bump the file's schema version if you
    /// want to reject pre-v4 traces explicitly.
    #[serde(default)]
    pub engagement: Engagement,
    pub removed_volume_est_mm3: f64,
    pub mrr_mm3_s: f64,
    pub semantic_item_id: Option<u64>,
    /// Indices into [`crate::trace::toolpath_spans::AnnotatedToolpath::spans`] for
    /// every non-boundary span covering this sample's move. Outermost-first
    /// (Operation, then DepthPass, …). Empty when the toolpath had no spans.
    #[serde(default)]
    pub span_path: Vec<SpanId>,
    /// True when this sample's move sits in a transit-style span (Entry,
    /// LeadOut, LinkBridge, WaterlineCleanup, DressupArtifact). The dexel
    /// reading at transit samples reports `stock_top − cutter_z` over
    /// neighbouring stock, not steady-state engagement; extreme-value
    /// metrics (`peak_axial_doc_mm`, `peak_chipload_mm_per_tooth`) skip
    /// transit samples to avoid lift-bridge artifacts.
    /// P3 — see `planning/P3_TRANSIT_PEAK_DOC_RCA.md`.
    #[serde(default)]
    pub in_transit_span: bool,
    /// **Source role** of the move this sample was emitted from — the
    /// generator's own [`crate::toolpath::MoveIntent`] tag, carried through
    /// unmodified.
    ///
    /// R-11 (census §8.2 / Checkpoint D). Before this field the only
    /// per-sample role handle was the collapsed boolean
    /// [`Self::in_transit_span`], which answers "was this move in a
    /// transit-style span" and nothing else. Any probe that wanted to group
    /// samples by *what the generator meant them to be* had to re-join
    /// through `move_index` against the annotated toolpath — see
    /// `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §6.5 item 3 — and there was no
    /// MCP route to it at all.
    ///
    /// This is deliberately a **source** key (programme rule 5): it survives
    /// arc-fitting, TSP reordering and every other post-transform relabel,
    /// because it is what the generator emitted, not what a later pass
    /// inferred.
    ///
    /// **`None` = not carried**, never "Unknown": legacy traces
    /// deserialised before this field existed, and hand-built test fixtures.
    /// `Some(MoveIntent::Unknown)` is the distinct case of a generator that
    /// emitted a move without tagging it. Do not coerce one to the other.
    #[serde(default)]
    pub source_intent: Option<crate::toolpath::MoveIntent>,
}

impl SimulationCutSample {
    /// Neutral test fixture: a non-cutting sample at the origin with zeroed
    /// metrics. Test code overrides the fields under test via struct-update
    /// syntax. Not for production paths.
    pub fn test_fixture() -> Self {
        Self {
            toolpath_id: ToolpathId(0),
            move_index: 0,
            sample_index: 0,
            position: [0.0, 0.0, 0.0],
            cumulative_time_s: 0.0,
            segment_time_s: 0.0,
            is_cutting: false,
            cut_kinematics: CutKinematics::default(),
            feed_rate_mm_min: 0.0,
            spindle_rpm: 0,
            flute_count: 0,
            axial_doc_mm: 0.0,
            axial_engagement_mm: 0.0,
            plunge_descent_mm: 0.0,
            arc_engagement_radians: None,
            chipload_mm_per_tooth: 0.0,
            effective_chip_thickness_mm: None,
            engagement: Engagement::default(),
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: None,
            span_path: Vec::new(),
            in_transit_span: false,
            source_intent: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutIssue {
    pub kind: SimulationCutIssueKind,
    pub toolpath_id: ToolpathId,
    /// First move in the issue segment.
    pub move_index: usize,
    /// First sample in the issue segment.
    pub sample_index: usize,
    /// Cumulative time at the first sample in the segment (seconds).
    pub cumulative_time_s: f64,
    /// Position at the first sample in the segment.
    pub position: [f64; 3],
    /// Radial engagement at the first sample (always < 0.02 for AirCut,
    /// < 0.10 for LowEngagement). For the worst engagement in the segment
    /// use `min_radial_engagement`.
    pub radial_engagement: f64,
    pub semantic_item_id: Option<u64>,
    pub label: String,

    /// Last move covered by the issue segment.
    #[serde(default)]
    pub end_move_index: usize,
    /// Last sample covered by the issue segment.
    #[serde(default)]
    pub end_sample_index: usize,
    /// Cumulative time at the last sample in the segment (seconds).
    #[serde(default)]
    pub end_cumulative_time_s: f64,
    /// Position at the last sample in the segment.
    #[serde(default)]
    pub end_position: [f64; 3],
    /// Total samples coalesced into this segment (≥ 1).
    #[serde(default = "default_sample_count")]
    pub sample_count: usize,
    /// Duration from first to last sample in the segment (seconds).
    #[serde(default)]
    pub duration_s: f64,
    /// Minimum radial engagement across all samples in the segment.
    #[serde(default)]
    pub min_radial_engagement: f64,

    /// Span path inherited from the first sample in the segment. Empty when
    /// the toolpath had no spans.
    #[serde(default)]
    pub span_path: Vec<SpanId>,
}

fn default_sample_count() -> usize {
    1
}

/// Per-`CutKinematics` summary block emitted alongside the scalar summary
/// fields. Step 2 of the dexel-fidelity roadmap (see
/// `planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.D) — lets readers ask the
/// engagement question that applies to their op kind without committing to
/// whether the scalar `average_engagement` means what they expect.
///
/// Each kinematics class carries the axes that are well-defined for it:
/// `Linear` reports radial-WOC + arc + chip thickness; `Plunge` reports
/// axial-DOC + leading-edge speed; `Helix`/`Arc` carry both. Fields are
/// time-weighted averages over samples with the matching `cut_kinematics`
/// tag that were also `is_cutting`. `Rapid` is excluded (the canonical
/// rapid metrics live on the top-level summary).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KinematicsSummary {
    /// Total cutting time spent in this kinematics class (seconds).
    pub cutting_runtime_s: f64,
    /// Time-weighted mean of `engagement.radial_woc_fraction`. `0.0` when
    /// `cutting_runtime_s == 0`.
    pub average_radial_woc_fraction: f64,
    /// Maximum `engagement.radial_woc_fraction` observed.
    pub peak_radial_woc_fraction: f64,
    /// Time-weighted mean of `engagement.axial_doc_fraction` over the
    /// samples that carried one. `None` = **not measured**: no sample in this
    /// class reported an axial-DOC fraction at all (C2 — same contract as
    /// [`Self::average_arc_radians`] beside it). `Some(0.0)` means measured
    /// and zero.
    #[serde(default)]
    pub average_axial_doc_fraction: Option<f64>,
    /// Maximum `engagement.axial_doc_fraction` observed; `None` when none was
    /// measured (see [`Self::average_axial_doc_fraction`]).
    #[serde(default)]
    pub peak_axial_doc_fraction: Option<f64>,
    /// Maximum lateral/arc/helix axial engagement observed (millimetres).
    pub peak_axial_doc_mm: f64,
    /// Maximum pure-vertical plunge descent observed (millimetres).
    #[serde(default)]
    pub peak_plunge_descent_mm: f64,
    /// Time-weighted mean of `engagement.arc_radians` across samples that
    /// carried it. `None` when no sample in this class reported an arc
    /// (e.g. pure plunges).
    pub average_arc_radians: Option<f64>,
    /// Time-weighted mean of `engagement.mean_chip_thickness_mm` across
    /// samples that carried it. `None` when no sample reported chip
    /// thickness.
    pub average_mean_chip_thickness_mm: Option<f64>,
    /// Maximum `engagement.peak_chip_thickness_mm` observed. `None` =
    /// **not measured**: no sample in this class carried a peak chip
    /// thickness. `Some(0.0)` is a measured zero (STK-08 — this field used to
    /// publish `None` for a measured zero, against the contract the sibling
    /// [`Self::peak_axial_doc_fraction`] already kept).
    pub peak_chip_thickness_mm: Option<f64>,
    /// Number of cutting samples that landed in this kinematics class.
    pub sample_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationToolpathCutSummary {
    pub toolpath_id: ToolpathId,
    pub sample_count: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    /// Time spent cutting with `engagement.radial_woc_fraction < 0.02`.
    /// Triggered on the radial-WOC axis only. For toolpaths whose
    /// kinematics make radial-WOC meaningless (drill / pin-drill — see
    /// `metrics_not_applicable`), this field still accumulates but
    /// downstream consumers MUST suppress it via the flag; it does not
    /// indicate an actual problem. Per-kinematics breakdown is on
    /// `per_kinematics` for callers that need an axis-aware reading.
    ///
    /// To express this as a percentage use [`AirCutRatios`] and name the
    /// denominator — `total_runtime_s` and `cutting_runtime_s` give two
    /// different numbers and both have shipped under the name "air cut %".
    pub air_cut_time_s: f64,
    /// Time spent cutting with `0.02 ≤ engagement.radial_woc_fraction < 0.10`.
    /// Same axis + caveats as `air_cut_time_s`.
    pub low_engagement_time_s: f64,
    /// Time-weighted mean of `engagement.radial_woc_fraction` across
    /// cutting samples. For axis-aware reporting (axial-DOC, arc, chip
    /// thickness, leading-edge speed), read `per_kinematics`.
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    #[serde(default)]
    pub peak_plunge_descent_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
    /// True when the dexel's radial-engagement and air-cut metrics
    /// cannot be measured for this toolpath's kinematics (currently
    /// drill / alignment-pin-drill cycles — Z-only moves the XY-cylinder
    /// engagement model can't see). Downstream consumers MUST suppress
    /// `air_cut_time_s`, `average_engagement`, and related per-TP UI
    /// elements when this is set. The time totals stay populated for
    /// MRR / runtime accounting.
    ///
    /// Drill ops still produce drill-native metrics — look up the
    /// matching [`crate::ops::drill_metrics::DrillToolpathSummary`] in
    /// [`SimulationCutTrace::drill_summaries`] by `toolpath_id` (or via
    /// [`SimulationCutTrace::drill_summary_for`]) for peck adequacy,
    /// chip-welding risk, and cycle time. The flag means "no engagement
    /// metrics" — not "no metrics at all" (§6.E / Step 3 PR2).
    /// P4 — see `planning/P4_DRILL_METRIC_SUPPRESSION_RCA.md`.
    #[serde(default)]
    pub metrics_not_applicable: bool,
    /// Per-`CutKinematics` summary block. Lets readers ask the engagement
    /// question that applies to the op kind producing this toolpath — e.g.
    /// drill cycles carry meaningful `Plunge` axial-DOC stats without
    /// muddying the scalar `average_engagement`. Step 2 of the
    /// dexel-fidelity roadmap. Missing keys mean "no samples landed in
    /// that kinematics class for this toolpath."
    #[serde(default)]
    pub per_kinematics: BTreeMap<CutKinematics, KinematicsSummary>,
    /// F-034 integrator runtime decomposed by `MoveIntent` class
    /// (P0 unified-finishing probe). `Some` only when the simulation
    /// ran with a kinematics context — naive traces leave it `None`.
    /// On the project-wide summary this is the field-wise sum across
    /// toolpaths the integrator walked.
    #[serde(default)]
    pub runtime_by_intent: Option<crate::machine::kinematics::CycleTimeBreakdown>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationSemanticCutSummary {
    pub toolpath_id: ToolpathId,
    pub semantic_item_id: u64,
    pub label: String,
    pub kind: ToolpathSemanticKind,
    pub move_start: usize,
    pub move_end: usize,
    pub sample_count: usize,
    pub representative_sample_index: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub wasted_runtime_s: f64,
    pub average_engagement: f64,
    pub peak_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    #[serde(default)]
    pub peak_plunge_descent_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
    pub peak_mrr_mm3_s: f64,
    pub air_cut_issue_count: usize,
    pub low_engagement_issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutHotspot {
    pub toolpath_id: ToolpathId,
    pub semantic_item_id: Option<u64>,
    pub move_start: usize,
    pub move_end: usize,
    pub sample_index_start: usize,
    pub sample_index_end: usize,
    pub representative_position: [f64; 3],
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub wasted_runtime_s: f64,
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    #[serde(default)]
    pub peak_plunge_descent_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
    /// Span path inherited from the first sample contributing to this
    /// hotspot. Empty when the toolpath had no spans.
    #[serde(default)]
    pub span_path: Vec<SpanId>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutSummary {
    pub sample_count: usize,
    pub toolpath_count: usize,
    pub issue_count: usize,
    pub hotspot_count: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    /// Sum of `air_cut_time_s` across toolpaths. Triggered on the
    /// radial-WOC axis (`engagement.radial_woc_fraction < 0.02`); see
    /// per-toolpath summary's `metrics_not_applicable` flag for ops where
    /// radial-WOC is the wrong axis (drilling). For axis-aware reporting
    /// use `per_kinematics`.
    pub air_cut_time_s: f64,
    /// Sum of `low_engagement_time_s` across toolpaths. Same radial-WOC
    /// axis caveats as `air_cut_time_s`.
    pub low_engagement_time_s: f64,
    /// Aggregate time-weighted mean of `engagement.radial_woc_fraction`
    /// across all cutting samples. Use `per_kinematics` for axis-aware
    /// reporting.
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    #[serde(default)]
    pub peak_plunge_descent_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
    /// Per-`CutKinematics` summary block across all toolpaths. See
    /// [`KinematicsSummary`] for axis semantics. Step 2 of the
    /// dexel-fidelity roadmap.
    #[serde(default)]
    pub per_kinematics: BTreeMap<CutKinematics, KinematicsSummary>,
    /// F-034 integrator runtime decomposed by `MoveIntent` class
    /// (P0 unified-finishing probe). `Some` only when the simulation
    /// ran with a kinematics context — naive traces leave it `None`.
    /// On the project-wide summary this is the field-wise sum across
    /// toolpaths the integrator walked.
    #[serde(default)]
    pub runtime_by_intent: Option<crate::machine::kinematics::CycleTimeBreakdown>,
}

// ── LH-1: air cut has TWO denominators; both must be named ──────────────

/// Air-cut time expressed as a percentage — under the **name of its
/// denominator**, because there are two and they are not the same number.
///
/// `air_cut_time_s` is a duration. Turning it into a "%" requires choosing
/// what it is a percentage *of*, and this codebase historically chose both:
///
/// | surface | denominator |
/// |---|---|
/// | `ProjectDiagnostics::air_cut_pct_of_total_runtime`, the GUI banner + "% of total runtime" chips, the `>40%` verdict rule, `OperationType::air_cut_high_threshold_pct` | **total runtime** (cutting + rapids) |
/// | the MCP `narrate_toolpath` air-cut line, and `CLAUDE.md`'s metric caveats | **cutting runtime** (rapids excluded) |
///
/// On a retract-heavy op the two differ by a large factor: total runtime is
/// always ≥ cutting runtime, so the total-runtime reading is always the
/// smaller (and never fires a threshold the cutting-time reading would).
/// Neither is wrong; publishing either as a bare "air cut %" is
/// (`MEASUREMENT_DOMAINS.md` LH-1 / X-3).
///
/// **That order holds only while all three fields share one time base, and
/// between 2026-08 and 2026-09-08 they did not (G-AIRDENOM).**
/// `air_cut_time_s` and `cutting_runtime_s` were naive dexel seconds at the
/// pre-modulation COMMANDED feed while `total_runtime_s` was overwritten
/// with the kinematics-integrated wall clock at the MODULATED feed
/// (`compute/simulate.rs` F-034, `session/compute.rs` F-036b). Where
/// modulation raised the feed the order inverted — 1.76× on a wanaka rough.
/// [`rebase_cutting_times`] now moves the cutting slices onto the
/// integrator's clock at the F-036b site, so the three agree again and
/// `cutting_runtime_s + rapid_runtime_s == total_runtime_s`. With
/// modulation OFF nothing is rebased and the fields keep their naive
/// values; the order still holds there because F-034's accel model can only
/// LENGTHEN the total.
///
/// **Thresholds follow the total-runtime measure.** Every shipped threshold
/// — the GUI's 40%, the CLI's 40%, and every per-operation value in
/// [`crate::compute::catalog::OperationType::air_cut_high_threshold_pct`] —
/// was tuned against [`Self::air_cut_pct_of_total_runtime`] and keeps using
/// it. This trait changed no number; it named them.
///
/// Both readings share the same caveat as the numerator: `air_cut_time_s` is
/// triggered on the radial-WOC axis only, so consumers MUST suppress it when
/// [`SimulationToolpathCutSummary::metrics_not_applicable`] is set.
pub trait AirCutRatios {
    /// Time at `radial_woc_fraction < 0.02` (seconds).
    fn air_cut_seconds(&self) -> f64;
    /// Total integrator runtime including rapids (seconds).
    fn total_runtime_seconds(&self) -> f64;
    /// Cutting-feed runtime, rapids excluded (seconds).
    fn cutting_runtime_seconds(&self) -> f64;

    /// Air-cut time as a percentage of **total runtime (cutting + rapids)** —
    /// the measure every shipped threshold is tuned against. `0.0` when the
    /// toolpath has no runtime.
    #[must_use]
    fn air_cut_pct_of_total_runtime(&self) -> f64 {
        let total = self.total_runtime_seconds();
        if total > 0.0 {
            self.air_cut_seconds() / total * 100.0
        } else {
            0.0
        }
    }

    /// Air-cut time as a percentage of **cutting time (rapids excluded)** —
    /// ≥ [`Self::air_cut_pct_of_total_runtime`], and the measure the MCP
    /// narration reports. `0.0` when the toolpath has no cutting time.
    ///
    /// The `≥` relation requires all three fields on one time base. It was
    /// FALSE under feed modulation between 2026-08 and 2026-09-08
    /// (G-AIRDENOM — see the trait doc). [`rebase_cutting_times`] restores
    /// it. Two populations still fall outside the guarantee, because
    /// nothing rebases them: [`SimulationSemanticCutSummary`] rows and any
    /// summary a caller builds by hand.
    #[must_use]
    fn air_cut_pct_of_cutting_time(&self) -> f64 {
        let cutting = self.cutting_runtime_seconds();
        if cutting > 0.0 {
            self.air_cut_seconds() / cutting * 100.0
        } else {
            0.0
        }
    }
}

macro_rules! impl_air_cut_ratios {
    ($($ty:ty),+ $(,)?) => {
        $(impl AirCutRatios for $ty {
            fn air_cut_seconds(&self) -> f64 {
                self.air_cut_time_s
            }
            fn total_runtime_seconds(&self) -> f64 {
                self.total_runtime_s
            }
            fn cutting_runtime_seconds(&self) -> f64 {
                self.cutting_runtime_s
            }
        })+
    };
}

impl_air_cut_ratios!(
    SimulationCutSummary,
    SimulationToolpathCutSummary,
    SimulationSemanticCutSummary,
    SimulationCutHotspot,
    SummaryAccumulator,
);

// ── G-AIRDENOM: one time base for the air / cutting / total figures ─────

/// The cutting-time slices of one toolpath summary, re-expressed on the
/// kinematics integrator's wall clock. See [`rebase_cutting_times`].
///
/// Stays `pub`: `rebase_cutting_times` returns it, so a crate-private form
/// raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RebasedCuttingTimes {
    /// `total_s − rapid_s − retract_s` from the integrator, distributed
    /// over the dexel's cutting samples.
    pub cutting_runtime_s: f64,
    /// The share of [`Self::cutting_runtime_s`] at
    /// `radial_woc_fraction < 0.02`.
    pub air_cut_time_s: f64,
    /// The share at `0.02 ≤ radial_woc_fraction < 0.10`.
    pub low_engagement_time_s: f64,
    /// `rapid_s + retract_s` — the integrator's answer for the moves the
    /// dexel marks `is_cutting = false`.
    pub rapid_runtime_s: f64,
}

/// One toolpath's kinematics-integrated runtime — see
/// [`SimulationCutTrace::toolpath_runtimes`] for why this is a separate list
/// from `toolpath_summaries` rather than a field on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolpathKinematicRuntime {
    pub toolpath_id: crate::ids::ToolpathId,
    pub breakdown: crate::machine::kinematics::CycleTimeBreakdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutTrace {
    pub schema_version: u32,
    pub sample_step_mm: f64,
    pub summary: SimulationCutSummary,
    pub toolpath_summaries: Vec<SimulationToolpathCutSummary>,
    pub semantic_summaries: Vec<SimulationSemanticCutSummary>,
    pub hotspots: Vec<SimulationCutHotspot>,
    pub issues: Vec<SimulationCutIssue>,
    pub samples: Vec<SimulationCutSample>,
    #[serde(default)]
    pub provenance: Option<SimulationProvenance>,
    /// Per-peck drill samples (§6.E / Step 3 PR2). Empty for traces that
    /// only carried milling toolpaths; populated by
    /// [`crate::ops::drill_metrics::emit_drill_samples`] from each drill
    /// toolpath's `DrillOp`. Joinable to [`Self::drill_summaries`] by
    /// `toolpath_id`.
    #[serde(default)]
    pub drill_samples: Vec<DrillSample>,
    /// Per-toolpath drill summary (§6.E / Step 3 PR2). One entry per drill
    /// toolpath, joined to [`Self::toolpath_summaries`] by `toolpath_id`.
    /// Carries the inputs to the drill gates (chip welding,
    /// peck adequacy) and a cycle-time / chip-evacuation rollup that
    /// covers what `SimulationToolpathCutSummary` cannot for ops with no
    /// engagement-axis samples.
    #[serde(default)]
    pub drill_summaries: Vec<DrillToolpathSummary>,
    /// **G-DRILLTIME (2026-08-22)** — the kinematics integrator's answer for
    /// **every** toolpath it walked, whether or not that toolpath produced
    /// engagement metrics.
    ///
    /// # Why this is not just a column on `toolpath_summaries`
    ///
    /// `toolpath_summaries` is the ENGAGEMENT summary list. A drill toolpath
    /// sets `metrics_not_applicable` and publishes `drill_summaries` instead,
    /// so it has no row there — and `apply_kinematics_cycle_time` used to fold
    /// the project total over that list, which meant a drill's runtime was
    /// computed and then thrown away. Downstream,
    /// `readiness::toolpath_cycle_time` found no summary, correctly fell back
    /// to `cutting_distance / feed` with basis `CuttingOnly`, and correctly
    /// degraded the whole project to the weakest label. Every layer behaved as
    /// designed; the composite answer was that **0.8 % of runtime being
    /// unmodelled dragged a 99.2 %-modelled estimate to "cutting only, no
    /// accel"** — measured live at `2:02:34` where wall clock was expected.
    ///
    /// The root cause is that one slot carried two different facts: "has no
    /// engagement metrics" and "was not integrated". This field is the second
    /// fact, given its own slot.
    ///
    /// # What it does and does not cover
    ///
    /// It is an integral of **stored motion** — the same `Toolpath` the
    /// exporter emits — so a peck cycle's real R-plane and re-entry moves are
    /// in it. It does **not** include G82 dwell, which is not motion; that is
    /// reported separately as `DrillToolpathSummary::dwell_time_s`. A dwelling
    /// cycle's true wall clock is this plus that, and no surface currently
    /// adds them.
    #[serde(default)]
    pub toolpath_runtimes: Vec<ToolpathKinematicRuntime>,
    /// F-035 — per-`(toolpath_id, move_index)` predicted achieved
    /// feed (mm/min) under the active machine kinematics.
    ///
    /// Populated only when `SimulationOptions::use_predicted_feed_in_gates`
    /// is on **and** the active `MachineProfile` carries
    /// `kinematics`. When empty (the default), the chipload + power
    /// gates fall back to each sample's commanded `feed_rate_mm_min` —
    /// byte-identical to pre-F-035 behaviour.
    ///
    /// Tuple-keyed `BTreeMap` does not serialise cleanly through
    /// `serde_json` (JSON requires string keys), so the field is
    /// `#[serde(skip)]`: a trace round-tripped through the artifact
    /// loses its predicted-feed map and the gates fall back to
    /// commanded feed. This is acceptable for v1 because the map is
    /// re-derivable from the toolpath IR + kinematics at any time.
    #[serde(skip)]
    pub predicted_feeds: crate::machine::kinematics::PredictedFeedMap,
    /// F-039 — per-`(toolpath_id, move_index)` emitted feed
    /// (mm/min) plus the binding constraint that drove it.
    /// Populated by `apply_adaptive_feed_modulation` when the
    /// constrained-max or band-mid strategy ran on this toolpath.
    /// `#[serde(skip)]` for the same reason as
    /// [`Self::predicted_feeds`] — re-derivable from the toolpath
    /// IR + modulation context, and JSON can't serialise tuple keys.
    #[serde(skip)]
    pub modulated_feeds:
        std::collections::BTreeMap<(ToolpathId, usize), (f64, crate::tool_load::BindingConstraint)>,
    /// F-039 — per-toolpath modulation rollup, keyed by
    /// `toolpath_id`. The [`crate::gcode::project_load_report`]
    /// builder reads this map and writes the corresponding
    /// `ModulationSummary` onto each `ToolpathLoadVerdict`.
    /// `#[serde(skip)]` — derived from the per-move map above.
    #[serde(skip)]
    pub modulation_summaries:
        std::collections::BTreeMap<ToolpathId, crate::tool_load::ModulationSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutArtifact {
    pub schema_version: u32,
    pub resolution_mm: f64,
    pub sample_step_mm: f64,
    pub stock_bbox_min: [f64; 3],
    pub stock_bbox_max: [f64; 3],
    pub included_toolpath_ids: Vec<ToolpathId>,
    pub request_snapshot: Value,
    pub trace: SimulationCutTrace,
}

impl SimulationCutArtifact {
    pub fn new(
        resolution_mm: f64,
        sample_step_mm: f64,
        stock_bbox_min: [f64; 3],
        stock_bbox_max: [f64; 3],
        included_toolpath_ids: Vec<ToolpathId>,
        request_snapshot: Value,
        trace: SimulationCutTrace,
    ) -> Self {
        Self {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            resolution_mm,
            sample_step_mm,
            stock_bbox_min,
            stock_bbox_max,
            included_toolpath_ids,
            request_snapshot,
            trace,
        }
    }
}

/// Time-weighted aggregator over `SimulationCutSample`s. Canonical source of
/// engagement / DOC / chipload / removed-volume summary math; the MCP per-span
/// and per-depth-pass summaries route through the same `observe` so the
/// statistics, gating, and edge cases (P3 transit-span peak gating, air-cut
/// thresholds) stay in lock-step with the top-level toolpath summary.
#[derive(Default)]
pub struct SummaryAccumulator {
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub engagement_time_weighted_sum: f64,
    pub peak_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    pub peak_plunge_descent_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub peak_mrr_mm3_s: f64,
    pub air_cut_issue_count: usize,
    pub low_engagement_issue_count: usize,
    pub sample_count: usize,
    /// Per-`CutKinematics` sub-accumulators. Step 2 D substrate — let the
    /// final summary expose engagement axes that apply to each kinematics
    /// class. Only cutting samples land here (`is_cutting == true`); `Rapid`
    /// samples are filtered upstream since they don't model cutting.
    ///
    /// Indexed by `CutKinematics::index()`. Fixed-size array (not BTreeMap)
    /// because aggregating 250 k samples is hot enough that map allocs +
    /// rebalances showed up as a 50% regression in the
    /// `simulation_cut_trace_aggregation/from_samples` bench.
    pub per_kinematics: [KinematicsAccumulator; CutKinematics::COUNT],
}

/// Per-`CutKinematics` sub-accumulator. Time-weighted means + extrema across
/// the engagement vector axes. Lives inside `SummaryAccumulator::per_kinematics`.
#[derive(Default, Clone)]
pub struct KinematicsAccumulator {
    pub cutting_runtime_s: f64,
    pub radial_woc_time_weighted_sum: f64,
    pub peak_radial_woc_fraction: f64,
    pub axial_doc_fraction_time_weighted_sum: f64,
    /// Runtime of the samples that actually carried an axial-DOC fraction —
    /// the denominator for the mean, and what makes "measured zero"
    /// distinguishable from "never measured" (C2).
    pub axial_doc_observed_runtime_s: f64,
    pub peak_axial_doc_fraction: Option<f64>,
    pub peak_axial_doc_mm: f64,
    pub peak_plunge_descent_mm: f64,
    /// Sum of `arc_radians * segment_time_s` for samples carrying arc; paired
    /// with `arc_observed_runtime_s` to compute a defined mean only when at
    /// least one sample reported an arc.
    pub arc_time_weighted_sum: f64,
    pub arc_observed_runtime_s: f64,
    pub mean_chip_thickness_time_weighted_sum: f64,
    pub mean_chip_thickness_observed_runtime_s: f64,
    /// `None` until a sample carries a peak chip thickness — the same
    /// `Option`-preserving accumulation `peak_axial_doc_fraction` uses, so a
    /// measured zero stays `Some(0.0)` (STK-08).
    pub peak_chip_thickness_mm: Option<f64>,
    pub sample_count: usize,
}

#[cfg(test)]
mod tests;
