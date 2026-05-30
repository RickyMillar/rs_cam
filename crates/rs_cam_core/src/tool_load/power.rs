//! Power guardrail — per-sample instantaneous spindle power vs available
//! power × machine safety factor.
//!
//! `P_kW = Kc_eff × axial_doc × radial_width × feed / 60_000_000` where:
//! - `Kc_eff = GRAIN_ANISOTROPY_FACTOR × material.kc_n_per_mm2()`.
//!   `GRAIN_ANISOTROPY_FACTOR = 2.0` is the measured directional spread
//!   of specific cutting force for wood-class materials per Pałubicki
//!   2021 (DOI 10.3390/ma14092208, particleboard peripheral up-milling
//!   across grain orientations). Pre-Phase-2 the factor was 2.5 — a
//!   magic number chosen to absorb under-modeled `Kc`. Phase 2B paired
//!   the rename + value change with literature-anchored sheet-good Kc
//!   so the product `Kc × factor` reflects physics rather than the old
//!   absorption split.
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

use crate::compute::catalog::OperationType;
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath_spans::Span;

use super::locality::SpanLookup;
use super::verdict::{Confidence, EntrySpike, PowerVerdict, SampleEvidence, UnmodeledReason};

/// Wood grain anisotropy factor on Kc — Pałubicki 2021 (DOI
/// 10.3390/ma14092208) measured the directional spread of specific
/// cutting force for particleboard peripheral up-milling across grain
/// orientations. The rename from `ANISOTROPY_MULTIPLIER` (pre-Phase-2,
/// value 2.5) reflects that this is a documented physical factor, not
/// a knob to tune around under-modeled Kc.
const GRAIN_ANISOTROPY_FACTOR: f64 = 2.0;

#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    toolpath_id: usize,
    tool: &ToolDefinition,
    material: &Material,
    machine: &MachineProfile,
    sim_trace: Option<&SimulationCutTrace>,
    spans: Option<&[Span]>,
    operation_kind: OperationType,
    tolerance: &super::ToleranceBands,
) -> PowerVerdict {
    // Roadmap F.8 — short-circuit before any gate-input checks when
    // the op is geometrically plunge-only. Drilling has no continuous
    // engagement to drive a power-vs-RPM curve; the right answer is
    // "doesn't apply" not "arc engagement not captured".
    if is_plunge_only_op(operation_kind) {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(
                "drill cycle — no continuous engagement".to_owned(),
            ),
        };
    }
    let Some(trace) = sim_trace else {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };

    // Material::Custom without an explicitly-validated Kc: refuse. The
    // `kc_n_per_mm2` accessor on Custom returns whatever the user typed;
    // unless a project-level "validated" flag exists, the safest default
    // is to refuse rather than predict force from an unvetted constant.
    if let Material::Custom { .. } = material {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }

    // Materials without a primary-source Kc (e.g. most plastics, aluminum
    // pre Phase 3 beat F) return None and refuse here. The type-level
    // Option encodes "no validated cutting-force model" — no fabricated
    // constant ever drives a force prediction.
    let Some(kc) = material.kc_n_per_mm2() else {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    };
    let kc_eff = GRAIN_ANISOTROPY_FACTOR * kc;

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
    // D7 — span-aware entry filter. Configured Entry transients
    // (Adaptive3D plunge / helix / ramp, dressup lead-ins) bypass the
    // trip decision and surface separately as `entry_spike` on the
    // `Within` arm. The peak Entry-ancestry power sample (whether or
    // not it exceeds available_kw) is recorded for the advisory.
    let span_lookup = spans.map(SpanLookup::new);
    let mut entry_peak_power: f64 = 0.0;
    let mut entry_peak_idx: Option<usize> = None;
    let mut entry_peak_available: f64 = 0.0;

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

        // Power formula. Arc-equivalent radial slab width:
        //   radial_width = (arc / π) × engagement_radius × 2
        // For a half-engagement (arc = π/2), this gives `engagement_radius`.
        // For a slot (arc = π), it gives 2× engagement_radius — the full
        // tool diameter — which is the correct engaged width for slotting.
        let engagement_radius = tool.engagement_radius(s.axial_doc_mm).max(0.0);
        let radial_width = (arc / std::f64::consts::PI) * engagement_radius * 2.0;
        if radial_width <= 0.0 {
            continue;
        }

        // P_kW = Kc × DOC × WOC × feed / (60 * 1e6)
        //
        // F-035: read the *effective* feed for this sample —
        // predicted (achieved) when the trace carries a populated
        // `predicted_feeds` map AND this `(toolpath_id, move_index)`
        // lookup hits, commanded otherwise. Power is linear in
        // feed, so the substitution scales the predicted load
        // proportionally; corner-decel reduction in predicted feed
        // shows up directly as reduced predicted power.
        let feed_for_power = super::effective_feed_for_sample(s, &trace.predicted_feeds);
        // Engaged chip cross-section is shape-dependent: rectangular slab
        // for endmills, triangular groove for V-bits. The cutter owns that
        // geometry; the gate owns the material + machine physics.
        let cross_section_mm2 = tool.mrr_cross_section_mm2(s.axial_doc_mm, radial_width);
        let p_kw = kc_eff * cross_section_mm2 * feed_for_power / 60_000_000.0;
        let avail = machine.power_at_rpm(s.spindle_rpm as f64) * machine.safety_factor;

        // Route Entry-ancestry samples to the spike track; they don't
        // drive the steady-state trip but `any_slot`/`last_available_kw`
        // are bookkeeping that still applies.
        if !super::locality::is_steady_state_for_gate(s, span_lookup.as_ref()) {
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

        last_available_kw = avail;

        if p_kw > peak_power {
            peak_power = p_kw;
            peak_idx = Some(i);
            peak_available_at_peak = avail;
        }
    }

    if !any_arc_captured {
        // No samples carried arc data — likely capture_arc_engagement was
        // off when the trace was recorded.
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

    let evidence = match peak_idx {
        Some(idx) => SampleEvidence::at(idx).with_locality(
            trace
                .samples
                .get(idx)
                .and_then(|s| super::locality::classify_sample_locality(s, span_lookup.as_ref())),
        ),
        None => SampleEvidence::empty(),
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

    // Layer 1 tolerance band: widen the peak-vs-available trigger by
    // `power_breach`. Default is 0 (preserves the strict machine-ceiling
    // behaviour) — see `ToleranceBands::power_breach` doc.
    let power_trigger = peak_available_at_peak * (1.0 + tolerance.power_breach);
    if peak_available_at_peak > 0.0 && peak_power > power_trigger {
        return PowerVerdict::Exceeds {
            peak_kw: peak_power,
            available_kw,
            evidence,
            confidence,
        };
    }
    // D7 entry-spike advisory: surface a configured-entry sample whose
    // power exceeded the (possibly tolerance-widened) machine ceiling,
    // even though the steady-state trip didn't fire.
    let entry_spike = match entry_peak_idx {
        Some(idx) if entry_peak_available > 0.0 && entry_peak_power > entry_peak_available => {
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
    }
}

/// Roadmap F.8 — operation kinds whose geometry is plunge-only. The
/// three load gates (chipload, power, deflection) all need the same
/// short-circuit, so the predicate lives in one place.
pub(super) fn is_plunge_only_op(op_kind: OperationType) -> bool {
    matches!(
        op_kind,
        OperationType::Drill | OperationType::AlignmentPinDrill
    )
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
    use crate::machine::MachineProfile;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
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
            toolpath_id: 0,
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
            plunge_descent_mm: 0.0,
            arc_engagement_radians: Some(arc_rad),
            chipload_mm_per_tooth: feed_mmpm / (18_000.0 * 2.0),
            effective_chip_thickness_mm: Some(feed_mmpm / (18_000.0 * 2.0)),
            engagement: crate::simulation_cut::Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            semantic_item_id: None,
            span_path: Vec::new(),
            in_transit_span: false,
        }
    }

    fn trace_with(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            schema_version: 1,
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
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            samples,
            provenance: None,
            drill_samples: Vec::new(),
            drill_summaries: Vec::new(),
            predicted_feeds: crate::machine_kinematics::PredictedFeedMap::new(),
            modulated_feeds: std::collections::BTreeMap::new(),
            modulation_summaries: std::collections::BTreeMap::new(),        }
    }

    #[test]
    fn no_trace_returns_simulation_required() {
        let v = evaluate(
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
        let v = evaluate(
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
        let v = evaluate(
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
        let v = evaluate(
            0,
            &tool(),
            &Material::Custom {
                name: "Mystery".into(),
                hardness_index: 1.0,
                kc: 10.0,
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
    fn light_cut_is_within_with_available_kw() {
        // 6.35mm flat in hard maple, half-engagement (arc=π/2), 1mm DOC,
        // 1000 mm/min feed:
        //   engagement_radius = 3.175
        //   radial_width = (π/2 / π) × 3.175 × 2 = 3.175
        //   Kc_eff = 2.0 × 15 = 30.0 N/mm² (Phase 2B grain anisotropy)
        //   P_kW = 30.0 × 1 × 3.175 × 1000 / 60e6 ≈ 0.00159 kW
        // Shapeoko Makita ≈ 0.71 kW × 0.8 safety = 0.568. Within, and the
        // verdict must surface the available headroom for UI rendering.
        let trace = trace_with(vec![cutting_sample(
            0,
            1.0,
            std::f64::consts::FRAC_PI_2,
            1000.0,
        )]);
        let v = evaluate(
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
                assert!(peak_kw > 0.0 && peak_kw < 0.01, "peak power {peak_kw} kW");
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
        // vs available × safety = 0.568 kW. Exceeds, and the verdict
        // must carry both peak_kw and available_kw.
        let trace = trace_with(vec![cutting_sample(0, 20.0, std::f64::consts::PI, 6000.0)]);
        let v = evaluate(
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

    #[test]
    fn vbit_triangular_cross_section_halves_power_vs_flat() {
        // Same DOC / arc / feed on a 6.35 mm flat vs a 6.35 mm 90° V-bit.
        // The flat removes a rectangular slab; the V-bit removes a
        // triangular groove of half the area, so its predicted power must
        // be ~half. (engagement_radius differs by shape, so we compare the
        // ratio of peak_kw, which isolates the cross-section model: both
        // tools see the same radial_width at this DOC because a 90° V-bit's
        // width_at_height(1.0) = 1.0 ≠ flat's 3.175 — so we instead assert
        // the V-bit power equals 0.5 · kc_eff · doc · woc_vbit · feed.)
        let doc = 1.0;
        let arc = std::f64::consts::FRAC_PI_2;
        let feed = 1000.0;
        let trace = trace_with(vec![cutting_sample(0, doc, arc, feed)]);
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let v = evaluate(
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
        // 0.5 mm². Kc_eff = 2.0 · 15 = 30.0 (Phase 2B grain anisotropy).
        // P = 30.0·0.5·1000/60e6.
        let expected = 30.0 * 0.5 * 1.0 * feed / 60_000_000.0;
        assert!(
            (peak - expected).abs() / expected < 0.02,
            "V-bit triangular power {peak} should match {expected}"
        );
    }

    #[test]
    fn slot_annotates_approximate() {
        let trace = trace_with(vec![cutting_sample(0, 1.0, std::f64::consts::PI, 1000.0)]);
        let v = evaluate(
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

    /// Same fixture as `heavy_cut_exceeds_machine_with_available_kw` but
    /// with `power_breach` set generously enough to admit the candidate.
    /// Confirms the tolerance band actually widens the gate trigger;
    /// `power_breach = 0` (the default) preserves today's strict ceiling.
    #[test]
    fn heavy_cut_within_with_power_breach_tolerance() {
        let trace = trace_with(vec![cutting_sample(0, 20.0, std::f64::consts::PI, 6000.0)]);
        let bands = crate::tool_load::ToleranceBands {
            power_breach: 1.0, // widen by 100 % so the heavy-cut probe lands Within
            ..crate::tool_load::ToleranceBands::default()
        };
        let v = evaluate(
            0,
            &tool(),
            &Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
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
    }

    /// Roadmap F.8 — drill / pin-drill operations short-circuit to
    /// `NotApplicableForOp` before any trace inspection. Even with a
    /// fully-populated trace at heavy load (the same fixture as
    /// `heavy_cut_exceeds_machine_with_available_kw`), the power gate
    /// must refuse with the "doesn't apply" reason, not Exceeds.
    #[test]
    fn drill_op_routes_to_not_applicable_regardless_of_trace() {
        let trace = trace_with(vec![cutting_sample(0, 20.0, std::f64::consts::PI, 6000.0)]);
        let v = evaluate(
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
        let v = evaluate(
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
