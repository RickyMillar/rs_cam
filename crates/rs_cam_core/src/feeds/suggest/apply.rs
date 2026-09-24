//! The write-back side of Suggest: the apply scope, the funnel that resolves
//! the whole recommended operating point and copies a subset back, the
//! per-field preview the inline pill reads, the preview / applicable-
//! recommendation pair the Feeds modal holds, and the stock and drill
//! defaults.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{FeedsError, FeedsInput, FeedsResult, PassRole, VendorLut};
use crate::machine::MachineProfile;
use crate::material::Material;

use super::invariants::enforce_invariants;
use super::{
    CalculatorOperatingPoint, StockContext, SuggestContext, SuggestWarning,
    feeds_input_for_operation, round_suggestion_value, round_suggestion_value_down,
};

/// Which dimensions of a [`FeedsResult`] an apply writes back to the operation.
///
/// W3.1 (IA cleanup) split the single apply into a SPEED path (feed / plunge /
/// RPM — "how fast") and a CUT-geometry path (stepover / DOC — "how deep/wide,
/// changes the cut"), so the Feeds UI can offer a speed-only "Apply recommended
/// speeds" that never silently rewrites the cut geometry.
///
/// **There is deliberately no `Field` arm** (Checkpoint I-1, 2026-08-12). The
/// Feeds & Speeds modal used to carry six per-field `Apply` buttons that wrote
/// `FeedsExplain::recommended` straight into the operation, skipping
/// [`enforce_invariants`] entirely; on the shipped default fixture that wrote a
/// **4.445 mm** depth of cut where this funnel writes **1.27 mm** (3.50×). The
/// ruling deleted those buttons rather than plumbing a per-field scope, so the
/// scope vocabulary stays "how fast" / "changes the cut" — the two things a
/// user can be told about — and every apply resolves the *whole* operating
/// point before copying a subset back.
///
/// The surviving per-field affordance — the inline ⚡ pill — does not get a
/// scope either. Since G-PILLCLAMP (2026-09-10) it reads its value from
/// [`preview_field_applies`], a dry run of `Both` on a scratch clone, so it
/// offers and writes the funnel's number for that one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyScope {
    /// feed / plunge / RPM — "how fast". Never touches the cut geometry.
    Speeds,
    /// stepover / DOC — "changes the cut". Never touches the speeds.
    CutGeometry,
    /// Both halves, in one transaction.
    Both,
}

/// Apply the calculator's recommendation to `operation`, writing back only the
/// requested [`ApplyScope`].
///
/// The recommended *full* operating point is run through `enforce_invariants`
/// on a scratch clone — so the feed ↔ chipload ↔ DPP coupling is resolved
/// against the complete recommended state — and then only the requested fields
/// are copied into the real operation. This keeps a speed-only or geometry-only
/// apply byte-identical to the combined apply for the fields it does write,
/// while leaving the others (and their provenance) untouched.
// SAFETY: the canonical suggest funnel needs the operation, the provenance
// out-param, the result, tool/machine/material, pass_role, context, the apply
// subset and the speeds-explored flag together.
#[allow(clippy::too_many_arguments)]
fn apply_feeds_subset(
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    subset: ApplyScope,
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
    scratch.set_feed_rate(round_suggestion_value_down(result.feed_rate_mm_min, 1.0));
    scratch.set_plunge_rate(round_suggestion_value_down(result.plunge_rate_mm_min, 1.0));
    // T-12: both setters return `bool` so the caller can refuse instead of
    // discarding the value — their doc comments say exactly that. Capture
    // the refusals here and report them below, but ONLY when this call was
    // asked to write the cut geometry. Under `ApplyScope::Speeds` not
    // writing the depth is the contract, not a dropped recommendation.
    //
    // The two geometry fields keep the NEAREST rounding. Their clamps sit in
    // `enforce_invariants`, which runs BELOW this point and moves geometry
    // downward, so neither value is bound from above here — the direction
    // T-9 is about. The depth is then snapped to the reachable staircase
    // anyway, which dominates a 0.0005 mm rounding step.
    let stepover_mm = round_suggestion_value(result.radial_width_mm, 0.001);
    let depth_mm = round_suggestion_value(result.axial_depth_mm, 0.001);
    let stepover_held = scratch.set_stepover(stepover_mm);
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
    let depth_held = scratch.set_depth_per_pass(depth_mm);
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
    let mut warnings =
        enforce_invariants(&mut scratch, tool, machine, material, pass_role, enriched);

    let write_speeds = matches!(subset, ApplyScope::Speeds | ApplyScope::Both);
    let write_geometry = matches!(subset, ApplyScope::CutGeometry | ApplyScope::Both);
    // Ruling R4 Q10 (2026-09-24): the calculator lowered the RPM to hold the
    // chip at the feed ceiling. When this apply writes that RPM, the card
    // gets a rationale row on the RPM entry.
    if write_speeds && rpm_written {
        for w in &result.warnings {
            if let crate::feeds::FeedsWarning::RpmLoweredForFeedCeiling {
                rpm_from,
                rpm_to,
                feed_ceiling_mm_min,
                rpm_floor,
                floor_source,
                held,
            } = w
            {
                warnings.push(SuggestWarning::RpmLoweredForFeedCeiling {
                    rpm_from: *rpm_from,
                    rpm_to: *rpm_to,
                    feed_ceiling_mm_min: *feed_ceiling_mm_min,
                    rpm_floor: *rpm_floor,
                    floor_source: *floor_source,
                    held: *held,
                });
            }
        }
    }
    // Ruling R4 (2026-09-24): pass 6b ran on the scratch copy. When this
    // apply does not write the cut geometry, its record must not claim the
    // operation carries the engagement it computed.
    if !write_geometry {
        for w in &mut warnings {
            if let SuggestWarning::EngagementReducedForAggressiveness { applied, .. } = w {
                *applied = false;
            }
        }
    }
    if write_speeds {
        operation.set_feed_rate(scratch.feed_rate());
        operation.set_plunge_rate(scratch.plunge_rate());
        if rpm_written {
            operation.set_spindle_rpm(scratch.spindle_rpm());
        }
    }
    if write_geometry {
        if let Some(v) = scratch.as_params().stepover() {
            operation.set_stepover(v);
        }
        if let Some(v) = scratch.as_params().depth_per_pass() {
            operation.set_depth_per_pass(v);
        }
        // T-12: this call was asked to write the cut geometry, and the
        // operation has no field to hold one of the values. Say so. The
        // pre-T-12 funnel returned success here, so a caller that needed a
        // shallower pass — the power derate does — could not tell the
        // difference between "applied" and "silently dropped".
        let op_kind = operation.op_type().name();
        if !depth_held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "depth_per_pass",
                op_kind,
                recommended_mm: depth_mm,
            });
        }
        if !stepover_held {
            warnings.push(SuggestWarning::CutGeometryFieldNotHeld {
                param_name: "stepover",
                op_kind,
                recommended_mm: stepover_mm,
            });
        }
    }
    // Stamp per-field provenance from what actually produced these values
    // (W2.1), gated to the subset we wrote. enforce_invariants may have
    // recalibrated feed/DPP, but the values remain suggest-derived, so the
    // source labels still hold.
    provenance.apply_suggested_subset(result, operation, rpm_written, write_speeds, write_geometry);
    warnings
}

/// Apply the full recommendation — both speeds and cut geometry. The canonical
/// suggest funnel (`suggest_for_operation`, CLI, the GUI "Apply all").
///
/// Values are rounded for UI-friendly display before clamping, matching the
/// historical Suggest-button behaviour.
///
/// `context` carries project-level slots (model bbox, upstream leftover,
/// strategy hint) used by gate-aware Suggest paths — v1.2 reads
/// `context.model_bbox` to gate the runtime-sanity stepover back-off.
/// Callers that don't have the context cheaply available should pass
/// [`SuggestContext::default()`]; the back-off short-circuits to a no-op
/// when `model_bbox` is `None`.
// SAFETY: W2.1 added the `provenance` out-param for per-field stamping, so the
// funnel needs the operation, the provenance, the result, tool/machine/material,
// pass_role and context together.
#[allow(clippy::too_many_arguments)]
pub fn apply_feeds_result_to_op(
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
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Both,
        false,
    )
}

/// Apply only the recommended *speeds* (feed / plunge / RPM), leaving the cut
/// geometry (stepover / DOC) and its provenance untouched. The "Apply
/// recommended speeds" action — speed-only by construction (W3.1).
// SAFETY: this door forwards the whole funnel argument list — operation,
// provenance, result, tool/machine/material, pass_role and context — to
// `apply_feeds_subset`, so it cannot carry fewer arguments.
#[allow(clippy::too_many_arguments)]
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
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Speeds,
        false,
    )
}

/// Apply only the recommended *cut geometry* (stepover / DOC), leaving the
/// speeds and their provenance untouched. The attributed "Apply cut" action
/// (W3.1) — kept distinct because changing DOC/WOC changes the cut.
// SAFETY: this door forwards the whole funnel argument list — operation,
// provenance, result, tool/machine/material, pass_role and context — to
// `apply_feeds_subset`, so it cannot carry fewer arguments.
#[allow(clippy::too_many_arguments)]
pub fn apply_cut_geometry_to_op(
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
        operation,
        provenance,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::CutGeometry,
        false,
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
        }
    }
}

/// Dry-run the apply funnel and read back every field it would write.
///
/// Runs `apply_feeds_subset(ApplyScope::Both)` on a scratch clone of
/// `operation` with a scratch provenance, then reads each field's value and
/// stamp off the scratch. There is still no `ApplyScope::Field` arm (Checkpoint
/// I-1): the whole operating point is resolved, and a single field is copied
/// out of it — which is exactly what a per-field pill must offer.
pub fn preview_field_applies(
    operation: &OperationConfig,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> FieldApplyPreviews {
    use crate::feeds::FeedsField;
    let mut scratch = operation.clone();
    let mut prov = crate::feeds::FeedsProvenance::default();
    apply_feeds_subset(
        &mut scratch,
        &mut prov,
        result,
        tool,
        machine,
        material,
        pass_role,
        context,
        ApplyScope::Both,
        false,
    );
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
        feed_rate: slot(FeedsField::FeedRate, Some(scratch.feed_rate())),
        plunge_rate: slot(FeedsField::PlungeRate, Some(scratch.plunge_rate())),
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
// SAFETY: one field of the dry run, so it carries the whole
// `preview_field_applies` argument list plus the field selector.
#[allow(clippy::too_many_arguments)]
pub fn preview_field_apply(
    operation: &OperationConfig,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    field: crate::feeds::FeedsField,
) -> Option<FieldApplyPreview> {
    preview_field_applies(
        operation, result, tool, machine, material, pass_role, context,
    )
    .get(field)
    .cloned()
}

// ── The one application funnel (Checkpoint I, 2026-08-12) ──────────────────
//
// A-3's census (`planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md`) found
// thirteen GUI affordances writing a recommendation into an `OperationConfig`,
// of which eleven were reachable on a tool × operation pairing the engine had
// already declared physically unrunnable, and seven wrote the raw calculator
// output with no clamp, back-off or rounding at all.
//
// The repair is structural rather than defensive: the type a *preview* surface
// holds ([`FeedsPreview`]) carries no method that can write, and the only way
// to obtain something that can write ([`ApplicableRecommendation`]) is
// [`FeedsPreview::applicable`], which returns `None` when
// [`crate::feeds::validate_tool_for_operation`] refused. There is exactly one
// write function ([`apply`]), and it always runs `enforce_invariants`.
//
// EXCLUDED FROM THIS FUNNEL, DELIBERATELY (Checkpoint I-4):
//
// - The optimizer's candidate apply (`AppEvent::ApplyOptimizeCandidate`) and
//   its project batch. Their candidates are whole `OperationConfig` snapshots
//   that have been **scored against a simulated cut trace end to end** — they
//   answer to the gate verdicts, not to the pre-simulation feeds calculator,
//   and re-clamping a sim-verified operating point against a pre-sim estimator
//   would substitute the weaker evidence for the stronger one.
// - NOT excluded: the optimizer's single-axis suggestion accept
//   (`reoptimize_with_axis_override`). That writes an **un-simulated**
//   suggested value with no clamp, which is the same defect in a second
//   neighbourhood, so it routes through [`resolve_operation_invariants`].

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

    /// The only bridge from a preview to a write. `None` exactly when the
    /// pairing was refused.
    ///
    /// The recommendation handed back is `explain().recommended`, which is
    /// `calculate(input)` — bit-identical to what
    /// [`feeds_result_for_operation`] returns for the same input, since both
    /// call the same calculator on the same [`FeedsInput`]. Pinned by
    /// `preview_recommendation_is_bit_identical_to_validated_recipe`.
    pub fn applicable(&self) -> Option<ApplicableRecommendation<'_>> {
        if self.refusal.is_some() {
            return None;
        }
        Some(ApplicableRecommendation {
            result: std::borrow::Cow::Borrowed(&self.explain.recommended),
            speeds_explored: false,
        })
    }
}

/// A recommendation that has passed [`crate::feeds::validate_tool_for_operation`].
///
/// Constructible **only** via [`FeedsPreview::applicable`] — the field is
/// private and there is no public constructor — so possession of one is proof
/// that the pairing validated. It is the sole input to [`apply`].
#[derive(Debug, Clone)]
pub struct ApplicableRecommendation<'a> {
    result: std::borrow::Cow<'a, FeedsResult>,
    /// Set by [`ApplicableRecommendation::with_explored_speeds`]: the feed and
    /// RPM on `result` came off a chart the operator dragged, not out of
    /// [`crate::feeds::calculate`].
    ///
    /// Read by [`apply_feeds_subset`], which then withholds
    /// [`SuggestContext::calculator_operating_point`] so the pass-9 rescale
    /// finds no derivation to reconcile and leaves the dialled value alone.
    speeds_explored: bool,
}

impl ApplicableRecommendation<'_> {
    /// The validated operating point.
    pub fn result(&self) -> &FeedsResult {
        &self.result
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
    /// silently re-solved back to the band target. Applying an explored point
    /// and then having the feed move on its own would be a new instance of
    /// the defect this funnel closes, not a fix for it.
    ///
    /// The dropped band is **not** a general "do not touch this feed" signal
    /// and must not be reused as one — a legitimate calculator result on an
    /// RPM-only vendor row publishes no band either, and that feed does want
    /// reconciling. So the 2026-08-19 pass-9 rescale keys off its own explicit
    /// [`ApplicableRecommendation::speeds_explored`] flag, set here, rather
    /// than inferring the operator's intent from an absent band.
    #[must_use]
    pub fn with_explored_speeds(self, feed_mm_min: f64, rpm: f64) -> Self {
        let mut owned = self.result.into_owned();
        owned.feed_rate_mm_min = feed_mm_min.max(1.0);
        owned.rpm = rpm.max(1.0);
        owned.chipload_bounds = None;
        // A2: the point goes with the band. An explored feed is the
        // operator's, so no printed chipload describes it.
        owned.chipload_point_mm = None;
        Self {
            result: std::borrow::Cow::Owned(owned),
            speeds_explored: true,
        }
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

/// **The single write.** Every apply surface — properties panel, Feeds &
/// Speeds modal, the modal's project batch, the MCP `apply_feeds` tool, the
/// CLI — reaches an `OperationConfig` through here, and this always runs
/// `enforce_invariants`.
///
/// Because the only `ApplicableRecommendation` in existence came out of a
/// [`FeedsPreview`] whose validation succeeded, a refused pairing cannot reach
/// this function at all; and because there is one function, a write cannot
/// skip the clamps.
pub fn apply(
    rec: &ApplicableRecommendation<'_>,
    scope: ApplyScope,
    operation: &mut OperationConfig,
    provenance: &mut crate::feeds::FeedsProvenance,
    ctx: ApplyContext<'_>,
) -> Vec<SuggestWarning> {
    apply_feeds_subset(
        operation,
        provenance,
        rec.result(),
        ctx.tool,
        ctx.machine,
        ctx.material,
        ctx.pass_role,
        ctx.suggest,
        scope,
        rec.speeds_explored,
    )
}

/// Run the funnel's **clamp stage** over an operation that has just been
/// edited by hand or by the optimizer — an edit that is not a `FeedsResult`
/// and therefore has nothing for [`apply`] to write.
///
/// Checkpoint I-4 routes `reoptimize_with_axis_override` (OPT-005, "accept
/// this axis suggestion") through here. That path sets one of feed / RPM /
/// stepover / DOC / scallop height to a value the optimizer *suggested* but
/// never simulated, and before this it did so with no clamp at all.
///
/// The value itself is left alone — no rounding, no feed re-solve. Callers
/// that want the chipload recalibration must populate
/// `context.chipload_bounds`; with the default context that pass
/// short-circuits, which is what an accepted operator choice should get: the
/// safety clamps, not a substitute number.
pub fn resolve_operation_invariants(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    enforce_invariants(operation, tool, machine, material, pass_role, context)
}

/// Build a [`FeedsPreview`] for an operation in a project context — the
/// preview counterpart of [`feeds_result_for_operation`] and the entry point
/// every apply surface should use.
///
/// Prefer this over [`feeds_explain_for_operation`] anywhere an Apply button
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
/// the pin clamp cannot be applied**, not that it was applied and found
/// nothing to do — the value is left at the unclamped Suggest default,
/// exactly as before, and generation's own emitter guard
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
            cfg.peck_depth = clamp_peck_to_depth(peck, cfg.depth);
        }
        OperationConfig::AlignmentPinDrill(cfg) => {
            // Same clamp, same ceiling factor, against the depth this
            // family will actually drill. Mirrors `generate_alignment_
            // pin_drill`'s `let depth = stock_z + cfg.spoilboard_
            // penetration;` — if that expression moves, this one has to
            // move with it.
            cfg.peck_depth = match stock {
                Some(ctx) => clamp_peck_to_depth(peck, ctx.stock_z + cfg.spoilboard_penetration),
                None => peck,
            };
        }
        _ => {}
    }
}

/// Ensure the Suggest-default peck depth still produces a real peck
/// cycle. If `peck >= depth` the operator gets a single-shot drill
/// with no chip evacuation; clamp to a fixed fraction of the hole
/// depth so the cycle always pecks at least twice. Chosen factor:
/// 0.75 — keeps the peck count low (2 pecks for typical holes) while
/// staying strictly below `drill_depth`.
fn clamp_peck_to_depth(peck: f64, drill_depth: f64) -> f64 {
    if !drill_depth.is_finite() || drill_depth <= 0.0 {
        return peck;
    }
    let ceiling = drill_depth * 0.75;
    peck.min(ceiling)
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
        OperationConfig::Profile(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Drill(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Pocket(cfg) => {
            cfg.depth = (ctx.stock_z * 0.5).min(5.0);
        }
        OperationConfig::Adaptive(cfg) => {
            cfg.depth = ctx.stock_z * 0.5;
        }
        _ => {}
    }
}

/// Extract operation-specific hints for the feeds calculator.
/// Returns `(axial_depth_hint, radial_width_hint, scallop_hint)`.
///
/// Thin tuple adapter over the registry-layer
/// [`OperationConfig::feeds_hints`] accessor (Phase 1, architectural
/// refactor T3) — the per-op decision table lives there as an exhaustive
/// match; the old `_ => (None, None, None)` wildcard is gone.
pub fn operation_feeds_hints(
    operation: &OperationConfig,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let hints = operation.feeds_hints();
    (
        hints.axial_depth_mm,
        hints.radial_width_mm,
        hints.target_scallop_mm,
    )
}
