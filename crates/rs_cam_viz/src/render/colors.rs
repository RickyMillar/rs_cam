//! Centralized color constants for all rendering pipelines.
//!
//! Every hardcoded color in the render and UI modules should be defined here
//! so that color changes propagate from a single source.

use crate::ui::tokens;

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
/// H2: a soft head part (a dust shoe brush). Fainter than [`TOOL_HOLDER`]:
/// it may touch the stock, so it is drawn lighter than a part that blocks.
pub const TOOL_HEAD_SOFT: [f32; 3] = [0.26, 0.26, 0.23];

// ── Stock & simulation ──────────────────────────────────────────────────

/// Stock wireframe outline.
pub const STOCK_OUTLINE: [f32; 3] = [0.4, 0.6, 0.8];

/// Solid stock face (warm wood tone).
pub const STOCK_SOLID_FACE: [f32; 3] = [0.65, 0.50, 0.30];

/// The simulated stock with no material colours (was a literal in
/// `app/gpu_upload.rs` `solid_sim_colors`).
pub const SIM_STOCK_WOOD: [f32; 3] = [0.65, 0.45, 0.25];

// ── Obstacles (K2) ──────────────────────────────────────────────────────
//
// The line pipeline has no alpha, so a "faint" line is a darker colour.

/// An obstacle's own geometry (the prism of its outline).
pub const OBSTACLE: [f32; 3] = [0.9, 0.7, 0.2];
/// The selected obstacle's geometry.
pub const OBSTACLE_SELECTED: [f32; 3] = [1.0, 0.9, 0.4];
/// The clearance ring: the footprint grown by the clearance, at the grown
/// top. A dim, desaturated form of [`OBSTACLE`].
pub const OBSTACLE_CLEARANCE: [f32; 3] = [0.5, 0.42, 0.24];
/// The obstacle plane: the Z the tool crosses an obstacle at.
pub const OBSTACLE_PLANE: [f32; 3] = [0.85, 0.5, 0.15];

// ── Polygon / 2D geometry ───────────────────────────────────────────────

/// DXF/SVG polygon outline.
pub const POLYGON_OUTLINE: [f32; 3] = [0.9, 0.75, 0.2];

/// The render colour of a UI token: each channel divided by 255, so a
/// panel chip and a viewport line of one state are one colour (geometry
/// selection P4.4).
pub const fn token_rgb(token: egui::Color32) -> [f32; 3] {
    [
        token.r() as f32 / 255.0,
        token.g() as f32 / 255.0,
        token.b() as f32 / 255.0,
    ]
}

// ── Selection states (geometry selection P4.4, lead ruling L-10) ────────
//
// The line pipeline has no alpha and no dash, so a faint state is a darker
// token. Each state is one token of `ui/tokens.rs`.

/// An item the op cuts.
pub const SELECTION_CUT: [f32; 3] = token_rgb(tokens::OK);
/// A closed ring that the op cuts round (a hole of a region).
pub const SELECTION_ISLAND: [f32; 3] = token_rgb(tokens::ACCENT);
/// An item the default rule matched and the op cannot cut.
pub const SELECTION_SKIPPED: [f32; 3] = token_rgb(tokens::CAUTION);
/// An item that an exclude removed; the upload adds an X at its anchor.
pub const SELECTION_EXCLUDED: [f32; 3] = token_rgb(tokens::INK_35);
/// An item that no rule and no pick matches.
pub const SELECTION_IDLE: [f32; 3] = token_rgb(tokens::INK_50);
/// A lost pick: a ghost ring or cross at the stored key position.
pub const SELECTION_LOST: [f32; 3] = token_rgb(tokens::DANGER);
/// The flash of the items that "Show me" frames, drawn last.
pub const SELECTION_HOVER: [f32; 3] = token_rgb(tokens::TEXT_STRONG);
/// An open path with no selection state.
pub const OPEN_PATH: [f32; 3] = token_rgb(tokens::PATH_OPEN);
/// The mark at the start of an open path (a triangle along the path).
pub const PATH_START: [f32; 3] = token_rgb(tokens::OK);
/// The mark at the end of an open path (a square).
pub const PATH_END: [f32; 3] = token_rgb(tokens::DANGER);
/// The boundary regions of the selected toolpath (`boundary_outline`).
pub const BOUNDARY_OUTLINE: [f32; 3] = token_rgb(tokens::TEXT_BODY);
/// The drawing of a model that no drawn toolpath reads (geometry selection
/// P4.D): it draws on import, in the active setup frame, with no state.
pub const DRAWING_NEUTRAL: [f32; 3] = token_rgb(tokens::TEXT_MUTED);

/// The line colour of one selection state (the table of P4.4).
pub fn selection_state_color(state: rs_cam_core::session::ItemState) -> [f32; 3] {
    use rs_cam_core::session::ItemState;
    match state {
        ItemState::Cut => SELECTION_CUT,
        ItemState::Island => SELECTION_ISLAND,
        ItemState::Skipped => SELECTION_SKIPPED,
        ItemState::Excluded => SELECTION_EXCLUDED,
        ItemState::NotSelected => SELECTION_IDLE,
    }
}

// ── Profile tabs (geometry selection P4.6) ──────────────────────────────

/// An automatic tab in the tab editor: a bar across the loop.
pub const TAB_AUTO: [f32; 3] = token_rgb(tokens::INK_65);
/// A hand tab in the tab editor: a bar across the loop and a handle ring.
pub const TAB_HAND: [f32; 3] = token_rgb(tokens::ACCENT);
/// The tab that the pointer drags (drawn at the snapped point).
pub const TAB_DRAGGED: [f32; 3] = token_rgb(tokens::TEXT_STRONG);

// ── Drill markers ───────────────────────────────────────────────────────

/// A drill hole that the selection holds (was a literal in
/// `app/gpu_upload.rs` until P4.4).
pub const HOLE_SELECTED: [f32; 3] = [0.2, 0.95, 0.4];
/// A drill hole that the selection does not hold.
pub const HOLE_FREE: [f32; 3] = [0.95, 0.55, 0.15];

// ── Setup marks ─────────────────────────────────────────────────────────

/// The datum crosshair (magenta; was a literal in `app/gpu_upload.rs`).
pub const DATUM_MARK: [f32; 3] = [0.9, 0.2, 0.9];

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
