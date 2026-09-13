//! G-PIPESMOKE — every viewport render pipeline builds on a real wgpu device.
//!
//! Commit 241efd45 (G-LINEVIS) made the line shader's FRAGMENT stage read
//! `uniforms.dim`. The matching `line_bind_group_layout` entry in
//! `render/mod.rs` stayed VERTEX-only. wgpu validates a pipeline layout
//! against BOTH stages, so `create_render_pipeline('line_pipeline')` failed
//! and the P5.3 release crashed on launch. No test in this workspace created
//! a wgpu device, so no gate could see it.
//!
//! This sentry adds that gate. It requests a headless adapter: a software
//! adapter first, any adapter second. It then calls
//! `RenderResources::new(&device, format)`. That function is the app's ONE
//! construction site for the viewport's GPU state, and it builds:
//!
//! - 4 shader modules: `mesh_shader`, `sim_mesh_shader`, `line_shader`,
//!   `blit_shader`
//! - 3 bind group layouts: `mesh_bgl`, `line_bgl`, `blit_bgl`
//! - 6 render pipelines: `mesh_pipeline`, `sim_mesh_pipeline`,
//!   `height_plane_pipeline`, `colored_opaque_pipeline`, `line_pipeline`,
//!   `blit_pipeline`
//!
//! `app.rs` calls the same function with `render_state.target_format`, and
//! nothing else in the crate creates a pipeline, a layout, or a shader
//! module. So the sentry's subject is the whole surface.
//!
//! Every pipeline uses `MultisampleState::default()` (sample count 1), so the
//! sample count is not a parameter of the seam and the test does not vary it.
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
//! - `blit_bg` is built per frame in the private `ensure_offscreen`, not in
//!   `new`. It is the same defect class — a resource against a layout — but
//!   it is outside this sentry's subject.
//! - A uniform buffer that is SMALLER than its WGSL struct still passes. All
//!   three layouts set `min_binding_size: None`, so wgpu defers that check to
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

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::future::Future;
use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu;
use rs_cam_viz::render::RenderResources;

/// The surface formats the app can be handed, in `egui_wgpu`'s own priority
/// order. See the module doc.
const APP_TARGET_FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Rgba8UnormSrgb,
];

/// Poll ceiling for [`block_on`].
///
/// Every wgpu future this test awaits is already resolved on a native
/// backend, so one poll is enough. The ceiling exists so a future that never
/// resolves fails the test instead of hanging the suite.
const MAX_POLLS: u32 = 100_000;

/// Drive a future to completion on the calling thread.
///
/// The test adds no executor dependency. `wgpu`'s native futures resolve on
/// the first poll, and `Waker::noop` covers the rest.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    for _ in 0..MAX_POLLS {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
    panic!("a wgpu future did not resolve after {MAX_POLLS} polls");
}

/// Ask for a software adapter first, then for any adapter.
///
/// The error collects both refusals, so a skip says which backends were
/// tried and why each declined.
fn request_any_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    let mut refusals = Vec::new();
    for force_fallback_adapter in [true, false] {
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter,
            compatible_surface: None,
            // wgpu 30 added limit bucketing. It exists to stop untrusted
            // content fingerprinting the adapter, and this is a desktop
            // binary, so `false` is correct and it is also wgpu's own
            // default. The smoke test asks for the real adapter limits,
            // exactly as it did on wgpu 29.
            apply_limit_buckets: false,
        };
        match block_on(instance.request_adapter(&options)) {
            Ok(adapter) => return Ok(adapter),
            Err(err) => {
                refusals.push(format!(
                    "force_fallback_adapter={force_fallback_adapter}: {err}"
                ));
            }
        }
    }
    Err(refusals.join("; "))
}

#[test]
fn every_render_pipeline_builds_on_a_headless_adapter_g_pipesmoke() {
    // The noop backend accepts everything and validates nothing. Exclude it
    // so this sentry can never pass on a device that cannot fail.
    let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_desc.backends = wgpu::Backends::all() - wgpu::Backends::NOOP;
    let instance = wgpu::Instance::new(instance_desc);

    let adapter = match request_any_adapter(&instance) {
        Ok(adapter) => adapter,
        Err(reason) => {
            eprintln!("G-PIPESMOKE SKIPPED — no wgpu adapter on this machine ({reason})");
            return;
        }
    };

    let info = adapter.get_info();
    eprintln!(
        "G-PIPESMOKE adapter: {} | backend {:?} | type {:?} | driver {} {}",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    assert_ne!(
        info.backend,
        wgpu::Backend::Noop,
        "the noop backend validates nothing, so it cannot serve this sentry"
    );

    let device_request = adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("g_pipesmoke_device"),
        required_limits: adapter.limits(),
        ..Default::default()
    });
    let (device, _queue) = match block_on(device_request) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!(
                "G-PIPESMOKE SKIPPED — adapter {} gave no device ({err})",
                info.name
            );
            return;
        }
    };

    // The uncaptured handler catches anything the validation scope does not
    // filter. Install it before the first construction.
    let uncaptured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&uncaptured);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        if let Ok(mut messages) = sink.lock() {
            messages.push(error.to_string());
        }
    }));

    for &format in APP_TARGET_FORMATS {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = RenderResources::new(&device, format);
        let error = block_on(scope.pop());
        assert!(
            error.is_none(),
            "{format:?}: wgpu rejected the viewport's render resources — {}",
            error.as_ref().map_or_else(String::new, |e| e.to_string())
        );
        drop(resources);
    }

    let messages = uncaptured
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        messages.is_empty(),
        "uncaptured wgpu errors while building the viewport's render resources: {messages:?}"
    );
}
