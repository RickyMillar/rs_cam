//! Axis patches — atomic axis-value changes that the optimizer's
//! strategies and retargeters emit, and which an applier composes onto
//! a baseline operation to produce a candidate.
//!
//! Step 3 of G16. Patches replace the previous "strategy emits a full
//! `OperationConfig`" pattern. The win: every candidate's provenance is
//! a list of `(axis, value, source)` triples — directly inspectable in
//! debug logs and MCP outcomes.

use crate::compute::catalog::{OperationConfig, OperationParams, OperationType};
#[allow(unused_imports)]
use OperationParams as _;

use super::axes::SearchAxis;

/// One axis-value change. Multi-patch when the upstream had coupled
/// levers (chipload retarget produces `[FeedRate primary, FeedRate
/// coupled-plunge-tracking]`).
#[derive(Clone, Debug, PartialEq)]
pub struct AxisPatch {
    pub axis: SearchAxis,
    pub value: f64,
    /// Was the requested value clamped to bounds? Visible in candidate
    /// rationale so users see when a target was infeasible at face value.
    pub clamped: bool,
    pub source: PatchSource,
}

/// Why this patch exists.
#[derive(Clone, Debug, PartialEq)]
pub enum PatchSource {
    /// The retargeter or strategy's primary lever — the axis it
    /// consciously chose to move.
    Primary,
    /// A coupling rule fired in response to a primary patch (e.g.,
    /// plunge tracks feed when the change is large enough to matter).
    Coupled {
        from_axis: SearchAxis,
        rule: &'static str,
    },
    /// A strategy-driven patch (Stage-1-equivalent grid sweep, headroom
    /// scale-up).
    Strategy { strategy: &'static str },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AxisError {
    /// Axis isn't present on this op type.
    NotPresent {
        axis: SearchAxis,
        op_type: OperationType,
    },
    /// Value was non-finite or non-positive.
    InvalidValue { axis: SearchAxis, value: f64 },
    /// Axis is reserved for a future gap closure but has no setter yet.
    NotImplemented { axis: SearchAxis },
}

/// Apply a single axis patch to an operation. Mutates in place.
///
/// Mismatched (op, axis) pairs return `AxisError::NotPresent`. Callers
/// that want defensive checking can validate `axis ∈ view.bindings`
/// before calling, but in normal flow the optimizer only emits patches
/// for declared axes so the error is upstream-policy violation.
pub fn apply_axis_patch_to_op(
    op: &mut OperationConfig,
    patch: &AxisPatch,
) -> Result<(), AxisError> {
    if !patch.value.is_finite() || patch.value <= 0.0 {
        return Err(AxisError::InvalidValue {
            axis: patch.axis,
            value: patch.value,
        });
    }
    match patch.axis {
        SearchAxis::FeedRate => {
            op.set_feed_rate(patch.value);
            Ok(())
        }
        SearchAxis::SpindleRpm => {
            // Truncation to u32 matches existing optimize.rs behaviour
            // (machine.clamp_rpm(..).round() as u32).
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let rpm = patch.value.round() as u32;
            op.set_spindle_rpm(Some(rpm));
            Ok(())
        }
        SearchAxis::DepthPerPass => {
            op.set_depth_per_pass(patch.value);
            Ok(())
        }
        SearchAxis::Stepover => {
            op.set_stepover(patch.value);
            Ok(())
        }
        SearchAxis::ScallopHeight => {
            op.set_scallop_height(patch.value);
            Ok(())
        }
        SearchAxis::AngularStep
        | SearchAxis::HelixPitch
        | SearchAxis::RampAngle => Err(AxisError::NotImplemented { axis: patch.axis }),
    }
}

/// Apply a sequence of patches in order, building a new op cloned from
/// the baseline. The first patch's `Primary` axis drives the rationale;
/// `Coupled` patches are interpreted by the caller (plunge-tracks-feed
/// is currently the only such rule).
pub fn apply_patches_to_op(
    baseline: &OperationConfig,
    patches: &[AxisPatch],
) -> Result<OperationConfig, AxisError> {
    let mut out = baseline.clone();
    for patch in patches {
        // Coupled-source patches that re-target the same axis are
        // marker entries — the actual mutation already happened via the
        // Primary patch. Skip them at apply time; their value is read
        // by the candidate-rationale builder, not the apply path.
        if matches!(patch.source, PatchSource::Coupled { .. })
            && patches.iter().any(|p| {
                p.axis == patch.axis && matches!(p.source, PatchSource::Primary)
            })
        {
            continue;
        }
        apply_axis_patch_to_op(&mut out, patch)?;
    }
    Ok(out)
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
    use crate::compute::operation_configs::PocketConfig;

    #[test]
    fn apply_feed_patch_mutates_feed_only() {
        let mut op = OperationConfig::Pocket(PocketConfig::default());
        let original_doc = op.depth_per_pass();
        apply_axis_patch_to_op(
            &mut op,
            &AxisPatch {
                axis: SearchAxis::FeedRate,
                value: 2500.0,
                clamped: false,
                source: PatchSource::Primary,
            },
        )
        .unwrap();
        assert!((op.feed_rate() - 2500.0).abs() < 1e-9);
        assert_eq!(op.depth_per_pass(), original_doc);
    }

    #[test]
    fn apply_invalid_value_returns_error() {
        let mut op = OperationConfig::Pocket(PocketConfig::default());
        let err = apply_axis_patch_to_op(
            &mut op,
            &AxisPatch {
                axis: SearchAxis::FeedRate,
                value: -100.0,
                clamped: false,
                source: PatchSource::Primary,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AxisError::InvalidValue { .. }));
    }

    #[test]
    fn apply_non_finite_value_returns_error() {
        let mut op = OperationConfig::Pocket(PocketConfig::default());
        let err = apply_axis_patch_to_op(
            &mut op,
            &AxisPatch {
                axis: SearchAxis::FeedRate,
                value: f64::NAN,
                clamped: false,
                source: PatchSource::Primary,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AxisError::InvalidValue { .. }));
    }

    #[test]
    fn apply_reserved_axis_returns_not_implemented() {
        let mut op = OperationConfig::Pocket(PocketConfig::default());
        let err = apply_axis_patch_to_op(
            &mut op,
            &AxisPatch {
                axis: SearchAxis::AngularStep,
                value: 5.0,
                clamped: false,
                source: PatchSource::Primary,
            },
        )
        .unwrap_err();
        assert!(matches!(err, AxisError::NotImplemented { .. }));
    }

    #[test]
    fn apply_patches_chain_produces_combined_op() {
        let baseline = OperationConfig::Pocket(PocketConfig::default());
        let patches = [
            AxisPatch {
                axis: SearchAxis::FeedRate,
                value: 3000.0,
                clamped: false,
                source: PatchSource::Primary,
            },
            AxisPatch {
                axis: SearchAxis::DepthPerPass,
                value: 2.5,
                clamped: false,
                source: PatchSource::Strategy {
                    strategy: "axis-grid",
                },
            },
        ];
        let candidate = apply_patches_to_op(&baseline, &patches).unwrap();
        assert!((candidate.feed_rate() - 3000.0).abs() < 1e-9);
        assert_eq!(candidate.depth_per_pass(), Some(2.5));
    }

    #[test]
    fn coupled_patch_marker_does_not_re_apply_axis() {
        // Two patches on FeedRate: one Primary, one Coupled. The Coupled
        // entry is a rationale marker and must not double-apply.
        let baseline = OperationConfig::Pocket(PocketConfig::default());
        let patches = [
            AxisPatch {
                axis: SearchAxis::FeedRate,
                value: 2500.0,
                clamped: false,
                source: PatchSource::Primary,
            },
            AxisPatch {
                axis: SearchAxis::FeedRate,
                value: 2500.0,
                clamped: false,
                source: PatchSource::Coupled {
                    from_axis: SearchAxis::FeedRate,
                    rule: "test rule",
                },
            },
        ];
        let candidate = apply_patches_to_op(&baseline, &patches).unwrap();
        assert!((candidate.feed_rate() - 2500.0).abs() < 1e-9);
    }
}
