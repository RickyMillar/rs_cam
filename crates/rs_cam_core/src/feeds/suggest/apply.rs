//! The write-back side of Suggest: the speeds write, the pick write, the
//! per-field preview the inline pill reads, the preview / applicable-
//! recommendation pair the Feeds modal holds, and the stock and drill
//! defaults.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::envelope::{CutPoint, EnvelopeAxes, EnvelopePick, PickRequest, Placement};
use crate::feeds::{FeedsError, FeedsInput, FeedsResult, PassRole, VendorLut};
use crate::machine::MachineProfile;
use crate::material::Material;

use super::invariants::enforce_invariants_at_fixed_geometry;
use super::{
    CalculatorOperatingPoint, StockContext, SuggestContext, SuggestWarning,
    feeds_input_for_operation, round_suggestion_value, round_suggestion_value_down,
};

/// Write the calculator's speeds (feed, plunge, RPM, ramp) to `operation`.
///
/// K2b (feeds contract P5, ruling Q4): Suggest never writes geometry by
/// itself. [`apply`] writes the geometry of an envelope pick first, then
/// calls this. The lead ruling of 2026-11-03 deleted the geometry passes
/// of the chain (0, 2-6, 6b) and the chain test doors.
///
/// **The speeds-only rule (G-SPEEDSAPPLY, 2026-10-12).** The apply does not
/// write the depth per pass or the stepover, so the speeds it writes must be
/// the speeds for the geometry the operation keeps. The operation's own
/// geometry is the fixed input. The feed-side passes (1 plunge ≤ feed, 9 the
/// feed rescale at the final geometry, 10 the power ceiling) and the
/// warn-only passes read that geometry. Pass 9 compares the kept geometry
/// with the calculator's geometry, so the feed that ships is the
/// calculator's feed for the depth and stepover the operation carries:
///
/// - When the result was sized at the kept geometry, the feed is the
///   result's feed rounded down to 1 mm/min.
/// - When the operation already carries the geometry that `Both` writes,
///   the speeds are bit-identical to the speeds of `Both`.
///
/// Before this rule the funnel ran the full chain on the scratch and copied
/// the speeds back. A 3 mm flat at 2.5 x D in oak (`depth_per_pass` 7.5 mm)
/// then shipped 3877.8 mm/min, the feed for the 0.75 mm rigidity cap, where
/// the calculator at 7.5 mm gives 2423 mm/min. Sentry:
/// `tests/a_speeds_only_apply_ships_the_feed_for_the_depth_it_keeps_g_speedsapply.rs`.
///
/// The fields this call does not write, and their provenance, stay untouched
/// under every scope.
#[expect(
    clippy::too_many_arguments,
    reason = "the canonical suggest funnel needs the operation, the provenance out-param, the result, tool/machine/material, pass_role, context and the speeds-explored flag together."
)]
fn apply_feeds_subset(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    speeds_explored: bool,
) -> Vec<SuggestWarning> {
    let mut scratch = operation.clone();
    // T-9: the feed and the plunge round DOWN, not to the nearest.
    //
    // Every ceiling in `calculate` — the Step 6 spindle power gate and the
    // Step 7 machine cutting-feed ceiling — is satisfied at the value
    // `result` carries. A nearest rounding goes UP about half the time, and
    // nothing below re-checks a limit, so the shipped recipe sat up to
    // +0.5 mm/min above the ceiling the clamp exists to enforce. Measured
    // before the fix: Generic Wood Router / Walnut, calculator 2562.5040
    // -> shipped 2563.0000.
    //
    // The cost is stated where it lands: a feed the RUBBING FLOOR raised
    // rounds AWAY from that floor by up to one mm/min. The floor is an
    // advisory band and the power gate is a physical limit, so the trade is
    // correct. `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`
    // pins the direction and `rubbing_floor_never_exceeds_band` pins the
    // band.
    let written_feed = round_suggestion_value_down(result.feed_rate_mm_min, 1.0);
    let written_plunge = round_suggestion_value_down(result.plunge_rate_mm_min, 1.0);
    scratch.set_feed_rate(written_feed);
    scratch.set_plunge_rate(written_plunge);
    // S11: the scratch holds a feed and a plunge from here on (the two
    // writes above). The passes below can move them, never clear them.
    let scratch_feed = |op: &OperationConfig| op.feed_rate().unwrap_or(written_feed);
    let scratch_plunge = |op: &OperationConfig| op.plunge_rate().unwrap_or(written_plunge);
    // The calculator's geometry, rounded to the nearest 0.001 mm and snapped
    // as a geometry write would be, is the pass 9 baseline. This write does
    // not put it on the operation (G-SPEEDSAPPLY), so not holding it is the
    // contract, not a dropped recommendation (T-12).
    let stepover_mm = round_suggestion_value(result.radial_width_mm, 0.001);
    let depth_mm = round_suggestion_value(result.axial_depth_mm, 0.001);
    // The calculator's geometry on the scratch is the pass 9 baseline only;
    // the operation keeps its own (below).
    let _ = scratch.set_stepover(stepover_mm);
    // Snap the proposal to a depth the machine will actually cut.
    //
    // Generation steps a 2.5D cut as `total / ceil(total / per_pass)`
    // (`DepthDistribution::Even`, which no operation makes configurable), so
    // the realised depth is a staircase: on a 12 mm pocket only 4.00, 3.00,
    // 2.40, 2.00, 1.71, 1.50 and 1.33 are reachable.
    //
    // This does NOT change the cut — generation applies the same arithmetic to
    // whatever is written. It makes the number the engine writes, reasons
    // about and shows the operator equal the number the machine will cut.
    // Without it a recommendation of 2.90 mm is reported as 2.90 while 2.40 is
    // cut, off by 17 %, and the power ladder's own account of what it did
    // names a depth that never happens. See T-12.
    //
    // No total, no snap: an operation whose depth comes from the model surface
    // has nothing to divide, and inventing a total would be worse than leaving
    // the proposal alone.
    let depth_mm = match scratch.total_depth() {
        Some(total) => crate::ops::depth::realised_step_down(total, depth_mm).unwrap_or(depth_mm),
        None => depth_mm,
    };
    let _ = scratch.set_depth_per_pass(depth_mm);
    // v3.0d (2026-06-04): also write the calculator's chosen RPM so the
    // rest of enforce_invariants reads a consistent operating point
    // instead of the operation's prior spindle_rpm value.
    //
    // The idempotency argument originally named the chipload
    // recalibration pass (retired 2026-08-13) whose closed-form solve
    // `target × rpm × flutes / arc_fit` propagated RPM drift linearly
    // into feed. That consumer is gone, but the write stays: A-5 measured
    // the rewritten RPM differing from the authored value on three of four
    // fixtures (14 000 → 16 000, 18 000 → 19 000, 20 000 → 19 000), so any
    // downstream pass or reconstruction that reads the operation's RPM
    // still needs this to be the calculator's choice, not a stale one.
    let rpm_written = result.rpm.is_finite() && result.rpm > 0.0;
    if rpm_written {
        scratch.set_spindle_rpm(Some(result.rpm.round() as u32));
    }
    // Enrich the caller-supplied context with the LUT chipload band
    // derived by the calculator so the invariant passes can read it
    // without a separate parameter. Also thread through the matched LUT
    // row + effective diameter so the axial-DOC envelope pass
    // (`pick_axial_envelope`) can query vendor `ap_*_factor` / `ap_*_mm`
    // and the chipload-bounds re-derivation step in `enforce_invariants`
    // can re-apply `doc_derating_scale` against a mutated DPP.
    // ...and the operating point the calculator sized the feed at, so the
    // final pass can re-derive the chip-thinning and depth-tier terms once
    // the clamps above have settled what the operation actually runs.
    // Unrounded on purpose — see `CalculatorOperatingPoint`.
    //
    // Withheld for a drag-to-explore apply: those speeds are the operator's,
    // not the calculator's, so there is no derivation to reconcile and pass 9
    // must leave them exactly as dialled. See `with_explored_speeds`.
    let enriched = SuggestContext {
        chipload_bounds: result.chipload_bounds,
        matched_lut_row: result.matched_lut_row.as_ref(),
        effective_diameter_mm: result.effective_diameter_mm,
        calculator_operating_point: (!speeds_explored).then_some(CalculatorOperatingPoint {
            radial_width_mm: result.radial_width_mm,
            axial_depth_mm: result.axial_depth_mm,
            feed_rate_mm_min: result.feed_rate_mm_min,
            rpm: result.rpm,
        }),
        ..context
    };
    // G-SPEEDSAPPLY (2026-10-12): a speeds-only apply resolves the speeds
    // for the geometry the operation keeps. The scratch gets the operation's
    // own depth per pass and stepover back, the passes that move geometry do
    // not run, and pass 9 re-derives the feed from the calculator's geometry
    // (the values written above) to the kept geometry. See the doc of
    // `apply_feeds_subset`.
    let mut warnings = {
        let calculator_stepover = scratch.stepover();
        let calculator_dpp = scratch.depth_per_pass();
        if let Some(kept) = operation.stepover() {
            scratch.set_stepover(kept);
        }
        if let Some(kept) = operation.depth_per_pass() {
            scratch.set_depth_per_pass(kept);
        }
        enforce_invariants_at_fixed_geometry(
            &mut scratch,
            tool,
            machine,
            material,
            pass_role,
            enriched,
            calculator_stepover,
            calculator_dpp,
        )
    };

    // G10 (2026-09-25): the plunge is a rule of the feed that ships. Pass 9,
    // pass 10 and an explored feed can move the feed after the calculator,
    // so the funnel runs the rule again at the final feed. It writes the
    // plunge only when the value moved by 1 mm/min or more, and files the
    // move. A drill cycle gives no value: its plunge is its feed. The ramp
    // below reads this plunge, so the plunge comes first.
    // Q8 (P8): the axial-chip bound of a Project Curve plunge reads the RPM
    // that ships.
    let shipped_rpm = scratch.spindle_rpm().map_or(result.rpm, f64::from);
    let plunge_binding =
        crate::feeds::plunge::resolve_plunge(&result.plunge, scratch_feed(&scratch), shipped_rpm);
    if let Some((to_mm_min, binding)) = plunge_binding {
        let from_mm_min = scratch_plunge(&scratch);
        if (to_mm_min - from_mm_min).abs() >= 1.0 {
            scratch.set_plunge_rate(to_mm_min);
            warnings.push(SuggestWarning::PlungeReDerived {
                from_mm_min,
                to_mm_min,
                binding,
            });
        }
    }
    // Ruling R4 Q10 (2026-09-24): the calculator lowered the RPM to hold the
    // chip at the feed ceiling. When this apply writes that RPM, the card
    // gets a rationale row on the RPM entry.
    if rpm_written {
        for w in &result.warnings {
            if let crate::feeds::FeedsWarning::RpmLoweredForFeedCeiling {
                rpm_from,
                rpm_to,
                feed_ceiling_mm_min,
                rpm_floor,
                floor_source,
                held,
                arms,
            } = w
            {
                warnings.push(SuggestWarning::RpmLoweredForFeedCeiling {
                    rpm_from: *rpm_from,
                    rpm_to: *rpm_to,
                    feed_ceiling_mm_min: *feed_ceiling_mm_min,
                    rpm_floor: *rpm_floor,
                    floor_source: *floor_source,
                    held: *held,
                    arms: *arms,
                });
            }
        }
    }
    // Ruling R4 (2026-09-24) marked the pass 6b record `applied: false` here
    // when this apply did not write the cut geometry. Since G-SPEEDSAPPLY
    // (2026-10-12) the only scope that does not write the geometry,
    // `Speeds`, does not run pass 6b, so no such record reaches this point.
    {
        operation.set_feed_rate(scratch_feed(&scratch));
        operation.set_plunge_rate(scratch_plunge(&scratch));
        if rpm_written {
            operation.set_spindle_rpm(scratch.spindle_rpm());
        }
    }
    // G6 ramp (2026-09-25): the entry feed. It reads the real `operation`
    // after the copy-back, so it uses the feed and the RPM that ship and
    // the entry that ships (a strategy rewrite on the scratch does not
    // reach the operation). An operation with no ramp field (a drill cycle)
    // refuses the write, and then no record is filed.
    //
    // G10 (2026-09-25): with no G6 chip the ramp holds its vertical rate at
    // the plunge that ships (Q2), and the record carries the entry notes
    // (Q6-Q10, Q12) of the operation that ships.
    let mut ramp_record = None;
    {
        let from_mm_min = operation.ramp_feed_rate();
        let entry = crate::feeds::ramp::entry_geometry(operation, context.dressups, tool);
        let record = crate::feeds::ramp::resolve_ramp_feed(
            &result.ramp,
            entry,
            scratch_feed(operation),
            operation.spindle_rpm(),
            tool.flute_count_planning(),
            scratch_plunge(operation),
        );
        if operation.as_params_mut().set_ramp_feed_rate(record.value()) {
            ramp_record = Some(record.clone());
            let notes =
                crate::feeds::ramp::entry_notes(operation, context.dressups, tool, pass_role);
            warnings.push(SuggestWarning::RampFeed {
                from_mm_min,
                record,
                notes,
            });
        }
    }
    // Stamp per-field provenance from what actually produced these values
    // (W2.1): the speeds only.
    provenance.apply_suggested_subset(result, operation, rpm_written, true, false);
    // G10: the plunge stamp names the term that binds at the feed that
    // ships, which can differ from the calculator's feed.
    if let Some((_, binding)) = plunge_binding
        && let Some(feed_stamp) = provenance.feed_rate.clone()
    {
        provenance.plunge_rate = Some(result.plunge.provenance(binding, feed_stamp));
    }
    if let Some(record) = &ramp_record {
        provenance.stamp_ramp(record);
    }
    warnings
}

/// Apply only the recommended *speeds* (feed / plunge / RPM), leaving the cut
/// geometry (stepover / DOC) and its provenance untouched. The "Apply
/// recommended speeds" action — speed-only by construction (W3.1). F042: a
/// test door (`test-support`); production writes through [`apply`].
#[expect(
    clippy::too_many_arguments,
    reason = "this door forwards the whole funnel argument list — operation, provenance, result, tool/machine/material, pass_role and context — to `apply_feeds_subset`, so it cannot carry fewer arguments."
)]
#[cfg(any(test, feature = "test-support"))]
pub fn apply_speeds_to_op(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation, provenance, result, tool, machine, material, pass_role, context, false,
    )
}

// ── Per-field preview of the funnel (G-PILLCLAMP, 2026-09-10) ──────────────
//
// The GUI's per-field ⚡ pill (Feeds tab feed / plunge, Geometry tab stepover /
// depth-per-pass / drill feed) used to write the RAW calculator value —
// `round_suggestion_value(result.axial_depth_mm, 0.001)` — while every Apply
// button went through `apply_feeds_subset` and wrote the clamped one. On the
// demo pocket (Ø6 flat, Generic Wood Router) the pill wrote 4.2 mm of DOC where
// Apply wrote 1.2 mm (UX-R03-014). The pill now offers and writes the value
// below, which is the funnel's own output read back per field, so the clamp
// chain is never duplicated in the GUI.

/// The value one per-field apply would write, and the stamp it would record.
///
/// Produced by [`preview_field_applies`]. `value` is bit-identical to what
/// `apply_feeds_subset` leaves on the operation for that field (it is read
/// off the funnel's scratch clone, not recomputed), and `provenance` is the
/// stamp the funnel records for it.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldApplyPreview {
    pub field: crate::feeds::FeedsField,
    pub value: f64,
    pub provenance: crate::feeds::ValueProvenance,
}

impl FieldApplyPreview {
    /// Write this one field into `operation` and stamp its provenance. Nothing
    /// else on the operation moves — that is the whole contract of a per-field
    /// pill, and `pill_writes_clamped_value_g_pillclamp.rs` pins it.
    ///
    /// Test door:
    /// `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs`.
    pub fn write_to(
        &self,
        operation: &mut OperationConfig,
        provenance: &mut crate::feeds::FeedsProvenance,
    ) {
        use crate::feeds::FeedsField;
        match self.field {
            FeedsField::FeedRate => operation.set_feed_rate(self.value),
            FeedsField::PlungeRate => operation.set_plunge_rate(self.value),
            // `value` came from `spindle_rpm()` on the scratch clone, a `u32`,
            // so the round-trip is exact (same cast the funnel itself makes).
            FeedsField::SpindleRpm => operation.set_spindle_rpm(Some(self.value.round() as u32)),
            // These two setters report whether the operation carries the
            // field (N5). This funnel keeps its existing behaviour and
            // discards the answer.
            FeedsField::Stepover => {
                operation.set_stepover(self.value);
            }
            FeedsField::DepthPerPass => {
                operation.set_depth_per_pass(self.value);
            }
            FeedsField::ScallopHeight => operation.set_scallop_height(self.value),
            // No pill writes the ramp today; the arm keeps the per-field
            // contract whole. A drill cycle has no field and refuses it.
            FeedsField::RampFeedRate => {
                operation
                    .as_params_mut()
                    .set_ramp_feed_rate(Some(self.value));
            }
            // P10: no pill writes these; the arm writes the slot.
            FeedsField::PeckDepth | FeedsField::ZStep => {
                let field = match self.field {
                    FeedsField::PeckDepth => "peck_depth",
                    FeedsField::FeedRate
                    | FeedsField::PlungeRate
                    | FeedsField::SpindleRpm
                    | FeedsField::Stepover
                    | FeedsField::DepthPerPass
                    | FeedsField::ScallopHeight
                    | FeedsField::RampFeedRate
                    | FeedsField::ZStep => "z_step",
                };
                if let Some(slot) = crate::compute::geometry_default::slot_mut(operation, field) {
                    *slot = Some(self.value);
                }
            }
        }
        provenance.set(self.field, self.provenance.clone());
    }
}

/// Every field the funnel would write for this recommendation, as one
/// preview per field. A field the funnel does **not** write — the operation
/// carries no such dial, or no RPM was recommended — is `None`, and a GUI
/// pill for it must say so rather than offer the raw calculator value as if
/// it were clamped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldApplyPreviews {
    pub feed_rate: Option<FieldApplyPreview>,
    pub plunge_rate: Option<FieldApplyPreview>,
    pub spindle_rpm: Option<FieldApplyPreview>,
    pub stepover: Option<FieldApplyPreview>,
    pub depth_per_pass: Option<FieldApplyPreview>,
}

impl FieldApplyPreviews {
    pub fn get(&self, field: crate::feeds::FeedsField) -> Option<&FieldApplyPreview> {
        use crate::feeds::FeedsField;
        match field {
            FeedsField::FeedRate => self.feed_rate.as_ref(),
            FeedsField::PlungeRate => self.plunge_rate.as_ref(),
            FeedsField::SpindleRpm => self.spindle_rpm.as_ref(),
            FeedsField::Stepover => self.stepover.as_ref(),
            FeedsField::DepthPerPass => self.depth_per_pass.as_ref(),
            // Never suggested, never written by the funnel.
            FeedsField::ScallopHeight => None,
            // Written by the funnel, but no pill offers it: the
            // `SuggestWarning::RampFeed` record carries the value.
            FeedsField::RampFeedRate => None,
            // P10: the tool rule writes these, no pill.
            FeedsField::PeckDepth | FeedsField::ZStep => None,
        }
    }
}

/// Dry-run the apply funnel and read back every field it would write.
///
/// P5 (K2b): the dry run is the apply of the envelope's proposal
/// ([`PickRequest::Proposal`], the "Apply all" of every surface) on a
/// scratch clone of `operation` with a scratch provenance. Each field's
/// value and stamp are read off the scratch. So a ⚡ pill offers the number
/// that "Apply all" writes, the proposal's depth and stepover and the speeds
/// at that point (G-PILLCLAMP; P5 card trap: point the pills at the
/// proposal). A refused pairing gives no previews.
pub fn preview_field_applies(
    operation: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    lut: &'static VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
    context: SuggestContext<'_>,
) -> FieldApplyPreviews {
    use crate::feeds::FeedsField;
    let mut scratch = operation.clone();
    let mut prov = crate::feeds::FeedsProvenance::default();
    let input = OperationApplyInput {
        tool,
        machine,
        material,
        spindle_strategy,
        lut,
        suggest: context,
    };
    if apply_to_operation(
        input,
        Some(PickRequest::Proposal),
        None,
        &mut scratch,
        &mut prov,
    )
    .is_err()
    {
        return FieldApplyPreviews::default();
    }
    let slot = |field: FeedsField, value: Option<f64>| -> Option<FieldApplyPreview> {
        let value = value?;
        let provenance = prov.get(field)?.clone();
        Some(FieldApplyPreview {
            field,
            value,
            provenance,
        })
    };
    FieldApplyPreviews {
        feed_rate: slot(FeedsField::FeedRate, scratch.feed_rate()),
        plunge_rate: slot(FeedsField::PlungeRate, scratch.plunge_rate()),
        spindle_rpm: slot(FeedsField::SpindleRpm, scratch.spindle_rpm().map(f64::from)),
        stepover: slot(FeedsField::Stepover, scratch.stepover()),
        depth_per_pass: slot(FeedsField::DepthPerPass, scratch.depth_per_pass()),
    }
}

/// One field of [`preview_field_applies`].
///
/// Test door:
/// `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs` and
/// `crates/rs_cam_viz/tests/apply_contract_a3.rs`.
#[expect(
    clippy::too_many_arguments,
    reason = "one field of the dry run, so it carries the whole `preview_field_applies` argument list plus the field selector."
)]
pub fn preview_field_apply(
    operation: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    lut: &'static VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
    context: SuggestContext<'_>,
    field: crate::feeds::FeedsField,
) -> Option<FieldApplyPreview> {
    preview_field_applies(
        operation,
        tool,
        machine,
        material,
        lut,
        spindle_strategy,
        context,
    )
    .get(field)
    .cloned()
}

// ── The one application funnel (Checkpoint I, 2026-08-12; K2, P5) ──────────
//
// A-3's census (`planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md`) found
// thirteen GUI affordances writing a recommendation into an `OperationConfig`,
// of which eleven were reachable on a tool × operation pairing the engine had
// already declared physically unrunnable, and seven wrote the raw calculator
// output with no clamp, back-off or rounding at all.
//
// The repair is structural rather than defensive: the type a *preview* surface
// holds ([`FeedsPreview`]) carries no method that can write, and the only way
// to obtain a recommendation is [`FeedsPreview::applicable`], which returns
// `None` when [`crate::feeds::validate_tool_for_operation`] refused. CONTRACT
// K2 (P5): that recommendation is `Recommendation<Partial>`, and [`apply`]
// takes only `Recommendation<Complete>`. The one way between them is
// [`Recommendation::accept_unsourced`] (lead decision D4: an apply on any
// surface is the acceptance).

/// A read-only feeds preview. Infallible by construction — buildable for any
/// input, including a tool × operation pairing the engine refuses, because
/// drawing the nomogram for a pairing you would decline to *run* is exactly
/// what an explanatory surface is for.
///
/// It carries no method that writes to an [`OperationConfig`]. The only bridge
/// from here to a write is [`FeedsPreview::applicable`].
#[derive(Debug, Clone)]
pub struct FeedsPreview {
    explain: crate::feeds::FeedsExplain,
    refusal: Option<FeedsError>,
}

impl FeedsPreview {
    /// Build a preview from a calculator input. Never fails: when
    /// [`crate::feeds::validate_tool_for_operation`] refuses, the refusal is
    /// **recorded**, not returned, and the explanation is still computed.
    pub fn build(input: &FeedsInput<'_>) -> Self {
        let refusal = crate::feeds::validate_tool_for_operation(input).err();
        Self {
            explain: crate::feeds::explain_feeds(input),
            refusal,
        }
    }

    /// The full explanation payload — matched LUT row, sibling rows, machine
    /// envelope. Display only.
    pub fn explain(&self) -> &crate::feeds::FeedsExplain {
        &self.explain
    }

    /// The recommended operating point. **Display only** — this is the raw
    /// calculator output, before any clamp or back-off; writing it into an
    /// operation is the defect this module exists to prevent.
    pub fn recommended(&self) -> &FeedsResult {
        &self.explain.recommended
    }

    /// The refusal, when the tool × operation pairing is physically
    /// unrunnable. A UI holding a `Some` here must render it *in place of*
    /// its apply affordances (Checkpoint I-3) — the explanation survives, the
    /// write becomes impossible.
    pub fn refusal(&self) -> Option<&FeedsError> {
        self.refusal.as_ref()
    }

    /// The only bridge from a preview to a recommendation. `None` exactly
    /// when the pairing was refused.
    ///
    /// The recommendation handed back is `explain().recommended`, which is
    /// `calculate(input)` — bit-identical to what
    /// [`feeds_result_for_operation`](super::feeds_result_for_operation) returns for the same input, since both
    /// call the same calculator on the same [`FeedsInput`]. Pinned by
    /// `preview_recommendation_is_bit_identical_to_validated_recipe`.
    pub fn applicable(&self) -> Option<Recommendation<'_, Partial>> {
        self.try_applicable().ok()
    }

    /// As [`Self::applicable`], with the refusal in the `Err` arm.
    ///
    /// # Errors
    ///
    /// The recorded refusal, when the pairing was refused.
    pub fn try_applicable(&self) -> Result<Recommendation<'_, Partial>, FeedsError> {
        match &self.refusal {
            Some(refusal) => Err(refusal.clone()),
            None => Ok(Recommendation::new(std::borrow::Cow::Borrowed(
                &self.explain.recommended,
            ))),
        }
    }
}

/// The completeness state of a [`Recommendation`]. Sealed: [`Partial`] and
/// [`Complete`] are the only states.
pub trait Completeness: sealed::Sealed {}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Partial {}
    impl Sealed for super::Complete {}
}

/// A recommendation with a written field that reads an Unsourced rule, or
/// one that no apply door has accepted yet. It shows on the card; it cannot
/// be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partial;

/// A recommendation that [`apply`] takes: each written field has a basis
/// that is not an Unsourced rule, or the apply door accepted the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Complete;

impl Completeness for Partial {}
impl Completeness for Complete {}

/// The geometry of an envelope pick that a recommendation carries.
#[derive(Debug, Clone, PartialEq)]
pub struct PickedGeometry {
    /// The point and its basis.
    pub point: crate::feeds::Sourced<CutPoint>,
    /// The axes of the envelope: which fields the pick writes.
    pub axes: EnvelopeAxes,
    /// Where the point sits.
    pub placement: Placement,
    /// The request that gave the point.
    pub request: PickRequest,
}

/// CONTRACT K2. A validated recommendation in the state `S`.
///
/// Constructible only through [`FeedsPreview::applicable`] (or the
/// crate-private `validated` door after the same validation) and
/// [`EnvelopePick::recommendation`]. [`apply`] takes only
/// `Recommendation<Complete>`; the one way there is
/// [`Recommendation::accept_unsourced`].
#[derive(Debug, Clone)]
pub struct Recommendation<'a, S: Completeness> {
    result: std::borrow::Cow<'a, FeedsResult>,
    /// Set by [`Recommendation::with_explored_speeds`]: the feed and RPM on
    /// `result` came off a chart the operator dragged, not out of
    /// [`crate::feeds::calculate`].
    ///
    /// Read by [`apply_feeds_subset`], which then withholds
    /// [`SuggestContext::calculator_operating_point`] so the pass-9 rescale
    /// finds no derivation to reconcile and leaves the dialled value alone.
    speeds_explored: bool,
    /// The geometry of an envelope pick, if the recommendation came from one.
    pick: Option<PickedGeometry>,
    _state: std::marker::PhantomData<S>,
}

impl<'a> Recommendation<'a, Partial> {
    const fn new(result: std::borrow::Cow<'a, FeedsResult>) -> Self {
        Self {
            result,
            speeds_explored: false,
            pick: None,
            _state: std::marker::PhantomData,
        }
    }

    /// F042: a recommendation for a result that
    /// [`super::feeds_result_for_operation`] returned. That door runs
    /// [`crate::feeds::validate_tool_for_operation`] before it calculates,
    /// so the result is validated. Crate-private: only that door's callers
    /// in `feeds::suggest` mint one this way.
    pub(super) const fn validated(result: &'a FeedsResult) -> Self {
        Self::new(std::borrow::Cow::Borrowed(result))
    }

    /// The names of the Unsourced rules that the written fields read: the
    /// RPM, the chip, the plunge and the ramp, and the picked geometry. The
    /// feed is the chip x RPM x Z at the depth the operation keeps (pass 9),
    /// so the chip and the RPM name its rules.
    #[must_use]
    pub fn unsourced_rows(&self) -> Vec<String> {
        let b = &self.result.bases;
        let mut rows: Vec<String> = Vec::new();
        let mut take = |basis: &crate::feeds::Basis| {
            for c in basis.unsourced_rules() {
                let name = c.name.to_string();
                if !rows.contains(&name) {
                    rows.push(name);
                }
            }
        };
        take(&b.rpm);
        take(&b.chip);
        if !self.speeds_explored {
            take(&b.plunge);
            take(&b.ramp);
        }
        if let Some(pick) = &self.pick {
            take(pick.point.basis());
        }
        rows
    }

    /// The one way from `Partial` to `Complete` (lead decision D4): every
    /// apply door calls it. It returns the warning that names the
    /// Unsourced rows, when there are any; the door records it with the
    /// apply.
    #[must_use]
    pub fn accept_unsourced(self) -> (Recommendation<'a, Complete>, Option<SuggestWarning>) {
        let rows = self.unsourced_rows();
        let warning = (!rows.is_empty()).then_some(SuggestWarning::UnsourcedRowsAccepted { rows });
        (
            Recommendation {
                result: self.result,
                speeds_explored: self.speeds_explored,
                pick: self.pick,
                _state: std::marker::PhantomData,
            },
            warning,
        )
    }

    /// Replace the two speed axes with an operator-chosen point, keeping the
    /// rest of the validated recommendation. Backs the modal's
    /// drag-to-explore apply, whose values come off a chart the operator
    /// dragged rather than off the calculator.
    ///
    /// The chipload band is **dropped** on the returned recommendation, and
    /// that is load-bearing: `enforce_invariants`'
    /// `recalibrate_feed_for_chipload` pass short-circuits without a band, so
    /// the funnel's clamps (plunge-to-feed, machine envelope, the DOC chain)
    /// still run while the feed the operator explicitly dialled is **not**
    /// silently re-solved back to the band target.
    ///
    /// The pass-9 rescale keys off the explicit `speeds_explored` flag, set
    /// here, rather than inferring the operator's intent from an absent band.
    /// P5 (K2): the dragged feed and RPM are the operator's values.
    #[must_use]
    pub fn with_explored_speeds(self, feed_mm_min: f64, rpm: f64) -> Self {
        let mut owned = self.result.into_owned();
        owned.feed_rate_mm_min = feed_mm_min.max(1.0);
        owned.rpm = rpm.max(1.0);
        owned.chipload_bounds = None;
        // A2: the point goes with the band. An explored feed is the
        // operator's, so no printed chipload describes it.
        owned.chipload_point_mm = None;
        owned.bases.feed = crate::feeds::Basis::picked();
        owned.bases.rpm = crate::feeds::Basis::picked();
        owned.bases.chip = crate::feeds::Basis::picked();
        Self {
            result: std::borrow::Cow::Owned(owned),
            speeds_explored: true,
            pick: self.pick,
            _state: std::marker::PhantomData,
        }
    }
}

impl<S: Completeness> Recommendation<'_, S> {
    /// The validated operating point.
    pub fn result(&self) -> &FeedsResult {
        &self.result
    }

    /// The envelope pick that this recommendation writes, if any.
    pub const fn pick(&self) -> Option<&PickedGeometry> {
        self.pick.as_ref()
    }
}

impl<'env> EnvelopePick<'env> {
    /// The recommendation of this pick: the speeds at the point (one
    /// closed-form `calculate` call of the envelope) and the point itself.
    #[must_use]
    pub fn recommendation(&self) -> Recommendation<'static, Partial> {
        let mut rec = Recommendation::new(std::borrow::Cow::Owned(self.result().clone()));
        rec.pick = Some(PickedGeometry {
            point: self.point().clone(),
            axes: self.axes(),
            placement: self.placement(),
            request: self.request(),
        });
        rec
    }
}

/// Everything the funnel needs about the world the operation lives in.
/// Bundled so [`apply`] takes one context parameter rather than five.
#[derive(Debug, Clone, Copy)]
pub struct ApplyContext<'a> {
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub pass_role: PassRole,
    /// Project-level slots (model bbox, stock, policy). Pass
    /// [`SuggestContext::default()`] when the caller doesn't cheaply have
    /// them; the funnel enriches it with the recommendation's own LUT band,
    /// matched row and effective diameter regardless.
    pub suggest: SuggestContext<'a>,
}

/// **The single write.** Every apply surface — the Feeds card, the MCP
/// `apply_feeds` tool and the CLI `--apply-suggest` through
/// `Command::ApplyFeeds` ([`apply_to_operation`]), and the add doors through
/// [`super::suggest_for_operation`] — reaches an `OperationConfig` through
/// here.
///
/// CONTRACT K2b (ruling Q4, P5): the write never moves the geometry by
/// itself.
///
/// 1. When the recommendation carries an [`EnvelopePick`], the picked
///    depth and width are written first (the depth snaps once to the
///    staircase the generator cuts; a drill takes the pick as its peck).
///    The stamps record the pick (`Basis::Operator`, law "picked"). A point
///    outside the boundary adds `SuggestWarning::OutsideEnvelope` (K6
///    `project.outside_envelope`, a warning; Q6: no refusal).
/// 2. The speeds are written for the geometry the operation now keeps
///    (G-SPEEDSAPPLY: the geometry passes do not run; pass 9 reconciles the
///    feed with the kept depth and width).
pub fn apply(
    rec: &Recommendation<'_, Complete>,
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    ctx: ApplyContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(pick) = &rec.pick {
        warnings.extend(write_pick(operation, provenance, pick, ctx));
    }
    warnings.extend(apply_feeds_subset(
        operation,
        provenance,
        rec.result(),
        ctx.tool,
        ctx.machine,
        ctx.material,
        ctx.pass_role,
        ctx.suggest,
        rec.speeds_explored,
    ));
    if let Some(pick) = &rec.pick
        && let Placement::Outside { binding, margin } = pick.placement
    {
        let p = pick.point.get();
        warnings.push(SuggestWarning::OutsideEnvelope {
            binding,
            margin,
            ap_mm: p.ap_mm,
            ae_mm: p.ae_mm,
        });
    }
    warnings
}

/// A picked depth or width, rounded DOWN to 0.001 mm (K2: the write never
/// ships above the picked point). The quotient takes a tolerance of 1e-6
/// of a step, so a value that is a whole number of steps in decimal (1.2)
/// but not in binary (1199.9999999999998 steps) keeps its value.
fn pick_mm_down(mm: f64) -> f64 {
    /// The step of a written geometry value (mm): the step of the funnel's
    /// nearest rounding of the same fields.
    const STEPS_PER_MM: f64 = 1000.0;
    /// The tolerance of the quotient, in steps.
    const STEP_TOLERANCE: f64 = 1.0e-6;
    (mm * STEPS_PER_MM + STEP_TOLERANCE).floor() / STEPS_PER_MM
}

/// Write the geometry of an envelope pick: the free axes only.
fn write_pick(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    pick: &PickedGeometry,
    ctx: ApplyContext<'_>,
) -> Vec<SuggestWarning> {
    use crate::compute::catalog::Radial;
    let mut warnings = Vec::new();
    let p = pick.point.get();
    let stamp = crate::feeds::ValueProvenance::of_basis(&crate::feeds::Basis::picked());
    let op_kind = operation.op_type().name();
    let ap_free = matches!(
        pick.axes,
        EnvelopeAxes::Plane { .. } | EnvelopeAxes::AxialBar { .. }
    );
    let ae_free = matches!(
        pick.axes,
        EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. }
    );
    if ae_free {
        let radial = operation.cut_spec().radial;
        let held = match radial {
            // A scallop width: the chord height of the ball at the picked
            // width (h = r - sqrt(r^2 - (ae/2)^2)) is the target that gives it.
            Radial::Scallop(_) => {
                let r = match ctx.tool.geometry_hint() {
                    crate::feeds::ToolGeometryHint::Ball => ctx.tool.diameter / 2.0,
                    crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
                    crate::feeds::ToolGeometryHint::Bull { corner_radius } => corner_radius,
                    crate::feeds::ToolGeometryHint::Flat
                    | crate::feeds::ToolGeometryHint::VBit { .. } => 0.0,
                };
                let half = p.ae_mm / 2.0;
                if r > half {
                    operation.set_scallop_height(r - (r * r - half * half).sqrt());
                    true
                } else {
                    false
                }
            }
            Radial::Width(_)
            | Radial::Slot
            | Radial::Groove
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => {
                // K2 (P5): the write rounds DOWN, so a pick at a ceiling
                // never ships above it (T-9 is the same rule for the feed).
                let held = operation.set_stepover(pick_mm_down(p.ae_mm));
                if held {
                    provenance.stepover = Some(stamp.clone());
                }
                held
            }
        };
        if !held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "stepover",
                op_kind,
                recommended_mm: p.ae_mm,
            });
        }
    }
    if ap_free {
        let held = match operation {
            OperationConfig::Drill(cfg) => {
                cfg.peck_depth = Some(peck_for_hole(
                    OperationType::Drill,
                    p.ap_mm,
                    cfg.floor.depth_below_top(),
                ));
                true
            }
            OperationConfig::AlignmentPinDrill(cfg) => {
                cfg.peck_depth = Some(peck_for_hole(
                    OperationType::AlignmentPinDrill,
                    p.ap_mm,
                    ctx.suggest
                        .stock
                        .and_then(|s| s.stock_z)
                        .map(|z| z + cfg.spoilboard_penetration),
                ));
                true
            }
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::VCarve(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Bore(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::DropCutter(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::Waterline(_)
            | OperationConfig::Pencil(_)
            | OperationConfig::Scallop(_)
            | OperationConfig::UnifiedFinish(_)
            | OperationConfig::SteepShallow(_)
            | OperationConfig::RampFinish(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::HorizontalFinish(_)
            | OperationConfig::ProjectCurve(_) => {
                // The snap applies once, to the picked value (P5 card
                // trap): generation cuts `total / ceil(total / per_pass)`.
                // K2 (P5): the write rounds DOWN, so a pick at a ceiling
                // never ships above it (a nearest rounding raised a 0.43793
                // mm rigidity cap to 0.438).
                let depth = pick_mm_down(p.ap_mm);
                let depth = match operation.total_depth() {
                    Some(total) => {
                        crate::ops::depth::realised_step_down(total, depth).unwrap_or(depth)
                    }
                    None => depth,
                };
                let held = operation.set_depth_per_pass(depth);
                if held {
                    provenance.depth_per_pass = Some(stamp);
                }
                held
            }
        };
        if !held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "depth_per_pass",
                op_kind,
                recommended_mm: p.ap_mm,
            });
        }
    }
    warnings
}

/// A feed and an RPM the operator dragged on the explore chart (the
/// nomogram), for [`Recommendation::with_explored_speeds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExploredSpeeds {
    /// The dragged feed, mm/min.
    pub feed_mm_min: f64,
    /// The dragged spindle speed, rev/min.
    pub rpm: f64,
}

/// The project inputs of [`apply_to_operation`]: the tool, the machine,
/// the stock material, the post spindle strategy and the project slots of
/// the Suggest context.
#[derive(Clone, Copy)]
pub struct OperationApplyInput<'a> {
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub spindle_strategy: crate::feeds::SpindleStrategy,
    pub lut: &'static VendorLut,
    /// The project slots: the model bbox, the stock and the dressups.
    pub suggest: SuggestContext<'a>,
}

/// F114: THE Apply-Suggest funnel for an existing operation. It builds the
/// preview, refuses a pairing Suggest refuses, builds the envelope and
/// picks the point when `pick` is given (K2b), takes the explored speeds
/// when given, accepts the Unsourced rows (D4) and runs [`apply`].
/// `Command::ApplyFeeds` calls it on a draft, so the GUI, MCP and CLI write
/// the same values for the same state.
///
/// With `pick: None` the funnel writes the speeds for the geometry the
/// operation keeps and no geometry (Q4).
///
/// # Errors
///
/// [`FeedsError::ToolOutOfDomain`] for a tool that builds no cutter, and
/// the refusal of [`crate::feeds::validate_tool_for_operation`]. Nothing is
/// written then.
pub fn apply_to_operation(
    input: OperationApplyInput<'_>,
    pick: Option<PickRequest>,
    explored: Option<ExploredSpeeds>,
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
) -> Result<Vec<SuggestWarning>, FeedsError> {
    input.tool.validate().map_err(FeedsError::ToolOutOfDomain)?;
    let preview = feeds_preview_for_operation(
        operation,
        input.tool,
        input.material,
        input.machine,
        input.lut,
        input.spindle_strategy,
    );
    let rec = preview.try_applicable()?;
    let envelope = match pick {
        Some(_) => Some(crate::feeds::envelope::SafeEnvelope::for_cut(
            crate::feeds::envelope::EnvelopeInput {
                operation,
                tool: input.tool,
                machine: input.machine,
                material: input.material,
                lut: input.lut,
                spindle_strategy: input.spindle_strategy,
                context: input.suggest,
            },
        )?),
        None => None,
    };
    let rec = match (&envelope, pick) {
        (Some(env), Some(request)) => env.pick(request).recommendation(),
        (Some(_) | None, None | Some(_)) => Recommendation {
            result: std::borrow::Cow::Owned(rec.result.into_owned()),
            speeds_explored: rec.speeds_explored,
            pick: rec.pick,
            _state: std::marker::PhantomData,
        },
    };
    let rec = match explored {
        Some(speeds) => rec.with_explored_speeds(speeds.feed_mm_min, speeds.rpm),
        None => rec,
    };
    let (rec, accepted) = rec.accept_unsourced();
    let pass_role = operation.feeds_style().1;
    let mut warnings = apply(
        &rec,
        operation,
        provenance,
        ApplyContext {
            tool: input.tool,
            machine: input.machine,
            material: input.material,
            pass_role,
            suggest: input.suggest,
        },
    );
    warnings.extend(accepted);
    Ok(warnings)
}

/// Build a [`FeedsPreview`] for an operation in a project context — the
/// preview counterpart of [`feeds_result_for_operation`](super::feeds_result_for_operation) and the entry point
/// every apply surface should use.
///
/// Prefer this over [`feeds_explain_for_operation`](super::feeds_explain_for_operation) anywhere an Apply button
/// might live: the explain payload alone cannot tell a UI that the pairing was
/// refused, which is precisely how the modal came to offer eleven writes on
/// operations the engine had declined to run.
pub fn feeds_preview_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> FeedsPreview {
    let input =
        feeds_input_for_operation(operation, tool, material, machine, lut, spindle_strategy);
    FeedsPreview::build(&input)
}

/// Apply drill-cycle defaults that depend on tool diameter + material —
/// specifically `peck_depth`, which gets overwritten with
/// `material.drill_default_peck_depth_mm(tool.diameter)`.
///
/// Pre-2026-06-02 `DrillConfig::default()` hardcoded `peck_depth = 3.0`
/// regardless of cutter diameter or material; for a 3 mm bit that's a
/// full-diameter peck (unsafe), for a 12 mm bit that's 0.25×D
/// (trivially shallow). The Suggest path now overwrites the value
/// every time it runs, so the operating point auto-scales with tool
/// diameter and material per-peck threshold (audit finding "peck_depth
/// is a hardcoded constant with no diameter/material scaling",
/// workflow `w39ma2j1y`, fix #6).
///
/// # Source status (ruling B5, G6 drill, 2026-09-24)
///
/// The peck depth is a repo rule. The per-peck maximum is 6, 5 or 4 x D
/// by Janka band, and 1.5 x D for plywood and sheet goods
/// ([`Material::drill_per_peck_max_dtd`]). Suggest writes half of that
/// maximum. No vendor prints a per-peck depth: the G6 fetch found none.
/// The G6 drill claim card and `trace::narrate` state the same rule.
///
/// Non-drill ops are untouched.
///
/// # Why `stock` (DR-PIN, 2026-08-14)
///
/// Both drill families get the same Suggest peck, but only `Drill`
/// carries its hole depth in its own config. `AlignmentPinDrill`'s hole
/// depth is `stock_z + spoilboard_penetration`, computed at generation
/// time in `compute::execute::generate_alignment_pin_drill` — so until
/// this parameter existed the pin family was written **unclamped**, and
/// a softwood Ø6 pin drill took an 18 mm Suggest peck against a ~13 mm
/// hole: a single-shot cycle wearing a `Peck` label, which the per-peck
/// gate then passed at 2.17 vs 6.0 because one descent is a legal
/// descent (`TECH_DEBT_2_CLOSEOUT.md` §4.4, DR-PIN).
///
/// `stock` is `Option` because not every Suggest caller has a project:
/// the canonical GUI/MCP path
/// ([`crate::session::ProjectSession::cutter_op_profile`]) populates
/// [`SuggestContext::stock`], the strategy-advisor probe in
/// `session::compute` passes `SuggestContext::default()`. **`None` means
/// the pin depth cap cannot be applied**, not that it was applied and
/// found nothing to do. The value is the Suggest default inside the
/// schema range ([`peck_for_hole`]), and generation's own emitter guard
/// ([`crate::ops::drill::fed_descents`]) remains the last line.
pub fn apply_drill_defaults(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    stock: Option<&StockContext>,
) {
    let d = tool.diameter;
    if !d.is_finite() || d <= 0.0 {
        return;
    }
    let peck = material.drill_default_peck_depth_mm(d);
    match operation {
        OperationConfig::Drill(cfg) => {
            // Clamp peck so the cycle still pecks at least once
            // (peck must be strictly less than hole depth). With the
            // Janka-banded per-peck max introduced 2026-06-03, a
            // softwood Suggest default of 3×D can exceed shallow
            // drill_depth values (e.g. 12 mm bit + 25 mm hole gives
            // 36 mm peck before clamping). Without this guard the
            // matrix `drill_no_peck_cycle` anti-pattern fires.
            cfg.peck_depth = Some(peck_for_hole(
                OperationType::Drill,
                peck,
                cfg.floor.depth_below_top(),
            ));
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // Same clamp, same ceiling factor, against the depth this
            // family will actually drill. Mirrors `generate_alignment_
            // pin_drill`'s `let depth = stock_z + cfg.spoilboard_
            // penetration;` — if that expression moves, this one has to
            // move with it.
            cfg.peck_depth = Some(peck_for_hole(
                OperationType::AlignmentPinDrill,
                peck,
                stock
                    .and_then(|ctx| ctx.stock_z)
                    .map(|z| z + cfg.spoilboard_penetration),
            ));
        }
        // No drill peck: Suggest writes no peck for this op. A new
        // drill-like op fails to compile here.
        OperationConfig::Face(_)
        | OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_) => {}
    }
}

/// The peck Suggest writes for a hole of `drill_depth` (`None`: the depth
/// is not known). One rule for every drill family:
///
/// 1. The peck is at most `0.75 × drill_depth`, so the cycle pecks at
///    least twice. Chosen factor: 0.75, which keeps the peck count low
///    (2 pecks for typical holes).
/// 2. The peck stays inside the `peck_depth` range of the op's schema
///    (`ParamDef`, B14). The setter refuses a value outside it, so Suggest
///    does not write one. Where rule 1 gives less than the floor, the
///    peck is the floor.
/// 3. A hole shallower than the floor takes one peck equal to its depth.
///    This is the only case that writes a peck under the floor.
///
/// A non-positive or non-finite depth counts as not known.
pub(crate) fn peck_for_hole(op_type: OperationType, peck: f64, drill_depth: Option<f64>) -> f64 {
    let depth = drill_depth.filter(|d| d.is_finite() && *d > 0.0);
    let capped = depth.map_or(peck, |d| peck.min(d * 0.75));
    let Some(range) = OperationConfig::param_range_for_type(op_type, "peck_depth") else {
        return capped;
    };
    let bounds = range.inclusive_bounds();
    let in_range = capped.max(*bounds.start()).min(*bounds.end());
    match depth {
        Some(d) if *bounds.start() > d => d,
        _ => in_range,
    }
}

/// Apply stock-aware overrides to defaults that require stock context.
pub fn apply_stock_defaults(operation: &mut OperationConfig, ctx: &StockContext) {
    match operation {
        OperationConfig::DropCutter(cfg) => {
            cfg.min_z = ctx.stock_bottom_z;
        }
        OperationConfig::Face(cfg) => {
            cfg.depth = ctx.stock_padding.max(1.0);
        }
        // Through the stock. S11: a stock with no thickness gives no floor;
        // the floor stays `Floor::Unset`.
        OperationConfig::Profile(cfg) => {
            if let Some(z) = ctx.stock_z {
                cfg.floor = crate::compute::floor::Floor::top(z);
            }
        }
        OperationConfig::Drill(cfg) => {
            if let Some(z) = ctx.stock_z {
                cfg.floor = crate::compute::floor::Floor::top(z);
            }
        }
        // F161/F183: these ops take no stock-aware default. Each is named, so
        // a new op is a compile error here until it states its default.
        // S11 (operator ruling 2026-10-23): Pocket and Adaptive 2D had a floor
        // of a part of the stock (min(half, 5 mm) and half). No rule gives
        // that part, so their floor stays `Floor::Unset` until the operator
        // sets it. A face pick still sets "each picked face"
        // (`each_face_floor_due`).
        OperationConfig::Pocket(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_)
        | OperationConfig::AlignmentPinDrill(_) => {}
    }
}

/// Extract operation-specific hints for the feeds calculator.
/// Returns `(axial_depth_hint, radial_width_hint, scallop_hint)`.
///
/// Thin tuple adapter over CONTRACT K1 and K2 (feeds contract P5): the op
/// builds its [`CutSpec`](crate::compute::catalog::CutSpec) with
/// `OperationConfig::cut_spec`, and
/// [`CutSpec::calc_inputs`](crate::compute::catalog::CutSpec::calc_inputs)
/// gives the operator's cut to the calculator: the stated depth, the stated
/// width (the tool diameter for a slot) and the scallop target. Both
/// matches name every variant; no wildcard arm is left.
pub fn operation_feeds_hints(
    operation: &OperationConfig,
    tool_diameter_mm: f64,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    operation.cut_spec().calc_inputs(tool_diameter_mm)
}
