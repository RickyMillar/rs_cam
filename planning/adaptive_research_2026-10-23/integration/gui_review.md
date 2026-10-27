# GUI review: Geodesic Adaptive and the rest workflow

Reviewer: independent GUI review agent, 2026-10-28.
Tree: `.claude/worktrees/geodesic-integration`, branch `geodesic-integration`,
HEAD `a757e51dd`. Base: `ab208f83a`.
Method: code reading only. Paths are relative to the worktree root. Line
numbers are from HEAD `a757e51dd`.

The branch changes the GUI in one file of substance,
`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`. It removes
the Contour Spiral "Optimal load" slider and the three removed strategies
from the Strategy combo, and it adds the Geodesic Adaptive option with a
hover. Every other GUI surface is unchanged. The findings below are mostly
about what the unchanged surfaces do not show for the new strategy.

## 1. Make a 3D Rough op and choose Geodesic Adaptive

The editor is `draw_adaptive3d_params`
(`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:54-226`).
It draws the same rows for both strategies:

| Row | Control | Help source | Geodesic reads it? |
|---|---|---|---|
| Stepover | `dv_pill` with a Suggest pill (`surface_3d.rs:65-72`) | schema `HELP_STEPOVER` | Yes: `adaptive3d/path.rs:1044`; the chord cap `mod.rs:399-405` |
| Depth/Pass | `dv_pill` with a pill (`:73-80`) | schema | Yes |
| Stock to Leave | `dv` (`:81-91`) | schema | Yes (`session/compute/simulation.rs:551`) |
| Entry Style | combo Plunge / Helix / Ramp, no hover (`:92-105`) | none | Yes: `path.rs:1322` |
| Ramp Angle, Entry Clearance | shown for Ramp (`:108-125`) | schema | Yes |
| Helix Radius, Helix Pitch, Entry Clearance | shown for Helix (`:126-157`) | schema | Yes |
| Strategy | combo, label has no hover (`:159-190`) | none: `clearing_strategy` has no `#[param(help)]` (`crates/rs_cam_core/src/compute/operation_configs.rs:1672-1673`) | — |
| Advanced: Tolerance | `dv` (`:192-198`) | schema | Yes: `path.rs:388, 718`; and the load-margin cap `mod.rs:399` |
| Advanced: Min Cut Radius | `dv` (`:199-208`) | schema | Yes: `path.rs:1878`, `clearing.rs:2215-2304` |
| Advanced: Detect Flat | checkbox, no hover (`:209-211`) | none | Yes: `path.rs:991` (shared level planning) |
| Advanced: Ordering | combo Global / By Area, no hover (`:212-223`) | none | Yes: `path.rs:1074`; the Geodesic boundary reach (G-CLIPAIR) runs with Global only (`path.rs:885-887`) |
| Advanced: Z Blend | checkbox, no hover (`:224-226`) | none | **No.** The only reader is `clear_z_level_contour_parallel` (`crates/rs_cam_core/src/adaptive3d/clearing.rs:1304`, read at `:1378`). The slice path from `:1857` has no `z_blend` read. |

Tool, boundary, containment, offset and "Start from" live on the Cut and
Geometry tabs and are the same for every op (see §3). Feeds live on the
Feeds tab (`feeds_speeds.rs`). The Linking dressup tab is the generic one.

Findings:

- **Z Blend is a dead control under Geodesic Adaptive.** The checkbox
  stays on, the value is saved, and nothing reads it. A user who turns it on
  for terrain sees no change and no explanation. Fix: hide the row, or grey
  it with the hover "Contour Parallel only", when
  `cfg.clearing_strategy == GeodesicAdaptive` (`surface_3d.rs:224-226`).
- **Ordering has a Geodesic-only side effect with no hover.** With By Area
  the Geodesic boundary reach clip does not run (`path.rs:885-887`). The
  combo has no hover text at all (`surface_3d.rs:212-223`). Fix: a hover on
  the row that names the Global rule.
- **The Strategy row has no help on its label**, and the Contour Parallel
  option has no hover (`surface_3d.rs:159-190`). The Geodesic hover says
  "follow it with a Contour Parallel rest op for the edges" but does not
  name the two settings that make one (boundary source Rest Regions with
  this op as source; Start from: Rest machining). A user who reads the
  hover still has to find the workflow. Fix: give `clearing_strategy` a
  schema help and make the combo label `p(...)` keyed (the ui04 rule), and
  name the two settings in the Geodesic hover.
- **Three schema fields have no GUI row for either strategy**:
  `min_region_cut_length_mm`, `max_stay_down_distance_mm`,
  `stay_down_clearance_mm` (`operation_configs.rs:1693-1717`). No viz file
  reads them (rg). The MCP schema exposes them. For Geodesic the stay-down
  route runs at cut feed by a strategy default with no dial
  (`adaptive3d/mod.rs:318-336`). This is pre-existing, not new on the
  branch, but the branch's hover claims "a keep-down link is routed at the
  cut feed" without a control. Nice-to-have: one hover sentence on the
  Strategy row that says the stay-down rule is fixed.
- The two load-only fields `trochoid_cap_mult` and `engagement_measure`
  are not drawn (good). The MCP schema lists them with the "Not read by 3D
  Rough" help (`operation_configs.rs:1674-1690`).

## 2. Switch between Contour Parallel and Geodesic Adaptive

The combo holds two values only (`surface_3d.rs:164-190`). The rows do not
change on a switch (the editor has no strategy gate). The "Optimal load"
slider and `FALLBACK_TOOL_RADIUS` are gone; `OpDrawCtx::tool_radius` is
gone (`registry.rs:65-68` diff). An rg for `AgentSearch|ContourSpiral|
agent_search|contour_spiral|Optimal load|trochoid|engagement_measure` in
`crates/rs_cam_viz/src` and `crates/rs_cam_mcp/src` finds one comment
(`surface_3d.rs:61`) and one test fixture
(`crates/rs_cam_viz/tests/fixtures/stale_cards/face_and_rest_rough.toml:216-217`,
load-only keys, which still load). No old control is reachable.

MCP: `set_toolpath_param clearing_strategy = "contour_spiral"` refuses
with the loader's own text
(`crates/rs_cam_core/src/session/compute/params.rs:177-181`,
`CLEARING_STRATEGY_REPLACEMENT` at `session/project_file.rs:252-254`).

Findings:

- Stale wording that names the removed spiral verdict:
  `crates/rs_cam_viz/src/mcp_server.rs:1902` ("the parallel-vs-spiral
  verdict from `recommend_clearing_strategy`") and
  `crates/rs_cam_mcp/src/server.rs:1680` ("parallel-vs-spiral strategy
  verdict"). The MCP wire snapshot carries the first one. Nice-to-have.
- The Z Blend row stays visible after a switch to Geodesic (see §1).

## 3. The body + edges workflow

The user can set it up in the GUI:

- Source picker: the "Rest Regions" button
  (`crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:655-690`) is
  enabled when another toolpath exists; the click takes the first candidate.
  The "Rest source:" combo (`:711-760`) lists every other toolpath, with
  "(regions ready)" when its cached result has non-empty regions
  (`ui/properties/mod.rs:606-625`). The hover says picking a source switches
  its rest analysis on. The pick writes `Command::AutoEnableRestAnalysis`,
  which stales the source (`crates/rs_cam_core/src/session/mutation/config.rs:279`),
  so the plan regenerates the source with the demand set.
- Containment: combo Center / Inside / Outside (`:786-818`), hovers "Tool
  center stays inside boundary" and so on.
- Offset: drag value (`:821-833`).
- Remaining stock: the "Rest machining: from previous ops" checkbox on the
  Geometry tab (`ui/properties/tab_badges.rs:662-675`,
  `START_FROM_HOVER`).
- The op list draws the Regions rail with "Rest regions of X", "X is not
  current" or "source missing" on hover (`ui/toolpath_panel.rs:1376-1382`).

The refusals all fire at generate time in core, as
`SessionError::OperationFailed`:

- source not generated: `session/compute/generation.rs:618-621`;
- source produced no regions: `:626-629`;
- D53 larger tool with Center or Outside: `:522-577`.

Each one lands as `ToolpathRun::Failed` (`controller/events/compute.rs:612`)
→ `FreshnessState::Error` (`state/freshness.rs:93`) → the op row reads
"ERR" in danger with the message on hover (`ui/toolpath_panel.rs:1116`),
the inspector header prints "Error: …" wrapped with the full text
(`ui/properties/toolpath_panel.rs:1440-1448`), and the rest overlay reads
`Live::Failed` with a Generate recovery (`ui/overlays/live.rs:191-194`).
The D53 sentence names both ops, both radii, the overrun and both fixes.
The user sees it where they work, after a generate.

Findings:

- **D53 has no edit-time caution.** The Containment combo shows "Center"
  with the generic hover, and nothing on the Cut tab or the Generate
  validation ladder (`ui/properties/operations/validate.rs`: the context
  holds hole refusals and no-tool states only, `:13-22`) says the tool is
  larger than the source's. The user learns it after a generate, which on a
  3D op costs a plan step. Fix: expose `refuse_larger_rest_consumer` as a
  read door and draw its sentence as a DANGER line under the Containment
  row, and in the validation ladder, when the source is `DerivedRestRegions`.
- **No "regions left" figure for a Geodesic source.** The rest disclosure
  (`ui/properties/toolpath_panel.rs:863-1000`) draws a pathology line only
  (too many islands, one giant region). A Geodesic rough that left N
  pieces for the rest op shows nothing that says N, and the "(regions
  ready)" tag in the consumer's combo is the only sign. Fix: one `OneLine`
  "N regions left for a rest op · top Z z" from `produced.rest_regions`
  and `rest_regions_top_z`.
- The "Rest Regions" and "Rest source" hovers describe a rest-analysis
  heatmap ("computed by another toolpath's rest analysis",
  `:674-677`, `:748-753`). For a Geodesic source the regions are the pieces
  the rough did not cut (D44), published under the same demand. The words
  are not wrong, but they do not tell the user the new rule. Nice-to-have.
- First-generate order: a source that generated before the consumer
  existed has no regions. The pick stales it, so the plan regenerates it.
  I did not run it; see "Not checked".

## 4. Open an old project that names a removed strategy

Core refuses in `check_retired_keys`
(`session/project_file.rs:383-410`) with `SessionError::RetiredKeys`:
"The project file uses N retired key(s)." then one line per hit with the
path (`setups[0].toolpaths[0].operation.params.clearing_strategy =
"agent_search"`) and the replacement text, which names the two valid values
(`session/mod.rs:560-561`, `project_file.rs:252-254`).

GUI: `open_job_from_path` wraps it as `VizError::ProjectLoad`
(`controller/io.rs:500-502`), whose user text is "Load failed: …"
(`error.rs:49`). The two interactive routes call `push_error`
(`app/input.rs:713-725`), which is a toast: bottom-right, max width
400 px, error TTL 8 s by default (`controller.rs:561-569`,
`toast_ttl` `:104-114`, `DEFAULT_TOAST_SECONDS` in core settings), four
toasts visible at once (`app.rs:1300-1335`). The `RS_CAM_JOB` route only
logs (`app.rs:366-372`). MCP `load_project` returns the text
(`app/mcp/project.rs:835`).

Findings:

- **The refusal is a transient toast.** It is multi-line, names a TOML
  path and a value, and the user must edit the file by hand (nothing
  converts, by ruling). Eight seconds in a 400 px box is not enough to act
  on, and there is no copy. Fix: route `VizError::ProjectLoad` to the
  load-warnings window (`ui/components/notice.rs` names one) or a modal
  with the full text and the file path, with a copy act. Should-fix.
- No GUI door offers "open with Contour Parallel". That is the ruling, so
  it is not a finding; the message does name the fix.

## 5. Feed caps

`Move::feed_cap` is bound in every feed stage in core, and the simulation
writes `trace.capped_moves` (`crates/rs_cam_core/src/compute/simulate.rs:1423-1430`,
`stock/simulation_cut.rs:855-867`).

No viz or MCP surface reads it:

- `rg "capped_moves|CappedMove|FeedCap|FeedBound"` in
  `crates/rs_cam_viz/src` finds nothing. The `feed_cap` hits in
  `ui/feeds/explore.rs` are the machine feed cap of the feeds explorer,
  another thing.
- The toolpath view does not colour a capped move; `render/toolpath_render.rs`
  only fills `feed_cap: None` in a test.
- `build_cut_trace_response` (`app/mcp/simulation.rs:509-780`) has no
  `capped_moves` key, so MCP `get_cut_trace` does not return the table.
- The tool-load verdict has `FeedBound::FeedCap`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:76-81`), but its comment
  says "No shipped planner writes a cap, so a shipped run never reports
  it", which is false on this branch, and no GUI prints a feed bound.
- The simulation trace modal and timeline print no per-move feed source.

Does it matter? Yes. The cap is the load-safety mechanism of the strategy
(a turn or a slot fed down to the pass load) and it changes cycle time. A
user who sees a slow turn in the simulation cannot tell a cap from a
modulation or a plunge rate, and a reviewer cannot confirm from the GUI
or MCP that a cap survived the dressups (the sentry
`a_geodesic_cap_survives_every_dressup_s7_4` is the only check). Fix, in
order: add `capped_moves` (count per toolpath, min cap, and the rows under
the response cap) to `get_cut_trace`; one line in the simulation trace
modal "N moves at a feed cap, lowest X mm/min"; a colour or a hover in
the toolpath view; update the `verdict.rs:80` comment. Should-fix.

## 6. Strategy advisor and Suggest

Advisor: MCP only. `recommend_clearing_strategy` (`mcp_server.rs:536-548`)
compares Contour Parallel and Geodesic Adaptive
(`crates/rs_cam_core/src/machine/strategy_advisor.rs:108-118, 300-400`).
It runs as a `JobHandle::RecommendClearingStrategy` on the worker job lane
(`compute/worker.rs:1787-1791`, `run_job`), which holds no session, so a
38 s run does not block the UI thread; the MCP call waits for the answer.
The job list names it "Strategy advisor (op)" (`worker.rs:1459`). There is
no GUI button: `rg "advisor|Advisor"` in `ui/` finds only simulation
advisories. The catalog row says "the strategy advisor compares Contour
Parallel with Geodesic Adaptive" without a surface; a GUI user has no way
to run it.

Suggest: core adds the warn-only
`SuggestWarning::StrategyRecommendedNotApplied` for an Adaptive3d roughing
op on Contour Parallel over a MixedTerrain bbox with scope
`StrategyAndFeeds`, the default (`feeds/suggest/adaptive_entry.rs:62-93`,
`feeds/suggest.rs:108-121`). The GUI feeds card maps that warning to no
visible line (`ui/feeds/why.rs:1140`, the arm returns `None`). Its
rationale headline "clearing_strategy: classifier recommends
geodesic_adaptive (not auto-applied, current contour_parallel)"
(`feeds/rationale.rs:669-690`) contains none of the row keywords
(`why.rs:85-93`), so it lands only in the Advance row's catch-all hover
(`why.rs:446-470`). MCP `get_suggest_rationale` returns it in full. The
Suggest preview itself is the feeds calculator, memoised once per input
(F216); it plans no toolpath and does not block.

Findings:

- **The Geodesic recommendation is hidden in a hover.** A user who clicks
  Suggest on a terrain rough never sees "try Geodesic Adaptive" unless they
  hover the Advance row. Fix: a visible caution line on the feeds card, or
  a line on the Strategy row, with a one-click "Switch" act that writes
  `clearing_strategy` through the panel. Should-fix.
- **No GUI door to the advisor.** Nice-to-have: a "Compare strategies"
  button on the Strategy row that submits the job and shows the ranked
  result; or state in the catalog that the advisor is MCP-only.

## 7. Export and the tool-load gate

The core door `export_program` runs the gate over the emitted toolpaths
only (D47, `session/compute/export.rs:88-92`). A refusal is
`ExportRefusal::LoadExceeded` with "G-code export refused: tool load
exceeded on toolpath(s):" then one line per toolpath, the non-refusing
rows beside it, and "To override (a known-dangerous override): …"
(`crates/rs_cam_core/src/gcode/mod.rs:197-201, 252-255`);
`LoadUnmodeled` and `StaleSimulation` have the same shape.

GUI: the wizard save maps it to `VizError::Export` and `push_error`
(`app/export.rs:31-43`), a toast. Before Save, the pre-flight modal shows
the tool-load status, its summary and the override flags
(`ui/preflight.rs:154-160, 231, 271`), and the readiness panel shows "Review
tool load" (`ui/readiness_panel.rs:34-45`). MCP `export_gcode` runs the
same door with the same policy.

The message is clear. Nothing on the branch changes the gate for these
ops. The one branch-specific gap is §5: a Geodesic cap lowers the feed the
chipload verdict reads, and no surface says a cap did it. No separate
finding.

## 8. MCP parity

| Control (§) | GUI | MCP | Parity |
|---|---|---|---|
| Strategy choice (1, 2) | combo | `set_toolpath_param clearing_strategy`; `get_operation_schema` lists the variants (`ParamEnum` on `ClearingStrategy`, `operation_configs.rs:134-136`) | Yes |
| Retired strategy value (2, 4) | load refuses, toast | `set_toolpath_param` refuses with the same text (`params.rs:177-181`); `load_project` returns the message | Yes |
| Stepover, DPP, stock to leave, entry style and its fields, tolerance, min cut radius, detect flat, ordering, Z blend (1) | rows | schema | Yes (Z Blend dead for both) |
| `min_region_cut_length_mm`, `max_stay_down_distance_mm`, `stay_down_clearance_mm` (1) | none | schema | **MCP only** |
| `trochoid_cap_mult`, `engagement_measure` (1) | none | schema, load-only help | MCP only, by design (R6) |
| Boundary source, source id, containment, offset (3) | Cut tab | `set_boundary_config` (`crates/rs_cam_mcp/src/server.rs:1712-1726`) | Yes |
| Start from (3) | Geometry tab checkbox | `set_stock_source` | Yes |
| D53 and the other rest refusals (3) | row ERR + inspector | `generate_toolpath` error | Yes |
| Feed caps (5) | none | none (`get_cut_trace` omits `capped_moves`) | Parity by absence |
| Strategy advisor (6) | none | `recommend_clearing_strategy` | **MCP only** |
| Suggest strategy recommendation (6) | Advance row hover only | `get_suggest_rationale` full entry | GUI weaker |
| Export gate (7) | pre-flight modal, toast | `export_gcode` | Yes |

Stale MCP text: `mcp_server.rs:1902` and `rs_cam_mcp/src/server.rs:1680`
("parallel-vs-spiral"); `set_boundary_config` and `set_rest_analysis_config`
descriptions (`mcp_server.rs:1361, 1375`) and the `source_toolpath_id` doc
("whose pencil rest-depth result", `rs_cam_mcp/src/server.rs:1723-1725`)
do not say a Geodesic rough publishes its own left regions. Nice-to-have.

## 9. Architecture

Every GUI write found goes through `apply(Command)`:

- The inspector draft commits in `write_entry_config_to_session` as one
  `Command::ReplaceToolpathConfig` through `AppState::apply`
  (`ui/properties/mod.rs:1005-1025`). The strategy, entry style, boundary
  source, containment and offset all ride it.
- The rest-source pick auto-enables through
  `Command::AutoEnableRestAnalysis` (`ui/properties/apply_auto_enable`,
  `session/command.rs:2866-2876`).
- MCP `set_boundary_config` builds the same `BoundarySource` and takes the
  core request path (`app/mcp/commands.rs:1379-1410`).
- Load is `ProjectSession::load_with_warnings` (`controller/io.rs:501`),
  the existing constructor route.
- `rg "session_mut\(|project_mut\(|toolpaths_mut\("` in
  `crates/rs_cam_viz/src` (non-test) finds nothing. The branch's viz diff
  deletes code only and adds no write path.

No finding.

## Findings table

| # | Severity | Use case | Finding | Evidence | Suggested fix |
|---|---|---|---|---|---|
| 1 | should-fix | 5 | Feed caps are invisible: no toolpath colour, no trace line, no `get_cut_trace` field, no tool-load bound; the verdict comment says no shipped planner writes a cap | `simulate.rs:1423`; rg in viz: none; `app/mcp/simulation.rs:509-780`; `tool_load/verdict.rs:76-81` | `capped_moves` in `get_cut_trace`; one line in the sim trace modal; a hover or colour in the toolpath view; fix the comment |
| 2 | should-fix | 3 | D53 (larger tool, Center/Outside) is known only after a generate; no caution under Containment and none in the validation ladder | `generation.rs:522-577`; `toolpath_panel.rs:786-818`; `validate.rs:13-22` | Read door for `refuse_larger_rest_consumer`; DANGER line under Containment and in the Generate ladder |
| 3 | should-fix | 6 | Suggest's "use Geodesic Adaptive" is hidden in the Advance row hover | `adaptive_entry.rs:62-93`; `why.rs:1140, 446-470, 85-93` | Visible caution line with a "Switch" act on the feeds card or the Strategy row |
| 4 | should-fix | 4 | The retired-strategy load refusal is an 8 s, 400 px toast with a TOML path to edit by hand | `io.rs:500-502`; `error.rs:49`; `controller.rs:561-569`; `app.rs:1300-1335` | Route `VizError::ProjectLoad` to the load-warnings window or a modal with copy |
| 5 | should-fix | 1, 2 | Z Blend is a dead control under Geodesic Adaptive | `surface_3d.rs:224-226`; `clearing.rs:1304, 1378`; no read after `:1857` | Hide or grey the row with a hover when the strategy is Geodesic |
| 6 | should-fix | 3 | No "N regions left · top Z" figure on a Geodesic source; only the pathology line | `toolpath_panel.rs:863-1000`; `mod.rs:606-625` | One `OneLine` from `produced.rest_regions` and `rest_regions_top_z` |
| 7 | nice-to-have | 1 | Strategy label and Contour Parallel option have no help; the Geodesic hover does not name the two rest settings | `surface_3d.rs:159-190`; `operation_configs.rs:1672-1673` | Schema help on `clearing_strategy`; `p(...)` keyed label; name "Rest Regions" and "Rest machining" in the hover |
| 8 | nice-to-have | 1 | Ordering By Area turns the Geodesic boundary reach off, with no hover | `path.rs:885-887`; `surface_3d.rs:212-223` | Hover on the Ordering row |
| 9 | nice-to-have | 6 | No GUI door to the strategy advisor; MCP only | `mcp_server.rs:536`; rg in `ui/`: none | "Compare strategies" button that submits the job; or say MCP-only in the catalog |
| 10 | nice-to-have | 1, 8 | Three stay-down fields are MCP-only; the Geodesic hover claims a routed keep-down link with no dial | `operation_configs.rs:1693-1717`; `adaptive3d/mod.rs:318-336` | Hover sentence that the stay-down rule is fixed; or rows in Advanced |
| 11 | nice-to-have | 2, 8 | Stale "parallel-vs-spiral" wording in two MCP descriptions; rest-source hovers and MCP docs do not say a rough publishes its own left regions | `mcp_server.rs:1902, 1361, 1375`; `rs_cam_mcp/src/server.rs:1680, 1723-1725`; `toolpath_panel.rs:674-677, 748-753` | Reword |

No blocker. A user can make the op, choose the strategy, build the rest
op, and export; every refusal reaches the op row and the inspector.

## Not checked

- Live GUI. The operator's GUI (PID 4074772) runs from the main tree's
  `target/release/rs_cam_gui`, on `master`. This session's `rs-cam` MCP
  tools talk to that GUI, not to a build of the branch. A second GUI from
  the worktree would need its own MCP channel, which this session does not
  have; the worktree `target/` is 56 GB and the disk has 30 GB free, just
  over the 25 GB floor. I did not build.
- The first-generate order of the body + edges workflow (source generated
  before the consumer exists, then the pick stales it and the plan
  regenerates it). The code path reads right
  (`mutation/config.rs:279`); not run.
- Whether a generate-time refusal also raises a toast beside the row ERR
  and the inspector line. Not traced past `ToolpathRun::Failed`.
- The Suggest card's scope on the inspector path: the call builds
  `SuggestContext::default()` (`feeds_speeds.rs:206-226`); the policy
  default is `StrategyAndFeeds` (`suggest.rs:120-121`), so the warning
  fires. Not run.
- The exact rendering of the `(regions ready)` tag after a Geodesic
  source generates with the demand set.
