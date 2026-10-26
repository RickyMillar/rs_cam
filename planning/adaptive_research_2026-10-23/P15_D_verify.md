# P1.5/P2 verifier D: merge, build, gates (2026-10-23)

Worktree `.claude/worktrees/adaptive-head`, branch `adaptive-instruments`.
Every cargo command ran through the worktree `scripts/cargo_lane.sh` with
`CARGO_TARGET_DIR=$PWD/target` and a `timeout`. Disk: 63 GB free at the
start, 53 GB free at the end.

## 1. Merge of master

- `git merge master` (master at 3df759fa, 4 commits past a23df1f8, S11
  74af0f8e included). The merge commit is **7d25a787** ("Merge branch
  'master' into adaptive-instruments", default message). It is the only
  commit that this pass made.
- Master moved after the merge: 7e1a7038 "planning: Phase 1.5/2 editors
  done, verifier running" (STATUS.md only). It is NOT merged. The lead owns
  that file.

### Conflicts

There was ONE textual conflict.

| File | Resolution |
|---|---|
| `crates/rs_cam_core/src/compute/CLAUDE.md` (the `Files` line) | Master's line kept (it adds `not_set.rs` (S11) and `fixture_values.rs`). Our G-ROUGHGOUGE clause (`Allowance`, `GougeProbe::limit_offset_mm`, `model_gouge_limit_offset_mm`, the sentry) appended after `GougeProbe::depth_bases` from `execute::drill_depth_bases`, where our side had it. The file is 40 lines. |

Git merged all the other expected files with no conflict:
`diagnostics/ids.rs`, `compute/catalog.rs`, `session/compute.rs`,
`session/compute/{diagnostics,export,simulation}.rs`, `session/CLAUDE.md`,
`tests/a_default_step_pocket_never_cuts_below_a_ring_ux7b.rs`, viz
`app/mcp/diagnostics.rs`, viz `ui/properties/tab_badges.rs`. The three
check builds below compiled them with no error, so no semantic conflict
remained in library code. One semantic conflict was in a test target (fix 3).

## 2. Fixes (all uncommitted in the working tree)

1. **rustfmt** on editor A files only (`cargo fmt --all`; no other file
   changed):
   - `crates/rs_cam_core/src/stock/sim_triage.rs` (~2917): line wrap.
   - `crates/rs_cam_core/src/stock/standing_islands.rs` (~682): an
     `assert!` wrapped to four lines.
   - `crates/rs_cam_core/tests/a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs`
     (168, 228, 241): line wraps.
2. **clippy `manual_chunks_exact`-family lint** (`as_chunks` help),
   `crates/rs_cam_core/src/stock/standing_islands.rs:607`.
   - Before:
     ```rust
     for pair in crossings.chunks_exact(2) {
         let [x0, x1] = [pair.first(), pair.get(1)].map(|x| x.copied().unwrap_or(0.0));
     ```
   - After:
     ```rust
     for &[x0, x1] in crossings.as_chunks::<2>().0 {
     ```
   - Why: the lint denies it. The repo already uses `as_chunks` (for
     example `export/fingerprint.rs:1066`). The behaviour is the same:
     `chunks_exact(2)` never gave a short chunk, so the `unwrap_or(0.0)`
     fallback was dead. `--lib standing_islands` is 4/4 after the fix.
3. **S11 semantic conflict** (E0308, `expected f64, found Option<f64>`),
   `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs:122-123`.
   - Before: `feed_rate: cfg.feed_rate, plunge_rate: cfg.plunge_rate,`
     (`cfg = AdaptiveConfig::default()`).
   - After: `let (feed_rate, plunge_rate) = fixture_feeds(OperationType::Adaptive);`
     with a one-line comment, and `use rs_cam_core::compute::fixture_values::fixture_feeds;`.
   - Why: S11 made the feeds `Option<f64>` with no default. Master's
     pattern for a test is `compute::fixture_values::fixture_feeds` (for
     example `disconnected_finish_retract_structure_p0.rs:177`). The
     harness requires `test-support`, so the door is available. The values
     are (1500, 500), the pre-S11 Adaptive values. The feeds do not change
     the planner geometry.

No other fix. No shipped code was changed.

After fix 2, these tests ran again and are green: the G-ROUGHCOV sentry
(4/4), `--lib standing_islands` (4/4), `--lib g_roughcov` (2/2),
`--lib audit_separates_standing_clean_and_gouged` (1/1) and
`--lib the_narration_sentence_states` (1/1).

## 3. Results

| # | Command (all `-p rs_cam_core -q` unless stated) | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green after fix 1 (rechecked at the end) |
| 2a | `check --features test-support` | green |
| 2b | `check --features research,test-support` | green |
| 2c | `check -p rs_cam_viz` | green |
| 3 | `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov` | green 4/4 |
| 3 | `--lib standing_islands` | green 4/4 |
| 3 | `--lib g_roughcov` | green 2/2 |
| 3 | `--lib audit_separates_standing_clean_and_gouged` | green 1/1 |
| 3 | `--lib the_narration_sentence_states` | green 1/1 |
| 3 | `--test a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` | green 4/4 |
| 3 | `--test every_op_states_its_capabilities_w2_4` | green 21/21 |
| 3 | `--features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b` | green 10/10 |
| 3 | `a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b` | not a target; it is a test in the ux7b file. Run by filter: green 1/1 |
| 3 | `--features research,test-support --lib research_geodesic` | green 3/3 (the pass-load test is green) |
| 3 | adaptive: `--lib adaptive` | green 141 passed, 7 ignored |
| 3 | adaptive: `adaptive_keeps_its_planner_order_g_adaptorder`, `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0`, `adaptive3d_emission_byte_parity`, `adaptive_plans_every_inset_piece_g_adaptpieces`, `a_link_feeds_only_within_the_pass_load_g_adaptlinkload` | green |
| 3 | adaptive: `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | **RED, master** (below) |
| 3 | adaptive: `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` | **RED, master** (below) |
| 3 | adaptive3d, `--features test-support`: all 14 (`adaptive3d_boundary_clear_parity` .. `adaptive3d_by_area_matches_global_stock`) | green |
| 3 | stock: `material_per_segment_s1`, `measurability_abstention_r8`, `narration_denominator_and_hints_d7`, `air_cut_one_time_base_g_airdenom`, `engagement_denominator_m3`, `..._g_overcutref`, `..._g_linkplough`, `..._g_rapid6497`, `..._g_rapidplungetol` | green |
| 3 | stock: `added_material_cuts_as_stock_s6` | **RED, master** (below) |
| 3 | `--features test-support --test union_coverage_m1` | 0 run, 1 ignored. Its only test is `#[ignore = "evidence: full wanaka200 chains, release mode, tens of minutes per arm"]`. Not run. The union refactor is covered by `audit_separates_standing_clean_and_gouged` (green) |
| 3 | `--test the_research_arms_are_feature_gated_fin01` | green |
| 3 | `--test the_test_doors_are_gated_fld0405` | green |
| 4 | `test -p rs_cam_viz -q --lib` | green 582 passed, 2 ignored |
| 4 | `test -p rs_cam_mcp -q` | green 36/36 |
| 5 | `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | green after fixes 2 and 3 |
| 5+ | `clippy -p rs_cam_core --all-targets -- -D warnings` (research off) | green |
| 5+ | `clippy -p rs_cam_core --no-default-features --lib -- -D warnings` | **RED, pre-existing**: 7 errors, all unconditional `rayon` use in shipped files (`dexel_stock/band.rs:38`, `band_batch.rs:49`, `simulation.rs:33`, `swept.rs:46`, `surface/pushcutter.rs:262`, `surface/slope.rs:186`, `band.rs:185`). None is in `standing_islands.rs`, so its non-`parallel` arm is NOT compiled by any gate |
| 7 | render harness (release) | green, 1/1, 1.27 s run |

### The three reds: all on master, not ours

1. `a_clearing_cut_holds_the_pass_load_g_adaptpassload::a_straight_plunge_entry_is_exempt_only_leaving_its_hole`
   ```
   panicked at crates/rs_cam_core/tests/a_clearing_cut_holds_the_pass_load_g_adaptpassload.rs:703:5:
   non-exempt ClearingCut samples over 0.5460 that the exact oracle also reads over (move, sim, exact): [(2810, 0.8329080445671909, 0.9403847504031052), (6159, 0.8329080445671909, 0.9403847504031052)]; per producer: {... "Residue mop": (2, 0.8329080445671909, 53.11648661865778), ...}
   ```
2. `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift::a_rapid_leaves_cut_depth_straight_up`
   ```
   panicked at crates/rs_cam_core/tests/a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift.rs:188:5:
   assertion `left == right` failed: sim-only rapid flags moved (move, exact clearance mm): [(6930, 0.24951132908296492)]
     left: 1
    right: 0
   ```
3. `added_material_cuts_as_stock_s6` (4 of 5 tests)
   ```
   panicked at crates/rs_cam_core/tests/added_material_cuts_as_stock_s6.rs:95:71:
   generate: Operation(NotSet(NotSet { op: "Pocket", kind: Feeds, fields: ["spindle_rpm"] }))
   ```

Proof:

- Reds 1 and 2: I built master 7e1a7038 in a temporary detached worktree
  with its own target dir and ran both binaries. Both fail there with the
  same moves and the same numbers (2810/6159 at 0.8329; 6930 at 0.2495).
  The temporary worktree and its target are deleted.
- Red 3: the refusal is S11's `NotSet` precondition (the fixture Pocket has
  no `spindle_rpm`). Our branch does not change this test or any refusal
  code. NOT run on master: this attribution comes from inspection only.
  The S11 commit message (74af0f8e) says that "141 core folder binaries"
  were green. `added_material_cuts_as_stock_s6` is a stock folder sentry.
  Thus the S11 gate did not run this binary, or the binary went red after
  S11. The lead must find which one is true.
- Our branch changes no 2D adaptive behaviour. `adaptive/path.rs` and
  `spiral.rs` change visibility only. `fast_sweep` changes `break` to
  `return true` with no code after the loop. `iso_contour` calls
  `iso_contour_where` with a filter that keeps every cell.

## 4. Magic numbers (not changed; the lead rules)

Non-test product code:

| File:line | Literal | Derivation |
|---|---|---|
| `adaptive/research_geodesic.rs:609` | `/ 6.0` in `(R / 6.0).max(tolerance)` | Doc: "The planner cell: `max(R / 6, tolerance)` (`path.rs`)". A COPY of `path.rs:699`, not a shared constant |
| `adaptive/research_geodesic.rs:771` | `cell * 3.0` | Module doc line 90: `path.rs`, `LinkLoad::new`: `step_len: grid.cell_size * 3.0`. A COPY, not a shared constant |

`standing_islands.rs` and `model_gouge.rs` have no float literal outside
the allowed set in product code. The other product hunks of the three
commits (`sim_triage.rs`, `session/compute/*.rs`, `catalog.rs`,
`capabilities.rs`, `registry.rs`, `adaptive3d/mod.rs`, `session/compute.rs`,
`scallop_isofield.rs`, viz) add no float literal.

Test and harness values:

| Place | Literal | Note |
|---|---|---|
| `research_geodesic.rs:1182` (unit test) and `tests/adaptive_geodesic_2d_renders_g1.rs` | `trochoid_cap_mult: 1.2` | Copied from `clearing_2d.rs` (2D Adaptive). UNDERIVED at its source; the STATUS trochoid-cap row owns it. Editor C states that the spiral does not read it under the 2D pass load |
| `research_geodesic.rs:1172-1174` (unit test) | `feed_rate 1000.0`, `plunge_rate 500.0`, `safe_z 5.0` | Fixture values |
| `research_geodesic.rs:1041..1162` (unit tests) | `R 3.0`, disc 20.0, island 6.0, pocket 60 x 40 | Fixture geometry |
| `standing_islands.rs:659..787` (unit tests) | 10.0, 20.0, 0.25, 2.5, 16/19 block, 3.0, 0.75, 0.2 | Fixture values |
| `sim_triage.rs` unit tests | 0.25, 0.75, 6.25, 30.0 | Fixture values |
| `..._g_roughcov.rs` | 40.0, 10.0, 0.25, 189.0, 9.0, 0.6, 5.0, 50/51, `x <= 20.1`, block corners | Fixture values |
| `..._g_roughgouge.rs` | `FLOOR_Z 10.0`, `TOOL_D 6.0`, ±50, ±30, +20, feeds 300/1000, `tol + 0.2` | Fixture values |
| render harness | `TOOL_DIAMETER_MM 6.0`, `CANVAS_PX 900.0`, `MARGIN_PX 24.0`, SVG stroke widths | Fixture and drawing values |

## 5. Render harness

`timeout 600 cargo test --release -p rs_cam_core --features research,test-support --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture`
(binary built first with `--no-run`). Result: 1 passed, run time 1.27 s.

`target/tmp/adaptive_geodesic_2d_g1/summary.txt`, verbatim:

```
star / agent
pieces: 1
cut length: 1379.2 mm
keep-down links: 261 (596.2 mm)
entries (plunges): 25
retracts (lifts to safe Z, last included): 25
pass load (planner readout): max 0.4515 vs ceiling 0.3626; 1 of 1943 steps over; 0 exempt plunge-departure steps

star / contour_spiral
pieces: 1
cut length: 2850.6 mm
keep-down links: 25 (141.2 mm)
entries (plunges): 101
retracts (lifts to safe Z, last included): 101
pass load (planner readout): max 0.3613 vs ceiling 0.3626; 0 of 4498 steps over; 0 exempt plunge-departure steps

star / geodesic_a
pieces: 1
cut length: 1010.9 mm
keep-down links: 66 (206.1 mm)
entries (plunges): 86
retracts (lifts to safe Z, last included): 86
pass load (planner readout): max 0.3614 vs ceiling 0.3626; 0 of 2559 steps over; 0 exempt plunge-departure steps
field piece 0: applied true, seed Some([49.5, 50.5]), seed radius 3.00 mm, levels 18, wrap pieces 103, converged true

pocket / agent
pieces: 1
cut length: 1471.0 mm
keep-down links: 374 (496.1 mm)
entries (plunges): 31
retracts (lifts to safe Z, last included): 31
pass load (planner readout): max 0.3618 vs ceiling 0.3626; 0 of 2024 steps over; 0 exempt plunge-departure steps

pocket / contour_spiral
pieces: 1
cut length: 2630.0 mm
keep-down links: 22 (137.3 mm)
entries (plunges): 91
retracts (lifts to safe Z, last included): 91
pass load (planner readout): max 0.3723 vs ceiling 0.3626; 1 of 4282 steps over; 0 exempt plunge-departure steps

pocket / geodesic_a
pieces: 1
cut length: 1237.8 mm
keep-down links: 133 (315.5 mm)
entries (plunges): 89
retracts (lifts to safe Z, last included): 89
pass load (planner readout): max 0.4478 vs ceiling 0.3626; 1 of 2891 steps over; 0 exempt plunge-departure steps
field piece 0: applied true, seed Some([18.5, 18.5]), seed radius 3.00 mm, levels 32, wrap pieces 88, converged true
```

Observation (not a ruling): three of the six columns read one step over the
ceiling: star/agent (0.4515), pocket/contour_spiral (0.3723) and
pocket/geodesic_a (0.4478). Two of the three are shipped producers. This
is consistent with editor C's note 5 (a `cut_only` walk drops stamped air
steps), but this pass did not locate the steps.

The harness cannot locate them. `PassLoadReadout`
(`research_geodesic.rs:733`) holds only `ceiling`, `max_reading`, `steps`,
`steps_over` and `exempt_steps`: no step position, no move index and no
producer (wrap, wall pass, mop). The `.txt` files hold the same fields. So
"candidate or shipped code" for `pocket/geodesic_a` stays OPEN. The unit
test `every_emitted_step_holds_the_pass_load` is green (0 over) on its own
60 x 40 mm pocket. The harness pocket is a different shape, and there the
candidate reads 0.4478 against the ceiling 0.3626. That is well over the
ceiling, not a rounding edge.

## 7. Top three for the lead

1. **Pass-load over-steps in the render harness.** `pocket/geodesic_a`
   reads 1 step at 0.4478 against 0.3626. Two shipped columns also read
   one step over: `star/agent` 0.4515 and `pocket/contour_spiral` 0.3723.
   The readout gives no place. Add the position and the producer to
   `PassLoadReadout` before you rule on candidate or shipped code.
2. **Three reds on master after S11.** `..._g_adaptpassload` and
   `..._g_adaptrapidlift` were proved on a clean master worktree.
   `added_material_cuts_as_stock_s6` (`NotSet spindle_rpm`) was attributed
   by inspection only. The S11 commit says its folder binaries were green.
   This contradiction needs an owner.
3. **The non-`parallel` arm of `StandingField::walk` has no compile gate.**
   `--no-default-features` fails on master in 7 places in shipped files
   (unconditional `rayon`). No gate compiles that arm of
   `standing_islands.rs`.

Also for the lead: the `/ 6.0` and `* 3.0` in `research_geodesic.rs`
(lines 609 and 771) are copies of `path.rs` values, not shared constants.

Files in `target/tmp/adaptive_geodesic_2d_g1/` (worktree):

- `star_agent.svg`, `star_contour_spiral.svg`, `star_geodesic_a.svg`
- `pocket_agent.svg`, `pocket_contour_spiral.svg`, `pocket_geodesic_a.svg`
- a `.txt` beside each SVG, and `summary.txt`

## 6. `git status --short` at the end

```
 M crates/rs_cam_core/src/stock/sim_triage.rs
 M crates/rs_cam_core/src/stock/standing_islands.rs
 M crates/rs_cam_core/tests/a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
?? planning/adaptive_research_2026-10-23/P15_D_verify.md
```

## Re-gate at 2b72f57b

HEAD 2b72f57b (the lead's second merge of master; it brings 5516aa8b, the
chamfer stop-short). The fixes above are in a2ceb3e1. Same lane script,
own target dir, a `timeout` on each command. Disk: 50 GB free at the end.

| Step | Command | Result |
|---|---|---|
| 1 | `check -p rs_cam_core --features research,test-support` | green |
| 1 | `check -p rs_cam_viz` | green |
| 2 | `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov` | green 4/4 |
| 2 | `--test a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` | green 4/4 |
| 2 | `--features research,test-support --lib research_geodesic` | green 3/3 |
| 2 | `--test every_op_states_its_capabilities_w2_4` | green 21/21 |
| 2 | `--features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b` | green 10/10 |
| 2 | the same, filter `a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b` (a test in the ux7b file, not a target) | green 1/1 |
| 2 | `--features step --test a_step_chamfer_cuts_convex_edges_only_rc` | green 7/7 |
| 2 | `--features step --test a_step_chamfer_stops_short_of_a_wall_chamferclear` | green 6/6 |
| 3 | `test -p rs_cam_viz -q --lib` | green 582 passed, 2 ignored |
| 3 | `test -p rs_cam_mcp -q` | green 36/36 |
| 4 | full workspace clippy (`heavy-tests,research,test-support`, `-D warnings`) | green |
| 5 | `fmt --all -- --check` | green (0 diffs) |

The merge broke nothing. No fix was made, and the working tree has no
change other than this section.

## D2 at 009579fc

Base 0e79f0fc (d0dd680f A2, c6d1abb3 B2, 0e79f0fc C2). Same lane script,
own target dir, a `timeout` on every command. Disk: 47 GB free at the
start, 46 GB at the end.

### Merge

`git merge master` (master 5591d974: 4240dfdd "Cut past open sides",
0e6788b2 the s6 fixture fix). Merge commit **009579fc** (default message).

| File | Conflict | Resolution |
|---|---|---|
| `crates/rs_cam_core/src/compute/CLAUDE.md` | textual (the `Files` line) | Master's text kept, with its new `open_sides.rs` entry. Our clauses (G-ROUGHGOUGE `Allowance` with `model_gouge_allowance`, probe R − t, limit CL Z + L − t, B2 Blocking only below CL Z − t, the sentry) inserted at the end of the `model_gouge.rs` entry, before master's closing parenthesis. 40 lines. |
| `crates/rs_cam_core/src/compute/catalog.rs` | none (git merged it) | the check builds compile it |
| `crates/rs_cam_core/src/session/CLAUDE.md` | none | — |

### Fixes (uncommitted)

1. **rustfmt**, A2 files only (all five are in d0dd680f):
   `session/compute/simulation.rs`, `stock/standing_islands.rs`, viz
   `controller/tests/mod.rs`,
   `controller/tests/the_gui_adopt_reads_the_rough_coverage_g_roughcov.rs`,
   viz `state/simulation.rs`.
2. **A2 viz test red (not the merge; the test was never compiled before).**
   `crates/rs_cam_viz/src/controller/tests/the_gui_adopt_reads_the_rough_coverage_g_roughcov.rs`,
   `rough_controller`. Both tests panicked at line 67:
   ```
   the toolpath sits at a live index: HeightOrder(HeightOrderRefusal { toolpath: "Scallop", violations: [BottomNotBelowTop { bottom_z: 0.0, top_z: 0.0 }] })
   ```
   - Cause: the sample stock top resolves to Z 0, and the flat sample
     model is at Z 0. For a 3D Rough the height order reads the top from
     the stock box (`top_on_stock_box`) and the Auto bottom from the model,
     so top = bottom = 0. The Scallop ladder does not refuse this. The 3D
     Rough ladder does.
   - Fix: after `config.operation = operation;` the test sets
     `config.heights.bottom_z = HeightMode::Manual(-FIXTURE_STOCK_Z_MM)`
     (the sample stock bottom, −25), with a comment. The pin is below the
     model, so the coverage target `max(model + leave, pinned bottom)`
     does not change. A first try that pinned `top_z` did not work
     (`top_on_stock_box` ignores it) and was replaced.
   - After the fix: `--lib g_roughcov` 2/2, viz `--lib` 585/0.

No product code was changed. Both planted defects (below) were reverted with
`git checkout` of `compute/model_gouge.rs`, which has no change.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | red (5 A2 files) → green after fix 1; green again at the end |
| 1 | `check -p rs_cam_core --features test-support` | green |
| 1 | `check -p rs_cam_core --features research,test-support` | green |
| 1 | `check -p rs_cam_viz` | green |
| 2 | viz `--lib g_roughcov` | red → green 2/2 after fix 2 |
| 2 | viz `--lib w2e` (memory sentry) | green 3/3 |
| 2 | viz `--lib g_rapidframe` (A2's list) | green 5/5 |
| 2 | viz `--lib` in full | green 585 passed, 2 ignored (after fix 2) |
| 2 | core `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov` | green 4/4 |
| 2 | core `--lib standing_islands`, `--lib g_roughcov` | green 4/4, 2/2 |
| 3 | `--test a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` | green 6/6 |
| 3 | `--test every_op_states_its_capabilities_w2_4` | green 21/21 |
| 3 | `--test findings_transport_join_h21` | green 3/3 |
| 3 | `--features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b` | green 10/10 |
| 3 | viz `--test a_blocked_result_does_not_read_ok_f20` | green 1/1 |
| 3 | viz `--test the_geometry_rows_fit_the_inspector_f22` | green 6/6 |
| 4 | `--features research,test-support --lib research_geodesic` | **RED** 4 passed, 1 failed (below) |
| 4 | `--lib adaptive` | green 141 passed, 7 ignored |
| 4 | `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | red, the SAME red as master (moves 2810 and 6159, sim 0.8329, exact 0.9404) |
| 4 | `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` | red, the SAME red as master (move 6930, clearance 0.2495) |
| 4 | `a_link_feeds_only_within_the_pass_load_g_adaptlinkload`, `adaptive_keeps_its_planner_order_g_adaptorder`, `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0`, `adaptive_plans_every_inset_piece_g_adaptpieces`, `the_research_arms_are_feature_gated_fin01` | green |
| 4 | `adaptive3d_emission_byte_parity` | green: `path.rs`'s new shared constants change no shipped number |
| 4 | the 14 adaptive3d sentries (`--features test-support`) | green |
| 5 | `test -p rs_cam_mcp -q` | green 36/36 |
| 5 | full workspace clippy (`heavy-tests,research,test-support`, `-D warnings`) | green |
| 6 | render harness, release, `timeout 900` | green 1/1, 1.95 s |

The red unit test, verbatim:

```
panicked at crates/rs_cam_core/src/adaptive/research_geodesic.rs:2494:9:
assertion `left == right` failed: every step holds the pass load: PassLoadReadout { ceiling: 0.36264459002853555, max_reading: 0.36687206264009165, steps: 2060, steps_over: 1, exempt_steps: 0, over: [OverStep { from: [48.4686454913005, 22.631632685874997], to: [48.53109906196661, 23.37902786348257], reading: 0.36687206264009165, producer: ResidueMop, link: false, segment: 258 }] }
  left: 1
```

The one over-step is a `ResidueMop` cut, which is the shipped mop. It is not
a candidate wrap or ramp. This is the case of C's note 5 (a `cut_only` walk
drops stamped air steps). The test was not loosened.

### Planted-defect checks (B2)

| Plant | Expected | Tripped |
|---|---|---|
| (i) the `Allowance` arm of `probe_radial_tolerance_mm` returns `MODEL_GOUGE_TOL_MM` | test 1 fails | test 1 `a_rough_at_its_leave_or_grazing_within_its_tolerance_reads_clean_g_roughgouge` FAILED at `:202:5`: `a graze within the tolerance: ModelGougeFinding { rule: Allowance { .. }, judged: 43, gouging: 42, deepest_mm: 9.899999999999999, at: [17.05, -30.0, 10.5], surface_z: 20.4, move_index: 1, depth_top_z: None, below_surface_mm: 9.399999999999999, at_wall: true }`. Also test 5 `the_pocket_rule_reads_no_allowance_g_roughgouge` FAILED at `:326:5`: `assertion failed: near(probe_radial_tolerance_mm(rule, a, R), a.tolerance_mm)`. 4 passed, 2 failed. |
| (ii) `blocks_export` returns `self.gouges()` | test 2 fails | test 2 `a_cut_into_the_leave_is_a_caution_g_roughgouge` FAILED at `:216:5`: `assertion left == right failed: ModelGougeFinding { .. deepest_mm: 0.040000000000000924, at: [-30.0, -20.0, 10.36], surface_z: 10.4, .. below_surface_mm: -0.4599999999999991, at_wall: false }  left: Some(Blocking)  right: Some(Caution)`. 5 passed, 1 failed. |

After both reverts the sentry is green 6/6 again.

### Magic numbers (three commits' diffs; not changed)

Product code:

| Place | Literal | Derivation |
|---|---|---|
| `adaptive/path.rs` `PLANNER_CELLS_PER_TOOL_RADIUS` | 6.0 | Now ONE named shipped value (doc: a shipped value, not derived here). Byte parity proves the arithmetic is unchanged. UNDERIVED at its origin |
| `adaptive/path.rs` `PASS_STEP_CELLS` | 3.0 | The same: one named shipped value, UNDERIVED at its origin |
| `research_geodesic.rs` `EntryLaw` (the C2 entry-ramp rule) | none | Every value comes from `R`, `s` and the ceiling: `s = stepover.min(2R)`, `ceiling_width = c·2R`, `h = √(R² − (R − s)²)`, `ρ = hypot(h, R)`, `φ₀ = atan2(R, h)`, `β(d) = φ₀ + asin((c·D − d − R)/ρ)`. The `2.0` is the diameter. No tuned value |
| `research_geodesic.rs:1344`, `:1392` | `.max(3.0)` | The fewest chords of a closed polygon (a triangle). The chord count otherwise comes from `acos(1 − tolerance/r)` (doc at `:1337`) |
| `research_geodesic.rs` | `1e-9` (`FieldSolver::SourceTracking` convergence, test bounds) | A float-equality epsilon. UNDERIVED, but not a physical value |
| B2 Blocking threshold | 0.02 mm | `MODEL_GOUGE_Z_TOL_MM` (`compute/model_gouge.rs:123`) = `selection::FLAT_REGION_Z_BAND_MM` (0.01, `selection/flat_regions.rs:36`) + `selection::CAD_ROUNDING_MM` (0.01, `selection/key.rs:38`, ruling L-13). Not a new literal |
| B2 probe bounds | none new | `MODEL_GOUGE_TOL_MM` = `CAD_ROUNDING_MM` (`model_gouge.rs:114`) |
| A2 product code | none | — |

Test and harness values (fixture or drawing values): A2 viz test
`stock_bbox` (±20, 10), `CELL_MM 0.25`; B2 sentry `BLOCK_Z 20.0`,
`WALL_X 20.0`, ±50, ±30, feeds 300/1000, `+ 0.1` past the tolerance,
`tol + 0.2`; `below_surface_mm` 5.0/10.0 in the h21, f20, f22 and
properties literals; C2 unit-test star and pocket vertices, `read_limit 33.0`,
`read_to >= 25.0` (bounds from the Python prototype); harness stroke widths,
dot and ring radii (4.0, 7.0), dash 6.0.

### Render harness

`target/tmp/adaptive_geodesic_2d_g1/` in the worktree (absolute:
`/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`).
It holds an `.svg`, a `.png` and a `.txt` for each of the 10 columns
(`{star,pocket}_{agent,contour_spiral,geodesic_g1,geodesic_a,geodesic_a_through}`)
and `summary.txt`.

| Shape / column | Entries (plunges) | Retracts | Keep-down links | Cut mm | Max / ceiling 0.3626 | Over-steps (producer, segment, from → to, reading) |
|---|---|---|---|---|---|---|
| star / agent | 25 | 25 | 261 (596.2 mm) | 1379.2 | 0.4515 | clearing cut, seg 85, (83.47, 40.11) → (83.35, 38.62), 0.4515 |
| star / contour_spiral | 101 | 101 | 25 (141.2 mm) | 2850.6 | 0.3613 | none |
| star / geodesic_g1 | 86 | 86 | 66 (206.1 mm) | 1010.9 | 0.3614 | none |
| star / geodesic_a | 87 | 87 | 125 (283.9 mm) | 993.2 | 0.4468 | mop, seg 188, (34.30, 53.26) → (33.83, 52.68), 0.3661; mop, seg 193, (19.22, 39.99) → (18.89, 40.66), 0.4468 |
| star / geodesic_a_through | 87 | 87 | 125 (283.9 mm) | 993.2 | 0.4468 | the same two as geodesic_a (front branches 1: no meeting line on the star) |
| pocket / agent | 31 | 31 | 374 (496.1 mm) | 1471.0 | 0.3618 | none |
| pocket / contour_spiral | 91 | 91 | 22 (137.3 mm) | 2630.0 | 0.3723 | mop, seg 205, (57.59, 33.55) → (57.82, 32.83), 0.3723 |
| pocket / geodesic_g1 | 89 | 89 | 133 (315.5 mm) | 1237.8 | 0.4478 | mop, seg 168, (54.44, 28.13) → (55.00, 28.64), 0.4478 |
| pocket / geodesic_a | 89 | 89 | 181 (437.9 mm) | 1077.6 | 0.4484 | mop, seg 353, (11.50, 11.40) → (11.34, 12.13), 0.4484 |
| pocket / geodesic_a_through | 92 | 92 | 150 (669.5 mm) | 1490.1 | 0.4743 | mop, seg 251, (42.76, 51.64) → (42.03, 51.77), 0.4511; mop, seg 260, (9.63, 42.16) → (9.04, 41.70), 0.4743; mop, seg 260, (8.48, 25.15) → (8.87, 24.51), 0.4595 |

Field lines: star geodesic_a: levels 18, wrap pieces 92, converged, front
branches 1, spiral levels 6. Pocket geodesic_a: levels 31, wrap pieces 158,
converged, front branches 3, spiral levels 3.

Readings:

- The ramp join did NOT cut the plunges. C2 predicted 1 to 6 entries for
  the clearing producer. The harness reads 87 (star) and 89 (pocket), the
  same as G1 (86, 89). The summary counts the entries of the whole
  pipeline (clearing, residue contours, wall pass, mop). Thus the cause can
  be in the candidate or in the later producers. The summary does not
  split the entries per producer.
- Every over-step of the candidate is in a `residue mop` cut (shipped
  code). The shipped contour_spiral pocket has one too. The only other
  over-step is a clearing cut of the shipped Agent on the star.

### `git status --short` at the end of D2

```
 M crates/rs_cam_core/src/session/compute/simulation.rs
 M crates/rs_cam_core/src/stock/standing_islands.rs
 M crates/rs_cam_viz/src/controller/tests/mod.rs
 M crates/rs_cam_viz/src/controller/tests/the_gui_adopt_reads_the_rough_coverage_g_roughcov.rs
 M crates/rs_cam_viz/src/state/simulation.rs
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **The plunge count did not fall.** Candidate (a) still enters 87 times
   on the star and 89 on the pocket, against C2's prediction of 1 to 6.
   Count the entries per producer before judging the ramp join.
2. **The residue mop steps over the pass load in 6 of the 10 columns.**
   These are five of the six candidate (a) columns (all except
   star/geodesic_g1) and the shipped pocket/contour_spiral. The
   `research_geodesic` pass-load unit test is red only on a `ResidueMop`
   step (0.3669 against 0.3626). This looks like a shipped-code finding
   (C's note 5), not a candidate defect. The lead decides.
3. **A2's GUI test needed a fixture fix.** The 3D Rough refused the sample
   project's heights (top = bottom = 0). The bottom pin at −25 is my
   choice. Check that it keeps the test's intent.

## D3 at c8db8b55

C3 on top (`research_geodesic.rs` and the harness only). Same lane script,
own target dir, a `timeout` on each command. Disk: 46 GB free.

### Fix (uncommitted)

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs:2304`, clippy
  `unnecessary_lazy_evaluations` ("unnecessary closure used with
  `bool::then`").
  - Before: `(report.spiral_levels > 0).then(|| { report.seed_radius + report.spiral_levels as f64 * params.stepover })`
  - After: `(report.spiral_levels > 0).then_some(report.seed_radius + report.spiral_levels as f64 * params.stepover)`
  - Why: the lint denies it. The expression is plain arithmetic with no
    side effect, so the eager form gives the same value. The harness ran
    before the fix, and its numbers stay valid.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green (also green after the fix) |
| 2 | `--features research,test-support -q --lib research_geodesic` | green: 4 passed, 1 ignored (`every_emitted_step_holds_the_pass_load`, ignored by bd287bd0, G-MOPLOAD) |
| 3 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | red (1 lint) → green after the fix; unit tests green again after it |
| 4 | the harness, release, `timeout 900` | green 1/1, 2.76 s |

### Harness table

Output: `/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`
(an `.svg`, `.png` and `.txt` per column, and `summary.txt`). Retracts
equal entries in every column. Entry split: S starter, J clearing at a wrap
end (join), M clearing mid-wrap (replay split), C residue contour, W wall
pass, P residue mop (other = 0 everywhere).

| Shape / column | Entries | S | J | M | C | W | P | Cut mm | Max / 0.3626 | Emitted over-steps | Raw plan over-steps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| star / agent | 25 | 1 | 0 | 17 | 0 | 0 | 7 | 1379.2 | 0.4515 | 1: clearing seg 85 (83.47, 40.11)→(83.35, 38.62) 0.4515 | 9 of 1749 |
| star / contour_spiral | 101 | 1 | 0 | 93 | 0 | 0 | 7 | 2850.6 | 0.3613 | 0 | 501 of 4873 |
| star / geodesic_g1 | 86 | 1 | 58 | 12 | 0 | 0 | 15 | 1010.9 | 0.3614 | 0 | 219 of 2422 |
| star / geodesic_a | 92 | 1 | 56 | 20 | 0 | 0 | 15 | 957.7 | 0.3600 | 0 | 206 of 2330 |
| star / geodesic_a_through | 92 | 1 | 56 | 20 | 0 | 0 | 15 | 957.7 | 0.3600 | 0 | 206 of 2330 |
| star / geodesic_a_ramp | 87 | 1 | 35 | 23 | 9 | 0 | 19 | 993.2 | 0.4468 | 2 mop: seg 188 (34.30, 53.26)→(33.83, 52.68) 0.3661; seg 193 (19.22, 39.99)→(18.89, 40.66) 0.4468 | 338 of 3046 |
| pocket / agent | 31 | 1 | 0 | 19 | 0 | 0 | 11 | 1471.0 | 0.3618 | 0 | 9 of 1805 |
| pocket / contour_spiral | 91 | 1 | 0 | 72 | 8 | 0 | 10 | 2630.0 | 0.3723 | 1 mop: seg 205 (57.59, 33.55)→(57.82, 32.83) 0.3723 | 420 of 3797 |
| pocket / geodesic_g1 | 89 | 1 | 43 | 23 | 6 | 0 | 16 | 1237.8 | 0.4478 | 1 mop: seg 168 (54.44, 28.13)→(55.00, 28.64) 0.4478 | 291 of 2974 |
| pocket / geodesic_a | 88 | 1 | 42 | 27 | 4 | 0 | 14 | 1126.3 | 0.3624 | 0 | 289 of 2845 |
| pocket / geodesic_a_through | 87 | 1 | 39 | 28 | 4 | 0 | 15 | 1126.3 | 0.4483 | 1 mop: seg 477 (53.95, 24.79)→(53.75, 24.07) 0.4483 | 276 of 2875 |
| pocket / geodesic_a_ramp | 98 | 1 | 49 | 24 | 9 | 0 | 15 | 1507.9 | 0.4743 | 3 mop: seg 264 0.4511; seg 273 0.4743; seg 273 0.4595 | 472 of 5378 |

Field lines: star geodesic_a: 18 levels, 98 wrap pieces, converged,
1 branch, 0 spiral levels. Pocket geodesic_a: 31 levels, 91 wrap pieces
(was 158 at D2: the shadow-line fix holds), converged, 3 branches,
0 spiral levels. The ramp columns: 6 (star) and 3 (pocket) spiral levels.

### Refused-step dump, `geodesic_a_ramp` (first ten lines, verbatim)

star:

```
  refused: clearing cut spiral (segment 4) from (46.39, 48.02) to (46.87, 47.20), reading 0.4210
  refused: clearing cut spiral (segment 4) from (46.87, 47.20) to (47.58, 46.51), reading 0.4071
  refused: clearing cut spiral (segment 4) from (47.58, 46.51) to (48.48, 46.02), reading 0.4650
  refused: clearing cut spiral (segment 4) from (48.48, 46.02) to (49.50, 45.77), reading 0.4612
  refused: clearing cut spiral (segment 4) from (49.50, 45.77) to (50.58, 45.79), reading 0.4946
  refused: clearing cut spiral (segment 4) from (50.58, 45.79) to (51.63, 46.07), reading 0.4395
  refused: clearing cut spiral (segment 4) from (51.63, 46.07) to (52.60, 46.61), reading 0.5045
  refused: clearing cut spiral (segment 4) from (52.60, 46.61) to (53.41, 47.38), reading 0.4799
  refused: clearing cut spiral (segment 4) from (53.41, 47.38) to (54.00, 48.33), reading 0.4071
  refused: clearing cut spiral (segment 4) from (54.00, 48.33) to (54.37, 49.39), reading 0.4562
```

pocket:

```
  refused: clearing cut spiral (segment 4) from (15.21, 17.53) to (15.30, 16.45), reading 0.3876
  refused: clearing cut spiral (segment 4) from (15.30, 16.45) to (15.81, 15.39), reading 0.3832
  refused: clearing cut spiral (segment 4) from (15.81, 15.39) to (16.69, 14.53), reading 0.4088
  refused: clearing cut spiral (segment 4) from (16.69, 14.53) to (17.85, 13.98), reading 0.4035
  refused: clearing cut spiral (segment 4) from (17.85, 13.98) to (19.17, 13.83), reading 0.4803
  refused: clearing cut spiral (segment 4) from (19.17, 13.83) to (20.51, 14.10), reading 0.4565
  refused: clearing cut spiral (segment 4) from (20.51, 14.10) to (21.72, 14.78), reading 0.4686
  refused: clearing cut spiral (segment 4) from (21.72, 14.78) to (22.71, 15.80), reading 0.5234
  refused: clearing cut spiral (segment 4) from (22.71, 15.80) to (23.30, 17.09), reading 0.4697
  refused: clearing cut spiral (segment 4) from (23.30, 17.09) to (23.50, 18.50), reading 0.4717
```

### Readings

- C3's prediction holds in kind: the first ten refused steps of each ramp
  column are all `spiral` steps. C3 predicted radius 5 to 7 mm and
  readings 0.40 to 0.47. The measured radii from the seed are 3.4 to
  5.0 mm (star seed (49.5, 50.5), pocket seed (18.5, 18.5)), and the
  readings are 0.38 to 0.52 of D. The ramp law does not hold on the curved
  front (C3 §3).
- Without the ramp (`geodesic_a`) the emitted plan holds the ceiling on
  both shapes (0.3600, 0.3624). Entries stay high: 92 (star), 88 (pocket).
  Joins at wrap ends are the largest part (56 and 42).
- In the candidate columns without the ramp (`geodesic_g1`, `geodesic_a`,
  `geodesic_a_through`), the first refused raw steps are the radial
  seam join and the first wrap at the seed (reading 0.8333 of D), then
  steps on the first wraps (0.37 to 0.45).
- Emitted over-steps: all of them are `residue mop` cuts, except one
  clearing cut of the shipped Agent on the star.

### `git status --short` at the end of D3

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
```

## D4 at 6f699bcf

A3 cbeb0acb (the plan's own floor, `adaptive3d/intended_floor.rs`) and C4
6f699bcf (core spiral, lobe morph, return option). No merge of master. Same
lane script, own target dir, a `timeout` on each command. Disk: 45 GB free.

### Fix (uncommitted)

- `crates/rs_cam_core/src/stock/standing_islands.rs:1100` (A3 unit-test
  helper), clippy `manual_is_multiple_of`.
  - Before: `(i / cols) % 5 != 0`
  - After: `!(i / cols).is_multiple_of(5)`
  - Why: the lint denies it. Same value. `--lib standing_islands` 6/6 after.

No other fix. Arm (e) of the roughcov sentry passed, so its assertion is
unchanged.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green (also green after the fix) |
| 1 | three `check` builds (core `test-support`; core `research,test-support`; viz) | green |
| 2 | `--lib standing_islands` | green 6/6 |
| 2 | `--lib sim_triage` | green 22/22 |
| 2 | `--lib adaptive3d` | green 66 passed, 7 ignored |
| 2 | `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov` | green 8/8 (arm (e) included) |
| 2 | `--features test-support`: all 14 adaptive3d sentries, `adaptive3d_emission_byte_parity`, `..._g_plansimgap`, `..._planner_never_ahead_of_emitted_path`, `..._global_gate_drapes_below_floor` included | green. Byte parity green: the factored floor rule changes no shipped byte |
| 2 | `test -p rs_cam_viz -q --lib` | green 585 passed, 2 ignored; `the_gui_adopt_reads_the_rough_coverage_g_roughcov` asserts `island.cells == 9 * 17` (line 188) and passes |
| 2 | `test -p rs_cam_mcp -q` | green 36/36 |
| 3 | `--features research,test-support --lib research_geodesic` | green 7 passed, 1 ignored (the G-MOPLOAD pass-load test) |
| 3 | the harness, release, `timeout 900` | green 1/1, 2.64 s |
| 4 | full workspace clippy | red (1 lint) → green after the fix |

### C4 refactor check: `geodesic_a` against D3

| Shape | D3 | D4 | Same |
|---|---|---|---|
| star | 92 = S1 J56 M20 P15; max 0.3600; raw 206 / 2330 | 92 = S1 J56 M20 P15; max 0.3600; raw 206 / 2330 | yes |
| pocket | 88 = S1 J42 M27 C4 P14; max 0.3624; raw 289 / 2845 | 88 = S1 J42 M27 C4 P14; max 0.3624; raw 289 / 2845 | yes |

The C4 refactor did not change candidate (a). The agent, contour_spiral and
geodesic_g1 columns also keep their D3 entry splits.

### Harness table

Output: `/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
The `geodesic_a_through` and `geodesic_a_ramp` columns are gone in C4. Split:
S starter, J join at a wrap end, M mid-wrap split, C residue contour, W wall
pass, P residue mop. Retracts equal entries.

| Shape / column | Entries | S/J/M/C/W/P | Max emitted / 0.3626 | Emitted over | Raw over |
|---|---|---|---|---|---|
| star / agent | 25 | 1/0/17/0/0/7 | 0.4515 | 1 | 9 / 1749 |
| star / contour_spiral | 101 | 1/0/93/0/0/7 | 0.3613 | 0 | 501 / 4873 |
| star / geodesic_g1 | 86 | 1/58/12/0/0/15 | 0.3614 | 0 | 219 / 2422 |
| star / geodesic_a | 92 | 1/56/20/0/0/15 | 0.3600 | 0 | 206 / 2330 |
| star / geodesic_morph_alt | 69 | 1/30/14/12/0/12 | **0.9745** | **27 / 2522** | 272 / 2312 |
| star / geodesic_morph_return | 79 | 1/35/14/12/0/17 | **0.9745** | **24 / 2357** | 314 / 2264 |
| pocket / agent | 31 | 1/0/19/0/0/11 | 0.3618 | 0 | 9 / 1805 |
| pocket / contour_spiral | 91 | 1/0/72/8/0/10 | 0.3723 | 1 | 420 / 3797 |
| pocket / geodesic_g1 | 89 | 1/43/23/6/0/16 | 0.4478 | 1 | 291 / 2974 |
| pocket / geodesic_a | 88 | 1/42/27/4/0/14 | 0.3624 | 0 | 289 / 2845 |
| pocket / geodesic_morph_alt | 77 | 1/45/11/9/0/11 | 0.4517 | 1 (mop) | 280 / 3690 |
| pocket / geodesic_morph_return | 83 | 1/53/10/9/0/10 | 0.4517 | 1 (mop) | 316 / 3521 |

Per-producer load, morph columns (max reading, steps over / steps):

| Class | star morph_alt | star morph_return | pocket morph_alt | pocket morph_return |
|---|---|---|---|---|
| emitted: starter | 0.3114, 0/160 | 0.3114, 0/160 | 0.3114, 0/160 | 0.3114, 0/160 |
| emitted: clearing | 0.3618, 0/816 | 0.3618, 0/759 | 0.3602, 0/1926 | 0.3591, 0/1833 |
| emitted: clearing link | 0.3185, 0/5 | 0.3185, 0/5 | 0.3229, 0/12 | 0.3229, 0/9 |
| emitted: residue contour link | **0.9745, 17/67** | **0.9745, 17/46** | 0.3093, 0/11 | 0.3093, 0/11 |
| emitted: residue contour | 0.6086, 1/670 | 0.6086, 1/618 | 0.3101, 0/767 | 0.3333, 0/769 |
| emitted: wall pass link | 0.3508, 0/63 | 0.1442, 0/104 | 0.1632, 0/11 | 0.1632, 0/32 |
| emitted: wall pass | 0.4198, 1/31 | 0.3230, 0/43 | 0.2500, 0/6 | 0.2500, 0/14 |
| emitted: residue mop link | 0.0527, 0/122 | 0.2500, 0/65 | 0.1448, 0/74 | 0.1448, 0/74 |
| emitted: residue mop | 0.4530, 8/588 | 0.4814, 6/557 | 0.4517, 1/388 | 0.4517, 1/386 |
| raw: clearing spiral | 0.5044, 138/392 | 0.5044, 138/392 | 0.4882, 55/151 | 0.4882, 55/151 |
| raw: clearing morph | 0.5972, 51/994 | 0.7598, 75/514 | 0.7064, 123/1930 | 0.8658, 113/983 |
| raw: clearing wrap | 0.9550, 52/651 | 0.9550, 56/609 | 0.9316, 63/1287 | 0.8802, 81/1233 |
| raw: clearing join | 0.8333, 28/30 | 0.8333, 42/47 | 0.8685, 17/28 | 0.9119, 35/52 |
| raw: clearing link | 0.5376, 3/85 | 0.5376, 3/84 | 0.9601, 22/134 | 0.9647, 24/133 |
| raw: clearing return | — | 0.3586, 0/458 | — | 0.9412, 8/809 |

Lobe stop reasons (C4 `lobes` line):

| Shape | Lobes / arcs | Plain wraps | Not joined: split | branch | length ratio | gap | Left the region | Return length |
|---|---|---|---|---|---|---|---|---|
| star (alt and return) | 6 / 29 | 63 | 53 | 0 | 11 | 0 | 0 | 0.0 mm (alt), 192.1 mm (return) |
| pocket (alt and return) | 5 / 31 | 57 | 51 | 2 | 0 | 2 | 0 | 0.0 mm (alt), 363.8 mm (return) |

The star morph over-steps, verbatim from `summary.txt` (first three of 17 on
the one link):

```
  over: residue contour link (segment 96) from (48.00, 67.43) to (49.19, 66.53), reading 0.8836
  over: residue contour link (segment 96) from (49.19, 66.53) to (50.39, 65.63), reading 0.9663
  over: residue contour link (segment 96) from (50.39, 65.63) to (51.58, 64.73), reading 0.9663
```

The link runs straight from (48.00, 67.43) to (69.51, 51.19), about 27 mm,
at 0.55 to 0.97 of D. It is an EMITTED link of the residue contour cleanup
(shipped code), on the stock that the morph plan left. The same link
is in both star morph columns (segment 96 and 106).

### Magic numbers (A3 and C4 diffs; not changed)

Product code:

| Place | Literal | Derivation |
|---|---|---|
| `stock/standing_islands.rs:665` (A3) | `1e-9 * cell * cell` in `r2 = r * r + 1e-9 * cell * cell` | A float-equality epsilon on the disk test, scaled by the cell. UNDERIVED, but not a physical value |
| `adaptive/research_geodesic.rs:786` (C4) | `cell / 2.0` (`ray_step`) | Derived in its doc: half a cell, so every cell that a ray crosses for more than half a cell holds a sample |
| `research_geodesic.rs` (C4) | `.max(3.0)` | Unchanged from C2: the fewest chords of a closed polygon |
| `research_geodesic.rs` (C4) | `1e-9` (`eps`) | Float epsilon |
| C4 length ratio `(R + 2s)/(R + s)` and half pitch | no literal | Derived, as C4 states |
| `adaptive3d/intended_floor.rs`, `clearing.rs`, `search.rs`, `path.rs` (A3) | none | The floor rule is a factor of the shipped one; byte parity proves it |

Test fixture values (A3 sentry and unit tests; C4 unit tests): tool 6.0 x
20.0/25.0, grid 23 x 31 at 0.25, `NO_BLOCK`, `VALLEY_SLOPE 0.4`,
`VALLEY_X 20.0`, `level_z 6.5`, band 26.0..30.0 and the arm (e) `<= 28.0`
bound, boundary 10.1/29.9, fringe and strip blocks 32.7..33.1, 41 cells,
`PLATEAU_Z 5.0`, `PLATEAU_EDGE_X 20.1`, h0/h1 15.1/25.1, disc radii 20.0 and
30.0, `rectangle_lobe(5, 40.0, ..)`, `4.0 * s`; epsilons 1e-4, 1e-9, 1e-12.

### `git status --short` at the end of D4

```
 M crates/rs_cam_core/src/stock/standing_islands.rs
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **The star morph columns emit a keep-down link over the pass load.** A
   residue contour link of about 27 mm reads 0.55 to 0.97 of D (17 emitted
   steps over). It is shipped cleanup code reacting to the stock the morph
   plan leaves, but it is in the EMITTED plan. Find why the residue
   contour cleanup keeps this link (a G-MOPLOAD-type shipped defect, or a
   link that the readout judges and the cleanup does not).
2. **The core spiral does not hold the pass load in the raw plan.** Its
   raw steps are refused on 138 of 392 (star) and 55 of 151 (pocket), max
   0.50 and 0.49. This is the same curved-front cause that C3 found for
   the ramp (the straight-edge bound). The morph is refused on 51 to 123
   steps per column (max 0.60 to 0.87).
3. **The morph lowers entries but most wraps stay unjoined.** Entries fall
   from 92 to 69 (star alt) and from 88 to 77 (pocket alt). The return
   option gives more entries (79, 83) than alt. The largest stop reason is
   "split": 53 not joined on the star, 51 on the pocket (against 6 lobes of
   29 arcs and 5 lobes of 31 arcs). The star also stops 11 for the length
   ratio; the pocket stops 2 for branch and 2 for gap.

## D5 at 04b24757

C5 cc0b6d93 (cascade diagnostic, curvature spacing law, columns
`geodesic_curv` / `geodesic_curv_morph`) and A4 04b24757 (free islands
first, op-independent wall tag). No merge of master. Same lane script, own
target dir, a `timeout` on each command. Disk: 45 GB free.

### Fixes

None. The tree had no change before this section.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | three `check` builds (core `test-support`; core `research,test-support`; viz) | green |
| 2 | `--lib standing_islands` | green 6/6 |
| 2 | `--lib sim_triage` | green 22/22 |
| 2 | `--test a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov` | green 9/9 |
| 2 | `--lib the_narration_sentence_states` | green 1/1 |
| 2 | `test -p rs_cam_viz -q --lib` (the g_roughcov island and the narration sentence) | green 585 passed, 2 ignored |
| 2 | `test -p rs_cam_mcp -q` | green 36/36 |
| 3 | `--features research,test-support --lib research_geodesic` | green 8 passed, 1 ignored (G-MOPLOAD) |
| 3 | the harness, release, `timeout 900` | green 1/1, 3.81 s |
| 4 | full workspace clippy | green |

### C5 refactor controls

| Column | D4 | D5 | Same |
|---|---|---|---|
| star / geodesic_a | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | yes |
| pocket / geodesic_a | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| star / geodesic_morph_alt | 69 = S1 J30 M14 C12 P12; 0.9745; raw 272/2312 | 69 = S1 J30 M14 C12 P12; 0.9745; raw 272/2312 | yes |
| pocket / geodesic_morph_alt | 77 = S1 J45 M11 C9 P11; 0.4517; raw 280/3690 | 77 = S1 J45 M11 C9 P11; 0.4517; raw 280/3690 | yes |

The C5 refactor did not change the existing columns.

### The two new columns

Output: `/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
Split: S starter, J join at a wrap end, M mid-wrap split, C residue contour,
W wall pass, P residue mop. Retracts equal entries.

| Shape / column | Entries | S/J/M/C/W/P | Max emitted / 0.3626 | Emitted over-steps | Raw over | C5 prediction |
|---|---|---|---|---|---|---|
| star / geodesic_curv | 73 | 1/58/0/0/0/14 | 0.3581 | 0 | 145 / 2545 | 70 to 80 (holds); M 0 to 5 (holds, 0) |
| star / geodesic_curv_morph | 84 | 1/56/12/0/0/15 | 0.3625 | 0 | 163 / 2602 | 35 to 50 (FAILS) |
| pocket / geodesic_curv | 63 | 1/34/12/6/0/10 | 0.4677 | 2 (mop: seg 134, 0.4526 and 0.4677) | 260 / 2952 | 75 to 82 (better); M 10 to 15 (holds, 12) |
| pocket / geodesic_curv_morph | 86 | 1/60/13/0/0/12 | 0.3616 | 0 | 254 / 3677 | 45 to 60 (FAILS) |

Field lines: star curv 19 levels, 100 wrap pieces, 0 spiral levels; star
curv_morph 19 levels, 128 wrap pieces, 6 spiral levels; pocket curv 32
levels, 99 wrap pieces, 0 spiral levels; pocket curv_morph 32 levels, 126
wrap pieces, 4 spiral levels. All converged.

### Cascade diagnostic (replay model on the raw plan)

| Column | Class | Refused | Cascade | Intrinsic | Max planned reading | Raw tally (over / steps) |
|---|---|---|---|---|---|---|
| star curv | total | 128 | 20 | 108 | | |
| | clearing join | 97 | 0 | 97 | 0.8956 | 106 / 130 |
| | clearing wrap | 30 | 20 | 10 | 0.9308 | 38 / 2173 |
| | clearing link | 1 | 0 | 1 | 0.3768 | 1 / 82 |
| star curv_morph | total | 119 | 19 | 100 | | |
| | clearing spiral | 12 | 0 | 12 | 0.4044 | 22 / 348 |
| | clearing join | 38 | 0 | 38 | 0.8485 | 44 / 68 |
| | clearing wrap | 19 | 11 | 8 | 0.9308 | 23 / 760 |
| | clearing link | 1 | 0 | 1 | 0.3768 | 1 / 82 |
| | clearing morph | 49 | 8 | 41 | 0.6467 | 73 / 1184 |
| pocket curv | total | 876 | 663 | 213 | | |
| | clearing join | 104 | 17 | 87 | 0.9460 | 89 / 113 |
| | clearing wrap | 763 | 640 | 123 | 0.9300 | 146 / 2545 |
| | clearing link | 9 | 6 | 3 | 0.4080 | 25 / 134 |
| pocket curv_morph | total | 311 | 151 | 160 | | |
| | clearing spiral | 12 | 0 | 12 | 0.4211 | 17 / 187 |
| | clearing link | 9 | 2 | 7 | 0.7145 | 23 / 129 |
| | clearing wrap | 95 | 55 | 40 | 0.9419 | 60 / 1209 |
| | clearing join | 42 | 12 | 30 | 0.9302 | 39 / 62 |
| | clearing morph | 153 | 82 | 71 | 0.7510 | 115 / 1930 |

First refusal of each producer piece: star curv 74 pieces (12 cascades);
star curv_morph 43 (7); pocket curv 131 (56); pocket curv_morph 63 (19).

### The core spiral: the first three intrinsic refusals (verbatim)

Only the `_curv_morph` columns have a core spiral (`geodesic_curv` has 0
spiral levels). Star seed (49.5, 50.5), pocket seed (18.5, 18.5).

star / geodesic_curv_morph:

```
  intrinsic: clearing spiral (segment 4) from (46.20, 55.28) to (45.10, 54.40), replay reading 0.4008, planned reading 0.4008, intrinsic, 5.88 mm from the seed
  intrinsic: clearing spiral (segment 4) from (45.37, 45.83) to (45.91, 45.30), replay reading 0.4044, planned reading 0.4044, intrinsic, 6.31 mm from the seed
  intrinsic: clearing spiral (segment 4) from (55.83, 48.11) to (56.18, 48.85), replay reading 0.3630, planned reading 0.3630, intrinsic, 6.88 mm from the seed
```

pocket / geodesic_curv_morph:

```
  intrinsic: clearing spiral (segment 4) from (15.72, 21.71) to (14.87, 20.83), replay reading 0.4039, planned reading 0.4039, intrinsic, 4.31 mm from the seed
  intrinsic: clearing spiral (segment 4) from (14.87, 20.83) to (14.30, 19.73), replay reading 0.3809, planned reading 0.3781, intrinsic, 4.37 mm from the seed
  intrinsic: clearing spiral (segment 4) from (15.43, 14.96) to (16.53, 14.18), replay reading 0.3859, planned reading 0.3859, intrinsic, 4.75 mm from the seed
```

Under the law, the spiral's raw over-steps fall from 138/392 to 22/348
(star) and from 55/151 to 17/187 (pocket). 12 intrinsic refusals remain on
each shape, all within 4.3 to 6.9 mm of the seed, at 0.36 to 0.40 of D.

### Lobe stop reasons (split three ways)

| Column | Lobes / arcs | Plain wraps | Split: total (two or more children / child with two or more parents / closed) | Branch | Length ratio | Gap | Left the region |
|---|---|---|---|---|---|---|---|
| star curv_morph | 6 / 34 | 60 | 51 (43 / 2 / 6) | 0 | 8 | 0 | 0 |
| pocket curv_morph | 5 / 31 | 64 | 53 (40 / 3 / 10) | 2 | 2 | 2 | 0 |

### C5's keep-or-drop rule for the morph (P2_C2 §(3))

Rule: keep the morph when, in `geodesic_curv_morph`, the `clearing morph`
intrinsic rate (intrinsic / raw-tally steps) is at or below the
`clearing wrap` intrinsic rate of the same column, AND most of the morph's
replay refusals are cascades. Otherwise drop it.

| Shape | Morph intrinsic rate | Wrap intrinsic rate | Morph ≤ wrap | Morph cascades / refused | Most are cascades |
|---|---|---|---|---|---|
| star | 41 / 1184 = 3.46 % | 8 / 760 = 1.05 % | no | 8 / 49 | no |
| pocket | 71 / 1930 = 3.68 % | 40 / 1209 = 3.31 % | no | 82 / 153 | yes |

The first condition fails on both shapes, so the tie-break does not apply.
**The rule says DROP the morph.** The fallback is the `geodesic_curv` column,
which needs no code. The entries agree: `geodesic_curv` gives 73 and 63,
`geodesic_curv_morph` gives 84 and 86.

### Magic numbers (C5 and A4 diffs; not changed)

- Product code: none. The C5 spacing law `s'(ρ)` and
  `ρ* = s(2R − s)/(2(cD − s))` have no literal.
- Test values: C5 epsilons `1e-9`, `1e-3`, and `s_prime(1e6, ..)` (a large ρ
  to test the limit `s' → cD`); A4 sentry fixtures `HOLE_LO 15.1`,
  `HOLE_HI 25.1`, `core_top = FLOOR_Z + STL_MM + 1.5`, tool 25.0 long, the core
  block 19.5..20.5, the sliver 15.2..15.3 x 19.0..21.0, `flat(50.0, ..)`.

### `git status --short` at the end of D5

```
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **The morph rule says drop it.** Its intrinsic refusal rate is above the
   plain wrap's rate on both shapes (3.46 % vs 1.05 %; 3.68 % vs 3.31 %), and
   `geodesic_curv_morph` has MORE entries than `geodesic_curv` (84 vs 73;
   86 vs 63). C5 predicted 35 to 50 and 45 to 60.
2. **Joins are the remaining cost of `geodesic_curv`.** Star: 58 of 73
   entries are joins, and 97 of its 108 intrinsic refusals are joins
   (planned reading up to 0.90). The first ones are the radial seam steps
   at 4.4 to 15 mm from the seed (0.50 to 0.83). The spacing law removes the
   mid-wrap splits on the star (M 20 → 0), not the joins.
3. **The pocket cascade count is large.** In `pocket / geodesic_curv`, 640
   of 763 wrap refusals are cascades (663 of 876 in total). The law
   under-protects behind the island, as C5 expected. Its emitted mop also
   steps over the ceiling twice (0.4526, 0.4677).

## D6 at f5b3725c

C6 (transition join, local spacing, half-cell read columns;
`research_geodesic.rs` and the harness only). No merge of master. Same lane
script, own target dir, a `timeout` on each command. Disk: 44 GB free.

### Fix (uncommitted)

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs:2255`
  (`curvature_levels`), rustc `unused variable: b` (clippy `-D warnings`
  refused the build; `check` gave the warning).
  - Before: `let r = tool_radius; let b = r - ceiling_width; let mut out = vec![0.0];`
  - After: `let r = tool_radius; let mut out = vec![0.0];`
  - Why: `b` is dead. The function calls `law_next_radius`, which computes
    the same `b = r - ceiling_width` itself (`:2040`). The deletion is
    behaviour-neutral.

No other fix. The red unit test below is NOT fmt or clippy, so it is not
changed.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --features research,test-support` | green, 1 warning (`b`) → fixed |
| 2 | `--features research,test-support --lib research_geodesic` | **RED**: 8 passed, 1 failed, 1 ignored |
| 3 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | red (`b`) → green after the fix |
| 4 | the harness, release, `timeout 900` | green 1/1, 6.05 s |

The red test, `the_transition_join_nests_its_sectors_on_a_disc` (C6's new
disc join test), first failing assertion verbatim (record list shortened):

```
panicked at crates/rs_cam_core/src/adaptive/research_geodesic.rs:4914:13:
assertion `left == right` failed: every transition is traced: JoinReport { rim_transition: 1, closed_transition: 15, transition_fallback: 1, closed_seam: 0, closed_to_arc: 9, from_arc: 15, transitions: [ .. TransitionRecord { level: 16, rho: 34.678787860002444, delta: 2.0, fraction: 0.4491409471307148, sector: 2.891589484113832, applied: false }] }
  left: 1
 right: 0
```

- Only the outermost transition (level 16, ρ 34.68) is not traced. The
  tool-centre disc has radius 35, so that wrap lies within 0.32 mm of the
  region edge. A transition that leaves the region falls back. This looks
  like an edge case of the test fixture or of the trace at the region
  edge. The lead decides which.
- The test has later checks that would also fail on these records (they
  were not reached):
  - "the sectors nest": level 2 has sector 6.7325 after 6.2660 at level 1
    (allowed 6.2660 + 0.5/4.32 = 6.382); level 9 has 6.3561 after 6.2014
    (allowed about 6.228); level 12 has 6.4872 after 6.3863.
  - `all(|r| r.applied)` fails on level 16.
  Only the first assertion fired.

### Controls (exact)

| Column | Required | D6 | Same |
|---|---|---|---|
| star / geodesic_a | 92 | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | yes |
| pocket / geodesic_a | 88 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| star / geodesic_curv | 73 = S1 J58 M0 P14; 0.3581; raw 145/2545 | the same | yes |
| pocket / geodesic_curv | 63 = S1 J34 M12 C6 P10; 0.4677; raw 260/2952 | the same | yes |
| geodesic_curv_fine | entries, split, emitted max of geodesic_curv | star 73 S1 J58 M0 P14, 0.3581; pocket 63 S1 J34 M12 C6 P10, 0.4677 | yes |
| geodesic_curv_join_fine | the same as geodesic_curv_join | star 89 S1 J56 M18 P14, 0.3616; pocket 72 S1 J32 M23 C6 P10, 0.4677 | yes |

The C6 refactor did not change the existing columns. (The `_fine` columns
read the raw plan at the half cell, so their raw counts differ by design:
star 298/2738 and pocket 539/3177 for `curv_fine`; 417/3070 and 604/3347
for `curv_join_fine`.)

### The new columns

Output: `/home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
Split: S starter, J join at a wrap end, M mid-wrap split, C residue contour,
W wall pass, P residue mop. Retracts equal entries.

| Shape / column | Entries | S/J/M/C/W/P | Max emitted / 0.3626 | Emitted over | Raw over |
|---|---|---|---|---|---|
| star / geodesic_curv (reference) | 73 | 1/58/0/0/0/14 | 0.3581 | 0 | 145 / 2545 |
| star / geodesic_curv_join | 89 | 1/56/18/0/0/14 | 0.3616 | 0 | 169 / 2883 |
| star / geodesic_curv_local | 85 | 1/62/5/0/0/17 | 0.3620 | 0 | 173 / 2691 |
| star / geodesic_curv_local_join | 100 | 1/59/24/0/0/16 | 0.3578 | 0 | 205 / 3145 |
| star / geodesic_curv_fine | 73 | 1/58/0/0/0/14 | 0.3581 | 0 | 298 / 2738 |
| star / geodesic_curv_join_fine | 89 | 1/56/18/0/0/14 | 0.3616 | 0 | 417 / 3070 |
| pocket / geodesic_curv (reference) | 63 | 1/34/12/6/0/10 | 0.4677 | 2 (mop) | 260 / 2952 |
| pocket / geodesic_curv_join | 72 | 1/32/23/6/0/10 | 0.4677 | 2 (mop) | 269 / 3128 |
| pocket / geodesic_curv_local | 89 | 1/53/21/0/0/14 | 0.3620 | 0 | 253 / 3231 |
| pocket / geodesic_curv_local_join | 94 | 1/51/28/0/0/14 | 0.3620 | 0 | 262 / 3423 |
| pocket / geodesic_curv_fine | 63 | 1/34/12/6/0/10 | 0.4677 | 2 (mop) | 539 / 3177 |
| pocket / geodesic_curv_join_fine | 72 | 1/32/23/6/0/10 | 0.4677 | 2 (mop) | 604 / 3347 |

### Joins by kind (verbatim `joins piece 0:` lines)

```
star / geodesic_curv_join:       rim transition 1, closed-to-closed transition 13, transition refused (seam) 3, closed-to-closed seam 0, closed to arc 33, from an arc 50; local-spacing sweep converged None
star / geodesic_curv_local:      rim transition 0, closed-to-closed transition 0, transition refused (seam) 0, closed-to-closed seam 20, closed to arc 36, from an arc 56; local-spacing sweep converged Some(true)
star / geodesic_curv_local_join: rim transition 1, closed-to-closed transition 16, transition refused (seam) 4, closed-to-closed seam 0, closed to arc 36, from an arc 56; local-spacing sweep converged Some(true)
star / geodesic_curv_fine:       rim transition 0, closed-to-closed transition 0, transition refused (seam) 0, closed-to-closed seam 16, closed to arc 33, from an arc 50; local-spacing sweep converged None
pocket / geodesic_curv_join:       rim transition 1, closed-to-closed transition 10, transition refused (seam) 2, closed-to-closed seam 0, closed to arc 27, from an arc 59; local-spacing sweep converged None
pocket / geodesic_curv_local:      rim transition 0, closed-to-closed transition 0, transition refused (seam) 0, closed-to-closed seam 3, closed to arc 5, from an arc 68; local-spacing sweep converged Some(true)
pocket / geodesic_curv_local_join: rim transition 1, closed-to-closed transition 3, transition refused (seam) 0, closed-to-closed seam 0, closed to arc 5, from an arc 68; local-spacing sweep converged Some(true)
pocket / geodesic_curv_fine:       rim transition 0, closed-to-closed transition 0, transition refused (seam) 0, closed-to-closed seam 12, closed to arc 27, from an arc 59; local-spacing sweep converged None
```

(`_join_fine` repeats `_join` on both shapes.) In the star join column, the
transitions of levels 6→7, 7→8 and 15→16 are `applied false`. All
transitions in the core zone have f = 1.000.

### Cascade block per class (refused / cascade / intrinsic, max planned reading)

| Column | Total | transition | join | wrap | link |
|---|---|---|---|---|---|
| star curv | 128 / 20 / 108 | — | 97 / 0 / 97 (0.8956) | 30 / 20 / 10 (0.9308) | 1 / 0 / 1 (0.3768) |
| star curv_join | 154 / 28 / 126 | 46 / 8 / 38 (0.9375) | 87 / 2 / 85 (0.8956) | 20 / 18 / 2 (0.8333) | 1 / 0 / 1 (0.3768) |
| star curv_local | 144 / 24 / 120 | — | 83 / 1 / 82 (0.9002) | 59 / 23 / 36 (0.9128) | 2 / 0 / 2 (0.4210) |
| star curv_local_join | 160 / 22 / 138 | 39 / 1 / 38 (0.8188) | 73 / 0 / 73 (0.9002) | 46 / 21 / 25 (0.9128) | 2 / 0 / 2 (0.4210) |
| pocket curv | 876 / 663 / 213 | — | 104 / 17 / 87 (0.9460) | 763 / 640 / 123 (0.9300) | 9 / 6 / 3 (0.4080) |
| pocket curv_join | 884 / 665 / 219 | 67 / 29 / 38 (0.9531) | 89 / 15 / 74 (0.9460) | 719 / 615 / 104 (0.9300) | 9 / 6 / 3 (0.4080) |
| pocket curv_local | 202 / 39 / 163 | — | 67 / 0 / 67 (0.8888) | 128 / 38 / 90 (0.9282) | 7 / 1 / 6 (0.6647) |
| pocket curv_local_join | 207 / 40 / 167 | 11 / 1 / 10 (0.5012) | 65 / 0 / 65 (0.8888) | 124 / 38 / 86 (0.9282) | 7 / 1 / 6 (0.6647) |

The `_fine` columns print the totals only: star curv_fine 231 / 47 / 184,
pocket curv_fine 1331 / 900 / 431; star curv_join_fine 272 / 61 / 211,
pocket curv_join_fine 1336 / 895 / 441.

### The quantum test (intrinsic refusals in the first quantum over the ceiling, (c, c + cell/D])

| Column | Planner cell 0.5 mm: near-ceiling / intrinsic | Half cell 0.25 mm: near-ceiling / intrinsic |
|---|---|---|
| star curv | 5 / 108 (4.6 %) | 19 / 184 (10.3 %) |
| pocket curv | 56 / 213 (26.3 %) | 134 / 431 (31.1 %) |
| star curv_join | 21 / 126 (16.7 %) | 31 / 211 (14.7 %) |
| pocket curv_join | 60 / 219 (27.4 %) | 142 / 441 (32.2 %) |

The quantum at the half cell is half as wide (cell/D = 0.0417 against
0.0833). Even so, the near-ceiling count RISES at the half cell (5 → 19,
56 → 134, 21 → 31, 60 → 142), and the intrinsic total rises too (108 →
184, 213 → 431). The near-ceiling refusals do not go away at a finer read,
so they are not a quantum artefact of the planner cell. The finer read
finds more steps over the ceiling. This agrees with the
`PassLoad::swept_width` doc: the lattice reads a side cut up to one cell
NARROW, so the planner cell under-reads.

### `git status --short` at the end of D6

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **C6's new disc join test is red.** The outermost transition (level
   16, 0.32 mm from the region edge) is not traced. The later checks of the
   same test (sector nesting at levels 2, 9 and 12; all applied) would also
   fail on these records. Decide whether the test or the trace near the
   edge is wrong.
2. **No C6 column beats `geodesic_curv` on entries.** Star: curv 73, join
   89, local 85, local_join 100. Pocket: curv 63, join 72, local 89,
   local_join 94. The transition join adds mid-wrap splits (star M 0 →
   18). The local spacing is the only variant that removes the pocket
   cascades (876 → 202 refusals) and the mop over-steps (max 0.4677 →
   0.3620), but it costs entries.
3. **The quantum test says the near-ceiling refusals are real.** At the
   half cell they rise (star curv 5 → 19, pocket 56 → 134), so the planner
   cell under-reads the swept width. A plan that the planner cell reads as
   legal can be over the ceiling at a finer read.

## D7 at 8deecf98

Master 629748f1 merged by the lead, then C7 (`PassLoadMeasure` threaded
through `adaptive/path.rs` and `search.rs`, shipped sites on `SweptWidth`;
research columns `geodesic_curv_area`, `agent_area`,
`contour_spiral_area`). Same lane script, own target dir, a `timeout` on
each command. Disk: 43 GB free.

### Fixes

None.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core` (plain) | green, no warning |
| 1 | `check -p rs_cam_core --features research,test-support` | green, no warning |
| 2 | `--lib adaptive` | green 141 passed, 7 ignored |
| 2 | `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | red, the SAME red as master: moves 2810 and 6159, sim 0.8329080445671909, exact 0.9403847504031052 |
| 2 | `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` | red, the SAME red as master: move 6930, clearance 0.24951132908296492 |
| 2 | `a_link_feeds_only_within_the_pass_load_g_adaptlinkload`, `adaptive_keeps_its_planner_order_g_adaptorder`, `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0`, `adaptive_plans_every_inset_piece_g_adaptpieces` | green |
| 2 | `adaptive3d_emission_byte_parity` | green 5/5: the threaded measure changes no shipped byte |
| 2 | the 14 adaptive3d sentries (`--features test-support`) | green |
| 3 | `--features research,test-support --lib research_geodesic` | green 11 passed, 2 ignored (G-MOPLOAD; and the C6 disc join test, now `#[ignore]` with the D6 reason) |
| 4 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | green |
| 4 | `clippy -p rs_cam_core --all-targets -- -D warnings` (plain: the `expect(dead_code)` without `research`) | green |
| 5 | the harness, release, `timeout 900` | green 1/1, 7.67 s |

### Controls (exact)

| Column | Star | Pocket | Same as before |
|---|---|---|---|
| geodesic_a | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| geodesic_curv | 73 = S1 J58 M0 P14; 0.3581; raw 145/2545 | 63 = S1 J34 M12 C6 P10; 0.4677; raw 260/2952 | yes |
| agent | 25 = S1 M17 P7; 0.4515; raw 9/1749 | 31 = S1 M19 P11; 0.3618; raw 9/1805 | yes (D3) |
| contour_spiral | 101 = S1 M93 P7; 0.3613; raw 501/4873 | 91 = S1 M72 C8 P10; 0.3723; raw 420/3797 | yes (D3) |

The harness also asserts that each shipped column equals
`adaptive_toolpath` move by move, and it passed: product parity holds.

### The three area columns (replay measure `AreaPerMm`)

Split: S starter, J join at a wrap end, M mid-wrap split, C residue contour,
P residue mop. Retracts equal entries.

| Shape / column | Entries (swept-width column) | S/J/M/C/P | Emitted: swept width max, over | Emitted: area per mm max, over | Raw over (area / steps) |
|---|---|---|---|---|---|
| star / geodesic_curv_area | **199** (73) | 6/75/104/0/14 | 0.7492, 20 | 0.7187, 6 | 1543 / 2545 |
| star / agent_area | **119** (25) | 6/0/105/0/8 | 0.4746, 6 | 0.3622, 0 | 672 / 1749 |
| star / contour_spiral_area | **134** (101) | 6/0/124/0/4 | 0.6778, 16 | 0.3854, 1 | 887 / 4873 |
| pocket / geodesic_curv_area | **217** (63) | 6/74/124/0/13 | 0.6458, 6 | 0.4375, 1 | 1934 / 2952 |
| pocket / agent_area | **140** (31) | 6/0/128/0/6 | 0.9792, 7 | 0.4757, 5 | 915 / 1805 |
| pocket / contour_spiral_area | **161** (91) | 6/0/123/25/7 | 0.8141, 7 | 0.4132, 2 | 996 / 3797 |

Per class, emitted plan (swept width max, over | area per mm max, over):

| Class | star curv_area | pocket curv_area | star agent_area | pocket agent_area | star cs_area | pocket cs_area |
|---|---|---|---|---|---|---|
| starter | 0.2179, 0 \| 0.3622, 0 | 0.2179, 0 \| 0.3622, 0 | the same | the same | the same | the same |
| clearing | 0.5587, 7 \| 0.3622, 0 | 0.6458, 6 \| 0.3620, 0 | 0.4651, 4 \| 0.3611, 0 | 0.9792, 5 \| 0.3611, 0 | 0.6778, 10 \| 0.3854, 1 | 0.5458, 5 \| 0.3621, 0 |
| clearing link | 0.2707, 0 \| 0.3350, 0 | 0.2639, 0 \| 0.2213, 0 | 0.3277, 0 \| 0.3495, 0 | 0.0620, 0 \| 0.0891, 0 | 0.3053, 0 \| 0.3559, 0 | 0, 0 \| 0, 0 |
| residue contour | — | — | — | — | — | 0.1623, 0 \| 0.3509, 0 |
| wall pass link | 0.5009, 1 \| 0.1874, 0 | 0.2938, 0 \| 0.3611, 0 | 0, 0 \| 0.0285, 0 | 0, 0 \| 0.0160, 0 | 0, 0 \| 0.0039, 0 | 0.8141, 2 \| 0.3603, 0 |
| wall pass | 0.6777, 1 \| 0.2780, 0 | 0.1667, 0 \| 0.3308, 0 | 0, 0 \| 0.1338, 0 | 0.3333, 0 \| 0.2751, 0 | 0.4512, 1 \| 0.2317, 0 | 0.1529, 0 \| 0.3502, 0 |
| residue mop | 0.7492, 11 \| 0.7187, 6 | 0.3618, 0 \| 0.4375, 1 | 0.4746, 2 \| 0.3125, 0 | 0.7995, 2 \| 0.4757, 5 | 0.5180, 5 \| 0.3194, 0 | 0.3321, 0 \| 0.4132, 2 |
| residue mop link | 0.2991, 0 \| 0.1738, 0 | 0, 0 \| 0.0833, 0 | 0.1747, 0 \| 0.1667, 0 | 0, 0 \| 0.0833, 0 | 0, 0 \| 0.0892, 0 | 0, 0 \| 0.1968, 0 |

Per class, raw plan (swept width over | area per mm max, over / steps):

| Class | star curv_area | pocket curv_area | star agent_area | pocket agent_area | star cs_area | pocket cs_area |
|---|---|---|---|---|---|---|
| starter | 0 \| 0.8287, 36 / 160 | 0 \| 0.8287, 36 / 160 | 0 \| 0.8287, 36 / 160 | the same | the same | the same |
| clearing join | 106 \| 1.0569, 109 / 130 | 89 \| 1.0784, 96 / 113 | — | — | — | — |
| clearing wrap | 38 \| **4.8449**, 1396 / 2173 | 146 \| **7.8531**, 1775 / 2545 | — | — | — | — |
| clearing link | 1 \| 0.5112, 2 / 82 | 25 \| 0.9781, 27 / 134 | 0 \| 0.3495, 0 / 350 | 0 \| 0.1397, 0 / 258 | — | — |
| clearing | — | — | 9 \| 0.5694, 636 / 1239 | 9 \| 0.5833, 879 / 1387 | 501 \| 1.0954, 851 / 4713 | 420 \| 0.9572, 960 / 3637 |

Cascade (replay model on the raw plan, area measure; refused / cascade /
intrinsic; near-ceiling = intrinsic in the first quantum over the ceiling):

| Column | Total | Near-ceiling | starter | join | wrap | link | clearing |
|---|---|---|---|---|---|---|---|
| star curv_area | 269 / 5 / 264 | 31 | 6 / 0 / 6 | 47 / 0 / 47 | 214 / 5 / 209 (planned max 3.9384) | 2 / 0 / 2 | — |
| pocket curv_area | 333 / 3 / 330 | 43 | 6 / 0 / 6 | 53 / 0 / 53 | 268 / 3 / 265 (planned max 7.8531) | 6 / 0 / 6 | — |
| star agent_area | 168 / 2 / 166 | 103 | 6 / 0 / 6 | — | — | — | 162 / 2 / 160 |
| pocket agent_area | 260 / 10 / 250 | 150 | 6 / 0 / 6 | — | — | — | 254 / 10 / 244 |
| star cs_area | 274 / 2 / 272 | 35 | 6 / 0 / 6 | — | — | — | 268 / 2 / 266 |
| pocket cs_area | 264 / 0 / 264 | 60 | 6 / 0 / 6 | — | — | — | 258 / 0 / 258 |

### C7's prediction: raw join refusals about halve, J the same

| Shape | Join refusals, replay model (swept → area) | Raw join over (swept → area) | J entries (swept → area) |
|---|---|---|---|
| star | 97 → 47 (halved) | 106 → 109 (not halved) | 58 → **75** |
| pocket | 104 → 53 (halved) | 89 → 96 (not halved) | 34 → **74** |

- The replay-model join refusals halve, as C7 predicted.
- The raw tally of join steps over the ceiling does not fall.
- J does NOT stay the same: it rises on both shapes. The total entries of
  `geodesic_curv_area` rise 2.7 and 3.4 times, mostly from mid-wrap splits (M 0 → 104, 12 → 124).

### Readings

- The area measure changes the plan of every column a lot, the shipped
  producers under the research measure included (agent 25 → 119 and
  31 → 140; contour spiral 101 → 134 and 91 → 161).
- The STARTER is refused under the area measure. Its raw steps read up to
  0.8287 of D (36 of 160 over), and every area column has 6 starter
  entries (1 in the swept-width columns). Swept width reads the same
  starter steps at 0.3114.
- The raw wrap readings reach 4.84 (star) and 7.85 (pocket) of D. An
  area per mm above 1 D clears more than a full slot per mm, which
  cannot be the geometry. The doc of `compute_area_per_mm`
  (`search.rs:650`) gives the quantum error as `√2 · cell · P / ℓ` per mm,
  and that has no limit as the step length `ℓ` goes to 0. A short step (for
  example the last piece of a `subdivide`d move) over-reads without limit.
  This is the likely cause of the readings over 1 D, of the starter
  refusals and of the entry growth. It is NOT proven: this pass did not
  dump the step lengths of the refused steps.
- The area measure admits emitted steps that the swept width refuses: the
  emitted swept-width maxima in the area columns are 0.47 to 0.98 of D
  (pocket agent clearing at 0.9792).

### Magic numbers (C7 diff; not changed)

- Product code (`search.rs`, `path.rs`, `research_geodesic.rs`): none.
  `compute_area_per_mm` uses `cell²` and `(cell / SUB)²`; `area_per_mm`
  reuses the swept-width ceiling, target and band.
- Test fixtures (C7 unit tests): `stock_grid(45.0 | 40.0, ..)`,
  `10.0 + R`, `y = 10.0 + s`, `5.0` to `20.0`, `y_k = 15.0`,
  `wall_x = 40.0 - R`, `x0 = 20.0`, the epsilon `1e-9`.

### `git status --short` at the end of D7

```
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **The area-per-mm reading is not bounded on short steps.** Raw readings
   reach 4.8 and 7.9 of D. The quantum error grows as `1/ℓ` (the doc says
   so). Before the area columns are read as evidence, bound the step
   length (for example, read the area over a fixed length, or over the
   whole subdivided move), or dump `ℓ` for the refused steps.
2. **Under the area measure every column needs more entries: 1.3 times
   (star contour spiral) to 4.8 times (star agent).** This includes the shipped agent (25 → 119). The starter alone
   is refused 6 times. C7's prediction holds for the replay-model join
   refusals (97 → 47, 104 → 53), but J rises (58 → 75, 34 → 74), and M
   jumps (0 → 104, 12 → 124).
3. **Shipped neutrality holds.** Byte parity is green, the two master reds
   are unchanged, the 14 adaptive3d sentries and `--lib adaptive` are
   green, and both clippy configurations hold the `expect(dead_code)`.

## D8 at 71f4cf0b

C8 on top of the lead's master merge 3517d845: a mid-arc tangential
lead-in (`LobeMorph::Lead`), keep-down return links, and one research-only
line in `search.rs::compute_area_per_mm` (it divides by `max(ℓ, cell)`).
Same lane script, own target dir, a `timeout` on each command. Disk: 43 GB
free.

### Fix (uncommitted)

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs` ~5792 and ~5798,
  test `the_lobe_morph_on_a_rectangle_lobe_holds_one_stepover`: rustc
  E0004 `non-exhaustive patterns: LobeMorph::Lead not covered` (two
  `match morph`). The lib test target did not compile, so
  `--lib research_geodesic` and the research clippy could not run.
  - Before: the arms `Alternate` and `AlternateFull | Return`.
  - After: one more arm in each,
    `LobeMorph::Lead => panic!("this test covers Alternate and Return only")`.
  - Why: the test loops over `[LobeMorph::Alternate, LobeMorph::Return]`
    only, so the arm is never reached. The arm states the test's scope; it
    assumes nothing about the legs of `Lead`. The module already
    `#[expect(clippy::panic, ..)]`. Test code only: the harness ran before the
    fix, and its numbers stay valid.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green (also after the fix) |
| 1 | `check -p rs_cam_core` plain, and `--features research,test-support` | green, no warning |
| 2 | `--lib adaptive` | green 141 passed, 7 ignored |
| 2 | `--test adaptive3d_emission_byte_parity` | green 5/5 (the `search.rs` line is research-only) |
| 3 | `--features research,test-support --lib research_geodesic` | red (did not compile) → green 13 passed, 2 ignored after the fix |
| 4 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | red (the same E0004) → green after the fix |
| 4 | `clippy -p rs_cam_core --all-targets -- -D warnings` | green |
| 5 | the harness, release, `timeout 900` | green 1/1, 6.84 s |

### Controls (exact)

| Column | Star | Pocket | Same |
|---|---|---|---|
| geodesic_a | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| geodesic_curv | 73 = S1 J58 M0 P14; 0.3581; raw 145/2545 | 63 = S1 J34 M12 C6 P10; 0.4677; raw 260/2952 | yes |
| agent | 25; 0.4515; raw 9/1749 | 31; 0.3618; raw 9/1805 | yes |
| contour_spiral | 101; 0.3613; raw 501/4873 | 91; 0.3723; raw 420/3797 | yes |

The harness's move-by-move product assertion passed. Every column now
prints the L (lead-in) entry kind; it is 0 in all controls.

### `geodesic_curv_lead`

| Item | Star | Pocket |
|---|---|---|
| Entries (= retracts) | 82 | 73 |
| S / L / J / M / C / W / P | 1 / **0** / 55 / 12 / 0 / 0 / 14 | 1 / **0** / 34 / 22 / 6 / 0 / 10 |
| Lead-ins line (verbatim) | `lobe bands 0 (fell back to the seam 28), closed wraps 0 (fell back 10), planned link length 0.0 mm, lead-in radius 0.0 to 0.0 mm` | `lobe bands 0 (fell back to the seam 26), closed wraps 0 (fell back 9), planned link length 0.0 mm, lead-in radius 0.0 to 0.0 mm` |
| Planned links | 0, 0.0 mm | 0, 0.0 mm |
| Emitted keep-down links | 98 (236.7 mm) — the same as `geodesic_curv` | 171 (387.2 mm); `geodesic_curv` 173 (382.7 mm) |
| Max emitted swept width | 0.3625, 0 over | 0.4677, 2 over (mop) |
| Field | 19 levels, 94 wrap pieces, 6 spiral levels | 32 levels, 95 wrap pieces, 4 spiral levels |
| Lobes | 6 / 34 arcs, 60 plain; split 51 (43 / 2 / 6), length ratio 8 | 5 / 31 arcs, 64 plain; split 53 (40 / 3 / 10), branch 2, length ratio 2, gap 2 |

Max swept width per class, emitted plan:

| Class | Star | Pocket |
|---|---|---|
| starter | 0.3114, 0 / 160 | 0.3114, 0 / 160 |
| clearing | 0.3625, 0 / 1662 | 0.3616, 0 / 1509 |
| clearing link | 0.1081, 0 / 22 | 0.3221, 0 / 25 |
| residue contour link | — | 0.2271, 0 / 20 |
| residue contour | — | 0.3333, 0 / 567 |
| wall pass link | 0.0000, 0 / 22 | 0.3439, 0 / 74 |
| wall pass | 0.1562, 0 / 75 | 0.3333, 0 / 21 |
| residue mop | 0.3581, 0 / 91 | **0.4677, 2 / 337** |
| residue mop link | 0.2500, 0 / 174 | 0.2395, 0 / 248 |

Raw plan (swept width max, over / steps): star spiral 0.4044, 22/348; join
0.8956, 100/124; wrap 0.9308, 33/1328; link 0.3768, 1/82. Pocket spiral
0.4211, 17/187; link 0.9571, 24/134; wrap 0.9300, 146/2094; join 0.9460,
85/109. No `lead` class appears in the raw plan.

Cascade (refused / cascade / intrinsic):

| Class | Star | Pocket |
|---|---|---|
| total | 135 / 20 / 115 (near-ceiling 17) | 887 / 663 / 224 (near-ceiling 70) |
| clearing spiral | 12 / 0 / 12 (0.4044) | 12 / 0 / 12 (0.4211) |
| clearing join | 94 / 0 / 94 (0.8956) | 102 / 17 / 85 (0.9460) |
| clearing wrap | 28 / 20 / 8 (0.9308) | 764 / 640 / 124 (0.9300) |
| clearing link | 1 / 0 / 1 (0.3768) | 9 / 6 / 3 (0.4080) |

The area-per-mm floor holds the reading bounded: emitted area-per-mm max
0.5885 (star) and 0.6094 (pocket), against 7.85 at D7. The raw wrap still
reads 1.1288 of D on the pocket (above 1 D, but no longer unbounded).

### C8's predictions

| Prediction | Measured | Held? |
|---|---|---|
| star 75 to 90 entries | 82 | in range, but with 0 lead-ins |
| pocket 60 to 80 entries | 73 | in range, but with 0 lead-ins |
| few lead-ins on the star | 0 on the star AND 0 on the pocket | the mechanism never applied |

Every lobe band and every closed wrap fell back to the seam (star 28 + 10,
pocket 26 + 9). The column is therefore `geodesic_curv` with the core spiral
(6 and 4 spiral levels) and no lead-in: J 55 against 58, M 12 against 0
(star); J 34, M 22 against 12 (pocket). The entry counts are in range
because of the spiral, not because of the lead-in.

### Magic numbers (C8 diff)

Product code: the floor `len.max(grid.cell_size)` (derived in its doc: the
lattice cannot resolve travel under one cell). The lead-in radius
`ρ_L` is derived per C8 (`lead_in_radius`); this pass read no new float
literal in the product hunks.

### `git status --short` at the end of D8

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M planning/adaptive_research_2026-10-23/P15_D_verify.md
```

### Top three for the lead

1. **The lead-in never applies.** `lead-ins piece 0: lobe bands 0` on both
   shapes; all 28 + 10 (star) and 26 + 9 (pocket) fall back to the seam,
   with planned link length 0.0 mm. Find the guard that rejects every
   candidate (the lead-in radius reads 0.0 to 0.0 mm, so no radius was ever
   accepted) before reading the column as evidence for or against the
   FreeCAD mechanism.
2. **C8's new test target did not compile** (`LobeMorph::Lead` missing in
   an older test's two matches). Fixed with a scope arm. C8's own two new
   tests pass; they test the lead-in geometry, not the harness fallback.
3. **The `geodesic_curv_lead` numbers (82, 73) come from the spiral.** They
   match the predicted ranges only because the core spiral replaces the
   inner circles. The pocket mop still steps over the ceiling (0.4677, 2
   steps).

## D9 at 609ce75b + uncommitted C9

C9 on top of 609ce75b: a re-seed of the geodesic field from the cleared
area of a replay model (`Reseed::Cleared`, `GeodesicOptions::C9_RESEED`),
and the split of `track_sources` into `sweep_sources`. Worktree
`.claude/worktrees/adaptive-head`, own target dir, the lane script and a
`timeout` on each command. Disk: 40 GB free at the start and at the end.

### Fix

None. Every command compiled and passed on C9's tree as delivered.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --features research,test-support` | green, no warning |
| 2 | `--features research,test-support --lib research_geodesic` | green 14 passed, 2 ignored (C9's `the_reseeded_field_grows_from_the_cleared_area` passes) |
| 2 | `--lib adaptive` | green 141 passed, 7 ignored |
| 2 | `--features test-support --test adaptive3d_emission_byte_parity` | green 5/5 (shipped path neutral) |
| 3 | `clippy -p rs_cam_core --all-targets -- -D warnings` | green |
| 3 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | green |
| 4 | the harness, release, `--ignored --nocapture` | green 1/1, 8.22 s; output in `phase2/d9_harness.txt` |

### Controls (exact)

| Column | Star | Pocket | Same |
|---|---|---|---|
| geodesic_a | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| geodesic_curv | 73 = S1 J58 M0 P14; 0.3581; raw 145/2545 | 63 = S1 J34 M12 C6 P10; 0.4677; raw 260/2952 | yes |
| agent | 25; 0.4515; raw 9/1749 | 31; 0.3618; raw 9/1805 | yes |
| contour_spiral | 101; 0.3613; raw 501/4873 | 91; 0.3723; raw 420/3797 | yes |

The `track_sources` split and the new option field change no control.

### `geodesic_curv_reseed`

| Item | Star | Pocket |
|---|---|---|
| Re-seed line (verbatim) | `re-seed piece 0: 0 re-seeds, 1 fields in use, stop Finished` | `re-seed piece 0: 47 re-seeds, 48 fields in use, stop Finished` |
| Entries (= retracts) | 73 | **74** |
| S / L / J / M / C / W / P | 1 / 0 / 58 / 0 / 0 / 0 / 14 | 1 / 0 / 63 / 0 / 0 / 0 / 10 |
| Max emitted swept width | 0.3581, 0 over (residue mop) | 0.3563, 0 over (clearing) |
| Residue mop max | 0.3581, 0 / 91 | 0.3484, 0 / 97 (`geodesic_curv`: 0.4677, 2 / 349) |
| Raw plan, over / steps | 145 / 2545 | 232 / 4135 |
| Cut length | as `geodesic_curv` | 1598.5 mm (`geodesic_curv` 1343.7 mm) |
| Keep-down links | as `geodesic_curv` | 143 (411.6 mm); `geodesic_curv` 173 (382.7 mm) |

Star, 0 re-seeds: the `.txt` equals `star_geodesic_curv.txt` except the
title, the new re-seed lines and the `joins piece 0` line (below). The
consistency rule of C9 ("0 re-seeds gives `geodesic_curv`'s numbers") holds.
The star's only field: 19 levels, 100 wraps placed, uncleared at the solve
1759.5 mm², stock cut (model) 2404.2 mm², trigger none.

Cascade (refused / cascade / intrinsic), raw plan:

| Class | Star reseed (= curv) | Pocket reseed | Pocket `geodesic_curv` |
|---|---|---|---|
| total | 128 / 20 / 108 | 106 / 20 / 86 | 876 / 663 / 213 |
| clearing join | 97 / 0 / 97 | 30 / 0 / 30 | 104 / 17 / 87 |
| clearing wrap | 30 / 20 / 10 | **43 / 18 / 25** | 763 / 640 / 123 |
| clearing link | 1 / 0 / 1 | 33 / 2 / 31 | 9 / 6 / 3 |

### Pocket re-seeds (48 fields)

| Field | Levels | Wraps placed | Uncleared at solve (mm²) | Stock cut, model (mm²) | Trigger |
|---|---|---|---|---|---|
| 0 | 32 | 9 | 2236.2 | 653.0 | level 9 at (27.50, 35.31) |
| 1 | 25 | 5 | 1778.8 | 270.0 | level 5 at (8.50, 43.97) |
| 2 | 23 | 1 | 1585.0 | 39.5 | level 1 at (34.03, 44.00) |
| 3 | 23 | 1 | 1554.5 | 11.5 | level 1 at (35.34, 43.50) |
| 4 | 23 | 5 | 1550.8 | 154.2 | level 3 at (36.50, 48.16) |
| 5 to 28 | 23 to 10 | 1 each | 1442.8 to 1024.2 | 7.5 to 61.0 | level 1 each |
| 29 | 9 | 4 | 997.2 | 56.0 | level 1 at (54.50, 14.56) |
| 30 | 9 | 3 | 964.5 | 39.0 | level 3 at (34.48, 15.50) |
| 31 | 9 | 2 | 946.5 | 65.0 | level 1 at (54.57, 13.50) |
| 32 to 44 | 9 to 3 | 1 each | 891.2 to 467.2 | 13.5 to 59.5 | level 1 each |
| 45 | 3 | 4 | 431.5 | 125.2 | level 1 at (71.44, 19.50) |
| 46 | 3 | 4 | 309.0 | 60.5 | level 1 at (32.50, 11.56) |
| 47 | 3 | 13 | 257.2 | 110.2 | none (stop Finished) |

Sum of the stock cut over the 48 fields: 2893.3 mm². 39 of the 48 fields
place one wrap only; 43 of the 47 triggers are on level 1. Fields 15 to 23
and 37 to 43 trigger at y 48.50, at x 53.84, 54.84, … 70.34: one re-seed
per 1 mm of advance along the same line. This is C9's "known limit" (the
end of a cut wrap is a tighter point than the law assumes), and it sets
most of the re-seed count. The full per-field lines are in
`pocket_geodesic_curv_reseed.txt` lines 61 to 109.

### Keep-or-drop (C9's pre-registered rule, applied as written)

| Rule | Limit | Measured | Holds |
|---|---|---|---|
| 1. pocket `clearing wrap` cascades ≤ half of `geodesic_curv` | ≤ 320 (640 / 2) | 18 | yes |
| 2. entries ≤ `geodesic_curv` on each shape | star ≤ 73, pocket ≤ 63 | star 73, pocket **74** | **no** (pocket +11) |
| 3. max emitted swept width ≤ 0.3626, mop excepted | 0.3626 | star 0.3581, pocket 0.3563; 0 over | yes |

**Verdict: DROP** (rule 2 fails on the pocket: 74 > 63). The pocket J count
rises from 34 to 63 while M falls from 12 to 0 and C from 6 to 0. The
cascade falls from 640 to 18 and the mop over-step (G-MOPLOAD) is gone on
the pocket; those are facts of the run, but the rule does not weigh them.

### Readout defect (not fixed)

`reseed_segments` builds its `FieldReport` with `joins:
JoinReport::default()` and never counts the seam joins. So the
`joins piece 0` line of the re-seed column reads 0 for every class, also on
the star where the moves equal `geodesic_curv` (16 / 33 / 50 there). Only
the readout is wrong; no move and no assertion depends on it.

### Magic numbers (C9 diff)

No bare float literal without a derivation in the product hunks:
`0.5 * SQRT_2 * cell` in `cleared_nodes` has its comment (half a cell
diagonal, the reach of `clear_circle` under a fringe); `2.0 * r` is the
diameter; the `0.0` values are initial values. Test code: the island
`(25, 20)`, radius 6 restates `island_pocket` (same values, checked); the
seed offset 17 has its comment; two `1e-9` float tolerances have no
derivation (comparison slack only).

### Files the harness wrote

73 files in
`.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`:
`summary.txt` and, for each of 12 columns × 2 shapes, `<shape>_<column>.{txt,svg,png}`.
New in this run: `star_geodesic_curv_reseed.{txt,svg,png}` and
`pocket_geodesic_curv_reseed.{txt,svg,png}`. Outside `target/`, the only
new file is `phase2/d9_harness.txt` (main checkout).

### `git status --short` (worktree) at the end of D9

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
```

## D10 at f576e51e + uncommitted C11

C11 on top of f576e51e (C9 committed): the split at the meeting line
(`Reseed::Zone`, `GeodesicOptions::C11_SPLIT`, column
`geodesic_curv_split`), `GeodesicField::grown_from` (`reseeded` calls it),
and the fix of the D9 join readout (`seam_joins`). Same worktree, own
target dir, the lane script and a `timeout` on each command. Disk: 40 GB
free at the start and at the end.

### Fix

None. Every command compiled and passed on C11's code as delivered.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --features research,test-support` | green, no warning |
| 2 | `--features research,test-support --lib research_geodesic` | green 15 passed, 2 ignored |
| 2 | `--lib the_far_side_zone_lies_behind_the_island` (alone) | green 1/1; the shadow assertion holds both ways, so the risk that C11 named did not occur |
| 2 | `--lib adaptive` | green 141 passed, 7 ignored |
| 2 | `--features test-support --test adaptive3d_emission_byte_parity` | green 5/5 (shipped path neutral) |
| 3 | `clippy -p rs_cam_core --all-targets -- -D warnings` | green |
| 3 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | green |
| 4 | the harness, release, `--ignored --nocapture` | green 1/1, 9.00 s; output in `phase2/d10_harness.txt` |

### Controls (exact)

| Column | Star | Pocket | Same |
|---|---|---|---|
| geodesic_a | 92 = S1 J56 M20 P15; 0.3600; raw 206/2330 | 88 = S1 J42 M27 C4 P14; 0.3624; raw 289/2845 | yes |
| geodesic_curv | 73 = S1 J58 M0 P14; 0.3581; raw 145/2545 | 63 = S1 J34 M12 C6 P10; 0.4677; raw 260/2952 | yes |
| geodesic_curv_reseed | 73 = S1 J58 M0 P14; 0 re-seeds | 74 = S1 J63 M0 C0 P10; 47 re-seeds, 48 fields | yes |
| agent | 25; 0.4515; raw 9/1749 | 31; 0.3618; raw 9/1805 | yes |
| contour_spiral | 101; 0.3613; raw 501/4873 | 91; 0.3723; raw 420/3797 | yes |

I compared every control `.txt` byte for byte against its D9 copy: all
twelve columns on both shapes (the five above and the seven older
`geodesic_*` variants). The only difference is the `joins piece 0` line of
`geodesic_curv_reseed`, which C11 fixed:

- star, D9: `closed-to-closed seam 0, closed to arc 0, from an arc 0`
- star, D10: `closed-to-closed seam 16, closed to arc 33, from an arc 50`
  (now equal to `star_geodesic_curv.txt`)
- pocket, D9: `closed-to-closed seam 0, closed to arc 0, from an arc 0`
- pocket, D10: `closed-to-closed seam 4, closed to arc 7, from an arc 76`

### `geodesic_curv_split`

Star: the zone is empty (`zone 0 nodes (0.0 mm²)`, `1 fields in use`). The
`.txt` equals `star_geodesic_curv.txt` except the title and the two new
`split` lines. Thus the star gives 73 = S1 J58 M0 P14, 0.3581, raw
145/2545, as predicted.

| Item | Pocket split | Pocket `geodesic_curv` |
|---|---|---|
| Fields in use | 2 | 1 |
| Zone | 3709 nodes, 927.2 mm² (prototype 959), 2 front branches, 60 meeting-line cells | — |
| First region | 32 levels, 44 wraps | 32 levels, 99 wrap pieces |
| Zone field | T = 0 on 4978 cleared nodes (1244.5 mm²); 18 levels, 35 wraps, 2 front branches, 64 meeting-line cells, 0 zone nodes not reached, converged true | — |
| Entries (= retracts) | **84** | 63 |
| S / L / J / M / C / W / P | 1 / 0 / 64 / 3 / 0 / 0 / 16 | 1 / 0 / 34 / 12 / 6 / 0 / 10 |
| Joins line | closed-to-closed seam 4, closed to arc 2, from an arc 72 | — |
| Max emitted swept width | 0.3609, 0 over (clearing) | 0.4677, 2 over (mop) |
| Residue mop max | 0.3597, 0 / 111 | 0.4677, 2 / 349 |
| Raw plan, over / steps | 230 / 2889 | 260 / 2952 |
| Cut length | 1173.8 mm | 1343.7 mm |
| Keep-down links | 161 (232.8 mm) | 173 (382.7 mm) |

Cascade (refused / cascade / intrinsic), raw plan:

| Class | Pocket split | Pocket `geodesic_curv` |
|---|---|---|
| total | 194 / 69 / 125 (near-ceiling 17) | 876 / 663 / 213 |
| clearing join | 102 / 8 / 94 (0.9426) | 104 / 17 / 87 |
| clearing wrap | **86 / 60 / 26** (0.8333) | 763 / 640 / 123 |
| clearing link | 6 / 1 / 5 (0.8972) | 9 / 6 / 3 |

C11's predictions: zone about 3800 nodes / 950 mm² (measured 3709 / 927.2);
2 branches (2); fields 2 (2); zone levels 16 to 18 (18); zone wraps 30 to 45
(35); first-region wraps 35 to 45 (44); wrap cascades 20 to 150 (60);
entries 45 to 70 (84: outside the range, above it); max width under the
ceiling (0.3609). All hold except the entries.

### Keep-or-drop (C11's pre-registered rule, applied as written)

| Rule | Limit | Measured | Holds |
|---|---|---|---|
| 1. pocket `clearing wrap` cascades ≤ half of `geodesic_curv` | ≤ 320 | 60 | yes |
| 2. entries ≤ `geodesic_curv` on each shape | star ≤ 73, pocket ≤ 63 | star 73, pocket **84** | **no** (pocket +21) |
| 3. max emitted swept width ≤ 0.3626, mop excepted | 0.3626 | star 0.3581, pocket 0.3609; 0 over | yes |

**Verdict: DROP** (rule 2 fails on the pocket: 84 > 63). The joins (J)
rise from 34 to 64 and the mop entries (P) from 10 to 16. The mid-wrap
splits (M) fall from 12 to 3 and the residue contours (C) from 6 to 0. The
cascade falls from 640 to 60 and the mop step over the ceiling is gone, but
the rule does not count these.

### Magic numbers (C11 diff)

No bare float literal without a derivation in the product hunks
(`pass.ceiling * 2.0 * r` is the ceiling width). Test code: the island
`(25, 20)` and the seed offset 17 restate C9's test position; `1e-9` is
comparison slack with no derivation; `let reach = s + (1.0 + SQRT_2) *
cell` restates the product formula in `geodesic_segments` (a copy, not a
call: if the product formula changes, the test does not follow).

### Files the harness wrote

79 files in
`.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`:
`summary.txt` and `<shape>_<column>.{txt,svg,png}` for 13 columns × 2
shapes. New in this run: `star_geodesic_curv_split.{txt,svg,png}` and
`pocket_geodesic_curv_split.{txt,svg,png}`. Outside `target/`, the only
new file is `phase2/d10_harness.txt` (main checkout).

### `git status --short` (worktree) at the end of D10

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
```

## D11 at ea468814 + uncommitted C12 and C14

Two independent uncommitted changes, verified together in one union run:

- C12: the research-gated 3D arm `ClearingStrategy3d::Geodesic`
  (`adaptive3d/{mod,path,clearing}.rs`, the C12 block in
  `research_geodesic.rs`, `Cargo.toml` `[[test]]`, the new test
  `tests/adaptive3d_geodesic_arm_c12.rs`).
- C14: the arc links `geodesic_curv_oneway` / `geodesic_curv_zigzag` with
  counted turns (`adaptive/path.rs` shipped file: `LinkLoad::turns`,
  `admits_turn`, the `_turns` cleanup variants; `research_geodesic.rs`; the
  2D harness).

Same worktree, own target dir, the lane script and a `timeout` on each
command. Disk: 40 GB free at the start, 39 GB at the end. The fixture runs
of C12 §8 were not run (the lead asks the operator first).

### Fix

None. I changed no file in the tree under test.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --all-targets` (no research) | green, no warning |
| 1 | `check -p rs_cam_core --all-targets --features research,test-support` | green, no warning |
| 2 | `--features research,test-support --test adaptive3d_geodesic_arm_c12 -- --nocapture` | **RED** (C12's load assertion; below) |
| 3 | `--features test-support --test adaptive3d_emission_byte_parity` | green 5/5 |
| 3 | `--features research,test-support --test adaptive3d_emission_byte_parity` (env var unset) | green 5/5 |
| 3 | the 13 adaptive3d sentries of `adaptive3d/CLAUDE.md`, `--features test-support` | green, all 13 |
| 4 | `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | red, the SAME red as master: `a_straight_plunge_entry_is_exempt_only_leaving_its_hole`, moves 2810 and 6159, sim 0.8329080445671909, exact 0.9403847504031052 (G-MOPLOAD) |
| 4 | `a_link_feeds_only_within_the_pass_load_g_adaptlinkload` | green |
| 4 | `adaptive_property_harness` | green |
| 4 | the other four `adaptive/CLAUDE.md` sentries (C14 changed `path.rs`, so I added them) | `adaptive_keeps_its_planner_order_g_adaptorder`, `contour_spiral_gcode_validity_phase0`, `adaptive_plans_every_inset_piece_g_adaptpieces` green. `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` red, the SAME red as master: `sim-only rapid flags moved`, move 6930, 0.24951132908296492 |
| 5 | `--lib adaptive` | green 143 passed, 7 ignored (C14's two `link_load_tests` included) |
| 5 | `--features research,test-support --lib adaptive::research_geodesic` | green 17 passed, 2 ignored |
| 5 | the same, `-- --ignored` | red 0/2, the SAME at HEAD ea468814 (below) |
| 6 | `clippy -p rs_cam_core --all-targets -- -D warnings` | green |
| 6 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | green |
| 6 | `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | green |
| 7 | the 2D harness, release, `--ignored --nocapture` | green 1/1, 14.29 s; output in `phase2/d11_harness.txt` |

**The two ignored kernel tests.** The two failures are:

- `the_transition_join_nests_its_sectors_on_a_disc`: `every transition is
  traced`, left 1, right 0. This is C6's known red, and its ignore reason
  names it.
- `every_emitted_step_holds_the_pass_load`: one `ResidueMop` step from
  (3.437, 35.053) to (3.606, 35.784) reads 0.45244 against 0.3626.

Both tests are `#[ignore]` with a reason. No D record ran them after D2.
The ignore reason of the second test still says 0.3669 (D2). To find the
owner, I ran the same command at HEAD ea468814: a detached worktree in the
session scratchpad with its own target dir, removed after the run. HEAD
fails the same two tests with the same 0.45244 mop step. So C12 and C14 do
not cause them. The mop reading grew from 0.3669 to 0.4524 at some commit
between D2 and ea468814. The ignore reason is now stale.

### C12: the 3D arm test (red)

`the_geodesic_arm_generates_and_holds_the_pass_load_at_two_levels` panics
at `adaptive3d_geodesic_arm_c12.rs:370`:

```
geodesic_curv: 128 judged samples read over the bound 0.546 (peak 1.000); the first: move 60 ClearingCut at (-16.04, -10.42, 7.00) wall distance 13.58: radial 0.709
```

The panic happens in the first mode, so the `geodesic_curv_split` mode and
the "the two toolpaths differ" assertion never run. The printed lines:

```
control AgentSearch: 657 moves, 3006 judged, 260 over 0.546, peak 0.928; entry 291, plunge hole 296, perimeter band 1212 (852 over)
geodesic_curv: 2058 moves, 6303 judged at levels {45 … 105} (Z x 10), 128 over 0.546, peak 1.000; entry 1858, plunge hole 3219, perimeter band 3040 (856 over)
```

The 128 over samples (115 distinct moves), grouped from the printed lines:

| Intent | Z | Wall distance | radial < 0.7 | 0.7 to < 1 | 1.000 |
|---|---|---|---|---|---|
| ClearingCut | 7.00 | ≥ 8 mm | 35 | 7 | 0 |
| ClearingCut | 7.00 | < 4 mm | 3 | 9 | 0 |
| ClearingCut | 4.50 | ≥ 8 mm | 36 | 3 | 0 |
| ClearingCut | 4.50 | < 4 mm | 3 | 1 | 0 |
| ClearingCut | other Z (4.6 to 5.0, draped) | < 4 mm | 6 | 3 | 0 |
| Linking | 7.00 | ≥ 8 mm | 3 | 2 | 7 |
| Linking | 4.75 | ≥ 8 mm | 1 | 3 | 6 |

The facts that the lead must weigh (I make no classification beyond them):

1. **The control fails the same bound.** AgentSearch, the shipped arm,
   reads 260 judged samples over 0.546 (peak 0.928). The test prints the
   control and does not assert on it. A bound that the shipped arm fails
   2:1 more often than the research arm does not separate the two arms.
2. **Most of the over samples are mid-pocket.** 81 of 128 are ClearingCut
   samples at ≥ 8 mm from the wall, on the two levels Z 7 and Z 4.5. They
   are not in the perimeter band and not the mop. C12's P5 expected only
   "a few" RDP slivers and a mop step. The run does not show which of the
   two causes these samples have; `print_exact_width_along_move` (the
   `adaptive/CLAUDE.md` rule) is the next read.
3. **13 Linking samples read 1.000** (a full-width cut), at Z 7.00 and at
   Z 4.75, all ≥ 8 mm from the wall. `links_as_cuts` writes a `Link` as a
   `Cut`, but these moves have intent `Linking`. A Z of 4.75 is 0.25 mm
   above the level Z 4.5.
4. The arm's judged samples lie at 61 Z values from 4.5 to 10.5. The
   control does not print its levels.

The bound was not changed. The test was not changed.

### Controls (exact)

All 26 control `.txt` files (13 columns × 2 shapes) that the harness wrote
again are byte-identical to the D10 copies. `summary.txt` differs only by
the four new column blocks. So `geodesic_a` 92/88, `geodesic_curv` 73 = S1
J58 M0 P14 / 63 = S1 J34 M12 C6 P10, `reseed` 73/74, `split` 73/84, agent
25/31, contour_spiral 101/91 are all unchanged. C14's threading changes no
control.

### C14 columns

| Item | star oneway | star zigzag | pocket oneway | pocket zigzag |
|---|---|---|---|---|
| Entries | **39** | **50** | 54 | 66 |
| S / L / J / M / C / W / P | 1/0/26/0/0/0/12 | 1/0/30/0/0/0/19 | 1/0/42/2/0/0/9 | 1/0/48/4/0/0/13 |
| Arc-to-arc joins | 44 | 44 | 51 | 51 |
| Turns planned / over the bound 3.207 / outside | 40 / 4 / 0 | 40 / 4 / 0 | 41 / 10 / 0 | 43 / 8 / 0 |
| Turn length (mm), min … max | 1.745 … 2.727 | 0.643 … 2.727 | 1.750 … 3.011 | 0.284 … 3.203 |
| Turn steps, over the ceiling | 80, 73 | 78, 73 | 83, 65 | 81, 65 |
| Turns fed / without | 40 / **0** | 39 / **1** | 41 / **0** | 42 / **1** |
| Turn swept/D: min, Q1/Q2/Q3, max | 0.056, 0.658/0.712/0.771, 0.830 | 0.000, 0.663/0.729/0.786, 0.896 | 0.000, 0.617/0.716/0.857, 0.946 | 0.000, 0.500/0.696/0.847, 0.946 |
| Turn area/D: min, Q1/Q2/Q3, max | 0.122, 0.687/0.753/0.860, 1.000 | 0.075, 0.708/0.800/0.860, 0.961 | 0.000, 0.616/0.825/0.941, 1.032 | 0.000, 0.452/0.740/0.923, 1.085 |
| Feed factor: min, Q1/Q2/Q3, max | 0.363, 0.418/0.474/0.516, 1.000 | 0.377, 0.415/0.446/0.498, 1.000 | 0.351, 0.386/0.439/0.546, 1.000 | 0.334, 0.388/0.474/0.610, 1.000 |
| Steps on no turn: max, over | 0.3586, 0 | 0.3589, 0 | 0.3624, 0 | 0.3610, 0 |
| Return links | 876 (338.7 mm) | 0 | 1466 (593.4 mm) | 0 |
| Keep-down links | 928 (588.7 mm) | 82 (394.3 mm) | 1585 (865.1 mm) | 30 (185.5 mm) |
| Cut length (mm) | 1192.5 | 1177.2 | 1321.6 | 1212.9 |
| Raw plan, over / steps | 90 / 3484 | 80 / 2559 | 175 / 4525 | 193 / 2971 |

For comparison, `geodesic_curv` has 73 / 63 entries and a cut length of
1343.7 mm on the pocket.

Cascade (refused / cascade / intrinsic), raw plan:

| Class | star oneway | star zigzag | pocket oneway | pocket zigzag |
|---|---|---|---|---|
| total | 70 / 17 / 53 | 68 / 16 / 52 | 98 / 22 / 76 | 319 / 185 / 134 |
| clearing join | 24 / 0 / 24 | 26 / 0 / 26 | 12 / 0 / 12 | 21 / 1 / 20 |
| clearing wrap | 36 / 11 / 25 | 38 / 13 / 25 | 64 / 11 / 53 | 287 / 182 / 105 |
| clearing link | 10 / 6 / 4 | 4 / 3 / 1 | 22 / 11 / 11 | 11 / 2 / 9 |

No turn is refused in the cascade model (the `clearing turn` class has no
refusal row).

**The unfed turn of each zigzag shape.** `apply_turn_feeds` counts a turn
as unfed when no emitted `Linear` move has both its start and its target
on the turn segment (within 1e-6 mm). The readout does not name the unfed
turn. The turn lines with factor 1.0000 are under the ceiling (they keep
the full feed by design) and cannot be told apart from the unfed turn. The
cause is not measured. A candidate: the emission drops collinear points
(`adaptive/CLAUDE.md`), so a turn that is collinear with the step before or
after it can become part of a longer move whose start is off the turn
segment. The readout should print which turn is unfed.

### Keep-or-drop (C14's pre-registered rule, applied as written)

| Rule | oneway | zigzag |
|---|---|---|
| 1. star entries ≤ 36 | **39: no** | **50: no** |
| 2. no non-turn step over 0.3626 (mop excepted) | star 0.3586, pocket 0.3624, 0 over: yes | star 0.3589, pocket 0.3610, 0 over: yes |
| 3. every turn ≤ its bound and fed | max 2.727 / 3.011 ≤ 3.207, without 0 / 0: yes | max 2.727 / 3.203 ≤ 3.207, but without **1 / 1: no** |
| **Verdict** | **DROP** (rule 1) | **DROP** (rules 1 and 3) |

Facts that the rule does not weigh: on the star, one-way takes 39 entries
against 73 (−34) and zigzag 50 (−23). On the pocket the counts are 54 and
66 against 63. No non-turn step goes over the ceiling, and the mop holds
the ceiling on all four plans.

### Magic numbers (C12 + C14 diff)

- Product hunks: the same-point tolerance `1e-6` (stated as the walker's
  own); a midpoint `0.5 * (a + b)`. No underived constant.
- C14 unit tests: `1e-12` / `1e-9` comparison slack; the scene points
  (±15, 0, 2, 4) are test inputs.
- C12 test, copied values:
  - `const PERIMETER_INSET_MARGIN_MM: f64 = 0.25` is a local copy of the
    shipped constant, not an import.
  - `(TOOL_RADIUS / 6.0).max(TOLERANCE)` restates the planner cell formula.
  - `quantum = 1.5 * planner_cell()` (the band margin) is not derived in
    the test.
  - `trochoid_cap_mult: 1.6`, `entry_clearance_mm: 0.5` and
    `stay_down_clearance_mm: 0.5` are parameter inputs.

### Fixture runs (C12 §8): not run; predicted duration

I found no record of a CLI `project` run time for these files. The only
generate times on record are GUI times: Corne roughs under 6 s each and
the wanaka200 back rough about 20 s (`P15_E_evidence.md` E2). So these are
estimates, not measurements:

| Item | Predicted | Basis |
|---|---|---|
| Release build of `rs_cam_cli` with `rs_cam_core/research` (once) | 3 to 8 min | core is rebuilt in release without `test-support`, then the CLI |
| Corne, per run (3 runs) | under 3 min each | 2 adaptive3d roughs at under 6 s each, plus the other ops of the file and the `--summary` sim |
| wanaka200, per run (3 runs) | 1 to 3 min each | back rough about 20 s, plus the other ops and the sim |
| rivmap100, per run (3 runs) | unknown; may exceed 3 min | no generate time on record; the file also holds a Scallop Finish and other ops |

To bound the runs, use `--skip` on the non-rough ops, if the lead accepts
that the run then measures the rough alone.

### Files the harness wrote

91 files in
`.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`:
`summary.txt` and `<shape>_<column>.{txt,svg,png}` for 15 columns × 2
shapes. New: `{star,pocket}_geodesic_curv_oneway.{txt,svg,png}` and
`{star,pocket}_geodesic_curv_zigzag.{txt,svg,png}`. Outside `target/`, the
only new file is `phase2/d11_harness.txt` (main checkout).

### `git status --short` (worktree) at the end of D11

```
 M crates/rs_cam_core/Cargo.toml
 M crates/rs_cam_core/src/adaptive/path.rs
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M crates/rs_cam_core/src/adaptive3d/CLAUDE.md
 M crates/rs_cam_core/src/adaptive3d/clearing.rs
 M crates/rs_cam_core/src/adaptive3d/mod.rs
 M crates/rs_cam_core/src/adaptive3d/path.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
?? crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs
```

### D11 addendum: the C12 over samples, classified (lead's request)

**The two ignored kernel tests: the recorded reasons hold.**

- `the_transition_join_nests_its_sectors_on_a_disc` fails on `every
  transition is traced` (`transition_fallback: 1`). This is C6's recorded
  red (D6; the ignore reason names the unapplied edge transition).
- `every_emitted_step_holds_the_pass_load` fails on one `ResidueMop` step.
  This is G-MOPLOAD (D2). The reading is now 0.4524, not 0.3669, and HEAD
  ea468814 gives the same 0.4524.
- The plunge sentry (`a_straight_plunge_entry_is_exempt_only_leaving_its_hole`,
  moves 2810 and 6159) is the master-side red.

**The instrument.** A scratch test, `tests/zz_d11_c12_probe.rs`, gated on
`research` and `test-support`. It copied the C12 fixture, params, plan,
simulation and judge without change. For each over sample it added the
exact swept width of its move on the emitted path. That reading is
`exact_width_at` of `a_clearing_cut_holds_the_pass_load_g_adaptpassload`,
copied for `Linear` moves, at a 0.05 mm pitch. The probe also added the
move context and the distance to the last entry. I ran it once (487 s) and
deleted it from the crate.

- The source is `phase2/d11_c12_probe.rs.txt`.
- The output is `phase2/d11_c12_probe_output.txt`.
- The C12 test output is `phase2/d11_c12_test_output.txt`.

The probe gives the same counts as the C12 test: AgentSearch 260 and
`geodesic_curv` 128. It also measures `geodesic_curv_split`, which the
test never reaches: 121.

**The rule (applied in this order to each over sample):**

- **A:** a `Linking` move of zero XY length (a vertical lift).
- **B:** wall distance < 4 mm.
- **C:** one of the first cut moves after an `EntryPlunge` (only
  `ClearingCut` moves between them).
- **D:** the remainder.

| Class | geodesic_curv | geodesic_curv_split | AgentSearch control | Exact peak of the move |
|---|---|---|---|---|
| A: vertical lift after a cut (`Linking`, dz +3.0 / +3.5) | 22 | 19 | 0 | 0.000 on every one |
| B: wall band, wall distance 2.75 to 3.6 mm | 25 | 24 | 26 | 0.724 to 0.990 |
| C: the first steps out of a straight plunge, 3.0 to 3.6 mm from the plunge point | 34 | 27 | 10 | 0.655 to 0.998 |
| D: wraps mid-pocket, 8 to 15 mm from the walls | 47 | 51 | 224 | 0.558 to 0.794 (arm); 0.490 to 0.999 (control) |
| total | 128 | 121 | 260 | |

**The worst 10 samples of `geodesic_curv` by sim reading.** All ten are
class A:

| # | Move | Intent | Class | Sample (x, y) | Z | Wall (mm) | Sim radial | Exact peak | To last entry (mm) |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 150 | Linking | A | (-9.14, -15.38) | 7.25 | 8.62 | 1.000 | 0.000 | 3.36 |
| 2 | 204 | Linking | A | (20.38, -12.38) | 7.25 | 11.62 | 1.000 | 0.000 | 3.07 |
| 3 | 229 | Linking | A | (-24.95, 0.43) | 7.25 | 9.05 | 1.000 | 0.000 | 3.46 |
| 4 | 349 | Linking | A | (14.07, 11.12) | 7.25 | 12.88 | 1.000 | 0.000 | 3.23 |
| 5 | 356 | Linking | A | (19.05, 5.22) | 7.25 | 14.75 | 1.000 | 0.000 | 3.56 |
| 6 | 390 | Linking | A | (19.91, 14.12) | 7.25 | 9.88 | 1.000 | 0.000 | 3.48 |
| 7 | 728 | Linking | A | (6.42, 13.97) | 7.25 | 10.03 | 1.000 | 0.000 | 4.41 |
| 8 | 1226 | Linking | A | (-9.14, -15.38) | 4.75 | 8.62 | 1.000 | 0.000 | 3.36 |
| 9 | 1263 | Linking | A | (20.38, -12.38) | 4.75 | 11.62 | 1.000 | 0.000 | 3.07 |
| 10 | 1286 | Linking | A | (-24.95, 0.43) | 4.75 | 9.05 | 1.000 | 0.000 | 3.46 |

**The worst class D moves by exact reading** (each appears at both levels):

| Moves (Z 7 / Z 4.5) | Sample (x, y) | Wall (mm) | Sim radial | Exact peak | To last entry (mm) |
|---|---|---|---|---|---|
| 61 / 1146 | (-15.52, -9.11) | 12.99 | 0.649 | 0.794 | 4.70 |
| 67 / 1152 | (-19.63, -6.17) | 14.37 | 0.667 | 0.766 | 5.06 |
| 65 / 1150 | (-17.14, -6.21) | 13.23 | 0.671 | 0.765 | 5.64 |
| 66 / 1151 | (-18.26, -5.91) | 14.19 | 0.572 | 0.763 | 5.51 |
| 64 / 1149 | (-16.20, -6.93) | 12.62 | 0.585 | 0.742 | 5.55 |

**The verdict against the lead's four classes:**

- **(i) The oracle filter has gaps. Classes A, B and C: 81 of 128
  (geodesic_curv), 70 of 121 (split).**
  - A: the sim reads radial 1.000 on a vertical lift that the exact
    geometry reads 0.000. The lift is `Linking` intent, straight up from
    the last cut point (for example 149 → 150: (-9.14, -15.38), Z 7.00 →
    10.50). The test judges every cutting sample that is not an entry, so
    it judges these lifts. The sim measure is not defined on a vertical
    move.
  - B: the C12 note says that the perimeter band [5.5, 7.46] mm "also
    hides the 2D wall pass". It does not. These cuts run at wall distance
    2.75 to 3.6 mm (about R) along the outer pocket walls (for example
    move 998, x = -31.25, y −19 → 20, Z 7.50, draped over the 0.5 mm wall
    slope). The control has the same 26 samples. The exact reading counts
    the uncut wall slope as stock, so B is a wall-pass reading. The test
    says that it does not judge the wall pass.
  - C: the 2D exemption is per step. `emit_capped_walk_with` exempts a
    step whose START lies within R of the plunge point (`departing`,
    `adaptive/path.rs`). The test exempts a sample within R of the hole.
    The C samples lie 3.0 to 3.6 mm from the plunge, on the step out of
    the hole.
- **(ii) RDP slivers: at most 2 samples.** The cut RDP tolerance is 0.1 mm,
  so a sliver can add at most 0.1 / D = 0.017 of D. Only 2 of the 47
  class D samples read within 0.017 of the bound. The median excess is
  0.137 D (0.8 mm) and the max is 0.248 D (1.5 mm). RDP cannot make these.
- **(iii) 2D links emitted as 3D cuts through stock: none found.**
  - No over sample sits on a straight chain of planner-cell cut moves
    (the shape that `links_as_cuts` writes).
  - Every class D move is 0.6 to 4.5 mm long, on a curved wrap.
  - Every `Linking` over sample is a vertical lift with exact 0.000.
- **(iv) A real arm defect, or a defect in the shared 3D slice: class D,
  47 (geodesic_curv) / 51 (split).** The exact geometry reads 0.558 to
  0.794 mid-pocket.
  - The worst moves (57 to 70, and 1142 to 1155 at Z 4.5) are the arm's
    starter: a straight `EntryPlunge` at (-19.71, -11.23) (move 57), then
    a full-depth lap of radius 2.4 to 3.4 mm about (-18.05, -8.38). The
    lap meets standing stock on most of its length.
  - The 3D slice sets `entry_helix_radius: 0.0` (`clear_z_level_agent_2d_slice`,
    params_2d). The 2D harness plans the starter after a helix of
    `HELIX_RADIUS_OVER_D · D`, and there it reads 0.3114. A plunge-entered
    starter lap at full depth cannot hold the ceiling.
  - Unmeasured: does the 2D replay judge the starter's steps? The note
    (`research_geodesic.rs`, "Walk planned segments (the starter pocket)")
    suggests that the starter is planned segments.
  - The other D moves are wraps 4 to 18 moves after other entries, 73 of
    81 at x < 0 (the far side of the island from the seed region).
  - The shipped AgentSearch control has 224 class D samples (exact up to
    0.999). So the 3D slice path, not only the kernel, may hold part of
    this. The 3D AgentSearch never claimed the 2D pass load (`adaptive/CLAUDE.md`:
    "Adaptive3d keeps the historical rule").
  - The cause of the non-starter D samples is not isolated.

**Next reads (not done):**

1. The 2D segments of the first slice outline after the cleanup: the
   output of `research_geodesic::slice_segments` plus the
   `apply_contour_parallel_residue_cleanup` replay. Compare them with the
   3D XY moves of level Z 7.
2. The starter under `entry_helix_radius = 0`: does the replay judge it,
   or does it pass as an exempt or modelled entry?
3. A C12 filter that matches the 2D rules: per-step departure, vertical
   moves, and the wall pass at wall distance about R. Only after that
   does the bound say anything about class D.

## D12 at c3d38bf4 + uncommitted C16 and C18

Two independent uncommitted changes, verified together in one union run
(`phase2/d12_harness.txt` for the 2D harness):

- C16: `Spacing::LocalLevel`, `Chaining::Whole`, the columns
  `geodesic_local`, `geodesic_local_oneway` and `geodesic_curv_oneway_whole`,
  the `.c16.txt` readouts, and a pure split in the SHIPPED
  `finish/scallop_isofield.rs` (`iso_segments_where`).
- C18: the C12 oracle on the 2D rules (`tests/adaptive3d_geodesic_arm_c12.rs`)
  and the research-only disc air read in `adaptive3d/clearing.rs`.

I copied the D11 render dir to the session scratchpad before the run. Same
lane script, own target dir, a `timeout` on each command. Disk: 39 GB free
at the start and at the end.

### Fix

None. Two scratch probes (`tests/zz_d12_starter_probe.rs`) ran once and
were deleted. Their source and output are in `phase2/`.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --all-targets`, plain and `--features research,test-support` | green, no warning |
| 2 | `--features research,test-support --lib research_geodesic` | green 19 passed, 2 ignored (C16's two unit tests included) |
| 2 | `--lib scallop_isofield` | green 3/3 |
| 2 | `--lib adaptive` | green 143 passed, 7 ignored |
| 3 | iso-field sentries (shipped file changed): `scallop_iso_field_config`, `a_scallop_iso_field_link_clears_standing_stock_d1`, `finish_resolution_policy_pr3` | green 2/2, 2/2, 10/10 |
| 3 | `crease_own_region_pr6b` | red 2/3: `production_unified_finish_output_is_byte_identical` panics at :229, `Operation(NotSet { op: "Unified Finish", kind: Feeds, fields: ["feed_rate", "plunge_rate", "spindle_rpm"] })`. Generation stops at the feeds check before any generator runs, so the iso-field code is not reached. The test sets no feeds since 74af0f8e (S11, "the op refuses until it is set"). Master-side, not C16. As a result, this pin does not check `iso_contour` today |
| 3 | `scallop_isofield_gouge_m4` (237 s) and `scallop_candidates_m4` (143 s), heavy | NOT RUN (over 3 min; the operator's go-ahead is needed) |
| 4 | `adaptive3d_emission_byte_parity`, plain and with research | green 5/5, 5/5 |
| 4 | the 13 adaptive3d sentries | green, all 13 |
| 4 | `every_ignore_states_its_kind_f152` | green 2/2 |
| 5 | `--test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture` | **green 1/1** (427 s); output in `phase2/d12_c12_test_output.txt` |
| 6 | the 2D harness, release | green 1/1, 11.42 s |
| 7 | clippy: core plain, core research, workspace full lint | green, green, green |
| 8 | `phase2/c18/assert_c18.py` | `C18: all claims hold` |

### Controls (exact, against D11)

All 30 control `.txt` files that the harness wrote again (15 columns × 2
shapes) are byte-identical to the D11 copies. `summary.txt` differs only by
the six new blocks (three columns × two shapes) and loses no line. The
`.c16.txt` files are new.

### C16: the new columns

| Item | star local | star local_oneway | star oneway_whole | pocket local | pocket local_oneway | pocket oneway_whole |
|---|---|---|---|---|---|---|
| Entries | 78 | **34** | 30 | 68 | **41** | 46 |
| S / L / J / M / C / W / P | 1/0/57/0/0/0/20 | 1/0/19/0/0/0/14 | 1/0/17/0/0/0/12 | 1/0/40/14/0/0/13 | 1/0/29/0/0/0/11 | 1/0/32/3/0/0/10 |
| Levels | 19 | 19 | 19 | 36 | 36 | 32 |
| Turns planned / over the bound | — | 45 / 3 | 46 / 3 | — | 50 / 16 | 44 / 14 |
| Turn length (mm) | — | 2.058 to 2.960 | 2.046 to 3.192 | — | 0.048 to 2.987 | 0.035 to 2.967 |
| Turns fed / without | — | 45 / 0 | 46 / 0 | — | 49 / **1** | 43 / **1** |
| Steps on no turn: max, over | 0.3497, 0 | **0.4638, 1** (residue mop) | 0.3573, 0 | 0.3598, 0 | 0.3574, 0 | 0.3624, 0 |
| Clearing-wrap cascades | 14 | 10 | 4 | 520 | 5 | 9 |
| Keep-down links | 85 (309.6 mm) | 1129 (967.4 mm) | 1156 (572.4 mm) | 114 (272.6 mm) | 2029 (975.3 mm) | 1717 (993.3 mm) |
| Cut length (mm) | 1007.1 | 1193.4 | 1195.5 | 1322.8 | 1488.2 | 1321.8 |

C16's entry predictions: `geodesic_local` 68 to 73 / 55 to 65 (measured 78 /
68: above both); `geodesic_local_oneway` 27 to 33 / 42 to 52 (34 / 41: both
just outside); `geodesic_curv_oneway_whole` 27 to 33 / 46 to 54 (30 / 46:
both in).

**The star non-turn step over the ceiling** (`geodesic_local_oneway`): the
only `over:` line, `residue mop cut (segment 1202) from (28.17, 81.05) to
(27.53, 80.66), reading 0.4638`, class `residue mop` (1 of 157 mop steps).
That is the G-MOPLOAD class, which rule (2) excepts. Every clearing class
holds (clearing 0.3565, clearing link 0.1633, 0 over).

**Levels (`.c16.txt`).** Star `LocalLevel`: 20 values, steps 1.375 →
2.000 mm (C5: 1.437 → 2.000). Pocket `LocalLevel`: 37 values. The steps
rise to 1.938, then fall to 1.438 between about 19.5 and 39.8 mm from the
rim (the far side's radius sets the whole level), then rise to 2.000.
`geodesic_curv`: 33 values, steps 1.437 → 2.000 by level 11.

**The arm to (5, 35)** (all entry rings in x < 40, y < 47, read from the
SVG renders at 8.52 px/mm; the mop included):

| Column | Entries | In the arm | On the seam ray y = 50.5, left of the seed |
|---|---|---|---|
| geodesic_curv_oneway | 39 | 12 | 4 |
| geodesic_curv_oneway_whole | 30 | 3 | 4 |
| geodesic_local_oneway | 34 | 3 | 4 |

`Chaining::Whole` takes 9 entries out of the arm. That is the whole star
difference between `oneway` (39) and `oneway_whole` (30).

**Unfed turns (`.c16.txt`, `unfed turn:`).**

| Column | Unfed turn | Length | Matches C16 |
|---|---|---|---|
| star zigzag | (36.4094, 39.5000) → (36.0045, 40.0000) | 0.643 mm, swept 0.3147 | yes, the first star candidate |
| pocket zigzag | (42.0063, 49.0000) → (42.4098, 48.5000) | 0.642 mm, swept 0.4312 | yes, the last pocket candidate |
| pocket local_oneway | (27.5284, 25.0000) → (27.5000, 25.0387) | 0.048 mm, swept 0.0000 | a NEW kind: a turn of under one tenth of a cell |
| pocket oneway_whole | (27.0230, 26.0000) → (27.0000, 26.0260) | 0.035 mm, swept 0.0000 | the same new kind |

The two new unfed turns are 35 to 48 µm long and engage nothing (swept 0).
They are not the chaining gap of task 3. Both sit near (27, 25.5) on the
pocket, by the island's lower-left side. A turn that short can lie within
the emitter's collinear-point tolerance of its neighbours, the same drop
mechanism as task 3. That cause is not measured.

**Slanted-wall turns (task 4).** The `.c16.txt` gives one line per join
over 3.207 mm: the length, θ, and the slanted bound s / cos θ + (1 + √2)·cell.

| Shape / column | Join (from → to) | Length mm | θ deg | Slanted bound mm | Fits |
|---|---|---|---|---|---|
| star curv_oneway | (32.67, 53.50) → (47.68, 67.50) | 20.526 | 55.6 | 4.751 | no |
| star curv_oneway | (29.00, 81.37) → (62.59, 61.50) | 39.025 | 73.2 | 8.121 | no |
| star curv_oneway | (71.92, 80.00) → (61.50, 38.32) | 42.963 | 61.1 | 5.348 | no |
| star curv_oneway | (49.00, 17.45) → (36.41, 39.50) | 25.391 | 77.1 | 10.194 | no |
| pocket curv_oneway | (36.16, 43.50) → (9.50, 50.30) | 27.513 | 60.6 | 5.280 | no |
| pocket curv_oneway | (9.50, 50.30) → (16.63, 51.50) | 7.234 | 85.0 | 24.265 | yes |
| pocket curv_oneway | (16.63, 51.50) → (37.53, 45.00) | 21.882 | 62.0 | 5.461 | no |
| pocket curv_oneway | (44.52, 49.00) → (48.00, 40.84) | 8.877 | 49.9 | 4.311 | no |
| pocket curv_oneway | (53.50, 38.43) → (23.00, 8.08) | 43.027 | 72.4 | 7.827 | no |
| pocket curv_oneway | (23.00, 8.08) → (27.00, 8.35) | 4.010 | 50.7 | 4.366 | yes |
| pocket curv_oneway | (64.50, 8.60) → (67.00, 11.16) | 3.574 | 53.4 | 4.558 | yes |
| pocket curv_oneway | (69.00, 11.00) → (67.13, 32.00) | 21.080 | 60.4 | 5.257 | no |
| pocket curv_oneway | (71.20, 45.50) → (13.50, 8.31) | 68.647 | 33.1 | 3.594 | no |
| pocket curv_oneway | (13.50, 8.31) → (9.98, 11.00) | 4.434 | 82.6 | 16.681 | yes |
| star local_oneway | (49.50, 16.00) → (60.55, 38.00) | 24.621 | 70.8 | 7.284 | no |
| star local_oneway | (83.79, 38.00) → (65.00, 56.68) | 26.496 | 69.1 | 6.818 | no |
| star local_oneway | (65.00, 56.68) → (65.50, 60.03) | 3.392 | 48.3 | 4.216 | yes |

Summary per column: fits / joins over the bound.
- star `curv_oneway`: 0 / 4. Pocket `curv_oneway`: 4 / 10.
- star `local_oneway`: 1 / 3. Pocket `local_oneway`: 3 / 16 (lines in its `.c16.txt`).
- star `curv_oneway_whole`: 0 / 3. Pocket `curv_oneway_whole`: 6 / 14.

Every join that fits is 3.4 to 7.2 mm long. Every join that does not fit
is 6.3 to 68.6 mm long. Most of those are jumps between arms or across
the island, not wall turns.

### C16 keep-or-drop (`geodesic_local_oneway`, the rule as written)

| Rule | Limit | Measured | Holds |
|---|---|---|---|
| 1. entries < `geodesic_curv_oneway` | star < 39, pocket < 54 | star 34, pocket 41 | yes |
| 2. no non-turn step over 0.3626, mop excepted | 0 | star 1 over (residue mop, 0.4638: excepted); pocket 0 | yes |
| 3. every turn ≤ its bound and fed | `without 0` | star 0; pocket **1** (0.048 mm turn, swept 0) | **no** |
| 4. pocket C = 0 and wrap cascades ≤ 11 | C 0, ≤ 11 | C 0, 5 | yes |
| **Verdict** | | | **DROP** (rule 3, pocket) |

The rule fails on one turn of 0.048 mm that engages nothing (swept 0, area
0.0833). It holds on every other count. `geodesic_curv_oneway_whole` (not
under the rule) has the same failure kind on the pocket.

### C18: the C12 oracle on the 2D rules

The C12 test is green when run by name (`-- --ignored`). Its summary lines
(the counts are samples):

| Item | AgentSearch control | geodesic_curv | geodesic_curv_split |
|---|---|---|---|
| moves | 657 | 2229 (D11: 2058) | 2228 (D11: 1941) |
| judged | 1295 | 2484 | 2902 |
| levels judged (Z × 10) | 45, 70, 75, 80, 105 | 45, 50, 70, 75, 80, 105 | 45, 70, 75, 80, 105 |
| over the screen 0.546 (sim peak) | 225 (0.928) | **0** (0.518) | **0** (0.441) |
| over the exact bound 0.471 (exact peak) | 225 (0.999) | **0** (0.000) | **0** (0.000) |
| entry (not judged) | 291 | 1469 | 1064 |
| vertical (not judged) | 250 | 1695 | 1259 |
| departing (not judged) | 798 | **5770** | **4488** |
| waterline (not judged): over the screen, exact peak | 1301: 28, 0.990 | 1301: 27, 0.990 | 1301: 26, 0.990 |
| perimeter band (not judged): over the screen | 870: 632 | 1665: 632 | 2029: 633 |

Per D11 class, `geodesic_curv` (D11 → D12):
- A, vertical lifts: 22 over → 0 judged (in `vertical`).
- B, wall band: 25 over → 0 judged (the waterline tag; 27 over the screen
  there, exact peak 0.990).
- C, plunge departure: 34 over → 0 judged (in `departing`).
- (iv), all wraps: 47 over → 0 over the screen.

The two modes have 0 samples over the exact bound 0.471, and 0 over the
screen. So no judged sample needed an exact reading.

**The starter, before and after** (scratch probe on the C18 path:
`phase2/d12_starter_probe.rs.txt`, `phase2/d12_starter_probe_output.txt`):

| | D11 (before C18) | D12 (after C18) |
|---|---|---|
| Plunges | one at the seed, then a demoted air run, then a SECOND plunge at (-19.71, -11.23) (move 57) | ONE plunge at the seed (-18.45, -8.88) (move 52; 1222 at Z 4.5); the next plunge within 5 mm is move 113, after the spiral ends |
| The lap | moves 58 to 70 judged, exact 0.739 to 0.794 | one continuous spiral of 56 cut moves (53 to 108) |
| Moves that start within R of the plunge | — | 45; exact max 0.999 at the first move; 13 over 0.471 (moves 53 to 66 at most, 0.999 → 0.473) |
| Moves that start at R or more | the lap, 0.74 to 0.79 | 11; exact max 0.280 |

The same numbers hold in both modes and at both levels. The 13 starter
moves over 0.471 all start within R of the plunge. They are the
plunge-departure class, which the 2D rule exempts. The C18 filter exempts
them as `departing`.

**Two limits for the un-ignore decision.** The ignore reason says
"Un-ignore when the verifier reads 0 samples over the exact bound in both
modes". That condition is now met, with two limits:

1. **The `departing` count is large.** It is 5770 samples on
   `geodesic_curv` against 2484 judged. The C18 rule exempts every move
   whose start lies within R of the last vertical plunge, until the next
   entry. The 2D walker exempts only the steps of the first path after the
   plunge (`plunged_at.take()`, `adaptive/path.rs`). A later move of the
   same 3D run that comes back within R of the plunge is exempt under C18
   but judged in 2D. The run does not show how many samples that is.
2. **A sample under the screen is not read exactly.** A sample at 0.546 or
   less is never judged against 0.471 (C18 §2, "Limit").

### Magic numbers (C16 + C18 diff)

I did not scan the diff in this run.

### Files

- Renders: `.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
  - New: `<shape>_{geodesic_local,geodesic_local_oneway,geodesic_curv_oneway_whole}.{txt,svg,png}`.
  - New: `<shape>_<column>.c16.txt` for every candidate (a) column.
- New in `phase2/`: `d12_harness.txt`, `d12_c12_test_output.txt`,
  `d12_starter_probe.rs.txt`, `d12_starter_probe_output.txt`.

### `git status --short` (worktree) at the end of D12

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M crates/rs_cam_core/src/adaptive3d/clearing.rs
 M crates/rs_cam_core/src/finish/scallop_isofield.rs
 M crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
```

### D12 addendum: the two heavy iso-scallop tests (operator approved)

The aim: to confirm that C16's split of `iso_contour_where`
(`iso_segments_where`) leaves the shipped iso-scallop output unchanged.
The byte pin `crease_own_region_pr6b` cannot show this today (G-CREASEPIN:
it stops at the feeds check).

- Commit: c60ed442 (C16 and C18 committed). `crates/rs_cam_core/src/finish/`
  has no uncommitted change. C20 started to edit `research_geodesic.rs` and
  the 2D harness during the run. Neither one enters this build:
  `adaptive::research_geodesic` is `#[cfg(feature = "research")]`, and the
  harness requires `research`.
- Features: `heavy-tests,test-support` only (their `[[test]]`
  `required-features`), no `research`. Lane script, `timeout 900` each.

| Test | Result | Wall time (build included) | Test time |
|---|---|---|---|
| `scallop_isofield_gouge_m4` | green, 2 passed, 3 ignored (instruments / evidence) | 222 s | 198.10 s |
| `scallop_candidates_m4` | green, 3 passed, 4 ignored (evidence) | 139 s | 117.95 s |

These are the shipped tests of the iso-field scallop (gouge, chord
refinement, candidates). They pass at c60ed442. This is evidence that
C16's split changes no shipped iso-scallop output that these tests cover.

## D13 at c60ed442 + uncommitted C20

C20 is uncommitted in three files:

- `research_geodesic.rs`: `full_feed_unmatched`, `HoldingMove`, and the
  preset `C20_LOCAL_ZIGZAG`.
- The 2D harness: the column `geodesic_local_zigzag` and the holding-move
  lines.
- The C12 test: `departing_moves` (the 2D departure rule), and an ignore of
  kind `heavy:`.

Before the run, I copied the D12 render dir to the session scratchpad. Same
lane script, own target dir, a `timeout` on each command. C20's diff did
not change during the run. Disk: 38 GB free.

### Fix (uncommitted, test code)

- `research_geodesic.rs:8438`, C20's new unit test
  `a_turn_whose_vertex_the_emitter_drops_is_unfed_only_when_it_needs_a_cut_feed`.
  Clippy `manual_let_else` (`-D clippy::manual-let-else`) failed both
  research clippy configs (`could not compile rs_cam_core (lib test)`).
  - Before: `let (turn, after) = match fed.as_slice() { &[_, _, turn, after] => (turn, after), _ => panic!("four moves: {fed:?}"), };`
  - After: `let &[_, _, turn, after] = fed.as_slice() else { panic!("four moves: {fed:?}") };`
  - Why: this is the form that clippy suggests. The same binding and the
    same panic message. No assertion changes.
  - After the fix: fmt is green, both research clippy configs are green,
    and the test passes (1/1). The fix is in test code only, so the
    harness and the C12 numbers below stay valid.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green (also after the fix) |
| 1 | `check -p rs_cam_core --all-targets --features research,test-support` | green |
| 2 | `--features research,test-support --lib research_geodesic` | green 20 passed, 2 ignored |
| 3 | `adaptive3d_emission_byte_parity`, plain and with research | green 5/5, 5/5 |
| 4 | `--test adaptive3d_geodesic_arm_c12` (no `--ignored`) | 0 passed, 1 ignored, as C20 states |
| 4 | the same with `-- --ignored --nocapture` | **green 1/1**, 461.53 s (486 s with the build); output in `phase2/d13_c12_test_output.txt` |
| 5 | `every_ignore_states_its_kind_f152` | green 2/2 (the `heavy:` reason is accepted) |
| 6 | the 2D harness, release | green 1/1, 11.98 s; output in `phase2/d13_harness.txt` |
| 7 | `clippy -p rs_cam_core --all-targets -- -D warnings` | green |
| 7 | `clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings` | red (`manual_let_else`) → green after the fix |
| 7 | `clippy --workspace ... heavy-tests,research,test-support -- -D warnings` | red (the same) → green after the fix |

### Byte compare against D12

I compared 141 render files (`.txt`, `.svg`, `.png`, `.c16.txt`) that the
harness wrote again. Exactly C20's predicted 11 files differ:

- `pocket_geodesic_local_oneway.{txt,svg,c16.txt}`
- `pocket_geodesic_curv_oneway_whole.{txt,svg,c16.txt}`
- `star_geodesic_curv_zigzag.{txt,svg,c16.txt}`
- `pocket_geodesic_curv_zigzag.c16.txt`
- `summary.txt`

Every `.png` is byte-equal. In each `.txt` the one changed line is
`clearing turns (emitted): ... without 1` → `... without 0`. In each `.svg`
only that caption line changes: 0 non-text lines differ. `summary.txt`
loses only those three `without 1` lines and gains the two
`geodesic_local_zigzag` blocks. Every other control file is byte-equal.

### C20's cause: the holding move of each turn with no move on it

Each such turn ends within 1 µm (the emitter's `MIN_EMITTED_SEGMENT_MM`) of
the emitted move that holds it. That confirms the collinear drop.

| Column | Turn | Length | Engagement (swept / area) | Holding move | Turn ends off the move (mm) | Kind |
|---|---|---|---|---|---|---|
| pocket local_oneway | (27.5284, 25.0000) → (27.5000, 25.0387) | 0.048 | 0.0000 / 0.0833 | (27.5284, 25.0000) → (27.1455, 25.5000) | 0 and 9.932e-4 | full feed |
| pocket curv_oneway_whole | (27.0230, 26.0000) → (27.0000, 26.0260) | 0.035 | 0.0000 / 0.0833 | (27.0230, 26.0000) → (26.5551, 26.5000) | 0 and 9.190e-4 | full feed |
| star curv_zigzag | (36.4094, 39.5000) → (36.0045, 40.0000) | 0.643 | 0.3147 / 0.3441 | (36.4094, 39.5000) → (36.0000, 40.0058) | 0 and 1.717e-4 | full feed |
| pocket curv_zigzag | (42.0063, 49.0000) → (42.4098, 48.5000) | 0.642 | 0.4312 / 0.5067 | (42.0000, 49.0075) → (42.4098, 48.5000) | 2.202e-4 and 0 | **unfed** (needs factor < 1) |
| pocket local_zigzag | (29.0600, 22.0000) → (29.0000, 22.1696) | 0.180 | 0.0786 / 0.1927 | (29.0600, 22.0000) → (28.8807, 22.5000) | 0 and 7.731e-4 | full feed |
| pocket local_zigzag | (27.5284, 25.0000) → (27.5000, 25.0387) | 0.048 | 0.0000 / 0.0833 | (27.5284, 25.0000) → (27.1455, 25.5000) | 0 and 9.932e-4 | full feed |
| pocket local_zigzag | (56.0000, 38.3673) → (56.4055, 40.9436) | 2.608 (2 steps) | 0.2165 / 0.2916 | (56.0000, 38.3673) → (56.4153, 41.0000) | 0 and 9.542e-4 | full feed |

The largest distance is 9.932e-4 mm, 7 nm under the 1 µm drop tolerance.
Each full-feed turn reads under the ceiling 0.3626, so its feed factor is
1.

`unfed` / `full_feed_unmatched` per column (every other candidate (a) column
is 0 / 0):

| Column | star | pocket |
|---|---|---|
| geodesic_curv_zigzag | 0 / 1 | **1** / 0 |
| geodesic_curv_oneway_whole | 0 / 0 | 0 / 1 |
| geodesic_local_oneway | 0 / 0 | 0 / 1 |
| geodesic_local_zigzag | 0 / 0 | 0 / 3 |

The pocket `geodesic_curv_zigzag` control stays `without 1`: its gap turn
needs factor 0.7158, and the emitter dropped its vertex. That is C20's
stated limit.

### The rule on `geodesic_local_oneway` and `geodesic_local_zigzag`

| Item | local_oneway star | local_oneway pocket | local_zigzag star | local_zigzag pocket |
|---|---|---|---|---|
| Entries | 34 | 41 | 38 | 44 |
| S / L / J / M / C / W / P | 1/0/19/0/0/0/14 | 1/0/29/0/0/0/11 | 1/0/23/0/0/0/14 | 1/0/30/0/0/0/13 |
| Turns emitted of planned; over the bound | 45 of 45; 3 | 50 of 50; 16 | **45 of 46**; 2 | 59 of 59; 7 |
| Turn length (mm) | 2.058 to 2.960 | 0.048 to 2.987 | 2.059 to 2.751 | 0.048 to 2.987 |
| Fed / without | 45 / 0 | 49 / 0 (1 full feed) | 45 / 0 | 56 / 0 (3 full feed) |
| Turn feed factor Q1 / Q2 / Q3 | 0.414 / 0.448 / 0.487 | 0.371 / 0.401 / 0.458 | 0.424 / 0.445 / 0.481 | 0.389 / 0.429 / 0.516 |
| Steps on no turn: max, over | 0.4638, 1 (residue mop, segment 1202) | 0.3574, 0 | 0.4509, 1 (residue mop, segment 163: (71.77, 80.74) → (71.28, 81.30)) | 0.3625, 0 |
| Clearing-wrap cascades | 10 | 5 | 7 | 10 |
| Return links | 1037 (408.8 mm) | 1954 (790.1 mm) | 0 | 0 |
| Keep-down links | 1129 (967.4 mm) | 2029 (975.3 mm) | 59 (251.2 mm) | 111 (211.0 mm) |
| Cut length (mm) | 1193.4 | 1488.2 | 1173.0 | 1443.1 |

| Rule | local_oneway | local_zigzag |
|---|---|---|
| 1. entries < 39 star, < 54 pocket | 34, 41: yes | 38, 44: yes |
| 2. no non-turn step over 0.3626, mop excepted | the only over is a residue mop step (0.4638): yes | the only over is a residue mop step (0.4509): yes |
| 3. every turn ≤ 3.207 mm and `without 0` | max 2.960 / 2.987; 0 / 0: yes | max 2.751 / 2.987; 0 / 0: yes |
| 4. pocket C = 0, wrap cascades ≤ 11 | C 0, 5: yes | C 0, 10: yes |
| **Verdict** | **KEEP** | **KEEP** |

Notes for the lead:

- Star `local_zigzag` emits 45 of 46 planned turns. The readout does not
  say where the 46th turn went. The rule does not read this count.
- C20's default criterion, read from the run:
  - Entries: zigzag has +4 on the star (38 against 34) and +3 on the pocket
    (44 against 41).
  - Keep-down travel: zigzag has 716 mm less on the star (251 against 967)
    and 764 mm less on the pocket (211 against 975).
  - Turn feed medians: 0.445 against 0.448 on the star, 0.429 against
    0.401 on the pocket.
  - C20's recommendation for entries "within a few of each other" is
    zigzag. The operator decides.

### C12 3D test with the 2D departure rule

| Item | AgentSearch control | geodesic_curv | geodesic_curv_split |
|---|---|---|---|
| over the screen 0.546 (sim peak) | 225 (0.928) | **0** (0.518) | **0** (0.441) |
| over the exact bound 0.471 (exact peak) | 225 (0.999) | **0** | **0** |
| judged | 1460 (D12: 1295) | 2700 (D12: 2484) | 3107 (D12: 2902) |
| departing | 562 (D12: 798) | 5439 (D12: 5770) | 4196 (D12: 4488) |
| perimeter band | 941 (D12: 870) | 1780 (D12: 1665) | 2116 (D12: 2029) |
| departing + band + judged | 2963 = D12 | 9919 = D12 | 9419 = D12 |
| entry / vertical / waterline | 291 / 250 / 1301, as D12 | 1469 / 1695 / 1301, as D12 | 1064 / 1259 / 1301, as D12 |

The new rule moves 331 (`geodesic_curv`) and 292 (split) samples out of
`departing`. All of them read under the screen. The per-arm sums are
exactly as C20 predicted. `departing` is still twice `judged` on the arm.
The starter spiral's moves within R stay exempt, because they run on from
the plunge (D12 starter probe).

### Files

- Renders: `.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
  New: `<shape>_geodesic_local_zigzag.{txt,svg,png,c16.txt}`.
- New in `phase2/`: `d13_harness.txt`, `d13_c12_test_output.txt`.

### `git status --short` (worktree) at the end of D13

```
 M crates/rs_cam_core/src/adaptive/research_geodesic.rs
 M crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs
 M crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs
```

## D-FIX1 at cdc9814e: first 3D fixture renders (Corne; wanaka200 stopped)

**Build.** A clean detached worktree `.claude/worktrees/fixture-cdc9814e`
at cdc9814e, with its own target dir. The main worktree holds C22's edits,
so I did not use it. `cargo build --release -p rs_cam_cli --features
rs_cam_core/research` through the lane script took 3 min 36 s. Binary
`.claude/worktrees/fixture-cdc9814e/target/release/rs_cam_cli`, mtime
2026-10-24T16:13:21. Disk: 38 GB free before, 37 GB after. The lead
removes the worktree and its target at the end of the night's fixture
work.

**Project files (new; the originals are untouched).**

- `planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml`:
  lines 140 and 242 `clearing_strategy = "contour_parallel"` →
  `"agent_search"` ("3D Rough 1" and "3D Rough 1 (copy)").
- `planning/multitool_2026-08-23/wanaka200_mt1_c12_agent_search.toml`:
  - Line 347: the same change on "2 Back Rough".
  - Line 432: `depth = 12.0` → `floor = { from = "stock_top", depth = 12.0 }`.
    This is beyond §8. The original does not load at HEAD (`retired
    key setups[0].toolpaths[2].operation.params.depth`, R-B/F19), and this
    is the migration that the error names. The op ("3 Holes") is skipped
    in these runs.

**Runs** (`phase3/fix1/run_fix1.sh`, the binary direct, `timeout 200` per
run as the 3-minute stop):

| Fixture | Arm | Args | Result |
|---|---|---|---|
| Corne | agent (no env) | `--setup 0 --skip 4` (Waterline 3) | rc 0, 4.5 s |
| Corne | geodesic_curv | the same, `RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_curv` | rc 0, 8.7 s |
| wanaka200 | agent | `--setup 0 --skip 0,2,3,4` (Back Rough only) | **stopped at 200 s**, no output |
| wanaka200 | geodesic_curv | the same, with the env | **stopped at 200 s**, no output |

Both Corne runs pass `--emit-gcode program.nc --accept-unmodeled-tool-load`.
Without the flag, the export refuses: the skipped toolpath 4 has a stale
simulation. The flag is for the render only. The CLI prints "OVERRIDE USED"
and the findings.

**Renders** (`phase3/fix1/render_fix1.py`, from the exported G-code): per
rough op, (a) the cutting moves coloured by Z, (b) the rapids with XY
travel, (c) the entries.
- An entry is a G1 move that only descends, after a G0 or a lift.
- Keep-down links are G1 moves, so they are drawn as cuts. The G-code
  does not mark them.
- Page: `fixture3d_v1.html`.

**Corne numbers** (the CLI totals of the two roughs; entries and cut XY per
op from the G-code):

| Item | AgentSearch | geodesic_curv |
|---|---|---|
| Cycle time | 2112 s | 2749 s |
| Cutting / rapid distance | 38487 / 17531 mm | 65056 / 19141 mm |
| Air cut, % of runtime | 55.2 | 56.5 |
| Collisions (rapid) | 0 (0) | 0 (0) |
| Moves: Rough 1 / copy | 4168 / 6060 | 10673 / 5931 |
| Entries: Rough 1 / copy | 132 / 263 | 400 / 233 |
| Cut XY mm: Rough 1 / copy | 15053 / 8003 | 35073 / 7908 |
| Safety, islands > 0.8 mm: Rough 1 | 457, 6832.7 mm² | 928, 5249.9 mm² |
| Safety, islands > 0.8 mm: copy | 1257, 3652.5 mm² | 1143, 3479.8 mm² |
| Act on | DOC over the planned 3.00 mm on 2.6% of samples (peak 15.00 mm); high air cut on Rough 1 (45%) | high air cut on Rough 1 (47%) |
| Export load advisories | DOC peak 12.0 against 9.0 (tp 1), 15.0 against 4.5 (tp 3) | none printed |
| NOT MEASURED (tp 3, cell 0.300 mm) | 67% zero-engagement samples | 75% |

Notes:
- "3D Rough 1 (copy)" runs on the tool "End Mill" (a tool change in the
  program; the advisory reads its engaged diameter as 3.00 mm).
- The arm's Rough 1 has 123 Z values in its cuts, and the cut view shows
  concentric wraps round one seed.
- The research arm here has no turns (C22 is pending).

**D-FIX1, wanaka200 (operator-approved retry; outcome: no row).**

- First retry, `timeout 900`, lane script, `cargo run --release` in the
  fixture worktree:
  - AgentSearch: rc 124 at 900 s. It generated in 11.5 s ("3D adaptive
    toolpath complete moves=7665 … at 11.47 s", the lead's read), then
    the closing simulation used the rest of the 900 s.
  - geodesic_curv: stopped by the lead's order (rc 143 at 142 s).
- The CLI has no G-code export without the simulation. `project` has no
  no-sim flag. `rough-score --no-sim` writes no G-code. `run` plans one op
  from a model file, not the project's flipped setup.
- `project --resolution 1.0`: refused at once, for both arms (rc 1): "the
  project simulation resolution is 1 mm, coarser than the 0.1 mm the rest
  this project machines needs (the rest tool's TIP radius / 5)". The rule
  reads the whole project (the R0.5 tool of setup 2), also when those ops
  are skipped. Only a further edit of the copy (removing the rest ops)
  would allow a coarser cell. That is outside the brief, so I did not do
  it.
- The page's wanaka200 row states this reason. Script:
  `phase3/fix1/run_fix1_wanaka.sh` (its last form has `--resolution 1.0`).

## D14 at cdc9814e + uncommitted C22 and C23

- C22: the 3D arm with turns, turn feeds and held links.
  - Files: `adaptive3d/{mod,path,clearing}.rs`, the C12 block of
    `research_geodesic.rs`, the C12 test.
  - Note: `P3_C22_3d_turns.md`.
- C23: the pocket v2 columns.
  - Files: the rest of `research_geodesic.rs`, the 2D harness.
  - Note: §C23 of `P2_C2_geodesic_fixes.md`.

The run was the union of both command lists, in the main adaptive-head
worktree. The D13 render dir was copied first. Lane script, a `timeout` on
each command. The diff did not change during the run. Disk: 37 GB free.

### Fix

None. The two C22 reds are real; I did not patch them.

### Results

| Step | Command | Result |
|---|---|---|
| 1 | `fmt --all -- --check` | green |
| 1 | `check -p rs_cam_core --all-targets`, plain and research | green, green |
| 2 | `--lib adaptive::research_geodesic` (research) | green 23 passed, 2 ignored |
| 2 | the same `-- --ignored` | red 0/2, the recorded reasons: `every transition is traced` (`transition_fallback: 1`, C6) and the G-MOPLOAD `ResidueMop` step 0.45244 |
| 3 | `adaptive3d_emission_byte_parity`, plain and research | green 5/5, 5/5 |
| 3 | the 13 adaptive3d sentries | green, all 13 |
| 3 | `every_ignore_states_its_kind_f152` | green 2/2 |
| 4 | `--test adaptive3d_geodesic_arm_c12 -- --nocapture` (the plain test) | **red**: `the_geodesic_arm_feeds_its_turns_down_at_the_level` panics at T:906 |
| 4 | the same `-- --ignored --nocapture` (4 modes) | **red** at T:836 in mode 3 (`geodesic_local_oneway`), 598.88 s |
| 5 | the 2D harness, release | green 1/1, 14.02 s; output in `phase2/d14_harness.txt` |
| 6 | clippy core plain, core research, workspace full lint | green, green, green |

### Controls against D13

All 149 render files that the harness wrote again (`.txt`, `.svg`, `.png`,
`.c16.txt`) are byte-equal to D13, except `summary.txt`. `summary.txt`
gains the 8 new blocks (4 columns × 2 shapes) and loses no line.

### C23: the rule

| Item | oneway_v2 star | oneway_v2 pocket | zigzag_v2 star | zigzag_v2 pocket |
|---|---|---|---|---|
| Entries (base) | **35** (34) | 29 (41) | 38 (38) | 26 (44) |
| S / L / J / M / C / W / P | 1/0/19/0/0/0/15 | 1/0/15/0/0/0/13 | 1/0/23/0/0/0/14 | 1/0/11/0/0/0/14 |
| Turns emitted of planned; over the bound | 45 of 45; 3 | 48 of 48; 9 | 45 of 46; 2 | 53 of 53; 4 |
| Turn length (mm) | 2.058 to 2.960 | 0.904 to 2.890 | 2.059 to 2.751 | 0.048 to 3.199 |
| without | 0 | 0 | 0 | 0 |
| Steps on no turn: max, over | 0.3565, 0 | 0.3582, 0 | 0.4509, 1: `residue mop cut (segment 163) (71.77, 80.74) → (71.28, 81.30)` | 0.3582, 0 |
| Clearing-wrap cascades | 10 | 0 | 7 | 0 |
| Return links | 1037 (408.8 mm) | 2235 (904.7 mm) | 0 | 2143 (866.9 mm) |
| Keep-down links | 1054 (532.5 mm) | 2302 (1120.0 mm) | 59 (251.2 mm) | 2249 (998.1 mm) |
| Cut length (mm) | 1186.4 | 1506.7 | 1173.0 | 1519.5 |

| Rule | oneway_v2 | zigzag_v2 |
|---|---|---|
| 1. pocket < base, star ≤ base | pocket 29 < 41; **star 35 > 34** | pocket 26 < 44; star 38 ≤ 38 |
| 2. no non-turn step over (mop excepted) | 0 / 0 | star: 1, the residue mop (excepted); pocket 0 |
| 3. `without 0` | yes | yes |
| 4. pocket C 0, cascades ≤ base | C 0, 0 ≤ 5 | C 0, 0 ≤ 10 |
| **Verdict** | **DROP** (rule 1: the star has one more residue-mop entry, P 15 against 14) | **KEEP** |

The star `zigzag_v2` equals its base (38 entries, 251.2 mm keep-down). On
the pocket, `zigzag_v2` runs mostly one-way: 2143 return links, 866.9 mm.

**Head-on-only columns (attribution, not under the rule):**

| Column | Entries star / pocket | Pocket C, cascades | Pocket keep-down |
|---|---|---|---|
| oneway_headon | 35 / 35 | 0, 0 | 2050 (1082.2 mm) |
| zigzag_headon | 38 / 34 | 0, 0 | 1950 (992.6 mm) |

On the pocket, the head-on rule alone takes one-way 41 → 35 and zigzag
44 → 34. The merged meeting line then takes them to 29 and 26.

### C22: the 3D test

The heavy test, per mode (counts in samples):

| Item | AgentSearch | geodesic_curv | geodesic_curv_split | geodesic_local_oneway |
|---|---|---|---|---|
| moves | 657 | 2169 | 2157 | 4447 |
| over the screen / the exact bound | 225 / 225 | 0 / 0 | 0 / 0 | 0 / 0 |
| entry (D13) | 291 (291) | **1410** (1469) | **986** (1064) | 809 |
| judged | 1460 | 2964 | 3541 | 6820 |
| departing | 562 | 4978 | 3523 | 2331 |
| turn samples (over the screen, sim peak) | 0 | 0 | 0 | 337 (0, 0.443) |
| turns: planned / cut / reduced / lowest factor / longest | — | 0 | 0 | 86 / 86 / 84 / 0.310 / 2.984 mm (bound 3.207) |
| turns: fed / unfed / full feed with no move | — | — | — | 82 / **3** / 1 |

`geodesic_local_zigzag` is not reached: the test panics in
`geodesic_local_oneway`. The `turn_faults`:

```
"3 turns unfed",
"turn 42 (-11.9113, -17.8757) to (-9.1576, -17.8757) is counted fed, and no move carries factor 0.5099",
"turn 43 (-9.1576, -17.8757) to (-6.6862, -17.8757) is counted fed, and no move carries factor 0.4780",
"turn 44 (-6.6862, -17.8757) to (-4.3977, -17.8757) is counted fed, and no move carries factor 0.4086",
"turn 45 (-4.3977, -17.8757) to (-2.1443, -17.8757) is counted fed, and no move carries factor 0.4275",
"turn 56 (14.4668, -17.8757) to (15.9548, -17.8757) is counted fed, and no move carries factor 0.4485",
"turn 61 (-11.4500, -2.7361) to (-11.9500, 0.1526) is counted fed, and no move carries factor 0.5770",
```

The plain test (one level, Z 4.5), `geodesic_local_oneway`: turns planned
42, cut 42, reduced 42 (lowest 0.310), longest 2.984 mm; fed 39, **unfed
3**. 69 entries, 2121 moves. It panics before its zigzag mode.

**Reading (not measured).** Turns 42 to 45 lie end to end on one straight
line (y = -17.8757, x -11.91 to -2.14), with turn 56 on the same line.
That is C22's P3 risk: the 3D RDP (`cut_simplify_tolerance`, 0.1 mm)
merges collinear turn moves, so no emitted move starts and ends on one
turn, and the feed cannot attach. The turn samples read under the screen
(sim peak 0.443), so the fault is the feed attachment, not an overload.
The held links lower the entry samples of `geodesic_curv` (1469 → 1410)
and of `geodesic_curv_split` (1064 → 986), with 0 over in both modes, as
C22 predicted.

**Consequence.** C22 is not green, so the Corne zigzag/oneway fixture runs
(the lead's condition) are not started.

### Files

- `phase2/d14_harness.txt`, `phase2/d14_c12_test_output.txt`,
  `phase2/d14_c12_plain_output.txt`.
- Renders: `.claude/worktrees/adaptive-head/target/tmp/adaptive_geodesic_2d_g1/`.
  New: `<shape>_geodesic_local_{oneway,zigzag}_{v2,headon}.{txt,svg,png,c16.txt}`.

### `git status --short` (worktree) at the end of D14

The same six paths as at the start (C22 and C23 only).

### D14 addendum: `wanaka200_mt1_c12_rough_only.toml` (no cargo)

A NEW copy, `planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml`,
made from `wanaka200_mt1_c12_agent_search.toml` (the lead's order). It
keeps only the roughing ops, so the resolution rule ("the rest tool's TIP
radius / 5") no longer reads a skipped rest op. The original
`wanaka200_mt1.toml` and the first copy are unchanged (md5 checked before
and after).

`diff wanaka200_mt1_c12_agent_search.toml wanaka200_mt1_c12_rough_only.toml`
deletes three line ranges of the first copy and adds nothing (`diff` prints
`242,318d241`, `418,653d340`, `763,1035d449`. The first range is the same cut
as 241-317 below, because lines 241 and 318 are both `[[setups.toolpaths]]`):

| Lines removed | Setup | Toolpaths removed (id, name, stock source) |
|---|---|---|
| 241-317 | 0 "Setup 1" | 0 "1 Pin Drill" (fresh) |
| 418-653 | 0 "Setup 1" | 2 "3 Holes (6mm pilot)" (fresh; its migrated `floor` line goes with it), 3 "4 Rivers (back, V-bit)" (from_remaining_stock), 4 "5 Lakes (back, V-bit)" (from_remaining_stock) |
| 763-1035 (to the end) | 1 "Setup 2 — front" | 8 "7 3D Finish tier-A (R2.0)", 10 "7b 3D Finish tier-B rest (R1.0)", 9 "8 Pencil detail (R0.5)" (all from_remaining_stock) |

Kept:
- `[job]`, the stock, the alignment pins, the post, the machine, all six
  `[[tools]]`, all four `[[models]]`, and both `[[setups]]` headers.
- Setup 0: id 1 "2 Back Rough", `stock_source = "fresh"`,
  `clearing_strategy = "agent_search"`.
- Setup 1: id 5 "6 3D Rough (front)", `stock_source = "fresh"`,
  `clearing_strategy = "contour_parallel"`, as in the original.

Neither kept op reads another op's stock (both are `fresh`), and nothing
outside `[[setups]]` names a toolpath id. The file parses
(`tomllib`): Setup 1 → ["2 Back Rough"], Setup 2 — front → ["6 3D Rough
(front)"]. 449 lines (the first copy has 1035).

**Not yet checked (needs a run):** whether the CLI now accepts
`--resolution 1.0`. The six tools stay in the file. If the rule reads the
tool list rather than the ops, the refusal stays. The first run will show
it.

## D15 at cdc9814e + uncommitted C22 fix (committed after the run as 31935b89)

The C22 fix (P3_C22_3d_turns.md §9):
- `split_at_turns` (`research_geodesic.rs:7839`), called in
  `adaptive3d/clearing.rs:2475`;
- the new test attribution (`turn_moves` / `turn_faults`: every record
  that a move lies on; the feed must be at most the lowest F·factor).

The steps ran in the lead's order, through the lane script with a
`timeout`. The D14 render dir was copied first.
- The run did not start at first: the disk had 3.8 GB free. The lead
  freed the main checkout's `target/debug/deps` files older than one day
  (92.5 GB). The run then had 89 GB free.
- The diff did not change during the run. No fix.

| Step | Command | Result |
|---|---|---|
| 1 | `--test adaptive3d_geodesic_arm_c12 -- --nocapture` (the plain test) | **green** 1/1 (7.00 s) |
| 2 | the same `-- --ignored --nocapture` (4 modes) | **green** 1/1 (738.67 s) |
| 3 | `adaptive3d_emission_byte_parity`, plain and research | green 5/5, 5/5 |
| 4 | clippy research, clippy plain, `fmt --all -- --check` | green, green, green |
| 5 | the 13 adaptive3d sentries | green, all 13 |
| 6 | the 2D harness, release | green 1/1 (13.87 s); output in `phase2/d15_harness.txt` |

**2D byte compare against D14:** all 181 render files that the harness
wrote again are byte-equal, `summary.txt` included. The split runs only in
3D, as the lead required.

**The plain test** (one level, Z 4.5):

| Mode | Turns planned / cut / reduced (lowest factor) | Longest (bound 3.207) | Fed / unfed / full feed with no move | Entries, moves |
|---|---|---|---|---|
| geodesic_local_oneway | 42 / 42 / 42 (0.310) | 2.984 mm | 42 / 0 / 0 | 68, 2117 |
| geodesic_local_zigzag | 46 / 27 / 23 (0.376) | 3.145 mm | 27 / 0 / 0 | 68, 831 |

**The heavy test** (two levels; counts in samples):

| Item | AgentSearch | geodesic_curv | geodesic_curv_split | geodesic_local_oneway | geodesic_local_zigzag |
|---|---|---|---|---|---|
| moves | 657 | 2169 | 2157 | 4427 | 1868 |
| over the screen / the exact bound | 225 / 225 | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| sim peak (judged) | 0.928 | 0.518 | 0.441 | 0.367 | 0.377 |
| entry | 291 | 1410 | 986 | 792 | 836 |
| judged | 1460 | 2964 | 3541 | 6824 | 3100 |
| turn samples (over the screen, sim peak) | 0 | 0 | 0 | 366 (0, 0.443) | 271 (0, 0.423) |
| turns planned / cut / reduced (lowest) | — | 0 | 0 | 86 / 86 / 84 (0.310) | **95 / 56** / 48 (0.375) |
| longest turn (bound 3.207) | — | — | — | 2.984 mm | 3.163 mm |
| fed / unfed / full feed with no move | — | — | — | 86 / 0 / 0 | 56 / 0 / 0 |

The control, `geodesic_curv` and `geodesic_curv_split` are the same as D14
on every count. `geodesic_local_oneway` changes from D14 (fed 82, unfed 3)
to fed 86, unfed 0. `geodesic_local_zigzag` runs for the first time: 95
planned turns, of which 56 are cut. The other 39 are examined in D-FIX2.

Files: `phase2/d15_harness.txt`, `phase2/d15_c12_test_output.txt`,
`phase2/d15_c12_plain_output.txt`.

## D-FIX2 at 31935b89: Corne zigzag and oneway, wanaka200 rough-only, fixture3d_v2.html

**Build.** The fixture worktree moved to 31935b89 (detached, clean).
`cargo build --release -p rs_cam_cli --features rs_cam_core/research`
through the lane script. Binary
`.claude/worktrees/fixture-cdc9814e/target/release/rs_cam_cli`, mtime
2026-10-25T13:40:24. Disk: 89 GB free before and after the runs.

**Runs** (`phase3/fix2/run_fix2.sh`; lane script, `cargo run --release`,
`timeout 900` each; all rc 0):

| Fixture | Arm | Args | Time |
|---|---|---|---|
| Corne | geodesic_local_zigzag | `--setup 0 --skip 4`, the D-FIX1 flags | 9 s |
| Corne | geodesic_local_oneway | the same | 10 s |
| wanaka200 rough-only | AgentSearch | `--setup 0 --resolution 1.0` | 26 s |
| wanaka200 rough-only | geodesic_local_zigzag | the same | 49 s |

The CLI accepted `--resolution 1.0` on `wanaka200_mt1_c12_rough_only.toml`
with no refusal (`summary.json`: `{"mm": 1.0, "mode": "fixed", "source":
"override", "project_mm": 0.5}`). The Corne AgentSearch control is the
D-FIX1 run at cdc9814e. C22 and C23 do not change the AgentSearch output
(byte parity, D14 and D15).

**Page** `fixture3d_v2.html` (v1 kept):
- One Z level per image: the first, a middle and the last level that
  every arm cuts, the same for every column. Corne "3D Rough 1": Z 17.00,
  5.00, 0.50. wanaka200 "2 Back Rough": Z 19.54, 8.62, 4.00.
- The 2D colours. One frame and scale per fixture. One sentence per image.
  A numbers table per column, and the CLI hard bars in a fold.
- Generator: `phase3/fix2/fixture3d_v2.py` plus `spec.json`. Assert
  script: `phase3/fix2/assert_fixture3d_v2.py`. It has its own parser, does
  not import the generator, and re-derives every number from the raw files:
  "all 158 checks hold". One injected wrong number (a cycle time) was
  caught; the page was restored.

How the page reads the data:
- Link tags: the `semantic_trace` entry spans of each `tp_*.json`
  (top-level "plunge entry" and "keep-down link" spans). A move index maps
  to G-code line i + 1 of the op section, with a checked surplus of 1 or 2.
- Turns carry no tag in the trace, so they draw blue with the cuts. The
  page says so.
- Levels: the trace's `depth_level` items carry no move range (null in
  every run), so a move takes its level from Z.
  - The G-code Z frame offset is the one that puts the most flat fed XY
    moves on the label planes: Corne -23.0, wanaka200 -25.0. The script
    stops if the top plane holds no flat move.
  - A fed move is at the highest plane at or below its Z. A G0 takes the
    level of the next fed move.

**Numbers** (CLI totals of the rough ops; per op from the trace and the
G-code):

| Item | Corne AgentSearch | Corne zigzag | Corne oneway | wanaka200 AgentSearch | wanaka200 zigzag |
|---|---|---|---|---|---|
| Commit | cdc9814e | 31935b89 | 31935b89 | 31935b89 | 31935b89 |
| Cycle time | 2112 s | 2745 s | 3068 s | 1339 s | 3950 s |
| Cutting distance | 38487 mm | 65025 mm | 76743 mm | 38108 mm | 90906 mm |
| Entries, first rough op | 48 | 56 | 91 | 32 | 22 |
| Keep-down links, first rough op | 136, 741 mm | 1035, 2026 mm | 1324, 3021 mm | 510, 1728 mm | 3402, 3423 mm |
| Entries, Corne copy op | 182 | 176 | 176 | — | — |
| Sim cell | 0.3 mm (auto) | 0.3 mm (auto) | 0.3 mm (auto) | 1.0 mm (override) | 1.0 mm (override) |

**The planned zigzag turns that are not cut** (heavy C12 test: 95 planned,
56 cut). A temporary `eprintln` in `plan_one_outline` (fixture worktree,
debug test build only; reverted with `git checkout`; the tree is clean and
the release binary is unchanged) ran on the one-level C12 test (46
planned, 27 cut).
- All 19 uncut turns show the same thing: the turn's START is the target
  of a `Rapid` in the cleaned 2D plan, and its end is a cut point.
- So the plan lifts the cutter at the arc end, re-enters (a plunge) at
  the turn start, and cuts the turn as the first step out of that plunge.
  The replay counts that step as a departure, not a turn.
- 17 of the 19 start 11.27 to 12.01 mm from the island centre (the
  island side). The other 2 start at 24.7 and 27.7 mm.
- The heavy test's 39 were not probed one by one. Output:
  `phase3/fix2/turn_probe_output.txt`.

### D-FIX2 addendum: pass spacing and load (the lead's row)

**The reruns for the load.** The five page runs ran again with
`--sim-artifact` (`phase3/fix2/run_simart.sh`; lane script, `timeout 900`;
all rc 0, about 13:47 to 13:51).
- Each `program.nc` is byte-equal to its page run, so the plans did not
  change. The assert script checks this too.
- The CLI was rebuilt at 13:50:30 from the same 31935b89 source, because
  the probe revert touched an mtime. The page header says so.
- `simulation.json` is 60 MB to 401 MB per run, under `phase3/fix2/simart/`.
- **My process fault:** the runs ended at about 13:51, and I did not check
  them until the lead asked at 16:37.

**Spacing.** Rule:
- 4 axis rays (0, 90, 180, 270 deg) from the centre: the geodesic zigzag
  entry ring nearest the AgentSearch first entry ring at the middle level.
  Corne (51.05, 58.55), wanaka200 (119.55, 124.624).
- Each ray is cut against the blue cut moves of the first rough op at the
  middle level. Crossings within 0.05 mm are one pass. A gap over one tool
  diameter (6 mm) is set apart.
- The four diagonal rays give the same medians within 0.07 mm.

**Load.** The share of the first rough op's cut length (the linear cutting
samples, weighted by XY length) whose sim radial width of cut is over 0.471
(C18's exact bound for s 2.0, R 3), and over C18's screen for the run's sim
cell: 0.3626 + (0.5 + cell + 0.1)/6.

| Fixture, level | Arm | Spacing median (range, n, gaps set apart) | Over 0.471 | Over the screen |
|---|---|---|---|---|
| Corne Z 5.00, stepover 1.2 | AgentSearch | 2.554 mm (0.258-5.944, 60, 2) | 20.19% | 18.10% (0.513) |
| | geodesic_local_zigzag | 1.125 mm (0.092-3.937, 179, 0) | 4.60% | 4.32% |
| | geodesic_local_oneway | 0.960 mm (0.069-5.159, 216, 0) | 4.39% | 4.13% |
| wanaka200 Z 8.62, stepover 2.0 | AgentSearch | 3.538 mm (0.124-5.970, 97, 5) | 44.81% | 11.84% (0.629) |
| | geodesic_local_zigzag | 1.979 mm (0.050-6.000, 207, 0) | 3.59% | 3.57% |

**Reading.**
- The geodesic arm spaces its passes at about the stepover. AgentSearch
  spaces them 2.27 times (Corne) and 1.79 times (wanaka200) wider.
- The geodesic arm cuts 2.42 times and 1.90 times the length on the first
  rough op. The page states that the cycle-time gap comes mostly from this
  spacing.
- AgentSearch is over the bound on both fixtures.

**Limits (stated on the page).**
- Corne's first rough op has stepover 1.2, not 2.0, so its ceiling is
  lower than 0.3626, and 0.471 under-counts there.
- The wanaka200 sim cell is 1.0 mm. Its 44.81% over 0.471 includes cell
  blending; the cell-aware screen gives 11.84%.

**Check.** `assert_fixture3d_v2.py` now re-derives the centre, the spacing
and the load with its own code, and checks the plan identity of each rerun:
"all 225 checks hold". One injected wrong spacing median was caught; the
page was restored.

## D16 at 31935b89 + C25b (committed as 4cceca914); C24 measured, no code

### D16a: the plain C12 test (C25b in the worktree; `research_geodesic.rs` unchanged)

Green: 1 passed, 1 ignored. Output in `phase2/d16a_c12_plain_output.txt`.

| Mode | Turns planned / cut / not cut | Reduced (lowest factor) | Longest (bound 3.207) | Fed / unfed | Moves | Entries |
|---|---|---|---|---|---|---|
| geodesic_local_oneway | 42 / 42 / 0 | 42 (0.310) | 2.984 mm | 42 / 0 | 2117 | 68 |
| geodesic_local_zigzag | 46 / 27 / 19 | 23 (0.376) | 3.145 mm | 27 / 0 | 831 | 68 |
| geodesic_local_zigzag_v2 | 46 / 46 / 0 | 45 (0.310) | 2.932 mm | 46 / 0 | 2314 | 48 |

C25's predictions for zigzag_v2 all hold: not cut 19 → 0 (prediction 0
to 2), entries 68 → 48 (prediction 45 to 58), unfed 0.

### D16b steps 1 and 2 (adaptive-head; the diff did not change during the run)

| Step | Command | Result |
|---|---|---|
| 1 | the heavy C12 test, `--ignored`, 6 modes | **green** 1/1, 940.70 s; output in `phase2/d16b_c12_test_output.txt` |
| 2 | `adaptive3d_emission_byte_parity`, plain and research | green 5/5, 5/5 |
| 2 | `fmt --all -- --check` | green |
| 2 | research clippy | green |

The heavy test, `geodesic_local_zigzag_v2`:
- 4651 moves, judged 7020, entry 611.
- 0 over the screen 0.546 (sim peak 0.377) and 0 over the exact bound
  0.471.
- Turn samples 366, 0 over the screen.
- Turns: planned 92, cut 91, **not cut 1** (C25: 0 to 4). Reduced 89
  (lowest 0.310), longest 2.932 mm. Fed 91, unfed 0. No turn faults.

Every other mode and the control equal D15 on every printed count.

### D16b step 4: the fixture runs at 4cceca914

- The fixture worktree moved to 4cceca914 (clean). Release CLI with
  research rebuilt; binary 2026-10-25 17:11:57. Script:
  `phase3/fix2/run_v2.sh`. The D-FIX2 flags plus `--sim-artifact`,
  `timeout 900`. Both rc 0.
- Corne zigzag_v2: 10 s. Both roughs: cutting 74059 mm, rapid
  18970 mm, time 2954 s. Its plan differs from zigzag.
- wanaka200 rough-only zigzag_v2: 52 s. Cutting 90906 mm, time 3950 s.
  Its `program.nc` is byte-identical to the D-FIX2 zigzag run. The arm
  name is accepted (no unknown-value warning). So the head-on rule and the
  merged meeting line change nothing on that back rough. The cause is not
  checked.

### D16b step 5: the page

`fixture3d_v2.html` gains the zigzag_v2 column (commit in the column
head). The spacing table now uses C24's measure, `phase3/fix2/normal_step.py`:
- **The step:** the step along the normal between adjacent passes. From
  each pass midpoint, the normal is cast both ways to the nearest other
  pass within 6 mm. The samples are length-weighted (median and the 10th
  and 90th percentiles). A side with no pass within 6 mm is set apart.
- **A pass:** a cut move of the level that the closing simulation reads
  over one sim cell / D. A straight move across the cut runs through
  cleared stock, so it drops out.
- **The starter helix:** moves within R + 0.05 mm of the starter centre
  are left out.
- **Check against C24:** the measure reproduces C24 on Corne (one-way
  1.200, zigzag 1.166; C24: 1.200 and 1.200).
- **The old ray medians** stay in a fold, with the sentence on why they
  read low.

| Fixture, level | Arm | Normal step, median (p10 to p90) | Over 0.471 | Cycle time | Entries, first rough op | Keep-down, first rough op |
|---|---|---|---|---|---|---|
| Corne Z 5.00, s 1.2 | AgentSearch | 2.680 (2.349 to 4.781) | 20.19% | 2112 s | 48 | 136, 741 mm |
| | geodesic_local_zigzag | 1.166 (1.006 to 1.420) | 4.60% | 2745 s | 56 | 1035, 2026 mm |
| | geodesic_local_oneway | 1.200 (1.028 to 1.331) | 4.39% | 3068 s | 91 | 1324, 3021 mm |
| | geodesic_local_zigzag_v2 | 1.200 (1.022 to 1.352) | 4.53% | 2954 s | 67 | 1271, 2450 mm |
| wanaka200 Z 8.62, s 2.0 | AgentSearch | 3.628 (3.405 to 4.490) | 44.81% | 1339 s | 32 | 510, 1728 mm |
| | geodesic_local_zigzag | 2.008 (1.884 to 3.852) | 3.59% | 3950 s | 22 | 3402, 3423 mm |
| | geodesic_local_zigzag_v2 | 2.008 (1.884 to 3.852) | 3.59% | 3950 s | 22 | 3402, 3423 mm |

On Corne, zigzag_v2 has more entries, more cut length and more keep-down
length than zigzag, and the cycle time rises by 209 s.

The assert script `phase3/fix2/assert_fixture3d_v2.py` re-derives the
normal step with its own code: "all 351 checks hold". An injected wrong
v2 cycle time was caught; the page was restored.

## D16c at 4cceca914 + uncommitted C26 (G-RDPLOAD chord cap; held links as Link)

C26 (P3_C22_3d_turns.md §10) changes two things in the research arm:
- The chord tolerance is capped at the load margin c·D − s: 0.1132 mm
  on Corne instead of 0.3, and unchanged at 0.1 in the C12 test.
- The held return links are emitted as `Adaptive3dSegment::Link`
  (tagged Linking).

The gates ran in the lead's order, through the lane script with a
`timeout`, in adaptive-head. The diff did not change during the run.
Disk: 86 GB.

| Step | Command | Result |
|---|---|---|
| 1 | `adaptive3d_emission_byte_parity`, plain and research | green 5/5, 5/5 |
| 2 | `adaptive3d_plan_matches_merged_path_g_plansimgap` + the 12 other adaptive3d sentries | green, all 13 (plansimgap 2/2) |
| 3 | the plain C12 test | green 1/1 |
| 3 | the heavy C12 test, `--ignored` | green 1/1, 946.09 s; output in `phase2/d16c_c12_test_output.txt` |
| 4 | `fmt --all -- --check`, research clippy | green, green |

The turn counts equal D16a (plain) and D16b (heavy) in every mode:
- Plain: oneway 42/42/0, zigzag 46/27/19, zigzag_v2 46/46/0.
- Heavy: oneway 86/86/0, zigzag 95/56/39, zigzag_v2 92/91/1.
- Every mode: fed = cut, unfed 0.

Every geodesic mode reads 0 over the screen and 0 over the exact bound.
The entry samples are unchanged. Moves and judged samples rise slightly,
and departing samples fall, as expected now that held links are `Link`s.
The heavy test, D16b → D16c:

| Mode | Moves | Judged | Departing |
|---|---|---|---|
| geodesic_curv | 2169 → 2298 | 2964 → 3099 | 4978 → 4689 |
| geodesic_local_zigzag_v2 | 4651 → 4719 | 7020 → 7043 | 1903 → 1624 |

The AgentSearch control is identical to D16b.

**Doc nit (the lead's, recorded; it does not block).** §10 and the doc of
`Adaptive3dParams::emission_chord_tolerance` say the model gouge guard
"stays safe" because it reads the uncapped `chord_tolerance`. The
opposite holds: the uncapped value is the larger tolerance, so the guard
is MORE lenient than the emitted path needs. On Corne the difference is
0.3 − 0.1132 = 0.187 mm.

### D16c addendum: the Corne re-runs at 6ce373922 (C26 committed)

The fixture worktree moved to 6ce373922 (clean). The release CLI with
research was rebuilt (17:43:35). Script `phase3/fix3/run_fix3.sh`: the
D-FIX2 flags plus `--sim-artifact`, `timeout 900`. Outputs in
`phase3/fix3/corne/<arm>/`.

| Arm | rc | Time | Result |
|---|---|---|---|
| geodesic_local_zigzag | **1** | 10 s | export REFUSED: the toolpath cuts into the model |
| geodesic_local_zigzag_v2 | **1** | 10 s | export REFUSED: the same point |
| geodesic_local_oneway | 0 | 10 s | cycle 2823 s (was 3068), cutting 70128 mm |

The refusal (zigzag; zigzag_v2 reads "1 of 40288"):

> Toolpath '3D Rough 1': The toolpath cuts into the model: 3 of 34016
> sampled cutting positions go below the limit of the stock to leave (the
> model surface plus the leave, less the op tolerance). The deepest is
> 1.450 mm below that limit at X 55.935 Y -15.766: the tool tip is at Z
> 2.250, and the limit there is Z 3.700. The tip is 0.950 mm below the
> model surface less the op tolerance. A wall steeper than 45 degrees
> stands within the tool radius here, or no surface lies under the tool
> centre: the depths are the height of that wall above the tip, and the
> tool may touch only the side of the wall. Look at the move named here: a
> cut below the model surface is a planner defect, so report it.

At 4cceca914, zigzag_v2 exported cleanly with the same file and flags.
So C26 brings this in. The guard's text allows a side-of-wall reading;
it is not classified.

**C22's prediction 1: the over-0.471 share.** Measured on "3D Rough 1",
over all fed linear and arc cutting samples (plunge, helix and ramp left
out), length-weighted. The runs wrote `simulation.json` before the export
refusal.

| Arm | Before C26 | After C26 | Over-bound length |
|---|---|---|---|
| zigzag | 4.59% | 4.15% | 1521 → 1376 mm |
| zigzag_v2 | 3.66% | 3.31% | 1524 → 1378 mm |
| oneway | 3.48% | 3.13% | 1520 → 1373 mm |

The prediction (zigzag about 1.5 to 3%) is NOT met. The over-bound
length falls about 10%. The linking samples read 0 over in every arm.

**C22's prediction 2: link spans for the straight returns.**
- The links show as move intent. The `linking` length of the sim samples
  (linear plus arc) rises: zigzag 1894 → 2445 mm, zigzag_v2 2293 →
  11079 mm, oneway 2858 → 13177 mm.
- `semantic_trace.items` has no new kind: the same "Pass N keep-down
  link" entry spans, fewer of them (1035 → 1012, 1271 → 939,
  1324 → 928).

**A correction to fixture3d_v2.html (not yet applied).**
- The page's load rule counted `linear` kinematics only. The emitter
  also fits arcs: before C26, about 8 to 9 m of the zigzag_v2 and
  one-way cut were arcs.
- So the page's shares for zigzag_v2 (4.53%) and oneway (4.39%) are too
  high. Over linear plus arc they are 3.66% and 3.48%. For zigzag the
  difference is small (4.60 against 4.59%).
- The rule must count every fed cutting sample except plunge, helix and
  ramp.
- The page's link tags must read the move intent rather than the trace
  spans.

### D16c-fix3: RED for the fixtures (the lead's ruling)

C26 (6ce373922) stays committed on the research branch. It is RED for the
fixtures: on Corne, the export of both zigzag arms is refused (the gouge
above). C22 is finding the move at X 55.935 Y -15.766. D17 (C28) is held
until one combined brief comes.

**The page update** (`fixture3d_v2.html`; the lead's answers (a) and (b)):
- **The zigzag arms at 6ce373922 stay off the page.** The zigzag and
  zigzag_v2 columns stay at 31935b89 and 4cceca914. One sentence on the
  page says C26 broke their export.
- **The new column** "geodesic_local_oneway (C26)" at 6ce373922
  (`phase3/fix3/corne/geodesic_local_oneway`).
- **The load rule for EVERY column:** every fed cutting sample with
  linear or arc kinematics (plunge, helix and ramp left out). One sentence
  states the correction of the earlier linear-only shares.
- **Link tags:** from the per-move intent (the `source_intent` of the
  move's first sample in `simulation.json`). A move with no sample keeps
  its trace-span tag. A keep-down link count is a run of consecutive link
  moves. On every pre-C26 run the intent tags give the same keep-down
  counts as the old span tags.
- **Asserts:** `assert_fixture3d_v2.py` reads the intents and the load
  rule with its own code: "all 402 checks hold". One injected wrong number
  (the C26 one-way keep-down length) was caught; the page was restored.

| Corne column | Commit | Over 0.471 (was, linear-only) | Over the screen | Cycle time | Entries, 3D Rough 1 | Keep-down, 3D Rough 1 | Normal step |
|---|---|---|---|---|---|---|---|
| AgentSearch | cdc9814e | 20.19% (20.19%) | 18.08% | 2112 s | 48 | 136, 741 mm | 2.68 mm |
| geodesic_local_zigzag | 31935b89 | 4.59% (4.60%) | 4.32% | 2745 s | 56 | 1035, 2026 mm | 1.166 mm |
| geodesic_local_oneway | 31935b89 | 3.48% (4.39%) | 3.28% | 3068 s | 91 | 1324, 3021 mm | 1.200 mm |
| geodesic_local_zigzag_v2 | 4cceca914 | 3.66% (4.53%) | 3.44% | 2954 s | 67 | 1271, 2450 mm | 1.200 mm |
| geodesic_local_oneway (C26) | 6ce373922 | 3.13% | 2.92% | 2823 s | 91 | 1156, 13477 mm | 1.131 mm |

wanaka200 under the corrected rule: AgentSearch 44.78% (screen 11.83%),
zigzag and zigzag_v2 3.59% (screen 3.56%).

**The remaining load (no new diagnosis; candidates only).** The C26
tolerance cap cut the over length by only about 10% (zigzag 1521 →
1376 mm), so the remaining ~4% over 0.471 has another cause. Top
candidates from the C26 sim of "3D Rough 1":
1. **A producer shared by both arms.** zigzag and one-way have the same
   over length (1376 against 1373 mm) and the same top cells. The
   candidate is the shipped perimeter sweep and waterline cleanup. The
   page counts them; the C18 test leaves them out.
2. **The levels.** Z 11 and Z 5 hold 750 of the 1376 mm (55%), and
   Z 17 holds 219 mm.
3. **Moves between level planes.** Z 4.0 and Z 18.3, draped moves, hold
   337 mm (24%).
4. **The place.** It is spread: no 10 mm cell holds more than 17 mm; the
   top cells are the strip y −20 to −10 at x 120 to 140 and at x 20 to 30.
5. **The producer tag.** The trace tags every over sample with the
   operation item only (intent clearing_cut, never linking), so it cannot
   name the producer more finely.

## D17 at 6ce373922 + uncommitted C26b and C28

- C26b (C22): `adaptive3d/clearing.rs` and `HeldLinks` in
  `research_geodesic.rs`; P3_C22_3d_turns.md §10b.
- C28 (C23): the pick (`geodesic_local_zigzag_pick`); end of §C28 in
  P2_C2_geodesic_fixes.md.

The 2D baseline is the D15 render dir (D16 ran no harness). Lane script,
`timeout` on each command, adaptive-head. The diff did not change during
the run. Disk: 85 GB.

| Step | Command | Result |
|---|---|---|
| 1 | byte parity, plain and research; fmt; research clippy; the 13 adaptive3d sentries | green: 5/5, 5/5, clean, clean, 13/13 (plansimgap 2/2) |
| 2 | `--lib research_geodesic`; `the_pick_keeps_the_plan_the_replay_re_enters_less` | green 24 passed, 2 ignored; green 1/1 |
| 3 | the 2D harness, release | green 1/1 (14.87 s); output in `phase2/d17_harness.txt` |
| 4 | the plain C12 test | green 1/1 |
| 4 | the heavy C12 test | green 1/1 (961.37 s); equals D16c on every printed count; output in `phase2/d17_c12_test_output.txt` |

**2D byte compare against D15.** All 181 render files that the harness
wrote again are byte-equal, except `summary.txt`. That file only adds the
two `geodesic_local_zigzag_pick` blocks and loses no line.

**The pick column** (verbatim `.c16.txt` lines):
- Star: 38 = S1 J23 P14. "pick piece 0: zigzag re-entries 38 (keep-down
  251.2 mm), zigzag_v2 re-entries 38 (keep-down 251.2 mm); kept zigzag".
  Pick A: holds.
- Pocket: 26 = S1 J11 P14. "pick piece 0: zigzag re-entries 44 (keep-down
  211.0 mm), zigzag_v2 re-entries 26 (keep-down 998.1 mm); kept
  zigzag_v2". Pick B: holds.
- Star: the one non-turn over is the known mop step, 0.4509; `without 0`.
- Pocket: 0 over; `without 0`.

**The plain C12 test.** The pick mode keeps zigzag_v2: 46/46/0, reduced 45
(lowest 0.310), longest 2.932 mm, fed 46, unfed 0, 2362 moves, 48 entries.
That is identical to the zigzag_v2 line. The other modes equal D16c.

### D17 fixture runs at 13667775d (C26b + C28 committed) and the page

**Runs.** The fixture worktree moved to 13667775d (clean). The release CLI
with research was rebuilt (18:23:12). Script `phase3/fix4/run_fix4.sh`:
Corne, the D-FIX2 flags plus `--sim-artifact`, `timeout 900`.

**All four export:** rc 0, and no "cuts into the model" line.

| Arm | Time | Cycle | Cutting | Entries, 3D Rough 1 (Z 17 / 11 / 5 / 0.5) | Keep-down, 3D Rough 1 | Over 0.471 (screen) | Normal step |
|---|---|---|---|---|---|---|---|
| zigzag | 9 s | 2607 s | 60483 mm | 56 (10 / 6 / 6 / 34) | 1050, 2371 mm | 4.14% (3.86%) | 1.136 mm |
| zigzag_v2 | 10 s | 2815 s | 69545 mm | 67 (14 / 8 / 8 / 37) | 1413, 8085 mm | 3.30% (3.07%) | 1.131 mm |
| oneway | 11 s | 2934 s | 72262 mm | 91 (23 / 13 / 13 / 42) | 1487, 9605 mm | 3.13% (2.92%) | 1.131 mm |
| zigzag_pick | 12 s | 2815 s | 69545 mm | 67 (14 / 8 / 8 / 37) | 1413, 8085 mm | 3.30% (3.07%) | 1.131 mm |

**The pick against the 3D entries.**
- `zigzag_pick`'s `program.nc` is byte-identical to zigzag_v2.
- Its four "C28 pick" lines (stdout; one per slice piece, one per level)
  all read `kept=B`:
  - Z 17: entries_a 91 (keep-down 401.2 mm), entries_b 87 (2427.2 mm).
  - Z 11, Z 5 and Z 0.5: entries_a 107 (110.7 mm), entries_b 80
    (2235.8 mm).
- In 3D, plan B plunges MORE than plan A at every level: 14 / 8 / 8 / 37
  against 10 / 6 / 6 / 34.
- So C23's prediction ("plain zigzag at every level, 56 entries") is NOT
  met. The pick's 2D re-entry count does not predict the 3D plunges:
  most 2D re-entries become 3D keep-down links. Link runs per level:
  zigzag 648 / 62 / 293 / 47; zigzag_v2 778 / 106 / 406 / 123.

**The page** `fixture3d_v2.html`:
- The Corne section is AgentSearch (cdc9814e) plus the four columns at
  13667775d. The earlier Corne geodesic columns are in this record.
- Two sentences: C26b removes the C26 export refusal; the pick result.
- The asserts read "all 402 checks hold". One injected wrong number (the
  zigzag cycle time) was caught; the page was restored.
- The previous spec is kept as `phase3/fix2/spec_d16c.json`.

## D18 at 13667775d + uncommitted C29 (the 3D-scored pick; SHIPPED path.rs touched)

C29 (P3_C22_3d_turns.md §10c):
- `try_emit_stay_down_link` is split into a pure `stay_down_link_z`
  (the SHIPPED `adaptive3d/path.rs`).
- The region body in `clearing.rs` moves into `plan_and_emit_region`.
- `research_geodesic.rs` changes too.

The 2D baseline is the D17 render dir. Lane script, `timeout` on each
command, adaptive-head. The diff did not change during the run. Disk:
84 GB.

| Step | Command | Result |
|---|---|---|
| 1 | byte parity, plain and research | green 5/5, 5/5 |
| 1 | `adaptive3d_keep_down_link_f038b`; `adaptive3d_plan_matches_merged_path_g_plansimgap` | green 4/4; green 2/2 |
| 1 | all 13 adaptive3d sentries | green |
| 1 | fmt; clippy research; clippy plain | green, green, green |
| 2 | the 2D harness, release | green 1/1 (14.90 s). All 189 rewritten render files are byte-equal to D17, `summary.txt` included. Output in `phase2/d18_harness.txt` |
| 3 | the plain C12 test | green 1/1. The pick keeps B: 46/46/0, fed 46, unfed 0, 2362 moves, 48 entries (zigzag: 68). The other modes equal D17 |
| 3 | the heavy C12 test | green 1/1 (941.79 s). Every control and mode summary line is text-identical to D17. Output in `phase2/d18_c12_test_output.txt` |

### D18 fixture run at f32401326 (C29 committed) and the page

**Run.** The fixture worktree moved to f32401326 (clean). The release CLI
with research was rebuilt (19:05:46). Script `phase3/fix5/run_fix5.sh`:
Corne `geodesic_local_zigzag_pick`, `RUST_LOG=info`, the D-FIX2 flags plus
`--sim-artifact`, `timeout 900`. rc 0, exports, 12 s wall. Cycle 2607 s,
cutting 60483 mm. Its `program.nc` is byte-identical to D17's zigzag
(13667775d).

**C23's prediction holds:** A at every level, plunges 10/6/6/34 = 56,
2607 s.

| Level | C29 line: plunges A (keep-down mm) / B (mm), kept | Trace plunges, "3D Rough 1" |
|---|---|---|
| Z 17 (region 1 of 1) | 4 (1016.1) / 8 (2223.9), A | 10 |
| Z 11 (region 2 of 2) | 5 (477.6) / 7 (2739.3), A | 6 |
| Z 5 (region 2 of 2) | 5 (429.8) / 7 (2665.5), A | 6 |
| Z 0.5 (region 2 of 2) | 4 (436.4) / 7 (632.3), A | 34 |

- **The score is partial.** It scores one region per level, and at Z 11,
  Z 5 and Z 0.5 only region 2 of 2. So it is below the level's plunges
  (4 of 34 at Z 0.5). Here A wins both counts, so the pick is right.
- **Generation of "3D Rough 1"** (tp `debug_trace` `total_elapsed_us`):
  the pick 4.33 s against D17 zigzag 2.33 s (+2.0 s, +86%). D17's
  2D-scored pick took 4.36 s. The copy op is about 3.0 s in all three.

**The page.**
- The Corne zigzag_pick column is now the f32401326 run. The other Corne
  geodesic columns stay at 13667775d.
- One sentence states the pick result, the generation time, and the
  scored-against-trace plunges. One sentence states the C28 result.
- `assert_fixture3d_v2.py` now also re-derives the numbers of that
  sentence (`debug_trace`, the C29 log lines, the trace plunges) and the
  pick/zigzag plan identity: "all 408 checks hold". One injected wrong
  number (the pick generation time) was caught; the page was restored.
  The previous spec is kept as `phase3/fix2/spec_d17.json`.

**D18 page correction (the lead, after C22's §10d).** The pick sentence
now says:
- the score covers the region bodies: 4/5/5/4 equal the body plunges per
  level;
- the plunges outside the score are the op's first entry, the waterline
  cleanup (2/2/2/31, cited from §10d) and the F-038-skipped regions;
- the first entry after each region body is not scored (G-PICKEXIT).

The sentence gives both level counts with their methods:
- **7/7/7/35:** the trace's plunge-entry items, placed by move order. Each
  level runs from its first cut move to the next level's first cut move,
  at moves 2, 4095, 5974 and 7875. This reproduces §10d.
- **10/6/6/34:** the page's red rings, placed by the Z of the move after
  the entry's last rapid.

The asserts re-derive both counts and the scored 4/5/5/4: "all 410 checks
hold". One injected wrong count was caught; the page was restored.

## D-FIX6: contour_parallel as the main control (operator ruling)

The operator roughs mostly with contour_parallel and considers AgentSearch
broken. So contour_parallel becomes the page's first column, and
AgentSearch moves to the right as a secondary column.

**New copies (the originals are untouched).**
- `planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_contour_parallel.toml`:
  the c12 copy with lines 140 and 242 back to `contour_parallel`. It is
  byte-identical to the original `corne_job_corrected.toml`.
- `planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only_contour_parallel.toml`:
  the rough-only copy with line 270 ("2 Back Rough") `agent_search` →
  `contour_parallel`. Same tool, stepover, depth and leave. "6 3D Rough
  (front)" was already contour_parallel.

**Runs** (`phase3/fix6/run_fix6.sh`): fixture worktree at f32401326,
binary 19:05:46, no arm variable, D-FIX2 flags plus `--sim-artifact`,
`timeout 900`. Both export (rc 0, no refusal).
- Corne: 4 s, cycle 2022 s, cutting 58238 mm, rapid 21069 mm.
- wanaka200: 44 s, cycle 5750 s, cutting 88747 mm, rapid 158535 mm.

**The producer split** (`phase3/fix2/producer_split.py`): C30's phases
from `phase3/c30/attribute.py`, generalised to any toolpath file.
- Agent-slice arms: perimeter from the level start to the level's second
  entry, then the body, then the waterline.
- contour_parallel has no `agent2d_region` spans and no perimeter sweep,
  so its body runs from the level start to the waterline, and the page
  shows "—" for its perimeter.
- The port reproduces C30's `c30_output.txt` exactly: zigzag_pick 1026.1
  / 32.6 / 317.1 mm; AgentSearch 1146.5 / 1269.1 / 364.2 mm.

| Fixture | Column | Commit | Cycle | Entries | Keep-down | Normal step | Over 0.471 | Over 0.471: perimeter / body / waterline (mm) |
|---|---|---|---|---|---|---|---|---|
| Corne | contour_parallel | f32401326 | 2022 s | 73 | 176, 768 mm | 1.125 | 8.06% | — / 3103.5 / 9.5 |
| Corne | geodesic_local_zigzag | 13667775d | 2607 s | 56 | 1050, 2371 mm | 1.136 | 4.14% | 1026.1 / 32.6 / 317.1 |
| Corne | geodesic_local_zigzag_v2 | 13667775d | 2815 s | 67 | 1413, 8085 mm | 1.131 | 3.30% | 1026.1 / 34.5 / 317.1 |
| Corne | geodesic_local_oneway | 13667775d | 2934 s | 91 | 1487, 9605 mm | 1.131 | 3.13% | 1026.1 / 33.4 / 313.2 |
| Corne | geodesic_local_zigzag_pick | f32401326 | 2607 s | 56 | 1050, 2371 mm | 1.136 | 4.14% | 1026.1 / 32.6 / 317.1 |
| Corne | AgentSearch | cdc9814e | 2112 s | 48 | 136, 741 mm | 2.68 | 20.19% | 1146.5 / 1269.1 / 364.2 |
| wanaka200 | contour_parallel | f32401326 | 5750 s | 964 | 982, 14856 mm | 2.000 | 3.01% | — / 1586.9 / 0 |
| wanaka200 | geodesic_local_zigzag | 31935b89 | 3950 s | 22 | 3402, 3423 mm | 2.008 | 3.59% | 1564.9 / 31.4 / 10.6 |
| wanaka200 | geodesic_local_zigzag_v2 | 4cceca914 | 3950 s | 22 | 3402, 3423 mm | 2.008 | 3.59% | 1564.9 / 31.4 / 10.6 |
| wanaka200 | AgentSearch | 31935b89 | 1339 s | 32 | 510, 1728 mm | 3.628 | 44.78% | 1564.9 / 9008.7 / 9.6 |

The entries and keep-down are for the first rough op. Over 0.471 counts
every fed linear and arc sample.

**Reading.**
- contour_parallel spaces its passes like the geodesic zigzag: 1.125
  against 1.136 mm on Corne, 2.000 against 2.008 mm on wanaka200.
- The geodesic zigzag cuts 0.86 times (Corne) and 0.85 times (wanaka200)
  the contour_parallel length.
- On Corne, contour_parallel reads 8.06% over 0.471, all in its body. The
  geodesic arm's over length is in the shared perimeter sweep and
  waterline; its body is 32.6 mm.
- On wanaka200, contour_parallel makes 964 plunges and runs 5750 s.

**The page.**
- contour_parallel is the first column for both parts, and AgentSearch is
  the last.
- The spacing table gains the producer-split column.
- The spacing sentence now compares the geodesic zigzag with
  contour_parallel, and states AgentSearch's over-bound share as secondary.
- `assert_fixture3d_v2.py` re-derives the producer split with its own
  phase code, plus the new sentence numbers: "all 546 checks hold". One
  injected wrong producer number was caught; the page was restored. The
  previous spec is kept as `phase3/fix2/spec_d18.json`.

### D-FIX6 addendum: the load rule by intent (the lead, from C30)

**The rule.**
- The load shares and the producer split now count every cutting sample
  whose intent is a cut (`source_intent == "clearing_cut"`), of any
  kinematics.
- So the draped helix-kinematics cut samples are in. Entry samples
  (`entry_plunge`) and link samples (`linking`) are out.
- In all ten sim artifacts the cutting intents are only `clearing_cut`,
  `entry_plunge` and `linking`.
- One sentence on the page states the rule. The earlier correction
  sentence now refers to it.

**Applied in:** `fixture3d_v2.py` (`load_share`), `producer_split.py`, and
the assert's own `load_check` and `producers_check`. "All 546 checks
hold".

**Check against C30.** On Corne zigzag, perimeter + waterline over 0.471
rise by 542.6 mm (1026.1 + 317.1 → 1293.0 + 592.8), which is C30's
543 mm. The body also rises by 10.6 mm (32.6 → 43.2).

| Fixture | Column | Over 0.471 (was) | Over the screen | Perimeter / body / waterline (mm) |
|---|---|---|---|---|
| Corne | contour_parallel | 8.01% (8.06%) | 7.19% | — / 3149.4 / 9.5 |
| Corne | geodesic zigzag (= pick) | 5.88% (4.14%) | 5.56% | 1293.0 / 43.2 / 592.8 |
| Corne | geodesic zigzag_v2 | 5.44% (3.30%) | 5.15% | 1293.0 / 47.1 / 592.9 |
| Corne | geodesic one-way | 5.34% (3.13%) | 5.05% | 1293.0 / 43.6 / 588.7 |
| Corne | AgentSearch | 23.50% (20.19%) | 21.41% | 1423.5 / 1269.2 / 645.4 |
| wanaka200 | contour_parallel | 10.65% (3.01%) | 8.91% | — / 6201.7 / 0 |
| wanaka200 | geodesic zigzag and zigzag_v2 | 7.22% (3.59%) | 6.18% | 2245.6 / 1871.1 / 20.1 |
| wanaka200 | AgentSearch | 48.02% (44.78%) | 16.96% | 2249.8 / 12171.9 / 19.2 |

**Reading.**
- On wanaka200 (terrain) the draped helix samples matter most:
  contour_parallel goes from 3.01 to 10.65%, and the geodesic zigzag body
  from 31.4 to 1871.1 mm.
- The geodesic zigzag still reads below contour_parallel on both parts:
  5.88 against 8.01% (Corne) and 7.22 against 10.65% (wanaka200).
- The geodesic zigzag cuts 0.83 times (Corne) and 0.98 times (wanaka200)
  the contour_parallel cut length.

**Old and new side by side (the lead, 22:32).**
- The page now has one table per part, "Load rule change: over 0.471, old
  rule against new rule". The old rule is linear and arc samples of any
  intent; the new rule is the cut intent of any kinematics. The table gives
  both shares and both cut lengths for every column, and the change in
  points.
- The generator keeps the old rule as `load_share_old`; the assert
  re-derives both with its own code: "all 576 checks hold". One injected
  wrong change was caught; the page was restored.
- The page holds no "linear or arc" rule text.
- State: the lead's commit 2a4cf2a83 holds the intent-rule scripts, but a
  page render from before the rule change. The current page, numbers and
  spec are in the working tree, not committed.

## D19 at f32401326 + uncommitted C31 part 1 — RED

C31 part 1 (P3_C22_3d_turns.md §11.6) changes the research arm:
- the 2D planner gets the region grown by R − m;
- a closing guard decides per region;
- no perimeter sweep is emitted.

The C12 test now judges the geodesic modes in the perimeter band. Files:
`adaptive3d/clearing.rs` and the C12 test. The 2D baseline is the D18
render dir. Lane script, `timeout`, adaptive-head. The diff did not
change during the run. Disk: 83 GB.

| Step | Command | Result |
|---|---|---|
| 1 | byte parity, plain and research; fmt; clippy research and plain; the 13 adaptive3d sentries | green, all 18 commands (plans_every_slice_piece 3/3) |
| 2 | the 2D harness | green 1/1; all 189 rewritten render files byte-equal to D18 |
| 3 | the plain C12 test | **RED**, at T:969 (`the_geodesic_arm_feeds_its_turns_down_at_the_level`) |
| 3 | the heavy C12 test | **RED**, at T:862, 154.56 s |

**The plain test** fails in its first mode, geodesic_local_oneway:

> geodesic_local_oneway: turn moves off the level Z 4.5: [move 1075: Z
> 10.500 to 10.500, move 1157: Z 10.500 to 10.500, move 1159: Z 5.956 to
> 5.956, move 1244: Z 10.500 to 10.500, move 1247: Z 10.500 to 10.500,
> move 1338: Z 10.500 to 10.500, move 1340: Z 6.279 to 4.500, move 1432:
> Z 4.500 to 4.820, move 1433: Z 4.820 to 4.500, move 1729: Z 10.500 to
> 10.500, move 1799: Z 10.500 to 10.500]

- 7 turn moves are at Z 10.5 (rim and island top Z 10 plus the 0.5
  leave), and 4 are partly draped.
- Against D18: turns planned 42 → 36, entries 68 → 119, moves 2123 →
  3015. Turns are still fed 36, unfed 0.
- My reading, not measured: in the grown region some turns lie where the
  disc covers the wall, so the drop-cutter lifts them onto the rim.

**The heavy test** fails in its first geodesic mode, geodesic_curv:

> geodesic_curv: 5 judged samples read over the exact bound 0.471 (exact
> peak 0.999); the first: move 496 ClearingCut at (12.81, 15.62, 7.00)
> wall distance 8.38: sim radial 0.744, exact peak of the move 0.999

| geodesic_curv | D18 | D19 |
|---|---|---|
| moves | 2298 | 3081 |
| judged | 3099 | 7947 |
| over the screen / the exact bound | 0 / 0 | **5 / 5** |
| sim peak | 0.518 | 0.784 |
| entry / departing samples | 1410 / 4689 | 2265 / 5025 |
| perimeter band | 2494 samples, exempt | 0 samples (judged) |

- The other modes did not run, because the test stops at the first
  failing mode.
- The AgentSearch control is unchanged (225/225).
- The over sample is mid-pocket (8.38 mm from the wall), and the exact
  peak 0.999 is a full-width cut.

**Verdict.** C31 part 1 is not committable. No fixture run was started.
Outputs: `phase2/d19_harness.txt`, `phase2/d19_c12_plain_output.txt`,
`phase2/d19_c12_test_output.txt`.

## C32 measure on the page: over 0.471 through stock at least one planner cell thick

The lead's request, after §C32 of P3_C18_3d_oracle.md.

**The measure** (`phase3/fix2/thick_measure.py`, generalised from
`phase3/c32/c32.py`):
- **The samples:** every cutting sample of the first rough op with intent
  clearing_cut that the sim reads over 0.471, in every phase.
- **The reading:** the exact swept width on the sample path densified to
  0.1 mm, on a 0.05 mm lattice. Only earlier path points at Z ≤ sample Z +
  0.5 mm (one planner cell) stamp. So only stock at least 0.5 mm thick
  counts.
- **The result:** the XY length of the samples still over 0.471, per
  producer and in total.
- **Check against C32:** the body part reproduces C32 exactly: wanaka200
  zigzag 263.8 mm, contour_parallel 2822.0 mm.
- **Cache:** results are cached in `phase3/fix2/thick_cache.json`, keyed by
  the sim file's size and mtime. The first build takes about 5 min.

| Fixture | Column | Over 0.471 (current) | Over 0.471, stock ≥ 0.5 mm: total = % of cut (perimeter / body / waterline mm) |
|---|---|---|---|
| Corne | contour_parallel | 8.01% | 3138.9 mm = 7.96% (— / 3132.7 / 6.3) |
| Corne | geodesic zigzag (= pick) | 5.88% | 1557.1 mm = 4.74% (1293.0 / 33.6 / 230.4) |
| Corne | geodesic zigzag_v2 | 5.44% | 1559.9 mm = 4.39% (1293.0 / 36.9 / 229.9) |
| Corne | geodesic one-way | 5.34% | 1554.4 mm = 4.31% (1293.0 / 35.1 / 226.3) |
| Corne | AgentSearch | 23.50% | 2901.5 mm = 20.42% (1422.3 / 1223.1 / 256.2) |
| wanaka200 | contour_parallel | 10.65% | 2822.0 mm = 4.84% (— / 2822.0 / 0) |
| wanaka200 | geodesic zigzag (= v2) | 7.22% | 2516.6 mm = 4.39% (2238.1 / 263.8 / 14.7) |
| wanaka200 | AgentSearch | 48.02% | 14295.3 mm = 47.53% (2241.3 / 12040.3 / 13.8) |

**Reading.**
- On wanaka200, the thick measure removes most of the draped body skim:
  zigzag body 1871.1 → 263.8 mm, contour_parallel 6201.7 → 2822.0 mm.
- What remains for the geodesic arms is mostly the perimeter sweep, a
  slot through fresh stock (wanaka 2238.1 mm, Corne 1293.0 mm). C31 aims
  to remove that sweep.

**The page.**
- A new column on the spacing table, next to the current shares.
- One sentence: a full-width skim of a thin layer is a light cut, and
  this measure leaves it out.

**Asserts.** `assert_fixture3d_v2.py` has its own implementation of the
measure (it does not import `thick_measure.py`), and checks the table cells
and the sentence: "all 637 checks hold" (about 4 min). One injected wrong
value was caught; the page was restored.

## D19b at f32401326 + uncommitted C31 with the §11.8 fix — RED

The fix in `adaptive3d/clearing.rs` (P3_C22_3d_turns.md §11.8): smooth,
then a shortfall inset so that measured + m ≥ cell, then grow, then the
guard. The C12 test is as in D19. Same steps and order as D19, with
`RUST_LOG=info` on the C12 runs.

| Step | Command | Result |
|---|---|---|
| 1 | byte parity, plain and research; fmt; both clippy; the 13 adaptive3d sentries | green, all 18 |
| 2 | the 2D harness | green; all 189 rewritten render files byte-equal to D18 |
| 3 | the plain C12 test | green 1/1. Every turn move at Z 4.5. Every printed count identical to D18: oneway 42/42/0, 2123 moves, 68 entries; zigzag 46/27/19, 870, 68; v2 = pick 46/46/0, 2362, 48 |
| 3 | the heavy C12 test | **RED** at T:862 (336 s) |

The heavy test fails in its first geodesic mode:

> geodesic_curv: 632 judged samples read over the exact bound 0.471 (exact
> peak 1.000); the first: move 6 ClearingCut at (28.00, -15.25, 7.00) wall
> distance 6.00: sim radial 0.833, exact peak of the move 1.000

| geodesic_curv | D18 | D19 | D19b |
|---|---|---|---|
| moves | 2298 | 3081 | **2298** |
| entry / departing samples | 1410 / 4689 | 2265 / 5025 | **1410 / 4689** |
| judged levels | 45 to 105 with 50 | without 50 | with 50 |
| perimeter band | 2494, exempt | 0 (judged) | 0 (judged) |
| over the exact bound | 0 | 5 | **632** (sim peak 0.918, exact 1.000) |

**Reading.**
- The plan is the D18 plan, perimeter sweep included, so the closing
  guard falls back to the old path on this fixture.
- Because C31 now judges the band in the geodesic modes, the old sweep (a
  full-width slot; move 6 at wall distance 6.00 lies in the old band
  [5.5, 7.46]) fails.
- C22's prediction "every C12 region keeps the grow" does not hold here.

**The C31 lines.** The "C31 region opening" lines are `tracing::info!`
events (`clearing.rs:2720`). The C12 test installs no tracing subscriber,
so `RUST_LOG=info` prints nothing in the test logs (0 lines). The decision
per region cannot be read from the tests.

**Verdict.** C31 is not committable as is. No fixture run was started.
Outputs: `phase2/d19b_harness.txt`, `phase2/d19b_c12_plain_output.txt`,
`phase2/d19b_c12_test_output.txt`.

**D19b probe (evidence only; not a fixture result).** CLI release with
research, built in adaptive-head (uncommitted C31 + §11.8). Corne
`corne_job_corrected_c12_agent_search.toml`, setup 0, `--skip 4`, the
D-FIX flags, `RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_zigzag_pick`,
`RUST_LOG=info`, no `--sim-artifact`. rc 0, 208 s with the build. Output
in `phase3/d19b_probe/`. The "C31 region opening" lines (stdout):

| z | region | measured_before | shortfall_inset | measured_after | excess / allowed mm² | decision |
|---|---|---|---|---|---|---|
| 17.0 | 1 | 0.258 | 0.242 | 0.500 | 2.02 / 238.36 | "grown, no sweep" |
| 11.0 | 2 | 0.276 | 0.224 | 0.500 | 1.98 / 198.35 | "measured margin under one planner cell: shipped sweep" |
| 5.0 | 2 | 0.276 | 0.224 | 0.500 | 1.98 / 198.35 | the same |
| 0.5 | 2 | 0.276 | 0.224 | 0.500 | 1.98 / 198.35 | the same |

All four lines have cell_mm 0.5 and nominal_margin_mm 0.550. In each, the
shortfall inset is exactly cell − measured_before. So measured_after lands
on the cell boundary (prints 0.500). At Z 17 that gives "grown"; at the
other three levels it gives "under one planner cell". My reading, not
checked in code: the guard's comparison at the boundary (strict or
without a tolerance, or the float rounding) decides.

## D19c at f32401326 + uncommitted C31 with the §11.9 fix — RED (heavy)

The fix (P3_C22_3d_turns.md §11.9): the shortfall inset adds
`FlattenPolicy::UNTOLERANCED_MM`; the region decisions go into
`Research3d.openings`; the C12 tests print one line per opening, and the
heavy test asserts "grown, no sweep" per region. Same steps as D19b.
The diff did not change during the run. Disk: 82 GB.

| Step | Result |
|---|---|
| 1: parity plain and research, fmt, clippy research and plain, 13 sentries | green, all 18 |
| 2: the 2D harness | green 1/1; 189 rewritten render files, all byte-equal to D18 |
| 3: the plain C12 test | green 1/1 (21.19 s) |
| 3: the heavy C12 test | **RED** at T:913 (150.48 s), the exact-bound assert in geodesic_curv |

**Openings.** Every opening reads "grown, no sweep":

| test / mode | Z | before | shortfall | after | excess / allowed mm² |
|---|---|---|---|---|---|
| plain, all four modes | 4.50 | 0.2669 | 0.2431 | 0.5100 | 0.74 / 129.89 |
| heavy, geodesic_curv | 7.00 | 0.2669 | 0.2431 | 0.5100 | 0.74 / 129.89 |
| heavy, geodesic_curv | 4.50 | 0.2609 | 0.2491 | 0.5100 | 0.96 / 130.67 |

**Plain counts** (D18 in brackets): oneway 2575 moves, 49 entries (2123,
68), turns 35/35/0; zigzag 954, 47 (870, 68), turns 45/29/16; v2 3122, 28,
turns 42/42/0; pick = zigzag 954, 47 (2362, 48).

**Heavy, geodesic_curv:** 2578 moves; 7013 judged at Z 4.5, 7.0, 7.5,
8.0, 10.0, 10.5; perimeter band 0. 4 over the screen 0.546 and the exact
bound 0.471 (sim peak 0.833, exact peak 0.975):
- move 435 at (-2.50, -8.59, 7.00), wall 3.95: sim 0.685, exact 0.707
- move 533 at (12.93, 5.52, 7.00), wall 9.06: sim 0.815, exact 0.957
- move 1711 at (-2.57, -8.88, 4.50), wall 4.24: sim 0.667, exact 0.775
- move 1805 at (17.82, 12.12, 4.50), wall 11.88: sim 0.833, exact 0.975

The test stops at the first failing mode, so the later geodesic modes of
the heavy test did not run.

**Against C22's prediction.**
- every region "grown, no sweep": holds;
- before 0.25–0.35, shortfall 0.16–0.26, after 0.500–0.510: hold (after
  is at the top end, 0.5100);
- heavy 0 over: **fails** (4 over);
- plain entries 68–80: **fails**, the counts are lower (28–49).

**Verdict.** RED. Not committable as is. No fixture run started.
Outputs: `phase2/d19c_harness.txt`, `phase2/d19c_c12_plain_output.txt`,
`phase2/d19c_c12_test_output.txt`.

## D19d: the heavy C12 test with C22's probe — RED (G-ARCENDHOOK)

Production diff as in D19c; only the C12 test changed (the probe, and the
failures are collected over all five geodesic modes). One mechanical fix
for clippy `doc_lazy_continuation` at the `probe()` doc comment (C12 test
lines 842–843): a blank `///` line between the list and "It names the
pass ...". Then fmt and clippy (research, heavy-tests, test-support) green.
The heavy test: rc 101 at T:1035 (181.62 s). Output:
`phase2/d19d_c12_test_output.txt`. The run ended at 07:52; I did not
report it to the lead until 08:20.

| mode | moves | over the exact bound | exact peak |
|---|---|---|---|
| geodesic_curv | 2578 | **4** | 0.975 |
| geodesic_curv_split | 2410 | **2** | 0.775 |
| geodesic_local_oneway | 5266 | 0 | — |
| geodesic_local_zigzag | 2000 | 0 | — |
| geodesic_local_zigzag_v2 | 6357 | 0 | — |

Every opening in every mode reads "grown, no sweep" (Z 7.00 and Z 4.50,
the same values as D19c).

## D19e: the final check of C31 part 1 — GREEN

Production diff as in D19c (clearing.rs, mod.rs, path.rs checked against
the D19c start diff). Only the C12 test changed: the heavy test asserts
the exact bound on the G1 arms only, adds `geodesic_local_zigzag_pick`,
and prints `geodesic_curv` / `geodesic_curv_split` as "attribution only
(G-ARCENDHOOK)". Lane script, `timeout`, adaptive-head. Disk 82 GB.

| Step | Result |
|---|---|
| 1: fmt; clippy all targets research+heavy-tests+test-support; clippy plain all targets | green, all 3 |
| 2: the plain C12 test | green 1/1 (20.46 s); every number as in D19c |
| 3: the heavy C12 test | green 1/1 (194.68 s; 215 s with the build) |

**Heavy, per mode** ("entry" is the count of judged entry samples):

| mode | moves | over the exact bound | exact peak | entry samples | turns planned / cut / not cut / fed |
|---|---|---|---|---|---|
| geodesic_curv (attribution only) | 2578 | 4 | 0.975 | 1433 | 0 |
| geodesic_curv_split (attribution only) | 2410 | 2 | 0.775 | 1066 | 0 |
| geodesic_local_oneway | 5266 | 0 | 0.000 | 665 | 71 / 71 / 0 / 71 |
| geodesic_local_zigzag | 2000 | 0 | 0.000 | 641 | 92 / 61 / 31 / 61 |
| geodesic_local_zigzag_v2 | 6357 | 0 | 0.000 | 439 | 84 / 83 / 1 / 83 |
| geodesic_local_zigzag_pick | 2000 | 0 | 0.000 | 641 | 92 / 61 / 31 / 61 |

Every mode prints two openings, both "grown, no sweep": Z 7.00 (before
0.2669, shortfall 0.2431, after 0.5100, excess 0.74 of 129.89 mm²) and
Z 4.50 (0.2609, 0.2491, 0.5100, 0.96 of 130.67 mm²).

**Plain, per mode:** oneway 2575 moves, 49 entries, turns 35/35/0;
zigzag 954, 47, 45/29/16; v2 3122, 28, 42/42/0; pick = zigzag. One
opening each, Z 4.50, "grown, no sweep" (0.2669 / 0.2431 / 0.5100).

**Against C22's prediction.** Heavy green: holds. zigzag_pick reads
like zigzag: holds (every heavy and plain number is equal). curv and
split print 4 and 2 attribution samples: holds.

**Verdict.** Green. Outputs: `phase2/d19e_c12_plain_output.txt`,
`phase2/d19e_c12_test_output.txt`. No harness and no fixture run.

## D20: the fixtures at b37cad1a4 (C31 part 1), geodesic_local_zigzag_pick

Fixture worktree checked out at b37cad1a4; release CLI with research
rebuilt (08:30; the binary's stamp string reads `0.1.0b37cad1a4`).
`phase3/fix7/run_fix7.sh`: `RUST_LOG=info`, `--sim-artifact`, the D-FIX
flags, timeout 900, lane script. Corne: `corne_job_corrected_c12_agent_search.toml`,
setup 0, `--skip 4` (rc 0, 12 s). wanaka200: `wanaka200_mt1_c12_rough_only.toml`,
setup 0, `--resolution 1.0` (rc 0, 63 s). Numbers:
`phase3/fix7/d20_numbers.py` (the page's `load_op` / `op_numbers`,
`producer_split.split`, `thick_measure.thick_over`, the CLI bar lines).
References: Corne D18 = `fix5` (f32401326); wanaka200 = the zigzag
`--sim-artifact` run at 31935b89 (`fix2/simart`), which the lead names as
D18 (22 / 3950 s). Opening lines: `phase3/fix7/corne_openings.txt`,
`wanaka_openings.txt`.

**C31 openings.**
- Corne: 4 of 4 "grown, no sweep". Z 17.0 region 1: before 0.258,
  shortfall 0.252, after 0.510. Z 11.0, 5.0, 0.5 region 2: 0.276, 0.234,
  0.510.
- wanaka200: 3 "grown, no sweep" (Z 19.54, 14.08, 8.62, region 1:
  before 0.550, no inset). Two shipped sweeps at Z 4.0:
  - region 1: `measured_before_mm=0.250 shortfall_inset_mm=0.260
    measured_after_mm=0.509 region_area_mm2=2580.5 centre_area_mm2=77602.8
    target_area_mm2=1157.2 excess_mm2=76445.52 allowed_mm2=864.92
    decision="closing guard: shipped sweep"`. The centre area is 30 times
    the region area.
  - region 3: `measured_before_mm=0.253 shortfall_inset_mm=0.257
    measured_after_mm=0.510 region_area_mm2=159.9 centre_area_mm2=108.1
    target_area_mm2=107.0 excess_mm2=1.12 allowed_mm2=31.31
    decision="the shortfall inset split the outline: shipped sweep"`.

| | Corne D18 | Corne D20 | wanaka D18 | wanaka D20 |
|---|---|---|---|---|
| plunges, first rough op | 56 | 60 | 22 | 20 |
| cycle time | 2607 s | 2760 s | 3950 s | 4832 s |
| keep-down links | 1050 (2371 mm) | 1239 (1702 mm) | 3402 (3423 mm) | 3982 (3349 mm) |
| moves, first rough op | 10918 | 11724 | 20853 | 25573 |
| cutting | 60483 mm | 64712 mm | 90906 mm | 100294 mm |
| over 0.471: perimeter / body / waterline | 1293.0 / 43.2 / 592.8 | 1.3 / 423.6 / 37.1 | 2245.6 / 1871.1 / 20.1 | 12.3 / 1701.4 / 19.5 |
| thick over 0.471: total (per / body / wl) | 1557.1 (1293.0 / 33.6 / 230.4) | 300.3 (0 / 268.0 / 32.3) | 2516.6 (2238.1 / 263.8 / 14.7) | 372.7 (5.0 / 353.2 / 14.6) |
| collisions (feed / rapid / failed checks) | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |

The producer split calls the span from a level start to its second entry
"perimeter" when the trace has `agent2d_region` spans. Where C31 emits no
sweep, that span is the start of the body. On wanaka the Z 4.0 regions do
ship the sweep.

**Coverage bar.** Corne 3D Rough 1: D18 961 islands (11 free), 5106.2
mm²; D20 1024 (6 free), 4095.6 mm². The D18 free island (16.50 mm high,
1400.9 mm² at (28.8, -16.6)) is gone. Two listed free islands of 0.80 mm
at (6.0, -45.7) and (6.0, -37.9) are not in the D18 list. Listed wall
islands in D20: 22.50 mm 3273.9 mm² at (14.1, -109.3) (also listed in
the zigzag_v2 column at 13667775d), 14.77 mm at (45.6, -7.3), 14.50 mm
412.1 mm² at (24.9, -100.3), 11.70 mm at (109.5, -7.9). The copy op: D18
1024 (14 free), 3449.4 mm²; D20 1027 (8 free), 2582.2 mm². The CLI lists
10 islands per op, free first; the D18 list shows no wall island, so a
full new-island check is not possible from the CLI text. wanaka200: no
standing-island line in D18 or D20.

**Gouge bar.** No gouge, "deeper than commanded" or "fed link cuts
stock" line in any of the four runs. No export refusal (the stale-trace
override as before); machine-safety errors 0.

## D20b: the full standing-island diff — STOPPED, no path without new code

Brief: the full island list of Corne "3D Rough 1", D18 (`fix5`) against
D20 (`fix7`), from the in-repo island walk, not from the CLI's list.
Search at b37cad1a4 (`rg` over the crates, tests and CLI):
- `standing_islands::measure_rough_coverage` and
  `ProjectSession::rough_coverage_for_run` / `rough_coverage_reading`
  return the full `RoughCoverage.islands`.
- Their only outputs are the triage advisory
  (`stock/sim_triage.rs:1143`, capped by `ADVISORY_CAP_PER_TOOLPATH = 10`,
  `sim_triage.rs:75`), the MCP narration (the tallest island only) and the
  viz state. The CLI `project` command writes no island file;
  `summary.json` and `simulation.json` hold no island list.
- The tests (`a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov.rs`)
  build their own stock and do not load a project.
So no existing path prints the full list. No code was written; the lead
decides on a path.

### D20b, run (lead-approved option b)

`phase3/fix7/run_d20b.sh` in the fixture worktree only: per commit, a
checkout, then `sed` of `ADVISORY_CAP_PER_TOOLPATH: usize = 10` → `100000`
(`stock/sim_triage.rs:75`), a release build, the Corne D20 run
(zigzag_pick, RUST_LOG=info, --sim-artifact, setup 0, --skip 4), then
`git checkout -- crates/rs_cam_core/src/stock/sim_triage.rs`.
- f32401326: rc 0, 11 s; program.nc byte-equal to `fix5`.
- b37cad1a4: rc 0, 12 s; program.nc byte-equal to `fix7`.
- After both: clean rebuild at b37cad1a4 (stamp `0.1.0b37cad1a4`);
  `git status --short` of the fixture worktree is empty.
Parser and diff: `phase3/fix7/d20b_islands.py` (it asserts the parsed
count and wall count equal the CLI's header and that no list is cut).
Lists: `phase3/fix7/islands_d18.json`, `islands_d20.json` (both rough
ops). Match rule: an island of the other run within 0.3 mm (one cell)
plus 0.05 mm print rounding in X and in Y, and in height.

**3D Rough 1.** D18 961 (950 wall), 5106.2 mm²; D20 1024 (1018 wall),
4095.6 mm².
- In D20, not in D18: 153 islands, 462.9 mm², 2 free. Of these, one is
  large: **14.50 mm high, 2.38 mm wide, 412.1 mm² at (24.9, -100.3), at a
  wall**; D18 has no island within 8 mm of it of more than 49.0 mm²
  (3.50 mm, 49.0 mm² at (27.6, -93.7)). The rest: 117 wall slivers of
  3.50 mm high (one step plus a little), each 4.4 mm² or less; 33 wall
  islands of lower height; the two free ones are 0.80 mm high (2.5 mm² at
  (6.0, -45.7), 0.2 mm² at (6.0, -37.9)).
- In D18, not in D20: 90 islands, 1482.5 mm², 7 free; the large one is
  the free 16.50 mm, 1400.9 mm² island at (28.8, -16.6).
- **The 22.50 mm / 3273.9 mm² wall island at (14.1, -109.3) is in D18**,
  with the same height, width, area and XY.

**3D Rough 1 (copy)** (for information): D20 adds 68 islands (693.4 mm²),
D18 has 64 not in D20 (698.2 mm²). The 16.51 mm, 678.0 mm² wall island is
in both with its peak point moved from (5.4, -13.9) to (20.4, -102.4), so
the XY rule counts it on both sides. The three free 3.50 mm islands of D18
at (28.5, -91.0), (19.8, -85.6), (54.3, -85.6) are gone.

### D20b, the copy op at (24.9, -100.3) (from the lists, no new run)

The lists give each island's peak XY, height, width and area, not its
footprint, so "inside" cannot be tested exactly.
- D18 copy op: the nearest islands to (24.9, -100.3) are 0.82 mm high,
  0.1 mm² at (19.5, -99.7) (5.4 mm away, free) and 3.17 mm, 0.3 mm² at
  (28.5, -93.7) (7.5 mm, wall).
- D20 copy op: the 16.51 mm, 5.07 mm wide, 678.0 mm² wall island has its
  peak at (20.4, -102.4), 5.0 mm away. The next is 1.48 mm, 0.1 mm² at
  (24.3, -91.0) (9.3 mm).
- The 678.0 mm² island has the same height, width and area in both runs
  (D18 peak (5.4, -13.9)). So the 412.1 mm² rough-1 island does not stand
  as a separate copy-op island, and it adds no area to the 678.0 mm²
  island.
- Observation: the D20 copy op lists 14.5 mm, 2.38 mm wide, 424.0 mm² at
  (141.3, -71.8); D18 has 14.5 mm, 4.54 mm wide, 1285.6 mm² there. The
  height and width equal those of the new rough-1 island; I did not
  examine why.

## D21: C35, the core spiral arm geodesic_local_spiral_pick — RED (step 10)

adaptive-head at b37cad1a4 plus the uncommitted C35 diff in 5 files
(`research_geodesic.rs`, `adaptive3d/clearing.rs`, `mod.rs`, `path.rs`,
the C12 test; 651 lines, `phase3/d21/start.diff`). The diff did not change
during the run. Lane script, `timeout`, each step in the foreground.
Outputs: `phase3/d21/<step>.txt`. Disk 81 GB.

| Step | Result |
|---|---|
| 1 fmt | green |
| 2 clippy core plain | green (46 s) |
| 3 clippy core research+test-support | green (50 s) |
| 4 clippy workspace heavy+research+test-support | green (57 s; only the dependency future-incompat note) |
| 5 parity plain | green 5/5 |
| 6 parity research | green 5/5 |
| 7 `--lib adaptive::research_geodesic` | green 24, 2 ignored |
| 8 2D harness | green 1/1; 189 rewritten files byte-equal to the last run (D19c) |
| 9 plain C12 | green 1/1 |
| 10 heavy C12 | **RED** at T:1151 (210.30 s) |
| 11 f152 | green 2/2 |

**Plain (step 9), spiral:** "core Z 4.50 seed (-18.46, -8.88) radius
9.31 mm, 4 levels: 2D re-entries 8, 3D entries inside 8"; 981 moves, 55
entries against zigzag_pick 954 / 47; turns the same; opening "grown, no
sweep". The other four modes are as in D19e.

**Heavy (step 10):** the spiral holds the exact bound (0 over, peak
0.000; 2068 moves; entry samples 753, vertical 656, departing 1512
against zigzag_pick 641 / 552 / 1360). It fails the core-zone assert:
"geodesic_local_spiral_pick: 2 core zones; with a re-entry inside" — core
Z 7.00 and Z 4.50, each "2D re-entries 8, 3D entries inside 8". The other
modes are as in D19e (curv 4 and split 2 attribution only).

**Verdict.** RED. Step 12 (the fixtures) not started.

### D21b: the C35 instrument run (prints only)

adaptive-head at b37cad1a4 + the C35 diff with C22's added prints. fmt
green; clippy research+test-support green (51 s); plain C12 green (28.55
s). Outputs: `phase3/d21/b_1_fmt.txt`, `b_2_clippy_research.txt`,
`b_3_c12_plain.txt`. Every line of the other modes, and the spiral's
summary lines, are identical to D21 step 9. The spiral core block at Z
4.50 adds 8 "rapid at … into Clearing" lines (r 4.312 to 8.250 from the
seed) and 9 "raw over" Spiral steps (readings 0.3631 to 0.3931 against
the ceiling 0.3626). Each of the 8 rapids lands on the end point of one
of the first 8 raw-over steps; the 9th (r 9.125 → 9.250, 0.3683) has no
rapid.

## D21c: the full C35 verify after the chord-tilt fix — GREEN; fixtures (step 12)

adaptive-head at b37cad1a4 plus the C35 diff (1029 lines,
`phase3/d21c/start.diff`); unchanged from step 1 to step 11. Outputs:
`phase3/d21c/<step>.txt`.

| Step | Result |
|---|---|
| 1 fmt | green |
| 2, 3, 4 clippy core plain, core research, workspace heavy | green (46, 46, 59 s) |
| 5, 6 parity plain, research | green 5/5, 5/5 |
| 7 `--lib adaptive::research_geodesic` | green 24, 2 ignored |
| 8 2D harness | green; 189 rewritten files byte-equal to D21 |
| 9 plain C12 | green; spiral core "2D re-entries 0, 3D entries inside 0"; 983 moves, 43 entries (zigzag_pick 47) |
| 10 heavy C12 | green (212.91 s); spiral 0 over the exact bound; both cores 0 / 0 |
| 11 f152 | green 2/2 |

Step 9 prints one "raw over" line, a Join step from the core edge
(r 9.312 → 10.219, reading 0.5000); the heavy test prints it at core Z
7.00. The lead accepted step 9: the Join is the hand-off to the first
wrap, outside the core zone, and goes to C22 as a follow-up.

**Commit.** The lead committed the diff as 3cc257c1f; `git diff b37cad1a4
3cc257c1f` equals `d21c/start.diff` (index lines aside).

**Step 12 runs.** The first Corne run was built in adaptive-head at
3cc257c1f. During its build, C22's C38 work-in-progress came into
adaptive-head (mod.rs 17:48:03; research_outer_band.rs 17:49:11), so
that run is kept as `fix12/corne_adaptive_head/` only. Both fixtures then
ran in the fixture worktree at 3cc257c1f (clean before and after; stamp
`0.1.03cc257c1f`; `phase3/fix12/run_fix12.sh`, as fix7). The Corne
program.nc is byte-equal to the adaptive-head run, and so is its audit
text. Corne rc 0, 205 s; wanaka200 rc 0, 69 s. Audit:
`fix12/audit_corne.txt`, `audit_wanaka200.txt` (fix11/audit.py, nominal
MRR as run_all.py). The §12.5 instrument (fix9/entries.py with the new
columns): `fix12/entries_fix12.py`, `entries_fix12.json`; it reproduces
D20 (1195 / 3578).

| | Corne D20 | Corne C35 | wanaka D20 | wanaka C35 |
|---|---|---|---|---|
| descents into stock, audit predicate | 1236 | 1118 | 3412 | 2031 |
| lift-over descents, entries.py (§12.5) | 1195 | 1076 (P 1080–1150) | 3578 | 2016 (P 3330–3550) |
| plunge-entry spans | 60 | 60 (P 60 ± 3) | 20 | 20 (P 20 ± 2) |
| planar turns over 30° | 1074 | 1063 | 221 | 188 |
| plunge feed / sim runtime | 545.3 / 1454.0 s = 37.5% | 489.2 / 1451.7 s = 33.7% | 2329.7 / 4832.2 s = 48.2% | 1396.4 / 3365.5 s = 41.5% |
| cycle time (CLI) | 2760 s | 2757 s (P ≈ 2715) | 4832 s | 3366 s (P ≈ −100 s) |
| moves, first rough op | 11724 | 13250 | 25573 | 18963 |
| over 0.471 per / body / wl (mm) | 1.3 / 423.6 / 37.1 | 1.3 / 443.8 / 41.2 | 12.3 / 1701.4 / 19.5 | 12.3 / 2207.4 / 15.0 |
| thick over 0.471 total (mm) | 300.3 | 294.9 | 372.7 | 466.7 |

- The audit's wanaka time split sums to more than the sim runtime in
  every wanaka column (D20 7095.1 of 4832.2 s; C35 6025.9 of 3365.5 s);
  the shares above divide by the sim runtime.
- C31 openings: Corne 4 of 4 grown. wanaka Z 4.0 changed: region 1 now
  "the shortfall inset split the outline: shipped sweep" (D20: closing
  guard, centre 77602.8 mm²; now centre 1228.2, excess 83.48 of 861.00);
  region 3 now "grown, no sweep" (D20: inset split).
- Bars: Corne islands 865 (6 free), 4307.3 mm² (D20 1024, 4095.6); copy
  op 1031, 2599.7 mm². wanaka: no island line. No gouge, deeper-than or
  fed-link line; collisions 0; no export refusal.

D21c addendum, the plunge-feed share on the program clock (C39; the
fix11/c39_runtime.py rules, copy `fix12/c39_fix12.py` → `c39_fix12.json`):
Corne D20 545.3 / 1455.3 s = 37.5 %, C35 489.2 / 1453.0 s = 33.7 %;
wanaka200 D20 2350.0 / 3866.0 s = 60.8 % (C39's value), C35 1416.7 /
2732.5 s = 51.8 %.

## D22: C38 part 1 (c38a.patch) in the fixture worktree — gates green; F1 run

Fixture worktree clean at 3cc257c1f; `git apply` of `phase3/c38/c38a.patch`
(md5 40a2353cec9ae4ef1de1cfe8ebc0f179; clearing.rs, mod.rs, new
research_outer_band.rs). Outputs `phase3/d22/`. Gates, all green: fmt;
clippy core plain (53 s) and research (47 s); byte parity plain 5/5 and
research 5/5; the 13 adaptive3d sentries; plain C12 (every line identical
to D21c step 9); workspace clippy (76 s).

**F1 run** (`phase3/c38/corne_c38a/`): spiral_pick,
`RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug`, `--sim-artifact`,
plus `--emit-gcode` (for the fix9 scripts); rc 0, 207 s; stamp
`0.1.03cc257c1f` (the stamp names HEAD, not the patch). Analysis copies
in `phase3/d22/`: `entries_d22.py` (fix9/entries.py, columns only),
`stockmap_d22.py` (fix9/stockmap.py; its D20 map is array-equal to
fix9's), `bands_d22.py` (R4's band table; reproduces R4's D20 row
exactly), `classify_d22.py` (c38/classify_outer_descents.py on three
columns; reproduces the C38 note's D20 numbers).

§3.4:
1. op 1 region=1 at Z 11, 5, 0.5: "grown, no sweep" (region area 1375.2,
   1334.8, 1334.8 mm²). P holds.
2. No "F-038: skipping micro-region" line. 12 "C38 G-b: skipping region"
   lines, all in the copy op, region 2 of 2, two per level (planned_mm
   -0.0).
3. Copy op, P-C1 **fails**: all 12 copy-op openings (2 regions × 6
   levels) read "grown, no sweep", and region 1 plans 2–3 plunges per
   level. Copy op 6042 → 10822 moves.
4. Slot records: not readable from the CLI log, and the plain C12 test
   prints no slot line; not checked.
5. Bands (mm² over 0.8 mm, outside the silhouette), D20 / C35 / C38a:
   0–1.5 600 / 600 / 514; 1.5–3 591 / 591 / 304; 3–4.5 620 / 620 / 313;
   4.5–6 511 / 511 / 180; 6–9 820 / 820 / 301; **3–9 1951 / 1951 / 794**.
   F1 (gate only) predicted ≥ 1756: **falsified** (794); by §3.3 the
   narrow loops do cut under the gate change alone.
   Z 17 outer descents (§5.4), D20 / C35 / C38a: all 238 / 206 / 184;
   outside the silhouette 124 / 125 / 101; beyond the mesh bbox 45 / 45 /
   14 (P-B1 ≤ 5: **fails**); on the grid 79 / 80 / 87 (P-B2 59–99:
   holds).
- Lift-over descents (entries.py) 1195 / 1076 / 1116; plunge-entry spans
  60 / 60 / 70.
- Cycle time 2760 / 2757 / 3445 s; cutting 64712 / 71615 / 83233 mm.
- Hard bars: collisions 0; no export refusal. **New in C38a:** the copy
  op "a fed link cuts stock it was planned to cross in air or on cut
  ground: link move 8460 removes 15.43 mm at (3.0, -71.6, 0.50). 301 of
  4596 link samples on 193 link moves remove more than 3.30 mm". Islands:
  op 1 869 (2729.9 mm²), copy 1028 (1538.3 mm²).
- After the run: `git checkout -- .`, research_outer_band.rs removed;
  `git status --short` empty at 3cc257c1f.

## D23: the full stack (C38 parts 1–2, slot types, C37 stage 1) — gates green; fixtures

Fixture worktree clean at 3cc257c1f; `git apply` of
`phase3/d23/tree_vs_3cc257c1f.patch` (md5 1afb532bc11a4340bbe2a3c694f5ac5e;
5 tracked files, 598+/81−, and the new research_outer_band.rs; saved as
`d23/applied.diff`, unchanged to the end). Outputs `phase3/d23/`.

| Step | Result |
|---|---|
| 1 fmt; 2–4 clippy core plain, core research, workspace heavy | green |
| 5, 6 byte parity plain, research | green 5/5 each; outputs identical but for timing |
| 7 `--lib adaptive::research_geodesic` | green 24, 2 ignored |
| 8 2D harness | green; 189 rewritten files byte-equal to D21c |
| 9 plain C12 | green; old modes identical to D21c apart from the new c37 / hand-off lines |
| 10 heavy C12 | green (225.73 s); spiral_headon 0 over the exact bound; cores 0 / 0 |
| 11 f152 | green |
| 12 the 13 adaptive3d sentries | green |

Step 9, 2D re-entries by join: spiral_headon 18 {CoreHandOff 1, Mixed 1,
OverBound 8, Turn 8}; spiral_pick 32 {CoreHandOff 1, Mixed 2, OverBound
10, Turn 19}. Entries: headon 24, spiral_pick 43. Heavy: headon 6407
moves, entry samples 383 (spiral_pick 585).

**Step 13 runs** (`phase3/fix13/run_fix13.sh`; the D23 build;
`RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug`, `--sim-artifact`):
fix13/corne (headon) rc 0; fix13/corne_pick (spiral_pick) **rc 1, export
refused**: "G-code export refused: tool load exceeded on toolpath(s):
toolpath 3: deflection=stiffness" (log: peak_um=200.476); no program.nc;
fix13/wanaka200 (headon) rc 0; fix13/wanaka200_pick rc 0. Scripts:
`d23_log.py` (log lines; pick runs count the kept plan only, split by the
per-plan "F-038: dropped short cut groups" marker, or equal corridor
halves; every block split), `entries_d23.py`, `stockmap_d23.py`,
`bands_d23.py`, `classify_d23.py`, `c39_d23.py` (copies of the d22 /
fix12 scripts; each reproduces its D20 reference), fix11/audit.py.

| | D20 | C35 | D23 headon | D23 pick |
|---|---|---|---|---|
| Corne descents (audit) | 1236 | 1118 | 1209 | — (no program.nc) |
| Corne lift-over descents (entries.py) | 1195 | 1076 | 1093 | 1010 |
| Corne plunge-entry spans | 60 | 60 | 74 | 67 |
| Corne planar turns > 30° | 1074 | 1063 | 1468 | — |
| Corne plunge share, program clock | 37.5 % | 33.7 % | 31.9 % | — |
| Corne cycle time | 2760 s | 2757 s | 3199 s | 2932 s |
| Corne 3–9 mm band (mm²) | 1951 | 1951 | 455 | 455 |
| wanaka descents (audit) | 3412 | 2031 | 2035 | 2031 |
| wanaka lift-over descents (entries.py) | 3578 | 2016 | 2016 | 2016 |
| wanaka plunge-entry spans | 20 | 20 | 20 | 20 |
| wanaka planar turns > 30° | 221 | 188 | 183 | 177 |
| wanaka plunge share, program clock | 60.8 % | 51.8 % | 51.9 % | 51.9 % |
| wanaka cycle time | 4832 s | 3366 s | 3372 s | 3367 s |

Bands, Corne (outside the silhouette, mm² over 0.8 mm), D23 headon = pick:
0–1.5 443, 1.5–3 230, 3–4.5 233, 4.5–6 96, 6–9 126 (C38a 794 for 3–9).

C37 re-entries (kept plan): Corne headon 120 {ClosedSeam 6, CoreHandOff 1,
Mixed 16, OverBound 55, Turn 42}, by z 36 / 28 / 28 / 28 (Z 17 / 11 / 5 /
0.5); pick 161 {ClosedSeam 1, CoreHandOff 4, Mixed 22, OverBound 44, Turn
90}, 38 / 41 / 41 / 41. Corne headon by place: inside the silhouette 84,
outside 3–4.5 1, 4.5–6 23, beyond the mesh bbox 12 (pick 123 / 1 / 24 /
13). wanaka headon 164 {Mixed 1, OverBound 153, Start 1, Turn 9}; pick 163
{OverBound 153, Start 1, Turn 9}; 54 per level at Z 19.54 / 14.08 / 8.62.
C35b hand-off: Corne Z 17 (1 re-entry) and Z 11 / 5 / 0.5 (2 each in
headon, 1 each in pick); wanaka Z 19.54 / 14.08 / 8.62 (0 re-entries,
"raw over none").

§3.4 and §3.6:
1. op 1 region=1 at Z 11 / 5 / 0.5: "grown, no sweep". Holds.
2. No "F-038: skipping micro-region" line in any run. G-b: Corne none;
   wanaka Z 4.0 region 2 (planned_mm -0.0).
3. Copy op, P-C1 **fails**: all 12 copy openings "grown, no sweep", and
   the copy op has 12 corridor openings (1 per region per level, about
   120–129 mm each).
4. Slot records with their feeds: no CLI or test line prints them; not
   checked.
5. 3–9 mm band 455 (P ≤ 321) **fails**. Z 17 outer descents beyond the
   mesh bbox: headon 18, pick 14 (P-B1 ≤ 5 **fails**); on the grid headon
   101 (P-B2 59–99 **fails**), pick 87 (holds).
6. Parity and sentries green.
- P-O1: openings at Z 11, 5, 0.5 in op 1 region 1: yes, but 4 per level,
  not 1. P-O2: each opening loops=1; cut 578–579 mm per level against P
  890 ± 15 % (757–1024): **fails**. P-O3: no other op 1 opening.
- P-W1: no opening at Z 19.54 / 14.08 / 8.62: holds. P-W2: the one
  opening (38 slot steps, 27.0 mm, centre 17.0 mm²) follows the Z 4.0
  region 1 line ("shortfall inset split: shipped sweep"), not region 2:
  **fails** as written. P-W3: 27.0 mm ≤ 1874: holds.

Hard bars: collisions 0 in all four. Corne headon and pick add a triage
line "crosses standing material deeper than this pass was planned to cut:
3.5 % (pick 3.6 %) of cutting samples remove more than the commanded 3.00
mm depth per pass by over 0.300 mm, peaking at 9.00 mm (pick 12.00 mm)"
and non-gating depth-of-cut advisories (op 1 peak 12 mm against 9; copy
op 9 / 12 mm against 4.5). Corne pick: export refused on the copy op's
deflection (above). Islands: Corne headon op 1 847 (2237.3 mm²), copy 1015
(1105.1 mm²). wanaka: no island, gouge or advisory line.

After the runs: `git checkout -- .`, research_outer_band.rs removed;
`git status --short` empty at 3cc257c1f.

### D23 addendum: the override rerun, the peaks, and the mrcl0 run

All on the D23 build (fixture worktree at 3cc257c1f, the D23 patch applied
again and checked against `d23/applied.diff`; `fix13/run_extra.sh`;
spiral_pick, the D22 flags). Restored after: `git status --short` empty.

**Override rerun (lead-approved, analysis only):**
`fix13/corne_pick_override/`, `--accept-exceeded-tool-load`; rc 0, 204 s.
The program is renamed `program_REFUSED_ANALYSIS_ONLY.nc`, with a
`README.txt` that quotes the refusal and says the file must never go to a
machine. simulation.json is byte-equal to the refused run, summary.json
is equal apart from the export block, and the toolpath files differ only
in debug timing. So the entries and stock map of `fix13/corne_pick` stand
for it. Audit (through a scratchpad view that links the renamed file as
program.nc): descents into stock 1062; planar turns over 30° 1096;
plunge feed on the program clock 475.1 / 1469.6 s = 32.3 %
(`fix13/audit_corne_pick_override.*`, `c39_d23_override.json`).

**Peaks** (`fix13/peaks_d23.py` → `peaks_d23.json`; place by the
hole-filled silhouette; the trace has no slot tag):
- Op 1 depth of cut 12.0 mm: headon 544 samples, pick 526; all
  clearing_cut linear at Z 11.0 and 5.0, in the outer strip (412 / 409) or
  beyond the mesh bbox (132 / 117). That is the outer strip cut at its
  full height from the stock top (23 − 11 = 12 mm) by the corridor
  openings, which run at Z 11, 5 and 0.5 in region 1.
- Copy op depth of cut (headon 9.0, pick 12.0 mm): 24 samples on one move
  (headon 1421, pick 1420), clearing_cut linear at x 151.06, y −70.6 to
  −64.3, Z 2.01: beyond the mesh bbox, 2.0 mm inside the stock's +X edge
  (153.07). The "peaking at 9.00 / 12.00 mm" of the triage line is this
  move. The copy op's samples over 3.3 mm are 173 linear beyond the bbox
  (Z 2.01), 79 helix inside the silhouette (Z 14.0 to 17.75, entry
  helices), 32 linear inside. My count gives 1.0 % of the copy op's
  cutting samples, not the CLI's 3.5 / 3.6 %; I did not reproduce the
  gate's population (its steady-state filter needs the span lookup), so
  the share is not checked.
- Copy op deflection peak: no output names the sample. Two proxies
  (ap · fz · sin θ, and ap alone) both put it on the same move 1420, at
  (151.06, −70.58, 2.01): ap 12.0 mm, arc 2.5 rad, fz 0.060 mm.
- C22a ground: the D23 build drops no copy-op group (0 "F-038: dropped
  short cut groups" lines for the copy op; D22 c38a dropped 1018 of 1220
  over both plans, 509 per plan). So none of these peaks is on dropped-group
  ground; they are on ground no op cut before, beyond the mesh bbox.

**mrcl0 run** (`phase3/c38/corne_mrcl0/`, job `corne_c38_mrcl0.toml`, md5
9682112c4dbfc8183f321ba21a9149eb; only the copy op's
`min_region_cut_length_mm` 15 → 0): rc 1, export refused, the same
deflection text as the base. Every toolpath file equals the D23
spiral_pick base apart from debug timing; summary.json and the CLI triage
lines are the same. The base has no "fed link cuts stock" line, and the
copy op drops no group at 15 mm on this build, so the threshold change has
nothing to act on. Copy op entries 188 (all plunge-entry spans) in both.

D23 code commit check: the lead committed the D23 code as 6aefca969. `git
diff 3cc257c1f 6aefca969` equals `d23/applied.diff` for the tracked files,
and its research_outer_band.rs equals the patch's (index lines aside). So
every D23 run (fix13, the override rerun, mrcl0) is the 6aefca969 code; no
rerun is needed. The fixture worktree is now at 6aefca969, clean.

## D24: C22a (replay after the F-038 filter, held-link counter), corridor-slot removal, turn summary — gates green; fixtures

Fixture worktree clean at 6aefca969; `git apply` of
`phase3/d24/tree_vs_6aefca969.patch` (md5 8438961ef5587949212b5095ec19c63f;
7 tracked files, 455+/229−; `d24/applied.diff`, unchanged to step 12).
Outputs `phase3/d24/`.

| Step | Result |
|---|---|
| 1–4 fmt, clippy core plain / research, workspace clippy | green |
| 5, 6 parity plain, research | green 5/5 each |
| 7 `--lib adaptive::research_geodesic` | green 25, 2 ignored (`a_link_over_a_dropped_group_is_refused_after_the_filter` ok) |
| 8 2D harness | green; 189 files byte-equal to D23 |
| 9 plain C12 | green 2/2; every old line identical to D23; 5 new lines, each "0 held links over standing stock" |
| 10 heavy C12 | green (222.57 s); output identical to D23 |
| 11 f152; 12 the 13 sentries | green |

Step 9 new lines ("with the group filter at 15 mm"): oneway 5077 moves /
117 entries; zigzag 1908 / 117; zigzag_v2 6228 / 61; spiral_pick 1958 /
105; spiral_headon 6278 / 49; all "0 held links over standing stock".

**Step 13** (`phase3/fix14/run_fix14.sh`, the D24 build, spiral_pick, the
debug filter, `--sim-artifact`): Corne rc 0, 209 s, **export not refused**;
wanaka200 rc 0, 73 s. Scripts: copies of the fix13 scripts
(`entries_d24.py`, `stockmap_d24.py`, `bands_d24.py`, `c39_d24.py`,
`peaks_d24.py`) and `held_links.py` (kept plan only).

- C22a replayed: Corne 40 lines (20 per plan); rounds 1 to **3** (P ≤ 2
  fails: Z 17 r1 3; Z 11 r1 3; Z 5 / 0.5 r1 3; copy op Z 14.01, 8.01,
  5.01 r1 3). wanaka 10 lines, rounds 1–2.
- C22a held link over standing stock: Corne 0. **wanaka 66 lines, 33 in
  the kept plan** (P 0 fails): Z 19.54 12, Z 14.08 12, Z 8.62 9;
  stock_above 1.28 to 16.38 mm (multiples of 5.46 at x 219.3, x 20.8, y
  25.874, y 223.874). The wanaka sim prints no fed-link line.
- C22 turn summary (kind="turns" only; no slot line): Corne op 1
  planned 225, cut 140, fed 140, unfed 0, full_feed_unmatched 0, reduced
  137, lowest 0.280, longest 2.356 mm; copy op planned 10, cut 0.
- Hard bars, Corne: **fed link** "3D Rough 1": link move 6921 removes
  10.50 mm at (149.7, -78.7, 11.00); 2 of 39900 link samples on 2 link
  moves over 6.30 mm (beyond the mesh bbox). Deep-cut line: 4.0 %, peaking
  at 15.56 mm. Depth-of-cut advisories: op 1 peak 18.0 (limit 9), copy op
  15.56 (limit 4.5). Collisions 0. wanaka: no bar line.
- Peaks: op 1 samples at 12 mm or more 81 (D23 pick 526); new peak 18.0
  mm on 8 samples at Z 5.0 (7 in the outer strip, 1 beyond the bbox).
  Copy op peak 15.56 mm at (2.78, -75.21, 0.5), beyond the bbox, 2.5 mm
  inside the stock's -X edge; the x 151 peak is gone (0 samples over 3.3
  mm at x > 150). Copy-op deflection proxy: the same sample, move 5379.
- Bands, Corne, D23 pick → D24: 3–9 mm 455 → **999**; 0–1.5 443 → 543;
  1.5–3 230 → 365; whole box 4793 → 5650 mm².

| | D23 pick | D24 pick |
|---|---|---|
| Corne descents (audit) | 1062 | 1291 |
| Corne lift-over descents (entries.py) | 1010 | 1194 |
| Corne plunge entries | 67 | 65 |
| Corne planar turns > 30° | 1096 | 3007 (reversals 1968; 135–180° 1639) |
| Corne plunge share, program clock | 32.3 % | 36.9 % |
| Corne cycle time | 2932 s | 3405 s |
| wanaka descents (audit) | 2031 | 2031 |
| wanaka plunge entries | 20 | 20 |
| wanaka planar turns > 30° | 177 | 180 |
| wanaka plunge share, program clock | 51.9 % | 51.9 % |
| wanaka cycle time | 3367 s | 3365 s |

After the runs: `git checkout -- .`; `git status --short` empty at
6aefca969.

## D25: D24 + C40 (entry-cause instrument) + G-CLIPAIR — RED at step 9

Fixture worktree clean at 6aefca969; `git apply` of
`phase3/d25/tree_vs_6aefca969.patch` (md5 b8202dd7df199888c908d2e1478f38f7;
7 tracked files, 934+/218−; `d25/applied.diff`). Outputs `phase3/d25/`.

- Step 1 fmt: green. Step 2 clippy plain: green.
- Steps 3 and 4 (clippy research, workspace): RED as patched, 1 error,
  `clippy::type_complexity` on the 7-element return tuple of `fn plan` in
  the C12 test (line 326). Mechanical fix, recorded: before
  `/// standing stock and (C40) its entries with their causes.` /
  `fn plan(`; after, the same with
  `#[expect(clippy::type_complexity, reason = "one tuple of the research
  records; each caller destructures it at once")]` between them. Then fmt
  and all three clippy runs green. Tree = patch + this hunk
  (`d25/applied_with_fix.diff`).
- Steps 5, 6 parity plain / research: green 5/5 each. Step 7: green 25,
  2 ignored. Step 8 harness: green; 189 files byte-equal to D24.
- **Step 9 plain C12: RED** (1 passed, 1 failed, 76.72 s).
  `the_geodesic_arm_feeds_its_turns_down_at_the_level` fails at T:1357 in
  its first mode: "geodesic_local_oneway c40 entries: 49 by kind and
  cause {"LiftOver Some(LargeZ)": 5, "LiftOver Some(Reentry2d)": 40,
  "Plunge None": 2, "Plunge Some(Reentry2d)": 1, "Plunge Some(Start)":
  1}; drop of 1 mm or more by cause {"None": 2, "Some(LargeZ)": 5,
  "Some(Reentry2d)": 41, "Some(Start)": 1}" / "geodesic_local_oneway: 2
  entries with no recorded cause (Other)". The group-filter test passes
  (D24's lines, zigzag_pick added: 1908 / 117 / 0).
- Step 11 f152 and step 12 the 13 sentries: green.
- Step 10 (heavy) and step 13 (fixtures) held for the lead.
- Restored: `git checkout -- .`; `git status --short` empty at 6aefca969.

### D25, steps 10 and 13 (as evidence, lead's order; not a commit candidate)

Tree: clean 6aefca969 + `d25/applied_with_fix.diff` (the patch and the
recorded expect hunk), checked byte for byte before each run; restored
after (`git status --short` empty at 6aefca969). Lead's rule from now on:
on a red gate, stop and report before any code edit.

**Step 10 heavy: RED** (T:1287, 238.33 s). The only failures are C40
Other = 4 in each of the 8 modes ("Plunge None", each with a drop ≥ 1
mm). Every other line is identical to D24.

**Step 13** (`phase3/fix15/run_fix15.sh`): corne_pick rc 0 (3247 s),
corne_headon rc 0 (3527 s), wanaka200_pick rc 0 (3513 s); no export
refused. Scripts in fix15 (copies of the fix14 ones, plus `c40_join.py`)
and `d25/why/` (copies of c38/d23_why extract, peak_move, clipair_base,
op1_deep on D24 pick, D25 pick, D25 headon).

C40 (per-entry lines; Other = cause None):

| run / op | entries | Other | Other % | LargeZ(+Held) % |
|---|---|---|---|---|
| Corne pick op 1 | 1188 | 37 | 3.11 | 73.6 |
| Corne pick copy op | 225 | 176 | 78.2 | 0 |
| Corne headon op 1 | 1463 | 42 | 2.87 | 81.0 |
| Corne headon copy op | 224 | 176 | 78.6 | 0 |
| wanaka pick | 2238 | 203 | 9.07 | 90.6 |

Join with entries.py (op 1 descents ≥ 1 mm removing stock, every one
matched within 0.5 mm): Corne pick 1156: LargeZ 858, Reentry2d 258, None
19, Start 13, LargeZHeld 6, AirRun 2 → LargeZ 74.7 %, **Other 1.64 %**;
Corne headon 1253: LargeZ(+Held) 79.6 %, **Other 1.52 %**; wanaka 2030:
LargeZ(+Held) 90.0 %, **Other 9.8 %**. Falsifiers (§15.3): LargeZ ≥ 50 %
on both parts (holds); Other > 1 % on both parts (**the falsifier
fires**). The Other entries: Corne op 1 Plunges at (20.31, −105.6 /
−106.18, 17), (26.31, −96.59) at Z 17 / 11 / 5, and many Plunges of drop
0.5 at Z 4.0 or drop 4.0 at Z 0.5 on a grid of x 20.31 / 53.31 / 62.31 /
71.31 / 80.31 / 89.31 / 98.31; headon adds LiftOvers at y −18.26, Z 4.0.
Copy op: Plunges at (20.31, −104.36, 17) and (24.81, −99.19) at every
level, then a 4.0 / 2.01 grid. wanaka: LiftOvers of drop 5.96 at Z 19.54
along x 22.76, x 217.24, y 27.76, y 221.74 (the frame edges).

G-CLIPAIR (§11.3 / §11.1b), D24 pick → D25 pick (headon = pick for the
copy op):
- P-CA1: copy-op deep samples (axial > 4.5) at its containment line 61
  → 38; axial peak 15.56 → 16.5 (P ≤ 4.5): **fails**. The x 151 move is
  absent in D24 and D25 (no copy-op cut on that line).
- P-CA2: no export refusal: holds. Copy-op deflection proxy peak: move
  5898 at (10.02, −89.30, 0.5), ap 16.5 mm.
- P-CA3: op 1 deep at its line 21 → 7 (P ≤ 10 %, so ≤ 2): **fails**.
  Op 1 deep in all 114 → 57 (headon 70), all in the outer strip, Z 11
  and 5, peak 18.0.
- P-CA4: 6–9 mm band 395 → 379 (P ≤ 126, the D23 value): **fails**
  against D23, not worse than D24. 3–9 mm 999 → 965.
- P-CA5: copy-op runtime 1526.8 s (P ≤ 1829): holds.
- P-CA6: parity and the sentries green: holds.
- P-CA7: wanaka cycle 3365 → 3513 s (+4.4 %, P < 2 %): **fails**; no
  new hard bar: holds. Wanaka Z 4.0 region 1 now "grown, no sweep" (was
  the split).

Other, D24 → D25 (Corne pick / headon; wanaka pick):
- C22a replayed rounds: Corne pick rounds 1 ×3, 2 ×24, 3 ×13; headon 1 /
  12 / 6; wanaka 7 / 2 / 1 (one round 3). Held links over standing stock:
  Corne 0; wanaka 33 in the kept plan (66 lines), as in D24.
- Turn summaries: Corne pick op 1 planned 222, cut 132, fed 132, unfed
  0; headon 240 / 230 / 230 / 0. **wanaka planned 331, cut 77, fed 5,
  unfed 72** (D24: 77 / 74 / 2 / 72 — the unfed 72 were already in D24
  and I did not report them then).
- Hard bars: op 1 fed link "link move 5565 removes 10.50 mm at (149.7,
  −78.7, 11.00)" (as D24); **new copy-op fed link** "link move 6340
  removes 5.52 mm at (27.6, −106.6, 0.50)" (headon move 6318). The
  deeper-than triage line is gone. Collisions 0. wanaka: none.
- Corne pick: descents (audit) 1291 → 1164 (headon 1315); lift-over
  descents 1194 → 1112 (headon 1201); plunges 65 → 65 (headon 71); turns
  over 30° 3007 → 3027 (headon 3444); plunge share 36.9 → 34.8 %
  (headon 34.1 %); cycle 3405 → 3247 s (headon 3527).
- wanaka: descents 2031 → 2035; plunges 20; turns over 30° 180 → 186;
  plunge share 51.9 → 48.4 %; cycle 3365 → 3513 s.

## D26: D24 commit 01637fb5f + C40 (waterline / clip gap fixed) + G-CLIPAIR + the held-link counter after the clip — GREEN; fixtures in fix16

Fixture worktree clean, checked out at 01637fb5f; `git apply` of
`phase3/d26/tree_vs_01637fb5f.patch` (md5 aae493f3ddec094bdb9966e62ae8af16;
5 files, 650+/66−; `d26/applied.diff`, unchanged to step 12). No edit.
Outputs `phase3/d26/`. No D25 fixture was running when the STOP came.

| Step | Result |
|---|---|
| 1–4 fmt, clippy plain / research / workspace | green |
| 5, 6 parity plain, research | green 5/5 each |
| 7 `--lib adaptive::research_geodesic` | green 25, 2 ignored |
| 8 2D harness | green; 189 files byte-equal to D25 = D24 |
| 9 plain C12 | green 2/2; Other 0 on every arm; each arm "Plunge Some(Waterline)": 2; all other lines as D24 |
| 10 heavy C12 | green (239.90 s); Other 0; D25's 4 "Plunge None" per mode are "Plunge Some(Waterline)": 4; other lines as D24 |
| 11 f152; 12 the 13 sentries | green |

**Step 13** (`phase3/fix16/run_fix16.sh`): corne_pick rc 0, corne_headon
rc 0, wanaka200_pick rc 0; no export refused. All three program.nc and
simulation.json files are byte-equal to the D25 runs (fix15); the toolpath
files differ only in debug timing. So every geometry number of D25 holds
for D26 (G-CLIPAIR P-CA1..CA7, peaks, bands, descents, plunges, turns,
plunge share, cycle). Only the log lines differ. Restored after: `git
status --short` empty at 01637fb5f.

C40 (`fix16/c40_join.py`; entries.py rows from fix15, equal runs):

| run / op | entries | Other | LargeZ(+Held) % | new causes |
|---|---|---|---|---|
| Corne pick op 1 | 1188 | 0 | 73.6 | Waterline 37 (was None) |
| Corne pick copy op | 225 | 0 | 0 | Waterline 176 |
| Corne headon op 1 | 1463 | 0 | 81.0 | Waterline 37, Clip 5 (LiftOver) |
| Corne headon copy op | 224 | 0 | 0 | Waterline 176 |
| wanaka pick | 2238 | 0 | 90.6 | Clip 197 (LiftOver 190, Plunge 7), Waterline 6 |

Join (op 1 descents ≥ 1 mm removing stock, all matched): Corne pick 1156:
LargeZ 74.2 %, Reentry2d 22.3, Waterline 1.6, Start 1.1, LargeZHeld 0.5,
AirRun 0.2; headon 1253: LargeZ 68.6, Reentry2d 17.8, LargeZHeld 11.0,
Waterline 1.4, Start 1.0, Clip 0.1; wanaka 2030: LargeZ 90.0, Clip 9.7,
Start, LargeZHeld, Waterline 0.1 each. Falsifiers: LargeZ ≥ 50 % on both
(holds); Other 0 % on both (holds).

Counter: "C22a: held link over standing stock" 0 lines in all three runs
(wanaka P 0: holds; D24 / D25 had 33 kept). C22a rounds (pick / headon /
wanaka): 1 ×3, 2 ×24, 3 ×13 / 1 ×1, 2 ×12, 3 ×6 / 1 ×7, 2 ×2, 3 ×1;
dropped_more sum 2323 (max 266) / 1140 (266) / 326 (165). D24: Corne 1 ×3,
2 ×20, 3 ×17, dropped_more 1696 (max 271); wanaka 6.

Turn summaries: Corne pick op 1 planned 222, cut 132, fed 132, unfed 0,
full_feed_unmatched 0, reduced 129, lowest 0.280, longest 2.380 mm; copy
op planned 10, cut 0. Headon op 1 240 / 230 / 230 / 0, reduced 227,
lowest 0.272, longest 2.379. **wanaka planned 331, cut 77, fed 5, unfed
72**, full_feed_unmatched 0, reduced 77, lowest 0.365, longest 3.199.

Fed-link bar (as D25): op 1 link move 5565 (D24's 6921: the same move, the
same XY and Z) is a 9-sample linking / linear move at Z 11.0 from (149.81,
−76.42) to (149.73, −78.67), no entry span (only the boundary clip span),
peak 10.5 mm; copy op link move 6340, linking / linear at Z 0.5, (28.14,
−106.30) to (27.64, −106.55), peak 5.52 mm. The held-link counter reads 0
on both, so the planner stock shows no stock there.

Planar turns over 30° / over 90° by place (`fix16/turn_place.py`; the
G-code frame moved to the model frame by the stock box minimum):
- D24 pick: inside the silhouette 848 / 274, outside 1911 / 1501, beyond
  the mesh bbox 248 / 193.
- D26 pick: 831 / 274, 1933 / 1583, 263 / 202.
- D26 headon: 1137 / 506, 2022 / 1619, 285 / 223.

## D27 (first freeze) — RED at step 3, stopped

`phase3/d27/tree_vs_2c32340ab.patch` (md5 dd501f12bd408a7fcc785e8e46b3e9f9)
on a clean 2c32340ab: 7 files, 753+/27−. Step 1 fmt green; step 2 clippy
plain green. Steps 3 and 4 (clippy research, workspace) RED, 1 error:
`clippy::mut_range_bound` at `adaptive3d/research_outer_band.rs:624`
(`cursor = k;` inside `for k in cursor..chains.len()`, before `break`).
Stopped with no edit (lead's rule). The lead re-freezes D27 with C12's fix
and C22's hole-pocket fix. Fixture worktree restored: clean 2c32340ab.

## D27 v2 / v3 and D29 (first freeze): stops

- **D27 v2** (`tree_vs_2c32340ab_v2.patch`, md5 071e6895…): RED at step 2,
  `dead_code` on `keep_down_excess` (clearing.rs:938; both callers
  cfg(research)); steps 3–4 RED, a duplicated
  `#[expect(clippy::too_many_arguments)]` on `ordered_wraps`. Stopped, no
  edit.
- **D27 v3** (`_v3.patch`, md5 eadd8e20…): steps 1–12 green (`d27/v3_*`).
  Step 9: main-test lines as D26; group-filter lines oneway 5085 / 121,
  zigzag 1839 / 118, v2 6236 / 65, spiral_pick 1889 / 106, headon 6286 /
  53, all 0 held. Step 10: green; exact bound as D26; new C40 cause
  HeldLinkRefused 1–6 per mode. **Extra step RED:**
  `the_corridor_slots_run_at_the_reduced_feed` (T:1616): "channel: half
  width 4.800 mm; slot chains 0 steps 0 …" / "the channel planned no
  corridor slot". Stopped; fixtures not run (the lead skipped them).
- **D29** (`d29/tree_vs_2c32340ab.patch`, md5 2fe3ec9e…; 24 files + new
  keep_down_route.rs): fmt and clippy plain green; steps 3–4 do not
  compile with research: "lifetime may not live long enough" at
  clearing.rs:602, `stamp_drape(&self) -> StampDrape<'a>` returning
  `route_stats: Some(&self.research_route_stats)`. Stopped, no edit.
- Fixture worktree restored to a clean 2c32340ab (keep_down_route.rs
  removed). Waiting for D29 v2.

## D29 v2 — RED at step 10 (the exact bound), stopped

`d29/tree_vs_2c32340ab_v2.patch` (md5 c154fdb46bac3a2cf84f071dc386fea5) on
a clean 2c32340ab (24 files, 1472+/75−, + keep_down_route.rs). Steps 1–9
green: fmt; clippy plain / research / workspace; parity 5/5 and 5/5;
research_geodesic 26 / 2 ignored; harness 189 files byte-equal (the render
dir as found had mtime 20:57:28, a run not mine); plain C12 2 passed.
Step 9 against D27 v3 (moves / entries): oneway 2575/49 → 2575/26; zigzag
954/47 → 948/14; v2 3122/28 → 3153/13; spiral_pick 983/43 → 979/12;
headon 3151/24 → 3184/11; LargeZ gone in every arm.

**Step 10 heavy: RED** (T:1277, 426.54 s). Over the exact bound 0.471,
D27 v3 → D29 v2: oneway 0 → 24 (peak 0.857); zigzag = zigzag_pick 0 → 55
(0.992); v2 0 → 12 (0.857); spiral_pick 0 → 51 (0.992); headon 0 → 8
(0.837); attribution only curv 4 → 12, split 2 → 10. First sample, four
arms: "move 64 ClearingCut at (-14.32, -8.90, 7.00) wall distance 11.86:
sim radial 0.754, exact peak of the move 0.856"; spiral_pick move 285 at
(1.67, -8.95, 7.00); headon move 2942 at (-29.84, 17.94, 7.00). C40
entries fall (spiral_pick 78 → 31). Stopped; no edit; slot test and
fixtures not run.

D29 v2, step 10b (lead's order: split Part A from Part B): the heavy C12
test on the same build with `RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=off` is GREEN
(221.93 s; `d29/v2_10b_heavy_route_off.txt`), no route line, and every
line equals D27 v3 step 10 (empty diff; the same moves and over counts).
The C12 test sets `step_split: Default::default()` (PathShape), so Part A
is not exercised there; the v2 over-bound samples all come from Part B.
The 20:57:28 render dir of step 8 was my own D27 v3 harness (stamp
20:54:52), not a foreign run; that remark is withdrawn.

## D29 v4: D27 v3 + Part A + fixed Part B + C12's slot fix + the A gate test — gates GREEN; fixtures fix20

`d29/tree_vs_2c32340ab_v4.patch` (md5 63c572afb65dc16d2fda61f9fcfe2896) on a
clean 2c32340ab: 24 files, 1585+/77−, + keep_down_route.rs. No edit.

- Steps 1–8 green (clippy workspace 60 s; parity 5/5, 5/5; research_geodesic
  26 / 2; harness 189 byte-equal).
- Step 9 plain C12 green. Moves / entries, D27 v3 → v4: oneway 2575/49 →
  2565/44; zigzag 954/47 → 940/39; v2 3122/28 → 3116/25; spiral_pick 983/43
  → 969/35; headon 3151/24 → 3145/21. LargeZ gone in every arm.
- Step 10 heavy green (224.54 s): 0 over on the G1 arms (lines identical to
  D27 v3); curv 2573 → 2669 moves, split 2414 → 2576. The route lines are
  tracing events: the test prints none.
- Steps 11, 12 green. Slot test GREEN: "slot chains 2 steps 116 reduced 108
  fed 116 unfed 0 …", "444 samples on slot moves, 0 over the screen".
- A gate test GREEN (35.21 s): every arm "step split made 0, avoided 3–11,
  max ap avoided 5.500 mm; route tried 0", 0 over.

**Fixtures** (`fix20/run_fix20.sh`; columns a_only ROUTE=off, cut_feed):
Corne a_only and cut_feed program.nc are byte-equal (the router took 0).

| | Corne D26 pick | Corne a_only (= cut_feed) | wanaka D26 | wanaka a_only | wanaka cut_feed |
|---|---|---|---|---|---|
| cycle (CLI) | 3247 s | 2458 s | 3513 s | 1991 s | 1991 s |
| descents (audit) | 1164 | 200 | 2035 | 340 | 339 |
| lift-over descents (entries.py) | 1112 | 157 | 2016 | 349 | 349 |
| plunge-entry spans | 65 | 62 | 20 | 19 | 19 |
| plunge share (program clock) | 34.8 % | 13.6 % | 48.4 % | 31.5 % | 31.5 % |
| planar turns > 30° | 3027 | 2201 | 186 | 1451 | 1454 |
| reversals (> 90°) | 2059 | 390 | 18 | 126 | 126 |
| over 0.471 per / body / wl (mm) | 2.9 / 474.6 / 303.3 | 185.4 / 794.7 / 297.5 | 0 / 2209.7 / 15.7 | 2121.0 / 1223.9 / 20.1 | — |
| thick over 0.471 total | 399.1 | 763.8 | 463.3 | 638.3 | — |

- Step split: Corne made 0, avoided 2527, max ap avoided 6.0; wanaka made
  496, avoided 4201, max 5.46. Route (cut_feed): Corne tried 24 / 372,
  taken 0, refused_into_stock 12 / 363; wanaka tried 774, taken 5,
  refused_into_stock 731, travel 31.5 mm, side 1.6 mm.
- C40 (Other 0 everywhere): Corne op 1 descents 198: Reentry2d 77.3 %,
  Waterline 9.6, Start 8.1, HeldLinkRefused 4.0, AirRun 1.0; **LargeZ 0 %**.
  wanaka 361: Reentry2d 41.6, LargeZ 40.4, LargeZHeld 11.4,
  HeldLinkRefused 5.5 (LargeZ(+Held) 51.8 %).
- Turns over 30° / over 90° by place, Corne: D26 pick inside 831 / 274,
  outside 1933 / 1583, beyond 263 / 202; D29 inside 1767 / 313, outside 308
  / 34, beyond 126 / 43.
- Turn summaries: Corne op 1 turns 222 / 132 / 132 / 0 unfed; slots 2655 /
  2561 / 2561 / 0; copy op slots 2648 / 2571 / 2571 / 0; wanaka turns 80 /
  76 / 76 / **0 unfed** (D26: 72); slots 93 / 23 / 23 / 0; clipped 0 in all.
  But "C38 slot chains": Corne op 1 steps 2561, fed 444, **unfed 1604**;
  copy op steps 2571, fed 329, **unfed 2130**; wanaka steps 23, fed 23,
  unfed 0.
- Hard bars: **Corne 3 rapid-through-stock collisions** ("Rapid passes
  through uncut stock on TP1 '3D Rough 1' at local move 3701 / 6704 / 9786:
  (91.3, -1.1, 28.000) -> (91.3, -1.1, 17.95 / 17.62 / 17.57)"). No fed-link
  line, no deeper-than line; depth-of-cut advisories op 1 13.0 mm (Corne),
  9.18 mm (wanaka). No export refused.
- Band, Corne 3–9 mm: D25/D26 965 → 453 mm²; 0–1.5 525 → 421.
- Fixture worktree restored: clean 2c32340ab.

## D30: feed cap + level-aware chain match + Part A width read + start-entry probe — gates green (2 known pre-existing reds, 1 known A-test red); fixtures fix21

`d30/tree_vs_3181ae54f.patch` (md5 9368abb799a249bbf49e15338195d91b) on a
clean 3181ae54f: 28 files (433+/55−) + the new
`tests/a_feed_cap_binds_every_feed_stage_c38.rs`. No edit.

- Green: fmt; clippy plain / research / workspace; parity 5/5 and 5/5;
  research_geodesic 26 / 2; harness 189 byte-equal to D29 v4; plain C12
  (identical to D29 v4); heavy C12 (identical to D29 v4); f152; the 13
  adaptive3d sentries; 16 of 17 dressup sentries (incl. the new
  a_feed_cap_binds_every_feed_stage_c38 4/4, constrained_max_modulation_f039
  6, plunge_guard_p3 7, chipload_boundary_g_chip_ulp 6); cli 92 passed.
- Slot test green; its chain line now reads "slot chains 0 steps 0 …"
  (D29 v4: 2 chains, 116 steps); accepted by the lead (the exact match
  feeds every record).
- **Known red (patch-new assert, accepted by the lead):** the draped-step
  A test: 0 over in every arm, but "no arm had a step the width read
  split" (every arm "made 0 (by width 0)").
- **Known pre-existing reds (proven on a clean 3181ae54f, patch removed and
  re-applied byte for byte):** `tabs_sit_on_straight_sides_p5`
  (tabs_lifts_match_apply_tabs_for_the_same_points_p5, :444 "S11: the
  fixture sets it"; `d30_12b_tabs_on_clean_base.txt`; from 74af0f8e1) and
  viz `an_unbacked_cell_still_adds_its_operation_fm5` (2 fails, :142
  "anchor: the MCP add door has an Unbacked arm", :100 the toast text;
  `d30_16b_fm5_on_clean_base.txt`). Viz otherwise 631 passed, 2 ignored.

**Fixtures** (`fix21/run_fix21.sh`): a_only and cut_feed program.nc are
byte-equal on both parts (Corne route tried 1596 / 24, taken 2 / 0,
refused 1327 / 12; wanaka tried 2501, taken 1, refused 2260).

| | Corne D26 | Corne D29 | Corne D30 | wanaka D26 | wanaka D29 | wanaka D30 |
|---|---|---|---|---|---|---|
| cycle (CLI) | 3247 s | 2458 s | 2996 s (P 2600–2950) | 3513 s | 1991 s | 2693 s (P 2350–2950) |
| over 0.471 (mm) | 780.8 | 1277.7 | 1069.6 (P ≤ 820) | 2225.3 | 3365.1 | 2858.2 (P ≤ 2337) |
| thick over 0.471 | 399.1 | 763.8 | 661.6 (P ≤ 419) | 463.3 | 638.3 | 537.3 (P ≤ 486) |
| lift-over descents (entries.py) | 1112 | 157 | 745 (P 350–700) | 2016 | 349 | 1160 (P 700–1400) |
| descents (audit) | 1164 | 200 | 788 | 2035 | 340 | 1154 |
| plunge-entry spans | 65 | 62 | 62 | 20 | 19 | 19 |
| plunge share (program clock) | 34.8 % | 13.6 % | 23.4 % | 48.4 % | 31.5 % | 41.5 % |
| planar turns > 30° | 3027 | 2201 | 1500 | 186 | 1451 | 924 |

- Over 0.471 split (`fix21/load_split.py`): Corne D29 → D30: near A's steps
  1003.2 → 562.9; slot moves 39.8 → 344.7; turn moves 27.8 → 26.5; other
  206.9 → 135.5. wanaka: near A's steps 2558.3 → 2040.6; slots 5.7 → 1.5;
  turns 40.1 → 41.6; other 761.1 → 774.4.
- Feed check on the program (`fix21/feed_check.py`): Corne 118 capped turn
  moves, 677 samples, 0 above the cap at program feed (D29: the same moves
  at F 4000). Strip samples over 0.369 at program F ≥ 3999: 1554 → **87**;
  over 0.219: 1895 → **105** (P 0: fails). wanaka 80 matched, 3 samples
  over: all on move 16906, a helix whose sample feed is the base 1000 (not a
  cap), a false match of the 1.05 mm XY tolerance; so 0 real breaks.
- Lines: "C38 feed caps exact=2124 chain=0" (op 1), "exact=2452 chain=0"
  (copy), wanaka "exact=83 chain=0"; "C38 slot chains loops=0 steps=0 …" on
  every op; "C22 step split (stock-read rule) made=1439 width_split=1439
  avoided=1088 max_ap_avoided_mm=6.000" (Corne), "made=2493
  width_split=1995 avoided=2274 max_ap_avoided_mm=5.460" (wanaka). Turn
  summaries: Corne op 1 turns 222/132/132/0, slots 2639/2553/2553/0; copy
  slots 2648/2571/2571/0; wanaka turns 83/76/76/0, slots 67/0/0/0.
- C40 (Other 0): Corne op 1 descents 786: LargeZ 74.0 %, Reentry2d 19.3;
  wanaka 1172: LargeZ 81.7 %, Reentry2d 12.8, LargeZHeld 3.5.
- Turns > 30° / > 90°, Corne: inside 1176 / 271, outside 219 / 34, beyond
  the bbox 105 / 42.
- Start-entry probe at (91.30, −1.13): Z 11 entry_z 11.0, floor_top 17.451,
  centre_top 17.0; Z 5 floor_top 17.119, centre_top 11.283; Z 0.5 floor_top
  17.073, centre_top 5.296. **The 3 rapid-through-stock collisions remain**
  (moves 4803 / 7810 / 10892: (91.3, −1.1, 28.0) → 17.951 / 17.619 /
  17.573). floor_top (17.45 / 17.12 / 17.07) is under the depth the rapid
  ended (17.95 / 17.62 / 17.57) by 0.50 mm each time: by §16.10 the planner
  stock there is lower than the sim's.
- Hard bars: rapid collisions 3 (Corne); no fed-link or deeper-than line;
  depth-of-cut advisories op 1 13.0 mm (Corne), 9.18 mm (wanaka); no export
  refused. Band, Corne 3–9 mm: 453 (D29 453).
- Disk: 72 → 40 GB free during D30 (the fixture worktree target is 36 GB
  after the viz build). Fixture worktree restored: clean 3181ae54f.

## D31: D30 + C22 §16.11 (width-read descent fix, shelf test) — gates green (known tabs red); fixtures fix22 (cut_feed)

Before the build: `cargo clean -p rs_cam_viz` in the fixture worktree
(lane) removed 1814 files, 25.7 GiB; target 36 → 10 GB; free 40 → 65 GB.
`d31/tree_vs_3181ae54f.patch` (md5 e01c11b6a08943450a984d06ee022a07) on a
clean 3181ae54f: 29 files (550+/64−) + the a_feed_cap test. No edit.

- Green: fmt; clippy plain / research / workspace; parity 5/5, 5/5;
  research_geodesic 26 / 2; harness byte-equal to D30; plain C12 and heavy
  C12 identical to D30; f152; 13 adaptive3d sentries; dressup sentries
  except the known pre-existing tabs_sit_on_straight_sides_p5 (9 / 1); slot
  test (as D30). cli and viz skipped (brief).
- Draped-step A test green: 0 over in six arms; "made 0 (by width 0)".
- Shelf test `the_width_read_splits_a_wide_draped_step` green: oneway
  "made 37 (by width 37), avoided 24", base 2786 moves / 5 over (0.990) →
  2732 / 5 over (0.990); spiral_pick "made 69 (by width 69), avoided 57",
  base 1141 / 7 over (0.994) → 1090 / **9 over samples** (0.988). The test
  asserts on over moves, not samples.

**Fixtures** (cut_feed; `fix22/run_fix22.sh`):

| | Corne D26 | Corne D30 | Corne D31 | wanaka D26 | wanaka D30 | wanaka D31 |
|---|---|---|---|---|---|---|
| cycle | 3247 s | 2996 s | 3011 s | 3513 s | 2693 s | 2694 s |
| over 0.471 (mm) | 780.8 | 1069.6 | 1065.9 | 2225.3 | 2858.2 | 2856.0 |
| thick over | 399.1 | 661.6 | 661.8 | 463.3 | 537.3 | 538.1 |
| lift-over descents (entries.py) | 1112 | 745 | 802 | 2016 | 1160 | 1163 |
| descents (audit) | 1164 | 788 | 845 | 2035 | 1154 | 1156 |
| plunge entries | 65 | 62 | 62 | 20 | 19 | 19 |
| plunge share | 34.8 % | 23.4 % | 24.7 % | 48.4 % | 41.5 % | 41.6 % |
| planar turns > 30° | 3027 | 1500 | 1382 | 186 | 924 | 912 |

- Load split D30 → D31: Corne near A 562.9 → 559.2, slots 344.7 → 344.7,
  turns 26.5, other 135.5; wanaka near A 2040.6 → 2038.4, others equal.
- Step split: Corne "made=1630 width_split=1630 avoided=897
  max_ap_avoided_mm=6.000" (D30 1439 / 1439 / 1088); wanaka "made=2505
  width_split=2007 avoided=2262 max_ap_avoided_mm=5.460" (D30 2493 / 1995 /
  2274). Route: Corne tried 1760 / 24, taken 2 / 0; wanaka tried 2507,
  taken 1.
- Feed check as D30: Corne 0 above the cap; strip over 0.369 at F ≥ 3999
  87, over 0.219 105; wanaka 0 real breaks (the same 3 false-match samples).
- Feed caps, slot chains, turn summaries: as D30.
- Probe at (91.3, −1.13): as D30 (floor_top 17.451 / 17.119 / 17.073).
  **The 3 rapid-through-stock collisions remain** (moves 4849 / 7856 /
  10938, → 17.951 / 17.619 / 17.573).
- Hard bars: rapid collisions 3; no fed-link or deeper-than line;
  depth-of-cut advisories 13.0 mm (Corne), 9.18 mm (wanaka); no refusal.
- Fixture worktree restored: clean 3181ae54f.

## D32: D31 + C12 departures + C22 probes — gates green (known tabs red); fixtures skipped (lead)

`d32/tree_vs_3181ae54f.patch` (md5 8d6224a1d3553a0e438e1ab8f1f888f6) on a
clean 3181ae54f: 31 files (1206+/100−) + the a_feed_cap test. No edit.
- Green: fmt; clippy plain / research / workspace; parity 5/5, 5/5;
  research_geodesic 26 / 2; harness byte-equal to D31; f152; 13 adaptive3d
  sentries; dressup sentries except the known pre-existing tabs red (9 / 1);
  cli 92 passed.
- Plain C12 green: only move counts change against D31 (oneway 2565 →
  2905, zigzag 940 → 1344, v2 3116 → 3392, spiral_pick 969 → 1331, headon
  3145 → 3379); entries, C40, turns and openings unchanged.
- Heavy C12 green (232.69 s): G1 0 over; moves oneway 5943, zigzag 2801,
  v2 6933, spiral_pick 2767, headon 6899; curv 4 → 6 over (0.994,
  attribution only), split 2.
- Slot test green: "departure records 8 (reduced 8), fed 8, unfed 0";
  "8 moves on departure windows"; "444 samples on slot moves, 0 over".
- Draped-step A test green (0 over in six arms). Shelf test green: oneway
  made 37 by width, base 6 over / A 6 over (0.994); spiral_pick made 68,
  base 9 over / A 11 over samples (12 over the screen).
- Fixtures skipped by the lead (D33 follows). Fixture worktree restored:
  clean 3181ae54f.

## D33: D32 + C22's step-split switch (default path rule) + C12's §15.8 departure groups — gates green (known tabs red); fixtures fix23

`d33/tree_vs_3181ae54f.patch` (md5 700a64ed3c57006f1dc561ce5601a104) on a
clean 3181ae54f: 31 files (1327+/112−) + the a_feed_cap test. No edit.
- Green: fmt; clippy ×3; parity 5/5, 5/5; research_geodesic 26 / 2;
  harness byte-equal to D32; f152; 30 sentries but the known tabs red; cli
  92. Plain C12 moves within C12's P (D31 + 2 per entry): oneway 2585,
  zigzag 966, v2 3128, spiral_pick 994, headon 3156; entries and C40 as
  D27 v3 (LargeZ back). Heavy green: G1 0 over; curv 6, split 2. Draped-step
  A test green; shelf test green (spiral_pick 9 → 11 over samples).
- **Slot test (green) changed:** "departure records 8 (reduced 8), fed 0,
  unfed 8" and "2 moves on departure windows" (D32: fed 8, unfed 0; 8
  moves); "slot chains 2 steps 8 reduced 8 fed 8 unfed 0 lowered 2 …".

**Fixtures** (`fix23/run_fix23.sh`; router CutFeed in both columns):

| | Corne D26 | D31 | D33 default | D33 stock_depth | wanaka D26 | D31 | D33 default | D33 stock_depth |
|---|---|---|---|---|---|---|---|---|
| cycle | 3247 | 3011 | 3165 | 3039 | 3513 | 2694 | 3403 | 2708 |
| over 0.471 (mm) | 780.8 | 1065.9 | 1044.1 | 1058.8 | 2225.3 | 2856.0 | 2558.4 | 2864.0 |
| thick over | 399.1 | 661.8 | 676.0 | 657.1 | 463.3 | 538.1 | 531.5 | 537.3 |
| lift-over descents | 1112 | 802 | 1019 | 802 | 2016 | 1163 | 1967 | 1163 |
| descents (audit) | 1164 | 845 | 1065 | 845 | 2035 | 1156 | 1961 | 1156 |
| plunge entries | 65 | 62 | 62 | 62 | 20 | 19 | 19 | 19 |
| plunge share | 34.8 % | 24.7 % | 28.6 % | 24.3 % | 48.4 % | 41.6 % | 48.0 % | 41.3 % |
| turns > 30° | 3027 | 1382 | 1094 | 1397 | 186 | 912 | 370 | 916 |

- Load split (default / stock_depth): Corne near A 498.9 / 514.2, slot
  moves 387.6 / 387.6, turns 26.5, other 131.1 / 130.5; wanaka near A
  1759.6 / 2038.8, slots 5.8 / 8.3, turns 34.9 / 41.6, other 758.2 / 775.3.
- Feed check: Corne capped turns 0 above the cap; strip over 0.369 at F ≥
  3999: **30** (D31 87; P ≤ 1 fails); over 0.219: 48. wanaka: 0 real
  breaks (3 false-match helix samples, sample feed 1000).
- Departures ("C22 turn summary kind=departures"): Corne op 1 cut 2537,
  fed 128, unfed 851, full_feed_unmatched 1558, reduced 947 (default;
  stock_depth 126 / 852 / 1559); copy op cut 397, fed 0, unfed 169,
  full_feed_unmatched 228; wanaka cut 2178, fed 378 / 286, unfed 497 / 508,
  full_feed_unmatched 1303 / 1384. "C22 turn not fed kind=Departure" lines:
  Corne op 1 1087 per column.
- Slot chains: Corne op 1 loops 184, steps 2409, fed 1322, unfed 422,
  lowered 135; copy loops 13, steps 397, fed 383, unfed 14; wanaka loops
  236 / 200, steps 1800 / 1892, unfed 70 / 52. Feed caps: Corne op 1
  exact 2218 chain 135; copy 2452 / 54; wanaka 130 / 138 (125 / 148).
- Step split (stock_depth only): Corne made 1630, width 1630, avoided 897;
  wanaka made 2505, width 2007, avoided 2262. Default: no line.
- Probes at (91.3, −1.13), default column: "C22 start cell probe z=11.0
  x=91.298 y=-1.128 entry_z=11.000 top=23.000 cell_x=89.814 cell_y=1.684
  centre_top=Some(17.330778121948242) touches=3"; z=5.0 "top=17.119 …
  centre_top=Some(14.976056098937988) touches=4"; z=0.5 "top=17.073 …
  centre_top=Some(11.26547908782959) touches=5"; "C22 short cut probe
  z=11.0 points=2 xy_len=0.0952 x=91.393 y=-1.121 z_lo_raw=11.000
  z_hi_raw=11.000 stamped=true tool_z=Some("11.000")". Rapid collisions:
  **default 2** (moves 8528 / 11619 → 17.619 / 17.573; the Z 11 one is gone,
  its floor_top now 23.0); **stock_depth 3** (as D31).
- Hard bars: no fed-link or deeper-than line; depth-of-cut 12.0 mm
  (default) / 13.0 mm (stock_depth) Corne, 9.18 mm wanaka; no refusal.
- Fixture worktree restored: clean 3181ae54f.
