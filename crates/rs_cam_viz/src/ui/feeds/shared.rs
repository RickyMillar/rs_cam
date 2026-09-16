//! What more than one feeds job needs.
//!
//! DC5a split `ui/feeds_modal.rs` into one file per JOB. This file is the
//! residue: the palette, the unit and label helpers, the current-value
//! record every job reads, and the one plot primitive that two jobs draw.
//!
//! A function belongs here when two or more callers need it. Those callers
//! are [`super::compare`], [`super::why`], [`super::explore`] and — since
//! the project rollup left for the Readiness workspace —
//! `ui/readiness_panel.rs`. A function that only ONE of them calls belongs
//! in that caller's file.
//!
//! C29 added a fifth caller: the vendor LUT viewer in `ui/properties/mod.rs`
//! reads [`material_family_label`] and [`tool_family_label`] here. It carried
//! its own copies of both tables, and the two spellings had drifted apart.
//!
//! [`draw_machine_envelope`] is the one item DC5a's plan placed with the
//! project rollup and the call graph placed here. Chart C draws it and the
//! project scatter draws it, so it is shared geometry and not a
//! project-scope function.
//!
//! **One item is below the rule and is recorded rather than moved.** After
//! the rollup left, [`chart`] has a single caller, [`super::explore`]. It is
//! the palette for the chart family, which is exactly what `explore` holds,
//! so it belongs there. It stays here for now because moving it buys no
//! correctness and this package is already large. Move it when the file is
//! next opened.

use egui_plot::{Line, PlotPoints, Polygon};
use rs_cam_core::feeds::{
    FeedsExplain, ToolGeometryHint, vendor_lut::HardnessKind, vendor_lut::MaterialFamily,
    vendor_lut::ToolFamily,
};

use crate::state::AppState;
use crate::ui::tokens;

/// The chart palette, named by what each series IS.
///
/// These charts draw RANGES and ORDERED VALUES, never verdicts, so every one
/// of them reads a data scale rather than a semantic role (`DESIGN_SPEC.md`
/// §2.6). The machine envelope is the one exception and takes `DANGER`
/// directly, because a wall the machine cannot pass IS a verdict.
// SAFETY: every index below is a literal inside its own array's length.
#[allow(clippy::indexing_slicing)]
pub(crate) mod chart {
    use crate::ui::tokens;

    /// The vendor advance-per-tooth range.
    pub const BAND: egui::Color32 = tokens::CHART_SERIES[1];
    /// The vendor RPM window, marked on the RPM axis.
    pub const RPM_RANGE: egui::Color32 = tokens::CHART_SERIES[3];
    /// The drag-to-explore proposal.
    pub const EXPLORE: egui::Color32 = tokens::CHART_SERIES[2];
    // ADMITTED, ISO_MIN, ISO_MID and ISO_MAX were deleted on 2026-09-16 with
    // the series they coloured: three iso-advance lines that restated the
    // band's own edges and midpoint, and a ±5 % ribbon pair. Four colours
    // for one fact the wedge already draws.
}

/// A token at a given alpha, for a chart wash.
///
/// Every translucent fill in these charts is one of these, so the hue stays a
/// token and only the alpha is written at the call site.
pub(crate) fn wash(base: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
}

pub(crate) fn toolpath_name(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Option<String> {
    state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map(|tc| tc.name.clone())
}

pub(crate) fn combined_scale(row: &rs_cam_core::feeds::vendor_lookup::MatchedRow) -> f64 {
    row.chipload_diameter_scale * row.chipload_hardness_scale
}

pub(crate) fn tool_family_label(f: ToolFamily) -> &'static str {
    match f {
        ToolFamily::FlatEnd => "Flat end",
        ToolFamily::BallNose => "Ball nose",
        ToolFamily::BullNose => "Bull nose",
        ToolFamily::ChamferVbit => "V-bit",
        ToolFamily::TaperedBallNose => "Tapered ball",
        ToolFamily::FacingBit => "Facing bit",
    }
}

pub(crate) fn material_family_label(m: MaterialFamily) -> &'static str {
    match m {
        MaterialFamily::Softwood => "Softwood",
        MaterialFamily::Hardwood => "Hardwood",
        MaterialFamily::PlywoodSoftwood => "Plywood (soft)",
        MaterialFamily::PlywoodHardwood => "Plywood (hard)",
        MaterialFamily::Mdf => "MDF",
        MaterialFamily::Hdf => "HDF",
        MaterialFamily::Particleboard => "Particle board",
        MaterialFamily::Acrylic => "Acrylic",
        MaterialFamily::Hdpe => "HDPE",
        MaterialFamily::Delrin => "Delrin",
        MaterialFamily::Polycarbonate => "Polycarbonate",
        MaterialFamily::Aluminum => "Aluminum",
        MaterialFamily::Fiberglass => "Fiberglass",
    }
}

pub(crate) fn hardness_label(kind: Option<HardnessKind>, value: Option<f64>) -> String {
    match (kind, value) {
        (Some(HardnessKind::Janka), Some(v)) => format!("Janka {v:.0}"),
        (Some(HardnessKind::ShoreD), Some(v)) => format!("Shore D {v:.0}"),
        (Some(HardnessKind::Hb), Some(v)) => format!("HB {v:.0}"),
        _ => "—".to_owned(),
    }
}

// ── Comparison card ─────────────────────────────────────────────────

/// Live current values pulled from the toolpath's `OperationConfig`.
/// All values are denormalised so the card can render without
/// dispatching on operation variant.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CurrentValues {
    pub(crate) feed_rate_mm_min: f64,
    pub(crate) plunge_rate_mm_min: f64,
    pub(crate) spindle_rpm: Option<u32>,
    pub(crate) depth_per_pass: Option<f64>,
    pub(crate) stepover: Option<f64>,
    pub(crate) flute_count: u32,
    /// Active scallop target (mm). `Some` means stepover is derived from
    /// the cusp geometry, not a manual value. Only DropCutter exposes
    /// this as an *optional* override (see S1).
    pub(crate) scallop_height: Option<f64>,
    /// True when this operation supports the scallop-driven-stepover
    /// override (currently DropCutter / 3D Finish). Gates the
    /// "Scallop height" input and the derived-stepover rendering.
    pub(crate) supports_scallop_override: bool,
    /// Pass role of this operation — drives the finish-only chipload-min
    /// warning (S3).
    pub(crate) pass_role: rs_cam_core::feeds::PassRole,
}

impl CurrentValues {
    /// Compute the resulting chipload from current settings:
    /// `feed / (rpm * flutes)`. Returns 0.0 when any input is missing
    /// or zero — the UI degrades gracefully.
    pub(crate) fn chipload_mm(&self) -> f64 {
        let rpm = match self.spindle_rpm {
            Some(r) if r > 0 => f64::from(r),
            _ => return 0.0,
        };
        if self.flute_count == 0 {
            return 0.0;
        }
        self.feed_rate_mm_min / (rpm * f64::from(self.flute_count))
    }
}

/// Ball-tip radius (mm) used for scallop/cusp geometry, or `None` for
/// tools without a spherical tip (the scallop override has no effect on
/// those — the cusp curve is undefined).
pub(crate) fn ball_tip_radius(explain: &FeedsExplain) -> Option<f64> {
    match explain.tool_geometry {
        ToolGeometryHint::Ball => Some(explain.tool_diameter_mm / 2.0),
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some(tip_radius),
        _ => None,
    }
}

/// Vendor chipload band `(min, max)` for the matched row, with the same
/// `None`-max fallback the charts use. `None` when no row matched.
pub(crate) fn vendor_band(explain: &FeedsExplain) -> Option<(f64, f64)> {
    explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    })
}

/// For tapered-ball / V-bit tools, describe the engaged cutting diameter
/// at the operation's depth of cut: `(doc_mm, engaged_dia_mm,
/// tip_dia_mm, tip_label)`. `None` for flat / ball / bull tools, whose
/// engaged diameter equals the nominal diameter regardless of DOC (no
/// cone shoulder), so the annotation would be redundant.
pub(crate) fn engaged_diameter_context(
    current: &CurrentValues,
    explain: &FeedsExplain,
) -> Option<(f64, f64, f64, &'static str)> {
    let (tip_dia, kind) = match explain.tool_geometry {
        ToolGeometryHint::TaperedBall { tip_radius, .. } => (tip_radius * 2.0, "tip"),
        ToolGeometryHint::VBit { tip_diameter, .. } => (tip_diameter, "tip"),
        _ => return None,
    };
    // Use the operator's per-pass DOC when set, else the DOC the
    // recommendation itself was built on.
    let doc = current
        .depth_per_pass
        .filter(|d| *d > 0.0)
        .unwrap_or(explain.recommended.axial_depth_mm);
    if doc <= 0.0 {
        return None;
    }
    let engaged = explain.tool_geometry.engaged_diameter_at_doc(
        doc,
        explain.tool_diameter_mm,
        explain.shank_diameter_mm,
    );
    Some((doc, engaged, tip_dia, kind))
}

/// Render the shared machine-envelope overlay used by Chart C and the
/// project scatter: shaded forbidden zones beyond max RPM and max feed
/// (plus the left-side dim zone below spindle min RPM when non-zero),
/// solid red borders on the cap walls, and value labels.
pub(crate) fn draw_machine_envelope(
    plot_ui: &mut egui_plot::PlotUi,
    env: &rs_cam_core::feeds::MachineEnvelope,
    axis_rpm_max: f64,
    axis_feed_max: f64,
) {
    // These two ARE verdicts — beyond them the machine cannot go — so the
    // envelope keeps the danger hue that every category in these charts just
    // gave up. It used to be a hand-mixed red one step off `DANGER`.
    let forbidden_fill = wash(tokens::DANGER, 35);
    let forbidden_edge = wash(tokens::DANGER, 200);

    if axis_rpm_max > env.spindle_max_rpm {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [env.spindle_max_rpm, 0.0],
                    [axis_rpm_max, 0.0],
                    [axis_rpm_max, axis_feed_max],
                    [env.spindle_max_rpm, axis_feed_max],
                ]),
            )
            .fill_color(forbidden_fill)
            .stroke(egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT))
            .name(format!(
                "Past machine cap ({} RPM)",
                env.spindle_max_rpm as i64
            )),
        );
    }
    if axis_feed_max > env.max_feed_mm_min {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [0.0, env.max_feed_mm_min],
                    [axis_rpm_max, env.max_feed_mm_min],
                    [axis_rpm_max, axis_feed_max],
                    [0.0, axis_feed_max],
                ]),
            )
            .fill_color(forbidden_fill)
            .stroke(egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT))
            .name(format!(
                "Past machine feed ({} mm/min)",
                env.max_feed_mm_min as i64
            )),
        );
    }
    // Below the spindle minimum is the SOFT half of the same envelope
    // verdict the wall above carries, so it reads as caution rather than as
    // the inert grey it used to mix, which said nothing at all.
    if env.spindle_min_rpm > 0.0 {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [0.0, 0.0],
                    [env.spindle_min_rpm, 0.0],
                    [env.spindle_min_rpm, axis_feed_max],
                    [0.0, axis_feed_max],
                ]),
            )
            .fill_color(wash(tokens::CAUTION, 25))
            .stroke(egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT))
            .name(format!(
                "Below spindle min ({} RPM)",
                env.spindle_min_rpm as i64
            )),
        );
        // The shaded region and the axis run below already say where the
        // spindle minimum is. A third full-height rule in the caution colour
        // made the left third of the chart the heaviest thing on it.
    }
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![
                [env.spindle_max_rpm, 0.0],
                [env.spindle_max_rpm, axis_feed_max],
            ]),
        )
        .color(forbidden_edge)
        .width(2.0_f32)
        .name(format!("machine max {} RPM", env.spindle_max_rpm as i64)),
    );
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![
                [0.0, env.max_feed_mm_min],
                [axis_rpm_max, env.max_feed_mm_min],
            ]),
        )
        .color(forbidden_edge)
        .width(2.0_f32)
        .name(format!(
            "machine max feed {} mm/min",
            env.max_feed_mm_min as i64
        )),
    );
    // W6: the RPM wall is labelled at the FOOT of the wall, not the head.
    //
    // Both wall labels used to sit near the top-right: this one at
    // `axis_feed_max * 0.97` and the feed label at the feed wall, which is
    // `axis_feed_max / 1.05`. Those two heights are about 5 % of the axis
    // apart, and both labels are right-anchored, so on any machine whose
    // caps are both in frame they overlapped — `max 24000 RPM` printed
    // through `max 4000 mm/min`. The walls cross at the top-right corner;
    // only one label can live there.
    // The limits are marked ON the axes they belong to, not lettered into
    // the plot body.
    //
    // They used to be two labelled hexagons floating inside the drawing.
    // They collided with each other and with the band, one of them clipped
    // at the plot edge, and a label inside a chart competes with the data
    // for attention it does not deserve — the wall already says where the
    // limit is; only the NUMBER was ever in the label, and the legend row
    // carries that. Operator, 2026-09-16: "the max and min lines should be
    // on the axis, not marked on the chart as points".
    //
    // A segment along y = 0 from the cap to the axis end says "this part of
    // the RPM axis is out of bounds" with no words at all.
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![[env.spindle_max_rpm, 0.0], [axis_rpm_max, 0.0]]),
        )
        .color(forbidden_edge)
        .width(crate::ui::feeds::explore::AXIS_MARK_WIDTH)
        .name(format!(
            "Past machine cap ({} RPM)",
            env.spindle_max_rpm as i64
        )),
    );
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![[0.0, env.max_feed_mm_min], [0.0, axis_feed_max]]),
        )
        .color(forbidden_edge)
        .width(crate::ui::feeds::explore::AXIS_MARK_WIDTH)
        .name(format!(
            "Past machine feed ({} mm/min)",
            env.max_feed_mm_min as i64
        )),
    );
    if env.spindle_min_rpm > 0.0 {
        plot_ui.line(
            Line::new(
                "",
                PlotPoints::from(vec![[0.0, 0.0], [env.spindle_min_rpm, 0.0]]),
            )
            .color(tokens::CAUTION)
            .width(crate::ui::feeds::explore::AXIS_MARK_WIDTH)
            .name(format!(
                "Below spindle min ({} RPM)",
                env.spindle_min_rpm as i64
            )),
        );
    }
}

pub(crate) fn speedup(current: &CurrentValues, explain: &FeedsExplain) -> f64 {
    if current.feed_rate_mm_min <= 0.0 {
        return 1.0;
    }
    explain.recommended.feed_rate_mm_min / current.feed_rate_mm_min
}
