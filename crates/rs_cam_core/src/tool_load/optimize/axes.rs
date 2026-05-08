//! Search axis topology — which knobs the optimizer can move on each
//! operation, with units and load/quality semantics.
//!
//! Step 3 of G16. Replaces the scattered `has_doc_knob` allowlist + the
//! `OperationParams::stepover()` Option-return heuristics with a typed
//! `&'static [AxisBinding]` declared per `OperationConfig` variant via
//! [`crate::compute::catalog::OperationConfig::optimization_surface`].
//!
//! Adding a new operation type without classifying its axes is a
//! compile-time error in `optimization_surface`. Adding a new
//! [`SearchAxis`] variant is a compile-time error in every match below.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::tool::ToolDefinition;

/// Knobs the optimizer may move on a toolpath. Closed enum; adding a
/// variant forces every match site to consider it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SearchAxis {
    /// Feed rate, mm/min. Resolved off the op's `feed_rate()`.
    FeedRate,
    /// Spindle RPM. Resolved via [`AxisContext::project_default_rpm`]
    /// when the op's `spindle_rpm()` is `None` (meaning "inherit project
    /// default", not "axis absent").
    SpindleRpm,
    /// Axial depth-per-pass, mm. Op-specific field name (`depth_per_pass`,
    /// `z_step`, `max_stepdown`).
    DepthPerPass,
    /// Radial engagement / stepover, mm.
    Stepover,
    /// Scallop-height quality target, mm. Quality axis, not load-driving.
    ScallopHeight,
    /// Angular step, degrees. Reserved for RadialFinish (G3a).
    AngularStep,
    /// Helix entry pitch, mm. Reserved for Adaptive3d helix entry.
    HelixPitch,
    /// Ramp-entry angle, degrees. Reserved.
    RampAngle,
}

/// Physical unit for a [`SearchAxis`]. Prevents accidental cross-unit
/// arithmetic at sites that operate on axes generically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisUnit {
    MmPerMin,
    Rpm,
    Mm,
    Deg,
}

/// What the axis means for optimization. Drives strategy selection:
/// `LoadDriving` axes feed retargeters; `QualityTarget` axes are only
/// swept for cycle-time impact, never retargeted to fix a load gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisSemantics {
    /// Directly affects per-sample cutting load. Retargeters drive these
    /// in response to chipload/power/deflection verdicts.
    LoadDriving {
        affects_chipload: bool,
        affects_force: bool,
    },
    /// Operator-set quality target — affects finish/tolerance and may
    /// affect cycle time, but retargeters do not drive it.
    QualityTarget,
    /// Cycle-time driver with no direct load impact. Reserved for future.
    CycleTimeDriving,
}

/// One axis of an operation, with the metadata the optimizer needs to
/// reason about it generically.
#[derive(Clone, Copy, Debug)]
pub struct AxisBinding {
    pub axis: SearchAxis,
    pub field_name: &'static str,
    pub unit: AxisUnit,
    pub semantics: AxisSemantics,
}

impl SearchAxis {
    pub const fn unit(self) -> AxisUnit {
        match self {
            SearchAxis::FeedRate => AxisUnit::MmPerMin,
            SearchAxis::SpindleRpm => AxisUnit::Rpm,
            SearchAxis::DepthPerPass | SearchAxis::Stepover | SearchAxis::ScallopHeight
            | SearchAxis::HelixPitch => AxisUnit::Mm,
            SearchAxis::AngularStep | SearchAxis::RampAngle => AxisUnit::Deg,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            SearchAxis::FeedRate => "Feed rate",
            SearchAxis::SpindleRpm => "Spindle RPM",
            SearchAxis::DepthPerPass => "Depth per pass",
            SearchAxis::Stepover => "Stepover",
            SearchAxis::ScallopHeight => "Scallop height",
            SearchAxis::AngularStep => "Angular step",
            SearchAxis::HelixPitch => "Helix pitch",
            SearchAxis::RampAngle => "Ramp angle",
        }
    }

    pub const fn is_feed_axis(self) -> bool {
        matches!(self, SearchAxis::FeedRate | SearchAxis::SpindleRpm)
    }
}

// ── Per-axis static AxisBinding constants ─────────────────────────────

const BIND_FEED: AxisBinding = AxisBinding {
    axis: SearchAxis::FeedRate,
    field_name: "feed_rate",
    unit: AxisUnit::MmPerMin,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,
        affects_force: true,
    },
};

const BIND_RPM: AxisBinding = AxisBinding {
    axis: SearchAxis::SpindleRpm,
    field_name: "spindle_rpm",
    unit: AxisUnit::Rpm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,
        affects_force: true,
    },
};

const BIND_DOC: AxisBinding = AxisBinding {
    axis: SearchAxis::DepthPerPass,
    field_name: "depth_per_pass",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: false,
        affects_force: true,
    },
};

const BIND_STEPOVER: AxisBinding = AxisBinding {
    axis: SearchAxis::Stepover,
    field_name: "stepover",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,
        affects_force: true,
    },
};

const BIND_SCALLOP_HEIGHT: AxisBinding = AxisBinding {
    axis: SearchAxis::ScallopHeight,
    field_name: "scallop_height",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::QualityTarget,
};

// ── Per-variant binding arrays ────────────────────────────────────────

pub(crate) const FEED_RPM_ONLY: &[AxisBinding] = &[BIND_FEED, BIND_RPM];
pub(crate) const FEED_RPM_DOC: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_DOC];
pub(crate) const FEED_RPM_STEPOVER: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_STEPOVER];
pub(crate) const FEED_RPM_DOC_STEPOVER: &[AxisBinding] =
    &[BIND_FEED, BIND_RPM, BIND_DOC, BIND_STEPOVER];
pub(crate) const FEED_RPM_SCALLOP: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_SCALLOP_HEIGHT];

// ── Surface types: AxisView, AxisContext ──────────────────────────────

/// Borrowed view onto an [`OperationConfig`] that's been classified as
/// `Optimizable`. Carries the static binding list so the optimizer can
/// iterate axes without per-call allocation.
#[derive(Clone, Copy, Debug)]
pub struct AxisView<'op> {
    pub op: &'op OperationConfig,
    pub bindings: &'static [AxisBinding],
    pub op_type: OperationType,
}

/// Runtime context needed to resolve axis values that depend on
/// inherited / environmental state.
///
/// **Critical for `SpindleRpm`:** `op.spindle_rpm()` returns
/// `Option<u32>` where `None` means "use project default", not "axis
/// absent". Without this context the optimizer would silently
/// under-search RPM for the common default-inheritance case.
#[derive(Clone, Copy)]
pub struct AxisContext<'a> {
    pub project_default_rpm: u32,
    pub machine: &'a MachineProfile,
    pub tool: &'a ToolDefinition,
    pub material: &'a Material,
}

impl<'op> AxisView<'op> {
    /// Iterate the axes this op exposes.
    pub fn axes(&self) -> impl Iterator<Item = SearchAxis> + '_ {
        self.bindings.iter().map(|b| b.axis)
    }

    /// Read the current value of an axis. Returns `None` for axes the op
    /// doesn't expose, or for runtime-conditional axes whose value isn't
    /// applicable in the current state (e.g., Pencil's stepover when
    /// `num_offset_passes <= 1`).
    pub fn axis_value(&self, axis: SearchAxis, ctx: &AxisContext<'_>) -> Option<f64> {
        match axis {
            SearchAxis::FeedRate => Some(self.op.feed_rate()),
            SearchAxis::SpindleRpm => Some(
                self.op
                    .spindle_rpm()
                    .map(f64::from)
                    .unwrap_or(f64::from(ctx.project_default_rpm)),
            ),
            SearchAxis::DepthPerPass => self.op.depth_per_pass(),
            SearchAxis::Stepover => self.op.stepover(),
            SearchAxis::ScallopHeight => self.op.scallop_height(),
            // Reserved for future gap closures. The optimizer will skip
            // them per the runtime-absent rule below.
            SearchAxis::AngularStep | SearchAxis::HelixPitch | SearchAxis::RampAngle => None,
        }
    }

    /// Active axes — the subset of declared bindings whose value is
    /// currently resolvable. Filters runtime-conditional axes (Pencil
    /// stepover, future angular_step, etc.).
    pub fn active_axes(&self, ctx: &AxisContext<'_>) -> Vec<AxisBinding> {
        self.bindings
            .iter()
            .copied()
            .filter(|b| self.axis_value(b.axis, ctx).is_some())
            .collect()
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
    fn search_axis_unit_label_consistency() {
        // Spot-check that each axis pairs sensibly. Compile-time
        // exhaustiveness already guards against a new axis missing.
        assert_eq!(SearchAxis::FeedRate.unit(), AxisUnit::MmPerMin);
        assert_eq!(SearchAxis::Stepover.unit(), AxisUnit::Mm);
        assert_eq!(SearchAxis::AngularStep.unit(), AxisUnit::Deg);
        assert!(SearchAxis::FeedRate.is_feed_axis());
        assert!(SearchAxis::SpindleRpm.is_feed_axis());
        assert!(!SearchAxis::DepthPerPass.is_feed_axis());
    }

    #[test]
    fn binding_array_shapes_have_expected_sizes() {
        assert_eq!(FEED_RPM_ONLY.len(), 2);
        assert_eq!(FEED_RPM_DOC.len(), 3);
        assert_eq!(FEED_RPM_STEPOVER.len(), 3);
        assert_eq!(FEED_RPM_DOC_STEPOVER.len(), 4);
        assert_eq!(FEED_RPM_SCALLOP.len(), 3);
    }

    #[test]
    fn binding_arrays_always_lead_with_feed_and_rpm() {
        for arr in [
            FEED_RPM_ONLY,
            FEED_RPM_DOC,
            FEED_RPM_STEPOVER,
            FEED_RPM_DOC_STEPOVER,
            FEED_RPM_SCALLOP,
        ] {
            assert_eq!(arr[0].axis, SearchAxis::FeedRate);
            assert_eq!(arr[1].axis, SearchAxis::SpindleRpm);
        }
    }

    #[test]
    fn scallop_height_is_quality_target_not_load_driving() {
        assert!(matches!(
            BIND_SCALLOP_HEIGHT.semantics,
            AxisSemantics::QualityTarget
        ));
        assert!(matches!(
            BIND_STEPOVER.semantics,
            AxisSemantics::LoadDriving { .. }
        ));
    }

    #[test]
    fn every_op_type_has_explicit_optimization_surface() {
        // The compile-time match-exhaustiveness in
        // OperationConfig::optimization_surface guarantees every variant
        // is classified. This test catches a runtime regression — e.g.,
        // someone adding a wildcard arm later.
        use crate::compute::catalog::{OperationConfig, OptimizationSurface};

        for &op_type in OperationType::ALL {
            let op = OperationConfig::new_default(op_type);
            match op.optimization_surface() {
                OptimizationSurface::Optimizable(view) => {
                    assert!(
                        !view.bindings.is_empty(),
                        "{op_type:?} declared Optimizable with empty bindings"
                    );
                    assert_eq!(
                        view.op_type, op_type,
                        "{op_type:?}: surface op_type mismatch ({:?})",
                        view.op_type
                    );
                }
                OptimizationSurface::NotOptimizable { .. } => {
                    // Drill / AlignmentPinDrill are the only two that
                    // currently classify as NotOptimizable. Any new
                    // NotOptimizable variant should be added here
                    // explicitly so the assertion list is auditable.
                    assert!(
                        matches!(
                            op_type,
                            OperationType::Drill | OperationType::AlignmentPinDrill
                        ),
                        "{op_type:?} unexpectedly NotOptimizable; if intentional, \
                         update this test list explicitly"
                    );
                }
            }
        }
    }

    #[test]
    fn axis_view_axis_value_resolves_for_declared_axes() {
        use crate::compute::catalog::{OperationConfig, OptimizationSurface};
        use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
        use crate::machine::MachineProfile;
        use crate::material::Material;
        use crate::tool::ToolDefinition;

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

        for &op_type in OperationType::ALL {
            let op = OperationConfig::new_default(op_type);
            let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
                continue;
            };
            for binding in view.bindings {
                let v = view.axis_value(binding.axis, &ctx);
                // Stepover is allowed to be None on Pencil (G3 conditional
                // when num_offset_passes <= 1; default config has 1 pass).
                let stepover_conditional = binding.axis == SearchAxis::Stepover
                    && op_type == OperationType::Pencil;
                if !stepover_conditional {
                    assert!(
                        v.is_some(),
                        "{op_type:?}.{:?} returned None for declared binding",
                        binding.axis
                    );
                }
            }
        }
    }

    #[test]
    fn spindle_rpm_falls_back_to_project_default() {
        // When op.spindle_rpm() returns None, AxisView::axis_value must
        // surface the project default — not None.
        use crate::compute::catalog::{OperationConfig, OptimizationSurface};
        use crate::compute::operation_configs::PocketConfig;
        use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
        use crate::machine::MachineProfile;
        use crate::material::Material;
        use crate::tool::ToolDefinition;

        let pocket = PocketConfig {
            spindle_rpm: None,
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
            project_default_rpm: 21_000,
            machine: &machine,
            tool: &tool,
            material: &material,
        };

        let rpm = view.axis_value(SearchAxis::SpindleRpm, &ctx);
        assert_eq!(
            rpm,
            Some(21_000.0),
            "spindle_rpm should fall back to project default when op's value is None"
        );
    }
}
