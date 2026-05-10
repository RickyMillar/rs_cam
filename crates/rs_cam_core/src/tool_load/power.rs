//! Power guardrail — per-sample instantaneous spindle power vs available
//! power × machine safety factor.
//!
//! `P_kW = Kc_eff × axial_doc × radial_width × feed / 60_000_000` where:
//! - `Kc_eff = 2.5 × material.kc_n_per_mm2()` is a worst-case anisotropy
//!   multiplier. Real wood Kc varies 2-3× with grain direction; we don't
//!   model grain, so we use the upper bound of the published range.
//!   Equivalently: any predicted power below 1/2.5 of the machine limit
//!   is *guaranteed* safe regardless of grain orientation.
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

use crate::machine::MachineProfile;
use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};

use super::verdict::{Confidence, PowerVerdict, SampleEvidence, UnmodeledReason};

/// Worst-case anisotropy multiplier on Kc. See module-level doc.
const ANISOTROPY_MULTIPLIER: f64 = 2.5;

pub fn evaluate(
    toolpath_id: usize,
    tool: &ToolDefinition,
    material: &Material,
    machine: &MachineProfile,
    sim_trace: Option<&SimulationCutTrace>,
    tolerance: &super::ToleranceBands,
) -> PowerVerdict {
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

    let kc = material.kc_n_per_mm2();
    if !(kc.is_finite()) || kc <= 0.0 {
        return PowerVerdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }
    let kc_eff = ANISOTROPY_MULTIPLIER * kc;

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

    for (i, s) in trace.samples.iter().enumerate() {
        if s.toolpath_id != toolpath_id {
            continue;
        }
        if !s.is_cutting {
            continue;
        }
        if s.radial_engagement < 0.02 {
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
        let p_kw = kc_eff * s.axial_doc_mm * radial_width * s.feed_rate_mm_min / 60_000_000.0;

        if arc >= std::f64::consts::PI - 1e-3 {
            any_slot = true;
        }

        let avail = machine.power_at_rpm(s.spindle_rpm as f64) * machine.safety_factor;
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
            "isotropic Kc with 2.5× anisotropy multiplier; no helix/grain decomposition".to_owned(),
        )
    };

    let evidence = match peak_idx {
        Some(idx) => SampleEvidence::at(idx).with_locality(
            trace
                .samples
                .get(idx)
                .and_then(super::locality::classify_sample_locality),
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
    PowerVerdict::Within {
        peak_kw: peak_power,
        available_kw,
        evidence,
        confidence,
        // C1+C2 reverted 2026-05-10 (D3 Path A); D7 will repopulate
        // from span-aware entry detection.
        entry_spike: None,
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
    use crate::machine::MachineProfile;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
    use crate::tool::FlatEndmill;

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
            radial_engagement: 0.5,
            arc_engagement_radians: Some(arc_rad),
            chipload_mm_per_tooth: feed_mmpm / (18_000.0 * 2.0),
            effective_chip_thickness_mm: Some(feed_mmpm / (18_000.0 * 2.0)),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            semantic_item_id: None,
            span_path: Vec::new(),
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
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
            },
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            samples,
            provenance: None,
        }
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
        //   Kc_eff = 2.5 × 15 = 37.5 N/mm²
        //   P_kW = 37.5 × 1 × 3.175 × 1000 / 60e6 ≈ 0.00198 kW
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
            &bands,
        );
        assert!(
            matches!(v, PowerVerdict::Within { .. }),
            "expected Within with power_breach=1.0, got {v:?}"
        );
    }
}
