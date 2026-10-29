pub mod camera;
pub mod colors;
pub mod fixture_render;
pub mod gpu_safety;
pub mod grid_render;
pub mod height_planes;
pub mod mesh_render;
pub mod outline;
pub mod sim_render;
pub mod stock_render;
pub mod toolpath_render;
pub mod upload_cache;

use egui_wgpu::wgpu;
use rs_cam_core::settings::DisplayMode;

use fixture_render::FixtureGpuData;
use grid_render::GridGpuData;
use height_planes::HeightPlanesGpuData;
use mesh_render::{MeshGpuData, MeshVertex};
use sim_render::{ColoredMeshVertex, SimMeshGpuData, ToolModelGpuData};
use stock_render::StockGpuData;
use toolpath_render::ToolpathGpuData;

/// GPU uniform data for mesh rendering (Phong shading). 144 B; the face and
/// the STL shaders read every field (round 3: one material, one light).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshUniforms {
    pub view_proj: [[f32; 4]; 4],
    /// The direction TO the light, world frame (the headlight,
    /// [`headlight_dir`]).
    pub light_dir: [f32; 3],
    pub _pad0: f32,
    pub camera_pos: [f32; 3],
    pub _pad1: f32,
    /// The material (`colors::MODEL_SURFACE`), w = 1.
    pub surface: [f32; 4],
    /// The hemisphere light from +Z (`colors::LIGHT_SKY`), w = 1.
    pub sky: [f32; 4],
    /// The hemisphere light from −Z (`colors::LIGHT_GROUND`), w = 1.
    pub ground: [f32; 4],
}

impl MeshUniforms {
    /// The uniforms of one frame: the camera, the light and the material
    /// and hemisphere tokens of `colors.rs`.
    pub fn new(view_proj: [[f32; 4]; 4], light_dir: [f32; 3], camera_pos: [f32; 3]) -> Self {
        let rgba = |c: colors::Rgb| {
            let [r, g, b] = c.array();
            [r, g, b, 1.0]
        };
        Self {
            view_proj,
            light_dir,
            _pad0: 0.0,
            camera_pos,
            _pad1: 0.0,
            surface: rgba(colors::MODEL_SURFACE),
            sky: rgba(colors::LIGHT_SKY),
            ground: rgba(colors::LIGHT_GROUND),
        }
    }
}

/// The headlight in the world frame (round 3): `colors::HEADLIGHT_VIEW`
/// (view space: x right, y up, z toward the eye) through the camera basis
/// `right`, `up` and `back` (the unit vector from the target to the eye),
/// normalised.
pub fn headlight_dir(right: [f32; 3], up: [f32; 3], back: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = colors::HEADLIGHT_VIEW;
    let v = [
        right[0] * x + up[0] * y + back[0] * z,
        right[1] * x + up[1] * y + back[1] * z,
        right[2] * x + up[2] * y + back[2] * z,
    ];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
        .sqrt()
        .max(f32::MIN_POSITIVE);
    [v[0] / len, v[1] / len, v[2] / len]
}

/// GPU uniform data for colored mesh rendering (simulation stock with opacity).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredMeshUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub light_dir: [f32; 3],
    pub _pad0: f32,
    pub camera_pos: [f32; 3],
    pub opacity: f32,
}

/// Fixed opacity for the rest-depth heatmap overlay (pencil detector #4),
/// independent of `sim_mesh_opacity` / the shared solid-stock / height-plane
/// translucency. Deliberately higher than the 0.15 those overlays use in the
/// non-highlighted state — the heatmap's whole purpose is to be read at a
/// glance over a textured model in the Toolpaths workspace, so it needs to
/// stay clearly visible rather than blend into the surface.
const REST_HEATMAP_OPACITY: f32 = 0.6;

/// Fixed opacity for the multi-tool tier-preview overlay (Phase U). Its own
/// constant beside [`REST_HEATMAP_OPACITY`] rather than a shared one: this
/// overlay answers "which tool owns this ground" and is meant to be read as
/// flat territory, so it sits slightly more opaque than the rest heatmap while
/// still letting the surface it drapes read through.
const TIER_PREVIEW_OPACITY: f32 = 0.7;

/// Fixed opacity for the By Area regions overlay. The overlay is a flat
/// sheet at the stock top, so the model under it must stay visible.
const AREA_REGIONS_OPACITY: f32 = 0.55;

/// GPU uniform data for line rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineUniforms {
    pub view_proj: [[f32; 4]; 4],
    /// Multiplier applied to every line's colour in the fragment stage.
    ///
    /// `1.0` for the ordinary pass. The second, DIMMED bind group carries
    /// [`MOVE_DIM_UNDER_REACH`] and the toolpath move draws switch to it
    /// while the reach overlay is on — see the constant.
    pub dim: f32,
    /// The minimum edge offset toward the eye, in mm (D4). It is 0 for every
    /// line except the model edges, and 0 turns the offset off.
    pub edge_tol: f32,
    /// The offset per mm² of eye distance (D4): see [`EdgeOffset`].
    pub edge_k: f32,
    /// The block ends on a 16-byte stride; the padding is explicit so the
    /// Rust and WGSL layouts cannot silently disagree.
    pub _pad: f32,
    /// xyz = the eye position with w = 1 (perspective), or the unit
    /// direction to the eye with w = 0 (orthographic).
    pub eye: [f32; 4],
    /// The quad lines (`vs_wide`, D §3.5): x, y = the viewport size in
    /// physical pixels, z = the line width in physical pixels, w = the lift
    /// slope (`OUTLINE_FOLD_PX`: the quad moves toward the eye by `w` pixel
    /// footprints per pixel of half width, live look round 3). The
    /// `LineList` entry point does not read it.
    pub wide: [f32; 4],
}

/// The viewport facts of the quad lines: the viewport size in physical
/// pixels and the pixels per point. `frame_set` turns the width tokens
/// (`ui::tokens::LINE_*_PX`, in points) into physical pixels with them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LineWidths {
    pub viewport_px: [f32; 2],
    pub pixels_per_point: f32,
}

/// The six line uniforms that `prepare` writes each frame (`frame_set`).
#[derive(Debug, Clone, Copy)]
pub struct LineGroups {
    /// The grid, the stock and the toolpath moves: no offset.
    pub plain: LineUniforms,
    /// The toolpath moves under the reach overlay: no offset, dimmed.
    pub dimmed: LineUniforms,
    /// The 1 px lines on the model (the drawing): the edge offset.
    pub edge: LineUniforms,
    /// The model edges as quads: the edge offset, `LINE_EDGE_PX`.
    pub edge_wide: LineUniforms,
    /// The selection highlight as quads: the edge offset,
    /// `LINE_SELECTED_PX`.
    pub selected: LineUniforms,
    /// The hover ring and the tab marks as quads: the edge offset,
    /// `LINE_HOVER_PX`.
    pub hover: LineUniforms,
}

impl LineUniforms {
    /// The uniforms of one frame, with no edge offset.
    pub fn new(view_proj: [[f32; 4]; 4], eye: [f32; 4]) -> Self {
        Self {
            view_proj,
            dim: 1.0,
            edge_tol: 0.0,
            edge_k: 0.0,
            _pad: 0.0,
            eye,
            wide: [0.0; 4],
        }
    }

    /// The six line uniforms that `prepare` writes each frame. Only the
    /// plain and the dimmed groups carry no offset: the grid and the
    /// toolpaths do not move. The lines on the model (the model edges, the
    /// drawing and the overlay lines) take the edge offset; the quad groups
    /// also take their width token in physical pixels.
    pub fn frame_set(self, edges: EdgeOffset, widths: LineWidths) -> LineGroups {
        use crate::ui::tokens::{LINE_EDGE_PX, LINE_HOVER_PX, LINE_SELECTED_PX};
        let plain = Self {
            dim: 1.0,
            edge_tol: 0.0,
            edge_k: 0.0,
            wide: [0.0; 4],
            ..self
        };
        let dimmed = Self {
            dim: MOVE_DIM_UNDER_REACH,
            ..plain
        };
        let edge = Self {
            edge_tol: edges.tol,
            edge_k: edges.k,
            ..plain
        };
        let [w, h] = widths.viewport_px;
        // The lift slope of the quads (`vs_wide`, `lifted`): the fold slope
        // of the outline, so a quad covers its face up to the angle the
        // outline still calls a plane (atan 8 = 82.9°).
        let slope = outline::OUTLINE_FOLD_PX;
        let quad = |points: f32| Self {
            wide: [w, h, points * widths.pixels_per_point, slope],
            ..edge
        };
        LineGroups {
            plain,
            dimmed,
            edge,
            edge_wide: quad(LINE_EDGE_PX),
            selected: quad(LINE_SELECTED_PX),
            hover: quad(LINE_HOVER_PX),
        }
    }
}

/// The move of a model edge vertex toward the eye (D4), so that the edge
/// wins the depth test against its own face. wgpu refuses a depth bias on
/// a line topology, so the vertex shader moves the vertex instead. A move
/// along the eye ray does not change the screen position in perspective.
///
/// The offset at eye distance `z` is `max(tol, k · z²)`:
///
/// - Perspective depth is `d ≈ 1 − n / z`, so `∂d/∂z = n / z²`. One
///   `Depth32Float` step near `d = 1` is `2^-24`, so it spans
///   `2^-24 z² / n`. Four steps: `2^-22 z² / n`, so `k = 2^-22 / n`.
/// - Orthographic depth is linear over `2 far`. Four steps span
///   `4 · 2^-24 · 2 far`; `k = 0`, and `tol` holds the max.
/// - The edge polyline can sit off the facets by the STEP chord tolerance,
///   so `tol` is at least that chord.
///
/// nalgebra builds GL-convention matrices (clip z in `[-w, w]`) and wgpu
/// keeps only `[0, w]` with no remap, so the real depth slope is twice the
/// one above: the margin is eight depth steps, not four.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeOffset {
    /// The minimum offset, in mm. A value above 0 turns the offset on.
    pub tol: f32,
    /// The offset per mm² of eye distance.
    pub k: f32,
}

impl EdgeOffset {
    /// Perspective with near plane `near`, chord tolerance `chord` (mm).
    pub fn perspective(near: f32, chord: f32) -> Self {
        Self {
            tol: chord.max(f32::MIN_POSITIVE),
            k: 2f32.powi(-22) / near,
        }
    }

    /// Orthographic with depth range `-far..far`, chord tolerance `chord`.
    pub fn orthographic(far: f32, chord: f32) -> Self {
        Self {
            tol: chord.max(4.0 * 2f32.powi(-24) * 2.0 * far),
            k: 0.0,
        }
    }

    /// The offset at eye distance `dist`: the Rust mirror of the line
    /// vertex shader.
    pub fn at(&self, dist: f32) -> f32 {
        self.tol.max(self.k * dist * dist)
    }
}

/// Colour multiplier for toolpath moves while the reach overlay is drawing
/// on the model (P5.3, 2026-09-09).
///
/// The offscreen composite already does this — `CompositeSubject::Background`
/// drops the moves to 0.45 colour and 0.4 ribbon radius, because 17 959 green
/// moves seen from above are an opaque mat over the shading. The live
/// viewport had the same problem and no such treatment.
///
/// **Only the colour factor transfers.** The offscreen renderer draws moves
/// as TUBES and can thin them; the viewport draws the moves as
/// `PrimitiveTopology::LineList`, one pixel wide. The quad lines (D §3.5)
/// stay off the moves until the WP27 draw set is measured with them, so the
/// live rule is the 0.45 colour factor alone.
///
/// This is a DRAW-TIME treatment, not a visibility toggle: no registry flag
/// moves, so the Overlays panel still shows Cutting moves ON and the operator
/// still owns that switch.
pub const MOVE_DIM_UNDER_REACH: f32 = 0.45;

/// Line vertex for grid, stock wireframe, and toolpath rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

/// The scene MSAA sample count. wgpu guarantees 4 samples for the app
/// colour formats and `Depth32Float`; a higher count needs an adapter
/// feature that egui does not request.
pub const MSAA_SAMPLES: u32 = 4;

/// The scene sample count that this adapter gives for `format`.
///
/// The value is [`MSAA_SAMPLES`] when the adapter supports that count for
/// `format` and for `Depth32Float`. Otherwise the value is 1.
pub fn scene_sample_count(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
    let supported = |f: wgpu::TextureFormat| {
        adapter
            .get_texture_format_features(f)
            .flags
            .sample_count_supported(MSAA_SAMPLES)
    };
    if supported(format) && supported(wgpu::TextureFormat::Depth32Float) {
        MSAA_SAMPLES
    } else {
        1
    }
}

/// Offscreen render targets with depth buffer.
struct OffscreenTargets {
    /// The multisampled colour target. It is `None` when the sample count
    /// is 1. The pass resolves it into `color_view`.
    msaa_color_view: Option<wgpu::TextureView>,
    /// The single-sample colour: the resolve target and the blit source.
    color_view: wgpu::TextureView,
    /// The texture of `color_view`. It allows a copy out, so a headless
    /// test reads the frame back (`headless_color_texture`).
    color_texture: wgpu::Texture,
    /// The depth target at the scene sample count.
    depth_view: wgpu::TextureView,
    /// The ID target, its depth and the outline bind group (D §4).
    outline: outline::OutlineTargets,
    blit_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl OffscreenTargets {
    /// Begins the 3D scene pass on these targets. This is the one function
    /// that builds the scene pass: the frame and the headless door both
    /// call it.
    ///
    /// With MSAA the pass renders into `msaa_color_view` and resolves into
    /// `color_view`. At sample count 1 it renders into `color_view`.
    fn begin_scene_pass<'e>(&self, encoder: &'e mut wgpu::CommandEncoder) -> wgpu::RenderPass<'e> {
        let (view, resolve_target, store) = match &self.msaa_color_view {
            // The multisampled colour resolves into `color_view` at the end
            // of the pass. Nothing reads the samples after the resolve.
            Some(msaa) => (msaa, Some(&self.color_view), wgpu::StoreOp::Discard),
            None => (&self.color_view, None, wgpu::StoreOp::Store),
        };
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3d_scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear({
                        // UP3. This was (0.102, 0.102, 0.149) — the old
                        // violet (26, 26, 38), the same literal the four
                        // CentralPanel fills carried before UP1. The
                        // panels moved onto the cold ramp and the
                        // viewport did not, so the largest area on
                        // screen fought everything around it.
                        //
                        // §2.3 names SURFACE_SUNKEN the viewport ground.
                        let [r, g, b] =
                            crate::ui::tokens::as_unit_rgb(crate::ui::tokens::SURFACE_SUNKEN);
                        wgpu::Color { r, g, b, a: 1.0 }
                    }),
                    store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    // No reader follows the pass: the blit samples colour only.
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        })
    }
}

/// GPU data for polygon/DXF/SVG line rendering.
pub struct PolygonGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
}

/// All GPU resources for the 3D viewport, stored in egui_wgpu::CallbackResources.
pub struct RenderResources {
    // 3D scene pipelines (render to offscreen with depth)
    mesh_pipeline: wgpu::RenderPipeline,
    sim_mesh_pipeline: wgpu::RenderPipeline,
    height_plane_pipeline: wgpu::RenderPipeline,
    /// Opaque colored mesh pipeline — same vertex format as sim_mesh but no
    /// alpha blending. The reach overlay draws with it.
    colored_opaque_pipeline: wgpu::RenderPipeline,
    /// The face pipeline of the STEP models (D2): `FaceMeshVertex`, group 0
    /// the mesh uniforms, group 1 the model's face state and the palette.
    face_mesh_pipeline: wgpu::RenderPipeline,
    /// Group 1 of `face_mesh_pipeline`: binding 0 the face state words
    /// (read-only storage), binding 1 the palette uniform.
    pub face_state_bgl: wgpu::BindGroupLayout,
    /// The face palette uniform (`colors::face_palette`). `new` writes it
    /// once; every model's state bind group reads it.
    pub face_palette_buffer: wgpu::Buffer,
    line_pipeline: wgpu::RenderPipeline,
    /// The quad lines (D §3.5): the `LineList` buffers bind as an instance
    /// buffer (one segment per instance) and `vs_wide` pushes a screen
    /// space quad of the group's width. The selection highlight, the hover
    /// ring and the tab marks draw with it.
    wide_line_pipeline: wgpu::RenderPipeline,
    /// `wide_line_pipeline` with the depth write off: the model edges only
    /// (D4). The edges test the depth but write none, so a later overlay
    /// line on the same segment (the pick highlight, the hover ring, a tab
    /// mark) is not hidden behind its own edge.
    wide_edge_line_pipeline: wgpu::RenderPipeline,
    mesh_uniform_buffer: wgpu::Buffer,
    sim_mesh_uniform_buffer: wgpu::Buffer,
    line_uniform_buffer: wgpu::Buffer,
    mesh_bind_group: wgpu::BindGroup,
    sim_mesh_bind_group: wgpu::BindGroup,
    line_bind_group: wgpu::BindGroup,
    /// Second uniform + bind group holding the same `view_proj` with `dim`
    /// set to [`MOVE_DIM_UNDER_REACH`]. A second bind group rather than one
    /// buffer rewritten mid-pass, because a buffer write cannot happen inside
    /// a render pass, and rather than a dynamic offset because two 80-byte
    /// buffers need no alignment arithmetic to get wrong.
    line_dim_uniform_buffer: wgpu::Buffer,
    line_dim_bind_group: wgpu::BindGroup,
    /// Third uniform + bind group: the same camera with the edge offset
    /// set (D4). The model edges and the overlay lines on the model (the
    /// drawing and its pick highlight, the hover ring, the tab marks) draw
    /// with it, so they pass the depth test of the faces. The grid and the
    /// toolpaths do not.
    line_edge_uniform_buffer: wgpu::Buffer,
    line_edge_bind_group: wgpu::BindGroup,
    /// The quad groups (`LineGroups`): the model edges, the selection
    /// highlight, and the hover ring with the tab marks. Each holds the
    /// edge offset and its width.
    line_edge_wide_uniform_buffer: wgpu::Buffer,
    line_edge_wide_bind_group: wgpu::BindGroup,
    line_selected_uniform_buffer: wgpu::Buffer,
    line_selected_bind_group: wgpu::BindGroup,
    line_hover_uniform_buffer: wgpu::Buffer,
    line_hover_bind_group: wgpu::BindGroup,
    /// Dedicated uniform buffer + bind group for the rest-depth heatmap
    /// overlay, carrying its own fixed opacity independent of
    /// `sim_mesh_uniform_buffer`. That buffer is shared by sim mesh, solid
    /// stock, AND height planes — writing a single opacity into it each
    /// frame (`sim_mesh_opacity` when the sim mesh is shown, else a
    /// hardcoded 0.15 "translucency" value) meant the heatmap, drawn with
    /// the same `height_plane_pipeline` + `sim_mesh_bind_group`, inherited
    /// whatever the *other* overlays wanted — 0.15 in the Toolpaths
    /// workspace, which reads as invisible over a textured model. A
    /// separate buffer/bind-group lets the heatmap always draw at its own
    /// clearly-visible opacity.
    rest_heatmap_uniform_buffer: wgpu::Buffer,
    rest_heatmap_bind_group: wgpu::BindGroup,
    /// The multi-tool tier preview's own uniform buffer + bind group, for the
    /// same reason the rest heatmap has its own: a shared opacity slot is how
    /// one overlay ends up wearing another's translucency. Both overlays can
    /// be visible in one frame, so they cannot take turns writing one buffer.
    tier_preview_uniform_buffer: wgpu::Buffer,
    tier_preview_bind_group: wgpu::BindGroup,
    /// The By Area regions overlay's own uniform buffer + bind group, for
    /// the same reason as the tier preview: it can be on screen with the
    /// other two overlays, each at its own opacity.
    area_regions_uniform_buffer: wgpu::Buffer,
    area_regions_bind_group: wgpu::BindGroup,

    // Blit pipeline (copy offscreen to egui render pass)
    blit_pipeline: wgpu::RenderPipeline,
    /// The ID pass, the outline and the id readback ring (D §4).
    outline: outline::OutlinePipelines,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,

    // Offscreen render targets (resized per frame)
    offscreen: Option<OffscreenTargets>,
    target_format: wgpu::TextureFormat,
    /// The scene sample count. `new` stores it once; every scene pipeline,
    /// the multisampled colour and the depth target read it.
    sample_count: u32,

    // Scene data
    pub mesh_data_list: Vec<MeshGpuData>,
    /// The STEP models of the face pipeline, one entry per drawn model.
    pub face_mesh_data_list: Vec<mesh_render::FaceMeshGpuData>,
    pub grid_data: GridGpuData,
    pub stock_data: Option<StockGpuData>,
    pub solid_stock_data: Option<stock_render::SolidStockGpuData>,
    pub fixture_data: Option<FixtureGpuData>,
    pub toolpath_data: Vec<ToolpathGpuData>,
    pub sim_mesh_data: Option<SimMeshGpuData>,
    pub height_planes_data: Option<HeightPlanesGpuData>,
    /// Rest-depth heatmap overlay mesh (pencil detector #4) — uploaded only
    /// when a toolpath with a populated `rest_grid` is selected. Reuses
    /// `SimMeshGpuData`'s chunked-upload machinery since it's the same
    /// `ColoredMeshVertex` layout as the sim stock mesh; drawn with the
    /// `height_plane_pipeline` (depth-read-only, alpha-blended) so it drapes
    /// over the model without z-fighting or occluding it.
    pub rest_heatmap_data: Option<SimMeshGpuData>,
    /// Multi-tool tier-map preview overlay (Phase U). Its own slot, not a
    /// second tenant of `rest_heatmap_data` — a selected toolpath's rest grid
    /// and a plan preview are different answers to different questions and can
    /// be on screen together. Same `SimMeshGpuData` machinery and same
    /// depth-read-only pipeline.
    pub tier_preview_data: Option<SimMeshGpuData>,
    /// The By Area regions overlay: the selected 3D Rough's detected
    /// regions, one flat colour per region at the stock top. Same
    /// `SimMeshGpuData` machinery and same depth-read-only pipeline as the
    /// tier preview.
    pub area_regions_data: Option<SimMeshGpuData>,
    /// Per-tool reach-map overlay (P5) — the MODEL mesh uploaded with one
    /// reach colour per vertex.
    ///
    /// Not a drape like the two overlays above. It REPLACES the plain and
    /// enriched model draws for the frame, so the two cannot z-fight, and it
    /// therefore uses the opaque `colored_opaque_pipeline` rather than the
    /// depth-read-only height-plane pipeline. `EnrichedMeshGpuData` is the
    /// container because it is already exactly one indexed
    /// `ColoredMeshVertex` buffer pair.
    pub reach_overlay_data: Option<mesh_render::EnrichedMeshGpuData>,
    pub tool_model_data: Option<ToolModelGpuData>,
    pub polygon_data: Vec<PolygonGpuData>,
    /// The selection highlight: the items of the drawing that the
    /// selection takes (cut and island) and the flashed items, out of
    /// `polygon_data`, so they draw as quads of `LINE_SELECTED_PX`.
    pub selection_lines: Option<PolygonGpuData>,
    /// Geometry selection P4.5: the ring of the item under the pointer in
    /// pick mode, in `SELECTION_HOVER`. It is its own small buffer, so a
    /// hover change uploads only this buffer and never the drawing
    /// (`polygon_data`). It draws after the drawing, over it.
    pub hover_lines: Option<PolygonGpuData>,
    /// Geometry selection P4.6: the tab marks of the tab editor (a bar
    /// across the loop per tab, a ring per hand tab). Its own small buffer:
    /// it uploads on a change of the layout or of the drag only
    /// (`state::tab_edit::TabMarksDetector`).
    pub tab_lines: Option<PolygonGpuData>,
    /// D4: the boundary loops of every drawn STEP face, in `MODEL_EDGE`
    /// (`app/gpu_upload/model_edges.rs`). It draws with the edge bind group.
    pub model_edge_lines: Option<PolygonGpuData>,
    /// Inputs `model_edge_lines` was last built from.
    pub model_edges_key: Option<upload_cache::ModelEdgesKey>,
    /// FOLLOW_UPS §1: the unique triangle edges of every drawn mesh model,
    /// in `MODEL_WIREFRAME` (`app/gpu_upload/wireframe.rs`). Built only in
    /// the `Wireframe` display mode.
    pub model_wireframe_lines: Option<PolygonGpuData>,
    /// Inputs `model_wireframe_lines` was last built from.
    pub model_wireframe_key: Option<upload_cache::MeshUploadKey>,
    pub collision_vertex_buffer: Option<wgpu::Buffer>,
    pub collision_vertex_count: u32,
    pub origin_axes_data: Option<grid_render::OriginAxesGpuData>,
    /// Cached GPU device limits for buffer size validation.
    pub gpu_limits: gpu_safety::GpuLimits,

    // --- Upload cache (V8) ---
    //
    // `upload_gpu_data` used to clear and rebuild every buffer above on any
    // `pending_upload`. These keys record what each expensive resource was
    // last built from, so a pass rebuilds only the resources whose inputs
    // actually moved. See `upload_cache` for the key contracts.
    /// Inputs `mesh_data_list` was last built from.
    pub mesh_upload_key: Option<upload_cache::MeshUploadKey>,
    /// Inputs `face_mesh_data_list` was last built from.
    pub enriched_upload_key: Option<upload_cache::EnrichedUploadKey>,
    /// Inputs the face state words were last written from (D3).
    pub face_state_key: Option<upload_cache::FaceStateKey>,
    /// Inputs the collision-marker buffer was last built from.
    pub collision_upload_key: Option<upload_cache::CollisionUploadKey>,
    /// Inputs `rest_heatmap_data` was last built from. `None` means the
    /// overlay is (correctly) absent, which is also a cacheable state.
    pub rest_heatmap_upload_key: Option<upload_cache::RestHeatmapUploadKey>,
    /// Inputs `tier_preview_data` was last built from. `None` means no
    /// preview is held, which is also a cacheable state — so toggling the
    /// overlay's visibility checkbox rebuilds nothing.
    pub tier_preview_upload_key: Option<upload_cache::TierPreviewUploadKey>,
    /// Inputs `area_regions_data` was last built from. `None` means the
    /// selected toolpath carries no region map, which is also a cacheable
    /// state — so toggling the overlay's checkbox rebuilds nothing.
    pub area_regions_upload_key: Option<upload_cache::AreaRegionsUploadKey>,
    /// Inputs `reach_overlay_data` was last built from. `None` means no map
    /// is held, which is also a cacheable state — so toggling the overlay's
    /// visibility checkbox rebuilds nothing.
    pub reach_overlay_upload_key: Option<upload_cache::ReachOverlayUploadKey>,
    /// Inputs `obstacle_lines` was last built from (K2).
    pub obstacle_lines_key: Option<upload_cache::ObstacleLinesKey>,
    /// The obstacle lines of the displayed setup. The reference-line
    /// buffer appends them on each upload; only a key change rebuilds them.
    pub obstacle_lines: Vec<LineVertex>,
    /// Lifetime counts of upload passes and per-resource buffer builds.
    pub upload_stats: upload_cache::UploadStats,
}

impl RenderResources {
    /// Builds every viewport pipeline. `sample_count` is the scene sample
    /// count from [`scene_sample_count`]. Every scene pipeline and the
    /// offscreen colour and depth targets read this one value.
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        // One sample count for every scene pipeline. A pipeline and an
        // attachment with different counts fail at `begin_render_pass`.
        let scene_multisample = wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };
        // --- Mesh pipeline (renders to offscreen with depth) ---
        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(MESH_SHADER_SRC.into()),
        });

        let mesh_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mesh_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let mesh_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh_uniforms"),
            size: std::mem::size_of::<MeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh_bg"),
            layout: &mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mesh_uniform_buffer.as_entire_binding(),
            }],
        });

        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh_pl"),
            bind_group_layouts: &[Some(&mesh_bind_group_layout)],
            immediate_size: 0,
        });

        let depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        // Read-only depth for transparent overlays (height planes): depth-test
        // against opaque geometry but don't write, so they don't block each other
        // or the model behind them.
        let depth_read_only = wgpu::DepthStencilState {
            depth_write_enabled: Some(false),
            ..depth_stencil.clone()
        };

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh_pipeline"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mesh_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(MeshVertex::layout())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample: scene_multisample,
            multiview_mask: None,
            cache: None,
        });

        // --- Face pipeline (STEP models, D2) ---
        // The colour of a face comes from its state word and the palette,
        // so a hover or a selection change rewrites the words only (D3).
        let face_mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("face_mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(FACE_MESH_SHADER_SRC.into()),
        });
        let face_state_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("face_state_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let face_palette_buffer = {
            use wgpu::util::DeviceExt;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("face_palette"),
                contents: bytemuck::cast_slice(&colors::face_palette()),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };
        let face_mesh_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("face_mesh_pl"),
                bind_group_layouts: &[Some(&mesh_bind_group_layout), Some(&face_state_bgl)],
                immediate_size: 0,
            });
        let face_mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("face_mesh_pipeline"),
            layout: Some(&face_mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &face_mesh_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(mesh_render::FaceMeshVertex::layout())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &face_mesh_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample: scene_multisample,
            multiview_mask: None,
            cache: None,
        });

        // --- Sim mesh pipeline (per-vertex colored, alpha blending) ---
        let sim_mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sim_mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(COLORED_MESH_SHADER_SRC.into()),
        });

        let sim_mesh_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim_mesh_uniforms"),
            size: std::mem::size_of::<ColoredMeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sim_mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim_mesh_bg"),
            layout: &mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sim_mesh_uniform_buffer.as_entire_binding(),
            }],
        });

        // --- Rest-depth heatmap uniform buffer + bind group (own opacity,
        // see the `rest_heatmap_uniform_buffer` field doc) ---
        let rest_heatmap_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rest_heatmap_uniforms"),
            size: std::mem::size_of::<ColoredMeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rest_heatmap_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rest_heatmap_bg"),
            layout: &mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rest_heatmap_uniform_buffer.as_entire_binding(),
            }],
        });

        // --- Tier-preview uniform buffer + bind group (own opacity; see the
        // `tier_preview_uniform_buffer` field doc) ---
        let tier_preview_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tier_preview_uniforms"),
            size: std::mem::size_of::<ColoredMeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tier_preview_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tier_preview_bg"),
            layout: &mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: tier_preview_uniform_buffer.as_entire_binding(),
            }],
        });

        // --- By Area regions uniform buffer + bind group (own opacity) ---
        let area_regions_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("area_regions_uniforms"),
            size: std::mem::size_of::<ColoredMeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let area_regions_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("area_regions_bg"),
            layout: &mesh_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: area_regions_uniform_buffer.as_entire_binding(),
            }],
        });

        let sim_mesh_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sim_mesh_pl"),
                bind_group_layouts: &[Some(&mesh_bind_group_layout)],
                immediate_size: 0,
            });

        let sim_mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sim_mesh_pipeline"),
            layout: Some(&sim_mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sim_mesh_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ColoredMeshVertex::layout())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sim_mesh_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample: scene_multisample,
            multiview_mask: None,
            cache: None,
        });

        // --- Height plane pipeline (transparent overlay, depth read-only) ---
        // Identical to sim_mesh_pipeline but doesn't write depth, so height
        // planes don't occlude the model or each other.
        let height_plane_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("height_plane_pipeline"),
                layout: Some(&sim_mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &sim_mesh_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(ColoredMeshVertex::layout())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &sim_mesh_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_read_only.clone()),
                multisample: scene_multisample,
                multiview_mask: None,
                cache: None,
            });

        // --- Opaque colored mesh pipeline (STEP face colors, no alpha blending) ---
        // Uses fs_opaque which outputs alpha=1.0, decoupled from the shared
        // opacity uniform that height planes and sim stock use.
        let colored_opaque_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("colored_opaque_pipeline"),
                layout: Some(&sim_mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &sim_mesh_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(ColoredMeshVertex::layout())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &sim_mesh_shader,
                    entry_point: Some("fs_opaque"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_stencil.clone()),
                multisample: scene_multisample,
                multiview_mask: None,
                cache: None,
            });

        // --- Line pipeline (renders to offscreen with depth) ---
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line_shader"),
            source: wgpu::ShaderSource::Wgsl(LINE_SHADER_SRC.into()),
        });

        let line_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("line_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // P5.3 made the fragment stage read the line uniforms
                    // (the reach-overlay dim factor); wgpu validates the
                    // layout against BOTH stages, so VERTEX-only here is a
                    // startup crash (`create_render_pipeline` 'line_pipeline').
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let line_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_uniforms"),
            size: std::mem::size_of::<LineUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_bg"),
            layout: &line_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: line_uniform_buffer.as_entire_binding(),
            }],
        });

        let line_dim_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_uniforms_dim"),
            size: std::mem::size_of::<LineUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_dim_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_bg_dim"),
            layout: &line_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: line_dim_uniform_buffer.as_entire_binding(),
            }],
        });

        let line_edge_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_uniforms_edge"),
            size: std::mem::size_of::<LineUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_edge_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line_bg_edge"),
            layout: &line_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: line_edge_uniform_buffer.as_entire_binding(),
            }],
        });

        let line_group = |label: &str| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<LineUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &line_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            (buffer, group)
        };
        let (line_edge_wide_uniform_buffer, line_edge_wide_bind_group) =
            line_group("line_uniforms_edge_wide");
        let (line_selected_uniform_buffer, line_selected_bind_group) =
            line_group("line_uniforms_selected");
        let (line_hover_uniform_buffer, line_hover_bind_group) = line_group("line_uniforms_hover");

        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line_pl"),
            bind_group_layouts: &[Some(&line_bind_group_layout)],
            immediate_size: 0,
        });

        let line_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        };

        let line_pipeline_with = |label: &str, depth: wgpu::DepthStencilState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&line_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &line_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(line_vertex_layout.clone())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &line_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: scene_multisample,
                multiview_mask: None,
                cache: None,
            })
        };
        let line_pipeline = line_pipeline_with("line_pipeline", depth_stencil.clone());

        // The quad lines read the same `LineVertex` buffers, two vertices
        // (one segment) per instance: stride 48, `a.pos, a.col, b.pos, b.col`.
        let segment_stride = 2 * std::mem::size_of::<LineVertex>() as u64;
        let segment_attributes = [
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 36,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x3,
            },
        ];
        let segment_layout = wgpu::VertexBufferLayout {
            array_stride: segment_stride,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &segment_attributes,
        };
        let wide_pipeline_with = |label: &str, depth: wgpu::DepthStencilState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&line_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &line_shader,
                    entry_point: Some("vs_wide"),
                    buffers: &[Some(segment_layout.clone())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &line_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                // No culling: a quad's winding follows the segment
                // direction on screen.
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth),
                multisample: scene_multisample,
                multiview_mask: None,
                cache: None,
            })
        };
        let wide_line_pipeline = wide_pipeline_with("wide_line_pipeline", depth_stencil);
        let wide_edge_line_pipeline =
            wide_pipeline_with("wide_edge_line_pipeline", depth_read_only);

        // --- Blit pipeline (fullscreen triangle, samples offscreen texture) ---
        // --- The ID pass and the selection outline (D §4) ---
        let outline = outline::OutlinePipelines::new(
            device,
            target_format,
            scene_multisample,
            &mesh_bind_group_layout,
            &face_state_bgl,
        );

        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER_SRC.into()),
        });

        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pl"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None, // Blit into egui pass (no depth)
            // egui's pass is single-sample, so the blit stays at count 1.
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let gpu_limits = gpu_safety::GpuLimits::from_device(device);
        let grid_data = GridGpuData::new(device, &gpu_limits, 200.0, 10.0);

        Self {
            mesh_pipeline,
            sim_mesh_pipeline,
            height_plane_pipeline,
            colored_opaque_pipeline,
            face_mesh_pipeline,
            face_state_bgl,
            face_palette_buffer,
            line_pipeline,
            wide_line_pipeline,
            wide_edge_line_pipeline,
            mesh_uniform_buffer,
            sim_mesh_uniform_buffer,
            line_uniform_buffer,
            mesh_bind_group,
            sim_mesh_bind_group,
            line_bind_group,
            line_dim_uniform_buffer,
            line_dim_bind_group,
            line_edge_uniform_buffer,
            line_edge_bind_group,
            line_edge_wide_uniform_buffer,
            line_edge_wide_bind_group,
            line_selected_uniform_buffer,
            line_selected_bind_group,
            line_hover_uniform_buffer,
            line_hover_bind_group,
            rest_heatmap_uniform_buffer,
            rest_heatmap_bind_group,
            tier_preview_uniform_buffer,
            tier_preview_bind_group,
            area_regions_uniform_buffer,
            area_regions_bind_group,
            blit_pipeline,
            outline,
            blit_bind_group_layout,
            blit_sampler,
            offscreen: None,
            target_format,
            sample_count,
            mesh_data_list: Vec::new(),
            face_mesh_data_list: Vec::new(),
            grid_data,
            stock_data: None,
            solid_stock_data: None,
            fixture_data: None,
            polygon_data: Vec::new(),
            selection_lines: None,
            hover_lines: None,
            tab_lines: None,
            model_edge_lines: None,
            model_edges_key: None,
            model_wireframe_lines: None,
            model_wireframe_key: None,
            toolpath_data: Vec::new(),
            sim_mesh_data: None,
            height_planes_data: None,
            rest_heatmap_data: None,
            tier_preview_data: None,
            area_regions_data: None,
            reach_overlay_data: None,
            tool_model_data: None,
            collision_vertex_buffer: None,
            collision_vertex_count: 0,
            origin_axes_data: None,
            gpu_limits,
            mesh_upload_key: None,
            enriched_upload_key: None,
            face_state_key: None,
            collision_upload_key: None,
            rest_heatmap_upload_key: None,
            tier_preview_upload_key: None,
            area_regions_upload_key: None,
            reach_overlay_upload_key: None,
            obstacle_lines_key: None,
            obstacle_lines: Vec::new(),
            upload_stats: upload_cache::UploadStats::default(),
        }
    }

    /// The scene sample count that `new` stored.
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Test door: renders one scene frame without egui (G-PIPESMOKE).
    ///
    /// A pipeline and an attachment with different sample counts fail at
    /// pass time, not at pipeline creation. So the door runs
    /// `ensure_offscreen` and records the frame's own scene pass with an
    /// empty callback that draws the grid, the face meshes, the model
    /// edges and the polygon lines (`show_polygons` is the curves row: it
    /// gates `polygon_data` only). A first pass from the same
    /// `begin_scene_pass` sets each scene pipeline once. The door then
    /// submits, so a validation error reaches the caller's error scope.
    #[doc(hidden)]
    pub fn render_headless_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        selection_outline: bool,
        show_polygons: bool,
    ) {
        self.ensure_offscreen(device, width, height);
        let Some(offscreen) = self.offscreen.as_ref() else {
            return;
        };
        // The model arm draws `face_mesh_data_list` and `model_edge_lines`,
        // and the polygon arm `polygon_data` and `selection_lines`, so a
        // caller that fills them
        // first gets those draws too. The uniforms are the caller's
        // (`write_frame_uniforms`); with none written, nothing is visible.
        let callback = ViewportCallback {
            show_grid: true,
            show_model: true,
            display_mode: DisplayMode::ShadedWithEdges,
            show_polygons,
            show_selection_outline: selection_outline,
            ..ViewportCallback::empty(width, height)
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headless_frame"),
        });
        // The pipeline pass runs first: each pass clears the targets, so
        // the scene pass runs last and its frame stays for a readback.
        {
            let mut pass = offscreen.begin_scene_pass(&mut encoder);
            for pipeline in [
                &self.mesh_pipeline,
                &self.sim_mesh_pipeline,
                &self.height_plane_pipeline,
                &self.colored_opaque_pipeline,
                &self.face_mesh_pipeline,
                &self.line_pipeline,
                &self.wide_line_pipeline,
                &self.wide_edge_line_pipeline,
                self.outline.outline_pipeline(),
            ] {
                pass.set_pipeline(pipeline);
            }
        }
        callback.record_scene_pass(self, offscreen, &mut encoder);
        queue.submit(Some(encoder.finish()));
    }

    /// Ask the id readback ring for the id at pixel `px` (D §4.3). The next
    /// frame with the outline on copies it.
    #[doc(hidden)]
    pub fn request_id_pixel(&self, request: u64, px: [u32; 2]) {
        self.outline
            .readback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .request(request, px);
    }

    /// Advance the id readback ring and return the last answer. `wait`
    /// polls the device until the maps land (a test); the frame does not
    /// wait.
    #[doc(hidden)]
    pub fn collect_id_readback(&self, device: &wgpu::Device, wait: bool) -> Option<(u64, u32)> {
        let mut ring = self
            .outline
            .readback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ring.collect(device, wait);
        ring.answer()
    }

    /// The single-sample colour of the last headless frame, or `None`
    /// before the first frame. A test copies it out to read a pixel.
    #[doc(hidden)]
    pub fn headless_color_texture(&self) -> Option<&wgpu::Texture> {
        self.offscreen.as_ref().map(|o| &o.color_texture)
    }

    /// Write the mesh uniforms, the six line uniforms of one frame
    /// (`LineUniforms::frame_set`) and the outline uniforms (D §4). `prepare` calls it each frame; a
    /// headless test calls it before `render_headless_frame`.
    #[doc(hidden)]
    pub fn write_frame_uniforms(
        &self,
        queue: &wgpu::Queue,
        mesh: &MeshUniforms,
        lines: LineUniforms,
        edges: EdgeOffset,
        widths: LineWidths,
        outline: &outline::OutlineUniforms,
    ) {
        queue.write_buffer(&self.mesh_uniform_buffer, 0, bytemuck::bytes_of(mesh));
        self.outline.write_uniforms(queue, outline);
        // Every line group: one camera, written together so the groups
        // can never hold different view matrices.
        let groups = lines.frame_set(edges, widths);
        for (buffer, uniforms) in [
            (&self.line_uniform_buffer, &groups.plain),
            (&self.line_dim_uniform_buffer, &groups.dimmed),
            (&self.line_edge_uniform_buffer, &groups.edge),
            (&self.line_edge_wide_uniform_buffer, &groups.edge_wide),
            (&self.line_selected_uniform_buffer, &groups.selected),
            (&self.line_hover_uniform_buffer, &groups.hover),
        ] {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(uniforms));
        }
    }

    /// Ensure offscreen render targets exist at the given size.
    fn ensure_offscreen(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let max_tex = self.gpu_limits.max_texture_size;
        let width = width.max(1).min(max_tex);
        let height = height.max(1).min(max_tex);

        if let Some(existing) = &self.offscreen
            && existing.width == width
            && existing.height == height
        {
            return;
        }

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        gpu_safety::log_offscreen_budget(width, height, self.sample_count, self.target_format);

        // The multisampled colour. The pass only renders into it and
        // resolves it, so it needs no TEXTURE_BINDING.
        let msaa_color_view = (self.sample_count > 1).then(|| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("offscreen_color_msaa"),
                    size,
                    mip_level_count: 1,
                    sample_count: self.sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.target_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });

        // The single-sample colour: the resolve target and the blit source.
        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_depth"),
            size,
            mip_level_count: 1,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let color_view = color_texture.create_view(&Default::default());
        let depth_view = depth_texture.create_view(&Default::default());

        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bg"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });

        let outline = outline::OutlineTargets::new(device, &self.outline, width, height);
        self.offscreen = Some(OffscreenTargets {
            msaa_color_view,
            color_view,
            color_texture,
            depth_view,
            outline,
            blit_bind_group,
            width,
            height,
        });
    }
}

/// Per-frame callback data passed to the paint callback.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is one independent draw toggle for this frame"
)]
pub struct ViewportCallback {
    pub mesh_uniforms: MeshUniforms,
    pub line_uniforms: LineUniforms,
    /// The plain (STL) model list draws under this. Derived as
    /// `<any model carries a mesh> && show_model && workspace != Simulation`.
    pub has_mesh: bool,
    /// The enriched (STEP) model list draws under this. Before P6 that loop
    /// sat OUTSIDE `has_mesh`, so "Wireframe" hid an STL model and did
    /// nothing at all to a STEP one — one control, two behaviours, neither of
    /// them a wireframe (audit §3.1, fix §6.2).
    pub show_model: bool,
    /// FOLLOW_UPS §1: the model look. `Wireframe` draws no face fill and
    /// the triangle edges; `ShadedWithEdges` draws the STEP face edges
    /// (D4). Gated with `show_model`.
    pub display_mode: DisplayMode,
    /// D4: the move of the model edges toward the eye.
    pub edge_offset: EdgeOffset,
    /// The viewport facts of the quad lines (D §3.5).
    pub line_widths: LineWidths,
    /// D §4: the `selection_outline` row (the ID pass and the outline).
    pub show_selection_outline: bool,
    /// D §4.2: the camera facts and colours of the outline.
    pub outline_uniforms: outline::OutlineUniforms,
    pub show_grid: bool,
    pub show_stock: bool,
    /// The one line buffer that carries the obstacles, the alignment pins,
    /// the flip axis and the datum crosshair. Each kind is filtered INTO the
    /// buffer at upload time by its own registry flag, so this gate is "did
    /// any kind contribute".
    pub show_line_marks: bool,
    pub show_polygons: bool,
    pub show_solid_stock: bool,
    pub show_height_planes: bool,
    /// The cyan entry / ramp / helix markers on the selected toolpath.
    /// Before P6 they drew whenever a toolpath was selected, ignoring
    /// `show_cutting`, the per-toolpath entry and the scrub move limit
    /// (audit §6.10).
    pub show_entry_markers: bool,
    /// Rest-depth heatmap overlay (pencil detector #4). Derived as
    /// `viewport.show_rest_heatmap && workspace == Toolpaths && <selected
    /// toolpath has a rest_grid>` — see `app/viewport.rs`.
    pub show_rest_heatmap: bool,
    /// Multi-tool tier-map preview overlay (Phase U). Derived as
    /// `viewport.show_tier_preview && workspace == Toolpaths && <the planner
    /// holds a Ready preview>` — see `app/viewport.rs`. Independent of
    /// `show_rest_heatmap`: both may be true in one frame.
    pub show_tier_preview: bool,
    /// The By Area regions overlay. Derived as `viewport.show_area_regions
    /// && <the row precondition is Ready>` — see `app/viewport.rs`. Stacks
    /// with the two overlays above.
    pub show_area_regions: bool,
    /// Per-tool reach-map overlay (P5). Derived as
    /// `viewport.show_reach_map && workspace == Toolpaths &&
    /// viewport.show_model && <a Ready map is held for the selected
    /// toolpath>` — see `app/viewport.rs`.
    ///
    /// Unlike the two overlays above this one is EXCLUSIVE with the plain and
    /// enriched model draws: it is the model, re-coloured. Drawing both would
    /// z-fight two copies of the same surface.
    pub show_reach_overlay: bool,
    pub show_sim_mesh: bool,
    pub sim_mesh_opacity: f32,
    pub show_cutting: bool,
    pub show_rapids: bool,
    pub show_collisions: bool,
    /// Per-toolpath move-type visibility (toolpath_id → (show_cut, show_rapid)).
    /// AND'd with the global `show_cutting` / `show_rapids`. Missing entries
    /// default to both-visible.
    pub toolpath_move_visibility: std::collections::HashMap<rs_cam_core::ToolpathId, (bool, bool)>,
    /// Toolpaths whose [`crate::state::freshness::FreshnessState`] is
    /// `EditedSince` — the geometry uploaded for them was generated from
    /// inputs the project no longer holds (F2.2, R0.1 §4.4).
    ///
    /// Their moves draw through the same dimmed bind group the reach overlay
    /// uses. Drawn, not hidden: the operator asked to see this operation, and
    /// hiding it would replace a wrong picture with no picture. Dimming says
    /// "this is last generation's answer" while leaving it locatable.
    pub stale_toolpaths: std::collections::HashSet<rs_cam_core::ToolpathId>,
    pub show_tool_model: bool,
    /// Sim scrubbing: if Some, each toolpath in the map draws only its first
    /// N moves, N counted in its OWN moves (render review 2026-10-09, F6;
    /// see `SimulationState::toolpath_move_limits`). A toolpath that is not
    /// in the map is not in the run; it draws in full.
    pub toolpath_move_limits: Option<std::collections::HashMap<rs_cam_core::ToolpathId, usize>>,
    /// Show XYZ axes at the stock origin.
    ///
    /// The origin and the length that used to ride here were never read: the
    /// axes geometry is baked at upload from the stock config, and the draw
    /// uses `resources.origin_axes_data` alone (audit §4.1, fix §6.14).
    pub show_origin_axes: bool,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl ViewportCallback {
    /// A callback that draws nothing. Every toggle is off and the uniforms
    /// are zero. The headless frame door starts from it.
    #[doc(hidden)]
    pub fn empty(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            mesh_uniforms: bytemuck::Zeroable::zeroed(),
            line_uniforms: bytemuck::Zeroable::zeroed(),
            has_mesh: false,
            show_model: false,
            display_mode: DisplayMode::Shaded,
            edge_offset: EdgeOffset::default(),
            line_widths: LineWidths::default(),
            show_selection_outline: false,
            outline_uniforms: bytemuck::Zeroable::zeroed(),
            show_grid: false,
            show_stock: false,
            show_line_marks: false,
            show_polygons: false,
            show_solid_stock: false,
            show_height_planes: false,
            show_entry_markers: false,
            show_rest_heatmap: false,
            show_tier_preview: false,
            show_area_regions: false,
            show_reach_overlay: false,
            show_sim_mesh: false,
            sim_mesh_opacity: 0.0,
            show_cutting: false,
            show_rapids: false,
            show_collisions: false,
            toolpath_move_visibility: std::collections::HashMap::new(),
            stale_toolpaths: std::collections::HashSet::new(),
            show_tool_model: false,
            toolpath_move_limits: None,
            show_origin_axes: false,
            viewport_width,
            viewport_height,
        }
    }

    /// Records the 3D scene pass into `encoder`: the pass from
    /// [`OffscreenTargets::begin_scene_pass`] and every draw of this frame.
    fn record_scene_pass(
        &self,
        resources: &RenderResources,
        offscreen: &OffscreenTargets,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // D §4: the ID pass runs before the scene pass, only while the
        // outline draws (the row on, the model shaded).
        let outline = self.show_selection_outline
            && self.show_model
            && self.display_mode != DisplayMode::Wireframe;
        if outline {
            resources.outline.record_id_pass(
                encoder,
                &offscreen.outline,
                &resources.mesh_bind_group,
                &resources.face_mesh_data_list,
                if self.has_mesh {
                    &resources.mesh_data_list
                } else {
                    &[]
                },
            );
        }
        let mut pass = offscreen.begin_scene_pass(encoder);

        pass.set_viewport(
            0.0,
            0.0,
            self.viewport_width as f32,
            self.viewport_height as f32,
            0.0,
            1.0,
        );

        // Draw grid
        if self.show_grid {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, resources.grid_data.vertex_buffer.slice(..));
            pass.draw(0..resources.grid_data.vertex_count, 0..1);
        }

        // Draw stock wireframe
        if self.show_stock
            && let Some(stock) = &resources.stock_data
        {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, stock.vertex_buffer.slice(..));
            pass.draw(0..stock.vertex_count, 0..1);
        }

        // (Height planes drawn after opaque geometry — see below)

        // Draw origin axes at stock origin
        if self.show_origin_axes
            && let Some(axes) = &resources.origin_axes_data
        {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, axes.vertex_buffer.slice(..));
            pass.draw(0..axes.vertex_count, 0..1);
        }

        // Draw the reference lines: obstacles, pins, flip axis, datum.
        if self.show_line_marks
            && let Some(fixture) = &resources.fixture_data
        {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, fixture.vertex_buffer.slice(..));
            pass.draw(0..fixture.vertex_count, 0..1);
        }

        // Draw meshes (sim mesh replaces raw models when simulation is active)
        if self.show_sim_mesh {
            if let Some(sim) = &resources.sim_mesh_data {
                pass.set_pipeline(&resources.sim_mesh_pipeline);
                pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
                for chunk in &sim.chunks {
                    pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                    pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                }
            }
        } else if self.show_reach_overlay
            && let Some(reach) = &resources.reach_overlay_data
        {
            // P5 — the reach overlay IS the model, re-coloured per
            // vertex. It draws INSTEAD of the two model lists below, not
            // over them: two copies of one surface at the same depth
            // z-fight, and the reach answer is the one the operator asked
            // for. Opaque pipeline, so it also writes depth exactly as
            // the plain model does and the toolpath lines occlude the
            // same way.
            pass.set_pipeline(&resources.colored_opaque_pipeline);
            pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
            pass.set_vertex_buffer(0, reach.vertex_buffer.slice(..));
            pass.set_index_buffer(reach.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..reach.index_count, 0, 0..1);
        } else if self.show_model && self.display_mode != DisplayMode::Wireframe {
            // Draw all STEP models with the face pipeline (D2): group 1
            // is the model's own face state.
            for face_mesh in &resources.face_mesh_data_list {
                pass.set_pipeline(&resources.face_mesh_pipeline);
                pass.set_bind_group(0, &resources.mesh_bind_group, &[]);
                pass.set_bind_group(1, &face_mesh.state_bind_group, &[]);
                pass.set_vertex_buffer(0, face_mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(face_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..face_mesh.index_count, 0, 0..1);
            }
            // Draw all plain (STL) models
            if self.has_mesh {
                for mesh in &resources.mesh_data_list {
                    pass.set_pipeline(&resources.mesh_pipeline);
                    pass.set_bind_group(0, &resources.mesh_bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        // D §4.2: the selection outline, after the model and before every
        // line, so a highlight line or the hover ring stays on top.
        if outline {
            resources
                .outline
                .draw_outline(&mut pass, &offscreen.outline);
        }

        // Draw solid stock (semi-transparent, after mesh so model renders first)
        if self.show_solid_stock
            && let Some(solid) = &resources.solid_stock_data
        {
            pass.set_pipeline(&resources.sim_mesh_pipeline);
            pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
            pass.set_vertex_buffer(0, solid.vertex_buffer.slice(..));
            pass.set_index_buffer(solid.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..solid.index_count, 0, 0..1);
        }

        // Draw height plane overlays AFTER opaque geometry so they
        // alpha-blend over the model instead of blocking it via depth writes.
        if self.show_height_planes
            && let Some(hp) = &resources.height_planes_data
        {
            pass.set_pipeline(&resources.height_plane_pipeline);
            pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
            pass.set_vertex_buffer(0, hp.vertex_buffer.slice(..));
            pass.set_index_buffer(hp.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..hp.index_count, 0, 0..1);
        }

        // Draw rest-depth heatmap overlay (pencil detector #4) — same
        // depth-read-only, alpha-blended treatment as height planes so it
        // drapes over the model/stock without z-fighting or occluding it.
        if self.show_rest_heatmap
            && let Some(heatmap) = &resources.rest_heatmap_data
        {
            pass.set_pipeline(&resources.height_plane_pipeline);
            pass.set_bind_group(0, &resources.rest_heatmap_bind_group, &[]);
            for chunk in &heatmap.chunks {
                pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }
        }

        // Draw the multi-tool tier preview — same treatment again, and
        // drawn after the rest heatmap so that when both are up the plan
        // preview is the one on top: it is the thing being decided on.
        if self.show_tier_preview
            && let Some(preview) = &resources.tier_preview_data
        {
            pass.set_pipeline(&resources.height_plane_pipeline);
            pass.set_bind_group(0, &resources.tier_preview_bind_group, &[]);
            for chunk in &preview.chunks {
                pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }
        }

        // Draw the By Area regions overlay — the same treatment again.
        if self.show_area_regions
            && let Some(regions) = &resources.area_regions_data
        {
            pass.set_pipeline(&resources.height_plane_pipeline);
            pass.set_bind_group(0, &resources.area_regions_bind_group, &[]);
            for chunk in &regions.chunks {
                pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }
        }

        // D4: the face edges, after every model draw (the reach overlay
        // too: the edges are geometry, not a colour field). The edge
        // offset moves them toward the eye, so they win the depth test.
        // They write no depth, so the overlay lines below, with the same
        // offset, draw over them on a shared segment. Quads of
        // `LINE_EDGE_PX` (D §3.5).
        // FOLLOW_UPS §1: the wireframe (no face fill) draws the triangle
        // edges first, as quads of `LINE_EDGE_PX`; the face edges draw on
        // top of it.
        if self.show_model
            && self.display_mode == DisplayMode::Wireframe
            && let Some(wire) = &resources.model_wireframe_lines
        {
            pass.set_pipeline(&resources.wide_edge_line_pipeline);
            pass.set_bind_group(0, &resources.line_edge_wide_bind_group, &[]);
            draw_segments(&mut pass, wire);
        }
        // The face edges draw with Shaded with edges only. In Wireframe the
        // dark edge ink (`MODEL_EDGE`) cannot read on the dark ground, and
        // every face boundary is a triangle edge the wireframe already
        // draws (live look round 2).
        if self.show_model
            && self.display_mode == DisplayMode::ShadedWithEdges
            && let Some(edges) = &resources.model_edge_lines
        {
            pass.set_pipeline(&resources.wide_edge_line_pipeline);
            pass.set_bind_group(0, &resources.line_edge_wide_bind_group, &[]);
            draw_segments(&mut pass, edges);
        }

        // Draw polygon/DXF/SVG lines, then the pick highlight and the hover
        // ring. They lie on the faces, so they take the edge offset, as the
        // model edges do. The drawing stays 1 px `LineList`; the selection
        // highlight and the hover ring are quads. The curves row
        // (`show_polygons`) gates the drawing only: the highlight and the
        // hover ring draw whenever their buffers exist (live look round 5;
        // a hidden model uploads neither).
        if self.show_polygons {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_edge_bind_group, &[]);
            for poly_gpu in &resources.polygon_data {
                pass.set_vertex_buffer(0, poly_gpu.vertex_buffer.slice(..));
                pass.draw(0..poly_gpu.vertex_count, 0..1);
            }
        }
        pass.set_pipeline(&resources.wide_line_pipeline);
        if let Some(selected) = &resources.selection_lines {
            pass.set_bind_group(0, &resources.line_selected_bind_group, &[]);
            draw_segments(&mut pass, selected);
        }
        if let Some(hover) = &resources.hover_lines {
            pass.set_bind_group(0, &resources.line_hover_bind_group, &[]);
            draw_segments(&mut pass, hover);
        }

        // Geometry selection P4.6: the tab marks of the tab editor. The
        // editor is on by an explicit act, so the marks draw with the
        // curves overlay off too. Quads of `LINE_HOVER_PX`.
        if let Some(tabs) = &resources.tab_lines {
            pass.set_pipeline(&resources.wide_line_pipeline);
            pass.set_bind_group(0, &resources.line_hover_bind_group, &[]);
            draw_segments(&mut pass, tabs);
        }

        // Draw collision markers
        if self.show_collisions
            && resources.collision_vertex_count > 0
            && let Some(buf) = &resources.collision_vertex_buffer
        {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..resources.collision_vertex_count, 0..1);
        }

        // Draw tool model during simulation
        if self.show_tool_model
            && let Some(tool) = &resources.tool_model_data
        {
            pass.set_pipeline(&resources.line_pipeline);
            pass.set_bind_group(0, &resources.line_bind_group, &[]);
            pass.set_vertex_buffer(0, tool.vertex_buffer.slice(..));
            pass.draw(0..tool.vertex_count, 0..1);
        }

        // Draw toolpaths (with optional move limit for sim scrubbing)
        for tp_gpu in &resources.toolpath_data {
            let limit = self
                .toolpath_move_limits
                .as_ref()
                .and_then(|limits| tp_gpu.toolpath_id.and_then(|id| limits.get(&id).copied()));
            let (max_cut, max_rapid) = if let Some(limit) = limit {
                tp_gpu.vertices_for_moves(limit)
            } else {
                (tp_gpu.cut_vertex_count, tp_gpu.rapid_vertex_count)
            };

            pass.set_pipeline(&resources.line_pipeline);
            // P5.3 — while the reach overlay is painting the model, the
            // moves are drawn DIMMED so the shading reads under them.
            // The condition is the reach draw's own, not a second
            // opinion about it: same flag, same buffer presence, so the
            // dim cannot be on while the shading is absent. Cutting moves
            // and rapids alike; no registry flag moves, so the Overlays
            // panel still shows the row ON and the operator still owns
            // the switch.
            //
            // F2.2 adds the second reason to take the same treatment: a
            // toolpath whose result the core no longer holds is drawn
            // from the GUI's retained copy, i.e. from the PREVIOUS
            // generation's inputs. One dim factor serves both because
            // the line pipeline carries exactly one dim uniform, and a
            // second constant for a 0.05 difference nobody has ruled on
            // would be a new tunable, not a clearer picture. Consequence
            // to know: while the reach overlay is painting, every move is
            // dimmed anyway, so the stale dim adds no signal there — the
            // card chip and the inspector header are the ones that still
            // say it.
            let stale = tp_gpu
                .toolpath_id
                .is_some_and(|id| self.stale_toolpaths.contains(&id));
            let moves_bind_group =
                if stale || (self.show_reach_overlay && resources.reach_overlay_data.is_some()) {
                    &resources.line_dim_bind_group
                } else {
                    &resources.line_bind_group
                };
            pass.set_bind_group(0, moves_bind_group, &[]);

            let (tp_show_cut, tp_show_rapid) = tp_gpu
                .toolpath_id
                .and_then(|id| self.toolpath_move_visibility.get(&id).copied())
                .unwrap_or((true, true));
            if self.show_cutting && tp_show_cut && max_cut > 1 {
                pass.set_vertex_buffer(0, tp_gpu.cut_vertex_buffer.slice(..));
                pass.draw(0..max_cut, 0..1);
            }
            if self.show_rapids && tp_show_rapid && max_rapid > 1 {
                pass.set_vertex_buffer(0, tp_gpu.rapid_vertex_buffer.slice(..));
                pass.draw(0..max_rapid, 0..1);
            }

            // P6 (audit §6.10) — the entry markers and the cutter ghost
            // now respect the move gates. Before this they drew whenever
            // the buffer existed: switching cutting moves off left the
            // cyan markers floating on their own, and a sim scrub
            // revealed the whole entry at move zero.
            //
            // Neither buffer carries a per-move index, so
            // `vertices_for_moves` cannot trim them. The honest gate is
            // therefore "not scrubbing": under a move limit they are
            // hidden outright rather than shown untrimmed.
            let overlays_allowed =
                self.show_cutting && tp_show_cut && self.toolpath_move_limits.is_none();

            // Draw entry path preview overlay (ramp/helix/lead-in indicator)
            if overlays_allowed
                && self.show_entry_markers
                && let Some(ref buf) = tp_gpu.entry_preview_buffer
                && tp_gpu.entry_preview_count > 1
            {
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..tp_gpu.entry_preview_count, 0..1);
            }

            // Draw tool-profile preview overlay (cutter silhouette ghost)
            if overlays_allowed
                && let Some(ref buf) = tp_gpu.tool_profile_preview_buffer
                && tp_gpu.tool_profile_preview_count > 1
            {
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..tp_gpu.tool_profile_preview_count, 0..1);
            }
        }
    }
}

/// Draw a `LineList` buffer as quads: one instance per segment, six
/// vertices per quad (`vs_wide`). A trailing odd vertex draws nothing.
fn draw_segments(pass: &mut wgpu::RenderPass<'_>, lines: &PolygonGpuData) {
    pass.set_vertex_buffer(0, lines.vertex_buffer.slice(..));
    pass.draw(0..6, 0..lines.vertex_count / 2);
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        #[expect(
            clippy::unwrap_used,
            reason = "RenderResources inserted in RsCamApp::new; always present."
        )]
        let resources: &mut RenderResources = callback_resources.get_mut().unwrap();

        // Ensure offscreen targets are the right size
        resources.ensure_offscreen(device, self.viewport_width, self.viewport_height);
        // D §4.3: map the readback copies of earlier frames; no wait.
        let _ = resources.collect_id_readback(device, false);

        // Upload uniforms: the mesh and the three line groups.
        resources.write_frame_uniforms(
            queue,
            &self.mesh_uniforms,
            self.line_uniforms,
            self.edge_offset,
            self.line_widths,
            &self.outline_uniforms,
        );
        // Upload the colored mesh uniforms when a colored pipeline draws.
        // The reach overlay draws with `colored_opaque_pipeline` through
        // `sim_mesh_bind_group`, so it needs these uniforms even on a project
        // that holds no STEP part and runs no simulation. That pipeline uses
        // REPLACE blend, so the opacity written below does not reach it.
        let needs_colored_uniforms = self.show_sim_mesh
            || self.show_solid_stock
            || self.show_height_planes
            || self.show_reach_overlay;
        if needs_colored_uniforms {
            let opacity = if self.show_sim_mesh {
                self.sim_mesh_opacity
            } else {
                0.15 // solid stock / height planes translucency
            };
            let sim_uniforms = ColoredMeshUniforms {
                view_proj: self.mesh_uniforms.view_proj,
                light_dir: self.mesh_uniforms.light_dir,
                _pad0: 0.0,
                camera_pos: self.mesh_uniforms.camera_pos,
                opacity,
            };
            queue.write_buffer(
                &resources.sim_mesh_uniform_buffer,
                0,
                bytemuck::bytes_of(&sim_uniforms),
            );
        }
        // Rest-depth heatmap: own uniform buffer/bind-group (see the
        // `rest_heatmap_uniform_buffer` field doc) so its opacity never
        // inherits the 0.15 solid-stock/height-plane translucency above.
        if self.show_rest_heatmap {
            let heatmap_uniforms = ColoredMeshUniforms {
                view_proj: self.mesh_uniforms.view_proj,
                light_dir: self.mesh_uniforms.light_dir,
                _pad0: 0.0,
                camera_pos: self.mesh_uniforms.camera_pos,
                opacity: REST_HEATMAP_OPACITY,
            };
            queue.write_buffer(
                &resources.rest_heatmap_uniform_buffer,
                0,
                bytemuck::bytes_of(&heatmap_uniforms),
            );
        }
        // Multi-tool tier preview: its own buffer for the same reason, so
        // the two overlays can be up together without sharing an opacity.
        if self.show_tier_preview {
            let tier_uniforms = ColoredMeshUniforms {
                view_proj: self.mesh_uniforms.view_proj,
                light_dir: self.mesh_uniforms.light_dir,
                _pad0: 0.0,
                camera_pos: self.mesh_uniforms.camera_pos,
                opacity: TIER_PREVIEW_OPACITY,
            };
            queue.write_buffer(
                &resources.tier_preview_uniform_buffer,
                0,
                bytemuck::bytes_of(&tier_uniforms),
            );
        }
        if self.show_area_regions {
            let area_uniforms = ColoredMeshUniforms {
                view_proj: self.mesh_uniforms.view_proj,
                light_dir: self.mesh_uniforms.light_dir,
                _pad0: 0.0,
                camera_pos: self.mesh_uniforms.camera_pos,
                opacity: AREA_REGIONS_OPACITY,
            };
            queue.write_buffer(
                &resources.area_regions_uniform_buffer,
                0,
                bytemuck::bytes_of(&area_uniforms),
            );
        }
        // Render 3D scene to offscreen texture with depth buffer.
        // After ensure_offscreen and write_buffer, we only need immutable access.
        #[expect(
            clippy::unwrap_used,
            reason = "RenderResources inserted in RsCamApp::new; always present."
        )]
        let resources: &RenderResources = callback_resources.get().unwrap();
        #[expect(
            clippy::unwrap_used,
            reason = "ensure_offscreen was called above, so offscreen is always Some."
        )]
        let offscreen = resources.offscreen.as_ref().unwrap();

        self.record_scene_pass(resources, offscreen, encoder);

        vec![]
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        #[expect(
            clippy::unwrap_used,
            reason = "RenderResources inserted in RsCamApp::new; always present."
        )]
        let resources: &RenderResources = callback_resources.get().unwrap();

        if let Some(offscreen) = &resources.offscreen {
            let viewport = info.viewport_in_pixels();
            render_pass.set_viewport(
                viewport.left_px as f32,
                viewport.top_px as f32,
                viewport.width_px as f32,
                viewport.height_px as f32,
                0.0,
                1.0,
            );
            let clip = info.clip_rect_in_pixels();
            render_pass.set_scissor_rect(
                clip.left_px as u32,
                clip.top_px as u32,
                clip.width_px as u32,
                clip.height_px as u32,
            );

            // Blit offscreen texture to viewport
            render_pass.set_pipeline(&resources.blit_pipeline);
            render_pass.set_bind_group(0, &offscreen.blit_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // fullscreen triangle
        }
    }
}

// --- WGSL Shaders ---

const MESH_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,
    camera_pos: vec3<f32>,
    surface: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.world_pos = in.position;
    return out;
}

// Round 3: the lighting of the face shader (one material, the headlight,
// the hemisphere ambient), so an STL and a STEP part read alike.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = uniforms.surface.rgb;
    let light = normalize(uniforms.light_dir);
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let normal = normalize(in.world_normal);
    // The normal on the side the eye sees.
    let n = select(normal, -normal, dot(normal, view_dir) < 0.0);
    let hemi = mix(uniforms.ground.rgb, uniforms.sky.rgb, n.z * 0.5 + 0.5);
    let ndotl = max(dot(n, light), 0.0);
    let half_dir = normalize(light + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 32.0);
    let specular = vec3<f32>(0.25, 0.25, 0.25) * spec;
    return vec4<f32>(c * hemi + c * ndotl + specular, 1.0);
}
"#;

const COLORED_MESH_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,
    camera_pos: vec3<f32>,
    opacity: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) vertex_color: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.world_pos = in.position;
    out.vertex_color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light = normalize(uniforms.light_dir);

    // Ambient (tinted by vertex color so unlit areas retain hue)
    let ambient = in.vertex_color * 0.25;

    // Two-sided lighting: flip normal if facing away from light
    let n = select(normal, -normal, dot(normal, light) < 0.0);

    // Diffuse (Lambert)
    let ndotl = max(dot(n, light), 0.0);
    let diffuse = in.vertex_color * ndotl;

    // Specular (Blinn-Phong)
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(light + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 32.0);
    let specular = vec3<f32>(0.2, 0.2, 0.2) * spec;

    let color = ambient + diffuse + specular;
    return vec4<f32>(color, uniforms.opacity);
}

@fragment
fn fs_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    let light = normalize(uniforms.light_dir);
    let normal = normalize(in.world_normal);
    let ambient = in.vertex_color * 0.25;
    let n = select(normal, -normal, dot(normal, light) < 0.0);
    let ndotl = max(dot(n, light), 0.0);
    let diffuse = in.vertex_color * ndotl;
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(light + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 32.0);
    let specular = vec3<f32>(0.2, 0.2, 0.2) * spec;
    let color = ambient + diffuse + specular;
    return vec4<f32>(color, 1.0);
}
"#;

/// The face pipeline (D2). The palette slots are `colors::FACE_SLOT_*`;
/// `face_shader_reads_the_palette_slots` holds the literals to them. The
/// shading terms 0.2 and 32.0 are the `fs_opaque` terms; Q5 replaced the
/// flat ambient 0.25 with the hemisphere term and the flat hover tint with
/// the fresnel rim (`FACE_RIM_*`); round 3 made the base the one material
/// (`MeshUniforms::surface`, `colors::MODEL_SURFACE`) and the light the
/// headlight (`headlight_dir`).
const FACE_MESH_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,
    camera_pos: vec3<f32>,
    surface: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var<storage, read> face_state: array<u32>;
@group(1) @binding(1) var<uniform> palette: array<vec4<f32>, 10>;

struct VsIn  { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) face_id: u32 };
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) nrm: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) face_id: u32,
};

@vertex fn vs_main(i: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.nrm = i.nrm; o.world = i.pos; o.face_id = i.face_id;
    return o;
}

fn face_colour(id: u32) -> vec3<f32> {
    let s = face_state[id];
    if ((s & 2u) != 0u) { return palette[0].rgb; }
    let st = (s >> 2u) & 63u;
    if (st != 0u) { return palette[st - 1u].rgb; }
    return u.surface.rgb;
}

@fragment fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    let c = face_colour(i.face_id);
    let l = normalize(u.light_dir);
    let v = normalize(u.camera_pos - i.world);
    let n0 = normalize(i.nrm);
    // The normal on the side the eye sees.
    let n = select(n0, -n0, dot(n0, v) < 0.0);
    // Q5: the hemisphere term, the sky from +Z and the ground from -Z.
    let hemi = mix(u.ground.rgb, u.sky.rgb, n.z * 0.5 + 0.5);
    let spec = pow(max(dot(n, normalize(l + v)), 0.0), 32.0);
    var rgb = c * hemi + c * max(dot(n, l), 0.0) + vec3<f32>(0.2) * spec;
    // Q5: the hovered face takes a fresnel rim (Schlick) in the hover colour.
    if ((face_state[i.face_id] & 1u) != 0u) {
        let rim = palette[9];
        let f = rim.z + (1.0 - rim.z) * pow(1.0 - max(dot(n, v), 0.0), rim.y);
        rgb = rgb + palette[1].rgb * (rim.x * f);
    }
    return vec4<f32>(rgb, 1.0);
}
"#;

const LINE_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    dim: f32,
    edge_tol: f32,
    edge_k: f32,
    _pad: f32,
    eye: vec4<f32>,
    wide: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// D4: the lines on the model move toward the eye by
// max(edge_tol, edge_k * dist^2) (`EdgeOffset`). edge_tol = 0 for every
// other line.
fn moved(position: vec3<f32>) -> vec3<f32> {
    var p = position;
    if (uniforms.edge_tol > 0.0) {
        let to_eye = uniforms.eye.xyz - p * uniforms.eye.w;
        let dist = length(to_eye);
        let off = max(uniforms.edge_tol, uniforms.edge_k * dist * dist);
        p = p + to_eye / max(dist, 1e-6) * off;
    }
    return p;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(moved(in.position), 1.0);
    out.color = in.color;
    return out;
}

struct SegmentInput {
    @location(0) a_position: vec3<f32>,
    @location(1) a_color: vec3<f32>,
    @location(2) b_position: vec3<f32>,
    @location(3) b_color: vec3<f32>,
};

// Live look round 3: a quad pixel half a width from the centre line lies
// over the face beside the line. Where that face rises toward the eye it
// is nearer than the line, wins the depth test and shows the outline's ink
// beside the line. So the quad moves toward the eye by the depth a face at
// the slope `wide.w` (depth per pixel footprint, `OUTLINE_FOLD_PX`) gains
// over half the width. The footprint is the world length of one pixel at
// the vertex, across the eye ray. A vertex behind the eye does not move.
fn lifted(p: vec3<f32>, half_viewport: vec2<f32>, half_width: f32) -> vec3<f32> {
    let c = uniforms.view_proj * vec4<f32>(p, 1.0);
    if (c.w <= 1e-5 || uniforms.wide.w <= 0.0) {
        return p;
    }
    let e = normalize(uniforms.eye.xyz - p * uniforms.eye.w);
    let axis = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(e.z) > 0.9);
    let across = normalize(cross(e, axis));
    let cu = uniforms.view_proj * vec4<f32>(p + across, 1.0);
    if (cu.w <= 1e-5) {
        return p;
    }
    let px_per_mm = length((cu.xy / cu.w - c.xy / c.w) * half_viewport);
    let footprint = 1.0 / max(px_per_mm, 1e-6);
    return p + e * (half_width * footprint * uniforms.wide.w);
}

// D §3.5: one segment per instance, six vertices per quad. The quad is
// `wide.z` physical pixels across in screen space, and each end extends
// by half the width (a square cap), so a ring closes without a gap.
@vertex
fn vs_wide(@builtin(vertex_index) vi: u32, s: SegmentInput) -> VertexOutput {
    var ends = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var sides = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let t = ends[vi % 6u];
    let side = sides[vi % 6u];
    let half_viewport = uniforms.wide.xy * 0.5;
    let half_width = uniforms.wide.z * 0.5;
    var ca = uniforms.view_proj * vec4<f32>(lifted(moved(s.a_position), half_viewport, half_width), 1.0);
    var cb = uniforms.view_proj * vec4<f32>(lifted(moved(s.b_position), half_viewport, half_width), 1.0);
    var out: VertexOutput;
    out.color = select(s.a_color, s.b_color, t > 0.5);
    // Clip the segment to the front of the eye (w > 0), so the screen
    // direction is defined. A segment wholly behind the eye draws nothing.
    let w_min = 1e-5;
    if (ca.w < w_min && cb.w < w_min) {
        out.clip_position = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        return out;
    }
    if (ca.w < w_min) {
        ca = mix(ca, cb, (w_min - ca.w) / (cb.w - ca.w));
    }
    if (cb.w < w_min) {
        cb = mix(cb, ca, (w_min - cb.w) / (ca.w - cb.w));
    }
    let sa = ca.xy / ca.w * half_viewport;
    let sb = cb.xy / cb.w * half_viewport;
    var dir = sb - sa;
    let len = length(dir);
    if (len > 1e-6) {
        dir = dir / len;
    } else {
        dir = vec2<f32>(1.0, 0.0);
    }
    let normal = vec2<f32>(-dir.y, dir.x);
    let push = normal * side * half_width + dir * (t * 2.0 - 1.0) * half_width;
    var c = select(ca, cb, t > 0.5);
    c = vec4<f32>(c.xy + push / half_viewport * c.w, c.z, c.w);
    out.clip_position = c;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color * uniforms.dim, 1.0);
}
"#;

const BLIT_SHADER_SRC: &str = r#"
@group(0) @binding(0) var t_color: texture_2d<f32>;
@group(0) @binding(1) var s_color: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // Fullscreen triangle (oversized, clipped by viewport)
    let x = f32(i32(idx & 1u)) * 4.0 - 1.0;
    let y = f32(i32(idx >> 1u)) * 4.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_color, s_color, in.uv);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// The line uniform block is 112 B in Rust; the WGSL struct holds the
    /// same fields in the same order.
    #[test]
    fn line_uniforms_are_112_bytes() {
        assert_eq!(std::mem::size_of::<LineUniforms>(), 112);
        for field in [
            "dim: f32,",
            "edge_tol: f32,",
            "edge_k: f32,",
            "_pad: f32,",
            "eye: vec4<f32>,",
            "wide: vec4<f32>,",
        ] {
            assert!(
                LINE_SHADER_SRC.contains(field),
                "the WGSL Uniforms lacks `{field}`"
            );
        }
    }

    /// Only the edge group carries the offset: the plain and the dimmed
    /// groups write edge_tol = 0, so the grid and the toolpaths do not
    /// move. The edge group is for the lines on the model: the model
    /// edges and the overlay lines (`record_scene_pass`).
    #[test]
    fn only_the_edge_group_moves() {
        let base = LineUniforms {
            edge_tol: 5.0,
            edge_k: 5.0,
            ..LineUniforms::new([[0.0; 4]; 4], [1.0, 2.0, 3.0, 1.0])
        };
        let edges = EdgeOffset::perspective(0.1, 0.1);
        let widths = LineWidths {
            viewport_px: [800.0, 600.0],
            pixels_per_point: 2.0,
        };
        let groups = base.frame_set(edges, widths);
        let (plain, dimmed) = (groups.plain, groups.dimmed);
        assert_eq!((plain.edge_tol, plain.edge_k, plain.dim), (0.0, 0.0, 1.0));
        assert_eq!(
            (dimmed.edge_tol, dimmed.edge_k, dimmed.dim),
            (0.0, 0.0, MOVE_DIM_UNDER_REACH)
        );
        assert_eq!((plain.wide, dimmed.wide), ([0.0; 4], [0.0; 4]));
        // Every group on the model moves; the quad groups also carry their
        // width token in physical pixels (points times pixels per point).
        use crate::ui::tokens::{LINE_EDGE_PX, LINE_HOVER_PX, LINE_SELECTED_PX};
        for (group, width) in [
            (groups.edge, 0.0),
            (groups.edge_wide, LINE_EDGE_PX * 2.0),
            (groups.selected, LINE_SELECTED_PX * 2.0),
            (groups.hover, LINE_HOVER_PX * 2.0),
        ] {
            assert_eq!(
                (group.edge_tol, group.edge_k, group.dim),
                (edges.tol, edges.k, 1.0)
            );
            assert_eq!(group.eye, base.eye);
            assert_eq!(group.view_proj, base.view_proj);
            let expect = if width > 0.0 {
                [800.0, 600.0, width, outline::OUTLINE_FOLD_PX]
            } else {
                [0.0; 4]
            };
            assert_eq!(group.wide, expect);
        }
    }

    /// The offset follows the derivation on `EdgeOffset`.
    #[test]
    fn the_edge_offset_follows_the_depth_step_derivation() {
        let camera = camera::OrbitCamera::new();
        let chord = rs_cam_core::io::STEP_CHORD_TOLERANCE as f32;
        let z = 300.0_f32;
        let near = camera.near;
        let p = EdgeOffset::perspective(near, chord);
        let four_steps = 2f32.powi(-22) * z * z / near;
        assert!(
            (p.at(z) - four_steps.max(chord)).abs() <= 1e-6 * four_steps,
            "perspective: max(chord, 2^-22 z^2 / near)"
        );
        // Close to the eye the chord tolerance is the floor.
        assert_eq!(p.at(1.0), chord);
        let o = EdgeOffset::orthographic(camera.far, chord);
        assert_eq!(o.k, 0.0);
        assert_eq!(
            o.at(z),
            chord.max(4.0 * 2f32.powi(-24) * 2.0 * camera.far),
            "orthographic: max(chord, 4 depth steps over 2 far)"
        );
        // A zero chord still turns the perspective offset on.
        assert!(EdgeOffset::perspective(near, 0.0).tol > 0.0);
    }

    /// The WGSL literals of the face shader are the `colors::FACE_SLOT_*`
    /// slots and the palette length, so the shader and the palette cannot
    /// drift apart.
    #[test]
    fn face_shader_reads_the_palette_slots() {
        let src = FACE_MESH_SHADER_SRC;
        let len = colors::FACE_PALETTE_LEN;
        assert!(src.contains(&format!("array<vec4<f32>, {len}>")));
        let selected = colors::FACE_SLOT_SELECTED;
        assert!(src.contains(&format!("return palette[{selected}].rgb; }}")));
        // Round 3: a face with no state is the one material.
        assert!(src.contains("return u.surface.rgb;"));
        assert!(!src.contains("palette[id %"), "no per-face pastel");
        // Q5: the hover colour feeds the rim, not a flat tint.
        let hover = colors::FACE_SLOT_HOVER;
        assert!(src.contains(&format!("rgb = rgb + palette[{hover}].rgb * (rim.x * f);")));
        let rim = colors::FACE_SLOT_RIM;
        assert!(src.contains(&format!("let rim = palette[{rim}];")));
        // The hemisphere and the material come from `MeshUniforms`, which
        // the STL shader reads too.
        for shader in [FACE_MESH_SHADER_SRC, MESH_SHADER_SRC] {
            assert!(shader.contains("surface: vec4<f32>,"));
            assert!(shader.contains("ground.rgb, "));
        }
        assert_eq!(std::mem::size_of::<MeshUniforms>(), 144);
        let palette = colors::face_palette();
        assert_eq!(
            palette.get(rim as usize).copied(),
            Some([
                colors::FACE_RIM_STRENGTH,
                colors::FACE_RIM_POWER,
                colors::FACE_RIM_F0,
                0.0
            ])
        );
        // The hemisphere mean is the ambient token.
        for (k, (sky, ground)) in colors::LIGHT_SKY
            .array()
            .into_iter()
            .zip(colors::LIGHT_GROUND.array())
            .enumerate()
        {
            let mean = (sky + ground) / 2.0;
            assert!(
                (mean - colors::LIGHT_AMBIENT_MEAN).abs() <= 0.005,
                "channel {k}: {mean}"
            );
        }
        assert_eq!(
            std::mem::size_of_val(&colors::face_palette()),
            16 * colors::FACE_PALETTE_LEN
        );
    }

    /// Round 3: the headlight follows the camera. A camera that looks down
    /// -Z with +X right and +Y up puts it at `HEADLIGHT_VIEW`, normalised.
    #[test]
    fn the_headlight_turns_with_the_camera() {
        let top = headlight_dir([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let [x, y, z] = colors::HEADLIGHT_VIEW;
        let len = (x * x + y * y + z * z).sqrt();
        for (a, b) in top.iter().zip([x / len, y / len, z / len]) {
            assert!((a - b).abs() < 1e-6, "{top:?}");
        }
        // A front camera (looking +Y, eye at -Y): back = -Y, up = +Z.
        let front = headlight_dir([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]);
        assert!(front[1] < 0.0 && front[2] > 0.0, "{front:?}");
    }
}
