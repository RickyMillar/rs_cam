//! W1 (toolpath inspector programme, 2026-10-30): the owner and the short
//! face of each diagnostic rule.
//!
//! A [`Diagnostic`](super::Diagnostic) carries two presentation fields:
//!
//! - `face` — the operator text for the panel face: at most
//!   [`FACE_MAX_CHARS`] characters, plain words, no category prefix and no
//!   derivation. The full text stays in `message`.
//! - `anchor` — the place that holds the fix ([`Anchor`]).
//!
//! [`TABLE`] is THE map from a rule id to its anchor and its default face.
//! Every id of [`super::ids::ALL`] has one row; the sentry
//! `every_id_has_an_anchor_and_a_short_face_w1` fails on an id with no row.
//! An adapter fills both fields from [`face`] and [`anchor`]. A site whose
//! face depends on its inputs (the field that is not set, the tool kind the
//! op needs, the derate factor) writes its own face and keeps it within
//! [`FACE_MAX_CHARS`].
//!
//! The anchor rule:
//!
//! - [`Anchor::Field`] — one op parameter (a registry `ParamDef` name), a
//!   height (`top_z`, `bottom_z`, ...) or a dressup field holds the fix.
//! - [`Anchor::Tool`] — the tool binding of the op holds the fix.
//! - [`Anchor::Operation`] — the finding is about the op as a whole.
//! - [`Anchor::Note`] — advisory: every site of the rule gives `Info` or
//!   `Hint`. It informs and asks for no fix.
//! - [`Anchor::Project`] — a setup, stock, pin or holder finding.
//! - [`Anchor::View`] — a fact for the viewport only.

use serde::{Deserialize, Serialize};

use super::ids;

/// The longest face, in characters (`char`s, not bytes).
pub const FACE_MAX_CHARS: usize = 60;

/// The face of a rule id with no row in [`TABLE`] (a test id).
pub const UNKNOWN_FACE: &str = "See the details";

/// The name of the field that holds the fix of a finding.
///
/// The name is the key the registry, the help text and MCP
/// `set_toolpath_param` use: an op `ParamDef` name, a [`HEIGHT_KEYS`] entry
/// or a dressup field name. [`FieldKey::parse`] accepts only those names, so
/// a deserialised key is always a real field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldKey(&'static str);

/// The five height fields of `HeightsConfig`.
pub const HEIGHT_KEYS: &[&str] = &["clearance_z", "retract_z", "feed_z", "top_z", "bottom_z"];

/// The selection of the op (`ToolpathConfig::selection_of`). The registry
/// publishes no `ParamDef` for it: `EditToolpathSelection` writes it. A
/// selection refusal anchors here.
pub const SELECTION_KEY: &str = "selection";

impl FieldKey {
    /// The key for a name the tables write. A `const` table cannot call
    /// [`Self::parse`]; the sentry checks each table key with it.
    #[must_use]
    pub const fn new_unchecked(name: &'static str) -> Self {
        Self(name)
    }

    /// The key, or `None` when no op parameter, height or dressup field has
    /// this name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        known_field_names().find(|known| *known == name).map(Self)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// Every field name a [`FieldKey`] may hold: each op parameter of the
/// registry, each height, each dressup field and the selection.
pub fn known_field_names() -> impl Iterator<Item = &'static str> {
    use crate::compute::catalog::OperationType;
    let params = OperationType::ALL
        .iter()
        .flat_map(|op| op.registry_entry().params.defs.iter().map(|d| d.name));
    params.chain(field_names_of_every_op())
}

/// The field names every op has: the heights, the dressup fields and the
/// selection. An op parameter name is not in this set.
fn field_names_of_every_op() -> impl Iterator<Item = &'static str> {
    let dressups = crate::compute::config::DressupConfig::FIELD_DEFS
        .iter()
        .map(|d| d.name);
    HEIGHT_KEYS
        .iter()
        .copied()
        .chain(dressups)
        .chain(std::iter::once(SELECTION_KEY))
}

/// Every field name a finding can anchor to: each field anchor of
/// [`TABLE`], and each value that S11 can name as not set
/// (`compute::not_set::NOT_SET_FIELDS`). A name that [`FieldKey::parse`]
/// refuses is not in the set. A panel that draws field rows covers these
/// names (the GUI `placement::drawn_fields`).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "projection: only the named variant(s) carry this (named: Anchor::Field)"
)]
pub fn anchor_field_names() -> std::collections::BTreeSet<&'static str> {
    let table = TABLE.iter().filter_map(|(_, anchor, _)| match anchor {
        Anchor::Field { key } => Some(key.as_str()),
        _ => None,
    });
    table
        .chain(crate::compute::not_set::NOT_SET_FIELDS.iter().copied())
        .filter(|name| FieldKey::parse(name).is_some())
        .collect()
}

/// The anchor of a finding on an op of kind `op`. A field anchor that names
/// an op parameter the op does not publish becomes [`Anchor::Operation`],
/// so a panel never points at a field the op does not draw. Example: a
/// Drill carries a plunge rate but publishes no `plunge_rate` row.
#[must_use]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "pass-through: every other variant keeps its own value"
)]
pub fn fit_anchor(anchor: Anchor, op: crate::compute::catalog::OperationType) -> Anchor {
    match anchor {
        Anchor::Field { key } if !op_has_field(op, key.as_str()) => Anchor::Operation,
        other => other,
    }
}

/// True when an op of kind `op` has a field named `name`: a parameter it
/// publishes, or a field every op has.
#[must_use]
pub fn op_has_field(op: crate::compute::catalog::OperationType, name: &str) -> bool {
    op.registry_entry()
        .params
        .defs
        .iter()
        .any(|d| d.name == name)
        || field_names_of_every_op().any(|known| known == name)
}

impl Serialize for FieldKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for FieldKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::parse(&name).ok_or_else(|| {
            serde::de::Error::custom(format!("`{name}` names no op parameter, height or dressup"))
        })
    }
}

/// Where a diagnostic lives: the place that holds its fix. The module doc
/// gives the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Anchor {
    /// One field holds the fix.
    Field { key: FieldKey },
    /// The tool binding of the op: no tool, or the wrong kind of tool.
    Tool,
    /// The op as a whole, with no one field.
    Operation,
    /// Advisory: every site of the rule gives `Info` or `Hint`.
    Note,
    /// A project or setup finding: pins, holder, obstacles, stock.
    Project,
    /// A fact for the viewport only.
    View,
}

impl Anchor {
    /// The field anchor of `name`. The tables call it with a literal; the
    /// sentry checks each literal with [`FieldKey::parse`].
    #[must_use]
    pub const fn field(name: &'static str) -> Self {
        Self::Field {
            key: FieldKey::new_unchecked(name),
        }
    }
}

const fn f(name: &'static str) -> Anchor {
    Anchor::field(name)
}
const TOOL: Anchor = Anchor::Tool;
const OP: Anchor = Anchor::Operation;
const NOTE: Anchor = Anchor::Note;
const PROJECT: Anchor = Anchor::Project;

/// THE map from a rule id to its anchor and its default face. One row per
/// id of [`ids::ALL`].
pub const TABLE: &[(&str, Anchor, &str)] = &[
    // The load gates.
    (ids::LOAD_CHIPLOAD_HIGH, f("feed_rate"), "Chipload too high"),
    (ids::LOAD_CHIPLOAD_LOW, f("feed_rate"), "Chipload too low"),
    (ids::LOAD_CHIPLOAD_WITHIN, NOTE, "Chipload within band"),
    (ids::LOAD_CHIPLOAD_UNMODELED, NOTE, "Chipload not measured"),
    (
        ids::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND,
        NOTE,
        "Feed per tooth above vendor band",
    ),
    (
        ids::LOAD_POWER_EXCEEDS,
        f("feed_rate"),
        "Spindle power exceeded",
    ),
    (ids::LOAD_POWER_WITHIN, NOTE, "Power within budget"),
    (ids::LOAD_POWER_UNMODELED, NOTE, "Power not measured"),
    (
        ids::LOAD_DEFLECTION_EXCEEDS,
        f("feed_rate"),
        "Tool deflection too high",
    ),
    (ids::LOAD_DEFLECTION_WITHIN, NOTE, "Deflection within limit"),
    (
        ids::LOAD_DEFLECTION_UNMODELED,
        NOTE,
        "Deflection not measured",
    ),
    (ids::LOAD_DEPTH_WITHIN, NOTE, "Depth of cut within cap"),
    (
        ids::LOAD_DEPTH_EXCEEDS,
        OP,
        "Depth of cut too deep for this tool",
    ),
    (
        ids::LOAD_DEPTH_REPORTED,
        NOTE,
        "Finish pass: depth of cut has no cap",
    ),
    (ids::LOAD_DEPTH_UNMODELED, NOTE, "Depth of cut not measured"),
    (
        ids::LOAD_MATERIAL_NOT_JUDGED,
        NOTE,
        "Cuts in added material not checked",
    ),
    (
        ids::DRILL_CHIP_WELDING,
        f("spindle_rpm"),
        "Chip welding risk",
    ),
    (
        ids::DRILL_PECK_ADEQUACY,
        f("peck_depth"),
        "Peck depth too deep",
    ),
    (
        ids::DRILL_PLUNGE_FEED,
        f("feed_rate"),
        "Drill feed outside safe band",
    ),
    // Project findings.
    (
        ids::PROJECT_HOLDER_COLLISION,
        PROJECT,
        "Holder hits the stock",
    ),
    (
        ids::PROJECT_HOLDER_CHECK_FAILED,
        PROJECT,
        "Holder check failed",
    ),
    (
        ids::PROJECT_RAPID_COLLISION,
        f("retract_z"),
        "Rapid move hits stock",
    ),
    (
        ids::PROJECT_OBSTACLE_COLLISION,
        PROJECT,
        "Path hits an obstacle",
    ),
    (
        ids::PROJECT_PLUNGE_STRESS,
        f("entry_style"),
        "Hard plunge: use a ramp or helix entry",
    ),
    (ids::PROJECT_AIR_CUT_HIGH, NOTE, "Much of the path cuts air"),
    (ids::PROJECT_GENERATED_EMPTY, OP, "Removes no material"),
    (
        ids::PROJECT_ALIGNMENT_PINS_UNKEYED,
        PROJECT,
        "Alignment pins not keyed",
    ),
    (
        ids::PROJECT_PIN_HOLES_DRILLED_TWICE,
        PROJECT,
        "Pin holes drilled twice",
    ),
    (
        ids::PROJECT_DOWEL_HOLES_DISAGREE,
        PROJECT,
        "Dowel holes do not match",
    ),
    (
        ids::PROJECT_PIN_WALL_THIN,
        PROJECT,
        "Thin wall at a pin hole",
    ),
    (
        ids::PROJECT_PIN_TOOL_MISMATCH,
        PROJECT,
        "Pin tool does not fit",
    ),
    (ids::PROJECT_SHOE_GAP_TOO_LOW, OP, "Dust shoe gap too low"),
    (
        ids::PROJECT_MEASURABILITY_ABSTAINED,
        NOTE,
        "Air cut not measured",
    ),
    (
        ids::PROJECT_CROSSES_STANDING_MATERIAL,
        OP,
        "Link crosses uncut stock",
    ),
    (
        ids::PROJECT_ENTRY_LOAD,
        f("entry_style"),
        "Entry cuts too hard",
    ),
    (ids::PROJECT_LINK_CUTS_STOCK, OP, "Link move cuts stock"),
    (
        ids::PROJECT_ROUGH_LEAVES_ISLANDS,
        OP,
        "Roughing leaves islands of stock",
    ),
    (
        ids::PROJECT_PLUNGE_CLASS_LOAD,
        f("plunge_rate"),
        "Plunges faster than the plunge rate",
    ),
    (
        ids::PROJECT_REST_SLOWER_THAN_FRESH,
        NOTE,
        "Rest pass is slower than a fresh pass",
    ),
    (
        ids::PROJECT_KINEMATIC_UTILIZATION,
        NOTE,
        "Machine speed not used",
    ),
    // The feeds calculator.
    (
        ids::FEEDS_FEED_CLAMPED,
        NOTE,
        "Feed clamped to machine limit",
    ),
    (
        ids::FEEDS_POWER_LIMITED,
        f("feed_rate"),
        "Spindle power limits the cut",
    ),
    (
        ids::FEEDS_POWER_LADDER_REDUCED_CUT,
        f("feed_rate"),
        "Cut reduced to fit spindle power",
    ),
    (
        ids::FEEDS_SHANK_TOO_LARGE,
        TOOL,
        "Tool shank too large for the machine",
    ),
    (
        ids::FEEDS_DOC_EXCEEDS_FLUTE,
        f("depth_per_pass"),
        "Depth per pass capped to flute length",
    ),
    (
        ids::FEEDS_SLOTTING_DETECTED,
        f("depth_per_pass"),
        "Slotting: depth per pass reduced",
    ),
    (
        ids::FEEDS_SCALLOP_INVALID,
        f("scallop_height"),
        "Scallop target not reachable",
    ),
    (
        ids::FEEDS_CHIPLOAD_BELOW_FLOOR,
        f("feed_rate"),
        "Feed too low: the tool can rub",
    ),
    (
        ids::FEEDS_LONG_TOOL_DERATE,
        NOTE,
        "Long tool: load target lowered",
    ),
    (
        ids::FEEDS_AGGRESSIVENESS_ENGAGEMENT,
        OP,
        "Cut made smaller for the aggressiveness",
    ),
    (
        ids::FEEDS_RPM_LOWERED_FOR_CEILING,
        NOTE,
        "RPM lowered to fit the feed limit",
    ),
    (
        ids::FEEDS_DRILL_FEED_CLAMPED_TO_ENVELOPE,
        f("feed_rate"),
        "Drill feed clamped to safe band",
    ),
    (
        ids::FEEDS_VENDOR_ROW_PUBLISHES_NO_CHIPLOAD,
        NOTE,
        "Vendor data gives no chipload",
    ),
    (
        ids::FEEDS_NO_VENDOR_ROWS_FOR_ROUTED_OPERATION,
        OP,
        "No vendor data: feeds from formula",
    ),
    (
        ids::FEEDS_DEPTH_BEYOND_PUBLISHED_TABLE,
        f("depth_per_pass"),
        "Depth per pass beyond vendor table",
    ),
    (
        ids::FEEDS_FEED_VS_LUT_HIGH,
        NOTE,
        "Feed far above the recommended feed",
    ),
    (
        ids::FEEDS_FEED_VS_LUT_LOW,
        NOTE,
        "Feed far below the recommended feed",
    ),
    (
        ids::FEEDS_STEPOVER_VS_LUT,
        NOTE,
        "Stepover far above the recommended value",
    ),
    (
        ids::FEEDS_DPP_VS_LUT,
        NOTE,
        "Depth per pass far above the recommended value",
    ),
    // Geometry of the op and its heights.
    (
        ids::GEOM_STEPOVER_EXCEEDS_DIAMETER,
        f("stepover"),
        "Stepover wider than the tool",
    ),
    (
        ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH,
        f("depth_per_pass"),
        "Depth per pass past flute length",
    ),
    (
        ids::GEOM_BOTTOM_ABOVE_TOP_Z,
        f("bottom_z"),
        "Bottom height above top height",
    ),
    (
        ids::GEOM_FEED_Z_BELOW_TOP_Z,
        f("feed_z"),
        "Feed height below top height",
    ),
    (
        ids::GEOM_RETRACT_Z_BELOW_FEED_Z,
        f("retract_z"),
        "Retract height below feed height",
    ),
    (
        ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
        f("clearance_z"),
        "Clearance height below retract height",
    ),
    (
        ids::GEOM_RETRACT_Z_BELOW_TOP_Z,
        f("retract_z"),
        "Retract height below top height",
    ),
    (
        ids::GEOM_PLUNGE_EXCEEDS_FEED,
        f("plunge_rate"),
        "Plunge rate above feed rate",
    ),
    (ids::GEOM_DEPTH_BEYOND_STOCK, OP, "Cuts below the stock"),
    (
        ids::GEOM_STANDING_MATERIAL,
        OP,
        "Material left uncut inside",
    ),
    (
        ids::GEOM_UNMACHINED_BAND,
        OP,
        "Heights leave a feature uncut",
    ),
    (
        ids::GEOM_CLIPPED_BAND,
        NOTE,
        "Heights leave part of a feature unfinished",
    ),
    (
        ids::GEOM_TIP_FLOAT,
        OP,
        "Tool too large to reach some valley floors",
    ),
    (
        ids::GEOM_RAMP_REACH_CLAMP,
        f("entry_style"),
        "Ramp raised: tool cannot reach",
    ),
    (ids::GEOM_ZERO_REMOVAL, OP, "Cuts no material"),
    (
        ids::GEOM_BURIED_CURVE,
        NOTE,
        "Curve cuts deeper than its set depth",
    ),
    (
        ids::GEOM_SELECTION_NOTE,
        OP,
        "Some selected items are not cut",
    ),
    (ids::GEOM_TAB_PLACEMENT, NOTE, "Tabs not placed as asked"),
    (
        ids::GEOM_OFFSET_LIBRARY_FAILURE,
        OP,
        "Offset failed: material can be left",
    ),
    (
        ids::GEOM_BOUNDARY_CLIP_DROPPED,
        OP,
        "Boundary not applied: the tool is too wide",
    ),
    (
        ids::GEOM_WATERLINE_LEVELS_BELOW_STOCK,
        OP,
        "Z levels below the stock dropped",
    ),
    (
        ids::GEOM_WATERLINE_LADDER_EMPTY,
        OP,
        "Too few Z levels to cut",
    ),
    (
        ids::GEOM_WATERLINE_PART_UNHELD,
        OP,
        "Part not held at the end: add tabs",
    ),
    (
        ids::GEOM_REGION_CAP_TRUNCATED,
        OP,
        "Some rest areas dropped: too many found",
    ),
    (
        ids::GEOM_REST_LEVELS_FALLBACK,
        OP,
        "Rest cut the whole outline",
    ),
    (
        ids::CONFIG_DERIVED_STEPOVER,
        NOTE,
        "Stepover sized for this tool",
    ),
    // Caution when the reference needs attention, so not a note.
    (
        ids::CONFIG_CLAIMS_REFERENCE,
        OP,
        "Check the rest stock source",
    ),
    (ids::CONFIG_INERT_CLAIMS_DIAL, OP, "Rest dial has no effect"),
    // Tool and op compatibility.
    (ids::COMPAT_TOOL_KIND_REFUSED, TOOL, "Wrong tool kind"),
    (
        ids::COMPAT_BALL_NOSE_FLAT_CLEARING,
        NOTE,
        "Ball nose leaves ridges on flat floors",
    ),
    (
        ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION,
        f("projection"),
        "Point projection can cut the far surface",
    ),
    (
        ids::TOOL_NAME_SIZE_MISMATCH,
        NOTE,
        "Tool name and size differ",
    ),
    (
        ids::TOOL_STICKOUT_SHORTER_THAN_FLUTE,
        TOOL,
        "Stick-out shorter than the flute",
    ),
    (
        ids::QUALITY_STEPOVER_OVER_80_PCT,
        NOTE,
        "Wide stepover can leave ridges",
    ),
    (
        ids::QUALITY_FINISH_STEPOVER_OVER_50_PCT,
        NOTE,
        "Wide finish stepover: rough surface",
    ),
    (
        ids::QUALITY_BALL_SCALLOP_HEIGHT,
        NOTE,
        "Ball nose leaves ridges at this stepover",
    ),
    (
        ids::EFFICIENCY_VERY_FINE_STEPOVER,
        NOTE,
        "Very fine stepover: long cycle time",
    ),
    // Stale defaults.
    (
        ids::STALE_DROP_CUTTER_MIN_Z,
        OP,
        "Min Z is at an old default value",
    ),
    (
        ids::STALE_TAPERED_BALL_PLUNGE,
        OP,
        "Plunge too fast for a small ball nose",
    ),
    (
        ids::STALE_WOOD_ADAPTIVE_STEPOVER,
        f("stepover"),
        "Adaptive stepover narrower than the wood target",
    ),
    (
        ids::STALE_PROJECT_CURVE_NEGATIVE_DEPTH,
        f("depth"),
        "Negative depth: the toolpath cuts air",
    ),
    // Preconditions.
    (
        ids::PRECOND_DRILL_NO_HOLES,
        f("selection"),
        "No holes to drill",
    ),
    (
        ids::PRECOND_ALIGNMENT_PIN_DRILL_NO_HOLES,
        PROJECT,
        "No pin holes to drill",
    ),
    (
        ids::PRECOND_SELECTION_REFUSED,
        f("selection"),
        "Fix the selection: generation refuses it",
    ),
    (
        ids::PRECOND_SELECTION_EMPTY,
        f("selection"),
        "Select the geometry to cut",
    ),
    (
        ids::PRECOND_BURIED_FACE,
        f("selection"),
        "Face below the stock top",
    ),
    (ids::PRECOND_NOT_SET, OP, "Value not set"),
    (ids::PRECOND_NO_TOOL, TOOL, "No tool chosen"),
    (ids::SAFETY_CUTS_INTO_MODEL, OP, "Cuts into the model"),
    (
        ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE,
        TOOL,
        "Plunges a tool that does not cut at its centre",
    ),
    (
        ids::SAFETY_NON_CENTRE_CUTTING_DESCENT,
        TOOL,
        "Tool goes down too steeply for its centre",
    ),
    (ids::PRECOND_PROJECT_CURVE_NO_CURVE, OP, "No source curve"),
    (
        ids::PRECOND_PROJECT_CURVE_NO_SURFACE,
        PROJECT,
        "No surface mesh",
    ),
    (
        ids::REF_MODEL_MISSING,
        OP,
        "Model missing: pick a loaded model",
    ),
    (
        ids::EXPORT_OP_LEFT_OUT,
        OP,
        "Left out of the export: no current result",
    ),
];

fn row(id: &str) -> Option<&'static (&'static str, Anchor, &'static str)> {
    TABLE.iter().find(|(row_id, _, _)| *row_id == id)
}

/// The anchor of a rule id. An id with no row (a test id) is a
/// [`Anchor::Note`].
#[must_use]
pub fn anchor(id: &str) -> Anchor {
    row(id).map_or(Anchor::Note, |(_, anchor, _)| *anchor)
}

/// The default face of a rule id, as an owned `String` for the
/// `Diagnostic` field. An id with no row gives [`UNKNOWN_FACE`].
#[must_use]
pub fn face(id: &str) -> String {
    row(id)
        .map_or(UNKNOWN_FACE, |(_, _, face)| *face)
        .to_owned()
}

/// `text` when it fits in [`FACE_MAX_CHARS`], else the default face of
/// `id`. A site that builds a face from its inputs calls it, so a long
/// input never reaches the panel face.
#[must_use]
pub fn face_or_default(id: &str, text: String) -> String {
    if text.chars().count() <= FACE_MAX_CHARS {
        text
    } else {
        face(id)
    }
}

/// The text with its first letter in upper case ("needs a ball nose" gives
/// "Needs a ball nose").
#[must_use]
pub fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}
