# C2, G-FEEDMAX: pre-registration (written before the code change)

Date: 2026-10-28. Tree: `geodesic-integration` at `e46edcc53` (C1), with
only temporary `eprintln!("PREREG-FEEDMAX …")` lines in the two modulator
call sites (the post-sim pass, `session/compute/simulation.rs`, and the
strategy advisor, `session/compute.rs::optimized_candidate`). Each line
names an op whose modulated feeds (`ModulationOutcome::per_move`) go above
its `DressupConfig::feed_max_rate`. The operator approved the run.

## Method

- Batch 1: the 112 core test binaries that name modulation and need no
  `heavy-tests` or `research` feature, plus the sim goldens
  (`perf_golden_sim_metrics`, `smoke_baseline_regression_f037`,
  `feed_explanation_snapshot_b3`, `gcode_validator_baseline`).
  `--no-fail-fast -- --nocapture`. The k-th `running` block is the k-th
  name in sorted order (112 blocks, 112 names).
- Batch 2: the 4 viz test binaries that name modulation; every
  `rs_cam_cli` test.

## Hits: the tests whose modulated feeds change

| Test binary | Op (path) | Rate | Moves above | Max feed today |
|---|---|---|---|---|
| `a_rest_op_slower_than_fresh_is_told_r3` | `prior`, `case` (sim) | 3000 | 289, 31 | 4000 |
| `arc_fit_disposition_a5` | `A3D-1 … raised cutting ceiling`, `A3D-2 … stock ceiling`, `Rough (upstream, fresh stock)` (sim) | 3000 | 31 to 80 | 6912, 6000, 4196 |
| `job_rows_recommend_and_preview_wp14a` | `advisor adaptive3d` (advisor) | 3000 | 45, 94 | 4000 |
| `modulated_feeds_read_their_own_prefix_b5` | `Pocket`, `Rest 2`, `Rest 3`, `Finish` (sim) | 3000 | 1 to 75 | 4000 |
| `the_adopt_modulates_as_run_simulation_does_f115` | `Pocket` (sim) | 3000 | 123 | 4000 |

No golden and no fixture file hits. The four sim goldens print nothing
(`perf_golden_sim_metrics` runs with modulation off). No viz test hits.

Prediction: these five tests assert relations (a feed moved, two paths
agree, a ranking), not pinned feed values. So they stay green after the
change, and no golden or fixture needs a re-bless. If one goes red, it is
a pinned value and goes in the re-bless list with old → new.

Not observable: the `rs_cam_cli` tests start the CLI as a child process
and capture its stderr. They all pass today. The run after the change
shows whether any of them pins a modulated feed.

## Base reds (with the prints only; not caused by C1 or C2)

- S11 `NotSet` feeds in the fixture (the G-MASTERREDS-1027 class):
  - `adaptive3d_lift_bridge_b1`
  - `axial_engagement_vs_dpp_detector_f2`
  - `drill_runtime_survives_retime_n2` (5)
  - `retime_respects_no_kinematics_n7` (6)
  - `simulation_issue_channel_m1` (2)
  - `modulated_feeds_read_their_own_prefix_b5` (1 of its tests)
- `perf_golden_sim_metrics::sim_metrics_match_golden`: 6 Profile fields
  (air-cut time 1.9083 against the golden 1.9295). Red with the same
  numbers at `f098d1623`, before C1 (a temporary worktree, removed).
  Modulation is off in this golden.
