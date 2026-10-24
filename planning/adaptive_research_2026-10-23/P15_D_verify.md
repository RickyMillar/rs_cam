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
