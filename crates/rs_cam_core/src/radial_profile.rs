//! Precomputed radial profile lookup table for milling cutters.

use crate::tool::MillingCutter;

/// Precomputed radial profile lookup table for a cutter.
///
/// Indexes by dist_sq to avoid per-cell sqrt() calls. Bilinear interpolation
/// between samples gives sub-micron accuracy with 256+ samples.
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
