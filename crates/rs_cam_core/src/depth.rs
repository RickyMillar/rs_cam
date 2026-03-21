//! Depth stepping for multi-pass 2.5D operations.
//!
//! Real CNC cuts can't remove full pocket depth in one pass — the cutter
//! would break or deflect. Depth stepping divides the total depth into
//! multiple passes, each within the tool's axial depth-of-cut limit.
//!
//! Supports even distribution (equal passes, best for tool life) and
//! constant stepping (max step + shallower final pass). Optional finish
//! allowance leaves material for a separate finish pass at exact depth.

use crate::toolpath::Toolpath;

/// How to distribute depth across passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepthDistribution {
    /// Distribute depth equally across passes. Better for consistent chip load
    /// and tool life. E.g., 10mm total at 3mm max → 4 passes of 2.5mm each.
    Even,
    /// Use max_step_down for all passes except the last, which may be shallower.
    /// E.g., 10mm total at 3mm max → passes of 3, 3, 3, 1mm.
    Constant,
}

/// Parameters controlling depth stepping for multi-pass operations.
#[derive(Debug, Clone)]
pub struct DepthStepping {
    /// Z height of material surface (usually 0.0).
    pub start_z: f64,
    /// Target Z depth (typically negative, e.g., -12.0).
    pub final_z: f64,
    /// Maximum depth per pass in mm (positive, e.g., 3.0).
    pub max_step_down: f64,
    /// How to distribute depth across passes.
    pub distribution: DepthDistribution,
    /// Material to leave at bottom for a finish pass (mm, >= 0).
    /// Roughing passes stop at `final_z + finish_allowance`.
    /// Set to 0.0 for no separate finish pass.
    pub finish_allowance: f64,
    /// Number of finishing (spring) passes to repeat at the final depth.
    /// Each spring pass repeats the last Z level for dimensional accuracy.
    /// 0 = no extra spring passes.
    pub finishing_passes: usize,
}

impl DepthStepping {
    /// Create with defaults: even distribution, no finish allowance.
    pub fn new(start_z: f64, final_z: f64, max_step_down: f64) -> Self {
        Self {
            start_z,
            final_z,
            max_step_down,
            distribution: DepthDistribution::Even,
            finish_allowance: 0.0,
            finishing_passes: 0,
        }
    }

    /// Total depth to cut (positive value).
    pub fn total_depth(&self) -> f64 {
        (self.start_z - self.final_z).max(0.0)
    }

    /// Number of roughing passes needed.
    pub fn roughing_pass_count(&self) -> usize {
        let rough_depth = self.roughing_depth();
        if rough_depth <= 0.0 || self.max_step_down <= 0.0 {
            return 0;
        }
        (rough_depth / self.max_step_down).ceil() as usize
    }

    /// Total roughing depth (total minus finish allowance).
    fn roughing_depth(&self) -> f64 {
        (self.total_depth() - self.finish_allowance).max(0.0)
    }

    /// Z of the roughing floor (may be above final_z if finish_allowance > 0).
    pub fn roughing_floor(&self) -> f64 {
        self.final_z + self.finish_allowance
    }

    /// Whether a separate finish pass is configured.
    pub fn has_finish_pass(&self) -> bool {
        self.finish_allowance > 0.0 && self.total_depth() > 0.0
    }

    /// Calculate Z levels for roughing passes (top to bottom).
    ///
    /// Each value is the Z height of that pass. The first pass is near
    /// start_z, the last is at roughing_floor().
    pub fn roughing_levels(&self) -> Vec<f64> {
        let rough_depth = self.roughing_depth();
        if rough_depth <= 0.0 || self.max_step_down <= 0.0 {
            return Vec::new();
        }

        let n = self.roughing_pass_count();
        if n == 0 {
            return Vec::new();
        }

        let floor = self.roughing_floor();

        match self.distribution {
            DepthDistribution::Even => {
                let step = rough_depth / n as f64;
                (1..=n).map(|i| self.start_z - step * i as f64).collect()
            }
            DepthDistribution::Constant => {
                let mut levels = Vec::with_capacity(n);
                for i in 1..=n {
                    let z = self.start_z - self.max_step_down * i as f64;
                    levels.push(z.max(floor));
                }
                levels
            }
        }
    }

    /// Calculate all Z levels: roughing + optional finish pass + spring passes.
    pub fn all_levels(&self) -> Vec<f64> {
        let mut levels = self.roughing_levels();
        if self.has_finish_pass() {
            levels.push(self.final_z);
        }
        // Add spring passes (repeat final Z for dimensional accuracy)
        let final_z = levels.last().copied().unwrap_or(self.final_z);
        for _ in 0..self.finishing_passes {
            levels.push(final_z);
        }
        levels
    }

    /// The finish pass Z level, if configured.
    pub fn finish_level(&self) -> Option<f64> {
        if self.has_finish_pass() {
            Some(self.final_z)
        } else {
            None
        }
    }

    /// Actual step size for each roughing pass.
    ///
    /// For Even: all steps are equal.
    /// For Constant: all steps are max_step_down except possibly the last.
    pub fn roughing_steps(&self) -> Vec<f64> {
        let levels = self.roughing_levels();
        if levels.is_empty() {
            return Vec::new();
        }

        let mut steps = Vec::with_capacity(levels.len());
        let mut prev_z = self.start_z;
        for &z in &levels {
            steps.push(prev_z - z);
            prev_z = z;
        }
        steps
    }
}

/// Generate a depth-stepped toolpath by applying a 2D operation at each Z level.
///
/// The `operation` closure receives the Z depth for each pass and returns
/// the toolpath for that single pass. This function sequences the passes
/// top-to-bottom with retracts between levels.
///
/// # Example
/// ```ignore
/// let depth = DepthStepping::new(0.0, -12.0, 3.0);
/// let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
///     pocket_toolpath(&polygon, &PocketParams { cut_depth: z, ..params })
/// });
/// ```
pub fn depth_stepped_toolpath<F>(depth: &DepthStepping, safe_z: f64, operation: F) -> Toolpath
where
    F: Fn(f64) -> Toolpath,
{
    let levels = depth.all_levels();
    combine_level_toolpaths(&levels, safe_z, &operation)
}

/// Like `depth_stepped_toolpath` but uses a different operation for the
/// finish pass (e.g., profile finish after pocket roughing).
///
/// `rough_op` is called for each roughing level. `finish_op` is called
/// for the final finish pass (if finish_allowance > 0).
pub fn depth_stepped_with_finish<R, F>(
    depth: &DepthStepping,
    safe_z: f64,
    rough_op: R,
    finish_op: F,
) -> Toolpath
where
    R: Fn(f64) -> Toolpath,
    F: Fn(f64) -> Toolpath,
{
    let mut tp = Toolpath::new();

    // Roughing passes
    let roughing = depth.roughing_levels();
    let rough_tp = combine_level_toolpaths(&roughing, safe_z, &rough_op);
    tp.moves.extend(rough_tp.moves);

    // Finish pass
    if let Some(finish_z) = depth.finish_level() {
        let finish_tp = finish_op(finish_z);
        if !finish_tp.moves.is_empty() {
            // Ensure retract before finish pass
            tp.final_retract(safe_z);
            tp.moves.extend(finish_tp.moves);
        }
    }

    tp
}

fn combine_level_toolpaths<F>(levels: &[f64], safe_z: f64, operation: &F) -> Toolpath
where
    F: Fn(f64) -> Toolpath,
{
    let mut tp = Toolpath::new();

    for (i, &z) in levels.iter().enumerate() {
        let level_tp = operation(z);
        if level_tp.moves.is_empty() {
            continue;
        }

        // Ensure retract between levels (not before first)
        if i > 0 {
            tp.final_retract(safe_z);
        }

        tp.moves.extend(level_tp.moves);
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pocket::{PocketParams, pocket_toolpath};
    use crate::polygon::Polygon2;
    use crate::profile::{ProfileParams, ProfileSide, profile_toolpath};
    use crate::toolpath::MoveType;
    use crate::zigzag::{ZigzagParams, zigzag_toolpath};

    // --- Z-level calculation tests ---

    #[test]
    fn test_even_distribution_exact() {
        // 12mm depth at 3mm max → 4 even passes of 3mm
        let ds = DepthStepping::new(0.0, -12.0, 3.0);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 4);
        assert!((levels[0] - -3.0).abs() < 1e-10);
        assert!((levels[1] - -6.0).abs() < 1e-10);
        assert!((levels[2] - -9.0).abs() < 1e-10);
        assert!((levels[3] - -12.0).abs() < 1e-10);
    }

    #[test]
    fn test_even_distribution_uneven() {
        // 10mm depth at 3mm max → 4 passes of 2.5mm each
        let ds = DepthStepping::new(0.0, -10.0, 3.0);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 4);
        assert!((levels[0] - -2.5).abs() < 1e-10);
        assert!((levels[1] - -5.0).abs() < 1e-10);
        assert!((levels[2] - -7.5).abs() < 1e-10);
        assert!((levels[3] - -10.0).abs() < 1e-10);

        // All steps should be equal
        let steps = ds.roughing_steps();
        for step in &steps {
            assert!((*step - 2.5).abs() < 1e-10);
        }
    }

    #[test]
    fn test_constant_distribution_uneven() {
        // 10mm depth at 3mm max → 3mm, 3mm, 3mm, 1mm
        let mut ds = DepthStepping::new(0.0, -10.0, 3.0);
        ds.distribution = DepthDistribution::Constant;
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 4);
        assert!((levels[0] - -3.0).abs() < 1e-10);
        assert!((levels[1] - -6.0).abs() < 1e-10);
        assert!((levels[2] - -9.0).abs() < 1e-10);
        assert!((levels[3] - -10.0).abs() < 1e-10);

        let steps = ds.roughing_steps();
        assert!((steps[0] - 3.0).abs() < 1e-10);
        assert!((steps[1] - 3.0).abs() < 1e-10);
        assert!((steps[2] - 3.0).abs() < 1e-10);
        assert!((steps[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_constant_distribution_exact() {
        // 9mm at 3mm → exactly 3 passes
        let mut ds = DepthStepping::new(0.0, -9.0, 3.0);
        ds.distribution = DepthDistribution::Constant;
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 3);
        assert!((levels[2] - -9.0).abs() < 1e-10);
    }

    #[test]
    fn test_shallow_cut_single_pass() {
        // 1mm depth at 3mm max → 1 pass
        let ds = DepthStepping::new(0.0, -1.0, 3.0);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 1);
        assert!((levels[0] - -1.0).abs() < 1e-10);
    }

    #[test]
    fn test_depth_equals_step() {
        // Exactly one pass
        let ds = DepthStepping::new(0.0, -3.0, 3.0);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 1);
        assert!((levels[0] - -3.0).abs() < 1e-10);
    }

    #[test]
    fn test_zero_depth() {
        let ds = DepthStepping::new(0.0, 0.0, 3.0);
        assert_eq!(ds.roughing_levels().len(), 0);
        assert_eq!(ds.total_depth(), 0.0);
    }

    #[test]
    fn test_inverted_depth_returns_empty() {
        // final_z above start_z → no cuts
        let ds = DepthStepping::new(0.0, 5.0, 3.0);
        assert_eq!(ds.roughing_levels().len(), 0);
    }

    #[test]
    fn test_nonzero_start_z() {
        // Start from Z=5 down to Z=-7 → 12mm total
        let ds = DepthStepping::new(5.0, -7.0, 4.0);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 3); // 12/4 = 3 passes
        assert!((levels[0] - 1.0).abs() < 1e-10);
        assert!((levels[1] - -3.0).abs() < 1e-10);
        assert!((levels[2] - -7.0).abs() < 1e-10);
    }

    #[test]
    fn test_very_small_step() {
        // Many passes
        let ds = DepthStepping::new(0.0, -10.0, 0.5);
        let levels = ds.roughing_levels();
        assert_eq!(levels.len(), 20);
        assert!((levels[19] - -10.0).abs() < 1e-10);
    }

    // --- Finish allowance tests ---

    #[test]
    fn test_finish_allowance() {
        // 12mm total, 0.5mm finish allowance
        let mut ds = DepthStepping::new(0.0, -12.0, 3.0);
        ds.finish_allowance = 0.5;

        assert!(ds.has_finish_pass());
        assert!((ds.roughing_floor() - -11.5).abs() < 1e-10);

        let roughing = ds.roughing_levels();
        // Roughing depth = 11.5mm, 11.5/3 → 4 passes
        assert_eq!(roughing.len(), 4);
        // Last roughing level should be at -11.5, not -12.0
        assert!((roughing.last().unwrap() - -11.5).abs() < 1e-10);

        assert_eq!(ds.finish_level(), Some(-12.0));

        let all = ds.all_levels();
        assert_eq!(all.len(), 5); // 4 roughing + 1 finish
        assert!((all.last().unwrap() - -12.0).abs() < 1e-10);
    }

    #[test]
    fn test_finish_allowance_exceeds_depth() {
        // Finish allowance >= total depth → no roughing, only finish
        let mut ds = DepthStepping::new(0.0, -2.0, 3.0);
        ds.finish_allowance = 3.0;

        assert_eq!(ds.roughing_levels().len(), 0);
        assert!(ds.has_finish_pass());
        let all = ds.all_levels();
        assert_eq!(all.len(), 1);
        assert!((all[0] - -2.0).abs() < 1e-10);
    }

    #[test]
    fn test_no_finish_allowance() {
        let ds = DepthStepping::new(0.0, -10.0, 3.0);
        assert!(!ds.has_finish_pass());
        assert_eq!(ds.finish_level(), None);
        assert_eq!(ds.roughing_levels().len(), ds.all_levels().len());
    }

    // --- Step calculation tests ---

    #[test]
    fn test_roughing_steps_sum_to_depth() {
        let ds = DepthStepping::new(0.0, -10.0, 3.0);
        let steps = ds.roughing_steps();
        let total: f64 = steps.iter().sum();
        assert!((total - 10.0).abs() < 1e-10, "Steps sum {} != 10.0", total);
    }

    #[test]
    fn test_roughing_steps_with_finish() {
        let mut ds = DepthStepping::new(0.0, -10.0, 3.0);
        ds.finish_allowance = 1.0;
        let steps = ds.roughing_steps();
        let total: f64 = steps.iter().sum();
        assert!(
            (total - 9.0).abs() < 1e-10,
            "Roughing steps sum {} != 9.0 (with 1mm finish allowance)",
            total
        );
    }

    #[test]
    fn test_constant_steps_never_exceed_max() {
        let mut ds = DepthStepping::new(0.0, -10.0, 3.0);
        ds.distribution = DepthDistribution::Constant;
        let steps = ds.roughing_steps();
        for (i, step) in steps.iter().enumerate() {
            assert!(
                *step <= ds.max_step_down + 1e-10,
                "Step {} = {} exceeds max {}",
                i,
                step,
                ds.max_step_down
            );
        }
    }

    #[test]
    fn test_even_steps_never_exceed_max() {
        let ds = DepthStepping::new(0.0, -10.0, 3.0);
        let steps = ds.roughing_steps();
        for (i, step) in steps.iter().enumerate() {
            assert!(
                *step <= ds.max_step_down + 1e-10,
                "Step {} = {} exceeds max {}",
                i,
                step,
                ds.max_step_down
            );
        }
    }

    #[test]
    fn test_last_level_is_exact() {
        // Verify final level lands exactly at target (no floating point drift)
        for depth in [7.0, 10.0, 13.0, 15.5, 100.0] {
            let ds = DepthStepping::new(0.0, -depth, 3.0);
            let levels = ds.roughing_levels();
            assert!(
                (levels.last().unwrap() - (-depth)).abs() < 1e-10,
                "Last level {} != target -{} for total depth {}",
                levels.last().unwrap(),
                depth,
                depth
            );
        }
    }

    // --- Integration with operations ---

    #[test]
    fn test_multipass_pocket() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let depth = DepthStepping::new(0.0, -9.0, 3.0);

        let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
            pocket_toolpath(
                &sq,
                &PocketParams {
                    tool_radius: 3.175,
                    stepover: 2.0,
                    cut_depth: z,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 10.0,
                    climb: false,
                },
            )
        });

        assert!(!tp.moves.is_empty());

        // Collect all unique Z levels of cutting moves
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| (m.target.z * 1000.0).round() / 1000.0) // round to 3 decimals
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        cut_zs.dedup();

        assert_eq!(cut_zs.len(), 3, "Expected 3 depth levels, got {:?}", cut_zs);
        assert!((cut_zs[0] - -3.0).abs() < 0.01);
        assert!((cut_zs[1] - -6.0).abs() < 0.01);
        assert!((cut_zs[2] - -9.0).abs() < 0.01);
    }

    #[test]
    fn test_multipass_profile() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let depth = DepthStepping::new(0.0, -6.0, 2.0);

        let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
            profile_toolpath(
                &sq,
                &ProfileParams {
                    tool_radius: 3.175,
                    side: ProfileSide::Outside,
                    cut_depth: z,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 10.0,
                    climb: false,
                },
            )
        });

        assert!(!tp.moves.is_empty());

        // Should have 3 depth levels: -2, -4, -6
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| (m.target.z * 100.0).round() / 100.0)
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        cut_zs.dedup();

        assert_eq!(cut_zs.len(), 3, "Expected 3 levels, got {:?}", cut_zs);
    }

    #[test]
    fn test_multipass_zigzag() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let depth = DepthStepping::new(0.0, -8.0, 4.0);

        let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
            zigzag_toolpath(
                &sq,
                &ZigzagParams {
                    tool_radius: 3.175,
                    stepover: 2.0,
                    cut_depth: z,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 10.0,
                    angle: 0.0,
                },
            )
        });

        assert!(!tp.moves.is_empty());

        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| (m.target.z * 100.0).round() / 100.0)
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        cut_zs.dedup();

        assert_eq!(cut_zs.len(), 2, "Expected 2 levels, got {:?}", cut_zs);
    }

    #[test]
    fn test_multipass_with_finish() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let mut depth = DepthStepping::new(0.0, -10.0, 3.0);
        depth.finish_allowance = 0.5;

        // Rough with pocket, finish with profile
        let tp = depth_stepped_with_finish(
            &depth,
            10.0,
            |z| {
                pocket_toolpath(
                    &sq,
                    &PocketParams {
                        tool_radius: 3.175,
                        stepover: 2.0,
                        cut_depth: z,
                        feed_rate: 1000.0,
                        plunge_rate: 500.0,
                        safe_z: 10.0,
                        climb: false,
                    },
                )
            },
            |z| {
                profile_toolpath(
                    &sq,
                    &ProfileParams {
                        tool_radius: 3.175,
                        side: ProfileSide::Inside,
                        cut_depth: z,
                        feed_rate: 800.0,
                        plunge_rate: 400.0,
                        safe_z: 10.0,
                        climb: true,
                    },
                )
            },
        );

        assert!(!tp.moves.is_empty());

        // Should have roughing levels + finish level
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| (m.target.z * 100.0).round() / 100.0)
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        cut_zs.dedup();

        // Should include -10.0 (the finish pass)
        assert!(
            cut_zs.iter().any(|z| (*z - -10.0).abs() < 0.1),
            "Should have finish pass at -10.0, got {:?}",
            cut_zs
        );
        // Should NOT have roughing at -10.0 (roughing stops at -9.5)
        // The -10.0 entries should only come from the finish operation
    }

    #[test]
    fn test_multipass_all_rapids_at_safe_z() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let depth = DepthStepping::new(0.0, -9.0, 3.0);

        let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
            pocket_toolpath(
                &sq,
                &PocketParams {
                    tool_radius: 3.175,
                    stepover: 2.0,
                    cut_depth: z,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 10.0,
                    climb: false,
                },
            )
        });

        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - 10.0).abs() < 1e-10,
                    "Rapid at z={}, expected safe_z=10.0",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn test_multipass_monotonic_depth() {
        // Verify that Z levels in the toolpath are non-increasing
        // (we never cut shallower after going deeper, except via rapids)
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let depth = DepthStepping::new(0.0, -12.0, 3.0);

        let tp = depth_stepped_toolpath(&depth, 10.0, |z| {
            pocket_toolpath(
                &sq,
                &PocketParams {
                    tool_radius: 3.175,
                    stepover: 2.0,
                    cut_depth: z,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 10.0,
                    climb: false,
                },
            )
        });

        // Track deepest cutting Z seen so far
        let mut deepest_cut_z = 0.0_f64;
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - 1000.0).abs() < 1e-10
            {
                // This is a cutting move
                assert!(
                    m.target.z <= deepest_cut_z + 0.01,
                    "Cutting at z={} after already reaching z={}",
                    m.target.z,
                    deepest_cut_z
                );
                deepest_cut_z = deepest_cut_z.min(m.target.z);
            }
        }
    }
}
