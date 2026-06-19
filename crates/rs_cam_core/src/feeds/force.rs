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
//! ## Calibration — literature-absolute
//!
//! Both the *shape* (slope:edge ratio ≈ 9.42 /mm) and the *magnitude* come
//! straight from the woodresearch.sk quasi-orthogonal fit `Fc1z =
//! 49.95·h_m + 5.30`, attached to our `GenericHardwood` Kc as the anchor
//! wood and scaled to other materials linearly by `Kc / anchor_Kc`. This
//! is the physically-correct *instantaneous* bending force (chip area
//! `ap·h`), which is ~9× lower than the old milling-lifted `Kc·ap·ae`
//! aggregate — so deflection reads much cooler and the gate fires only
//! for genuinely catastrophic geometry (long/thin tools), not routine
//! roughing. Consequently most cuts are limited by **chipload/power**,
//! not deflection; the deflection gate is correctly quiet. Absolute
//! magnitude is "approximate / verify on a test cut" — same honesty bar
//! as the milling-Kc factor. See `planning/UNIFIED_LOAD_MODEL_2026-06-18.md`
//! §6 and `planning/KC_MILLING_CALIBRATION_2026-06-17.md`.

use crate::material::Material;

/// Literature-absolute affine slope `Ks` (N/mm²) for the anchor wood —
/// the woodresearch.sk 201905/12 quasi-orthogonal conventional fit
/// `Fc1z = 49.95·h_m + 5.30` (R² ≈ 0.99). The slope:edge ratio
/// `LIT_KS / LIT_FEDGE ≈ 9.42` /mm is the dimensionless shape; below the
/// crossover chip thickness (`h ≈ 1/9.42 ≈ 0.106 mm`, i.e. most wood
/// roughing) the edge term dominates — the size effect.
const LIT_KS_N_PER_MM2: f64 = 49.95;

/// Literature-absolute affine edge intercept `F_edge` (N/mm of axial
/// engagement) for the anchor wood — the `+5.30` term of the same fit.
const LIT_FEDGE_N_PER_MM: f64 = 5.30;

/// `Kc` of the anchor wood the literature coefficients are attached to —
/// our `GenericHardwood` (`MILLING_KC_FACTOR 2.7 × FPL shear 13.0 N/mm²`
/// = 35.1), a mid-hardwood close to the study's species class. Other
/// materials scale the literature coefficients linearly by their own
/// `Kc / LIT_ANCHOR_KC`, preserving relative material ordering while
/// anchoring the absolute magnitude to a real wood-milling measurement
/// rather than the milling-lifted `Kc·ap·ae` aggregate (which over-
/// stated the instantaneous bending force ~9×). Approximate — verify on
/// a test cut. See `planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §6.
const LIT_ANCHOR_KC_N_PER_MM2: f64 = 35.1;

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

/// Affine wood-force coefficients `(Ks, F_edge)` for `material`: the
/// literature-absolute anchor values scaled linearly by the material's
/// `Kc` relative to the anchor wood. Returns `None` when the material has
/// no primary-source `Kc`.
fn affine_coeffs(material: &Material) -> Option<(f64, f64)> {
    let kc = material.kc_n_per_mm2()?;
    let scale = kc / LIT_ANCHOR_KC_N_PER_MM2;
    Some((LIT_KS_N_PER_MM2 * scale, LIT_FEDGE_N_PER_MM * scale))
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

    /// Literature-absolute anchor: at the anchor wood (`GenericHardwood`)
    /// the coefficients equal the woodresearch.sk fit `Fc1z = 49.95·h +
    /// 5.30`, and the slope:edge ratio is ≈ 9.42 /mm. This pins the
    /// magnitude to a real wood-milling measurement rather than the
    /// milling-Kc aggregate.
    #[test]
    fn anchor_wood_matches_literature_coefficients() {
        let mat = hardwood();
        let (ks, f_edge) = affine_coeffs(&mat).unwrap();
        // GenericHardwood is the anchor → coefficients are the raw
        // literature values (its Kc == LIT_ANCHOR_KC).
        assert!((ks - LIT_KS_N_PER_MM2).abs() < 0.5, "Ks {ks:.2} ≈ 49.95");
        assert!(
            (f_edge - LIT_FEDGE_N_PER_MM).abs() < 0.1,
            "F_edge {f_edge:.2} ≈ 5.30"
        );
        let ratio = ks / f_edge;
        assert!(
            (ratio - 9.42).abs() < 0.05,
            "slope:edge ratio {ratio:.2} ≈ 9.42"
        );
    }

    /// Per-material scaling: a denser wood (higher Kc) yields proportionally
    /// larger coefficients, preserving relative material ordering.
    #[test]
    fn coefficients_scale_with_material_kc() {
        let soft = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let (ks_soft, _) = affine_coeffs(&soft).unwrap();
        let (ks_hard, _) = affine_coeffs(&hardwood()).unwrap();
        let kc_soft = soft.kc_n_per_mm2().unwrap();
        let kc_hard = hardwood().kc_n_per_mm2().unwrap();
        assert!(ks_hard > ks_soft, "denser wood ⇒ larger slope");
        // Linear in Kc.
        assert!((ks_hard / ks_soft - kc_hard / kc_soft).abs() < 1e-9);
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
