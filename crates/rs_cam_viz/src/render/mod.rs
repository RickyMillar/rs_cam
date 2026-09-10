pub mod camera;
pub mod colors;
pub mod fixture_render;
pub mod gpu_safety;
pub mod grid_render;
pub mod height_planes;
pub mod mesh_render;
pub mod sim_render;
pub mod stock_render;
pub mod toolpath_render;
pub mod upload_cache;

use egui_wgpu::wgpu;

use fixture_render::FixtureGpuData;
use grid_render::GridGpuData;
use height_planes::HeightPlanesGpuData;
use mesh_render::{MeshGpuData, MeshVertex};
use sim_render::{ColoredMeshVertex, SimMeshGpuData, ToolModelGpuData};
use stock_render::StockGpuData;
use toolpath_render::ToolpathGpuData;

/// GPU uniform data for mesh rendering (Phong shading).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub light_dir: [f32; 3],
    pub _pad0: f32,
    pub camera_pos: [f32; 3],
    pub _pad1: f32,
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

/// GPU uniform data for line rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineUniforms {
    pub view_proj: [[f32; 4]; 4],
    /// Multiplier applied to every line's colour in the fragment stage.
    ///
    /// `1.0` for the ordinary pass. The second, DIMMED bind group carries
    /// [`MOVE_DIM_UNDER_REACH`] and the toolpath move draws switch to it
    /// while the reach overlay is on — see the constant.
    pub dim: f32,
    /// `dim` is one f32 in a uniform block, which WGSL rounds up to a
    /// 16-byte stride; the padding is explicit so the Rust and WGSL layouts
    /// cannot silently disagree.
    pub _pad: [f32; 3],
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
/// as TUBES and can thin them; the viewport draws
/// `PrimitiveTopology::LineList`, which is one pixel wide and has no width
/// control in wgpu at all. So the live rule is the 0.45 colour factor alone,
/// and the ribbon half of F4 has no lever here.
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

/// Offscreen render targets with depth buffer.
struct OffscreenTargets {
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    blit_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
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
    /// Opaque colored mesh pipeline — same vertex format as sim_mesh but no alpha blending.
    /// Used for STEP models with per-face colors that should render as solid objects.
    colored_opaque_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
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

    // Blit pipeline (copy offscreen to egui render pass)
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,

    // Offscreen render targets (resized per frame)
    offscreen: Option<OffscreenTargets>,
    target_format: wgpu::TextureFormat,

    // Scene data
    pub mesh_data_list: Vec<MeshGpuData>,
    pub enriched_mesh_data_list: Vec<mesh_render::EnrichedMeshGpuData>,
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
    /// Inputs `enriched_mesh_data_list` was last built from.
    pub enriched_upload_key: Option<upload_cache::EnrichedUploadKey>,
    /// Inputs the collision-marker buffer was last built from.
    pub collision_upload_key: Option<upload_cache::CollisionUploadKey>,
    /// Inputs `rest_heatmap_data` was last built from. `None` means the
    /// overlay is (correctly) absent, which is also a cacheable state.
    pub rest_heatmap_upload_key: Option<upload_cache::RestHeatmapUploadKey>,
    /// Inputs `tier_preview_data` was last built from. `None` means no
    /// preview is held, which is also a cacheable state — so toggling the
    /// overlay's visibility checkbox rebuilds nothing.
    pub tier_preview_upload_key: Option<upload_cache::TierPreviewUploadKey>,
    /// Inputs `reach_overlay_data` was last built from. `None` means no map
    /// is held, which is also a cacheable state — so toggling the overlay's
    /// visibility checkbox rebuilds nothing.
    pub reach_overlay_upload_key: Option<upload_cache::ReachOverlayUploadKey>,
    /// Lifetime counts of upload passes and per-resource buffer builds.
    pub upload_stats: upload_cache::UploadStats,
}

impl RenderResources {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
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
                buffers: &[MeshVertex::layout()],
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
            multisample: wgpu::MultisampleState::default(),
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
                buffers: &[ColoredMeshVertex::layout()],
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
            multisample: wgpu::MultisampleState::default(),
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
                    buffers: &[ColoredMeshVertex::layout()],
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
                depth_stencil: Some(depth_read_only),
                multisample: wgpu::MultisampleState::default(),
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
                    buffers: &[ColoredMeshVertex::layout()],
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
                multisample: wgpu::MultisampleState::default(),
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

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_pipeline"),
            layout: Some(&line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[line_vertex_layout],
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
            depth_stencil: Some(depth_stencil),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- Blit pipeline (fullscreen triangle, samples offscreen texture) ---
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
            line_pipeline,
            mesh_uniform_buffer,
            sim_mesh_uniform_buffer,
            line_uniform_buffer,
            mesh_bind_group,
            sim_mesh_bind_group,
            line_bind_group,
            line_dim_uniform_buffer,
            line_dim_bind_group,
            rest_heatmap_uniform_buffer,
            rest_heatmap_bind_group,
            tier_preview_uniform_buffer,
            tier_preview_bind_group,
            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
            offscreen: None,
            target_format,
            mesh_data_list: Vec::new(),
            enriched_mesh_data_list: Vec::new(),
            grid_data,
            stock_data: None,
            solid_stock_data: None,
            fixture_data: None,
            polygon_data: Vec::new(),
            toolpath_data: Vec::new(),
            sim_mesh_data: None,
            height_planes_data: None,
            rest_heatmap_data: None,
            tier_preview_data: None,
            reach_overlay_data: None,
            tool_model_data: None,
            collision_vertex_buffer: None,
            collision_vertex_count: 0,
            origin_axes_data: None,
            gpu_limits,
            mesh_upload_key: None,
            enriched_upload_key: None,
            collision_upload_key: None,
            rest_heatmap_upload_key: None,
            tier_preview_upload_key: None,
            reach_overlay_upload_key: None,
            upload_stats: upload_cache::UploadStats::default(),
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

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
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

        self.offscreen = Some(OffscreenTargets {
            color_view,
            depth_view,
            blit_bind_group,
            width,
            height,
        });
    }
}

/// Per-frame callback data passed to the paint callback.
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
    pub show_grid: bool,
    pub show_stock: bool,
    /// The one line buffer that carries fixtures, keep-outs, alignment pins,
    /// the flip axis and the datum crosshair. Each kind is filtered INTO the
    /// buffer at upload time by its own registry flag, so this gate is "did
    /// any kind contribute".
    pub show_fixtures: bool,
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
    /// If Some, only draw toolpath moves up to this index (sim scrubbing).
    pub toolpath_move_limit: Option<usize>,
    /// Show XYZ axes at the stock origin.
    ///
    /// The origin and the length that used to ride here were never read: the
    /// axes geometry is baked at upload from the stock config, and the draw
    /// uses `resources.origin_axes_data` alone (audit §4.1, fix §6.14).
    pub show_origin_axes: bool,
    pub viewport_width: u32,
    pub viewport_height: u32,
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
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &mut RenderResources = callback_resources.get_mut().unwrap();

        // Ensure offscreen targets are the right size
        resources.ensure_offscreen(device, self.viewport_width, self.viewport_height);

        // Upload uniforms
        queue.write_buffer(
            &resources.mesh_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.mesh_uniforms),
        );
        // Always upload colored mesh uniforms when any colored pipeline will be used.
        // The colored_opaque_pipeline (STEP) uses REPLACE blend so opacity doesn't
        // affect it, but it still needs valid view_proj/light_dir uniforms.
        // The reach overlay draws with `colored_opaque_pipeline` through
        // `sim_mesh_bind_group`, so it needs these uniforms even on a project
        // that holds no STEP part and runs no simulation. That pipeline uses
        // REPLACE blend, so the opacity written below does not reach it.
        let needs_colored_uniforms = self.show_sim_mesh
            || self.show_solid_stock
            || self.show_height_planes
            || self.show_reach_overlay
            || !resources.enriched_mesh_data_list.is_empty();
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
        queue.write_buffer(
            &resources.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.line_uniforms),
        );
        // Same camera, dimmed colours. Written every frame beside the plain
        // one so the two can never hold different view matrices.
        queue.write_buffer(
            &resources.line_dim_uniform_buffer,
            0,
            bytemuck::bytes_of(&LineUniforms {
                dim: MOVE_DIM_UNDER_REACH,
                ..self.line_uniforms
            }),
        );

        // Render 3D scene to offscreen texture with depth buffer.
        // After ensure_offscreen and write_buffer, we only need immutable access.
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
        let resources: &RenderResources = callback_resources.get().unwrap();
        // SAFETY: ensure_offscreen was called above, so offscreen is always Some.
        #[allow(clippy::unwrap_used)]
        let offscreen = resources.offscreen.as_ref().unwrap();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3d_scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &offscreen.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.102,
                            g: 0.102,
                            b: 0.149,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &offscreen.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

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

            // Draw fixture and keep-out wireframes
            if self.show_fixtures
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
                        pass.set_index_buffer(
                            chunk.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
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
            } else if self.show_model {
                // Draw all enriched (STEP) models
                for enriched in &resources.enriched_mesh_data_list {
                    pass.set_pipeline(&resources.colored_opaque_pipeline);
                    pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
                    pass.set_vertex_buffer(0, enriched.vertex_buffer.slice(..));
                    pass.set_index_buffer(
                        enriched.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..enriched.index_count, 0, 0..1);
                }
                // Draw all plain (STL) models
                if self.has_mesh {
                    for mesh in &resources.mesh_data_list {
                        pass.set_pipeline(&resources.mesh_pipeline);
                        pass.set_bind_group(0, &resources.mesh_bind_group, &[]);
                        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    }
                }
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

            // Draw polygon/DXF/SVG lines
            if self.show_polygons {
                pass.set_pipeline(&resources.line_pipeline);
                pass.set_bind_group(0, &resources.line_bind_group, &[]);
                for poly_gpu in &resources.polygon_data {
                    pass.set_vertex_buffer(0, poly_gpu.vertex_buffer.slice(..));
                    pass.draw(0..poly_gpu.vertex_count, 0..1);
                }
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
                let (max_cut, max_rapid) = if let Some(limit) = self.toolpath_move_limit {
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
                let moves_bind_group = if stale
                    || (self.show_reach_overlay && resources.reach_overlay_data.is_some())
                {
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
                    self.show_cutting && tp_show_cut && self.toolpath_move_limit.is_none();

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
        } // render pass ends

        vec![]
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        // SAFETY: RenderResources inserted in RsCamApp::new; always present.
        #[allow(clippy::unwrap_used)]
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light = normalize(uniforms.light_dir);

    // Ambient
    let ambient = vec3<f32>(0.15, 0.15, 0.18);

    // Two-sided lighting: flip normal if facing away from light
    let n = select(normal, -normal, dot(normal, light) < 0.0);

    // Diffuse (Lambert)
    let ndotl = max(dot(n, light), 0.0);
    let diffuse_color = vec3<f32>(0.6, 0.55, 0.5);
    let diffuse = diffuse_color * ndotl;

    // Specular (Blinn-Phong)
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(light + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 32.0);
    let specular = vec3<f32>(0.25, 0.25, 0.25) * spec;

    let color = ambient + diffuse + specular;
    return vec4<f32>(color, 1.0);
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

const LINE_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    dim: f32,
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

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
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
