//! The original theme surface, now a thin re-export over [`crate::ui::tokens`].
//!
//! Every name this module exported before UP1 still exists and still means
//! what it meant, so no call site changed when the token module arrived. The
//! VALUES moved onto `DESIGN_SPEC.md` §2, which is the point of the package:
//! one assignment here retunes several hundred call sites.
//!
//! **Do not add a name here.** New work names `tokens::` directly. This
//! module exists to keep the old call sites compiling while UP2 to UP8
//! migrate them, and it shrinks as they do.

use egui::Color32;

use super::tokens;

// --- Semantic status colors ---
//
// The five roles in `DESIGN_SPEC.md` §2.6 replace the eight names below.
// `WARNING`, `WARNING_MILD` and `WARNING_TEXT` were three tones of one
// intent; they now all read CAUTION. `ERROR` / `ERROR_MILD` and
// `SUCCESS` / `SUCCESS_BRIGHT` collapse the same way.
//
// `theme::ERROR` was `(220, 80, 80)` and read **4.19** against the panel
// fill, below the normal-text floor, while colouring the `ERR` chip and
// every collision string. `DANGER` clears 4.5 on all four surfaces.

pub const WARNING: Color32 = tokens::CAUTION;
pub const WARNING_MILD: Color32 = tokens::CAUTION;
pub const WARNING_TEXT: Color32 = tokens::CAUTION;

pub const ERROR: Color32 = tokens::DANGER;
pub const ERROR_MILD: Color32 = tokens::DANGER;

pub const SUCCESS: Color32 = tokens::OK;
pub const SUCCESS_BRIGHT: Color32 = tokens::OK;

pub const INFO: Color32 = tokens::INFO;

/// Not measured. New in UP1, and the reason §2.6 has five roles rather than
/// four: an abstaining gate and a passing gate must not look alike.
pub const UNKNOWN: Color32 = tokens::UNKNOWN;

// --- Text hierarchy ---
//
// `TEXT_DIM` and `TEXT_FAINT` were two names one step apart and both are now
// `INK_50`, per §2.4. The old `TEXT_FAINT` `(100, 100, 115)` read **2.85**
// against the panel fill and was used at 9 points.

pub const TEXT_HEADING: Color32 = tokens::TEXT_STRONG;
pub const TEXT_STRONG: Color32 = tokens::TEXT_STRONG;
pub const TEXT_MUTED: Color32 = tokens::TEXT_MUTED;
pub const TEXT_DIM: Color32 = tokens::TEXT_FAINT;
pub const TEXT_FAINT: Color32 = tokens::TEXT_FAINT;

// --- Accent ---
pub const ACCENT: Color32 = tokens::ACCENT;

// --- Card / frame ---
pub const CARD_FILL: Color32 = tokens::SURFACE_RAISED;
pub const CARD_FILL_SELECTED: Color32 = tokens::ACCENT_QUIET;

// --- Lane / status ---
pub const LANE_IDLE: Color32 = tokens::LANE_IDLE;
pub const LANE_QUEUED: Color32 = tokens::LANE_QUEUED;
pub const LANE_RUNNING: Color32 = tokens::LANE_RUNNING;
pub const LANE_CANCELLING: Color32 = tokens::LANE_CANCELLING;

// The inline "results are stale" cue now lives in the component layer as
// `ui::components::FreshnessGate::banner` (W0.5 → CL); the old
// `theme::stale_banner` free function was superseded and removed.

/// Standard card frame for list items and info panels.
///
/// UP2: one implementation, in `components::Card`. The five call sites keep
/// this signature and gain the treatment without being edited.
pub fn card_frame(selected: bool) -> egui::Frame {
    super::components::Card::new().selected(selected).frame()
}
