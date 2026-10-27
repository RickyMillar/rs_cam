# Design: remove the AgentSearch, ContourSpiral and Adaptive 3D clearing strategies

Date: 2026-10-28. Tree: `geodesic-integration` at `7ee3d3226`. Author:
implementer I1. Design only: no code until D64 reports and the lead
confirms.

Operator ruling (via the lead, 2026-10-28): the 3D Adaptive op keeps two
clearing strategies, Contour Parallel (CP) and Geodesic Adaptive. The
strategies AgentSearch, ContourSpiral and Adaptive (the variable-offset EDT
planner) go. D10 measures all five strategies first (D64).

Line numbers are of `7ee3d3226`. "Lines" counts are measured spans unless
marked "about", which is an estimate to be measured in the step.

## 1. What goes

### 1.1 The enum values

| Item | Place | Lines | Note |
|---|---|---|---|
| `ClearingStrategy::{Adaptive, AgentSearch, ContourSpiral}` | `compute/operation_configs.rs:136-155` (enum, 20 lines) | about 12 (three variants and their docs) | serde names `adaptive`, `agent_search`, `contour_spiral`. The load rule (§3) decides whether the values leave the type or stay as removed values |
| `ClearingStrategy3d::{AgentSearch, Adaptive, ContourSpiral}` | `adaptive3d/mod.rs:75-97` (enum) | about 25 (variants and docs; the enum doc at `mod.rs:60-74` describes them too) | the planner enum keeps `ContourParallel` (default) and `GeodesicAdaptive` |
| `From<ClearingStrategy> for ClearingStrategy3d` | `adaptive3d/mod.rs:111-123` | 3 arms | becomes a fallible map or a precondition (§3) |
| `effective_step_split`, `effective_keep_down_route` | `adaptive3d/mod.rs:333-360` | 6 arm lines | the shipped arms reduce to `ContourParallel` |

### 1.2 The planners and the dispatch

| Item | Place | Lines | Note |
|---|---|---|---|
| dispatch arm `Adaptive` | `adaptive3d/path.rs:527-540` | 14 | |
| dispatch arm `AgentSearch \| ContourSpiral` | `adaptive3d/path.rs:542-544` | 2 | the arm keeps `GeodesicAdaptive` only |
| `clear_z_level_adaptive` (the variable-offset EDT planner) | `adaptive3d/clearing.rs:1665-1842` | 178 | its only caller is the arm above |
| `edt_curvature_field`, `smooth_grid` | `geometry/grid_field.rs:137-222` | about 85 | their only caller is `clear_z_level_adaptive`, and no test calls them; delete with it |
| the non-geodesic branches of the slice | `adaptive3d/clearing.rs`, 10 `is_geodesic` sites (`:2473` and nine gates in `plan_one_outline`, `clear_one_region`, `plan_and_emit_region`, the 2D params at `:3860-3900`) | about 150 | after the removal every slice run is geodesic: each gate keeps its geodesic branch. This includes the agent/spiral plan call in `plan_one_outline` (`:2626-2641`), the shipped `slice_cleanup` call when no turns are planned stays (the geodesic plan with no turns uses it) |
| the Stage 4 planner-engagement plumbing of the 3D slice | `adaptive3d/path.rs:464, 552, 637, 1099, 1185, 1241, 1280`; `clearing.rs:2314, 2592-2605, 2637, 3359`, `:3801-3810` | about 40 | ContourSpiral (3D) was its only writer; `Produced::planner_engagement` stays (2D Adaptive writes it) |
| `path_strategy` choice in the slice's 2D params | `adaptive3d/clearing.rs:3876-3881` | 6 | becomes `PathStrategy2d::Agent` (the value the geodesic door sets anyway, `adaptive/geodesic/mod.rs:141`) |

### 1.3 Advisor, capability, Suggest

| Item | Place | Lines | Note |
|---|---|---|---|
| `is_constant_engagement`, `strategy_label` arms | `machine/strategy_advisor.rs:107-123` | 4 | |
| advisor unit tests on ContourSpiral | `machine/strategy_advisor.rs:310-430` (`:324, 347, 371, 400`) | about 30 to edit | they use ContourSpiral as the "spiral-class" candidate |
| `ADVISOR_CANDIDATE_STRATEGIES = [CP, ContourSpiral]` | `session/compute.rs:1415-1426` | 12 | DECISION: `[CP, GeodesicAdaptive]` after D64 measures it, or retire the advisor. The advisor needs a second candidate to compare |
| capability `planner_uniform_engagement` for 3D Adaptive | `compute/catalog.rs:1026-1034` | 9 | always false after the removal; the 2D arm (`:1054-1059`) stays |
| Suggest heuristic B (recommends `adaptive` on mixed terrain, warn-only) | `feeds/suggest/adaptive_entry.rs:30-90`, test `feeds/suggest/tests.rs:2500` | about 60 | its recommended value is removed. DECISION: delete heuristic B, or recommend `geodesic_adaptive` after D64 |

### 1.4 GUI, MCP, CLI

| Item | Place | Lines | Note |
|---|---|---|---|
| combo rows Adaptive, Agent Search, Contour Spiral and their labels | `rs_cam_viz/src/ui/properties/operations/surface_3d.rs:180-232` | about 30 | two rows stay |
| the ContourSpiral "Optimal load" slider | `surface_3d.rs:71-87` (branch), `:282-313` (`draw_spiral_load_control`) and its helpers if no other caller (`sane_tool_radius`, `FALLBACK_TOOL_RADIUS`) | about 50 | the stepover pill is the only stepover control |
| MCP | no wire enum (`rs_cam_mcp` has no strategy type). The values reach MCP through `get_operation_schema` (the `ParamEnum` string) and `set_toolpath_param`. The `recommend_clearing_strategy` tool text names "ContourParallel vs ContourSpiral" (`rs_cam_viz/src/mcp_server.rs:537`) | 1 description | `mcp_wire_surface_pin` pins tool names and argument shapes; a description edit may move the snapshot: report it, do not re-pin without the lead |
| CLI | job files read the value through serde; `run --list-params` prints the schema | 0 | follows the load rule |

### 1.5 Tests and goldens

Byte-parity goldens (`tests/fixtures/adaptive3d_emission/`): delete
`adaptive.txt` (593 lines), `agent_search.txt` (380), `contour_spiral.txt`
(997) and their three cases in `adaptive3d_emission_byte_parity.rs`.
`contour_parallel.txt` and `geodesic_adaptive.txt` stay byte-identical.

Op goldens (`tests/fixtures/op_golden_g_opgolden.toml`): the rows
`adaptive3d/adaptive` (`:48`), `adaptive3d/by_area_helix_spiral` (`:62`) and
`adaptive3d/ramp_agent` (`:90`) and their setters
(`every_op_keeps_its_toolpath_g_opgolden.rs:274-305`) go. They also carried
the only op-golden coverage of By Area, the helix entry and the ramp entry
on 3D Rough. Add new rows `adaptive3d/by_area_helix_cp` and
`adaptive3d/ramp_geodesic` with new pins (new rows, not a re-bless).
`adaptive3d/fresh` and `adaptive3d/rest` (CP) stay.

Integration tests that name a removed strategy (reference counts from `rg`):

| Test file | Subject | Action |
|---|---|---|
| `agent_search_coverage.rs` (459 lines) | O5b interior coverage of the slice | retarget to Geodesic Adaptive; coverage claims there may not hold (the arm leaves edges): re-pin or narrow to the body, by measurement |
| `agent_search_axial_doc.rs` (510, ignored) | diagnostic | delete (an instrument of the removed arm) |
| `adaptive3d_plans_every_slice_piece_g_adapt3dpieces.rs` | G-ADAPT3DPIECES for AgentSearch and ContourSpiral | retarget to Geodesic Adaptive (it plans each piece through the same slice) |
| `adaptive3d_entry_coalescing_f038.rs`, `adaptive3d_keep_down_link_f038b.rs` | F-038 filter and keep-down link on the slice | retarget to Geodesic Adaptive; F-038b's link rule differs on the arm (held links), so its claims need a reread |
| `adaptive3d_by_area_cells_confine_jobs.rs`, `adaptive3d_by_area_matches_global_stock.rs`, `adaptive3d_global_gate_drapes_below_floor.rs` | By Area and the Global gate, per strategy | drop the removed strategies from the strategy lists; add Geodesic Adaptive where missing |
| `adaptive3d_boundary_clear_parity.rs`, `adaptive3d_planner_never_ahead_of_emitted_path.rs` | planner/sim parity on the slice | retarget the AgentSearch arm to Geodesic Adaptive |
| `the_volumetric_wrapper_keeps_its_produced_w6_5.rs` | the 3D Rough side channel (planner engagement from ContourSpiral) | the 3D planner engagement goes; reduce to the fields that stay |
| `strategy_advisor_smoke.rs` | the advisor picks CP or ContourSpiral | follows the advisor DECISION |
| `param_sweep.rs` (ignored harness) | the strategy sweep | drop the removed values |
| `wanaka_z_layer_render.rs` (ignored diagnostic) | AgentSearch on wanaka | delete or retarget |
| `a_larger_rest_consumer_tool_refuses_d53.rs` | its source rough is AgentSearch | change the source to Geodesic Adaptive (the refusal does not depend on the strategy) |
| `adaptive3d/tests.rs` unit tests: `agent_search_z_drop_diag`, `traced_adaptive3d_emits_spans_hotspots_and_annotations`, `boundary_split`, `agent_search_cut_path_point_spacing_probe`, five `planner_sim_*_agent_search*` parity tests, `test_adaptive_3d_hemisphere_multi_level` | slice parity and traces | retarget the parity tests to Geodesic Adaptive; delete the two diagnostics |
| `operation_configs.rs:3810-3836` serde round trip, `every_op_states_its_capabilities_w2_4.rs`, `feeds/suggest/tests.rs:2500`, `session/compute/tests.rs:2150` | value lists | edit the lists |

Test fixtures (project and job files) that name a removed value:

| Fixture | Value | Loaded by |
|---|---|---|
| `tests/fixtures/wanaka_airrun_2026-06-01_2c908dca.toml` | `agent_search` ×2 | 3 tests (phantom-stamp rough6, `p1_headless_ab_wanaka`, `finish_planner_wanaka_decompose`) |
| `tests/fixtures/wanaka_2026-08-16_f530995a.toml` | `agent_search` ×2 | 7 tests (drill, suggest, tool hygiene, finish planner) |
| `tests/fixtures/wanaka100/wanaka_full_tuned.toml` | `contour_spiral` ×2 | 11 tests (kinematics, chipload gate, cycle time, Z layer, …) |
| `tests/fixtures/test_job.toml` | `agent_search` ×1 | CLI integration and 5 core tests |

Planning files: 40 project and job files under `planning/` name a removed
value (evidence of past runs; `planning/fixtures/rivmap100/` uses CP only).
They are not changed (evidence hygiene); they follow the load rule.

### 1.6 Docs

- `FEATURE_CATALOG.md`: the 3D Adaptive row (`:33`) and the strategy
  lines; record the removal and the load rule.
- `adaptive3d/CLAUDE.md` (lines 5, 16, 35: the AgentSearch slice, "keep
  the AgentSearch arm", `agent_search_coverage`); `adaptive/CLAUDE.md`
  (sentry list); the root and session maps if a sentry name moves.
- `planning/PROGRESS.md` and the lead's records: the lead's call.

## 2. What stays, and why

Geodesic Adaptive runs through the slice pipeline that AgentSearch and
ContourSpiral shared, and through the 2D adaptive engine. CP has its own
planner. These stay:

| Piece | Place | Used by |
|---|---|---|
| the slice pipeline: `clear_z_level_agent_2d_slice`, `detect_and_order_regions`, `clear_one_region`, `plan_and_emit_region`, `plan_one_outline`, `planner_outlines`, `LevelSink`, `coalesce_level_entries` | `adaptive3d/clearing.rs:2137-3981` | Geodesic Adaptive |
| the F-038 short-group filter and its forecast | `filter_short_groups` (`:2765`); the forecast in `clear_one_region` calls `adaptive::adaptive_segments_with_debug` with `PathStrategy2d::Agent` (`:3162`) | Geodesic Adaptive (C38 §16: the arm takes the forecast). So the 2D AGENT planner is still on the geodesic path; it must not be deleted as "agent" code |
| the keep-down proof and router, the step split | `adaptive3d/keep_down.rs`, `clearing.rs:902-1208` | Geodesic Adaptive, and CP for `link_footprint_ok` / `side_link_ok` |
| the waterline cleanup, the perimeter sweep (the fallback when no grown outline exists: "no smoothed or grown outline: shipped sweep") | `clearing.rs:1843`, `clear_one_region` | Geodesic Adaptive and CP |
| `clear_z_level_contour_parallel`, `build_material_bool_grid`, `level_gate`, `push_segment_with_stamp`, `z_blend` | `clearing.rs:1279-1664` | CP (`build_material_bool_grid` also the slice, `:2159`) |
| the 2D engine `adaptive/` (`path.rs`, `search.rs`, `spiral.rs`, `material_grid.rs`, `mod.rs`) | | the 2D Adaptive op (§4); Geodesic Adaptive through the cleanup, the replay, `LinkLoad`, `MaterialGrid`, `measure_step`, `PassLoad` and the forecast |
| `Produced::planner_engagement`, the session's planner-engagement floor | `session/compute.rs:1959-` | the 2D Adaptive op with `path_strategy = ContourSpiral` |
| `DebugSpanKind::ContourSpiral`, `contour_spiral` labels | `trace/debug_trace.rs:98-196` | the 2D spiral |

Renames (identifiers only, no behaviour change, byte parity):

| Now | Proposed | Place |
|---|---|---|
| `clear_z_level_agent_2d_slice` | `clear_z_level_slice` | `clearing.rs:3793`, `path.rs` (2), `tests.rs` (2) |
| `Agent2dRegion` (internal name) | keep the type name or rename to `SliceRegion` | `trace/debug_trace.rs:110` |
| `agent_walk_cut_length_mm` (field) | keep | `trace/narrate.rs`, `semantic_trace.rs`, `compute/annotate.rs` |

The WIRE labels `agent2d_region` (debug trace span) and
`agent_walk_cut_length_mm` (semantic trace key) are read by MCP
(`get_generation_debug_trace`, `narrate_toolpath`). A rename there is a
wire change. Proposal: keep both labels in this programme; their docs say
"the 2D planner of the slice", not AgentSearch.

Two config fields become inert on 3D Rough after the removal:
`Adaptive3dConfig::trochoid_cap_mult` (`operation_configs.rs:1691`, read
only by the ContourSpiral 2D generator) and `engagement_measure` (`:1697`;
the slice runs under `KeepDownLinks::WithinPassLoad`, which reads the
swept width). Every saved 3D Rough op writes both keys, so retiring them
(`RETIRED_KEYS`) would refuse every saved project. Proposal: keep them as
load-only fields with a "Not read by 3D Rough" help, as the 2D op's
`engagement_measure` already does (`operation_configs.rs:1078-1089`). The
step proves "inert" by a test: two values, one golden.

## 3. Project files that name a removed strategy

### 3.1 What the loader does today

`ProjectFile::from_toml_str` (`session/project_file.rs:415`) deserialises
with serde. `ClearingStrategy` is a plain `snake_case` unit enum, so an
unknown value fails the whole file with serde's "unknown variant", which
names no fix. The job-file reader (CLI) and MCP `set_toolpath_param` (it
merges the value into the config JSON and hands it to serde,
`schema_enum_values_g_schemaenum` doc) refuse the same way.

The codebase has one precedent for a unit value that leaves the format:
`RETIRED_VALUES` (`project_file.rs:217`, decision D8, P2.5
`face_selection`). The loader then refuses the WHOLE file before serde,
names every retired value and its replacement (`SessionError::RetiredKeys`,
C2), and converts nothing (rulings 2026-09-16 "no legacy" and 2026-10-15).

### 3.2 Option A (the lead's recommendation): the file loads, the op refuses at generate

- `ClearingStrategy` keeps the three values as REMOVED values: a variant
  `Removed(RemovedClearingStrategy)` with `#[serde(untagged)]`, where
  `RemovedClearingStrategy` is the unit enum `{Adaptive, AgentSearch,
  ContourSpiral}` with the old serde names. Serde reads and writes the old
  string unchanged, so a file round-trips byte for byte.
- `rs_cam_derive::ParamEnum` must leave the removed values out of the
  schema string. Today it skips only serde-`skip` variants
  (`rs_cam_derive/src/lib.rs:507-513`), and `skip_deserializing` would stop
  the load. A new variant attribute (for example `#[param(removed)]`)
  hides a variant from `enum:a|b|…`. `schema_enum_values_g_schemaenum`
  then holds: every advertised value deserialises.
- Generation refuses: a precondition in `session/compute.rs` (beside the
  D53 refusal) returns `SessionError::OperationFailed("The clearing
  strategy '<name>' was removed. Choose Geodesic Adaptive or Contour
  Parallel.")`. The diagnostics precondition ribbon shows the same text
  before generation (as `precondition.buried_face`). No conversion: the op
  never cuts with another strategy without the operator's choice.
- The planner enum has no removed value, so the map
  `ClearingStrategy → ClearingStrategy3d` becomes a function that returns
  `None` for a removed value; the precondition makes the `None` arm
  unreachable at generation.
- The doors refuse a NEW removed value: MCP `set_toolpath_param`, the
  CLI job reader for a new op, and the GUI (the combo offers only the two
  values; a removed value shows as a read-only "Agent Search (removed)"
  label). W6b (`a_value_outside_its_domain_is_refused_at_every_door_w6b`)
  is the place for the door test. The job file: a job op that names a
  removed value refuses at generate, like a project op.
- Cost: about 120 lines (derive attribute, enum, precondition, GUI label,
  door check) and one sentry.

### 3.3 Option B (the codebase precedent): refuse the file at load

- Three `RETIRED_VALUES` rows (`setups[].toolpaths[].operation.params`,
  key `clearing_strategy`, values `adaptive`, `agent_search`,
  `contour_spiral`), replacement text "Removed: choose geodesic_adaptive
  or contour_parallel". The enum values leave the type.
- The job-file path needs the same check (it does not run
  `check_retired_keys` today; to verify in the step).
- Cost: about 25 lines and one sentry. It matches the 2026-09-16 ruling.
- Effect: a project with one removed op does not open until the operator
  edits the file by hand.

### 3.4 Recommendation

Both options refuse and convert nothing, so no toolpath changes silently.
Option A lets the operator open the project and change the one op in the
GUI; option B is smaller and follows the standing rule. I recommend
option A for this removal, because the strategy is the main dial of the
most used roughing op and many operator projects name it. Option A is an
exception to the "refuse at load" precedent, so the operator must confirm
it.

## 4. The 2D Adaptive op

The 2D Adaptive op (`OperationType::Adaptive`, `AdaptiveConfig`,
`compute/operation_configs.rs:1078-1090`) is a different op. Its dial is
`path_strategy: PathStrategy2d {Agent, ContourSpiral}`, not
`ClearingStrategy`. The operator's "Adaptive" is the 3D strategy
`ClearingStrategy3d::Adaptive` (the variable-offset EDT planner). The 2D
Adaptive op stays, with both of its path strategies.

Shared code between the 2D op and what stays on 3D Rough: the whole
`adaptive/` engine (§2). After the removal, `adaptive/spiral.rs` and the
`PathStrategy2d::ContourSpiral` branches serve the 2D op only;
`adaptive_segments_with_debug` serves the 2D op and the geodesic forecast.
Nothing in `adaptive/` becomes dead.

## 5. Steps

Each step is one reviewable patch, frozen with its md5, and stops. Every
step keeps `contour_parallel.txt` and `geodesic_adaptive.txt` byte-
identical and the CP and Geodesic op-golden rows unchanged.

| Step | Patch | Gates |
|---|---|---|
| R0 | D64: D10 measures all five strategies. No code | the lead's confirmation |
| R1 | Fixtures: move the four test fixtures (§1.5) off the removed values. Each test that reads a rough op's numbers is re-measured and re-pinned with its cause stated; tests that read only other ops must not move. New op-golden rows `by_area_helix_cp`, `ramp_geodesic` | every loader of the four fixtures, opgolden, byte parity |
| R2 | The load rule (option A or B) and its sentry, with the removed values still planned: the refusal is the only new behaviour | the new sentry, `schema_enum_values_g_schemaenum`, W6b, the project round trip, viz `-j 2` (surface_3d, round trip), `mcp_wire_surface_pin` |
| R3 | Remove 3D Adaptive: the dispatch arm, `clear_z_level_adaptive`, `edt_curvature_field`, `smooth_grid`, its golden case, its opgolden row, its entries in the strategy-list tests | byte parity (4 cases), adaptive3d sentries, clippy (dead code) |
| R4 | Remove AgentSearch and ContourSpiral from the planner enum, the dispatch, the GUI rows and the spiral slider, the capability, the advisor arms; retarget or delete the tests of §1.5; delete their goldens | byte parity (2 cases), adaptive3d sentries, the retargeted tests, viz `-j 2` |
| R5 | Collapse the slice: every `is_geodesic` gate keeps its geodesic branch; the 3D planner-engagement plumbing goes; `path_strategy` is fixed. Geodesic output byte-identical | byte parity, the C12 judge (by name), the cap census, the s7_4 pins, D44/D48/D50 |
| R6 | Renames (§2), the inert-field help and its test, the advisor and Suggest DECISIONS, docs (FEATURE_CATALOG, folder maps) | fmt, clippy, folder maps, byte parity |

R1 goes first under either load rule: under option B the four fixtures do
not load, and under option A every test that generates their rough op
refuses.

## 6. Risks

1. D64 can show Geodesic Adaptive slower or worse than a removed strategy
   on some part. The removal then takes an option from the operator. R0
   gates every code step.
2. Geodesic Adaptive clears the body only. A job that roughed edges and
   channels with AgentSearch leaves them standing unless a rest op
   follows (7.8, the template, waits for the operator). A silent switch
   would hide this; both load options refuse instead.
3. Fixture migration (R1) moves the numbers of up to 24 tests that load
   the four wanaka fixtures. Each re-pin needs a stated cause; tests that
   do not read the rough op must not move, which is the check.
4. Coverage loss: By Area, the helix and ramp entries, planner/sim parity
   and boundary-clear parity were tested on AgentSearch and ContourSpiral.
   R1 and R4 must retarget them, not only delete them.
5. The 2D agent planner stays on the geodesic path (the F-038 forecast).
   A later "delete the agent" cleanup would break Geodesic Adaptive; the
   folder maps must say so.
6. Wire labels `agent2d_region` and `agent_walk_cut_length_mm` stay; a
   rename is an MCP wire change and needs its own ruling.
7. `trochoid_cap_mult` and `engagement_measure` become inert on 3D Rough;
   retiring them would refuse every saved project. They stay load-only.
8. The advisor has one candidate left. Without a decision it compares CP
   with itself.
9. Suggest heuristic B recommends a removed value.
10. Option A adds a derive attribute and a removed-value variant that
    later code must not offer at a door. The door tests (W6b,
    `schema_enum_values_g_schemaenum`) are the guard.
11. 40 planning projects name a removed value. They are evidence, not
    fixtures, and stay unchanged; they no longer reproduce their runs.
12. An older build reads a new project file without change (the values
    are a subset). A new build reads an old file by the load rule.
