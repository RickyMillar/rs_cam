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
//! Both keep the ladder valid with [`normalize_ladder`]: coarsest first,
//! and each coarse step larger than `depth_per_pass`. The adapter check
//! `check_adaptive3d_step_ladder` (compute/execute/finish_3d.rs) states the
//! same rule; it is not reachable from this module.

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::Adaptive3dConfig;

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

/// Remove the coarse steps that break the ladder rule, and keep the rest in
/// their order. A coarse step that is not larger than `depth_per_pass` goes,
/// and so does a second copy of one step. Returns `true` when a step went.
///
/// The function does not sort. The writes in this module keep the order: a
/// cap and a positive scale do not change the order of two values.
pub(super) fn normalize_ladder(operation: &mut OperationConfig) -> bool {
    match operation {
        OperationConfig::Adaptive3d(cfg) => normalize_ladder_of(cfg),
        _ => false,
    }
}

/// [`normalize_ladder`] on the 3D Rough config itself.
pub(super) fn normalize_ladder_of(cfg: &mut Adaptive3dConfig) -> bool {
    let before = cfg.coarse_steps.len();
    let base = cfg.depth_per_pass;
    cfg.coarse_steps.retain(|s| *s > base + SAME_STEP_EPS_MM);
    cfg.coarse_steps
        .dedup_by(|a, b| (*a - *b).abs() <= SAME_STEP_EPS_MM);
    cfg.coarse_steps.len() != before
}

/// Move each coarse step above `cap_mm` down to `cap_mm`, then keep the
/// ladder valid. Returns the `(from, to)` pair of each coarse step that
/// moved, coarsest first. The base step does not move here; the caller
/// caps it on its own path, before this call.
pub(super) fn cap_coarse_steps(operation: &mut OperationConfig, cap_mm: f64) -> Vec<(f64, f64)> {
    match operation {
        OperationConfig::Adaptive3d(cfg) => cap_coarse_steps_of(cfg, cap_mm),
        _ => Vec::new(),
    }
}

/// [`cap_coarse_steps`] on the 3D Rough config itself.
pub(super) fn cap_coarse_steps_of(cfg: &mut Adaptive3dConfig, cap_mm: f64) -> Vec<(f64, f64)> {
    let mut moved = Vec::new();
    for step in &mut cfg.coarse_steps {
        if *step > cap_mm {
            moved.push((*step, cap_mm));
            *step = cap_mm;
        }
    }
    normalize_ladder_of(cfg);
    moved
}

/// Put every axial step at or below `cap_mm`: the base step and each coarse
/// step. With no ladder this is `set_depth_per_pass(cap_mm)` when the base
/// step is above the cap, and nothing when it is not.
pub(super) fn cap_axial_steps(operation: &mut OperationConfig, cap_mm: f64) {
    if operation.depth_per_pass().is_some_and(|d| d > cap_mm) {
        operation.set_depth_per_pass(cap_mm);
    }
    cap_coarse_steps(operation, cap_mm);
}

/// Scale the whole ladder so that its deepest step is `deepest_mm`. Every
/// step moves by the same factor `deepest_mm / deepest_axial_step()`. With
/// no ladder this is `set_depth_per_pass(deepest_mm)`.
///
/// The deepest coarse step gets `deepest_mm` exactly. `round` maps each
/// other scaled coarse step to the value that ships (the dial rounds DOWN
/// to 0.001 mm). The base step is the caller's to write, with its own snap;
/// pass it as `base_mm`.
pub(super) fn set_deepest_axial_step(
    operation: &mut OperationConfig,
    deepest_mm: f64,
    base_mm: f64,
    round: &dyn Fn(f64) -> f64,
) {
    let Some(deepest_now) = operation.deepest_axial_step() else {
        return;
    };
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
    normalize_ladder(operation);
}
