//! Feeds contract P29-6 6d: the one scope of every limit over every cut
//! group.
//!
//! [`scope`] answers, for each [`LimitId`] and each [`CutGroup`], whether
//! a sample gate judges the limit on the samples of that group. It is one
//! match with no wildcard arm, so a new limit or a new group does not
//! compile until it answers every cell. Every gate asks it through the one
//! driver (`tool_load::groups::judge_by_group`); no gate decides its own
//! scope. A cell that is not judged names its row of
//! [`super::EXEMPTIONS`] and carries the marker of that row on the line
//! above it; the sentry `tests/the_exemption_list_only_shrinks_p29_3.rs`
//! holds the link.

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

/// The answer of [`scope`] for one limit and one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// A sample gate judges the limit on the samples of the group.
    Judged,
    /// A sample gate does not judge the limit on this group.
    NotJudged(NotJudged),
    /// No sample gate judges this limit: it is a limit of the plan before
    /// the simulation (the envelope, Suggest, the RPM choice), of the feed
    /// modulator, or of the drill gates, which read the drill cycle and
    /// not the samples. It needs no row.
    NotASampleLimit,
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
        LimitId::RubbingFloor
        | LimitId::FluteGuard
        | LimitId::CuttingLength
        | LimitId::VendorAxial
        | LimitId::VendorDepthTable
        | LimitId::CornerCoverage
        | LimitId::ScallopDepth
        | LimitId::ToolDiameter
        | LimitId::MoveBudget
        | LimitId::EngagementFloor
        | LimitId::CuttingFeedCap
        | LimitId::PlannerFeedCap
        | LimitId::KinematicReach
        | LimitId::PlungeRate
        | LimitId::SpindleMaxRpm
        | LimitId::ZAxisRate
        | LimitId::OperatorRpmCeiling
        | LimitId::ChartRpm
        | LimitId::VendorRpmMax
        | LimitId::GantryPush
        | LimitId::DrillChipWelding
        | LimitId::DrillPeckAdequacy
        | LimitId::DrillPlungeFeed => Scope::NotASampleLimit,
    }
}

/// The scope of a gate that judges every limit of `limits` at once (the
/// chip gate judges [`LimitId::ChipMax`] and [`LimitId::ChipMin`] in one
/// walk): the first cell that is not judged, else judged. A limit that no
/// sample gate judges gives [`Scope::NotASampleLimit`]; the sentry holds
/// that no gate names one.
#[must_use]
pub fn scope_of_all(limits: &[LimitId], group: CutGroup) -> Scope {
    let mut out = Scope::Judged;
    for &limit in limits {
        match scope(limit, group) {
            Scope::Judged => {}
            other @ (Scope::NotJudged(_) | Scope::NotASampleLimit) => {
                out = other;
                break;
            }
        }
    }
    out
}
