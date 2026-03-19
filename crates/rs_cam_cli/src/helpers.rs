//! Shared helper functions used by both CLI subcommands and job execution.

use anyhow::{bail, Context, Result};
use std::path::Path;

use rs_cam_core::{
    dressup::EntryStyle,
    polygon::Polygon2,
};

/// Parse an entry style string into an `EntryStyle`.
///
/// Returns `Ok(None)` for "plunge" (meaning no entry dressup needed),
/// `Ok(Some(style))` for "ramp" or "helix", and an error for anything else.
pub fn parse_entry_style(entry: &str) -> Result<Option<EntryStyle>> {
    match entry {
        "plunge" => Ok(None),
        "ramp" => Ok(Some(EntryStyle::Ramp { max_angle_deg: 3.0 })),
        "helix" => Ok(Some(EntryStyle::Helix { radius: 2.0, pitch: 1.0 })),
        _ => bail!("Unknown entry style '{}'. Supported: plunge, ramp, helix", entry),
    }
}

/// Load 2D polygons from an SVG or DXF file.
///
/// The file format is determined by extension. Returns an error if no closed
/// paths/entities are found or if the format is unsupported.
pub fn load_polygons(path: &Path) -> Result<Vec<Polygon2>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "svg" => {
            let polys = rs_cam_core::svg_input::load_svg(path, 0.1)
                .context("Failed to load SVG")?;
            if polys.is_empty() {
                bail!("No closed paths found in SVG file");
            }
            Ok(polys)
        }
        "dxf" => {
            let polys = rs_cam_core::dxf_input::load_dxf(path, 5.0)
                .context("Failed to load DXF")?;
            if polys.is_empty() {
                bail!("No closed entities found in DXF file");
            }
            Ok(polys)
        }
        _ => bail!("Unsupported input format '{}'. Supported: .svg, .dxf", ext),
    }
}
