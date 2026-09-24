//! The 3D Rough step ladder as Suggest sees it (trap D7 of
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md`).
//!
//! `Adaptive3dConfig::coarse_steps` lists coarser axial steps above the base
//! step `depth_per_pass`, coarsest first. The deepest bite of the operation
//! is `OperationConfig::deepest_axial_step`. A Suggest pass that reads the
//! deepest bite and then writes a new depth must write it through this
//! module, because a write to `depth_per_pass` alone does not move a coarse
//! step, and the reader would then see no change.
//!
//! Two write shapes:
//!
//! - [`cap_axial_steps`] is for a depth LIMIT (the envelope, the rigidity
//!   cap, the flute length, the deflection back-off). Each step above the
//!   cap moves down to the cap; a step at or below the cap does not move.
//! - [`set_deepest_axial_step`] is for a common SCALE (the aggressiveness
//!   dial). Every step moves by the same factor.
//!
//! Both keep the ladder valid: coarsest first, and each coarse step larger
//! than `depth_per_pass`. The adapter check `check_adaptive3d_step_ladder`
//! (compute/execute/finish_3d.rs) states the same rule; it is not reachable
//! from this module.
//!
//! A write only lowers a step ("keep my steps, only cap", operator ruling 1
//! of 2026-09-24). When a lowered step is no longer above the next step, it
//! goes. Each write returns the steps that went, as [`RemovedStep`], and the
//! caller files a [`SuggestWarning::CoarseStepRemoved`] for each one. A
//! ladder never goes empty with no note.

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::Adaptive3dConfig;

use super::SuggestWarning;

/// Two steps closer than this are one step. The dial writes to 0.001 mm,
/// so two different written steps are always further apart than this.
const SAME_STEP_EPS_MM: f64 = 1e-9;

/// The coarse steps of the ladder, coarsest first. Empty for an operation
/// with no ladder and for every operation that is not a 3D Rough.
pub(super) fn coarse_steps(operation: &OperationConfig) -> &[f64] {
    match operation {
        OperationConfig::Adaptive3d(cfg) => &cfg.coarse_steps,
        _ => &[],
    }
}

/// `true` when the operation carries a non-empty step ladder.
pub(super) fn has_ladder(operation: &OperationConfig) -> bool {
    !coarse_steps(operation).is_empty()
}

/// A copy of `operation` that cuts at the one axial step `depth_mm`: the
/// base step is `depth_mm` and the ladder is empty. A probe that asks "what
/// is the load at this depth" evaluates this copy, so a coarse step of the
/// original cannot hide the depth under test.
pub(super) fn at_single_step(operation: &OperationConfig, depth_mm: f64) -> OperationConfig {
    let mut probe = operation.clone();
    if let OperationConfig::Adaptive3d(cfg) = &mut probe {
        cfg.coarse_steps.clear();
    }
    probe.set_depth_per_pass(depth_mm);
    probe
}

/// One coarse step that a write removed from the ladder. The step moved to
/// a value that is not above the next step, so it was no longer a step of
/// its own. The caller turns it into a [`SuggestWarning::CoarseStepRemoved`]
/// with [`removal_notes`], so no removal is silent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RemovedStep {
    /// The index of the step in the ladder before the write.
    pub(super) index: usize,
    /// The coarse step before the write.
    pub(super) step_mm: f64,
    /// The value that the write gave the step.
    pub(super) lowered_to_mm: f64,
    /// The step that it is no longer above.
    pub(super) next_step_mm: f64,
    /// `true` when the next step is the base step `depth_per_pass`.
    pub(super) next_is_base: bool,
}

/// The removal notes for `removed`, one for each step. `cap` names the
/// limit that lowered the steps, in the words of the card.
pub(super) fn removal_notes(removed: &[RemovedStep], cap: &'static str) -> Vec<SuggestWarning> {
    removed
        .iter()
        .map(|r| SuggestWarning::CoarseStepRemoved {
            step_mm: r.step_mm,
            lowered_to_mm: r.lowered_to_mm,
            next_step_mm: r.next_step_mm,
            next_is_base: r.next_is_base,
            cap,
        })
        .collect()
}

/// Remove the coarse steps that break the ladder rule, and keep the rest in
/// their order. A coarse step that is not larger than `depth_per_pass` goes,
/// and so does a second copy of one step. Returns each step that went.
///
/// `before` is the ladder before the write that the caller made, index for
/// index. The writes in this module change the values in place and do not
/// change the order (a cap and a positive scale keep the order of two
/// values), so index `i` of `before` is the same step as index `i` now.
/// Without a write, pass the ladder itself.
fn prune_ladder_of(cfg: &mut Adaptive3dConfig, before: &[f64]) -> Vec<RemovedStep> {
    let base = cfg.depth_per_pass;
    let mut kept: Vec<f64> = Vec::with_capacity(cfg.coarse_steps.len());
    let mut removed = Vec::new();
    for (i, &step) in cfg.coarse_steps.iter().enumerate() {
        let was = before.get(i).copied().unwrap_or(step);
        if step <= base + SAME_STEP_EPS_MM {
            removed.push(RemovedStep {
                index: i,
                step_mm: was,
                lowered_to_mm: step,
                next_step_mm: base,
                next_is_base: true,
            });
            continue;
        }
        if let Some(&last) = kept.last()
            && (last - step).abs() <= SAME_STEP_EPS_MM
        {
            removed.push(RemovedStep {
                index: i,
                step_mm: was,
                lowered_to_mm: step,
                next_step_mm: last,
                next_is_base: false,
            });
            continue;
        }
        kept.push(step);
    }
    cfg.coarse_steps = kept;
    removed
}

/// Remove the coarse steps that break the ladder rule when no write moved
/// them (see [`prune_ladder_of`]). Returns each step that went; for each,
/// `lowered_to_mm` equals `step_mm`.
pub(super) fn normalize_ladder_of(cfg: &mut Adaptive3dConfig) -> Vec<RemovedStep> {
    let before = cfg.coarse_steps.clone();
    prune_ladder_of(cfg, &before)
}

/// What a cap did to the coarse steps.
#[derive(Debug, Default)]
pub(super) struct CoarseCap {
    /// The `(from, to)` pair of each coarse step that moved, coarsest first.
    pub(super) moved: Vec<(f64, f64)>,
    /// Each coarse step that the cap removed from the ladder.
    pub(super) removed: Vec<RemovedStep>,
}

/// Move each coarse step above `cap_mm` down to `cap_mm`, then keep the
/// ladder valid. The base step does not move here; the caller caps it on
/// its own path, before this call.
pub(super) fn cap_coarse_steps(operation: &mut OperationConfig, cap_mm: f64) -> CoarseCap {
    match operation {
        OperationConfig::Adaptive3d(cfg) => cap_coarse_steps_of(cfg, cap_mm),
        _ => CoarseCap::default(),
    }
}

/// [`cap_coarse_steps`] on the 3D Rough config itself.
pub(super) fn cap_coarse_steps_of(cfg: &mut Adaptive3dConfig, cap_mm: f64) -> CoarseCap {
    let before = cfg.coarse_steps.clone();
    let mut moved = Vec::new();
    for step in &mut cfg.coarse_steps {
        if *step > cap_mm {
            moved.push((*step, cap_mm));
            *step = cap_mm;
        }
    }
    let removed = prune_ladder_of(cfg, &before);
    CoarseCap { moved, removed }
}

/// Put every axial step at or below `cap_mm`: the base step and each coarse
/// step. With no ladder this is `set_depth_per_pass(cap_mm)` when the base
/// step is above the cap, and nothing when it is not. Returns each coarse
/// step that the cap removed; the caller gives each one a note with
/// [`removal_notes`].
pub(super) fn cap_axial_steps(operation: &mut OperationConfig, cap_mm: f64) -> Vec<RemovedStep> {
    if operation.depth_per_pass().is_some_and(|d| d > cap_mm) {
        operation.set_depth_per_pass(cap_mm);
    }
    cap_coarse_steps(operation, cap_mm).removed
}

/// Scale the whole ladder so that its deepest step is `deepest_mm`. Every
/// step moves by the same factor `deepest_mm / deepest_axial_step()`. With
/// no ladder this is `set_depth_per_pass(deepest_mm)`.
///
/// The deepest coarse step gets `deepest_mm` exactly. `round` maps each
/// other scaled coarse step to the value that ships (the dial rounds DOWN
/// to 0.001 mm). The base step is the caller's to write, with its own snap;
/// pass it as `base_mm`. Returns each coarse step that the scale removed
/// (a rounding can put two steps on one value).
pub(super) fn set_deepest_axial_step(
    operation: &mut OperationConfig,
    deepest_mm: f64,
    base_mm: f64,
    round: &dyn Fn(f64) -> f64,
) -> Vec<RemovedStep> {
    let Some(deepest_now) = operation.deepest_axial_step() else {
        return Vec::new();
    };
    let before = coarse_steps(operation).to_vec();
    if let OperationConfig::Adaptive3d(cfg) = operation
        && !cfg.coarse_steps.is_empty()
        && deepest_now.is_finite()
        && deepest_now > 0.0
    {
        let factor = deepest_mm / deepest_now;
        for step in &mut cfg.coarse_steps {
            *step = if (*step - deepest_now).abs() <= SAME_STEP_EPS_MM {
                deepest_mm
            } else {
                round(*step * factor)
            };
        }
    }
    operation.set_depth_per_pass(base_mm);
    match operation {
        OperationConfig::Adaptive3d(cfg) => prune_ladder_of(cfg, &before),
        _ => Vec::new(),
    }
}
