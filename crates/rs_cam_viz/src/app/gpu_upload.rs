use std::collections::HashMap;

use rs_cam_core::feeds::VendorChiploadBand;

use crate::render::RenderResources;
use crate::render::sim_render;
use crate::render::upload_cache;
use crate::state::job::{SetupFrame, SetupId};
use crate::state::selection::Selection;
use crate::state::simulation::StockVizMode;

use super::RsCamApp;

mod drawing;
mod meshes;
mod overlays;
mod reference;
mod sim;
mod toolpaths;

impl RsCamApp {
    /// Get selected BREP face IDs for rendering highlights.
    /// Reads the faces that the selection of the selected toolpath that a
    /// face click edits resolves to (geometry selection P2.6: the
    /// `selection_status` memo, so a face that a rule matches is lit too;
    /// P4.1: the boundary selection of an op with no selection slot), or
    /// the visual Selection::Face/Faces state otherwise.
    fn selected_face_ids(&self) -> Vec<rs_cam_core::geometry::enriched_mesh::FaceGroupId> {
        let state = self.controller.state();
        match &state.selection {
            Selection::Toolpath(tp_id) => state
                .session()
                .find_toolpath_config_by_id(*tp_id)
                .and_then(|(_, tc)| {
                    let target = rs_cam_core::session::SelectionTarget::for_face_pick(tc);
                    state.session().selection_status(tc, target).ok()
                })
                .map(|report| {
                    report
                        .selected
                        .iter()
                        .filter(|k| k.kind == rs_cam_core::selection::ItemKind::Face)
                        .filter_map(|k| k.brep)
                        .map(rs_cam_core::geometry::enriched_mesh::FaceGroupId)
                        .collect()
                })
                .unwrap_or_default(),
            Selection::Face(_, face_id) => vec![*face_id],
            Selection::Faces(_, face_ids) => face_ids.clone(),
            _ => Vec::new(),
        }
    }

    /// Get the currently hovered face ID (for hover highlighting).
    fn hovered_face_id(&self) -> Option<rs_cam_core::geometry::enriched_mesh::FaceGroupId> {
        self.last_hover_face
    }

    /// Compute per-vertex colors for the sim mesh based on current viz mode.
    ///
    /// `mesh_is_final` says whether `mesh` is the run's final composite mesh.
    /// The deviation colours go only on that mesh; any other mesh draws in
    /// its plain colours (render review 2026-10-09, F4).
    pub(super) fn compute_sim_colors(
        &self,
        mesh: &rs_cam_core::stock::stock_mesh::StockMesh,
        mesh_is_final: bool,
    ) -> Vec<[f32; 3]> {
        let num_verts = mesh.vertices.len() / 3;
        // S5: the plain stock colours show the material of each vertex.
        let palette = self.controller.state().session().stock_material_palette();
        match self.controller.state().simulation.stock_viz_mode {
            StockVizMode::Solid => solid_sim_colors(mesh, &palette),
            StockVizMode::Deviation => sim_render::deviation_colors_for_mesh(
                self.controller
                    .state()
                    .simulation
                    .playback
                    .display_deviations
                    .as_deref(),
                mesh_is_final,
                num_verts,
            )
            .unwrap_or_else(|| solid_sim_colors(mesh, &palette)),
            StockVizMode::ByHeight => {
                rs_cam_core::export::ribbon::height_gradient_colors(&mesh.vertices)
            }
        }
    }

    /// Transform a `StockMesh` from its simulation frame to the setup-local
    /// display frame for the given simulation `move_idx`.
    ///
    /// Every sim mesh arrives in the simulator's ZERO-ROOTED stock-relative
    /// global frame — checkpoint meshes via `transform_stock_mesh_to_global`
    /// and live-scrub meshes off the equally zero-rooted global stock. For
    /// an identity setup that frame IS the display frame, so nothing to do.
    /// Non-identity setups only need the face/rotation part of
    /// `world_to_local`; the origin translation was already applied
    /// upstream.
    ///
    /// This used to subtract `stock.origin` on the identity arm (heights /
    /// setup-frame audit 2026-06-12, finding 1). That shift was correct
    /// against a WORLD-framed mesh, which is what the simulator used to
    /// hand back for identity groups — and it cancelled the mis-registered
    /// carve those groups produced in the zero-rooted global stock, which
    /// is why the viewport looked right while
    /// `screenshot_simulation` (no such compensation) showed the part cut
    /// through. Both halves are fixed at the source now
    /// (G-SIM-IDENTITY-FRAME, 2026-08-19); keeping the shift here would
    /// double-count it.
    // SAFETY: step_by(3) loop with i+1, i+2 bounded by vertices.len() (always multiple of 3)
    #[allow(clippy::indexing_slicing)]
    pub(super) fn transform_mesh_to_local_frame(
        &self,
        mesh: &mut rs_cam_core::stock::stock_mesh::StockMesh,
        move_idx: usize,
    ) {
        let Some((face_up, z_rot, true)) = self.active_setup_orientation(move_idx) else {
            return;
        };
        let stock_cfg = self.controller.state().session().stock_config();
        let (eff_w, eff_d, _) = face_up.effective_stock(stock_cfg.x, stock_cfg.y, stock_cfg.z);
        for i in (0..mesh.vertices.len()).step_by(3) {
            let p = rs_cam_core::geo::P3::new(
                mesh.vertices[i] as f64,
                mesh.vertices[i + 1] as f64,
                mesh.vertices[i + 2] as f64,
            );
            let flipped = face_up.transform_point(p, stock_cfg.x, stock_cfg.y, stock_cfg.z);
            let local = z_rot.transform_point(flipped, eff_w, eff_d);
            mesh.vertices[i] = local.x as f32;
            mesh.vertices[i + 1] = local.y as f32;
            mesh.vertices[i + 2] = local.z as f32;
        }
    }

    /// Upload the hover buffer: the ring of the item under the pointer in
    /// pick mode (geometry selection P4.5). The viewport calls it on a
    /// change of the hovered item only (`HoverDetector`). It rebuilds this
    /// one small buffer and no other, so a pointer move never rebuilds the
    /// drawing buffer (the V8/V13 lesson).
    pub(super) fn upload_hover_lines(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };
        let verts = {
            let state = self.controller.state();
            match (
                self.hover_detector.hovered(),
                crate::state::selection_highlight::highlight_toolpath(state),
            ) {
                (Some(key), Some(tc)) => hover_vertices(state.session(), tc, key),
                _ => Vec::new(),
            }
        };
        let mut renderer = render_state.renderer.write();
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
        resources.hover_lines = if verts.is_empty() {
            None
        } else {
            crate::render::gpu_safety::try_create_buffer(
                &render_state.device,
                &resources.gpu_limits,
                "hover_lines",
                bytemuck::cast_slice(&verts),
                egui_wgpu::wgpu::BufferUsages::VERTEX,
            )
            .map(|vertex_buffer| crate::render::PolygonGpuData {
                vertex_buffer,
                vertex_count: verts.len() as u32,
            })
        };
    }

    /// Upload the tab marks buffer of the tab editor (geometry selection
    /// P4.6). The frame calls it on a change of the layout or of the drag
    /// only (`TabMarksDetector`); it rebuilds this one small buffer.
    pub(super) fn upload_tab_lines(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };
        let verts = {
            let state = self.controller.state();
            match crate::state::tab_edit::TabMarksKey::of(state).map(|key| key.toolpath_id) {
                Some(id) => state
                    .gui
                    .selection_ui
                    .tab_edit(id)
                    .and_then(|mode| {
                        crate::interaction::picking::tab_scene(state.session(), id, mode)
                    })
                    .map(|scene| tab_vertices(&scene))
                    .unwrap_or_default(),
                None => Vec::new(),
            }
        };
        let mut renderer = render_state.renderer.write();
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
        resources.tab_lines = if verts.is_empty() {
            None
        } else {
            crate::render::gpu_safety::try_create_buffer(
                &render_state.device,
                &resources.gpu_limits,
                "tab_lines",
                bytemuck::cast_slice(&verts),
                egui_wgpu::wgpu::BufferUsages::VERTEX,
            )
            .map(|vertex_buffer| crate::render::PolygonGpuData {
                vertex_buffer,
                vertex_count: verts.len() as u32,
            })
        };
    }

    /// Re-upload whatever GPU state has actually changed since the last pass.
    ///
    /// **V8 (2026-08-19).** This used to clear and rebuild *every* buffer in
    /// the scene on any `pending_upload` — and `pending_upload` is set by
    /// roughly forty call sites, including each completed toolpath during
    /// `generate_all` and every selection click. An 8-operation generate
    /// therefore re-transformed and re-uploaded the model mesh, all eight
    /// toolpaths, the stock and the fixtures eight times over.
    ///
    /// The expensive resources now carry content keys
    /// ([`crate::render::upload_cache`]) recording what they were last built
    /// from; each is rebuilt only when its own inputs moved. The forty
    /// setters are unchanged — `set_pending_upload()` still means "look
    /// again", it just no longer means "rebuild everything". Cheap resources
    /// (stock wireframe, origin axes, fixture boxes, polygon outlines,
    /// height planes — tens to a few thousand vertices) are still rebuilt
    /// unconditionally; keying them would cost more in key comparison than
    /// the rebuild costs.
    ///
    /// F132: the pass is one function per slot (`gpu_upload/`). Each keyed
    /// slot reads its inputs once into a [`upload_cache::SlotInputs`] value;
    /// its key derives from that value and its build takes that value, so
    /// the key and the build cannot read different state. The slots run in
    /// the order they always ran.
    pub(super) fn upload_gpu_data(&mut self, frame: &mut eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        let mut renderer = render_state.renderer.write();
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &mut RenderResources = renderer.callback_resources.get_mut().unwrap();
        let stats_before = resources.upload_stats;
        resources.upload_stats.passes += 1;
        let gpu = Gpu {
            device: &render_state.device,
            queue: &render_state.queue,
        };

        let state = self.controller.state();
        let pass = PassFrame::of(state);

        let (plain, enriched) = meshes::inputs(
            state,
            &pass,
            self.selected_face_ids(),
            self.hovered_face_id(),
        );
        meshes::upload(plain, enriched, &gpu, resources);
        drawing::upload(state, &pass, &gpu, resources);
        let stock_bbox = reference::upload_stock(&pass, &gpu, resources);
        reference::upload_origin_axes(&pass, &gpu, resources);
        reference::upload_fixture_lines(state, &pass, &gpu, resources);
        sim::upload_sim_mesh(self, &gpu, resources);
        sim::upload_collisions(
            &sim::CollisionInputs {
                positions: self.controller.collision_positions(),
                shift: pass.display_shift(),
            },
            &gpu,
            resources,
        );
        toolpaths::upload(state, &pass, &gpu, resources);
        toolpaths::upload_height_planes(state, &pass, &stock_bbox, &gpu, resources);
        overlays::upload_rest_heatmap(state, &pass, &gpu, resources);
        overlays::upload_tier_preview(state, &pass, &gpu, resources);
        overlays::upload_area_regions(state, &pass, &gpu, resources);
        overlays::upload_reach(state, &pass, &gpu, resources);

        // V8 instrument. There is no criterion harness for the GUI loop, so
        // the pass reports what it actually rebuilt; `RUST_LOG=rs_cam_viz=debug`
        // turns an 8-op `generate_all` into eight lines that each name one
        // toolpath build, where they used to name eight.
        let delta = resources.upload_stats.since(&stats_before);
        tracing::debug!(
            mesh_builds = delta.mesh_builds,
            enriched_builds = delta.enriched_builds,
            toolpath_builds = delta.toolpath_builds,
            toolpath_reuses = delta.toolpath_reuses,
            collision_builds = delta.collision_builds,
            rest_heatmap_builds = delta.rest_heatmap_builds,
            total_passes = resources.upload_stats.passes,
            "gpu upload pass"
        );
    }
}

/// The GPU handles a slot build needs.
struct Gpu<'a> {
    device: &'a egui_wgpu::wgpu::Device,
    queue: &'a egui_wgpu::wgpu::Queue,
}

/// The display frame of one upload pass: the active setup and the stock it
/// is measured against, read once per pass.
///
/// Everything is always displayed in the active setup's local coordinate
/// frame ("machine view"). Toolpaths, simulation, mesh, stock — all at
/// (0,0,0)-relative local coords. The setup id selects that setup's own
/// fixtures, obstacles and toolpaths; the frame places them. C8: the one
/// selector (`pin_view::active_display_setup`), shared with the pin numbers
/// and the pick.
struct PassFrame {
    /// `None` when no setup is resolvable (world frame).
    setup: Option<(SetupId, SetupFrame)>,
    stock: rs_cam_core::compute::StockConfig,
}

impl PassFrame {
    fn of(state: &crate::state::AppState) -> Self {
        Self {
            setup: crate::state::pin_view::active_display_setup(state.session(), &state.selection)
                .map(|(frame, id)| (id, frame)),
            stock: state.session().stock_config().clone(),
        }
    }

    fn setup_id(&self) -> Option<SetupId> {
        self.setup.map(|(id, _)| id)
    }

    /// The active setup's frame, `None` for the world frame.
    fn frame(&self) -> Option<&SetupFrame> {
        self.setup.as_ref().map(|(_, frame)| frame)
    }

    fn stock(&self) -> &rs_cam_core::compute::StockConfig {
        &self.stock
    }

    /// Emission-frame → display-frame shift for the active setup.
    /// Identity setups emit toolpaths (and sim positions) in world
    /// coordinates while the viewport draws everything zero-rooted; the
    /// difference is exactly -stock.origin. Zero for non-identity setups
    /// and for stocks with origin at (0,0,0).
    fn display_shift(&self) -> rs_cam_core::geo::P3 {
        self.frame()
            .map_or(rs_cam_core::geo::P3::new(0.0, 0.0, 0.0), |s| {
                s.emission_to_display_shift(&self.stock)
            })
    }

    /// The key of this frame. Destructured with no `..`, like every slot
    /// key.
    fn key(&self) -> upload_cache::FrameKey {
        let Self { setup, stock } = self;
        upload_cache::FrameKey {
            setup: setup.map(
                |(
                    id,
                    SetupFrame {
                        face_up,
                        z_rotation,
                    },
                )| (id, face_up, z_rotation),
            ),
            stock: stock.clone(),
        }
    }
}

/// The key form of a display shift.
fn shift_key(shift: rs_cam_core::geo::P3) -> [f64; 3] {
    [shift.x, shift.y, shift.z]
}

/// Is `shift` a real translation (not the zero shift)?
fn has_shift(shift: rs_cam_core::geo::P3) -> bool {
    shift.x != 0.0 || shift.y != 0.0 || shift.z != 0.0
}

/// Translate an annotated toolpath by `shift` — the display-frame adapter
/// for identity setups, whose toolpaths emit in world coordinates while the
/// viewport draws zero-rooted local. Delegates to
/// [`rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::translated`], which
/// re-frames every coordinate-bearing field (move targets, planner
/// engagement samples, rest-grid heatmap) in lockstep — arc center offsets
/// (`i`/`j` on the `MoveType`) are relative and survive translation
/// unchanged.
/// The plain colours of a sim mesh: the per-vertex colours core baked in,
/// or one wood tone when the mesh has none.
/// The plain stock colours: the baked wood shade, and the material colour
/// on each vertex of an added material (S5,
/// `rs_cam_core::export::material_colour`).
fn solid_sim_colors(
    mesh: &rs_cam_core::stock::stock_mesh::StockMesh,
    palette: &rs_cam_core::export::material_colour::MaterialPalette,
) -> Vec<[f32; 3]> {
    let num_verts = mesh.vertices.len() / 3;
    if mesh.colors.len() / 3 >= num_verts {
        rs_cam_core::export::material_colour::material_colors(mesh, palette)
    } else {
        vec![crate::render::colors::SIM_STOCK_WOOD; num_verts]
    }
}

fn translate_annotated(
    annotated: &rs_cam_core::trace::toolpath_spans::AnnotatedToolpath,
    shift: rs_cam_core::geo::P3,
) -> rs_cam_core::trace::toolpath_spans::AnnotatedToolpath {
    annotated.translated(shift)
}

/// The same display-frame adapter for a bare overlay mesh.
///
/// The tier map is built in the EMISSION frame — world for an identity setup,
/// setup-local otherwise — which is the frame a `rest_grid` arrives in too, so
/// it needs the same re-framing before it can sit on the drawn model. A
/// no-shift call is left untouched rather than walked, which is the common
/// case (`stock.origin == 0`).
fn shift_stock_mesh(
    mut mesh: rs_cam_core::stock::stock_mesh::StockMesh,
    shift: rs_cam_core::geo::P3,
) -> rs_cam_core::stock::stock_mesh::StockMesh {
    if shift.x == 0.0 && shift.y == 0.0 && shift.z == 0.0 {
        return mesh;
    }
    let delta = [shift.x as f32, shift.y as f32, shift.z as f32];
    for vertex in mesh.vertices.as_chunks_mut::<3>().0 {
        for (component, offset) in vertex.iter_mut().zip(delta) {
            *component += offset;
        }
    }
    mesh
}

/// Build a `toolpath_id -> vendor chipload band` map from the matched LUT
/// row per toolpath. Toolpaths with no LUT match (custom material, no
/// vendor data for the tool/op family, etc.) are absent from the map; the
/// renderer falls back to grey.
///
/// The core helper returns a bare `Range<f64>`; this is the display
/// boundary where the range acquires its unit
/// (`VendorChiploadBand` — linear advance per tooth).
fn build_advance_bands(
    session: &rs_cam_core::session::ProjectSession,
    sim_trace: Option<&rs_cam_core::stock::simulation_cut::SimulationCutTrace>,
) -> HashMap<rs_cam_core::ToolpathId, VendorChiploadBand> {
    rs_cam_core::tool_load::chipload_envelopes_for_session(session, sim_trace)
        .iter()
        .map(|(id, range)| (*id, VendorChiploadBand::from_advance_range(range)))
        .collect()
}

/// The line vertices of the hover ring of the item `key` of the toolpath
/// `tc` (geometry selection P4.5), in `SELECTION_HOVER`: the drawn lines of
/// the item ([`rs_cam_core::selection::Shape::pick_lines`]) at the point
/// function and the plane of the draw and the pick. A face draws its rings
/// at its own plane. A point draws a ring mark. No item, no vertex.
/// The line vertices of the tab marks of `scene` (geometry selection
/// P4.6): per tab a bar across the loop, `half_bar` to each side of the
/// lift centre, and a handle ring of `POINT_MARK_RADIUS_MM` round a hand
/// tab.
pub(crate) fn tab_vertices(
    scene: &crate::interaction::picking::TabScene,
) -> Vec<crate::render::LineVertex> {
    use crate::interaction::picking::POINT_MARK_RADIUS_MM;
    use crate::render::{LineVertex, colors};
    let z = scene.z as f32;
    let mut verts = Vec::new();
    for tab in &scene.tabs {
        let color = if tab.dragged {
            colors::TAB_DRAGGED
        } else if tab.hand {
            colors::TAB_HAND
        } else {
            colors::TAB_AUTO
        };
        // The normal of the loop: the direction turned a quarter turn.
        let [ax, ay] = tab.along;
        let (nx, ny) = (-ay * scene.half_bar, ax * scene.half_bar);
        verts.push(LineVertex {
            position: [(tab.at.x - nx) as f32, (tab.at.y - ny) as f32, z],
            color,
        });
        verts.push(LineVertex {
            position: [(tab.at.x + nx) as f32, (tab.at.y + ny) as f32, z],
            color,
        });
        if tab.hand || tab.dragged {
            super::push_circle_vertices(
                &mut verts,
                tab.at.x as f32,
                tab.at.y as f32,
                z,
                POINT_MARK_RADIUS_MM as f32,
                color,
                24,
            );
        }
    }
    verts
}

/// The lines of the drawing of `model` when no drawn toolpath reads it
/// (geometry selection P4.D): every loop, open path and circle ring, and a
/// ring mark at each point, in `frame` (the active setup) on the displayed
/// stock top plane (`drawing_display_z`), in `colors::DRAWING_NEUTRAL`. A
/// face draws nothing here: the mesh shows it.
pub(crate) fn neutral_drawing_vertices(
    model: &rs_cam_core::session::LoadedModel,
    frame: Option<&SetupFrame>,
    stock: &rs_cam_core::compute::StockConfig,
) -> Vec<crate::render::LineVertex> {
    use crate::interaction::picking::{POINT_MARK_RADIUS_MM, drawing_display_z, item_display_xy};
    use crate::render::{LineVertex, colors};
    let color = colors::DRAWING_NEUTRAL;
    let z = drawing_display_z(frame, stock) as f32;
    let at = |p: &rs_cam_core::geo::P2| -> [f32; 3] {
        let q = item_display_xy(frame, stock, *p);
        [q.x as f32, q.y as f32, z]
    };
    let mut verts = Vec::new();
    for item in model.items.iter() {
        match item.shape() {
            rs_cam_core::selection::Shape::Point(p) => {
                let c = at(p);
                super::push_circle_vertices(
                    &mut verts,
                    c[0],
                    c[1],
                    c[2],
                    POINT_MARK_RADIUS_MM as f32,
                    color,
                    16,
                );
            }
            shape => {
                let Some(ring) = shape.drawing_ring() else {
                    continue;
                };
                let pts = &ring.exterior;
                if pts.len() < 2 {
                    continue;
                }
                let ends = if ring.closed {
                    pts.len()
                } else {
                    pts.len() - 1
                };
                for i in 0..ends {
                    let (Some(a), Some(b)) = (pts.get(i), pts.get((i + 1) % pts.len())) else {
                        continue;
                    };
                    verts.push(LineVertex {
                        position: at(a),
                        color,
                    });
                    verts.push(LineVertex {
                        position: at(b),
                        color,
                    });
                }
            }
        }
    }
    verts
}

pub(crate) fn hover_vertices(
    session: &rs_cam_core::session::ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    key: &rs_cam_core::selection::MatchKey,
) -> Vec<crate::render::LineVertex> {
    use crate::interaction::picking::{
        POINT_MARK_RADIUS_MM, drawing_plane_z, item_display_xy, toolpath_setup_frame,
    };
    use crate::render::{LineVertex, colors};
    use rs_cam_core::selection::Shape;
    let color = colors::SELECTION_HOVER;
    let Some(item) = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| m.items.iter().find(|i| i.key().names_same_geometry(key)))
    else {
        return Vec::new();
    };
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, tc.id);
    let plane = drawing_plane_z(session, tc, frame.as_ref());
    let at = |p: &rs_cam_core::geo::P2| -> [f32; 3] {
        match item.shape() {
            Shape::Face(face) => {
                let q = rs_cam_core::geo::P3::new(p.x, p.y, face.plane_z);
                let q = frame.as_ref().map_or(q, |f| f.transform_point(q, stock));
                [q.x as f32, q.y as f32, q.z as f32]
            }
            Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Circle { .. } => {
                let q = item_display_xy(frame.as_ref(), stock, *p);
                [q.x as f32, q.y as f32, plane as f32]
            }
        }
    };
    let mut verts = Vec::new();
    for (pts, closed) in item.shape().pick_lines() {
        match pts {
            [] => {}
            [single] => {
                let c = at(single);
                super::push_circle_vertices(
                    &mut verts,
                    c[0],
                    c[1],
                    c[2],
                    POINT_MARK_RADIUS_MM as f32,
                    color,
                    24,
                );
            }
            _ => {
                let ends = if closed { pts.len() } else { pts.len() - 1 };
                for i in 0..ends {
                    let (Some(a), Some(b)) = (pts.get(i), pts.get((i + 1) % pts.len())) else {
                        continue;
                    };
                    verts.push(LineVertex {
                        position: at(a),
                        color,
                    });
                    verts.push(LineVertex {
                        position: at(b),
                        color,
                    });
                }
            }
        }
    }
    verts
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::translate_annotated;

    /// Geometry selection P4.D, fix 3: the neutral drawing of a DXF with no
    /// toolpath gives lines, all in `DRAWING_NEUTRAL`, on the displayed
    /// stock top plane. Red proof: the item loop skipped (no lines).
    #[test]
    fn a_drawing_with_no_toolpath_gives_neutral_lines_p4_d() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
        let model = rs_cam_core::io::load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            rs_cam_core::compute::stock_config::ModelKind::Dxf,
            rs_cam_core::compute::stock_config::ModelUnits::Millimeters,
            Default::default(),
        )
        .unwrap();
        let stock = rs_cam_core::compute::StockConfig::default();
        let verts = super::neutral_drawing_vertices(&model, None, &stock);
        assert!(
            verts.len() >= 2 * model.items.ring_count(),
            "every drawn ring gives lines: {} vertices for {} rings",
            verts.len(),
            model.items.ring_count()
        );
        let z = crate::interaction::picking::drawing_display_z(None, &stock) as f32;
        for v in &verts {
            assert_eq!(v.color, crate::render::colors::DRAWING_NEUTRAL);
            assert!((v.position[2] - z).abs() < 1e-4, "{:?}", v.position);
        }
    }
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
    use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

    #[test]
    fn translate_annotated_shifts_targets_and_preserves_arc_offsets() {
        let mut tp = Toolpath::new();
        tp.moves.push(Move {
            target: P3::new(1.0, 2.0, 3.0),
            move_type: MoveType::Rapid,
            intent: MoveIntent::Unknown,
        });
        tp.moves.push(Move {
            target: P3::new(4.0, 5.0, -2.0),
            move_type: MoveType::ArcCW {
                i: 0.5,
                j: -0.5,
                feed_rate: 1000.0,
            },
            intent: MoveIntent::Unknown,
        });
        let annotated = AnnotatedToolpath {
            toolpath: tp,
            spans: Vec::new(),
            spans_valid: true,
            produced: rs_cam_core::trace::produced::Produced::default(),
        };

        let shifted = translate_annotated(&annotated, P3::new(0.0, 0.0, 19.0));
        assert_eq!(shifted.toolpath.moves[0].target.z, 22.0);
        assert_eq!(shifted.toolpath.moves[1].target.z, 17.0);
        // Arc center offsets are relative — translation must not touch them.
        match shifted.toolpath.moves[1].move_type {
            MoveType::ArcCW { i, j, .. } => {
                assert_eq!((i, j), (0.5, -0.5));
            }
            _ => panic!("arc move type changed"),
        }
        // Original untouched.
        assert_eq!(annotated.toolpath.moves[0].target.z, 3.0);
    }
}
