//! Drill-native per-peck metrics — DEXEL roadmap §6.E / Step 3 PR2.
//!
//! For drill operations, the dexel simulator bypasses per-segment stamping
//! (see [`crate::dexel_stock::TriDexelStock::apply_drill_op`]) and the usual
//! `SimulationCutSample` stream is empty. Instead, drill cycles produce
//! their own samples: one [`DrillSample`] per peck per hole, plus a
//! per-toolpath [`DrillToolpathSummary`] that aggregates peck adequacy,
//! chip-welding risk, and cycle time.
//!
//! Both types are carried on [`crate::simulation_cut::SimulationCutTrace`]
//! alongside the engagement-side samples and summaries so MCP / narrate /
//! tool-load consumers can read drill metrics from the same trace surface
//! used for milling ops.

use crate::drill::DrillCycle;
use crate::drill_op::{DrillOp, ToolProfile};
use crate::material::Material;
use serde::{Deserialize, Serialize};

/// Per-peck sample emitted by drill cycles.
///
/// One entry per (hole, peck) pair. The "peck" concept includes the single
/// full-depth feed in `Simple` / `Dwell` cycles (peck_index = 0), and each
/// progressive descent in `Peck` / `ChipBreak` cycles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillSample {
    pub toolpath_id: usize,
    /// Index into [`DrillOp::holes`].
    pub hole_id: usize,
    /// 0-based peck index within this hole.
    pub peck_index: u32,
    /// Z descent of this peck (mm), always positive. For the last peck of a
    /// `Peck` / `ChipBreak` cycle this is typically smaller than the cycle's
    /// nominal peck depth (it's clamped against the hole's `bottom_z`).
    pub descent_mm: f64,
    /// Cumulative descent below the hole's `top_z` after this peck (mm).
    /// Used by gates to compute depth-to-diameter ratios.
    pub cumulative_depth_mm: f64,
    /// Axial chip-load — feed per spindle revolution. For drilling this is
    /// the natural unit (independent of flute count, unlike milling's
    /// per-tooth chipload).
    pub axial_chipload_mm_per_rev: f64,
    /// Dwell time at the bottom of this peck (seconds). Non-zero only for
    /// `Dwell` cycles, and only on the final peck.
    pub dwell_s: f64,
    /// 0..1 heuristic estimate of chip-evacuation quality at this peck. 1.0
    /// = chips fully cleared (Peck cycle, shallow), 0.0 = chips trapped
    /// (Simple cycle, deep relative to diameter). Material-dependent — see
    /// [`chip_evacuation_score`].
    pub chip_evacuation_score: f64,
}

/// Chip-welding risk classification for a hole based on depth-to-diameter
/// ratio against the material's safe envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipWeldingRisk {
    /// Depth-to-diameter is well below the material's chip-welding threshold.
    Low,
    /// Depth-to-diameter is approaching the material's threshold (within
    /// 25%). Operator should consider switching to a peck cycle if currently
    /// running `Simple` / `Dwell`.
    Elevated,
    /// Depth-to-diameter exceeds the material's threshold. Without a peck
    /// cycle the operation is likely to weld chips to the flutes and stall
    /// the cutter.
    High,
}

/// Per-toolpath summary for drilling operations.
///
/// Lives on [`crate::simulation_cut::SimulationCutTrace::drill_summaries`]
/// (one entry per drill toolpath). Mirrors the role of
/// [`crate::simulation_cut::SimulationToolpathCutSummary`] for milling ops —
/// the same `toolpath_id` joins the two views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillToolpathSummary {
    pub toolpath_id: usize,
    pub hole_count: usize,
    /// Total pecks emitted across all holes (sum of peck_index + 1 per hole).
    pub peck_count: usize,
    /// Sum of feed-down time across every peck (seconds). Excludes rapid
    /// retract / repositioning between holes, which are bookkept by the
    /// existing runtime accounting on the linearized toolpath.
    pub feed_time_s: f64,
    /// Total dwell time across all holes (seconds). Non-zero only for
    /// `Dwell` cycles.
    pub dwell_time_s: f64,
    /// `top_z - bottom_z` for the deepest hole in the toolpath (mm).
    pub deepest_hole_mm: f64,
    /// `deepest_hole_mm / tool_diameter`. The primary input to the
    /// chip-welding gate.
    pub max_depth_to_diameter: f64,
    /// Material-aware classification — see [`ChipWeldingRisk`].
    pub chip_welding_risk: ChipWeldingRisk,
    /// True when the cycle's peck depth (or absence of pecking, for
    /// `Simple` / `Dwell`) keeps each single peck under the material's safe
    /// per-peck depth-to-diameter ratio. False indicates the operator
    /// should reduce peck depth or switch from `Simple` / `Dwell` to a
    /// pecking cycle.
    pub peck_pattern_adequate: bool,
    /// Time-weighted mean of `chip_evacuation_score` across every emitted
    /// peck (0..1). Aggregate read for narrate / MCP.
    pub avg_chip_evacuation_score: f64,
}

/// Material-aware threshold (depth-to-diameter ratio) above which
/// chip welding becomes likely without a pecking cycle.
///
/// Rough first-pass values for wood-router materials. Steel / aluminum
/// would be ~2x and 3x respectively but aren't currently in [`Material`].
pub fn chip_welding_threshold(material: &Material) -> f64 {
    match material {
        Material::SolidWood { species } => {
            // Softer wood evacuates chips better. Use Janka hardness as a proxy.
            let janka = species.janka_lbf();
            if janka <= 700.0 {
                8.0 // softwood
            } else if janka <= 1500.0 {
                6.0 // medium hardwood
            } else {
                5.0 // dense hardwood
            }
        }
        Material::Plywood { .. } | Material::SheetGood { .. } => 5.0,
        Material::Plastic { .. } => 4.0,
        // Aluminum chip welding starts around D/d ≈ 3 (industry rule of
        // thumb; pecks are mandatory beyond that). Conservative even
        // for 6061 — the deeper the alloy the lower the safe D/d.
        Material::Aluminum { .. } => 3.0,
        Material::Foam { .. } => 12.0,
        Material::Custom { hardness_index, .. } => {
            // Scale roughly with hardness — softer materials evacuate better.
            (8.0 / hardness_index.max(0.5)).clamp(2.0, 12.0)
        }
    }
}

/// Material-aware per-peck depth-to-diameter ratio. A single peck deeper
/// than this is likely to trap chips even within a pecking cycle.
pub fn per_peck_max_depth_to_diameter(material: &Material) -> f64 {
    match material {
        Material::SolidWood { .. } => 2.0,
        Material::Plywood { .. } | Material::SheetGood { .. } => 1.5,
        Material::Plastic { .. } => 1.0,
        // Aluminum per-peck ≤ 1×D is the standard machining-textbook
        // limit for chip evacuation without through-coolant.
        Material::Aluminum { .. } => 1.0,
        Material::Foam { .. } => 4.0,
        Material::Custom { .. } => 1.5,
    }
}

/// Per-peck chip-evacuation heuristic.
///
/// `cum_depth_to_diameter`: cumulative descent / diameter after this peck.
/// `cycle`: the drill cycle in effect — `Peck` evacuates fully between
/// pecks (high score regardless of depth); `Simple` / `Dwell` keep all
/// chips in the flutes (score falls off with depth); `ChipBreak` breaks
/// chips but doesn't clear them (intermediate falloff).
pub fn chip_evacuation_score(
    cum_depth_to_diameter: f64,
    cycle: DrillCycle,
    material: &Material,
) -> f64 {
    let threshold = chip_welding_threshold(material);
    match cycle {
        DrillCycle::Peck(_) => 1.0,
        DrillCycle::Simple | DrillCycle::Dwell(_) => {
            (-cum_depth_to_diameter / threshold).exp().clamp(0.0, 1.0)
        }
        DrillCycle::ChipBreak(_, _) => {
            // Chip-break flicks the chip but doesn't evacuate it — call
            // the effective depth half what `Simple` would see at this
            // depth.
            (-(cum_depth_to_diameter * 0.5) / threshold)
                .exp()
                .clamp(0.0, 1.0)
        }
    }
}

/// Emit the per-peck [`DrillSample`] stream for a single drill toolpath.
///
/// Walks each hole in `drill_op.holes`, expands `drill_op.cycle` into a
/// sequence of pecks, and produces one `DrillSample` per peck. The order
/// matches the linearized toolpath: hole 0 fully drilled, then hole 1, etc.
pub fn emit_drill_samples(toolpath_id: usize, drill_op: &DrillOp) -> Vec<DrillSample> {
    let diameter = drill_op.tool_diameter_mm.max(f64::MIN_POSITIVE);
    let feed = drill_op.feed_rate_mm_min.max(f64::MIN_POSITIVE);
    let rpm = drill_op.spindle_rpm.max(1) as f64;
    let chipload_mm_per_rev = feed / rpm;
    let dwell_each = match drill_op.cycle {
        DrillCycle::Dwell(s) => s.max(0.0),
        _ => 0.0,
    };

    let mut samples = Vec::new();
    for (hole_id, hole) in drill_op.holes.iter().enumerate() {
        let total_depth = (hole.top_z - hole.bottom_z).max(0.0);
        if total_depth == 0.0 {
            continue;
        }
        let descents = peck_descents(&drill_op.cycle, total_depth);
        let mut cum = 0.0;
        let n = descents.len();
        for (i, descent) in descents.into_iter().enumerate() {
            cum += descent;
            let dtd = cum / diameter;
            let score = chip_evacuation_score(dtd, drill_op.cycle, &drill_op.material);
            let dwell_s = if i + 1 == n { dwell_each } else { 0.0 };
            samples.push(DrillSample {
                toolpath_id,
                hole_id,
                peck_index: i as u32,
                descent_mm: descent,
                cumulative_depth_mm: cum,
                axial_chipload_mm_per_rev: chipload_mm_per_rev,
                dwell_s,
                chip_evacuation_score: score,
            });
        }
    }
    samples
}

/// Build the per-toolpath drill summary from the sample stream and the
/// underlying [`DrillOp`].
///
/// Samples for `toolpath_id` are expected to be supplied already filtered
/// (caller passes only this toolpath's samples).
pub fn build_drill_toolpath_summary(
    toolpath_id: usize,
    drill_op: &DrillOp,
    samples: &[DrillSample],
) -> DrillToolpathSummary {
    let diameter = drill_op.tool_diameter_mm.max(f64::MIN_POSITIVE);
    let feed = drill_op.feed_rate_mm_min.max(f64::MIN_POSITIVE);
    let mut deepest = 0.0_f64;
    for hole in &drill_op.holes {
        let d = (hole.top_z - hole.bottom_z).max(0.0);
        if d > deepest {
            deepest = d;
        }
    }
    let max_dtd = deepest / diameter;
    let chip_welding_risk = classify_chip_welding(max_dtd, &drill_op.material);

    let mut feed_time_s = 0.0;
    let mut dwell_time_s = 0.0;
    let mut weighted_score_sum = 0.0;
    let mut peck_count = 0usize;
    let mut per_peck_max_dtd = 0.0_f64;
    for s in samples {
        let peck_time = (s.descent_mm / feed) * 60.0;
        feed_time_s += peck_time;
        dwell_time_s += s.dwell_s;
        weighted_score_sum += s.chip_evacuation_score * peck_time;
        peck_count += 1;
        let single_dtd = s.descent_mm / diameter;
        if single_dtd > per_peck_max_dtd {
            per_peck_max_dtd = single_dtd;
        }
    }
    let avg_chip_evacuation_score = if feed_time_s > 0.0 {
        weighted_score_sum / feed_time_s
    } else {
        0.0
    };
    let peck_pattern_adequate =
        per_peck_max_dtd <= per_peck_max_depth_to_diameter(&drill_op.material);

    let _ = ToolProfile::Flat; // reference to silence unused-import worry in cone-only refactors

    DrillToolpathSummary {
        toolpath_id,
        hole_count: drill_op.holes.len(),
        peck_count,
        feed_time_s,
        dwell_time_s,
        deepest_hole_mm: deepest,
        max_depth_to_diameter: max_dtd,
        chip_welding_risk,
        peck_pattern_adequate,
        avg_chip_evacuation_score,
    }
}

/// Classify the deepest-hole depth-to-diameter against material thresholds.
pub fn classify_chip_welding(max_dtd: f64, material: &Material) -> ChipWeldingRisk {
    let t = chip_welding_threshold(material);
    if max_dtd < t * 0.75 {
        ChipWeldingRisk::Low
    } else if max_dtd < t {
        ChipWeldingRisk::Elevated
    } else {
        ChipWeldingRisk::High
    }
}

/// Expand a drill cycle into per-peck Z descents (always positive) totaling
/// `total_depth`. For `Simple` / `Dwell`, returns a single-element vector.
fn peck_descents(cycle: &DrillCycle, total_depth: f64) -> Vec<f64> {
    match *cycle {
        DrillCycle::Simple | DrillCycle::Dwell(_) => vec![total_depth],
        DrillCycle::Peck(peck) | DrillCycle::ChipBreak(peck, _) => {
            let peck = peck.max(f64::MIN_POSITIVE);
            let mut out = Vec::new();
            let mut remaining = total_depth;
            while remaining > 1e-9 {
                let step = remaining.min(peck);
                out.push(step);
                remaining -= step;
            }
            if out.is_empty() {
                out.push(total_depth);
            }
            out
        }
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
    use crate::drill_op::{DrillHole, HoleSource};

    fn op_with(cycle: DrillCycle, diameter: f64, depth: f64, material: Material) -> DrillOp {
        DrillOp {
            holes: vec![DrillHole {
                xy: [0.0, 0.0],
                top_z: 0.0,
                bottom_z: -depth,
            }],
            hole_source: HoleSource::ModelDerived,
            tool_profile: ToolProfile::StandardTwist,
            tool_diameter_mm: diameter,
            cycle,
            feed_rate_mm_min: 300.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            material,
        }
    }

    #[test]
    fn simple_cycle_emits_one_peck_per_hole() {
        let op = op_with(DrillCycle::Simple, 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(7, &op);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].toolpath_id, 7);
        assert_eq!(samples[0].hole_id, 0);
        assert_eq!(samples[0].peck_index, 0);
        assert!((samples[0].descent_mm - 12.0).abs() < 1e-9);
        assert!((samples[0].cumulative_depth_mm - 12.0).abs() < 1e-9);
    }

    #[test]
    fn peck_cycle_emits_one_per_peck_with_final_clamp() {
        // 12 mm depth, 5 mm peck → descents 5, 5, 2.
        let op = op_with(DrillCycle::Peck(5.0), 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        assert_eq!(samples.len(), 3);
        assert!((samples[0].descent_mm - 5.0).abs() < 1e-9);
        assert!((samples[1].descent_mm - 5.0).abs() < 1e-9);
        assert!((samples[2].descent_mm - 2.0).abs() < 1e-9);
        assert!((samples[2].cumulative_depth_mm - 12.0).abs() < 1e-9);
    }

    #[test]
    fn peck_cycle_chip_evacuation_is_high() {
        let op = op_with(DrillCycle::Peck(2.0), 4.0, 16.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        for s in &samples {
            assert!(
                s.chip_evacuation_score >= 0.99,
                "Peck cycle should fully evacuate; got {}",
                s.chip_evacuation_score
            );
        }
    }

    #[test]
    fn simple_cycle_evacuation_falls_off_with_depth() {
        let shallow = op_with(DrillCycle::Simple, 4.0, 4.0, Material::default());
        let deep = op_with(DrillCycle::Simple, 4.0, 40.0, Material::default());
        let s1 = &emit_drill_samples(0, &shallow)[0];
        let s2 = &emit_drill_samples(0, &deep)[0];
        assert!(
            s2.chip_evacuation_score < s1.chip_evacuation_score,
            "deeper hole should evacuate worse: shallow={} deep={}",
            s1.chip_evacuation_score,
            s2.chip_evacuation_score
        );
    }

    #[test]
    fn dwell_only_on_final_peck() {
        let op = op_with(DrillCycle::Dwell(0.5), 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        assert_eq!(samples.len(), 1);
        assert!((samples[0].dwell_s - 0.5).abs() < 1e-9);

        // Peck cycles never dwell.
        let peck_op = op_with(DrillCycle::Peck(5.0), 4.0, 12.0, Material::default());
        for s in emit_drill_samples(0, &peck_op) {
            assert_eq!(s.dwell_s, 0.0);
        }
    }

    #[test]
    fn chip_welding_classification_progresses_with_depth() {
        let mat = Material::default(); // softwood — threshold ~8
        assert_eq!(classify_chip_welding(2.0, &mat), ChipWeldingRisk::Low);
        assert_eq!(classify_chip_welding(7.0, &mat), ChipWeldingRisk::Elevated);
        assert_eq!(classify_chip_welding(10.0, &mat), ChipWeldingRisk::High);
    }

    #[test]
    fn summary_aggregates_pecks_and_classifies_risk() {
        let op = op_with(DrillCycle::Peck(3.0), 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        let summary = build_drill_toolpath_summary(0, &op, &samples);
        assert_eq!(summary.toolpath_id, 0);
        assert_eq!(summary.hole_count, 1);
        assert_eq!(summary.peck_count, 4);
        assert!((summary.deepest_hole_mm - 12.0).abs() < 1e-9);
        assert!((summary.max_depth_to_diameter - 3.0).abs() < 1e-9);
        // 12mm / 4mm = 3.0, threshold = 8, so well below → Low.
        assert_eq!(summary.chip_welding_risk, ChipWeldingRisk::Low);
        // Each peck is 3mm, diameter 4mm → 0.75 ratio, threshold 2.0 → adequate.
        assert!(summary.peck_pattern_adequate);
    }

    #[test]
    fn simple_cycle_at_deep_hole_flags_high_welding_risk() {
        // 32 mm depth in Ø4 softwood (d/D = 8.0; threshold = 8 → exactly at edge).
        let op = op_with(DrillCycle::Simple, 4.0, 36.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        let summary = build_drill_toolpath_summary(0, &op, &samples);
        assert_eq!(summary.chip_welding_risk, ChipWeldingRisk::High);
    }

    #[test]
    fn peck_pattern_inadequate_when_single_peck_too_deep() {
        // Ø2 tool, single peck depth 6 mm → d/D = 3.0, threshold = 2.0 → inadequate.
        let op = op_with(DrillCycle::Peck(6.0), 2.0, 6.0, Material::default());
        let samples = emit_drill_samples(0, &op);
        let summary = build_drill_toolpath_summary(0, &op, &samples);
        assert!(
            !summary.peck_pattern_adequate,
            "single peck of d/D = 3.0 should be flagged inadequate"
        );
    }

    /// F-016: chip-welding threshold must branch on material. Pre-fix
    /// callers passed `Material::default()` everywhere, so hardwood and
    /// plastic stocks silently inherited the softwood threshold.
    #[test]
    fn chip_welding_threshold_per_material_family() {
        use crate::material::{PlasticFamily, WoodSpecies};

        let softwood = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let hardwood = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let dense_hardwood = Material::SolidWood {
            species: WoodSpecies::Jarrah,
        };
        let plastic = Material::Plastic {
            family: PlasticFamily::Acrylic,
        };

        assert!((chip_welding_threshold(&softwood) - 8.0).abs() < 1e-9);
        // GenericHardwood (janka 1450) → medium-hardwood band (6.0).
        assert!((chip_welding_threshold(&hardwood) - 6.0).abs() < 1e-9);
        // Jarrah (janka 1910) → dense-hardwood band (5.0).
        assert!((chip_welding_threshold(&dense_hardwood) - 5.0).abs() < 1e-9);
        // Plastic → 4.0 per CLAUDE.md drill threshold table.
        assert!((chip_welding_threshold(&plastic) - 4.0).abs() < 1e-9);
    }
}
