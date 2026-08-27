//! Bit-exact identity of a cutter's *shape*, in the terms a drop-cutter walk
//! reads it — the shared key component for every tool-parameterised memo.
//!
//! # Why this is its own module
//!
//! [`crate::tier_map_cache`] introduced this key and argued it at length in its
//! own module doc. [`crate::finish_surface_cache`] needs exactly the same
//! question answered — *"is this the same cutter, as far as a drop cutter can
//! tell?"* — and a second copy of a memo key is the divergence class this repo
//! has already paid for more than once: two copies drift, and a memo key that
//! has drifted serves a stale answer **silently**. One definition, two
//! consumers.
//!
//! # Why these fields
//!
//! The scalars are the scales the drop cutter and the grid geometry read
//! directly: `envelope_radius_mm` sizes grid padding, `cusp_radius_mm` sizes
//! cusp-derived cells, and `diameter` / `length` / `corner_radius_mm` /
//! `flat_tip_diameter` shape the contact math. Every one is compared through
//! [`f64::to_bits`], so `-0.0` and `0.0` are distinct and `NaN` is an exact bit
//! pattern rather than a value that never equals itself.
//!
//! The [`ToolGeometryHint`] discriminant and its dials ride alongside because
//! **the scalars alone do not separate every shipped shape**: a Ø6 ball and the
//! shipped Ø1-tip/Ø6-shank taper both report `radius() == 3.0`, so a key that
//! read only the envelope would collide on them — which is exactly the
//! radius-semantics defect class this repo has been paying down.
//!
//! # Residual risk, stated
//!
//! A cutter whose drop-cutter behaviour is **not** a function of the hint plus
//! these six scalars would collide. No shipped shape is one:
//! [`ToolGeometryHint`] is a complete parameterisation of Flat / Ball / Bull /
//! VBit / TaperedBall. A new shape that is not must extend this key.
//! `crate::compute::sim_prefix::hash_tool` answers the same question one level
//! more conservatively, by *probing* the profile at 64 radii and the chip model
//! at 6 operating points; that is the model to follow if this key ever needs
//! hardening.

use crate::feeds::ToolGeometryHint;
use crate::tool::MillingCutter;

/// [`ToolGeometryHint`] reduced to something `Eq`: the discriminant plus its
/// dials as bit patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HintKey {
    Flat,
    Ball,
    Bull(u64),
    VBit(u64, u64),
    TaperedBall(u64, u64),
}

impl HintKey {
    pub(crate) fn new(hint: ToolGeometryHint) -> Self {
        match hint {
            ToolGeometryHint::Flat => Self::Flat,
            ToolGeometryHint::Ball => Self::Ball,
            ToolGeometryHint::Bull { corner_radius } => Self::Bull(corner_radius.to_bits()),
            ToolGeometryHint::VBit {
                included_angle,
                tip_diameter,
            } => Self::VBit(included_angle.to_bits(), tip_diameter.to_bits()),
            ToolGeometryHint::TaperedBall {
                tip_radius,
                taper_angle_deg,
            } => Self::TaperedBall(tip_radius.to_bits(), taper_angle_deg.to_bits()),
        }
    }
}

/// Bit-exact identity of a cutter's *shape*. See the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolShapeKey {
    diameter: u64,
    length: u64,
    corner_radius: u64,
    flat_tip_diameter: u64,
    cusp_radius: u64,
    envelope_radius: u64,
    hint: HintKey,
}

impl ToolShapeKey {
    pub(crate) fn new(tool: &dyn MillingCutter) -> Self {
        Self {
            diameter: tool.diameter().to_bits(),
            length: tool.length().to_bits(),
            corner_radius: tool.corner_radius_mm().to_bits(),
            flat_tip_diameter: tool.flat_tip_diameter().to_bits(),
            cusp_radius: tool.cusp_radius_mm().to_bits(),
            envelope_radius: tool.envelope_radius_mm().to_bits(),
            hint: HintKey::new(tool.geometry_hint()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::ToolShapeKey;
    use crate::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

    /// Moved here with the struct from `tier_map_cache::tests`; it is the
    /// reason the hint rides alongside the scalars.
    #[test]
    fn a_ball_and_a_taper_of_equal_envelope_have_different_shape_keys() {
        // Both report radius() == 3.0; only the hint and the cusp separate
        // them. A key that read the envelope alone would collide here.
        let ball = BallEndmill::new(6.0, 25.0);
        let taper = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        assert!((ball.envelope_radius_mm() - taper.envelope_radius_mm()).abs() < 1e-12);
        assert_ne!(ToolShapeKey::new(&ball), ToolShapeKey::new(&taper));
    }

    #[test]
    fn the_same_cutter_keys_equal() {
        let ball = BallEndmill::new(6.0, 25.0);
        assert_eq!(ToolShapeKey::new(&ball), ToolShapeKey::new(&ball));
    }

    #[test]
    fn a_diameter_change_is_a_distinct_key() {
        let a = BallEndmill::new(6.0, 25.0);
        let b = BallEndmill::new(2.0, 25.0);
        assert_ne!(ToolShapeKey::new(&a), ToolShapeKey::new(&b));
    }

    #[test]
    fn a_length_change_is_a_distinct_key() {
        // Length does not move the cusp or the envelope, so a key built from
        // radii alone would miss this.
        let a = BallEndmill::new(6.0, 25.0);
        let b = BallEndmill::new(6.0, 40.0);
        assert_ne!(ToolShapeKey::new(&a), ToolShapeKey::new(&b));
    }
}
