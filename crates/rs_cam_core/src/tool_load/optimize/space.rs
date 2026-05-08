//! Top-level search space — the bounds of every axis an op exposes,
//! resolved once and queried by candidate generators and retargeters.
//!
//! Step 4 of G16. Companion to `bounds.rs`. The variant builders read
//! per-axis bounds directly via `resolve_*_bounds`; retargeters (Step 5)
//! will read whole-space context via [`SearchSpace::axis`] for hard
//! clamping.

use std::collections::BTreeMap;

use crate::feeds::vendor_lookup::MatchedRow;

use super::axes::{AxisContext, AxisView, SearchAxis};
use super::bounds::{AxisBounds, resolve_axis_bounds};
use super::policy::SearchPolicy;

/// Bounds for every axis the op surfaces. Built once per optimization
/// run; queried by strategies to generate candidates and by retargeters
/// to clamp computed targets.
#[derive(Debug, Clone)]
pub struct SearchSpace {
    pub bounds: BTreeMap<SearchAxis, AxisBounds>,
}

impl SearchSpace {
    /// Resolve every axis the view exposes that has a working resolver.
    /// Reserved axes (AngularStep / HelixPitch / RampAngle) return None
    /// from the resolver and are silently skipped.
    pub fn build(
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
        lut_row: Option<&MatchedRow>,
        policy: &SearchPolicy,
    ) -> Self {
        let mut bounds = BTreeMap::new();
        for binding in view.bindings {
            if let Some(b) = resolve_axis_bounds(binding.axis, view, ctx, lut_row, policy) {
                bounds.insert(binding.axis, b);
            }
        }
        Self { bounds }
    }

    /// Empty search space, used as a fallback when no axes are exposed.
    pub fn empty() -> Self {
        Self {
            bounds: BTreeMap::new(),
        }
    }

    pub fn axis(&self, axis: SearchAxis) -> Option<&AxisBounds> {
        self.bounds.get(&axis)
    }

    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }

    /// Multi-line debug summary, axis-by-axis. Useful for tracing and
    /// for refusal-explanation strings.
    pub fn summary(&self) -> String {
        if self.bounds.is_empty() {
            return "search space empty".to_owned();
        }
        let mut lines: Vec<String> = self.bounds.values().map(|b| b.summary()).collect();
        lines.sort();
        lines.join("\n")
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
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::tool::ToolDefinition;

    fn make_ctx_for_test<'a>(
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

    fn make_tool() -> ToolDefinition {
        let tool_config = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        ToolDefinition::new(
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
        )
    }

    #[test]
    fn search_space_built_for_pocket_has_doc_stepover_feed_rpm() {
        use crate::compute::operation_configs::PocketConfig;

        let op = OperationConfig::Pocket(PocketConfig::default());
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!("Pocket should be Optimizable");
        };
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let tool = make_tool();
        let ctx = make_ctx_for_test(&machine, &material, &tool);
        let policy = SearchPolicy::default();

        let space = SearchSpace::build(&view, &ctx, None, &policy);
        assert!(space.axis(SearchAxis::FeedRate).is_some());
        assert!(space.axis(SearchAxis::SpindleRpm).is_some());
        assert!(space.axis(SearchAxis::DepthPerPass).is_some());
        assert!(space.axis(SearchAxis::Stepover).is_some());
        // Pocket has no scallop_height axis.
        assert!(space.axis(SearchAxis::ScallopHeight).is_none());
    }

    #[test]
    fn search_space_summary_lists_every_axis() {
        use crate::compute::operation_configs::PocketConfig;

        let op = OperationConfig::Pocket(PocketConfig::default());
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!("Pocket should be Optimizable");
        };
        let machine = MachineProfile::shapeoko_makita();
        let material = Material::default();
        let tool = make_tool();
        let ctx = make_ctx_for_test(&machine, &material, &tool);
        let policy = SearchPolicy::default();

        let space = SearchSpace::build(&view, &ctx, None, &policy);
        let summary = space.summary();
        for needle in ["FeedRate", "SpindleRpm", "DepthPerPass", "Stepover"] {
            assert!(
                summary.contains(needle),
                "summary missing '{needle}': {summary}"
            );
        }
    }

    #[test]
    fn search_space_empty_returns_no_axes() {
        let space = SearchSpace::empty();
        assert!(space.is_empty());
        assert!(space.axis(SearchAxis::FeedRate).is_none());
    }
}
