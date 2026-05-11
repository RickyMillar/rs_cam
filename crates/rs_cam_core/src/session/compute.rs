//! Compute and mutation methods on [`ProjectSession`].

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::collision_check::{
    CollisionCheckRequest, CollisionCheckResult, run_collision_check,
};
use crate::compute::config::{HeightContext, ToolpathStats};
use crate::compute::cutter::build_cutter;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::debug_trace::ToolpathDebugRecorder;
use crate::dexel_stock::StockCutDirection;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::TriangleMesh;
use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces};
use crate::simulation_cut::SimulationMetricOptions;
use crate::tool::MillingCutter;

use super::{
    ProjectDiagnostics, ProjectSession, SessionError, SimulationOptions, ToolpathComputeResult,
    ToolpathDiagnostic,
};

/// Translate a triangle mesh by (dx, dy, dz) and rebuild bbox/faces.
fn translate_mesh(mesh: &TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    let verts: Vec<P3> = mesh
        .vertices
        .iter()
        .map(|v| P3::new(v.x + dx, v.y + dy, v.z + dz))
        .collect();
    TriangleMesh::from_raw(verts, mesh.triangles.clone())
}

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
    #[instrument(skip(self, value))]
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

        // Numeric fields accept either JSON numbers or numeric strings —
        // some MCP / JSON-RPC clients stringify scalar values when the
        // schema type is permissive (`serde_json::Value`), so this
        // fallback keeps the API resilient.
        let as_number = |v: &serde_json::Value| -> Option<f64> {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        };

        match param {
            "feed_rate" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("feed_rate must be a number".to_owned())
                })?;
                tc.operation.set_feed_rate(v);
            }
            "plunge_rate" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("plunge_rate must be a number".to_owned())
                })?;
                tc.operation.set_plunge_rate(v);
            }
            "stepover" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("stepover must be a number".to_owned())
                })?;
                tc.operation.set_stepover(v);
            }
            "depth_per_pass" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("depth_per_pass must be a number".to_owned())
                })?;
                tc.operation.set_depth_per_pass(v);
            }
            "spindle_rpm" => {
                // Accept Null, integer, integer-valued float, or numeric string.
                // MCP / JSON-RPC clients vary in how they encode numerics —
                // some always serialize as f64 (so 13500 arrives as 13500.0),
                // others stringify when the schema's `value` is permissive.
                // The router is the right place to absorb the friction; we
                // reject only on actual loss of precision or out-of-range.
                let rpm = match &value {
                    serde_json::Value::Null => None,
                    other => {
                        let f = as_number(other).ok_or_else(|| {
                            SessionError::InvalidParam(
                                "spindle_rpm must be a non-negative integer or null".to_owned(),
                            )
                        })?;
                        if !f.is_finite() || f < 0.0 || f > f64::from(u32::MAX) {
                            return Err(SessionError::InvalidParam(format!(
                                "spindle_rpm out of range: {f}"
                            )));
                        }
                        if f.fract() != 0.0 {
                            return Err(SessionError::InvalidParam(format!(
                                "spindle_rpm must be a whole number: {f}"
                            )));
                        }
                        // SAFETY: bounds + finite + integer-valued checked above.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        Some(f as u32)
                    }
                };
                tc.operation.set_spindle_rpm(rpm);
            }
            "debug_enabled" => {
                let v = match &value {
                    serde_json::Value::Bool(b) => *b,
                    serde_json::Value::Number(n) => n.as_i64().is_some_and(|i| i != 0),
                    _ => {
                        return Err(SessionError::InvalidParam(
                            "debug_enabled must be a bool or 0/1".to_owned(),
                        ));
                    }
                };
                tc.debug_options.enabled = v;
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
                // Insert the value — even if the key doesn't exist yet. This
                // handles Optional fields that serde skips when None (e.g.
                // surface_model_id on ProjectCurve).
                let existed = params_obj.contains_key(param);
                // Coerce 0/1 -> bool when the existing field is a boolean.
                // Lets callers that can only produce JSON numbers (e.g. MCP
                // clients that treat every value as numeric) drive boolean
                // params like z_blend, detect_flat_areas, slot_clearing.
                //
                // Also coerce numeric strings ("7", "12.5") into JSON
                // numbers when the existing field is numeric (or absent).
                // The four explicitly-handled fields above (`feed_rate`,
                // `plunge_rate`, `stepover`, `depth_per_pass`) get this
                // for free via `as_number`; this wildcard fallback
                // extends the same tolerance to op-specific fields like
                // `depth`, `cut_depth`, `min_z`, etc., which previously
                // round-tripped through serde and rejected strings.
                // (Roadmap E.6.a)
                let value = match (params_obj.get(param), &value) {
                    (Some(existing), serde_json::Value::Number(n)) if existing.is_boolean() => {
                        match n.as_i64() {
                            Some(0) => serde_json::Value::Bool(false),
                            Some(1) => serde_json::Value::Bool(true),
                            _ => value,
                        }
                    }
                    (existing_opt, serde_json::Value::String(s))
                        if existing_opt.is_none_or(|v| v.is_number()) =>
                    {
                        match s.parse::<f64>() {
                            Ok(n) => serde_json::Number::from_f64(n)
                                .map(serde_json::Value::Number)
                                .unwrap_or(value),
                            Err(_) => value,
                        }
                    }
                    _ => value,
                };
                params_obj.insert(param.to_owned(), value);
                let new_op: crate::compute::catalog::OperationConfig = serde_json::from_value(json)
                    .map_err(|e| {
                        SessionError::InvalidParam(format!("invalid value for '{param}': {e}"))
                    })?;
                // Verify the param was actually consumed: re-serialize and check.
                // Serde ignores unknown fields by default, so a truly unknown param
                // would deserialize successfully but be silently dropped.
                if !existed {
                    let check = serde_json::to_value(&new_op).map_err(|e| {
                        tracing::error!(%e, "failed to re-serialize operation config for param verification");
                        SessionError::InvalidParam(format!(
                            "failed to verify param '{param}': {e}"
                        ))
                    })?;
                    let found = check
                        .get("params")
                        .and_then(|v| v.as_object())
                        .is_some_and(|obj| obj.contains_key(param));
                    if !found {
                        return Err(SessionError::InvalidParam(format!(
                            "unknown parameter '{param}' for {} operation",
                            tc.operation.label()
                        )));
                    }
                }
                tc.operation = new_op;
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
    #[instrument(skip(self, value))]
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
    #[instrument(skip(self, cancel))]
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

        // ProjectCurve optionally references a separate surface model for
        // the mesh (so polygons can come from a DXF while the projection
        // target is a terrain STL). Match the GUI compute path.
        if let crate::compute::OperationConfig::ProjectCurve(ref cfg) = tc.operation
            && let Some(surface_id) = cfg.surface_model_id
            && let Some(surface) = self.find_model_by_raw_id(surface_id.0)
            && surface.mesh.is_some()
        {
            mesh = surface.mesh.clone();
        }

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
        // setup-local frame. `effective_safe_z` floors the user-configured
        // `post.safe_z` at `stock_top + clearance` so rapids clear the stock.
        let safe_z =
            crate::compute::config::effective_safe_z(self.post.safe_z, effective_stock_bbox.max.z);

        let model_bbox = mesh.as_ref().map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z,
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

        // Clone operation so we can patch `setup_z_flipped` on ProjectCurve.
        // This flag is #[serde(skip)] and set at compute time — single source of
        // truth is the setup transform's `is_z_flipped()`.
        let mut operation = tc.operation.clone();
        if let crate::compute::OperationConfig::ProjectCurve(ref mut cfg) = operation {
            let xform = self.setup_transform_info(face_up, z_rotation);
            cfg.setup_z_flipped = needs_transform && xform.is_z_flipped();
        }

        // Pre-resolve the effective boundary polygon so adaptive3d can
        // pre-clip its internal stock (mirrors apply_boundary_clip's
        // computation at line ~570). Doing this before generation rather
        // than after avoids the "cut moves outside boundary become rapids"
        // failure mode that left dexel cells unstamped in deep passes.
        let pre_boundary: Option<crate::polygon::Polygon2> = if boundary_config.enabled {
            Self::resolve_containment_polygon(
                &boundary_config,
                &effective_stock_bbox,
                mesh.as_deref(),
                &keep_out_footprints,
            )
        } else {
            None
        };

        // Execute the operation via the shared compute::execute module (annotated variant)
        let op_scope = semantic_root.start_item(ToolpathSemanticKind::Operation, &op_label);
        let child_ctx = op_scope.context();
        let tp_result = crate::compute::execute::execute_operation_annotated(
            &operation,
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
            Some(&child_ctx),
            pre_boundary.as_ref(),
        );

        match tp_result {
            Ok(annotated) => {
                let mut annotated = annotated;

                if !annotated.toolpath.moves.is_empty() {
                    core_scope.set_move_range(0, annotated.toolpath.moves.len().saturating_sub(1));
                    op_scope.bind_to_toolpath(
                        &annotated.toolpath,
                        0,
                        annotated.toolpath.moves.len(),
                    );
                }
                drop(core_scope);

                // Apply dressups. Pass `prior_stock` if a simulation has
                // already produced a snapshot for this toolpath id (enables
                // air-cut filter + rest-machining-aware dressups).
                let prior_stock_arc = self
                    .simulation
                    .as_ref()
                    .and_then(|sim| sim.prior_stocks.get(&tc.id).cloned());
                let prior_stock_ref = prior_stock_arc.as_deref();
                let dressed = crate::compute::execute::apply_dressups(
                    annotated,
                    &tc.dressups,
                    tool_def.diameter(),
                    heights.retract_z,
                    prior_stock_ref,
                    None,
                    None,
                    tc.operation.transform_capabilities(),
                    None,
                    None,
                );
                annotated = dressed;

                // ── Boundary clipping ─────────────────────────────────
                // After dressups, clip the toolpath to the machining boundary
                // (matching the GUI compute path). Spans are precisely
                // remapped through the clip via the input→output provenance
                // map (S83) so spans_valid stays true.
                if boundary_config.enabled {
                    let clipped = Self::apply_boundary_clip(
                        annotated,
                        &boundary_config,
                        &effective_stock_bbox,
                        mesh.as_deref(),
                        &keep_out_footprints,
                        tool_def.diameter(),
                        heights.retract_z,
                        &semantic_root,
                    );
                    annotated = clipped;
                }

                let stats = ToolpathStats {
                    move_count: annotated.toolpath.moves.len(),
                    cutting_distance: annotated.toolpath.total_cutting_distance(),
                    rapid_distance: annotated.toolpath.total_rapid_distance(),
                };

                let mut debug_trace = debug_recorder.finish();
                let mut semantic_trace = semantic_recorder.finish();
                enrich_traces(&mut debug_trace, &mut semantic_trace);

                self.results.insert(
                    index,
                    ToolpathComputeResult {
                        annotated: Arc::new(annotated),
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

    /// Resolve the boundary "containment polygon" — the polygon the cutter's
    /// footprint must stay inside (Containment=Inside) or outside (Outside).
    /// For ModelSilhouette source this returns the silhouette itself
    /// (after keep-outs and user offset). The downstream toolpath clip
    /// (`clip_toolpath_to_boundary`) does its own tool-radius inset to gate
    /// CUTTER CENTER positions — but for adaptive3d's internal-stock
    /// pre-clip we want the silhouette itself, since the cutter footprint
    /// (when its center is at silhouette - tool_radius) reaches the
    /// silhouette boundary and validly stamps cells in that band.
    pub(crate) fn resolve_containment_polygon(
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&crate::mesh::TriangleMesh>,
        keep_out_footprints: &[crate::polygon::Polygon2],
    ) -> Option<crate::polygon::Polygon2> {
        use crate::boundary::subtract_keepouts;
        use crate::compute::config::BoundarySource;

        let mut stock_poly = match &boundary_config.source {
            BoundarySource::ModelSilhouette if mesh.is_some() => {
                #[allow(clippy::unwrap_used)]
                let m = mesh.unwrap();
                crate::boundary::model_silhouette(m, None)
                    .into_iter()
                    .max_by(|a, b| {
                        a.area()
                            .partial_cmp(&b.area())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or_else(|| {
                        crate::polygon::Polygon2::rectangle(
                            stock_bbox.min.x,
                            stock_bbox.min.y,
                            stock_bbox.max.x,
                            stock_bbox.max.y,
                        )
                    })
            }
            _ => crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            ),
        };
        if !keep_out_footprints.is_empty() {
            stock_poly = subtract_keepouts(&stock_poly, keep_out_footprints);
        }
        if boundary_config.offset.abs() > 1e-9 {
            let offset_polys = crate::polygon::offset_polygon(&stock_poly, -boundary_config.offset);
            if let Some(largest) = offset_polys.into_iter().max_by(|a, b| {
                a.area()
                    .partial_cmp(&b.area())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                stock_poly = largest;
            }
        }
        Some(stock_poly)
    }

    /// Apply boundary clipping to a toolpath, subtracting keep-out footprints.
    ///
    /// Takes/returns an [`AnnotatedToolpath`]. Spans are precisely remapped
    /// through the clip via the provenance map returned from
    /// [`crate::boundary::clip_toolpath_to_boundary_with_provenance`]; the
    /// clipper never drops input moves, only inserts retract/rapid pairs
    /// between them, so a Region span that originally covered "the moves
    /// doing the cut for region X" still covers them post-clip plus any
    /// retracts inserted into the middle. `spans_valid` stays `true`.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_boundary_clip(
        annotated: crate::toolpath_spans::AnnotatedToolpath,
        boundary_config: &crate::compute::config::BoundaryConfig,
        stock_bbox: &BoundingBox3,
        mesh: Option<&crate::mesh::TriangleMesh>,
        keep_out_footprints: &[crate::polygon::Polygon2],
        tool_diameter: f64,
        safe_z: f64,
        semantic_ctx: &crate::semantic_trace::ToolpathSemanticContext,
    ) -> crate::toolpath_spans::AnnotatedToolpath {
        use crate::boundary::{
            ToolContainment, clip_toolpath_to_boundary_with_provenance, effective_boundary,
            subtract_keepouts,
        };
        use crate::compute::config::BoundarySource;

        let crate::toolpath_spans::AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid,
        } = annotated;

        // Resolve the source polygon for the boundary. ModelSilhouette and
        // FaceSelection fall back to the stock rectangle when the required
        // geometry isn't available.
        let mut stock_poly = match &boundary_config.source {
            BoundarySource::ModelSilhouette if mesh.is_some() => {
                // SAFETY: matched `mesh.is_some()` in the pattern guard.
                #[allow(clippy::unwrap_used)]
                let m = mesh.unwrap();
                crate::boundary::model_silhouette(m, None)
                    .into_iter()
                    .max_by(|a, b| {
                        a.area()
                            .partial_cmp(&b.area())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or_else(|| {
                        crate::polygon::Polygon2::rectangle(
                            stock_bbox.min.x,
                            stock_bbox.min.y,
                            stock_bbox.max.x,
                            stock_bbox.max.y,
                        )
                    })
            }
            _ => crate::polygon::Polygon2::rectangle(
                stock_bbox.min.x,
                stock_bbox.min.y,
                stock_bbox.max.x,
                stock_bbox.max.y,
            ),
        };

        // Subtract keep-out footprints (fixtures + keep-out zones).
        if !keep_out_footprints.is_empty() {
            stock_poly = subtract_keepouts(&stock_poly, keep_out_footprints);
        }

        // Apply user-configured offset (positive = expand boundary outward,
        // negative = shrink). cavalier_contours convention: positive distance
        // is INWARD shrink, so flip the sign.
        if boundary_config.offset.abs() > 1e-9 {
            let offset_polys = crate::polygon::offset_polygon(&stock_poly, -boundary_config.offset);
            if let Some(largest) = offset_polys.into_iter().max_by(|a, b| {
                a.area()
                    .partial_cmp(&b.area())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                stock_poly = largest;
            }
            // If the offset collapsed the polygon, fall through with the
            // unmodified stock_poly — the containment offset below may still
            // collapse it, in which case the toolpath is returned uncut.
        }

        // Map BoundaryContainment -> ToolContainment.
        let containment = match boundary_config.containment {
            crate::compute::config::BoundaryContainment::Center => ToolContainment::Center,
            crate::compute::config::BoundaryContainment::Inside => ToolContainment::Inside,
            crate::compute::config::BoundaryContainment::Outside => ToolContainment::Outside,
        };

        let tool_radius = tool_diameter / 2.0;
        let boundaries = effective_boundary(&stock_poly, containment, tool_radius);
        let (clipped, mapping) = match boundaries.first() {
            Some(boundary) => {
                let (clipped, mapping) =
                    clip_toolpath_to_boundary_with_provenance(&toolpath, boundary, safe_z);

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

                (clipped, mapping)
            }
            None => {
                // Boundary collapsed (e.g. tool too large for stock) — return
                // original toolpath with an identity mapping so spans pass
                // through unchanged.
                let n = toolpath.moves.len();
                (toolpath, (0..=n).collect())
            }
        };

        let remapped: Vec<crate::toolpath_spans::Span> =
            spans.iter().map(|s| s.remap(&mapping)).collect();

        crate::toolpath_spans::AnnotatedToolpath {
            toolpath: clipped,
            spans: remapped,
            spans_valid,
        }
    }

    /// Generate all enabled toolpaths, skipping those whose IDs are in `skip`.
    #[instrument(skip(self, skip_ids, cancel))]
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
    #[instrument(skip(self, opts))]
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
                    if result.annotated.toolpath.moves.len() < 2 {
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
                        annotated: Arc::clone(&result.annotated),
                        tool: tool_def,
                        flute_count,
                        tool_summary,
                        semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
                        spindle_rpm: tc.operation.spindle_rpm(),
                    });
                }
            }

            if !entries.is_empty() {
                // Per-setup local stock bbox and transform info derived from
                // the shared SetupTransformInfo helper (Phase E/D dedup).
                let xform = self.setup_transform_info(setup.face_up, setup.z_rotation);
                let local_stock_bbox = Some(xform.effective_stock_bbox());
                let local_to_global = if xform.needs_transform() {
                    Some(xform)
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
                capture_arc_engagement: opts.metrics_enabled,
            },
            spindle_rpm: self.post.spindle_speed,
            rapid_feed_mm_min: if self.post.high_feedrate_mode {
                self.post.high_feedrate
            } else {
                self.machine.max_feed_mm_min.max(1.0)
            },
            model_mesh: self.models.iter().find_map(|m| m.mesh.clone()).map(|m| {
                // Deviation comparison happens in the simulation's
                // stock-relative global frame (0..stock_size). Translate the
                // world-space model mesh by -stock_origin so the two sides of
                // the comparison live in the same frame. For any setup,
                // local_to_global ∘ world_to_local collapses to this
                // translation because face/rotation transforms cancel — so
                // this single shift is correct for all setups.
                Arc::new(translate_mesh(
                    &m,
                    -self.stock.origin_x,
                    -self.stock.origin_y,
                    -self.stock.origin_z,
                ))
            }),
        };

        let result = run_simulation(&request, cancel)?;
        self.simulation = Some(result);
        // SAFETY: we just assigned Some
        #[allow(clippy::unwrap_used)]
        Ok(self.simulation.as_ref().unwrap())
    }

    /// Run a collision check for a specific toolpath by index.
    #[instrument(skip(self))]
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
            toolpath: result.toolpath(),
            tool: tool_def,
            mesh: model,
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// Narrate one generated toolpath in prose for agent-oriented debugging.
    #[instrument(skip(self))]
    pub fn narrate_toolpath(&self, index: usize) -> Result<String, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);
        let cut_trace = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        let context = crate::narrate::ToolpathNarrationContext {
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            depth_per_pass_mm: tc.operation.depth_per_pass(),
            stepover_mm: tc.operation.stepover(),
            tool_diameter_mm: Some(tool.diameter),
            feed_rate_mm_min: Some(tc.operation.feed_rate()),
            spindle_rpm: Some(
                tc.operation
                    .spindle_rpm()
                    .unwrap_or(self.post.spindle_speed),
            ),
            flute_count: Some(tool.flute_count),
            is_drill_cycle: matches!(
                tc.operation.op_type(),
                crate::compute::catalog::OperationType::Drill
                    | crate::compute::catalog::OperationType::AlignmentPinDrill
            ),
        };

        Ok(crate::narrate::narrate_toolpath_with_context(
            &result.annotated,
            result.semantic_trace.as_ref(),
            cut_trace,
            result.debug_trace.as_ref(),
            &tool_def,
            &context,
        ))
    }

    /// Compute project diagnostics from current results and simulation.
    ///
    /// Rapid collision counts come from the simulation result (which checks
    /// against the actual remaining stock surface). If no simulation has
    /// been run, rapid collision counts are 0 — we don't fall back to the
    /// inaccurate original-bbox check.
    #[instrument(skip(self))]
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

    /// Export G-code for all computed toolpaths under the default tool-load
    /// policy (refuse on Exceeds or Unmodeled). For an override-capable
    /// variant see [`export_gcode_with_policy`].
    #[instrument(skip(self))]
    pub fn export_gcode(&self, path: &Path, _setup_id: Option<usize>) -> Result<(), SessionError> {
        self.export_gcode_with_policy(
            path,
            _setup_id,
            crate::gcode::ToolLoadExportPolicy::default(),
        )
    }

    /// Export G-code with an explicit tool-load policy. Used by callers that
    /// want to override `Exceeds` or `Unmodeled` verdicts (UI checkbox,
    /// `--accept-…` CLI flag, MCP parameter).
    #[instrument(skip(self))]
    pub fn export_gcode_with_policy(
        &self,
        path: &Path,
        _setup_id: Option<usize>,
        policy: crate::gcode::ToolLoadExportPolicy,
    ) -> Result<(), SessionError> {
        let gcode = crate::gcode::export_gcode_checked(
            self,
            self.simulation
                .as_ref()
                .and_then(|simulation| simulation.cut_trace.as_deref()),
            policy,
        )
        .map_err(|e| SessionError::Export(e.to_string()))?;
        std::fs::write(path, gcode).map_err(|e| {
            SessionError::Export(format!("Failed to write G-code to {}: {e}", path.display()))
        })
    }

    /// Compute the per-toolpath tool-load report against current state. Used
    /// by `get_tool_load_report` (MCP) and the export gate. Returns deflection
    /// + chipload populated; power is `Unmodeled(NotImplemented)` until
    ///
    /// Phase 1b lands the arc-engagement-driven power criterion.
    pub fn tool_load_report(&self) -> crate::tool_load::ToolLoadReport {
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|simulation| simulation.cut_trace.as_deref());
        crate::gcode::project_load_report(self, sim_trace)
    }

    /// Export diagnostics as JSON files to an output directory.
    #[instrument(skip(self))]
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::session::ToolpathConfig;
    use serde_json::json;

    fn make_session() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        let tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        s.add_tool(tool);
        s
    }

    fn make_tc(tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            feeds_auto: crate::compute::config::FeedsAutoMode::default(),
            debug_options: ToolpathDebugOptions::default(),
        }
    }

    // ── set_toolpath_param ───────────────────────────────────────

    #[test]
    fn set_toolpath_param_feed_rate() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "feed_rate", json!(2000.0)).unwrap();
        // Verify via OperationParams trait
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.feed_rate - 2000.0).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_plunge_rate() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "plunge_rate", json!(500.0))
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.plunge_rate - 500.0).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_stepover() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "stepover", json!(0.5)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.stepover - 0.5).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_depth_per_pass() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "depth_per_pass", json!(1.5))
            .unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!((cfg.depth_per_pass - 1.5).abs() < 1e-9),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_coerces_integer_to_bool() {
        // MCP clients that can only produce JSON numbers should still be
        // able to set boolean params like `climb`, `z_blend`, etc.
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Pocket default has climb=true; flip it via integer 0.
        s.set_toolpath_param(0, "climb", json!(0)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
            _ => panic!("expected Pocket"),
        }
        // Flip back with integer 1.
        s.set_toolpath_param(0, "climb", json!(1)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(cfg.climb),
            _ => panic!("expected Pocket"),
        }
        // Non-0/1 integers fall through to serde, which will reject them.
        let result = s.set_toolpath_param(0, "climb", json!(42));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
        // Actual booleans still work.
        s.set_toolpath_param(0, "climb", json!(false)).unwrap();
        match &s.toolpath_configs()[0].operation {
            OperationConfig::Pocket(cfg) => assert!(!cfg.climb),
            _ => panic!("expected Pocket"),
        }
    }

    #[test]
    fn set_toolpath_param_wrong_type_errors() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "feed_rate", json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_toolpath_param_unknown_param() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "totally_fake_param", json!(42.0));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_toolpath_param_invalid_index() {
        let mut s = make_session();
        let result = s.set_toolpath_param(99, "feed_rate", json!(100.0));
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }

    #[test]
    fn set_toolpath_param_spindle_rpm() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Default is None.
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
        s.set_toolpath_param(0, "spindle_rpm", json!(15000))
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), Some(15000));
    }

    #[test]
    fn set_toolpath_param_spindle_rpm_null_clears() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.set_toolpath_param(0, "spindle_rpm", json!(20_000))
            .unwrap();
        assert_eq!(
            s.toolpath_configs()[0].operation.spindle_rpm(),
            Some(20_000)
        );
        s.set_toolpath_param(0, "spindle_rpm", serde_json::Value::Null)
            .unwrap();
        assert_eq!(s.toolpath_configs()[0].operation.spindle_rpm(), None);
    }

    #[test]
    fn set_toolpath_param_spindle_rpm_invalid_type() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        let result = s.set_toolpath_param(0, "spindle_rpm", json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
        // Negative numbers fail u64 conversion.
        let result = s.set_toolpath_param(0, "spindle_rpm", json!(-1));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    /// F2 — MCP / JSON-RPC clients sometimes serialize integer literals as
    /// f64 (so 13500 arrives as 13500.0). The router accepts integer-valued
    /// floats and parseable numeric strings as well as plain integers.
    #[test]
    fn set_toolpath_param_spindle_rpm_accepts_f64_and_string() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        // Integer-valued f64.
        s.set_toolpath_param(0, "spindle_rpm", json!(13500.0))
            .unwrap();
        assert_eq!(
            s.toolpath_configs()[0].operation.spindle_rpm(),
            Some(13500)
        );
        // Numeric string.
        s.set_toolpath_param(0, "spindle_rpm", json!("18000"))
            .unwrap();
        assert_eq!(
            s.toolpath_configs()[0].operation.spindle_rpm(),
            Some(18000)
        );
        // Non-integer float is rejected (would lose precision).
        let result = s.set_toolpath_param(0, "spindle_rpm", json!(13500.5));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_toolpath_param_invalidates_result() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.results.insert(
            0,
            ToolpathComputeResult {
                annotated: Arc::new(crate::toolpath_spans::AnnotatedToolpath::new(
                    crate::toolpath::Toolpath::new(),
                )),
                stats: ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );
        s.set_toolpath_param(0, "feed_rate", json!(1000.0)).unwrap();
        assert!(!s.results.contains_key(&0));
    }

    // ── set_tool_param ───────────────────────────────────────────

    #[test]
    fn set_tool_param_diameter() {
        let mut s = make_session();
        s.set_tool_param(0, "diameter", &json!(6.0)).unwrap();
        assert!((s.tools()[0].diameter - 6.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_flute_count() {
        let mut s = make_session();
        s.set_tool_param(0, "flute_count", &json!(4)).unwrap();
        assert_eq!(s.tools()[0].flute_count, 4);
    }

    #[test]
    fn set_tool_param_stickout() {
        let mut s = make_session();
        s.set_tool_param(0, "stickout", &json!(25.0)).unwrap();
        assert!((s.tools()[0].stickout - 25.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_corner_radius() {
        let mut s = make_session();
        s.set_tool_param(0, "corner_radius", &json!(0.5)).unwrap();
        assert!((s.tools()[0].corner_radius - 0.5).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_cutting_length() {
        let mut s = make_session();
        s.set_tool_param(0, "cutting_length", &json!(20.0)).unwrap();
        assert!((s.tools()[0].cutting_length - 20.0).abs() < 1e-9);
    }

    #[test]
    fn set_tool_param_invalid_index() {
        let mut s = make_session();
        let result = s.set_tool_param(99, "diameter", &json!(6.0));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_tool_param_wrong_type() {
        let mut s = make_session();
        let result = s.set_tool_param(0, "diameter", &json!("not a number"));
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    #[test]
    fn set_tool_param_invalidates_toolpath_results() {
        let mut s = make_session();
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s.results.insert(
            0,
            ToolpathComputeResult {
                annotated: Arc::new(crate::toolpath_spans::AnnotatedToolpath::new(
                    crate::toolpath::Toolpath::new(),
                )),
                stats: ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );

        s.set_tool_param(0, "diameter", &json!(8.0)).unwrap();
        assert!(!s.results.contains_key(&0));
    }

    // ── generate_toolpath error paths ────────────────────────────

    #[test]
    fn generate_toolpath_not_found() {
        let mut s = make_session();
        let cancel = AtomicBool::new(false);
        let result = s.generate_toolpath(99, &cancel);
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }

    // ── diagnostics ──────────────────────────────────────────────

    #[test]
    fn diagnostics_empty() {
        let s = ProjectSession::new_empty();
        let diag = s.diagnostics();
        assert_eq!(diag.verdict, "OK");
        assert!(diag.per_toolpath.is_empty());
    }
}
