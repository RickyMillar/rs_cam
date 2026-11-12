//! 3D Rough (`Adaptive3d`) on the Volumetric runner (`volumetric.rs`,
//! W6.5). The op builds the planner params from the config; the runner
//! owns the territory, the seed stock, the heights, the cancel, the spans
//! and `Produced`. Moved out of `finish_3d.rs` (W6.5).

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::Adaptive3dConfig;
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
    // G-DRAPEGAP: the model gouge guard's reduced tool, built as the guard
    // builds it (the op's allowance, `allowance_probe_radial_tolerance_mm`,
    // `reduced_tool_config`), so the planner's chord lift holds the guard's
    // own limit. A tool that cannot shrink gives `None`: the cutter.
    let gouge_probe =
        op.model_gouge_allowance(ctx.segment_merge_tolerance)
            .and_then(|allowance| {
                let radial = crate::compute::model_gouge::allowance_probe_radial_tolerance_mm(
                    allowance,
                    ctx.tool_def.radius(),
                );
                crate::compute::cutter::build_cutter(
                    &crate::compute::model_gouge::reduced_tool_config(ctx.tool_cfg, radial),
                )
                .ok()
            })
            .map(|t| std::sync::Arc::new(t) as std::sync::Arc<dyn MillingCutter>);
    let rough = Adaptive3dRough {
        cfg,
        stepover: crate::compute::not_set::required_geometry(
            cfg.stepover,
            op.op_type().label(),
            "stepover",
        )?,
        depth_per_pass: crate::compute::not_set::required_geometry(
            cfg.depth_per_pass,
            op.op_type().label(),
            "depth_per_pass",
        )?,
        tool: ctx.tool_def,
        // P29-8 8b: the ramp angle rule reads the stock material class.
        // The session always passes it.
        material: ctx.material.ok_or_else(|| {
            OperationError::Internal(
                "the 3D Rough entry rule reads the stock material, and the context holds none"
                    .to_owned(),
            )
        })?,
        gouge_probe,
        feed_rate: op.cut_feeds()?.feed_rate,
        plunge_rate: op.cut_feeds()?.plunge_rate,
        entry_feed_rate: op.entry_feed_rate(),
        link_kinematics: ctx.link_kinematics.clone(),
    };
    run_volumetric(&rough, ctx, prepared)
}

/// The 3D Rough op: the config, the tool and the resolved rates.
pub(crate) struct Adaptive3dRough<'a> {
    cfg: &'a Adaptive3dConfig,
    /// The set stepover and depth per pass (P10: the prologue stops an
    /// unset one).
    stepover: f64,
    depth_per_pass: f64,
    tool: &'a ToolDefinition,
    /// The stock material: the ramp angle rule reads its class (P29-8 8b).
    material: &'a crate::material::Material,
    /// G-DRAPEGAP: the guard's reduced tool (`Adaptive3dGeometry::gouge_probe`).
    gouge_probe: Option<std::sync::Arc<dyn MillingCutter>>,
    feed_rate: f64,
    plunge_rate: f64,
    entry_feed_rate: Option<f64>,
    /// C4: the machine's link timing, for the contour on each level.
    link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
}

impl Adaptive3dRough<'_> {
    /// The planner entry: P29-8, THE entry rule
    /// (`feeds::entry_rule::ResolvedEntry`) of the config on the tool (G10
    /// Q6: the helix radius D x factor, capped at the flat bottom so the
    /// helix leaves no core).
    fn entry_style(&self) -> crate::adaptive3d::EntryStyle3d {
        crate::feeds::entry_rule::ResolvedEntry::of_adaptive3d(self.cfg, self.tool, self.material)
            .style_3d()
    }

    /// The planner params, from the config and the runner's inputs.
    fn params(&self, inputs: &VolumetricInputs<'_>) -> crate::adaptive3d::Adaptive3dParams {
        let cfg = self.cfg;
        // Adaptive3d spaces passes by the tool's *engagement* radius at
        // the depth-of-cut, not the envelope radius — for tapered tools
        // these differ a lot. Floor at 0.01mm to keep stepover math safe
        // for degenerate (zero-tip) geometry.
        let engagement_radius = self
            .tool
            .engagement_radius_mm(self.depth_per_pass)
            .max(0.01);
        crate::adaptive3d::Adaptive3dParams {
            geometry: crate::adaptive3d::Adaptive3dGeometry {
                tool_radius: engagement_radius,
                envelope_radius: self.tool.envelope_radius_mm(),
                stepover: self.stepover,
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
                gouge_probe: self.gouge_probe.clone(),
            },
            depth: crate::adaptive3d::Adaptive3dDepth {
                depth_per_pass: self.depth_per_pass,
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
                rest_top_z: inputs.rest_top_z,
            },
            linking: crate::adaptive3d::Adaptive3dLinking {
                region_ordering: cfg.region_ordering.into(),
                // F-038: drop marching-squares regions whose forecast cut
                // length (perimeter + 2D adaptive walk) is below this floor.
                // Only honored by the GeodesicAdaptive strategy.
                min_region_cut_length_mm: cfg.min_region_cut_length_mm,
                // F-038b: keep-tool-down link policy. None lets the planner
                // default to 8 × tool diameter; Some(0.0) disables the
                // feature; Some(x) caps stay-down at x mm.
                max_stay_down_distance_mm: cfg.max_stay_down_distance_mm,
                stay_down_clearance_mm: cfg.stay_down_clearance_mm,
                step_split: None,
                keep_down_route: None,
                // C4: the contour on each level, when the op asks for it.
                level_contour: cfg
                    .contour_each_level
                    .then(|| crate::adaptive3d::LevelContour {
                        link_kinematics: self.link_kinematics.clone(),
                    }),
                // The rest region filter acts only on remaining stock (a
                // rest rough), and only when a bound is set.
                rest_region_filter: inputs.seed_stock.and(cfg.rest_region_filter()),
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
            // Load-only fields: no 3D Rough strategy reads them.
            trochoid_cap_mult: cfg.trochoid_cap_mult,
            engagement_measure: cfg.engagement_measure,
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
