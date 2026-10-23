# P2 C17 — the streamline march (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Pages: `farside_ideas_v3.html` (frozen) and `farside_ideas_v4.html` (current). Scripts: `phase2/c17/`
(`stream.py`, `run_c17.py`, `page_c17.py`, `assert_c17.py`; `try.py`,
`try2.py`, `look.py` and `look2.py` are the debugging tools). Run
`run_c17.py` (about 70 s), then `page_c17.py`, then `assert_c17.py`.
`page_c17.py` writes this note and the page from `results_c17.json` and
`sweep.json` (`sweep.py` with `plan.py`, §7–§8);
`assert_c17.py` checks every number in both. C15's scripts are imported from
`phase2/c15/` and not edited.

## 1. The question

The operator, on C15 (e): "Harmonic looks amazing, but it overlaps the same
area a lot; I wonder if a post-process could make it more efficient?" A loop
of (e) is a level line of u, so its gap follows 1/|∇u| and the loops crowd
where u is steep. The lead's idea (untested before C17): use ∇u only for the
direction; move each point of a loop along its own streamline by its own law
gap; a point that reaches a wall stops on it.

## 2. Method

- Geometry, fields and replay: C15 (`geom.py`, `shapes.py`, `whole.py`
  `harmonic`, `march.py` `Replay`). c·D = 2.175868 mm from the code formula
  (c = 0.3626), R = 3, s = 2, tolerance 0.1 mm, pass step 1.5 mm.
- Direction: the unit gradient of the field, bilinear on the 0.1 mm grid;
  the streamline is traced by the midpoint rule in sub-steps of at most one
  cell. A step that leaves the region is bisected onto the wall.
- Step: per point, the C5 law gap min(s, s'(ρ)) with ρ the signed radius of
  the NEW loop at that point (three points at ± one pass step of arc, centre
  inside the loop = cleared side). The steps of all points are one implicit
  system (each radius depends on the neighbours' steps). Two failures came
  first and are part of the result:
  - The explicit rule (radius of the old loop) and a "lower monotonically
    from s" rule both amplify a small wave each loop; in the first trials
    the star loop length grew without bound (those trials are not kept).
  - A damped fixed point (the damping halves whenever the largest residual
    grows) converges to 0.0001 mm; the largest rule violation of a final
    loop is 0.0002 mm (star) and 0.0002 mm (pocket).
- Two step variants: "step" (the lead's rule: the arc length along the
  streamline is the law gap) and "normal" (the distance from the new point
  to the cut is the law gap).
- Stops: a wall (or the island), and the air stop: a point whose move cuts
  no standing stock (the tool disc at its new place holds no stock in the
  replay raster) stops there.
  - The pocket needs it. Behind the island the field has a saddle; the
    fronts from above and below meet along its outgoing line, from the
    island to the far corner. Without a stop there, the points on that line
    move along it and the march does not end (seen in a trial, not kept).
  - A "fronts meet" test on distances was tried first. It needs labels to
    tell its own front from the other one, and it stopped points on lines
    that still had stock beyond them. The air stop replaced it.
- Resampling: a segment longer than the tolerance gets points at the
  tolerance; a moving point nearer than half the tolerance to the last kept
  point is dropped. A segment between two wall stops gets new points only if
  its midpoint lies more than half a cell from the wall (a chord across a
  corner with stock beyond it). No other number enters the march.
- Metrics: the C15 replay cuts every loop in order (each loop includes its
  stopped parts, which cut nothing). "Interior" leaves out R of arc at the
  seam and at each change between moving and stopped parts. "Unreachable"
  residue is farther than R (plus half a cell) from every region node.
- Control: the same march along the gradient of the exact entry distance.

## 3. Tables

Star:

<!-- star:begin -->
| Plan | Loops / levels | Open arcs | Self-crossings | Cut mm | Moving mm | Wall + air runs mm | Mean width D | Interior max D (over c) | > R from wall, max D | Interior p99 D | Area/mm max D | Residue mm² (unreachable) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| streamline march (the lead's rule) | 30 | 0 | 0 | 5490 | 1424 | 4067 + 0 | 0.076 | 0.5133 (73) | 0.3622 | 0.3748 | 0.3878 | 56.9 (55.5) |
| streamline march, normal-gap step | 30 | 0 | 0 | 5553 | 1369 | 4184 + 0 | 0.075 | 0.5539 (297) | 0.3685 | 0.4164 | 0.4278 | 56.6 (55.6) |
| control: exact-distance streamlines | 28 | 0 | 0 | 4995 | 1387 | 3608 + 0 | 0.084 | 0.5662 (257) | 0.3679 | 0.4094 | 0.4326 | 56.7 (55.5) |
| C15 (e): harmonic level lines | 32 | 0 | — | 5516 | — | — | 0.076 | 0.8046 | — | — | 0.3717 | 57.3 |
| C15 (a): exact-distance wraps | 19 | 56 | — | 900 | — | — | 0.456 | 0.3615 | — | — | 0.3771 | 77.3 |
<!-- star:end -->

Pocket:

<!-- pocket:begin -->
| Plan | Loops / levels | Open arcs | Self-crossings | Cut mm | Moving mm | Wall + air runs mm | Mean width D | Interior max D (over c) | > R from wall, max D | Interior p99 D | Area/mm max D | Residue mm² (unreachable) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| streamline march (the lead's rule) | 49 | 0 | 4 | 10313 | 1851 | 7936 + 526 | 0.049 | 0.4393 (38) | 0.3741 | 0.3634 | 0.3801 | 1.7 (0.0) |
| streamline march, normal-gap step | 41 | 0 | 32127 | 9045 | 2428 | 6066 + 552 | 0.055 | 0.7718 (313) | 0.6933 | 0.4078 | 0.5389 | 0.0 (0.0) |
| control: exact-distance streamlines | 39 | 0 | 89 | 6216 | 1284 | 4136 + 796 | 0.052 | 0.8559 (107) | 0.3842 | 0.3850 | 0.4247 | 1046.8 (0.0) |
| C15 (e): harmonic level lines | 58 | 0 | — | 10536 | — | — | 0.048 | 0.3651 | — | — | 0.3746 | 0.0 |
| C15 (a): exact-distance wraps | 35 | 56 | — | 1260 | — | — | 0.386 | 0.3617 | — | — | 0.3771 | 50.4 |
<!-- pocket:end -->

## 4. The five questions

### 1. One closed loop?

Yes, on both shapes, for every loop (30 on the star, 49 on the pocket). A streamline that reaches the island stops on it, so the loop runs along the near side of the island as it runs along a wall. The loop does not have to split. Behind the island the two fronts come together on the line from the island to the far corner. There the rule must stop a point when its move cuts no stock (the air stop, 267 points). The two fronts then stop on the two sides of a corridor of cleared stock, and every later loop runs up one side and down the other. Without the air stop the points on that line never stop (seen in a trial; the march did not end). The pocket loops cross themselves 4 times, all in loops 48, 49 at x 70.5 to 72.0, y 46.4 to 50.5, where the corridor meets the corner wall.

### 2. The real gap and load

The lead's prediction holds: the gap normal to the loop is at most the streamline step. Normal gap / step: median 0.979 and smallest 0.792 on the star; median 0.926 and smallest 0.048 on the pocket. More than R from every wall the replay holds the ceiling to within one cell: 0.3622 D star, 0.3741 D pocket (c = 0.3626 D; one cell is 0.0167 D). The interior samples over c (73 star, 38 pocket of 9234 and 12565) all lie within R of a wall. On the star they are on loops 8 to 17, the loops that first reach the walls; the largest is 0.5133 D. Distribution in the interior: median 0.328, 90 % 0.351, 99 % 0.375 D on the star. The normal-gap variant steps farther where the streamline is oblique and reads up to 0.77 D on the pocket.

### 3. Coverage

The star leaves 56.9 mm²; 55.5 mm² of it is in the five tips, farther than R from every centre position, so no plan cuts it ((e) leaves 57.3 mm²). The pocket leaves 1.73 mm² in one piece at (73.9, 49.4), 1.33 mm from the wall, in the corner where the corridor ends. Where streamlines diverge the loop gets new points at the tolerance, so no wedge stays uncut.

### 4. Loops and length

Star: 30 loops against 19 levels of (a) and 32 of (e). Cut length 5490 mm: 6.1 times (a) and 1.00 times (e). Pocket: 49 loops, 10313 mm, 8.2 times (a) and 0.98 times (e). The wall and corridor runs are 74% (star) and 82% (pocket) of the length: each loop runs along all the wall that earlier loops reached. The moving part alone is 1.58 times (a) on the star and 1.47 times on the pocket. A straight link across each stopped run instead of the wall run is a lower bound for a shortcut; it still gives 5.0 times (a) on the star.

### 5. Smoothness

Away from the stops (more than one pass step of arc) the smallest loop radius is 1.98 mm (star) and 2.13 mm (pocket); next to a stop it falls to 0.75 mm. Where a loop leaves a wall it turns by a median 34° and at most 136° (star; pocket 99°) over one pass step: a corner in the loop.

## 5. Verdict

- The march gives one closed loop per level on both shapes, the load holds
  more than R from the walls, and the stock is cut (only the unreachable
  tips of the star and 1.73 mm² in one pocket corner stay).
- It does not make (e) efficient. Its cut length is 1.00 times (e) on the
  star and 0.98 times on the pocket; the mean width (stock per mm of cut)
  is 0.076 D against (e) 0.076 D and (a) 0.456 D on the star. The overlap of (e)
  is not removed; it moves into the wall runs: 74% of the star length and
  82% of the pocket length.
- The reason is the closed loop itself, not the field: a closed loop must
  run along every part of the wall that earlier loops have reached. The
  control with straight rays shows the same (5.5 times (a) on the star).
  So no post-process of the loop shape can remove it while the loops stay
  closed.
- The moving part alone is 1.58 times (a) (star) and 1.47 times (pocket): the
  loops also take more levels into the star tips (30 loops against 19).
- The normal-gap variant is not stable on the pocket (steps to 14.6 mm,
  32127 self-crossings, 0.77 D).
- **Not a candidate for Rust.** By cut length, every closed-loop plan
  measured so far ((e) and this march with either field, where it ends) cuts
  5.5 to 8.4 times the length of (a). With the air counted as keep-down
  travel (§7), the march takes 1.00 (star) and 0.97 (pocket) times the time
  of (e) at the cut feed, but its cut parts read over c where a loop leaves a
  wall (§8). So it is still not a candidate.
- In the family from (a) to (e) (§8) only (e) has no retract and holds c in
  its cut parts: λ* is (e) on both shapes. (a) is faster unless a retract
  costs more than 4.0 s (star) or 13.8 s (pocket); that cost is not measured.
  The lead decides between (a) with the C14 turns and (e) on that cost.
- What a Rust version would cost (prediction, not measured): a Laplace
  solve on the region, a streamline tracer, the implicit step solve per
  loop (many radius reads over the whole loop per loop), the air
  stop (a stock query per point per loop), and the loop resampling. That is
  more than the C15 (a) level rule, for a plan that cuts 5.5 to 8.4 times as much.

## 6. Limits

- Two shapes, one tool, one stepover. The replay is C15's 0.1 mm raster,
  not the planner.
- The links between loops are not modelled; every loop is cut whole.
- The air stop reads the replay raster; a Rust stock model would read its
  own.

## 7. Addition 1 (operator): cut parts and link parts

The operator: the overlap of the harmonic plans is keep-down travel through
cleared stock, not load, so cut length is the wrong cost. Script:
`sweep.py` with `plan.py` (a few minutes on a process pool).

- Engaged / air. Each plan is replayed in order, links included. A sample
  is engaged when its area per mm over one pass step (centred) is more than
  one cell width, 0.1 mm (the raster's resolution); else it is air.
- Links: FreeCAD's rule, R1 §4 (ideas only). A link is clear when the tool
  disc grown by s·R = 1.00 mm holds no standing stock, tested from
  2·s·R inside each end. A keep-down route may be up to 3 × the direct
  length (the shortest 8-connected path over clear nodes, then string
  pulled: each kept point goes straight to the farthest later point it sees
  over clear nodes; the grid path alone reads up to 8.2% long). A gap
  under 2·s·R may be cut straight at most
  1.5 × c·D per mm. Otherwise the link is a retract.
- Order: levels in order; in a level the nearest curve next. An open arc
  starts at its nearer end and a closed loop at its nearest point. The cut
  direction (climb or conventional) is not kept.
- Feeds:
  - Cut feed 1500 mm/min: `fixture_feeds(Adaptive)`,
    `crates/rs_cam_core/src/compute/fixture_values.rs:34-35` (S11: the op
    has no feed default).
  - The 2D adaptive emits its keep-down `Link` at `params.feed_rate`, the
    cut feed: `crates/rs_cam_core/src/adaptive/path.rs:3163-3168`.
  - A separate link feed exists only in the link-moves dressup:
    `link_feed_rate`, default 500 mm/min, `compute/config.rs:1077-1083`
    and `1439`.
  - Times are information only: length over feed, no acceleration, and
    retracts are counted, not timed.

Star:

<!-- plans-star:begin -->
| Plan | Engaged mm | Air mm (share) | Keep-down links | Retracts | Time, air at cut feed min | Time, air at link feed min | Cut-part interior max D (over c) | Any engaged max D |
|---|---|---|---|---|---|---|---|---|
| C15 (a): exact-distance wraps | 932 | 10 (1%) | 18 | 46 | 0.63 | 0.64 | 0.3613 (0) | 0.519 |
| C15 (e): harmonic level lines | 2389 | 3119 (57%) | 32 | 0 | 3.67 | 7.83 | 0.3575 (0) | 0.784 |
| streamline march | 1422 | 4084 (74%) | 11 | 0 | 3.67 | 9.12 | 0.4988 (89) | 0.717 |
| control: exact-distance streamlines | 1371 | 3641 (73%) | 11 | 0 | 3.34 | 8.20 | 0.5662 (280) | 0.763 |
| (e), air runs replaced by links | 2406 | 3089 (56%) | 96 | 0 | 3.66 | 7.78 | 0.3575 (0) | 0.786 |
| march, air runs replaced by links | 1430 | 3776 (73%) | 27 | 0 | 3.47 | 8.50 | 0.5078 (90) | 0.700 |
<!-- plans-star:end -->

Pocket:

<!-- plans-pocket:begin -->
| Plan | Engaged mm | Air mm (share) | Keep-down links | Retracts | Time, air at cut feed min | Time, air at link feed min | Cut-part interior max D (over c) | Any engaged max D |
|---|---|---|---|---|---|---|---|---|
| C15 (a): exact-distance wraps | 1316 | 8 (1%) | 34 | 27 | 0.88 | 0.89 | 0.3617 (0) | 0.668 |
| C15 (e): harmonic level lines | 3852 | 6774 (64%) | 68 | 0 | 7.08 | 16.12 | 0.3632 (0) | 0.363 |
| streamline march | 1735 | 8584 (83%) | 8 | 0 | 6.88 | 18.32 | 0.4396 (91) | 0.552 |
| control: exact-distance streamlines | 1138 | 5082 (82%) | 7 | 0 | 4.15 | 10.92 | 0.5098 (175) | 0.856 |
| (e), air runs replaced by links | 3936 | 5105 (56%) | 279 | 0 | 6.03 | 12.83 | 0.3632 (0) | 0.363 |
| march, air runs replaced by links | 1743 | 4440 (72%) | 88 | 0 | 4.12 | 10.04 | 0.4396 (98) | 0.552 |
<!-- plans-pocket:end -->

- With air at the cut feed, (e) and the march take about the same time:
  3.67 and 3.67 min on the star, 7.08 and 6.88 min on the pocket.
- (a) takes 0.63 and 0.88 min, but needs 46 and 27 retracts.
- (e) engages over 2.6 times the length of (a) on the star: its
  engaged part is also a thinner cut.
- The cut-part interior of (e) holds c on both shapes. Its tip samples
  (up to 0.784 D on the star) lie within R of where an engaged run begins.

## 8. Addition 2 (operator): one family from (a) to (e)

- Family: u_λ = (1 − λ)·T/T_max + λ·u_harm. T is the exact entry distance,
  u_harm the harmonic field; both are 0 on the entry disc and 1 at their far
  end. Outside the region T keeps its closed form and u_harm is 1, so a level
  line that reaches a wall is cut there (a point within one cell outside is
  the contour's own error and is kept). λ = 0 is (a), λ = 1 is (e).
- Why this blend: it is the one-parameter path between the two plans with no
  other parameter. A blend that keeps the wall value constant (closed loops)
  is (e) itself; for λ < 1 the wall value λ + (1 − λ)·T/T_max varies, so the
  late levels reach the walls as open arcs.
- Levels: the C15 rule, run until every node lies within one cell of a cut
  (the same stop for every λ; it differs from C15's (a), which stopped at s).
- Grid: steps of 0.05, then finer below the smallest λ that meets both
  conditions, down to 0.001 (34 values per shape).
- λ* = the least time at the cut feed among the λ with (1) no retract and
  (2) cut-part interior width at most c plus one cell. It is computed.

Star:

<!-- lam-star:begin -->
| λ | Levels | Curves (open arcs) | Levels with a retract | Retracts | Engaged mm | Air mm | Time cut feed min | Time link feed min | Cut-part interior max D | Any engaged max D | Both conditions |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.000 | 20 | 73 (66) | 13 | 48 | 945 | 7 | 0.63 | 0.64 | 0.3618 | 0.513 | no |
| 0.050 | 20 | 72 (65) | 13 | 49 | 950 | 7 | 0.64 | 0.65 | 0.3609 | 0.503 | no |
| 0.100 | 20 | 72 (65) | 13 | 45 | 970 | 13 | 0.66 | 0.67 | 0.3598 | 0.507 | no |
| 0.150 | 20 | 70 (63) | 13 | 47 | 972 | 14 | 0.66 | 0.68 | 0.3604 | 0.503 | no |
| 0.200 | 20 | 71 (64) | 13 | 46 | 985 | 14 | 0.67 | 0.68 | 0.3595 | 0.510 | no |
| 0.250 | 20 | 68 (60) | 12 | 43 | 1000 | 7 | 0.67 | 0.68 | 0.3596 | 0.502 | no |
| 0.300 | 21 | 72 (64) | 13 | 44 | 1013 | 10 | 0.68 | 0.70 | 0.3609 | 0.504 | no |
| 0.350 | 21 | 71 (63) | 12 | 42 | 1027 | 12 | 0.69 | 0.71 | 0.3612 | 0.508 | no |
| 0.400 | 21 | 70 (62) | 13 | 43 | 1042 | 13 | 0.70 | 0.72 | 0.3604 | 0.505 | no |
| 0.450 | 21 | 69 (61) | 13 | 43 | 1054 | 18 | 0.71 | 0.74 | 0.3598 | 0.507 | no |
| 0.500 | 21 | 67 (58) | 12 | 42 | 1071 | 9 | 0.72 | 0.73 | 0.3601 | 0.507 | no |
| 0.550 | 21 | 69 (60) | 12 | 42 | 1088 | 9 | 0.73 | 0.74 | 0.3609 | 0.506 | no |
| 0.600 | 21 | 70 (61) | 11 | 38 | 1118 | 28 | 0.76 | 0.80 | 0.3592 | 0.504 | no |
| 0.650 | 21 | 67 (58) | 12 | 41 | 1135 | 20 | 0.77 | 0.80 | 0.3592 | 0.503 | no |
| 0.700 | 21 | 65 (55) | 11 | 36 | 1167 | 40 | 0.81 | 0.86 | 0.3591 | 0.506 | no |
| 0.750 | 22 | 67 (57) | 10 | 37 | 1201 | 35 | 0.82 | 0.87 | 0.3586 | 0.506 | no |
| 0.800 | 22 | 64 (53) | 10 | 34 | 1255 | 62 | 0.88 | 0.96 | 0.3576 | 0.534 | no |
| 0.850 | 22 | 65 (54) | 9 | 31 | 1312 | 113 | 0.95 | 1.10 | 0.3580 | 0.511 | no |
| 0.900 | 23 | 65 (53) | 9 | 29 | 1396 | 149 | 1.03 | 1.23 | 0.3581 | 0.506 | no |
| 0.950 | 23 | 62 (49) | 7 | 23 | 1554 | 264 | 1.21 | 1.57 | 0.3576 | 0.507 | no |
| 0.955 | 23 | 61 (48) | 7 | 22 | 1575 | 324 | 1.27 | 1.70 | 0.3576 | 0.504 | no |
| 0.960 | 23 | 59 (45) | 6 | 20 | 1606 | 353 | 1.31 | 1.78 | 0.3576 | 0.503 | no |
| 0.965 | 24 | 62 (48) | 7 | 22 | 1630 | 337 | 1.31 | 1.76 | 0.3576 | 0.512 | no |
| 0.970 | 24 | 61 (47) | 7 | 21 | 1668 | 346 | 1.34 | 1.80 | 0.3576 | 0.506 | no |
| 0.975 | 24 | 60 (46) | 6 | 19 | 1711 | 437 | 1.43 | 2.02 | 0.3576 | 0.502 | no |
| 0.980 | 24 | 59 (44) | 6 | 18 | 1765 | 415 | 1.45 | 2.01 | 0.3576 | 0.514 | no |
| 0.985 | 25 | 61 (46) | 5 | 16 | 1836 | 474 | 1.54 | 2.17 | 0.3576 | 0.513 | no |
| 0.990 | 25 | 58 (42) | 6 | 15 | 1921 | 531 | 1.63 | 2.34 | 0.3576 | 0.524 | no |
| 0.995 | 26 | 59 (42) | 5 | 12 | 2048 | 742 | 1.86 | 2.85 | 0.3575 | 0.513 | no |
| 0.996 | 26 | 54 (36) | 5 | 11 | 2082 | 753 | 1.89 | 2.89 | 0.3575 | 0.511 | no |
| 0.997 | 26 | 57 (39) | 4 | 9 | 2118 | 873 | 1.99 | 3.16 | 0.3575 | 0.504 | no |
| 0.998 | 27 | 55 (36) | 4 | 9 | 2165 | 986 | 2.10 | 3.42 | 0.3575 | 0.501 | no |
| 0.999 | 27 | 53 (33) | 3 | 5 | 2227 | 1204 | 2.29 | 3.89 | 0.3575 | 0.514 | no |
| 1.000 | 32 | 32 (0) | 0 | 0 | 2389 | 3119 | 3.67 | 7.83 | 0.3575 | 0.784 | yes |
<!-- lam-star:end -->

Pocket:

<!-- lam-pocket:begin -->
| λ | Levels | Curves (open arcs) | Levels with a retract | Retracts | Engaged mm | Air mm | Time cut feed min | Time link feed min | Cut-part interior max D | Any engaged max D | Both conditions |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.000 | 44 | 104 (100) | 27 | 34 | 1619 | 1314 | 1.96 | 3.71 | 0.4406 | 0.814 | no |
| 0.050 | 44 | 104 (99) | 26 | 32 | 1634 | 1367 | 2.00 | 3.82 | 0.4996 | 0.731 | no |
| 0.100 | 43 | 102 (97) | 27 | 34 | 1600 | 1278 | 1.92 | 3.62 | 0.4614 | 0.783 | no |
| 0.150 | 43 | 103 (98) | 24 | 32 | 1626 | 1273 | 1.93 | 3.63 | 0.4419 | 0.781 | no |
| 0.200 | 43 | 102 (96) | 23 | 31 | 1631 | 1341 | 1.98 | 3.77 | 0.3933 | 0.699 | no |
| 0.250 | 42 | 99 (93) | 24 | 30 | 1615 | 1241 | 1.90 | 3.56 | 0.4661 | 0.723 | no |
| 0.300 | 42 | 99 (92) | 24 | 28 | 1643 | 1236 | 1.92 | 3.57 | 0.3889 | 0.710 | no |
| 0.350 | 42 | 100 (93) | 24 | 28 | 1654 | 1250 | 1.94 | 3.60 | 0.3992 | 0.809 | no |
| 0.400 | 42 | 101 (94) | 24 | 29 | 1660 | 1259 | 1.95 | 3.62 | 0.3661 | 0.779 | no |
| 0.450 | 41 | 97 (89) | 20 | 24 | 1659 | 1206 | 1.91 | 3.52 | 0.3703 | 0.700 | no |
| 0.500 | 41 | 98 (90) | 22 | 26 | 1671 | 1234 | 1.94 | 3.58 | 0.3613 | 0.883 | no |
| 0.550 | 41 | 98 (90) | 21 | 25 | 1698 | 1214 | 1.94 | 3.56 | 0.3605 | 0.865 | no |
| 0.600 | 41 | 98 (89) | 20 | 24 | 1719 | 1222 | 1.96 | 3.59 | 0.3893 | 0.700 | no |
| 0.650 | 41 | 98 (89) | 21 | 25 | 1742 | 1205 | 1.96 | 3.57 | 0.3652 | 0.623 | no |
| 0.700 | 42 | 100 (90) | 20 | 25 | 1771 | 1252 | 2.02 | 3.68 | 0.3674 | 0.659 | no |
| 0.750 | 42 | 99 (88) | 20 | 24 | 1850 | 1210 | 2.04 | 3.65 | 0.5115 | 0.606 | no |
| 0.800 | 42 | 104 (93) | 20 | 26 | 1876 | 1275 | 2.10 | 3.80 | 0.3621 | 0.652 | no |
| 0.850 | 43 | 102 (90) | 20 | 25 | 1949 | 1268 | 2.14 | 3.83 | 0.3623 | 0.616 | no |
| 0.900 | 43 | 100 (87) | 20 | 24 | 2077 | 1195 | 2.18 | 3.77 | 0.3693 | 0.614 | no |
| 0.950 | 45 | 105 (90) | 18 | 23 | 2284 | 1348 | 2.42 | 4.22 | 0.3646 | 0.616 | no |
| 0.955 | 45 | 103 (87) | 19 | 22 | 2306 | 1245 | 2.37 | 4.03 | 0.3615 | 0.620 | no |
| 0.960 | 45 | 101 (85) | 18 | 22 | 2328 | 1237 | 2.38 | 4.03 | 0.3648 | 0.626 | no |
| 0.965 | 46 | 103 (87) | 20 | 23 | 2351 | 1355 | 2.47 | 4.28 | 0.3677 | 0.614 | no |
| 0.970 | 46 | 105 (88) | 17 | 21 | 2402 | 1355 | 2.50 | 4.31 | 0.3662 | 0.625 | no |
| 0.975 | 46 | 104 (87) | 20 | 23 | 2439 | 1337 | 2.52 | 4.30 | 0.3591 | 0.622 | no |
| 0.980 | 47 | 103 (85) | 18 | 21 | 2479 | 1384 | 2.58 | 4.42 | 0.3658 | 0.616 | no |
| 0.985 | 47 | 103 (84) | 18 | 21 | 2542 | 1395 | 2.62 | 4.49 | 0.3656 | 0.623 | no |
| 0.990 | 48 | 103 (83) | 17 | 19 | 2596 | 1533 | 2.75 | 4.80 | 0.3644 | 0.605 | no |
| 0.995 | 49 | 104 (83) | 14 | 16 | 2640 | 1742 | 2.92 | 5.24 | 0.3657 | 0.599 | no |
| 0.996 | 49 | 103 (81) | 14 | 17 | 2668 | 1751 | 2.95 | 5.28 | 0.3595 | 0.609 | no |
| 0.997 | 49 | 101 (78) | 16 | 19 | 2676 | 1761 | 2.96 | 5.31 | 0.3635 | 0.581 | no |
| 0.998 | 50 | 105 (81) | 15 | 19 | 2717 | 1965 | 3.12 | 5.74 | 0.3624 | 0.609 | no |
| 0.999 | 51 | 103 (77) | 12 | 14 | 2775 | 2195 | 3.31 | 6.24 | 0.3625 | 0.592 | no |
| 1.000 | 58 | 68 (0) | 0 | 0 | 3852 | 6774 | 7.08 | 16.12 | 0.3632 | 0.363 | yes |
<!-- lam-pocket:end -->

Result:

- Only λ = 1 meets both conditions on both shapes: λ* = 1.000 (star) and
  1.000 (pocket), which is (e).
- Every λ below 1 has retracts: at least 5 (star) and 14 (pocket). At
  λ = 0.999 there are still 5 and 14. The late levels reach the walls in the
  lobes and behind the island, and an open arc in a lobe has no keep-down
  route to the next one.
- So the family cannot be set so that the overlap is only useful links:
  the plan has either retracts or the air of (e).
- Break-even: a plan below λ* is faster than (e) unless a retract costs more
  than (time of (e) − time of the plan) / retracts. Over λ < 1 that is
  3.7 to 16.6 s per retract (star) and 9.1 to 16.2 s (pocket); for (a)
  4.0 s and 13.8 s. These are at the cut feed with no acceleration. The cost of a
  retract (lift, travel, re-entry at the plunge feed) is not measured here,
  so this note does not choose between (a) and (e) on time.
- The streamline march has no retract. But its cut parts read
  0.4988 D (star) and 0.4396 D (pocket) where a loop leaves a wall, which is over c. So it
  fails condition (2) and does not beat λ* (star no, pocket no).

## 9. Lead's variant: keep the loop shape, replace the air runs by links

- Each loop of (e) (and of the march) keeps its shape. An air run (it cuts
  nothing, because an earlier loop already cut it) is replaced by the
  shortest keep-down route between its ends (§7 rule) when that route is
  shorter. This is what the C14 turns do for (a). The run is classified
  against the stock at the time it is cut.
- Star (e): 64 of 159 air runs get a shorter link, which saves only 2.7 mm. The air runs lie
  along the straight lobe walls, where the run is already the shortest
  route. The time is 3.66 min against 3.67 min.
- Pocket (e): 212 of 334 runs, which saves 1580 mm. The time is 6.03 min against 7.08 min.
- March: it saves 301 mm (star) and 4135 mm (pocket), for 3.47 and 4.12 min.
  Its cut parts still read 0.5078 D and 0.4396 D, which is over c.
- No retract is added (0 and 0). Against (a) (46 and 27 retracts), the
  trimmed (e) is faster only if a retract costs more than 4.0 s (star) or
  11.4 s (pocket).
- The rest of the extra time of (e) is engaged path: (e) engages 2.6 and 2.9
  times the length of (a). No change to the links removes that.
