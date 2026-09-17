//! Closed-form deflection predictor for the Suggest path.
//!
//! [`predict_peak_deflection_um`] estimates the peak tip-deflection (in µm)
//! that Suggest would produce at a proposed `(operation, tool, material,
//! machine)` *without* running a simulation. It is the forward-callable
//! sibling of the post-simulation deflection gate in
//! [`crate::tool_load::deflection`] — same physics (cantilever-beam tip
//! displacement under transverse cutting force), pulled into a single
//! closed-form evaluation so the back-off loop scheduled for v1.1 step 2
//! can iterate cheaply.
//!
//! ## Physics
//!
//! `F = Kc · axial_doc · radial_woc` (N) — same form as the canonical
//! sample-level force in
//! [`crate::tool_load::deflection::sample_tip_deflection_mm`]. No
//! grain-anisotropy factor: deflection responds to sustained mean force,
//! not transient grain spikes (the 2.0× factor in
//! [`crate::tool_load::power::GRAIN_ANISOTROPY_FACTOR`] is power-scoped).
//!
//! The cantilever displacement under that force is **delegated to the
//! shared integrated model** [`tip_deflection_from_engagement`] →
//! [`crate::tool::ToolDefinition::tip_deflection_mm`] — the *same*
//! two-section (stiff shank above the flutes + cutter section below)
//! numerically-integrated beam the post-sim deflection gate and the
//! axial-DOC envelope ([`crate::feeds::cutter_constraints`]) already use.
//! The predictor exists to forecast what the post-sim gate will measure,
//! so it must use the gate's physics. (Before 2026-06-17 it used a bespoke
//! single-section `δ = F·a²·(3L−a)/(6·E·I)` with `I = π·(0.7·D)⁴/64`,
//! which applied the end-mill flute-relief factor to the *entire* stickout
//! — including the stiff shank — and read ~3× hotter than the gate. That
//! divergence made the Suggest back-off loop chase a phantom target and
//! exhaust its iteration cap; delegating fixes both.)
//!
//! `bending_diameter_mm` / `I_eff` are still computed and surfaced in the
//! [`DeflectionBreakdown`] as a representative-section *diagnostic*, but
//! no longer drive the deflection magnitude. A V-bit has no representative
//! section, so that diagnostic reads zero for one — the magnitude still
//! comes from the integrator, which models a V-bit through
//! `lookup_diameter_at`.
//!
//! ## Refusal (T-4, 2026-09-18)
//!
//! Until T-4 every refusal returned `predicted_um == 0.0`. A modelled zero
//! is unreachable, so that zero was always an absence written in a notation
//! that cannot say so. The refusal is now a type:
//! [`predict_peak_deflection_um`] returns
//! `Result<DeflectionPrediction, DeflectionUnmodeled>`, and the `Ok` branch
//! always carries a modelled figure.
//!
//! **Eleven exits refuse, and they map onto nine
//! [`DeflectionUnmodeled`] variants:**
//!
//! - Drill operation family — Z-only kinematics, no continuous engagement
//!   ([`DeflectionUnmodeled::NotApplicableForOp`]).
//! - Material has no primary-source `kc_n_per_mm2()` — out-of-band
//!   `SolidWoodByJanka`, fiberglass, nine of the ten shipped plastics
//!   ([`DeflectionUnmodeled::MaterialUnvalidated`]).
//! - `Material::Custom`, **including one carrying a positive `kc`** that
//!   the `Kc` guard above lets through. The delegate refuses every custom
//!   material ([`DeflectionUnmodeled::MaterialCustom`]).
//! - A tool diameter that is zero, negative or not a number
//!   ([`DeflectionUnmodeled::NoDiameter`]).
//! - No depth per pass, or one that is not positive and finite
//!   ([`DeflectionUnmodeled::NoDepthPerPass`]).
//! - A resolved radial width of cut that is not positive
//!   ([`DeflectionUnmodeled::NoRadialEngagement`]).
//! - Feed, RPM or flute count resolving to a zero chipload
//!   ([`DeflectionUnmodeled::NoChipload`]).
//! - A tool stickout that is not positive and finite
//!   ([`DeflectionUnmodeled::NoStickout`]).
//! - An engagement deeper than twice the stickout, which puts the load
//!   point at or below the tip ([`DeflectionUnmodeled::DegenerateCantilever`]).
//!
//! A V-bit is **no longer** a refusal. The predictor delegates to the
//! integrator, and the integrator models a V-bit. It returns an `Ok` that
//! carries [`DeflectionCaveat::FluteReliefUnmodeled`] — read the figure as
//! a floor, not a bound.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{CutterKind, OperationFamily as FeedsOperationFamily};
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::tool_load::verdict::UnmodeledReason;

/// Why the closed-form predictor produced no deflection figure.
///
/// Every variant maps onto the post-simulation refusal vocabulary in
/// [`UnmodeledReason`] through [`Self::as_unmodeled_reason`], so the
/// pre-simulation abstention and the post-simulation abstention print the
/// same words to the operator.
///
/// This is a pre-simulation type and it stays one: it carries no sample
/// range, no population and no confidence tier, because a closed form
/// evaluated at one operating point has none of those. See
/// `feeds/profile.rs` §Boundaries — the feeds layer must not shadow
/// `tool_load`'s taxonomy with a competing one, so it borrows the
/// vocabulary and keeps its own shape.
///
/// The shape follows [`crate::feeds::force::DeflectionCapRefusal`] in this
/// same folder. See `planning/TECH_DEBT_REGISTER.md` T-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflectionUnmodeled {
    /// Drill family — Z-only kinematics, no continuous radial engagement.
    NotApplicableForOp(OperationType),
    /// The material carries no primary-source `Kc`.
    MaterialUnvalidated,
    /// `Material::Custom`. The delegate refuses every custom material,
    /// including one carrying a positive `kc` — a case the early `Kc`
    /// guard lets through.
    MaterialCustom,
    /// The tool diameter is zero, negative or not a number.
    NoDiameter,
    /// The operation carries no depth per pass.
    NoDepthPerPass,
    /// The resolved radial width of cut is not positive.
    NoRadialEngagement,
    /// Feed, RPM or flute count resolved to a zero chipload.
    NoChipload,
    /// The tool reports no usable stickout.
    NoStickout,
    /// The engagement is deeper than the cantilever, so the load point
    /// sits at or below the tool tip.
    DegenerateCantilever,
}

impl DeflectionUnmodeled {
    /// The post-simulation refusal this pre-simulation abstention
    /// corresponds to. The operator reads one vocabulary, not two.
    pub fn as_unmodeled_reason(&self) -> UnmodeledReason {
        match self {
            Self::NotApplicableForOp(op) => UnmodeledReason::NotApplicableForOp(format!(
                "{} — no continuous radial engagement",
                op.spec().label
            )),
            Self::MaterialUnvalidated | Self::MaterialCustom => {
                UnmodeledReason::MaterialUnvalidated
            }
            Self::NoDiameter
            | Self::NoDepthPerPass
            | Self::NoRadialEngagement
            | Self::NoChipload
            | Self::NoStickout
            | Self::DegenerateCantilever => {
                UnmodeledReason::NotImplemented(self.clause().to_owned())
            }
        }
    }

    /// One operator-facing clause, identical on every surface, so the
    /// wording cannot drift between the GUI, the CLI and the MCP bridge.
    /// The same discipline `GatePopulation::vacuity_clause` applies to
    /// the X-VAC marker.
    pub fn clause(&self) -> &'static str {
        match self {
            Self::NotApplicableForOp(_) => "the deflection model does not apply to this operation",
            Self::MaterialUnvalidated => "this material has no measured cutting coefficient",
            Self::MaterialCustom => "a custom material carries no validated force model",
            Self::NoDiameter => "the tool has no usable diameter",
            Self::NoDepthPerPass => "the operation has no depth per pass",
            Self::NoRadialEngagement => "the operation has no radial width of cut",
            Self::NoChipload => "the feed, the speed or the flute count resolves to a zero chip",
            Self::NoStickout => "the tool reports no usable stickout",
            Self::DegenerateCantilever => "the cut is deeper than the tool stands out",
        }
    }
}

/// A modelled figure that is a floor rather than a bound.
///
/// A caveated [`DeflectionPrediction`] is a real number and a surface may
/// display it. It is **not** proof that a cut is inside a bound. A
/// consumer that compares `predicted_um` against a budget must treat a
/// caveated value the way it treats a [`DeflectionUnmodeled`]: not shown
/// safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflectionCaveat {
    /// A V-bit's cone is modelled as a solid section. Its flute relief
    /// is not. The real deflection is larger than the figure. This
    /// error is NON-conservative; every other gap in this model errs safe.
    FluteReliefUnmodeled,
}

impl DeflectionCaveat {
    /// One operator-facing clause, in the same style as
    /// [`DeflectionUnmodeled::clause`], so one surface words it once.
    pub fn clause(&self) -> &'static str {
        match self {
            Self::FluteReliefUnmodeled => {
                "a V-bit is modelled as a solid cone, so the real deflection is larger"
            }
        }
    }
}

/// The **equivalent bending diameter** of a fluted cutter, as a fraction of
/// its cutting diameter.
///
/// ## Equivalent diameter, not core diameter
///
/// These are two different numbers and the literature conflates them:
///
/// | | Published range | What it is |
/// |---|---|---|
/// | Core diameter | 0.47 – 0.85 D | the geometric root between the flutes |
/// | **Equivalent diameter** | 0.75 – 0.93 D | the solid shaft with the SAME bending compliance |
///
/// A cantilever model needs the second. Reaching for a core figure here gets
/// the magnitude wrong; that mistake was made once during the work that
/// produced this constant.
///
/// ## Source
///
/// Kops and Vo, *Determination of the Equivalent Diameter of an End Mill
/// Based on its Compliance*, Annals of the CIRP 39(1):93-96 (1990). They
/// report about **0.80 D** for both two- and four-flute tools, obtained from
/// COMPLIANCE — which is exactly the quantity a cantilever model needs.
///
/// Corroborated two ways, both in
/// `planning/load_model_2026-09-16/FLUTE_SECTION_MATH.md`:
///
/// - The inscribed-square bound. Flutes cut to the centre on both axes give
///   `m4 = 4/(3·pi) = 0.4244`, so `f = 0.807` — a geometric worst case that
///   the general section formula reproduces exactly.
/// - The section derivation. For any star-shaped section the
///   revolution-averaged `(d_eq/D)^4` is the circumferential mean of
///   `(rho/R)^4`. A real end-mill family has a core that RISES with flute
///   count and a land fraction that RISES with flute count, and those two
///   trends very nearly cancel — which is why a flat value is what a
///   compliance measurement finds.
///
/// ## Why there is no flute-count term
///
/// There was one: 2 -> 0.889, 3 -> 0.841, 4 -> 0.748, from Kivanc and Budak
/// (Sabanci MSc thesis 2004, Tables 3.1/3.2). The derivation reproduces those
/// three numbers, but **only** under the same "one flute shape, copied N
/// times" family the thesis models. Real end mills are not made that way, and
/// under a realistic family the trend REVERSES. The N-trend is an artefact of
/// that model, not a property of end mills, so it is not encoded here.
///
/// The two-flute figure had a second defect. A two-flute section is the only
/// one that is NOT isotropic: for three or more flutes the deviatoric part of
/// the inertia tensor vanishes by symmetry, but two flutes leave principal
/// values differing by 2.4x to 2.7x. The published 0.889 is the ARITHMETIC
/// mean of those two. A rotating tool under a fixed-direction load averages
/// COMPLIANCE, so the correct statistic is the harmonic mean, which gives
/// 0.84 — and 0.889 therefore under-predicts revolution-averaged deflection
/// by 20 % to 26 %.
///
/// ## Which shapes this applies to
///
/// Every fluted cutter except a V-bit. Derived in FLUTE_SECTION_MATH.md
/// section 5: a bull nose has an identical section above its corner radius; a
/// ball nose's ball region carries under 1 % of the compliance at L/D >= 3; a
/// tapered ball takes it at each local diameter. A V-bit's flute is not an
/// end-mill flute, no source gives a figure, and it stays unmodelled — see
/// [`bending_diameter_mm`], which marks that gap as non-conservative.
pub const ENDMILL_EQUIVALENT_DIAMETER_FRACTION: f64 = 0.80;

/// Radial WOC fallback fraction of tool diameter for non-Adaptive
/// families when `operation.stepover()` is `None`. Matches the
/// Pocket-roughing-equivalent stepover the calculator writes when no
/// explicit value is supplied.
const PREDICTOR_NON_ADAPTIVE_WOC_FRACTION: f64 = 0.35;

/// Radial WOC fraction of tool diameter for Adaptive 2D / 3D ops in
/// the move-count predictor. Matches the narrow-engagement design
/// point of the adaptive strategy (vs. the calculator's
/// `adaptive_woc_factor` which carries the per-machine rigidity
/// tuning).
const PREDICTOR_ADAPTIVE_WOC_FRACTION: f64 = 0.3;

/// Raster-pass sample density used by the move-count predictor: the
/// number of motion samples emitted per cutter footprint along the
/// fast scan axis is roughly `bbox_x / (diameter × 0.1)`, i.e. ~10
/// samples per diameter. Upper-bound-optimistic by design — over-
/// predicting is what makes the back-off trigger early.
const PREDICTOR_RASTER_SAMPLE_DENSITY: f64 = 0.1;

/// DPP fallback fraction of tool diameter for Adaptive 3D when
/// `operation.depth_per_pass()` is `None` in the move-count
/// predictor. Conservative middle of the rigidity envelope.
const PREDICTOR_ADAPTIVE3D_DPP_FRACTION: f64 = 0.5;

/// Tapered-ball-nose core-diameter weighting: 60% shank + 40% tip.
/// The bending stiffness integral above the ball junction is
/// dominated by the larger cone-shoulder section, so the weighting is
/// pulled toward the shank.
const PREDICTOR_TAPERED_BALL_SHANK_WEIGHT: f64 = 0.6;
const PREDICTOR_TAPERED_BALL_TIP_WEIGHT: f64 = 0.4;

/// Spindle RPM fallback when `operation.spindle_rpm()` is `None`.
/// Used by [`predict_peak_deflection_um`] (chipload diagnostic).
/// Matches the conservative "wood router default" the rest of the
/// codebase falls back to when RPM is unset.
///
/// (Until 2026-08-13 the retired `predict_observed_chipload_mm` was its
/// second consumer — see the retirement note further down this file.)
const PREDICTOR_FALLBACK_RPM: f64 = 18_000.0;

/// Closed-form prediction of peak tip deflection (µm) Suggest would
/// produce at the given (operation, tool, material, machine).
///
/// `Ok` always carries a modelled figure. Every refusal is an `Err` that
/// names itself — see [`DeflectionUnmodeled`] and the module docs. An
/// `Ok` whose `caveat` is `Some(_)` carries a modelled FLOOR: the figure
/// is real, and it does not show the cut inside a bound.
#[tracing::instrument(level = "debug", skip_all, fields(op = ?operation.op_type()))]
pub fn predict_peak_deflection_um(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> Result<DeflectionPrediction, DeflectionUnmodeled> {
    let op_type = operation.op_type();
    let feeds_family = op_type.spec().feeds_family;

    // Drill ops: Z-only kinematics, no continuous radial engagement —
    // the cantilever-deflection model doesn't apply. Mirrors the
    // `NotApplicableForOp` refusal in `tool_load::deflection::evaluate`.
    if feeds_family == FeedsOperationFamily::Drill || op_type == OperationType::Drill {
        tracing::debug!(reason = "drill_not_applicable", "predictor abstains");
        return Err(DeflectionUnmodeled::NotApplicableForOp(op_type));
    }

    // A V-bit used to refuse here. It no longer does. The guard's own
    // reason said "the post-sim integrator with `lookup_diameter_at`
    // handles V-bits" — and since `3b0dc487` this predictor CALLS that
    // integrator, so the guard refused before reaching the model that
    // works. `feeds::cutter_constraints::invert_deflection` has computed
    // a V-bit deflection bound from the same integrator throughout. The
    // remaining gap is the flute relief, not the cone, and it rides out
    // on `DeflectionCaveat::FluteReliefUnmodeled`. See T-4.

    // Material: only primary-source Kc materials get a numeric
    // prediction. Out-of-band SolidWoodByJanka, fiberglass and nine of
    // the ten shipped plastics return None — matches the
    // `MaterialUnvalidated` refusal in the post-sim gate. The force
    // magnitude itself is recomputed inside
    // `feeds::force::lateral_cutting_force`; here we only need the
    // existence check for the early refusal.
    if material.kc_n_per_mm2().is_none() {
        tracing::debug!(
            reason = "material_unvalidated",
            material = %material.label(),
            "predictor abstains — no primary-source Kc"
        );
        return Err(DeflectionUnmodeled::MaterialUnvalidated);
    }

    // `Material::Custom` refuses even when `kc_n_per_mm2()` returned
    // `Some` for a positive `kc`, because the delegate
    // `tip_deflection_from_engagement` refuses every custom material.
    // Before T-4 this exit hid inside the delegate's `None` and the
    // module header did not list it. State it here instead.
    if matches!(material, Material::Custom { .. }) {
        tracing::debug!(
            reason = "material_custom",
            "predictor abstains — a custom material carries no validated force model"
        );
        return Err(DeflectionUnmodeled::MaterialCustom);
    }

    // --- Engagement geometry ---
    let diameter_mm = tool.diameter;
    if !(diameter_mm.is_finite() && diameter_mm > 0.0) {
        return Err(DeflectionUnmodeled::NoDiameter);
    }

    let Some(axial_doc_mm) = operation.depth_per_pass() else {
        tracing::debug!(
            reason = "no_dpp",
            "predictor abstains — operation has no DPP"
        );
        return Err(DeflectionUnmodeled::NoDepthPerPass);
    };
    if !(axial_doc_mm.is_finite() && axial_doc_mm > 0.0) {
        return Err(DeflectionUnmodeled::NoDepthPerPass);
    }

    // Radial WOC: operation.stepover() if Some. For Adaptive ops with
    // no explicit stepover, fall back to `adaptive_woc_factor × D` —
    // the same target/ceiling `feeds::calculate` writes for adaptive
    // engagement (see `feeds/mod.rs:1342`). For non-adaptive families,
    // fall back to a Pocket-roughing-equivalent `0.35 × D`.
    let radial_woc_mm = match operation.stepover() {
        Some(s) if s.is_finite() && s > 0.0 => s,
        _ => {
            if feeds_family == FeedsOperationFamily::Adaptive {
                machine.rigidity.adaptive_woc_factor.max(0.0) * diameter_mm
            } else {
                PREDICTOR_NON_ADAPTIVE_WOC_FRACTION * diameter_mm
            }
        }
    };
    if !(radial_woc_mm.is_finite() && radial_woc_mm > 0.0) {
        return Err(DeflectionUnmodeled::NoRadialEngagement);
    }

    // Chipload — surfaced in the breakdown for the rationale tree the
    // back-off loop will render. Force F doesn't directly multiply by
    // chipload (the canonical Kc·ap·ae form is the same one the
    // post-sim sample-level force uses in
    // `tool_load::deflection::sample_tip_deflection_mm`); chipload is
    // a useful diagnostic when the back-off loop's DPP reduction needs
    // an honest "why" string.
    let rpm = operation
        .spindle_rpm()
        .map_or(PREDICTOR_FALLBACK_RPM, |r| r as f64);
    let flute_count = tool.flute_count.max(1) as f64;
    let feed_rate = operation.feed_rate();
    let chipload_per_tooth_mm = if rpm > 0.0 && flute_count > 0.0 && feed_rate.is_finite() {
        feed_rate / (rpm * flute_count)
    } else {
        0.0
    };

    // Zero feed → zero chipload → the calling Suggest path will have
    // already refused. Abstain here so the back-off loop doesn't see
    // a phantom deflection from an unconfigured op.
    if chipload_per_tooth_mm <= 0.0 {
        tracing::debug!(
            reason = "zero_chipload",
            "predictor abstains — feed/RPM/flutes resolved to zero chipload"
        );
        return Err(DeflectionUnmodeled::NoChipload);
    }

    // --- Cantilever geometry ---
    //
    // This reads `tool.stickout` and nothing else. `build_cutter` copies
    // that same field into the `ToolDefinition` the delegate bends, so a
    // fallback here would put a stickout in the breakdown that the model
    // never used. Until T-4 there was one — `cutting_length + 5 mm` — and
    // the refusal then came out of the delegate as a bare zero. One
    // guard, one number, one exit.
    let stickout_mm = tool.stickout;
    if !(stickout_mm.is_finite() && stickout_mm > 0.0) {
        tracing::debug!(
            reason = "no_stickout",
            "predictor abstains — the tool reports no usable stickout"
        );
        return Err(DeflectionUnmodeled::NoStickout);
    }

    // The load point sits at `stickout − axial/2` above the collet face.
    // An engagement deeper than twice the stickout puts it at or below
    // the tip and the cantilever has no length left to bend over. The
    // integrator returns 0.0 mm there — the sentinel T-4 removes — so
    // the predictor refuses before it calls.
    if stickout_mm - axial_doc_mm * 0.5 <= 0.0 {
        tracing::debug!(
            reason = "degenerate_cantilever",
            "predictor abstains — the cut is deeper than the tool stands out"
        );
        return Err(DeflectionUnmodeled::DegenerateCantilever);
    }

    // Effective core section — retained as a *breakdown diagnostic only*.
    // The deflection magnitude no longer comes from a bespoke single-
    // section formula here; it is delegated to the shared two-section
    // integrated cantilever (`tip_deflection_from_engagement`), the SAME
    // model the post-sim deflection gate (`sample_tip_deflection_mm`) and
    // the axial envelope (`invert_deflection`) use. The predictor's job
    // is to forecast what the gate will measure, so it must use the gate's
    // physics — the old uniform `I = π(0.7·D)⁴/64` applied the flute-relief
    // factor to the *whole* stickout (including the stiff shank) and read
    // ~3× hotter than the gate, which is why the back-off loop chased a
    // phantom target and exhausted its iteration cap.
    let d_core = bending_diameter_mm(tool);
    let i_eff_mm4 = if d_core.is_finite() && d_core > 0.0 {
        std::f64::consts::PI * d_core.powi(4) / 64.0
    } else {
        0.0
    };

    // --- Cutting force (N) ---
    // Feed-aware affine model: F = ap · (Ks · fz·sin θ_peak + F_edge),
    // with θ_peak from the engagement arc ψ = immersion_angle(ae, r).
    // The radial WOC enters through ψ (arc, not a linear width), and feed
    // enters through the chip thickness — so the predictor forecasts the
    // same force the post-sim gate measures. Surfaced in the breakdown
    // for the rationale tree; the integrated model recomputes it from the
    // same inputs, so the reported force can't drift from the one the
    // deflection used. No grain anisotropy factor here: static deflection
    // responds to mean force, not transient spikes.
    let immersion_rad = crate::feeds::force::immersion_angle(radial_woc_mm, diameter_mm / 2.0);
    let f_lateral_n = crate::feeds::force::lateral_cutting_force(
        material,
        axial_doc_mm,
        immersion_rad,
        chipload_per_tooth_mm,
    )
    .unwrap_or(0.0);

    // Delegate the cantilever to the canonical integrated model. The
    // guards above cover every documented refusal of the delegate, so the
    // arm below is a backstop: it keeps a non-positive or non-finite
    // figure out of the `Ok` branch, which must always carry a modelled
    // number. A figure of zero can only come from a load point at or
    // below the tip, which is the degenerate cantilever.
    let tool_def = crate::compute::cutter::build_cutter(tool);
    let predicted_um = match tip_deflection_from_engagement(
        &tool_def,
        material,
        axial_doc_mm,
        immersion_rad,
        chipload_per_tooth_mm,
    ) {
        Some(delta_mm) if delta_mm.is_finite() && delta_mm > 0.0 => delta_mm * 1000.0,
        _ => return Err(DeflectionUnmodeled::DegenerateCantilever),
    };

    // A V-bit's cone is modelled; its flute relief is not, and that gap
    // runs in the UNSAFE direction (see `bending_diameter_mm`). The
    // figure is a floor. Every other fluted shape takes the measured
    // equivalent-diameter fraction, so it carries no caveat. The match is
    // exhaustive: a 6th cutter shape fails to compile here rather than
    // inheriting a silence.
    let caveat = match tool.tool_type.cutter_kind() {
        CutterKind::VBit => Some(DeflectionCaveat::FluteReliefUnmodeled),
        CutterKind::Flat | CutterKind::Ball | CutterKind::Bull | CutterKind::TaperedBall => None,
    };

    tracing::debug!(
        predicted_um,
        f_lateral_n,
        stickout_mm,
        i_eff_mm4,
        axial_doc_mm,
        radial_woc_mm,
        caveat = ?caveat,
        "deflection prediction (delegated to integrated two-section cantilever)"
    );

    Ok(DeflectionPrediction {
        predicted_um,
        caveat,
        breakdown: DeflectionBreakdown {
            f_lateral_n,
            stickout_mm,
            i_eff_mm4,
            chipload_per_tooth_mm,
            axial_doc_mm,
            radial_woc_mm,
        },
    })
}

/// Effective bending-section diameter for the closed-form cantilever.
///
/// Every fluted shape takes [`ENDMILL_EQUIVALENT_DIAMETER_FRACTION`]. The
/// flute relief is a property of the CROSS-SECTION, so it does not care what
/// the end of the tool looks like. Derived per shape in
/// `planning/load_model_2026-09-16/FLUTE_SECTION_MATH.md` section 5:
///
/// - End mill: the section the fraction was measured on.
/// - Bull nose: identical section above the corner radius. The corner
///   contributes under 1e-4 of the compliance.
/// - Ball nose: the ball region carries under 1 % of the compliance at
///   L/D >= 3, so the fluted shaft above it sets the bending.
/// - Tapered ball nose: the fraction is dimensionless, so it applies at each
///   local diameter. Exact for a proportional grind and conservative for a
///   constant-depth grind. The tip/shank weighting below is unchanged; only
///   the section fraction is new.
/// - V-bit: returns 0. A cone has no one representative section, and this
///   value is a breakdown diagnostic only. The caller no longer refuses a
///   V-bit — it takes its magnitude from the integrator, which reads the
///   local diameter per step through `lookup_diameter_at`. Read the zero
///   as "no representative section", not as "no deflection".
///
/// **The V-bit gap is non-conservative.** A V-bit's flute is not an end-mill
/// flute, and no source gives an equivalent diameter for one. Treating it as
/// solid under-states deflection by `(1/f_V)^4` for an unknown `f_V`. It is
/// left unmodelled rather than guessed, because a fabricated constant in a
/// safety guard is worse than an absent one. T-4 makes the gap reach the
/// operator: the prediction carries
/// [`DeflectionCaveat::FluteReliefUnmodeled`], so the figure reads as a
/// floor rather than as a clean bound.
///
/// Routes on [`CutterKind`] (Phase 3) — the bending-section model is a
/// per-shape-class decision, so a 6th cutter shape fails to compile
/// here instead of inheriting a wrong section silently.
pub fn bending_diameter_mm(tool: &ToolConfig) -> f64 {
    match tool.tool_type.cutter_kind() {
        CutterKind::Flat | CutterKind::Ball | CutterKind::Bull => {
            ENDMILL_EQUIVALENT_DIAMETER_FRACTION * tool.diameter
        }
        CutterKind::TaperedBall => {
            let shank = tool.shaft_diameter.max(tool.diameter);
            // Weight 60% shank / 40% tip: the bending stiffness
            // integral above the ball junction is dominated by the
            // larger cone-shoulder section.
            (ENDMILL_EQUIVALENT_DIAMETER_FRACTION
                * (PREDICTOR_TAPERED_BALL_SHANK_WEIGHT * shank
                    + PREDICTOR_TAPERED_BALL_TIP_WEIGHT * tool.diameter))
                .max(0.5)
        }
        CutterKind::VBit => 0.0,
    }
}

/// Predicted tip deflection (mm) for an arbitrary engagement of `tool`
/// in `material`. Shared canonical force + cantilever model: both the
/// post-sim deflection gate
/// ([`crate::tool_load::deflection::sample_tip_deflection_mm`]) and the
/// pre-sim cutter-axial-constraints envelope
/// ([`crate::feeds::cutter_constraints`]) route through here so they
/// can't drift out of phase.
///
/// `axial_mm` is the axial DOC; `immersion_rad` is the engagement arc
/// angle ψ (the post-sim gate hands in the sample's
/// `arc_engagement_radians` directly; the predictor / envelope derive it
/// from `ae/r` via [`crate::feeds::force::immersion_angle`]); `fz_mm` is
/// feed per tooth. The force is the canonical feed-aware affine model in
/// [`crate::feeds::force::lateral_cutting_force`] — `F_lat = ap · (Ks ·
/// fz·sin θ_peak + F_edge)` — so every consumer reads the same physics
/// and feed genuinely moves deflection.
///
/// Returns `None` on the cases the gate would refuse:
/// - `material.kc_n_per_mm2()` is `None` (Custom, unvalidated species).
/// - Tool has zero stickout.
/// - Inputs are non-positive.
pub fn tip_deflection_from_engagement(
    tool: &crate::tool::ToolDefinition,
    material: &Material,
    axial_mm: f64,
    immersion_rad: f64,
    fz_mm: f64,
) -> Option<f64> {
    if axial_mm <= 0.0 || immersion_rad <= 0.0 || fz_mm <= 0.0 || tool.stickout <= 0.0 {
        return None;
    }
    if matches!(material, Material::Custom { .. }) {
        return None;
    }
    let force_n =
        crate::feeds::force::lateral_cutting_force(material, axial_mm, immersion_rad, fz_mm)?;
    let e = tool.tool_material.youngs_modulus_n_per_mm2();
    Some(tool.tip_deflection_mm(force_n, axial_mm, e))
}

/// Closed-form deflection prediction. `predicted_um` is ALWAYS a
/// modelled figure: the refusal cases return `Err` and name themselves.
///
/// Before T-4 this type carried the refusal as `predicted_um == 0.0`. A
/// rigid cut and an unmodelled cut read the same, and the Suggest
/// back-off treated the second as the first.
#[derive(Debug, Clone)]
pub struct DeflectionPrediction {
    pub predicted_um: f64,
    /// `Some(_)` when the figure is a modelled FLOOR rather than a bound.
    /// A surface may display a caveated figure. No consumer may read one
    /// as proof that the cut sits inside a budget — see
    /// [`DeflectionCaveat`].
    pub caveat: Option<DeflectionCaveat>,
    /// Inputs the prediction consumed — surfaced so the rationale tree
    /// the back-off loop renders can show *why* a given iteration's
    /// δ landed where it did.
    pub breakdown: DeflectionBreakdown,
}

/// v1.2 combined-Suggest: first-order estimate of the total number of
/// motion samples / cutting moves a toolpath would emit at the given
/// operating point. Used by [`crate::feeds::suggest::enforce_invariants`]
/// to detect catastrophic stepover values (the Wanaka 3D Finish 0.03 mm
/// failure mode — 4.6 M-move toolpath effectively blocks generation).
///
/// ## Formulas (upper-bound-optimistic by design)
///
/// The intent is "right within ~3× for the catastrophic case." The
/// predictor's bias is upper-bound-optimistic — it should over-predict
/// rather than under-predict so the back-off triggers earlier rather
/// than later. Matches the +36% safer-side bias the v1.1 deflection
/// predictor carries.
///
/// - **Parallel-raster ops** (DropCutter, Scallop, HorizontalFinish,
///   ZigZag, SpiralFinish, RadialFinish, Waterline, SteepShallow):
///   `passes = bbox.y / stepover`,
///   `moves_per_pass = bbox.x / (tool.diameter × 0.1)` (rough sample
///   density; ~10 samples per cutter footprint).
/// - **Adaptive (2D / 3D)**: `area_per_pass = stepover × adaptive_woc`
///   where `adaptive_woc = tool.diameter × 0.3` (adaptive's narrow
///   engagement). `passes_per_z = bbox_area / area_per_pass`. Total =
///   `passes_per_z × (stock_z / dpp)` — adaptive3d only, adaptive 2D
///   uses 1 Z level.
/// - **Pocket**: similar to adaptive but single Z level and full
///   `stepover × stepover` cell (no narrow-WOC discount).
/// - **Profile / V-carve / Project Curve / Drill / Trace / Face / etc.**:
///   `0` — these are feature-driven (polygon perimeter, curve segments,
///   hole positions); their move count doesn't scale with stepover ×
///   bbox in a useful way.
///
/// Returns `0` when `model_bbox` is `None`, the op is skipped, or any
/// input is non-finite / non-positive. The caller (back-off loop) treats
/// `0` as "no constraint, can't gate" and short-circuits.
pub fn predict_move_count(
    operation: &OperationConfig,
    model_bbox: Option<&crate::geo::BoundingBox3>,
    tool: &ToolConfig,
) -> u64 {
    let Some(bbox) = model_bbox else {
        return 0;
    };

    let dx = (bbox.max.x - bbox.min.x).max(0.0);
    let dy = (bbox.max.y - bbox.min.y).max(0.0);
    let dz = (bbox.max.z - bbox.min.z).max(0.0);
    if !(dx.is_finite() && dy.is_finite() && dx > 0.0 && dy > 0.0) {
        return 0;
    }

    let diameter = tool.diameter;
    if !(diameter.is_finite() && diameter > 0.0) {
        return 0;
    }

    let op_type = operation.op_type();

    // Feature-driven ops: move count is dominated by the input geometry
    // (curve length, polygon perimeter, hole count) rather than the
    // bbox × stepover envelope. Return 0 → caller skips gating.
    match op_type {
        OperationType::VCarve
        | OperationType::ProjectCurve
        | OperationType::Drill
        | OperationType::AlignmentPinDrill
        | OperationType::Trace
        | OperationType::Profile
        | OperationType::Chamfer
        | OperationType::Pencil
        | OperationType::Face
        | OperationType::RampFinish
        | OperationType::Inlay => return 0,
        _ => {}
    }

    // Resolve stepover. Most envelope-driven ops carry it; for the few
    // that don't (e.g. an Adaptive3d configured purely by scallop hint
    // before Suggest writes it back), bail to 0 rather than guess.
    let Some(stepover) = operation.stepover() else {
        return 0;
    };
    if !(stepover.is_finite() && stepover > 0.0) {
        return 0;
    }

    let moves_f = match op_type {
        // Parallel-raster family: passes along Y, samples along X.
        OperationType::DropCutter
        | OperationType::Scallop
        | OperationType::HorizontalFinish
        | OperationType::SteepShallow
        | OperationType::Waterline
        | OperationType::SpiralFinish
        | OperationType::RadialFinish
        | OperationType::Zigzag => {
            let passes = dy / stepover;
            let moves_per_pass = dx / (diameter * PREDICTOR_RASTER_SAMPLE_DENSITY);
            passes * moves_per_pass
        }
        // Adaptive 3D: per-Z-level area-fill × number of Z levels.
        OperationType::Adaptive3d => {
            let woc = diameter * PREDICTOR_ADAPTIVE_WOC_FRACTION;
            let area_per_pass = stepover * woc;
            if area_per_pass <= 0.0 {
                return 0;
            }
            let passes_per_z = (dx * dy) / area_per_pass;
            let dpp = operation
                .depth_per_pass()
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(diameter * PREDICTOR_ADAPTIVE3D_DPP_FRACTION);
            let z_levels = if dz > 0.0 { (dz / dpp).max(1.0) } else { 1.0 };
            passes_per_z * z_levels
        }
        // Adaptive 2D: single Z level worth of area-fill at narrow WOC.
        OperationType::Adaptive => {
            let woc = diameter * PREDICTOR_ADAPTIVE_WOC_FRACTION;
            let area_per_pass = stepover * woc;
            if area_per_pass <= 0.0 {
                return 0;
            }
            (dx * dy) / area_per_pass
        }
        // Pocket: bbox area / (stepover × stepover) — full cell at the
        // suggested stepover, no adaptive narrow-WOC discount.
        OperationType::Pocket | OperationType::Rest => {
            let cell = stepover * stepover;
            if cell <= 0.0 {
                return 0;
            }
            (dx * dy) / cell
        }
        // Skipped above, but kept here for the match's exhaustiveness
        // discipline (any new op type defaults to "no envelope gate").
        _ => return 0,
    };

    if !moves_f.is_finite() || moves_f <= 0.0 {
        return 0;
    }
    // Saturating cast to u64 — moves_f for catastrophic 0.03 mm cases
    // hits ~10⁶..10⁷, well under u64::MAX.
    moves_f.min(u64::MAX as f64) as u64
}

// # ⚰ RETIRED 2026-08-13 — the arc-fit observed-chipload prediction
//
// This module used to carry `predict_observed_chipload_mm`, its
// `ObservedChiploadPrediction` / `ArcFitRatioSource` types, and the
// per-operation-family `arc_fit_ratio_for_op` table (Adaptive3d 0.25,
// DropCutter 0.15 tagged `Calibrated`; fifteen other families a
// conservative `Default`). Every ratio in that table was fitted against
// the post-sim chipload gate's **arc-mean chip thickness** observation.
// That observation was deleted on 2026-08-06 — the gate now reports
// `effective_feed / (rpm · flutes)`, a linear advance per tooth. The
// table did not follow, and `suggest::recalibrate_feed_for_chipload`
// went on solving `feed = target / arc_fit_ratio × rpm × flutes`
// against a quantity nothing reported any more.
//
// Measured cost, on the real Suggest path, four synthetic fixtures at
// simulation cell 0.4 mm (A-5,
// `planning/review_2026-08-08/ARC_FIT_RATIO_EVIDENCE.md`): Suggest's own
// shipped recommendation, applied unmodified and simulated, tripped the
// chipload gate on **4 of 4** fixtures, at **1.85×–5.00×** the gate's
// band maximum. Removing the lift leaves the commanded advance per tooth
// on the derated band **minimum** unaided (0.999× measured on both
// Adaptive3d fixtures) — the calculator was already placing the
// operating point inside the band, and pass 8 was multiplying it out.
//
// **Checkpoint J-1/J-5 (ruled 2026-08-13, operator, BINDING)** retired
// the whole chain rather than re-keying the ratios to 1.0: no
// pre-simulation solve can know the achieved/commanded feed ratio, which
// is what the current gate observation actually depends on. Any feed
// lift now belongs to the **simulation-backed** path
// (`feed_modulation`, `tool_load::optimize::retarget::chipload`), which
// corrects from a measured observation — that path is measured clean 4/4
// on the same fixtures. The two `SuggestWarning` variants
// (`FeedRaisedForChipload`, `ChiploadStillLowAfterRecalibration`) were
// deliberately KEPT for that path to reuse; see their own docs.
//
// Do not reintroduce a pre-simulation feed lift keyed on an
// operation-family constant.

/// Components that fed the closed-form δ_tip evaluation. Each field is
/// in its natural unit (N, mm, mm⁴) — the back-off loop's UI surface
/// formats these into operator-readable strings.
#[derive(Debug, Clone)]
pub struct DeflectionBreakdown {
    /// Peak lateral cutting force (N): `Kc · axial_doc · radial_woc`.
    pub f_lateral_n: f64,
    /// Cantilever length (mm) from the collet face.
    pub stickout_mm: f64,
    /// Effective second moment of area (mm⁴): `π · d_core⁴ / 64`.
    pub i_eff_mm4: f64,
    /// Computed feed-per-tooth (mm) — diagnostic, not in the δ formula.
    pub chipload_per_tooth_mm: f64,
    /// Resolved axial DOC (mm) — `operation.depth_per_pass()` when set.
    pub axial_doc_mm: f64,
    /// Resolved radial WOC (mm) — `operation.stepover()` if set, else
    /// `adaptive_woc_factor × D` for Adaptive family, `0.35 × D` else.
    pub radial_woc_mm: f64,
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
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::{AdaptiveConfig, PocketConfig};
    use crate::compute::tool_config::{
        BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType,
    };
    use crate::machine::MachineProfile;
    use crate::material::{Material, WoodSpecies};

    fn carbide_endmill(diameter: f64, stickout: f64, cutting_length: f64) -> ToolConfig {
        ToolConfig {
            id: ToolId(0),
            name: "test endmill".to_owned(),
            tool_number: 1,
            tool_type: ToolType::EndMill,
            diameter,
            cutting_length,
            helix_deg: 30.0,
            corner_radius_mm: 0.0,
            corner_radius: 0.0,
            included_angle: 90.0,
            taper_half_angle: 15.0,
            shaft_diameter: diameter,
            holder_diameter: 25.0,
            shank_diameter: diameter,
            shank_length: 20.0,
            stickout,
            flute_count: 2,
            tool_material: ToolMaterial::Carbide,
            cut_direction: BitCutDirection::UpCut,
            vendor: String::new(),
            product_id: String::new(),
        }
    }

    /// Wanaka Back Rough configuration — Adaptive op, 6 mm carbide
    /// endmill at 45 mm stickout, DPP 9 mm, stepover 1.2 mm, feed
    /// 911 mm/min, RPM 16 000, 2 flutes, HardMaple.
    fn wanaka_adaptive_op(dpp: f64, stepover: f64, feed: f64, rpm: u32) -> OperationConfig {
        OperationConfig::Adaptive(AdaptiveConfig {
            depth_per_pass: dpp,
            stepover,
            feed_rate: feed,
            spindle_rpm: Some(rpm),
            ..AdaptiveConfig::default()
        })
    }

    fn hardwood() -> Material {
        Material::SolidWood {
            species: WoodSpecies::HardMaple,
        }
    }

    fn shapeoko() -> MachineProfile {
        // Generic-wood-router default; the predictor reads only
        // `machine.rigidity.adaptive_woc_factor` (used as a WOC
        // fallback when the op has no explicit stepover), so the
        // choice of preset doesn't materially shift these tests.
        MachineProfile::default()
    }

    #[test]
    fn wanaka_back_rough_predicts_within_post_sim_band() {
        // A roughing endmill (6 mm flat, 45 mm stickout, hardwood, 9 mm DPP,
        // 1.2 mm radial). The predictor delegates its cantilever to the same
        // integrated two-section model the post-sim gate uses, so it
        // forecasts what the gate will measure.
        //
        // Under the feed-aware literature-absolute force model
        // (`feeds::force`, woodresearch.sk affine fit), the instantaneous
        // bending force here is ~58 N (ap 9 · (Ks·h_eff + F_edge), h_eff ≈
        // 0.023 mm at this chipload/immersion), and the stiff-shank
        // two-section beam puts the predicted peak at ~48 µm — comfortably
        // Within. A stubby 6 mm flat at L/D 7.5 is NOT deflection-limited;
        // its limiter is chipload/power, not tip wander. (The old
        // `Kc·ap·ae` aggregate read ~316 µm here — ~4× the honest
        // instantaneous force, which is why the deflection gate used to
        // cry wolf on routine roughing.)
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = wanaka_adaptive_op(9.0, 1.2, 911.0, 16_000);
        let mat = hardwood();
        let machine = shapeoko();
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine)
            .expect("a modelled roughing cut must produce a figure");
        assert_eq!(pred.caveat, None, "a flat end mill carries no caveat");
        let um = pred.predicted_um;
        assert!(
            (30.0..=70.0).contains(&um),
            "stubby roughing endmill is deflection-safe (~48 µm Within) under the feed-aware force model; got {um:.1} µm"
        );
        // Sanity: the breakdown should record the inputs we passed.
        assert!((pred.breakdown.axial_doc_mm - 9.0).abs() < 1e-9);
        assert!((pred.breakdown.radial_woc_mm - 1.2).abs() < 1e-9);
        assert!((pred.breakdown.stickout_mm - 45.0).abs() < 1e-9);
        assert!(pred.breakdown.f_lateral_n > 0.0);
        assert!(pred.breakdown.i_eff_mm4 > 0.0);
        assert!(pred.breakdown.chipload_per_tooth_mm > 0.0);
    }

    #[test]
    fn shallow_dpp_predicts_well_below_threshold() {
        // A genuinely-shallow cut: DPP=3 mm on a 30 mm-reach tool.
        // Deflection is roughly linear in axial DOC (force scales
        // linearly; load_pos shifts only ~5%). Under the feed-aware
        // literature-absolute force model the instantaneous force is
        // modest, so a shallow 3 mm cut on a short-reach tool reads ~5 µm
        // — deeply Within. Test intent: a shallow cut clears the bound by
        // a wide margin.
        let tool = carbide_endmill(6.0, 30.0, 25.0);
        let op = wanaka_adaptive_op(3.0, 1.2, 911.0, 16_000);
        let mat = hardwood();
        let machine = shapeoko();
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine)
            .expect("a modelled shallow cut must produce a figure");
        let um = pred.predicted_um;
        assert!(
            um < 200.0,
            "Shallow-DPP prediction should clear the 200 µm critical threshold; got {um:.1} µm"
        );
        assert!(
            (1.0..=50.0).contains(&um),
            "Shallow-DPP prediction should land in a low Within band (real signal, not zero); got {um:.1} µm"
        );
    }

    #[test]
    fn doubling_stickout_octuples_deflection() {
        // Cantilever law: δ ∝ a²·(3L−a) ≈ L³ for load near tip. Two
        // synthetic tools that differ only in stickout (and matching
        // cutting_length so the bending region is consistent) should
        // produce a ratio ≈ 8 ± 20% — validates the cubic-in-length
        // law is wired through correctly.
        //
        // Use DPP=2 mm to keep `a ≈ L` so the formula is dominated by
        // the L³ term and the (3L−a) factor doesn't drift more than a
        // few percent between the two cases.
        //
        // `cutting_length == stickout` on BOTH tools, so the beam really is
        // uniform and the cubic law is the thing under test. They used to be
        // 20 mm and 45 mm against stickouts of 25 mm and 50 mm, which left a
        // fixed 5 mm of stiff shank on each. That was 21 % of the short tool
        // and 10 % of the long one, so the long tool was proportionally
        // softer and the ratio ran high. It stayed inside the +-20 % window
        // only while the cutter section equalled the shank section; T-17 made
        // the cutter softer and the asymmetry surfaced at 9.79x.
        //
        // Uniform, the hand value is a²(3L−a) short-to-long:
        // 24²(75−24) = 29 376 against 49²(150−49) = 242 501, ratio 8.26.
        let short = carbide_endmill(6.0, 25.0, 25.0);
        let long = carbide_endmill(6.0, 50.0, 50.0);
        // Pocket op so the predictor path doesn't apply the Adaptive
        // WOC fallback (we pass an explicit stepover anyway).
        let op = OperationConfig::Pocket(PocketConfig {
            depth_per_pass: 2.0,
            stepover: 1.0,
            feed_rate: 800.0,
            spindle_rpm: Some(16_000),
            ..PocketConfig::default()
        });
        let mat = hardwood();
        let machine = shapeoko();
        let p_short = predict_peak_deflection_um(&op, &short, &mat, &machine)
            .expect("short tool is modelled")
            .predicted_um;
        let p_long = predict_peak_deflection_um(&op, &long, &mat, &machine)
            .expect("long tool is modelled")
            .predicted_um;
        assert!(p_short > 0.0, "short-stickout prediction must be non-zero");
        let ratio = p_long / p_short;
        assert!(
            (6.4..=9.6).contains(&ratio),
            "δ ratio for doubled stickout should be ~8× (±20%); got {ratio:.2}× \
             (short={p_short:.1} µm, long={p_long:.1} µm)"
        );
    }

    #[test]
    fn zero_feed_names_its_refusal() {
        // Feed=0 → chipload=0. The predictor refuses so callers don't
        // see a phantom deflection signal on an unconfigured op. Before
        // T-4 the refusal was a 0.0 that read as a measurement.
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = wanaka_adaptive_op(9.0, 1.2, 0.0, 16_000);
        let mat = hardwood();
        let machine = shapeoko();
        assert_eq!(
            predict_peak_deflection_um(&op, &tool, &mat, &machine)
                .expect_err("a zero chipload must refuse"),
            DeflectionUnmodeled::NoChipload
        );
    }

    #[test]
    fn drill_op_names_its_refusal() {
        // Drill family is Z-only — the closed-form has no meaning for
        // it. Mirrors the `NotApplicableForOp` refusal in the post-sim
        // gate, and now says so in the same vocabulary.
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = OperationConfig::new_default(OperationType::Drill);
        let mat = hardwood();
        let machine = shapeoko();
        assert_eq!(
            predict_peak_deflection_um(&op, &tool, &mat, &machine)
                .expect_err("a drill must refuse"),
            DeflectionUnmodeled::NotApplicableForOp(OperationType::Drill)
        );
    }

    /// v1.2 combined-Suggest: V-carve toolpaths are feature-driven —
    /// the move count is governed by the input vector geometry, not by
    /// `stepover × bbox`. The runtime-sanity back-off must not try to
    /// gate v-carve runs, so the predictor returns 0 regardless of
    /// stepover or bbox dimensions.
    #[test]
    fn predict_move_count_zero_for_v_carve() {
        use crate::compute::operation_configs::VCarveConfig;
        let mut tool = carbide_endmill(6.35, 25.0, 12.0);
        tool.tool_type = ToolType::VBit;
        let op = OperationConfig::VCarve(VCarveConfig::default());
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -10.0),
            max: crate::geo::P3::new(500.0, 400.0, 0.0),
        };
        let moves = predict_move_count(&op, Some(&bbox), &tool);
        assert_eq!(
            moves, 0,
            "V-carve must return 0 moves regardless of stepover/bbox (feature-driven), got {moves}"
        );

        // None bbox also returns 0 — the "no constraint" pass-through.
        let mut pocket_tool = carbide_endmill(6.35, 25.0, 12.0);
        pocket_tool.tool_type = ToolType::EndMill;
        let pocket_op = OperationConfig::Pocket(PocketConfig {
            stepover: 1.0,
            depth_per_pass: 2.0,
            ..PocketConfig::default()
        });
        assert_eq!(
            predict_move_count(&pocket_op, None, &pocket_tool),
            0,
            "model_bbox=None must yield 0 (no constraint signal)"
        );
    }

    // ── RETIRED 2026-08-13 (Checkpoint J-1/J-5) ────────────────────────
    //
    // Five tests stood here and were deleted with the code they pinned:
    // `adaptive3d_wanaka_predicts_observed_chipload_within_band` (pinned
    // arc_fit_ratio 0.25 / Calibrated), `drop_cutter_wanaka_predicts_
    // observed_chipload` (0.15 / Calibrated), `drill_returns_not_
    // applicable`, `v_carve_returns_not_applicable` and
    // `zero_feed_returns_zero_observed` (0.25 / Calibrated). All five
    // asserted properties of `predict_observed_chipload_mm`, which no
    // longer exists — see the retirement note above `DeflectionBreakdown`.
    // The behaviour they guarded is now guarded from the other end, by
    // `tests/arc_fit_disposition_a5.rs::retired_lift_leaves_feed_at_the_
    // calculator_value`, which asserts Suggest ships the un-lifted feed.

    #[test]
    fn vbit_is_modelled_and_carries_the_caveat() {
        // T-4 deleted the blanket V-bit guard. Its stated reason — "the
        // post-sim integrator handles V-bits" — stopped holding when
        // this predictor started calling that integrator. The cone is
        // modelled through `lookup_diameter_at`; the flute relief is
        // not, and that gap is non-conservative, so the figure ships as
        // a floor.
        let mut tool = carbide_endmill(6.35, 25.0, 12.0);
        tool.tool_type = ToolType::VBit;
        let op = OperationConfig::Pocket(PocketConfig {
            depth_per_pass: 1.0,
            stepover: 0.5,
            feed_rate: 600.0,
            spindle_rpm: Some(18_000),
            ..PocketConfig::default()
        });
        let mat = hardwood();
        let machine = shapeoko();
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine)
            .expect("a V-bit is modelled since T-4");
        assert!(
            pred.predicted_um > 0.0 && pred.predicted_um.is_finite(),
            "the V-bit figure must be a real number, got {} µm",
            pred.predicted_um
        );
        assert_eq!(
            pred.caveat,
            Some(DeflectionCaveat::FluteReliefUnmodeled),
            "the V-bit figure is a floor and must say so"
        );
    }
}
