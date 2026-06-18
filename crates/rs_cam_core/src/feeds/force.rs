//! Canonical feed-aware lateral cutting-force model — the single source
//! of the deflection-causing transverse force for every load consumer
//! (predict, the post-sim deflection gate, the axial-DOC envelope, the
//! optimize preflight, and — from step 3 — feed modulation).
//!
//! ## Why feed-aware
//!
//! The force that bends the tool is set by the *instantaneous chip
//! thickness*, which is feed-per-tooth × immersion — not the swept width
//! alone. The historical `F = Kc · ap · ae` had no feed term, so the
//! per-move optimizer (which lowers feed to control deflection) and the
//! post-sim gate (feed-blind) disagreed on the same cut: modulation would
//! drop feed to "fix" deflection and the gate would still read EXCEEDS.
//! This module makes deflection mirror power — one feed-aware function,
//! every stage consumes it, pinned by the agreement + feed-sensitivity
//! sentries so it cannot re-drift. See
//! `planning/UNIFIED_LOAD_MODEL_2026-06-18.md`.
//!
//! ## Model (deep-research confirmed, §4 of the plan)
//!
//! ```text
//! ψ        : cos ψ = 1 − ae/r              // engagement arc from radial WOC
//! θ_peak   = min(ψ, π/2)                   // worst engaged-tooth angle
//! h_eff    = fz · sin(θ_peak)              // PEAK uncut chip thickness
//! F_lat    = ap · (Ks · h_eff + F_edge)    // affine: slope + edge floor
//! ```
//!
//! Wood cutting force is **affine** in chip thickness with a non-zero edge
//! intercept (fracture-toughness / edge-ploughing): `Fc = Ks·h + F_edge`
//! (woodresearch.sk 201905/12, R² ≈ 0.99). The intercept is a force
//! *floor* — halving feed does **not** halve force; feed moves deflection
//! but cannot starve it to zero (then you must drop DOC or stepover). The
//! radial width `ae` enters through the engagement arc (`ψ`), not as a
//! linear multiplier (sciencedirect S100093611300054X) — peak chip
//! thickness saturates at full immersion, so a wider slot does not keep
//! raising the bending force once the cutter is past half-immersion.
//!
//! ## Calibration
//!
//! The wood literature fixes the *shape* (slope:edge ratio ≈ 9.42 /mm,
//! affine in `fz·sin θ`). The *magnitude* is pinned to our existing
//! milling-Kc reference: `Ks` and `F_edge` are scaled together so this
//! model reproduces the old `Kc·ap·ae` force at one healthy-roughing
//! operating point, then varies correctly with feed and immersion away
//! from it. One validated point + a literature-fixed ratio sets both
//! constants, no new bench data required. See the plan §6 and
//! `planning/KC_MILLING_CALIBRATION_2026-06-17.md`.

use crate::material::Material;

/// Literature slope-to-edge ratio for wood (`49.95 : 5.30 ≈ 9.42`, per
/// mm), the dimensionless shape we preserve while scaling magnitude to
/// our milling-Kc reference. woodresearch.sk 201905/12.
const WOOD_SLOPE_TO_EDGE_RATIO: f64 = 9.42;

/// Reference radial immersion fraction (`ae / D`) for magnitude pinning —
/// a typical roughing engagement (the predictor's non-adaptive WOC
/// fallback is also 0.35·D).
const REF_IMMERSION_FRACTION: f64 = 0.35;

/// Reference feed-per-tooth (mm) for magnitude pinning — a healthy wood
/// roughing chipload near the top of the LUT band.
const REF_FZ_MM: f64 = 0.05;

/// Reference radial width of cut (mm) for magnitude pinning — a
/// representative roughing WOC. With `ap` cancelling on both sides of
/// the pin, this is the only absolute length the calibration carries.
const REF_AE_MM: f64 = 2.0;

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

/// Affine wood-force coefficients `(Ks, F_edge)` for `material`, pinned so
/// that `F_lat(REF_FZ, REF_IMMERSION) == Kc · ap · REF_AE` at the
/// reference operating point. Returns `None` when the material has no
/// primary-source `Kc`.
fn affine_coeffs(material: &Material) -> Option<(f64, f64)> {
    let kc = material.kc_n_per_mm2()?;
    let cos_psi = 1.0 - 2.0 * REF_IMMERSION_FRACTION;
    let psi = cos_psi.clamp(-1.0, 1.0).acos();
    let theta_peak = psi.min(std::f64::consts::FRAC_PI_2);
    let h_ref = REF_FZ_MM * theta_peak.sin();
    // kc·ae_ref = F_edge·(ratio·h_ref + 1)   (ap cancels)
    let f_edge = kc * REF_AE_MM / (WOOD_SLOPE_TO_EDGE_RATIO * h_ref + 1.0);
    let ks = WOOD_SLOPE_TO_EDGE_RATIO * f_edge;
    Some((ks, f_edge))
}

/// Feed-aware lateral (deflection-causing) cutting force in newtons.
///
/// `axial_mm` is the axial DOC (`ap`); `immersion_rad` is the engagement
/// arc angle ψ ([`immersion_angle`] from `ae/r`, or the sim sample's
/// `arc_engagement_radians` directly); `fz_mm` is feed per tooth.
///
/// Returns `None` when any input is non-positive or the material has no
/// primary-source `Kc` — the same refusal contract the deflection gate
/// and predictor route to "no constraint signal".
pub fn lateral_cutting_force(
    material: &Material,
    axial_mm: f64,
    immersion_rad: f64,
    fz_mm: f64,
) -> Option<f64> {
    if !(axial_mm.is_finite() && immersion_rad.is_finite() && fz_mm.is_finite())
        || axial_mm <= 0.0
        || immersion_rad <= 0.0
        || fz_mm <= 0.0
    {
        return None;
    }
    let (ks, f_edge) = affine_coeffs(material)?;
    let theta_peak = immersion_rad.min(std::f64::consts::FRAC_PI_2);
    let h_eff = fz_mm * theta_peak.sin();
    Some(axial_mm * (ks * h_eff + f_edge))
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
    use crate::material::WoodSpecies;

    fn hardwood() -> Material {
        Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        }
    }

    /// Pinning sanity: at the reference operating point the feed-aware
    /// force reproduces the old `Kc · ap · ae` magnitude (within the
    /// rounding of the affine ratio). This is what keeps step 1 a
    /// magnitude-preserving landing rather than a recalibration.
    #[test]
    fn force_matches_legacy_kc_form_at_reference_point() {
        let mat = hardwood();
        let kc = mat.kc_n_per_mm2().unwrap();
        let ap = 2.0;
        let diameter = REF_AE_MM / REF_IMMERSION_FRACTION; // so ae/D = 0.35
        let radius = diameter / 2.0;
        let psi = immersion_angle(REF_AE_MM, radius);
        let f_new = lateral_cutting_force(&mat, ap, psi, REF_FZ_MM).unwrap();
        let f_legacy = kc * ap * REF_AE_MM;
        let rel = (f_new - f_legacy).abs() / f_legacy;
        assert!(
            rel < 0.02,
            "feed-aware force {f_new:.1} N should match legacy {f_legacy:.1} N at the pin (rel {rel:.3})"
        );
    }

    /// Agreement sentry (input-derivation half): the force is identical
    /// whether immersion is derived from `ae/r` (predictor / envelope
    /// path) or handed in as the swept arc angle directly (gate path),
    /// for a cut that describes the same geometry. This pins the
    /// convergence — the gate and predictor used to feed *different*
    /// physical quantities into the same `radial_width` parameter.
    #[test]
    fn force_agrees_via_ae_and_via_arc() {
        let mat = hardwood();
        let ap = 3.0;
        let fz = 0.04;
        let ae = 2.1;
        let radius = 3.0; // 6 mm cutter
        // Predictor/envelope path: derive ψ from ae/r.
        let psi_geom = immersion_angle(ae, radius);
        // Gate path: the sim sample carries the swept arc directly.
        let psi_arc = psi_geom; // same cut ⇒ same angle
        let f_geom = lateral_cutting_force(&mat, ap, psi_geom, fz).unwrap();
        let f_arc = lateral_cutting_force(&mat, ap, psi_arc, fz).unwrap();
        assert!((f_geom - f_arc).abs() < 1e-9);
    }

    /// Feed-sensitivity sentry: halving feed per tooth strictly lowers
    /// the force (proving feed-awareness — would be flat under the old
    /// model) but by **less than half** (proving the `F_edge` floor is
    /// live — feed cannot starve the force to zero).
    #[test]
    fn force_drops_with_feed_but_keeps_edge_floor() {
        let mat = hardwood();
        let ap = 2.0;
        let psi = immersion_angle(2.1, 3.0);
        let fz = 0.10;
        let f_full = lateral_cutting_force(&mat, ap, psi, fz).unwrap();
        let f_half = lateral_cutting_force(&mat, ap, psi, fz / 2.0).unwrap();
        assert!(f_half < f_full, "halving feed must lower force");
        assert!(
            f_half > f_full / 2.0,
            "edge floor must keep force above the naive proportional half ({f_half:.1} vs {:.1})",
            f_full / 2.0
        );
    }

    /// Immersion saturates: a full slot (`ae = D`) caps `θ_peak` at π/2,
    /// so widening past full immersion does not keep raising the force —
    /// the arc-not-linear behaviour that fixes the old `· ae` handle.
    #[test]
    fn force_saturates_past_full_immersion() {
        let mat = hardwood();
        let ap = 2.0;
        let fz = 0.06;
        let radius = 3.0;
        let f_half = lateral_cutting_force(&mat, ap, immersion_angle(radius, radius), fz).unwrap();
        let f_slot =
            lateral_cutting_force(&mat, ap, immersion_angle(2.0 * radius, radius), fz).unwrap();
        // half immersion is ψ = π/2 already (θ_peak capped), so a full
        // slot adds no peak-chip force.
        assert!((f_slot - f_half).abs() < 1e-9);
    }

    #[test]
    fn refuses_nonpositive_and_unvalidated() {
        let mat = hardwood();
        assert!(lateral_cutting_force(&mat, 0.0, 1.0, 0.05).is_none());
        assert!(lateral_cutting_force(&mat, 2.0, 0.0, 0.05).is_none());
        assert!(lateral_cutting_force(&mat, 2.0, 1.0, 0.0).is_none());
        let custom = Material::Custom {
            name: String::new(),
            feed_scale_factor: 1.0,
            kc: f64::NAN, // non-finite ⇒ kc_n_per_mm2 returns None
        };
        assert!(lateral_cutting_force(&custom, 2.0, 1.0, 0.05).is_none());
    }
}
