//! The ID pass and the selection outline (design D §4).
//!
//! The ID pass is a second, single-sample pass before the scene pass. It
//! draws every face mesh and every plain mesh into an `R32Uint` target with
//! its own depth: `(selected << 31) | (model slot << 16) | face id`. An STL
//! takes face `0xFFFF`; the background stays 0. The pass runs only while the
//! `selection_outline` row is on (`ViewportCallback::show_selection_outline`).
//!
//! The outline is a full-screen draw inside the scene pass, after the model
//! and before the lines, so a selection highlight line or a hover ring stays
//! on top. It reads the id and the id depth with `textureLoad` and writes:
//!
//! - `accent` on a pixel that is not on a selected face and has a neighbour
//!   that is (just outside a selected face);
//! - `ink` on a silhouette (a neighbour of another model, or the background)
//!   and on a fold of one model (a depth step above [`OUTLINE_FOLD_PX`]
//!   pixel footprints);
//! - nothing on every other pixel (`discard`).
//!
//! Design D §4.2 puts the edge test in the blit. It moved into the scene
//! pass so that the overlay lines draw over the outline and the headless
//! door can read it back; the outline-off path draws nothing new.
//!
//! [`IdReadback`] is the one-pixel readback ring for a later visible-only
//! pick (the readback belt idea of rerun `re_renderer` and the GPU picking
//! article of WebGPU Fundamentals, `CREDITS.md`; no code is copied).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use egui_wgpu::wgpu;

use super::mesh_render::{FaceMeshGpuData, FaceMeshVertex, MeshGpuData, MeshVertex};

/// The fold threshold of the outline, in pixel footprints: a neighbour
/// deeper than this many pixel footprints is across a fold. A smooth face
/// steps by `footprint · tan θ` per pixel at the angle θ between its normal
/// and the view, so 8 footprints is a face steeper than atan 8 = 82.9°.
/// Only the rim of a curved face reaches that. A design choice (D §4.2:
/// the threshold is a ratio of the depth step to the depth), not a
/// measurement; it follows the eight depth steps of `EdgeOffset`.
pub const OUTLINE_FOLD_PX: f32 = 8.0;

/// The id format of the ID pass.
pub const ID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// The uniforms of the outline draw. 64 B; the WGSL `Outline` struct holds
/// the same fields in the same order.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OutlineUniforms {
    /// The silhouette and fold colour (palette slot `FACE_SLOT_INK`).
    pub ink: [f32; 4],
    /// The colour outside a selected face (palette slot `FACE_SLOT_ACCENT`).
    pub accent: [f32; 4],
    /// The near plane (perspective); unused in orthographic.
    pub near: f32,
    /// The far plane (perspective), or the half depth range (orthographic,
    /// `-far..far`).
    pub far: f32,
    /// One pixel footprint over the depth (perspective: `2 tan(fov/2) / H`)
    /// or in mm (orthographic: `2 half_h / H`).
    pub px_ratio: f32,
    /// [`OUTLINE_FOLD_PX`].
    pub fold_px: f32,
    /// 1 for orthographic.
    pub ortho: u32,
    pub _pad: [u32; 3],
}

impl OutlineUniforms {
    fn colours() -> ([f32; 4], [f32; 4]) {
        let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        (
            rgba(super::colors::MODEL_EDGE),
            rgba(super::colors::OUTLINE_ACCENT),
        )
    }

    /// Perspective with the vertical field of view `fov` (radians) and a
    /// viewport `height_px` physical pixels high.
    pub fn perspective(near: f32, far: f32, fov: f32, height_px: f32) -> Self {
        let (ink, accent) = Self::colours();
        Self {
            ink,
            accent,
            near,
            far,
            px_ratio: 2.0 * (fov * 0.5).tan() / height_px.max(1.0),
            fold_px: OUTLINE_FOLD_PX,
            ortho: 0,
            _pad: [0; 3],
        }
    }

    /// Orthographic with depth range `-far..far` and the visible half
    /// height `half_h` (mm).
    pub fn orthographic(far: f32, half_h: f32, height_px: f32) -> Self {
        let (ink, accent) = Self::colours();
        Self {
            ink,
            accent,
            near: 0.0,
            far,
            px_ratio: 2.0 * half_h / height_px.max(1.0),
            fold_px: OUTLINE_FOLD_PX,
            ortho: 1,
            _pad: [0; 3],
        }
    }
}

const ID_SHADER_SRC: &str = r#"
struct Uniforms { view_proj: mat4x4<f32>, light_dir: vec3<f32>, camera_pos: vec3<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var<storage, read> face_state: array<u32>;

struct FaceIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) face_id: u32 };
struct MeshIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32> };
struct IdOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) id: u32,
};

// The model slot is the instance index of the draw (1-based, < 2^15).
@vertex fn vs_face(i: FaceIn, @builtin(instance_index) slot: u32) -> IdOut {
    var o: IdOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.id = ((slot & 0x7FFFu) << 16u) | (i.face_id & 0xFFFFu);
    return o;
}

@fragment fn fs_face(i: IdOut) -> @location(0) u32 {
    let sel = (face_state[i.id & 0xFFFFu] >> 1u) & 1u;
    return (sel << 31u) | i.id;
}

@vertex fn vs_mesh(i: MeshIn, @builtin(instance_index) slot: u32) -> IdOut {
    var o: IdOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.id = ((slot & 0x7FFFu) << 16u) | 0xFFFFu;
    return o;
}

@fragment fn fs_mesh(i: IdOut) -> @location(0) u32 {
    return i.id;
}
"#;

/// The outline draw (D §4.2 rules, see the module doc).
pub(super) const OUTLINE_SHADER_SRC: &str = r#"
struct Outline {
    ink: vec4<f32>,
    accent: vec4<f32>,
    near: f32,
    far: f32,
    px_ratio: f32,
    fold_px: f32,
    ortho: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
};
@group(0) @binding(0) var t_id: texture_2d<u32>;
@group(0) @binding(1) var t_depth: texture_depth_2d;
@group(0) @binding(2) var<uniform> o: Outline;

@vertex fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(idx & 1u)) * 4.0 - 1.0;
    let y = f32(i32(idx >> 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// The eye distance of a stored depth. nalgebra builds GL matrices (clip z
// in [-w, w]); wgpu keeps [0, w] with no remap (`EdgeOffset`).
fn eye_depth(d: f32) -> f32 {
    if (o.ortho != 0u) {
        return d * o.far;
    }
    return 2.0 * o.far * o.near / ((o.far + o.near) - d * (o.far - o.near));
}

@fragment fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dim = vec2<i32>(textureDimensions(t_id));
    let p = clamp(vec2<i32>(pos.xy), vec2<i32>(0), dim - 1);
    let c = textureLoad(t_id, p, 0).r;
    let cm = (c >> 16u) & 0x7FFFu;
    let zc = eye_depth(textureLoad(t_depth, p, 0));
    let step = o.fold_px * o.px_ratio * select(zc, 1.0, o.ortho != 0u);
    var offs = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    var ink = false;
    var accent = false;
    for (var k = 0; k < 4; k++) {
        let q = clamp(p + offs[k], vec2<i32>(0), dim - 1);
        let n = textureLoad(t_id, q, 0).r;
        let nm = (n >> 16u) & 0x7FFFu;
        if (nm != cm) {
            // A silhouette: another model, or the background.
            ink = true;
        } else if (cm != 0u) {
            // A fold of one model.
            let zn = eye_depth(textureLoad(t_depth, q, 0));
            if (abs(zn - zc) > step) { ink = true; }
        }
        if ((n >> 31u) == 1u && (c >> 31u) == 0u) { accent = true; }
    }
    if (accent) { return vec4<f32>(o.accent.rgb, 1.0); }
    if (ink) { return vec4<f32>(o.ink.rgb, 1.0); }
    discard;
}
"#;

/// The pipelines, the layout and the uniform of the ID pass and the
/// outline. `RenderResources::new` builds them, so G-PIPESMOKE validates
/// them.
pub(super) struct OutlinePipelines {
    id_face_pipeline: wgpu::RenderPipeline,
    id_mesh_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    outline_bgl: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    pub(super) readback: Mutex<IdReadback>,
}

impl OutlinePipelines {
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        scene_multisample: wgpu::MultisampleState,
        mesh_bgl: &wgpu::BindGroupLayout,
        face_state_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let id_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("id_shader"),
            source: wgpu::ShaderSource::Wgsl(ID_SHADER_SRC.into()),
        });
        let id_depth = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let id_target = [Some(wgpu::ColorTargetState {
            format: ID_FORMAT,
            // Blending an integer format is a validation error.
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let id_pipeline = |label: &str,
                           layout: &wgpu::PipelineLayout,
                           vs: &str,
                           fs: &str,
                           buffer: wgpu::VertexBufferLayout<'static>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &id_shader,
                    entry_point: Some(vs),
                    buffers: &[Some(buffer)],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &id_shader,
                    entry_point: Some(fs),
                    targets: &id_target,
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(id_depth.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let id_face_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("id_face_pl"),
            bind_group_layouts: &[Some(mesh_bgl), Some(face_state_bgl)],
            immediate_size: 0,
        });
        let id_mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("id_mesh_pl"),
            bind_group_layouts: &[Some(mesh_bgl)],
            immediate_size: 0,
        });
        let id_face_pipeline = id_pipeline(
            "id_face_pipeline",
            &id_face_layout,
            "vs_face",
            "fs_face",
            FaceMeshVertex::layout(),
        );
        let id_mesh_pipeline = id_pipeline(
            "id_mesh_pipeline",
            &id_mesh_layout,
            "vs_mesh",
            "fs_mesh",
            MeshVertex::layout(),
        );

        let outline_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outline_shader"),
            source: wgpu::ShaderSource::Wgsl(OUTLINE_SHADER_SRC.into()),
        });
        let outline_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
        let outline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("outline_pl"),
            bind_group_layouts: &[Some(&outline_bgl)],
            immediate_size: 0,
        });
        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("outline_pipeline"),
            layout: Some(&outline_layout),
            vertex: wgpu::VertexState {
                module: &outline_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &outline_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // In the scene pass: test nothing, write nothing, so the lines
            // after it keep their own depth test against the model.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: scene_multisample,
            multiview_mask: None,
            cache: None,
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("outline_uniforms"),
            size: std::mem::size_of::<OutlineUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            id_face_pipeline,
            id_mesh_pipeline,
            outline_pipeline,
            outline_bgl,
            uniform_buffer,
            readback: Mutex::new(IdReadback::new(device)),
        }
    }

    /// The outline pipeline, for the pipeline pass of the headless door
    /// (it is the one outline pipeline at the scene sample count).
    pub(super) fn outline_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.outline_pipeline
    }

    pub(super) fn write_uniforms(&self, queue: &wgpu::Queue, uniforms: &OutlineUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Record the ID pass into `targets`: every face mesh (slots 1..) and
    /// then every plain mesh (the slots after). Then copy the pixel of a
    /// pending readback request.
    pub(super) fn record_id_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &OutlineTargets,
        mesh_bind_group: &wgpu::BindGroup,
        faces: &[FaceMeshGpuData],
        meshes: &[MeshGpuData],
    ) {
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("id_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.id_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.id_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let mut slot: u32 = 1;
            pass.set_pipeline(&self.id_face_pipeline);
            pass.set_bind_group(0, mesh_bind_group, &[]);
            for face in faces {
                pass.set_bind_group(1, &face.state_bind_group, &[]);
                pass.set_vertex_buffer(0, face.vertex_buffer.slice(..));
                pass.set_index_buffer(face.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..face.index_count, 0, slot..slot + 1);
                slot += 1;
            }
            pass.set_pipeline(&self.id_mesh_pipeline);
            pass.set_bind_group(0, mesh_bind_group, &[]);
            for mesh in meshes {
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, slot..slot + 1);
                slot += 1;
            }
        }
        self.readback
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .encode(encoder, targets);
    }

    /// Draw the outline in the scene pass (after the model, before the
    /// lines).
    pub(super) fn draw_outline(&self, pass: &mut wgpu::RenderPass<'_>, targets: &OutlineTargets) {
        pass.set_pipeline(&self.outline_pipeline);
        pass.set_bind_group(0, &targets.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The ID targets of one offscreen size.
pub(super) struct OutlineTargets {
    id_texture: wgpu::Texture,
    id_view: wgpu::TextureView,
    id_depth_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl OutlineTargets {
    pub(super) fn new(
        device: &wgpu::Device,
        pipelines: &OutlinePipelines,
        width: u32,
        height: u32,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let id_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("id_target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ID_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let id_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("id_depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let id_view = id_texture.create_view(&Default::default());
        let id_depth_view = id_depth.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("outline_bg"),
            layout: &pipelines.outline_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&id_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: pipelines.uniform_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            id_texture,
            id_view,
            id_depth_view,
            bind_group,
            width,
            height,
        }
    }
}

/// The size of one readback buffer: the copy alignment of a wider crop.
const READBACK_BYTES: u64 = 256;

/// One buffer of the ring.
enum ReadbackSlot {
    Free,
    /// The copy of request `.0` is in a submitted command buffer.
    Copied(u64),
    /// The map of request `.0` is asked; the flag turns on when it lands.
    Mapping(u64, Arc<AtomicBool>),
}

/// The one-pixel readback ring of the ID target (D §4.3). A request names a
/// pixel; the ID pass copies it into a free buffer; a later frame maps the
/// buffer and publishes `(request, id)`. The GUI never waits on it: the
/// answer lands one or two frames later.
pub struct IdReadback {
    buffers: [wgpu::Buffer; 3],
    slots: [ReadbackSlot; 3],
    request: Option<(u64, [u32; 2])>,
    answer: Option<(u64, u32)>,
}

impl IdReadback {
    fn new(device: &wgpu::Device) -> Self {
        let buffer = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: READBACK_BYTES,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            buffers: [
                buffer("id_readback_0"),
                buffer("id_readback_1"),
                buffer("id_readback_2"),
            ],
            slots: [ReadbackSlot::Free, ReadbackSlot::Free, ReadbackSlot::Free],
            request: None,
            answer: None,
        }
    }

    /// Ask for the id at pixel `px` (physical pixels of the offscreen
    /// target) under the request number `id`.
    pub fn request(&mut self, id: u64, px: [u32; 2]) {
        self.request = Some((id, px));
    }

    /// The last published `(request, id)`.
    pub fn answer(&self) -> Option<(u64, u32)> {
        self.answer
    }

    /// Copy the requested pixel into a free buffer (after the ID pass). A
    /// pixel outside the target drops the request.
    fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, targets: &OutlineTargets) {
        let Some((id, [x, y])) = self.request else {
            return;
        };
        if x >= targets.width || y >= targets.height {
            self.request = None;
            return;
        }
        let Some(i) = self
            .slots
            .iter()
            .position(|s| matches!(s, ReadbackSlot::Free))
        else {
            return;
        };
        let (Some(buffer), Some(slot)) = (self.buffers.get(i), self.slots.get_mut(i)) else {
            return;
        };
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.id_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                // One row: no 256 B row padding is needed.
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        *slot = ReadbackSlot::Copied(id);
        self.request = None;
    }

    /// Advance the ring: ask the map of each copied buffer (its copy was
    /// submitted with an earlier frame), poll the device without a wait
    /// (`wait` true: until the maps land, for a test), and publish the
    /// mapped ids.
    pub(super) fn collect(&mut self, device: &wgpu::Device, wait: bool) {
        for (buffer, slot) in self.buffers.iter().zip(self.slots.iter_mut()) {
            if let ReadbackSlot::Copied(id) = *slot {
                let done = Arc::new(AtomicBool::new(false));
                let flag = Arc::clone(&done);
                buffer.map_async(wgpu::MapMode::Read, ..4, move |result| {
                    flag.store(result.is_ok(), Ordering::Release);
                });
                *slot = ReadbackSlot::Mapping(id, done);
            }
        }
        let poll = if wait {
            wgpu::PollType::wait_indefinitely()
        } else {
            wgpu::PollType::Poll
        };
        let _ = device.poll(poll);
        for (buffer, slot) in self.buffers.iter().zip(self.slots.iter_mut()) {
            let ReadbackSlot::Mapping(id, done) = slot else {
                continue;
            };
            if !done.load(Ordering::Acquire) {
                continue;
            }
            let value = buffer.get_mapped_range(..4).ok().and_then(|view| {
                let bytes: [u8; 4] = view.get(..4)?.try_into().ok()?;
                Some(u32::from_le_bytes(bytes))
            });
            buffer.unmap();
            if let Some(value) = value {
                self.answer = Some((*id, value));
            }
            *slot = ReadbackSlot::Free;
        }
    }
}
