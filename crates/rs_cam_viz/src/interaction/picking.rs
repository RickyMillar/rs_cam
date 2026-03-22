use crate::render::camera::OrbitCamera;
use crate::state::Workspace;
use crate::state::job::{FixtureId, JobState, KeepOutId, SetupId};
use crate::state::toolpath::ToolpathId;
use rs_cam_core::geo::{P3, V3};

/// The result of a viewport pick operation.
#[derive(Debug, Clone)]
pub enum PickHit {
    /// Hit a collision marker at the given index in `collision_positions`.
    CollisionMarker { index: usize },
    /// Hit a stock-level alignment pin.
    AlignmentPin { pin_index: usize },
    /// Hit a fixture bounding box.
    Fixture {
        setup_id: SetupId,
        fixture_id: FixtureId,
    },
    /// Hit a keep-out zone bounding box.
    KeepOut {
        setup_id: SetupId,
        keep_out_id: KeepOutId,
    },
    /// Hit the stock bounding box. `face_normal` indicates which face.
    StockFace { face_normal: [f32; 3] },
    /// Hit a toolpath.
    Toolpath { id: ToolpathId },
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
pub fn pick(
    ctx: &PickContext<'_>,
    job: &JobState,
    collision_positions: &[[f32; 3]],
    workspace: Workspace,
    isolate_toolpath: Option<ToolpathId>,
) -> Option<PickHit> {
    // 1. Screen-space picks (small targets first)
    if workspace == Workspace::Simulation
        && let Some(hit) = pick_collision_markers(ctx, collision_positions)
    {
        return Some(hit);
    }

    if workspace == Workspace::Setup
        && let Some(hit) = pick_alignment_pins(ctx, job)
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

    // Fixtures (Setup only)
    if workspace == Workspace::Setup {
        for setup in &job.setups {
            for fixture in &setup.fixtures {
                if !fixture.enabled {
                    continue;
                }
                if let Some(t) = fixture.clearance_bbox().ray_intersect(&origin, &dir)
                    && t < best_t
                {
                    best_t = t;
                    best_hit = Some(PickHit::Fixture {
                        setup_id: setup.id,
                        fixture_id: fixture.id,
                    });
                }
            }
        }
    }

    // Keep-outs (Setup only)
    if workspace == Workspace::Setup {
        for setup in &job.setups {
            for keep_out in &setup.keep_out_zones {
                if !keep_out.enabled {
                    continue;
                }
                let bbox = keep_out.bbox(&job.stock);
                if let Some(t) = bbox.ray_intersect(&origin, &dir)
                    && t < best_t
                {
                    best_t = t;
                    best_hit = Some(PickHit::KeepOut {
                        setup_id: setup.id,
                        keep_out_id: keep_out.id,
                    });
                }
            }
        }
    }

    // Stock (Setup + Toolpaths)
    if matches!(workspace, Workspace::Setup | Workspace::Toolpaths) {
        let stock_bbox = job.stock.bbox();
        if let Some(t) = stock_bbox.ray_intersect(&origin, &dir)
            && t < best_t
        {
            let hit_point = origin + dir * t;
            let face_normal = determine_face_normal(&stock_bbox, &hit_point);
            best_hit = Some(PickHit::StockFace { face_normal });
        }
    }

    if best_hit.is_some() {
        return best_hit;
    }

    // 3. Toolpath screen-space pick (Toolpaths + Simulation)
    if matches!(workspace, Workspace::Toolpaths | Workspace::Simulation)
        && let Some(hit) = pick_toolpaths(ctx, job, isolate_toolpath)
    {
        return Some(hit);
    }

    None
}

fn pick_collision_markers(ctx: &PickContext<'_>, positions: &[[f32; 3]]) -> Option<PickHit> {
    let threshold = 12.0f32;
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

fn pick_alignment_pins(ctx: &PickContext<'_>, job: &JobState) -> Option<PickHit> {
    let threshold = 12.0f32;
    let mut best_dist = threshold;
    let mut best_hit = None;
    let stock_top = (job.stock.origin_z + job.stock.z) as f32;

    for (pin_idx, pin) in job.stock.alignment_pins.iter().enumerate() {
        let world = [pin.x as f32, pin.y as f32, stock_top];
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

fn pick_toolpaths(
    ctx: &PickContext<'_>,
    job: &JobState,
    isolate_toolpath: Option<ToolpathId>,
) -> Option<PickHit> {
    let threshold = 15.0f32;
    let mut best_dist = threshold;
    let mut best_id = None;

    for tp in job.all_toolpaths() {
        if !tp.visible {
            continue;
        }
        if let Some(iso_id) = isolate_toolpath
            && tp.id != iso_id
        {
            continue;
        }
        let result = match &tp.result {
            Some(r) => r,
            None => continue,
        };

        let moves = &result.toolpath.moves;
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
                    best_id = Some(tp.id);
                }
            }
        }
    }

    best_id.map(|id| PickHit::Toolpath { id })
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
