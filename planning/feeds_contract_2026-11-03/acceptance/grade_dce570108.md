# Grade of build dce570108 (P21c), 2026-11-10

A fresh agent graded the build through the read-only MCP tools. No cargo run, no edit under
`crates/`, no `set_*`, apply, generate, simulate, save or load call.

Build note (as it is): the GUI process started 2026-11-10 13:54:43 and runs
`target/release/rs_cam_gui --mcp`, mtime 1794270915 (13:35:15). That is EARLIER than the commit
`dce570108` (commit time 1794272543, 14:02:23). Lead statement: the binary was built from the
P21c working tree shortly before the commit; after that build only test files, the
literature-matrix shim and one snapshot changed (no product source). The README step 1 rule is
not met to the letter; the lead ruled to grade. The grader did not call `gui_status`. HEAD at
the read was `3898f1b82` (planning only, after `dce570108`). `git status crates` is empty.

- Grade: PASS 64, FAIL 6, NEEDS-RULING 28, N/A 10 (108 rows).
- The previous grade (build b9e67b24e, P20e): PASS 47, FAIL 23, NEEDS-RULING 28, N/A 10.
- Status counts: DERIVED 58, UNSOURCED 33, N/A 10, SOURCED 6, POLICY 1 (no change).
- P21_DESIGN section 6.1 predicted PASS 63, FAIL 7. The difference is in "Rows that the build
  moved" below.

## Inputs (README step 3)

- `list_toolpaths`: 18 toolpaths, index 0-17, the names in the table.
- One input changed: idx 1 `entry_style` is `helix` (operator ruling 2026-11-10;
  `helix_radius_factor` 0.3, `helix_pitch` 2.0). Every other input that the script reads is the
  same as at b9e67b24e: stepover, `depth_per_pass`, `trochoid_cap_mult` (1.6), the tools.
- The project file states `spindle_strategy = "constant_chip"` and `spindle_rpm_ceiling =
  24000` (lines 50-51) and `rpm_max = 24000.0` (line 94). The two new lines moved the SRC
  lines: MACH is now 60-66 and 85-89; T6CC is now 384-387. The script and the header quote the
  new line numbers. The quoted text did not change.
- Nobody applied Suggest after P21c. Each stored param holds its old value (18000 rpm; 10610
  on idx 5 and 17). The table grades the Suggest column.

## Table changes (operator rulings 2026-11-10 only)

1. Spindle policy constant chip, ceiling 24000: recorded in the header. No expected value
   changed. The table grades P1 (b) since 2026-11-08, and the ceiling is equal to the spindle
   max.
2. idx 1 helix: `TP[1]` entry is `helix`. The script has a ramp rule (a full slot, lift 1) and
   no helix rule. The idx 1 "ramp feed" row is NEEDS-RULING with no candidate (new question
   H1 in the header). It was NEEDS-RULING before (U1 bracket), so the count does not move.

Three note strings held typed app numbers (240 and "3.7x", 1010, "1347 vs 1582"). They now
read the APP dict. No expected value, candidate or status depends on them.

## Rows that the build moved

FAIL to PASS, 18 rows (all rpm rows):

| idx | Expected | App before | App now | Binding in the app |
|---|---|---|---|---|
| 0, 1, 2, 6-8, 10-16 | 24000 | 18000 | 24000 | spindle max |
| 3, 4 | 19685 | 18000 | 19685 | Z axis rate |
| 17 | 24000 | 10610 | 24000 | spindle max |
| 5 | 24000 | none (stored 10610) | 24000 | spindle max |
| 9 | 24000 | none (stored 18000) | 24000 | spindle max |

idx 5 and 9: Suggest now prints the RPM in `basis.rpm.value` (lead ruling Q13), while
`shipped_power` stays `{"unmodeled": "DepthAfterSimulation"}`. The design predicted FAIL for
these two rows; they pass.

PASS to FAIL, 1 row (new):

- idx 5 feed: expected 6096-8534 at the app rpm 24000; app 3165. Suggest prints an RPM and no
  feed for this op, so the script grades the stored feed (3165, written at 10610 rpm) at the
  Suggest RPM. At 24000 rpm a feed of 3165 is a chip of 0.066 mm, below the Onsrud band
  0.127-0.178 mm. At its own 10610 rpm the stored feed passes, as before. The row is a true
  FAIL only if an apply writes the RPM and leaves the feed; the grader did not call apply.

Rows that kept the grade with a new value (graded at the app rpm, so a feed that scales with
n keeps its grade):

| idx | Quantity | Before | Now | Grade |
|---|---|---|---|---|
| 0, 12 | feed / ramp | 6488 / 6447 | 8651 / 8597 | NEEDS-RULING (U1) |
| 1 | feed / entry feed | 6999 / 6448 | 9332 / 8597 | NEEDS-RULING (U1; H1) |
| 2 | feed / plunge / ramp | 5279 / 2285 / 4571 | 7039 / 2500 / 6095 | PASS |
| 3, 4 | drill feed = plunge | 2286 | 2499 | PASS |
| 5 | plunge | 1347 | 2500 | PASS |
| 6 | feed | 1010 | 1347 | NEEDS-RULING (U2) |
| 7, 8 | feed (8: ramp too) | 990 | 1320 | NEEDS-RULING (U2) |
| 10, 13 | feed / plunge | 2286 / 762 | 3048 / 1016 | PASS |
| 11 | feed | 6488 | 8651 | NEEDS-RULING (U1) |
| 14 | feed / ramp | 8060 / 6448 | 10747 / 8597 | NEEDS-RULING (U1) |
| 15 | feed / plunge / ramp | 5429 / 2285 / 4571 | 7239 / 2500 / 6095 | PASS |
| 16 | feed | 2317 | 3089 | FAIL (as before) |
| 17 | feed / plunge / ramp | 3165 / 1347 / 3165 | 7161 / 2500 / 7161 | PASS |

Every power row passes. The ratio did not change (idx 1: 1.3132 / 1.5 = 0.875; before
0.9849 / 1.125 = 0.875). Tool 6 plunges stay at 2162 (the design predicted 2500): the repo
material-base rule does not scale with the RPM. The grade does not move (bracket 0-2500).

## P21c fields (`basis.rpm`)

| idx | value | binding | X/Y limit rpm | reached_feed_share |
|---|---|---|---|---|
| 0 | 24000 | spindle_max | 41610 | 1.000 |
| 1 | 24000 | spindle_max | 38574 | null |
| 2 | 24000 | spindle_max | 51143 | null |
| 3 | 19685 | z_axis_rate (19685.04) | 118110 | 1.000 |
| 4 | 19685 | z_axis_rate (19685.04) | 118110 | 1.000 |
| 5 | 24000 | spindle_max | 50271 | null |
| 6 | 24000 | spindle_max | 267168 | 0.9994 |
| 7 | 24000 | spindle_max | 272677 | 0.9977 |
| 8 | 24000 | spindle_max | 272677 | 1.000 |
| 9 | 24000 | spindle_max | 118110 | null |
| 10 | 24000 | spindle_max | 118110 | 1.000 |
| 11 | 24000 | spindle_max | 41610 | 1.000 |
| 12 | 24000 | spindle_max | 41610 | 0.9146 |
| 13 | 24000 | spindle_max | 118110 | 0.9987 |
| 14 | 24000 | spindle_max | 33496 | null |
| 15 | 24000 | spindle_max | 49730 | null |
| 16 | 24000 | spindle_max | 116514 | null |
| 17 | 24000 | spindle_max | 50271 | 0.9985 |

Each row lists `operator_ceiling` 24000. `feed_limit` is `op_feed` on all 16 modelled rows.
The share is null where the toolpath is not generated (status Pending). The lowest share is
idx 12 (0.915). The app X/Y limit uses the app chip; the script uses the top of each chip
band. Both are above 24000 on every side cut.

## FAIL rows (6)

- idx 6, 7, 8 plunge: 240 against 1219-1402 mm/min at 24000 rpm (U2 and U3: the unsourced
  material base; the candidate rises with n, the app value does not).
- idx 16 chip: 0.06437 against SpeTool 0.0762 mm.
- idx 16 feed: 3089 against 3658 (P4a) or 6096 (P4b); P4 is open.
- idx 5 feed: 3165 (stored) against 6096-8534 (see above).

## NEEDS-RULING rows (28), by question

| Question | Rows | Count |
|---|---|---|
| U1: the 18 mm 2F chip (bracket 0.1448-0.2286 mm; app 0.1791) | chip and feed of idx 0, 1, 11, 12, 14; ramp of idx 0, 12, 14 | 13 |
| U3 tool 6: the plunge of a tool that is not centre-cutting (bound 0-2500; app 2162) | plunge of idx 0, 1, 11, 12, 14 | 5 |
| U2: the 2 mm 1F chip (2F bracket 0.0508-0.0584 mm; app 0.0550) | chip and feed of idx 6, 7, 8; ramp of idx 8 | 7 |
| U3 tool 1: the plunge of a 20 deg V-bit (no source; app 381) | plunge of idx 9 | 1 |
| U3 tool 4: the plunge of a 2 mm tapered ball (no source; app 240) | plunge of idx 16 | 1 |
| H1 (new): the feed of a helix entry (no script rule; app 8597) | "ramp feed" of idx 1 | 1 |

A ruling on U2 or U3 for tool 5 also decides the three FAIL plunge rows of idx 6, 7, 8. A
ruling on P4 decides the idx 16 feed row (FAIL under both candidates today, because the app
chip is 0.06437).

## The 7 findings of grade_4b7aa0e96.md

1. **Holds.** idx 2, 15: the ramp (6095) reads a cut feed of 6096 (0.127 x 2 x 24000), not
   the Suggest feed (7039, 7239). idx 0, 1, 12, 14: the card says "cut feed 8597" (8598 on
   idx 14) against feeds 8651, 9332, 8651, 10747. The value agrees with the table (lift 1).
   The plunge of idx 2 and 15 is now 2500 (the Z rate), so the plunge part is gone.
2. **Holds.** idx 10, 13: `ae_mm` and `case_ae_mm` say 6.35 on a `v_groove` section.
3. **Holds.** idx 5, 17: the band ships `onsrud-hardwood-60-200-1_4-finish` (0.149 mm), but
   `members`, `highest` and `lowest` list only `onsrud-hardwood-60-100mw-1_4-finish`.
4. **Changed.** idx 17: Suggest gives plunge 2500; the stored param is 1582. idx 5 Suggest is
   2500 too, so check C2 is closed in Suggest.
5. **Holds.** C1: tool 0 has two target chips, 0.127 (idx 2, 15) and 0.149 (idx 5, 17).
6. **Closed.** idx 3, 4: the drills print 19685 rpm, Z axis rate binding.
7. **Holds.** idx 16 power: `rectangle` section (0.001426 kW), graded as an upper bound.

## App output that no row covers (new)

1. idx 3, 4 `basis.detail` still says "the chart's 18000 RPM ships, clamped only to the machine
   spindle range". The RPM that ships is 19685 (Z axis rate). The same reply's plunge text says
   "the RPM follows a capped feed down to hold the chip". The two texts disagree.
2. idx 5, 9: Suggest prints an RPM (24000) and no feed. The stored feeds were written at
   10610 and 18000 rpm. If an apply writes the RPM alone, the chip drops to 0.066 mm (idx 5)
   and 0.095 mm (idx 9; inside the AMS band 0.076-0.178, so that row passes).
3. idx 1 helix entry: the card says "Helix leaves no core: r 5.40 mm <= 9.00 mm, the flat
   bottom". Tool 6 is not centre-cutting (core 8 mm, protrusion 1.0 mm, T6CC). The card does
   not show a check against the core. Grader arithmetic: the tool sinks 2.0 x 8 / (2 pi x 5.4)
   = 0.47 mm while it moves one core width; that is less than the protrusion 1.0 mm.
4. Rounding: the ramp headline says 6095 and its detail says "cut feed 6096" (idx 2, 15);
   8597 against 8598 on idx 14.

## Table, script and README findings (not edited)

1. README step 4 names `basis.shipped_power.rpm` only. On idx 5 and 9 that field does not
   exist; the RPM is in `basis.rpm.value`. The grader read `basis.rpm.value` there.
2. The script grades a fallback stored feed at the Suggest RPM (idx 5, 9). The feed rule
   assumed that a row with no Suggest feed has no Suggest RPM. This makes the idx 5 feed FAIL.
3. README line 57 still says the project sets `rpm_max = 1000.0`. The file says 24000.0.
4. README step 1 cannot pass for a binary that was built before the commit of its own source.
   A rule on the source tree state at build time is necessary, or the build must follow the
   commit.
5. The header P1 ruling text says "The app fails these rows until P21". P21c is in; the text
   is history now.
