//! CONTRACT K8 (feeds contract P5b, ruling Q7): every generated toolpath
//! carries its lineage.
//!
//! [`Lineage`] is one record per generated toolpath. It names the facts
//! that the toolpath used and how each fact is known:
//!
//! - the cut of the op ([`CutSpec`], K1);
//! - the envelope pick, when the operator picked a point (K2b);
//! - the stored feeds and speeds with their stamps ([`SpeedBases`]);
//! - each tool fact that the op read ([`FactUse`]; the state is
//!   [`FactState::NotRecorded`] until P6 gives `ToolFact`);
//! - the entry contract and the entries made ([`EntryLineage`], K4);
//! - the K6 findings of the generation, each with its consequence;
//! - the K5 rule outcomes (empty until P7).
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
use crate::compute::tool_config::ToolConfig;
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
    /// The K5 rule outcomes. Empty until P7.
    pub rules: Vec<RuleOutcome>,
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
            Self::FluteCount => f64::from(tool.flute_count),
            Self::FluteLength => tool.cutting_length,
            Self::Stickout => tool.stickout,
        }
    }
}

/// How a tool fact is known. P6 (K3 `ToolFact`) adds the states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactState {
    /// The tool record holds the bare value; P6 records how it is known.
    NotRecorded,
}

/// One tool fact that the op read, with its bare value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FactUse {
    /// The fact.
    pub fact: ToolFactKind,
    /// The bare value.
    pub value: f64,
    /// How it is known.
    pub state: FactState,
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
}

/// The outcome of one K5 physical rule. P7 adds the variants; until then
/// the list is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOutcome {}

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
                consequence: crate::diagnostics::ids::row_kind(&id).and_then(|k| match k {
                    RowKind::Finding(c) => Some(c),
                    RowKind::NotReady => None,
                }),
                id,
            })
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
                    state: FactState::NotRecorded,
                })
                .collect(),
            entry: EntryLineage {
                contract: entry_contract,
                records: entries.to_vec(),
            },
            findings,
            rules: Vec::new(),
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
            })).collect::<Vec<_>>(),
            "rules": Vec::<Value>::new(),
        })
    }
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
