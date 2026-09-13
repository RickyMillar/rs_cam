# Feature Catalog

Canonical product-surface reference for `rs_cam`.

For source attribution and upstream lineage, see [`CREDITS.md`](CREDITS.md).

## Product surface

| Component | Role |
|-----------|------|
| `rs_cam_core` | CAM library: geometry, import, tool modeling, toolpath generation, dressups, simulation, feeds/speeds, and G-code |
| `rs_cam_cli` | Batch CLI and TOML job runner |
| `rs_cam_viz` / `rs_cam_gui` | Desktop CAM application built with `egui` and `wgpu` |

## Operations

| Category | Operation | Core module | GUI | Direct CLI | Status |
|----------|-----------|-------------|-----|------------|--------|
| 2.5D | Face | `face.rs` | Yes | No | Shipped |
| 2.5D | Pocket | `pocket.rs` | Yes | Yes | Shipped |
| 2.5D | Profile | `profile.rs` | Yes | Yes | Shipped — when the cut bottom reaches the stock bottom the inspector prints an informational "Through cut of a N mm board · Holding: …" line under Depth and opens the Tabs disclosure by default when no tabs are set (G-THROUGHCUT, 2026-09-10); zero tabs stays valid. Side is Outside / Inside; the schema's `on` value has no generator arm and is not offered |
| 2.5D | Adaptive | `adaptive.rs` | Yes | Yes | Shipped |
| 2.5D | VCarve | `vcarve.rs` | Yes | Yes | Shipped |
| 2.5D | Rest Machining | `rest.rs` | Yes | Yes | Shipped |
| 2.5D | Inlay | `inlay.rs` | Yes | Yes | Shipped |
| 2.5D | Zigzag | `zigzag.rs` | Yes | No | Shipped |
| 2.5D | Trace | `trace.rs` | Yes | No | Shipped |
| 2.5D | Drill | `drill.rs` | Yes | No | Shipped — first-class `OperationFamily` with peck cycles, diameter-scaled `peck_depth` / `plunge_rate_base`, and drill-native metrics (`DrillToolpathSummary` + `drill_gates`) in place of engagement axes. Hole positions come from the model's drill targets — circle-like closed polygons (every vertex within max(2 %, 0.05 mm) of the mean radius, 8+ vertices; `svg_input::circle_like_drill_targets`, layer `circles`) and DXF POINT entities / circle/arc centres with layer attribution — picked via viewport click or per-layer "select all"; default (no selection) drills every target the model exposes. A drawing with none (a star, an outline, a mesh) refuses with "No drill targets — pick points/circles or import a drawing with circles"; the inspector, the static validator and the diagnostics ribbon print the same sentence (G-DRILLCENTROID, 2026-09-10 — before that the default drilled the centroid of EVERY closed polygon, so a drawing with no circles got a hole through the middle of each shape) |
| 2.5D | Chamfer | `chamfer.rs` | Yes | No | Shipped |
| 3D | 3D Finish | `dropcutter.rs` | Yes | Yes | Shipped |
| 3D | 3D Rough | `adaptive3d.rs` | Yes | Yes | Shipped. The planner's stamp now mirrors the emitter's stock-to-leave drape (2026-08-04); before that the planner over-stated removal on curved and steep terrain, whose operator-visible symptom was skipped passes and standing material, not a gouge. Planner/simulator parity is gated directionally as well as by count |
| 3D | Waterline | `waterline.rs` | Yes | Yes | Shipped |
| 3D | Pencil Finish | `pencil.rs` | Yes | Yes | Shipped |
| 3D | Scallop Finish | `scallop.rs` | Yes | Yes | Shipped. `iso_field` dial (2026-09-03, default off): rings from the iso-scallop field — per-point spacing, spec-correct cosine slope law, completion by construction; measured 0.875× the unified op's time at 2.5 pp better envelope coverage on terrain (`planning/metrology_2026-09-02/FINDINGS.md` §M7–M8) |
| 3D | Unified Finish | `unified_finish.rs` + `finish_planner.rs` | Yes | Yes | Experimental (P2.f — regions by true-surface slope, waterline/scallop/raster per region, greedy link-costed routing; sweep-locked defaults 45/75; scallop chord-refinement + tip-radius cusp + serpentine, live-validated; creases fold into the claims pipeline and are live — `unified_finish.rs:22-31`, `ClaimsConfig::crease_reference`; what is NOT built is the region-level territory filter ("S2"): it was built on top of S1's per-cell rest measurement, measured dead on both settings it could take, and removed 2026-07-27 — `unified_finish.rs:33-54`). **The tier-by-tier speed/quality comparison this row used to assert ("−20% at the speed tier", "plain Scallop wins the fine tier") is SUPERSEDED — it was measured through four instrument defects since fixed. See `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`; a dated verdict belongs in planning prose, not in a capability catalog.** |
| 3D | Steep/Shallow | `steep_shallow.rs` | Yes | Yes | Shipped |
| 3D | Ramp Finish | `ramp_finish.rs` | Yes | Yes | Shipped |
| 3D | Spiral Finish | `spiral_finish.rs` | Yes | No | Shipped |
| 3D | Radial Finish | `radial_finish.rs` | Yes | No | Shipped |
| 3D | Horizontal Finish | `horizontal_finish.rs` | Yes | No | Shipped |
| 3D | Project Curve | `project_curve.rs` | Yes | No | Shipped |

## Tooling and setup

### Tool families

- Flat end mill
- Ball nose
- Bull nose
- V-bit
- Tapered ball nose

### Tool metadata exposed in the GUI

- geometry: diameter, cutting length, corner radius, included angle, taper angle
- collision envelope: holder diameter, shank diameter, shank length, stickout
- cutting metadata: flute count, tool material, cut direction
- catalog metadata: vendor, product ID

### Machine and material models

- stock material library in `rs_cam_core::material`
- machine profiles in `rs_cam_core::machine`
- feeds/speeds calculator in `rs_cam_core::feeds`
- one apply funnel for every recommendation write (`feeds::suggest::apply_feeds_subset` → `enforce_invariants`): the Feeds-tab Apply buttons, the modal, the batch and MCP `apply_feeds` write the clamped, rounded operating point. Since G-PILLCLAMP (2026-09-10) the 24 per-field ⚡ pills do too — they read `feeds::suggest::preview_field_applies`, a dry run of that funnel, so a pill offers and writes the same number as Apply and stamps the recommendation's provenance; the one dial the funnel does not write (VCarve Max Depth) offers the raw calculator value and its hover says "not clamped". Sentries: `pill_writes_clamped_value_g_pillclamp.rs`, `apply_contract_a3.rs`
- vendor LUT seeding from embedded observations in `crates/rs_cam_core/data/vendor_lut`

## Toolpath modifiers and control layers

- heights system: clearance, retract, feed, top, bottom
- entry dressups: plunge replacement via ramp or helix
- dogbone overcuts
- lead-in / lead-out arcs
- link moves / keep-tool-down linking
- arc fitting to `G2` / `G3`. **Arcs do not cross an intent boundary** (2026-08-04): the fitter's run key carries `Move::intent` and breaks on `Region` span edges, so a fitted arc's label is exact rather than inherited from its first source move. Fitted arcs are tagged `SpanKind::GeometryRefit` and stay **in** tool-load gate populations; only true dressup bridges (dogbones, links, lead-outs) are excluded
- feed optimization dressup with stock-aware engagement estimation on supported workflows
- air-cut filter dressup: removes cutting moves through cleared stock when using remaining-stock mode
- stock-aware generation: per-toolpath "Use remaining stock" toggle pre-simulates prior operations to build actual material state
- per-operation manual pre/post G-code blocks (editable in GUI, emitted in export)
- TSP rapid-order optimization
- stock-boundary clipping with center / inside / outside containment. **Not unconditional** (2026-08-05): when the boundary offset *collapses*, the path passes through unclipped rather than over-clipped, and the report-only `ToolpathStats::boundary_clip_dropped` finding names it; when the offset *fails* (rejected input or a contained library panic), the operation now refuses instead of silently emitting an uncontained path
- dual compute lanes: toolpath generation plus analysis (simulation / collision)
- lane-status chips and a single `Cancel All` overlay action
- plan-order edits land where the operator puts them (G-DROPINDEX, 2026-09-10): a card dragged inside its setup is INSERTED at the drop position and the ops between the two ends shift by one, and a card dragged into another setup lands at the drop position there instead of at the bottom. Move Up / Move Down are unchanged — they pass adjacent positions, where an insert and the swap this replaced are the same permutation — and MCP `move_toolpath_to_setup`, which has no position argument, still appends
- Generate All submits only ENABLED operations on every project (G-GENALLDISABLED, 2026-09-10); before, a project with no rest-stock chain took a branch that submitted every operation, enabled or not. A project with nothing enabled now says "No enabled toolpaths to generate" rather than starting a run with no work in it
- the rest-stock fixpoint ladder names the simulation resolution it takes over (G-RESNOTICE, 2026-09-10): between generate rounds it pins `simulation.resolution` and unticks "Auto from tool size", and the new value stays after the run, so a run whose cell size differs from the operator's dials now raises a notification carrying the old value, the new value and the control that was cleared. When it changes nothing — the GUI's own Generate All can only write back a value the operator pinned — it stays silent. When and whether the ladder rewrites the setting is unchanged

## Simulation, verification, and export

### Import

- STL mesh import
- SVG vector import
- DXF vector import
- STEP file import (AP203/AP214 via truck crate, face-aware tessellation)
- **"Locate file…" repairs a model whose file has moved** (G-MODELRELINK, 2026-09-10). The model inspector prints the loader's own reason when a model failed to load (`Not loaded: …` — before this, `load_error` was rendered nowhere in the GUI and every failure read "was not found", including a corrupt file sitting where the project said) and offers a browse action that points the model at a different file. The model keeps its id, its name and its declared units, so every operation built on it survives — and every one is invalidated and marked for regeneration, because the geometry changed under it. A relink to a different KIND is refused, naming the operation's Input control as the tool for "use a different model". Before this a project moved between machines had no repair route at all: Reload retried the path that had just failed, and Delete is refused while any toolpath references the model
- **model paths: resolved in memory, relative on disk when the model is under the project directory** (G-MODELRELINK, 2026-09-10). A project folder that carries its own models can be copied or moved and still open. Previously the loader stored the raw string from the file — so a relative path resolved against whatever directory `rs_cam_gui` was launched from — and the save wrote that string back verbatim, making portability depend on how each model happened to have been added. A model outside the project directory is stored absolute, which is the only honest description of where it is

### BREP / face selection

- BREP face picking and selection in the viewport (click to toggle faces on/off)
- Per-face pastel coloring with selection highlighting on enriched meshes
- Face-derived 2D boundaries for 2.5D operations (horizontal planar faces)
- Face-derived containment boundaries for 3D operations
- Face selection persistence in project files (deterministic face IDs from STEP topology)
- BREP topology metadata panel (face count, adjacency, surface type breakdown)

### Export

- G-code: GRBL, grblHAL, LinuxCNC, Mach3 (`PostFormat`; grblHAL differs from GRBL by accepting `M6`/`M7`, which GRBL's post filters out)
- core/session export (used by CLI) skips disabled operations even when their generated result is retained in cache; re-enabling restores that cached operation without regeneration (N1, 2026-09-10; `tests/export_disabled_cached_n1.rs`)
- the GUI / MCP export refuses, naming the operation, when an ENABLED operation has no result (`'X' is still waiting on upstream stock …`, `'X' failed to generate: …`, `'X' is not generated`) instead of emitting the program without it; the pre-flight modal lists each such operation as a blocking row with the same text, and a disabled operation is still skipped (G-EXPORTSKIP, 2026-09-10)
- SVG toolpath preview
- HTML setup sheet
- TOML project/job persistence with editable-state round-trip

### Verification

- tri-dexel stock simulation (Z/X/Y grids). That is a claim about the dexel **engine**, which addresses all 6 cardinal face orientations; it is not a claim that the side-face *workflow* is finished. Setups on `Front`/`Back`/`Left`/`Right` emit G-code, metrics, gates and rest stock correctly (each is simulated in its own setup-local frame, where local Z is always the tool axis), and since 2026-08-22 a 2D drawing on such a setup is consumed in that setup's work plane rather than collapsing to a line. Also since 2026-08-22: the live-scrub viewport shows lateral cuts (G-LATERALSCRUB fixed — a lateral group is replayed in its own frame and mapped out, because the global playback stock's side grids append open surfaces to a closed solid and can never show the cut), and G-DRILLLATERAL — analytic drill removal abstaining on a lateral axis — is closed as **unreachable** the same day: lateral drills are simulated setup-locally where the axis is always Z, and no shipped path passes a lateral direction to the kernel any more. The abstention arm stays as the kernel's honesty contract. One lateral gap remains open: fixtures/keep-out zones are refused rather than projected (G-LATERALKEEPOUT)
- agent-readable MCP toolpath narration (`narrate_toolpath`) for Z-level structure, cut-run vs marching-squares region counts, engagement histogram, suspicious arcs, peak axial DOC, and air-cut summaries — timed at 4 ms on a 12.6k-move pass with a 70k-sample cut trace (2026-08-06)
- operation diagnostics in the GUI ribbon use the same precondition and model-reference contexts as session/MCP diagnosis, including a Rest operation with no prior cut and an unresolved model reference (N9; `ribbon_and_mcp_diagnostic_ids_n4.rs`)
- bounded typed simulation triage (`SimulationTriage`, `ProjectSession::simulation_triage`) — one contract consumed by the GUI diagnostics panel, MCP `get_diagnostics`, the CLI `project` report and narration: safety events, then actions, then capped/deduped advisories with a true pre-cap count. Replaces reading the raw `issue_count`, of which three different quantities ship under one name
- measurability abstention (`sim_measurability::MeasurabilityReport`) — a metric that cannot be resolved at the selected cell reports `NotMeasurable` with a reason and its gate ABSTAINS, instead of a hard zero dressed as a percent clearing every bar. Collision detection is never disabled by an abstention; the GUI prints a `NOT MEASURED:` strip above the panel
- structural toolpath spans (`Operation`, `DepthPass`, `Region`, entry/lead/link/dressup artifacts) propagated into simulation cut samples for span-aware filtering and outline navigation
- playback, scrub, and checkpoints
- tool visualization during playback
- holder/shank collision checks
- deterministic renderless GUI regression harness with stable automation IDs
- literature-matrix feeds validation suite — 56 cited cells × 19 invariants exercising the feeds engine against vendor chipload and RPM envelopes (`crates/rs_cam_core/tests/literature_matrix/` plus the `_litmatrix_*` sentries). The **drill** per-peck and chip-welding bands in it are declared repo-authored, not vendor or handbook: a 2026-08-04 audit retrieved the cited Onsrud drill chart and the FPL Wood Handbook and neither contains peck or depth-to-diameter guidance for wood
- drill ops produce drill-native verification (`DrillToolpathSummary` incl. `per_peck_max_dtd`, `drill_gates` for chip welding / peck adequacy / plunge feed sanity) instead of engagement metrics, which don't apply to Z-only kinematics. The cycle model is rooted at the **R-plane**, matching the emitter, through one shared `drill::fed_descents`
- per-toolpath **kinematic utilization** (`kinematic_utilization::analyse_toolpath`, 2026-09-07) — how hard the machine actually works on a pass: utilization (achieved ÷ commanded feed, time-weighted), the feed-bound share and the headroom a ×1.30 constant-chipload rise would release (`headroom_at_1_30`), the machine-bound share (accel / per-axis rate / junction) that a feed rise cannot move, per-axis rate-bound time, and the plunge-class population with its peak ratio against the operation's own `plunge_rate`. It needs no simulation. Surfaces: the CLI `project` report line, the GUI sim-op-list pill, MCP `narrate_toolpath`'s kinematics sentence, the `ToolpathLoadVerdict::kinematic_utilization` slot in `get_tool_load_report`, and the non-blocking `project.plunge_class_load` finding in triage `actions` (`Caution` above 1×, `Critical` above 2×, `fix: None`). Two honesty notes: every reading is **PLANNED before a simulation and EMITTED after** — the modulator runs post-sim, and each surface states which it read (`FeedsProvenance`); and the **optimizer** path publishes `kinematic_utilization: None`, because it holds no emitted `Toolpath`, so an absent slot means *not measured*, never "clean"
- per-tool **reach map** (`reach_map::compute_reach_map`, P5, 2026-09-08) — which of the model surface a single finishing cutter can actually form, and which it cannot: a per-cell gap in mm between the surface that cutter would leave and the true mesh, an AREA-weighted unreachable percentage and the worst gap. The gap is the model surface against the **machined** surface (a min-filter of the drop-cutter CL plane with the cutter's own `height_at_radius` profile), which is exact on a plane of any slope for every tool shape — a ball, a flat and a tapered ball alike — so it needs no slope estimate and no abstain-above-75° arm. Cached per (mesh identity, tool shape, tolerance, cell) and computed off the render loop. Surfaces: a green/red per-vertex overlay on the model in the Toolpaths workspace whenever a finishing op is selected, the two numbers beside its toggle, MCP `reach_map`, and `screenshot_toolpath`'s `reach_overlay`. Three honesty notes: it is a **top-down** measure, so undersides, walls and a band one envelope radius wide inside the part outline are NOT MEASURED rather than "reachable" (read `is_measured` and `measured_area_mm2` before believing a percentage); the verdict is THREE-WAY, because `discretisation_floor_mm` is measured per cell on the surface the walk built and a gap above the tolerance but under that cell's own floor is `unresolved_area_mm2` — neither reached nor proven missed; and the tolerance is the operation's own declared cusp / scallop height, else the cusp its own raster stepover leaves on the TIP sphere, else 0.05 mm — never `stock_to_leave`, which is an intended offset rather than a bar. Three things moved on 2026-09-08 after a live look reported a terrain painted almost entirely red (F1-F5). (a) The **floor now measures curvature and the verdict abstains on it**: the old figure was a plane-only profile-sampling shortfall, and the term that bites is `cell^2 / (8 (rho - R))` on a concave feature of radius rho. Measured on the reported case (Ø4 tapered ball, 0.645 mm cell) the plane-only term is 0.132 mm and the curvature term 0.234 mm, so the old figure understated the grid's limit by 1.8x — but the larger half of the finding is that 0.132 mm was ALREADY far above the 0.05 mm bar it was read against, and **no operator surface compared the two**. `profile_floor_mm` and `curvature_floor_p95_mm` are now published apart, `tolerance_below_floor` says when the bar is under the arithmetic, and the band between them is `unresolved_area_mm2` rather than unreachable. **Which way the error runs is now stated and was stated BACKWARDS until 2026-09-08**: the sampled minimum sits at or above the continuum minimum, so the gap bias is non-negative, the unreachable percentage OVER-states and the true share is AT OR BELOW it — measured 59.05 against a true 58.6 % at 0.05, 51.11 against 42.1 at 0.146, 36.36 against 26.8 at 0.30. `reached` is the sound side: a cell called reached really is formed. Same tool and cell, before -> after at the three bars: 68.2 % -> **59.05 %** (9.14 % unresolved) at 0.05, 51.11 % (1.71 %) at 0.146, 36.36 % (0.03 %) at 0.30. An independent 0.15 mm rasterisation of that STL puts the truth at 55.0 / 39.5 / 25.2 %, so the error at the default bar falls from +13.2 pp to +4.05 pp — and the red was mostly REAL: that mesh's own facet roughness puts its median gap at the 0.05 mm bar. The residual is a slope-KINK regime the curvature term does not bound (it is a C2 bound); ledgered in `curvature_floor_plane` with a measured table, not fixed. (b) The **bar is derived from the raster**: `drop_cutter` declares a stepover, not a scallop height, so every raster finish fell to 0.05 mm; a 1.5 mm stepover on a R2.0 tip has a 0.146 mm cusp, where the same terrain reads 39.5 %. (a2, P5.2 2026-09-08) The **area base is stated on every surface** and it is not a detail: both halves of the ratio are TRUE 3D SURFACE AREA over the rim-eroded population, and a planar whole-board comparison of the same closing reads 5 points lower on a terrain whose mean sec(theta) is 1.34. An `area_basis` line now rides the MCP reply. Put on that base the independent rasteriser reads 58.6 / 42.1 / 26.8 % against the map's 59.05 / 51.11 / 36.36 %, so 3.5 of an apparent 12-point offset was the rasteriser's own base; the rest is the documented discretisation, which survives the tolerance because it is additive in the GAP (+0.054 mm at the median) rather than in the percentage. The Ø4 taper's CONE was tested as a candidate and contributes **+0.01 pp** — it rises 19 mm per mm of radius, so it almost never rests on a neighbouring flank. (a3) The overlay colours by **GAP DEPTH**, log-scaled from the bar to the deepest gap, with unresolved cells in neutral grey and the ramp's stops in the panel legend; the old ramp saturated at 5x the tolerance, so a 0.3 mm near-miss and a 4.5 mm gorge floor were the same red and the distribution's shape — the thing that says which valleys need the finer tool — was invisible. `tolerance_source` names the rung. (c) The **cell no longer moves with the tolerance** — three probes of one tool came back on 0.645, 0.75 and 0.625 mm, so three percentages meant to be compared sat on three grids; the cell now follows the tool's tip sphere and the model bbox, and `cell_mm` plus the floor lead the MCP reply (`grid_note`) and the panel legend
- modulator **geometric plunge guard** (`BindingConstraint::PlungeRate`, 2026-09-07) — a vertical-dominant fed move is capped at the operation's own `plunge_rate` whatever its intent tag says, using the same classifier the utilization instrument uses. It closes the gap where a generator emitted a step-down descent as a plain cutting move and the feed modulator lifted it to the lateral chipload band. It does NOT reach a move that already carries a plunge intent — those keep their commanded feed by design

### Viewport overlays

- **Overlays panel** (P6, 2026-09-08) — one surface listing **every** viewport overlay in four groups (Geometry 12, Toolpath 10, Regions 5, Analysis 12 rows). Opened from the `Overlays (n)` button on the viewport strip or with `O`; `Shift+O` pins it as a column inside the viewport. The rule it implements: every overlay is listed always, one that cannot draw is greyed with a one-line reason, and where an action would make it drawable the reason carries that button (Run simulation, Run collision check, Plan…, Generate all, Record & re-generate, Rest Analysis…). One declarative registry (`crates/rs_cam_viz/src/ui/overlays/registry.rs`) feeds the panel, the MCP surface and the completeness sentries, so the three cannot disagree about what exists or why it cannot draw
- it replaces `Show ▼` (twelve unrelated toggles in one popover), the `Inspector ▸ View` display section (opacity, stock colour modes, generator steps) and the duplicate per-toolpath Cut / Rapid checkboxes in the properties panel. The per-toolpath eye / C / R / bullseye stay on the operation row — they are per-object, and the panel points at them
- nineteen overlays that no control named now have one: the model surface, the solid stock, the origin axes, the datum crosshair, keep-out zones, alignment pins, the flip axis, the orientation gizmo, entry markers, the height planes, the simulated stock and the tool-deflection panel among them. Five of those used to ride ONE checkbox labelled "Fixtures"
- **one colour source per surface**: the model (Reach or the rest heatmap), the simulated stock (Solid / Deviation / By height) and the move lines (Palette / Engagement / Advance per tooth). Enabling one clears the others on that surface. On the model the operator ruled which side owns the default: **Reach is ON in the Toolpaths workspace** — "show the reach map when ANY finishing op is selected" — and the **rest heatmap is OFF** in every workspace, as the diagnostic switched on to inspect rest regions. Switching either on clears the other, and the panel prints the reason on both rows. Region overlays are territories and stack. Each active scalar field draws a legend built from the SAME colour function the mesh or line buffer uses, so a legend cannot drift from what is drawn — five colour-carrying overlays had no legend before
- **per-workspace defaults** replace six convenience hard gates: a workspace supplies a default set and the operator may override any member; leaving the workspace restores what its defaults displaced. Two gates stay HARD and render as disabled rows with a reason — the model in Simulation ("replaced by the simulated stock here") and the tier map outside its own setup ("previewed on setup N — switch setup to see it")
- **MCP**: `set_ui_view` takes `overlays: {"<id>": true|false}` using the registry ids. Nothing is silently dropped — the reply echoes `overlays.applied` and `overlays.refused`, and a refusal carries the same reason string the panel prints. Unknown ids, colour choices switched OFF, and every id in the Readiness workspace (which renders no viewport) are refused. Pair with `screenshot_gui` to photograph what was enabled
- **the viewport draws the SELECTED toolpath only** (WP27, 2026-09-13) — the default in every workspace but Simulation. The operator ruled it: the renderer issues a pipeline, a bind group, a vertex buffer and a draw per resident toolpath per frame, so a project with many rows pays for all of them on every frame. With nothing selected the viewport draws no toolpath — the model and the stock still draw, and the operations list is the picker. Two one-click ways back to all: the `Selected only` / `All toolpaths` button on the viewport strip, and the `All toolpaths` overlay row (MCP: `set_ui_view(overlays: {"all_toolpaths": true})`). The **Simulation** workspace draws every toolpath, because playback reviews the whole program, and leaving it restores the choice the operator had. The isolate pin overrides both: it keeps ONE toolpath drawn while the selection moves. One pure function, `state::viewport::toolpaths_to_draw`, answers for the GPU upload AND the click pick, so a click can never select geometry the viewport does not draw — and the consequence of that is that a toolpath the viewport hides cannot be clicked to select it
- three rows are listed **permanently disabled and say so**: derived rest regions and the boundary outline read "not drawn yet (no renderer)", and the planner islands read "drawn by Regions ▸ Tier map" (they are the territory the tier map paints, so a flag of their own would be a second writer of one state)

### MCP authoring surface

- **GUI-path toolpath add and the toast stack** (F3.1 / F3.5, 2026-09-10) — `add_toolpath_via_gui(operation_type, setup_index?)` adds a toolpath by dispatching the Add menu's own `AppEvent::AddToolpath` through `handle_add_toolpath`, so the GUI's binding (`tools().first()`, `models().first()`) and its add-time Suggest refusal run rather than being reimplemented; where that path refuses, nothing is created. It is the route that makes "what would the operator see" answerable from a test, and it is what the refusal contract will be driven from. Because the GUI add path reports a refusal only by pushing a toast, the reply reads it back off the notification stack: `created` with the new index, id and bound tool/model, or `created: null` plus `refusal` and the toasts the call pushed. `get_notifications` publishes that stack — newest first, with severity, age since the push, TTL by severity and whether it is still visible — and is **read-only**, so a reader cannot consume what the operator is still looking at. The stack is not persisted and is empty after a restart. Sentries: `add_toolpath_via_gui_g_guiadd.rs`, `get_notifications_g_toastread.rs`
- **toolpath rebinding** (F3.7 / F3.8, 2026-09-10) — `set_toolpath_tool(index, tool_id)` and `set_toolpath_model(index, model_id)` change a toolpath's cutter and its geometry input over MCP. Before them the two fields were reachable only from the GUI inspector's Tool: / Input: combos, so an operation blocked on its tool shape or holding a missing model reference was a dead end for an agent: `set_toolpath_param` writes the OPERATION's params and refuses both keys ("unknown parameter 'tool_id' for Pocket operation…"), and that refusal is kept — the two namespaces stay disjoint. Both take a project-assigned **id**, not a positional index, and echo the resolved tool / model in the reply. A rebind invalidates the toolpath's own result and every downstream operation that machines the stock it leaves (`ProjectSession::set_toolpath_tool` / `set_toolpath_model` → `invalidate_result_chain`), which is strictly more than the GUI combos do today — they write the same fields and invalidate nothing. `set_toolpath_tool` deliberately allows a tool the operation's shape constraint rejects, so a blocked operation stays repairable; the generator is still the one that refuses. Sentries: `toolpath_rebind_g_mcprebind.rs` (core behaviour), `mcp_rebind_surface_g_mcprebind.rs` (registration and schema)

### Provenance gates

- source-freshness reporter — flags warn/stale vendor citations in `crates/rs_cam_core/tests/literature_matrix/sources.toml`, plus offline `citation_url` shape validation. The clock runs: it was frozen until 2026-08-04 because its default "today" equalled the seed date of 30 of 32 rows, so no row could ever age
- procedural analytic reference fixture (ARP-1, `crates/rs_cam_core/tests/common/reference_plate.rs`) — closed-form height and normal everywhere, tessellated to a measured rule; worst-zone p99 tessellation error 4.54 um against a 50 um grid-alias floor at the finest cell this repo has ever simulated. Quality bins are qualified against it, not against `terrain.stl`
- `/refresh-lit-matrix` skill — guided re-verification or replacement of stale citations

## Known partial areas

These features exist in state, UI, or helper code, but are not yet end-to-end complete:

| Area | Current state |
|------|---------------|
| Project save/load | editable state round-trips and model files are re-imported on load, but computed toolpaths, simulation checkpoints, and collision outputs are not persisted |
| Controller-side compensation | `G41` / `G42` output is not yet implemented; the “In Control” option has been hidden from the Profile UI until it is wired |
| Feed-optimization dressup | Supported only for fresh-stock, flat-stock workflows with known stock bounds; remaining-stock workflows use the air-cut filter instead |
| Rapid collision rendering | Core collision detection exists, but rapid collisions are not yet rendered in the viewport |
| Region-outline overlays | The Overlays panel lists derived rest regions and the machining-boundary outline, permanently disabled with "not drawn yet (no renderer)". Both are authored as combos and numbers in the properties panel and neither reaches the 3D view. A line-list upload is the shape of the fix; core's boundary resolver is `pub(crate)` and the model-silhouette source rasterises the mesh, so it is not a free one |
| Simulation deviation colors | Wired end-to-end (`StockVizMode::Deviation` → `sim_render::deviation_colors`, `crates/rs_cam_viz/src/app/gpu_upload.rs:66-77`). The real limitation: this path colors per-VERTEX deviations, which are corner-bilinear averages over 2×2 dexel columns and are for DISPLAY only — quality work (fidelity histograms, on-size verdicts) must read the unaveraged per-column instrument, `SimulationResult::column_deviations` / `ColumnDeviation` (`crates/rs_cam_core/src/compute/simulate.rs:245-262`) |
| Vendor LUT integration | Fully wired: embedded Amana vendor observations are auto-loaded at startup via `LazyLock` and passed into the feeds calculator for all GUI operations |
| BREP face selection scope | Face-derived boundaries work only for approximately-horizontal planar faces; non-planar and tilted faces produce no polygon (falls back to stock bounds). Surface classifier is heuristic (axis-aligned planes only). |
| BREP hover highlighting | Rendering path supports hover colors, but hover face tracking is not yet wired (face under cursor is not detected on mouse move) |
| ~~Workholding rigidity UI~~ | Fully wired: GUI ComboBox (Low/Medium/High) on stock panel, passed through to feeds calculator |
| 2D offset failure reporting | `offset_polygon_reported` distinguishes collapse / rejected input / library failure, and four families (pocket, profile, trace, zigzag) opt in and publish `ToolpathStats::offset_library_failures`. The other consumers still call the plain name and honestly report `None` = not measured. The count is per offset **call**, not per ring |
| Offset panic classes in debug vs release | Three `debug_assert!` sites (two in transitive dependencies) are caught in debug and **not** in release, where the library proceeds on unvalidated input. Accepted and documented, not measured; `offset_library_failures` is not comparable across builds |
| UnifiedFinish band residual | The band mix's off-part run-off is fixed and measured at zero, but ~200 um of overcut remains on the grooved reference fixture, located to the rim/wall break line and attributed to the shallow raster band. Open with a named single-column follow-up |
| Drill gate tool divisor | All three drill gates divide by the tool's **envelope** radius with a flat profile hardcoded, and `Drill` carries no tool precondition. On a tapered ball this overstates diameter and the gates read `Within` on an overloaded cutter — a silent pass. Tracked in the radius programme |

## CLI surface

Verified direct CLI commands (T9 cull, 2026-06-07 — the ~13 per-op
subcommands were replaced by the registry-driven generic `run`):

- `version` — build info for all workspace crates
- `job` — TOML job file (multi-tool/multi-op batch; executes through the session pipeline)
- `run` — ANY of the 23 operations: `run <op> --input model --tool type:diameter --set k=v --output out.nc`; `run --list-ops` / `run <op> --list-params` print the registry
- `sweep` — parameter sweep over a job file with fingerprint diffs
- `project` — GUI project file (format_version=3) full-diagnostics executor
- `smoke` — F-037 smoke baseline suite
- `nc-time` — G-code cycle-time prediction

Every operation in the registry is reachable from the CLI; a new
operation appears in `run` with zero CLI code.
