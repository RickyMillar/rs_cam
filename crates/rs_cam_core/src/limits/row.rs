//! Feeds contract P29-10: [`CheckRow`], THE display row of a check, and the
//! projections that build it.
//!
//! A row holds one [`Check`] and its subject (a declared cut, the stated
//! point, the samples of one cut group, the RPM choice), its caveats, its
//! exemption row, its tone and its text. Every surface reads rows. No
//! surface computes a margin, a percent, a worst row or a state word.
//!
//! Only core builds a row: the constructors are `pub(crate)`. The envelope
//! writes its planned rows and its stated rows ([`CheckRow::planned`],
//! [`CheckRow::stated`]); [`rows_of_verdict`] and [`rows_of_rpm_choice`]
//! project the gate verdicts and the RPM choice. Each projection is a match
//! with no wildcard and a destructure with no `..`, so a new limit, cut
//! group, gate, reason or caveat does not compile until it states its row.

use serde::Serialize;

use super::{
    BoundSide, Check, CheckState, CutGroup, CutKind, EXEMPTIONS, Geometry, LimitId, NoNumberReason,
    SampleDepth, of_criterion, of_rpm_choice,
};
use crate::feeds::RpmChoice;
use crate::tool_load::groups::Gate;
use crate::tool_load::verdict::{
    AdvisoryLowSide, CriterionKind, CriterionStatus, ToolpathLoadVerdict,
};
use crate::tool_load::{GroupCell, MaterialSplit, OwnMaterialVerdict};

/// The longest face (characters): the face limit of every short panel
/// text (`diagnostics::FACE_MAX_CHARS`, design rule 6, IC1).
pub const ROW_FACE_MAX_CHARS: usize = crate::diagnostics::FACE_MAX_CHARS;

/// P29-10z (lead ruling T): the title of the stated-point rows of the
/// limits that are not limits of each cut, in part one of the drawer
/// ([`Subject::Stated`]), in the style of a cut title.
pub const STATED_POINT_TITLE: &str = "the stated point";

/// What a row is about.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    /// A declared cut before the simulation, at the proposal (P29-7 7c).
    /// A body, slot or cleanup row reads the proposal speeds (P29-9
    /// ruling); an entry row reads the stored entry feed over the stored
    /// speed (P29-8 8d). `at`: the point of the cut on the width x depth
    /// plane (P29-10 10g), `None` for a cut with no width or no depth there
    /// (a plunge, a cut with no width rule).
    Planned {
        /// The declared cut.
        #[serde(serialize_with = "cut_label")]
        cut: CutKind,
        /// Whether the plan always cuts it.
        occurs: Occurs,
        /// The plane point of the cut (P29-10 10g). An entry at its own
        /// depth holds that depth here (`own_depth` true).
        at: Option<PlanePoint>,
    },
    /// P29-10 10i (lead ruling L1): a declared cut at the STATED point (the
    /// cut as the op stands), judged by the same driver as a planned cut at
    /// the proposal (`SafeEnvelope::read_declared_cuts`, one judge, two
    /// points): the stated depth and width, the stored op feed over the
    /// stated speed (`StoredFeedChip`), the entry rule's feeds. `at`: the
    /// point of the cut on the plane at the stated point.
    StatedCut {
        /// The declared cut.
        #[serde(serialize_with = "cut_label")]
        cut: CutKind,
        /// Whether the plan always cuts it.
        occurs: Occurs,
        /// The plane point of the cut at the stated point.
        at: Option<PlanePoint>,
    },
    /// The operator's stated point against one limit of the boundary that
    /// is not a limit of each cut (`of_envelope`): the cut as the op stands.
    Stated,
    /// The samples of one cut group after the simulation.
    Group {
        /// The group.
        group: CutGroup,
        /// `true` for the group of the op's own cut (the headline fields).
        headline: bool,
        /// The samples of the group that remove material. `None` for a
        /// headline cell (the check population holds its count).
        samples: Option<usize>,
        /// `None` for the stock population; the material label of an S6
        /// own-material verdict.
        material: Option<String>,
    },
    /// The worst sample placed on the envelope (the lineage, K7).
    MeasuredPoint,
    /// The RPM choice.
    Rpm,
}

/// P29-10 10g: the point of a planned cut on the width x depth plane: its
/// width and its depth. One fact, one field (lead ruling R5): the own depth
/// of an entry is the depth of its point, and `own_depth` says that the
/// depth is the cut's own, not the depth of the body plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PlanePoint {
    /// The width (mm).
    pub ae_mm: f64,
    /// The depth (mm).
    pub ap_mm: f64,
    /// `true` when the depth is the cut's own depth (an entry), not the
    /// depth of the body plane.
    pub own_depth: bool,
}

fn cut_label<S: serde::Serializer>(cut: &CutKind, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(cut.label())
}

impl Subject {
    /// The subject in words, for the face.
    fn label(&self) -> String {
        match self {
            Self::Planned { cut, .. } | Self::StatedCut { cut, .. } => cut.label().to_owned(),
            Self::Stated => "stated point".to_owned(),
            Self::Group {
                group,
                material: None,
                ..
            } => group.label().to_owned(),
            Self::Group {
                group,
                material: Some(m),
                ..
            } => format!("{} ({m})", group.label()),
            Self::MeasuredPoint => "measured point".to_owned(),
            Self::Rpm => "rpm".to_owned(),
        }
    }

    /// The rank for [`CheckRow::worse_than`]: the stated point is the cut
    /// of today, so it outranks every other subject (ruling O6).
    const fn rank(&self) -> u8 {
        match self {
            Self::Stated | Self::StatedCut { .. } => 0,
            Self::Planned { .. } | Self::Group { .. } | Self::MeasuredPoint | Self::Rpm => 1,
        }
    }
}

/// What a row compares (lead ruling 3): the face names the quantity that
/// the value and the bound are. A planned force-budget row and the depth
/// gate row compare a depth with the depth where the force meets the
/// budget, so their face says "depth" and names the force cap as the
/// cause; only a row that compares the limit's own quantity (newtons for
/// the force budget) shows a share of that quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compared {
    /// The limit's own quantity: power, chip, deflection, force, feed, RPM,
    /// moves.
    Load,
    /// A depth against the depth where the limit binds.
    Depth,
    /// A width against the width where the limit binds.
    Width,
}

impl Compared {
    /// The compared quantity of a stated row: the axis of its edge.
    #[must_use]
    pub const fn of_edge(edge: &crate::feeds::envelope::Edge) -> Self {
        use crate::feeds::envelope::Edge;
        match edge {
            Edge::ApCeiling { .. } | Edge::ApFloor { .. } | Edge::ApCurve { .. } => Self::Depth,
            Edge::AeCeiling { .. } | Edge::AeFloor { .. } | Edge::AeCurve { .. } => Self::Width,
            Edge::PerPoint | Edge::Unmodelled { .. } => Self::Load,
        }
    }

    /// The compared quantity of a judged value of a measured point
    /// (`JudgedValue::axis`: "depth", "width" or "power").
    #[must_use]
    pub fn of_judged_axis(axis: &str) -> Self {
        match axis {
            "depth" => Self::Depth,
            "width" => Self::Width,
            _ => Self::Load,
        }
    }

    /// The compared quantity of a gate row: the depth gate compares the
    /// sample depth with its cap; every other gate compares its own load.
    #[must_use]
    pub const fn of_criterion(kind: CriterionKind) -> Self {
        match kind {
            CriterionKind::DepthOfCut => Self::Depth,
            CriterionKind::Chipload
            | CriterionKind::Power
            | CriterionKind::Deflection
            | CriterionKind::GantryPush
            | CriterionKind::DrillChipWelding
            | CriterionKind::DrillPeckAdequacy
            | CriterionKind::DrillPlungeFeed => Self::Load,
        }
    }

    /// The word of the compared quantity on a face, or `None` for the
    /// limit's own quantity (the face then names the limit).
    const fn word(self) -> Option<&'static str> {
        match self {
            Self::Load => None,
            Self::Depth => Some("depth"),
            Self::Width => Some("width"),
        }
    }
}

/// `compute::catalog::Presence` seen from `limits/` (invariant 1): the
/// exemption row of a cut that may not occur is in [`Caveat::MayOccur`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Occurs {
    /// The plan always cuts it.
    Always,
    /// The plan cuts it only where a generation fact asks.
    MayOccur,
}

/// A fact of the record that qualifies the row. The list is closed; each
/// caveat has one clause ([`Self::clause`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caveat {
    /// P29-10z (lead ruling Q2), P30-2: the row reads the stated feed, and
    /// the op holds a CURRENT modulated result: the simulation's feed
    /// modulation re-timed the moves of the cut and capped them at the
    /// bound of this limit (`dressup::feed_modulation::RunModulation::caps`,
    /// the modulator's declared list), so the export carries the capped
    /// feeds. The row keeps its true value and state; the caveat softens
    /// the tone.
    BeforeModulation,
    /// P30-2 (lead ruling 2026-11-14): the row reads the stated feed, and
    /// the op holds NO current modulated result (no simulation, a planned
    /// result, a superseded result). A simulation with feed modulation
    /// would re-time the moves of the cut and cap them at this bound, but
    /// an export now carries `export`. The caveat does not soften the tone.
    UntilModulated {
        /// What an export of the op carries now.
        export: ExportFeed,
    },
    /// An entry at its own depth: the depth per pass of a ramp, the worst
    /// case (lead ruling Q1b of P29-8).
    WorstCase,
    /// The value is a lower bound of the cut (`BodyDepth::AtLeast`).
    LowerBound,
    /// The chip reads a stored whole-mm/min feed (`StoredFeedChip`) and the
    /// chip interval of that feed holds the bound: the rounding of the feed
    /// can change the state (lead ruling 2). A row far from its bound has
    /// no such caveat.
    StoredFeed,
    /// P29-10 PB (lead ruling on the point band): the chip floor of the
    /// row is a reference, not a floor that gates: the low side of its
    /// vendor row is advisory by THE one classification of the gate
    /// (`ChipBoundsSource::advisory_low_side`), for the reason named.
    AdvisoryFloor {
        /// Why the low side is advisory.
        why: AdvisoryLowSide,
    },
    /// The cut may not occur (`Presence::MayOccur`), with its row.
    MayOccur {
        /// The exemption row of the cut.
        row: &'static str,
    },
    /// The bound does not gate an export (`Basis::gates_export` false).
    Advisory,
    /// The gate stated an empty population (X-VAC).
    NoSamples,
    /// M4: the sample depth that the row reads.
    SampleDepth {
        /// The depth.
        depth: SampleDepth,
    },
    /// A finish pass judged at the earlier op's leave
    /// (`BodyDepth::EarlierLeave`).
    AtEarlierLeave,
    /// P29-10 10i (lead ruling 5): the op states no feed or speed, so a
    /// depth ceiling at the stated point reads the chip of the drawn edge
    /// (the suggested chip).
    AtSuggestedChip,
}

impl Caveat {
    /// The order of the caveats on a row.
    const fn rank(self) -> u8 {
        match self {
            Self::BeforeModulation | Self::UntilModulated { .. } => 0,
            Self::WorstCase => 1,
            Self::LowerBound => 2,
            Self::StoredFeed => 3,
            Self::AdvisoryFloor { .. } => 4,
            Self::MayOccur { .. } => 5,
            Self::Advisory => 6,
            Self::NoSamples => 7,
            Self::SampleDepth { .. } => 8,
            Self::AtEarlierLeave => 9,
            Self::AtSuggestedChip => 10,
        }
    }

    /// The word on the face, for a caveat that the face names.
    const fn face_word(self) -> Option<&'static str> {
        match self {
            // P29-10z: the face holds it; the longest face with the word on
            // the four project files is 59 characters.
            Self::BeforeModulation => Some("before modulation"),
            // P30-2: 15 characters, so every face that fits with "before
            // modulation" (17) fits with it.
            Self::UntilModulated { .. } => Some("until modulated"),
            Self::WorstCase => Some("worst case"),
            Self::LowerBound => Some("lower bound"),
            Self::StoredFeed => Some("stored feed"),
            Self::MayOccur { .. } => Some("may not occur"),
            // P29-10 10e (the words judged on the screenshots): the tone
            // states an advisory bound (`Tone::ExceedsAdvisory`, its chip
            // word "advisory"); on a row inside its bound the word says
            // nothing to act on. The detail keeps the clause. P29-10 PB: an
            // advisory chip floor the same (the word clipped the drawer row
            // at the rail width, 340 points); the detail names the reason.
            Self::AdvisoryFloor { .. }
            | Self::Advisory
            | Self::NoSamples
            | Self::SampleDepth { .. }
            | Self::AtEarlierLeave
            | Self::AtSuggestedChip => None,
        }
    }

    /// `true` for a caveat of the cut, not of the row: the own depth of a
    /// ramp and the presence of the cut (P29-10 10f: a cut title names it).
    const fn of_the_cut(self) -> bool {
        match self {
            Self::WorstCase | Self::MayOccur { .. } => true,
            Self::BeforeModulation
            | Self::UntilModulated { .. }
            | Self::LowerBound
            | Self::StoredFeed
            | Self::AdvisoryFloor { .. }
            | Self::Advisory
            | Self::NoSamples
            | Self::SampleDepth { .. }
            | Self::AtEarlierLeave
            | Self::AtSuggestedChip => false,
        }
    }

    /// `true` when an `Exceeds` row under this caveat is a caution, not a
    /// danger (ruling O2: the caveat is a fact of the record).
    const fn softens(self) -> bool {
        match self {
            Self::BeforeModulation
            | Self::WorstCase
            | Self::LowerBound
            | Self::StoredFeed
            | Self::AdvisoryFloor { .. }
            | Self::MayOccur { .. } => true,
            // P30-2: the export carries the stated feed, so the row keeps
            // its gating tone.
            Self::UntilModulated { .. }
            | Self::Advisory
            | Self::NoSamples
            | Self::SampleDepth { .. }
            | Self::AtEarlierLeave
            | Self::AtSuggestedChip => false,
        }
    }

    /// The word of a row under the title of its cut (the drawer,
    /// [`CheckRow::face_in_group`]). P29-10z: "before modulation" is not
    /// there: the row is clipped at the rail width (340 points) with the
    /// word, as the advisory-floor word was in PB; the chip word and the
    /// hover carry it.
    const fn group_word(self) -> Option<&'static str> {
        match self {
            Self::BeforeModulation | Self::UntilModulated { .. } => None,
            Self::WorstCase
            | Self::LowerBound
            | Self::StoredFeed
            | Self::AdvisoryFloor { .. }
            | Self::MayOccur { .. }
            | Self::Advisory
            | Self::NoSamples
            | Self::SampleDepth { .. }
            | Self::AtEarlierLeave
            | Self::AtSuggestedChip => self.face_word(),
        }
    }

    /// The caveat in one clause, for the detail.
    #[must_use]
    pub fn clause(self) -> String {
        match self {
            Self::BeforeModulation => {
                "before modulation: the simulation re-times the moves of this cut and caps them \
                 at this bound; the row reads the stated feed"
                    .to_owned()
            }
            Self::UntilModulated {
                export: ExportFeed::Stated,
            } => "until modulated: a simulation with feed modulation re-times the moves of this \
                  cut and caps them at this bound; an export now carries the stated feed"
                .to_owned(),
            Self::UntilModulated {
                export: ExportFeed::OlderModulation,
            } => "until modulated: a current simulation with feed modulation re-times the moves \
                  of this cut and caps them at this bound; an export now carries the feeds of an \
                  older simulation"
                .to_owned(),
            Self::WorstCase => {
                "worst case: the ramp is judged at its full depth per pass".to_owned()
            }
            Self::LowerBound => "the value is a lower bound of the cut".to_owned(),
            Self::StoredFeed => {
                "the bound is inside the chip interval of the stored whole-mm/min feed: \
                 the rounding of the feed decides the state"
                    .to_owned()
            }
            Self::AdvisoryFloor { why } => format!(
                "the chip floor is a reference, not a floor that gates: {}",
                advisory_floor_clause(why)
            ),
            Self::MayOccur { row } => format!("the plan may not cut it (row {row})"),
            Self::Advisory => "the bound does not gate an export".to_owned(),
            Self::NoSamples => "the gate read no sample".to_owned(),
            Self::SampleDepth {
                depth: SampleDepth::RemovedLayer,
            } => "the sample depth read is the removed layer depth".to_owned(),
            Self::SampleDepth {
                depth: SampleDepth::EngagedEdge,
            } => "the sample depth read is the engaged edge depth".to_owned(),
            Self::AtEarlierLeave => "judged at the leave of the earlier op".to_owned(),
            Self::AtSuggestedChip => {
                "the op states no feed or speed: the depth is judged at the suggested chip of \
                 the drawn edge"
                    .to_owned()
            }
        }
    }
}

/// The clause of an advisory chip floor, for the detail.
const fn advisory_floor_clause(why: AdvisoryLowSide) -> &'static str {
    match why {
        AdvisoryLowSide::OnePrintedValue => {
            "the vendor row prints one chip value, a point and not a band (A2)"
        }
        AdvisoryLowSide::Extrapolated => {
            "the vendor row is extrapolated to this diameter or hardness"
        }
        AdvisoryLowSide::NoWidthWindow => "the vendor row prints no width (ae) window",
    }
}

/// P30-2: what an export of an op carries now, for a row that a
/// simulation with feed modulation would cap (`Caveat::UntilModulated`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFeed {
    /// The stated feed (the planned result, or no result yet).
    Stated,
    /// The feeds of an older simulation: the stock before the op changed
    /// since the modulation (a superseded result).
    OlderModulation,
}

/// P29-10z (lead ruling Q2), P30-2: whether the feed modulation caps the
/// bound of a planned row on its cut (`RunModulation::caps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowModulation {
    /// No modulation re-times the cut at this bound: the row reads the feed
    /// that the machine runs.
    NotCapped,
    /// The current modulated result re-timed the moves of the cut and
    /// capped them at this bound: the row reads the stated feed, before
    /// modulation (`Caveat::BeforeModulation`, it softens).
    CapsTheBound,
    /// No current modulated result: a simulation with feed modulation would
    /// cap the cut at this bound, and the export carries `export` now
    /// (`Caveat::UntilModulated`, it does not soften).
    WouldCapTheBound {
        /// What an export carries now.
        export: ExportFeed,
    },
}

/// P29-10 10i (lead ruling 5): the chip a depth ceiling row reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingChipSource {
    /// The chip of its point (the proposal speeds, the stated op feed), or
    /// a row that is no depth ceiling.
    OfThePoint,
    /// The op states no feed or speed: the suggested chip of the drawn
    /// edge (`Caveat::AtSuggestedChip`).
    Suggested,
}

/// P29-10 10i (lead ruling L1): the point at which a declared cut is
/// judged: one judge, two points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgedAt {
    /// The proposal that ⚡ writes (part two of the drawer).
    Proposal,
    /// The cut as the op stands (part one of the drawer).
    Stated,
}

/// The tone of a row: what a surface colours it. One rule ([`Tone::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Inside the bound.
    Within,
    /// Outside a bound that gates an export, with no caveat that softens it.
    Exceeds,
    /// Outside an advisory bound, or outside under a caveat that softens it.
    ExceedsAdvisory,
    /// No judgement.
    NoNumber,
    /// The limit has no meaning on the cut.
    NotApplicable,
}

impl Tone {
    /// THE tone rule. `Exceeds` softens to [`Self::ExceedsAdvisory`] when
    /// the bound does not gate an export or a caveat among
    /// `BeforeModulation`, `WorstCase`, `LowerBound`, `StoredFeed`,
    /// `AdvisoryFloor`, `MayOccur` holds. No threshold: a caveat
    /// is a fact of the record. No tone refuses (K6).
    #[must_use]
    pub fn of(check: &Check, caveats: &[Caveat]) -> Self {
        match check.state {
            CheckState::Within => Self::Within,
            CheckState::NoNumber(_) => Self::NoNumber,
            CheckState::NotApplicable(_) => Self::NotApplicable,
            CheckState::Exceeds => {
                let advisory = check.basis.as_ref().is_some_and(|b| !b.gates_export());
                if advisory || caveats.iter().any(|c| c.softens()) {
                    Self::ExceedsAdvisory
                } else {
                    Self::Exceeds
                }
            }
        }
    }

    /// The tone in words, for the detail.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Within => "within",
            Self::Exceeds => "exceeds",
            Self::ExceedsAdvisory => "exceeds (advisory)",
            Self::NoNumber => "no number",
            Self::NotApplicable => "not applicable",
        }
    }

    /// The tone in one short word, for a status chip. An exceeding row
    /// whose bound does not gate an export, or that a caveat softens,
    /// reads "advisory": the face already says "over" or "under".
    #[must_use]
    pub const fn chip_word(self) -> &'static str {
        match self {
            Self::Within => "within",
            Self::Exceeds => "exceeds",
            Self::ExceedsAdvisory => "advisory",
            Self::NoNumber => "no number",
            Self::NotApplicable => "N/A",
        }
    }

    /// `true` for a row outside its bound (either exceed tone).
    #[must_use]
    pub const fn exceeds(self) -> bool {
        match self {
            Self::Exceeds | Self::ExceedsAdvisory => true,
            Self::Within | Self::NoNumber | Self::NotApplicable => false,
        }
    }

    /// The tone group of [`CheckRow::rank_cmp`]: `Exceeds`, then
    /// `ExceedsAdvisory`, then every other tone.
    const fn rank(self) -> u8 {
        match self {
            Self::Exceeds => 0,
            Self::ExceedsAdvisory => 1,
            Self::Within | Self::NoNumber | Self::NotApplicable => 2,
        }
    }
}

/// The facts of a planned row that the envelope knows and `limits/` does
/// not (invariant 1: no `compute/` type crosses).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedFacts {
    /// The exemption row of a cut that the plan may not cut.
    pub may_occur_row: Option<&'static str>,
    /// The own depth of an entry.
    pub own_depth: Option<PlannedOwnDepth>,
    /// The value is a lower bound of the cut.
    pub lower_bound: bool,
    /// The chip reads a stored entry feed whose interval holds the bound.
    pub stored_feed: bool,
    /// P29-10 PB: the chip floor is a reference, with the reason.
    pub advisory_floor: Option<AdvisoryLowSide>,
    /// P29-10z (lead ruling Q2): whether the run's feed modulation re-times
    /// the moves of the cut and caps them at the bound of this limit
    /// (`RunModulation::caps`). Only a row that holds a number takes the
    /// caveat.
    pub modulation: RowModulation,
    /// P29-10 10i: the point the row judges (the proposal or the stated
    /// cut).
    pub judged_at: JudgedAt,
    /// P29-10 10i (ruling 5): the chip a depth ceiling reads.
    pub ceiling_chip: CeilingChipSource,
    /// P29-10 10i (ruling 3): the exemption row of a cell that the scope
    /// does not judge (`limits::scope`, `NotJudged::row`).
    pub scope_row: Option<&'static str>,
    /// The row reads the earlier op's leave.
    pub at_earlier_leave: bool,
    /// Why the entry rule gives the cut no feed (`EntryNoNumber::clause`).
    pub entry_no_number: Option<&'static str>,
    /// What the row compares (lead ruling 3).
    pub compared: Compared,
    /// P29-10 10g: the point of the cut on the plane.
    pub at: Option<PlanePoint>,
}

/// The own depth of a planned entry row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedOwnDepth {
    /// The depth (mm).
    pub mm: f64,
    /// The rule of the depth in words (`OwnDepthRule::clause`).
    pub rule: &'static str,
    /// The depth is the worst case (a ramp's depth per pass).
    pub worst_case: bool,
}

/// THE display row (P29-10): one check and what a reader needs to show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckRow {
    subject: Subject,
    check: Check,
    compared: Compared,
    caveats: Vec<Caveat>,
    exemption: Option<&'static str>,
    tone: Tone,
    face: String,
    /// The face without its subject and without a caveat of the cut (P29-10
    /// 10f: a row under the title of its cut). Not on the wire: `face`
    /// holds the whole line.
    #[serde(skip)]
    face_in_group: String,
    detail: String,
}

impl CheckRow {
    /// The one builder: the tone, the exemption, the face and the detail
    /// come from the check, the subject and the caveats.
    fn new(
        subject: Subject,
        check: Check,
        compared: Compared,
        mut caveats: Vec<Caveat>,
        extra: &[String],
        own_row: Option<&'static str>,
    ) -> Self {
        if let Some(basis) = &check.basis
            && !basis.gates_export()
            && !caveats.contains(&Caveat::Advisory)
        {
            caveats.push(Caveat::Advisory);
        }
        if check.population.is_some_and(|p| p.is_vacuous()) {
            caveats.push(Caveat::NoSamples);
        }
        caveats.sort_by_key(|c| c.rank());
        caveats.dedup();
        // P29-10 10f: a check never holds a NaN or an infinity (`judged`
        // and `of_rpm_choice` give `None` in their place).
        debug_assert!(
            [check.value, check.bound, check.margin]
                .iter()
                .flatten()
                .all(|v| v.is_finite()),
            "a check holds finite numbers: {check:?}"
        );
        let exemption = own_row.or_else(|| exemption_of(&check, &caveats));
        let tone = Tone::of(&check, &caveats);
        let face = face_of(&subject, &check, compared, &caveats);
        let face_in_group = group_face_of(&subject, &check, compared, &caveats);
        let detail = detail_of(&subject, &check, compared, &caveats, exemption, tone, extra);
        Self {
            subject,
            check,
            compared,
            caveats,
            exemption,
            tone,
            face,
            face_in_group,
            detail,
        }
    }

    /// A declared cut before the simulation (the envelope), at the proposal
    /// or at the stated point (`PlannedFacts::judged_at`).
    #[must_use]
    pub(crate) fn planned(cut: CutKind, check: Check, facts: PlannedFacts) -> Self {
        let PlannedFacts {
            may_occur_row,
            own_depth,
            lower_bound,
            stored_feed,
            advisory_floor,
            modulation,
            judged_at,
            ceiling_chip,
            scope_row,
            at_earlier_leave,
            entry_no_number,
            compared,
            at,
        } = facts;
        let mut caveats = Vec::new();
        let holds_a_number = matches!(check.state, CheckState::Within | CheckState::Exceeds);
        match modulation {
            RowModulation::CapsTheBound if holds_a_number => {
                caveats.push(Caveat::BeforeModulation);
            }
            RowModulation::WouldCapTheBound { export } if holds_a_number => {
                caveats.push(Caveat::UntilModulated { export });
            }
            RowModulation::CapsTheBound
            | RowModulation::WouldCapTheBound { .. }
            | RowModulation::NotCapped => {}
        }
        if own_depth.is_some_and(|o| o.worst_case) {
            caveats.push(Caveat::WorstCase);
        }
        if lower_bound {
            caveats.push(Caveat::LowerBound);
        }
        if stored_feed {
            caveats.push(Caveat::StoredFeed);
        }
        if let Some(why) = advisory_floor {
            caveats.push(Caveat::AdvisoryFloor { why });
        }
        if let Some(row) = may_occur_row {
            caveats.push(Caveat::MayOccur { row });
        }
        if at_earlier_leave {
            caveats.push(Caveat::AtEarlierLeave);
        }
        match ceiling_chip {
            CeilingChipSource::OfThePoint => {}
            CeilingChipSource::Suggested => caveats.push(Caveat::AtSuggestedChip),
        }
        let mut extra = Vec::new();
        if let Some(o) = own_depth {
            extra.push(format!("own depth {} mm ({})", num(o.mm), o.rule));
        }
        if let Some(clause) = entry_no_number {
            extra.push(format!("no entry feed: {clause}"));
        }
        let occurs = if may_occur_row.is_some() {
            Occurs::MayOccur
        } else {
            Occurs::Always
        };
        // P29-10 10i: one builder for both points; the subject names the
        // point.
        let subject = match judged_at {
            JudgedAt::Proposal => Subject::Planned { cut, occurs, at },
            JudgedAt::Stated => Subject::StatedCut { cut, occurs, at },
        };
        Self::new(subject, check, compared, caveats, &extra, scope_row)
    }

    /// The stated point against one limit (`of_envelope`).
    #[must_use]
    pub(crate) fn stated(check: Check, compared: Compared) -> Self {
        Self::new(Subject::Stated, check, compared, Vec::new(), &[], None)
    }

    /// The worst measured sample placed on the envelope (the lineage,
    /// K7): its value against the limit that it crosses.
    #[must_use]
    pub(crate) fn measured(check: Check, compared: Compared) -> Self {
        Self::new(
            Subject::MeasuredPoint,
            check,
            compared,
            Vec::new(),
            &[],
            None,
        )
    }

    /// The subject.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// The check.
    #[must_use]
    pub const fn check(&self) -> &Check {
        &self.check
    }

    /// What the row compares (lead ruling 3).
    #[must_use]
    pub const fn compared(&self) -> Compared {
        self.compared
    }

    /// The caveats, in rank order.
    #[must_use]
    pub fn caveats(&self) -> &[Caveat] {
        &self.caveats
    }

    /// The `EXEMPTIONS` row that holds a `NoBound`, `NotJudgedHere` or
    /// not-judged group row, else the row of a cut that may not occur.
    #[must_use]
    pub const fn exemption(&self) -> Option<&'static str> {
        self.exemption
    }

    /// The tone.
    #[must_use]
    pub const fn tone(&self) -> Tone {
        self.tone
    }

    /// One line, at most [`ROW_FACE_MAX_CHARS`] characters.
    #[must_use]
    pub fn face(&self) -> &str {
        &self.face
    }

    /// The face without its subject (P29-10 10f): the row under a title
    /// that names its cut or its part ("depth 387 % over force cap"). A
    /// caveat of the cut ([`Self::cut_words`]) is in the title, not here;
    /// a caveat of the row ("lower bound", "stored feed") stays.
    #[must_use]
    pub fn face_in_group(&self) -> &str {
        &self.face_in_group
    }

    /// The face words of the caveats that belong to the cut, not to the
    /// row: "worst case" (the own depth of a ramp) and "may not occur" (the
    /// presence of the cut). A title that groups the rows of one cut names
    /// them once (P29-10 10f).
    #[must_use]
    pub fn cut_words(&self) -> Vec<&'static str> {
        self.caveats
            .iter()
            .filter(|c| c.of_the_cut())
            .filter_map(|c| c.face_word())
            .collect()
    }

    /// `true` when the row holds a judgement: a state `Within` or
    /// `Exceeds`. A row with no number (`NoNumber`) or no meaning
    /// (`NotApplicable`) is `false`; a surface may fold it (P29-10 10f).
    #[must_use]
    pub const fn holds_a_number(&self) -> bool {
        match self.check.state {
            CheckState::Within | CheckState::Exceeds => true,
            CheckState::NoNumber(_) | CheckState::NotApplicable(_) => false,
        }
    }

    /// The words of a row with no judgement ("not modelled", "not judged
    /// here", "no vertical feed"), for a line that counts such rows. `None`
    /// for a row that [`Self::holds_a_number`].
    #[must_use]
    pub const fn no_number_words(&self) -> Option<&'static str> {
        match self.check.state {
            CheckState::NoNumber(reason) => Some(reason.face()),
            CheckState::NotApplicable(geometry) => Some(geometry.face()),
            CheckState::Within | CheckState::Exceeds => None,
        }
    }

    /// The hover: every clause of the row.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// The declared cut of a planned row.
    #[must_use]
    pub const fn cut(&self) -> Option<CutKind> {
        match self.subject {
            Subject::Planned { cut, .. } | Subject::StatedCut { cut, .. } => Some(cut),
            Subject::Stated | Subject::Group { .. } | Subject::MeasuredPoint | Subject::Rpm => None,
        }
    }

    /// The plane point of a planned row (P29-10 10g).
    #[must_use]
    pub const fn plane_point(&self) -> Option<PlanePoint> {
        match self.subject {
            Subject::Planned { at, .. } | Subject::StatedCut { at, .. } => at,
            Subject::Stated | Subject::Group { .. } | Subject::MeasuredPoint | Subject::Rpm => None,
        }
    }

    /// The own depth of a planned entry row (mm): the depth of its plane
    /// point when that depth is the cut's own (lead ruling R5).
    #[must_use]
    pub const fn own_depth_mm(&self) -> Option<f64> {
        match self.plane_point() {
            Some(PlanePoint {
                ap_mm,
                own_depth: true,
                ..
            }) => Some(ap_mm),
            Some(PlanePoint {
                own_depth: false, ..
            })
            | None => None,
        }
    }

    /// The exemption row of a cut that may not occur.
    #[must_use]
    pub fn may_occur_row(&self) -> Option<&'static str> {
        self.caveats.iter().find_map(|c| match c {
            Caveat::MayOccur { row } => Some(*row),
            Caveat::BeforeModulation
            | Caveat::UntilModulated { .. }
            | Caveat::WorstCase
            | Caveat::LowerBound
            | Caveat::StoredFeed
            | Caveat::AdvisoryFloor { .. }
            | Caveat::Advisory
            | Caveat::NoSamples
            | Caveat::SampleDepth { .. }
            | Caveat::AtEarlierLeave
            | Caveat::AtSuggestedChip => None,
        })
    }

    /// `true` for a gate row of the op's own cut group (the headline
    /// fields of the verdict, an own-material row included). The default
    /// view of the simulation surfaces shows these rows and every other
    /// group row that exceeds (ruling Q5).
    #[must_use]
    pub const fn is_headline(&self) -> bool {
        match self.subject {
            Subject::Group { headline, .. } => headline,
            Subject::Planned { .. }
            | Subject::StatedCut { .. }
            | Subject::Stated
            | Subject::MeasuredPoint
            | Subject::Rpm => false,
        }
    }

    /// `true` for a row of the default view of the simulation surfaces
    /// (ruling Q5, design 3.7): a headline row or a row that exceeds. A
    /// `NotApplicable` row (a milling gate on a drill cycle) is only in the
    /// full list.
    #[must_use]
    pub const fn in_default_view(&self) -> bool {
        match self.tone {
            Tone::NotApplicable => false,
            Tone::Exceeds | Tone::ExceedsAdvisory => true,
            Tone::Within | Tone::NoNumber => self.is_headline(),
        }
    }

    /// `true` when the row has no number for a reason that a run or a
    /// datum can fill (a simulation, a vendor row, a force line, an arc).
    /// A known absence (a limit not modelled, no bound on this cut, a cell
    /// not judged here) is not open. A surface that prompts an action reads
    /// it.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        match self.check.state {
            CheckState::NoNumber(reason) => reason.is_open(),
            CheckState::Within | CheckState::Exceeds | CheckState::NotApplicable(_) => false,
        }
    }

    /// `true` for a row whose gate read no sample (X-VAC).
    #[must_use]
    pub fn is_vacuous(&self) -> bool {
        self.caveats.contains(&Caveat::NoSamples)
    }

    /// `true` for a planned row: a declared cut at the PROPOSAL (part two
    /// of the Suggest drawer, "If you apply").
    #[must_use]
    pub const fn is_planned(&self) -> bool {
        match self.subject {
            Subject::Planned { .. } => true,
            Subject::StatedCut { .. }
            | Subject::Stated
            | Subject::Group { .. }
            | Subject::MeasuredPoint
            | Subject::Rpm => false,
        }
    }

    /// P29-10 10i: `true` for a row of the cut as the op stands (part one
    /// of the Suggest drawer): a declared cut at the stated point, or the
    /// stated point against a limit that is not a limit of each cut.
    #[must_use]
    pub const fn is_stated(&self) -> bool {
        match self.subject {
            Subject::StatedCut { .. } | Subject::Stated => true,
            Subject::Planned { .. }
            | Subject::Group { .. }
            | Subject::MeasuredPoint
            | Subject::Rpm => false,
        }
    }

    /// THE rank of two rows (ruling O6, lead rulings after 10d): an
    /// `Exceeds` row before an `ExceedsAdvisory` row before every other
    /// row; inside one tone group a row of a cut that always occurs before
    /// a row with the caveat "may not occur" (P29-10z, lead ruling R: the
    /// certain cut ranks first), then the stated point (it is the cut of
    /// today), then the lower margin (a row with no margin last). A total
    /// preorder: two rows with equal keys rank equal.
    #[must_use]
    pub fn rank_cmp(&self, other: &Self) -> std::cmp::Ordering {
        let key = |r: &Self| {
            (
                r.tone.rank(),
                u8::from(r.may_occur_row().is_some()),
                r.subject.rank(),
            )
        };
        key(self).cmp(&key(other)).then_with(|| {
            let m = |r: &Self| r.check.margin.unwrap_or(f64::INFINITY);
            m(self).total_cmp(&m(other))
        })
    }

    /// `true` when this row ranks before `other` ([`Self::rank_cmp`]).
    #[must_use]
    pub fn worse_than(&self, other: &Self) -> bool {
        self.rank_cmp(other) == std::cmp::Ordering::Less
    }
}

/// P29-10 10g (item 4c): the rows of one cut in words, for ONE closed line
/// of a drawer ("6 within · 1 no number"). `None` when a row exceeds: such
/// a cut stays open. The rows that hold a number count by their tone word;
/// the rows with no number count by their shared reason ("7 not
/// modelled"), else as "no number". The words come from the tone and the
/// reason of each row; a surface writes no state word of its own.
#[must_use]
pub fn quiet_summary(rows: &[&CheckRow]) -> Option<String> {
    if rows.iter().any(|row| row.tone.exceeds()) {
        return None;
    }
    let within = rows.iter().filter(|row| row.holds_a_number()).count();
    let none: Vec<&'static str> = rows
        .iter()
        .filter_map(|row| row.no_number_words())
        .collect();
    let mut parts = Vec::new();
    if within > 0 {
        parts.push(format!("{within} {}", Tone::Within.chip_word()));
    }
    if let Some(first) = none.first() {
        let word = if none.iter().all(|w| w == first) {
            *first
        } else {
            Tone::NoNumber.chip_word()
        };
        parts.push(format!("{} {word}", none.len()));
    }
    Some(parts.join(" \u{00B7} "))
}

/// The exemption row of a check: the row of a scope state first, else the
/// row of a cut that may not occur.
fn exemption_of(check: &Check, caveats: &[Caveat]) -> Option<&'static str> {
    let state_row = match check.state {
        // EXEMPT(S-NoBound)
        CheckState::NoNumber(NoNumberReason::NoBound) => Some("S-NoBound"),
        // P29-11 11-3: a not-judged scope cell names its own row (the
        // caller passes it); no stated-point row is `NotJudgedHere` now.
        CheckState::NoNumber(
            NoNumberReason::NotJudgedHere
            | NoNumberReason::JudgedPerCut
            | NoNumberReason::NoForceLine
            | NoNumberReason::NoSimulation
            | NoNumberReason::StaleSimulation
            | NoNumberReason::NoVendorRow
            | NoNumberReason::NoMaterialRemoved
            | NoNumberReason::NoArc
            | NoNumberReason::NoToolBody
            | NoNumberReason::NotModelled
            | NoNumberReason::OnlyALowerBound
            | NoNumberReason::NoEarlierLeave
            | NoNumberReason::NoStatedFeed,
        )
        | CheckState::Within
        | CheckState::Exceeds
        | CheckState::NotApplicable(_) => None,
    };
    state_row.or_else(|| {
        caveats.iter().find_map(|c| match c {
            Caveat::MayOccur { row } => Some(*row),
            Caveat::BeforeModulation
            | Caveat::UntilModulated { .. }
            | Caveat::WorstCase
            | Caveat::LowerBound
            | Caveat::StoredFeed
            | Caveat::AdvisoryFloor { .. }
            | Caveat::Advisory
            | Caveat::NoSamples
            | Caveat::SampleDepth { .. }
            | Caveat::AtEarlierLeave
            | Caveat::AtSuggestedChip => None,
        })
    })
}

/// A number for the detail: four decimals, trailing zeros cut.
fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// A percent for the face: 0 or 1 decimal (the kit units rule,
/// `ui/components/CLAUDE.md`: "% 0-1"). A share that is not zero and
/// prints under 0.1 % reads "< 0.1", never "0".
fn pct(share: f64) -> String {
    let p = (share * 100.0).abs();
    if p == 0.0 {
        "0".to_owned()
    } else if p < 0.1 {
        "< 0.1".to_owned()
    } else if p < 10.0 {
        format!("{p:.1}")
    } else {
        format!("{p:.0}")
    }
}

/// THE margin face of a check. The face follows the state (lead ruling
/// after 10e): a ceiling inside reads its share of the limit ("87 % of
/// limit"), outside its share over ("12 % over limit"); a floor that holds
/// reads its share of the floor ("333 % of floor"), one that fails its
/// share under ("40 % under floor"). A `Within` row whose value is past
/// the bound inside the gate tolerance reads "at limit" (or "at floor"),
/// and so does an `Exceeds` row whose margin is not negative: a `Within`
/// face never says "over" or "under", an `Exceeds` face never says only
/// "of". The detail holds the exact numbers. A row that compares a depth
/// or a width names the limit as the cause of its cap ("% of force cap").
/// `None` with no margin.
#[must_use]
pub fn margin_face(check: &Check, compared: Compared) -> Option<String> {
    let m = check.margin?;
    let (ceiling, floor) = match compared.word() {
        None => ("limit".to_owned(), "floor".to_owned()),
        Some(_) => {
            let short = check.limit.text().short;
            (format!("{short} cap"), format!("{short} floor"))
        }
    };
    let past = m < 0.0;
    let judged = match check.state {
        CheckState::Within => Some(false),
        CheckState::Exceeds => Some(true),
        CheckState::NoNumber(_) | CheckState::NotApplicable(_) => None,
    };
    Some(match (check.side, past, judged) {
        // The face follows the state where the state and the sign differ.
        (BoundSide::Ceiling, true, Some(false)) | (BoundSide::Ceiling, false, Some(true)) => {
            format!("at {ceiling}")
        }
        (BoundSide::Floor, true, Some(false)) | (BoundSide::Floor, false, Some(true)) => {
            format!("at {floor}")
        }
        (BoundSide::Ceiling, false, Some(false) | None) => {
            format!("{} % of {ceiling}", pct(1.0 - m))
        }
        (BoundSide::Ceiling, true, Some(true) | None) => format!("{} % over {ceiling}", pct(m)),
        (BoundSide::Floor, false, Some(false) | None) => format!("{} % of {floor}", pct(1.0 + m)),
        (BoundSide::Floor, true, Some(true) | None) => format!("{} % under {floor}", pct(m)),
    })
}

/// The state of a check in short words, when no margin speaks for it.
fn state_face(check: &Check) -> &'static str {
    match check.state {
        CheckState::Within => "inside, no bound here",
        CheckState::Exceeds => "outside",
        CheckState::NoNumber(reason) => reason.face(),
        CheckState::NotApplicable(geometry) => geometry.face(),
    }
}

/// The reading of a row: the limit or the compared quantity, then the
/// margin face or the state face. P29-10 10f: an RPM limit that admits
/// every RPM (no bound) says so in plain words.
fn reading_of(subject: &Subject, check: &Check, compared: Compared) -> String {
    let short = compared.word().unwrap_or(check.limit.text().short);
    let reading = margin_face(check, compared).unwrap_or_else(|| {
        match (subject, check.state, check.bound) {
            (Subject::Rpm, CheckState::Within, None) => "does not limit the RPM",
            _ => state_face(check),
        }
        .to_owned()
    });
    format!("{short} {reading}")
}

/// The face of a row under the title of its cut or part (P29-10 10f): the
/// reading, then the first word of a caveat of the row (a caveat of the
/// cut is in the title).
fn group_face_of(
    subject: &Subject,
    check: &Check,
    compared: Compared,
    caveats: &[Caveat],
) -> String {
    let reading = reading_of(subject, check, compared);
    let word = caveats
        .iter()
        .filter(|c| !c.of_the_cut())
        .find_map(|c| c.group_word());
    match word {
        Some(w) if reading.chars().count() + w.chars().count() + 3 <= ROW_FACE_MAX_CHARS => {
            format!("{reading} · {w}")
        }
        Some(_) | None => reading,
    }
}

fn face_of(subject: &Subject, check: &Check, compared: Compared, caveats: &[Caveat]) -> String {
    // Lead ruling 3: a row names what it compares (a depth row: "depth 87 %
    // of force cap"); P29-10 10e: a row with no number too ("drill · depth
    // not judged here"). A row that compares its limit's own quantity names
    // the limit ("gantry push not modelled").
    let reading = reading_of(subject, check, compared);
    let core = format!("{} · {reading}", subject.label());
    let with_word = caveats
        .iter()
        .find_map(|c| c.face_word())
        .map(|w| format!("{core} · {w}"));
    match with_word {
        Some(face) if face.chars().count() <= ROW_FACE_MAX_CHARS => face,
        Some(_) | None if core.chars().count() <= ROW_FACE_MAX_CHARS => core,
        // The subject is in the detail; the limit and the reading fit.
        Some(_) | None => reading,
    }
}

fn detail_of(
    subject: &Subject,
    check: &Check,
    compared: Compared,
    caveats: &[Caveat],
    exemption: Option<&'static str>,
    tone: Tone,
    extra: &[String],
) -> String {
    let mut parts = vec![match compared.word() {
        Some(word) => format!(
            "{}: the {word} against the {} ({})",
            subject.label(),
            check.limit.text().noun,
            tone.word()
        ),
        None => format!(
            "{}: {} ({})",
            subject.label(),
            check.limit.text().noun,
            tone.word()
        ),
    }];
    match (check.value, check.bound) {
        (Some(v), Some(b)) => parts.push(format!(
            "value {} {u}, bound {} {u}",
            num(v),
            num(b),
            u = check.unit
        )),
        (Some(v), None) => parts.push(format!("value {} {}, no bound", num(v), check.unit)),
        (None, Some(b)) => parts.push(format!("bound {} {}, no value", num(b), check.unit)),
        (None, None) => {}
    }
    if let Some(face) = margin_face(check, compared) {
        parts.push(face);
    }
    match check.state {
        CheckState::NoNumber(reason) => parts.push(reason.clause().to_owned()),
        CheckState::NotApplicable(geometry) => parts.push(geometry.clause().to_owned()),
        CheckState::Within | CheckState::Exceeds => {}
    }
    if let Some(basis) = &check.basis {
        parts.push(format!("the {} sets the bound", basis.setting()));
    }
    if check.binding {
        parts.push("this limit binds".to_owned());
    }
    parts.extend(caveats.iter().map(|c| c.clause()));
    if let Some(p) = check.population {
        // P29-10 10g: the unit of the population (a drill gate counts holes).
        parts.push(format!(
            "{} of {} {}",
            p.contributing,
            p.offered,
            p.unit.plural()
        ));
    }
    if let Some(row) = exemption {
        parts.push(format!("exemption row {row}"));
    }
    parts.extend(extra.iter().cloned());
    parts.join("; ")
}

/// The sample depth that a gate of `limit` reads (M4): the power and the
/// chip read the removed layer, the deflection and the force the engaged
/// edge (`tool_load/CLAUDE.md`).
const fn sample_depth_of(limit: LimitId) -> Option<SampleDepth> {
    match limit {
        LimitId::SpindlePower | LimitId::ChipMax | LimitId::ChipMin => {
            Some(SampleDepth::RemovedLayer)
        }
        LimitId::ToolDeflection | LimitId::ForceBudget => Some(SampleDepth::EngagedEdge),
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
        | LimitId::DrillPlungeFeed => None,
    }
}

/// The row of one gate row of the verdict.
fn group_row(status: &CriterionStatus<'_>, subject: Subject) -> CheckRow {
    group_row_with(status, subject, of_criterion(status), Vec::new())
}

/// [`group_row`] with the check and the extra clauses the caller built.
fn group_row_with(
    status: &CriterionStatus<'_>,
    subject: Subject,
    check: Check,
    mut extra: Vec<String>,
) -> CheckRow {
    let mut caveats = Vec::new();
    if let Some(depth) = sample_depth_of(check.limit) {
        caveats.push(Caveat::SampleDepth { depth });
    }
    // P29-10 10e: the detail carries what the old row hover said beside
    // the check: the source of the bound, the reason of an unmodelled
    // gate, the confidence of an approximate input and the remedy of a
    // gate that exceeds. Text only; no field and no number moves.
    extra.extend(
        status
            .bound_source
            .as_ref()
            .map(|source| format!("bound from {}", source.clause())),
    );
    if let Some(reason) = status.unmodeled_reason {
        extra.push(reason.operator_text());
    }
    if let Some(crate::tool_load::verdict::Confidence::Approximate(why)) = status.confidence {
        extra.push(format!("approximate: {why}"));
    }
    if let Some(exceeded) = status.exceeded.as_ref() {
        extra.push(format!("remedy: {}", exceeded.remedy));
    }
    let compared = Compared::of_criterion(status.kind);
    CheckRow::new(subject, check, compared, caveats, &extra, None)
}

/// The row of a cell with no verdict: a group with no sample, or a group
/// that the scope function does not judge.
fn empty_row(
    kind: CriterionKind,
    subject: Subject,
    state: CheckState,
    extra: &[String],
    own_row: Option<&'static str>,
) -> CheckRow {
    let limit = LimitId::of_criterion(kind, crate::tool_load::verdict::ChipSide::High);
    let check = Check {
        limit,
        value: None,
        bound: None,
        unit: kind.unit(),
        side: BoundSide::Ceiling,
        margin: None,
        state,
        basis: None,
        binding: false,
        population: None,
    };
    CheckRow::new(
        subject,
        check,
        Compared::of_criterion(kind),
        Vec::new(),
        extra,
        own_row,
    )
}

/// The rows of one gate over every cut group: one row per group.
///
/// P29-11 section 13, ruling 4: the headline group is a scope cell too.
/// When `limits::scope` does not judge the gate's `limits` on it (a drill
/// cycle), its row is the row of a `NotJudged` cell: the state of
/// `scope_cell_state` and the row id, as on every other scope cell. The
/// headline verdict (`NotApplicableForOp`) does not change.
fn gate_rows<V>(
    kind: CriterionKind,
    limits: &[LimitId],
    headline: &V,
    cells: &super::ByGroup<GroupCell<V>>,
    headline_group: CutGroup,
    status_of: impl Fn(&V) -> CriterionStatus<'_>,
    out: &mut Vec<CheckRow>,
) {
    for group in CutGroup::ALL {
        let row = match cells.get(group) {
            GroupCell::Headline => {
                let subject = Subject::Group {
                    group,
                    headline: true,
                    samples: None,
                    material: None,
                };
                match super::scope_of_all(limits, group) {
                    super::Scope::NotJudged(cell) => {
                        scope_cell_row(kind, subject, cell.row, cell.reason)
                    }
                    super::Scope::Judged | super::Scope::NotACutLimit(_) => {
                        group_row(&status_of(headline), subject)
                    }
                }
            }
            GroupCell::Judged { samples, verdict } => group_row(
                &status_of(verdict),
                Subject::Group {
                    group,
                    headline: group == headline_group,
                    samples: Some(*samples),
                    material: None,
                },
            ),
            GroupCell::NoSamples => empty_row(
                kind,
                Subject::Group {
                    group,
                    headline: group == headline_group,
                    samples: Some(0),
                    material: None,
                },
                CheckState::NoNumber(NoNumberReason::NoMaterialRemoved),
                &[],
                None,
            ),
            GroupCell::NotJudged {
                samples,
                row,
                reason,
            } => scope_cell_row(
                kind,
                Subject::Group {
                    group,
                    headline: group == headline_group,
                    samples: Some(*samples),
                    material: None,
                },
                row,
                reason,
            ),
        };
        out.push(row);
    }
}

/// The row of a gate cell that `limits::scope` does not judge, on the
/// headline group or another group. P29-11 11-2b: the cell takes the class
/// of its row, as the envelope cell does (one classification,
/// `scope_cell_state`), and names its own row of `EXEMPTIONS`.
fn scope_cell_row(kind: CriterionKind, subject: Subject, row: &str, reason: &str) -> CheckRow {
    empty_row(
        kind,
        subject,
        super::scope_cell_state(row),
        &[reason.to_owned()],
        EXEMPTIONS.iter().find(|e| e.id == row).map(|e| e.id),
    )
}

/// THE projection of a gate verdict (P29-10): one row per gate and cut
/// group (every `GroupCell`, the `NotJudged` and `NoSamples` cells
/// included), the gantry-push row of the headline group, one row per drill
/// gate, and one row per own material and gate (S6) plus one row per
/// material that no gate judged.
///
/// The destructures name every field with no `..`: a new verdict field, a
/// new gate field of `GroupVerdicts` or a new `GroupCell` does not compile
/// until it states its rows here.
#[must_use]
pub fn rows_of_verdict(verdict: &ToolpathLoadVerdict) -> Vec<CheckRow> {
    let ToolpathLoadVerdict {
        toolpath_id: _,
        chipload,
        power,
        deflection,
        depth,
        drill_gates,
        // Report-only records: no limit, so no row.
        modulation_summary: _,
        feed_explanation: _,
        kinematic_utilization: _,
        material_split,
        // P17: the force reading is report-only. P29-11 11-5 (F6-a, lead
        // ruling L1): the depth gate reads the reading's engagement door,
        // so the depth row is the one force-budget record (same population,
        // `Exceeds` exactly when the force is over the budget); the reading
        // gives no row.
        cut_force: _,
        groups,
    } = verdict;
    let crate::tool_load::verdict::GroupVerdicts {
        headline,
        chipload: chip_cells,
        power: power_cells,
        deflection: deflection_cells,
        depth: depth_cells,
        // As `cut_force` above: report-only.
        cut_force: _,
    } = groups;
    let h = *headline;
    let mut out = Vec::new();
    gate_rows(
        CriterionKind::Chipload,
        Gate::Chipload.limits(),
        chipload,
        chip_cells,
        h,
        |v| v.as_criterion_status(),
        &mut out,
    );
    gate_rows(
        CriterionKind::Power,
        Gate::Power.limits(),
        power,
        power_cells,
        h,
        |v| v.as_criterion_status(),
        &mut out,
    );
    gate_rows(
        CriterionKind::Deflection,
        Gate::Deflection.limits(),
        deflection,
        deflection_cells,
        h,
        |v| v.as_criterion_status(),
        &mut out,
    );
    gate_rows(
        CriterionKind::DepthOfCut,
        Gate::Depth.limits(),
        depth,
        depth_cells,
        h,
        |v| v.as_criterion_status(),
        &mut out,
    );
    // The gantry push: a known absence, on the headline group only.
    if let Some(gantry) = verdict
        .milling_criteria()
        .iter()
        .find(|s| s.kind == CriterionKind::GantryPush)
    {
        out.push(group_row(
            gantry,
            Subject::Group {
                group: h,
                headline: true,
                samples: None,
                material: None,
            },
        ));
    }
    if let Some(d) = drill_gates {
        for (kind, outcome) in [
            (CriterionKind::DrillChipWelding, &d.chip_welding),
            (CriterionKind::DrillPeckAdequacy, &d.peck_adequacy),
            (CriterionKind::DrillPlungeFeed, &d.plunge_feed),
        ] {
            out.push(drill_row(kind, outcome, d, h));
        }
    }
    if let Some(split) = material_split.as_deref() {
        own_material_rows(split, h, &mut out);
    }
    out
}

/// The row of one drill gate. A two-sided gate (the plunge feed) reports
/// the nearer bound of its envelope (`DrillGateOutcome::bound`); when that
/// bound is the FLOOR of the envelope the row states it as a floor, so a
/// feed inside the envelope near its floor reads "333 % of floor", not a
/// share over a ceiling (P29-10 10e2). The state stays the gate's own; the
/// detail names the whole envelope.
fn drill_row(
    kind: CriterionKind,
    outcome: &crate::tool_load::drill_gates::DrillGateOutcome,
    d: &crate::tool_load::drill_gates::DrillGatesVerdict,
    headline: CutGroup,
) -> CheckRow {
    let status = outcome.as_criterion_status(kind, d.cycle, d.population);
    let mut check = of_criterion(&status);
    let mut extra = Vec::new();
    // Only the plunge feed is a two-sided gate; the chip welding band
    // `[0.75t, t]` of an `Elevated` outcome is no envelope.
    if kind == CriterionKind::DrillPlungeFeed
        && let Some((lo, hi)) = outcome.two_sided_envelope()
    {
        extra.push(format!(
            "the envelope is {} to {} {}",
            num(lo),
            num(hi),
            check.unit
        ));
        if outcome.judges_its_floor() {
            check.side = BoundSide::Floor;
            check.margin = check
                .value
                .zip(check.bound)
                .and_then(|(v, b)| BoundSide::Floor.margin(v, b));
        }
    }
    group_row_with(
        &status,
        Subject::Group {
            group: CutGroup::Drill,
            headline: headline == CutGroup::Drill,
            samples: None,
            material: None,
        },
        check,
        extra,
    )
}

/// The rows of a toolpath with no verdict (lead ruling 6: a toolpath never
/// has an empty list): one `NoNumber(reason)` row per gate and cut group,
/// the body group as the headline.
#[must_use]
pub fn rows_without_a_verdict(reason: NoNumberReason) -> Vec<CheckRow> {
    let mut out = Vec::new();
    for kind in [
        CriterionKind::Chipload,
        CriterionKind::Power,
        CriterionKind::Deflection,
        CriterionKind::DepthOfCut,
    ] {
        for group in CutGroup::ALL {
            out.push(empty_row(
                kind,
                Subject::Group {
                    group,
                    headline: group == CutGroup::Body,
                    samples: None,
                    material: None,
                },
                CheckState::NoNumber(reason),
                &[],
                None,
            ));
        }
    }
    out
}

/// S6: the rows of each own material and of each material that no gate
/// judged.
fn own_material_rows(split: &MaterialSplit, headline: CutGroup, out: &mut Vec<CheckRow>) {
    let MaterialSplit {
        // The stock population is the main verdict.
        stock_samples: _,
        own,
        not_judged,
    } = split;
    for o in own {
        let OwnMaterialVerdict {
            material,
            samples,
            chipload,
            power,
            deflection,
            depth,
        } = o;
        let subject = || Subject::Group {
            group: headline,
            headline: true,
            samples: Some(*samples),
            material: Some(material.clone()),
        };
        out.push(group_row(&chipload.as_criterion_status(), subject()));
        out.push(group_row(&power.as_criterion_status(), subject()));
        out.push(group_row(&deflection.as_criterion_status(), subject()));
        out.push(group_row(&depth.as_criterion_status(), subject()));
    }
    for n in not_judged {
        out.push(empty_row(
            CriterionKind::Power,
            Subject::Group {
                group: headline,
                headline: true,
                samples: Some(n.samples),
                material: Some(n.material.clone()),
            },
            CheckState::NoNumber(NoNumberReason::NoForceLine),
            &[n.label()],
            None,
        ));
    }
}

/// THE projection of an RPM choice: one row per named limit.
#[must_use]
pub fn rows_of_rpm_choice(choice: &RpmChoice) -> Vec<CheckRow> {
    of_rpm_choice(choice)
        .into_iter()
        .map(|check| CheckRow::new(Subject::Rpm, check, Compared::Load, Vec::new(), &[], None))
        .collect()
}

impl NoNumberReason {
    /// `true` for a reason that a run or a datum can fill
    /// ([`CheckRow::is_open`]).
    #[must_use]
    pub const fn is_open(self) -> bool {
        match self {
            Self::NoSimulation
            | Self::StaleSimulation
            | Self::NoArc
            | Self::NoVendorRow
            | Self::NoForceLine
            | Self::NoToolBody
            | Self::NoEarlierLeave
            | Self::NoStatedFeed => true,
            Self::NoMaterialRemoved
            | Self::NotModelled
            | Self::OnlyALowerBound
            | Self::NoBound
            | Self::NotJudgedHere
            | Self::JudgedPerCut => false,
        }
    }

    /// The reason in short words, for the face.
    #[must_use]
    pub const fn face(self) -> &'static str {
        match self {
            Self::NoForceLine => "no force line",
            Self::NoSimulation => "no simulation",
            Self::StaleSimulation => "stale simulation",
            Self::NoVendorRow => "no vendor row",
            Self::NoMaterialRemoved => "no material removed",
            Self::NoArc => "no arc",
            Self::NoToolBody => "no tool body",
            Self::NotModelled => "not modelled",
            Self::OnlyALowerBound => "inside at a lower bound",
            Self::NoEarlierLeave => "no earlier leave",
            Self::NoStatedFeed => "no stated feed",
            Self::NoBound => "no limit here",
            Self::NotJudgedHere => "not judged here",
            Self::JudgedPerCut => "judged per cut",
        }
    }

    /// The reason in one clause, for the detail.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::NoForceLine => "the material has no force line",
            Self::NoSimulation => "no simulation ran",
            Self::StaleSimulation => "the simulation is stale",
            Self::NoVendorRow => "no vendor row gives a chip band",
            Self::NoMaterialRemoved => "no sample removes material",
            Self::NoArc => "the trace holds no engagement arc",
            Self::NoToolBody => "the tool has no built body",
            Self::NotModelled => "the model does not cover the cut",
            Self::OnlyALowerBound => {
                "the value is a lower bound inside the limit, so the check proves nothing"
            }
            Self::NoEarlierLeave => "no earlier op of the setup states a leave",
            Self::NoStatedFeed => "the op states no feed or no spindle speed",
            Self::NoBound => "the limit gives this cut no bound",
            Self::NotJudgedHere => "this limit is not judged on this cut",
            Self::JudgedPerCut => "the rows of each declared cut judge this limit",
        }
    }
}

impl Geometry {
    /// The geometry reason in short words, for the face.
    #[must_use]
    pub const fn face(self) -> &'static str {
        match self {
            Self::NoRadialEngagement => "no side cut",
            Self::NotAnOffsetRing => "not an offset ring",
            Self::NoVerticalFeed => "no vertical feed",
        }
    }

    /// The geometry reason in one clause, for the detail.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::NoRadialEngagement => "the cut has no radial engagement",
            Self::NotAnOffsetRing => "the cut is not an offset ring",
            Self::NoVerticalFeed => "the cut has no vertical feed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state: CheckState, margin: Option<f64>) -> CheckRow {
        CheckRow::stated(
            Check {
                limit: LimitId::SpindlePower,
                value: margin.map(|_| 1.0),
                bound: margin.map(|_| 2.0),
                unit: "kW",
                side: BoundSide::Ceiling,
                margin,
                state,
                basis: None,
                binding: false,
                population: None,
            },
            Compared::Load,
        )
    }

    /// P29-10 10g (item 4c): a cut with no row over is one line of counts;
    /// a cut with a row over has none.
    #[test]
    fn a_quiet_cut_is_one_line_of_counts_p29_10() {
        let within = row(CheckState::Within, Some(0.5));
        let modelled = row(CheckState::NoNumber(NoNumberReason::NotModelled), None);
        let no_bound = row(CheckState::NoNumber(NoNumberReason::NoBound), None);
        let over = row(CheckState::Exceeds, Some(-0.5));
        assert_eq!(
            quiet_summary(&[&within, &within, &modelled]).as_deref(),
            Some("2 within \u{00B7} 1 not modelled")
        );
        assert_eq!(
            quiet_summary(&[&within, &modelled, &no_bound]).as_deref(),
            Some("1 within \u{00B7} 2 no number")
        );
        assert_eq!(
            quiet_summary(&[&within, &over]),
            None,
            "a row over keeps the cut open"
        );
    }

    /// A stated force-budget depth row of one cut: `margin` over its cap,
    /// with the presence row of a cut that may not occur, else a worst-case
    /// own depth (a caveat that softens the tone and is no presence).
    fn stated_cut(cut: CutKind, margin: f64, may_occur_row: Option<&'static str>) -> CheckRow {
        CheckRow::planned(
            cut,
            Check {
                limit: LimitId::ForceBudget,
                value: Some(1.0 - margin),
                bound: Some(1.0),
                unit: "mm",
                side: BoundSide::Ceiling,
                margin: Some(margin),
                state: CheckState::Exceeds,
                basis: None,
                binding: false,
                population: None,
            },
            PlannedFacts {
                may_occur_row,
                own_depth: may_occur_row.is_none().then_some(PlannedOwnDepth {
                    mm: 1.0 - margin,
                    rule: "test",
                    worst_case: true,
                }),
                lower_bound: false,
                stored_feed: false,
                advisory_floor: None,
                modulation: RowModulation::NotCapped,
                judged_at: JudgedAt::Stated,
                ceiling_chip: CeilingChipSource::OfThePoint,
                scope_row: None,
                at_earlier_leave: false,
                entry_no_number: None,
                compared: Compared::Depth,
                at: None,
            },
        )
    }

    /// P29-10z (lead ruling R): inside one tone group the row of a cut that
    /// always occurs ranks before the row of a cut that may not occur, even
    /// when the second is further over (operator idx 1: the planner cap at
    /// 387 % before the first slot at 420 %); then the existing order.
    #[test]
    fn the_certain_cut_ranks_first_p29_10() {
        use super::super::FirstPassKind;
        let slot = stated_cut(
            CutKind::FirstPass(FirstPassKind::Slot),
            -4.2,
            Some("P28-01"),
        );
        let cap = stated_cut(CutKind::FirstPass(FirstPassKind::PlannerCap), -3.87, None);
        let body = stated_cut(CutKind::Body, -3.78, None);
        for r in [&slot, &cap, &body] {
            assert_eq!(r.tone(), Tone::ExceedsAdvisory, "one tone group");
        }
        assert!(cap.worse_than(&slot), "the certain cut first");
        assert!(body.worse_than(&slot), "the certain cut first");
        assert!(cap.worse_than(&body), "then the lower margin");
        let mut rows = [&slot, &body, &cap];
        rows.sort_by(|a, b| a.rank_cmp(b));
        assert_eq!(
            rows.map(CheckRow::face),
            [cap.face(), body.face(), slot.face()]
        );
    }
}
