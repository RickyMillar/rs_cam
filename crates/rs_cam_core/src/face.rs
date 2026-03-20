//! Face/surfacing operation for leveling the top of stock material.
//!
//! Generates a zigzag raster toolpath that covers the full stock boundary
//! (plus optional offset). Supports single-pass facing at Z=0 or multi-pass
//! depth stepping for removing material from the stock top.

use crate::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use crate::geo::BoundingBox3;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;
use crate::zigzag::{ZigzagParams, zigzag_toolpath};

/// Direction of facing passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaceDirection {
    /// All passes cut in the same direction (rapid return between passes).
    OneWay,
    /// Alternating pass directions (zigzag / bidirectional).
    Zigzag,
}

/// Parameters for a face/surfacing operation.
#[derive(Debug, Clone)]
pub struct FaceParams {
    /// Tool radius in mm.
    pub tool_radius: f64,
    /// Distance between passes in mm (default: 80% of tool diameter).
    pub stepover: f64,
    /// Total face depth in mm. 0 = single pass at stock top (Z=0).
    pub depth: f64,
    /// Maximum depth per pass in mm.
    pub depth_per_pass: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Extra offset beyond stock boundary on each side in mm.
    pub stock_offset: f64,
    /// Direction of facing passes.
    pub direction: FaceDirection,
}

/// Generate a face/surfacing toolpath over the XY extent of a bounding box.
///
/// Creates a rectangle from the bounding box XY bounds (expanded by
/// `stock_offset`), then fills it with zigzag passes. If `depth > 0`,
/// multiple passes are generated using depth stepping.
pub fn face_toolpath(bounds: &BoundingBox3, params: &FaceParams) -> Toolpath {
    // Build the facing rectangle from XY bounds + stock_offset
    let rect = Polygon2::rectangle(
        bounds.min.x - params.stock_offset,
        bounds.min.y - params.stock_offset,
        bounds.max.x + params.stock_offset,
        bounds.max.y + params.stock_offset,
    );

    if params.depth <= 0.0 {
        // Single pass at Z=0 (stock top)
        let zp = ZigzagParams {
            tool_radius: params.tool_radius,
            stepover: params.stepover,
            cut_depth: 0.0,
            feed_rate: params.feed_rate,
            plunge_rate: params.plunge_rate,
            safe_z: params.safe_z,
            angle: 0.0,
        };
        zigzag_toolpath(&rect, &zp)
    } else {
        // Multi-pass depth stepping
        let stepping = DepthStepping {
            start_z: 0.0,
            final_z: -params.depth,
            max_step_down: params.depth_per_pass,
            distribution: DepthDistribution::Even,
            finish_allowance: 0.0,
        };

        depth_stepped_toolpath(&stepping, params.safe_z, |z| {
            let zp = ZigzagParams {
                tool_radius: params.tool_radius,
                stepover: params.stepover,
                cut_depth: z,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                angle: 0.0,
            };
            zigzag_toolpath(&rect, &zp)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::toolpath::MoveType;

    fn stock_100x100() -> BoundingBox3 {
        BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 100.0, 25.0),
        }
    }

    fn default_params() -> FaceParams {
        FaceParams {
            tool_radius: 12.5, // 25mm face mill
            stepover: 20.0,    // 80% of 25mm diameter
            depth: 0.0,
            depth_per_pass: 2.0,
            feed_rate: 2000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            stock_offset: 5.0,
            direction: FaceDirection::Zigzag,
        }
    }

    #[test]
    fn face_100x100_produces_nonempty_toolpath() {
        let bounds = stock_100x100();
        let params = default_params();
        let tp = face_toolpath(&bounds, &params);

        assert!(
            !tp.moves.is_empty(),
            "Facing a 100x100 stock should produce moves"
        );

        // All cutting moves should be at Z=0 (single pass, depth=0)
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type {
                if (feed_rate - params.feed_rate).abs() < 1e-10 {
                    assert!(
                        (m.target.z - 0.0).abs() < 1e-10,
                        "Single-pass face should cut at Z=0, got Z={}",
                        m.target.z
                    );
                }
            }
        }

        // All rapids should be at safe_z
        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - params.safe_z).abs() < 1e-10,
                    "Rapid at Z={}, expected safe_z={}",
                    m.target.z,
                    params.safe_z
                );
            }
        }
    }

    #[test]
    fn face_with_depth_stepping_produces_multiple_z_levels() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.depth = 6.0;
        params.depth_per_pass = 2.0;

        let tp = face_toolpath(&bounds, &params);

        assert!(
            !tp.moves.is_empty(),
            "Multi-pass face should produce moves"
        );

        // Collect unique Z levels of cutting moves (at feed_rate)
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| (m.target.z * 1000.0).round() / 1000.0)
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        cut_zs.dedup();

        assert_eq!(
            cut_zs.len(),
            3,
            "6mm depth at 2mm/pass should produce 3 Z levels, got {:?}",
            cut_zs
        );
        assert!((cut_zs[0] - -2.0).abs() < 0.01);
        assert!((cut_zs[1] - -4.0).abs() < 0.01);
        assert!((cut_zs[2] - -6.0).abs() < 0.01);
    }

    #[test]
    fn face_stock_offset_expands_coverage() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.stock_offset = 10.0;

        let tp = face_toolpath(&bounds, &params);

        // Cutting moves should extend beyond the stock boundary
        // The rectangle is (-10, -10) to (110, 110), inset by tool_radius
        let cutting_xs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| m.target.x)
            .collect();

        if !cutting_xs.is_empty() {
            let min_x = cutting_xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_x = cutting_xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            // Inset polygon goes from -10+12.5=2.5 to 110-12.5=97.5
            // So cutting should span roughly that range
            assert!(
                min_x < 5.0,
                "With stock_offset=10, cutting should start before X=5, got {}",
                min_x
            );
            assert!(
                max_x > 95.0,
                "With stock_offset=10, cutting should extend past X=95, got {}",
                max_x
            );
        }
    }

    #[test]
    fn face_zero_depth_single_pass() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.depth = 0.0;

        let tp = face_toolpath(&bounds, &params);

        // All cutting moves at exactly Z=0
        let cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect();

        for z in &cut_zs {
            assert!(
                z.abs() < 1e-10 || (*z - 0.0).abs() < 1e-10,
                "Zero-depth face should only cut at Z=0, got Z={}",
                z
            );
        }
    }
}
