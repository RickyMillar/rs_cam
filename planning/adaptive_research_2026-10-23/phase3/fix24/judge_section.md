## 4. The C12 heavy test judge (applied; not committed)

File: `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs` in the
`adaptive-head` worktree (branch `adaptive-instruments`). C22 and C12
cleared the ranges. The change:

- `exact_width_at(tp, m, from, to, cutter, depth_per_pass) -> (w, l)`: a
  height lattice in place of the boolean "cut" lattice. Each point starts
  at `TOP_Z`. Every earlier move, then move `m` up to `from`, lowers it to
  the tip Z where the cutter passes (Z linear along the move;
  `move_points_z`). A move that cannot reach the lattice is skipped
  (`move_is_far`). In the disc at the sub-segment midpoint, a point has
  stock above the cutter when its top is more than 0.01 mm over the tip Z
  there plus `cutter.height_at_radius(r)`. `w` is the sideways extent of
  those points over D, as before. `l` is
  `Σ cutter.mrr_cross_section_mm2(h, pitch) / cutter.mrr_cross_section_mm2(depth_per_pass, D)`
  over the sideways strips, `h` the deepest stock over the cutter in the
  strip.
- `exact_peak(..., cutter, depth_per_pass) -> (width peak, load peak)`.
- `read(..., depth_per_pass)`: a judged sample is screened when the sim
  radial or the sim box `radial · axial_engagement_mm / depth_per_pass` is
  over `sim_bound()`. It fails when its move's exact load peak is over
  `exact_bound()` (0.471). `Readout::peak_exact` is now the load peak, so
  the tests that compare it with `exact_bound()` judge the load. The
  `screened:` lines print both peaks.
- The seven `read(` calls pass the op's Depth/Pass: `DEPTH_PER_PASS`, or
  `p.depth.depth_per_pass` / `base.depth.depth_per_pass` where the test sets
  it to `TOP_Z − FLOOR_Z`.
- Not changed: the `link_over` bar (it still reads `DEPTH_PER_PASS`).

**Run** (`c12_test_run.txt`; release, `--include-ignored`, on the shared
tree at 3181ae54f with the other agents' uncommitted edits): 7 passed,
2 failed. The two failures do not call the judge (no `read`,
`exact_peak` or `exact_width_at` in them):
`the_geodesic_arm_feeds_its_turns_down_at_the_level` (two departures
without a capped move) and `the_research_arm_leaves_the_channel_to_the_edge_pass`
(the research arm cuts inside the channel).

The judge lines of the run, exact load peak against 0.471:

| Run | screened | over 0.471 | exact load peak |
|---|---|---|---|
| control AgentSearch | 226 | 225 | 1.207 |
| geodesic_curv | 6 | 6 | 0.967 |
| geodesic_curv_split | 2 | 2 | 0.725 |
| geodesic_local_spiral_pick, slope base | 172 | 0 | 0.353 |
| geodesic_local_spiral_pick, slope field | 13 | 0 | 0.423 |
| geodesic_local_spiral_pick, shelf base | 9 | 3 | 0.494 |
| geodesic_local_spiral_pick, shelf field | 36 | 0 | 0.413 |

**Clippy** (`-p rs_cam_core --all-targets`): clean. With
`--features research,test-support`: three errors, all outside the judge
ranges: an unused import `TurnRecord` (line 141) and two
`type_complexity` errors (lines 1954 and 1980, the corridor-slot test).
