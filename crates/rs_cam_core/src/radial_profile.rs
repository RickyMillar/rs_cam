//! Precomputed radial profile lookup table for milling cutters.

use crate::tool::MillingCutter;

/// Standard sample count for [`RadialProfileLUT::from_cutter`].
///
/// The LUT samples uniformly in dist² over the FULL cutter radius, so a
/// small-tip tool on a wide shank concentrates almost no samples on the
/// tip: a Ø1 tip on a Ø6 shank puts ~7 of 256 bins on the tip sphere, and
/// linear interpolation of the convex ball cap then OVERSHOOTS by up to
/// ~18 µm at the ball-side contact distance for 70–75° slopes (P2.g
/// probe, 2026-07-09) — the dexel sim under-cuts terrain-parallel finish
/// passes by that amount. 4096 samples bound the same error at < 0.1 µm
/// through 80° for that geometry (error scales with the SQUARE of bin
/// width). Cost: 32 KB per LUT, built once per simulated toolpath.
pub const LUT_SAMPLES: usize = 4096;

/// Precomputed radial profile lookup table for a cutter.
///
/// Indexes by dist_sq to avoid per-cell sqrt() calls. Bilinear interpolation
/// between samples gives sub-micron accuracy with 256+ samples for uniform
/// tools; use [`LUT_SAMPLES`] so small-tip tapered tools stay accurate too.
pub struct RadialProfileLUT {
    /// Height values indexed by dist_sq. Entry N corresponds to dist_sq = N / inv_step.
    heights: Vec<f64>,
    radius_sq: f64,
    inv_step: f64, // num_samples / radius_sq
}

impl RadialProfileLUT {
    /// Build a LUT from any MillingCutter.
    pub fn from_cutter(cutter: &dyn MillingCutter, num_samples: usize) -> Self {
        let r = cutter.radius();
        let r_sq = r * r;
        let inv_step = num_samples as f64 / r_sq;
        // num_samples + 2 to have room for interpolation at the boundary
        let mut heights = Vec::with_capacity(num_samples + 2);
        for i in 0..=num_samples {
            let dist_sq = i as f64 / inv_step;
            let dist = dist_sq.sqrt();
            match cutter.height_at_radius(dist) {
                Some(h) => heights.push(h),
                None => heights.push(f64::INFINITY),
            }
        }
        // Extra sentinel for interpolation past the last sample
        heights.push(f64::INFINITY);
        Self {
            heights,
            radius_sq: r_sq,
            inv_step,
        }
    }

    /// Look up the cutter height at a given dist_sq (no sqrt needed).
    /// Returns None if outside the cutter radius.
    #[inline]
    pub fn height_at_dist_sq(&self, dist_sq: f64) -> Option<f64> {
        if dist_sq > self.radius_sq {
            return None;
        }
        let idx_f = dist_sq * self.inv_step;
        let idx = idx_f as usize;
        let frac = idx_f - idx as f64;
        // SAFETY: dist_sq <= radius_sq guarantees idx+1 is within heights.len()
        #[allow(clippy::indexing_slicing)]
        let h0 = self.heights[idx];
        #[allow(clippy::indexing_slicing)]
        let h1 = self.heights[idx + 1];
        if h0 == f64::INFINITY {
            return None;
        }
        // Linearly interpolate; if h1 is INFINITY, just use h0 (at boundary)
        let h = if h1 == f64::INFINITY {
            h0
        } else {
            h0 + frac * (h1 - h0)
        };
        Some(h)
    }

    /// The squared radius of the cutter.
    #[inline]
    pub fn radius_sq(&self) -> f64 {
        self.radius_sq
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::tool::TaperedBallEndmill;

    /// P2.g sentry (2026-07-09): the dist²-uniform LUT must stay accurate
    /// on small-tip tapered tools. At 256 samples the Ø1-tip/Ø6-shank
    /// profile read up to ~18 µm HIGH at the ball-side contact distance
    /// for 70–75° slopes, which made the dexel sim under-cut terrain-
    /// parallel finish passes by the same amount (the false "unified loses
    /// the fine tier" matrix verdict). [`LUT_SAMPLES`] must hold the
    /// overshoot below 1 µm through 80° contact.
    #[test]
    fn tapered_tip_lut_error_bounded() {
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
        let tip_r: f64 = 0.5;
        let mut max_err_um = 0.0f64;
        // scan the whole tip sphere up to the 80°-slope contact distance
        let d_max = tip_r * 80.0f64.to_radians().sin();
        let mut d = 0.0;
        while d <= d_max {
            let exact =
                crate::tool::MillingCutter::height_at_radius(&cutter, d).expect("inside profile");
            let interp = lut.height_at_dist_sq(d * d).expect("inside profile");
            max_err_um = max_err_um.max((interp - exact).abs() * 1000.0);
            d += 0.0005;
        }
        assert!(
            max_err_um < 1.0,
            "tapered-tip LUT error {max_err_um:.2}um >= 1um — LUT_SAMPLES too coarse \
             for small-tip tools (dexel sim will under-cut steep finish passes)"
        );
    }
}
