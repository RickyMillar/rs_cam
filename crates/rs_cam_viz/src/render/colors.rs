//! Centralized color constants for all rendering pipelines.
//!
//! Every hardcoded color in the render and UI modules should be defined here
//! so that color changes propagate from a single source.

// ── Toolpath palette ────────────────────────────────────────────────────

/// 8-color deterministic palette for per-toolpath coloring.
pub const TOOLPATH_PALETTE: [[f32; 3]; 8] = [
    [0.2, 0.5, 0.95],   // blue
    [0.2, 0.8, 0.3],    // green
    [0.95, 0.6, 0.15],  // orange
    [0.7, 0.3, 0.9],    // purple
    [0.9, 0.85, 0.2],   // yellow
    [0.2, 0.85, 0.85],  // cyan
    [0.95, 0.25, 0.25], // red
    [0.5, 0.9, 0.2],    // lime
];

/// Get the palette color for a toolpath at the given index.
#[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
pub fn palette_color(index: usize) -> [f32; 3] {
    TOOLPATH_PALETTE[index % TOOLPATH_PALETTE.len()]
}

// ── Tool assembly ───────────────────────────────────────────────────────

pub const TOOL_CUTTER: [f32; 3] = [0.8, 0.8, 0.3];
pub const TOOL_SHANK: [f32; 3] = [0.6, 0.6, 0.5];
pub const TOOL_HOLDER: [f32; 3] = [0.4, 0.4, 0.35];

// ── Stock & simulation ──────────────────────────────────────────────────

/// Stock wireframe outline.
pub const STOCK_OUTLINE: [f32; 3] = [0.4, 0.6, 0.8];

/// Solid stock face (warm wood tone).
pub const STOCK_SOLID_FACE: [f32; 3] = [0.65, 0.50, 0.30];

/// Collision point marker.
pub const COLLISION_POINT: [f32; 3] = [1.0, 0.0, 0.0];

// ── Polygon / 2D geometry ───────────────────────────────────────────────

/// DXF/SVG polygon outline.
pub const POLYGON_OUTLINE: [f32; 3] = [0.9, 0.75, 0.2];

// ── Grid & axes ─────────────────────────────────────────────────────────

/// The viewport grid.
///
/// Was `[0.25, 0.25, 0.28]`, a violet-leaning grey from the same accidental
/// family as the old viewport ground. Retuned onto the neutral ramp at
/// `INK_25` (`#31373E`), which is the hairline token — a grid IS a rule.
pub const GRID_BASE: [f32; 3] = [0.192, 0.216, 0.243];
pub const AXIS_X: [f32; 3] = [0.9, 0.2, 0.2];
pub const AXIS_Y: [f32; 3] = [0.2, 0.9, 0.2];
pub const AXIS_Z: [f32; 3] = [0.3, 0.4, 0.95];

// ── Height planes ───────────────────────────────────────────────────────

pub const HEIGHT_CLEARANCE: [f32; 3] = [0.3, 0.5, 0.9];
pub const HEIGHT_RETRACT: [f32; 3] = [0.3, 0.8, 0.8];
pub const HEIGHT_FEED: [f32; 3] = [0.3, 0.8, 0.3];
pub const HEIGHT_TOP: [f32; 3] = [0.9, 0.8, 0.2];
pub const HEIGHT_BOTTOM: [f32; 3] = [0.9, 0.3, 0.2];
