//! Tip-deflection guardrail — predicts how far the tool tip wanders
//! under cutting load.
//!
//! For each cutting sample of the toolpath, computes the transverse
//! cutting force from the canonical feed-aware affine model
//! ([`crate::feeds::force::lateral_cutting_force`]):
//!
//! ```text
//! F = axial_engagement_mm × (Ks · fz·sin θ_peak + F_edge)    [N]
//! ```
//!
//! where the immersion angle ψ (→ θ_peak) is the sample's swept
//! `arc_engagement_radians` and `fz` is its commanded chipload per tooth.
//! Then [`ToolDefinition::tip_deflection_mm`] integrates the stepped
//! cantilever (shank + cutting region) using the per-cutter
//! `lookup_diameter_at` profile, and returns the predicted tip
//! displacement. The Suggest predictor and the pre-sim axial envelope
//! route through the same [`crate::feeds::predict::tip_deflection_from_engagement`]
//! so the three cannot drift apart.
//!
//! Verdict from the **peak `δ` across all cutting samples** of the
//! toolpath:
//!
//! | Peak tip deflection | Verdict | Confidence              |
//! |---------------------|---------|-------------------------|
//! | `< 50 µm`           | Within  | Validated               |
//! | `50 – 200 µm`       | Within  | Approximate             |
//! | `> 200 µm`          | Exceeds | LongToolStiffnessUnsafe |
//!
//! Refusal cases (mirrored with `power::evaluate`):
//! - No simulation trace → `Unmodeled(SimulationRequired)`
//! - Trace lacks `arc_engagement_radians` → `Unmodeled(ArcEngagementNotCaptured)`
//! - `Material::Custom` without explicit Kc handling → `Unmodeled(MaterialUnvalidated)`
//! - Zero stickout → `Unmodeled(NotImplemented)`
//!
//! ## Modeling assumptions
//!
//! - **Raw `Kc(material)`**, no grain-anisotropy factor. Static
//!   deflection responds to sustained mean force, not the transient
//!   grain spikes the power-safety 2.0× factor (Pałubicki 2021,
//!   `tool_load::power::GRAIN_ANISOTROPY_FACTOR`) is scoped to.
//! - Force is treated as a point load at the midpoint of axial
//!   engagement. Distributing along the engaged depth would refine the
//!   moment integral by under 10 % for fully-engaged flat endmills,
//!   well below the 50/200 µm threshold tolerance.
//! - **Bending only.** Torsion (tangential force × engagement radius)
//!   produces twist about the tool axis, which for a coaxial cutter does
//!   not translate the tip — only rotates it. The "tip-wander" metric
//!   this gate predicts therefore ignores torsion.

use crate::material::Material;
use crate::simulation_cut::SimulationCutSample;
use crate::tool::ToolDefinition;

use super::locality::SpanLookup;
use super::verdict::{
    Confidence, DeflectionBounds, DeflectionVerdict, EntrySpike, SampleEvidence, UnmodeledReason,
};

/// Below this peak tip deflection (mm), the cut is `Within(Validated)`.
pub const WITHIN_BOUND_MM: f64 = 0.050; // 50 µm

/// Above this peak tip deflection (mm), the cut is `Exceeds`.
pub const EXCEEDS_BOUND_MM: f64 = 0.200; // 200 µm

fn standard_bounds() -> DeflectionBounds {
    DeflectionBounds {
        validated_within_mm: WITHIN_BOUND_MM,
        exceeds_mm: EXCEEDS_BOUND_MM,
    }
}

/// Predict tip deflection for one simulation sample.
///
/// Returns `None` when the sample is not a usable cutting sample for the
/// force-derived deflection model: no arc engagement was captured, no radial
/// width/axial DOC is present, material Kc is unavailable, or the tool has no
/// stickout. The returned value is the same mm-scale tip displacement used by
/// [`evaluate`].
pub fn sample_tip_deflection_mm(
    tool: &ToolDefinition,
    material: &Material,
    sample: &SimulationCutSample,
    feed_per_tooth_mm: f64,
) -> Option<f64> {
    if !sample.is_cutting
        || sample.engagement.radial_woc_fraction < 0.02
        || sample.axial_engagement_mm <= 0.0
    {
        return None;
    }
    // The swept engagement arc IS the immersion angle ψ — hand it to the
    // canonical force model directly (no lossy arc→slab-width conversion).
    let immersion_rad = sample.arc_engagement_radians?;
    if immersion_rad <= 0.0 {
        return None;
    }
    // Canonical feed-aware force + cantilever model lives in
    // `feeds::force`/`feeds::predict` so the pre-sim cutter-axial-
    // constraints envelope, the Suggest predictor, and this post-sim gate
    // cannot drift apart. `feed_per_tooth_mm` is the *effective* chipload
    // (after kinematic prediction / F-039 modulation) the caller resolves
    // via `effective_feed_for_sample`, so a path the optimizer feeds down
    // for deflection reads safe here too. A sample with no chipload signal
    // yields `None` and is skipped (no deflection-relevant cutting load).
    crate::feeds::predict::tip_deflection_from_engagement(
        tool,
        material,
        sample.axial_engagement_mm,
        immersion_rad,
        feed_per_tooth_mm,
    )
}

#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id.0, op = ?ctx.operation_kind))]
pub fn evaluate(
    ctx: &super::ToolpathLoadContext<'_>,
    env: &super::GateEnv<'_>,
) -> DeflectionVerdict {
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
        tolerance,
        ..
    } = env;
    // Roadmap F.8 — same short-circuit as power/chipload: drill cycles
    // have no continuous engagement, so the cantilever-deflection
    // metric is meaningless. Return "doesn't apply" rather than the
    // misleading `ArcEngagementNotCaptured`.
    if operation_kind.is_drill_kinematics() {
        tracing::debug!(
            reason = "NotApplicableForOp",
            "deflection gate refuses: plunge-only op has no continuous engagement"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        };
    }
    let Some(trace) = sim_trace else {
        tracing::debug!(
            reason = "SimulationRequired",
            "deflection gate refuses: no simulation trace"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };

    if let Material::Custom { .. } = material {
        tracing::debug!(
            reason = "MaterialUnvalidated",
            material = "Custom",
            "deflection gate refuses: Custom material has no validated Kc"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }
    // Materials without a primary-source Kc refuse here. See power.rs
    // for the same pattern.
    if material.kc_n_per_mm2().is_none() {
        tracing::debug!(
            reason = "MaterialUnvalidated",
            material = %material.label(),
            "deflection gate refuses: material has no primary-source Kc"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }

    if tool.stickout <= 0.0 {
        tracing::debug!(
            reason = "NotImplemented",
            "deflection gate refuses: tool reports zero stickout"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("tool reports zero stickout".to_owned()),
        };
    }

    let mut peak_delta_mm = 0.0_f64;
    let mut peak_idx: Option<usize> = None;
    let mut any_arc_captured = false;
    let mut any_slot = false;
    // D7 — span-aware entry filter. Configured Entry transients bypass
    // the steady-state trip and surface separately as `entry_spike` on
    // `Within`. The peak Entry-ancestry tip-deflection sample (above
    // the EXCEEDS bound) is recorded for the advisory.
    let span_lookup = spans.map(SpanLookup::new);
    let mut entry_peak_delta_mm = 0.0_f64;
    let mut entry_peak_idx: Option<usize> = None;

    for (i, s) in trace.samples.iter().enumerate() {
        if s.toolpath_id != toolpath_id {
            continue;
        }
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

        // Effective feed per tooth (after kinematic prediction / F-039
        // modulation), mirroring the power + chipload gates so all three
        // evaluate the cut that will actually run. With no predicted-feed
        // map this is the sample's commanded chipload (unchanged).
        let eff_feed = super::effective_feed_for_sample(s, &trace.predicted_feeds);
        let flutes = s.flute_count.max(1) as f64;
        let eff_fz = if s.spindle_rpm > 0 {
            eff_feed / (s.spindle_rpm as f64 * flutes)
        } else {
            s.chipload_mm_per_tooth
        };

        let Some(delta_mm) = sample_tip_deflection_mm(tool, material, s, eff_fz) else {
            continue;
        };

        // Finding 3 split (2026-06-04): phantom-transit samples
        // (WaterlineCleanup / LinkBridge / LeadOut / DressupArtifact)
        // carry inflated dexel axial_engagement, so the computed tip
        // deflection is phantom — drop them entirely. Configured Entry
        // samples (real plunge / ramp / helix transients) still route
        // to the entry_spike advisory.
        if super::locality::is_phantom_transit(s, span_lookup.as_ref()) {
            continue;
        }
        if super::locality::is_configured_entry(s, span_lookup.as_ref()) {
            if delta_mm > entry_peak_delta_mm {
                entry_peak_delta_mm = delta_mm;
                entry_peak_idx = Some(i);
            }
            continue;
        }

        if arc >= std::f64::consts::PI - 1e-3 {
            any_slot = true;
        }
        if delta_mm > peak_delta_mm {
            peak_delta_mm = delta_mm;
            peak_idx = Some(i);
        }
    }

    if !any_arc_captured {
        tracing::debug!(
            reason = "ArcEngagementNotCaptured",
            "deflection gate refuses: trace lacks arc_engagement_radians on all samples"
        );
        return DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::ArcEngagementNotCaptured,
        };
    }

    let peak_um = peak_delta_mm * 1000.0;
    let base_detail = if any_slot {
        format!(
            "peak tip deflection {peak_um:.0} µm (slot engagement; climb/conventional split not modeled)"
        )
    } else {
        format!("peak tip deflection {peak_um:.0} µm (isotropic Kc, bending only)")
    };

    let evidence = match peak_idx {
        Some(idx) => SampleEvidence::at(idx).with_locality(
            trace
                .samples
                .get(idx)
                .and_then(|s| super::locality::classify_sample_locality(s, span_lookup.as_ref())),
        ),
        None => SampleEvidence::empty(),
    };
    let bounds = standard_bounds();

    let exceeds_trigger = EXCEEDS_BOUND_MM * (1.0 + tolerance.deflection_breach);
    if peak_delta_mm > exceeds_trigger {
        tracing::warn!(
            verdict = "Exceeds",
            peak_mm = peak_delta_mm,
            peak_um = peak_delta_mm * 1000.0,
            bound_mm = exceeds_trigger,
            "deflection gate Exceeds: peak tip deflection above the Exceeds safety threshold"
        );
        return DeflectionVerdict::Exceeds {
            peak_mm: peak_delta_mm,
            bounds,
            evidence,
            confidence: Confidence::Approximate(base_detail),
        };
    }
    let confidence = if peak_delta_mm > WITHIN_BOUND_MM {
        Confidence::Approximate(format!(
            "{base_detail} — surface finish degradation expected"
        ))
    } else if any_slot {
        Confidence::Approximate(base_detail)
    } else {
        Confidence::Validated
    };
    // D7 entry-spike advisory: an Entry-ancestry sample whose tip
    // deflection landed past the EXCEEDS bound, even though the
    // steady-state trip didn't fire.
    let entry_spike = match entry_peak_idx {
        Some(idx) if entry_peak_delta_mm > EXCEEDS_BOUND_MM => {
            let locality = trace
                .samples
                .get(idx)
                .and_then(|s| super::locality::classify_sample_locality(s, span_lookup.as_ref()))
                .unwrap_or_else(|| "entry".to_owned());
            Some(EntrySpike {
                observed: entry_peak_delta_mm,
                bound: EXCEEDS_BOUND_MM,
                locality,
                side: None,
            })
        }
        _ => None,
    };
    DeflectionVerdict::Within {
        peak_mm: peak_delta_mm,
        bounds,
        evidence,
        confidence,
        entry_spike,
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
    use crate::compute::tool_config::ToolMaterial;
    use crate::ids::ToolpathId;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };

    /// Adapts this module's legacy positional-arg test calls to the
    /// Phase 6 `(ctx, env)` gate signature.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_args(
        toolpath_id: usize,
        tool: &crate::tool::ToolDefinition,
        material: &crate::material::Material,
        sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
        spans: Option<&[crate::toolpath_spans::Span]>,
        operation_kind: crate::compute::catalog::OperationType,
        tolerance: &crate::tool_load::ToleranceBands,
    ) -> DeflectionVerdict {
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
                machine: None,
                tolerance,
            },
        )
    }
    use crate::tool::{FlatEndmill, TaperedBallEndmill};

    fn carbide_flat(diameter_mm: f64, stickout_mm: f64) -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(diameter_mm, stickout_mm.max(20.0))),
            diameter_mm,
            (stickout_mm - 20.0).max(10.0),
            25.0,
            stickout_mm,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn hss_flat(diameter_mm: f64, stickout_mm: f64) -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(diameter_mm, stickout_mm.max(20.0))),
            diameter_mm,
            (stickout_mm - 20.0).max(10.0),
            25.0,
            stickout_mm,
            2,
            ToolMaterial::Hss,
        )
    }

    fn wanaka_tapered_ball() -> ToolDefinition {
        // Wanaka tool 2: 2 mm tip / 7° taper / 6 mm shank, 35 mm stickout.
        ToolDefinition::new(
            Box::new(TaperedBallEndmill::new(2.0, 7.0, 6.0, 30.0)),
            6.0,
            10.0,
            25.0,
            35.0,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn cutting_sample(
        toolpath_id: usize,
        idx: usize,
        axial: f64,
        arc_rad: f64,
        feed_mmpm: f64,
        radial_eng: f64,
    ) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: ToolpathId(toolpath_id),
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
            engagement: crate::simulation_cut::Engagement::with_radial_woc(radial_eng),
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
            },
            samples,
            ..SimulationCutTrace::test_fixture()
        }
    }

    /// Feed-sensitivity sentry (gate level): a sample with higher feed
    /// per tooth produces a strictly higher tip deflection — but **less
    /// than proportional**, because the affine `F_edge` floor means feed
    /// cannot starve the bending force to zero. This is the behaviour
    /// that would have been flat under the old feed-blind model, and is
    /// the whole reason the per-move feed optimizer and this gate can now
    /// agree on the same cut.
    #[test]
    fn higher_feed_raises_deflection_but_edge_floor_holds() {
        let tool = carbide_flat(6.0, 45.0);
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let arc = std::f64::consts::FRAC_PI_2;
        let slow = cutting_sample(0, 0, 3.0, arc, 900.0, 0.5);
        let fast = cutting_sample(0, 0, 3.0, arc, 1800.0, 0.5); // 2× feed
        let d_slow = sample_tip_deflection_mm(&tool, &mat, &slow, slow.chipload_mm_per_tooth)
            .expect("slow δ");
        let d_fast = sample_tip_deflection_mm(&tool, &mat, &fast, fast.chipload_mm_per_tooth)
            .expect("fast δ");
        assert!(d_fast > d_slow, "2× feed must raise δ: {d_slow} → {d_fast}");
        assert!(
            d_fast < 2.0 * d_slow,
            "edge floor must keep δ sub-proportional: {d_fast} vs 2×{d_slow}"
        );
    }

    /// Agreement sentry (gate wiring): the gate feeds the sample's swept
    /// `arc_engagement_radians` as the immersion angle and its commanded
    /// `chipload_mm_per_tooth` as feed per tooth — i.e. it routes through
    /// the same canonical model the predictor and envelope use. Pins the
    /// wiring so the gate cannot silently start passing a different
    /// quantity (the latent arc-slab vs raw-WOC divergence this fixed).
    #[test]
    fn gate_routes_through_canonical_force_model() {
        let tool = carbide_flat(6.0, 45.0);
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let arc = 1.2_f64;
        let s = cutting_sample(0, 0, 2.5, arc, 1200.0, 0.5);
        let via_gate =
            sample_tip_deflection_mm(&tool, &mat, &s, s.chipload_mm_per_tooth).expect("gate δ");
        let via_canonical = crate::feeds::predict::tip_deflection_from_engagement(
            &tool,
            &mat,
            s.axial_engagement_mm,
            s.arc_engagement_radians.expect("arc"),
            s.chipload_mm_per_tooth,
        )
        .expect("canonical δ");
        assert!((via_gate - via_canonical).abs() < 1e-12);
    }

    /// Step-4 optimizer↔gate consistency: a long/thin tool full-slotting
    /// at the commanded feed Exceeds, but stamping the deflection-safe feed
    /// the F-039 optimizer would produce (computed here from the SAME
    /// affine-inverse formula the optimizer uses) into the trace's
    /// `predicted_feeds` flips the gate to `Within`. This is the dead-end
    /// the unified model closes: before, the gate ignored the optimizer's
    /// feed-down (feed-blind) and kept reading Exceeds.
    #[test]
    fn gate_honors_optimizer_feed_down_into_within() {
        let tool = carbide_flat(3.0, 45.0); // L/D 15, uniform 3 mm beam
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let ap = 1.5_f64;
        // Full-slot sample (arc = π) at a high commanded feed.
        let commanded_feed = 5000.0_f64;
        let sample = cutting_sample(0, 0, ap, std::f64::consts::PI, commanded_feed, 1.0);

        // Commanded (no predicted-feed map): the gate Exceeds.
        let trace_cmd = trace_with(vec![sample.clone()]);
        let v_cmd = evaluate_args(
            0,
            &tool,
            &mat,
            Some(&trace_cmd),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(
            matches!(v_cmd, DeflectionVerdict::Exceeds { .. }),
            "long/thin tool at full commanded feed must Exceed; got {v_cmd:?}"
        );

        // The optimizer's deflection-safe feed (affine inverse, full slot
        // ⇒ sinθ_peak = 1), with a small margin so we land clearly Within.
        let (ks, f_edge) = crate::feeds::force::affine_coefficients(&mat).expect("coeffs");
        let e = tool.tool_material.youngs_modulus_n_per_mm2();
        let compliance = tool.tip_deflection_mm(1.0, ap, e);
        let budget_force = EXCEEDS_BOUND_MM / compliance;
        let safe_fz = (budget_force / ap - f_edge) / ks;
        let safe_feed = safe_fz * sample.spindle_rpm as f64 * sample.flute_count as f64 * 0.98;
        assert!(
            safe_feed > 0.0 && safe_feed < commanded_feed,
            "safe feed should be a real feed-down: {safe_feed}"
        );

        // Stamp it as the predicted (modulated) feed for the cutting move.
        let mut trace_safe = trace_with(vec![sample.clone()]);
        trace_safe
            .predicted_feeds
            .insert((ToolpathId(0), sample.move_index), safe_feed);
        let v_safe = evaluate_args(
            0,
            &tool,
            &mat,
            Some(&trace_safe),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v_safe {
            DeflectionVerdict::Within { peak_mm, .. } => {
                assert!(
                    peak_mm <= EXCEEDS_BOUND_MM,
                    "optimizer feed-down must read Within: {peak_mm} mm"
                );
            }
            other => panic!("expected Within after optimizer feed-down, got {other:?}"),
        }
    }

    #[test]
    fn no_trace_returns_simulation_required() {
        let v = evaluate_args(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            None,
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    fn no_arc_data_returns_arc_engagement_not_captured() {
        let mut s = cutting_sample(0, 0, 3.0, std::f64::consts::FRAC_PI_2, 1500.0, 0.5);
        s.arc_engagement_radians = None;
        let trace = trace_with(vec![s]);
        let v = evaluate_args(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::ArcEngagementNotCaptured
            }
        ));
    }

    #[test]
    fn custom_material_returns_material_unvalidated() {
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            3.0,
            std::f64::consts::FRAC_PI_2,
            1500.0,
            0.5,
        )]);
        let v = evaluate_args(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::test_fixture_custom("Mystery"),
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        assert!(matches!(
            v,
            DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::MaterialUnvalidated
            }
        ));
    }

    #[test]
    fn wanaka_endmill_back_rough_deflection_is_within() {
        // A stubby roughing endmill (6 mm carbide flat, 45 mm stickout)
        // full-slotting hardwood at 2.5 mm DOC. Under the feed-aware
        // literature-absolute force model the instantaneous bending force
        // is modest (~18 N: ap 2.5 · (Ks·fz + F_edge) at full immersion),
        // so peak tip deflection is ~17 µm — comfortably Within. Deflection
        // is NOT the binding constraint for a stubby 6 mm flat at L/D 7.5;
        // the real limiter for full-slotting hardwood is chipload / power /
        // chip evacuation. (The old `Kc·ap·ae` aggregate read ~494 µm here
        // — ~4× the honest instantaneous force, which made the deflection
        // gate cry tool-limited on a cut that isn't.) The slot annotation
        // still rides on the verdict so downstream surfaces can name the
        // engagement.
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            2.5,
            std::f64::consts::PI,
            1500.0,
            1.0,
        )]);
        let v = evaluate_args(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            DeflectionVerdict::Within {
                peak_mm, evidence, ..
            } => {
                let um = peak_mm * 1000.0;
                assert!(
                    (8.0..=40.0).contains(&um),
                    "stubby roughing endmill full-slot is deflection-safe (~17 µm) under the feed-aware force model; got {um:.1} µm"
                );
                assert_eq!(
                    evidence.locality.as_deref(),
                    Some("slot section"),
                    "slot annotation expected, got: {evidence:?}"
                );
            }
            other => panic!("expected Within (deflection not the limiter), got {other:?}"),
        }
    }

    #[test]
    fn wanaka_tapered_ball_finishing_is_within() {
        // Wanaka TP 5/6/11: 2 mm tip / 6 mm shank tapered ball at 35 mm
        // stickout, finishing pass — small DOC, small engagement, low
        // force. Predicted δ should easily clear the Within band.
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            0.5,
            std::f64::consts::FRAC_PI_4,
            800.0,
            0.15,
        )]);
        let v = evaluate_args(
            0,
            &wanaka_tapered_ball(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            DeflectionVerdict::Within { peak_mm, .. } => {
                let um = peak_mm * 1000.0;
                assert!(
                    um < 50.0,
                    "tapered-ball finishing should land Within(Validated); got {um:.1} µm"
                );
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn small_engraver_low_feed_in_hardwood_passes() {
        // 1 mm carbide flat engraver, light cut in hardwood — the gap
        // doc's "should still pass" workflow. Tiny chip cross-section
        // keeps the instantaneous force low; under the feed-aware
        // literature-absolute force model a light 1 mm engraver cut at
        // 15 mm stickout reads ~50 µm — Within. (Note: deflection DOES
        // gate genuinely long/thin tools and aggressive small-tool cuts —
        // a 3 mm tool crosses 200 µm Exceeds by L/D ~14, or sooner under a
        // deep/over-fed cut — so the gate is appropriately scoped, not dead.)
        let tool = carbide_flat(1.0, 15.0);
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            0.2,
            std::f64::consts::FRAC_PI_4,
            120.0,
            0.2,
        )]);
        let v = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        let peak_um = match v {
            DeflectionVerdict::Within { peak_mm, .. }
            | DeflectionVerdict::Exceeds { peak_mm, .. } => peak_mm * 1000.0,
            other => panic!("expected modeled verdict, got {other:?}"),
        };
        assert!(
            peak_um < EXCEEDS_BOUND_MM * 1000.0,
            "1 mm engraver light cut should not Exceed; got {peak_um:.1} µm"
        );
    }

    #[test]
    fn long_hss_in_steel_kc_exceeds_at_model_level() {
        // The codebase is wood-only at the Material enum, so this test
        // exercises the underlying `tip_deflection_mm` model directly
        // with a steel-equivalent Kc to verify the formula isn't
        // wood-only. 6 mm HSS at 60 mm stickout, mild-steel Kc=2000,
        // half-engagement at axial 1 mm: F = 2000·1·3 = 6000 N (one
        // sample worst case). Even at avg engagement (rwidth 1 mm)
        // the resulting δ should overshoot the 200 µm bound.
        let tool = hss_flat(6.0, 60.0);
        let kc = 2000.0_f64;
        let axial = 1.0_f64;
        let radial_width = 1.0_f64;
        let force_n = kc * axial * radial_width;
        let e = tool.tool_material.youngs_modulus_n_per_mm2();
        let delta_mm = tool.tip_deflection_mm(force_n, axial, e);
        let delta_um = delta_mm * 1000.0;
        assert!(
            delta_um > 200.0,
            "HSS at long stickout in steel-equivalent Kc must exceed; got {delta_um:.1} µm"
        );
    }

    #[test]
    fn slot_engagement_annotates_approximate_within() {
        // Light slot: high arc but small chip — should produce a Within
        // verdict whose confidence flags the slot annotation.
        let tool = carbide_flat(6.0, 25.0);
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            1.0,
            std::f64::consts::PI,
            1000.0,
            1.0,
        )]);
        let v = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            DeflectionVerdict::Within {
                confidence: Confidence::Approximate(detail),
                ..
            } => assert!(
                detail.contains("slot"),
                "slot annotation expected in detail string, got: {detail}"
            ),
            other => panic!("expected Within(Approximate(slot...)), got {other:?}"),
        }
    }

    #[test]
    fn deflection_breach_tolerance_widens_exceeds_trigger() {
        // Compare strict vs widened bands on the same trace. Either
        // strict already lands Within (geometry doesn't push past 200µm
        // in wood — this is the common case) and widened stays Within,
        // OR strict lands Exceeds and widened *must* flip to Within.
        // The wrong direction (strict Within → widened Exceeds) is a
        // regression in the trigger math.
        let tool = hss_flat(6.0, 60.0);
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            6.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
            0.4,
        )]);
        let strict = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands::default(),
        );
        let widened = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Pocket,
            &crate::tool_load::ToleranceBands {
                deflection_breach: 100.0, // absurdly wide → no peak can trip Exceeds
                ..crate::tool_load::ToleranceBands::default()
            },
        );
        match (&strict, &widened) {
            (DeflectionVerdict::Exceeds { .. }, DeflectionVerdict::Within { .. }) => {}
            (DeflectionVerdict::Within { .. }, DeflectionVerdict::Within { .. }) => {}
            _ => panic!(
                "deflection_breach must not flip verdict in the wrong direction; \
                 strict={strict:?} widened={widened:?}"
            ),
        }
    }

    /// Roadmap F.8 — same short-circuit as the chipload and power
    /// gates: drill cycles have no continuous engagement, so the
    /// cantilever-deflection metric has no meaning. Refuse with
    /// `NotApplicableForOp` even on a fully-engaged sample stream.
    /// Finding 3 split sentry (2026-06-04): phantom-transit samples
    /// must NOT surface as an `entry_spike` — their dexel-inflated
    /// axial_engagement produces a phantom deflection that misleads
    /// the operator. The wanaka Back Rough symptom was a 608 µm
    /// "waterline cleanup" entry_spike alongside a healthy 165 µm
    /// steady-state peak.
    ///
    /// Mirror the structure here: one steady-state sample with
    /// realistic engagement, plus one WaterlineCleanup-ancestry
    /// sample with deeply-inflated engagement. Pre-fix the inflated
    /// sample routed to entry_spike with phantom-large delta. Post-fix
    /// it's dropped, entry_spike is `None`, steady-state peak is what
    /// the verdict reports.
    #[test]
    fn phantom_waterline_cleanup_does_not_surface_as_entry_spike() {
        use crate::toolpath_spans::Span;
        use std::borrow::Cow;

        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7)
        // lifts the deflection force ~2.7×; the original 45 mm-stickout
        // steady sample now reads ~399 µm and Exceeds. This test is about
        // phantom-sample FILTERING (entry_spike must stay None), not the
        // deflection magnitude — so shorten stickout 45 → 30 mm (δ ∝
        // stickout³ → ~0.30×) to keep the steady sample Within and keep
        // the phantom-filter assertion the thing under test.
        let tool = carbide_flat(6.0, 30.0);
        // Steady-state sample: in DepthPass, healthy 2 mm axial DOC.
        let mut steady = cutting_sample(0, 0, 2.0, std::f64::consts::PI, 1500.0, 1.0);
        steady.span_path = vec![
            crate::toolpath_spans::SpanId(0), // Operation
            crate::toolpath_spans::SpanId(1), // DepthPass
        ];
        // Phantom sample: WaterlineCleanup ancestor, 20 mm "axial DOC"
        // (dexel bridge artifact). 10× the steady sample → would
        // produce a deflection ~10× larger if not filtered.
        let mut phantom = cutting_sample(0, 1, 20.0, std::f64::consts::PI, 1500.0, 1.0);
        phantom.span_path = vec![
            crate::toolpath_spans::SpanId(0), // Operation
            crate::toolpath_spans::SpanId(2), // WaterlineCleanup
        ];
        phantom.in_transit_span = true;

        let spans = vec![
            Span {
                start_move: 0,
                end_move: 2,
                kind: crate::toolpath_spans::SpanKind::Operation,
                label: Cow::Borrowed("op"),
                payload: None,
            },
            Span {
                start_move: 0,
                end_move: 1,
                kind: crate::toolpath_spans::SpanKind::DepthPass,
                label: Cow::Borrowed("pass"),
                payload: None,
            },
            Span {
                start_move: 1,
                end_move: 2,
                kind: crate::toolpath_spans::SpanKind::WaterlineCleanup,
                label: Cow::Borrowed("cleanup"),
                payload: None,
            },
        ];

        let trace = trace_with(vec![steady, phantom]);
        let v = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            Some(&spans),
            OperationType::Adaptive3d,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            DeflectionVerdict::Within {
                entry_spike,
                peak_mm,
                ..
            } => {
                assert!(
                    entry_spike.is_none(),
                    "phantom WaterlineCleanup must NOT surface as entry_spike, got {entry_spike:?}"
                );
                // Sanity: peak comes from the 2 mm steady sample, not
                // the 20 mm phantom. Tip deflection scales linearly
                // with axial engagement → if the phantom leaked into
                // the steady-state track, peak would be ~10× the
                // steady-only value for these inputs).
                // The realistic 2 mm-axial slot peak for a 6 mm carbide
                // flat at 30 mm stickout, 1500 mm/min in HardMaple lands
                // around ~120 µm under the milling-Kc calibration.
                assert!(
                    peak_mm < 0.5,
                    "steady-state peak should reflect the 2 mm steady sample (~150 µm), \
                     got {peak_mm} mm — phantom 20 mm sample leaked into peak",
                );
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn drill_op_routes_to_not_applicable() {
        let tool = carbide_flat(6.0, 45.0);
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            2.5,
            std::f64::consts::PI,
            1500.0,
            1.0,
        )]);
        let v = evaluate_args(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
            None,
            OperationType::Drill,
            &crate::tool_load::ToleranceBands::default(),
        );
        match v {
            DeflectionVerdict::Unmodeled {
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
}
