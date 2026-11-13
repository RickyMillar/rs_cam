//! CONTRACT K2 (feeds contract P5): every number that Suggest ships names
//! its source.
//!
//! A [`Sourced`] value holds a value and its [`Basis`]. No code outside
//! `feeds/` can make one: the constructor is `pub(in crate::feeds)`, so the
//! function that owns a rule is the only builder of its value (as
//! `plunge::resolve_plunge` owns the plunge). A [`Basis`] is a vendor row,
//! a claim on a row, a named rule (Cited or Unsourced, F201), the
//! operator, the machine profile, a tool fact with its kind (P13a), or a
//! law over other bases.
//!
//! The unit newtypes ([`Rpm`], [`FeedMmMin`], [`PlungeMmMin`], [`ChipMm`])
//! keep one quantity from the place of another: a plunge cannot take a
//! side feed by mistake.
//!
//! The refusals of the compiler are `compile_fail` doctests on
//! [`sourced_refusals`].

use serde::Serialize;
use serde::ser::SerializeMap;

use crate::compute::tool_config::FactKind;
use crate::material::{ConstantSource, FormulaConstant};

/// One vendor row: the observation id of the embedded LUT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RowRef(pub String);

impl RowRef {
    /// The observation id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0
    }
}

/// The claim kinds of the extrapolation and entry programmes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// G1: a size claim scales the row to the tool diameter.
    SizeG1,
    /// G2: a hardness transfer scales the row to the material.
    HardnessG2,
    /// G3: a family rule carries the row to another operation family.
    FamilyG3,
    /// G6: the drill rule reads a side row for a plunge.
    DrillG6,
    /// G10: the plunge is a printed fraction of the side feed.
    PlungeG10,
}

/// A claim on a vendor row: the kind, the row and the rule id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimRef {
    /// The kind of the claim.
    pub kind: ClaimKind,
    /// The row that the claim reads.
    pub row: RowRef,
    /// The rule id (for example `g10_plunge_flat`), or the kind name when
    /// the claim has no id of its own.
    pub rule: String,
}

/// A field of the machine profile (lead decision D3). The operator sets
/// these values in the machine file; no vendor row prints them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineField {
    /// `MachineProfile::rpm_range` and the spindle speed list.
    SpindleRpm,
    /// `MachineProfile::power_at_rpm`: the rated spindle curve.
    PowerCurve,
    /// `MachineProfile::cutting_feed_ceiling_mm_min`: the feed cap.
    CuttingFeedCap,
    /// `MachineProfile::rigidity`: the depth caps and adaptive factors.
    Rigidity,
    /// `MachineProfile::z_axis_rate_mm_min` (GRBL `$112`): the vertical
    /// rate cap (P20b).
    ZAxisRate,
}

impl MachineField {
    /// The field name that the card and MCP print.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SpindleRpm => "machine.rpm_range",
            Self::PowerCurve => "machine.power_at_rpm",
            Self::CuttingFeedCap => "machine.cutting_feed_ceiling_mm_min",
            Self::Rigidity => "machine.rigidity",
            Self::ZAxisRate => "machine.kinematics.max_rate_xyz_mm_min[2]",
        }
    }
}

/// The law of a value that the operator states in the op config.
pub const LAW_STATED: &str = "stated";
/// The law of a geometry value that the operator picked on the envelope.
pub const LAW_PICKED: &str = "picked";
/// A tool fact that a value reads (CONTRACT K3, feeds contract P13a,
/// review H3). The diameter and the corner radius are plain fields of the
/// tool, so they are always stated; the other facts carry a `FactKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFactName {
    /// The nominal diameter.
    Diameter,
    /// The corner radius of a bull nose.
    CornerRadius,
    /// The flute count.
    FluteCount,
    /// The flute (cutting) length.
    CuttingLength,
    /// The shank diameter.
    ShankDiameter,
    /// The stick-out.
    Stickout,
    /// The taper half-angle.
    TaperHalfAngle,
}

impl ToolFactName {
    /// The name that the card and MCP print.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Diameter => "tool.diameter",
            Self::CornerRadius => "tool.corner_radius",
            Self::FluteCount => "tool.flute_count",
            Self::CuttingLength => "tool.cutting_length",
            Self::ShankDiameter => "tool.shank_diameter",
            Self::Stickout => "tool.stickout",
            Self::TaperHalfAngle => "tool.taper_half_angle",
        }
    }

    /// The legacy loader default that an `Unknown` fact plans at
    /// (`compute::tool_config::LEGACY_DEFAULT_*`, an F201 row). The
    /// diameter and the corner radius have none.
    #[must_use]
    pub const fn legacy_row(self) -> Option<FormulaConstant> {
        use crate::compute::tool_config as tc;
        match self {
            Self::Diameter | Self::CornerRadius => None,
            Self::FluteCount => Some(tc::LEGACY_DEFAULT_FLUTE_COUNT_ROW),
            Self::CuttingLength => Some(tc::LEGACY_DEFAULT_CUTTING_LENGTH_MM_ROW),
            Self::ShankDiameter => Some(tc::LEGACY_DEFAULT_SHANK_DIAMETER_MM_ROW),
            Self::Stickout => Some(tc::LEGACY_DEFAULT_STICKOUT_MM_ROW),
            Self::TaperHalfAngle => Some(tc::LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG_ROW),
        }
    }
}

/// The basis of a tool fact: the fact, its kind, and the legacy row that
/// the value plans at when the kind is `Unknown`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolFactBasis {
    fact: ToolFactName,
    kind: FactKind,
    legacy: Option<FormulaConstant>,
}

impl ToolFactBasis {
    /// The fact.
    #[must_use]
    pub const fn fact(&self) -> ToolFactName {
        self.fact
    }

    /// How the fact is known.
    #[must_use]
    pub const fn kind(&self) -> FactKind {
        self.kind
    }

    /// The legacy loader default that the value plans at: `Some` exactly
    /// when the kind is `Unknown` and the fact has a default.
    #[must_use]
    pub const fn legacy_row(&self) -> Option<&FormulaConstant> {
        self.legacy.as_ref()
    }
}

/// The kinds of the tool facts that the calculator reads (feeds contract
/// P13a). `FeedsInput::of_operation` reads them from the tool; a
/// caller that states each value (a test fixture) uses [`Self::STATED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolFactKinds {
    /// The kind of the flute count.
    pub flute_count: FactKind,
    /// The kind of the flute (cutting) length.
    pub cutting_length: FactKind,
}

impl ToolFactKinds {
    /// Every fact stated.
    pub const STATED: Self = Self {
        flute_count: FactKind::Stated,
        cutting_length: FactKind::Stated,
    };

    /// The kinds of the facts of `tool`.
    #[must_use]
    pub fn of_tool(tool: &crate::compute::tool_config::ToolConfig) -> Self {
        Self {
            flute_count: tool.flute_count_fact().kind(),
            cutting_length: tool.cutting_length_fact().kind(),
        }
    }
}

/// P29-2: the law of the basis of a drill gate's envelope
/// (`limits::basis_of_bound`); [`Basis::setting`] reads it.
pub const DRILL_ENVELOPE_LAW: &str = "drill envelope of the material";

/// How one shipped value is known. CONTRACT K2.
#[derive(Debug, Clone, PartialEq)]
pub enum Basis {
    /// The operator: a `Stated` value of the `CutSpec`, or a pick on the
    /// envelope. `law` is [`LAW_STATED`] or [`LAW_PICKED`].
    Operator {
        /// [`LAW_STATED`] or [`LAW_PICKED`].
        law: &'static str,
    },
    /// A printed vendor row cell.
    Printed(RowRef),
    /// A claim (G1/G2/G3/G6/G10) on a vendor row.
    Claim(ClaimRef),
    /// A named rule of `feeds::formula_constants` (F201): Cited or
    /// Unsourced. The row is a copy, so a computed row name (the family
    /// table) needs no static item.
    Rule(FormulaConstant),
    /// A field of the machine profile (lead decision D3).
    Machine(MachineField),
    /// A tool fact and its kind (K3, P13a). An `Unknown` fact carries the
    /// legacy row that the value plans at.
    ToolFact(ToolFactBasis),
    /// A law over other bases.
    Derived {
        /// The law, in a few words.
        law: &'static str,
        /// The inputs of the law, first the one that decides the value.
        from: Vec<Basis>,
    },
}

impl Basis {
    /// P29-2: the setting an operator changes to move a bound with this
    /// basis: `"machine"`, `"tool"`, `"material"`, `"vendor row"` or
    /// `"operation"`. It equals `tool_load::verdict::BoundSource::setting`
    /// for each basis that `limits::basis_of_bound` makes (sentry
    /// `every_limit_is_a_limit_id_p29_2`). A constant (`Rule`) gives
    /// `"tool"`: no setting moves the bound, the tool moves the reading
    /// under it.
    #[must_use]
    pub fn setting(&self) -> &'static str {
        match self {
            Self::Operator { .. } => "operation",
            Self::Printed(_) | Self::Claim(_) => "vendor row",
            Self::Machine(_) => "machine",
            Self::Rule(_) | Self::ToolFact(_) => "tool",
            Self::Derived { law, .. } if *law == DRILL_ENVELOPE_LAW => "material",
            Self::Derived { .. } => "tool",
        }
    }

    /// P29-2: `false` when a bound with this basis must not refuse an
    /// export: the machine rigidity values (the force budget of a machine
    /// class), as
    /// `tool_load::verdict::BoundSource::gates_export`. P29-10: a law over
    /// such a value (the envelope force budget, `feeds::rigidity_basis`)
    /// does not gate either, so a planned row and a gate row of one limit
    /// take one tone.
    #[must_use]
    pub fn gates_export(&self) -> bool {
        match self {
            Self::Machine(MachineField::Rigidity) => false,
            Self::Derived { from, .. } => from.iter().all(Self::gates_export),
            Self::Operator { .. }
            | Self::Printed(_)
            | Self::Claim(_)
            | Self::Machine(_)
            | Self::Rule(_)
            | Self::ToolFact(_) => true,
        }
    }

    /// A value that the operator states.
    #[must_use]
    pub const fn stated() -> Self {
        Self::Operator { law: LAW_STATED }
    }

    /// A geometry value that the operator picked on the envelope.
    #[must_use]
    pub const fn picked() -> Self {
        Self::Operator { law: LAW_PICKED }
    }

    /// A tool fact of kind `kind` (P13a, review H3). An `Unknown` fact
    /// carries the legacy row of the fact, so `unsourced_rules` reaches it.
    #[must_use]
    pub const fn tool_fact(fact: ToolFactName, kind: FactKind) -> Self {
        let legacy = match kind {
            FactKind::Unknown => fact.legacy_row(),
            FactKind::Stated | FactKind::Catalogue | FactKind::Estimated => None,
        };
        Self::ToolFact(ToolFactBasis { fact, kind, legacy })
    }

    /// The tool diameter: a plain field of the tool, always stated.
    #[must_use]
    pub const fn tool_diameter() -> Self {
        Self::tool_fact(ToolFactName::Diameter, FactKind::Stated)
    }

    /// A law over `from`.
    #[must_use]
    pub const fn derived(law: &'static str, from: Vec<Self>) -> Self {
        Self::Derived { law, from }
    }

    /// The Unsourced rules that this basis reads, in tree order, with no
    /// repeat. A `Recommendation` is `Partial` while one of its written
    /// fields reads an Unsourced rule.
    #[must_use]
    pub fn unsourced_rules(&self) -> Vec<&FormulaConstant> {
        let mut out: Vec<&FormulaConstant> = Vec::new();
        self.collect_unsourced(&mut out);
        out
    }

    fn collect_unsourced<'a>(&'a self, out: &mut Vec<&'a FormulaConstant>) {
        match self {
            Self::Rule(c) => {
                if matches!(c.source, ConstantSource::Unsourced { .. })
                    && !out.iter().any(|o| o.name == c.name)
                {
                    out.push(c);
                }
            }
            Self::ToolFact(t) => {
                if let Some(c) = &t.legacy
                    && matches!(c.source, ConstantSource::Unsourced { .. })
                    && !out.iter().any(|o| o.name == c.name)
                {
                    out.push(c);
                }
            }
            Self::Derived { from, .. } => {
                for b in from {
                    b.collect_unsourced(out);
                }
            }
            Self::Operator { .. } | Self::Printed(_) | Self::Claim(_) | Self::Machine(_) => {}
        }
    }

    /// The vendor row that decides this value, if one does: a printed row,
    /// or the row of a claim. A `Derived` value reads its first input.
    #[must_use]
    pub fn deciding_row(&self) -> Option<&RowRef> {
        match self {
            Self::Printed(row) => Some(row),
            Self::Claim(c) => Some(&c.row),
            Self::Derived { from, .. } => from.first().and_then(Self::deciding_row),
            Self::Operator { .. } | Self::Rule(_) | Self::Machine(_) | Self::ToolFact(_) => None,
        }
    }

    /// The kind token that MCP and the card print.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Operator { .. } => "operator",
            Self::Printed(_) => "printed",
            Self::Claim(_) => "claim",
            Self::Rule(_) => "rule",
            Self::Machine(_) => "machine",
            Self::ToolFact(_) => "tool_fact",
            Self::Derived { .. } => "derived",
        }
    }
}

impl Serialize for Basis {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", self.kind())?;
        match self {
            Self::Operator { law } => map.serialize_entry("law", law)?,
            Self::Printed(row) => map.serialize_entry("row", row)?,
            Self::Claim(c) => map.serialize_entry("claim", c)?,
            Self::Rule(c) => {
                map.serialize_entry("name", c.name.as_ref())?;
                map.serialize_entry("value", &c.value)?;
                let source = match c.source {
                    ConstantSource::Cited { .. } => "cited",
                    ConstantSource::Unsourced { .. } => "unsourced",
                };
                map.serialize_entry("source", source)?;
            }
            Self::Machine(field) => map.serialize_entry("field", field.name())?,
            Self::ToolFact(t) => {
                map.serialize_entry("fact", t.fact.name())?;
                map.serialize_entry("fact_kind", t.kind.label())?;
                if let Some(row) = &t.legacy {
                    map.serialize_entry("legacy_row", &Self::Rule(row.clone()))?;
                }
            }
            Self::Derived { law, from } => {
                map.serialize_entry("law", law)?;
                map.serialize_entry("from", from)?;
            }
        }
        map.end()
    }
}

/// A value and its basis. CONTRACT K2: no code outside `feeds/` makes one,
/// and no method changes the value after it is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced<T> {
    value: T,
    basis: Basis,
}

impl<T> Sourced<T> {
    /// A value with its basis. Only the owner of a rule (a function in
    /// `feeds/`) calls this.
    pub(in crate::feeds) const fn new(value: T, basis: Basis) -> Self {
        Self { value, basis }
    }

    /// The value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// The basis.
    pub const fn basis(&self) -> &Basis {
        &self.basis
    }
}

impl<T: Copy> Sourced<T> {
    /// The value, by copy.
    #[must_use]
    pub const fn get(&self) -> T {
        self.value
    }
}

impl<T: Serialize> Serialize for Sourced<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("value", &self.value)?;
        map.serialize_entry("basis", &self.basis)?;
        map.end()
    }
}

/// A spindle speed, rev/min.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Rpm(pub f64);

/// A side (cutting) feed, mm/min.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FeedMmMin(pub f64);

/// A plunge feed, mm/min.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PlungeMmMin(pub f64);

/// An advance per tooth, mm.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ChipMm(pub f64);

/// The basis of each value that one `calculate` point ships. Each step of
/// `calculate` that writes a field writes its basis in the same statement.
#[derive(Debug, Clone, PartialEq)]
pub struct PointBases {
    /// The spindle speed.
    pub rpm: Basis,
    /// The advance per tooth.
    pub chip: Basis,
    /// The side feed.
    pub feed: Basis,
    /// The plunge feed.
    pub plunge: Basis,
    /// The ramp feed (the G6 chip, or the plunge it falls back to).
    pub ramp: Basis,
    /// The flute count Z of the feed `chip x RPM x Z` (P13a: a tool fact
    /// with its kind).
    pub flute_count: Basis,
    /// The axial depth of the point.
    pub ap: Basis,
    /// The radial width of the point.
    pub ae: Basis,
    /// The spindle power at the point.
    pub power: Basis,
}

impl PointBases {
    /// The bases of a point that no step computed: every field is a law
    /// with no input. A result literal in a test uses it; `calculate`
    /// writes each field.
    #[must_use]
    pub fn not_computed() -> Self {
        let none = || Basis::derived("not computed", Vec::new());
        Self {
            rpm: none(),
            chip: none(),
            feed: none(),
            plunge: none(),
            ramp: none(),
            flute_count: none(),
            ap: none(),
            ae: none(),
            power: none(),
        }
    }
}

/// Lower a value to a cap. CONTRACT K2: this is the one cap API, and it
/// returns `min(value, cap)` only. A cap above the value gives the value
/// back with its own basis; no call raises it.
#[must_use]
pub(in crate::feeds) fn lower_to_cap(
    value: Sourced<f64>,
    cap_mm: f64,
    law: &'static str,
    cap_basis: &Basis,
) -> Sourced<f64> {
    if cap_mm.is_finite() && cap_mm < value.value {
        let from = vec![value.basis, cap_basis.clone()];
        Sourced::new(cap_mm, Basis::derived(law, from))
    } else {
        value
    }
}

/// The K2 refusals of the compiler. Each `compile_fail` block has a
/// sibling block that compiles.
///
/// Code outside `feeds/` cannot make a sourced value from a bare number:
///
/// ```compile_fail
/// use rs_cam_core::feeds::sourced::{Basis, Rpm, Sourced};
/// let _ = Sourced::new(Rpm(18_000.0), Basis::stated());
/// ```
///
/// No method changes the value of a sourced value (the field is private):
///
/// ```compile_fail
/// use rs_cam_core::feeds::sourced::Sourced;
/// fn raise(s: &mut Sourced<f64>) {
///     s.value = 14.047;
/// }
/// ```
///
/// The sibling: code outside `feeds/` reads a value and its basis.
///
/// ```
/// use rs_cam_core::feeds::sourced::{Basis, Sourced};
/// fn read(s: &Sourced<f64>) -> (f64, &Basis) {
///     (s.get(), s.basis())
/// }
/// ```
///
/// `apply` takes only a `Complete` recommendation; a `Partial` one does
/// not compile:
///
/// ```compile_fail
/// use rs_cam_core::compute::catalog::OperationConfig;
/// use rs_cam_core::feeds::FeedsProvenance;
/// use rs_cam_core::feeds::suggest::{ApplyContext, Partial, Recommendation, apply};
/// fn write(
///     rec: &Recommendation<'_, Partial>,
///     op: &mut OperationConfig,
///     prov: &mut FeedsProvenance,
///     ctx: ApplyContext<'_>,
/// ) {
///     let _ = apply(rec, op, prov, ctx);
/// }
/// ```
///
/// The sibling: the apply door accepts the Unsourced rows (lead decision
/// D4), and the `Complete` recommendation applies.
///
/// ```
/// use rs_cam_core::compute::catalog::OperationConfig;
/// use rs_cam_core::feeds::FeedsProvenance;
/// use rs_cam_core::feeds::suggest::{ApplyContext, Partial, Recommendation, apply};
/// fn write(
///     rec: Recommendation<'_, Partial>,
///     op: &mut OperationConfig,
///     prov: &mut FeedsProvenance,
///     ctx: ApplyContext<'_>,
/// ) {
///     let (complete, _warning) = rec.accept_unsourced();
///     let _ = apply(&complete, op, prov, ctx);
/// }
/// ```
///
/// A recommendation is not built from a bare result (no public
/// constructor):
///
/// ```compile_fail
/// use rs_cam_core::feeds::FeedsResult;
/// use rs_cam_core::feeds::suggest::{Complete, Recommendation};
/// fn make(r: &FeedsResult) -> Recommendation<'_, Complete> {
///     Recommendation::validated(r).accept_unsourced().0
/// }
/// ```
#[doc(hidden)]
pub mod sourced_refusals {}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_api_never_raises_a_stated_value() {
        let stated = Sourced::new(9.0, Basis::stated());
        let cap = Basis::tool_diameter();
        let lowered = lower_to_cap(stated.clone(), 16.0, "flute guard", &cap);
        assert_eq!(lowered, stated, "a cap above the value keeps it");
        let lowered = lower_to_cap(stated, 4.0, "flute guard", &cap);
        assert!((lowered.get() - 4.0).abs() < 1e-12);
        assert_eq!(lowered.basis().kind(), "derived");
    }

    #[test]
    fn unsourced_rules_walk_the_tree_once_each() {
        let rule = FormulaConstant::unsourced("feeds.x", 0.8, "no record");
        let b = Basis::derived(
            "law",
            vec![
                Basis::Rule(rule.clone()),
                Basis::derived("inner", vec![Basis::Rule(rule), Basis::stated()]),
            ],
        );
        let rows = b.unsourced_rules();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.first().unwrap().name, "feeds.x");
    }

    #[test]
    fn an_unknown_tool_fact_reads_its_legacy_row() {
        let unknown = Basis::tool_fact(ToolFactName::FluteCount, FactKind::Unknown);
        let rows = unknown.unsourced_rules();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows.first().unwrap().name,
            "compute.tool_config.LEGACY_DEFAULT_FLUTE_COUNT"
        );
        for kind in [FactKind::Stated, FactKind::Catalogue, FactKind::Estimated] {
            let known = Basis::tool_fact(ToolFactName::FluteCount, kind);
            assert!(known.unsourced_rules().is_empty(), "{kind:?}");
        }
        assert!(
            Basis::tool_fact(ToolFactName::Diameter, FactKind::Unknown)
                .unsourced_rules()
                .is_empty(),
            "the diameter has no legacy row"
        );
    }
}
