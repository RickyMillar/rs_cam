//! GPU resource safety — buffer size guards to prevent device-limit crashes.

use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;

/// Cached GPU device limits queried once at startup.
pub struct GpuLimits {
    pub max_buffer_size: usize,
    pub max_texture_size: u32,
}

impl GpuLimits {
    /// Query limits from the device. Call once at initialization.
    pub fn from_device(device: &wgpu::Device) -> Self {
        let limits = device.limits();
        Self {
            max_buffer_size: limits.max_buffer_size as usize,
            max_texture_size: limits.max_texture_dimension_2d,
        }
    }
}

/// Create a GPU buffer with size validation. Returns `None` if the data
/// exceeds the device's maximum buffer size, logging a warning.
pub fn try_create_buffer(
    device: &wgpu::Device,
    limits: &GpuLimits,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> Option<wgpu::Buffer> {
    if contents.len() > limits.max_buffer_size {
        tracing::warn!(
            label,
            size_mb = contents.len() / (1024 * 1024),
            limit_mb = limits.max_buffer_size / (1024 * 1024),
            "GPU buffer exceeds device limit — skipping upload"
        );
        return None;
    }
    Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    }))
}
