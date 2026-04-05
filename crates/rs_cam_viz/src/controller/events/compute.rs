use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::BoundingBox3;

use crate::compute::{ComputeBackend, ComputeError, ComputeMessage, ComputeRequest};
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
use crate::state::toolpath::{ComputeStatus, OperationConfig, ToolpathId};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub(crate) fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((
            tool_id,
            model_id,
            mut operation,
            dressups,
            heights_config,
            stock_source,
            toolpath_name,
            boundary_enabled,
            boundary_containment,
            debug_options,
            face_selection_for_toolpath,
        )) = self.state.job.find_toolpath(tp_id).map(|toolpath| {
            (
                toolpath.tool_id,
                toolpath.model_id,
                toolpath.operation.clone(),
                toolpath.dressups.clone(),
                toolpath.heights.clone(),
                toolpath.stock_source,
                toolpath.name.clone(),
                toolpath.boundary_enabled,
                toolpath.boundary_containment,
                toolpath.debug_options,
                toolpath.face_selection.clone(),
            )
        })
        else {
            return;
        };

        let Some(tool) = self
            .state
            .job
            .tools
            .iter()
            .find(|tool| tool.id == tool_id)
            .cloned()
        else {
            self.push_notification(
                "Cannot generate: no tool assigned to this toolpath".into(),
                super::super::Severity::Warning,
            );
            return;
        };

        // Run the same validation the UI uses so both paths are consistent.
        {
            let validation =
                crate::ui::properties::ToolpathValidationContext::from_job(&self.state.job);
            if let Some(entry) = self.state.job.find_toolpath(tp_id) {
                let errs = crate::ui::properties::validate_toolpath(entry, &validation);
                if !errs.is_empty() {
                    if let Some(tp) = self.state.job.find_toolpath_mut(tp_id) {
                        tp.status = ComputeStatus::Error(errs.join("; "));
                    }
                    return;
                }
            }
        }

        let setup_ref = self
            .state
            .job
            .setups
            .iter()
            .find(|setup| setup.toolpaths.iter().any(|toolpath| toolpath.id == tp_id));
        let mut keep_out_footprints = setup_ref
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
        let transform_setup = setup_ref.map(|setup| {
            let mut transform_setup = crate::state::job::Setup::new(setup.id, setup.name.clone());
            transform_setup.face_up = setup.face_up;
            transform_setup.z_rotation = setup.z_rotation;
            transform_setup
        });
        let stock_snapshot = self.state.job.stock.clone();

        let model = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == model_id);
        let mut polygons = model.and_then(|model| model.polygons.clone());
        let mut mesh = model.and_then(|model| model.mesh.clone());
        let enriched_mesh = model.and_then(|model| model.enriched_mesh.clone());
        let face_selection = face_selection_for_toolpath;

        // ProjectCurve: use a separate model's mesh for the 3D surface when configured.
        if let OperationConfig::ProjectCurve(ref cfg) = operation
            && let Some(surface_id) = cfg.surface_model_id
        {
            mesh = self
                .state
                .job
                .models
                .iter()
                .find(|m| m.id == surface_id)
                .and_then(|m| m.mesh.clone());
        }

        // Derive polygons from selected BREP faces when no explicit polygons exist.
        // This enables all 2.5D operations (pocket, profile, adaptive, trace, etc.)
        // to work with STEP models by extracting face boundary loops as Polygon2.
        // Also extract the face Z height to set the toolpath top_z correctly.
        let mut face_top_z: Option<f64> = None;
        if polygons.is_none()
            && let (Some(face_ids), Some(enriched)) = (&face_selection, &enriched_mesh)
            && !face_ids.is_empty()
        {
            if let Some(poly) = enriched.faces_boundary_as_polygon(face_ids) {
                polygons = Some(Arc::new(vec![poly]));
                // Extract the Z height from the selected faces' bounding boxes.
                // For horizontal planar faces, bbox.min.z ≈ bbox.max.z ≈ face Z.
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
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status =
                    ComputeStatus::Error("No 3D mesh (import STL or STEP)".to_owned());
            }
            return;
        }
        if !is_3d && !operation.is_stock_based() && polygons.is_none() {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status = ComputeStatus::Error(
                    "No 2D geometry (import SVG/DXF or select STEP faces)".to_owned(),
                );
            }
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &operation {
            config.prev_tool_id.and_then(|prev_tool_id| {
                self.state
                    .job
                    .tools
                    .iter()
                    .find(|tool| tool.id == prev_tool_id)
                    .map(|tool| tool.diameter / 2.0)
            })
        } else {
            None
        };

        // Refresh pin drill holes from current stock state before submitting.
        if let OperationConfig::AlignmentPinDrill(ref mut cfg) = operation {
            cfg.holes = self
                .state
                .job
                .stock
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
        }

        if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
            toolpath.status = ComputeStatus::Computing;
            toolpath.result = None;
            toolpath.debug_trace = None;
            toolpath.semantic_trace = None;
            toolpath.debug_trace_path = None;
        }

        let safe_z = self.state.job.post.safe_z;

        // Compute setup-local stock bbox FIRST so heights resolve in the correct frame.
        let stock_bbox = if let Some(transform_setup) = transform_setup.as_ref() {
            let (width, depth, height) = transform_setup.effective_stock(&stock_snapshot);
            BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(width, depth, height),
            }
        } else {
            self.state.job.stock.bbox()
        };

        let model_bb = self
            .state
            .job
            .models
            .iter()
            .find(|m| m.id == model_id)
            .and_then(|m| m.bbox());
        // Transform model bbox into setup-local frame when a setup exists.
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
        // When face selection provides a Z height, use it as the top_z
        // so the toolpath cuts at the face level, not at Z=0.
        if let Some(fz) = face_top_z
            && heights_config.top_z.is_auto()
        {
            heights.top_z = fz;
            // Shift bottom_z relative to the face top
            if heights_config.bottom_z.is_auto() {
                heights.bottom_z = fz - operation.default_depth_for_heights().abs();
            }
        }

        // TODO: build_prior_stock for FromRemainingStock requires tri-dexel
        // simulation of prior toolpaths — not yet implemented.
        let prior_stock: Option<TriDexelStock> = None;

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
            boundary_enabled,
            boundary_containment,
            keep_out_footprints,
            heights,
            prior_stock,
        });
    }

    /// Build a TriDexelStock representing the remaining material after simulating
    /// all prior enabled toolpaths (those that appear before `tp_id`) in the same
    /// setup.  Returns `None` when there are no prior results to simulate.
    // SAFETY: tp_index from position() within setup.toolpaths, slice always in bounds
    #[allow(clippy::indexing_slicing)]
    pub(crate) fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(result.toolpath_id) {
                        toolpath.debug_trace = result.debug_trace.clone();
                        toolpath.semantic_trace = result.semantic_trace.clone();
                        toolpath.debug_trace_path = result.debug_trace_path.clone();
                        match result.result {
                            Ok(computed) => {
                                toolpath.status = ComputeStatus::Done;
                                toolpath.result = Some(computed);
                            }
                            Err(ComputeError::Cancelled) => {
                                toolpath.status = ComputeStatus::Pending;
                                toolpath.result = None;
                            }
                            Err(ComputeError::Message(error)) => {
                                toolpath.status = ComputeStatus::Error(error);
                                toolpath.result = None;
                            }
                        }
                    }
                    self.pending_upload = true;
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        // Warn user if resolution was silently coarsened
                        if simulation.resolution_clamped {
                            self.push_notification(
                                "Sim resolution was coarsened to fit grid limits — \
                                 consider reducing stock size or increasing resolution"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        // Warn if mesh is empty (would render as blank)
                        if simulation.mesh.indices.is_empty() {
                            self.push_notification(
                                "Simulation produced an empty mesh — \
                                 try increasing resolution or check stock dimensions"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        // Build boundaries
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

                        // Build setup boundaries
                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.state.job.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .job
                                            .setups
                                            .iter()
                                            .find(|setup| setup.id == setup_id)
                                            .map(|setup| setup.name.clone())
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

                        // Build checkpoints
                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                stock: Some(checkpoint.stock),
                            })
                            .collect();

                        // Store rapid collision data in checks
                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        // Cache deviations for viz mode re-coloring.
                        // display_mesh starts as None — the first playback frame
                        // will fill it in from the live stock, showing progressive
                        // cutting from the uncut block.
                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = None;

                        // Global stock bbox (stock-relative, origin at 0,0,0)
                        let stock_bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                            max: rs_cam_core::geo::P3::new(
                                self.state.job.stock.x,
                                self.state.job.stock.y,
                                self.state.job.stock.z,
                            ),
                        };

                        // Store results as cached artifact
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
                            // Start playback from the beginning so the user sees
                            // the tool progressively cutting the uncut block.
                            self.state.simulation.playback.current_move = 0;
                            self.state.simulation.playback.playing = true;
                        }

                        // Store fresh tri-dexel stock for playback (global frame)
                        let initial_stock = TriDexelStock::from_bounds(
                            &stock_bbox,
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_stock = Some(initial_stock);
                        self.state.simulation.playback.live_sim_move = 0;

                        // Update staleness metadata
                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: self.state.job.edit_counter,
                        });

                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Simulation failed: {error}");
                        self.push_notification(
                            format!("Simulation failed: {error}"),
                            super::super::Severity::Error,
                        );
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
                        // Wire results into simulation checks state
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        self.state.simulation.checks.collision_report = Some(collision.report);
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Collision check failed: {error}");
                        self.push_notification(
                            format!("Collision check failed: {error}"),
                            super::super::Severity::Error,
                        );
                    }
                },
            }
        }
    }
}
