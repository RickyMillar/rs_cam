//! CONTRACT K8 (feeds contract P5b, ruling Q7): every generated toolpath
//! carries its lineage.
//!
//! [`Lineage`] is one record per generated toolpath. It names the facts
//! that the toolpath used and how each fact is known:
//!
//! - the cut of the op ([`CutSpec`], K1);
//! - the envelope pick, when the operator picked a point (K2b);
//! - the stored feeds and speeds with their stamps ([`SpeedBases`]);
//! - each tool fact that the op read ([`FactUse`]) with its kind
//!   ([`FactKind`], K3: stated, catalogue, estimated or unknown);
//! - the entry contract and the entries made ([`EntryLineage`], K4);
//! - the K6 findings of the generation, each with its consequence, and one
//!   `tool.fact_unknown` warning per Unknown tool fact (K3, decision E5);
//! - the K5 rule outcomes ([`RuleResult`], P7): one row per physical rule
//!   that warns on this toolpath.
//!
//! The session builds it in `JobPhases::finish` from the persisted
//! `FeedsProvenance` stamps and the generation output. It does not run
//! Suggest again (ruling Q5).
//!
//! `ToolpathComputeResult::lineage` is a required field and `Lineage` has
//! no `Default`, so a result cannot be built without a lineage:
//!
//! ```compile_fail
//! let _ = rs_cam_core::session::ToolpathComputeResult {
//!     op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
//!         rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
//!             rs_cam_core::toolpath::Toolpath::new(),
//!         ),
//!     )),
//!     stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
//!     debug_trace: None,
//!     semantic_trace: None,
//!     provenance: rs_cam_core::session::ResultProvenance::Dressed,
//! };
//! ```
//!
//! ```compile_fail
//! let _: rs_cam_core::trace::lineage::Lineage = Default::default();
//! ```

use serde::Serialize;
use serde_json::{Value, json};

use crate::compute::catalog::{
    Axial, CutSpec, EntryContract, EntryKind, EntryRecord, FirstPass, OperationConfig,
    OperationType, Radial, TotalDepth,
};
use crate::compute::feasibility::RuleResult;
use crate::compute::tool_config::{
    FactKind, LEGACY_DEFAULT_SHANK_DIAMETER_MM, LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG, ToolConfig,
    ToolType,
};
use crate::diagnostics::consequence::{Consequence, Danger, RowKind};
use crate::feeds::provenance::{FeedsProvenance, ProvenanceSource, ValueProvenance};
use crate::feeds::sourced::LAW_PICKED;

/// CONTRACT K8. One record per generated toolpath. No `Default`: only
/// [`Lineage::of`] (generation) and [`Lineage::unrecorded`] (a hand-built
/// result) make one.
#[derive(Debug, Clone, PartialEq)]
pub struct Lineage {
    /// The cut of the op (K1).
    pub cut: CutSpec,
    /// The envelope pick (K2b), when a stored geometry value has the
    /// "picked" stamp. `None`: the operator states the cut.
    pub pick: Option<PickRecord>,
    /// The stored cut and speeds, each with its stamp.
    pub speeds: SpeedBases,
    /// Each tool fact that the op read.
    pub tool_facts: Vec<FactUse>,
    /// The entry contract and the entries made (K4).
    pub entry: EntryLineage,
    /// The K6 findings of the generation.
    pub findings: Vec<FindingUse>,
    /// The K5 rule outcomes: one row per physical rule that warns.
    pub rules: Vec<RuleResult>,
    /// K7 (feeds contract P9): what a fresh simulation measured, placed on
    /// the K2b envelope. Generation records `None`; a reader with a fresh
    /// trace attaches it (`ProjectSession::lineage_measured`). Ruling Q5:
    /// Suggest reads none of it.
    pub measured: Option<MeasuredLineage>,
}

/// K7 (feeds contract P9): the measured engagement of a toolpath on its
/// envelope, as the lineage records it. The case is "measured".
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MeasuredLineage {
    /// Always "measured": the engagement of the simulation samples, not a
    /// pre-sim case (nominal, slot, planner cap).
    pub case: &'static str,
    /// The steady-state cutting samples placed.
    pub placed: usize,
    /// The samples outside a ceiling of the envelope.
    pub outside: usize,
    /// The sample of the largest power: its cut, power and placement.
    pub peak: Option<crate::compute::simulate::MeasuredPoint>,
    /// The limit that the worst outside sample crosses, if one does.
    pub binding: Option<crate::feeds::envelope::LimitKind>,
    /// The value that limit judges at the worst sample, and the limit.
    pub judged: Option<crate::feeds::envelope::JudgedValue>,
}

impl MeasuredLineage {
    /// The lineage record of one placement.
    #[must_use]
    pub fn of(p: &crate::compute::simulate::EnvelopePlacement) -> Self {
        let binding = p.worst.and_then(|w| match w.placement {
            crate::feeds::envelope::Placement::Outside { binding, .. } => Some(binding),
            crate::feeds::envelope::Placement::Inside { .. } => None,
        });
        Self {
            case: "measured",
            placed: p.placed,
            outside: p.outside,
            peak: p.peak,
            binding,
            judged: p.worst.and_then(|w| w.judged),
        }
    }
}

/// The envelope pick that the stored geometry records (K2b). The apply
/// funnel stamps a picked depth or width `Manual` with the reference
/// [`LAW_PICKED`]; the lineage reads that stamp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PickRecord {
    /// The picked depth per pass (mm), when the pick wrote it.
    pub ap_mm: Option<f64>,
    /// The picked width (mm), when the pick wrote it.
    pub ae_mm: Option<f64>,
}

/// One stored value and its stamp.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueUse {
    /// The stored value. `None`: the op has no such field, or it is not
    /// set.
    pub value: Option<f64>,
    /// The stamp. `None`: no stamp (a default that nothing wrote).
    pub basis: Option<ValueProvenance>,
}

/// The stored cut and speeds of the op, each with its stamp.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedBases {
    /// The depth per pass (mm).
    pub depth_per_pass: ValueUse,
    /// The stepover (mm).
    pub stepover: ValueUse,
    /// The spindle speed (rev/min).
    pub spindle_rpm: ValueUse,
    /// The side feed (mm/min).
    pub feed_rate: ValueUse,
    /// The plunge feed (mm/min).
    pub plunge_rate: ValueUse,
    /// The helix and ramp entry feed (mm/min).
    pub ramp_feed_rate: ValueUse,
}

/// A tool fact that the op read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFactKind {
    /// The cutting diameter.
    Diameter,
    /// The flute count.
    FluteCount,
    /// The cutting (flute) length.
    FluteLength,
    /// The stickout from the holder.
    Stickout,
}

impl ToolFactKind {
    /// The facts that every op reads: the feeds calculator reads all four.
    pub const ALL: [Self; 4] = [
        Self::Diameter,
        Self::FluteCount,
        Self::FluteLength,
        Self::Stickout,
    ];

    /// The name that the GUI and MCP print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Diameter => "Diameter",
            Self::FluteCount => "Flutes",
            Self::FluteLength => "Flute length",
            Self::Stickout => "Stickout",
        }
    }

    /// The unit of the value ("" for a count).
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Diameter | Self::FluteLength | Self::Stickout => "mm",
            Self::FluteCount => "",
        }
    }

    fn value_of(self, tool: &ToolConfig) -> f64 {
        match self {
            Self::Diameter => tool.diameter,
            Self::FluteCount => f64::from(tool.flute_count_planning()),
            Self::FluteLength => tool.cutting_length_planning_mm(),
            Self::Stickout => tool.stickout_planning_mm(),
        }
    }

    /// How the fact is known (K3). The diameter is not a `ToolFact` in
    /// this plan: it is `Stated` (decision E6).
    fn kind_of(self, tool: &ToolConfig) -> FactKind {
        match self {
            Self::Diameter => FactKind::Stated,
            Self::FluteCount => tool.flute_count_fact().kind(),
            Self::FluteLength => tool.cutting_length_fact().kind(),
            Self::Stickout => tool.stickout_fact().kind(),
        }
    }
}

/// One tool fact that the op read: the value planning used and how it is
/// known. An `Unknown` fact's value is its legacy loader default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FactUse {
    /// The fact.
    pub fact: ToolFactKind,
    /// The value planning used.
    pub value: f64,
    /// How it is known (K3).
    pub state: FactKind,
}

/// K3 + K6 (decision E5): one `tool.fact_unknown` text per Unknown fact
/// that the toolpath used: the facts of [`ToolFactKind::ALL`], the shank
/// diameter, and the taper half-angle of a tapered tool. The centre-cutting
/// and V tip facts warn in P7, not here.
fn unknown_fact_texts(tool: &ToolConfig) -> Vec<String> {
    let text = |name: &str, value: f64, unit: &str| {
        let unit = if unit.is_empty() {
            String::new()
        } else {
            format!(" {unit}")
        };
        format!("{name} Unknown: planned at the old default ({value}{unit})")
    };
    let mut out: Vec<String> = ToolFactKind::ALL
        .iter()
        .filter(|f| f.kind_of(tool) == FactKind::Unknown)
        .map(|f| text(f.label(), f.value_of(tool), f.unit()))
        .collect();
    if tool.shank_diameter_fact().is_unknown() {
        out.push(text(
            "Shank diameter",
            LEGACY_DEFAULT_SHANK_DIAMETER_MM,
            "mm",
        ));
    }
    if tool.tool_type == ToolType::TaperedBallNose && tool.taper_half_angle_fact().is_unknown() {
        out.push(text(
            "Taper half-angle",
            LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG,
            "deg",
        ));
    }
    out
}

/// The entry contract of the op and the entries that the toolpath made.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryLineage {
    /// The contract of the registry row (K4).
    pub contract: EntryContract,
    /// One record per entry (`Produced::entries`).
    pub records: Vec<EntryRecord>,
}

impl EntryLineage {
    /// The count of the entries of each kind, in [`EntryKind::ALL`] order;
    /// a kind with no entry is absent.
    #[must_use]
    pub fn counts(&self) -> Vec<(EntryKind, usize)> {
        EntryKind::ALL
            .iter()
            .map(|k| (*k, self.records.iter().filter(|r| r.kind() == *k).count()))
            .filter(|(_, n)| *n > 0)
            .collect()
    }
}

/// One K6 finding of the generation.
#[derive(Debug, Clone, PartialEq)]
pub struct FindingUse {
    /// The diagnostic id.
    pub id: String,
    /// The consequence of its `diag_ids!` row. `None` for an id that is
    /// not a finding row.
    pub consequence: Option<Consequence>,
    /// The sentence of the finding, when the lineage writes it (K3
    /// `tool.fact_unknown`: which fact, and the default it planned at).
    pub detail: Option<String>,
}

/// What [`Lineage::of`] reads.
pub struct LineageInputs<'a> {
    /// The op, with the operator's values.
    pub operation: &'a OperationConfig,
    /// The tool.
    pub tool: &'a ToolConfig,
    /// The persisted feeds stamps of the toolpath.
    pub feeds_provenance: &'a FeedsProvenance,
    /// The entry contract of the registry row.
    pub entry_contract: EntryContract,
    /// The entries of the generated toolpath.
    pub entries: &'a [EntryRecord],
    /// The K5 rule outcomes of the feasibility stage.
    pub rules: Vec<RuleResult>,
    /// The ids of the generation findings.
    pub finding_ids: Vec<String>,
}

impl Lineage {
    /// Build the lineage of one generation. It reads stored values only;
    /// it runs no calculator.
    #[must_use]
    pub fn of(inputs: LineageInputs<'_>) -> Self {
        let LineageInputs {
            operation,
            tool,
            feeds_provenance: prov,
            entry_contract,
            entries,
            rules,
            finding_ids,
        } = inputs;
        let use_of = |value: Option<f64>, basis: Option<&ValueProvenance>| ValueUse {
            value,
            basis: basis.cloned(),
        };
        let speeds = SpeedBases {
            depth_per_pass: use_of(operation.depth_per_pass(), prov.depth_per_pass.as_ref()),
            stepover: use_of(operation.stepover(), prov.stepover.as_ref()),
            spindle_rpm: use_of(
                operation.spindle_rpm().map(f64::from),
                prov.spindle_rpm.as_ref(),
            ),
            feed_rate: use_of(operation.feed_rate(), prov.feed_rate.as_ref()),
            plunge_rate: use_of(operation.plunge_rate(), prov.plunge_rate.as_ref()),
            ramp_feed_rate: use_of(operation.ramp_feed_rate(), prov.ramp_feed_rate.as_ref()),
        };
        let picked = |u: &ValueUse| {
            u.basis
                .as_ref()
                .is_some_and(|b| b.reference.as_deref() == Some(LAW_PICKED))
                .then_some(u.value)
                .flatten()
        };
        let pick = match (picked(&speeds.depth_per_pass), picked(&speeds.stepover)) {
            (None, None) => None,
            (ap_mm, ae_mm) => Some(PickRecord { ap_mm, ae_mm }),
        };
        let mut seen: Vec<String> = Vec::new();
        let findings = finding_ids
            .into_iter()
            .filter(|id| {
                let fresh = !seen.contains(id);
                if fresh {
                    seen.push(id.clone());
                }
                fresh
            })
            .map(|id| FindingUse {
                consequence: consequence_of(&id),
                id,
                detail: None,
            })
            .chain(unknown_fact_texts(tool).into_iter().map(|text| FindingUse {
                id: crate::diagnostics::ids::TOOL_FACT_UNKNOWN.to_owned(),
                consequence: consequence_of(crate::diagnostics::ids::TOOL_FACT_UNKNOWN),
                detail: Some(text),
            }))
            .collect();
        Self {
            cut: operation.cut_spec(),
            pick,
            speeds,
            tool_facts: ToolFactKind::ALL
                .iter()
                .map(|f| FactUse {
                    fact: *f,
                    value: f.value_of(tool),
                    state: f.kind_of(tool),
                })
                .collect(),
            entry: EntryLineage {
                contract: entry_contract,
                records: entries.to_vec(),
            },
            findings,
            rules,
            measured: None,
        }
    }

    /// This lineage with what a fresh simulation measured (K7).
    #[must_use]
    pub fn with_measured(&self, measured: Option<MeasuredLineage>) -> Self {
        Self {
            measured,
            ..self.clone()
        }
    }

    /// A test door: the lineage of a result that no generation made (a
    /// hand-built result in a test). The cut is the cut of a default
    /// Pocket; every other section is empty. No production path calls it.
    #[doc(hidden)]
    #[must_use]
    pub fn unrecorded() -> Self {
        let op = OperationConfig::new_default(OperationType::Pocket);
        Self {
            cut: op.cut_spec(),
            pick: None,
            speeds: SpeedBases {
                depth_per_pass: ValueUse::NONE,
                stepover: ValueUse::NONE,
                spindle_rpm: ValueUse::NONE,
                feed_rate: ValueUse::NONE,
                plunge_rate: ValueUse::NONE,
                ramp_feed_rate: ValueUse::NONE,
            },
            tool_facts: Vec::new(),
            entry: EntryLineage {
                contract: op.op_type().registry_entry().entry,
                records: Vec::new(),
            },
            findings: Vec::new(),
            rules: Vec::new(),
            measured: None,
        }
    }

    /// The lineage as JSON: MCP `get_toolpath_lineage` and the CLI
    /// `summary.json` print it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "cut": cut_json(&self.cut),
            "pick": self.pick,
            "speeds": self.speeds,
            "tool_facts": self.tool_facts,
            "entry": {
                "contract": contract_name(self.entry.contract),
                "records": self.entry.counts().iter().map(|(k, n)| json!({
                    "kind": entry_kind_name(*k),
                    "count": n,
                })).collect::<Vec<_>>(),
            },
            "findings": self.findings.iter().map(|f| json!({
                "id": f.id,
                "consequence": consequence_name(f.consequence),
                "detail": f.detail,
            })).collect::<Vec<_>>(),
            "rules": self.rules.iter().map(|r| json!({
                "rule": format!("{:?}", r.rule()),
                "id": r.id(),
                "fact": r.fact(),
                "measured": r.measured(),
                "limit": r.limit(),
                "text": r.text(),
            })).collect::<Vec<_>>(),
            "measured": self.measured,
        })
    }
}

/// The consequence of the `diag_ids!` row of `id`; `None` for an id that is
/// not a finding row.
fn consequence_of(id: &str) -> Option<Consequence> {
    crate::diagnostics::ids::row_kind(id).and_then(|k| match k {
        RowKind::Finding(c) => Some(c),
        RowKind::NotReady => None,
    })
}

impl Serialize for Lineage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(serializer)
    }
}

impl ValueUse {
    const NONE: Self = Self {
        value: None,
        basis: None,
    };

    /// True when the stamp is a Suggest stamp that names no source (a
    /// formula, a floor or a repo rule).
    #[must_use]
    pub fn is_unsourced(&self) -> bool {
        self.basis.as_ref().is_some_and(|b| match b.source {
            ProvenanceSource::Formula
            | ProvenanceSource::EdgeRadiusFloor
            | ProvenanceSource::RepoRule => true,
            ProvenanceSource::VendorLut
            | ProvenanceSource::Manual
            | ProvenanceSource::AutoCorrect
            | ProvenanceSource::PublishedRule => false,
        })
    }
}

/// The name and the stated value of the radial cut.
#[must_use]
pub fn radial_text(r: Radial) -> (&'static str, Option<f64>) {
    match r {
        Radial::Width(s) => ("stepover", Some(s.mm())),
        Radial::Slot => ("slot", None),
        Radial::Groove => ("groove", None),
        Radial::Scallop(s) => ("scallop", Some(s.mm())),
        Radial::AngularStep(s) => ("angular step", Some(s.get())),
        Radial::SurfaceFollow => ("surface", None),
        Radial::Axial => ("none", None),
    }
}

/// The name and the stated value of the axial cut.
#[must_use]
pub fn axial_text(a: Axial) -> (&'static str, Option<f64>) {
    let name = match a {
        Axial::PerPass(_) => "depth per pass",
        Axial::ZStep(_) => "z step",
        Axial::OnePass(_) => "one pass",
        Axial::Peck(_) => "peck",
        Axial::HelixPitch(_) => "helix pitch",
        Axial::ChamferWidth(_) => "chamfer width",
        Axial::Surface { .. } => "surface",
    };
    (name, a.stated_mm())
}

fn cut_json(c: &CutSpec) -> Value {
    let (radial, radial_mm) = radial_text(c.radial);
    let (axial, axial_mm) = axial_text(c.axial);
    let total = match c.total {
        TotalDepth::Known(s) => json!({ "kind": "known", "mm": s.mm() }),
        TotalDepth::FromHeights => json!({ "kind": "from_heights" }),
        TotalDepth::Unset => json!({ "kind": "unset" }),
        TotalDepth::None(reason) => json!({ "kind": "none", "reason": reason }),
    };
    json!({
        "radial": { "kind": radial, "value": radial_mm },
        "axial": { "kind": axial, "value": axial_mm },
        "total": total,
        "first_pass": match c.first_pass {
            FirstPass::Slot => "slot",
            FirstPass::NotSlot => "not_slot",
        },
    })
}

/// The name of an entry contract kind.
#[must_use]
pub const fn contract_name(c: EntryContract) -> &'static str {
    match c {
        EntryContract::Dressup { .. } => "dressup",
        EntryContract::Planner { .. } => "planner",
        EntryContract::DescentIsCut => "descent_is_cut",
        EntryContract::StraightPlunge { .. } => "straight_plunge",
    }
}

/// The name of an entry kind.
#[must_use]
pub const fn entry_kind_name(k: EntryKind) -> &'static str {
    match k {
        EntryKind::Plunge => "plunge",
        EntryKind::Ramp => "ramp",
        EntryKind::Helix => "helix",
        EntryKind::PeckLadder => "peck_ladder",
    }
}

/// The name of a consequence: `warn_machine`, `warn_tool`, `warn_part`,
/// `info`, or `none` for an id that is not a finding row.
#[must_use]
pub const fn consequence_name(c: Option<Consequence>) -> &'static str {
    match c {
        Some(Consequence::Warn(Danger::Machine)) => "warn_machine",
        Some(Consequence::Warn(Danger::Tool)) => "warn_tool",
        Some(Consequence::Warn(Danger::Part)) => "warn_part",
        Some(Consequence::Info) => "info",
        None => "none",
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn an_unrecorded_lineage_names_a_cut_and_no_fact() {
        let l = Lineage::unrecorded();
        assert!(l.tool_facts.is_empty());
        assert!(l.pick.is_none());
        assert!(l.findings.is_empty());
    }

    #[test]
    fn a_picked_stamp_is_the_pick_record() {
        let op = OperationConfig::new_default(OperationType::Pocket);
        let tool = ToolConfig::new_default(
            crate::ids::ToolId::from_raw(1),
            crate::compute::tool_config::ToolType::EndMill,
        );
        let prov = FeedsProvenance {
            depth_per_pass: Some(ValueProvenance {
                source: ProvenanceSource::Manual,
                reference: Some(LAW_PICKED.to_owned()),
            }),
            ..FeedsProvenance::default()
        };
        let l = Lineage::of(LineageInputs {
            operation: &op,
            tool: &tool,
            feeds_provenance: &prov,
            entry_contract: op.op_type().registry_entry().entry,
            entries: &[],
            rules: Vec::new(),
            finding_ids: vec!["x".to_owned(), "x".to_owned()],
        });
        let pick = l.pick.expect("a picked depth is a pick");
        assert_eq!(pick.ap_mm, op.depth_per_pass());
        assert_eq!(pick.ae_mm, None);
        assert_eq!(l.findings.len(), 1, "an id is one row");
        assert_eq!(l.tool_facts.len(), ToolFactKind::ALL.len());
        assert_eq!(l.cut, op.cut_spec());
    }
}
