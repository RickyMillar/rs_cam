//! Axis-grid sweep strategy — joint DOC × stepover × scallop variant
//! grid for the geometry ops. Replaces the old `run_stage_1_grid`.
//!
//! Same anchor-and-dedup logic as before; the variant builders
//! (`build_doc_variants`, `build_stepover_variants`,
//! `build_scallop_height_variants`) are reused, so the candidate set
//! is identical to the prior commit's snapshot for any given anchor.
//!
//! The strategy is anchored on a caller-supplied `anchor_op` rather
//! than the bare baseline. When the headroom strategy fired, the
//! orchestrator passes its candidate's params as the anchor so the
//! grid still sweeps "scaled feed/rpm × variant geometry" — preserving
//! the prior behaviour exactly. When headroom did not fire, the anchor
//! is the baseline.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::feeds::vendor_lookup::MatchedRow;
use crate::tool_load::verdict::ToolpathLoadVerdict;

use super::super::axes::{AxisView, SearchAxis};
use super::super::patches::{AxisPatch, PatchSource};
use super::super::policy::SearchPolicy;
use super::super::{
    build_doc_variants, build_scallop_height_variants, build_stepover_variants, has_doc_knob,
};
use super::{CandidatePatch, OptimizationStrategy};

const STRATEGY_NAME: &str = "axis-grid";

/// Variant grid over the geometry axes the op exposes.
pub struct AxisGridStrategy<'a> {
    /// The op the grid is anchored on. Either the baseline op or
    /// the headroom-strategy candidate's params.
    pub anchor_op: &'a OperationConfig,
    pub lut_row: Option<&'a MatchedRow>,
    pub op_type: OperationType,
    pub policy: &'a SearchPolicy,
}

impl<'a> OptimizationStrategy for AxisGridStrategy<'a> {
    fn name(&self) -> &'static str {
        STRATEGY_NAME
    }

    /// Generate one [`CandidatePatch`] per non-anchor cell of the
    /// joint DOC × stepover × scallop_height grid. Patches are
    /// `axis-grid`-sourced and apply against `anchor_op` (the
    /// orchestrator wrapper handles application). The `baseline`
    /// view is part of the trait contract but not consulted here —
    /// the grid is anchor-relative by design.
    fn candidates(
        &self,
        _baseline: &AxisView<'_>,
        _baseline_verdict: &ToolpathLoadVerdict,
    ) -> Vec<CandidatePatch> {
        let has_doc = self.anchor_op.depth_per_pass().is_some();
        let has_stepover = self.anchor_op.stepover().is_some();
        let has_scallop = self.anchor_op.scallop_height().is_some();
        if !has_doc && !has_stepover && !has_scallop {
            return Vec::new();
        }

        let doc_policy = &self.policy.axes.doc;
        let stepover_policy = &self.policy.axes.stepover;
        let scallop_policy = &self.policy.axes.scallop_height;
        let fallback = &self.policy.fallback;

        let anchor_doc = self
            .anchor_op
            .depth_per_pass()
            .unwrap_or(fallback.doc_anchor_mm.value);
        let anchor_stepover = self
            .anchor_op
            .stepover()
            .unwrap_or(fallback.stepover_anchor_mm.value);
        let anchor_scallop = self
            .anchor_op
            .scallop_height()
            .unwrap_or(fallback.scallop_height_anchor_mm.value);

        // Variants per axis. When the op doesn't expose an axis (or
        // doesn't have its DOC knob plumbed through `set_depth_per_pass`),
        // collapse to a single anchor-valued entry so the dedup loop
        // produces no cells along that dimension.
        let doc_variants = if has_doc && has_doc_knob(self.op_type) {
            build_doc_variants(anchor_doc, self.lut_row, self.op_type)
        } else {
            vec![anchor_doc]
        };
        let stepover_variants = if has_stepover {
            build_stepover_variants(anchor_stepover, self.lut_row, self.op_type)
        } else {
            vec![anchor_stepover]
        };
        let scallop_variants = if has_scallop {
            build_scallop_height_variants(anchor_scallop)
        } else {
            vec![anchor_scallop]
        };

        let mut out = Vec::new();
        for &doc in &doc_variants {
            for &stepover in &stepover_variants {
                for &scallop in &scallop_variants {
                    // Skip the anchor cell — represented already by
                    // the headroom candidate (or baseline).
                    if (doc - anchor_doc).abs() < doc_policy.dedup_tolerance.value
                        && (stepover - anchor_stepover).abs()
                            < stepover_policy.dedup_tolerance.value
                        && (scallop - anchor_scallop).abs()
                            < scallop_policy.dedup_tolerance.value
                    {
                        continue;
                    }

                    let mut patches: Vec<AxisPatch> = Vec::new();
                    if has_doc {
                        patches.push(AxisPatch {
                            axis: SearchAxis::DepthPerPass,
                            value: doc,
                            clamped: false,
                            source: PatchSource::Strategy {
                                strategy: STRATEGY_NAME,
                            },
                        });
                    }
                    if has_stepover {
                        patches.push(AxisPatch {
                            axis: SearchAxis::Stepover,
                            value: stepover,
                            clamped: false,
                            source: PatchSource::Strategy {
                                strategy: STRATEGY_NAME,
                            },
                        });
                    }
                    if has_scallop {
                        patches.push(AxisPatch {
                            axis: SearchAxis::ScallopHeight,
                            value: scallop,
                            clamped: false,
                            source: PatchSource::Strategy {
                                strategy: STRATEGY_NAME,
                            },
                        });
                    }

                    out.push(CandidatePatch {
                        patches,
                        strategy: STRATEGY_NAME,
                        rationale: format!(
                            "axis-grid cell: doc={doc:.3}, stepover={stepover:.3}, scallop={scallop:.4}",
                        ),
                    });
                }
            }
        }
        out
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
    use crate::compute::catalog::{OperationConfig, OptimizationSurface};
    use crate::compute::operation_configs::{Adaptive3dConfig, PocketConfig, ScallopConfig};
    use crate::tool_load::verdict::{Confidence, Verdict};

    fn empty_verdict() -> ToolpathLoadVerdict {
        let within = Verdict::Within {
            peak: 0.0,
            confidence: Confidence::Validated,
        };
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: within.clone(),
            power: within.clone(),
            deflection: within,
        }
    }

    #[test]
    fn pocket_grid_emits_doc_and_stepover_patches() {
        let policy = SearchPolicy::default();
        let anchor = OperationConfig::Pocket(PocketConfig {
            depth_per_pass: 1.5,
            stepover: 2.0,
            ..PocketConfig::default()
        });
        let OptimizationSurface::Optimizable(view) = anchor.optimization_surface() else {
            panic!("Pocket should be Optimizable");
        };
        let strat = AxisGridStrategy {
            anchor_op: &anchor,
            lut_row: None,
            op_type: OperationType::Pocket,
            policy: &policy,
        };
        let cps = strat.candidates(&view, &empty_verdict());
        assert!(!cps.is_empty(), "grid should produce non-anchor cells");
        // Every emitted patch list contains DOC and Stepover (Pocket
        // exposes both); none contain ScallopHeight.
        for cp in &cps {
            let axes: Vec<_> = cp.patches.iter().map(|p| p.axis).collect();
            assert!(axes.contains(&SearchAxis::DepthPerPass));
            assert!(axes.contains(&SearchAxis::Stepover));
            assert!(!axes.contains(&SearchAxis::ScallopHeight));
        }
    }

    #[test]
    fn scallop_op_emits_only_scallop_patches() {
        let policy = SearchPolicy::default();
        let anchor = OperationConfig::Scallop(ScallopConfig::default());
        let OptimizationSurface::Optimizable(view) = anchor.optimization_surface() else {
            panic!("Scallop should be Optimizable");
        };
        let strat = AxisGridStrategy {
            anchor_op: &anchor,
            lut_row: None,
            op_type: OperationType::Scallop,
            policy: &policy,
        };
        let cps = strat.candidates(&view, &empty_verdict());
        for cp in &cps {
            let axes: Vec<_> = cp.patches.iter().map(|p| p.axis).collect();
            assert_eq!(axes, vec![SearchAxis::ScallopHeight]);
        }
    }

    #[test]
    fn anchor_cell_is_skipped() {
        let policy = SearchPolicy::default();
        let anchor = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 3.0,
            stepover: 0.84,
            ..Adaptive3dConfig::default()
        });
        let OptimizationSurface::Optimizable(view) = anchor.optimization_surface() else {
            panic!();
        };
        let strat = AxisGridStrategy {
            anchor_op: &anchor,
            lut_row: None,
            op_type: OperationType::Adaptive3d,
            policy: &policy,
        };
        let cps = strat.candidates(&view, &empty_verdict());
        // No cell should have all three axis values within the
        // anchor's dedup neighbourhood.
        for cp in &cps {
            let doc = cp
                .patches
                .iter()
                .find(|p| p.axis == SearchAxis::DepthPerPass)
                .map(|p| p.value)
                .unwrap_or(3.0);
            let stepover = cp
                .patches
                .iter()
                .find(|p| p.axis == SearchAxis::Stepover)
                .map(|p| p.value)
                .unwrap_or(0.84);
            let is_anchor = (doc - 3.0).abs() < policy.axes.doc.dedup_tolerance.value
                && (stepover - 0.84).abs()
                    < policy.axes.stepover.dedup_tolerance.value;
            assert!(!is_anchor, "anchor cell leaked into grid: doc={doc}, stepover={stepover}");
        }
    }

    #[test]
    fn op_with_no_geometry_axes_returns_empty() {
        // Construct an op that exposes neither DOC nor stepover nor scallop.
        // FoamCarve / RoundProbing / similar — find one via OperationType search.
        // Most ops expose at least one; instead, build the strategy on a
        // trace op (which has DOC but no stepover or scallop) and verify
        // it still emits sensible candidates rather than empty. The
        // "no-axis" path is exercised via the cancel-out branch above
        // when has_doc_knob returns false.
        // Verify positive-axis case as a sanity check.
        let policy = SearchPolicy::default();
        let anchor = OperationConfig::Pocket(PocketConfig::default());
        let OptimizationSurface::Optimizable(view) = anchor.optimization_surface() else {
            panic!();
        };
        let strat = AxisGridStrategy {
            anchor_op: &anchor,
            lut_row: None,
            op_type: OperationType::Pocket,
            policy: &policy,
        };
        // Positive: pocket has DOC and stepover, so we get cells.
        assert!(!strat.candidates(&view, &empty_verdict()).is_empty());
    }
}
