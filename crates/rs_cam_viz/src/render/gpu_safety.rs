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
    Some(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage,
        }),
    )
}

/// The bytes of the scene targets at `width` × `height`.
///
/// The sum is the multisampled colour, the depth at `samples`, and the
/// single-sample resolve colour. At `samples` = 1 no multisampled colour
/// exists. wgpu gives no VRAM query, so this value is an instrument for the
/// log, not a gate. The hard bound stays the `max_texture_size` clamp.
pub fn offscreen_bytes(width: u32, height: u32, samples: u32, format: wgpu::TextureFormat) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    let colour = u64::from(format.block_copy_size(None).unwrap_or(4));
    // Depth32Float: 4 bytes per sample.
    let depth = 4;
    let msaa_colour = if samples > 1 {
        pixels * colour * u64::from(samples)
    } else {
        0
    };
    msaa_colour + pixels * depth * u64::from(samples) + pixels * colour
}

/// Logs the scene target bytes at `debug` level. `ensure_offscreen` calls
/// it each time it builds the targets.
pub fn log_offscreen_budget(width: u32, height: u32, samples: u32, format: wgpu::TextureFormat) {
    tracing::debug!(
        width,
        height,
        samples,
        mib = offscreen_bytes(width, height, samples, format) / (1 << 20),
        "offscreen scene targets"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: f64 = (1u64 << 20) as f64;

    /// The MiB value to one decimal place, as `render/CLAUDE.md` and the
    /// design table write it.
    fn mib_tenths(bytes: u64) -> f64 {
        (bytes as f64 / MIB * 10.0).round() / 10.0
    }

    #[test]
    fn offscreen_bytes_give_the_design_table_rows() {
        let format = wgpu::TextureFormat::Bgra8Unorm;
        // 1920 × 1080 × 4 B = 8 294 400 B for each single-sample target.
        assert_eq!(offscreen_bytes(1920, 1080, 1, format), 16_588_800);
        assert_eq!(offscreen_bytes(1920, 1080, 4, format), 74_649_600);
        assert_eq!(offscreen_bytes(3840, 2160, 4, format), 298_598_400);
        assert!((mib_tenths(offscreen_bytes(1920, 1080, 4, format)) - 71.2).abs() < 1e-9);
        assert!((mib_tenths(offscreen_bytes(3840, 2160, 4, format)) - 284.8).abs() < 1e-9);
    }
}
