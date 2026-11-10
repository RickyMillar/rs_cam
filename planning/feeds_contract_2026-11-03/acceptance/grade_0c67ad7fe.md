# Grade of build 0c67ad7fe (P23a, P23b), 2026-11-10

A fresh agent graded the build through the read-only MCP tools. No cargo run, no edit under
`crates/`, no `set_*`, apply, generate, simulate, save or load call.

Build: HEAD `0c67ad7fe` (commit time 1794276928, 15:15:28). The GUI process started
2026-11-10 15:38:58 and runs `target/release/rs_cam_gui --mcp`, mtime 1794277254 (15:20:54),
later than HEAD. `git status crates` is empty. The grader did not call `gui_status`.

- Grade: PASS 95, FAIL 3, NEEDS-RULING 0, N/A 10 (108 rows).
- The previous grade (build dce570108 after the six rulings): PASS 92, FAIL 6, NEEDS-RULING 0, N/A 10.
- Status counts: DERIVED 89, SOURCED 6, UNSOURCED 2, POLICY 1, N/A 10 (no change).

## Inputs (README step 3)

- `list_toolpaths`: 18 toolpaths, index 0-17, the names in the table.
- `get_toolpath_params` was read for idx 1 and 14 only: both are `clearing_strategy =
  geodesic_adaptive` (stepover 5.5 and 3.6; idx 1 `entry_style` helix). The project file did
  not change since the read at dce570108 (`git diff dce570108 HEAD` on the file is empty, no
  working-tree edit), so the other stored params were not read again. idx 2 and 15 were
  `geodesic_adaptive` at that read.
- `get_suggest_rationale` was read for all 18.
- The SRC files of the plunge rule and the size law did not change since dce570108.

## Table changes

1. P3 width (operator ruling Q2 of 2026-11-09, recorded in the header): the lift of an
   adaptive row reads the pass ceiling `c D`, computed by `pass_ceiling_width` from the
   planner formula (SRC PASSLIM, PASSTOL, PASSSW, PASSARC). `stepover x 1.6` is deleted. The
   rule covers idx 1, 2, 14 and 15 (all four are geodesic Adaptive3d rows of one kind in the
   script). Note: `radial_woc_fraction_from_leading_arc` and `target_engagement_fraction` are
   in `crates/rs_cam_core/src/ops/adaptive_shared.rs`, not under `adaptive/`.

   | idx | stepover | D | c D (script) | app `case_ae_mm` | lift (script) | app `radial_lift` |
   |---|---|---|---|---|---|---|
   | 1 | 5.5 | 18 | 5.9913 | 5.9913 | 1.0610 | 1.0610 |
   | 2 | 1.5 | 6 | 1.6380 | 1.6380 | 1.1223 | 1.1223 |
   | 14 | 3.6 | 18 | 3.9395 | 3.9395 | 1.2093 | 1.2093 |
   | 15 | 1.382 | 6 | 1.5104 | 1.5104 | 1.1520 | 1.1520 |

2. The fallback to a stored param is deleted. idx 5 and 9 read `basis.rpm.feed_mm_min`,
   `.value`, `.plunge_mm_min`, `.ramp_feed_mm_min`.
3. README: step 1 (a binary built before its commit), step 4 (the `basis.rpm` fields), the
   `rpm_max` note.

## Rows that changed grade

FAIL to PASS, 3 rows:

| idx | Quantity | Expected | App before | App now |
|---|---|---|---|---|
| 1 | feed | 9122 (was 8599 at the 1.6 cap) | 9332 | 9123 |
| 14 | feed | 10396 (was 9215 at the 1.6 cap) | 10747 | 10397 |
| 5 | feed | 6096-8534 | 3165 (stored) | 7161 (`basis.rpm.feed_mm_min`) |

idx 1 and 14 pass by both changes together: the expected value moved with the ruling, and
the app moved with P23a. Against the old 1.6 cap rule the new app values would fail (9123
against 8599; 10397 against 9215).

Same grade, new value: idx 2 feed 7039 to 6841 (band 6842-9578; the app is at the low edge,
because it ships the Spektra chip 0.127, the low end of the Onsrud band); idx 15 feed 7239
to 7022 (band 7023-9832); idx 9 feed 2286 (stored) to 3048 (band 1829-4267); idx 1 entry
feed 8597 to 8598; power of idx 1 (0.9400 kW, ratio 0.627), 2, 14, 15 (the app evaluates the
power at the ceiling width; the model agrees).

## FAIL rows (3)

- idx 6 feed: expected 1320, app 1347 (+2.0 %). `radial_lift` 1.0206 = the lift at ae 0.8 on
  D 2, in a pocket whose first ring is a slot (check C3). The power case of the same reply
  is `slot`, `case_ae_mm` 2.0: the feed and the power case read two widths.
- idx 16 chip: expected 0.0762 (SpeTool), app 0.06437.
- idx 16 feed: expected 3658 (P4a) or 6096 (P4b), app 3089. P4 is open.

## NEEDS-RULING rows

None.

## Findings (not edited)

1. idx 2 and 15 pass the feed row by 1 mm/min at the band's low edge after rounding (6841
   against 6842; 7022 against 7023; inside the 1 % tolerance). Check C1 holds: tool 0 ships
   chip 0.127 on idx 2 and 15 and 0.149 on idx 5 and 17.
2. `trochoid_cap_mult` 1.6 is still a param of the four adaptive toolpaths
   (`get_toolpath_params`); the ruling retires it.
3. idx 3, 4 `basis.detail` still says "the chart's 18000 RPM ships"; 19685 ships.
4. Rounding: idx 1 ramp headline 8598; idx 14 headline 8597 with detail "cut feed 8598".
5. The findings of `grade_dce570108.md` on the vendor-band member list (idx 5, 17), the
   `v_groove` width label (idx 10, 13) and the helix core check (idx 1) hold.
