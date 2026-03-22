# Review: Operation Creation & Generation Flow

## Summary

The operation creation and generation pipeline is well-structured with clear separation between UI event handling, operation configuration, compute worker execution, and result storage. The system supports 22 operation types across 2.5D and 3D workflows, with sensible defaults for each. However, there are three notable gaps: (1) parameter changes don't automatically mark results as stale (auto_regen is stubbed but unused), (2) no toolpath preview before full generation, and (3) operation ordering is tracked locally within setups but order dependency checking (e.g., rest machining) exists only in validation, not enforcement.

## Findings

### Operation Creation

- User clicks "+ Add Toolpath" -> `AppEvent::AddToolpath(op_type)` -> handler in controller/events.rs:190-229 creates new `ToolpathEntry::for_operation()`
- Default tool and model auto-assigned (first in list or fallback to ID 0)
- Operation defaults from `OperationConfig::new_default(op_type)` in state/toolpath/catalog.rs:679
- Each operation type has a spec with sensible defaults (e.g., Pocket: 2mm stepover, 1.5mm depth/pass, 1000 mm/min feed)
- All 22 operation types fully enumerated in `OperationType::ALL` with 2D/3D partitions
- Entry immediately added to current setup or falls back to first setup
- No automatic tool assignment logic beyond "first tool in list" -- if no tools exist, ToolId(0) assigned (may not exist)
- No validation that selected tool/model exist at creation time -- validation only runs at generation

### Properties & Configuration

- Properties panel in ui/properties/mod.rs:743-969 renders toolpath controls
- Tool and model combo-box selectable after creation
- Operation-specific parameter grids with labeled drag-value controls (e.g., `draw_pocket_params`, `draw_adaptive3d_params`)
- Tool, stepover, depths, feed/plunge rates, and operation-specific toggles editable via egui
- Validation runs only at generation time, not on each edit
- For rest machining, user must select "Previous Tool" (separate selector); no enforcement that tool geometry is compatible with operation except for VCarve/Inlay/Chamfer/Rest at validation time (ui/properties/mod.rs:2408-2481)
- Dressup modifications: entry style (Plunge/Helix/Ramp), dogbone overcuts, lead-in/out, tabs
- Heights configuration: stock-to-leave per axis, final depth
- Boundary clipping: optional "Clip to stock boundary" with Center/Inside/Outside containment modes
- Status display: "Ready" | "Computing..." | "Done" (green) | "Error: ..." (red) with move count, cutting/rapid distance

### Generation Pipeline

**Submission Phase (events.rs:684-854):**
- "Generate" button -> `AppEvent::GenerateToolpath(tp_id)`
- `submit_toolpath_compute()` extracts toolpath state, tool config, model (mesh or polygons)
- Geometry requirement check: 3D ops require mesh, 2.5D ops require polygons; if missing -> error status set immediately
- Setup transformation applied (FaceUp, Z-rotation mapped to global frame)
- Keep-out footprints (fixtures, zones) collected
- Previous tool radius resolved for rest machining
- Heights config resolved to absolute Z values
- Status set to `ComputeStatus::Computing`, result/traces cleared
- `ComputeRequest` submitted to worker thread

**Queue Management (worker.rs):**
- Two independent lanes: `ComputeLane::Toolpath` and `ComputeLane::Analysis`
- Toolpath lane has single serial FIFO queue
- `GenerateAll` appends every enabled toolpath to queue in order
- No queue depth limit or priority system; user can submit more work while compute is running
- No built-in per-toolpath cancellation (can cancel entire lane)

**Execution Phase (worker/execute.rs):**
- Worker thread dequeues request, dispatches to operation-specific handler (~20 handlers)
- Calls rs_cam_core compute functions with traced execution
- Semantic trace captured if `debug_options.enabled`
- Output: `ComputeResult { toolpath_id, result: Ok(Toolpath), debug_trace, semantic_trace }`

**Result Storage (events.rs:855-878):**
- Main loop calls `compute.drain_results()` each frame
- Ok -> status Done, result stored, traces attached
- Cancelled -> status Pending, result cleared
- Error -> status Error(message), result cleared
- GPU re-upload triggered via `pending_upload = true`

### Result Display & Inspection

- Selected toolpath rendered with palette color (8-color cycle)
- Cutting moves: Z-depth color blending (darker at bottom, brighter at top)
- Rapid moves: 30% dimmed version of cutting color
- Selected toolpath: 30% brightness boost
- Per-move vertex counts pre-computed for simulation scrubbing
- Statistics: move count, cutting distance (mm), rapid distance (mm) shown in properties panel
- `ToggleIsolateToolpath` event hides all other toolpaths; stored in `ViewportState::isolate_toolpath`
- Context menu "Inspect in Simulation" available when result exists
- No quick preview during parameter edit; only available after full generation

### Ordering & Dependencies

- Drag-to-reorder in project tree via `dnd_drop_zone` (ui/toolpath_panel.rs:62-102)
- Drop zone per setup for same-setup reordering; drag between setups triggers `MoveToolpathToSetup`
- Backend: `move_toolpath_up()`, `move_toolpath_down()`, `reorder_toolpath()`, `move_toolpath_to_setup()` in state/job.rs:1194-1302
- All preserve toolpath object state (config, status, result)
- Order in list -> order in G-code output (sequential emission)
- Rest machining validation checks `prev_tool_id` is selected but does NOT check that the prior-tool toolpath exists earlier in sequence
- No automatic invalidation of downstream rest operations when prior tool is deleted/reordered

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | Parameter changes don't mark result stale; `auto_regen` field exists but is never used in controller logic. `process_auto_regen()` has `stale_since` logic but nothing triggers it. | state/toolpath/entry.rs:135, controller.rs:116-137 |
| 2 | Medium | Validation rules fragmented between UI (`validate_toolpath`) and runtime (`submit_toolpath_compute`). VCarve/Inlay/Chamfer validated in UI; geometry existence checked only at generation. | ui/properties/mod.rs:2408-2481 vs controller/events.rs:786-799 |
| 3 | Low | Default tool/model assignment uses first-in-list fallback without existence check. If no tools exist, ToolId(0) assigned and error only appears at generation. | controller/events.rs:201-214 |
| 4 | Low | Rest machining validation checks `prev_tool_id` is set but does NOT verify toolpath ordering -- user could create Rest operation before prior-tool operation in list. | ui/properties/mod.rs:2462-2476 |
| 5 | Low | Operation-type defaults don't adapt to context (stock thickness, tool diameter). Pocket always defaults to 3mm depth regardless of stock. | state/toolpath/configs.rs:190-204 |
| 6 | Low | `auto_regen` field always `true` for 2.5D ops, `false` for 3D ops per spec, but user has no visibility or control over this. | state/toolpath/catalog.rs:50, 145-360 |
| 7 | Low | Deletion of toolpath doesn't cascade invalidation to dependent Rest toolpaths; dependent Rest toolpath silently uses stale stock model. | state/job.rs:1155-1188 |

## Test Gaps

- No test for operation creation with missing tool/model (should show error at generation)
- No test for rest machining validation with toolpath ordering (prev tool after current tool)
- No test for parameter change invalidation (auto_regen feature partially stubbed)
- No integration test for complete flow: create -> configure -> generate -> visualize
- No test for operation duplication preserving parameters correctly
- No test for drag-drop reordering across setups preserving setup ownership
- Existing tests in entry.rs cover duplication and initialization but not generation pipeline
- Compute worker has tests for feed optimization but not for queue ordering with multiple submissions

## Suggestions

1. **Implement auto-regeneration:** Use `stale_since` timestamp currently set in IO but unused in events.rs. On property edit, set `stale_since = Instant::now()` and let `process_auto_regen()` handle delayed re-submission after debounce
2. **Centralize validation:** Move operation-specific checks to a unified `validate_operation_for_generation()` function; call from UI (for immediate feedback) AND from submit_toolpath_compute (for safety)
3. **Unify rest machining dependencies:** When Rest operation is created, automatically find prior larger tool in earlier sequence; validate at creation time. If user reorders, re-validate immediately
4. **Surface auto_regen setting in UI:** Add checkbox "Auto-regenerate on parameter change" with tooltip. Let user opt-out for expensive operations (3D roughing)
5. **Display queue depth:** Show compute lane status in status bar -- "Computing [2/5]" when queue has work
6. **Add "regenerate all stale" button:** Complement "Generate All" with "Regenerate Changed" that re-submits only operations with `stale_since` set
