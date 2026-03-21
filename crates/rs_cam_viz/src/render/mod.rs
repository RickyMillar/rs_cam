pub mod camera;
pub mod fixture_render;
pub mod grid_render;
pub mod mesh_render;
pub mod sim_render;
pub mod stock_render;
pub mod toolpath_render;

use egui_wgpu::wgpu;

use fixture_render::FixtureGpuData;
use grid_render::GridGpuData;
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

/// GPU uniform data for line rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineUniforms {
    pub view_proj: [[f32; 4]; 4],
}

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

/// All GPU resources for the 3D viewport, stored in egui_wgpu::CallbackResources.
pub struct RenderResources {
    // 3D scene pipelines (render to offscreen with depth)
    mesh_pipeline: wgpu::RenderPipeline,
    sim_mesh_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    mesh_uniform_buffer: wgpu::Buffer,
    sim_mesh_uniform_buffer: wgpu::Buffer,
    line_uniform_buffer: wgpu::Buffer,
    mesh_bind_group: wgpu::BindGroup,
    sim_mesh_bind_group: wgpu::BindGroup,
    line_bind_group: wgpu::BindGroup,

    // Blit pipeline (copy offscreen to egui render pass)
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,

    // Offscreen render targets (resized per frame)
    offscreen: Option<OffscreenTargets>,
    target_format: wgpu::TextureFormat,

    // Scene data
    pub mesh_data: Option<MeshGpuData>,
    pub grid_data: GridGpuData,
    pub stock_data: Option<StockGpuData>,
    pub solid_stock_data: Option<stock_render::SolidStockGpuData>,
    pub fixture_data: Option<FixtureGpuData>,
    pub toolpath_data: Vec<ToolpathGpuData>,
    pub sim_mesh_data: Option<SimMeshGpuData>,
    pub tool_model_data: Option<ToolModelGpuData>,
    pub collision_vertex_buffer: Option<wgpu::Buffer>,
    pub collision_vertex_count: u32,
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
            bind_group_layouts: &[&mesh_bind_group_layout],
            push_constant_ranges: &[],
        });

        let depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
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
            multiview: None,
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

        let sim_mesh_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sim_mesh_pl"),
                bind_group_layouts: &[&mesh_bind_group_layout],
                push_constant_ranges: &[],
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
            multiview: None,
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
                    visibility: wgpu::ShaderStages::VERTEX,
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

        let line_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line_pl"),
            bind_group_layouts: &[&line_bind_group_layout],
            push_constant_ranges: &[],
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
            multiview: None,
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
            bind_group_layouts: &[&blit_bind_group_layout],
            push_constant_ranges: &[],
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
            multiview: None,
            cache: None,
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let grid_data = GridGpuData::new(device, 200.0, 10.0);

        Self {
            mesh_pipeline,
            sim_mesh_pipeline,
            line_pipeline,
            mesh_uniform_buffer,
            sim_mesh_uniform_buffer,
            line_uniform_buffer,
            mesh_bind_group,
            sim_mesh_bind_group,
            line_bind_group,
            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
            offscreen: None,
            target_format,
            mesh_data: None,
            grid_data,
            stock_data: None,
            solid_stock_data: None,
            fixture_data: None,
            toolpath_data: Vec::new(),
            sim_mesh_data: None,
            tool_model_data: None,
            collision_vertex_buffer: None,
            collision_vertex_count: 0,
        }
    }

    /// Ensure offscreen render targets exist at the given size.
    fn ensure_offscreen(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

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
    pub has_mesh: bool,
    pub show_grid: bool,
    pub show_stock: bool,
    pub show_fixtures: bool,
    pub show_solid_stock: bool,
    pub show_sim_mesh: bool,
    pub sim_mesh_opacity: f32,
    pub show_cutting: bool,
    pub show_rapids: bool,
    pub show_collisions: bool,
    pub show_tool_model: bool,
    /// If Some, only draw toolpath moves up to this index (sim scrubbing).
    pub toolpath_move_limit: Option<usize>,
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
        let resources: &mut RenderResources = callback_resources.get_mut().unwrap();

        // Ensure offscreen targets are the right size
        resources.ensure_offscreen(device, self.viewport_width, self.viewport_height);

        // Upload uniforms
        queue.write_buffer(
            &resources.mesh_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.mesh_uniforms),
        );
        if self.show_sim_mesh || self.show_solid_stock {
            let opacity = if self.show_sim_mesh {
                self.sim_mesh_opacity
            } else {
                0.18 // solid stock translucency
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
        queue.write_buffer(
            &resources.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.line_uniforms),
        );

        // Render 3D scene to offscreen texture with depth buffer.
        // After ensure_offscreen and write_buffer, we only need immutable access.
        let resources: &RenderResources = callback_resources.get().unwrap();
        let offscreen = resources.offscreen.as_ref().unwrap();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3d_scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &offscreen.color_view,
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

            // Draw solid stock (semi-transparent, before wireframe)
            if self.show_solid_stock
                && let Some(solid) = &resources.solid_stock_data
            {
                pass.set_pipeline(&resources.sim_mesh_pipeline);
                pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
                pass.set_vertex_buffer(0, solid.vertex_buffer.slice(..));
                pass.set_index_buffer(solid.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..solid.index_count, 0, 0..1);
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

            // Draw fixture and keep-out wireframes
            if self.show_fixtures
                && let Some(fixture) = &resources.fixture_data
            {
                pass.set_pipeline(&resources.line_pipeline);
                pass.set_bind_group(0, &resources.line_bind_group, &[]);
                pass.set_vertex_buffer(0, fixture.vertex_buffer.slice(..));
                pass.draw(0..fixture.vertex_count, 0..1);
            }

            // Draw mesh (sim mesh replaces raw STL when simulation is active)
            if self.show_sim_mesh {
                if let Some(sim) = &resources.sim_mesh_data {
                    pass.set_pipeline(&resources.sim_mesh_pipeline);
                    pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
                    pass.set_vertex_buffer(0, sim.vertex_buffer.slice(..));
                    pass.set_index_buffer(sim.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..sim.index_count, 0, 0..1);
                }
            } else if self.has_mesh
                && let Some(mesh) = &resources.mesh_data
            {
                pass.set_pipeline(&resources.mesh_pipeline);
                pass.set_bind_group(0, &resources.mesh_bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
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
                pass.set_bind_group(0, &resources.line_bind_group, &[]);

                if self.show_cutting && max_cut > 1 {
                    pass.set_vertex_buffer(0, tp_gpu.cut_vertex_buffer.slice(..));
                    pass.draw(0..max_cut, 0..1);
                }
                if self.show_rapids && max_rapid > 1 {
                    pass.set_vertex_buffer(0, tp_gpu.rapid_vertex_buffer.slice(..));
                    pass.draw(0..max_rapid, 0..1);
                }

                // Draw entry path preview overlay (ramp/helix/lead-in indicator)
                if let Some(ref buf) = tp_gpu.entry_preview_buffer
                    && tp_gpu.entry_preview_count > 1
                {
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..tp_gpu.entry_preview_count, 0..1);
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
"#;

const LINE_SHADER_SRC: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
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
    return vec4<f32>(in.color, 1.0);
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
