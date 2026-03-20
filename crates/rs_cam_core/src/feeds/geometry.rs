//! Geometric helpers for feeds calculation — effective diameter, chip thinning,
//! scallop stepover, V-bit width at depth.
//!
//! These are pure geometry functions with no material/machine dependencies.
//! Ported from reference/shapeoko_feeds_and_speeds/src/calcs.rs lines 523-720.

/// Radial Chip Thinning Factor (RCTF).
///
/// When radial engagement is less than half the tool diameter, the actual chip
/// is thinner than the nominal feed-per-tooth. Feed rate must increase to
/// maintain consistent chip load.
///
/// `ae_mm` — radial width of cut (mm)
/// `diameter_mm` — tool diameter (mm)
///
/// Returns a multiplier >= 1.0 to apply to the nominal feed rate.
pub fn radial_chip_thinning_factor(ae_mm: f64, diameter_mm: f64) -> f64 {
    if diameter_mm <= 0.0 || ae_mm <= 0.0 {
        return 1.0;
    }
    let ae_ratio = (ae_mm / diameter_mm).clamp(0.0, 0.5);
    if ae_ratio >= 0.5 {
        return 1.0;
    }
    let denom = (1.0 - (1.0 - 2.0 * ae_ratio).powi(2)).sqrt();
    if denom <= 0.0 {
        1.0
    } else {
        (1.0 / denom).clamp(1.0, 4.0)
    }
}

/// Effective cutting diameter for a ball nose end mill at shallow axial depth.
///
/// For `ap >= R` (radius), the full diameter is engaged.
/// For `ap < R`, only a smaller circle of the ball contacts the material.
pub fn ball_effective_diameter(nominal_d: f64, axial_depth: f64) -> f64 {
    if nominal_d <= 0.0 {
        return 0.0;
    }
    let radius = nominal_d * 0.5;
    let ap = axial_depth.max(0.0);
    if ap <= 0.0 {
        return 0.01_f64.max(nominal_d * 0.01);
    }
    if ap >= radius {
        return nominal_d;
    }
    let value = 2.0 * (ap * (nominal_d - ap)).sqrt();
    value.max(0.01)
}

/// Effective cutting diameter for a tapered ball nose end mill.
///
/// The local radius at depth `ap` is `tip_r + ap * tan(taper_angle)`.
pub fn tapered_ball_effective_diameter(
    nominal_d: f64,
    tip_r: f64,
    taper_angle_deg: f64,
    axial_depth: f64,
) -> f64 {
    if nominal_d <= 0.0 || tip_r <= 0.0 {
        return nominal_d.max(0.01);
    }
    let ap = axial_depth.max(0.0);
    let side_angle_rad = taper_angle_deg.to_radians().max(0.0);
    let local_radius = tip_r + ap * side_angle_rad.tan();
    (2.0 * local_radius).clamp(0.01, nominal_d)
}

/// Effective cutting diameter for a bull nose end mill.
///
/// Below the corner radius, behaves like a ball of diameter `2*corner_r`.
/// Above the corner radius, full nominal diameter.
pub fn bull_nose_effective_diameter(nominal_d: f64, corner_r: f64, axial_depth: f64) -> f64 {
    if nominal_d <= 0.0 || corner_r <= 0.0 {
        return nominal_d.max(0.01);
    }
    let ap = axial_depth.max(0.0);
    if ap <= corner_r {
        let corner_effective = ball_effective_diameter(2.0 * corner_r, ap);
        corner_effective.clamp(0.01, nominal_d)
    } else {
        nominal_d
    }
}

/// Scallop-based stepover for a ball nose end mill.
///
/// Given a target scallop height, computes the required stepover distance.
/// Returns `None` if the scallop target is invalid (>= ball radius or <= 0).
pub fn scallop_stepover(ball_radius: f64, target_scallop: f64) -> Option<f64> {
    if ball_radius <= 0.0 || target_scallop <= 0.0 || target_scallop >= ball_radius {
        return None;
    }
    let inside = 2.0 * ball_radius * target_scallop - target_scallop.powi(2);
    if inside <= 0.0 {
        return None;
    }
    Some(2.0 * inside.sqrt())
}

/// V-bit cut width at a given depth.
///
/// `included_angle` — full V angle in degrees
/// `tip_d` — tip flat diameter (mm), 0 for pointed
/// `ap` — axial depth (mm)
pub fn vbit_width_at_depth(included_angle: f64, tip_d: f64, ap: f64) -> Option<f64> {
    if included_angle <= 0.0 || included_angle >= 180.0 || tip_d < 0.0 || ap < 0.0 {
        return None;
    }
    let half_angle = (included_angle * 0.5).to_radians();
    let width = tip_d + 2.0 * ap * half_angle.tan();
    Some(width.max(tip_d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rctf_full_engagement() {
        let f = radial_chip_thinning_factor(6.0, 6.0);
        assert!((f - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rctf_half_engagement() {
        let f = radial_chip_thinning_factor(3.0, 6.0);
        assert!((f - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rctf_quarter_engagement() {
        let f = radial_chip_thinning_factor(1.5, 6.0);
        assert!(f > 1.1 && f < 1.3, "got {f}");
    }

    #[test]
    fn test_rctf_light_engagement() {
        let f = radial_chip_thinning_factor(0.6, 6.0);
        assert!(f > 1.5, "got {f}");
    }

    #[test]
    fn test_rctf_zero_engagement() {
        assert_eq!(radial_chip_thinning_factor(0.0, 6.0), 1.0);
    }

    #[test]
    fn test_ball_effective_full_depth() {
        let d_eff = ball_effective_diameter(6.0, 3.0);
        assert!((d_eff - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_ball_effective_shallow() {
        let d_eff = ball_effective_diameter(6.0, 0.3);
        assert!(d_eff > 0.0 && d_eff < 6.0, "got {d_eff}");
    }

    #[test]
    fn test_tapered_ball_effective_scales_with_depth() {
        let shallow = tapered_ball_effective_diameter(6.0, 0.5, 2.0, 0.2);
        let deep = tapered_ball_effective_diameter(6.0, 0.5, 2.0, 2.0);
        assert!(shallow < deep && deep <= 6.0);
    }

    #[test]
    fn test_bull_nose_transitions_to_nominal() {
        let shallow = bull_nose_effective_diameter(6.0, 1.0, 0.2);
        let deep = bull_nose_effective_diameter(6.0, 1.0, 1.5);
        assert!(shallow < 6.0);
        assert!((deep - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_scallop_stepover_valid() {
        let stepover = scallop_stepover(3.0, 0.03).expect("should be valid");
        assert!(stepover > 0.0 && stepover < 6.0);
    }

    #[test]
    fn test_scallop_stepover_invalid() {
        assert!(scallop_stepover(3.0, 3.0).is_none());
        assert!(scallop_stepover(3.0, 0.0).is_none());
        assert!(scallop_stepover(0.0, 0.03).is_none());
    }

    #[test]
    fn test_vbit_width_increases_with_depth() {
        let shallow = vbit_width_at_depth(60.0, 0.2, 0.2).unwrap();
        let deep = vbit_width_at_depth(60.0, 0.2, 1.0).unwrap();
        assert!(deep > shallow);
    }

    #[test]
    fn test_vbit_invalid_angles() {
        assert!(vbit_width_at_depth(0.0, 0.2, 1.0).is_none());
        assert!(vbit_width_at_depth(180.0, 0.2, 1.0).is_none());
    }

    #[test]
    fn test_scallop_stepover_reference_value() {
        // 3mm ball radius, 0.1mm scallop target
        // stepover = 2 * sqrt(2*R*h - h^2) = 2 * sqrt(2*3*0.1 - 0.01) = 2 * sqrt(0.59) ≈ 1.536
        let stepover = scallop_stepover(3.0, 0.1).unwrap();
        assert!((stepover - 1.536).abs() < 0.01, "got {stepover}");
    }
}
