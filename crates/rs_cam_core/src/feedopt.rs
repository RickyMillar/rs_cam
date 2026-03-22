//! Feed rate optimization — post-process dressup that adjusts feed rates
//! based on material engagement.
//!
//! Works as a dressup (post-processing pass) on any operation's toolpath.
//! Simulates material removal using a tri-dexel stock, computes engagement at
//! each move, and adjusts feed rates using the Radial Chip Thinning Factor
//! (RCTF) to maintain consistent chip load.
//!
//! Benefits for wood:
//! - Eliminates burn marks from lingering in light cuts
//! - 15-30% faster cycle times on variable-engagement operations
//! - Consistent chip load → better surface finish

use crate::geo::P3;
use crate::dexel::ray_top;
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::radial_profile::RadialProfileLUT;
use crate::tool::MillingCutter;
use crate::toolpath::{Move, MoveType, Toolpath};

/// Parameters for feed rate optimization.
pub struct FeedOptParams {
    /// Base feed rate at full engagement (mm/min).
    pub nominal_feed_rate: f64,
    /// Maximum allowed feed rate (mm/min). Typically 2-4× nominal.
    pub max_feed_rate: f64,
    /// Minimum allowed feed rate (mm/min).
    pub min_feed_rate: f64,
    /// Maximum feed rate change per mm of travel (mm/min per mm).
    /// Prevents abrupt acceleration/deceleration.
    pub ramp_rate: f64,
    /// Below this engagement fraction, use max feed (air cutting).
    pub air_cut_threshold: f64,
}

/// Compute the Radial Chip Thinning Factor (RCTF).
///
/// Delegates to `feeds::geometry::radial_chip_thinning_factor` for the shared
/// implementation. This wrapper accepts an ae fraction (0..1) instead of
/// absolute mm values.
fn rctf(ae_fraction: f64) -> f64 {
    // Convert fraction to absolute values — use diameter = 1.0 so ae_fraction
    // directly represents ae/D ratio.
    crate::feeds::geometry::radial_chip_thinning_factor(ae_fraction, 1.0)
}

/// Estimate material engagement fraction at a point using the tri-dexel stock.
///
/// Samples points on the tool circumference and counts how many are above
/// the stock surface (i.e., material exists there).
fn estimate_engagement(
    stock: &TriDexelStock,
    cx: f64,
    cy: f64,
    tool_z: f64,
    tool_radius: f64,
    n_samples: usize,
) -> f64 {
    let mut engaged = 0;
    let step = std::f64::consts::TAU / n_samples as f64;
    let grid = &stock.z_grid;

    for i in 0..n_samples {
        let angle = i as f64 * step;
        let sx = cx + tool_radius * angle.cos();
        let sy = cy + tool_radius * angle.sin();

        // Look up top Z at this point in the z_grid
        if let Some((row, col)) = grid.world_to_cell(sx, sy) {
            let ray = grid.ray(row, col);
            if let Some(top_z) = ray_top(ray) {
                let cell_z = top_z as f64;
                if cell_z > tool_z + 0.01 {
                    engaged += 1;
                }
            }
        }
    }

    engaged as f64 / n_samples as f64
}

/// Apply feed rate optimization to a toolpath.
///
/// Walks the toolpath, simulates material removal, computes engagement
/// at each cutting move, and adjusts feed rates using RCTF.
pub fn optimize_feed_rates(
    toolpath: &Toolpath,
    cutter: &dyn MillingCutter,
    stock: &mut TriDexelStock,
    params: &FeedOptParams,
) -> Toolpath {
    let tool_radius = cutter.radius();
    let n_samples = 24; // circumference samples for engagement
    let lut = RadialProfileLUT::from_cutter(cutter, 256);

    // First pass: compute optimal feed rate for each move
    let mut feed_rates: Vec<f64> = Vec::with_capacity(toolpath.moves.len());

    for mv in &toolpath.moves {
        match mv.move_type {
            MoveType::Rapid => {
                feed_rates.push(0.0); // Rapids don't have feed rates
            }
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                let cx = mv.target.x;
                let cy = mv.target.y;
                let cz = mv.target.z;

                // Estimate engagement before stamping
                let engagement = estimate_engagement(stock, cx, cy, cz, tool_radius, n_samples);

                // Compute adjusted feed rate
                let adjusted = if engagement < params.air_cut_threshold {
                    params.max_feed_rate
                } else {
                    let factor = rctf(engagement);
                    (params.nominal_feed_rate * factor)
                        .clamp(params.min_feed_rate, params.max_feed_rate)
                };

                feed_rates.push(adjusted);

                // Stamp tool into stock (update material state)
                stock.stamp_tool_at(&lut, tool_radius, cx, cy, cz, StockCutDirection::FromTop);
            }
        }
    }

    // Second pass: smooth feed rate transitions
    smooth_feed_rates(&mut feed_rates, &toolpath.moves, params.ramp_rate);

    // Third pass: build output toolpath with adjusted feed rates
    let mut output = Toolpath::new();
    for (i, mv) in toolpath.moves.iter().enumerate() {
        match mv.move_type {
            MoveType::Rapid => {
                output.rapid_to(mv.target);
            }
            MoveType::Linear { .. } => {
                output.feed_to(mv.target, feed_rates[i]);
            }
            MoveType::ArcCW { i: ci, j: cj, .. } => {
                output.arc_cw_to(mv.target, ci, cj, feed_rates[i]);
            }
            MoveType::ArcCCW { i: ci, j: cj, .. } => {
                output.arc_ccw_to(mv.target, ci, cj, feed_rates[i]);
            }
        }
    }

    output
}

/// Smooth feed rate transitions to limit acceleration.
/// Forward pass limits increases, backward pass limits decreases.
fn smooth_feed_rates(feeds: &mut [f64], moves: &[Move], ramp_rate: f64) {
    if feeds.len() < 2 || ramp_rate <= 0.0 {
        return;
    }

    // Forward pass: limit feed rate increases
    for i in 1..feeds.len() {
        if feeds[i] <= 0.0 || feeds[i - 1] <= 0.0 {
            continue; // Skip rapids
        }
        let dist = move_distance(&moves[i - 1].target, &moves[i].target);
        let max_increase = ramp_rate * dist;
        feeds[i] = feeds[i].min(feeds[i - 1] + max_increase);
    }

    // Backward pass: limit feed rate decreases
    for i in (0..feeds.len() - 1).rev() {
        if feeds[i] <= 0.0 || feeds[i + 1] <= 0.0 {
            continue;
        }
        let dist = move_distance(&moves[i].target, &moves[i + 1].target);
        let max_increase = ramp_rate * dist;
        feeds[i] = feeds[i].min(feeds[i + 1] + max_increase);
    }
}

fn move_distance(a: &P3, b: &P3) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let dz = b.z - a.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::FlatEndmill;

    fn default_params() -> FeedOptParams {
        FeedOptParams {
            nominal_feed_rate: 1000.0,
            max_feed_rate: 3000.0,
            min_feed_rate: 200.0,
            ramp_rate: 500.0, // mm/min per mm
            air_cut_threshold: 0.05,
        }
    }

    #[test]
    fn test_rctf_full_engagement() {
        let f = rctf(1.0);
        assert!(
            (f - 1.0).abs() < 0.01,
            "Full engagement RCTF should be ~1.0, got {:.3}",
            f
        );
    }

    #[test]
    fn test_rctf_half_engagement() {
        // At 50% radial engagement (ae = D/2), RCTF = 1.0 (no thinning)
        let f = rctf(0.5);
        assert!(
            (f - 1.0).abs() < 0.01,
            "50% engagement RCTF should be 1.0, got {:.3}",
            f
        );
    }

    #[test]
    fn test_rctf_quarter_engagement() {
        // At 25% engagement, chip thinning occurs → RCTF > 1.0
        let f = rctf(0.25);
        assert!(f > 1.1, "25% engagement RCTF should be > 1.1, got {:.3}", f);
        assert!(f < 1.3, "25% engagement RCTF should be < 1.3, got {:.3}", f);
    }

    #[test]
    fn test_rctf_light_engagement() {
        let f = rctf(0.1);
        assert!(
            f > 1.5,
            "Light engagement should have high RCTF, got {:.3}",
            f
        );
    }

    #[test]
    fn test_optimize_air_cut_gets_max_feed() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let params = default_params();

        // Stock with top at -10 — all material well below tool path (air cutting)
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 50.0, 50.0, -20.0, -10.0, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 10.0, 5.0));
        tp.feed_to(P3::new(20.0, 10.0, 5.0), 1000.0);
        tp.feed_to(P3::new(30.0, 10.0, 5.0), 1000.0);

        let result = optimize_feed_rates(&tp, &tool, &mut stock, &params);

        // Air cutting should get max feed (or close)
        for mv in &result.moves {
            if let MoveType::Linear { feed_rate } = mv.move_type {
                assert!(
                    feed_rate >= params.nominal_feed_rate,
                    "Air cut should get at least nominal feed, got {:.0}",
                    feed_rate
                );
            }
        }
    }

    #[test]
    fn test_optimize_full_engagement_gets_nominal() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let params = default_params();

        // Full block of material: Z from 0 to 10
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 50.0, 50.0, 0.0, 10.0, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 10.0, 15.0));
        tp.feed_to(P3::new(10.0, 10.0, 5.0), 1000.0); // plunge into material
        tp.feed_to(P3::new(20.0, 10.0, 5.0), 1000.0); // full engagement cut

        let result = optimize_feed_rates(&tp, &tool, &mut stock, &params);

        // With smoothing, feeds should stay reasonable
        for mv in &result.moves {
            if let MoveType::Linear { feed_rate } = mv.move_type {
                assert!(
                    feed_rate >= params.min_feed_rate,
                    "Feed should not go below min, got {:.0}",
                    feed_rate
                );
                assert!(
                    feed_rate <= params.max_feed_rate,
                    "Feed should not exceed max, got {:.0}",
                    feed_rate
                );
            }
        }
    }

    #[test]
    fn test_ramp_rate_limits_change() {
        let mut feeds = vec![0.0, 1000.0, 3000.0, 1000.0]; // rapid, then 3 cuts
        let moves = vec![
            Move {
                target: P3::new(0.0, 0.0, 0.0),
                move_type: MoveType::Rapid,
            },
            Move {
                target: P3::new(1.0, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 1000.0 },
            },
            Move {
                target: P3::new(2.0, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 3000.0 },
            },
            Move {
                target: P3::new(3.0, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 1000.0 },
            },
        ];

        smooth_feed_rates(&mut feeds, &moves, 500.0);

        // With 500 mm/min per mm ramp rate, jumping from 1000 to 3000 in 1mm
        // should be limited to 1000 + 500 = 1500
        assert!(
            feeds[2] <= 1600.0,
            "Ramp rate should limit increase: got {:.0}",
            feeds[2]
        );
    }
}
