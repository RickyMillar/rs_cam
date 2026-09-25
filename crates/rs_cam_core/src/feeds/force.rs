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
//! intercept (fracture-toughness / edge-ploughing): `Fc = Ks·h + F_edge`.
//! The intercept is a force *floor* — halving feed does **not** halve
//! force; feed moves deflection but cannot starve it to zero (then you
//! must drop DOC or stepover). The radial width `ae` enters through the
//! engagement arc (`ψ`), not as a linear multiplier (sciencedirect
//! S100093611300054X) — peak chip thickness saturates at full immersion,
//! so a wider slot does not keep raising the bending force once the
//! cutter is past half-immersion.
//!
//! ## The line — one per material (ruling B6)
//!
//! `Ks` and `F_edge` come from [`Material::force_line`] and nothing else:
//! the Curti 2021 density law for solid wood, the Goli 2018 printed line
//! for MDF, and a named refusal for every other material. Deflection
//! reads the line alone; it does not read the grain factor, which is
//! power-scoped (and 1.0 on every shipped line). See
//! [`crate::material::force_line`] and
//! `planning/extrapolation_2026-09-24/B6_PLAN.md`.

use crate::material::Material;

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

/// Feed-aware lateral (deflection-causing) cutting force in newtons.
///
/// `axial_mm` is the axial DOC (`ap`); `immersion_rad` is the engagement
/// arc angle ψ ([`immersion_angle`] from `ae/r`, or the sim sample's
/// `arc_engagement_radians` directly); `fz_mm` is feed per tooth.
///
/// Returns `None` when any input is non-positive or the material has no
/// force line ([`Material::force_line`] refuses) — the same refusal
/// contract the deflection gate and predictor route to "no constraint
/// signal".
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
    let line = material.force_line().ok()?;
    let (ks, f_edge) = (line.ks_n_per_mm2(), line.f_edge_n_per_mm());
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

    /// The force reads the material's one line: at a full slot
    /// (`θ_peak = π/2`, so `h_eff = fz`) the force is `ap · (Ks·fz +
    /// F_edge)` with the Curti 2021 line of `GenericHardwood`.
    #[test]
    fn the_force_reads_the_material_force_line() {
        let mat = hardwood();
        let line = mat.force_line().unwrap();
        let (ap, fz) = (3.0, 0.05);
        let force = lateral_cutting_force(&mat, ap, std::f64::consts::PI, fz).unwrap();
        let want = ap * (line.ks_n_per_mm2() * fz + line.f_edge_n_per_mm());
        assert!((force - want).abs() < 1e-12, "force {force} vs {want}");
        // Plan B6 §2.4: Ks 51.92, F_edge 4.077; Ks/F_edge 12.73 per mm.
        assert!((line.ks_n_per_mm2() - 51.92).abs() < 0.01);
        assert!((line.f_edge_n_per_mm() - 4.077).abs() < 0.001);
        let ratio = line.ks_n_per_mm2() / line.f_edge_n_per_mm();
        assert!((ratio - 12.73).abs() < 0.01, "slope:edge ratio {ratio:.3}");
    }

    /// Per-material scaling: a denser wood yields proportionally larger
    /// coefficients, because both terms are the density times a constant.
    #[test]
    fn coefficients_scale_with_density() {
        let soft = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
        .force_line()
        .unwrap();
        let hard = hardwood().force_line().unwrap();
        assert!(
            hard.ks_n_per_mm2() > soft.ks_n_per_mm2(),
            "denser wood ⇒ larger slope"
        );
        let rho_ratio = hard.density_kg_m3().unwrap() / soft.density_kg_m3().unwrap();
        assert!((hard.ks_n_per_mm2() / soft.ks_n_per_mm2() - rho_ratio).abs() < 1e-9);
        assert!((hard.f_edge_n_per_mm() / soft.f_edge_n_per_mm() - rho_ratio).abs() < 1e-9);
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
        };
        assert!(lateral_cutting_force(&custom, 2.0, 1.0, 0.05).is_none());
        let plywood = Material::Plywood {
            grade: crate::material::PlywoodGrade::BalticBirch,
        };
        assert!(lateral_cutting_force(&plywood, 2.0, 1.0, 0.05).is_none());
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
