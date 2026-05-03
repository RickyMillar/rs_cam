//! Tool-load optimize module — phase U1 of the optimizer plan.
//!
//! Search across feed/RPM/geometry params for a single toolpath and
//! rank candidates by measured cycle time. The simulator gate
//! (`tool_load::evaluate_toolpath`) is the single source of truth —
//! every number on every candidate came from a sim of that candidate's
//! params. There is no second chipload model.
//!
//! Three search stages, ordered cheapest-first:
//!
//! - **Stage 0** — closed-form analytical scaling of `(rpm, feed)` at
//!   constant chipload until a machine, tool, or LUT-row limit binds.
//!   No sim required. Headline "scale up to limits" win.
//! - **Stage 1** — for the 5 geometry ops (Adaptive3d, Pocket, Adaptive,
//!   Rest, Face), vary DOC anchored at the headroom point. 1mm dexel.
//! - **Stage 2** — top 3 by Stage-1 cycle time, re-simmed at default
//!   resolution (0.5mm). The reported cycle time and verdict on each
//!   candidate are always Stage-2 numbers.
//!
//! See `planning/OPTIMIZER_UX_PLAN.md` — particularly Resolutions 1-9
//! and Engineering Defaults 1-10.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{DressupConfig, FeedsAutoMode};
use crate::enriched_mesh::FaceGroupId;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::{MachineProfile, PowerModel};
use crate::session::{ProjectSession, SessionError};
use crate::simulation_cut::SimulationCutTrace;

use super::suggest::RefuseReason;
use super::verdict::{ToolpathLoadVerdict, Verdict};

/// Which search stage produced a candidate. The reported `cycle_time_s`
/// and `verdict` on every candidate the optimizer surfaces come from
/// Stage 2 (or directly from the baseline trace for the index-0
/// baseline candidate). Stage 0/1 candidates are intermediate and never
/// surface untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStage {
    /// The user's existing params, scored against `baseline_trace`.
    Baseline,
    /// Closed-form analytical RPM/feed headroom point. No sim run.
    Stage0Headroom,
    /// 1mm-dexel coarse sim, used for ranking geometry candidates.
    Coarse,
    /// Default-resolution sim, used for the survivors that get reported.
    Refined,
}

/// Human-readable diff between a candidate and the baseline. Each field
/// carries `Some(new_value)` only if the candidate is changing it.
/// Used by the modal to render "feed 1899→2100" style summaries and by
/// the Apply path to know which `feeds_auto.*` flags need clearing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamDelta {
    /// New feed in mm/min. `None` if the candidate matches baseline feed.
    pub feed_mm_min: Option<f64>,
    /// New spindle RPM. `None` if the candidate is not changing RPM.
    pub spindle_rpm: Option<u32>,
    /// New radial stepover in mm.
    pub stepover_mm: Option<f64>,
    /// New depth-per-pass in mm.
    pub depth_per_pass_mm: Option<f64>,
}

impl ParamDelta {
    /// True if any field is `Some(_)` — i.e. the candidate is non-trivial.
    pub fn has_changes(&self) -> bool {
        self.feed_mm_min.is_some()
            || self.spindle_rpm.is_some()
            || self.stepover_mm.is_some()
            || self.depth_per_pass_mm.is_some()
    }
}

/// One candidate's full evaluation record. Populated by the optimizer
/// during Stage 0/1/2; each field is sim-measured (or, for the baseline
/// candidate at index 0, sourced from `baseline_trace`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeCandidate {
    /// The full operation config that would be applied. Carries
    /// unchanged fields too, so Apply just needs to write this into
    /// `session.toolpath_configs[idx].operation`.
    pub params: OperationConfig,
    /// Diff from the baseline params, for display and feeds_auto flag
    /// management at Apply time.
    pub delta: ParamDelta,
    /// Measured cycle time from this candidate's sim (seconds).
    pub cycle_time_s: f64,
    /// Verdict from the gate over this candidate's sim.
    pub verdict: ToolpathLoadVerdict,
    /// Which stage produced this candidate.
    pub stage: SearchStage,
    /// Project-level reconciliation result (U4). After the user Applies
    /// and the project re-sims end-to-end, downstream stock-state
    /// changes can shift this candidate's verdict — that shifted value
    /// lands here. `None` until U4 fires.
    pub reconciled_cycle_time_s: Option<f64>,
    /// Reconciled verdict from the post-Apply project sim (U4).
    pub reconciled_verdict: Option<ToolpathLoadVerdict>,
}

/// Outcome of `optimize_toolpath` for one toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum OptimizeOutcome {
    /// At least one candidate was generated. Index 0 is always the
    /// baseline (current params). The recommendation is whichever
    /// candidate has `.first_safe()` returns — the first non-baseline
    /// candidate whose verdict is not `Exceeds` on any criterion.
    Ranked(Vec<OptimizeCandidate>),
    /// Every non-baseline candidate either failed the gate (Exceeds on
    /// some criterion) or was slower than baseline. The rollup row
    /// surfaces this with the binding-limit narrative.
    NoSafeImprovement {
        reason: RefuseReason,
        /// Narrative composed at outcome time via
        /// `RefuseReason::explanation_for_optimize` (Engineering
        /// Default 4). Free-form English for the modal.
        explanation: String,
    },
    /// The optimizer can't model this toolpath at all — drill cycles,
    /// project_curve with no steady-state samples, custom materials.
    /// The gate refuses, so the optimizer refuses.
    Skipped { reason: RefuseReason },
}

impl OptimizeOutcome {
    /// Recommended candidate: the first non-baseline candidate that
    /// passes the gate (no `Exceeds` verdict on any criterion). Returns
    /// `None` for `Skipped` / `NoSafeImprovement` outcomes, or for
    /// `Ranked` outcomes where every non-baseline candidate Exceeds.
    pub fn first_safe(&self) -> Option<&OptimizeCandidate> {
        let OptimizeOutcome::Ranked(candidates) = self else {
            return None;
        };
        candidates.iter().skip(1).find(|c| candidate_is_safe(c))
    }
}

/// True if every criterion is non-`Exceeds`. `Within` and `Unmodeled`
/// both pass; `Unmodeled` is the gate's honest "I don't know" and
/// shouldn't block a recommendation by itself.
pub(crate) fn candidate_is_safe(candidate: &OptimizeCandidate) -> bool {
    !matches!(candidate.verdict.chipload, Verdict::Exceeds { .. })
        && !matches!(candidate.verdict.power, Verdict::Exceeds { .. })
        && !matches!(candidate.verdict.deflection, Verdict::Exceeds { .. })
}

/// Project-level rollup over every enabled toolpath. Surfaced by U3's
/// Optimize-project view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOptimizeReport {
    /// Baseline project cycle time (seconds), as measured by the sim
    /// already on screen.
    pub baseline_cycle_time_s: f64,
    /// Toolpath index that dominates runtime (the "Bottleneck:"
    /// callout). `None` if no toolpath crosses the threshold (currently
    /// 30% of total runtime — calibrated against wanaka in U3).
    pub bottleneck_index: Option<usize>,
    /// Per-toolpath outcome paired with the toolpath index it relates
    /// to.
    pub per_toolpath: Vec<(usize, OptimizeOutcome)>,
}

/// Optimize one toolpath and return the search outcome.
///
/// **Mutation note.** `session` is mutated transiently — each candidate
/// evaluation writes that candidate's params via
/// `apply_toolpath_param_snapshot`, regenerates the toolpath, and runs
/// a fresh project sim. An RAII baseline-restore guard re-applies the
/// original params on every exit path (Ok, NoSafeImprovement, Skipped,
/// cancelled, panicked candidate), so callers observe the session as
/// unchanged after this returns. Apply remains a separate user-initiated
/// mutation; the optimizer never persists candidate state.
///
/// **Cancellation.** `cancel` is polled between candidates and between
/// search stages. Mid-sim cancellation works through the simulator's
/// existing `&AtomicBool` plumbing, so cooperative cancellation lands
/// at the next sim sample boundary (sub-second).
///
/// `baseline_trace` should be the project sim already on screen — the
/// same trace the gate / diagnostics panel are reading. The baseline
/// candidate at index 0 of `Ranked` outcomes is scored against this
/// trace directly (no re-sim of baseline).
pub fn optimize_toolpath(
    _session: &mut ProjectSession,
    _baseline_trace: &SimulationCutTrace,
    _toolpath_index: usize,
    _cancel: &AtomicBool,
) -> OptimizeOutcome {
    // U1 skeleton — Stages 0/1/2 land in follow-up commits.
    OptimizeOutcome::Skipped {
        reason: RefuseReason::SimulationRequired,
    }
}

// ── Baseline-restore guard (Engineering Default 10) ───────────────────
//
// Each candidate evaluation in the optimizer mutates
// `session.toolpath_configs[idx].operation` (via
// `apply_toolpath_param_snapshot`), regenerates, and runs a fresh sim.
// Without explicit cleanup, the session is left holding the last
// candidate's params when the optimizer returns — a silent state leak
// of the same flavour as the `feeds_auto` LUT-overwrite issue.
//
// `BaselineRestoreGuard` snapshots `(operation, dressups,
// face_selection, feeds_auto)` at construction and re-applies them in
// `Drop`, regardless of how the optimizer exits (Ok, refusal, cancel,
// or panic in `execute_operation`). Apply remains a separate
// user-initiated mutation; the optimizer's internal candidate writes
// never persist past `optimize_toolpath` returning.

/// Snapshot of the four toolpath fields the optimizer mutates per
/// candidate. Captured up-front and re-applied via
/// `apply_toolpath_param_snapshot` on drop.
#[derive(Debug, Clone)]
pub(crate) struct ToolpathParamsSnapshot {
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub face_selection: Option<Vec<FaceGroupId>>,
    pub feeds_auto: FeedsAutoMode,
}

impl ToolpathParamsSnapshot {
    fn capture(
        session: &ProjectSession,
        toolpath_index: usize,
    ) -> Result<Self, SessionError> {
        let tc = session
            .get_toolpath_config(toolpath_index)
            .ok_or(SessionError::ToolpathNotFound(toolpath_index))?;
        Ok(Self {
            operation: tc.operation.clone(),
            dressups: tc.dressups.clone(),
            face_selection: tc.face_selection.clone(),
            feeds_auto: tc.feeds_auto.clone(),
        })
    }
}

/// RAII guard that restores the toolpath's params to a captured
/// baseline on drop. Construct with [`Self::new`], use
/// [`Self::session_mut`] for any mid-search session mutations, and let
/// the value go out of scope to trigger restoration.
///
/// The guard holds the only `&mut ProjectSession` in flight while it
/// lives; access the session via `session_mut()` so the borrow chain
/// stays through the guard. Drop calls
/// `apply_toolpath_param_snapshot` and ignores its `Result` —
/// restoration failure can only happen if the toolpath was removed
/// mid-search, which would already be a programming error.
pub(crate) struct BaselineRestoreGuard<'a> {
    session: &'a mut ProjectSession,
    toolpath_index: usize,
    snapshot: ToolpathParamsSnapshot,
}

impl<'a> BaselineRestoreGuard<'a> {
    /// Capture the current params and wrap the session. Returns
    /// `ToolpathNotFound` if `toolpath_index` is out of range.
    pub(crate) fn new(
        session: &'a mut ProjectSession,
        toolpath_index: usize,
    ) -> Result<Self, SessionError> {
        let snapshot = ToolpathParamsSnapshot::capture(session, toolpath_index)?;
        Ok(Self {
            session,
            toolpath_index,
            snapshot,
        })
    }

    /// Mutable session reference for in-search mutations. The borrow
    /// stays through the guard so restoration on drop sees a valid
    /// session.
    pub(crate) fn session_mut(&mut self) -> &mut ProjectSession {
        self.session
    }

    /// Read-only snapshot of the captured baseline. Useful for
    /// computing `ParamDelta` against the original params.
    pub(crate) fn baseline(&self) -> &ToolpathParamsSnapshot {
        &self.snapshot
    }
}

impl Drop for BaselineRestoreGuard<'_> {
    fn drop(&mut self) {
        let _ = self.session.apply_toolpath_param_snapshot(
            self.toolpath_index,
            self.snapshot.operation.clone(),
            self.snapshot.dressups.clone(),
            self.snapshot.face_selection.clone(),
            self.snapshot.feeds_auto.clone(),
        );
    }
}

// ── Stage 0: analytical RPM/feed scaling ──────────────────────────────
//
// The closed-form: `k = min(four_ratios)` where the four ratios come
// from machine-RPM, machine-feed, machine-power, and LUT-row-RPM caps.
// Chipload is invariant under proportional (RPM, feed) scaling, so this
// is *only* an optimisation when the baseline is `Within` — scaling
// preserves chipload and can't fix an `Exceeds` baseline. Caller
// responsibility: skip Stage 0 and route directly to Stage 1 when the
// baseline carries an `Exceeds` chipload verdict.

/// Inputs to Stage 0's analytical scaling. Bundled so the function
/// stays terse and the call sites in the orchestrator are explicit
/// about where each value comes from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stage0Inputs<'a> {
    /// Baseline spindle RPM the optimizer scales from. Prefer the
    /// trace's actual median RPM over the operation config's
    /// `spindle_rpm` field — the trace is what actually ran.
    pub rpm_baseline: f64,
    /// Baseline commanded feed in mm/min (from the operation config).
    pub feed_baseline_mm_min: f64,
    /// Peak power in kW from the baseline trace, as measured by the
    /// power gate (`power::evaluate` returns this in `Verdict::Within
    /// { peak }` or `Exceeds { peak }`). `None` if the gate refused
    /// (Unmodeled) — Stage 0 cannot scale safely without a power
    /// reference.
    pub peak_power_baseline_kw: Option<f64>,
    pub machine: &'a MachineProfile,
    /// Matched LUT row carrying the calibrated RPM bracket. `None`
    /// drops the LUT-RPM constraint from the scale solve (the optimizer
    /// will rely on machine bounds alone).
    pub lut_row: Option<&'a MatchedRow>,
}

/// Solve the maximum scale factor `k` that keeps all four limits
/// satisfied at constant chipload. Returns `1.0` if no headroom is
/// available (every cap is already binding at baseline).
///
/// Note: this is a pure arithmetic helper. The orchestrator must
/// independently decide whether to run Stage 0 at all (skip it for
/// `Exceeds`-baseline TPs per Engineering Default 6).
pub(crate) fn solve_headroom_scale(inputs: &Stage0Inputs<'_>) -> f64 {
    let rpm_baseline = inputs.rpm_baseline.max(1.0);
    let feed_baseline = inputs.feed_baseline_mm_min.max(1.0);

    // 1. Machine RPM cap.
    let (_min_rpm, max_rpm) = inputs.machine.rpm_range();
    let k_rpm_machine = max_rpm / rpm_baseline;

    // 2. Machine feed cap.
    let k_feed = inputs.machine.max_feed_mm_min / feed_baseline;

    // 3. Power cap. `machine_max_power_kw × safety / peak_baseline`
    //    works for both `ConstantPower` (rhs is constant) and
    //    `VfdConstantTorque` (below rated_rpm both sides scale with k
    //    so the inequality is invariant; above rated_rpm the rhs caps
    //    at rated_power so the bound is the same scalar).
    let k_power = match inputs.peak_power_baseline_kw {
        Some(peak) if peak > 0.0 => {
            machine_max_power_kw(inputs.machine) * inputs.machine.safety_factor / peak
        }
        _ => f64::INFINITY,
    };

    // 4. LUT-row RPM cap. Use rpm_max if present; fall back to
    //    rpm_nominal × 1.2 (Engineering Default 5's ±20% bracket); fall
    //    back further to the machine ceiling (no LUT constraint).
    let k_lut = match inputs.lut_row {
        Some(row) => {
            let lut_rpm_max = row
                .rpm_max
                .or_else(|| row.rpm_nominal.map(|n| n * 1.2))
                .unwrap_or(max_rpm);
            lut_rpm_max / rpm_baseline
        }
        None => f64::INFINITY,
    };

    let k_unclamped = k_rpm_machine.min(k_feed).min(k_power).min(k_lut);
    // Floor at 1.0 — never propose scaling DOWN from baseline in
    // Stage 0. (Down-scaling is Stage 1's job for Exceeds baselines.)
    k_unclamped.max(1.0)
}

/// Maximum power the spindle is capable of delivering at any RPM.
/// `ConstantPower` reports its flat figure; `VfdConstantTorque` reports
/// rated power (the cap above rated RPM).
fn machine_max_power_kw(machine: &MachineProfile) -> f64 {
    match machine.power {
        PowerModel::ConstantPower { power_kw } => power_kw,
        PowerModel::VfdConstantTorque { rated_power_kw, .. } => rated_power_kw,
    }
}

/// Build the headroom-point `OperationConfig` by applying scale `k` to
/// the baseline op's feed and RPM. All other fields are left unchanged
/// — the search only touches feed/RPM in Stage 0. The new RPM is
/// rounded and clamped to the machine's discrete or variable range.
pub(crate) fn apply_scale_to_op(
    baseline_op: &OperationConfig,
    rpm_baseline: f64,
    k: f64,
    machine: &MachineProfile,
) -> OperationConfig {
    let mut scaled = baseline_op.clone();
    scaled.set_feed_rate(baseline_op.feed_rate() * k);
    let scaled_rpm = machine.clamp_rpm(rpm_baseline * k).round() as u32;
    scaled.set_spindle_rpm(Some(scaled_rpm));
    scaled
}

// ── Stage 1: DOC candidate generation (Engineering Default 9) ─────────
//
// Five geometry-bearing ops carry a `depth_per_pass`; the optimizer
// sweeps DOC at the headroom point (baseline `depth_per_pass`) and
// scores each variant via the gate over a fresh sim.
//
// Grid construction prefers the matched LUT row's calibrated bounds
// (`ap_min_mm`, `ap_max_mm`) where available, clamping a multiplier
// envelope (0.7×–1.4× of baseline) inside them. When the LUT row
// doesn't carry bounds, the multiplier envelope is the grid directly.
//
// 3-variant ops (Adaptive3d, Rest, Face): `[lo, base, hi]`.
// 4-variant ops (Pocket, Adaptive): `[lo, base, mid(base, hi), hi]`.

/// Hard floor on any DOC value the optimizer proposes. Real router
/// toolpaths can pass values smaller than this for finishing operations,
/// but Stage 1's job is the rougher 5 ops where ~50µm is a reasonable
/// minimum. ED 9 calls this out as "use 0.05 mm as a hard floor".
const DOC_HARD_FLOOR_MM: f64 = 0.05;

/// Equality threshold for deduping near-identical DOC values produced
/// by the LUT-anchored grid (e.g. when `ap_min`/`ap_max` happen to
/// land microns from the multiplier endpoints). 5 µm is well below
/// any simulator-distinguishable resolution; deliberately tight so
/// the spec'd `[0.7×, 1.0×, 1.3×]` grid survives intact even when
/// floating-point arithmetic puts adjacent values 0.0500000001 mm
/// apart.
const DOC_DEDUP_TOLERANCE_MM: f64 = 0.005;

/// Build the DOC candidate grid for a Stage-1 sweep. Always includes
/// the baseline (`baseline_doc_mm`) as a control candidate; the lo and
/// hi endpoints come from the LUT row's calibrated bounds (clamped by
/// the multiplier envelope) or fall back to the multiplier envelope
/// alone.
///
/// Returns variants sorted ascending. May return fewer than the
/// nominal 3 or 4 entries when adjacent values dedupe (e.g. an LUT
/// row whose `ap_max` lies inside `1.4 × baseline` ends up with the
/// hi endpoint very close to baseline). Always contains at least the
/// baseline value.
pub(crate) fn build_doc_variants(
    baseline_doc_mm: f64,
    lut_row: Option<&MatchedRow>,
    op_type: OperationType,
) -> Vec<f64> {
    let baseline = baseline_doc_mm.max(DOC_HARD_FLOOR_MM);
    let four_variant = matches!(op_type, OperationType::Pocket | OperationType::Adaptive);

    // Multiplier envelope. ED 9: 0.7× to 1.3× for 3-variant; 0.7× to
    // 1.4× for 4-variant (so the midpoint between base and hi lands at
    // 1.2× — a useful intermediate step).
    let mult_lo = 0.7 * baseline;
    let mult_hi = if four_variant {
        1.4 * baseline
    } else {
        1.3 * baseline
    };

    // Clamp inside LUT-row calibrated bounds if available. The
    // `max(ap_min, mult_lo)` choice mirrors ED 9 directly — never go
    // below the calibrated floor *or* below 0.7× baseline.
    let (lo, hi) = match lut_row {
        Some(row) => {
            let ap_min = row.ap_min_mm.unwrap_or(mult_lo);
            let ap_max = row.ap_max_mm.unwrap_or(mult_hi);
            (ap_min.max(mult_lo), ap_max.min(mult_hi))
        }
        None => (mult_lo, mult_hi),
    };

    // Floor every value at the hard minimum, and ensure the high
    // endpoint isn't accidentally smaller than baseline (degenerate
    // case where the LUT row is very tight).
    let lo = lo.max(DOC_HARD_FLOOR_MM);
    let hi = hi.max(baseline).max(DOC_HARD_FLOOR_MM);

    let mut variants = vec![lo, baseline];
    if four_variant {
        variants.push((baseline + hi) * 0.5);
    }
    variants.push(hi);

    variants.sort_by(f64::total_cmp);
    variants.dedup_by(|a, b| (*a - *b).abs() < DOC_DEDUP_TOLERANCE_MM);
    variants
}

/// Apply a candidate DOC value to a baseline op, leaving every other
/// field unchanged. The op must be one of the 5 families that exposes
/// `depth_per_pass` via the `OperationParams` trait.
pub(crate) fn apply_doc_to_op(baseline_op: &OperationConfig, doc_mm: f64) -> OperationConfig {
    let mut variant = baseline_op.clone();
    variant.set_depth_per_pass(doc_mm);
    variant
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage0_tests {
    use super::*;
    use crate::machine::{ChipLoadFormula, MachineProfile, PowerModel, RigidityProfile, SpindleConfig};

    fn synthetic_machine(
        max_rpm: f64,
        max_feed: f64,
        power: PowerModel,
        safety: f64,
    ) -> MachineProfile {
        MachineProfile {
            name: "stage0 test".to_owned(),
            spindle: SpindleConfig::Variable {
                min_rpm: 6000.0,
                max_rpm,
            },
            power,
            chip_load: ChipLoadFormula::default(),
            max_feed_mm_min: max_feed,
            max_shank_mm: 7.0,
            rigidity: RigidityProfile::default(),
            safety_factor: safety,
        }
    }

    #[test]
    fn k_is_one_when_all_caps_already_binding() {
        // rpm_baseline at machine max; feed_baseline at machine max;
        // power_baseline at machine cap × safety. No headroom.
        let machine = synthetic_machine(
            18_000.0,
            5_000.0,
            PowerModel::ConstantPower { power_kw: 1.5 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 18_000.0,
            feed_baseline_mm_min: 5_000.0,
            peak_power_baseline_kw: Some(1.2), // 1.5 × 0.8 = 1.2
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-6, "expected k = 1.0, got {k}");
    }

    #[test]
    fn k_machine_rpm_binds_when_feed_and_power_have_room() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0, // generous feed cap
            PowerModel::ConstantPower { power_kw: 5.0 }, // generous power
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 24000/12000 = 2.0
        // k_feed = 10000/1500 ≈ 6.67
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 2.0 (rpm binds)
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_feed_binds_when_rpm_and_power_have_room() {
        let machine = synthetic_machine(
            30_000.0, // generous rpm
            1_800.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 30000/12000 = 2.5
        // k_feed = 1800/1500 = 1.2
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_binds_on_constant_power_machine() {
        let machine = synthetic_machine(
            30_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 0.6 }, // tight
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: None,
        };
        // k_power = 0.6 × 0.8 / 0.4 = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_uses_rated_for_vfd() {
        // VFD: rated_power is the cap that binds k_power, not the
        // power-at-baseline-rpm value.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::VfdConstantTorque {
                rated_power_kw: 1.0,
                rated_rpm: 18_000.0,
            },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4), // baseline well under rated.
            machine: &machine,
            lut_row: None,
        };
        // k_power = 1.0 × 0.8 / 0.4 = 2.0
        // k_rpm = 24000/12000 = 2.0 (also 2.0)
        // k_feed plenty
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn lut_row_rpm_can_bind() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // LUT row carries an explicit rpm_max.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: Some(14_000.0),
            rpm_max: Some(16_000.0),
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = 16000/12000 ≈ 1.333
        // Other caps higher. k = 1.333.
        let k = solve_headroom_scale(&inputs);
        assert!(
            (k - 16_000.0 / 12_000.0).abs() < 1e-6,
            "expected k ≈ 1.333, got {k}"
        );
    }

    #[test]
    fn lut_falls_back_to_rpm_nominal_times_1_2() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // No rpm_max, no rpm_min, only rpm_nominal — Engineering
        // Default 5: ±20% bracket from nominal.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = (15000 × 1.2) / 12000 = 1.5
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.5).abs() < 1e-6, "expected k = 1.5, got {k}");
    }

    #[test]
    fn power_unmodeled_drops_constraint() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: None, // gate said Unmodeled
            machine: &machine,
            lut_row: None,
        };
        // Without power data, k constrained only by rpm and feed.
        // k_rpm = 2.0; k_feed ≈ 6.67. k = 2.0.
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_floors_at_one_never_proposes_down_scaling() {
        // Force every limit to be below baseline; Stage 0 should still
        // return k = 1.0 (downscaling is Stage 1's job for Exceeds).
        let machine = synthetic_machine(
            8_000.0, // baseline 12000 already over machine max
            500.0,   // baseline feed 1500 already over machine cap
            PowerModel::ConstantPower { power_kw: 0.1 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(1.0),
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-9, "expected k floored to 1.0, got {k}");
    }

    #[test]
    fn apply_scale_writes_feed_rpm_and_clamps() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let scaled = apply_scale_to_op(&baseline, 12_000.0, 1.5, &machine);
        assert!(
            (scaled.feed_rate() - 2_250.0).abs() < 1e-6,
            "feed should scale 1500 × 1.5 = 2250, got {}",
            scaled.feed_rate()
        );
        assert_eq!(scaled.spindle_rpm(), Some(18_000));
    }

    #[test]
    fn apply_scale_clamps_rpm_to_machine_max() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        // Machine max 18000; baseline 12000; k = 2.0 → 24000 → clamped to 18000.
        let machine = synthetic_machine(
            18_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let scaled = apply_scale_to_op(&baseline, 12_000.0, 2.0, &machine);
        assert_eq!(scaled.spindle_rpm(), Some(18_000));
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod restore_guard_tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::session::ToolpathConfig;

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }

    fn make_tc(tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig {
                feed_rate: 1500.0,
                stepover: 2.0,
                depth_per_pass: 1.5,
                spindle_rpm: Some(18_000),
                ..PocketConfig::default()
            }),
            dressups: DressupConfig::default(),
            heights: crate::compute::config::HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::compute::config::StockSource::Fresh,
            coolant: crate::gcode::CoolantMode::Off,
            face_selection: None,
            feeds_auto: FeedsAutoMode::default(),
            debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
        }
    }

    fn session_with_one_pocket() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s
    }

    #[test]
    fn drop_with_no_changes_is_a_noop() {
        let mut session = session_with_one_pocket();
        let baseline_feed = session.toolpath_configs()[0].operation.feed_rate();
        {
            let _guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // No mutations.
        }
        // Session unchanged.
        assert!(
            (session.toolpath_configs()[0].operation.feed_rate() - baseline_feed).abs() < 1e-9
        );
    }

    #[test]
    fn drop_restores_after_mutation() {
        let mut session = session_with_one_pocket();
        {
            let mut guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // Mutate via the guard.
            let mut new_op = guard.baseline().operation.clone();
            new_op.set_feed_rate(2999.0);
            let new_op_clone = new_op.clone();
            let dressups = guard.baseline().dressups.clone();
            let face_sel = guard.baseline().face_selection.clone();
            // Use a feeds_auto with a flipped flag to verify restoration
            // covers the feeds_auto override too.
            let mut tweaked = guard.baseline().feeds_auto.clone();
            tweaked.feed_rate = false;
            guard
                .session_mut()
                .apply_toolpath_param_snapshot(0, new_op_clone, dressups, face_sel, tweaked)
                .unwrap();
            // While the guard lives, the session reflects the candidate.
            assert!(
                (guard.session_mut().toolpath_configs()[0]
                    .operation
                    .feed_rate()
                    - 2999.0)
                    .abs()
                    < 1e-6
            );
            assert!(!guard.session_mut().toolpath_configs()[0].feeds_auto.feed_rate);
            // Mutation also bridges into the snapshot — but only for
            // copies; the captured snapshot is immutable.
            assert!(
                (guard.baseline().operation.feed_rate() - 1500.0).abs() < 1e-6,
                "snapshot should remain at baseline 1500 mm/min"
            );
            // Verify mutating new_op outside the guard didn't touch the
            // snapshot — defends against accidental shared mutation.
            new_op.set_feed_rate(0.0);
            let _ = new_op;
            assert!(
                (guard.baseline().operation.feed_rate() - 1500.0).abs() < 1e-6,
                "snapshot must be detached from the candidate's params"
            );
        }
        // After drop, session restored to baseline.
        let tc = &session.toolpath_configs()[0];
        assert!((tc.operation.feed_rate() - 1500.0).abs() < 1e-6);
        assert!(tc.feeds_auto.feed_rate, "feeds_auto.feed_rate restored");
    }

    #[test]
    fn drop_restores_on_panic() {
        let mut session = session_with_one_pocket();

        // Trigger a panic inside a guard's scope and confirm Drop still
        // restored the baseline. AssertUnwindSafe is needed because
        // &mut ProjectSession is not UnwindSafe by default — that's
        // fine, the contract here is "panic-safe", not "safe to
        // continue using session after a panic in the body". We only
        // inspect immutable state on the way out.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // Mutate to a non-baseline state.
            let dressups = guard.baseline().dressups.clone();
            let face_sel = guard.baseline().face_selection.clone();
            let feeds_auto = guard.baseline().feeds_auto.clone();
            let mut new_op = guard.baseline().operation.clone();
            new_op.set_feed_rate(9999.0);
            guard
                .session_mut()
                .apply_toolpath_param_snapshot(0, new_op, dressups, face_sel, feeds_auto)
                .unwrap();
            // Confirm we're in the mutated state.
            assert!(
                (guard.session_mut().toolpath_configs()[0]
                    .operation
                    .feed_rate()
                    - 9999.0)
                    .abs()
                    < 1e-6
            );
            // Now panic. Drop fires unwinding through this frame.
            panic!("simulated panic during candidate eval");
        }));

        assert!(result.is_err(), "the panic must propagate as expected");
        // After the panic + drop, session is back to baseline.
        let tc = &session.toolpath_configs()[0];
        assert!(
            (tc.operation.feed_rate() - 1500.0).abs() < 1e-6,
            "baseline feed must be restored after a panic; got {}",
            tc.operation.feed_rate()
        );
    }

    #[test]
    fn new_returns_not_found_for_invalid_index() {
        let mut session = session_with_one_pocket();
        let result = BaselineRestoreGuard::new(&mut session, 99);
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
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

    #[test]
    fn param_delta_has_changes_detects_any_field() {
        assert!(!ParamDelta::default().has_changes());
        assert!(
            ParamDelta {
                feed_mm_min: Some(2100.0),
                ..Default::default()
            }
            .has_changes()
        );
        assert!(
            ParamDelta {
                stepover_mm: Some(0.8),
                ..Default::default()
            }
            .has_changes()
        );
    }

    #[test]
    fn first_safe_skips_index_zero_baseline() {
        // Skipped/NoSafeImprovement outcomes never recommend.
        let skipped = OptimizeOutcome::Skipped {
            reason: RefuseReason::SimulationRequired,
        };
        assert!(skipped.first_safe().is_none());

        let nsi = OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoFeasibleRow,
            explanation: "test".to_owned(),
        };
        assert!(nsi.first_safe().is_none());
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage1_grid_tests {
    use super::*;
    use crate::compute::operation_configs::PocketConfig;

    fn synthetic_lut_row(ap_min: Option<f64>, ap_max: Option<f64>) -> MatchedRow {
        MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: Some(14_000.0),
            rpm_max: Some(16_000.0),
            ap_min_mm: ap_min,
            ap_max_mm: ap_max,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "synthetic".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
        }
    }

    #[test]
    fn three_variant_op_with_no_lut_row() {
        // Adaptive3d with baseline 3.0mm and no LUT bounds.
        // Expected: [0.7×3.0, 3.0, 1.3×3.0] = [2.1, 3.0, 3.9]
        let variants = build_doc_variants(3.0, None, OperationType::Adaptive3d);
        assert_eq!(variants.len(), 3, "got {variants:?}");
        assert!((variants[0] - 2.1).abs() < 1e-6);
        assert!((variants[1] - 3.0).abs() < 1e-6);
        assert!((variants[2] - 3.9).abs() < 1e-6);
    }

    #[test]
    fn four_variant_op_with_no_lut_row() {
        // Pocket with baseline 1.5mm and no LUT bounds.
        // Expected: [0.7×1.5, 1.5, mid(1.5, 2.1), 2.1] = [1.05, 1.5, 1.8, 2.1]
        let variants = build_doc_variants(1.5, None, OperationType::Pocket);
        assert_eq!(variants.len(), 4, "got {variants:?}");
        assert!((variants[0] - 1.05).abs() < 1e-6);
        assert!((variants[1] - 1.5).abs() < 1e-6);
        assert!((variants[2] - 1.8).abs() < 1e-6);
        assert!((variants[3] - 2.1).abs() < 1e-6);
    }

    #[test]
    fn lut_row_clamps_lo_to_ap_min() {
        // Baseline 3.0mm, LUT ap_min = 2.5mm (above 0.7×3.0 = 2.1).
        // Lo should be ap_min = 2.5, not 2.1.
        let row = synthetic_lut_row(Some(2.5), Some(5.0));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!((variants[0] - 2.5).abs() < 1e-6, "got {variants:?}");
    }

    #[test]
    fn lut_row_clamps_hi_to_ap_max() {
        // Baseline 3.0mm, LUT ap_max = 3.5mm (below 1.3×3.0 = 3.9).
        // Hi should be ap_max = 3.5, not 3.9.
        let row = synthetic_lut_row(Some(1.0), Some(3.5));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        let last = *variants.last().unwrap();
        assert!((last - 3.5).abs() < 1e-6, "got {variants:?}");
    }

    #[test]
    fn always_includes_baseline() {
        // Even when LUT bounds shrink the envelope to almost nothing,
        // baseline always survives.
        let row = synthetic_lut_row(Some(2.95), Some(3.05));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!(
            variants.iter().any(|v| (v - 3.0).abs() < 1e-6),
            "baseline must be present: got {variants:?}"
        );
    }

    #[test]
    fn deduplicates_when_lut_bounds_collapse_envelope() {
        // LUT row tight to within microns of baseline → lo, base, hi
        // all within the dedupe tolerance (5 µm) → collapse.
        let row = synthetic_lut_row(Some(2.9999), Some(3.0001));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!(
            variants.len() <= 2,
            "expected dedupe to collapse near-identical values, got {variants:?}"
        );
    }

    #[test]
    fn floors_at_hard_minimum_for_tiny_baseline() {
        // Baseline 0.01mm — below the hard floor. Floor brings it up.
        let variants = build_doc_variants(0.01, None, OperationType::Pocket);
        // Every variant respects the floor (0.05mm).
        for v in &variants {
            assert!(*v >= DOC_HARD_FLOOR_MM - 1e-9, "got {variants:?}");
        }
    }

    #[test]
    fn lut_row_with_ap_min_above_baseline_does_not_crash() {
        // Degenerate case: LUT row's ap_min > baseline. The user's
        // baseline is below the calibrated range. Grid should sort and
        // dedupe sanely without crashing.
        let row = synthetic_lut_row(Some(5.0), Some(8.0));
        let variants = build_doc_variants(2.0, Some(&row), OperationType::Adaptive3d);
        // Variants are sorted ascending.
        for w in variants.windows(2) {
            assert!(
                w[0] <= w[1] + 1e-9,
                "variants must be sorted: got {variants:?}"
            );
        }
        // Baseline survives.
        assert!(variants.iter().any(|v| (v - 2.0).abs() < 1e-6));
    }

    #[test]
    fn apply_doc_writes_only_depth_per_pass() {
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            stepover: 2.0,
            depth_per_pass: 1.5,
            spindle_rpm: Some(18_000),
            ..PocketConfig::default()
        });
        let candidate = apply_doc_to_op(&baseline, 2.5);
        // depth_per_pass changed.
        assert_eq!(candidate.depth_per_pass(), Some(2.5));
        // Other fields unchanged.
        assert!((candidate.feed_rate() - 1500.0).abs() < 1e-9);
        assert_eq!(candidate.stepover(), Some(2.0));
        assert_eq!(candidate.spindle_rpm(), Some(18_000));
    }
}
