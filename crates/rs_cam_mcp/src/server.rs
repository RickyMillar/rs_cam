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
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::ZRotation;
use rs_cam_core::feeds::WorkholdingRigidity;
use rs_cam_core::material::Material;

// ── Parameter structs ─────────────────────────────────────────────────

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddSetupParam {
    /// Optional name for the new setup
    pub name: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetSetupFaceParam {
    /// Setup index (0-based)
    pub setup_index: usize,
    /// Face orientation: "top", "bottom", "front", "back", "left", "right"
    pub face_up: String,
}

/// The four legal in-plane stock rotations, as a wire enum (CLI-09).
///
/// The wire used to carry a bare `String` here, so the published tool
/// schema said only `"type": "string"` and a client had to read the
/// doc comment to learn the four legal values. The viz server then
/// hand-parsed the string with `trim_end_matches("deg")`. This enum is
/// a thin mirror of [`ZRotation`], which cannot derive
/// `schemars::JsonSchema` itself because `rs_cam_core` does not depend
/// on schemars. The token of each variant is the token `ZRotation::
/// to_key` already stores in a project file.
/// `inline` keeps the four values IN the property that uses this type.
/// Without it schemars emits a `$ref` into `$defs`, and the published
/// tool schema carries no `$defs`, so the reference does not resolve for
/// a client — the very discovery problem CLI-09 is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, schemars::JsonSchema)]
#[schemars(inline)]
pub enum ZRotationParam {
    #[serde(rename = "0")]
    #[default]
    Deg0,
    #[serde(rename = "90")]
    Deg90,
    #[serde(rename = "180")]
    Deg180,
    #[serde(rename = "270")]
    Deg270,
}

impl From<ZRotationParam> for ZRotation {
    fn from(p: ZRotationParam) -> Self {
        match p {
            ZRotationParam::Deg0 => ZRotation::Deg0,
            ZRotationParam::Deg90 => ZRotation::Deg90,
            ZRotationParam::Deg180 => ZRotation::Deg180,
            ZRotationParam::Deg270 => ZRotation::Deg270,
        }
    }
}

impl std::fmt::Display for ZRotationParam {
    /// The wire token, so a message that echoes the request reads the
    /// same words the caller sent.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = match self {
            ZRotationParam::Deg0 => "0",
            ZRotationParam::Deg90 => "90",
            ZRotationParam::Deg180 => "180",
            ZRotationParam::Deg270 => "270",
        };
        f.write_str(token)
    }
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetSetupRotationParam {
    /// Setup index (0-based)
    pub setup_index: usize,
    /// In-plane rotation of the stock about Z for this setup. One of
    /// "0", "90", "180", "270" (degrees). Note a 90-degree rotation
    /// does not map a non-square stock onto itself — for a diagonal
    /// alignment-pin pair on a rectangular board, 180 is usually the
    /// physical flip.
    pub z_rotation: ZRotationParam,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct MoveToolpathToSetupParam {
    /// Toolpath index (0-based, global across all setups)
    pub toolpath_index: usize,
    /// Target setup index (0-based)
    pub target_setup_index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ImportModelParam {
    /// File path to import. Supported formats: .stl, .dxf, .svg, .step/.stp
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct LoadProjectParam {
    /// Path to the project TOML file
    pub path: String,
    /// Throw away the open project's unsaved changes and load anyway.
    /// Default false.
    ///
    /// Loading REPLACES the project in the GUI. When the open one has
    /// edits that are not on disk, the load is refused and the refusal
    /// says what would be lost — there is no dialog to ask, the way the
    /// GUI asks a human. Save it first with `save_project`, or set this
    /// knowingly.
    #[serde(default)]
    pub discard_unsaved: bool,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct IndexParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

/// P5 — the per-tool reach map, as numbers rather than pixels.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ReachMapParam {
    /// Toolpath index (0-based). The operation must be one a reach map
    /// speaks about: a mesh operation that is not a roughing pass.
    pub index: usize,
    /// Gap (mm) above which a spot counts as unreachable. Unset = the
    /// operation's own declared cusp / scallop height, else the core's
    /// 0.05 mm default. Overriding it re-walks the grid under a new memo
    /// key, so probe a few values freely but expect the first of each to
    /// cost a build.
    pub tolerance_mm: Option<f64>,
    /// Number of gap-histogram buckets in the reply (default 8, max 64).
    /// The reply never carries the raw cell grid — a board-sized map is
    /// over a hundred thousand cells.
    pub histogram_bins: Option<usize>,
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
    /// setup and its datum — `X0 Y0 = stock min corner`, identical in
    /// every setup file, plus that file's own Z zero relative to the
    /// up-facing stock surface — with a FLIP reminder on setups after
    /// the first, so a two-sided job is run as `setup1` → flip & re-zero
    /// Z (keeping the same XY zero) → `setup2`.
    /// Output files are named `<stem>_<n>_<setup name>.<ext>` next to
    /// `path`. Ignored for single-setup projects.
    #[serde(default)]
    pub split_setups: bool,
    /// Emit the PREVIOUS generation's geometry for any operation that was
    /// edited after it was generated, instead of refusing. Default false.
    ///
    /// What is being accepted: the file will cut the geometry from before
    /// the edit, which is NOT what the operation's parameters now
    /// describe. Regenerating the operation is the fix; this flag exists
    /// for the case where the operator knowingly wants the earlier
    /// program. It does NOT make a missing, failed or still-generating
    /// result exportable — there is no geometry to put in its place.
    #[serde(default)]
    pub accept_previous_geometry: bool,
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
    /// Shade the MODEL surface by the per-tool reach map behind the
    /// toolpath (default false, PNG only). Green = this tool forms the
    /// surface inside the operation's tolerance, red = it cannot reach,
    /// neutral = not measured (an underside, a wall, or the eroded rim
    /// band). This is the explicit form of the overlay on purpose: this
    /// call renders offscreen and inherits no viewport state, so the
    /// screenshot asks for the overlay rather than inheriting it. (The
    /// live GUI's own toggle is reachable since P6 —
    /// `set_ui_view(overlays: {"reach_map": true})` — but that switches
    /// the WINDOW's overlay, which only `screenshot_gui` captures.) The
    /// call refuses when the toolpath is not a finishing operation a reach
    /// map speaks about, and it takes a second or two on a cold map — the
    /// same memo `reach_map` fills.
    ///
    /// When this is on the SHADING is the subject: the model keeps its own
    /// colours undimmed and the cutting moves are drawn thin and dimmed over
    /// it. Before F4 (2026-09-08) the moves were full width and full colour
    /// over a background dimmed to 0.35, and on a 200 mm board with 17 959
    /// moves the top panel came back an opaque green mat — the shading only
    /// showed in the bottom panel, where the moves are edge-on. Ask for a
    /// plain `screenshot_toolpath` when the PATH is what you need to read.
    pub reach_overlay: Option<bool>,
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
    /// Viewport overlays to switch on or off, as `{"<id>": true|false}`.
    ///
    /// The ids are the rows of the GUI's Overlays panel — the same list the
    /// panel renders, so anything the operator can switch, an agent can:
    /// `grid`, `model`, `stock_box`, `stock_solid`, `origin_axes`, `datum`,
    /// `fixtures`, `keep_outs`, `alignment_pins`, `flip_axis`, `curves`,
    /// `orientation_gizmo`, `cutting_moves`, `rapids`, `entry_markers`,
    /// `height_planes`, `tool_profile_ghost`, `span_entry`,
    /// `span_lead_out`, `span_link_bridge`, `span_dressup`,
    /// `rest_heatmap`, `tier_map`, `reach_map`, `simulated_stock`,
    /// `collisions`, `tool_deflection`, `generator_steps`,
    /// `active_step_highlight`, and the colour choices
    /// `stock_colour_solid` / `stock_colour_deviation` /
    /// `stock_colour_by_height` and `move_colour_palette` /
    /// `move_colour_engagement` / `move_colour_advance_per_tooth`.
    ///
    /// Nothing is silently dropped. The reply reports every key under
    /// `overlays.applied` or `overlays.refused`, and a refusal carries the
    /// same reason string the panel prints beside the greyed row — for
    /// example `{"tier_map": "previewed on setup 2 — switch setup to see
    /// it"}`. Switching a colour choice OFF is refused (switch a sibling on
    /// instead), an unknown id is refused, and every id is refused in the
    /// Readiness workspace, which renders no viewport.
    ///
    /// One colour source per surface: enabling one clears the others on the
    /// model, the simulated stock or the move lines. Pair with
    /// `screenshot_gui` to photograph what was enabled.
    pub overlays: Option<std::collections::BTreeMap<String, bool>>,
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
    /// New value. **Any JSON type** — a number for scalar params, a
    /// string for enum-valued params, `true`/`false` for flags, and an
    /// ARRAY for list-valued params (e.g. a drill op's `holes`:
    /// `[[2.5, 2.5], [237.5, 247.5]]`). Pass the array itself, not a
    /// string containing one. See [`any_json_value_schema`] for why
    /// this field carries an explicit type list.
    #[schemars(schema_with = "any_json_value_schema")]
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

/// F3.1 — add a toolpath through the GUI's own add path.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolpathViaGuiParam {
    /// Operation type, same vocabulary `add_toolpath` accepts (e.g.
    /// "pocket", "scallop", "adaptive3d").
    pub operation_type: String,
    /// Setup index (0-based) to add into. The GUI takes the target setup
    /// from the CURRENT SELECTION, so passing this selects that setup
    /// first, exactly as clicking the setup would. Omit to add into
    /// whatever is selected now — which is what the operator's next click
    /// would do, and may not be setup 0.
    pub setup_index: Option<usize>,
}

/// F3.5 — read the toast stack.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GetNotificationsParam {
    /// Include entries that have aged past their TTL but have not been
    /// collected yet. Default `true`: a test asserting what the operator
    /// saw must not lose the evidence to a slow assertion. Pass `false`
    /// for only what is on screen right now.
    pub include_expired: Option<bool>,
    /// Return at most this many, newest first. Omit for all of them.
    pub limit: Option<usize>,
}

/// F3.7 — rebind a toolpath's cutter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathToolParam {
    /// Toolpath index (0-based) from `list_toolpaths`.
    pub index: usize,
    /// Project-assigned tool **id** — the `id` field of a `list_tools`
    /// row, NOT the 0-based position in that list. This differs from
    /// `add_toolpath`, which takes `tool_index`. The two numbers agree
    /// in a project that has never had a tool removed, so read the
    /// `tool` object in the reply to confirm which tool you bound.
    pub tool_id: usize,
}

/// F3.8 — rebind a toolpath's input model / geometry.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathModelParam {
    /// Toolpath index (0-based) from `list_toolpaths`.
    pub index: usize,
    /// Project-assigned model **id** — the `id` field of an
    /// `inspect_model` row, NOT a 0-based positional index. Same
    /// convention `add_toolpath`'s `model_id` uses.
    pub model_id: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolParamInput {
    /// Tool index (0-based)
    pub index: usize,
    /// Parameter name. The COMPLETE accepted set (anything else is
    /// refused): "diameter", "flute_count", "stickout", "corner_radius"
    /// (bull-nose corner), "cutting_length", "included_angle" (V-bit,
    /// degrees), "taper_half_angle" (tapered ball nose, degrees),
    /// "shaft_diameter", "shank_diameter", "shank_length",
    /// "holder_diameter".
    pub param: String,
    /// New value (numeric — `flute_count` must be a whole number).
    #[schemars(schema_with = "any_json_value_schema")]
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
    pub span_kind: Option<String>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// this exact span id. SpanIds come from `inspect_spans`.
    pub span_id: Option<u32>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// a `DepthPass` span with this `pass_index` payload value (0-based).
    pub pass_index: Option<u32>,
    /// Optional: also include the per-peck `drill_samples` array in the
    /// response. Defaults to `false` because the stream can be verbose on
    /// large drill cycles; per-toolpath `drill_summaries` are always
    /// included regardless.
    pub include_drill_samples: Option<bool>,
}

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

#[derive(Deserialize, schemars::JsonSchema, Default, Clone, Debug, PartialEq)]
pub struct AddToolParam {
    /// Display name for the tool
    pub name: String,
    /// Tool type (e.g. "end_mill", "ball_nose", "bull_nose", "v_bit", "tapered_ball_nose")
    pub tool_type: String,
    /// Tool diameter in mm. For `tapered_ball_nose` this is the BALL TIP
    /// diameter (2 x tip radius) — the cone base is `shaft_diameter`.
    pub diameter: f64,
    /// **Required for `v_bit`** — the full included angle in degrees
    /// (a "20 degree V-bit" is 20.0). Must be > 0 and < 180. There is
    /// no honest default: the angle IS the tool, so an omitted angle is
    /// refused rather than guessed.
    pub included_angle: Option<f64>,
    /// **Required for `tapered_ball_nose`** — the cone HALF angle in
    /// degrees (a "5.6 degree per side" taper is 5.6). Must be > 0 and
    /// < 90. Refused when omitted, for the same reason as
    /// `included_angle`.
    pub taper_half_angle: Option<f64>,
    /// **Required for `bull_nose`** — corner radius in mm. Must be > 0
    /// and <= diameter / 2 (a corner radius of exactly diameter/2 is a
    /// ball nose). Refused when omitted.
    pub corner_radius: Option<f64>,
    /// Flute count. Default 2.
    pub flute_count: Option<u32>,
    /// Usable cutting-edge length in mm. Default 25.0 — CHECK IT
    /// against your real tool: it caps depth-of-cut and drives the
    /// deflection model.
    pub cutting_length: Option<f64>,
    /// Cone-base / shaft diameter in mm — the cutting ENVELOPE for a
    /// `tapered_ball_nose` (`envelope_diameter()` takes the larger of
    /// this and `diameter`). Default 6.35.
    pub shaft_diameter: Option<f64>,
    /// Shank diameter in mm (collision + machine collet check).
    /// Default 6.35.
    pub shank_diameter: Option<f64>,
    /// Shank length in mm. Default 20.0.
    pub shank_length: Option<f64>,
    /// Tool stickout from the holder in mm — drives the deflection
    /// model. Default 45.0.
    pub stickout: Option<f64>,
    /// Holder diameter in mm (holder-collision check). Default 25.0.
    pub holder_diameter: Option<f64>,
    /// G-code tool number for M6 output. Omit to auto-allocate the
    /// next free number in the project (distinct numbers are what make
    /// an M6 tool change re-trigger — identical numbers silently
    /// collapse the changes).
    pub tool_number: Option<u32>,
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

#[derive(Deserialize, schemars::JsonSchema, Default, Clone, Debug, PartialEq)]
pub struct SetStockConfigParam {
    /// Stock width (X) in mm. Omit to leave unchanged.
    pub x: Option<f64>,
    /// Stock depth (Y) in mm. Omit to leave unchanged.
    pub y: Option<f64>,
    /// Stock height (Z) in mm. Omit to leave unchanged.
    pub z: Option<f64>,
    /// Stock origin X in mm (the stock spans `origin_x ..= origin_x + x`).
    /// Omit to leave unchanged.
    pub origin_x: Option<f64>,
    /// Stock origin Y in mm. Omit to leave unchanged.
    pub origin_y: Option<f64>,
    /// Stock origin Z in mm — the BOTTOM of the stock, so the stock top
    /// sits at `origin_z + z`. 2D operations cut at negative Z relative
    /// to the model plane, so a 2D job normally wants
    /// `origin_z = -z` (top at Z = 0). This decides the Z frame the
    /// whole job cuts in. Omit to leave unchanged.
    pub origin_z: Option<f64>,
    /// Stock material by name, e.g. "White Oak", "Baltic Birch Plywood",
    /// "MDF", "Acrylic", "Aluminum 6061-T6". Matched case- and
    /// punctuation-insensitively against the material catalog + wood
    /// species library; an unrecognised or ambiguous name is REFUSED
    /// with the candidate list rather than guessed. Load-bearing: every
    /// feed, chipload band and power estimate depends on it.
    pub material: Option<String>,
    /// Workholding rigidity for the feeds calculation: "low", "medium"
    /// or "high". Omit to leave unchanged.
    pub workholding_rigidity: Option<String>,
    /// Auto-fit the stock to the next imported model's bounding box.
    ///
    /// Setting any dimension or origin above CLEARS this flag
    /// automatically (see the tool description) — pass `true` here only
    /// if you deliberately want the next `import_model` to overwrite
    /// what you just set. Passing `false` alone turns auto-fit off
    /// without changing any number.
    pub auto_from_model: Option<bool>,
}

/// `set_machine_kinematics` — typed write path for the accel /
/// junction-deviation numbers that decide cycle time and the
/// parallel-vs-spiral strategy verdict.
#[derive(Deserialize, schemars::JsonSchema, Default, Clone, Debug, PartialEq)]
pub struct SetMachineKinematicsParam {
    /// X-axis acceleration limit in mm/s^2 (GRBL `$120`).
    pub acceleration_x_mm_s2: Option<f64>,
    /// Y-axis acceleration limit in mm/s^2 (GRBL `$121`).
    pub acceleration_y_mm_s2: Option<f64>,
    /// Z-axis acceleration limit in mm/s^2 (GRBL `$122`).
    pub acceleration_z_mm_s2: Option<f64>,
    /// Isotropic fallback acceleration in mm/s^2. Only used when the
    /// per-axis triple is absent; when all three axes are given this is
    /// set to their mean unless explicitly passed.
    pub acceleration_mm_s2: Option<f64>,
    /// X-axis maximum rate in mm/min (GRBL `$110`).
    pub max_rate_x_mm_min: Option<f64>,
    /// Y-axis maximum rate in mm/min (GRBL `$111`).
    pub max_rate_y_mm_min: Option<f64>,
    /// Z-axis maximum rate in mm/min (GRBL `$112`). On a typical router
    /// this is far below X/Y, so it is what throttles a Z-dominant move.
    pub max_rate_z_mm_min: Option<f64>,
    /// GRBL junction deviation `$11` in mm (stock GRBL default 0.010).
    pub junction_deviation_mm: Option<f64>,
    /// Optional hard cap on junction velocity in mm/min. Omit to leave
    /// unchanged.
    pub max_junction_velocity_mm_min: Option<f64>,
    /// Optional jerk limit in mm/s^3. Omit to leave unchanged.
    pub jerk_mm_s3: Option<f64>,
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

/// Phase O — the machine-readable trigger for the multi-tool island
/// finishing planner. Every dial is optional; unset means the campaign
/// default named in that field's doc.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct PlanMultitoolFinishingParam {
    /// Setup index (0-based) the emitted finishing chain belongs to.
    pub setup_index: usize,
    /// Model id the tier map is measured against. Unset = the project's
    /// only model; the call REFUSES rather than picking when there is
    /// more than one.
    pub model_id: Option<usize>,
    /// Library tool ids taking part in the ladder. Order is irrelevant —
    /// the planner sorts coarse to fine on tip-sphere (cusp) radius, never
    /// on envelope radius. Each becomes one emitted `unified_finish`
    /// operation carrying its tier's islands; tier 0 cuts fresh stock and
    /// every later tier takes the remaining stock of the one before it.
    pub tool_ids: Vec<usize>,
    /// Planning-grid cell size (mm) for the residual walk. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_CELL_MM` (0.4). Plan in the
    /// 0.3-0.6 band, NEVER 0.15 — the map is O(cells) in both time and
    /// memory, and 0.15 mm costs ~125 s per ladder tool on a 200 mm board.
    pub cell_mm: Option<f64>,
    /// Residual (mm) above which a cell is handed to a finer tier. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_TOLERANCE_MM` (0.05).
    pub tolerance_mm: Option<f64>,
    /// Grid padding (mm) beyond the finest tool's envelope. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_MARGIN_MM` (0.5).
    pub margin_mm: Option<f64>,
    /// Cusp height (mm) every tier is dialled to. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_CUSP_HEIGHT_MM` (0.03). It is one
    /// number for the whole ladder on purpose: an equal cusp is what makes a
    /// tier seam blend instead of printing a height step.
    pub cusp_height_mm: Option<f64>,
    /// Region coarseness. 1.0 = neutral; below 1 gives many small
    /// islands, above 1 a few large ones. Scales the derived merge radius
    /// and minimum island area together. Default 1.0.
    pub coarseness: Option<f64>,
    /// Seam-blend band (mm) each finer tier's islands are grown by, into
    /// the coarser tier's territory. Default 2.0. `0.0` turns it off.
    pub overlap_mm: Option<f64>,
    /// Per-tier island cap. When filtering leaves more islands than this,
    /// the merge radius is raised and the close re-run (bounded), and what
    /// merged is reported. Default 24.
    pub max_regions_per_tier: Option<usize>,
    /// When true, tier 0 SKIPS the fine tiers' owned islands instead of
    /// sweeping the whole board — the coarse tool leaves ground a finer
    /// tool will re-finish anyway. The seam still blends (fine tiers
    /// machine their islands plus the overlap band), but the fine tools
    /// then meet the ROUGHING pass's terraces inside their islands instead
    /// of a coarse-finished surface — higher load on small cutters,
    /// measurable by the load gates. Default false.
    pub coarse_skips_fine_islands: Option<bool>,
    /// When true, every emitted tier's SHALLOW band splits each region into
    /// monotone CELLS on that region's own raster lattice, and rotates that
    /// lattice to the region's PCA-minor axis where its elongation clears
    /// 3.0. The passes stay a raster and every cell shares one lattice:
    /// per-cell sweep directions, a cell visit order and contour-per-cell
    /// were each measured and are each SLOWER. Measured on the reference
    /// relief under a realistic machined-stock link ceiling: 1.155x across
    /// the top three shallow regions, 1.215x on the elongated one — rig
    /// figures to approach, not promises, and cell seams change the cusp
    /// pattern, so review the rendered surface. Default true since
    /// 2026-09-01 (C4 operator surface review passed).
    pub monotone_cell_decomposition: Option<bool>,
    /// Per-tier operation choice, LADDER order (coarse → fine):
    /// "unified_finish" | "scallop" | "iso_scallop". Unset or shorter than
    /// the ladder = unified_finish for the unnamed tiers (the historical
    /// planner). The tier's TERRITORY is identical whichever strategy cuts
    /// it — regions come from the tier map; this only picks the operation.
    /// Evidence for the scallop/iso options:
    /// planning/metrology_2026-09-02/FINDINGS.md §M7–M8.
    #[serde(default)]
    pub tier_strategies: Option<Vec<String>>,
}

/// Phase U — the look-before-emit twin of [`PlanMultitoolFinishingParam`].
/// Every dial is spelled and defaulted identically, so a preview and the
/// plan that follows it describe the same territory; the only extra field is
/// where to write the SVG.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct PreviewTierMapParam {
    /// Setup index (0-based) the ladder would be planned into.
    pub setup_index: usize,
    /// Model id the tier map is measured against. Unset = the project's
    /// only model; the call REFUSES rather than picking when there is
    /// more than one.
    pub model_id: Option<usize>,
    /// Library tool ids taking part in the ladder. Order is irrelevant —
    /// the planner sorts coarse to fine on tip-sphere (cusp) radius, never
    /// on envelope radius.
    pub tool_ids: Vec<usize>,
    /// Planning-grid cell size (mm) for the residual walk. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_CELL_MM` (0.4). Plan in the
    /// 0.3-0.6 band, NEVER 0.15 — the map is O(cells) in both time and
    /// memory, and 0.15 mm costs ~125 s per ladder tool on a 200 mm board.
    pub cell_mm: Option<f64>,
    /// Residual (mm) above which a cell is handed to a finer tier. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_TOLERANCE_MM` (0.05).
    pub tolerance_mm: Option<f64>,
    /// Grid padding (mm) beyond the finest tool's envelope. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_MARGIN_MM` (0.5).
    pub margin_mm: Option<f64>,
    /// Cusp height (mm) every tier would be dialled to. Unset =
    /// `rs_cam_core::session::DEFAULT_PLAN_CUSP_HEIGHT_MM` (0.03). Carried
    /// on the preview so the dial set matches the planner's exactly; it
    /// sizes the emitted stepover, not the tier territory.
    pub cusp_height_mm: Option<f64>,
    /// Region coarseness. 1.0 = neutral; below 1 gives many small
    /// islands, above 1 a few large ones. Scales the derived merge radius
    /// and minimum island area together. Default 1.0.
    pub coarseness: Option<f64>,
    /// Seam-blend band (mm) each finer tier's islands are grown by, into
    /// the coarser tier's territory. Default 2.0. `0.0` turns it off.
    pub overlap_mm: Option<f64>,
    /// Per-tier island cap. When filtering leaves more islands than this,
    /// the merge radius is raised and the close re-run (bounded), and what
    /// merged is reported. Default 24.
    pub max_regions_per_tier: Option<usize>,
    /// When true, tier 0 SKIPS the fine tiers' owned islands instead of
    /// sweeping the whole board — spelled identically to the planner's
    /// dial so one dial set drives both calls. The island preview itself
    /// does not change (the dial moves tier 0's boundary, not the island
    /// map), but the value is threaded through so the previewed spec IS
    /// the planned spec. Default false.
    pub coarse_skips_fine_islands: Option<bool>,
    /// When true, every emitted tier's shallow band splits into monotone
    /// cells — spelled identically to the planner's dial so one dial set
    /// drives both calls. The island preview itself does not change (the
    /// dial moves what each tier's shallow band EMITS, not the island map),
    /// but the value is threaded through so the previewed spec IS the
    /// planned spec. Default true since 2026-09-01 (C4 ruling).
    pub monotone_cell_decomposition: Option<bool>,
    /// Accepted for dial parity with `plan_multitool_finishing` and
    /// IGNORED: a tier's strategy picks its OPERATION, not its territory,
    /// so the island preview is identical whichever strategies you pass.
    #[serde(default)]
    pub tier_strategies: Option<Vec<String>>,
    /// Absolute path ending in `.svg` to write the island preview to.
    /// Unset = numbers only. The parent directory must already exist —
    /// the call refuses rather than creating one. Polygons only, so a
    /// 64-island board is tens of KB.
    pub svg_path: Option<String>,
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
    #[schemars(schema_with = "any_json_value_schema")]
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

/// `apply_feeds` — the agent-facing entry to the one application funnel
/// (Checkpoint I-5, 2026-08-12).
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ApplyFeedsParam {
    /// Toolpath index (0-based), as listed by `list_toolpaths`.
    pub index: usize,
    /// Which dimensions to write. **Say what you mean** — the scope is the
    /// whole safety vocabulary of this tool:
    ///
    /// - `"speeds"` — feed / plunge / RPM. "How fast". Does NOT change the
    ///   cut, so the geometry you simulated stays the geometry you cut.
    /// - `"cut_geometry"` — stepover / DOC. **CHANGES THE CUT**: the removed
    ///   material, the engagement, the runtime and every gate verdict move
    ///   with it. Re-simulate afterwards.
    /// - `"both"` — both halves in one transaction. Also changes the cut.
    ///
    /// Defaults to `"speeds"`, the conservative choice: an omitted scope must
    /// not silently rewrite geometry. Mirrors
    /// `rs_cam_core::feeds::suggest::ApplyScope`; there is deliberately no
    /// per-field scope.
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SaveProjectParam {
    /// File path to save the project TOML to (required)
    pub path: String,
}

/// Model ID parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
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
pub struct RemoveAlignmentPinParam {
    /// Index of the alignment pin to remove (0-based)
    pub index: usize,
}

/// Simulation jump-to-move parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SimJumpToMoveParam {
    /// Move index to jump to (0-based, up to total_moves)
    pub move_index: usize,
}

/// Per-toolpath percentage-based simulation scrub parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SimScrubToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Position within this toolpath as percentage (0.0 = start, 100.0 = end)
    pub percent: f64,
}

/// Jump to the start or end of a specific toolpath in the simulation.
#[derive(Deserialize, schemars::JsonSchema, Default)]
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
/// the canonical snake_case token of each type, case folded — so the
/// MCP surface can't drift from the project-file parsers. L9 deleted
/// the historical loader aliases, so this surface accepts canonical
/// names only. Unknown input stays an explicit `Err` here (deliberate
/// Q4 carve-out: this feeds live mutations like `add_tool`, where an
/// error beats silently creating an end mill the caller didn't ask
/// for).
pub fn parse_tool_type(s: &str) -> Result<ToolType, String> {
    ToolType::parse_lenient(s).ok_or_else(|| {
        format!(
            "Unknown tool type '{s}'. Valid types: end_mill, ball_nose, bull_nose, v_bit, tapered_ball_nose"
        )
    })
}

// ── Wire-value plumbing ───────────────────────────────────────────────

/// JSON-Schema for a tool argument that accepts *any* JSON value.
///
/// `serde_json::Value`'s own `JsonSchema` impl emits the bare schema
/// `true` — "anything goes", with no type information at all. Measured
/// on a live job 2026-08-19: a client handed the untyped `value`
/// argument of `set_toolpath_param` a nested array and the server
/// received the *string* `"[[2.5,2.5],[237.5,247.5]]"`. serde then
/// refused with `invalid type: string "...", expected a sequence`, and
/// the drill hole list had to be hand-written into the project TOML.
///
/// Enumerating the permitted JSON types tells the client that an array
/// is a legal argument shape, so it stops stringifying.
/// [`coerce_json_container_string`] is the second layer, for clients
/// that stringify anyway.
pub fn any_json_value_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let mut map = serde_json::Map::new();
    map.insert(
        "type".to_owned(),
        serde_json::json!(["number", "string", "boolean", "array", "object", "null"]),
    );
    schemars::Schema::from(map)
}

/// Recover an array/object argument that arrived as a JSON *string*.
///
/// Deliberately narrow: only strings whose first non-space byte is `[`
/// or `{` are parsed, and only an array or object result is accepted.
/// Scalars are left alone — `"climb"` must stay the string `"climb"`,
/// and numeric strings are already handled type-aware downstream by
/// `ProjectSession::set_toolpath_param` (which knows whether the target
/// field is an integer). A string that merely *starts* like a container
/// but does not parse is returned unchanged so the caller still sees
/// the original text in the error.
pub fn coerce_json_container_string(value: serde_json::Value) -> serde_json::Value {
    let parsed = match &value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if matches!(trimmed.as_bytes().first(), Some(b'[' | b'{')) {
                serde_json::from_str::<serde_json::Value>(trimmed).ok()
            } else {
                None
            }
        }
        _ => None,
    };
    match parsed {
        Some(p) if p.is_array() || p.is_object() => p,
        _ => value,
    }
}

// ── Stock config vocabulary ───────────────────────────────────────────

/// Parse a workholding-rigidity name. Unknown input is an explicit
/// error — this feeds the feeds calculation, so a silent fallback to
/// `Medium` would be a wrong number with no trace.
pub fn parse_workholding_rigidity(s: &str) -> Result<WorkholdingRigidity, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "low" => Ok(WorkholdingRigidity::Low),
        "medium" | "med" => Ok(WorkholdingRigidity::Medium),
        "high" => Ok(WorkholdingRigidity::High),
        other => Err(format!(
            "Unknown workholding rigidity '{other}'. Valid values: low, medium, high."
        )),
    }
}

/// Punctuation- and case-insensitive key for material-name matching.
fn material_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Resolve a material by name against the curated catalog plus the
/// ~148-entry wood species library (the same list the GUI picker walks).
///
/// Matching is exact-first on either the picker label or the material's
/// own `label()`, then a unique substring match in either direction. An
/// unrecognised name, or an ambiguous one, is REFUSED with candidates —
/// never resolved to a near-miss, because every feed in the project
/// depends on the answer.
pub fn resolve_material(name: &str) -> Result<Material, String> {
    let want = material_key(name);
    if want.is_empty() {
        return Err(
            "material must be a non-empty name, e.g. \"White Oak\" or \"Baltic Birch Plywood\"."
                .to_owned(),
        );
    }

    let mut near: Vec<(String, Material)> = Vec::new();
    for (_category, entries) in Material::materials_by_category() {
        for (picker_label, mat) in entries {
            let own_label = mat.label();
            let keys = [material_key(&own_label), material_key(picker_label)];
            if keys.contains(&want) {
                return Ok(mat.clone());
            }
            let touches = keys
                .iter()
                .any(|k| !k.is_empty() && (k.contains(&want) || want.contains(k.as_str())));
            if touches && !near.iter().any(|(l, _)| *l == own_label) {
                near.push((own_label, mat.clone()));
            }
        }
    }

    match near.len() {
        1 => near
            .into_iter()
            .next()
            .map(|(_, mat)| mat)
            .ok_or_else(|| "material lookup lost its single candidate".to_owned()),
        0 => Err(format!(
            "Unknown material '{name}'. Names come from the material catalog + wood species \
             library — e.g. \"White Oak\", \"Hard Maple\", \"Baltic Birch Plywood\", \"MDF\", \
             \"Acrylic\", \"Aluminum 6061-T6\". Nothing matched, so nothing was written."
        )),
        n => {
            let mut labels: Vec<String> = near.into_iter().map(|(l, _)| l).collect();
            labels.sort();
            labels.truncate(20);
            Err(format!(
                "Ambiguous material '{name}' — {n} candidates. Nothing was written. Did you mean \
                 one of: {}",
                labels.join(", ")
            ))
        }
    }
}

// ── Tool geometry ─────────────────────────────────────────────────────

/// A tool built from an [`AddToolParam`], plus the list of fields that
/// were filled from a default rather than supplied by the caller.
///
/// The `defaulted` list is reported straight back on the wire. The
/// defect it exists for (2026-08-19): `add_tool` filled type-agnostic
/// defaults silently, so a 20-degree V-bit became a 90-degree V-bit —
/// a different tool, with no signal anywhere that a number had been
/// invented. Six tools needed ~30 corrections in one session.
#[derive(Debug)]
pub struct BuiltTool {
    pub config: ToolConfig,
    pub defaulted: Vec<&'static str>,
}

/// Build a [`ToolConfig`] from an `add_tool` request.
///
/// Three rules:
///
/// 1. The geometry that *defines* the tool for its type — V-bit
///    included angle, tapered-ball half angle, bull-nose corner radius
///    — is REQUIRED. No default is honest there.
/// 2. Geometry belonging to another type is zeroed rather than left at
///    the struct default, so a flat end mill stops reporting a 2 mm
///    corner radius and a 90-degree point on the wire (`list_tools`
///    publishes `corner_radius_mm.max(corner_radius)` for every type).
///    Every consumer of those three fields dispatches on `tool_type`
///    first, so zeroing changes no geometry.
/// 3. Everything else keeps its documented default but is NAMED in
///    `defaulted`.
///
/// `tool_number` is not assigned here — the caller allocates it against
/// the project's existing tools (see the `add_tool` handler).
pub fn build_tool_config(spec: &AddToolParam) -> Result<BuiltTool, String> {
    let tool_type = parse_tool_type(&spec.tool_type)?;
    if !spec.diameter.is_finite() || spec.diameter <= 0.0 {
        return Err(format!(
            "diameter must be a positive number of mm (got {}).",
            spec.diameter
        ));
    }

    let mut config = ToolConfig::new_default(ToolId(0), tool_type);
    config.name = spec.name.clone();
    config.diameter = spec.diameter;

    // Rule 2 — clear the geometry this type does not own.
    config.corner_radius = 0.0;
    config.included_angle = 0.0;
    config.taper_half_angle = 0.0;

    // Rule 1 — the defining geometry, per type.
    match tool_type {
        ToolType::VBit => {
            let angle = spec.included_angle.ok_or_else(|| {
                "v_bit requires `included_angle` (full included angle in degrees, e.g. 20 for a \
                 20-degree V-bit). No tool was added: the angle IS the tool, so guessing one \
                 would create a different cutter than the one you asked for."
                    .to_owned()
            })?;
            if !angle.is_finite() || angle <= 0.0 || angle >= 180.0 {
                return Err(format!(
                    "included_angle must be > 0 and < 180 degrees (got {angle})."
                ));
            }
            config.included_angle = angle;
        }
        ToolType::TaperedBallNose => {
            let angle = spec.taper_half_angle.ok_or_else(|| {
                "tapered_ball_nose requires `taper_half_angle` (cone HALF angle in degrees, e.g. \
                 5.6). No tool was added: guessing the taper would create a different cutter."
                    .to_owned()
            })?;
            if !angle.is_finite() || angle <= 0.0 || angle >= 90.0 {
                return Err(format!(
                    "taper_half_angle must be > 0 and < 90 degrees (got {angle})."
                ));
            }
            config.taper_half_angle = angle;
        }
        ToolType::BullNose => {
            let radius = spec.corner_radius.ok_or_else(|| {
                "bull_nose requires `corner_radius` (mm). No tool was added: the corner radius \
                 IS the difference between a bull nose, a flat end mill and a ball nose."
                    .to_owned()
            })?;
            if !radius.is_finite() || radius <= 0.0 || radius > spec.diameter / 2.0 {
                return Err(format!(
                    "corner_radius must be > 0 and <= diameter/2 ({:.4} mm) (got {radius}).",
                    spec.diameter / 2.0
                ));
            }
            config.corner_radius = radius;
        }
        ToolType::EndMill | ToolType::BallNose => {}
    }

    // Rule 3 — everything else: honour the override, name the default.
    let mut defaulted: Vec<&'static str> = Vec::new();

    macro_rules! apply_f64 {
        ($field:ident, $name:literal) => {
            match spec.$field {
                Some(v) if v.is_finite() && v > 0.0 => config.$field = v,
                Some(v) => {
                    return Err(format!(
                        "{} must be a positive number of mm (got {}).",
                        $name, v
                    ));
                }
                None => defaulted.push($name),
            }
        };
    }

    apply_f64!(cutting_length, "cutting_length");
    apply_f64!(shaft_diameter, "shaft_diameter");
    apply_f64!(shank_diameter, "shank_diameter");
    apply_f64!(shank_length, "shank_length");
    apply_f64!(stickout, "stickout");
    apply_f64!(holder_diameter, "holder_diameter");

    match spec.flute_count {
        Some(0) => return Err("flute_count must be at least 1.".to_owned()),
        Some(n) => config.flute_count = n,
        None => defaulted.push("flute_count"),
    }

    Ok(BuiltTool { config, defaulted })
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
        "core_version": rs_cam_core::util::build_info::CORE_VERSION,
        // git short-sha (with -dirty suffix) of the rs_cam_core build,
        // captured by its build.rs. Authoritative "which commit is this".
        "git_desc": rs_cam_core::util::build_info::GIT_DESC,
        "build_timestamp": rs_cam_core::util::build_info::BUILD_TIMESTAMP,
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
            // TD3 B-5 (G-RESULTS): `get_diagnostics`' per-toolpath rows are
            // the core `ToolpathDiagnostic` — `op_kind`, per-toolpath
            // collision counts, and the report-only generation-finding
            // areas — not a GUI-local subset. Probe this before assuming a
            // missing `truncated_core_mm2` means "not measured": on a
            // binary without the flag it means "not published".
            "diagnostics_row_core_parity",
            // C2 follow-up 2 (2026-08-30): the per-toolpath row carries
            // `monotone_cells` — the whole `MonotoneCellTotals` object or
            // `null`. Same precedent as the flag above: on a binary WITHOUT
            // this flag an absent `monotone_cells` means "not published",
            // not "the decomposition did not run". Read the fallback counts
            // (`membership_fallbacks`, `empty_fallbacks`) before concluding
            // a dial-on shallow band was decomposed everywhere.
            "diagnostics_row_monotone_cells",
            // MCP authoring surface (2026-08-21), closing the gaps the
            // 2026-08-19 from-scratch run hit. Probe these before
            // assuming a missing argument means "not supported":
            // - `set_toolpath_param` / `set_tool_param` / `set_dressup_field`
            //   declare a typed `value`, so ARRAYS survive the wire.
            "typed_param_value",
            // - `set_stock_config` carries origin, material and
            //   workholding rigidity, and clears `auto_from_model` when
            //   geometry is set explicitly.
            "stock_config_origin_material",
            // - `add_tool` takes per-type geometry, REFUSES the
            //   type-defining angle/radius when it is missing, and
            //   allocates distinct tool numbers.
            "add_tool_type_aware",
            // - `set_machine_kinematics` writes accel + junction
            //   deviation without a GRBL `$$` dump.
            "set_machine_kinematics",
            // - `set_setup_rotation` writes a setup's in-plane Z
            //   rotation (0/90/180/270).
            "set_setup_rotation",
            // Phase O (2026-08-27): `plan_multitool_finishing` emits a
            // coarse->fine `unified_finish` chain with `planner_origin`
            // provenance, and GUI Generate All runs the same rest-stock
            // fixpoint ladder the MCP tool does.
            "multitool_finishing_planner",
            // Phase U (2026-08-27): `preview_tier_map` answers "what
            // territory would each tool get?" WITHOUT emitting or
            // generating anything — compact SVG + numeric rows, never an
            // HTML dump (the interactive path measured 948 MB on the
            // reference board).
            "preview_tier_map",
            // P5 (2026-09-08): `reach_map` answers "does this tool's tip
            // fit into the valleys?" for one finishing toolpath — an
            // area-weighted unreachable percentage, the worst gap, and a
            // gap histogram. `screenshot_toolpath` takes the same map as a
            // `reach_overlay` shading on the model surface.
            "reach_map",
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
            "multitool_finishing_planner",
            "preview_tier_map",
            "reach_map",
        ] {
            assert!(
                features.contains(&flag),
                "build_info dropped published feature flag `{flag}` — agents probe these"
            );
        }
    }

    /// Phase O — the planner trigger's wire contract. Only `setup_index`
    /// and `tool_ids` are required; every dial deserialises to `None`,
    /// which is what lets "unset" mean the campaign default instead of a
    /// zero the caller never asked for.
    #[test]
    fn plan_multitool_finishing_param_round_trips_with_dials_omitted() {
        let minimal: PlanMultitoolFinishingParam = serde_json::from_value(serde_json::json!({
            "setup_index": 0,
            "tool_ids": [3, 7, 11],
        }))
        .expect("setup_index + tool_ids alone must deserialise");
        assert_eq!(minimal.setup_index, 0);
        assert_eq!(minimal.tool_ids, vec![3, 7, 11]);
        assert!(minimal.model_id.is_none());
        for (name, unset) in [
            ("cell_mm", minimal.cell_mm.is_none()),
            ("tolerance_mm", minimal.tolerance_mm.is_none()),
            ("margin_mm", minimal.margin_mm.is_none()),
            ("cusp_height_mm", minimal.cusp_height_mm.is_none()),
            ("coarseness", minimal.coarseness.is_none()),
            ("overlap_mm", minimal.overlap_mm.is_none()),
            (
                "max_regions_per_tier",
                minimal.max_regions_per_tier.is_none(),
            ),
        ] {
            assert!(unset, "`{name}` must be None when omitted, not defaulted");
        }

        let full: PlanMultitoolFinishingParam = serde_json::from_value(serde_json::json!({
            "setup_index": 2,
            "model_id": 5,
            "tool_ids": [1],
            "cell_mm": 0.4,
            "tolerance_mm": 0.02,
            "margin_mm": 0.75,
            "cusp_height_mm": 0.05,
            "coarseness": 1.5,
            "overlap_mm": 3.0,
            "max_regions_per_tier": 8,
        }))
        .expect("every dial must be accepted");
        assert_eq!(full.model_id, Some(5));
        assert_eq!(full.cell_mm, Some(0.4));
        assert_eq!(full.tolerance_mm, Some(0.02));
        assert_eq!(full.margin_mm, Some(0.75));
        assert_eq!(full.cusp_height_mm, Some(0.05));
        assert_eq!(full.coarseness, Some(1.5));
        assert_eq!(full.overlap_mm, Some(3.0));
        assert_eq!(full.max_regions_per_tier, Some(8));
    }

    /// P5 — an omitted dial must stay `None`, never default to a number.
    /// `tolerance_mm` is the one that matters: `Some(0.0)` and `None` mean
    /// different things (a zero bar against the operation's own bar), and a
    /// `#[serde(default)]` on an `f64` would silently turn the second into
    /// the first.
    #[test]
    fn reach_map_param_leaves_omitted_dials_unset() {
        let minimal: ReachMapParam = serde_json::from_value(serde_json::json!({
            "index": 3,
        }))
        .expect("an index alone must deserialise");
        assert_eq!(minimal.index, 3);
        assert!(
            minimal.tolerance_mm.is_none(),
            "an omitted tolerance means the operation's own, never 0.0"
        );
        assert!(minimal.histogram_bins.is_none());

        let full: ReachMapParam = serde_json::from_value(serde_json::json!({
            "index": 0,
            "tolerance_mm": 0.02,
            "histogram_bins": 16,
        }))
        .expect("every dial must be accepted");
        assert_eq!(full.tolerance_mm, Some(0.02));
        assert_eq!(full.histogram_bins, Some(16));
    }

    /// P5 — the screenshot's reach flag is opt-in and tri-state. An agent
    /// cannot see the GUI's own viewport toggle, so the absence of the flag
    /// must mean "no overlay", not "whatever the window happens to show".
    #[test]
    fn screenshot_toolpath_reach_overlay_is_opt_in() {
        let plain: ScreenshotToolpathParam = serde_json::from_value(serde_json::json!({
            "index": 0,
            "path": "/tmp/tp.png",
        }))
        .expect("index + path alone must deserialise");
        assert!(plain.reach_overlay.is_none());

        let with_overlay: ScreenshotToolpathParam = serde_json::from_value(serde_json::json!({
            "index": 0,
            "path": "/tmp/tp.png",
            "reach_overlay": true,
        }))
        .expect("the flag must be accepted");
        assert_eq!(with_overlay.reach_overlay, Some(true));
    }

    /// Phase U — the preview's wire contract, and the reason it is a
    /// separate test rather than a parametrised one: the two structs must
    /// agree field for field, and a test that shared a body could not fail
    /// when one of them grew a dial the other did not.
    #[test]
    fn preview_tier_map_param_round_trips_with_dials_omitted() {
        let minimal: PreviewTierMapParam = serde_json::from_value(serde_json::json!({
            "setup_index": 1,
            "tool_ids": [4, 9],
        }))
        .expect("setup_index + tool_ids alone must deserialise");
        assert_eq!(minimal.setup_index, 1);
        assert_eq!(minimal.tool_ids, vec![4, 9]);
        assert!(minimal.model_id.is_none());
        assert!(
            minimal.svg_path.is_none(),
            "an omitted svg_path means numbers only, never a guessed file"
        );
        for (name, unset) in [
            ("cell_mm", minimal.cell_mm.is_none()),
            ("tolerance_mm", minimal.tolerance_mm.is_none()),
            ("margin_mm", minimal.margin_mm.is_none()),
            ("cusp_height_mm", minimal.cusp_height_mm.is_none()),
            ("coarseness", minimal.coarseness.is_none()),
            ("overlap_mm", minimal.overlap_mm.is_none()),
            (
                "max_regions_per_tier",
                minimal.max_regions_per_tier.is_none(),
            ),
        ] {
            assert!(unset, "`{name}` must be None when omitted, not defaulted");
        }

        let full: PreviewTierMapParam = serde_json::from_value(serde_json::json!({
            "setup_index": 2,
            "model_id": 5,
            "tool_ids": [1],
            "cell_mm": 0.4,
            "tolerance_mm": 0.02,
            "margin_mm": 0.75,
            "cusp_height_mm": 0.05,
            "coarseness": 1.5,
            "overlap_mm": 3.0,
            "max_regions_per_tier": 8,
            "svg_path": "/tmp/tiers.svg",
        }))
        .expect("every dial must be accepted");
        assert_eq!(full.model_id, Some(5));
        assert_eq!(full.cell_mm, Some(0.4));
        assert_eq!(full.tolerance_mm, Some(0.02));
        assert_eq!(full.margin_mm, Some(0.75));
        assert_eq!(full.cusp_height_mm, Some(0.05));
        assert_eq!(full.coarseness, Some(1.5));
        assert_eq!(full.overlap_mm, Some(3.0));
        assert_eq!(full.max_regions_per_tier, Some(8));
        assert_eq!(full.svg_path.as_deref(), Some("/tmp/tiers.svg"));
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
        // L9: the historical loader aliases are gone. This surface
        // takes canonical names only, and it says so.
        assert!(parse_tool_type("ball").is_err());
        assert!(parse_tool_type("flat").is_err());
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

    /// The `overlays` map deserializes as `{id: bool}`, and its absence is
    /// still a valid request — an agent that never touches an overlay must
    /// not have to send an empty object.
    #[test]
    fn set_ui_view_param_carries_an_overlays_map() {
        let p: SetUiViewParam = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(p.overlays.is_none());

        let p: SetUiViewParam = serde_json::from_value(serde_json::json!({
            "workspace": "toolpaths",
            "overlays": { "rest_heatmap": true, "tier_map": false },
        }))
        .unwrap();
        let overlays = p.overlays.expect("the map deserializes");
        assert_eq!(overlays.get("rest_heatmap"), Some(&true));
        assert_eq!(overlays.get("tier_map"), Some(&false));
    }

    // ── Gap 6 (2026-08-19 run log): typed `value` ─────────────────────

    /// The blocker. `serde_json::Value`'s own schema is the bare `true`
    /// — no type information at all — and a client handed that field an
    /// array serialised it as a STRING, so the call failed outright:
    /// `invalid type: string "[[2.5,2.5],[237.5,247.5]]", expected a
    /// sequence`. The schema must now name the JSON types it accepts,
    /// and `array` must be one of them.
    #[test]
    fn set_toolpath_param_value_schema_names_its_types_including_array() {
        for schema in [
            schemars::SchemaGenerator::default().into_root_schema_for::<SetToolpathParamInput>(),
            schemars::SchemaGenerator::default().into_root_schema_for::<SetToolParamInput>(),
            schemars::SchemaGenerator::default().into_root_schema_for::<SetDressupFieldParam>(),
        ] {
            let json = serde_json::to_value(&schema).expect("schema serialises");
            let value_schema = json
                .pointer("/properties/value")
                .expect("the tool exposes a `value` property");
            assert!(
                value_schema.is_object(),
                "`value` must carry a real schema, not the untyped `true`: {value_schema}"
            );
            let types = value_schema
                .pointer("/type")
                .and_then(|t| t.as_array())
                .expect("`value` must declare a type list");
            let types: Vec<&str> = types.iter().filter_map(|t| t.as_str()).collect();
            for expected in ["array", "object", "number", "string", "boolean"] {
                assert!(
                    types.contains(&expected),
                    "`value` must accept {expected}: {types:?}"
                );
            }
        }
    }

    /// Second layer: a client that stringifies anyway. Only container
    /// shapes are unwrapped — enum strings and numeric strings are left
    /// for the type-aware coercion in `ProjectSession`.
    #[test]
    fn stringified_containers_are_unwrapped_and_nothing_else_is() {
        let holes = coerce_json_container_string(serde_json::json!("[[2.5,2.5],[237.5,247.5]]"));
        assert_eq!(holes, serde_json::json!([[2.5, 2.5], [237.5, 247.5]]));

        let obj = coerce_json_container_string(serde_json::json!("{\"a\": 1}"));
        assert_eq!(obj, serde_json::json!({"a": 1}));

        // Leading whitespace is tolerated.
        assert_eq!(
            coerce_json_container_string(serde_json::json!("  [1, 2]")),
            serde_json::json!([1, 2])
        );

        // Untouched: enum strings, numeric strings, already-typed values,
        // and malformed container text (so the error still quotes what
        // the caller actually sent).
        for untouched in [
            serde_json::json!("climb"),
            serde_json::json!("7"),
            serde_json::json!(7),
            serde_json::json!(true),
            serde_json::json!(null),
            serde_json::json!([1, 2]),
            serde_json::json!("[1, 2"),
        ] {
            assert_eq!(
                coerce_json_container_string(untouched.clone()),
                untouched,
                "value must survive unchanged"
            );
        }
    }

    // ── Gap 4: type-aware, honest `add_tool` ─────────────────────────

    fn add_tool_spec(tool_type: &str, diameter: f64) -> AddToolParam {
        AddToolParam {
            name: "T".to_owned(),
            tool_type: tool_type.to_owned(),
            diameter,
            ..AddToolParam::default()
        }
    }

    /// The headline defect: a 20-degree V-bit created as `included_angle
    /// 90` is silently a different tool. There is no honest default, so
    /// the call is refused — and the refusal says which field and why.
    #[test]
    fn defining_geometry_is_required_per_tool_type() {
        let err = build_tool_config(&add_tool_spec("v_bit", 12.7)).unwrap_err();
        assert!(err.contains("included_angle"), "{err}");
        let err = build_tool_config(&add_tool_spec("tapered_ball_nose", 3.0)).unwrap_err();
        assert!(err.contains("taper_half_angle"), "{err}");
        let err = build_tool_config(&add_tool_spec("bull_nose", 12.7)).unwrap_err();
        assert!(err.contains("corner_radius"), "{err}");

        // …and supplying it keeps the number the caller asked for.
        let spec = AddToolParam {
            included_angle: Some(20.0),
            ..add_tool_spec("v_bit", 12.7)
        };
        let built = build_tool_config(&spec).unwrap();
        assert_eq!(built.config.included_angle, 20.0);
        assert_eq!(built.config.tool_type, ToolType::VBit);
    }

    /// Out-of-domain geometry is refused rather than stored.
    #[test]
    fn defining_geometry_is_range_checked() {
        let spec = AddToolParam {
            included_angle: Some(180.0),
            ..add_tool_spec("v_bit", 12.7)
        };
        assert!(build_tool_config(&spec).is_err());

        // A bull-nose corner radius above diameter/2 is not a bull nose.
        let spec = AddToolParam {
            corner_radius: Some(7.0),
            ..add_tool_spec("bull_nose", 12.0)
        };
        assert!(build_tool_config(&spec).is_err());
        let spec = AddToolParam {
            corner_radius: Some(6.0),
            ..add_tool_spec("bull_nose", 12.0)
        };
        assert!(build_tool_config(&spec).is_ok());

        assert!(build_tool_config(&add_tool_spec("end_mill", 0.0)).is_err());
        assert!(build_tool_config(&add_tool_spec("not_a_tool", 6.0)).is_err());
    }

    /// A flat end mill used to be stored — and REPORTED, via
    /// `list_tools`' `corner_radius_mm.max(corner_radius)` — carrying a
    /// 2 mm corner radius and a 90-degree point it does not have.
    #[test]
    fn a_tool_does_not_carry_another_types_geometry() {
        let built = build_tool_config(&add_tool_spec("end_mill", 6.0)).unwrap();
        assert_eq!(built.config.corner_radius, 0.0);
        assert_eq!(built.config.included_angle, 0.0);
        assert_eq!(built.config.taper_half_angle, 0.0);

        let spec = AddToolParam {
            taper_half_angle: Some(5.6),
            ..add_tool_spec("tapered_ball_nose", 3.0)
        };
        let built = build_tool_config(&spec).unwrap();
        assert_eq!(built.config.taper_half_angle, 5.6);
        assert_eq!(built.config.included_angle, 0.0);
        assert_eq!(built.config.corner_radius, 0.0);
    }

    /// Defaults that remain are NAMED, so "25 mm of cutting length" can
    /// never again read as a measurement of the caller's tool.
    #[test]
    fn every_unsupplied_field_is_named_in_defaulted() {
        let built = build_tool_config(&add_tool_spec("end_mill", 6.0)).unwrap();
        for field in [
            "cutting_length",
            "shaft_diameter",
            "shank_diameter",
            "shank_length",
            "stickout",
            "holder_diameter",
            "flute_count",
        ] {
            assert!(
                built.defaulted.contains(&field),
                "`{field}` was defaulted but not reported: {:?}",
                built.defaulted
            );
        }

        let spec = AddToolParam {
            cutting_length: Some(32.0),
            flute_count: Some(3),
            stickout: Some(28.0),
            ..add_tool_spec("end_mill", 6.0)
        };
        let built = build_tool_config(&spec).unwrap();
        assert_eq!(built.config.cutting_length, 32.0);
        assert_eq!(built.config.flute_count, 3);
        assert_eq!(built.config.stickout, 28.0);
        for field in ["cutting_length", "flute_count", "stickout"] {
            assert!(
                !built.defaulted.contains(&field),
                "`{field}` was supplied — it must not be reported as defaulted"
            );
        }
        assert!(build_tool_config(&AddToolParam {
            flute_count: Some(0),
            ..add_tool_spec("end_mill", 6.0)
        })
        .is_err());
    }

    /// `add_tool` is all-optional beyond name/type/diameter, and the
    /// old three-field call still deserializes unchanged.
    #[test]
    fn add_tool_param_deserializes_old_and_new_forms() {
        let p: AddToolParam = serde_json::from_value(serde_json::json!({
            "name": "6mm EM", "tool_type": "end_mill", "diameter": 6.0
        }))
        .unwrap();
        assert!(p.included_angle.is_none());
        assert!(p.tool_number.is_none());

        let p: AddToolParam = serde_json::from_value(serde_json::json!({
            "name": "20deg V", "tool_type": "v_bit", "diameter": 12.7,
            "included_angle": 20.0, "flute_count": 1, "cutting_length": 12.0,
            "tool_number": 4
        }))
        .unwrap();
        assert_eq!(p.included_angle, Some(20.0));
        assert_eq!(p.flute_count, Some(1));
        assert_eq!(p.tool_number, Some(4));
    }

    // ── Gaps 1 + 2: stock config ─────────────────────────────────────

    /// Every field optional — an omitted field means "leave unchanged",
    /// so a caller can set material alone without restating dimensions.
    #[test]
    fn set_stock_config_param_is_an_all_optional_patch() {
        let p: SetStockConfigParam = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(p.x.is_none() && p.material.is_none() && p.auto_from_model.is_none());

        let p: SetStockConfigParam = serde_json::from_value(serde_json::json!({
            "x": 240.0, "y": 250.0, "z": 25.0,
            "origin_x": 0.0, "origin_y": 0.0, "origin_z": -25.0,
            "material": "White Oak",
            "workholding_rigidity": "high",
            "auto_from_model": false
        }))
        .unwrap();
        assert_eq!(p.x, Some(240.0));
        assert_eq!(p.origin_z, Some(-25.0));
        assert_eq!(p.material.as_deref(), Some("White Oak"));
        assert_eq!(p.workholding_rigidity.as_deref(), Some("high"));
        assert_eq!(p.auto_from_model, Some(false));

        // The pre-existing three-field call still deserializes.
        let p: SetStockConfigParam =
            serde_json::from_value(serde_json::json!({"x": 100.0, "y": 100.0, "z": 20.0})).unwrap();
        assert_eq!(p.z, Some(20.0));
    }

    /// Material is load-bearing for every feed in the project, so an
    /// unrecognised or ambiguous name is refused with candidates rather
    /// than resolved to a near-miss.
    #[test]
    fn material_resolves_exactly_or_refuses_with_candidates() {
        for (query, expected) in [
            ("White Oak", "White Oak"),
            ("white oak", "White Oak"),
            ("  WHITE-OAK ", "White Oak"),
            ("MDF", "MDF"),
            ("Aluminum 6061-T6", "Aluminum 6061-T6"),
        ] {
            let mat = resolve_material(query).unwrap_or_else(|e| panic!("{query}: {e}"));
            assert_eq!(mat.label(), expected);
        }

        let err = resolve_material("unobtainium").unwrap_err();
        assert!(err.contains("Unknown material"), "{err}");
        assert!(err.contains("nothing was written"), "{err}");

        // Two catalog aluminums — a bare "aluminum" must not pick one.
        let err = resolve_material("aluminum").unwrap_err();
        assert!(err.contains("Ambiguous"), "{err}");
        assert!(err.contains("6061"), "{err}");

        assert!(resolve_material("   ").is_err());
    }

    #[test]
    fn workholding_rigidity_parses_or_refuses() {
        assert_eq!(
            parse_workholding_rigidity("Low"),
            Ok(WorkholdingRigidity::Low)
        );
        assert_eq!(
            parse_workholding_rigidity(" medium "),
            Ok(WorkholdingRigidity::Medium)
        );
        assert_eq!(
            parse_workholding_rigidity("HIGH"),
            Ok(WorkholdingRigidity::High)
        );
        let err = parse_workholding_rigidity("rigid").unwrap_err();
        assert!(err.contains("low, medium, high"), "{err}");
    }

    /// Machine kinematics: all-optional patch, so a caller can set
    /// junction deviation without restating the accelerations.
    #[test]
    fn set_machine_kinematics_param_is_an_all_optional_patch() {
        let p: SetMachineKinematicsParam = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(p.acceleration_x_mm_s2.is_none() && p.junction_deviation_mm.is_none());
        assert!(p.max_rate_x_mm_min.is_none() && p.max_rate_z_mm_min.is_none());

        let p: SetMachineKinematicsParam = serde_json::from_value(serde_json::json!({
            "acceleration_x_mm_s2": 500.0,
            "acceleration_y_mm_s2": 500.0,
            "acceleration_z_mm_s2": 270.0,
            "junction_deviation_mm": 0.02
        }))
        .unwrap();
        assert_eq!(p.acceleration_z_mm_s2, Some(270.0));
        assert_eq!(p.junction_deviation_mm, Some(0.02));
        // The rate triple is its own independent patch (P1).
        assert_eq!(p.max_rate_z_mm_min, None);

        let p: SetMachineKinematicsParam = serde_json::from_value(serde_json::json!({
            "max_rate_x_mm_min": 10000.0,
            "max_rate_y_mm_min": 10000.0,
            "max_rate_z_mm_min": 1000.0
        }))
        .unwrap();
        assert_eq!(p.max_rate_x_mm_min, Some(10000.0));
        assert_eq!(p.max_rate_z_mm_min, Some(1000.0));
        assert_eq!(p.acceleration_x_mm_s2, None);
    }

    /// CLI-09 sentry: `set_setup_rotation` publishes its four legal
    /// values in the schema, and refuses everything else.
    ///
    /// The field used to be a bare `String`. `schemars` emitted an
    /// unconstrained `"type": "string"`, so a client could not discover
    /// the legal values from the schema at all — only from the prose of
    /// the doc comment — and the viz server accepted `"90deg"` and
    /// `"90 deg"` through a hand parse it had written itself.
    #[test]
    fn set_setup_rotation_publishes_its_four_values() {
        let schema = schemars::schema_for!(SetSetupRotationParam);
        let root = serde_json::to_value(&schema).unwrap();
        let property = root
            .pointer("/properties/z_rotation")
            .expect("z_rotation must be a published property");

        // schemars may either inline the enum or reference it from
        // `$defs`. Follow the reference when there is one.
        let resolved = match property.get("$ref").and_then(serde_json::Value::as_str) {
            Some(reference) => {
                let name = reference.rsplit('/').next().expect("a $ref names a def");
                root.pointer(&format!("/$defs/{name}"))
                    .expect("the $ref must resolve")
            }
            None => property,
        };
        let values = resolved
            .get("enum")
            .and_then(serde_json::Value::as_array)
            .expect("z_rotation must publish an enum of its legal values");
        let tokens: Vec<&str> = values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert_eq!(tokens, ["0", "90", "180", "270"]);
    }

    /// CLI-09: the token maps to the core variant, and the lenient
    /// spellings the hand parse used to accept are now refused on the
    /// wire (operator ruling 2026-09-16, no legacy support).
    #[test]
    fn set_setup_rotation_takes_the_token_and_refuses_the_rest() {
        for (token, expected) in [
            ("0", ZRotation::Deg0),
            ("90", ZRotation::Deg90),
            ("180", ZRotation::Deg180),
            ("270", ZRotation::Deg270),
        ] {
            let p: SetSetupRotationParam =
                serde_json::from_value(serde_json::json!({"setup_index": 0, "z_rotation": token}))
                    .unwrap_or_else(|e| panic!("the wire refused the legal token {token}: {e}"));
            assert_eq!(ZRotation::from(p.z_rotation), expected, "token {token}");
        }
        for token in ["90deg", "90 deg", "Deg90", "", "45"] {
            let parsed: Result<SetSetupRotationParam, _> =
                serde_json::from_value(serde_json::json!({"setup_index": 0, "z_rotation": token}));
            assert!(
                parsed.is_err(),
                "the wire still accepts {token:?}; the typed enum did not replace the hand parse"
            );
        }
    }
}
