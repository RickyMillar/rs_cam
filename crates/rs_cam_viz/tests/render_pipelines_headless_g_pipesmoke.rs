//! G-PIPESMOKE — every viewport render pipeline builds on a real wgpu device.
//!
//! Commit 0e6fb9f1 (G-LINEVIS) made the line shader's FRAGMENT stage read
//! `uniforms.dim`. The matching `line_bind_group_layout` entry in
//! `render/mod.rs` stayed VERTEX-only. wgpu validates a pipeline layout
//! against BOTH stages, so `create_render_pipeline('line_pipeline')` failed
//! and the P5.3 release crashed on launch. No test in this workspace created
//! a wgpu device, so no gate could see it.
//!
//! This sentry adds that gate. It requests a headless adapter: a software
//! adapter first, any adapter second (`fixtures/gpu.rs`). It then calls
//! `RenderResources::new(&device, format, sample_count)`. That function is the app's ONE
//! construction site for the viewport's GPU state, and it builds:
//!
//! - 5 shader modules: `mesh_shader`, `face_mesh_shader`, `sim_mesh_shader`,
//!   `line_shader`, `blit_shader`
//! - 4 bind group layouts: `mesh_bgl`, `face_state_bgl`, `line_bgl`,
//!   `blit_bgl`
//! - 12 render pipelines: `mesh_pipeline`, `face_mesh_pipeline` (D2),
//!   `sim_mesh_pipeline`, `height_plane_pipeline`, `colored_opaque_pipeline`,
//!   `line_pipeline`, `wide_line_pipeline` (D §3.5 quads),
//!   `wide_edge_line_pipeline` (D4: quads, no depth write), `id_face_pipeline`,
//!   `id_mesh_pipeline`, `outline_pipeline` (D §4: `render::outline`),
//!   `blit_pipeline`
//!
//! `app.rs` calls the same function with `render_state.target_format`, and
//! nothing else in the crate creates a pipeline, a layout, or a shader
//! module. So the sentry's subject is the whole surface.
//!
//! The five scene pipelines take the scene sample count from `new` (D1:
//! 4× MSAA where the adapter supports it). The blit stays at count 1,
//! because egui's pass is single-sample. A pipeline and an attachment with
//! different counts fail at pass time, not at pipeline creation. So the
//! second arm renders one frame through `render_headless_frame` at the
//! count that `scene_sample_count` gives, and one frame at count 1.
//!
//! The format list is the app's own. `egui_wgpu::preferred_framebuffer_format`
//! picks `Rgba8Unorm` or `Bgra8Unorm` when the surface offers one, else the
//! surface's first format — which on Linux is commonly an sRGB variant. The
//! test builds for all four.
//!
//! The third arm reads pixels back. It draws one STEP face at Z 20, its
//! model edges, and a pick highlight line on one edge segment. The pick
//! highlight colour must win over the edge and the face on that segment.
//! Before the fix the edges wrote the depth with their offset, so the
//! highlight (same segment, no offset) failed the depth test and the
//! operator saw no highlight on a picked STEP ring. The highlight is a
//! quad line of `LINE_SELECTED_PX` (D §3.5): at one pixel per point the
//! fourth arm reads at least 2 pixels across it.
//!
//! The sixth arm is the Q5 lighting: a face that faces away from the
//! light is not black. It takes the hemisphere sky (`colors::LIGHT_SKY`)
//! times its colour, since the diffuse term is 0 there.
//!
//! The fifth arm is the selection outline (D §4): with the face selected
//! (bit 1 of its state word), the pixels just outside its silhouette are
//! the accent colour and no pixel inside is. The id readback ring returns
//! the id of a pixel inside: selected, model slot 1, face 0.
//!
//! ## Not covered
//!
//! Two gaps. State them before you read a pass as total cover.
//!
//! - The headless frame records the scene pass only. The blit into egui's
//!   pass (`paint`) does not run. `ensure_offscreen` builds `blit_bg`, so
//!   wgpu checks it against `blit_bgl`, but no pass binds it.
//! - A uniform buffer that is SMALLER than its WGSL struct still passes. All
//!   the layouts set `min_binding_size: None`, so wgpu defers that check to
//!   draw time. The `_pad` field on `LineUniforms` guards the size by
//!   convention alone. `min_binding_size: Some(...)` on each layout entry
//!   would move the check here; that is a `render/mod.rs` change.
//!
//! ## No adapter
//!
//! A machine with no GPU and no software Vulkan or GL driver cannot run this
//! check. The test then prints the reason and returns. It does not fail, and
//! it is not `#[ignore]`d, so the run is a real pass with a visible reason.
//!
//! **Run it with `-- --nocapture`** to see the adapter line and any skip
//! reason. The harness hides a passing test's output otherwise.

#![expect(
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

#[path = "fixtures/gpu.rs"]
mod gpu;

use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu;
use gpu::{adapter_line, block_on, headless_device};
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::enriched_mesh::{
    EnrichedMesh, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::ids::ModelId;
use rs_cam_viz::render::mesh_render::{face_edge_vertices, face_mesh_gpu_data};
use rs_cam_viz::render::outline::OutlineUniforms;
use rs_cam_viz::render::{
    EdgeOffset, LineUniforms, LineVertex, LineWidths, MSAA_SAMPLES, MeshUniforms, PolygonGpuData,
    RenderResources, scene_sample_count,
};
use wgpu::util::DeviceExt;

/// The surface formats the app can be handed, in `egui_wgpu`'s own priority
/// order. See the module doc.
const APP_TARGET_FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Rgba8UnormSrgb,
];

/// The headless frame size: small, and not square, so a swapped width and
/// height cannot pass.
const FRAME_WIDTH: u32 = 64;
const FRAME_HEIGHT: u32 = 48;

/// A one-triangle STEP mesh with one face and its boundary loop.
fn one_face_mesh() -> EnrichedMesh {
    one_face_mesh_at(0.0)
}

/// The one-triangle mesh with its face at `z`.
fn one_face_mesh_at(z: f64) -> EnrichedMesh {
    let p = |x: f64, y: f64| P3::new(x, y, z);
    let ring = vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)];
    let face = FaceTessellation {
        vertices: ring.clone(),
        triangles: vec![[0, 1, 2]],
        surface_type: SurfaceType::Unknown,
        surface_params: SurfaceParams::Unknown,
        boundary_loops: vec![ring],
    };
    build_enriched_mesh(vec![face], Vec::new(), Vec::new())
        .unwrap_or_else(|e| panic!("the one-face mesh builds: {e}"))
}

/// Collects the errors that no error scope filters. Install it before the
/// first construction.
fn uncaptured_sink(device: &wgpu::Device) -> Arc<Mutex<Vec<String>>> {
    let uncaptured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&uncaptured);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        if let Ok(mut messages) = sink.lock() {
            messages.push(error.to_string());
        }
    }));
    uncaptured
}

fn assert_no_uncaptured(uncaptured: &Arc<Mutex<Vec<String>>>, what: &str) {
    let messages = uncaptured
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        messages.is_empty(),
        "uncaptured wgpu errors while {what}: {messages:?}"
    );
}

#[test]
fn every_render_pipeline_builds_on_a_headless_adapter_g_pipesmoke() {
    let gpu = match headless_device("g_pipesmoke_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — {reason}");
            return;
        }
    };
    eprintln!("G-PIPESMOKE adapter: {}", adapter_line(&gpu.adapter));
    let uncaptured = uncaptured_sink(&gpu.device);

    for &format in APP_TARGET_FORMATS {
        let samples = scene_sample_count(&gpu.adapter, format);
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = RenderResources::new(&gpu.device, format, samples);
        let error = block_on(scope.pop());
        assert!(
            error.is_none(),
            "{format:?}: wgpu rejected the viewport's render resources — {}",
            error.as_ref().map_or_else(String::new, |e| e.to_string())
        );
        drop(resources);
    }

    assert_no_uncaptured(&uncaptured, "building the viewport's render resources");
}

#[test]
fn the_scene_pass_renders_at_the_adapter_sample_count_g_pipesmoke() {
    let gpu = match headless_device("g_pipesmoke_frame_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — {reason}");
            return;
        }
    };
    eprintln!("G-PIPESMOKE adapter: {}", adapter_line(&gpu.adapter));
    let uncaptured = uncaptured_sink(&gpu.device);

    let x4 = |format: wgpu::TextureFormat| {
        gpu.adapter
            .get_texture_format_features(format)
            .flags
            .sample_count_supported(MSAA_SAMPLES)
    };
    let depth_x4 = x4(wgpu::TextureFormat::Depth32Float);

    for &format in APP_TARGET_FORMATS {
        let samples = scene_sample_count(&gpu.adapter, format);
        let expected = if x4(format) && depth_x4 {
            MSAA_SAMPLES
        } else {
            1
        };
        assert_eq!(
            samples, expected,
            "{format:?}: scene_sample_count disagrees with the adapter's X4 flags"
        );
        eprintln!("G-PIPESMOKE {format:?}: scene sample count {samples}");

        // Both paths: the adapter's count, then the single-sample fallback.
        // The count is stored once per `RenderResources`, so each path
        // needs its own instance.
        for count in [samples, 1] {
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut resources = RenderResources::new(&gpu.device, format, count);
            assert_eq!(resources.sample_count(), count);
            // One face mesh, so the frame draws the face pipeline with its
            // state bind group (D2), and its loop, so the frame draws the
            // model edges with the edge bind group (D4).
            let enriched = one_face_mesh();
            let face_mesh = face_mesh_gpu_data(
                &gpu.device,
                &resources.gpu_limits,
                &resources.face_state_bgl,
                &resources.face_palette_buffer,
                ModelId::from_wire(1),
                &enriched,
                &None,
            )
            .unwrap_or_else(|| panic!("the one-face mesh fits the device limits"));
            resources.face_mesh_data_list.push(face_mesh);
            let edges = face_edge_vertices(&enriched, &None);
            assert_eq!(edges.len(), 6, "a 3-point loop gives 3 closed pairs");
            resources.model_edge_lines = Some(PolygonGpuData {
                vertex_buffer: gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("g_pipesmoke_edges"),
                        contents: bytemuck::cast_slice(&edges),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                vertex_count: edges.len() as u32,
            });
            // The outline on, so the frame also runs the ID pass and the
            // outline draw (D §4) at this sample count.
            resources.render_headless_frame(
                &gpu.device,
                &gpu.queue,
                FRAME_WIDTH,
                FRAME_HEIGHT,
                true,
            );
            let error = block_on(scope.pop());
            assert!(
                error.is_none(),
                "{format:?} at {count} samples: wgpu rejected the headless scene frame — {}",
                error.as_ref().map_or_else(String::new, |e| e.to_string())
            );
            drop(resources);
        }
    }

    assert_no_uncaptured(&uncaptured, "rendering the headless scene frame");
}

/// The face Z of the pixel arm: the Z of the stepped block ring.
const PIXEL_FACE_Z: f32 = 20.0;

/// The pixel arm's view: an orthographic top view, column-major. World X
/// and Y map to NDC with a shift `(cx, cy)` that puts the edge `y = 0`
/// on the centre of row 36 and the edge `x = 0` on the centre of column
/// 20. Depth is `0.5 - z / 100`, so a higher Z is nearer.
fn pixel_view() -> [[f32; 4]; 4] {
    // Pixel centre of column c: x_ndc = (c + 0.5) * 2 / W - 1.
    let cx = (20.5 * 2.0 / FRAME_WIDTH as f32) - 1.0;
    // Pixel centre of row r (from the top): y_ndc = 1 - (r + 0.5) * 2 / H.
    let cy = 1.0 - (36.5 * 2.0 / FRAME_HEIGHT as f32);
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, -0.01, 0.0],
        [cx, cy, 0.5, 1.0],
    ]
}

/// The pixel `(column, row)` of world `(x, y)` in `pixel_view`.
fn pixel_of(x: f32, y: f32) -> (u32, u32) {
    let v = pixel_view();
    let x_ndc = x + v[3][0];
    let y_ndc = y + v[3][1];
    let col = (x_ndc + 1.0) * 0.5 * FRAME_WIDTH as f32 - 0.5;
    let row = (1.0 - y_ndc) * 0.5 * FRAME_HEIGHT as f32 - 0.5;
    (col.round() as u32, row.round() as u32)
}

/// One pixel of the last headless frame, RGBA8. `FRAME_WIDTH` × 4 is
/// 256 B, so a row needs no padding to `COPY_BYTES_PER_ROW_ALIGNMENT`.
fn read_pixel(
    gpu: &gpu::HeadlessGpu,
    resources: &RenderResources,
    (col, row): (u32, u32),
) -> [u8; 4] {
    let bytes_per_row = FRAME_WIDTH * 4;
    assert_eq!(bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    let texture = resources
        .headless_color_texture()
        .unwrap_or_else(|| panic!("a headless frame ran"));
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("g_pipesmoke_readback"),
        size: u64::from(bytes_per_row * FRAME_HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("g_pipesmoke_readback"),
        });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(FRAME_HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(Some(encoder.finish()));
    buffer.map_async(wgpu::MapMode::Read, .., |r| {
        r.unwrap_or_else(|e| panic!("the readback buffer maps: {e}"));
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap_or_else(|e| panic!("the device poll ends: {e}"));
    let view = buffer
        .get_mapped_range(..)
        .unwrap_or_else(|e| panic!("the mapped range reads: {e}"));
    let at = (row * bytes_per_row + col * 4) as usize;
    let mut px = [0_u8; 4];
    px.copy_from_slice(
        view.get(at..at + 4)
            .unwrap_or_else(|| panic!("the pixel {col},{row} lies in the frame")),
    );
    px
}

/// An RGB colour as the RGBA8 bytes `Rgba8Unorm` stores, ±1 for rounding.
fn same_colour(px: [u8; 4], rgb: [f32; 3]) -> bool {
    rgb.iter()
        .zip(px)
        .all(|(c, b)| (f32::from(b) - c * 255.0).abs() <= 1.0)
}

fn line_buffer(gpu: &gpu::HeadlessGpu, label: &str, verts: &[LineVertex]) -> PolygonGpuData {
    PolygonGpuData {
        vertex_buffer: gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        vertex_count: verts.len() as u32,
    }
}

#[test]
fn the_pick_highlight_wins_over_the_model_edge_g_pipesmoke() {
    let gpu = match headless_device("g_pipesmoke_pixel_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — {reason}");
            return;
        }
    };
    eprintln!("G-PIPESMOKE adapter: {}", adapter_line(&gpu.adapter));
    let uncaptured = uncaptured_sink(&gpu.device);

    // One sample and a linear format: one pixel is one colour.
    let mut resources = RenderResources::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
    let enriched = one_face_mesh_at(f64::from(PIXEL_FACE_Z));
    let face_mesh = face_mesh_gpu_data(
        &gpu.device,
        &resources.gpu_limits,
        &resources.face_state_bgl,
        &resources.face_palette_buffer,
        ModelId::from_wire(1),
        &enriched,
        &None,
    )
    .unwrap_or_else(|| panic!("the one-face mesh fits the device limits"));
    resources.face_mesh_data_list.push(face_mesh);
    let edges = face_edge_vertices(&enriched, &None);
    let edge_colour = edges
        .first()
        .map(|v| v.color)
        .unwrap_or_else(|| panic!("the loop gives edge vertices"));
    resources.model_edge_lines = Some(line_buffer(&gpu, "g_pipesmoke_edges", &edges));

    // The orthographic eye is a direction (w = 0): straight down the Z axis.
    let view_proj = pixel_view();
    let mesh = MeshUniforms {
        view_proj,
        light_dir: [0.0, 0.0, 1.0],
        _pad0: 0.0,
        camera_pos: [0.0, 0.0, 100.0],
        _pad1: 0.0,
    };
    let lines = LineUniforms::new(view_proj, [0.0, 0.0, 1.0, 0.0]);
    let offset = EdgeOffset { tol: 0.5, k: 0.0 };
    // One pixel per point, so a width token is that many pixels.
    let widths = LineWidths {
        viewport_px: [FRAME_WIDTH as f32, FRAME_HEIGHT as f32],
        pixels_per_point: 1.0,
    };
    resources.write_frame_uniforms(
        &gpu.queue,
        &mesh,
        lines,
        offset,
        widths,
        &OutlineUniforms::orthographic(50.0, 1.0, FRAME_HEIGHT as f32),
    );

    let inside = pixel_of(0.25, 0.25);
    let outside = pixel_of(0.9, 0.9);
    let on_edge = pixel_of(0.5, 0.0);
    assert_eq!(
        on_edge,
        (36, 36),
        "the probe sits mid-segment, on pixel centres"
    );

    // Frame 1: the face and its edges.
    resources.render_headless_frame(&gpu.device, &gpu.queue, FRAME_WIDTH, FRAME_HEIGHT, false);
    let face_px = read_pixel(&gpu, &resources, inside);
    let empty_px = read_pixel(&gpu, &resources, outside);
    let edge_px = read_pixel(&gpu, &resources, on_edge);
    eprintln!("G-PIPESMOKE pixels: face {face_px:?}, empty {empty_px:?}, edge {edge_px:?}");
    assert_ne!(
        face_px, empty_px,
        "the face draws: its pixel is not the background"
    );
    assert!(
        same_colour(edge_px, edge_colour),
        "the model edge wins over its own face: {edge_px:?} against {edge_colour:?}"
    );

    // Frame 2: the pick highlight on the same segment, in a colour that is
    // neither the face nor the edge.
    let magenta = [1.0, 0.0, 1.0];
    let highlight = [
        LineVertex {
            position: [0.0, 0.0, PIXEL_FACE_Z],
            color: magenta,
        },
        LineVertex {
            position: [1.0, 0.0, PIXEL_FACE_Z],
            color: magenta,
        },
    ];
    resources.selection_lines = Some(line_buffer(&gpu, "g_pipesmoke_highlight", &highlight));
    resources.render_headless_frame(&gpu.device, &gpu.queue, FRAME_WIDTH, FRAME_HEIGHT, false);
    let highlight_px = read_pixel(&gpu, &resources, on_edge);
    eprintln!("G-PIPESMOKE pixels: highlight {highlight_px:?}");
    assert!(
        same_colour(highlight_px, magenta),
        "the pick highlight wins over the model edge on a shared segment: {highlight_px:?}"
    );

    // The width arm: the highlight is a quad of `LINE_SELECTED_PX` (2.5 px
    // at one pixel per point). The column through the probe holds at least
    // 2 highlight pixels across the line.
    let (col, row) = on_edge;
    let across: Vec<[u8; 4]> = (row.saturating_sub(3)..=row + 3)
        .map(|r| read_pixel(&gpu, &resources, (col, r)))
        .collect();
    let covered = across
        .iter()
        .filter(|px| same_colour(**px, magenta))
        .count();
    eprintln!(
        "G-PIPESMOKE width: {covered} highlight pixels across at {} px",
        rs_cam_viz::ui::tokens::LINE_SELECTED_PX
    );
    assert!(
        covered >= 2,
        "a {} px line covers at least 2 pixels across: {across:?}",
        rs_cam_viz::ui::tokens::LINE_SELECTED_PX
    );

    assert_no_uncaptured(&uncaptured, "rendering the pixel frames");
}

#[test]
fn a_selected_face_gets_an_accent_outline_outside_only_g_pipesmoke() {
    let gpu = match headless_device("g_pipesmoke_outline_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — {reason}");
            return;
        }
    };
    eprintln!("G-PIPESMOKE adapter: {}", adapter_line(&gpu.adapter));
    let uncaptured = uncaptured_sink(&gpu.device);

    let mut resources = RenderResources::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
    let enriched = one_face_mesh_at(f64::from(PIXEL_FACE_Z));
    let face_mesh = face_mesh_gpu_data(
        &gpu.device,
        &resources.gpu_limits,
        &resources.face_state_bgl,
        &resources.face_palette_buffer,
        ModelId::from_wire(1),
        &enriched,
        &None,
    )
    .unwrap_or_else(|| panic!("the one-face mesh fits the device limits"));
    // Face 0 selected: bit 1 of its state word.
    gpu.queue
        .write_buffer(&face_mesh.face_state, 0, bytemuck::bytes_of(&2u32));
    resources.face_mesh_data_list.push(face_mesh);
    // No model edges and no lines: the outline is the only line drawn.

    let view_proj = pixel_view();
    let mesh = MeshUniforms {
        view_proj,
        light_dir: [0.0, 0.0, 1.0],
        _pad0: 0.0,
        camera_pos: [0.0, 0.0, 100.0],
        _pad1: 0.0,
    };
    resources.write_frame_uniforms(
        &gpu.queue,
        &mesh,
        LineUniforms::new(view_proj, [0.0, 0.0, 1.0, 0.0]),
        EdgeOffset { tol: 0.5, k: 0.0 },
        LineWidths {
            viewport_px: [FRAME_WIDTH as f32, FRAME_HEIGHT as f32],
            pixels_per_point: 1.0,
        },
        &OutlineUniforms::orthographic(50.0, 1.0, FRAME_HEIGHT as f32),
    );

    let inside = pixel_of(0.25, 0.25);
    resources.request_id_pixel(7, [inside.0, inside.1]);
    resources.render_headless_frame(&gpu.device, &gpu.queue, FRAME_WIDTH, FRAME_HEIGHT, true);
    let accent = rs_cam_viz::render::colors::OUTLINE_ACCENT;

    // The column through the middle of the edge y = 0 (row 36): the face
    // is above the edge (smaller rows), the background below.
    let (col, edge_row) = pixel_of(0.5, 0.0);
    let column: Vec<(u32, [u8; 4])> = (edge_row - 6..=edge_row + 6)
        .map(|r| (r, read_pixel(&gpu, &resources, (col, r))))
        .collect();
    eprintln!("G-PIPESMOKE outline column {col}: {column:?}");
    let accent_rows: Vec<u32> = column
        .iter()
        .filter(|(_, px)| same_colour(*px, accent))
        .map(|(r, _)| *r)
        .collect();
    assert!(
        !accent_rows.is_empty(),
        "the pixels just outside the selected face are accent: {column:?}"
    );
    assert!(
        accent_rows
            .iter()
            .all(|r| *r >= edge_row && *r <= edge_row + 2),
        "accent only just outside the silhouette (rows {edge_row}..={}): {accent_rows:?}",
        edge_row + 2
    );
    // Inside the face: no accent.
    for (x, y) in [(0.25, 0.25), (0.1, 0.1), (0.4, 0.3), (0.2, 0.6)] {
        let px = read_pixel(&gpu, &resources, pixel_of(x, y));
        assert!(
            !same_colour(px, accent),
            "no accent inside the selected face at ({x}, {y}): {px:?}"
        );
    }

    // The readback ring: the id under the inside pixel.
    let answer = resources.collect_id_readback(&gpu.device, true);
    let expected = (1u32 << 31) | (1u32 << 16);
    eprintln!("G-PIPESMOKE id readback: {answer:?}");
    assert_eq!(
        answer,
        Some((7, expected)),
        "selected, model slot 1, face 0"
    );

    assert_no_uncaptured(&uncaptured, "rendering the outline frame");
}

#[test]
fn a_face_away_from_the_light_is_not_black_g_pipesmoke() {
    let gpu = match headless_device("g_pipesmoke_light_device") {
        Ok(gpu) => gpu,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — {reason}");
            return;
        }
    };
    eprintln!("G-PIPESMOKE adapter: {}", adapter_line(&gpu.adapter));
    let uncaptured = uncaptured_sink(&gpu.device);

    let mut resources = RenderResources::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
    let enriched = one_face_mesh_at(f64::from(PIXEL_FACE_Z));
    let face_mesh = face_mesh_gpu_data(
        &gpu.device,
        &resources.gpu_limits,
        &resources.face_state_bgl,
        &resources.face_palette_buffer,
        ModelId::from_wire(1),
        &enriched,
        &None,
    )
    .unwrap_or_else(|| panic!("the one-face mesh fits the device limits"));
    resources.face_mesh_data_list.push(face_mesh);

    // The eye looks down from +Z; the light comes from below the face
    // (not exactly opposite the eye, so the half vector is defined).
    let view_proj = pixel_view();
    let mesh = MeshUniforms {
        view_proj,
        light_dir: [0.0, 0.6, -0.8],
        _pad0: 0.0,
        camera_pos: [0.0, 0.0, 100.0],
        _pad1: 0.0,
    };
    resources.write_frame_uniforms(
        &gpu.queue,
        &mesh,
        LineUniforms::new(view_proj, [0.0, 0.0, 1.0, 0.0]),
        EdgeOffset { tol: 0.5, k: 0.0 },
        LineWidths {
            viewport_px: [FRAME_WIDTH as f32, FRAME_HEIGHT as f32],
            pixels_per_point: 1.0,
        },
        &OutlineUniforms::orthographic(50.0, 1.0, FRAME_HEIGHT as f32),
    );
    resources.render_headless_frame(&gpu.device, &gpu.queue, FRAME_WIDTH, FRAME_HEIGHT, false);
    let px = read_pixel(&gpu, &resources, pixel_of(0.25, 0.25));
    let empty = read_pixel(&gpu, &resources, pixel_of(0.9, 0.9));
    eprintln!("G-PIPESMOKE away from the light: face {px:?}, empty {empty:?}");
    assert_ne!(px, empty, "the face draws");
    // The diffuse term is 0 (the light is behind the face); the specular
    // term at n·h = 0.32 is (0.32)^32, under one 8-bit step. So the pixel
    // is the face colour times the hemisphere sky (the face looks at +Z).
    let face = rs_cam_viz::render::colors::face_group_color(0).array();
    let sky = rs_cam_viz::render::colors::LIGHT_SKY.array();
    let expected = [face[0] * sky[0], face[1] * sky[1], face[2] * sky[2]];
    assert!(
        same_colour(px, expected),
        "the face takes the sky term: {px:?} against {expected:?}"
    );
    assert!(px[..3].iter().all(|&c| c > 0), "not black: {px:?}");

    assert_no_uncaptured(&uncaptured, "rendering the lighting frame");
}
