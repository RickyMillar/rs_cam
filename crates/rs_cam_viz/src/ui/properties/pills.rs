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
    round_suggestion_value, round_suggestion_value_down,
};
use rs_cam_core::feeds::{FeedsField, FeedsResult};

use crate::ui::components::{ProvKind, Suggestion};

/// Build the [`Suggestion`] a pill shows for `field`.
///
/// With a funnel preview the pill offers the as-applied value with the stamp
/// the funnel would record. Without one the funnel does not write this field
/// on this operation, so the pill offers the raw calculator value
/// (`calculator`, quantised by `round` to `step`) coloured by the chipload
/// source, marked `clamped: false` so the hover says so.
///
/// `round` is the caller's choice of quantiser, because the direction is not
/// the same for every field. T-9: a value a ceiling already bound takes
/// `round_suggestion_value_down`, a value with no upper bound takes
/// `round_suggestion_value`.
pub(super) fn suggestion_for<'a>(
    preview: Option<&'a FieldApplyPreview>,
    calculator: f64,
    step: f64,
    round: fn(f64, f64) -> f64,
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
            recommended: round(calculator, step),
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
    pub(crate) fn record_click(&self) {
        PillClick {
            field: self.field,
            clicked: self.clicked,
        }
        .record();
    }
}

impl<'a> PillSuggestions<'a> {
    /// Dry-run the apply funnel for `operation` against `result`.
    ///
    /// Q1: `model_bbox` is the box of the model this toolpath machines,
    /// carried in by `ToolpathPanelSnapshot`. The funnel's
    /// runtime-sanity stepover back-off reads it; this site passed
    /// `SuggestContext::default()` before, so the back-off could not
    /// fire and the pill offered a value the controller and the MCP
    /// surfaces would have backed off. `None` when the toolpath names no
    /// model with finite geometry.
    pub fn new(
        operation: &OperationConfig,
        result: &'a FeedsResult,
        tool: &rs_cam_core::compute::tool_config::ToolConfig,
        machine: &rs_cam_core::machine::MachineProfile,
        material: &rs_cam_core::material::Material,
        model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    ) -> Self {
        let previews = preview_field_applies(
            operation,
            result,
            tool,
            machine,
            material,
            operation.feeds_style().1,
            // The stock is not carried here: this preview reads the
            // operation the panel already holds, and the wired Suggest
            // sites leave `SuggestContext::stock` empty too.
            SuggestContext {
                model_bbox,
                ..SuggestContext::default()
            },
        );
        Self {
            result,
            previews,
            clicked: Cell::new(None),
        }
    }

    /// The funnel's preview for `field`, or the raw calculator value
    /// (`calculator`, quantised by `round` to `step`) labelled as not clamped
    /// when the funnel does not write this field on this operation.
    fn suggestion(
        &self,
        field: FeedsField,
        calculator: f64,
        step: f64,
        round: fn(f64, f64) -> f64,
    ) -> PillSuggestion<'_> {
        let preview = self.previews.get(field);
        PillSuggestion {
            suggestion: suggestion_for(
                preview,
                calculator,
                step,
                round,
                super::prov_from_chipload(&self.result.chipload_source),
            ),
            field: preview.map(|p| p.field),
            clicked: &self.clicked,
        }
    }

    /// Stepover / WOC pill. Rounds to the NEAREST: the geometry clamps run
    /// below this point, so the value is not bound from above here.
    pub fn stepover(&self) -> PillSuggestion<'_> {
        self.suggestion(
            FeedsField::Stepover,
            self.result.radial_width_mm,
            0.001,
            round_suggestion_value,
        )
    }

    /// Depth-per-pass / DOC pill (also VCarve `Max Depth`, which the funnel
    /// does not write — that one falls back to the raw value, labelled).
    /// Rounds to the NEAREST, for the same reason as [`Self::stepover`].
    ///
    /// On a 3D Rough with a step ladder this is the BASE step, not
    /// `deepest_axial_step` (D7): the pill sits on the Depth/Pass field and
    /// writes that field only. The deepest step is on the feeds card.
    pub fn depth_per_pass(&self) -> PillSuggestion<'_> {
        self.suggestion(
            FeedsField::DepthPerPass,
            self.result.axial_depth_mm,
            0.001,
            round_suggestion_value,
        )
    }

    /// Feed-rate pill (drill ops edit their single feed on the Geometry tab).
    ///
    /// The feed rounds DOWN. `calculate` already applied the Step 6 power
    /// gate and the Step 7 machine ceiling, so the fallback branch below
    /// quantises a value that is bound from above; a nearest rounding would
    /// offer the operator a feed up to +0.5 mm/min past the ceiling. T-9.
    pub fn feed_rate(&self) -> PillSuggestion<'_> {
        self.suggestion(
            FeedsField::FeedRate,
            self.result.feed_rate_mm_min,
            1.0,
            round_suggestion_value_down,
        )
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
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::session::{Command, ProjectSession, SetMachineArgs};

    /// The depth per pass the funnel applies on the demo pocket (mm). See
    /// `depth_per_pass_pill_offers_the_as_applied_value_and_greys_on_a_match`.
    const DEMO_POCKET_APPLIED_DPP: f64 = 0.75;

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
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_strategy,
        )
        .expect("valid pairing");
        (session, tool, op, result)
    }

    /// The feed-floor fixture: the demo pocket in generic hardwood (Janka
    /// 1450), on a machine whose cutting-feed ceiling is not an integer.
    ///
    /// Feeds matrix R5 re-bless (2026-09-23). The demo pocket's default
    /// stock resolves to the printed row `amana-zrn-flat-softwood-pocket-6000-2f`
    /// (0.2032 mm/tooth). Its raw feed is 0.2032 x 18 000 x 2 = 7315 mm/min,
    /// which the 4000 mm/min cutting ceiling clamps to the integer 4000. A
    /// floor and a nearest rounding agree on an integer, so the demo pocket
    /// cannot tell them apart.
    ///
    /// In generic hardwood the pocket resolves to the printed Spektra row
    /// `amana-flat-hardwood-pocket-6000-2f-spektra`: 0.127 mm/tooth (one
    /// value), Ø6.0, Janka 1450, `rpm_nominal` 18 000. The row is
    /// VendorBacked, so R1 does not refuse it. The `feeds::calculate` stages
    /// (ruling R4 WP3, 2026-09-24):
    ///
    /// * chipload 0.127 mm/tooth: the diameter scale (6/6)^0.61 and the
    ///   hardness scale (1450/1450)^0.5 are both 1.0;
    /// * RPM 18 000, from the row, inside the 8000-24 000 machine range;
    /// * depth scale 1.0: the default pocket depth is 4.2 mm = 0.7 x D;
    /// * raw feed = 18 000 x 0.127 x 2 x 1.0 = 4572 mm/min;
    /// * the long-tool share (stickout 45 mm, 7.5 x D) is a load target, not
    ///   a feed factor (ruling Q7), and the 0.75 safety factor is gone;
    /// * no workholding factor (ruling R4 Q8 deleted it);
    /// * the cutting ceiling binds. This fixture's machine sets
    ///   `max_cutting_feed_mm_min` to 3999.9 mm/min;
    /// * the RPM follows the feed down to hold the chip (ruling R4 Q10): the
    ///   chip per rev is 4572 / 18 000 = 0.254 mm, the RPM target is
    ///   floor(3999.9 / 0.254) = 15 747, and the feed is 0.254 x 15 747 =
    ///   3999.738 mm/min;
    /// * the rubbing floor does not fire: the advance stays 0.127 mm/tooth,
    ///   above 0.025.
    ///
    /// 3999.738 has a fraction above 0.5, so a nearest rounding gives 4000
    /// and the floor gives 3999. A ceiling of 3999.5 no longer separates
    /// them: the RPM goes to 15 746 and the feed to 3999.484, which both
    /// roundings take to 3999.
    fn hardwood_pocket() -> (
        ProjectSession,
        ToolConfig,
        OperationConfig,
        FeedsResult,
        Material,
    ) {
        let mut session = ProjectSession::new_empty();
        let mut machine = session.machine().clone();
        machine.max_cutting_feed_mm_min = Some(3999.9);
        let _ = session
            .apply(Command::SetMachine(SetMachineArgs {
                machine: Box::new(machine),
            }))
            .expect("set the fixture machine");
        let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
        tool.diameter = 6.0;
        let op = OperationConfig::new_default(OperationType::Pocket);
        let material = Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        };
        let result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            &op,
            &tool,
            &material,
            session.machine(),
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_strategy,
        )
        .expect("valid pairing");
        (session, tool, op, result, material)
    }

    /// The pill offers the funnel's value, not the calculator's 4.2, and with
    /// the configured value already at that value the near-match test greys
    /// it.
    ///
    /// Ruling R4 WP3 (2026-09-24): the funnel now runs the aggressiveness
    /// dial (Suggest pass 6b) after the rigidity cap. The chain:
    ///
    /// * the rigidity cap gives 0.2 x 6 = 1.2 mm, the base depth;
    /// * the dial target is k x L/D share = 0.85 x 0.75 = 0.6375 (the default
    ///   tool's stickout is 45 mm, 7.5 x D);
    /// * the lateral force is about linear in the depth and weak in the
    ///   stepover (the edge term), so the common scale is about 0.64 to 0.70,
    ///   and the raw depth is about 0.77 to 0.84 mm;
    /// * the dial snaps the depth to a step the generator cuts: the pocket is
    ///   3.0 mm deep, so four passes of 0.75 mm.
    ///
    /// 0.75 is the editor's estimate; the orchestrator measures it. If the
    /// funnel's context carries no calculator operating point, the dial does
    /// not act and the pill stays at 1.2.
    #[test]
    fn depth_per_pass_pill_offers_the_as_applied_value_and_greys_on_a_match() {
        let (session, tool, op, result) = demo_pocket();
        let pills = PillSuggestions::new(
            &op,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
            None,
        );
        let dpp = pills.depth_per_pass();
        assert!(dpp.suggestion.clamped);
        assert_eq!(dpp.field, Some(FeedsField::DepthPerPass));
        assert!(
            (dpp.suggestion.recommended - DEMO_POCKET_APPLIED_DPP).abs() < 1e-9,
            "pill offers {}",
            dpp.suggestion.recommended
        );
        assert!(
            (result.axial_depth_mm - 4.2).abs() < 1e-9,
            "fixture drift: raw DOC {}",
            result.axial_depth_mm
        );
        // Configured at the as-applied value (what the add-time funnel
        // writes on the demo seed).
        assert!(
            near_match(DEMO_POCKET_APPLIED_DPP, dpp.suggestion.recommended),
            "pill must be greyed"
        );
        assert!(
            !near_match(DEMO_POCKET_APPLIED_DPP, result.axial_depth_mm),
            "pre-fix comparison stayed lit"
        );
    }

    /// The two axes quantise in OPPOSITE directions, and both are pinned here
    /// so an edit that unifies them fails.
    ///
    /// The feed floors (T-9): `calculate` already applied the Step 6 power
    /// gate and the Step 7 machine ceiling, so the value is bound from above
    /// and a nearest rounding would offer a feed past the ceiling. The depth
    /// rounds to the nearest: its clamps run below this point, so it is not
    /// bound from above here.
    ///
    /// Arm 1 goes through the real `feed_rate()` accessor, so it pins the
    /// wiring and not just the helper. Arm 2 drives the fallback branch
    /// directly, because the demo pocket has a funnel preview for the feed
    /// and cannot reach that branch; it pins that `suggestion_for` honours
    /// the quantiser it is handed, on both directions side by side.
    #[test]
    fn the_feed_floors_while_the_depth_rounds_to_the_nearest() {
        // Arm 1 — end to end, on `hardwood_pocket` (the arithmetic is on
        // that fixture): the calculator gives 3999.738 mm/min and the pill
        // offers 3999. A nearest rounding offers 4000, which is ABOVE the
        // value every ceiling was satisfied at, so this arm is red on the
        // pre-T-9 code. Before feeds matrix R5 the demo pocket gave 1290.9375
        // here; it now lands on the integer 4000. Until ruling R4 WP3 this
        // fixture gave 2571.75 (a 0.75 long-tool and a 0.75 safety factor).
        let (session, tool, op, result, material) = hardwood_pocket();
        let pills = PillSuggestions::new(&op, &result, &tool, session.machine(), &material, None);
        let feed_pill = pills.feed_rate();
        let calculator = result.feed_rate_mm_min;
        assert!(
            (calculator - 3999.738).abs() < 1e-6,
            "fixture drift: raw feed {calculator}"
        );
        assert!(
            feed_pill.suggestion.recommended <= calculator,
            "the feed pill offers {} ABOVE the calculator's {calculator}, which the \
             Step 6 power gate and the Step 7 machine ceiling already bound",
            feed_pill.suggestion.recommended
        );
        assert!(
            (feed_pill.suggestion.recommended - 3999.0).abs() < 1e-9,
            "the feed pill offers {}, not the floor 3999.0",
            feed_pill.suggestion.recommended
        );
        // Non-vacuity: the nearest rounding really does go up on this
        // fixture, so the two assertions above are not an identity.
        assert!(
            round_suggestion_value(calculator, 1.0) > calculator,
            "{calculator} no longer rounds UP at step 1, so arm 1 pins nothing"
        );

        // Arm 2 — the fallback branch, both directions side by side.
        // 2562.504 is the worst case the T-9 core sweep measured, where the
        // funnel shipped 2563 against a calculator 2562.504.
        let raw = (ProvKind::Formula, None);

        // The feed: a fraction of .5 or more, which is exactly where the
        // nearest rounding goes UP.
        let feed = 2562.504_f64;
        let offered = suggestion_for(None, feed, 1.0, round_suggestion_value_down, raw);
        assert!(!offered.clamped, "the fallback branch is the raw value");
        assert!(
            (offered.recommended - 2562.0).abs() < 1e-9,
            "the feed pill offers {}, not the floor 2562.0",
            offered.recommended
        );
        assert!(
            offered.recommended <= feed,
            "the feed pill offers {} ABOVE the calculator's {feed}, which the ceiling \
             already bound",
            offered.recommended
        );
        // The non-vacuity partner: the nearest rounding really does go up
        // here, so the assertion above is not an identity.
        assert!(
            round_suggestion_value(feed, 1.0) > feed,
            "2562.504 no longer rounds UP at step 1, so this test pins nothing"
        );

        // The depth, at the same kind of fraction on its own 0.001 step:
        // 4.2005 sits half way between 4.200 and 4.201 and must go UP.
        let depth = 4.2005_f64;
        let offered_depth = suggestion_for(None, depth, 0.001, round_suggestion_value, raw);
        assert!(
            offered_depth.recommended > depth,
            "the depth pill offers {}, which did not round to the nearest",
            offered_depth.recommended
        );
        assert!(
            (offered_depth.recommended - 4.201).abs() < 1e-9,
            "the depth pill offers {}, not 4.201",
            offered_depth.recommended
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
            None,
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
            rs_cam_core::feeds::embedded_vendor_lut(),
            session.post_config().spindle_strategy,
        )
        .expect("V-bit on VCarve is a valid pairing");
        let pills = PillSuggestions::new(
            &op,
            &result,
            &tool,
            session.machine(),
            &stock.material,
            None,
        );
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
