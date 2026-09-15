//! Cut efficiency — the bounded typed answer to "what does this chipload
//! cost per mm³ of wood, and is it inside the vendor window?"
//!
//! Core computes the verdict; the UI renders it. The rule is
//! `crates/rs_cam_viz/CLAUDE.md`'s — *"GUI state is not an alternate data
//! model … do not recompute a narrower stale answer in the UI"* — and the
//! shape is `SimulationTriage`'s: one struct, every field either a number
//! the model stands behind or an explicit `None`.
//!
//! ## The model
//!
//! `feeds::force` fits the cutting force as **affine** in chip thickness,
//! `Fc/ap = Ks·h + F_edge`. Power is tangential force times cutting
//! velocity, so that split gives two terms of power:
//!
//! ```text
//! P  =  Ks · MRR                            ← shear: proportional to volume
//!    +  F_edge · ap · Vc · (z · ψ / 2π)     ← edge: proportional to DISTANCE
//! ```
//!
//! with `Vc = π·D·n` and ψ from [`force::immersion_angle`]. Dividing by
//! `MRR = ap · ae · fz · z · n` cancels both the axial DOC `ap` and the
//! spindle speed `n`:
//!
//! ```text
//! u  =  P / MRR  =  Ks  +  (F_edge · D · ψ) / (2 · ae · fz)   [J/mm³]
//! ```
//!
//! That cancellation is the point. `u` is an **efficiency** figure, not a
//! rate figure: RPM buys rate, depth buys rate, and neither buys
//! efficiency. Only the chipload and the radial engagement move `u`.
//! `tests/cut_efficiency_is_closed_form_g_specenergy.rs` pins both the
//! invariance and the sensitivity, because a later "simplification" that
//! reintroduces `n` or `ap` would turn this number back into a rate.
//!
//! `u` is a wear proxy. Energy that is not removing wood heats the edge,
//! so `ploughing_share = (u − Ks) / u` is the fraction of the cutting
//! power spent rubbing rather than cutting. Typical wood-routing
//! chiploads (0.03–0.09 mm/tooth) all sit below the crossover chip
//! thickness `F_edge/Ks ≈ 0.106 mm`, so that share is routinely 70–85 %.
//!
//! Derivation, measured fixture values and the reasoning behind each
//! recommendation: `planning/load_model_2026-09-16/ADVICE.md` §2.
//!
//! ## Absence is not zero
//!
//! Every field refuses independently, and a refusal is `None`. A material
//! with no primary-source `Kc` has no force model at all, so
//! [`cut_efficiency`] itself returns `None` — the same refusal the engine
//! already makes through `UnmodeledReason::MaterialUnvalidated`. No band
//! means no ratios. A deflection model that refuses means no ceiling and
//! no headroom. **Render a `None` as an abstention, never as a zero.**

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::quantities::{AdvancePerToothMm, ChiploadBandClass, VendorChiploadBand};
use crate::feeds::{ChiploadBounds, FeedsResult, effective_rubbing_floor, force};
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::tool_load::deflection::EXCEEDS_BOUND_MM;

/// Where the operating chipload sits against the matched vendor band.
///
/// The three in-band classes of [`ChiploadBandClass`] collapse to
/// [`ChipVerdict::InBand`]; this type answers "is the cut thin, right or
/// heavy", not "how close to which edge". The band itself travels beside
/// the verdict on [`CutEfficiency::band`] for a caller that needs the
/// finer question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipVerdict {
    /// Below the band. The chip is thinner than the vendor window, so
    /// more of the cutting power is ploughing — burn and wear risk.
    Thin,
    /// Inside the band.
    InBand,
    /// Above the band ceiling. Tooth force and breakage risk.
    Heavy,
    /// No vendor band was matched, so there is nothing to be inside of.
    /// `u` and the ploughing share still stand; the ratios do not.
    NoBand,
}

/// One bounded answer about the efficiency of a cut.
///
/// Built by [`cut_efficiency`]. Each `Option` is `None` when its own
/// input is missing, independently of the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutEfficiency {
    /// The operating point's commanded advance per tooth (mm),
    /// `feed / (rpm · flutes)` — `COMMANDED_ADVANCE_PER_TOOTH` in
    /// [`crate::feeds::quantities`], the unit every vendor chipload
    /// column is published in.
    pub advance_per_tooth_mm: f64,
    /// `u = P / MRR`, in J/mm³. ADVICE.md §2. Independent of RPM and of
    /// the axial DOC — both cancel in the derivation.
    pub specific_energy_j_per_mm3: f64,
    /// `(u − Ks) / u` — the share of cutting power spent ploughing
    /// rather than shearing, 0..1.
    pub ploughing_share: f64,
    /// Where [`Self::advance_per_tooth_mm`] falls against
    /// [`Self::band`].
    pub verdict: ChipVerdict,
    /// The matched vendor chipload window, when one exists and carries
    /// usable bounds. Every ratio below is derived from this band, so a
    /// `None` here and a `None` ratio always agree.
    pub band: Option<ChiploadBounds>,
    /// The chip-formation floor this band implies —
    /// [`effective_rubbing_floor`]. Below it the cutter burnishes
    /// instead of cutting.
    pub rubbing_floor_mm: f64,
    /// The largest advance per tooth (mm) that keeps the predicted tip
    /// deflection inside `EXCEEDS_BOUND_MM`, from
    /// [`force::chipload_cap_for_deflection`]. `None` when the
    /// deflection model refuses: no stickout, no axial DOC, no lateral
    /// engagement, or an edge force already over the budget.
    pub deflection_ceiling_mm: Option<f64>,
    /// `u(fz) / u(band midpoint)` — how much more energy this cut spends
    /// per mm³ than the middle of the vendor window. `None` without a
    /// band.
    pub wear_ratio_vs_band_mid: Option<f64>,
    /// `fz_mid / fz` — time is inversely proportional to chipload at
    /// fixed RPM, DOC and WOC. `None` without a band.
    pub time_ratio_vs_band_mid: Option<f64>,
    /// `1 − predicted_δ / EXCEEDS_BOUND`. `None` when
    /// [`crate::feeds::predict::predict_peak_deflection_um`] refuses —
    /// a drill, a V-bit, no `Kc`, no DPP or no stickout, each of which
    /// the predictor reports as its "no constraint signal" zero.
    ///
    /// A **negative** value is a real finding, not an error: the
    /// predicted deflection is already past the bound. Do not clamp it
    /// to zero on the way to the screen.
    pub force_headroom: Option<f64>,
}

/// The efficiency answer for one operating point, or `None` when the
/// material carries no force model.
///
/// `result` is the operating point being described — the recommendation
/// the inspector renders, not the operation's stored parameters. The
/// advance per tooth, the axial DOC and the radial width all come from
/// it, so the verdict describes the numbers beside it on screen.
/// `operation` and `machine` reach the pre-simulation deflection
/// predictor for [`CutEfficiency::force_headroom`] alone.
///
/// Returns `None` when:
///
/// - the material has no primary-source `Kc`, so
///   [`force::affine_coefficients`] refuses;
/// - the tool diameter, the flute count or the RPM is not positive, so
///   there is no advance per tooth;
/// - the radial width of cut is not positive, so the edge term has no
///   denominator.
///
/// **A refusal is `None`, never a fabricated zero.** A zero `u` would
/// read as a perfectly efficient cut.
#[must_use]
pub fn cut_efficiency(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    result: &FeedsResult,
) -> Option<CutEfficiency> {
    let (ks_n_per_mm2, f_edge_n_per_mm) = force::affine_coefficients(material)?;

    let diameter_mm = tool.diameter;
    if !(diameter_mm.is_finite() && diameter_mm > 0.0) {
        return None;
    }
    let radial_width_mm = result.radial_width_mm;
    let axial_doc_mm = result.axial_depth_mm;

    let advance_per_tooth_mm = commanded_advance_per_tooth_mm(result, tool.flute_count)?;
    let immersion_rad = force::immersion_angle(radial_width_mm, diameter_mm / 2.0);

    let specific_energy_j_per_mm3 = specific_energy(
        ks_n_per_mm2,
        f_edge_n_per_mm,
        diameter_mm,
        radial_width_mm,
        immersion_rad,
        advance_per_tooth_mm,
    )?;
    // `u ≥ Ks > 0` by construction, so the share is in 0..1.
    let ploughing_share = (specific_energy_j_per_mm3 - ks_n_per_mm2) / specific_energy_j_per_mm3;

    // A band with a non-positive or inverted window classifies nothing.
    // Drop it here so the verdict, the ratios and the reported band
    // cannot disagree about whether a band exists.
    let band = result.chipload_bounds.filter(|b| {
        b.min_mm_per_tooth.is_finite()
            && b.max_mm_per_tooth.is_finite()
            && b.min_mm_per_tooth > 0.0
            && b.max_mm_per_tooth >= b.min_mm_per_tooth
    });

    let verdict = match band {
        None => ChipVerdict::NoBand,
        Some(b) => {
            let window = VendorChiploadBand::new(
                AdvancePerToothMm::new(b.min_mm_per_tooth),
                AdvancePerToothMm::new(b.max_mm_per_tooth),
            );
            match window.classify(AdvancePerToothMm::new(advance_per_tooth_mm)) {
                ChiploadBandClass::BelowBand => ChipVerdict::Thin,
                ChiploadBandClass::AboveBand => ChipVerdict::Heavy,
                ChiploadBandClass::JustAboveFloor
                | ChiploadBandClass::Within
                | ChiploadBandClass::NearCeiling => ChipVerdict::InBand,
            }
        }
    };

    // Both ratios are measured against the band midpoint — Suggest's own
    // `SuggestAggressiveness::Default` target, so "vs the middle of the
    // window" means the same thing here as it does there.
    let band_mid_mm = band.map(|b| (b.min_mm_per_tooth + b.max_mm_per_tooth) / 2.0);
    let wear_ratio_vs_band_mid = band_mid_mm
        .and_then(|mid| {
            specific_energy(
                ks_n_per_mm2,
                f_edge_n_per_mm,
                diameter_mm,
                radial_width_mm,
                immersion_rad,
                mid,
            )
        })
        .map(|u_mid| specific_energy_j_per_mm3 / u_mid)
        .filter(|r| r.is_finite());
    let time_ratio_vs_band_mid = band_mid_mm
        .map(|mid| mid / advance_per_tooth_mm)
        .filter(|r| r.is_finite());

    let deflection_ceiling_mm = deflection_chipload_ceiling_mm(
        tool,
        ks_n_per_mm2,
        f_edge_n_per_mm,
        axial_doc_mm,
        radial_width_mm / diameter_mm,
    );

    let force_headroom = force_headroom(operation, tool, material, machine);

    Some(CutEfficiency {
        advance_per_tooth_mm,
        specific_energy_j_per_mm3,
        ploughing_share,
        verdict,
        band,
        rubbing_floor_mm: effective_rubbing_floor(band),
        deflection_ceiling_mm,
        wear_ratio_vs_band_mid,
        time_ratio_vs_band_mid,
        force_headroom,
    })
}

/// `feed / (rpm · flutes)` for the operating point, or `None` when the
/// divisor or the feed is not positive.
///
/// This is [`crate::feeds::quantities::COMMANDED_ADVANCE_PER_TOOTH`].
/// The divisor is applied here rather than through
/// [`AdvancePerToothMm::from_commanded`] because `FeedsResult::rpm` is an
/// `f64` and that constructor takes a `u32`; rounding the spindle speed
/// to feed a type conversion would move the answer.
fn commanded_advance_per_tooth_mm(result: &FeedsResult, flute_count: u32) -> Option<f64> {
    let divisor = result.rpm * f64::from(flute_count);
    let feed = result.feed_rate_mm_min;
    if !(divisor.is_finite() && divisor > 0.0 && feed.is_finite() && feed > 0.0) {
        return None;
    }
    let fz = feed / divisor;
    (fz.is_finite() && fz > 0.0).then_some(fz)
}

/// `u = Ks + (F_edge · D · ψ) / (2 · ae · fz)` in J/mm³ — ADVICE.md §2.
///
/// Neither the spindle speed nor the axial DOC appears, because both
/// cancel against `MRR`. `None` for any input that leaves the closed
/// form undefined.
fn specific_energy(
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    diameter_mm: f64,
    radial_width_mm: f64,
    immersion_rad: f64,
    advance_per_tooth_mm: f64,
) -> Option<f64> {
    let inputs_modelled = ks_n_per_mm2.is_finite()
        && ks_n_per_mm2 > 0.0
        && f_edge_n_per_mm.is_finite()
        && diameter_mm.is_finite()
        && diameter_mm > 0.0
        && radial_width_mm.is_finite()
        && radial_width_mm > 0.0
        && immersion_rad.is_finite()
        && immersion_rad > 0.0
        && advance_per_tooth_mm.is_finite()
        && advance_per_tooth_mm > 0.0;
    if !inputs_modelled {
        return None;
    }
    let ploughing_term = (f_edge_n_per_mm * diameter_mm * immersion_rad)
        / (2.0 * radial_width_mm * advance_per_tooth_mm);
    let u = ks_n_per_mm2 + ploughing_term;
    (u.is_finite() && u > 0.0).then_some(u)
}

/// The advance-per-tooth ceiling (mm) that holds the predicted tip
/// deflection inside `EXCEEDS_BOUND_MM`.
///
/// The inputs are assembled exactly as `session::compute` assembles them
/// for the feed-modulation optimizer's `DeflectionLimitInputs`: the
/// affine coefficients, the integrated cantilever compliance at this
/// axial DOC, and the gate's own bound. One assembly, so the inspector
/// and the optimizer cannot disagree about the ceiling.
///
/// `None` for every refusal — see [`force::chipload_cap_for_deflection`].
fn deflection_chipload_ceiling_mm(
    tool: &ToolConfig,
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    axial_doc_mm: f64,
    radial_woc_fraction: f64,
) -> Option<f64> {
    if !(axial_doc_mm.is_finite() && axial_doc_mm > 0.0) {
        return None;
    }
    if !radial_woc_fraction.is_finite() {
        return None;
    }
    let tool_def = crate::compute::cutter::build_cutter(tool);
    let stickout = tool_def.stickout;
    let youngs = tool_def.tool_material.youngs_modulus_n_per_mm2();
    if !(stickout.is_finite() && stickout > 0.0 && youngs.is_finite() && youngs > 0.0) {
        return None;
    }
    let compliance = tool_def.tip_deflection_mm(1.0, axial_doc_mm, youngs);
    if !(compliance.is_finite() && compliance > 0.0) {
        return None;
    }
    force::chipload_cap_for_deflection(
        ks_n_per_mm2,
        f_edge_n_per_mm,
        compliance,
        EXCEEDS_BOUND_MM,
        axial_doc_mm,
        radial_woc_fraction,
    )
}

/// `1 − predicted_δ / EXCEEDS_BOUND`, or `None` when the predictor
/// refuses.
///
/// [`crate::feeds::predict::predict_peak_deflection_um`] reports every
/// refusal as `0.0 µm` — "no constraint signal", its documented
/// contract. A zero therefore carries no headroom claim, and this
/// function turns it back into the abstention it is rather than
/// publishing a headroom of 100 %.
fn force_headroom(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> Option<f64> {
    let predicted_um =
        crate::feeds::predict::predict_peak_deflection_um(operation, tool, material, machine)
            .predicted_um;
    if !(predicted_um.is_finite() && predicted_um > 0.0) {
        return None;
    }
    let headroom = 1.0 - (predicted_um / 1000.0) / EXCEEDS_BOUND_MM;
    headroom.is_finite().then_some(headroom)
}
