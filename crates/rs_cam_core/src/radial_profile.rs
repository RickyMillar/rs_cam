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
    /// Every sampled height inside the cutter radius is finite, `>= 0`, and
    /// non-decreasing. See [`RadialProfileLUT::profile_is_nonneg_total`].
    nonneg_total: bool,
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
        // Whether this profile satisfies the two properties the dexel
        // air-skip (PERF_REVIEW S2) needs — measured on the table that will
        // actually be queried, rather than assumed from the cutter shape.
        // Sampled over `0..=num_samples`, which is every index
        // `height_at_dist_sq` can read `h0` from for `dist_sq <= radius_sq`.
        let mut nonneg_total = true;
        let mut prev = f64::NEG_INFINITY;
        for &h in heights.iter() {
            if !(h.is_finite() && h >= 0.0 && h >= prev) {
                nonneg_total = false;
                break;
            }
            prev = h;
        }
        // Extra sentinel for interpolation past the last sample
        heights.push(f64::INFINITY);
        Self {
            heights,
            radius_sq: r_sq,
            inv_step,
            nonneg_total,
        }
    }

    /// `true` when the profile is **total** (defined everywhere inside the
    /// cutter radius) and **non-negative and non-decreasing** in radius.
    ///
    /// This is the precondition the dexel simulator's air-skip rests on
    /// (`PERF_REVIEW.md` S2). The skip's argument is that a stamp whose
    /// *lowest* tip position already sits at or above the material top cannot
    /// remove anything, because the removal surface is `tip + h(d)` and
    /// `h(d) >= 0`. Three things have to hold for that to be an *exact*
    /// statement about this code rather than about the geometry:
    ///
    /// * **`h >= 0`** — otherwise the cutter surface can dip BELOW the tip and
    ///   a stamp the skip declared inert would have cut.
    /// * **non-decreasing** — the interpolation `h0 + frac·(h1 − h0)` is then
    ///   `>= h0 >= 0` in `f64` as well as over the reals. Without it, a
    ///   descending pair can round the interpolated value a few ULP below
    ///   `h1`, and "`h >= 0` at the samples" would not give "`h >= 0` at the
    ///   query".
    /// * **total** — [`Self::height_at_dist_sq`] returns `None` where the
    ///   table holds `INFINITY`, and the stamp kernels treat `None` as "this
    ///   cell contributes nothing at all", which is NOT the same as "this cell
    ///   is inert" once the skip has to reproduce the kernel's accumulator
    ///   arithmetic.
    ///
    /// Every cutter shape in [`crate::tool`] satisfies all three
    /// (`radial_profile_nonneg_total_holds_for_every_shipped_shape`); the flag
    /// exists so that one which does not simply turns the optimisation off
    /// instead of turning it wrong.
    #[inline]
    pub fn profile_is_nonneg_total(&self) -> bool {
        self.nonneg_total
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
    use crate::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, VBitEndmill};

    /// The S2 air-skip is gated on [`RadialProfileLUT::profile_is_nonneg_total`],
    /// and a gate that silently reads `false` everywhere would disable the
    /// optimisation without failing anything. This asserts the flag is
    /// actually TRUE for every cutter shape the crate ships, across a spread
    /// of sizes — including the small-tip tapered ball, whose profile is
    /// piecewise (ball then cone) and is the one plausible candidate for a
    /// float-noise dip at the junction.
    ///
    /// If a future shape legitimately fails this, the correct response is to
    /// exclude that shape here with a reason — **not** to weaken the flag,
    /// which is what makes the skip exact.
    #[test]
    fn radial_profile_nonneg_total_holds_for_every_shipped_shape() {
        let shapes: Vec<(&str, Box<dyn MillingCutter>)> = vec![
            ("flat_6", Box::new(FlatEndmill::new(6.0, 25.0))),
            ("flat_0p5", Box::new(FlatEndmill::new(0.5, 6.0))),
            ("ball_6", Box::new(BallEndmill::new(6.0, 25.0))),
            ("ball_1", Box::new(BallEndmill::new(1.0, 8.0))),
            (
                "bullnose_6r1",
                Box::new(BullNoseEndmill::new(6.0, 1.0, 25.0)),
            ),
            (
                "bullnose_12r3",
                Box::new(BullNoseEndmill::new(12.0, 3.0, 40.0)),
            ),
            ("vbit_60", Box::new(VBitEndmill::new(12.7, 60.0, 20.0))),
            ("vbit_90", Box::new(VBitEndmill::new(25.4, 90.0, 20.0))),
            (
                "tapered_ball_1_7deg",
                Box::new(TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)),
            ),
            (
                "tapered_ball_0p5_3deg",
                Box::new(TaperedBallEndmill::new(0.5, 3.0, 6.0, 30.0)),
            ),
        ];
        for (name, cutter) in shapes {
            let lut = RadialProfileLUT::from_cutter(cutter.as_ref(), LUT_SAMPLES);
            assert!(
                lut.profile_is_nonneg_total(),
                "{name}: profile_is_nonneg_total() is false — the S2 dexel \
                 air-skip silently disables itself for this shape"
            );
            // And the property the flag stands for, checked at the query
            // surface rather than at the samples.
            let mut prev = f64::NEG_INFINITY;
            for i in 0..=4096 {
                let d_sq = lut.radius_sq() * i as f64 / 4096.0;
                let h = lut
                    .height_at_dist_sq(d_sq)
                    .expect("total profile answers everywhere inside the radius");
                assert!(h >= 0.0, "{name}: h({d_sq}) = {h} < 0");
                assert!(h >= prev, "{name}: h decreased at d_sq={d_sq}");
                prev = h;
            }
        }
    }

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
