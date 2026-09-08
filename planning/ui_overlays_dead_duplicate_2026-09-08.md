# Viewport overlays — dead and duplicate functionality

**Date:** 2026-09-08
**Type:** audit. No code changed. No `cargo` command ran.
**Baseline:** git `master` (`4a7a84f3`). The shared checkout sits on
`reach-map-p5` with another agent's uncommitted work. This audit reads a
clean `git archive master` export, so every line cite below is a master
line.
**Input:** `planning/ui_overlays_ux_2026-09-08.md` §2, 42 rows.

All viz paths are relative to `crates/rs_cam_viz/src/`.

## 0. How to read the classification

| Class | Meaning |
|---|---|
| **WORKS** | The chain runs from the widget to a draw call. No dead gate. |
| **DEAD CONTROL** | A widget writes a flag, and no pixel follows. The gate is named. |
| **DEAD CODE** | A control writes state, and no render path consumes it. |
| **DUPLICATE** | Two or more widgets write one flag. |
| **SILENT COUPLING** | One control drives several overlays. |
| **OUT OF SCOPE** | The row is not a viewport overlay. |

Two mechanisms decide whether a flag reaches the screen, and they behave
differently. A **draw-time** flag rides `ViewportCallback`
(`render/mod.rs:772-811`), which `app/viewport.rs:442-530` rebuilds every
frame. It takes effect at once. An **upload-time** flag is consumed in
`app/gpu_upload.rs`, which runs only when `take_pending_upload()` fires
(`app.rs:666-668`). An upload-time flag with no `set_pending_upload()`
writer takes effect only when an unrelated event triggers the next upload
pass. This audit calls that condition **stale**. The codebase already
names the defect class at `controller/events/model.rs:36-47` (V13).

---

## 1. The table

| # | Overlay | Class | Evidence | Verdict | Rationale |
|---|---|---|---|---|---|
| G1 | Grid | WORKS | `ui/viewport_overlay.rs:117` → `app/viewport.rs:458` → `render/mod.rs:947-951` | KEEP | Draw-time flag, complete chain. |
| G2 | Model mesh | **DEAD CONTROL** | `ui/viewport_overlay.rs:74-77` writes `RenderMode::Wireframe`; the only reader is `app/viewport.rs:456`, which sets `has_mesh=false`. `render/` holds no wireframe pipeline and no line-list model topology. The STEP loop (`render/mod.rs:1002-1011`) is outside the gate. | **FIX** | "Wireframe" draws nothing. It hides an STL model and does nothing to a STEP one. See §3.1. |
| G3 | Stock box | WORKS (3D) / **DEAD CONTROL (2D)** | `ui/viewport_overlay.rs:118` → `app/viewport.rs:459-464` → `render/mod.rs:955-961`. The gate needs `model.mesh.is_some()`, and a 2D model carries `mesh: None` (`rs_cam_core/src/session/project_file.rs:867`). | **FIX** | The buffer is built unconditionally (`app/gpu_upload.rs:518`); only the gate suppresses it. See §3.2. |
| G4 | Solid stock | SILENT COUPLING | `app/viewport.rs:470` — `show_stock && workspace == Setup`. **No mesh clause**, unlike G3 and G5. | MERGE-INTO G3 | Third consumer of one flag, and the only one without the mesh gate. |
| G5 | Origin axes | SILENT COUPLING | `app/viewport.rs:512-517` → `render/mod.rs:967-973`. Mesh-gated like G3. | **FIX** | No control names the axes. They vanish with the stock box and on 2D jobs. |
| G6 | Datum crosshair | SILENT COUPLING | Built into `fixture_data` at `app/gpu_upload.rs:668-736`; drawn under `show_fixtures` at `render/mod.rs:977-983`. | **FIX** | Rides a checkbox labelled "Fixtures". Its Setup-only clause is upload-time; see §3.3. |
| G7 | Fixtures | WORKS | `ui/viewport_overlay.rs:119`; boxes at `app/gpu_upload.rs:583-604`; drawn `render/mod.rs:977-983` | KEEP | The one control of the five-way coupling. |
| G8 | Keep-out zones | SILENT COUPLING | `app/gpu_upload.rs:605-618`, same buffer, same flag | **FIX** | No control names keep-outs. |
| G9 | Alignment pins | SILENT COUPLING | `app/gpu_upload.rs:623-636`, same buffer, same flag | **FIX** | Drawn in all workspaces, unlike G7 and G8, under a label that omits them. |
| G10 | Flip-axis centreline | SILENT COUPLING | `app/gpu_upload.rs:638-663`, same buffer, same flag | **FIX** | Same buffer, no name. |
| G11 | Curves (DXF/SVG) | WORKS | `ui/viewport_overlay.rs:120` → `app/viewport.rs:468-469` → `render/mod.rs:1081-1087` | KEEP | Complete chain. |
| G12 | Orientation gizmo | WORKS | `app/viewport.rs:536`, painter at `:614-660` | KEEP | Always on by design. egui overlay, not GPU. |
| T1 | Cutting moves | WORKS | `ui/viewport_overlay.rs:122` → `app/viewport.rs:492` → `render/mod.rs:1126-1129` | KEEP | Draw-time. Immediate. |
| T2 | Rapids | WORKS | `ui/viewport_overlay.rs:123` → `app/viewport.rs:493` → `render/mod.rs:1130-1133` | KEEP | Draw-time. Immediate. |
| T3 | Per-toolpath cut / rapid | **DUPLICATE** | Two writers of one map entry: `ui/toolpath_row_controls.rs:50,75,101` and `ui/properties/mod.rs:1002-1011`. Read `app/viewport.rs:495-500` → `render/mod.rs:1122-1133`. | **MERGE-INTO** row controls | Neither write is ignored. See §3.4. |
| T4 | Per-toolpath visibility | WORKS | Three homes, one event: `ui/toolpath_row_controls.rs:41`, `ui/toolpath_panel.rs:585`, `app/input.rs:488` → `controller/events/mod.rs:122-126` sets `pending_upload` → consumed `app/gpu_upload.rs:978` | KEEP | Upload-time, and its writer fires the upload. Contrast T8. |
| T5 | Isolation | WORKS | `ui/viewport_overlay.rs:254-260`, `ui/toolpath_row_controls.rs:104-132`, `app/input.rs:476-481` → `controller/events/toolpath.rs:415-422` sets `pending_upload` → `app/gpu_upload.rs:912,979-982` | KEEP | Three homes, one event. Benign. |
| T6 | Span-kind filter | **DEAD CONTROL** in 2 of 3 colour modes | `ui/viewport_overlay.rs:130-157`. `app/gpu_upload.rs:1104` passes the filter only in the `Normal` arm. The `Engagement` (`:1065-1070`) and `AdvancePerTooth` (`:1089-1095`) builders take no filter. | **FIX** | Confirmed at the builder, not only the comment. See §3.5. |
| T7 | Toolpath colour mode | WORKS | `ui/viewport_overlay.rs:191-227` → change detector `app.rs:644-648` fires the upload → `app/gpu_upload.rs:1063-1107` | KEEP | Upload-time, and it has a detector. It is the model T8 should copy. |
| T8 | Tool-profile ghost | **DEAD CONTROL** (stale) | Four sites only: `ui/viewport_overlay.rs:190`, `state/viewport.rs:62`, `:130`, `app/gpu_upload.rs:1015`. **No `set_pending_upload` anywhere.** | **FIX** | Its two upload-time peers have detectors at `app.rs:644-656`; this one has none. See §3.6. |
| T9 | Entry markers | SILENT COUPLING | Built when selected, `app/gpu_upload.rs:993-1014,1113-1121`. Drawn `render/mod.rs:1136-1141` with **no** `show_cutting`, `toolpath_move_visibility` or `toolpath_move_limit` gate. | **FIX** | T1 off does not hide them, and sim scrub does not trim them. |
| T10 | Height planes | WORKS | `app/viewport.rs:471-472` → `app/gpu_upload.rs:1143-1170` → `render/mod.rs:1040-1048` | KEEP | No toggle by design. Selection drives it. |
| R1 | Rest heatmap | WORKS | `ui/viewport_overlay.rs:164-172` → `app/viewport.rs:473-475` → `render/mod.rs:1053-1063` | KEEP | Chain complete. Its hover text is wrong; §3.7. |
| R2 | Rest legend | WORKS | `ui/viewport_overlay.rs:306-310`, painter `:328-356` | KEEP | Built from `rest_ramp_color`, the mesh's own function. The legend precedent to copy. |
| R3 | Derived rest regions | **DEAD CODE** | Written `ui/properties/mod.rs:4144-4146` as `BoundarySource::DerivedRestRegions`. Consumed only as a generation-time clip (`controller/events/compute.rs:602`, `compute/worker/execute/mod.rs:122`). **No consumer in `render/` or `gpu_upload.rs`.** | **FIX** (add render) | The operator confines a toolpath to a region set and never sees it. Wanted per use case (d). |
| R4 | Tier-map preview | WORKS, gate mismatch | `ui/viewport_overlay.rs:177-187` greys on `ready_preview().is_some()` (`app/viewport.rs:267-272`); render additionally needs the previewed setup active (`app/viewport.rs:484-488`, upload `app/gpu_upload.rs:1246-1258`) | **FIX** (checkbox) | The checkbox reads ON while nothing draws. §3.8. |
| R5 | Boundary polygon | **DEAD CODE** | Authored `ui/properties/mod.rs:4050-4281`. A case-insensitive `boundary` search over `render/` and `app/gpu_upload.rs` returns one unrelated comment (`app/gpu_upload.rs:1348`). No buffer, no draw call. | **FIX** (new render) | Wanted per use case (b). New line buffer needed. |
| R6 | Monotone cells (C2) | OUT OF SCOPE | `ui/multitool_planner.rs:317-321` and `ui/properties/operations/surface_3d.rs:941` write `monotone_cell_decomposition`, an **emission** dial (`state/multitool_planner.rs:172,337`). No occurrence in `render/` or `gpu_upload.rs`. | KEEP as a dial | Not a dead control. The UX doc lists it as an overlay; it never was one. Its own hover says "Emission-only". |
| R7 | Planner islands | WORKS | Table `ui/multitool_planner.rs:575-591`. The islands themselves draw through R4 (`app/gpu_upload.rs:1271-1274` passes `preview.islands`). | KEEP | A readout, plus territory drawn by R4. |
| S1 | Simulated stock | WORKS, unhideable | `app/viewport.rs:489-490` reads workspace and `has_results()` only → `render/mod.rs:987-999` | **FIX** | `show_stock` never reaches it. §3.9. |
| S2 | Stock opacity | WORKS | `ui/sim_diagnostics.rs:102` → `app/viewport.rs:491` → `render/mod.rs:843-844` | KEEP | Affects S1 only; solid stock and height planes are pinned at `0.15` (`render/mod.rs:846`). |
| S3 | Stock colour mode | WORKS | `ui/sim_diagnostics.rs:138-168` → `AppEvent::SimVizModeChanged` → `app/input.rs:151-154` sets the upload → `app/gpu_upload.rs:47-86` | KEEP | The only row with an event and a compute affordance. Best precedent in the app (`ui/sim_diagnostics.rs:169-189`). |
| S4 | Tool model | WORKS | `app/viewport.rs:501-503` → `render/mod.rs:1102-1109` | KEEP | No toggle by design. |
| S5 | Scrub reveal | WORKS | `app/viewport.rs:504-511` → `render/mod.rs:1113-1117` | KEEP | Note T9 and T8 ignore the limit. |
| S6 | Collision markers | WORKS | `ui/viewport_overlay.rs:124` → `app/viewport.rs:494` → `render/mod.rs:1091-1099`; buffer `app/gpu_upload.rs:783-885` | KEEP | Draw-time. Complete. |
| S7 | Tool deflection panel | WORKS, no toggle | `app/viewport.rs:293`, `:744-752` | KEEP | egui overlay. The operator cannot switch it off. |
| S8 | Active generator step | WORKS, hidden | `ui/sim_diagnostics.rs:195-207` → `app/viewport.rs:587` | **FIX** (unhide) | `ui/sim_diagnostics.rs:194` hides the checkbox until a trace exists. |
| S9 | Loading pill | WORKS | `app/viewport.rs:544-570` | KEEP | Transient, no toggle. |
| S10 | Hotspot pins | OUT OF SCOPE | Removed by design, `app/viewport.rs:572-583` | KEEP removed | F6.1 decision. Do not re-add without F6.2 tiering. |
| S11 | Reach map | OUT OF SCOPE | No reference in `crates/rs_cam_viz/src/` on master | DEFER | Kernel is untracked work on another branch. |
| S12 | Kinematic utilization | OUT OF SCOPE | `ui/sim_op_list.rs:950-958` | KEEP | A row pill, not a viewport overlay. |
| S13 | Plunge / entry findings | OUT OF SCOPE | `ui/sim_op_list.rs:997-1005`; `ui/sim_diagnostics.rs:614,651-665` | KEEP | Hover text and a triage row, not an overlay. |

### Counts

| Class | Count | Rows |
|---|---|---|
| WORKS | **24** | G1, G3, G7, G11, G12, T1, T2, T4, T5, T7, T10, R1, R2, R4, R7, S1-S9 |
| DEAD CONTROL | **3** | G2, T6, T8 |
| DEAD CODE | **2** | R3, R5 |
| DUPLICATE | **1** | T3 |
| SILENT COUPLING | **7** | G4, G5, G6, G8, G9, G10, T9 |
| OUT OF SCOPE | **5** | R6, S10, S11, S12, S13 |
| **Total** | **42** | |

Five rows carry a second condition: G3 is dead on a 2D project, R4's checkbox
disagrees with its gate, S1 has no toggle, S8 hides its own checkbox, and G6
applies its workspace clause at upload time.

---

## 2. The four correctness items

### (a) The tier-preview checkbox against its render gate — **CONFIRMED**

The checkbox greys on `has_tier_preview`, which `app/viewport.rs:267-272`
computes as `ready_preview().is_some()`. The render gate at
`app/viewport.rs:484-488` adds `state.active_setup_index() == Some(p.setup_index)`.
The upload gate at `app/gpu_upload.rs:1252-1254` adds the same clause. So on
any other setup the checkbox is enabled and reads ON, and nothing draws.

The setup clause is a **correctness** gate and must stay
(`app/viewport.rs:477-483`). The checkbox needs the same clause.

### (b) The Stock checkbox against the simulated stock — **CONFIRMED**

`show_sim_mesh` reads the workspace and `has_results()` only
(`app/viewport.rs:489-490`). `viewport.show_stock` reaches three other
consumers and never this one:

- the stock wireframe, `app/viewport.rs:459-464`
- the solid stock, `app/viewport.rs:470`
- the origin axes, `app/viewport.rs:512-517`

The comment at `ui/sim_diagnostics.rs:90-93` states the opposite: it says the
show-and-hide toggle "lives in the viewport `Show ▼` menu (W4.3: one home for
visibility)". No control hides the simulated stock.

A second inconsistency sits inside those three consumers. The wireframe and
the axes both require `model.mesh.is_some()`. The solid stock at
`app/viewport.rs:470` does not. One flag, three consumers, two preconditions.

### (c) The pencil-only rest-grid strings — **CONFIRMED, and larger than five**

The true condition, from `rs_cam_core/src/compute/execute.rs`:

> Any operation attaches a `rest_grid` and `rest_regions` when its
> `rest_analysis.enabled` is set and a mesh plus spatial index are present
> (guard at `execute.rs:3192-3196`, write at `:3316-3317`). Two further paths
> attach their own artifacts unconditionally, and the generic guard defers to
> them: the pencil `RestDepth` arm (`execute.rs:2127,2130`) and the
> UnifiedFinish claims pipeline (`execute.rs:2541-2542`).

So there are **three** writers, not one. The core pins this with a test at
`execute.rs:5574-5576`, which asserts a Scallop op receives a `rest_grid`.

Note the extra clause the UX doc did not carry: the generic pass needs a mesh
and an index, so a 2D operation never attaches a rest grid.

**Five user-visible strings** carry the stale claim:

| # | Site | Text |
|---|---|---|
| 1 | `ui/viewport_overlay.rs:169` | "Rest-depth heatmap from the pencil rest-depth detector (detector #4)" |
| 2 | `ui/viewport_overlay.rs:171` | "Select a pencil rest-depth toolpath to enable" |
| 3 | `ui/properties/mod.rs:4130-4133` | "rest regions computed by another toolpath's pencil rest-depth detector" |
| 4 | `ui/properties/mod.rs:4135-4137` | "add one and generate it with a pencil rest-depth detector to use as the source" |
| 5 | `ui/properties/mod.rs:4196-4197` | "The toolpath whose pencil rest-depth detector supplies the rest regions." |

**Four code comments** carry it too, and they are why the strings survived:

- `ui/viewport_overlay.rs:24-27` — "only the pencil rest-depth detector populates one"
- `ui/viewport_overlay.rs:161-163` — "populated solely by the pencil rest-depth detector"
- `state/viewport.rs:63-66` — "(pencil detector #4)"
- `app/viewport.rs:243-245` — "only the pencil rest-depth detector populates this"

One MCP parameter description repeats it: `app/mcp.rs:4057-4059`. One further
comment: `mcp_bridge.rs:661`.

Six attribution-only comments say "(pencil detector #4)" without the word
"only": `render/mod.rs:45,187,782,1050` and `app/gpu_upload.rs:1172`.

One string is **correct** and must not be changed: `ui/properties/mod.rs:4310`
is gated on `is_rest_depth_pencil` (`:4300-4307`). The section comment at
`ui/properties/mod.rs:4285-4299` already states the true model, and the MCP
description at `mcp_server.rs:1138` already states it correctly. The GUI
strings contradict the app's own MCP surface.

### (d) The three-site cut / rapid duplication — **CONFIRMED. No write is ignored.**

There are **two** write sites, not three. The third site named in the UX doc
is the render, which reads.

| Site | Widget | Writes | Target |
|---|---|---|---|
| `ui/toolpath_row_controls.rs:50,75,101` | `C` / `R` glyph buttons | `toolpath_move_visibility[tp_id]` | the row's own toolpath |
| `ui/properties/mod.rs:1002-1011` | "Cut" / "Rapid" checkboxes | the same map entry | the row's own toolpath |

Both write the same field of the same entry. Neither routes through an
`AppEvent`, so neither is undoable. Neither sets a pending upload, and neither
needs to: `app/viewport.rs:495-500` re-collects the whole map into the callback
every frame, and `render/mod.rs:1122-1133` reads it there.

**Both writes take effect.** The duplication is a UX defect, not a correctness
one. Two differences matter:

- The row controls grey each button when its global flag is off, and the
  disabled hover names the blocking control (`ui/toolpath_row_controls.rs:61,69-72,87,95-98`).
  The properties checkboxes carry no such gate. They stay clickable and appear
  to work while `render/mod.rs:1126` is ANDing them away.
- The row controls' hover text tracks the current state. The properties hover
  text always reads "Show …", whatever the state.

Both render in one frame in the Simulation workspace: `ui/properties/mod.rs:207`
draws the panel, and `ui/sim_op_list.rs:325` draws the row controls.

**Keep the row controls. Replace the properties checkboxes with a call to
`toolpath_row_controls::draw`.** The row controls are the richer surface, and
they are the one the Inspector already points at (`ui/sim_diagnostics.rs:117-124`).

---

## 3. The consequential findings in detail

### 3.1 "Wireframe" renders nothing — it hides the model

`RenderMode` has one non-UI reader. `app/viewport.rs:456` requires
`render_mode == RenderMode::Shaded` before `has_mesh` can be true, and
`render/mod.rs:1013-1024` draws plain models only under `has_mesh`.

No wireframe pipeline exists. `render/mod.rs:294-323` builds `mesh_pipeline`
with `PrimitiveTopology::TriangleList`. Every model pipeline is a
`TriangleList` (`render/mod.rs:314,410,445,480,635`). The one `LineList`
pipeline (`render/mod.rs:571`) consumes `LineVertex` and never binds
`MeshVertex` or `ColoredMeshVertex` data. `src/` holds no `PolygonMode` at
all. The only `Wireframe` token under `render/` is a doc comment on the stock
box (`render/stock_render.rs:6`).

So the operator picks "Wireframe" from the `Shaded ▼` menu and the model
disappears with no explanation. The UX doc records this menu as G2's only
affordance and does not note that one of its two options draws nothing.

**The behaviour also differs by model type.** `has_mesh` gates only the plain
STL loop (`render/mod.rs:1013-1024`). The enriched STEP loop sits in the same
`else` arm but outside that gate (`render/mod.rs:1002-1011`), so it draws
unconditionally through `colored_opaque_pipeline`. On a STEP project
"Wireframe" therefore changes nothing at all; on an STL project it hides the
model. One control, two behaviours, neither of them a wireframe.

The control is useful as a model hide-and-show switch, which the app otherwise
lacks. **FIX by renaming**, not by deleting: make it `Model: Shaded / Hidden`,
and apply the flag to both loops. Or register the model as an Overlays row and
drop the mode menu.

### 3.2 The stock box and the origin axes never draw on a 2D job

A 2D model carries `mesh: None`
(`rs_cam_core/src/session/project_file.rs:867`, the project-load door).
`app/viewport.rs:459-464` and `:512-517` both require some model to carry a
mesh. So on an SVG or DXF project — the whole pocket, profile, v-carve and
trace family — the stock wireframe and the origin axes are unreachable, and
the "Stock" checkbox does nothing for them.

The buffers are built regardless. `app/gpu_upload.rs:518` assigns
`stock_data` unconditionally, and `render/mod.rs:955-961` would draw it. Only
the callback gate suppresses it, so the fix is one clause.

The solid stock at `app/viewport.rs:470` carries no mesh clause, so it does
draw in Setup on a 2D job. The three consumers of one flag disagree.

### 3.3 The fixture buffer carries five overlays and one workspace clause

`app/gpu_upload.rs:738-746` packs fixtures, keep-outs, pins, the flip axis and
the datum crosshair into one `fixture_data` buffer, drawn by one flag at
`render/mod.rs:977-983`. The UX doc records this. Two details it does not:

- The per-kind conditions are **upload-time**, not draw-time. `in_sim`
  (`app/gpu_upload.rs:582-583`) suppresses fixtures and keep-outs, and
  `app/gpu_upload.rs:668` restricts the datum to Setup.
- `AppEvent::SwitchWorkspace` (`app/input.rs:72-96`) does **not** call
  `set_pending_upload()`. So a workspace change does not itself rebuild that
  buffer.

Read this finding narrowly. The UX doc's screenshots 01, 02 and 03 show the
datum absent in Toolpaths and the model gone in Simulation, so in practice an
upload does fire. This audit identified **no stale frame**. The code fact
stands on its own: `SwitchWorkspace` sets no flag, and every workspace clause
in the fixture buffer depends on some other event to fire one. A future edit
that removes the incidental trigger would expose the gap silently.

### 3.4 One control drives the per-workspace defaults already

`app/input.rs:77-94` saves `show_cutting`, `show_rapids` and `show_stock` on
entry to Simulation, forces cutting and rapids off and stock on, and restores
the three on exit. `SavedViewportState` (`state/simulation.rs:734-738`) holds
them, and all three fields are written and read.

The §6.4 per-workspace default mechanism the UX doc proposes therefore exists
already, in ad-hoc form, for three flags. P6 should generalise this code rather
than add a second mechanism beside it.

### 3.5 The span-kind filter is inert in two colour modes — confirmed at the builder

`app/gpu_upload.rs:1063-1107` dispatches on the colour mode. Only the `Normal`
arm passes the filter (`:1104`). `from_toolpath_engagement` (`:1065-1070`) and
`from_toolpath_advance_per_tooth` (`:1089-1095`) take no filter argument, so
the submenu cannot affect them.

The submenu gives no sign of this. `ui/viewport_overlay.rs:126-129` states the
restriction in a comment the operator cannot read.

### 3.6 The tool-profile ghost is a stale control — the highest-value fix

`show_tool_profile_preview` has exactly four sites in the crate:

| Site | Role |
|---|---|
| `state/viewport.rs:62` | definition |
| `state/viewport.rs:130` | default, `false` |
| `ui/viewport_overlay.rs:190` | the checkbox — writes the flag directly |
| `app/gpu_upload.rs:1015` | the consumer, at upload time |

There is **no `set_pending_upload()` writer**. The flag is correctly carried in
`ToolpathUploadKey::tool_profile` (`render/upload_cache.rs:224`,
`app/gpu_upload.rs:1040`), so the cache would rebuild the buffer — but the pass
that compares the key never runs.

Its two peers each have a per-frame change detector. `app.rs:644-648` watches
`toolpath_color_mode`. `app.rs:652-656` watches `span_kind_filter`, with a
comment explaining exactly why: "the filter is applied at GPU upload … so
toggling a kind needs a fresh upload to take visible effect". The third
upload-time dial was never given the same line.

The checkbox therefore does nothing until the operator selects a toolpath,
generates, moves a fixture, or triggers any of the other upload sites. It then
appears to work, which is why the UX doc scored it a working control.

A second defect sits on the same row: `app/gpu_upload.rs:1015` requires
`selected`, and the checkbox carries no greyed state saying so. That is the
same shape as the tier-preview mismatch in §2(a).

### 3.7 The rest-heatmap hover text sends the operator to the wrong control

Covered in §2(c). The operator reads "Select a pencil rest-depth toolpath to
enable" and cannot learn that `Geometry ▸ Rest Analysis ▸ Compute rest heatmap`
(`ui/properties/mod.rs:4322-4325`) enables it on the op already selected.

### 3.8 The tier-preview checkbox — see §2(a)

### 3.9 The simulated stock cannot be hidden — see §2(b)

---

## 4. Overlay-adjacent dead code

### 4.1 Dead state

| Item | Site | Status | Verdict |
|---|---|---|---|
| **`SimulationState::analytics_tab`** | `state/simulation.rs:761` | **Six UI writers, zero readers.** `ui/sim_timeline.rs:178,207,612,1444,2181,2212` each write it as an explicit "drill into the Safety / CutQuality / DebugTrace tab" affordance. Nothing reads the field. | **FIX** — see §4.4 |
| `ViewportCallback::origin_axes_origin` | constructed `app/viewport.rs:518` | Never read in `render/mod.rs`. The axes geometry is baked at upload from the stock config (`app/gpu_upload.rs:538`); the draw at `render/mod.rs:967-975` uses `resources.origin_axes_data` only. | **DELETE** |
| `ViewportCallback::origin_axes_length` | constructed `app/viewport.rs:523` | Same. The length is recomputed at `app/gpu_upload.rs:537`. | **DELETE** |
| `LineWidthConfig` and `RenderResources::line_width_config` | `render/mod.rs:87-107`, `:207`, constructed `:691` | **No writer, no reader.** Its own doc says "stored but not consumed by the GPU" (`render/mod.rs:84-85`), and the "so the UI can set a desired width" doc at `:69-70` describes a UI that does not exist. | **DELETE** |
| `SimMeshGpuData::generation` | `render/sim_render.rs:215,236`; bumped `:353,:387` | Write-only. The doc at `:57-60` describes a "callers compare against their last-seen generation" protocol no caller implements. The live paths branch on `chunks.len() == 1` (`app/gpu_upload.rs:758`) and the colour fingerprint instead. | **DELETE** |
| `SpanScope::clear`, `SpanScope::is_active` | `state/simulation.rs:494`, `:490` | Zero call sites in `src/`. The `is_active()` hits elsewhere belong to `ComputeLaneSnapshot`. | **DELETE** |
| `JobState::dirty` | `state/job.rs:545` | No writer or reader outside `job.rs`. Legacy migration struct. | **DELETE** |

### 4.2 Dead render functions

All are `pub` items in a `pub mod` chain rooted at `lib.rs:15`, so rustc treats
them as reachable API and emits no `dead_code` warning.

| Function | Site | Evidence |
|---|---|---|
| `MeshGpuData::from_mesh_flat` | `render/mesh_render.rs:138` | Zero callers. Already carries `#[allow(dead_code, …)]` at `:137`. |
| `FixtureGpuData::from_boxes` | `render/fixture_render.rs:14` | Zero callers. Forwards to `from_boxes_and_lines` with an empty slice; the real caller (`app/gpu_upload.rs:741`) calls that directly. |
| `ToolModelGpuData::from_tool_geometry` | `render/sim_render.rs:539` | Zero callers. A wrapper supplying an all-zero `ToolAssemblyInfo`. |
| `ToolModelGpuData::from_tool_assembly` | `render/sim_render.rs:559` | Dead by chain — its only caller is `from_tool_geometry:544`. The live path is `from_tool_assembly_colored` (`:571`) ← `app/simulation.rs:614`. |
| `entry_marker_vertices` | `render/toolpath_render.rs:963` | Zero callers. Checked for indirect dispatch: `EntryStyle` (`:788`) has no `Marker` variant, and `entry_preview_vertices` (`:806`) never calls it. Not re-exported. |
| `UploadStats::is_idle` | `render/upload_cache.rs:285` | Callers are `:523` and `:530`, both inside `#[cfg(test)] mod tests` (`:295-296`). Deleting it also edits the test `stats_deltas_and_idle` (`:507`). Its sibling `since` is live (`app/gpu_upload.rs:1290`). |

### 4.3 Stale comments and an unused parameter

| Item | Site | Verdict |
|---|---|---|
| `viewport_overlay::draw`'s `_sim_active` parameter | `ui/viewport_overlay.rs:17`, passed `app/viewport.rs:277` | **DELETE** the parameter. `sim_active` is computed at `app/viewport.rs:226` for this call only. |
| The W4.3 comment | `ui/sim_diagnostics.rs:90-93` | **FIX** — it states a unification that does not exist (§2b). |
| Four "pencil only" comments | `ui/viewport_overlay.rs:24-27`, `:161-163`; `state/viewport.rs:63-66`; `app/viewport.rs:243-245` | **FIX** (§2c). They are the reason the five strings survived. |

### 4.4 The analytics-tab defect

`SimulationState::analytics_tab` is the one dead-state item that costs the
operator a working affordance today. Six timeline sites write it, and each is
a click that should open a named analytics section. No code reads the field,
so whatever selects the right-panel section is not this state. All six
drill-ins are silent no-ops.

This sits outside the 42 overlay rows. It is reported here because the sweep
found it and because it is the same defect shape as T8: a widget writes state,
and nothing consumes it.

### 4.5 What is clean

`ViewportState` (`state/viewport.rs:51-89`) holds **no** dead field. All
fourteen have a real writer and a real reader. `SavedViewportState`
(`state/simulation.rs:734-738`) is fully live, and so are `stock_opacity`,
`stock_viz_mode`, `debug.enabled`, `debug.highlight_active_item` and
`playback.display_deviations`.

Read that sentence precisely. `show_tool_profile_preview` is live by the
writer-and-reader measure and is still a dead control by §3.6. **A writer and
a reader are necessary, not sufficient** — an upload-time flag also needs
something to fire the upload. A field census cannot find that class of defect;
only the trigger trace can.

No type in `src/state/` derives `serde::Serialize` or `Deserialize`
(`state/job.rs:4` records that the derives were removed). Persistence runs
through `io/project.rs`. So the "serializes but never draws" class is
structurally empty. The only visibility flags that round-trip a project file
are `visible` and `locked` (`io/project.rs:583-584`, `:1178-1179`), and
`visible` is read by the draw path (`app/gpu_upload.rs:978`).

No MCP path writes any `ViewportState` field or any `simulation.debug` field.
`set_ui_view` (`app/mcp.rs:5426-5499`) writes the selection, the pending
properties tab and two modal flags only. This confirms UX-doc Finding 11 at
the state layer: an agent cannot switch an overlay.

---

## 5. Registry input for P6

The KEEP set, with the one canonical toggle site each row should own and the
reason string the panel should print when the row cannot draw. A row with no
site today needs a new one; the table says so.

### Geometry

| Row | Canonical toggle | Precondition reason string |
|---|---|---|
| Grid | `Show ▼ ▸ Grid` (`ui/viewport_overlay.rs:117`) | — always drawable |
| Model | **new** (replaces `Shaded ▼ ▸ Wireframe`) | "replaced by the simulated stock here" — in Simulation only |
| Stock — box | `Show ▼ ▸ Stock` (`:118`), **split from G4/G5** | "this project has no 3D model" — when every model carries `mesh: None` |
| Stock — solid | **new**, split from Stock | "solid stock draws in Setup only" |
| Origin axes | **new**, split from Stock | same as Stock — box |
| Datum crosshair | **new**, split from Fixtures | "no datum set on this setup" / "datum draws in Setup only" |
| Fixtures | `Show ▼ ▸ Fixtures` (`:119`) | "no fixture in this setup" / "hidden during simulation" |
| Keep-out zones | **new**, split from Fixtures | "no keep-out zone in this setup" / "hidden during simulation" |
| Alignment pins | **new**, split from Fixtures | "no alignment pin on this stock" |
| Flip axis | **new**, split from Fixtures | "no flip axis set on this stock" |
| Curves (DXF/SVG) | `Show ▼ ▸ Curves (DXF/SVG)` (`:120`) | "this project carries no 2D curves" |
| Orientation gizmo | **new** | — always drawable |

### Toolpath

| Row | Canonical toggle | Precondition reason string |
|---|---|---|
| Cutting moves | `Show ▼ ▸ Paths (cutting)` (`:122`) | "no toolpath generated yet" |
| Rapids | `Show ▼ ▸ Rapids` (`:123`) | "no toolpath generated yet" |
| Entry markers | **new** | "select a toolpath to see its entry moves" |
| Height planes | **new** | "select a toolpath to see its Z planes" |
| Tool-profile ghost | `Show ▼ ▸ Tool-profile ghost` (`:190`) | "select a toolpath to see its cutter ghost" |
| Hide spans | `Show ▼ ▸ By SpanKind ▾` (`:130-157`) | "applies in Palette colour mode only" |

Per-toolpath eye, `C`, `R` and bullseye stay on the operation row
(`ui/toolpath_row_controls.rs`). They are per-object. The panel carries a
pointer to them, mirroring `ui/sim_diagnostics.rs:117-124`.

### Regions

| Row | Canonical toggle | Precondition reason string |
|---|---|---|
| Rest heatmap | `Show ▼ ▸ Rest heatmap` (`:164-172`) | "the selected toolpath carries no rest grid — switch on Geometry ▸ Rest Analysis and regenerate" |
| Tier map | `Show ▼ ▸ Tier preview` (`:177-187`) | "previewed on Setup N — switch setup to see it" / "run a preview from Toolpath ▸ Plan multi-tool finishing…" |
| Planner islands | **new** | "run a plan preview" |
| Derived rest regions | **new** + new render (§3, R3) | "this operation's boundary source is not Rest Regions" |
| Boundary outline | **new** + new render (§3, R5) | "no machining boundary enabled on this operation" |

Monotone cells (R6) is an emission dial, not an overlay. It should **not**
enter the registry. If a cell overlay is wanted later, it is new render work
and a new row, and the existing dial stays where it is.

### Analysis

| Row | Canonical toggle | Precondition reason string |
|---|---|---|
| Model colour: none / Reach | **new** (P5) | "select a finishing operation" |
| Stock colour: Solid / Deviation / By height | `Inspector ▸ View ▸ Stock color` (`ui/sim_diagnostics.rs:138-168`) | "no deviation data — run a simulation" (the existing precedent, `:169-189`) |
| Move colour: Palette / Engagement / Advance per tooth | `Show ▼ ▸ Toolpath color` (`:191-227`) | "advance per tooth needs a simulation" |
| Simulated stock | **new** (§2b) | "run a simulation" |
| Stock opacity | `Inspector ▸ View ▸ Opacity` (`ui/sim_diagnostics.rs:100-103`) | "affects the simulated stock only" |
| Collisions | `Show ▼ ▸ Collisions` (`:124`) | "no collision check has run" |
| Tool deflection | **new** | "run a simulation" |
| Generator steps | `Inspector ▸ View` (`ui/sim_diagnostics.rs:195`), **unhidden** | "switch on Record generator trace and regenerate" |

Two rules the registry must carry, from this audit:

1. **Every upload-time flag needs an upload trigger.** A registry row whose
   flag is consumed in `app/gpu_upload.rs` must either route through an
   `AppEvent` that sets `pending_upload`, or gain a change detector beside
   `app.rs:644-664`. T8 is what happens when neither exists. The registry is
   the right place to record which mechanism each row uses.
2. **The panel reflects state it did not set.** The planner writes
   `show_tier_preview` on its own (`controller/events/planner.rs:147,256`;
   `controller/events/compute.rs:1372`), and `app/input.rs:77-94` rewrites
   three flags on a workspace change. The panel must read each flag every
   frame and cache nothing.

---

## 6. Cheap correctness fixes

Ordered by value against effort. None needs new render work.

| # | Fix | Change | Site |
|---|---|---|---|
| 1 | **T8 takes effect** | Add a third change detector beside the two that exist: capture `show_tool_profile_preview`, compare to a `last_tool_profile_preview` field, call `set_pending_upload()`. | `app.rs:643-664`, new field beside `app.rs:38-39` and `:193-194` |
| 2 | **"Wireframe" stops hiding the model** | Rename the two menu entries to `Shaded` / `Hidden`, or delete `RenderMode::Wireframe` and give the model a plain visibility flag. Apply the flag to the STEP loop too, so both model kinds behave alike. | `ui/viewport_overlay.rs:61-80`; `state/viewport.rs:92-95`; `app/viewport.rs:456`; `render/mod.rs:1002-1011` |
| 3 | **The stock box draws on a 2D job** | Drop the `model.mesh.is_some()` clause from the stock and axes gates, or widen it to `mesh.is_some() \|\| polygons.is_some()`. | `app/viewport.rs:459-464`, `:512-517` |
| 4 | **The five pencil strings** | Replace with the true condition from §2(c): any operation with `rest_analysis.enabled` and a mesh. | `ui/viewport_overlay.rs:169,171`; `ui/properties/mod.rs:4130-4133,4135-4137,4196-4197` |
| 5 | **The four stale comments** | Same correction, so the strings do not regrow. | `ui/viewport_overlay.rs:24-27,161-163`; `state/viewport.rs:63-66`; `app/viewport.rs:243-245` |
| 6 | **The tier checkbox matches its gate** | Extend `has_tier_preview` with `active_setup_index() == Some(p.setup_index)`, and give the disabled hover the reason "previewed on Setup N — switch setup to see it". | `app/viewport.rs:267-272`; `ui/viewport_overlay.rs:181-187` |
| 7 | **One home for per-toolpath Cut / Rapid** | Replace the properties checkboxes with `toolpath_row_controls::draw`. | `ui/properties/mod.rs:999-1011` |
| 8 | **The W4.3 comment stops lying** | Either state that the simulated stock has no visibility toggle, or make `show_sim_mesh` read `viewport.show_stock`. | `ui/sim_diagnostics.rs:90-93`; `app/viewport.rs:489-490` |
| 9 | **The span submenu says it is conditional** | Show the "applies in Palette colour mode only" note inside the submenu, or disable the submenu outside Palette. | `ui/viewport_overlay.rs:130-157` |
| 10 | **T9 and T8 respect the move gates** | AND the entry-preview and ghost draws with `show_cutting`, the per-toolpath entry, and `toolpath_move_limit`. | `render/mod.rs:1136-1149` |
| 11 | **The generator-step checkbox stays visible** | Show it always, disabled with "switch on Record generator trace and regenerate", following the Deviation precedent ten lines above. | `ui/sim_diagnostics.rs:194` |
| 12 | **The six analytics drill-ins work** | Either read `analytics_tab` where the right-panel section is chosen, or delete the field and the six writes. Decide which, then do it — the current state is a click that lies. | `state/simulation.rs:761`; `ui/sim_timeline.rs:178,207,612,1444,2181,2212` |
| 13 | **Delete `LineWidthConfig`** | Remove the struct, the field and its construction. | `render/mod.rs:67-107`, `:205-207`, `:691` |
| 14 | **Delete the two dead callback fields** | Remove `origin_axes_origin` and `origin_axes_length`; the upload already recomputes both. | `render/mod.rs:805-808`; `app/viewport.rs:518-527` |
| 15 | **Delete the six dead render functions** | §4.2. One of them needs a test edit (`UploadStats::is_idle`). | `render/mesh_render.rs:138`; `render/fixture_render.rs:14`; `render/sim_render.rs:539,559`; `render/toolpath_render.rs:963`; `render/upload_cache.rs:285` |
| 16 | **Delete `SimMeshGpuData::generation`** | Write-only, and its doc describes a protocol no caller uses. | `render/sim_render.rs:57-60,215,236,353,387` |
| 17 | **Delete `_sim_active`** | Remove the parameter and its argument. | `ui/viewport_overlay.rs:17`; `app/viewport.rs:226,277` |

Items 1, 2 and 3 change what an operator sees today. Item 12 is the same class
and sits outside the viewport. Items 13-17 remove code and change no
behaviour, so they are safe to batch.

---

## 7. Not verified

- **Runtime behaviour of S1-S9.** This audit ran no simulation, by
  instruction. Their preconditions are read from code.
- **The visible extent of the §3.3 workspace staleness.** No
  `set_pending_upload()` accompanies `AppEvent::SwitchWorkspace`
  (`app/input.rs:72-96`). Whether the datum crosshair survives visibly into
  the Toolpaths workspace depends on what else fires an upload in that frame.
  The code fact is stated; the pixel result is not observed.
- **The interactive import door for a 2D model.** §3.2 cites the project-load
  door (`rs_cam_core/src/session/project_file.rs:867`). CLAUDE.md records that
  interactive import is a separate door. The two doors have diverged before.
- **T8's stale window in practice.** The four-site census and the missing
  detector are code facts. How long the checkbox appears dead to an operator
  depends on which unrelated event fires next.
</content>
</invoke>
