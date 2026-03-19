//! Scallop height formulas for 3D finishing strategies.
//!
//! Provides the math for computing scallop height from stepover (and vice versa)
//! on flat, curved, and inclined surfaces. Used by scallop finishing, steep & shallow,
//! and any operation that needs to convert between scallop height and stepover.
//!
//! Reference: `research/02_algorithms.md` section 8, `research/raw_algorithms.md` lines 789-875.

/// Scallop height for a ball endmill on a flat surface.
///
/// `h = R - sqrt(R^2 - (stepover/2)^2)`
///
/// Returns 0 if stepover >= 2*R (fully engaged, no scallop defined).
pub fn scallop_height_flat(tool_radius: f64, stepover: f64) -> f64 {
    let half_so = stepover * 0.5;
    let r_sq = tool_radius * tool_radius;
    let half_sq = half_so * half_so;
    if half_sq >= r_sq {
        return tool_radius; // Fully engaged — max scallop is the radius
    }
    tool_radius - (r_sq - half_sq).sqrt()
}

/// Stepover for a given scallop height on a flat surface.
///
/// `stepover = 2 * sqrt(2*R*h - h^2)`
///
/// Returns 0 if scallop_height >= R or <= 0.
pub fn stepover_from_scallop_flat(tool_radius: f64, scallop_height: f64) -> f64 {
    if scallop_height <= 0.0 || scallop_height >= tool_radius {
        return 0.0;
    }
    2.0 * (2.0 * tool_radius * scallop_height - scallop_height * scallop_height).sqrt()
}

/// Effective tool radius on a curved surface.
///
/// - Convex (curvature_radius > 0): `R_eff = R * Rc / (R + Rc)` — tighter stepover needed
/// - Concave (curvature_radius < 0): `R_eff = R * |Rc| / (|Rc| - R)` — wider stepover OK
/// - Flat (curvature_radius → ∞ or 0 curvature): `R_eff = R`
///
/// `curvature_radius` is the radius of curvature of the surface (1/curvature).
/// Positive = convex, negative = concave.
///
/// Returns `tool_radius` if curvature_radius is very large or the formula is undefined.
pub fn effective_radius(tool_radius: f64, curvature_radius: f64) -> f64 {
    let abs_rc = curvature_radius.abs();
    if abs_rc < 1e-6 {
        // Zero curvature radius means infinite curvature — not meaningful
        return tool_radius;
    }
    if abs_rc > 1e6 {
        // Nearly flat surface
        return tool_radius;
    }
    if curvature_radius > 0.0 {
        // Convex: R_eff = R * Rc / (R + Rc)
        tool_radius * abs_rc / (tool_radius + abs_rc)
    } else {
        // Concave: R_eff = R * |Rc| / (|Rc| - R)
        if abs_rc <= tool_radius {
            // Tool fits inside concavity — effectively flat
            return tool_radius;
        }
        tool_radius * abs_rc / (abs_rc - tool_radius)
    }
}

/// Stepover for a given scallop height, accounting for surface curvature.
///
/// `curvature`: positive = convex, negative = concave, 0 = flat.
/// This is the mean curvature value (1/radius), not the radius itself.
pub fn stepover_from_scallop_curved(
    tool_radius: f64,
    scallop_height: f64,
    curvature: f64,
) -> f64 {
    let r_eff = if curvature.abs() < 1e-9 {
        tool_radius
    } else {
        effective_radius(tool_radius, 1.0 / curvature)
    };
    stepover_from_scallop_flat(r_eff, scallop_height)
}

/// Scallop height given stepover, tool radius, and surface curvature.
pub fn scallop_height_curved(
    tool_radius: f64,
    stepover: f64,
    curvature: f64,
) -> f64 {
    let r_eff = if curvature.abs() < 1e-9 {
        tool_radius
    } else {
        effective_radius(tool_radius, 1.0 / curvature)
    };
    scallop_height_flat(r_eff, stepover)
}

/// Compute variable stepover at a point given desired scallop height,
/// tool radius, local surface slope angle, and curvature.
///
/// On inclined surfaces, the effective radius perpendicular to the slope is
/// `R / cos(slope_angle)` for a ball endmill — the projected cut profile
/// is wider. Combined with curvature adjustment, this gives the full formula.
///
/// `slope_angle`: radians from horizontal (0 = flat, PI/2 = vertical).
/// `curvature`: mean curvature (positive = convex, negative = concave, 0 = flat).
pub fn variable_stepover(
    tool_radius: f64,
    scallop_height: f64,
    slope_angle: f64,
    curvature: f64,
) -> f64 {
    // Slope adjustment: on an incline, the cross-slope effective radius is larger.
    // R_slope = R / cos(theta), but cap at reasonable values near vertical.
    let cos_theta = slope_angle.cos().max(0.05); // Cap at ~87 degrees
    let r_slope_adjusted = tool_radius / cos_theta;

    // Then apply curvature adjustment on top of the slope-adjusted radius.
    let r_eff = if curvature.abs() < 1e-9 {
        r_slope_adjusted
    } else {
        effective_radius(r_slope_adjusted, 1.0 / curvature)
    };

    // Compute stepover from the fully-adjusted effective radius.
    // Cap at a reasonable maximum (2× tool diameter).
    let so = stepover_from_scallop_flat(r_eff, scallop_height);
    so.min(tool_radius * 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    #[test]
    fn test_scallop_flat_known_values() {
        // R=5, stepover=2: h = 5 - sqrt(25 - 1) = 5 - sqrt(24) ≈ 0.10102
        let h = scallop_height_flat(5.0, 2.0);
        assert!((h - 0.10102).abs() < 0.001, "Expected ~0.101, got {:.5}", h);
    }

    #[test]
    fn test_stepover_flat_known_values() {
        // R=5, h=0.1: stepover = 2*sqrt(2*5*0.1 - 0.01) = 2*sqrt(0.99) ≈ 1.990
        let so = stepover_from_scallop_flat(5.0, 0.1);
        assert!((so - 1.990).abs() < 0.01, "Expected ~1.990, got {:.4}", so);
    }

    #[test]
    fn test_roundtrip_flat() {
        let r = 3.175; // Ball endmill radius
        let so = 1.5;
        let h = scallop_height_flat(r, so);
        let so_back = stepover_from_scallop_flat(r, h);
        assert!(
            (so - so_back).abs() < 1e-10,
            "Roundtrip failed: {} -> {} -> {}",
            so, h, so_back
        );
    }

    #[test]
    fn test_effective_radius_flat() {
        // Very large curvature radius → R_eff ≈ R
        let r_eff = effective_radius(5.0, 1e8);
        assert!((r_eff - 5.0).abs() < 0.01, "Flat surface R_eff should equal R, got {:.4}", r_eff);
    }

    #[test]
    fn test_effective_radius_convex() {
        // Convex surface: R_eff < R (tighter stepover needed)
        let r_eff = effective_radius(5.0, 20.0);
        assert!(
            r_eff < 5.0,
            "Convex R_eff should be less than R, got {:.4}",
            r_eff
        );
        // R_eff = 5 * 20 / (5 + 20) = 100/25 = 4.0
        assert!((r_eff - 4.0).abs() < 0.001, "Expected 4.0, got {:.4}", r_eff);
    }

    #[test]
    fn test_effective_radius_concave() {
        // Concave surface: R_eff > R (wider stepover OK)
        let r_eff = effective_radius(5.0, -20.0);
        assert!(
            r_eff > 5.0,
            "Concave R_eff should be greater than R, got {:.4}",
            r_eff
        );
        // R_eff = 5 * 20 / (20 - 5) = 100/15 ≈ 6.667
        assert!((r_eff - 6.667).abs() < 0.01, "Expected ~6.667, got {:.4}", r_eff);
    }

    #[test]
    fn test_scallop_curved_convex_tighter() {
        // Same stepover on convex surface → higher scallop than flat
        let h_flat = scallop_height_flat(5.0, 2.0);
        let h_convex = scallop_height_curved(5.0, 2.0, 0.05); // curvature=0.05 → Rc=20
        assert!(
            h_convex > h_flat,
            "Convex scallop ({:.4}) should exceed flat ({:.4})",
            h_convex, h_flat
        );
    }

    #[test]
    fn test_scallop_curved_concave_lower() {
        // Same stepover on concave surface → lower scallop than flat
        let h_flat = scallop_height_flat(5.0, 2.0);
        let h_concave = scallop_height_curved(5.0, 2.0, -0.05);
        assert!(
            h_concave < h_flat,
            "Concave scallop ({:.4}) should be less than flat ({:.4})",
            h_concave, h_flat
        );
    }

    #[test]
    fn test_variable_stepover_flat_zero_slope() {
        // Flat surface, zero slope → same as flat formula
        let so_flat = stepover_from_scallop_flat(5.0, 0.1);
        let so_var = variable_stepover(5.0, 0.1, 0.0, 0.0);
        assert!(
            (so_flat - so_var).abs() < 0.001,
            "Zero slope should match flat: flat={:.4} var={:.4}",
            so_flat, so_var
        );
    }

    #[test]
    fn test_variable_stepover_steep_wider() {
        // Steep surface (80 degrees) → wider stepover than flat
        let so_flat = variable_stepover(5.0, 0.1, 0.0, 0.0);
        let so_steep = variable_stepover(5.0, 0.1, 80.0_f64.to_radians(), 0.0);
        assert!(
            so_steep > so_flat,
            "Steep slope should give wider stepover: flat={:.4} steep={:.4}",
            so_flat, so_steep
        );
    }

    #[test]
    fn test_variable_stepover_45_degree() {
        // 45 degree slope: R_eff = R / cos(45) = R * sqrt(2) ≈ 1.414*R
        let so = variable_stepover(5.0, 0.1, FRAC_PI_4, 0.0);
        let r_eff = 5.0 / FRAC_PI_4.cos(); // ~7.071
        let expected = stepover_from_scallop_flat(r_eff, 0.1);
        assert!(
            (so - expected).abs() < 0.001,
            "45° slope: expected {:.4} got {:.4}",
            expected, so
        );
    }

    #[test]
    fn test_edge_cases() {
        // Zero scallop height → zero stepover
        assert_eq!(stepover_from_scallop_flat(5.0, 0.0), 0.0);
        // Scallop height >= radius → zero stepover
        assert_eq!(stepover_from_scallop_flat(5.0, 5.0), 0.0);
        assert_eq!(stepover_from_scallop_flat(5.0, 6.0), 0.0);
        // Stepover >= diameter → max scallop
        assert_eq!(scallop_height_flat(5.0, 10.0), 5.0);
        assert_eq!(scallop_height_flat(5.0, 12.0), 5.0);
    }
}
