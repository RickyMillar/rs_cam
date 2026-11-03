//! CONTRACT K6 (feeds contract P2): each `diag_ids!` row states what its
//! finding does to the job.
//!
//! A row is a finding with a [`Consequence`], or a missing input
//! ([`RowKind::NotReady`]). A missing input is not a finding: no toolpath
//! can exist without it, so generation stops with a reason (ruling Q10).
//! The `diag_ids!` macro does not compile a row with no column.
//!
//! The doors that refused a job before P2 read this column through
//! [`Gate::refuses_at`], so the table and the doors cannot disagree.

use crate::gcode::LoadOverride;

/// What a finding does to the job. CONTRACT K6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consequence {
    /// The finding refuses the job at each stage in `stages` while its
    /// `gate` raises it. `override_` names the operator override that
    /// waives it, when one does. Commit B of P2 deletes this arm.
    Refuse {
        danger: Danger,
        stages: &'static [Stage],
        gate: Gate,
        override_: Option<LoadOverride>,
    },
    /// The job goes ahead, and every surface names the finding.
    Warn(Danger),
    /// The finding informs only.
    Info,
}

/// What a warning can damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Danger {
    /// The machine: a collision, a plane order, the spindle power.
    Machine,
    /// The tool: its load, its entry, its reach.
    Tool,
    /// The part: a cut into the model, material left, an op left out.
    Part,
}

/// A stage of the job where a finding can refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// An edit door (the height edit of F033).
    Edit,
    /// Generation, and the run state of the next generate.
    Generate,
    /// The G-code export.
    Export,
}

/// The predicate that raises a finding at a door. It decides whether the
/// finding is raised; the [`Consequence`] decides what the finding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// `HeightOrderViolation::refuses` (F033).
    HeightRefuses,
    /// `ModelGougeFinding::blocks_export` (UX-7b).
    GougeBlocks,
    /// `Produced::non_centre_cutting_refusal` is `Some` (G-CENTRECUT).
    CentreCutPlunge,
    /// `non_centre_cutting_refusal_rows` is not empty (G-CENTRECUT).
    CentreCutDescent,
    /// `CriterionStatus::refuses_export` (S4).
    LoadRowRefuses,
    /// `gcode::refusing_unmodeled` is not empty (T-19).
    LoadUnmodeled,
    /// An enabled op has no current result (G-EXPORTSKIP, G-STALEXPORT).
    NoCurrentResult,
    /// The GUI pre-flight asks the operator to confirm (collisions).
    GuiConfirm,
    /// `ToolpathHeights::buried_face_refusal` or the prologue
    /// `refuse_buried_face` (UX-1c).
    BuriedFace,
    /// `OperationType::tool_refusal` (W2.2).
    ToolKind,
}

impl Gate {
    /// Every gate, for a test that walks them.
    pub const ALL: [Self; 10] = [
        Self::HeightRefuses,
        Self::GougeBlocks,
        Self::CentreCutPlunge,
        Self::CentreCutDescent,
        Self::LoadRowRefuses,
        Self::LoadUnmodeled,
        Self::NoCurrentResult,
        Self::GuiConfirm,
        Self::BuriedFace,
        Self::ToolKind,
    ];

    /// True when a row that this gate raises refuses the job at `stage`.
    #[must_use]
    pub fn refuses_at(self, stage: Stage) -> bool {
        super::ids::CONSEQUENCES.iter().any(|(_, row)| match row {
            RowKind::Finding(Consequence::Refuse { stages, gate, .. }) => {
                *gate == self && stages.contains(&stage)
            }
            RowKind::Finding(Consequence::Warn(_) | Consequence::Info) | RowKind::NotReady => false,
        })
    }

    /// The override of the rows this gate raises, when one waives them.
    #[must_use]
    pub fn override_(self) -> Option<LoadOverride> {
        super::ids::CONSEQUENCES
            .iter()
            .find_map(|(_, row)| match row {
                RowKind::Finding(Consequence::Refuse {
                    gate, override_, ..
                }) if *gate == self => Some(*override_),
                RowKind::Finding(
                    Consequence::Refuse { .. } | Consequence::Warn(_) | Consequence::Info,
                )
                | RowKind::NotReady => None,
            })
            .flatten()
    }
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

/// A finding that warns. A `diag_ids!` row writes `=> warn(Danger::Tool)`.
#[must_use]
pub const fn warn(danger: Danger) -> RowKind {
    RowKind::Finding(Consequence::Warn(danger))
}

/// A finding that refuses at `stages` while `gate` raises it.
#[must_use]
pub const fn refuse(
    danger: Danger,
    stages: &'static [Stage],
    gate: Gate,
    override_: Option<LoadOverride>,
) -> RowKind {
    RowKind::Finding(Consequence::Refuse {
        danger,
        stages,
        gate,
        override_,
    })
}

/// A finding that informs only.
pub const INFO: RowKind = RowKind::Finding(Consequence::Info);

/// A missing input.
pub const NOT_READY: RowKind = RowKind::NotReady;
