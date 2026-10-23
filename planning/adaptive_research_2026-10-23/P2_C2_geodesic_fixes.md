# P2 editor C2 — candidate (a), the G1 fixes (2026-10-23)

Status: CODE COMPLETE for every defect, NOT RUN. This file grew one section per fix. **No cargo was
run** (the brief says so). Nothing is committed. Worktree
`.claude/worktrees/adaptive-head`, branch `adaptive-instruments`, base
405cbd23.

Order of work: housekeeping, G1-1, G1-2 (measure, then fix), G1-3 (measure,
then fix), the plunge per wrap, then the verifier commands.

## Files touched so far

- `crates/rs_cam_core/src/adaptive/path.rs` (housekeeping)
- `crates/rs_cam_core/src/adaptive/research_geodesic.rs` (housekeeping,
  G1-1, G1-2, G1-3, the join; module doc)
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs` (G1-1
  render, G1-2 over-step lines)


## 1. Housekeeping: the two copied values (DONE)

The verifier found `/ 6.0` and `* 3.0` in `research_geodesic.rs`, copied
from `path.rs`. `path.rs` itself held five more copies of the same two
values (`LinkLoad::new`, `adaptive_segments_with_debug` twice,
`apply_residue_mop_cleanup`, `apply_contour_parallel_residue_cleanup`).

Change (behaviour-neutral, same arithmetic):

- `path.rs` now has one source: `pub(super) const
  PLANNER_CELLS_PER_TOOL_RADIUS = 6.0`, `pub(super) const PASS_STEP_CELLS =
  3.0`, and `pub(super) fn planner_cell_size(tool_radius, tolerance)` and
  `pub(super) fn pass_step_len(cell_size)`, each with its doc. The doc says
  that these are shipped values, not values derived here.
- The five `path.rs` copies in product code call the two functions.
- `research_geodesic.rs` calls them (`planner_cell`, the readout step, the
  unit tests).
- Not changed: the test modules of `path.rs` (`R / 6.0`, `* 3.0` in one
  test) and of `mod.rs` (lines 2252, 2284). They are test fixtures.

## 2. G1-1 edge spill (DONE in code, not run)

### Measurement

- **Why the PNGs were one colour.** The harness SVGs DO carry colours
  (`star_geodesic_a.svg`: 2061 strokes `#1f5fbf`, 66 `#f08000`, 86 red entry
  rings `#d00`, 103 pale wraps `#d0d0d0`). The PNGs beside them are
  1031 x 987 px (the SVG is 900 x 862) and hold only grey levels: the
  outside converter that made them dropped every colour and every
  `<circle>`. So the operator saw no entry ring and no link colour. The
  harness wrote no PNG itself.
- **What the black strokes at the arm tips are.** The old SVG drew every
  emitted move one colour by intent (cut or link), not by producer. The
  wall pass, the residue loops and the mop are all `Cut` moves, so they drew
  blue like the wraps. The straight lines from wrap ends to the tips are the
  wall pass and the residue passes, not wraps.
- **Real spill of the wraps.** A wrap vertex lies on a cell edge between two
  reachable nodes (every corner of its cell is inside the region). It can
  leave the region only where the region edge is concave inside one cell:
  the inset arc of an island, the arc at a reflex corner of the part. The
  sag of a cell chord on an arc of radius `R` is `cell² / (8 R)` = 0.01 mm
  at cell 0.5, R 3. So the real spill of a vertex is about 0.01 mm. It is
  not what the render shows; it is still a legality defect (the planner
  holds a move to the region exactly).

### Fix

1. Render (`tests/adaptive_geodesic_2d_renders_g1.rs`):
   - The harness draws the cleaned PLANNER plan (`Run2d::spans`, new),
     one span per `Cut`, `Link` and `Rapid`, each tagged with its producer
     (`Producer`, new: the kind of the last marker before the segment).
   - Colours and widths: clearing cut blue 1.0 px, starter purple 1.2,
     wall pass green 2.0, residue contour brown 1.6, residue mop magenta
     1.6, keep-down link orange 1.0, rapid grey dashed 0.8, each entry a
     red ring, each step over the pass load a black ring, (a)'s wraps pale
     grey 0.6. The legend is in the SVG caption.
   - The harness writes the PNG itself (`image`, already a dev-dependency)
     from the same mark list as the SVG, so no converter can drop a colour.
     The PNG has no text; the caption is in the SVG and the `.txt`.
   - The SVG is smaller: one `<path>` per span, coordinates at 0.1 px,
     double-quoted attributes.
   - Note: the render is now the planner plan, before the toolpath
     emission (no helix, no arc fit). The shipped columns still assert
     move-by-move parity of the emitted toolpath with `adaptive_toolpath`.
2. Legality (`research_geodesic.rs`, `inside_pieces`): each level chain is
   cut where a vertex is outside `ToolCentreRegion` or a segment leaves it
   (`contains_segment`, the planner's own exact test). A loop that loses no
   vertex stays closed; every other piece is an open arc.
3. Unit test `every_wrap_lies_inside_the_tool_centre_region` (island pocket
   and the demo star): every wrap vertex and every wrap segment is inside
   the region. Tolerance: none. The brief allowed half a cell; the planner
   holds a move to the region with no tolerance, so the test does too.

### What the render should now show

Green wall-pass strokes along every wall and the arm tips; the straight
transitions at wrap ends orange (link) or grey dashed with a red ring
(re-entry). No blue stroke outside the region.

## 3. G1-2 meeting line — the instrument (DONE in code, not run)

- `PassLoadReadout` (research-only) now keeps `over: Vec<OverStep>`: each
  step over the ceiling with `from`, `to`, `reading`, `producer`, `link`
  (link step or cut step) and the segment index. It is no longer `Copy`.
  No planner code changed: the readout replays the cleaned plan on its own
  grid, as before.
- The harness prints one line per over-step under the pass-load line
  (`over: <producer> <cut|link> (segment i) from (x, y) to (x, y), reading
  r`) and marks each with a black ring in the SVG and the PNG.
- This also answers the verifier's open item: for `pocket/geodesic_a`,
  `star/agent` and `pocket/contour_spiral`, the line names the producer, so
  "candidate or shipped code" is decided by the run.

## 4. Prototype measurements (Python, no cargo)

Scripts: `/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/a5fdffa7-9517-40c1-b382-2e96c28e2e27/scratchpad/proto/`
(`field.py`, `weno.py`, `pivot.py`, `star.py`, `star_weno.py`, `pocket.py`,
`pocket_pivot.py`; a session scratchpad, so it can be cleared). No third
shape was added to the harness: `test_data/` has no two-island 2D fixture. They
rebuild the planner lattice (cell `max(R / 6, tol)` = 0.5 mm, origin
`min - cell`), the tool-centre region (part inset by R = 3), the seed (EDT
max) and the disc seed of radius R, and run three solvers. Exact reference:
the Euclidean distance where the seed sees the node; two tangents and the
arc round the inflated island elsewhere.

| Shape | Solver | Max error (cells) | p99 | Note |
|---|---|---|---|---|
| demo star, seed (50, 50.5) | first-order (G1) | 1.74 | — | 0.70-0.80 at 2+ cells from a wall; 1.74 within one cell of a wall; 2 rounds |
| demo star | WENO3 (ZZQ 2005) | 0.79 | — | 0.011 at 2+ cells from a wall (the stencil is cut short at the wall); 48 rounds |
| demo star | source tracking | 0 | 0 | every node sees the seed centre |
| G1 pocket, seed (61, 19) | first-order (G1) | 6.09 | 4.10 | behind the island; 4 rounds |
| G1 pocket | WENO3 | 3.94 | 2.57 | the island-side rarefaction stays first order; 46 rounds |
| G1 pocket | source tracking | 1.14 | 0.75 | never under; 3 rounds |
| unit-test island (r 6, seed (-22, 0)) | source tracking | 0.55 | 0.30 | 0.29 at the test point (12, 0) |

Arm axes of the star (error in cells at 5, 9, ..., 37 mm from the seed):

- first-order: top arm (axis-aligned) 0.00 everywhere; the 19-degree pair
  0.07 rising to 0.49; the 54-degree pair 0.15 rising to 0.84. The mirror
  twins read identical values.
- WENO3: every arm 0.00 to 0.01.
- source tracking: 0.

The fixture star is not regular (`fixtures/demo_star.svg`): its tips lie
45.5, 47.6, 49.8, 49.8 and 47.6 mm from the seed, and its tip angles are
40.3, 39.2, 36.2, 36.2 and 39.2 degrees. It is mirror-symmetric about
x = 50. The G1 seed was (49.5, 50.5) in the run, one cell off the mirror
axis (an EDT-max tie on the lattice).

## 5. G1-3 arm asymmetry (DONE in code, not run)

### Cause: the lead's hypothesis is CONFIRMED in part, and it is not the main cause

- The first-order sweep does have direction-dependent error: 0 on the
  axis-aligned arm, 0.84 of a cell (0.42 mm) on the 54-degree arms at
  37 mm. That moves a wrap end along a wall by about 0.4 mm, a fifth of a
  stepover. The spread is under the lead's one-cell criterion.
- The mirror-twin arms have identical fields in the prototype, but they
  render differently at G1 (the left arm carries the residue mess, the
  right arm a clean ladder). So the field cannot be the main cause of the
  difference between twin arms. The main causes are:
  1. the shape: the arms are of three lengths and three tip angles;
  2. the G1 seed sat one cell off the mirror axis;
  3. the G1 join: arcs alternate direction from the end nearer the cutter,
     so the parity of the alternation, the radial seam steps and the
     replay's splits differ arm by arm, and the residue cleanup takes what
     each arm's re-entries leave.
- The ramp join (section 7) removes cause 3: every arm is cut one way,
  from the rim, by the same rule. Causes 1 and 2 are the fixture's.

### Fix: source tracking (not one of the brief's three options; the reason)

The brief offered (1) WENO3 fast sweeping, (2) a one-cell smoothing of T,
(3) a cell/2 solve. The data rule them out or rank them below a fourth:

- (2) cannot remove a bias that is a smooth function of the direction (it
  is not noise), and `geometry::grid_field::smooth_grid` would spread the
  `INFINITY` of the walls into the field. Rejected.
- (3) halves a first-order error: at best 0.42 cell on the star and about
  3 cells behind the island. Rejected.
- (1) WENO3 is exact enough in the open (0.011 cell) but not behind the
  island (3.9 cells), where R3 Source 3 says a rarefaction pollutes the
  whole downstream field whatever the stencil. 48 rounds against 3.
- **Source tracking** (`FieldSolver::SourceTracking`, `track_sources`):
  each node keeps the point its straight path to the seed comes from and
  reads its distance exactly from it, with the planner's own straight-move
  test (`ToolCentreRegion::contains_segment`). A node that does not see a
  neighbour's pivot takes the neighbour itself as a new bend pivot.
  - Error bound: 0 (rounding) where the seed centre sees the node, which on
    the star is everywhere. Behind a wall: at most one cell diagonal over
    (each bend pivot is a lattice node beside the taut path), measured 0.55
    and 1.14 cells. Never under (every pivot chain is a legal path).
  - Rounds: at most `rows + cols` (the longest lattice path), 3 in the
    prototype.
  - It also names the side of each wall a node's path went round (the
    front branch), which G1-2 needs.
- The first-order solver stays as `FieldSolver::FirstOrder` for the
  comparison (`GeodesicOptions::G1`).

### Tests

- `the_five_arm_axes_of_the_star_read_alike` (new): on the demo star with
  the seed on the mirror axis, the five arm-axis errors at equal distance
  (5 to 33 mm, one stepover apart) spread at most 1e-9 mm under source
  tracking, and at most one cell under the first-order solve (prototype:
  0.73 cell at 33 mm). It reads at least to 25 mm.
- `the_field_on_a_disc_is_the_euclidean_distance` (was `..._within_one_cell`):
  both solvers; source tracking exact to 1e-9 mm, first order within one
  cell. One front branch.
- `the_field_goes_round_an_island`: both solvers; source tracking between
  the exact geodesic and one cell diagonal over it, and at least three
  branches (the seed side and the two sides of the island).
- Both field tests now build the region as the planner does: a part grown
  by R and `ToolCentreRegion` (`GeodesicField::over_part`, which replaces
  `over_region`).

### What the render should show

On the star the five arms are cut by one rule. The arcs in each arm are
concentric circle arcs about the seed (T is exactly Euclidean there).
The arms still differ in length and tip angle, as the fixture does.

## 6. G1-2 meeting line (DONE in code, not run)

### Measurement

- The prototype field of the G1 pocket (`proto/pocket_levels.png`) shows
  clean V-shaped level sets behind the island, with the kink on the meeting
  line. The field does not break down. The G1 breakdown is in the path
  built over the kink: the wrap turns through the kink, the replay splits
  the step there, and the order and the residue passes fill the rest.
- The kink points back to the seed: the stock beyond the wrap has an
  inside corner there, so a cutter that turns through it is over-engaged.
- The planner-level dump of the meeting-line steps needs the run. The
  instrument is in place (section 3); the harness renders both rules
  (`geodesic_a` = `Split`, `geodesic_a_through` = `Through`, both with the
  new solver and join), so one run gives the over-steps with positions
  for each rule.

### Fix: candidate (1), "continue the front from one side only"

`MeetingRule::Split`. Source tracking gives each node its front branch
(the side of each wall its path went round; `Branches`). A cell whose
corners lie in two branches holds the meeting line; `level_chains` gives
no segment there, so each level set ends at the line in two branches. In
the order, the branch cut first runs to the line; when the second branch
of the same level reaches the line (an end within the reach of a cut shock
end), it is trimmed by one stepover. The trimmed piece lies within `√2 · s`
of the first branch's last cutter centre, so it is inside that disc when
`s ≤ R / √2` (2.0 ≤ 2.12 at s 2, R 3): the first branch cut it. The rule is
named in the module doc ("The meeting line").

Candidate (2), a blend of T within a band, was not built: a rounded kink is
still an inside corner of the stock, and a band width that holds the pass
load needs the kink angle, so it is not one derived constant. Candidate (3)
(status quo) is the `Through` column.

Not covered: a meeting of two fronts with no wall between them (a lobe
junction) is one branch and is not split.

### What the render should show

Behind the island, two families of arcs that end on the meeting line from
the two sides, the second family one stepover short of it, and no sawtooth
of short fragments there.

## 7. The plunge per wrap: the ramp join (DONE in code, not run)

### Measurement (why every wrap re-entered)

- Under the swept-width measure a radial step of one stepover into the
  next band reads the chord of the cap of depth s, `2 √(R² - (R - s)²) / D`
  = 0.94 of D at s 2, R 3, against the ceiling 0.3626. So every G1 seam and
  every wall-end step between arcs was refused and became a re-entry: 86 on
  the star, 89 on the pocket (verifier D's run).
- The shipped spiral's join is not a blend: it is the same radial seam step,
  and when the pass load refuses it, a trochoid insert whose loop radius and
  pitch are tuned values (the operator's ruling forbids them). So there was
  no shipped join to reuse.
- A ramp at a constant tilt needs `β ≤ β(s)` = 0.060 rad at full depth,
  33 mm long. A ramp whose tilt follows the depth (`β(d)`, steep where the
  band is still shallow) reaches the full depth in 7.7 mm (9 mm at cell
  steps). The exact swept extent along it stays under the ceiling
  (1.56, 1.89, 2.08, 2.17 mm at depths 0.5, 1, 1.5, 2 mm, against 2.1756).

### Fix: `JoinRule::Ramp` and `EntryLaw`

- The law: `β(d) = φ₀ + asin((c·D - d - R) / ρ)` from the bound
  `d + R (1 - cos β) + h sin β ≤ c·D` on the swept width of a tilted step
  (derivation in the `EntryLaw` doc). Every value comes from R, s and the
  pass ceiling.
- The spiral (editor C's `T/s - φ/2π` item): the levels whose wrap is a
  whole circle round the seed, `⌊(clearance - R) / s⌋`, are one spiral out
  of the starter rim, the radius growing by the law against the previous
  turn, then one closing lap. Source tracking makes T exactly Euclidean
  there, so the spiral is built in closed form about the seed (no contour
  interpolation).
- Every other wrap is entered from its parent (the cut wrap one level lower
  nearest its start, or the closing lap) by a ramp: its points are pulled
  toward the parent until the law's depth reaches the gap.
- One way: an arc's ramp leaves a wedge at its start. The child arc starts
  at that end and first runs back over the parent from its full-depth point
  to its start (the back-run, at most one stepover deep, heading along the
  wedge), then ramps out. A zig-zag order would meet the parent's wedge
  with two stepovers of stock (worked through in the note's derivation:
  the lag at the wall end grows by one ramp every two arcs).
- From an arc's end to the next start the cutter takes a keep-down `Link`
  over cut stock; the replay admits it under the pass load or makes it a
  re-entry.
- Behind an island (two branches) the join rule follows the meeting rule:
  the branch cut first continues; the order picks a child of the wrap just
  cut first, so one branch's lobe is cut to its end before the other.
- Reused: the replay, `subdivide`, `spiral::nearest_index`. No third join
  shape was invented beyond the ramp; the shipped spiral has no blend to
  reuse (above).

### Plunge counts (G1 measured; after PREDICTED, not run)

| Shape | G1 (verifier D) | After, predicted for the clearing producer |
|---|---|---|
| star | 86 entries | 1 (the starter) plus at most one per arm whose first link from the previous arm leaves the region at a reflex corner: 1 to 6. Spiral levels: clearance 16 mm, so 6 |
| pocket | 89 entries | 1 plus at most one per lobe (the two sides of the island, the corners): about 1 to 6. Spiral levels: clearance 10.7 mm, so 3 |

The wall pass, the residue contours and the mop add their own entries
(the summary's over-step lines and the producer colours show where). An
arc shorter than its ramp (at the arm tips) cannot reach full depth; the
next level then over-reads there and the replay splits it, so a few more
re-entries at the tips are possible.

## 8. Every file changed

- `crates/rs_cam_core/src/adaptive/path.rs`: the one source of the planner
  lattice (`PLANNER_CELLS_PER_TOOL_RADIUS`, `PASS_STEP_CELLS`,
  `planner_cell_size`, `pass_step_len`, `pub(super)`), used by its five
  product call sites. Behaviour-neutral.
- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: everything else
  (research only, behind `research`): `FieldSolver`, `track_sources`,
  `Branches`, `Walls`, `GeodesicField::over_part` (replaces
  `over_region`), `MeetingRule`, `JoinRule`, `GeodesicOptions` (`G1`, `C2`),
  `inside_pieces`, the new `ordered_wraps`, `EntryLaw` (ramp, spiral),
  `Producer`, `OverStep`, `PassLoadReadout::over`, `Span2d`, `Run2d::spans`,
  `FieldReport::{branches, spiral_levels}`, `Candidate2d::Geodesic(options)`;
  the module doc; the unit tests (5: disc, island, star axes, pass load,
  wraps inside the region).
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: producer
  colours, the PNG writer, over-step lines, the three candidate (a)
  columns.
- `planning/adaptive_research_2026-10-23/P2_C2_geodesic_fixes.md`: this
  note.

No shipped behaviour changed: `path.rs` keeps the same arithmetic;
`spiral.rs`, `search.rs` and `scallop_isofield.rs` are not edited by this
pass.

## 9. Commands for the verifier

Every command through `scripts/cargo_lane.sh`, with a `timeout`, in the
worktree.

I ran `rustfmt --edition 2024` (not cargo) on the three Rust files one by
one: they parse, and `rustfmt --check` is clean on each. No other file
changed (`tests/common` untouched). Nothing was compiled or linted.

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
cargo test -p rs_cam_core -q --lib adaptive
cargo test -p rs_cam_core -q --test a_clearing_cut_holds_the_pass_load_g_adaptpassload \
    --test a_link_feeds_only_within_the_pass_load_g_adaptlinkload \
    --test a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift \
    --test adaptive_keeps_its_planner_order_g_adaptorder \
    --test adaptive_property_harness --test contour_spiral_gcode_validity_phase0 \
    --test adaptive3d_emission_byte_parity --test adaptive_plans_every_inset_piece_g_adaptpieces
cargo test -p rs_cam_core -q --test the_research_arms_are_feature_gated_fin01
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings
```

Known reds before this pass (verifier D, on master too):
`a_clearing_cut_holds_the_pass_load_g_adaptpassload::a_straight_plunge_entry_is_exempt_only_leaving_its_hole`
and `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift`.

What to read in the harness output (`summary.txt`): for each shape, the
`geodesic_g1` row is the before (it also has the G1-1 clip, so it may differ
from verifier D's numbers by a few steps); `geodesic_a` is the after;
`geodesic_a_through` isolates the meeting rule. Each `over:` line names the
producer and the place of a step over the ceiling. If `geodesic_a` reads
over-steps in `clearing` cuts at ramp ends, the arc was shorter than its
ramp (section 7). If they sit at the meeting line, compare with
`geodesic_a_through` before choosing between candidates (1) and (2).

## 10. Risks for the verifier (code that no compiler has seen)

- `track_sources` calls `contains_segment` for each improving candidate;
  the pocket lattice is about 14 000 nodes, the star about 31 000. In
  debug the unit tests may take seconds.
- The unit test bounds come from the Python prototype, not the Rust code.
  A red bound is a finding: report the number, do not loosen it.
- The ramp's back-run and the one-way order are new logic with no
  geometric unit test of their own; the pass-load unit test
  (`every_emitted_step_holds_the_pass_load`) and the harness are their
  checks.

## C3 — the ramp join regressed the render; diagnosis and decision (2026-10-23)

Data: the verifier's run at bd287bd0 (G1 fixes in 0e79f0fc), copies in the
main checkout `planning/adaptive_research_2026-10-23/phase2/renders2/`.
I read the SVG spans in plan order with a small parser (session scratchpad
`spans.py`, `split.py`); no cargo.

### (1) The grey lines

- Pale grey (`#d0d0d0`, 0.6 px) is NOT a producer. It is `Run2d::wraps`,
  the nominal level sets the producer planned, drawn UNDER the moves in
  every candidate (a) column. Grey dashed (`#999`) is a rapid.
- In `geodesic_g1` the blue cuts lie on the wraps and hide them. In
  `geodesic_a` (ramp join) the wraps show because the emitted cuts did not
  follow them: the producer DID emit every wrap as a `Cut` (in runs of
  back-run + ramp + wrap), but the replay refused their steps, and the
  residue contour and the mop cut the stock instead. Also the closed wraps
  round the seed (levels 1 to 6 on the star, 1 to 3 on the pocket) are not
  in `wraps` under the ramp join: the spiral replaced them, and the spiral
  itself was refused (below).

### (2) Where the entries come from

From the SVGs: each rapid's producer is the producer of the next cut; a
clearing re-entry within 1.5 cells of a planned wrap end is a join, else a
mid-wrap split by the replay.

| Column | Entries | Clearing at a wrap end (join) | Clearing mid-wrap (replay split) | Residue contour | Residue mop | Starter | Wall pass |
|---|---|---|---|---|---|---|---|
| star `geodesic_g1` | 86 | 56 | 14 | 0 | 14 | 1 | 1 |
| star `geodesic_a` (ramp) | 87 | 24 | 34 | 9 | 19 | 1 | 0 |
| pocket `geodesic_g1` | 89 | 41 | 25 | 6 | 16 | 1 | 0 |
| pocket `geodesic_a` (ramp) | 89 | 36 | 38 | 0 | 14 | 1 | 0 |
| pocket `geodesic_a_through` (ramp) | 92 | 35 | 36 | 7 | 13 | 1 | 0 |
| star agent | 25 | — | 17 (all clearing; frontier hops included) | 0 | 5 | 1 | 2 |
| star contour_spiral | 101 | — | 93 | 0 | 7 | 1 | 0 |

The ramp join traded join re-entries (56 -> 24 on the star) for mid-wrap
splits (14 -> 34) and residue work (+9 contour, +5 mop). The harness now
computes this split itself (`Summary2d::entry_split`, `EntrySplit`; the
join radius is one pass step, the largest gap between two replay walk
points) and prints it per column. A frontier hop is not a separate
producer in the plan: it falls under "clearing" for the agent.

### (3) Does the ramp hold the pass load?

No. The plan of `star/geodesic_a` right after the starter (SVG spans 3 to
21) is: a clearing piece of 2.5 mm, a rapid, 2.3 mm, a rapid, 3.2, 4.0,
5.0, 5.8, 5.6 mm, each followed by a rapid outward. That is the spiral,
split by the replay every few steps. No blue spiral survives; a brown
residue contour and the mop clear the centre.

Cause (derived, and it explains the G1 column too): `EntryLaw` bounds the
swept width of a tilted step against a STRAIGHT cleared edge. Round the
seed the cleared edge is a circle, and the stock wraps round the cutter.
For a pass at radius `ρ` one stepover outside a cleared disc of radius
`a = ρ - s + R`, the stock in the cutter's disc spans, across the heading,
from the intersection of the two circles to the disc edge:
`width = R - (a² - R² - ρ²) / (2ρ)`, which is `2 + 4/ρ` mm at s 2, R 3.
It is over the ceiling (2.1756 mm) for every `ρ` below about 23 mm (2.8 mm
at `ρ` = 5). The G1 column shows the same: its first circle (`ρ` = 5) is cut
in six pieces with six entries; only the circles past about 15 mm come out
whole (the lattice reads a cut up to one cell narrow).

The refused-step dump: the harness now replays the producer's RAW plan
through the readout (`Summary2d::raw_pass_load`) and prints the first ten
over-steps with producer, part of the plan (spiral, on a wrap, join) and
reading. The run that wrote renders2 did not have it, so the dump is for
the next run: column `geodesic_a_ramp` keeps the ramp join for exactly
this record. Predicted: its first refused steps are `spiral` steps at
radius 5 to 7 mm, reading about 0.40 to 0.47 of D.

### A second defect found: the meeting rule split the wraps at the shadow lines

`pocket/geodesic_a` had 158 wrap pieces against 83 for
`geodesic_a_through`. The meeting line behind the island crosses at most
about 25 levels, so it cannot add 75 pieces. Cause: `cell_on_shock` marked
every cell whose corners have different front branches. That includes the
two SHADOW lines, where the paths start to bend round the island (branch 0
next to its child branch). The field is smooth there; no fronts meet. So
every level set that crossed a shadow line was cut in two there: more
arcs, more joins, more entries.

Fix: a cell holds a meeting line only when two of its corners lie in
UNRELATED branches (neither is an ancestor of the other: the paths went
round a wall on different sides). The branch table now keeps each
branch's parent (`GeodesicField::branch_parent`, `related`,
`is_ancestor`). The island unit test now asserts that every meeting-line
cell lies behind the island on its axis (within one cell diagonal) and
that at least one exists.

### Decision

1. **The join is REVERTED to the G1 join** (`GeodesicOptions::C2.join =
   JoinRule::Seam`). The ramp join is recorded as "tried, refused by the
   replay": its entry law assumed a straight cleared edge, and the front
   round the seed is a circle (derivation above, and in the
   `JoinRule::Ramp` and `EntryLaw` docs). It stays selectable
   (`geodesic_a_ramp` column) for the refused-step dump only.
2. Kept: G1-1 clipping, G1-3 source-tracked field, the one-side meeting
   rule (with the shadow-line fix above).
3. Not done, the next lever (derived, for the lead to rule on): the
   join and the inner wraps can hold the load only if the wrap spacing
   follows the front's curvature. From the width formula above, a pass at
   radius `ρ` round a cleared disc holds the ceiling `c` when its spacing is
   at most `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))` (1.21 mm at `ρ` = 3,
   1.50 at 5, 1.80 at 10, 1.92 at 15, 1.98 at 20, and s = 2.0 from
   about 23 mm, where the width formula meets the ceiling). With
   source tracking, `ρ` is the distance of a node from its pivot, so a
   field `U` with `dU = dT / s'(ρ)` is well defined in the seen region. It
   would remove the mid-wrap splits on the inner circles (14 on the star,
   25 on the pocket, as measured above) for about 1.3 extra wraps. A ramp
   law built on the same curved-edge width, not the straight-edge one,
   is the condition for any join that removes the join re-entries. Neither
   is built: both change what the operator saw at G1.

### Predicted counts after C3 (not run)

`geodesic_a` = G1 join + source tracking + split meeting rule + clipping.
The star has no island, one front branch, so its clearing plan equals
`geodesic_g1`'s except for the field (exact, so the wraps sit up to
0.84 cell from the G1 ones): about 86 entries, the same split. The pocket:
the meeting rule now cuts only at the meeting line (about one extra arc
per level behind the island, each a join), and the arcs no longer turn
through the kink (fewer mid-wrap splits there). Expect about 89 or fewer;
the run decides. If `geodesic_a` reads MORE entries than `geodesic_g1` on
the pocket, the meeting rule costs more joins than it saves splits, and
`geodesic_a_through` (the same with the G1 meeting rule) is the fallback.

### Files changed by C3

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`: `C2.join =
  Seam`; `JoinRule::Ramp` and `EntryLaw` docs record the refusal; module
  doc "The join and the order" rewritten; `EntrySplit`, `entry_split`,
  `Summary2d::{raw_pass_load, entry_split}`, `StepPart`, `classify_parts`,
  `OverStep::part`; the shadow-line fix (`branch_parent`, `related`,
  `is_ancestor`, `track_sources` returns the parent table); the island
  test's meeting-line check.
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`: the
  `geodesic_a_ramp` column; the entry split line; the first ten refused
  raw-plan steps.
- This note.

Not touched (the release GUI build): `adaptive/path.rs`, `spiral.rs`,
`search.rs`, `scallop_isofield.rs`. `rustfmt --edition 2024 --check` is
clean on both Rust files. No cargo, no commit.

### Commands for the verifier (C3)

Through `scripts/cargo_lane.sh`, each with a `timeout`, after the GUI build
is done:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings
```

Read in `summary.txt`: per column the `entries by producer` line (compare
`geodesic_g1` with `geodesic_a`: the target is fewer, never more), the
`raw plan` block of `geodesic_a_ramp` (the first ten refused steps), and
the pocket's `wrap pieces` for `geodesic_a` (expected near
`geodesic_a_through`'s, not double).

## C4 — the morph join (2026-10-23)

Status: CODE COMPLETE, NOT RUN. No cargo (the brief says so); no commit.
`rustfmt --edition 2024 --check` is clean on both files. Base 26355fe9.
Spec: main-checkout `STATUS.md`, "Next for the geodesic candidate (C4,
after D3): the morph join". Shipped code is not touched.

### Method

1. **Core-zone spiral** (`core_spiral`, `GeodesicField::star_levels`).
   - The star-shape test: rays from the seed-disc rim, spaced so that the
     arc between two rays at the lattice diagonal is one cell, each
     sampled every half cell on the bilinear field (the interpolation the
     level sets use). Level `k` passes when every ray crosses `T = k`
     exactly once, and that crossing comes before the ray first leaves
     the reachable cells. `K` is the largest `k` for which levels 1..k all
     pass. A level set behind an island fails: the rays toward the island
     leave the region before they cross it.
   - The spiral is the level set `F = 0` of `F(x) = T(x)/s − φ(x)/2π`.
     **Branch cut: φ is unwrapped along the curve.** The tracer walks the
     unwrapped angle θ from 0 to 2πK. On the ray at `φ₀ + θ` (φ₀ = the
     starter's end direction, so the spiral starts where the starter
     ends) it takes the radius where `T = θ/2π` stepovers, by bisection
     to the op tolerance. The star-shape test guarantees one crossing per
     ray, so no half-field is necessary. A closing lap on level K
     follows. A spiral that leaves the region drops its outer level and
     is traced again.
   - Angle step: the chord rule of `circle_from` at `R + K·s`.
2. **Lobes** (`morph_plan`, `lobe_link`, `morph_legs`, `LobeMorph`).
   - Arc `a` links to arc `b` when `b` is the one child of `a` and `a` the
     one parent of `b` (the parent relation of the order rule: one level
     apart, within the reach), both are open, and `lobe_link` holds:
     1. the same set of front branches at their vertices;
     2. the length ratio is at most `(R + 2s)/(R + s)`;
     3. every matched pair `A_k(u)`, `A_{k+1}(u)` (one sample per cell of
        the longer arc) is within the reach.
     The links are read on the untrimmed arcs. A run of linked arcs is a
     lobe; every other arc is a plain wrap (the C3 rule).
   - The order: a unit (lobe or plain wrap) is cut after every parent of
     its first arc; the nearest ready unit goes next (the order rule of
     `ordered_wraps`).
   - In a lobe the first arc is cut plain from the end nearer the cutter;
     `p = 0` is where that pass ends. Each later arc is turned to pair its
     ends with the arc before. The one-side meeting trim (`meeting_trim`,
     factored out of `ordered_wraps` with the same arithmetic) applies to
     every arc as it is placed. Then the legs, and the lobe ends with a
     plain pass of its last arc.
   - Check: if any leg segment leaves the tool-centre region (a blend of
     two arcs that end on a curved wall cuts that wall's chord), the lobe
     is cut as plain wraps. `LobeReport::outside` counts this.
3. **Behind an island**: rule 1 above (branch sets) and the split rule
   (one child, one parent) stop a lobe there. The C3 one-side rule
   applies to the arcs that fall back. The counts per reason are in
   `LobeReport` (`stop_split`, `stop_branch`, `stop_ratio`, `stop_gap`).
   As the advisor noted, branch-set equality also stops a lobe at the
   level where a shadow line first crosses the arc ({0} against {0, b});
   that stop is counted under `branch`.
4. **Readout**: `PassLoadReadout::tallies`. Each class of steps has its
   max reading, steps over and steps. The classes of the emitted plan
   are per producer. The classes of the raw plan are per producer and
   part: spiral, morph, return, wrap, join, link. A part is the planned
   polyline within one cell of the step end; a morph step or a return
   step must also run the way of the nearest segment, because a return
   lies on its leg. The raw plan's "steps over" are the steps the replay
   refuses. The S/J/M/C/W/P entry split stays.
   `classify_parts` (the disc test of the ramp spiral) is replaced by
   `PartIndex`, so the ramp column now classes a spiral step by the
   spiral polyline, not by the disc.

### Derivations (only R, s, the cell, the tolerance and the ceiling enter)

- **Leg bookkeeping.** A leg is `B(p, λ) = (1 − λ)·A_k(p) + λ·A_{k+1}(p)`.
  The stock that a leg meets at `p` is its λ minus the largest λ cut at
  `p` so far, in stepovers. The bound is 1, the spacing of every other
  wrap.
  - **The brief's literal (i), one leg per band in alternating
    directions:** the leg back runs λ = 1 − p from the far end and meets
    `2 − 2p`: two stepovers at the near end. An alternation needs two
    legs per band. Selectable as `LobeMorph::AlternateFull`; not a
    default column.
  - **Default `geodesic_morph_alt` = half-pitch alternation:** out with
    λ = p/2, back with λ = 1 − p/2. The leg out meets
    `p/2 + p/2 = p ≤ 1` (the band before left the edge at −p/2 of this
    band), and the leg back meets `1 − p ≤ 1`. Both turns are
    continuous. The last band leaves `A_n` short by p/2, so a plain pass
    of `A_n` follows (it meets ≤ 1/2). "Two" is derived: it is the least
    number of legs for which an alternation holds one stepover. Cost:
    about twice the lobe cut length, at half the mean depth.
  - **`geodesic_morph_return` = the brief's (ii):** a leg out (λ = p,
    meets exactly 1), then a return along it through cleared stock (zero
    load), then the step `A_k(0) → A_{k+1}(0)`. That step is radial: it
    meets one full stepover head-on, which is the seam step of the G1
    join (about 0.94 of D). **It is predicted refused once per band.**
    The last band does not return; `A_n` is cut back from the far end
    (meets 1 − p). The return is a `Cut` along the reversed leg, not
    `Link`s: `feed_link_checked` reads link samples on the lattice with
    the wall test, and a leg ends on the wall, so a link could become a
    re-entry and hide the mechanism. The return length is reported apart
    (`return_length_mm`) and is in the cut length.
- **Length factor** `(R + 2s)/(R + s)` = 1.4 at s 2, R 3. A circular front
  of level k has radius `R + k·s`. An arc of one angular span grows by
  `(R + (k+1)s)/(R + ks)` per level, the largest at the first wrap. This
  is the loosest growth the front itself gives, not a fit. A larger ratio
  either way means the arc gained or lost length at a wall.
- **Matched-gap bound** = the reach `s + (1 + √2)·cell`. **Samples per
  band** = one per cell of the longer arc (the contour has about one
  vertex per cell).
- **Rays**: `⌈2π·L/cell⌉` with L the lattice diagonal; samples every
  cell/2; bisection to the op tolerance.

### Unit tests (new)

- `the_core_spiral_on_a_disc_is_one_piece_with_monotone_radius`: disc of
  tool-centre radius 20, seed at the centre. K lies in
  `⌊(20 − cell − √2·cell − R)/s⌋ ..= ⌊(20 − R)/s⌋` (7..=8). Every segment
  is inside the region. The radius never falls by more than the
  tolerance. The spiral starts at R and ends within one cell of R + K·s.
- `the_lobe_morph_on_a_rectangle_lobe_holds_one_stepover`: 5 straight
  arcs, 40 mm long, s apart. For `Alternate` and `Return`, no two cut
  segments cross, and the legs that run the same way are s ± one cell
  apart. There are 2(n−1) or n−1 legs, and the last piece is the last
  arc. For `AlternateFull`, the leg back is at least 2s − cell from the
  leg before (the doc's claim).
- `the_star_shape_test_rejects_a_level_set_behind_an_island`: disc of
  radius 30 with an island hole of radius 5 at (14, 0). Levels 1 and 2
  pass, levels 4 and 7 fail, and 2 ≤ K ≤ 3. The control is the same disc
  with no island, where levels 4 and 7 pass.

### Predicted entries per column (not run; the run decides)

D3 for reference: star `geodesic_a` 92 = S1 J56 M20 P15, pocket 88 = S1
J42 M27 C4 P14.

- **Core (both morph columns).** The C3 width formula `2 + 4/ρ` mm
  (against 2.18) applies to a pitch-s spiral as it applies to the wraps.
  The D3 dump shows spiral steps at ρ 3.4 to 5.0 reading 0.38 to 0.52,
  at the ramp's tighter pitch. So **the replay still splits the inner
  turns**. The core removes the seam joins (J) but not the mid-turn splits
  (M). I do NOT claim one plunge per core zone. Star: K ≈ 6 (the ramp's
  clearance count was 6); the core holds about 10 to 20 of the old J+M,
  and those become about 10 to 20 M. Pocket: K ≈ 3.
- **Star, `geodesic_morph_alt`.** 5 arms are 5 lobes (level K+1 is 5
  arcs). Count:
  - one entry per arm, because the link from one arm tip to the next
    arm's base crosses the wall: 4 to 5;
  - near each tip the arcs shrink faster than 1/1.4 and fall back to
    plain wraps, with a radial join per tip arc: about 2 to 3 per arm,
    10 to 15 in total;
  - S 1 and P about 15 as before.
  Total about **40 to 55** (D3: 92), J about 15 to 20, M about 10 to
  20. The morph-step load should be at or below the plain-wrap load: a
  leg meets the full stepover only at its ends, and s/2 at mid-arc.
- **Star, `geodesic_morph_return`.** Add one refused near-end step per band
  (about 7 to 10 bands × 5 arms). Total about **80 to 100**, J high. This
  column is expected to lose to the alternation; it is built so the
  readout shows that.
- **Pocket, both columns.** Three front branches. The lobes stop at the
  island (split), at the shadow lines (branch) and at the meeting line
  (branch; the two sides do not merge again), so about 4 to 8 lobes. Alt:
  about **40 to 55** (D3: 88). Return: about **75 to 95**. If
  `LobeReport::outside` is not 0, read it first: lobes that end on the
  island may leave the region by the chord sag (about s²/(8·ρ_island) ≈
  0.06 mm) and fall back to plain wraps whole.
- **Readout targets:**
  - `clearing morph` max at or below `clearing wrap` max;
  - `clearing return` max about 0;
  - `clearing join` steps over ≈ refused near-end steps in the return
    column.

### What the mid-wrap splits (M) look like with the morph (item 7)

Curvature-aware spacing is not built. After the morph, M comes from two
places:

1. The inner core turns (ρ under about 23 mm, the C3 bound).
2. The arm bases of the star, where ρ is about 15 to 23 mm and a leg end
   meets a full stepover.

Both are the `2 + 4/ρ` excess. The C3 §3 field `s'(ρ)` (1.50 mm at ρ 5,
1.80 at 10, 1.92 at 15) is the lever that closes both. In the morph form
it is a T/s' − φ/2π spiral, with no change to the lobe legs. Read the
core's M count in the run before that lever is ruled on.

### Risks

- Code that no compiler has seen: about 1300 new lines.
  - `JoinRule` has a payload variant now; `== JoinRule::Ramp` still
    compiles (`PartialEq` is derived).
  - `level_is_star_shaped` is `#[cfg(test)]`, because only the tests call
    it.
- The star-shape test cost: about 1800 rays × 570 samples on the star (a
  bilinear sample each). That is cheap in release. The unit tests use
  discs of 20 and 30 mm.
- A lobe's legs are checked against the region only as a whole; one bad
  segment drops the whole lobe to plain wraps (counted).
- The fallback "lowest pending unit" in the order loop is not expected to
  fire; if a run shows an odd order, look there first.
- `PartIndex::part` is O(steps × polyline points), with a bounding-box
  filter. It runs on the raw plan only.

### Harness columns

Default: agent, contour_spiral, geodesic_g1, geodesic_a,
geodesic_morph_alt, geodesic_morph_return. `geodesic_a_through` is NOT in
the default run any more (the brief did not name it; D3 has its row) and,
with `geodesic_a_ramp` and `geodesic_morph_alt_full`, runs when
`ADAPTIVE_GEODESIC_EXTRA` is set. A "wrap end" for J in the morph columns is
the end of a planned piece (core spiral, plain pass, leg; not a return),
so a replay split at a leg's turn counts as J.

### Files changed by C4

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`
- this note.

No `pub(crate)` request elsewhere.

### Commands for the verifier (C4)

Through `scripts/cargo_lane.sh`, each with a `timeout`:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
ADAPTIVE_GEODESIC_EXTRA=1 cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture   # the record columns
```

**First check, the regression anchor.** The refactor (`collect_chains`,
`parents_of`, `meeting_trim`, `append_path`) must leave `geodesic_a`
exactly at D3:

| Shape | Entries | Split | Max reading | Raw plan over-steps |
|---|---|---|---|---|
| star | 92 | S1 J56 M20 P15 | 0.3600 | 206 of 2330 |
| pocket | 88 | S1 J42 M27 C4 P14 | 0.3624 | 289 of 2845 |

A difference there is a refactor defect: stop and report it.

Then read, per morph column:

- the `entries by producer` line;
- the `lobes piece 0` line;
- the `load by class, raw plan` block: `clearing spiral`,
  `clearing morph`, `clearing return`, `clearing wrap`, `clearing join`;
- the first ten refused steps.
