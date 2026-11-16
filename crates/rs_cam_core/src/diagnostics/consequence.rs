//! CONTRACT K6 (feeds contract P2): each `diag_ids!` row states what its
//! finding does to the job.
//!
//! A row is a finding with a [`Consequence`], or a missing input
//! ([`RowKind::NotReady`]). A missing input is not a finding: no toolpath
//! can exist without it, so generation stops with a reason (ruling Q10).
//! The `diag_ids!` macro does not compile a row with no column.
//!
//! No finding refuses a job (ruling Q6): [`Consequence`] has no refusing
//! arm. Generation and every export door carry the warnings with the
//! output, and the G-code header names each one.

/// What a finding does to the job. CONTRACT K6. No arm refuses (ruling Q6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consequence {
    /// The job goes ahead, and every surface names the finding: the export
    /// result, the G-code header and the GUI pre-flight chips.
    Warn(Danger),
    /// The finding informs only. No surface prints it in the export.
    Info,
}

/// What a warning can damage. The GUI pre-flight draws one chip per value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Danger {
    /// The machine: a collision, a plane order, the spindle power. The GUI
    /// Export button takes the Danger style while one stands.
    Machine,
    /// The tool: its load, its entry, its reach.
    Tool,
    /// The part: a cut into the model, material left, an op left out.
    Part,
}

impl Danger {
    /// Every value, in the chip order of the GUI pre-flight.
    pub const ALL: [Self; 3] = [Self::Machine, Self::Tool, Self::Part];

    /// The word the G-code header and the reports print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Machine => "Machine",
            Self::Tool => "Tool",
            Self::Part => "Part",
        }
    }
}

/// The predicate that raises a finding at an export door. It decides
/// whether the finding is raised; the [`Consequence`] decides what the
/// finding does. `ProjectSession::export_warnings` walks [`Gate::ALL`]
/// with one exhaustive `match`, so a new gate does not compile until the
/// export raises its findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// `HeightOrderViolation::refuses` (F033): the `geom.*` order breaks.
    HeightRefuses,
    /// `ModelGougeFinding::warns_at_export` (UX-7b).
    GougeWarns,
    /// `ToolpathStats::non_centre_cutting_plunge` is `Some` (G-CENTRECUT).
    CentreCutPlunge,
    /// `non_centre_cutting_warning_rows` is not empty (G-CENTRECUT).
    CentreCutDescent,
    /// `CriterionStatus::warns_on_export` (S4).
    LoadRowRefuses,
    /// `gcode::warning_unmodeled` is not empty (T-19).
    LoadUnmodeled,
    /// An enabled op has no current result (G-EXPORTSKIP, G-STALEXPORT).
    NoCurrentResult,
    /// A holder or rapid collision of the simulation; the GUI Export button
    /// takes the Danger style.
    GuiConfirm,
    /// `ToolpathHeights::buried_face_refusal` or the prologue
    /// `refuse_buried_face` (UX-1c).
    BuriedFace,
    /// `OperationType::tool_refusal` of a kind the generator can compute
    /// with (W2.2, decision E1).
    ToolKind,
    /// A row of `ToolpathStats::feasibility` (K5, feeds contract P7): a
    /// physical rule warns on the emitted toolpath.
    Feasibility,
    /// K5 ReachFlute, the simulation stage: a fresh simulation measured
    /// wood above the flute (`compute::simulate::flute_reach`).
    FluteOverreach,
    /// K7 (feeds contract P9): a fresh simulation measured a point outside
    /// the K2b envelope (`compute::simulate::envelope_placement`).
    SimOutsideEnvelope,
    /// Review 2026-11-15 F4: an emitted op with no current measured verdict
    /// has a stated row that exceeds with a gating tone, whatever the limit
    /// (`ProjectSession::stated_cut_over_a_limit`). Review R8: after a fresh
    /// simulation, only a stated row of a limit with no sample gate
    /// (`tool_load::groups::judged_only_before_the_simulation`).
    StatedCutOverLimit,
    /// Operator ruling 2026-11-15: the emitted feeds of an op hold a fed
    /// cutting move above the stated feed, or an entry above its stated
    /// feed (`session::EmittedFeeds::above_stated_warning`).
    FeedAboveStated,
    /// P32 (lead rule 2026-11-15): the feed modulation held moves of an
    /// emitted op at the rubbing floor, above a load cap
    /// (`session::EmittedFeeds::held_warning`).
    FeedHeldAtFloor,
    /// P37: a 3D Rough held stock, planned no cut there, and the stock
    /// stands at the end of its plan
    /// (`ToolpathStats::rough_left_stock`, `standing_pieces > 0`).
    RoughLeftStock,
}

impl Gate {
    /// Every gate, for the export walk and the tests.
    pub const ALL: [Self; 17] = [
        Self::HeightRefuses,
        Self::GougeWarns,
        Self::CentreCutPlunge,
        Self::CentreCutDescent,
        Self::LoadRowRefuses,
        Self::LoadUnmodeled,
        Self::NoCurrentResult,
        Self::GuiConfirm,
        Self::BuriedFace,
        Self::ToolKind,
        Self::Feasibility,
        Self::FluteOverreach,
        Self::SimOutsideEnvelope,
        Self::StatedCutOverLimit,
        Self::FeedAboveStated,
        Self::FeedHeldAtFloor,
        Self::RoughLeftStock,
    ];
}

/// The column of one `diag_ids!` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A finding, and what it does to the job.
    Finding(Consequence),
    /// A missing input: generation stops with a reason (`RunState::NotReady`,
    /// ruling Q10). Not a finding.
    NotReady,
}

impl RowKind {
    /// The danger of a row that warns; `None` for an `Info` row and a
    /// missing input.
    #[must_use]
    pub const fn warns(self) -> Option<Danger> {
        match self {
            Self::Finding(Consequence::Warn(danger)) => Some(danger),
            Self::Finding(Consequence::Info) | Self::NotReady => None,
        }
    }
}

/// A finding that warns. A `diag_ids!` row writes `=> warn(Danger::Tool)`.
#[must_use]
pub const fn warn(danger: Danger) -> RowKind {
    RowKind::Finding(Consequence::Warn(danger))
}

/// A finding that informs only.
pub const INFO: RowKind = RowKind::Finding(Consequence::Info);

/// A missing input.
pub const NOT_READY: RowKind = RowKind::NotReady;
