//! G-ENTRYEMPTY — the empty-generation gate.
//!
//! # The finding
//!
//! Observed live 2026-08-23 on `wanaka200`: an `adaptive3d` back-rough with
//! `entry_style = helix` generated a toolpath with **zero moves** and
//! reported **success**. Nothing gated it. The same gap let a heights-tab
//! mishap silently empty a healthy operation — 262 downstream rapid
//! collisions before a human noticed, because a `Done` op carrying 0 moves
//! is indistinguishable from a healthy one on *every* surface: the status
//! badge says `Done`, the diagnostics list is empty (there is no motion to
//! find issues in), the air-cut percentage is `NaN`-free zero, and the
//! simulator skips the entry entirely (`moves.len() < 2`) so no collision,
//! no engagement and no removed volume are attributed to it.
//!
//! The only pre-existing signal was
//! [`crate::compute::config::ToolpathStats::zero_removal`], which is
//! **report-only by operator ruling** (A4, `zero_removal_rest_pass_a4.rs`)
//! and — decisively — cannot fire on an empty toolpath at all: it measures
//! engagement of *emitted cutting geometry* against a reference stock, and
//! with no cutting moves it samples zero positions and records nothing
//! (X-19: an absent measurement is not a defect claim).
//!
//! # The ruling
//!
//! A generator that emits **0 cutting moves from a non-empty input region**
//! produces a typed refusal, not `Done`-with-0-moves. The refusal is a
//! genuine [`crate::session::SessionError::GeneratedEmpty`] /
//! `ComputeError::Message` — re-running will not fix it, so `generate_all`'s
//! fixpoint loop must not retry it, and it must NOT be confused with
//! [`crate::compute::config::AwaitingPriorStock`], which is a *sequencing*
//! state that the loop does retry.
//!
//! # What is deliberately NOT refused
//!
//! An empty result is legitimate whenever the *region* the operation was
//! pointed at can itself be empty. Five such cases, and each one is a
//! ruling rather than a convenience:
//!
//! * **Rest machining** ([`StockSource::FromRemainingStock`], or any op
//!   seeded with a machined-stock snapshot). Its whole job is to cut what
//!   an upstream op left; when the upstream genuinely left nothing there is
//!   nothing to do. Refusing here would wedge exactly the chains
//!   `generate_all(fixpoint: true)` exists to walk.
//! * **`Rest` operations.** `rest.rs` returns empty *by contract* when
//!   `tool_radius >= prev_tool_radius`. This is the same exemption
//!   `tests/adversarial_2d_campaign_r2.rs` already carves out of its
//!   silent-empty gate, and for the same geometric reason.
//! * **A `DerivedRestRegions` boundary.** That boundary source means "cut
//!   only what the named op left" — a rest-shaped intent expressed through
//!   the boundary rather than the stock source.
//! * **Drill cycles.** A drill's "region" is a hole list, not material;
//!   an `AlignmentPinDrill` on a stock with no pins configured yet is a
//!   normal intermediate state, not a failure. (Deliberate scope boundary:
//!   a hole-less drill cycle is arguably worth its own refusal, but that is
//!   a different gate with a different false-positive profile.)
//! * **Feature-selective operations** — `Pencil` (valleys and creases) and
//!   `HorizontalFinish` (near-flat areas). See
//!   [`feature_selective_exemption`] for the list and for why the families
//!   that merely *look* similar are gated.
//! * **No input region at all** — no mesh, no polygons, and not a
//!   stock-driven op. There is nothing for the generator to have failed at.
//!
//! # Where it is applied
//!
//! At the two — and only two — points where a finished generation becomes a
//! *persisted result*: [`crate::session::ProjectSession::generate_toolpath`]
//! and the GUI worker's `run_compute_with_phase_tracker`. Deliberately NOT
//! inside `compute::execute`: the individual generators are also called
//! directly by unit tests and by the strategy advisor's throwaway
//! candidates, where an empty return is data, not an outcome. The gate is
//! about what gets *cached and shown as Done*.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::StockSource;
use crate::toolpath::{MoveType, Toolpath};

/// Everything the gate needs about the generation's *inputs*, gathered at
/// the persist site where the finished toolpath and its inputs are both in
/// hand.
///
/// Both call sites build this from their own request/config shape; the
/// decision itself lives here exactly once so the session path and the GUI
/// worker path cannot drift (the failure mode this repo has hit repeatedly
/// — see `resolve_containment_polygon`'s S.9 note).
#[derive(Debug, Clone, Copy)]
pub struct EmptyGenerationInputs<'a> {
    /// Operator-facing name of the toolpath, for the refusal text.
    pub toolpath_name: &'a str,
    /// The operation as it was actually executed (post compute-time
    /// patching), so the label and op kind in the message are the real ones.
    pub operation: &'a OperationConfig,
    /// The configured stock source.
    pub stock_source: StockSource,
    /// `true` when generation was seeded with a machined-stock snapshot.
    /// Read *in addition to* [`Self::stock_source`] rather than derived from
    /// it: the two are set on different paths and either one being rest-shaped
    /// is enough to earn the exemption.
    pub seeded_machined_stock: bool,
    /// Whether a 3D mesh reached the generator.
    pub has_mesh: bool,
    /// How many 2D polygons reached the generator. `0` with no mesh and a
    /// non-stock-driven op means there was no region to cut.
    pub polygon_count: usize,
    /// Whether the enabled boundary's source is
    /// [`crate::compute::config::BoundarySource::DerivedRestRegions`].
    pub boundary_is_derived_rest_regions: bool,
}

/// Why an empty generation was let through.
///
/// Carried on [`EmptyGenerationVerdict::Legitimate`] so the call site can log
/// *which* exemption applied — an empty result that is expected still costs
/// the operator a wondering minute, and "which rule let this through" is the
/// cheapest thing to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegitimateEmptyReason {
    /// Seeded with (or configured for) a machined-stock snapshot.
    RestMachining,
    /// An `OperationType::Rest` op; `rest.rs` returns empty by contract.
    RestOperation,
    /// The enabled boundary is `DerivedRestRegions`.
    DerivedRestBoundary,
    /// A drill cycle: its region is a hole list, not material.
    DrillCycle,
    /// The operation targets a FEATURE of the surface rather than the
    /// surface, so "this surface has none" is a statement about the
    /// geometry, not a failure. See [`feature_selective_exemption`].
    FeatureSelectiveOperation,
    /// No mesh, no polygons, not stock-driven — nothing to cut.
    NoInputRegion,
}

impl LegitimateEmptyReason {
    /// One clause an operator can read, used in the log line.
    pub fn describe(self) -> &'static str {
        match self {
            Self::RestMachining => {
                "it reads remaining stock, and the upstream operations left nothing to cut"
            }
            Self::RestOperation => {
                "a rest operation returns empty by contract when its tool is not finer than the \
                 previous one"
            }
            Self::DerivedRestBoundary => {
                "its boundary is derived from another operation's rest regions, which can be empty"
            }
            Self::DrillCycle => "a drill cycle's region is a hole list, and it is empty",
            Self::FeatureSelectiveOperation => {
                "it targets a feature of the surface rather than the surface, and this surface \
                 has none of that feature"
            }
            Self::NoInputRegion => "no mesh or 2D geometry reached the generator",
        }
    }
}

/// The operations that target a FEATURE of the surface rather than the
/// surface itself, and are therefore allowed to find none of it.
///
/// This is the gate's one judgement call, so it is one function rather than
/// a condition buried in [`classify`]: when a family turns out to be
/// mis-classified, this is the list to edit.
///
/// * `Pencil` cuts concave valleys and creases. A surface without any is a
///   surface, not a broken generation.
/// * `HorizontalFinish` cuts near-flat areas only — `CLAUDE.md` states the
///   consequence outright ("useless on terrain"), so an operator pointing it
///   at terrain and getting nothing is meeting documented behaviour.
/// * `Waterline` cuts slopes steeper than its threshold only — a model with
///   no steep walls legitimately yields nothing
///   (`pencil_tip_float_channel_d1.rs` builds exactly that fixture). The
///   cost of this exemption is stated honestly: a standalone waterline
///   emptied by a mis-resolved Z window (the G-UNIFIEDBOTTOMZ shape) also
///   succeeds silently. The gate cannot tell "no steep slopes" from
///   "window missed the material" without slope analysis it does not have;
///   `UnifiedFinish` stays gated because ALL THREE of its bands empty at
///   once is not a slope story.
///
/// Everything else is gated, INCLUDING the families whose emptiness looks
/// similar but is not: `SteepShallow`, `Scallop`, `DropCutter`,
/// `UnifiedFinish` and the whole 2.5D set all cover the region they are
/// given, so an empty result from them is a failure to plan, not an absent
/// feature. `Adaptive3d` — the operation the live G-ENTRYEMPTY defect was
/// found on — is squarely in that gated set.
pub fn feature_selective_exemption(op_type: OperationType) -> bool {
    matches!(
        op_type,
        OperationType::Pencil
            | OperationType::HorizontalFinish
            | OperationType::Waterline
    )
}

/// A typed refusal: the operation had a region to cut and produced no
/// cutting motion at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEmptyRefusal {
    /// Operator-facing toolpath name.
    pub toolpath_name: String,
    /// Human label of the operation (`"Adaptive 3D"`, …).
    pub op_label: &'static str,
    /// Stable snake_case op kind, for machine consumers.
    pub op_kind: &'static str,
    /// Total move count, including rapids. `0` and "rapids only" are
    /// different failures and the operator should be able to tell them apart.
    pub total_move_count: usize,
}

impl std::fmt::Display for GeneratedEmptyRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "'{name}' ({label}) generated ZERO cutting moves from a non-empty input region \
             ({moves} move(s) emitted in total). That is a generation failure, not an empty \
             result: the operation had geometry to cut and produced no cutting motion at all. \
             Check the heights (top/bottom Z, depth per pass) — a cut window that misses the \
             material empties an otherwise healthy operation — then the machining boundary, the \
             stepover / stock-to-leave, and the entry style (a helix or ramp entry whose descent \
             cannot be placed leaves each pass with nowhere to start). Refusing rather than \
             recording success with no moves: a Done-with-0-moves operation looks healthy on \
             every surface, exports nothing, and lets the next operation's rest chain inherit \
             stock nobody cut. Operations that can legitimately be empty — rest machining, \
             `rest` ops, a derived-rest-regions boundary, drill cycles, the feature-selective \
             finishes (pencil, horizontal finish, waterline), and ops with no input region — are exempt \
             from this check and still succeed.",
            name = self.toolpath_name,
            label = self.op_label,
            moves = self.total_move_count,
        )
    }
}

/// The gate's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmptyGenerationVerdict {
    /// The toolpath carries cutting motion; nothing to say.
    NotEmpty,
    /// Empty, and one of the ruled-legitimate cases applies.
    Legitimate(LegitimateEmptyReason),
    /// Empty from a non-empty region — refuse.
    Refuse(GeneratedEmptyRefusal),
}

impl EmptyGenerationVerdict {
    /// The refusal, when there is one. `None` covers both "not empty" and
    /// "legitimately empty" — the two outcomes a caller treats identically.
    pub fn refusal(self) -> Option<GeneratedEmptyRefusal> {
        match self {
            Self::Refuse(r) => Some(r),
            Self::NotEmpty | Self::Legitimate(_) => None,
        }
    }
}

/// Number of moves that actually cut — everything that is not a `Rapid`.
///
/// Same population `Toolpath::total_cutting_distance` integrates over
/// (`Linear | ArcCW | ArcCCW`), and the same one
/// `tests/adversarial_2d_campaign_r2.rs`'s silent-empty gate counts, so the
/// campaign's verdict and this one cannot disagree about what "empty" means.
pub fn cutting_move_count(toolpath: &Toolpath) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .count()
}

/// Classify a finished generation. See the module doc for the ruling and
/// for every exemption.
pub fn classify(toolpath: &Toolpath, inputs: &EmptyGenerationInputs<'_>) -> EmptyGenerationVerdict {
    if cutting_move_count(toolpath) > 0 {
        return EmptyGenerationVerdict::NotEmpty;
    }

    let op_type = inputs.operation.op_type();

    if matches!(
        op_type,
        OperationType::Drill | OperationType::AlignmentPinDrill
    ) {
        return EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::DrillCycle);
    }
    if inputs.seeded_machined_stock || inputs.stock_source == StockSource::FromRemainingStock {
        return EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::RestMachining);
    }
    if op_type == OperationType::Rest {
        return EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::RestOperation);
    }
    if inputs.boundary_is_derived_rest_regions {
        return EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::DerivedRestBoundary);
    }
    if feature_selective_exemption(op_type) {
        return EmptyGenerationVerdict::Legitimate(
            LegitimateEmptyReason::FeatureSelectiveOperation,
        );
    }

    let has_region =
        inputs.has_mesh || inputs.polygon_count > 0 || inputs.operation.is_stock_based();
    if !has_region {
        return EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::NoInputRegion);
    }

    EmptyGenerationVerdict::Refuse(GeneratedEmptyRefusal {
        toolpath_name: inputs.toolpath_name.to_owned(),
        op_label: inputs.operation.label(),
        op_kind: op_type.kind_str(),
        total_move_count: toolpath.moves.len(),
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::operation_configs::{Adaptive3dConfig, DrillConfig, PocketConfig};
    use crate::geo::P3;

    fn empty_tp() -> Toolpath {
        Toolpath::new()
    }

    fn rapids_only_tp() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.rapid_to(P3::new(10.0, 0.0, 5.0));
        tp
    }

    fn cutting_tp() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 500.0);
        tp
    }

    fn inputs<'a>(op: &'a OperationConfig, name: &'a str) -> EmptyGenerationInputs<'a> {
        EmptyGenerationInputs {
            toolpath_name: name,
            operation: op,
            stock_source: StockSource::Fresh,
            seeded_machined_stock: false,
            has_mesh: true,
            polygon_count: 0,
            boundary_is_derived_rest_regions: false,
        }
    }

    #[test]
    fn a_toolpath_with_cutting_moves_is_never_refused() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let verdict = classify(&cutting_tp(), &inputs(&op, "Back Rough"));
        assert_eq!(verdict, EmptyGenerationVerdict::NotEmpty);
    }

    #[test]
    fn rapids_without_cutting_are_empty_and_refused() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let refusal = classify(&rapids_only_tp(), &inputs(&op, "Back Rough"))
            .refusal()
            .expect("rapids-only from a meshed region must refuse");
        assert_eq!(refusal.total_move_count, 2);
        assert_eq!(refusal.op_kind, "adaptive3d");
        assert!(refusal.to_string().contains("Back Rough"));
    }

    #[test]
    fn a_zero_move_generation_from_a_mesh_refuses() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let refusal = classify(&empty_tp(), &inputs(&op, "Back Rough"))
            .refusal()
            .expect("0 moves from a meshed region must refuse");
        assert_eq!(refusal.total_move_count, 0);
    }

    #[test]
    fn rest_machining_stays_legitimate() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let mut i = inputs(&op, "Back Rough");
        i.stock_source = StockSource::FromRemainingStock;
        assert_eq!(
            classify(&empty_tp(), &i),
            EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::RestMachining)
        );

        let mut i = inputs(&op, "Back Rough");
        i.seeded_machined_stock = true;
        assert_eq!(
            classify(&empty_tp(), &i),
            EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::RestMachining)
        );
    }

    #[test]
    fn a_derived_rest_regions_boundary_stays_legitimate() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let mut i = inputs(&op, "Detail");
        i.boundary_is_derived_rest_regions = true;
        assert_eq!(
            classify(&empty_tp(), &i),
            EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::DerivedRestBoundary)
        );
    }

    #[test]
    fn a_drill_cycle_with_no_holes_stays_legitimate() {
        let op = OperationConfig::Drill(DrillConfig::default());
        let mut i = inputs(&op, "Pins");
        i.has_mesh = false;
        assert_eq!(
            classify(&empty_tp(), &i),
            EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::DrillCycle)
        );
    }

    #[test]
    fn a_feature_selective_op_stays_legitimate_and_adaptive3d_does_not() {
        assert!(feature_selective_exemption(OperationType::Pencil));
        assert!(feature_selective_exemption(OperationType::HorizontalFinish));
        // Slope-selective: a model with no steep walls is a legitimate empty
        // (pencil_tip_float_channel_d1.rs builds exactly that fixture).
        assert!(feature_selective_exemption(OperationType::Waterline));
        // The operation the live defect was found on must stay gated, or
        // this whole gate is decorative.
        assert!(!feature_selective_exemption(OperationType::Adaptive3d));
        assert!(!feature_selective_exemption(OperationType::Pocket));
    }

    #[test]
    fn no_region_at_all_stays_legitimate() {
        let op = OperationConfig::Pocket(PocketConfig::default());
        let mut i = inputs(&op, "Pocket");
        i.has_mesh = false;
        i.polygon_count = 0;
        assert_eq!(
            classify(&empty_tp(), &i),
            EmptyGenerationVerdict::Legitimate(LegitimateEmptyReason::NoInputRegion)
        );
    }

    #[test]
    fn a_2d_op_with_polygons_refuses() {
        let op = OperationConfig::Pocket(PocketConfig::default());
        let mut i = inputs(&op, "Pocket");
        i.has_mesh = false;
        i.polygon_count = 3;
        assert!(classify(&empty_tp(), &i).refusal().is_some());
    }

    /// The refusal text must never contain the word "cancel": the
    /// adversarial campaign's cancellation sentry classifies an outcome by
    /// searching the error string for it, and a refusal that borrowed the
    /// word would read as an honoured cancel flag.
    #[test]
    fn the_refusal_text_cannot_be_mistaken_for_a_cancellation() {
        let op = OperationConfig::Adaptive3d(Adaptive3dConfig::default());
        let refusal = classify(&empty_tp(), &inputs(&op, "Back Rough"))
            .refusal()
            .expect("refusal");
        assert!(!refusal.to_string().to_ascii_lowercase().contains("cancel"));
    }
}
