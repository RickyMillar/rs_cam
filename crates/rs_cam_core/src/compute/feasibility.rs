//! CONTRACT K5 (feeds contract P7): every physical rule has the G-CENTRECUT
//! shape.
//!
//! A rule reads a tool fact (K3), tests a predicate, and has one outcome at
//! each stage: the plan, the generation, the simulation and the export. One
//! [`physical_rules!`] row per rule names every stage, so a rule cannot skip
//! one. The feasibility stage (`JobPhases::guard_feasibility`, after the
//! session stages) reads the emitted toolpath, the tool facts and the stock
//! box, and gives a typed [`FeasibilityReport`]. It needs no mesh.
//!
//! Every outcome is a warning (ruling Q6): the program never refuses a job.
//! [`Stage`] has no refusing value, so a row cannot write a refusal. The
//! warning text names the fact, its value and how it is known (ruling Q7):
//! "flute 25 mm, estimated".
//!
//! A row must name every stage:
//!
//! ```compile_fail
//! rs_cam_core::physical_rules! {
//!     Rule { fact: CuttingLength, plan: None("no planner moves a floor"), gen: Warn,
//!            sim: Measure, id: TOOL_REACH_PAST_FLUTE },
//! }
//! ```
//!
//! A `None` stage must give its reason:
//!
//! ```compile_fail
//! rs_cam_core::physical_rules! {
//!     Rule { fact: CuttingLength, plan: None, gen: Warn, sim: Measure, export: Warn,
//!            id: TOOL_REACH_PAST_FLUTE },
//! }
//! ```
//!
//! No stage refuses: `Refuse` is not a [`Stage`] value.
//!
//! ```compile_fail
//! rs_cam_core::physical_rules! {
//!     Rule { fact: CuttingLength, plan: None("no planner moves a floor"), gen: Refuse,
//!            sim: Measure, export: Warn, id: TOOL_REACH_PAST_FLUTE },
//! }
//! ```
//!
//! A full row compiles:
//!
//! ```
//! rs_cam_core::physical_rules! {
//!     Rule { fact: CuttingLength, plan: None("no planner moves a floor"), gen: Warn,
//!            sim: Measure, export: Warn, id: TOOL_REACH_PAST_FLUTE },
//! }
//! assert_eq!(PhysicalRule::ALL.len(), 1);
//! ```

use crate::compute::tool_config::{FactKind, ToolConfig, ToolType};
use crate::geo::BoundingBox3;
use crate::toolpath::{MoveType, Toolpath};

/// The outcome of a rule at one stage. CONTRACT K5. No value refuses
/// (ruling Q6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The planner keeps the motion inside the limit (the G-CENTRECUT ramp
    /// at the limit angle).
    Clamp,
    /// The cutter model carries the fact (the V tip in `build_cutter`).
    Model,
    /// The simulation measures the rule on the cut.
    Measure,
    /// The stage names the finding; the job goes ahead.
    Warn,
    /// The stage does nothing; the text says why.
    None(&'static str),
}

/// The tool fact that a rule reads (K3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleFact {
    /// `ToolConfig::centre_cutting_fact`.
    CentreCutting,
    /// `ToolConfig::cutting_length_fact`: the flute length.
    CuttingLength,
    /// `ToolConfig::stickout_fact`.
    Stickout,
    /// `ToolConfig::v_tip_diameter_fact`.
    VTipDiameter,
}

impl RuleFact {
    /// The word that the warning text prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CentreCutting => "centre cutting",
            Self::CuttingLength => "flute",
            Self::Stickout => "stickout",
            Self::VTipDiameter => "V tip",
        }
    }
}

/// The stages of one rule: the row of [`physical_rules!`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleStages {
    /// The fact that the rule reads.
    pub fact: RuleFact,
    /// The plan stage.
    pub plan: Stage,
    /// The generation stage.
    pub gen_stage: Stage,
    /// The simulation stage.
    pub sim: Stage,
    /// The export stage.
    pub export: Stage,
    /// The `diag_ids!` row of the finding (K6).
    pub id: &'static str,
}

/// CONTRACT K5: declares the physical rules. Each row names the fact, the
/// four stages and the `diag_ids!` constant of its finding. The macro writes
/// `PhysicalRule`, `PhysicalRule::ALL` and `PhysicalRule::stages`. A row
/// with a missing stage does not match the pattern; a `None` stage with no
/// reason and a stage value that does not exist do not compile.
#[doc(hidden)]
#[macro_export]
macro_rules! physical_rules {
    ($(
        $(#[$meta:meta])*
        $rule:ident {
            fact: $fact:ident,
            plan: $plan:ident $(($plan_why:literal))?,
            gen: $gen_stage:ident $(($gen_why:literal))?,
            sim: $sim:ident $(($sim_why:literal))?,
            export: $export:ident $(($export_why:literal))?,
            id: $id:ident $(,)?
        }
    ),* $(,)?) => {
        /// CONTRACT K5: one physical rule. `physical_rules!` writes it.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PhysicalRule {
            $($(#[$meta])* $rule),*
        }

        impl PhysicalRule {
            /// Every rule, in row order.
            pub const ALL: &'static [Self] = &[$(Self::$rule),*];

            /// The row of this rule.
            #[must_use]
            pub const fn stages(self) -> $crate::compute::feasibility::RuleStages {
                match self {
                    $(Self::$rule => $crate::compute::feasibility::RuleStages {
                        fact: $crate::compute::feasibility::RuleFact::$fact,
                        plan: $crate::compute::feasibility::Stage::$plan $(($plan_why))?,
                        gen_stage: $crate::compute::feasibility::Stage::$gen_stage $(($gen_why))?,
                        sim: $crate::compute::feasibility::Stage::$sim $(($sim_why))?,
                        export: $crate::compute::feasibility::Stage::$export $(($export_why))?,
                        id: $crate::diagnostics::ids::$id,
                    }),*
                }
            }
        }
    };
}

physical_rules! {
    /// G-CENTRECUT: a tool that does not cut at its centre goes down into
    /// stock no steeper than its limit. Plan: the entry emitters, the 3D
    /// Rough cut and link paths and its terrain ramp hold the limit slope
    /// (P7 C). Generation: the entry plunge that `run_stages` records and
    /// each `StraightPlunge` descent deeper than the protrusion. Simulation:
    /// the descent rows (`non_centre_cutting_descents`).
    CentreCut {
        fact: CentreCutting,
        plan: Clamp,
        gen: Warn,
        sim: Measure,
        export: Warn,
        id: SAFETY_NON_CENTRE_CUTTING_PLUNGE,
    },
    /// G-SIMCORE (P22-0b): a helix entry of a tool that does not cut at its
    /// centre has a radius of at least half the core diameter. Plan: the
    /// core floor of `HelixRadius::resolve`. Generation: a contained helix
    /// with no room for the floor falls back (ramp, then plunge) and this
    /// row names it. Simulation: the core check (`non_centre_cutting_descents`).
    HelixCore {
        fact: CentreCutting,
        plan: Clamp,
        gen: Warn,
        sim: Measure,
        export: Warn,
        id: SAFETY_HELIX_INSIDE_CORE,
    },
    /// The cut goes deeper below the stock top than the flute is long.
    ReachFlute {
        fact: CuttingLength,
        plan: None("no planner moves a floor"),
        gen: Warn,
        sim: Measure,
        export: Warn,
        id: TOOL_REACH_PAST_FLUTE,
    },
    /// The cut goes as deep below the stock top as the stickout: the
    /// holder face reaches the stock.
    ReachHolder {
        fact: Stickout,
        plan: None("no planner moves a floor"),
        gen: Warn,
        sim: Measure,
        export: Warn,
        id: TOOL_REACH_TO_HOLDER,
    },
    /// The cone height of a V-bit's stated flat tip disagrees with the flute
    /// length. Only a `ToolType::VBit` has the rule. Ruling D12: an
    /// `Unknown` tip is a missing input that stops generation (commit A2),
    /// not a finding of this rule.
    VTipFlat {
        fact: VTipDiameter,
        plan: Model,
        gen: Warn,
        sim: None("the cutter shape carries the tip"),
        export: Warn,
        id: TOOL_V_TIP_UNMODELLED,
    },
}

/// The largest difference (mm) between the flute length of a V-bit and the
/// cone height of its stated tip that is not a disagreement: half of the
/// last printed digit of the records. The records print the flute length
/// to 9 decimals (the V20 row: `cutting_length = 8.152467616`), so a
/// printed value is within 5e-10 mm of the value it stands for.
pub const CONE_HEIGHT_PRINT_TOLERANCE_MM: f64 = 5e-10;

/// The outcome of one rule on one toolpath. CONTRACT K5.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleResult {
    rule: PhysicalRule,
    fact: FactKind,
    measured: f64,
    limit: Option<f64>,
    text: String,
}

impl RuleResult {
    /// The rule.
    #[must_use]
    pub fn rule(&self) -> PhysicalRule {
        self.rule
    }

    /// How the fact that the rule read is known (K3).
    #[must_use]
    pub fn fact(&self) -> FactKind {
        self.fact
    }

    /// The measured value: the depth (mm), or the flute length (mm) of a
    /// V-bit whose cone disagrees.
    #[must_use]
    pub fn measured(&self) -> f64 {
        self.measured
    }

    /// The limit that the measured value passed, or `None` when the rule
    /// knows no limit.
    #[must_use]
    pub fn limit(&self) -> Option<f64> {
        self.limit
    }

    /// The warning sentence: the fact, its value, how it is known, and the
    /// measure.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The `diag_ids!` row of the finding.
    #[must_use]
    pub fn id(&self) -> &'static str {
        self.rule.stages().id
    }
}

/// The rule outcomes of one toolpath. CONTRACT K5. An empty report is a
/// measurement: the stage ran and no rule warned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeasibilityReport {
    rows: Vec<RuleResult>,
}

impl FeasibilityReport {
    /// One row per rule that warns.
    #[must_use]
    pub fn rows(&self) -> &[RuleResult] {
        &self.rows
    }
}

/// What the feasibility stage reads.
pub(crate) struct FeasibilityInputs<'a> {
    /// The emitted toolpath (after every session stage).
    pub toolpath: &'a Toolpath,
    /// The tool and its facts.
    pub tool: &'a ToolConfig,
    /// The core zone of the tool body (`ToolBody::core`, P22-0c-2); `None`
    /// for a centre-cutting tool. The CentreCut rule reads it.
    pub core: Option<crate::tool::body::CoreZone>,
    /// The stock box in the emission frame.
    pub stock_bbox: &'a BoundingBox3,
    /// The XY and the depth base of each hole of a Drill
    /// (`execute::drill_depth_bases`); empty for every other op.
    pub depth_bases: &'a [([f64; 2], f64)],
    /// The entry contract of the registry row (K4). A `StraightPlunge` op
    /// keeps its plunges; the CentreCut rule judges each one.
    pub entry_contract: crate::compute::catalog::EntryContract,
    /// The G-CENTRECUT entry plunge that an entry emitter recorded
    /// (`Produced::non_centre_cutting_refusal`), if any.
    pub entry_plunge: Option<crate::compute::tool_config::NonCentreCuttingRefusal>,
    /// G-SIMCORE: the helix entries that fell back for want of room for the
    /// core floor (`Produced::helix_core_fallbacks`).
    pub helix_fallbacks: &'a [crate::compute::tool_config::HelixCoreFallback],
}

/// A length for the warning text: at most three decimals, no trailing
/// zero.
fn mm(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_owned()
}

/// "flute 25 mm, estimated" or, for an `Unknown` fact, "flute 25 mm (the
/// old default), unknown" (decision F6).
fn fact_text(fact: RuleFact, value: f64, kind: FactKind) -> String {
    match kind {
        FactKind::Unknown => format!(
            "{} {} mm (the old default), {}",
            fact.label(),
            mm(value),
            kind.label()
        ),
        FactKind::Stated | FactKind::Catalogue | FactKind::Estimated => {
            format!("{} {} mm, {}", fact.label(), mm(value), kind.label())
        }
    }
}

impl FeasibilityInputs<'_> {
    /// True when the XY is inside the stock footprint.
    fn in_footprint(&self, x: f64, y: f64) -> bool {
        let b = self.stock_bbox;
        x >= b.min.x && x <= b.max.x && y >= b.min.y && y <= b.max.y
    }

    /// The top that a move at this XY cuts down from: the depth base of the
    /// Drill hole that the tool sits in, else the stock box top.
    fn reference_top(&self, x: f64, y: f64) -> f64 {
        let radius = self.tool.diameter / 2.0;
        self.depth_bases
            .iter()
            .find(|(xy, _)| (xy[0] - x).hypot(xy[1] - y) <= radius)
            .map_or(self.stock_bbox.max.z, |(_, base)| *base)
    }

    /// The deepest material contact (mm) below the reference top: the
    /// lowest end of a fed move inside the stock footprint. `None` when no
    /// fed move ends below the top inside the footprint.
    fn deepest_contact_mm(&self) -> Option<f64> {
        self.toolpath
            .moves
            .iter()
            .filter(|m| m.move_type != MoveType::Rapid)
            .filter(|m| self.in_footprint(m.target.x, m.target.y))
            .map(|m| self.reference_top(m.target.x, m.target.y) - m.target.z)
            .filter(|depth| *depth > 0.0)
            .max_by(f64::total_cmp)
    }

    /// K5 CentreCut (P7 C): the descents of a `StraightPlunge` op with a
    /// tool that does not cut at its centre that go into stock deeper than
    /// the blade protrusion, by the G-CENTRECUT predicate
    /// (`CoreZone::plunge_refusal`) from the reference top. A
    /// descent is a fed move steeper than the limit
    /// (`is_steeper_than_limit`). Empty for every other op and tool.
    pub(crate) fn straight_plunge_refusals(
        &self,
    ) -> Vec<crate::compute::tool_config::NonCentreCuttingRefusal> {
        let Some(core) = self.core else {
            return Vec::new();
        };
        if !matches!(
            self.entry_contract,
            crate::compute::catalog::EntryContract::StraightPlunge { .. }
        ) {
            return Vec::new();
        }
        self.toolpath
            .moves
            .windows(2)
            .filter_map(|pair| {
                let [prev, current] = pair else {
                    return None;
                };
                if current.move_type == MoveType::Rapid
                    || !core.is_steeper_than_limit(prev.target, current.target)
                    || !self.in_footprint(current.target.x, current.target.y)
                {
                    return None;
                }
                let top = self.reference_top(current.target.x, current.target.y);
                core.plunge_refusal(top, current.target)
            })
            .collect()
    }

    /// K5 CentreCut (P7 C): the row of the G-CENTRECUT records, so the
    /// lineage lists the rule where it fires. The entry plunge of an entry
    /// emitter, and each `StraightPlunge` descent deeper than the
    /// protrusion. The surfaces print the G-CENTRECUT row
    /// (`safety.non_centre_cutting_plunge`), not this one.
    fn centre_cut(&self) -> Option<RuleResult> {
        let straight = self.straight_plunge_refusals();
        let deepest = straight
            .iter()
            .chain(self.entry_plunge.as_ref())
            .max_by(|a, b| a.depth_mm.total_cmp(&b.depth_mm))?;
        let count = straight.len() + usize::from(self.entry_plunge.is_some());
        Some(RuleResult {
            rule: PhysicalRule::CentreCut,
            fact: self.tool.centre_cutting_fact().kind(),
            measured: deepest.depth_mm,
            limit: Some(deepest.max_plunge_depth_mm),
            text: format!(
                "centre cutting no, {}: {count} descent(s) go into stock deeper than the {} mm \
                 blade protrusion, deepest {} mm",
                self.tool.centre_cutting_fact().kind().label(),
                mm(deepest.max_plunge_depth_mm),
                mm(deepest.depth_mm)
            ),
        })
    }

    /// G-SIMCORE (P22-0b): the row of the helix entries that fell back for
    /// want of room for the core floor. One row per toolpath: the count,
    /// the first place, its room, the floor and the fallback.
    fn helix_core(&self) -> Option<RuleResult> {
        use crate::compute::tool_config::EntryFallback;
        let first = self.helix_fallbacks.first()?;
        let kind = self.tool.centre_cutting_fact().kind().label();
        let (rings, entries): (
            Vec<&crate::compute::tool_config::HelixCoreFallback>,
            Vec<&crate::compute::tool_config::HelixCoreFallback>,
        ) = self
            .helix_fallbacks
            .iter()
            .partition(|f| f.fallback == EntryFallback::NoPass);
        let mut parts = Vec::new();
        if let Some(head) = entries.first() {
            let count = entries.len();
            let plunges = entries
                .iter()
                .filter(|f| f.fallback == EntryFallback::Plunge)
                .count();
            parts.push(format!(
                "{count} helix entr{} had room for r {} mm only, under the core radius {} mm (a \
                 smaller helix leaves a post under the core); the entry falls back to the {} \
                 ({plunges} plunge(s) in all), first at X{:.3} Y{:.3} Z{:.3}",
                if count == 1 { "y" } else { "ies" },
                mm(head.fit_mm),
                mm(head.core_floor_mm),
                head.fallback.label(),
                head.at.x,
                head.at.y,
                head.at.z
            ));
        }
        // P22-0c-4: a ramp-finish ring inside the core diameter.
        if let Some(head) = rings.first() {
            parts.push(format!(
                "{} ramp ring(s) hold r {} mm only, inside the core radius {} mm: the core \
                 cannot descend on them, so no pass is cut and the ring is left uncut, first \
                 at X{:.3} Y{:.3} Z{:.3}",
                rings.len(),
                mm(head.fit_mm),
                mm(head.core_floor_mm),
                head.at.x,
                head.at.y,
                head.at.z
            ));
        }
        Some(RuleResult {
            rule: PhysicalRule::HelixCore,
            fact: self.tool.centre_cutting_fact().kind(),
            measured: first.fit_mm,
            limit: Some(first.core_floor_mm),
            text: format!("centre cutting no, {kind}: {}", parts.join("; ")),
        })
    }

    fn reach(&self, rule: PhysicalRule) -> Option<RuleResult> {
        let deepest = self.deepest_contact_mm()?;
        let (fact, (limit, kind), passes) = match rule {
            PhysicalRule::ReachFlute => {
                let limit = self.tool.cutting_length_planning_mm();
                let kind = self.tool.cutting_length_fact().kind();
                (RuleFact::CuttingLength, (limit, kind), deepest > limit)
            }
            // Equality warns: the holder face meets the stock top.
            PhysicalRule::ReachHolder => {
                let limit = self.tool.stickout_planning_mm();
                let kind = self.tool.stickout_fact().kind();
                (RuleFact::Stickout, (limit, kind), deepest >= limit)
            }
            PhysicalRule::CentreCut | PhysicalRule::HelixCore | PhysicalRule::VTipFlat => {
                return None;
            }
        };
        passes.then(|| RuleResult {
            rule,
            fact: kind,
            measured: deepest,
            limit: Some(limit),
            text: format!(
                "{}: cuts {} mm below the top (by the stock box)",
                fact_text(fact, limit, kind),
                mm(deepest)
            ),
        })
    }

    fn v_tip_flat(&self) -> Option<RuleResult> {
        if self.tool.tool_type != ToolType::VBit {
            return None;
        }
        // An `Unknown` tip gives no row here: ruling D12 makes it a
        // missing input (commit A2).
        let (tip, kind) = self.tool.v_tip_diameter_fact().planning()?;
        let half_angle = (self.tool.included_angle / 2.0).to_radians();
        let cone = (self.tool.diameter / 2.0 - tip / 2.0) / half_angle.tan();
        let flute = self.tool.cutting_length_planning_mm();
        ((flute - cone).abs() > CONE_HEIGHT_PRINT_TOLERANCE_MM).then(|| RuleResult {
            rule: PhysicalRule::VTipFlat,
            fact: kind,
            measured: flute,
            limit: Some(cone),
            text: format!(
                "{}: the cone is {} mm high, but {}",
                fact_text(RuleFact::VTipDiameter, *tip, kind),
                mm(cone),
                fact_text(
                    RuleFact::CuttingLength,
                    flute,
                    self.tool.cutting_length_fact().kind()
                )
            ),
        })
    }
}

/// THE feasibility stage: the outcome of each rule that this stage judges,
/// in [`PhysicalRule::ALL`] order. It reads the emitted moves, the tool
/// facts and the stock box; it needs no mesh. Nothing refuses.
#[must_use]
pub(crate) fn assess(inputs: &FeasibilityInputs<'_>) -> FeasibilityReport {
    let rows = PhysicalRule::ALL
        .iter()
        .filter_map(|rule| match rule {
            // The G-CENTRECUT records: the entry plunge that `run_stages`
            // records and the `StraightPlunge` descents (P7 C).
            PhysicalRule::CentreCut => inputs.centre_cut(),
            PhysicalRule::HelixCore => inputs.helix_core(),
            PhysicalRule::ReachFlute | PhysicalRule::ReachHolder => inputs.reach(*rule),
            PhysicalRule::VTipFlat => inputs.v_tip_flat(),
        })
        .collect();
    FeasibilityReport { rows }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::tool_config::{
        CentreCutting, EntryFallback, HelixCoreFallback, NonCentreCuttingGeometry, ToolFact, ToolId,
    };
    use crate::geo::P3;

    /// G-SIMCORE (P22-0b): a helix entry that fell back for want of room for
    /// the core floor gives the `HelixCore` row: its id, the room, the floor,
    /// the fallback and the place. No fallback: no row.
    #[test]
    fn a_helix_fallback_is_a_helix_core_row_g_simcore() {
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 18.0;
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 8.0,
                blade_protrusion_mm: 1.0,
            },
        )));
        let toolpath = Toolpath::new();
        let stock_bbox = BoundingBox3 {
            min: P3::new(-30.0, -30.0, 0.0),
            max: P3::new(30.0, 30.0, 12.0),
        };
        let fallbacks = [HelixCoreFallback {
            at: P3::new(10.0, 10.0, 7.0),
            fit_mm: 3.0,
            core_floor_mm: 4.0,
            fallback: EntryFallback::Ramp,
        }];
        let inputs = |helix_fallbacks: &'static [HelixCoreFallback]| FeasibilityInputs {
            toolpath: &toolpath,
            tool: &tool,
            core: tool
                .non_centre_cutting_geometry()
                .map(crate::tool::body::CoreZone::from_geometry),
            stock_bbox: &stock_bbox,
            depth_bases: &[],
            entry_contract: crate::compute::catalog::EntryContract::DescentIsCut,
            entry_plunge: None,
            helix_fallbacks,
        };
        let leaked: &'static [HelixCoreFallback] = Box::leak(Box::new(fallbacks));
        let report = assess(&inputs(leaked));
        let rows: Vec<_> = report
            .rows()
            .iter()
            .filter(|r| r.rule() == PhysicalRule::HelixCore)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].id(),
            crate::diagnostics::ids::SAFETY_HELIX_INSIDE_CORE
        );
        let text = rows[0].text();
        for needle in [
            "r 3 mm",
            "core radius 4 mm",
            "ramp",
            "X10.000 Y10.000 Z7.000",
        ] {
            assert!(text.contains(needle), "{needle:?} not in {text:?}");
        }
        assert!(
            !assess(&inputs(&[]))
                .rows()
                .iter()
                .any(|r| r.rule() == PhysicalRule::HelixCore)
        );

        // P22-0c-4: a ramp-finish ring inside the core diameter (fallback
        // `NoPass`) gives the same row with its own words.
        let rings: &'static [HelixCoreFallback] = Box::leak(Box::new([HelixCoreFallback {
            at: P3::new(1.0, 2.0, 3.0),
            fit_mm: 2.5,
            core_floor_mm: 4.0,
            fallback: EntryFallback::NoPass,
        }]));
        let report = assess(&inputs(rings));
        let ring_rows: Vec<_> = report
            .rows()
            .iter()
            .filter(|r| r.rule() == PhysicalRule::HelixCore)
            .collect();
        assert_eq!(
            ring_rows.len(),
            1,
            "a ring inside the core gives the HelixCore row"
        );
        assert_eq!(
            ring_rows[0].id(),
            crate::diagnostics::ids::SAFETY_HELIX_INSIDE_CORE
        );
        let text = ring_rows[0].text();
        for needle in [
            "1 ramp ring(s) hold r 2.5 mm",
            "core radius 4 mm",
            "left uncut",
            "X1.000 Y2.000 Z3.000",
        ] {
            assert!(text.contains(needle), "{needle:?} not in {text:?}");
        }
        assert!(!text.contains("helix entr"), "{text:?}");
    }
}
