//! 3D Rough (`Adaptive3d`) on the Volumetric runner (`volumetric.rs`,
//! W6.5). The op builds the planner params from the config; the runner
//! owns the territory, the seed stock, the heights, the cancel, the spans
//! and `Produced`. Moved out of `finish_3d.rs` (W6.5).

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
use crate::interrupt::CancelCheck;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::Span;

use super::volumetric::{
    HeightsPolicy, PlannerOwns, TopAnchor, VolumetricInputs, VolumetricOp, VolumetricOutput,
    run_volumetric,
};
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// Read the single leave-stock scalar the adaptive3d planner consumes.
///
/// The planner (`crate::adaptive3d::{path,clearing,search}`) is a
/// drop-cutter / dexel heightmap engine: every use of `stock_to_leave`
/// raises the "protected surface" (`surf_z + stock_to_leave`) purely in
/// the Z direction — z-level floors, waterline lift, and gouge-guard
/// drape all key off a single vertical offset from `point_drop_cutter`.
/// There is no wall-normal / horizontal offset path (no polygon inset,
/// no lateral shift of the EDT-derived contours), so this engine cannot
/// honour a radial (sidewall) leave allowance at all.
///
/// L2 deleted the inert `stock_to_leave_radial` dial that used to sit
/// beside the axial one. A caller that wants a wall offset needs a
/// planner mechanism first, not a dial the planner drops.
pub(super) fn adaptive3d_effective_stock_to_leave(cfg: &Adaptive3dConfig) -> f64 {
    cfg.stock_to_leave_axial
}

/// Adaptive3d family adapter: the op on the Volumetric runner.
pub(crate) fn generate_adaptive3d(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive3d, "generate_adaptive3d");
    let rough = Adaptive3dRough {
        cfg,
        tool: ctx.tool_def,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        entry_feed_rate: op.entry_feed_rate(),
    };
    run_volumetric(&rough, ctx, prepared)
}

/// The 3D Rough op: the config, the tool and the resolved rates.
pub(crate) struct Adaptive3dRough<'a> {
    cfg: &'a Adaptive3dConfig,
    tool: &'a ToolDefinition,
    feed_rate: f64,
    plunge_rate: f64,
    entry_feed_rate: Option<f64>,
}

impl Adaptive3dRough<'_> {
    /// The planner entry. Helix and Ramp carry values from the config and
    /// the tool, so this is a method, not a `From`.
    fn entry_style(&self) -> crate::adaptive3d::EntryStyle3d {
        match self.cfg.entry_style {
            Adaptive3dEntryStyle::Plunge => crate::adaptive3d::EntryStyle3d::Plunge,
            Adaptive3dEntryStyle::Ramp => crate::adaptive3d::EntryStyle3d::Ramp {
                max_angle_deg: self.cfg.ramp_angle_deg,
            },
            // G10 Q6: D x factor, capped at the flat bottom so the helix
            // leaves no core.
            Adaptive3dEntryStyle::Helix => crate::adaptive3d::EntryStyle3d::Helix {
                radius: self.cfg.helix_radius_for(self.tool).emitted_mm,
                pitch: self.cfg.helix_pitch,
            },
        }
    }

    /// The planner params, from the config and the runner's inputs.
    fn params(&self, inputs: &VolumetricInputs<'_>) -> crate::adaptive3d::Adaptive3dParams {
        let cfg = self.cfg;
        // Adaptive3d spaces passes by the tool's *engagement* radius at
        // the depth-of-cut, not the envelope radius — for tapered tools
        // these differ a lot. Floor at 0.01mm to keep stepover math safe
        // for degenerate (zero-tip) geometry.
        let engagement_radius = self.tool.engagement_radius_mm(cfg.depth_per_pass).max(0.01);
        crate::adaptive3d::Adaptive3dParams {
            geometry: crate::adaptive3d::Adaptive3dGeometry {
                tool_radius: engagement_radius,
                envelope_radius: self.tool.envelope_radius_mm(),
                stepover: cfg.stepover,
                tolerance: cfg.tolerance,
                segment_merge_tolerance: inputs.segment_merge_tolerance,
                min_cutting_radius: cfg.min_cutting_radius,
                boundary: inputs.outline.cloned(),
                // F-027: forward the world stock XY bounds so the planner's
                // internal `material_stock` extends to cover every cell the
                // simulator's per-setup dexel grid will look at. Pre-fix the
                // planner was bounded by `mesh.bbox + tool_radius`, while the
                // simulator's grid is bounded by the (auto-grown) world stock
                // bbox; cells inside the simulator grid but outside the
                // planner grid were never stamped, so the final pass carved
                // through the full stock height in one shot at model-edge
                // cells, blowing up `axial_engagement_mm` and the downstream
                // deflection gate.
                world_stock_xy_bbox: Some(inputs.world_stock_xy),
                // G-BOUNDARYPHANTOM: the containment the session clips to after
                // generation; the planner stamps only what that clip keeps.
                centre_boundary: inputs
                    .centre
                    .map(<[crate::polygon::Polygon2]>::to_vec)
                    .unwrap_or_default(),
            },
            depth: crate::adaptive3d::Adaptive3dDepth {
                depth_per_pass: cfg.depth_per_pass,
                stock_to_leave: adaptive3d_effective_stock_to_leave(cfg),
                // `HEIGHTS.top = StockBox`: see `TopAnchor::StockBox`.
                stock_top_z: inputs.top_z,
                // A user-pinned `bottom_z` IS honored: it floors the Z-level
                // plan (raising the floor only removes deep passes — always
                // safe; it's the lever that stops a rough descending through
                // holes in open meshes). Auto leaves the floor to the surface
                // heightmap (Auto resolves to `top - op_depth`, which has no
                // meaning for surface-driven roughing).
                z_floor: inputs.z_floor,
                detect_flat_areas: cfg.detect_flat_areas,
            },
            linking: crate::adaptive3d::Adaptive3dLinking {
                region_ordering: cfg.region_ordering.into(),
                // F-038: drop marching-squares regions whose forecast cut
                // length (perimeter + 2D adaptive walk) is below this floor.
                // Only honored by the AgentSearch strategy.
                min_region_cut_length_mm: cfg.min_region_cut_length_mm,
                // F-038b: keep-tool-down link policy. None lets the planner
                // default to 8 × tool diameter; Some(0.0) disables the
                // feature; Some(x) caps stay-down at x mm.
                max_stay_down_distance_mm: cfg.max_stay_down_distance_mm,
                stay_down_clearance_mm: cfg.stay_down_clearance_mm,
            },
            feed_rate: self.feed_rate,
            plunge_rate: self.plunge_rate,
            // G-RAMPCLAMP: the entry never runs faster than the cut feed.
            ramp_feed_rate: self.entry_feed_rate,
            entry_clearance_mm: cfg.entry_clearance_mm,
            entry_style: self.entry_style(),
            // `RestUse::Planner`: the raw seed stock (see the runner).
            initial_stock: inputs.seed_stock.cloned(),
            safe_z: inputs.safe_z,
            clearing_strategy: cfg.clearing_strategy.into(),
            // "Nibble" dial — forwarded to the ContourSpiral slice path.
            trochoid_cap_mult: cfg.trochoid_cap_mult,
            engagement_measure: cfg.engagement_measure,
            z_blend: cfg.z_blend,
        }
    }
}

impl VolumetricOp for Adaptive3dRough<'_> {
    const OWNS: PlannerOwns = PlannerOwns {
        stock: true,
        level_anchor: true,
        entries: true,
        links: true,
        segment_merge: true,
    };
    // Heights audit 2026-06-12, finding 2: the rough always anchors its top
    // on the stock bbox — a pinned `top_z` is deliberately NOT honored.
    // Roughing must start from the real material top: anchoring lower
    // leaves the overhead band unplanned while the simulator still carries
    // it, and the first pass would sweep the full pre-stamp ray in one bite
    // (the F-027 failure shape). It would also rewrite tuned historical
    // projects whose pinned tops were vacuous while heights were ignored
    // (wanaka100 Back Rough pins `top_z = model_top` under an 18 mm
    // overhead — the F-034 machine-calibration anchor measures that path).
    const HEIGHTS: HeightsPolicy = HeightsPolicy {
        top: TopAnchor::StockBox,
        pinned_bottom: true,
    };
    type Annotation = crate::adaptive3d::Adaptive3dRuntimeAnnotation;

    fn plan(
        &self,
        inputs: &VolumetricInputs<'_>,
        cancel: &dyn CancelCheck,
        debug: Option<&ToolpathDebugContext>,
    ) -> Result<VolumetricOutput<Self::Annotation>, OperationError> {
        let MeshInput { mesh, index } = inputs.surface;
        let crate::adaptive3d::Adaptive3dOutput {
            toolpath,
            annotations,
            produced,
        } = crate::adaptive3d::adaptive_3d_toolpath_output_with_cancel(
            mesh,
            index,
            self.tool,
            &self.params(inputs),
            cancel,
            debug,
        )?;
        Ok(VolumetricOutput {
            toolpath,
            annotations,
            produced,
        })
    }

    fn spans(&self, annotations: &[Self::Annotation], toolpath: &Toolpath) -> Vec<Span> {
        crate::compute::spans::spans_from_adaptive3d_annotations(annotations, toolpath.moves.len())
    }

    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        sem: &ToolpathSemanticContext,
    ) {
        crate::compute::annotate::annotate_adaptive3d(annotations, toolpath, sem);
    }
}
