//! The quantities the operator-facing surfaces display, typed.
//!
//! **Why this lives in core.** Until 2026-08-08 the viewport heat-map's
//! measure was a private helper in `rs_cam_viz::app::gpu_upload`
//! (`build_chipload_per_move`) paired with a private colour function in
//! `rs_cam_viz::render::toolpath_render` (`chipload_segment_color`). The
//! pairing was the F-HEATMAP defect — an arc-mean chip thickness handed
//! to a comparison against a vendor band published in advance per tooth
//! — and it was unreachable from the crate that owns the band, so the
//! A-1 census could only *mirror* the thresholds in a test file and hope
//! the mirror stayed faithful.
//!
//! Checkpoint H (`planning/review_2026-08-08/ORCHESTRATION_LOG.md`,
//! binding) ruled the display measure to be the **achieved advance per
//! tooth** — `effective_feed / (rpm · flutes)`, bit-for-bit the quantity
//! the post-simulation chipload gate observes
//! (`super::chipload`'s header, since 2026-08-06). Putting the measure
//! here makes "the colour and the verdict agree" a property a test can
//! assert on production code, which is what
//! `tests/heatmap_two_arc_divergence_a1.rs` now does.
//!
//! Colour is deliberately NOT here: mapping a
//! [`crate::feeds::ChiploadBandClass`] to RGB is a viz concern and stays
//! in `rs_cam_viz::render::toolpath_render`.

use std::collections::HashMap;

use crate::feeds::{AchievedFeedMmMin, AdvancePerToothMm, ArcMeanChipThicknessMm};
use crate::ids::ToolpathId;
use crate::stock::simulation_cut::{SimulationCutSample, SimulationCutTrace};

/// The display measure: **achieved advance per tooth** for one sample,
/// `effective_feed / (rpm · flutes)`.
///
/// P11b2: the chipload gate observes this advance over the radial
/// chip-thinning lift ([`peak_chip_per_tooth`]). `None` when the sample carries no positive
/// `rpm × flutes` divisor — a broken sample, not a modelling limit.
#[must_use]
pub fn achieved_advance_per_tooth(
    sample: &SimulationCutSample,
    predicted_feeds: &crate::machine::kinematics::PredictedFeedMap,
) -> Option<AdvancePerToothMm> {
    AdvancePerToothMm::from_achieved(
        AchievedFeedMmMin::new(super::effective_feed_for_sample(sample, predicted_feeds)),
        sample.spindle_rpm,
        sample.flute_count,
    )
}

/// Feeds contract P11b2: the PEAK chip per tooth of one sample, and the lift
/// that separates it from the achieved advance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakChipReading {
    /// The achieved advance per tooth over the lift (mm/tooth).
    pub peak_chip_mm: f64,
    /// The radial chip-thinning lift at the sample's measured engagement
    /// (`feeds::geometry::RadialThinning::lift`); 1 on a finish pass.
    pub lift: f64,
}

/// Why a sample gives no peak chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeakChipRefusal {
    /// The sample carries no positive `rpm x flutes` divisor.
    NoDivisor,
    /// The cut thins, but the sample gives no engagement
    /// (`feeds::engagement::Engagement::from_sample` is `None`: no arc, or
    /// no positive width), so the lift is not known.
    NoEngagement,
}

/// Feeds contract P11b2 (rulings L1 and L2): the measure of the post-sim
/// chipload gate, its histogram and the heat map. It is the PEAK chip,
/// `achieved advance / lift`, where the lift is the one law
/// (`feeds::geometry::RadialThinning::lift`, Garr) at the sample's measured
/// engagement: the width `r (1 - cos psi)` of the measured arc on the
/// engaged diameter at the sample depth (the K7 `Engagement::from_sample`
/// rule). This is the chip that Suggest sizes the feed for, so the three
/// readers and Suggest judge one chip.
///
/// A finish pass (`RadialThinning::FinishPass`, L2) takes the advance as its
/// peak chip and reads no engagement.
pub fn peak_chip_per_tooth(
    sample: &SimulationCutSample,
    predicted_feeds: &crate::machine::kinematics::PredictedFeedMap,
    tool: &crate::tool::ToolDefinition,
    thinning: crate::feeds::geometry::RadialThinning,
) -> Result<PeakChipReading, PeakChipRefusal> {
    let advance = achieved_advance_per_tooth(sample, predicted_feeds)
        .ok_or(PeakChipRefusal::NoDivisor)?
        .mm();
    let lift = sample_thinning_lift(sample, tool, thinning).ok_or(PeakChipRefusal::NoEngagement)?;
    Ok(PeakChipReading {
        peak_chip_mm: advance / lift,
        lift,
    })
}

/// Feeds contract P11b2: the radial chip-thinning lift of one sample, the one
/// door that the gate ([`peak_chip_per_tooth`]) and the feed modulator (per
/// move, in `session::compute`) read. The lift is
/// `feeds::geometry::RadialThinning::lift` at the sample's measured
/// engagement (`Engagement::from_sample`: the width `r (1 - cos psi)` of the
/// measured arc on the engaged diameter at the sample depth). 1 when the cut
/// does not thin. `None` when the cut thins and the sample gives no
/// engagement (no arc, or no positive width).
#[must_use]
pub fn sample_thinning_lift(
    sample: &SimulationCutSample,
    tool: &crate::tool::ToolDefinition,
    thinning: crate::feeds::geometry::RadialThinning,
) -> Option<f64> {
    match thinning {
        crate::feeds::geometry::RadialThinning::Applied => {
            let engagement = crate::feeds::engagement::Engagement::from_sample(sample, tool)?;
            Some(thinning.lift(engagement.ae_mm(), engagement.d_eng_mm()))
        }
        crate::feeds::geometry::RadialThinning::FinishPass
        | crate::feeds::geometry::RadialThinning::DrillCycle => Some(1.0),
    }
}

/// The sample's dexel-measured arc-average chip thickness, typed so it
/// cannot be handed to a band comparison.
///
/// This is a real engagement/force signal and it keeps exactly one
/// visual (the sim-timeline track, drawn **unbanded**). It is not the
/// unit any shipped vendor chipload column is published in
/// (`CHIPLOAD_LITERATURE_VERDICT.md` §2.3), so no conversion into
/// [`AdvancePerToothMm`] exists and none may be added.
#[must_use]
pub fn arc_mean_chip_thickness(sample: &SimulationCutSample) -> Option<ArcMeanChipThicknessMm> {
    sample
        .effective_chip_thickness_mm
        .map(ArcMeanChipThicknessMm::new)
}

/// Build the `toolpath_id -> { move_index -> achieved advance/tooth }`
/// map the viewport heat-map colours by.
///
/// Worst-case-per-move: several samples share one `move_index`, and the
/// question the colour answers is "did this segment reach the ceiling?".
/// Rapid moves contribute nothing; a sample that cannot state its own
/// spindle is skipped and its move renders as "no data" (dim grey), the
/// same way a chip-model-less sample did before Checkpoint H.
///
/// Pre-Checkpoint-H this function's body read
/// `max(effective_chip_thickness_mm)` — an arc-mean chip thickness — and
/// the resulting colour split one identical recipe across two classes
/// purely on engagement arc. The pre-fix numbers are recorded in
/// `tests/heatmap_two_arc_divergence_a1.rs` and in
/// `planning/review_2026-08-08/HEATMAP_VOCAB_CENSUS.md` §3.
#[must_use]
pub fn advance_per_tooth_per_move(
    sim_trace: Option<&SimulationCutTrace>,
) -> HashMap<ToolpathId, HashMap<usize, AdvancePerToothMm>> {
    let mut map: HashMap<ToolpathId, HashMap<usize, AdvancePerToothMm>> = HashMap::new();
    let Some(trace) = sim_trace else {
        return map;
    };
    for s in &trace.samples {
        if !s.is_cutting {
            continue;
        }
        let Some(advance) = achieved_advance_per_tooth(s, &trace.predicted_feeds) else {
            continue;
        };
        map.entry(s.toolpath_id)
            .or_default()
            .entry(s.move_index)
            .and_modify(|prev| {
                if advance.mm() > prev.mm() {
                    *prev = advance;
                }
            })
            .or_insert(advance);
    }
    map
}
