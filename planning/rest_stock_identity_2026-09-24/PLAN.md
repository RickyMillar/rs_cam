# G-RESTRES + G-RESTSTALE: rest stock identity and one stored resolution

Status: PLAN (Stage 1). No code changed. Written 2026-09-24 on master
`1b69ceaa`. Evidence: `adaptive3d_step_ladder_roughing_2026-09-24/RESULTS.md`
("Parity check", "Finish check"). Paths are under `crates/`. Each claim is a
code read, not a run.

## 1. Current mechanism

**Snapshot and key.** The simulator keeps the stock before each carved entry
in `SimulationResult::prior_stocks`, keyed by the consumer id only
(`rs_cam_core/src/compute/simulate.rs:514-517`). It has no cell and no
predecessor identity. `start()` refuses a `FromRemainingStock` op when the
key is absent (`session/compute.rs:2094-2111`), reads it by key (`:2148-2151`),
and `generator_seed_stock` (`:222-227`) seeds the generator. `start()` also
stamps `stats.stock_snapshot` (cell + content digest, S-4;
`session/compute.rs:941-949`, `compute/toolpath_stats.rs:389, 523-607`).
The stamp is report-only: no wire publishes it (`rg stock_snapshot` in viz,
cli, mcp: 0 hits) and no rule compares it.

**Two builders.** The GUI builder carves `gui.toolpath_rt[id].result`
(`rs_cam_viz/src/controller/events/simulation.rs:128-176`). The core builder
carves `session.results` (`session/compute/simulation.rs:110-140`).

**Staleness.** The walker drops results on edits
(`session/mutation/toolpath.rs:304-360`). A Stock edge exists only for an
ENABLED consumer (`session/dependencies.rs:155`). `AdoptResult` walks with
`chain_seeds` empty and keeps the simulation (`session/command.rs:2228-2267`).
`AdoptSimulation` stores on an epoch match with `stale` empty (`:2269-2295`).
`set_toolpath_enabled` keeps the op's own result
(`mutation/toolpath.rs:423-441`). `plan()` emits Simulate only when the key
is absent (`session/generation_plan.rs:132, 152-156`); the Stock arm of
`dependencies::state` reads the same key (`dependencies.rs:266-273`).
`stamp_stale` does not clear `rt.result` (`rs_cam_viz/src/state/stale.rs:42-61`),
and a refused adopt keeps it (`controller/events/compute.rs:583-613`).

**Resolution: four rules, no stored value.**
- GUI panel: `SimulationState::{resolution, auto_resolution}`, GUI-only, not
  saved (`state/simulation.rs:1044`, `ui/sim_op_list.rs:119-150`). Auto uses
  the tools of each request (`controller/events/simulation.rs:251-253, 577`).
- GUI plan (R1): `required_resolution_mm` over this plan's Simulate steps
  (`session/generation_plan.rs:230-248`) as `PlanResolution::AtMost`
  (`controller/generate_all.rs:205-230`); the confirm modal asks when a
  pinned panel is coarser (`controller/events/toolpath.rs:505-594`).
- MCP: `run_simulation.resolution` writes the panel
  (`app/mcp/simulation.rs:202-211`); `generate_all.simulation_resolution_mm`
  is required with rest ops (`controller/events/compute.rs:1614-1680`,
  `generate_all.rs:115`, `rs_cam_mcp/src/server.rs:188-210`);
  `generate_toolpath` goes through the GUI modal (G-MCPMODAL).
- CLI: `--resolution` is required when the plan simulates
  (`rs_cam_cli/src/project.rs:256-288, 334-356`). Core auto is a second copy
  (`session/compute/simulation.rs:255-259`).

## 2. Root causes

**G-RESTRES: confirmed, with one correction.** The result does record a cell
and a digest (S-4), but no surface shows it and no rule reads it. Causes:
1. The cell depends on the plan scope: the rest rule reads only this plan's
   Simulate steps; GUI auto reads only this request's tools.
2. The scallop's plan simulates at 0.2 mm and stores a snapshot for EVERY
   carved entry, the rough included. A later Generate on the rough plans no
   Simulate (key present), so the rough reads a 0.2 mm Face stock.
3. A snapshot has no provenance, so `plan`, `state` and `start` cannot tell
   0.2 mm from 0.5 mm; a simulation at another cell drops nothing.
4. The project file has no key: the CLI takes an argument, the GUI a dial.

**G-RESTSTALE: three candidate paths. RESULTS says only "at the restore".
NOT MEASURED.** Each gives the scallop B5's count:
- **M-C (most likely: two result caches).** The restore edit drops the rough
  and the scallop in core; `rt.result` keeps the B5 rough. A plain Run
  Simulation (GUI or MCP) carves B5 from `rt` and stores
  `prior_stocks[scallop]`; the epoch matches, so core adopts it. The rough
  (A) generates and the simulation stays. The scallop sees the key, plans no
  Simulate, and reads the B5 carve. Only this path puts B5 GEOMETRY in a
  snapshot while A is in place.
- **M-A (the gate M-C passes).** Key presence is the only test, and
  `AdoptResult` keeps the simulation. A predecessor whose output changes
  with no input edit leaves a snapshot of its old output.
- **M-B (held result).** A disabled consumer has no Stock edge, so an edit
  above it does not drop it, and re-enable keeps it. The scallop was
  disabled in Phase 1.
Load is not a door for M-C: it seeds `rt` with no result
(`controller/io.rs:469-474`). §3 closes all three. §5 has one reproduction test for each.

## 3. Design

**3.1 One stored resolution (core).** `ProjectSession::simulation_resolution:
SimulationResolution { Auto, Fixed(f64) }`. File key `[job.simulation]
resolution_mm = 0.2`; a missing key reads as `Auto` and the writer omits it
for `Auto`, so old files load. One function,
`ProjectSession::simulation_resolution_mm()`, gives the cell for EVERY
simulation: plan prefixes, the closing SimulateAll, Run Simulation, the CLI.
`Auto` is project-wide, never per scope or request: min(tool rule, rest rule),
where tool rule = smallest tool radius over enabled ops (generated or not) / 5,
clamp [0.02, 0.5], grid cap (today's formula), and rest rule = finest TIP radius / 5 over enabled
`FromRemainingStock` ops, floor 0.02 (R1's rule). The two auto copies call it
or go. `Command::SetSimulationResolution` is the one setter; it drops the
simulation (epoch bump), then runs the §3.3 sweep. The panel slider keeps a
draft and sends ONE command on release, not one per drag frame (the
`panel_drafts_leave_egui_memory_ui09` / `egui_draw_sites_..._wp6` pattern).

**3.2 The recorded source stock.** `SourceStock { cell_mm, after:
Vec<SourceEntry { id, output: u64, stock: Option<StockSnapshotStamp> }> }`.
`after` lists, in carve order, every enabled op the simulator carves before
the consumer: all earlier setups (R2), then the rows above it. `output` is a
content digest of the predecessor's emitted toolpath (one pass, like
`StockSnapshotStamp::of`), not its revision: `drop_result` bumps a revision
on every call (`mutation/toolpath.rs:827`), and the D1 walk calls it on
Regions consumers, so a revision would stale a chain whose carve did not
move. Tool, setup and stock edits already drop the simulation and the chain
through the walker. So an equal regenerate keeps the identity, and a
re-simulation that changes nothing stales nothing.
- `cell_mm` on BOTH sides comes from one function that applies the
  simulator's grid clamp (`DexelGrid::would_exceed_grid`, `stock/dexel.rs`;
  `simulate.rs:1375`). If one side used the stored cell and the other the
  clamped cell, a `Fixed` below the cap would never match, and the sweep
  would drop every rest result after every simulation.
- `ProjectSession::expected_source(id)` derives it from the current state;
  `None` when an op in `after` has no result.
- Both builders record `sources: HashMap<ToolpathId, SourceStock>` at SUBMIT
  (a predecessor can adopt while the run works). Core keeps them beside
  `simulation`; `AdoptSimulationArgs` gains `sources`; `drop_simulation`
  clears them.
- `has_current_snapshot(id)` = key present AND recorded source ==
  `expected_source(id)`. `start()`, `plan()` and `dependencies::state` read
  it. `start()` refuses a stale snapshot and names the reason (cell, or the
  op that changed), and writes `stats.source_stock` beside `stock_snapshot`.
- The GUI builder takes geometry and `drill_op` from `session.results`
  (M-C), and the semantic trace from `rt` by id only when `rt`'s annotated
  `Arc` is the core one. `rt.result` stays for the viewport. (Ledger item,
  not this package: the core builder already carves GUI-adopted results
  with no trace, because viz adopts `semantic_trace: None`, `compute.rs:566`.)

**3.3 The stale rule.** A rest result is out of date when its
`stats.source_stock` differs from `expected_source`. One function,
`rest_results_out_of_date()`, runs at the end of `ProjectSession::apply`,
INSIDE the `try_with_effects` closure so the revision diff reports it. It
drops each hit, walks dependents with the pure `walk_output_dependents` (no
simulation drop), and adds them to `Effects.stale`. It covers M-A, M-B, a
stored-value change, and an `Auto` cell that moves after a tool or op edit.
Walker rule (a) stays off in `AdoptResult`; a no-op regenerate drops nothing.

**3.4 Surfaces.**
| Surface | Today | After |
|---|---|---|
| GUI panel | GUI-only dial | reads/writes the stored value by command; saved |
| GUI plan (R1) | `AtMost(required)` per scope | stored value; modal when `Fixed` is coarser than the project rest rule; Accept sends the command |
| MCP `run_simulation.resolution`, `generate_all.simulation_resolution_mm` | panel write / required | optional; sends the command; reply names the cell and the stale set |
| MCP `generate_toolpath` | GUI modal | no modal; a too-coarse `Fixed` refuses with the shared sentence |
| CLI `--resolution` | required with rest ops | optional in-memory override; output says `override` and the project value |
The user sees: the panel value saves with the project. A change turns rest
rows WAIT (amber rail) and the simulation stale. MCP `list_toolpaths` and CLI
`tp_*.json` carry `source_stock` (cell, digest, `after`); `project_summary`
and `summary.json` carry `simulation_resolution {mode, mm, source}`.

## 4. The gen_sim_rest_ux rulings

- **R1** holds in intent (the panel is the standing choice; auto takes the
  finer of auto and required; a coarser pinned value asks). Ruling (2) of
  2026-09-24 changes three details: the panel is project state; auto is
  project-wide (IMPL_W1 §3 said per request); "MCP keeps its argument" goes.
- **A/M10** and `rs_cam_cli/CLAUDE.md` ("refused, never defaulted") conflict
  with ruling (2). The stored value is a recorded choice, not a silent
  default. I update both texts and retire `REST_NEEDS_RESOLUTION` (Q4).
- **R2** holds: `after` covers every earlier setup, as the simulator does.
- **G-MCPMODAL** (owner "viz MCP") sits in `start_gui_plan`, which this
  package changes (Q5). R11/R12: no conflict.

## 5. Tests (1 mm cell, 20 x 20 x 5 stock, cheap ops, seconds each)

Core, new `rs_cam_core/tests/rest_stock_identity_g_restres.rs`:
1. The result records the stored cell; the plan walk needs no argument.
2. `SetSimulationResolution` drops the rest result and the simulation;
   `Effects.stale` names it.
3. Idempotency: re-simulate at the same cell over the same results, and
   regenerate a predecessor with equal output; nothing drops, and `plan()`
   emits zero Simulate steps. Repeat with a `Fixed` cell below the grid cap.
4. M-A: a predecessor adopts a new identity with no edit; the consumer drops,
   `plan()` emits Simulate, `start()` refuses the old snapshot.
5. M-B: disable the consumer, change the predecessor, re-enable: no result.
6. Chain: the middle op regenerates; the tail drops.
7. Project file round trip with and without `resolution_mm`.
Viz, new `rs_cam_viz/tests/rest_sim_reads_core_results_g_reststale.rs`:
8. M-C: edit the rough, Run Simulation; the snapshot has no rough carve.
9. Parity: the GUI plan (real worker, `compute/worker/test_fixture.rs`) and
   the core walk the CLI runs, on one saved TOML: equal moves, equal
   `stock_snapshot` digest, equal `source_stock` (precedent
   `export_parity_core_vs_gui_p0.rs`). In `rs_cam_cli`: `run_generation_plan`
   with no `--resolution` gives the same digest.
Sentries (`scripts/cargo_lane.sh`, one job at a time, no heavy gate): the
seven `session/CLAUDE.md` sentries, `adopt_simulation_stores_prior_stocks`,
`save_keeps_the_simulation_wp17`, `sim_prefix_memo_s5`, core `--lib session::`;
viz `generate_all_fixpoint_parity`, `mcp_wire_surface_pin`,
`command_registry_surfaces`, `command_surface_completeness`,
`egui_draw_sites_write_through_commands_wp6`,
`connector_reads_edge_state_g_connector`, `awaiting_prior_stock_has_one_shape_d4`,
viz lib; `rs_cam_cli -q`; `rs_cam_mcp -q`; workspace clippy; fmt.

## 6. Files and commits (blast radius from `rg`)

1. **core: stored resolution.** `session/{mod,project_file,save,builder,
   command}.rs`, `session/compute/simulation.rs`, `session/generation_plan.rs`
   (project-wide rest rule), `session/CLAUDE.md`. `SimulationOptions` keeps
   its shape (88 literals, 58 files); callers pass the stored value.
2. **core: source identity + sweep.** `compute/toolpath_stats.rs` (18
   `ToolpathStats {` hits, 10 files), `compute/stats.rs`, `trace/narrate.rs`
   (exhaustive pattern), `session/{compute,dependencies,generation_plan,
   command}.rs` (`AdoptSimulationArgs`: 11 literals, 9 files),
   `session/mutation/toolpath.rs`, `session/compute/diagnostics.rs`,
   `session/diagnostics_types.rs`, tests 1-7, a minimal viz compile fix.
3. **viz.** `controller/events/{simulation,toolpath,compute}.rs`,
   `controller/generate_all.rs` (`PlanResolution`, `require_resolution`: 11
   hits, 3 files), `state/simulation.rs`, `state/simulation/playback_state.rs`,
   `ui/sim_op_list.rs`, `app/simulation.rs`, `app/mcp/{simulation,diagnostics,
   project,generation}.rs`, `ui/mod.rs` (event origin), `mcp_server.rs`,
   tests 8-9.
4. **wire + CLI.** `rs_cam_mcp/src/server.rs`, `tests/mcp_wire_surface.json`,
   `rs_cam_cli/src/{project,main,rough_score}.rs`, `compute/config.rs`
   (`REST_NEEDS_RESOLUTION`: 11 hits, 4 files), `rs_cam_cli/CLAUDE.md`. The
   commit states the break: MCP param meaning and the CLI refusal change; the
   file format does not break.

No file on the two avoid lists is touched. Named, not fixed:
`tool_load/optimize/outcome.rs` sets `auto_resolution` for its isolated runs
(a parity gap for the power session); `smoke.rs` keeps its explicit cell.

## 7. Open questions and rulings

**Rulings, 2026-09-24.** The operator (asked directly): Q1 ONE project
value for every simulation, default `Auto` worked out project-wide; the
operator accepts stale-on-dial-change and Q7's slower, finer prefix
simulations. Q6 DROP a result whose source stock no longer matches (the row
shows WAIT). Q3 the MCP resolution parameters SET the project value, and
the reply says so. The coordinator, on my recommendations: Q2 store `Auto`
as a mode; Q4 retire the "resolution required" refusal and keep "coarser
than the rest needs"; Q5 fix G-MCPMODAL here (an MCP-started plan never
opens the confirm modal and never waits on a click); Q8 connector hover in
the GUI, full record on MCP and CLI.

The questions as asked:


- **Q1** One stored value for every simulation, or a rest-only value beside
  the panel? **One.** The parity ruling covers simulation metrics too. Note:
  this stales rest results on the DIAL change, which is stronger than the
  ruling's words ("a simulation at another resolution"). With one stored
  cell the two are the same event, so I recommend it.
- **Q2** Store `Auto` as a mode (key absent), or always write the number?
  **Mode.** A tool change then moves the cell and stales honestly. Every
  output prints the resolved number and its source.
- **Q3** MCP resolution params: change the project value, or a one-off run?
  **Change the value**, and say so. A one-off makes evidence that no project
  state reproduces.
- **Q4** Retire the MCP/CLI "resolution required" refusal (A/M10) and keep
  only "coarser than the rest needs" (GUI asks, MCP/CLI refuse)? **Yes.**
- **Q5** Fold G-MCPMODAL in (an MCP-origin plan never opens the modal)?
  **Yes.** `AppEvent::GenerateToolpath` (`ui/mod.rs:151`) then carries an
  origin from `app/mcp/generation.rs:822` to `start_gui_plan`.
- **Q6** Drop an out-of-date result (row reads WAIT), or flag it? **Drop.**
  Freshness, the rail and export read `get_result().is_some()`; a flag is a
  second truth.
- **Q7** Project-wide `Auto` makes each prefix simulation as fine as the
  finest rest tool (wanaka 0.10 mm: slower). **Accept**; pin `Fixed` for
  speed. The rivmap100 Phase 1 arms need `Fixed(0.5)` to reproduce.
- **Q8** Show the source stock in the GUI? **Rail hover only** ("After 3D
  Rough · 0.20 mm"); the full record on MCP and CLI.

## 8. Stage 2 result (2026-09-24)

What landed, against §3:

- One stored `SimulationResolution { Auto, Fixed }` on the session and in
  `[job.simulation] resolution_mm`; one setter row
  `SetSimulationResolution`; `Auto` project-wide (`session/rest_stock.rs`).
- `SourceStock` (`compute/source_stock.rs`): cell, carve kernel, and a
  geometry digest of each carved toolpath. The digest leaves out feeds,
  because the adaptive feed modulation rewrites feeds after each
  simulation. The simulator writes one record per snapshot
  (`SimulationResult::prior_stock_sources`); `start` copies it onto
  `ToolpathStats::source_stock`; the sweep in `try_with_effects` drops a
  rest result whose record no longer matches.
- The GUI builder carves core results (M-C). MCP `generate_toolpath` never
  opens the confirm modal (G-MCPMODAL). MCP `run_simulation`,
  `generate_all` and the new `set_simulation_resolution` tool set the
  stored value and say so. The CLI reads the stored value; `--resolution`
  is an override that `summary.json` names.

Deviations from the plan:

- §3.2 said revision; the code uses a geometry digest (review point 2).
- The comparison uses the REQUESTED cell on both sides, not the clamped
  one: the clamp is a pure function of the request and the stock.
- The parity test found a new gap: the cutting-metrics kernel and the plain
  kernel carve DIFFERENT stock (fixture snapshot digest `b491…` against
  `f458…`, every other input equal). The CLI and MCP always carve with
  metrics; the GUI followed its capture toggle. Fix: the record carries the
  kernel, only the metrics carve is current, and every GUI plan simulation
  carves with metrics. A plain GUI Run Simulation with capture off leaves
  rest snapshots that read Pending ("carved without cutting metrics").
- The core builder skipped no disabled row with a result; it now does, as
  the GUI builder does.
- One code commit, not four: the struct changes cross all four crates, and
  a split commit would not build.

Open for the operator:

- **Q9** The two carve kernels disagree. Recommendation: every GUI
  simulation carves with metrics and the capture toggle goes, or the two
  kernels are made to carve the same stock (a `dexel_stock/` change).
- `tool_load/optimize/outcome.rs` still sets `auto_resolution` for its
  isolated runs (power session).
