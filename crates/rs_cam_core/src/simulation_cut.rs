use crate::debug_trace::TOOLPATH_DEBUG_SCHEMA_VERSION;
use crate::drill_metrics::{DrillSample, DrillToolpathSummary};
use crate::ids::ToolpathId;
use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticTrace};
use crate::toolpath_spans::SpanId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationMetricOptions {
    pub enabled: bool,
    #[serde(default)]
    pub capture_arc_engagement: bool,
}

// v5 (2026-06-10, F1): `DrillToolpathSummary` gains `chip_welding_dtd`
// (evacuation-credited ratio the chip-welding risk is classified from).
pub const SIMULATION_CUT_TRACE_SCHEMA_VERSION: u32 = 5;

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

/// Cutter-side engagement orientation relative to the feed direction.
/// `Mixed` is the safe fallback when the sample emitter cannot determine
/// orientation (e.g. plunges, helix entries with rapidly changing tangent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementDirection {
    Climb,
    Conventional,
    #[default]
    Mixed,
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
    /// Feed velocity at the engaged cutting edge (mm/min). For 3-axis
    /// lateral moves this is `feed_rate_mm_min`. Used by the chipload gate.
    pub leading_edge_speed_mm_min: f64,
    /// Climb / conventional / mixed. `Mixed` when ambiguous.
    pub direction: EngagementDirection,
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
    /// Indices into [`crate::toolpath_spans::AnnotatedToolpath::spans`] for
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

/// Start a new issue segment from a sample known to be `is_cutting` and
/// below the engagement threshold for the given `kind`.
fn new_open_segment(
    sample: &SimulationCutSample,
    kind: SimulationCutIssueKind,
) -> SimulationCutIssue {
    let label = match kind {
        SimulationCutIssueKind::AirCut => "Air cut".to_owned(),
        SimulationCutIssueKind::LowEngagement => "Low engagement".to_owned(),
    };
    SimulationCutIssue {
        kind,
        toolpath_id: sample.toolpath_id,
        move_index: sample.move_index,
        sample_index: sample.sample_index,
        cumulative_time_s: sample.cumulative_time_s,
        position: sample.position,
        radial_engagement: sample.engagement.radial_woc_fraction,
        semantic_item_id: sample.semantic_item_id,
        label,
        end_move_index: sample.move_index,
        end_sample_index: sample.sample_index,
        end_cumulative_time_s: sample.cumulative_time_s,
        end_position: sample.position,
        sample_count: 1,
        duration_s: 0.0,
        min_radial_engagement: sample.engagement.radial_woc_fraction,
        span_path: sample.span_path.clone(),
    }
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
    /// Maximum `engagement.peak_chip_thickness_mm` observed.
    pub peak_chip_thickness_mm: Option<f64>,
    /// Time-weighted mean of `engagement.leading_edge_speed_mm_min`.
    pub average_leading_edge_speed_mm_min: f64,
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
    /// matching [`crate::drill_metrics::DrillToolpathSummary`] in
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
    pub runtime_by_intent: Option<crate::machine_kinematics::CycleTimeBreakdown>,
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
    pub runtime_by_intent: Option<crate::machine_kinematics::CycleTimeBreakdown>,
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
/// | `ProjectDiagnostics::air_cut_percentage`, the GUI banner + "% of total runtime" chips, the `>40%` verdict rule, `OperationType::air_cut_high_threshold_pct` | **total runtime** (cutting + rapids) |
/// | the MCP `narrate_toolpath` air-cut line, and `CLAUDE.md`'s metric caveats | **cutting runtime** (rapids excluded) |
///
/// On a retract-heavy op the two differ by a large factor: total runtime is
/// always ≥ cutting runtime, so the total-runtime reading is always the
/// smaller (and never fires a threshold the cutting-time reading would).
/// Neither is wrong; publishing either as a bare "air cut %" is
/// (`MEASUREMENT_DOMAINS.md` LH-1 / X-3).
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
    /// always ≥ [`Self::air_cut_pct_of_total_runtime`], and the measure the
    /// MCP narration reports. `0.0` when the toolpath has no cutting time.
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

/// One toolpath's kinematics-integrated runtime — see
/// [`SimulationCutTrace::toolpath_runtimes`] for why this is a separate list
/// from `toolpath_summaries` rather than a field on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolpathKinematicRuntime {
    pub toolpath_id: crate::ids::ToolpathId,
    pub breakdown: crate::machine_kinematics::CycleTimeBreakdown,
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
    /// [`crate::drill_metrics::emit_drill_samples`] from each drill
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
    pub predicted_feeds: crate::machine_kinematics::PredictedFeedMap,
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

impl SimulationCutTrace {
    /// Neutral test fixture: an empty trace at the current
    /// [`SIMULATION_CUT_TRACE_SCHEMA_VERSION`] with no samples, summaries,
    /// issues, or drill data. Test code overrides the fields under test via
    /// struct-update syntax. Not for production paths.
    pub fn test_fixture() -> Self {
        Self {
            schema_version: SIMULATION_CUT_TRACE_SCHEMA_VERSION,
            sample_step_mm: 0.0,
            summary: SimulationCutSummary::default(),
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            samples: Vec::new(),
            provenance: None,
            drill_samples: Vec::new(),
            drill_summaries: Vec::new(),
            toolpath_runtimes: Vec::new(),
            predicted_feeds: crate::machine_kinematics::PredictedFeedMap::new(),
            modulated_feeds: std::collections::BTreeMap::new(),
            modulation_summaries: std::collections::BTreeMap::new(),
        }
    }

    pub fn from_samples(sample_step_mm: f64, samples: Vec<SimulationCutSample>) -> Self {
        Self::from_samples_with_semantics(
            sample_step_mm,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        )
    }

    pub fn from_samples_with_semantics<'a, I>(
        sample_step_mm: f64,
        samples: Vec<SimulationCutSample>,
        semantic_traces: I,
    ) -> Self
    where
        I: IntoIterator<Item = (ToolpathId, &'a ToolpathSemanticTrace)>,
    {
        Self::from_samples_with_context(
            sample_step_mm,
            samples,
            semantic_traces,
            &std::collections::BTreeSet::new(),
        )
    }

    /// Trace builder with op-kind context.
    ///
    /// `metrics_not_applicable_toolpath_ids` is the set of toolpath ids whose
    /// kinematics don't fit the dexel's XY-cylinder side-engagement model
    /// (drill / alignment-pin-drill — Z-only moves). For samples in those
    /// toolpaths the builder suppresses air-cut / low-engagement
    /// `SimulationCutIssue` emission (otherwise every sample emits one and
    /// inflates `issue_count` to the thousands) and sets the per-TP
    /// `metrics_not_applicable` flag on the summary.
    /// P4 — see `planning/P4_DRILL_METRIC_SUPPRESSION_RCA.md`.
    pub fn from_samples_with_context<'a, I>(
        sample_step_mm: f64,
        samples: Vec<SimulationCutSample>,
        semantic_traces: I,
        metrics_not_applicable_toolpath_ids: &std::collections::BTreeSet<ToolpathId>,
    ) -> Self
    where
        I: IntoIterator<Item = (ToolpathId, &'a ToolpathSemanticTrace)>,
    {
        let semantic_traces: BTreeMap<ToolpathId, &ToolpathSemanticTrace> =
            semantic_traces.into_iter().collect();
        let mut toolpaths: BTreeMap<ToolpathId, SummaryAccumulator> = BTreeMap::new();
        let mut hotspot_accs: BTreeMap<(ToolpathId, Option<u64>), HotspotAccumulator> =
            BTreeMap::new();
        let mut semantic_accs: BTreeMap<(ToolpathId, u64), SemanticSummaryAccumulator> =
            BTreeMap::new();
        let mut overall = SummaryAccumulator::default();
        let mut issues = Vec::new();
        // Coalesce contiguous air-cut / low-engagement samples into a single
        // issue per segment instead of one issue per sample. Before this
        // coalescing, a 30K-move adaptive3d run emitted ~141K "air_cut"
        // issues — most of a run's sample stream — drowning out any
        // actionable signal. Tracked per-toolpath so interleaving of
        // samples across toolpaths doesn't bleed one toolpath's segment
        // into another's.
        //
        // See planning/adaptive_review_2026-04.md F-15.
        let mut open_segments: BTreeMap<ToolpathId, SimulationCutIssue> = BTreeMap::new();

        for sample in &samples {
            overall.observe(sample);
            toolpaths
                .entry(sample.toolpath_id)
                .or_default()
                .observe(sample);
            hotspot_accs
                .entry((sample.toolpath_id, sample.semantic_item_id))
                .or_insert_with(|| HotspotAccumulator::new(sample))
                .observe(sample);
            if let Some(item_id) = sample.semantic_item_id {
                semantic_accs
                    .entry((sample.toolpath_id, item_id))
                    .or_insert_with(|| SemanticSummaryAccumulator::new(sample))
                    .observe(sample);
            }

            // P4: for ops whose kinematics fall outside the dexel's
            // XY-cylinder engagement model (drill / pin-drill), every
            // cutting sample reads ~0 radial engagement. Skip issue
            // emission for these to avoid drowning the issue list in
            // thousands of false "air_cut" entries.
            let kind = if !sample.is_cutting
                || metrics_not_applicable_toolpath_ids.contains(&sample.toolpath_id)
            {
                None
            } else if sample.engagement.radial_woc_fraction < 0.02 {
                Some(SimulationCutIssueKind::AirCut)
            } else if sample.engagement.radial_woc_fraction < 0.10 {
                Some(SimulationCutIssueKind::LowEngagement)
            } else {
                None
            };

            match (open_segments.get_mut(&sample.toolpath_id), kind) {
                (Some(open), Some(k)) if open.kind == k => {
                    // Same kind — extend the current segment.
                    open.end_move_index = sample.move_index;
                    open.end_sample_index = sample.sample_index;
                    open.end_cumulative_time_s = sample.cumulative_time_s;
                    open.end_position = sample.position;
                    open.duration_s = sample.cumulative_time_s - open.cumulative_time_s;
                    if sample.engagement.radial_woc_fraction < open.min_radial_engagement {
                        open.min_radial_engagement = sample.engagement.radial_woc_fraction;
                    }
                    open.sample_count += 1;
                }
                (Some(_), Some(k)) => {
                    // Kind changed — flush the old segment and open a new one.
                    if let Some(old) = open_segments.remove(&sample.toolpath_id) {
                        issues.push(old);
                    }
                    open_segments.insert(sample.toolpath_id, new_open_segment(sample, k));
                }
                (Some(_), None) => {
                    // Issue ended (either rapid or good engagement).
                    if let Some(old) = open_segments.remove(&sample.toolpath_id) {
                        issues.push(old);
                    }
                }
                (None, Some(k)) => {
                    open_segments.insert(sample.toolpath_id, new_open_segment(sample, k));
                }
                (None, None) => {}
            }
        }

        // Flush any segments still open at the end of the sample stream.
        for (_, open) in open_segments {
            issues.push(open);
        }

        let toolpath_summaries: Vec<_> = toolpaths
            .into_iter()
            .map(|(toolpath_id, acc)| {
                let mut s = acc.finish_toolpath(toolpath_id);
                if metrics_not_applicable_toolpath_ids.contains(&toolpath_id) {
                    s.metrics_not_applicable = true;
                }
                s
            })
            .collect();
        let mut semantic_summaries: Vec<_> = semantic_accs
            .into_iter()
            .filter_map(|((toolpath_id, semantic_item_id), acc)| {
                let trace = semantic_traces.get(&toolpath_id)?;
                let item = trace
                    .items
                    .iter()
                    .find(|item| item.id == semantic_item_id)?;
                Some(acc.finish(toolpath_id, item))
            })
            .collect();
        semantic_summaries.sort_by(|left, right| {
            right
                .wasted_runtime_s
                .total_cmp(&left.wasted_runtime_s)
                .then_with(|| left.average_mrr_mm3_s.total_cmp(&right.average_mrr_mm3_s))
                .then_with(|| right.total_runtime_s.total_cmp(&left.total_runtime_s))
                .then_with(|| left.move_start.cmp(&right.move_start))
        });
        let mut hotspots: Vec<_> = hotspot_accs
            .into_values()
            .map(HotspotAccumulator::finish)
            .collect();
        hotspots.sort_by(|left, right| {
            right
                .wasted_runtime_s
                .total_cmp(&left.wasted_runtime_s)
                .then_with(|| right.total_runtime_s.total_cmp(&left.total_runtime_s))
                .then_with(|| left.move_start.cmp(&right.move_start))
        });

        let summary = overall.finish_summary(
            samples.len(),
            toolpath_summaries.len(),
            issues.len(),
            hotspots.len(),
        );

        Self {
            schema_version: SIMULATION_CUT_TRACE_SCHEMA_VERSION,
            sample_step_mm,
            summary,
            toolpath_summaries,
            semantic_summaries,
            hotspots,
            issues,
            samples,
            provenance: None,
            drill_samples: Vec::new(),
            drill_summaries: Vec::new(),
            toolpath_runtimes: Vec::new(),
            predicted_feeds: crate::machine_kinematics::PredictedFeedMap::new(),
            modulated_feeds: std::collections::BTreeMap::new(),
            modulation_summaries: std::collections::BTreeMap::new(),
        }
    }

    /// Look up the per-toolpath drill summary by id. `None` when this
    /// toolpath isn't a drill op (no entry in [`Self::drill_summaries`]).
    /// Pairs with the per-toolpath
    /// [`SimulationToolpathCutSummary::metrics_not_applicable`] flag —
    /// when that flag is set, this lookup is the right place to read
    /// drill-native metrics in lieu of engagement-axis ones.
    pub fn drill_summary_for(&self, toolpath_id: ToolpathId) -> Option<&DrillToolpathSummary> {
        self.drill_summaries
            .iter()
            .find(|s| s.toolpath_id == toolpath_id)
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
    pub peak_chip_thickness_mm: f64,
    pub leading_edge_speed_time_weighted_sum: f64,
    pub sample_count: usize,
}

/// Fold the array-of-accumulators into a sparse `BTreeMap` of finished
/// summaries, dropping empty (no-sample) entries. The reporting layer wants
/// "only the kinematics classes that were observed"; the accumulation hot
/// path wants O(1) array indexing.
pub fn finalize_per_kinematics(
    accs: [KinematicsAccumulator; CutKinematics::COUNT],
) -> BTreeMap<CutKinematics, KinematicsSummary> {
    let mut out = BTreeMap::new();
    for (kind, acc) in CutKinematics::ALL.into_iter().zip(accs.into_iter()) {
        if acc.sample_count > 0 {
            out.insert(kind, acc.finish());
        }
    }
    out
}

impl KinematicsAccumulator {
    fn observe_cutting(&mut self, sample: &SimulationCutSample) {
        let dt = sample.segment_time_s;
        self.cutting_runtime_s += dt;
        self.sample_count += 1;
        let eng = &sample.engagement;
        self.radial_woc_time_weighted_sum += eng.radial_woc_fraction * dt;
        self.peak_radial_woc_fraction = self.peak_radial_woc_fraction.max(eng.radial_woc_fraction);
        if let Some(axial) = eng.axial_doc_fraction {
            self.axial_doc_fraction_time_weighted_sum += axial * dt;
            self.axial_doc_observed_runtime_s += dt;
            self.peak_axial_doc_fraction =
                Some(self.peak_axial_doc_fraction.map_or(axial, |p| p.max(axial)));
        }
        // P3: transit-span samples produce dexel-bridge artifacts on peak
        // DOC. Defer to the same gating the top-level accumulator uses.
        if !sample.in_transit_span {
            self.peak_axial_doc_mm = self
                .peak_axial_doc_mm
                .max(sample.axial_engagement_mm.max(0.0));
        }
        self.peak_plunge_descent_mm = self
            .peak_plunge_descent_mm
            .max(sample.plunge_descent_mm.max(0.0));
        if let Some(arc) = eng.arc_radians {
            self.arc_time_weighted_sum += arc * dt;
            self.arc_observed_runtime_s += dt;
        }
        if let Some(mct) = eng.mean_chip_thickness_mm {
            self.mean_chip_thickness_time_weighted_sum += mct * dt;
            self.mean_chip_thickness_observed_runtime_s += dt;
        }
        if let Some(pct) = eng.peak_chip_thickness_mm {
            self.peak_chip_thickness_mm = self.peak_chip_thickness_mm.max(pct);
        }
        self.leading_edge_speed_time_weighted_sum += eng.leading_edge_speed_mm_min * dt;
    }

    pub fn finish(self) -> KinematicsSummary {
        let t = self.cutting_runtime_s.max(1e-12);
        KinematicsSummary {
            cutting_runtime_s: self.cutting_runtime_s,
            average_radial_woc_fraction: if self.cutting_runtime_s > 1e-9 {
                self.radial_woc_time_weighted_sum / t
            } else {
                0.0
            },
            peak_radial_woc_fraction: self.peak_radial_woc_fraction,
            average_axial_doc_fraction: if self.axial_doc_observed_runtime_s > 1e-9 {
                Some(self.axial_doc_fraction_time_weighted_sum / self.axial_doc_observed_runtime_s)
            } else {
                None
            },
            peak_axial_doc_fraction: self.peak_axial_doc_fraction,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            average_arc_radians: if self.arc_observed_runtime_s > 1e-9 {
                Some(self.arc_time_weighted_sum / self.arc_observed_runtime_s)
            } else {
                None
            },
            average_mean_chip_thickness_mm: if self.mean_chip_thickness_observed_runtime_s > 1e-9 {
                Some(
                    self.mean_chip_thickness_time_weighted_sum
                        / self.mean_chip_thickness_observed_runtime_s,
                )
            } else {
                None
            },
            peak_chip_thickness_mm: if self.peak_chip_thickness_mm > 0.0 {
                Some(self.peak_chip_thickness_mm)
            } else {
                None
            },
            average_leading_edge_speed_mm_min: if self.cutting_runtime_s > 1e-9 {
                self.leading_edge_speed_time_weighted_sum / t
            } else {
                0.0
            },
            sample_count: self.sample_count,
        }
    }
}

impl SummaryAccumulator {
    pub fn observe(&mut self, sample: &SimulationCutSample) {
        self.sample_count += 1;
        self.total_runtime_s += sample.segment_time_s;
        self.total_removed_volume_est_mm3 += sample.removed_volume_est_mm3.max(0.0);
        self.peak_engagement = self
            .peak_engagement
            .max(sample.engagement.radial_woc_fraction.max(0.0));
        // P3: skip extreme-value updates for transit-span samples. The
        // dexel reports `stock_top − cutter_z` over uncleared neighbouring
        // stock during link bridges, helix entries, lead-outs, and
        // waterline-cleanup spans — not steady-state engagement. Including
        // those samples inflates peak DOC to multiples of the configured
        // depth_per_pass (Wanaka TP3 reads 18.59 mm on a 6 mm tool).
        // See `planning/P3_TRANSIT_PEAK_DOC_RCA.md`.
        if !sample.in_transit_span {
            self.peak_chipload_mm_per_tooth = self
                .peak_chipload_mm_per_tooth
                .max(sample.chipload_mm_per_tooth.max(0.0));
            self.peak_axial_doc_mm = self
                .peak_axial_doc_mm
                .max(sample.axial_engagement_mm.max(0.0));
        }
        self.peak_plunge_descent_mm = self
            .peak_plunge_descent_mm
            .max(sample.plunge_descent_mm.max(0.0));
        self.peak_mrr_mm3_s = self.peak_mrr_mm3_s.max(sample.mrr_mm3_s.max(0.0));

        if sample.is_cutting {
            self.cutting_runtime_s += sample.segment_time_s;
            self.engagement_time_weighted_sum +=
                sample.engagement.radial_woc_fraction * sample.segment_time_s;
            if sample.engagement.radial_woc_fraction < 0.02 {
                self.air_cut_time_s += sample.segment_time_s;
                self.air_cut_issue_count += 1;
            } else if sample.engagement.radial_woc_fraction < 0.10 {
                self.low_engagement_time_s += sample.segment_time_s;
                self.low_engagement_issue_count += 1;
            }
            // Step 2 D substrate: route cutting samples to their kinematics
            // sub-accumulator. `Rapid` is skipped (samples emitted as
            // `is_cutting=true` with `Rapid` kinematics would be a bug in the
            // emitter; we honour the `is_cutting` gate first). `Linear`
            // samples with `MoveIntent::Retract` were already reclassified
            // to `is_cutting=false` in Step 1, so they don't reach this
            // branch.
            #[allow(clippy::indexing_slicing)]
            // SAFETY: `cut_kinematics.index()` is bounded by COUNT.
            self.per_kinematics[sample.cut_kinematics.index()].observe_cutting(sample);
        } else {
            self.rapid_runtime_s += sample.segment_time_s;
        }
    }

    pub fn average_engagement(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.engagement_time_weighted_sum / self.cutting_runtime_s
        }
    }

    pub fn average_mrr(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.total_removed_volume_est_mm3 / self.cutting_runtime_s
        }
    }

    pub fn finish_toolpath(self, toolpath_id: ToolpathId) -> SimulationToolpathCutSummary {
        let average_engagement = self.average_engagement();
        let average_mrr_mm3_s = self.average_mrr();
        let per_kinematics = finalize_per_kinematics(self.per_kinematics);
        SimulationToolpathCutSummary {
            toolpath_id,
            sample_count: self.sample_count,
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            rapid_runtime_s: self.rapid_runtime_s,
            air_cut_time_s: self.air_cut_time_s,
            low_engagement_time_s: self.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            metrics_not_applicable: false,
            per_kinematics,
            runtime_by_intent: None,
        }
    }

    fn finish_summary(
        self,
        sample_count: usize,
        toolpath_count: usize,
        issue_count: usize,
        hotspot_count: usize,
    ) -> SimulationCutSummary {
        let average_engagement = self.average_engagement();
        let average_mrr_mm3_s = self.average_mrr();
        let per_kinematics = finalize_per_kinematics(self.per_kinematics);
        SimulationCutSummary {
            sample_count,
            toolpath_count,
            issue_count,
            hotspot_count,
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            rapid_runtime_s: self.rapid_runtime_s,
            air_cut_time_s: self.air_cut_time_s,
            low_engagement_time_s: self.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            per_kinematics,
            runtime_by_intent: None,
        }
    }
}

/// Scatter one toolpath's samples into per-span accumulators in **one pass**
/// over the sample vector.
///
/// Returns a vector of length `span_count`, indexed by span id. A span with
/// no samples comes back as a `SummaryAccumulator::default()` whose
/// `sample_count` is `0` — callers filter on that, exactly as the per-span
/// loop they are replacing did.
///
/// **A sample belongs to every span in its `span_path`, not to one of them.**
/// Spans nest (Operation ⊃ Region ⊃ DepthPass ⊃ Entry …), and the per-span
/// summaries are read as "everything that happened inside this span", so the
/// scatter fans each sample out across its whole path. That is the one
/// behavioural detail a single-pass rewrite can get wrong, and it is what
/// separates this from `build_per_depth_pass_summary`'s sibling loop, which
/// picks the *first* matching id because depth passes do not nest.
///
/// `accept` optionally restricts which span ids accumulate (the
/// `get_cut_trace` span filter). `None` accumulates every span.
///
/// # Why this exists
///
/// The GUI's `get_cut_trace` used to run this as a nested loop — for every
/// span, a full scan of the project's entire sample vector — so a bare
/// `get_cut_trace()` cost `Σ_toolpaths (spans × total_samples)`, on the egui
/// frame-loop thread, with every other queued MCP request waiting behind it.
/// An accidental quadratic, diagnosed as C1 in
/// `planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.D.2.
/// Cost here is `Σ_samples |span_path|` plus `span_count`, i.e. linear in the
/// trace with a small constant.
pub fn accumulate_by_span(
    samples: &[SimulationCutSample],
    toolpath_id: ToolpathId,
    span_count: usize,
    accept: Option<&std::collections::HashSet<u32>>,
) -> Vec<SummaryAccumulator> {
    let mut accs: Vec<SummaryAccumulator> = (0..span_count)
        .map(|_| SummaryAccumulator::default())
        .collect();
    if span_count == 0 {
        return accs;
    }
    for sample in samples.iter().filter(|s| s.toolpath_id == toolpath_id) {
        for (pos, &SpanId(id)) in sample.span_path.iter().enumerate() {
            // The loop this replaces asked `span_path.contains(id)` once per
            // span, so a path that repeats an id counted the sample ONCE.
            // Preserve that: skip an id already seen earlier in this path.
            if sample.span_path.iter().take(pos).any(|&SpanId(p)| p == id) {
                continue;
            }
            if accept.is_some_and(|set| !set.contains(&id)) {
                continue;
            }
            let Some(acc) = accs.get_mut(id as usize) else {
                // A span id past the end of this toolpath's span table.
                // Dropped rather than panicking: span tables and traces can
                // be regenerated independently, and a stale id is not worth
                // taking the frame loop down for.
                continue;
            };
            acc.observe(sample);
        }
    }
    accs
}

struct HotspotAccumulator {
    toolpath_id: ToolpathId,
    semantic_item_id: Option<u64>,
    move_start: usize,
    move_end: usize,
    sample_index_start: usize,
    sample_index_end: usize,
    representative_position: [f64; 3],
    representative_segment_time_s: f64,
    span_path: Vec<SpanId>,
    summary: SummaryAccumulator,
}

impl HotspotAccumulator {
    fn new(sample: &SimulationCutSample) -> Self {
        Self {
            toolpath_id: sample.toolpath_id,
            semantic_item_id: sample.semantic_item_id,
            move_start: sample.move_index,
            move_end: sample.move_index,
            sample_index_start: sample.sample_index,
            sample_index_end: sample.sample_index,
            representative_position: sample.position,
            representative_segment_time_s: sample.segment_time_s,
            span_path: sample.span_path.clone(),
            summary: SummaryAccumulator::default(),
        }
    }

    fn observe(&mut self, sample: &SimulationCutSample) {
        self.move_start = self.move_start.min(sample.move_index);
        self.move_end = self.move_end.max(sample.move_index);
        self.sample_index_start = self.sample_index_start.min(sample.sample_index);
        self.sample_index_end = self.sample_index_end.max(sample.sample_index);
        if sample.segment_time_s >= self.representative_segment_time_s {
            self.representative_position = sample.position;
            self.representative_segment_time_s = sample.segment_time_s;
        }
        self.summary.observe(sample);
    }

    fn finish(self) -> SimulationCutHotspot {
        let average_engagement = self.summary.average_engagement();
        let average_mrr_mm3_s = self.summary.average_mrr();
        SimulationCutHotspot {
            toolpath_id: self.toolpath_id,
            semantic_item_id: self.semantic_item_id,
            move_start: self.move_start,
            move_end: self.move_end,
            sample_index_start: self.sample_index_start,
            sample_index_end: self.sample_index_end,
            representative_position: self.representative_position,
            total_runtime_s: self.summary.total_runtime_s,
            cutting_runtime_s: self.summary.cutting_runtime_s,
            rapid_runtime_s: self.summary.rapid_runtime_s,
            air_cut_time_s: self.summary.air_cut_time_s,
            low_engagement_time_s: self.summary.low_engagement_time_s,
            wasted_runtime_s: self.summary.air_cut_time_s + self.summary.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.summary.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.summary.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.summary.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            span_path: self.span_path,
        }
    }
}

struct SemanticSummaryAccumulator {
    move_start: usize,
    move_end: usize,
    representative_sample_index: usize,
    representative_segment_time_s: f64,
    summary: SummaryAccumulator,
}

impl SemanticSummaryAccumulator {
    fn new(sample: &SimulationCutSample) -> Self {
        Self {
            move_start: sample.move_index,
            move_end: sample.move_index,
            representative_sample_index: sample.sample_index,
            representative_segment_time_s: sample.segment_time_s,
            summary: SummaryAccumulator::default(),
        }
    }

    fn observe(&mut self, sample: &SimulationCutSample) {
        self.move_start = self.move_start.min(sample.move_index);
        self.move_end = self.move_end.max(sample.move_index);
        if sample.segment_time_s >= self.representative_segment_time_s {
            self.representative_sample_index = sample.sample_index;
            self.representative_segment_time_s = sample.segment_time_s;
        }
        self.summary.observe(sample);
    }

    fn finish(
        self,
        toolpath_id: ToolpathId,
        item: &crate::semantic_trace::ToolpathSemanticItem,
    ) -> SimulationSemanticCutSummary {
        SimulationSemanticCutSummary {
            toolpath_id,
            semantic_item_id: item.id,
            label: item.label.clone(),
            kind: item.kind.clone(),
            move_start: item.move_start.unwrap_or(self.move_start),
            move_end: item.move_end.unwrap_or(self.move_end),
            sample_count: self.summary.sample_count,
            representative_sample_index: self.representative_sample_index,
            total_runtime_s: self.summary.total_runtime_s,
            cutting_runtime_s: self.summary.cutting_runtime_s,
            rapid_runtime_s: self.summary.rapid_runtime_s,
            air_cut_time_s: self.summary.air_cut_time_s,
            low_engagement_time_s: self.summary.low_engagement_time_s,
            wasted_runtime_s: self.summary.air_cut_time_s + self.summary.low_engagement_time_s,
            average_engagement: self.summary.average_engagement(),
            peak_engagement: self.summary.peak_engagement,
            peak_chipload_mm_per_tooth: self.summary.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.summary.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.summary.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s: self.summary.average_mrr(),
            peak_mrr_mm3_s: self.summary.peak_mrr_mm3_s,
            air_cut_issue_count: self.summary.air_cut_issue_count,
            low_engagement_issue_count: self.summary.low_engagement_issue_count,
        }
    }
}

pub fn write_simulation_cut_artifact(
    dir: &Path,
    file_stem: &str,
    artifact: &SimulationCutArtifact,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_name = format!(
        "{}_{}.json",
        timestamp_ms,
        sanitize_filename_component(file_stem)
    );
    let path = dir.join(file_name);
    let payload = serde_json::to_vec_pretty(artifact)?;
    std::fs::write(&path, payload)?;
    Ok(path)
}

fn sanitize_filename_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        "simulation_cut_trace".to_owned()
    } else {
        output.to_owned()
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
    use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder};
    use crate::toolpath::Toolpath;

    /// C2: an unmeasured axial-DOC fraction must not be averaged in as a
    /// zero. The mean is taken over the samples that carried one — the same
    /// contract `average_arc_radians` has had since Step 2 — and a class where
    /// nothing was measured reports `None`, not `0.0`.
    #[test]
    fn unmeasured_axial_doc_fraction_is_none_not_a_zero_in_the_mean() {
        let sample = |axial: Option<f64>, dt: f64, idx: usize| SimulationCutSample {
            toolpath_id: ToolpathId(1),
            move_index: idx,
            sample_index: idx,
            position: [idx as f64, 0.0, -1.0],
            cumulative_time_s: dt * (idx as f64 + 1.0),
            segment_time_s: dt,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            engagement: Engagement {
                radial_woc_fraction: 0.5,
                axial_doc_fraction: axial,
                ..Default::default()
            },
            ..SimulationCutSample::test_fixture()
        };

        // Nothing measured anywhere → None, and the radial axis is unaffected.
        let none_trace =
            SimulationCutTrace::from_samples(0.5, vec![sample(None, 0.4, 0), sample(None, 0.6, 1)]);
        let lin = none_trace
            .summary
            .per_kinematics
            .get(&CutKinematics::Linear)
            .expect("linear samples were observed");
        assert_eq!(lin.average_axial_doc_fraction, None);
        assert_eq!(lin.peak_axial_doc_fraction, None);
        assert!((lin.average_radial_woc_fraction - 0.5).abs() < 1e-9);

        // One measured 0.8 over 0.4 s, one unmeasured over 0.6 s: the mean is
        // 0.8 (over the observed runtime), NOT 0.32 (over all cutting time),
        // which is what the pre-C2 zero-sentinel produced.
        let mixed = SimulationCutTrace::from_samples(
            0.5,
            vec![sample(Some(0.8), 0.4, 0), sample(None, 0.6, 1)],
        );
        let lin = mixed
            .summary
            .per_kinematics
            .get(&CutKinematics::Linear)
            .expect("linear samples were observed");
        assert!(
            lin.average_axial_doc_fraction
                .is_some_and(|a| (a - 0.8).abs() < 1e-9),
            "mean must be over MEASURED runtime, got {:?}",
            lin.average_axial_doc_fraction
        );
        assert!(
            lin.peak_axial_doc_fraction
                .is_some_and(|p| (p - 0.8).abs() < 1e-9)
        );

        // A measured zero is still a measurement.
        let zero = SimulationCutTrace::from_samples(0.5, vec![sample(Some(0.0), 0.4, 0)]);
        let lin = zero
            .summary
            .per_kinematics
            .get(&CutKinematics::Linear)
            .expect("linear samples were observed");
        assert_eq!(lin.average_axial_doc_fraction, Some(0.0));
        assert_eq!(lin.peak_axial_doc_fraction, Some(0.0));
    }

    #[test]
    fn trace_from_samples_accumulates_summary_and_issues() {
        let trace = SimulationCutTrace::from_samples(
            0.5,
            vec![
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 4,
                    position: [1.0, 2.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.5,
                    axial_engagement_mm: 1.5,
                    chipload_mm_per_tooth: 0.0166,
                    effective_chip_thickness_mm: Some(0.0166),
                    engagement: Engagement::with_radial_woc(0.01),
                    removed_volume_est_mm3: 2.0,
                    mrr_mm3_s: 10.0,
                    semantic_item_id: Some(9),
                    ..SimulationCutSample::test_fixture()
                },
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 5,
                    sample_index: 1,
                    position: [2.0, 2.0, -1.0],
                    cumulative_time_s: 0.5,
                    segment_time_s: 0.3,
                    is_cutting: true,
                    cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 2.0,
                    axial_engagement_mm: 2.0,
                    chipload_mm_per_tooth: 0.0166,
                    effective_chip_thickness_mm: Some(0.0166),
                    engagement: Engagement::with_radial_woc(0.08),
                    removed_volume_est_mm3: 3.0,
                    mrr_mm3_s: 10.0,
                    semantic_item_id: Some(9),
                    ..SimulationCutSample::test_fixture()
                },
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 6,
                    sample_index: 2,
                    position: [3.0, 2.0, 5.0],
                    cumulative_time_s: 0.6,
                    segment_time_s: 0.1,
                    cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 5000.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    ..SimulationCutSample::test_fixture()
                },
            ],
        );

        assert_eq!(trace.summary.sample_count, 3);
        assert_eq!(trace.summary.issue_count, 2);
        assert_eq!(trace.toolpath_summaries.len(), 1);
        assert_eq!(trace.hotspots.len(), 2);
        assert!((trace.summary.total_runtime_s - 0.6).abs() < 1e-9);
        assert!((trace.summary.air_cut_time_s - 0.2).abs() < 1e-9);
        assert!((trace.summary.low_engagement_time_s - 0.3).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn trace_from_samples_with_semantics_emits_per_item_summaries() {
        let recorder = ToolpathSemanticRecorder::new("Metrics", "Metrics");
        let root = recorder.root_context();
        let op = root.start_item(ToolpathSemanticKind::Operation, "Metrics");
        let mut tp = Toolpath::new();
        tp.rapid_to(crate::geo::P3::new(0.0, 0.0, 5.0));
        tp.feed_to(crate::geo::P3::new(0.0, 0.0, -1.0), 300.0);
        tp.feed_to(crate::geo::P3::new(10.0, 0.0, -1.0), 300.0);
        tp.rapid_to(crate::geo::P3::new(10.0, 0.0, 5.0));
        let pass = op
            .context()
            .start_item(ToolpathSemanticKind::Pass, "Pass 1");
        pass.bind_to_toolpath(&tp, 0, tp.moves.len());
        let trace = recorder.finish();

        let cut_trace = SimulationCutTrace::from_samples_with_semantics(
            0.5,
            vec![
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 1,
                    position: [0.0, 0.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.0,
                    axial_engagement_mm: 1.0,
                    chipload_mm_per_tooth: 0.0083,
                    effective_chip_thickness_mm: Some(0.0083),
                    engagement: Engagement::with_radial_woc(0.08),
                    removed_volume_est_mm3: 0.2,
                    mrr_mm3_s: 1.0,
                    semantic_item_id: Some(2),
                    ..SimulationCutSample::test_fixture()
                },
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 2,
                    sample_index: 1,
                    position: [5.0, 0.0, -1.0],
                    cumulative_time_s: 0.4,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.2,
                    axial_engagement_mm: 1.2,
                    chipload_mm_per_tooth: 0.0083,
                    effective_chip_thickness_mm: Some(0.0083),
                    engagement: Engagement::with_radial_woc(0.15),
                    removed_volume_est_mm3: 0.4,
                    mrr_mm3_s: 2.0,
                    semantic_item_id: Some(2),
                    ..SimulationCutSample::test_fixture()
                },
            ],
            [(ToolpathId(1), &trace)],
        );

        let summary = cut_trace
            .semantic_summaries
            .iter()
            .find(|summary| summary.semantic_item_id == 2)
            .expect("semantic summary");
        assert_eq!(summary.label, "Pass 1");
        assert_eq!(summary.kind, ToolpathSemanticKind::Pass);
        assert_eq!(summary.sample_count, 2);
        assert!(summary.peak_engagement >= summary.average_engagement);
        assert!(summary.peak_mrr_mm3_s >= summary.average_mrr_mm3_s);
    }

    #[test]
    fn issues_and_hotspots_inherit_span_path_from_first_sample() {
        let span_path = vec![
            crate::toolpath_spans::SpanId(0),
            crate::toolpath_spans::SpanId(1),
        ];
        let trace = SimulationCutTrace::from_samples(
            0.5,
            vec![
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 4,
                    position: [1.0, 2.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: CutKinematics::Linear,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.5,
                    axial_engagement_mm: 1.5,
                    chipload_mm_per_tooth: 0.0166,
                    effective_chip_thickness_mm: Some(0.0166),
                    engagement: Engagement::with_radial_woc(0.005),
                    removed_volume_est_mm3: 0.1,
                    mrr_mm3_s: 0.5,
                    semantic_item_id: Some(7),
                    span_path: span_path.clone(),
                    ..SimulationCutSample::test_fixture()
                },
                SimulationCutSample {
                    toolpath_id: ToolpathId(1),
                    move_index: 4,
                    sample_index: 1,
                    position: [1.5, 2.0, -1.0],
                    cumulative_time_s: 0.4,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: CutKinematics::Linear,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.5,
                    axial_engagement_mm: 1.5,
                    chipload_mm_per_tooth: 0.0166,
                    effective_chip_thickness_mm: Some(0.0166),
                    engagement: Engagement::with_radial_woc(0.005),
                    removed_volume_est_mm3: 0.1,
                    mrr_mm3_s: 0.5,
                    semantic_item_id: Some(7),
                    span_path: span_path.clone(),
                    ..SimulationCutSample::test_fixture()
                },
            ],
        );
        assert_eq!(trace.issues.len(), 1, "one coalesced air-cut issue");
        assert_eq!(trace.issues[0].span_path, span_path);
        assert_eq!(trace.hotspots.len(), 1);
        assert_eq!(trace.hotspots[0].span_path, span_path);
    }

    #[test]
    fn simulation_cut_artifact_writer_creates_json() {
        let artifact = SimulationCutArtifact::new(
            0.25,
            0.25,
            [0.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            vec![ToolpathId(1), ToolpathId(2)],
            serde_json::json!({"resolution": 0.25}),
            SimulationCutTrace::from_samples(0.25, Vec::new()),
        );
        let dir = std::env::temp_dir().join(format!(
            "rs_cam_sim_cut_artifact_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let path = write_simulation_cut_artifact(&dir, "Adaptive 3D", &artifact)
            .expect("write sim cut artifact");
        let text = std::fs::read_to_string(&path).expect("read sim cut artifact");
        assert!(text.contains("\"included_toolpath_ids\""));
        std::fs::remove_file(path).ok();
        std::fs::remove_dir(dir).ok();
    }

    // --- Task E-simcut: Empty samples produce valid defaults ---

    #[test]
    fn trace_from_empty_samples() {
        let trace = SimulationCutTrace::from_samples(1.0, Vec::new());

        assert_eq!(trace.summary.sample_count, 0);
        assert_eq!(trace.summary.toolpath_count, 0);
        assert_eq!(trace.summary.issue_count, 0);
        assert_eq!(trace.summary.hotspot_count, 0);
        assert!((trace.summary.total_runtime_s).abs() < 1e-9);
        assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
        assert!((trace.summary.rapid_runtime_s).abs() < 1e-9);
        assert!((trace.summary.average_engagement).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
        assert!(trace.toolpath_summaries.is_empty());
        assert!(trace.issues.is_empty());
        assert!(trace.hotspots.is_empty());
        assert!(trace.samples.is_empty());
    }

    // --- Rapid-only toolpath (no cutting) ---

    #[test]
    fn trace_rapid_only_has_zero_cutting_time() {
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, 10.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                feed_rate_mm_min: 5000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [50.0, 0.0, 10.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.2,
                feed_rate_mm_min: 5000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(1.0, samples);

        assert_eq!(trace.summary.sample_count, 2);
        assert_eq!(trace.summary.toolpath_count, 1);
        assert!((trace.summary.total_runtime_s - 0.3).abs() < 1e-9);
        assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
        assert!((trace.summary.rapid_runtime_s - 0.3).abs() < 1e-9);
        assert_eq!(trace.summary.issue_count, 0);
        assert!((trace.summary.average_engagement).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
    }

    // --- Engagement metrics classification ---

    #[test]
    fn trace_classifies_air_cut_and_low_engagement() {
        let samples = vec![
            // Air cut: is_cutting=true, engagement < 0.02
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                removed_volume_est_mm3: 0.1,
                mrr_mm3_s: 1.0,
                ..SimulationCutSample::test_fixture()
            },
            // Low engagement: is_cutting=true, 0.02 <= engagement < 0.10
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.05),
                removed_volume_est_mm3: 0.3,
                mrr_mm3_s: 3.0,
                ..SimulationCutSample::test_fixture()
            },
            // Good engagement: is_cutting=true, engagement >= 0.10
            SimulationCutSample {
                move_index: 2,
                sample_index: 2,
                position: [2.0, 0.0, -1.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                axial_engagement_mm: 2.0,
                chipload_mm_per_tooth: 0.02,
                effective_chip_thickness_mm: Some(0.02),
                engagement: Engagement::with_radial_woc(0.50),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Two issues: one air cut and one low engagement
        assert_eq!(trace.summary.issue_count, 2);
        assert_eq!(trace.issues.len(), 2);

        let air_cuts: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.kind == SimulationCutIssueKind::AirCut)
            .collect();
        let low_engs: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.kind == SimulationCutIssueKind::LowEngagement)
            .collect();
        assert_eq!(air_cuts.len(), 1, "Should have exactly 1 air cut issue");
        assert_eq!(
            low_engs.len(),
            1,
            "Should have exactly 1 low engagement issue"
        );

        // Air cut time = 0.1s, low engagement time = 0.1s
        assert!((trace.summary.air_cut_time_s - 0.1).abs() < 1e-9);
        assert!((trace.summary.low_engagement_time_s - 0.1).abs() < 1e-9);
    }

    #[test]
    fn old_trace_json_deserializes_without_new_fields() {
        let json = r#"{
            "schema_version": 1,
            "sample_step_mm": 1.0,
            "summary": {
                "sample_count": 0,
                "toolpath_count": 0,
                "issue_count": 0,
                "hotspot_count": 0,
                "total_runtime_s": 0.0,
                "cutting_runtime_s": 0.0,
                "rapid_runtime_s": 0.0,
                "air_cut_time_s": 0.0,
                "low_engagement_time_s": 0.0,
                "average_engagement": 0.0,
                "peak_chipload_mm_per_tooth": 0.0,
                "peak_axial_doc_mm": 0.0,
                "total_removed_volume_est_mm3": 0.0,
                "average_mrr_mm3_s": 0.0
            },
            "toolpath_summaries": [],
            "semantic_summaries": [],
            "hotspots": [],
            "issues": [],
            "samples": [{
                "toolpath_id": 0,
                "move_index": 0,
                "sample_index": 0,
                "position": [0.0, 0.0, 0.0],
                "cumulative_time_s": 0.0,
                "segment_time_s": 0.0,
                "is_cutting": false,
                "feed_rate_mm_min": 0.0,
                "spindle_rpm": 0,
                "flute_count": 0,
                "axial_doc_mm": 0.0,
                "radial_engagement": 0.0,
                "chipload_mm_per_tooth": 0.0,
                "removed_volume_est_mm3": 0.0,
                "mrr_mm3_s": 0.0,
                "semantic_item_id": null
            }]
        }"#;
        let trace: SimulationCutTrace = serde_json::from_str(json).expect("old trace deserializes");
        assert!(trace.provenance.is_none());
        assert_eq!(trace.samples[0].arc_engagement_radians, None);
        assert_eq!(trace.samples[0].cut_kinematics, CutKinematics::Rapid);
    }

    #[test]
    fn trace_coalesces_contiguous_air_cut_samples_into_one_issue() {
        // Ten consecutive air-cut samples, then one good sample, then
        // five more consecutive air-cut samples. Should produce exactly
        // TWO issue segments (not fifteen). See F-15 in the April review.
        let mut samples = Vec::new();
        for i in 0..10 {
            samples.push(SimulationCutSample {
                move_index: i,
                sample_index: i,
                position: [i as f64, 0.0, -1.0],
                cumulative_time_s: 0.1 * (i as f64 + 1.0),
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                // all below 0.02
                engagement: Engagement::with_radial_woc(0.005 + i as f64 * 0.001),
                ..SimulationCutSample::test_fixture()
            });
        }
        // One good sample breaks the segment
        samples.push(SimulationCutSample {
            move_index: 10,
            sample_index: 10,
            position: [10.0, 0.0, -1.0],
            cumulative_time_s: 1.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 2.0,
            axial_engagement_mm: 2.0,
            chipload_mm_per_tooth: 0.02,
            effective_chip_thickness_mm: Some(0.02),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            ..SimulationCutSample::test_fixture()
        });
        for i in 0..5 {
            samples.push(SimulationCutSample {
                move_index: 11 + i,
                sample_index: 11 + i,
                position: [11.0 + i as f64, 0.0, -1.0],
                cumulative_time_s: 1.2 + 0.1 * i as f64,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            });
        }

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Two segments, not 15 issues-per-sample.
        assert_eq!(
            trace.issues.len(),
            2,
            "expected 2 coalesced segments, got {}",
            trace.issues.len()
        );
        assert_eq!(trace.summary.issue_count, 2);

        let first = &trace.issues[0];
        assert_eq!(first.kind, SimulationCutIssueKind::AirCut);
        assert_eq!(first.sample_count, 10);
        assert_eq!(first.move_index, 0);
        assert_eq!(first.end_move_index, 9);
        // Duration from t=0.1 to t=1.0
        assert!((first.duration_s - 0.9).abs() < 1e-9);
        // Minimum engagement should be the first sample's 0.005
        assert!((first.min_radial_engagement - 0.005).abs() < 1e-9);

        let second = &trace.issues[1];
        assert_eq!(second.kind, SimulationCutIssueKind::AirCut);
        assert_eq!(second.sample_count, 5);
        assert_eq!(second.move_index, 11);
        assert_eq!(second.end_move_index, 15);

        // Sample-level counters in the summary are still per-sample
        // (not per-segment), so air_cut_time_s accumulates all 15
        // air-cut sample times: 10 × 0.1 + 5 × 0.1 = 1.5s.
        assert!((trace.summary.air_cut_time_s - 1.5).abs() < 1e-9);
    }

    #[test]
    fn trace_flushes_open_segment_at_end_of_stream() {
        // If the sample stream ends mid-segment (last samples are still
        // air-cut), the open segment must be flushed, not lost.
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);
        assert_eq!(trace.issues.len(), 1);
        let seg = &trace.issues[0];
        assert_eq!(seg.kind, SimulationCutIssueKind::AirCut);
        assert_eq!(seg.sample_count, 2);
        assert_eq!(seg.end_move_index, 1);
    }

    #[test]
    fn trace_tracks_open_segments_per_toolpath() {
        // Interleaved samples from two toolpaths: each toolpath's
        // air-cut run should produce its own segment, not bleed into
        // the other's.
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                position: [10.0, 0.0, -1.0],
                cumulative_time_s: 0.15,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.01),
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);
        // Two segments — one per toolpath — not three per-sample issues
        // and not one bleeding across toolpath boundaries.
        assert_eq!(trace.issues.len(), 2);
        let tp0_segs: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.toolpath_id == ToolpathId(0))
            .collect();
        let tp1_segs: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.toolpath_id == ToolpathId(1))
            .collect();
        assert_eq!(tp0_segs.len(), 1);
        assert_eq!(tp1_segs.len(), 1);
        assert_eq!(tp0_segs[0].sample_count, 2);
        assert_eq!(tp1_segs[0].sample_count, 1);
    }

    // --- Peak metrics tracking ---

    #[test]
    fn trace_tracks_peak_chipload_and_axial_doc() {
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                axial_engagement_mm: 1.5,
                chipload_mm_per_tooth: 0.05,
                effective_chip_thickness_mm: Some(0.05),
                engagement: Engagement::with_radial_woc(0.30),
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 20.0,
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -2.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 3.0,
                axial_engagement_mm: 3.0,
                chipload_mm_per_tooth: 0.08,
                effective_chip_thickness_mm: Some(0.08),
                engagement: Engagement::with_radial_woc(0.60),
                removed_volume_est_mm3: 5.0,
                mrr_mm3_s: 50.0,
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        assert!(
            (trace.summary.peak_chipload_mm_per_tooth - 0.08).abs() < 1e-9,
            "Peak chipload should be 0.08, got {}",
            trace.summary.peak_chipload_mm_per_tooth
        );
        assert!(
            (trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9,
            "Peak axial DOC should be 3.0, got {}",
            trace.summary.peak_axial_doc_mm
        );
        assert!(
            (trace.summary.total_removed_volume_est_mm3 - 7.0).abs() < 1e-9,
            "Total removed volume should be 7.0, got {}",
            trace.summary.total_removed_volume_est_mm3
        );
    }

    // --- Multiple toolpaths produce separate summaries ---

    #[test]
    fn trace_multiple_toolpaths_produce_separate_summaries() {
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.40),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                sample_index: 1,
                position: [10.0, 0.0, -2.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 300.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                axial_engagement_mm: 2.0,
                chipload_mm_per_tooth: 0.02,
                effective_chip_thickness_mm: Some(0.02),
                engagement: Engagement::with_radial_woc(0.50),
                removed_volume_est_mm3: 3.0,
                mrr_mm3_s: 15.0,
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(1.0, samples);

        assert_eq!(trace.summary.sample_count, 2);
        assert_eq!(trace.summary.toolpath_count, 2);
        assert_eq!(trace.toolpath_summaries.len(), 2);

        let tp0 = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(0))
            .expect("should have toolpath 0");
        let tp1 = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(1))
            .expect("should have toolpath 1");

        assert_eq!(tp0.sample_count, 1);
        assert_eq!(tp1.sample_count, 1);
        assert!((tp0.total_runtime_s - 0.1).abs() < 1e-9);
        assert!((tp1.total_runtime_s - 0.2).abs() < 1e-9);
        assert!(
            (tp0.total_removed_volume_est_mm3 - 1.0).abs() < 1e-9,
            "TP0 removed volume should be 1.0, got {}",
            tp0.total_removed_volume_est_mm3
        );
        assert!(
            (tp1.total_removed_volume_est_mm3 - 3.0).abs() < 1e-9,
            "TP1 removed volume should be 3.0, got {}",
            tp1.total_removed_volume_est_mm3
        );
    }

    // --- Average engagement is time-weighted ---

    #[test]
    fn trace_average_engagement_is_time_weighted() {
        let samples = vec![
            // 0.1s at engagement=0.20
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.20),
                removed_volume_est_mm3: 0.5,
                mrr_mm3_s: 5.0,
                ..SimulationCutSample::test_fixture()
            },
            // 0.3s at engagement=0.80
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.4,
                segment_time_s: 0.3,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                axial_engagement_mm: 2.0,
                chipload_mm_per_tooth: 0.02,
                effective_chip_thickness_mm: Some(0.02),
                engagement: Engagement::with_radial_woc(0.80),
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 6.67,
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Time-weighted average: (0.20*0.1 + 0.80*0.3) / (0.1+0.3)
        // = (0.02 + 0.24) / 0.4 = 0.26 / 0.4 = 0.65
        let expected_avg = (0.20 * 0.1 + 0.80 * 0.3) / (0.1 + 0.3);
        assert!(
            (trace.summary.average_engagement - expected_avg).abs() < 1e-6,
            "Average engagement should be {}, got {}",
            expected_avg,
            trace.summary.average_engagement
        );
    }

    // --- Hotspots are created per (toolpath_id, semantic_item_id) ---

    #[test]
    fn trace_hotspots_grouped_by_toolpath_and_semantic_id() {
        let samples = vec![
            SimulationCutSample {
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.50),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(1),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.50),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(2),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                move_index: 2,
                sample_index: 2,
                position: [2.0, 0.0, -1.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: crate::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.01,
                effective_chip_thickness_mm: Some(0.01),
                engagement: Engagement::with_radial_woc(0.50),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(1),
                ..SimulationCutSample::test_fixture()
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Hotspots are keyed by (toolpath_id, semantic_item_id).
        // We have 2 unique keys: (0, Some(1)) and (0, Some(2))
        assert_eq!(
            trace.hotspots.len(),
            2,
            "Should have 2 hotspots for 2 distinct semantic_item_ids, got {}",
            trace.hotspots.len()
        );

        // The hotspot for semantic_item_id=1 should have 2 samples
        let hotspot_1 = trace
            .hotspots
            .iter()
            .find(|h| h.semantic_item_id == Some(1))
            .expect("should have hotspot for semantic_item_id=1");
        assert_eq!(
            hotspot_1.sample_index_start, 0,
            "Hotspot 1 sample start should be 0"
        );
        assert_eq!(
            hotspot_1.sample_index_end, 2,
            "Hotspot 1 sample end should be 2"
        );
    }

    // --- Sanitize filename component ---

    #[test]
    fn sanitize_filename_handles_special_chars() {
        assert_eq!(sanitize_filename_component("Adaptive 3D"), "adaptive_3d");
        assert_eq!(
            sanitize_filename_component("my/file:name.ext"),
            "my_file_name_ext"
        );
        assert_eq!(sanitize_filename_component("---test---"), "---test---");
        assert_eq!(sanitize_filename_component(""), "simulation_cut_trace");
        assert_eq!(sanitize_filename_component("___"), "simulation_cut_trace");
    }

    // ── P3: transit-span gating of peak-axial-DOC ───────────────────────

    /// Build a minimal cutting sample for the peak-DOC accumulator tests.
    fn make_sample(
        sample_index: usize,
        axial_doc_mm: f64,
        in_transit_span: bool,
    ) -> SimulationCutSample {
        SimulationCutSample {
            move_index: sample_index,
            sample_index,
            cumulative_time_s: sample_index as f64 * 0.01,
            segment_time_s: 0.01,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm,
            axial_engagement_mm: axial_doc_mm,
            chipload_mm_per_tooth: 0.03,
            effective_chip_thickness_mm: Some(0.03),
            engagement: Engagement::with_radial_woc(0.3),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 50.0,
            in_transit_span,
            ..SimulationCutSample::test_fixture()
        }
    }

    #[test]
    fn peak_doc_includes_cutting_samples() {
        // Wanaka commanded DOC is 3 mm. A clean cutting sample at 3 mm
        // axial DOC should appear in the peak.
        let samples = vec![make_sample(0, 3.0, false)];
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        assert!((trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9);
    }

    #[test]
    fn peak_doc_suppresses_transit_lift_bridge_artifact() {
        // Wanaka TP3 pattern: a transit sample reads 18.59 mm because the
        // cutter is bridging cleared air over uncleared neighbouring stock.
        // It must NOT contaminate peak axial DOC.
        let samples = vec![
            make_sample(0, 3.0, false),
            make_sample(1, 18.59, true), // lift-bridge artifact
            make_sample(2, 3.0, false),
        ];
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        assert!(
            (trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9,
            "transit-span sample at 18.59 mm should be excluded from peak DOC; got {}",
            trace.summary.peak_axial_doc_mm
        );
    }

    #[test]
    fn peak_doc_excludes_purely_transit_streams() {
        // If every sample is in a transit span (extreme case), peak DOC is 0.
        let samples = vec![
            make_sample(0, 18.0, true),
            make_sample(1, 12.0, true),
            make_sample(2, 7.0, true),
        ];
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        assert_eq!(trace.summary.peak_axial_doc_mm, 0.0);
    }

    #[test]
    fn peak_chipload_also_skips_transit_samples() {
        // The same lift-bridge artifact inflates peak_chipload — the gate
        // applies to both extreme-value metrics.
        let mut transit = make_sample(0, 18.0, true);
        transit.chipload_mm_per_tooth = 5.0; // wildly high
        let mut cutting = make_sample(1, 3.0, false);
        cutting.chipload_mm_per_tooth = 0.05;
        let trace = SimulationCutTrace::from_samples(0.5, vec![transit, cutting]);
        assert!(
            trace.summary.peak_chipload_mm_per_tooth < 0.1,
            "transit-span sample must be excluded from peak chipload"
        );
    }

    // ── P4: suppress issues + mark `metrics_not_applicable` for drills ──

    fn make_drill_sample(toolpath_id: usize, sample_index: usize) -> SimulationCutSample {
        // Drill kinematics: is_cutting=true, radial_engagement=0 (dexel can't
        // see Z-only moves).
        let mut s = make_sample(sample_index, 1.0, false);
        s.toolpath_id = ToolpathId(toolpath_id);
        s.engagement.radial_woc_fraction = 0.0;
        s
    }

    #[test]
    fn drill_samples_dont_emit_air_cut_issues() {
        // 100 samples of a drill TP; without the gate they'd produce 1
        // coalesced air-cut issue. With the gate, zero.
        let samples: Vec<_> = (0..100).map(|i| make_drill_sample(7, i)).collect();
        let drill_ids: std::collections::BTreeSet<ToolpathId> =
            std::iter::once(ToolpathId(7)).collect();
        let trace = SimulationCutTrace::from_samples_with_context(
            0.5,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
            &drill_ids,
        );
        assert_eq!(
            trace.issues.len(),
            0,
            "drill TP should emit zero air-cut issues"
        );
    }

    #[test]
    fn drill_toolpath_summary_is_marked_not_applicable() {
        let samples = vec![make_drill_sample(7, 0), make_drill_sample(7, 1)];
        let drill_ids: std::collections::BTreeSet<ToolpathId> =
            std::iter::once(ToolpathId(7)).collect();
        let trace = SimulationCutTrace::from_samples_with_context(
            0.5,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
            &drill_ids,
        );
        let tp = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(7))
            .expect("toolpath 7 should have a summary");
        assert!(
            tp.metrics_not_applicable,
            "drill TP summary should be marked metrics_not_applicable"
        );
    }

    #[test]
    fn non_drill_toolpath_still_emits_issues() {
        // 100 air-cut samples on a non-drill toolpath → still emit 1 coalesced issue.
        let mut samples = Vec::new();
        for i in 0..100 {
            let mut s = make_sample(i, 1.0, false);
            s.toolpath_id = ToolpathId(1);
            s.engagement.radial_woc_fraction = 0.0;
            samples.push(s);
        }
        let drill_ids = std::collections::BTreeSet::new();
        let trace = SimulationCutTrace::from_samples_with_context(
            0.5,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
            &drill_ids,
        );
        assert_eq!(
            trace.issues.len(),
            1,
            "non-drill TP with low engagement should emit 1 coalesced air-cut issue"
        );
        // Summary should NOT be marked metrics_not_applicable.
        let tp = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(1))
            .unwrap();
        assert!(!tp.metrics_not_applicable);
    }

    #[test]
    fn mixed_drill_and_non_drill_isolates_suppression() {
        // 1 drill (ids=7) + 1 non-drill (id=1) toolpath in one trace.
        // Only TP1's air-cut samples should produce issues.
        let mut samples: Vec<_> = (0..50).map(|i| make_drill_sample(7, i)).collect();
        for i in 50..100 {
            let mut s = make_sample(i, 1.0, false);
            s.toolpath_id = ToolpathId(1);
            s.engagement.radial_woc_fraction = 0.0;
            samples.push(s);
        }
        let drill_ids: std::collections::BTreeSet<ToolpathId> =
            std::iter::once(ToolpathId(7)).collect();
        let trace = SimulationCutTrace::from_samples_with_context(
            0.5,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
            &drill_ids,
        );
        assert_eq!(trace.issues.len(), 1, "only non-drill TP emits an issue");
        let drill_summary = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(7))
            .unwrap();
        let mill_summary = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(1))
            .unwrap();
        assert!(drill_summary.metrics_not_applicable);
        assert!(!mill_summary.metrics_not_applicable);
    }

    #[test]
    fn per_toolpath_peak_doc_respects_transit_flag() {
        let samples = vec![
            make_sample(0, 3.0, false),
            make_sample(1, 5.5, true), // lift-bridge artifact (Wanaka TP6 pattern)
        ];
        let trace = SimulationCutTrace::from_samples(0.5, samples);
        let tp = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == ToolpathId(0))
            .expect("toolpath 0 should have a summary");
        assert!(
            (tp.peak_axial_doc_mm - 3.0).abs() < 1e-9,
            "per-TP peak DOC must respect transit flag; got {}",
            tp.peak_axial_doc_mm
        );
    }
}
