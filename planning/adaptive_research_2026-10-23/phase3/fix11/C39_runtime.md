# C39: which runtime is right on wanaka200 (2026-10-26)

Read-only. No cargo, no code edit, no commit. The code cites are at
725be6a00 in the main checkout. `c39_runtime.py` computes the numbers
(`c39_runtime.json`); `assert_c39.py` re-derives each number in this file
with its own reader and checks the code cites (`python3 -I assert_c39.py`).

## Answer

**The toolpath summary runtime (the G2 page's cycle time) is the right
model for the emitted program on wanaka200.** The sample clock, which is
also the per-kinematics breakdown and fix11's C36 time split and MRR, is
timed at the feed **before** the feed modulation. The program runs at the
modulated feeds, which are higher.

The code says so itself:
- `stock/simulation_cut/reporting.rs:20-22`: `segment_time_s` "is naive:
  `segment_len / commanded_feed × 60`" at "the PRE-modulation commanded
  feed".
- `reporting.rs:59`: the per-class times "keep the naive clock too, so the
  per-class times no longer sum to the toolpath's".

So nothing in the code is wrong here. The fault was mine: I read the sample
clock as a second runtime model in C36. Two consequences follow.

1. **Corne and wanaka200 are on different clocks on the G2 page.**
   - wanaka200's machine profile has a `[job.machine.kinematics]` block.
     So `session/compute/simulation.rs:897`
     (`if self.machine.kinematics.is_some()`) re-integrates
     `total_runtime_s` at the modulated feeds, with acceleration and
     junction deviation.
   - Corne's profile has no such block. There the guard skips the
     re-time, and `total_runtime_s` stays the naive sample sum: no
     acceleration.

   A cycle time compares within one part, not across the two parts.
2. **C36's wanaka200 time split and MRR are at the pre-modulation feed.**
   Timed at the program's feeds, plunge feed is a larger share. Plunge
   feed moves keep their feed, and the cuts got faster. See the table
   below.

## The three values ('2 Back Rough' on wanaka200; '3D Rough 1' on Corne D20)

- **summary:** `toolpath_summaries[tp].total_runtime_s`.
- **samples:** the sum of the samples' `segment_time_s`.
- **program:** `program.nc` read alone: each fed move at its own F and its
  length (an arc at its arc length), each G0 at the post's rapid rate. The
  rapid rate is the machine's `max_feed_mm_min` (`gcode/mod.rs:1430-1436`
  with `high_feedrate_mode` off): Corne 4000 mm/min, wanaka200
  10000 mm/min. There is no acceleration, so this value is a lower bound
  at the emitted feeds.
- **samples re-timed:** each fed sample scaled by its sample feed / the F
  of its move in `program.nc`.

| Column | Summary (s) | Samples (s) | Per-kinematics sum (s) | Program (s): fed + rapid | Samples re-timed at the program feed (s) | Program F / sample feed, median over fed moves | Moves with sample feed below / above the program F |
|---|---|---|---|---|---|---|---|
| Corne zigzag_pick D20 | 1454.0 | 1454.0 | 1370.8 | 1455.3: 1370.8 + 84.6 | 1454.0 | 1.0 | 1446 / 0 of 11017 |
| wanaka200 contour_parallel | 5750.3 | 6645.8 | 5694.6 | 4280.4: 3329.0 + 951.5 | 4280.2 | 1.349 | 5936 / 991 of 10527 |
| wanaka200 zigzag_pick D18 | 3950.5 | 6229.7 | 6209.9 | 3153.1: 3133.0 + 20.1 | 3152.8 | 1.701 | 11225 / 768 of 20165 |
| wanaka200 zigzag_pick D20 | 4832.2 | 7095.1 | 7076.2 | 3866.0: 3846.0 + 20.0 | 3864.9 | 1.878 | 14272 / 507 of 24327 |
| wanaka200 AgentSearch | 1339.2 | 2449.0 | 2427.4 | 1052.4: 1030.5 + 21.9 | 1052.1 | 2.36 | 6139 / 23 of 7399 |

How to read it:
- **The feed explains the whole gap between the samples and the
  program.** Re-timed at the program's F, the samples give the program
  value to within 1.1 s on every wanaka200 column.
- **On wanaka200, summary ≥ program in every column.** The integrator
  adds acceleration to the program's feeds: contour_parallel +34.3 %,
  D18 +25.3 %, D20 +25.0 %, AgentSearch +27.3 %.
  - Its rapid share is also larger. contour_parallel's rapids are
    2093.1 s against 951.5 s at the flat rapid rate: short G0 hops cannot
    reach 10000 mm/min.
- **On Corne the three values agree.** The program's fed time equals the
  sample cutting time (1370.8 s). Two causes:
  - the Corne machine has no kinematics block, so there is no
    acceleration term;
  - modulation did not change the emitted fed time of '3D Rough 1'.

### C36's plunge share, re-timed at the program's feeds (no acceleration)

| Column | Plunge feed at the program F (s) | Share of the program time | C36 share (sample clock) |
|---|---|---|---|
| Corne zigzag_pick D20 | 545.3 | 37.5 % | 37.5 % |
| wanaka200 contour_parallel | 1070.8 | 25.0 % | 14.3 % |
| wanaka200 zigzag_pick D18 | 1627.3 | 51.6 % | 26.1 % |
| wanaka200 zigzag_pick D20 | 2350.0 | 60.8 % | 32.8 % |
| wanaka200 AgentSearch | 329.9 | 31.4 % | 13.0 % |

On wanaka200, D20 spends 60.8 % of its time at the emitted feeds in
vertical plunges (before acceleration). The C36 README's wanaka200 time
split and MRR need this correction. The corner, label and descent counts of
C36 do not depend on time and stand.

## Proposals (for the lead)

1. **Page:** state the clock under each cycle time.
   - wanaka200: machine model (acceleration, emitted feeds).
   - Corne: naive at the emitted feeds; the Corne project has no
     kinematics block.

   Or add a kinematics block to the Corne machine profile, so both parts
   use one clock. That is a project-file edit and needs a ruling.
2. **fix11 / C36:** re-time the sample split and the MRR at the program
   feed (per sample: `segment_time_s × feed_rate_mm_min / F`) when the
   run's machine has a kinematics block. This is the same per-sample
   scaling as `rebase_cutting_times`.
