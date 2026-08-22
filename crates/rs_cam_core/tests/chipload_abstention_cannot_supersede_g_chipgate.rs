//! **G-CHIPGATE-POPULATION, re-diagnosed.** The chipload gate does *not*
//! pass on an empty band — and this file is the pin that says so, plus the
//! pin on the thing that would have made the original fear real.
//!
//! # What was reported, and what was actually true
//!
//! Raised 2026-08-22 from a live session: a pre-simulation heuristic said
//! `Feed 3000 mm/min is 6.3x recommendation (473) — tool breakage risk` on a
//! Ø1.0 tapered ball, and after simulation that caution was **superseded by
//! `load.chipload.within`**. The conclusion drawn at the time was that the
//! gate had returned `Within` over an empty vendor band — a fifth instance of
//! this programme's recurring "a gate handed an empty population passes and
//! looks healthy" failure.
//!
//! That conclusion was wrong, in two separable ways, and both are worth
//! writing down because each is a mechanism a future reader will re-derive:
//!
//! 1. **The gate already refuses.** `tool_load::chipload::evaluate` returns
//!    `Unmodeled(NoVendorData)` when `matched_chip_envelope` finds no row, and
//!    again when the matched row's bounds fail `derate_chipload_bounds`. Every
//!    path that reaches `Within` has a band in hand. There is no empty-band
//!    `Within` to fix.
//! 2. **The probe that "showed" no band asked the wrong resolver.** It called
//!    `feeds::calculate`, which resolves through
//!    `find_best_row_for_geometry` — the *recipe* resolver, which lets
//!    RPM-only anchors win and then publishes no band. The gate resolves
//!    through `find_best_chip_envelope_row`, which excludes exactly those
//!    rows. The two disagreeing on a tapered ball is not a gate defect; it is
//!    the asymmetry that P1 (`rubbing_floor_envelope_band_p1.rs`) addresses on
//!    the Suggest side.
//!
//! # The thing that *would* have been the defect
//!
//! An abstention is published under the id `LOAD_CHIPLOAD_WITHIN` — the same
//! id a real pass uses — and it carries a populated `supersedes` list naming
//! the `feeds.*_vs_lut.*` heuristics. If the supersession reducer keyed on the
//! id alone, an abstention really would silently delete the breakage caution,
//! which is the failure the report described even though it is not the one
//! that happened.
//!
//! It does not, because the reducer is **state-gated**: only a
//! `DiagnosticState::Current` diagnostic supersedes, and every abstention
//! carries `NeedsSimulation`, `StaleEvidence` or `NotApplicable`. That is a
//! two-place invariant — the adapter must not mark an abstention `Current`,
//! and the reducer must not stop checking — so it is pinned here rather than
//! left as a comment at either end.
//!
//! # What remains open
//!
//! Why a commanded 3000 mm/min on that tool produced a genuine `Within` is
//! **not** answered here and is not answerable from this crate's fixtures.
//! The gate observes the *achieved* advance per tooth
//! (`effective_feed / (rpm · flutes)`, from the kinematics-predicted feed),
//! while the pre-sim heuristic compares the *commanded* feed to a LUT
//! recommendation. On short finishing moves with a small tool the predicted
//! feed can be a fraction of the commanded one, in which case both surfaces
//! are right about different quantities. That is a hypothesis, not a finding.
//! The probe that settles it is one call —
//! `get_tool_load_report().per_toolpath[].chipload` carries the observed
//! advance beside the band — and it needs the live project. Recorded in
//! `planning/airrun_2026-08-19/RUN_LOG.md` under G-CHIPGATE-POPULATION.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::supersession::apply_supersession;
use rs_cam_core::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::tool_load::verdict::{
    ChiploadVerdict, DeflectionVerdict, PowerVerdict, ToolpathLoadVerdict, UnmodeledReason,
};

const TP: ToolpathId = ToolpathId(15);

/// A verdict whose chipload gate abstained for the stated reason, with the
/// other gates abstaining too so nothing else contributes a `Current` row.
fn abstaining_verdict(reason: UnmodeledReason) -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: TP,
        chipload: ChiploadVerdict::Unmodeled { reason },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
    }
}

/// The pre-simulation caution the operator actually saw, as the reducer sees
/// it: a Current-state heuristic that the chipload family claims to supersede.
fn breakage_caution() -> Diagnostic {
    Diagnostic {
        id: DiagnosticId::from(ids::FEEDS_FEED_VS_LUT_HIGH),
        scope: Scope::Toolpath { id: TP },
        category: Category::ToolLoad,
        severity: Severity::Info,
        confidence: Confidence::Static,
        state: DiagnosticState::Current,
        source: Source::FeedsCalculator,
        message: "Feed 3000 mm/min is 6.3x recommendation (473) — tool breakage risk".to_owned(),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    }
}

/// Every `Unmodeled` reason the chipload gate can produce must publish a
/// **non-Current** diagnostic. This is the adapter half of the invariant.
#[test]
fn every_chipload_abstention_is_published_non_current() {
    let reasons = [
        UnmodeledReason::NoVendorData,
        UnmodeledReason::SimulationRequired,
        UnmodeledReason::SteadyStateSamplesNotPresent,
        UnmodeledReason::AllSamplesAirCutOrRapid,
        UnmodeledReason::StaleSimulation,
        UnmodeledReason::MaterialUnvalidated,
        UnmodeledReason::ArcEngagementNotCaptured,
    ];
    let mut seen = 0;
    for reason in reasons {
        let rows = diagnostics_from_load_verdict(&abstaining_verdict(reason.clone()));
        let chip = rows
            .iter()
            .find(|d| d.id.0 == ids::LOAD_CHIPLOAD_WITHIN)
            .unwrap_or_else(|| panic!("no chipload row published for abstention {reason:?}"));
        seen += 1;
        assert_ne!(
            chip.state,
            DiagnosticState::Current,
            "abstention {reason:?} published a CURRENT diagnostic under the \
             `within` id — the reducer keys on exactly that, so this would let \
             an abstention delete the pre-sim breakage caution"
        );
    }
    // Non-vacuity: the loop must have actually inspected rows.
    assert_eq!(seen, 7, "every reason must produce a chipload row");
}

/// The reducer half: an abstention sitting next to the breakage caution
/// leaves it standing. This is the operator-visible statement of the whole
/// file — the caution the person needs to read does not disappear because a
/// gate could not evaluate.
#[test]
fn an_abstention_does_not_delete_the_pre_sim_breakage_caution() {
    let mut rows = vec![breakage_caution()];
    rows.extend(diagnostics_from_load_verdict(&abstaining_verdict(
        UnmodeledReason::NoVendorData,
    )));

    // Non-vacuity: both halves must be present before the reducer runs, or
    // "it survived" is a statement about an empty input.
    assert!(
        rows.iter().any(|d| d.id.0 == ids::FEEDS_FEED_VS_LUT_HIGH),
        "fixture must contain the caution"
    );
    assert!(
        rows.iter().any(|d| d.id.0 == ids::LOAD_CHIPLOAD_WITHIN),
        "fixture must contain the abstention"
    );

    let after = apply_supersession(rows);
    assert!(
        after.iter().any(|d| d.id.0 == ids::FEEDS_FEED_VS_LUT_HIGH),
        "the breakage caution was deleted by an ABSTAINING chipload gate — \
         the reducer must stay state-gated"
    );
}

/// And the control: a *modelled* `Within` is allowed to supersede, because it
/// carries evidence the heuristic does not. Without this arm the test above
/// would pass just as well if supersession were removed entirely, which is
/// the vacuous version of the same claim.
#[test]
fn a_modelled_within_still_supersedes_the_heuristic() {
    let mut rows = vec![breakage_caution()];
    let within = Diagnostic {
        id: DiagnosticId::from(ids::LOAD_CHIPLOAD_WITHIN),
        scope: Scope::Toolpath { id: TP },
        category: Category::ToolLoad,
        severity: Severity::Info,
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::ToolLoad,
        message: "Chipload within band".to_owned(),
        evidence: None,
        fix: None,
        supersedes: vec![DiagnosticId::from(ids::FEEDS_FEED_VS_LUT_HIGH)],
        suppressed_diagnostics: vec![],
    };
    rows.push(within);

    let after = apply_supersession(rows);
    assert!(
        !after.iter().any(|d| d.id.0 == ids::FEEDS_FEED_VS_LUT_HIGH),
        "a Current, modelled chipload verdict must still supersede the \
         pre-sim heuristic — otherwise the previous test proves nothing"
    );
    let survivor = after
        .iter()
        .find(|d| d.id.0 == ids::LOAD_CHIPLOAD_WITHIN)
        .expect("the within row survives");
    assert!(
        survivor
            .suppressed_diagnostics
            .iter()
            .any(|s| s.0 == ids::FEEDS_FEED_VS_LUT_HIGH),
        "the survivor must record what it silenced, so a missing heuristic is \
         explainable rather than merely absent"
    );
}
