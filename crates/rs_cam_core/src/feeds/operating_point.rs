//! Spindle power at the operating point an operation ships.
//!
//! ## Why this door exists
//!
//! [`crate::feeds::calculate`] publishes `FeedsResult::power_kw` at the
//! geometry the CALCULATOR was handed. `enforce_invariants_at_fixed_geometry` then runs, and
//! `clamp_dpp_to_rigidity` lowers the depth per pass. The published figure
//! therefore describes a depth that will not be cut — a value computed at one
//! state and consumed at another. Suggest pass 10
//! ([`crate::feeds::suggest`]) already knew how to evaluate power at the FINAL
//! state, but privately. This module is that evaluation, made public, so a
//! display and a Suggest pass read one number rather than two.
//!
//! ## The one model
//!
//! [`crate::tool_load::power::PowerTerms`], and nothing else. Power is affine
//! in the feed — the shear term carries the feed and the cross-section, the
//! edge term carries `ap` alone — so the two terms are kept apart. The model
//! type stays where it lives, inside `tool_load`: [`PowerFigure`] holds the
//! terms privately and answers through
//! [`PowerFigure::required_kw_at_feed`] and [`PowerFigure::feed_for_kw`].
//! Every caller solves through those two methods rather than rebuilding the
//! model.
//!
//! K7 (feeds contract P9): every reader builds its inputs through
//! [`crate::feeds::engagement::Engagement`]: `feeds::calculate` Step 6
//! (`power_model_terms`), this door, the post-simulation gate
//! (`tool_load::power::sample_power_kw`, `Engagement::from_sample`) and the
//! modulator power cap. This door ships the maximum over the op's
//! [`crate::feeds::engagement::PowerCases`] and names the case
//! ([`PowerFigure::case`]).
//!
//! ## The axis
//!
//! One axis. [`power_at_operating_point`] reads the operation's feed and
//! states the ceiling as the rated curve `power_at_rpm(rpm)`, the ceiling
//! `tool_load::power::evaluate` compares against. Until ruling R4 (2026-09-24)
//! both sides carried a `safety_factor`; the factor is gone (Q2). No load
//! margin scales the ceiling.
//!
//! ## The force line
//!
//! [`PowerFigure::force`] carries the line the figure was built from and
//! the mean chip it was evaluated at ([`ForceAtPoint`]). A mean chip
//! outside the printed range does not refuse; the point names its
//! [`crate::material::force_line::ChipRegime`], and the card states the
//! extrapolation. [`force_at_operating_point`] gives the same point
//! without a machine.
//!
//! ## Refusal
//!
//! Every absence is an [`Err`]. The `Ok` branch always carries a finite,
//! positive `available_kw` and a finite, non-negative `required_kw`, so a
//! display never paints an absence as a zero. The shape follows
//! [`crate::feeds::predict::DeflectionUnmodeled`] in this same folder.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::engagement::{EngagedDiameter, Engagement, PowerCase, PowerCases};
use crate::feeds::envelope::CutPoint;
use crate::feeds::suggest::CalculatorOperatingPoint;
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::material::force_line::{ForceAtPoint, mean_chip_mm};
use crate::tool_load::power::PowerTerms;

/// Why [`power_at_operating_point`] produced no power figure.
///
/// Each variant is one missing input, and each carries one operator-facing
/// clause through [`Self::clause`], so the GUI, the CLI and the MCP bridge
/// cannot word the same absence three ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerUnmodeled {
    /// The material has no force line (`Material::force_line` refuses).
    /// There is no power model without one, and `feeds::calculate` Step 6
    /// applied no ceiling either.
    MaterialUnvalidated,
    /// The operation carries no positive, finite feed rate.
    NoFeed,
    /// The operation carries no radial width of cut, and the caller supplied
    /// no fallback operating point.
    NoRadialEngagement,
    /// The operation carries no depth per pass, and the caller supplied no
    /// fallback operating point.
    NoDepthPerPass,
    /// The operation carries no spindle speed, and the caller supplied no
    /// fallback operating point.
    NoSpindleSpeed,
    /// The machine reports no positive power at this spindle speed, so there
    /// is no ceiling to quote the figure against.
    NoAvailablePower,
    /// The tool has no positive cutting diameter at this depth. A tapered
    /// ball or a V-bit above its cone reaches this.
    NoEngagementDiameter,
    /// P16 (review D7): the op follows a surface at an offset
    /// (`Axial::SurfaceOffset`, Project Curve), so the depth into the stock
    /// is known after the simulation. The post-sim power gate judges it.
    DepthAfterSimulation,
}

impl PowerUnmodeled {
    /// One operator-facing clause, identical on every surface.
    pub fn clause(&self) -> &'static str {
        match self {
            Self::MaterialUnvalidated => "this material has no measured force line (ruling B6)",
            Self::NoFeed => "the operation has no feed rate",
            Self::NoRadialEngagement => "the operation has no radial width of cut",
            Self::NoDepthPerPass => "the operation has no depth per pass",
            Self::NoSpindleSpeed => "the operation has no spindle speed",
            Self::NoAvailablePower => "the machine publishes no spindle power at this speed",
            Self::NoEngagementDiameter => "the tool has no cutting diameter at this depth",
            Self::DepthAfterSimulation => "the depth into the stock is known after the simulation",
        }
    }
}

/// Spindle power at one operating point, with the point it was evaluated at.
///
/// Both power figures sit on the COMMANDED axis — see the module doc.
///
/// The figure holds the model's two terms PRIVATELY. A caller that needs the
/// power at another feed, or the feed that meets a budget, asks
/// [`Self::required_kw_at_feed`] or [`Self::feed_for_kw`]; it never rebuilds
/// the model and it never reads the terms apart. That keeps
/// [`crate::tool_load::power::PowerTerms`] inside `tool_load`, where the
/// model lives, and it keeps every caller on one solver.
///
/// [`power_at_operating_point`] is the only constructor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerFigure {
    /// Predicted spindle power (kW) at the operation's feed, on the
    /// COMMANDED axis. Finite and non-negative.
    pub required_kw: f64,
    /// The gate's ceiling (kW): the rated curve `power_at_rpm(rpm)`. Finite
    /// and positive.
    pub available_kw: f64,
    /// The axial depth of cut (mm) the figure was evaluated at.
    pub ap_mm: f64,
    /// The radial width of cut (mm) the figure was evaluated at.
    pub ae_mm: f64,
    /// The spindle speed (rev/min) the figure was evaluated at.
    pub rpm: f64,
    /// The feed (mm/min) the figure was evaluated at.
    pub feed_mm_min: f64,
    /// The force line the figure was built from, at the mean chip of this
    /// operating point.
    pub force: ForceAtPoint,
    /// K7 (feeds contract P9): the case of the pre-sim power set that
    /// `required_kw` is the power of: the maximum over the cases the
    /// operation's `CutSpec` admits ([`PowerCases::of_operation`]).
    pub case: PowerCase,
    /// The radial width (mm) of [`Self::case`]: `ae_mm` for the nominal
    /// case, D for a slot, the capped width for the planner cap.
    pub case_ae_mm: f64,
    /// The model this figure was built from: the terms of [`Self::case`].
    /// The widest case is the largest at every feed (both terms grow with
    /// the width), so one set of terms answers every feed. Private on
    /// purpose — see the type doc.
    terms: PowerTerms,
}

impl PowerFigure {
    /// Predicted spindle power (kW) at `feed_mm_min`, at THIS operating
    /// point's geometry and speed, on the COMMANDED axis.
    ///
    /// `self.required_kw` is this function at `self.feed_mm_min`. Power is
    /// affine in the feed, so two evaluations determine the whole line: the
    /// value at a zero feed is the edge floor, and the slope is the shear
    /// term.
    pub fn required_kw_at_feed(&self, feed_mm_min: f64) -> f64 {
        self.terms.kw_at_feed(feed_mm_min)
    }

    /// The feed (mm/min) at which this cut draws exactly `budget_kw`, in
    /// closed form. No bisection is needed, and the answer is exact rather
    /// than converged.
    ///
    /// `None` when no feed answers the question: the feed-free edge term
    /// alone already meets or exceeds the budget, so thinning the chip
    /// cannot rescue the cut (drop the depth, the stepover or the RPM
    /// instead), or there is no shear slope at all.
    ///
    /// The answer is on the COMMANDED axis, like `budget_kw` and like
    /// `self.feed_mm_min`.
    pub fn feed_for_kw(&self, budget_kw: f64) -> Option<f64> {
        self.terms.feed_for_kw(budget_kw)
    }
}

/// Evaluate spindle power at the point `operation` ships.
///
/// # What it reads
///
/// The operation, not a `FeedsInput`. Suggest and the GUI both hold an
/// operation after the clamps have run, and that is the state the machine
/// receives:
///
/// - `ap` / `ae` — the operation's depth per pass and stepover, with
///   `fallback` for an operation that exposes no such field. The
///   surface-following finishes command no axial step, so they cannot have
///   gone stale on that axis and the calculator's own value is the correct
///   substitute.
/// - `rpm` — the operation's spindle speed, with `fallback` again.
/// - the feed — the operation's feed rate, always. A feed has no fallback
///   because every operation carries one.
/// - `effective_d` — [`crate::feeds::effective_diameter`] at the FINAL
///   depth, the chip-thinning contact circle calculator Step 5 computes. It
///   sets both the immersion angle ψ and the cutting velocity `Vc = π·D·n`.
/// - ψ — [`crate::feeds::force::immersion_angle`], one immersion definition
///   for the whole engine.
/// - the shank — `tool.shank_diameter`, falling back to the cutting
///   diameter. It reaches `effective_diameter` on the tapered-ball arm only.
///
/// `fallback` is the point `feeds::calculate` derived its feed at
/// ([`CalculatorOperatingPoint`]). Pass `None` when there is no such point;
/// an operation that carries its own depth, stepover and speed never needs
/// it.
///
/// # What it does not do
///
/// It does not clamp, warn or mutate. It answers one question — how much
/// power this cut draws, and how much the spindle has — and leaves the
/// decision to the caller.
pub fn power_at_operating_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    fallback: Option<CalculatorOperatingPoint>,
) -> Result<PowerFigure, PowerUnmodeled> {
    // P16 (review D7): no depth before the simulation, so no power figure.
    // A material with no force line keeps its `MaterialUnvalidated` reason.
    if material.force_line().is_ok() && operation.cut_spec().axial.depth_after_simulation() {
        return Err(PowerUnmodeled::DepthAfterSimulation);
    }
    let point = resolve_operating_point(operation, tool, material, Some(machine), fallback)?;

    // K7 change 3: the power of each case the CutSpec admits; the figure is
    // the maximum, with its case name.
    let terms_at = |ae: f64| {
        PowerTerms::of(
            Engagement::from_cut(
                CutPoint {
                    ap_mm: point.ap_mm,
                    ae_mm: ae,
                },
                point.geometry,
                point.effective_d,
                Some(point.helix_deg),
            )
            .power_inputs(point.force.line, point.rpm, point.flute_count),
        )
    };
    let (case, required_kw, case_ae_mm) =
        PowerCases::of_operation(operation).worst(point.ae_mm, tool.diameter, |ae| {
            terms_at(ae).kw_at_feed(point.feed_mm_min)
        });
    let terms = terms_at(case_ae_mm);

    Ok(PowerFigure {
        required_kw,
        available_kw: point.available_kw,
        ap_mm: point.ap_mm,
        ae_mm: point.ae_mm,
        rpm: point.rpm,
        feed_mm_min: point.feed_mm_min,
        force: point.force,
        case,
        case_ae_mm,
        terms,
    })
}

/// The force line of `material` at the mean chip of the point `operation`
/// ships, with no machine.
///
/// It reads the operating point the same way [`power_at_operating_point`]
/// does (the same fallbacks and the same effective diameter), so the card
/// and the power figure name one chip. The mean chip is
/// `fz · (1 − cos ψ) / ψ` with `fz = feed / (rpm · Z)` and ψ from
/// [`crate::feeds::force::immersion_angle`].
///
/// # Errors
///
/// The same [`PowerUnmodeled`] variants as [`power_at_operating_point`],
/// less [`PowerUnmodeled::NoAvailablePower`]. For the named reason of a
/// [`PowerUnmodeled::MaterialUnvalidated`], read
/// [`Material::force_line`].
pub fn force_at_operating_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    fallback: Option<CalculatorOperatingPoint>,
) -> Result<ForceAtPoint, PowerUnmodeled> {
    // P16 (item 2b): the guard of `power_at_operating_point`. No depth
    // before the simulation, so no force at D x D. A material with no force
    // line keeps its `MaterialUnvalidated` reason.
    if material.force_line().is_ok() && operation.cut_spec().axial.depth_after_simulation() {
        return Err(PowerUnmodeled::DepthAfterSimulation);
    }
    resolve_operating_point(operation, tool, material, None, fallback).map(|p| p.force)
}

/// Spindle power at the point `operation` ships, with the point
/// `recommended` was sized at as the fallback (feeds contract P14, review
/// M9). The one door of the Feeds card, MCP `get_suggest_rationale` and
/// the CLI `suggest` record: no surface builds the fallback by hand.
pub fn power_at_shipped_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    recommended: &crate::feeds::FeedsResult,
) -> Result<PowerFigure, PowerUnmodeled> {
    power_at_operating_point(
        operation,
        tool,
        material,
        machine,
        Some(CalculatorOperatingPoint::of(recommended)),
    )
}

/// The force line at the mean chip of the point `operation` ships, with
/// the point `recommended` was sized at as the fallback (P14, M9). The
/// twin of [`power_at_shipped_point`] that needs no machine.
pub fn force_at_shipped_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    recommended: &crate::feeds::FeedsResult,
) -> Result<ForceAtPoint, PowerUnmodeled> {
    force_at_operating_point(
        operation,
        tool,
        material,
        Some(CalculatorOperatingPoint::of(recommended)),
    )
}

/// P16 (item 2b): the JSON of [`crate::feeds::FeedsResult::mrr_mm3_min`]:
/// the number, or `{"unmodeled": "<reason>"}` (the `shipped_power` shape).
#[must_use]
pub fn mrr_json(mrr: &Result<f64, PowerUnmodeled>) -> serde_json::Value {
    match mrr {
        Ok(v) => serde_json::json!(v),
        Err(e) => serde_json::json!({ "unmodeled": format!("{e:?}") }),
    }
}

/// The `shipped_power` object that the CLI `suggest` record and MCP
/// `get_suggest_rationale` print (P14, M9: one shape on both surfaces).
#[must_use]
pub fn shipped_power_json(figure: &Result<PowerFigure, PowerUnmodeled>) -> serde_json::Value {
    match figure {
        Ok(p) => serde_json::json!({
            "required_kw": p.required_kw,
            "available_kw": p.available_kw,
            "ap_mm": p.ap_mm,
            "ae_mm": p.ae_mm,
            "rpm": p.rpm,
            "feed_mm_min": p.feed_mm_min,
            // K7 (feeds contract P9): the worst case of the pre-sim set.
            "case": p.case.label(),
            "case_ae_mm": p.case_ae_mm,
        }),
        Err(e) => serde_json::json!({ "unmodeled": format!("{e:?}") }),
    }
}

/// The resolved inputs of one operating point.
struct ResolvedPoint {
    force: ForceAtPoint,
    feed_mm_min: f64,
    ae_mm: f64,
    ap_mm: f64,
    rpm: f64,
    /// `power_at_rpm(rpm)`; 0.0 when no machine was given.
    available_kw: f64,
    effective_d: EngagedDiameter,
    geometry: crate::feeds::ToolGeometryHint,
    helix_deg: f64,
    flute_count: f64,
}

/// Resolve the operating point. With `machine` it also checks the ceiling,
/// in the order [`power_at_operating_point`] has always refused.
fn resolve_operating_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: Option<&MachineProfile>,
    fallback: Option<CalculatorOperatingPoint>,
) -> Result<ResolvedPoint, PowerUnmodeled> {
    let usable = |v: f64| v.is_finite() && v > 0.0;

    let Ok(line) = material.force_line() else {
        return Err(PowerUnmodeled::MaterialUnvalidated);
    };

    // S11: an unset feed is no feed.
    let Some(feed_mm_min) = operation.feed_rate().filter(|v| usable(*v)) else {
        return Err(PowerUnmodeled::NoFeed);
    };

    // K7 (feeds contract P9): a `Radial::Slot` op states its width, the
    // tool diameter (K1 `CutSpec::calc_inputs`), so a Profile, a Trace or a
    // Project Curve has a power figure without a fallback.
    let slot_width = match operation.cut_spec().radial {
        crate::compute::catalog::Radial::Slot => Some(tool.diameter),
        crate::compute::catalog::Radial::Width(_)
        | crate::compute::catalog::Radial::Groove
        | crate::compute::catalog::Radial::Scallop(_)
        | crate::compute::catalog::Radial::AngularStep(_)
        | crate::compute::catalog::Radial::SurfaceFollow
        | crate::compute::catalog::Radial::Axial => None,
    };
    let Some(ae_mm) = operation
        .stepover()
        .filter(|v| usable(*v))
        .or(slot_width.filter(|v| usable(*v)))
        .or_else(|| fallback.map(|c| c.radial_width_mm))
        .filter(|v| usable(*v))
    else {
        return Err(PowerUnmodeled::NoRadialEngagement);
    };
    let Some(ap_mm) = operation
        .depth_per_pass()
        .filter(|v| usable(*v))
        .or_else(|| fallback.map(|c| c.axial_depth_mm))
        .filter(|v| usable(*v))
    else {
        return Err(PowerUnmodeled::NoDepthPerPass);
    };

    let Some(rpm) = operation
        .spindle_rpm()
        .map(f64::from)
        .filter(|v| usable(*v))
        .or_else(|| fallback.map(|c| c.rpm).filter(|v| usable(*v)))
    else {
        return Err(PowerUnmodeled::NoSpindleSpeed);
    };

    let available_kw = match machine {
        Some(machine) => {
            let available_kw = machine.power_at_rpm(rpm);
            if !usable(available_kw) {
                return Err(PowerUnmodeled::NoAvailablePower);
            }
            available_kw
        }
        None => 0.0,
    };

    let geom = tool.geometry_hint();
    let shank = if usable(tool.shank_diameter_planning_mm()) {
        tool.shank_diameter_planning_mm()
    } else {
        tool.diameter
    };
    let effective_d = EngagedDiameter::of_hint(geom, tool.diameter, shank, ap_mm);
    if !usable(effective_d.mm()) {
        return Err(PowerUnmodeled::NoEngagementDiameter);
    }

    let flute_count = f64::from(tool.flute_count_planning().max(1));
    // K7: psi of the stated cut, from the one engagement.
    let immersion_rad = Engagement::from_cut(
        CutPoint { ap_mm, ae_mm },
        geom,
        effective_d,
        Some(tool.helix_deg),
    )
    .psi_rad();
    // Every input above is finite and positive, so the mean chip exists;
    // a degenerate arc reads as a zero chip, which is below any range.
    let fz_mm = feed_mm_min / (rpm * flute_count);
    let chip = mean_chip_mm(fz_mm, immersion_rad).unwrap_or(0.0);

    Ok(ResolvedPoint {
        force: line.at_mean_chip(chip),
        feed_mm_min,
        ae_mm,
        ap_mm,
        rpm,
        available_kw,
        effective_d,
        geometry: geom,
        helix_deg: tool.helix_deg,
        flute_count,
    })
}
