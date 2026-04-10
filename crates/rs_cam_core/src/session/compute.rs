//! Compute and mutation methods on [`ProjectSession`].

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::compute::annotate::annotate_from_runtime_events;
use crate::compute::collision_check::{
    CollisionCheckRequest, CollisionCheckResult, run_collision_check,
};
use crate::compute::config::{HeightContext, ToolpathStats};
use crate::compute::cutter::build_cutter;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use crate::debug_trace::ToolpathDebugRecorder;
use crate::dexel_stock::StockCutDirection;
use crate::geo::{BoundingBox3, P3};
use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces};
use crate::simulation_cut::SimulationMetricOptions;

use super::{
    ProjectDiagnostics, ProjectSession, SessionError, SimulationOptions, ToolpathComputeResult,
    ToolpathDiagnostic,
};

impl ProjectSession {
    // ── Mutation ──────────────────────────────────────────────────

    /// Set a parameter on a toolpath's operation config.
    ///
    /// Common parameters (`feed_rate`, `plunge_rate`, `stepover`, `depth_per_pass`)
    /// are applied via the [`OperationParams`] trait. Config-specific parameters
    /// (e.g. `angle`, `min_z`, `passes`) are applied via serde round-trip so that
    /// all 23 operation variants are handled generically.
    ///
    /// Invalidates the cached compute result for this toolpath.
    pub fn set_toolpath_param(
        &mut self,
        index: usize,
        param: &str,
        value: serde_json::Value,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        match param {
            "feed_rate" => {
                let v = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("feed_rate must be a number".to_owned())
                })?;
                tc.operation.set_feed_rate(v);
            }
            "plunge_rate" => {
                let v = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("plunge_rate must be a number".to_owned())
                })?;
                tc.operation.set_plunge_rate(v);
            }
            "stepover" => {
                let v = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("stepover must be a number".to_owned())
                })?;
                tc.operation.set_stepover(v);
            }
            "depth_per_pass" => {
                let v = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("depth_per_pass must be a number".to_owned())
                })?;
                tc.operation.set_depth_per_pass(v);
            }
            _ => {
                // Config-specific param: serialize -> merge -> deserialize
                let mut json = serde_json::to_value(&tc.operation).map_err(|e| {
                    SessionError::InvalidParam(format!("failed to serialize config: {e}"))
                })?;
                // The OperationConfig uses tagged representation: { "kind": "...", "params": { ... } }
                // Merge the param into the "params" object.
                let params_obj = json
                    .get_mut("params")
                    .and_then(|v| v.as_object_mut())
                    .ok_or_else(|| {
                        SessionError::InvalidParam(
                            "unexpected config structure during serde round-trip".to_owned(),
                        )
                    })?;
                // Insert the value — even if the key doesn't exist yet (handles
                // Optional fields that serde skips when None). The deserialize
                // step below will reject truly unknown fields.
                params_obj.insert(param.to_owned(), value);
                tc.operation = serde_json::from_value(json).map_err(|e| {
                    SessionError::InvalidParam(format!("invalid value for '{param}': {e}"))
                })?;
            }
        }

        // Invalidate cached result for this toolpath
        self.results.remove(&index);
        self.simulation = None;

        Ok(())
    }

    /// Set a parameter on a tool definition.
    ///
    /// Supported parameters: `diameter`, `flute_count`, `stickout`, `corner_radius`,
    /// `cutting_length`, `included_angle`, `taper_half_angle`, `shaft_diameter`,
    /// `shank_diameter`, `shank_length`, `holder_diameter`.
    ///
    /// Invalidates cached results for all toolpaths that reference this tool.
    pub fn set_tool_param(
        &mut self,
        index: usize,
        param: &str,
        value: &serde_json::Value,
    ) -> Result<(), SessionError> {
        let tool = self.tools.get_mut(index).ok_or_else(|| {
            SessionError::InvalidParam(format!("tool index {index} out of bounds"))
        })?;

        match param {
            "diameter" => {
                tool.diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("diameter must be a number".to_owned())
                })?;
            }
            "flute_count" => {
                let v = value.as_u64().ok_or_else(|| {
                    SessionError::InvalidParam("flute_count must be an integer".to_owned())
                })?;
                tool.flute_count = v as u32;
            }
            "stickout" => {
                tool.stickout = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("stickout must be a number".to_owned())
                })?;
            }
            "corner_radius" => {
                tool.corner_radius = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("corner_radius must be a number".to_owned())
                })?;
            }
            "cutting_length" => {
                tool.cutting_length = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("cutting_length must be a number".to_owned())
                })?;
            }
            "included_angle" => {
                tool.included_angle = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("included_angle must be a number".to_owned())
                })?;
            }
            "taper_half_angle" => {
                tool.taper_half_angle = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("taper_half_angle must be a number".to_owned())
                })?;
            }
            "shaft_diameter" => {
                tool.shaft_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shaft_diameter must be a number".to_owned())
                })?;
            }
            "shank_diameter" => {
                tool.shank_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shank_diameter must be a number".to_owned())
                })?;
            }
            "shank_length" => {
                tool.shank_length = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("shank_length must be a number".to_owned())
                })?;
            }
            "holder_diameter" => {
                tool.holder_diameter = value.as_f64().ok_or_else(|| {
                    SessionError::InvalidParam("holder_diameter must be a number".to_owned())
                })?;
            }
            _ => {
                return Err(SessionError::InvalidParam(format!(
                    "unknown tool parameter '{param}'"
                )));
            }
        }

        // Invalidate cached results for all toolpaths that use this tool
        let tool_raw_id = tool.id.0;
        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if tc.tool_id == tool_raw_id {
                self.results.remove(&idx);
            }
        }
        self.simulation = None;

        Ok(())
    }

    // ── Compute ────────────────────────────────────────────────────

    /// Generate a single toolpath by index.
    pub fn generate_toolpath(
        &mut self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<&ToolpathComputeResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?
            .clone();

        let model = self.find_model_by_raw_id(tc.model_id);

        let mut mesh = model.and_then(|m| m.mesh.clone());
        let mut polygons = model.and_then(|m| m.polygons.clone());

        // Validate geometry requirements
        if tc.operation.is_3d() && mesh.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires a 3D mesh (STL/STEP)".to_owned(),
            ));
        }
        if !tc.operation.is_3d() && !tc.operation.is_stock_based() && polygons.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires 2D geometry (SVG/DXF)".to_owned(),
            ));
        }

        // Find the setup for orientation and keep-out info
        let setup = self.find_setup_for_toolpath_index(index);
        let face_up = setup.map(|s| s.face_up).unwrap_or(FaceUp::Top);
        let z_rotation = setup.map(|s| s.z_rotation).unwrap_or(ZRotation::Deg0);
        let needs_transform = face_up != FaceUp::Top || z_rotation != ZRotation::Deg0;

        // Collect keep-out footprints from setup fixtures and keep-out zones
        let mut keep_out_footprints: Vec<crate::polygon::Polygon2> = Vec::new();
        if let Some(s) = setup {
            for fixture in &s.fixtures {
                if fixture.enabled {
                    keep_out_footprints.push(fixture.footprint());
                }
            }
            for keep_out in &s.keep_out_zones {
                if keep_out.enabled {
                    keep_out_footprints.push(keep_out.footprint());
                }
            }
        }

        // Clone boundary config before we lose the borrow on tc
        let boundary_config = tc.boundary.clone();

        // ── Setup transforms ──────────────────────────────────────────
        // When a setup has non-identity face_up or z_rotation, transform
        // mesh and polygons into setup-local coordinates (matching the GUI
        // compute path).
        if needs_transform {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(
                    self.transform_mesh_to_setup(raw_mesh, face_up, z_rotation),
                ));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(self.transform_polygons_to_setup(
                    raw_polygons,
                    face_up,
                    z_rotation,
                )));
            }
            // Transform keep-out footprints into setup-local frame
            if !keep_out_footprints.is_empty() {
                keep_out_footprints =
                    self.transform_polygons_to_setup(&keep_out_footprints, face_up, z_rotation);
            }
        }

        // Build effective stock bbox in setup-local coordinates
        let effective_stock_bbox = self.effective_stock_bbox_with_rotation(face_up, z_rotation);

        // Resolve heights — use the transformed mesh bbox so Z values are in the
        // setup-local frame.
        let model_bbox = mesh.as_ref().map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z: self.post.safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: effective_stock_bbox.max.z,
            stock_bottom_z: effective_stock_bbox.min.z,
            model_top_z: model_bbox.map(|b| b.max.z),
            model_bottom_z: model_bbox.map(|b| b.min.z),
        };
        let heights = tc.heights.resolve(&height_ctx);

        // Build tool definition
        let tool_def = build_cutter(&tool);

        // Build spatial index for 3D ops
        let spatial_index = mesh
            .as_ref()
            .map(|m| crate::mesh::SpatialIndex::build_auto(m));

        // Create recorders
        let debug_recorder = ToolpathDebugRecorder::new(tc.name.clone(), tc.operation.label());
        let semantic_recorder =
            ToolpathSemanticRecorder::new(tc.name.clone(), tc.operation.label());
        let debug_root = debug_recorder.root_context();
        let semantic_root = semantic_recorder.root_context();

        let core_scope = debug_root.start_span("core_generate", tc.operation.label());
        let core_ctx = core_scope.context();

        let op_label = tc.operation.label().to_owned();

        // Compute cutting levels from the operation config (empty for 3D ops,
        // actual depth levels for 2D ops like Profile, Pocket, Adaptive, etc.)
        let cutting_levels = tc.operation.cutting_levels(heights.top_z);

        // For Rest machining, resolve prev_tool_radius from the RestConfig's
        // prev_tool_id, matching the GUI compute path.
        let prev_tool_radius = if let crate::compute::OperationConfig::Rest(ref cfg) = tc.operation
        {
            cfg.prev_tool_id.and_then(|prev_id| {
                self.tools
                    .iter()
                    .find(|t| t.id == prev_id)
                    .map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        // Execute the operation via the shared compute::execute module (annotated variant)
        let tp_result = crate::compute::execute::execute_operation_annotated(
            &tc.operation,
            mesh.as_deref(),
            spatial_index.as_ref(),
            polygons.as_deref().map(|v| v.as_slice()),
            &tool_def,
            &tool,
            &heights,
            &cutting_levels,
            &effective_stock_bbox,
            prev_tool_radius,
            Some(&core_ctx),
            cancel,
            None, // no initial_stock for session path
        );

        match tp_result {
            Ok(annotated) => {
                let mut toolpath = annotated.toolpath;
                let annotations = annotated.annotations;

                if !toolpath.moves.is_empty() {
                    core_scope.set_move_range(0, toolpath.moves.len().saturating_sub(1));
                }
                drop(core_scope);

                // Build semantic trace from runtime annotations
                let op_scope = semantic_root.start_item(ToolpathSemanticKind::Operation, &op_label);
                if !toolpath.moves.is_empty() {
                    op_scope.bind_to_toolpath(&toolpath, 0, toolpath.moves.len());
                }
                let child_ctx = op_scope.context();
                annotate_from_runtime_events(&annotations, &toolpath, &child_ctx);

                // Apply dressups
                toolpath = crate::compute::execute::apply_dressups(
                    toolpath,
                    &tc.dressups,
                    tool.diameter,
                    heights.retract_z,
                    None,
                    None,
                    None,
                );

                // ── Boundary clipping ─────────────────────────────────
                // After dressups, clip the toolpath to the machining boundary
                // (matching the GUI compute path).
                if boundary_config.enabled {
                    toolpath = Self::apply_boundary_clip(
                        toolpath,
                        &boundary_config,
                        &effective_stock_bbox,
                        &keep_out_footprints,
                        tool.diameter,
                        heights.retract_z,
                        &semantic_root,
                    );
                }

                let stats = ToolpathStats {
                    move_count: toolpath.moves.len(),
                    cutting_distance: toolpath.total_cutting_distance(),
                    rapid_distance: toolpath.total_rapid_distance(),
                };

                let mut debug_trace = debug_recorder.finish();
                let mut semantic_trace = semantic_recorder.finish();
                enrich_traces(&mut debug_trace, &mut semantic_trace);

                self.results.insert(
                    index,
                    ToolpathComputeResult {
                        toolpath: Arc::new(toolpath),
                        stats,
                        debug_trace: Some(debug_trace),
                        semantic_trace: Some(semantic_trace),
                    },
                );
                // SAFETY: we just inserted at this key
                #[allow(clippy::indexing_slicing)]
                Ok(&self.results[&index])
            }
            Err(e) => {
                drop(core_scope);
                let _ = debug_recorder.finish();
                let _ = semantic_recorder.finish();
                Err(SessionError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Apply boundary clipping to a toolpath, subtracting keep-out footprints.
    fn apply_boundary_clip(
        toolpath: crate::toolpath::Toolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        semantic_ctx: &crate::semantic_trace::ToolpathSemanticContext,
    ) -> crate::toolpath::Toolpath {
        use crate::boundary::{
            ToolContainment, clip_toolpath_to_boundary, effective_boundary, subtract_keepouts,
        };

        // Build the boundary polygon from stock bbox (setup-local coordinates).
        let mut stock_poly = crate::polygon::Polygon2::rectangle(
            stock_bbox.min.x,
            stock_bbox.min.y,
            stock_bbox.max.x,
            stock_bbox.max.y,
        );

        // Subtract keep-out footprints (fixtures + keep-out zones).
        if !keep_out_footprints.is_empty() {
            stock_poly = subtract_keepouts(&stock_poly, keep_out_footprints);
        }

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        let tool_radius = tool_diameter / 2.0;
        let boundaries = effective_boundary(&stock_poly, containment, tool_radius);
        if let Some(boundary) = boundaries.first() {
            let clipped = clip_toolpath_to_boundary(&toolpath, boundary, safe_z);

            // Record semantic trace for boundary clip
            let clip_scope =
                semantic_ctx.start_item(ToolpathSemanticKind::BoundaryClip, "Boundary clip");
            clip_scope.set_param(
                "containment",
                match boundary_config.containment {
                    crate::compute::config::BoundaryContainment::Center => "center",
                    crate::compute::config::BoundaryContainment::Inside => "inside",
                    crate::compute::config::BoundaryContainment::Outside => "outside",
                },
            );
            clip_scope.set_param("keep_out_count", keep_out_footprints.len());
            if !clipped.moves.is_empty() {
                clip_scope.bind_to_toolpath(&clipped, 0, clipped.moves.len());
            }

            clipped
        } else {
            // Boundary collapsed (e.g. tool too large for stock) — return original
            toolpath
        }
    }

    /// Generate all enabled toolpaths, skipping those whose IDs are in `skip`.
    pub fn generate_all(
        &mut self,
        skip_ids: &[usize],
        cancel: &AtomicBool,
    ) -> Result<(), SessionError> {
        // Collect info needed for skip/logging before mutable borrow
        let tp_info: Vec<(usize, usize, String, bool)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| (idx, tc.id, tc.name.clone(), tc.enabled))
            .collect();

        for (idx, tp_id, tp_name, enabled) in &tp_info {
            if !enabled {
                continue;
            }
            if skip_ids.contains(tp_id) {
                tracing::info!(id = tp_id, name = %tp_name, "Skipping toolpath (skip list)");
                continue;
            }
            match self.generate_toolpath(*idx, cancel) {
                Ok(_) => {}
                Err(SessionError::MissingGeometry(msg)) => {
                    tracing::warn!(id = tp_id, name = %tp_name, reason = %msg, "Skipping toolpath");
                }
                Err(e) => {
                    tracing::error!(id = tp_id, name = %tp_name, error = %e, "Toolpath failed");
                }
            }
        }
        Ok(())
    }

    // ── Analysis ───────────────────────────────────────────────────

    /// Run tri-dexel stock simulation over all computed toolpaths.
    pub fn run_simulation(
        &mut self,
        opts: &SimulationOptions,
        cancel: &AtomicBool,
    ) -> Result<&super::SimulationResult, SessionError> {
        let stock_bbox = self.stock_bbox();

        // Build simulation groups from setups
        let mut groups = Vec::new();
        for setup in &self.setups {
            let direction = match setup.face_up {
                FaceUp::Bottom => StockCutDirection::FromBottom,
                _ => StockCutDirection::FromTop,
            };

            let mut entries = Vec::new();
            for &tp_idx in &setup.toolpath_indices {
                if let Some(result) = self.results.get(&tp_idx) {
                    let Some(tc) = self.toolpath_configs.get(tp_idx) else {
                        continue;
                    };
                    if opts.skip_ids.contains(&tc.id) {
                        continue;
                    }
                    if result.toolpath.moves.len() < 2 {
                        continue;
                    }

                    let tool_config = self.find_tool_by_raw_id(tc.tool_id);
                    let flute_count = tool_config.map(|t| t.flute_count).unwrap_or(2);
                    let tool_summary = tool_config
                        .map(|t| t.summary())
                        .unwrap_or_else(|| "Unknown".to_owned());
                    let tool_def = tool_config.map(build_cutter).unwrap_or_else(|| {
                        build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill))
                    });

                    entries.push(SimToolpathEntry {
                        id: tc.id,
                        name: tc.name.clone(),
                        toolpath: Arc::clone(&result.toolpath),
                        tool: tool_def,
                        flute_count,
                        tool_summary,
                        semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
                    });
                }
            }

            if !entries.is_empty() {
                // Compute per-setup local stock bbox and transform info.
                let z_rotation = setup.z_rotation;
                let (eff_w, eff_d, eff_h) = {
                    let (w, d, h) =
                        setup
                            .face_up
                            .effective_stock(self.stock.x, self.stock.y, self.stock.z);
                    z_rotation.effective_stock(w, d, h)
                };
                let local_stock_bbox = Some(BoundingBox3 {
                    min: P3::new(0.0, 0.0, 0.0),
                    max: P3::new(eff_w, eff_d, eff_h),
                });
                let local_to_global =
                    if setup.face_up != FaceUp::Top || z_rotation != ZRotation::Deg0 {
                        Some(SetupTransformInfo {
                            face_up: setup.face_up,
                            z_rotation,
                            stock_x: self.stock.x,
                            stock_y: self.stock.y,
                            stock_z: self.stock.z,
                        })
                    } else {
                        None
                    };

                groups.push(SimGroupEntry {
                    toolpaths: entries,
                    direction,
                    local_stock_bbox,
                    local_to_global,
                });
            }
        }

        // Compute effective resolution: auto-resolution matches the GUI's
        // heuristic (5 cells across the smallest tool radius, clamped to
        // [0.02, 0.5] mm, further capped so the grid stays under ~8M cells).
        let resolution = if opts.auto_resolution {
            auto_resolution_for_groups(&groups, &stock_bbox)
        } else {
            opts.resolution
        };

        let request = SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution,
            metric_options: SimulationMetricOptions {
                enabled: opts.metrics_enabled,
            },
            spindle_rpm: self.post.spindle_speed,
            rapid_feed_mm_min: if self.post.high_feedrate_mode {
                self.post.high_feedrate
            } else {
                self.machine.max_feed_mm_min.max(1.0)
            },
            model_mesh: self.models.iter().find_map(|m| m.mesh.clone()),
        };

        let result = run_simulation(&request, cancel)?;
        self.simulation = Some(result);
        // SAFETY: we just assigned Some
        #[allow(clippy::unwrap_used)]
        Ok(self.simulation.as_ref().unwrap())
    }

    /// Run a collision check for a specific toolpath by index.
    pub fn collision_check(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<CollisionCheckResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let model = self
            .find_model_by_raw_id(tc.model_id)
            .and_then(|m| m.mesh.as_ref())
            .ok_or_else(|| {
                SessionError::MissingGeometry("Collision check requires a 3D mesh".to_owned())
            })?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);

        let request = CollisionCheckRequest {
            toolpath: &result.toolpath,
            tool: tool_def,
            mesh: model,
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// Compute project diagnostics from current results and simulation.
    ///
    /// Rapid collision counts come from the simulation result (which checks
    /// against the actual remaining stock surface). If no simulation has
    /// been run, rapid collision counts are 0 — we don't fall back to the
    /// inaccurate original-bbox check.
    pub fn diagnostics(&self) -> ProjectDiagnostics {
        let mut per_toolpath = Vec::new();
        let mut total_collision_count: usize = 0;
        let mut total_rapid_collision_count: usize = 0;
        let no_cancel = AtomicBool::new(false);

        // Build per-boundary rapid collision counts from the simulation result.
        // Each boundary maps to one toolpath via its id.
        let rapid_counts_by_boundary: Vec<(usize, usize)> = if let Some(sim) = &self.simulation {
            sim.boundaries
                .iter()
                .map(|b| {
                    let count = sim
                        .rapid_collision_move_indices
                        .iter()
                        .filter(|&&mi| mi >= b.start_move && mi < b.end_move)
                        .count();
                    (b.id, count)
                })
                .collect()
        } else {
            Vec::new()
        };

        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                // Look up rapid collision count from simulation boundaries.
                let rapid_count = rapid_counts_by_boundary
                    .iter()
                    .filter(|(id, _)| *id == idx)
                    .map(|(_, count)| *count)
                    .sum::<usize>();
                total_rapid_collision_count += rapid_count;

                // Run holder/shank collision check; gracefully default to 0
                // if model geometry is missing or check otherwise fails.
                let holder_collision_count = self
                    .collision_check(idx, &no_cancel)
                    .map(|r| r.collision_report.collisions.len())
                    .unwrap_or(0);
                total_collision_count += holder_collision_count;

                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();

                per_toolpath.push(ToolpathDiagnostic {
                    toolpath_id: tc.id,
                    name: tc.name.clone(),
                    operation_type: tc.operation.label().to_owned(),
                    tool_name,
                    move_count: result.stats.move_count,
                    cutting_distance_mm: result.stats.cutting_distance,
                    rapid_distance_mm: result.stats.rapid_distance,
                    collision_count: holder_collision_count,
                    rapid_collision_count: rapid_count,
                });
            }
        }

        // Extract simulation metrics if available
        let (total_runtime_s, air_cut_percentage, average_engagement) =
            if let Some(sim) = &self.simulation {
                if let Some(trace) = &sim.cut_trace {
                    let summary = &trace.summary;
                    let air_pct = if summary.total_runtime_s > 0.0 {
                        summary.air_cut_time_s / summary.total_runtime_s * 100.0
                    } else {
                        0.0
                    };
                    (summary.total_runtime_s, air_pct, summary.average_engagement)
                } else {
                    (0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0)
            };

        let verdict = if total_collision_count > 0 {
            format!(
                "ERROR: {} holder/shank collisions detected",
                total_collision_count
            )
        } else if total_rapid_collision_count > 0 {
            format!(
                "WARNING: {} rapid-through-stock collisions",
                total_rapid_collision_count
            )
        } else if air_cut_percentage > 40.0 {
            format!("WARNING: {air_cut_percentage:.1}% air cutting")
        } else {
            "OK".to_owned()
        };

        ProjectDiagnostics {
            total_runtime_s,
            air_cut_percentage,
            average_engagement,
            collision_count: total_collision_count,
            rapid_collision_count: total_rapid_collision_count,
            per_toolpath,
            verdict,
        }
    }

    // ── Export ──────────────────────────────────────────────────────

    /// Export G-code for all computed toolpaths.
    pub fn export_gcode(&self, path: &Path, _setup_id: Option<usize>) -> Result<(), SessionError> {
        use crate::compute::{CompensationType, OperationConfig};
        use crate::gcode::{
            ControllerCompensation, CoolantMode, GcodePhase, PostFormat, emit_gcode_phased,
        };
        use crate::profile::ProfileSide;

        let post_format = match self.post.format.to_ascii_lowercase().as_str() {
            "linuxcnc" | "linux_cnc" => PostFormat::LinuxCnc,
            "mach3" => PostFormat::Mach3,
            _ => PostFormat::Grbl,
        };
        let post = post_format.post_processor();

        // Collect all computed toolpaths as phases
        let mut phases: Vec<GcodePhase<'_>> = Vec::new();
        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                // Determine controller compensation for profile operations
                let controller_compensation =
                    if let OperationConfig::Profile(ref cfg) = tc.operation {
                        if cfg.compensation == CompensationType::InControl {
                            Some(match (cfg.side, cfg.climb) {
                                (ProfileSide::Outside, true) => ControllerCompensation::Right,
                                (ProfileSide::Outside, false) => ControllerCompensation::Left,
                                (ProfileSide::Inside, true) => ControllerCompensation::Left,
                                (ProfileSide::Inside, false) => ControllerCompensation::Right,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                phases.push(GcodePhase {
                    toolpath: &result.toolpath,
                    spindle_rpm: self.post.spindle_speed,
                    label: &tc.name,
                    tool_number: None,
                    coolant: CoolantMode::Off,
                    pre_gcode: tc.pre_gcode.as_deref(),
                    post_gcode: tc.post_gcode.as_deref(),
                    controller_compensation,
                });
            }
        }

        let gcode = emit_gcode_phased(&phases, post.as_ref());
        std::fs::write(path, gcode).map_err(|e| {
            SessionError::Export(format!("Failed to write G-code to {}: {e}", path.display()))
        })
    }

    /// Export diagnostics as JSON files to an output directory.
    pub fn export_diagnostics_json(&self, output_dir: &Path) -> Result<(), SessionError> {
        std::fs::create_dir_all(output_dir)?;
        let diag = self.diagnostics();
        let json = serde_json::to_string_pretty(&diag)
            .map_err(|e| SessionError::Export(format!("Failed to serialize diagnostics: {e}")))?;
        let path = output_dir.join("summary.json");
        std::fs::write(&path, json)
            .map_err(|e| SessionError::Export(format!("Failed to write {}: {e}", path.display())))
    }
}

/// Compute auto-resolution from simulation groups and stock bbox.
///
/// Mirrors the GUI's `auto_resolution_for_tools` heuristic:
/// - 5 cells across the smallest tool radius for decent curve resolution
/// - Clamped to [0.02, 0.5] mm
/// - Further limited so the grid stays under ~8M cells
fn auto_resolution_for_groups(groups: &[SimGroupEntry], stock_bbox: &BoundingBox3) -> f64 {
    use crate::tool::MillingCutter as _;

    let min_radius = groups
        .iter()
        .flat_map(|g| g.toolpaths.iter())
        .map(|entry| entry.tool.radius())
        .fold(f64::INFINITY, f64::min);

    // 5 cells across the radius gives decent curve resolution
    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    // Cap so grid stays under ~8M cells (reasonable memory / mesh size)
    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    from_tool.max(from_grid)
}
