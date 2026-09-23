//! Power guardrail — per-sample instantaneous spindle power vs the rated
//! machine power curve `power_at_rpm(rpm)` (ruling R4 Q2: no fraction).
//!
//! ## The model (R1, 2026-09-16)
//!
//! Power is tangential force × cutting velocity, and the tangential
//! force this engine models is **affine** in chip thickness
//! (`feeds::force`: `Fc/ap = Ks·h + F_edge`, woodresearch.sk 201905/12,
//! R² ≈ 0.99). Splitting that force gives two power terms that behave
//! completely differently:
//!
//! ```text
//! P_kW = A · ( Ks · MRR  +  F_edge · ap · Vc · z·ψ/2π ) / 60_000_000
//!             \_________/    \____________________________/
//!              shear: ∝ feed   edge: NO feed term at all
//! ```
//!
//! with `Vc = π·D·n` (mm/min), `ψ` the engagement arc
//! ([`crate::feeds::force::immersion_angle`]), `z` the flute count and
//! `A = GRAIN_ANISOTROPY_FACTOR`.
//!
//! **The edge term contains no feed.** Halving the feed at constant RPM
//! halves the shear term and leaves the edge term untouched, so power
//! falls toward a floor rather than to zero.
//!
//! This matters because the crossover chip thickness `F_edge/Ks` is
//! 0.106 mm and wood routing runs at 0.03–0.09 mm/tooth — entirely
//! below it. Routing wood is edge-dominated, and the pre-R1 model
//! (`P = A·Kc·ap·ae·feed/60e6`, constant specific energy, linear in
//! MRR, no chip-thickness term) was blind to the dominant term. On the
//! reference fixture it understated cutting power by ~8.6× at the
//! running chipload, and the error grew as the chip thinned — exactly
//! where the warning matters. See
//! `planning/load_model_2026-09-16/ADVICE.md` §1, §2 and §6.
//!
//! The duty-cycle factor `z·ψ/2π` is the average number of teeth
//! engaged. The same expression already exists in
//! `tool::flat_chip_geometry_for_radius` as
//! `arc_engagement_radians / flute_pitch` (plus a helix-wrap term this
//! model does not carry), and `ψ` is the same immersion angle
//! `feeds::force`, `feed_modulation` and `dexel_stock::stamping` all
//! use.
//!
//! ## Why the anisotropy factor stays
//!
//! - `Kc_eff = GRAIN_ANISOTROPY_FACTOR × material.kc_n_per_mm2()`.
//!   `GRAIN_ANISOTROPY_FACTOR = 2.0` is the measured directional spread
//!   of specific cutting force for wood-class materials per Pałubicki
//!   2021 (DOI 10.3390/ma14092208, particleboard peripheral up-milling
//!   across grain orientations). Pre-Phase-2 the factor was 2.5 — a
//!   magic number chosen to absorb under-modeled `Kc`. Phase 2B paired
//!   the rename + value change with literature-anchored sheet-good Kc
//!   so the product `Kc × factor` reflects physics rather than the old
//!   absorption split.
//!   The factor is NOT shared with `feeds::force` / `tool_load::
//!   deflection`, and that split is deliberate, not a defect:
//!   deflection responds to sustained mean force and takes raw `Kc`
//!   (see `deflection.rs` module docs), while power carries a safety
//!   allowance for the transient grain spikes Pałubicki 2021 measured.
//!   R1 changed the SHAPE of this model, not its safety scoping.
//! - `radial_width = (arc_engagement_radians / π) × engagement_radius × 2`
//!   is an arc-length-equivalent slab width. Honest within isotropy
//!   bounds because Phase 2's arc engagement replaced the old cylinder-
//!   volume metric.
//!
//! Refusal cases:
//! - No simulation trace → `Unmodeled(SimulationRequired)`
//! - Trace lacks `arc_engagement_radians` (capture flag was off) →
//!   `Unmodeled(ArcEngagementNotCaptured)`
//! - `Material::Custom` without explicit Kc handling → `Unmodeled(MaterialUnvalidated)`
//!
//! Slot engagement (`arc >= π`) annotates the result with
//! `Approximate(SlotEngagement)` because chip-distribution between climb
//! and conventional sides differs there and we don't decompose.

use crate::material::Material;
use crate::tool::MillingCutter;

use super::locality::SpanLookup;
use super::verdict::{Confidence, EntrySpike, PowerVerdict, SampleEvidence, UnmodeledReason};

/// Wood grain anisotropy factor on Kc — Pałubicki 2021 (DOI
/// 10.3390/ma14092208) measured the directional spread of specific
/// cutting force for particleboard peripheral up-milling across grain
/// orientations. The rename from `ANISOTROPY_MULTIPLIER` (pre-Phase-2,
/// value 2.5) reflects that this is a documented physical factor, not
/// a knob to tune around under-modeled Kc.
///
/// Exposed as `pub(crate)` so every Kc-consuming path
/// (`session::compute` building `PowerLimitInputs`, the Suggest path
/// in `feeds::mod`, the constrained-max solver in `feed_modulation`)
/// references the single source of truth rather than re-encoding the
/// numeric literal. Bumping this constant should produce one diff
/// site, not five.
pub(crate) const GRAIN_ANISOTROPY_FACTOR: f64 = 2.0;

/// The feed-independent half of a power prediction: material, engaged
/// geometry and spindle speed. Feed is supplied separately because the
/// whole point of the two-term model is that only one term moves with
/// it — see [`PowerTerms`].
///
/// `kc_n_per_mm2` is the RAW material `Kc` from
/// `Material::kc_n_per_mm2()`. [`PowerTerms::of`] applies
/// [`GRAIN_ANISOTROPY_FACTOR`], so callers never pre-multiply and a
/// change to the factor stays one diff.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PowerModelInputs {
    /// Raw material `Kc` (N/mm²).
    pub kc_n_per_mm2: f64,
    /// Shape-correct engaged chip cross-section (mm²) at this DOC + WOC
    /// — `MillingCutter::mrr_cross_section_mm2` / the geometry hint's
    /// equivalent. Rectangular slab for endmills, triangular for
    /// V-bits. `cross_section × feed` is the MRR the shear term needs.
    pub cross_section_mm2: f64,
    /// Axial engagement `ap` (mm). `F_edge` is a force per mm of axial
    /// engagement, so the edge term scales with this and NOT with the
    /// cross-section. Kept plain (not divided by a taper's edge
    /// obliquity) to stay consistent with `feeds::force`, which does
    /// the same.
    pub axial_doc_mm: f64,
    /// Engagement (immersion) arc ψ (rad) — the simulator's
    /// `arc_engagement_radians`, or
    /// [`crate::feeds::force::immersion_angle`] from `ae/r`. Drives the
    /// duty cycle `z·ψ/2π`: how much of each revolution a tooth spends
    /// ploughing.
    pub immersion_rad: f64,
    /// Cutting diameter at the engaged depth (mm) — sets the cutting
    /// velocity `Vc = π·D·n`. For a V-bit or tapered ball this is the
    /// contact circle at the DOC, not the nominal shank.
    pub engagement_diameter_mm: f64,
    /// Spindle speed (rev/min).
    pub spindle_rpm: f64,
    /// Flute count `z`.
    pub flute_count: f64,
}

/// A power prediction split by feed dependence: `P(feed) = shear·feed +
/// edge`.
///
/// Holding the two apart is what the R1 model buys. The edge term is
/// the floor power cannot fall below at a given RPM and engagement, so
/// a solver that needs "what feed reaches this power budget" must
/// invert an affine function, not divide a ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PowerTerms {
    /// kW per mm/min of linear feed — the shear term's slope,
    /// `A·Ks·cross_section/60e6`.
    pub shear_kw_per_mm_min: f64,
    /// kW at zero feed — the edge/ploughing floor,
    /// `A·F_edge·ap·Vc·(z·ψ/2π)/60e6`.
    pub edge_kw: f64,
}

impl PowerTerms {
    /// Build the two terms from the material and the engaged geometry.
    ///
    /// The coefficients come from
    /// [`crate::feeds::force::affine_coefficients_for_kc`] — the same
    /// `(Ks, F_edge)` pair the deflection gate and the feed-modulation
    /// solver read — so power and deflection cannot disagree about what
    /// a thin chip costs. [`GRAIN_ANISOTROPY_FACTOR`] rides on both
    /// terms: it is a power-safety allowance, and it is scoped to the
    /// whole power prediction rather than to one of its halves.
    ///
    /// An edge input that is not finite and positive (no rotation, no
    /// engagement arc, no diameter) contributes **zero** edge power
    /// rather than a fabricated one. The shear term still stands, so
    /// the result degrades to the pre-R1 answer instead of refusing —
    /// the callers that can refuse already do so upstream on the same
    /// inputs.
    pub(crate) fn of(inputs: PowerModelInputs) -> Self {
        let PowerModelInputs {
            kc_n_per_mm2,
            cross_section_mm2,
            axial_doc_mm,
            immersion_rad,
            engagement_diameter_mm,
            spindle_rpm,
            flute_count,
        } = inputs;
        let (ks, f_edge) = crate::feeds::force::affine_coefficients_for_kc(kc_n_per_mm2);

        let shear_kw_per_mm_min =
            GRAIN_ANISOTROPY_FACTOR * ks * cross_section_mm2.max(0.0) / 60_000_000.0;

        // Duty cycle `z·ψ/2π` — the average number of teeth in cut.
        // ψ is clamped to a full slot; beyond π the arc is not a
        // peripheral immersion any more and the model says nothing.
        let psi = immersion_rad.clamp(0.0, std::f64::consts::PI);
        let edge_inputs_usable = psi > 0.0
            && axial_doc_mm.is_finite()
            && axial_doc_mm > 0.0
            && engagement_diameter_mm.is_finite()
            && engagement_diameter_mm > 0.0
            && spindle_rpm.is_finite()
            && spindle_rpm > 0.0
            && flute_count.is_finite()
            && flute_count > 0.0;
        let edge_kw = if edge_inputs_usable {
            let duty = flute_count * psi / std::f64::consts::TAU;
            // Vc in mm/min, so the /60e6 that turns N·mm/min into kW is
            // the same divisor the shear term uses.
            let vc_mm_min = std::f64::consts::PI * engagement_diameter_mm * spindle_rpm;
            GRAIN_ANISOTROPY_FACTOR * f_edge * axial_doc_mm * vc_mm_min * duty / 60_000_000.0
        } else {
            0.0
        };

        Self {
            shear_kw_per_mm_min,
            edge_kw,
        }
    }

    /// Predicted instantaneous spindle power (kW) at `feed_mm_min`.
    pub(crate) fn kw_at_feed(self, feed_mm_min: f64) -> f64 {
        self.shear_kw_per_mm_min * feed_mm_min.max(0.0) + self.edge_kw
    }

    /// The linear feed (mm/min) at which this cut draws exactly
    /// `budget_kw`.
    ///
    /// `None` when no feed answers the question:
    /// - the edge floor alone already meets or exceeds the budget, so
    ///   thinning the chip cannot rescue it (drop DOC, stepover or RPM
    ///   instead — this is the power-side twin of
    ///   [`crate::feeds::force::DeflectionCapRefusal::EdgeForceOverBudget`]);
    /// - there is no shear slope, so feed does not move power at all.
    ///
    /// Never returns a negative or non-finite feed.
    pub(crate) fn feed_for_kw(self, budget_kw: f64) -> Option<f64> {
        if !budget_kw.is_finite() || self.shear_kw_per_mm_min <= 0.0 {
            return None;
        }
        let headroom = budget_kw - self.edge_kw;
        if headroom <= 0.0 {
            return None;
        }
        let feed = headroom / self.shear_kw_per_mm_min;
        (feed.is_finite() && feed > 0.0).then_some(feed)
    }
}

/// Predicted instantaneous spindle power (kW) for an engaged cut at
/// `feed_mm_min`. The single canonical formula:
///
/// ```text
/// P_kW = A · ( Ks · cross_section · feed  +  F_edge · ap · π·D·n · z·ψ/2π ) / 60 000 000
/// ```
///
/// The Sim verdict (`evaluate` below), the Suggest path
/// (`feeds::calculate` Step 6) and the constrained-max solver in
/// `feed_modulation` all route through here, so the gate, the
/// recommendation and the optimizer can never predict different power
/// for the same cut.
pub(crate) fn predicted_power_kw(inputs: PowerModelInputs, feed_mm_min: f64) -> f64 {
    PowerTerms::of(inputs).kw_at_feed(feed_mm_min)
}

/// **The predicted spindle power (kW) of one simulation sample**, as the
/// power gate computes it.
///
/// The gate ([`evaluate`]) calls this function for each sample, and the
/// cut-metrics distribution ([`super::distribution`]) calls it too. Thus
/// the histogram and the gate read one number.
///
/// - `kc_n_per_mm2` is the RAW material `Kc` from
///   `Material::kc_n_per_mm2()`. Do not multiply it by the anisotropy
///   factor; [`PowerTerms::of`] applies that factor.
/// - `feed_mm_min` is the effective feed that
///   [`super::effective_feed_for_sample`] resolves for the sample.
///
/// Returns `None` when the sample is not a usable cutting sample for this
/// model: it is not cutting, it is an air cut (`radial_woc_fraction <
/// 0.02`), it carries no arc engagement, or the engaged radial width is
/// not positive. The gate skips such a sample. A `Some` value does not
/// say that the gate COUNTED the sample: the gate also removes transit
/// samples through [`super::locality::is_steady_state_for_gate`].
#[must_use]
pub fn sample_power_kw(
    tool: &crate::tool::ToolDefinition,
    kc_n_per_mm2: f64,
    sample: &crate::stock::simulation_cut::SimulationCutSample,
    feed_mm_min: f64,
) -> Option<f64> {
    if !sample.is_cutting || sample.engagement.radial_woc_fraction < 0.02 {
        return None;
    }
    let arc = sample.arc_engagement_radians?;
    // Power formula. Arc-equivalent radial slab width:
    //   radial_width = (arc / π) × engagement_radius × 2
    // For a half-engagement (arc = π/2), this gives `engagement_radius`.
    // For a slot (arc = π), it gives 2× engagement_radius — the full
    // tool diameter — which is the correct engaged width for slotting.
    let engagement_radius = tool.engagement_radius(sample.axial_doc_mm).max(0.0);
    let radial_width = (arc / std::f64::consts::PI) * engagement_radius * 2.0;
    if radial_width <= 0.0 {
        return None;
    }
    // Engaged chip cross-section is shape-dependent: rectangular slab
    // for endmills, triangular groove for V-bits. The cutter owns that
    // geometry; the gate owns the material + machine physics. Route
    // through the canonical `predicted_power_kw` helper so this gate
    // and the Suggest path (`feeds::calculate`) can't diverge.
    //
    // R1: the edge term needs the sample's own rotation and arc, not
    // just its swept volume. `arc` IS ψ — the same immersion angle
    // `dexel_stock::stamping` derived it from — and the engaged
    // diameter is twice the engagement radius already computed above.
    let cross_section_mm2 = tool.mrr_cross_section_mm2(sample.axial_doc_mm, radial_width);
    Some(predicted_power_kw(
        PowerModelInputs {
            kc_n_per_mm2,
            cross_section_mm2,
            axial_doc_mm: sample.axial_doc_mm,
            immersion_rad: arc,
            engagement_diameter_mm: engagement_radius * 2.0,
            spindle_rpm: f64::from(sample.spindle_rpm),
            // The sample's own flute count, falling back to the
            // tool's when the trace left it at zero — a zero would
            // silently zero the edge term, which is the
            // absence-rendered-as-a-reading shape this model exists
            // to remove.
            flute_count: if sample.flute_count > 0 {
                f64::from(sample.flute_count)
            } else {
                f64::from(tool.flute_count)
            },
        },
        feed_mm_min,
    ))
}

#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id.0, op = ?ctx.operation_kind))]
pub fn evaluate(ctx: &super::ToolpathLoadContext<'_>, env: &super::GateEnv<'_>) -> PowerVerdict {
    let &super::ToolpathLoadContext {
        toolpath_id,
        tool,
        material,
        operation_kind,
        spans,
        ..
    } = ctx;
    let &super::GateEnv {
        sim_trace,
        machine,
        tolerance,
    } = env;
    // The power gate is the only one that reads a machine profile.
    // Checked first (before the drill short-circuit) to preserve the
    // pre-Phase-6 behaviour, where `evaluate_toolpath` routed a missing
    // machine to this refusal without ever entering the gate.
    let Some(machine) = machine else {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "machine profile not provided to evaluator".to_owned(),
            ),
        };
    };
    // Roadmap F.8 — short-circuit before any gate-input checks when
    // the op is geometrically plunge-only. Drilling has no continuous
    // engagement to drive a power-vs-RPM curve; the right answer is
    // "doesn't apply" not "arc engagement not captured".
    if operation_kind.is_drill_kinematics() {
        tracing::debug!(
            reason = "NotApplicableForOp",
            "power gate refuses: plunge-only op has no continuous engagement"
        );
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        };
    }
    let Some(trace) = sim_trace else {
        tracing::debug!(
            reason = "SimulationRequired",
            "power gate refuses: no simulation trace"
        );
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };

    // Material::Custom without an explicitly-validated Kc: refuse. The
    // `kc_n_per_mm2` accessor on Custom returns whatever the user typed;
    // unless a project-level "validated" flag exists, the safest default
    // is to refuse rather than predict force from an unvetted constant.
    if let Material::Custom { .. } = material {
        tracing::debug!(
            reason = "MaterialUnvalidated",
            material = "Custom",
            "power gate refuses: Custom material has no validated Kc"
        );
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }

    // Materials without a primary-source Kc (e.g. most plastics, aluminum
    // pre Phase 3 beat F) return None and refuse here. The type-level
    // Option encodes "no validated cutting-force model" — no fabricated
    // constant ever drives a force prediction.
    let Some(kc) = material.kc_n_per_mm2() else {
        tracing::debug!(
            reason = "MaterialUnvalidated",
            material = %material.label(),
            "power gate refuses: material has no primary-source Kc"
        );
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    };

    // Walk samples for this toolpath.
    let mut peak_power: f64 = 0.0;
    let mut peak_idx: Option<usize> = None;
    let mut any_arc_captured = false;
    let mut any_slot = false;
    let mut peak_available_at_peak: f64 = 0.0;
    // Track an available_kw for any captured sample so the Within case
    // (which often falls through with `peak_power == 0.0` on light cuts)
    // still has a usable headroom number to surface.
    let mut last_available_kw: f64 = 0.0;
    // S4: the rpm that built `last_available_kw` / `peak_available_at_peak`.
    // The rpm has to be carried out with `available_kw`, or the row cannot
    // state where its ceiling came from.
    let mut last_rpm: f64 = 0.0;
    let mut peak_rpm_at_peak: f64 = 0.0;
    // D7 — span-aware entry filter. Configured Entry transients
    // (Adaptive3D plunge / helix / ramp, dressup lead-ins) bypass the
    // trip decision and surface separately as `entry_spike` on the
    // `Within` arm. The peak Entry-ancestry power sample (whether or
    // not it exceeds available_kw) is recorded for the advisory.
    let span_lookup = spans.map(SpanLookup::new);
    let mut entry_peak_power: f64 = 0.0;
    let mut entry_peak_idx: Option<usize> = None;
    let mut entry_peak_available: f64 = 0.0;
    // X-VAC (`planning/review_2026-08-08/XVAC_CENSUS.md`). `offered`
    // counts every sample this toolpath emitted; `contributing` counts
    // the samples that reached the peak comparison. The 2026-08-05
    // measurement is exactly the case where the second is zero while the
    // first is large: `any_arc_captured` is set BEFORE the phantom /
    // entry split below, so a toolpath whose every cutting sample is
    // transit reaches the `Within` arm with `peak_idx == None`,
    // `available_kw == 0.0` and empty evidence — a healthy-looking pass
    // resting on nothing.
    let mut offered: usize = 0;
    let mut contributing: usize = 0;

    for (i, s) in trace.samples.iter().enumerate() {
        if s.toolpath_id != toolpath_id {
            continue;
        }
        offered += 1;
        if !s.is_cutting {
            continue;
        }
        if s.engagement.radial_woc_fraction < 0.02 {
            continue;
        }
        let Some(arc) = s.arc_engagement_radians else {
            continue;
        };
        any_arc_captured = true;

        // F-035: read the *effective* feed for this sample —
        // predicted (achieved) when the trace carries a populated
        // `predicted_feeds` map AND this `(toolpath_id, move_index)`
        // lookup hits, commanded otherwise. Power is linear in
        // feed, so the substitution scales the predicted load
        // proportionally; corner-decel reduction in predicted feed
        // shows up directly as reduced predicted power.
        let feed_for_power = super::effective_feed_for_sample(s, &trace.predicted_feeds);
        // The per-sample prediction lives in `sample_power_kw`, so the
        // cut-metrics distribution (`super::distribution`) reads the
        // same number this gate compares.
        let Some(p_kw) = sample_power_kw(tool, kc, s, feed_for_power) else {
            continue;
        };
        // Ruling R4 Q2 (2026-09-24): the ceiling is the rated spindle curve,
        // with no fraction.
        let avail = machine.power_at_rpm(s.spindle_rpm as f64);

        // Finding 3 split (2026-06-04): phantom-transit samples
        // (WaterlineCleanup / LinkBridge / LeadOut / DressupArtifact)
        // carry inflated dexel axial_doc, so the predicted cutting
        // power is phantom — drop them entirely. Configured Entry
        // samples (real plunge / ramp / helix transients) still route
        // to the entry_spike advisory.
        if super::locality::is_phantom_transit(s, span_lookup.as_ref()) {
            continue;
        }
        if super::locality::is_configured_entry(s, span_lookup.as_ref()) {
            if p_kw > entry_peak_power {
                entry_peak_power = p_kw;
                entry_peak_idx = Some(i);
                entry_peak_available = avail;
            }
            continue;
        }

        if arc >= std::f64::consts::PI - 1e-3 {
            any_slot = true;
        }

        contributing += 1;
        last_available_kw = avail;
        last_rpm = f64::from(s.spindle_rpm);

        if p_kw > peak_power {
            peak_power = p_kw;
            peak_idx = Some(i);
            peak_available_at_peak = avail;
            peak_rpm_at_peak = f64::from(s.spindle_rpm);
        }
    }

    if !any_arc_captured {
        // No samples carried arc data — likely capture_arc_engagement was
        // off when the trace was recorded.
        tracing::debug!(
            reason = "ArcEngagementNotCaptured",
            "power gate refuses: trace lacks arc_engagement_radians on all samples"
        );
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::ArcEngagementNotCaptured,
        };
    }

    let confidence = if any_slot {
        Confidence::Approximate(
            "slot engagement (arc >= π) — climb/conventional split not modeled".to_owned(),
        )
    } else {
        Confidence::Approximate(
            "isotropic Kc with 2.0× grain anisotropy factor (Pałubicki 2021); no helix/grain decomposition".to_owned(),
        )
    };

    // X-VAC: state the population on BOTH arms — `Exceeds` needs it too,
    // so a consumer can always ask the same question of any verdict
    // rather than inferring "populated" from the state.
    let population = super::verdict::GatePopulation::new(
        contributing,
        offered,
        super::verdict::PopulationUnit::Samples,
    );
    let evidence =
        match peak_idx {
            Some(idx) => SampleEvidence::at(idx)
                .with_locality(trace.samples.get(idx).and_then(|s| {
                    super::locality::classify_sample_locality(s, span_lookup.as_ref())
                }))
                .with_population(population),
            None => SampleEvidence::empty().with_population(population),
        };

    // available_kw on the verdict reflects the capacity at the worst
    // sample we saw. Falls back to the last captured sample's available
    // power when no peak was set (light cuts where p_kw never exceeded
    // 0). Both arms set `available_kw` so consumers can render the
    // headroom band uniformly.
    let available_kw = if peak_available_at_peak > 0.0 {
        peak_available_at_peak
    } else {
        last_available_kw
    };
    // S4: the provenance of `available_kw`, from the same sample that
    // set it. `None` when no sample set one, so the row states no
    // source rather than one built from a zero rpm.
    let bound_rpm = if peak_available_at_peak > 0.0 {
        peak_rpm_at_peak
    } else {
        last_rpm
    };
    let bound_source = (available_kw > 0.0 && bound_rpm > 0.0)
        .then_some(super::verdict::BoundSource::MachinePowerCurve { rpm: bound_rpm });

    // Layer 1 tolerance band: widen the peak-vs-available trigger by
    // `power_breach`. Default is 0 (preserves the strict machine-ceiling
    // behaviour) — see `ToleranceBands::power_breach` doc.
    // Checkpoint K (b1) — through the shared boundary contract, so the
    // trigger is `bound × (1 + dial)` widened by the boundary epsilon in
    // exactly one place. This gate's observation is integrated rather
    // than reconstructed, so it does not exhibit G-CHIP-ULP; it is
    // inside the contract because "has no epsilon because nothing has
    // bitten yet" is not a contract.
    if peak_available_at_peak > 0.0
        && super::boundary::exceeds_high(peak_power, peak_available_at_peak, tolerance.power_breach)
    {
        tracing::warn!(
            verdict = "Exceeds",
            peak_kw = peak_power,
            available_kw = peak_available_at_peak,
            ratio = peak_power / peak_available_at_peak,
            "power gate Exceeds: peak instantaneous spindle power exceeds the rated curve"
        );
        return PowerVerdict::Exceeds {
            peak_kw: peak_power,
            available_kw,
            evidence,
            confidence,
            bound_source,
        };
    }
    // D7 entry-spike advisory: surface a configured-entry sample whose
    // power exceeded the (possibly tolerance-widened) machine ceiling,
    // even though the steady-state trip didn't fire.
    let entry_spike = match entry_peak_idx {
        // Checkpoint K (b1) — the entry-spike advisory reads the same
        // boundary contract as the trip, at zero tolerance.
        Some(idx)
            if entry_peak_available > 0.0
                && super::boundary::exceeds_high(entry_peak_power, entry_peak_available, 0.0) =>
        {
            let locality = trace
                .samples
                .get(idx)
                .and_then(|s| super::locality::classify_sample_locality(s, span_lookup.as_ref()))
                .unwrap_or_else(|| "entry".to_owned());
            Some(EntrySpike {
                observed: entry_peak_power,
                bound: entry_peak_available,
                locality,
                side: None,
            })
        }
        _ => None,
    };
    PowerVerdict::Within {
        peak_kw: peak_power,
        available_kw,
        evidence,
        confidence,
        entry_spike,
        bound_source,
    }
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
    use crate::compute::catalog::OperationType;
    use crate::ids::ToolpathId;
    use crate::machine::MachineProfile;
    use crate::material::WoodSpecies;
    use crate::stock::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
    use crate::tool::ToolDefinition;

    /// Adapts this module's legacy positional-arg test calls to the
    /// Phase 6 `(ctx, env)` gate signature.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_args(
        toolpath_id: usize,
        tool: &crate::tool::ToolDefinition,
        material: &crate::material::Material,
        machine: &crate::machine::MachineProfile,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
        spans: Option<&[crate::trace::toolpath_spans::Span]>,
        operation_kind: crate::compute::catalog::OperationType,
        tolerance: &crate::tool_load::ToleranceBands,
    ) -> PowerVerdict {
        evaluate(
            &crate::tool_load::ToolpathLoadContext {
                toolpath_id: ToolpathId(toolpath_id),
                tool,
                material,
                operation_family: crate::feeds::vendor_lut::LutOperationFamily::Pocket,
                pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
                operation_feed_rate_mm_min: 0.0,
                operation_kind,
                spans,
                drill_op: None,
            },
            &crate::tool_load::GateEnv {
                sim_trace,
                machine: Some(machine),
                tolerance,
            },
        )
    }
    use crate::tool::{FlatEndmill, VBitEndmill};

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.35, 20.0)),
            6.35,
            30.0,
            20.0,
            30.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn vbit_tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(VBitEndmill::new(6.35, 90.0, 20.0)),
            6.35,
            30.0,
            20.0,
            30.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn shapeoko_makita() -> MachineProfile {
        MachineProfile::shapeoko_makita()
    }

    fn cutting_sample(idx: usize, axial: f64, arc_rad: f64, feed_mmpm: f64) -> SimulationCutSample {
        SimulationCutSample {
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, -axial],
            cumulative_time_s: 0.1 * idx as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: feed_mmpm,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: axial,
            axial_engagement_mm: axial,
            arc_engagement_radians: Some(arc_rad),
            chipload_mm_per_tooth: feed_mmpm / (18_000.0 * 2.0),
            effective_chip_thickness_mm: Some(feed_mmpm / (18_000.0 * 2.0)),
            engagement: crate::stock::simulation_cut::Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        }
    }

    fn trace_with(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            sample_step_mm: 1.0,
            summary: SimulationCutSummary {
                sample_count: samples.len(),
                toolpath_count: 1,
                issue_count: 0,
                hotspot_count: 0,
                total_runtime_s: 1.0,
                cutting_runtime_s: 1.0,
                rapid_runtime_s: 0.0,
                air_cut_time_s: 0.0,
                low_engagement_time_s: 0.0,
                average_engagement: 0.5,
                peak_chipload_mm_per_tooth: 0.05,
                peak_axial_doc_mm: 1.0,
                peak_plunge_descent_mm: 0.0,
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
                per_kinematics: std::collections::BTreeMap::new(),
                runtime_by_intent: None,
            },
            samples,
            ..SimulationCutTrace::test_fixture()
        }
    }

    #[test]
    fn no_trace_returns_simulation_required() {
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            &shapeoko_makita(),
            None,
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    fn no_arc_data_returns_arc_engagement_not_captured() {
        let mut s = cutting_sample(0, 1.0, std::f64::consts::FRAC_PI_2, 1000.0);
        s.arc_engagement_radians = None;
        let trace = trace_with(vec![s]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::ArcEngagementNotCaptured
            }
        ));
    }

    #[test]
    fn plastic_without_validated_kc_refuses_material_unvalidated() {
        // Acrylic has no fetched primary force study; the gate must
        // refuse rather than predict force from a fabricated constant.
        let trace = trace_with(vec![cutting_sample(
            0,
            1.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
        )]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::Plastic {
                family: crate::material::PlasticFamily::Acrylic,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::MaterialUnvalidated
            }
        ));
    }

    #[test]
    fn custom_material_returns_material_unvalidated() {
        let trace = trace_with(vec![cutting_sample(
            0,
            1.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
        )]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::test_fixture_custom("Mystery"),
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::MaterialUnvalidated
            }
        ));
    }

    #[test]
    fn light_cut_is_within_with_available_kw() {
        // 6.35mm flat in hard maple, half-engagement (arc=π/2), 1mm DOC,
        // 1000 mm/min feed, 18 000 RPM, 2 flutes, HardMaple (Kc = 43.2):
        //   engagement_radius = 3.175
        //   radial_width = (π/2 / π) × 3.175 × 2 = 3.175
        //   cross_section   = 1.0 × 3.175 = 3.175 mm²
        //   (Ks, F_edge)    = (49.95, 5.30) × 43.2/35.1 = (61.48, 6.523)
        //   shear = 2.0 × 61.48 × 3.175 × 1000 / 60e6   = 0.00651 kW
        //   edge  = 2.0 × 6.523 × 1.0 × (π×6.35×18000) × (2×(π/2)/2π) / 60e6
        //                                                = 0.03904 kW
        //   P_kW  = 0.04554 kW
        // R1 (2026-09-16) re-baseline: the pre-R1 linear model read
        // 0.00229 kW here (Kc_eff 86.4 × 3.175 × 1000 / 60e6). The edge
        // term is 86 % of the honest answer at this thin chip — it is
        // the term the old model could not see. Bound widened from
        // 0.01 to 0.1 kW; still well under the ~0.71 kW ceiling,
        // so "light cut" still means light.
        // Shapeoko Makita ≈ 0.71 kW rated (R4 Q2: no fraction). Within, and the
        // verdict must surface the available headroom for UI rendering.
        let trace = trace_with(vec![cutting_sample(
            0,
            1.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
        )]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            PowerVerdict::Within {
                peak_kw,
                available_kw,
                ..
            } => {
                assert!(peak_kw > 0.0 && peak_kw < 0.1, "peak power {peak_kw} kW");
                assert!(
                    available_kw > 0.0,
                    "Within must carry available_kw for headroom rendering, got {available_kw}"
                );
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn heavy_cut_exceeds_machine_with_available_kw() {
        // Slot at 20mm DOC, 6000 mm/min in Ipe (Kc=28) → P ≈ 0.889 kW
        // vs the rated ≈ 0.71 kW. Exceeds, and the verdict
        // must carry both peak_kw and available_kw.
        let trace = trace_with(vec![cutting_sample(0, 20.0, std::f64::consts::PI, 6000.0)]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            PowerVerdict::Exceeds {
                peak_kw,
                available_kw,
                ..
            } => {
                assert!(peak_kw > available_kw, "exceedance must hold by definition");
                assert!(available_kw > 0.0, "available_kw must be populated");
            }
            other => panic!("expected Exceeds, got {other:?}"),
        }
    }

    /// R1 (2026-09-16) re-baseline. The claim used to be "the V-bit's
    /// power is half the flat's, because its cross-section is half".
    /// That is now true of the SHEAR term only: the edge term is
    /// `F_edge · ap · Vc · z·ψ/2π` and reads no cross-section at all, so
    /// halving the removed area does not halve the power. The test
    /// therefore pins both terms — the cross-section model is still
    /// checked, and the claim it makes is now the true one.
    #[test]
    fn vbit_triangular_cross_section_halves_the_shear_term() {
        // Same DOC / arc / feed on a 6.35 mm flat vs a 6.35 mm 90° V-bit.
        // The flat removes a rectangular slab; the V-bit removes a
        // triangular groove of half the area. (engagement_radius differs
        // by shape: a 90° V-bit's width_at_height(1.0) = 1.0 ≠ flat's
        // 3.175 — so this asserts the V-bit's own hand-computed power
        // rather than a ratio against the flat.)
        let doc = 1.0;
        let arc = std::f64::consts::FRAC_PI_2;
        let feed = 1000.0;
        let trace = trace_with(vec![cutting_sample(0, doc, arc, feed)]);
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let v = evaluate_args(
            0,
            &vbit_tool(),
            &mat,
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        let peak = match v {
            PowerVerdict::Within { peak_kw, .. } | PowerVerdict::Exceeds { peak_kw, .. } => peak_kw,
            other => panic!("expected modeled verdict, got {other:?}"),
        };
        // Hand compute: engagement_radius(1.0) for 90° V-bit = 1.0 mm;
        // radial_width = (arc/π)·2·1.0 = 1.0; triangular area = 0.5·1·1 =
        // 0.5 mm²; engaged diameter = 2.0 mm. HardMaple MILLING Kc =
        // 16 × 2.7 = 43.2 (FPL shear lifted by MILLING_KC_FACTOR,
        // KC_MILLING_CALIBRATION_2026-06-17) ⇒ (Ks, F_edge) =
        // (61.48, 6.523). A = GRAIN_ANISOTROPY_FACTOR = 2.0.
        //   shear = 2.0 · 61.48 · 0.5 · 1000 / 60e6         = 0.001025 kW
        //   edge  = 2.0 · 6.523 · 1.0 · (π·2·18000) · 0.5 / 60e6
        //                                                   = 0.012296 kW
        // Pre-R1 this fixture read 86.4 · 0.5 · 1000 / 60e6 = 0.00072 kW.
        let (ks, f_edge) = crate::feeds::force::affine_coefficients_for_kc(43.2);
        let shear = GRAIN_ANISOTROPY_FACTOR * ks * 0.5 * feed / 60_000_000.0;
        let vc = std::f64::consts::PI * 2.0 * 18_000.0;
        let edge = GRAIN_ANISOTROPY_FACTOR * f_edge * doc * vc * 0.5 / 60_000_000.0;
        let expected = shear + edge;
        assert!(
            (peak - expected).abs() / expected < 0.02,
            "V-bit two-term power {peak} should match {expected}"
        );
        // The cross-section model is what this test exists to check: the
        // triangular groove must still halve the SHEAR term against a
        // rectangular slab of the same DOC × radial width.
        let slab_shear = GRAIN_ANISOTROPY_FACTOR * ks * 1.0 * feed / 60_000_000.0;
        assert!(
            (shear - 0.5 * slab_shear).abs() < 1e-12,
            "the triangular cross-section must halve the shear term"
        );
    }

    #[test]
    fn slot_annotates_approximate() {
        let trace = trace_with(vec![cutting_sample(0, 1.0, std::f64::consts::PI, 1000.0)]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            PowerVerdict::Within {
                confidence: Confidence::Approximate(reason),
                ..
            } => assert!(reason.contains("slot"), "got {reason}"),
            other => panic!("expected Within(Approximate(slot ...)), got {other:?}"),
        }
    }

    /// A borderline cut that Exceeds the strict ceiling (power_breach = 0)
    /// but lands Within once `power_breach` is set generously. Confirms the
    /// tolerance band actually widens the gate trigger; the default
    /// preserves today's strict ceiling.
    ///
    /// Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7): the
    /// old fixture (Ipe full slot, 20 mm DOC, 6000 mm/min) predicted
    /// ~1.92 kW — beyond reach even with power_breach = 1.0 (which admits
    /// up to 2× the 0.568 kW available). The test's intent is "the band
    /// admits a *borderline* cut", not "Ipe full slot is fine". It was
    /// retuned then to HardMaple at 14 mm DOC ≈ 0.77 kW.
    ///
    /// R1 (2026-09-16) re-baseline: the two-term model reads that same
    /// 14 mm / 6000 mm/min fixture as 2.19 kW, so the probe left the band
    /// again — the fixture moved, the assertion did not. A full slot runs
    /// the edge term at its maximum duty cycle (ψ = π ⇒ z·ψ/2π = 1.0),
    /// and at 18 000 RPM that term alone is 0.078 kW per mm of DOC.
    /// Retuned to 7 mm DOC at 3000 mm/min ≈ 0.82 kW (edge 0.547 + shear
    /// 0.273): above the 0.568 kW strict ceiling, under the 1.136 kW
    /// widened one. Ruling R4 Q2 (2026-09-24) raised the strict ceiling to
    /// the rated 0.71 kW (widened 1.42 kW); 0.82 kW is still between them. Both halves of "borderline" are asserted now, so a
    /// future drift cannot leave this passing vacuously.
    #[test]
    fn heavy_cut_within_with_power_breach_tolerance() {
        let trace = trace_with(vec![cutting_sample(0, 7.0, std::f64::consts::PI, 3000.0)]);
        let material = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let bands = crate::tool_load::ToleranceBands {
            power_breach: 1.0, // widen by 100 % so the borderline probe lands Within
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate_args(
            0,
            &tool(),
            &material,
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &bands,
        );
        assert!(
            matches!(v, PowerVerdict::Within { .. }),
            "expected Within with power_breach=1.0, got {v:?}"
        );
        // Non-vacuity: the same cut must Exceed the STRICT ceiling, else
        // the widened band proved nothing.
        let strict = evaluate_args(
            0,
            &tool(),
            &material,
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(strict, PowerVerdict::Exceeds { .. }),
            "the probe must be borderline — Exceeds at the default band, got {strict:?}"
        );
    }

    /// Roadmap F.8 — drill / pin-drill operations short-circuit to
    /// `NotApplicableForOp` before any trace inspection. Even with a
    /// fully-populated trace at heavy load (the same fixture as
    /// `heavy_cut_exceeds_machine_with_available_kw`), the power gate
    /// must refuse with the "doesn't apply" reason, not Exceeds.
    #[test]
    fn drill_op_routes_to_not_applicable_regardless_of_trace() {
        let trace = trace_with(vec![cutting_sample(0, 20.0, std::f64::consts::PI, 6000.0)]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::Drill,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::NotApplicableForOp(detail),
            } => {
                assert!(
                    detail.contains("drill"),
                    "NotApplicableForOp detail should name the op family, got: {detail}"
                );
            }
            other => panic!("drill must route to Unmodeled(NotApplicableForOp), got {other:?}"),
        }
    }

    /// Same for alignment-pin drilling — also plunge-only kinematics.
    #[test]
    fn alignment_pin_drill_routes_to_not_applicable() {
        let trace = trace_with(vec![cutting_sample(
            0,
            1.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
        )]);
        let v = evaluate_args(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            &shapeoko_makita(),
            Some(&trace),
            None,
            OperationType::AlignmentPinDrill,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            PowerVerdict::Unmodeled {
                reason: UnmodeledReason::NotApplicableForOp(_)
            }
        ));
    }
}

/// R1 (2026-09-16) — the two-term power model, pinned on the reference
/// fixture of `planning/load_model_2026-09-16/ADVICE.md` §1.
///
/// These arms are the re-baseline record for R1. They are separate from
/// the gate tests above because they judge the MODEL, not the verdict
/// plumbing, and because the pre-R1 linear formula is written out here
/// in full so the before/after is readable without reading git history.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod r1_two_term_model {
    use super::*;
    use crate::material::{Material, WoodSpecies};

    /// 6 mm 2-flute flat, 17 000 RPM, DOC 4.20, WOC 2.10, generic
    /// softwood — the fixture ADVICE.md §1 measured.
    fn fixture() -> (PowerModelInputs, f64) {
        let kc = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
        .kc_n_per_mm2()
        .unwrap();
        let (d, ap, ae) = (6.0_f64, 4.20_f64, 2.10_f64);
        (
            PowerModelInputs {
                kc_n_per_mm2: kc,
                cross_section_mm2: ap * ae,
                axial_doc_mm: ap,
                immersion_rad: crate::feeds::force::immersion_angle(ae, d / 2.0),
                engagement_diameter_mm: d,
                spindle_rpm: 17_000.0,
                flute_count: 2.0,
            },
            kc,
        )
    }

    /// The PRE-R1 model, written out so the before/after ratio below is
    /// a measurement against a stated baseline rather than a memory.
    fn linear_power_kw(kc: f64, cross_section_mm2: f64, feed_mm_min: f64) -> f64 {
        GRAIN_ANISOTROPY_FACTOR * kc * cross_section_mm2 * feed_mm_min / 60_000_000.0
    }

    /// R1's headline measurement, re-taken from live code.
    ///
    /// | fz (mm/tooth) | pre-R1 kW | R1 kW | ratio | edge share |
    /// |---|---|---|---|---|
    /// | 0.0380 (running) | 0.006666 | 0.057399 | **8.61×** | 83.5 % |
    /// | 0.0675 (vendor mid) | 0.011842 | 0.064763 | 5.47× | 74.0 % |
    /// | 0.0850 (vendor max) | 0.014912 | 0.069132 | 4.64× | 69.3 % |
    ///
    /// ADVICE.md §1's table quoted 4.3× / 2.7× / 2.3× because it
    /// computed the two-term column WITHOUT `GRAIN_ANISOTROPY_FACTOR`;
    /// §6 corrects that — the factor stays on power and rides both
    /// terms, so the change is twice §1's number.
    #[test]
    fn the_understatement_grows_as_the_chip_thins() {
        let (inputs, kc) = fixture();
        let terms = PowerTerms::of(inputs);
        let expected = [
            (0.0380_f64, 0.006_666_f64, 0.057_399_f64, 8.61_f64),
            (0.0675, 0.011_842, 0.064_763, 5.47),
            (0.0850, 0.014_912, 0.069_132, 4.64),
        ];
        let mut previous_ratio = f64::INFINITY;
        for (fz, want_old, want_new, want_ratio) in expected {
            let feed = fz * inputs.spindle_rpm * inputs.flute_count;
            let old = linear_power_kw(kc, inputs.cross_section_mm2, feed);
            let new = terms.kw_at_feed(feed);
            assert!(
                (old - want_old).abs() < 1e-5,
                "pre-R1 baseline moved at fz {fz}: {old} vs {want_old}"
            );
            assert!(
                (new - want_new).abs() < 1e-5,
                "R1 power moved at fz {fz}: {new} vs {want_new}"
            );
            let ratio = new / old;
            assert!(
                (ratio - want_ratio).abs() < 0.01,
                "understatement ratio moved at fz {fz}: {ratio} vs {want_ratio}"
            );
            // Non-vacuity: the whole point is that the error GROWS as
            // the chip thins. A model that merely scaled power up by a
            // constant would pass the three cells above and fail here.
            assert!(
                ratio < previous_ratio,
                "the ratio must fall as chipload rises; {ratio} did not beat {previous_ratio}"
            );
            previous_ratio = ratio;
        }
    }

    /// The finding, stated as behaviour: halving the feed at constant
    /// RPM does NOT halve power, because the edge term carries no feed.
    /// Power falls toward a floor. The pre-R1 model said it falls
    /// linearly to zero.
    #[test]
    fn halving_the_feed_does_not_halve_the_power() {
        let (inputs, kc) = fixture();
        let terms = PowerTerms::of(inputs);
        let feed = 0.0675 * inputs.spindle_rpm * inputs.flute_count;

        let full = terms.kw_at_feed(feed);
        let half = terms.kw_at_feed(feed / 2.0);
        assert!(
            half > 0.6 * full,
            "power at half feed ({half}) collapsed like the linear model would;              the edge floor is {} kW",
            terms.edge_kw
        );
        // At zero feed the shear term is gone and the edge floor stands.
        assert!(
            (terms.kw_at_feed(0.0) - terms.edge_kw).abs() < 1e-12,
            "zero feed must leave exactly the edge floor"
        );
        assert!(terms.edge_kw > 0.0, "the fixture must have an edge term");

        // Non-vacuity against the baseline: the pre-R1 model DID halve.
        let old_full = linear_power_kw(kc, inputs.cross_section_mm2, feed);
        let old_half = linear_power_kw(kc, inputs.cross_section_mm2, feed / 2.0);
        assert!(
            (old_half / old_full - 0.5).abs() < 1e-12,
            "the stated baseline must be linear in feed, else the contrast is empty"
        );
    }

    /// RPM moves the edge term and only the edge term — `Vc = π·D·n`.
    /// The pre-R1 model did not read RPM at all.
    #[test]
    fn rpm_moves_the_edge_term_and_not_the_shear_slope() {
        let (inputs, _) = fixture();
        let slow = PowerTerms::of(inputs);
        let fast = PowerTerms::of(PowerModelInputs {
            spindle_rpm: inputs.spindle_rpm * 2.0,
            ..inputs
        });
        assert!(
            (fast.edge_kw - 2.0 * slow.edge_kw).abs() < 1e-12,
            "the edge term is linear in RPM through Vc"
        );
        assert!(
            (fast.shear_kw_per_mm_min - slow.shear_kw_per_mm_min).abs() < 1e-18,
            "the shear slope is per mm/min of feed and must not read RPM"
        );
    }

    /// The affine inversion round-trips, and refuses rather than
    /// returning a fabricated feed when the edge floor alone is over
    /// budget.
    #[test]
    fn feed_for_kw_inverts_the_model_and_refuses_below_the_edge_floor() {
        let (inputs, _) = fixture();
        let terms = PowerTerms::of(inputs);

        let budget = terms.edge_kw * 1.5;
        let feed = terms
            .feed_for_kw(budget)
            .expect("a budget above the edge floor has a feed answer");
        assert!(
            (terms.kw_at_feed(feed) - budget).abs() < 1e-12,
            "feed_for_kw must land exactly on the budget"
        );

        // Below the floor there is no feed. Not zero, not the floor —
        // no answer, because thinning the chip cannot buy power back.
        assert!(
            terms.feed_for_kw(terms.edge_kw * 0.9).is_none(),
            "a budget under the edge floor has no feed answer"
        );
        assert!(
            terms.feed_for_kw(terms.edge_kw).is_none(),
            "a budget exactly at the edge floor leaves no shear headroom"
        );
    }

    /// The model degrades to the shear term rather than fabricating an
    /// edge term when the engagement inputs are absent — and the
    /// degraded answer is exactly the pre-R1 number, so the fallback is
    /// identifiable rather than merely small.
    #[test]
    fn missing_engagement_inputs_drop_the_edge_term_rather_than_invent_one() {
        let (inputs, kc) = fixture();
        let feed = 2000.0;
        for broken in [
            PowerModelInputs {
                immersion_rad: 0.0,
                ..inputs
            },
            PowerModelInputs {
                spindle_rpm: 0.0,
                ..inputs
            },
            PowerModelInputs {
                flute_count: 0.0,
                ..inputs
            },
            PowerModelInputs {
                engagement_diameter_mm: 0.0,
                ..inputs
            },
        ] {
            let terms = PowerTerms::of(broken);
            assert!(terms.edge_kw.abs() < 1e-18, "edge term must be zero");
            // Ks is the affine slope, not Kc, so the degraded answer is
            // the pre-R1 formula with Ks in Kc's place — NOT equal to
            // the old number. Stated explicitly so nobody reads this
            // fallback as "R1 off".
            let (ks, _) = crate::feeds::force::affine_coefficients_for_kc(kc);
            let want =
                GRAIN_ANISOTROPY_FACTOR * ks * inputs.cross_section_mm2 * feed / 60_000_000.0;
            assert!((terms.kw_at_feed(feed) - want).abs() < 1e-12);
        }
    }
}
