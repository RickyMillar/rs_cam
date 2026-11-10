# P24 compare: 18 mm router bit against 6 mm end mill for the wanaka 21 bulk rough (2026-11-11)

The operator's question: do the two bulk roughing ops of the wanaka 21 project stay on the
18 mm 2-flute router bit, or do they move to the 6 mm flat end mill?

This report gives measured facts only. It gives no recommendation.

## 1. Method

- Build: `target/release/rs_cam_gui`, built from HEAD `910dcf6d5`. `gui_status` gave
  `binary.newer_than_process: false` before the first variant and after each `gui_restart`.
- Source project: `planning/rivmap_block_job/nz_wanaka_21_operator.toml`. No step wrote to this
  file. Its sha256 was `f5149e33…912717` before the first variant and after the last one.
- Each variant started from `gui_restart`, then `load_project` of the operator file with
  `discard_unsaved: true`.
- Each variant used these steps, in this order:
  1. `set_spindle_strategy`: `constant_chip`, `rpm_ceiling` 18000. The saved files hold
     `spindle_rpm_ceiling = 18000` (the operator file holds 24000).
  2. The edit of the variant (table below).
  3. `set_toolpath_enabled false` on idx 3 to 11, 13, 16 and 17. Idx 0, 1, 2, 12, 14 and 15
     stayed enabled.
  4. `apply_feeds` on idx 1, 2, 14 and 15.
  5. `save_project`, `generate_all`, `run_simulation` at the stored resolution (0.2 mm, fixed).
  6. `get_tool_load_report` and `get_toolpath_diagnostics`.
- No `apply_feeds` ran on the two face ops (idx 0 and idx 12). They keep the feeds of the
  operator file (18000 RPM, feed 6488, plunge 2162) in all four variants.
- All four variants completed. The total time was 37 minutes (23:42 to 00:19 NZDT).
- The GUI holds no simulation now: a `gui_restart` ran after variant D.

| Variant | File (in `planning/rivmap_block_job/`) | Edit | Pick for idx 1 and idx 14 |
|---|---|---|---|
| A | `nz_wanaka_21_cmp_a_18mm_spiral_model.toml` | none (18 mm tool: `up_cut`, helix 30) | none |
| B | `nz_wanaka_21_cmp_b_18mm_straight.toml` | 18 mm tool `cut_direction = "straight"` | none |
| C | `nz_wanaka_21_cmp_c_6mm_rough.toml` | idx 1 and idx 14 bound to the 6 mm flat end mill (tool id 0) | `"proposal"` |
| D | `nz_wanaka_21_cmp_d_18mm_straight_6mm_depth.toml` | as B | point: ap 6.0 mm, the stepover the op holds |

Notes on the edits:

- B and D: `set_tool_param` accepted `"straight"`. The reply was `"applied":"straight"` with no
  warning. The saved file holds `helix_deg = 0.0` and `cut_direction = "straight"`. The chip
  row of the 18 mm tool changed from `amana-flat-hardwood-pocket-12700-2f-spektra` (scaled
  x1.2371 from 12.7 mm, point 0.1791 mm/tooth) to `onsrud-hardwood-48-000-3_4-straight`
  (scaled x0.9640 from 19.05 mm, band 0.1959 to 0.2449 mm/tooth).
- C: the fallback (stepover 40 % of the diameter, depth 1 x the diameter) was not necessary.
  The proposal keeps a stepover of 5.5 mm on idx 1, which is not wider than the 6 mm tool, and
  the generator did not refuse.
- D: the pick raised the depth of idx 14 from 4.166 mm to 6.0 mm. The depth of idx 1 went
  from 10.0 mm to 6.0 mm.

### Safe envelope of variant C (`get_safe_envelope`, 6 mm tool)

| Field | idx 1 | idx 14 |
|---|---|---|
| `stated` | ap 10.0, ae 5.5 | ap 4.166, ae 3.6 |
| `operator` | outside, binding `vendor_axial`, margin -0.667 | inside, margin 0.306 |
| `proposal` | ap 6.0, ae 5.5 | ap 4.166, ae 3.6 (equal to stated) |
| `proposal_binding` | `vendor_axial` | null |

The `vendor_axial` limit is ap 6.0 mm, from row `amana-flat-hardwood-pocket-6000-2f-spektra`.

## 2. Applied values

Format: RPM / feed mm/min / plunge mm/min / ramp feed mm/min, then depth x stepover in mm.

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 1 Back bulk rough | 18000 / 6842 / 2162 / 6448, 10.0 x 5.5 | 18000 / 8417 / 2162 / 7932, 10.0 x 5.5 | 18000 / 4572 / 2286 / 4572, 6.0 x 5.5 | 18000 / 8417 / 2162 / 7932, 6.0 x 5.5 |
| idx 2 Back rest 6 mm | 18000 / 5131 / 2285 / 4571, 6.0 x 1.5 | same as A | same as A | same as A |
| idx 14 Front bulk rough | 18000 / 7798 / 2162 / 6448, 4.166 x 3.6 | 18000 / 9593 / 2162 / 7932, 4.166 x 3.6 | 18000 / 4572 / 2286 / 4572, 4.166 x 3.6 | 18000 / 9593 / 2162 / 7932, 6.0 x 3.6 |
| idx 15 Front 3D rough | 18000 / 5267 / 2285 / 4571, 5.532 x 1.382 | same as A | same as A | same as A |

- The operator file held feed 8475 (idx 1), 5279 (idx 2), 8060 (idx 14) and 5429 (idx 15).
- Each apply on the 18 mm tool named one rule with no source:
  `material.plunge_rate_base.solid_wood`. The applies on the 6 mm tool named none.
- No `apply_feeds` call refused. No `apply_feeds` call gave a warning.

## 3. Results per op

### Moves

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 Face back | 317 | 317 | 317 | 317 |
| idx 1 Back bulk rough | 14628 | 14628 | 26603 | 23628 |
| idx 2 Back rest 6 mm | 20044 | 20044 | 8374 | 17571 |
| idx 12 Face front | 371 | 371 | 371 | 371 |
| idx 14 Front bulk rough | 22517 | 22517 | 29834 | 17649 |
| idx 15 Front 3D rough | 40124 | 40124 | 0 | 40946 |

### Cutting distance (m) and rapid distance (m)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | 24.0 / 1.1 | 24.0 / 1.1 | 24.0 / 1.1 | 24.0 / 1.1 |
| idx 1 | 28.6 / 0.7 | 28.6 / 0.7 | 59.6 / 5.0 | 54.4 / 1.3 |
| idx 2 | 40.1 / 3.8 | 40.1 / 3.8 | 17.4 / 1.6 | 38.0 / 3.6 |
| idx 12 | 25.1 / 2.0 | 25.1 / 2.0 | 25.1 / 2.0 | 25.1 / 2.0 |
| idx 14 | 54.7 / 3.3 | 54.7 / 3.3 | 70.1 / 9.0 | 42.0 / 1.1 |
| idx 15 | 51.8 / 13.0 | 51.8 / 13.0 | 0.0 / 0.0 | 54.9 / 13.8 |

### Fed time (s), `kinematic_utilization.fed_time_s`

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | 227.5 | 227.5 | 227.5 | 227.5 |
| idx 1 | 253.4 | 195.5 | 820.8 | 364.6 |
| idx 2 | 583.9 | 583.9 | 247.9 | 549.2 |
| idx 12 | 231.3 | 174.2 | 231.3 | 174.2 |
| idx 14 | 464.2 | 361.6 | 939.0 | 277.4 |
| idx 15 | 811.6 | 811.6 | 0.0 | 848.2 |

The fed time of idx 12 changes in B and D although its stored feed did not change. The
modulation raised the feed of its moves (median +36.2 %) to the straight-bit chip row.

### Power: peak kW, and percent of the available 1.125 kW

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | unmodeled | unmodeled | unmodeled | unmodeled |
| idx 1 | 1.125 (100 %) | 1.125 (100 %) | 0.388 (34 %) | 0.897 (80 %) |
| idx 2 | 0.481 (43 %) | 0.481 (43 %) | 0.229 (20 %) | 0.481 (43 %) |
| idx 12 | 0.091 (8 %) | 0.111 (10 %) | 0.091 (8 %) | 0.111 (10 %) |
| idx 14 | 0.769 (68 %) | 0.941 (84 %) | 0.218 (19 %) | 1.125 (100 %) |
| idx 15 | 0.301 (27 %) | 0.301 (27 %) | unmodeled (no moves) | 0.251 (22 %) |

- Every modeled power verdict is `within`.
- A peak of 1.125 kW is the limit itself: the modulation holds the feed at the power limit
  (`power_max` binds on 0.1 % to 0.6 % of the moves of that op).
- Idx 0 is `unmodeled` in all variants: every sample cuts air. The face pass is at the stock
  top.

### Peak tip deflection (µm), limit 200 µm

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | unmodeled | unmodeled | unmodeled | unmodeled |
| idx 1 | 7.3 | 10.1 | 29.1 | 9.3 |
| idx 2 | 21.0 | 21.0 | 16.7 | 21.0 |
| idx 12 | 0.5 | 0.6 | 0.5 | 0.6 |
| idx 14 | 3.9 | 4.9 | 20.1 | 6.3 |
| idx 15 | 24.9 | 24.9 | unmodeled (no moves) | 22.2 |

Every modeled deflection verdict is `within`.

### Peak cutting force (N), `cut_force.peak_n` (no limit on this op family)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 1 | 137.5 | 189.2 | 88.8 | 170.3 |
| idx 2 | 83.3 | 83.3 | 46.4 | 83.3 |
| idx 14 | 62.5 | 78.8 | 53.2 | 104.1 |
| idx 15 | 69.7 | 69.7 | 0.0 | 60.2 |

### Chipload verdict (peak observed / bound, mm per tooth)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | unmodeled | unmodeled | unmodeled | unmodeled |
| idx 1 | within 0.1791 / 0.1791 | within 0.2449 / 0.2449 | within 0.1270 / 0.1270 | within 0.2449 / 0.2449 |
| idx 2 | within 0.1270 / 0.1270 | same | same | same |
| idx 12 | within 0.1791 / 0.1791 | within 0.2449 / 0.2449 | within 0.1791 / 0.1791 | within 0.2449 / 0.2449 |
| idx 14 | within 0.1791 / 0.1791 | within 0.2449 / 0.2449 | within 0.1270 / 0.1270 | within 0.2449 / 0.2449 |
| idx 15 | within 0.1270 / 0.1270 | same | unmodeled (no moves) | within 0.1270 / 0.1270 |

Info-level text on the median chip (not a finding of caution level):

- A: idx 1 median 0.1059, idx 14 median 0.1336, both "BELOW the 0.1791 mm/tooth printed point.
  The tool can rub."
- B: idx 1 median 0.1438, idx 14 median 0.1771, both "BELOW the 0.1959 mm/tooth burn floor.
  The tool can rub."
- D: idx 1 median 0.1413, idx 14 median 0.1876, both below the 0.1959 burn floor.
- C: idx 1 and idx 14 "Peak chip within the vendor range (0.1270 mm)".
- Idx 2 and idx 15 (6 mm tool, all variants where they cut): median 0.099 to 0.109, "BELOW
  the 0.1270 mm/tooth printed point. The tool can rub."

### Depth verdict (peak engaged depth, mm)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | within 0.00 (vacuous) | within 0.00 (vacuous) | within 0.00 (vacuous) | within 0.00 (vacuous) |
| idx 1 | within 10.00 of 36 | within 10.00 of 36 | within 9.00 of 12 | within 9.00 of 36 |
| idx 2 | exceeds 20.70 of 12 | exceeds 20.70 of 12 | exceeds 16.95 of 12 | exceeds 20.70 of 12 |
| idx 12 | within 0.50 | within 0.50 | within 0.50 | within 0.50 |
| idx 14 | within 5.73 of 36 | within 5.73 of 36 | within 4.66 of 12 | within 5.50 of 36 |
| idx 15 | within 6.41 of 12 | within 6.41 of 12 | within 0.00 (vacuous) | within 5.38 of 12 |

The limit is 2.00 x the tool diameter (machine rigidity factor). The app states that this rule
has no published source.

In C and D, idx 1 holds a depth per pass of 6.0 mm, and the simulation measured a peak engaged
depth of 9.0 mm.

### Modulation (moves touched / moves total, median feed change, binding distribution)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | 0 / 60, 0 %, machine_max_feed 1.00 | same | same | same |
| idx 1 | 14528 / 14606, -5.7 %, chipload_max 0.51, machine_max_feed 0.45, kinematic_reach 0.03, feed_cap 0.01 | 14559 / 14606, +4.7 %, machine_max_feed 0.48, chipload_max 0.47, kinematic_reach 0.04, feed_cap 0.01 | 11441 / 26401, -16.0 %, machine_max_feed 0.39, chipload_max 0.36, kinematic_reach 0.21, plunge_rate 0.03, feed_cap 0.01 | 23503 / 23593, +4.7 %, machine_max_feed 0.48, chipload_max 0.47, kinematic_reach 0.04, feed_cap 0.01 |
| idx 2 | 19750 / 19906, -10.9 %, chipload_max 0.37, machine_max_feed 0.30, feed_cap 0.18, kinematic_reach 0.13, plunge_rate 0.01 | same as A | 8257 / 8317, -10.9 %, chipload_max 0.43, machine_max_feed 0.25, feed_cap 0.16, kinematic_reach 0.16 | 17318 / 17444, -10.9 %, chipload_max 0.42, machine_max_feed 0.22, feed_cap 0.20, kinematic_reach 0.14, plunge_rate 0.01 |
| idx 12 | 180 / 182, -0.9 %, machine_max_feed 0.36, kinematic_reach 0.32, chipload_max 0.32 | 180 / 182, +36.2 %, same distribution | same as A | same as B |
| idx 14 | 22058 / 22469, -17.3 %, chipload_max 0.56, machine_max_feed 0.32, kinematic_reach 0.06, feed_cap 0.06 | 22083 / 22469, -8.1 %, chipload_max 0.50, machine_max_feed 0.34, kinematic_reach 0.10, feed_cap 0.07 | 11620 / 28795, -0.4 %, chipload_max 0.64, kinematic_reach 0.23, machine_max_feed 0.08, feed_cap 0.06 | 17404 / 17628, -8.1 %, chipload_max 0.54, machine_max_feed 0.30, kinematic_reach 0.10, feed_cap 0.05, power_max 0.01 |
| idx 15 | 38184 / 39518, -13.2 %, chipload_max 0.37, kinematic_reach 0.30, feed_cap 0.17, machine_max_feed 0.16 | same as A | none (no moves) | 37835 / 40389, -13.2 %, kinematic_reach 0.34, chipload_max 0.30, machine_max_feed 0.22, feed_cap 0.15 |

`power_max` also binds on idx 1 in A (0.001) and B (0.005).

### Findings of critical or caution level (`get_toolpath_diagnostics`)

| Op | A | B | C | D |
|---|---|---|---|---|
| idx 0 | none | none | not read again (same op, same tool as A) | not read again (same op, same tool as B) |
| idx 1 | caution `safety.helix_inside_core`: 9 + 19 helix entries fall back to the ramp | same as A | caution `project.sim_outside_envelope`: 677 of 149879 samples, vendor axial bound +50 % (depth 9.00 mm against 6.00 mm) | caution `safety.helix_inside_core`: 25 + 34 helix entries fall back to the ramp |
| idx 2 | caution `load.depth.exceeds` (20.70 of 12.00 mm); caution `project.sim_outside_envelope` (301 of 57551, +245 %); **critical** `project.entry_load` (182 of 46299 entry samples, peak 14.70 mm) | same as A | caution `load.depth.exceeds` (16.95 of 12.00 mm); caution `project.sim_outside_envelope` (12 of 28706, +182 %); **critical** `project.entry_load` (388 of 13625, peak 14.50 mm) | caution `load.depth.exceeds` (20.70 of 12.00 mm); caution `project.sim_outside_envelope` (232 of 57227, +245 %); **critical** `project.entry_load` (181 of 40270, peak 14.70 mm) |
| idx 12 | caution `project.sim_outside_envelope` (211 of 90004, corner coverage +15 %) | same as A | same as A | not read again (same op, same tool as B) |
| idx 14 | **critical** `safety.non_centre_cutting_descent`: 1 move, 1.03 mm of wood over the tip | same as A | none | **critical** `safety.non_centre_cutting_descent`: 6 moves, 1.58 mm of wood over the tip; **critical** `project.rapid_collision`: 1 collision |
| idx 15 | caution `project.sim_outside_envelope` (44 of 67397, +7 %); **critical** `project.entry_load` (1825 of 28721, peak 4.19 mm) | same as A | project caution `project.generated_empty` (0 moves) | **critical** `project.entry_load` (5046 of 31196, peak 5.20 mm) |

Count of critical findings: A 3, B 3, C 1, D 4.

### Simulation verdict

| | A | B | C | D |
|---|---|---|---|---|
| Headline | "WARNING: 55.5% air cutting of total runtime" | "WARNING: 56.6% air cutting of total runtime" | "OK" | "WARNING: 1 rapid-through-stock collisions" |
| `collision_count` | 0 | 0 | 0 | 0 |
| `rapid_collision_count` | 0 | 0 | 0 | 1 |
| `collision_checks_failed` | 0 | 0 | 0 | 0 |
| `total_runtime_s` (six ops, rapids included) | 3579 | 3424 | 3119 | 3548 |
| Tool-load summary | 5 within, 1 exceeds (idx 2 depth) | same | same | same |

## 4. Totals of the roughing chain per setup (fed time)

| Setup | A | B | C | D |
|---|---|---|---|---|
| Back (idx 1 + idx 2) | 837.3 s | 779.4 s | 1068.7 s | 913.8 s |
| Front (idx 14 + idx 15) | 1275.7 s | 1173.1 s | 939.0 s | 1125.5 s |
| Back + Front | 2113.1 s (35.2 min) | 1952.5 s (32.5 min) | 2007.7 s (33.5 min) | 2039.4 s (34.0 min) |
| Two face ops (idx 0 + idx 12) | 458.8 s | 401.7 s | 458.8 s | 401.7 s |
| All six ops | 2571.9 s | 2354.2 s | 2466.5 s | 2441.1 s |

In C, the Front total is idx 14 alone: idx 15 generated no moves.

## 5. Refusals and warnings, verbatim

No call refused. These calls gave a warning or a finding at the time of the edit.

Variant C, `set_toolpath_tool` idx 1 to tool id 0 (reply `ok: true`):

- caution, `project.outside_envelope`: "Cut 10.00 x 5.50 mm is outside the safe envelope:
  vendor axial bound The point is 67 % past the vendor axial bound limit. The value is written;
  it is a warning (ruling Q6)."
- hint, `quality.stepover_over_corner_coverage`: "The stepover is 92 % of the tool diameter,
  past the corner-coverage bound of 85 %: a sharp offset corner keeps uncut material." This
  hint stays after the `"proposal"` pick, because the proposal keeps the stepover of 5.5 mm.
- info, `feeds.long_tool_derate`: "The tool sticks out 30.0 mm, which is 5.0 × its diameter
  (6.000 mm). The calculator lowers the load target to 88 % to limit deflection. This limit
  has no published source. The feed rate does not change."

Variant C, `set_toolpath_tool` idx 14 to tool id 0: the `feeds.long_tool_derate` info only.

Variant C, simulation and project diagnostics:

- "WARNING: TP21 'Front 3D rough' (3D Adaptive) generated but produced zero in-material cut"
- `project.generated_empty`: "TP21 'Front 3D rough' (3D Adaptive) generated but produced zero
  in-material cut. Check that the setup orientation, the stock position and the model overlap.
  For a Project Curve, a positive depth goes into the material, and a negative depth lifts the
  tool into the air. The negative-depth check flags this before generation, and its fix flips
  the sign."

Variants A and B, simulation verdict (identical text in both):

- "WARNING: TP20 'Front bulk rough 18 mm': the tool does not cut at its centre, and 1 move(s)
  push the centre into wood, measured by the simulation (worst: move 18235 at X116.434 Y291.638
  Z17.983 (G-code frame), 1.03 mm of wood over the tip, descent 7.1 deg; the limits are a
  1.00 mm plunge, and at most 1.00 mm of descent along any 8.00 mm of path)"

Variant D, simulation verdicts:

- "WARNING: rapid collisions on TP20 'Front bulk rough 18 mm' (1 collisions, deepest at local
  move 13984 (run move 55871), z=20.723)"
- "WARNING: TP20 'Front bulk rough 18 mm': the tool does not cut at its centre, and 6 move(s)
  push the centre into wood, measured by the simulation (worst: move 17632 at X121.683 Y291.531
  Z17.923 (G-code frame), 1.58 mm of wood over the tip, descent 0.2 deg; the limits are a
  1.00 mm plunge, and at most 1.00 mm of descent along any 8.00 mm of path)"
- "NOT MEASURED: air-cut % withheld for 1 toolpath(s) — 'Front 3D rough': 54% of
  material-removing samples read zero engagement at 0.200 mm cell, while removing up to
  3.726 mm: the contact patch is narrower than the grid can resolve. Re-simulate below the
  tool's TIP radius."

Variants A, B and D, idx 1, `safety.helix_inside_core` (A and B text; D has 25 and 34 entries):

- "centre cutting no, stated: 9 helix entries with no room for the helix: the entry centre is
  on the edge of the region the helix must stay inside; the entry falls back to the ramp (0
  plunge(s) in all), first at X152.710 Y111.782 Z20.500; 19 helix entries with helix radius
  below the core radius: room for r 1.5 mm only, under the core radius 4 mm (a smaller helix
  leaves a post under the core); the entry falls back to the ramp (0 plunge(s) in all), first
  at X36.282 Y206.281 Z16.500"

All variants, idx 2, `load.depth.exceeds` (A text):

- "Depth of cut 20.700 mm is more than the rule-of-thumb limit of 12.000 mm (2.00 × the
  6.00 mm diameter). The rule has no published source and does not stop the export"

All variants, idx 2, `project.entry_load` (A text):

- "entry motion cuts far harder than the pass does: 182 of 46299 entry samples remove more
  than 2.21 mm (2x this pass's own 1.11 mm median body bite), peaking at 14.70 mm at (138.0,
  34.8, 10.71). The load gates skip entry spans, so nothing else grades this."

## 6. What the numbers say

Time:

- B has the shortest fed time for the roughing chain: 1952.5 s. C follows at 2007.7 s, then D
  at 2039.4 s, then A at 2113.1 s. The spread from the shortest to the longest is 160.6 s
  (8 %).
- Per setup, the order differs. B is the shortest on the Back (779.4 s). C is the shortest on
  the Front (939.0 s) and the longest on the Back (1068.7 s).
- In C, the two bulk ops take 1759.8 s with the 6 mm tool. In B, they take 557.1 s with the
  18 mm tool. C gets the time back in the rest ops: idx 2 falls from 583.9 s to 247.9 s, and
  idx 15 has no moves.
- C has the shortest simulated total runtime with rapids (3119 s against 3424 s to 3579 s).
- The straight-bit row (B against A) raises the applied feed of idx 1 from 6842 to 8417 mm/min
  and of idx 14 from 7798 to 9593 mm/min. The depth and the stepover do not change.

Power:

- C has the lowest peak power: 0.388 kW (34 % of the available power) on the worst op.
- A, B and D each have one op at the limit of 1.125 kW: idx 1 in A and B, idx 14 in D.
- In D, the depth of 6 mm lowers the peak of idx 1 to 0.897 kW, and the depth of 6 mm raises
  the peak of idx 14 from 0.941 kW to 1.125 kW (its depth was 4.166 mm).

Deflection:

- All modeled peaks are below 30 µm against the limit of 200 µm.
- On the bulk ops, the 18 mm tool gives 3.9 to 10.1 µm and the 6 mm tool gives 20.1 to
  29.1 µm.
- The worst op per variant: A 24.9 µm (idx 15), B 24.9 µm (idx 15), C 29.1 µm (idx 1),
  D 22.2 µm (idx 15). D has the lowest worst value and C has the highest.

Findings:

- C has one critical finding (entry load on idx 2). A and B have three each. D has four.
- C is the only variant with no centre-core finding on idx 14. The 6 mm tool has no such
  check in these replies; the 18 mm tool states that it does not cut at its centre.
- D is the only variant with a rapid collision (1, on idx 14).
- C is the only variant with an empty op (idx 15) and with the corner-coverage hint on idx 1.
- The critical entry-load finding on idx 2 is in all four variants.
- A and B have the same set of critical and caution findings, with the same numbers.

## 7. Not measured

- The stock that stays after the roughing chain. This run did not compare the end state of C
  with the end state of A, B and D. The corner-coverage hint on idx 1 in C says that a sharp
  offset corner keeps uncut material at a stepover of 92 %.
- The effect on the ops after the chain (idx 3 to 11, 13, 16, 17). They were disabled.
- Tool-change time. In C the roughing chain uses the 6 mm tool for idx 1, 2 and 14; the face
  ops stay on the 18 mm tool in all variants.
- The effect of the helix on the power of variant A. The power verdict of every op, in every
  variant, states "force line curti2021_density_law_helix0_envelope … helix 0 (no helix
  input)". The power of A and B differs through the feed and the chip row only.
- Power, deflection and chipload of idx 0 (all variants) and of idx 15 in C: `unmodeled`.
- Air-cut percent of idx 15 in D: the app withheld it (text in section 5).
- The cause of the rapid collision in D. `inspect_collisions` was not in the tool list of this
  run.
- The diagnostics of idx 0 in C and of idx 0 and idx 12 in D were not read again. These ops
  have the same tool and the same parameters as in A (for C) and B (for D).
- `get_suggest_rationale` was not read.
- Each variant ran one time. This run did not check that a second run gives the same numbers.
- Generation time, as a side note: A, B and D took about 4.5 minutes each. C took about
  13.5 minutes; idx 14 with the 6 mm tool took about 11 minutes.

## 7. Lead reading (2026-11-11, for the operator)

- The four variants finish the roughing chain within 8 % of each other in fed time. The 18 mm bit
  clears its own ops three times faster, and the 6 mm rest ops then take that time back. With the
  rapids, the 6 mm variant (C) is the shortest run.
- C is the only variant with a clean simulation headline, no core finding and no op at the power
  limit: 0.39 kW of 1.125 on its worst op, against 1.125 of 1.125 for the 18 mm variants. It keeps
  one critical (the idx 2 entry load, which every variant has).
- C charges the 6 mm spiral tool the straight-blade force line (`P24b_survey.md`), the cautious
  side for it.
- Marking the 18 mm tool straight (B) changes the applied feed only (6842 -> 8417 on idx 1, the
  Onsrud 48-000 row). The simulated power and deflection gates already read helix 0 for every
  tool, so B's verdicts equal A's. The lead's earlier sentence to the operator ("the app
  under-reads the 18 mm tool's peak force") holds for the pre-simulation engagement only, not for
  the simulated gates.
- The deflection of the 18 mm tool (4-10 um) assumes a solid carbide body (the tool file says
  carbide). A carbide-tipped bit has a steel body: about three times the deflection, still far
  inside 200 um. No source gives a shank stress limit, so the app cannot judge the shank.
- Gentler is not simply safer on the 18 mm bit: D (6 mm depth on both bulk ops) RAISED idx 14 from
  its 4.166 mm to 6 mm and gave six core contacts and one rapid collision. Open defects to look
  at: the rapid collision in D (cause not found), and idx 1 at ap 6 measuring a 9 mm engaged depth
  in C and D.
- Not measured: the stock left after each chain (C's end state against the others), tool-change
  time, and a second run of each variant.
