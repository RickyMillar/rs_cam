# W4: one simulation-freshness function

Implementation brief for `PLAN.md` §4.5 and §6 W4. It closes the ledger
**G-FRESHNESSDISAGREE** (`planning/arch_consolidation_2026-09-09/STATUS.md:581`,
indexed at `planning/PROGRESS.md:170`). Nothing is implemented; no cargo ran.

The ledger says: after a fresh `run_simulation` and a regenerate, the inspector
read `✓ live` while the Optimize window's `FreshnessGate::banner` read
`⚠ Results stale (params changed) — re-run sim`. Its stated cause (`stale_since`
on disabled rows) is marked NOT MEASURED. This brief does not reproduce that
pair: `✓ live` has no literal in `crates/rs_cam_viz/src`. One half of the row IS
refuted from source: the banner is `optimize_modal.rs:49`, which reads
`is_stale(edit_counter)`; `is_stale` reads `last_run` and
`metric_options_are_stale` and never `stale_since`, so the stated likely cause
cannot produce the banner. What did produce it is not answerable from here. W4
closes the STRUCTURAL split the row names; the on-screen pair stays unverified.

## 1. The truth table today

**Core.** `ProjectSession::simulation` is dropped at 19 sites (9 in
`session/mutation/toolpath.rs`, 6 in `mutation/config.rs`, 4 in
`mutation/entities.rs`; two further `rg "simulation = None"` hits are doc
comments). `Effects` reports `simulation_cleared =
simulation_before && self.simulation.is_none()` (`session/command.rs:2572`;
field at `:1073`). The public read is `ProjectSession::simulation_result()`
(`session/mod.rs:1440`).

**GUI.** `state/simulation/playback_state.rs:168-172`:

```rust
pub fn is_stale(&self, current_edit_counter: u64) -> bool {
    self.last_run.as_ref().is_some_and(|meta| {
        current_edit_counter > meta.last_sim_edit_counter || self.metric_options_are_stale()
    })
}
```

`GuiState::mark_edited` (`state/runtime.rs:341-344`) sets `dirty` and bumps
`edit_counter`. It has about 100 call sites.

A third actor decides what the GUI SHOWS. `invalidate_simulation`
(`controller/events/simulation.rs:26-37`) clears `results`, `playback`,
`checks`, `last_run` and both submit stamps. Three doors call it on
`Effects::simulation_cleared`: `apply_controller_command`
(`controller/events/mod.rs:64-70`), `apply_quietly` (`:101-105`) and
`apply_panel_command` (`ui/properties/panel_apply.rs:154-165`, through
`PanelSideEffects::invalidate_simulation`).

| Class | Core sim | Counter | Mirror | Reading |
|---|---|---|---|---|
| (i) both act | cleared | bumped | yes | `NoRun`. Correct. |
| (ii) counter only | cleared | bumped | **no** | stale by the counter alone |
| (iii) counter alone | KEPT | bumped | n/a | **false stale** |
| (iv) neither | cleared | no | no | **false fresh** |

**(i)** Toolpath parameter, tool, model, stock, setup, enable toggle, reorder,
remove and `set_toolpath_operation`. Each reaches
`invalidate_output_dependents_of_set` (`session/mutation/toolpath.rs:307`),
`drop_all_results` (`:802`) or `drop_setup_results` (`:846`), and each GUI route
takes a mirroring door. The GUI then reads **nothing**, not "stale": the mirror
wipes `last_run` too. That is the controller invariant, and it is why `is_stale`
is near-unreachable on GUI paths.

**(ii), the counter is load-bearing.** Every MCP mutation: `app/mcp.rs:361`
applies straight to the session and never reads `simulation_cleared`;
`describe_core` calls `mark_edited` at 28 sites (`app/mcp/commands.rs:1792 …
:2505`). The preparation half of an `ApplyPair` is NOT in this class:
`run_core_preparation` (`app/mcp/commands.rs:261-274`) mirrors through
`adopt_post_effects`; only the row's own command at `app/mcp.rs:361` does not.
This is WP28 parts 1 and 4, G-MCPSIMMIRROR, still open (`STATUS.md:273`).
Optimize Apply,
`controller/events/mod.rs:890-912`, applies `RestoreToolpathSnapshot`, stamps
`Effects::stale`, calls `mark_edited` at `:910` and mirrors nothing; the redo
twin is at `:1644` and the feeds funnel at `:1081`.

**(iii), the core is right and the counter is wrong.** `set_post_config` clears
the simulation ONLY when `post_change_reaches_motion`
(`session/mutation/config.rs:516-524`); a `format` or `spindle_strategy` change
keeps it while the GUI bumps anyway (`app/input.rs:119`,
`controller/io.rs:106-158`); WP17's `save_keeps_the_simulation_wp17.rs` pins the
core half. The export wizard bumps for nine GUI-only fields
(`app/input.rs:57,62,67,72,77,82,90,98,103`), which `state/runtime.rs:307-316`
itself calls GUI state, not project data. `ui/properties/machine_panel.rs:80`
bumps after SAVING the machine to the library, which writes a file and changes
no project state. `set_keep_out` drops setup results only when
`keep_out_collision_inputs_moved` (`session/mutation/entities.rs:440-446`).

**(iv), the dangerous class.** `insert_result` does not clear the simulation
(`session/mutation/toolpath.rs:720-730`) and `AdoptResult`
(`session/command.rs:2198-2216`) bumps no counter. Generate a toolpath that had
no result while a simulation is held: both stores read fresh and the run never
saw that motion. Only `SimEvidenceMeta` catches it (§7). `InvalidateMachine`
(`session/mutation/config.rs:312-316`) clears the simulation and drops no
result; it is a `Reach::Skip` row everywhere (`app/mcp/commands.rs:1750`), so it
has no live caller to classify.

**Metric options** are runtime-only: `set_metric_capture_enabled`
(`playback_state.rs:147-155`) bumps `metric_options_revision` and touches no
project state, so the core cannot see it, and `metric_options_are_stale`
(`:160`) already answers it apart. This half stays in the GUI. **Resolution**
(`resolution`, `auto_resolution`) is a GUI field too: a re-run at a different
resolution counts as stale on neither store today, and W4 does not change that.
Record that as accepted.

## 2. D7: a late simulation re-fills the core. This blocks W4.

`Command::AdoptSimulation` (`session/command.rs:2219-2229`) stores
unconditionally:

```rust
Command::AdoptSimulation(args) => {
    let AdoptSimulationArgs { result } = args;
    Ok(self.with_effects(None, move |session| { session.simulation = Some(*result); }))
}
```

`AdoptResult` beside it (`:2198-2216`) compares the submitted `revision`
against `toolpath_revision(index)` and refuses with
`SessionError::StaleCompletion`. `AdoptSimulation` has no such check. So an edit
clears the field, the in-flight run lands, `adopt_simulation_result`
(`controller/events/compute.rs:820-840`) adopts it, and the core reads `Some`
again. A core-based reader would call that run CURRENT. That is F2.10 /
G-LATESIM reopened, and `an_edit_during_a_simulation_leaves_the_result_stale_g_latesim`
(`controller/tests/undo.rs:427-486`) would go red. It is not only a display
defect: `ProjectSession::start` reads that field for `FromRemainingStock` prior
stock, so the stored run hands a rest operation a superseded snapshot.

**Option A, recommended, core.**
1. Add `ProjectSession::simulation_epoch: u64` beside `next_revision`
   (`session/mod.rs:1258`), and `pub fn simulation_epoch(&self) -> u64`.
2. Add one private `drop_simulation(&mut self)` that writes `self.simulation =
   None; self.simulation_epoch += 1;` and route all 19 assignment sites through
   it. That is the consolidation `drop_result` already is
   (`toolpath.rs:760-770`). Bump unconditionally, so two edits move it twice.
3. `AdoptSimulationArgs` gains `epoch: u64`; `apply` refuses a mismatch, exactly
   as `AdoptResult` does. The submit path
   (`controller/events/simulation.rs:269-271`) stamps
   `submitted_simulation_epoch: Option<u64>` instead of the edit counter.

**An unstamped result never reaches the core.** Today the drain falls back to
the live counter when `submitted_edit_counter` is `None`
(`controller/events/compute.rs:894`, "not a claim this guard can make"). Under A
there is no fallback: no stamp means skip `session.apply(AdoptSimulation)`, so
the view holds the result, the core stays `None`, and the state reads
`EditedSince`. That is the rule `accepted_metric_options_revision` already
applies. The rename carries the three clear sites with it
(`compute.rs:931, 945` and `controller/events/simulation.rs:34`); `Running` is
derived from that stamp, so a terminal path that forgets to clear it sticks.

`next_revision` cannot serve: `invalidate_machine`, `set_machine`,
`set_machine_kinematics`, `import_machine_settings`, `set_post_config`,
`replace_tools` and `set_toolpath_enabled` clear the simulation and bump no
toolpath revision. Under A the view KEEPS the result (`state.simulation.results`
is written before the adopt) and the core refuses it, so F2.10's rule holds:
stored, and marked not-current. The one `simulation = Some(..)` site inside the
core (`session/compute/simulation.rs:343`) needs no epoch: it is synchronous and
in-process, so no edit can land between its inputs and its store.

**Option B, viz only.** Gate the call: `adopt_simulation_result` skips
`session.apply(AdoptSimulation)` when `submitted_edit_counter !=
Some(gui.edit_counter)`. `edit_counter` then keeps one job, the adopt gate, and
`Running` reads `submitted_edit_counter.is_some()` instead.

A is recommended: the guard sits where `AdoptResult`'s guard sits, and
`simulation_result().is_some()` then means the same on every surface. Check
either way that `resume_generate_all_after_simulation`
(`controller/events/compute.rs:919`), which runs after the adopt, does not leave
the ladder waiting on a refusal.

## 3. The new function

Mirror `FreshnessState`'s style (`state/freshness.rs:25-44`): derived, never
stored, with a thin predicate over it. Put both in `state/freshness.rs`.

```rust
/// Where the simulation stands. Derived on every read: the core answers for
/// every project input, the GUI for capture options alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimFreshness {
    NoRun,                 // no run landed, or an edit cleared core and view
    Running,               // a run is in flight
    Current,               // the core holds the simulation these inputs produce
    EditedSince,           // the core dropped it; the view still draws the trace
    CaptureOptionsChanged, // the project is unchanged, the capture options are not
}

impl SimFreshness {
    pub fn label(self) -> &'static str { /* one word per arm, as FreshnessState */ }
    /// Every arm a reader must not present as current evidence.
    pub fn is_stale(self) -> bool {
        matches!(self, Self::EditedSince | Self::CaptureOptionsChanged)
    }
}

pub fn simulation_freshness(session: &ProjectSession, sim: &SimulationState) -> SimFreshness {
    if sim.submitted_simulation_epoch.is_some() { return SimFreshness::Running; }
    if !sim.has_results() { return SimFreshness::NoRun; }
    if session.simulation_result().is_none() { return SimFreshness::EditedSince; }
    if sim.metric_options_are_stale() { return SimFreshness::CaptureOptionsChanged; }
    SimFreshness::Current
}
```

`Running` reads the outstanding submit stamp, which lives on `SimulationState`:
the compute lane's status is on the controller, not on `AppState`, so a draw site
cannot see it. Add one facade on `AppState`, `sim_freshness()` calling
`simulation_freshness(&self.session, &self.simulation)` and
`simulation_is_stale()` calling `self.sim_freshness().is_stale()`, so each reader
changes one line. Delete `SimulationState::is_stale(u64)`; keep
`metric_options_are_stale` and `has_results`, which the new function calls.

**`EditedSince`.** Core `None` plus a view trace IS exactly
`FreshnessState::EditedSince` for the simulation, and the name should match. It
is reachable only where the GUI does not mirror: the MCP door, Optimize Apply,
and a refused late adopt. Everywhere else the mirror wipes the view and the
answer is `NoRun`. Say so in the enum's doc, or a reader will look for a state
the GUI usually erases.

### 3.1 The readers, one line each

Replace `sim.is_stale(gui.edit_counter)` with `state.simulation_is_stale()`.

| File:line | Today |
|---|---|
| `ui/sim_op_list.rs:56` / `:208` | header state branch / row chip |
| `ui/sim_timeline.rs:54` / `:298` | `has_results() && is_stale` banner / plot dimming |
| `ui/sim_diagnostics.rs:228` | banner |
| `ui/status_bar.rs:132` | status word |
| `ui/workspace_bar.rs:227` / `:250` | tab badge / `has_results() && is_stale` |
| `ui/readiness_panel.rs:101` | `has_results() && is_stale`, shown at `:143` |
| `ui/readiness.rs:175` | `readiness::simulation_check` |
| `ui/optimize_modal.rs:49` | `baseline_stale` → banner at `:60` |
| `ui/optimize_project.rs:56` | banner at `:57` |
| `ui/preflight.rs:81` | pre-flight gate |

Nine of the thirteen guard with `has_results()` or an enclosing "no results"
branch. `SimFreshness` carries that distinction, so each loses its second read.

## 4. The collision-check stamps

`checked_at_edit_counter` (`state/simulation.rs:725`),
`submitted_collision_edit_counter` (`:813`) and `submitted_collision_scope`
(`:823`), read by `collision_check_is_stale` (`playback_state.rs:187-192`) and
by `readiness::holder_clearance_check` (`ui/readiness.rs:240-310`).

The check reads the holder assembly, the workholding and the emitted motion, and
every one of those clears `session.simulation`: the holder assembly through
`set_tool_param`, `replace_tool` and `invalidate_tool`, which all reach
`drop_tool_results` → `invalidate_output_dependents_of_set`
(`session/mutation/toolpath.rs:307`); the workholding through `add_fixture`,
`remove_fixture`, `add_keep_out`, `remove_keep_out`
(`session/mutation/entities.rs:466, 488, 510, 532`) and `set_keep_out` when the
collision inputs moved (`entities.rs:440-446`); the population through
`set_toolpath_enabled` (`:406`); the setup frame through `drop_setup_results`
(`:846`).

So under option A the stamps move to the same read: store `checked_at_epoch:
Option<u64>` and answer `checked_at_epoch != Some(session.simulation_epoch())`.
Under option B they keep the counter. **The verdict has no core mirror**: there
is no `AdoptCollision` row (`rg AdoptCollision crates/` is empty), so it lives
in view state alone and the stamp is the only record of when it was measured.
Keep the two-stamp submit shape either way, and keep the rule that staleness
withdraws a CLEARANCE claim and never a measured STRIKE. W4 may defer the
collision move to a follow-up; state which in the fix body.

## 5. `edit_counter` after W4

**No `mark_edited` caller is deleted.** `mark_edited` also sets `dirty`, and
every caller is live for that. `ReachOverlayState::edit_counter`
(`state/runtime.rs:120-132`) also uses `gui.edit_counter` as its scheduling key;
that reader survives and its known blink stays known. What changes is who READS
the counter for staleness: nothing, after W4, except the collision stamps under
option B.

The class (iii) sites stay false-DIRTY: the wizard rows (`app/input.rs:57-103`),
`ui/properties/machine_panel.rs:80` and the non-motion post fields. That is
G-DIRTYONLOAD (`STATUS.md:591`), not W4. Ledger it; do not fix it here. Dead
after W4: `SimulationRunMeta::last_sim_edit_counter`
(`state/simulation.rs:753-755`) loses its only reader, and `sim_generation` has
none already (its only mentions are its own write at
`controller/events/compute.rs:876, 898` and a fixture at
`controller/tests/mod.rs:1113`). Delete both fields; keep
`accepted_metric_options_revision`.

## 6. The MCP mirror

**The MCP surfaces report no simulation staleness at all today.** Every
simulation reader guards on presence alone and answers "No simulation result.
Run run_simulation first." (`app/mcp/simulation.rs:82, 119, 269, 373, 388, 401,
419`). `mcp_project_summary` (`app/mcp/project.rs:19-39`) carries
`stale_defaults`, a validator over toolpath defaults, and nothing about the
simulation. `app/mcp/diagnostics.rs` matches nothing for `stale`.
`mcp_list_toolpaths` (`project.rs:43-77`) reports per-toolpath `stale` from
`ToolpathRuntime::stale_since`; that is a different question and it stays.

An agent can therefore read collision counts, engagement and air-cut off a
superseded run with no cue. Align: add `"simulation": {"freshness": <label>}` to
`project_summary` and `get_diagnostics`; have `run_simulation`'s reply carry the
label for the run it accepted; and where the state is `EditedSince` rather than
`NoRun`, say so instead of "No simulation result". The labels are the enum's, as
`FreshnessState::label` already does it (`state/freshness.rs:47-57`).

## 7. The core's own hash, a third truth

`SimEvidenceMeta::{Missing, Fresh, Stale}` (`gcode/mod.rs:489-531`) and
`sim_trace_is_fresh` (`:400-440`) answer a DIFFERENT question: does this cut
trace cover the emitted motion this project now holds? It hashes every enabled
toolpath's emitted moves and its operation config against the trace's
provenance block.

**It stays.** It is strictly stronger on class (iv): a toolpath generated while
the simulation was held has a result but no provenance hash, so
`sim_trace_is_fresh` returns false where both other stores read fresh. It feeds
a report, not a chip: `project_load_report` rewrites
`Unmodeled::SimulationRequired` into `Unmodeled::StaleSimulation`, which the
diagnostics adapter renders as `DiagnosticState::StaleEvidence`. **Do not make
`AdoptResult` clear the simulation to close class (iv):** the `generate_all`
ladder reads `session.simulation` for prior stock between regenerates inside one
round. Align the WORDS only, so `StaleSimulation`, `StaleEvidence` and
`SimFreshness::EditedSince` read as one vocabulary.

## 8. Sentry

`crates/rs_cam_viz/src/controller/tests/sim_stale_is_the_core_answer_g_freshnessdisagree.rs`,
a new child module registered in `controller/tests/mod.rs` beside `freshness.rs`
and `simulation_state.rs`. The harness is `controller/tests/mod.rs`:
`sample_controller()`, `generate_all_for_test()`, `panel_edit()` and
`inject_sim_results()` (`:366-420`), which pushes a `ComputeMessage::Simulation`
onto `ScriptedBackend::drained` so the drain runs `adopt_simulation_result`.
The D7 arm needs that route. Assert the STATE, not `is_stale ==
simulation_result().is_none()`: the GUI mirror wipes the view, so the two are
not equal on GUI paths.

1. GUI edits after a run (toolpath parameter, tool, stock, setup, enable
   toggle, reorder) → `NoRun`, with `session.simulation_result().is_none()`.
   **Read `panel_edit` (`controller/tests/mod.rs:1068-1087`) first.** It ends in
   `write_entry_config_to_session` (`ui/properties/mod.rs:701-725`), which only
   RAISES `PanelSideEffects::invalidate_simulation`; the frame loop discharges
   it, and the controller harness runs no frame loop. Under `panel_edit` alone
   these arms read `EditedSince`, not `NoRun`. Drive a door that mirrors
   synchronously (the enable-toggle and reorder handlers in
   `controller/events/toolpath.rs` take `apply_quietly`), or discharge
   `state.panel_side_effects` in the arm.
3. MCP `set_toolpath_param` through the MCP door → `EditedSince` if W4 leaves
   `app/mcp.rs:361` unmirrored, `NoRun` if it routes through a mirroring door.
   Pick one and pin it.
4. Metric-capture toggle → `CaptureOptionsChanged`, with
   `session.simulation_result().is_some()`. This arm proves the GUI half is
   still needed.
5. Export-wizard change → `Current`; post `format` change → `Current` (WP17).
7. D7: submit, `panel_edit`, `inject_sim_results` → `EditedSince`,
   `has_results()` true, `session.simulation_result().is_none()`. This arm works
   BECAUSE `panel_edit` leaves the submit stamp alone. Do not "fix" it to a
   mirroring door.
8. Clean run → `Current`; in flight → `Running`.

Red-first. Arms 5 and 7 are red against today's code.

## 9. Existing tests that move

| Test | File:line | Move |
|---|---|---|
| `an_edit_during_a_simulation_leaves_the_result_stale_g_latesim` | `undo.rs:427-486` | Keeps its claim. The reader swaps; the `last_sim_edit_counter` assertion at `:471-484` becomes "the core refused the adopt". **F2.10 stays covered.** |
| `a_simulation_with_no_edit_in_flight_is_current_g_latesim` | `undo.rs:487-500` | One-line reader swap. |
| `the_submit_stamp_belongs_to_one_run_g_latesim` | `undo.rs:501-517` | Stamp renamed under A, and its CLAIM changes: a second result with no submit behind it no longer falls back to the live counter, so it reads `EditedSince`, not current. |
| `undo.rs:106`, `workspace.rs:89, 100` | | Reader swap; the claim is arm 4. |
| `holder_clearance_staleness_g_holderstale.rs` | whole file | Unchanged under B; renamed stamp under A. |
| `workspace.rs:127-287`, `effects_are_stamped_wp19.rs`, `save_keeps_the_simulation_wp17.rs` | | Unchanged. The last two guard W4: the mirror, and arm 5's core half. |
| `adopt_simulation_stores_prior_stocks.rs:208` | core | **Moves under A**: the fixture must stamp a matching epoch. |

`rg "G-LATESIM|G-HOLDERSTALE|F2.10" crates/rs_cam_viz` finds no further readers.

## 10. Blast radius, risks and rulings

13 draw-site readers, one new enum, one facade method, one deleted method. Under
option A `rs_cam_core` also gains a field, a helper, a public read and one
payload field; 19 assignment sites route through the helper, and the payload
change moves `command_registry_completeness` and
`adopt_simulation_stores_prior_stocks`.

- **Risk 1.** D7. Land the guard in the same commit as the reader swap, or
  G-LATESIM reopens silently between the two.
- **Risk 2.** The `generate_all` ladder reads the same field. Run
  `generate_all_fixpoint_parity` and `controller/tests/generate_all.rs`.
- **Risk 3.** `EditedSince` is rare on GUI paths. Do not design a chip that only
  that arm reaches before the MCP mirror is decided.
- **Risk 4.** W4 touches `app/mcp.rs:361` only on a ruling; that is WP28.
- Not touched: `ToolpathRuntime::stale_since`, `FreshnessState`,
  `Effects::stale`, the feeds and tool_load trees.

**R-W4.1** Option A (core epoch, refused late adopt) or option B (viz adopt
gate)? A is recommended and grows W4 into `rs_cam_core`.
**R-W4.2** Do the collision stamps move in W4, or in a follow-up?
**R-W4.3** Does W4 close the MCP mirror at `app/mcp.rs:361`, or leave it to WP28
and pin `EditedSince` in the sentry?
**R-W4.4** One word for stale evidence across the chip, the tool-load report and
the MCP label.
