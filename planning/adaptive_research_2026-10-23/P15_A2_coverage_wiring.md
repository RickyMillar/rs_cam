# P1.5 editor A2: the coverage reading reaches the GUI

Editor A, 2026-10-23 (after 17:11 NZDT). Worktree `.claude/worktrees/adaptive-head`,
branch `adaptive-instruments` at 405cbd23. No cargo was run. Nothing is
committed. The verifier builds and runs everything below.

## The defect (P15_E_evidence.md, Finding 0)

`core_simulation_from_lane` (`rs_cam_viz/src/controller/events/compute.rs`)
gives the session copy of a GUI run `checkpoints: Vec::new()` (W2-E memory
work). The session reading (`rough_coverage_at`) found no checkpoint and
returned no reading. `triage.coverage` was `[]` on `run_simulation`,
`get_diagnostics`, `get_toolpath_diagnostics` and `narrate_toolpath`.
An empty list reads like a measured zero, which it was not.

## The fix: readings, not stock

The GUI reads the coverage ONCE, at adopt, from the VIEW's checkpoints.
The view keeps those checkpoints anyway. The readings, not the stock,
then reach the session through the evidence.

1. **Core** (`session/compute/simulation.rs`):
   - `ProjectSession::rough_coverage_for_run(boundary_ids, checkpoints)` is
     public. It gives one reading per enabled 3D Rough in the run, from any
     caller's checkpoints. A rough with no checkpoint reads
     `NotMeasurable(NoCheckpoint)`.
   - `rough_coverage_where` (behind `rough_coverage_readings` and
     `rough_coverage_reading`) answers in this order:
     1. the evidence carries readings: those, as given;
     2. the session holds the evidence's run (a core run: CLI, tests, which
        keep their checkpoints): computed from its checkpoints;
     3. otherwise: `NotMeasurable(RunNotStored)` per enabled 3D Rough in the
        evidence's boundaries.

     An enabled 3D Rough in a run is never absent from the list.
2. **Core** (`session/diagnostics_types.rs`): `ProjectEvidence::rough_coverage:
   Option<&[RoughCoverageReading]>`. `from_simulation` sets `None`. Every
   other literal uses `..Default::default()` (`ProjectEvidence` derives
   `Default`), so no other core site changes.
3. **Core** (`stock/standing_islands.rs`): new abstentions `NoCheckpoint` and
   `RunNotStored`, with `describe()` texts.
4. **Viz state** (`state/simulation.rs`, `state/simulation/playback_state.rs`):
   - `SimulationState::rough_coverage: Option<RoughCoverageMemo>`.
   - `RoughCoverageMemo::read(session, results)` holds the readings and the
     key of the run they came from: the boundaries, plus the address of the
     first checkpoint `Arc`.
   - `SimulationState::rough_coverage_of_results()` gives the readings only
     while `results` is that run.
5. **Viz adopt** (`controller/events/compute.rs`): when
   `Command::AdoptSimulation` succeeds, the drain sets the memo from
   `self.state.simulation.results` (the view's checkpoints) against the
   session as it now stands. The `run_simulation` reply is built later in
   the same drain (`notify_mcp_simulation_complete`), so it carries the
   readings.
6. **Viz evidence** (`state/simulation/issue_triage.rs`):
   - `project_evidence()` passes `rough_coverage_of_results()`.
   - The triage-cache `evidence_fingerprint` hashes the readings' address
     and length (an adopt replaces them).
   - `narrate_toolpath`, `get_toolpath_diagnostics`, `get_diagnostics` and
     `run_simulation` all build their evidence through `project_evidence()`,
     so the four surfaces read one list.

## Memory cost

- **Kept:** the readings only, a `Vec<RoughCoverageReading>` per run. Each
  island is about 100 bytes. No stock is kept or copied. The session copy
  still holds no checkpoint, so the W2-E claim and its strong counts are
  unchanged: test + view = 2 on the checkpoint `Arc`.
- **Transient, during the adopt read, per rough, freed after:**
  - the standing field (16 B per grid column);
  - the reach mask (1 B per column) and its distance transform (8 B);
  - the island distance transform (8 B per padded column).

  That is about 35–40 B per column: about 10 MB at a 0.25 mm cell on the
  Corne stock (about 600 × 440 columns).
- **Time:** the read runs on the GUI thread, once per adopted run. In the
  evidence run, one narration took about 2.4 s (round trip included); that
  is the scale per rough. The narration, diagnostics and triage calls after
  the adopt read the memo and do not walk the grid again. The
  `PlannedTierRegions` tail (P15_A_coverage.md, "Risks for the evidence
  run") now falls on the adopt, not on each call.

## Test

`crates/rs_cam_viz/src/controller/tests/the_gui_adopt_reads_the_rough_coverage_g_roughcov.rs`
(registered in `controller/tests/mod.rs`) has two tests:

- `a_gui_adopted_rough_reads_its_island_through_the_session_g_roughcov`:
  the sample project's toolpath becomes a 3D Rough (stock to leave
  0.5 mm). A lane answer is landed through the drain, with a checkpoint
  that holds one standing block. The test asserts:
  - the session copy holds no checkpoint (W2-E);
  - the evidence carries readings;
  - `session.simulation_triage(evidence).coverage` reads ONE island of
    9 × 17 cells;
  - the `project.rough_leaves_islands` finding is raised;
  - `rough_coverage_reading(id, evidence)` (the narration door) gives the
    same reading.
- `a_gui_run_with_no_checkpoint_for_the_rough_abstains_g_roughcov`: a lane
  answer with no checkpoint reads `NotMeasurable(NoCheckpoint)`, not an
  empty list.

`session_keeps_only_what_it_reads_w2e` is unchanged and must stay green.
Its op is a Scallop, so the memo is empty and holds no `Arc`.

## Files (A2)

- `crates/rs_cam_core/src/session/compute/simulation.rs`
- `crates/rs_cam_core/src/session/diagnostics_types.rs`
- `crates/rs_cam_core/src/stock/standing_islands.rs`
- `crates/rs_cam_core/src/session/CLAUDE.md`
- `crates/rs_cam_viz/src/state/simulation.rs`
- `crates/rs_cam_viz/src/state/simulation/playback_state.rs`
- `crates/rs_cam_viz/src/state/simulation/issue_triage.rs`
- `crates/rs_cam_viz/src/controller/events/compute.rs` (the adopt arm and the
  `core_simulation_from_lane` doc)
- `crates/rs_cam_viz/src/controller/tests/the_gui_adopt_reads_the_rough_coverage_g_roughcov.rs` (new)
- `crates/rs_cam_viz/src/controller/tests/mod.rs` (one `mod` line)
- `crates/rs_cam_viz/src/controller/CLAUDE.md` (sentry line)

## Verifier commands (through `scripts/cargo_lane.sh` in the worktree)

1. `cargo test -p rs_cam_viz -q --lib g_roughcov`
2. `cargo test -p rs_cam_viz -q --lib w2e` (the memory sentry, unchanged)
3. `cargo test -p rs_cam_viz -q --lib g_rapidframe` (it reads
   `diagnose_toolpath_with_evidence`)
4. `cargo test -p rs_cam_core -q --test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov`
5. `cargo test -p rs_cam_core -q --lib g_roughcov` and `--lib standing_islands`
6. viz `--lib` in full, `rs_cam_mcp` tests, then clippy (`--all-targets`).

## Re-run for the evidence

Rebuild the release GUI from this branch and `gui_restart`. Then run
`run_simulation` on Corne. `triage.coverage` must hold one reading per
enabled 3D Rough (measured or abstained), and the same readings must
appear in `narrate_toolpath` ("coverage: …") and
`get_toolpath_diagnostics`.

## Limits

- The memo is read against the session at adopt. If an edit that changes
  a rough's target does not drop the simulation, the memo describes the
  target as it stood at adopt. An edit to a rough's config or to the model
  drops its result and the simulation, so this case is expected to be
  empty. That was not verified edge by edge.
- If the adopt is refused (a stale run), there is no memo. The view still
  shows that run, and every rough in it reads `RunNotStored`.
