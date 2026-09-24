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

use egui_plot::{Line, PlotPoints};
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

    /// The advance-per-tooth series. The suggested value is ONE solid line
    /// in this colour. The vendor limits are two fainter lines in the same
    /// colour, and the chart draws them only when the row publishes both.
    pub const BAND: egui::Color32 = tokens::CHART_SERIES[1];
    /// The vendor RPM window, marked on the RPM axis.
    pub const RPM_RANGE: egui::Color32 = tokens::CHART_SERIES[3];
    /// The drag-to-explore proposal.
    pub const EXPLORE: egui::Color32 = tokens::CHART_SERIES[2];
    // ADMITTED, ISO_MIN, ISO_MID and ISO_MAX were deleted on 2026-09-16 with
    // the series they coloured: three iso-advance lines that restated the
    // band's own edges and midpoint, and a ±5 % ribbon pair. Four colours
    // for one fact. The shaded wedge went on 2026-09-23 (ruling G-CHARTLINES):
    // the chart draws lines, not shading.
}

/// A token at a given alpha, for a fainter chart line.
///
/// Every translucent colour in these charts is one of these, so the hue stays
/// a token and only the alpha is written at the call site. The charts draw no
/// filled region (ruling G-CHARTLINES, 2026-09-23).
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

/// What `⚡ Apply all` writes on each row of the comparison card.
///
/// G-RECOMAPPLIED (2026-09-24). The card's recommended column printed the
/// raw calculator DOC and WOC. `⚡ Apply all` writes the values after
/// `enforce_invariants`, and the aggressiveness dial (Suggest pass 6b)
/// scales them by one common factor. On a Ø6 hardwood pocket the column
/// read "DOC 0.81 mm → 4.20 mm" while the apply wrote 0.81 mm again.
///
/// This record holds the funnel's own dry run,
/// `feeds::suggest::preview_field_applies`, which is the call the ⚡ pills
/// make (G-PILLCLAMP). The card prints its value for each field it writes.
/// A field the funnel does not write, and every field on a refused pairing,
/// falls back to the raw calculator value.
#[derive(Debug, Clone, Default)]
pub(crate) struct AppliedRecipe {
    previews: Option<rs_cam_core::feeds::suggest::FieldApplyPreviews>,
    dial: Option<DialRecord>,
}

/// `true` when the dial evaluated a lever and moved it. A lever the dial
/// left at its base value did not set the applied value; another clamp did.
fn changed(from: Option<f64>, to: Option<f64>) -> bool {
    from.zip(to).is_some_and(|(f, t)| (f - t).abs() > 1e-9)
}

/// The part of the aggressiveness record the row hovers quote.
#[derive(Debug, Clone, Copy)]
struct DialRecord {
    /// `aggressiveness × long-tool share`, the load fraction of the base.
    target_share: f64,
    /// The dial moved the depth per pass.
    moved_depth: bool,
    /// The dial moved the stepover.
    moved_stepover: bool,
}

impl AppliedRecipe {
    /// Build the record from the funnel's dry run and the Suggest pass's
    /// warnings. `previews` is `None` when the pairing is refused: Apply then
    /// writes nothing, and the card shows the calculator values.
    pub(crate) fn new(
        previews: Option<rs_cam_core::feeds::suggest::FieldApplyPreviews>,
        warnings: Option<&[rs_cam_core::feeds::suggest::SuggestWarning]>,
    ) -> Self {
        let dial = warnings.unwrap_or_default().iter().find_map(|w| match w {
            rs_cam_core::feeds::suggest::SuggestWarning::EngagementReducedForAggressiveness {
                target_share,
                dpp_from,
                dpp_to,
                stepover_from,
                stepover_to,
                applied: true,
                ..
            } => Some(DialRecord {
                target_share: *target_share,
                moved_depth: changed(*dpp_from, *dpp_to),
                moved_stepover: changed(*stepover_from, *stepover_to),
            }),
            _ => None,
        });
        Self { previews, dial }
    }

    /// The value Apply writes for `field`, or `raw` when the funnel does not
    /// write that field.
    pub(crate) fn value(&self, field: rs_cam_core::feeds::FeedsField, raw: f64) -> f64 {
        self.written(field).unwrap_or(raw)
    }

    /// The value Apply writes for `field`, or `None` when the funnel does not
    /// write that field.
    pub(crate) fn written(&self, field: rs_cam_core::feeds::FeedsField) -> Option<f64> {
        self.previews.as_ref()?.get(field).map(|p| p.value)
    }

    /// The dial's load target in percent, when the dial set `field`.
    pub(crate) fn dial_load_pct(&self, field: rs_cam_core::feeds::FeedsField) -> Option<f64> {
        use rs_cam_core::feeds::FeedsField;
        let dial = self.dial?;
        let moved = match field {
            FeedsField::DepthPerPass => dial.moved_depth,
            FeedsField::Stepover => dial.moved_stepover,
            _ => false,
        };
        moved.then_some(dial.target_share * 100.0)
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

/// The vendor chipload band `(min, max)` that the matched row PUBLISHES.
///
/// `None` when no row matched, when the row publishes a maximum only, and
/// when the row publishes one point (`min == max`). The rule is
/// `FeedsExplain::published_chipload_band` in core.
///
/// Until 2026-09-23 this made a band from a maximum alone, as `0.7 × max`
/// to `max`. That lower limit was invented: the row did not publish it, and
/// the chart drew it as if it had. Operator ruling (G-CHARTLINES): draw band
/// lines "only if they exist".
pub(crate) fn vendor_band(explain: &FeedsExplain) -> Option<(f64, f64)> {
    explain.published_chipload_band()
}

/// The one chipload value (mm/tooth) that a matched row publishes when it
/// publishes no band.
///
/// `None` when the row publishes a band (use [`vendor_band`]), when no row
/// matched, or when the row publishes no chipload. A max-only row gives its
/// maximum. A point row gives its one point.
pub(crate) fn vendor_single_value(explain: &FeedsExplain) -> Option<f64> {
    if vendor_band(explain).is_some() {
        return None;
    }
    let row = explain.matched_row.as_ref()?;
    row.chip_load_max_mm
        .or(row.chip_load_min_mm)
        .filter(|value| value.is_finite() && *value > 0.0)
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
/// project scatter: solid red walls at max RPM and max feed, and axis marks
/// for the parts of each axis past the caps and below the spindle minimum.
///
/// The envelope draws lines only. It drew three shaded regions until
/// 2026-09-23; the operator ruled "no shading" for these charts
/// (G-CHARTLINES). The wall and the axis mark already say where each limit
/// is.
pub(crate) fn draw_machine_envelope(
    plot_ui: &mut egui_plot::PlotUi,
    env: &rs_cam_core::feeds::MachineEnvelope,
    axis_rpm_max: f64,
    axis_feed_max: f64,
) {
    // These two ARE verdicts — beyond them the machine cannot go — so the
    // envelope keeps the danger hue that every category in these charts just
    // gave up. It used to be a hand-mixed red one step off `DANGER`.
    let forbidden_edge = wash(tokens::DANGER, 200);

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
