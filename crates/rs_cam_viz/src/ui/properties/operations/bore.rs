//! The Bore editor (geometry selection P6.6): a helix mills each picked
//! circle to its drawn size.
//!
//! The rows are the floor (the one floor row of Pocket, Profile, Zigzag
//! and Adaptive 2D, P3.7), the helix pitch, Climb and the finish
//! allowance. The feed, the plunge rate and the helix (ramp) feed are on
//! the Feeds tab, as for every op.

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::operation_configs::BoreConfig;
use rs_cam_core::session::HoleSelection;

use super::super::linking_dressup::{Fields, check_in_row, plain_row};
use super::super::{dv, p};
use crate::ui::components::OneLine;

/// What a Bore cuts, printed on the panel so the operator knows which
/// circles the op takes (L-12).
const BORE_SCOPE_NOTE: &str = "Bores circles larger than the tool, up to two tool diameters. \
     Points and other circles are left out with a note.";

pub(in crate::ui::properties) fn draw_bore_params(
    ui: &mut egui::Ui,
    cfg: &mut BoreConfig,
    holes: Option<&HoleSelection>,
    fields: Fields<'_>,
) {
    dv(
        ui,
        p(OperationType::Bore, "pitch", "Pitch"),
        &mut cfg.pitch,
        " mm",
        0.1,
        fields,
    );
    check_in_row(
        ui,
        plain_row(
            "Climb",
            Some("Climb milling: the cutter edge meets the material in the direction of the feed."),
        ),
        &mut cfg.climb,
    );
    dv(
        ui,
        p(OperationType::Bore, "finish_allowance", "Finish allowance"),
        &mut cfg.finish_allowance,
        " mm",
        0.05,
        fields,
    );
    let bored = holes.map_or(0, |h| h.holes.len());
    // P6.7 (IC1): one row, the count first; the hover gives the scope.
    // The hole size rows of the Selection section give each size, its
    // fit and its helix radius.
    ui.add(OneLine::new(format!("{bored} circle(s) bored")).detail(BORE_SCOPE_NOTE));
}
