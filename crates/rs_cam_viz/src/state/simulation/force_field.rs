//! The depth top of the force field of the sim width x depth chart.
//!
//! P29-10 10g (assembly sites 18 to 20): the field moved to core
//! (`rs_cam_core::tool_load::force_field`, built through the session door
//! `ProjectSession::force_plane` once per load report in the cut-metric
//! memo). Only the depth top stays here: it is a view fact, the top of the
//! scene the chart draws, so the field covers every depth the chart shows.

/// The depth top (mm) of the field of an operation with the envelope
/// `env` and the cut samples `samples`: the top of the scene the width x
/// depth chart draws (`EnvelopeScene::with_depth_mark` at the deepest
/// sample, as `draw_cut_chart` builds it). The chart picks no range above
/// it, so a contour reaches every depth the chart shows. The envelope's
/// own view top is not enough: on the rivmap Face 5 it is 1.06 mm under
/// samples to 2 mm (P17b round 1).
#[must_use]
pub fn field_top(
    env: &rs_cam_core::feeds::envelope::SafeEnvelope,
    samples: &[rs_cam_core::tool_load::cut_force::CutPoint3],
) -> f64 {
    let deepest = samples
        .iter()
        .map(|s| s.ap_mm)
        .filter(|ap| ap.is_finite())
        .fold(0.0_f64, f64::max);
    crate::ui::feeds::envelope::EnvelopeScene::with_depth_mark(
        env,
        None,
        (deepest > 0.0).then_some(deepest),
    )
    .ap_range
    .1
}
