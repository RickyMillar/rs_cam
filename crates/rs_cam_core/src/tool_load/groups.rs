//! **Every gate judges every cut group** (feeds contract P29-6 6d).
//!
//! The rule, in the order a reader meets it:
//!
//! 1. Each sample that removes material belongs to exactly one
//!    [`CutGroup`] ([`super::locality::judged_group`]). [`populations`]
//!    walks the trace once and puts each such sample in its group.
//! 2. A gate is a [`MetricEvaluator`]: the limits it judges and one
//!    function that judges the samples of one group. A gate never walks the
//!    trace for its population; it reads the samples it is given.
//! 3. [`judge_by_group`] is the one driver. It asks [`crate::limits::scope`]
//!    for each group and each limit of the gate, calls the gate for each
//!    judged group, and builds the fixed record ([`ByGroup`]) of the
//!    result. No gate decides its own scope, and a record cannot leave a
//!    group out.
//! 4. The headline verdict of a gate (the one row that each surface showed
//!    before P29-6) is the verdict of the group of the op's own cut
//!    ([`super::ToolpathLoadContext::headline_group`]): the body, or the
//!    drill cycle, which the side gates do not judge. The headline group is
//!    judged even with no sample, so it always holds a verdict.
//!
//! To add a mechanism: add a `LimitId`, answer its cells in
//! `limits::scope` (the compiler asks for every group), name its gate in
//! [`gate_of`] (the compiler asks again), and write a `MetricEvaluator`
//! whose `judge` reads the samples it is given. The side before the
//! simulation has the same shape (`feeds::planned`, P29-7).

use crate::limits::{ByGroup, CutGroup, LimitId, NotJudged, Scope};
use crate::stock::simulation_cut::{SimulationCutSample, SimulationCutTrace};

use super::{GateEnv, ToolpathLoadContext};

/// What the whole toolpath gives every group of every gate: the counts that
/// a gate states as its `offered` population, and the trace facts that are
/// not a property of one group.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolpathSamples {
    /// Every sample of the toolpath in the trace (the `offered` count of the
    /// power, deflection, depth and force readings).
    pub toolpath_samples: usize,
    /// Every sample of the toolpath that removes material, in every group
    /// (the `offered` count of the chip gate, and its "removed nothing"
    /// test).
    pub removing_samples: usize,
    /// True when a cutting sample of the toolpath that is not air class
    /// carries an arc of engagement: the trace captured arcs. Without one,
    /// the power and deflection gates say `ArcEngagementNotCaptured`. A
    /// property of the trace, not of a group.
    pub arc_captured: bool,
    /// The largest removed-layer depth (`axial_doc_mm`) of the body group:
    /// the depth of the chip gate's vendor row key
    /// (`geometry::lut_key_diameter_for_cutter`, which reads no depth
    /// today, finding F5). Every group reads the body row (the vendor row
    /// has no entry or cleanup row; P29-8 owns the entry rules).
    pub row_key_depth_mm: f64,
}

/// The samples of one toolpath, each in its group. Built once per toolpath
/// by [`populations`].
pub struct Populations<'t> {
    toolpath: ToolpathSamples,
    groups: ByGroup<Vec<(usize, &'t SimulationCutSample)>>,
}

impl<'t> Populations<'t> {
    /// The population of `group`.
    #[must_use]
    pub fn of(&self, group: CutGroup) -> GroupPopulation<'_> {
        GroupPopulation {
            group,
            samples: self.groups.get(group),
            toolpath: &self.toolpath,
        }
    }

    /// The counts and the trace facts of the whole toolpath.
    #[must_use]
    pub fn toolpath(&self) -> &ToolpathSamples {
        &self.toolpath
    }
}

/// The samples that one gate judges for one group: every sample of the
/// toolpath that removes material and lies in `group`, as
/// `(index into the trace, sample)`, in trace order.
#[derive(Clone, Copy)]
pub struct GroupPopulation<'a> {
    /// The group.
    pub group: CutGroup,
    /// The samples of the group.
    pub samples: &'a [(usize, &'a SimulationCutSample)],
    /// The counts and the trace facts of the whole toolpath.
    pub toolpath: &'a ToolpathSamples,
}

/// The samples of the toolpath of `ctx`, each in its group. Empty with no
/// trace. One walk of the trace.
#[must_use]
pub fn populations<'t>(
    ctx: &ToolpathLoadContext<'_>,
    trace: Option<&'t SimulationCutTrace>,
) -> Populations<'t> {
    let mut toolpath = ToolpathSamples::default();
    let mut groups: ByGroup<Vec<(usize, &'t SimulationCutSample)>> =
        ByGroup::from_fn(|_| Vec::new());
    let Some(trace) = trace else {
        return Populations { toolpath, groups };
    };
    let sample_ctx = ctx.sample_context();
    for (i, s) in trace.samples.iter().enumerate() {
        if s.toolpath_id != ctx.toolpath_id() {
            continue;
        }
        toolpath.toolpath_samples += 1;
        if s.is_cutting
            && s.radial_engagement_class()
                != crate::stock::simulation_cut::RadialEngagementClass::Air
            && s.arc_engagement_radians.is_some()
        {
            toolpath.arc_captured = true;
        }
        let Some(group) = super::locality::judged_group(s, &sample_ctx) else {
            continue;
        };
        toolpath.removing_samples += 1;
        if group == CutGroup::Body {
            toolpath.row_key_depth_mm = toolpath.row_key_depth_mm.max(s.axial_doc_mm.max(0.0));
        }
        groups.get_mut(group).push((i, s));
    }
    Populations { toolpath, groups }
}

/// The verdict of one gate for one group.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum GroupCell<V> {
    /// The group of the op's own cut: its verdict is the headline field of
    /// the gate on [`super::ToolpathLoadVerdict`] (`chipload`, `power`,
    /// `deflection`, `depth`, `cut_force`).
    Headline,
    /// The group holds no sample that removes material.
    NoSamples,
    /// The gate judged the samples of the group.
    Judged {
        /// The samples of the group that remove material.
        samples: usize,
        /// The verdict.
        verdict: V,
    },
    /// The scope function does not judge a limit of this gate on the group
    /// (`limits::scope`).
    NotJudged {
        /// The samples of the group that remove material.
        samples: usize,
        /// The exemption row that holds the cell.
        row: String,
        /// The clause for the operator.
        reason: String,
    },
}

impl<V> GroupCell<V> {
    /// The verdict of a judged group; `None` for every other state.
    #[must_use]
    pub fn verdict(&self) -> Option<&V> {
        match self {
            Self::Judged { verdict, .. } => Some(verdict),
            Self::Headline | Self::NoSamples | Self::NotJudged { .. } => None,
        }
    }

    /// The verdict of a judged group, to change.
    pub fn verdict_mut(&mut self) -> Option<&mut V> {
        match self {
            Self::Judged { verdict, .. } => Some(verdict),
            Self::Headline | Self::NoSamples | Self::NotJudged { .. } => None,
        }
    }
}

/// **One gate.** A gate states the limits it judges and judges the samples
/// of one group. The driver [`judge_by_group`] is its only caller.
pub trait MetricEvaluator {
    /// The limits this gate judges. The driver asks
    /// [`crate::limits::scope`] for each of them on each group.
    const LIMITS: &'static [LimitId];
    /// The verdict of one group.
    type Verdict;
    /// What the gate gives beside the headline verdict (the chip gate: its
    /// feed explanation). The driver drops it for the other groups.
    type Extra: Default;
    /// Judge the samples of one group. `population.samples` is the whole
    /// population: the gate reads no other sample of the trace for it.
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: GroupPopulation<'_>,
    ) -> (Self::Verdict, Self::Extra);
    /// The verdict of a headline group that the scope function does not
    /// judge (`UnmodeledReason::NotApplicableForOp` with the clause).
    fn not_judged(cell: NotJudged) -> Self::Verdict;
}

/// The result of one gate over every group.
pub struct GroupJudgement<V, E> {
    /// The verdict of the headline group.
    pub headline: V,
    /// What the gate gave beside the headline verdict.
    pub extra: E,
    /// Every group, one cell each; the headline group holds
    /// [`GroupCell::Headline`].
    pub groups: ByGroup<GroupCell<V>>,
}

/// **The one driver.** For each group: the scope of the gate's limits;
/// the headline group is judged even with no sample; another judged group
/// with no sample is [`GroupCell::NoSamples`].
pub fn judge_by_group<G: MetricEvaluator>(
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
    populations: &Populations<'_>,
) -> GroupJudgement<G::Verdict, G::Extra> {
    let headline_group = ctx.headline_group();
    let mut headline: Option<(G::Verdict, G::Extra)> = None;
    let groups = ByGroup::from_fn(|group| {
        let population = populations.of(group);
        let samples = population.samples.len();
        let scope = crate::limits::scope_of_all(G::LIMITS, group);
        if group == headline_group {
            headline = Some(match scope {
                Scope::Judged => G::judge(ctx, env, population),
                Scope::NotJudged(cell) => (G::not_judged(cell), G::Extra::default()),
                Scope::NotACutLimit(_) => (G::not_judged(NOT_A_CUT_LIMIT), G::Extra::default()),
            });
            return GroupCell::Headline;
        }
        match scope {
            Scope::Judged if samples == 0 => GroupCell::NoSamples,
            Scope::Judged => GroupCell::Judged {
                samples,
                verdict: G::judge(ctx, env, population).0,
            },
            Scope::NotJudged(cell) => not_judged_cell(samples, cell),
            Scope::NotACutLimit(_) => not_judged_cell(samples, NOT_A_CUT_LIMIT),
        }
    });
    // `from_fn` calls the closure once for every group, the headline group
    // included, so the headline is always set; the fallback is the verdict
    // of a scope that judges nothing.
    let (headline, extra) =
        headline.unwrap_or_else(|| (G::not_judged(NOT_A_CUT_LIMIT), G::Extra::default()));
    GroupJudgement {
        headline,
        extra,
        groups,
    }
}

/// The headline verdict of one gate (the tests and the single-gate doors).
pub fn headline<G: MetricEvaluator>(
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
) -> G::Verdict {
    let populations = populations(ctx, env.sim_trace);
    judge_by_group::<G>(ctx, env, &populations).headline
}

/// True when `limits::scope` judges `limit` on the headline group of
/// `ctx` (false on a drill cycle). The cards read it before they bin.
#[must_use]
pub fn headline_is_judged(ctx: &ToolpathLoadContext<'_>, limit: LimitId) -> bool {
    matches!(
        crate::limits::scope(limit, ctx.headline_group()),
        Scope::Judged
    )
}

/// A gate that names a limit that is not a limit of each cut. The sentry
/// `the_exemption_list_only_shrinks_p29_3` holds that no gate does.
const NOT_A_CUT_LIMIT: NotJudged = NotJudged {
    row: "-",
    reason: "this limit is not a limit of each cut",
};

fn not_judged_cell<V>(samples: usize, cell: NotJudged) -> GroupCell<V> {
    GroupCell::NotJudged {
        samples,
        row: cell.row.to_owned(),
        reason: cell.reason.to_owned(),
    }
}

/// One sample gate (a [`MetricEvaluator`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// The chip band gate (`super::ChiploadGate`).
    Chipload,
    /// The spindle power gate (`super::PowerGate`).
    Power,
    /// The tool deflection gate (`super::DeflectionGate`).
    Deflection,
    /// The depth gate (`super::DepthGate`).
    Depth,
    /// The cut force reading (`super::CutForceGate`).
    CutForce,
}

impl Gate {
    /// Every gate, in declaration order.
    pub const ALL: [Self; 5] = crate::all_variants!(Gate:
        Chipload, Power, Deflection, Depth, CutForce,
    );

    /// The name of the gate, as the verdict record names its field.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chipload => "chipload",
            Self::Power => "power",
            Self::Deflection => "deflection",
            Self::Depth => "depth",
            Self::CutForce => "cut_force",
        }
    }

    /// The limits that the gate judges: its `MetricEvaluator::LIMITS`.
    #[must_use]
    pub const fn limits(self) -> &'static [LimitId] {
        match self {
            Self::Chipload => super::ChiploadGate::LIMITS,
            Self::Power => super::PowerGate::LIMITS,
            Self::Deflection => super::DeflectionGate::LIMITS,
            Self::Depth => super::DepthGate::LIMITS,
            Self::CutForce => super::CutForceGate::LIMITS,
        }
    }
}

/// The sample gates of one limit (P29-7 7c, in the place of the hand list
/// `GATE_LIMITS`). One match names every limit and has no wildcard arm: a
/// new limit does not compile until it names its gates, or states that no
/// sample gate judges it (an empty list). The sentry
/// `every_limit_judges_every_declared_cut_p29_7` holds that this list
/// equals the gates whose [`Gate::limits`] name the limit.
///
/// The vendor depth table, the rubbing floor and the plunge rate are limits
/// of each cut (`limits::scope` judges them) with no sample gate: the
/// envelope judges them before the simulation only (row `S-NoSampleGate`,
/// P29-11 section 13, ruling 1).
#[must_use]
pub const fn gate_of(limit: LimitId) -> &'static [Gate] {
    match limit {
        LimitId::ChipMax | LimitId::ChipMin => &[Gate::Chipload],
        LimitId::SpindlePower => &[Gate::Power],
        LimitId::ToolDeflection => &[Gate::Deflection],
        LimitId::ForceBudget => &[Gate::Depth, Gate::CutForce],
        // EXEMPT(S-NoSampleGate)
        LimitId::RubbingFloor | LimitId::VendorDepthTable | LimitId::PlungeRate => &[],
        LimitId::FluteGuard
        | LimitId::CuttingLength
        | LimitId::VendorAxial
        | LimitId::CornerCoverage
        | LimitId::ScallopDepth
        | LimitId::ToolDiameter
        | LimitId::MoveBudget
        | LimitId::EngagementFloor
        | LimitId::CuttingFeedCap
        | LimitId::PlannerFeedCap
        | LimitId::KinematicReach
        | LimitId::SpindleMaxRpm
        | LimitId::ZAxisRate
        | LimitId::OperatorRpmCeiling
        | LimitId::ChartRpm
        | LimitId::VendorRpmMax
        | LimitId::GantryPush
        | LimitId::DrillChipWelding
        | LimitId::DrillPeckAdequacy
        | LimitId::DrillPlungeFeed => &[],
    }
}

/// **A limit of each cut that only the envelope judges** (review
/// 2026-11-15 R8): the limit has a planned role (`feeds::planned`, a depth
/// ceiling or a value check of each declared cut) and [`gate_of`] gives no
/// sample gate. These are the limits of the row `S-NoSampleGate`; claim 1
/// of the sentry `every_limit_judges_every_declared_cut_p29_7` holds that
/// this set equals the row. After a simulation no gate row judges such a
/// limit, so the export warns for its stated rows. A new sample gate for
/// the limit removes it from this set.
#[must_use]
pub const fn judged_only_before_the_simulation(limit: LimitId) -> bool {
    gate_of(limit).is_empty()
        && matches!(
            crate::feeds::planned::planned_limit_of(limit),
            crate::feeds::planned::PlaneRole::DepthCeiling(_)
                | crate::feeds::planned::PlaneRole::ValueCheck(_)
        )
}
