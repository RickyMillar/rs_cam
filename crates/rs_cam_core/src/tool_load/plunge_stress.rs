//! Plunge-stress gate: warns when configured plunge rate would damage
//! the cutter tip on a ball / tapered-ball tool.
//!
//! The cap formula matches `feeds/mod.rs` Fix 2 (Wanaka audit):
//! `150 mm/min × effective tip diameter (mm)` for ball and tapered-ball
//! geometries; no cap for flat / bull / V-bit (those plunge at the
//! material plunge_rate floor without flute-tip risk).
//!
//! The LUT cap protects fresh toolpaths, but pre-Fix-2 projects carry
//! static-default plunge rates. This module surfaces those mismatches at
//! diagnostics time. See `planning/P2_PLUNGE_STRESS_GATE_RCA.md`.

use crate::feeds::ToolGeometryHint;
use serde::{Deserialize, Serialize};

/// Cap in mm/min for an effective tip diameter (mm).
///
/// Calibrated from FSWizard / GWizard published plunge ranges for small
/// ball / tapered-ball in wood (100–300 mm/min for sub-2 mm tools).
const PLUNGE_CAP_PER_MM_TIP_DIAMETER: f64 = 150.0;

/// Smallest tapered-ball tip diameter we'll treat as a real cutter.
/// Matches the `(tip_radius * 2).max(0.5)` floor in `feeds/mod.rs`.
const MIN_TAPERED_TIP_DIAMETER_MM: f64 = 0.5;

/// Maximum safe plunge rate for the given tool geometry. Returns
/// `None` when the geometry has no flute-tip plunge risk (flat, bull,
/// V-bit). Mirrors the Fix 2 cap in `feeds::compute_feeds`.
pub fn safe_plunge_cap_mm_min(geometry: ToolGeometryHint, diameter_mm: f64) -> Option<f64> {
    let tip_d = match geometry {
        ToolGeometryHint::Ball => diameter_mm,
        ToolGeometryHint::TaperedBall { tip_radius, .. } => {
            (tip_radius * 2.0).max(MIN_TAPERED_TIP_DIAMETER_MM)
        }
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. } => {
            return None;
        }
    };
    Some(PLUNGE_CAP_PER_MM_TIP_DIAMETER * tip_d)
}

/// A plunge-stress warning surfaced at diagnostics time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlungeStressWarning {
    pub plunge_rate_mm_min: f64,
    pub safe_cap_mm_min: f64,
    pub effective_tip_diameter_mm: f64,
}

/// Returns `Some(warning)` when `plunge_rate` exceeds the safe cap for
/// this geometry, `None` otherwise (including for non-ball geometries
/// which have no cap).
pub fn check_plunge_stress(
    geometry: ToolGeometryHint,
    diameter_mm: f64,
    plunge_rate_mm_min: f64,
) -> Option<PlungeStressWarning> {
    let cap = safe_plunge_cap_mm_min(geometry, diameter_mm)?;
    // Checkpoint K (b1) — same boundary contract as the load gates; this
    // one has no tolerance dial and never had an epsilon.
    if crate::tool_load::boundary::exceeds_high(plunge_rate_mm_min, cap, 0.0) {
        let tip_d = match geometry {
            ToolGeometryHint::Ball => diameter_mm,
            ToolGeometryHint::TaperedBall { tip_radius, .. } => {
                (tip_radius * 2.0).max(MIN_TAPERED_TIP_DIAMETER_MM)
            }
            _ => return None,
        };
        Some(PlungeStressWarning {
            plunge_rate_mm_min,
            safe_cap_mm_min: cap,
            effective_tip_diameter_mm: tip_d,
        })
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn flat_endmill_has_no_cap() {
        assert!(safe_plunge_cap_mm_min(ToolGeometryHint::Flat, 6.0).is_none());
        assert!(check_plunge_stress(ToolGeometryHint::Flat, 6.0, 750.0).is_none());
    }

    #[test]
    fn bull_and_vbit_have_no_cap() {
        assert!(
            safe_plunge_cap_mm_min(ToolGeometryHint::Bull { corner_radius: 0.5 }, 6.0).is_none()
        );
        assert!(
            safe_plunge_cap_mm_min(
                ToolGeometryHint::VBit {
                    included_angle: 60.0,
                    tip_diameter: 0.0
                },
                6.0
            )
            .is_none()
        );
    }

    #[test]
    fn ball_cap_scales_with_diameter() {
        // 6 mm ball: cap = 150 × 6 = 900 mm/min
        let cap = safe_plunge_cap_mm_min(ToolGeometryHint::Ball, 6.0).unwrap();
        assert!((cap - 900.0).abs() < 1e-6);
        // 2 mm ball: cap = 150 × 2 = 300 mm/min
        let cap = safe_plunge_cap_mm_min(ToolGeometryHint::Ball, 2.0).unwrap();
        assert!((cap - 300.0).abs() < 1e-6);
    }

    #[test]
    fn tapered_ball_cap_uses_tip_diameter() {
        // 1 mm tapered ball (tip_radius = 0.5): cap = 150 × 1 = 150 mm/min
        let cap = safe_plunge_cap_mm_min(
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
            3.0, // shank diameter — irrelevant
        )
        .unwrap();
        assert!((cap - 150.0).abs() < 1e-6);
    }

    #[test]
    fn tapered_ball_tip_floors_at_0p5() {
        // Tip radius 0.1 → tip_d would be 0.2; floored to 0.5.
        let cap = safe_plunge_cap_mm_min(
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.1,
                taper_angle_deg: 7.0,
            },
            3.0,
        )
        .unwrap();
        assert!((cap - 75.0).abs() < 1e-6); // 150 × 0.5
    }

    #[test]
    fn ball_at_cap_is_silent() {
        // 6 mm ball at exactly 900 mm/min → no warning (at-cap, not over).
        assert!(check_plunge_stress(ToolGeometryHint::Ball, 6.0, 900.0).is_none());
        // Just under cap is silent.
        assert!(check_plunge_stress(ToolGeometryHint::Ball, 6.0, 899.9).is_none());
    }

    #[test]
    fn wanaka_tp7_tapered_ball_750_warns() {
        // Wanaka TP7: 1 mm tapered ball at 750 mm/min. Cap = 150.
        let w = check_plunge_stress(
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
            3.0,
            750.0,
        )
        .expect("750 mm/min on 1 mm TB must warn (cap = 150)");
        assert!((w.plunge_rate_mm_min - 750.0).abs() < 1e-6);
        assert!((w.safe_cap_mm_min - 150.0).abs() < 1e-6);
    }

    #[test]
    fn wanaka_tp4_tapered_ball_400_warns() {
        // Wanaka TP4/TP5: 400 mm/min on 1 mm tapered ball.
        let w = check_plunge_stress(
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
            3.0,
            400.0,
        )
        .expect("400 mm/min on 1 mm TB must warn");
        assert!(w.plunge_rate_mm_min > w.safe_cap_mm_min);
    }

    #[test]
    fn wanaka_6mm_em_plunge_750_silent() {
        // 6 mm flat end-mill is not a ball-class tool; no cap applies.
        // Mirrors test_flat_endmill_plunge_unchanged_by_fix2 in feeds tests.
        assert!(check_plunge_stress(ToolGeometryHint::Flat, 6.0, 750.0).is_none());
    }

    #[test]
    fn ball_2mm_at_400_warns() {
        // 2 mm ball, cap = 300. 400 > 300 → warns.
        assert!(check_plunge_stress(ToolGeometryHint::Ball, 2.0, 400.0).is_some());
    }
}
