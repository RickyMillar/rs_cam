# fix24 (C43): the depth-weighted load oracle

Operator ruling, 2026-10-27: the load bound weights the engagement by the
axial depth. Operator instruction, the same day: use the load calculations
that the codebase already has, with one definition across the codebase.

C18 owns the measurement side: the exact oracle, the C12 heavy test judge
and the fixture counts. C22 owns the planner side (`P3_C22_3d_turns.md`
§18.6). Python only on the fixtures; the C12 test edit is in §4.

## 1. The definition (lead's ruling 2026-10-27; C22 §18.6)

The load of a step is the codebase's own chip cross-section, applied per
lateral strip of the cutter disc, over that of a full pass:

`A = Σ over the strips of cutter.mrr_cross_section_mm2(h(u), strip width)`

`L = A / cutter.mrr_cross_section_mm2(d, D)`

- `MillingCutter::mrr_cross_section_mm2` (`crates/rs_cam_core/src/tool/mod.rs:508`)
  is ap · ae. The V-bit halves it (`tool/vbit.rs:149`); the factor cancels
  in the ratio. The power gate multiplies it by the feed for the MRR
  (`tool_load/power.rs:287`).
- u: the lateral coordinate, perpendicular to the XY travel of the step.
  The strips are one lattice pitch wide on the exact oracle (0.05 mm) and
  one planner cell wide on the planner.
- h(u): the deepest H − (z + p(r)) of the disc points in the strip with
  stock above the cutter. H is the stock top, z the tip Z, p(r) the
  cutter's height above its tip at radius r (`height_at_radius`; 0 for the
  flat end mill of every fixture and test). A point has stock above the
  cutter when H − (z + p(r)) > 0.01 mm (the C12 oracle's 0.01).
- d: the Depth/Pass of the op that is read. D: the tool diameter.
- Bound: 0.471, the radial bound of today.
- **Screen only:** the simulation's box `radial_woc_fraction ·
  axial_engagement_mm / d` (the deepest point times the full width). It
  is an upper bound of L: on the exact lattice the box is under L by at
  most two strips, from the rounding of the end strips. A sample is read
  on the exact lattice when the sim radial or the sim box is over the
  screen bound.

**Info column:** `L_box`, the same box on the exact lattice:
mrr_cross_section_mm2(ap, ae) / mrr_cross_section_mm2(d, D), with ap the
deepest uncut height in the disc and ae its lateral extent.

## 2. The derivation (no new number)

- At a full pass h(u) = d across the engaged width, so A = d · ae and
  L = ae / D, the radial fraction of today. The ceiling
  (1 − cos(1.05 · acos(1 − s/R))) / 2 and its tolerance terms carry over
  with no change. That is 0.471 at s 2, R 3.
- A thin layer of height t < d across the band reads L = (ae / D) · t / d:
  light. A narrow deep strip counts only its own width.
- A cut deeper than d reads L > ae / D: held, and held tighter than now.
- The lattice reads A to one strip: |ΔL| ≤ pitch · h / (D · d).
- d is read from the op's project file: Corne op 1 "3D Rough 1" 6.0 mm
  (`corne_job_corrected_c12_agent_search.toml` line 127), wanaka200 op 1
  "2 Back Rough" 5.46 mm (`wanaka200_mt1_c12_rough_only.toml` line 256).

## 3. The oracle: `area_oracle.py`

- The height field H is a lattice of pitch 0.05 mm (the C12
  `EXACT_PITCH_MM`) over the stock box, from the stock top. A point outside
  the box holds no stock. wanaka200: the samples are in the setup frame of
  the flipped stock, so the box is x 0–240, y 0–250, z 0–25
  (`fix9/stockmap.py`).
- The samples are walked in trace order. Each sample is read first, then
  its step (from the previous sample of the same toolpath) is stamped: a
  flat end mill of R 3 mm sets H = min(H, z) at the lattice points within R
  of points 0.3 mm apart or less along the step, Z linear along the step.
  So a read sees every earlier step and the current move up to `from`, as
  `exact_width_at` does. The longest sample step is 0.54 mm on Corne
  and 1.83 mm on wanaka200.
- The read: the disc of R at the step midpoint, the tip Z at the midpoint.
- Usage: `area_oracle.py <run> <corne|wanaka200> [toolpath id] [Depth/Pass]`.
  A later toolpath (a CP rest op) is read on the stock that the earlier
  toolpaths of the trace leave. Every toolpath stamps with R 3 mm, so an
  earlier toolpath with another tool gives a wrong stock.

The synthetic checks (`synthetic.py`): a straight pass, then a parallel
pass at stepover e on stock of height t over the tip (d = 6.0 mm, R 3 mm).

| stepover e (mm) | stock height t (mm) | heading (rad) | L read | L = e·t/(D·d) | w read | w = e/D |
|---|---|---|---|---|---|---|
| 2.0 | 6.0 | 0.0 | 0.3417 | 0.3333 | 0.3333 | 0.3333 |
| 1.2 | 6.0 | 0.0 | 0.2000 | 0.2000 | 0.1917 | 0.2000 |
| 2.0 | 0.5 | 0.0 | 0.0285 | 0.0278 | 0.3333 | 0.3333 |
| 1.2 | 3.0 | 0.0 | 0.1000 | 0.1000 | 0.1917 | 0.2000 |
| 2.0 | 6.0 | 0.5 | 0.3417 | 0.3333 | 0.3288 | 0.3333 |
| 1.2 | 1.0 | 0.5 | 0.0347 | 0.0333 | 0.1958 | 0.2000 |
| 2.0 | 9.0 | 0.5 | 0.5125 | 0.5000 | 0.3288 | 0.3333 |

Each L is within one lattice bin, pitch · t / (D · d), of e·t/(D·d), and
each w within pitch / D = 0.0083 of e/D. At t = 9.0 mm > d the load is
0.5125 against w 0.3288: a deep side cut is held.

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

## 5. The fixture counts

The old count (C22 §16.9; `fix21`–`fix23` `load_split.py`) is the XY length
of op 1's clearing_cut samples with sim radial > 0.471. The new count is
the XY length of the read samples with L > 0.471 (`rescore.py`,
`area_over_mm`). The old counts here are equal to `P15_D_verify.md` (D26,
D33 default, D33 stock_depth), `P3_C38_outer_band.md` §16.4 (C35) and
`P3_C18_3d_oracle.md` §C32.2 (wanaka200 contour_parallel, "body over 0.471").

## 6. The re-score (read only)

The classes of the old count: sim-only (L ≤ 0.471 and the exact width
w ≤ 0.471), thin (w > 0.471, L ≤ 0.471), still over (L > 0.471).
"Lowered feed": a sample feed below the op's base clearing feed, the most
common one (the `load_split.py` rule): the C38 fed slots and the capped
turns.

### Corne, op 1 "3D Rough 1"

| | contour_parallel (`fix6/corne/contour_parallel`) | D26 (`fix16/corne_pick`) | C35 (`fix12/corne`) | D33 (`fix23/default/corne`) | D33 stock_depth (`fix23/stock_depth/corne`) |
|---|---|---|---|---|---|
| op 1 Depth/Pass d (mm) | 6.00 | 6.00 | 6.00 | 6.00 | 6.00 |
| old count: sim radial > 0.471 (mm) | 3158.9 | 780.8 | 486.3 | 1044.1 | 1058.8 |
| **load count, per strip: L > 0.471 (mm)** | **2226.0** | **142.7** | **86.9** | **424.9** | **424.9** |
| box count, info: L_box > 0.471 (mm) | 2604.1 | 598.4 | 410.8 | 866.3 | 879.8 |
| box-only: L_box > 0.471, L ≤ 0.471 (mm) | 378.0 | 456.0 | 323.9 | 441.5 | 454.9 |
| old count, of it sim-only: L ≤ 0.471, w ≤ 0.471 (mm) | 20.4 | 6.4 | 7.1 | 3.6 | 5.0 |
| old count, of it thin: w > 0.471, L ≤ 0.471 (mm) | 915.9 | 634.2 | 392.3 | 615.6 | 628.9 |
| old count, of it still over: L > 0.471 (mm) | 2222.5 | 140.2 | 86.9 | 424.9 | 424.9 |
| new over: sim radial ≤ 0.471, L > 0.471 (mm) | 3.5 | 2.4 | 0.0 | 0.0 | 0.0 |
| load count at a lowered feed / at the base feed (mm) | 0.0 / 2222.5 | 0.0 / 140.2 | 0.0 / 86.9 | 387.7 / 37.2 | 387.7 / 37.2 |
| base clearing feed (mm/min) | 4000 | 3556 | 4000 | 4000 | 4000 |
| peak L | 1.031 | 1.933 | 2.010 | 1.792 | 1.792 |

### wanaka200, op 1 "2 Back Rough"

| | contour_parallel (`fix6/wanaka200/contour_parallel`) | D26 (`fix16/wanaka200_pick`) | C35 (`fix12/wanaka200`) | D33 (`fix23/default/wanaka200`) | D33 stock_depth (`fix23/stock_depth/wanaka200`) |
|---|---|---|---|---|---|
| op 1 Depth/Pass d (mm) | 5.46 | 5.46 | 5.46 | 5.46 | 5.46 |
| old count: sim radial > 0.471 (mm) | 6201.7 | 2225.3 | 2234.8 | 2558.4 | 2864.0 |
| **load count, per strip: L > 0.471 (mm)** | **2133.1** | **26.6** | **26.0** | **52.2** | **48.4** |
| box count, info: L_box > 0.471 (mm) | 4953.1 | 1531.1 | 1531.0 | 1816.3 | 2052.1 |
| box-only: L_box > 0.471, L ≤ 0.471 (mm) | 2820.0 | 1504.5 | 1505.0 | 1764.0 | 2003.7 |
| old count, of it sim-only: L ≤ 0.471, w ≤ 0.471 (mm) | 272.0 | 90.5 | 90.4 | 92.6 | 127.5 |
| old count, of it thin: w > 0.471, L ≤ 0.471 (mm) | 3802.3 | 2108.2 | 2118.4 | 2413.6 | 2688.1 |
| old count, of it still over: L > 0.471 (mm) | 2127.4 | 26.6 | 26.0 | 52.2 | 48.4 |
| new over: sim radial ≤ 0.471, L > 0.471 (mm) | 5.7 | 0.0 | 0.0 | 0.0 | 0.0 |
| load count at a lowered feed / at the base feed (mm) | 0.0 / 2127.4 | 0.0 / 26.6 | 0.0 / 26.0 | 17.2 / 35.1 | 17.8 / 30.7 |
| base clearing feed (mm/min) | 1000 | 1000 | 1000 | 1000 | 1000 |
| peak L | 2.493 | 0.862 | 0.599 | 0.617 | 0.617 |

## 7. Reading

1. **Most of the old over-load was thin layers.** The per-strip load count
   against the old count: Corne D26 142.7 of 780.8 mm,
   C35 86.9 of 486.3 mm, D33 424.9 of
   1044.1 mm; wanaka200 D26 26.6 of
   2225.3 mm, C35 26.0 of 2234.8 mm,
   D33 52.2 of 2558.4 mm.
2. **The control.** contour_parallel reads 2226.0 mm on Corne
   and 2133.1 mm on wanaka200. Every research column is
   below it on both parts.
3. **The box would keep most of it.** The box (info) reads
   866.3 mm on Corne D33 and 1816.3 mm on wanaka200 D33.
   The difference is disc reads with a deep part on a narrow strip and a
   thin layer on the rest: the box multiplies the full width by the deepest
   point. That is why the box is only the screen.
4. **The D33 slots, as context.** 387.7 mm of the Corne D33
   load count runs at a lowered feed (the C38 fed slots and the capped
   turns). The edges move to a separate contour pass (C12 §16), so these
   slots leave the geodesic op.
5. **Deep cuts.** The peak L is 2.010 in the research columns
   and 2.493 in contour_parallel. The new-over class (over under the
   load, not under the sim radial) is 5.7 mm or less in every run.

## 8. Limits

- The base feed of D26 Corne is 3556 mm/min, not 4000, so its feed split is
  not comparable with the other columns.
- The oracle stamps each sample step, an arc or helix chord included, as
  a straight step (wanaka200: up to 1.83 mm).
- The stock outside the stock box is air. A cut never goes below the
  target, so the stock above the tip is the only quantity.

## 9. Files

- `area_oracle.py`: the oracle. `rescore.py`: the classes (`rescore.json`).
- `synthetic.py`: the synthetic checks (`synthetic.json`).
- `judge_section.md`: §4. `make_readme.py`: this file. `assert_fix24.py`: the checks.
- `run_fix24.sh`: the oracle on the ten runs, then the rest, then the checks.
- `area_<run>.npz`: one row per read sample.
