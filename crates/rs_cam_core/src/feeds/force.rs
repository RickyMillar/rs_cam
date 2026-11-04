//! The engagement arc and the refusals of the deflection feed cap.
//!
//! Feeds contract P11a (decision L5) moved the lateral force onto the K7
//! engagement: [`crate::feeds::engagement::Engagement::edge_force`] sums
//! the force line `dF = Ks h + F_edge` over the engaged edge with the
//! helix lag, and gives the mean and the peak resultant. Deflection reads
//! the peak; power reads the mean (through `PowerTerms`).
//! [`crate::feeds::engagement::Engagement::fz_cap_for_peak_force`] is the
//! feed cap of a force budget, with the refusals of this file.
//!
//! The one-tooth peak model `F = ap (Ks fz sin(min(psi, pi/2)) + F_edge)`
//! and its closed-form feed cap are deleted (operator ruling: no legacy).
//! It overstated a deep narrow cut about 4x and a slot 1.16x against the
//! mean engaged-edge force (`planning/feeds_contract_2026-11-03/research_ae/`).
//!
//! `Ks` and `F_edge` come from [`crate::material::Material::force_line`]
//! and nothing else (ruling B6). Deflection reads the line alone; the
//! grain factor is power-scoped (and 1.0 on every shipped line).

/// Engagement (immersion) arc angle ψ in radians for a radial width of
/// cut `ae_mm` on a cutter of radius `radius_mm`: `cos ψ = 1 − ae/r`,
/// clamped to `[0, π]`. Full slot (`ae = D`) gives `ψ = π`; half
/// immersion (`ae = r`) gives `ψ = π/2`.
///
/// Returns `0.0` for degenerate inputs so callers treat it as "no
/// engagement" (the force model then returns `None`).
pub fn immersion_angle(ae_mm: f64, radius_mm: f64) -> f64 {
    if !(ae_mm.is_finite() && radius_mm.is_finite()) || ae_mm <= 0.0 || radius_mm <= 0.0 {
        return 0.0;
    }
    let cos_psi = (1.0 - ae_mm / radius_mm).clamp(-1.0, 1.0);
    cos_psi.acos()
}

/// The reason the deflection feed cap gives no feed answer
/// ([`crate::feeds::engagement::Engagement::fz_cap_for_peak_force`]).
///
/// A solver needs the
/// distinction: a bound that **no** feed satisfies is a different
/// finding from a set of inputs that carries no model at all. The first
/// is a constraint the caller must report; the second is an abstention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflectionCapRefusal {
    /// No edge is engaged (the arc is zero), so the chip is not there to
    /// be thinned and feed does not move the bending force.
    /// The cap is undefined, not infinite. Do not read this as a pass.
    NoLateralEngagement,
    /// The feed-independent edge force alone already meets or exceeds the
    /// force budget. `F_edge` is a floor: the edge term stands at a feed
    /// of zero. So no feed rescues the deflection bound on this geometry.
    /// The fix is a smaller axial DOC or a smaller stepover.
    EdgeForceOverBudget,
    /// The inputs carry no usable model. The axial DOC, the budget, the
    /// helix or the affine slope `Ks` is zero, negative or not a number, or the
    /// solved cap is not a finite positive feed. The honest answer is an
    /// abstention, not a number.
    Unmodelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arc of a slot is pi, of half immersion pi/2, and a degenerate
    /// input gives no arc.
    #[test]
    fn the_arc_reads_ae_over_r() {
        assert!((immersion_angle(6.0, 3.0) - std::f64::consts::PI).abs() < 1e-12);
        assert!((immersion_angle(3.0, 3.0) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(immersion_angle(0.0, 3.0), 0.0);
        assert_eq!(immersion_angle(1.0, 0.0), 0.0);
    }
}
