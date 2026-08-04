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
use crate::ids::ToolpathId;
use crate::material::Material;
use serde::{Deserialize, Serialize};

/// Per-peck sample emitted by drill cycles.
///
/// One entry per (hole, peck) pair. The "peck" concept includes the single
/// full-depth feed in `Simple` / `Dwell` cycles (peck_index = 0), and each
/// progressive descent in `Peck` / `ChipBreak` cycles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillSample {
    pub toolpath_id: ToolpathId,
    /// Index into [`DrillOp::holes`].
    pub hole_id: usize,
    /// 0-based peck index within this hole.
    pub peck_index: u32,
    /// Z distance FED on this descent (mm), always positive — what the
    /// emitter emits, air included.
    ///
    /// R-2 (2026-08-04): this used to be the modelled cutting descent
    /// of a cycle rooted at `hole.top_z`, which is not the cycle the
    /// emitter emits. The emitter roots at the R-plane (the Fanuc G83
    /// convention, and correct), so the first descent of every hole is
    /// partly or wholly air, and every descent after a retract re-feeds
    /// the re-entry clearance. Both are real machine time and both were
    /// invisible. See [`cutting_descent_mm`] for the part that cuts.
    ///
    /// [`cutting_descent_mm`]: DrillSample::cutting_descent_mm
    pub descent_mm: f64,
    /// The portion of `descent_mm` below the material surface — the
    /// part that removes stock. `0.0` for a descent that is entirely
    /// approach through air above `top_z`.
    ///
    /// Gate-relevant depth reads this, never `descent_mm`: adding air
    /// above the stock must not move a verdict.
    pub cutting_descent_mm: f64,
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
    pub toolpath_id: ToolpathId,
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
    /// Index into [`DrillOp::holes`] of the hole `deepest_hole_mm` came
    /// from — the hole every depth-derived gate verdict is about.
    /// `None` when the toolpath has no hole with positive depth.
    ///
    /// R-7 (2026-08-04): drill exceedance diagnostics carried
    /// `SampleRange { sample_start: 0, sample_end: 0 }`, pointing the
    /// operator at a real sample that had nothing to do with the
    /// finding. Drill samples are keyed `(hole_id, peck_index)`, so the
    /// identity was available and simply never carried.
    pub deepest_hole_index: Option<usize>,
    /// `deepest_hole_mm / tool_diameter` — total-hole ratio, kept for
    /// cycle-time / geometry reads.
    pub max_depth_to_diameter: f64,
    /// Evacuation-credited depth-to-diameter the chip-welding risk was
    /// classified from — see [`effective_chip_welding_dtd`]. Equals
    /// `max_depth_to_diameter` for `Simple` / `Dwell`, the deepest
    /// single peck's ratio for `Peck`, half the total for `ChipBreak`.
    pub chip_welding_dtd: f64,
    /// Material-aware classification — see [`ChipWeldingRisk`].
    pub chip_welding_risk: ChipWeldingRisk,
    /// Worst single-peck depth-to-diameter ratio in this toolpath —
    /// the number [`peck_pattern_adequate`] and the peck-adequacy gate
    /// are both decided from.
    ///
    /// R-6 (2026-08-04): this quantity was computed twice and exposed
    /// zero times. `build_drill_toolpath_summary` derived it from the
    /// sample stream and stored only the boolean;
    /// `tool_load::drill_gates::evaluate_peck_adequacy` re-derived it
    /// from the config. Two implementations of one number, and no
    /// consumer could display the number behind the boolean. There is
    /// now one implementation — [`per_peck_max_depth_to_diameter_of`] —
    /// and the gate reads this field.
    ///
    /// [`peck_pattern_adequate`]: DrillToolpathSummary::peck_pattern_adequate
    pub per_peck_max_dtd: f64,
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

/// Thin convenience wrapper around
/// [`Material::drill_chip_welding_threshold_dtd`]. The canonical
/// dispatch lives on `Material` to match the rest of the per-material
/// accessor pattern (`kc_n_per_mm2`, `hardness_index`, etc.); this
/// free function exists for the established call-sites in this module
/// and `tool_load::drill_gates`. New code should call the method
/// directly.
pub fn chip_welding_threshold(material: &Material) -> f64 {
    material.drill_chip_welding_threshold_dtd()
}

/// Thin convenience wrapper around
/// [`Material::drill_per_peck_max_dtd`]. See
/// [`chip_welding_threshold`] for the rationale.
pub fn per_peck_max_depth_to_diameter(material: &Material) -> f64 {
    material.drill_per_peck_max_dtd()
}

/// The worst single-peck depth-to-diameter ratio a cycle produces.
///
/// **The** implementation (R-6). `Simple` / `Dwell` cut the whole hole
/// in one descent, so the worst peck is the hole; `Peck` / `ChipBreak`
/// step by the nominal peck depth, except where the hole is shallower
/// than one peck, in which case the single descent is the hole.
///
/// Deliberately closed-form over the *cutting* geometry rather than a
/// max over the emitted sample stream. The two agreed exactly before
/// R-2, because the first modelled descent was `min(peck, total)`; they
/// would stop agreeing the moment the sample stream models the
/// emitter's R-plane rooting, where the first descent carries air. A
/// gate must not read a number that moves when air is added above the
/// stock, so it reads this.
pub fn per_peck_max_depth_to_diameter_of(
    cycle: DrillCycle,
    deepest_hole_mm: f64,
    diameter_mm: f64,
) -> f64 {
    let diameter = diameter_mm.max(f64::MIN_POSITIVE);
    match cycle {
        DrillCycle::Simple | DrillCycle::Dwell(_) => deepest_hole_mm / diameter,
        DrillCycle::Peck(peck) | DrillCycle::ChipBreak(peck, _) => {
            peck.min(deepest_hole_mm) / diameter
        }
    }
}

/// Per-peck chip-evacuation heuristic.
///
/// `cum_depth_to_diameter`: cumulative descent / diameter after this peck.
/// `per_peck_dtd`: this peck's own cutting descent / diameter — what
/// the flutes have to clear in one go before the next retract.
/// `cycle`: the drill cycle in effect — `Peck` retracts fully between
/// pecks, so the flutes only ever hold one peck's worth of chips;
/// `Simple` / `Dwell` keep all chips in the flutes (score falls off with
/// total depth); `ChipBreak` breaks chips but doesn't clear them
/// (intermediate falloff).
///
/// R-5 (2026-08-04): the `Peck` arm returned a hard **1.0 regardless of
/// peck depth**, so `avg_chip_evacuation_score` was 1.00 for any pecking
/// op — including one whose peck was 7.33×D. Narrate printed, two lines
/// apart, `peck pattern INADEQUATE — reduce peck depth` and
/// `mean chip-evacuation score 1.00 (0=trapped, 1=cleared)`. A score
/// whose own legend says 1 = cleared, beside a verdict saying evacuation
/// is inadequate, because the arm ignored the depth the verdict was
/// about.
///
/// The arm now falls off with the **per-peck** ratio on the same
/// exponential the other arms use against the material's chip-welding
/// threshold — the retract earns the credit, the peck depth spends it.
/// A shallow peck still scores ~1.0, which is what makes the credit
/// meaningful.
pub fn chip_evacuation_score(
    cum_depth_to_diameter: f64,
    per_peck_dtd: f64,
    cycle: DrillCycle,
    material: &Material,
) -> f64 {
    let threshold = chip_welding_threshold(material);
    match cycle {
        DrillCycle::Peck(_) => (-per_peck_dtd.max(0.0) / threshold).exp().clamp(0.0, 1.0),
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
pub fn emit_drill_samples(toolpath_id: ToolpathId, drill_op: &DrillOp) -> Vec<DrillSample> {
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
        // R-2: the emitter's own schedule, rooted at the R-plane. A
        // `retract_z_mm` below `top_z` (never produced by
        // `effective_safe_z`, but a hand-built `DrillOp` can express
        // it) degrades to starting at the surface rather than
        // inventing negative air.
        let entry_z = drill_op.retract_z_mm.max(hole.top_z);
        let descents = crate::drill::fed_descents(drill_op.cycle, hole.bottom_z, entry_z);
        let n = descents.len();
        for (i, descent) in descents.iter().enumerate() {
            let cutting = descent.cutting_length(hole.top_z);
            // Depth below the surface reached by the END of this
            // descent — the emitter's grid revisits depth after a
            // re-entry, so this is a position, not a running sum.
            let cum = (hole.top_z - descent.to_z).max(0.0);
            let dtd = cum / diameter;
            let score =
                chip_evacuation_score(dtd, cutting / diameter, drill_op.cycle, &drill_op.material);
            let dwell_s = if i + 1 == n { dwell_each } else { 0.0 };
            samples.push(DrillSample {
                toolpath_id,
                hole_id,
                peck_index: i as u32,
                descent_mm: descent.length(),
                cutting_descent_mm: cutting,
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
    toolpath_id: ToolpathId,
    drill_op: &DrillOp,
    samples: &[DrillSample],
) -> DrillToolpathSummary {
    let diameter = drill_op.tool_diameter_mm.max(f64::MIN_POSITIVE);
    let feed = drill_op.feed_rate_mm_min.max(f64::MIN_POSITIVE);
    let mut deepest = 0.0_f64;
    let mut deepest_hole_index: Option<usize> = None;
    for (idx, hole) in drill_op.holes.iter().enumerate() {
        let d = (hole.top_z - hole.bottom_z).max(0.0);
        if d > deepest {
            deepest = d;
            deepest_hole_index = Some(idx);
        }
    }
    let max_dtd = deepest / diameter;

    let mut feed_time_s = 0.0;
    let mut dwell_time_s = 0.0;
    let mut weighted_score_sum = 0.0;
    let mut peck_count = 0usize;
    // R-6: one implementation, closed-form over the cutting geometry —
    // NOT a max over the sample stream, whose descents model the
    // emitter and therefore include air above the stock.
    let per_peck_max_dtd = per_peck_max_depth_to_diameter_of(drill_op.cycle, deepest, diameter);
    for s in samples {
        let peck_time = (s.descent_mm / feed) * 60.0;
        feed_time_s += peck_time;
        dwell_time_s += s.dwell_s;
        weighted_score_sum += s.chip_evacuation_score * peck_time;
        peck_count += 1;
    }
    let avg_chip_evacuation_score = if feed_time_s > 0.0 {
        weighted_score_sum / feed_time_s
    } else {
        0.0
    };
    let peck_pattern_adequate =
        per_peck_max_dtd <= per_peck_max_depth_to_diameter(&drill_op.material);
    let effective_welding_dtd =
        effective_chip_welding_dtd(max_dtd, per_peck_max_dtd, drill_op.cycle);
    let chip_welding_risk = classify_chip_welding(effective_welding_dtd, &drill_op.material);

    let _ = ToolProfile::Flat; // reference to silence unused-import worry in cone-only refactors

    DrillToolpathSummary {
        toolpath_id,
        hole_count: drill_op.holes.len(),
        peck_count,
        feed_time_s,
        dwell_time_s,
        deepest_hole_mm: deepest,
        deepest_hole_index,
        max_depth_to_diameter: max_dtd,
        chip_welding_dtd: effective_welding_dtd,
        chip_welding_risk,
        per_peck_max_dtd,
        peck_pattern_adequate,
        avg_chip_evacuation_score,
    }
}

/// Effective depth-to-diameter for chip-welding classification, with
/// the drill cycle's evacuation credited (F1, 2026-06-10 defect-class
/// cleanup — pre-fix the classifier keyed on total-hole D/d even for
/// peck cycles, so a pecking drill read Elevated while its own remedy
/// text said "switch to a peck cycle").
///
/// Credit follows the same convention as [`chip_evacuation_score`]:
/// - `Peck`: full retract clears the flutes between pecks, so the
///   deepest *single peck* governs chip packing, not the total hole.
/// - `ChipBreak`: chips broken but not evacuated — half credit on the
///   total depth (matches the 0.5 effective-depth factor in
///   `chip_evacuation_score`).
/// - `Simple` / `Dwell`: chips stay in the flutes — total-hole D/d,
///   unchanged.
pub fn effective_chip_welding_dtd(total_dtd: f64, per_peck_max_dtd: f64, cycle: DrillCycle) -> f64 {
    match cycle {
        DrillCycle::Simple | DrillCycle::Dwell(_) => total_dtd,
        DrillCycle::ChipBreak(_, _) => total_dtd * 0.5,
        DrillCycle::Peck(_) => per_peck_max_dtd,
    }
}

/// Classify an (evacuation-credited) depth-to-diameter against material
/// thresholds. Bands are half-open: Low `[0, 0.75t)`, Elevated
/// `[0.75t, t)`, High `[t, ∞)` — an observation at exactly 0.75× the
/// threshold reads Elevated.
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
            // R-2: no R-plane air in this fixture — it models the cycle
            // from the material surface, which is what this test's numbers
            // were written against. Production sets
            // `effective_safe_z(cfg.retract_z, stock_top)` (= stock top +
            // 5 mm by default); `drill_evidence_wording_d3.rs` is the
            // sentry that pins the emitter-matching case.
            retract_z_mm: 0.0,
        }
    }

    #[test]
    fn simple_cycle_emits_one_peck_per_hole() {
        let op = op_with(DrillCycle::Simple, 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(ToolpathId(7), &op);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].toolpath_id, ToolpathId(7));
        assert_eq!(samples[0].hole_id, 0);
        assert_eq!(samples[0].peck_index, 0);
        assert!((samples[0].descent_mm - 12.0).abs() < 1e-9);
        assert!((samples[0].cumulative_depth_mm - 12.0).abs() < 1e-9);
    }

    /// R-2 re-pin. 12 mm depth, 5 mm peck, fixture rooted at the
    /// surface (`retract_z_mm: 0.0`) so the only change from the old
    /// expectation is the one R-2 is about.
    ///
    /// Before: this asserted fed descents of 5, 5, 2 — the *modelled*
    /// cycle. The emitter retracts to the R-plane after each peck and
    /// rapids back to `previous_depth + 0.5`, so it FEEDS that 0.5 mm
    /// again: the real descents are 5.0, 5.5, 2.5, and the last one
    /// still lands the hole at 12 mm. The old numbers were not a
    /// tighter bar, they were a different cycle.
    #[test]
    fn peck_cycle_emits_one_per_peck_with_final_clamp() {
        let op = op_with(DrillCycle::Peck(5.0), 4.0, 12.0, Material::default());
        let samples = emit_drill_samples(ToolpathId(0), &op);
        assert_eq!(samples.len(), 3);
        // Fed distance, including the re-cut re-entry clearance.
        assert!((samples[0].descent_mm - 5.0).abs() < 1e-9);
        assert!((samples[1].descent_mm - 5.5).abs() < 1e-9);
        assert!((samples[2].descent_mm - 2.5).abs() < 1e-9);
        // With no air above the surface, every fed millimetre cuts.
        for s in &samples {
            assert!(
                (s.cutting_descent_mm - s.descent_mm).abs() < 1e-9,
                "no R-plane air in this fixture, so fed == cutting"
            );
        }
        // Depth reached is unchanged — that is the invariant the old
        // expectation was really protecting.
        assert!((samples[2].cumulative_depth_mm - 12.0).abs() < 1e-9);
    }

    #[test]
    fn effective_chip_welding_dtd_credits_cycle() {
        // Simple / Dwell: total-hole ratio, unchanged.
        assert_eq!(
            effective_chip_welding_dtd(6.0, 0.5, DrillCycle::Simple),
            6.0
        );
        assert_eq!(
            effective_chip_welding_dtd(6.0, 0.5, DrillCycle::Dwell(0.5)),
            6.0
        );
        // ChipBreak: chips broken but not evacuated — half credit,
        // matching `chip_evacuation_score`'s 0.5 effective-depth factor.
        assert_eq!(
            effective_chip_welding_dtd(6.0, 0.5, DrillCycle::ChipBreak(2.0, 0.2)),
            3.0
        );
        // Peck: full retract clears the flutes — deepest single peck governs.
        assert_eq!(
            effective_chip_welding_dtd(6.0, 0.5, DrillCycle::Peck(2.0)),
            0.5
        );
    }

    #[test]
    fn summary_chip_welding_uses_evacuation_credited_dtd() {
        // Ø4 × 40 mm: total D/d = 10 (Critical for softwood threshold 8
        // when Simple), but Peck(2) credits to per-peck 0.5 → Low.
        let op = op_with(DrillCycle::Peck(2.0), 4.0, 40.0, Material::default());
        let samples = emit_drill_samples(ToolpathId(0), &op);
        let s = build_drill_toolpath_summary(ToolpathId(0), &op, &samples);
        assert_eq!(s.max_depth_to_diameter, 10.0, "total ratio preserved");
        assert!(
            (s.chip_welding_dtd - 0.5).abs() < 1e-9,
            "credited ratio = deepest peck / d, got {}",
            s.chip_welding_dtd
        );
        assert_eq!(s.chip_welding_risk, ChipWeldingRisk::Low);
    }

    /// R-5 re-pin. A pecking cycle still scores high — that is the
    /// credit the retract earns — but no longer a flat 1.0 regardless
    /// of peck depth, which is what let narrate print
    /// "score 1.00 (1=cleared)" beside "peck pattern INADEQUATE".
    ///
    /// 2 mm peck on Ø4 = 0.5 D/d against the softwood chip-welding
    /// threshold 8: exp(-0.5/8) = 0.939.
    #[test]
    fn peck_cycle_chip_evacuation_is_high_but_not_free() {
        let op = op_with(DrillCycle::Peck(2.0), 4.0, 16.0, Material::default());
        let samples = emit_drill_samples(ToolpathId(0), &op);
        for s in &samples {
            assert!(
                s.chip_evacuation_score >= 0.9,
                "a shallow peck cycle should evacuate well; got {}",
                s.chip_evacuation_score
            );
        }
        // A peck an order of magnitude deeper must score materially
        // worse on the same hole.
        let deep = op_with(DrillCycle::Peck(16.0), 4.0, 16.0, Material::default());
        let deep_samples = emit_drill_samples(ToolpathId(0), &deep);
        let worst = deep_samples
            .iter()
            .map(|s| s.chip_evacuation_score)
            .fold(f64::INFINITY, f64::min);
        assert!(
            worst < 0.7,
            "a 4×D single peck must not claim to be cleared; got {worst}"
        );
    }

    #[test]
    fn simple_cycle_evacuation_falls_off_with_depth() {
        let shallow = op_with(DrillCycle::Simple, 4.0, 4.0, Material::default());
        let deep = op_with(DrillCycle::Simple, 4.0, 40.0, Material::default());
        let s1 = &emit_drill_samples(ToolpathId(0), &shallow)[0];
        let s2 = &emit_drill_samples(ToolpathId(0), &deep)[0];
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
        let samples = emit_drill_samples(ToolpathId(0), &op);
        assert_eq!(samples.len(), 1);
        assert!((samples[0].dwell_s - 0.5).abs() < 1e-9);

        // Peck cycles never dwell.
        let peck_op = op_with(DrillCycle::Peck(5.0), 4.0, 12.0, Material::default());
        for s in emit_drill_samples(ToolpathId(0), &peck_op) {
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
        let samples = emit_drill_samples(ToolpathId(0), &op);
        let summary = build_drill_toolpath_summary(ToolpathId(0), &op, &samples);
        assert_eq!(summary.toolpath_id, ToolpathId(0));
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
        let samples = emit_drill_samples(ToolpathId(0), &op);
        let summary = build_drill_toolpath_summary(ToolpathId(0), &op, &samples);
        assert_eq!(summary.chip_welding_risk, ChipWeldingRisk::High);
    }

    #[test]
    fn peck_pattern_inadequate_when_single_peck_too_deep() {
        // Ø2 tool, single peck depth 14 mm → d/D = 7.0, softwood
        // threshold = 6.0 (Janka-banded, 2026-06-03) → inadequate.
        let op = op_with(DrillCycle::Peck(14.0), 2.0, 14.0, Material::default());
        let samples = emit_drill_samples(ToolpathId(0), &op);
        let summary = build_drill_toolpath_summary(ToolpathId(0), &op, &samples);
        assert!(
            !summary.peck_pattern_adequate,
            "single peck of d/D = 7.0 should be flagged inadequate vs softwood 6.0 threshold"
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
