# fix18: can a descent be a low travel over cut ground? (C42, 2026-10-26)

Python only, on committed run artefacts: the C35 spiral_pick runs (fix12) and, by the operator's ruling
that the keep-down link is a shared component, the contour_parallel runs (fix6). No code edit, no cargo,
no commit. `lowtravel.py` does the work (its docstring gives each rule), `make_readme.py` writes this
file and `quotes.json`, and `assert_fix18.py` runs `lowtravel.py` again from the raw files and checks
every number here, plus independent checks of the selection, the frames, the stock rebuild and the paths.
Run: `python3 -I lowtravel.py && python3 -I make_readme.py && python3 -I assert_fix18.py` (about 5 min).

## Answer

**spiral_pick: the descents are not travels, they are side steps.** The lift point and the plunge point
are almost at one XY: the median straight distance is 0.57 mm (Corne) and 1.22 mm
(wanaka200); the median drop is 5.16 and 4.16 mm. The plunge point is in stock by the
selection, so no path over cut ground reaches it; a path reaches the free point nearest to it, and a short
side leg at the level does the rest.

- Strictly over cut ground to the plunge point: 20 of 1076 (Corne) and 2 of 2016 (wanaka200).
- With a side leg at the end: 781 (72.6 %) and 1393 (69.1 %);
  median side leg 0.58 and 1.19 mm. Side leg within one stepover (an ordinary cut):
  734 (68.2 % of all) and 1189 (59.0 %); and its axial depth
  (drop − 0.5 mm) within the depth per pass: 691 (64.2 %) and 1116 (55.4 %).
- No path: the lift floor is above the plunge level + 0.5 mm in 255 and 449; the lift
  point itself has stock over its disc in 40 and 174.
- Time of the descents with a path: 435.6 s and 1570.6 s now (their whole spans;
  plunge feed alone 326.0 s and 909.8 s). The low travels: 168.2 / 514.5 s at the
  link feed (500 mm/min), 55.4 / 257.2 s at the cut feed. Taking the low travel only where it is
  faster (at the cut feed) saves 380.2 s of 1451.7 s and 1313.4 s of 6025.9 s (sample clock).

**C42b, the side leg on the swept disc** (section C42b): the disc swept along the leg meets stock within
depth per pass + 0.5 mm axially and within one stepover radially for
756 of 1076 (70.3 %) on Corne and 1366 of 2016 (67.8 %) on wanaka200.
Axially every Corne leg passes (781 of 781, max 6.11 mm); the radial test removes
the rest. Median ae 0.51 and 0.99 mm (0.42 and 0.50 of the stepover). On those
entries the low travel at the cut feed saves 364.2 s and 1253.6 s. On contour_parallel
the same test passes 52 of 95 and 547 of 1609 entries, with 8.1 s and 273.2 s saved at the cut feed.

**contour_parallel: the shared link would save little, at feed.** Its entries are long hops: the median
straight distance is 37.3 mm (Corne) and 64.0 mm (wanaka200). A path over cut ground
exists for 75 of 95 entries (78.9 %) and 1138 of 1609 (70.7 %),
median 51.0 and 67.8 mm. Those entries take 126.9 s and 2179.2 s now (mostly rapids). The same
travels take 582.4 / 12502.2 s at the link feed, 191.7 / 6251.1 s at the cut feed and 72.8 / 625.1 s at the rapid rate.
Per entry, the low travel is faster than the entry now for:

| Column | Entries with a path | Faster at the link feed (saved s) | at the cut feed (saved s) | at the rapid rate (saved s) | Op time s |
|---|---|---|---|---|---|
| corne spiral_pick | 781 | 711 (294.0) | 781 (380.2) | 781 (414.6) | 1451.7 |
| wanaka200 spiral_pick | 1393 | 1382 (1062.9) | 1393 (1313.4) | 1393 (1544.9) | 6025.9 |
| corne contour_parallel | 75 | 1 (0.2) | 28 (14.2) | 70 (55.2) | 973.2 |
| wanaka200 contour_parallel | 1138 | 255 (217.4) | 449 (499.0) | 1138 (1554.1) | 6645.8 |

So on contour_parallel a keep-down link at feed pays only on the short hops; a low link at the rapid
rate pays more. The rows by kind are below.

## Method

- Selection. spiral_pick: `fix12/entries_fix12.py`'s rule, keep-down-link spans whose entry_plunge samples
  descend 1 mm or more and remove more than 0.1 mm³ (1076 and 2016).
  contour_parallel: every entry span of op 1 (both styles); 'into stock' marks the same rule, 'with a rapid'
  a span that holds a G0 (a retract).
- Lift point: the tool position at the start of the span, from `program.nc` (move i = command i + 1 of the op
  section, as `fix11/audit.py` maps it). Frames: Corne sim = G-code + (stock box corner XY, 23); wanaka200
  sim = G-code + (0, 0, 25).
- Plunge point: the last entry_plunge sample of the span (no such sample: the span's own entry point). Level Z:
  the first later sample that is not entry_plunge or retract (no plunge sample: the span's entry Z). Drop: the
  highest entry_plunge sample Z less the level Z. Level plane: the highest plane at or below the level Z + 0.35.
- Stock top at the entry: `fix9/stockmap.py`'s rebuild (flat end mill, R 3, path densified at g/2) over the
  samples before the span, stamped in order. Grid: Corne 0.25 mm, wanaka200 0.5 mm.
- Free centre cell: the highest stock over its disc of R ≤ level Z + 0.5 mm (stay_down_clearance_mm). Dijkstra
  over free cells, 16 neighbours (knight steps need their two crossed cells free), from a virtual source joined to
  the free cells within 2 g of the lift point at their true distance, in a window of the two points + 24 mm.
  The path ends at the reachable free cell nearest the plunge point; travel = free path + side leg. 16-neighbour
  grid paths are at most 2.7 % longer than the true shortest path.
- Times on the samples' clock (no acceleration), as `fix11/audit.py` reads them: plunge feed = the span's fed,
  vertical, downward moves; span = all samples of the span. Low travel = travel over 500 mm/min (link_feed_rate),
  over the cut feed (the adaptive3d keep-down feed, path.rs:1576), or over max_feed_mm_min (4000 / 10000).
  'Saved' sums span − low travel over the entries where that is positive.
- C40 cause (spiral_pick only): the nearest unused op-1 'C40 entry' line of the D25 build (fix15) within 0.5 mm
  XY and 1.0 mm Z, as `fix15/c40_join.py`; pools 1188 and 2238. A different build: a guide only.

## corne spiral_pick, '3D Rough 1'

| Group | Entries | With a path (%) | Side leg ≤ stepover | and ap ≤ depth per pass | Lift above level | Lift not free | Straight median mm | Travel median mm | Travel < 1 / 2 / 4 / 8 D (% of paths) | Plunge feed now s | Span now s | Low travel at 500 / cut feed / rapid s | Saved at cut feed / rapid s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| all | 1076 | 781 (72.6) | 734 | 691 | 255 | 40 | 0.57 | 1.11 | 98.6 / 99.0 / 99.1 / 100.0 | 326.0 | 435.6 | 168.2 / 55.4 / 21.0 | 380.2 / 414.6 |
| Z 17 | 665 | 488 (73.4) | 466 | 466 | 137 | 40 | 0.57 | 1.09 | 99.0 / 99.6 / 99.8 / 100.0 | 221.0 | 282.9 | 81.0 / 26.7 / 10.1 | 256.2 / 272.8 |
| Z 11 | 36 | 36 (100.0) | 27 | 24 | 0 | 0 | 0.54 | 1.27 | 94.4 / 94.4 / 94.4 / 100.0 | 20.8 | 29.4 | 16.9 / 5.6 / 2.1 | 23.9 / 27.3 |
| Z 5 | 36 | 36 (100.0) | 27 | 8 | 0 | 0 | 0.54 | 1.27 | 94.4 / 94.4 / 94.4 / 100.0 | 32.7 | 43.6 | 16.9 / 5.6 / 2.1 | 38.0 / 41.5 |
| Z 0.5 | 339 | 221 (65.2) | 214 | 193 | 118 | 0 | 0.62 | 1.25 | 99.1 / 99.1 / 99.1 / 100.0 | 51.5 | 79.7 | 53.4 / 17.6 / 6.7 | 62.2 / 73.1 |
| C40 Some(LargeZ) | 423 | 278 (65.7) | 275 | 275 | 132 | 13 | 0.58 | 1.15 | 100.0 / 100.0 / 100.0 / 100.0 | 64.5 | 93.0 | 49.0 / 16.1 / 6.1 | 76.9 / 86.9 |
| C40 Some(LargeZHeld) | 3 | 1 (33.3) | 1 | 1 | 1 | 1 | 0.68 | 1.08 | 100.0 / 100.0 / 100.0 / 100.0 | 0.5 | 0.7 | 0.1 / 0.0 / 0.0 | 0.6 / 0.6 |
| C40 Some(Reentry2d) | 110 | 107 (97.3) | 82 | 40 | 1 | 2 | 0.54 | 1.27 | 96.3 / 97.2 / 97.2 / 100.0 | 82.1 | 108.2 | 39.7 / 13.1 / 5.0 | 95.1 / 103.3 |
| C40 unmatched | 540 | 395 (73.1) | 376 | 375 | 121 | 24 | 0.57 | 1.10 | 98.2 / 98.7 / 99.0 / 100.0 | 178.9 | 233.7 | 79.4 / 26.1 / 9.9 | 207.6 / 223.8 |

## wanaka200 spiral_pick, '2 Back Rough'

| Group | Entries | With a path (%) | Side leg ≤ stepover | and ap ≤ depth per pass | Lift above level | Lift not free | Straight median mm | Travel median mm | Travel < 1 / 2 / 4 / 8 D (% of paths) | Plunge feed now s | Span now s | Low travel at 500 / cut feed / rapid s | Saved at cut feed / rapid s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| all | 2016 | 1393 (69.1) | 1189 | 1116 | 449 | 174 | 1.22 | 2.16 | 92.0 / 98.6 / 99.0 / 100.0 | 909.8 | 1570.6 | 514.5 / 257.2 / 25.7 | 1313.4 / 1544.9 |
| Z 19.54 | 80 | 80 (100.0) | 14 | 14 | 0 | 0 | 3.61 | 5.54 | 60.0 / 90.0 / 92.5 / 100.0 | 57.3 | 115.6 | 75.0 / 37.5 / 3.7 | 78.1 / 111.9 |
| Z 14.08 | 80 | 80 (100.0) | 14 | 0 | 0 | 0 | 3.61 | 5.54 | 60.0 / 90.0 / 92.5 / 100.0 | 110.2 | 195.0 | 75.0 / 37.5 / 3.7 | 157.5 / 191.2 |
| Z 8.62 | 1856 | 1233 (66.4) | 1161 | 1102 | 449 | 174 | 1.17 | 1.92 | 96.1 / 99.8 / 99.8 / 100.0 | 742.3 | 1260.0 | 364.5 / 182.3 / 18.2 | 1077.7 / 1241.8 |
| C40 None | 190 | 180 (94.7) | 36 | 14 | 5 | 5 | 3.68 | 5.58 | 60.6 / 90.6 / 92.8 / 100.0 | 205.3 | 373.8 | 166.5 / 83.3 / 8.3 | 290.6 / 365.5 |
| C40 Some(LargeZ) | 1824 | 1212 (66.4) | 1152 | 1102 | 444 | 168 | 1.16 | 1.91 | 96.6 / 99.8 / 99.9 / 100.0 | 702.6 | 1193.9 | 347.8 / 173.9 / 17.4 | 1020.0 / 1176.5 |
| C40 Some(LargeZHeld) | 2 | 1 (50.0) | 1 | 0 | 0 | 1 | 0.56 | 1.14 | 100.0 / 100.0 / 100.0 / 100.0 | 1.9 | 2.9 | 0.1 / 0.1 / 0.0 | 2.8 / 2.9 |

## corne contour_parallel, '3D Rough 1'

| Group | Entries | With a path (%) | Side leg ≤ stepover | and ap ≤ depth per pass | Lift above level | Lift not free | Straight median mm | Travel median mm | Travel < 1 / 2 / 4 / 8 D (% of paths) | Plunge feed now s | Span now s | Low travel at 500 / cut feed / rapid s | Saved at cut feed / rapid s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| all | 95 | 75 (78.9) | 52 | 52 | 20 | 0 | 37.30 | 51.04 | 0.0 / 0.0 / 25.3 / 44.0 | 15.5 | 126.9 | 582.4 / 191.7 / 72.8 | 14.2 / 55.2 |
| into stock | 40 | 22 (55.0) | 6 | 6 | 18 | 0 | 23.30 | 37.50 | 0.0 / 0.0 / 22.7 / 54.5 | 8.6 | 34.9 | 160.6 / 52.9 / 20.1 | 4.3 / 15.2 |
| not into stock | 55 | 53 (96.4) | 46 | 46 | 2 | 0 | 51.50 | 53.47 | 0.0 / 0.0 / 26.4 / 39.6 | 6.9 | 92.0 | 421.7 / 138.8 / 52.7 | 10.0 / 39.9 |
| with a rapid | 79 | 62 (78.5) | 42 | 42 | 17 | 0 | 48.99 | 63.06 | 0.0 / 0.0 / 22.6 / 33.9 | 9.9 | 108.1 | 543.0 / 178.8 / 67.9 | 7.2 / 41.3 |
| no rapid (keep-down) | 16 | 13 (81.2) | 10 | 10 | 3 | 0 | 18.00 | 25.79 | 0.0 / 0.0 / 38.5 / 92.3 | 5.6 | 18.8 | 39.3 / 13.0 / 4.9 | 7.1 / 13.8 |
| Z 17 | 9 | 7 (77.8) | 2 | 2 | 2 | 0 | 61.04 | 61.12 | 0.0 / 0.0 / 14.3 / 14.3 | 1.3 | 9.7 | 50.8 / 16.7 / 6.3 | 0.0 / 3.4 |
| Z 11 | 17 | 15 (88.2) | 10 | 10 | 2 | 0 | 51.50 | 70.12 | 0.0 / 0.0 / 20.0 / 26.7 | 3.4 | 27.0 | 153.5 / 50.5 / 19.2 | 2.1 / 8.5 |
| Z 5 | 17 | 15 (88.2) | 10 | 10 | 2 | 0 | 51.50 | 70.12 | 0.0 / 0.0 / 20.0 / 26.7 | 4.9 | 30.9 | 153.5 / 50.5 / 19.2 | 4.0 / 12.0 |
| Z 0.5 | 52 | 38 (73.1) | 30 | 30 | 14 | 0 | 28.36 | 33.06 | 0.0 / 0.0 / 31.6 / 63.2 | 5.8 | 59.2 | 224.6 / 73.9 / 28.1 | 8.0 / 31.2 |

## wanaka200 contour_parallel, '2 Back Rough'

| Group | Entries | With a path (%) | Side leg ≤ stepover | and ap ≤ depth per pass | Lift above level | Lift not free | Straight median mm | Travel median mm | Travel < 1 / 2 / 4 / 8 D (% of paths) | Plunge feed now s | Span now s | Low travel at 500 / cut feed / rapid s | Saved at cut feed / rapid s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| all | 1609 | 1138 (70.7) | 601 | 455 | 425 | 46 | 64.00 | 67.76 | 12.0 / 16.4 / 26.3 / 40.6 | 704.1 | 2179.2 | 12502.2 / 6251.1 / 625.1 | 499.0 / 1554.1 |
| into stock | 380 | 259 (68.2) | 133 | 10 | 102 | 19 | 100.25 | 107.54 | 10.4 / 17.0 / 23.9 / 32.4 | 426.0 | 755.4 | 3397.1 / 1698.5 / 169.9 | 192.9 / 585.6 |
| not into stock | 1229 | 879 (71.5) | 468 | 445 | 323 | 27 | 60.00 | 61.16 | 12.4 / 16.3 / 27.0 / 43.0 | 278.1 | 1423.8 | 9105.1 / 4552.6 / 455.3 | 306.0 / 968.5 |
| with a rapid | 1057 | 708 (67.0) | 275 | 129 | 311 | 38 | 118.24 | 136.58 | 3.2 / 5.5 / 8.1 / 10.0 | 531.5 | 1327.5 | 11325.2 / 5662.6 / 566.3 | 188.1 / 761.3 |
| no rapid (keep-down) | 552 | 430 (77.9) | 326 | 326 | 114 | 8 | 20.12 | 19.75 | 26.3 / 34.4 / 56.3 / 90.9 | 172.7 | 851.7 | 1176.9 / 588.5 / 58.8 | 310.9 / 792.9 |
| Z 19.54 | 6 | 1 (16.7) | 0 | 0 | 1 | 4 | 193.77 | 188.38 | 0.0 / 0.0 / 0.0 / 0.0 | 0.7 | 1.9 | 22.6 / 11.3 / 1.1 | 0.0 / 0.8 |
| Z 14.08 | 6 | 1 (16.7) | 0 | 0 | 1 | 4 | 193.77 | 188.38 | 0.0 / 0.0 / 0.0 / 0.0 | 0.7 | 2.0 | 22.6 / 11.3 / 1.1 | 0.0 / 0.9 |
| Z 8.62 | 1511 | 1073 (71.0) | 552 | 407 | 401 | 37 | 68.50 | 73.14 | 8.4 / 12.9 / 23.0 / 37.8 | 692.6 | 2141.8 | 12364.2 / 6182.1 / 618.2 | 485.9 / 1523.6 |
| Z 4 | 86 | 63 (73.3) | 49 | 48 | 22 | 1 | 3.65 | 1.14 | 73.0 / 77.8 / 82.5 / 88.9 | 10.1 | 33.5 | 92.8 / 46.4 / 4.6 | 13.0 / 28.8 |

Plunge feed, span and low travel columns sum over the entries with a path.

## C42b: the side leg on the swept disc

The side-leg test above reads only the leg's length. Here the tool disc is swept along each side leg (path end
→ plunge point) and sampled as `corridor_ceiling_z` samples a corridor (`adaptive3d/clearing.rs:872-888`):
n = ceil(length / (g / 2)) steps, both ends included, the highest stock top over the disc of R at each step
(cells whose centre is within R; the Rust reads a conservative top per cell, so it can read a little higher).

- Axial depth ap = that highest top − the level Z. Within limit: ap ≤ depth per pass + 0.5 mm (Corne 6.5,
  wanaka200 5.96).
- Radial engagement ae = R − the least distance from a step to the centre of a cell whose stock stands above
  the level Z + 0.5 mm (at least 0): how far the swept disc reaches into that stock. Within limit: ae ≤ the stepover
  (Corne 1.2, wanaka200 2.0 mm).

| Column | Group | Entries | With a path | ap within | ae within | Both (% of entries) | ap median / max mm | ae median mm | ae / stepover median | Span now of 'both' s | Saved at cut feed on 'both' s |
|---|---|---|---|---|---|---|---|---|---|---|---|
| corne spiral_pick | all | 1076 | 781 | 781 | 756 | 756 (70.3) | 4.50 / 6.11 | 0.51 | 0.42 | 409.4 | 364.2 |
| corne spiral_pick | Z 17 | 665 | 488 | 488 | 479 | 479 (72.0) | 4.81 / 6.00 | 0.47 | 0.39 | 274.4 | 250.6 |
| corne spiral_pick | Z 11 | 36 | 36 | 36 | 31 | 31 (86.1) | 6.00 / 6.00 | 0.54 | 0.45 | 23.7 | 20.6 |
| corne spiral_pick | Z 5 | 36 | 36 | 36 | 31 | 31 (86.1) | 6.00 / 6.11 | 0.54 | 0.45 | 37.3 | 34.1 |
| corne spiral_pick | Z 0.5 | 339 | 221 | 221 | 215 | 215 (63.4) | 1.00 / 1.15 | 0.51 | 0.43 | 74.0 | 59.0 |
| corne spiral_pick | C40 Some(LargeZ) | 423 | 278 | 278 | 278 | 278 (65.7) | 1.00 / 6.00 | 0.49 | 0.41 | 93.0 | 76.9 |
| corne spiral_pick | C40 Some(LargeZHeld) | 3 | 1 | 1 | 1 | 1 (33.3) | 5.74 / 5.74 | 0.54 | 0.45 | 0.7 | 0.6 |
| corne spiral_pick | C40 Some(Reentry2d) | 110 | 107 | 107 | 91 | 91 (82.7) | 6.00 / 6.11 | 0.54 | 0.45 | 90.5 | 84.8 |
| corne spiral_pick | C40 unmatched | 540 | 395 | 395 | 386 | 386 (71.5) | 4.72 / 6.00 | 0.47 | 0.39 | 225.2 | 202.0 |
| wanaka200 spiral_pick | all | 2016 | 1393 | 1384 | 1370 | 1366 (67.8) | 3.96 / 15.63 | 0.99 | 0.50 | 1503.1 | 1253.6 |
| wanaka200 spiral_pick | Z 19.54 | 80 | 80 | 80 | 79 | 79 (98.8) | 5.46 / 5.46 | 1.93 | 0.97 | 114.4 | 77.1 |
| wanaka200 spiral_pick | Z 14.08 | 80 | 80 | 80 | 79 | 79 (98.8) | 5.96 / 5.96 | 1.93 | 0.97 | 192.7 | 155.5 |
| wanaka200 spiral_pick | Z 8.62 | 1856 | 1233 | 1224 | 1212 | 1208 (65.1) | 3.81 / 15.63 | 0.91 | 0.45 | 1196.1 | 1021.0 |
| wanaka200 spiral_pick | C40 None | 190 | 180 | 180 | 178 | 178 (93.7) | 5.46 / 5.96 | 1.92 | 0.96 | 370.3 | 287.6 |
| wanaka200 spiral_pick | C40 Some(LargeZ) | 1824 | 1212 | 1203 | 1191 | 1187 (65.1) | 3.78 / 15.63 | 0.90 | 0.45 | 1130.0 | 963.2 |
| wanaka200 spiral_pick | C40 Some(LargeZHeld) | 2 | 1 | 1 | 1 | 1 (50.0) | 4.79 / 4.79 | 0.13 | 0.07 | 2.9 | 2.8 |
| corne contour_parallel | all | 95 | 75 | 62 | 52 | 52 (54.7) | 0.15 / 14.50 | 0.00 | 0.00 | 89.3 | 8.1 |
| corne contour_parallel | into stock | 40 | 22 | 13 | 6 | 6 (15.0) | 4.00 / 14.50 | 2.99 | 2.49 | 7.6 | 0.3 |
| corne contour_parallel | not into stock | 55 | 53 | 49 | 46 | 46 (83.6) | 0.00 / 13.50 | 0.00 | 0.00 | 81.6 | 7.9 |
| corne contour_parallel | with a rapid | 79 | 62 | 51 | 42 | 42 (53.2) | 0.15 / 14.50 | 0.00 | 0.00 | 74.6 | 3.2 |
| corne contour_parallel | no rapid (keep-down) | 16 | 13 | 11 | 10 | 10 (62.5) | 1.00 / 13.50 | 0.70 | 0.58 | 14.7 | 5.0 |
| corne contour_parallel | Z 17 | 9 | 7 | 7 | 2 | 2 (22.2) | 1.50 / 4.50 | 2.98 | 2.49 | 3.4 | 0.0 |
| corne contour_parallel | Z 11 | 17 | 15 | 10 | 10 | 10 (58.8) | 0.00 / 7.50 | 0.00 | 0.00 | 18.8 | 0.9 |
| corne contour_parallel | Z 5 | 17 | 15 | 10 | 10 | 10 (58.8) | 0.00 / 13.50 | 0.00 | 0.00 | 20.9 | 1.6 |
| corne contour_parallel | Z 0.5 | 52 | 38 | 35 | 30 | 30 (57.7) | 0.15 / 14.50 | 0.00 | 0.00 | 46.1 | 5.6 |
| wanaka200 contour_parallel | all | 1609 | 1138 | 1004 | 630 | 547 (34.0) | 0.93 / 16.80 | 0.53 | 0.26 | 964.6 | 273.2 |
| wanaka200 contour_parallel | into stock | 380 | 259 | 132 | 142 | 60 (15.8) | 5.45 / 16.80 | 1.40 | 0.70 | 166.9 | 31.7 |
| wanaka200 contour_parallel | not into stock | 1229 | 879 | 872 | 488 | 487 (39.6) | 0.58 / 15.26 | 0.24 | 0.12 | 797.7 | 241.5 |
| wanaka200 contour_parallel | with a rapid | 1057 | 708 | 574 | 301 | 218 (20.6) | 1.48 / 16.80 | 2.94 | 1.47 | 389.0 | 35.7 |
| wanaka200 contour_parallel | no rapid (keep-down) | 552 | 430 | 430 | 329 | 329 (59.6) | 0.19 / 4.67 | 0.00 | 0.00 | 575.6 | 237.5 |
| wanaka200 contour_parallel | Z 19.54 | 6 | 1 | 1 | 1 | 1 (16.7) | 5.46 / 5.46 | 1.74 | 0.87 | 1.9 | 0.0 |
| wanaka200 contour_parallel | Z 14.08 | 6 | 1 | 1 | 1 | 1 (16.7) | 5.46 / 5.46 | 1.74 | 0.87 | 2.0 | 0.0 |
| wanaka200 contour_parallel | Z 8.62 | 1511 | 1073 | 940 | 578 | 496 (32.8) | 1.04 / 16.56 | 0.63 | 0.31 | 946.7 | 264.6 |
| wanaka200 contour_parallel | Z 4 | 86 | 63 | 62 | 50 | 49 (57.0) | 0.32 / 16.80 | 0.00 | 0.00 | 14.0 | 8.6 |


## Reading

- spiral_pick lifts where the next point is a short step away but its column still stands (C40 LargeZ is the
  main cause in the join). There the useful link is not a travel but a side step at the level, about one
  stepover long; where the drop is within the depth per pass it is an ordinary cut and removes the lift, the
  link and the plunge. Drops over the depth per pass (wanaka200 has drops near 11 mm) need the descent.
- On spiral_pick, Corne Z 11 and Z 5 (36 each) and wanaka200 Z 19.54 and Z 14.08 (80 each) have the same XY
  and path figures: the same descents repeat at each level; only the drops and times differ.
- contour_parallel's entries are retracts over tens of millimetres. A keep-down link at feed beats the
  rapid only on the short hops; most of the gain of a shared link there would need it to run at the rapid
  rate over cut ground, which the samples' clock (no acceleration) favours.

## Limits

- The stock is the sample rebuild of `fix9/stockmap.py`, not the CLI's dexel stock.
- The side leg is a straight cut; its engagement is read only as its length against the stepover.
- Times are the samples' clock (no acceleration). A rapid's lift and descent cost more with acceleration;
  a low travel has no lift, so the clock is not biased against the low travel.
- The C40 causes come from the D25 build, not fix12. contour_parallel (fix6) is a different build from fix12.
