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

use egui_wgpu::wgpu;

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
