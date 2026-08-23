# T3 — UX findings: multi-tool island finishing

> Investigation track T3 of `planning/multitool_2026-08-23/INVESTIGATION_PROMPT.md`.
> READ-ONLY survey, 2026-08-23, branch `master`. No source edited, no cargo run.
> Every claim carries a `file:line`. Paths are repo-relative to
> `/home/ricky/personal_repos/rs_cam`.
>
> Scope note: a concurrent lane was editing pencil code; nothing in this doc
> depends on uncommitted pencil changes.

---

## 1. How would an operator express a tool ladder?

### 1.a What the data model permits — the hard constraints

**One operation cuts with exactly one tool.** `ToolpathConfig.tool_id: usize`
(`crates/rs_cam_core/src/session/mod.rs:668`) is a bare scalar — no `Vec`, no
`Option`, no per-region override. Everything downstream keys off it:

| Consumer | Evidence | Consequence for a one-op ladder |
|---|---|---|
| Load gates | `ToolpathLoadContext.tool: &'a ToolDefinition` — `crates/rs_cam_core/src/tool_load/mod.rs:372`; one context per toolpath, `evaluate_project` at `tool_load/mod.rs:529-541` | Chipload/power/deflection would be evaluated against ONE tool for a path cut by three |
| G-code | `GcodePhase.tool: Option<PhaseTool>` — `crates/rs_cam_core/src/gcode/mod.rs:134-152`; tool-change emitted when a phase's `id` differs from the previous (`gcode/mod.rs:142-145`, identity rule at `:114-131`) | One op = one phase = one `M6`. A three-tier op emits no mid-op tool change |
| Feeds provenance | `ToolpathConfig.feeds_provenance` — `session/mod.rs:696` — one `FeedsProvenance` per op | Per-tier feeds cannot be stamped or explained |
| Stock source | `ToolpathConfig.stock_source` — `session/mod.rs:685` | Tier 2 cannot declare "from remaining stock after tier 1" inside one op |
| Boundary | `ToolpathConfig.boundary` / `boundary_inherit` — `session/mod.rs:675-677` | One containment region for all tiers |

The two existing "second tool" fields are **reference geometry that never
cuts**, and the codebase says so: `RestConfig.prev_tool_id`
(`crates/rs_cam_core/src/compute/operation_configs.rs:453`) resolves to a
radius scalar only (`crates/rs_cam_core/src/session/compute.rs:1180-1191` →
`crates/rs_cam_core/src/rest.rs:38`); `PencilConfig.reference_tool_id`
(`operation_configs.rs:849`) is the rest-field probe; `RestAnalysisConfig.
reference_tool_id` (`crates/rs_cam_core/src/compute/config.rs:1522-1526`) is
"the real library tool whose geometry defines the rest reference". Reusing
that slot to mean "a tool that cuts" would invert an established convention
in three places at once.

`ChamferConfig` (`operation_configs.rs:290-311`) and `DrillConfig`
(`operation_configs.rs:181-224`) carry **zero** tool references — there is no
lead-tool or spot-drill precedent to lean on.

### 1.b Precedent for an op that GENERATES other ops — exactly one, and it is narrow

`AlignmentPinDrill` is the only auto-generated operation. It is declared
`SystemOnly` in the X-macro that generates the whole op registry —
`crates/rs_cam_core/src/compute/catalog.rs:175-177` ("Auto-generated drilling
operation for stock alignment pin holes — in `ALL`, in neither user menu"),
with the category enum at `catalog.rs:184-193`. The GUI menus iterate only
`ALL_2D` / `ALL_3D` (`crates/rs_cam_viz/src/ui/toolpath_panel.rs:659-666`,
lists at `catalog.rs:258-289`), so it is invisible to the `+ Add` menu.

Its synthesis is a **three-way reconciler in viz, not core**:
`Controller::sync_alignment_pin_drill` —
`crates/rs_cam_viz/src/controller/events/model.rs:645-790`. Create branch
`:660-770` (snapshots pin XY at `:676-682`, runs `suggest_params` for real
feeds at `:705-719`, `session.add_toolpath` at `:768`); delete branch
`:770-775`; update branch `:775-789`. Called from two pin-edit sites
(`model.rs:417`, `:641`).

**The limit that matters:** it finds its op with
`find(|tc| matches!(tc.operation, OperationConfig::AlignmentPinDrill(_)))`
(`model.rs:653-659`) — i.e. it assumes **at most one** such op exists
project-wide. A k-tier ladder needs a group key, and none exists (§1.d).

There is no template / wizard / preset / recipe machinery for operations.
`session/wizard.rs` is the *export* wizard's resumable state
(`crates/rs_cam_core/src/session/wizard.rs:1-4`, `WizardState` at `:40`).
`io/presets.rs` stores a **single-operation** param blob —
`Preset { name, operation_label, toml_content }`
(`crates/rs_cam_viz/src/io/presets.rs:5-11`, `save_preset` at `:47`) — one op
label, one TOML body; there is no "preset set".

`finish_planner.rs` and `finish_setup.rs` plan **geometry**, not operations:
`decompose` (`crates/rs_cam_core/src/finish_planner.rs:352`) and
`decompose_surface` (`:570`) return `PlannedRegions` (`:311`); `finish_setup`'s
`z_ladder` (`crates/rs_cam_core/src/finish_setup.rs:640`) is a Z-level ladder
**inside** one op — the only thing called a "ladder" in this codebase today,
and it is not a tool ladder.

`ProjectSession::add_toolpath` inserts exactly one and returns one index
(`crates/rs_cam_core/src/session/mutation.rs:63-85`). There is no bulk API.
Four non-test call sites, each inserting one op: GUI add
(`crates/rs_cam_viz/src/controller/events/toolpath.rs:165`), GUI duplicate
(`toolpath.rs:218`), the pin-drill reconciler
(`crates/rs_cam_viz/src/controller/events/model.rs:768`), and MCP add
(`crates/rs_cam_viz/src/app/mcp.rs:3358`).

### 1.c Precedent for one op with many internal passes — UnifiedFinish, and its postmortem

`UnifiedFinish` is the strongest existing "one op, many internal strategies"
shape: three slope bands (raster / scallop / waterline) plus optional crease
claims, all on one tool
(`crates/rs_cam_core/src/compute/operation_configs.rs:947-1101`; registry at
`crates/rs_cam_core/src/compute/catalog.rs:2083-2113`). Its regions are even
labelled with band + strategy and narrated as a mix table
(`crates/rs_cam_core/src/narrate.rs:860-865`, `append_region_mix` at `:865`),
and the planner-vs-generator distinction is a first-class span role
(`RegionSpanRole::{Node, GeneratorPass, …}` —
`crates/rs_cam_core/src/toolpath_spans.rs:326-379`).

**But the codebase carries an explicit measured warning against bolting one
more internal pass onto it.** `pencil_claims`'s own doc
(`operation_configs.rs:988-1005`): the crease node cost **+22.5% finish time
for zero measured quality gain**, and a region-level territory filter
("S2") "was tried and measured ineffective on real terrain". It ships
`false` by default (`operation_configs.rs:1133-1135`).

### 1.d Nothing marks auto-generated ops as belonging together

`ToolpathConfig` (`crates/rs_cam_core/src/session/mod.rs:661-697`) has no
`parent_id`, no `group_id`, no `origin`/provenance field. The GUI toolpath
panel is a **flat list grouped by Setup only**, one level deep, no nesting,
no multi-select (`crates/rs_cam_viz/src/ui/toolpath_panel.rs:52-107`; the
only `CollapsingHeader` in the file is the Tool Library at `:180`). Reorder /
duplicate / move-to-setup / drag-drop are all single-op events
(`crates/rs_cam_viz/src/ui/mod.rs:161-169`, dispatch at
`crates/rs_cam_viz/src/controller/events/mod.rs:103-112`, drop-index at
`toolpath_panel.rs:630-645`).

So if a planner emits k tier ops, the operator can freely delete tier 2 or
drag tier 3 above tier 1 and **nothing detects the inconsistency**. That is a
real cost of the op-chain design and must be planned for (§5).

### 1.e The op-chain UX today — measured, and already known to be bad in the GUI

`StockSource` has two variants only, `Fresh` and `FromRemainingStock`
(`crates/rs_cam_core/src/compute/config.rs:3-12`). The dependency is
**implicit in plan order** — there is no "depends on op N" pointer.
`AwaitingPriorStock` is a sequencing state, not an error
(`compute/config.rs:20-47`, `ComputeStatus` at `:57-59`); generation reads
the source at `crates/rs_cam_core/src/session/compute.rs:1476` with a
"never silently fall back to fresh stock" rule at `:1369-1382`. Setter:
`crates/rs_cam_core/src/session/mutation.rs:443`, with the downstream
invalidation cascade at `:165-260`.

GUI controls: a generic "Use remaining stock" checkbox
(`crates/rs_cam_viz/src/ui/properties/mod.rs:3751-3767`, hidden for Pencil at
`:3746` because two controls silently disagreed — rationale at `:3741-3748`)
and Pencil's own segmented "Rest reference" control
(`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:526-548`).
Neither says which upstream op supplies the stock. The nearest signal is the
Rest-op dependency badge (`toolpath_panel.rs:708-762`).

**The fixpoint that makes a k-op chain one click is MCP-only.** Core
`generate_all` is a single pass with no loop
(`crates/rs_cam_core/src/session/compute.rs:2222-2252`); GUI
`handle_generate_all` submits every id once and stops
(`crates/rs_cam_viz/src/controller/events/toolpath.rs:339-350`). The loop
lives behind `#[cfg(feature = "mcp")]`: `mcp_start_generate_all`
(`crates/rs_cam_viz/src/controller/events/compute.rs:1387-1493`, chain bound
at `:1405-1414`, resolution refusal at `:1436-1450`),
`settle_generate_all_round` (`:1498-1567`),
`resume_generate_all_after_simulation` (`:1571-1616`); termination argument
at `crates/rs_cam_viz/src/mcp_bridge.rs:1157-1171`.

The GUI's own block message spells out the manual procedure —
`crates/rs_cam_viz/src/controller/events/compute.rs:142-157` — and points the
operator at MCP. For k tiers that is: add k ops, assign k tools, tick k
checkboxes, verify order, then **up to k rounds of simulate → Generate All**,
because the phantom prior-stock scan unlocks only the FIRST pending op per
round (`crates/rs_cam_core/src/compute/simulate.rs:146-205`, driven from
`session/compute.rs:2290-2312`, pinned by
`phantom_prior_stock_ladder_unlocks_only_first_pending_op` at
`session/compute.rs:5054-5085`).

**This is the single biggest UX finding of T3: the op-chain design is only
acceptable if the ladder generator ALSO gives the GUI a one-click path.**

### 1.f How choices are presented today — three precedents, ranked

1. **Feeds Suggest — the real "propose / veto / record provenance" precedent.**
   Per-value Suggest buttons (`crates/rs_cam_viz/src/ui/components/suggest.rs`),
   the Feeds modal (`crates/rs_cam_viz/src/ui/feeds_modal.rs`), the MCP
   `apply_feeds` + `get_suggest_rationale`
   (`crates/rs_cam_viz/src/mcp_server.rs:526+`), and — critically — a
   per-dimension provenance stamp on the config
   (`ToolpathConfig.feeds_provenance`, `session/mod.rs:692-697`; written on
   every manual set at `session/compute.rs:312-315`, `:322-325`, `:332-335`).
   This is the only place where "where did this number come from" survives
   into the project file.

2. **Optimize project (U3) — the multi-row propose/select/apply precedent.**
   An expensive search is run on its own compute lane, cached in
   `AppState::optimize_project`, rendered as a table with a **per-row
   checkbox**, and applied with one "Apply selected" button:
   `crates/rs_cam_viz/src/ui/optimize_project.rs:222-236` (button + enablement
   at `:223-227`), row toggle event `ToggleOptimizeProjectRow`
   (`crates/rs_cam_viz/src/controller/events/mod.rs:281-287`), apply handler
   `apply_optimize_project` (`events/mod.rs:1156-1201+`, which skips
   unselected rows at `:1174-1176`). The per-toolpath sibling is
   `crates/rs_cam_viz/src/ui/optimize_modal.rs:1-9` (expensive outcome
   stashed, not recomputed per frame) with `ApplyOptimizeCandidate` at
   `optimize_modal.rs:858`. **This is the closest UI shape to "planner
   proposes k tiers, operator unticks the ones it doesn't want".** Note it
   only mutates params on *existing* ops — it never creates or deletes one.

3. **The strategy advisor — advisory only, MCP only, NO apply path.**
   `ProjectSession::recommend_clearing_strategy`
   (`crates/rs_cam_core/src/session/compute.rs:694-698`) clones the op and
   overrides the strategy on the clone (`:726-730`) — it never touches stored
   config. Returns `StrategyRecommendation { chosen, regime, reason, ranked,
   time_ratio_vs_runner_up }`
   (`crates/rs_cam_core/src/strategy_advisor.rs:88-101`). **Zero hits for
   `recommend_clearing_strategy` / `StrategyRecommendation` /
   `strategy_advisor` anywhere in `crates/rs_cam_viz/src/ui`, `src/controller`
   or `src/state`** — no button, no panel. Exposed only as an MCP tool
   (`crates/rs_cam_viz/src/mcp_server.rs:511-524`, handler
   `crates/rs_cam_viz/src/app/mcp.rs:1371-1380`), whose description warns
   "HEAVY: … ~30-60 s and a GUI freeze". Acting on it means separately
   calling `set_toolpath_param`; nothing links advice to mutation and nothing
   records that a value came from the advisor. **Do not copy this pattern.**

### 1.g MCP authoring surface

`add_toolpath` takes `setup_index, operation_type, tool_index, model_id,
name` — **no param map**
(`crates/rs_cam_viz/src/mcp_server.rs:982-1007`; handler
`crates/rs_cam_viz/src/app/mcp.rs:3263-3390`). Params come from
`suggest_params` at creation (`app/mcp.rs:3302-3324`, rationale at
`:3297-3299`) and `stock_source` is hard-wired `Fresh` (`app/mcp.rs:3352`).
So an agent authoring a k-tier ladder makes roughly `3k` calls
(`add_toolpath` + `set_stock_source` + n× `set_toolpath_param`).

`get_operation_schema` is **derived from the registry**, not hand-written:
`mcp_server.rs:472-488` → `app/mcp.rs:1185-1191` →
`ProjectSession::operation_schema` (`session/compute.rs:561-580`) →
`OperationConfig::schema_for_type` (`catalog.rs:961-984`), which merges
`new_default(op).params_value_including_nulls()` with
`param_defs_for_type(op)` (`catalog.rs:2259-2261`) and
`tool_constraints_for_type` (`:2263-2265`). Sync is enforced by tests, not
types: `operation_schema_params_match_params_with_nulls_for_every_op`
(`session/compute.rs:4619`) and
`add_toolpath_description_lists_every_operation_it_accepts`
(`crates/rs_cam_viz/tests/mcp_authoring_surface.rs:222-231`) — the latter
because `add_toolpath`'s description is hand-written prose
(`mcp_server.rs:985`).

**Param plumbing is nearly free.** `set_toolpath_param` handles four common
params explicitly and falls through to a **serde round-trip** for everything
else (`session/compute.rs:259-264` doc, fallthrough at `:400-542`, with a
registry range refusal at `:487-504` and an "unknown param" verification at
`:519-540`). So a new config field is settable over MCP and round-trips
through the project TOML the moment it is added — but it needs a `ParamDef`
row for the schema-parity test, and a **hand-written egui widget** for the
GUI (§4).

---

## 2. Island PREVIEW before generation

### 2.a What can draw a mask today

Everything in the 3D viewport goes through one bundle + one draw pass:
`ViewportCallback` (`crates/rs_cam_viz/src/render/mod.rs:727-761`, flags only)
over `RenderResources` (`render/mod.rs:126-211`, the geometry), uploaded by
`RsCamApp::upload_gpu_data` (`crates/rs_cam_viz/src/app/gpu_upload.rs:153`),
with flags computed at `crates/rs_cam_viz/src/app/viewport.rs:434-509` and
operator toggles in the "Show ▼" menu
(`crates/rs_cam_viz/src/ui/viewport_overlay.rs:112-210`, state at
`crates/rs_cam_viz/src/state/viewport.rs:55,71,88`).

| Overlay | Renders from | Available pre-generation? |
|---|---|---|
| **Rest-depth heatmap** (per-cell scalar grid draped on the surface) | `rest_grid_to_heatmap_mesh(&RestGrid) -> Option<StockMesh>` — `crates/rs_cam_core/src/rest_heatmap_mesh.rs:52`; upload `gpu_upload.rs:1222-1230`; draw `render/mod.rs:985-995` (own opacity uniform, `render/mod.rs:151-153`) | **No** — source is `result.annotated.rest_grid` (`gpu_upload.rs:1200-1219`), i.e. post-generation. **But the builder takes a plain raster**: `RestGrid { nx, ny, origin_x, origin_y, cell_mm, rest: Vec<f32>, surface_z: Vec<f32>, threshold }` (`crates/rs_cam_core/src/rest_field.rs:225-239`), with NaN = "outside/untrusted" (skipped at `rest_heatmap_mesh.rs:113-115`) — **exactly the semantics an island mask needs** |
| Height planes (5 translucent quads) | `HeightPlanesGpuData::from_heights` (`crates/rs_cam_viz/src/render/height_planes.rs:24`) from `tc.heights.resolve(...)` — **config, not result** (`gpu_upload.rs:1143-1165`), gated `Workspace::Toolpaths && Selection::Toolpath(_)` (`app/viewport.rs:463-464`) | **YES — the best UX precedent** for a per-op, config-derived, selection-gated overlay |
| DXF/SVG polygons + drill-target markers | `PolygonGpuData` (`render/mod.rs:120`), built at `gpu_upload.rs:327-500`, drawn `render/mod.rs:1000-1010`; draped at stock-top Z + 0.05 (`gpu_upload.rs:432-440`) | **YES** — the existing "arbitrary 2D rings over the part" path, already setup-transform-aware |
| Fixtures / keep-outs / alignment pins | `gpu_upload.rs:546-620`, `:621-635` | **YES** — pure config |
| Engagement colouring | `from_toolpath_engagement` (`crates/rs_cam_viz/src/render/toolpath_render.rs:414`) | No |
| Chipload / advance-per-tooth heat | `from_toolpath_advance_per_tooth` (`toolpath_render.rs:590`) + cut trace (`gpu_upload.rs:1072-1090`) | No — needs generation **and** simulation |
| Collision markers | `gpu_upload.rs:776-884` | No |
| Hotspot markers | **removed** — F6.1 reverted, rationale at `app/viewport.rs:552-563` ("danger everywhere") | — |

**No machining-boundary / `ToolContainment` visualization exists at all.**
`BoundaryConfig` / `BoundarySource` / containment are config-and-persistence
only in viz (`crates/rs_cam_viz/src/state/toolpath/entry.rs:29-31,139-140`;
`crates/rs_cam_viz/src/io/project.rs:241-256,1187-1199`; MCP param at
`crates/rs_cam_viz/src/mcp_bridge.rs:657-661`) — nothing in `src/render/` or
`gpu_upload.rs` draws one. A territory-island overlay would be the **first
thing in the viewport that shows where an operation is allowed to cut**.

### 2.b There is no image/texture panel — do not plan on one

- `egui::ColorImage` appears twice, both **capture** not display
  (`crates/rs_cam_viz/src/app.rs:465`;
  `crates/rs_cam_viz/src/app/mcp.rs:4768`).
- Zero hits in `crates/` for `TextureHandle`, `load_texture`, `egui::Image`,
  `ui.image`, `egui_extras`.
- The only `wgpu::Texture`s are the offscreen colour/depth targets and the
  blit sampler (`render/mod.rs:676-699`, shader `:1276-1297`) — **no
  textured-quad pipeline**.
- The `image` crate is a **write-only PNG encoder** in all three crates
  (`crates/rs_cam_core/Cargo.toml:32,49`, `crates/rs_cam_cli/Cargo.toml:20`,
  `crates/rs_cam_viz/Cargo.toml:33`).

A 2D mask panel would need either a new textured-quad pipeline or an
`egui::Painter` rect loop — the rest-heatmap legend
(`crates/rs_cam_viz/src/ui/viewport_overlay.rs:310`, ramp shared with the 3D
mesh via `rest_ramp_color` at `crates/rs_cam_core/src/rest_heatmap_mesh.rs:141`)
is the template if that route is taken.

### 2.c The raster/PGM lineage — all test-only

The `arp1_*.pgm` artifacts referenced in the investigation prompt were written
by a **test**: `render_the_plate_before_any_verdict_is_read` —
`crates/rs_cam_core/tests/reference_plate_contract.rs:1119` (header/body write
`:1165-1177`, output dir resolution `:1124-1126`), whose doc at `:1104-1118`
states the standing rule *"never gate on an aggregate without rendering the
surface"*. A second, richer PGM renderer is `render_pgm` at
`crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs:1092` (header
`:1146`). PGM was chosen because it needs no dependency. **Neither is
reachable from CLI, MCP or GUI — one-off in-test diagnostics, not tooling.**

### 2.d The ready-made tier-map renderer nobody calls

`planned_regions_to_svg(&PlannedRegions, width, height) -> String` —
`crates/rs_cam_core/src/finish_planner.rs:978`. It renders the planner's
territory decomposition top-down: per-`FinishBand` filled evenodd polygons
(palette `finish_planner.rs:958-964`), crease own-regions, crease
centrelines, and a legend (`:1064-1082`). It consumes `PlannedRegions`
(`:311`) from `decompose_surface` (`:570`) — **plan-time data, produced
before any toolpath is emitted**.

**It is shipped `src/` code with NO shipped caller.** Callers are the unit
test at `finish_planner.rs:1908-1935` and
`crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs:176,203`. The
underlying mask→polygon extraction is `region_polygons_from_mask` /
`_clamped` (`crates/rs_cam_core/src/region_mask.rs:49,88`), whose module doc
(`:1-8`) says it is already shared by rest analysis and the unified-finish
planner.

**This is the cheapest honest preview in the repo.** It needs a caller, not a
renderer.

### 2.e The 6-view composite and the screenshot surfaces

Composite renderer, all in `crates/rs_cam_core/src/fingerprint.rs`:
`render_stock_composite` (`:658`) / `_in_frame` (`:677`),
`render_toolpath_composite` (`:703`) / `_in_frame` (`:725`),
`render_mesh_composite` (`:759`) / `_in_frame` (`:797` — the core: one shared
camera, 6 supersampled panels, box downsample, captions),
`save_mesh_composite_png` (`:771`), layout `CompositePanel` (`:905`) /
`composite_panel_layout` (`:923`). It is a **software rasterizer** — no GPU,
no window — and it renders **any per-vertex-coloured `StockMesh`** (dexel
stock is just one adapter, `fingerprint.rs:684`).

Entry points: `crates/rs_cam_core/tests/param_sweep.rs:161-168` (sweeps),
`crates/rs_cam_cli/src/sweep.rs:350-356` (CLI `sweep` subcommand,
`crates/rs_cam_cli/src/main.rs:125`), and MCP (below). Conventions are
sentried by `crates/rs_cam_core/tests/composite_render_convention.rs:32-33,124,168`.

**It could render a mask with no renderer change**, because
`rest_grid_to_heatmap_mesh` already produces the `StockMesh` shape from a 2D
grid, and `render_toolpath_composite_in_frame`'s dimmed-background trick
(`fingerprint.rs:747-753`, `StockMesh::with_dimmed_colors(0.35)` + `append`)
is the ready-made way to composite a mask **over** the part.

MCP surfaces: `screenshot_simulation` (declared
`crates/rs_cam_viz/src/mcp_server.rs:1459-1483`, implemented
`crates/rs_cam_viz/src/app/mcp.rs:4452-4560`) and `screenshot_toolpath`
(declared `mcp_server.rs:1485-1512`, implemented `app/mcp.rs:4562-...`). Both
are **CPU rasterisers**, not framebuffer captures — stated at
`crates/rs_cam_viz/src/mcp_bridge.rs:1081-1082,1128-1129`. The `path`
extension is the mode switch: `.png` → composite (`app/mcp.rs:4465-4527`),
anything else → interactive Three.js HTML (`:4528-4556`). **Both refuse
without a generated result / simulation** (`app/mcp.rs:4462-4464`,
`:4583-4586`) — **there is no MCP surface today that renders anything at plan
time.**

### 2.f What produced `p2_a1_lakes_vbit_chk4.html` — and why it is a warning

Shipped tooling: `rs_cam_core::viz::stock_mesh_to_3d_html(mesh, toolpaths,
title)` — `crates/rs_cam_core/src/viz.rs:1988` (the `#info` format string at
`:2020-2038` matches the file byte-for-byte; importmap at `:123-140`;
`const stockVerts = new Float32Array([...])` at `:2056`), called from
`mcp_screenshot_simulation`'s non-`.png` branch
(`crates/rs_cam_viz/src/app/mcp.rs:4545-4552`, title
`"{} -- Simulation"` at `:4549`). Siblings: `toolpath_to_3d_html`
(`viz.rs:373`), `toolpath_standalone_3d_html` (`:487`), `simulation_3d_html`
(`:694`), `stacked_simulation_3d_html` (`:1351`).

**The file is 948,300,086 bytes** (verified on disk) — 10.7 M vertices
inlined as decimal text. Do **not** put a tier-map preview down this path.

---

## 3. Per-tier diagnostics

Everything actionable in this codebase is keyed by `toolpath_id`. If tiers
are separate ops, all of it comes free; if tiers are internal to one op,
none of it does.

**What is per-toolpath today:**

| Surface | Evidence | Key |
|---|---|---|
| Load gates (chipload / power / deflection) | `ToolpathLoadVerdict` — `crates/rs_cam_core/src/tool_load/verdict.rs:202-237` | `toolpath_id` (`:203`) |
| Gate population | `ToolpathLoadContext` — `crates/rs_cam_core/src/tool_load/mod.rs:369-399`, one **tool** at `:372`; `evaluate_project` maps 1:1 (`:529-541`) | per toolpath |
| Op-family-specific gates | `drill_gates: Option<DrillGatesVerdict>` — `verdict.rs:207-215`; struct `crates/rs_cam_core/src/tool_load/drill_gates.rs:89-114` | per toolpath, `None` for milling |
| Report row | `ToolpathDiagnostic` — `crates/rs_cam_core/src/session/mod.rs:922-975` (`op_kind`, `collision_count`, `rapid_collision_count`, `truncated_core_mm2`, `untouched_material_mm2`, `reached_uncut_estimate_mm2`, `unmachined_band_area_mm2`, `tip_float_points`, `max_tip_float_mm`) | `toolpath_id` (`:923`) |
| Air-cut threshold | `OperationType::air_cut_high_threshold_pct` — `crates/rs_cam_core/src/compute/catalog.rs:473-512` (45.0 for the 3D finish family at `:504-505`) | per **operation type** |
| Sim metrics | `SimulationCutTrace::{toolpath_summaries, drill_summaries, toolpath_runtimes}` — `crates/rs_cam_core/src/simulation_cut.rs:679,700,~710`; `metrics_not_applicable` at `:461` | `toolpath_id` |
| Narration | `ProjectSession::narrate_toolpath(index)` — `crates/rs_cam_core/src/session/compute.rs:3145`; core `narrate_toolpath_with_context` — `crates/rs_cam_core/src/narrate.rs:419` | per toolpath |
| GUI panels | "Verification" op list (`crates/rs_cam_viz/src/ui/sim_op_list.rs:17-27`) and "Inspector" (`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:19-29`), both built on `ToolpathLoadVerdict` rows with `drill_gates` fan-out | per toolpath |

**The argument FOR separate tier ops (strong):**

1. **Gates need one tool.** `ToolpathLoadContext.tool` is a single
   `&ToolDefinition` (`tool_load/mod.rs:372`). Three tiers in one op means
   the chipload/deflection verdicts are computed against one of the three —
   and CLAUDE.md already records the class of defect that produces: a gate
   whose observation and threshold describe different things reads `Within`
   on a grossly overloaded cutter.
2. **A gate handed an empty population passes and looks healthy** (measured
   2026-08-05; `sample_range 0..0`, `available_kw 0.0`, indistinguishable
   from a clean cut). Splitting per tier keeps each population attributable;
   merging them makes "which tier was this measured on?" unanswerable.
3. **Air-cut thresholds are per operation type**
   (`catalog.rs:473-512`), and a coarse R2.0 flats tier and a fine R0.5
   valleys tier have genuinely different expected air fractions. One op = one
   threshold applied to a blended number.
4. **`drill_gates` is the shipped precedent for op-family-specific gates**
   (`verdict.rs:207-215` — additive `Option` field, existing `criteria()`
   consumers unchanged, GUI fans it out at
   `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:12-16`). If tier ops later
   need a tier-specific gate ("did this tier's territory actually get
   cleared?"), that is the pattern to copy — and it is per-toolpath.
5. **Feeds provenance is per op** (`session/mod.rs:692-697`). Per-tier feeds
   with a recorded origin are only expressible as separate ops.
6. **G-code correctness.** One phase per op, tool change on phase boundary
   (`crates/rs_cam_core/src/gcode/mod.rs:134-152`). Separate tier ops emit
   correct `M6`s with no emitter change.

**The argument AGAINST (honest):**

1. Narration would be k reports instead of one; there is no cross-op rollup
   that says "the ladder as a whole cost X and left Y". `ProjectDiagnostics`
   rolls to the project, not to an arbitrary subset.
2. There is **no group identity** to hang a per-ladder report on
   (`session/mod.rs:661-697`, §1.d).
3. UnifiedFinish already narrates its internal regions with a band+strategy
   mix table (`crates/rs_cam_core/src/narrate.rs:860-865`) and a first-class
   planner-vs-generator span role
   (`crates/rs_cam_core/src/toolpath_spans.rs:326-379`), so the *reporting*
   half of a one-op design is not hopeless — only the *gating* half is.

**Verdict: the diagnostics surface argues decisively for separate tier ops.**
The report-level downside (no ladder rollup) is additive work on a surface
that already exists; the gate-level downside of one op is a class of silent
pass this repo has been burned by repeatedly.

---

## 4. Where per-op unified-finish params live — the touch-point list

### 4.a Config struct (source of truth for serde + TOML + MCP)

`UnifiedFinishConfig` — `crates/rs_cam_core/src/compute/operation_configs.rs:954-1101`:

| Field | Line | GUI widget? |
|---|---|---|
| `steep_threshold_deg` | `:958` | yes |
| `waterline_threshold_deg` | `:961` | yes |
| `overlap_mm` | `:964` | yes |
| `scallop_height` | `:965` | yes |
| `tolerance` | `:966` | yes |
| `raster_stepover` | `:967` | yes |
| `z_step` | `:968` | yes |
| `sampling` | `:969` | yes |
| `stock_to_leave` | `:983` (vertical-offset doc `:970-982`) | yes |
| `feed_rate` / `plunge_rate` / `spindle_rpm` | `:984-987` | Feeds tab |
| `pencil_claims` | `:1005` | yes (claims block) |
| `min_rest_depth_mm` | `:1016` | yes (claims block) |
| `claims_reference` | `:1036` | yes (claims block) |
| `territory_clip` | `:1055` | yes (claims block) |
| `intra_region_hookup_mm` | `:1073` | **NO** |
| `crease_hookup_mm` | `:1082` | **NO** |
| `classification_sampler` | `:1100` | **NO** (and no `ParamDef` either) |

Defaults: `impl Default` at `:1103-1131`; per-field default fns at
`:1133-1180+`.

### 4.b Registry / schema

- `ParamDef` struct — `crates/rs_cam_core/src/compute/catalog.rs:1142-1151`
  (`name`, `type_name`, `optional`, `description`, `range: Option<ParamRange>`;
  `ParamRange::describe` at `:1106`, `to_json` at `:1125`).
- `UNIFIED_FINISH_PARAMS` — `catalog.rs:1602-1645`. Note
  `classification_sampler` is absent, and `stock_to_leave` is the one entry
  carrying a `required_desc` prose block (`:1611-1620`) — the template for
  documenting a new dial on the MCP wire.
- `REG_UNIFIED_FINISH` — `catalog.rs:2083-2113` (`param_defs` at `:2096`,
  ball/tapered-ball tool constraint at `:2097-2100`,
  `DressupPolicy::strip_all` at `:2109-2111`, generator at `:2112`).
- Schema derivation — `OperationConfig::schema_for_type`, `catalog.rs:961-984`;
  `param_defs_for_type` `:2259-2261`. Parity test
  `operation_schema_params_match_params_with_nulls_for_every_op` —
  `crates/rs_cam_core/src/session/compute.rs:4619`.

### 4.c Setter

`ProjectSession::set_toolpath_param` — `crates/rs_cam_core/src/session/compute.rs:268`.
Four common params handled explicitly (`:306-345`), everything else via a
serde round-trip (`:400-542`), with a registry-range refusal at `:487-504` and
an unknown-param verification re-serialize at `:519-540`. **Adding a config
field + a `ParamDef` row gives MCP and TOML support with no setter change.**

### 4.d GUI panel bindings

- Panel: `draw_unified_finish_params` —
  `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:861-928`
  (the 9-dial grid `:868-926`, claims block call `:927`).
- Claims block: `draw_unified_finish_claims` — `surface_3d.rs:704-859`
  (`pencil_claims` checkbox `:717`, `claims_reference` combo `:726-770`,
  `territory_clip` checkbox `:771`, `min_rest_depth_mm` `:782`, resolved-
  reference readout `:795-830`, self-probe warning `:844-858`).
- Dispatch: `crates/rs_cam_viz/src/ui/properties/mod.rs:3928-3944` — the
  `OperationConfig::UnifiedFinish` arm, which also threads
  `resolved_claims_reference` and `stock_source_for_claims` in (rationale at
  `:3930-3937`).
- Stock-source checkbox (shared, all ops): `properties/mod.rs:3751-3767`.
- Rest-analysis config type: `RestAnalysisConfig` —
  `crates/rs_cam_core/src/compute/config.rs:1520-1559` (`enabled`,
  `reference_tool_id`, `cell_mm`, `min_valley_depth`, `region_margin_mm`,
  `offset_stepover_mm`, `num_offset_passes`) — **already 80% of the shape a
  per-tier territory dial needs**, including a "which tool's geometry is the
  reference" field.

### 4.e The planner dials the operator CANNOT reach today — the key finding for island filtering

The operator's asked-for dials ("min island 50 mm², merge radius 5 mm")
**already exist**, as `FinishPlannerParams` —
`crates/rs_cam_core/src/finish_planner.rs:109-178`:

| Dial | Line | Default | Exposed to operator? |
|---|---|---|---|
| `steep_threshold_deg` | `:112` | 45.0 | yes (via cfg) |
| `waterline_threshold_deg` | `:126` | 75.0 | yes (via cfg) |
| `hysteresis_deg` | `:129` | 10.0 | **NO** — and the doc at `:202-203` says "hysteresis is load-bearing (0 → the raw masks storm to O(100) islands)" |
| `overlap_mm` | `:133` | 0.0 | yes (via cfg) |
| `crease_own_region_half_width_mm` | `:162` | `2.0 × cusp_radius` | **NO** |
| `pencil_claim_floor` | `:170` | `cusp_radius × 0.25` | **NO** |
| `close_radius_mm` (**merge radius**) | `:173` | `cusp_radius × 0.5` | **NO** |
| `min_region_area_mm2` (**min island**) | `:177` | `(2 × cusp_radius)² × 4` | **NO** |

Derivation: `FinishPlannerParams::for_tool(cusp_radius)` — `:211-222`, with a
long doc at `:187-210` warning that these are **feature scales** and must use
the **cusp (tip) radius**, not `radius()` (a Ø1 tip on a 6 mm shank made
`min_region_area_mm2` 144 mm² instead of 4 and "closed and absorbed every
steep ribbon").

**The exact override site** is
`crates/rs_cam_core/src/compute/execute.rs:2138-2142`:
`for_tool(ctx.tool_def.cusp_radius_mm())` followed by exactly three
assignments (`steep_threshold_deg`, `waterline_threshold_deg`, `overlap_mm`),
consumed at `:2243`. **Adding `min_island_area_mm2` / `merge_radius_mm` /
`hysteresis_deg` to `UnifiedFinishConfig` is four more lines here plus a
`ParamDef` row plus a widget — the plumbing already exists.**

`decompose`'s signature is the other seam:
`decompose(&SlopeMap, covered: &[bool], &[RestCenterline], &FinishPlannerParams)`
— `finish_planner.rs:352-357`. **It already takes an external boolean mask
(`covered`), row-major `rows*cols`** — a tier mask is exactly that shape.
`decompose_surface` (`:570-584`) is the convenience wrapper that supplies the
surface's own coverage.

---

## 5. UX recommendation

### 5.a Ladder expression — **auto-generated op chain, one op per tier**

**Recommendation: a planner that emits k `UnifiedFinish` ops (plus the
existing pencil), one per tool tier, each with `stock_source =
FromRemainingStock` after the first, each carrying its tier's territory as
an operation input.**

Why, in order of weight:

1. **One op = one tool is load-bearing in four independent subsystems**
   (gates `tool_load/mod.rs:372`, G-code `gcode/mod.rs:134-152`, feeds
   provenance `session/mod.rs:696`, stock chaining `session/mod.rs:685`).
   A one-op ladder breaks all four; an op chain breaks none.
2. **The diagnostics case is decisive** (§3): per-tier gates, per-tier
   air-cut thresholds, per-tier narration and per-tier sim metrics all come
   free, and a merged population is exactly the shape of the silent-pass
   defects this repo keeps finding.
3. **The one-op alternative has a measured warning attached.**
   `pencil_claims` bolted one extra internal pass onto UnifiedFinish and cost
   +22.5% time for zero quality gain (`operation_configs.rs:988-1005`); the
   region-level "S2" filter was tried and measured dead. That is the exact
   design shape a one-op ladder would repeat, three times over.
4. **Precedent exists for app-owned ops** — `AlignmentPinDrill`'s
   `SystemOnly` category (`catalog.rs:175-177`) plus the reconciler
   (`crates/rs_cam_viz/src/controller/events/model.rs:645-790`).

**The two costs the plan MUST budget for, because they are real:**

- **(C1) No group identity.** `ToolpathConfig` has no `group_id` / `origin`
  (`session/mod.rs:661-697`), and the panel is flat with no nesting
  (`toolpath_panel.rs:52-107`). Emit k ops and the operator can delete tier 2
  or reorder tier 3 above tier 1 with no consistency check. The pin-drill
  reconciler dodges this only because it assumes at most one such op
  (`model.rs:653-659`). **Recommendation:** add an optional
  `ladder: Option<LadderMembership { ladder_id, tier_index }>` to
  `ToolpathConfig`, modelled on how `feeds_provenance` (`session/mod.rs:696`)
  records where a value came from — it is the only existing field on that
  struct whose job is provenance, and it round-trips through project IO
  already. Even if the first cut only *displays* the badge and warns on
  reorder, having the field prevents the "indistinguishable from hand-made"
  failure.
- **(C2) The GUI has no ladder generate.** The fixpoint that resolves a k-op
  rest chain in one call is `#[cfg(feature = "mcp")]`
  (`crates/rs_cam_viz/src/controller/events/compute.rs:1387-1493`); the GUI
  path is a single pass (`events/toolpath.rs:339-350`) and the app's own
  block message tells the operator to run simulate→generate up to k times
  (`events/compute.rs:142-157`). **Shipping a k-tier planner without also
  wiring the GUI to the fixpoint would make the feature strictly worse to use
  in the GUI than in MCP.** Budget this as a first-class phase, not a
  follow-up.

**Where the ladder itself is expressed:** as a **project-level planner
invocation**, not as a param on an operation. Concretely, a "Plan multi-tool
finish" action that takes `tools: Vec<ToolId>` (ordered coarse→fine),
`tolerance_mm`, `min_island_area_mm2`, `merge_radius_mm` and emits the tier
map + k proposed ops. Putting the ladder on an op config would mean an op
whose params describe *other* ops — a shape with no precedent here, and one
that `set_toolpath_param`'s serde round-trip (`session/compute.rs:400-542`)
would happily let an agent corrupt.

### 5.b Preview mechanism — **two surfaces, staged**

**Stage 1 (ship first, cheapest honest preview): reuse the rest-heatmap slot
for a plan-time tier mask.**

The tier map is per-cell data over the part footprint. `RestGrid`
(`crates/rs_cam_core/src/rest_field.rs:225-239`) is already exactly that
shape, NaN already means "outside/untrusted"
(`rest_heatmap_mesh.rs:113-115`), and the whole render path already exists:
`rest_grid_to_heatmap_mesh` (`rest_heatmap_mesh.rs:52`) →
`SimMeshGpuData::from_heightmap_mesh`
(`crates/rs_cam_viz/src/render/sim_render.rs:149`) → `rest_heatmap_data`
(`gpu_upload.rs:1222-1230`) → the dedicated depth-read-only draw with its own
opacity uniform (`render/mod.rs:985-995`). **The only structural change is
the source**: today it is `result.annotated.rest_grid`, post-generation
(`gpu_upload.rs:1200-1219`). Copy the **height-planes gating shape** —
config-derived, selection-gated, no result required
(`app/viewport.rs:463-464` + `gpu_upload.rs:1143-1165`) — and colour by tier
index instead of rest depth (the ramp fn is
`rest_heatmap_mesh.rs:141`, and the legend template is
`crates/rs_cam_viz/src/ui/viewport_overlay.rs:310`). Add a "Show ▼" toggle
alongside the existing ones (`ui/viewport_overlay.rs:112-210`,
`state/viewport.rs:55,71,88`).

This gives the operator "R2 territory vs R0.5 territory" **on the part, in
the viewport, before generation**, which is exactly the veto affordance
asked for. It would also be the first overlay in the app that shows where an
operation is allowed to cut (§2.a).

**Stage 2 (agent-visible / headless, near-free): a plan-time render tool.**

Two options, both cheap:

- `planned_regions_to_svg` (`crates/rs_cam_core/src/finish_planner.rs:978`)
  is a finished top-down band/territory map renderer with **no shipped
  caller** — it needs a caller, not a renderer. Colour-per-tier instead of
  colour-per-band is a palette swap (`finish_planner.rs:958-964`).
- `render_mesh_composite_in_frame` (`crates/rs_cam_core/src/fingerprint.rs:797`)
  accepts any per-vertex-coloured `StockMesh` and needs **no change** —
  `rest_grid_to_heatmap_mesh` produces one, and
  `StockMesh::with_dimmed_colors(0.35)` + `append`
  (`fingerprint.rs:747-753`) is the ready-made way to composite the mask over
  a dimmed part.

Either way the MCP surface should be a thin sibling of
`mcp_screenshot_toolpath` (`crates/rs_cam_viz/src/app/mcp.rs:4562`) **minus
its "must be generated" guard** (`:4583-4586`) — there is no plan-time render
tool today, and both existing screenshot tools refuse without a result.

**Explicitly NOT recommended:**

- **An HTML artifact** like `p2_a1_lakes_vbit_chk4.html`. That file is
  948,300,086 bytes because `stock_mesh_to_3d_html`
  (`crates/rs_cam_core/src/viz.rs:1988`, verts at `:2056`) inlines the dexel
  mesh as decimal text. A mask overlay pushed down this path only inflates it.
- **An egui image panel.** No texture-display capability exists anywhere in
  viz (§2.b) — it would mean a new textured-quad pipeline.
- **A PGM dump.** Both PGM writers are test-only
  (`crates/rs_cam_core/tests/reference_plate_contract.rs:1119`,
  `crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs:1092`) and
  neither is operator-reachable.

**The veto UI shape** should copy Optimize-project (U3), not the strategy
advisor: an expensive plan run on its own lane, cached in `AppState`, shown
as a per-tier table with a **per-row checkbox** and one "Apply selected"
button — `crates/rs_cam_viz/src/ui/optimize_project.rs:222-236`,
`ToggleOptimizeProjectRow` (`controller/events/mod.rs:281-287`),
`apply_optimize_project` (`controller/events/mod.rs:1156-1201+`, unselected
rows skipped at `:1174-1176`); per-item modal precedent
`crates/rs_cam_viz/src/ui/optimize_modal.rs:1-9` ("the modal does not
recompute the outcome on every frame"). The strategy advisor is the
**anti**-pattern: no GUI surface at all, no apply path, no provenance
(§1.f.3).

The plan lane itself can ride `AnalysisRequest`
(`crates/rs_cam_viz/src/compute/worker.rs:377`, currently
`Simulation | Collision`, submitted via `submit_analysis` at `:734`) or copy
Optimize's session-ownership pattern (`crates/rs_cam_viz/src/compute/mod.rs:15-23`).

### 5.c Param / panel touch-point list

Ordered by dependency. Every row is a concrete file:line.

**Core — config + schema (adds MCP + TOML support automatically):**

1. `crates/rs_cam_core/src/compute/operation_configs.rs:954-1101` — add
   island-filter fields to `UnifiedFinishConfig`
   (`min_island_area_mm2`, `merge_radius_mm`, `hysteresis_deg`, and the tier
   inputs). Serde-default them (`:1004`, `:1015`, `:1054` are the templates)
   so existing project files load unchanged.
2. `crates/rs_cam_core/src/compute/operation_configs.rs:1103-1131` — extend
   `impl Default`; add per-field default fns beside `:1133-1180`.
3. `crates/rs_cam_core/src/compute/catalog.rs:1602-1645` — add `ParamDef`
   rows to `UNIFIED_FINISH_PARAMS`. Use `required_desc` (template at
   `:1611-1620`) — these dials carry the cusp-radius footgun and need the
   prose on the wire. **Required** by
   `operation_schema_params_match_params_with_nulls_for_every_op`
   (`crates/rs_cam_core/src/session/compute.rs:4619`).
4. *(optional)* `crates/rs_cam_core/src/compute/catalog.rs:1142-1151` +
   `ParamRange` — declare domains so `set_toolpath_param` refuses out-of-range
   values (`session/compute.rs:487-504`) rather than clamping.

**Core — planner plumbing:**

5. `crates/rs_cam_core/src/compute/execute.rs:2138-2142` — the
   `FinishPlannerParams::for_tool(...)` + three-assignment block. Add the new
   overrides here; consumed at `:2243`.
6. `crates/rs_cam_core/src/finish_planner.rs:109-178` — `FinishPlannerParams`
   already holds `close_radius_mm` (`:173`), `min_region_area_mm2` (`:177`),
   `hysteresis_deg` (`:129`). **No new field needed** — only an override path.
   Respect the cusp-vs-envelope rule at `:187-210`.
7. `crates/rs_cam_core/src/finish_planner.rs:352-357` — `decompose`'s
   `covered: &[bool]` is the external-mask seam; the tier mask is exactly
   this shape (row-major `rows*cols`). Mask→polygon helper already shared:
   `crates/rs_cam_core/src/region_mask.rs:49,88`.

**Core — tier reference geometry (reuse, don't reinvent):**

8. `crates/rs_cam_core/src/compute/config.rs:1520-1559` —
   `RestAnalysisConfig` already carries `reference_tool_id`, `cell_mm`,
   `min_valley_depth`, `region_margin_mm`. Model the tier dial on it.

**Session — op creation:**

9. `crates/rs_cam_core/src/session/mutation.rs:63-85` — `add_toolpath`;
   call k times. Note it appends to `setup.toolpath_indices` (`:79`), so
   tiers land in emission order; `reorder_toolpath` (`:137`) exists if a
   specific slot is needed.
10. `crates/rs_cam_core/src/session/mutation.rs:443` — `set_stock_source` for
    tiers 2..k; downstream invalidation cascade at `:165-260`.
11. `crates/rs_cam_core/src/session/mod.rs:661-697` — **(C1)** the place a
    `ladder`/group field would go.

**Viz — panel:**

12. `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:861-928` —
    `draw_unified_finish_params`; add the island-filter dials to the grid
    (`dv(...)` helper usage at `:872-925`).
13. `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:704-859` —
    `draw_unified_finish_claims`; the "Rest Claims" block is the right home
    for territory dials, and its resolved-reference readout (`:795-830`) is
    the template for showing what the planner actually decided.
14. `crates/rs_cam_viz/src/ui/properties/mod.rs:3928-3944` — the dispatch arm;
    thread any extra read-only context in the same way
    `resolved_claims_reference` / `stock_source_for_claims` are.
    **Note the existing gap:** `intra_region_hookup_mm`, `crease_hookup_mm`
    and `classification_sampler` have **no GUI widget at all** (zero hits in
    `crates/rs_cam_viz/src/`) — adding a config field does *not* get you a
    dial.

**Viz — planner UI + preview:**

15. `crates/rs_cam_viz/src/ui/optimize_project.rs:222-236` +
    `crates/rs_cam_viz/src/controller/events/mod.rs:281-290,1156-1201` — copy
    this shape for the tier-review table (per-row checkbox + Apply selected).
16. `crates/rs_cam_viz/src/app/gpu_upload.rs:1195-1230` — the rest-heatmap
    source; add the plan-time mask branch.
17. `crates/rs_cam_viz/src/app/viewport.rs:463-464` +
    `crates/rs_cam_viz/src/app/gpu_upload.rs:1143-1165` — the height-planes
    gating shape to copy (config-derived, selection-gated, pre-generation).
18. `crates/rs_cam_viz/src/ui/viewport_overlay.rs:112-210` (Show ▼ menu) and
    `crates/rs_cam_viz/src/state/viewport.rs:55,71,88` (toggle state);
    legend template at `ui/viewport_overlay.rs:310`.
19. `crates/rs_cam_viz/src/compute/worker.rs:377` — `AnalysisRequest`, where a
    `TierPlan` variant would go (`submit_analysis` at `:734`).
20. **(C2)** `crates/rs_cam_viz/src/controller/events/compute.rs:1387-1616` —
    the MCP-only fixpoint; the GUI Generate All at
    `crates/rs_cam_viz/src/controller/events/toolpath.rs:339-350` is what
    needs to reach it. Block message to retire:
    `controller/events/compute.rs:142-157`.

**Viz — MCP:**

21. `crates/rs_cam_viz/src/mcp_server.rs:1485-1512` +
    `crates/rs_cam_viz/src/app/mcp.rs:4562` — template for a plan-time render
    tool, minus the generated-result guard at `app/mcp.rs:4583-4586`.
22. `crates/rs_cam_viz/src/mcp_server.rs:985` — `add_toolpath`'s hand-written
    description, kept honest by
    `crates/rs_cam_viz/tests/mcp_authoring_surface.rs:222-231`. If a new op
    type is introduced (rather than reusing `UnifiedFinish`), this prose must
    be edited by hand.

**Project IO:**

23. `crates/rs_cam_viz/src/io/project.rs:241-256,1187-1199` — where
    per-toolpath config (incl. boundary) round-trips; any new
    `ToolpathConfig` field (C1) needs a row here. CLAUDE.md's standing rule
    applies: *if GUI state adds a field, audit setup-sheet, project-IO and
    test initializers.*
