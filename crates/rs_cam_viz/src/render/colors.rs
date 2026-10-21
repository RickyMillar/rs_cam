//! Centralized color constants for all rendering pipelines.
//!
//! Every colour a render pipeline draws is defined here, so that a colour
//! change propagates from a single source (`render/CLAUDE.md`). A colour that
//! only `render/` reads is an [`Rgb`]: only this module can make one. A
//! colour that is derived from another (a shade, a ramp, a blend) is a
//! function here. `panels_read_the_token_module_up1` scans the pipelines for
//! a literal colour (F173).

use crate::ui::tokens;

/// One RGB colour of a viewport pipeline, each channel in `0..=1`.
///
/// The field is private and [`Rgb::new`] is private to this module, so a
/// pipeline cannot make a colour of its own; it names one of the constants
/// below. A vertex takes the channels through [`Rgb::array`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb([f32; 3]);

impl Rgb {
    const fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }

    /// The channels, for a vertex buffer.
    #[must_use]
    pub const fn array(self) -> [f32; 3] {
        self.0
    }
}

impl From<Rgb> for [f32; 3] {
    fn from(c: Rgb) -> Self {
        c.0
    }
}

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
pub const TOOL_SHANK: Rgb = Rgb::new(0.6, 0.6, 0.5);
pub const TOOL_HOLDER: Rgb = Rgb::new(0.4, 0.4, 0.35);
/// H2: a soft head part (a dust shoe brush). Fainter than [`TOOL_HOLDER`]:
/// it may touch the stock, so it is drawn lighter than a part that blocks.
pub const TOOL_HEAD_SOFT: Rgb = Rgb::new(0.26, 0.26, 0.23);

// ── Stock & simulation ──────────────────────────────────────────────────

/// Stock wireframe outline.
pub const STOCK_OUTLINE: Rgb = Rgb::new(0.4, 0.6, 0.8);

/// Solid stock face (warm wood tone).
pub const STOCK_SOLID_FACE: Rgb = Rgb::new(0.65, 0.50, 0.30);

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
pub const GRID_BASE: Rgb = Rgb::new(0.192, 0.216, 0.243);
pub const AXIS_X: Rgb = Rgb::new(0.9, 0.2, 0.2);
pub const AXIS_Y: Rgb = Rgb::new(0.2, 0.9, 0.2);
pub const AXIS_Z: Rgb = Rgb::new(0.3, 0.4, 0.95);

// ── Height planes ───────────────────────────────────────────────────────

pub const HEIGHT_CLEARANCE: [f32; 3] = [0.3, 0.5, 0.9];
pub const HEIGHT_RETRACT: [f32; 3] = [0.3, 0.8, 0.8];
pub const HEIGHT_FEED: [f32; 3] = [0.3, 0.8, 0.3];
pub const HEIGHT_TOP: [f32; 3] = [0.9, 0.8, 0.2];
pub const HEIGHT_BOTTOM: [f32; 3] = [0.9, 0.3, 0.2];

// ── Toolpath lines (F173: moved from `toolpath_render.rs`, same values) ──

/// An entry move (ramp, helix, plunge) on a toolpath line.
pub const SPAN_ENTRY: Rgb = Rgb::new(0.20, 0.95, 0.95);
/// A lead-out move.
pub const SPAN_LEAD_OUT: Rgb = Rgb::new(0.95, 0.30, 0.85);
/// A link bridge (drawn dashed).
pub const SPAN_LINK_BRIDGE: Rgb = Rgb::new(0.45, 0.45, 0.55);
/// The rapids of the feed-rate and the chipload line views.
pub const RAPID_DIM: Rgb = Rgb::new(0.15, 0.15, 0.2);
/// The schematic entry preview (bright cyan).
pub const ENTRY_PREVIEW: Rgb = Rgb::new(0.2, 0.9, 0.9);
/// The tool-profile ghost overlay (pale white).
pub const TOOL_PROFILE: Rgb = Rgb::new(0.85, 0.85, 1.0);

/// A selected toolpath draws 30 % brighter, each channel capped at 1.
#[must_use]
pub fn brighten_selected(mut c: [f32; 3], selected: bool) -> [f32; 3] {
    if selected {
        c[0] = (c[0] * 1.3).min(1.0);
        c[1] = (c[1] * 1.3).min(1.0);
        c[2] = (c[2] * 1.3).min(1.0);
    }
    c
}

/// The palette colour shaded by depth and pass: `t` is the move Z in the
/// toolpath Z range (`0` at the bottom), darker at the bottom, with a small
/// lightness step per pass so passes stratify.
#[must_use]
pub fn palette_depth_shade(base: [f32; 3], t: f32, pass_index: Option<u32>) -> [f32; 3] {
    let depth_factor = 0.7 + t * 0.3; // darker at bottom, brighter at top
    let pass_shift = match pass_index {
        Some(p) => 1.0 + ((p as f32 % 4.0) - 1.5) * 0.06, // ±~9% spread across 4 passes
        None => 1.0,
    };
    let f = depth_factor * pass_shift;
    [
        (base[0] * f).min(1.0),
        (base[1] * f).min(1.0),
        (base[2] * f).min(1.0),
    ]
}

/// A rapid of a palette toolpath: the palette colour at 35 %.
#[must_use]
pub fn palette_rapid(base: [f32; 3]) -> [f32; 3] {
    [base[0] * 0.35, base[1] * 0.35, base[2] * 0.35]
}

/// A dressup move of a palette toolpath: halfway to mid grey.
#[must_use]
pub fn palette_dressup(base: [f32; 3]) -> [f32; 3] {
    [
        0.5 * (base[0] + 0.5),
        0.5 * (base[1] + 0.5),
        0.5 * (base[2] + 0.5),
    ]
}

// ── Chipload band (the advance-per-tooth line view) ─────────────────────

/// No vendor band to compare against.
pub const CHIPLOAD_NO_BAND: Rgb = Rgb::new(0.40, 0.40, 0.40);
/// No reading for the move.
pub const CHIPLOAD_NO_READING: Rgb = Rgb::new(0.25, 0.25, 0.30);
/// Under-engaged: rubbing risk.
pub const CHIPLOAD_BELOW_BAND: Rgb = Rgb::new(0.20, 0.40, 0.90);
/// Inside the band.
pub const CHIPLOAD_WITHIN: Rgb = Rgb::new(0.20, 0.85, 0.30);
/// Near the band ceiling.
pub const CHIPLOAD_NEAR_CEILING: Rgb = Rgb::new(1.00, 0.60, 0.10);
/// Above the band.
pub const CHIPLOAD_ABOVE_BAND: Rgb = Rgb::new(0.95, 0.20, 0.20);

/// Just above the floor: the blend blue → green at `t` in `0..=1`.
#[must_use]
pub fn chipload_floor_blend(t: f32) -> [f32; 3] {
    [
        0.20 + (0.00 - 0.20) * t,
        0.40 + (0.85 - 0.40) * t,
        0.90 + (0.30 - 0.90) * t,
    ]
}

// ── Engagement (the feed-rate line view) ────────────────────────────────

/// `ratio` = feed / nominal, clamped to `0..=1.5`: 1.0 and up is green,
/// 0.5 is yellow, 0.0 is red.
#[must_use]
pub fn engagement_ramp(ratio: f32) -> [f32; 3] {
    if ratio >= 1.0 {
        // At or above nominal: green (light engagement / air cut)
        [0.2, 0.8, 0.3]
    } else if ratio >= 0.5 {
        // 50-100% of nominal: green → yellow
        let t = (ratio - 0.5) * 2.0; // 0..1
        [
            0.2 + (1.0 - t) * 0.7,
            0.8 - (1.0 - t) * 0.1,
            0.3 - (1.0 - t) * 0.2,
        ]
    } else {
        // Below 50%: yellow → red (heavy engagement)
        let t = ratio * 2.0; // 0..1
        [0.9 - (1.0 - t) * 0.1, 0.7 * t, 0.1 * t]
    }
}

// ── Stock deviation (the simulation deviation view) ─────────────────────

/// The colour of one deviation `d = sim_z - model_z` (mm): blue where
/// material remains, green on target, yellow then red for an overcut.
#[must_use]
pub fn deviation_color(d: f32) -> [f32; 3] {
    if d > 0.1 {
        // Material remaining — blue, intensity by amount
        let t = ((d - 0.1) / 1.0).min(1.0); // 0..1 over 0.1..1.1 mm
        [0.0, 0.3 * (1.0 - t), 0.5 + 0.5 * t] // darker blue as more remaining
    } else if d < -0.5 {
        // Major overcut — red
        let t = ((-d - 0.5) / 1.0).min(1.0);
        [0.6 + 0.4 * t, 0.0, 0.0]
    } else if d < -0.1 {
        // Slight overcut — yellow, blending toward red
        let t = ((-d - 0.1) / 0.4).min(1.0); // 0..1 over -0.1..-0.5
        [0.8 + 0.2 * t, 0.8 * (1.0 - t), 0.0]
    } else {
        // On target — green
        [0.1, 0.75, 0.1]
    }
}

// ── Model faces (the enriched-mesh pipeline) ────────────────────────────

/// A selected face group (bright blue).
pub const FACE_SELECTED: Rgb = Rgb::new(0.3, 0.5, 1.0);
/// The hovered face group (soft cyan).
pub const FACE_HOVER: Rgb = Rgb::new(0.4, 0.7, 0.85);

/// The pastel palette a face group takes by its id.
const FACE_GROUP_PALETTE: [Rgb; 10] = [
    Rgb::new(0.75, 0.85, 0.90), // light blue
    Rgb::new(0.85, 0.75, 0.90), // light purple
    Rgb::new(0.75, 0.90, 0.80), // light green
    Rgb::new(0.90, 0.85, 0.75), // light orange
    Rgb::new(0.90, 0.78, 0.80), // light pink
    Rgb::new(0.80, 0.90, 0.88), // light teal
    Rgb::new(0.88, 0.82, 0.75), // light tan
    Rgb::new(0.78, 0.82, 0.92), // periwinkle
    Rgb::new(0.88, 0.90, 0.75), // light yellow-green
    Rgb::new(0.82, 0.75, 0.85), // light mauve
];

/// The deterministic pastel colour of face group `index`.
#[must_use]
// SAFETY: modulo indexing into a constant-length palette cannot go out of bounds.
#[allow(clippy::indexing_slicing)]
pub fn face_group_color(index: usize) -> Rgb {
    FACE_GROUP_PALETTE[index % FACE_GROUP_PALETTE.len()]
}
