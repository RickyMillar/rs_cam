# P2 C15 — far-side ideas for the pocket (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Look page: `farside_ideas_v1.html`. Scripts:
`phase2/c15/` (`geom.py`, `fields.py`, `march.py`, `harness_svg.py`,
`run_c15.py`, `page.py`, `assert_c15.py`, `diag.py`). Run
`python3 -I phase2/c15/run_c15.py`, then `page.py`, then `assert_c15.py`
(about two minutes in all). `assert_c15.py` checks every number on the page
and in this note.

The brief: the operator sees clean wraps on the near side of the island and
wobbly wraps on the far side in all three v5 columns. C15 tests the lead's
three hypotheses and four far-side ideas, each with a checking number.

## 1. Method

### Geometry (each value from code or the fixture, read only)

- `fixtures/demo_pocket.svg`: rect 5..75 × 5..55, rx 8; island r 10 at
  (40, 30). The harness `View` maps world = SVG coordinates; the seed
  (18.5, 18.5) of `pocket_geodesic_curv.txt` confirms it.
- R = 3 (the 6 mm end mill), s = 2.0 and tolerance 0.1
  (`AdaptiveConfig::default()`), planner cell max(R/6, tolerance) = 0.5
  (`path.rs` `planner_cell_size`), pass step 3 cells = 1.5 mm
  (`PASS_STEP_CELLS`).
- Ceiling c = `radial_woc_fraction_from_leading_arc(1.05 · acos(1 − s/R)/2π)`
  = 0.3626 D, so c·D = 2.1759 mm. The brief's 2.1756 is 0.3626 × 6 with c
  rounded. With the unrounded c, ρ* = s(2R − s)/(2(cD − s)) = 22.74 mm (the
  C5 note gives 22.78 from the rounded c).
- Tool-centre space: the region 8..72 × 8..52 with corner radius 5, the hole
  r 13 at (40, 30). Seed to island centre 24.38 mm. Shadow lines leave the
  seed at −4.08° and 60.36° (wedge 64.44°), tangent points (39.08, 17.03)
  and (28.70, 36.43); they meet the wall at (72.00, 14.69) and
  (37.56, 52.00). The zone (the points the hole hides from the seed) is
  944 mm² on the 0.1 mm grid (C11: 959 mm² on 0.5 mm, the harness 927 mm²).

### Exact fields (no lattice)

- Shortest paths round a disc have a closed form: straight, or tangent +
  arc + tangent. `geom.geo` evaluates it at each node of a 0.1 mm grid (the
  tolerance). The rounded outer region is convex, so only the hole bends a
  path.
- (a) T = geodesic distance from the seed − R. The distance from a cut-edge
  segment (C11 without the lattice, and (c), (d)) is the minimum over source
  points on the segment at the grid spacing, with paths kept on the far side
  of the hole.
- Wraps are the level lines of the field (bilinear contouring, `contourpy`).

### The level rule (the same for every idea)

- A new level is the largest value at which EVERY point of the new wrap is
  at most the C5 law gap `min(s, s'(ρ))` from the cut set (the starter
  circle, the earlier wraps, and for the split ideas the exact cut edge).
- ρ is the wrap's own signed curvature radius, from the circle through
  three points at ± one pass step (1.5 mm) of arc length. ρ > 0 when the
  centre lies on the cleared side; a straight or concave piece gets s.
- For a field from the seed, the wrap's own curvature is the true local
  pivot: the seed in view, the tangent point on the island behind it.
- Check: on the near side this rule gives the harness levels to within
  0.025 mm (26 levels).

### The replay (the checking numbers)

- Material grid at the tolerance (0.1 mm) over the pocket; the starter
  clears a disc of radius 2R at the seed. The wraps are cut in level order.
- At each 0.1 mm step the replay reads the swept width as the planner does
  (`compute_swept_width`: the perpendicular extent of standing cells in the
  cutter disc, before the disc is cut), and the area per mm over one pass
  step. The quantum is one cell, 0.1 mm = 0.017 D.
- "Arc interior" is more than R of arc length from either end of an arc.
  The arc ends read high in every idea AND on the clean near side (near
  side: 0.63 D swept, 0.59 D area per mm). So the ends are the wall-end
  class of C7, which the C14 turns address. The ideas are compared on the
  interior.
- The C5 width formula (`area/mm = g + g(R − g/2)/ρ`, the brief's form)
  with the measured gap g and radius ρ is the second number.
- Coverage: the standing stock after the replay, inside R of the zone, and
  farther than the wall scallop `R − √(R² − (s/2)²)` = 0.17 mm plus one cell
  from the pocket wall or the island.
- The harness wraps are read from `phase2/renders5/*.svg` (the pale grey
  paths are `Run2d::wraps`; the SVG holds 0.1 px = 0.008 mm).

## 2. Per-idea table

All loads are fractions of D; the ceiling is c = 0.3626. The near-side
reference (exact circles under the law) reads 0.3616 swept and 0.3771 area
per mm in the arc interior. For (a) and (a) + (c) the levels are of the
whole pocket; for the split ideas they follow the 26 near-side levels.
"Open arcs, plan" counts every wrap of the plan plus one per V (the one-side
rule splits or turns there); it is the count that the C14 turns must join.

<!-- table:begin -->
| Idea | Levels | Zone pieces (V incl.) | V | Open arcs, plan | Zone cut mm | Max gap mm | C5 formula max D | Replay interior D | Area/mm interior D | Arc-end D | Min radius mm | Residue mm² / wall dist mm |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| (a) at the harness levels | 32 | 44 | 9 | 64 | 468 | 2.02 | 0.4527 | 0.4129 | 0.4138 | 0.68 | 0.92 | 0.00 / 0.20 |
| (a) exact, local radius | 35 | 50 | 9 | 70 | 533 | 2.00 | 0.3626 | 0.3574 | 0.3704 | 0.67 | 0.87 | 0.00 / 0.20 |
| exact split (C11, no lattice) | 19 | 38 | 8 | 82 | 508 | 2.00 | 0.3626 | 0.3579 | 0.3679 | 0.86 | 0.87 | 1.30 / 0.76 |
| (b) harmonic | 25 | 54 | 0 | 98 | 1181 | 2.00 | 0.3626 | 0.3513 | 0.3623 | 0.84 | 1.58 | 1.59 / 1.35 |
| (c) round union | 23 | 34 | 0 | 78 | 603 | 2.00 | 0.3626 | 0.3579 | 0.3679 | 0.86 | 2.79 | 1.30 / 0.76 |
| (c), fillet radius ρ* | 23 | 33 | 0 | 77 | 656 | 2.00 | 0.3626 | 0.9934 | 0.3416 | 0.96 | 4.58 | 0.97 / 0.74 |
| (d) one front | 23 | 23 | 0 | 67 | 522 | 2.00 | 0.3626 | 0.3694 | 0.3631 | 0.76 | 5.93 | 2.04 / 0.96 |
| (a) + (c) | 37 | 43 | 0 | 63 | 607 | 2.00 | 0.3626 | 0.3280 | 0.3348 | 0.75 | 3.29 | 0.25 / 1.47 |
<!-- table:end -->

Notes on the table:

- "Min radius" includes the V corners (0.87 mm in (a) and the exact split).
  The smallest convex radius of (a) is 5.95 mm.
- (d) reads 0.3694 swept in one place, 0.04 mm over c·D: less than half the
  0.1 mm quantum. Its area per mm (0.3631) is under the near-side reference.
- Every residue lies less than 1.5 mm from a wall or the island; the split
  ideas leave a sliver at the island where a shadow line touches it. The
  wall pass, which every column runs, removes it.
- The harness split column has 35 zone wraps (`geodesic_curv_split.txt`:
  18 zone levels).

## 3. The ideas

**(a) Exact geodesic distance, curvature read locally.** One front from the
seed. Behind the island the wraps are circle involutes and continue the
near-side circles (G1 at the shadow lines). With the harness levels (the law
with ρ = R + T) the exact wraps read 0.4129 in the arc interior on 172
samples, all 2.8 to 5.9 mm from the grown island. With the wrap's own radius
the same field holds the ceiling (0.3574) for 3 more levels over the whole
pocket, and leaves no residue. The two branches still meet in a V behind the
island (9 V corners).

**(b) Harmonic fill.** Laplace u = 0 on the zone, five-point stencil on the
0.1 mm grid, Shortley–Weller arms at the Dirichlet boundary.

- Boundary conditions and why:
  - u = 0 on the cut edge (the two shadow segments, placed exactly by the
    arm fractions): the zone's wraps must start parallel to the cut.
  - u = 1 on the outer wall: the last wrap is the wall's offset (the
    Bieterman–Sandström property, R3 §Source 2, step 330).
  - ∂u/∂n = 0 on the island arc: the island is neither cut nor the last
    wrap. A wrap meets it at a right angle, as the first region's wraps meet
    a wall. (Ghost node equal to the centre: first order.)
- The levels follow the same law rule (no tuned spacing).
- Result: every level line runs into the two points where u = 0 meets u = 1
  (a shadow line meets the wall). The wraps fan into those points, so the
  zone takes 1181 mm of cut against 508 for the exact split (2.3 times) and
  54 open arcs. The load holds (0.3513). Rejected on cut length and arcs.
  Moving the junction singularity needs non-constant wall data, which is a
  tuned choice.

**(c) Merged fronts (round union).** `τ = min(d_up, d_lo)` where
`|d_up − d_lo| ≥ r`, else `τ = (d_up + d_lo + √(2r² − (d_up − d_lo)²))/2 − r`.
This is the round union of the two cleared strips; it is G1 at `|Δ| = r`
(∇τ = ∇d there).

- **The direction of the bound is the other way round from the brief.** At
  the meeting line the cleared side is concave: the stock is the convex
  wedge between the fronts. So a rounded corner has ρ < 0, and the C5 width
  `g + g(R − g/2)/ρ` is LESS than the gap. The law gives no lower bound on
  the fillet radius.
- The bound comes from the gap. Fillets of one radius on successive levels
  are translates along the meeting line, so the gap at the vertex is
  `g_a = s / sin(ψ/2)` = 3.75 mm (ψ = 64.44°, the angle between the shadow
  lines). With ρ = −r: `g_a − g_a(R − g_a/2)/r ≤ cD` gives
  **`r ≤ g_a(R − g_a/2)/(g_a − cD)` = 2.68 mm**, an upper bound.
- In (d_up, d_lo) the blend is an ellipse; its vertex radius is
  `r_b · sin(α)/(√2 cos² α)` with α = ψ/2, factor 0.5268. So
  r_b = 2.68 / 0.5268. `assert_c15.py` checks the factor on a test V.
- The only fillet with a constant gap is one whose radius falls by Δ per
  level (a fixed centre on the meeting line). That centre needs its feet on
  both cut edges, and near the island no such centre exists (the foot from
  the island's upper tangent point is the island centre). So a fixed-radius
  fillet is the choice.
- Result: 0 V corners, smallest radius 2.79 mm, the load holds (0.3579),
  for 4 more zone levels and 95 mm more cut than the exact split.
- With the brief's ρ* (22.74 mm) the blend covers most of the zone. It no
  longer stays next to the cut: one wrap crosses standing stock at the
  island's far point and reads 0.9934. Rejected.

**(d) One front only (my idea).** The front grows from ONE shadow line: the
one whose farthest zone point is nearer (`d_lo` max 47.84 mm against
`d_up` 49.95 mm, so the lower line). It sweeps the whole zone as lines
parallel to the lower shadow line and wraps round the island as involutes.
Justification: the meeting line and its V exist only because two fronts
enter the zone; one front has no meeting line by construction, and the
other shadow line is already cut, so the front may end there.

- Result: 23 levels, ONE arc per level, each from the island to the wall
  (`diag.ends`). 0 V corners, smallest radius 5.93 mm, 522 mm of cut (3 %
  more than the exact split), load 0.3694 swept (within the quantum) and
  0.3631 area per mm.
- Successive arcs end Δ apart on the island and on the wall, so C14 turns
  can join them in a zigzag (one entry for the zone). The cut direction then
  alternates (climb, then conventional); a climb-only order needs a
  keep-down return along each cut arc.

**(a) + (c).** The fronts of (a) with the same round union where both
branches bend round the island (214.8 mm² of the zone). 37 levels (2 more
than (a)), 0 V corners, the lowest load (0.3280 swept, 0.3348 area per mm)
and the fewest open arcs of any plan (63).

## 4. Verdict on the three hypotheses

1. **Lattice pivots: a small part, not the visible wobble.** Behind the
   island the `geodesic_curv` lattice wraps lie within 0.47 mm of the exact
   wraps at the same levels (RMS 0.18 mm, on average 0.15 mm farther out;
   the near side is exact to 0.008 mm, the SVG precision). Along one wrap the
   error has a standard deviation of at most 0.11 mm. The far-side mess of
   `geodesic_curv` comes from the SPACING (the C5 law's known limit): at the
   harness levels the exact wraps read 0.4129 > c behind the island, the
   replay refuses those steps, and the cascade follows. The harness's first
   far-side wrap refusals ((27.50, 35.31), (29.04, 39.00)) are at the
   island's upper tangent point.
2. **Jagged seed: the main cause in the split column.** The zone field of
   `geodesic_curv_split` seeds T = 0 on the cleared lattice nodes. Near the
   walls at the shadow-line ends that set reaches into the zone, so the zone
   wraps bend toward the wall. Against the exact distance from the shadow
   lines, one harness zone wrap varies by up to 1.29 mm (standard deviation;
   median 0.31 mm over 32 wraps), against 0.11 mm for the lattice alone.
3. **The V: a corner, not a wobble.** The exact split wraps are straight and
   meet in V corners (8, radius 0.87 mm). The V costs arcs (each V is a split
   or a turn), not shape. (c), (d) and (a) + (c) remove it.

## 5. Recommended next Rust step

**(a): one field from the seed, levels by the law at each level's own
curvature, minimum along the level.** Then (c) on top of it if the operator
wants the V behind the island rounded.

- What it is: a new `Spacing` value (for example `CurvatureLevel`) in
  `research_geodesic.rs`. For each next level, contour a trial T, read the
  three-point radius over the pass step at each contour point, take
  `min(s, s'(ρ))`, and find the largest ΔT at which every point of the new
  contour is within it of the cut (bisection). It keeps ONE T per level, so
  the levels stay parallel; this differs from C6's `CurvatureLocal`, which
  solves a pass count U with a variable speed (non-parallel levels that pack
  round the island corners; D6: cascades 876 → 202 but entries 89).
- Lattice risk, measured: on the harness lattice wraps the pass-step radius
  changes the allowed gap by −0.18 to +0.07 mm (95 % within 0.06 mm) against
  the true pivot distance. On exact wraps the reading is −0.04 to +0.21 mm
  (95 % within 0.02 mm); the +0.21 is next to the tangent point, where the
  replay still holds.
- Cost: about 30 contourings per level (bisection) and 35 levels on the
  pocket; no new field, no split, no zone. The plan keeps the
  `geodesic_curv` structure, so the C14 turns apply unchanged. The far-side
  wraps then continue the near-side circles, which is the clean pattern the
  operator asked for.
- (c) on top costs a second branch value per zone node (the source tracking
  keeps one T per node; the round union needs both `T_up` and `T_lo`): one
  more solve restricted to the zone, and the blend formula above with
  r_b = 2.68 / 0.5268 mm.
- If the lead keeps the split instead: take (d) (seed only the lower shadow
  line's cut edge) and seed from the exact cut edge, not from the cleared
  nodes, to remove the hypothesis-2 bend.
- Do not build (b).

## 6. Limits

- The replay is a 0.1 mm raster model, not the planner's replay; the arc
  ends are not judged here (C14).
- The fields are exact for one disc island in a convex outer region. A
  general part needs the lattice; the radius reading above is the part of
  (a) that a lattice must carry.
- The Neumann condition of (b) is first order; (b) is rejected on its shape,
  which does not depend on that order.
