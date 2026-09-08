# Viewport overlays — UX design pass

**Date:** 2026-09-08
**Type:** design document. No code changed. No `cargo` command ran.
**Branch observed:** `reach-map-p5`
**Screenshots:** `planning/ui_overlays_ux_2026-09-08/*.png`

## 0. Scope and method

The operator reports that the viewport overlays are hard to find:

> "The overlays are getting a bit confusing. Rest is kept in a button, the
> split regions are shown only when calculated and then clicked on. The new
> ones (reach map, tier map, plunge/kinematic findings) might be hard. I
> wonder if we need an 'Overlays' control in the viewport that shows a
> checkmark for region, rest, etc. Regions should include any region that
> reaches the dropdown menu. The current 'Show' menu is a little rough — do
> we need something better than these dropdowns? A lot of the features are a
> little invisible and hard to get to in the UX."

This document does four things. It inventories every viewport overlay. It
names where each overlay is switched on today. It walks six use cases. It
proposes one design and a migration.

**Evidence rules.** Every claim about current behaviour cites a `file:line`
or a screenshot. A claim this pass could not verify carries the words
**not verified**. This session ran no generation and no simulation, so no
overlay that needs simulated data was seen on screen.

**A caution about the older audit.** `planning/ui_audit/` is partly stale.
It lists `crates/rs_cam_viz/src/ui/project_tree.rs`, which commit
`57422407` deleted. It marks "Inspector › View" red for duplicate visibility
toggles, and that duplication is already removed
(`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:105-124`). Prefer the code
cites in this document.

All viz paths below are relative to `crates/rs_cam_viz/src/`.

---

## 1. The three switching mechanisms

An overlay is switched by up to three separate mechanisms. The distinction
matters, because a redesign can only move one of them.

| Mechanism | Meaning | Can the operator override it? |
|---|---|---|
| **Hard gate** | A workspace or correctness condition in `app/viewport.rs` or `app/gpu_upload.rs` | No |
| **Precondition** | The data must exist — computed, selected, or simulated | Only by computing the data |
| **Default** | A `bool` in `ViewportState` that a control writes | Yes |

`app/viewport.rs:442-530` builds one `ViewportCallback` per frame. It applies
the hard gates and the preconditions there, on top of the flags in
`state/viewport.rs:51-89`.

The hard gates in force today:

- the model mesh is hidden in Simulation (`app/viewport.rs:451-457`)
- the solid stock draws only in Setup (`app/viewport.rs:470`)
- the height planes draw only in Toolpaths, with a toolpath selected (`app/viewport.rs:471-472`)
- the rest heatmap draws only in Toolpaths (`app/viewport.rs:473-475`)
- the tier preview draws only in Toolpaths, and only when the previewed setup is active (`app/viewport.rs:484-488`)
- fixtures and keep-outs are suppressed in Simulation (`app/gpu_upload.rs:582-584`), and the whole fixture buffer is hidden there unless pins exist (`app/viewport.rs:465-467`)
- the datum crosshair draws only in Setup (`app/gpu_upload.rs:670`)
- the simulated stock draws only in Simulation, with results (`app/viewport.rs:489-490`)
- the origin axes have no flag of their own; they follow `show_stock` (`app/viewport.rs:512-517`)

One of these is a **correctness** gate and must stay. The tier map lives in
its own setup's emission frame. In another setup's display frame it draws at
a wrong offset, which an operator observed (`app/viewport.rs:477-483`). The
rest are conveniences that a redesign can convert into per-workspace
defaults.

**There are two overlay inventories, not one.** The GPU overlays travel
through `ViewportCallback` (`render/mod.rs:772-810`). A second set is painted
by egui on top of the viewport and does not appear in that struct:

- the tool-deflection panel (`app/viewport.rs:293`, `app/viewport.rs:744-800`)
- the orientation gizmo (`app/viewport.rs:536`, `:614-660`)
- the checkpoint loading pill (`app/viewport.rs:544-570`)
- the active generator-step box (`app/viewport.rs:585-608`)
- the rest-heatmap legend (`ui/viewport_overlay.rs:303-310`)

Three of those five have no switch at all.

---

## 2. Overlay inventory

**Disc.** scores discoverability from 1 (invisible) to 5 (obvious). The
reason follows the score.

### 2.1 Geometry

| # | Overlay | What it shows | Toggle lives in | Precondition | Hard gate | Disc. |
|---|---|---|---|---|---|---|
| G1 | Grid | Ground grid | `Show ▼ ▸ Grid` (`ui/viewport_overlay.rs:117`) | none | none | 4 — named, but inside a popover |
| G2 | Model mesh | Imported STL / STEP surface | **No toggle.** Render mode only (`Shaded ▼`, `ui/viewport_overlay.rs:61-80`) | a model carries a mesh | hidden in Simulation (`app/viewport.rs:451-457`) | 2 — the operator cannot hide it, and cannot learn why it vanishes in Simulation |
| G3 | Stock box | Stock wireframe (`render/stock_render.rs:14`) | `Show ▼ ▸ Stock` (`ui/viewport_overlay.rs:118`) | a model carries a mesh (`app/viewport.rs:459-464`) | none | 4 |
| G4 | Solid stock | Opaque stock block (`render/stock_render.rs:146`) | **No toggle.** Derived from `show_stock` (`app/viewport.rs:470`) | as G3 | Setup only | 1 — one checkbox removes G3, G4 and G5 together |
| G5 | Origin axes | XYZ axes at the stock origin | **No toggle.** Derived from `show_stock` (`app/viewport.rs:512-517`) | a model carries a mesh | none | 1 — no control names it |
| G6 | Datum crosshair | The XY zero the export uses | **No toggle.** In the fixture buffer (`app/gpu_upload.rs:670-748`) | a datum is set (`ui/properties/setup.rs:114,148`) | **Setup only** (`app/gpu_upload.rs:670`) | 1 — magenta crosshair with no label; distinct from G5 and easily confused with it |
| G7 | Fixtures | Fixture clearance boxes | `Show ▼ ▸ Fixtures` (`ui/viewport_overlay.rs:119`) | a fixture exists and is `enabled` | suppressed in Simulation (`app/gpu_upload.rs:582-584`) | 3 |
| G8 | Keep-out zones | Red wireframe forbidden volumes (`app/gpu_upload.rs:605-619`) | **No toggle of their own.** They ride `show_fixtures` | a keep-out zone exists (`ui/properties/setup.rs:334-361`) | as G7 | 1 — the only checkbox says "Fixtures" |
| G9 | Alignment pins | Green circles (`app/gpu_upload.rs:622-638`) | **No toggle of their own.** They ride `show_fixtures` | pins exist on the stock | drawn in **all** setups, unlike G7 / G8 | 1 — edited in the Stock panel (screenshot 05), drawn under a "Fixtures" label |
| G10 | Flip-axis centreline | Dashed line for a two-sided job (`app/gpu_upload.rs:640-668`) | **No toggle of their own.** They ride `show_fixtures` | `stock.flip_axis` is set | as G7 | 1 |
| G11 | Curves (DXF / SVG) | Imported 2D polygons | `Show ▼ ▸ Curves (DXF/SVG)` (`ui/viewport_overlay.rs:120`) | a model carries polygons (`app/viewport.rs:468-469`) | none | 4 |
| G12 | Orientation gizmo | Camera axis triad | **No toggle** (`app/viewport.rs:536`) | none | none | 5 |

> **One checkbox drives five overlays.** `show_fixtures`
> (`ui/viewport_overlay.rs:119`) gates a single `fixture_data` line buffer
> (`render/mod.rs:183`, drawn `render/mod.rs:977-978`) that carries G7, G8,
> G9, G10 and G6. There is no per-kind visibility control.

### 2.2 Toolpath

| # | Overlay | What it shows | Toggle lives in | Precondition | Hard gate | Disc. |
|---|---|---|---|---|---|---|
| T1 | Cutting moves | Green fed moves | `Show ▼ ▸ Paths (cutting)` (`ui/viewport_overlay.rs:122`) | a generated toolpath | none | 4 |
| T2 | Rapids | Orange rapid moves | `Show ▼ ▸ Rapids` (`ui/viewport_overlay.rs:123`) | a generated toolpath | none | 4 |
| T3 | Per-toolpath cut / rapid | One toolpath's moves | **Three sites**: row `C` / `R` glyphs (`ui/toolpath_row_controls.rs:52-102`), and plain "Cut" / "Rapid" checkboxes in the properties simulation panel (`ui/properties/mod.rs:999-1011`) | as T1 / T2 | AND'd with T1 / T2 (`render/mod.rs:1122-1133`) | 3 — the row glyphs grey out and **name the control that blocks them** (`ui/toolpath_row_controls.rs:69-72`), but one state has two differently-styled homes |
| T4 | Per-toolpath visibility | Hide one whole toolpath | Row eye button (`ui/toolpath_row_controls.rs:27-42`); key `H` (`app/input.rs:483-489`) | a toolpath exists | none | 3 |
| T5 | Isolation | Only the selected toolpath | `Isolate` button (`ui/viewport_overlay.rs:254-260`); row bullseye (`ui/toolpath_row_controls.rs:104-132`); key `I` (`app/input.rs:476-481`) | a toolpath is selected | none | 4 — three homes; the only overlay-adjacent keyboard shortcut |
| T6 | Span-kind filter | Hide Entry / LeadOut / LinkBridge / Dressup segments | `Show ▼ ▸ By SpanKind ▾` (`ui/viewport_overlay.rs:130-157`) | a generated toolpath | **inert outside Palette colour mode** (`ui/viewport_overlay.rs:126-129`) | 1 — a submenu inside a popover that silently does nothing in two of three colour modes |
| T7 | Toolpath colour mode | Palette / Engagement / Advance-per-tooth (`render/toolpath_render.rs:117,414,590`) | `Show ▼ ▸ Toolpath color:` combo (`ui/viewport_overlay.rs:191-227`) | Advance-per-tooth needs a simulation | none | 2 — a combo at the foot of a popover, with **no legend** |
| T8 | Tool-profile ghost | Swept cutter silhouette (`render/toolpath_render.rs:387,1072`) | `Show ▼ ▸ Tool-profile ghost` (`ui/viewport_overlay.rs:190`) | a generated toolpath | `app/gpu_upload.rs:1015` | 3 |
| T9 | Entry markers | Where each entry / ramp / helix starts, in cyan (`render/toolpath_render.rs:362,806,963`; `colors.rs:27`) | **No toggle.** Drawn whenever a toolpath is selected (`app/gpu_upload.rs:993`) | a toolpath is selected | none | 2 — geometric only. It shows **where** an entry is, never **how hard** it is |
| T10 | Height planes | Five Z planes: clearance, retract, feed, top, bottom (`render/height_planes.rs:24`; `colors.rs:66-71`) | **No toggle** (`app/viewport.rs:471-472`) | a toolpath is selected | Toolpaths only | 1 — the planes appear and vanish with selection, unlabelled |

### 2.3 Regions

This is the group the operator asked for. Today it has no group, and **only
two of its seven members ever reach the 3D view.**

| # | Overlay | What it shows | Toggle lives in | Precondition | Hard gate | Disc. |
|---|---|---|---|---|---|---|
| R1 | Rest heatmap | Depth of material this op leaves | `Show ▼ ▸ Rest heatmap` (`ui/viewport_overlay.rs:164-172`) | the **selected** toolpath's result carries a `rest_grid` (`app/viewport.rs:246-264`) | Toolpaths only (`app/viewport.rs:473-475`) | 2 — the checkbox greys with a reason, and the reason is **wrong** (Finding 3) |
| R2 | Rest legend | Threshold and peak, in mm | **No toggle.** Auto with R1 (`ui/viewport_overlay.rs:303-310`) | as R1 | as R1 | 5 when it draws (screenshot 06, top left) |
| R3 | Derived rest regions | The islands a rest pass would cut | **Never drawn in 3D.** A combo only: `Geometry ▸ Machining Boundary ▸ Source ▸ Rest Regions` (`ui/properties/mod.rs:4117-4148`), then a `Rest source:` picker (`:4159-4200`) | another toolpath computed rest analysis | none | 1 — the operator's "shown only when calculated and then clicked on" |
| R4 | Tier-map preview | One colour per tool tier over its territory (`app/gpu_upload.rs:1246-1280`) | `Show ▼ ▸ Tier preview` (`ui/viewport_overlay.rs:177-187`) | the planner holds a `Ready` preview (`state/multitool_planner.rs:354-359`) | Toolpaths **and** the previewed setup is active (`app/viewport.rs:484-488`) | 2 — entry point is `Toolpath ▸ Plan multi-tool finishing…` (`ui/menu_bar.rs:189-201`), three menus from the checkbox. **No legend.** |
| R5 | Boundary polygon | The clip polygon in use | **Never drawn.** No boundary term appears in `render/` or `app/gpu_upload.rs` | a boundary is enabled (`ui/properties/mod.rs:4050-4281`) | n/a | 1 |
| R6 | Monotone cells (C2) | The shallow-band decomposition | **Never drawn.** Two dials only: planner-wide (`ui/multitool_planner.rs:315-332`) and per-op Unified Finish (`ui/properties/operations/surface_3d.rs:940-955`) | none | n/a | 1 — the planner hover says "Emission-only: the preview's islands are unchanged by this" (`ui/multitool_planner.rs:330`) |
| R7 | Planner islands | Per-tier island polygons | Table inside the planner dialog only (`ui/multitool_planner.rs:575-591,622,677`) | a `Ready` preview | none | 2 |

**There is no `PickHit` for a rest region, a tier island, a monotone cell or
a boundary** (`interaction/picking.rs:19`). Fixtures, keep-outs and pins are
pickable, and only in the Setup workspace (`interaction/picking.rs:112-158`).

### 2.4 Simulation and analysis

| # | Overlay | What it shows | Toggle lives in | Precondition | Hard gate | Disc. |
|---|---|---|---|---|---|---|
| S1 | Simulated stock | The dexel stock as cut | **No toggle** (`app/viewport.rs:489-490`) | a simulation ran | Simulation only | 2 — see Finding 5 |
| S2 | Stock opacity | How transparent S1 is | `Inspector ▸ View ▸ Opacity` (`ui/sim_diagnostics.rs:99-103`) | as S1 | affects **only** S1 (`render/mod.rs:838-846`) | 2 — inside a header that is **closed by default** (`ui/sim_diagnostics.rs:87-88`) |
| S3 | Stock colour mode | Solid / Deviation / By Height (`app/gpu_upload.rs:47-86`; `render/sim_render.rs:118`) | `Inspector ▸ View ▸ Stock color` (`ui/sim_diagnostics.rs:126-164`) | Deviation needs `display_deviations` | as S1 | 2 — same closed header. **Best precedent in the app:** with no data it prints "No deviation data —" beside a `Re-run simulation` button (`ui/sim_diagnostics.rs:169-190`) |
| S4 | Tool model | The cutter at the playhead | **No toggle** (`app/viewport.rs:501-503`) | a simulation ran, playback holds a position | Simulation only | 3 |
| S5 | Scrub reveal | Moves drawn up to the playhead | **No toggle.** Driven by playback (`app/viewport.rs:504-511`) | a simulation ran | Simulation only | 4 |
| S6 | Collision markers | Holder and shank strike points, as 3-axis crosses graded yellow→red by local density (`app/gpu_upload.rs:776-886`) | `Show ▼ ▸ Collisions` (`ui/viewport_overlay.rs:124`) | a collision check ran (`controller/events/compute.rs:1253`) | none | 3 |
| S7 | Tool deflection panel | Deflection in µm, drawn as a bent cutter | **No toggle at all** (`app/viewport.rs:744-752`) | a simulation ran, playback holds a position | Simulation only | 3 — visible, and the operator cannot switch it off |
| S8 | Active generator step | A box around the step now playing | `Inspector ▸ View ▸ Highlight active step` (`ui/sim_diagnostics.rs:199-207`) | `Record generator trace` was on for the run | Simulation only | 1 — **the checkbox is hidden entirely when no trace exists** (`ui/sim_diagnostics.rs:193`), so the feature is invisible until it is already usable |
| S9 | Loading pill | Spinner during a backward seek | **No toggle** (`app/viewport.rs:544-570`) | a backward seek is in flight | Simulation only | 5 |
| S10 | Hotspot pins | Wasted-runtime buckets | **Removed by design** (`app/viewport.rs:572-583`) | — | — | n/a — Finding 8 |
| S11 | Reach map (P5) | Per-vertex reached / unreachable on the model | **Does not exist in the GUI** — Finding 9 | a finishing op is selected | — | 0 |
| S12 | Kinematic utilization | Binding shares, and the planned / emitted qualifier | **Not a viewport overlay.** A pill on the operation-list row (`ui/sim_op_list.rs:950-958`, builder `:972-1024`) | a generated toolpath | — | 1 |
| S13 | Plunge-class / entry-load findings | Over-limit plunges and buried entry chords | **Not a viewport overlay.** The plunge ratio is **hover text on the S12 pill** (`ui/sim_op_list.rs:997-1005`); `PROJECT_ENTRY_LOAD` reaches the GUI only through the generic triage list (`ui/sim_diagnostics.rs:614,651-665`) with no id-specific handling | a simulation ran | — | 1 |

### 2.5 Counts

**The counting rule.** A row counts once. "No switch" means no control
anywhere turns it on or off. "Never draws" means a control exists and no
render call consumes it.

| Category | Count | Rows |
|---|---|---|
| Rows listed | **42** | G1-G12, T1-T10, R1-R7, S1-S13 |
| Have a working switch | 19 | G1, G3, G7, G11; T1-T8; R1, R4, R7; S2, S3, S6, S8 |
| **Have no switch of any kind** | **16** | G2, G4, G5, G6, G8, G9, G10, G12; T9, T10; R2; S1, S4, S5, S7, S9 |
| Have a control and never draw | 3 | R3, R5, R6 |
| Removed by design | 1 | S10 |
| Not built in the GUI | 1 | S11 |
| Not viewport overlays at all | 2 | S12, S13 |

- 12 controls live in `Show ▼`. 3 live in `Inspector ▸ View`. 4 live on a toolpath row. 2 live in the toolpath properties panel. 1 lives in a top-level menu.
- **1 overlay has a legend** (R2). Five colour-carrying overlays have none: T7 Engagement, T7 Advance-per-tooth, S3 Deviation, S3 By Height, R4 Tier map. Their scales exist only in `on_hover_text` (`ui/viewport_overlay.rs:205-223`; `ui/sim_diagnostics.rs:150-163`).
- **0 overlays have a keyboard shortcut.** `I`, `H`, `G` and `1`-`4` exist (`app/input.rs:438-511`), and none switches an overlay.

---

## 3. Where the controls are, on screen

| Screenshot | Shows |
|---|---|
| `01-setup-workspace.png` | Setup workspace. The strip reads `View ▼ · Shaded ▼ · Persp ▼ · Show ▼ · Isolate`. Toolpaths draw here too. |
| `02-toolpaths-geometry.png` | 3D rough selected, Geometry tab. `Machining Boundary` and `Rest Analysis` sit at the foot of a long scrolling tab. The row controls (eye, C, R, bullseye) are on the selected card. |
| `03-simulation-empty.png` | Simulation with no results. The model mesh is gone (G2's hard gate). The `Inspector ▸ View` header is collapsed. |
| `04-readiness.png` | Readiness workspace. **It renders no viewport**, so no overlay reaches it. |
| `05-setup-stock-panel.png` | Stock Setup panel. Alignment pins are numeric fields here, far from the `Fixtures` checkbox that draws them. |
| `06-toolpaths-pencil-linking.png` | Pencil op selected. The rest legend draws at the viewport top left. The panel reads `Rest reference: Machined stock (requires simulation)`. |
| `07-toolpaths-linking-tab.png` | Linking tab. Entry style, lead-in and lead-out have **diagrams in the panel** and no viewport overlay beyond the unlabelled cyan markers (T9). |

`set_ui_view` cannot open the `Show ▼` popover, so no screenshot of it
exists. Its contents are transcribed from `ui/viewport_overlay.rs:115-228`.

---

## 4. Findings

**Finding 1 — `Show ▼` is a flat list of twelve unrelated things.**
`ui/viewport_overlay.rs:115-228` puts the grid, the stock, fixtures, curves,
paths, rapids, collisions, a span-kind submenu, the rest heatmap, the tier
preview, the tool ghost and a colour combo in one popover with two
separators. The popover closes on click, so comparing two overlays costs two
round trips.

**Finding 2 — nineteen of thirty-six overlays have no control.** An operator
cannot learn that height planes (T10), the datum crosshair (G6), keep-out
zones (G8) or the deflection panel (S7) exist, because nothing names them.

**Finding 3 — the rest-heatmap hover text misinstructs the operator.** The
`Show ▼` entry says "Select a pencil rest-depth toolpath to enable"
(`ui/viewport_overlay.rs:171`), and the comments above it say only the pencil
detector fills a `rest_grid` (`ui/viewport_overlay.rs:24-27`;
`state/viewport.rs:64-66`). That is stale. Since the P2.5 consolidation
**any** operation can attach a rest grid — `Geometry ▸ Rest Analysis ▸
Compute rest heatmap` (`ui/properties/mod.rs:4322-4325`) sets the flag, and
`rs_cam_core::compute::execute::attach_generic_rest_analysis:3316-3317`
writes `rest_grid` and `rest_regions` for any op with
`rest_analysis.enabled`. The hover text sends the operator to the wrong
control.

Two more strings carry the same stale claim, in the boundary picker:
"rest regions computed by another toolpath's **pencil** rest-depth
detector" and "add one and generate it with a **pencil** rest-depth
detector" (`ui/properties/mod.rs:4129-4136`), and "the toolpath whose
**pencil** rest-depth detector supplies the rest regions"
(`ui/properties/mod.rs:4195-4196`). All five strings need the same fix.

**Finding 4 — "Rest" names five unrelated surfaces.** The operator's
sentence "Rest is kept in a button" understates the problem. There is no
single rest button. There are five:

| Surface | Where | What it is |
|---|---|---|
| Rest **operation** | `ui/properties/operations/boundary_2d.rs:340`, prev-tool combo `:358`; queue badge `ui/toolpath_panel.rs:713` | a 2.5D op that re-cuts what a larger tool left |
| Rest **analysis** | `ui/properties/mod.rs:4285-4455` | attaches a heatmap grid and regions to any op |
| Rest **heatmap overlay** | `ui/viewport_overlay.rs:164-172` | the only rest thing in 3D |
| Rest **claims** | `ui/properties/operations/surface_3d.rs:714-790` | Unified Finish `claims_reference` |
| Rest **regions as a boundary** | `ui/properties/mod.rs:4117-4200` | consumes another op's regions |

A single "Rest" entry in an Overlays panel is therefore not correct. The proposal
below names each surface separately and links to its authoring home.

**Finding 5 — the "Stock" checkbox cannot hide the simulated stock.**
`ui/sim_diagnostics.rs:90-93` states that the show-and-hide toggle "lives in
the viewport `Show ▼` menu (W4.3: one home for visibility)". It does not.
`viewport.show_stock` gates three things — the stock wireframe
(`app/viewport.rs:459-464`), the solid stock in Setup (`:470`) and the
origin axes (`:512-517`). It never reaches `show_sim_mesh`, which reads only
the workspace and `has_results()` (`app/viewport.rs:489-490`). So the
simulated stock cannot be hidden by any control. The W4.3 unification is
incomplete, and the comment claims otherwise. Relatedly, `stock_opacity` affects only the sim
mesh; the solid stock and the height planes are pinned at `0.15` with no
control (`render/mod.rs:838-846`).

**Finding 6 — the tier-preview checkbox and its render gate disagree.** The
checkbox greys on `ready_preview().is_some()` alone
(`app/viewport.rs:267-272, 285`). The render additionally requires the
previewed setup to be the active setup (`app/viewport.rs:484-488`). On any
other setup the checkbox therefore reads **on** while nothing draws, with no
explanation. The setup clause is correct and must stay
(`app/viewport.rs:477-483`); the checkbox needs the same condition and a
reason line.

**Finding 7 — the app already holds both the good pattern and the bad one,
ten lines apart.** `Inspector ▸ View` shows the `Deviation` mode always, and
when the data is missing it prints a reason beside a `Re-run simulation`
button (`ui/sim_diagnostics.rs:169-190`). Ten lines later it **hides**
`Show generator steps` unless a trace was recorded
(`ui/sim_diagnostics.rs:193`). The first pattern teaches the feature. The
second hides it. The row controls use the first pattern and even name the
blocking control (`ui/toolpath_row_controls.rs:69-72`). The design below
generalises the first pattern.

**Finding 8 — hotspot pins were removed on purpose.**
`app/viewport.rs:572-583` records the F6.1 decision:
`SimulationCutTrace.hotspots` is one aggregator per
`(toolpath_id, semantic_item_id)`, not a problem flag, so pins read as
danger everywhere. The Inspector hotspot table replaces them
(`ui/sim_diagnostics.rs:292-380, 808-855`), and F6.2 reserves pins for the
Critical and Risky tier. Any proposal to re-add them must adopt that
tiering. This document does not propose re-adding them.

**Finding 9 — the reach map has a core, and no GUI.** The kernel is
**untracked working-tree work**: `crates/rs_cam_core/src/reach_map.rs`,
`reach_map_cache.rs`, `session/reach.rs`, `tests/reach_map_p5.rs`, plus
uncommitted edits to `lib.rs`, `session/mod.rs` and `compute/catalog.rs`.
`git log` returns nothing for `reach_map.rs`. A search of
`crates/rs_cam_viz/src/` finds **no reference to it** — no `ViewportState`
flag, no render slot, no `Show ▼` entry, no MCP tool, no panel readout.

The core already publishes what a UI needs:
`ReachMap::vertex_gaps(mesh, index)` returns one gap per mesh vertex with
`NaN` for "not measured" (`reach_map.rs:339`); `reach_color` and
`reach_colors` map a gap to green-through-red against the tolerance
(`reach_map.rs:403,420`); `unreachable_pct()` (`:299`) and `max_gap_mm`
(`:231`) are the two numbers the plan asks for;
`OperationType::supports_reach_map()` names exactly ten finishing ops
(`compute/catalog.rs:329-360`); `session/reach.rs:55` splits a cheap
UI-thread spec from a memoised worker-side compute.

**One mechanism gap matters for the design.** The rest heatmap and the tier
map drape a *generated heightmap mesh* through `SimMeshGpuData`
(`render/mod.rs:1053-1080`). The reach map colours the *model mesh itself*,
and the plain model path carries no per-vertex colour
(`render/mesh_render.rs:64`). The reach overlay therefore needs a new
coloured-model upload path, or the enriched-mesh route
(`render/mesh_render.rs:233`). Every reach-map statement in §6 is **design
intent, not observed behaviour.**

**Finding 10 — the plan document is stale about its own state.**
`planning/machine_kinematics_confidence_2026-09-07.md:526` says "Not
started" while the core exists in the working tree. Two name drifts also
sit in the new code: `session/reach.rs:12,35` call the type `ReachMapSpec`
where `reach_map.rs:215` defines `ReachMapRequest`.

**Finding 11 — the MCP agent cannot touch an overlay.** `SetUiViewParam`
carries `workspace`, `toolpath_index`, `properties_tab`, `select` and
`modal` (`crates/rs_cam_mcp/src/server.rs:222-240`). It carries no overlay
field. An agent can navigate to a panel and photograph it. It cannot switch
on the overlay it wants to photograph.

**Finding 12 — the span-kind filter is silently inert in two colour modes.**
`ui/viewport_overlay.rs:126-129` states that the filter applies only in
Palette mode. The submenu gives no sign of this while Engagement or
Advance-per-tooth is active.

---

## 5. Use cases

### (a) Setup review before a run

**Needs:** stock (G3, G4), origin axes (G5), datum (G6), fixtures (G7),
keep-outs (G8), pins (G9), flip axis (G10), model (G2).

**What makes it hard.** Six of those eight have no control. One checkbox
named "Fixtures" draws five different things (`render/mod.rs:977-978`).
Turning "Stock" off to inspect a fixture also removes the origin axes and
the solid block. The pins are edited in the Stock panel (screenshot 05) and
drawn under a label that does not mention them. The datum, which decides
where the G-code zeroes, is an unlabelled magenta crosshair.

### (b) Authoring a toolpath

**Needs:** the boundary polygon (R5), regions (R3, R7), entry moves (T9,
T6), height planes (T10), the tool ghost (T8).

**What makes it hard.** The boundary is authored as combos and numbers
(`ui/properties/mod.rs:4050-4281`) and **is never drawn**. Entries appear as
2D diagrams in the Linking tab (screenshot 07) plus unlabelled cyan markers
in 3D, and the 3D filter for them is inert unless the colour mode is Palette
(Finding 12). The height planes appear on selection with no label and no
legend.

### (c) Reviewing a simulation

**Needs:** simulated stock (S1, S2), deviation (S3), collisions (S6),
chipload heat (T7), deflection (S7), findings (S13).

**What makes it hard.** The controls are split across two homes with a
one-way pointer: `Show ▼` holds Collisions and the move colour mode;
`Inspector ▸ View` holds opacity and the stock colour mode behind a header
closed by default (`ui/sim_diagnostics.rs:87-88`). Two colour scales can be
active at once — advance-per-tooth on the moves and deviation on the stock —
and **neither has a legend**. The stock cannot be hidden at all (Finding 5).

### (d) Planning rest and multitool finishing

**Needs:** rest heatmap (R1, R2), derived rest regions (R3), tier map (R4),
islands (R7).

**What makes it hard.** This use case has the most scattered controls. Four
related overlays sit in four places: a properties-tab checkbox, a
properties-tab combo that draws nothing at all, a top-level menu item, and
two `Show ▼` checkboxes. One of the four is written by the planner without
the operator asking (`controller/events/planner.rs:147`;
`controller/events/compute.rs:1372`), and its checkbox can read on while
nothing draws (Finding 6). The word "rest" means five things (Finding 4).

### (e) Choosing a finishing tool

**Needs:** the reach map (S11), the tier map (R4), a cusp readout.

**What makes it hard.** The reach map has no GUI (Finding 9). The tier map
answers a neighbouring question and needs two tools plus a planner run
before it draws (`ui/menu_bar.rs:186-201`). The operator's own question —
"does this ball radius fit into the mountain valleys?"
(`reach_map.rs:4-6`) — has no surface today.

### (f) An agent driving the GUI over MCP

**Needs:** reach a workspace, select a toolpath, switch on one overlay,
photograph it.

**What makes it hard.** `set_ui_view` cannot switch an overlay (Finding 11).
Two contract notes observed this session:

- `properties_tab` took effect only on the **second** `set_ui_view` call. The first echoed `"properties_tab":"linking"` while the panel still showed Geometry (screenshot 06 against screenshot 07). The parameter doc already warns that it applies "the next time the properties panel renders" (`crates/rs_cam_mcp/src/server.rs:229-232`).
- Readiness renders no viewport (screenshot 04), so an overlay request against it must be refused, not silently accepted.

---

## 6. Proposal — an Overlays panel in the viewport

### 6.1 The rule that drives the design

> **Every overlay appears in the list, always. An overlay that cannot draw
> is shown disabled, with one line that says why, and — where one exists —
> a button that makes it drawable.**

This generalises the `Deviation` precedent (`ui/sim_diagnostics.rs:169-190`)
and the row-control precedent (`ui/toolpath_row_controls.rs:69-72`). It
replaces the `Show generator steps` precedent
(`ui/sim_diagnostics.rs:193`), which hides a feature until it is already in
use.

### 6.2 Shape

Replace `Show ▼` with an **`Overlays` button** on the same strip. It opens a
**pinnable panel** docked to the left edge of the viewport, in two states:

- **Popover** (default): it closes on click-away, as `Show ▼` does today.
- **Pinned**: a pin icon docks it as a narrow column inside the viewport. It stays open across clicks, and its pin state is remembered per workspace.

Four collapsible groups, each remembering its open state:

1. **Geometry** — G1 … G12
2. **Toolpath** — T1, T2, T6, T8, T9, T10 (T3, T4, T5 stay on the row; §6.7)
3. **Regions** — R1, R3, R4, R5, R6, R7
4. **Analysis** — T7, S1, S2, S3, S6, S7, S8, S11, S13

T7 the move colour mode sits in Analysis, beside the other two colour
sources, because §6.5 groups the three radios by surface. S12 stays on the
operation-list row; it is a text readout, and the panel links to it.

The button carries a count of non-default overlays, so a pinned panel is not
needed to see that something is switched on.

### 6.3 Mockup

```
┌─ 3D viewport ───────────────────────────────────────────────────────┐
│ View ▼  Shaded ▼  Persp ▼  [ Overlays (3) ]  Isolate      Generate  │
├──────────────────────────┬──────────────────────────────────────────┤
│ OVERLAYS            📌 ✕ │                                          │
│                          │                                          │
│ ▾ Geometry               │                                          │
│   ☑ Grid                 │                                          │
│   ☑ Model                │                                          │
│   ☑ Stock — box          │                                          │
│   ☑ Stock — solid        │                                          │
│   ☑ Origin axes          │                                          │
│   ☑ Datum crosshair      │                                          │
│   ☑ Fixtures         (2) │                                          │
│   ☐ Keep-out zones       │                                          │
│     · none in this setup │                                          │
│   ☑ Alignment pins   (2) │                                          │
│   ☑ Flip axis            │                                          │
│   ☑ Curves (DXF/SVG)     │                                          │
│                          │                                          │
│ ▾ Toolpath               │                                          │
│   ☑ Cutting moves        │                                          │
│   ☑ Rapids               │                                          │
│   ☑ Entry markers        │                                          │
│   ☑ Height planes        │                                          │
│   ☐ Tool-profile ghost   │                                          │
│   Hide spans:  ▾         │                                          │
│     ☑ Entry  ☑ LeadOut   │                                          │
│     ☑ Link   ☑ Dressup   │                                          │
│     ⚠ applies in Palette │                                          │
│       colour only        │                                          │
│                          │                                          │
│ ▾ Regions                │                                          │
│   ☑ Rest heatmap    [⚙]  │                                          │
│   ☐ Tier map        [⚙]  │                                          │
│     · previewed on       │                                          │
│       Setup 2 — switch   │                                          │
│       setup to see it    │                                          │
│   ☐ Rest regions         │                                          │
│     · 5 Rough can        │                                          │
│       produce these      │                                          │
│       [Compute]          │                                          │
│   ☐ Planner islands      │                                          │
│     · [Plan…]            │                                          │
│   ☐ Boundary outline     │                                          │
│   ☐ Monotone cells       │                                          │
│                          │                                          │
│ ▾ Analysis               │                                          │
│   Colour the MODEL by:   │                                          │
│   ◉ none                 │                                          │
│   ○ Reach (this tool)    │                                          │
│     · select a finishing │                                          │
│       operation          │                                          │
│     ⚠ clears Regions ▸   │                                          │
│       Rest heatmap       │                                          │
│   Colour the STOCK by:   │                                          │
│   ◉ Solid                │                                          │
│   ○ Deviation            │                                          │
│     · [Run simulation]   │                                          │
│   ○ By height            │                                          │
│   Colour the MOVES by:   │                                          │
│   ◉ Palette              │                                          │
│   ○ Engagement           │                                          │
│   ○ Advance / tooth      │                                          │
│   ─────────────────────  │                                          │
│   ☑ Simulated stock      │                                          │
│     Opacity  ▁▄▆█        │                                          │
│   ☑ Collisions       (0) │                                          │
│   ☑ Tool deflection      │                                          │
│   ☐ Generator steps      │                                          │
│     · [Record & re-run]  │                                          │
│   ☐ Plunge-class marks   │                                          │
│     · [Run simulation]   │                                          │
│                          │                                          │
│ ┌ LEGEND ──────────────┐ │                                          │
│ │ Advance / tooth      │ │                                          │
│ │ ▐▓▓▒▒░░▒▒▓▓▌         │ │                                          │
│ │ 0.010  0.025  0.040  │ │                                          │
│ │ mm/tooth · vendor    │ │                                          │
│ └──────────────────────┘ │                                          │
└──────────────────────────┴──────────────────────────────────────────┘
```

Four conventions in that sketch:

- **`·` prefixes a reason line.** It states in one short sentence what is missing.
- **A bracketed button is a compute affordance.** It runs the thing the reason names.
- **`⚙` opens the authoring home** for that overlay. Rest heatmap jumps to `Geometry ▸ Rest Analysis`. Tier map opens the planner.
- **A count in parentheses** reports how many objects the overlay draws, so an empty overlay is distinguishable from a hidden one.

### 6.4 Per-workspace defaults

Today the workspace decides by hard gate. The proposal keeps the decision
and changes the mechanism. A workspace supplies a **default set**, and the
operator may override any member.

| Overlay | Setup | Toolpaths | Simulation |
|---|---|---|---|
| Grid, Stock box, Origin axes, Datum, Fixtures, Keep-outs | on | on | off |
| Alignment pins | on | on | **on** — today the fixture buffer is forced visible in Simulation when pins exist (`app/viewport.rs:465-467`), so pin-drill review keeps working |
| Stock solid | on | off | off |
| Model | on | on | **off — stays a hard gate** |
| Cutting, Rapids, Entry markers | off | on | on |
| Height planes | off | on (with a selection) | off |
| Simulated stock | off | off | on |
| Tool model, Deflection, Scrub reveal | off | off | on |
| Collisions | off | off | on |

Two gates must stay hard, and the panel must say so rather than offer a dead
checkbox:

- **Model in Simulation.** The dexel stock replaces it (`app/viewport.rs:451-457`). Show the row disabled with "replaced by the simulated stock here".
- **Tier map outside its own setup.** It draws at a wrong offset in another setup's frame (`app/viewport.rs:477-483`). Show it disabled with "previewed on Setup 2 — switch setup to see it". This also closes Finding 6.

Everything else in §1's gate list becomes a default.

### 6.5 Mutual exclusion

The brief asks for one per-vertex colour source on the model at a time. The
code disagrees in one place, deliberately: the rest heatmap and the tier
preview hold **separate GPU slots, separate uniform buffers and separate
draw calls** so both can draw in one frame (`render/mod.rs:159-166,
349-380, 1053-1080, 786-790`; `app/viewport.rs:476-478`).

**Resolution adopted here.** Separate two ideas.

- A **territory** overlay paints a region label over an area. The tier map, the derived rest regions, the planner islands, the boundary outline and the monotone cells are territories. They stack, and the code's independence is correct.
- A **scalar field** overlay paints a continuous value per vertex or per move. The rest heatmap, the reach map, deviation, and height are scalar fields.

Apply exclusion **per surface**, and only among scalar fields:

| Surface | Radio group | Members |
|---|---|---|
| Model mesh | one at a time | none · Reach (S11) |
| Simulated stock | one at a time | Solid · Deviation · By height (S3) |
| Toolpath moves | one at a time | Palette · Engagement · Advance-per-tooth (T7) |

**Every row in the Regions group is a checkbox, not a radio.** The operator
asked for "a checkmark for region, rest, etc.", and territories stack by
design. R1 the rest heatmap is the one scalar field in that group, and it
keeps a checkbox so the shipped rest-plus-tier pair still works.

One **cross-group** rule covers the remaining conflict. The reach map and
the rest heatmap both shade the model surface, so switching on Reach clears
`Regions ▸ Rest heatmap`, and the panel prints the reason on both rows. That
is a UX exclusion, not a renderer limit: R1 drapes a generated heightmap
mesh, and the reach map colours the model mesh itself (Finding 9). Nothing
else in the panel excludes anything else.

**One legend per active scalar field.** The legend block at the foot of the
panel shows one row per active field. The existing rest legend moves there
from the viewport strip (`ui/viewport_overlay.rs:303-310`), and the five
overlays with no legend today gain one (§2.5). Follow the rest legend's
rule: build the legend from the same colour function the mesh uses
(`ui/viewport_overlay.rs:313-327`), so it cannot drift. `reach_color`
(`reach_map.rs:403`) is the reach map's equivalent, and it is already
public.

### 6.6 The Regions group

Every region kind that reaches a dropdown must reach this group. Today four
do not draw at all.

| Region kind | Today | Under the proposal |
|---|---|---|
| Rest heatmap (R1) | `Show ▼` checkbox | Regions ▸ checkbox, `⚙` to Rest Analysis; cleared by Reach (§6.5) |
| Derived rest regions (R3) | a boundary-source combo, **no drawing** | Regions ▸ checkbox that **draws the source op's regions**; the combo stays the authoring home |
| Tier map (R4) | `Show ▼` checkbox | Regions ▸ checkbox, `⚙` to the planner, with the setup condition on the checkbox (Finding 6) |
| Planner islands (R7) | planner dialog only | Regions ▸ checkbox, disabled with "run a preview" until ready |
| Boundary outline (R5) | **never drawn** | Regions ▸ checkbox — **new render work**, §8 step 4 |
| Monotone cells (R6) | **never drawn**; two dials | Regions ▸ checkbox — **new render work**, §8 step 4 |

Three rules keep this honest:

- **The panel is a viewer, not an editor.** Choosing a rest source or setting a boundary offset stays in the properties panel. `⚙` navigates there.
- **The panel reflects state it did not set.** The planner writes `show_tier_preview` on its own (`controller/events/planner.rs:147,256`; `controller/events/compute.rs:1372`), so the radio must read the flag each frame and never cache it.
- **Rest gets five rows, not one** (Finding 4). Only the heatmap and the regions are overlays. The operation, the analysis and the claims are authoring surfaces, and the panel links to them rather than duplicating them.

### 6.7 What does not move

The per-toolpath eye, `C`, `R` and bullseye stay on the operation row
(`ui/toolpath_row_controls.rs`). They are per-object; the Overlays panel is
per-scene. The panel gains a one-line pointer to them, mirroring the pointer
the Inspector already carries the other way
(`ui/sim_diagnostics.rs:105-124`).

The duplicate "Cut" / "Rapid" checkboxes at `ui/properties/mod.rs:999-1011`
should be replaced by a call to `toolpath_row_controls::draw`, so one state
has one affordance.

### 6.8 Keyboard shortcuts

No overlay has a shortcut (`app/input.rs:438-511`). Add a small set, beside
the existing block, so `ui/shortcuts_window.rs` has one source to follow:

| Key | Action |
|---|---|
| `O` | Open or close the Overlays panel |
| `Shift+O` | Pin or unpin it |
| `S` | Stock box |
| `P` | Cutting paths |
| `R` | Rapids |
| `X` | Collisions |
| `,` / `.` | Step the active model or stock colour source |

`I` and `H` keep their meanings, and `1`-`4` keep the camera presets.

---

## 7. Alternatives considered

| | **A. Popover from a toolbar button** | **B. Docked strip, always visible** | **C. Layer list, image-editor style** |
|---|---|---|---|
| Shape | Today's `Show ▼`, regrouped | A permanent column inside the viewport | Rows with an eye column, drag to reorder, per-row opacity |
| Discoverability | Medium. The operator must know to open it. | High. Every overlay is named on screen at all times. | High. |
| Viewport cost | None | 180-220 px of width, always | 220-260 px, always |
| egui cost | Low. `menu_button` already does this. | Low. A `SidePanel` inside the central panel. | **High.** Reordering needs `dnd_drag_source`. Per-layer opacity needs a uniform and a pipeline change per overlay — today each overlay has its own uniform buffer and its own hardcoded opacity constant (`render/mod.rs:45,53-58,838-846`). |
| Fits `set_ui_view` | Yes. The MCP path writes the flag; the panel is a view of it. | Yes. | Yes, but the MCP surface must then also express order and opacity. |
| Handles "not computed yet" | Yes | Yes | Yes |
| Risk | The list stays out of sight, so Finding 2 partly survives. | Costs width on a 3D view the operator uses to judge geometry. | Draw order is **not** a real degree of freedom here. Depth testing and a fixed draw sequence decide it (`render/mod.rs:1053-1080`), not a list. The control would promise something the renderer does not offer. |

**Recommendation: a hybrid of A and B — one control with a pin.** It opens
as a popover, so nothing costs width by default. One click pins it as a
strip for the two workflows that need it open, use cases (c) and (d). The
pin state persists per workspace. Alternative C is rejected because ordering
and per-layer opacity are not real capabilities of this renderer, and a
control that offers them reports a capability the application does not have.

---

## 8. Migration plan

The migration removes no function. Each row names where a control's function
moves.

| Step | Change | Function moves to |
|---|---|---|
| 1 | Add `OverlayPanelState` beside `ViewportState`: `open`, `pinned`, per-group expanded, per-workspace default-applied flag. | new |
| 2 | Build the panel from a **declarative registry** — one row per overlay carrying label, group, flag accessor, precondition closure, reason string, object count, and optional compute action. | new; the MCP layer reads the same registry in step 8 |
| 3 | Move all twelve `Show ▼` entries into the registry. Keep `Show ▼` as a visible alias for one release, rendering the same registry. | unchanged; two views of one list |
| 4 | Add the 19 overlays that have no control today (§2.5) as registry rows. Two of them (R5 boundary outline, R6 monotone cells) need a new line buffer and draw call; the rest only need a flag. | new |
| 5 | Convert the convenience hard gates (§1) into per-workspace defaults. Keep the two correctness gates and render them as disabled rows with a reason. This closes Finding 6. | `app/viewport.rs:442-530` keeps only the two correctness gates plus the data preconditions |
| 6 | Move the rest legend into the panel's legend block. Add legends for T7 Engagement, T7 Advance-per-tooth, S3 Deviation, S3 By Height and R4 Tier map, each built from the mesh's own colour function. | `ui/viewport_overlay.rs:303-327` moves into the panel |
| 7 | Give `Inspector ▸ View` the same treatment: opacity and the stock colour mode become panel rows. The Inspector keeps its pointer text. Fix the stale W4.3 comment and decide whether `show_stock` should gate the sim mesh (Finding 5). | `ui/sim_diagnostics.rs:87-207` → the Analysis group |
| 8 | Add `overlays` to `SetUiViewParam` (§9). | new |
| 9 | Delete `Show ▼`. Leave `View ▼`, `Shaded ▼`, `Persp ▼` and `Isolate` on the strip. | every entry already lives in the panel from step 3 |
| 10 | Fix all five stale "pencil" strings and the two stale comments (Finding 3). Replace the duplicate Cut / Rapid checkboxes (§6.7). | `ui/viewport_overlay.rs:24-27,169-172`; `state/viewport.rs:64-66`; `ui/properties/mod.rs:4129-4136,4195-4196`; `ui/properties/mod.rs:999-1011` |

**Two automation labels must survive.** `automation::record` registers
`overlay_collision_check` on the `Show ▼` button
(`ui/viewport_overlay.rs:229-236`) and `overlay_cancel_all` on the cancel
button (`ui/viewport_overlay.rs:277`). Step 9 must re-register
`overlay_collision_check` on the `Overlays` button, or on the Collisions row,
before `Show ▼` is deleted. `overlay_cancel_all` is unaffected, because the
compute indicator stays on the strip.

**Suggested order of value.** Steps 1-4 alone close Findings 1 and 2 and
answer the operator's request. Step 5 is the larger change and can follow.
Steps 6, 7 and 10 are small and independent.

**Where the reach map joins.** When the P5 GUI lands it adds one registry row
in the Analysis group's model radio, one reason string
("select a finishing operation" — the condition is
`OperationType::supports_reach_map()`, `compute/catalog.rs:329-360`), one
legend built from `reach_color` (`reach_map.rs:403`), and the two panel
numbers from `unreachable_pct()` and `max_gap_mm`. It also needs the new
coloured-model upload path named in Finding 9. If the registry lands first,
the reach map costs one row rather than a new menu entry.

---

## 9. What the MCP surface gains

Add one optional field to `SetUiViewParam`
(`crates/rs_cam_mcp/src/server.rs:222-240`):

```jsonc
{
  "workspace": "toolpaths",
  "toolpath_index": 9,
  "overlays": {
    "rest_heatmap": true,
    "tier_map": false,
    "keep_out_zones": true,
    "model_colour": "reach",      // none | reach | rest_heatmap
    "stock_colour": "deviation",  // solid | deviation | by_height
    "move_colour": "advance_per_tooth"
  }
}
```

Behaviour mirrors the greyed-with-a-reason rule:

- A key whose overlay **can** draw is applied, and the reply echoes it under `applied`.
- A key whose overlay **cannot** draw is **not** silently dropped. The reply reports it under `refused` with the same reason string the panel prints — for example `{"tier_map": "previewed on Setup 2 — switch setup to see it"}`.
- A key naming a workspace with no viewport (Readiness, screenshot 04) is refused with that reason.
- The registry from step 2 is the single source of both strings, so the panel and the MCP reply can never disagree.

This closes Finding 11 and makes every overlay in §2 reachable by an agent
for `screenshot_gui`. It also gives the reach map an MCP surface on the day
its GUI lands, which
`planning/machine_kinematics_confidence_2026-09-07.md:517-518` asks for.

---

## 10. Not verified

Each item needs a generated and simulated project, or geometry this project
does not carry.

- **G8 keep-out zones, drawn.** The draw call is confirmed (`app/gpu_upload.rs:605-619`). The loaded project carries no keep-out zone, so none was seen.
- **R1 rest heatmap, drawn.** The legend was seen (screenshot 06). The coloured mesh was not confirmed on screen at the camera position used.
- **Every simulation-gated overlay** (S1-S9). This session ran no simulation, by instruction. Their preconditions are read from code, not observed.
- **The `Show ▼` popover.** `set_ui_view` cannot open it. Its contents are transcribed from `ui/viewport_overlay.rs:115-228`.
- **The exact draw-order consequence** of stacking a territory over a scalar field on the same surface. The two existing overlays use separate draw calls in a fixed order (`render/mod.rs:1053-1080`), and this pass did not test a three-way stack.
