# Removal step R1: fixtures off the removed strategies, new op-golden rows

Date: 2026-10-28. Branch `geodesic-integration`, base `a151149fd` (D65).
The implementer did not commit. Design: `removal_design.md` §1.5, §5.

Patch: `stepR1_tree_vs_a151149fd.patch`, md5
`981cf94a93ab95a2b03c1ff94a22dde2` (re-frozen after the lead's rulings; the freezes `7851608f…` and
`87c723e4…` are void). It holds 18 file entries: the two
renamed fixtures (delete and add), the two other fixtures, the op-golden
test and file, the `wanaka100/README.md`, the F-034 test (the re-recorded
pin) and nine test files whose
references or doc comments changed. No planning file. `git apply --check
-R` passes.

## 1. Fixtures

| Fixture | Op | Old → new | Why this value |
|---|---|---|---|
| `wanaka_airrun_2026-06-01_2c908dca.toml` | Back Rough, 3D Rough 6 | `agent_search` → `geodesic_adaptive` | its rough sentry (G-PHANTOMSTAMP, 3D Rough 6) is a slice-pipeline claim; Geodesic Adaptive is the one strategy left on that pipeline |
| `wanaka_2026-08-16_f530995a.toml` | Back Rough, 3D Rough 6 | `agent_search` → `geodesic_adaptive` | a pinned, non-default strategy, as before. `contour_parallel` was tried first: it is the unpinned default, so Suggest heuristic B fired on Back Rough and `wanaka_suggest_baseline` failed ("unexpected StrategyRecommendedNotApplied … recommended: adaptive"). That is a change of the test's subject, so the value is `geodesic_adaptive` |
| `wanaka100/wanaka_full_tuned.toml` | Back Rough, 3D Rough 6 | `contour_spiral` → `geodesic_adaptive` | the constant-engagement successor |
| `test_job.toml` | 3D Rough 5 | `agent_search` → `geodesic_adaptive` | the job keeps one op of each kept strategy (Rough bottom stays `contour_parallel`) |

No other key changed.

Renames (lead answer 2): the two dated fixtures take the sha256 prefix of
their new content.

| Old name | New name | sha256 of the new file |
|---|---|---|
| `wanaka_airrun_2026-06-01_2c908dca.toml` | `wanaka_airrun_2026-06-01_bb713634.toml` | `bb71363469205c96…` |
| `wanaka_2026-08-16_f530995a.toml` | `wanaka_2026-08-16_afc6ecd4.toml` | `afc6ecd4cee06077…` |

The old prefixes were of the planning source file, not of the fixture (the
fixture at `a151149fd` hashes to `c7f9335f…` and `2fb9005f…`): the model
paths had already moved to `wanaka100/` (`wanaka100/README.md`). Every
reference moved (8 test files, the README; `rg` finds no old name in
`crates/`). The doc comments now say "derived from <old prefix> (the
planning file); R1 changed only `clearing_strategy` `agent_search` →
`geodesic_adaptive`, because the strategy was removed" in
`p1_headless_ab_wanaka.rs`, `wanaka_suggest_integration.rs`,
`suggest_feed_matches_final_geometry.rs`, the G-PHANTOMSTAMP test (which
also says that its numbers are of the AgentSearch chain) and
`wanaka100/README.md`. The renamed loaders run as before (8 binaries,
including `gate_tests_read_no_operator_paths_f155`).

## 2. The loaders

Every non-heavy loader of the four fixtures, run before and after on the
same build (21 binaries, `--features test-support`, plus `rs_cam_cli
--test integration`): the results are the same, line for line. No pin
moved, so no re-pin.

- Before and after: `simulation_issue_channel_m1` has 2 reds
  (`engagement_is_unmeasurable_below_the_fresh_material_floor`,
  `issue_channel_census_synthetic_2d`). They are red on the base
  `a151149fd`, before any fixture edit: not caused by R1.
- The loaders that read the rough op's numbers are evidence or instrument
  runs, `#[ignore]` (G-PHANTOMSTAMP wanaka chain, `p1_headless_ab_wanaka`,
  `kinematics_histogram`, `wanaka_step4/5_*`, `wanaka_axial_doc`,
  `wanaka_boundary_diag`, `wanaka_z_layer_render`, `tier_trial_350`,
  `agent_search_axial_doc`). They were not run. Their documented numbers
  came from AgentSearch or ContourSpiral and now describe a different
  plan; each is a re-measure when someone runs it.
- Heavy gates, run with the operator's approval (foreground, worktree
  lane):
  - `feed_modulation_cycle_time_f036c`: 2/2 ok (256 s). Model modulated
    1609 s against the bench 1224 s (ratio 1.315, envelope [0.45, 1.7]);
    modulated / unmodulated 1609 / 1273 s = 1.264 against the bench 1.480
    (envelope [0.5, 2.3]).
  - `machine_kinematics_cycle_time_f034`: 4 ok, 1 RED,
    `cycle_time_calibrated_against_shapeoko_reference`. Model 1272.6 s
    against the bench 827 s (ratio 1.539, inside its anchor band
    [0.30, 1.6]); the P1 sentry (e) pin `P1_RECORDED_PREDICTION_S = 1053.0`
    (recorded 2026-09-07 on the ContourSpiral Back Rough) reads +219.6 s,
    tolerance ±25 s. The test text: "If the generator moved, re-record".
    The generator moved by R1 (Back Rough is Geodesic Adaptive).
    LEAD RULING: re-record. `P1_RECORDED_PREDICTION_S` 1053.0 → 1272.6 s,
    with a dated comment ("R1: Back Rough changed from contour_spiral
    (removed) to geodesic_adaptive; the old pin 1053.0 s was the
    ContourSpiral path"). No envelope or band changed. The op's prediction
    on the two generators: ContourSpiral 1053.0 s, Geodesic Adaptive
    1272.6 s (+21 %). Re-run: 5/5 ok (102 s).
  - `finish_planner_wanaka_diagnostics` (heavy): all 3 tests ignored; not
    run.

Baseline constants measured on the old chain:

| Constant | Test | Ran on the new fixture? | Read |
|---|---|---|---|
| `BASELINE_RAPID_COLLISIONS = 0` | `p1_headless_ab_wanaka` | no: `#[ignore]` evidence (several full simulations, minutes); not approved | — |
| `MIN_RAPID_END_Z` (22.9) and the zero-collision claim | G-PHANTOMSTAMP wanaka chain | no: `#[ignore]` evidence (420 s release); not approved | — |
| `MEASURED_MODULATED_S = 1224`, `MEASURED_UNMODULATED_S = 827`, ratio envelopes | `feed_modulation_cycle_time_f036c` | yes | green, numbers above |
| `BACK_ROUGH_MEASURED_S = 827`, `P1_RECORDED_PREDICTION_S` | `machine_kinematics_cycle_time_f034` | yes | 827 s: inside its band (ratio 1.539); the P1 pin re-recorded 1053.0 → 1272.6 s (lead ruling), then green |
| `PIN_DRILL_MEASURED_S = 65` | `machine_kinematics_cycle_time_f034` | yes (same test) | not asserted (the constant documents a bench reading only); the pin drill is not a rough op |
| the Suggest dial baseline (R4 WP3, B6 force and power) | `wanaka_suggest_integration` | yes | green (Suggest does not read the strategy once it is pinned) |
| tier-trial constants | `tier_trial_350` | no: instrument, `#[ignore]` | — |

The other evidence runs of §2 carry no constant, only printed numbers.

## 3. New op-golden rows

`every_op_keeps_its_toolpath_g_opgolden.rs` gains two W6.5.0 arms; the
golden file gains two rows and no other line (`git diff`: +28, 0 removed;
the bless rewrote the file and every old row came back equal).

| Row | Settings | Pins |
|---|---|---|
| `adaptive3d/by_area_helix_cp` | By Area, helix entry, Contour Parallel | 651 moves (6 rapid, 386 linear, 259 arc), cutting 2397.741 mm, moves hash `53ac4fc26e5fc900`, 331 spans `7cf02096aabac105` |
| `adaptive3d/ramp_geodesic` | ramp entry, Geodesic Adaptive | 1122 moves (9 rapid, 1100 linear, 13 arc), cutting 2428.636 mm, moves hash `4ff4b73d11cca590`, 95 spans `e39798b0dd140947` |

A probe (a temporary print in `common/op_fixtures.rs`, reverted; `git
diff` of the file is empty) read what the rows hold: `by_area_helix_cp` has
319 `EntryHelix` moves, `ramp_geodesic` 108 `EntryRamp` moves and 363
capped moves. So the rows keep the helix, ramp and By Area coverage of the
rows that go in R3/R4 (`by_area_helix_spiral`: 414 helix; `ramp_agent`: 65
ramp), and add the cap. The three old rows stay until R3/R4.

## 4. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy `rs_cam_core` all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6 (CP and Geodesic unchanged) |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1 (new rows blessed; old rows equal) |
| the 21 fixture loaders, `rs_cam_cli --test integration` | same results before and after (§2) |

## 5. Next

R2: the config layer and the load rule (refuse at load), as
`removal_design.md` §3.2 and §5.
