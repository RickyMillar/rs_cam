//! The gate context of one toolpath (P29-6 commit 6a).
//!
//! [`ToolpathLoadContext`] has private fields and one constructor,
//! [`ToolpathLoadContext::of_operation`]. The facts of the context come from
//! the operation, and only from it: the operation kind, the vendor row key,
//! the body facts of the op's cut list and the commanded thinning lift. No
//! code outside this module can build a literal of the context (E0451), the
//! gate modules of `tool_load` included, so a new gate cannot build a
//! context that lacks the facts or states a fact that no op gives.
//!
//! The context also carries the cap table of the toolpath, so the gates can
//! build the cut of each sample ([`ToolpathLoadContext::sample_context`],
//! `limits::Cut::of_sample`).

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::ids::ToolpathId;
use crate::limits::{CapTable, CutFacts, CutGroup, CutKind, SampleContext};
use crate::machine::SteadyFeedMmMin;
use crate::material::Material;
use crate::ops::drill_op::DrillOp;
use crate::tool::ToolDefinition;
use crate::trace::toolpath_spans::Span;

/// The per-toolpath inputs of the gates. Build it with
/// [`Self::of_operation`]; a product door builds it through
/// `ProjectSession::load_context_parts(..).ctx()` (F070).
#[derive(Clone, Copy)]
pub struct ToolpathLoadContext<'a> {
    /// Stable simulator-side toolpath id (matches
    /// `SimulationCutSample::toolpath_id`).
    toolpath_id: ToolpathId,
    tool: &'a ToolDefinition,
    material: &'a Material,
    /// The vendor row key: the op's feeds family. Only the chip row lookup
    /// reads it (`chipload::matched_chip_envelope`); it is not a limit
    /// scope.
    row_family: LutOperationFamily,
    /// The vendor row key: the op's pass role. Only the chip row lookup
    /// reads it; it is not a limit scope.
    row_pass_role: LutPassRole,
    /// P20e: the feed the op commands in steady cutting,
    /// `min(op feed, feed_max_rate, machine cutting ceiling)`
    /// (`MachineProfile::op_steady_feed`, the ceiling the modulator caps
    /// at). The chipload steady-state filter and the histogram compare each
    /// sample feed with it, and the commanded stage states it (the G-code F
    /// word under a cap).
    steady_feed: SteadyFeedMmMin,
    /// The toolpath's operation kind. Only the chip row lookup reads it
    /// (`chipload::routed_lookup_family`, through [`Self::chip_row_key`]); no
    /// gate reads it to decide a scope (P29-6 6d).
    operation_kind: OperationType,
    /// The group of the op's own cut: [`CutGroup::Drill`] when the op
    /// declares a drill cycle (`CutList::declares(CutKind::Drill)`), else
    /// [`CutGroup::Body`]. The headline verdict of each gate is the verdict
    /// of this group (`groups::judge_by_group`); with no trace it still
    /// tells a drill cycle, which has no sample yet.
    headline_group: CutGroup,
    /// Structural spans on the (annotated) toolpath, for the span lookup of
    /// `tool_load::locality`. `None` when the spans are not valid.
    spans: Option<&'a [Span]>,
    /// The cap table of the toolpath (`CapTable::of_toolpath`), for the
    /// cut kind of a sample (`limits::CutKind::of_sample`).
    caps: &'a CapTable,
    /// The facts of the body cut of the op (`OperationConfig::cuts`). The
    /// chip thinning rule of the chip gate and the histogram, and the leave
    /// of the deflection limit, read them.
    body_facts: CutFacts,
    /// `DrillOp` payload for drill toolpaths — `Some` for
    /// `OperationType::{Drill, AlignmentPinDrill}`, `None` otherwise. Drives
    /// the `drill_gates` field on the resulting verdict.
    drill_op: Option<&'a DrillOp>,
    /// Feeds contract P11b2: the radial chip-thinning lift of the cut the op
    /// STATES (`CutSpec::calc_inputs`: both a depth and a width), by the rule
    /// Suggest sizes the feed with. The `commanded_above_band` comparison
    /// reads the commanded peak chip `fz / lift` with it. `None` when the op
    /// states no width or no depth: the gate then takes the median measured
    /// lift of its trip set.
    commanded_thinning_lift: Option<f64>,
}

impl<'a> ToolpathLoadContext<'a> {
    /// The context of the toolpath `toolpath_id` of `operation`.
    ///
    /// The operation fills the operation kind, the vendor row key
    /// (`OperationConfig::feeds_style`), the body facts
    /// (`operation.cuts().body_facts()`) and the commanded thinning lift.
    /// The caller gives the rest:
    ///
    /// - `tool`: the built tool of the op.
    /// - `steady_feed`: `MachineProfile::op_steady_feed` of the op feed (an
    ///   op with no feed has no context, S11: the caller checks it, because
    ///   it needs the feed for the steady feed).
    /// - `spans`: the valid spans of the annotated toolpath, else `None`.
    /// - `caps`: `CapTable::of_toolpath` of the result, or
    ///   [`CapTable::empty`] for a hand-built trace.
    /// - `drill_op`: the drill payload of a drill result, else `None`.
    #[expect(
        clippy::too_many_arguments,
        reason = "one constructor for every input of the gates: a bundle type would be a second door"
    )]
    #[must_use]
    pub fn of_operation(
        toolpath_id: ToolpathId,
        operation: &OperationConfig,
        tool: &'a ToolDefinition,
        material: &'a Material,
        steady_feed: SteadyFeedMmMin,
        spans: Option<&'a [Span]>,
        caps: &'a CapTable,
        drill_op: Option<&'a DrillOp>,
    ) -> Self {
        let (family, pass_role) = operation.feeds_style();
        let cuts = operation.cuts();
        let body_facts = cuts.body_facts();
        let headline_group = if cuts.declares(CutKind::Drill) {
            CutKind::Drill.group()
        } else {
            CutKind::Body.group()
        };
        Self {
            toolpath_id,
            tool,
            material,
            row_family: crate::feeds::vendor_normalize::op_family_to_lut(family),
            row_pass_role: pass_role.into(),
            steady_feed,
            operation_kind: operation.op_type(),
            headline_group,
            spans,
            caps,
            body_facts,
            drill_op,
            commanded_thinning_lift: commanded_thinning_lift(operation, tool, body_facts),
        }
    }

    /// The context of the S6 own-material population (`CutAs::OwnMaterial`):
    /// this context with `material`, no drill payload and no commanded lift
    /// (the own-material verdict states no drill gates and takes the median
    /// measured lift).
    #[must_use]
    pub(super) fn with_own_material<'m>(&self, material: &'m Material) -> ToolpathLoadContext<'m>
    where
        'a: 'm,
    {
        ToolpathLoadContext {
            material,
            drill_op: None,
            commanded_thinning_lift: None,
            ..*self
        }
    }

    /// The toolpath id.
    #[must_use]
    pub fn toolpath_id(&self) -> ToolpathId {
        self.toolpath_id
    }

    /// The built tool.
    #[must_use]
    pub fn tool(&self) -> &'a ToolDefinition {
        self.tool
    }

    /// The stock material.
    #[must_use]
    pub fn material(&self) -> &'a Material {
        self.material
    }

    /// The vendor row key of the chip gate: the op kind (the row routing of
    /// `chipload::routed_lookup_family`), the feeds family and the pass
    /// role. Only the chip row lookup reads it; it is a row key, not a limit
    /// scope (P29-6 6d: the op kind has no other reader in the gates).
    #[must_use]
    pub(super) fn chip_row_key(&self) -> ChipRowKey {
        ChipRowKey {
            operation_kind: self.operation_kind,
            family: self.row_family,
            pass_role: self.row_pass_role,
        }
    }

    /// The group of the op's own cut (the headline group of each gate).
    #[must_use]
    pub fn headline_group(&self) -> CutGroup {
        self.headline_group
    }

    /// The steady feed (P20e).
    #[must_use]
    pub fn steady_feed(&self) -> SteadyFeedMmMin {
        self.steady_feed
    }

    /// The valid spans of the toolpath, or `None`.
    #[must_use]
    pub fn spans(&self) -> Option<&'a [Span]> {
        self.spans
    }

    /// The facts of the body cut of the op.
    #[must_use]
    pub fn body_facts(&self) -> CutFacts {
        self.body_facts
    }

    /// The drill payload, or `None`.
    #[must_use]
    pub fn drill_op(&self) -> Option<&'a DrillOp> {
        self.drill_op
    }

    /// The commanded thinning lift (P11b2), or `None`.
    #[must_use]
    pub fn commanded_thinning_lift(&self) -> Option<f64> {
        self.commanded_thinning_lift
    }

    /// What `limits::Cut::of_sample` reads beside a sample of this
    /// toolpath: the tool, the spans, the cap table and the body facts.
    #[must_use]
    pub fn sample_context(&self) -> SampleContext<'a> {
        SampleContext::new(self.tool, self.spans, self.caps, self.body_facts)
    }
}

/// The vendor row key of the chip gate ([`ToolpathLoadContext::chip_row_key`]).
#[derive(Debug, Clone, Copy)]
pub(super) struct ChipRowKey {
    pub(super) operation_kind: OperationType,
    pub(super) family: LutOperationFamily,
    pub(super) pass_role: LutPassRole,
}

/// P11b2: the lift of the stated cut, by Suggest's own rule (the engaged
/// diameter at the stated depth, `feeds::effective_diameter`). `None` when
/// the op states no depth or no width.
fn commanded_thinning_lift(
    operation: &OperationConfig,
    tool: &ToolDefinition,
    body_facts: CutFacts,
) -> Option<f64> {
    let hint = tool.to_geometry_hint();
    let diameter = stated_diameter_mm(tool);
    let (stated_ap, stated_ae, _) = operation.cut_spec().calc_inputs(diameter);
    stated_ap.zip(stated_ae).map(|(ap, ae)| {
        let d_eng = crate::feeds::engagement::EngagedDiameter::of_hint(
            hint,
            diameter,
            // P22-0c-6b: the body's cutting cap (ruling 1).
            tool.body().max_cut_diameter_mm(),
            ap,
        );
        // P23a: the width of Suggest's lift (the pass ceiling of an
        // adaptive plan; `PowerCases::full_feed_width`).
        let width = crate::feeds::engagement::PowerCases::of_operation(operation)
            .full_feed_width(ae, diameter);
        // P29-1: the lift of the one load, on the stated cut at the
        // planner width.
        crate::feeds::cut_load::CutLoad::thinning_lift(
            &crate::feeds::engagement::Engagement::from_cut(
                crate::feeds::envelope::CutPoint {
                    ap_mm: ap,
                    ae_mm: width,
                },
                hint,
                d_eng,
                None,
            ),
            // P29-3: the thinning rule of the op's body cut.
            body_facts.thins_chip,
        )
    })
}

/// The stated diameter of the tool record (`ToolConfig::diameter`): the tip
/// diameter of a tapered ball (its cutter reports the shaft as its
/// diameter), else the cutter diameter. `build_cutter` builds each cutter
/// from `ToolConfig::diameter`, so the value is that field to the bit.
fn stated_diameter_mm(tool: &ToolDefinition) -> f64 {
    match tool.to_geometry_hint() {
        crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => 2.0 * tip_radius,
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::VBit { .. } => crate::tool::MillingCutter::diameter(tool),
    }
}
