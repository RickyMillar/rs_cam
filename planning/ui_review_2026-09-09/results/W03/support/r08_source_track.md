# R08 source track — iteration, persistence, background work, recovery

Read-only source investigation for review package R08. All paths are
repo-relative. Every claim below was read from source at the cited line
unless it is marked **HYPOTHESIS**. No GUI was available; section 9 lists
what only a live test can settle.

Vocabulary used throughout:

- **dirty** — `GuiState::dirty` (`crates/rs_cam_viz/src/state/runtime.rs:260`).
  Drives the "Modified" label and the quit dialog.
- **edit_counter** — `GuiState::edit_counter` (`runtime.rs:261`). Both are
  set by one function, `GuiState::mark_edited` (`runtime.rs:338-341`).
- **sim stale** — `SimulationState::is_stale(edit_counter)` is true when
  `edit_counter > last_run.last_sim_edit_counter`
  (`crates/rs_cam_viz/src/state/simulation.rs:1003-1006`).
- **toolpath stale** — `ToolpathRuntime::stale_since: Option<Instant>`
  (`runtime.rs:34`). Viz-only. It is never persisted and never read by
  `ComputeStatus`.
- **core result cache** — `ProjectSession::results` and
  `ProjectSession::simulation` (core). Cleared by the `invalidate_*`
  setters in `crates/rs_cam_core/src/session/mutation.rs`.
- **viz result cache** — `ToolpathRuntime::result` (`runtime.rs:33`), the
  store the Toolpaths-workspace card and the viewport draw from.

---

## 1. Mutation → commit → affected results → visible freshness → undo → persisted

### 1.1 The three commit models in the properties panel

`crates/rs_cam_viz/src/ui/properties/mod.rs::draw` (line 204) runs four
flush helpers on every frame before it draws the selected item
(`mod.rs:212-215`):

| Panel | Commit moment | Undo capture | Equality check before push |
|---|---|---|---|
| Tool (`Selection::Tool`) | **Draft**: edits go to `history.tool_draft`, committed on **Apply** (`mod.rs:375-379`) or **auto-committed on navigate-away** (`flush_tool_draft`, `mod.rs:90-101`). Revert restores the committed tool (`mod.rs:380-382`). | `commit_tool_draft` pushes `UndoAction::ToolChange` (`mod.rs:120-129`). | Yes: `if draft == committed { return; }` (`mod.rs:120-122`). |
| Stock (`Selection::Stock`) | **Live**: `stock::draw` edits `session.stock_mut()` directly (`mod.rs:255`); the `StockChanged` event is emitted by the widget on release. | Snapshot taken on first render (`mod.rs:247-249`); `StockChange` pushed when a `StockChanged`/`StockMaterialChanged` event is seen this frame (`mod.rs:257-268`). | Yes: `old != *state.session.stock_config()` (`mod.rs:261`). |
| Post (`Selection::PostProcessor`) | **Live**: `gui.post` is edited and pushed straight into `session.set_post_config` whenever it differs (`mod.rs:336-345`). | Snapshot on first render (`mod.rs:332-334`); `PostChange` pushed by `flush_post_snapshot` on navigate-away (`mod.rs:143-160`). | **No.** The push and `mark_edited()` happen unconditionally when the selection leaves Post (`mod.rs:149-155`). |
| Machine (`Selection::Machine`) | **Live**: `draw_machine_panel` edits `session.machine_mut()` and emits `MachineChanged` (`mod.rs:1153,1236,1299,1318,1477,1612`). | Snapshot on first render (`mod.rs:349-351`); `MachineChange` pushed by `flush_machine_snapshot` on navigate-away (`mod.rs:163-177`). | **No.** Push and `mark_edited()` are unconditional on navigate-away (`mod.rs:166-172`). |
| Toolpath (`Selection::Toolpath`) | **Live, per frame**: `draw_toolpath_panel` edits a temporary `ToolpathEntry`; `write_entry_config_to_session` writes every field back every frame (`mod.rs:692-693`, `3443-3479`). | Snapshot on first render (`mod.rs:406-416`); `ToolpathParamChange` pushed by `flush_toolpath_snapshot` on navigate-away (`mod.rs:180-202`). | **No.** The push and `mark_edited()` are unconditional when the selection leaves the toolpath (`mod.rs:184-196`). |

Consequence (read from source, not yet observed live): selecting a
toolpath, a machine, or the post processor and then clicking anything else
pushes a no-op undo entry, sets `dirty`, and bumps `edit_counter`, which
flips every simulation readout to "stale". The tool panel and the stock
panel do not have this defect. The sentry
`opening_heights_tab_does_not_pin_heights_or_mark_stale_g_heightstab`
(`crates/rs_cam_viz/src/controller/tests.rs:445-487`) covers rendering the
Heights tab of the *same* toolpath; it does not cover navigating away.
Marked for live test (section 9, Q1).

### 1.2 Which toolpath-panel fields mark the toolpath stale

After write-back, `mod.rs:704-719` compares before/after strings of
`tc.operation`, `tc.heights`, and `tc.boundary` only. When one differs it
sets `stale_since` and calls `mark_edited()`. Fields written back at
`mod.rs:3453-3477` that are **outside** that comparison:

- `tc.tool_id` (combo at `mod.rs:3871`) and `tc.model_id` (combo at
  `mod.rs:3888`): a tool or model **reassignment** in the GUI neither
  marks the toolpath stale nor sets dirty. **HYPOTHESIS** to confirm live:
  the card keeps "OK · N moves" and the project keeps "saved" state after
  swapping a toolpath onto a different tool.
- `tc.dressups` (edited at `mod.rs:4796`, `4804`, `5062`): same gap. No
  `stale_since` site exists after `mod.rs:4531` in the file, and
  `draw_dressup_params` sets none.
- `tc.name`, `tc.enabled`, `tc.coolant`, `tc.pre_gcode`, `tc.post_gcode`,
  `tc.rest_analysis` (except the auto-enable branch at `mod.rs:4529-4531`),
  `tc.stock_source` (this one does set `entry.stale_since` at
  `mod.rs:3959-3965`).

The MCP path does not have this gap: `set_dressup_field`,
`set_dressup_config`, `set_stock_source`, `set_rest_analysis_config` and
`set_boundary_config` all call `mark_edited()` then `mcp_apply_stale`
(`crates/rs_cam_viz/src/app/mcp.rs:4731-4732`, `4694-4695`, `4800-4801`,
`4269-4270`, `4149-4150`).

### 1.3 The matrix

Columns: **Event** (controller or MCP entry), **Commit** (when the session
changes), **Stale mechanism** (what marks dependents), **Simulation** (GUI
`state.simulation`), **Undo**, **Persisted** (in
`crates/rs_cam_core/src/session/project_file.rs`).

| Row | Event | Commit | Stale mechanism | Simulation | Undo entry | Persisted |
|---|---|---|---|---|---|---|
| Tool geometry field (diameter, corner radius, taper…) | GUI: `commit_tool_draft` (`properties/mod.rs:106-140`). MCP: `mcp_set_tool_param` (`mcp.rs:3414-3446`). | GUI: Apply or navigate-away. MCP: immediate. | GUI: `session.invalidate_tool(id)` clears **core** `results` for toolpaths on that tool or on a `PlannedTierRegions` ladder that names it (`mutation.rs:930-957`). **It sets no `stale_since` and clears no viz `rt.result`.** MCP: `mcp_apply_stale(ToolParamChanged)` sets `stale_since` on the same set (`mcp.rs:2627-2649`, `compute.rs:60-71`). | GUI: `invalidate_tool` sets `session.simulation = None` (core); viz `state.simulation` is left, and reads stale only because `mark_edited` bumped the counter. MCP: same. | GUI: `ToolChange {old,new}` (`mod.rs:125`). MCP: none. | Yes: `[[tools]]` (`project_file.rs:177-219`). |
| Tool flute count | Same as above; `flute_count` is a `ToolConfig` field (`project_file.rs:209`). | Same. | Same. | Same. | Same. | Yes. |
| Op geometry field (stepover, DOC, feed…) | GUI: per-frame write-back (`mod.rs:693`), stale check (`mod.rs:704-719`). MCP: `mcp_set_toolpath_param` (`mcp.rs:3301-3347`). | GUI: immediately, per frame. MCP: immediate. | Both set `stale_since` on **that toolpath only** (`mod.rs:715-717`; `mcp.rs:3321`). The core setter also drops `results[idx]` and `simulation` (via `set_toolpath_param`; the undo path's `apply_toolpath_param_snapshot` does the same at `mutation.rs:1079-1080`). Downstream rest ops are marked by the core `invalidate_output_dependents` (`mutation.rs:221-`), core side only. | Kept; stale by counter (`mark_edited` at `mod.rs:718`, `mcp.rs:3320`). | GUI: `ToolpathParamChange` on navigate-away (`mod.rs:187`). MCP: none. | Yes: `[[setups.toolpaths]].operation` (`project_file.rs:411`). |
| Op heights | Same path; `heights_changed` (`mod.rs:708-710`). Also emits `StockChanged` to re-upload planes (`mod.rs:720-723`), which calls `handle_stock_changed` → `sync_alignment_pin_drill` and a second `mark_edited` (`model.rs:630-642`). MCP: `mcp_set_toolpath_heights` (`mcp.rs:3349-3395`). | Immediate. | `stale_since` on that toolpath. | Kept; stale by counter. | `ToolpathParamChange` does **not** carry heights — the snapshot tuple is `(id, operation, dressups, face_selection)` (`state/history.rs:46-51`). **Undo cannot restore a heights edit.** | Yes: `.heights` (`project_file.rs:421`). |
| Op boundary | GUI: `boundary_changed` (`mod.rs:711-713`); a `DerivedRestRegions` pick also stales the **source** toolpath (`mod.rs:741-750`). MCP: `mcp_set_boundary_config` (`mcp.rs:4067-4150`). | Immediate. | `stale_since` on that toolpath (+ source). | Kept; stale by counter. | Not in the snapshot tuple. **Undo cannot restore a boundary edit.** | Yes: `.boundary`, `.boundary_inherit` (`project_file.rs:430-433`). |
| Toggle enabled | GUI: `AppEvent::ToggleToolpathEnabled` → `session.set_toolpath_enabled` (`controller/events/mod.rs:114-118`). MCP: `mcp_set_toolpath_enabled` (`mcp.rs:4755-4764`). | Immediate. | Core: `invalidate_output_dependents(index, true)` and `simulation = None` (`mutation.rs:302-316`). Viz: nothing. `ComputeStatus::effective` derives `Disabled` for display (`crates/rs_cam_core/src/compute/config.rs:44`). | GUI: **no `mark_edited`** at `events/mod.rs:114-118`, so dirty stays false and the sim does **not** read stale. MCP: `mark_edited` (`mcp.rs:4764`). | None. | Yes: `.enabled` (`project_file.rs:413`). |
| Reorder | `handle_move_toolpath_up/down`, `handle_reorder_toolpath`, `handle_move_toolpath_to_setup` (`controller/events/toolpath.rs:236-318`). | Immediate. | None on the viz side (no `stale_since`). Core `reorder_toolpath` (`mutation.rs:137-`) — **HYPOTHESIS**: it does not invalidate downstream rest ops; not read past line 151. | `mark_edited` (`toolpath.rs:252,269,290,316`), so the sim reads stale. | None. | Yes: order of `[[setups.toolpaths]]`. |
| Stock dims / material | GUI: `stock::draw` edits `session.stock_mut()` live; `StockChanged` → `handle_stock_changed` (`model.rs:630-642`); `StockMaterialChanged` → `mark_edited` only (`events/mod.rs:233-235`). MCP: `mcp_set_stock_config` (`mcp.rs:3740-3835`). | Immediate. | GUI: `invalidate_stock` sets core `simulation = None` only (`mutation.rs:918-920`); **no toolpath `stale_since`**. MCP: `mcp_apply_stale(StockChanged)` marks **every** toolpath stale (`compute.rs:76-78`). | GUI: viz sim kept, stale by counter. MCP: same. | GUI: `StockChange` when a stock event fires in the same frame (`mod.rs:257-268`). MCP: none. | Yes: `[job.stock]` incl. material, pins, flip axis (`project_file.rs:71-95`). |
| Setup face / rotation | GUI: `setup::draw` writes `setup_data.face_up`/`z_rotation` directly and emits `FixtureChanged` (`ui/properties/setup.rs:66-68`, `80-81`); handler is `pending_upload` + `mark_edited` (`events/mod.rs:98-101`). MCP: `mcp_set_setup_face`/`_rotation` (`mcp.rs:2771-2804`). | Immediate. | GUI: **nothing** — no `stale_since`, no core invalidation. MCP: `mcp_apply_stale(SetupChanged)` marks that setup's toolpaths (`mcp.rs:2804`, `compute.rs:72-75`). | Kept; stale by counter. | None. | Yes: `face_up`, `z_rotation`, datum (`project_file.rs:294-310`). |
| Machine kinematics | GUI: `draw_machine_kinematics` writes `machine_mut().kinematics` and emits `MachineChanged` (`mod.rs:1476-1477`); handler calls `session.invalidate_machine()` + `mark_edited` (`events/mod.rs:236-239`). MCP: `mcp_set_machine_kinematics` writes kinematics and `mark_edited` (`mcp.rs:3990-4036` region) with no `invalidate_machine`. | Immediate. | None on toolpaths (correct: kinematics affects timing and modulation, not geometry). | Kept; stale by counter. | `MachineChange` on navigate-away (`mod.rs:168`, unconditional). MCP: none. | Yes: `[job.machine]` inline copy (`project_file.rs:56`). |
| Dressup field | GUI: `draw_dressup_params` (`mod.rs:4804`, `5062`). MCP: `mcp_set_dressup_field` (`mcp.rs:4717-4745`). | Immediate. | GUI: **none** (see 1.2). MCP: `stale_since` on that toolpath. | GUI: **not marked edited**, sim reads fresh. MCP: stale by counter. | `dressups` **is** in the snapshot tuple, so a GUI dressup edit is undoable once the selection changes — but only if `flush_toolpath_snapshot` runs, which it does unconditionally. | Yes: `.dressups` (`project_file.rs:419`). |
| Sim resolution / capture options | `sim_op_list.rs:76-101` edits `state.simulation.resolution`/`auto_resolution` directly; `metric_options` is "runtime-only" (`state/simulation.rs:742`). | Immediate. | Not applicable. | The next run uses the new value (`events/simulation.rs:245-247`, `264-269`). | None. | **No.** `ProjectFile` has no simulation section (`project_file.rs:26-39`). |

Not undoable at all (no `UndoAction` variant, `state/history.rs:15-42`):
add/duplicate/remove toolpath, add/duplicate/remove tool, add/remove model,
add/remove setup, fixtures, keep-outs, enabled toggle, reorder, face/rotation,
model rescale/reload, library import. The five `UndoAction` construction
sites are all in `ui/properties/mod.rs` (lines 125, 151, 168, 187, 265).

---

## 2. The live observation: "OK · 725 moves" with no stale badge after `set_toolpath_param`

### What the MCP write did

`mcp_set_toolpath_param` (`mcp.rs:3301-3347`) calls
`session.set_toolpath_param` (core drops `results[idx]`), then
`gui.mark_edited()` (`mcp.rs:3320`), then `mcp_apply_stale` which sets
`rt.stale_since = Some(now)` on that toolpath (`mcp.rs:2643-2646`). It does
**not** touch `rt.status` or `rt.result`. The reply text says
"Regenerate to apply." (`mcp.rs:3339`).

### Why the card still read "OK · 725 moves"

`draw_toolpath_card` (`crates/rs_cam_viz/src/ui/toolpath_panel.rs:259-`)
receives a `RuntimeSnapshot` with exactly five fields — `visible`,
`auto_regen`, `status`, `has_result`, `stats` (`toolpath_panel.rs:28-34`,
built at `126-132`). **`stale_since` is not in the snapshot.** The status
chip is a match on `ComputeStatus::effective(tc.enabled, status)`
(`toolpath_panel.rs:275-278`, `398-409`): `Done` → "OK". Row 3 prints
`stats.move_count` whenever `rt.result` is `Some` (`toolpath_panel.rs:494-504`).
Neither reads `stale_since`, so a stale-but-Done toolpath is drawn exactly
like a fresh one.

The only card-level consumer of `stale_since` is the **rest-dependency
badge**: `draw_rest_badge` paints "dep" yellow when the *upstream* toolpath
has `needs_generation() || stale_since.is_some()` (`toolpath_panel.rs:743-757`).
So a stale toolpath is visible on its **dependents'** cards, never on its own.
The context-menu "Generate" and the quick "▶" button appear only when
`status.needs_generation()` (`toolpath_panel.rs:482-489`), which is false for
`Done`.

The MCP surface *does* publish the flag: `list_toolpaths` rows carry
`"stale": rt.stale_since.is_some()` (`controller/events/compute.rs:2125`;
also `mcp.rs:951`, `1251`).

### Why the pocket was not auto-regenerated

`process_auto_regen` (`crates/rs_cam_viz/src/controller.rs:302-333`) runs on
both pump paths (`app.rs:562`, `951`) and submits any runtime with
`auto_regen && !locked` whose `stale_since` is older than 500 ms
(`controller.rs:312-318`). A pocket's `default_auto_regen()` is used when
the runtime is created (`mcp.rs:3559-3572`; `toolpath.rs:224-229`), but a
runtime created by `open_job_from_path` is `ToolpathRuntime::new(true)` for
every op (`controller/io.rs:180`). So on source alone a 2.5D pocket **should**
have regenerated ~500 ms after each write. Three readings are consistent with
the observation:

1. The three writes (2.1 → 6 → 2.1) landed inside one 500 ms window, and
   the final value equals the original, so the regenerated result has the
   same 725 moves and the card never visibly changed. The "Auto-regenerating
   1 toolpath(s)" toast (`controller.rs:328-331`) lasts 4 s
   (`controller.rs:72`) and is easy to miss.
2. `rt.locked` was true (the lock control in `toolpath_row_controls`), or
   `auto_regen` was false on this runtime. Not determinable from source.
3. The writes happened while the GUI frame loop was parked; the off-frame
   pump exists for this (`app.rs:560-569`) but only runs `process_auto_regen`
   when the event loop wakes.

**HYPOTHESIS**: reading 1. A live test should read `list_toolpaths.stale`
and `generation_status` immediately after a single `set_toolpath_param`
(section 9, Q2).

### Where the workspace-bar chips come from

`crates/rs_cam_viz/src/ui/workspace_bar.rs`:

- Toolpaths tab badge: `"{n} pending"` counts runtimes whose effective
  status `needs_generation()` or is `Computing` (`workspace_bar.rs:113-134`).
  It does **not** count `stale_since`.
- Simulation tab badge: `" stale"` when `sim.is_stale(edit_counter)`;
  collisions outrank it (`workspace_bar.rs:137-156`).
- Readiness tab badge: `"sim stale"` after collisions and `"{n} uncomputed"`
  (`workspace_bar.rs:160-196`). `uncomputed` means `rt.result.is_none()`
  (`workspace_bar.rs:168-177`), so a stale toolpath with an old result is
  **not** uncomputed.

So the two small chips seen live were the Simulation tab's `stale` and the
Readiness tab's `sim stale`, both driven by `edit_counter`, not by
`stale_since`. The status bar shows nothing for toolpath staleness; it has
`Toolpaths: done/total` (`status_bar.rs:31-41`), lane chips
(`status_bar.rs:43-74`), a green `SIM` chip whenever results exist
(`status_bar.rs:76-80`), a collision count greyed "(from a stale run)"
(`status_bar.rs:88-100`), and "Modified" (`status_bar.rs:103-111`).

Result: **no surface in the GUI names a stale-but-Done toolpath directly.**
The only shared "stale" component, `FreshnessGate`
(`crates/rs_cam_viz/src/ui/components/freshness.rs:29-61`), is a
*simulation*-stale banner ("Results stale (params changed) — re-run sim")
consumed by the sim panels (`sim_timeline.rs:53`, `sim_diagnostics.rs:177`,
`sim_op_list.rs:183`, `optimize_modal.rs:39`, `readiness_panel.rs:19`,
`preflight.rs:40`).

Export consequence (R07 owns it, noted here because it is the sharp edge):
`io::export::emitted_toolpaths` reads `session.get_result(idx)` first and
falls back to `gui.toolpath_rt[id].result` (`crates/rs_cam_viz/src/io/export.rs:129-131`).
The doc comment states the fallback exists precisely for the case where
`invalidate_tool` / `apply_toolpath_param_snapshot` dropped the core entry
"while the viz store keeps its result so the UI can draw the stale toolpath"
(`export.rs:102-109`). Neither `preflight.rs` nor `export_wizard.rs`
references `stale_since` (grep over `src/`), so a stale toolpath exports its
pre-edit geometry with no gate.

---

## 3. Undo model

- **Per-item snapshot, not whole-project.** `UndoAction` has five variants:
  `StockChange`, `PostChange`, `ToolChange`, `ToolpathParamChange`,
  `MachineChange` (`crates/rs_cam_viz/src/state/history.rs:15-42`). Each
  stores `old` and `new` whole configs. `ToolpathParamChange` stores
  `operation`, `dressups`, `face_selection` — not heights, boundary,
  tool_id, model_id, enabled, name (`history.rs:29-37`).
- **Stack**: 100 entries, oldest dropped; a new push clears redo
  (`history.rs:85-91`).
- **Shortcuts**: Ctrl+Z undo, Ctrl+Shift+Z redo, read in `menu_bar::draw`
  (`crates/rs_cam_viz/src/ui/menu_bar.rs:10-17`) and listed in the Edit menu
  (`menu_bar.rs:131-144`) and the shortcuts window (`shortcuts_window.rs:17-18`).
  Dispatch: `AppEvent::Undo => self.undo()` (`controller/events/mod.rs:228-229`).
- **What undo restores** (`crates/rs_cam_viz/src/controller/events/undo.rs:7-57`):

  > ```rust
  > UndoAction::StockChange { old, .. } => {
  >     self.state.session.set_stock_config(old);
  >     self.invalidate_simulation();
  > }
  > ...
  > UndoAction::ToolChange { tool_id, old, .. } => {
  >     if let Some(tool) = ...find(|tool| tool.id == tool_id) { *tool = old; }
  >     self.invalidate_simulation();
  > }
  > UndoAction::ToolpathParamChange { tp_id, old_op, old_dressups, old_face_selection, .. } => {
  >     ... apply_toolpath_param_snapshot(idx, old_op, old_dressups, old_face_selection);
  >     if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id) {
  >         rt.stale_since = Some(std::time::Instant::now());
  >     }
  > }
  > UndoAction::MachineChange { old, .. } => {
  >     self.state.session.set_machine(old);
  >     self.invalidate_simulation();
  > }
  > ```

  Read against the matrix:
  - **Does undo re-stale toolpaths?** Only `ToolpathParamChange` sets
    `stale_since` (`undo.rs:46-48`, redo `98-100`). `ToolChange` undo
    replaces the tool **without** `session.invalidate_tool` and without
    `stale_since`, so dependent toolpaths keep both caches
    (`undo.rs:20-31`). `StockChange` and `MachineChange` undo set no
    `stale_since`.
  - **Does undo invalidate the simulation?** Stock, tool, machine: yes,
    via `invalidate_simulation` (`controller/events/simulation.rs:22-31`
    — cancels the Analysis lane and clears results, playback, checks,
    `last_run`). `ToolpathParamChange`: **no.** The core drops
    `session.simulation` (`mutation.rs:1080`), but `state.simulation`
    stays and `is_stale` does not change because **undo never calls
    `mark_edited`** (no `mark_edited` in `undo.rs`). So after undoing a
    parameter edit the Simulation and Readiness chips read fresh, the
    toolpath is stale on the viz side only, and `dirty` is whatever it was.
  - **Does undo restore dependent meaning?** It restores the snapshot
    field values. It does not restore `feeds_provenance` (the snapshot has
    no slot for it; the optimizer apply path sets provenance separately at
    `events/mod.rs:610`), heights, or boundary.
- **Timing surprise**: because the toolpath / post / machine snapshot is
  pushed on navigate-away with no equality check (section 1.1), the first
  Ctrl+Z after inspecting and leaving a toolpath "undoes" a no-op and the
  second one reaches the real edit. Marked for live test (Q1).

---

## 4. Save / reopen

`crates/rs_cam_viz/src/controller/io.rs`:

- **Save** (`io.rs:152-164`): syncs `gui.post` into the session, calls
  `ProjectSession::save`, sets `file_path`, clears `dirty`. Core save is
  atomic: temp file `.rs_cam_save_{pid}.tmp` in the same directory, then
  rename (`crates/rs_cam_core/src/session/save.rs:52-65`). Model paths are
  written as stored (`save.rs:141`, `to_string_lossy`), i.e. whatever path
  the import produced; on load a relative path is joined to the project
  file's directory and an absolute path is used as-is
  (`project_file.rs:571-577`).
- **What is persisted**: `ProjectFile` = `format_version`, `[job]` (name,
  stock, post, machine inline, legacy `machine_ref`), `[[tools]]`,
  `[[models]]` (path, name, kind, units — no geometry), `[[setups]]`
  (face, rotation, datum, fixtures, keep-outs, toolpaths), and per-toolpath
  config incl. `feeds_provenance`, `rest_analysis`, `planner_origin`
  (`project_file.rs:26-465`).
- **What is not persisted**: toolpath results, simulation results,
  diagnostics, cut traces, `stale_since`, `auto_regen`, `locked`,
  `visible`, `tool_load_overrides`, sim resolution / metric options,
  undo history. `GuiState` is rebuilt from scratch on load
  (`io.rs:170-173`) and `SimulationState::new()` replaces the old one
  (`io.rs:247`).
- **Reopen behaviour** (`io.rs:166-254`): every toolpath runtime is created
  `ToolpathRuntime::new(true)` with `stale_since = Some(loaded_at)`
  (`io.rs:179-182`). Because `auto_regen` is forced **true** for every op
  regardless of `default_auto_regen()`, `process_auto_regen` submits **all**
  of them 500 ms after load, including 3D ops that the card labels "MAN"
  (`toolpath_panel.rs:419-429`). This is the G-REGEN-RACE precondition
  described in `controller/tests.rs:3016-3027`. Selection resets to `None`,
  `dirty = false`. Legacy fallback loader keeps `tp.auto_regen`
  (`io.rs:276-279`).
- **Load warnings**: missing tool id, missing model id, model file not
  found, alignment-pin flip warnings (`io.rs:184-238`), deduped, logged,
  and shown in a "Project Load Warnings" window (`app.rs:869-890`). MCP
  `load_project` surfaces the same list in its reply (`mcp.rs:2989-2996`).
- **Dirty flag**: set only through `GuiState::mark_edited`. Cleared by save
  and by load. Displayed as "Modified" in the status bar
  (`status_bar.rs:103-111`).
- **Close interception**: `draw_frame` checks `close_requested()` and, if
  `dirty`, sends `CancelClose` and opens the quit dialog (`app.rs:609-614`).
  The dialog (`app/export.rs:415-460`) offers "Save & Quit" (Save As if no
  path; cancelling the file dialog keeps the window open), "Discard & Quit"
  (clears `dirty` to escape the CancelClose loop), "Cancel". File ▸ Quit and
  `AppEvent::Quit` use the same flag (`app/input.rs:392-398`).
- **Open another job with unsaved work**: `AppEvent::OpenJob` goes straight
  to `rfd::FileDialog::pick_file` then `open_job_from_path`
  (`app/input.rs:374-386`) — **no dirty check, no prompt**. Ctrl+O is the
  shortcut (`menu_bar.rs:28-30`). The same applies to MCP `load_project`
  (`mcp.rs:2981-2982`). Unsaved edits are lost silently.
- **Autosave / backup**: none found. Grep for `autosave`, `backup`, `.bak`
  over `crates/rs_cam_viz/src` returns nothing; the only temp file is the
  atomic-save temp (`save.rs:59`). There is no "New project" event either
  (no `NewJob` variant in `ui/mod.rs`); starting fresh means opening another
  file or restarting.
- **Save-as / variants**: `SaveJob` reuses `file_path` when set
  (`input.rs:354-360`), so a "save a variant" needs the MCP `save_project`
  with an explicit path (`mcp.rs:3009-3010`) or a fresh session with no
  path. No "Save As" menu item was found in `menu_bar.rs` (grep `SaveJobAs`
  returns nothing). **HYPOTHESIS**: there is no GUI Save As.

Sentry: `controller_save_open_and_export_smoke` saves, reopens and asserts
that G-code and SVG export now **fail** (no results) while the setup sheet
still renders (`controller/tests.rs:527-577`).

---

## 5. Missing external model

Fixture: `crates/rs_cam_viz/tests/fixtures/missing_model_project.toml` —
one tool, one model at relative path `missing_model.stl`, one setup, no
toolpaths. Used only by `load_warning_window_can_be_shown_and_dismissed`
(`controller/tests.rs:490`).

On load:

1. Core `build_session_from_project` calls `load_model_geometry`
   (`project_file.rs:834`); on error it logs "Failed to load model, skipping"
   and pushes a `LoadedModel` with `mesh: None`, `polygons: None`,
   `load_error: Some(e.to_string())` (`project_file.rs:901-921`). The
   session loads successfully.
2. The controller sees no geometry and adds the warning "Model 'Missing'
   could not be loaded because '<path>' was not found." (`io.rs:201-211`).
3. The "Project Load Warnings" window opens with that line (`app.rs:869-890`).
   It is a plain list with a close box; no buttons.

What the GUI then shows: the Setup panel lists the model by name
(`setup_panel.rs:103-108`); the model properties panel shows `Type:` and
`Path:` (`properties/mod.rs:770-771`) and skips the dimension block because
`model.mesh` is `None` (`mod.rs:773`). **`load_error` is never rendered in
the GUI** — grep over `src/ui` and `src/app.rs` finds no reader; only the MCP
`inspect_model` reply carries it (`mcp.rs:1886`).

Repair routes that exist:

- **Reload from disk** — right-click the model in the Setup panel
  (`setup_panel.rs:110-114`) → `reload_model` re-imports the **same stored
  path** (`io.rs:116-150`). It fixes the case "file was restored to the
  same location". It marks `pending_upload` and `mark_edited`, but sets
  **no `stale_since`** on toolpaths using that model (`io.rs:147-148`) —
  the sim reads stale by counter; toolpath cards do not.
- **Units rescale** — the model panel's units combo emits `RescaleModel`
  (`properties/mod.rs:867`, `886`) → `rescale_model` re-imports with new
  units (`io.rs:70-114`); STEP is refused (`io.rs:84-86`). Same
  invalidation gap.
- **Delete + re-import** — `RemoveModel` refuses while any toolpath
  references the model ("Cannot remove model: still referenced…",
  `model.rs:590-606`), so the operator must first re-point every toolpath's
  Input combo (`properties/mod.rs:3877-3891`) to a newly imported model, which
  (section 1.2) marks nothing stale.

Repair routes that do **not** exist: no "Locate…" / browse-for-file action,
no path edit field, no `SetModelPath` event (grep over `src/ui`,
`src/controller`, `src/app`). Moving a project with relative asset paths
works because of the `base_dir.join` at load (`project_file.rs:577`); a
project that stored absolute paths (whatever import produced) breaks on
move and can only be repaired by hand-editing the TOML.

MCP `import_model` plus `set_toolpath_param(model_id)` is the scripted
route; `load_project` reports the warning list (`mcp.rs:2989-2996`).

---

## 6. Library snapshot semantics

- **Tools**: "Add to project" in the Tool Library modal is documented as
  "Copy a snapshot of this tool into the open project."
  (`ui/tool_library_modal.rs:374-375`). `handle_add_tool_from_library`
  resets the id to a sentinel and `session.add_tool` assigns a project id
  (`controller/events/model.rs:77-86`); the tool is then an ordinary
  `[[tools]]` entry with no back-reference to the catalog
  (`project_file.rs:177-219` has no catalog field; `vendor`/`product_id`
  are plain strings). Editing the project copy does not touch the library.
  "Save to library" on the tool panel writes the project tool **into** a
  catalog, add-or-replace by geometry signature (`ui/properties/tool.rs:61-66`,
  TOO-004). The modal's own Save edits the catalog entry
  (`tool_library_modal.rs:528`).
- **Machines**: "Import into project" hover text: "Copy this machine into
  the project (snapshot — no live link)" (`ui/machine_library_modal.rs:197-198`).
  `import_machine_from_library` overwrites `session.machine_mut()`,
  `invalidate_machine`, `mark_edited`, status "Imported machine '…'
  (snapshot copy)" (`model.rs:180-190`). The file stores the machine inline
  at `[job.machine]`; a legacy `machine_ref` is read and **dropped** with a
  warning ("machines are now stored inline", `project_file.rs:1081-1097`).
  "Save to library" on the machine panel also calls `mark_edited`
  (`properties/mod.rs:1192-1194`) even though nothing in the project changed.
- **Reopen**: the project re-reads its own inline copies. It never
  re-reads the library. A catalog edit after import does not reach the
  project; a project edit does not reach the catalog unless "Save to
  library" is pressed. The GUI states this in two hover texts and one status
  line; the properties panel itself does not say "copy of catalog X".

---

## 7. Background work

### Worker and lanes

`ComputeBackend` has four lanes: Toolpath, Analysis, Optimize, Reach
(`crates/rs_cam_viz/src/compute/mod.rs`). `LaneSnapshot` carries `state`
(Idle / Queued / Running / Cancelling), `queue_depth`, `current_job`,
`current_phase`, `started_at`, `active_toolpath_id`, `active_toolpath_index`
(`compute/mod.rs:46-62`). The threaded worker's toolpath lane has a
resubmit rule: submitting the toolpath that is currently active sets the
lane's cancel flag and requeues, returning `SupersededActive`
(`compute/worker.rs:686-696`). Results arrive on a channel and are drained
by `drain_compute_results` on every pump (`app.rs:561`, `647`).

### Progress / stage surfaces

- Status bar: one chip per non-idle lane, `"{TP|AN|OPT|RCH} {state} · q{n} · {job} · {elapsed}s"`
  (`status_bar.rs:43-74`, `115-139`), with the four states colour-coded
  including `cancelling` (`status_bar.rs:57-61`).
- Viewport overlay: names of active jobs joined by " | " and a **Cancel All**
  button (`viewport_overlay.rs:140-159`) → `AppEvent::CancelCompute` →
  `compute.cancel_all()` (`events/mod.rs:156`; `compute/mod.rs:291-296`).
  A per-lane `CancelToolpathGeneration` event exists (`events/mod.rs:157-160`)
  but no GUI button in `toolpath_panel.rs`, `sim_op_list.rs` or
  `sim_timeline.rs` emits it (grep "Cancel" finds none).
- Card chip: `GEN` while `Computing` (`toolpath_panel.rs:400`), `WAIT` for
  `AwaitingPriorStock` with the blocker on hover (`403-406`), `ERR` with the
  message on hover (`408`, `416-418`).
- Workspace bar: `"{n} pending"` includes Computing (`workspace_bar.rs:126`).

So the five states the brief asks about map as: waiting-for-prerequisite =
card `WAIT`; queued = status-bar lane `queued · qN`; computing = `GEN` +
lane `running`; cancelling = lane `cancelling` (only while the worker has
not yet acknowledged); failed = `ERR`; completed = `OK`. There is no
per-toolpath progress percentage; `current_phase` exists on the snapshot but
`lane_chip_label` does not print it (`status_bar.rs:115-139`).

### Cancellation observability

- GUI: Cancel All flips lanes to `Cancelling`; the toolpath's status goes to
  `Pending` when the `Err(Cancelled)` drains (`compute.rs:934-938`), and
  its core result is forgotten (`forget_core_result`, `compute.rs:175`).
  No toast is pushed on cancel.
- MCP: `cancel_generation` and `generation_status` are served off the frame
  loop and answer within 1 s even with the GUI thread gone — sentried in
  `crates/rs_cam_viz/tests/mcp_escape_hatches.rs`
  (`cancel_generation_answers_and_cancels_with_the_gui_thread_gone` :125,
  `generation_status_names_the_in_flight_op_and_advances` :301,
  `generate_then_status_then_cancel_over_the_mcp_surface` :375,
  `generation_status_flags_a_parked_frame_loop_holding_a_generate_all` :590).
  Controller-side: `cancel_toolpath_generation_event_only_cancels_toolpath_lane`
  (`controller/tests.rs:992`), `reset_simulation_cancels_analysis_lane`
  (`:1646`), `a_genuine_cancel_is_still_reported_by_generate_all` (`:3238`).

### Late-result guard when the user edits during an in-flight generation

There is **no epoch / generation id / config fingerprint** on toolpath
results. Grep for `epoch|generation_id|revision|job_id|request_id` over
`src/compute` and `src/controller` finds only the optimizer / planner
"arrived after the dialog was closed — discarded" branches
(`compute.rs:1373-1429`). The toolpath drain (`compute.rs:838-987`) accepts
`Ok(computed)` for `tp_id` unconditionally: sets `Done`, syncs
`session.results`, sets `rt.result` (`compute.rs:872-925`). It does **not**
read or clear `stale_since`. What protects the user is indirect:

1. An edit during flight sets `stale_since` (section 1.2 / MCP). If the op
   has `auto_regen` (every op after a project load, `io.rs:180`),
   `process_auto_regen` resubmits after 500 ms, the worker cancels the
   running job, and the drain drops the superseded `Cancelled` as a
   non-outcome (`compute.rs:868-871`; test
   `a_param_edit_mid_generate_regenerates_without_a_pending_flicker`,
   `controller/tests.rs:3283-3337`).
2. If the in-flight job **finishes within the 500 ms window**, or the op has
   `auto_regen == false` or `locked == true`, the pre-edit result lands as
   `Done` with `stale_since` still `Some`. The card then shows "OK · N moves"
   with no badge (section 2). The late result *is* visibly current evidence
   for changed inputs.

Simulation has the same shape with a sharper edge: `last_sim_edit_counter`
is stamped with `gui.edit_counter` **at drain time**, not at submit time
(`compute.rs:1176-1185`). A simulation started before an edit and finishing
after it is recorded as fresh for the post-edit counter, so
`is_stale` reads **false** for a trace of the old inputs. The submit path
(`events/simulation.rs:237-269`) records no counter. **HYPOTHESIS** only in
the sense that it has not been reproduced live; the code path is direct.

Renderless test infrastructure: `ScriptedBackend` (`controller/tests.rs:42-98`)
records submits and returns `SupersededActive` when the id matches
`active_toolpath_id`; `LaneModelBackend` (`controller/tests.rs:3033-`) models
the cancel-and-requeue rule; `workflow_tests.rs` has its own backend
(`workflow_tests.rs:58-80`) and `w4_face_selection_in_undo_snapshot`
(`:426`). `render_snapshot` drives the egui tree headlessly
(`controller/tests.rs:470`).

---

## 8. Notifications

- Mechanism: `AppController::push_notification(message, severity)` appends a
  `Notification { message, severity, created_at }` (`controller.rs:62-66`,
  `269-275`). `push_error` wraps a `VizError` at Error severity
  (`controller.rs:259`).
- Lifetime: Info 4 s, Warning 6 s, Error 8 s (`controller.rs:70-76`); expired
  entries are garbage-collected every frame (`app.rs:894`,
  `controller.rs:283-285`). There is no history, no click-to-dismiss, no
  action button.
- Position: bottom-right `egui::Area` anchored `RIGHT_BOTTOM` at (-12, -12),
  max width 400, coloured frames per severity, text only
  (`app.rs:892-931`). The frame requests a repaint after 1 s while any toast
  is alive (`app.rs:930`).
- A separate 5 s status message exists (`set_status`, `controller.rs:288-300`),
  used by the machine-library actions (`model.rs:186,194,201,208`).
- Persistent surfaces are only the "Project Load Warnings" window
  (`app.rs:869-890`) and the `NOT MEASURED` / stale banners in the sim panels.

### MCP pre-announce pattern

`drain_mcp_requests` in `crates/rs_cam_viz/src/app/mcp.rs` contains 16
`push_notification` calls (rg count). **Every one is issued before the
handler runs**, and none is conditional on the handler's `Ok`. Confirmed for
`AddToolpath`: the toast "MCP: Added toolpath '{name}'" is pushed at
`mcp.rs:573-576`, then `mcp_add_toolpath` runs at `mcp.rs:580-581`. Full list:

| Line | Toast text | Handler that runs afterwards |
|---|---|---|
| 368 | "MCP: Adding setup" | `mcp_add_setup` |
| 376 | "MCP: Set setup {i} face to '{face}'" | `mcp_set_setup_face` |
| 392 | "MCP: Set setup {i} Z rotation to '{rot}'" | `mcp_set_setup_rotation` |
| 408 | "MCP: Moving toolpath {i} to setup {j}" | `mcp_move_toolpath_to_setup` |
| 422 | "MCP: Importing '{name}'" | `mcp_import_model` |
| 433 | "MCP: Loaded '{name}'" | `mcp_load_project` |
| 439 | "MCP: Saved project" | `mcp_save_project` |
| 495 | "MCP: Set {param} = {value} on '{tp}'" | `mcp_set_toolpath_param` |
| 548 | "MCP: Set {param} = {value} on '{tool}'" | `mcp_set_tool_param` |
| 573 | "MCP: Added toolpath '{name}'" | `mcp_add_toolpath` |
| 598 | "MCP: Removed toolpath {i}" | `mcp_remove_toolpath` |
| 604 | "MCP: Added tool '{name}'" | `mcp_add_tool` |
| 609 | "MCP: Imported tool from library '{cat}' #{i}" | `mcp_add_tool_from_library` |
| 726 | "MCP: Generating toolpath '{name}'..." | `mcp_generate_toolpath` |
| 740 | "MCP: Generating all toolpaths..." | `mcp_generate_all` |
| 748 | "MCP: Running simulation..." | `RunSimulation` event |

Nine of these use the past tense ("Loaded", "Saved", "Set", "Added",
"Removed", "Imported") for an action that has not yet run and may fail; the
error path in each handler returns a JSON error to the agent and pushes no
correcting toast. The remaining seven use a progressive form and are
truthful. Not pre-announced at all (no toast): `set_toolpath_heights`,
`set_boundary_config`, `set_dressup_*`, `set_stock_config`,
`set_machine_kinematics`, `set_toolpath_enabled`, `remove_tool`,
`remove_alignment_pin`, and every read.

---

## 9. Questions only a live test can answer

1. **Inspect-then-leave dirties the project?** Open a saved project, click a
   toolpath, click Stock. Expect from source: "Modified" appears, the
   Simulation chip turns stale, Ctrl+Z becomes enabled and its first press
   changes nothing. Repeat for Machine and Post. (`properties/mod.rs:143-202`)
2. **Why did the pocket not regenerate?** Issue one `set_toolpath_param`
   and within 300 ms read `list_toolpaths.stale`, then at 1 s read
   `generation_status`. Also check the card's lock glyph and the "MAN" badge.
   Distinguishes the three readings in section 2.
3. **Does a tool edit in the GUI leave dependent cards reading "OK"?** Change
   a tool diameter, Apply. Expect: cards unchanged, "Modified" set,
   Simulation chip stale, Readiness "uncomputed" **not** shown, export
   allowed and emitting the old geometry via the viz fallback
   (`export.rs:129-131`).
4. **Tool / model reassignment and dressup edits from the GUI**: expect no
   stale mark and no "Modified" (section 1.2). Confirm that Ctrl+S is not
   offered anything to save.
5. **Late simulation result**: start a long simulation, change a stock
   dimension while it runs, wait for completion. Expect the Simulation chip
   to read fresh (✓) although the trace predates the edit
   (`compute.rs:1176-1185`).
6. **Late toolpath result on a MAN (3D) op**: generate, edit a parameter
   during flight, let it finish. Expect "OK · N moves" for the pre-edit
   geometry and no badge.
7. **Enable toggle in GUI**: toggle an op off. Expect the "Modified" label
   **not** to appear and the Simulation chip **not** to go stale
   (`events/mod.rs:114-118`); then save and reopen to confirm the toggle was
   nevertheless persisted.
8. **Open with unsaved work**: edit, Ctrl+O, pick another file. Expect no
   prompt and silent loss (`input.rs:374-386`).
9. **Missing model UX**: open the fixture, close the warning window, select
   the model. Expect `Path:` only, no error text, and no locate action.
   Then place `missing_model.stl` beside the TOML and use "Reload from
   disk"; check whether dependent toolpath cards change state (source says
   they do not).
10. **Undo of a parameter edit after a simulation**: edit stepover,
    simulate, Ctrl+Z (twice, per Q1). Expect the sim chips to read fresh
    while the toolpath is stale on the MCP `list_toolpaths.stale` field
    (`undo.rs:32-50`, no `mark_edited`).
11. **Cancel observability**: start Generate All on a slow project, press
    Cancel All. Time how long the lane chip shows `cancelling` and whether
    any toast appears (source: none).
12. **Toast readability**: trigger an MCP `add_toolpath` with an invalid
    tool index and watch whether the "MCP: Added toolpath" toast is
    contradicted by anything visible (source: nothing corrects it).
