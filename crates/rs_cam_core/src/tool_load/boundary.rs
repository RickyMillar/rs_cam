//! **The gate boundary contract — Checkpoint K (b1), 2026-08-13.**
//!
//! Every load gate in this crate decides `Within` vs `Exceeds` by
//! comparing an observation against a bound. Until this module existed,
//! each gate wrote that comparison itself as a bare `>` or `<`, and the
//! comparisons were **already in agreement**: A-6 measured all nine
//! shipped gate/side pairs and every one returns `Within` at exact
//! equality (`LUT_BOUNDARY_EVIDENCE.md` §3.5). The ledger's
//! "boundary semantics disagree" reading was wrong.
//!
//! What is genuinely wrong is **reconstruction**, and it has a name:
//! G-CHIP-ULP.
//!
//! # The defect this contract closes
//!
//! Suggest **multiplies** to build a feed (`feed = fpt × rpm × flutes`,
//! `feeds/mod.rs`); the chipload gate **divides** to recover an advance
//! per tooth (`observed = effective_feed / (rpm × flutes)`). In binary
//! floating point `(x·a)/a ≠ x`. On a recipe the engine's own
//! rubbing-floor clamp parked **exactly on the band ceiling** — routine
//! on sub-Ø2 tools, where the whole derated band sits below the 0.025
//! mm/tooth chip-formation floor and `effective_rubbing_floor` returns
//! the band *maximum* — the round trip lands 1 ulp above the bound in
//! **6–8 %** of the (rpm, flutes) grid, and the gate reports
//!
//! ```text
//! observed 0.011525  vs  max 0.011525  →  Exceeds(High)
//! ```
//!
//! a verdict decided by the rounding of a multiply/divide pair, printed
//! as an unexplainable exceedance.
//!
//! # The contract
//!
//! Bounds are **inclusive, with a relative epsilon**. A comparison trips
//! only when the observation clears its bound by more than
//! [`BOUNDARY_EPSILON_REL`] of that bound's own magnitude.
//!
//! - The epsilon is **relative**, because the quantities span
//!   millimetres per tooth (1e-2), kilowatts (1e0) and depth ratios
//!   (1e1); one absolute number cannot serve all three.
//! - It is **named and documented against a measurement**: the worst
//!   reconstruction error A-6's census found is **1 ulp**, so `8 ulp` is
//!   8× headroom and still ≈ 1.8e-15 relative — orders of magnitude
//!   below any physically meaningful difference in any gated quantity.
//!   A genuine exceedance of even 0.001 % is unaffected.
//! - It is **not** [`super::ToleranceBands`]. Those are the optimizer's
//!   dials, they default to zero, and widening one moves *every* verdict
//!   — option (b3), rejected at Checkpoint K. The epsilon here moves
//!   only verdicts that were decided by float noise. Both apply: the
//!   tolerance widens the bound, the epsilon absorbs the reconstruction.
//!
//! # Where it applies
//!
//! Everywhere. The three milling gates each carried a tolerance dial
//! defaulting to zero; the three drill gates carried **no dial at all**.
//! The drill gates do not exhibit the round-trip defect (their
//! observation is a single division), but "has no epsilon because
//! nothing has bitten yet" is not a contract, so they are inside this
//! one too.
//!
//! No gate writes a bare bound comparison any more. If you are adding a
//! gate, call [`exceeds_high`] / [`below_low`]; if you are adding a
//! *chipload* gate, call the methods on
//! [`super::verdict::ChipBounds`], which delegate here.

/// Relative slack applied to every gate bound comparison, as a multiple
/// of `f64::EPSILON` (i.e. of one ulp at magnitude 1).
///
/// **8 ulp**, chosen against a measurement rather than a feeling: the
/// worst multiply→divide reconstruction error in A-6's 1 164-point
/// census is exactly **1 ulp**, so this is 8× headroom. As a relative
/// figure it is ≈ 1.78e-15 — for a 0.0115 mm/tooth bound that is
/// 2e-17 mm/tooth, which is 12 orders of magnitude below the least
/// significant digit any surface prints.
///
/// **A relative epsilon is not a fixed ulp count, and the name is a
/// rounding of the truth.** For `x ∈ [2ᵉ, 2ᵉ⁺¹)` one ulp is `2ᵉ⁻⁵²`
/// while `x × f64::EPSILON ∈ [1 ulp, 2 ulp)`, so this slack is
/// **8–16 ulp** — 8 at the bottom of a binade, just under 16 at the
/// top. On the G-CHIP-ULP reference bound (0.011525…) it is ≈ 11.8 ulp.
/// Relative is still the right choice, because the gated quantities
/// span mm/tooth (1e-2), kW (1e0) and depth ratios (1e1) and no single
/// ulp count serves all three; the variation is recorded here so a
/// fixture probing "just past the slack" uses 20 ulp and not 9.
pub const BOUNDARY_EPSILON_REL: f64 = 8.0 * f64::EPSILON;

/// The slack, in the units of `bound`. Uses `bound.abs()` so a
/// (hypothetical) negative bound widens in the right direction.
#[must_use]
pub fn slack_for(bound: f64) -> f64 {
    bound.abs() * BOUNDARY_EPSILON_REL
}

/// **The high-side comparison.** `true` when `observed` is above
/// `bound`, widened by the gate's own `tolerance` (a *fraction*, e.g.
/// `ToleranceBands::breakage`) and then by the boundary epsilon.
///
/// At exact equality this is `false` — the pre-existing, already-uniform
/// semantics — and it stays `false` for a reconstruction that lands a
/// few ulp above.
///
/// Pass `tolerance = 0.0` for the gates that carry no dial.
#[must_use]
pub fn exceeds_high(observed: f64, bound: f64, tolerance: f64) -> bool {
    let trigger = bound * (1.0 + tolerance);
    observed > trigger + slack_for(trigger)
}

/// **The low-side comparison.** `true` when `observed` is below
/// `bound`, narrowed by the gate's own `tolerance` (e.g.
/// `ToleranceBands::burn`) and then by the boundary epsilon.
#[must_use]
pub fn below_low(observed: f64, bound: f64, tolerance: f64) -> bool {
    let trigger = bound * (1.0 - tolerance);
    observed < trigger - slack_for(trigger)
}

/// `true` when `observed` is indistinguishable from `bound` at the
/// boundary epsilon — i.e. the two differ by no more than the
/// reconstruction noise this contract absorbs.
///
/// This is the predicate Checkpoint K (c2) requires for the ceiling
/// advisory: it says "the recipe is *on* the bound", not "the recipe is
/// near the bound". Anything an operator would call *near* is orders of
/// magnitude outside it.
#[must_use]
pub fn is_at_bound(observed: f64, bound: f64) -> bool {
    (observed - bound).abs() <= slack_for(bound)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn exact_equality_is_within_on_both_sides() {
        assert!(!exceeds_high(0.05, 0.05, 0.0));
        assert!(!below_low(0.05, 0.05, 0.0));
    }

    #[test]
    fn one_ulp_of_reconstruction_noise_is_absorbed_on_both_sides() {
        let bound = 0.011_525_378_354_629_83_f64;
        let above = f64::from_bits(bound.to_bits() + 1);
        let below = f64::from_bits(bound.to_bits() - 1);
        assert!(
            !exceeds_high(above, bound, 0.0),
            "1 ulp above must not trip — that is G-CHIP-ULP"
        );
        assert!(!below_low(below, bound, 0.0), "1 ulp below must not trip");
        assert!(is_at_bound(above, bound));
        assert!(is_at_bound(below, bound));
    }

    #[test]
    fn a_genuine_exceedance_still_trips_and_is_not_at_the_bound() {
        let bound = 0.011_525_378_354_629_83_f64;
        // 5 % over — the case Checkpoint K (c2) must never demote.
        assert!(exceeds_high(bound * 1.05, bound, 0.0));
        assert!(!is_at_bound(bound * 1.05, bound));
        // …and so does a difference twelve orders of magnitude smaller
        // than 5 %, so the epsilon is not a tolerance in disguise.
        assert!(exceeds_high(bound * (1.0 + 1e-9), bound, 0.0));
        assert!(below_low(bound * (1.0 - 1e-9), bound, 0.0));
    }

    #[test]
    fn the_gate_tolerance_dial_still_composes_on_top() {
        let bound = 1.0_f64;
        assert!(exceeds_high(1.05, bound, 0.0));
        assert!(!exceeds_high(1.05, bound, 0.10));
        assert!(exceeds_high(1.15, bound, 0.10));
    }

    /// The epsilon is 8 ulp and no more — a regression that widened it
    /// into a tolerance would be caught here, not by a moved verdict
    /// somewhere downstream.
    #[test]
    fn the_epsilon_is_eight_ulp_relative() {
        assert!((BOUNDARY_EPSILON_REL / f64::EPSILON - 8.0).abs() < 1e-12);
        let bound = 1.0_f64;
        let nine_ulp = f64::from_bits(bound.to_bits() + 9);
        assert!(
            exceeds_high(nine_ulp, bound, 0.0),
            "9 ulp above 1.0 must still trip — the slack is bounded"
        );
    }
}
