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
//! `draw_machine_envelope` (the egui_plot walls of the project scatter)
//! lived here while Chart C and the scatter both drew it. P17b moved the
//! nomogram onto the chart kit; the layer has one caller and lives in
//! `ui/readiness_panel.rs`. No `egui_plot` is left in this file.
//!
//! **One item is below the rule and is recorded rather than moved.** After
//! the rollup left, [`chart`] has a single caller, [`super::explore`]. It is
//! the palette for the chart family, which is exactly what `explore` holds,
//! so it belongs there. It stays here for now because moving it buys no
//! correctness and this package is already large. Move it when the file is
//! next opened.

use rs_cam_core::feeds::{
    FeedsExplain, ToolGeometryHint, vendor_lut::HardnessKind, vendor_lut::MaterialFamily,
    vendor_lut::ToolFamily,
};

use crate::state::AppState;

// The chart label kit of the nomogram lived here as a re-export of
// `explore.rs` (P5b, P5c round 2). P17 moved the kit to
// `ui/components/chart/labels.rs`; `explore.rs` keeps the names through its
// own `pub(crate) use`, and both charts draw through
// `components::chart::PlaneChart` (P17b).

/// The chart palette, named by what each series IS.
///
/// These charts draw RANGES and ORDERED VALUES, never verdicts, so every one
/// of them reads a data scale rather than a semantic role (`DESIGN_SPEC.md`
/// §2.6). The machine envelope is the one exception and takes `DANGER`
/// directly, because a wall the machine cannot pass IS a verdict.
pub(crate) mod chart {
    use crate::ui::tokens;

    /// The advance-per-tooth series. The suggested value is ONE solid line
    /// in this colour. The vendor limits are two fainter lines in the same
    /// colour, and the chart draws them only when the row publishes both.
    pub const BAND: egui::Color32 = tokens::CHART_SERIES[1];
    // RPM_RANGE went with the vendor RPM window mark on 2026-10-31 (W3k):
    // the inline chart drew it unlabelled under the spindle minimum band.
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
        .session()
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map(|tc| tc.name.clone())
}

pub(crate) fn combined_scale(row: &rs_cam_core::feeds::vendor_lookup::MatchedRow) -> f64 {
    row.chipload_diameter_scale * row.chipload_hardness_scale
}

/// Core's [`ToolFamily::display_label`] (F074: the one table).
pub(crate) const fn tool_family_label(f: ToolFamily) -> &'static str {
    f.display_label()
}

/// Core's [`MaterialFamily::display_label`] (F074: the one table).
pub(crate) const fn material_family_label(m: MaterialFamily) -> &'static str {
    m.display_label()
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

/// One operation's feeds inputs, as the Feeds tab holds them (W3i).
///
/// The tab draws the chart and the recommendation card from the panel's
/// scratch entry and inputs, not from `AppState`. `preview` and `card` are
/// the memo's answers (`FeedsMemo::preview`, `FeedsMemo::card`, F216), so
/// neither the chart nor the card runs the calculator or the funnel on a
/// frame where no input moved.
pub(crate) struct OpFeeds<'a> {
    pub(crate) toolpath_id: rs_cam_core::ToolpathId,
    pub(crate) operation: &'a crate::state::toolpath::OperationConfig,
    pub(crate) tool: &'a crate::state::job::ToolConfig,
    pub(crate) material: &'a rs_cam_core::material::Material,
    pub(crate) machine: &'a rs_cam_core::machine::MachineProfile,
    /// P20e: the op's feed cap (`DressupConfig::feed_max_rate`). The power
    /// row grades the steady feed under it.
    pub(crate) feed_max_rate: Option<f64>,
    pub(crate) preview: &'a rs_cam_core::feeds::suggest::FeedsPreview,
    /// The card's funnel answers, memoised with the preview (`FeedsMemo::card`).
    pub(crate) card: &'a crate::state::runtime::FeedsCard,
}

/// Live current values pulled from the toolpath's `OperationConfig`.
/// All values are denormalised so the card can render without
/// dispatching on operation variant.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CurrentValues {
    /// S11: `None` when the op has no feed.
    pub(crate) feed_rate_mm_min: Option<f64>,
    /// S11: `None` when the op has no plunge.
    pub(crate) plunge_rate_mm_min: Option<f64>,
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
        // S11: an unset feed is a missing input.
        let Some(feed) = self.feed_rate_mm_min else {
            return 0.0;
        };
        feed / (rpm * f64::from(self.flute_count))
    }
}

/// What `⚡ Apply all` writes on each row of the comparison card.
///
/// G-RECOMAPPLIED (2026-09-24). The card's recommended column printed the
/// raw calculator DOC and WOC. `⚡ Apply all` wrote the values after the
/// `enforce_invariants` chain (deleted in feeds contract P5). On a Ø6
/// hardwood pocket the column
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
}

/// Every field the funnel can write, in the order a hover names them.
const WRITTEN_FIELDS: [rs_cam_core::feeds::FeedsField; 5] = [
    rs_cam_core::feeds::FeedsField::FeedRate,
    rs_cam_core::feeds::FeedsField::PlungeRate,
    rs_cam_core::feeds::FeedsField::SpindleRpm,
    rs_cam_core::feeds::FeedsField::Stepover,
    rs_cam_core::feeds::FeedsField::DepthPerPass,
];

impl AppliedRecipe {
    /// Build the record from the funnel's dry run. `previews` is `None` when
    /// the pairing is refused: Apply then writes nothing, and the card shows
    /// the calculator values.
    pub(crate) fn new(previews: Option<rs_cam_core::feeds::suggest::FieldApplyPreviews>) -> Self {
        Self { previews }
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

    /// The operation and the operating point `⚡ Apply all` writes.
    ///
    /// The chipload verdict and the power row describe the cut the operator
    /// will make, so they read this pair, the same as the rail rows. The
    /// operation carries every field the funnel writes; the result carries
    /// the applied feed, RPM, plunge, depth and width, and the MRR of that
    /// geometry. On a refused pairing nothing is written, and the pair is
    /// the operation and the calculator result.
    pub(crate) fn applied_cut(
        &self,
        operation: &rs_cam_core::compute::catalog::OperationConfig,
        recommended: &rs_cam_core::feeds::FeedsResult,
    ) -> (
        rs_cam_core::compute::catalog::OperationConfig,
        rs_cam_core::feeds::FeedsResult,
    ) {
        use rs_cam_core::feeds::FeedsField;
        let mut cut = operation.clone();
        if let Some(previews) = &self.previews {
            let mut scratch = rs_cam_core::feeds::FeedsProvenance::default();
            for field in WRITTEN_FIELDS {
                if let Some(preview) = previews.get(field) {
                    preview.write_to(&mut cut, &mut scratch);
                }
            }
        }
        let mut point = recommended.clone();
        point.feed_rate_mm_min = self.value(FeedsField::FeedRate, recommended.feed_rate_mm_min);
        point.rpm = self.value(FeedsField::SpindleRpm, recommended.rpm);
        point.plunge_rate_mm_min =
            self.value(FeedsField::PlungeRate, recommended.plunge_rate_mm_min);
        point.axial_depth_mm = self.value(FeedsField::DepthPerPass, recommended.axial_depth_mm);
        point.radial_width_mm = self.value(FeedsField::Stepover, recommended.radial_width_mm);
        // P16 (item 2b): a surface offset keeps its unmodeled reason.
        point.mrr_mm3_min = recommended
            .mrr_mm3_min
            .map(|_| point.axial_depth_mm * point.radial_width_mm * point.feed_rate_mm_min);
        (cut, point)
    }

    /// The calculator's own cut: the operation with the raw calculator value
    /// in every field the funnel writes. The row hovers quote this cut beside
    /// the applied one.
    pub(crate) fn calculator_cut(
        &self,
        operation: &rs_cam_core::compute::catalog::OperationConfig,
        recommended: &rs_cam_core::feeds::FeedsResult,
    ) -> rs_cam_core::compute::catalog::OperationConfig {
        use rs_cam_core::feeds::FeedsField;
        let mut cut = operation.clone();
        for field in WRITTEN_FIELDS {
            if self.written(field).is_none() {
                continue;
            }
            match field {
                FeedsField::FeedRate => cut.set_feed_rate(recommended.feed_rate_mm_min),
                FeedsField::PlungeRate => cut.set_plunge_rate(recommended.plunge_rate_mm_min),
                // The funnel writes the RPM as a whole number, and so does
                // this. `rpm` is a finite spindle speed inside the machine
                // range, so the cast cannot wrap.
                FeedsField::SpindleRpm => {
                    cut.set_spindle_rpm(Some(recommended.rpm.round().max(0.0) as u32));
                }
                FeedsField::Stepover => {
                    cut.set_stepover(recommended.radial_width_mm);
                }
                FeedsField::DepthPerPass => {
                    cut.set_depth_per_pass(recommended.axial_depth_mm);
                }
                // No row and no pill: the `RampFeed` record is its line.
                FeedsField::ScallopHeight
                | FeedsField::RampFeedRate
                | FeedsField::PeckDepth
                | FeedsField::ZStep => {}
            }
        }
        cut
    }

    /// Why the applied cut differs from the calculator's cut.
    pub(crate) fn cut_cause(&self) -> String {
        "the invariant funnel changes the cut".to_owned()
    }
}

/// Ball-tip radius (mm) used for scallop/cusp geometry, or `None` for
/// tools without a spherical tip (the scallop override has no effect on
/// those — the cusp curve is undefined).
pub(crate) fn ball_tip_radius(explain: &FeedsExplain) -> Option<f64> {
    match explain.tool_geometry {
        ToolGeometryHint::Ball => Some(explain.tool_diameter_mm / 2.0),
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some(tip_radius),
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. } => {
            None
        }
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
/// maximum. A point row gives its one point. The rule is
/// `FeedsExplain::published_chipload_point` in core (A2).
pub(crate) fn vendor_single_value(explain: &FeedsExplain) -> Option<f64> {
    // A2: the core classifies the row (`LookupResult::printed_chipload`);
    // a point is never a band, so this reads the point alone.
    explain.published_chipload_point()
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
        ToolGeometryHint::Flat | ToolGeometryHint::Ball | ToolGeometryHint::Bull { .. } => {
            return None;
        }
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
        explain.max_cut_diameter_mm,
    );
    Some((doc, engaged, tip_dia, kind))
}

pub(crate) fn speedup(current: &CurrentValues, explain: &FeedsExplain) -> f64 {
    // S11: an unset feed has no speed-up to report.
    let Some(feed) = current.feed_rate_mm_min.filter(|feed| *feed > 0.0) else {
        return 1.0;
    };
    explain.recommended.feed_rate_mm_min / feed
}
