//! Feeds contract P29-3 (commit A): the one list of every exemption from a
//! limit.
//!
//! An exemption is a cut, a kind of cut or a reader that a limit does not
//! judge today. Each one is a row of [`EXEMPTIONS`]. A row is physical (it
//! names a [`Geometry`] reason), it needs a ruling, or it quotes the
//! operator ruling that keeps it. No fourth class exists.
//!
//! Each scope read in product code carries a marker comment: the word
//! `EXEMPT` with the row ids in parentheses, on the line above the read.
//! The sentry `tests/the_exemption_list_only_shrinks_p29_3.rs` holds the
//! link both ways and pins the count of rows that need a ruling. A step
//! that closes a row deletes the row and its markers, and lowers the pin.
//!
//! The row ids `P28-*` are the 34 rows of `phases/P28_limit_audit.md`; the
//! ids `M1` to `M12` are section 3.3 of `phases/P29_parts/A_cuts_and_limits.md`.
//! The other ids are named on their rows. P29-6 6d closed every row of a
//! cut group that no gate judged (`P28-22`, `P28-23`, `M2`, `M3`, `M8`,
//! the `Q10-*`, `Q4-*` and `C-*` rows): a gate now asks
//! [`super::scope`] for each group.

use super::{CutKind, EntryKind, Geometry, LimitId};

/// Who is exempt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Who {
    /// The cuts whose facts (today: label values) hold this condition.
    Facts(&'static str),
    /// The cuts of these kinds.
    Kind(&'static [CutKind]),
    /// A cut that the generator makes and no reader reads before the
    /// simulation (`P29-3_DESIGN.md` section 2.1).
    Undeclared(&'static str),
    /// A reader that judges by its own rule.
    Reader(&'static str),
}

/// The class of an exemption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExemptionClass {
    /// The limit has no meaning for the cut, for this geometry reason.
    Physical(Geometry),
    /// No ruling keeps it. An operator ruling closes it or makes it
    /// [`Self::Ruled`].
    NeedsRuling,
    /// An operator ruling keeps it.
    Ruled {
        /// Who ruled, and where the ruling is recorded.
        by: &'static str,
        /// The date of the ruling (`YYYY-MM-DD`).
        date: &'static str,
        /// The words of the ruling.
        text: &'static str,
    },
}

/// One exemption from one or more limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exemption {
    /// The row id.
    pub id: &'static str,
    /// The limit or the rule, in words: the "Limit" column of the audit.
    /// Some rules are no [`LimitId`] (a width rule, a default cut).
    pub what: &'static str,
    /// The limits that the row exempts from; empty for a rule that is no
    /// [`LimitId`].
    pub limit: &'static [LimitId],
    /// Who is exempt.
    pub who: Who,
    /// The class.
    pub class: ExemptionClass,
    /// The files (from the crate root) that hold the scope read and its
    /// marker; empty for a cut that no reader reads.
    pub where_: &'static [&'static str],
    /// The step that closes the row, or why it stays.
    pub closes_in: &'static str,
}

/// The load caps of the feed modulator.
const MODULATOR_CAPS: &[LimitId] = &[
    LimitId::ChipMax,
    LimitId::ChipMin,
    LimitId::SpindlePower,
    LimitId::ToolDeflection,
    LimitId::ForceBudget,
];

const DRILL: &[CutKind] = &[CutKind::Drill];

const R2: ExemptionClass = ExemptionClass::Ruled {
    by: "operator, ruling R2 (feeds_matrix_2026-09-23/RULINGS.md)",
    date: "2026-09-23",
    text: "I want the most flexibility for the user",
};

const NR: ExemptionClass = ExemptionClass::NeedsRuling;

/// Every exemption from a limit today. See the module doc.
pub static EXEMPTIONS: [Exemption; 51] = [
    // P28: the limit audit.
    Exemption {
        id: "P28-02",
        what: "the machine force budget",
        limit: &[LimitId::ForceBudget],
        who: Who::Facts("no depth limit, finish pass: wall and surface finishes"),
        class: R2,
        where_: &["src/machine/mod.rs", "src/tool_load/depth.rs"],
        closes_in: "stays, as a ruling",
    },
    Exemption {
        id: "P28-03",
        what: "the machine force budget",
        limit: &[LimitId::ForceBudget],
        who: Who::Facts(
            "groove cuts before the simulation (VCarve, Inlay, InlayPlug, Chamfer): the \
             envelope reads no groove width",
        ),
        class: NR,
        where_: &["src/feeds/envelope.rs"],
        closes_in: "P29-5c",
    },
    Exemption {
        id: "P28-04",
        what: "the machine force budget",
        limit: &[LimitId::ForceBudget],
        who: Who::Facts("no depth limit, drill cycle"),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/machine/mod.rs", "src/limits/scope.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "P28-05",
        what: "the width rule (the most removal inside the limits)",
        limit: &[],
        who: Who::Facts("not an offset ring; the Adaptive family"),
        class: NR,
        where_: &["src/feeds/envelope.rs", "src/feeds/suggest.rs"],
        closes_in: "P29-4, or a width ruling",
    },
    Exemption {
        id: "P28-06",
        what: "the width rule",
        limit: &[],
        who: Who::Facts("finish cuts with a stepover"),
        class: NR,
        where_: &["src/feeds/envelope.rs", "src/feeds/suggest.rs"],
        closes_in: "later",
    },
    Exemption {
        id: "P28-07",
        what: "the corner coverage of an offset ring",
        limit: &[LimitId::CornerCoverage],
        who: Who::Facts("not an offset ring"),
        class: ExemptionClass::Physical(Geometry::NotAnOffsetRing),
        where_: &["src/feeds/envelope.rs", "src/feeds/suggest.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "P28-08",
        what: "a width ceiling of an adaptive cut",
        limit: &[],
        who: Who::Facts("the Adaptive family: an unsourced default and a hint only"),
        class: NR,
        where_: &["src/feeds/mod.rs", "src/compute/validate.rs"],
        closes_in: "ruling R2 of P29_FINDINGS.md section 7",
    },
    Exemption {
        id: "P28-09",
        what: "the move budget (a width floor)",
        limit: &[LimitId::MoveBudget],
        who: Who::Reader("ops with no move model, and ops with no stated stepover"),
        class: NR,
        where_: &["src/feeds/envelope.rs", "src/feeds/predict.rs"],
        closes_in: "later",
    },
    Exemption {
        id: "P28-10",
        what: "the spindle power as a limit (ladder, power edge, RPM limit)",
        limit: &[LimitId::SpindlePower],
        who: Who::Kind(DRILL),
        class: NR,
        where_: &[
            "src/feeds/calculator/power.rs",
            "src/feeds/calculator/spindle.rs",
            "src/feeds/calculator/close.rs",
            "src/limits/scope.rs",
        ],
        closes_in: "P29-8 (no end-cut force source)",
    },
    Exemption {
        id: "P28-11",
        what: "every pre-simulation limit that reads a depth",
        limit: &[
            LimitId::SpindlePower,
            LimitId::ToolDeflection,
            LimitId::ForceBudget,
            LimitId::FluteGuard,
        ],
        who: Who::Facts("the depth is known after the simulation (Project Curve)"),
        class: NR,
        where_: &[
            "src/compute/catalog/cut_spec.rs",
            "src/feeds/envelope.rs",
            "src/feeds/operating_point.rs",
        ],
        closes_in: "P29-9",
    },
    Exemption {
        id: "P28-12",
        what: "the spindle power, the slot case",
        limit: &[LimitId::SpindlePower],
        who: Who::Undeclared(
            "the first slots of 3D Adaptive, region Face and 2D Adaptive without slot \
             clearing",
        ),
        class: NR,
        where_: &["src/compute/catalog.rs"],
        closes_in: "P29-5b (filing)",
    },
    Exemption {
        id: "P28-13",
        what: "the spindle power, the planner width",
        limit: &[LimitId::SpindlePower],
        who: Who::Reader("the contour-parallel 3D rough reads trochoid_cap_mult (1.6)"),
        // The operator ruled on the 1.6 (Q2, P22_DESIGN.md section 9,
        // 2026-11-09): "not a magic number; choose the most correct one",
        // and to retire it. The lead kept the read for the power case
        // (P23a section 4). No operator ruling keeps the exemption.
        class: NR,
        where_: &["src/compute/catalog.rs"],
        closes_in: "later (a contour-parallel width rule)",
    },
    Exemption {
        id: "P28-14",
        what: "the pass ceiling that the planner holds",
        limit: &[],
        who: Who::Reader("the contour-parallel 3D rough holds no bound"),
        class: NR,
        where_: &["src/compute/catalog.rs"],
        closes_in: "later",
    },
    Exemption {
        id: "P28-15",
        what: "the tool deflection before the simulation",
        limit: &[LimitId::ToolDeflection],
        who: Who::Facts("finish and semi-finish cuts"),
        class: NR,
        where_: &["src/feeds/envelope.rs"],
        closes_in: "P29-7",
    },
    Exemption {
        id: "P28-16",
        what: "the tool deflection, two limit values",
        limit: &[LimitId::ToolDeflection],
        who: Who::Reader("a rough cut: 200 um before the simulation, the leave after it"),
        class: NR,
        where_: &["src/tool_load/deflection.rs", "src/session/compute.rs"],
        closes_in: "P29-7",
    },
    Exemption {
        id: "P28-17",
        what: "the tool deflection",
        limit: &[LimitId::ToolDeflection],
        who: Who::Kind(DRILL),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/limits/scope.rs", "src/feeds/predict.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "P28-18",
        what: "the radial chip-thinning lift",
        limit: &[LimitId::ChipMax],
        who: Who::Facts("thins_chip is the finish pass"),
        class: ExemptionClass::Ruled {
            by: "operator, P11 L1/L2 (feeds_contract_2026-11-03/STATUS.md)",
            date: "2026-11-05",
            text: "yes, as planned",
        },
        where_: &["src/feeds/geometry.rs", "src/feeds/calculator/feed.rs"],
        closes_in: "stays, as a ruling",
    },
    Exemption {
        id: "P28-19",
        what: "the axial chip thinning of a ball at a shallow depth",
        limit: &[LimitId::ChipMax],
        who: Who::Reader("ball and tapered ball: observed, not applied"),
        class: NR,
        where_: &["src/feeds/calculator/feed.rs"],
        closes_in: "later",
    },
    Exemption {
        id: "P28-20",
        what: "the chip band gate",
        limit: &[LimitId::ChipMax, LimitId::ChipMin],
        who: Who::Kind(DRILL),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/limits/scope.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "P28-21",
        what: "the envelope finding of the simulation",
        limit: &[LimitId::SpindlePower],
        who: Who::Reader(
            "the envelope finding places the body group only; an entry is a different cut \
             (P29-6 ruling Q4)",
        ),
        class: NR,
        where_: &["src/compute/simulate.rs"],
        closes_in: "P29-8 (entries get their own envelope)",
    },
    Exemption {
        id: "P28-24",
        what: "the vendor depth table",
        limit: &[LimitId::VendorDepthTable],
        who: Who::Facts("a peck"),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/feeds/envelope.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "P28-25",
        what: "the vendor axial bound",
        limit: &[LimitId::VendorAxial],
        who: Who::Reader("ops whose axial envelope is NotApplied"),
        class: NR,
        where_: &[
            "src/feeds/envelope.rs",
            "src/feeds/suggest/axial_envelope.rs",
        ],
        closes_in: "later",
    },
    Exemption {
        id: "P28-26",
        what: "the cutting-feed cap arms (Step 7)",
        limit: &[LimitId::CuttingFeedCap],
        who: Who::Kind(DRILL),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/feeds/calculator/feed.rs"],
        closes_in: "- (the drill takes Step 9c)",
    },
    Exemption {
        id: "P28-27",
        what: "the tip plunge cap",
        limit: &[LimitId::PlungeRate],
        who: Who::Reader("flat, bull nose and V-bit tools"),
        class: NR,
        where_: &["src/tool_load/plunge_stress.rs"],
        closes_in: "P29-8 (no source)",
    },
    Exemption {
        id: "P28-28",
        what: "the plunge-entry stability warning",
        limit: &[],
        who: Who::Reader("every family but Adaptive"),
        class: NR,
        where_: &["src/feeds/suggest/adaptive_entry.rs"],
        closes_in: "P29-8",
    },
    Exemption {
        id: "P28-29",
        what: "the load caps of the feed modulator",
        limit: MODULATOR_CAPS,
        who: Who::Kind(&[CutKind::LeadIn, CutKind::LeadOut]),
        class: NR,
        where_: &["src/dressup/feed_modulation.rs"],
        closes_in: "P29-6",
    },
    Exemption {
        id: "P28-30",
        what: "the load caps of the feed modulator",
        limit: MODULATOR_CAPS,
        who: Who::Kind(&[CutKind::Entry(EntryKind::Plunge)]),
        class: NR,
        where_: &["src/dressup/feed_modulation.rs"],
        closes_in: "P29-8 (no plunge model)",
    },
    Exemption {
        id: "P28-31",
        what: "the default cut when the operator states none",
        limit: &[],
        who: Who::Reader("one table of depth and width factors for each family and role"),
        class: NR,
        where_: &["src/feeds/mod.rs"],
        closes_in: "P27 (no source)",
    },
    Exemption {
        id: "P28-32",
        what: "the depth that the calculator reads",
        limit: &[],
        who: Who::Reader("a helix pitch, a chamfer width and a peck give no depth"),
        class: NR,
        where_: &["src/compute/catalog/cut_spec.rs"],
        closes_in: "P29-9",
    },
    Exemption {
        id: "P28-33",
        what: "the tool deflection and the chip at the tip of a V-bit",
        limit: &[LimitId::ToolDeflection, LimitId::ChipMax],
        who: Who::Reader("V-bit: a caveat, or no model"),
        class: NR,
        where_: &["src/feeds/predict.rs"],
        closes_in: "later",
    },
    // The physical half of P28 row 30 (the plunge half is P28-30).
    Exemption {
        id: "P28-30p",
        what: "the load caps of the feed modulator",
        limit: MODULATOR_CAPS,
        who: Who::Kind(&[CutKind::Drill, CutKind::EntryApproach]),
        class: ExemptionClass::Physical(Geometry::NoRadialEngagement),
        where_: &["src/dressup/feed_modulation.rs"],
        closes_in: "- (also a rapid and a retract: no cut)",
    },
    // M: the findings that P28 missed.
    Exemption {
        id: "M4",
        what: "the spindle power against the tool deflection",
        limit: &[LimitId::SpindlePower, LimitId::ToolDeflection],
        who: Who::Reader("one sample has two depths (axial_doc_mm, axial_engagement_mm)"),
        class: NR,
        where_: &["src/feeds/engagement.rs", "src/tool_load/deflection.rs"],
        closes_in: "P29-7 (a probe on ball and V tools)",
    },
    Exemption {
        id: "M6",
        what: "the chip band",
        limit: &[LimitId::ChipMax, LimitId::ChipMin],
        who: Who::Reader("the vendor row is chosen by the op kind (3D Adaptive, Project Curve)"),
        class: NR,
        where_: &["src/feeds/vendor_normalize.rs"],
        closes_in: "P25 / P27",
    },
    Exemption {
        id: "M9",
        what: "the Z axis rate in the RPM choice",
        limit: &[LimitId::ZAxisRate],
        who: Who::Reader("a cut that is not a drill cycle: its plunge reads the rate"),
        class: ExemptionClass::Physical(Geometry::NoVerticalFeed),
        where_: &["src/feeds/calculator/spindle.rs"],
        closes_in: "-",
    },
    Exemption {
        id: "M10",
        what: "the planner engagement floor of the modulator",
        limit: &[],
        who: Who::Reader("by strategy and by move intent"),
        class: NR,
        where_: &["src/session/compute.rs"],
        closes_in: "later",
    },
    Exemption {
        id: "M11",
        what: "the spindle power, the slot case",
        limit: &[LimitId::SpindlePower],
        who: Who::Reader(
            "wrong first-pass filings (region Face, 3D sweep, Bore depth, VCarve stepover, \
             Inlay one pass)",
        ),
        class: NR,
        where_: &["src/compute/catalog.rs"],
        closes_in: "P29-5b (filing)",
    },
    Exemption {
        id: "M12",
        what: "the machine force budget and the tool deflection",
        limit: &[LimitId::ForceBudget, LimitId::ToolDeflection],
        who: Who::Kind(&[
            CutKind::FirstPass(super::FirstPassKind::Slot),
            CutKind::FirstPass(super::FirstPassKind::PlannerCap),
            CutKind::SlotRing,
        ]),
        class: NR,
        where_: &["src/compute/catalog/cuts.rs", "src/feeds/envelope.rs"],
        closes_in: "P29-7",
    },
    // S: the two scope reasons of P29-2 (`NoNumberReason`).
    Exemption {
        id: "S-NoBound",
        what: "the machine force budget",
        limit: &[LimitId::ForceBudget],
        who: Who::Reader("the depth row of a finish pass (NoNumberReason::NoBound) = P28-02"),
        class: R2,
        where_: &["src/limits/project.rs", "src/machine/mod.rs"],
        closes_in: "stays, as P28-02",
    },
    Exemption {
        id: "S-NotJudgedHere",
        what: "every envelope limit",
        limit: &[],
        who: Who::Reader(
            "a limit on a fixed axis, a per-point limit, the drawn move-budget curve \
             (NoNumberReason::NotJudgedHere)",
        ),
        class: NR,
        where_: &["src/limits/project.rs"],
        closes_in: "P29-10",
    },
    // U: one row for each undeclared cut (ruling Q3). No reader reads
    // these cuts, so no file holds a scope read.
    Exemption {
        id: "U-SpringPass",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the spring passes at the final depth (Pocket, Profile)"),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-ResidueCleanup",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared(
            "the residue cleanup of a geodesic slice and of 2D Adaptive (residue mop, \
             contour residue)",
        ),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-PocketZigzag",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the Zigzag pattern of a Pocket (no slot ring, no own cut)"),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-BoreWallFinish",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the wall finish pass of a Bore (the allowance, the whole depth)"),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-TabReplunge",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the re-plunge after each tab of a Profile"),
        class: NR,
        where_: &[],
        closes_in: "P29-8",
    },
    Exemption {
        id: "U-RowConnector",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the row connectors of Face and Zigzag"),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-CappedStep",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared(
            "the steps a planner cuts at a capped feed, up to a slot (3D Adaptive, the \
             departure out of a plunge)",
        ),
        class: NR,
        where_: &[],
        closes_in: "P29-7",
    },
    Exemption {
        id: "U-PerimeterSweep",
        what: "every limit",
        limit: &[],
        who: Who::Undeclared("the full-slot perimeter sweep that opens a 3D Adaptive level"),
        class: NR,
        where_: &[],
        closes_in: "P29-5b (filing)",
    },
    // Ruling Q7: `feeds/efficiency.rs` reads the nominal diameter.
    Exemption {
        id: "U-efficiency",
        what: "the chip-thinning lift and the specific energy of the efficiency card",
        limit: &[],
        who: Who::Reader("feeds/efficiency.rs reads the nominal diameter, not the engagement"),
        class: NR,
        where_: &["src/feeds/efficiency.rs"],
        closes_in: "P29-11",
    },
    // F: the facts of the cut that hold a label value (ruling Q2).
    Exemption {
        id: "F-ThinsChip",
        what: "the radial chip-thinning rule",
        limit: &[LimitId::ChipMax],
        who: Who::Facts("thins_chip is set from the family and the role"),
        class: NR,
        where_: &[
            "src/compute/catalog/cuts.rs",
            "src/feeds/geometry.rs",
            "src/feeds/calculator/feed.rs",
        ],
        closes_in: "P29-4 to P29-7",
    },
    Exemption {
        id: "F-OffsetRing",
        what: "the corner coverage of an offset ring",
        limit: &[LimitId::CornerCoverage],
        who: Who::Facts("offset_ring is set from the Pocket family and the Roughing role"),
        class: NR,
        where_: &["src/compute/catalog/cuts.rs"],
        closes_in: "P29-4 to P29-7",
    },
    Exemption {
        id: "F-LeaveAfter",
        what: "the tool deflection limit",
        limit: &[LimitId::ToolDeflection],
        who: Who::Facts("leave_after_mm is set from the role and the stock-to-leave field"),
        class: NR,
        where_: &["src/compute/catalog/cuts.rs", "src/tool_load/deflection.rs"],
        closes_in: "P29-7",
    },
];
