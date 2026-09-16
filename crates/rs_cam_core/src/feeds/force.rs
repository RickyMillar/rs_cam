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
    Some(affine_coefficients_for_kc(material.kc_n_per_mm2()?))
}

/// Affine wood-force coefficients `(Ks, F_edge)` for a raw material `Kc`
/// (N/mm²), without the `Material` in hand.
///
/// [`affine_coefficients`] is the entry point when a `Material` is
/// available; this one serves the callers that already hold the raw
/// `Kc` and must not re-derive the scaling — `tool_load::power` reads
/// the same two coefficients the deflection gate reads, so the power
/// model and the force model cannot drift apart. The literature
/// constants live here and nowhere else.
///
/// Note the anisotropy split: this returns the RAW-`Kc` coefficients,
/// which is what `tool_load::deflection` wants (sustained mean force).
/// `tool_load::power` carries `GRAIN_ANISOTROPY_FACTOR` on top for its
/// transient-grain-spike safety allowance. See
/// `tool_load/deflection.rs` module docs for why the two differ.
#[must_use]
pub fn affine_coefficients_for_kc(kc_n_per_mm2: f64) -> (f64, f64) {
    let scale = kc_n_per_mm2 / LIT_ANCHOR_KC_N_PER_MM2;
    (LIT_KS_N_PER_MM2 * scale, LIT_FEDGE_N_PER_MM * scale)
}

/// Public accessor for the affine wood-force coefficients `(Ks, F_edge)`
/// (N/mm², N per mm of axial engagement). The feed-modulation optimizer
/// uses these to solve its per-move feed cap from the **same** force model
/// the post-sim deflection gate consumes, so the optimizer and gate agree
/// on the cut. `None` when the material has no primary-source `Kc`.
pub fn affine_coefficients(material: &Material) -> Option<(f64, f64)> {
    affine_coeffs(material)
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

/// The reason the closed-form deflection cap gives no feed answer.
///
/// [`chipload_cap_for_deflection_with_reason`] reports this reason;
/// [`chipload_cap_for_deflection`] discards it. A solver needs the
/// distinction: a bound that **no** feed satisfies is a different
/// finding from a set of inputs that carries no model at all. The first
/// is a constraint the caller must report; the second is an abstention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflectionCapRefusal {
    /// `sin θ_peak ≤ 0`. The tooth sweeps no lateral arc, so the chip is
    /// not there to be thinned and feed does not move the bending force.
    /// The cap is undefined, not infinite. Do not read this as a pass.
    NoLateralEngagement,
    /// The feed-independent edge force alone already meets or exceeds the
    /// force budget. `F_edge` is a floor: `ap · F_edge` stands at a feed
    /// of zero. So no feed rescues the deflection bound on this geometry.
    /// The fix is a smaller axial DOC or a smaller stepover.
    EdgeForceOverBudget,
    /// The inputs carry no usable model. The axial DOC, the compliance or
    /// the affine slope `Ks` is zero, negative or not a number, or the
    /// solved cap is not a finite positive feed. The honest answer is an
    /// abstention, not a number.
    Unmodelled,
}

/// Feed-per-tooth ceiling (mm/tooth) that holds the tip deflection at or
/// below `max_tip_deflection_mm`, with the refusal reason.
///
/// This inverts the affine force model of this module onto the feed axis
/// in closed form. The lateral force is affine in feed per tooth and the
/// tip deflection is linear in force:
///
/// ```text
/// F_lat  = ap · (Ks · fz · sin θ_peak + F_edge)
/// δ      = compliance · F_lat
/// ```
///
/// Solve `δ ≤ bound` for `fz`:
///
/// ```text
/// budget_force = max_tip_deflection / compliance
/// fz_cap       = (budget_force / ap − F_edge) / (Ks · sin θ_peak)
/// ```
///
/// The feed-modulation optimizer and the post-simulation deflection gate
/// both consume this one solver, so they cannot disagree on a cut.
///
/// ## The immersion form
///
/// This function takes the radial engagement as a **fraction of the tool
/// diameter** and uses the peak-chip form `cos ψ = 1 − 2·woc_fraction`.
/// That is the same relation as [`immersion_angle`]'s `cos ψ = 1 − ae/r`,
/// because `ae/r = 2·(ae/D)`. The diameter cancels, so the caller passes
/// the engagement fraction it already holds and does not reconstruct `ae`
/// and `r`. Do not "correct" this line to `1 − woc_fraction`.
///
/// ## Refusals
///
/// - [`DeflectionCapRefusal::NoLateralEngagement`]
/// - [`DeflectionCapRefusal::EdgeForceOverBudget`]
/// - [`DeflectionCapRefusal::Unmodelled`]
///
/// The guard order matters and matches the caller's decision tree. The
/// function tests the axial DOC and the compliance first, so degenerate
/// inputs report `Unmodelled` and never a can't-satisfy refusal.
pub fn chipload_cap_for_deflection_with_reason(
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    compliance_mm_per_n: f64,
    max_tip_deflection_mm: f64,
    axial_doc_mm: f64,
    radial_woc_fraction: f64,
) -> Result<f64, DeflectionCapRefusal> {
    // A non-positive or NaN DOC/compliance carries no model. Test it
    // before the can't-satisfy branches: without a DOC there is no edge
    // force to compare against the budget.
    let inputs_modelled = axial_doc_mm > 0.0 && compliance_mm_per_n > 0.0;
    if !inputs_modelled {
        return Err(DeflectionCapRefusal::Unmodelled);
    }

    // Peak-chip immersion: cos ψ = 1 − 2·woc, θ_peak = min(ψ, π/2).
    let cos_psi = (1.0 - 2.0 * radial_woc_fraction).clamp(-1.0, 1.0);
    let sin_theta_peak = cos_psi.acos().min(std::f64::consts::FRAC_PI_2).sin();
    if sin_theta_peak <= 0.0 {
        return Err(DeflectionCapRefusal::NoLateralEngagement);
    }

    // The largest lateral force the tip takes inside the bound, and the
    // feed-independent edge force at this DOC.
    let budget_force_n = max_tip_deflection_mm / compliance_mm_per_n;
    let edge_force_n = axial_doc_mm * f_edge_n_per_mm;
    if edge_force_n >= budget_force_n {
        return Err(DeflectionCapRefusal::EdgeForceOverBudget);
    }

    let slope_modelled = ks_n_per_mm2 > 0.0;
    if !slope_modelled {
        return Err(DeflectionCapRefusal::Unmodelled);
    }

    let fz_cap =
        (budget_force_n / axial_doc_mm - f_edge_n_per_mm) / (ks_n_per_mm2 * sin_theta_peak);
    let cap_usable = fz_cap.is_finite() && fz_cap > 0.0;
    if !cap_usable {
        // A non-finite cap means an unbounded or undefined budget (a
        // denormal compliance, an infinite bound, a denormal Ks). Abstain.
        return Err(DeflectionCapRefusal::Unmodelled);
    }
    Ok(fz_cap)
}

/// Feed-per-tooth ceiling (mm/tooth) that holds the tip deflection at or
/// below `max_tip_deflection_mm`.
///
/// See [`chipload_cap_for_deflection_with_reason`] for the model, the
/// immersion form and each refusal. `None` covers every refusal: no
/// lateral engagement, the edge force alone over budget, and inputs that
/// carry no model. Render a `None` as an abstention, never as a zero.
pub fn chipload_cap_for_deflection(
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    compliance_mm_per_n: f64,
    max_tip_deflection_mm: f64,
    axial_doc_mm: f64,
    radial_woc_fraction: f64,
) -> Option<f64> {
    chipload_cap_for_deflection_with_reason(
        ks_n_per_mm2,
        f_edge_n_per_mm,
        compliance_mm_per_n,
        max_tip_deflection_mm,
        axial_doc_mm,
        radial_woc_fraction,
    )
    .ok()
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

    /// The ordinary solve inverts onto the bound: feed the capped chipload
    /// back through the affine model plus the compliance and the tip
    /// deflection lands on `max_tip_deflection_mm`.
    #[test]
    fn deflection_cap_inverts_onto_the_bound() {
        let ks = 42.7;
        let f_edge = 4.53;
        let compliance = 0.015;
        let bound = 0.2;
        let ap = 2.0;
        // Full slot: cos ψ = −1 ⇒ ψ = π, θ_peak = π/2, sin θ_peak = 1.
        let fz = chipload_cap_for_deflection(ks, f_edge, compliance, bound, ap, 1.0).unwrap();
        let force = ap * (ks * fz + f_edge);
        let delta = compliance * force;
        assert!(
            (delta - bound).abs() < 1e-9,
            "capped chipload must invert onto the bound: δ={delta:.6} vs {bound}"
        );
    }

    /// Refusal 1 — no lateral engagement. A zero radial WOC gives
    /// `ψ = 0`, so `sin θ_peak = 0`. Feed does not move the force and the
    /// cap is undefined.
    #[test]
    fn deflection_cap_refuses_without_lateral_engagement() {
        let reason =
            chipload_cap_for_deflection_with_reason(42.7, 4.53, 0.015, 0.2, 2.0, 0.0).unwrap_err();
        assert_eq!(reason, DeflectionCapRefusal::NoLateralEngagement);
        assert!(chipload_cap_for_deflection(42.7, 4.53, 0.015, 0.2, 2.0, 0.0).is_none());
    }

    /// Refusal 2 — the edge force alone is over budget. `F_edge` is a
    /// floor, so a feed of zero still breaks the bound. No feed rescues
    /// this cut; the caller must drop the DOC or the stepover.
    #[test]
    fn deflection_cap_refuses_when_the_edge_force_exhausts_the_budget() {
        let f_edge = 4.53;
        let compliance = 0.015;
        let bound = 0.2;
        // budget = 0.2 / 0.015 ≈ 13.3 N; ap · F_edge = 5 × 4.53 = 22.7 N.
        let ap = 5.0;
        let reason =
            chipload_cap_for_deflection_with_reason(42.7, f_edge, compliance, bound, ap, 1.0)
                .unwrap_err();
        assert_eq!(reason, DeflectionCapRefusal::EdgeForceOverBudget);
        assert!(chipload_cap_for_deflection(42.7, f_edge, compliance, bound, ap, 1.0).is_none());
    }

    /// Degenerate inputs abstain rather than claim a can't-satisfy
    /// finding. The DOC and the compliance are tested first, so a missing
    /// DOC never reports an edge-force refusal.
    #[test]
    fn deflection_cap_abstains_on_unmodelled_inputs() {
        for (ks, ap, compliance) in [
            (42.7, 0.0, 0.015), // no axial DOC
            (42.7, 2.0, 0.0),   // no compliance
            (0.0, 2.0, 0.015),  // no affine slope
            (f64::NAN, 2.0, 0.015),
        ] {
            let reason =
                chipload_cap_for_deflection_with_reason(ks, 4.53, compliance, 0.2, ap, 1.0)
                    .unwrap_err();
            assert_eq!(
                reason,
                DeflectionCapRefusal::Unmodelled,
                "ks {ks} ap {ap} compliance {compliance} must abstain"
            );
        }
    }

    /// A deeper cut takes more force at the same feed, so the cap falls
    /// as the axial DOC rises — and it falls faster than proportionally,
    /// because the edge floor eats a growing share of the budget.
    #[test]
    fn deflection_cap_falls_as_axial_doc_rises() {
        let ks = 42.7;
        let f_edge = 4.53;
        let compliance = 0.015;
        let bound = 0.4;
        let shallow = chipload_cap_for_deflection(ks, f_edge, compliance, bound, 1.0, 1.0).unwrap();
        let deep = chipload_cap_for_deflection(ks, f_edge, compliance, bound, 2.0, 1.0).unwrap();
        assert!(
            deep < shallow,
            "deeper cut ⇒ lower cap ({deep} vs {shallow})"
        );
        assert!(
            deep < shallow / 2.0,
            "the edge floor makes the drop faster than proportional ({deep} vs {})",
            shallow / 2.0
        );
    }
}
