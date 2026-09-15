//! `PillSuggestions` — what the per-field ⚡ pills on the Geometry tab offer.
//!
//! G-PILLCLAMP (UX-R03-014, 2026-09-10). The 22 `dv_pill` sites in
//! `operations/*.rs` used to be handed `(r.axial_depth_mm, &r.chipload_source)`
//! straight off the `FeedsResult` — the raw calculator output — while the
//! canonical `⚡ Apply all` action runs the same recommendation through the
//! invariant funnel and writes the clamped value. On
//! the demo pocket that was 4.2 mm versus 1.2 mm of DOC, both labelled as the
//! recommendation.
//!
//! This struct runs the funnel ONCE per frame (a dry run on a scratch clone,
//! `rs_cam_core::feeds::suggest::preview_field_applies`) and hands each pill
//! the as-applied value for its field, with the stamp the funnel would record.
//! A field the funnel does not write (VCarve `max_depth`: the op reports no
//! `depth_per_pass`) falls back to the raw calculator value with
//! `clamped: false`, and the pill's hover says so.
//!
//! The click is recorded here rather than returned, because the `draw_*_params`
//! functions own only the inner config struct and cannot reach the entry's
//! `feeds_provenance`. `properties/mod.rs` reads `take_clicked()` after the
//! per-op draw and stamps the recommendation's provenance on the entry.

use std::cell::Cell;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::feeds::suggest::{
    FieldApplyPreview, FieldApplyPreviews, SuggestContext, preview_field_applies,
    round_suggestion_value,
};
use rs_cam_core::feeds::{FeedsField, FeedsResult};

use crate::ui::components::{ProvKind, Suggestion};

/// Build the [`Suggestion`] a pill shows for `field`.
///
/// With a funnel preview the pill offers the as-applied value with the stamp
/// the funnel would record. Without one the funnel does not write this field
/// on this operation, so the pill offers the raw calculator value
/// (`calculator`, rounded to `step`) coloured by the chipload source, marked
/// `clamped: false` so the hover says so.
pub(super) fn suggestion_for<'a>(
    preview: Option<&'a FieldApplyPreview>,
    calculator: f64,
    step: f64,
    raw_source: (ProvKind, Option<&'a str>),
) -> Suggestion<'a> {
    match preview {
        Some(p) => Suggestion {
            recommended: p.value,
            source: ProvKind::from(&p.provenance),
            reference: p.provenance.reference.as_deref(),
            clamped: true,
            calculator: Some(calculator),
        },
        None => Suggestion {
            recommended: round_suggestion_value(calculator, step),
            source: raw_source.0,
            reference: raw_source.1,
            clamped: false,
            calculator: Some(calculator),
        },
    }
}

/// The per-field suggestions for one operation, for one frame.
pub(super) struct PillSuggestions<'a> {
    result: &'a FeedsResult,
    previews: FieldApplyPreviews,
    clicked: Cell<Option<FeedsField>>,
}

/// One pill's suggestion plus the hook that records its click.
pub(super) struct PillSuggestion<'a> {
    pub suggestion: Suggestion<'a>,
    /// `Some` when the funnel writes this field (so a click must be stamped);
    /// `None` for a raw-calculator fallback.
    pub field: Option<FeedsField>,
    clicked: &'a Cell<Option<FeedsField>>,
}

/// The click hook of a [`PillSuggestion`] once its [`Suggestion`] has moved
/// into a `ValueRow`.
pub(super) struct PillClick<'a> {
    field: Option<FeedsField>,
    clicked: &'a Cell<Option<FeedsField>>,
}

impl PillClick<'_> {
    /// Called by `dv_pill` when the ⚡ was clicked this frame.
    pub fn record(&self) {
        if let Some(field) = self.field {
            self.clicked.set(Some(field));
        }
    }
}

impl<'a> PillSuggestion<'a> {
    /// Split into the suggestion the row consumes and the click hook the
    /// caller keeps.
    pub fn into_parts(self) -> (Suggestion<'a>, PillClick<'a>) {
        (
            self.suggestion,
            PillClick {
                field: self.field,
                clicked: self.clicked,
            },
        )
    }

    /// Record a click without splitting (tests).
    #[cfg(test)]
    pub fn record_click(&self) {
        PillClick {
            field: self.field,
            clicked: self.clicked,
        }
        .record();
    }
}

impl<'a> PillSuggestions<'a> {
    /// Dry-run the apply funnel for `operation` against `result`.
    pub fn new(
        operation: &OperationConfig,
        result: &'a FeedsResult,
        tool: &rs_cam_core::compute::tool_config::ToolConfig,
        machine: &rs_cam_core::machine::MachineProfile,
        material: &rs_cam_core::material::Material,
    ) -> Self {
        let previews = preview_field_applies(
            operation,
            result,
            tool,
            machine,
            material,
            operation.feeds_style().1,
            SuggestContext::default(),
        );
        Self {
            result,
            previews,
            clicked: Cell::new(None),
        }
    }

    /// The funnel's preview for `field`, or the raw calculator value
    /// (`calculator`, rounded to `step`) labelled as not clamped when the
    /// funnel does not write this field on this operation.
    fn suggestion(&self, field: FeedsField, calculator: f64, step: f64) -> PillSuggestion<'_> {
        let preview = self.previews.get(field);
        PillSuggestion {
            suggestion: suggestion_for(
                preview,
                calculator,
                step,
                super::prov_from_chipload(&self.result.chipload_source),
            ),
            field: preview.map(|p| p.field),
            clicked: &self.clicked,
        }
    }

    /// Stepover / WOC pill.
    pub fn stepover(&self) -> PillSuggestion<'_> {
        self.suggestion(FeedsField::Stepover, self.result.radial_width_mm, 0.001)
    }

    /// Depth-per-pass / DOC pill (also VCarve `Max Depth`, which the funnel
    /// does not write — that one falls back to the raw value, labelled).
    pub fn depth_per_pass(&self) -> PillSuggestion<'_> {
        self.suggestion(FeedsField::DepthPerPass, self.result.axial_depth_mm, 0.001)
    }

    /// Feed-rate pill (drill ops edit their single feed on the Geometry tab).
    pub fn feed_rate(&self) -> PillSuggestion<'_> {
        self.suggestion(FeedsField::FeedRate, self.result.feed_rate_mm_min, 1.0)
    }

    /// The field whose pill was clicked this frame, with the preview whose
    /// value it wrote, so the caller can stamp the entry's provenance.
    pub fn take_clicked(&self) -> Option<(FeedsField, &FieldApplyPreview)> {
        let field = self.clicked.take()?;
        self.previews.get(field).map(|p| (field, p))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ui::components::value_row::near_match;
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::session::ProjectSession;

    /// The demo pocket of UX-R03-014: Ø6 two-flute flat end mill, fresh
    /// Pocket op, default stock / material / Generic Wood Router.
    fn demo_pocket() -> (ProjectSession, ToolConfig, OperationConfig, FeedsResult) {
        let session = ProjectSession::new_empty();
        let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
        tool.diameter = 6.0;
        let op = OperationConfig::new_default(OperationType::Pocket);
        let stock = session.stock_config();
        let result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            &op,
            &tool,
            &stock.material,
            session.machine(),
            stock.workholding_rigidity,
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_strategy,
        )
        .expect("valid pairing");
        (session, tool, op, result)
    }

    /// The pill offers the funnel's 1.2, not the calculator's 4.2, and with
    /// the configured value already at 1.2 the near-match test greys it.
    #[test]
    fn depth_per_pass_pill_offers_the_as_applied_value_and_greys_on_a_match() {
        let (session, tool, op, result) = demo_pocket();
        let pills = PillSuggestions::new(
            &op,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
        );
        let dpp = pills.depth_per_pass();
        assert!(dpp.suggestion.clamped);
        assert_eq!(dpp.field, Some(FeedsField::DepthPerPass));
        assert!(
            (dpp.suggestion.recommended - 1.2).abs() < 1e-9,
            "pill offers {}",
            dpp.suggestion.recommended
        );
        assert!(
            (result.axial_depth_mm - 4.2).abs() < 1e-9,
            "fixture drift: raw DOC {}",
            result.axial_depth_mm
        );
        // Configured 1.2 (what the add-time funnel wrote on the demo seed).
        assert!(
            near_match(1.2, dpp.suggestion.recommended),
            "pill must be greyed"
        );
        assert!(
            !near_match(1.2, result.axial_depth_mm),
            "pre-fix comparison stayed lit"
        );
    }

    /// A click on a funnel-backed pill is recorded and hands back the preview
    /// whose value the pill wrote.
    #[test]
    fn click_is_recorded_with_the_preview_it_wrote() {
        let (session, tool, op, result) = demo_pocket();
        let pills = PillSuggestions::new(
            &op,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
        );
        assert!(pills.take_clicked().is_none());
        let dpp = pills.depth_per_pass();
        let offered = dpp.suggestion.recommended;
        dpp.record_click();
        let (field, preview) = pills.take_clicked().expect("click recorded");
        assert_eq!(field, FeedsField::DepthPerPass);
        assert_eq!(preview.value.to_bits(), offered.to_bits());
        assert!(pills.take_clicked().is_none(), "take drains the click");
    }

    /// VCarve carries no `depth_per_pass`, so the funnel never writes its
    /// `max_depth`; the pill offers the raw value and says it is not clamped,
    /// and a click records nothing to stamp.
    #[test]
    fn vcarve_max_depth_falls_back_to_the_raw_value_labelled_not_clamped() {
        let session = ProjectSession::new_empty();
        let tool = ToolConfig::new_default(ToolId(1), ToolType::VBit);
        let op = OperationConfig::new_default(OperationType::VCarve);
        let stock = session.stock_config();
        let result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            &op,
            &tool,
            &stock.material,
            session.machine(),
            stock.workholding_rigidity,
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_strategy,
        )
        .expect("V-bit on VCarve is a valid pairing");
        let pills = PillSuggestions::new(&op, &result, &tool, session.machine(), &stock.material);
        let max_depth = pills.depth_per_pass();
        assert!(!max_depth.suggestion.clamped);
        assert_eq!(max_depth.field, None);
        assert_eq!(
            max_depth.suggestion.recommended,
            round_suggestion_value(result.axial_depth_mm, 0.001)
        );
        max_depth.record_click();
        assert!(
            pills.take_clicked().is_none(),
            "nothing to stamp on a raw fallback"
        );
    }
}
