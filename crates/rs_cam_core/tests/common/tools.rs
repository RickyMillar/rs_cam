//! Cutter fixtures — both the geometric `MillingCutter` shapes the generators
//! take directly and the [`ToolConfig`] records a `ProjectSession` needs.
//!
//! These dimensions are not arbitrary. The Ø1-tip / 7° / Ø6-shank taper is
//! *the tool this project finishes with*, and its 6× envelope-vs-cusp split
//! (`envelope_radius_mm() == 3.0`, `cusp_radius_mm() == 0.5`) is what makes
//! every radius-semantics sentry discriminating. Ø3 ball is the control where
//! the two radii collapse to one number, so a fix that is supposed to be an
//! identity transformation can be shown to be one.
//!
//! Reference consumers: `checkpoint_a_valley_matrix.rs` (`wanaka_taper`,
//! `ball_control`), `finish_resolution_policy_pr3.rs` (`wanaka_taper`),
//! `standing_material_channel_am9.rs` (the `ToolConfig` builders).

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::tool::{BallEndmill, TaperedBallEndmill};

/// Default stickout for the geometric cutters (mm). Long enough that reach
/// guards are never the binding constraint on the small fixtures.
pub const DEFAULT_STICKOUT_MM: f64 = 25.0;

// ── Geometric cutters (`MillingCutter`) ─────────────────────────────────

/// The project's finishing tool: Ø1 tip, 7° half-angle, Ø6 shaft.
/// Envelope 3.0 mm, cusp 0.5 mm — a 6× split, so any drift between the two
/// scales is visible.
pub fn wanaka_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, DEFAULT_STICKOUT_MM)
}

/// A second taper angle, to show angle-dependent boundaries actually move
/// with α rather than being fixtures of the one shipped tool.
pub fn steep_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 15.0, 6.0, DEFAULT_STICKOUT_MM)
}

/// Ball control: `cusp_radius_mm() == envelope_radius_mm()`, so anything
/// tool-scaled collapses onto one value.
pub fn ball_control() -> BallEndmill {
    BallEndmill::new(3.0, DEFAULT_STICKOUT_MM)
}

/// Ball of an arbitrary diameter at the shared stickout.
pub fn ball_cutter(diameter_mm: f64) -> BallEndmill {
    BallEndmill::new(diameter_mm, DEFAULT_STICKOUT_MM)
}

/// Taper with all three shape dials explicit, for the cases that need an
/// angle or shaft other than the shipped one.
pub fn tapered_cutter(
    tip_diameter_mm: f64,
    taper_half_angle_deg: f64,
    shaft_diameter_mm: f64,
) -> TaperedBallEndmill {
    TaperedBallEndmill::new(
        tip_diameter_mm,
        taper_half_angle_deg,
        shaft_diameter_mm,
        DEFAULT_STICKOUT_MM,
    )
}

// ── Session-level tool records (`ToolConfig`) ───────────────────────────

/// Ball-nose `ToolConfig` at `ToolId(0)`, everything but the diameter left at
/// `new_default`.
pub fn ball_tool_config(diameter_mm: f64) -> ToolConfig {
    ToolConfig {
        diameter: diameter_mm,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

/// Tapered-ball `ToolConfig` — the class where `radius()` (envelope) and
/// `cusp_radius()` (tip sphere) diverge, which is why so many sentries pair
/// it against [`ball_tool_config`].
pub fn tapered_ball_tool_config(
    tip_diameter_mm: f64,
    taper_half_angle_deg: f64,
    shaft_diameter_mm: f64,
) -> ToolConfig {
    ToolConfig {
        diameter: tip_diameter_mm,
        taper_half_angle: taper_half_angle_deg,
        shaft_diameter: shaft_diameter_mm,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

/// Flat end mill `ToolConfig` at `ToolId(0)`.
///
/// Distinct from `common::make_endmill_6mm`, which pins a full set of
/// shank/stickout/flute dimensions the F-### sentries depend on; this one
/// changes nothing but the diameter.
pub fn endmill_tool_config(diameter_mm: f64) -> ToolConfig {
    ToolConfig {
        diameter: diameter_mm,
        ..ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }
}
