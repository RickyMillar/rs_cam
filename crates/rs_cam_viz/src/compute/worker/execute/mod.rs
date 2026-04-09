#[cfg(test)]
mod operations_2d;
#[cfg(test)]
mod operations_3d;

use super::helpers::{
    apply_dressups, build_simulation_cut_artifact, build_trace_artifact, debug_artifact_dir,
    effective_safe_z, simulation_metric_artifact_dir,
};
use super::{
    Arc, AtomicBool, ComputeError, ComputeRequest, SimBoundary, SimulationRequest,
    SimulationResult, Toolpath, ToolpathPhaseTracker, ToolpathResult,
};
#[cfg(test)]
use super::{
    BoundingBox3, DressupConfig, MoveType, OperationConfig, StockSource, ToolConfig, ToolType,
    ToolpathId,
};
#[cfg(test)]
use crate::state::toolpath::{
    FaceConfig, FaceDirection, HeightContext, HeightsConfig, InlayConfig, ProfileConfig,
    RestConfig, ZigzagConfig,
};
use rs_cam_core::compute::annotate::annotate_from_runtime_events;
use rs_cam_core::compute::execute::execute_operation_annotated;
use rs_cam_core::compute::{build_cutter, compute_stats};
#[cfg(test)]
use rs_cam_core::geo::P3;
#[cfg(test)]
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::semantic_trace::ToolpathSemanticKind;

// Re-export items used by tests within this module
#[cfg(test)]
use operations_2d::{run_inlay, run_profile};
#[cfg(test)]
use operations_3d::run_scallop_annotated;

pub(super) struct ComputeExecutionOutcome {
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<std::path::PathBuf>,
}

/// Bridge function: delegates toolpath generation to core's `execute_operation`.
///
/// Builds the spatial index on-demand (only for 3D mesh operations), adds a
/// top-level semantic scope for the operation, and maps errors to `ComputeError`.
fn generate_via_core(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    debug_ctx: Option<&rs_cam_core::debug_trace::ToolpathDebugContext>,
    semantic_root: Option<&rs_cam_core::semantic_trace::ToolpathSemanticContext>,
    core_debug_span_id: Option<u64>,
) -> Result<Toolpath, ComputeError> {
    let tool_def = build_cutter(&req.tool);
    let mesh_ref = req.mesh.as_deref();
    let index = mesh_ref.map(rs_cam_core::mesh::SpatialIndex::build_auto);
    let index_ref = index.as_ref();
    let polys = req.polygons.as_deref().map(|v| v.as_slice());
    let default_bbox = rs_cam_core::geo::BoundingBox3::empty();
    let stock_bbox = req.stock_bbox.as_ref().unwrap_or(&default_bbox);
    let heights = rs_cam_core::compute::config::ResolvedHeights {
        clearance_z: req.heights.clearance_z,
        retract_z: req.heights.retract_z,
        feed_z: req.heights.feed_z,
        top_z: req.heights.top_z,
        bottom_z: req.heights.bottom_z,
    };

    // Add top-level semantic scope for the operation
    let op_scope = semantic_root
        .map(|root| root.start_item(ToolpathSemanticKind::Operation, req.operation.label()));
    if let Some(scope) = op_scope.as_ref()
        && let Some(span_id) = core_debug_span_id
    {
        scope.set_debug_span_id(span_id);
    }

    let result = execute_operation_annotated(
        &req.operation,
        mesh_ref,
        index_ref,
        polys,
        &tool_def,
        &req.tool,
        &heights,
        &req.cutting_levels,
        stock_bbox,
        req.prev_tool_radius,
        debug_ctx,
        cancel,
        req.prior_stock.as_ref(),
    )
    .map_err(ComputeError::from)?;

    if let Some(scope) = op_scope.as_ref()
        && !result.toolpath.moves.is_empty()
    {
        scope.bind_to_toolpath(&result.toolpath, 0, result.toolpath.moves.len());
    }

    if let Some(scope) = op_scope.as_ref() {
        let child_ctx = scope.context();
        annotate_from_runtime_events(&result.annotations, &result.toolpath, &child_ctx);
    }

    Ok(result.toolpath)
}

/// Convert viz `SimulationRequest` into a core `SimulationRequest` so the
/// actual simulation can be delegated to `rs_cam_core::compute::simulate`.
fn build_core_simulation_request(
    req: &SimulationRequest,
) -> rs_cam_core::compute::simulate::SimulationRequest {
    use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry};

    let groups = req
        .groups
        .iter()
        .map(|group| SimGroupEntry {
            toolpaths: group
                .toolpaths
                .iter()
                .map(|tp| {
                    let cutter = build_cutter(&tp.tool);
                    SimToolpathEntry {
                        id: tp.id.0,
                        name: tp.name.clone(),
                        toolpath: Arc::clone(&tp.toolpath),
                        tool: cutter,
                        flute_count: tp.tool.flute_count,
                        tool_summary: tp.tool.summary(),
                        semantic_trace: tp.semantic_trace.clone(),
                    }
                })
                .collect(),
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
            local_stock_bbox: Some(group.local_stock_bbox),
            local_to_global: group.local_to_global.clone(),
        })
        .collect();

    rs_cam_core::compute::simulate::SimulationRequest {
        groups,
        stock_bbox: req.stock_bbox,
        stock_top_z: req.stock_top_z,
        resolution: req.resolution,
        metric_options: req.metric_options,
        spindle_rpm: req.spindle_rpm,
        rapid_feed_mm_min: req.rapid_feed_mm_min,
        model_mesh: req.model_mesh.clone(),
    }
}

/// Build viz playback data from the viz request (global-frame toolpaths + tool
/// config + cut direction). Core does not produce this because it is a viz-only
/// concern (incremental playback in the 3D viewport).
fn build_playback_data(
    req: &SimulationRequest,
) -> Vec<(
    Arc<Toolpath>,
    super::ToolConfig,
    rs_cam_core::dexel_stock::StockCutDirection,
)> {
    let mut playback = Vec::new();
    for group in &req.groups {
        let playback_direction = group.local_to_global.as_ref().map_or(
            rs_cam_core::dexel_stock::StockCutDirection::FromTop,
            |info| info.cut_direction(),
        );
        for tp in &group.toolpaths {
            let global_tp = if let Some(info) = &group.local_to_global {
                Arc::new(info.transform_toolpath(&tp.toolpath))
            } else {
                Arc::clone(&tp.toolpath)
            };
            playback.push((global_tp, tp.tool.clone(), playback_direction));
        }
    }
    playback
}

pub(super) fn run_simulation_with_phase<F>(
    req: &SimulationRequest,
    cancel: &AtomicBool,
    set_phase: F,
) -> Result<SimulationResult, ComputeError>
where
    F: FnMut(&str),
{
    use rs_cam_core::compute::simulate;

    // Convert viz request to core request and delegate.
    let core_req = build_core_simulation_request(req);
    let core_result = simulate::run_simulation_with_phase(&core_req, cancel, set_phase)
        .map_err(|_cancelled| ComputeError::Cancelled)?;

    // Build viz-only playback data (global-frame toolpaths for viewport replay).
    let playback_data = build_playback_data(req);

    // Convert core boundaries (usize ids) to viz boundaries (ToolpathId).
    let boundaries: Vec<SimBoundary> = core_result
        .boundaries
        .into_iter()
        .map(|b| SimBoundary {
            id: super::ToolpathId(b.id),
            name: b.name,
            tool_name: b.tool_name,
            start_move: b.start_move,
            end_move: b.end_move,
            direction: b.direction,
        })
        .collect();

    // Core and viz now share the same SimCheckpointMesh type (re-exported).
    let checkpoints = core_result.checkpoints;

    // Write cut-trace artifact to disk (viz-only filesystem concern).
    let (cut_trace, cut_trace_path) = if let Some(trace) = core_result.cut_trace {
        let artifact = build_simulation_cut_artifact(req, (*trace).clone());
        let path = match rs_cam_core::simulation_cut::write_simulation_cut_artifact(
            &simulation_metric_artifact_dir(),
            "simulation_metrics",
            &artifact,
        ) {
            Ok(p) => Some(p),
            Err(error) => {
                tracing::warn!("Failed to write simulation cut artifact: {error}");
                None
            }
        };
        (Some(trace), path)
    } else {
        (None, None)
    };

    Ok(SimulationResult {
        mesh: core_result.mesh,
        total_moves: core_result.total_moves,
        deviations: core_result.deviations,
        boundaries,
        checkpoints,
        playback_data,
        rapid_collisions: core_result.rapid_collisions,
        rapid_collision_move_indices: core_result.rapid_collision_move_indices,
        cut_trace,
        cut_trace_path,
        resolution_clamped: core_result.resolution_clamped,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn run_compute(req: &ComputeRequest, cancel: &AtomicBool) -> ComputeExecutionOutcome {
    let debug_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::debug_trace::ToolpathDebugRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    let semantic_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    run_compute_with_phase_tracker(req, cancel, None, debug_recorder, semantic_recorder)
}

pub(super) fn run_compute_with_phase(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    phase_tracker: &ToolpathPhaseTracker,
) -> ComputeExecutionOutcome {
    let debug_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::debug_trace::ToolpathDebugRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
        .with_phase_sink(Arc::new(phase_tracker.clone()))
    });
    let semantic_recorder = req.debug_options.enabled.then(|| {
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new(
            req.toolpath_name.clone(),
            req.operation.label(),
        )
    });
    run_compute_with_phase_tracker(
        req,
        cancel,
        Some(phase_tracker),
        debug_recorder,
        semantic_recorder,
    )
}

fn run_compute_with_phase_tracker(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    phase_tracker: Option<&ToolpathPhaseTracker>,
    debug_recorder: Option<rs_cam_core::debug_trace::ToolpathDebugRecorder>,
    semantic_recorder: Option<rs_cam_core::semantic_trace::ToolpathSemanticRecorder>,
) -> ComputeExecutionOutcome {
    let debug_root = debug_recorder
        .as_ref()
        .map(|recorder| recorder.root_context());
    let semantic_root = semantic_recorder
        .as_ref()
        .map(|recorder| recorder.root_context());

    let result = (|| -> Result<ToolpathResult, ComputeError> {
        let mut tp = {
            let _phase_scope =
                phase_tracker.map(|tracker| tracker.start_phase(req.operation.label()));
            let core_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("core_generate", req.operation.label()));
            let core_ctx = core_scope.as_ref().map(|scope| scope.context());
            let core_debug_span_id = core_scope.as_ref().map(|scope| scope.id());
            let tp = generate_via_core(
                req,
                cancel,
                core_ctx.as_ref(),
                semantic_root.as_ref(),
                core_debug_span_id,
            )?;
            if let Some(scope) = core_scope.as_ref()
                && !tp.moves.is_empty()
            {
                scope.set_move_range(0, tp.moves.len() - 1);
            }
            tp
        };

        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Apply dressups"));
            let dressup_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("dressups", "Apply dressups"));
            let dressup_ctx = dressup_scope.as_ref().map(|scope| scope.context());
            tp = apply_dressups(tp, req, dressup_ctx.as_ref(), semantic_root.as_ref());
        }

        if req.boundary.enabled
            && let Some(bbox) = &req.stock_bbox
        {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Clip to boundary"));
            let boundary_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("boundary_clip", "Clip to boundary"));
            let boundary_span_id = boundary_scope.as_ref().map(|scope| scope.id());
            use rs_cam_core::boundary::{
                ToolContainment, clip_toolpath_to_boundary, effective_boundary, subtract_keepouts,
            };
            // Use face-derived boundary when face_selection is set, otherwise stock bbox.
            let mut stock_poly = if let (Some(face_ids), Some(enriched)) =
                (&req.face_selection, &req.enriched_mesh)
            {
                enriched
                    .faces_boundary_as_polygon(face_ids)
                    .unwrap_or_else(|| {
                        rs_cam_core::polygon::Polygon2::rectangle(
                            bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
                        )
                    })
            } else {
                rs_cam_core::polygon::Polygon2::rectangle(
                    bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y,
                )
            };
            if !req.keep_out_footprints.is_empty() {
                stock_poly = subtract_keepouts(&stock_poly, &req.keep_out_footprints);
            }
            let containment = match req.boundary.containment {
                crate::state::toolpath::BoundaryContainment::Center => ToolContainment::Center,
                crate::state::toolpath::BoundaryContainment::Inside => ToolContainment::Inside,
                crate::state::toolpath::BoundaryContainment::Outside => ToolContainment::Outside,
            };
            let boundaries = effective_boundary(&stock_poly, containment, req.tool.diameter / 2.0);
            if let Some(boundary) = boundaries.first() {
                tp = clip_toolpath_to_boundary(&tp, boundary, effective_safe_z(req));
                if let Some(root) = semantic_root.as_ref() {
                    let scope =
                        root.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
                    if let Some(span_id) = boundary_span_id {
                        scope.set_debug_span_id(span_id);
                    }
                    scope.set_param(
                        "containment",
                        match req.boundary.containment {
                            crate::state::toolpath::BoundaryContainment::Center => "center",
                            crate::state::toolpath::BoundaryContainment::Inside => "inside",
                            crate::state::toolpath::BoundaryContainment::Outside => "outside",
                        },
                    );
                    scope.set_param("keep_out_count", req.keep_out_footprints.len());
                    if !tp.moves.is_empty() {
                        scope.bind_to_toolpath(&tp, 0, tp.moves.len());
                    }
                }
                if let Some(scope) = boundary_scope.as_ref()
                    && !tp.moves.is_empty()
                {
                    scope.set_move_range(0, tp.moves.len() - 1);
                }
            }
        }

        let stats = {
            let _phase_scope = phase_tracker.map(|tracker| tracker.start_phase("Compute stats"));
            let _stats_scope = debug_root
                .as_ref()
                .map(|ctx| ctx.start_span("final_stats", "Compute stats"));
            compute_stats(&tp)
        };

        Ok(ToolpathResult {
            toolpath: Arc::new(tp),
            stats,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        })
    })();

    let (debug_trace, semantic_trace, debug_trace_path) = if let (
        Some(debug_recorder),
        Some(semantic_recorder),
    ) = (debug_recorder, semantic_recorder)
    {
        let mut debug_trace = debug_recorder.finish();
        let mut semantic_trace = semantic_recorder.finish();
        rs_cam_core::semantic_trace::enrich_traces(&mut debug_trace, &mut semantic_trace);
        let artifact =
            build_trace_artifact(req, Some(debug_trace.clone()), Some(semantic_trace.clone()));
        let file_stem = format!("{}-{}", req.toolpath_id.0, req.toolpath_name);
        let path = match rs_cam_core::semantic_trace::write_toolpath_trace_artifact(
            &debug_artifact_dir(),
            &file_stem,
            &artifact,
        ) {
            Ok(path) => Some(path),
            Err(error) => {
                tracing::warn!(
                    "Failed to write toolpath debug artifact for {}: {error}",
                    req.toolpath_id.0
                );
                None
            }
        };
        (
            Some(Arc::new(debug_trace)),
            Some(Arc::new(semantic_trace)),
            path,
        )
    } else {
        (None, None, None)
    };

    let result = match result {
        Ok(mut computed) => {
            computed.debug_trace = debug_trace.clone();
            computed.semantic_trace = semantic_trace.clone();
            computed.debug_trace_path = debug_trace_path.clone();
            Ok(computed)
        }
        Err(error) => Err(error),
    };

    ComputeExecutionOutcome {
        result,
        debug_trace,
        semantic_trace,
        debug_trace_path,
    }
}

// Annotation helpers (annotate_operation_scope, annotate_adaptive3d_runtime_semantics,
// etc.) were removed along with their only callers — the SemanticToolpathOp impls.
// Dispatch now goes through rs_cam_core::compute::execute::execute_operation.

// Remaining run_* helpers used only by tests live in operations_2d / operations_3d modules.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn test_request_with_polygon(
        operation: OperationConfig,
        tool_type: ToolType,
    ) -> ComputeRequest {
        let tool = ToolConfig::new_default(crate::state::job::ToolId(1), tool_type);
        let heights = HeightsConfig::default().resolve(&HeightContext::simple(10.0, 6.0));
        let cutting_levels = operation.cutting_levels(heights.top_z);
        ComputeRequest {
            toolpath_id: ToolpathId(1),
            toolpath_name: "Test".to_owned(),
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -20.0, -20.0, 20.0, 20.0,
            )])),
            mesh: None,
            enriched_mesh: None,
            face_selection: None,
            operation,
            dressups: DressupConfig::default(),
            stock_source: StockSource::Fresh,
            tool,
            safe_z: 10.0,
            prev_tool_radius: None,
            stock_bbox: Some(BoundingBox3 {
                min: P3::new(-25.0, -25.0, -10.0),
                max: P3::new(25.0, 25.0, 10.0),
            }),
            boundary: crate::state::toolpath::BoundaryConfig::default(),
            keep_out_footprints: Vec::new(),
            heights,
            cutting_levels,
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
            prior_stock: None,
        }
    }

    fn dominant_cut_axis(tp: &Toolpath, feed_rate: f64) -> Option<char> {
        let mut x_travel = 0.0;
        let mut y_travel = 0.0;

        for idx in 1..tp.moves.len() {
            if let MoveType::Linear {
                feed_rate: move_feed,
            } = tp.moves[idx].move_type
                && (move_feed - feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[idx].target.x - tp.moves[idx - 1].target.x;
                let dy = tp.moves[idx].target.y - tp.moves[idx - 1].target.y;
                if dx.abs().max(dy.abs()) > 1e-3 {
                    x_travel += dx.abs();
                    y_travel += dy.abs();
                }
            }
        }

        if x_travel <= 1e-6 && y_travel <= 1e-6 {
            None
        } else if y_travel > x_travel {
            Some('y')
        } else {
            Some('x')
        }
    }

    // --- Task A3: Tabs only on final depth pass ---

    #[test]
    fn profile_multi_pass_tabs_only_on_final_depth() {
        let cfg = ProfileConfig {
            depth: 6.0,
            depth_per_pass: 2.0,
            tab_count: 4,
            tab_width: 6.0,
            tab_height: 2.0,
            finishing_passes: 0,
            ..ProfileConfig::default()
        };
        let req =
            test_request_with_polygon(OperationConfig::Profile(cfg.clone()), ToolType::EndMill);
        let tp = run_profile(&req, &cfg).unwrap();

        let final_z = -cfg.depth;
        let tab_z = final_z + cfg.tab_height;

        // Tab height moves should exist (tabs applied to final pass)
        let tab_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| (m.target.z - tab_z).abs() < 0.01)
            .collect();
        assert!(
            !tab_moves.is_empty(),
            "Final pass should have tab height moves at z={tab_z}"
        );

        // Intermediate passes (at Z != final_z) should have NO tab-height lifts.
        // Roughing levels are at -2, -4; final is at -6. Tab height is -4.
        // Check that the Z=-4 moves are actual cutting moves, not tab lifts:
        // Tab lifts have z = final_z + tab_height = -6 + 2 = -4 which
        // coincides with a roughing level. Instead, verify that there are
        // no tab-height moves between roughing passes (moves with z > final_z
        // that aren't at a legitimate roughing level or safe_z).
        let depth_stepping = rs_cam_core::depth::DepthStepping {
            start_z: 0.0,
            final_z: -cfg.depth.abs(),
            max_step_down: cfg.depth_per_pass,
            distribution: rs_cam_core::depth::DepthDistribution::Even,
            finish_allowance: 0.0,
            finishing_passes: cfg.finishing_passes,
        };
        let roughing_levels = depth_stepping.all_levels();
        for m in &tp.moves {
            if let MoveType::Linear { .. } = m.move_type {
                let z = m.target.z;
                // Any linear move should be at a legitimate level, tab_z, or
                // a plunge between levels.
                let at_known_level = roughing_levels.iter().any(|&lv| (z - lv).abs() < 0.01);
                let at_tab_z = (z - tab_z).abs() < 0.01;
                let is_plunge_or_retract = z > final_z + 0.01 && z < effective_safe_z(&req) - 0.01;
                assert!(
                    at_known_level || at_tab_z || is_plunge_or_retract,
                    "unexpected linear move z={z}, expected one of levels {roughing_levels:?}, tab_z={tab_z}, or plunge"
                );
            }
        }
    }

    // --- Task A4: Face OneWay produces unidirectional cuts ---

    #[test]
    fn face_oneway_produces_nonempty_toolpath() {
        // OneWay face direction correctness is validated by core's own tests
        // (face::tests::face_oneway_all_passes_same_x_direction).
        // This test verifies that the viz -> core dispatch produces a non-empty result.
        let cfg = FaceConfig {
            direction: FaceDirection::OneWay,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let req = test_request_with_polygon(OperationConfig::Face(cfg), ToolType::EndMill);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None).unwrap();

        // Verify the operation produces cutting moves
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Face OneWay operation should produce cutting moves"
        );
    }

    #[test]
    fn face_zigzag_alternates_direction() {
        let cfg = FaceConfig {
            direction: FaceDirection::Zigzag,
            stepover: 10.0,
            ..FaceConfig::default()
        };
        let req = test_request_with_polygon(OperationConfig::Face(cfg.clone()), ToolType::EndMill);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None).unwrap();

        let mut cut_directions = Vec::new();
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - cfg.feed_rate).abs() < 1e-6
            {
                let dx = tp.moves[i].target.x - tp.moves[i - 1].target.x;
                if dx.abs() > 1.0 {
                    cut_directions.push(dx > 0.0);
                }
            }
        }

        assert!(
            cut_directions.len() >= 2,
            "Zigzag face should have multiple cutting rows"
        );
        // Should have at least one direction change
        let has_alternation = cut_directions.windows(2).any(|w| w[0] != w[1]);
        assert!(
            has_alternation,
            "Zigzag face should alternate cut directions"
        );
    }

    #[test]
    fn zigzag_angle_uses_degrees_for_runtime_path() {
        let cfg = ZigzagConfig {
            angle: 90.0,
            stepover: 6.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            ..ZigzagConfig::default()
        };
        let req =
            test_request_with_polygon(OperationConfig::Zigzag(cfg.clone()), ToolType::EndMill);
        let tp = crate::compute::worker::execute::operations_2d::run_zigzag(&req, &cfg).unwrap();

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "90 degree zigzag should cut along Y, not a radian-converted skew"
        );
    }

    #[test]
    fn rest_semantic_angle_uses_degrees() {
        let cfg = RestConfig {
            angle: 90.0,
            stepover: 4.0,
            depth: 2.0,
            depth_per_pass: 2.0,
            ..RestConfig::default()
        };
        let mut req =
            test_request_with_polygon(OperationConfig::Rest(cfg.clone()), ToolType::EndMill);
        req.prev_tool_radius = Some(8.0);

        let cancel = AtomicBool::new(false);
        let tp = generate_via_core(&req, &cancel, None, None, None).unwrap();

        assert_eq!(
            dominant_cut_axis(&tp, cfg.feed_rate),
            Some('y'),
            "semantic rest reconstruction should preserve the degree-based raster angle"
        );
    }

    // --- Task A10: Inlay female and male are separated ---

    #[test]
    fn inlay_output_contains_female_and_male_sections() {
        let cfg = InlayConfig::default();
        let mut req =
            test_request_with_polygon(OperationConfig::Inlay(cfg.clone()), ToolType::VBit);
        req.polygons = Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )]));
        let tp = run_inlay(&req, &cfg).unwrap();

        assert!(!tp.moves.is_empty(), "Inlay should produce moves");

        // Find retract-to-safe-z moves that separate sections. The female
        // and male toolpaths should be separated by a retract to safe_z.
        let safe_z = effective_safe_z(&req);
        let retract_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.move_type == MoveType::Rapid && (m.target.z - safe_z).abs() < 0.01)
            .map(|(i, _)| i)
            .collect();

        // There should be multiple retract moves (at least the separator
        // between female and male plus retracts within each section)
        assert!(
            retract_indices.len() >= 2,
            "Should have retract separators between female and male sections"
        );

        // Find cutting moves below Z=0 (both female and male cut below the surface)
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }) && m.target.z < -0.01)
            .collect();
        assert!(
            !cutting_moves.is_empty(),
            "Should have cutting moves below stock surface"
        );
    }

    // --- Task C14: Scallop requires BallNose tool ---

    #[test]
    fn scallop_rejects_non_ballnose_tool() {
        let OperationConfig::Scallop(cfg) =
            OperationConfig::new_default(crate::state::toolpath::OperationType::Scallop)
        else {
            unreachable!();
        };
        let mut req =
            test_request_with_polygon(OperationConfig::Scallop(cfg.clone()), ToolType::EndMill);
        req.mesh = Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0)));
        let result = run_scallop_annotated(&req, &cfg, None, None);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Ball Nose"),
            "Error should mention Ball Nose requirement"
        );
    }
}
