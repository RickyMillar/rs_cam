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
//! `core_diameter_mm` / `I_eff` are still computed and surfaced in the
//! [`DeflectionBreakdown`] as a representative-section *diagnostic*, but
//! no longer drive the deflection magnitude. V-bit geometry still returns
//! zero (the post-sim integrator handles those).
//!
//! ## Refusal cases (return `predicted_um == 0.0`)
//!
//! - V-bit tool geometry (closed-form not modeled — use the post-sim
//!   integrator).
//! - Drill operation family (Z-only kinematics, no continuous engagement).
//! - Material has no primary-source `kc_n_per_mm2()` (`Custom`,
//!   out-of-band `SolidWoodByJanka`).
//! - Zero stickout, zero diameter, zero feed (chipload), or unset DPP.
//!
//! The 0.0 return is the contract for the back-off loop the next task
//! delivers — "no constraint signal", caller treats as a pass-through.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{CutterKind, OperationFamily as FeedsOperationFamily};
use crate::machine::MachineProfile;
use crate::material::Material;

/// Flute-relief factor for end mills. Pre-2026-06-04 deflection
/// audits (Wanaka Back Rough) consistently undershot post-sim by ~3×
/// when the bending section was treated as solid `D`. The 0.7×
/// reduction is the canonical handbook value for 2- to 3-flute
/// end mills (Machinery's Handbook stiffness-correction notes; matches
/// the FSWizard "effective root diameter" recommendation).
///
/// Cross-reference: the post-sim deflection integrator in
/// [`crate::tool_load::deflection`] / `ToolDefinition::tip_deflection_mm`
/// derives its own bending section independently. Drift between the
/// two — and the +36% safe-side bias the predictor currently carries
/// — is called out on
/// [`crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_UM`].
pub const ENDMILL_CORE_FRACTION: f64 = 0.7;

/// Stickout fallback when [`ToolConfig::stickout`] is non-positive
/// (zero or NaN). Returning 0 deflection on a misconfigured stickout
/// would make the predictor silently pass every back-off iteration —
/// instead we use the cutting-length + a 5 mm collet exposure margin
/// (matching the convention in `tool_load::deflection` test fixtures
/// which build `stickout = cutting_length + 5 mm`).
pub const COLLET_EXPOSURE_MARGIN_MM: f64 = 5.0;

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
/// Used by both [`predict_peak_deflection_um`] (chipload diagnostic)
/// and [`predict_observed_chipload_mm`] (nominal-chipload solve).
/// Matches the conservative "wood router default" the rest of the
/// codebase falls back to when RPM is unset.
const PREDICTOR_FALLBACK_RPM: f64 = 18_000.0;

/// Closed-form prediction of peak tip deflection (µm) Suggest would
/// produce at the given (operation, tool, material, machine).
///
/// Returns `DeflectionPrediction { predicted_um: 0.0, .. }` for any
/// refusal case (see module docs); callers downstream treat zero as
/// "no constraint signal" rather than an error condition.
#[tracing::instrument(level = "debug", skip_all, fields(op = ?operation.op_type()))]
pub fn predict_peak_deflection_um(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> DeflectionPrediction {
    let breakdown_zero = DeflectionBreakdown {
        f_lateral_n: 0.0,
        stickout_mm: 0.0,
        i_eff_mm4: 0.0,
        chipload_per_tooth_mm: 0.0,
        axial_doc_mm: 0.0,
        radial_woc_mm: 0.0,
    };

    let op_type = operation.op_type();
    let feeds_family = op_type.spec().feeds_family;

    // Drill ops: Z-only kinematics, no continuous radial engagement —
    // the cantilever-deflection model doesn't apply. Mirrors the
    // `NotApplicableForOp` refusal in `tool_load::deflection::evaluate`.
    if feeds_family == FeedsOperationFamily::Drill || op_type == OperationType::Drill {
        tracing::debug!(reason = "drill_not_applicable", "predictor returns 0 µm");
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
    }

    // V-bit closed-form is undefined (engaged-D grows linearly with DOC,
    // and the "core" of a triangular profile isn't a bending section).
    // The post-sim integrator with `lookup_diameter_at` handles V-bits;
    // the closed-form predictor refuses.
    if tool.tool_type.cutter_kind() == CutterKind::VBit {
        tracing::debug!(
            reason = "vbit_unsupported",
            "predictor returns 0 µm — V-bit closed-form not modeled"
        );
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
    }

    // Material: only primary-source Kc materials get a numeric
    // prediction. Custom / out-of-band SolidWoodByJanka return None
    // and we route to 0 — matches the `MaterialUnvalidated` refusal in
    // the post-sim gate. The force magnitude itself is recomputed inside
    // `feeds::force::lateral_cutting_force`; here we only need the
    // existence check for the early refusal.
    if material.kc_n_per_mm2().is_none() {
        tracing::debug!(
            reason = "material_unvalidated",
            material = %material.label(),
            "predictor returns 0 µm — no primary-source Kc"
        );
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
    }

    // --- Engagement geometry ---
    let diameter_mm = tool.diameter;
    if !(diameter_mm.is_finite() && diameter_mm > 0.0) {
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
    }

    let Some(axial_doc_mm) = operation.depth_per_pass() else {
        tracing::debug!(
            reason = "no_dpp",
            "predictor returns 0 µm — operation has no DPP"
        );
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
    };
    if !(axial_doc_mm.is_finite() && axial_doc_mm > 0.0) {
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
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
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
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
    // already refused. Return 0 here so the back-off loop doesn't see
    // a phantom deflection from an unconfigured op.
    if chipload_per_tooth_mm <= 0.0 {
        tracing::debug!(
            reason = "zero_chipload",
            "predictor returns 0 µm — feed/RPM/flutes resolved to zero chipload"
        );
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: DeflectionBreakdown {
                axial_doc_mm,
                radial_woc_mm,
                chipload_per_tooth_mm,
                ..breakdown_zero
            },
        };
    }

    // --- Cantilever geometry ---
    let stickout_mm = if tool.stickout.is_finite() && tool.stickout > 0.0 {
        tool.stickout
    } else {
        (tool.cutting_length + COLLET_EXPOSURE_MARGIN_MM).max(0.0)
    };
    if stickout_mm <= 0.0 {
        return DeflectionPrediction {
            predicted_um: 0.0,
            breakdown: breakdown_zero,
        };
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
    let d_core = core_diameter_mm(tool);
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

    // Delegate the cantilever to the canonical integrated model. Returns
    // None only for refusal cases the guards above already excluded
    // (Custom material / non-positive stickout / inputs) or a degenerate
    // engagement-deeper-than-stickout geometry — treat any None as "no
    // constraint signal" (0 µm), matching the DeflectionPrediction contract.
    let tool_def = crate::compute::cutter::build_cutter(tool);
    let predicted_um = tip_deflection_from_engagement(
        &tool_def,
        material,
        axial_doc_mm,
        immersion_rad,
        chipload_per_tooth_mm,
    )
    .map_or(0.0, |delta_mm| delta_mm * 1000.0);

    tracing::debug!(
        predicted_um,
        f_lateral_n,
        stickout_mm,
        i_eff_mm4,
        axial_doc_mm,
        radial_woc_mm,
        "deflection prediction (delegated to integrated two-section cantilever)"
    );

    DeflectionPrediction {
        predicted_um,
        breakdown: DeflectionBreakdown {
            f_lateral_n,
            stickout_mm,
            i_eff_mm4,
            chipload_per_tooth_mm,
            axial_doc_mm,
            radial_woc_mm,
        },
    }
}

/// Effective bending-section diameter for the closed-form cantilever.
///
/// - End mill: `0.7 · D` — flute relief reduces the bending section
///   well below nominal outer diameter (Machinery's Handbook stiffness
///   notes, FSWizard effective root diameter).
/// - Ball nose: `D` — the ball/shank junction is the limiting section;
///   the flute relief above the ball is short relative to total stickout.
/// - Bull nose: `D` — same as ball, the corner radius leaves a near-solid
///   shaft above.
/// - Tapered ball nose: mean of tip and shank, weighted toward the
///   shank (the cone-shoulder section dominates bending stiffness above
///   the tip). Floor at 0.5 mm to keep `1/d⁴` finite.
/// - V-bit: returns 0 (caller refuses earlier).
///
/// Routes on [`CutterKind`] (Phase 3) — the bending-section model is a
/// per-shape-class decision, so a 6th cutter shape fails to compile
/// here instead of inheriting a wrong core silently.
pub fn core_diameter_mm(tool: &ToolConfig) -> f64 {
    match tool.tool_type.cutter_kind() {
        CutterKind::Flat => ENDMILL_CORE_FRACTION * tool.diameter,
        CutterKind::Ball | CutterKind::Bull => tool.diameter,
        CutterKind::TaperedBall => {
            let shank = tool.shaft_diameter.max(tool.diameter);
            // Weight 60% shank / 40% tip: the bending stiffness
            // integral above the ball junction is dominated by the
            // larger cone-shoulder section.
            (PREDICTOR_TAPERED_BALL_SHANK_WEIGHT * shank
                + PREDICTOR_TAPERED_BALL_TIP_WEIGHT * tool.diameter)
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

/// Closed-form deflection prediction surfaced to the Suggest back-off
/// loop. `predicted_um == 0.0` is the contract for "no constraint
/// signal" — the caller treats it as a pass-through rather than a hard
/// refusal (see module docs for the cases that route to zero).
#[derive(Debug, Clone)]
pub struct DeflectionPrediction {
    pub predicted_um: f64,
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

/// Forward-predict the median observed chipload per tooth that the
/// post-sim chipload gate will report. Bridges the LUT nominal vs
/// arc-fit observed gap.
///
/// Nominal chipload = `feed / (rpm × flutes)`. Observed median is
/// `nominal × arc_fit_ratio`, where `arc_fit_ratio` depends on the
/// op family (engagement geometry) and is empirically calibrated
/// against post-sim measurements where available — conservative
/// defaults are used for op families that don't yet have a
/// measured Wanaka cell.
///
/// ## Why this exists
///
/// The vendor LUT publishes a nominal `chipload_per_tooth` value
/// (what an ideal full-flute engagement would shave per tooth).
/// Suggest writes `feed/RPM` so the geometric nominal matches the
/// LUT. The post-sim chipload gate, however, measures the **median
/// of per-sample chip thickness across the arc-fit kinematics path**
/// — and on narrow-engagement strategies (Adaptive3D, DropCutter)
/// the observed median runs 0.15–0.25× of nominal because each
/// engagement arc only briefly sees the full chip.
///
/// Step 2 of the combined-Suggest plan uses this predictor to lift
/// feed back up so the *observed* median lands inside the LUT band
/// rather than the *nominal*. This function is the building block
/// for that — step 1 is pure forward-prediction with calibration.
///
/// ## Refusal cases (`source == NotApplicable`, observed = 0.0)
///
/// - Drill / AlignmentPinDrill — Z-only kinematics, no continuous
///   radial engagement; drill ops route through drill-native gates
///   (peck adequacy, chip welding) instead of the chipload gate.
/// - V-carve / Project Curve — feature-driven (curve segments,
///   projected geometry); chipload doesn't apply as a continuous
///   per-tooth measurement.
/// - `feed_rate <= 0` or any non-finite input — produces zero
///   observed (matches the contract for the back-off loop's
///   "no constraint signal" pass-through).
#[tracing::instrument(level = "debug", skip_all, fields(op = ?operation.op_type()))]
pub fn predict_observed_chipload_mm(
    operation: &OperationConfig,
    tool: &ToolConfig,
) -> ObservedChiploadPrediction {
    let op_type = operation.op_type();

    // Feature-driven and Z-only operations: no continuous per-tooth
    // chipload to predict. Mirrors the post-sim gate's
    // `NotApplicableForOp` refusal path.
    let arc_fit = match arc_fit_ratio_for_op(op_type) {
        ArcFitDispatch::Ratio { value, source } => (value, source),
        ArcFitDispatch::NotApplicable => {
            tracing::debug!(
                reason = "op_not_applicable",
                "predictor returns 0 chipload — feature-driven or Z-only kinematics"
            );
            return ObservedChiploadPrediction {
                nominal_mm_per_tooth: 0.0,
                arc_fit_ratio: 0.0,
                observed_median_mm_per_tooth: 0.0,
                source: ArcFitRatioSource::NotApplicable,
            };
        }
    };
    let (arc_fit_ratio, source) = arc_fit;

    // Nominal chipload = feed / (rpm × flutes). Mirrors the gate's
    // own derivation in `feeds::calculate` (and the diagnostic field
    // in `DeflectionBreakdown::chipload_per_tooth_mm` above).
    let rpm = operation
        .spindle_rpm()
        .map_or(PREDICTOR_FALLBACK_RPM, |r| r as f64);
    let flute_count = tool.flute_count.max(1) as f64;
    let feed_rate = operation.feed_rate();
    let nominal = if feed_rate.is_finite() && feed_rate > 0.0 && rpm > 0.0 && flute_count > 0.0 {
        feed_rate / (rpm * flute_count)
    } else {
        0.0
    };

    if nominal <= 0.0 {
        tracing::debug!(
            reason = "zero_nominal",
            "predictor returns 0 chipload — feed/RPM/flutes resolved to zero nominal"
        );
        return ObservedChiploadPrediction {
            nominal_mm_per_tooth: 0.0,
            arc_fit_ratio,
            observed_median_mm_per_tooth: 0.0,
            source,
        };
    }

    let observed = nominal * arc_fit_ratio;

    tracing::debug!(
        nominal,
        arc_fit_ratio,
        observed,
        ?source,
        "observed-chipload prediction"
    );

    ObservedChiploadPrediction {
        nominal_mm_per_tooth: nominal,
        arc_fit_ratio,
        observed_median_mm_per_tooth: observed,
        source,
    }
}

/// Internal dispatch enum: either an arc-fit ratio with its source
/// tag, or a hard refusal. Keeps the per-op-type table in one place.
enum ArcFitDispatch {
    Ratio {
        value: f64,
        source: ArcFitRatioSource,
    },
    NotApplicable,
}

/// Per-operation arc-fit ratio table. Calibrated where Wanaka post-sim
/// data exists, conservative-default otherwise. Conservative meaning
/// "biased toward predicting a lower observed median," which makes the
/// step-2 feed-up calibration err on the side of pulling feed slightly
/// higher than strictly necessary — same safer-side bias the v1.1
/// deflection predictor uses.
///
/// | Op family | ratio | source | notes |
/// |-----------|-------|--------|-------|
/// | Adaptive (2D) | 0.30 | Default | narrow WOC, less than full slot |
/// | Adaptive3D | 0.25 | Calibrated | Wanaka Back Rough / 3D Rough 6 |
/// | DropCutter | 0.15 | Calibrated | Wanaka 3D Finish 6 |
/// | Scallop, SpiralFinish | 0.15 | Default | DropCutter-like geometry |
/// | Waterline | 0.40 | Default | longer cut spans |
/// | SteepShallow | 0.30 | Default | mixed engagement |
/// | HorizontalFinish, RadialFinish, Zigzag, RampFinish | 0.40 | Default | flat / raster scans |
/// | Pocket | 0.60 | Default | full-slot first pass |
/// | Rest | 0.50 | Default | mid-engagement |
/// | Profile | 0.80 | Default | side-step, mostly full-flute height |
/// | Trace, Chamfer, Pencil, Inlay, Face | 0.50 | Default | conservative middle |
/// | Drill, AlignmentPinDrill, VCarve, ProjectCurve | — | NotApplicable | feature-driven or Z-only |
/// # ⚠ STALE SINCE 2026-08-06 — this table predicts a quantity the gate
/// # no longer reports. NOT fixed here, on purpose.
///
/// Every ratio below was fitted against the post-sim chipload gate's
/// **arc-mean chip thickness** observation. That observation was deleted
/// on 2026-08-06: the gate now reports `effective_feed / (rpm · flutes)`,
/// a linear advance per tooth (`tool_load::chipload`'s header, and
/// `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` for the
/// primary sources). Against that observation the correct arc-fit ratio
/// is the **achieved/commanded feed ratio**, which this pre-sim
/// predictor cannot know — not a per-operation-family constant, because
/// the quantity the constants approximate no longer exists.
///
/// B-lit §3.3 and §6.1 (C-13 / F-5) rule the disposition explicitly:
/// **retire this table, do not re-key it.** The census's earlier advice
/// — "replace with `f_lut × expected_feed_ratio`" — is superseded.
///
/// It is left standing here because retiring it is a *number-moving*
/// change to Suggest, not to the gate: `feeds::suggest::recalibrate_feed_for_chipload`
/// solves `target_nominal = target / arc_fit_ratio` and is gated on
/// `ArcFitRatioSource::Calibrated`, so today only Adaptive3d (0.25) and
/// DropCutter (0.15) get a feed lift at all. Setting every ratio to 1.0
/// would (a) change the solved feed on those two families by 4× and
/// 6.7×, and (b) extend the lift to every other family for the first
/// time. That is a separate approval with its own before/after, and
/// folding it into the unit conversion would make the conversion's
/// verdict-flip table unattributable.
///
/// Owner: census T3.5. Re-open condition: none needed — it is the next
/// item in the same chain.
fn arc_fit_ratio_for_op(op_type: OperationType) -> ArcFitDispatch {
    use ArcFitRatioSource::{Calibrated, Default as DefSrc};
    match op_type {
        // Calibrated against Wanaka post-sim (2026-06-03).
        OperationType::Adaptive3d => ArcFitDispatch::Ratio {
            value: 0.25,
            source: Calibrated,
        },
        OperationType::DropCutter => ArcFitDispatch::Ratio {
            value: 0.15,
            source: Calibrated,
        },

        // Defaults — no calibration cell yet.
        OperationType::Adaptive => ArcFitDispatch::Ratio {
            value: 0.30,
            source: DefSrc,
        },
        // UnifiedFinish: no calibration cell yet (new op) — mirrors
        // Scallop's ratio per the registration decision (its mid-steep
        // band literally IS a scallop pass; the waterline/raster bands
        // don't have their own calibration either). Revisit once Wanaka
        // post-sim data exists for this op.
        OperationType::Scallop | OperationType::UnifiedFinish | OperationType::SpiralFinish => {
            ArcFitDispatch::Ratio {
                value: 0.15,
                source: DefSrc,
            }
        }
        OperationType::Waterline => ArcFitDispatch::Ratio {
            value: 0.40,
            source: DefSrc,
        },
        OperationType::SteepShallow => ArcFitDispatch::Ratio {
            value: 0.30,
            source: DefSrc,
        },
        OperationType::HorizontalFinish
        | OperationType::RadialFinish
        | OperationType::Zigzag
        | OperationType::RampFinish => ArcFitDispatch::Ratio {
            value: 0.40,
            source: DefSrc,
        },
        OperationType::Pocket => ArcFitDispatch::Ratio {
            value: 0.60,
            source: DefSrc,
        },
        OperationType::Rest => ArcFitDispatch::Ratio {
            value: 0.50,
            source: DefSrc,
        },
        OperationType::Profile => ArcFitDispatch::Ratio {
            value: 0.80,
            source: DefSrc,
        },
        OperationType::Trace
        | OperationType::Chamfer
        | OperationType::Pencil
        | OperationType::Inlay
        | OperationType::Face => ArcFitDispatch::Ratio {
            value: 0.50,
            source: DefSrc,
        },

        // Feature-driven / Z-only — chipload-as-continuous-metric
        // doesn't apply. Caller treats observed = 0.0 as "no signal".
        OperationType::VCarve
        | OperationType::ProjectCurve
        | OperationType::Drill
        | OperationType::AlignmentPinDrill => ArcFitDispatch::NotApplicable,
    }
}

/// Forward-callable prediction of the median observed chipload per
/// tooth the post-sim chipload gate will measure.
///
/// `observed_median_mm_per_tooth == 0.0` is the contract for "no
/// constraint signal" — callers downstream of the back-off loop
/// treat zero as a pass-through rather than a hard refusal.
#[derive(Debug, Clone)]
pub struct ObservedChiploadPrediction {
    /// Geometric nominal `feed / (rpm × flutes)` (mm/tooth).
    pub nominal_mm_per_tooth: f64,
    /// Arc-fit multiplier in `[0.0, 1.0]`. Zero indicates the op
    /// family doesn't have a continuous-engagement chipload (drill,
    /// v-bit, project-curve).
    pub arc_fit_ratio: f64,
    /// Forward-predicted median chipload the gate will report (mm/tooth):
    /// `nominal_mm_per_tooth × arc_fit_ratio`.
    ///
    /// ⚠ **Stale since 2026-08-06** — see [`arc_fit_ratio_for_op`]. The
    /// gate now reports a linear advance per tooth, so the honest
    /// prediction is `nominal_mm_per_tooth × achieved_feed_ratio`, which
    /// this pre-sim predictor cannot measure. Owner: census T3.5.
    pub observed_median_mm_per_tooth: f64,
    /// Provenance of the arc-fit ratio — calibrated against a Wanaka
    /// cell, conservative default, or refusal.
    pub source: ArcFitRatioSource,
}

/// Provenance of the per-op-family arc-fit ratio. Surfaced so the
/// rationale tree the back-off loop renders can show *why* a given
/// observed-chipload prediction landed where it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcFitRatioSource {
    /// Empirically calibrated against post-sim observation for this
    /// op family. Currently: Adaptive3D (Wanaka Back Rough / 3D Rough
    /// 6 → 0.25), DropCutter (Wanaka 3D Finish 6 → 0.15).
    Calibrated,
    /// Conservative default — used when this op family doesn't yet
    /// have a measured Wanaka cell. The numeric value is biased to
    /// over-predict the gap between nominal and observed, so step-2
    /// feed-up corrections err on the safe side.
    Default,
    /// Operation doesn't have a continuous-engagement chipload at
    /// all — drill / pin drill (Z-only kinematics), v-carve /
    /// project-curve (feature-driven). Observed = 0.0.
    NotApplicable,
}

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
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine);
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
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine);
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
        let short = carbide_endmill(6.0, 25.0, 20.0);
        let long = carbide_endmill(6.0, 50.0, 45.0);
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
        let p_short = predict_peak_deflection_um(&op, &short, &mat, &machine).predicted_um;
        let p_long = predict_peak_deflection_um(&op, &long, &mat, &machine).predicted_um;
        assert!(p_short > 0.0, "short-stickout prediction must be non-zero");
        let ratio = p_long / p_short;
        assert!(
            (6.4..=9.6).contains(&ratio),
            "δ ratio for doubled stickout should be ~8× (±20%); got {ratio:.2}× \
             (short={p_short:.1} µm, long={p_long:.1} µm)"
        );
    }

    #[test]
    fn zero_feed_returns_near_zero_deflection() {
        // Feed=0 → chipload=0. The predictor's force formula doesn't
        // include chipload directly, but the path refuses chipload≤0
        // so callers don't see a phantom deflection signal on an
        // unconfigured op. Per the contract, predicted_um stays at 0
        // (well under the 1 µm bound the task specifies).
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = wanaka_adaptive_op(9.0, 1.2, 0.0, 16_000);
        let mat = hardwood();
        let machine = shapeoko();
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine);
        assert!(
            pred.predicted_um <= 1.0,
            "Zero-feed op should yield ≤1 µm prediction; got {:.4} µm",
            pred.predicted_um
        );
    }

    #[test]
    fn drill_op_returns_zero() {
        // Drill family is Z-only — the closed-form has no meaning for
        // it. Mirrors the `NotApplicableForOp` refusal in the post-sim
        // gate.
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = OperationConfig::new_default(OperationType::Drill);
        let mat = hardwood();
        let machine = shapeoko();
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine);
        assert_eq!(pred.predicted_um, 0.0);
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

    /// Wanaka Back Rough / 3D Rough 6 (Adaptive3D, 6 mm endmill,
    /// feed 911 mm/min, 16 000 RPM, 2 flutes). LUT nominal chipload
    /// = 911 / (16000 × 2) = 0.0285 mm/tooth. Post-sim observed
    /// median = 0.0067 mm/tooth (ratio 0.23). Calibrated Adaptive3D
    /// ratio 0.25 predicts ≈ 0.0071 — within ±20% of the empirical
    /// 0.0067.
    #[test]
    fn adaptive3d_wanaka_predicts_observed_chipload_within_band() {
        use crate::compute::operation_configs::Adaptive3dConfig;
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 9.0,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            ..Adaptive3dConfig::default()
        });
        let pred = predict_observed_chipload_mm(&op, &tool);

        // Nominal: 911 / (16000 × 2) = 0.02846875
        let expected_nominal = 911.0 / (16_000.0 * 2.0);
        assert!(
            (pred.nominal_mm_per_tooth - expected_nominal).abs() < 1e-6,
            "Adaptive3D nominal should be {expected_nominal:.6}; got {:.6}",
            pred.nominal_mm_per_tooth
        );
        assert_eq!(pred.arc_fit_ratio, 0.25);
        assert_eq!(pred.source, ArcFitRatioSource::Calibrated);

        // Predicted observed ≈ 0.0285 × 0.25 = 0.0071. Empirical
        // Wanaka measurement is 0.0067 — the ±20% band on the
        // measurement covers 0.00536..0.00804.
        let empirical = 0.0067;
        let low = empirical * 0.80;
        let high = empirical * 1.20;
        assert!(
            (low..=high).contains(&pred.observed_median_mm_per_tooth),
            "Adaptive3D predicted observed should land within ±20% of \
             Wanaka empirical {empirical}; got {:.6} (band {low:.6}..={high:.6})",
            pred.observed_median_mm_per_tooth
        );
    }

    /// Wanaka 3D Finish 6 (DropCutter, 4 mm tapered ball nose).
    /// The empirical case ran at feed 3000 mm/min, 20 000 RPM, 2
    /// flutes → nominal = 3000 / (20000 × 2) = 0.075 mm/tooth.
    /// Post-sim observed median was 0.011 mm/tooth (ratio 0.147).
    /// Calibrated DropCutter ratio 0.15 predicts ≈ 0.01125 — within
    /// ±20% of empirical 0.011.
    #[test]
    fn drop_cutter_wanaka_predicts_observed_chipload() {
        use crate::compute::operation_configs::DropCutterConfig;
        let mut tool = carbide_endmill(4.0, 30.0, 20.0);
        tool.tool_type = ToolType::TaperedBallNose;
        tool.shaft_diameter = 6.0;

        let op = OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.18,
            feed_rate: 3000.0,
            plunge_rate: 800.0,
            spindle_rpm: Some(20_000),
            ..DropCutterConfig::default()
        });
        let pred = predict_observed_chipload_mm(&op, &tool);

        let expected_nominal = 3000.0 / (20_000.0 * 2.0);
        assert!(
            (pred.nominal_mm_per_tooth - expected_nominal).abs() < 1e-6,
            "DropCutter nominal should be {expected_nominal:.6}; got {:.6}",
            pred.nominal_mm_per_tooth
        );
        assert_eq!(pred.arc_fit_ratio, 0.15);
        assert_eq!(pred.source, ArcFitRatioSource::Calibrated);

        let empirical = 0.011;
        let low = empirical * 0.80;
        let high = empirical * 1.20;
        assert!(
            (low..=high).contains(&pred.observed_median_mm_per_tooth),
            "DropCutter predicted observed should land within ±20% of \
             Wanaka empirical {empirical}; got {:.6} (band {low:.6}..={high:.6})",
            pred.observed_median_mm_per_tooth
        );
    }

    /// Drill ops route through drill-native gates (peck adequacy,
    /// chip welding) — the chipload gate doesn't apply. Predictor
    /// returns source=NotApplicable with observed = 0.0.
    #[test]
    fn drill_returns_not_applicable() {
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = OperationConfig::new_default(OperationType::Drill);
        let pred = predict_observed_chipload_mm(&op, &tool);
        assert_eq!(pred.source, ArcFitRatioSource::NotApplicable);
        assert_eq!(pred.observed_median_mm_per_tooth, 0.0);
        assert_eq!(pred.arc_fit_ratio, 0.0);
    }

    /// V-carve is feature-driven (curve segments, projected geometry)
    /// — the chipload gate doesn't apply. Same NotApplicable contract
    /// as Drill.
    #[test]
    fn v_carve_returns_not_applicable() {
        use crate::compute::operation_configs::VCarveConfig;
        let mut tool = carbide_endmill(6.35, 25.0, 12.0);
        tool.tool_type = ToolType::VBit;
        let op = OperationConfig::VCarve(VCarveConfig::default());
        let pred = predict_observed_chipload_mm(&op, &tool);
        assert_eq!(pred.source, ArcFitRatioSource::NotApplicable);
        assert_eq!(pred.observed_median_mm_per_tooth, 0.0);
        assert_eq!(pred.arc_fit_ratio, 0.0);
    }

    /// Zero feed → zero nominal → zero observed. The back-off loop's
    /// "no constraint signal" contract: callers should treat observed
    /// = 0.0 as a pass-through rather than a hard refusal.
    #[test]
    fn zero_feed_returns_zero_observed() {
        use crate::compute::operation_configs::Adaptive3dConfig;
        let tool = carbide_endmill(6.0, 45.0, 25.0);
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 9.0,
            stepover: 1.2,
            feed_rate: 0.0,
            spindle_rpm: Some(16_000),
            ..Adaptive3dConfig::default()
        });
        let pred = predict_observed_chipload_mm(&op, &tool);
        assert_eq!(pred.nominal_mm_per_tooth, 0.0);
        assert_eq!(pred.observed_median_mm_per_tooth, 0.0);
        // Arc-fit ratio is still reported (Adaptive3d's calibrated
        // 0.25) — the refusal is on the nominal side, not the ratio.
        assert_eq!(pred.arc_fit_ratio, 0.25);
        assert_eq!(pred.source, ArcFitRatioSource::Calibrated);
    }

    #[test]
    fn vbit_returns_zero() {
        // Closed-form intentionally refuses V-bits — engaged-D grows
        // linearly with DOC and the bending-section "core" of a
        // triangular profile isn't well defined. The post-sim
        // integrator handles those.
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
        let pred = predict_peak_deflection_um(&op, &tool, &mat, &machine);
        assert_eq!(pred.predicted_um, 0.0);
    }
}
