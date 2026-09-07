# Roughing strategy A/B for heightfield terrain — results (2026-09-07)

This document reports the A/B run that
`planning/roughing_strategy_terrain_2026-09-07.md` designed. That document
designed the arms and ran nothing. This run executed the runnable arms on the
live GUI through the rs-cam MCP, one arm at a time, at one simulation cell
size. All artifacts (screenshots, per-arm project files) are in
`planning/roughing_strategy_ab_2026-09-07/`.

## Summary

- **Under the task's ranking rule** (wall-clock first, zero collisions, air and
  retract trips as the explanation, finish stock as the tie-breaker) the
  winner is **Arm BE**: ContourSpiral with `depth_per_pass` 5.46. It is
  25.7 % faster than the baseline. It raises air-cut and it hands the finish
  more standing material.
- **Under the scoping document's verdict criterion** (lower wall-clock AND
  lower `air_cut_pct_of_total_runtime`, no worse finish stock) only **Arm C2**
  passes: `region_ordering = by_area` alone. It is 8.1 % faster and lowers air
  from 56.5 % to 55.3 %. Its finish stock is inferred equal to the baseline
  because its Z-level structure is identical to the baseline. It was not
  finish-verified.
- **No runnable arm reaches a sane air ratio.** The best reading is 55.3 %
  against the 40 % threshold for 2.5D/3D roughs. The scoping document says this
  outcome is the evidence to fund Arm F. The evidence here weakens that case as
  specified. See "The hypothesis, answered".
- **Every arm is feed-bound**, not machine-bound. The belt router's
  acceleration is not the binding constraint for any strategy on this terrain.
  The commanded feed, clamped to the 0.025 mm/tooth rubbing floor, is.
- **Safety**: every arm reads 0 collisions and 0 rapid collisions at 0.2 mm.
  Every tool-load gate reads Within on every arm with a real population.

## Fixture and protocol

Fixture: `planning/airrun_2026-08-19/wanaka200.toml`, toolpath index 5
"6 3D Rough (front)", `adaptive3d`, 6 mm 2-flute flat end mill,
`stock_source = fresh`. Stock 240 × 250 × 25 mm, stock top at setup-local
Z = 7.0. The GUI binary was the release build of HEAD on branch
`machine-kinematics-confidence` (built 2026-09-07 before the run).

Protocol per arm, identical every time:

1. `load_project` on the original file. The GUI auto-starts generation on
   load. `cancel_generation` stopped it.
2. `set_toolpath_enabled false` on every toolpath except index 5. The front
   rough is fresh-stock, so it depends on no upstream operation. This
   deviation from the prompt was approved by the orchestrator and applied
   identically to every arm, including the baseline.
3. `set_toolpath_param` for ONLY the arm's parameters.
4. `generate_all(fixpoint = true, simulation_resolution_mm = 0.2,
   timeout_s = 900)`. Every rough-only arm finished in one round, no internal
   simulation. From Arm D onward the parameter write itself re-triggered an
   auto-regeneration with the correct parameters, and `generate_all` was
   issued on top of it.
5. `run_simulation(resolution = 0.2)`.
6. `narrate_toolpath(5)`, `get_tool_load_report`, `screenshot_simulation`,
   `save_project` to a scratch copy. The original file was never modified.
7. `free -g` before every `generate_all` and `run_simulation`. It never read
   below 19 Gi available.

Baseline field values, stated so that every arm is a known change:
`clearing_strategy = contour_parallel`, `region_ordering = global`,
`entry_style = plunge`, `stepover = 1.2`, `depth_per_pass = 4.2`,
`feed_rate = 750`, `plunge_rate = 541`, `spindle_rpm = 15000`,
`stock_to_leave_radial = 0.3`, `stock_to_leave_axial = 0.5`,
`fine_stepdown = 0.0`, `detect_flat_areas = false`,
`mill_shallow_areas = false`, `max_stay_down_distance_mm = null`
(planner default 8 × D = 48 mm), `min_region_cut_length_mm = 15`.

Finish-stock measure: the fixture's finish is "7 3D Finish (R1.5
drop_cutter)" (index 6, id 8, `stock_source = from_remaining_stock`,
stepover 0.3, F1260, 19 000 RPM). It is a drop_cutter, not the scallop the
scoping document mentions. It was re-enabled on the baseline and on two
candidate arms. `generate_all` then took 2 rounds and 1 internal simulation
per run.

## Arms run

| Arm | Change from baseline | In scoping doc? |
|---|---|---|
| A | none (baseline) | yes |
| B | `clearing_strategy = contour_spiral` | yes, gated on `recommend_clearing_strategy` |
| C | `region_ordering = by_area` + `max_stay_down_distance_mm = 150` | yes |
| C2 | `region_ordering = by_area` only | no — attribution sub-arm for C |
| D | `entry_style = ramp` (`ramp_angle_deg` 10) | yes |
| E | `depth_per_pass = 5.46` + `mill_shallow_areas = true` | yes |
| BE | B + E | yes — "pair bigger DOC with (3)" |
| C2E | C2 + E | yes — "pair bigger DOC with (2)" |

`recommend_clearing_strategy` on the baseline chose ContourSpiral, regime
MachineLimited, predicted 2207 s against 2745 s for ContourParallel
(ratio 1.244). The measured B/A ratio is 1.238. Arm B was therefore gated in.

## Rough-only results (index 5 alone, sim 0.2 mm)

Wall-clock is `total_runtime_s` from `run_simulation`, integrated through the
accel-aware model at the machine's kinematics (XY 500, Z 270 mm/s²,
junction deviation 0.02 mm).

| Arm | total_runtime_s | Δ vs A | air_cut_pct_of_total_runtime | cutting mm | rapid mm | moves | retract_trips | collisions / rapid | removed volume est. mm³ |
|---|---|---|---|---|---|---|---|---|---|
| A | 2578.1 | — | 56.51 | 49 237 | 11 407 | 13 239 | 471 | 0 / 0 | 155 744 |
| B | 2081.8 | −19.2 % | 69.67 | 42 210 | 4 555 | 20 171 | 57 | 0 / 0 | 146 796 (−5.7 %) |
| C | 2901.0 | +12.5 % | 92.63 | 63 575 | 4 295 | 11 168 | 58 | 0 / 0 | 156 052 |
| C2 | 2369.9 | −8.1 % | 55.30 | 46 468 | 8 966 | 12 106 | 433 | 0 / 0 | 155 763 |
| D | 2727.1 | +5.8 % | 60.93 | 50 818 | 12 070 | 14 087 | 477 | 0 / 0 | 155 738 |
| E | 2218.7 | −13.9 % | 59.56 | 42 150 | 8 328 | 10 908 | 311 | 0 / 0 | 154 107 (−1.1 %) |
| BE | 1916.4 | −25.7 % | 73.78 | 37 012 | 3 527 | 19 071 | 35 | 0 / 0 | 145 907 (−6.3 %) |
| C2E | 2112.1 | −18.1 % | 58.77 | 40 747 | 6 930 | 10 436 | 290 | 0 / 0 | 154 107 (−1.1 %) |

Removed volume is the sum of `per_depth_pass[].total_removed_volume_est_mm3`
from `get_tool_load_report`.

### Tool-load gates and triage actions

| Arm | chipload | deflection peak mm | power peak / avail kW | peak axial DOC mm | triage.actions |
|---|---|---|---|---|---|
| A | Within (0.0568 = band max) | 0.022 | 0.047 / 0.703 | 4.20 | crosses_standing 10.4 % > 4.11; plunge_class 7/588 at 1.4×; air_cut_high 57 % |
| B | Within | 0.022 | 0.047 / 0.703 | 4.20 | plunge_class 7/1118 at 1.4×; air_cut_high 70 % |
| C | Within | 0.022 | 0.047 / 0.703 | 4.20 | crosses_standing 14.7 % > 3.51; air_cut_high 93 % |
| C2 | Within | 0.022 | 0.047 / 0.703 | 4.20 | air_cut_high 55 % only |
| D | Within | 0.022 | 0.047 / 0.703 | 4.20 | crosses_standing 11.4 % > 3.87; plunge_class 7/493 at 1.4×; air_cut_high 61 % |
| E | Within (band derated to 0.0516) | 0.039 | 0.082 / 0.703 | **8.19** | crosses_standing 7.5 % > 5.10; plunge_class 1/390; air_cut_high 60 % |
| BE | Within (0.0516) | 0.039 | 0.084 / 0.703 | **8.19** | crosses_standing 9.0 % > 4.90; plunge_class 1/1213; air_cut_high 74 % |
| C2E | Within (0.0516) | 0.039 | 0.082 / 0.703 | **8.19** | crosses_standing 6.6 % > 5.35; air_cut_high 59 % |

Every gate population is real (contributing samples 74 736 to 193 860). No
gate abstained. `triage.safety` was empty on every arm.

### Kinematic utilization (from `get_tool_load_report`, read after `run_simulation`)

| Arm | utilization | feed_bound | machine_bound (accel / junction / rate) | headroom_at_1_30 | plunge.peak_ratio (over 1×) | feeds_provenance | fed_time_s |
|---|---|---|---|---|---|---|---|
| A | 0.9998 | 0.9928 | 0.0072 (0.0071 / 0.0001 / 0) | 0.2156 | 1.386 (7/588) | emitted | 2036 |
| B | 0.9967 | 0.9545 | 0.0455 (0.0330 / 0.0125 / 0) | 0.1677 | 1.386 (7/1118) | emitted | 1670 |
| C | 0.9998 | 0.9947 | 0.0053 (0.0053 / 0.0001 / 0) | 0.2193 | 1.000 (0/351) | emitted | 2685 |
| C2 | 0.9998 | 0.9926 | 0.0074 (0.0073 / 0.0001 / 0) | 0.2157 | 1.000 (0/538) | emitted | 1910 |
| D | 0.9998 | 0.9938 | 0.0062 (0.0062 / 0.0000 / 0) | 0.2167 | 1.386 (7/493) | emitted | 2126 |
| E | 0.9999 | 0.9982 | 0.0018 (0.0016 / 0.0001 / 0) | 0.2210 | 1.386 (1/390) | emitted | 1856 |
| BE | 0.9973 | 0.9701 | 0.0299 (0.0277 / 0.0022 / 0) | 0.1794 | 1.386 (1/1213) | emitted | 1592 |
| C2E | 0.9999 | 0.9983 | 0.0017 (0.0016 / 0.0001 / 0) | 0.2209 | 1.000 (0/361) | emitted | 1784 |

Reading: every arm runs at 99.7 % or more of its commanded feed. Every
parallel arm is more than 99 % feed-bound with machine_bound below 0.8 %.
Only the two spiral arms show machine_bound above 1 % (4.5 % and 3.0 %) and
lower headroom (0.17 to 0.18 against 0.22). The `narrate_toolpath` kinematics
sentence agrees on every arm ("100 % feed-bound; 0 % machine-bound and will
not move" on the parallel arms, "99 % / 1 %" on the spiral arms). The
`plunge.peak_ratio` of 1.386 is the same seven (or one) untagged vertical
descents at 750 mm/min against a 541 mm/min plunge rate; `by_area` ordering
removes them (C, C2, C2E read 1.000).

### Screenshots and saved arms

All under `planning/roughing_strategy_ab_2026-09-07/`:

| Arm | rough screenshot | saved project |
|---|---|---|
| A | `armA_sim.png` | `armA_baseline.toml` |
| B | `armB_sim.png` | `armB_contour_spiral.toml` |
| C | `armC_sim.png` | `armC_byarea_staydown150.toml` |
| C2 | `armC2_sim.png` | `armC2_byarea_only.toml` |
| D | `armD_sim.png` | `armD_ramp_entry.toml` |
| E | `armE_sim.png` | `armE_dpp546_shallow.toml` |
| BE | `armBE_sim.png` | `armBE_spiral_dpp546.toml` |
| C2E | `armC2E_sim.png` | `armC2E_byarea_dpp546.toml` |

What the screenshots show: the 6-view composite renders at 0.894 mm per
pixel. At that scale the roughed surface of every arm looks the same: the
terrain relief is present, no island is missed, no gouge is visible. The
composite cannot resolve sub-tool residual islands. The BE composite shows a
faint lighter margin just inside the stock border in the TOP view that the
other arms do not show. That margin is consistent with the spiral's lower
removed volume. The finish runs below are the measure that resolves it.

## Finish-stock results (index 5 + index 6, sim 0.2 mm)

The finish operation is mesh-driven. Its motion is identical in every run
(444 893 moves, 150 034 mm cutting, 208 mm rapid, 2 retract trips). Its
wall-clock is constant to ±0.3 %. What changes is its load: how much material
the rough left for it.

| Rough arm | project total_runtime_s | Δ vs A | finish crosses_standing_material | finish peak bite mm | finish `peak_removed_mm` | finish deflection mm | finish fed_time_s | collisions / rapid |
|---|---|---|---|---|---|---|---|---|
| A | 14 245.4 | — | 12.2 % > 0.56 mm | 2.93 at (38.7, 86.7) | 0.97 | 0.021 | 11 486.8 | 0 / 0 |
| BE | 13 544.5 | −4.9 % | 15.9 % > 0.62 mm | 3.14 at (120.3, 116.1) | 1.57 | 0.022 | 11 451.9 | 0 / 0 |
| C2E | 13 782.5 | −3.2 % | 12.6 % > 0.58 mm | 2.93 at (38.7, 86.7) | 1.17 | 0.021 | 11 491.2 | 0 / 0 |

Finish screenshots: `armA_finish.png`, `armBE_finish.png`,
`armC2E_finish.png`. The finished surface looks identical across the three at
composite scale. The finish reached every part of the surface in every run.

Finish kinematic utilization was 0.998 in every run, 99.5 % feed-bound,
`plunge.peak_ratio` 1.0, `feeds_provenance` emitted.

`entry_load`: no diagnostic with that id exists on this build.
`get_toolpath_diagnostics(6)` returned `feeds.chipload_clamped_to_floor`,
`load.chipload.within`, `load.chipload.commanded_above_band`,
`load.power.within` and `load.deflection.within`. In its place this run reports
the finish's 2 retract trips and its `plunge.peak_ratio` of 1.0 (45
vertical-dominant moves, none over the plunge rate).

The C2E finish stock lands between A and BE. Its peak bite and location are
the baseline's. Its `peak_removed_mm` of 1.17 against 0.97 is the single-level
drape leaving slightly taller steps than the two-level baseline.

## Ranked verdict

### Reading 1 — the task's rule (wall-clock, 0 collisions; air and retract trips explain; finish stock breaks ties)

1. **BE** — 1916 s (−25.7 %). 35 retract trips. Air 73.8 %. Finish stock
   worst of the three measured (15.9 % crossings, peak bite 3.14 mm, peak
   removed 1.57 mm).
2. **B** — 2082 s (−19.2 %). 57 retract trips. Air 69.7 %. Removed volume
   −5.7 %. Not finish-verified; BE's finish result is the proxy.
3. **C2E** — 2112 s (−18.1 %). 290 retract trips. Air 58.8 %. Finish stock
   near-baseline (12.6 %, 2.93 mm, 1.17 mm).
4. **E** — 2219 s (−13.9 %). 311 retract trips. Air 59.6 %.
5. **C2** — 2370 s (−8.1 %). 433 retract trips. Air 55.3 % (lowest).
6. **A** — 2578 s. Baseline.
7. **D** — 2727 s (+5.8 %).
8. **C** — 2901 s (+12.5 %).

### Reading 2 — the scoping document's criterion (lower wall-clock AND lower air, finish stock not worse)

Only **C2** passes. B, E, BE and C2E all raise `air_cut_pct_of_total_runtime`.
C2's finish stock is inferred equal to A: its per-level cut runs, cutting
moves and cutting distance are byte-identical to A (215/217 runs,
7247/3522 moves, 34 060/12 407 mm). `by_area` reorders the same runs. This
inference was not measured with a finish run.

The two readings disagree. The operator decides which bar applies. If the
question is "fastest safe rough that does not push work onto the finish",
C2E is the strongest measured answer: −18.1 % with finish stock at baseline
peak. If the question is "fastest safe rough, full stop", it is BE at
−25.7 %, with the finish absorbing a 1.6× peak bite.

### Scoring the scoping document's five ranked levers

| Doc rank | Lever | Result |
|---|---|---|
| 1 | Terrain-clipped boustrophedon fill (Arm F) | UNTESTED — not runnable, needs the adapter the doc describes |
| 2 | ByArea + raised stay-down (Arm C) | REFUTED as designed. Fed 750 mm/min stay-down links through air cost more than retract round trips on this terrain (+12.5 %). ByArea alone (C2) gains 8.1 %. |
| 3 | ContourSpiral (Arm B) | Wins wall-clock (−19.2 %). The advisor's prediction (1.244×) matched the measurement (1.238×). Loses on air (+13 pp) and removed volume (−5.7 %). |
| 4 | Bigger DOC (Arm E) | Wins (−13.9 %). Mechanism: 2 Z levels become 1; the in-region drape does the rest of the descent, which is where the 8.19 mm single-column bite comes from. |
| 5 | Ramp entry (Arm D) | The doc called it "a small, safe win". It lost 5.8 %. |

Not runnable, listed as UNTESTED: Arm F (terrain-clipped boustrophedon fill)
and Arm G (drop-cutter cascade, needs a `stock_to_leave` on
`DropCutterConfig`).

## The hypothesis, answered

The scoping document's verdict criterion says: if no runnable arm reaches a
sane air ratio, that is the evidence to fund Arm F. No runnable arm reached
it. The best air reading is 55.3 % against a 40 % threshold.

Two measurements weaken the case for Arm F as the document specifies it:

1. **Fragmentation rapids are already a minor term.** The document cites
   42 667 mm of rapid on the front rough. The baseline here emits 11 407 mm.
   The TSP rapid reorder of 2026-08-21 landed between the two readings. The
   spiral arm drops rapid to 3 527 mm and its air still rises.
2. **The residual air is in-cut drape air.** On the baseline, 51.3 % of
   IN-CUT samples read engagement below 0.02 (234 873 of 458 236). These are
   fed moves, not rapids. They are the document's mechanism 2: the emitted
   cut Z drapes to the surface and rides over cleared terrain between islands.
   A boustrophedon fill keeps the same drape and would ride through the same
   air.

The next arm should target in-cut drape air, not inter-island rapids. That is
a different fill contract from Arm F as written: one that lifts or skips a
draped chord when the stock above the surface is already gone, not one that
changes the XY pattern of the fill.

## Caveats

- **Measurability.** `radial_engagement`, `air_cut` and `chip_engagement`
  read `degraded` on every arm at 0.2 mm cells (reason
  `cell_too_coarse_for_tip_contact`, blind fraction 0.21 to 0.24). Cross-arm
  comparison at one cell size is valid. Absolute air levels are not.
- **Peak axial bite on the single-level arms.** E, BE and C2E remove up to
  8.19 mm at one column, 1.37 × the tool diameter, from a commanded step of
  5.46 mm. Deflection reads 0.039 mm, 78 % of the 0.05 mm validated bar. The
  gates read Within. The number is stated so the operator can judge it.
- **`mill_shallow_areas = true` produced no visible sub-pass.** The narration
  shows one Z level on E, BE and C2E. `per_depth_pass` has one row. Arm E is
  in effect "DPP 5.46, one level".
- **`runtime_by_intent` was not collected.** The rapid and cutting distances
  and the kinematic `fed_time_s` stand in for the intent split.
- **Air-cut denominators read inverted on every one-op run.** On every
  rough-only arm `air_cut_pct_of_total_runtime` is HIGHER than
  `air_cut_pct_of_cutting_time` (56.5 > 36.3 on A, 92.6 > 52.7 on C).
  `CLAUDE.md` states the cutting-time reading is always the larger one. On the
  two-op finish runs the order flips to the documented direction
  (33.2 < 42.4). This document reports the fields by their names and does not
  reinterpret them. Flagged to the orchestrator.
- **`crosses_standing_material` is a load signal, not an air signal.** The
  scoping document says so, and this document uses it only for finish-stock
  load.
- **Finish-stock inference for C2** is by structural identity with A, not by
  a finish run.
- **B was not finish-verified.** BE's finish result is the proxy for the
  spiral's coverage gap.

## Deviations from the prompt

- All toolpaths other than the front rough were disabled in every arm
  (orchestrator-approved). The whole-project 54 % figure the scoping document
  cites is therefore not directly comparable to the 56.5 % baseline here; the
  two runs differ in build, sim cell size and enabled operations.
- `load_project` auto-starts generation on this build. Every load was
  followed by `cancel_generation`, twice when the queue had already advanced.
- From Arm D onward the parameter write re-triggered an auto-regeneration
  with the correct parameters. `generate_all` was issued on top of it rather
  than after a further cancel. The result is the same toolpath.
- The scoping document's step 6 named "the existing R1.5 scallop". The
  fixture carries an R1.5 drop_cutter. That operation was used.
- The `entry_load` diagnostic named in the prompt does not exist on this
  build. See the finish-stock section for what was recorded instead.

## Files

- results (this file): `planning/roughing_strategy_ab_results_2026-09-07.md`
- artifacts: `planning/roughing_strategy_ab_2026-09-07/` (11 PNG, 11 TOML)
- scoping: `planning/roughing_strategy_terrain_2026-09-07.md`
- fixture (unmodified): `planning/airrun_2026-08-19/wanaka200.toml`
