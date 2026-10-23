# P2 C17 — the streamline march (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Page: `farside_ideas_v3.html`. Scripts: `phase2/c17/`
(`stream.py`, `run_c17.py`, `page_c17.py`, `assert_c17.py`; `try.py`,
`try2.py`, `look.py` and `look2.py` are the debugging tools). Run
`run_c17.py` (about 70 s), then `page_c17.py`, then `assert_c17.py`.
`page_c17.py` writes this note and the page from `results_c17.json`;
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
- **Not a candidate for Rust.** §5 of C15 stands: (a), with its open arcs
  joined by the C14 turns. Every closed-loop plan measured so far ((e) and
  this march with either field, where it ends) cuts 5.5 to 8.4 times the
  length of (a).
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
