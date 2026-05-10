//! Deflection retargeter — Step 5, G16.
//!
//! Per `planning/STEP5_PREP_RETARGETERS.md` §3. Drives `DepthPerPass`
//! down (or up) when the deflection gate reports tip-deflection over its
//! safe envelope. The cube-root scaling factor is the simple
//! approximation: at fixed stepover, lateral force scales linearly with
//! DOC, deflection of a cantilever scales linearly with force × L³ — so
//! over a single-axis sweep we model `δ ∝ DOC` to first order, but the
//! deflection module integrates the stepped cantilever properly and the
//! actual relationship is closer to `δ ∝ DOC^k` with k between 1 and 3
//! depending on which moment dominates. We pick the cube-root midpoint
//! `k = 1/3` as a stable seeding factor — it overshoots correction
//! relative to a strict linear model so the candidate is *more*
//! conservative than necessary, and the sim then verifies. Per the prep
//! doc this is intentionally a candidate-seeding step, not an exact
//! force model.

use crate::tool_load::optimize::axes::{AxisContext, AxisView, SearchAxis};
use crate::tool_load::optimize::patches::{AxisPatch, PatchSource};
use crate::tool_load::optimize::space::SearchSpace;
use crate::tool_load::verdict::DeflectionVerdict;

use super::{Retargeter, RetargetSolution};

/// Drives DOC in response to a deflection-exceeded verdict.
///
/// `threshold_mm` is the per-policy `EXCEEDS_BOUND_MM` for tip
/// deflection (200 µm in `tool_load::deflection`). Constructor-injected
/// rather than read off `SearchSpace` because the threshold is a
/// criterion-internal constant — `SearchSpace` deliberately doesn't know
/// about per-gate bounds, only per-axis bounds.
///
/// `headroom` < 1.0 — the retargeter aims for `threshold × headroom`
/// rather than the bare threshold, leaving margin for sim variance.
#[derive(Debug, Clone, Copy)]
pub struct DeflectionDocRetargeter {
    pub threshold_mm: f64,
    pub headroom: f64,
}

impl DeflectionDocRetargeter {
    /// Construct with default headroom from
    /// `policy.retarget.deflection_headroom` (0.75).
    pub fn new(threshold_mm: f64) -> Self {
        Self {
            threshold_mm,
            headroom: 0.75,
        }
    }

    /// Construct with explicit headroom.
    pub fn with_headroom(threshold_mm: f64, headroom: f64) -> Self {
        Self {
            threshold_mm,
            headroom,
        }
    }
}

const DRIVING_AXES: &[SearchAxis] = &[SearchAxis::DepthPerPass];

impl Retargeter for DeflectionDocRetargeter {
    type Verdict = DeflectionVerdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        DRIVING_AXES
    }

    fn target(
        &self,
        verdict: &DeflectionVerdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        // Only deflection-exceeded verdicts retarget. The typed
        // `DeflectionVerdict::Exceeds` carries the peak directly; we
        // prefer the verdict's bounds (when present) over the
        // constructor-injected threshold so the actual evaluator's
        // numbers drive the math.
        let peak_mm = match verdict {
            DeflectionVerdict::Exceeds { peak_mm, .. } => *peak_mm,
            _ => return None,
        };

        // Op must expose DOC for retargeting to make sense.
        let baseline_doc = view.axis_value(SearchAxis::DepthPerPass, ctx)?;

        // Peak must be a usable positive number — otherwise the
        // multiplier math diverges or goes complex.
        if !peak_mm.is_finite() || peak_mm <= 0.0 {
            return None;
        }
        if !self.threshold_mm.is_finite() || self.threshold_mm <= 0.0 {
            return None;
        }

        let target_deflection = self.threshold_mm * self.headroom;
        // Cube-root scaling — see module docs. cbrt is f64-defined for
        // any finite positive ratio; defensive guard against pathological
        // inputs above already.
        let multiplier = (target_deflection / peak_mm).cbrt();
        let raw_target_doc = baseline_doc * multiplier;

        let doc_bounds = space.axis(SearchAxis::DepthPerPass)?;
        let clamped_value = doc_bounds.hard.clamp(raw_target_doc);
        let was_clamped = (clamped_value - raw_target_doc).abs() > 1e-6;

        let rationale = format!(
            "scale DOC by {:.3}x (cube-root) to bring tip deflection from {:.4} mm \
             toward target {:.4} mm (threshold × headroom)",
            multiplier, peak_mm, target_deflection,
        );

        Some(RetargetSolution {
            patches: vec![AxisPatch {
                axis: SearchAxis::DepthPerPass,
                value: clamped_value,
                clamped: was_clamped,
                source: PatchSource::Primary,
            }],
            rationale,
        })
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
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::tool::ToolDefinition;
    use crate::tool_load::optimize::policy::SearchPolicy;
    use crate::tool_load::verdict::Confidence;

    /// Build a SearchSpace seeded by a `PocketConfig` with a custom DOC
    /// baseline. Mirrors the helper pattern in `space.rs` tests.
    fn build_space(baseline_doc_mm: f64) -> (OperationConfig, SearchSpace, ToolDefinition) {
        let pocket = PocketConfig {
            depth_per_pass: baseline_doc_mm,
            ..PocketConfig::default()
        };
        let op = OperationConfig::Pocket(pocket);
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!("Pocket should be Optimizable");
        };

        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let tool_config = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        let tool = ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(
                tool_config.diameter,
                tool_config.cutting_length,
            )),
            tool_config.shank_diameter,
            tool_config.shank_length,
            tool_config.holder_diameter,
            tool_config.stickout,
            tool_config.flute_count,
            tool_config.tool_material,
        );
        let ctx = AxisContext {
            project_default_rpm: 18_000,
            machine: &machine,
            tool: &tool,
            material: &material,
        };
        let policy = SearchPolicy::default();
        let space = SearchSpace::build(&view, &ctx, None, &policy);
        (op, space, tool)
    }

    /// Re-create an `AxisView<'op>` borrowing `op` for use after build_space.
    /// (The view inside `build_space` borrows a local that goes out of scope.)
    fn view_of(op: &OperationConfig) -> AxisView<'_> {
        match op.optimization_surface() {
            OptimizationSurface::Optimizable(v) => v,
            OptimizationSurface::NotOptimizable { .. } => {
                panic!("test op must be Optimizable")
            }
        }
    }

    fn make_ctx<'a>(
        machine: &'a MachineProfile,
        material: &'a Material,
        tool: &'a ToolDefinition,
    ) -> AxisContext<'a> {
        AxisContext {
            project_default_rpm: 18_000,
            machine,
            tool,
            material,
        }
    }

    fn exceeds(peak_mm: f64) -> DeflectionVerdict {
        use crate::tool_load::verdict::{DeflectionBounds, SampleEvidence};
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    #[test]
    fn halves_doc_when_peak_is_8x_target() {
        // target = threshold × headroom = 0.04 × 1.0 = 0.04
        // peak = 0.32 → ratio 1/8 → cbrt(1/8) = 0.5
        // baseline DOC = 1.5mm (Pocket default) → target = 0.75mm.
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        let r = DeflectionDocRetargeter::with_headroom(0.04, 1.0);
        let v = exceeds(0.32);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        assert_eq!(sol.patches.len(), 1, "single primary DOC patch expected");
        let p = &sol.patches[0];
        assert_eq!(p.axis, SearchAxis::DepthPerPass);
        assert!(matches!(p.source, PatchSource::Primary));
        assert!(
            (p.value - 0.75).abs() < 1e-6,
            "expected 0.75, got {}",
            p.value
        );
        assert!(!p.clamped, "should not have clamped on this case");
    }

    #[test]
    fn near_threshold_produces_near_unity_multiplier() {
        // peak ≈ target → cbrt(~1) ≈ 1 → DOC barely changes.
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        // threshold = 0.200, headroom = 1.0, peak = 0.21 (just over)
        let r = DeflectionDocRetargeter::with_headroom(0.200, 1.0);
        let v = exceeds(0.21);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        let p = &sol.patches[0];
        // multiplier = cbrt(0.200 / 0.21) ≈ cbrt(0.9524) ≈ 0.9839
        // target DOC = 1.5 × 0.9839 ≈ 1.4759
        assert!(
            (p.value - 1.4759).abs() < 5e-3,
            "near-unity mult expected ~1.476, got {}",
            p.value
        );
        assert!(p.value < 1.5, "must shrink slightly");
        assert!(p.value > 1.4, "must shrink only slightly");
    }

    #[test]
    fn target_is_clamped_to_doc_bounds() {
        // Extreme overshoot → multiplier << 1 → raw target below hard
        // floor (0.05). Must clamp to floor.
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        // peak 1000× target → multiplier = cbrt(0.001) = 0.1
        // raw target = 1.5 × 0.1 = 0.15 (still above 0.05 floor)
        // Use a more extreme case: peak 1e9 × target → mult = 1e-3,
        // raw = 1.5e-3 → below 0.05 floor.
        let r = DeflectionDocRetargeter::with_headroom(1e-9, 1.0);
        let v = exceeds(1.0);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        let p = &sol.patches[0];
        let floor = space
            .axis(SearchAxis::DepthPerPass)
            .expect("doc bounds present")
            .hard
            .lo;
        assert!(
            (p.value - floor).abs() < 1e-9,
            "expected clamp to floor {}, got {}",
            floor,
            p.value
        );
    }

    #[test]
    fn returns_none_for_non_exceeds_deflection_verdict() {
        // The retargeter's `Verdict` associated type is now
        // `DeflectionVerdict`, so non-deflection verdicts can't reach it.
        // The remaining axis is verifying Within / Unmodeled are no-ops.
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        let r = DeflectionDocRetargeter::new(0.200);
        let within = DeflectionVerdict::Within {
            peak_mm: 0.05,
            bounds: crate::tool_load::verdict::DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: crate::tool_load::verdict::SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        };
        assert!(r.target(&within, &space, &view, &ctx).is_none());

        let unmodeled = DeflectionVerdict::Unmodeled {
            reason: crate::tool_load::verdict::UnmodeledReason::SimulationRequired,
        };
        assert!(r.target(&unmodeled, &space, &view, &ctx).is_none());
    }

    #[test]
    fn clamped_field_set_when_arithmetic_clamped() {
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        // Same construction as the floor-clamp test — just assert the
        // clamped flag this time.
        let r = DeflectionDocRetargeter::with_headroom(1e-9, 1.0);
        let v = exceeds(1.0);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        assert!(sol.patches[0].clamped, "expected clamped=true");
    }

    #[test]
    fn rationale_string_mentions_observed_peak_and_target() {
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        let r = DeflectionDocRetargeter::with_headroom(0.04, 1.0);
        let v = exceeds(0.32);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        // 0.32 mm peak, 0.04 mm target, 0.500 multiplier.
        assert!(
            sol.rationale.contains("0.3200"),
            "rationale missing peak: {}",
            sol.rationale
        );
        assert!(
            sol.rationale.contains("0.0400"),
            "rationale missing target: {}",
            sol.rationale
        );
        assert!(
            sol.rationale.contains("0.500"),
            "rationale missing multiplier: {}",
            sol.rationale
        );
    }

    #[test]
    fn emits_only_primary_patch_no_coupling() {
        let (op, space, tool) = build_space(1.5);
        let view = view_of(&op);
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let ctx = make_ctx(&machine, &material, &tool);

        let r = DeflectionDocRetargeter::new(0.200);
        let v = exceeds(0.4);
        let sol = r.target(&v, &space, &view, &ctx).expect("must retarget");
        assert_eq!(sol.patches.len(), 1);
        assert!(matches!(sol.patches[0].source, PatchSource::Primary));
    }
}
