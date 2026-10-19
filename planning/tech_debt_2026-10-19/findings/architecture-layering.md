# Findings: architecture, layering and coupling (`architecture-layering`)

Baseline 99b6fb41, branch geo-select. Read-only sweep. Each claim cites
`path:line` from `rg` or a file read. Caller counts come from `rg`. The
folder cycle table comes from a scan of `crate::<folder>` paths in
production lines of `crates/rs_cam_core/src` (comment lines and
`#[cfg(test)]` tails are skipped).

Summary, most severe first:

| # | Title | Tier | Cost | In flux |
|---|---|---|---|---|
| AL-1 | Three surfaces build the project verdict; MCP says "OK" over holder collisions that the CLI calls "ERROR" | A | S | no |
| AL-2 | Two "apply Suggest" funnels: GUI/MCP Apply skips the drill defaults that CLI `--apply-suggest` writes | B | M | yes |
| AL-3 | Feed modulation writes `session.results` outside `apply(Command)`; the viz result twin keeps the old feeds | B | M | yes |
| AL-4 | The CLI job pipeline is a parallel flow: own cutter builder, hand-built phases, export with an empty load report | B | M | yes |
| AL-5 | Pre-generation refusals live in the viz UI folder; CLI never runs them and core has a different stepover rule | B | M | yes |
| AL-6 | MCP wire patch structs are hand-kept partial copies of core args (`padding` is unreachable from MCP) | B | M | yes |
| AL-7 | MCP replies are hand-built `json!` inside the viz controller | B | M | no |
| AL-8 | Surfaces hold copies of core domain rules (load-context loop, rest-source staleness, ProjectCurve projection) | D | M | partly |
| AL-9 | `gcode/mod.rs` owns session-level policy: load report, trace freshness, export orchestration | E | M | yes |
| AL-10 | Core layering is prose only: 42 folder-level import cycles in production code | E | L | partly |
| AL-11 | Core has no facade: 34 `pub mod`, 689 deep paths from viz/cli/mcp into core internals | E | L | yes |
| AL-12 | The toolpath IR file depends on planner types and hosts raster generators | E | S | no |
| AL-13 | viz `state/`, `render/`, `io/` and `controller/` import the `ui/` folder | E | M | partly |
| AL-14 | `compute::validate::apply_stale_default_fix` is a public session mutator outside `apply`, with test callers only | C | S | yes |
| AL-15 | (selection programme note) `selection` imports `session` and `compute`, and both import it back | E | S | yes |

---

## AL-1. Three surfaces build the project verdict; MCP says "OK" over holder collisions

- Tier A, cost S, enforcement now NONE, fix enforcement TYPES. Not in flux.
- Sites:
  - `crates/rs_cam_viz/src/controller/events/compute.rs:2491-2508` (MCP `get_diagnostics` verdict)
  - `crates/rs_cam_cli/src/project.rs:960-987` (CLI `summary` verdict)
  - `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:179-300` (GUI verdict line from the triage), `:341` (`HIGH_AIR_CUT_PCT = 40.0`)

**Claim.** The project verdict string has three producers. The MCP chain
is: failed checks, then rapid collisions, then air cut > 40 %, then "OK".
It has no holder-collision arm. The CLI chain has
`total_collision_count > 0 => "ERROR: {n} holder/shank collisions detected"`
(`project.rs:971-975`). A project with holder collisions and no rapid
collision therefore reads "ERROR" in the CLI and "OK" in MCP
`get_diagnostics`. The MCP reply publishes `collision_count` beside the
verdict (`compute.rs:2520`), so the verdict contradicts its own row.

**Evidence.** Read `compute.rs:2474-2508`: `rapid_collision_count` is
`checks.rapid_collisions.len()`, and no arm reads
`core_diagnostics.collision_count`. Read `project.rs:960-987`. The 40.0
band is written three times (`compute.rs:2503`, `project.rs:981`,
`sim_diagnostics.rs:341`); the comment at `compute.rs:2476-2479` states
the two copies must stay equal. The 2026-09-16 plan retired
`ProjectDiagnostics::verdict` (L11, `bbaa193e`) as a duplicated key, and
no core owner of the verdict replaced it.

**Fix.** Add `ProjectDiagnostics::verdict() -> ProjectVerdict` in core: an
enum with `Unknown { failed }`, `HolderCollisions(n)`, `RapidCollisions(n)`,
`HighAirCut(pct)`, `Ok`, and one named const for the air band. The CLI,
MCP and GUI render the enum. An exhaustive `match` in each renderer makes
a new arm a compile error on every surface.

---

## AL-2. Two "apply Suggest" funnels; GUI/MCP Apply skips the drill defaults

- Tier B, cost M, enforcement now CONVENTION, fix enforcement TYPES. In flux.
- Known: `session/command.rs:50-55` names `apply_feeds` as a hand-written holdout (§15 ruling 4); `session/mutation/toolpath.rs:931`.
- Sites:
  - `crates/rs_cam_viz/src/controller/events/mod.rs:687-797` (`apply_feeds_through_funnel`)
  - `crates/rs_cam_viz/src/app/mcp/generation.rs:861-900` (`mcp_apply_feeds` calls the controller funnel)
  - `crates/rs_cam_cli/src/project.rs:1140-1215` (`apply_suggested_feeds_to_session`)
  - `crates/rs_cam_core/src/session/mod.rs:1689-1721` (`cutter_op_profile`)
  - `crates/rs_cam_core/src/feeds/suggest.rs:904-936` (`suggest_for_operation` calls `apply_drill_defaults`)
  - `crates/rs_cam_core/src/feeds/suggest/apply.rs:97`, `:821-839` (`apply` -> `apply_feeds_subset`)

**Claim.** The GUI and MCP Apply path builds the recommendation with
`feeds_preview_for_operation` and writes it with `suggest::apply`
(`apply_feeds_subset`). Its `SuggestContext` carries `model_bbox` and
`dressups` and leaves `stock` at `None` (`events/mod.rs:769-775`). The CLI
`--apply-suggest` path uses `cutter_op_profile`, which passes
`stock: Some(..)` and calls `suggest_for_operation`. That function runs
`apply_feeds_subset` (through `apply_feeds_result_to_op`, `apply.rs:390-412`)
and then `apply_drill_defaults(.., input.context.stock)` (`suggest.rs:925-930`).
`apply_drill_defaults` rewrites `peck_depth` for Drill and AlignmentPinDrill
(`apply.rs:888-966`). The GUI/MCP funnel never calls it: `rg
"apply_drill_defaults\("` finds only `suggest.rs:925` and tests. So the same
project gets a different peck depth from CLI `--apply-suggest` than from
GUI or MCP Apply.

**Evidence.** The rg and reads above. The orchestration (resolve tool,
machine, material, spindle strategy; clone a draft; send
`ReplaceToolpathConfig`) lives in the viz controller, so the CLI cannot
call it and holds a second assembly.

**Fix.** Add a core `Command::ApplyFeeds { index, scope, explored }` row.
The arm builds the context with `cutter_op_profile` and runs one funnel
(`apply_feeds_subset` plus the drill defaults). Delete the viz and CLI
assemblies. The registry row then forces every surface to reach it.

---

## AL-3. Feed modulation writes `session.results` outside `apply(Command)`

- Tier B, cost M, enforcement now DOC, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_core/src/session/compute/simulation.rs:411-420` (`pub fn modulate_simulation_trace(&mut self, ..)`)
  - `crates/rs_cam_core/src/session/compute/simulation.rs:537-631` (writes `slot.op_data` and `slot.provenance`)
  - `crates/rs_cam_core/src/session/compute/simulation.rs:349-351` (`run_simulation_memoized` calls `store_simulation` directly)
  - `crates/rs_cam_viz/src/controller/events/compute.rs:885-888` (viz calls the modulation door), `:922-934` (then `AdoptSimulation`)
  - `crates/rs_cam_viz/src/controller/events/compute.rs:612` (the only write of `rt.result = Some(..)`)
  - `crates/rs_cam_viz/src/app/gpu_upload.rs:1040`, `crates/rs_cam_viz/src/render/toolpath_render.rs:502` (viewport feed colour reads `rt.result`)

**Claim.** The root `CLAUDE.md` says every product surface mutates
`ProjectSession` through `apply(Command)`. `modulate_simulation_trace` is a
public `&mut self` method that replaces the `Arc` toolpath in each result
slot and stamps `ResultProvenance::Modulated { simulation_run, .. }`. The
viz controller calls it before it sends `Command::AdoptSimulation`. The
pairing "modulate, then store this run" is a comment only
(`simulation.rs:537-540`, "every caller stores this run next"). The core
door `run_simulation_memoized` stores with `store_simulation` and does not
use the `AdoptSimulation` row at all, while `generate_toolpath`
(`session/compute.rs:2554-2584`) routes its adopt through `apply`.

The viz keeps a second result cache, `rt.result`. Only
`compute.rs:612` writes it, at generation time. After a modulated
simulation, `session.results` holds the modulated feeds and `rt.result`
holds the planned feeds. The viewport colours by `rt.result` feeds, and
core G-code export reads `session.results`. The viewport can therefore
show feeds that the export does not emit (PLAUSIBLE: read, not run).

**Evidence.** Reads above; `rg "rt\.result = |\.result = Some"` in
`rs_cam_viz/src` finds no write after modulation.

**Fix.** Move the modulation into the `Command::AdoptSimulation` arm, and
make `modulate_simulation_trace` `pub(crate)`. Route
`run_simulation_memoized` through the same row. Delete `rt.result` and
read `session.get_result(index)` in the viz (this also deletes the four
hand-written keep-in-step arms at `compute.rs:34-70`, `:600-641`).

---

## AL-4. The CLI job pipeline is a parallel flow

- Tier B, cost M, enforcement now DOC, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_cli/src/job.rs:460-494` (`export_gcode_phases_with_overlay_checked` with `ToolLoadReport { per_toolpath: vec![] }`)
  - `crates/rs_cam_cli/src/job.rs:854-885` (`build_tool`), `:1008` (phase cutter), `:1053-1080` (`tool_config_from_def`)
  - `crates/rs_cam_core/src/compute/cutter.rs:8-40` (`build_cutter`)
  - `crates/rs_cam_core/src/gcode/mod.rs:376-411` (`export_gcode_checked`: load report, export datum)
  - `crates/rs_cam_cli/CLAUDE.md` ("Generated G-code must use the core export path")

**Claim.** `rs_cam_cli/CLAUDE.md` says: route export through the core
doors the GUI uses, and use the core export path. The `job` subcommand
generates each operation through a session, then builds its own
`OpResult` phases and calls the phase-level export door with an explicitly
empty `ToolLoadReport`. `enforce_load_policy` then has nothing to refuse,
so the chipload, power and deflection gates never run on a job export.
The session door `export_gcode_checked` runs `project_load_report` and the
export datum shift (`gcode/mod.rs:386`, `:408`).

The job path also builds the phase cutter with its own `build_tool`, not
`compute::cutter::build_cutter`. `build_tool` does not set `helix_deg` or
`corner_radius_mm`, which `build_cutter` sets (`cutter.rs:11-13`). The
tool that the session generates with (`tool_config_from_def`) and the tool
the phases carry are two builds of one definition.

**Evidence.** Reads above. The comment at `job.rs:460-463` states the
bypass ("no ProjectSession / load evaluation context").

**Fix.** Build one session for the whole job and export through
`export_gcode_checked` / `export_gcode_multi_setup_checked`. Delete
`build_tool`. Make the phase-level `export_gcode_phases_with_overlay_checked`
`pub(crate)`, so a surface cannot pass an empty report.

---

## AL-5. Pre-generation refusals live in the viz UI folder

- Tier B, cost M, enforcement now CONVENTION, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_viz/src/ui/properties/operations/validate.rs:13-129` (`ToolpathValidationContext`, `validate_toolpath_config`)
  - `crates/rs_cam_viz/src/ui/properties/operations/registry.rs:421-434` (`val_stepover_under_diameter`)
  - `crates/rs_cam_viz/src/controller/events/compute.rs:231-233` (the controller, and so MCP, calls the UI validator)
  - `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:245-275` (core stepover rule)

**Claim.** The Generate refusal ("Selected model has no 3D mesh",
"Stepover must be less than tool diameter", hole refusals) is computed in
`ui::properties`. The controller imports it, so MCP generation gets it
through the controller. The CLI generates through
`ProjectSession::generate_toolpath` and never runs it. Core has its own
stepover rule with a different comparison and scope: viz refuses
`stepover >= diameter` for Pocket and Adaptive only; core reports a
Critical diagnostic for `stepover > diameter` on every operation with a
stepover. At `stepover == diameter` the GUI blocks and core says nothing.

**Evidence.** Reads above. `rg "validate_toolpath_config\("` finds two
callers, both in viz (`compute.rs:233`, `operations/mod.rs:701`).

**Fix.** Move the refusal into core as a `ProjectSession::generation_refusals(index)`
read, or into the `start` precondition, with one stepover rule. The GUI,
MCP and CLI read that one answer. The viz validator keeps only widget
hints.

---

## AL-6. MCP wire patch structs are hand-kept partial copies of core args

- Tier B, cost M, enforcement now NONE, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_mcp/src/server.rs:758-791` (`SetStockConfigParam`)
  - `crates/rs_cam_viz/src/app/mcp/commands.rs:1107-1207` (`core_set_stock_config` copies fields one by one)
  - `crates/rs_cam_core/src/compute/stock_config.rs:69-80` (`StockConfig`)
  - `crates/rs_cam_core/src/session/command.rs:383-388` (row says `mcp: Reach::Reached`)

**Claim.** `StockConfig` has `padding` and `pin_part_models`.
`SetStockConfigParam` has neither (`x, y, z, origin_x/y/z, material,
auto_from_model` only), so MCP cannot set `padding`. The registry row says
MCP reaches `SetStockConfig`, because the census counts rows, not fields.
Each MCP mutation has three shapes: the `rs_cam_mcp` param struct, the
viz field-by-field copy, and the core `*Args`. Nothing ties them.

**Evidence.** Reads above. `rg padding rs_cam_mcp/src/server.rs` finds
only the rest-analysis grid padding (`:1223`, `:1297`). Cold-start task 5
found the same gap.

**Fix.** Define the patch type once in core (a `StockConfigPatch` with
`Option` fields, made by a derive or macro from `StockConfig`, with an
`apply_to(&mut StockConfig)`). The MCP param wraps it. A new
`StockConfig` field then fails to compile until the patch has it.

---

## AL-7. MCP replies are hand-built `json!` inside the viz controller

- Tier B, cost M, enforcement now SENTRY (partial), fix enforcement TYPES. Not in flux.
- Sites:
  - `crates/rs_cam_viz/src/controller/events/compute.rs:2345` (`build_mcp_diagnostics`), `:2202-2600` (`notify_mcp_*`)
  - `crates/rs_cam_viz/src/app/mcp/project.rs:480-490` (`inspect_stock`), `:866` (`pin_report_json`)
  - `crates/rs_cam_viz/tests/results_parity_tests.rs:1-25` (documents the past break)

**Claim.** The controller holds MCP wire code: 25 `json!` sites and 24
`#[cfg(feature = "mcp")]` gates in `controller/events/compute.rs` (also 8
in `controller.rs`, 21 in `app.rs`). A field added to a core report does
not reach the reply, because the reply names keys by hand. The parity
test header records that ten published channels once never reached the
agent this way. AL-1 is one result of this shape.

**Evidence.** `rg -c 'cfg\(feature = "mcp"\)'` and `rg -c "json!"` on
the files above.

**Fix.** Serialize the core report types (`ProjectDiagnostics`,
`ToolpathDiagnostic`, the pin report) with `serde` and send them. Move the
reply code from `controller/` to `app/mcp/`. The compiler then carries a
new field to the wire.

---

## AL-8. Surfaces hold copies of core domain rules

- Tier D, cost M, enforcement now DOC, fix enforcement TYPES. Partly in flux.
- Sites:
  - `crates/rs_cam_viz/src/state/simulation.rs:275-360` (`build_cut_metric_set`: "core has no public door that builds this context for one toolpath, so this function holds a copy of that loop body")
  - `crates/rs_cam_viz/src/app/mcp/commands.rs:2933-2974` (`core_stale_rest_source`), called at `:2775` (in flux)
  - `crates/rs_cam_core/src/session/mutation/config.rs:249-272` (core `auto_enable_rest_analysis_for_source`, same `RestDepth` predicate)
  - `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:930-934` (third copy of the predicate)
  - `crates/rs_cam_viz/src/app/gpu_upload.rs:463-530` (ProjectCurve surface projection and Trace level in the GPU upload; in flux)

**Claim.** Three surface sites re-derive a core decision:

1. The viz cut-metric cards build a `ToolpathLoadContext` with a copy of
   the loop body of `gcode::project_load_report`. A new context field in
   core is not added to the card.
2. MCP `set_boundary_config` marks the rest source stale by its own rule.
   It copies the `Pencil + RestDepth` exclusion of the core hook and adds
   staleness that core `Effects` does not give ("slightly conservative",
   `commands.rs:2771-2774`). GUI and MCP then report different stale sets.
3. The GPU upload projects a ProjectCurve drawing onto the mesh with its
   own `z_at_xy` max/min by `ProjectCurveDirection`, and computes the
   Trace level from `cutting_levels`. This is one more "mesh Z at (x, y)"
   reader beside the two that D12 (2026-09-16 plan) merged.

**Fix.** Add a core door for each: `ProjectSession::load_context(index)`;
put the rest-source staleness in the `SetBoundaryConfig` `Effects`; a core
`project_curve_preview(&cfg, mesh)` that the generator and the preview
share. Delete the surface copies.

---

## AL-9. `gcode/mod.rs` owns session-level policy

- Tier E, cost M, enforcement now NONE, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_core/src/gcode/mod.rs:22` (`use crate::session::ProjectSession`), `:349`, `:376`, `:583`, `:608`, `:706`
  - `crates/rs_cam_core/src/export/gcode_validator.rs:721-725` (`validate_machine_safety(session: &ProjectSession, ..)`)
  - `crates/rs_cam_core/src/gcode/emitter.rs:129`, `crates/rs_cam_core/src/gcode/program_builder.rs:484` (call `export::gcode_validator::strip_comments`); `crates/rs_cam_core/src/export/gcode_validator.rs:39` (imports `gcode::PostFormat`)

**Claim.** The G-code folder is the last layer in `lib.rs:5-17`, below the
IR. `gcode/mod.rs` (3230 lines) imports `ProjectSession` and holds
`project_load_report`, `sim_trace_is_fresh`, `sim_trace_is_fresh_for` and
the session export orchestration. It names `tool_load` 54 times. The viz
reads the simulation freshness rule from `rs_cam_core::gcode`
(`state/simulation.rs:338`). `gcode` and `export` import each other.

**Evidence.** `rg "crate::(\w+)" gcode/mod.rs` counts: tool_load 54,
session 8, compute 8. Reads of the lines above.

**Fix.** Move `project_load_report` and the freshness rule to
`session/` (or `tool_load/`). Keep `gcode/` on `GcodePhase` and
`PostDefinition` only. Move `strip_comments` to `gcode/` and let the
validator import it.

---

## AL-10. Core layering is prose only: 42 folder-level import cycles

- Tier E, cost L, enforcement now DOC, fix enforcement SENTRY (now), TYPES (crate split, later). Partly in flux.
- Sites (one edge each; the strongest first):
  - `crates/rs_cam_core/src/lib.rs:3-25` (layer list; omits `session`, `selection`, `settings`, `budget`, `diagnostics`, `metrology`)
  - `crates/rs_cam_core/src/compute/validate.rs:44`, `compute/simulate.rs:180,201`, `compute/obstacle.rs:326,447,477` (compute -> session)
  - `crates/rs_cam_core/src/tool/mod.rs:23,585,669-671,779,1368` (cutter layer -> compute, feeds, machine, stock); `feeds/mod.rs:75` defines `ToolGeometryHint`
  - `crates/rs_cam_core/src/trace/produced.rs:18,51`, `trace/transform_provenance.rs:503-507` (trace -> surface, adaptive3d, finish)
  - `crates/rs_cam_core/src/stock/sim_triage.rs:284,343,615,735,1008` (stock -> compute, finish)
  - `crates/rs_cam_core/src/dressup/feed_modulation.rs:64-69` with `tool_load/mod.rs:192,247` (dressup <-> tool_load)
  - `crates/rs_cam_core/src/feeds/mod.rs:1395-1396` with `tool_load/mod.rs:85` (feeds <-> tool_load)
  - `crates/rs_cam_core/src/io/mod.rs:25` with `session/mod.rs:411` (io <-> session)
  - `crates/rs_cam_core/src/surface/mod.rs:5` ("depends downward only", DOC)

**Claim.** `CLAUDE.md` names distinct layers, but nothing checks an
import against them. The production scan finds 42 two-way folder edges.
The largest: compute/session (8/317), compute/finish (128/25),
compute/ops (89/9), compute/trace (53/25), compute/feeds (18/53),
session/trace (46/1), dexel_stock/stock (38/2), feeds/tool_load (8/27).
`compute` is the hub: fan-in 16, fan-out 26 (instability 0.62). It is
depended on by most folders and depends on most folders.

**Evidence.** The scan script at
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/4701f13e-49b3-4e10-8e26-76541cdd081e/scratchpad/edges.py`
(run with the core `src` path); the area maps (`maps/core-*.md` section 3)
list the same edges.

**Fix.** Now: add a source-scan sentry with an allowed-edge table for
each folder and a ratchet on the current cycle count (fails on a new
cycle). Next: move the shared types out of the cycles
(`ToolGeometryHint` to `tool/`, the finding types from
`compute::toolpath_stats` to `finish/`, `SetupData`-free signatures in
`compute::simulate` and `obstacle`). Later: split crates along the layers
so the compiler refuses an upward import.

---

## AL-11. Core has no facade

- Tier E, cost L, enforcement now NONE, fix enforcement TYPES. In flux (`lib.rs`).
- Sites:
  - `crates/rs_cam_core/src/lib.rs` (34 `pub mod`, 1 `pub use`)
  - `crates/rs_cam_core/src/compute/mod.rs` (24 `pub mod`), `feeds/mod.rs` (21), `stock/mod.rs` (11)

**Claim.** Every core module is public, so every internal item is
workspace API. viz, cli and mcp name 689 paths three levels deep into
core (`feeds::suggest` 65, `stock::simulation_cut` 38, `compute::simulate`
35, `compute::catalog` 33, `compute::config` 31, `trace::debug_trace` 27).
A refactor inside a folder breaks a surface crate, and a surface can call
an internal step (AL-2, AL-3 and AL-4 each start with a surface calling an
inner door).

**Evidence.** `rg -o "rs_cam_core::[a-z_0-9]+::[a-z_0-9]+::[A-Za-z_0-9]+"`
over the three crates, tests excluded: 689 hits.

**Fix.** Make the folder modules `pub(crate)`. Give each layer a narrow
`pub use` facade (session doors, report types, wire types). The compiler
then rejects a surface call to an internal step.

---

## AL-12. The toolpath IR file depends on planner types

- Tier E, cost S, enforcement now NONE, fix enforcement TYPES. Not in flux.
- Sites:
  - `crates/rs_cam_core/src/toolpath.rs:7-8` (imports `geometry::region_set::RegionSet`, `surface::dropcutter::DropCutterGrid`)
  - `crates/rs_cam_core/src/toolpath.rs:613`, `:779` (`raster_toolpath_from_grid*`)
  - callers: `crates/rs_cam_core/src/compute/execute/finish_raster.rs:248,261`, `crates/rs_cam_core/src/finish/unified_finish/band_kernels.rs:352,422`

**Claim.** `CLAUDE.md` names the toolpath IR as the boundary between
planning and output. The IR file imports two planning types and holds two
raster generators. Every consumer of the IR (dressups, simulation, export)
therefore depends on the surface and geometry folders.

**Fix.** Move the two generators to `finish/` (or `surface/`). The IR file
then imports only `geo`.

---

## AL-13. viz `state/`, `render/`, `io/` and `controller/` import `ui/`

- Tier E, cost M, enforcement now NONE, fix enforcement SENTRY. Partly in flux.
- Sites:
  - `crates/rs_cam_viz/src/state/mod.rs:198,535` (`ui::preferences::PreferencesState`), `state/panels.rs:176,183`, `state/runtime.rs:300`, `state/app_settings.rs:79`, `state/pin_view.rs:82-84,101` (`ui::tokens` colours and `egui::Color32` in state)
  - `crates/rs_cam_viz/src/render/mod.rs:1071` (`ui::tokens`)
  - `crates/rs_cam_viz/src/io/setup_sheet.rs:57` (`ui::readiness`)
  - `crates/rs_cam_viz/src/controller/events/undo.rs:184,188`, `controller/events/model.rs:11,746`, `controller/events/mod.rs:487-489`, `controller/events/preferences.rs:26`
  - `crates/rs_cam_viz/src/ui/mod.rs:80` (`AppEvent` is defined in the UI folder)

**Claim.** In viz the state, render, io and controller folders import the
UI folder for state types, colours, command builders
(`stock_edit_command`, `keyed_pin_plan`) and the overlay registry. The
event vocabulary of the whole app is in `ui/mod.rs`. Logic that MCP and
the controller need is in the immediate-mode draw folder. AL-5 is the
case with a cross-surface effect.

**Fix.** Move the shared types, builders and colours to `state/` or a new
`model/` module, and `AppEvent` to `events/`. Add a viz source-scan
sentry that refuses `crate::ui::` in `state/`, `render/`, `io/`,
`controller/` and `compute/`.

---

## AL-14. `apply_stale_default_fix` is a public session mutator outside `apply`

- Tier C, cost S, enforcement now NONE, fix enforcement TYPES. In flux.
- Sites:
  - `crates/rs_cam_core/src/compute/validate.rs:164-175` (`pub fn apply_stale_default_fix(session: &mut ProjectSession, ..)` writes through `toolpath_configs_mut`)
  - `crates/rs_cam_core/src/session/mod.rs:1809` (`pub(crate) fn toolpath_configs_mut`)
  - `crates/rs_cam_core/src/diagnostics/fix.rs:30-33` (doc tells consumers to call it)

**Claim.** The function is public in a public module (`compute/mod.rs:30`)
and mutates the session with no `Command`, no revision and no `Effects`.
`rg "apply_stale_default_fix\("` finds only its two tests
(`validate.rs:631,772`). A doc comment points consumers to it, so it reads
as the fix door. It is also the reason a compute file needs
`toolpath_configs_mut`.

**Fix.** Delete it, or make it a `Command` row that the diagnostic fix
sends. Drop the doc pointer in `diagnostics/fix.rs`.

---

## AL-15. (Selection programme note) `selection` and `session`/`compute` import each other

- Tier E, cost S, enforcement now NONE, fix enforcement TYPES. In flux. Selection programme.
- Sites:
  - `crates/rs_cam_core/src/selection/resolve.rs:14,17` (`compute::transform::SetupTransformInfo`, `session::LoadedModel`)
  - `crates/rs_cam_core/src/selection/refusal.rs:11` (`compute::catalog::ItemUse`)
  - `crates/rs_cam_core/src/compute/config.rs:449`, `crates/rs_cam_core/src/compute/catalog.rs:987` (compute uses `selection::Selection`)

**Claim.** The new selection folder sits beside geometry in the layer
story but imports the session model record and compute types. Compute and
session import it back (scan: session/selection 30/1, compute/selection
11/3). This is a property of the unfinished geometry-selection work, so it
is a note for that programme, not debt.

**Fix.** Give `resolve` a narrow input (mesh, items, transform) and keep
`LoadedModel` out of `selection/`. Put `ItemUse` in `selection/`.

---

## Coverage

Swept: all crates under `crates/` (core folder edges by script; viz,
cli, mcp by `rg` and reads). Read the checklist section 9, section 3 of
all 10 area maps, the coldstart silent misses, the register and the
2026-09-16 plan. Not checked: `rs_cam_mcp_proxy` (depends on
`serde_json` only, `Cargo.toml`), `rs_cam_derive` beyond its generated
`crate::compute::catalog` paths, and the test trees. AL-3's viewport
divergence is read from code, not run.
