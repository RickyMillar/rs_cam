# C2, G-FEEDMAX: the feed modulator obeys `feed_max_rate`

Date: 2026-10-28. Branch `geodesic-integration`, base `5b4a561ae` (the
commits since C1 change no core code). The implementer did not commit.

Patch: `c2_feedmax_tree_vs_5b4a561ae.patch`,
md5 `a1ace97a8fc285ec8b43ad28c3ea008f`. 8 files (7 changed, 1 new). No
viz file, no I5 file. `git apply --check -R` passes. The temporary
pre-registration prints are removed (`PREREG` occurs 0 times in the patch).
Pre-registration: `c2_feedmax_PREREG.md`
(md5 `4c7ff80ceaba5b3797c92708f96ae315`), written before the change.

## 1. Change

- `machine/mod.rs`: `CuttingCeilingMmMin::capped_at(Option<f64>)`. It
  lowers the ceiling to the op's cap when the cap is finite and above
  zero, else it returns the ceiling. It never raises, so F002 holds: a
  cut is still no faster than the machine.
- `session/compute/simulation.rs` (the post-sim pass): each toolpath's
  ceiling is `max_feed.capped_at(tc.dressups.feed_max_rate)`.
- `session/compute.rs` and `session/compute/params.rs` (the strategy
  advisor): `AdvisorContext::feed_max_rate`, from the toolpath's dressups;
  `optimized_candidate` applies `capped_at` before `at_least(1.0)`. So the
  advisor times the feeds the post-sim pass gives.
- `dressup/feed_modulation.rs`: the ConstrainedMax arm with no engagement
  kept the commanded feed; it now takes `min(commanded, ceiling)`. Every
  arm and the kinematic probe read the lowered ceiling. The binding tag
  stays `MachineMaxFeed` (no new wire variant).
- `Move::feed_cap` still binds last and only lowers
  (`a_feed_cap_binds_every_feed_stage_c38` 4/4).
- Docs: `DressupConfig::feed_max_rate` ("caps every feed stage … only
  lowers"), the `DressupFieldDef` description (the MCP and schema help:
  "upper cutting feed (mm/min); caps every feed stage (the optimiser and
  the feed modulation after a simulation); null = the machine maximum
  feed"; no snapshot pins it), `ModulationContext::cutting_ceiling`, the
  `CuttingCeilingMmMin` type doc, `dressup/CLAUDE.md` (40 lines).

Not changed (viz, ask first): the GUI shows the Max Rate field only when
"Feed rate optimization" is on (`ui/properties/linking_dressup.rs:645-660`).
With C2 the dial also caps the modulation, so the GUI must show it always.

## 2. Pre-registration against the result

| Pre-registered test (moves above 3000 today) | After |
|---|---|
| `a_rest_op_slower_than_fresh_is_told_r3` (289, 31) | 2/2 ok |
| `arc_fit_disposition_a5` (31 to 80; max 6912) | 3/3 ok |
| `job_rows_recommend_and_preview_wp14a` (advisor, 45 and 94) | 5/5 ok |
| `modulated_feeds_read_their_own_prefix_b5` (1 to 75) | 1 ok, 1 red: the same S11 `NotSet` base red |
| `the_adopt_modulates_as_run_simulation_does_f115` (123) | 2/2 ok |

The prediction held: no test pins a modulated value above 3000, so
**nothing is re-blessed**. No golden and no fixture changes.

## 3. New sentry

`no_modulated_feed_exceeds_feed_max_rate_g_feedmax` (2/2), on the F115
pocket (ConstrainedMax, Shapeoko XXL kinematics, commanded 770 mm/min):

1. Non-vacuity: with `feed_max_rate = None` the run lifts a feed above
   3000. With 3000 no fed move is above 3000, and some feed is still above
   770.
2. A rate of twice the machine ceiling gives the same moves as `None`.
   `capped_at` ignores `None`, NaN and 0, and gives min(rate, ceiling).

Planted red: the post-sim pass takes `capped_at(None)`. Claim 1 fails with
"123 fed moves above feed_max_rate 3000: max 4000" (the pre-registered
count). File restored.

## 4. Gates

| Gate | Result |
|---|---|
| batch 1 after the change (112 core binaries, `--no-fail-fast`) | the base reds below, and nothing else |
| the dressup sentries in it: c38, f039, f040, p3, f036b | 4/4, 6/6, 5/5, 7/7, 10/10 |
| viz modulation tests (4 binaries) | all ok |
| `rs_cam_cli` (every test) | all ok |
| `rs_cam_core --lib` | 2720 ok |
| clippy `--lib --test no_modulated…`, full features | clean |
| rustfmt on my files | clean |

Full `--all-targets` clippy stops on I5's new
`a_helical_waterline_cuts_a_stack_with_one_entry_d69.rs:113`
(`redundant_clone`). Not mine.

## 5. Base reds (none caused by C1 or C2)

- S11 `NotSet` fixtures: `adaptive3d_lift_bridge_b1`,
  `axial_engagement_vs_dpp_detector_f2`, `drill_runtime_survives_retime_n2`,
  `retime_respects_no_kinematics_n7`, `simulation_issue_channel_m1`, one
  test of `modulated_feeds_read_their_own_prefix_b5`.
- `perf_golden_sim_metrics` (Profile air-cut 1.9083 against 1.9295;
  modulation off). Red with the same numbers on a clean `f098d1623` (before
  C1) and a clean `e5614d2b8`, each in a detached worktree run by ITS OWN
  `scripts/cargo_lane.sh` (correction, see below).
- `a_rest_pocket_cuts_only_below_the_prior_floor_g_restlevels::a_rest_waterline_cuts_only_the_runs_that_pass_material`:
  **green (21/21) on a clean `e5614d2b8`** (C1 and C2 included). Its red
  in the shared tree comes from I5's uncommitted waterline edits, not from
  C2. (The first version of this report said "red at clean commits"; that
  was wrong, see below.)

**Correction (after the first freeze).** `scripts/cargo_lane.sh` does `cd`
to the git top level of the directory that holds the SCRIPT. I called the
shared tree's copy from the temporary worktrees, so those "clean" runs ran
in the shared tree (with I5's edits), only with another target directory.
The two checks above were run again with each worktree's own lane script.
The C2 patch and its gates are not affected.

Hazards found, for any "clean base" check:
- A temporary worktree that shares this tree's `target/` writes the SAME
  test binary names (one hash per test). Use a separate `CARGO_TARGET_DIR`.
- Call the temporary worktree's OWN `scripts/cargo_lane.sh`. The shared
  tree's copy runs cargo in the shared tree.
