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
