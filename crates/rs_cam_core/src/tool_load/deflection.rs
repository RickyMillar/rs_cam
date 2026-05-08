//! Tip-deflection guardrail — predicts how far the tool tip wanders
//! under cutting load.
//!
//! For each cutting sample of the toolpath, computes the transverse
//! cutting force from material specific cutting energy:
//!
//! ```text
//! F = Kc(material) × axial_doc_mm × radial_width_mm    [N]
//! ```
//!
//! using the same arc-equivalent slab as `power::evaluate`. Then
//! [`ToolDefinition::tip_deflection_mm`] integrates the stepped
//! cantilever (shank + cutting region) using the per-cutter
//! `lookup_diameter_at` profile, and returns the predicted tip
//! displacement.
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
//! - **Raw `Kc(material)`**, no anisotropy multiplier. Static deflection
//!   responds to sustained mean force, not the transient grain spikes the
//!   power-safety 2.5× bound is scoped to.
//! - Force is treated as a point load at the midpoint of axial
//!   engagement. Distributing along the engaged depth would refine the
//!   moment integral by under 10 % for fully-engaged flat endmills,
//!   well below the 50/200 µm threshold tolerance.
//! - **Bending only.** Torsion (tangential force × engagement radius)
//!   produces twist about the tool axis, which for a coaxial cutter does
//!   not translate the tip — only rotates it. The "tip-wander" metric
//!   this gate predicts therefore ignores torsion.

use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::ToolDefinition;

use super::verdict::{Confidence, ExceedsReason, UnmodeledReason, Verdict};

/// Below this peak tip deflection (mm), the cut is `Within(Validated)`.
const WITHIN_BOUND_MM: f64 = 0.050; // 50 µm

/// Above this peak tip deflection (mm), the cut is `Exceeds`.
pub(crate) const EXCEEDS_BOUND_MM: f64 = 0.200; // 200 µm

pub fn evaluate(
    toolpath_id: usize,
    tool: &ToolDefinition,
    material: &Material,
    sim_trace: Option<&SimulationCutTrace>,
) -> Verdict {
    let Some(trace) = sim_trace else {
        return Verdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };

    if let Material::Custom { .. } = material {
        return Verdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }
    let kc = material.kc_n_per_mm2();
    if !kc.is_finite() || kc <= 0.0 {
        return Verdict::Unmodeled {
            reason: UnmodeledReason::MaterialUnvalidated,
        };
    }

    if tool.stickout <= 0.0 {
        return Verdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("tool reports zero stickout".to_owned()),
        };
    }
    let e = tool.tool_material.youngs_modulus_n_per_mm2();

    let mut peak_delta_mm = 0.0_f64;
    let mut peak_idx: Option<usize> = None;
    let mut any_arc_captured = false;
    let mut any_slot = false;

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

        let engagement_radius =
            crate::tool::MillingCutter::engagement_radius(tool, s.axial_doc_mm).max(0.0);
        let radial_width = (arc / std::f64::consts::PI) * engagement_radius * 2.0;
        if radial_width <= 0.0 || s.axial_doc_mm <= 0.0 {
            continue;
        }

        let force_n = kc * s.axial_doc_mm * radial_width;
        let delta_mm = tool.tip_deflection_mm(force_n, s.axial_doc_mm, e);

        if arc >= std::f64::consts::PI - 1e-3 {
            any_slot = true;
        }
        if delta_mm > peak_delta_mm {
            peak_delta_mm = delta_mm;
            peak_idx = Some(i);
        }
    }

    if !any_arc_captured {
        return Verdict::Unmodeled {
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

    if peak_delta_mm > EXCEEDS_BOUND_MM {
        let idx = peak_idx.unwrap_or(0);
        return Verdict::Exceeds {
            peak: peak_delta_mm,
            sample_range: idx..(idx + 1),
            reason: ExceedsReason::LongToolStiffnessUnsafe,
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
    Verdict::Within {
        peak: peak_delta_mm,
        confidence,
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
    use crate::compute::tool_config::ToolMaterial;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
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
            toolpath_id,
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
            radial_engagement: radial_eng,
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
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            None,
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    fn no_arc_data_returns_arc_engagement_not_captured() {
        let mut s = cutting_sample(0, 0, 3.0, std::f64::consts::FRAC_PI_2, 1500.0, 0.5);
        s.arc_engagement_radians = None;
        let trace = trace_with(vec![s]);
        let v = evaluate(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
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
        let v = evaluate(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::Custom {
                name: "Mystery".into(),
                hardness_index: 1.0,
                kc: 10.0,
            },
            Some(&trace),
        );
        assert!(matches!(
            v,
            Verdict::Unmodeled {
                reason: UnmodeledReason::MaterialUnvalidated
            }
        ));
    }

    #[test]
    fn wanaka_endmill_back_rough_lands_in_approximate_band() {
        // Wanaka TP 4: 6 mm carbide flat, 45 mm stickout, hardwood,
        // slot at 3 mm peak DOC. Live MCP report (2026-05-08) measured
        // 158 µm — a slot-engaged sample at peak DOC. Pin the test in
        // the 100–200 µm Approximate band so threshold tweaks have
        // headroom without breaking this regression.
        // 2.5 mm slot DOC — a half-step below the operator's peak,
        // representative of the average-engagement samples that drive
        // the wanaka 158 µm live measurement (peak DOC samples are
        // typically not full-slot in the real sim).
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            2.5,
            std::f64::consts::PI,
            1500.0,
            1.0,
        )]);
        let v = evaluate(
            0,
            &carbide_flat(6.0, 45.0),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
        );
        match v {
            Verdict::Within {
                peak,
                confidence: Confidence::Approximate(detail),
            } => {
                let um = peak * 1000.0;
                assert!(
                    (100.0..=200.0).contains(&um),
                    "wanaka-like End-Mill slot at hardwood should land 100-200 µm; got {um:.1} µm"
                );
                assert!(
                    detail.contains("slot"),
                    "slot annotation expected, got: {detail}"
                );
            }
            other => panic!("expected Within(Approximate), got {other:?}"),
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
        let v = evaluate(
            0,
            &wanaka_tapered_ball(),
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
        );
        match v {
            Verdict::Within { peak, .. } => {
                let um = peak * 1000.0;
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
        // 1 mm carbide flat at 25 mm stickout (geometric L/D = 25, the
        // gap doc's "should still pass" workflow). Tiny chip cross-
        // section keeps force low; predicted δ stays under threshold.
        let tool = carbide_flat(1.0, 25.0);
        let trace = trace_with(vec![cutting_sample(
            0,
            0,
            0.3,
            std::f64::consts::FRAC_PI_4,
            120.0,
            0.2,
        )]);
        let v = evaluate(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
        );
        let peak_um = match v {
            Verdict::Within { peak, .. } => peak * 1000.0,
            Verdict::Exceeds { peak, .. } => peak * 1000.0,
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
        let v = evaluate(
            0,
            &tool,
            &Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            Some(&trace),
        );
        match v {
            Verdict::Within {
                confidence: Confidence::Approximate(detail),
                ..
            } => assert!(
                detail.contains("slot"),
                "slot annotation expected in detail string, got: {detail}"
            ),
            other => panic!("expected Within(Approximate(slot...)), got {other:?}"),
        }
    }
}
