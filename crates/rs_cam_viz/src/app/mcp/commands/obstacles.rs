//! K1 (`planning/op_architecture_2026-10-14/PLAN.md`, item K): the three
//! MCP obstacle rows and the obstacle JSON that the read tools print.
//!
//! The wire tools are `add_obstacle`, `edit_obstacle` and
//! `remove_obstacle`. Each turns into ONE command through the core route of
//! `commands.rs`; this file holds the conversion and the reply. A refusal of
//! the core validation reaches the agent in the core's words.

use rs_cam_core::compute::obstacle::{
    DEFAULT_OBSTACLE_CLEARANCE_MM, Obstacle, ObstacleGeometry, next_obstacle_id,
};
use rs_cam_core::ids::{ModelId, ObstacleId};
use rs_cam_core::session::{
    AddObstacleArgs, Command, Effects, RemoveObstacleArgs, ReplaceObstacleArgs,
};
use rs_cam_mcp::server::{
    AddObstacleParam, EditObstacleParam, ObstacleGeometryKindParam, RemoveObstacleParam,
};

use super::{CoreBefore, CorePlan, CoreReply};
use crate::app::RsCamApp;

/// One obstacle as the MCP read tools print it. Every value is in the
/// setup frame.
pub(crate) fn obstacle_json(obstacle: &Obstacle) -> serde_json::Value {
    serde_json::json!({
        "id": obstacle.id.0,
        "name": obstacle.name,
        "enabled": obstacle.enabled,
        "clearance": obstacle.clearance,
        // K2: `geometry.kind` (the serde tag, snake_case) is the one kind
        // spelling. It equals the `kind` input of `add_obstacle`.
        "geometry": serde_json::to_value(&obstacle.geometry)
            .unwrap_or(serde_json::Value::Null),
    })
}

/// A wire refusal: the sentence and the field it names.
type WireRefusal = (String, &'static str);

/// The flat wire fields of a geometry.
struct GeometryFields<'a> {
    min: Option<[f64; 3]>,
    max: Option<[f64; 3]>,
    model_ids: &'a [usize],
    z_bottom: Option<f64>,
    z_top: Option<f64>,
}

/// Build a geometry from the wire's flat fields. Every field the kind needs
/// must be present, and a field the kind does not hold is refused.
fn geometry_of(
    kind: ObstacleGeometryKindParam,
    f: &GeometryFields<'_>,
) -> Result<ObstacleGeometry, WireRefusal> {
    let stray = |field: &'static str, kind: &str| -> WireRefusal {
        (
            format!("Error: {field} does not apply to kind '{kind}'"),
            field,
        )
    };
    let missing = |field: &'static str, kind: &str| -> WireRefusal {
        (format!("Error: kind '{kind}' needs {field}"), field)
    };
    let ids: Vec<ModelId> = f.model_ids.iter().map(|&id| ModelId(id)).collect();
    match kind {
        ObstacleGeometryKindParam::Box => {
            if !ids.is_empty() {
                return Err(stray("model_ids", "box"));
            }
            for (field, value) in [("z_bottom", f.z_bottom), ("z_top", f.z_top)] {
                if value.is_some() {
                    return Err(stray(field, "box"));
                }
            }
            Ok(ObstacleGeometry::Box {
                min: f.min.ok_or_else(|| missing("min", "box"))?,
                max: f.max.ok_or_else(|| missing("max", "box"))?,
            })
        }
        ObstacleGeometryKindParam::OutlineExtrude => {
            for (field, value) in [("min", f.min), ("max", f.max)] {
                if value.is_some() {
                    return Err(stray(field, "outline_extrude"));
                }
            }
            Ok(ObstacleGeometry::OutlineExtrude {
                model_ids: ids,
                z_bottom: f
                    .z_bottom
                    .ok_or_else(|| missing("z_bottom", "outline_extrude"))?,
                z_top: f.z_top.ok_or_else(|| missing("z_top", "outline_extrude"))?,
            })
        }
        ObstacleGeometryKindParam::Model => {
            for (field, value) in [("min", f.min), ("max", f.max)] {
                if value.is_some() {
                    return Err(stray(field, "model"));
                }
            }
            for (field, value) in [("z_bottom", f.z_bottom), ("z_top", f.z_top)] {
                if value.is_some() {
                    return Err(stray(field, "model"));
                }
            }
            let [model_id] = ids.as_slice() else {
                return Err((
                    format!(
                        "Error: kind 'model' takes exactly one model id, and the request gives {}",
                        ids.len()
                    ),
                    "model_ids",
                ));
            };
            Ok(ObstacleGeometry::Model {
                model_id: *model_id,
            })
        }
    }
}

/// Patch the current geometry with the fields the request gives, without a
/// kind change.
fn patch_geometry(
    current: &ObstacleGeometry,
    p: &EditObstacleParam,
) -> Result<ObstacleGeometry, WireRefusal> {
    match current {
        ObstacleGeometry::Box { min, max } => geometry_of(
            ObstacleGeometryKindParam::Box,
            &GeometryFields {
                min: Some(p.min.unwrap_or(*min)),
                max: Some(p.max.unwrap_or(*max)),
                model_ids: p.model_ids.as_deref().unwrap_or_default(),
                z_bottom: p.z_bottom,
                z_top: p.z_top,
            },
        ),
        ObstacleGeometry::OutlineExtrude {
            model_ids,
            z_bottom,
            z_top,
        } => {
            let ids: Vec<usize> = p
                .model_ids
                .clone()
                .unwrap_or_else(|| model_ids.iter().map(|id| id.0).collect());
            geometry_of(
                ObstacleGeometryKindParam::OutlineExtrude,
                &GeometryFields {
                    min: p.min,
                    max: p.max,
                    model_ids: &ids,
                    z_bottom: Some(p.z_bottom.unwrap_or(*z_bottom)),
                    z_top: Some(p.z_top.unwrap_or(*z_top)),
                },
            )
        }
        ObstacleGeometry::Model { model_id } => {
            let ids: Vec<usize> = p.model_ids.clone().unwrap_or_else(|| vec![model_id.0]);
            geometry_of(
                ObstacleGeometryKindParam::Model,
                &GeometryFields {
                    min: p.min,
                    max: p.max,
                    model_ids: &ids,
                    z_bottom: p.z_bottom,
                    z_top: p.z_top,
                },
            )
        }
    }
}

fn refuse((message, field): WireRefusal) -> CorePlan {
    CorePlan::Answered(CoreReply::refused(&message, Some(field)))
}

impl RsCamApp {
    /// The obstacles of the setup at `setup_index`, or the refusal reply
    /// that names the index.
    fn obstacle_setup(&self, setup_index: usize) -> Result<Vec<Obstacle>, CoreReply> {
        self.controller
            .state()
            .session()
            .list_setups()
            .get(setup_index)
            .map(|setup| setup.obstacles.clone())
            .ok_or_else(|| {
                CoreReply::refused(
                    &format!("Error: Setup index {setup_index} not found"),
                    Some("setup_index"),
                )
            })
    }

    /// `add_obstacle` → `Command::AddObstacle`.
    pub(super) fn plan_add_obstacle(
        &self,
        p: &AddObstacleParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let obstacles = match self.obstacle_setup(p.setup_index) {
            Ok(obstacles) => obstacles,
            Err(reply) => return CorePlan::Answered(reply),
        };
        let geometry = match geometry_of(
            p.kind,
            &GeometryFields {
                min: p.min,
                max: p.max,
                model_ids: &p.model_ids,
                z_bottom: p.z_bottom,
                z_top: p.z_top,
            },
        ) {
            Ok(geometry) => geometry,
            Err(refusal) => return refuse(refusal),
        };
        let id = next_obstacle_id(&obstacles);
        let obstacle = Obstacle {
            id,
            name: p
                .name
                .clone()
                .unwrap_or_else(|| format!("Obstacle {}", id.0 + 1)),
            enabled: p.enabled.unwrap_or(true),
            clearance: p.clearance.unwrap_or(DEFAULT_OBSTACLE_CLEARANCE_MM),
            geometry,
        };
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "obstacle_id": id.0 });
        CorePlan::Apply(
            Command::AddObstacle(AddObstacleArgs {
                setup_index: p.setup_index,
                obstacle: Box::new(obstacle),
            }),
            Box::new(before),
        )
    }

    /// `edit_obstacle` → `Command::ReplaceObstacle` with the stored record
    /// patched by the given fields.
    pub(super) fn plan_edit_obstacle(
        &self,
        p: &EditObstacleParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let obstacles = match self.obstacle_setup(p.setup_index) {
            Ok(obstacles) => obstacles,
            Err(reply) => return CorePlan::Answered(reply),
        };
        let obstacle_id = ObstacleId(p.obstacle_id);
        let Some(stored) = obstacles.iter().find(|o| o.id == obstacle_id) else {
            return CorePlan::Answered(CoreReply::refused(
                &format!(
                    "Error: setup {} carries no obstacle with id {}",
                    p.setup_index, p.obstacle_id
                ),
                Some("obstacle_id"),
            ));
        };
        let mut obstacle = stored.clone();
        if let Some(name) = &p.name {
            obstacle.name.clone_from(name);
        }
        if let Some(enabled) = p.enabled {
            obstacle.enabled = enabled;
        }
        if let Some(clearance) = p.clearance {
            obstacle.clearance = clearance;
        }
        let geometry = match p.kind {
            Some(kind) => {
                let ids = p.model_ids.clone().unwrap_or_else(|| {
                    obstacle
                        .geometry
                        .model_ids()
                        .iter()
                        .map(|id| id.0)
                        .collect()
                });
                geometry_of(
                    kind,
                    &GeometryFields {
                        min: p.min,
                        max: p.max,
                        model_ids: &ids,
                        z_bottom: p.z_bottom,
                        z_top: p.z_top,
                    },
                )
            }
            None => patch_geometry(&obstacle.geometry, p),
        };
        obstacle.geometry = match geometry {
            Ok(geometry) => geometry,
            Err(refusal) => return refuse(refusal),
        };
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "obstacle_id": p.obstacle_id });
        CorePlan::Apply(
            Command::ReplaceObstacle(ReplaceObstacleArgs {
                setup_index: p.setup_index,
                obstacle_id,
                obstacle: Box::new(obstacle),
            }),
            Box::new(before),
        )
    }

    /// `remove_obstacle` → `Command::RemoveObstacle`. Core refuses an
    /// unknown id.
    pub(super) fn plan_remove_obstacle(
        &self,
        p: &RemoveObstacleParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        if let Err(reply) = self.obstacle_setup(p.setup_index) {
            return CorePlan::Answered(reply);
        }
        before.index = Some(p.setup_index);
        before.extra = serde_json::json!({ "obstacle_id": p.obstacle_id });
        CorePlan::Apply(
            Command::RemoveObstacle(RemoveObstacleArgs {
                setup_index: p.setup_index,
                obstacle_id: ObstacleId(p.obstacle_id),
            }),
            Box::new(before),
        )
    }

    /// The reply of an applied obstacle row: the setup's list after the
    /// write, and the stale set.
    pub(super) fn describe_obstacle(
        &mut self,
        verb: &str,
        effects: &Effects,
        before: &CoreBefore,
    ) -> CoreReply {
        self.controller.state_mut().gui.mark_edited();
        self.controller.set_pending_upload();
        let setup_index = before.index();
        let obstacle_id = before
            .extra
            .get("obstacle_id")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        // K2: a removed obstacle closes its editor and gives the selection
        // to its setup (the S4 rule). An edit closes an editor open on the
        // obstacle too: its draft predates the edit, and the page re-seeds
        // it from the new record on the next frame.
        let setup_id = self
            .controller
            .state()
            .session()
            .list_setups()
            .get(setup_index)
            .map(|setup| crate::state::job::SetupId(setup.id));
        if let Some(setup_id) = setup_id
            && let Ok(raw) = usize::try_from(obstacle_id)
        {
            let state = self.controller.state_mut();
            let id = ObstacleId(raw);
            if verb == "Removed" {
                crate::state::panels::forget_obstacle(
                    &mut state.panels,
                    &mut state.selection,
                    setup_id,
                    id,
                );
            } else if state
                .panels
                .obstacle_editor
                .as_ref()
                .is_some_and(|e| e.setup_id == setup_id && e.obstacle_id == id)
            {
                state.panels.obstacle_editor = None;
            }
        }
        let stale = self.core_stale(effects);
        let rows: Vec<serde_json::Value> = self
            .controller
            .state()
            .session()
            .list_setups()
            .get(setup_index)
            .map(|setup| setup.obstacles.iter().map(obstacle_json).collect())
            .unwrap_or_default();
        CoreReply::succeeded(self.mcp_mutation_result(
            format!("{verb} obstacle {obstacle_id} in setup {setup_index}"),
            serde_json::json!({
                "setup_index": setup_index,
                "obstacle_id": obstacle_id,
                "simulation_cleared": effects.simulation_cleared,
                "obstacles": rows,
            }),
            stale,
            &before.diagnostics,
        ))
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed conversion is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// K2: the obstacle JSON names the kind once, as the serde tag that
    /// equals the `add_obstacle` input spelling.
    #[test]
    fn the_obstacle_json_names_the_kind_once_k2() {
        let mut obstacle = Obstacle::new_default_box(ObstacleId(0));
        obstacle.geometry = ObstacleGeometry::OutlineExtrude {
            model_ids: vec![ModelId(1)],
            z_bottom: -5.0,
            z_top: 5.0,
        };
        let json = obstacle_json(&obstacle);
        assert!(json.get("kind").is_none(), "no top-level kind: {json}");
        assert_eq!(json["geometry"]["kind"], "outline_extrude");
    }

    #[test]
    fn each_kind_needs_its_own_fields_and_refuses_the_others() {
        let empty = GeometryFields {
            min: None,
            max: None,
            model_ids: &[],
            z_bottom: None,
            z_top: None,
        };
        assert_eq!(
            geometry_of(
                ObstacleGeometryKindParam::Box,
                &GeometryFields {
                    min: Some([0.0; 3]),
                    max: Some([1.0; 3]),
                    ..empty
                }
            ),
            Ok(ObstacleGeometry::Box {
                min: [0.0; 3],
                max: [1.0; 3]
            })
        );
        assert_eq!(
            geometry_of(ObstacleGeometryKindParam::Box, &empty)
                .unwrap_err()
                .1,
            "min"
        );
        assert_eq!(
            geometry_of(
                ObstacleGeometryKindParam::Model,
                &GeometryFields {
                    model_ids: &[1, 2],
                    ..empty
                }
            )
            .unwrap_err()
            .1,
            "model_ids"
        );
        assert_eq!(
            geometry_of(
                ObstacleGeometryKindParam::OutlineExtrude,
                &GeometryFields {
                    model_ids: &[3],
                    z_bottom: Some(0.0),
                    min: Some([0.0; 3]),
                    ..empty
                }
            )
            .unwrap_err()
            .1,
            "min"
        );
    }
}
