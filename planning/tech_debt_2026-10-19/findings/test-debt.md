# Test debt (`test-debt`) — findings, baseline 99b6fb41

Sweep: all `crates/*/tests`, the in-source `mod tests` files, `crates/rs_cam_core/Cargo.toml`
test targets, `.github/workflows/ci.yml`, the folder `CLAUDE.md` sentry lists, and the
instruments `test_only_pub_api.md` and `in_flux_paths.txt`. Facts come from `rg` and file
reads. No cargo run. Counts come from `rg` and small Python scans over the source text.

Summary of the population:

- `crates/rs_cam_core/tests`: 495 `.rs` files, each one a separate test binary. 41 of them
  hold only `#[ignore]` tests. `crates/rs_cam_viz/tests`: 112 files. `crates/rs_cam_cli/tests`: 8.
- `#[ignore]` attributes: 299. Only one has no reason (`adaptive_property_harness.rs:641`).
- Every sentry name in the 37 `CLAUDE.md` files resolves to a file or a symbol (checked by a
  script). No dead sentry name was found.
- Test files that read Rust source text: about 103 (core 43, viz 55, cli 5). The 2026-09-16
  plan counted 79 (`planning/tech_debt_2026-09-16/TECH_DEBT_PLAN.md:145`).

---

## TD-1 (B) — CI never runs: it triggers on `main`, and the trunk is `master`

- `.github/workflows/ci.yml:6-7` triggers `push` on `branches: [main]`. The repository has no
  `main` branch (`git branch -r` lists `origin/master` and no `origin/main`). The operator does
  not use pull requests. So no workflow job runs. This is the cause of the root `CLAUDE.md:102`
  note "GitHub CI has not run a job since at least 2026-04".
- The instruction files still say that CI runs the gated targets:
  `crates/rs_cam_core/tests/CLAUDE.md:25` ("CI runs them") and
  `crates/rs_cam_core/Cargo.toml:16` ("the commit gate and CI must pass `--features heavy-tests`").
- The result: 13 `heavy-tests` binaries ("load-bearing sentries, NOT instruments",
  `Cargo.toml:16`), 8 `research` binaries, and the `gcode_emulator_validation` gate
  (96 of 97 tests are `#[ignore]`, run only by `ci.yml:83` with `--ignored`) have no regular
  runner. The operator ruling of 2026-09-11 forbids the full gate on the PC. A cloud session can
  run it (root `CLAUDE.md:100-102`), but nothing schedules that.
- `ci.yml:24` also runs a workspace-wide `cargo test`, which root `CLAUDE.md:104` forbids
  ("it can loop").

Fix: set the trigger to `master`. Replace `ci.yml:24` with per-crate commands that match the
root `CLAUDE.md` quality-gate table. Add a small sentry that parses `ci.yml` and asserts the
trigger branch and the per-crate commands. Correct `tests/CLAUDE.md:25` and `Cargo.toml:16`.

Enforcement now: NONE. After the fix: SENTRY.

---

## TD-2 (B) — `#[ignore]` parks known-red and retired tests, and print-only tests run unignored

`crates/rs_cam_core/tests/CLAUDE.md:27` says "`#[ignore]` marks an instrument or an evidence
run, not a broken test". The verify skill (`.claude/skills/verify/SKILL.md:30`) repeats it. The
rule is broken in both directions.

Known-red or retired tests hidden behind `#[ignore]`:

- `tests/a_descent_over_two_overlapping_swaths_g_staletop.rs:846` — "red until the
  conservative_top coverage-union fix lands".
- `tests/adaptive3d_boundary_clear_parity.rs:744` — "aspirational: red until D4 boundary fix".
- `tests/drop_cutter_off_mesh.rs:212` — "known-failing: residual ~260 dive columns".
- `tests/wanaka_e2e_chipload_gate.rs:55` — "blocked on O3 helix-descent + O4 burn-risk edge
  filter".
- `tests/adaptive_property_harness.rs:618` — "retired claim". The doc comment (`:610-616`) says
  the test is kept as a record. It reads like a travel contract, and it guards nothing.
- `src/adaptive3d/tests.rs:2229,2239,2249,2259` — four "Probe A … Bug 2" probes.
- `tests/adaptive_property_harness.rs:641` — the only bare `#[ignore]` in the workspace.

When the fix for a parked red lands, no test turns green or red. Nothing tells the author to
un-park the test.

Print-only tests that run in the default suite (no assertion, not `#[ignore]`):

- `tests/arcfit_intent_key_cost_f1.rs:374` (`pr6_measure_arcfit_intent_key_cost`, 134 lines)
- `tests/contour_spiral_gcode_validity_phase0.rs:343` ("Diagnostic only (prints; never fails)")
- `tests/power_ceiling_parity_f2.rs:507` (`report_before_after_feeds_on_representative_fixtures`)
- `tests/tapered_ball_relief_profile_probe.rs:140,170` (`report_*`)
- `tests/_litmatrix_rubbing_floor_clamp.rs:273` (`record_the_cell`)
- `tests/checkpoint_c9_sampled_reach.rs:674` (`resolution_sweep_separates_model_error_from_grid_error`)

These cost time in every gate and can never fail.

Fix:

1. Give each parked red an inverted assertion ("the defect is still present"). The test then
   fails when the fix lands, and the author must flip it. Record each one in
   `TECH_DEBT_REGISTER.md`.
2. Move the retired claim to the register text and delete the test.
3. Mark the print-only tests `#[ignore = "instrument: …"]`.
4. Add a sentry that reads every `#[ignore = "…"]` reason and requires one of a fixed set of
   prefixes (`instrument:`, `evidence:`, `slow:`). It refuses `red`, `known-failing`, `blocked`
   and `retired`.

Enforcement now: DOC. After the fix: SENTRY.

---

## TD-3 (B) — named sentries pass vacuously: a skip path returns green

A gate test that returns early on a missing fixture or an empty result passes without a check.
The pattern is `eprintln!("skip …"); return;` in a test that is not `#[ignore]`.

- `tests/dexel_stock_z_frame_f026.rs:136-150`
  (`auto_from_model_load_grows_stock_xy_to_enclose_polygon_model`). The test returns when
  `!session.stock_config().auto_from_model`. Its fixture `test_data/ux_2d_pocket_softwood.toml:15`
  reads `auto_from_model = false`, and it has read that value since `ed71c4e0`
  (2026-05-25, the day the test was added in `163b4897`). The test has never checked the
  F-026 claim. It also returns on any load error (`:139-144`), so a load regression is green.
- `tests/capability_link_moves_safety.rs:829-836` — the pencil arm returns "SKIPPED — pencil
  produced no chains on hemisphere fixture". `compute/CLAUDE.md:35` and `dressup/CLAUDE.md:37`
  name this file as a sentry. From the source text, the skip looks like the usual path
  (PLAUSIBLE; not run).
- `tests/machine_kinematics_cycle_time_f034.rs:283-287` and
  `tests/feed_modulation_cycle_time_f036c.rs:161,197` are `heavy-tests` sentries. They return
  green when `tests/fixtures/wanaka100/wanaka_full_tuned.toml` is absent. The file is tracked
  today, so a rename or a move turns both sentries green in silence.

Fix: add a `common::require_fixture(path)` helper that panics in a gate test. Only
`#[ignore]` instruments may skip. Fix the f026 fixture, or point the test at a fixture with
`auto_from_model = true` (six `test_data/ux_step_*.toml` and `ux_3d_terrain.toml` have it). The
pencil arm needs a mesh that gives chains, and an assertion that `raw.moves` is not empty.

Enforcement now: NONE. After the fix: SENTRY (the helper, plus a scan for `return;` after a
skip message in non-ignored tests).

In flux: `dexel_stock_z_frame_f026.rs`.

---

## TD-4 (B) — completeness sentries check one direction only; `ids::ALL` misses 8 ids

A registry test reads the rows that exist and checks each one. It does not check that each
thing that must have a row has one.

- `crates/rs_cam_core/src/diagnostics/ids.rs:416` `ALL` lists 93 names. The file declares 100
  `pub const …: &str` ids. These 8 are not in `ALL`: `GEOM_CLIPPED_BAND` (`:248`),
  `GEOM_RAMP_REACH_CLAMP`, `GEOM_OFFSET_LIBRARY_FAILURE`, `GEOM_BOUNDARY_CLIP_DROPPED`,
  `GEOM_WATERLINE_LEVELS_BELOW_STOCK`, `GEOM_WATERLINE_LADDER_EMPTY` (`:309`),
  `CONFIG_DERIVED_STEPOVER`, `CONFIG_CLAIMS_REFERENCE`. The doc at `:413-415` calls `ALL` "a
  registry for downstream consumers". The only test over `ALL`
  (`diagnostics/tests.rs:41`) checks for duplicates. Six test files assert membership one id
  at a time (`depth_beyond_stock_core_g_depthstockcore.rs:257`,
  `long_tool_derate_is_shown_ld1.rs:147`, `one_depth_derate_for_feed_and_band_fm3.rs:167`,
  `rpm_follows_the_feed_ceiling_fm8.rs:213`, `the_dial_holds_the_load_and_never_cuts_the_feed_fm7.rs:253`,
  `every_consumer_reads_one_claimed_band_g1.rs:568`).
- `crates/rs_cam_viz/tests/the_help_key_is_the_param_name_ui04.rs:14-22` resolves each GUI
  `dv`/`dv_dressup` call against the registry. It does not check the reverse. The cold-start
  run (`coldstart/task3.json`) found that `segment_merge`, a `DressupConfig::FIELD_DEFS` row
  (`compute/config.rs:919`), has no GUI control. `rg segment_merge crates/rs_cam_viz/src` finds
  only `mcp_server.rs`. Every gate is green.

Fix: write the ids through one macro, `diagnostic_ids! { NAME = "x.y", … }`, that emits each
const and `ALL` together. Then a const outside `ALL` cannot exist. Delete the six per-id
membership asserts. For `FIELD_DEFS` and the schema, add the reverse arm: each row has a GUI
call site or a declared `Skip` reason.

Enforcement now: SENTRY (one direction). After the fix: TYPES for the ids, SENTRY for the
reverse GUI arm.

In flux: `diagnostics/ids.rs`.

---

## TD-5 (B) — gate tests load operator work files from `planning/` and from `/home/ricky`

`crates/rs_cam_core/tests/CLAUDE.md:13-14`: "A project or baseline a test loads lives HERE, not
in `planning/`, dated and hashed". `ARCHIVED_HARNESSES.md:9-10` archived two harnesses because
they read a "live, user-modified" planning project and an absolute machine-local path.

Tests that run in the default gate and break the rule:

- `crates/rs_cam_viz/src/controller/tests/undo.rs:1380-1382`
  (`the_mini_project_keeps_one_pin_drill_r1`) loads
  `planning/rivmap_block_job/testpiece_mini.toml`. Three commits edited that file on
  2026-10-16. Its sibling `testpiece.toml` has uncommitted changes in the git status now. An
  operator edit to the project can turn the `viz --lib` gate red, which every agent runs.
- `crates/rs_cam_core/tests/test_data_smoke_csv_alignment.rs:50` reads
  `planning/toolpath_acceptance/cases_agent_smoke.csv`.

Instruments with the same dependency (lower risk, because they are `#[ignore]`):
`tests/a_descent_over_two_overlapping_swaths_g_staletop.rs:53` (absolute
`/home/ricky/personal_repos/rs_cam/planning/…` path), `tests/rapid_replay_shipped_gcode_s1.rs:98-110`,
`tests/swept_wanaka_ab_s1.rs:120`, and 23 core test files that name `/home/ricky/Downloads/…`
(for example `tests/direction_field_wanaka_f1.rs:169`, `tests/reach_map_residual_p5_1.rs:674`,
`tests/waterline_respects_a_vertical_wall_r9.rs:329`).

The root `CLAUDE.md` ("Benchmark projects live in `planning/fixtures/`") and
`tests/CLAUDE.md:13` disagree about where a loaded project lives. The tier tests read
`planning/fixtures/rivmap100/*.toml` (`tier_band_overlap_g_overlapfill.rs:481`,
`tier_arcfit_burial_f4.rs:558`).

Fix: copy `testpiece_mini.toml` and the smoke CSV into `tests/fixtures/` with a date and a
hash. Use a `common::fixture(name)` door that resolves only under `tests/fixtures`. Add a sentry
that refuses `planning/` and absolute paths in tests that are not `#[ignore]`. Write one rule
for `planning/fixtures` versus `tests/fixtures`.

Enforcement now: DOC. After the fix: SENTRY.

In flux: `controller/tests/undo.rs`.

---

## TD-6 (E) — source-scan sentries guard rules that visibility now holds; the population grows

Some scans now check a rule that the compiler enforces:

- `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs` property 1 (no
  `session.<setter>(` in viz or cli). WP15b made every setter `pub(crate)`
  (`crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs:1-11`), so a viz or cli call
  does not compile. The scan's documented gap, "a receiver spelled otherwise is NOT seen"
  (`wp15a.rs:37-42`), does not matter now. The scan code remains (`:444-450`).
- `crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs` arm 2 (no file outside core
  names a setter) is the same check for the same reason.
- `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` scan 1 (no
  `src/ui/` file names a hatch). The hatches are crate-private since WP7
  (`hatches_are_crate_private_wp7.rs`).

The visibility checks themselves (arm 1 of wp7, wp12, wp15b) parse `pub(crate) fn` from text.
A `compile_fail` doctest per door checks the same fact through the compiler. The workspace has
5 `compile_fail` blocks (`measurement.rs`, `trace/transform_provenance.rs`).

The register already holds two parts of this class. T-2 (a string scan that a refactor evades,
`TECH_DEBT_REGISTER.md:61`) is still true in form. T-3 (core tests scan viz sources; a viz gate
does not run them) is still open. It now covers 6 files that read viz paths in code:
`air_cut_denominators_lh1.rs`, `hatches_are_crate_private_wp7.rs`,
`loose_executor_is_crate_private_wp12.rs`, `setters_are_crate_private_wp15b.rs`,
`stale_set_has_one_answer_wp28.rs`, `gen_inputs_one_assembly_n12.rs`. The register says ten.

Fix: delete the three redundant arms. Replace each "is crate-private" text check with one
`compile_fail` doctest for each door. Move the six cross-crate scans into one workspace test
crate that both the core and the viz gates run (T-3).

Enforcement now: SENTRY. After the fix: TYPES (visibility plus `compile_fail`).

Known: T-2, T-3. In flux: `stale_set_has_one_answer_wp28.rs`.

---

## TD-7 (E) — test-only public API outside the `test-support` gate

FLD-04/05 put 24 test doors in `maps/` and `surface/` behind `cfg(feature = "test-support")`
(`Cargo.toml` feature comment; `tests/the_test_doors_are_gated_fld0405.rs:1-40`). The sentry
holds a fixed list. Test-only `pub` items outside that list are still in the default API
(rg: one production word, the definition; callers in tests only):

- `tool_load/mod.rs:267` `chipload_envelope_for_toolpath` (5 test files; power-calcs owner).
- `polygon.rs:166` `Polygon2::with_holes_closed` (4 test files).
- `adaptive3d/mod.rs:536` `adaptive_3d_toolpath_annotated` (3 test files).
- `export/fingerprint.rs:856,1009` `render_mesh_composite`, `composite_panel_layout`.
- `surface/flow_accum.rs:203,385` `resolve_flats`, `d8_accumulation`. These are in `surface/`,
  the folder FLD-05 gated, but they are not on the list.
- `maps/tier_map.rs:164` `drop_call_count` is ungated while `reset_drop_call_count` (`:176`)
  is gated.
- `mesh.rs:795,842` `make_test_hemisphere`, `make_test_flat`: fixtures in the crate spine, used
  by 47 and 50 files (tests and `cfg(test)` modules in core and viz), outside any gate.
- `compute/sim_prefix.rs:792` `is_populated`, `metrology/monge.rs:548`, `metrology/costing.rs:146`,
  `compute/toolpath_stats.rs:898,1806,1820`, `feeds/feed_explanation.rs:198,326`, and the rest of
  `instruments/test_only_pub_api.md` (44 rows).

Fix: put each item behind `test-support` and add a `required-features` row for each harness
that binds one. Change the FLD sentry from a fixed list to a rule that reads the instrument
output. Ask the power-calcs owner about the `tool_load/` and `feeds/` rows.

Enforcement now: SENTRY for 24 items, NONE for the rest. After the fix: TYPES (a cfg gate;
product code that names the door does not compile).

Known: S25 (`TECH_DEBT_PLAN.md:34`; core wave done, viz held, power-calcs open). Since then,
new rows appeared (for example `adaptive_3d_toolpath_annotated`, `with_holes_closed`).

---

## TD-8 (D) — three fixture builders write the same stock-frame knowledge

- `crates/rs_cam_viz/src/compute/worker/test_fixture.rs:1-14` (`RequestSpec`): "The stock hangs
  BELOW `z = 0`".
- `crates/rs_cam_core/tests/common/op_fixtures.rs:12-14`: "a flat board below world Z0".
- `crates/rs_cam_core/tests/common/session.rs:213,229` `stock_over` / `stock_under`.
- 44 test files define a private `fn …() -> StockConfig` or `-> ToolConfig` helper beside
  `common/` (rg, outside `common/`). Seven define their own `ball_tool`.

The core `tests/common` builders cannot reach `rs_cam_viz`, so viz wrote its own. The
2D-stock Z frame (stock top at Z0) is a known trap. A change to that rule must find every copy
by hand.

Fix: one builder under core `test-support` (for example `session::test_fixtures`), used by
`tests/common` and the viz worker tests.

Enforcement now: CONVENTION. After the fix: TYPES (one builder, one rule).

In flux: `crates/rs_cam_viz/src/compute/worker/tests.rs` (the main user of `RequestSpec`).

---

## TD-9 (F) — 41 instrument-only binaries compile in every core test build; the count doc is stale

- 41 of 495 core test files hold only `#[ignore]` tests (for example `param_sweep.rs` with 54,
  `wanaka_axial_doc.rs`, `tier_trial_350.rs`, `p1_headless_ab_wanaka.rs`). Each is its own
  binary that links `rs_cam_core`. `cargo test -p rs_cam_core` and `clippy --all-targets`
  build all of them. Only 8 sit behind the `research` feature (`Cargo.toml:169-198`).
- `crates/rs_cam_core/tests/CLAUDE.md:3` says "About 330 flat files". There are 495.
- Test-only modules sit among production files in `src/compute/execute/`
  (`pinned_bottom_z_reaches_motion_g_bottompin.rs`, `project_curve_chaining.rs`,
  `unified_finish_ring_collapse_g_unifiedcrash.rs`, `unified_finish_semantic_regions.rs`,
  declared `#[cfg(test)]` in `execute.rs`), and at the viz controller root
  (`controller/holder_clearance_*_g_*.rs`, `results_parity_tests.rs`, `workflow_tests.rs`).

Fix: add an `instruments` feature with a `required-features` row for each all-ignored file, or
merge them into one `tests/instruments/main.rs` binary with one module per file. Add a sentry:
a file whose tests are all `#[ignore]` must carry the feature. Move the ticket-named test
modules into a `tests/` sub-folder. Correct the count in `tests/CLAUDE.md`.

Enforcement now: DOC. After the fix: SENTRY.

---

## Not filed

- Exact float asserts: three hits (`rubbing_floor_warns_and_never_lifts.rs:170`,
  `a_size_claim_states_its_rule_range_and_residual_g1.rs:189,205`). All compare against
  printed source constants, not run output. The golden suites use relative tolerances
  (`perf_golden_sim_metrics.rs:159`).
- Hard-coded `24` operation counts (`every_cell_declares_its_feeds_support_fm0.rs:191,199`,
  `compute/catalog/tests.rs:10`): these are tripwires that fail loudly, not silent misses.
- `ARCHIVED_HARNESSES.md`: both archived files are gone from the tree. Nothing archived still
  compiles.
- `query_cycle_time_one_answer.rs` keeps a frozen copy of the old viz decision. It is a
  behaviour-neutral migration pin, and viz now calls the core query
  (`ui/readiness.rs:519-541`).
