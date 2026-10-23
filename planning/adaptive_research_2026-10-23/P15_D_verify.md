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
