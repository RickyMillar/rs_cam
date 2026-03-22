# Review: Simulation & Export Flow

## Summary

The Simulation & Export Flow is architecturally sound with most major components working end-to-end. The tri-dexel stock representation (Phases 1-6) is complete and handles multi-setup sequential simulation correctly. Playback, timeline scrubbing, and collision checking are functional. Export supports multiple dialects (GRBL/LinuxCNC/Mach3) and output modes (single/combined/per-setup). However, there are documented gaps in deviation coloring wiring, rapid collision visualization, and per-operation G-code injection that are tracked in the FEATURE_CATALOG.

## Findings

### Simulation Launch & Execution

- `RunSimulation` event triggers `run_simulation_with_all()` (controller/events.rs:445)
- `RunSimulationWith(Vec<ToolpathId>)` allows targeted re-simulation of specific toolpaths/setups
- Pre-transformation of toolpaths from setup-local to global stock frame (events.rs:474-482)
- Per-setup direction (`face_up_to_direction`) properly mapped to `StockCutDirection` (events.rs:459)
- Auto-resolution based on smallest tool diameter (events.rs:507-510)
- `run_simulation_with_phase()` initializes fresh `TriDexelStock` from bounds (compute/worker/execute.rs:76)
- Per-toolpath stamping collects `SimulationCutSample` when metric options enabled (execute.rs:95-110)
- Rapid collision detection runs separately post-simulation (execute.rs:142-158)
- Results packaged as `SimulationResult` with boundaries, checkpoints, and playback data (execute.rs:183-194)
- First setup always uses fresh stock; subsequent setups inherit remaining material via shared `TriDexelStock` instance (Phase 5 complete)
- Checkpoint deep-copies include X/Y grids when present (Phase 6 side-grid support complete)
- Multi-setup sequential simulation on single `TriDexelStock` confirmed working (PROGRESS.md)

### Playback & Timeline

**Transport controls fully wired:**
- Play/pause toggle (`ToggleSimPlayback`) at sim_timeline.rs:45
- Step forward/backward at events.rs:354-367
- Jump to start/end at events.rs:369-382
- Per-operation navigation (`SimJumpToOpStart`/`SimJumpToOpEnd`) at events.rs:384-406
- All events trigger `pending_checkpoint_load` when backtracking (app.rs:190)

**Timeline scrubber:**
- Slider at sim_timeline.rs:64-72 with no step quantization issues visible
- Playback pauses when scrubbing (sim_timeline.rs:71)
- Current move clamped to total_moves (app.rs:189)

**Playback initialization:**
- Results initialize with `current_move=0, playing=true` (events.rs:991-992) -- user sees progressive cutting
- After simulation complete, checkpoint stack and live stock reset from 0 (events.rs:996-1001)
- Incremental playback via `update_live_sim()` resimulates forward or loads nearest checkpoint when backtracking (app.rs:439-550)

**Per-operation visualization:**
- Boundaries stored with start/end moves per operation (simulation.rs:156-164)
- Setup boundaries tracked separately for multi-setup (simulation.rs:166-172)
- Tool position updates during playback via `update_sim_tool_position()` (app.rs:1131)
- Cutting vs rapid color coding handled by `MoveType` in toolpath IR

### Collision Detection

**Rapid collision detection integrated into simulation:**
- `check_rapid_collisions()` called post-stamping for all toolpaths (execute.rs:150)
- Results stored in `rapid_collision_move_indices` for timeline marker rendering (execute.rs:152)
- Displayed as orange tick marks on timeline (sim_timeline.rs:191-199)
- Navigation via `focus_issue_delta()` with `SimulationIssueKind::RapidCollision` (simulation.rs:1000)

**Holder/shank collision check (separate workflow):**
- `RunCollisionCheck` event queues `CollisionRequest` on Analysis lane (events.rs:413, 653-682)
- Takes first computed toolpath with STL mesh and tool (events.rs:654-671)
- Holder collisions rendered as cross-hair markers at collision points (app.rs:988-1031)
- Red tick marks on timeline at holder collision move indices (sim_timeline.rs:182-188)
- Collision count displayed in workspace bar (workspace_bar.rs:140)
- Results are transient and not persisted across workspace switches

**Known limitation:** Collision check processes only first enabled toolpath with STL mesh (events.rs:654) -- doesn't support multi-toolpath or multi-setup collision verification

**Preflight integration:**
- Rapid collision check: warning if detected (preflight.rs:79-102)
- Holder collision check: fail if detected (preflight.rs:104-128)
- Both allow "Export Anyway" with warning/fail status (preflight.rs:154-161)

### Simulation Staleness

- `SimulationRunMeta` stores `last_sim_edit_counter` (events.rs:256-260)
- On fresh simulation: `last_sim_edit_counter = job.edit_counter` (events.rs:1012)
- Staleness check: `is_stale()` compares current `edit_counter` to last sim's counter (simulation.rs:391-395)
- Any `mark_edited()` call bumps `job.edit_counter` (job.rs:1305)
- Preflight modal shows "Stale -- parameters changed" when stale (preflight.rs:45-62)
- No auto-re-run (intentional -- gives user control over expensive operations)
- No visual indicator outside preflight modal; only visible when opening Export G-code dialog
- Simulation workspace has no continuous staleness badge

### Export Pipeline

**Three export modes:**
1. **Single G-code** (`export_gcode`): all enabled toolpaths in one file (export.rs:8-42)
2. **Combined with M0 pauses** (`export_combined_gcode`): per-setup groups with M0 between setups (export.rs:44-92)
3. **Per-setup** (`export_setup_gcode`): single setup only (export.rs:94-134)

**Core emission:**
- Calls `emit_gcode_phased()` or `emit_gcode_multi_setup()` from `rs_cam_core::gcode` (export.rs:34, 85)
- Passes post-processor name (GRBL/LinuxCNC/Mach3) via `PostFormat` enum
- Collects enabled toolpaths with computed results into `GcodePhase` structs
- Returns error if no toolpaths to export (export.rs:30-32)

**High feedrate mode:**
- Converts G0 rapids to G1 at specified feedrate when enabled (export.rs:36-39, 87-89)
- Uses `replace_rapids_with_feed()` helper from core

**G-code dialects:** GRBL, LinuxCNC, Mach3 selectable via `PostFormat` enum

**Setup sheet generation:** HTML document with job name, stock dims, tool list, operation summary (setup_sheet.rs:75). 9 unit tests verify content (setup_sheet.rs:513-547)

**SVG preview:** toolpath preview exported as vector graphics for quick visual verification

### Export UX & Options

**Pre-flight checklist modal (preflight.rs:9-171):**
- Triggered by `ExportGcode` event
- Five checks: operations computed, simulation up-to-date, rapid collisions, holder clearance, cycle time
- Warnings link to relevant workspaces for fixes (preflight.rs:37-127)
- "Export Anyway" button available even with failures for user override

**File dialogs:**
- Default filenames generated from job name + mode (e.g., `{jobname}_combined.nc`)
- File picker filters for `.nc`, `.gcode`, `.ngc` extensions
- No preview before export: SVG preview and setup sheet are separate export options

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | Deviation coloring helper exists (`deviation_colors()`) but wiring is incomplete. `display_deviations` set to `None` on fresh sim results; deviation data never computed from model surface. | app.rs:417-429, events.rs:949, simulation.rs:237 |
| 2 | Medium | Rapid collisions rendered only as timeline markers (orange ticks); no 3D visualization in viewport. Holder collisions show cross-hair markers but rapid collisions do not. | app.rs:991-1031, sim_timeline.rs:191-199 |
| 3 | Low | Collision check processes only first toolpath with STL mesh. Multi-setup or multi-tool jobs may miss collisions on subsequent toolpaths. | controller/events.rs:654-671 |
| 4 | Low | `pre_gcode` and `post_gcode` fields in ToolpathEntry editable in UI but never emitted during G-code export. | state/toolpath/entry.rs:28-29, io/export.rs |
| 5 | Low | No visual staleness indicator in Simulation workspace outside of preflight modal. User sees "Stale" only when initiating export. | ui/preflight.rs:56, ui/sim_diagnostics.rs |
| 6 | Low | `StockVizMode::ByOperation` placeholder returns uniform wood-tone color instead of per-operation cell tracking. | render/sim_render.rs:124-127 |
| 7 | Low | Auto-resolution uses first stock boundary -- may produce coarse resolution if smallest tool is in a later setup. | controller/events.rs:509 |

## Test Gaps

- No integration test for staleness tracking across parameter edits (existing test at controller/tests.rs:550 checks flag but not full "edit then check" flow)
- No test for collision check result persistence (collision_positions cleared on ResetSimulation but untested)
- No test for multi-setup simulation carry-forward (no verification that Setup 2's simulation includes Setup 1's residual stock)
- No test for high-feedrate mode in export
- No test for workspace viewport preservation across workspace switches
- No test for playback increment with live_stock (`update_live_sim()` checkpoint-load logic untested)
- No test for toolpath pre/post G-code fields (fields exist but export doesn't use them)
- No test for per-setup export mode (only all + combined export tested)

## Suggestions

1. **Complete deviation coloring:** Wire `deviations` output from simulation worker into playback state and feed to renderer when `StockVizMode::Deviation` selected. Data pipeline is partially built (helper exists, state field exists) but data never flows from core to UI
2. **Add rapid collision 3D markers:** Compute line segments at rapid collision positions and render as red lines or spheres in viewport during Simulation workspace, similar to holder collision cross-hairs
3. **Extend collision check to all toolpaths:** Loop over all enabled toolpaths with results and STL meshes; merge results into summary report. Or warn user if multiple toolpaths exist and only first was checked
4. **Wire pre/post G-code:** Emit `toolpath.pre_gcode` before each `GcodePhase` and `post_gcode` after in `emit_gcode_phased()`. Mark as done in FEATURE_CATALOG
5. **Add staleness badge to Simulation panel:** Show "Stale" icon in sim_diagnostics header when `is_stale(job.edit_counter)` true, with "Re-run Simulation" button
6. **Implement per-operation cell tracking:** For `StockVizMode::ByOperation`, track which operation index last modified each vertex during mesh extraction
7. **Test playback frame-stepping:** Add regression test for `update_live_sim()` checkpoint-load logic with backward scrubbing -- this is complex and currently untested
8. **Clarify rapid vs holder collision UX:** Consider grouping both checks under "Run Collision Analysis" with unified results panel since users expect both to run together
