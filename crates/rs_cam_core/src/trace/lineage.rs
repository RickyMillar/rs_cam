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

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::compute::catalog::{
    Axial, CutSpec, EntryContract, EntryKind, EntryRecord, FirstPass, OperationConfig, Radial,
    TotalDepth,
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
    /// P13b (ruling Q7): each step that moved a feeds number, in order
    /// ([`StepRecord`]). The producers store them in
    /// `FeedsProvenance::steps` (the one store) when they write a value;
    /// the lineage is a view of it ([`current_steps`]: every chain, and one
    /// final record where a later write replaced the value).
    pub steps: Vec<StepRecord>,
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
    /// P29-10 (design item 11): the worst outside sample as one display
    /// row (`limits::Subject::MeasuredPoint`): the value that the crossed
    /// limit judges at the sample, against the limit there. `None` when
    /// no sample is outside or the placement judged no value.
    pub row: Option<crate::limits::CheckRow>,
}

impl MeasuredLineage {
    /// The lineage record of one placement.
    #[must_use]
    pub fn of(p: &crate::compute::simulate::EnvelopePlacement) -> Self {
        let binding = p.worst.and_then(|w| match w.placement {
            crate::feeds::envelope::Placement::Outside { binding, .. } => Some(binding),
            crate::feeds::envelope::Placement::Inside { .. } => None,
        });
        let judged = p.worst.and_then(|w| w.judged);
        let row = binding.zip(judged).map(|(binding, judged)| {
            crate::limits::CheckRow::measured(
                crate::limits::of_judged_value(
                    crate::limits::LimitId::of_limit_kind(binding),
                    // The sample is outside the binding edge, so the side
                    // of its value against the limit names the edge: above
                    // a ceiling, below a floor.
                    if judged.measured >= judged.limit {
                        crate::limits::BoundSide::Ceiling
                    } else {
                        crate::limits::BoundSide::Floor
                    },
                    &judged,
                ),
                crate::limits::Compared::of_judged_axis(judged.axis),
            )
        });
        Self {
            case: "measured",
            placed: p.placed,
            outside: p.outside,
            peak: p.peak,
            row,
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
    /// P10: the peck depth of a drill cycle (mm), with the stamp of its
    /// tool rule or the operator.
    pub peck_depth: ValueUse,
    /// P10: the `z_step` of Steep and Shallow and Unified Finish (mm).
    /// Waterline's `z_step` is its `depth_per_pass`.
    pub z_step: ValueUse,
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
            peck_depth: use_of(
                geometry_value(operation, "peck_depth"),
                prov.peck_depth.as_ref(),
            ),
            z_step: if matches!(operation, OperationConfig::Waterline(_)) {
                ValueUse::NONE
            } else {
                use_of(geometry_value(operation, "z_step"), prov.z_step.as_ref())
            },
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
            steps: current_steps(prov.steps.as_slice(), operation, prov),
        }
    }

    /// P13b: the stored lineage of an op that has no result: the stored
    /// values only (the step records of the project file included). The
    /// sections that a generation makes are empty. THE builder of
    /// `ProjectSession::stored_lineage` and the GUI inspector.
    #[must_use]
    pub fn stored(
        operation: &OperationConfig,
        tool: &ToolConfig,
        feeds_provenance: &FeedsProvenance,
    ) -> Self {
        Self::of(LineageInputs {
            operation,
            tool,
            feeds_provenance,
            entry_contract: operation.op_type().registry_entry().entry,
            entries: &[],
            rules: Vec::new(),
            finding_ids: Vec::new(),
        })
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
    /// Review L5 (P13b): it is behind the `test-support` feature.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    #[must_use]
    pub fn unrecorded() -> Self {
        let op = OperationConfig::new_default(crate::compute::catalog::OperationType::Pocket);
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
                peck_depth: ValueUse::NONE,
                z_step: ValueUse::NONE,
            },
            tool_facts: Vec::new(),
            entry: EntryLineage {
                contract: op.op_type().registry_entry().entry,
                records: Vec::new(),
            },
            findings: Vec::new(),
            rules: Vec::new(),
            measured: None,
            steps: Vec::new(),
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
            "steps": self.steps,
        })
    }
}

/// P13b (K8, ruling Q7): a feeds number that a step record follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepQuantity {
    /// The spindle speed (rev/min).
    Rpm,
    /// The chip load (mm/tooth).
    Chip,
    /// The side feed (mm/min).
    Feed,
    /// The axial depth (mm).
    Depth,
    /// The radial width (mm).
    Width,
    /// The plunge feed (mm/min).
    Plunge,
    /// The helix and ramp entry feed (mm/min).
    Ramp,
    /// The spindle power (kW).
    Power,
}

impl StepQuantity {
    /// Every quantity, in the order that the surfaces list them.
    pub const ALL: [Self; 8] = [
        Self::Rpm,
        Self::Chip,
        Self::Feed,
        Self::Depth,
        Self::Width,
        Self::Plunge,
        Self::Ramp,
        Self::Power,
    ];

    /// The name that the GUI prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rpm => "RPM",
            Self::Chip => "Chip",
            Self::Feed => "Feed",
            Self::Depth => "Depth",
            Self::Width => "Width",
            Self::Plunge => "Plunge",
            Self::Ramp => "Ramp",
            Self::Power => "Power",
        }
    }

    /// The unit of the value.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Rpm => "rpm",
            Self::Chip => "mm/tooth",
            Self::Feed | Self::Plunge | Self::Ramp => "mm/min",
            Self::Depth | Self::Width => "mm",
            Self::Power => "kW",
        }
    }

    /// The stored value of this quantity on `operation`. `None` for a
    /// quantity that the op does not store (the chip, the power) or a
    /// field that is not set.
    #[must_use]
    pub fn stored_value(self, operation: &OperationConfig) -> Option<f64> {
        match self {
            Self::Rpm => operation.spindle_rpm().map(f64::from),
            Self::Feed => operation.feed_rate(),
            Self::Depth => operation.depth_per_pass(),
            Self::Width => operation.stepover(),
            Self::Plunge => operation.plunge_rate(),
            Self::Ramp => operation.ramp_feed_rate(),
            Self::Chip | Self::Power => None,
        }
    }

    /// True when the op stores this quantity in a field, so a record chain
    /// can be checked against the stored value.
    #[must_use]
    pub const fn is_stored(self) -> bool {
        match self {
            Self::Rpm | Self::Feed | Self::Depth | Self::Width | Self::Plunge | Self::Ramp => true,
            Self::Chip | Self::Power => false,
        }
    }
}

/// P13b: the stored form of a [`crate::feeds::Basis`]. It has the same
/// JSON shape as the `Basis` serialization (`kind` and the fields of the
/// kind), and it owns its text, so a project file can hold it and load it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BasisRecord {
    /// The operator: a stated value or a pick.
    Operator {
        /// `stated` or `picked`.
        law: String,
    },
    /// A printed vendor row cell.
    Printed {
        /// The observation id.
        row: String,
    },
    /// A claim on a vendor row.
    Claim {
        /// The claim.
        claim: ClaimRecord,
    },
    /// A named F201 rule.
    Rule {
        /// The registry name.
        name: String,
        /// The value of the rule.
        value: f64,
        /// `cited` or `unsourced`.
        source: String,
    },
    /// A field of the machine profile.
    Machine {
        /// The field name.
        field: String,
    },
    /// A tool fact and its kind (K3).
    ToolFact {
        /// The fact name.
        fact: String,
        /// The kind label.
        fact_kind: String,
        /// The legacy row that an `Unknown` fact plans at.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_row: Option<Box<BasisRecord>>,
    },
    /// A law over other bases.
    Derived {
        /// The law, in a few words.
        law: String,
        /// The inputs of the law, first the one that decides the value.
        from: Vec<BasisRecord>,
    },
}

/// The stored form of a claim on a vendor row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    /// The claim kind (`size_g1`, `hardness_g2`, ...).
    pub kind: String,
    /// The row that the claim reads.
    pub row: String,
    /// The rule id.
    pub rule: String,
}

impl BasisRecord {
    /// The stored form of `basis`.
    #[must_use]
    pub fn of(basis: &crate::feeds::Basis) -> Self {
        use crate::feeds::Basis;
        use crate::material::ConstantSource;
        let rule = |c: &crate::material::FormulaConstant| Self::Rule {
            name: c.name.to_string(),
            value: c.value,
            source: match c.source {
                ConstantSource::Cited { .. } => "cited",
                ConstantSource::Unsourced { .. } => "unsourced",
            }
            .to_owned(),
        };
        match basis {
            Basis::Operator { law } => Self::Operator {
                law: (*law).to_owned(),
            },
            Basis::Printed(row) => Self::Printed {
                row: row.id().to_owned(),
            },
            Basis::Claim(c) => Self::Claim {
                claim: ClaimRecord {
                    kind: serde_json::to_value(c.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    row: c.row.id().to_owned(),
                    rule: c.rule.clone(),
                },
            },
            Basis::Rule(c) => rule(c),
            Basis::Machine(field) => Self::Machine {
                field: field.name().to_owned(),
            },
            Basis::ToolFact(t) => Self::ToolFact {
                fact: t.fact().name().to_owned(),
                fact_kind: t.kind().label().to_owned(),
                legacy_row: t.legacy_row().map(|c| Box::new(rule(c))),
            },
            Basis::Derived { law, from } => Self::Derived {
                law: (*law).to_owned(),
                from: from.iter().map(Self::of).collect(),
            },
        }
    }

    /// A short text of the basis for one GUI line: the kind and its name
    /// (the law of a derived basis, the rule name, the row id).
    #[must_use]
    pub fn short_text(&self) -> String {
        match self {
            Self::Operator { law } => format!("operator ({law})"),
            Self::Printed { row } => format!("vendor row {row}"),
            Self::Claim { claim } => format!("claim {} on {}", claim.rule, claim.row),
            Self::Rule { name, source, .. } => format!("rule {name} ({source})"),
            Self::Machine { field } => format!("machine {field}"),
            Self::ToolFact {
                fact, fact_kind, ..
            } => format!("{fact} ({fact_kind})"),
            Self::Derived { law, .. } => law.clone(),
        }
    }
}

/// P13b (K8, ruling Q7): one step that moved a feeds number: the value in,
/// the factor, the value out and the basis of the value out.
///
/// The feeds calculator (`feeds::calculator::calculate_with`) writes one
/// record per step call that changes a value. The apply funnel, the
/// envelope pick and the add-door rules write one record per value that
/// they set. `value_in` is `None` when the step sets the value from
/// nothing; `factor` is `value_out / value_in` when both are known and
/// `value_in` is not zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    /// The step: the function name of the producer (`step1_rpm`,
    /// `step7_feed_ceiling`, `apply_round_down`, `envelope_proposal`).
    pub step: String,
    /// The number that the step moved.
    pub quantity: StepQuantity,
    /// The value before the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_in: Option<f64>,
    /// `value_out / value_in`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
    /// The value after the step.
    pub value_out: f64,
    /// The basis of the value after the step.
    pub basis: BasisRecord,
}

impl StepRecord {
    /// The record of a step that moved `quantity` from `value_in` to
    /// `value_out` with `basis`.
    #[must_use]
    pub fn new(
        step: &str,
        quantity: StepQuantity,
        value_in: Option<f64>,
        value_out: f64,
        basis: BasisRecord,
    ) -> Self {
        let factor = value_in
            .filter(|v| *v != 0.0 && v.is_finite())
            .map(|v| value_out / v);
        Self {
            step: step.to_owned(),
            quantity,
            value_in,
            factor,
            value_out,
            basis,
        }
    }
}

/// P13b: the records of `steps` whose quantity is `q`, in order.
pub fn records_of(steps: &[StepRecord], q: StepQuantity) -> impl Iterator<Item = &StepRecord> {
    steps.iter().filter(move |r| r.quantity == q)
}

/// P13b: the step `current_steps` adds when a later write replaced the
/// value of the last record: a value that the operator typed.
pub const STEP_STATED_BY_OPERATOR: &str = "stated_by_operator";
/// P13b: the step `current_steps` adds when a later write that is not a
/// typed value (an automatic correction, a rule of another producer)
/// replaced the value of the last record.
pub const STEP_LATER_WRITE: &str = "later_write";

/// P13b (lead ruling 6): the records that describe the values `operation`
/// stores. Every chain stays. When the stored value of a quantity is not
/// the `value_out` of its last record, a later write replaced it, and the
/// view adds one final record: [`STEP_STATED_BY_OPERATOR`] with the basis
/// "stated" when the stamp is `Manual`, else [`STEP_LATER_WRITE`] with the
/// stamp as its law. The view writes nothing to the store.
#[must_use]
pub fn current_steps(
    steps: &[StepRecord],
    operation: &OperationConfig,
    prov: &FeedsProvenance,
) -> Vec<StepRecord> {
    let mut out = steps.to_vec();
    for q in StepQuantity::ALL {
        let Some(last) = records_of(steps, q).last() else {
            continue;
        };
        let Some(stored) = q.stored_value(operation) else {
            continue;
        };
        if last.value_out.to_bits() == stored.to_bits() {
            continue;
        }
        let stamp = match q {
            StepQuantity::Rpm => prov.spindle_rpm.as_ref(),
            StepQuantity::Feed => prov.feed_rate.as_ref(),
            StepQuantity::Depth => prov.depth_per_pass.as_ref(),
            StepQuantity::Width => prov.stepover.as_ref(),
            StepQuantity::Plunge => prov.plunge_rate.as_ref(),
            StepQuantity::Ramp => prov.ramp_feed_rate.as_ref(),
            StepQuantity::Chip | StepQuantity::Power => None,
        };
        let manual = stamp.is_none_or(|s| s.source == ProvenanceSource::Manual);
        let (step, basis) = if manual {
            (
                STEP_STATED_BY_OPERATOR,
                BasisRecord::of(&crate::feeds::Basis::stated()),
            )
        } else {
            let law = stamp.map_or_else(String::new, |s| {
                let source = serde_json::to_value(s.source)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                match &s.reference {
                    Some(r) => format!("a later write: {source} ({r})"),
                    None => format!("a later write: {source}"),
                }
            });
            (
                STEP_LATER_WRITE,
                BasisRecord::Derived {
                    law,
                    from: Vec::new(),
                },
            )
        };
        out.push(StepRecord::new(
            step,
            q,
            Some(last.value_out),
            stored,
            basis,
        ));
    }
    out
}

/// P13b (lead rulings 1 and 4): the step records of one toolpath, THE store
/// (`FeedsProvenance::steps`). The records are shared (`Arc`), so a clone
/// (an undo snapshot, a draft) copies no record. A producer replaces the
/// whole list ([`Self::replace`]); nothing appends to a stored list.
///
/// The project file stores them as one table (stored form C): each
/// distinct basis node once (`nodes`, the children by index, a child before
/// its parent), and each record names its basis by index (`records`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepRecords(std::sync::Arc<Vec<StepRecord>>);

impl StepRecords {
    /// The records.
    #[must_use]
    pub fn as_slice(&self) -> &[StepRecord] {
        &self.0
    }

    /// True when there is no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Replace the records with `records`.
    pub fn replace(&mut self, records: Vec<StepRecord>) {
        self.0 = std::sync::Arc::new(records);
    }

    /// The records with those of `q` replaced by `records` (in place of
    /// the old ones, at the end).
    pub fn replace_quantity(&mut self, q: StepQuantity, records: Vec<StepRecord>) {
        let mut all: Vec<StepRecord> = self.0.iter().filter(|r| r.quantity != q).cloned().collect();
        all.extend(records);
        self.replace(all);
    }

    /// The stored form C: the basis table and the records.
    #[must_use]
    pub fn to_table(&self) -> StepTable {
        let mut table = StepTable::default();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for r in self.0.iter() {
            let basis = table.intern(&r.basis, &mut index);
            table.records.push(StoredStep {
                step: r.step.clone(),
                quantity: r.quantity,
                value_in: r.value_in,
                factor: r.factor,
                value_out: r.value_out,
                basis,
            });
        }
        table
    }

    /// The records of the stored form C.
    ///
    /// # Errors
    ///
    /// A basis index that does not name an earlier node (a child must come
    /// before its parent, and a record names a node of the table).
    pub fn from_table(table: StepTable) -> Result<Self, String> {
        let mut built: Vec<BasisRecord> = Vec::with_capacity(table.nodes.len());
        for (i, node) in table.nodes.into_iter().enumerate() {
            let child = |k: usize| -> Result<BasisRecord, String> {
                if k < i {
                    built
                        .get(k)
                        .cloned()
                        .ok_or_else(|| format!("step basis node {i} names node {k}"))
                } else {
                    Err(format!(
                        "step basis node {i} names node {k}, not an earlier node"
                    ))
                }
            };
            let b = match node {
                BasisNode::Operator { law } => BasisRecord::Operator { law },
                BasisNode::Printed { row } => BasisRecord::Printed { row },
                BasisNode::Claim { claim } => BasisRecord::Claim { claim },
                BasisNode::Rule {
                    name,
                    value,
                    source,
                } => BasisRecord::Rule {
                    name,
                    value,
                    source,
                },
                BasisNode::Machine { field } => BasisRecord::Machine { field },
                BasisNode::ToolFact {
                    fact,
                    fact_kind,
                    legacy_row,
                } => BasisRecord::ToolFact {
                    fact,
                    fact_kind,
                    legacy_row: legacy_row.map(child).transpose()?.map(Box::new),
                },
                BasisNode::Derived { law, from } => BasisRecord::Derived {
                    law,
                    from: from.into_iter().map(child).collect::<Result<_, _>>()?,
                },
            };
            built.push(b);
        }
        let records = table
            .records
            .into_iter()
            .map(|r| {
                let basis = built
                    .get(r.basis)
                    .cloned()
                    .ok_or_else(|| format!("step `{}` names basis node {}", r.step, r.basis))?;
                Ok(StepRecord {
                    step: r.step,
                    quantity: r.quantity,
                    value_in: r.value_in,
                    factor: r.factor,
                    value_out: r.value_out,
                    basis,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self(std::sync::Arc::new(records)))
    }
}

impl Serialize for StepRecords {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_table().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StepRecords {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let table = StepTable::deserialize(deserializer)?;
        Self::from_table(table).map_err(serde::de::Error::custom)
    }
}

/// P13b: the stored form C of [`StepRecords`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepTable {
    /// Each distinct basis node once; a child comes before its parent.
    pub nodes: Vec<BasisNode>,
    /// The records, each naming its basis node by index.
    pub records: Vec<StoredStep>,
}

impl StepTable {
    /// The index of `b` in the table; its children first.
    fn intern(
        &mut self,
        b: &BasisRecord,
        index: &mut std::collections::HashMap<String, usize>,
    ) -> usize {
        let node = match b {
            BasisRecord::Operator { law } => BasisNode::Operator { law: law.clone() },
            BasisRecord::Printed { row } => BasisNode::Printed { row: row.clone() },
            BasisRecord::Claim { claim } => BasisNode::Claim {
                claim: claim.clone(),
            },
            BasisRecord::Rule {
                name,
                value,
                source,
            } => BasisNode::Rule {
                name: name.clone(),
                value: *value,
                source: source.clone(),
            },
            BasisRecord::Machine { field } => BasisNode::Machine {
                field: field.clone(),
            },
            BasisRecord::ToolFact {
                fact,
                fact_kind,
                legacy_row,
            } => BasisNode::ToolFact {
                fact: fact.clone(),
                fact_kind: fact_kind.clone(),
                legacy_row: legacy_row.as_deref().map(|l| self.intern(l, index)),
            },
            BasisRecord::Derived { law, from } => BasisNode::Derived {
                law: law.clone(),
                from: from.iter().map(|c| self.intern(c, index)).collect(),
            },
        };
        // The JSON text of a node is its key: two equal nodes (children by
        // index) are one entry.
        let key = serde_json::to_string(&node).unwrap_or_default();
        if let Some(i) = index.get(&key) {
            return *i;
        }
        let i = self.nodes.len();
        self.nodes.push(node);
        index.insert(key, i);
        i
    }
}

/// P13b: one basis node of a [`StepTable`]: a [`BasisRecord`] whose
/// children are indices into the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BasisNode {
    /// The operator.
    Operator {
        /// `stated` or `picked`.
        law: String,
    },
    /// A printed vendor row.
    Printed {
        /// The observation id.
        row: String,
    },
    /// A claim on a vendor row.
    Claim {
        /// The claim.
        claim: ClaimRecord,
    },
    /// A named F201 rule.
    Rule {
        /// The registry name.
        name: String,
        /// The value.
        value: f64,
        /// `cited` or `unsourced`.
        source: String,
    },
    /// A field of the machine profile.
    Machine {
        /// The field name.
        field: String,
    },
    /// A tool fact.
    ToolFact {
        /// The fact name.
        fact: String,
        /// The kind label.
        fact_kind: String,
        /// The node of the legacy row.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_row: Option<usize>,
    },
    /// A law over other nodes.
    Derived {
        /// The law.
        law: String,
        /// The nodes of the inputs.
        from: Vec<usize>,
    },
}

/// P13b: one record of a [`StepTable`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredStep {
    /// The step.
    pub step: String,
    /// The quantity.
    pub quantity: StepQuantity,
    /// The value before the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_in: Option<f64>,
    /// `value_out / value_in`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
    /// The value after the step.
    pub value_out: f64,
    /// The index of the basis node.
    pub basis: usize,
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
        Radial::Width(s) => ("stepover", s.map(|s| s.mm())),
        Radial::Slot => ("slot", None),
        Radial::Groove => ("groove", None),
        Radial::Scallop(s) => ("scallop", Some(s.mm())),
        Radial::AngularStep(s) => ("angular step", Some(s.get())),
        Radial::SurfaceFollow => ("surface", None),
        Radial::Axial => ("none", None),
    }
}

/// P10: the value of the tool-derived field `field` of `operation`, when
/// the op reads it.
fn geometry_value(operation: &OperationConfig, field: &str) -> Option<f64> {
    crate::compute::geometry_default::read_fields(operation)
        .into_iter()
        .find(|(name, _)| *name == field)
        .and_then(|(_, value)| value)
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
        Axial::SurfaceOffset(_) => "surface offset",
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
    use crate::compute::catalog::OperationType;

    /// P13b: the stored basis has the JSON shape of the live basis, and a
    /// record survives a TOML round trip.
    #[test]
    fn a_basis_record_keeps_the_basis_json_and_round_trips() {
        let mut op = OperationConfig::new_default(OperationType::Pocket);
        crate::compute::fixture_values::fill_unset_geometry(&mut op);
        let mut tool = ToolConfig::new_default(
            crate::ids::ToolId::from_raw(1),
            crate::compute::tool_config::ToolType::EndMill,
        );
        tool.set_flute_count(crate::compute::tool_config::ToolFact::Unknown);
        let machine = crate::machine::MachineProfile::generic_wood_router();
        let material = crate::material::Material::SolidWood {
            species: crate::material::WoodSpecies::WhiteOak,
        };
        let r = crate::feeds::calculate(&crate::feeds::FeedsInput::of_operation(
            &op,
            &tool,
            &material,
            &machine,
            crate::feeds::embedded_vendor_lut(),
            crate::feeds::SpindleStrategy::MatchChart,
        ));
        let b = &r.bases;
        for basis in [
            &b.rpm,
            &b.chip,
            &b.feed,
            &b.plunge,
            &b.ramp,
            &b.flute_count,
            &b.ap,
            &b.ae,
            &b.power,
        ] {
            assert_eq!(
                serde_json::to_value(BasisRecord::of(basis)).expect("json"),
                serde_json::to_value(basis).expect("json"),
            );
        }
        #[derive(Serialize, Deserialize)]
        struct Wrap {
            steps: Vec<StepRecord>,
        }
        assert!(!r.steps.is_empty());
        let text = toml::to_string(&Wrap {
            steps: r.steps.clone(),
        })
        .expect("toml");
        let back: Wrap = toml::from_str(&text).expect("toml back");
        assert_eq!(back.steps, r.steps);
    }

    #[test]
    fn an_unrecorded_lineage_names_a_cut_and_no_fact() {
        let l = Lineage::unrecorded();
        assert!(l.tool_facts.is_empty());
        assert!(l.pick.is_none());
        assert!(l.findings.is_empty());
    }

    #[test]
    fn a_picked_stamp_is_the_pick_record() {
        // P10: the fixture states the depth the registry held before P10.
        let mut op = OperationConfig::new_default(OperationType::Pocket);
        crate::compute::fixture_values::fill_unset_geometry(&mut op);
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
