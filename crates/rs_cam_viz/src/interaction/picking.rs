use crate::render::camera::OrbitCamera;
use crate::state::Workspace;
use crate::state::display_frame::{
    POINT_MARK_RADIUS_MM, drawing_display_z, drawing_plane_z, item_display_point, item_display_xy,
    toolpath_setup_frame,
};
use crate::state::job::{ModelId, ObstacleId, SetupFrame, SetupId, StockConfig};
use crate::state::overlays::ModelVisibility;
use crate::state::pick::PickFilter;
use crate::state::pin_view::pin_display_points;
use crate::state::runtime::GuiState;
use crate::state::toolpath::ToolpathId;
use crate::state::viewport::{ToolpathDrawFilter, toolpaths_to_draw};
use rs_cam_core::geo::{BoundingBox3, P2, P3, V3};
use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
use rs_cam_core::selection::ItemKind;
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
    /// position) can use it to look up the corresponding [`Move`](rs_cam_core::toolpath::Move) and
    /// [`Span`](rs_cam_core::trace::toolpath_spans::Span) data.
    Toolpath { id: ToolpathId, move_index: usize },
    /// Hit a BREP face on an enriched mesh model.
    ModelFace {
        model_id: ModelId,
        face_id: FaceGroupId,
    },
    /// Hit the mesh of a model with no BREP faces (an STL), in pick mode
    /// only (geometry selection P4.7). `point` is the hit in the MODEL
    /// frame; `ProjectSession::face_pick_key_at` names the flat-region face
    /// under it.
    MeshFace { model_id: ModelId, point: P3 },
    /// Hit a drawn item of the active toolpath: a hole marker of a drill op
    /// (geometry selection P2.6), or in pick mode any item the filter
    /// admits (P4.5, [`pick_items`]). `key` is the match key a pick of the
    /// item stores.
    Item {
        toolpath_id: ToolpathId,
        key: rs_cam_core::selection::MatchKey,
    },
}

/// The pick under the pointer, computed once per frame (B1).
#[derive(Debug, Clone, Default)]
pub struct HoverResult {
    /// The pointer position, or `None` when no pick ran.
    pub pos: Option<egui::Pos2>,
    /// Pick mode: the key (or its refusal) and the chip-off kind.
    pub pick_mode: Option<(
        Result<rs_cam_core::selection::MatchKey, String>,
        Option<ItemKind>,
    )>,
    /// The scene pick (`pick` with `drill_pick: None`): the face hover and
    /// the toolpath tooltip read it.
    pub scene: Option<PickHit>,
}

/// Context for a pick operation, bundling camera and viewport parameters.
pub struct PickContext<'a> {
    pub camera: &'a OrbitCamera,
    pub screen_x: f32,
    pub screen_y: f32,
    pub aspect: f32,
    pub vw: f32,
    pub vh: f32,
    /// The models the viewport draws (the eye of each model row,
    /// `OverlayPanelState::model_visibility`). A pick tests only what the
    /// viewport draws, so no pick hits a hidden model: not its mesh, its
    /// faces, its drawing items, its hole markers or its loops.
    pub visibility: &'a ModelVisibility,
}

impl PickContext<'_> {
    /// True when the viewport draws the model `model_id`, so a pick may
    /// hit it.
    fn shows(&self, model_id: rs_cam_core::ids::ModelId) -> bool {
        self.visibility.is_visible(model_id)
    }
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
#[expect(
    clippy::too_many_arguments,
    reason = "one flat pick entry; each argument is a distinct input the viewport already holds."
)]
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
        && let Some((_, setup)) = session.find_setup_by_id(*setup_id)
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

    // Model face picking (enriched mesh only, Toolpaths workspace).
    //
    // Geometry selection P2.6: the upload draws the enriched mesh moved by
    // the display frame (`SetupFrame::transform_point`). The ray moves back
    // into the model frame through the inverse of that one transform, so a
    // click hits the face where it is drawn. The transform is rigid (a
    // flip, a turn and a shift), so the ray parameter `t` keeps its length
    // and compares with the other hits.
    if matches!(workspace, Workspace::Toolpaths)
        && let Some((t, hit)) = pick_model_face(ctx, session, display_frame, origin, dir, false)
        && t < best_t
    {
        best_hit = Some(hit);
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

/// The nearest BREP face hit by the ray `origin` + t `dir` (display
/// frame), over the enriched mesh of every model, with its ray parameter.
///
/// Geometry selection P2.6: the upload draws the enriched mesh moved by the
/// display frame (`SetupFrame::transform_point`). The ray moves back into
/// the model frame through the inverse of that one transform, so a click
/// hits the face where it is drawn. The transform is rigid (a flip, a turn
/// and a shift), so `t` keeps its length and compares with other hits.
///
/// With `meshes` (pick mode only, geometry selection P4.7), the mesh of a
/// model with no BREP faces (an STL) is hit too, as [`PickHit::MeshFace`]
/// with the hit point in the model frame. A plain click does not test it,
/// so a click on a terrain keeps its old meaning.
fn pick_model_face(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    display_frame: Option<&SetupFrame>,
    origin: P3,
    dir: V3,
    meshes: bool,
) -> Option<(f64, PickHit)> {
    let stock = session.stock_config();
    let (model_origin, model_dir) = match display_frame {
        Some(frame) => {
            let a = frame.inverse_transform_point(origin, stock);
            let b = frame.inverse_transform_point(origin + dir, stock);
            (a, b - a)
        }
        None => (origin, dir),
    };
    let mut best: Option<(f64, PickHit)> = None;
    for model in session.models().iter().filter(|m| ctx.shows(m.id)) {
        // SEL-2: `ray_pick` holds the box gate and casts on the BVH when
        // the pick table has one, else by brute force: the same answer.
        if let Some(enriched) = &model.enriched_mesh {
            if let Some((tri_idx, t)) = model.ray_pick(&model_origin, &model_dir)
                && best.as_ref().is_none_or(|(b, _)| t < *b)
            {
                let face_id = enriched.face_for_triangle(tri_idx);
                best = Some((
                    t,
                    PickHit::ModelFace {
                        model_id: model.id,
                        face_id,
                    },
                ));
            }
        } else if meshes
            && model.faces_are_flat_regions()
            && let Some((_, t)) = model.ray_pick(&model_origin, &model_dir)
            && best.as_ref().is_none_or(|(b, _)| t < *b)
        {
            best = Some((
                t,
                PickHit::MeshFace {
                    model_id: model.id,
                    point: model_origin + model_dir * t,
                },
            ));
        }
    }
    best
}

/// The nearest model surface point under the pointer of `ctx`, in the
/// display frame (A C3a: the orbit pivot and the pan grab point).
///
/// It tests every model the viewport draws (`PickContext::visibility`):
/// the enriched mesh of a STEP model, else the mesh. The ray moves into the
/// model frame through the inverse of the display transform, as
/// [`pick_model_face`] does; the transform is rigid, so the ray parameter
/// gives the point in the display frame. `LoadedModel::ray_pick` casts on
/// the BVH when the pick table has one, else by brute force (SEL-2); it runs
/// once per drag start, never per frame. `pick` does not call it.
pub fn surface_hit(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    display_frame: Option<&SetupFrame>,
) -> Option<P3> {
    let (ray_origin, ray_dir) =
        ctx.camera
            .unproject_ray(ctx.screen_x, ctx.screen_y, ctx.aspect, ctx.vw, ctx.vh)?;
    let origin = P3::new(
        f64::from(ray_origin.x),
        f64::from(ray_origin.y),
        f64::from(ray_origin.z),
    );
    let dir = V3::new(
        f64::from(ray_dir.x),
        f64::from(ray_dir.y),
        f64::from(ray_dir.z),
    );
    let stock = session.stock_config();
    let (model_origin, model_dir) = match display_frame {
        Some(frame) => {
            let a = frame.inverse_transform_point(origin, stock);
            let b = frame.inverse_transform_point(origin + dir, stock);
            (a, b - a)
        }
        None => (origin, dir),
    };
    let best_t = session
        .models()
        .iter()
        .filter(|m| ctx.shows(m.id))
        .filter_map(|model| model.ray_pick(&model_origin, &model_dir).map(|(_, t)| t))
        .min_by(f64::total_cmp)?;
    Some(origin + dir * best_t)
}

/// The pick of a viewport click or of the pointer while a pick mode is on
/// (geometry selection P4.5). It wins over every other pick: the toolpath,
/// the stock, the pins.
///
/// - The drawing items of the toolpath model that `filter` admits
///   ([`PickFilter::admits`] with `kinds`): the item whose drawn lines
///   ([`Shape::pick_lines`], through [`item_display_xy`] on the
///   [`drawing_plane_z`] plane) pass nearest the pointer on the screen,
///   within [`PICK_THRESHOLD_TOOLPATH`] pixels ([`PICK_THRESHOLD_POINT`]
///   for a point). The distance is [`polyline_distance`], the rule of
///   MCP `list_model_items { near }`. So with the loop chip off, this
///   pass picks nothing of a loop and the next item under the pointer
///   wins; [`pick_mode_item`] then tries the kinds whose chip is off.
/// - With no such item and the face chip on: the BREP face under the ray
///   (a STEP face; `ProjectSession::face_pick_key` names its item), or the
///   mesh point under the ray (an STL, P4.7;
///   `ProjectSession::face_pick_key_at` names its flat-region face).
///
/// [`Shape::pick_lines`]: rs_cam_core::selection::Shape::pick_lines
/// [`polyline_distance`]: rs_cam_core::selection::polyline_distance
pub fn pick_items(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
    filter: &PickFilter,
    kinds: &[ItemKind],
) -> Option<PickHit> {
    let (_, tc) = session.find_toolpath_config_by_id(toolpath_id)?;
    let model = session.models().iter().find(|m| m.id == tc.model_id)?;
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, toolpath_id);
    let z = drawing_plane_z(session, tc, frame.as_ref());
    // A hidden model draws no drawing, so none of its items is picked.
    let shown = ctx.shows(model.id);
    let items = model.items.iter().filter(|_| shown);
    let screen = |item: &rs_cam_core::selection::Item, p: &P2| -> Option<[f64; 2]> {
        let q = item_display_point(frame.as_ref(), stock, item, *p, z);
        ctx.camera
            .project_to_screen(
                [q.x as f32, q.y as f32, q.z as f32],
                ctx.aspect,
                ctx.vw,
                ctx.vh,
            )
            .map(|s| [f64::from(s[0]), f64::from(s[1])])
    };
    let pointer = [f64::from(ctx.screen_x), f64::from(ctx.screen_y)];
    // Every item within its threshold: (screen distance, display Z, item).
    let mut hits: Vec<(f64, f64, &rs_cam_core::selection::Item)> = Vec::new();
    for item in items {
        // A face is a surface, not a drawn line: the ray arm below.
        if item.kind() == ItemKind::Face || !filter.admits(kinds, item) {
            continue;
        }
        let threshold = match item.kind() {
            ItemKind::Point => PICK_THRESHOLD_POINT,
            ItemKind::Loop | ItemKind::Path | ItemKind::Circle | ItemKind::Face => {
                PICK_THRESHOLD_TOOLPATH
            }
        };
        let distance = item
            .shape()
            .pick_lines()
            .into_iter()
            .map(|(pts, closed)| {
                let projected: Vec<[f64; 2]> = pts.iter().filter_map(|p| screen(item, p)).collect();
                rs_cam_core::selection::polyline_distance(pointer, projected, closed)
            })
            .fold(f64::INFINITY, f64::min);
        if distance <= f64::from(threshold) {
            let [ax, ay] = item.key().anchor;
            let display_z = item_display_point(frame.as_ref(), stock, item, P2::new(ax, ay), z).z;
            hits.push((distance, display_z, item));
        }
    }
    let surface_z = || {
        let (o, d) =
            ctx.camera
                .unproject_ray(ctx.screen_x, ctx.screen_y, ctx.aspect, ctx.vw, ctx.vh)?;
        let origin = P3::new(f64::from(o.x), f64::from(o.y), f64::from(o.z));
        let dir = V3::new(f64::from(d.x), f64::from(d.y), f64::from(d.z));
        pick_model_face(ctx, session, frame.as_ref(), origin, dir, false)
            .map(|(t, _)| (origin + dir * t).z)
    };
    if let Some(key) = choose_item(&hits, surface_z) {
        return Some(PickHit::Item {
            toolpath_id,
            key: key.clone(),
        });
    }
    if !kinds.contains(&ItemKind::Face) {
        return None;
    }
    let (ray_origin, ray_dir) =
        ctx.camera
            .unproject_ray(ctx.screen_x, ctx.screen_y, ctx.aspect, ctx.vw, ctx.vh)?;
    let origin = P3::new(
        ray_origin.x as f64,
        ray_origin.y as f64,
        ray_origin.z as f64,
    );
    let dir = V3::new(ray_dir.x as f64, ray_dir.y as f64, ray_dir.z as f64);
    pick_model_face(ctx, session, frame.as_ref(), origin, dir, true).map(|(_, hit)| hit)
}

/// A screen rectangle in viewport-local points (B6, box select).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl ScreenRect {
    /// The rectangle with corners `a` and `b`, in any order.
    pub fn from_corners(a: [f64; 2], b: [f64; 2]) -> Self {
        Self {
            min: [a[0].min(b[0]), a[1].min(b[1])],
            max: [a[0].max(b[0]), a[1].max(b[1])],
        }
    }

    fn holds(&self, p: [f64; 2]) -> bool {
        p[0] >= self.min[0] && p[0] <= self.max[0] && p[1] >= self.min[1] && p[1] <= self.max[1]
    }

    fn centre(&self) -> [f64; 2] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
        ]
    }

    /// True when the segment `a`-`b` crosses an edge of the rectangle.
    fn crosses(&self, a: [f64; 2], b: [f64; 2]) -> bool {
        let [x0, y0] = self.min;
        let [x1, y1] = self.max;
        let edges = [
            ([x0, y0], [x1, y0]),
            ([x1, y0], [x1, y1]),
            ([x1, y1], [x0, y1]),
            ([x0, y1], [x0, y0]),
        ];
        edges.iter().any(|&(c, d)| segments_cross(a, b, c, d))
    }
}

/// True when the segments `a`-`b` and `c`-`d` meet (touching counts).
fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let on = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        r[0] >= p[0].min(q[0])
            && r[0] <= p[0].max(q[0])
            && r[1] >= p[1].min(q[1])
            && r[1] <= p[1].max(q[1])
    };
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    if ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0))
        && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0))
    {
        return true;
    }
    (o1 == 0.0 && on(a, b, c))
        || (o2 == 0.0 && on(a, b, d))
        || (o3 == 0.0 && on(c, d, a))
        || (o4 == 0.0 && on(c, d, b))
}

/// True when `p` is inside the closed polygon `ring` (even-odd rule).
fn ring_holds(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (Some(a), Some(b)) = (ring.get(i), ring.get((i + n - 1) % n)) else {
            continue;
        };
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// The keys of the items of the model of `toolpath_id` that a box selects
/// (B6, design B §3.3). `ctx.screen_x/y` are not read; `rect` is in
/// viewport-local points. `kinds` are the chips that are on
/// (`PickFilter::effective_kinds`): a box ignores a chip-off kind (a click
/// turns a chip on; a box does not).
///
/// Each line of an item that `filter` admits projects as the drawing draws
/// it (`item_display_point`, a face at its own Z). Enclose: every vertex
/// projects into the rectangle (a vertex behind the eye fails the item).
/// Crossing: a vertex is in the rectangle, or a segment crosses an edge;
/// a face also when the rectangle centre is inside its outer ring and
/// outside its holes. The key follows the `face_pick_key` rule: the
/// toolpath model is stored as `ToolpathModel`. An STL takes its faces from
/// the flat-region cache (`peek_flat_regions`; a cold cache gives none).
/// Import order, no duplicates. A hidden model gives nothing.
pub fn pick_items_in_rect(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
    filter: &PickFilter,
    kinds: &[ItemKind],
    rect: ScreenRect,
    mode: crate::state::viewport_input::BoxMode,
) -> Vec<rs_cam_core::selection::MatchKey> {
    use crate::state::viewport_input::BoxMode;
    let Some((_, tc)) = session.find_toolpath_config_by_id(toolpath_id) else {
        return Vec::new();
    };
    let Some(model) = session.models().iter().find(|m| m.id == tc.model_id) else {
        return Vec::new();
    };
    if !ctx.shows(model.id) {
        return Vec::new();
    }
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, toolpath_id);
    let z = drawing_plane_z(session, tc, frame.as_ref());
    let screen = |item: &rs_cam_core::selection::Item, p: &P2| -> Option<[f64; 2]> {
        let q = item_display_point(frame.as_ref(), stock, item, *p, z);
        ctx.camera
            .project_to_screen(
                [q.x as f32, q.y as f32, q.z as f32],
                ctx.aspect,
                ctx.vw,
                ctx.vh,
            )
            .map(|s| [f64::from(s[0]), f64::from(s[1])])
    };
    let flat = match (&model.mesh, model.faces_are_flat_regions()) {
        (Some(mesh), true) => rs_cam_core::maps::geom_cache::peek_flat_regions(mesh),
        _ => None,
    };
    let items = model
        .items
        .iter()
        .chain(flat.iter().flat_map(|set| set.iter()));
    let mut keys: Vec<rs_cam_core::selection::MatchKey> = Vec::new();
    for item in items {
        if !filter.admits(kinds, item) {
            continue;
        }
        // Each line, projected; `None` for a vertex behind the eye.
        let lines: Vec<(Vec<Option<[f64; 2]>>, bool)> = item
            .shape()
            .pick_lines()
            .into_iter()
            .map(|(pts, closed)| (pts.iter().map(|p| screen(item, p)).collect(), closed))
            .collect();
        let taken = match mode {
            BoxMode::Enclose => {
                !lines.is_empty()
                    && lines
                        .iter()
                        .all(|(pts, _)| pts.iter().all(|p| p.is_some_and(|p| rect.holds(p))))
            }
            BoxMode::Crossing => {
                let touches = lines.iter().any(|(pts, closed)| {
                    let shown: Vec<[f64; 2]> = pts.iter().flatten().copied().collect();
                    if shown.iter().any(|p| rect.holds(*p)) {
                        return true;
                    }
                    let mut segments: Vec<([f64; 2], [f64; 2])> = shown
                        .windows(2)
                        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
                        .collect();
                    if *closed
                        && shown.len() > 2
                        && let (Some(first), Some(last)) = (shown.first(), shown.last())
                    {
                        segments.push((*last, *first));
                    }
                    segments.iter().any(|&(a, b)| rect.crosses(a, b))
                });
                let face_holds_centre = item.kind() == ItemKind::Face && {
                    let rings: Vec<Vec<[f64; 2]>> = lines
                        .iter()
                        .map(|(pts, _)| pts.iter().flatten().copied().collect())
                        .collect();
                    let centre = rect.centre();
                    rings.first().is_some_and(|outer| ring_holds(outer, centre))
                        && !rings.iter().skip(1).any(|hole| ring_holds(hole, centre))
                };
                touches || face_holds_centre
            }
        };
        if !taken {
            continue;
        }
        let key = rs_cam_core::selection::MatchKey {
            source: rs_cam_core::selection::SourceRef::ToolpathModel,
            ..item.key().clone()
        };
        if !keys.iter().any(|k| k.names_same_geometry(&key)) {
            keys.push(key);
        }
    }
    keys
}

/// A pick-mode hit (P4, reference audit 2026-10-23), and the kind whose
/// chip is off when the hit came from the second pass.
#[derive(Debug, Clone)]
pub struct PickModeHit {
    pub hit: PickHit,
    /// `Some(kind)` when the chip of the hit kind is off: the click turns
    /// that chip on and picks the item.
    pub chip_off: Option<ItemKind>,
}

/// THE pick of a pick-mode click and of the pick-mode hover (P4). The
/// chips are a view filter, not a gate:
///
/// 1. [`pick_items`] over `on_kinds` (the chips that are on, served and
///    held: `PickFilter::effective_kinds`). An item of an on chip wins.
/// 2. With no hit, [`pick_items`] over the kinds of `cut_kinds`
///    (`PickFilter::cut_kinds`) whose chip is off. A hit there names its
///    kind in [`PickModeHit::chip_off`].
///
/// The circle range of `filter` applies in both passes. A kind that the op
/// cannot cut is in neither list, so no pick refuses `KindNotCut`.
pub fn pick_mode_item(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
    filter: &PickFilter,
    on_kinds: &[ItemKind],
    cut_kinds: &[ItemKind],
) -> Option<PickModeHit> {
    if let Some(hit) = pick_items(ctx, session, toolpath_id, filter, on_kinds) {
        return Some(PickModeHit {
            hit,
            chip_off: None,
        });
    }
    let off: Vec<ItemKind> = cut_kinds
        .iter()
        .copied()
        .filter(|k| !on_kinds.contains(k))
        .collect();
    if off.is_empty() {
        return None;
    }
    let hit = pick_items(ctx, session, toolpath_id, filter, &off)?;
    let kind = match &hit {
        PickHit::Item { key, .. } => key.kind,
        PickHit::ModelFace { .. } | PickHit::MeshFace { .. } => ItemKind::Face,
        PickHit::CollisionMarker { .. }
        | PickHit::AlignmentPin { .. }
        | PickHit::Obstacle { .. }
        | PickHit::StockFace { .. }
        | PickHit::Toolpath { .. } => return None,
    };
    Some(PickModeHit {
        hit,
        chip_off: Some(kind),
    })
}

/// The item a click picks among `hits` (screen distance, display Z, item;
/// in import order). The nearest on the screen wins. Two or more within
/// [`PICK_TIE_PX`] of the nearest coincide to the eye (a STEP ring at Z 32
/// over a ring at Z 24, UX-7): the one whose Z is nearest the model
/// surface under the pointer (`surface_z`) wins; with no surface hit, the
/// highest. Equal Z keeps the import order, so the choice is fixed.
fn choose_item<'a>(
    hits: &[(f64, f64, &'a rs_cam_core::selection::Item)],
    surface_z: impl FnOnce() -> Option<f64>,
) -> Option<&'a rs_cam_core::selection::MatchKey> {
    let nearest = hits.iter().map(|h| h.0).fold(f64::INFINITY, f64::min);
    let tied: Vec<&(f64, f64, &rs_cam_core::selection::Item)> = hits
        .iter()
        .filter(|h| h.0 <= nearest + PICK_TIE_PX)
        .collect();
    let first = *tied.first()?;
    if tied.len() == 1 || tied.iter().all(|h| h.1 == first.1) {
        // One candidate, or all on one plane: the nearest on the screen.
        return tied
            .iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|h| h.2.key());
    }
    let score = match surface_z() {
        Some(at) => tied
            .iter()
            .min_by(|a, b| (a.1 - at).abs().total_cmp(&(b.1 - at).abs())),
        None => tied.iter().min_by(|a, b| b.1.total_cmp(&a.1)),
    };
    score.map(|h| h.2.key())
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

/// Where the hole item at model point `xy` is drawn and picked: the setup
/// transform of the point (`SetupFrame::transform_point`, the transform of
/// the drawing lines), on the displayed stock top plus
/// [`HOLE_MARK_LIFT_MM`](crate::state::display_frame::HOLE_MARK_LIFT_MM). `frame` is the setup of the drill toolpath;
/// `None` draws the raw model point on the world stock top.
///
/// Geometry selection P2.6: the upload draws each marker here and
/// [`pick`] tests a click here, so the two cannot disagree. The pick used
/// the raw model XY on the world stock top, so on a flipped or rotated
/// setup a click on a marker picked another hole or none.
#[must_use]
pub fn hole_display_point(frame: Option<&SetupFrame>, stock: &StockConfig, xy: P2) -> P3 {
    let p = item_display_xy(frame, stock, xy);
    P3::new(p.x, p.y, drawing_display_z(frame, stock))
}

/// Two pick candidates closer than this on the screen (pixels) are one
/// line to the eye: the pick then prefers the item nearest in Z to the
/// model surface under the pointer (UX-7, coincident STEP items).
const PICK_TIE_PX: f64 = 2.0;

/// The smallest radius (mm) of a drill marker of a circle (was the literal
/// floor in `app/gpu_upload.rs` until P4.4).
pub const HOLE_MARK_MIN_RADIUS_MM: f64 = 1.0;

/// The display frame of a selected model (geometry selection P4.9): the
/// frame `state::pin_view::active_display_setup` gives a model selection.
fn model_display_frame(session: &ProjectSession, model_id: ModelId) -> Option<SetupFrame> {
    crate::state::pin_view::active_display_setup(
        session,
        &crate::state::selection::Selection::Model(model_id),
    )
    .map(|(frame, _)| frame)
}

/// "Show me" of an open shape (geometry selection P4.9): the box of the
/// items `keys` of the model `model_id`, where the viewport draws a
/// selected model. `None` when no key names an item of the model.
#[must_use]
pub fn model_items_bounds(
    session: &ProjectSession,
    model_id: ModelId,
    keys: &[rs_cam_core::selection::MatchKey],
) -> Option<BoundingBox3> {
    let model = session.models().iter().find(|m| m.id == model_id)?;
    let stock = session.stock_config();
    let frame = model_display_frame(session, model_id);
    let z = drawing_display_z(frame.as_ref(), stock);
    let points: Vec<P2> = keys
        .iter()
        .filter_map(|key| {
            model
                .items
                .iter()
                .find(|i| key.names_same_geometry(i.key()))
        })
        .flat_map(|item| item.shape().pick_lines())
        .flat_map(|(pts, _)| pts.iter().copied())
        .map(|p| item_display_xy(frame.as_ref(), stock, p))
        .collect();
    let first = points.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in &points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Some(BoundingBox3 {
        min: P3::new(x0, y0, z),
        max: P3::new(x1, y1, z),
    })
}

/// The open path of the model `model_id` under the pointer (geometry
/// selection P4.9, the viewport popover): the nearest one within
/// [`PICK_THRESHOLD_TOOLPATH`] px, tested where the viewport draws a
/// selected model. `None` when no open path is near.
#[must_use]
pub fn pick_open_path(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    model_id: ModelId,
) -> Option<rs_cam_core::selection::MatchKey> {
    let model = session
        .models()
        .iter()
        .find(|m| m.id == model_id && ctx.shows(m.id))?;
    let stock = session.stock_config();
    let frame = model_display_frame(session, model_id);
    let z = drawing_display_z(frame.as_ref(), stock);
    let pointer = [f64::from(ctx.screen_x), f64::from(ctx.screen_y)];
    let mut best: Option<(f64, &rs_cam_core::selection::MatchKey)> = None;
    for item in model.items.iter().filter(|i| i.kind() == ItemKind::Path) {
        let distance = item
            .shape()
            .pick_lines()
            .into_iter()
            .map(|(pts, closed)| {
                let projected: Vec<[f64; 2]> = pts
                    .iter()
                    .filter_map(|p| {
                        let q = item_display_xy(frame.as_ref(), stock, *p);
                        ctx.camera
                            .project_to_screen(
                                [q.x as f32, q.y as f32, z as f32],
                                ctx.aspect,
                                ctx.vw,
                                ctx.vh,
                            )
                            .map(|s| [f64::from(s[0]), f64::from(s[1])])
                    })
                    .collect();
                rs_cam_core::selection::polyline_distance(pointer, projected, closed)
            })
            .fold(f64::INFINITY, f64::min);
        if distance <= f64::from(PICK_THRESHOLD_TOOLPATH) && best.is_none_or(|(b, _)| distance < b)
        {
            best = Some((distance, item.key()));
        }
    }
    best.map(|(_, key)| key.clone())
}

/// Pick the nearest hole item (a point or a circle centre) of the model
/// of the active drill toolpath, from the items the session's
/// `hole_selection` lists (geometry selection P2.3c). Each item is tested
/// where the viewport draws it ([`hole_display_point`], P2.6).
fn pick_drill_targets(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
) -> Option<PickHit> {
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id && ctx.shows(tc.model_id))?;
    let holes = session.hole_selection(tc)?;
    if holes.items.is_empty() {
        return None;
    }
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, toolpath_id);

    let mut best_dist = PICK_THRESHOLD_POINT;
    let mut best: Option<rs_cam_core::selection::MatchKey> = None;
    for item in holes.items {
        let p = hole_display_point(frame.as_ref(), stock, item.xy);
        let world = [p.x as f32, p.y as f32, p.z as f32];
        if let Some(screen) = ctx
            .camera
            .project_to_screen(world, ctx.aspect, ctx.vw, ctx.vh)
        {
            let dx = screen[0] - ctx.screen_x;
            let dy = screen[1] - ctx.screen_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best = Some(item.key);
            }
        }
    }
    best.map(|key| PickHit::Item { toolpath_id, key })
}

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
                rt.is_some_and(|r| r.run.result().is_some()),
            )
        }),
    );

    for tc in session.toolpath_configs() {
        let tp_id = tc.id;
        if !draw_set.contains(&tp_id) {
            continue;
        }
        let rt = gui.toolpath_rt.get(&tc.id);
        let result = rt.and_then(|r| r.run.result());
        let Some(result) = result else {
            continue;
        };

        let cursor = [ctx.screen_x, ctx.screen_y];
        let project = |p: [f32; 3]| ctx.camera.project_to_screen(p, ctx.aspect, ctx.vw, ctx.vh);
        if let Some((j, dist)) =
            nearest_drawn_move(&result.toolpath().moves, cursor, best_dist, project)
        {
            best_dist = dist;
            best = Some((tp_id, j));
        }
    }

    best.map(|(id, move_index)| PickHit::Toolpath { id, move_index })
}

/// The move of one toolpath nearest to `cursor` on screen, as
/// `(move index, distance in px)`, when the distance is less than
/// `threshold`. `project` takes a point of `moves` to the screen.
///
/// The pick tests about 200 moves of a toolpath (every Nth move). It walks
/// each tested move as the viewport draws it
/// ([`move_draw_points_into`](crate::render::toolpath_render::move_draw_points_into)),
/// and measures to the nearest drawn segment on screen. Thus a click on an
/// arc picks the arc move, not only a click near its chord ends (R-ARC).
/// A rapid tests its target point only.
pub fn nearest_drawn_move(
    moves: &[rs_cam_core::toolpath::Move],
    cursor: [f32; 2],
    threshold: f32,
    project: impl Fn([f32; 3]) -> Option<[f32; 2]>,
) -> Option<(usize, f32)> {
    use crate::render::toolpath_render::{arc_draw_deviation_mm, move_draw_points_into};

    let deviation = arc_draw_deviation_mm();
    let screen = |p: P3| project([p.x as f32, p.y as f32, p.z as f32]);
    let mut best: Option<(usize, f32)> = None;
    let mut best_dist = threshold;
    let mut points = Vec::new();
    let step = (moves.len() / 200).max(1);
    for j in (0..moves.len()).step_by(step) {
        let Some(m) = moves.get(j) else { break };
        // A rapid picks at its target only, as before R-ARC: a long rapid
        // must not take a click from a cut line near its middle.
        let prev = j.checked_sub(1).and_then(|k| moves.get(k));
        match prev.filter(|_| m.move_type.is_cutting()) {
            Some(prev) => move_draw_points_into(&mut points, prev.target, m, deviation),
            None => {
                points.clear();
                points.push(m.target);
            }
        }
        let projected: Vec<Option<[f32; 2]>> = points.iter().map(|&p| screen(p)).collect();
        let mut dist = f32::INFINITY;
        for p in projected.iter().flatten() {
            dist = dist.min(screen_distance(cursor, *p, *p));
        }
        for w in projected.windows(2) {
            if let [Some(a), Some(b)] = w {
                dist = dist.min(screen_distance(cursor, *a, *b));
            }
        }
        if dist < best_dist {
            best_dist = dist;
            best = Some((j, dist));
        }
    }
    best
}

/// The screen distance from `p` to the segment `a`..`b`.
fn screen_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (p[0] - (a[0] + t * abx), p[1] - (a[1] + t * aby));
    (dx * dx + dy * dy).sqrt()
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

// ── The hand-tab editor (geometry selection P4.6) ──────────────────────

/// One tab mark as the viewport draws and picks it, in the display frame.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayTab {
    /// The index of the mark in `TabLayoutAnswer::marks`.
    pub mark: usize,
    /// The centre of the lift (display XY). A dragged tab draws at the
    /// snapped point of the drag.
    pub at: P2,
    /// The unit direction of the loop at the mark (display XY). The bar
    /// draws across it.
    pub along: [f64; 2],
    /// True for a hand tab (it draws a handle ring).
    pub hand: bool,
    /// True while the tab is dragged.
    pub dragged: bool,
}

/// The tab marks of the tab editor of one Profile, in the display frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TabScene {
    pub tabs: Vec<DisplayTab>,
    /// The plane the drawing draws on ([`drawing_plane_z`]).
    pub z: f64,
    /// Half the length of a bar: the tool radius. A bar centred on the cut
    /// ring then spans the kerf from the outline to its far edge.
    pub half_bar: f64,
}

/// The ring (model frame) of the loop `key` of the toolpath model of `tc`,
/// as the drawing draws it: a loop, a circle ring or a face outline. `None`
/// for an item of another source.
#[must_use]
pub fn loop_ring(
    session: &ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    key: &rs_cam_core::selection::MatchKey,
) -> Option<Vec<P2>> {
    use rs_cam_core::selection::Shape;
    let item = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)?
        .items
        .iter()
        .find(|i| i.key().names_same_geometry(key))?;
    match item.shape() {
        Shape::Loop(p) => Some(p.exterior.clone()),
        Shape::Circle { ring, .. } => Some(ring.exterior.clone()),
        Shape::Face(face) => Some(face.rings.exterior.clone()),
        Shape::Path(_) | Shape::Point(_) => None,
    }
}

/// The nearest point of the closed ring `ring` (model frame) to the
/// display point `p`, measured where the viewport draws the ring
/// ([`item_display_xy`]). The answer is in the model frame: the display
/// map is affine, so the point at the same fraction of the same segment of
/// the model ring is the snapped point. Also gives the display distance.
#[must_use]
pub fn snap_to_loop(
    frame: Option<&SetupFrame>,
    stock: &StockConfig,
    ring: &[P2],
    p: P2,
) -> Option<([f64; 2], f64)> {
    let mut best: Option<([f64; 2], f64)> = None;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((k + 1) % ring.len()) else {
            continue;
        };
        let (da, db) = (
            item_display_xy(frame, stock, *a),
            item_display_xy(frame, stock, *b),
        );
        let (dx, dy) = (db.x - da.x, db.y - da.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p.x - da.x) * dx + (p.y - da.y) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = (p.x - (da.x + t * dx)).hypot(p.y - (da.y + t * dy));
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some(([a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)], d));
        }
    }
    best
}

/// The unit direction (display frame) of the segment of the closed ring
/// `ring` (model frame) nearest the model point `p`.
fn loop_direction(frame: Option<&SetupFrame>, stock: &StockConfig, ring: &[P2], p: P2) -> [f64; 2] {
    let mut best: Option<(f64, [f64; 2])> = None;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((k + 1) % ring.len()) else {
            continue;
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        if len2 <= 0.0 {
            continue;
        }
        let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
        let d = (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy));
        if best.is_none_or(|(bd, _)| d < bd) {
            let (da, db) = (
                item_display_xy(frame, stock, *a),
                item_display_xy(frame, stock, *b),
            );
            let (ex, ey) = (db.x - da.x, db.y - da.y);
            let len = ex.hypot(ey);
            best = Some((d, [ex / len, ey / len]));
        }
    }
    best.map_or([1.0, 0.0], |(_, dir)| dir)
}

/// The tab marks of the editor `mode` of the toolpath `id`, where the
/// viewport draws them. `None` when the editor has no layout.
#[must_use]
pub fn tab_scene(
    session: &ProjectSession,
    id: ToolpathId,
    mode: &crate::state::pick::TabEditMode,
) -> Option<TabScene> {
    use rs_cam_core::dressup::tabs::TabSource;
    let answer = mode.answer()?;
    let (_, tc) = session.find_toolpath_config_by_id(id)?;
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, id);
    let z = drawing_plane_z(session, tc, frame.as_ref());
    let half_bar = session
        .toolpath_tool(tc)
        .map_or(POINT_MARK_RADIUS_MM, |t| t.diameter / 2.0);
    let tabs = answer
        .marks
        .iter()
        .enumerate()
        .map(|(k, m)| {
            let drag_to = mode
                .drag
                .as_ref()
                .filter(|d| d.mark == k)
                .and_then(|d| d.to);
            let on_loop = drag_to.unwrap_or(m.on_loop);
            let at = drag_to.unwrap_or(m.at);
            let along = loop_ring(session, tc, &m.loop_key).map_or([1.0, 0.0], |ring| {
                loop_direction(
                    frame.as_ref(),
                    stock,
                    &ring,
                    P2::new(on_loop[0], on_loop[1]),
                )
            });
            DisplayTab {
                mark: k,
                at: item_display_xy(frame.as_ref(), stock, P2::new(at[0], at[1])),
                along,
                hand: matches!(m.source, TabSource::Hand(_)),
                dragged: drag_to.is_some(),
            }
        })
        .collect();
    Some(TabScene { tabs, z, half_bar })
}

/// The point of the plane `z` (display frame) under the pointer of `ctx`.
#[must_use]
pub fn pointer_on_plane(ctx: &PickContext<'_>, z: f64) -> Option<P2> {
    let (origin, dir) =
        ctx.camera
            .unproject_ray(ctx.screen_x, ctx.screen_y, ctx.aspect, ctx.vw, ctx.vh)?;
    let (oz, dz) = (f64::from(origin.z), f64::from(dir.z));
    if dz.abs() < f64::EPSILON {
        return None;
    }
    let t = (z - oz) / dz;
    Some(P2::new(
        f64::from(origin.x) + t * f64::from(dir.x),
        f64::from(origin.y) + t * f64::from(dir.y),
    ))
}

/// The tab of `scene` under the pointer of `ctx`: the nearest mark on the
/// screen within [`PICK_THRESHOLD_POINT`] pixels.
#[must_use]
pub fn pick_tab(ctx: &PickContext<'_>, scene: &TabScene) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for tab in &scene.tabs {
        let Some(s) = ctx.camera.project_to_screen(
            [tab.at.x as f32, tab.at.y as f32, scene.z as f32],
            ctx.aspect,
            ctx.vw,
            ctx.vh,
        ) else {
            continue;
        };
        let d = (s[0] - ctx.screen_x).hypot(s[1] - ctx.screen_y);
        if d <= PICK_THRESHOLD_POINT && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, tab.mark));
        }
    }
    best.map(|(_, mark)| mark)
}

/// The loop of `loops` under the pointer of `ctx` (the drawn line within
/// [`PICK_THRESHOLD_TOOLPATH`] pixels, the rule of [`pick_items`]) and the
/// point of that loop under the pointer, in the drawing frame.
#[must_use]
pub fn pick_tab_loop(
    ctx: &PickContext<'_>,
    session: &ProjectSession,
    id: ToolpathId,
    loops: &[rs_cam_core::selection::MatchKey],
) -> Option<(rs_cam_core::selection::MatchKey, [f64; 2])> {
    let (_, tc) = session
        .find_toolpath_config_by_id(id)
        .filter(|(_, tc)| ctx.shows(tc.model_id))?;
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, id);
    let z = drawing_plane_z(session, tc, frame.as_ref());
    let pointer = [f64::from(ctx.screen_x), f64::from(ctx.screen_y)];
    let mut best: Option<(f64, &rs_cam_core::selection::MatchKey, Vec<P2>)> = None;
    for key in loops {
        let Some(ring) = loop_ring(session, tc, key) else {
            continue;
        };
        let projected: Vec<[f64; 2]> = ring
            .iter()
            .filter_map(|p| {
                let q = item_display_xy(frame.as_ref(), stock, *p);
                ctx.camera
                    .project_to_screen(
                        [q.x as f32, q.y as f32, z as f32],
                        ctx.aspect,
                        ctx.vw,
                        ctx.vh,
                    )
                    .map(|s| [f64::from(s[0]), f64::from(s[1])])
            })
            .collect();
        let d = rs_cam_core::selection::polyline_distance(pointer, projected, true);
        if d <= f64::from(PICK_THRESHOLD_TOOLPATH) && best.as_ref().is_none_or(|(b, _, _)| d < *b) {
            best = Some((d, key, ring));
        }
    }
    let (_, key, ring) = best?;
    let on_plane = pointer_on_plane(ctx, z)?;
    let (at, _) = snap_to_loop(frame.as_ref(), stock, &ring, on_plane)?;
    Some((key.clone(), at))
}
