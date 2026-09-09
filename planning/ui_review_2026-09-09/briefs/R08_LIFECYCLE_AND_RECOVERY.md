# R08 — Iteration, persistence, background work and recovery

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R08.
Write `../results/R08/{REPORT,trace}.md` and evidence. Source paths are repo-relative.
Use scratch projects/assets and an approved isolated library, not user originals.

## Outcome

A user can change their mind, stop work, recover and return later without losing
intent or confusing an old result with evidence about the current job.

## Task cards

1. **One edit at a time:** V5 with current generated/simulated results. Change
   tool shape, feed, stock/material, heights, boundary and setup orientation in
   separate restored copies. Before each, predict affected operations/results.
   Record dirty state, stale markers, auto/manual generation and export readiness.
2. **Commit semantics:** edit tool fields, switch selection without Apply, use
   Revert, close a dialog and undo/redo. Repeat with an operation field and a
   machine/project setting. Ask the user when they believe each change commits.
   Inspect whether undo restores dependent meaning, not just the visible number.
3. **Ordering/deletion:** duplicate/remove/disable an op, remove a referenced
   tool/model in scratch and undo. Inspect surviving selections/references and
   clear refusal versus silent fallback. Coordinate chain cases with R05.
4. **Return tomorrow:** save/reopen a scratch job and reconstruct active inputs,
   enabled operations, intended outputs and what must be regenerated. Compare
   editable state with intentionally non-persisted toolpaths/simulation evidence.
   Attempt opening another job with unsaved work and closing with unsaved edits.
5. **External dependencies:** F7/V9. Open a missing-model project, repair a scratch
   reference through available GUI routes, reload a revised model and check units,
   selection and downstream invalidation. Test moving the scratch project with
   its assets; do not silently repair path errors for the user.
6. **Library reuse:** import a tool/machine snapshot, alter the scratch catalog
   entry, reopen the job and ask whether the project should change. Check what
   the GUI communicates about copies, ownership and saving back to a library.
7. **Long or failed work:** use a bounded review-owned generation/simulation.
   Observe queue/stage/progress, switch workspaces, cancel and retry. Test an edit
   while work is in flight. Verify a late result cannot visibly become current
   evidence for changed inputs. Use scripted tests as supplements, not proof of
   real responsiveness. Do not kill a user process to simulate a crash.

## Specific questions

- Can users predict what will be invalidated and how to get current again?
- Is merely inspecting a tab harmless? Do hidden auto-commits contradict Apply?
- Are saved project state, cached computation and exported files clearly distinct?
- Can the user distinguish waiting for a prerequisite, queued, computing,
  cancelling, failed and completed? Is cancellation genuinely observable?
- Are notifications persistent/actionable enough to support recovery?
- Is starting fresh, saving a variant or restoring a baseline practical through
  the GUI, or dependent on manual file editing? Do not assume autosave/versioning.

## Source anchors

- `crates/rs_cam_viz/src/controller/io.rs`: import/reload/save/load and warnings.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:90-204`: draft/snapshot flushing;
  tool edit handlers and `src/ui/properties/tool.rs:19-52`.
- `crates/rs_cam_viz/src/controller/events/{undo,model,toolpath,compute}.rs`:
  undo, dependent edits and compute dispatch; follow result handling in the
  current controller with code search.
- `crates/rs_cam_viz/src/app.rs`: close interception and warning dialogs.
- `crates/rs_cam_viz/src/ui/{status_bar,viewport_overlay}.rs` and
  `src/ui/components/freshness.rs`.
- `crates/rs_cam_viz/src/controller/{tests,workflow_tests}.rs` and
  `crates/rs_cam_viz/tests/mcp_escape_hatches.rs`.

## Deliverable additions and boundary

Provide a **mutation → commit → affected inputs/results → visible freshness →
undo → save/reopen** matrix. Include a late-result/cancellation trace and a
missing-reference recovery trace, or explicit blocks where not reproducible.

R01 owns first import; R04 owns recommendation apply; R05 owns chain-specific
invalidation; R07 owns the final export consequence. R08 synthesizes the shared
lifetime model rather than repeating each feature's review.
