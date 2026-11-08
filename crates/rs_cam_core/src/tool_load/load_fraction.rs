//! The one cutting load `L` of a pass (INTEGRATION_PLAN D5, Q1; integration
//! step 7.1): the chip area a cut engages, as a fraction of a full pass.
//!
//! This is not the chipload gate's quantity (`chipload.rs`, advance per
//! tooth). `L` is a load on the cutter's side: 1 is one full pass, Depth/Pass
//! deep across the whole diameter. The planner, the simulation report and
//! the judges read it from [`chip_load_fraction`] only; do not write a
//! second form (the box and the integral forms of the adaptive research are
//! retired by this function).
//!
//! The area of a strip is `depth x pitch`. Until P20d it read the
//! cutter's section, the rectangle or the half rectangle of a V-bit; the
//! factor 1/2 is the same in the sum and in the full pass, so it cancels
//! (multiplication by 1/2 is exact), and the fraction is the same number
//! for every shape. The power model reads its own section
//! (`feeds::section::EngagedSection`).
//!
//! Source: `adaptive/slab_field.rs::strips_chip_load` of the research branch
//! (`planning/adaptive_research_2026-10-23/P3_C22_3d_turns.md` §18.6, §18.11,
//! the per-strip ruling C43).

/// The chip-area load of a cut: the lateral strips `strips`
/// (`(strip index, engaged depth mm)`, in index order, `pitch_mm` apart)
/// integrated across the strip centres by the trapezoid, over the area of
/// a full pass across the cutter, `depth_per_pass x diameter`.
///
/// Each pair of neighbouring strips adds `(mean of their depths) x pitch`; strips with a gap between them add nothing across
/// it. So a full-depth band of `n` strip centres reads `(n − 1) · pitch`
/// wide over the diameter: the extent between its outermost centres, the
/// quantity the flat swept-width read of the 2D planner measures
/// (`adaptive/search.rs`). A layer `t` thinner than a pass reads
/// `t / depth_per_pass` of that.
#[must_use]
pub fn chip_load_fraction(
    depth_per_pass: f64,
    diameter: f64,
    strips: impl IntoIterator<Item = (i64, f64)>,
    pitch_mm: f64,
) -> f64 {
    let mut area = 0.0_f64;
    let mut prev: Option<(i64, f64)> = None;
    for (index, depth) in strips {
        if let Some((p_index, p_depth)) = prev
            && index == p_index + 1
        {
            area += 0.5 * (p_depth + depth) * pitch_mm;
        }
        prev = Some((index, depth));
    }
    area / (depth_per_pass * diameter)
}

#[cfg(test)]
mod tests {
    use super::chip_load_fraction;
    /// §18.11 sentry of the one definition: at a full pass (every strip one
    /// Depth/Pass deep) the load of `n` strip centres a cell apart is the
    /// extent between the outermost centres over the diameter,
    /// `(n − 1) · cell / D`, bit for bit, for 1 to 13 centres (a slot of a
    /// 6 mm cutter on a 0.5 mm lattice).
    #[test]
    fn a_full_pass_reads_the_extent_over_the_diameter() {
        const D: f64 = 6.0;
        const CELL: f64 = 0.5;
        for dpp in [3.0, 6.0] {
            for n in 1..=13_i64 {
                let load = chip_load_fraction(dpp, D, (0..n).map(|i| (i, dpp)), CELL);
                let extent = (n - 1) as f64 * CELL / D;
                assert_eq!(
                    load.to_bits(),
                    extent.to_bits(),
                    "DPP {dpp}, {n} centres: load {load}, extent {extent}"
                );
            }
        }
    }

    /// §18.11: a layer `t` across the strips reads `t / DPP` of the
    /// full-pass load of the same strips; a gap between strips adds nothing
    /// across it.
    #[test]
    fn a_thin_layer_reads_its_depth_over_the_pass_and_a_gap_adds_nothing() {
        const D: f64 = 6.0;
        const CELL: f64 = 0.5;
        const DPP: f64 = 6.0;
        for t in [0.1, 0.5, 1.5, 3.0] {
            let full = chip_load_fraction(DPP, D, (0..9).map(|i| (i, DPP)), CELL);
            let thin = chip_load_fraction(DPP, D, (0..9).map(|i| (i, t)), CELL);
            assert!(
                (thin - full * t / DPP).abs() <= 1e-15,
                "layer {t}: {thin} against {}",
                full * t / DPP
            );
        }
        let split = chip_load_fraction(DPP, D, [(0, DPP), (1, DPP), (5, DPP), (6, DPP)], CELL);
        assert_eq!(
            split.to_bits(),
            (2.0 * CELL / D).to_bits(),
            "two bands of two centres"
        );
    }
}
