# Viewport interaction redesign — phase 1 inventory

**Status:** Phase 1 evidence. Docs only. No source file changed, no cargo command ran.
**Date:** 2026-09-23. **Base commit:** `5c04b774`.
**Companion file:** `MOCKUPS.md` in this folder.

## 0. How to read this file

- Every claim about today's code carries a `file:line`. Paths are relative to
  `crates/rs_cam_viz/` unless they start with `crates/` or a root file name.
- Four files had uncommitted peer edits when this inventory was written:
  `src/app.rs`, `src/ui/sim_timeline.rs`, `src/ui/sim_op_list.rs` and
  `tests/the_simulation_page_is_summary_first_dc6.rs`. For those four files,
  the line numbers are from `HEAD` (`5c04b774`). All other files were clean
  against `HEAD`, and their line numbers are from the working tree.
- "Does not exist today" means that the code does not make the distinction.
  This file does not describe the plan's intent as a fact.

## 1. The registry

`src/ui/overlays/registry.rs` holds one `const ROWS: &[OverlayRow]`
(`registry.rs:394-1235`). It has **39 rows** in four groups
(`OverlayGroup::ALL`, `registry.rs:61-66`):

| Group | Rows |
|---|---|
| Geometry | 12 |
| Toolpath | 10 |
| Regions | 5 |
| Analysis | 12 |
| **Total** | **39** |

### 1.1 The row shape

`OverlayRow` (`registry.rs:193-219`) has these fields:

- `id` — the stable id. It is also the MCP wire name (`registry.rs:194-196`).
- `group`, `label`, `hover`.
- `surface` — `None`, `Model`, `Stock` or `Moves` (`registry.rs:76-86`).
  Exclusivity applies per surface, and only on `Model`, `Stock` and `Moves`
  (`OverlaySurface::EXCLUSIVE`, `registry.rs:90`).
- `mechanism` — `DrawTime` or `UploadTime(trigger)` (`registry.rs:104-110`).
  Every upload-time row names `UPLOAD_DETECTOR` (`registry.rs:116`).
- `flag` — the state path that the row drives, or `None` when no renderer
  exists (`registry.rs:201-205`).
- `radio` — `true` for a member of a radio surface whose "off" has no meaning
  (`registry.rs:206-209`).
- `get`, `set`, `precondition`, `default_for` — function pointers.

### 1.2 The availability model today

`Precondition` has **two** arms only (`registry.rs:150-156`):

- `Ready`.
- `Disabled { reason: String, compute: Option<OverlayAction> }`.

`OverlayAction` has five values (`registry.rs:120-134`). Their labels are at
`registry.rs:137-145`:

| `OverlayAction` | Label | What the panel does (`ui/overlays/panel.rs:267-297`) |
|---|---|---|
| `RunCollisionCheck` | `Run collision check` | pushes `AppEvent::RunCollisionCheck` |
| `OpenPlanner` | `Plan…` | opens the multi-tool planner |
| `GenerateAll` | `Generate all` | pushes `AppEvent::GenerateAll` |
| `RecordGeneratorTrace` | `Record & re-generate` | switches trace capture on, then generates all |
| `EnableRestAnalysis` | `Compute rest` | applies `AutoEnableRestAnalysis`, switches the row on, generates the selected toolpath |

The plan names seven availability states. This table maps each one to the
code today:

| Plan state | Code today | Where |
|---|---|---|
| Ready/off | `Precondition::Ready` and `(row.get)(state) == false` | `panel.rs:220-240` |
| Showing | `Precondition::Ready` and `get == true`. This is the **flag**, not the rendered result. Section 2.3 lists where the two differ. | `panel.rs:220-240` |
| Needs compute | `Disabled { compute: Some(_) }`. The panel draws the reason line and one small button. | `panel.rs:243-259` |
| Computing | **Does not exist today** as a row state. One row (`reach_map`) has a computing phase, but its precondition reports `Ready` while the map computes. | `registry.rs:359-379` |
| Blocked | `Disabled { compute: None }`. The panel draws the reason line only. | `panel.rs:243-251` |
| Stale | **Does not exist today** in the registry. No precondition reads freshness. | — |
| Failed | **Does not exist today** in the registry. `ReachStatus::Failed` exists (`state/runtime.rs:82`), but the reach row reports it as a Blocked reason with the wrong text. | `registry.rs:367-379`, `registry.rs:961` |

The panel shows a disabled row with its flag value. A disabled row with the
flag on reads as a ticked, greyed checkbox (`panel.rs:228-240`).

### 1.3 Column key for the tables below

- **Default S/T/Sim** — the value that `default_for` gives for Setup,
  Toolpaths and Simulation. `–` means "no default; the flag keeps its value".
  Readiness is `–` for every row. The helpers are at `registry.rs:224-302`.
- **Initial** — the value in the constructed state
  (`state/viewport.rs:211-241`; for the simulation fields,
  `state/simulation/playback_state.rs:67-75`).
- **Excl.** — the exclusive surface, or `—` for a stacking row.
- **Mech.** — `D` for draw-time, `U` for upload-time.
- **States today** — the plan states that the row can reach today:
  `R` Ready/off, `S` Showing (flag on and Ready), `N` Needs compute,
  `C` Computing, `B` Blocked, `St` Stale, `F` Failed.
- The MCP name is the `id` in every row. `apply_overlays`
  (`registry.rs:1334-1380`) looks up the id with `registry::row`.

### 1.4 Geometry (12 rows)

| id (line) | Label | Flag | Excl. | Mech. | Default S/T/Sim | Initial | Drawable when / blocked reason | States today |
|---|---|---|---|---|---|---|---|---|
| `grid` (397) | Grid | `viewport.show_grid` | — | D | on/on/off | on | always | R S |
| `model` (411) | Model | `viewport.show_model` | — | D | –/–/– | on | Blocked: "replaced by the simulated stock here" (in Simulation); "this project carries no 3D model" | R S B |
| `stock_box` (435) | Stock — box | `viewport.show_stock` | — | D | on/on/off | on | always | R S |
| `stock_solid` (452) | Stock — solid | `viewport.show_stock_solid` | — | D | on/off/off | off | always | R S |
| `origin_axes` (467) | Origin axes | `viewport.show_origin_axes` | — | D | on/on/off | on | always | R S |
| `datum` (482) | Datum crosshair | `viewport.show_datum` | — | U | on/on/off | on | Blocked: "no setup in this project"; "no datum set on this setup" | R S B |
| `fixtures` (505) | Fixtures | `viewport.show_fixtures` | — | U | on/on/off | on | Blocked: "no fixture in this setup"; "no setup in this project" | R S B |
| `keep_outs` (524) | Keep-out zones | `viewport.show_keep_outs` | — | U | on/on/off | on | Blocked: "no keep-out zone in this setup"; "no setup in this project" | R S B |
| `alignment_pins` (542) | Alignment pins | `viewport.show_alignment_pins` | — | U | on/on/on | on | Blocked: "no alignment pin on this stock" | R S B |
| `flip_axis` (562) | Flip axis | `viewport.show_flip_axis` | — | U | on/on/off | on | Blocked: "no flip axis set on this stock" | R S B |
| `curves` (582) | Curves (DXF/SVG) | `viewport.show_polygons` | — | D | –/–/– | on | Blocked: "this project carries no 2D curves" | R S B |
| `orientation_gizmo` (602) | Orientation gizmo | `viewport.show_orientation_gizmo` | — | D | –/–/– | on | always | R S |

### 1.5 Toolpath (10 rows)

| id (line) | Label | Flag | Excl. | Mech. | Default S/T/Sim | Initial | Drawable when / blocked reason | States today |
|---|---|---|---|---|---|---|---|---|
| `all_toolpaths` (617) | All toolpaths | `viewport.show_all_toolpaths` | — | U | –/–/off | off | Needs compute: "no toolpath generated yet" + `Generate all` | R S N |
| `cutting_moves` (640) | Cutting moves | `viewport.show_cutting` | — | D | off/on/off | on | Needs compute: "no toolpath generated yet" + `Generate all` | R S N |
| `rapids` (663) | Rapids | `viewport.show_rapids` | — | D | off/on/off | on | Needs compute: "no toolpath generated yet" + `Generate all` | R S N |
| `entry_markers` (686) | Entry markers | `viewport.show_entry_markers` | — | D | off/on/off | on | Blocked: "select a toolpath to see its entry moves" | R S B |
| `height_planes` (707) | Height planes | `viewport.show_height_planes` | — | D | off/on/off | on | Blocked: "select a toolpath to see its Z planes" | R S B |
| `tool_profile_ghost` (727) | Tool-profile ghost | `viewport.show_tool_profile_preview` | — | U | –/–/– | off | Blocked: "select a toolpath to see its cutter ghost" | R S B |
| `span_entry` (747) | Spans: Entry | `viewport.span_kind_filter.show_entry` | — | U | –/–/– | on | Blocked: "applies in Palette move colour only"; Needs compute: "no toolpath generated yet" + `Generate all` (`registry.rs:1241-1255`) | R S N B |
| `span_lead_out` (761) | Spans: LeadOut | `viewport.span_kind_filter.show_lead_out` | — | U | –/–/– | on | as `span_entry` | R S N B |
| `span_link_bridge` (775) | Spans: LinkBridge | `viewport.span_kind_filter.show_link_bridge` | — | U | –/–/– | on | as `span_entry` | R S N B |
| `span_dressup` (789) | Spans: DressupArtifact | `viewport.span_kind_filter.show_dressup` | — | U | –/–/– | on | as `span_entry` | R S N B |

The panel adds one fixed note under this group: "Per-toolpath visibility is
on each operation row: eye, C, R, and the bullseye for isolation."
(`panel.rs:143-152`). UR5 retired isolation (`tests/viewport_draws_selected_only_wp27.rs:237-319`),
so the words "the bullseye for isolation" are stale today.

### 1.6 Regions (5 rows)

| id (line) | Label | Flag | Excl. | Mech. | Default S/T/Sim | Initial | Drawable when / blocked reason | States today |
|---|---|---|---|---|---|---|---|---|
| `rest_heatmap` (806) | Rest heatmap | `viewport.show_rest_heatmap` | Model | D | off/off/off | off | Blocked: "the rest heatmap draws in the Toolpaths workspace"; "select a toolpath that carries a rest grid". Needs compute: "this toolpath has no rest grid yet" + `Compute rest` | R S N B |
| `tier_map` (840) | Tier map | `viewport.show_tier_preview` | — | D | –/–/– | off | Blocked: "the tier map draws in the Toolpaths workspace"; "previewed on setup N — switch setup to see it". Needs compute: "run a preview from Toolpath ▸ Plan multi-tool finishing…" + `Plan…` | R S N B |
| `planner_islands` (878) | Planner islands | `None` | — | D | –/–/– | off | Blocked always: "drawn by Regions ▸ Tier map — no separate island outline renderer yet" | B |
| `derived_rest_regions` (903) | Derived rest regions | `None` | — | D | –/–/– | off | Blocked always: "not drawn yet (no renderer)" | B |
| `boundary_outline` (919) | Boundary outline | `None` | — | D | –/–/– | off | Blocked always: "not drawn yet (no renderer)" | B |

### 1.7 Analysis (12 rows)

| id (line) | Label | Flag | Excl. | Mech. | Default S/T/Sim | Initial | Drawable when / blocked reason | States today |
|---|---|---|---|---|---|---|---|---|
| `reach_map` (936) | Model colour: Reach | `viewport.show_reach_map` | Model | D | off/on/off | on | Blocked: "the reach map draws in the Toolpaths workspace"; "the reach map IS the model, re-coloured — switch Geometry ▸ Model on"; "select a finishing operation". Ready while the map computes. | R S B, C (hidden as R/S), F (shown as B) |
| `stock_colour_solid` (975) | Stock colour: Solid | `simulation.stock_viz_mode.solid` | Stock (radio) | U | –/–/– | on | Blocked: "run a simulation" | R S B |
| `stock_colour_deviation` (993) | Stock colour: Deviation | `simulation.stock_viz_mode.deviation` | Stock (radio) | U | –/–/– | off | Blocked: "run a simulation"; "no deviation data — run a simulation" | R S B |
| `stock_colour_by_height` (1021) | Stock colour: By height | `simulation.stock_viz_mode.by_height` | Stock (radio) | U | –/–/– | off | Blocked: "run a simulation" | R S B |
| `move_colour_palette` (1039) | Move colour: Palette | `viewport.toolpath_color_mode.normal` | Moves (radio) | U | –/–/– | on | Needs compute: "no toolpath generated yet" + `Generate all` | R S N |
| `move_colour_engagement` (1058) | Move colour: Engagement | `viewport.toolpath_color_mode.engagement` | Moves (radio) | U | –/–/– | off | as Palette | R S N |
| `move_colour_advance_per_tooth` (1082) | Move colour: Advance / tooth | `viewport.toolpath_color_mode.advance_per_tooth` | Moves (radio) | U | –/–/– | off | as Palette; Blocked: "advance per tooth needs a simulation" | R S N B |
| `simulated_stock` (1117) | Simulated stock | `viewport.show_sim_stock` | — | D | off/off/on | off | Blocked: "the simulated stock draws in the Simulation workspace"; "run a simulation" | R S B |
| `collisions` (1139) | Collisions | `viewport.show_collisions` | — | D | off/off/on | off | Needs compute: "no collision check has run" + `Run collision check` | R S N |
| `tool_deflection` (1164) | Tool deflection | `viewport.show_tool_deflection` | — | D | off/off/on | off | Blocked: "the deflection panel draws in the Simulation workspace"; "run a simulation" | R S B |
| `generator_steps` (1187) | Generator steps | `simulation.debug.enabled` | — | D | –/–/– | off | Needs compute: "switch on Record generator trace and regenerate" + `Record & re-generate` | R S N |
| `active_step_highlight` (1211) | Highlight active step | `simulation.debug.highlight_active_item` | — | D | –/–/– | on | Needs compute: as `generator_steps`; Blocked: "switch Generator steps on first" | R S N B |

### 1.8 Controls that are not registry rows

The dock and the catalogue must keep these, because the registry does not list
them:

| Control | Where today | Note |
|---|---|---|
| Stock opacity slider | `panel.rs:188-209`, under `simulated_stock` | Writes `simulation.stock_opacity` (`state/simulation.rs:847`). The hover says the solid block and the height planes stay at 0.15. |
| Per-operation eye / C / R | `ui/toolpath_row_controls.rs`; state `viewport.toolpath_move_visibility` (`state/viewport.rs:171-177`) | Deliberately not a registry row: the panel is per-scene. AND-ed with the global `show_cutting` / `show_rapids`. |
| View presets Top / Front / Right / Iso, Reset view | `ui/viewport_overlay.rs:39-63`; also `ui/menu_bar.rs:250-273` and keys 1-4 (`app/input.rs:492-518`) | Camera, not a registry row. |
| Projection Perspective / Orthographic | `ui/viewport_overlay.rs:66-95` | The strip is the ONLY GUI constructor of `UiCommand::ToggleProjection`. |
| Draw scope button (Selected only / All toolpaths) | `ui/viewport_overlay.rs:111-128` | A second writer of `show_all_toolpaths`, through `UiCommand::ToggleShowAllToolpaths`. The handler calls `registry::set_overlay` (`controller/events/mod.rs:500-502`). The strip is the ONLY GUI constructor of that command. |
| Compute activity label + `Cancel All` | `ui/viewport_overlay.rs:131-149` | The ONLY GUI constructor of `UiCommand::CancelCompute`. |
| Simulation `Reset` | `ui/viewport_overlay.rs:166-170` | Also in `ui/menu_bar.rs:238`. |
| `,` / `.` surface cycle | `app/input.rs:583-596`; constants `panel.rs:538-540` | `COMMA_SURFACE = Model`, `PERIOD_SURFACE = Stock`. The constants live in `panel.rs`. |

## 2. State definitions

### 2.1 Where each toggle lives

| State | Struct and field | File |
|---|---|---|
| 24 `show_*` booleans | `ViewportState` | `state/viewport.rs:57-181` (fields 58-168) |
| Move colour mode | `ViewportState::toolpath_color_mode: ToolpathColorMode` | `state/viewport.rs:170`, enum `state/viewport.rs:185-203` |
| Span filter (4 booleans) | `ViewportState::span_kind_filter: SpanKindFilter` | `state/viewport.rs:13-18`, `state/viewport.rs:180` |
| Per-op cut / rapid | `ViewportState::toolpath_move_visibility` | `state/viewport.rs:177` |
| Stock colour mode | `SimulationState::stock_viz_mode: StockVizMode` | `state/simulation.rs:845`, enum `state/simulation.rs:467-474` |
| Stock opacity | `SimulationState::stock_opacity` | `state/simulation.rs:847` |
| Generator steps / highlight | `SimulationDebugState::{enabled, highlight_active_item}` | `state/simulation.rs:84-90` |
| Panel open / pinned / group expansion | `OverlayPanelState::{open, pinned, groups}` | `state/overlays.rs:34-42` |
| Workspace default bookkeeping | `OverlayPanelState::{defaults_applied_for, displaced}` | `state/overlays.rs:52-54`; read by `registry.rs:1391-1428` |
| Reach map life cycle | `GuiState::reach_overlay: ReachOverlayState` with `ReachStatus::{Idle, Computing, Ready, Failed}` | `state/runtime.rs:78-83`, `state/runtime.rs:92-145`, `state/runtime.rs:304` |
| Draw set (WP27) | derived, `ToolpathDrawFilter::from_state` + `toolpaths_to_draw` | `state/viewport.rs:256-295` |

### 2.2 Preferred analysis versus rendered result

The plan asks for two values per analysis: the operator's preferred mode and
the result that is on screen now.

**Today, one boolean per row holds the preference.** The rendered result is
not stored. `app/viewport.rs:455-521` derives it each frame into
`ViewportCallback`. For example:

- `show_reach_overlay = show_reach_map && workspace == Toolpaths && show_model && reach_map_ready` (`app/viewport.rs:513-516`).
- `show_rest_heatmap = show_rest_heatmap && workspace == Toolpaths && selected_rest_grid_info.is_some()` (`app/viewport.rs:488-490`).
- `show_sim_mesh = show_sim_stock && workspace == Simulation && has_results()` (`app/viewport.rs:520-522`).

No surface reads the derived value back. The panel shows the flag; the legend
list (`registry.rs:1512-1546`) shows "flag on and precondition Ready". So the
distinction **does not exist today** as data that a surface can show.

Selection changes the target and keeps the flag. This part already matches the
plan: no code path clears `show_reach_map` on a selection change. The reach
scheduler follows the selection (`controller.rs:640-700`).

### 2.3 Where the rendered result differs from the flag today

Each item is a place where the operator sees a state that the screen does not
show.

1. **Reach computing reads as Ready.** `reach_map_wanted` counts `Computing` as
   drawable (`registry.rs:367-378`). The draw gate needs a `Ready` map
   (`app/viewport.rs:513-516`). While the walk runs, the row is a ticked,
   enabled checkbox, the plain model draws, and no legend shows
   (`registry.rs:1522-1526`). Only the inspector prints "reach: computing…"
   (`ui/properties/toolpath_panel.rs:319-331`, the label at 327). The comment at
   `state/runtime.rs:118-133` records that an unrelated edit re-submits the
   walk, so the overlay "blinks off".
2. **Reach failure reads as "select a finishing operation".** The controller
   writes `ReachStatus::Failed(message)` (`controller/events/compute.rs:1379`).
   `reach_map_wanted` matches only `Ready | Computing` (`registry.rs:372-375`),
   so a failed walk falls to the Blocked reason at `registry.rs:961`. Only the
   inspector carries the failure (`ui/properties/mod.rs:519-521`).
3. **Deviation without data paints the solid colour.** The precondition blocks
   a switch ON (`registry.rs:1006-1016`). It does not move the mode when the
   data goes. `compute_sim_colors` then paints `[0.65, 0.45, 0.25]` for
   `StockVizMode::Deviation` with `display_deviations == None`
   (`app/gpu_upload.rs:74-86`). The row reads "Deviation", the stock looks
   Solid, and no legend shows. **This is a mode that silently substitutes
   another.** That literal also differs from `STOCK_SOLID_FACE = [0.65, 0.50, 0.30]`
   (`render/colors.rs:38`).
4. **Advance per tooth without a simulation paints grey.** The mode stays
   `AdvancePerTooth` after the simulation clears. The builder receives no
   band and no samples, so every cut is `[0.40, 0.40, 0.40]` or
   `[0.25, 0.25, 0.30]` (`render/toolpath_render.rs:755-762`). The row is
   Blocked, and no legend shows.
5. **Cutting moves Ready with nothing drawn.** `cutting_moves` and `rapids` are
   Ready when any toolpath is generated (`registry.rs:649-658`). With no
   selection and `show_all_toolpaths == false`, `toolpaths_to_draw` returns
   nothing (`state/viewport.rs:283-295`). The row is ticked and nothing draws.
6. **A second writer bypasses exclusivity.** The inspector's "Show reach map"
   checkbox (`ui/properties/toolpath_panel.rs:305`) writes
   `state.viewport.show_reach_map` directly (`ui/properties/mod.rs:1209`,
   `ui/properties/mod.rs:1239`). It does not call `registry::set_overlay`, so it
   does not clear `show_rest_heatmap`. The renderer can then draw both: reach
   replaces the model (`render/mod.rs:1084-1098`), and the rest heatmap drapes
   over it (`render/mod.rs:1152-1160`). The exclusivity tests exercise
   `set_overlay` only (`tests/overlays_registry.rs:493-511`). This finding is
   from code reading; no test ran.
7. **Rest heatmap on a stale result.** `rest_grid_info` reads the selected
   toolpath's `rt.result` with no freshness check (`registry.rs:331-348`). The
   toolpath lines of an `EditedSince` toolpath are dimmed
   (`app/viewport.rs:536-547`); the rest heatmap is not. Stale **does not exist
   today** for the heatmap.
8. **Simulation overlays on a stale simulation.** `simulated_stock`,
   `stock_colour_*`, `collisions` and `tool_deflection` test
   `has_results()` only. `AppState::simulation_is_stale` exists
   (`state/CLAUDE.md`, invariant 3), but no registry precondition reads it.
9. **Compute rest in flight reads as Needs compute.** After `Compute rest`, the
   row stays "this toolpath has no rest grid yet" with the same button until the
   generation lands (`registry.rs:818-824`). The same holds for
   `Run collision check` (`registry.rs:1148-1157`). Computing **does not exist
   today** for these rows.

### 2.4 Compute versus Compute & show

The plan asks for two distinct outcomes. **This distinction does not exist
today.** `OverlayAction::EnableRestAnalysis` does three things in one click
(`panel.rs:289-295`):

1. It applies `Command::AutoEnableRestAnalysis` through
   `ui::properties::apply_auto_enable` (`ui/properties/mod.rs:1325-1342`).
2. It switches the row on with `registry::set_overlay`.
3. It pushes `AppEvent::GenerateToolpath(id)`.

When the core answers `simulation_cleared`, the call sets
`panel_side_effects.invalidate_simulation` (`ui/properties/mod.rs:1338-1340`).
No confirm step names that consequence before the work starts.

## 3. Colour encodings in scope for the legend audit

"Legend today" means a legend in the Overlays panel legend block
(`panel.rs:159-165`, `panel.rs:344-516`) unless the row says otherwise. The
panel shows that block only while the panel is open.

| Encoding | Where the colours are chosen | Legend today | Where the legend draws | Audit note |
|---|---|---|---|---|
| Toolpath palette | `render/colors.rs:9-24` (8 colours by config index); Z-depth and pass blend `render/toolpath_render.rs:180-224`; selected brighten `render/toolpath_render.rs:200-207` | **None** | — | The index is the config order (`state/viewport.rs:276-281`). |
| Cut (Palette mode) | the palette colour, blended by Z (`render/toolpath_render.rs:210-224`) | **None** | — | Hover says "Green fed moves" (`registry.rs:660`). The lines are the toolpath's palette colour, not green. |
| Rapid | Palette mode: `base * 0.35` (`render/toolpath_render.rs:227`). Engagement and advance modes: fixed `[0.15, 0.15, 0.2]` (`render/toolpath_render.rs:471`, `render/toolpath_render.rs:628`) | **None** | — | Hover says "Orange rapid moves" (`registry.rs:683`). Three rapid colours exist; none is orange. |
| Span kinds (Palette mode) | entry cyan, lead-out magenta, link bridge grey, dressup tint (`render/toolpath_render.rs:228-235`) | **None** | — | The four `span_*` rows filter these; no swatch names them. |
| Entry markers / entry preview | `ENTRY_PREVIEW_COLOR` (`render/toolpath_render.rs:839`) | **None** | — | Hover names "Cyan markers" (`registry.rs:703`). |
| Height planes | `render/colors.rs:62-66` (5 colours) | **None** | — | The toolpath inspector has its own height diagram legend (`ui/properties/operations/height_diagram.rs:144-145`); it is not a viewport legend. |
| Collision | density ramp, yellow to red, literal in `app/gpu_upload.rs:959-962` | **None** | — | `COLLISION_POINT` (`render/colors.rs:41`) has no reader in `src/`. It is a dead constant. |
| Territory: tier map | `tier_fill_color` and `tier_overlap_color` (`crates/rs_cam_core/src/maps/rest_heatmap_mesh.rs:187-229`) | Yes: swatches per fine tier | `panel.rs:491-514`; also the planner dialog `ui/multitool_planner.rs:717-730` | The panel legend omits the overlap-band tint. The planner legend has it (`ui/multitool_planner.rs:851`). |
| Engagement | `engagement_color` (`render/toolpath_render.rs:818-836`) | Yes: gradient "Load heavy" to "light" | `panel.rs:459-464` | No unit. The ramp input is `feed / nominal`. Hover says "by feed rate" (`registry.rs:1078-1079`). |
| Advance per tooth | `advance_per_tooth_segment_color` (`render/toolpath_render.rs:755-781`) | Yes: 5 class swatches | `panel.rs:465-490` | Classes only, no mm/tooth numbers, by design (`panel.rs:468-471`). |
| Stock deviation | `deviation_colors` (`render/sim_render.rs:117-140`) | Yes: gradient "Over-cut −1 mm" to "+1 mm remaining" | `panel.rs:438-444` | Silent Solid substitution without data (section 2.3, item 3). |
| Stock height | `height_gradient_colors` (`crates/rs_cam_core/src/export/ribbon.rs:46`) | Yes: gradient "Height low" to "high" | `panel.rs:445-458` | No unit. The ramp normalises over the mesh Z range. |
| Stock solid | mesh wood tone; fallback `[0.65, 0.45, 0.25]` (`app/gpu_upload.rs:59-72`) | **None** | — | Not an analysis colour. It needs no legend. |
| Reach | `reach_color` (`crates/rs_cam_core/src/maps/reach_map.rs:800`) | Yes: log gradient + tolerance line + measured line + grid note | `panel.rs:357-437` | Also carries "moves dimmed while reach map is on" (`panel.rs:390-396`). The inspector repeats the numbers (`ui/properties/toolpath_panel.rs:300-380`). |
| Rest | `rest_ramp_color` (`crates/rs_cam_core/src/maps/rest_heatmap_mesh.rs:156-169`) | Yes: gradient "Rest {threshold} mm" to "{peak} mm" | `panel.rs:346-356` | **Scale mismatch.** The mesh normalises to the p95 of rest values (`rest_heatmap_mesh.rs:82-114`; `p95` at 90, the ramp call at 114). The legend normalises to the maximum (`registry.rs:340-345`). The red end of the legend names a larger number than the red end of the mesh. |

The legend list is `registry::active_legends` (`registry.rs:1512-1546`). It has
seven `Legend` arms (`registry.rs:1490-1508`). A legend shows only when the
flag is on AND the precondition is Ready. So a Computing reach map, a
data-less Deviation and a data-less Advance per tooth all show no legend.

## 4. The 320 pt viewport-width guard

- `pub const MIN_VIEWPORT_WIDTH: f32 = 320.0` (`ui/overlays/panel.rs:35`). The
  doc comment (`panel.rs:22-34`) records the 2026-09-08 `screenshot_gui`
  capture that came back with no 3D viewport.
- `draw_docked` refuses to dock when
  `ui.available_width() - PANEL_WIDTH < MIN_VIEWPORT_WIDTH` (`panel.rs:50`).
  `PANEL_WIDTH` is 232 (`panel.rs:20`). On refusal the caller draws the
  floating window instead (`app/viewport.rs:242`, `app/viewport.rs:261`).
- `draw_viewport` floors the 3D view allocation at
  `free.x.max(MIN_VIEWPORT_WIDTH)` (`app/viewport.rs:248-256`). So the 3D view
  is never narrower than 320 pt, even when the layout has less room.
- `SIDE_PANEL_MAX_WIDTH = 420.0` (HEAD `src/app.rs:109`) caps each workspace
  side panel. The doc comment (HEAD `src/app.rs:100-108`) says that this
  ceiling and the 320 pt floor are deliberately separate.
- The guard is about the dock column only. The top strip
  (`ui/viewport_overlay.rs:37`) uses `horizontal_wrapped`, so it wraps at any
  width and takes height from the 3D view.

## 5. Blast radius for phase 2

This list is for the phase-2 editor. Each entry gives the file, the lines, the
test name, and the exact assertion text that a change would break.

**Warning:** one entry (5.1.9) is already red on the working tree because of a
peer's uncommitted edit, not because of phase 2. Read 5.1.9 before you run the
`overlays_registry` sentry.

### 5.1 `tests/overlays_registry.rs`

1. `every_viewport_flag_has_exactly_one_registry_row` (105-156). Scans
   `pub struct ViewportState {` and `pub struct SpanKindFilter {` in
   `src/state/viewport.rs`. Breaks if phase 2 adds a `pub …: bool` field to
   `ViewportState` without a row. Texts: `` "`{head}` no longer exists in state/viewport.rs" ``;
   `"the field scan found only {} flags — the parser has drifted from state/viewport.rs, so this whole test is vacuous"` (needs > 20);
   `` "`{flag}` is driven by {} registry rows ({rows:?}); it must be exactly one …" ``;
   `` "row `{}` names `{flag}`, which state/viewport.rs no longer declares" ``.
   **Consequence:** any new dock state (open section, preferred mode) must not
   be a `pub bool` in `ViewportState`. Put it in `OverlayPanelState` or a new
   struct.
2. `every_registry_id_is_unique_and_wire_safe` (158-179). Texts:
   `` "duplicate registry id `{}` — the id is the MCP wire name" ``;
   `` "id `{}` is not a stable snake_case wire name" ``;
   `` "row `{}` has no label" ``; `` "row `{}` has no hover text" ``.
3. `a_flagless_row_can_never_be_switched_on` (183-216). Pins the flagless set
   to exactly `["planner_islands", "derived_rest_regions", "boundary_outline"]`.
   Text: `"the flagless set changed — a new one needs a renderer or a reason"`.
4. `every_upload_time_row_is_carried_in_the_upload_key` (225-277). Reads
   `pub(crate) fn overlay_upload_key` in `src/app.rs`. Texts:
   `"app.rs no longer builds an `overlay_upload_key`"`;
   `"only {upload_rows} upload-time rows found — the mechanism labels have drifted and this test is vacuous"` (needs ≥ 12).
5. `every_disabled_reason_is_a_non_empty_sentence` (281-309). Needs more than 20
   disabled rows on an empty project across four workspaces. Text:
   `"only {disabled_seen} disabled rows across four workspaces on an empty project — the preconditions are not being exercised"`.
   **Consequence:** a phase-2 split of `Precondition` into more arms must keep
   `Disabled`-like rows countable here, or this test changes.
6. `the_two_hard_gates_are_disabled_rows_with_reasons` (313-341). Needs the
   `model` reason in Simulation to contain `"simulated stock"`, and the
   `tier_map` reason to contain `"preview"`.
7. MCP arms (345-434): `set_ui_view_refuses_an_unknown_overlay_id` (reason
   contains the id); `set_ui_view_refuses_a_not_ready_id_with_the_panels_own_reason`
   (`"the MCP refusal must quote the string the panel prints, or the two surfaces disagree about why"`);
   `switching_an_overlay_off_is_applied_even_when_it_cannot_draw` (asserts
   `show_reach_map` is on initially and not Ready on an empty project);
   `switching_a_colour_choice_off_is_refused` (reason contains `"moves"`);
   `every_overlay_is_refused_in_the_readiness_workspace` (reason contains
   `"no viewport"`).
8. Exclusivity and default arms (445-761). Eleven tests, including `the_moves_family_is_on_in_toolpaths_and_off_elsewhere` (726-761). They read `ROWS`,
   `set_overlay`, `switch_workspace` and `non_default_count` only. They break
   only if phase 2 changes those functions. `the_constructed_state_equals_the_toolpaths_default_column`
   asserts `registry::non_default_count(&state) == 0` (568).
9. `the_viewport_keeps_a_minimum_width` (828-870). **Already red on the
   working tree.** It asserts `caps == 6` for `.max_size(SIDE_PANEL_MAX_WIDTH)`
   in `src/app.rs` (836-841, text: `"every left/right workspace panel must carry the ceiling — setup, toolpaths and simulation, two each; found {caps}"`).
   The peer's uncommitted `side_panel()` wrapper leaves one occurrence in the
   working tree. This break is not phase 2's. The other asserts in this test
   ARE phase 2's:
   - `panel.contains("pub const MIN_VIEWPORT_WIDTH")` — `"the viewport floor is gone"`.
   - `panel.contains("if ui.available_width() - PANEL_WIDTH < MIN_VIEWPORT_WIDTH")` — `"the docked Overlays column no longer refuses to dock when it would take the 3D view under its floor"`.
   - `!viewport.contains("ui.allocate_exact_size(ui.available_size()")`.
   - `viewport.contains("MIN_VIEWPORT_WIDTH")` — `"the viewport no longer floors its own allocation"`.
   - `viewport.contains("draw_floating(ui, state, events, rect, docked)")` — `"the floating fallback no longer knows whether the dock refused"`.
   All five read `src/ui/overlays/panel.rs` or `src/app/viewport.rs`. Removing
   the docked column breaks the second and fifth.
10. `the_retired_controls_have_no_second_home` (669-703). Reads
    `src/ui/viewport_overlay.rs`:
    - `!toolbar.contains("\"Show ▼\"")`.
    - `!toolbar.contains("RenderMode")`.
    - `toolbar.contains("panel::toolbar_button")` — `"the toolbar no longer opens the Overlays panel"`.
    - `toolbar.contains("overlay_collision_check")` — `"the automation label that located the collision toggle is gone"`.
    Deleting or renaming `viewport_overlay.rs` makes `source()` panic with
    `"read {}: {e}"` (44).
11. `every_reach_surface_quotes_the_shared_area_and_bias_notes` (880-917).
    `src/ui/overlays/panel.rs` must contain `area_basis_note` and
    `over_statement_note`. Texts: `"the {name} no longer quotes `ReachMap::area_basis_note` …"`,
    `"the {name} no longer quotes `ReachMap::over_statement_note` …"`. It also
    reads `src/app/mcp.rs` and `src/app/mcp/view.rs`. **Consequence:** if the
    reach legend moves to a new file, this test must read that file.
12. `the_live_viewport_dims_moves_under_the_reach_overlay` (932-975). `panel.rs`
    must contain `"moves dimmed while reach map is on"`. Text:
    `"the legend no longer says the moves are being dimmed, so a ticked Cutting moves row beside faint lines reads as a contradiction"`.
13. `a_selection_is_pumped_before_the_same_calls_overlays_map` (780-812). Reads
    `src/app/mcp/view.rs`. Unaffected unless phase 2 edits `mcp_set_ui_view`.

### 5.2 `tests/viewport_draws_selected_only_wp27.rs`

1. `source()` (49-64) asserts `path.is_file()` —
   `"scanned path {} no longer exists"` — and `text.len() > 500` —
   `"{} is too short to be the file this scan means"`.
2. `ur5_has_no_isolate_routes` (242-319). Entry `("viewport overlay", source("src/ui/viewport_overlay.rs"), "show_all_toolpaths")` (270-274).
   Text: `"non-vacuity: {name} lost `{anchor}`"`. **Deleting
   `viewport_overlay.rs`, or removing the draw-scope button from it, breaks
   this test.** Also anchors `show_all_toolpaths` in `src/ui/sim_op_list.rs`
   (291-294) and `src/app/input.rs` (254).
3. `the_upload_key_carries_the_scope_and_the_selection` (374-404). Reads
   `overlay_upload_key` in `src/app.rs`. Unaffected unless phase 2 changes the
   upload key.
4. `a_fresh_viewport_draws_the_selected_toolpath_only` and
   `entering_simulation_keeps_toolpath_drawing_off` call `assert_the_row_exists`
   (95-101): `registry::row("all_toolpaths").is_some()`.

### 5.3 `tests/the_simulation_page_is_summary_first_dc6.rs` (HEAD lines; peer-modified)

1. `RUN_PRODUCER_SCAN` (constants 55-57, array 58-66) names `ui/viewport_overlay.rs`,
   `ui/overlays/panel.rs`, `ui/overlays/registry.rs`.
2. `the_scan_is_not_vacuous_dc6` (397-462). For each scan file (402-408):
   `"{rel} no longer exists; the sentry is stale"` and
   `"{rel} is empty; an absence scan over it would pass vacuously"`. Anchors:
   `(VIEWPORT_OVERLAY, "pub fn draw(")`, `(OVERLAYS_PANEL, "fn run_action(")`,
   `(OVERLAYS_REGISTRY, "pub enum OverlayAction")` (443-447) — text
   `"{rel} no longer defines {anchor}"`.
3. `simulation_workspace_has_one_direct_run_producer_ur3` (311-349). Total
   `AppEvent::RunSimulation` count over the seven files must be 1, in
   `ui/sim_op_list.rs`. No file may contain `RunSimulationWith`.
   `panel.rs` and `registry.rs` may not contain `RunSimulation` at all —
   `"{rel} restores an indirect Run Simulation route through the overlay registry; the workspace primary must remain the only producer."`.
   **Consequence:** a Blocked "run a simulation" remedy in the dock must not
   push `AppEvent::RunSimulation`. It can move focus to the primary, or name
   it.
4. `DELETED_HELP_SENTENCES` (74-78) includes `"(shortcut: O)"` (76). The arm reads
   `ui/sim_diagnostics.rs` only. Do not add that sentence to the Simulation
   inspector.

### 5.4 `tests/command_surface_completeness.rs`

1. `every_gui_reached_view_row_is_constructed_in_the_view` (399-429). Every
   `UiCommand` with `gui: Reach::Reached` must be constructed in GUI source.
   `ui/viewport_overlay.rs` is the **sole** GUI constructor of:
   - `UiCommand::ToggleProjection` (`viewport_overlay.rs:79`, `viewport_overlay.rs:91`; row `src/ui_command.rs:378-383`),
   - `UiCommand::ToggleShowAllToolpaths` (`viewport_overlay.rs:127`; row `src/ui_command.rs:409-415`),
   - `UiCommand::CancelCompute` (`viewport_overlay.rs:147`; row `src/ui_command.rs:768-774`).
   Text: `"these view rows claim the GUI reaches them, and no production view file constructs one: {missing:?}. Delete such a row; do not flip its gui column to Skip. …"`.
2. `the_p2_view_locator_finds_a_known_construction` (432-452). Asserts
   `view_constructs(&text, UiCommandId::CancelCompute)` —
   `"the viewport overlay constructs UiCommand::CancelCompute"`.
   **Consequence:** the dock must construct all three commands.

### 5.5 In-crate tests (`--lib`)

1. `src/controller/tests/mod.rs:351-357` calls
   `crate::ui::viewport_overlay::draw(ui, &mut controller.state, crate::render::camera::ProjectionMode::Perspective, &lanes, events)`.
   A rename or signature change breaks the `--lib` build.
2. `src/controller/tests/smoke.rs:33-34`,
   `ui_harness_records_lane_status_overlay_and_stock_to_leave`:
   `assert!(snapshot.widgets.contains_key("overlay_cancel_all"));` and
   `assert!(snapshot.widgets.contains_key("overlay_collision_check"));`.
   The ids come from `automation::record` at `viewport_overlay.rs:103-108` and
   `viewport_overlay.rs:145`. The harness renders one frame with a Running
   toolpath lane, so the Cancel control must render when a lane is active.
3. **Compile-time dependents.** These call sites name items in `panel.rs` or
   `viewport_overlay.rs`. A delete or rename breaks the crate build before
   any test runs:
   - `app/input.rs:587-595` imports `panel::COMMA_SURFACE` and
     `panel::PERIOD_SURFACE`. Move the two constants first.
   - `app/viewport.rs:239-261` calls `viewport_overlay::draw`,
     `panel::draw_docked`, `panel::MIN_VIEWPORT_WIDTH` and
     `panel::draw_floating`.
   - `src/controller/tests/mod.rs:351` (item 1 above).
   - `src/ui/mod.rs:31` declares `pub mod viewport_overlay;`.

### 5.6 Style and kit sentries that a new dock must pass

1. `tests/ui_string_hygiene.rs`. Scans every `.rs` file under `src/ui/` and
   the MCP files (77-97). A single-line literal must not hold a run of 3 or more
   interior spaces (`MIN_RUN`, 70) unless a line within 3 lines above carries
   `ui-string-columns:` (62-66). Text:
   `"operator-visible strings carry a run of {MIN_RUN}+ interior spaces. …"`.
   It also asserts `allowed > 0` (220-225) and more than 500 literals over
   10 files (214-219). Do not pad dock labels with spaces for alignment.
2. `tests/panels_read_the_token_module_up1.rs`. `LITERAL_BUDGET = 1`: one
   `Color32::from_rgb(` call outside `ui/tokens.rs` and `render/colors.rs`, and
   the one is `sim_timeline.rs desaturate()`. A legend swatch must use
   `tokens::from_linear_rgb` (`ui/tokens.rs:739`), as `panel.rs:326-328` does.
   A new raw `Grid::new(` outside `ui/components/` fails
   `a_two_column_param_grid_uses_the_kit_renderer_ui02` unless the file gets a
   `RAW_GRID_ALLOWANCE` row.
3. `tests/component_contracts_up2.rs`.
   - `HAND_ROLLED_EMPHASIS` row `("ui/overlays/panel.rs", 1, "the OVERLAYS title shares a horizontal row with the close and pin buttons; …")` (739-743).
     `the_hand_rolled_emphasis_list_is_not_vacuous_ui03` asserts
     `found == allowed` — `"{rel} is allowed {allowed} hand-rolled chains and holds {found}. …"`.
     Removing the `OVERLAYS` title row makes `found = 0` and fails. A new
     dock file with any `.small().strong()` chain fails
     `a_section_header_is_the_kit_element_ui03` unless it gets its own row.
   - `MIN_KIT_HEADERS = 20` (758). `panel.rs:161` holds one
     `SectionHeader::new("LEGEND")`. Removing it lowers the count by one.
4. `RAW_DRAG_VALUE_ALLOWANCE` (same file). A new `DragValue::new(` outside
   `ui/components/` needs a row. The stock opacity control is a `Slider`, so it
   is not counted.

### 5.7 MCP wire snapshot

1. `tests/mcp_wire_surface_pin.rs` compares each tool's `inputSchema` with
   `tests/snapshots/mcp_wire_surface.json`. The schema carries field doc
   comments. The `overlays` field description (snapshot line 2268) quotes
   `crates/rs_cam_mcp/src/server.rs:351-377`: "The ids are the rows of the
   GUI's Overlays panel …". **Any edit to that doc comment moves the
   snapshot.** Re-bless with `RS_CAM_UPDATE_WIRE_SNAPSHOT=1` in the same commit,
   or leave the text as it is.
2. The `reach_overlay` description (snapshot line 1548) says "The live GUI's
   own toggle is reachable since P6 — `set_ui_view(overlays: {"reach_map": true})`".
   It does not name the panel.
3. `src/mcp_server.rs:1593` repeats the id list in the `set_ui_view` tool
   description ("using the ids of the GUI's Overlays panel"). The pin does not
   include descriptions (`tests/mcp_wire_surface_pin.rs:24-28`).
4. Both id lists omit `all_toolpaths` and the three flagless ids. This is
   existing drift, not a phase-2 break.
5. `registry.rs:1352` refuses an unknown id with
   `"unknown overlay id '{id}' — see the Overlays panel"`. The test checks only
   that the text contains the id.

### 5.8 Non-test surfaces that name the strip, the panel or the keys

| Surface | Line | Text |
|---|---|---|
| `src/ui/shortcuts_window.rs` | 43-52 | heading `"Overlays"`; rows `("O", "Open / close the Overlays panel")`, `("Shift+O", "Pin / unpin it")`. It also still lists `("I", "Toggle isolation")` (36). No `Key::I` binding exists in `src/` (`rg -n "Key::I\b" src` is empty), so the row names a dead key. |
| `src/ui/overlays/panel.rs` | 106, 119-121, 532-533 | hovers `"Close the Overlays panel (shortcut: O)"`, `"… (shortcut: Shift+O)"`, `"… (shortcut: O; Shift+O pins it)."` |
| `src/app/input.rs` | 551-560 | `O` toggles `overlays.open`; `Shift+O` toggles `pinned` and opens. |
| `src/state/overlays.rs` | 35-40 | doc comments name the `Overlays (n)` button, `O` and `Shift+O`. |
| `FEATURE_CATALOG.md` | 142 | "Overlays panel (P6, 2026-09-08) … Opened from the `Overlays (n)` button on the viewport strip or with `O`; `Shift+O` pins it …". It also names an action "Rest Analysis…"; the label in code is `Compute rest` (`registry.rs:143`). |
| `FEATURE_CATALOG.md` | 172 | "The Overlays panel lists derived rest regions and the machining-boundary outline …" |
| `crates/rs_cam_viz/src/ui/CLAUDE.md` | 16 | "`viewport_overlay.rs` … — the strip above the 3D view" |
| `crates/rs_cam_viz/src/ui/overlays/CLAUDE.md` | whole file | "The viewport Overlays panel" |
| `crates/rs_cam_viz/src/state/CLAUDE.md` | 18 | "the Overlays panel state" |
| `crates/rs_cam_viz/CLAUDE.md` | 22 | "`ui/overlays/` \| The viewport Overlays panel and its registry" |

### 5.9 Count

Section 5 lists **61 assertions** in **9 test files**. Each one is quoted or
named above.

| Section | File | Assertions |
|---|---|---|
| 5.1 | `tests/overlays_registry.rs` | 33 |
| 5.2 | `tests/viewport_draws_selected_only_wp27.rs` | 4 |
| 5.3 | `tests/the_simulation_page_is_summary_first_dc6.rs` | 9 |
| 5.4 | `tests/command_surface_completeness.rs` | 2 |
| 5.5 | `src/controller/tests/mod.rs`, `src/controller/tests/smoke.rs` (plus the compile-time dependents in 5.5.3, not counted) | 3 |
| 5.6 | `tests/ui_string_hygiene.rs`, `tests/panels_read_the_token_module_up1.rs`, `tests/component_contracts_up2.rs` | 9 |
| 5.7 | `tests/mcp_wire_surface_pin.rs` (one snapshot compare) | 1 |
| **Total** | | **61** |

Most of the 33 in `overlays_registry.rs` break only if phase 2 changes what
the plan keeps: ids, defaults, exclusivity and the MCP refusal text. These
assertions break on a presentation change alone:

- `overlays_registry.rs`: 5.1.9 (the dock literal and the `draw_floating`
  call), 5.1.10 (`panel::toolbar_button`, `overlay_collision_check`, and the
  file read), 5.1.11 and 5.1.12 (if the legend leaves `panel.rs`).
- `viewport_draws_selected_only_wp27.rs`: 5.2.1 and 5.2.2.
- `the_simulation_page_is_summary_first_dc6.rs`: 5.3.2.
- `command_surface_completeness.rs`: 5.4.1 and 5.4.2.
- `controller/tests`: 5.5.1 and 5.5.2.
- `component_contracts_up2.rs`: 5.6.3.

The `caps == 6` assertion in 5.1.9 is in the count, but it is already red on
the working tree for a reason outside phase 2.

## 6. Three findings to rule on before phase 3

These are defects today. The redesign exposes them. They are not phase-2 work.

1. The inspector reach checkbox bypasses exclusivity (section 2.3, item 6).
2. Deviation and Advance per tooth keep their mode without data and paint a
   substitute (section 2.3, items 3 and 4).
3. The rest legend and the rest mesh use different scale ends (section 3, the
   Rest row).
