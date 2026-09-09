# R08 source track — support agent report (v1, received by message 2026-09-09 16:35 +12)

Read-only Claude Code `Explore` subagent `r08-source`. Saved verbatim by the orchestrating
session `rs-cam-b9`. **Leads, not accepted findings** until the R08 reviewer verifies them.
The orchestrator spot-checked: export fallback to the GUI runtime result (io/export.rs:100-135) ✔;
`stale_since` has one GUI read (toolpath_panel.rs:746) ✔; `ToggleToolpathEnabled` handler
does not call `mark_edited` (events/mod.rs:114-118) ✔.

---

## Headline

Three invalidation systems exist and they do not agree.

- **Core result cache** — `ProjectSession::results` / `::simulation`, dropped by `invalidate_result_chain` / `invalidate_output_dependents`, `crates/rs_cam_core/src/session/mutation.rs:211-293`. The only transitive one.
- **GUI overlay staleness** — `ToolpathRuntime::stale_since`, `crates/rs_cam_viz/src/state/runtime.rs:34`. Read by exactly three places: the auto-regen sweep `controller.rs:302-333`, the MCP wire `app/mcp.rs:951,1251`, and the dependency chip `ui/toolpath_panel.rs:746`. **No user-visible badge reads it.**
- **Global edit counter** — `GuiState::edit_counter`, bumped by `mark_edited` at `state/runtime.rs:338-341`, compared by `SimulationState::is_stale` at `state/simulation.rs:1003-1006`. The only signal any simulation banner reads.

## 1. Mutation matrix

| Mutation | Handler | Commits | Core cache | `stale_since` | `edit_counter` / dirty | Sim results | Undo entry | Saved |
|---|---|---|---|---|---|---|---|---|
| Tool geometry field | `properties/mod.rs:106-140` | Apply, or navigate away | `invalidate_tool` drops referencing results | **no** | yes | kept, flagged | `ToolChange` | yes |
| Tool flutes | same | same | same | **no** | yes | kept, flagged | `ToolChange` | yes, `save.rs:126` |
| Op geometry field | `properties/mod.rs:697-720` | same frame, live | **not dropped** | yes | yes | kept, flagged | deferred to selection change | yes |
| Op heights | same + `StockChanged` at `mod.rs:719` | same frame | not dropped | yes | yes | kept, flagged | deferred | yes |
| Op boundary | same + rest-analysis hook `mod.rs:722-745` | same frame | not dropped | yes | yes | kept, flagged | deferred | yes |
| Toggle op enabled | `events/mod.rs:114-118` | immediate | `invalidate_output_dependents(true)` | **no** | **no** | kept, **not flagged** | none | yes |
| Reorder ops | `events/toolpath.rs:236-293` | immediate | both positions invalidated | **no** | yes | kept, flagged | none | yes |
| Stock dimensions | `events/model.rs:630-642` | every drag frame | `invalidate_stock` | **no** | yes | kept, flagged | `StockChange`, one per frame | yes |
| Stock material | `properties/stock.rs:27`, `events/mod.rs:233` | on pick | via `StockChanged` | **no** | yes | kept, flagged | `StockChange` | yes |
| Setup face / rotation | `properties/setup.rs:66,80`, emits only `FixtureChanged` | same frame | **nothing** | **no** | yes | kept, flagged | none | yes |
| Machine kinematics | `properties/mod.rs:1295-1300`, `events/mod.rs:236-239` | on widget change | `invalidate_machine` | **no** | yes | kept, flagged | `MachineChange` on navigate away | yes |
| Dressup field | `properties/mod.rs:4796,4804` | same frame | **nothing** | **no** | **no** | kept, **not flagged** | in the deferred toolpath entry only | yes |
| Sim resolution / capture | `ui/sim_op_list.rs:52-108` | same frame | nothing | no | **no** | kept, **not flagged** | none | **no** |

Five rows matter most.

**A tool edit leaves an exportable result cut with the old tool.** `commit_tool_draft` calls `invalidate_tool` (`properties/mod.rs:138`), which drops `session.results` but sets no `stale_since` and does not clear `gui.toolpath_rt[id].result`. Export falls back to that surviving viz result (`io/export.rs:129-131`), documented at `io/export.rs:102-107`. Old geometry stays exportable under a green `OK` chip.

**Setup face and rotation invalidate nothing in the GUI.** `properties/setup.rs:58-83` writes in place and pushes `FixtureChanged`; that handler (`events/mod.rs:98-101`) only sets `pending_upload` and `mark_edited`. The MCP path applies `MutationKind::SetupChanged` marking every toolpath in the setup stale (`app/mcp.rs:2804,2860`); the GUI path does not.

**Toggling an operation off does not dirty the project** (`events/mod.rs:114-118`): close interception (`app.rs:611`) reads only `gui.dirty`, so the change is discarded on close with no warning; `is_stale` does not move.

**Dressup edits are silent in every channel.** The change detector (`properties/mod.rs:700-718`) compares only `operation`, `heights`, `boundary`; dressups are written through by `write_entry_config_to_session` (`mod.rs:3458`) regardless.

**The same detector gap covers tool reassignment, model reassignment, name, coolant, pre/post G-code, `stock_source`, `rest_analysis`** (`mod.rs:3871`, `3888`, `3455-3456`). Some set `stale_since` by hand (`stock_source` 3960-3962, `rest_analysis` 4530); none calls `mark_edited`.

## 2. Undo model

Per-field, typed: `UndoAction` (`state/history.rs:15-42`) has five variants — `StockChange`, `PostChange`, `ToolChange`, `ToolpathParamChange`, `MachineChange`. Structural edits (add/duplicate/remove/reorder toolpaths, setups, fixtures, models, pins) push no entry (`events/toolpath.rs:177,232,252,269,290,316,338`).

Dependent meaning restored inconsistently (`events/undo.rs:10-55`): `StockChange`/`ToolChange`/`MachineChange` call `invalidate_simulation`; `ToolpathParamChange` sets `stale_since` only; no arm calls `mark_edited`. Redo exists (`undo.rs:59-109`); stack cap 100 (`history.rs:85-91`). Binding `menu_bar.rs:10-17` Ctrl+Z / Ctrl+Shift+Z with no focus guard; menu items never disabled.

Commit timing: Tool = draft + Apply/Revert with auto-commit on navigate away (`tool.rs:28-53`, `mod.rs:90-101`); toolpath/stock/machine/post commit live, undo entry deferred to selection change (`mod.rs:143-200`); **stock drags push one undo entry per frame** (`properties/stock.rs:84-166`, `mod.rs:256-268`), so one slow drag can evict all earlier history. G-HEIGHTSTAB sentry `controller/tests.rs:445-488`.

## 3. Save and reopen

`save_job_to_path` (`io.rs:152-164`) atomic write (`session/save.rs:52-66`). Persisted (format 3): job, stock, post, inline machine, tools, models as path+kind+units, setups, toolpath configs incl. dressups/heights/boundary/rest analysis/stock source/feeds provenance/planner origin. **Not persisted**: computed toolpaths, simulation, diagnostics, `stale_since`, visibility, lock, auto-regen, tool-load overrides, every simulation setting (`SimulationState::new()` on load, `io.rs:247`).

On load (`io.rs:166-293`) every toolpath is marked stale (`io.rs:180-182`); nothing auto-generates directly — the sweep picks up 2.5D ops 500 ms later; 3D ops stay Pending. Load warnings → non-modal dismissible "Project Load Warnings" window (`app.rs:869-889`), never re-openable. Legacy fallback `io.rs:255-291`.

**Close with unsaved edits** intercepted (`app.rs:610-614`, dialog `app/export.rs:415-460`: Save and Quit / Discard and Quit / Cancel). **Opening another project with unsaved work is not guarded** (`app/input.rs:379-390`, Ctrl+O `menu_bar.rs:28-30`). **No autosave, backup or versioning.**

## 4. Missing external model

`build_session_from_project` (`project_file.rs:834-905`) keeps a `LoadedModel` with `load_error`. GUI warnings (`io.rs:202-211`, `192-198`): "Model '{}' could not be loaded because '{}' was not found." / "Toolpath '{}' references missing model id {} and needs reassignment." Sentry `controller/tests.rs:490-504`. **Repair routes are thin**: `load_error` never rendered in the GUI (only MCP `app/mcp.rs:1886`); model path is a plain label (`properties/mod.rs:1085`); only actions are a right-click menu on the collapsed "Models" header (`setup_panel.rs:108-118`): "Reload from disk" (same failing path) and "Delete" (refused while referenced, `events/model.rs:590-607`). **No browse-for-new-path route.** Paths saved absolute as imported; relative paths resolve against the project dir (`project_file.rs:572-575`).

## 5. Library snapshot semantics

Tools: `handle_add_tool_from_library` (`events/model.rs:76-86`) resets the id; hover "Copy a snapshot of this tool into the open project." (`tool_library_modal.rs:374-375`). Reverse "Save to library" (`tool.rs:63-102`) writes the **draft**, overwriting by geometry signature. Machines: `import_machine_from_library` (`events/model.rs:178-190`) replaces the inline machine, `invalidate_machine`, status "Imported machine '{name}' (snapshot copy)"; modal says "Snapshot library — importing COPIES a machine…" (`machine_library_modal.rs:59`). Neither import marks any toolpath stale.

## 6. Background work, cancellation, late-result guard

Four lanes (`compute/mod.rs:20-34`), states Idle/Queued/Running/Cancelling (37-43), `LaneSnapshot` (45-62). Status bar one chip per non-idle lane (`status_bar.rs:43-74`, `lane_chip_label` 115-139); viewport strip with "Cancel All" (`viewport_overlay.rs:140-159`). No percentage. `AwaitingPriorStock` → yellow `WAIT` chip (`toolpath_panel.rs:400-403`).

Cancellation: `CancelCompute` / `CancelToolpathGeneration` (`events/mod.rs:156-160`); `GenerationControl` (`compute/mod.rs:262-283`); `mcp_escape_hatches.rs:125,301,343,375` against a stub lane. **A cancel is destructive to the previous result** (`events/compute.rs:476`, `934-938`).

**Late-result guard is a supersede filter, not an epoch check** (`compute/mod.rs:195-210`, `events/compute.rs:791-798`, `836-869`; G-REGEN-RACE). No generation id on the result. Auto-regen ops (2.5D): sweep resubmits, in-flight superseded, replacement lands (sentry `controller/tests.rs:3283-3335`). **Manual-regen ops (every 3D family, `catalog.rs:2142…2384`)**: the sweep never fires (`controller.rs:312`); the arriving result is **applied as current** (`events/compute.rs:872-932`); `stale_since` stays set but nothing renders it; the card shows green `OK`. Optimize moves the session into the worker (`events/mod.rs:496-499`) with an honest placeholder (`app.rs:737-757`). `ScriptedBackend` (`controller/tests.rs:21-80`) proves control flow, never timing.

## 7. Notifications

`Notification` (`controller.rs:62-80`): TTL 4 s Info, 6 s Warning, 8 s Error; `gc_notifications` (283-285). Rendered bottom-right (`app.rs:889-928`), **not interactive**, no history. Second channel `set_status` (288-290), 5 s. Only persistent notice: the load-warnings window.

## 8. Questions only a live test can answer

1. 500 ms auto-regen debounce feel and toast noise. 2. Undo entries per stock drag. 3. Ctrl+Z in a focused text field. 4. Any viewport treatment for stale? 5. 3D finish edited mid-generation: which result renders? 6. Missing-model project with a referencing toolpath. 7. Toast TTL vs slow generation errors. 8. Setup flip: do un-marked stale toolpaths look correct?

Suggested Track A traces: late result on a 3D finish (edit stepover mid-generation); missing reference (move the STL, reopen, recover via the Models context menu).
