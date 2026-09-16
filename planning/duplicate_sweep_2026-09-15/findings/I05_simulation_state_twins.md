# I05 — Simulation state twins
Investigated by: Claude (work account), 2026-09-16. Read-only deep-dive.

**Verdict: mixed — see sub-verdicts.** No `apply(Command)` violation found; two live data models and one visibility-forced copy do exist.

| # | Pair | Verdict |
|---|---|---|
| 1 | viz `state/simulation.rs:2907-2962` ↔ core `simulation_cut.rs:1949-2007` | DRIFTED_DUP (test scope) |
| 2 | viz `SimCheckpoint` `state/simulation.rs:546-576` ↔ core `SimCheckpointMesh` `compute/simulate.rs:273-309` | FALSE_POSITIVE (already delegates) |
| 3 | viz `ToolpathBoundary` `state/simulation.rs:518-528` ↔ core `SimBoundary` `compute/simulate.rs:261-271` | TRUE_DUP (3-way) |
| 4 | viz `Fixture` `state/job.rs:61-128` ↔ core `Fixture` `session/mod.rs:510-563` | DRIFTED_DUP (legacy-only) |
| 5 | viz `KeepOutZone` `state/job.rs:131-179` ↔ core `KeepOutZone` `session/mod.rs:565-600` | TRUE_DUP (legacy-only) |
| 6 | core `polygons_bbox` `session/mutation.rs:82-114` ↔ viz `session_polygons_bbox` `state/job.rs:436-463` | TRUE_DUP (visibility-forced) |

## Evidence

**Q2 — job.rs does NOT reimplement session mutation.** Every live fixture/keep-out edit builds a **core** `session::Fixture` / `session::KeepOutZone` and dispatches a command: `controller/events/model.rs:2-7` imports them from `rs_cam_core::session`; `:534-551` builds `Command::AddFixture`, `:570-574` `Command::RemoveFixture`, `:602-612` `Command::AddKeepOut`. Readers read core too: `interaction/picking.rs:120-146` and `app/gpu_upload.rs:611-638` iterate `session.list_setups()`. Sentry: `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs`, `egui_draw_sites_write_through_commands_wp6.rs`.
The viz `Fixture`/`KeepOutZone`/`Setup`/`JobState` in `state/job.rs` are reachable only from `io/project.rs` (the legacy project reader) and the fallback branch at `controller/io.rs:526-540`, which converts them to core types field-by-field at `controller/io.rs:722-764`. That is I01's parallel IO model, not a mutation hatch.

**Q3 — viz simulation state transforms, it does not recompute.** `SimulationState::cached_simulation_triage` (`state/simulation.rs:1055-1081`) calls `session.simulation_triage`; `cached_chipload_envelopes` (`:1008-1038`) calls `rs_cam_core::tool_load::chipload_envelopes_for_session`; `SimCheckpoint` holds `Arc<core::SimCheckpointMesh>` and forwards through `mesh()`/`stock()`/`stock_local_to_global()` (`:553-576`, doc note "Shared … not owned (S5)"). `issues()` (`:2001-2064`) folds core trace hotspots plus debug/semantic annotations into a display list — presentation, not re-derivation of cut metrics.
**The one computation that does exist in both places is the boundary record**, copied three times: core `SimBoundary` → viz worker `SimBoundary` (`compute/worker.rs:204-212`, mapped at `compute/worker/execute/mod.rs:243-254`) → viz `ToolpathBoundary` (mapped at `controller/events/compute.rs:642-653`) → mapped *back* to core `SimBoundary` at `controller/events/compute.rs:2244-2255`. All six fields and the `direction` doc comment are identical at every hop.

## Drift / differences

- **Pair 1 (authoritative: core).** Core's fixtures use the shared constructor `SimulationCutSample::test_fixture()` (`simulation_cut.rs:248`, `pub`, not `cfg(test)`) with struct-update syntax. The viz test module spells all 19 fields out longhand, twice (`state/simulation.rs:2910-2935`, `:2936-2961`). A field added to `SimulationCutSample` updates core's fixtures for free and breaks the viz ones. Both regions are `#[cfg(test)]` — the sweep's "tests excluded" filter missed inline test modules.
- **Pair 3 (authoritative: core).** No behavioural drift today; the risk is that `SimBoundary` and `ToolpathBoundary` can gain a field independently and the three map sites silently drop it.
- **Pair 4 (authoritative: core).** Default fixture size disagrees in three places: viz `Fixture::new_default` uses `size_y: 15.0` (`state/job.rs:91`), core's serde default is `30.0` (`session/mod.rs:544-546`), and `controller/events/model.rs:543` re-hardcodes `15.0`. A legacy file missing `size_y` therefore restores a different clamp than the GUI creates. Core also derives `PartialEq` (the WP6 no-op-edit guard, `session/mod.rs:511-515`) and serde; viz derives neither. `FixtureKind` split: viz owns `ALL`/`label()` (`state/job.rs:44-58`), core owns `from_key` (`session/mod.rs:500-507`), and `io/project.rs:653-656` re-implements the missing `to_key` inline.
- **Pair 6 (authoritative: core).** Same algorithm, same `z = 0.0..=0.0` result. Literal differences only: core takes `&[Polygon2]` and uses `if pt.x < min_x`; viz takes `Option<&[Polygon2]>` and uses `min_x.min(pt.x)` — equivalent, including for NaN. The copy exists because core's is `pub(crate)`. Viz also carries `Fixture::bbox`/`clearance_bbox` (`state/job.rs:98-122`) beside free functions `session_fixture_bbox`/`session_fixture_clearance_bbox` (`:466-490`); the comment at `:347-349` admits the mirror.

## Proposed cleanup

1. **Pair 6 — home: `rs_cam_core::session` (make `polygons_bbox` `pub`, re-export).** Risk: low. Delete `state/job.rs:436-463`; point the three viz callers (`controller/events/model.rs:351`, `app.rs:295`, `ui/properties/mod.rs:160`) at core. Proof: `cargo test -p rs_cam_viz --test the_heights_diagram_fits_f2 -q`.
2. **Pair 3 — home: `rs_cam_core::compute::simulate::SimBoundary`.** Risk: low-med. Delete `compute/worker.rs:204-212` and viz `ToolpathBoundary`; re-export core's type under both names, then drop the three map sites. Proof: `cargo test -p rs_cam_viz --test export_parity_core_vs_gui_p0 -q` plus `controller/results_parity_tests.rs`.
3. **Pair 1 — home: `SimulationCutSample::test_fixture()`.** Risk: low (test-only). Rewrite `state/simulation.rs:2907-2962` with `..SimulationCutSample::test_fixture()`. Proof: `cargo test -p rs_cam_viz state::simulation -q`. Also fix `scripts/duplicate_sweep.py` to drop inline `#[cfg(test)]` chunks.
4. **Pairs 4+5 — home: `rs_cam_core::session::{Fixture, KeepOutZone, FixtureKind}`; add `bbox`/`clearance_bbox`/`to_key` there.** Risk: med — **sequence this inside I01**, because deleting the viz twins means `io/project.rs` must read core types and the `controller/io.rs:526` fallback loses its converter. Reconcile the `size_y` default (15.0 vs 30.0) as a **separate, earlier** change: it is a live legacy-load divergence, not a merge artifact. Proof: `cargo test -p rs_cam_viz --test apply_contract_a3 -q` and the legacy round-trip tests at `io/project.rs:1824-1911`.

**Workspace-rule note:** the `apply(Command)` contract holds. The rule this item does touch is "do not add a parallel one-off flow" — `state::job::{JobState, Setup, Fixture, KeepOutZone}` is a second, still-compiled data model of the session, kept alive solely by the legacy loader. That is I01's call to make.
