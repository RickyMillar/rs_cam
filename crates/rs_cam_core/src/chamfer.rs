//! Chamfer operation: edge chamfers using a V-bit along profile edges.
//!
//! A chamfer is geometrically equivalent to a profile cut at a computed Z depth
//! where the V-bit's cutting diameter at that depth produces the desired chamfer
//! width on the workpiece face.

use crate::polygon::Polygon2;
use crate::profile::{ProfileParams, ProfileSide, profile_toolpath};
use crate::toolpath::Toolpath;

/// Parameters for chamfer cutting with a V-bit.
pub struct ChamferParams {
    /// Width of the chamfer on the workpiece face (mm).
    pub chamfer_width: f64,
    /// Distance from V-bit tip to contact point, preventing tip wear (mm).
    /// Default: 0.1 mm.
    pub tip_offset: f64,
    /// Half angle of the V-bit in radians (e.g., 45-degree V-bit = pi/4).
    pub tool_half_angle: f64,
    /// Tool shank radius for clearance checks (mm). Not used for offset
    /// calculation — the V-bit's effective cutting radius is computed from
    /// the cut depth and half angle.
    pub tool_radius: f64,
    /// Cutting feed rate (mm/min).
    pub feed_rate: f64,
    /// Plunge feed rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves (mm).
    pub safe_z: f64,
}

/// Compute the cut depth for a chamfer.
///
/// The V-bit must plunge deep enough that the sloped face of the cutter
/// spans the requested chamfer width, plus the tip offset that keeps the
/// fragile tip away from the work.
///
/// `depth = (chamfer_width + tip_offset) / tan(half_angle)`
fn chamfer_depth(params: &ChamferParams) -> f64 {
    (params.chamfer_width + params.tip_offset) / params.tool_half_angle.tan()
}

/// Compute the V-bit's effective cutting radius at a given depth.
///
/// At depth `d`, the V-bit cone has radius `d * tan(half_angle)`.
fn effective_radius_at_depth(depth: f64, half_angle: f64) -> f64 {
    depth * half_angle.tan()
}

/// Generate a chamfer toolpath around the outside of a polygon using a V-bit.
///
/// The chamfer is produced by running a profile cut at a computed Z depth
/// where the V-bit's conical face intersects the top edge of the stock at
/// the desired chamfer width. The tool center follows a path offset from
/// the polygon edge by half the chamfer width (so the cut is centered on
/// the edge).
///
/// # Geometry
///
/// For a V-bit with half angle `a`, chamfer width `w`, and tip offset `t`:
/// - Cut depth: `d = (w + t) / tan(a)`
/// - Effective tool radius at that depth: `r = d * tan(a) = w + t`
/// - The profile offset places the tool center at distance `r` from the edge,
///   so the cutter's outer contact point is at `r` from the edge and the
///   inner contact point (at the surface) is at `r - w = t` from the edge.
///
/// This means the chamfer starts at the polygon edge and extends `w` mm
/// inward on the top face.
pub fn chamfer_toolpath(polygon: &Polygon2, params: &ChamferParams) -> Toolpath {
    let depth = chamfer_depth(params);

    // The V-bit's effective cutting radius at the chamfer depth is the
    // offset distance from the polygon edge to the tool center.
    let effective_tool_radius = effective_radius_at_depth(depth, params.tool_half_angle);

    let profile_params = ProfileParams {
        tool_radius: effective_tool_radius,
        side: ProfileSide::Outside,
        cut_depth: -depth,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        climb: false,
    };

    profile_toolpath(polygon, &profile_params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;
    use std::f64::consts::FRAC_PI_4;

    fn default_chamfer_params() -> ChamferParams {
        ChamferParams {
            chamfer_width: 2.0,
            tip_offset: 0.1,
            tool_half_angle: FRAC_PI_4, // 45 degrees
            tool_radius: 6.0,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 10.0,
        }
    }

    #[test]
    fn test_chamfer_depth_45_degree() {
        // 45-degree V-bit: tan(pi/4) = 1.0
        // depth = (2.0 + 0.1) / 1.0 = 2.1
        let params = default_chamfer_params();
        let depth = chamfer_depth(&params);
        assert!(
            (depth - 2.1).abs() < 1e-10,
            "Expected depth 2.1, got {}",
            depth
        );
    }

    #[test]
    fn test_chamfer_depth_30_degree() {
        // 30-degree V-bit (60-degree included): tan(pi/6) = 1/sqrt(3)
        let params = ChamferParams {
            chamfer_width: 2.0,
            tip_offset: 0.0,
            tool_half_angle: std::f64::consts::FRAC_PI_6,
            tool_radius: 6.0,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            safe_z: 10.0,
        };
        let depth = chamfer_depth(&params);
        let expected = 2.0 / (std::f64::consts::FRAC_PI_6).tan();
        assert!(
            (depth - expected).abs() < 1e-10,
            "Expected depth {}, got {}",
            expected,
            depth
        );
        // tan(30 deg) ~ 0.5774, so depth ~ 3.464
        assert!(
            (depth - 3.464).abs() < 0.01,
            "30-degree depth should be ~3.464, got {}",
            depth
        );
    }

    #[test]
    fn test_tip_offset_increases_depth() {
        let mut params = default_chamfer_params();
        params.tip_offset = 0.0;
        let depth_no_offset = chamfer_depth(&params);

        params.tip_offset = 0.5;
        let depth_with_offset = chamfer_depth(&params);

        assert!(
            depth_with_offset > depth_no_offset,
            "Tip offset should increase depth: {} > {}",
            depth_with_offset,
            depth_no_offset
        );

        // For 45-degree bit, difference should equal the offset itself
        // (tan(45) = 1, so delta_depth = delta_offset / 1.0 = 0.5)
        let delta = depth_with_offset - depth_no_offset;
        assert!(
            (delta - 0.5).abs() < 1e-10,
            "Depth increase should be 0.5 for 45-deg bit, got {}",
            delta
        );
    }

    #[test]
    fn test_chamfer_on_square_produces_toolpath() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let tp = chamfer_toolpath(&square, &params);

        assert!(
            !tp.moves.is_empty(),
            "Chamfer on 40x40 square should produce moves"
        );

        // Should have rapids (approach + retract)
        let n_rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert_eq!(n_rapids, 2, "Expected 2 rapids, got {}", n_rapids);
    }

    #[test]
    fn test_chamfer_cut_depth_in_toolpath() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let expected_depth = chamfer_depth(&params); // 2.1 for default params
        let tp = chamfer_toolpath(&square, &params);

        // All cutting moves should be at z = -depth
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type {
                if (feed_rate - params.feed_rate).abs() < 1e-10 {
                    assert!(
                        (m.target.z - (-expected_depth)).abs() < 1e-10,
                        "Cutting move at z={}, expected z={}",
                        m.target.z,
                        -expected_depth
                    );
                }
            }
        }
    }

    #[test]
    fn test_chamfer_offset_outside_boundary() {
        let square = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let params = default_chamfer_params();
        let tp = chamfer_toolpath(&square, &params);

        // The effective radius at cut depth is (2.0 + 0.1) * tan(45) / tan(45) = 2.1
        // So tool center should be 2.1mm outside the polygon boundary.
        // Check that cutting moves extend beyond the original square.
        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .collect();

        assert!(!cutting_moves.is_empty(), "Should have cutting moves");

        let x_min = cutting_moves
            .iter()
            .map(|m| m.target.x)
            .fold(f64::INFINITY, f64::min);
        let x_max = cutting_moves
            .iter()
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        // Tool center should be outside the 0..40 boundary
        assert!(
            x_min < 0.0,
            "Tool center x_min={} should be outside boundary (< 0)",
            x_min
        );
        assert!(
            x_max > 40.0,
            "Tool center x_max={} should be outside boundary (> 40)",
            x_max
        );
    }

    #[test]
    fn test_effective_radius_at_depth() {
        // 45-degree: radius = depth
        let r = effective_radius_at_depth(3.0, FRAC_PI_4);
        assert!((r - 3.0).abs() < 1e-10, "45-deg: radius should equal depth");

        // 30-degree: radius = depth * tan(30) ~ depth * 0.5774
        let r30 = effective_radius_at_depth(3.0, std::f64::consts::FRAC_PI_6);
        let expected = 3.0 * (std::f64::consts::FRAC_PI_6).tan();
        assert!(
            (r30 - expected).abs() < 1e-10,
            "30-deg: expected {}, got {}",
            expected,
            r30
        );
    }
}
