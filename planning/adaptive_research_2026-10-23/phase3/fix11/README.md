# fix11: a kinematic audit that trusts no label (C36, R4 §3.2 items 1-4)

Python only, on the existing artefacts. No cargo, no commit.
`audit.py` takes one run folder (`program.nc`, `tp_1_*.json`, and the
`simulation.json` of the same plan) and prints four readings. Every count
comes from a predicate on the motion (the G-code moves) and on the sim
samples (removed volume, time). The intent labels and span styles are read
only for a cross-table, so a mislabelled move still counts.

```
python3 -I audit.py <run_dir> [--sim <dir>] [--op "3D Rough 1"] [--tp "tp_1_*.json"] [--nominal-mrr <mm3/s>] [--json out.json] [--list]
python3 -I run_all.py        # the 8 G2 columns -> audit_results.json, audit_output.txt
python3 -I make_readme.py    # the tables below
python3 -I assert_audit.py   # re-derives every number of this file with its own code
```

## The thresholds and their derivations

The old screening values were: descent ≥ 1.0 mm, removed > 0.1 mm³,
plane within 0.35 mm (fix9 `entries.py`, `render.py`), and island height
0.8 mm (the CLI triage). The audit replaces the first three:

| Old value | Replaced by | Derivation |
|---|---|---|
| descent ≥ 1.0 mm | Z drops by more than one G-code output step (`GCODE_EPS` = 0.001 mm) | The post writes X, Y and Z with 3 decimals. Every fed move that goes down by more is a descent. Whether it matters is decided by the volume, not by its length. |
| removed > 0.1 mm³ | removed > cell² × sample step (Corne 0.3² × 0.3 = 0.027 mm³; wanaka200 1.0² × 1.0 = 1.0 mm³) | One dexel column of one cell, swept over one sample step. This is the smallest removal that the trace resolves (`simulation.json` `resolution_mm`, `sample_step_mm`). |
| end plane within 0.35 mm | not needed | The audit reads the exact G-code Z of each move, not the sample Z. |
| island height 0.8 mm | not used here | It is the CLI triage's shipped value for the coverage bar. It is a screening value with no derivation in this programme (R4 §3.2 item 6). |

Further rules:
- **Vertical move:** XY travel under `GCODE_EPS`.
- **Descent event:** a run of vertical moves at one XY (fed downs, plus
  the G0 pecks between them) in which at least one fed down removes more
  than the quantum. One lift-over plunge with its peck is one event.
- **Reversal:** a junction between two fed moves whose 3D turn is over
  90 deg. Past 90 deg, the velocity component along the incoming direction
  changes sign, so the machine must stop that motion at the junction,
  whatever its acceleration. A turn of exactly 90 deg (horizontal to
  vertical, the lift-overs) is counted apart.
  - The machine profiles of both projects give no acceleration and no
    junction deviation, so the audit predicts no junction speed.
  - The histogram bins are for reading only.
  - A **planar junction** has both moves travelling in XY: the cutting
    path without the vertical lifts and plunges.
- **MRR:** `removed_volume_est_mm3 / segment_time_s` per sample, weighted
  by the sample time. The nominal MRR (feed_rate × stepover ×
  depth_per_pass of the project) is a reference line only.
- **Time split:** the sample time of each move, by motion:
  - rapid (G0);
  - plunge feed (fed, vertical, down);
  - lift feed (fed, vertical, up);
  - cut (fed, not vertical, removes more than the quantum);
  - fed in air (fed, not vertical, removes the quantum or less: links at
    depth and air cuts).

## Findings

1. **The labels hide descents on every strategy.** Every column has
   vertical descents into stock that carry the label "keep-down link".
   - On D18 and D20 they are the lift-over plunges of fix9.
   - Some carry the intent `linking` and no `entry_plunge` sample at all:
     Corne contour_parallel 3, D18 44, D20 4, AgentSearch 13; wanaka200
     contour_parallel 150, D18 92, D20 28, AgentSearch 21.
   - A count that reads only `entry_plunge` misses those.
2. **The plunge share of the time.** Plunge feed is 37.5 % of the sample
   time of '3D Rough 1' on Corne D20 (545.3 s), against 2.8 % for
   contour_parallel (26.9 s). On wanaka200 D20 it is 32.8 % (2329.7 s),
   against 14.3 % for contour_parallel.
3. **Smoothness: the operator's report holds in the plane; the lift-overs
   break it.** Planar turns over 30 deg as a share of the planar
   junctions:
   - Corne: contour_parallel 71.6 %, D20 19.3 %;
   - wanaka200: contour_parallel 47.8 %, D20 3.1 %.

   Planar reversals: Corne 503 against 354; wanaka200 293 against 22.
   With the vertical moves included, D20 has 4166 turns of exactly 90 deg
   on Corne (contour_parallel 222). These are the lifts and plunges of the
   lift-over links. So the geodesic cut path is smoother, and its links
   are not.
4. **MRR.** Time over twice the nominal MRR on Corne: contour_parallel
   274.1 s, D20 285.2 s. On wanaka200: 188.1 s against 487.7 s. The
   highest single-sample MRR is on a fed XY move in every column, not on a
   plunge.
5. **Two runtime models disagree on wanaka200.** The sim samples' time,
   which is also the sum of the trace's own per-kinematics times, is
   larger than the toolpath summary runtime:
   - contour_parallel 6645.8 against 5750.3 s;
   - D18 6229.7 against 3950.5 s;
   - D20 7095.1 against 4832.2 s;
   - AgentSearch 2449.0 against 1339.2 s.

   On Corne the two are equal. The G2 page's wanaka200 cycle times are the
   summary runtimes. Which model is right is not known; it needs an owner
   in the simulation code.
6. **Against fix9.** The vertical descent events of Corne D20 are 1236.
   fix9 counted 1195 + 36 = 1231 spans (drop ≥ 1.0 mm, removed > 0.1 mm³).
   On wanaka200 D20 the audit counts 3412 against fix9's 3578 + 14. The
   two predicates differ: the audit counts vertical descents only, with
   the 1.0 mm³ quantum of the 1.0 mm cell, while fix9 counted every
   entry_plunge span with a drop of 1.0 mm or more. C36 did not split the
   difference between these two causes.

## The 8 G2 columns ('3D Rough 1' on Corne, '2 Back Rough' on wanaka200)

`audit_output.txt` holds the full printout; `audit_results.json` the numbers.

### 1. Descent events into stock (motion predicate; labels only for the cross-table)

| Column | Events | Labels: plunge entry / keep-down link (entry_plunge) / keep-down link (linking) | Drop median / max (mm) | Time (s) |
|---|---|---|---|---|
| corne contour_parallel | 41 | 28 / 10 / 3 | 4.0 / 18.5 | 18.7 |
| corne zigzag_pick D18 | 886 | 37 / 805 / 44 | 5.0 / 17.0 | 350.9 |
| corne zigzag_pick D20 | 1236 | 38 / 1194 / 4 | 5.44 / 15.0 | 545.9 |
| corne AgentSearch | 159 | 31 / 115 / 13 | 5.0 / 17.0 | 79.2 |
| wanaka200 contour_parallel | 752 | 488 / 114 / 150 | 2.277 / 17.728 | 592.3 |
| wanaka200 zigzag_pick D18 | 2629 | 16 / 2521 / 92 | 4.636 / 17.728 | 1345.9 |
| wanaka200 zigzag_pick D20 | 3412 | 18 / 3366 / 28 | 4.765 / 17.728 | 2050.8 |
| wanaka200 AgentSearch | 530 | 29 / 480 / 21 | 4.516 / 17.728 | 313.6 |

### 2. Junctions at feed (turn = the 3D angle between the outgoing and the incoming direction)

Histogram bins (deg): (0,1] / (1,5] / (5,15] / (15,30] / (30,45] / (45,60] / (60,90] / (90,135] / (135,180].

All junctions at feed, then the planar ones (both moves travel in XY: the
cutting path without the vertical lifts and plunges).

| Column | Junctions | Reversals (> 90 deg) | Exactly 90 deg | Z reversals | Planar junctions | Planar reversals | Planar turns over 30 deg | Planar turn histogram |
|---|---|---|---|---|---|---|---|---|
| corne contour_parallel | 7676 | 503 | 222 | 281 | 7506 | 503 | 5373 (71.6 %) | 54 / 8 / 432 / 1639 / 3267 / 308 / 1295 / 277 / 226 |
| corne zigzag_pick D18 | 9910 | 480 | 3735 | 544 | 5643 | 270 | 1009 (17.9 %) | 103 / 493 / 3481 / 557 / 143 / 49 / 547 / 93 / 177 |
| corne zigzag_pick D20 | 10438 | 639 | 4166 | 555 | 5570 | 354 | 1074 (19.3 %) | 186 / 625 / 3283 / 402 / 128 / 53 / 539 / 139 / 215 |
| corne AgentSearch | 3816 | 367 | 753 | 176 | 3216 | 359 | 1763 (54.8 %) | 1 / 11 / 743 / 698 / 330 / 173 / 901 / 199 / 160 |
| wanaka200 contour_parallel | 8802 | 1119 | 1471 | 2552 | 4819 | 293 | 2304 (47.8 %) | 236 / 510 / 960 / 809 / 653 / 540 / 818 / 268 / 25 |
| wanaka200 zigzag_pick D18 | 19519 | 2875 | 8323 | 3359 | 5761 | 102 | 367 (6.4 %) | 5 / 309 / 4684 / 396 / 146 / 53 / 66 / 33 / 69 |
| wanaka200 zigzag_pick D20 | 23121 | 3401 | 9455 | 3987 | 7136 | 22 | 221 (3.1 %) | 0 / 1011 / 5619 / 285 / 116 / 39 / 44 / 14 / 8 |
| wanaka200 AgentSearch | 7210 | 739 | 1951 | 1074 | 5121 | 352 | 2204 (43.0 %) | 2 / 91 / 2232 / 592 / 422 / 157 / 1273 / 317 / 35 |

### 3. MRR over time (removed volume per second of each sample, weighted by time)

Bins (mm³/s): 0 / (0,25) / [25,50) / [50,100) / [100,200) / [200,400) / [400,800) / [800,1600) / ≥ 1600, in seconds.
Nominal MRR: Corne 182.25 mm³/s, wanaka200 182.0 mm³/s (`run_all.py`).

| Column | p50 / p95 / p99 / max (mm³/s) | Time over nominal (s) | Time over 2x nominal (s) | Time per bin (s) |
|---|---|---|---|---|
| corne contour_parallel | 25.6 / 960.0 / 2974.9 / 22810.4 | 343.0 | 274.1 | 307.9 / 177.9 / 31.2 / 77.9 / 41.7 / 129.9 / 152.4 / 22.7 / 31.5 |
| corne zigzag_pick D18 | 1.9 / 537.4 / 2430.0 / 36834.6 | 270.8 | 226.4 | 565.0 / 279.7 / 53.2 / 124.0 / 46.0 / 70.4 / 163.7 / 4.3 / 24.2 |
| corne zigzag_pick D20 | 13.5 / 534.6 / 1887.9 / 24405.0 | 336.5 | 285.2 | 358.7 / 471.5 / 77.8 / 169.9 / 46.7 / 86.2 / 226.2 / 2.2 / 14.9 |
| corne AgentSearch | 3.5 / 1823.8 / 9713.3 / 35648.6 | 124.1 | 99.5 | 196.0 / 112.5 / 26.8 / 38.8 / 41.5 / 32.6 / 20.1 / 37.3 / 28.4 |
| wanaka200 contour_parallel | 27.0 / 182.0 / 563.1 / 2927.7 | 1760.3 | 188.1 | 2628.9 / 652.2 / 345.9 / 237.9 / 2486.3 / 123.0 / 162.9 / 7.1 / 1.6 |
| wanaka200 zigzag_pick D18 | 30.4 / 546.0 / 615.4 / 3051.7 | 870.4 | 546.9 | 1653.3 / 1188.0 / 1082.0 / 496.7 / 1088.1 / 208.2 / 507.8 / 3.1 / 2.4 |
| wanaka200 zigzag_pick D20 | 27.0 / 516.4 / 605.5 / 1369.2 | 870.7 | 487.7 | 2047.7 / 1386.1 / 1163.0 / 640.8 / 1144.8 / 265.1 / 446.9 / 0.8 / 0.0 |
| wanaka200 AgentSearch | 82.1 / 1097.8 / 1282.2 / 3051.7 | 923.6 | 476.1 | 363.0 / 234.2 / 402.7 / 273.1 / 284.1 / 437.2 / 231.5 / 217.6 / 5.6 |

### 4. Time split by motion (sample time; share of the sample time)

| Column | Cut | Fed in air | Lift feed | Plunge feed | Rapid | Sample time (s) | Toolpath summary runtime (s) |
|---|---|---|---|---|---|---|---|
| corne contour_parallel | 719.1 (73.9 %) | 111.1 (11.4 %) | 3.5 (0.4 %) | 26.9 (2.8 %) | 112.5 (11.6 %) | 973.2 | 973.2 |
| corne zigzag_pick D18 | 580.6 (43.6 %) | 193.8 (14.6 %) | 89.9 (6.8 %) | 391.2 (29.4 %) | 75.1 (5.6 %) | 1330.5 | 1330.5 |
| corne zigzag_pick D20 | 640.9 (44.1 %) | 89.1 (6.1 %) | 95.5 (6.6 %) | 545.3 (37.5 %) | 83.3 (5.7 %) | 1454.0 | 1454.0 |
| corne AgentSearch | 324.0 (60.7 %) | 51.5 (9.6 %) | 13.8 (2.6 %) | 84.5 (15.8 %) | 60.2 (11.3 %) | 534.0 | 534.0 |
| wanaka200 contour_parallel | 3509.7 (52.8 %) | 956.2 (14.4 %) | 279.6 (4.2 %) | 949.1 (14.3 %) | 951.2 (14.3 %) | 6645.8 | 5750.3 |
| wanaka200 zigzag_pick D18 | 3321.1 (53.3 %) | 410.3 (6.6 %) | 854.6 (13.7 %) | 1623.9 (26.1 %) | 19.8 (0.3 %) | 6229.7 | 3950.5 |
| wanaka200 zigzag_pick D20 | 3292.1 (46.4 %) | 227.8 (3.2 %) | 1226.6 (17.3 %) | 2329.7 (32.8 %) | 19.0 (0.3 %) | 7095.1 | 4832.2 |
| wanaka200 AgentSearch | 1834.0 (74.9 %) | 116.1 (4.7 %) | 157.9 (6.4 %) | 319.4 (13.0 %) | 21.6 (0.9 %) | 2449.0 | 1339.2 |

## Files

| File | What |
|---|---|
| `audit.py` | the instrument (one run folder) |
| `run_all.py` | the 8 G2 columns; `audit_results.json`, `audit_output.txt` |
| `make_readme.py`, `README_head.md`, `README_tail.md` | this file |
| `assert_audit.py` | its own reader of the G-code and the samples; re-derives every number in this file |

## Limits

- No junction speed: the machine profiles give no acceleration or junction
  deviation. The reversal rule needs neither.
- The time split uses the sample time. Every sample of the op falls on a
  G-code move of the op, so the split's sum equals the sum of the op's
  sample times on all 8 columns.
- The audit reads the first rough op only.
