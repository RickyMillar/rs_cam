# Memory programme 2026-10-01: budget, release and degrade

Status: RUNNING. The operator ruled 2026-10-01: the local runner session
orchestrates; agents implement in parallel worktrees; merge later on branch
`memory-budget-2026-10-01` (off `fd06f407`). The operator ruled
2026-10-02 on two more items:
- The default budget is half of the RAM. File > Preferences sets it (W4-M).
- The metric-capture toggle is deleted. Every simulation captures the
  metrics (W4-L).

File groups (disjoint, one agent each):
- A viz adoption: M1, live_stock, U4, U5.
- B core result shape: M2, M3, M4 (core side), M5 playback, M8.
- C budget module: new `budget/`, grid-cap unification, `interrupt.rs`,
  settings io, estimators. No `simulate.rs` edits.
- D UX: U2, U3 (`cycle_time.rs`, `readiness.rs`, null-not-zero surfaces).
- After the first merge: U1 (toggle), B4 (ledger), preflight, degrade.
Owner of the decisions: the operator. Evidence: five read-only code audits
(2026-10-01, runner session) and the job 006 RSS logs on `comms/runner`.

## Why

On rivmap350 (380 x 510 mm terrain, 7 toolpaths, an 800k-move scallop):

| Run (5b34e710 / 16 GB cgroup) | Peak | Result |
|---|---|---|
| GUI simulate 0.2 mm, metrics OFF, after a resolution change | 9.17 GB | ok, but no trace: no chipload, "cutting only" time |
| GUI simulate 0.2 mm, metrics OFF, no resolution change | 14.84 GB | ok |
| MCP `run_simulation` 0.2 mm (metrics ON) | > 16 GB | OOM-killed |
| GUI simulate 0.2 mm, metrics ON | > 16 GB | OOM-killed |
| GUI simulate 0.1 mm, metrics OFF | > 16 GB | OOM-killed |

The app has no memory limit of its own. The only guard is an external
Linux cgroup, and it kills the whole process. The operator lost work and
read empty numbers without a hint why.

G-SIMMEM (`75aa2869`) fixed the trace density. It did not touch what is
below.

## Model (estimates from code; M0 measures them)

C = cells per grid (0.2 mm: 1901 x 2551 = 4.85 M; 0.1 mm: capped at 16 M).
E = simulated toolpaths (7). G = setup groups (2).

- One dexel cell: 28 B (`SmallVec<[DexelSegment;1]>` 24 B + 4 B top).
- One marching-cubes mesh: about 96 B/cell.
- One kept simulation result:
  R = C x [E x (28 checkpoint stock + 28 prior stock + 96 checkpoint mesh)
  + G x 96 x 2 (composite, copied into the session) + 28 live_stock]
  + 64 B x moves (+ trace when metrics are ON)
  = about C x 1476 B = 7.2 GB at 0.2 mm, 23.6 GB at 0.1 mm.
- Generation keeps only 0.2 to 0.4 GB (toolpath IR 64 B/move, traces,
  mesh indexes, finish-surface cache).
- Generate All ends in a full simulation, so "after generate" already
  holds R. A new simulate builds R_new while R_old is still held:
  8.1 + 6.7 = 14.8 GB, which is the measured peak.

Open: after the 09-30 GUI 0.2 mm run, RSS fell to 2.0 GB; the model says
about 7 GB stays. M0 must explain this (candidates: an invalidation, or
glibc returning memory differently).

## Findings

Release and copies:
- M1 A new simulation does not release the old result before it starts
  (`events/simulation.rs:382`; only a resolution change calls
  `invalidate_simulation`). Saving: R_old at peak (6.6 GB measured).
- M2 Every checkpoint keeps a full display mesh (`simulate.rs:1151`).
  Saving: E x 96 x C (3.3 GB) if built on demand from the checkpoint stock.
- M3 `prior_stocks` are cloned for every toolpath, also when no rest op
  reads them (`simulate.rs:1349`); checkpoint k and prior stock k+1 are
  two clones of the same grid (`simulate.rs:1163`). Saving: up to
  E x 28 x C x 2 (1.9 GB).
- M4 The session deep-copies the composite mesh and deviations
  (`controller/events/compute.rs:2640`). Saving: G x 96 x C (0.9 GB).
- M5 `live_stock` is allocated on every adopt (`compute.rs:962`); the
  playback data deep-copies every toolpath (`worker/execute/mod.rs:81,100`);
  after feed modulation `rt.result` keeps the old toolpath beside the new.
- M6 Mesh colours are a function of Z (shader can compute them); transforms
  push into an unreserved mesh (`stock_mesh.rs:37`). About 1/4 of mesh bytes.
- M7 A dexel cell could pack to about 14 B (u16 quantised segments).
  Halves every grid. Largest change; last.
- M8 Samples are built and thrown away when metrics are OFF
  (`simulate.rs:1296`).

Budget and control:
- B1 No budget, no estimate before a job. `SimPrefixCache` (1.5 GB) is the
  only "ask before allocate" site, and its estimate counts pinned Arcs as
  8 B each and grids at 32 B/cell (`sim_prefix.rs:603-627`).
- B2 Four grid caps disagree: 16 M (`dexel.rs:331`, the one that clamps),
  8 M (`rest_stock.rs:63`, `session/compute.rs:2451`), 4 M (`dressup/mod.rs:280`).
- B3 A cancel has no reason: `Cancelled` is a bare variant in three error
  types, and lanes overwrite results with it (`worker.rs:967,1071`).
- B4 Five compute lanes run at the same time (`worker.rs:581-589`); nothing
  sums their memory. `dequeue_running` (`worker.rs:432`) is the one choke.
- B5 No app settings file (only tool and machine libraries under
  `~/.config/rs_cam`). No `#[global_allocator]`, no `malloc_trim`.

UX (from the operator: "not ideal. I've been clueless."):
- U1 GUI metric capture defaults OFF (`playback_state.rs:63`); session, MCP
  and CLI force it ON. Same project, different numbers per surface.
- U2 Without a trace, surfaces show 0.0 (air %, runtime) where the rule is
  null = NOT MEASURED.
- U3 "Cutting only" says "Run a simulation" after a simulation ran; it never
  names the cause. `CycleTimeBasis::worse` folds "no trace" and "no machine
  kinematics" into one label.
- U4 MCP `generate_all` sets `debug_options.enabled = true` on every
  toolpath (`compute.rs:1715-1734`) and the project file saves it.
- U5 The CLI writes `diagnostics/` (8 GB `simulation.json` on rivmap350)
  into the cwd by default (`rs_cam_cli/src/main.rs:145`).

## Architecture (fits the existing core -> worker -> UI path)

1. `rs_cam_core/src/budget/`: `MemoryBudget { limit_bytes }`; estimators
   (`estimate_simulation` = the R formula + trace; generation estimate);
   ONE grid-cap function from the budget, which replaces the four constants.
2. `BudgetGuard` (Arc per job): the cancel flag, a usage probe and a
   `StopReason::{User, OverBudget { need, limit }}`. It implements
   `CancelCheck`, so every existing `check_cancel` site gets the budget
   check with no signature change. Add `OverBudget` to the three error
   types; lanes stop overwriting it with `Cancelled`.
3. `BudgetLedger` in `ThreadedComputeBackend`: `dequeue_running` reserves
   the estimate; a heavy job that does not fit waits in its queue
   (queued, visible) instead of running beside another heavy job.
4. Preflight before simulate / generate_all / optimize: if the estimate
   does not fit, DEGRADE in a fixed order and say so, else REFUSE:
   a. release the old result (always, M1);
   b. build the display mesh on a coarser stride than the sim grid;
   c. drop checkpoint meshes (scrub rebuilds);
   d. coarsen the trace sample step;
   e. refuse, with need / limit / the cell size that fits.
   The simulation grid itself is never coarsened silently.
5. Surfaces: the existing `resolution_clamped` toast pattern
   (`events/compute.rs:756`), `MeasurabilityReason::MemoryBudget`, a
   `Category::State` diagnostic, the playback banner, and
   `generation_status.budget { limit, reserved, used }` for MCP.
6. Config: `~/.config/rs_cam/settings.toml` `[memory] limit`, resolved like
   the libraries (+ a Windows fallback); CLI `--memory-limit`; MCP inherits
   the GUI's value. Default = a fraction of system RAM (`sysinfo`).
7. Usage probe: `memory-stats` (process RSS) at checkpoints, not a counting
   global allocator (per-allocation atomics cost time everywhere). A hard
   limit inside the process cannot fail gracefully in stable Rust
   (`set_alloc_error_hook` is unstable), so the cgroup stays as the outer
   net on Linux. On other platforms the in-app budget is the only guard.
8. Large reservations use `Vec::try_reserve` (stable) and map failure to
   `OverBudget` (the 9.5 GB `with_capacity` at `dexel_stock/simulation.rs:440`
   and the mesh `indices` at `dexel_mesh_mc.rs:147`).

## Waves (baselines before fixes; paired A/B per wave)

- W0 Baseline (M0). rivmap350 + rivmap100 fixtures. Per phase: RSS from
  `/proc/PID/smaps_rollup`, `SimPrefixCache::stats()`, result sizes.
  GUI via MCP and CLI. A `malloc_trim(0)` arm. Commit the instrument first.
  Output: `BASELINES.md` with the measured R per cell and the 2.0 GB answer.
- W1 Release: M1, M4, M5 (live_stock lazy, playback shares Arcs), U4, U5.
- W2 Stock and mesh: M2, M3, M6, M8.
- W3 Budget core: B1, B2, B3, config, estimators checked against W0.
- W4 Ledger, preflight, degrade and surfaces: B4, section 4 and 5.
- W5 UX: U1 (delete the toggle, always capture, the budget decides the
  step), U2, U3.
- W6 M7 grid packing, only if W1-W5 leave 0.1 mm out of reach and the
  operator wants it.

## Acceptance

- rivmap350 at 0.2 mm with the trace: generate all, simulate, simulate
  again, scrub: peak under the configured budget (default: half of the RAM,
  operator ruling 2026-10-02; File > Preferences sets it),
  and it never reaches the cgroup.
- rivmap350 at 0.1 mm: a clear refusal or a named degrade before any
  allocation; the GUI stays up; the project is not lost.
- Outputs of each wave are bit-identical (collisions, runtime, volume,
  goldens) unless the wave says why they move.
- `sim_peak_memory_g_simmem` extends to the release path (simulate twice).
- GUI, MCP and CLI give the same numbers for the same project.

## Decisions for the operator

1. Who runs the programme: the cloud lead, or the local runner session.
2. The default budget fraction of system RAM. RULED 2026-10-02: half of
   the RAM, set in File > Preferences.
3. Delete the metric-capture toggle (always capture) — recommended. RULED
   2026-10-02: deleted; every simulation captures the metrics.
4. Whether W6 (grid packing) is in scope.
5. Order against job 007 and the tiered-finishing work.
