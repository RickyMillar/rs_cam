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

use crate::compute::catalog::OperationConfig;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::{MachineProfile, PowerModel};
use crate::session::ProjectSession;
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
