//! MCP parameter struct definitions and small helpers shared with
//! `rs_cam_viz`'s embedded MCP server.
//!
//! Historically this crate also produced a standalone `rs_cam_mcp`
//! binary with its own `CamServer` dispatch implementation. The
//! standalone binary was retired in favour of the GUI-embedded server
//! (`rs_cam_viz --mcp`) — see `planning/CODEBASE_UNIFICATION_PLAN.md`
//! for context. Only the param structs and helper functions remain so
//! the embedded server can keep importing them.

use rmcp::schemars;
use serde::Deserialize;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolType;

// ── Parameter structs ─────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddSetupParam {
    /// Optional name for the new setup
    pub name: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetSetupFaceParam {
    /// Setup index (0-based)
    pub setup_index: usize,
    /// Face orientation: "top", "bottom", "front", "back", "left", "right"
    pub face_up: String,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct MoveToolpathToSetupParam {
    /// Toolpath index (0-based, global across all setups)
    pub toolpath_index: usize,
    /// Target setup index (0-based)
    pub target_setup_index: usize,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ImportModelParam {
    /// File path to import. Supported formats: .stl, .dxf, .svg, .step/.stp
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct LoadProjectParam {
    /// Path to the project TOML file
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct IndexParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GenerateToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Optional wait budget in seconds. If generation hasn't finished by
    /// then, the call returns a `status: "running"` response instead of
    /// blocking — the generate is NOT cancelled, it keeps running in the
    /// background. Omit (or pass `None`) to wait indefinitely, matching
    /// prior behavior. While it runs, `generation_status` reports the
    /// in-flight index / stage / elapsed live, `list_toolpaths` answers from
    /// a snapshot, and `cancel_generation` aborts it — all three are served
    /// off the GUI frame loop and answer within a second (A/M12).
    pub timeout_s: Option<u64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GenerateAllParam {
    /// Optional wait budget in seconds — see
    /// `GenerateToolpathParam::timeout_s`.
    pub timeout_s: Option<u64>,
    /// A/M11 — iterate to a fixpoint over the rest-machining chain: generate,
    /// simulate, regenerate whatever was blocked only on missing upstream
    /// stock, repeat until nothing new generates. Defaults to `true`.
    ///
    /// An operation whose stock source is "remaining stock" needs the
    /// *simulated* stock of the operations before it, and that snapshot only
    /// exists after a simulation — so it can never see stock produced earlier
    /// in the same pass. Without the loop, a chain of `k` such operations
    /// needs `k` manual sim/generate rounds and nothing tells you `k`. The
    /// reply reports how many rounds it actually took.
    ///
    /// Pass `false` for the old single-pass behaviour.
    pub fixpoint: Option<bool>,
    /// Cell size in mm for the simulations the fixpoint loop runs on your
    /// behalf.
    ///
    /// **Required** when the loop is on and the project contains any enabled
    /// rest-machining operation; the call refuses rather than guessing. A
    /// resolution is never a neutral default: collision counts and engagement
    /// both move with cell size, so a silently chosen one produces verdicts
    /// nobody asked for. Use the same value you intend for your verification
    /// simulation — well below the finishing tool's TIP radius (e.g. 0.1 for
    /// a 1 mm ball).
    pub simulation_resolution_mm: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct OperationSchemaParam {
    /// Operation type (e.g. "pocket", "adaptive3d", "rest")
    pub operation_type: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SimulationParam {
    /// Simulation resolution in mm (default 0.5)
    pub resolution: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ExportParam {
    /// Output file path for G-code
    pub path: String,
    /// Bypass the tool-load gate for criteria that returned `Unmodeled`
    /// (e.g. no simulation run, no vendor data). Default false.
    #[serde(default)]
    pub accept_unmodeled_tool_load: bool,
    /// Bypass the tool-load gate for criteria that returned `Exceeds`
    /// (the toolpath is predicted to break or burn the tool). Default
    /// false. Set this knowingly; it is the "I accept that this toolpath
    /// will damage tooling" override and is recorded separately from
    /// `accept_unmodeled_tool_load`.
    #[serde(default)]
    pub accept_exceeded_tool_load: bool,
    /// Tool-change handling for this export. One of: `"pause"` (manual
    /// `M5` + operator message + `M0` — the GRBL-family default),
    /// `"m6"` (native `M5` + `M6 T{n}` — what Fusion emits and what a
    /// gSender/BitSetter setup needs so each tool is probed), or
    /// `"suppress"` (replace each change with a bare `M0` pause).
    /// Omit to keep the project's current setting. This is the MCP
    /// equivalent of the export wizard's Tool Change dropdown. Note:
    /// for `"m6"` each tool must have a distinct tool number or the
    /// controller won't re-trigger the change.
    #[serde(default)]
    pub tool_change_mode: Option<String>,
    /// When true and the project has more than one setup, write one
    /// G-code file per setup instead of a single combined program. Each
    /// file is self-contained and carries a header comment naming the
    /// setup (with a FLIP + RE-ZERO reminder on setups after the first),
    /// so a two-sided job is run as `setup1` → flip & re-zero → `setup2`.
    /// Output files are named `<stem>_<n>_<setup name>.<ext>` next to
    /// `path`. Ignored for single-setup projects.
    #[serde(default)]
    pub split_setups: bool,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotSimParam {
    /// Output file path (.png for 6-view composite, .html for interactive 3D)
    pub path: String,
    /// Image width in pixels (default 1200, PNG only)
    pub width: Option<u32>,
    /// Image height in pixels (default 800, PNG only)
    pub height: Option<u32>,
    /// Checkpoint index to render (default: last). Each toolpath produces one
    /// checkpoint. Use a lower index to see intermediate states.
    pub checkpoint: Option<usize>,
    /// Include toolpath overlay lines (HTML only, default true)
    pub include_toolpaths: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Output file path (.png for 6-view composite, .html for interactive 3D)
    pub path: String,
    /// Image width in pixels (default 1200, PNG only)
    pub width: Option<u32>,
    /// Image height in pixels (default 800, PNG only)
    pub height: Option<u32>,
    /// Show machined stock as dimmed background context (default false, PNG only).
    /// Requires simulation to have been run first.
    pub show_stock: Option<bool>,
    /// Include rapid moves in the rendering (default true, PNG only)
    pub include_rapids: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotGuiParam {
    /// Output file path (must end in .png)
    pub path: String,
    /// Optional window width in logical points. When given, the window is
    /// resized before capture. The new size persists after the capture
    /// (it is not restored).
    pub width: Option<f32>,
    /// Optional window height in logical points. Same persistence note as
    /// `width`.
    pub height: Option<f32>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetUiViewParam {
    /// Workspace to switch to: "setup", "toolpaths", "simulation",
    /// or "readiness".
    pub workspace: Option<String>,
    /// Toolpath index (0-based) to select. Selecting a toolpath makes its
    /// properties panel visible in the Toolpaths workspace.
    pub toolpath_index: Option<usize>,
    /// Toolpath properties tab to activate: "geometry", "feeds",
    /// "linking", "heights", or "dressup". Takes effect the next time the
    /// properties panel renders a selected toolpath.
    pub properties_tab: Option<String>,
    /// Non-toolpath properties panel to select: "machine" (machine setup +
    /// kinematics + GRBL $$ import) or "stock". Switches to the Setup
    /// workspace so the panel is visible on the right.
    pub select: Option<String>,
    /// Modal to open: "feeds_modal", "optimize_modal", "export_wizard",
    /// "tool_library", or "none" to close all modals.
    pub modal: Option<String>,
}

/// GRBL `$$` settings dump to import onto the live machine profile.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ImportMachineSettingsParam {
    /// The full `$$` settings dump text (`$N=value` lines). Tolerates
    /// grblHAL `(description)` comments, CRLF, and unrelated `$N` lines.
    /// Maps `$11`→junction deviation, `$120/$121/$122`→per-axis accel,
    /// `$110/$111`→max feed (travel). Applying breaks any machine-library
    /// link since the values are now inline.
    pub dump: String,
}

/// Name of a machine in the per-user library to snapshot-import.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct LoadMachineFromLibraryParam {
    /// Library machine name (file stem) from `list_machine_library`. The
    /// machine is COPIED into the project (snapshot, no live link).
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathParamInput {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Parameter name (e.g. "feed_rate", "stepover", "depth_per_pass", "plunge_rate",
    /// or any config-specific field like "angle", "min_z", "passes")
    pub param: String,
    /// New value (numeric)
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct OptimizeToolpathInput {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathHeightsParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Clearance plane Z (absolute, in the operation's emission frame —
    /// for a Top setup the stock top is the high Z). Rapids travel at
    /// this height; raise it above the tallest uncut feature to avoid
    /// rapid collisions. Omit to leave unchanged (stays Auto/current).
    pub clearance_z: Option<f64>,
    /// Retract plane Z (absolute). Omit to leave unchanged.
    pub retract_z: Option<f64>,
    /// Feed (rapid-to-cut transition) plane Z (absolute). Omit to leave unchanged.
    pub feed_z: Option<f64>,
    /// Stock-top reference Z (absolute). Omit to leave unchanged.
    pub top_z: Option<f64>,
    /// Bottom / lowest-cut reference Z (absolute). Omit to leave unchanged.
    pub bottom_z: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolParamInput {
    /// Tool index (0-based)
    pub index: usize,
    /// Parameter name (e.g. "diameter", "flute_count", "stickout", "corner_radius",
    /// "cutting_length", "shaft_diameter", "shank_diameter", "shank_length", "holder_diameter")
    pub param: String,
    /// New value (numeric)
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct CollisionCheckParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct CutTraceParam {
    /// Optional: filter results to a single toolpath by its project-level
    /// **id** — NOT its index. Ids come from `list_toolpaths` (`id` field);
    /// the cut trace is keyed by id throughout. An id that matches no
    /// toolpath is refused with the list of valid ids rather than answered
    /// with an empty result. (Measured on a real project 2026-08-08: the
    /// values 4/5/6 were simultaneously valid *indices* and valid *ids of
    /// different toolpaths*, so this distinction is not academic.)
    pub toolpath_id: Option<usize>,
    /// Maximum hotspots to return (default: 20)
    pub max_hotspots: Option<usize>,
    /// Maximum issues to return (default: 50)
    pub max_issues: Option<usize>,
    /// Maximum `span_summaries` entries to return (default: 200). Order is
    /// toolpath index ascending, then span_id ascending, so a cap always
    /// takes the same leading entries. Narrow with `span_kind` /
    /// `pass_index` / `toolpath_id` rather than raising this.
    pub max_span_summaries: Option<usize>,
    /// Maximum `semantic_summaries` entries to return (default: 200). The
    /// array is ordered by `wasted_runtime_s` descending, so the first N
    /// are the N worst offenders.
    pub max_semantic_summaries: Option<usize>,
    /// Maximum `toolpath_summaries` entries to return (default: uncapped —
    /// one row per toolpath that produced samples).
    pub max_toolpath_summaries: Option<usize>,
    /// Maximum `drill_summaries` entries to return (default: uncapped —
    /// one row per drill toolpath).
    pub max_drill_summaries: Option<usize>,
    /// Maximum `drill_samples` entries to return when `include_drill_samples`
    /// is true (default: 500).
    pub max_drill_samples: Option<usize>,
    /// Optional: override the global response byte backstop (default:
    /// 8388608 = 8 MiB). Sections that do not fit are OMITTED and named in
    /// `sections_not_computed`, and `complete` becomes false — a section
    /// that did not fit is never rendered as an empty array or a zero.
    pub max_response_bytes: Option<usize>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// a span of this kind. Accepted values match `SpanKind`:
    /// "operation", "depth_pass", "region", "entry", "lead_out", "link_bridge",
    /// "dressup_artifact", "geometry_refit", "rapid_order_barrier".
    #[allow(dead_code)]
    pub span_kind: Option<String>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// this exact span id. SpanIds come from `inspect_spans`.
    #[allow(dead_code)]
    pub span_id: Option<u32>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// a `DepthPass` span with this `pass_index` payload value (0-based).
    #[allow(dead_code)]
    pub pass_index: Option<u32>,
    /// Optional: also include the per-peck `drill_samples` array in the
    /// response. Defaults to `false` because the stream can be verbose on
    /// large drill cycles; per-toolpath `drill_summaries` are always
    /// included regardless.
    #[allow(dead_code)]
    pub include_drill_samples: Option<bool>,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct InspectSpansParam {
    /// Toolpath index (0-based). Must have been generated first.
    pub index: usize,
    /// Optional `SpanKind` filter (snake_case). Accepted values:
    /// "operation", "depth_pass", "region", "entry", "lead_out",
    /// "link_bridge", "dressup_artifact", "geometry_refit",
    /// "rapid_order_barrier".
    pub kind: Option<String>,
    /// Optional parent span id (vec index). Restricts results to spans whose
    /// move range is contained within the parent's range. Pair with `kind` to
    /// drill from "Operation 0" → its DepthPasses → a single DepthPass's Regions.
    pub parent_id: Option<u32>,
    /// Optional `DepthPass` `pass_index` payload match (0-based).
    pub pass_index: Option<u32>,
    /// Optional `Region` `region_id` payload match.
    pub region_id: Option<u32>,
    /// Hard cap on returned spans (default 50). Result includes `truncated`
    /// and `total_matching` when capped.
    pub max_spans: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GenDebugTraceParam {
    /// Toolpath index (0-based). Must have been generated first.
    pub index: usize,
    /// Optional filter. Accepts EITHER a generation-debug kind string
    /// ("z_level", "adaptive_pass", "entry_search", "preflight", "pre_stamp",
    /// "widen_band", "waterline_cleanup") OR a structural `SpanKind` synonym
    /// in snake_case ("depth_pass" → matches z_level/adaptive_pass/z_level_clear,
    /// "entry" → matches entry_search). Omit to include all kinds. Each
    /// returned span carries a `span_kind_hint` field that maps the debug
    /// kind back to its structural SpanKind when one applies.
    pub span_kind: Option<String>,
    /// Optional filter: only include spans whose exit_reason contains this
    /// substring. Useful values for AgentSearch diagnosis:
    /// "loop closed", "idle", "no entry", "preflight skip", "no viable direction".
    pub exit_reason: Option<String>,
    /// Optional filter: only include spans where the `yield_ratio` counter is
    /// at most this value (e.g. 0.1 to see all low-yield passes). Spans without
    /// a `yield_ratio` counter are skipped when this filter is set.
    pub max_yield_ratio: Option<f64>,
    /// Maximum span count in the response (default: 100). Set to 0 for unlimited.
    pub max_spans: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolpathParam {
    /// Setup index (0-based) to add the toolpath into
    pub setup_index: usize,
    /// Operation type (e.g. "pocket", "adaptive3d", "drop_cutter", "profile")
    pub operation_type: String,
    /// Tool index (0-based) from list_tools
    pub tool_index: usize,
    /// Model ID (project-assigned `id` field — use the `id` value
    /// from `inspect_model` results, NOT a 0-based positional index;
    /// IDs typically start at 1 and increment per import).
    pub model_id: usize,
    /// Optional name for the toolpath
    pub name: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct RemoveToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolParam {
    /// Display name for the tool
    pub name: String,
    /// Tool type (e.g. "end_mill", "ball_nose", "bull_nose", "v_bit", "tapered_ball_nose")
    pub tool_type: String,
    /// Tool diameter in mm
    pub diameter: f64,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct RemoveToolParam {
    /// Tool index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ListToolCatalogParam {
    /// Catalog name (file stem) from `list_tool_library`, e.g.
    /// "end_mills", "v_bits", "tapered_ball".
    pub catalog: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolFromLibraryParam {
    /// Catalog name (file stem) as returned by `list_tool_library`,
    /// e.g. "end_mills", "v_bits", "tapered_ball".
    pub catalog: String,
    /// 0-based index of the tool within that catalog — use the `index`
    /// field from `list_tool_library` output.
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetStockConfigParam {
    /// Stock width (X) in mm
    pub x: f64,
    /// Stock depth (Y) in mm
    pub y: f64,
    /// Stock height (Z) in mm
    pub z: f64,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetBoundaryConfigParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Enable or disable boundary
    pub enabled: bool,
    /// Boundary source: "stock", "model_silhouette", or "derived_rest_regions"
    pub source: Option<String>,
    /// Containment mode: "center", "inside", or "outside"
    pub containment: Option<String>,
    /// Additional offset in mm (positive = expand, negative = shrink)
    pub offset: Option<f64>,
    /// Required when `source` is "derived_rest_regions": the stable id
    /// (from `get_toolpath_params`'s `id` field, not an index) of the
    /// toolpath whose pencil rest-depth result supplies the boundary.
    pub source_toolpath_id: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetRestAnalysisConfigParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Enable or disable rest analysis
    pub enabled: bool,
    /// Real library tool id whose geometry defines the rest reference.
    /// `None` = prefer the machined stock (when available), else a
    /// self-referenced bare-surface probe.
    pub reference_tool_id: Option<usize>,
    /// XY grid cell size (mm) for the rest field. Smaller = finer regions.
    pub cell_mm: Option<f64>,
    /// Rest-depth threshold (mm): a cell counts as REST material once the
    /// reference floats more than this above the true surface.
    pub min_valley_depth: Option<f64>,
    /// Extra clearance (mm) added around detected rest regions beyond this
    /// toolpath's own tool radius.
    pub region_margin_mm: Option<f64>,
    /// PR-7: offset stepover (mm) the ROUTING criterion assumes a downstream
    /// pencil fan would emit (`pencil` iff `reach <= cap * stepover`). Leave
    /// unset to size it from the canonical reach policy for this toolpath's
    /// own cutter — the correct choice unless you are modelling a specific
    /// downstream operation whose stepover is pinned.
    pub offset_stepover_mm: Option<f64>,
    /// PR-7: offset passes per side that fan is permitted (the `cap`).
    /// Unset = the detector's own default (0, centreline only).
    pub num_offset_passes: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetDressupConfigParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Dressup configuration as a JSON object (fields match DressupConfig)
    pub dressup: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetDressupFieldParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Dressup field name (e.g. "link_moves", "arc_fitting", "retract_strategy")
    pub key: String,
    /// New value for the field (JSON). Pass enum values as bare JSON
    /// strings — e.g. `"ramp"` (NOT `"\"ramp\""`); pass booleans as
    /// `true` / `false` or `0` / `1` (server coerces); pass numbers
    /// as JSON numbers OR numeric strings like `"7"` (server coerces
    /// when the existing field is numeric).
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathEnabledParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// `true` to enable, `false` to disable
    pub enabled: bool,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetStockSourceParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Either "fresh" or "from_remaining_stock"
    pub source: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetSpindleStrategyParam {
    /// Project-level spindle policy. Either "match_chart" (default —
    /// use vendor LUT row's chart-published RPM verbatim) or
    /// "max_speed" (push RPM up the constant-chipload line toward
    /// the spindle ceiling, scaling feed proportionally). Affects
    /// every Suggest call across the project. See
    /// `rs_cam_core::feeds::SpindleStrategy`.
    pub strategy: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SaveProjectParam {
    /// File path to save the project TOML to (required)
    pub path: String,
}

/// Model ID parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct ModelIdParam {
    /// Model ID as returned by `inspect_model` (the opaque DB-assigned
    /// `id` field). NOT a 0-based positional index — IDs typically start
    /// at 1 and are incremented per import. To find the right ID for a
    /// model, call `inspect_model` and use the `id` field from the
    /// response.
    pub model_id: usize,
}

/// Add alignment pin parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct AddAlignmentPinParam {
    /// X position of the alignment pin in mm
    pub x: f64,
    /// Y position of the alignment pin in mm
    pub y: f64,
    /// Diameter of the alignment pin in mm
    pub diameter: f64,
}

/// Remove alignment pin parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct RemoveAlignmentPinParam {
    /// Index of the alignment pin to remove (0-based)
    pub index: usize,
}

/// Simulation jump-to-move parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimJumpToMoveParam {
    /// Move index to jump to (0-based, up to total_moves)
    pub move_index: usize,
}

/// Per-toolpath percentage-based simulation scrub parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimScrubToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Position within this toolpath as percentage (0.0 = start, 100.0 = end)
    pub percent: f64,
}

/// Jump to the start or end of a specific toolpath in the simulation.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimJumpToToolpathBoundaryParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

/// Parse a string into an `OperationType` (snake_case).
pub fn parse_operation_type(s: &str) -> Result<OperationType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|e| format!("Unknown operation type '{s}' ({e}). Valid types: face, pocket, profile, adaptive, v_carve, rest, inlay, zigzag, trace, drill, chamfer, drop_cutter, adaptive3d, waterline, pencil, scallop, unified_finish, steep_shallow, ramp_finish, spiral_finish, radial_finish, horizontal_finish, project_curve, alignment_pin_drill"))
}

/// Parse a string into a `ToolType`.
///
/// Vocabulary is the unified core [`ToolType::parse_lenient`] (T8) —
/// canonical snake_case plus the historical loader aliases — so the
/// MCP surface can't drift from the project-file parsers. Unknown
/// input stays an explicit `Err` here (deliberate Q4 carve-out: this
/// feeds live mutations like `add_tool`, where an error beats silently
/// creating an end mill the caller didn't ask for).
pub fn parse_tool_type(s: &str) -> Result<ToolType, String> {
    ToolType::parse_lenient(s).ok_or_else(|| {
        format!(
            "Unknown tool type '{s}'. Valid types: end_mill, ball_nose, bull_nose, v_bit, tapered_ball_nose"
        )
    })
}

pub fn text(msg: impl Into<String>) -> String {
    msg.into()
}

// SAFETY: callers consistently pass owned `serde_json::Value` built inline
// via `serde_json::json!{...}` or `serde_json::to_value(...)`, so taking by
// value avoids forcing every caller to bind a temporary just to borrow it.
// Switching to `&Value` would cascade across ~30 call sites in rs_cam_viz
// without functional benefit. Tracked for a future refactor batch.
/// Serialise a tool response.
///
/// **Compact, not pretty** (Checkpoint L-4, 2026-08-08). Every one of the
/// ~68 tools goes through here, and every one of them used to ship its
/// indentation to the agent. Measured by wave B-1 on real responses:
/// pretty-printing cost **31.3 %** of a 56,225,225-byte `get_cut_trace`
/// payload (13,392,073 bytes of whitespace) and **46.6 %** of a
/// `get_diagnostics` / `run_simulation` response (79,135 B pretty vs
/// 42,295 B compact — the ratio is *worse* on the small, deeply nested
/// responses agents read most often).
///
/// Nothing consumes the formatting: every reader on the wire is a JSON
/// parser. The indentation was pure transport cost.
#[allow(clippy::needless_pass_by_value)]
pub fn json_str(data: serde_json::Value) -> String {
    serde_json::to_string(&data).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

/// Build-identification block embedded in `project_summary`.
///
/// Gives an agent a way to detect that the running MCP binary predates a
/// feature it expects. `git_sha` is `None` unless the build environment
/// sets `VERGEN_GIT_SHA`; `features` is a hand-curated list of capability
/// flags that consumers can probe.
pub fn build_info() -> serde_json::Value {
    serde_json::json!({
        "crate_version": env!("CARGO_PKG_VERSION"),
        "core_version": rs_cam_core::build_info::CORE_VERSION,
        // git short-sha (with -dirty suffix) of the rs_cam_core build,
        // captured by its build.rs. Authoritative "which commit is this".
        "git_desc": rs_cam_core::build_info::GIT_DESC,
        "build_timestamp": rs_cam_core::build_info::BUILD_TIMESTAMP,
        "git_sha": option_env!("VERGEN_GIT_SHA"),
        "features": [
            "stale_defaults",
            "drill_summaries",
            "drill_gates",
            "transit_span_doc",
            "air_cut_op_kind_aware",
            "plunge_stress_gate",
            "verdict_list",
            "operation_schema",
            "param_schema_hints",
            "param_schema_optional_nulls",
            "integer_param_coercion",
            "mutation_result_envelope",
            "valid_param_error_hints",
            "runtime_error_status_fields",
            "runtime_errors_in_diagnostics",
            // Adaptive clearing rework (2026-05-28). Probe these to
            // confirm the running binary has the new toolpaths.
            "contour_parallel_hybrid",
            "adaptive3d_hybrid",
            "helical_starter_pocket",
            "gradient_follow_narrow_strip",
            "spiral_cleanup_overlap",
            // Tool-library MCP surface (2026-05-29): list_tool_library +
            // add_tool_from_library. Probe to confirm agent-driven tool
            // selection from the user's catalogs is available.
            "tool_library_mcp",
            // GUI capture surface (2026-06-11): screenshot_gui (full-window
            // PNG capture) + set_ui_view (workspace/selection/tab/modal
            // navigation). Probe to confirm agent-driven UI inspection.
            "gui_screenshot",
            "set_ui_view",
        ],
    })
}

/// Standardized error response when no project is loaded.
pub fn no_project_error() -> String {
    json_str(serde_json::json!({"error": "No project loaded. Call load_project first."}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Parity freeze (architectural refactor §7.2): the `build_info`
    /// capability surface that agents probe. Removing a published
    /// feature flag or a top-level key breaks agents silently — this
    /// pins every flag shipped to date as REQUIRED (adding new flags is
    /// fine; this is a superset assertion, not exact-set).
    #[test]
    fn build_info_published_capability_flags_frozen() {
        let info = build_info();
        let obj = info.as_object().expect("build_info must be an object");
        for key in [
            "crate_version",
            "core_version",
            "git_desc",
            "build_timestamp",
            "git_sha",
            "features",
        ] {
            assert!(
                obj.contains_key(key),
                "build_info lost top-level key `{key}`"
            );
        }

        let features: Vec<&str> = obj
            .get("features")
            .and_then(|f| f.as_array())
            .expect("features must be an array")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        for flag in [
            "stale_defaults",
            "drill_summaries",
            "drill_gates",
            "transit_span_doc",
            "air_cut_op_kind_aware",
            "plunge_stress_gate",
            "verdict_list",
            "operation_schema",
            "param_schema_hints",
            "param_schema_optional_nulls",
            "integer_param_coercion",
            "mutation_result_envelope",
            "valid_param_error_hints",
            "runtime_error_status_fields",
            "runtime_errors_in_diagnostics",
            "contour_parallel_hybrid",
            "adaptive3d_hybrid",
            "helical_starter_pocket",
            "gradient_follow_narrow_strip",
            "spiral_cleanup_overlap",
            "tool_library_mcp",
            "gui_screenshot",
            "set_ui_view",
        ] {
            assert!(
                features.contains(&flag),
                "build_info dropped published feature flag `{flag}` — agents probe these"
            );
        }
    }

    /// Parity freeze (architectural refactor §7.2): the MCP parse
    /// helpers accept every canonical snake_case name — the same reprs
    /// pinned in core's `operation_type_serde_repr_pinned` /
    /// `tool_type_serde_repr_pinned`. A core serde rename that misses
    /// this surface fails here.
    #[test]
    fn mcp_parse_helpers_accept_all_canonical_names() {
        for &op_type in OperationType::ALL {
            let parsed = parse_operation_type(op_type.kind_str())
                .expect("canonical op name must parse on the MCP surface");
            assert_eq!(parsed, op_type);
        }
        for &tool_type in ToolType::ALL {
            let repr = serde_json::to_value(tool_type).expect("serialize tool type");
            let repr = repr.as_str().expect("tool type serializes to a string");
            let parsed =
                parse_tool_type(repr).expect("canonical tool name must parse on the MCP surface");
            assert_eq!(parsed, tool_type);
        }
        // Unknown names stay loud errors that list the valid vocabulary.
        let err = parse_operation_type("definitely_not_an_op").unwrap_err();
        assert!(
            err.contains("alignment_pin_drill"),
            "error must list valid types: {err}"
        );
        let err = parse_tool_type("definitely_not_a_tool").unwrap_err();
        assert!(
            err.contains("tapered_ball_nose"),
            "error must list valid types: {err}"
        );
        // T8: the unified lenient vocabulary reaches this surface too —
        // historical loader aliases parse instead of erroring.
        assert_eq!(parse_tool_type("ball"), Ok(ToolType::BallNose));
        assert_eq!(parse_tool_type("flat"), Ok(ToolType::EndMill));
    }

    /// GUI-capture surface (2026-06-11): `ScreenshotGuiParam` must accept
    /// path-only requests (no resize) and full path+size requests.
    #[test]
    fn screenshot_gui_param_deserializes() {
        let p: ScreenshotGuiParam =
            serde_json::from_value(serde_json::json!({"path": "/tmp/gui.png"})).unwrap();
        assert_eq!(p.path, "/tmp/gui.png");
        assert!(p.width.is_none());
        assert!(p.height.is_none());

        let p: ScreenshotGuiParam = serde_json::from_value(serde_json::json!({
            "path": "/tmp/gui.png", "width": 1600.0, "height": 1000.0
        }))
        .unwrap();
        assert_eq!(p.width, Some(1600.0));
        assert_eq!(p.height, Some(1000.0));
    }

    /// `SetUiViewParam` is all-optional — an empty object is a valid no-op
    /// request, and each field deserializes independently.
    #[test]
    fn set_ui_view_param_deserializes() {
        let p: SetUiViewParam = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(p.workspace.is_none());
        assert!(p.toolpath_index.is_none());
        assert!(p.properties_tab.is_none());
        assert!(p.modal.is_none());

        let p: SetUiViewParam = serde_json::from_value(serde_json::json!({
            "workspace": "toolpaths",
            "toolpath_index": 2,
            "properties_tab": "heights",
            "modal": "feeds_modal",
        }))
        .unwrap();
        assert_eq!(p.workspace.as_deref(), Some("toolpaths"));
        assert_eq!(p.toolpath_index, Some(2));
        assert_eq!(p.properties_tab.as_deref(), Some("heights"));
        assert_eq!(p.modal.as_deref(), Some("feeds_modal"));
    }
}
