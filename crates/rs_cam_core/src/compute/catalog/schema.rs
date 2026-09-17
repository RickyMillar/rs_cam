//! Schema types that describe one operation parameter, and the registry
//! row type that carries them.
//!
//! Split out of `compute/catalog.rs` (P4). Every item keeps its public
//! path through the parent's `pub use`.

use serde::{Deserialize, Serialize};

use crate::feeds::CutterKind;

use super::{OperationSpec, OperationType};

/// Lightweight per-field hint included beside `get_toolpath_params`.
///
/// Stays `pub`: `OperationConfig::param_schema_hints` returns it, so a
/// crate-private form raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamHint {
    #[serde(rename = "type")]
    pub type_name: String,
    pub required: bool,
    pub optional: bool,
    pub default: serde_json::Value,
}

/// One operation parameter entry returned by `operation_schema`.
///
/// **Test door.** Stays `pub` for two reasons: it is the element type of the
/// `pub` field `OperationSchema::params`, and the harnesses
/// `tests/radial_finish_ranges_n10.rs` and
/// `tests/unified_finish_planner_dials_f2.rs` bind it (S29, 2026-09-16).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParamSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub optional: bool,
    pub default: serde_json::Value,
    pub range: Option<serde_json::Value>,
    pub description: Option<String>,
    /// Other names the setter accepts for this same field (CMP-08).
    /// Usually empty. An alias writes the field this row names; it is not
    /// a second dial.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConstraints {
    pub required_tool_type: Vec<String>,
    pub supports_v_bit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSchema {
    pub operation_type: String,
    pub label: String,
    pub params: Vec<OperationParamSchema>,
    /// Names `set_toolpath_param` accepts that are NOT operation
    /// parameters — they write toolpath state instead of the operation
    /// config, so no `param_defs` array holds them (CMP-08).
    ///
    /// The same for every operation. Published here rather than mixed
    /// into [`Self::params`], because a caller reading `params` is
    /// reading the operation's own dials and `debug_enabled` is not one.
    pub toolpath_params: Vec<OperationParamSchema>,
    pub tool_constraints: ToolConstraints,
}

/// The settable names that are not operation parameters (CMP-08).
///
/// `debug_enabled` writes `ToolpathConfig::debug_options`, not the
/// operation config, so the generic serde arm of `set_toolpath_param`
/// never sees it and no `param_defs` array can hold it. Before this row
/// it was a fourth accepted name that the published schema did not
/// mention at all.
pub const TOOLPATH_PARAM_DEFS: &[ParamDef] = &[ParamDef::required("debug_enabled", "bool")];

/// The numeric domain a settable parameter is declared to accept.
///
/// # What `None` on [`ParamDef::range`] means (DR-LIVE, 2026-08-14)
///
/// **Not measured, not "unbounded".** Almost every entry in the registry
/// still carries `None`, because nobody has stated that param's domain —
/// exactly the state `peck_depth` was in when
/// `TECH_DEBT_2_CLOSEOUT.md` §4.4 ledgered DR-LIVE: `ParamDef::required
/// ("peck_depth", "f64")` advertised no domain at all, so MCP
/// `set_toolpath_param` accepted `0` and `-3` on a param whose emitter
/// refuses both. (Non-finite never got that far — JSON has no `NaN`
/// literal and the string path fails serde — so `accepts` rejecting it
/// is belt-and-braces, not the reported hole.) Reading an absent range
/// as "any f64 is valid here" is the same category error this whole
/// ledger keeps finding, so it is written down at the type.
///
/// A declared range is a **refusal at the setter**, not a clamp: the
/// value the caller asked for is rejected with a message naming the
/// domain, rather than silently becoming a different number. Project
/// TOML is deliberately **not** validated against it — a file is loaded
/// by serde with no registry in scope, and the generator-side guard
/// ([`crate::ops::drill::fed_descents`]) is what catches that path.
#[derive(Debug, Clone, Copy)]
pub struct ParamRange {
    /// Lower bound. Inclusive unless [`Self::min_exclusive`].
    pub min: Option<f64>,
    /// Upper bound, inclusive.
    pub max: Option<f64>,
    /// When true `min` is a STRICT bound: `value > min`, not `>=`.
    pub min_exclusive: bool,
}

impl ParamRange {
    /// A strictly-positive-and-above-`min` domain with no ceiling — the
    /// shape of every "a physical length that must be real" dial.
    pub(super) const fn greater_than(min: f64) -> Self {
        Self {
            min: Some(min),
            max: None,
            min_exclusive: true,
        }
    }

    /// `min` INCLUSIVE, no ceiling — the shape of a dial whose floor is a
    /// meaningful setting rather than a degenerate one. A cap that switches
    /// its own feature off at `0.0` needs this and not
    /// [`Self::greater_than`], which would refuse the off position.
    pub(super) const fn at_least(min: f64) -> Self {
        Self {
            min: Some(min),
            max: None,
            min_exclusive: false,
        }
    }

    /// Whether `value` is inside the declared domain. Non-finite is
    /// ALWAYS outside: a range says a quantity is numeric, and `NaN`
    /// compares false against every bound, so an unguarded comparison
    /// would let it through.
    pub fn accepts(&self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        if let Some(min) = self.min {
            let ok = if self.min_exclusive {
                value > min
            } else {
                value >= min
            };
            if !ok {
                return false;
            }
        }
        if let Some(max) = self.max
            && value > max
        {
            return false;
        }
        true
    }

    /// Human/agent-readable domain, used verbatim in the setter's
    /// refusal message and in the published schema.
    pub fn describe(&self) -> String {
        let lo = match (self.min, self.min_exclusive) {
            (Some(m), true) => format!("> {m}"),
            (Some(m), false) => format!(">= {m}"),
            (None, _) => String::new(),
        };
        let hi = match self.max {
            Some(m) => format!("<= {m}"),
            None => String::new(),
        };
        match (lo.is_empty(), hi.is_empty()) {
            (false, false) => format!("finite, {lo} and {hi}"),
            (false, true) => format!("finite and {lo}"),
            (true, false) => format!("finite and {hi}"),
            (true, true) => "finite".to_owned(),
        }
    }

    /// Schema projection for `get_operation_schema` consumers.
    pub(crate) fn to_json(self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        if let Some(min) = self.min {
            obj.insert("min".to_owned(), serde_json::json!(min));
            obj.insert(
                "min_exclusive".to_owned(),
                serde_json::json!(self.min_exclusive),
            );
        }
        if let Some(max) = self.max {
            obj.insert("max".to_owned(), serde_json::json!(max));
        }
        obj.insert("finite".to_owned(), serde_json::json!(true));
        serde_json::Value::Object(obj)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub name: &'static str,
    pub type_name: &'static str,
    pub optional: bool,
    /// One line saying what this dial does, for BOTH readers.
    ///
    /// UI-04: this is the single help text. The GUI tooltip used to be a
    /// 60-arm match on the visible LABEL string in
    /// `viz/ui/properties/linking_dressup.rs`, so `"Stepover:"` reached its
    /// help only because two spellings happened to agree by hand, and a
    /// label rename dropped the tooltip in silence. The GUI now keys on
    /// `(OperationType, param name)` and reads this field, which is the same
    /// text `operation_schema` serves an agent as `description`.
    pub help: Option<&'static str>,
    /// Declared numeric domain, or `None` for "no domain stated" — see
    /// [`ParamRange`], which spells out why those are different claims.
    pub range: Option<ParamRange>,
    /// Other names `set_toolpath_param` accepts for this same field.
    ///
    /// CMP-08: three alias setters existed in
    /// `session/compute/params.rs` and the registry published none of
    /// them — Waterline maps `depth_per_pass` onto `z_step`, RampFinish
    /// onto `max_stepdown`, and Pencil maps `stepover` onto
    /// `offset_stepover`. So `get_operation_schema` was wrong in both
    /// directions for those three ops: it omitted a name the setter
    /// takes, and the refusal message for a genuinely unknown name
    /// listed a valid set that did not include it.
    ///
    /// An alias is a second NAME, never a second field. It is published
    /// beside its def and it never becomes a key of its own in
    /// `params_value_including_nulls` or `param_schema_hints`.
    pub aliases: &'static [&'static str],
}

impl ParamDef {
    pub(super) const fn required(name: &'static str, type_name: &'static str) -> Self {
        Self {
            name,
            type_name,
            optional: false,
            help: None,
            range: None,
            aliases: &[],
        }
    }

    /// A required parameter whose numeric domain the registry states.
    pub(super) const fn required_ranged(
        name: &'static str,
        type_name: &'static str,
        range: ParamRange,
        help: &'static str,
    ) -> Self {
        Self {
            name,
            type_name,
            optional: false,
            help: Some(help),
            range: Some(range),
            aliases: &[],
        }
    }

    pub(super) const fn optional(name: &'static str, type_name: &'static str) -> Self {
        Self {
            name,
            type_name,
            optional: true,
            help: None,
            range: None,
            aliases: &[],
        }
    }

    pub(super) const fn optional_desc(
        name: &'static str,
        type_name: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            name,
            type_name,
            optional: true,
            help: Some(help),
            range: None,
            aliases: &[],
        }
    }

    /// A required parameter that carries an agent-facing description.
    ///
    /// The sibling of [`Self::optional_desc`] for the required case, added
    /// so a dial whose SEMANTICS an agent cannot guess from its name can say
    /// what it does. First used by F3 / D-16.2.
    pub(super) const fn required_desc(
        name: &'static str,
        type_name: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            name,
            type_name,
            optional: false,
            help: Some(help),
            range: None,
            aliases: &[],
        }
    }

    /// The same def, plus the other names `set_toolpath_param` accepts
    /// for this field. See [`Self::aliases`] for why a registry row and
    /// not a hidden match arm.
    pub(super) const fn with_aliases(self, aliases: &'static [&'static str]) -> Self {
        Self { aliases, ..self }
    }

    /// The same def, plus the one line that says what the dial does.
    ///
    /// UI-04 wrote the GUI's 60 tooltip texts onto the rows they belong to
    /// through this builder, so the constructor list above did not have to
    /// grow a fourth variant for every combination.
    pub(super) const fn with_help(self, help: &'static str) -> Self {
        Self {
            help: Some(help),
            ..self
        }
    }
}

// ── Phase 1 operation registry (architectural refactor 2026-06-06) ────
//
// Data-only per-operation metadata table. Each entry pairs the
// operation's `OperationSpec`, its settable-param schema, and its tool
// constraints in one place. `OperationType::registry_entry` is the
// exhaustive accessor; the public helpers below delegate to it. There
// are deliberately NO wildcard fallbacks here — every field of every
// entry is an explicit decision ("miss nothing, or don't compile").

/// Static-friendly tool-constraint data for a registry entry.
/// Materialized into the serde-facing [`ToolConstraints`] by
/// [`Self::to_schema`].
///
/// Typed on [`CutterKind`] (Phase 3) — the published schema still
/// speaks snake_case tool-type strings (derived via
/// `CutterKind::tool_type().serde_token()`, byte-identical to the
/// pre-Phase-3 literals), but the registry decision itself is a
/// compiler-checked shape-class list: a 6th cutter shape can't be
/// silently absent from a constraint list the way a typo'd string
/// could.
#[derive(Debug, Clone, Copy)]
pub struct ToolConstraintsDef {
    /// Cutter shape classes the operation requires; empty means any
    /// tool geometry is accepted.
    pub required_kinds: &'static [CutterKind],
    /// Whether a V-bit can run this operation at all.
    pub supports_v_bit: bool,
}

impl ToolConstraintsDef {
    /// Named "no restriction" policy: any tool geometry, V-bit included.
    /// Referenced explicitly by every unrestricted entry so the
    /// unrestricted set is a recorded decision, not a wildcard fallback.
    pub const ANY_TOOL: Self = Self {
        required_kinds: &[],
        supports_v_bit: true,
    };

    /// Whether a cutter of this shape class may run the operation.
    /// THE reconciliation point with runtime refusals (e.g. the
    /// Scallop ball-tip check in `execute_operation_annotated`): the
    /// refusal reads this predicate, so the registry list and the
    /// runtime gate cannot drift.
    pub fn allows(&self, kind: CutterKind) -> bool {
        if !self.supports_v_bit && kind == CutterKind::VBit {
            return false;
        }
        self.required_kinds.is_empty() || self.required_kinds.contains(&kind)
    }

    /// Materialize the serde-facing [`ToolConstraints`].
    pub fn to_schema(&self) -> ToolConstraints {
        ToolConstraints {
            required_tool_type: self
                .required_kinds
                .iter()
                .map(|k| k.tool_type().serde_token().to_owned())
                .collect(),
            supports_v_bit: self.supports_v_bit,
        }
    }
}

/// Cutting kinematics class of an operation.
///
/// CMP-07: this was `matches!(self, Drill | AlignmentPinDrill)` in
/// `catalog.rs` — a membership list that fails OPEN. A 25th operation
/// joined the false side of it without a compiler word. It is a registry
/// row field now, so every operation states its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kinematics {
    /// The cutter moves in XY and Z. Every milling operation.
    Milling,
    /// Z-only peck-plunge drilling. The verdict layer suppresses the
    /// rapid:cut-ratio and engagement signals for these, and the chipload,
    /// power and deflection gates, the optimizer skip and the narrate
    /// `is_drill_cycle` flag all read this one class.
    DrillZOnly,
}

/// The three per-operation policies that used to be `matches!` membership
/// lists outside the registry (CMP-07).
///
/// The registry's own header states the goal — data-only per-operation
/// metadata, with no wildcard fallbacks. These three failed open: a new
/// operation joined the false side of each in silence. As a row field the
/// decision is written per operation, beside `tool_constraints` and
/// `dressup_policy`, and the named constants below are what an entry
/// points at when it takes the ordinary answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpPolicy {
    /// The op's `stepover()` is a LATERAL raster spacing, so the cusp
    /// between neighbouring passes is the scallop it forms.
    ///
    /// False for an op whose stepover is not a raster pitch (Pencil's
    /// offset fan, RadialFinish's varying spoke spacing), for an op that
    /// publishes no stepover (RampFinish), and for the two that declare a
    /// `scallop_height()`, which is consulted first (Scallop,
    /// UnifiedFinish).
    pub raster_stepover_is_lateral: bool,
    /// Milling or Z-only drilling.
    pub kinematics: Kinematics,
    /// An empty generation from this op is a legitimately absent FEATURE,
    /// not a failure to plan, so the empty-generation gate exempts it.
    pub empty_is_feature_selective: bool,
}

impl OpPolicy {
    /// The ordinary answer: a milling op whose stepover is not a raster
    /// pitch and whose empty result is a planning failure.
    pub const MILLING: Self = Self {
        raster_stepover_is_lateral: false,
        kinematics: Kinematics::Milling,
        empty_is_feature_selective: false,
    };

    /// A milling op whose `stepover()` IS the lateral raster pitch.
    pub const LATERAL_RASTER: Self = Self {
        raster_stepover_is_lateral: true,
        kinematics: Kinematics::Milling,
        empty_is_feature_selective: false,
    };

    /// Z-only peck-plunge drilling.
    pub const DRILLING: Self = Self {
        raster_stepover_is_lateral: false,
        kinematics: Kinematics::DrillZOnly,
        empty_is_feature_selective: false,
    };

    /// A milling op that cuts one FEATURE class, so an empty result means
    /// the model has none of that feature.
    pub const FEATURE_SELECTIVE: Self = Self {
        raster_stepover_is_lateral: false,
        kinematics: Kinematics::Milling,
        empty_is_feature_selective: true,
    };
}

/// Entry-style coercion applied by `DressupConfig::normalize_for_op`
/// (and mirrored by the viz dressup panel) for one operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStylePolicy {
    /// Any configured entry style is accepted as-is.
    AnyEntry,
    /// Entry styles are meaningless or harmful for this op (stock-based /
    /// single-pass / planner-emitted entries) — always coerce to `None`.
    ForceNone,
    /// `Ramp` is upgraded to `Helix` (the op's pocketing geometry has
    /// natural circular boundaries); other styles pass through.
    PreferHelix,
}

/// Per-op dressup policy (Phase 1 registry field). ONE source for both
/// `DressupConfig::normalize_for_op` in compute and the viz dressup
/// panel's grey-out/tooltip — pre-registry these were two hand-synced
/// tables (compute/config.rs predicates + ui/properties/mod.rs match).
#[derive(Debug, Clone, Copy)]
pub struct DressupPolicy {
    /// `Some(reason)` ⇒ entry ramps, lead-in/out, AND link moves are
    /// geometrically incompatible with this op: compute strips them all
    /// and the UI greys the controls, showing this user-facing reason.
    pub strip_all_reason: Option<&'static str>,
    /// Entry-style coercion applied after (and independent of) stripping.
    pub entry: EntryStylePolicy,
}

impl DressupPolicy {
    /// Named "no restriction" policy: all dressups available as configured.
    pub const ANY_DRESSUP: Self = Self {
        strip_all_reason: None,
        entry: EntryStylePolicy::AnyEntry,
    };
    /// Entry style forced to `None`; lead-in/out and link moves untouched.
    pub const FORCE_NO_ENTRY: Self = Self {
        strip_all_reason: None,
        entry: EntryStylePolicy::ForceNone,
    };
    /// `Ramp` upgraded to `Helix`; everything else untouched.
    pub(crate) const PREFER_HELIX: Self = Self {
        strip_all_reason: None,
        entry: EntryStylePolicy::PreferHelix,
    };
    /// All topology-altering dressups stripped, with the user-facing reason.
    pub const fn strip_all(reason: &'static str) -> Self {
        Self {
            strip_all_reason: Some(reason),
            entry: EntryStylePolicy::AnyEntry,
        }
    }
}

/// One row of the Phase 1 operation registry. Data only — no behavior
/// function pointers until the Phase 5 adapters.
#[derive(Debug, Clone, Copy)]
pub struct OpRegistryEntry {
    pub op_type: OperationType,
    pub spec: OperationSpec,
    pub param_defs: &'static [ParamDef],
    pub tool_constraints: ToolConstraintsDef,
    pub dressup_policy: DressupPolicy,
    /// The three per-op policies CMP-07 moved off `matches!` lists.
    pub policy: OpPolicy,
    /// The family adapter `execute_operation_annotated` dispatches to.
    ///
    /// CMP-01: this was `Option<GenerateFn>` and `execute.rs` carried a
    /// 24-arm `else` match for the `None` case. The T11 cutover of
    /// 2026-06-07 filled every row, so that match never ran, and the
    /// sentry that asserted `generate.is_some()` for every op made the
    /// branch unreachable by test as well as in fact. The compile-time
    /// net survives the delete: `OperationType::registry_entry` is an
    /// exhaustive match, so a new variant fails to compile until it
    /// names a row, and a row cannot be written without an adapter.
    pub generate: crate::compute::execute::GenerateFn,
}
