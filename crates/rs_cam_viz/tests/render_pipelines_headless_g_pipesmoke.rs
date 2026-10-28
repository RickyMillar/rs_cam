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
//! - 7 render pipelines: `mesh_pipeline`, `face_mesh_pipeline` (D2),
//!   `sim_mesh_pipeline`, `height_plane_pipeline`, `colored_opaque_pipeline`,
//!   `line_pipeline`, `blit_pipeline`
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
use rs_cam_core::geometry::enriched_mesh::EnrichedMesh;
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_viz::render::mesh_render::face_mesh_gpu_data;
use rs_cam_viz::render::{MSAA_SAMPLES, RenderResources, scene_sample_count};

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

/// A one-triangle STEP mesh with one face.
fn one_face_mesh() -> EnrichedMesh {
    let p = |x: f64, y: f64| P3::new(x, y, 0.0);
    let mesh = TriangleMesh::from_raw(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)], vec![[0, 1, 2]]);
    EnrichedMesh {
        mesh: std::sync::Arc::new(mesh),
        face_groups: Vec::new(),
        triangle_to_face: vec![0],
        adjacency: Vec::new(),
        edges: Vec::new(),
    }
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
            // state bind group (D2).
            let face_mesh = face_mesh_gpu_data(
                &gpu.device,
                &resources.gpu_limits,
                &resources.face_state_bgl,
                &resources.face_palette_buffer,
                ModelId::from_wire(1),
                &one_face_mesh(),
                &None,
            )
            .unwrap_or_else(|| panic!("the one-face mesh fits the device limits"));
            resources.face_mesh_data_list.push(face_mesh);
            resources.render_headless_frame(&gpu.device, &gpu.queue, FRAME_WIDTH, FRAME_HEIGHT);
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
