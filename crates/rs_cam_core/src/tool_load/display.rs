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
use crate::simulation_cut::{SimulationCutSample, SimulationCutTrace};

/// The display measure: **achieved advance per tooth** for one sample,
/// `effective_feed / (rpm · flutes)`.
///
/// Identical in expression to the chipload gate's own observation
/// (`super::chipload::achieved_feed_per_tooth_mm`), which delegates here
/// so the two cannot drift. `None` when the sample carries no positive
/// `rpm × flutes` divisor — a broken sample, not a modelling limit.
#[must_use]
pub fn achieved_advance_per_tooth(
    sample: &SimulationCutSample,
    predicted_feeds: &crate::machine_kinematics::PredictedFeedMap,
) -> Option<AdvancePerToothMm> {
    AdvancePerToothMm::from_achieved(
        AchievedFeedMmMin::new(super::effective_feed_for_sample(sample, predicted_feeds)),
        sample.spindle_rpm,
        sample.flute_count,
    )
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
