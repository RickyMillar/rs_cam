//! The MCP project surface: the reads that describe the open project —
//! its toolpaths, tools, setups, model, stock and machine — and the one
//! door that loads another project.
//!
//! Split out of `app/mcp.rs` (P4). Every item is a verbatim move; the
//! handlers stay reachable as `RsCamApp::mcp_*` because an inherent
//! `impl` compiles in any module of the crate.

use std::path::Path;

use rs_cam_core::compute::config::ComputeStatus;
use rs_cam_core::session::{GetOperationSchemaArgs, Query, QueryAnswer};

use rs_cam_mcp::server::{json_str, no_project_error, text};

use crate::app::RsCamApp;
use crate::mcp_bridge::{McpOutcome, McpResponse};

impl RsCamApp {
    pub(super) fn mcp_project_summary(&self) -> String {
        let session = &self.controller.state().session;
        if session.toolpath_count() == 0 && session.models().is_empty() {
            return no_project_error();
        }
        let bbox = session.stock_bbox();
        let stale_defaults = rs_cam_core::compute::validate::validate_stale_defaults(session);
        json_str(serde_json::json!({
            "name": session.name(),
            "stock": {
                "width": bbox.max.x - bbox.min.x,
                "depth": bbox.max.y - bbox.min.y,
                "height": bbox.max.z - bbox.min.z,
            },
            "setup_count": session.setup_count(),
            "toolpath_count": session.toolpath_count(),
            "tools": session.list_tools(),
            "stale_defaults": stale_defaults,
            "build": rs_cam_mcp::server::build_info(),
        }))
    }

    pub(super) fn mcp_list_toolpaths(&self) -> String {
        // Override the core listing to inject viz-only fields (`stale`,
        // `status`) so an automation client can tell at a glance whether a
        // toolpath needs regeneration. Staleness lives in
        // `ToolpathRuntime.stale_since` — viz-only — and the core summary
        // never sees it. (Roadmap E.3)
        let state = self.controller.state();
        let session = &state.session;
        let summaries = session.list_toolpaths();
        let rows: Vec<serde_json::Value> = summaries
            .into_iter()
            .map(|s| {
                let rt = state.gui.toolpath_rt.get(&s.id);
                let stale = rt.is_some_and(|r| r.stale_since.is_some());
                // A/M11: `enabled: false` wins over whatever the op last
                // recorded, so a switched-off toolpath can never present the
                // rest-stock error it had while it was on.
                let raw = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
                let status = ComputeStatus::effective(s.enabled, raw);
                serde_json::json!({
                    "index": s.index,
                    "id": s.id,
                    "name": s.name,
                    "operation_label": s.operation_label,
                    "enabled": s.enabled,
                    "tool_name": s.tool_name,
                    "stale": stale,
                    "status": status.label(),
                    "error": status.error_text(),
                    // W5 item (a): the core struct serialises. Do not
                    // hand-build the three keys here.
                    "awaiting_prior_stock": status.blocked_on(),
                })
            })
            .collect();
        json_str(serde_json::Value::Array(rows))
    }

    pub(super) fn mcp_list_tools(&self) -> String {
        let session = &self.controller.state().session;
        json_str(serde_json::to_value(session.list_tools()).unwrap_or_default())
    }

    /// Top level of the tool-library drill-down: one small row per
    /// catalog (name, tool count, the tool types it contains). Cheap —
    /// no per-tool geometry. Independent of the loaded project.
    pub(super) fn mcp_list_tool_library(&self) -> String {
        use rs_cam_core::io::tool_library;
        let mut catalogs = Vec::new();
        for name in tool_library::list_libraries() {
            let Ok(catalog) = tool_library::load_library(&name) else {
                continue;
            };
            let mut types: Vec<String> = catalog
                .tools
                .iter()
                .filter_map(|t| {
                    serde_json::to_value(t.tool_type)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                })
                .collect();
            types.sort();
            types.dedup();
            catalogs.push(serde_json::json!({
                "catalog": name,
                "tool_count": catalog.tools.len(),
                "tool_types": types,
            }));
        }
        json_str(serde_json::json!({
            "note": "Dig into a catalog with list_tool_catalog{catalog} to see its tools, \
                     then import one with add_tool_from_library{catalog, index}.",
            "catalogs": catalogs,
        }))
    }

    /// Drill into one catalog: compact tool rows carrying just the fields
    /// needed to choose a tool, plus the 0-based `index` that
    /// `add_tool_from_library` consumes. Full geometry comes across on
    /// import (the project keeps a snapshot).
    pub(super) fn mcp_list_tool_catalog(&self, catalog: &str) -> String {
        use rs_cam_core::io::tool_library;
        let cat = match tool_library::load_library(catalog) {
            Ok(c) => c,
            Err(e) => return json_str(serde_json::json!({ "error": e.to_string() })),
        };
        let tools: Vec<serde_json::Value> = cat
            .tools
            .iter()
            .enumerate()
            .map(|(index, t)| {
                let ttype = serde_json::to_value(t.tool_type)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let mut row = serde_json::json!({
                    "index": index,
                    "name": t.name,
                    "type": ttype,
                    "diameter_mm": t.diameter,
                    "flutes": t.flute_count,
                    "cutting_length_mm": t.cutting_length,
                    "shank_diameter_mm": t.shank_diameter,
                });
                if let Some(obj) = row.as_object_mut() {
                    use crate::state::job::ToolType;
                    match t.tool_type {
                        ToolType::VBit => {
                            obj.insert(
                                "included_angle_deg".to_owned(),
                                serde_json::json!(t.included_angle),
                            );
                        }
                        ToolType::TaperedBallNose => {
                            obj.insert(
                                "taper_half_angle_deg".to_owned(),
                                serde_json::json!(t.taper_half_angle),
                            );
                            obj.insert(
                                "shaft_diameter_mm".to_owned(),
                                serde_json::json!(t.shaft_diameter),
                            );
                        }
                        _ => {}
                    }
                    if !t.vendor.is_empty() {
                        obj.insert("vendor".to_owned(), serde_json::json!(t.vendor));
                    }
                }
                row
            })
            .collect();
        json_str(serde_json::json!({
            "catalog": catalog,
            "tool_count": tools.len(),
            "tools": tools,
        }))
    }

    pub(super) fn mcp_list_setups(&self) -> String {
        let session = &self.controller.state().session;
        let setups: Vec<serde_json::Value> = session
            .list_setups()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "face_up": s.face_up.label(),
                    "toolpath_indices": s.toolpath_indices,
                })
            })
            .collect();
        json_str(serde_json::json!({ "setups": setups }))
    }

    pub(super) fn mcp_get_toolpath_params(&self, index: usize) -> String {
        let session = &self.controller.state().session;
        match session.get_toolpath_config(index) {
            Some(tc) => {
                let mut op_value =
                    serde_json::to_value(&tc.operation).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(obj) = op_value.as_object_mut() {
                    obj.insert(
                        "params".to_owned(),
                        tc.operation.params_value_including_nulls(),
                    );
                    obj.insert(
                        "param_schema".to_owned(),
                        serde_json::to_value(tc.operation.param_schema_hints())
                            .unwrap_or_else(|_| serde_json::json!({})),
                    );
                }
                json_str(serde_json::json!({
                    "id": tc.id,
                    "name": tc.name,
                    "enabled": tc.enabled,
                    "tool_id": tc.tool_id,
                    "model_id": tc.model_id,
                    "operation": op_value,
                    // P2.2: reports the machining boundary, including
                    // `derived_rest_regions`'s `source_toolpath_id` — there
                    // is no dedicated get_boundary_config tool, so this is
                    // the only MCP surface for reading it back.
                    "boundary": tc.boundary,
                    // P2.5: op-agnostic rest analysis config — mirrors `boundary`'s
                    // presence here; set via `set_rest_analysis_config`.
                    "rest_analysis": tc.rest_analysis,
                    "runtime": self.mcp_runtime_status_for_toolpath_id(tc.id),
                }))
            }
            None => {
                json_str(serde_json::json!({"error": format!("Toolpath index {index} not found")}))
            }
        }
    }

    /// Report the parameter schema of one operation kind.
    ///
    /// WP13 ruling 3: this is a core `Query` row, not a view read. The
    /// answer comes from the operation CATALOG, so the read never touches
    /// the view — and it takes the core read door like any other query.
    pub(super) fn mcp_get_operation_schema(&self, operation_type: &str) -> String {
        let query = Query::GetOperationSchema(GetOperationSchemaArgs {
            operation_type: operation_type.to_owned(),
        });
        match self.controller.state().session.query(query) {
            Ok(QueryAnswer::GetOperationSchema(answer)) => {
                json_str(serde_json::to_value(answer.schema).unwrap_or_default())
            }
            Ok(other) => json_str(serde_json::json!({
                "error": format!("the get_operation_schema read answered {other:?}"),
            })),
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
        }
    }

    pub(super) fn mcp_inspect_model(&self) -> String {
        let session = &self.controller.state().session;
        let models = session.models();
        if models.is_empty() {
            return json_str(serde_json::json!([]));
        }

        let mut result = Vec::new();
        for model in models {
            let mut map = serde_json::Map::new();
            map.insert("id".into(), serde_json::json!(model.id));
            map.insert("name".into(), serde_json::json!(model.name));
            map.insert(
                "kind".into(),
                serde_json::json!(model.kind.map(|k| format!("{k:?}").to_lowercase())),
            );
            map.insert(
                "units".into(),
                serde_json::json!(model.units.as_ref().map(|u| u.label())),
            );
            map.insert(
                "path".into(),
                serde_json::json!(model.path.display().to_string()),
            );
            map.insert("load_error".into(), serde_json::json!(model.load_error));

            // Mesh-level stats (STL and STEP)
            if let Some(ref mesh) = model.mesh {
                let bbox = &mesh.bbox;
                map.insert(
                    "bbox".into(),
                    serde_json::json!({
                        "min": [bbox.min.x, bbox.min.y, bbox.min.z],
                        "max": [bbox.max.x, bbox.max.y, bbox.max.z],
                    }),
                );
                map.insert(
                    "dimensions".into(),
                    serde_json::json!({
                        "x": bbox.max.x - bbox.min.x,
                        "y": bbox.max.y - bbox.min.y,
                        "z": bbox.max.z - bbox.min.z,
                    }),
                );
                map.insert(
                    "triangle_count".into(),
                    serde_json::json!(mesh.triangles.len()),
                );
                map.insert(
                    "vertex_count".into(),
                    serde_json::json!(mesh.vertices.len()),
                );
            }

            // Winding consistency (STL-specific)
            if let Some(winding) = model.winding_report {
                map.insert(
                    "winding_consistency".into(),
                    serde_json::json!(1.0 - winding),
                );
            }

            // BREP summary (STEP-specific)
            if let Some(ref em) = model.enriched_mesh {
                let concave_count = em.edges.iter().filter(|e| e.is_concave).count();
                let faces_json: Vec<serde_json::Value> = em
                    .face_groups
                    .iter()
                    .map(|fg| {
                        serde_json::json!({
                            "id": fg.id.0,
                            "surface_type": format!("{:?}", fg.surface_type),
                            "bbox": {
                                "min": [fg.bbox.min.x, fg.bbox.min.y, fg.bbox.min.z],
                                "max": [fg.bbox.max.x, fg.bbox.max.y, fg.bbox.max.z],
                            },
                        })
                    })
                    .collect();
                map.insert(
                    "brep".into(),
                    serde_json::json!({
                        "face_count": em.face_groups.len(),
                        "edge_count": em.edges.len(),
                        "concave_edge_count": concave_count,
                        "faces": faces_json,
                    }),
                );
            }

            // Polygon summary (SVG/DXF-specific)
            if let Some(ref polys) = model.polygons {
                let count = polys.len();
                let total_area: f64 = polys.iter().map(|p| p.area()).sum();
                let total_perimeter: f64 = polys.iter().map(|p| p.perimeter()).sum();
                let hole_count: usize = polys.iter().map(|p| p.holes.len()).sum();

                // Compute 2D bbox from polygon exteriors
                let mut min_x = f64::MAX;
                let mut min_y = f64::MAX;
                let mut max_x = f64::MIN;
                let mut max_y = f64::MIN;
                for poly in polys.iter() {
                    for pt in &poly.exterior {
                        if pt.x < min_x {
                            min_x = pt.x;
                        }
                        if pt.y < min_y {
                            min_y = pt.y;
                        }
                        if pt.x > max_x {
                            max_x = pt.x;
                        }
                        if pt.y > max_y {
                            max_y = pt.y;
                        }
                    }
                }

                let bbox_2d = if min_x <= max_x {
                    serde_json::json!({ "min": [min_x, min_y], "max": [max_x, max_y] })
                } else {
                    serde_json::json!(null)
                };

                map.insert(
                    "polygons".into(),
                    serde_json::json!({
                        "count": count,
                        "total_area": total_area,
                        "total_perimeter": total_perimeter,
                        "hole_count": hole_count,
                        "bbox_2d": bbox_2d,
                    }),
                );
            }

            result.push(serde_json::Value::Object(map));
        }

        json_str(serde_json::json!(result))
    }

    pub(super) fn mcp_inspect_stock(&self) -> String {
        let session = &self.controller.state().session;
        let stock = session.stock_config();

        let pins: Vec<serde_json::Value> = stock
            .alignment_pins
            .iter()
            .map(|pin| {
                serde_json::json!({
                    "x": pin.x,
                    "y": pin.y,
                    "diameter": pin.diameter,
                })
            })
            .collect();

        json_str(serde_json::json!({
            "dimensions": { "x": stock.x, "y": stock.y, "z": stock.z },
            "origin": { "x": stock.origin_x, "y": stock.origin_y, "z": stock.origin_z },
            "material": stock.material.label(),
            "padding": stock.padding,
            "auto_from_model": stock.auto_from_model,
            "workholding_rigidity": format!("{:?}", stock.workholding_rigidity),
            "alignment_pins": pins,
            "flip_axis": stock.flip_axis.map(|fa| fa.label()),
        }))
    }

    pub(super) fn mcp_inspect_machine(&self) -> String {
        let session = &self.controller.state().session;
        let machine = session.machine();

        let spindle = match &machine.spindle {
            rs_cam_core::machine::SpindleConfig::Variable { min_rpm, max_rpm } => {
                serde_json::json!({
                    "type": "Variable",
                    "min_rpm": min_rpm,
                    "max_rpm": max_rpm,
                })
            }
            rs_cam_core::machine::SpindleConfig::Discrete { speeds } => {
                serde_json::json!({
                    "type": "Discrete",
                    "speeds": speeds,
                })
            }
        };

        let power = match &machine.power {
            rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => {
                serde_json::json!({
                    "type": "ConstantPower",
                    "power_kw": power_kw,
                })
            }
            rs_cam_core::machine::PowerModel::VfdConstantTorque {
                rated_power_kw,
                rated_rpm,
            } => {
                serde_json::json!({
                    "type": "VfdConstantTorque",
                    "rated_power_kw": rated_power_kw,
                    "rated_rpm": rated_rpm,
                })
            }
        };

        // Acceleration-aware kinematics ($11 + per-axis $120-122 + per-axis
        // rates $110-112). `None` means the cycle-time model falls back to
        // the naive distance/feed sum (the F-034 feature flag).
        let kinematics = match &machine.kinematics {
            Some(k) => {
                let per_axis = match k.acceleration_xyz_mm_s2 {
                    Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
                    None => serde_json::Value::Null,
                };
                let per_axis_rate = match k.max_rate_xyz_mm_min {
                    Some([rx, ry, rz]) => serde_json::json!([rx, ry, rz]),
                    None => serde_json::Value::Null,
                };
                serde_json::json!({
                    "configured": true,
                    "acceleration_mm_s2": k.acceleration_mm_s2,
                    "acceleration_xyz_mm_s2": per_axis,
                    "max_rate_xyz_mm_min": per_axis_rate,
                    "junction_deviation_mm": k.junction_deviation_mm,
                    "jerk_mm_s3": k.jerk_mm_s3,
                    "max_junction_velocity_mm_min": k.max_junction_velocity_mm_min,
                })
            }
            None => serde_json::json!({
                "configured": false,
                "note": "no kinematics set — cycle time uses the naive distance/feed sum",
            }),
        };

        let r = &machine.rigidity;
        json_str(serde_json::json!({
            "name": machine.name,
            "max_feed_mm_min": machine.max_feed_mm_min,
            "cutting_feed_ceiling_mm_min": machine.cutting_feed_ceiling_mm_min(),
            "max_shank_mm": machine.max_shank_mm,
            "safety_factor": machine.safety_factor,
            "spindle": spindle,
            "power": power,
            "kinematics": kinematics,
            "rigidity": {
                "doc_roughing_factor": r.doc_roughing_factor,
                "doc_finishing_factor": r.doc_finishing_factor,
                "woc_roughing_factor": r.woc_roughing_factor,
                "woc_roughing_max_mm": r.woc_roughing_max_mm,
                "woc_finishing_mm": r.woc_finishing_mm,
                "adaptive_doc_factor": r.adaptive_doc_factor,
                "adaptive_woc_factor": r.adaptive_woc_factor,
            },
        }))
    }

    /// List the per-user machine library with a compact spec summary per
    /// entry (snapshot model — these are import sources, not live links).
    pub(super) fn mcp_list_machine_library(&self) -> String {
        let names = rs_cam_core::io::machine_library::list();
        let machines: Vec<serde_json::Value> = names
            .iter()
            .map(|name| match rs_cam_core::io::machine_library::load(name) {
                Ok(p) => {
                    let kinematics = match &p.kinematics {
                        Some(k) => {
                            let per_axis = match k.acceleration_xyz_mm_s2 {
                                Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
                                None => serde_json::Value::Null,
                            };
                            serde_json::json!({
                                "acceleration_xyz_mm_s2": per_axis,
                                "acceleration_mm_s2": k.acceleration_mm_s2,
                                "junction_deviation_mm": k.junction_deviation_mm,
                            })
                        }
                        None => serde_json::Value::Null,
                    };
                    serde_json::json!({
                        "name": name,
                        "profile_name": p.name,
                        "max_feed_mm_min": p.max_feed_mm_min,
                        "kinematics": kinematics,
                    })
                }
                Err(e) => serde_json::json!({ "name": name, "error": e.to_string() }),
            })
            .collect();
        let count = machines.len();
        json_str(serde_json::json!({ "count": count, "machines": machines }))
    }

    pub(super) fn mcp_inspect_brep_faces(&self, model_id: usize) -> String {
        let session = &self.controller.state().session;
        let models = session.models();
        let model = models.iter().find(|m| m.id == model_id);
        let Some(model) = model else {
            return json_str(serde_json::json!({"error": format!("Model {model_id} not found")}));
        };

        let Some(ref em) = model.enriched_mesh else {
            return json_str(serde_json::json!({
                "error": format!("Model '{}' has no BREP data (not a STEP model)", model.name)
            }));
        };

        let faces_json: Vec<serde_json::Value> = em
            .face_groups
            .iter()
            .map(|fg| {
                let mut map = serde_json::Map::new();
                map.insert("id".into(), serde_json::json!(fg.id.0));
                map.insert(
                    "surface_type".into(),
                    serde_json::json!(format!("{:?}", fg.surface_type)),
                );
                map.insert(
                    "bbox".into(),
                    serde_json::json!({
                        "min": [fg.bbox.min.x, fg.bbox.min.y, fg.bbox.min.z],
                        "max": [fg.bbox.max.x, fg.bbox.max.y, fg.bbox.max.z],
                    }),
                );
                map.insert(
                    "triangle_count".into(),
                    serde_json::json!(fg.triangle_range.len()),
                );
                map.insert(
                    "has_2d_boundary".into(),
                    serde_json::json!(fg.boundary_loops_2d.is_some()),
                );

                // Surface-type-specific fields
                match &fg.surface_params {
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Plane {
                        normal, ..
                    } => {
                        map.insert(
                            "normal".into(),
                            serde_json::json!([normal.x, normal.y, normal.z]),
                        );
                        map.insert(
                            "is_horizontal".into(),
                            serde_json::json!(normal.z.abs() > 0.95),
                        );
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Cylinder {
                        radius,
                        axis_dir,
                        ..
                    } => {
                        map.insert("radius".into(), serde_json::json!(radius));
                        map.insert(
                            "axis".into(),
                            serde_json::json!([axis_dir.x, axis_dir.y, axis_dir.z]),
                        );
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Cone {
                        half_angle,
                        axis,
                        ..
                    } => {
                        map.insert(
                            "half_angle_deg".into(),
                            serde_json::json!(half_angle.to_degrees()),
                        );
                        map.insert("axis".into(), serde_json::json!([axis.x, axis.y, axis.z]));
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Sphere {
                        radius, ..
                    } => {
                        map.insert("radius".into(), serde_json::json!(radius));
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Torus {
                        major_radius,
                        minor_radius,
                        axis,
                        ..
                    } => {
                        map.insert("major_radius".into(), serde_json::json!(major_radius));
                        map.insert("minor_radius".into(), serde_json::json!(minor_radius));
                        map.insert("axis".into(), serde_json::json!([axis.x, axis.y, axis.z]));
                    }
                    _ => {}
                }

                serde_json::Value::Object(map)
            })
            .collect();

        let edges_json: Vec<serde_json::Value> = em
            .edges
            .iter()
            .map(|edge| {
                serde_json::json!({
                    "id": edge.id,
                    "face_a": edge.face_a.0,
                    "face_b": edge.face_b.0,
                    "dihedral_angle_deg": edge.dihedral_angle.to_degrees(),
                    "is_concave": edge.is_concave,
                    "vertex_count": edge.vertices.len(),
                })
            })
            .collect();

        json_str(serde_json::json!({
            "model_id": model_id,
            "face_count": em.face_groups.len(),
            "faces": faces_json,
            "edges": edges_json,
        }))
    }

    /// Load a project. The reply is plain text, so the toast outcome rides
    /// beside it as the controller's own `Result` (G-MCPTOAST).
    pub(super) fn mcp_load_project(
        &mut self,
        path: &str,
        discard_unsaved: bool,
    ) -> (McpResponse, McpOutcome) {
        // G-OPENGUARD (F1.12): a load replaces the open project. The GUI
        // asks a human (Save / Discard / Cancel); an agent has nobody to
        // ask, so the equivalent is a refusal that names what would go and
        // an explicit flag to override it. Checked BEFORE the load, so a
        // refusal leaves the project, the camera and the file untouched.
        if !discard_unsaved && let Some(refusal) = self.unsaved_project_refusal() {
            return (
                McpResponse {
                    result: Ok(text(refusal.clone())),
                },
                McpOutcome::Refused(refusal),
            );
        }
        let loaded = self.controller.open_job_from_path(Path::new(path));
        let outcome = McpOutcome::from_result(&loaded);
        let resp = match loaded {
            Ok(()) => {
                // G-WSMENU (2026-09-10): the same fit the File > Open route
                // does. An agent's very next call is usually
                // `screenshot_gui`, and before this the viewport still held
                // the previous project's framing.
                self.fit_camera_to_first_model();
                let name = self.controller.state().session.name().to_owned();
                let tp_count = self.controller.state().session.toolpath_count();
                let setup_count = self.controller.state().session.setup_count();
                let mut msg =
                    format!("Loaded '{name}' -- {setup_count} setups, {tp_count} toolpaths");
                // Surface load warnings the GUI shows in a modal but the MCP
                // wrapper previously dropped on the floor (Roadmap E.1).
                let warnings = self.controller.load_warnings();
                if !warnings.is_empty() {
                    msg.push_str("\nWarnings:");
                    for w in warnings {
                        msg.push_str("\n  - ");
                        msg.push_str(w);
                    }
                }
                McpResponse {
                    result: Ok(text(msg)),
                }
            }
            Err(e) => McpResponse {
                result: Ok(text(format!("Failed to load: {e}"))),
            },
        };
        (resp, outcome)
    }

    /// The refusal text when the open project has unsaved changes, or
    /// `None` when a load may proceed (G-OPENGUARD).
    ///
    /// The sentence naming what would be lost is
    /// [`crate::state::unsaved_project_summary`], shared with anything
    /// else that has to describe the same situation; this adds only the
    /// two ways out.
    fn unsaved_project_refusal(&self) -> Option<String> {
        crate::state::unsaved_project_summary(self.controller.state()).map(|summary| {
            format!(
                "Refused: {summary}. Loading would replace it and throw those \
                 changes away. Save it first (save_project), or pass \
                 discard_unsaved: true."
            )
        })
    }
}
