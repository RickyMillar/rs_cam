# Grade of build b9e67b24e (P20e), 2026-11-09

A fresh agent graded the build. The lead checked the binary: the GUI process started
2026-11-09 09:38 runs `target/release/rs_cam_gui`, mtime 1794168764 (09:12), later than
HEAD `b9e67b24e` (commit time 1794168411, 09:06). No edits under `crates/`.

- Grade: PASS 47, FAIL 23, NEEDS-RULING 28, N/A 10 (108 rows).
- The previous grade (build 4b7aa0e96, P20b-P20d): PASS 47, FAIL 23, NEEDS-RULING 28, N/A 10.
- Status counts: DERIVED 58, UNSOURCED 33, N/A 10, SOURCED 6, POLICY 1 (no change).

## Inputs (README step 3)

- `list_toolpaths`: 18 toolpaths, index 0-17, the names in the table.
- Every input that the script reads is the same as at 4b7aa0e96: stepover, `depth_per_pass`,
  `trochoid_cap_mult` (1.6), `entry_style` and the tools. The TP and TOOLS dicts did not change.
- idx 5 (River channel): the selection is now `layers = ["VGROOVE_V20_RIVER"]`, kinds served.
  The table reads no input from the selection (ae = D, a slot; ap is not set). Suggest still
  gives `shipped_power: {"unmodeled": "DepthAfterSimulation"}`. No row moves.
- `feed_max_rate` is gone from the project. No param or rationale field shows it.
- The project file changed only after line 705. The SRC lines MACH (59-65, 84-88) and T6CC
  (383-386) did not move. The other SRC files did not change since `e323b3098`.

## Rows that the build moved

None. Every APP value (`sug` and `shp`) is equal to the APP of 4b7aa0e96
(`git show e323b3098:planning/feeds_contract_2026-11-03/acceptance/derive_wanaka_21.py`).
The script changed only in the docstring build line, the APP comment and `graded_build`.

P20e cannot move a row in this project: no cap is set, and every feed is below the XY rate
15000 and every vertical rate is below the Z rate 2500. Thus the steady feed is the op feed.

## P20e fields (`basis.shipped_power`)

| idx | feed_mm_min | op_feed_mm_min | feed_limit | toolpath `feed_rate` |
|---|---|---|---|---|
| 0 | 6488 | 6488 | op_feed | 6488 |
| 1 | 6999 | 6999 | op_feed | 8475 |
| 2 | 5279 | 5279 | op_feed | 5279 |
| 3 | 2286 | 2286 | op_feed | 1000 |
| 4 | 2286 | 2286 | op_feed | 1000 |
| 5 | - (unmodeled) | - | - | 3165 |
| 6 | 1010 | 1010 | op_feed | 1010 |
| 7 | 990 | 990 | op_feed | 1258 |
| 8 | 990 | 990 | op_feed | 1258 |
| 9 | - (unmodeled) | - | - | 2286 |
| 10 | 2286 | 2286 | op_feed | 2286 |
| 11 | 6488 | 6488 | op_feed | 6488 |
| 12 | 6488 | 6488 | op_feed | 6488 |
| 13 | 2286 | 2286 | op_feed | 2286 |
| 14 | 8060 | 8060 | op_feed | 8060 |
| 15 | 5429 | 5429 | op_feed | 5429 |
| 16 | 2317 | 2317 | op_feed | 2317 |
| 17 | 3165 | 3165 | op_feed | 3165 |

`feed_limit` is `op_feed` on all 16 modelled rows, as expected with no cap. No machine
ceiling binds.

## FAIL rows (same 23 as 4b7aa0e96)

- rpm, 16 rows (idx 0-17 except 5 and 17 at 18000; idx 5 and 17 at 10610): P1 (b). Wait on P21.
- idx 6, 7, 8 plunge: 240 against 889-1016 mm/min (U3: the unsourced material base).
- idx 16 chip: 0.06437 against SpeTool 0.0762 mm.
- idx 16 feed: 2317 against 2743 (P4a) or 4572 (P4b); P4 is open.

## The 7 findings of grade_4b7aa0e96.md

1. **Holds.** idx 2, 15: the plunge (2285) and the ramp (4571) read a cut feed of 4572
   (0.127 x 2 x 18000), not the shipped feed (5279, 5429). The same label occurs on idx 0, 1,
   12, 14: the ramp card says "cut feed 6448 mm/min", but the feed is 6488, 6999, 6488, 8060.
   6448 = 0.17913 x 2 x 18000, the slot feed. A ramp cuts a slot, so the value agrees with
   the table (lift 1). The text "the cut feed" does not agree with the feed that ships.
2. **Holds.** idx 10, 13: `ae_mm` and `case_ae_mm` say 6.35 on a `v_groove` section. The kW
   is correct.
3. **Holds.** idx 5, 17: the vendor band ships `onsrud-hardwood-60-200-1_4-finish` (0.149 mm),
   but `members`, `highest` and `lowest` list only `onsrud-hardwood-60-100mw-1_4-finish`.
4. **Holds.** idx 17: the param plunge is 1582; Suggest gives 1347.
5. **Holds.** C1: tool 0 has two target chips, 0.127 (idx 2, 15) and 0.149 (idx 5, 17).
6. **Holds.** idx 3, 4: the drills ship 18000 rpm; only the rpm row fails (P1, P21).
7. **Holds.** idx 16 power: `rectangle` section (0.001069 kW), graded as an upper bound.

## App output that no row covers (new)

1. `op_feed_mm_min` is the Suggest feed, not the toolpath `feed_rate`, where the two differ:
   idx 1 (6999 against 8475), idx 3 and 4 (2286 against 1000), idx 7 and 8 (990 against
   1258). P20e names this value "op feed". If the cap, the gates and the card must act on the
   feed that the toolpath ships, these five rows show a different feed. If "op feed" means
   the Suggest feed, the name does not say so. The lead must decide which one P20e intends.
   No row grades this field.

## Lead note on the new finding (2026-11-09)

`shipped_power` is the power at the cut that Suggest ships (`FeedsProfile::shipped_operation`,
the operation with Suggest's speeds; `ui/feeds/CLAUDE.md`: "Power and the verdict read the cut
Apply writes"). So `op_feed_mm_min` is the Suggest feed by design. The gates read the toolpath's
own feed (`session/compute/toolpath_diagnostics.rs`). Not a defect; the field name misleads.
Open: rename or document `op_feed_mm_min` as "the op feed of the shipped cut".
