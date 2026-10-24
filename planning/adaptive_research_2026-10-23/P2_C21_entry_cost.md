# P2 C21 — the cost of one entry, from rs_cam's own cycle-time model (2026-10-24)

Status: COMPLETE (Python only; no Rust edited, no cargo run, no commit).
Scripts: `phase2/c21/` (`model.py` the port, `gcode.py`, `run_c21.py`,
`note_c21.py`, `assert_c21.py`). Run `run_c21.py`, then `note_c21.py`, then
`assert_c21.py`. `note_c21.py` writes this note; `assert_c21.py` runs the model
again and checks every quoted number, and checks the port against closed forms.

## 1. Answer

An entry costs what its fed descent costs. At a 5 mm hop the lift, the rapid across and
the rapid down take less than one second on each machine. The entry style then feeds down at the
plunge feed, and on rivmap100 the helix feeds 2π·1.8 mm of path for each 1 mm of
descent. At a 5 mm hop, first level, into the level's own stock, the penalty
against a keep-down feed at the cut feed is:

- rivmap100 (helix): **4.41 s**.
- Corne (plunge, profile with no kinematics, the simulator's no-acceleration basis): **0.68 s**; with the 200 mm/s² default that the linker reads: 1.57 s.
- wanaka200 (plunge): **1.40 s**.

Into a column that is already cut (0.5 mm fed descent), the penalty falls to 0.74 s, 0.24 s and 0.75 s. Into a fresh column at the deepest level, it rises to 17.53 s, 2.34 s and 3.70 s.

Against C19's break-even (rescaled to each fixture's cut feed): on rivmap100 the
clipped (e) and the clipped march are faster than (a) on both shapes; on wanaka200 and on
Corne (its simulator's basis), (a) is faster on both shapes. With the 200 mm/s² envelope, Corne
tips to the clipped (e) on the star only. Section 5 gives the order of magnitude, not a
fixture verdict: the 2D shapes are not the fixtures.

## 2. The model, with citations (HEAD 6b194b59)

The port is `phase2/c21/model.py`. Each function cites the Rust lines it ports.

- The integrator: `compute_cycle_time_breakdown` (`machine/kinematics.rs:1177-1247`) over
  `digest_moves` (`:1071-1118`) and `walk_junctions` (`:1131-1171`). Each move is a trapezoid,
  `trapezoidal_time` (`:1582-1610`): accelerate from v_in to the cruise ceiling, cruise, decelerate
  to v_out; a short move is a triangle with v_peak² = a·L + (v_in² + v_out²)/2.
- The acceleration is direction-aware, `effective_accel` (`:396-415`): min_i(a_i / |d_i|). A Z move
  runs at the Z axis value (270 mm/s² on the Shapeoko profile), an XY move at 500 mm/s².
- The cruise ceiling is the commanded feed, capped by `max_feed` (`:1098`) and by the per-axis
  rate when the profile has one (`cruise_ceiling`, `:724-740`). A rapid runs at
  max(rapid, max_feed) (`:1189`).
- A junction runs at GRBL's junction-deviation speed √(a·R), R = δ·sin(θ/2)/(1 − sin(θ/2))
  (`junction_velocity`, `:1538-1567`). **A junction next to a rapid is a full stop** (`:1548-1550`).
  A 90° corner at δ = 0.02 mm and 500 mm/s² runs at 4.9 mm/s, nearly a stop too.
- The simulator calls the integrator with the profile's kinematics, `max_feed_mm_min`
  (`session/compute.rs:2305-2311`) and the rapid rate `PostConfig::rapid_feed_mm_min`
  (`gcode/mod.rs:1430-1436`: `max_feed_mm_min` unless high-feedrate mode is on)
  (`compute/simulate.rs:2474-2479`). A profile with no kinematics gets no integrator: the
  time is each move's length over its feed, rapids at the rapid rate
  (`dexel_stock/simulation.rs:579`, `:1032`; basis `SimulatedNoAccel`, `session/cycle_time.rs:62`).
- The linker's envelope reads `effective_kinematics`, which gives a profile with no
  kinematics the 200 mm/s² `generic_wood_router` (`machine/mod.rs:679-697`, `kinematics.rs:325-330`).

The entry, as the adaptive3d emitter writes it (`adaptive3d/path.rs:1514-1655`):

1. Lift: a rapid straight up to `safe_z` (`:1460-1469`). `safe_z` = max(post safe Z, stock top + 5)
   (`compute/config.rs:162-171`); on all three fixtures it is the stock top + 5.
2. Travel: a rapid across at `safe_z` (`:1583-1586`).
3. Descent: a rapid down to the rapid floor + 0.5 mm (`descent_start_z`, `:1323-1337`). The rapid
   floor is the planner's conservative stock top over the entry footprint (tool radius, plus the
   helix radius for a helix, `:1201-1205`).
4. The entry style from there to the cut Z, at the plunge feed:
   - plunge: the peck ladder (`emit_peck_plunge`, `:164-195`): pecks of one depth per pass, each
     with a 0.5 mm rapid back up, then the rest;
   - helix (`dressup/entry_descent.rs:779-918`): 36 steps per turn, pitch/36 down per step, at
     the ramp feed (unset on the fixtures, so the plunge feed, `compute/catalog.rs:1636-1640`),
     then a feed back to the centre. Radius = D × `helix_radius_factor`, capped at the flat bottom
     (`compute/operation_configs.rs:1820-1830`, `compute/config.rs:986-1000`): 0.3 × 6 = 1.8 mm.
     A column whose floor is at the cut Z is air: one straight 0.5 mm feed (`entry_descent.rs:444-470`).

A rs_cam keep-down link (`try_emit_stay_down_link`, `path.rs:1240-1317`) ends at the same rapid
floor, and the entry style still takes the material below it (`:1219-1221`). So in the 3D engine a
keep-down link saves the lift, the travel and the descent, not the helix or the pecks. The keep-down
of this note is the 2D research's: one feed at the cut feed and the cut Z over the hop L, with no
re-entry, as the brief defines it.

**The harness.** Cut A: 20 mm along +X at the cut feed, from rest. The link: the entry above, or the
keep-down feed, over L along +Y. Cut B: 20 mm along +X. The integrator times the whole path. Entry time
and keep-down time are the totals less cuts A and B timed alone from rest to rest; the penalty is the
difference of the two totals. Both corners are 90°. The floor is flat (no surface lift of the helix).

**The column**, which sets the fed descent d_f:

- open: the column is cut down to the cut Z; d_f = 0.5 mm;
- level: the floor is one depth per pass up (the level's own stock); d_f = depth per pass + 0.5 mm;
- fresh: the floor is the stock top; d_f = depth + 0.5 mm. At the first level, level = fresh.

**The port is checked against the Phase 0 runs.** `model.py` over each Phase 0 G-code gives:

| Op (ContourParallel) | Phase 0 runtime s | Port over the .nc, s | Difference |
|---|---|---|---|
| rivmap100 3D Rough | 3262.8 | 3264.3 | +0.05 % |
| Corne 3D Rough 1 (no-acceleration basis) | 973.2 | 973.5 | +0.03 % |
| wanaka200 2 Back Rough | 5776.8 | 5778.7 | +0.03 % |

The .nc rounds coordinates to 1 µm and carries the post-processed arcs, so a small difference is
expected. The port is the model.

## 3. Machine and op settings (read only)

| | rivmap100 | Corne | wanaka200 |
|---|---|---|---|
| File | `planning/fixtures/rivmap100/rivmap100.toml` | `planning/corne_case_analysis_2026-09-18/corne_job_corrected.toml` | `planning/multitool_2026-08-23/wanaka200_mt1.toml` |
| Machine | Shapeoko Pro XXL (:29) | Generic Wood Router (:29) | Shapeoko Pro XXL (:38) |
| `max_feed_mm_min` = rapid rate | 10000 (:30); high-feedrate mode off (:24) | 4000 (:30); off (:24) | 10000 (:39); off (:33) |
| Kinematics | accel X/Y/Z 500/500/270 mm/s², scalar 423.3, δ 0.02 mm, no per-axis rates, no jerk (:56-63) | none (:28-53) | as rivmap100 (:64-71) |
| Stock top / safe_z | top 12 after the Face (setup frame); safe = top + 5 | top 23 (:9, :12); safe 28 | top 25 (setup frame); safe 30 |
| Rough op | 3D Rough, adaptive3d (:241-276) | 3D Rough 1, adaptive3d (:112-147) | 2 Back Rough, adaptive3d (:318-354) |
| Cut feed / plunge feed mm/min | 2400 / 500 (:258-259) | 1518.75 / 750 (:129-130) | 1000 / 541 (:336-337) |
| Entry style | helix, factor 0.3, pitch 1.0 (:262-265); tool D 6 (:69) | plunge (:133) | plunge (:340) |
| Depth per pass | 2.0 (:256) | 6.0 (:127) | 5.46 (:333) |
| Levels (Phase 0, setup Z) | 10, 8, 6, 4, 2, 0.5 | 17, 11, 5, 0.5 | 19.54, 14.08, 8.62, 4.0 |

The fixtures' kinematics carry no per-axis rates, so the model runs a Z rapid at 10000 mm/min. The
preset `shapeoko_xxl_ricky_tuned` (`kinematics.rs:376-384`) holds the operator's `$112` = 1000 mm/min.
With it, the rivmap100 penalty at 5 mm becomes 4.64 s (first level) and 5.78 s (deepest level, level column), against 4.41 and 4.78 s.

## 4. One entry against one keep-down, by hop length

First level, level column (the 2D research's case: a new arc enters the level's own stock).
Entry / keep-down / penalty, seconds. The arc-to-arc hops of C19 and D12 are 2–8 mm; lobe
changes are 6–69 mm.

| Fixture | L = 0 mm | L = 2 mm | L = 5 mm | L = 6 mm | L = 8 mm | L = 20 mm | L = 50 mm | L = 69 mm |
|---|---|---|---|---|---|---|---|---|
| rivmap100 | 4.38 / -0.08 / **4.46** | 4.50 / 0.09 / **4.41** | 4.58 / 0.17 / **4.41** | 4.60 / 0.19 / **4.40** | 4.63 / 0.24 / **4.39** | 4.78 / 0.54 / **4.23** | 5.01 / 1.29 / **3.72** | 5.12 / 1.77 / **3.36** |
| Corne | 0.80 / 0.00 / **0.80** | 0.83 / 0.08 / **0.75** | 0.88 / 0.20 / **0.68** | 0.89 / 0.24 / **0.65** | 0.92 / 0.32 / **0.60** | 1.10 / 0.79 / **0.31** | 1.55 / 1.98 / **-0.43** | 1.83 / 2.73 / **-0.89** |
| wanaka200 | 1.50 / -0.03 / **1.54** | 1.63 / 0.12 / **1.51** | 1.70 / 0.30 / **1.40** | 1.72 / 0.36 / **1.36** | 1.76 / 0.48 / **1.28** | 1.90 / 1.20 / **0.70** | 2.14 / 3.00 / **-0.86** | 2.25 / 4.14 / **-1.89** |

Keep-down at L = 0 is slightly negative: cut B then runs on from cut A with no stop. The keep-down
grows by L over the cut feed; the entry grows only by the rapid across. On a 1 mm grid, the first
level penalty reaches zero at L = 33 mm (Corne) and 34 mm (wanaka200). On rivmap100 it stays positive to 70 mm (code value -1).

The parts of the first level entry at L = 5 mm (s; the stop at the end of cut A sits in cut A):

| Fixture | Lift | Travel | Descent | Entry style |
|---|---|---|---|---|
| rivmap100 | 0.32 | 0.20 | 0.26 | 3.81 |
| Corne | 0.16 | 0.07 | 0.07 | 0.57 |
| wanaka200 | 0.39 | 0.20 | 0.26 | 0.86 |

The penalty at L = 5 mm by column (first level open; deepest level open, level, fresh):

| Fixture | First level, open | Deepest, open | Deepest, level | Deepest, fresh |
|---|---|---|---|---|
| rivmap100 | 0.74 | 1.09 | 4.78 | 17.53 |
| Corne | 0.24 | 0.73 | 1.17 | 2.34 |
| wanaka200 | 0.75 | 1.21 | 1.92 | 3.70 |
| Corne, 200 mm/s² linker envelope | 1.57 (first level, level) | — | 2.19 | — |

## 5. C19's break-even

C19 (`P2_C19_march_arms.md`, plan `oneway_head`, read from `phase2/c19/results_c19.json` when this note
was written) gives the time per entry above which a plan with fewer entries beats (a) with the same links:
(time of the plan − time of (a)) / (entries of (a) − entries of the plan). Its times are length over a
1500 mm/min cut feed with no acceleration, so the break-even scales by 1500 / F for a fixture's cut feed F.

| Plan | Entries (plan / (a)) | Break-even at 1500 | at rivmap100's 2400 | at Corne's 1518.75 | at wanaka200's 1000 |
|---|---|---|---|---|---|
| star, clipped (e) | 35 / 85 | 1.3 | 0.8 | 1.3 | 1.9 |
| star, clipped march | 74 / 85 | 2.2 | 1.4 | 2.2 | 3.3 |
| pocket, clipped (e) | 23 / 75 | 2.9 | 1.8 | 2.8 | 4.3 |
| pocket, clipped march | 65 / 75 | 3.4 | 2.1 | 3.4 | 5.1 |

The penalty to compare is the first level, level column, at a 2–8 mm hop: C19's plunges and
refusals re-enter the level's stock a few millimetres away. Which plan is faster (feed-rescaled
break-even):

- rivmap100, penalty 4.41 s: clipped (e) star: plan; clipped march star: plan; clipped (e) pocket: plan; clipped march pocket: plan.
- Corne, penalty 0.68 s (no-acceleration basis): clipped (e) star: (a); clipped march star: (a); clipped (e) pocket: (a); clipped march pocket: (a). With the 200 mm/s² envelope, 1.57 s: clipped (e) star: plan; clipped march star: (a); clipped (e) pocket: (a); clipped march pocket: (a).
- wanaka200, penalty 1.40 s: clipped (e) star: (a); clipped march star: (a); clipped (e) pocket: (a); clipped march pocket: (a).
- Into an open column no plan with fewer entries wins anywhere: the penalty is 0.74, 0.24 and 0.75 s.

What decides it is the entry style and its fed descent, not the machine's rapids. The rivmap100 helix
feeds 2π·1.8 = 11.3 mm of path per mm of descent at 500 mm/min; a plunge feeds 1 mm per mm. On a
helix machine the plans with fewer entries pay; on a plunge machine (a) stays faster.

## 6. Cross-check with Phase 0

The 36–40 % of PLAN.md comes from `planning/adaptive3d_step_ladder_roughing_2026-09-24/RESULTS.md:288-304`
(rivmap100, By Area, `rough-score`): entry 538 s of 1379 s (39 %) for the base arm. That run carries no entry count. Phase 0 (2026-10-23,
ContourParallel) has both: entry 2364.3 s of 3262.8 s (72 %), 237 top-level entries, so 10.0 s per entry.

With the flat-floor level column (helix 3.81 s, plus a lift and a descent for each entry, an upper bound), 237 entries give 32 % of the Phase 0 runtime: the order of the 36–40 %, not the Phase 0 72 %.
The G-code says why: its moves at the plunge feed go down 6.0 mm per entry on average (the sum of the downward Z, an upper bound,
because the shipped helix lifts on the terrain and goes up and down), not 2.5 mm. The model with
the Phase 0 counts and that mean descent:

| Fixture | Lifts | Keep-down entries | Mean hop mm | Mean fed descent mm | Model, one retract entry s | Model, one keep-down entry style s | Model total s | G-code rapid + plunge-feed s | Model / G-code | Model % of runtime | G-code % of runtime |
|---|---|---|---|---|---|---|---|---|---|---|---|
| rivmap100 | 103 | 136 | 52.5 | 6.04 | 10.19 | 8.65 | 2226 | 2303 | 0.97 | 68 | 71 |
| Corne | 74 | 22 | 59.3 | 20.11 | 3.16 | 1.75 | 273 | 266 | 1.02 | 28 | 27 |
| wanaka200 | 964 | 645 | 129.5 | 6.03 | 3.14 | 0.87 | 3581 | 3225 | 1.11 | 62 | 56 |

The G-code has 103 lifts to the retract plane on rivmap100; Phase 0 counts 104 retracts; this note does not resolve the difference of one.

The per-entry model reproduces the G-code's entry time to within about 10 % on all three, and the
rivmap100 share (about 70 %) matches Phase 0's entry_s share. The check is not independent: the mean
hop and the mean descent come from the same G-code. It shows that the per-entry cost is the right
size when the descent is right, and that the descent, not the count, sets the share. On wanaka200
the rapids take 2095 s: 964 lifts with a mean hop of 130 mm.
On rivmap100 the moves at the plunge feed run 12.5 mm of path per mm of downward Z, against 11.3 for the flat helix.

## 7. Limits

- C19's times have no acceleration. With the model's ramps, the longer engaged path of (e) and
  of the march would cost more, so the true break-even is higher than the table's. The rivmap100
  verdict holds only where the penalty is well above the break-even.
- A flat floor: no surface lift of the helix. The shipped rivmap100 helix lifts on the terrain
  and makes more turns (section 6), so the flat value is a lower bound for a terrain entry.
- Both corners of the harness are 90°. A keep-down that runs straight on is faster by up to the
  two stops; at δ = 0.02 mm a 90° corner is nearly a stop, so the difference is small.
- The feeds are the op's nominal feeds. The feed modulation pass (the .nc shows F4721 cuts on
  rivmap100) changes the cut feed, not the plunge feed.
- Corne has no kinematics: its no-acceleration numbers are what its simulation reports, and they
  read optimistic (`session/cycle_time.rs:57-61`).
- The per-axis rate (`$112`) is not in the fixture files; section 3 gives its effect on rivmap100.
