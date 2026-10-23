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

## C5 — cascade diagnostic, curvature spacing law (2026-10-23)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base c97093af (C4
committed in 6f699bcf; D4 in `P15_D_verify.md` §D4). `rustfmt --edition
2024 --check` is clean on both files. Shipped code is not touched.

### Measurements I start from (D4, star / pocket, `geodesic_morph_alt`)

- Entries 92 → 69 and 88 → 77. Emitted max 0.9745 on the star, from a
  27 mm residue-contour link (shipped cleanup).
- Raw plan, every predecessor stamped (the raw readout stamps every step,
  so its over-steps are refusals with every step before them cut):

  | Class | Star | Pocket |
  |---|---|---|
  | spiral | 138 / 392 (max 0.50) | 55 / 151 |
  | morph | 51 / 994 (max 0.60) | 123 / 1930 (max 0.71) |
  | wrap | 52 / 651 | 63 / 1287 |
  | join | 28 / 30 | 17 / 28 |

- The first refused spiral steps lie 4.5 to 5.5 mm from the seed, reading
  0.37 to 0.50. The C3 model predicts `(2 + 4/ρ)/6` = 0.43 to 0.48 there:
  the curved-front excess.
- Lobe stops: star split 53, ratio 11; pocket split 51, branch 2, gap 2.
- The coordinator's reading (renders3): whole lobes are cut by the mop; the
  upper two star arms and the left half of the pocket were never cut by
  a clearing producer. Hypothesis: a refusal cascades.

The D4 readout could not test that hypothesis. Its raw grid stamps every
step, refused or not, so it shows only the intrinsic refusals. The replay
stamps only accepted steps.

### (1) The cascade diagnostic (`CascadeReadout`, `cascade_readout`)

- The method walks the raw plan on two grids at once:
  - the **planned** grid stamps every step (the same grid as the raw
    readout);
  - the **replay-model** grid stamps only the steps that hold on it.
- A refused step lifts the model cutter. While it is lifted, a step that
  holds WITHOUT the disc at its start cleared re-starts the cut, and the
  entry hole is stamped (`stamp_entry`; the replay may arrive by a
  keep-down link instead, which stamps the link's corridor, not a hole). A `Link` holds when every step of it holds; otherwise
  it is a re-entry at its end. A `Rapid` stamps its entry on both grids.
- A refusal on the model grid is a **cascade** when the same step holds
  on the planned grid. It is **intrinsic** when the step does not hold
  even with every step before it cut.
- Printed per column:
  - the totals (refusals, cascades, intrinsic);
  - per class (producer and part): refused, cascade, intrinsic, and the
    max planned reading of the intrinsic refusals;
  - the first refusal of each producer piece (a run of steps of one raw
    segment and one part), with how many of those are cascades, and the
    first ten;
  - the first three intrinsic refusals of each class, with the replay
    reading, the planned reading and the distance from the seed.
- Also changed:
  - `PartIndex` now requires the step's midpoint to lie on the part too.
    At C4, a radial join step that ended on the next wrap was classed
    "wrap", so D4's wrap row holds joins. The class counts of C5 are not
    directly comparable with D4's; the totals are.
  - `LobeReport` splits the "split" stop into: two or more children,
    child with two or more parents, closed loop. My reading of the 53 on
    the star: two arms' arcs come within the reach of each other at an
    inside corner, so an arc has two "children". This reading is not
    measured; the new counts test it.

### (2) The curvature spacing law (`Spacing::Curvature`, `curvature_levels`)

Derivation (module doc and `curvature_levels` doc; only R, s and the
ceiling enter):

1. Take a pass at cutter-centre radius ρ, a spacing s' outside the passes
   before it. They cleared the disc of radius `a = ρ − s' + R` about the
   seed.
2. The stock in the cutter's disc lies outside that disc. Across the
   heading it spans from the circles' intersection, at
   `(a² − R² + ρ²)/(2ρ)`, to the far edge at `ρ + R`. That span is the
   swept width: `w = R − (a² − R² − ρ²)/(2ρ)`. A straight edge gives
   w = s'; a circle gives more.
3. Set `w = c·D` and solve: **`s'(ρ) = ρ + R − √(R² + ρ² + 2ρ(R − c·D))`**.
4. The law is read at the OUTER pass (the one that meets the stock):
   `ρ_k = ρ_{k−1} + s'(ρ_k)`. This has the closed form
   `ρ_k = −(R − cD) + √((R − cD)² + (ρ_{k−1} + R)² − R²)`, capped at
   `ρ_{k−1} + s`, from `ρ_0 = R` (the starter's last lap).
5. **It reaches s at `ρ* = s(2R − s)/(2(cD − s))` = 22.78 mm** at s 2, R 3,
   c 0.3626 (cD = 2.1756).

Levels (Python check of the closed form; each has `w = 2.1756` exactly):

| Level | ρ (mm) | Step (mm) |
|---|---|---|
| 1 | 4.44 | 1.437 |
| 2 | 6.03 | 1.593 |
| 3 | 7.73 | 1.703 |
| 4 | 9.51 | 1.781 |
| 5 | 11.35 | 1.839 |
| 6 | 13.24 | 1.883 |
| 7 | 15.15 | 1.917 |
| 8 | 17.10 | 1.945 |
| 9 | 19.06 | 1.968 |
| 10 | 21.05 | 1.986 |
| 11 | 23.05 | 2.000 |

The cost is one extra level out to ρ*.

- **Where it applies.** The law changes only WHICH values of `T` are
  levels; the field and the contouring are unchanged. So the core spiral
  (turn n at level n, `T` linear in the angle between levels) and the
  plain wraps both follow it. From ρ* out (the lobes) the spacing is
  plain.
- **The spiral between levels.** The spacing between turns is the chord
  between two points of `s'`. `s'` is concave: the square root is of a
  quadratic with a negative discriminant, since `|R − cD| < R`. So the
  chord lies under the curve, and every point of the spiral holds the law.
- **Known limit.** The radius is `R + T`, a circular front about the seed.
  Behind a wall the front is a circle about the bend node, tighter than
  that, so the law spaces it too widely there (the pocket behind its
  island). The bend pivots are not used: at a bend, `ρ → 0` gives
  `s' → 0`. The lattice reads a side cut up to one cell NARROW
  (`PassLoad::swept_width` doc), so targeting `c·D` exactly does not need
  a lattice margin. D4's plain-wrap maximum 0.36 is the curvature excess
  at ρ ≈ 25 to 30 (`(2 + 4/ρ)/6`), not lattice noise.

### (3) Keep or drop the morph — the rule, set before the run

D4 already gives the intrinsic rates (predecessors stamped): on the star
the morph has 5.1 % (51/994) against 8.0 % for "wrap" (with joins mixed
in); on the pocket 6.4 % against 4.9 %. That evidence is mixed, so the C5
run decides:

- **Keep the morph** when, in `geodesic_curv_morph`, the `clearing morph`
  class has an intrinsic rate (intrinsic / steps of the raw tally) at or
  below the `clearing wrap` intrinsic rate of the SAME column (same law,
  same plan), AND most of its replay refusals are cascades. The tie-break
  is the `clearing wrap` rate of `geodesic_curv`.
- **Otherwise drop it.** The fallback is the `geodesic_curv` column:
  plain wraps and the C3 join under the same law. That column is built,
  so a fallback needs no code. The `intrinsic:` lines of the morph class
  (replay reading, planned reading, ρ) give the reason.

**(3) is undecided because there is no run.** The fallback is a column
choice; it needs no code.

### Predictions (not run)

| Column | Star (D4) | Pocket (D4) |
|---|---|---|
| `geodesic_a` (control) | 92 (unchanged) | 88 (unchanged) |
| `geodesic_curv` | 70 to 80 | 75 to 82 |
| `geodesic_curv_morph` | 35 to 50 | 45 to 60 |

- **Control `geodesic_a`:** must be D4/D3 exactly (92 = S1 J56 M20 P15,
  0.3600, raw 206/2330; 88 = S1 J42 M27 C4 P14, 0.3624, raw 289/2845).
  The spacing default is Plain and the level values are 0, 1, 2, … .
- **`geodesic_curv`, star:**
  - the inner-circle mid-wrap splits go: M 20 → 0 to 5;
  - the seam joins stay (one per circle), J about 57 with the extra
    level.
- **`geodesic_curv`, pocket:** M 27 → 10 to 15. The rest is behind the
  island, where the law under-protects.
- **`geodesic_curv_morph`, star:**
  - spiral intrinsic refusals 138 → 0 to 10;
  - the cascades fall with them, and the arms are entered on cut stock;
  - entries: S1, core 0 to 3, about 5 arm entries, 10 to 15 tip
    fallbacks, and the residue C+P.
- **`geodesic_curv_morph`, pocket:** the core is only 3 levels at plain
  spacing (about 4 under the law). The rest depends on the lobes behind
  the island.
- **Cascade share.** In the morph columns at plain spacing (re-run by C5),
  I predict most replay refusals are cascades (the coordinator's reading).
  Under the law, the cascade count falls by more than the intrinsic count,
  because the core no longer seeds them.

### Risks

- The replay model is a model. It re-enters at the first step that
  holds and stamps an entry hole. The replay's own keep-down link stamps
  less, and its walk rules (`emit_capped_walk_with`) have more cases. Read
  the cascade/intrinsic split as the diagnosis, and the cleaned plan's
  J/M as the outcome.
- The law uses the circular-front radius about the seed only (above).
- The level values are no longer integers under the law. `level_count`,
  `level_chains`, the star-shape test and the core spiral read them
  through `level_value`. The meeting trim stays one stepover; it is at
  most `√2·s` from the first branch's cutter, which is covered while
  `s ≤ R/√2` whatever the level spacing.
- About 650 new lines that no compiler has seen.

### Harness columns (C5)

- **Default:** agent, contour_spiral, geodesic_g1, geodesic_a (the
  control), geodesic_morph_alt, geodesic_curv, geodesic_curv_morph.
- **With `ADAPTIVE_GEODESIC_EXTRA`:** geodesic_morph_return (moved out of
  the default: D4 shows it worse than the alternation), geodesic_a_through,
  geodesic_a_ramp, geodesic_morph_alt_full.

### Unit tests (C5)

- `the_curvature_spacing_law_puts_each_tight_pass_on_the_ceiling`:
  - `s'(ρ*) = s`, and `s'(10⁶) → cD`;
  - every uncapped level has `w = cD` to 1e-9;
  - the steps never shrink, and each is at most s;
  - a step is capped exactly where `ρ ≥ ρ*`.
- `the_core_spiral_on_a_disc_is_one_piece_with_monotone_radius` now runs
  under both spacings. The bound is generic: the last core level fits in
  the disc, and the next one does not fit inside the read margin.

### Files changed by C5

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`
- this note.

### Commands for the verifier (C5)

Through `scripts/cargo_lane.sh`, each with a `timeout`:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

Read in this order:

1. **The two controls, exactly.**
   - `geodesic_a` as above.
   - `geodesic_morph_alt`: C5 does not change its plan. The `morph_plan`
     match is restructured only to count the split cases, and `PartIndex`
     only relabels steps. So its totals must equal D4: star 69 = S1 J30
     M14 C12 P12, emitted max 0.9745, raw 272 of 2312; pocket 77 = S1 J45
     M11 C9 P11, 0.4517, raw 280 of 3690. Only the per-class rows may move
     (joins out of "wrap").

   A difference in either is a defect: stop and report it.
2. **Diagnostic self-consistency.** Per class, `intrinsic` in the cascade
   block should be close to `steps over` in the raw-plan tally, because
   both read the planned grid. The only legitimate difference is where a
   model restart's entry stamp cleared more than the planned grid at that
   spot. A large gap is a defect in the walk, not a finding.
3. **The lattice quantum.** If `geodesic_curv`'s core circles are still
   refused with planned readings in `(c, c + cell/D]` (0.3626 to 0.446),
   that is the lattice's one-cell quantum (the `sliver_ceiling` term), not
   the model. A spacing margin of one cell would then be the next lever.
   The brief said "no other constant", so it is not built; it waits for a
   ruling.
4. Per column, the `cascade (replay model on the raw plan)` line and
   block. In `geodesic_morph_alt` (plain spacing), what share of the
   refusals are cascades? That share tests the coordinator's reading.
5. `geodesic_curv` against `geodesic_a`: M on both shapes; the
   `clearing spiral` row (none here) and the `clearing wrap` intrinsic
   count.
6. `geodesic_curv_morph`:
   - the `clearing spiral` intrinsic count (predicted near 0);
   - the `clearing morph` intrinsic rate against the same column's
     `clearing wrap` rate, then against `geodesic_curv`'s: the keep/drop
     rule (3);
   - the `intrinsic:` lines of the morph class with ρ.
7. The `lobes piece 0` line: the three split cases.

## C6 — transition join, local spacing, half-cell read (2026-10-23)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base 7d2c8988 (D5).
`rustfmt --edition 2024 --check` is clean on both files. Shipped code is
not touched.

### What D5 gave (start point)

- `geodesic_curv` is the best state: star 73 = S1 J58 M0 P14 (0.3581, raw
  145/2545); pocket 63 = S1 J34 M12 C6 P10 (0.4677, raw 260/2952).
- Star joins: 97 refusals, all intrinsic (planned up to 0.90), so the
  joins are the cost.
- Pocket wraps: 763 refusals, of which 640 are cascades (behind the
  island).
- At D5 the law spiral (only in `curv_morph`) kept 12 intrinsic refusals
  at 4.3 to 6.9 mm, reading 0.36 to 0.40.

### (i) The transition join (`JoinRule::Transition`, `transition_fraction`, `transition_wrap`)

- **Shape.**
  - A closed wrap followed by the closed wrap one level up drifts out
    over its final fraction `f` of arc length, from `T_k` to `T_{k+1}`.
  - Each point of that fraction moves along the ray from its pivot (the
    seed, or the bend node) to `T = T_k + λ(T_{k+1} − T_k)`, with λ from 0
    to 1. The drift ends on the next wrap's seam, so there is no radial
    step.
  - The starter rim is level 0: it drifts out to the first closed wrap.
  - A transition that leaves the region, or whose ray leaves the
    reachable cells, falls back to the seam join and is counted.
- **`f` derived.**
  - The plain spacing leaves the margin `m = cD − w(ρ, Δ)`, where
    `w = R − (a² − R² − ρ²)/(2ρ)` with `a = ρ − Δ + R` (the C5
    circular-front width).
  - The drift kinks the path at the start and the end of the fraction, by
    β with `tan β = Δ/(f·L)`. The lateral drift per pass step of length
    ℓ is `ℓ·tan β`.
  - A step tilted by β into a band against a straight cleared edge costs
    `R(1 − cos β) + h sin β`, with `h = √(R² − (R − Δ)²)` (the `EntryLaw`
    bound). Held to m:
    `β_max = atan2(R, h) + asin((m − R)/√(h² + R²))`.
  - So **`f(ρ) = min(1, Δ/(L·tan β_max))`**. Where m ≤ 0, which is all
    of ρ < ρ\* = 22.78 mm, f = 1. Then the whole wrap drifts: the spiral
    form `T/s − φ/2π`, which has no kink and runs each turn parallel to
    the turn before it.
  - Past ρ\* the margin and L both grow, so f and the sector angle
    `f·L/ρ_wrap` both shrink outward. The sectors therefore nest. This is
    the condition for the spacing to stay ≤ Δ: a wider sector would meet
    the plain wrap two levels down.
  - Worked numbers (s 2, R 3, cD 2.1756):
    - next level at 25 mm: m 0.016 mm, `f·L` = 455 mm, which is longer
      than L, so f = 1;
    - next level at 33 mm: m 0.055 mm, `f·L` ≈ 103 mm against L ≈ 195 mm,
      so f ≈ 0.53.
  - Δ under `CurvatureLocal` is the law's closed form from the wrap's own
    radius (`law_next_radius`), the value `Curvature` uses when the pivot
    is the seed.
- **Open arcs keep the seam join: a result, not an omission.**
  - One-way passes over an arc need an advance of Δ at the start end. The
    arc ends on a wall, so the cutter meets the band head-on there.
  - Alternation at one level per pass meets 2Δ at alternate ends.
  - The only alternation within Δ is the half pitch: the C4 morph,
    dropped by rule at D5.
  - `JoinReport` counts the joins by kind (rim transition,
    closed→closed transition, refused transition, closed→closed seam,
    closed→arc, from an arc). That count is the measured ceiling of this
    mechanism.

### (ii) Local spacing (`Spacing::CurvatureLocal`, `GeodesicField::use_local_spacing`)

- `ρ` is each node's distance from its source-tracking pivot: the centre
  of curvature of the front through it (the seed, or the bend node behind
  a wall).
- The field is replaced by the pass count `U`, solved with `fast_sweep`
  at speed `Δ(ρ) = min(s, s'(ρ))`, so `|∇U| = 1/Δ(ρ)`. The levels are the
  integers. T keeps the branches, meeting lines and pivots.
- Along a straight path from one pivot, U grows by `∫dρ/Δ`. Since `s'`
  grows with ρ, a pass at ρ_k lies at most `s'(ρ_k)` outside the pass
  before it.
- Across a shadow line ρ jumps (the seed circle meets the tighter bend
  circle). The sweep's solution stays continuous there, so the levels bend
  instead of ending.
- The seed nodes hold the rim's speed (a node at the seed centre has
  ρ = 0).
- **Known limits.**
  - The sweep is first order (C2: up to 6 cells behind an island in T).
    The error shifts the levels as a whole, so the spacing changes
    little.
  - Near a bend node `s'(ρ) → ρ·cD/R`, so the levels pack round each
    island corner: more short arcs, so more arc joins. A floor on ρ needs
    a derivation or a ruling; it is the next lever and is not built.
  - `distance_mm` returns `U·s` under this spacing. Only the tests read
    it, under plain spacing.
  - `fast_sweep` stops at 24 rounds. The harness prints
    `local-spacing sweep converged`; false there is a defect signal.

### (iii) Half-cell read (`ReadResolution`, `run_2d_with`)

- The raw readout and the cascade walk read the SAME raw plan at half the
  planner cell. The producer, the shipped cleanup and its replay, and the
  emitted readout stay at `max(R/6, tolerance)`, because the shipped
  cleanup derives its own cell in `path.rs`.
- **Request (not done):** a finer REPLAY needs a `pub(crate)` cell
  parameter on `apply_residue_mop_cleanup` /
  `apply_contour_parallel_residue_cleanup` in `path.rs`.
- `CascadeReadout::near_ceiling` counts the intrinsic refusals with a
  planned reading in `(c, c + cell/D]`, where cell is the read cell.
- Columns `geodesic_curv_fine` and `geodesic_curv_join_fine`. The second
  was added because `geodesic_curv` has no spiral, and the D5
  near-ceiling refusals were spiral steps. The full-turn transitions of
  `geodesic_curv_join` rebuild those steps.

### Columns

- **Default:** agent, contour_spiral, geodesic_g1, geodesic_a (control),
  geodesic_curv (control), geodesic_curv_join, geodesic_curv_local,
  geodesic_curv_local_join, geodesic_curv_fine, geodesic_curv_join_fine.
- **With `ADAPTIVE_GEODESIC_EXTRA`:** geodesic_morph_alt and
  geodesic_curv_morph (dropped by rule at D5), geodesic_morph_return,
  geodesic_a_through, geodesic_a_ramp, geodesic_morph_alt_full.
- The summary now has:
  - a `joins piece` line;
  - one `transition:` line per transition (level, ρ, Δ, f, sector,
    applied);
  - `local-spacing sweep converged`;
  - the read cell and the near-ceiling count on the cascade line;
  - the step class `clearing transition`.

### Unit test (new)

`the_transition_join_nests_its_sectors_on_a_disc` (tool-centre disc of
35 mm, beyond ρ\*), under both `C6_JOIN` and `C6_LOCAL_JOIN`:

- one rim transition, at least 2 closed→closed transitions, no fallback;
- 0 < f ≤ 1, and f = 1 wherever ρ < ρ\*;
- some f < 1 (the margin branch runs);
- each sector ≤ the one before, plus `cell/ρ_wrap` (the contour gives a
  wrap's length to about one cell).

### Predictions (not run)

| Column | Star (D5 73) | Pocket (D5 63) |
|---|---|---|
| geodesic_curv_join | 66 to 75 | 58 to 66 |
| geodesic_curv_local | 70 to 76 | 55 to 75 |
| geodesic_curv_local_join | ≈ curv_local minus the closed joins | ≈ curv_local minus 3 or 4 |
| *_fine | = its control, exactly | = its control, exactly |

- **geodesic_curv_join, star.**
  - The star's closed levels reach about 15 mm (about 7 levels: the rim
    plus 6 closed→closed transitions). So the transition can take about
    7 of 58 J; the rest are arc joins (5 arms).
  - J 58 → about 50.
  - **The M side:** inside ρ\* every transition is a full-turn drift,
    which is the spiral D5 read with 12 intrinsic refusals. So M 0 → 0
    to 12. Net entries 66 to 75: maybe no gain.
  - The class rows shift: those steps move from `wrap` to `transition`.
    Compare totals.
- **geodesic_curv_join, pocket.** About 3 to 4 closed levels. J 34 → 30
  to 31; M + 0 to 6.
- **geodesic_curv_local, star.** No island, so every pivot is the seed.
  The only difference from `geodesic_curv` is the first-order sweep
  against the exact remap: within a few entries (70 to 76). A larger gap
  is the solver, not the law.
- **geodesic_curv_local, pocket.** Levels pack behind the island's
  corners: more wrap pieces and more arc joins (J up), but fewer cascades
  than 640 of 763 wraps. Entries 55 to 75 (wide). Read `converged` first.
- **The fine columns.** Entries, split and emitted max equal their
  controls exactly; only the raw and cascade rows may move. The quantum
  claim is that `near_ceiling` empties at the half cell (its band is half
  as wide). It is not that the totals fall. If the 0.36–0.40 transition
  steps of `curv_join` leave the band at half cell, the cause is the
  measure's quantum. The next decision is the lead's (a finer planner
  grid, or a one-cell margin). If they stay, the excess is real stock (the
  drift).

### What C6 changes on the controls' path (each change has no effect on the plan)

- `Wrap.level`: a new field, set from the chain; nothing in the seam or
  ramp path reads it.
- Join counting: a counter beside the unchanged `join(&w.pts, …)` call; the
  seam path emits the same segments.
- `track_sources` returns the pivots: added output, the same T, branches
  and parents.
- The `PartIndex` `Transition` class: only `moved` paths are added, and
  the seam columns have none.
- The readouts take their cell as a parameter: `planner_cell` for every
  default-resolution column.

### Commands for the verifier (C6)

Through `scripts/cargo_lane.sh`, each with a `timeout`:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy -p rs_cam_core --all-targets -- -D warnings
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

**Exact-match conditions (a difference is a defect: stop and report):**

1. `geodesic_a`: star 92 = S1 J56 M20 P15, 0.3600, raw 206/2330; pocket
   88 = S1 J42 M27 C4 P14, 0.3624, raw 289/2845.
2. `geodesic_curv`: star 73 = S1 J58 M0 P14, 0.3581, raw 145/2545; pocket
   63 = S1 J34 M12 C6 P10, 0.4677, raw 260/2952.
3. `geodesic_curv_fine`: entries, split and emitted max equal
   `geodesic_curv`'s.
4. `geodesic_curv_join_fine`: entries, split and emitted max equal
   `geodesic_curv_join`'s.

**Then read:**

1. The `joins piece` line of `curv_join` and `curv_local_join` (the
   ceiling of the mechanism).
2. Their `transition:` lines (f = 1 inside ρ\*; sectors nest).
3. `clearing transition` in the raw tally and the cascade block.
4. `near_ceiling` at the planner cell against the half cell, for both
   fine columns.
5. `local-spacing sweep converged`.
6. The pocket cascades of `curv_local` against 640 of 763.

## C7 — the same plans judged by area per mm (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base 7c75324d (D6).
`rustfmt --edition 2024 --check` is clean on all four files. The brief
allowed edits to `path.rs` and `search.rs` for one threading only.

### The measure and its ceiling (no new constant)

- **Measure** (`search.rs`, `compute_area_per_mm`): the area of standing
  stock a step newly clears, per mm of travel. That is FreeCAD's quantity
  (R1 §3); their target 0.98948·s·D is not used.
  - The new region is the capsule of the tool radius about the step,
    outside the disc at its start (where the cutter already stands). This
    is a little wider than the end disc minus the start disc that the
    brief named: it includes the two side slivers between the discs.
  - Each standing lattice node in that region counts cell². Each standing
    fringe sub-point counts (cell/SUB)². The sum is divided by the step
    length.
- **Ceiling.** For a straight side cut of width w, every line parallel to
  the heading inside the band meets the new region in the step length
  (Cavalieri). So the area per mm is w, the swept width. The swept-width
  ceiling fraction c (0.3626 of D) is therefore the SAME ceiling on area
  per mm: `c·D` = 2.1756 mm²/mm.
  - `PassLoad::area_per_mm` is the swept-width load with the step read in
    area (same target, band and ceiling; no sliver reading, because the
    area counts the fringe sub-points itself).
- **Quantum.**
  - A node stands for its cell, so the count errs only in the cells the
    region's boundary crosses: at most `√2·cell·P` mm² for perimeter P,
    so at most `√2·cell·P/ℓ` per mm.
  - On a straight cut along a lattice axis with ℓ a whole number of cells,
    the start and end discs are the same node set shifted. The reading is
    then the band's rows times the cell, within one cell of w.
  - Thin caps under-read: the lattice reads 2.0 mm² for the first
    millimetre of the wall step below, where the exact cap is 3.10 mm².
    This is the same G-CELLREAD direction D6 found for the swept width.

### The threading (path.rs, search.rs; no shipped behaviour change)

- **`search.rs`:**
  - `StepMeasure::AreaPerMm` and `PassLoadMeasure { SweptWidth, AreaPerMm }`
    (`pub(crate)`).
  - `PassLoad::area_per_mm` and `compute_area_per_mm`.
  - `measure_step` returns the area reading over D first when the load's
    measure is `AreaPerMm`.
  - `measure_engagement` reads a single point as the swept width under
    both measures (a point has no travel; this serves presence and
    scoring).
  - `caps()` is true for both measures.
  - The `AreaPerMm` variant carries
    `#[cfg_attr(not(feature = "research"), expect(dead_code, ..))]`. Only
    the research harness builds it.
- **`path.rs`:**
  - `LinkLoad::new` calls the new `LinkLoad::new_with(.., measure)` with
    `SweptWidth`.
  - `apply_residue_mop_cleanup` and `apply_contour_parallel_residue_cleanup`
    keep their signatures and call new `_with(.., measure)` variants with
    `SweptWidth`. The bodies moved unchanged; only their `LinkLoad::new` became
    `new_with(.., measure)`.
  - Every ceiling decision in the replay, the residue contours, the wall
    pass and the mop goes through `measure_step` or
    `step_within_pass_load` with `link_load.pass`. One measure on the
    `LinkLoad` therefore reaches all of them.
- The callers in `mod.rs` and `adaptive3d/clearing.rs` are unchanged.

### Research side (research_geodesic.rs, the harness)

- `ReplayMeasure` (the public twin of the crate enum), and
  `run_2d_with(.., read, measure)`. The plan is the same whatever the
  measure; the cleanup runs `_with(measure)`.
- `PassLoadReadout` and `LoadTally` read BOTH measures for every step,
  side by side: `max_swept`/`over_swept` and `max_area`/`over_area`. Their
  `max_reading`, `steps_over` and over-step list are in the replay's
  measure. The cascade model reads in the replay's measure.
- **Columns.**
  - `geodesic_curv_area`: the plan of `geodesic_curv`, with the replay
    under area.
  - `agent_area` and `contour_spiral_area`: the shipped plans, replay under
    area. They show the measure is no gift to one candidate. No product
    parity is asserted for them.
  - The default set otherwise is as at C6.

### The claim the ruling rests on: it FAILS on the wall-end turn

The brief asked for a test that, at a wall-end turn, swept width reads ≥ 2s
while area per mm reads ≤ c·D.

- **Exact geometry.** A pass at level k reaches the wall and steps up it
  by s, head-on into the stock above `y_k + R`. A step of h head-on clears
  the cap of height h of the cutter's disc:
  `R² acos((R − h)/R) − (R − h)√(2Rh − h²)`.
  - That is 3.10 mm² for the first millimetre and 5.15 mm² for the second
    (R 3), against 2.18 mm² per mm.
  - In general, a head-on step into uncut stock removes about the full
    chord per mm once it is deeper than a few tenths of a millimetre.
- **Lattice (Python emulation of both measures, session scratchpad
  `c7_measures.py`).**

  | Step | Swept width (mm) | Area per mm (mm²/mm) | Ceiling c·D 2.1756 |
  |---|---|---|---|
  | straight side cut, w 2 | 1.5 | 2.0 | both under |
  | wall step, first 1 mm | 3.0 | 2.0 (thin cap under-read) | swept over, area under |
  | wall step, second 1 mm | 5.0 | 5.0 | both over |

So the area measure does NOT admit the radial advance at a wall end.

**An identity: the two measures are equal on every circular front.** Round a
cleared disc, the area per mm of a pass at ρ with spacing s is
`((ρ + R)² − (ρ + R − s)²)/(2ρ) = s + s(R − s/2)/ρ`. The C3 swept width
`R − (a² − R² − ρ²)/(2ρ)` with `a = ρ − s + R` expands to the same
expression, for every ρ, R and s. So the area measure cannot relax the core
or the arm bases either: the curvature spacing law is the same under both
measures. The two measures part where the stock in the disc is
not one band: two slivers either side, a lump the step only grazes, the
first touch of a head-on step. There the swept width reads the span and the
area reads the removal.

### Unit tests (C7, research module)

- `a_straight_side_cut_reads_alike_in_both_measures`: both readings are
  within one cell of s and of each other.
- `a_head_on_step_up_a_wall_is_over_the_ceiling_in_both_measures`: the
  second step reads a swept width ≥ 2s AND area per mm > c·D, and the mean
  area per mm of the whole advance is > c·D. This is the opposite of the
  brief's claim, pinned with the derivation above. The first step's thin
  cap is not asserted (lattice under-read).
- `two_slivers_read_wide_in_swept_width_and_narrow_in_area`: slivers
  2.5 mm either side of the path read a swept width ≥ 2s and area per
  mm ≤ c·D. This is where the measures part.

### Predictions (not run)

- **Lattice effect, set before the run (G-CELLREAD on the area measure).**
  The replay splits each radial join (s = 2) into two pass steps of 1 mm.
  The lattice reads the first one's thin cap at 2.0 mm²/mm (exact 3.10), so
  the area replay ADMITS the first sub-step and refuses the second.
  Expected:
  - J unchanged (the refusal is still within one pass step of the wrap
    end);
  - the raw intrinsic refusals in the `clearing join` class about halve;
  - the emitted plan carries a 1 mm stub into each join.

  That is the lattice under-reading a thin cap, not the area measure
  admitting the join.
- **`geodesic_curv_area` ≈ `geodesic_curv`** (73 / 63). The joins (J, head-on
  radial steps) stay refused: their planned swept readings at D5/D6 are
  0.50 to 0.90, and their area readings are of the same size by the cap
  argument. M stays about 0 on the star, because the curved front reads
  the same in both measures. Residue mop and wall-pass entries (P, W) may
  fall: those steps graze slivers, where area reads less than swept
  width. Expect J unchanged, P down by a few.
- **`agent_area` and `contour_spiral_area`:** the same direction. Fewer
  refusals where the shipped replay split on slivers (contour_spiral M was
  93 on the star), and no change on head-on steps.
- **Readout check:** in every column, `area max` ≤ `swept max` on most
  classes, with near-equality on clearing wraps (straight and curved
  fronts) and the largest gap on the residue mop and the wall pass.

If these hold, the operator's ruling is not between two measures that
disagree about the joins. The measures agree on the joins. They disagree
only on sliver work.

### Commands for the verifier (C7)

Through `scripts/cargo_lane.sh`, each with a `timeout`:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core -q --lib adaptive
# the adaptive folder's sentries (adaptive/CLAUDE.md), path.rs and search.rs changed:
cargo test -p rs_cam_core -q --features test-support --test a_clearing_cut_holds_the_pass_load_g_adaptpassload
cargo test -p rs_cam_core -q --features test-support --test a_link_feeds_only_within_the_pass_load_g_adaptlinkload
cargo test -p rs_cam_core -q --features test-support --test a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift
cargo test -p rs_cam_core -q --features test-support --test adaptive_keeps_its_planner_order_g_adaptorder
cargo test -p rs_cam_core -q --features test-support --test adaptive_property_harness
cargo test -p rs_cam_core -q --features test-support --test contour_spiral_gcode_validity_phase0
cargo test -p rs_cam_core -q --features test-support --test adaptive3d_emission_byte_parity
cargo test -p rs_cam_core -q --features test-support --test adaptive_plans_every_inset_piece_g_adaptpieces
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo clippy -p rs_cam_core --all-targets -- -D warnings
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
cargo test -p rs_cam_viz -q --lib
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

- **The threading proof:** `--lib adaptive` and the eight adaptive
  sentries green (byte parity among them).
- **If the straight-cut unit test reads a swept width of 1.0 or 2.0 instead
  of 1.5,** that is the `MaterialGrid` origin or boundary convention, which
  my emulation assumed. Report the two readings before anything else. Clippy must be green both WITHOUT
  `research` (the `expect(dead_code)` on `PassLoadMeasure::AreaPerMm`
  fires there) and with it.
- **Controls (exact, as D6):**
  - `geodesic_a`: star 92, pocket 88;
  - `geodesic_curv`: star 73 = S1 J58 M0 P14 (0.3581, raw 145/2545),
    pocket 63 = S1 J34 M12 C6 P10 (0.4677, raw 260/2952);
  - agent and contour_spiral keep product parity (the harness asserts it).
- **Then read** for the three `_area` columns:
  - entries by producer;
  - the `both measures of the emitted steps` line (area max must be at or
    under the ceiling where the replay held area; the swept width beside
    it);
  - the per-class tallies (area max against swept max, class by class).
- **The unit test the ruling rests on:**
  `a_head_on_step_up_a_wall_is_over_the_ceiling_in_both_measures`. If it
  is RED, the lattice says area admits the wall step and my derivation is
  wrong. Report the readings.

## C8 — area columns parked; the lead-in join (2026-10-24)

Status: CODE COMPLETE, NOT RUN (no cargo; no commit). Base 6d74feb9 (D7).
`rustfmt --edition 2024 --check` is clean.

### Area columns (D7: not evidence)

- `geodesic_curv_area`, `agent_area` and `contour_spiral_area` moved to
  the `ADAPTIVE_GEODESIC_EXTRA` set.
- **Floor:** `compute_area_per_mm` divides by `max(ℓ, cell)`. The lattice
  cannot resolve travel under one cell, so the quantum error is bounded at
  `√2 · P` per mm. This is a one-line edit to research-only code in
  `search.rs`; no shipped path builds the area measure. Nothing more is
  spent on the area columns.

### The lead-in join (`LobeMorph::Lead`, column `geodesic_curv_lead`)

`GeodesicOptions::C8_LEAD` uses the curvature law (C5), the core spiral
(C4, under the law) and lead-ins.

**Why not the brief's literal form.** One direction, a return link to wall
A, then a lead-in tangent to arc k+1 at its START: the lead-in arc lies
before its tangent point, so a lead-in tangent at the start begins beyond
wall A. A lead-in tangent further along leaves an uncut corner wedge at A.
The next band's lead-in then starts on stock the band before never cut,
and the wedge grows by one lead-in span per band.

**The built form (enter mid-arc, cut both ways).** Per lobe band, the
cutter stands on arc k at its end E:

1. A keep-down link along arc k (cut, zero load) to the lead-in start.
2. The lead-in: a circular arc of radius ρ_L, tangent to arc k+1 at P, on
   the cleared side. Its bite grows from 0 (its start lies on arc k,
   checked within one cell) to Δ.
3. Arc k+1 cut from P to its far end F.
4. A keep-down link back along that cut, to P.
5. Arc k+1 cut from P to E. The stock beside it there is the wedge the
   lead-in left; its bite grows from 0 at P, with the heading tangent, so
   the width is at most `w(ρ, Δ)`.

The cutter ends at E of arc k+1, and the next band starts there. No step
meets a band head-on, and no entry is at a wall.

- P is mid-arc, or further from E where the lead-in's span needs it. The
  band falls back to the seam join (counted) when the span does not fit
  the arc, there is no margin, or a piece leaves the region. After a
  fallback the cutter stands at F, so the next band reads the arcs from
  there.
- **Links.** Each `Link` piece is emitted as one keep-down `Link` per
  vertex along the cut arc. The shipped check (`feed_link_checked`: region,
  legality, `step_within_pass_load` per step) judges each, with no new
  rule. A refused link becomes a re-entry, as everywhere.
- **Closed wraps not in the core** (`lead_closed`): a link back along the
  lap just cut to the lead-in start, a lead-in tangent to the next loop at
  its point nearest the seam (the lap direction is kept), then the lap from
  there. The one-side meeting rule is unchanged (arcs are trimmed at
  placement as before).

### The lead-in radius (derived; only R, s or Δ, and c·D enter)

- At angle θ before the tangent point, the bite is `b = Δ − ρ_L(1 − cos θ)`
  and the heading is tilted θ into the stock.
- A step tilted θ into a band of depth b sweeps
  `b + R(1 − cos θ) + h(b) sin θ`, with `h(b) = √(R² − (R − b)²)` (the
  `EntryLaw` bound). With `h(b) ≤ h(Δ)` this is at most
  `Δ + h sin θ − (ρ_L − R)(1 − cos θ)`.
- Its largest value over θ is `Δ + √(h² + B²) − B`, with `B = ρ_L − R`.
- On a curved front the plain pass already reads `w(ρ, Δ)`, so the margin
  is `m = c·D − w(ρ, Δ)`.
- Held to the margin: **`ρ_L ≥ R + (h² − m²)/(2m)`**.
- **At the tangent point** the reading is the plain pass's
  `w(ρ, Δ) ≤ c·D` (the brief's bound). The binding points are just before
  it, where the tilt grows linearly and the bite falls only quadratically.
- **The tool radius does not satisfy the bound:** it needs `m ≥ h`. At s 2,
  R 3 a lead-in of radius R reads 3.67 mm against 2.18 mm (numerical check).

**The frame: the bound above is in the STRAIGHT frame.** On a front of
radius ρ (about the pivot), a lead circle of radius `ρ_L < ρ` tangent
inside it has, at angle θ:
- a tilt against the front of `ψ = θ(1 − ρ_L/ρ)` (the angle between the
  lead radius and the front radius through the point);
- a bite of `b = Δ − ρ_e ψ²/2`, with `ρ_e = ρ_L ρ/(ρ − ρ_L)`.

So the straight bound applies to `ρ_e`:
**`ρ_L ≥ ρ_min·ρ/(ρ + ρ_min)`** (`lead_in_radius_pivot`, always under ρ).
The circle reaches the cleared band only when `ρ_L ≤ ρ − Δ/2`.

The lead-in's start is not taken from a formula. It is traced: walk the
lead circle out from P one cell at a time until a point lies within one
cell of arc k, giving up past a right angle. The span used in the fit test
is measured from that traced start.

(The advisor caught the first version, which built the circle in the
straight frame. On these fronts that put the start in uncut stock, so
every band would have fallen back.)

Values, pivot frame, exact circles (Python), s 2, R 3, cD 2.1756:

| Front radius ρ | Straight ρ_min (mm) | Pivot ρ_L (mm) | Worst width (mm) | Start angle (rad) | Span P→start (mm) |
|---|---|---|---|---|---|
| straight | 25.7 | 25.7 | 2.167 | 0.40 | 9.9 |
| 60 | 39.7 | 23.9 | 2.1725 | 0.53 | 12.5 |
| 45 | 49.1 | 23.5 | 2.1736 | 0.60 | 13.9 |
| 35 | 68.2 | 23.1 | 2.1746 | 0.72 | 16.3 |
| 30 | 97.6 | 23.0 | 2.1751 | 0.88 | 19.4 |
| 25 | 259 | 22.8 | 2.1755 | 1.53 | 31.6 |
| < ρ\* = 22.8 | none (law: no margin) | — | — | — | — |

### Readout

- The entry split gains **L**: a clearing re-entry within one pass step
  of a planned lead-in (a refused lead-in step), counted before the join
  test.
- Class `clearing lead` in the tallies and the cascade block.
- A `lead-ins piece` line: lobe bands with a lead-in and those that fell
  back; closed wraps with a lead-in and those that fell back; the planned
  link length; the lead-in radius range.
- The emitted keep-down link count and length are the existing `keep-down
  links` line: **the cost to report**.

### Unit tests (C8)

- `the_lead_in_radius_holds_the_ceiling_on_a_straight_band`:
  - the radius equals the closed form (front radius 1 km);
  - the worst tilted-step width along the lead-in is ≤ c·D;
  - a lead-in of the tool radius reads over c·D;
  - a front inside ρ\* gets no lead-in.
- `the_pivot_frame_lead_in_holds_the_ceiling_on_curved_fronts`, on exact
  circles at ρ 30, 45 and 60 mm:
  - `ρ_L < ρ − Δ/2`;
  - the circle reaches the band before a right angle;
  - at every sample, `b + R(1 − cos ψ) + h(b) sin ψ + (w(ρ, s) − s) ≤ c·D`.

### Predictions (not run)

- **Star.** The arms' arcs lie at about 16 to 45 mm. Inside ρ\* the law
  leaves no margin, so there is no lead-in. Beyond it the span (14 to
  32 mm) is long against the arm widths, so few arcs can host P with the
  span behind it.
  - Expect `lead_fallback` ≫ `lead_ins`. The lobes are also few: D5
    `curv_morph` had 6 lobes of 34 arcs and 60 plain wraps, and most splits
    were "two children" at the inside corners.
  - Entries about 75 to 90 (`curv_morph` 84; `curv` 73).
  - The target, one plunge per lobe plus one for the core, is NOT
    expected. The two causes are the lobe split rule and the lead-in span
    at this stepover.
- **Pocket.** The arcs run across the rectangle at ρ up to about 60 mm:
  longer arcs and a larger margin, so some lead-ins fit. Expect
  `lead_ins` about 5 to 20, L small, J below `curv_morph`'s 60. Entries
  about 60 to 80.
- **Core spiral.** As at D5: 12 intrinsic refusals at 4.3 to 6.9 mm, so M
  about 12 on each shape.
- **The cost.** The planned link length is about one arc length per lead-in
  band (E→Q plus F→P).
- **M has two sources in this column.**
  - The core spiral's inner refusals.
  - Refused keep-down links: a refused `Link` becomes a `Rapid` at the
    link's next vertex, mid-arc on arc k, which the split counts as M, not
    L.

  The planned link count (one `Link` per vertex of every `Link` piece)
  against the emitted `keep-down links` count tells the two sources apart.
- **If L is high,** the replay refuses lead-in steps; read the `clearing
  lead` row and its intrinsic refusals. The bound uses the straight-edge
  `EntryLaw` and the circular excess, both upper bounds, so a lead-in
  refusal points at the lattice (G-CELLREAD) or at a link refusal before
  it (a cascade).

### What would make it fit (for the lead; not built)

The lead-in span is `≈ √(2Δρ_L)`, and ρ_L grows as the margin shrinks. A
smaller lobe spacing (a margin reserved for the lead-in) or a variable-tilt
ramp (the `EntryLaw` ramp, 7.7 mm on a straight edge at full depth) would
shorten it. Both change the spacing or the shape the operator saw, so both
are rulings.

### Files changed by C8

- `crates/rs_cam_core/src/adaptive/research_geodesic.rs`
- `crates/rs_cam_core/tests/adaptive_geodesic_2d_renders_g1.rs`
- `crates/rs_cam_core/src/adaptive/search.rs`: one line, the floor in the
  research-only `compute_area_per_mm`, plus its doc. No shipped path
  builds the area measure.
- this note.

### Commands for the verifier (C8)

Through `scripts/cargo_lane.sh`, each with a `timeout`:

```text
cargo fmt --all -- --check
cargo test -p rs_cam_core -q --lib adaptive
cargo test -p rs_cam_core -q --features test-support --test adaptive3d_emission_byte_parity
cargo test -p rs_cam_core --features research,test-support -q --lib research_geodesic
cargo clippy -p rs_cam_core --all-targets -- -D warnings
cargo clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
cargo test -p rs_cam_core --features research,test-support --release \
    --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
```

- **Controls (exact):** `geodesic_a` star 92 / pocket 88; `geodesic_curv`
  star 73 = S1 J58 M0 P14 (0.3581, raw 145/2545), pocket 63 = S1 J34 M12
  C6 P10 (0.4677, raw 260/2952); agent and contour_spiral product parity.
- **Read for `geodesic_curv_lead`:**
  - the `entries by producer` line (S, L, J, M, …);
  - the `lead-ins piece 0` line (lead-ins against fallbacks, the link
    length, the radius range);
  - `keep-down links` (count and mm);
  - the `clearing lead` and `clearing link` classes (max swept width, over);
  - the cascade block.
