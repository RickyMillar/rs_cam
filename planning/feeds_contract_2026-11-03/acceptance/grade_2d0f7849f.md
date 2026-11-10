# Grade of build 2d0f7849f (P23c), 2026-11-10

A fresh agent graded the build through the read-only MCP tools. No cargo run, no edit under
`crates/`, no `set_*`, apply, generate, simulate, save or load call.

Build: HEAD `2d0f7849f` (commit time 1794280255, 16:10:55). The GUI process started
2026-11-10 16:17:01 and runs `target/release/rs_cam_gui --mcp`, mtime 1794280585 (16:16:25),
later than HEAD. `git status crates` is empty. The grader did not call `gui_status`.

- Grade: PASS 97, FAIL 1, NEEDS-RULING 0, N/A 10 (108 rows).
- The previous grade (build 0c67ad7fe, P23a and P23b): PASS 95, FAIL 3, NEEDS-RULING 0, N/A 10.
- Status counts: DERIVED 90, SOURCED 6, UNSOURCED 2, N/A 10 (before: DERIVED 89, POLICY 1). No
  POLICY row is left. The 2 UNSOURCED rows are the unmodeled power rows of idx 5 and 9.

## Inputs

- `list_toolpaths`: 18 toolpaths, index 0-17, the names in the table.
- `get_suggest_rationale` was read for all 18. `get_toolpath_params` was not read again: the
  project file did not change since `dce570108`, and the stored params are information only.
- SRC: `vendor_lookup.rs` changed in P23c; `CHIPLOAD_DIAMETER_EXPONENT` moved from line 791
  to line 802 (SIZEC quotes the new line; the value 0.61 did not change). No other SRC file
  changed since `0c67ad7fe`.

## Table changes (operator rulings 2026-11-10, recorded in the header)

1. Tapered ball chip: the row printed at exactly the tool size wins ("1, yes"). The expected
   chip (SpeTool SPE23, 0.0762 mm) did not change; the ruling confirms it.
2. P4 (a): no ball lead-angle lift ("finishing feeds are already pretty high generally"). The
   P4b candidate is deleted. The idx 16 feed row is DERIVED: 0.0762 x 2 x 24000 = 3658.

## Rows that changed grade

FAIL to PASS, 2 rows:

| idx | Quantity | Expected | App before | App now |
|---|---|---|---|---|
| 16 | chip per tooth | 0.0762 | 0.06437 | 0.0762 |
| 16 | feed | 3658 (was 3658 or 6096) | 3089 | 3657 |

The app band on idx 16 now has two members: the SpeTool 2.0 mm row (`size_basis: Exact`) and
the Amana ZrN claim (0.0644 mid). `rule` is `exact_size`; the SpeTool row ships.

Same grade, new value: idx 16 power 0.001426 to 0.001522 kW (inside the bound).

## Rows that moved unexpectedly

None. For idx 0-15 and 17 every graded field (chip, rpm, feed, plunge, ramp feed, required
kW, available kW, case width) is equal to the value read at `0c67ad7fe`.

## FAIL rows (1)

- idx 6 feed: expected 1320, app 1347 (+2.0 %). `radial_lift` 1.0206 is the lift at ae 0.8
  on D 2, in a pocket whose first ring is a slot (check C3). The power case of the same reply
  is `slot`, `case_ae_mm` 2.0. idx 7 and 8 (stepover 1.0 = D/2, lift 1) pass.

## NEEDS-RULING rows

None.

## Findings that still hold (not edited)

1. C1: tool 0 ships chip 0.127 on idx 2 and 15 (band rule `lowest`, the exact-size Amana
   row) and 0.149 on idx 5 and 17 (`winner_ships`, a size claim). idx 2 and 15 feeds are
   1 mm/min under the Onsrud band's low edge (inside the tolerance).
2. idx 5, 17: the band member list omits the row that ships.
3. idx 3, 4 `basis.detail` says "the chart's 18000 RPM ships"; 19685 ships.
4. idx 10, 13: `ae_mm` 6.35 on a `v_groove` section. idx 1: the helix card shows no check
   against the 8 mm non-cutting core.
5. `trochoid_cap_mult` 1.6 is still a param on the four adaptive toolpaths (read at
   `0c67ad7fe`).
