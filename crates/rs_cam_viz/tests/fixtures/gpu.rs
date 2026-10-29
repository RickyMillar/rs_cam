//! The shared headless wgpu fixture for the device tests.
//!
//! A test binary pulls it in with `#[path = "fixtures/gpu.rs"] mod gpu;`.
//! Cargo builds a test target only for a `.rs` file directly under
//! `tests/`, so this file compiles into each consuming binary and is never
//! a target of its own.
//!
//! [`headless_device`] requests a software adapter first and any adapter
//! second, and never the noop backend: that backend validates nothing. It
//! gives `Err(reason)` when the machine has no adapter or no device. The
//! caller then prints the reason and returns, so the run is a real pass
//! with a visible reason.

#![allow(
    dead_code,
    reason = "each test binary that includes this module uses only part of it"
)]

use std::future::Future;

use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu;
use rs_cam_viz::render::{LineVertex, PolygonGpuData, RenderResources};
use wgpu::util::DeviceExt;

/// Poll ceiling for [`block_on`].
///
/// Every wgpu future these tests await is already resolved on a native
/// backend, so one poll is enough. The ceiling exists so a future that never
/// resolves fails the test instead of hanging the suite.
pub const MAX_POLLS: u32 = 100_000;

/// Drive a future to completion on the calling thread.
///
/// The tests add no executor dependency. `wgpu`'s native futures resolve on
/// the first poll, and `Waker::noop` covers the rest.
pub fn block_on<F: Future>(future: F) -> F::Output {
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
pub fn request_any_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    let mut refusals = Vec::new();
    for force_fallback_adapter in [true, false] {
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter,
            compatible_surface: None,
            // wgpu 30 added limit bucketing. It exists to stop untrusted
            // content fingerprinting the adapter, and this is a desktop
            // binary, so `false` is correct and it is also wgpu's own
            // default. The tests ask for the real adapter limits, exactly
            // as they did on wgpu 29.
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

/// A headless adapter, its device and its queue.
pub struct HeadlessGpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Request a headless adapter and a device with the adapter's own limits.
///
/// `label` names the device. `Err` carries the skip reason.
pub fn headless_device(label: &str) -> Result<HeadlessGpu, String> {
    // The noop backend accepts everything and validates nothing. Exclude it
    // so a device test can never pass on a device that cannot fail.
    let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_desc.backends = wgpu::Backends::all() - wgpu::Backends::NOOP;
    let instance = wgpu::Instance::new(instance_desc);

    let adapter = request_any_adapter(&instance)
        .map_err(|reason| format!("no wgpu adapter on this machine ({reason})"))?;
    let info = adapter.get_info();
    assert_ne!(
        info.backend,
        wgpu::Backend::Noop,
        "the noop backend validates nothing, so it cannot serve a device test"
    );

    let device_request = adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some(label),
        required_limits: adapter.limits(),
        ..Default::default()
    });
    let (device, queue) = block_on(device_request)
        .map_err(|err| format!("adapter {} gave no device ({err})", info.name))?;
    Ok(HeadlessGpu {
        adapter,
        device,
        queue,
    })
}

/// One line that names the adapter: name, backend, type and driver.
pub fn adapter_line(adapter: &wgpu::Adapter) -> String {
    let info = adapter.get_info();
    format!(
        "{} | backend {:?} | type {:?} | driver {} {}",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    )
}

// ── The pixel frame of the device tests ─────────────────────────────────

/// The headless frame size: small, and not square, so a swapped width and
/// height cannot pass.
pub const FRAME_WIDTH: u32 = 64;
pub const FRAME_HEIGHT: u32 = 48;

/// Collects the errors that no error scope filters. Install it before the
/// first construction.
pub fn uncaptured_sink(device: &wgpu::Device) -> Arc<Mutex<Vec<String>>> {
    let uncaptured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&uncaptured);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        if let Ok(mut messages) = sink.lock() {
            messages.push(error.to_string());
        }
    }));
    uncaptured
}

pub fn assert_no_uncaptured(uncaptured: &Arc<Mutex<Vec<String>>>, what: &str) {
    let messages = uncaptured
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        messages.is_empty(),
        "uncaptured wgpu errors while {what}: {messages:?}"
    );
}

/// The face Z of the pixel arm: the Z of the stepped block ring.
pub const PIXEL_FACE_Z: f32 = 20.0;

/// The pixel arm's view: an orthographic top view, column-major. World X
/// and Y map to NDC with a shift `(cx, cy)` that puts the edge `y = 0`
/// on the centre of row 36 and the edge `x = 0` on the centre of column
/// 20. Depth is `0.5 - z / 100`, so a higher Z is nearer.
pub fn pixel_view() -> [[f32; 4]; 4] {
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
pub fn pixel_of(x: f32, y: f32) -> (u32, u32) {
    let v = pixel_view();
    let x_ndc = x + v[3][0];
    let y_ndc = y + v[3][1];
    let col = (x_ndc + 1.0) * 0.5 * FRAME_WIDTH as f32 - 0.5;
    let row = (1.0 - y_ndc) * 0.5 * FRAME_HEIGHT as f32 - 0.5;
    (col.round() as u32, row.round() as u32)
}

/// One pixel of the last headless frame, RGBA8. `FRAME_WIDTH` × 4 is
/// 256 B, so a row needs no padding to `COPY_BYTES_PER_ROW_ALIGNMENT`.
pub fn read_pixel(
    gpu: &HeadlessGpu,
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
pub fn same_colour(px: [u8; 4], rgb: [f32; 3]) -> bool {
    rgb.iter()
        .zip(px)
        .all(|(c, b)| (f32::from(b) - c * 255.0).abs() <= 1.0)
}

pub fn line_buffer(gpu: &HeadlessGpu, label: &str, verts: &[LineVertex]) -> PolygonGpuData {
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
