//! The diagnostic result types: what a collision, air-cut or plunge scan
//! reports, and how it reaches the MCP and CLI wire.
//!
//! SES-03 moved these seven declarations out of `session/mod.rs`. The P4
//! split had moved the LOGIC that produces them to
//! `session/compute/diagnostics.rs` and left the types behind in the
//! facade, which then imported them back. `mod.rs` re-exports every name
//! here, so no caller's path changes.
//!
//! The four structs derive `Serialize` and the derive IS the wire
//! contract (SES-01). `tests/diagnostics_json_keys_ses01.rs` pins the
//! bytes. The two enums write their own snake_case tags.

use crate::compute::simulate::SimulationResult;
use crate::ids::ToolpathId;

/// Per-toolpath diagnostic summary.
///
/// The derive IS the wire contract (SES-01): every field serialises under
/// its own identifier, in declaration order, with `null` for a `None`.
/// Add a field and it reaches the MCP and CLI wire; a hand-written impl
/// used to drop it in silence. `tests/diagnostics_json_keys_ses01.rs`
/// pins the bytes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolpathDiagnostic {
    pub toolpath_id: ToolpathId,
    pub name: String,
    pub operation_type: String,
    /// Stable op-kind tag (e.g. `"drill"`, `"alignment_pin_drill"`, `"pocket"`).
    /// Lets consumers filter Z-only kinematics ops out of rapid:cut-ratio
    /// signals — see [`crate::compute::catalog::OperationType`].
    pub op_kind: String,
    pub tool_name: String,
    pub move_count: usize,
    pub cutting_distance_mm: f64,
    pub rapid_distance_mm: f64,
    /// Holder/shank collisions on this toolpath. `None` serialises as
    /// `null` and means the check **FAILED or never ran** — never
    /// "measured and clear", which is `Some(0)` (CMP-14). Consumers must
    /// not coerce it to 0: that is the defect commit `70a3db27` removed
    /// from the CLI and this row removes from core.
    pub collision_count: Option<usize>,
    pub rapid_collision_count: usize,
    /// A/M9: generation-time truncated cascade core, XY-projected mm².
    /// `None` serialises as `null` and means **not measured** (this operation
    /// runs no ring cascade) — never "nothing left uncut". See
    /// [`crate::compute::config::ToolpathStats::truncated_core_mm2`], which
    /// carries the wave-16 rename (A6) and the vocabulary it fixes.
    ///
    /// L5 retired the duplicate key `standing_material_mm2`, which named
    /// the wrong quantity. Report-only: no verdict reads this.
    pub truncated_core_mm2: Option<f64>,
    /// B8 (Checkpoint E): the hole-aware sibling of
    /// [`Self::truncated_core_mm2`], off
    /// [`crate::compute::config::ToolpathStats::untouched_material_mm2`].
    /// `None` = not measured; `Some(0.0)` = a cascade ran and left no
    /// unreached core. Narration has carried this split since wave 15; this
    /// is the MCP per-toolpath summary catching up. Report-only.
    pub untouched_material_mm2: Option<f64>,
    /// B8 (Checkpoint E): area the cascade DID ring but where every point was
    /// dropped — the oracle's *standing* (reached, left high), off
    /// [`crate::compute::config::ToolpathStats::reached_uncut_estimate_mm2`].
    /// An ESTIMATOR, not an exact area, and a different quantity from
    /// [`Self::truncated_core_mm2`]; the two must never be summed or
    /// compared. Report-only.
    pub reached_uncut_estimate_mm2: Option<f64>,
    /// Wave D1: XY-projected mm² of finish band that emitted no cutting
    /// because height resolution clipped its Z range away. `None`
    /// serialises as `null` and means **nothing dropped or nothing that
    /// plans bands ran** — never 0.0. The band name and the clipping height
    /// travel with the `geom.unmachined_band` diagnostic message.
    /// Report-only: no verdict reads it.
    pub unmachined_band_area_mm2: Option<f64>,
    /// Wave D1: emitted centreline points over material the tool cannot
    /// physically reach. `None` = the operation emits no centrelines
    /// (**not measured**); `Some(0)` = measured and clean.
    pub tip_float_points: Option<usize>,
    /// Wave D1: worst tip-float residual (mm) left beneath the emitted
    /// centreline. `None` under exactly the same condition as
    /// [`Self::tip_float_points`]. Report-only.
    pub max_tip_float_mm: Option<f64>,
    /// C2: what the shallow band's monotone-cell decomposition did, off
    /// [`crate::compute::config::ToolpathStats::monotone_cells`]. The whole
    /// [`crate::finish::unified_finish::MonotoneCellTotals`] travels, because its
    /// five counters only mean anything together — `regions` is the
    /// denominator of the other four.
    ///
    /// `None` = **not measured**: not a `unified_finish`, or one with
    /// `monotone_cell_decomposition` off, or one that emitted no Shallow
    /// region. Never read as "nothing was decomposed". A non-zero
    /// `membership_fallbacks` / `empty_fallbacks` is the reason this is on
    /// the wire at all: those regions emitted the pre-C2 undivided raster,
    /// and until now nothing on an operator surface said so. Report-only.
    pub monotone_cells: Option<crate::finish::unified_finish::MonotoneCellTotals>,
}

/// Severity bucket for a [`Verdict`]. Ordered: `Critical < Important < Polish`
/// so a sort on severity puts the most important first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerdictSeverity {
    /// Holder/shank collision, rapid-through-stock collision — likely to
    /// damage the part or the tool. Block export until resolved.
    Critical,
    /// Gate exceeded, toolpath generated zero in-material cut, unsafe plunge —
    /// the operator must consciously decide before proceeding.
    Important,
    /// Air-cut high, low engagement, slow cycle time — cosmetic / efficiency.
    Polish,
}

/// What triggered a [`Verdict`]. Stable tag so MCP consumers can branch on
/// kind without parsing the human-readable headline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictKind {
    HolderCollision,
    /// A holder/shank collision check that could not answer (CMP-14).
    /// Distinct from [`Self::HolderCollision`] on purpose: one says the
    /// toolpath collides, this one says nobody knows.
    HolderCheckFailed,
    RapidCollision,
    PlungeStress,
    AirCut,
    GeneratedEmpty,
    /// The stock's alignment pins do not key the flip a setup is
    /// programmed for (CMP-27). A statement about REGISTRATION, not about
    /// a toolpath: a pin pair that is invariant under the wrong symmetry
    /// seats in both orientations and both look right.
    AlignmentPinsUnkeyed,
    /// A gate declined to produce a verdict because the metric it reads is
    /// not measurable on this trace. Checkpoint D Q2, 2026-08-04 — see
    /// [`crate::stock::sim_measurability`]. This is **not** a warning about the
    /// toolpath; it is a statement about the simulation, and it carries the
    /// reason plus what remains valid (collision detection always does).
    MeasurabilityAbstained,
}

impl VerdictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HolderCollision => "holder_collision",
            Self::HolderCheckFailed => "holder_check_failed",
            Self::RapidCollision => "rapid_collision",
            Self::PlungeStress => "plunge_stress",
            Self::AirCut => "air_cut",
            Self::GeneratedEmpty => "generated_empty",
            Self::AlignmentPinsUnkeyed => "alignment_pins_unkeyed",
            Self::MeasurabilityAbstained => "measurability_abstained",
        }
    }
}

/// Backing evidence for a verdict — the move / Z that triggered the call.
/// Optional because some verdicts (e.g. holder collision summed across a
/// project) don't have a single representative move.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VerdictEvidence {
    pub move_index: Option<usize>,
    pub z_value: Option<f64>,
    pub count: Option<usize>,
}

/// Borrow view over the simulation evidence that
/// [`crate::session::ProjectSession::diagnostics_with_evidence`] (and friends) need.
///
/// Both the core session and the GUI hold the same evidence — but the
/// GUI's copy lives in a different struct (viz `SimulationResults` +
/// `SimulationChecks`). Both sides key boundaries by the shared
/// strongly-typed [`ToolpathId`] (R3 unified the previously-`usize`
/// core side). This shared borrow view lets both callers feed
/// `diagnostics_with_evidence` without forcing a copy of the full
/// simulation result.
#[derive(Default)]
pub struct ProjectEvidence<'a> {
    /// `(toolpath_id, start_move, end_move)` per simulation boundary —
    /// the rapid-collision counters use this to attribute counts to
    /// the right toolpath. The viz side flattens its
    /// `Vec<ToolpathBoundary>` into this shape.
    pub boundaries: Vec<(ToolpathId, usize, usize)>,
    pub rapid_collisions: &'a [crate::stock::collision::RapidCollision],
    pub rapid_collision_move_indices: &'a [usize],
    pub cut_trace: Option<&'a crate::stock::simulation_cut::SimulationCutTrace>,
    /// Per-toolpath holder/shank collision outcomes, supplied by the
    /// caller from its most recent dedicated collision check. Empty =
    /// no holder evidence (no holder verdicts are emitted), and so is a
    /// toolpath that is absent from a non-empty list.
    ///
    /// CMP-14: the element carries three states, not a count. A check
    /// that failed reads [`crate::compute::collision_check::HolderCollisionCheck::Failed`],
    /// which is not the same claim as `Measured(0)`.
    ///
    /// Evidence is an INPUT here on purpose: `diagnostics_with_evidence`
    /// used to run `collision_check` (spatial-index build + full
    /// toolpath sweep) per toolpath internally, and the GUI's setup
    /// panel calls project diagnostics every frame — on a generated
    /// project that recomputed every toolpath's collision sweep at
    /// frame rate (the 2026-06-11 setup-tab lag). Batch callers that
    /// want the sweep use [`crate::session::ProjectSession::holder_collision_counts`].
    pub holder_collisions: Vec<(
        ToolpathId,
        crate::compute::collision_check::HolderCollisionCheck,
    )>,
    /// Simulation cell size (mm) the trace was captured at, when known.
    ///
    /// Read only by [`crate::stock::sim_measurability`], and only to enrich the
    /// reason payload of a `CellTooCoarseForTipContact` abstention with the
    /// number the operator would have to change. It never decides a verdict,
    /// so `None` costs nothing but a vaguer message.
    pub resolution_mm: Option<f64>,
}

impl<'a> ProjectEvidence<'a> {
    /// Build evidence from a core [`SimulationResult`].
    pub(crate) fn from_simulation(sim: &'a SimulationResult) -> Self {
        let boundaries = sim
            .boundaries
            .iter()
            .map(|b| (b.id, b.start_move, b.end_move))
            .collect();
        Self {
            boundaries,
            rapid_collisions: &sim.rapid_collisions,
            rapid_collision_move_indices: &sim.rapid_collision_move_indices,
            cut_trace: sim.cut_trace.as_deref(),
            holder_collisions: Vec::new(),
            resolution_mm: Some(sim.column_grid_cell_mm),
        }
    }

    /// Same as `Self::from_simulation` plus holder-collision counts
    /// (see the `holder_collisions` field docs for why these are an
    /// input rather than computed internally).
    pub fn from_simulation_with_holder_collisions(
        sim: &'a SimulationResult,
        holder_collisions: Vec<(
            ToolpathId,
            crate::compute::collision_check::HolderCollisionCheck,
        )>,
    ) -> Self {
        Self {
            holder_collisions,
            ..Self::from_simulation(sim)
        }
    }
}

/// Structured project-level verdict. Replaces the legacy single-line
/// `ProjectDiagnostics::verdict` string, which L11 (`bbaa193e`,
/// 2026-09-17) deleted from the struct and from the wire. Read
/// [`ProjectDiagnostics::verdicts`]. The old single line is
/// `verdicts[0].headline`, or `"OK"` when the list is empty.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Verdict {
    pub severity: VerdictSeverity,
    pub kind: VerdictKind,
    /// One-line human-readable headline, names the offending TPs.
    pub headline: String,
    /// Toolpath ids this verdict refers to (may be empty for project-wide
    /// signals).
    pub offender_toolpath_ids: Vec<ToolpathId>,
    /// Suggested next action ("increase retract_z…", "set boundary…").
    pub fix_hint: String,
    pub evidence: VerdictEvidence,
}

/// Project-level diagnostics summary.
///
/// The derive IS the wire contract — see [`ToolpathDiagnostic`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectDiagnostics {
    pub total_runtime_s: f64,
    /// Air-cut time ÷ **total runtime (cutting + rapids)** × 100.
    /// The measure every shipped threshold is tuned against — the GUI's 40%
    /// banner, the CLI's 40% verdict, and
    /// [`crate::compute::catalog::OperationType::air_cut_high_threshold_pct`].
    pub air_cut_pct_of_total_runtime: f64,
    /// Air-cut time ÷ **cutting runtime (rapids excluded)** × 100 — always
    /// ≥ [`Self::air_cut_pct_of_total_runtime`]. This is what the MCP
    /// `narrate_toolpath` air-cut line reports and what `CLAUDE.md`'s metric
    /// caveats describe. No threshold is applied to it.
    pub air_cut_pct_of_cutting_time: f64,
    pub average_engagement: f64,
    /// Holder/shank collisions summed over every toolpath the evidence
    /// MEASURED. It is not the whole project's answer unless
    /// [`Self::collision_checks_failed`] is zero — read the two together.
    pub collision_count: usize,
    /// How many toolpaths' holder/shank checks failed (CMP-14). Non-zero
    /// means [`Self::collision_count`] is a partial sum and the project
    /// has no clean bill of health. The CLI carries the same pair.
    pub collision_checks_failed: usize,
    pub rapid_collision_count: usize,
    pub per_toolpath: Vec<ToolpathDiagnostic>,
    /// Severity-ranked list of structured verdicts (critical → polish).
    /// Empty when the project has no findings.
    pub verdicts: Vec<Verdict>,
}

// ── Serde for the verdict enums ────────────────────────────────────────
//
// SES-01: the four diagnostic STRUCTS derive `Serialize`; their
// hand-written impls wrote exactly what the derive writes and are gone.
// These two enums stay hand-written. They are not the derive's output:
// each writes a snake_case string tag, and `VerdictKind` reads `as_str`,
// the one spelling every surface shares.

impl serde::Serialize for VerdictSeverity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            Self::Critical => "critical",
            Self::Important => "important",
            Self::Polish => "polish",
        };
        serializer.serialize_str(s)
    }
}

impl serde::Serialize for VerdictKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
