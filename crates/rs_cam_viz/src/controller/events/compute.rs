use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::BoundingBox3;

use crate::compute::{ComputeBackend, ComputeError, ComputeMessage, ComputeRequest};
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
use crate::state::toolpath::{ComputeStatus, OperationConfig, ToolpathId};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub(crate) fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((tp_idx, tc)) = self.state.session.find_toolpath_config_by_id(tp_id.0) else {
            return;
        };

        let tool_id_raw = tc.tool_id;
        let model_id_raw = tc.model_id;
        let mut operation = tc.operation.clone();
        let dressups = tc.dressups.clone();
        let heights_config = tc.heights.clone();
        let stock_source = tc.stock_source;
        let toolpath_name = tc.name.clone();
        let boundary = tc.boundary.clone();
        let debug_options = tc.debug_options;
        let face_selection_for_toolpath = tc.face_selection.clone();

        let Some(tool) = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id.0 == tool_id_raw)
            .cloned()
        else {
            self.push_notification(
                "Cannot generate: no tool assigned to this toolpath".into(),
                super::super::Severity::Warning,
            );
            return;
        };

        // Run validation
        {
            let validation =
                crate::ui::properties::ToolpathValidationContext::from_session(&self.state.session);
            if let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(tp_id.0) {
                let errs = crate::ui::properties::validate_toolpath_config(tc, &validation);
                if !errs.is_empty() {
                    if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                        rt.status = ComputeStatus::Error(errs.join("; "));
                    }
                    return;
                }
            }
        }

        // Find the setup that contains this toolpath
        let setup_data = self
            .state
            .session
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_idx));

        let mut keep_out_footprints = setup_data
            .map(|setup| {
                let mut footprints = Vec::new();
                for fixture in &setup.fixtures {
                    if fixture.enabled {
                        footprints.push(fixture.footprint());
                    }
                }
                for keep_out in &setup.keep_out_zones {
                    if keep_out.enabled {
                        footprints.push(keep_out.footprint());
                    }
                }
                footprints
            })
            .unwrap_or_default();

        // Build a lightweight "setup" for transforms using session types
        let transform_setup = setup_data.map(|setup| {
            crate::state::job::Setup::new(crate::state::job::SetupId(setup.id), setup.name.clone())
        });
        // If there's a real setup, copy face_up and z_rotation
        let transform_setup = match (transform_setup, setup_data) {
            (Some(mut s), Some(sd)) => {
                s.face_up = sd.face_up;
                s.z_rotation = sd.z_rotation;
                Some(s)
            }
            _ => None,
        };

        // Flag project_curve when the setup Z is already inverted (bottom-facing).
        let needs_transform = transform_setup.is_some();
        let setup_is_bottom = setup_data
            .is_some_and(|s| s.face_up == rs_cam_core::compute::transform::FaceUp::Bottom);
        if let OperationConfig::ProjectCurve(ref mut cfg) = operation {
            cfg.setup_z_flipped = setup_is_bottom && needs_transform;
        }

        let stock_snapshot = self.state.session.stock_config().clone();

        let model = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id_raw);
        let mut polygons = model.and_then(|m| m.polygons.clone());
        let mut mesh = model.and_then(|m| m.mesh.clone());
        let enriched_mesh = model.and_then(|m| m.enriched_mesh.clone());
        let face_selection = face_selection_for_toolpath;

        // ProjectCurve: use a separate model's mesh for the 3D surface when configured.
        if let OperationConfig::ProjectCurve(ref cfg) = operation
            && let Some(surface_id) = cfg.surface_model_id
        {
            mesh = self
                .state
                .session
                .models()
                .iter()
                .find(|m| m.id == surface_id.0)
                .and_then(|m| m.mesh.clone());
        }

        // Derive polygons from selected BREP faces when no explicit polygons exist.
        let mut face_top_z: Option<f64> = None;
        if polygons.is_none()
            && let (Some(face_ids), Some(enriched)) = (&face_selection, &enriched_mesh)
            && !face_ids.is_empty()
        {
            if let Some(poly) = enriched.faces_boundary_as_polygon(face_ids) {
                polygons = Some(Arc::new(vec![poly]));
                let z = face_ids
                    .iter()
                    .filter_map(|fid| enriched.face_group(*fid))
                    .map(|fg| fg.bbox.max.z)
                    .fold(f64::NEG_INFINITY, f64::max);
                if z.is_finite() {
                    face_top_z = Some(z);
                }
            } else {
                tracing::warn!(
                    "Selected faces did not produce a boundary polygon (non-horizontal or non-planar)"
                );
                self.status_message = Some((
                    "Face selection ignored: selected faces are not horizontal planes".to_owned(),
                    std::time::Instant::now(),
                ));
            }
        }

        if let Some(transform_setup) = transform_setup.as_ref() {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(crate::state::job::transform_mesh(
                    raw_mesh,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(crate::state::job::transform_polygons(
                    raw_polygons,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            keep_out_footprints = crate::state::job::transform_polygons(
                &keep_out_footprints,
                transform_setup,
                &stock_snapshot,
            );
        }

        let is_3d = operation.is_3d();
        if is_3d && mesh.is_none() {
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                rt.status = ComputeStatus::Error("No 3D mesh (import STL or STEP)".to_owned());
            }
            return;
        }
        if !is_3d && !operation.is_stock_based() && polygons.is_none() {
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                rt.status = ComputeStatus::Error(
                    "No 2D geometry (import SVG/DXF or select STEP faces)".to_owned(),
                );
            }
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &operation {
            config.prev_tool_id.and_then(|prev_tool_id| {
                self.state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id == prev_tool_id)
                    .map(|t| t.diameter / 2.0)
            })
        } else {
            None
        };

        // Refresh pin drill holes from current stock state before submitting.
        if let OperationConfig::AlignmentPinDrill(ref mut cfg) = operation {
            cfg.holes = self
                .state
                .session
                .stock_config()
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
        }

        // Update GUI runtime status
        let rt = self.state.gui.toolpath_rt_or_default(tp_id.0);
        rt.status = ComputeStatus::Computing;
        rt.result = None;
        rt.debug_trace = None;
        rt.semantic_trace = None;
        rt.debug_trace_path = None;

        let safe_z = self.state.gui.post.safe_z;

        // Compute setup-local stock bbox FIRST so heights resolve in the correct frame.
        let stock_bbox = if let Some(transform_setup) = transform_setup.as_ref() {
            let (width, depth, height) = transform_setup.effective_stock(&stock_snapshot);
            BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(width, depth, height),
            }
        } else {
            stock_snapshot.bbox()
        };

        let model_bb = self
            .state
            .session
            .models()
            .iter()
            .find(|m| m.id == model_id_raw)
            .and_then(|m| m.mesh.as_ref().map(|mesh| mesh.bbox));
        let (model_top_z, model_bottom_z) = match (model_bb, transform_setup.as_ref()) {
            (Some(bb), Some(setup)) => {
                let mut min_z = f64::INFINITY;
                let mut max_z = f64::NEG_INFINITY;
                for &x in &[bb.min.x, bb.max.x] {
                    for &y in &[bb.min.y, bb.max.y] {
                        for &z in &[bb.min.z, bb.max.z] {
                            let local = setup.transform_point(
                                rs_cam_core::geo::P3::new(x, y, z),
                                &stock_snapshot,
                            );
                            if local.z < min_z {
                                min_z = local.z;
                            }
                            if local.z > max_z {
                                max_z = local.z;
                            }
                        }
                    }
                }
                (Some(max_z), Some(min_z))
            }
            (Some(bb), None) => (Some(bb.max.z), Some(bb.min.z)),
            _ => (None, None),
        };
        let height_ctx = crate::state::toolpath::HeightContext {
            safe_z,
            op_depth: operation.default_depth_for_heights(),
            stock_top_z: stock_bbox.max.z,
            stock_bottom_z: stock_bbox.min.z,
            model_top_z,
            model_bottom_z,
        };
        let mut heights = heights_config.resolve(&height_ctx);
        if let Some(fz) = face_top_z
            && heights_config.top_z.is_auto()
        {
            heights.top_z = fz;
            if heights_config.bottom_z.is_auto() {
                heights.bottom_z = fz - operation.default_depth_for_heights().abs();
            }
        }

        let prior_stock: Option<TriDexelStock> = None;
        let cutting_levels = operation.cutting_levels(heights.top_z);

        self.compute.submit_toolpath(ComputeRequest {
            toolpath_id: tp_id,
            toolpath_name,
            debug_options,
            polygons,
            mesh,
            enriched_mesh,
            face_selection,
            operation,
            dressups,
            stock_source,
            tool,
            safe_z,
            prev_tool_radius,
            stock_bbox: Some(stock_bbox),
            boundary,
            keep_out_footprints,
            heights,
            cutting_levels,
            prior_stock,
        });
    }

    // SAFETY: tp_index from position() within setup.toolpaths, slice always in bounds
    #[allow(clippy::indexing_slicing)]
    pub(crate) fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    let tp_id = result.toolpath_id;
                    let rt = self.state.gui.toolpath_rt_or_default(tp_id.0);
                    rt.debug_trace = result.debug_trace.clone();
                    rt.semantic_trace = result.semantic_trace.clone();
                    rt.debug_trace_path = result.debug_trace_path.clone();
                    match result.result {
                        Ok(computed) => {
                            rt.status = ComputeStatus::Done;
                            rt.result = Some(computed);
                        }
                        Err(ComputeError::Cancelled) => {
                            rt.status = ComputeStatus::Pending;
                            rt.result = None;
                        }
                        Err(ComputeError::Message(error)) => {
                            rt.status = ComputeStatus::Error(error);
                            rt.result = None;
                        }
                    }
                    self.pending_upload = true;

                    // Notify pending MCP request for this toolpath
                    #[cfg(feature = "mcp")]
                    self.notify_mcp_toolpath_complete(tp_id);
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        if simulation.resolution_clamped {
                            self.push_notification(
                                "Sim resolution was coarsened to fit grid limits — \
                                 consider reducing stock size or increasing resolution"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        if simulation.mesh.indices.is_empty() {
                            self.push_notification(
                                "Simulation produced an empty mesh — \
                                 try increasing resolution or check stock dimensions"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        let boundaries: Vec<_> = simulation
                            .boundaries
                            .iter()
                            .map(|boundary| crate::state::simulation::ToolpathBoundary {
                                id: boundary.id,
                                name: boundary.name.clone(),
                                tool_name: boundary.tool_name.clone(),
                                start_move: boundary.start_move,
                                end_move: boundary.end_move,
                                direction: boundary.direction,
                            })
                            .collect();

                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .session
                                            .list_setups()
                                            .iter()
                                            .find(|s| s.id == setup_id.0)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_default();
                                        sbs.push(crate::state::simulation::SetupBoundary {
                                            setup_id,
                                            setup_name,
                                            start_move: boundary.start_move,
                                        });
                                    }
                                    last_setup_id = setup_id;
                                }
                            }
                            sbs
                        };

                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                stock: Some(checkpoint.stock),
                            })
                            .collect();

                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = None;

                        let stock = self.state.session.stock_config();
                        let stock_bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
                        };

                        self.state.simulation.results = Some(SimulationResults {
                            mesh: simulation.mesh,
                            total_moves: simulation.total_moves,
                            boundaries,
                            setup_boundaries,
                            checkpoints,
                            selected_toolpaths: None,
                            playback_data: simulation.playback_data,
                            stock_bbox,
                            cut_trace: simulation.cut_trace,
                            cut_trace_path: simulation.cut_trace_path,
                        });

                        let inspect_target =
                            self.state.simulation.debug.pending_inspect_toolpath.take();
                        if let Some(move_index) = inspect_target.and_then(|toolpath_id| {
                            self.state
                                .simulation
                                .boundaries()
                                .iter()
                                .find(|boundary| boundary.id == toolpath_id)
                                .map(|boundary| boundary.start_move)
                        }) {
                            self.state.simulation.playback.current_move = move_index;
                            self.state.simulation.playback.playing = false;
                        } else {
                            self.state.simulation.playback.current_move = 0;
                            self.state.simulation.playback.playing = false;
                        }

                        let initial_stock = TriDexelStock::from_bounds(
                            &stock_bbox,
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_stock = Some(initial_stock);
                        self.state.simulation.playback.live_sim_move = 0;

                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: self.state.gui.edit_counter,
                        });

                        self.pending_upload = true;

                        // Notify pending MCP simulation request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_complete();
                    }
                    Err(ComputeError::Cancelled) => {
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error("Simulation cancelled");
                    }
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Simulation failed: {error}");
                        self.push_notification(
                            format!("Simulation failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error(&error);
                    }
                },
                ComputeMessage::Collision(result) => match result {
                    Ok(collision) => {
                        let count = collision.report.collisions.len();
                        if count == 0 {
                            tracing::info!("No holder clearance issues detected");
                            self.push_notification(
                                "No holder clearance issues detected".into(),
                                super::super::Severity::Info,
                            );
                        } else {
                            let msg = format!(
                                "{} holder clearance issues, min safe stickout: {:.1} mm",
                                count, collision.report.min_safe_stickout
                            );
                            tracing::warn!("{msg}");
                            self.push_notification(msg, super::super::Severity::Warning);
                        }
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        // Extract MCP response data before moving ownership
                        #[cfg(feature = "mcp")]
                        let mcp_collision_count = collision.report.collisions.len();
                        #[cfg(feature = "mcp")]
                        let mcp_min_safe_stickout = collision.report.min_safe_stickout;
                        #[cfg(feature = "mcp")]
                        let mcp_is_clear = collision.report.is_clear();

                        self.state.simulation.checks.collision_report = Some(collision.report);
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;

                        // Notify pending MCP collision request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_complete(
                            mcp_collision_count,
                            mcp_min_safe_stickout,
                            mcp_is_clear,
                        );
                    }
                    Err(ComputeError::Cancelled) => {
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error("Collision check cancelled");
                    }
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Collision check failed: {error}");
                        self.push_notification(
                            format!("Collision check failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error(&error);
                    }
                },
            }
        }
    }

    // ── MCP notification helpers ─────────────────────────────────────
    #[cfg(feature = "mcp")]
    fn notify_mcp_toolpath_complete(&mut self, tp_id: crate::state::toolpath::ToolpathId) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp {
            // Check individual toolpath request
            if let Some(sender) = pending.toolpath.remove(&tp_id) {
                let rt = self.state.gui.toolpath_rt.get(&tp_id.0);
                let resp = match rt.and_then(|rt| rt.result.as_ref()) {
                    Some(result) => json_str(serde_json::json!({
                        "id": tp_id.0,
                        "move_count": result.stats.move_count,
                        "cutting_distance_mm": result.stats.cutting_distance,
                        "rapid_distance_mm": result.stats.rapid_distance,
                    })),
                    None => {
                        let status_msg = rt
                            .map(|rt| match &rt.status {
                                ComputeStatus::Error(e) => format!("Error: {e}"),
                                _ => "Toolpath generation produced no result".to_owned(),
                            })
                            .unwrap_or_else(|| "Toolpath not found".to_owned());
                        json_str(serde_json::json!({"error": status_msg}))
                    }
                };
                let _ = sender.send(McpResponse { result: Ok(resp) });
            }

            // Check generate_all tracking
            if let Some(ref mut ga) = pending.generate_all {
                if let Some(pos) = ga.remaining.iter().position(|id| *id == tp_id) {
                    ga.remaining.remove(pos);
                    let rt = self.state.gui.toolpath_rt.get(&tp_id.0);
                    if rt.and_then(|rt| rt.result.as_ref()).is_some() {
                        ga.completed += 1;
                    } else {
                        ga.failed += 1;
                        let error_msg = rt
                            .map(|rt| match &rt.status {
                                crate::state::toolpath::ComputeStatus::Error(e) => e.clone(),
                                _ => "No result produced".to_owned(),
                            })
                            .unwrap_or_else(|| "Toolpath runtime not found".to_owned());
                        ga.errors.push((tp_id.0, error_msg));
                    }

                    // Send progress update via the progress channel (non-blocking).
                    if let Some(ref progress_tx) = ga.progress_tx {
                        let total = (ga.completed + ga.failed + ga.remaining.len()) as f64;
                        let current = (ga.completed + ga.failed) as f64;
                        let tp_name = self
                            .state
                            .session
                            .find_toolpath_config_by_id(tp_id.0)
                            .map(|(_, tc)| tc.name.clone())
                            .unwrap_or_else(|| format!("toolpath {}", tp_id.0));
                        let msg = format!(
                            "Completed {}/{}: {}",
                            current as usize, total as usize, tp_name
                        );
                        let _ = progress_tx.try_send(crate::mcp_bridge::ProgressUpdate {
                            message: msg,
                            progress: current,
                            total: Some(total),
                        });
                    }
                }

                if ga.remaining.is_empty()
                    && let Some(ga) = pending.generate_all.take()
                {
                    let resp = if ga.errors.is_empty() {
                        rs_cam_mcp::server::text(format!("Generated {} toolpaths", ga.completed,))
                    } else {
                        let error_details: Vec<String> = ga
                            .errors
                            .iter()
                            .map(|(id, msg)| format!("  toolpath {id}: {msg}"))
                            .collect();
                        rs_cam_mcp::server::text(format!(
                            "Generated {} toolpaths ({} failed):\n{}",
                            ga.completed,
                            ga.failed,
                            error_details.join("\n"),
                        ))
                    };
                    let _ = ga.response_tx.send(McpResponse { result: Ok(resp) });
                }
            }
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_complete(&mut self) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let resp = self.build_mcp_diagnostics();
            let _ = sender.send(McpResponse {
                result: Ok(json_str(resp)),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_complete(
        &mut self,
        collision_count: usize,
        min_safe_stickout: f64,
        is_clear: bool,
    ) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let resp = json_str(serde_json::json!({
                "collision_count": collision_count,
                "min_safe_stickout_mm": min_safe_stickout,
                "is_clear": is_clear,
            }));
            let _ = sender.send(McpResponse { result: Ok(resp) });
        }
    }

    /// Build diagnostics JSON from GUI state (toolpath_rt + simulation results).
    ///
    /// Unlike `session.diagnostics()` which reads from the session's internal
    /// result cache (only populated by the standalone MCP), this reads from
    /// `gui.toolpath_rt` and `state.simulation.results` — where the GUI's
    /// compute pipeline actually stores data.
    #[cfg(feature = "mcp")]
    pub fn build_mcp_diagnostics(&self) -> serde_json::Value {
        let session = &self.state.session;
        let gui = &self.state.gui;

        let mut per_toolpath = Vec::new();
        for tc in session.toolpath_configs() {
            if let Some(rt) = gui.toolpath_rt.get(&tc.id)
                && let Some(ref result) = rt.result
            {
                let tool_name = session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();

                per_toolpath.push(serde_json::json!({
                    "toolpath_id": tc.id,
                    "name": tc.name,
                    "operation_type": tc.operation.label(),
                    "tool_name": tool_name,
                    "move_count": result.stats.move_count,
                    "cutting_distance_mm": result.stats.cutting_distance,
                    "rapid_distance_mm": result.stats.rapid_distance,
                }));
            }
        }

        let (total_runtime_s, air_cut_pct, avg_engagement) = if let Some(ref sim_results) =
            self.state.simulation.results
            && let Some(ref ct) = sim_results.cut_trace
        {
            let s = &ct.summary;
            let air = if s.total_runtime_s > 0.0 {
                s.air_cut_time_s / s.total_runtime_s * 100.0
            } else {
                0.0
            };
            (s.total_runtime_s, air, s.average_engagement)
        } else {
            (0.0, 0.0, 0.0)
        };

        let rapid_collision_count = self.state.simulation.checks.rapid_collisions.len();

        let verdict = if rapid_collision_count > 0 {
            "WARNING: rapid collisions detected"
        } else if air_cut_pct > 20.0 {
            "WARNING: high air cutting"
        } else {
            "OK"
        };

        let mut resp = serde_json::json!({
            "total_runtime_s": total_runtime_s,
            "air_cut_percentage": air_cut_pct,
            "average_engagement": avg_engagement,
            "collision_count": 0,
            "rapid_collision_count": rapid_collision_count,
            "verdict": verdict,
            "per_toolpath": per_toolpath,
        });

        if let Some(ref sim_results) = self.state.simulation.results
            && let Some(ref ct) = sim_results.cut_trace
        {
            // SAFETY: resp is a known JSON object we just constructed
            #[allow(clippy::indexing_slicing)]
            {
                resp["semantic_summary_count"] = serde_json::json!(ct.semantic_summaries.len());
                resp["hotspot_count"] = serde_json::json!(ct.hotspots.len());
                resp["issue_count"] = serde_json::json!(ct.issues.len());
            }
        }

        resp
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }
}
