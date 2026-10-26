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
