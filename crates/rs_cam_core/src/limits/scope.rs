//! Feeds contract P29-6 6d and P29-7 7c: the one scope of every limit
//! over every cut group, before and after the simulation.
//!
//! [`scope`] answers, for each [`LimitId`] and each [`CutGroup`], whether
//! the limit judges the cuts of that group: the declared cuts before the
//! simulation (`feeds::planned::judge_declared_cuts`, P29-7) and the
//! samples after it (`tool_load::groups::judge_by_group`, P29-6). It is one
//! match with no wildcard arm, so a new limit or a new group does not
//! compile until it answers every cell. No limit decides its own scope. A
//! cell that is not judged names its row of [`super::EXEMPTIONS`] and
//! carries the marker of that row on the line above it; the sentry
//! `tests/the_exemption_list_only_shrinks_p29_3.rs` holds the link. A limit
//! that does not change with the cut answers [`Scope::NotACutLimit`] with
//! its [`PlanReason`].

use super::{CutGroup, LimitId};

/// A cell that a limit does not judge: the exemption row and the clause
/// that the verdict prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotJudged {
    /// The id of the row of `EXEMPTIONS`.
    pub row: &'static str,
    /// The clause for the operator (the reason of `NotApplicableForOp`).
    pub reason: &'static str,
}

/// Why a limit is not a limit of each cut (P29-7 7c). A closed list: a new
/// limit that does not change with the width or the chip of a cut picks one
/// reason, or it answers each group, and the compiler asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanReason {
    /// The bound reads the depth alone (the flute guard, the cutting
    /// length, the vendor axial bound): every declared cut at one depth
    /// gives the same answer.
    DepthOnly,
    /// A rule of the body plan (the corner coverage, the move budget, the
    /// engagement floor, the tool diameter, the scallop depth).
    PlanLevel,
    /// A limit of the machine, of the RPM choice or of the feed.
    MachineOrRpm,
    /// A gate of the drill cycle, which reads the cycle and not a cut.
    DrillGate,
}

/// The answer of [`scope`] for one limit and one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The limit judges the cuts of the group: each declared cut before the
    /// simulation, each sample after it.
    Judged,
    /// The limit does not judge the cuts of this group.
    NotJudged(NotJudged),
    /// The limit is not a limit of each cut: its bound does not change with
    /// the width or the chip of the cut, for the reason given. It needs no
    /// row, and no sample gate judges it.
    NotACutLimit(PlanReason),
}

/// The clause of a drill cycle (the four side gates printed it before
/// P29-6 as an early return by op kind).
const DRILL_CYCLE: &str = "drill cycle — no continuous engagement";

/// The scope of `limit` over the samples of `group`. See the module doc.
#[must_use]
pub const fn scope(limit: LimitId, group: CutGroup) -> Scope {
    match limit {
        LimitId::ChipMax | LimitId::ChipMin => match group {
            CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => Scope::Judged,
            // EXEMPT(P28-20)
            CutGroup::Drill => Scope::NotJudged(NotJudged {
                row: "P28-20",
                reason: DRILL_CYCLE,
            }),
        },
        LimitId::SpindlePower => match group {
            CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => Scope::Judged,
            // EXEMPT(P28-10)
            CutGroup::Drill => Scope::NotJudged(NotJudged {
                row: "P28-10",
                reason: DRILL_CYCLE,
            }),
        },
        LimitId::ToolDeflection => match group {
            CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => Scope::Judged,
            // EXEMPT(P28-17)
            CutGroup::Drill => Scope::NotJudged(NotJudged {
                row: "P28-17",
                reason: DRILL_CYCLE,
            }),
        },
        LimitId::ForceBudget => match group {
            CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => Scope::Judged,
            // EXEMPT(P28-04)
            CutGroup::Drill => Scope::NotJudged(NotJudged {
                row: "P28-04",
                reason: DRILL_CYCLE,
            }),
        },
        // P29-7 7c: a chip rule reads the chip of the cut (a slot has the
        // largest chip share), so it is a limit of each cut.
        LimitId::RubbingFloor | LimitId::VendorDepthTable => match group {
            CutGroup::Body | CutGroup::Entry | CutGroup::Cleanup | CutGroup::Link => Scope::Judged,
            // EXEMPT(P28-20)
            CutGroup::Drill => Scope::NotJudged(NotJudged {
                row: "P28-20",
                reason: DRILL_CYCLE,
            }),
        },
        // P29-8e (c): the vertical rate of each cut; a cut with no vertical
        // feed answers `NotApplicable(NoVerticalFeed)`.
        LimitId::PlungeRate => match group {
            CutGroup::Body
            | CutGroup::Entry
            | CutGroup::Cleanup
            | CutGroup::Link
            | CutGroup::Drill => Scope::Judged,
        },
        LimitId::FluteGuard | LimitId::CuttingLength | LimitId::VendorAxial => {
            Scope::NotACutLimit(PlanReason::DepthOnly)
        }
        LimitId::CornerCoverage
        | LimitId::ScallopDepth
        | LimitId::ToolDiameter
        | LimitId::MoveBudget
        | LimitId::EngagementFloor => Scope::NotACutLimit(PlanReason::PlanLevel),
        LimitId::CuttingFeedCap
        | LimitId::PlannerFeedCap
        | LimitId::KinematicReach
        | LimitId::SpindleMaxRpm
        | LimitId::ZAxisRate
        | LimitId::OperatorRpmCeiling
        | LimitId::ChartRpm
        | LimitId::VendorRpmMax
        | LimitId::GantryPush => Scope::NotACutLimit(PlanReason::MachineOrRpm),
        LimitId::DrillChipWelding | LimitId::DrillPeckAdequacy | LimitId::DrillPlungeFeed => {
            Scope::NotACutLimit(PlanReason::DrillGate)
        }
    }
}

/// The scope of a gate that judges every limit of `limits` at once (the
/// chip gate judges [`LimitId::ChipMax`] and [`LimitId::ChipMin`] in one
/// walk): the first cell that is not judged, else judged. A limit that is
/// not a limit of each cut gives [`Scope::NotACutLimit`]; the sentry holds
/// that no gate names one.
#[must_use]
pub fn scope_of_all(limits: &[LimitId], group: CutGroup) -> Scope {
    let mut out = Scope::Judged;
    for &limit in limits {
        match scope(limit, group) {
            Scope::Judged => {}
            other @ (Scope::NotJudged(_) | Scope::NotACutLimit(_)) => {
                out = other;
                break;
            }
        }
    }
    out
}
