//! Face/surfacing operation for leveling the top of stock material.
//!
//! Generates a zigzag raster toolpath that covers the full stock boundary
//! (plus optional offset). Supports single-pass facing at Z=0 or multi-pass
//! depth stepping for removing material from the stock top.

use crate::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use crate::geo::BoundingBox3;
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;
use crate::zigzag::{ZigzagParams, lines_to_toolpath, zigzag_lines, zigzag_toolpath};

/// Direction of facing passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Generate a one-way (unidirectional) raster toolpath inside a polygon.
///
/// Uses `zigzag_lines` to compute scan rows, then normalises every row so
/// they all cut in the same direction (the direction of the first row).
/// Between rows the tool rapids at `safe_z`, exactly like the zigzag path
/// but without alternating.
fn oneway_toolpath(polygon: &Polygon2, zp: &ZigzagParams) -> Toolpath {
    let mut lines = zigzag_lines(polygon, zp.tool_radius, zp.stepover, zp.angle);

    if lines.len() > 1 {
        // Pick the direction of the first row as the canonical direction.
        // For angle=0 this is the X component; generalise via the scan
        // direction vector (cos(angle), sin(angle)).
        let angle_rad = zp.angle.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // Dot the first row's displacement with the scan direction to
        // determine the canonical sign.
        // Compute reference dot product from the first line's direction.
        let ref_dot = lines.first().map(|first| {
            #[allow(clippy::indexing_slicing)]
            // SAFETY: each line is a fixed-size [P2; 2] array
            let dx = first[1].x - first[0].x;
            #[allow(clippy::indexing_slicing)]
            let dy = first[1].y - first[0].y;
            dx * cos_a + dy * sin_a
        });

        if let Some(ref_dot) = ref_dot {
            for line in &mut lines {
                #[allow(clippy::indexing_slicing)]
                // SAFETY: each line is a fixed-size [P2; 2] array
                let dot = (line[1].x - line[0].x) * cos_a + (line[1].y - line[0].y) * sin_a;
                // If this row goes the opposite way, swap its endpoints.
                if dot * ref_dot < 0.0 {
                    line.swap(0, 1);
                }
            }
        }
    }

    lines_to_toolpath(&lines, zp)
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

    // Choose the raster strategy based on direction.
    let raster_fn = |polygon: &Polygon2, zp: &ZigzagParams| -> Toolpath {
        match params.direction {
            FaceDirection::OneWay => oneway_toolpath(polygon, zp),
            FaceDirection::Zigzag => zigzag_toolpath(polygon, zp),
        }
    };

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
        raster_fn(&rect, &zp)
    } else {
        // Multi-pass depth stepping
        let stepping = DepthStepping {
            start_z: 0.0,
            final_z: -params.depth,
            max_step_down: params.depth_per_pass,
            distribution: DepthDistribution::Even,
            finish_allowance: 0.0,
            finishing_passes: 0,
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
            raster_fn(&rect, &zp)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
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
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                assert!(
                    (m.target.z - 0.0).abs() < 1e-10,
                    "Single-pass face should cut at Z=0, got Z={}",
                    m.target.z
                );
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

        assert!(!tp.moves.is_empty(), "Multi-pass face should produce moves");

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

    // --- OneWay direction produces same-direction passes ---

    #[test]
    fn face_oneway_all_passes_same_x_direction() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.direction = FaceDirection::OneWay;

        let tp = face_toolpath(&bounds, &params);

        // Collect X coordinates of cutting moves at feed_rate.
        // Each pass is a single cut segment; check that all segments have the
        // same X direction (i.e., all increasing or all decreasing).
        let cutting_moves: Vec<(f64, f64)> = {
            let mut segs = Vec::new();
            let mut prev_x: Option<f64> = None;
            for m in &tp.moves {
                if let MoveType::Linear { feed_rate } = m.move_type
                    && (feed_rate - params.feed_rate).abs() < 1e-10
                    && let Some(px) = prev_x
                {
                    segs.push((px, m.target.x));
                }
                prev_x = Some(m.target.x);
            }
            segs
        };

        assert!(
            !cutting_moves.is_empty(),
            "OneWay face should produce cutting segments"
        );

        // All deltas should have the same sign (or be near-zero for single-point segments).
        let mut positive = 0;
        let mut negative = 0;
        for (sx, ex) in &cutting_moves {
            let dx = ex - sx;
            if dx > 1.0 {
                positive += 1;
            } else if dx < -1.0 {
                negative += 1;
            }
        }

        // Exactly one of positive/negative should be non-zero.
        assert!(
            positive == 0 || negative == 0,
            "OneWay should have all passes in the same X direction, \
             but got {} positive and {} negative segments",
            positive,
            negative
        );
        assert!(
            positive > 0 || negative > 0,
            "Should have at least one significant cutting segment"
        );
    }

    #[test]
    fn face_oneway_with_depth_stepping() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.direction = FaceDirection::OneWay;
        params.depth = 4.0;
        params.depth_per_pass = 2.0;

        let tp = face_toolpath(&bounds, &params);

        // Should produce multiple Z levels
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| (m.target.z * 1000.0).round() / 1000.0)
            .collect();
        cut_zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cut_zs.dedup();

        assert_eq!(
            cut_zs.len(),
            2,
            "4mm depth at 2mm/pass should produce 2 Z levels, got {:?}",
            cut_zs
        );

        // All cutting segments should go in the same X direction (unidirectional)
        let cutting_moves: Vec<(f64, f64)> = {
            let mut segs = Vec::new();
            let mut prev_x: Option<f64> = None;
            for m in &tp.moves {
                if let MoveType::Linear { feed_rate } = m.move_type
                    && (feed_rate - params.feed_rate).abs() < 1e-10
                    && let Some(px) = prev_x
                {
                    segs.push((px, m.target.x));
                }
                prev_x = Some(m.target.x);
            }
            segs
        };

        let mut positive = 0;
        let mut negative = 0;
        for (sx, ex) in &cutting_moves {
            let dx = ex - sx;
            if dx > 1.0 {
                positive += 1;
            } else if dx < -1.0 {
                negative += 1;
            }
        }

        assert!(
            positive == 0 || negative == 0,
            "OneWay with depth stepping should have all passes in the same X direction"
        );
    }

    // --- Task E-face: Zigzag direction produces alternating X directions ---

    #[test]
    fn face_zigzag_has_alternating_x_directions() {
        let bounds = stock_100x100();
        let params = default_params(); // FaceDirection::Zigzag, angle=0 means raster along X

        let tp = face_toolpath(&bounds, &params);

        // Collect X coordinates of sequential cutting moves at feed_rate
        let cutting_xs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| m.target.x)
            .collect();

        // In zigzag at angle=0, each pass rasters along X. The zigzag means
        // consecutive passes alternate X direction (left-to-right then right-to-left).
        // We verify: both increasing and decreasing X transitions should appear
        // in the cutting moves.
        if cutting_xs.len() > 2 {
            let mut has_increase = false;
            let mut has_decrease = false;
            for w in cutting_xs.windows(2) {
                let dx = w[1] - w[0];
                if dx > 1.0 {
                    has_increase = true;
                }
                if dx < -1.0 {
                    has_decrease = true;
                }
            }
            assert!(
                has_increase && has_decrease,
                "Zigzag should have both increasing and decreasing X transitions among cutting moves"
            );
        }
    }

    // --- Stepover larger than stock width ---

    #[test]
    fn face_stepover_larger_than_width() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.stepover = 200.0; // stepover > stock width

        let tp = face_toolpath(&bounds, &params);

        // Should still produce some moves (at least one pass)
        assert!(
            !tp.moves.is_empty(),
            "Stepover > width should still produce moves"
        );

        // All cutting moves at Z=0
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                assert!(
                    (m.target.z - 0.0).abs() < 1e-10,
                    "Single-pass cut should be at Z=0, got {}",
                    m.target.z
                );
            }
        }
    }

    // --- Very small stepover ---

    #[test]
    fn face_very_small_stepover_produces_many_passes() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.stepover = 1.0; // very small stepover

        let tp = face_toolpath(&bounds, &params);

        // Should produce many more moves than with stepover=20
        let mut params_normal = default_params();
        params_normal.stepover = 20.0;
        let tp_normal = face_toolpath(&bounds, &params_normal);

        assert!(
            tp.moves.len() > tp_normal.moves.len(),
            "Small stepover ({} moves) should produce more moves than normal stepover ({} moves)",
            tp.moves.len(),
            tp_normal.moves.len()
        );
    }

    // --- Tool larger than stock ---

    #[test]
    fn face_tool_larger_than_stock() {
        let bounds = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let mut params = default_params();
        params.tool_radius = 25.0; // 50mm diameter >> 10mm stock

        let tp = face_toolpath(&bounds, &params);

        // Should produce moves (the zigzag still operates on the expanded polygon)
        // Even with a huge tool, the facing operation is still valid
        // (the tool covers the entire stock in one or very few passes)
        // The result is either some moves or empty, depending on how the
        // polygon inset handles a tool wider than the stock + offset.
        // This is an edge case -- we just verify no panic.
        let _ = tp;
    }

    // --- Negative depth treated as zero ---

    #[test]
    fn face_negative_depth_treated_as_single_pass() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.depth = -5.0;

        let tp = face_toolpath(&bounds, &params);

        // depth <= 0.0 should take the single-pass path
        // All cutting moves should be at Z=0
        let cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| m.target.z)
            .collect();

        for z in &cut_zs {
            assert!(
                z.abs() < 1e-10,
                "Negative depth face should produce cuts at Z=0, got Z={}",
                z
            );
        }
    }

    // --- Depth stepping with large depth_per_pass ---

    #[test]
    fn face_depth_per_pass_larger_than_total_depth() {
        let bounds = stock_100x100();
        let mut params = default_params();
        params.depth = 3.0;
        params.depth_per_pass = 10.0; // one pass covers all depth

        let tp = face_toolpath(&bounds, &params);

        assert!(!tp.moves.is_empty(), "Should produce moves");

        // Should produce only one Z level (-3.0)
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10)
            })
            .map(|m| (m.target.z * 1000.0).round() / 1000.0)
            .collect();
        cut_zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cut_zs.dedup();

        assert_eq!(
            cut_zs.len(),
            1,
            "depth_per_pass > depth should produce 1 Z level, got {:?}",
            cut_zs
        );
        assert!(
            (cut_zs[0] - (-3.0)).abs() < 0.01,
            "Single Z level should be -3.0, got {}",
            cut_zs[0]
        );
    }

    // --- Small stock ---

    #[test]
    fn face_tiny_stock_no_panic() {
        let bounds = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(1.0, 1.0, 1.0),
        };
        let params = default_params();

        // With tool_radius=12.5 and stock 1x1, the inset polygon may be empty.
        // Verify no panic.
        let tp = face_toolpath(&bounds, &params);
        let _ = tp;
    }
}
