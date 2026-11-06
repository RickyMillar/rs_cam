//! P10 (operator ruling D8, feeds contract): an op has no geometry default
//! without a tool. With a tool, each tool-derived geometry field is a rule of
//! that tool.
//!
//! - The tool-derived fields (lead answers H1 and Q-D): `stepover`,
//!   `depth_per_pass`, `peck_depth`, `offset_stepover`, `z_step`, Ramp
//!   Finish `max_stepdown` (the depth-per-pass role) and Unified Finish
//!   `raster_stepover` (a stepover), on every op that holds them. `scallop_height` is a surface tolerance, not a tool length: it
//!   keeps its fixed default.
//! - A new op (`OperationConfig::new_default`) holds each of them as `None`
//!   (S11 unset). Generation stops with [`NotSetKind::Geometry`] ("set a
//!   tool", ruling Q10); a load never refuses.
//! - The registry row of each op states one [`GeometryDefault`] per field
//!   (`OpRegistryEntry::geometry`, a required field: a row without it does
//!   not compile). [`GeometryDefault::derive`] gives the value for a tool
//!   and its [`Basis`].
//! - The add door (`ProjectSession::draft_toolpath`) and the tool change
//!   (`ProjectSession::seed_geometry`) write the derived value with the
//!   stamp of its rule ([`rule_stamp`]). A tool change moves a field that
//!   holds its rule stamp, or no value; a field the operator set keeps its
//!   value (lead change 3).
//!
//! [`NotSetKind::Geometry`]: crate::compute::not_set::NotSetKind::Geometry

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{Basis, FeedsField, FeedsProvenance, ValueProvenance};
use crate::material::FormulaConstant;
use crate::trace::lineage::{BasisRecord, StepQuantity, StepRecord};

/// The tool-derived geometry fields of `op` that the op reads with its
/// other settings, each with its value. A Simple or Dwell drill cycle reads
/// no `peck_depth`, so it is not listed then.
///
/// The `match` names every variant: a new op does not compile until it
/// states its fields.
#[must_use]
pub fn read_fields(op: &OperationConfig) -> Vec<(&'static str, Option<f64>)> {
    match op {
        OperationConfig::Face(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::Pocket(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::Zigzag(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::Adaptive(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::Adaptive3d(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::InlayPlug(c) => vec![
            ("stepover", c.stepover),
            ("depth_per_pass", c.depth_per_pass),
        ],
        OperationConfig::Profile(c) => vec![("depth_per_pass", c.depth_per_pass)],
        OperationConfig::Trace(c) => vec![("depth_per_pass", c.depth_per_pass)],
        OperationConfig::VCarve(c) => vec![("stepover", c.stepover)],
        OperationConfig::Inlay(c) => vec![("stepover", c.stepover)],
        OperationConfig::DropCutter(c) => vec![("stepover", c.stepover)],
        OperationConfig::SpiralFinish(c) => vec![("stepover", c.stepover)],
        OperationConfig::HorizontalFinish(c) => vec![("stepover", c.stepover)],
        OperationConfig::SteepShallow(c) => {
            vec![("stepover", c.stepover), ("z_step", c.z_step)]
        }
        OperationConfig::Waterline(c) => vec![("z_step", c.z_step)],
        OperationConfig::UnifiedFinish(c) => {
            vec![("z_step", c.z_step), ("raster_stepover", c.raster_stepover)]
        }
        OperationConfig::RampFinish(c) => vec![("max_stepdown", c.max_stepdown)],
        // The RestDepth detector routes against `cap x offset_stepover` at
        // any pass count, so every pencil reads it.
        OperationConfig::Pencil(c) => vec![("offset_stepover", c.offset_stepover)],
        OperationConfig::Drill(c) => {
            if c.cycle.reads_peck() {
                vec![("peck_depth", c.peck_depth)]
            } else {
                Vec::new()
            }
        }
        OperationConfig::AlignmentPinDrill(c) => {
            if c.cycle.reads_peck() {
                vec![("peck_depth", c.peck_depth)]
            } else {
                Vec::new()
            }
        }
        OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::ProjectCurve(_) => Vec::new(),
    }
}

/// The slot of the tool-derived field `field` of `op`, or `None` when the
/// op has no such field. Every field of the registry rule has a slot (the
/// sentry `every_geometry_default_is_a_fraction_of_the_tool_d8` holds it).
pub fn slot_mut<'a>(op: &'a mut OperationConfig, field: &str) -> Option<&'a mut Option<f64>> {
    match (op, field) {
        (OperationConfig::Face(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Face(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Pocket(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Pocket(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Zigzag(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Zigzag(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Adaptive(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Adaptive(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Adaptive3d(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Adaptive3d(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::InlayPlug(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::InlayPlug(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Profile(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::Trace(c), "depth_per_pass") => Some(&mut c.depth_per_pass),
        (OperationConfig::VCarve(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::Inlay(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::DropCutter(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::SpiralFinish(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::HorizontalFinish(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::SteepShallow(c), "stepover") => Some(&mut c.stepover),
        (OperationConfig::SteepShallow(c), "z_step") => Some(&mut c.z_step),
        (OperationConfig::Waterline(c), "z_step") => Some(&mut c.z_step),
        (OperationConfig::UnifiedFinish(c), "z_step") => Some(&mut c.z_step),
        (OperationConfig::UnifiedFinish(c), "raster_stepover") => Some(&mut c.raster_stepover),
        (OperationConfig::RampFinish(c), "max_stepdown") => Some(&mut c.max_stepdown),
        (OperationConfig::Pencil(c), "offset_stepover") => Some(&mut c.offset_stepover),
        (OperationConfig::Drill(c), "peck_depth") => Some(&mut c.peck_depth),
        (OperationConfig::AlignmentPinDrill(c), "peck_depth") => Some(&mut c.peck_depth),
        (
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Trace(_)
            | OperationConfig::VCarve(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::DropCutter(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::HorizontalFinish(_)
            | OperationConfig::SteepShallow(_)
            | OperationConfig::Waterline(_)
            | OperationConfig::UnifiedFinish(_)
            | OperationConfig::Pencil(_)
            | OperationConfig::Drill(_)
            | OperationConfig::AlignmentPinDrill(_)
            | OperationConfig::Bore(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::Scallop(_)
            | OperationConfig::RampFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::ProjectCurve(_),
            _,
        ) => None,
    }
}

/// The tool-derived fields of `op` that it reads and that are not set
/// ([`read_fields`] with no value), in that order.
#[must_use]
pub fn unset_geometry(op: &OperationConfig) -> Vec<&'static str> {
    read_fields(op)
        .into_iter()
        .filter_map(|(field, value)| value.is_none().then_some(field))
        .collect()
}

/// The source of a `k` of a [`ToolRule`] (F201: Cited or Unsourced).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KRow {
    /// A named constant of this module.
    Constant(&'static FormulaConstant),
    /// The family default `k` of the calculator: the `ap_factor` or
    /// `ae_factor` row of `feeds::operation_default_profile` for the
    /// registry row's `feeds_family` and `feeds_pass_role`. The calculator
    /// reads the same row on an unstated axis, so the add door and Suggest
    /// share one definition.
    FamilyProfile(ProfileAxis),
}

/// The axis of a [`KRow::FamilyProfile`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAxis {
    /// `ap_factor`: the axial depth over D.
    Ap,
    /// `ae_factor`: the radial width over D.
    Ae,
}

impl KRow {
    /// The F201 row of this `k` for the op `op_type`.
    #[must_use]
    pub fn row(self, op_type: OperationType) -> FormulaConstant {
        match self {
            Self::Constant(row) => row.clone(),
            Self::FamilyProfile(axis) => {
                let spec = op_type.spec();
                crate::feeds::default_profile_row(
                    spec.feeds_family,
                    spec.feeds_pass_role,
                    match axis {
                        ProfileAxis::Ap => crate::feeds::ProfileAxisName::Ap,
                        ProfileAxis::Ae => crate::feeds::ProfileAxisName::Ae,
                    },
                )
            }
        }
    }
}

/// How a tool gives one geometry field (P10, lead answers H2 and H3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolRule {
    /// `value = k x D`, D the tool's nominal diameter
    /// (`ToolConfig::diameter`, the one the operator enters).
    FractionOfD(KRow),
    /// On a ball or a tapered-ball tool, with the op's `scallop_height` h
    /// set and `0 < h < R`: the chord of the tip circle,
    /// `s = 2 sqrt(2 R h - h^2)` ([`SCALLOP_CHORD_LAW`],
    /// `finish::scallop_math::stepover_from_scallop_flat`). A finish
    /// stepover on flat ground and a Z step on a vertical wall are the
    /// same chord. Else `FractionOfD` of the row.
    ScallopChordElse(KRow),
    /// P13c (rulings O1, O3, O4): the K2b envelope proposal, the cut with
    /// the most removal per minute inside every limit (the force-budget
    /// depth, the corner-coverage width bound, the slot step, power,
    /// deflection). The width and the depth come from ONE envelope of the
    /// op with both pick fields unset, so the two rules of one op agree.
    /// When the envelope cannot be built (no backed row), or the force
    /// budget cannot set a depth (no force line), the width is the geometry
    /// rule (`geometry_width`) and the depth is left unset: no underived
    /// default; the op refuses until it is set (lead ruling, 2026-11-05).
    EnvelopeProposal(ProposalAxis),
}

/// The axis of a [`ToolRule::EnvelopeProposal`] rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalAxis {
    /// The depth (`ap`).
    Depth,
    /// The width (`ae`, the stepover).
    Width,
}

/// The law of a [`ToolRule::EnvelopeProposal`] value: its stamp reference.
pub const ENVELOPE_PROPOSAL_LAW: &str =
    "envelope proposal: most removal per minute inside the limits";

/// The stamp of an envelope-proposal value: the repo formula, with the law
/// as its reference.
#[must_use]
pub fn proposal_stamp() -> ValueProvenance {
    ValueProvenance {
        source: crate::feeds::ProvenanceSource::Formula,
        reference: Some(ENVELOPE_PROPOSAL_LAW.to_owned()),
    }
}

/// P13c: the envelope proposal of `op` in `ctx`, with every pick field of
/// its rules unset and no stated speeds (the add door writes the speeds
/// for the picked cut). `None` when the envelope refuses the op.
fn envelope_proposal(
    op: &OperationConfig,
    ctx: RuleContext<'_>,
) -> Option<crate::feeds::envelope::SafeEnvelope> {
    let mut probe = op.clone();
    for rule in rules(op.op_type()) {
        if matches!(rule.rule, ToolRule::EnvelopeProposal(_))
            && let Some(slot) = slot_mut(&mut probe, rule.field)
        {
            *slot = None;
        }
    }
    probe.set_spindle_rpm(None);
    crate::feeds::envelope::SafeEnvelope::for_cut(crate::feeds::envelope::EnvelopeInput {
        operation: &probe,
        tool: ctx.tool,
        machine: ctx.machine,
        material: ctx.material,
        lut: crate::feeds::embedded_vendor_lut(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: crate::feeds::suggest::SuggestContext::default(),
    })
    .ok()
}

/// P13c: the one envelope of the pick rules of `op`, or `None` when the op
/// has no pick rule or the envelope refuses it.
fn pick_envelope(
    op: &OperationConfig,
    ctx: RuleContext<'_>,
) -> Option<crate::feeds::envelope::SafeEnvelope> {
    rules(op.op_type())
        .iter()
        .any(|r| matches!(r.rule, ToolRule::EnvelopeProposal(_)))
        .then(|| envelope_proposal(op, ctx))
        .flatten()
}

/// P13c: the proposal of `env` on `axis`, or `None` when the force budget
/// cannot set the depth (a `Rigidity` entry that is `Unmodelled` for want
/// of an input: no force line, no chip). An absent force limit below the
/// flute is not a missing input. P16 (review D5): the depth is `None` when
/// the envelope proposes no depth (`ProposedDepth::None`).
fn proposal_value(
    env: &crate::feeds::envelope::SafeEnvelope,
    axis: ProposalAxis,
) -> Option<(f64, Basis)> {
    use crate::feeds::envelope::{Edge, LimitKind};
    let missing = env.limit(LimitKind::Rigidity).is_some_and(|l| {
        matches!(
            l.edge,
            Edge::Unmodelled { reason }
                if reason == crate::feeds::force_budget::NO_FORCE_LINE
                    || reason == crate::feeds::force_budget::NO_BUDGET_INPUT
        )
    });
    if missing {
        return None;
    }
    let p = env.proposal.get();
    let value = match axis {
        ProposalAxis::Depth => p.ap_mm()?,
        ProposalAxis::Width => p.ae_mm,
    };
    Some((value, env.proposal.basis().clone()))
}

/// P13c (lead ruling on item 1): the width of a pick with no force answer:
/// the corner-coverage bound `(1 + 1/sqrt 2) r`. No force enters it. P16
/// (D12): the slot threshold that stopped it is deleted with the slot cap.
pub(crate) fn geometry_width(d: f64) -> (f64, Basis) {
    let corner = crate::feeds::force_budget::corner_coverage_width_factor();
    let corner_basis = Basis::Rule(crate::feeds::force_budget::corner_coverage_row(
        crate::feeds::force_budget::CORNER_COVERAGE_ROW_NAME,
    ));
    (
        corner * d,
        Basis::derived(
            GEOMETRY_WIDTH_LAW,
            vec![corner_basis, Basis::tool_diameter()],
        ),
    )
}

/// The law of [`geometry_width`].
pub const GEOMETRY_WIDTH_LAW: &str = "corner coverage (1 + 1/sqrt 2) r (no force line or no row)";

/// The rule of one tool-derived field of one op. Each registry row holds
/// one per field (`OpRegistryEntry::geometry`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryDefault {
    /// The config field (`stepover`, `depth_per_pass`, `peck_depth`,
    /// `offset_stepover`, `z_step`).
    pub field: &'static str,
    /// The rule.
    pub rule: ToolRule,
}

/// The law of the scallop chord (H2). Its source is the repo math
/// `finish::scallop_math::stepover_from_scallop_flat` (CREDITS.md,
/// "constant-scallop-height formulas").
pub const SCALLOP_CHORD_LAW: &str = "scallop chord s = 2 sqrt(2 R h - h^2)";

/// The law of a fraction of the tool diameter.
pub const FRACTION_OF_D_LAW: &str = "k x D";

/// The law of a rule value held inside the field range of the schema.
pub const HELD_IN_RANGE_LAW: &str = "held in the field range";

/// One derived value of a tool-derived field.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedValue {
    /// The config field.
    pub field: &'static str,
    /// The value (mm).
    pub value: f64,
    /// How the value is known (K2).
    pub basis: Basis,
    /// The stamp the door writes ([`rule_stamp`]).
    pub stamp: ValueProvenance,
}

/// What a rule reads: the tool, and, for a radial rule, the material and
/// the machine that the calculator reads (lead gate A).
#[derive(Clone, Copy)]
pub struct RuleContext<'a> {
    /// The tool of the op.
    pub tool: &'a ToolConfig,
    /// The stock material.
    pub material: &'a crate::material::Material,
    /// The machine.
    pub machine: &'a crate::machine::MachineProfile,
}

impl GeometryDefault {
    /// The value of this field of `op` in `ctx`.
    ///
    /// A [`KRow::FamilyProfile`] radial rule reads the calculator's
    /// effective width (`feeds::default_radial_width`: the family
    /// `ae_factor` after the tool, flute, material and machine adjustments);
    /// its stamp names the family row, so it stays one stamp per op for a
    /// tool change. A [`KRow::Constant`] rule is `k x D`.
    /// P13c: `None` for a [`ToolRule::EnvelopeProposal`] rule whose
    /// envelope cannot answer (the field stays unset).
    #[must_use]
    pub fn derive(&self, op: &OperationConfig, ctx: RuleContext<'_>) -> Option<DerivedValue> {
        let env = matches!(self.rule, ToolRule::EnvelopeProposal(_))
            .then(|| envelope_proposal(op, ctx))
            .flatten();
        self.derive_with(op, ctx, env.as_ref())
    }

    /// [`Self::derive`] with the envelope of the op's pick rules built once
    /// by the caller (`write_rule_values`, `derive_all`: one envelope gives
    /// the width and the depth).
    fn derive_with(
        &self,
        op: &OperationConfig,
        ctx: RuleContext<'_>,
        pick_env: Option<&crate::feeds::envelope::SafeEnvelope>,
    ) -> Option<DerivedValue> {
        let op_type = op.op_type();
        let tool = ctx.tool;
        let fraction = |k: KRow| {
            let row = k.row(op_type);
            let stamp = ValueProvenance::of_basis(&Basis::Rule(row.clone()));
            match k {
                KRow::Constant(_) => (
                    row.value * tool.diameter,
                    Basis::derived(
                        FRACTION_OF_D_LAW,
                        vec![Basis::Rule(row), Basis::tool_diameter()],
                    ),
                    stamp,
                ),
                KRow::FamilyProfile(_) => {
                    let input = crate::feeds::suggest::feeds_input_for_operation(
                        op,
                        tool,
                        ctx.material,
                        ctx.machine,
                        crate::feeds::embedded_vendor_lut(),
                        crate::feeds::SpindleStrategy::default(),
                    );
                    let (value, basis) = crate::feeds::default_radial_width(&input, ctx.machine);
                    (value, basis, stamp)
                }
            }
        };
        let (value, basis, stamp) = match self.rule {
            ToolRule::FractionOfD(k) => fraction(k),
            ToolRule::ScallopChordElse(k) => {
                let r = tool.diameter / 2.0;
                match op.scallop_height() {
                    Some(h) if tool.tool_type.has_ball_tip() && h > 0.0 && h < r => (
                        crate::finish::scallop_math::stepover_from_scallop_flat(r, h),
                        Basis::derived(
                            SCALLOP_CHORD_LAW,
                            vec![Basis::stated(), Basis::tool_diameter()],
                        ),
                        scallop_stamp(),
                    ),
                    Some(_) | None => fraction(k),
                }
            }
            ToolRule::EnvelopeProposal(axis) => {
                let (value, basis) =
                    match (pick_env.and_then(|env| proposal_value(env, axis)), axis) {
                        (Some(v), _) => v,
                        // Lead ruling on P13c item 1 (2026-11-05): with no force
                        // line or no backed row the width is the geometry rule
                        // (it needs no force); the depth has no derivation and
                        // stays unset (no underived defaults, 2026-10-23).
                        (None, ProposalAxis::Width) => geometry_width(tool.diameter),
                        (None, ProposalAxis::Depth) => return None,
                    };
                (value, basis, proposal_stamp())
            }
        };
        // The value stays inside the field range of the schema (the add
        // door checks it, W6b): a rule value outside it is held at the
        // bound, and the basis says so.
        let range = OperationConfig::param_range_for_type(op_type, self.field);
        let held = range.map_or(value, |r| {
            let bounds = r.inclusive_bounds();
            value.clamp(*bounds.start(), *bounds.end())
        });
        let basis = if held.to_bits() == value.to_bits() {
            basis
        } else {
            Basis::derived(HELD_IN_RANGE_LAW, vec![basis])
        };
        Some(DerivedValue {
            field: self.field,
            value: held,
            basis,
            stamp,
        })
    }

    /// True when `stamp` is a stamp this rule writes ([`rule_stamp`]):
    /// the field still holds a rule value, so a tool change moves it.
    #[must_use]
    pub fn wrote(&self, op_type: OperationType, stamp: &ValueProvenance) -> bool {
        let k = match self.rule {
            ToolRule::FractionOfD(k) | ToolRule::ScallopChordElse(k) => k,
            ToolRule::EnvelopeProposal(_) => return *stamp == proposal_stamp(),
        };
        let fraction = ValueProvenance::of_basis(&Basis::Rule(k.row(op_type)));
        *stamp == fraction
            || (matches!(self.rule, ToolRule::ScallopChordElse(_)) && *stamp == scallop_stamp())
    }
}

/// The stamp of a scallop-chord value: the repo formula, with the law as
/// its reference.
#[must_use]
pub fn scallop_stamp() -> ValueProvenance {
    ValueProvenance {
        source: crate::feeds::ProvenanceSource::Formula,
        reference: Some(SCALLOP_CHORD_LAW.to_owned()),
    }
}

/// The rules of `op_type` (its registry row).
#[must_use]
pub fn rules(op_type: OperationType) -> &'static [GeometryDefault] {
    op_type.registry_entry().geometry
}

/// Each tool-derived value of `op` in `ctx`, in registry order.
#[must_use]
pub fn derive_all(op: &OperationConfig, ctx: RuleContext<'_>) -> Vec<DerivedValue> {
    let env = pick_envelope(op, ctx);
    rules(op.op_type())
        .iter()
        .filter_map(|rule| rule.derive_with(op, ctx, env.as_ref()))
        .collect()
}

/// The stamp slot of the field `field` of `op` in [`FeedsProvenance`]:
/// the slot of its accessor role (a Pencil `offset_stepover` is the
/// stepover, a Waterline `z_step` the depth per pass), else its own slot.
#[must_use]
pub fn stamp_slot(op: &OperationConfig, field: &str) -> FeedsField {
    match field {
        "stepover" | "offset_stepover" | "raster_stepover" => FeedsField::Stepover,
        "peck_depth" => FeedsField::PeckDepth,
        "z_step" if matches!(op, OperationConfig::Waterline(_)) => FeedsField::DepthPerPass,
        "z_step" => FeedsField::ZStep,
        _ => FeedsField::DepthPerPass,
    }
}

/// What a write of the rule values keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// The add door: every field takes its rule value.
    Nothing,
    /// The tool change (lead change 3): a field with a value keeps it,
    /// unless its stamp is the stamp of its rule. An empty field takes the
    /// rule value. A typed value, a pick and a value with no stamp (an old
    /// file) stay.
    StatedValues,
}

/// Write the rule value of each tool-derived field of `op` in `ctx`, with
/// its stamp in `stamps`, under `keep`. Returns the values written.
pub fn write_rule_values(
    op: &mut OperationConfig,
    stamps: &mut FeedsProvenance,
    ctx: RuleContext<'_>,
    keep: Keep,
) -> Vec<DerivedValue> {
    let op_type = op.op_type();
    let mut written = Vec::new();
    let env = pick_envelope(op, ctx);
    for rule in rules(op_type) {
        let derived = rule.derive_with(op, ctx, env.as_ref());
        let slot = stamp_slot(op, rule.field);
        let stamp = stamps.get(slot).cloned();
        let Some(value) = slot_mut(op, rule.field) else {
            continue;
        };
        let moves = match keep {
            Keep::Nothing => true,
            Keep::StatedValues => {
                value.is_none() || stamp.as_ref().is_some_and(|s| rule.wrote(op_type, s))
            }
        };
        if moves {
            // P13b: the depth and the width keep a step record of the rule
            // value (the envelope proposal, or the fraction of D).
            let quantity = match slot {
                FeedsField::DepthPerPass => Some(StepQuantity::Depth),
                FeedsField::Stepover => Some(StepQuantity::Width),
                FeedsField::FeedRate
                | FeedsField::PlungeRate
                | FeedsField::SpindleRpm
                | FeedsField::ScallopHeight
                | FeedsField::RampFeedRate
                | FeedsField::PeckDepth
                | FeedsField::ZStep => None,
            };
            let value_in = *value;
            if let Some(q) = quantity {
                stamps.steps.replace_quantity(q, Vec::new());
            }
            // P13c: a pick with no answer leaves the field unset.
            let Some(derived) = derived else {
                *value = None;
                continue;
            };
            *value = Some(derived.value);
            stamps.set(slot, derived.stamp.clone());
            if let Some(q) = quantity {
                let step = match rule.rule {
                    ToolRule::EnvelopeProposal(_) => "envelope_proposal",
                    ToolRule::FractionOfD(_) | ToolRule::ScallopChordElse(_) => "add_door_rule",
                };
                stamps.steps.replace_quantity(
                    q,
                    vec![StepRecord::new(
                        step,
                        q,
                        value_in,
                        derived.value,
                        BasisRecord::of(&derived.basis),
                    )],
                );
            }
            written.push(derived);
        }
    }
    written
}

// ── The rules of each registry row ─────────────────────────────────────
//
// Lead ruling (P10, 2026-11-05): a radial field (a stepover) reads the
// calculator's effective family `ae_factor` (`KRow::FamilyProfile`, through
// `feeds::default_radial_width`: one function with Suggest, after the tool,
// flute, material and machine adjustments; lead gate A). An axial field (a depth per pass, a Z step on a flat or bull
// tool, a ramp step, a peck) reads `k = today's default / 6 mm`: no file in
// the repo prints an axial k for a default, so each distinct value is one
// Unsourced F201 row that states its derivation. A 6 mm tool keeps the
// depth it had before P10. The family `ap_factor` is not used: at the add
// door it put more FM1 cells outside the K2b envelope on depth (Rigidity
// 118 -> 134, VendorAxial 10 -> 33), a finding for the P12 review.
//
// P13c (rulings O1, O3, O4, 2026-11-05): the Face, Pocket and Zigzag rows
// (the Pocket family roughs) and the Profile depth read the K2b envelope
// proposal (`ToolRule::EnvelopeProposal`): the most removal per minute
// inside every limit, the depth set by the machine force budget, the width
// bounded by the corner-coverage rule. One envelope gives both fields.

/// The reference diameter of the axial rows (mm): the old fixed defaults
/// convert to `k = default / 6`.
pub const AXIAL_REFERENCE_D_MM: f64 = 6.0;

/// `k = 0.5 / 6`: Trace `depth_per_pass` and Ramp Finish `max_stepdown`.
pub(crate) static AXIAL_K_0_5_MM: FormulaConstant = FormulaConstant::unsourced(
    "compute.geometry_default.AXIAL_K_0_5_MM",
    0.5 / AXIAL_REFERENCE_D_MM,
    "today's 0.5 mm at reference D 6 mm; no record in the repo",
);
/// `k = 1.0 / 6`: Face `depth_per_pass`; the `z_step` of Waterline, Steep
/// and Shallow and Unified Finish on a tool with no ball tip.
pub(crate) static AXIAL_K_1_0_MM: FormulaConstant = FormulaConstant::unsourced(
    "compute.geometry_default.AXIAL_K_1_0_MM",
    1.0 / AXIAL_REFERENCE_D_MM,
    "today's 1.0 mm at reference D 6 mm; no record in the repo",
);
/// `k = 1.5 / 6`: Pocket, Zigzag and Inlay plug `depth_per_pass`.
pub(crate) static AXIAL_K_1_5_MM: FormulaConstant = FormulaConstant::unsourced(
    "compute.geometry_default.AXIAL_K_1_5_MM",
    1.5 / AXIAL_REFERENCE_D_MM,
    "today's 1.5 mm at reference D 6 mm; no record in the repo",
);
/// `k = 2.0 / 6`: Profile and 2D Adaptive `depth_per_pass`.
pub(crate) static AXIAL_K_2_0_MM: FormulaConstant = FormulaConstant::unsourced(
    "compute.geometry_default.AXIAL_K_2_0_MM",
    2.0 / AXIAL_REFERENCE_D_MM,
    "today's 2.0 mm at reference D 6 mm; no record in the repo",
);
/// `k = 3.0 / 6`: 3D Rough `depth_per_pass`, and the Drill and Pin Drill
/// `peck_depth`.
pub(crate) static AXIAL_K_3_0_MM: FormulaConstant = FormulaConstant::unsourced(
    "compute.geometry_default.AXIAL_K_3_0_MM",
    3.0 / AXIAL_REFERENCE_D_MM,
    "today's 3.0 mm at reference D 6 mm; no record in the repo",
);

const fn axial(field: &'static str, row: &'static FormulaConstant) -> GeometryDefault {
    GeometryDefault {
        field,
        rule: ToolRule::FractionOfD(KRow::Constant(row)),
    }
}
const fn axial_chord(field: &'static str, row: &'static FormulaConstant) -> GeometryDefault {
    GeometryDefault {
        field,
        rule: ToolRule::ScallopChordElse(KRow::Constant(row)),
    }
}
const fn radial(field: &'static str) -> GeometryDefault {
    GeometryDefault {
        field,
        rule: ToolRule::FractionOfD(KRow::FamilyProfile(ProfileAxis::Ae)),
    }
}
const fn radial_chord(field: &'static str) -> GeometryDefault {
    GeometryDefault {
        field,
        rule: ToolRule::ScallopChordElse(KRow::FamilyProfile(ProfileAxis::Ae)),
    }
}

const fn pick(field: &'static str, axis: ProposalAxis) -> GeometryDefault {
    GeometryDefault {
        field,
        rule: ToolRule::EnvelopeProposal(axis),
    }
}

/// Face, Pocket and Zigzag: the Pocket family roughs, so both fields are
/// the envelope proposal (P13c, O3 and O4).
pub(crate) const FACE: &[GeometryDefault] = POCKET;
/// Pocket, Zigzag (P13c: the envelope proposal; the 0.35 D width and the
/// 1.5 mm at D 6 depth are deleted).
pub(crate) const POCKET: &[GeometryDefault] = &[
    pick("stepover", ProposalAxis::Width),
    pick("depth_per_pass", ProposalAxis::Depth),
];
/// Inlay plug (Trace family, a finish pass): the calculator width and the
/// k row of 1.5 mm at D 6.
pub(crate) const INLAY_PLUG: &[GeometryDefault] =
    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_1_5_MM)];
/// 2D Adaptive.
pub(crate) const ADAPTIVE: &[GeometryDefault] =
    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_2_0_MM)];
/// 3D Rough.
pub(crate) const ROUGH_3D: &[GeometryDefault] =
    &[radial("stepover"), axial("depth_per_pass", &AXIAL_K_3_0_MM)];
/// Profile: the width is the slot (D); the depth is the envelope proposal
/// (P13c, O1 and O4: the deepest depth inside every limit at ae = D).
pub(crate) const PROFILE: &[GeometryDefault] = &[pick("depth_per_pass", ProposalAxis::Depth)];
/// Trace.
pub(crate) const TRACE: &[GeometryDefault] = &[axial("depth_per_pass", &AXIAL_K_0_5_MM)];
/// A stepover only, not a finish (V-Carve, Inlay pocket).
pub(crate) const STEPOVER_ONLY: &[GeometryDefault] = &[radial("stepover")];
/// A finish stepover (Parallel, Spiral, Horizontal finish).
pub(crate) const FINISH: &[GeometryDefault] = &[radial_chord("stepover")];
/// Steep and Shallow.
pub(crate) const STEEP_SHALLOW: &[GeometryDefault] = &[
    radial_chord("stepover"),
    axial_chord("z_step", &AXIAL_K_1_0_MM),
];
/// Waterline.
pub(crate) const WATERLINE: &[GeometryDefault] = &[axial_chord("z_step", &AXIAL_K_1_0_MM)];
/// Unified Finish: the Z step and the raster stepover.
pub(crate) const UNIFIED: &[GeometryDefault] = &[
    axial_chord("z_step", &AXIAL_K_1_0_MM),
    radial_chord("raster_stepover"),
];
/// Ramp Finish.
pub(crate) const RAMP: &[GeometryDefault] = &[axial("max_stepdown", &AXIAL_K_0_5_MM)];
/// Pencil: the offset stepover.
pub(crate) const PENCIL: &[GeometryDefault] = &[radial("offset_stepover")];
/// A drill cycle: the peck.
pub(crate) const DRILL: &[GeometryDefault] = &[axial("peck_depth", &AXIAL_K_3_0_MM)];
/// No tool-derived field.
pub(crate) const NONE: &[GeometryDefault] = &[];

/// The F201 rows this module owns (the family rows are listed by
/// `feeds::formula_constants` already).
#[must_use]
pub fn formula_constants() -> Vec<FormulaConstant> {
    [
        &AXIAL_K_0_5_MM,
        &AXIAL_K_1_0_MM,
        &AXIAL_K_1_5_MM,
        &AXIAL_K_2_0_MM,
        &AXIAL_K_3_0_MM,
    ]
    .into_iter()
    .cloned()
    .collect()
}
