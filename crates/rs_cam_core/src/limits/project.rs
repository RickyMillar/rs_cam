//! Feeds contract P29-2: the projections of the old records to
//! [`Check`]. Each old type stays the source in this step; no old type is
//! built from a `Check`.

use super::check::basis_of_bound;
use super::{BoundSide, Check, CheckState, DepthRule, LimitId, NoNumberReason};
use crate::feeds::RpmChoice;
use crate::feeds::envelope::{CutPoint, Edge, JudgedValue, LimitKind, SafeEnvelope};
use crate::tool_load::verdict::{
    BoundSource, ChipSide, CriterionKind, CriterionStatus, ExceededCriterion, LoadState,
};

/// The state of a value against a bound on `side`: by the margin, and
/// [`NoNumberReason::NoBound`] when the bound gives no margin.
fn judged(side: BoundSide, value: f64, bound: f64) -> (Option<f64>, CheckState) {
    match side.margin(value, bound) {
        Some(margin) => (Some(margin), CheckState::of_margin(margin)),
        None => (None, CheckState::NoNumber(NoNumberReason::NoBound)),
    }
}

/// The check of one gate row (the six gate verdicts and the drill gates
/// project to `CriterionStatus`, and that row projects here).
///
/// - The state is the gate's own (`LoadState`), never a second judgement:
///   the gate applies its tolerance. A row that is `Within` with no bound
///   (the depth of a finish pass) is [`NoNumberReason::NoBound`].
/// - A chip row that exceeds on the low side is [`LimitId::ChipMin`]; its
///   bound is the floor of the band (the old row holds the ceiling in
///   `bound` and the floor in its bound source).
#[must_use]
pub fn of_criterion(status: &CriterionStatus<'_>) -> Check {
    let chip_side = if status.kind == CriterionKind::Chipload
        && status
            .exceeded
            .as_ref()
            .is_some_and(|e| *e == ExceededCriterion::chipload_burn())
    {
        ChipSide::Low
    } else {
        ChipSide::High
    };
    let limit = LimitId::of_criterion(
        status.kind,
        DepthRule::of_bound(status.bound_source.as_ref()),
        chip_side,
    );
    let (side, bound) = match (chip_side, status.bound_source.as_ref()) {
        (
            ChipSide::Low,
            Some(BoundSource::VendorChipBand {
                floor_mm_per_tooth, ..
            }),
        ) => (BoundSide::Floor, *floor_mm_per_tooth),
        (ChipSide::Low | ChipSide::High, _) => (BoundSide::Ceiling, status.bound),
    };
    let margin = match (status.display_peak, bound) {
        (Some(value), Some(bound)) => side.margin(value, bound),
        (Some(_) | None, _) => None,
    };
    let state = match status.state {
        // EXEMPT(S-NoBound)
        LoadState::Within if bound.is_none() => CheckState::NoNumber(NoNumberReason::NoBound),
        LoadState::Within => CheckState::Within,
        LoadState::Exceeds => CheckState::Exceeds,
        LoadState::Unmodeled => match status.unmodeled_reason {
            Some(reason) => CheckState::of_unmodeled(reason),
            None => CheckState::NoNumber(NoNumberReason::NotModelled),
        },
    };
    Check {
        limit,
        value: status.display_peak,
        bound,
        unit: status.unit,
        side,
        margin,
        state,
        basis: status.bound_source.as_ref().map(basis_of_bound),
        bound_detail: status.bound_source.clone(),
        binding: false,
        population: status.population,
    }
}

/// The checks of the planned point `p` against the limits of `envelope`:
/// one check for each limit of the boundary, in boundary order, then one
/// for the exact move budget when the envelope judges it.
///
/// The margins, the states and the binding limit are the ones of
/// `SafeEnvelope::placement_of(p)` (`SafeEnvelope::placing_margins`). A
/// limit that the placement does not judge at `p` is
/// [`NoNumberReason::NotJudgedHere`], or [`NoNumberReason::NotModelled`]
/// for an edge the model abstains on.
#[must_use]
pub fn of_envelope(envelope: &SafeEnvelope, p: CutPoint) -> Vec<Check> {
    let placed = envelope.placing_margins(p);
    // The fold of `SafeEnvelope::place`: the lowest margin below the
    // tolerance; the first one wins a tie.
    let mut worst: Option<(usize, f64)> = None;
    for (k, (_, margin, _)) in placed.iter().enumerate() {
        if *margin < -1e-9 && worst.is_none_or(|(_, w)| *margin < w) {
            worst = Some((k, *margin));
        }
    }
    let binding_at = worst.map(|(k, _)| k);
    let mut out = Vec::with_capacity(envelope.boundary.len() + 1);
    for (index, limit) in envelope.boundary.iter().enumerate() {
        let side = if limit.edge.is_floor() {
            BoundSide::Floor
        } else {
            BoundSide::Ceiling
        };
        let reading = envelope.planned_reading(limit, p);
        let at = placed.iter().position(|(_, _, i)| *i == Some(index));
        let (margin, state) = match (at.and_then(|k| placed.get(k)), &limit.edge) {
            (Some((_, margin, _)), _) => (Some(*margin), CheckState::of_margin(*margin)),
            (None, Edge::Unmodelled { .. }) => {
                (None, CheckState::NoNumber(NoNumberReason::NotModelled))
            }
            // EXEMPT(S-NotJudgedHere)
            (
                None,
                Edge::ApCeiling { .. }
                | Edge::ApFloor { .. }
                | Edge::AeCeiling { .. }
                | Edge::AeFloor { .. }
                | Edge::ApCurve { .. }
                | Edge::AeCurve { .. }
                | Edge::PerPoint,
            ) => (None, CheckState::NoNumber(NoNumberReason::NotJudgedHere)),
        };
        out.push(Check {
            limit: LimitId::of_limit_kind(limit.kind, DepthRule::of_limit(limit)),
            value: reading.map(|(value, _)| value),
            bound: reading.map(|(_, bound)| bound),
            unit: "mm",
            side,
            margin,
            state,
            basis: Some(limit.basis.clone()),
            bound_detail: None,
            binding: at.is_some() && at == binding_at,
            population: None,
        });
    }
    // The exact move budget: a margin with no boundary index.
    for (k, (kind, margin, index)) in placed.iter().enumerate() {
        if index.is_some() {
            continue;
        }
        out.push(Check {
            limit: LimitId::of_limit_kind(*kind, DepthRule::ForceBudget),
            value: None,
            bound: None,
            unit: "moves",
            side: BoundSide::Ceiling,
            margin: Some(*margin),
            state: CheckState::of_margin(*margin),
            basis: envelope
                .limit(LimitKind::Runtime)
                .map(|limit| limit.basis.clone()),
            bound_detail: None,
            binding: Some(k) == binding_at,
            population: None,
        });
    }
    out
}

/// The check of one judged value of a measured point
/// (`SafeEnvelope::judged_value`) against `limit` on `side`.
#[must_use]
pub fn of_judged_value(limit: LimitId, side: BoundSide, judged_value: &JudgedValue) -> Check {
    let (margin, state) = judged(side, judged_value.measured, judged_value.limit);
    Check {
        limit,
        value: Some(judged_value.measured),
        bound: Some(judged_value.limit),
        unit: judged_value.unit,
        side,
        margin,
        state,
        basis: None,
        bound_detail: None,
        binding: false,
        population: None,
    }
}

/// The checks of an RPM choice: the chosen RPM against each named limit,
/// with `binding` on the limit that set the RPM. Each limit is a ceiling.
/// A limit that only stops the rise can be below a chart RPM, so a check
/// can be `Exceeds`.
#[must_use]
pub fn of_rpm_choice(choice: &RpmChoice) -> Vec<Check> {
    choice
        .limits
        .iter()
        .map(|entry| {
            let (margin, state) = judged(BoundSide::Ceiling, choice.rpm, entry.rpm);
            Check {
                limit: LimitId::of_rpm_limit(entry.limit),
                value: Some(choice.rpm),
                bound: Some(entry.rpm),
                unit: "rpm",
                side: BoundSide::Ceiling,
                margin,
                state,
                basis: None,
                bound_detail: None,
                binding: entry.limit == choice.binding,
                population: None,
            }
        })
        .collect()
}
