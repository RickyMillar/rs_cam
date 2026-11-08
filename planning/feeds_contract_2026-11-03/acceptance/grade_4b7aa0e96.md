# Grade of build 4b7aa0e96 (P20b-P20d), 2026-11-08

A fresh agent graded the build. The lead re-ran the script and got the same counts.

- Grade: PASS 47, FAIL 23, NEEDS-RULING 28, N/A 10 (108 rows).
- The previous grade (build 998ac93a3, before the rulings of 2026-11-08): PASS 31, FAIL 12,
  NEEDS-RULING 55.
- The rulings alone (old app values) give PASS 42, FAIL 28, NEEDS-RULING 28.

## Rows that the build moved (all FAIL to PASS)

| idx | Row | Phase | Why |
|---|---|---|---|
| 3, 4 | power | P20d | `advisory: true`, `section: axial_circle` (0.141 kW; the grader checked the shear and edge terms by hand) |
| 10, 13 | power | P20d | `v_groove` section: 0.02822 and 0.00150 kW, the table's triangle within 1 % |
| 17 | plunge (Suggest) | P20b | 1347 = 0.0635 x 10610 x 2, the same as idx 5 (check C2 resolved in Suggest) |

No row shows a P20c effect: every feed is below the XY rate 15000 and every vertical rate is
below the Z rate 2500.

## FAIL rows

- rpm, 16 rows (idx 0-17 except 5 and 17 at 18000; idx 5 and 17 at 10610): P1 (b). Wait on P21.
- idx 6, 7, 8 plunge: 240 against 889-1016 mm/min (U3: the unsourced material base).
- idx 16 chip: 0.06437 against SpeTool 0.0762 mm.
- idx 16 feed: 2317 against 2743 (P4a) or 4572 (P4b); P4 is open.

## App output that no row covers (grader section 4)

1. idx 2, 15: the plunge (2285) and the ramp read a cut feed of 4572 mm/min (0.127 x 2 x 18000),
   not the shipped feed (5279, 5429). The card says "re-derived at the feed that ships". The
   text and the value disagree.
2. idx 10, 13: `shipped_power.ae_mm` and `case_ae_mm` say 6.35 on a `v_groove` section. The
   groove width is 2 x ap x tan 30 = 3.0 mm (ap 2.6) and 0.69 mm (ap 0.6). The kW is correct.
3. idx 5, 17: the vendor band ships `onsrud-hardwood-60-200-1_4-finish` (0.149 mm), but
   `members`, `highest` and `lowest` list only `onsrud-hardwood-60-100mw-1_4-finish` (0.37 mm).
4. idx 17: the toolpath param plunge is still 1582; Suggest gives 1347. The operator has not
   re-applied Suggest.
5. Check C1: tool 0 still has two target chips, 0.127 (idx 2, 15) and 0.149 (idx 5, 17). Both
   are inside the Onsrud range (P2 b), so no row fails. One tool in one material still has two
   chips.
6. idx 3, 4: the drills ship 18000 rpm because the feed 2286 is below the Z cap; only the rpm
   row fails (P1, P21).
7. idx 16 power: still a `rectangle` section (0.001069 kW), graded as an upper bound (PASS).
