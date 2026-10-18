use crate::render::camera::OrbitCamera;
use crate::state::Workspace;
use crate::state::job::{ModelId, ObstacleId, SetupFrame, SetupId, StockConfig};
use crate::state::pin_view::pin_display_points;
use crate::state::runtime::GuiState;
use crate::state::toolpath::ToolpathId;
use crate::state::viewport::{ToolpathDrawFilter, toolpaths_to_draw};
use rs_cam_core::geo::{BoundingBox3, P3, V3};
use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
use rs_cam_core::mesh::ray_pick_triangle;
use rs_cam_core::session::ProjectSession;

/// Screen-space pick threshold for point-like markers (collision dots, pins).
const PICK_THRESHOLD_POINT: f32 = 12.0;
/// Screen-space pick threshold for toolpath line segments (slightly larger
/// because lines are thin and harder to click precisely).
const PICK_THRESHOLD_TOOLPATH: f32 = 15.0;

/// The result of a viewport pick operation.
#[derive(Debug, Clone)]
pub enum PickHit {
    /// Hit a collision marker at the given index in `collision_positions`.
    CollisionMarker { index: usize },
    /// Hit a stock-level alignment pin.
    AlignmentPin { pin_index: usize },
    /// Hit an obstacle bounding box (K1).
    Obstacle {
        setup_id: SetupId,
        obstacle_id: ObstacleId,
    },
    /// Hit the stock bounding box. `face_normal` indicates which face.
    StockFace { face_normal: [f32; 3] },
    /// Hit a toolpath. `move_index` is the closest sampled move on that
    /// toolpath; consumers needing the cut-context (span path, hovered move
    /// position) can use it to look up the corresponding [`Move`] and
    /// [`Span`] data.
    Toolpath { id: ToolpathId, move_index: usize },
    /// Hit a BREP face on an enriched mesh model.
    ModelFace {
        model_id: ModelId,
        face_id: FaceGroupId,
    },
    /// Hit a drill target (DXF point / circle centre) while a drill toolpath
    /// is the active selection. `xy` is the target position in model space.
    DrillTarget {
        toolpath_id: ToolpathId,
        xy: [f64; 2],
    },
}

/// Context for a pick operation, bundling camera and viewport parameters.
pub struct PickContext<'a> {
    pub camera: &'a OrbitCamera,
    pub screen_x: f32,
    pub screen_y: f32,
    pub aspect: f32,
    pub vw: f32,
    pub vh: f32,
}

/// Run the picking pipeline for a viewport click.
/// Returns the highest-priority hit, or `None`.
///
/// `draw_filter` is the WP27 draw rule. A toolpath the viewport does not draw
/// cannot be picked, because the pick and the GPU upload read ONE function
/// (`state::viewport::toolpaths_to_draw`).
///
/// `display_setup` is the frame and the id of the setup the viewport
/// displays (`state::pin_view::active_display_setup`). The pins are picked
/// where that frame draws them (C8), and only that setup's obstacles are
/// picked (K2).
// SAFETY: one flat pick entry; each argument is a distinct input the
// viewport already holds.
#[allow(clippy::too_many_arguments)]
pub fn pick(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    gui: &GuiState,
    collision_positions: &[[f32; 3]],
    workspace: Workspace,
    draw_filter: ToolpathDrawFilter,
    drill_pick_toolpath: Option<ToolpathId>,
    display_setup: Option<&(SetupFrame, SetupId)>,
) -> Option<PickHit> {
    let display_frame = display_setup.map(|(frame, _)| frame);
    // 1. Screen-space picks (small targets first)
    if workspace == Workspace::Simulation
        && let Some(hit) = pick_collision_markers(ctx, collision_positions)
    {
        return Some(hit);
    }

    // Drill targets take priority while a drill op is being edited, so the
    // user can click points/holes to (de)select them.
    if workspace == Workspace::Toolpaths
        && let Some(tp_id) = drill_pick_toolpath
        && let Some(hit) = pick_drill_targets(ctx, session, tp_id)
    {
        return Some(hit);
    }

    if matches!(workspace, Workspace::Setup | Workspace::Toolpaths)
        && let Some(frame) = display_frame
        && let Some(hit) = pick_alignment_pins(ctx, session.stock_config(), frame)
    {
        return Some(hit);
    }

    // 2. Ray-based picks (3D geometry)
    let (ray_origin, ray_dir) =
        ctx.camera
            .unproject_ray(ctx.screen_x, ctx.screen_y, ctx.aspect, ctx.vw, ctx.vh)?;
    let origin = P3::new(
        ray_origin.x as f64,
        ray_origin.y as f64,
        ray_origin.z as f64,
    );
    let dir = V3::new(ray_dir.x as f64, ray_dir.y as f64, ray_dir.z as f64);

    let mut best_t = f64::INFINITY;
    let mut best_hit: Option<PickHit> = None;

    let stock = session.stock_config();

    // Obstacles (Setup only). K2: the obstacles of the DISPLAYED setup
    // only, the set the renderer draws (`active_display_setup`, the one
    // selector). An obstacle is in its setup frame; the display frame is
    // that frame moved by the setup's emission-to-display shift.
    if workspace == Workspace::Setup
        && let Some((frame, setup_id)) = display_setup
        && let Some((_, setup)) = session.find_setup_by_id(setup_id.0)
    {
        let shift = frame.emission_to_display_shift(stock);
        for obstacle in &setup.obstacles {
            let Some(bb) = crate::state::job::obstacle_display_extent(obstacle, session.models())
            else {
                continue;
            };
            let bbox = BoundingBox3 {
                min: P3::new(bb.min.x + shift.x, bb.min.y + shift.y, bb.min.z + shift.z),
                max: P3::new(bb.max.x + shift.x, bb.max.y + shift.y, bb.max.z + shift.z),
            };
            if let Some(t) = bbox.ray_intersect(&origin, &dir)
                && t < best_t
            {
                best_t = t;
                best_hit = Some(PickHit::Obstacle {
                    setup_id: *setup_id,
                    obstacle_id: obstacle.id,
                });
            }
        }
    }

    // Stock (Setup + Toolpaths)
    if matches!(workspace, Workspace::Setup | Workspace::Toolpaths) {
        let stock_bbox = stock.bbox();
        if let Some(t) = stock_bbox.ray_intersect(&origin, &dir)
            && t < best_t
        {
            let hit_point = origin + dir * t;
            let face_normal = determine_face_normal(&stock_bbox, &hit_point);
            best_hit = Some(PickHit::StockFace { face_normal });
        }
    }

    // Model face picking (enriched mesh only, Toolpaths workspace)
    if matches!(workspace, Workspace::Toolpaths) {
        for model in session.models() {
            if let Some(enriched) = &model.enriched_mesh {
                let mesh = enriched.as_mesh();
                // Fast AABB rejection
                if mesh.bbox.ray_intersect(&origin, &dir).is_none() {
                    continue;
                }
                if let Some((tri_idx, t)) = ray_pick_triangle(mesh, &origin, &dir)
                    && t < best_t
                {
                    best_t = t;
                    let face_id = enriched.face_for_triangle(tri_idx);
                    best_hit = Some(PickHit::ModelFace {
                        model_id: ModelId(model.id),
                        face_id,
                    });
                }
            }
        }
    }

    if best_hit.is_some() {
        return best_hit;
    }

    // 3. Toolpath screen-space pick (Toolpaths + Simulation)
    if matches!(workspace, Workspace::Toolpaths | Workspace::Simulation)
        && let Some(hit) = pick_toolpaths(ctx, session, gui, draw_filter)
    {
        return Some(hit);
    }

    None
}

fn pick_collision_markers(ctx: &PickContext<'_>, positions: &[[f32; 3]]) -> Option<PickHit> {
    let threshold = PICK_THRESHOLD_POINT;
    let mut best_dist = threshold;
    let mut best_idx = None;

    for (i, pos) in positions.iter().enumerate() {
        if let Some(screen) = ctx
            .camera
            .project_to_screen(*pos, ctx.aspect, ctx.vw, ctx.vh)
        {
            let dx = screen[0] - ctx.screen_x;
            let dy = screen[1] - ctx.screen_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }
    }

    best_idx.map(|index| PickHit::CollisionMarker { index })
}

/// The pin whose drawn centre is nearest the click, within
/// [`PICK_THRESHOLD_POINT`] pixels.
///
/// The centre is `state::pin_view::pin_display_point` in `frame`: the
/// point the GPU upload draws the circle at, with the stock origin and the
/// setup transform (C8).
pub fn pick_alignment_pins(
    ctx: &PickContext<'_>,
    stock: &StockConfig,
    frame: &SetupFrame,
) -> Option<PickHit> {
    let mut best_dist = PICK_THRESHOLD_POINT;
    let mut best_hit = None;
    for (pin_idx, centre) in pin_display_points(frame, stock).iter().enumerate() {
        let world = [centre.x as f32, centre.y as f32, centre.z as f32];
        if let Some(screen) = ctx
            .camera
            .project_to_screen(world, ctx.aspect, ctx.vw, ctx.vh)
        {
            let dx = screen[0] - ctx.screen_x;
            let dy = screen[1] - ctx.screen_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_hit = Some(PickHit::AlignmentPin { pin_index: pin_idx });
            }
        }
    }
    best_hit
}

/// Pick the nearest drill target (DXF point / circle centre) of the model
/// referenced by the active drill toolpath. Targets are picked at the stock
/// top plane in raw model XY — the same frame the drill toolpath emits holes.
fn pick_drill_targets(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
) -> Option<PickHit> {
    let model_id = session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map(|tc| tc.model_id)?;
    let model = session.models().iter().find(|m| m.id == model_id)?;
    let targets = model.drill_targets_pre_selection();
    if targets.is_empty() {
        return None;
    }
    let stock = session.stock_config();
    let top_z = (stock.origin_z + stock.z) as f32;

    let mut best_dist = PICK_THRESHOLD_POINT;
    let mut best: Option<[f64; 2]> = None;
    for t in targets.iter() {
        let world = [t.x as f32, t.y as f32, top_z];
        if let Some(screen) = ctx
            .camera
            .project_to_screen(world, ctx.aspect, ctx.vw, ctx.vh)
        {
            let dx = screen[0] - ctx.screen_x;
            let dy = screen[1] - ctx.screen_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best = Some([t.x, t.y]);
            }
        }
    }
    best.map(|xy| PickHit::DrillTarget { toolpath_id, xy })
}

// SAFETY: j bounded by 0..moves.len() via step_by loop
#[allow(clippy::indexing_slicing)]
fn pick_toolpaths(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    gui: &GuiState,
    draw_filter: ToolpathDrawFilter,
) -> Option<PickHit> {
    let threshold = PICK_THRESHOLD_TOOLPATH;
    let mut best_dist = threshold;
    let mut best: Option<(ToolpathId, usize)> = None;

    // WP27 — the same draw rule the GPU upload reads, through the same
    // function. The eye button and the "not generated yet" gate ride in it.
    let draw_set = toolpaths_to_draw(
        draw_filter,
        session.toolpath_configs().iter().map(|tc| {
            let rt = gui.toolpath_rt.get(&tc.id);
            (
                tc.id,
                rt.is_none_or(|r| r.visible),
                rt.is_some_and(|r| r.result.is_some()),
            )
        }),
    );

    for tc in session.toolpath_configs() {
        let tp_id = tc.id;
        if !draw_set.contains(&tp_id) {
            continue;
        }
        let rt = gui.toolpath_rt.get(&tc.id);
        let result = rt.and_then(|r| r.result.as_ref());
        let Some(result) = result else {
            continue;
        };

        let moves = &result.toolpath().moves;
        let step = (moves.len() / 200).max(1);
        for j in (0..moves.len()).step_by(step) {
            let m = &moves[j];
            let world = [m.target.x as f32, m.target.y as f32, m.target.z as f32];
            if let Some(screen) = ctx
                .camera
                .project_to_screen(world, ctx.aspect, ctx.vw, ctx.vh)
            {
                let dx = screen[0] - ctx.screen_x;
                let dy = screen[1] - ctx.screen_y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < best_dist {
                    best_dist = dist;
                    best = Some((tp_id, j));
                }
            }
        }
    }

    best.map(|(id, move_index)| PickHit::Toolpath { id, move_index })
}

/// Determine which face of an AABB was hit based on the intersection point.
fn determine_face_normal(bbox: &rs_cam_core::geo::BoundingBox3, hit: &P3) -> [f32; 3] {
    let eps = 1e-4;
    if (hit.z - bbox.max.z).abs() < eps {
        [0.0, 0.0, 1.0] // top
    } else if (hit.z - bbox.min.z).abs() < eps {
        [0.0, 0.0, -1.0] // bottom
    } else if (hit.x - bbox.max.x).abs() < eps {
        [1.0, 0.0, 0.0] // +X
    } else if (hit.x - bbox.min.x).abs() < eps {
        [-1.0, 0.0, 0.0] // -X
    } else if (hit.y - bbox.max.y).abs() < eps {
        [0.0, 1.0, 0.0] // +Y
    } else {
        [0.0, -1.0, 0.0] // -Y
    }
}
