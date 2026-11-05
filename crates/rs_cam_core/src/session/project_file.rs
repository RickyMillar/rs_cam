//! TOML project file types and loading/saving helpers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{
    DatumConfig, LoadedGeometry, LoadedModel, SessionError, SetupData, ToolpathConfig, XYDatum,
    ZDatum,
};
use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use crate::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolRecord, ToolType};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::gcode::CoolantMode;
use crate::ids::{ModelId, SetupId};
use crate::trace::debug_trace::ToolpathDebugOptions;

// ── Project file types (TOML deserialization) ──────────────────────────

/// Top-level project file structure (format_version=3).
///
/// F034 (operator ruling 5, 2026-10-19): the root and every section below
/// it that this file defines refuse a key they do not hold
/// (`deny_unknown_fields`). A misspelt key is a load error, not a silent
/// default. The toolpath section and the operation tables stay open: the
/// geometry-selection programme still adds and retires keys there. A
/// retired key keeps its own message: [`check_retired_keys`] runs on the
/// raw table BEFORE the typed parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub job: ProjectJobSection,
    #[serde(default)]
    pub tools: Vec<ProjectToolSection>,
    #[serde(default)]
    pub models: Vec<ProjectModelSection>,
    #[serde(default)]
    pub setups: Vec<ProjectSetupSection>,
}

/// The one project format this build reads and writes.
pub const SUPPORTED_FORMAT_VERSION: u32 = 3;

/// A file with no `format_version` key predates the key itself, so it
/// reads as version 1 and [`check_format_version`] refuses it.
fn default_format_version() -> u32 {
    1
}

/// Refuse a project file whose declared format this build does not read.
///
/// rs_cam writes `format_version = 3`. It stores the alignment pins on the
/// stock. Nothing converts an older shape. A converter is a second reader,
/// and a second reader is what I01 found drifting away from this one. An
/// older file therefore fails to open. The message names the version.
fn check_format_version(project: &ProjectFile) -> Result<(), SessionError> {
    if project.format_version == SUPPORTED_FORMAT_VERSION {
        return Ok(());
    }
    Err(SessionError::UnsupportedFormatVersion {
        found: project.format_version,
    })
}

/// One retired project-file key: the table that held it, its name, and
/// what replaced it.
///
/// `table` is a path from the file root. The step `"[]"` enters every
/// entry of an array of tables.
struct RetiredKey {
    table: &'static [&'static str],
    key: &'static str,
    replacement: &'static str,
}

const OBSTACLE_REPLACEMENT: &str = "Fixtures and keep-out zones are one concept now: write each \
     one as an obstacle, [[setups.obstacles]] with an id, a name, a clearance and a geometry \
     (kind = \"box\" with min and max, \"outline_extrude\" or \"model\"), with every value in \
     the setup frame. Delete the old key, an empty list too";

/// Every retired project-file key. The loader refuses a file that holds
/// one of them, and the message names the key and what replaced it.
///
/// Nothing converts an old key, and nothing ignores one. Ruling of
/// 2026-09-16: no legacy, breaking is acceptable. Ruling of 2026-10-15:
/// old code that is left behind is retired. A key that the loader ignores
/// hides an edit the operator thinks is live. Add a row here when a key
/// leaves the file format.
const RETIRED_KEYS: &[RetiredKey] = &[
    RetiredKey {
        table: &["setups", "[]"],
        key: "fixtures",
        replacement: OBSTACLE_REPLACEMENT,
    },
    RetiredKey {
        table: &["setups", "[]"],
        key: "keep_out_zones",
        replacement: OBSTACLE_REPLACEMENT,
    },
    RetiredKey {
        table: &["job", "stock"],
        key: "flip_axis",
        replacement: "Removed (P3): rs_cam derives the flip axis from the setups. Delete the key",
    },
    RetiredKey {
        table: &["job", "stock"],
        key: "workholding_rigidity",
        replacement: "Removed (ruling R4 Q8): machine rigidity is job.machine.rigidity. \
                      Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]", "dressups"],
        key: "air_bridge_policy",
        replacement: "Removed: one rule (retract_saves_time) bridges each air stretch with a \
                      retract only when the retract is faster than the feed. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]", "operation", "params"],
        key: "holes",
        replacement: "Removed (W6.5, C1): the pin drill reads the live stock pins of its \
                      diameter. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]", "operation", "params"],
        key: "selected_holes",
        replacement: "Removed (geometry selection P2.3c): drill picks are in \
                      operation.params.selection. Pick the holes again. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]", "operation", "params"],
        key: "selected_layers",
        replacement: "Removed (geometry selection P2.3c): use a layer rule in \
                      operation.params.selection. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]", "boundary", "source"],
        key: "model_outline",
        replacement: "Removed (geometry selection P2.5): use [boundary.source.selection] with \
                      a selection whose rule reads the model, for example \
                      selection = { rules = [{ source = { model = 3 }, filter = { kinds = \
                      { only = [\"loop\", \"circle\"] } } }] }, and holes in place of \
                      model_outline.holes. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]"],
        key: "face_selection",
        replacement: "Removed (geometry selection P2.6): face picks are in \
                      operation.params.selection. Re-pick the faces. Delete the key",
    },
    RetiredKey {
        table: &["setups", "[]", "toolpaths", "[]"],
        key: "type",
        replacement: "Removed (C2): the op kind is `kind` in the toolpath's [operation] \
                      table, the one source. Delete the key",
    },
    RetiredKey {
        table: &["tools", "[]"],
        key: "shank_length",
        replacement: "Removed (B18): the holder face is at the stickout above the tip, and the \
                      exposed shank is stickout - cutting_length. Set stickout. Delete the key",
    },
    RetiredKey {
        table: &[],
        key: "toolpaths",
        replacement: "Removed: a toolpath lives under its setup, [[setups.toolpaths]]. Delete \
                      the root key, an empty list too",
    },
    RetiredKey {
        table: &["job"],
        key: "machine_ref",
        replacement: "Removed (L6): the machine is a snapshot, the inline [job.machine] \
                      table. Nothing links to a library file. Delete the key",
    },
    RetiredKey {
        table: &["job", "machine"],
        key: "safety_factor",
        replacement: "Removed (ruling R4): machine rigidity is job.machine.rigidity. \
                      Delete the key",
    },
    RetiredKey {
        table: &["job", "machine"],
        key: "aggressiveness",
        replacement: "Removed (D9, 2026-11-04): the aggressiveness dial is gone; machine \
                      rigidity is job.machine.rigidity. Delete the key",
    },
    RetiredKey {
        table: &["job", "machine", "rigidity"],
        key: "woc_roughing_factor",
        replacement: "Removed (feeds contract P11c, D13 item 2): no code read it. Delete the \
                      key; set class = \"hobby\" or \"industrial\" in [job.machine]",
    },
    RetiredKey {
        table: &["job", "machine", "rigidity"],
        key: "woc_roughing_max_mm",
        replacement: "Removed (feeds contract P11c, review item M11): no code read it. Delete \
                      the key",
    },
    RetiredKey {
        table: &["job", "machine", "rigidity"],
        key: "woc_finishing_mm",
        replacement: "Removed (feeds contract P11c, review item M11): no code read it. Delete \
                      the key",
    },
    RetiredKey {
        table: &["job", "stock", "material", "Custom"],
        key: "kc",
        replacement: "Removed (ruling B6): a custom material has no force line and no Kc. \
                      Delete the key",
    },
    RetiredKey {
        table: &["tools", "[]"],
        key: "non_centre_cutting",
        replacement: "Removed (K3, feeds contract P6): write centre_cutting = { stated = \"no\", \
                      centre_core_diameter_mm = .., blade_protrusion_mm = .. } for a tool that \
                      does not cut at its centre, or { stated = \"yes\" }; with no key the tool \
                      is not known to cut at its centre. Delete the key",
    },
    RetiredKey {
        table: &["tools", "[]"],
        key: "holder_diameter",
        replacement: "Removed (H1): the collet and the parts above the holder face are the \
                      machine head, [job.machine.head], one for every tool. Delete the key",
    },
];

/// One retired value of a project-file key: the table that holds the
/// key, the key, the string value that left the format, and what replaced
/// it (decision D8, geometry selection P2.5).
///
/// A unit enum variant serializes as a VALUE (`source = "face_selection"`),
/// so a [`RetiredKey`] row cannot find it, and serde would refuse it with
/// "unknown variant" and name no fix.
struct RetiredValue {
    table: &'static [&'static str],
    key: &'static str,
    value: RetiredLiteral,
    replacement: &'static str,
}

/// The value of a [`RetiredValue`] row, compared as the TOML value: a
/// string (a unit enum variant) or a boolean (G-A: `z_blend = true`).
#[derive(Clone, Copy)]
enum RetiredLiteral {
    Str(&'static str),
    Bool(bool),
}

impl RetiredLiteral {
    /// True when the file value `value` is this literal.
    fn matches(self, value: &toml::Value) -> bool {
        match self {
            Self::Str(s) => value.as_str() == Some(s),
            Self::Bool(b) => value.as_bool() == Some(b),
        }
    }

    /// The literal as the file writes it.
    fn file_text(self) -> String {
        match self {
            Self::Str(s) => format!("\"{s}\""),
            Self::Bool(b) => b.to_string(),
        }
    }
}

/// Every retired string value. The loader refuses a file that holds one,
/// with the same refusal as a retired key ([`check_retired_keys`]). Add a
/// row when a unit variant leaves the file format.
const RETIRED_VALUES: &[RetiredValue] = &[
    RetiredValue {
        table: &["setups", "[]", "toolpaths", "[]", "boundary"],
        key: "source",
        value: RetiredLiteral::Str("face_selection"),
        replacement: "Removed (geometry selection P2.5): a face boundary is a \
                      [boundary.source.selection] with the face picks. Pick the faces again",
    },
    // Removal step R2 (operator ruling 2026-10-28): three 3D clearing
    // strategies left the format. Nothing converts: another strategy cuts
    // another toolpath, so the operator chooses.
    RetiredValue {
        table: TOOLPATH_PARAMS,
        key: "clearing_strategy",
        value: RetiredLiteral::Str("adaptive"),
        replacement: CLEARING_STRATEGY_REPLACEMENT,
    },
    RetiredValue {
        table: TOOLPATH_PARAMS,
        key: "clearing_strategy",
        value: RetiredLiteral::Str("agent_search"),
        replacement: CLEARING_STRATEGY_REPLACEMENT,
    },
    RetiredValue {
        table: TOOLPATH_PARAMS,
        key: "clearing_strategy",
        value: RetiredLiteral::Str("contour_spiral"),
        replacement: CLEARING_STRATEGY_REPLACEMENT,
    },
    // G-A (operator ruling D-3, 2026-10-28): Z Blend left the format. It cut
    // the inner Contour Parallel rings deeper than one Depth/Pass, and
    // Geodesic Adaptive never read it. `z_blend = false` (every older 3D
    // Rough saved it) loads, and the key is ignored: the output is the same.
    // `true` refuses, because the output of a Contour Parallel op would
    // change with no word.
    RetiredValue {
        table: TOOLPATH_PARAMS,
        key: "z_blend",
        value: RetiredLiteral::Bool(true),
        replacement: Z_BLEND_REPLACEMENT,
    },
];

/// The parameter table of every toolpath of a project file.
const TOOLPATH_PARAMS: &[&str] = &["setups", "[]", "toolpaths", "[]", "operation", "params"];

const Z_BLEND_REPLACEMENT: &str = "Removed (GUI fix G-A, operator ruling 2026-10-28): \
     Z Blend cut deeper than one Depth/Pass. Delete the key; the op then cuts one Depth/Pass \
     per level";

const CLEARING_STRATEGY_REPLACEMENT: &str = "Removed (removal step R2, operator ruling \
     2026-10-28): the 3D Adaptive clearing strategies are \"geodesic_adaptive\" and \
     \"contour_parallel\". Set clearing_strategy to one of them";

/// The replacement text of a retired value of a toolpath parameter
/// (`operation.params.<param> = "<value>"`), or `None`. The parameter
/// setter reads it (`set_toolpath_param`), so MCP and a job file refuse a
/// retired value with the same text as the project loader.
///
/// A boolean row names a parameter that left the format (G-A `z_blend`):
/// the setter refuses it with ANY value, because there is no field left to
/// set. The loader still loads the harmless value (`false`).
pub(crate) fn retired_param_value(param: &str, value: &serde_json::Value) -> Option<&'static str> {
    RETIRED_VALUES
        .iter()
        .find(|r| {
            r.table == TOOLPATH_PARAMS
                && r.key == param
                && match r.value {
                    RetiredLiteral::Str(s) => value.as_str() == Some(s),
                    RetiredLiteral::Bool(_) => true,
                }
        })
        .map(|r| r.replacement)
}

/// GA2: what a parameter table of a job file (`[[operation]]`) does with a
/// key that the schema does not publish, by the [`RETIRED_VALUES`] rows:
/// the same rule as the project loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetiredParam {
    /// The key left the format with this value: refuse with the text.
    Refuse(&'static str),
    /// The key left the format, and this value changes nothing (G-A
    /// `z_blend = false`): drop it, as the project loader does.
    Drop,
}

/// GA2: the [`RetiredParam`] of the toolpath parameter `param = value` in a
/// file, or `None` when no [`RETIRED_VALUES`] row names the key. A row with
/// a boolean value refuses that value; another boolean of a boolean row is
/// dropped, because the project loader loads it and ignores the key.
#[must_use]
pub fn retired_file_param(param: &str, value: &toml::Value) -> Option<RetiredParam> {
    let rows: Vec<&RetiredValue> = RETIRED_VALUES
        .iter()
        .filter(|r| r.table == TOOLPATH_PARAMS && r.key == param)
        .collect();
    if let Some(row) = rows.iter().find(|r| r.value.matches(value)) {
        return Some(RetiredParam::Refuse(row.replacement));
    }
    rows.iter()
        .any(|r| matches!(r.value, RetiredLiteral::Bool(_)) && value.is_bool())
        .then_some(RetiredParam::Drop)
}

/// One retired key of a toolpath table that left the format for some op
/// kinds only (geometry selection P3.2). A [`RetiredKey`] matches a path
/// and a key, but `depth` still lives on Face, Trace and Project Curve,
/// and `heights.bottom_z` on every op. This row reads
/// `operation.kind` of each `setups[].toolpaths[]` entry first.
struct RetiredOpEntry {
    /// The `operation.kind` values the row applies to.
    kinds: &'static [&'static str],
    /// The path from the toolpath table to the table that holds the key.
    table: &'static [&'static str],
    key: &'static str,
    /// `None`: any value retires. `Some(rule)`: only a value for which the
    /// rule holds.
    when: Option<fn(&toml::Value) -> bool>,
    replacement: &'static str,
}

/// The kinds whose floor is the `floor` field (ruling D9).
const FLOOR_OP_KINDS: &[&str] = &["pocket", "profile", "zigzag", "adaptive"];

/// A Heights row that is not Auto (`mode` is not `"auto"`).
fn pinned_height(value: &toml::Value) -> bool {
    value.get("mode").and_then(toml::Value::as_str) != Some("auto")
}

/// Every retired op-kind key. The loader refuses a file that holds one,
/// with the same refusal as a retired key ([`check_retired_keys`]).
///
/// P3.2 (lead ruling O-1): nothing converts. `depth = d` would cut the same
/// as `floor = { from = "top", depth = d }`, but a conversion is an alias.
/// A pinned Bottom Z on these ops never reached the cut (G-BOTTOMPIN), and
/// a conversion to a Z floor would cut deeper with no word.
const RETIRED_OP_ENTRIES: &[RetiredOpEntry] = &[
    RetiredOpEntry {
        kinds: FLOOR_OP_KINDS,
        table: &["operation", "params"],
        key: "depth",
        when: None,
        replacement: "Removed (geometry selection P3, ruling D9): the floor of this op is \
                      operation.params.floor. For the same cut write floor = { from = \"top\", \
                      depth = <this value> } in the same table. Delete the key",
    },
    RetiredOpEntry {
        kinds: FLOOR_OP_KINDS,
        table: &["heights"],
        key: "bottom_z",
        when: Some(pinned_height),
        replacement: "Removed (geometry selection P3, ruling D9): the floor of this op is \
                      operation.params.floor. A pinned Bottom Z never reached the cut of this op \
                      (G-BOTTOMPIN). Set bottom_z to { mode = \"auto\" } and set the floor",
    },
    // R-B, F19 (2026-10-23): the Drill depth counted from the stock top.
    // The Drill floor counts a depth from the rim of each hole, so the old
    // number names `stock_top`, not `top`.
    RetiredOpEntry {
        kinds: &["drill"],
        table: &["operation", "params"],
        key: "depth",
        when: None,
        replacement: "Removed (R-B, F19): the floor of a Drill is operation.params.floor. The \
                      old depth counted from the stock top: for the same cut write floor = { \
                      from = \"stock_top\", depth = <this value> } in the same table, or floor = \
                      { from = \"each_hole\", depth = 0.0 } to drill each hole of a STEP model \
                      to its own floor. Delete the key",
    },
    RetiredOpEntry {
        kinds: &["drill"],
        table: &["heights"],
        key: "bottom_z",
        when: Some(pinned_height),
        replacement: "Removed (R-B, F19): the floor of a Drill is operation.params.floor. A \
                      pinned Bottom Z never reached the cut of a Drill. Set bottom_z to { mode = \
                      \"auto\" } and set the floor",
    },
    // Feeds contract P0, decision D2 (2026-11-03): the Drill dwell field.
    // The Peck and Chip-break cycles never read it; the Dwell cycle (G82)
    // reads the one fixed dwell `DRILL_DWELL_S`, as the Pin Drill does.
    RetiredOpEntry {
        kinds: &["drill"],
        table: &["operation", "params"],
        key: "dwell_time",
        when: None,
        replacement: "Removed (feeds contract P0, decision D2): the Drill has no dwell field. \
                      The Peck and Chip-break cycles never dwelled; the Dwell cycle (G82) \
                      dwells the fixed 0.5 s of the Pin Drill (DRILL_DWELL_S). Delete the key",
    },
];

/// Push the path of every table under `steps` for which `hit` holds onto
/// `found`, with `name` (the key) appended.
fn find_retired(
    value: &toml::Value,
    steps: &[&str],
    name: &str,
    hit: &dyn Fn(&toml::Table) -> bool,
    at: &str,
    found: &mut Vec<String>,
) {
    let join = |step: &str| {
        if at.is_empty() {
            step.to_owned()
        } else {
            format!("{at}.{step}")
        }
    };
    match steps.split_first() {
        None => {
            if value.as_table().is_some_and(hit) {
                found.push(join(name));
            }
        }
        Some((&"[]", rest)) => {
            for (i, entry) in value.as_array().into_iter().flatten().enumerate() {
                find_retired(entry, rest, name, hit, &format!("{at}[{i}]"), found);
            }
        }
        Some((step, rest)) => {
            if let Some(next) = value.as_table().and_then(|t| t.get(*step)) {
                find_retired(next, rest, name, hit, &join(step), found);
            }
        }
    }
}

/// Refuse a project file that holds a key from [`RETIRED_KEYS`], a value
/// from [`RETIRED_VALUES`] or an op-kind key from [`RETIRED_OP_ENTRIES`].
/// C2: the refusal names EVERY retired key and value in the file, so one
/// edit pass clears them all.
fn check_retired_keys(raw: &toml::Value) -> Result<(), SessionError> {
    let mut found = Vec::new();
    for retired in RETIRED_KEYS {
        let mut paths = Vec::new();
        let hit = |t: &toml::Table| t.contains_key(retired.key);
        find_retired(raw, retired.table, retired.key, &hit, "", &mut paths);
        found.extend(paths.into_iter().map(|path| super::RetiredKeyFound {
            path,
            replacement: retired.replacement,
        }));
    }
    for retired in RETIRED_VALUES {
        let mut paths = Vec::new();
        let hit = |t: &toml::Table| t.get(retired.key).is_some_and(|v| retired.value.matches(v));
        find_retired(raw, retired.table, retired.key, &hit, "", &mut paths);
        found.extend(paths.into_iter().map(|path| super::RetiredKeyFound {
            path: format!("{path} = {}", retired.value.file_text()),
            replacement: retired.replacement,
        }));
    }
    found.extend(retired_op_entries(raw));
    if found.is_empty() {
        Ok(())
    } else {
        Err(SessionError::RetiredKeys(found))
    }
}

/// The [`RETIRED_OP_ENTRIES`] hits of a raw project file: one per old key,
/// in the file order of the toolpaths.
fn retired_op_entries(raw: &toml::Value) -> Vec<super::RetiredKeyFound> {
    let mut found = Vec::new();
    let setups = raw.get("setups").and_then(toml::Value::as_array);
    for (si, setup) in setups.into_iter().flatten().enumerate() {
        let toolpaths = setup.get("toolpaths").and_then(toml::Value::as_array);
        for (ti, toolpath) in toolpaths.into_iter().flatten().enumerate() {
            let kind = toolpath
                .get("operation")
                .and_then(|op| op.get("kind"))
                .and_then(toml::Value::as_str);
            for entry in RETIRED_OP_ENTRIES {
                if !kind.is_some_and(|k| entry.kinds.contains(&k)) {
                    continue;
                }
                let table = entry
                    .table
                    .iter()
                    .try_fold(toolpath, |value, step| value.get(*step));
                let Some(value) = table.and_then(|t| t.get(entry.key)) else {
                    continue;
                };
                if entry.when.is_some_and(|rule| !rule(value)) {
                    continue;
                }
                found.push(super::RetiredKeyFound {
                    path: format!(
                        "setups[{si}].toolpaths[{ti}].{}.{}",
                        entry.table.join("."),
                        entry.key
                    ),
                    replacement: entry.replacement,
                });
            }
        }
    }
    found
}

/// K3 (ruling Q9): refuse a project file that holds a tool fact as a bare
/// value. ONE refusal names every bare key in the file.
fn check_bare_tool_facts(raw: &toml::Value) -> Result<(), SessionError> {
    let tools = raw.get("tools").and_then(toml::Value::as_array);
    let found = crate::compute::tool_config::bare_fact_keys(
        tools
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, t)| (format!("tools[{i}]"), t)),
    );
    if found.is_empty() {
        Ok(())
    } else {
        Err(SessionError::BareToolFacts(found))
    }
}

impl ProjectFile {
    /// Parse a project file from its text. This is the one text door:
    /// it refuses a format that is not 3 and a retired key
    /// ([`RETIRED_KEYS`]) before it reads the typed sections.
    pub fn from_toml_str(text: &str) -> Result<Self, SessionError> {
        let raw: toml::Value =
            toml::from_str(text).map_err(|e| SessionError::TomlParse(e.to_string()))?;
        // F034: the version check comes before the closed typed parse. An
        // older format holds keys this one does not (format 2 kept the
        // pins on the setup), and its refusal must name the version, not
        // the first key it does not know.
        // A file with no key falls through: the typed parse and
        // `check_format_version` judge it (a TOML that is not a project at
        // all then reads as "not an rs_cam project").
        let found = raw
            .get("format_version")
            .and_then(toml::Value::as_integer)
            .and_then(|n| u32::try_from(n).ok());
        if let Some(found) = found
            && found != SUPPORTED_FORMAT_VERSION
        {
            return Err(SessionError::UnsupportedFormatVersion { found });
        }
        check_retired_keys(&raw)?;
        check_bare_tool_facts(&raw)?;
        // The second parse keeps the line and column in a typed error.
        toml::from_str(text).map_err(|e| SessionError::TomlParse(e.to_string()))
    }
}

/// Something a project load noticed, kept for the operator to read.
///
/// The loader does not fail on any of these. It loads what it can. It then
/// reports what it could not. The GUI shows `message()` in the load
/// warnings window. A headless caller may ignore the list.
///
/// The type lives here because the loader is the only producer. Before
/// C10 the GUI re-derived the same sentences from the loaded session.
/// That is how the two loaders in I01 came to word one fact two ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectLoadWarning {
    /// A model section named a file the loader could not read.
    ModelLoadFailed {
        name: String,
        path: PathBuf,
        detail: String,
    },
    /// A tool section carried a tool-type token the vocabulary does not
    /// hold. The loader substitutes an end mill.
    UnknownToolType { tool: String, token: String },
    /// A tool section carried a `tool_material` token the vocabulary does
    /// not hold. The loader substitutes carbide (FC0 item 6).
    UnknownToolMaterial { tool: String, token: String },
    /// A tool section carried a `cut_direction` token the vocabulary does
    /// not hold. The loader substitutes up-cut (FC0 item 6). The cut
    /// direction picks the vendor series of the feeds rows.
    UnknownCutDirection { tool: String, token: String },
    /// FC0 item 7 (decision D4 = warn): a tool section with a stick-out
    /// shorter than its flute. The loader keeps both values.
    StickoutShorterThanFlute { tool: String, text: String },
    /// A setup section carried a `face_up` token the vocabulary does not
    /// hold. The loader substitutes Top.
    ///
    /// CMP-15: `FaceUp::from_key` used to end `_ => FaceUp::Top` and say
    /// nothing. `face_up` chooses the cut direction, the local stock bbox
    /// and the whole emission frame, so one typo turned a bottom setup
    /// into a top one with no word to anyone.
    UnknownFaceUp { setup: String, token: String },
    /// A setup section carried a `z_rotation` token the vocabulary does
    /// not hold. The loader substitutes 0 degrees. Same defect class as
    /// [`Self::UnknownFaceUp`].
    UnknownZRotation { setup: String, token: String },
    /// A toolpath names a tool id no `[[tools]]` section defines.
    MissingToolReference { toolpath: String, tool_id: ToolId },
    /// A toolpath names a model id no `[[models]]` section defines.
    MissingModelReference { toolpath: String, model_id: ModelId },
    /// The stock's alignment pins do not key the flip a setup is
    /// programmed for.
    ///
    /// CMP-27: `StockConfig::validate_pins_for_flip`'s own doc says "Call
    /// this on load and publish `PinFlipReport::warnings`", and only the
    /// GUI load path did. The CLI's headless load and MCP `load_project`
    /// heard nothing. The consequence is registration: a pin pair that is
    /// invariant under the wrong symmetry seats in the flipped
    /// orientation and in the un-flipped one, and both look right.
    ///
    /// `detail` is the report's own sentence, verbatim, so the GUI — which
    /// runs the same check on its own load path and dedupes by exact
    /// message — collapses the pair instead of printing it twice.
    AlignmentPinsUnkeyed { detail: String },
    /// A toolpath carried a dressup value its operation does not allow.
    /// The loader rewrote the value. The file still holds the old one
    /// until the operator saves the project again.
    DressupsNormalized {
        toolpath: String,
        op: String,
        /// One `dial: before to after` phrase per rewritten value.
        changes: Vec<String>,
    },
    /// W1b: a Waterline stored the old key `continuous` (`stored` is its
    /// value). The loader turned the helical waterline on
    /// (`WaterlineConfig::helical`, operator ruling 2026-10-28).
    WaterlineHelical { toolpath: String, stored: bool },
    /// A toolpath's rest-analysis flag did not match the demand the
    /// project declares, so the loader rewrote it. `enabled` is the value
    /// the loader wrote.
    RestAnalysisNormalized { toolpath: String, enabled: bool },
    /// S3: a stock change breaks a validation rule against the loaded
    /// models (a deleted model, a model with no mesh or no closed outline,
    /// ...). The loader keeps the change; the simulation refuses it by
    /// name until the operator fixes or removes it.
    StockChangeRefused {
        setup: String,
        change: String,
        reason: String,
    },
}

impl ProjectLoadWarning {
    /// The operator-facing sentence for this warning.
    pub fn message(&self) -> String {
        match self {
            Self::ModelLoadFailed { name, path, detail } => format!(
                "Model '{name}' could not be loaded from '{}': {detail}",
                path.display()
            ),
            Self::UnknownToolType { tool, token } => {
                format!("Tool '{tool}' has unknown tool type '{token}' — defaulted to End Mill.")
            }
            Self::UnknownToolMaterial { tool, token } => format!(
                "Tool '{tool}' has unknown tool material '{token}' — defaulted to carbide. \
                 Valid values are carbide and hss."
            ),
            Self::StickoutShorterThanFlute { tool, text } => format!("Tool '{tool}': {text}"),
            Self::UnknownCutDirection { tool, token } => format!(
                "Tool '{tool}' has unknown cut direction '{token}' — defaulted to up-cut. The cut \
                 direction picks the vendor series of the feeds rows. Valid values are up_cut, \
                 down_cut and compression."
            ),
            Self::UnknownFaceUp { setup, token } => format!(
                "Setup '{setup}' has unknown face up '{token}' — defaulted to Top. The cut \
                 direction, the local stock box and the emission frame all follow this value."
            ),
            Self::UnknownZRotation { setup, token } => format!(
                "Setup '{setup}' has unknown Z rotation '{token}' — defaulted to 0 degrees. \
                 Valid values are 0, 90, 180 and 270."
            ),
            // Verbatim: the report wrote the sentence, and re-wording it
            // here is how one fact came to be worded two ways before C10.
            Self::AlignmentPinsUnkeyed { detail } => detail.clone(),
            Self::MissingToolReference { toolpath, tool_id } => format!(
                "Toolpath '{toolpath}' references missing tool id {tool_id} and needs reassignment."
            ),
            Self::MissingModelReference { toolpath, model_id } => format!(
                "Toolpath '{toolpath}' references missing model id {model_id} and needs reassignment."
            ),
            Self::StockChangeRefused {
                setup,
                change,
                reason,
            } => format!(
                "Setup '{setup}' has stock change '{change}' that cannot apply: {reason}. \
                 The simulation refuses it. Edit the change or remove it."
            ),
            Self::RestAnalysisNormalized { toolpath, enabled } => {
                let state = if *enabled { "on" } else { "off" };
                let because = if *enabled {
                    "another toolpath uses its rest regions as a machining boundary"
                } else {
                    "no toolpath uses its rest regions as a machining boundary"
                };
                format!(
                    "Toolpath '{toolpath}' had its rest analysis switched {state}: {because}. \
                     Save the project to keep the change."
                )
            }
            Self::DressupsNormalized {
                toolpath,
                op,
                changes,
            } => format!(
                "Toolpath '{toolpath}' carried dressups the {op} operation does not allow. \
                 The loader changed {}. Save the project to keep the change.",
                changes.join(", ")
            ),
            Self::WaterlineHelical { toolpath, stored } => {
                format!(
                    "Waterline '{toolpath}': helical descent is now on (was continuous = {stored})."
                )
            }
        }
    }
}

/// Job-level settings (name, stock, post).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectJobSection {
    #[serde(default = "default_job_name")]
    pub name: String,
    #[serde(default)]
    pub stock: ProjectStockConfig,
    #[serde(default)]
    pub post: crate::gcode::PostConfig,
    #[serde(default)]
    pub machine: crate::machine::MachineProfile,
    /// G-RESTRES: the one stored simulation resolution. A file with no
    /// `[job.simulation]` table, or no `resolution_mm` key, loads as `Auto`.
    #[serde(default, skip_serializing_if = "ProjectSimulationSection::is_auto")]
    pub simulation: ProjectSimulationSection,
}

/// `[job.simulation]`: the stored simulation resolution.
///
/// `resolution_mm` present = `SimulationResolution::Fixed`; absent =
/// `SimulationResolution::Auto`, the documented default.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectSimulationSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_mm: Option<f64>,
}

impl ProjectSimulationSection {
    /// The section as the session holds it.
    #[must_use]
    pub fn resolution(&self) -> super::SimulationResolution {
        self.resolution_mm.map_or(
            super::SimulationResolution::Auto,
            super::SimulationResolution::Fixed,
        )
    }

    /// The section for a session value.
    #[must_use]
    pub fn of(resolution: super::SimulationResolution) -> Self {
        Self {
            resolution_mm: match resolution {
                super::SimulationResolution::Auto => None,
                super::SimulationResolution::Fixed(mm) => Some(mm),
            },
        }
    }

    fn is_auto(&self) -> bool {
        self.resolution_mm.is_none()
    }
}

fn default_job_name() -> String {
    "Untitled".to_owned()
}

/// Stock dimensions as saved in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectStockConfig {
    #[serde(default = "default_stock_dim")]
    pub x: f64,
    #[serde(default = "default_stock_dim")]
    pub y: f64,
    /// S11: a file with no `z` loads a stock whose thickness is not set;
    /// the file writes no `z` for such a stock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    #[serde(default = "default_stock_padding")]
    pub padding: f64,
    // Ruling R4 Q8 (2026-09-24): `workholding_rigidity` is gone. The
    // loader refuses an old file's key (`RETIRED_KEYS`).
    #[serde(default = "default_true")]
    pub auto_from_model: bool,
    #[serde(default)]
    pub material: crate::material::Material,
    #[serde(default)]
    pub alignment_pins: Vec<crate::compute::alignment_pins::AlignmentPin>,
    /// The models that count as the part for the pins (P7). Absent means
    /// the default rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_part_models: Option<Vec<ModelId>>,
    // P3 (2026-10-12): `flip_axis` is gone. The flip axis is derived from
    // the setups. The loader refuses an old file's key (`RETIRED_KEYS`).
}

impl Default for ProjectStockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            // S11: no rule gives a thickness.
            z: None,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            auto_from_model: true,
            material: crate::material::Material::default(),
            alignment_pins: Vec::new(),
            pin_part_models: None,
        }
    }
}

fn default_stock_dim() -> f64 {
    100.0
}
fn default_stock_padding() -> f64 {
    5.0
}

// UI-07: `ProjectPostConfig` lived here — a second post config whose
// `format` was a `String`, with a hand-written translation to and from the
// GUI's `PostConfig` in each direction. The project file now holds
// `crate::gcode::PostConfig` itself. The on-disk bytes are unchanged:
// `PostFormat`'s serde spelling is the token the writer already wrote.

/// Tool definition in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectToolSection {
    #[serde(default)]
    pub id: Option<ToolId>,
    #[serde(default = "default_tool_name")]
    pub name: String,
    #[serde(rename = "type", default = "default_tool_type_str")]
    pub tool_type: String,
    #[serde(default = "default_tool_diameter")]
    pub diameter: f64,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub cutting_length: ToolFact<f64>,
    #[serde(default = "default_helix_deg")]
    pub helix_deg: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f64,
    #[serde(default = "default_included_angle")]
    pub included_angle: f64,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub v_tip_diameter: ToolFact<f64>,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub taper_half_angle: ToolFact<f64>,
    #[serde(default = "default_shaft_diameter")]
    pub shaft_diameter: f64,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub shank_diameter: ToolFact<f64>,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub stickout: ToolFact<f64>,
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    pub flute_count: ToolFact<u32>,
    #[serde(default)]
    pub tool_number: Option<usize>,
    #[serde(default = "default_tool_material")]
    pub tool_material: String,
    #[serde(default = "default_cut_direction")]
    pub cut_direction: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub product_id: String,
    /// The unit the tool's size is shown and entered in. Absent means
    /// "infer from the name". Display only: every length above is mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_units: Option<crate::compute::tool_config::SizeUnits>,
    /// K3: `{ stated = "yes" }` or `{ stated = "no", centre_core_diameter_mm
    /// = .., blade_protrusion_mm = .. }` (or the catalogue and estimated
    /// forms); an absent key is `Unknown`. The key `non_centre_cutting`
    /// is retired (`RETIRED_KEYS`).
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown",
        with = "crate::compute::tool_config::centre_cutting_serde"
    )]
    pub centre_cutting: ToolFact<crate::compute::tool_config::CentreCutting>,
}

fn default_tool_name() -> String {
    "Tool".to_owned()
}
fn default_tool_type_str() -> String {
    "end_mill".to_owned()
}
fn default_tool_diameter() -> f64 {
    6.35
}
fn default_helix_deg() -> f64 {
    crate::tool::DEFAULT_HELIX_DEG
}
fn default_corner_radius() -> f64 {
    2.0
}
fn default_included_angle() -> f64 {
    90.0
}
fn default_shaft_diameter() -> f64 {
    6.35
}
fn default_tool_material() -> String {
    "carbide".to_owned()
}
fn default_cut_direction() -> String {
    "up_cut".to_owned()
}

/// Model reference in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectModelSection {
    #[serde(default)]
    pub id: Option<ModelId>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<ModelKind>,
    #[serde(default)]
    pub units: Option<ModelUnits>,
    /// P4.8: `[models.import]`, the import options (the join tolerance).
    /// Additive under `format_version = 3`: a file with no key loads the
    /// defaults, which give the import before P4.8, and default options
    /// write no key.
    #[serde(default, skip_serializing_if = "crate::io::ImportOptions::is_default")]
    pub import: crate::io::ImportOptions,
    /// P4.9: `[[models.drawing_edits]]`, the edit ledger of a drawing (the
    /// join and close acts, in order). Every key anchor, key measure and
    /// tolerance is in FILE units: the frame of `import.join_tolerance_mm`,
    /// the units of the file as the door reads it (a DXF after
    /// `$INSUNITS`), before the declared `units`. A rescale changes no
    /// stored edit. Additive under `format_version = 3`:
    /// a file with no key loads an empty ledger, and an empty ledger writes
    /// no key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawing_edits: Vec<crate::selection::DrawingEdit>,
}

/// Setup section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSetupSection {
    #[serde(default)]
    pub id: Option<SetupId>,
    #[serde(default = "default_setup_name")]
    pub name: String,
    #[serde(default = "default_face_up")]
    pub face_up: String,
    #[serde(default)]
    pub z_rotation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_message: Option<String>,
    /// Setup datum — how the operator zeroes the machine (W9 / P-2).
    /// Key names and spellings came from the viz fallback schema, which
    /// wrote them first; C11 deleted that schema, so this is now the only
    /// place the project file defines them. A file written by the old
    /// schema still loads here. All three are written only when
    /// non-default, so files saved before this landed and projects that
    /// never touched the datum stay byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub xy_datum: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub z_datum: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datum_notes: String,
    /// Models in scope for this setup; empty = all (W9 / P-2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<ModelId>,
    /// K1: `[[setups.obstacles]]`, the setup's obstacles (fixtures and
    /// keep-out zones are one concept). Every value is in the setup frame.
    /// An empty list writes no key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub obstacles: Vec<crate::compute::obstacle::Obstacle>,
    /// S2: `[[setups.stock_changes]]`, the setup's stock changes in
    /// application order. Additive under `format_version = 3`: a file with
    /// no key loads an empty list, and an empty list writes no key, so a
    /// project with no stock change saves byte-identical to before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_changes: Vec<crate::compute::stock_change::StockChange>,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

fn default_setup_name() -> String {
    "Setup 1".to_owned()
}
fn default_face_up() -> String {
    "top".to_owned()
}

/// Toolpath section in the project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectToolpathSection {
    #[serde(default)]
    pub id: Option<crate::ids::ToolpathId>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub operation: Option<OperationConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// R2 (changes F053): optional. A toolpath with no tool writes no
    /// `tool_id` key and loads with no tool (it refuses generation until a
    /// tool is set). A stated id is EXACT (P2.4/P2.5): it never binds to
    /// another tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<ToolId>,
    /// F053: required. A toolpath with no model id used to bind to model 0
    /// with no word; the ids are EXACT (P2.4/P2.5).
    pub model_id: ModelId,
    #[serde(default)]
    pub dressups: DressupConfig,
    #[serde(default)]
    pub heights: HeightsConfig,
    /// Raw G-code to emit before this toolpath's moves.
    #[serde(default)]
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    #[serde(default)]
    pub post_gcode: Option<String>,
    /// Machining boundary configuration.
    #[serde(default)]
    pub boundary: BoundaryConfig,
    /// When true, inherit boundary from stock default.
    #[serde(default = "default_true")]
    pub boundary_inherit: bool,
    /// Where this toolpath's stock material comes from.
    #[serde(default)]
    pub stock_source: StockSource,
    /// Coolant mode for G-code output.
    #[serde(default)]
    pub coolant: CoolantMode,
    /// Debug trace options.
    #[serde(default)]
    pub debug_options: ToolpathDebugOptions,
    /// Per-dimension provenance of the stored feeds values (W2.1). Absent in
    /// projects saved before provenance tracking — defaults to all-`None`.
    #[serde(default, skip_serializing_if = "feeds_provenance_is_empty")]
    pub feeds_provenance: crate::feeds::FeedsProvenance,
    /// Op-agnostic rest analysis (P2.5). Absent in projects saved before this
    /// config existed — defaults to disabled.
    #[serde(default, skip_serializing_if = "rest_analysis_is_default")]
    pub rest_analysis: crate::compute::config::RestAnalysisConfig,
    /// Multi-tool planner provenance (Phase O item 1). Absent in every
    /// project written before the planner existed — and absent on every
    /// hand-built op forever after, which is why it is skipped when `None`
    /// rather than written as an empty table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_origin: Option<crate::session::PlannerOrigin>,
    /// H1: the dust shoe setting. Absent: Auto.
    #[serde(
        default,
        skip_serializing_if = "crate::machine::head::ShoeSetting::is_auto"
    )]
    pub shoe: crate::machine::head::ShoeSetting,
    /// W3k: the Feeds tab lock (`feed`, `spindle`, `chipload`). Absent:
    /// `chipload`, the default, which the file does not write.
    #[serde(default, skip_serializing_if = "crate::feeds::FeedsLock::is_default")]
    pub feeds_lock: crate::feeds::FeedsLock,
}

fn rest_analysis_is_default(r: &crate::compute::config::RestAnalysisConfig) -> bool {
    *r == crate::compute::config::RestAnalysisConfig::default()
}

fn feeds_provenance_is_empty(p: &crate::feeds::FeedsProvenance) -> bool {
    *p == crate::feeds::FeedsProvenance::default()
}

fn default_true() -> bool {
    true
}

// ── Free functions: project file to session state ─────────────────────

pub(crate) fn stock_from_project(ps: &ProjectStockConfig) -> StockConfig {
    StockConfig {
        x: ps.x,
        y: ps.y,
        z: ps.z,
        origin_x: ps.origin_x,
        origin_y: ps.origin_y,
        origin_z: ps.origin_z,
        auto_from_model: ps.auto_from_model,
        padding: ps.padding,
        material: ps.material.clone(),
        alignment_pins: ps.alignment_pins.clone(),
        pin_part_models: ps.pin_part_models.clone(),
    }
}

/// Q4 warn-and-default file-loading policy over the unified
/// [`ToolType::parse_lenient`] vocabulary (T8). Pre-T8 this had its own
/// alias table and a SILENT `_ => EndMill` — an unknown token became an
/// end mill with no trace. L9 deleted the aliases, so a file that
/// carries one now takes this arm and the operator reads the warning.
pub(crate) fn parse_tool_type(
    s: &str,
    tool_name: &str,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> ToolType {
    ToolType::parse_lenient(s).unwrap_or_else(|| {
        tracing::warn!(
            tool_type = s,
            "unknown tool type in project file — defaulting to end_mill"
        );
        warnings.push(ProjectLoadWarning::UnknownToolType {
            tool: tool_name.to_owned(),
            token: s.to_owned(),
        });
        ToolType::EndMill
    })
}

/// Read a setup's `face_up` key. An unknown token warns and takes Top —
/// the Q4 policy stated on `tool_config.rs` (a file-loading surface warns
/// and defaults; the MCP mutation surface refuses, which `add_setup` and
/// `set_setup_face` already do).
///
/// CMP-15: the loader used to call `FaceUp::from_key` and get a silent
/// `Top` for any token at all.
fn parse_face_up(s: &str, setup_name: &str, warnings: &mut Vec<ProjectLoadWarning>) -> FaceUp {
    FaceUp::from_key(s).unwrap_or_else(|| {
        tracing::warn!(
            face_up = s,
            setup = setup_name,
            "unknown face up in project file — defaulting to top"
        );
        warnings.push(ProjectLoadWarning::UnknownFaceUp {
            setup: setup_name.to_owned(),
            token: s.to_owned(),
        });
        FaceUp::Top
    })
}

/// Read a setup's `z_rotation` key. See [`parse_face_up`] for the policy.
///
/// The EMPTY string is the field's own serde default — a file that never
/// wrote the key. That is not a bad token, so it takes the default in
/// silence.
fn parse_z_rotation(
    s: &str,
    setup_name: &str,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> ZRotation {
    if s.is_empty() {
        return ZRotation::Deg0;
    }
    ZRotation::from_key(s).unwrap_or_else(|| {
        tracing::warn!(
            z_rotation = s,
            setup = setup_name,
            "unknown Z rotation in project file — defaulting to 0 degrees"
        );
        warnings.push(ProjectLoadWarning::UnknownZRotation {
            setup: setup_name.to_owned(),
            token: s.to_owned(),
        });
        ZRotation::Deg0
    })
}

pub(crate) fn tool_from_project_section(
    ts: &ProjectToolSection,
    idx: usize,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> ToolConfig {
    // A section with no id takes its position: the loader is the allocator.
    let tool_id = ts.id.unwrap_or(ToolId::mint(idx));
    let tool_number = ts.tool_number.unwrap_or(tool_id.raw() + 1) as u32;
    // K3 (feeds contract P6): each fact has the kind the file states. The
    // record literal names every field (no `..`), so a new `ToolConfig`
    // field does not compile here until the loader states it.
    let tool = ToolConfig::from_record(ToolRecord {
        id: tool_id,
        name: ts.name.clone(),
        tool_number,
        tool_type: parse_tool_type(&ts.tool_type, &ts.name, warnings),
        diameter: ts.diameter,
        cutting_length: ts.cutting_length.clone(),
        helix_deg: ts.helix_deg,
        corner_radius_mm: ts.corner_radius_mm,
        corner_radius: ts.corner_radius,
        included_angle: ts.included_angle,
        v_tip_diameter: ts.v_tip_diameter.clone(),
        taper_half_angle: ts.taper_half_angle.clone(),
        shaft_diameter: ts.shaft_diameter,
        shank_diameter: ts.shank_diameter.clone(),
        stickout: ts.stickout.clone(),
        flute_count: ts.flute_count.clone(),
        tool_material: parse_tool_material(&ts.tool_material, &ts.name, warnings),
        cut_direction: parse_cut_direction(&ts.cut_direction, &ts.name, warnings),
        vendor: ts.vendor.clone(),
        product_id: ts.product_id.clone(),
        size_units: ts.size_units,
        centre_cutting: ts.centre_cutting.clone(),
    });
    // FC0 item 7 (decision D4 = warn, ruling Q6): no refusal.
    if tool.stickout_shorter_than_flute() {
        warnings.push(ProjectLoadWarning::StickoutShorterThanFlute {
            tool: tool.name.clone(),
            text: tool.stickout_shorter_than_flute_text(),
        });
    }
    tool
}

/// Read a tool's `tool_material` key. The known tokens are the `save.rs`
/// spellings (`carbide`, `hss`), in any case. An unknown token warns and
/// takes carbide (FC0 item 6, the policy of [`parse_tool_type`]).
fn parse_tool_material(
    s: &str,
    tool_name: &str,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> crate::compute::tool_config::ToolMaterial {
    use crate::compute::tool_config::ToolMaterial;
    let token = s.to_ascii_lowercase();
    if token == "carbide" {
        return ToolMaterial::Carbide;
    }
    if token == "hss" {
        return ToolMaterial::Hss;
    }
    tracing::warn!(
        tool_material = s,
        tool = tool_name,
        "unknown tool material in project file — defaulting to carbide"
    );
    warnings.push(ProjectLoadWarning::UnknownToolMaterial {
        tool: tool_name.to_owned(),
        token: s.to_owned(),
    });
    ToolMaterial::Carbide
}

/// Read a tool's `cut_direction` key. The known tokens are the `save.rs`
/// spellings (`up_cut`, `down_cut`, `compression`) and the alias
/// `downcut`, in any case. An unknown token warns and takes up-cut (FC0
/// item 6, the policy of [`parse_tool_type`]).
fn parse_cut_direction(
    s: &str,
    tool_name: &str,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> crate::compute::tool_config::BitCutDirection {
    use crate::compute::tool_config::BitCutDirection;
    let token = s.to_ascii_lowercase();
    if token == "up_cut" {
        return BitCutDirection::UpCut;
    }
    if token == "down_cut" || token == "downcut" {
        return BitCutDirection::DownCut;
    }
    if token == "compression" {
        return BitCutDirection::Compression;
    }
    tracing::warn!(
        cut_direction = s,
        tool = tool_name,
        "unknown cut direction in project file — defaulting to up_cut"
    );
    warnings.push(ProjectLoadWarning::UnknownCutDirection {
        tool: tool_name.to_owned(),
        token: s.to_owned(),
    });
    BitCutDirection::UpCut
}

/// Resolve a stored model path against the project file's own directory.
///
/// G-MODELRELINK (F4.3): this rule used to live inside
/// [`load_model_geometry`] and be applied only for the READ — the session
/// then stored the raw string, so a project with `path = "m.stl"` kept a
/// path that resolves against the process working directory. Reload from
/// disk looked for `./m.stl` wherever `rs_cam_gui` was launched, and the
/// missing-model warning printed a string that named no directory anyone
/// had searched.
///
/// Hoisted so `LoadedModel::path` can hold the RESOLVED path, which is what
/// the interactive import door has always stored. One rule, one meaning:
/// **in memory a model path is absolute** (or as absolute as the stored
/// string and the project's location allow).
pub(crate) fn resolve_model_path(raw: &str, base_dir: &Path) -> PathBuf {
    let raw_path = Path::new(raw);
    if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    }
}

pub(crate) fn load_model_geometry(
    model: &ProjectModelSection,
    base_dir: &Path,
) -> Result<LoadedGeometry, SessionError> {
    let full_path = resolve_model_path(&model.path, base_dir);

    let kind = model
        .kind
        .or_else(|| crate::io::infer_kind_from_path(&full_path))
        .ok_or_else(|| SessionError::ModelLoad {
            name: model.name.clone(),
            detail: format!("Cannot determine file type for '{}'", full_path.display()),
        })?;

    // G-UNITSRELOAD (2026-08-22). The declared units must reach EVERY
    // geometry kind. A project file stores the model's *path* and its
    // declared units, not its geometry, so both doors re-import the same
    // file and must agree. An inch-authored DXF used to come back 25.4x
    // smaller after a save and a reload, silently, with the stock still at
    // its saved size. C13 removed the second reader that made such a
    // divergence possible; `model_units_survive_reload_g_unitsreload.rs`
    // still asserts the two doors agree.
    let scale = model
        .units
        .as_ref()
        .map(|u| u.scale_factor())
        .unwrap_or(1.0);

    crate::io::load_geometry(&full_path, kind, scale, &model.import).map_err(|detail| {
        SessionError::ModelLoad {
            name: model.name.clone(),
            detail,
        }
    })
}

/// Name every dressup value [`DressupConfig::normalize_for_op`] rewrote.
///
/// The policy rewrites three dials, so the reader names all three. A
/// policy that grows a fourth leaves the list empty; the last arm says
/// that plainly rather than reporting a silent nothing.
fn dressup_changes(before: &DressupConfig, after: &DressupConfig) -> Vec<String> {
    let mut changes = Vec::new();
    if before.entry_style != after.entry_style {
        changes.push(format!(
            "entry_style {:?} to {:?}",
            before.entry_style, after.entry_style
        ));
    }
    // CUT-13: the dressup is an `Option` now. The operator-facing line keeps
    // its "true to false" wording, which is what the enable key still says in
    // the project file.
    if before.lead_in_out.is_some() != after.lead_in_out.is_some() {
        changes.push(format!(
            "lead_in_out {} to {}",
            before.lead_in_out.is_some(),
            after.lead_in_out.is_some()
        ));
    }
    if before.link_moves.is_some() != after.link_moves.is_some() {
        changes.push(format!(
            "link_moves {} to {}",
            before.link_moves.is_some(),
            after.link_moves.is_some()
        ));
    }
    if changes.is_empty() {
        changes.push("a dressup value this reader does not name".to_owned());
    }
    changes
}

/// Convert a TOML toolpath section into a session `ToolpathConfig`.
fn toolpath_config_from_section(
    tp: &ProjectToolpathSection,
    tp_id: crate::ids::ToolpathId,
    operation: &OperationConfig,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> ToolpathConfig {
    // One-shot migration: projects saved before operation-specific dressup
    // restrictions shipped can have geometrically-invalid dressups (e.g.
    // entry_style=Ramp on a ProjectCurve with hundreds of small rings,
    // producing phantom diagonal cuts across the stock). Normalize on load
    // so the UI state and compute behaviour stay in lockstep.
    //
    // L1: the registry policy still grows inside format 3, so this fires
    // on a file the operator saved before the rule shipped. It overwrites
    // a stored value, so it reports through the same warning channel as
    // `parse_tool_type` — the log alone reaches nobody.
    let mut dressups = tp.dressups.clone();
    let op_type = operation.op_type();
    if dressups.normalize_for_op(op_type) {
        tracing::info!(
            toolpath = %tp.name,
            op = ?op_type,
            "Normalized incompatible dressups on load"
        );
        warnings.push(ProjectLoadWarning::DressupsNormalized {
            toolpath: tp.name.clone(),
            op: format!("{op_type:?}"),
            changes: dressup_changes(&tp.dressups, &dressups),
        });
    }
    // W1b (operator ruling 2026-10-28): the Waterline dial was the key
    // `continuous`, which no generator read before D69. A stored value is
    // the old default, so the op loads helical. The warning names the
    // change; a save writes `helical` and drops the old key.
    let mut operation = operation.clone();
    if let OperationConfig::Waterline(cfg) = &mut operation
        && let Some(old) = cfg.legacy_continuous.take()
    {
        cfg.helical = true;
        warnings.push(ProjectLoadWarning::WaterlineHelical {
            toolpath: tp.name.clone(),
            stored: old,
        });
    }
    ToolpathConfig {
        id: tp_id,
        name: tp.name.clone(),
        enabled: tp.enabled,
        operation,
        dressups,
        heights: tp.heights.clone(),
        tool_id: tp.tool_id,
        model_id: tp.model_id,
        pre_gcode: tp.pre_gcode.clone(),
        post_gcode: tp.post_gcode.clone(),
        boundary: tp.boundary.clone(),
        boundary_inherit: tp.boundary_inherit,
        stock_source: tp.stock_source,
        coolant: tp.coolant,
        debug_options: tp.debug_options,
        feeds_provenance: tp.feeds_provenance.clone(),
        rest_analysis: tp.rest_analysis.clone(),
        planner_origin: tp.planner_origin.clone(),
        shoe: tp.shoe,
        feeds_lock: tp.feeds_lock,
    }
}

/// Read the setup datum off a TOML setup section (W9 / P-2).
///
/// An **absent or empty** key keeps the type default: the writer leaves
/// the keys out for an untouched datum. F015 (operator ruling 1): a token
/// the vocabulary does not hold refuses the load and names the setup and
/// the token. An unknown `z_datum` never falls back to the stock top.
fn datum_from_section(section: &ProjectSetupSection) -> Result<DatumConfig, SessionError> {
    let unknown = |key: &'static str, token: &str| SessionError::UnknownSetupDatum {
        setup: section.name.clone(),
        key,
        token: token.to_owned(),
    };
    let xy_method = if section.xy_datum.is_empty() {
        XYDatum::default()
    } else {
        XYDatum::from_key(&section.xy_datum)
            .ok_or_else(|| unknown("xy_datum", &section.xy_datum))?
    };
    let z_method = if section.z_datum.is_empty() {
        ZDatum::default()
    } else {
        ZDatum::from_key(&section.z_datum).ok_or_else(|| unknown("z_datum", &section.z_datum))?
    };
    Ok(DatumConfig {
        xy_method,
        z_method,
        notes: section.datum_notes.clone(),
    })
}

/// F053: refuse two entries of one section kind with the same id. An
/// entry with no id takes its index, so an index can also collide with an
/// explicit id; that refuses too.
fn refuse_duplicate_ids(
    section: &'static str,
    ids: impl IntoIterator<Item = usize>,
) -> Result<(), SessionError> {
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(SessionError::DuplicateId { section, id });
        }
    }
    Ok(())
}

/// Reject TOML that parses but contains no recognizable CAM content. Without
/// this guard, `serde(default)` on every top-level field happily produces an
/// empty session from any well-formed TOML file (or a CSV opened by mistake),
/// and the user just sees an empty workspace with no error.
pub(super) fn validate_looks_like_cam_project(
    project: &ProjectFile,
    path: Option<&Path>,
) -> Result<(), SessionError> {
    // `[job]` field has #[serde(default)], so an absent table yields
    // ProjectJobSection::default() with an empty name; an empty `[job]` table
    // yields the field-level default (`default_job_name()`). Treat both as
    // "user did not supply a job name".
    let job_is_default = project.job.name.is_empty() || project.job.name == default_job_name();
    let nothing_loaded =
        project.setups.is_empty() && project.models.is_empty() && project.tools.is_empty();
    if job_is_default && nothing_loaded {
        let detail = match path {
            Some(p) => format!(
                "file '{}' does not look like an rs_cam project (no [job], setups, models, or tools)",
                p.display()
            ),
            None => {
                "file does not look like an rs_cam project (no [job], setups, models, or tools)"
                    .to_owned()
            }
        };
        return Err(SessionError::NotACamProject(detail));
    }
    Ok(())
}

/// Build a [`ProjectSession`](super::ProjectSession) from a parsed [`ProjectFile`].
///
/// This is the core of `ProjectSession::from_project_file` but lives here so
/// that all TOML-to-session conversion logic is co-located.
pub(super) fn build_session_from_project(
    project: ProjectFile,
    base_dir: &Path,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> Result<super::ProjectSession, SessionError> {
    validate_looks_like_cam_project(&project, None)?;
    check_format_version(&project)?;

    let mut stock = stock_from_project(&project.job.stock);
    // W6b (F210, no-legacy ruling): the loader refuses a stock, a machine
    // or a head outside its domain. The stock check reads the file values,
    // before the auto-from-model refit below.
    stock
        .validate()
        .map_err(crate::compute::domain::DomainRefusal::Stock)?;
    project
        .job
        .machine
        .validate()
        .map_err(crate::compute::domain::DomainRefusal::Machine)?;
    project
        .job
        .machine
        .head
        .validate()
        .map_err(crate::compute::domain::DomainRefusal::Head)?;

    // Load tools
    let tools: Vec<ToolConfig> = project
        .tools
        .iter()
        .enumerate()
        .map(|(idx, ts)| tool_from_project_section(ts, idx, warnings))
        .collect();
    // F029: the loader refuses a tool outside its domain (no-legacy
    // ruling), so no stored tool can panic a cutter build later.
    for tool in &tools {
        super::validate_tool(tool)?;
    }
    refuse_duplicate_ids("tools", tools.iter().map(|t| t.id.raw()))?;
    refuse_duplicate_ids(
        "models",
        project
            .models
            .iter()
            .enumerate()
            .map(|(idx, m)| m.id.map_or(idx, ModelId::raw)),
    )?;
    refuse_duplicate_ids(
        "setups",
        project
            .setups
            .iter()
            .enumerate()
            .map(|(idx, s)| s.id.map_or(idx, SetupId::raw)),
    )?;

    // Load models
    let mut models = Vec::new();
    for (idx, model_section) in project.models.iter().enumerate() {
        let model_id = model_section.id.unwrap_or(ModelId::mint(idx));
        // G-MODELRELINK: the RESOLVED path, not the raw string — see
        // `resolve_model_path`. All four arms below (mesh, polygons, step,
        // and the error placeholder) share this binding, so a model that
        // failed to load still carries the path that was actually searched.
        let model_path = resolve_model_path(&model_section.path, base_dir);
        let model_kind = model_section
            .kind
            .or_else(|| crate::io::infer_kind_from_path(&model_path));
        let model_units = model_section.units;

        match load_model_geometry(model_section, base_dir) {
            Ok(geometry) => {
                match &geometry {
                    LoadedGeometry::Mesh(mesh) => tracing::info!(
                        name = %model_section.name,
                        tris = mesh.triangles.len(),
                        "Loaded mesh model"
                    ),
                    LoadedGeometry::Drawing(items, _) => tracing::info!(
                        name = %model_section.name,
                        items = items.len(),
                        "Loaded 2D model"
                    ),
                    LoadedGeometry::Enriched(enriched) => tracing::info!(
                        name = %model_section.name,
                        tris = enriched.mesh.triangles.len(),
                        faces = enriched.face_count(),
                        "Loaded enriched (BREP) model"
                    ),
                }
                // C13: one builder, so the project door keeps the
                // `winding_report` the import door has always reported.
                models.push(
                    crate::io::model_from_geometry(
                        geometry,
                        model_id,
                        model_section.name.clone(),
                        &model_path,
                        model_kind,
                        model_units,
                        model_section.import,
                    )
                    .with_drawing_edits(model_section.drawing_edits.clone()),
                );
            }
            Err(e) => {
                tracing::warn!(
                    name = %model_section.name,
                    error = %e,
                    "Failed to load model, skipping"
                );
                warnings.push(ProjectLoadWarning::ModelLoadFailed {
                    name: model_section.name.clone(),
                    path: model_path.clone(),
                    detail: e.to_string(),
                });
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: None,
                    items: Arc::new(crate::selection::ItemSet::default()),
                    path: model_path,
                    kind: model_kind,
                    units: model_units,
                    enriched_mesh: None,
                    winding_report: None,
                    load_error: Some(e.to_string()),
                    import_options: model_section.import,
                    import_report: None,
                    // P4.9: the ledger round-trips; no item exists to
                    // replay it on.
                    drawing: crate::selection::DrawingLedger::unreplayed(
                        model_section.drawing_edits.clone(),
                    ),
                });
            }
        }
    }

    // F-026 (2026-05-25): when `stock.auto_from_model = true`, the stock
    // dimensions on disk may be stale relative to the current model
    // bbox — the user edited a model externally, or the saved file
    // pre-dates a mesh edit. The MCP `import_model` path already re-
    // runs `update_from_bbox` via `add_model`, but the load path was
    // using the raw TOML values unchanged. For `auto_from_model` projects
    // where the model has grown above `stock.z` (or shifted in XY), this
    // produced a stock bbox that didn't enclose the model — the
    // adaptive3d planner warned about it ("stock_top_z is below mesh
    // top"), but the simulator's per-setup dexel grid then sat at the
    // stale Z height while the toolpath cuts ran in the model's frame.
    // 3D ops like adaptive3d / scallop walk the whole model surface,
    // and Cut moves into uncleared cells produced full-stock-height
    // pre-stamp rays that the simulator interpreted as 25-50 mm axial
    // engagement (single-stamp clearing of virgin stock instead of the
    // commanded depth_per_pass). Rapids cleared at adaptive3d's
    // expected safe_z then registered as collisions against the stale
    // grid Z range.
    //
    // Fix: at load time, if `auto_from_model = true` and any loaded
    // model has a finite bbox, re-derive stock from the union of all
    // model bboxes. F31: this is the one rule every model-set door
    // runs (`session::fit_auto_stock`). No result exists yet, so the
    // answer needs no drop.
    let _refit = crate::session::fit_auto_stock(&mut stock, &models);

    // Collect toolpaths from setups
    let mut setups = Vec::new();
    let mut toolpath_configs = Vec::new();

    for (setup_idx, setup_section) in project.setups.iter().enumerate() {
        let setup_id = setup_section.id.unwrap_or(SetupId::mint(setup_idx));
        let face_up = parse_face_up(&setup_section.face_up, &setup_section.name, warnings);
        let z_rotation = parse_z_rotation(&setup_section.z_rotation, &setup_section.name, warnings);
        let mut tp_indices = Vec::new();

        for tp_section in &setup_section.toolpaths {
            let tp_idx = toolpath_configs.len();
            let tp_id = tp_section
                .id
                .unwrap_or(crate::ids::ToolpathId::mint(tp_idx));
            // A toolpath with no `[setups.toolpaths.operation]` table has no
            // parameters. The loader refuses it: a default operation would
            // cut with values the operator never set.
            let Some(operation) = tp_section.operation.clone() else {
                return Err(SessionError::MissingOperation {
                    setup: setup_section.name.clone(),
                    toolpath: tp_section.name.clone(),
                });
            };
            let config = toolpath_config_from_section(tp_section, tp_id, &operation, warnings);
            // W6b (F030, F036): the loader refuses an op number or a shoe
            // gap outside its domain (no-legacy ruling).
            crate::compute::domain::validate_toolpath(&config)?;
            toolpath_configs.push(config);
            tp_indices.push(tp_idx);
        }

        setups.push(SetupData {
            id: setup_id,
            name: setup_section.name.clone(),
            face_up,
            z_rotation,
            datum: datum_from_section(setup_section)?,
            model_ids: setup_section.model_ids.clone(),
            obstacles: setup_section.obstacles.clone(),
            stock_changes: setup_section.stock_changes.clone(),
            toolpath_indices: tp_indices,
            pause_message: setup_section.pause_message.clone(),
        });
    }

    refuse_duplicate_ids("toolpaths", toolpath_configs.iter().map(|tc| tc.id.raw()))?;

    // A toolpath that names a tool or a model the file does not define
    // cannot generate. The GUI used to re-derive these two sentences from
    // the loaded session; the loader knows the same fact first.
    for tc in &toolpath_configs {
        // R2: a toolpath with no tool names no tool; it loads as is.
        if let Some(tool_id) = tc.tool_id
            && !tools.iter().any(|t| t.id == tool_id)
        {
            warnings.push(ProjectLoadWarning::MissingToolReference {
                toolpath: tc.name.clone(),
                tool_id,
            });
        }
        if !models.iter().any(|m| m.id == tc.model_id) {
            warnings.push(ProjectLoadWarning::MissingModelReference {
                toolpath: tc.name.clone(),
                model_id: tc.model_id,
            });
        }
    }

    // S3: a stock change whose model is gone, or holds the wrong kind of
    // geometry, cannot apply. The command doors refuse it; a file can still
    // hold one, so the loader says so before the simulation refuses it.
    for setup in &setups {
        for change in setup.stock_changes.iter().filter(|c| c.enabled) {
            if let Err(refusal) =
                crate::compute::stock_change::validate_stock_change(change, |id| {
                    super::mutation::loaded_model_facts(&models, id)
                })
            {
                warnings.push(ProjectLoadWarning::StockChangeRefused {
                    setup: setup.name.clone(),
                    change: change.name.clone(),
                    reason: refusal.to_string(),
                });
            }
        }
    }

    // CMP-27: the pin-keying judgement, on the CORE load path. It ran on
    // the GUI's load path only, so a headless load and MCP `load_project`
    // heard nothing about a pin pair that cannot register the flip it is
    // for. Each setup is judged by its full map (`face_up` and
    // `z_rotation`); the findings come deduped by exact message.
    for finding in stock.pin_findings_for_setups(setups.iter().map(|s| (s.face_up, s.z_rotation))) {
        tracing::warn!("{}", finding.text);
        warnings.push(ProjectLoadWarning::AlignmentPinsUnkeyed {
            detail: finding.text,
        });
    }

    // Compute next IDs by scanning existing maximums
    let next_toolpath_id = toolpath_configs
        .iter()
        .map(|tc| tc.id.raw())
        .max()
        .map_or(0, |m| m + 1);
    let next_tool_id = tools.iter().map(|t| t.id.raw()).max().map_or(0, |m| m + 1);
    let next_setup_id = setups.iter().map(|s| s.id.raw()).max().map_or(0, |m| m + 1);
    let next_model_id = models.iter().map(|m| m.id.raw()).max().map_or(0, |m| m + 1);

    // Machines use snapshot semantics (like `[[tools]]`): the inline
    // `[job.machine]` copy is authoritative. L6 deleted the pre-snapshot
    // `machine_ref` link; the loader refuses the key (`RETIRED_KEYS`).

    let mut session = super::ProjectSession {
        name: project.job.name.clone(),
        stock,
        post: project.job.post,
        machine: project.job.machine,
        models,
        tools,
        setups,
        toolpath_configs,
        results: std::collections::HashMap::new(),
        rest_fresh_comparisons: std::collections::HashMap::new(),
        toolpath_revision: std::collections::HashMap::new(),
        next_revision: 0,
        run_record: super::run_state::RunRecord::default(),
        simulation_epoch: 0,
        simulation_runs: 0,
        simulation_drop_epochs: [0; super::SimulationDropCause::COUNT],
        simulation: None,
        simulation_resolution: project.job.simulation.resolution(),
        load_report_memo: super::load_report::LoadReportMemo::default(),
        selection_status_memo: crate::session::selection_status::SelectionStatusMemo::default(),
        next_toolpath_id,
        next_tool_id,
        next_setup_id,
        next_model_id,
    };
    normalise_rest_analysis_demand(&mut session, warnings);
    Ok(session)
}

/// Make the rest-analysis flag say what the project actually demands.
///
/// The analysis is demand-driven: a toolpath runs its rest-depth pass
/// because another toolpath consumes the regions as a machining boundary.
/// `auto_enable_rest_analysis_for_source` keeps that true for every EDIT,
/// and nothing ran it on LOAD, so the loader copied the stored flag
/// verbatim. A file saved before the hook shipped therefore carried both
/// errors: a producer switched on with no consumer, which pays for a rest
/// field solve at every generation that nothing reads, and a producer
/// switched off with a consumer, which refuses the consumer's boundary.
///
/// The rule is one line: a toolpath's rest analysis is on when at least
/// one enabled `DerivedRestRegions` boundary names it.
///
/// A pencil operation with the `RestDepth` detector is the one exception.
/// It attaches its own regions, so its flag answers a different question
/// and the loader leaves it as the file stored it. That is the exception
/// the producer hook already makes.
///
/// No legacy shim: the operator ruling of 2026-09-16 takes the break. A
/// file loads, the loader corrects it, and the warning tells the operator
/// to save.
fn normalise_rest_analysis_demand(
    session: &mut super::ProjectSession,
    warnings: &mut Vec<ProjectLoadWarning>,
) {
    let wanted: Vec<(usize, bool)> = session
        .toolpath_configs
        .iter()
        .enumerate()
        .filter_map(|(index, tc)| {
            if matches!(&tc.operation, OperationConfig::Pencil(cfg)
                if cfg.detector == crate::finish::pencil::PencilDetector::RestDepth)
            {
                return None;
            }
            let demand = !session.rest_region_consumers(tc.id).is_empty();
            (tc.rest_analysis.enabled != demand).then_some((index, demand))
        })
        .collect();

    for (index, demand) in wanted {
        let Some(tc) = session.toolpath_configs.get_mut(index) else {
            continue;
        };
        tc.rest_analysis.enabled = demand;
        let toolpath = tc.name.clone();
        tracing::info!(
            toolpath = %toolpath,
            enabled = demand,
            "Normalized the rest-analysis demand on load"
        );
        warnings.push(ProjectLoadWarning::RestAnalysisNormalized {
            toolpath,
            enabled: demand,
        });
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// Parity freeze (architectural refactor §7.2): the CORE project-file
    /// tool section serializes its tool type under the TOML key `type`
    /// (serde rename), holds it as a lenient **String** (unknown values
    /// are accepted here and resolved later by the lenient parser), and
    /// defaults a missing key to `"end_mill"`. The viz loader has a
    /// parallel `ProjectToolSection` with the same rename but a
    /// serde-direct `ToolType` — a registry/X-macro change must not
    /// silently alter either shape.
    #[test]
    fn project_tool_section_type_key_shape_frozen() {
        // `type` key round-trips into the String field.
        let section: ProjectToolSection =
            toml::from_str(r#"type = "ball_nose""#).expect("parse tool section");
        assert_eq!(section.tool_type, "ball_nose");

        // Lenient layer: unknown tool types are accepted as-is here
        // (resolution to ToolType happens later, warn-and-default).
        let weird: ProjectToolSection =
            toml::from_str(r#"type = "definitely_not_a_tool""#).expect("lenient parse");
        assert_eq!(weird.tool_type, "definitely_not_a_tool");

        // Missing key defaults to end_mill.
        let defaulted: ProjectToolSection = toml::from_str("").expect("parse empty section");
        assert_eq!(defaulted.tool_type, "end_mill");

        // Serialization emits `type`, never the field name `tool_type`.
        let out = toml::to_string(&section).expect("serialize tool section");
        assert!(
            out.contains("type = \"ball_nose\""),
            "expected renamed `type` key, got:\n{out}"
        );
        assert!(
            !out.contains("tool_type"),
            "field name `tool_type` must not leak into TOML:\n{out}"
        );
    }

    /// FC0 item 4 (decision D2): `dwell_time` left the Drill. The loader
    /// names the key; it does not load with no word (op tables are not
    /// `deny_unknown_fields`, F034). The same key on another op kind is not
    /// this row.
    #[test]
    fn a_drill_dwell_time_is_a_retired_key_fc0() {
        let text = r#"
            [[setups]]
            [[setups.toolpaths]]
            [setups.toolpaths.operation]
            kind = "drill"
            [setups.toolpaths.operation.params]
            cycle = "peck"
            dwell_time = 0.5
        "#;
        let raw: toml::Value = toml::from_str(text).expect("parse");
        let found = retired_op_entries(&raw);
        let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            ["setups[0].toolpaths[0].operation.params.dwell_time"],
            "the retired Drill dwell_time is named"
        );
        assert!(
            matches!(check_retired_keys(&raw), Err(SessionError::RetiredKeys(_))),
            "the loader refuses the key"
        );
    }
}
